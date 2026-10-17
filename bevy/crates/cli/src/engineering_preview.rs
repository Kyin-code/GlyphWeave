//! Production CLI surface for the read-only engineering preview (B01-G23).
//!
//! ```text
//!   glyphweave export-engineering-preview --input INPUT.json --bundle BUNDLE.json --out NEW_DIR
//!   glyphweave audit-engineering-preview  --preview DIR --report NEW_REPORT.json
//! ```
//!
//! Both consume a human-authored engineering package only. Neither triggers `generate`,
//! `build-engineering-connection`, `bake` or `realize`, and neither writes into the source
//! it reads.
//!
//! Publication is a transaction of this exporter's own: the target directory must not
//! exist, everything is written into a staging directory inside it, the payload is moved
//! into place, and `engineering-preview.json` is written last as the completion marker. A
//! failure removes only the paths this run created.
use super::*;
use glyphweave_core::engineering_preview as preview;

const STAGING: &str = ".staging";

struct ExportOptions {
    input: PathBuf,
    bundle: PathBuf,
    out: PathBuf,
    inject: Option<String>,
}

fn parse_export(args: &[String]) -> CliResult<ExportOptions> {
    let mut input = None;
    let mut bundle = None;
    let mut out = None;
    let mut inject = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} needs a path", args[index]))?
            .clone();
        match args[index].as_str() {
            "--input" => input = Some(PathBuf::from(value)),
            "--bundle" => bundle = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            // Test hook: fail at one publication stage so the cleanup path is exercised by
            // a real run rather than asserted from a description.
            "--inject-failure" => inject = Some(value),
            other => return Err(format!("unknown option {other:?}").into()),
        }
        index += 2;
    }
    Ok(ExportOptions {
        input: input.ok_or("--input is required")?,
        bundle: bundle.ok_or("--bundle is required")?,
        out: out.ok_or("--out is required")?,
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

fn remove_created(created: &[PathBuf], new_dir: &Path) {
    for path in created.iter().rev() {
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
    // The payload lives in subdirectories this run created. Remove them deepest first, and
    // only while they are empty: `remove_dir` refuses a non-empty directory, so a file that
    // belongs to somebody else is never deleted. The output directory was preflighted as
    // absent, so anything still empty here is this run's own scaffolding.
    let mut dirs: Vec<PathBuf> = created
        .iter()
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .filter(|dir| dir.starts_with(new_dir))
        .collect();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    dirs.dedup();
    for dir in dirs {
        let _ = fs::remove_dir(&dir);
    }
    let _ = fs::remove_dir(new_dir);
}

pub(super) fn run(args: &[String], audit: bool) -> CliResult<()> {
    if audit {
        audit_command(args)
    } else {
        export_command(args)
    }
}

fn export_command(args: &[String]) -> CliResult<()> {
    let options = parse_export(args)?;
    let input_bytes = fs::read(&options.input).map_err(|error| {
        format!("cannot read the input description {}: {error}", options.input.display())
    })?;
    let bundle_bytes = fs::read(&options.bundle).map_err(|error| {
        format!("cannot read the bundle {}: {error}", options.bundle.display())
    })?;
    let input_json = String::from_utf8(input_bytes).map_err(|_| "the input is not UTF-8 text")?;
    let bundle_json = String::from_utf8(bundle_bytes).map_err(|_| "the bundle is not UTF-8 text")?;

    // The gate: the production verifier decides, and nothing is written before it does.
    let export = match preview::build_export(&input_json, &bundle_json) {
        Ok(export) => export,
        Err(error) => {
            let report = serde_json::json!({
                "status": "rejected",
                "command": "export-engineering-preview",
                "category": error.category,
                "message": error.message,
                "detail": error.detail,
                "wrotePreview": false,
                "input": options.input.to_string_lossy(),
                "bundle": options.bundle.to_string_lossy(),
            });
            println!("{}", serde_json::to_string(&report)?);
            return Err(format!(
                "engineering preview export rejected [{}]: {}",
                error.category, error.message
            )
            .into());
        }
    };

    if options.out.exists() {
        let report = serde_json::json!({
            "status": "rejected",
            "command": "export-engineering-preview",
            "category": "output_conflict",
            "message": format!(
                "{} already exists; this exporter only ever creates a new directory and never overwrites",
                options.out.display()
            ),
            "detail": { "conflict": options.out.to_string_lossy() },
            "wrotePreview": false,
        });
        println!("{}", serde_json::to_string(&report)?);
        return Err("engineering preview export conflict: the target directory already exists".into());
    }

    fs::create_dir_all(&options.out)?;
    let mut created: Vec<PathBuf> = vec![];
    let staging = options.out.join(STAGING);
    let result = (|| -> CliResult<serde_json::Value> {
        fs::create_dir_all(&staging)?;
        created.push(staging.clone());
        // 1. Everything, including the two source copies, is written into staging first.
        let mut payload: Vec<(String, Vec<u8>)> = vec![
            (preview::SOURCE_INPUT_FILE.to_string(), export.source_input.clone()),
            (preview::SOURCE_BUNDLE_FILE.to_string(), export.source_bundle.clone()),
            (preview::RENDER_MODEL_FILE.to_string(), export.render_model_json.clone()),
        ];
        payload.extend(export.assets.iter().cloned());
        for (relative, bytes) in &payload {
            write_new(&staging.join(relative), bytes)?;
        }
        if options.inject.as_deref() == Some("staging") {
            return Err("injected failure after staging, before the commit (test hook)".into());
        }
        // 2. Commit: move the payload into place. The completion marker is not among them.
        let mut committed: Vec<String> = vec![];
        for (relative, _bytes) in &payload {
            let target = options.out.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if options.inject.as_deref() == Some("commit") && committed.len() == 1 {
                return Err("injected failure mid-commit (test hook)".into());
            }
            fs::rename(staging.join(relative), &target)?;
            created.push(target);
            committed.push(relative.clone());
        }
        let _ = fs::remove_dir_all(&staging);
        created.retain(|path| path != &staging);
        if options.inject.as_deref() == Some("marker") {
            return Err("injected failure before the completion marker (test hook)".into());
        }
        // 3. The description is the completion marker and is written last.
        write_new(&options.out.join(preview::DESCRIPTION_FILE), &export.description_json)?;
        created.push(options.out.join(preview::DESCRIPTION_FILE));
        let report = serde_json::json!({
            "status": "exported",
            "command": "export-engineering-preview",
            "out": options.out.to_string_lossy(),
            "sceneId": export.description.scene_id,
            "schema": export.description.schema,
            "exportStatus": export.description.export_status,
            "viewerEntry": export.description.viewer_entry,
            "files": export.description.files.len() + 1,
            "verified": {
                "verifyStatus": export.description.verification.verify_status,
                "findingCount": export.description.verification.verify_finding_count,
                "payloadHashMatches": export.description.verification.payload_hash_matches,
                "inputHashMatches": export.description.verification.input_hash_matches,
            },
            "source": {
                "inputSha256": export.description.source.input_sha256,
                "bundleSha256": export.description.source.bundle_sha256,
                "bundlePayloadHash": export.description.source.bundle_payload_hash,
            },
            "counts": {
                "parts": export
                    .description
                    .render_model
                    .objects,
                "files": export.description.files.len(),
            },
        });
        Ok(report)
    })();

    match result {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Err(error) => {
            remove_created(&created, &options.out);
            let report = serde_json::json!({
                "status": "rejected",
                "command": "export-engineering-preview",
                "category": "publication_failed",
                "message": error.to_string(),
                "detail": { "cleanedUp": true, "out": options.out.to_string_lossy() },
                "wrotePreview": false,
            });
            println!("{}", serde_json::to_string(&report)?);
            Err(error)
        }
    }
}

fn audit_command(args: &[String]) -> CliResult<()> {
    let mut target = None;
    let mut report_path = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} needs a path", args[index]))?
            .clone();
        match args[index].as_str() {
            "--preview" => target = Some(PathBuf::from(value)),
            "--report" => report_path = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option {other:?}").into()),
        }
        index += 2;
    }
    let target = target.ok_or("--preview is required")?;
    let report_path = report_path.ok_or("--report is required")?;
    if report_path.exists() {
        return Err(format!(
            "{} already exists; an audit report is never overwritten",
            report_path.display()
        )
        .into());
    }
    if !target.is_dir() {
        return Err(format!("{} is not a directory", target.display()).into());
    }
    let root = fs::canonicalize(&target)?;

    let read_optional = |relative: &str| -> Option<String> {
        fs::read(root.join(relative))
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    };
    let description = read_optional(preview::DESCRIPTION_FILE);
    let input = read_optional(preview::SOURCE_INPUT_FILE);
    let bundle = read_optional(preview::SOURCE_BUNDLE_FILE);
    let render_model = read_optional(preview::RENDER_MODEL_FILE);

    let report = match preview::observe_directory(&root) {
        Ok(observed) => preview::audit_export(
            &root.to_string_lossy(),
            description.as_deref(),
            input.as_deref(),
            bundle.as_deref(),
            render_model.as_deref(),
            &observed,
        ),
        Err(error) => preview::refusal_report(&root.to_string_lossy(), &error.message),
    };

    write_new(
        &report_path,
        serde_json::to_vec_pretty(&report)?.as_slice(),
    )?;
    let summary = serde_json::json!({
        "status": report.status,
        "command": "audit-engineering-preview",
        "preview": root.to_string_lossy(),
        "report": report_path.to_string_lossy(),
        "engineeringVerification": report.engineering_verification.status,
        "sourceBinding": report.source_binding.status,
        "renderParity": report.render_parity.status,
        "previewCompleteness": report.preview_completeness.status,
        "findingCount": report.finding_count,
        "findings": report.findings.iter().take(5).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string(&summary)?);
    if report.status == "pass" {
        Ok(())
    } else {
        Err(format!(
            "engineering preview audit failed: {} finding(s); first {:?}",
            report.finding_count,
            report.findings.first()
        )
        .into())
    }
}
