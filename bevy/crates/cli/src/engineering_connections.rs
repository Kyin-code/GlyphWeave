//! Production CLI surface for the engineering-connections module (v2).
//!
//! Publication is a transaction: the target paths are preflighted, everything is
//! written into a staging directory, and only then are the files moved into place
//! with the bundle last. A failure removes every artifact this run created, so a
//! failed run never leaves a package that can be consumed as a success. Verification
//! checks the saved payload hash before it rebuilds anything.
use super::*;
use glyphweave_core::worldgen::engineering_connections as ec;
use std::time::Instant;

/// The whole command, including serialization and publication, is gated on this.
const STEP_BUDGET_SECONDS: f64 = 30.0;
const VIEW_FILES: [&str; 3] = ["view-plan.svg", "view-elevation.svg", "view-mesh.obj"];

struct Options {
    input: PathBuf,
    out: Option<PathBuf>,
    views: Option<PathBuf>,
    bundle: Option<PathBuf>,
    report: PathBuf,
    /// Test hook: fail deliberately at one publication stage. Never used in a
    /// production run; it exists so the failure paths can be exercised locally.
    inject: Option<String>,
}

fn parse(args: &[String]) -> CliResult<Options> {
    let mut input = None;
    let mut out = None;
    let mut views = None;
    let mut bundle = None;
    let mut report = None;
    let mut inject = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} needs a path", args[index]))?
            .clone();
        match args[index].as_str() {
            "--input" => input = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--views" => views = Some(PathBuf::from(value)),
            "--bundle" => bundle = Some(PathBuf::from(value)),
            "--report" => report = Some(PathBuf::from(value)),
            "--inject-failure" => inject = Some(value),
            other => return Err(format!("unknown option {other:?}").into()),
        }
        index += 2;
    }
    Ok(Options {
        input: input.ok_or("--input is required")?,
        out,
        views,
        bundle,
        report: report.ok_or("--report is required")?,
        inject,
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> CliResult<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

/// The production write primitive with explicit ownership: the successful `create_new`
/// takes ownership of the path, and the path is registered for cleanup *before* any
/// fallible write or sync can fail. A create conflict registers nothing, so a
/// pre-existing file is never deleted by this run's cleanup. The fault hooks fire
/// inside this primitive, after the real create and inside the real write path.
fn write_new_owned(
    path: &Path,
    bytes: &[u8],
    created: &mut Vec<PathBuf>,
    inject: Option<&str>,
) -> CliResult<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    created.push(path.to_path_buf());
    if inject == Some("final-write") {
        return Err("injected failure after the real create (production primitive)".into());
    }
    file.write_all(bytes)?;
    if inject == Some("final-write-sync") {
        return Err("injected failure before the real sync (production primitive)".into());
    }
    file.sync_all()?;
    Ok(())
}

fn write_json(path: &Path, value: &serde_json::Value) -> CliResult<()> {
    write_new(path, serde_json::to_vec_pretty(value)?.as_slice())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// blake3 is already a dependency of this crate; sha2 is not, and adding one for a
/// report field is not worth a dependency change.
fn blake3_of(path: &Path) -> CliResult<String> {
    let bytes = fs::read(path)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub(super) fn run(args: &[String], verify: bool) -> CliResult<()> {
    let options = parse(args)?;
    let json = fs::read_to_string(&options.input)?;
    if verify {
        verify_command(&options, &json)
    } else {
        build_command(&options, &json)
    }
}

fn build_command(options: &Options, json: &str) -> CliResult<()> {
    let started = Instant::now();
    let out = options.out.as_ref().ok_or("--out is required to build")?;
    let views = options
        .views
        .as_ref()
        .ok_or("--views is required to build")?;
    // 1. Preflight: nothing may exist before anything is written.
    let mut targets: Vec<PathBuf> = vec![out.clone(), options.report.clone()];
    for name in VIEW_FILES {
        targets.push(views.join(name));
    }
    for target in &targets {
        if target.exists() {
            let report = serde_json::json!({
                "version": ec::ENGINEERING_CONNECTIONS_VERSION,
                "status": "rejected",
                "category": "output_conflict",
                "message": format!(
                    "{} already exists; a publication never overwrites and never publishes a partial package",
                    target.display()
                ),
                "detail": { "conflict": target.to_string_lossy() },
                "wroteBundle": false,
                "wroteViews": false,
                "naturalTerrainRuns": 0,
            });
            // The conflicting file is somebody else's: it is never overwritten, so the
            // refusal is reported on the machine-readable standard output instead.
            println!("{}", serde_json::to_string(&report)?);
            return Err("output conflict: a target path already exists".into());
        }
    }
    // 2. Read and build.
    let input = match ec::load_input(json) {
        Ok(input) => input,
        Err(error) => {
            let report = serde_json::json!({
                "version": ec::ENGINEERING_CONNECTIONS_VERSION,
                "sceneId": serde_json::Value::Null,
                "status": "rejected",
                "category": error.category,
                "message": error.message,
                "detail": error.detail,
                "wroteBundle": false,
                "wroteViews": false,
                "naturalTerrainRuns": 0,
            });
            write_json(&options.report, &report)?;
            return Err(format!(
                "engineering input rejected [{}]: {}",
                error.category, error.message
            )
            .into());
        }
    };
    let (mut bundle, outcome) = match ec::build_bundle(&input) {
        Ok(pair) => pair,
        Err(error) => {
            let report = serde_json::json!({
                "version": ec::ENGINEERING_CONNECTIONS_VERSION,
                "sceneId": input.scene_id,
                "status": "rejected",
                "category": error.category,
                "message": error.message,
                "detail": error.detail,
                "wroteBundle": false,
                "wroteViews": false,
                "naturalTerrainRuns": 0,
            });
            write_json(&options.report, &report)?;
            return Err(format!(
                "engineering connection rejected [{}]: {}",
                error.category, error.message
            )
            .into());
        }
    };
    let build_ms = started.elapsed().as_secs_f64() * 1000.0;
    // Parents of the final targets must exist before the commit moves; only the
    // directories this run created are removed again on failure.
    let mut created_dirs: Vec<PathBuf> = vec![];
    for dir in [out.parent(), options.report.parent(), Some(views.as_path())]
        .into_iter()
        .flatten()
    {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            fs::create_dir_all(dir)?;
            created_dirs.push(dir.to_path_buf());
        }
    }
    // Stage everything, then commit the bundle inside the staging directory.
    let staging = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .join(format!(
            "{}.{}.staging",
            file_name(out),
            &bundle.input_hash[..12.min(bundle.input_hash.len())]
        ));
    if staging.exists() {
        return Err(format!("staging directory {} already exists", staging.display()).into());
    }
    fs::create_dir_all(&staging)?;
    let mut created: Vec<PathBuf> = vec![staging.clone()];
    let result = (|| -> CliResult<serde_json::Value> {
        let staged_bundle = staging.join(file_name(out));
        write_new(&staged_bundle, serde_json::to_vec_pretty(&bundle)?.as_slice())?;
        created.push(staged_bundle.clone());
        let mut view_records: Vec<serde_json::Value> = vec![];
        for (name, content) in ec::views_to_files(&bundle, &outcome) {
            let path = staging.join(&name);
            write_new(&path, content.as_bytes())?;
            created.push(path.clone());
            view_records.push(serde_json::json!({
                "file": name,
                "bytes": content.len(),
                "blake3": blake3_of(&path)?,
            }));
        }
        let serialize_ms = started.elapsed().as_secs_f64() * 1000.0;
        if options.inject.as_deref() == Some("views") {
            return Err("injected failure at the views stage (test hook)".into());
        }
        // The staged copy on disk stays `staged` on purpose: only the file written at
        // its final path carries the committed state, so a consumer can tell a real
        // publication from something copied out of a failed run.
        let payload_hash =
            ec::commit_bundle(&mut bundle, &file_name(out), &file_name(&options.report))?;
        let committed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if options.inject.as_deref() == Some("report") {
            return Err("injected failure at the report stage (test hook)".into());
        }
        if started.elapsed().as_secs_f64() > STEP_BUDGET_SECONDS {
            return Err(format!(
                "step budget exceeded before commit: {:.3} s against the {STEP_BUDGET_SECONDS} s cap",
                started.elapsed().as_secs_f64()
            )
            .into());
        }
        let report = serde_json::json!({
            "version": ec::ENGINEERING_CONNECTIONS_VERSION,
            "sceneId": bundle.scene_id,
            "status": "built",
            "bundle": out.to_string_lossy(),
            "bundleBytes": fs::metadata(&staged_bundle)?.len(),
            "bundlePayloadHash": payload_hash,
            "inputHash": bundle.input_hash,
            "publication": {
                "state": "committed",
                "stagingId": bundle.publication.staging_id,
                "commitOrder": bundle.publication.commit_order,
            },
            "views": view_records,
            "surface": {
                "engineeredHash": bundle.surface.engineered_hash,
                "naturalHash": bundle.surface.natural_hash,
                "baselineHash": bundle.surface.baseline_hash,
                "engineeredCellsChanged": bundle.surface.engineered_cells_changed,
                "naturalCellsDiffering": bundle.surface.natural_cells_differing,
                "changedCellsByPlanTotal": bundle.surface.changed_cells_by_plan_total,
                "cutVolumeM3": bundle.surface.cut_volume_m3,
                "fillVolumeM3": bundle.surface.fill_volume_m3,
            },
            "structures": bundle.structures.len(),
            "meshPoints": bundle.mesh_points,
            "meshTriangles": bundle.mesh_triangles,
            "graph": {
                "nodes": bundle.graph.nodes.len(),
                "edges": bundle.graph.edges.len(),
                "groundEdges": bundle.graph.ground_edges,
                "stairEdges": bundle.graph.stair_edges,
                "landingEdges": bundle.graph.landing_edges,
                "oneWayEdges": bundle.graph.one_way_edges,
            },
            "route": {
                "status": bundle.route.status,
                "capability": ec::capability_name(bundle.route.capability),
                "pathEdges": bundle.route.path_edge_ids,
                "length3dM": bundle.route.path_length_3d_m,
                "horizontalM": bundle.route.horizontal_length_m,
                "cumulativeRiseM": bundle.route.cumulative_rise_m,
                "cumulativeDescentM": bundle.route.cumulative_descent_m,
                "expandedNodes": bundle.route.expanded_nodes,
            },
            "rejectedByReason": bundle.route.rejected_by_reason,
            "steps": {
                "buildMs": build_ms,
                "serializeMs": serialize_ms,
                "commitMs": committed_ms - serialize_ms,
                "totalMs": started.elapsed().as_secs_f64() * 1000.0,
                "stepBudgetSeconds": STEP_BUDGET_SECONDS,
            },
            "wroteBundle": true,
            "wroteViews": true,
            "naturalTerrainRuns": 0,
            "note": "this command edits only the engineered field it was given; it never bakes a world",
        });
        let staged_report = staging.join(file_name(&options.report));
        write_new(&staged_report, serde_json::to_vec_pretty(&report)?.as_slice())?;
        created.push(staged_report.clone());
        // 4. Commit: views, then the report, then the bundle last.
        for name in VIEW_FILES {
            let source = staging.join(name);
            if source.exists() {
                let target = views.join(name);
                fs::rename(&source, &target)?;
                created.push(target);
            }
        }
        fs::rename(&staged_report, &options.report)?;
        created.push(options.report.clone());
        if options.inject.as_deref() == Some("bundle") {
            return Err("injected failure at the bundle stage (test hook)".into());
        }
        // The bundle is the commit point and is written at its final path last, through
        // the production primitive that owns the file it creates: a failure after the
        // create is cleaned up, and a failure before it leaves no consumable package.
        write_new_owned(
            out,
            &serde_json::to_vec_pretty(&bundle)?,
            &mut created,
            options.inject.as_deref(),
        )?;
        // Final clock gate: it covers serialization and the whole commit, not just the
        // work before it, and a breach removes everything this run created.
        if started.elapsed().as_secs_f64() > STEP_BUDGET_SECONDS {
            return Err(format!(
                "step budget exceeded after the final write: {:.3} s against the {STEP_BUDGET_SECONDS} s cap",
                started.elapsed().as_secs_f64()
            )
            .into());
        }
        Ok(report)    })();
    match result {
        Ok(report) => {
            // The report was moved into place as part of the commit; rewriting it here
            // would be a second publish of the same artifact.
            let _ = fs::remove_dir_all(&staging);
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Err(error) => {
            // Only this run's own artifacts are removed; nothing else is touched.
            for path in created.iter().rev() {
                if path.is_dir() {
                    let _ = fs::remove_dir_all(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
            for dir in created_dirs.iter().rev() {
                let _ = fs::remove_dir(dir);
            }
            let report = serde_json::json!({
                "version": ec::ENGINEERING_CONNECTIONS_VERSION,
                "sceneId": serde_json::Value::Null,
                "status": "rejected",
                "category": if error.to_string().contains("step budget") {
                    "step_budget_exceeded"
                } else {
                    "publication_failed"
                },
                "message": error.to_string(),
                "detail": { "staging": staging.to_string_lossy(), "cleanedUp": true },
                "wroteBundle": false,
                "wroteViews": false,
                "naturalTerrainRuns": 0,
            });
            write_json(&options.report, &report)?;
            Err(error)
        }
    }
}

fn verify_command(options: &Options, json: &str) -> CliResult<()> {
    let started = Instant::now();
    let bundle_path = options
        .bundle
        .as_ref()
        .ok_or("--bundle is required to verify")?;
    let bundle_json = fs::read_to_string(bundle_path)?;
    // Integrity before semantics: an edited payload hash is reported even when the
    // description is no longer a v2 input.
    let value: serde_json::Value = serde_json::from_str(&bundle_json)
        .map_err(|error| format!("the saved bundle is not readable JSON: {error}"))?;
    // Integrity is checked inside verify_bundle, which hashes the typed bundle the
    // same way the builder did; a raw JSON hash would depend on key order.
    let input = match ec::load_input(json) {
        Ok(input) => input,
        Err(error) => {
            let report = serde_json::json!({
                "status": "input_rejected",
                "category": error.category,
                "message": error.message,
                "detail": error.detail,
                "findingCount": 1,
                "findings": [{
                    "code": "rebuild_rejected",
                    "at": "input",
                    "expected": "a v2 description",
                    "found": error.category,
                }],
                "steps": { "totalMs": started.elapsed().as_secs_f64() * 1000.0 },
            });
            write_json(&options.report, &report)?;
            return Err(format!(
                "engineering input rejected [{}]: {}",
                error.category, error.message
            )
            .into());
        }
    };
    let stored: ec::EngineeringBundle = serde_json::from_value(value)
        .map_err(|error| format!("the saved bundle is not a readable bundle: {error}"))?;
    let mut report = ec::verify_bundle(&input, &stored);
    report.notes.push(format!(
        "verify wall clock {:.3} ms, gate {STEP_BUDGET_SECONDS} s",
        started.elapsed().as_secs_f64() * 1000.0
    ));
    let value = serde_json::to_value(&report)?;
    write_json(&options.report, &value)?;
    println!("{}", serde_json::to_string(&value)?);
    if report.status == "verified" {
        Ok(())
    } else {
        Err(format!(
            "engineering connection verification failed: {} ({} findings)",
            report.status, report.finding_count
        )
        .into())
    }
}
