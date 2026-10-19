mod context_preview3d;
mod context_preview;
mod context_preview_entry;
mod engineering_connections;
mod engineering_context;
mod engineering_preview;
mod road_corridor;
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::height_payload::HeightEncoding;
use glyphweave_core::natural::verify_snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use glyphweave_core::intent::{IntentDraft, WorldIntent, draft_from_text};
use glyphweave_core::migration::{MigrationMode, migrate_legacy_json};
use glyphweave_core::rules::{ObjectRegistry, load_descriptor, load_dir};
use glyphweave_core::storage::archive::{ArchiveLimits, read_entries};
use glyphweave_core::storage::bitpack::unpack_indices;
use glyphweave_core::storage::canonical::chunk_id;
use glyphweave_core::storage::codec::{
    decode_world, decode_world_with_metadata, encode_world_with_metadata,
};
use glyphweave_core::storage::model::{Manifest, RegionManifest};
use glyphweave_core::voxel::{
    CHUNK_VOLUME, ChunkCoord, LocalVoxelCoord, RegionChunkCoord, RegionCoord, VoxelCoord,
};
use glyphweave_core::worldgen::{
    EntityChangeApproval, EntityChangeKind, EntityInstance, EntityInventoryMode, LandUseProfile,
    SceneIndex, WorldIndex, WorldManifest, WorldPatch, analyze_landuse_areas, apply_patch,
    audit_baked_scene, audit_entity_inventory, bake_world, build_access_graph,
    footprint_intersects_water, validate_access_surfaces, water_geometry, water_kind,
    world_manifest_revision, write_demo_manifest,
};

type CliResult<T> = Result<T, Box<dyn Error>>;
const DEFAULT_DUMP_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DumpSelector {
    Coord(VoxelCoord),
    Section(DumpSectionSelector),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DumpSectionSelector {
    region: RegionCoord,
    section: RegionChunkCoord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DumpTarget {
    region: RegionCoord,
    section: RegionChunkCoord,
    selected_coord: Option<VoxelCoord>,
    selected_local: Option<LocalVoxelCoord>,
}

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("glyphweave: {error}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> CliResult<()> {
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Err("missing command".into());
    };
    match command {
        "init-world" => init_world_command(&args[1..]),
        "generate-demo-world" => generate_demo_world_command(&args[1..]),
        "generate-procedural-world" => generate_procedural_world_command(&args[1..]),
        "generate-world" => generate_world_command(&args[1..]),
        "apply-patch" => apply_patch_command(&args[1..]),
        "apply-patch-preserving" => apply_patch_preserving_command(&args[1..]),
        "quality-report" => quality_report_command(&args[1..]),
        "world-quality-gate" => world_quality_gate_command(&args[1..]),
        "validate-world" => validate_world_command(&args[1..]),
        "check-baseline" => check_baseline_command(&args[1..]),
        "scale-audit" => scale_audit_command(&args[1..]),
        "preview" => preview_command(&args[1..]),
        "design-road-corridor" => road_corridor::run(&args[1..], false),
        "verify-road-corridor" => road_corridor::run(&args[1..], true),
        "build-engineering-connection" => engineering_connections::run(&args[1..], false),
        "verify-engineering-connection" => engineering_connections::run(&args[1..], true),
        "export-engineering-preview" => engineering_preview::run(&args[1..], false),
        "audit-engineering-preview" => engineering_preview::run(&args[1..], true),
        "export-engineering-context" => engineering_context::run(&args[1..], false),
        "audit-engineering-context" => engineering_context::run(&args[1..], true),
        "export-context-preview" => context_preview::run(&args[1..], false),
        "audit-context-preview" => context_preview::run(&args[1..], true),
        "saved-context-preview" => context_preview_entry::run(&args[1..]),
        "saved-context-preview-handoff" => context_preview_entry::run_handoff(&args[1..]),
        "export-context-preview3d" => context_preview3d::run(&args[1..], false),
        "audit-context-preview3d" => context_preview3d::run(&args[1..], true),
        "export-preview-stream" => export_preview_stream_command(&args[1..]),
        "audit-preview-stream" => audit_preview_stream_command(&args[1..]),
        "convert" => convert_command(&args[1..]),
        "dump-chunk" => dump_chunk_command(&args[1..]),
        "inspect" => inspect_command(&args[1..]),
        "validate" => validate_command(&args[1..]),
        "rules" => rules_command(&args[1..]),
        "intent" => intent_command(&args[1..]),
        "compact" => compact_command(&args[1..]),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            print_usage();
            Err(format!("unknown command {other:?}").into())
        }
    }
}

fn read_confirmed_intent(path: &Path) -> CliResult<WorldIntent> {
    let bytes = fs::read(path)?;
    if let Ok(draft) = serde_json::from_slice::<IntentDraft>(&bytes) {
        if !draft.is_ready() {
            return Err(format!(
                "intent draft is incomplete: {}",
                draft.missing_fields.join(", ")
            )
            .into());
        }
        return Ok(draft.intent.expect("ready draft has intent"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Deterministic intent tools are the contract boundary for an Agent: free
/// text becomes a reviewable draft, then only a complete confirmed intent can
/// compile into the generator's WorldManifest.
fn intent_command(args: &[String]) -> CliResult<()> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("intent requires draft | validate | compile".into());
    };
    match subcommand {
        "draft" => {
            if args.len() != 3 {
                return Err("intent draft requires INPUT.txt OUTPUT.json".into());
            }
            let source = fs::read_to_string(&args[1])?;
            let draft = draft_from_text(&source);
            fs::write(&args[2], serde_json::to_vec_pretty(&draft)?)?;
            println!(
                "intent draft: ready={} recognized={} missing={}",
                draft.is_ready(),
                draft.recognized.len(),
                draft.missing_fields.join(", ")
            );
            if !draft.is_ready() {
                return Err(
                    "intent draft is incomplete; resolve missingFields before compilation".into(),
                );
            }
            Ok(())
        }
        "validate" => {
            if args.len() != 2 {
                return Err("intent validate requires INTENT.json".into());
            }
            let intent = read_confirmed_intent(Path::new(&args[1]))?;
            intent.validate()?;
            let manifest = intent.compile_manifest();
            manifest.validate()?;
            println!(
                "valid intent: {} {}x{}m",
                intent.name, intent.scene.width_m, intent.scene.depth_m
            );
            Ok(())
        }
        "compile" => {
            if args.len() != 3 {
                return Err("intent compile requires INTENT.json MANIFEST.json".into());
            }
            let intent = read_confirmed_intent(Path::new(&args[1]))?;
            intent.validate()?;
            let manifest = intent.compile_manifest();
            manifest.validate()?;
            fs::write(&args[2], serde_json::to_vec_pretty(&manifest)?)?;
            println!("compiled intent into {}", args[2]);
            Ok(())
        }
        _ => Err("intent requires draft | validate | compile".into()),
    }
}

// Derived-only regeneration for existing audited worlds. Never changes the
// authoritative scene or reports; a projection is not an independent audit.
fn export_preview_stream_command(args: &[String]) -> CliResult<()> {
    let root = fs::canonicalize(one_path("export-preview-stream", args)?)?;
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    for scene_path in &world.scenes {
        let path = root.join(scene_path).canonicalize()?;
        if !path.starts_with(&root) {
            return Err("scene escapes world directory".into());
        }
        let scene: SceneIndex =
            serde_json::from_reader(std::io::BufReader::new(fs::File::open(&path)?))?;
        glyphweave_core::preview_stream::write(
            &scene,
            &world.revision,
            path.parent().ok_or("missing scene directory")?,
        )?;
    }
    glyphweave_core::worldgen::refresh_html_adapter(&root)?;
    println!(
        "exported render projections for {} scenes; authoritative files unchanged",
        world.scenes.len()
    );
    Ok(())
}

fn audit_preview_stream_command(args: &[String]) -> CliResult<()> {
    if args.len() != 2 {
        return Err("audit-preview-stream requires WORLD_DIR OUT_DIR".into());
    }
    let root = fs::canonicalize(&args[0])?;
    let parity = glyphweave_core::preview_stream_audit::audit_world(&root, Path::new(&args[1]))?;
    let status = parity
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("fail");
    let scene_count = parity
        .get("scenes")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    println!(
        "audit-preview-stream status={status} scenes={scene_count} out={}",
        args[1]
    );
    if status != "pass" {
        return Err("preview parity audit failed".into());
    }
    Ok(())
}

fn preview_command(args: &[String]) -> CliResult<()> {
    if args.is_empty() || args.len() > 2 {
        return Err("preview requires WORLD_DIR [PORT]".into());
    }
    let root = fs::canonicalize(&args[0])?;
    let port: u16 = match args.get(1) {
        Some(value) => value.parse().map_err(|_| "PORT must be a valid u16")?,
        None => 8080,
    };
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("preview: http://127.0.0.1:{port}/preview/");
    for stream in listener.incoming() {
        let mut stream = stream?;
        // Client EOF, cancellation and malformed feedback are connection-local.
        // A browser preconnect must never terminate the preview listener.
        if let Err(error) = serve_preview_connection(&root, &mut stream) {
            eprintln!("preview connection closed: {error}");
        }
    }
    Ok(())
}

fn serve_preview_connection(root: &Path, stream: &mut TcpStream) -> CliResult<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
    let request = read_http_request(stream)?;
    let line = String::from_utf8_lossy(&request);
    let request_line = line.lines().next().unwrap_or("");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("GET");
    let requested = request_parts.next().unwrap_or("/");
    let requested = requested.split('?').next().unwrap_or(requested);
    if method == "POST" && requested == "/api/feedback" {
        let body = line.split_once("\r\n\r\n").map_or("", |(_, body)| body);
        let feedback: serde_json::Value = serde_json::from_str(body)?;
        fs::write(
            root.join("visual-feedback.json"),
            serde_json::to_vec_pretty(&feedback)?,
        )?;
        let response =
            serde_json::to_vec(&serde_json::json!({"ok": true, "path": "visual-feedback.json"}))?;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.len()
        )?;
        stream.write_all(&response)?;
        return Ok(());
    }
    let relative = requested.trim_start_matches('/').replace('/', "\\");
    let relative = if relative.is_empty() {
        "preview\\index.html".to_owned()
    } else {
        relative
    };
    let path = root.join(&relative);
    let relative = if path.is_dir() {
        format!("{relative}index.html")
    } else {
        relative
    };
    let path = root.join(&relative);
    let canonical = path.canonicalize().ok();
    let allowed = canonical
        .as_ref()
        .is_some_and(|path| path.starts_with(&root));
    let (status, content_type, body) = if allowed {
        match fs::read(canonical.expect("checked canonical path")) {
            Ok(body) => ("200 OK", mime_for(&relative), body),
            Err(_) => ("404 Not Found", "text/plain", b"Not found".to_vec()),
        }
    } else {
        ("403 Forbidden", "text/plain", b"Forbidden".to_vec())
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> CliResult<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end;
    loop {
        let length = stream.read(&mut buffer)?;
        if length == 0 {
            return Err("HTTP request ended before headers".into());
        }
        request.extend_from_slice(&buffer[..length]);
        if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
        if request.len() > 64 * 1024 {
            return Err("HTTP headers exceed 64 KiB".into());
        }
    }
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    let total_length = header_end.saturating_add(content_length);
    if total_length > 4 * 1024 * 1024 {
        return Err("HTTP request exceeds 4 MiB".into());
    }
    while request.len() < total_length {
        let length = stream.read(&mut buffer)?;
        if length == 0 {
            return Err("HTTP request ended before body".into());
        }
        request.extend_from_slice(&buffer[..length]);
        if request.len() > 4 * 1024 * 1024 {
            return Err("HTTP request exceeds 4 MiB".into());
        }
    }
    Ok(request)
}

fn mime_for(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "bin" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

fn init_world_command(args: &[String]) -> CliResult<()> {
    let path = one_path("init-world", args)?;
    write_demo_manifest(path)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn generate_demo_world_command(args: &[String]) -> CliResult<()> {
    let output = one_path("generate-demo-world", args)?;
    let index = bake_world(&WorldManifest::default_demo(), output)?;
    write_generation_report(output, &index)?;
    Ok(())
}

fn generate_procedural_world_command(args: &[String]) -> CliResult<()> {
    if !(3..=7).contains(&args.len()) {
        return Err(
            "generate-procedural-world requires OUTPUT_DIR WIDTH_M DEPTH_M [SEED] [THEME] [URBAN_RATIO] [PROFILE]"
                .into(),
        );
    }
    let output = Path::new(&args[0]);
    let width_m: u32 = args[1].parse().map_err(|_| "WIDTH_M must be a u32")?;
    let depth_m: u32 = args[2].parse().map_err(|_| "DEPTH_M must be a u32")?;
    let seed: u64 = args.get(3).map_or(Ok(42), |value| {
        value.parse().map_err(|_| "SEED must be a u64")
    })?;
    let theme = args.get(4).cloned();
    let urban_ratio: Option<f64> = args.get(5).map_or(Ok(None), |value| {
        value
            .parse()
            .map(Some)
            .map_err(|_| "URBAN_RATIO must be a f64")
    })?;
    let mut manifest = WorldManifest::default_demo();
    manifest.world.seed = seed;
    manifest.scenes[0].width_m = width_m;
    manifest.scenes[0].depth_m = depth_m;
    if let Some(theme) = theme {
        if let Some(profile) = manifest.style.get_mut("landUseProfile") {
            profile["theme"] = serde_json::json!(theme);
        }
    }
    if let Some(ratio) = urban_ratio {
        if let Some(profile) = manifest.style.get_mut("landUseProfile") {
            profile["urbanCoreRatio"] = serde_json::json!(ratio);
            profile["suburbanRatio"] = serde_json::json!((ratio * 1.2).min(1.0 - ratio));
        }
    }
    if let Some(profile) = args.get(6) {
        if !matches!(
            profile.as_str(),
            "legacy" | "prototype" | "strict" | "release"
        ) {
            return Err("PROFILE must be legacy, prototype, strict, or release".into());
        }
        manifest.style["generationProfile"] = serde_json::json!(profile);
        if matches!(profile.as_str(), "strict" | "release") {
            manifest.style["rulesMode"] = serde_json::json!("rules");
        }
    }
    let index = bake_world(&manifest, output)?;
    write_generation_report(output, &index)
}

fn generate_world_command(args: &[String]) -> CliResult<()> {
    if args.len() != 2 {
        return Err("generate-world requires MANIFEST.json OUTPUT_DIR".into());
    }
    let mut manifest: WorldManifest = serde_json::from_slice(&fs::read(&args[0])?)?;
    let manifest_dir = Path::new(&args[0])
        .parent()
        .unwrap_or_else(|| Path::new("."));
    if let Some(rules_dir) = manifest
        .style
        .get("rulesDir")
        .and_then(serde_json::Value::as_str)
    {
        let path = PathBuf::from(rules_dir);
        if path.is_relative() {
            manifest.style["rulesDir"] = serde_json::json!(manifest_dir.join(path));
        }
    }
    let output = Path::new(&args[1]);
    let index = bake_world(&manifest, output)?;
    write_generation_report(output, &index)
}

fn write_generation_report(
    output: &Path,
    index: &glyphweave_core::worldgen::WorldIndex,
) -> CliResult<()> {
    let mut chunk_count = 0_usize;
    let mut entity_count = 0_usize;
    let mut landmark_count = 0_usize;
    for scene_path in &index.scenes {
        let scene: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_slice(&fs::read(output.join(scene_path))?)?;
        chunk_count += scene.chunks.len();
        entity_count += scene.entities.len();
        landmark_count += scene.landmarks.len();
    }
    let manifest: Option<WorldManifest> = fs::read(output.join("glyphweave.manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let profile = manifest
        .as_ref()
        .and_then(|m| m.style.get("generationProfile"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("legacy");
    let placement = fs::read(output.join("rules-placement.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let effective = fs::read(output.join("effective-programs.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let report = serde_json::json!({
        "format": "glyphweave.generation-report",
        "version": 1,
        "status": "baked",
        "nextGate": if matches!(profile, "strict" | "release") { "world-quality-gate" } else { "scale-audit" },
        "worldRevision": index.revision,
        "sceneCount": index.scenes.len(),
        "chunkCount": chunk_count,
        "entityCount": entity_count,
        "landmarkCount": landmark_count,
        "renderMode": index.render_mode,
        "generationProfile": profile,
        "generatorRevision": glyphweave_core::worldgen::WORLD_GENERATOR_REVISION,
        "patchChainHash": manifest.as_ref().map(|m| m.patch_chain_hash.clone()).unwrap_or_default(),
        "placement": placement,
        // Present only when a morphology profile compiled effective programs.
        "effectivePrograms": effective,
        "output": output.display().to_string(),
    });
    fs::write(
        output.join("generation-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn apply_patch_command(args: &[String]) -> CliResult<()> {
    if args.len() != 3 {
        return Err("apply-patch requires MANIFEST.json PATCH.json OUTPUT_DIR".into());
    }
    let manifest: WorldManifest = serde_json::from_slice(&fs::read(&args[0])?)?;
    let patch: WorldPatch = serde_json::from_slice(&fs::read(&args[1])?)?;
    let patched = apply_patch(&manifest, &patch)?;
    let index = bake_world(&patched, Path::new(&args[2]))?;
    println!("patched world revision: {}", index.revision);
    Ok(())
}

fn apply_patch_preserving_command(args: &[String]) -> CliResult<()> {
    if args.len() != 5 {
        return Err(
            "apply-patch-preserving requires BASE_WORLD MANIFEST.json PATCH.json APPROVALS.json OUTPUT_DIR"
                .into(),
        );
    }
    let base_root = fs::canonicalize(&args[0])?;
    let output = Path::new(&args[4]);
    if output.exists() {
        return Err(format!("output directory already exists: {}", output.display()).into());
    }
    let base_world: WorldIndex =
        serde_json::from_slice(&fs::read(base_root.join("world.json"))?)?;
    if base_world.generator_revision.is_empty()
        || base_world.generator_revision != glyphweave_core::worldgen::WORLD_GENERATOR_REVISION
    {
        return Err(format!(
            "migration-required: base world generator revision {:?} differs from current {}; preserving mode does not rename old identities implicitly",
            base_world.generator_revision,
            glyphweave_core::worldgen::WORLD_GENERATOR_REVISION
        )
        .into());
    }

    let manifest: WorldManifest = serde_json::from_slice(&fs::read(&args[1])?)?;
    let patch: WorldPatch = serde_json::from_slice(&fs::read(&args[2])?)?;
    let approvals = read_inventory_approvals(Path::new(&args[3]))?;
    let patched = apply_patch(&manifest, &patch)?;
    let staging = inventory_staging_path(output)?;

    let result = (|| -> CliResult<()> {
        let index = bake_world(&patched, &staging)?;
        write_generation_report(&staging, &index)?;
        let old_entities = read_world_entities(&base_root)?;
        let new_entities = read_world_entities(&staging)?;
        let audit = audit_entity_inventory(
            &old_entities,
            &new_entities,
            &approvals,
            EntityInventoryMode::Preserving {
                old_scheme: glyphweave_core::worldgen::ENTITY_ID_SCHEME.into(),
                new_scheme: glyphweave_core::worldgen::ENTITY_ID_SCHEME.into(),
            },
        );
        let mut report = serde_json::to_value(&audit)?;
        report["baseWorld"] = serde_json::json!(base_root.display().to_string());
        report["outputWorld"] = serde_json::json!(output.display().to_string());
        report["generatorRevision"] =
            serde_json::json!(glyphweave_core::worldgen::WORLD_GENERATOR_REVISION);
        fs::write(
            staging.join("entity-inventory-audit.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        if audit.status != "pass" {
            return Err(format!(
                "inventory audit failed: {}",
                if audit.failures.is_empty() {
                    "unknown inventory failure".into()
                } else {
                    audit.failures.join("; ")
                }
            )
            .into());
        }
        fs::rename(&staging, output)?;
        println!(
            "preserving patch published: {} entities, revision {}",
            new_entities.len(),
            index.revision
        );
        Ok(())
    })();

    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn inventory_staging_path(output: &Path) -> CliResult<PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .ok_or("output directory must have a name")?
        .to_string_lossy();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let staging = parent.join(format!(
        ".{name}.inventory-{}-{nonce}",
        std::process::id()
    ));
    if staging.exists() {
        return Err(format!(
            "inventory staging path already exists: {}",
            staging.display()
        )
        .into());
    }
    Ok(staging)
}

fn read_world_entities(root: &Path) -> CliResult<Vec<EntityInstance>> {
    let root = fs::canonicalize(root)?;
    let world: WorldIndex = serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let mut entities = Vec::new();
    for scene_path in &world.scenes {
        let path = root.join(scene_path).canonicalize()?;
        if !path.starts_with(&root) {
            return Err("scene escapes world directory".into());
        }
        let scene: SceneIndex =
            serde_json::from_reader(std::io::BufReader::new(fs::File::open(path)?))?;
        entities.extend(scene.entities);
    }
    Ok(entities)
}

fn read_inventory_approvals(path: &Path) -> CliResult<Vec<EntityChangeApproval>> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if value.get("format").and_then(serde_json::Value::as_str)
        != Some("glyphweave.entity-change-approvals")
        || value.get("version").and_then(serde_json::Value::as_u64) != Some(1)
    {
        return Err("unsupported entity change approvals format/version".into());
    }
    let items = value
        .get("approvals")
        .and_then(serde_json::Value::as_array)
        .ok_or("approvals file must contain an approvals array")?;
    let mut approvals = Vec::with_capacity(items.len());
    for item in items {
        let entity_id = item
            .get("entityId")
            .and_then(serde_json::Value::as_str)
            .ok_or("approval entityId must be a string")?
            .to_owned();
        let change: EntityChangeKind = serde_json::from_value(
            item.get("change")
                .cloned()
                .ok_or("approval change is required")?,
        )
        .map_err(|_| "approval change must be moved, updated, or removed")?;
        approvals.push(EntityChangeApproval { entity_id, change });
    }
    Ok(approvals)
}

/// Independent effective-program audit. The manifest's original style and
/// morphology profile are recompiled and compared with the published
/// `effective-programs.json`; the returned value is the recompiled effective
/// style used by the downstream program audits.
fn audit_effective_programs(
    root: &Path,
    manifest: &WorldManifest,
) -> CliResult<serde_json::Value> {
    let published: Option<serde_json::Value> = fs::read(root.join("effective-programs.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let compiled = glyphweave_core::settlement::morphology_compile::audit_published(
        published.as_ref().unwrap_or(&serde_json::Value::Null),
        &manifest.style,
        &manifest.scenes,
    )?;
    Ok(compiled
        .map(|compiled| compiled.style)
        .unwrap_or_else(|| manifest.style.clone()))
}

fn validate_world_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("validate-world", args)?);
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let manifest: WorldManifest =
        serde_json::from_slice(&fs::read(root.join("glyphweave.manifest.json"))?)?;
    manifest.validate()?;
    // Recompile the effective programs from the original manifest+profile and
    // bind them to the published document. The generator's own report is never
    // trusted as input.
    let effective_style = audit_effective_programs(root, &manifest)?;
    // The road geometry revision is part of the world identity: it is rebuilt
    // from the original manifest/effective programs, never read from the
    // product's own claim. A product that can use the 16m graded band must
    // publish it, and a product that cannot must not.
    let expected_geometry =
        glyphweave_core::worldgen::road_geometry_revision_for_style(&effective_style);
    if world.road_geometry_revision.as_deref() != expected_geometry {
        return Err(format!(
            "road geometry revision mismatch: published {:?}, expected {:?} from the recompiled manifest/effective programs",
            world.road_geometry_revision, expected_geometry
        )
        .into());
    }
    let expected_revision = world_manifest_revision(&manifest)?;
    if world.revision != expected_revision {
        return Err(format!(
            "world.json revision {} does not match manifest/generator revision {}",
            world.revision, expected_revision
        )
        .into());
    }
    if !world.generator_revision.is_empty()
        && world.generator_revision != glyphweave_core::worldgen::WORLD_GENERATOR_REVISION
    {
        return Err(format!(
            "unsupported world generator revision {}",
            world.generator_revision
        )
        .into());
    }
    let sidecar: serde_json::Value = serde_json::from_slice(&fs::read(root.join("sidecar.json"))?)?;
    let contract = sidecar
        .get("contract")
        .ok_or("sidecar.json missing contract")?;
    if contract.get("format").and_then(serde_json::Value::as_str) != Some("glyphweave-sidecar")
        || contract.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || contract
            .get("authoritativeForTerrain")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        || contract
            .get("gemapRole")
            .and_then(serde_json::Value::as_str)
            != Some("identity-anchor")
    {
        return Err("unsupported or incomplete sidecar contract".into());
    }
    let expected = glyphweave_core::worldgen::SidecarContract::for_style(&manifest.style)?;
    let published: glyphweave_core::worldgen::SidecarContract =
        serde_json::from_value(contract.clone())?;
    if serde_json::to_value(&expected)? != serde_json::to_value(&published)?
        || serde_json::to_value(&world.sidecar)? != serde_json::to_value(&expected)?
    {
        return Err("sidecar height contract differs from manifest".into());
    }
    let terrain_semantics = contract.get("terrainSemantics");
    if let Some(semantics) = terrain_semantics {
        let expected_channels = [
            "slope_degrees_u8",
            "curvature_signed_u8",
            "wetness_u8",
            "disturbance_u8",
        ];
        if semantics.get("format").and_then(serde_json::Value::as_str)
            != Some("glyphweave-terrain-semantics")
            || semantics.get("version").and_then(serde_json::Value::as_u64) != Some(1)
            || semantics
                .get("bytesPerCell")
                .and_then(serde_json::Value::as_u64)
                != Some(4)
            || semantics
                .get("channels")
                .and_then(serde_json::Value::as_array)
                .map(|channels| {
                    channels
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                })
                != Some(expected_channels.to_vec())
        {
            return Err("unsupported terrain semantics contract".into());
        }
    }
    if sidecar
        .get("worldIndex")
        .and_then(serde_json::Value::as_str)
        != Some("world.json")
    {
        return Err("sidecar.json must reference world.json".into());
    }
    let expected_scenes: Vec<String> = serde_json::from_value(
        sidecar
            .get("scenes")
            .cloned()
            .ok_or("sidecar.json missing scenes")?,
    )?;
    if expected_scenes != world.scenes {
        return Err("sidecar scene list does not match world.json".into());
    }
    let gemap = decode_world_with_metadata(
        &fs::read(root.join("world.gemap"))?,
        ArchiveLimits::default(),
    )?;
    let meta = gemap
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("world"))
        .ok_or("world.gemap missing world metadata")?;
    if meta.get("revision").and_then(serde_json::Value::as_str) != Some(world.revision.as_str())
        || meta.get("sidecar").and_then(serde_json::Value::as_str) != Some("sidecar.json")
        || meta.get("gemapRole").and_then(serde_json::Value::as_str) != Some("identity-anchor")
    {
        return Err("world.gemap metadata does not match sidecar contract".into());
    }
    for scene_path in &world.scenes {
        let scene: SceneIndex = serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
        for chunk in &scene.chunks {
            for file in [&chunk.height_file, &chunk.surface_file, &chunk.lod2_file] {
                if !root
                    .join("scenes")
                    .join(&scene.scene_id)
                    .join(file)
                    .is_file()
                {
                    return Err(
                        format!("missing sidecar payload {}/{}", scene.scene_id, file).into(),
                    );
                }
            }
            if terrain_semantics.is_some() {
                let terrain_file = chunk.terrain_file.as_ref().ok_or_else(|| {
                    format!(
                        "scene {} is missing terrain semantics payload metadata",
                        scene.scene_id
                    )
                })?;
                let terrain_path = root.join("scenes").join(&scene.scene_id).join(terrain_file);
                let expected = chunk.valid_width_m as u64 * chunk.valid_depth_m as u64 * 4;
                if !terrain_path.is_file() {
                    return Err(format!(
                        "missing terrain semantics payload {}/{}",
                        scene.scene_id, terrain_file
                    )
                    .into());
                }
                if fs::metadata(&terrain_path)?.len() != expected {
                    return Err(format!(
                        "terrain semantics payload {}/{} has invalid length",
                        scene.scene_id, terrain_file
                    )
                    .into());
                }
            }
        }
    }
    println!(
        "valid GlyphWeave world: {} scenes; gemap=identity-anchor; sidecar=terrain-authoritative",
        world.scenes.len()
    );
    Ok(())
}

fn check_baseline_command(args: &[String]) -> CliResult<()> {
    if args.len() != 2 {
        return Err("check-baseline requires BASELINE.json WORLD_DIR".into());
    }
    let baseline: serde_json::Value = serde_json::from_slice(&fs::read(&args[0])?)?;
    let root = Path::new(&args[1]);
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let baseline_scenes = baseline
        .get("scenes")
        .and_then(serde_json::Value::as_array)
        .ok_or("baseline missing scenes")?;
    if baseline_scenes.len() != world.scenes.len() {
        return Err("baseline scene count does not match world".into());
    }
    for baseline_scene in baseline_scenes {
        let id = baseline_scene
            .get("sceneId")
            .and_then(serde_json::Value::as_str)
            .ok_or("baseline scene missing sceneId")?;
        let scene_path = world
            .scenes
            .iter()
            .find(|path| path.split('/').nth(1) == Some(id))
            .ok_or_else(|| format!("world missing baseline scene {id}"))?;
        let scene: SceneIndex = serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
        let expected_entities = baseline_scene
            .get("entityCount")
            .and_then(serde_json::Value::as_u64)
            .ok_or("baseline scene missing entityCount")? as usize;
        if scene.entities.len() != expected_entities {
            return Err(format!(
                "{id}: entity count {} != baseline {expected_entities}",
                scene.entities.len()
            )
            .into());
        }
        let expected_landmarks = baseline_scene
            .get("landmarkCount")
            .and_then(serde_json::Value::as_u64)
            .ok_or("baseline scene missing landmarkCount")?
            as usize;
        if scene.landmarks.len() != expected_landmarks {
            return Err(format!(
                "{id}: landmark count {} != baseline {expected_landmarks}",
                scene.landmarks.len()
            )
            .into());
        }
        let mut kinds = BTreeMap::<String, usize>::new();
        for entity in &scene.entities {
            *kinds.entry(entity.kind.clone()).or_default() += 1;
        }
        let expected_kinds: BTreeMap<String, usize> = serde_json::from_value(
            baseline_scene
                .get("entityKinds")
                .cloned()
                .ok_or("baseline scene missing entityKinds")?,
        )?;
        if kinds != expected_kinds {
            return Err(format!("{id}: entity kind distribution differs from baseline").into());
        }
        let expected_hashes: Vec<String> = serde_json::from_value(
            baseline_scene
                .get("chunkHashes")
                .cloned()
                .ok_or("baseline scene missing chunkHashes")?,
        )?;
        let hashes: Vec<String> = scene
            .chunks
            .iter()
            .map(|chunk| chunk.hash.clone())
            .collect();
        if hashes != expected_hashes {
            return Err(format!("{id}: chunk hashes differ from baseline").into());
        }
    }
    println!("baseline matches {}", args[0]);
    Ok(())
}

fn world_quality_gate_command(args: &[String]) -> CliResult<()> {
    if args.len() != 1 {
        return Err("world-quality-gate requires WORLD_DIR".into());
    }
    // Run the existing structural gates first so this command is the single
    // release-facing entry point rather than a second, drifting validator.
    validate_world_command(args)?;
    scale_audit_command(args)?;
    quality_report_command(args)?;

    let root = Path::new(&args[0]);
    let manifest: WorldManifest =
        serde_json::from_slice(&fs::read(root.join("glyphweave.manifest.json"))?)?;
    manifest.validate()?;
    let profile = manifest
        .style
        .get("generationProfile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("legacy");
    let rules_mode = manifest
        .style
        .get("rulesMode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("legacy");
    let strict = matches!(profile, "strict" | "release");
    let mut failures = Vec::<String>::new();
    let mut warnings = Vec::<String>::new();
    if strict && rules_mode != "rules" {
        failures.push("generationProfile=strict requires style.rulesMode=rules".to_owned());
    }

    let quality: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("quality-report.json"))?)?;
    let mut natural_metrics = serde_json::Map::new();
    for scene in quality["scenes"]
        .as_array()
        .ok_or("quality report missing scenes")?
    {
        let audit = scene
            .get("naturalTerrainAudit")
            .ok_or("quality report missing independently recomputed natural audit")?;
        if strict && audit["status"] != "pass" {
            failures.push(format!(
                "{} recomputed natural audit failed",
                scene["sceneId"]
            ));
        }
        natural_metrics.insert(
            scene["sceneId"].as_str().ok_or("missing scene id")?.into(),
            audit.clone(),
        );
    }
    if quality.get("status").and_then(serde_json::Value::as_str) == Some("fail") {
        failures.push("quality-report status is fail".to_owned());
    }
    if let Some(items) = quality
        .get("warnings")
        .and_then(serde_json::Value::as_array)
    {
        warnings.extend(
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned),
        );
    }

    if strict {
        for scene in quality["scenes"]
            .as_array()
            .ok_or("quality report missing scenes")?
        {
            if scene["frontageAudit"].is_object() && scene["frontageAudit"]["status"] != "pass" {
                failures.push(format!("{} local-height frontage demand or realization remains incomplete",scene["sceneId"]));
            }
            if scene["purposeConversionAudit"].is_object()
                && scene["purposeConversionAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} rural-to-urban purpose conversion is not provable",
                    scene["sceneId"]
                ));
            }
            if scene["automaticRoads"]["unservedTargets"].as_u64().unwrap_or(0)>0 {
                failures.push(format!("{} automatic road targets remain out of reach after {}",scene["sceneId"],scene["automaticRoads"]["stopReason"]));
            }
            if scene["roadNetworkAudit"].is_object()
                && scene["roadNetworkAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} final road network failed independent design audit",
                    scene["sceneId"]
                ));
            }
            if scene["landProtectionAudit"].is_object()
                && scene["landProtectionAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} reserved land protection audit failed",
                    scene["sceneId"]
                ));
            }
            if scene["shoreBandAudit"].is_object()
                && scene["shoreBandAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} shoreline public passage band failed its independent audit",
                    scene["sceneId"]
                ));
            }
            if scene["entryCorridorAudit"].is_object()
                && scene["entryCorridorAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} graded entry corridor failed its independent audit",
                    scene["sceneId"]
                ));
            }
            if scene["jointLayoutAudit"].is_object()
                && scene["jointLayoutAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} joint frontage housing minimum remains unmet",
                    scene["sceneId"]
                ));
            }
            if scene["serviceProgramAudit"].is_object()
                && scene["serviceProgramAudit"]["status"] != "pass"
            {
                failures.push(format!(
                    "{} explicit service requests remain unrealized",
                    scene["sceneId"]
                ));
            }
            if scene["developmentAudit"].is_object() && scene["landUseLedger"]["status"] != "pass" {
                failures.push(format!(
                    "{} purpose-area allocation or realization remains incomplete",
                    scene["sceneId"]
                ));
            }
            if scene["landcoverAudit"].is_object() && scene["landcoverAudit"]["status"] != "pass" {
                failures.push(format!("{} landcover remains unrealized", scene["sceneId"]));
                if scene["landcoverAudit"]["ecologyUnfilledPlants"]
                    .as_u64()
                    .unwrap_or(0)
                    > 0
                {
                    failures.push(format!(
                        "{} ecology density budget has {} unfilled plants",
                        scene["sceneId"], scene["landcoverAudit"]["ecologyUnfilledPlants"]
                    ));
                }
            }
            if quality["landUseProfileApplied"] == true {
                if scene["landUseBudget"]["status"] != "planned_within_tolerance"
                    || scene["landUseBudget"]["entityCoverageStatus"] != "realized"
                {
                    failures.push(format!(
                        "{} land-use planning or realization remains incomplete",
                        scene["sceneId"]
                    ));
                }
                if scene["developmentAudit"].is_object()
                    && scene["developmentAudit"]["status"] != "pass"
                {
                    failures.push(format!(
                        "{} road-first land-use development remains incomplete",
                        scene["sceneId"]
                    ));
                }
                if !scene["developmentAudit"].is_object()
                    && scene["parcelRealization"]["unrealized"]
                        .as_u64()
                        .unwrap_or(1)
                        > 0
                {
                    failures.push(format!(
                        "{} has unrealized planned parcels",
                        scene["sceneId"]
                    ));
                }
                if quality["landUseProfile"]["urbanTarget"]
                    .as_f64()
                    .unwrap_or(0.)
                    > 0.
                    && scene["entityKinds"]["road"].as_u64().unwrap_or(0) == 0
                {
                    failures.push(format!(
                        "{} urban scene has no actual roads",
                        scene["sceneId"]
                    ));
                }
            }
            let access = scene
                .get("accessGraph")
                .ok_or("quality report missing access graph")?;
            let buildings = scene
                .get("entityKinds")
                .and_then(|kinds| kinds.as_object())
                .map(|kinds| {
                    kinds
                        .iter()
                        .filter(|(kind, _)| {
                            matches!(
                                kind.as_str(),
                                "building"
                                    | "building_tower"
                                    | "building_cluster"
                                    | "urban_building"
                                    | "residential_block"
                                    | "residential_tower"
                                    | "residential_home"
                                    | "resort_lodge"
                                    | "storefront"
                                    | "commercial_center"
                                    | "entertainment_center"
                                    | "school"
                                    | "town_hall"
                                    | "market"
                                    | "industrial"
                                    | "temple"
                                    | "church"
                                    | "shopping_mall"
                                    | "office_tower"
                                    | "convenience_store"
                                    | "night_market"
                            )
                        })
                        .map(|(_, count)| count.as_u64().unwrap_or(0))
                        .sum::<u64>()
                })
                .unwrap_or(0);
            if buildings == 0 {
                failures.push(format!(
                    "{} has no buildings; empty access graph cannot pass",
                    scene["sceneId"]
                ));
            }
            if access["version"].as_u64() != Some(2)
                || access["disconnectedEntities"].as_u64().unwrap_or(u64::MAX) > 0
            {
                failures.push(format!(
                    "{} has unverified building access routes",
                    scene["sceneId"]
                ));
            }
        }
    }
    let rules = fs::read(root.join("rules-audit.json"))
        .ok()
        .map(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes))
        .transpose()?;
    if strict && rules.is_none() {
        failures.push("strict profile requires rules-audit.json".to_owned());
    }
    let mut rule_metrics = serde_json::Map::new();
    if let Some(report) = &rules {
        for key in [
            "checked_items",
            "passed_items",
            "rejected_items",
            "floating_items",
            "submerged_items",
            "slope_too_high",
            "out_of_bounds",
            "blocked_entrances",
            "geometry_collisions",
            "disconnected_roads",
        ] {
            if let Some(value) = report.get(key) {
                rule_metrics.insert(key.to_owned(), value.clone());
            }
        }
        if report
            .get("rejected_items")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            > 0
        {
            failures.push("rules-audit contains rejected items".to_owned());
        }
        if let Some(unruled) = report
            .get("unruled_items")
            .and_then(serde_json::Value::as_array)
        {
            let mut kinds = unruled
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>();
            kinds.sort_unstable();
            kinds.dedup();
            rule_metrics.insert("unruled_kinds".to_owned(), serde_json::json!(kinds));
            let special_unruled = kinds
                .iter()
                .all(|kind| matches!(*kind, "river" | "lake" | "canal"));
            if strict && !kinds.is_empty() {
                failures.push(format!(
                    "strict profile has unruled entity kinds: {}",
                    kinds.join(", ")
                ));
            }
            if special_unruled {
                rule_metrics.insert("special_unruled_kinds".to_owned(), serde_json::json!(kinds));
            }
        }
    }

    let registry_path = root.join("assets").join("glyphweave.registry.json");
    let mut registry_metrics = serde_json::Map::new();
    match fs::read(&registry_path) {
        Ok(bytes) => {
            let registry: serde_json::Value = serde_json::from_slice(&bytes)?;
            let entries = registry
                .get("assets")
                .and_then(serde_json::Value::as_object)
                .ok_or("asset registry missing assets object")?;
            let mut missing_files = Vec::new();
            for (asset_id, entry) in entries {
                let Some(path) = entry.get("path").and_then(serde_json::Value::as_str) else {
                    missing_files.push(format!("{asset_id}: missing path"));
                    continue;
                };
                if !root.join(path).is_file() {
                    missing_files.push(format!("{asset_id}: missing published asset {path}"));
                }
                for field in ["format", "unit", "origin", "forward", "collision"] {
                    if entry
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .is_none()
                    {
                        missing_files.push(format!("{asset_id}: missing {field}"));
                    }
                }
            }
            registry_metrics.insert("entries".to_owned(), serde_json::json!(entries.len()));
            registry_metrics.insert("missing".to_owned(), serde_json::json!(missing_files));
            if strict
                && registry_metrics["missing"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
            {
                failures.push("published asset registry is incomplete".to_owned());
            }
        }
        Err(_) if strict => {
            failures.push("strict profile requires assets/glyphweave.registry.json".to_owned())
        }
        Err(_) => warnings.push("published asset registry is missing".to_owned()),
    }

    let report = serde_json::json!({
        "format": "glyphweave.world-quality-gate",
        "version": 1,
        "status": if !failures.is_empty() { "fail" } else if !warnings.is_empty() { "warn" } else { "pass" },
        "profile": profile,
        "rulesMode": rules_mode,
        "worldRevision": serde_json::from_slice::<serde_json::Value>(&fs::read(root.join("world.json"))?)?.get("revision").cloned(),
        "failures": failures,
        "warnings": warnings,
        "metrics": { "rules": rule_metrics, "assets": registry_metrics, "naturalTerrain": natural_metrics },
        "inputs": ["validate-world", "scale-audit", "quality-report", "rules-audit.json"]
    });
    fs::write(
        root.join("world-quality-gate.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report["status"] == "fail" {
        return Err("world quality gate failed".into());
    }
    Ok(())
}

// JSON float parsing may differ by one ULP. Tolerance applies only to
// numeric budget values; keys, statuses, nulls and structure stay exact.
fn budget_values_match(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) else {
                return false;
            };
            (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.)
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|w| budget_values_match(v, w)))
        }
        _ => a == b,
    }
}

fn quality_report_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("quality-report", args)?);
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let mut scene_reports = Vec::new();
    let mut access_surface_failed = false;
    let mut conversion_failed = false;
    // Round 19 published reconciliation removals (`skips[]`); this report does
    // not trust them, it recomputes them from the plan ids the bake published.
    let mut engineering_placement_failed = false;
    let mut engineering_placement_skips = 0_usize;
    let mut warnings = Vec::new();
    let mut chunks = 0_usize;
    let mut entities = 0_usize;
    let mut landmarks = 0_usize;
    let visual_feedback = fs::read(root.join("visual-feedback.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let scale_audit = fs::read(root.join("scale-audit.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let scale_failed = scale_audit
        .as_ref()
        .and_then(|report| report.get("status"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status != "pass");
    if scale_audit.is_none() {
        warnings.push(
            "scale-audit.json missing: run `glyphweave scale-audit WORLD_DIR` before this report"
                .to_owned(),
        );
    }
    if scale_failed {
        warnings.push("scale-audit failed".to_owned());
    }
    if let Some(feedback) = &visual_feedback {
        if feedback
            .get("verdict")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value != "pass")
        {
            warnings.push("visual feedback requires review".to_owned());
        }
    }
    let manifest: WorldManifest =
        serde_json::from_slice(&fs::read(root.join("glyphweave.manifest.json"))?)?;
    manifest.validate()?;
    // The program audits below must consume the same compiled effective input
    // that generation used; the published document is re-derived, not trusted.
    let effective_style = audit_effective_programs(&root, &manifest)?;
    let profile = LandUseProfile::from_style(&manifest.style);
    // The current profile-driven land-use generator runs on land scenes only.
    // Do not falsely grade a water template or naturalOnly scene against a
    // profile that the generator intentionally did not execute; surface the
    // applicability gap explicitly instead.
    let natural_only = manifest
        .style
        .get("naturalOnly")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let has_structured_water = manifest
        .style
        .get("water")
        .and_then(|water| water.get("waterType"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| matches!(kind, "lake" | "river"));
    let intent_settlement = manifest
        .style
        .get("generationIntent")
        .and_then(|intent| intent.get("settlement"))
        .and_then(serde_json::Value::as_str);
    let intent_planner_applied = matches!(intent_settlement, Some("city" | "town" | "pastoral"));
    let profile_applied =
        profile.is_some() && !natural_only && (!has_structured_water || intent_planner_applied);
    if profile.is_some() && !profile_applied && !natural_only {
        warnings.push(
            "landUseProfile targets are not evaluated for structured-water scene without an explicit settlement planner: the current generator does not apply the profile-driven land-use pass".to_owned(),
        );
    }
    let mut area_reports = Vec::new();
    // A world baked with `rulesMode = rules` publishes one reconciliation report
    // per scene. Removal is the only way reconciliation may shrink a scene, so a
    // report that cannot be recomputed (a skip for an unplanned item, a skip for
    // an authored landmark or infrastructure, a descriptor that never asked to be
    // skipped, or a deleted record that breaks plan conservation) fails the
    // quality report instead of passing silently.
    let engineering_placement = fs::read(root.join("engineering-placement.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let placement_registry = manifest
        .style
        .get("rulesDir")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .and_then(|dir| {
            ObjectRegistry::load_dir(&dir)
                .or_else(|_| ObjectRegistry::load_dir(&root.join(dir)))
                .ok()
        });
    for scene_path in &world.scenes {
        let scene: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
        let mut engineering_placement_audit = serde_json::Value::Null;
        if let Some(report) = &engineering_placement {
            let entry = report
                .get("scenes")
                .and_then(serde_json::Value::as_array)
                .and_then(|scenes| {
                    scenes.iter().find(|entry| {
                        entry.get("sceneId").and_then(serde_json::Value::as_str)
                            == Some(scene.scene_id.as_str())
                    })
                });
            if let Some(entry) = entry {
                let published: BTreeSet<String> =
                    scene.entities.iter().map(|e| e.entity_id.clone()).collect();
                let authored: BTreeSet<String> =
                    scene.landmarks.iter().map(|l| l.entity_id.clone()).collect();
                match glyphweave_core::worldgen::verify_published_placement(
                    entry,
                    &published,
                    &authored,
                    placement_registry.as_ref(),
                ) {
                    Ok(audit) => {
                        engineering_placement_skips += audit.skipped_items;
                        engineering_placement_audit = serde_json::json!({
                            "status": "pass",
                            "planItems": audit.plan_items,
                            "outputItems": audit.output_items,
                            "skippedItems": audit.skipped_items,
                            "skippedByRule": audit.skipped_by_rule,
                            "evidence": "recomputed-from-published-plan-ids-and-descriptor-fallback",
                        });
                    }
                    Err(message) => {
                        engineering_placement_failed = true;
                        warnings.push(format!(
                            "{}: engineering placement report is not recomputable: {message}",
                            scene.scene_id
                        ));
                        engineering_placement_audit = serde_json::json!({
                            "status": "fail",
                            "detail": message,
                        });
                    }
                }
            }
        }
        let mut kinds = BTreeMap::<String, usize>::new();
        for entity in &scene.entities {
            *kinds.entry(entity.kind.clone()).or_default() += 1;
        }
        let expected = (scene.chunk_count_x * scene.chunk_count_z) as usize;
        if scene.chunks.len() != expected {
            warnings.push(format!("{}: chunk index incomplete", scene.scene_id));
        }
        if scene.entities.len() < 100 && scene.width_m.saturating_mul(scene.depth_m) >= 1_000_000 {
            warnings.push(format!(
                "{}: low entity density ({})",
                scene.scene_id,
                scene.entities.len()
            ));
        }
        chunks += scene.chunks.len();
        entities += scene.entities.len();
        landmarks += scene.landmarks.len();
        let areas = analyze_landuse_areas(&scene);
        if let Some(budget) = &scene.land_use_budget {
            if budget.scene_area_m2 <= 0.0 || budget.assigned_m2 > budget.scene_area_m2 + 0.01 {
                warnings.push(format!(
                    "{}: invalid persisted land-use budget",
                    scene.scene_id
                ));
            }
            if budget.status != "planned_within_tolerance" {
                warnings.push(format!(
                    "{}: land-use planning status {} (planned unassigned {:.1} m2)",
                    scene.scene_id, budget.status, budget.unassigned_m2
                ));
            }
            if scene.development_plan.is_none() && budget.entity_coverage_status != "realized" {
                warnings.push(format!(
                    "{}: land-use entities are not yet fully realized (entity coverage {:.1} m2)",
                    scene.scene_id,
                    budget.entity_urban_m2 + budget.entity_rural_m2 + budget.entity_nature_m2
                ));
            }
            let entity_coverage =
                budget.entity_urban_m2 + budget.entity_rural_m2 + budget.entity_nature_m2;
            if scene.development_plan.is_none() && entity_coverage + 0.01 < budget.assigned_m2 * 0.5
            {
                warnings.push(format!(
                    "{}: planned land-use is not yet realized by generated entities (entity coverage {:.1} / planned {:.1} m2)",
                    scene.scene_id, entity_coverage, budget.assigned_m2
                ));
            }
        } else if profile_applied {
            warnings.push(format!(
                "{}: persisted land-use budget is missing",
                scene.scene_id
            ));
        }
        if scene.settlement_graph.is_none() {
            warnings.push(format!(
                "{}: persisted settlement graph is missing",
                scene.scene_id
            ));
        }
        let spec = manifest
            .scenes
            .iter()
            .find(|spec| spec.scene_id == scene.scene_id)
            .ok_or("quality report scene missing from manifest")?;
        let expected_water = water_geometry(
            water_kind(&manifest.style),
            &scene.landmarks,
            spec,
            &manifest.style,
        );
        if glyphweave_core::worldgen::road_network::enabled(&manifest.style, expected_water)
            && scene.road_network_plan.is_none()
        {
            return Err("missing required natural road network plan".into());
        }
        if let Some(profile) = &profile {
            // The zoning is re-derived independently BELOW, once this scene's natural raster is
            // loaded: the axis-and-node policy reads the road network the world builds, which is a
            // function of the natural terrain. What this block checks is that the PUBLISHED zoning,
            // the published plan and the published budget all describe one another.
            let zones = &scene.land_use_zones;
            let folder = root.join("scenes").join(&scene.scene_id);
            if scene.road_network_plan.is_some() {
                if scene.parcel_plan.is_some() {
                    return Err("road-first scene cannot publish legacy parcel authority".into());
                }
                let plan = scene
                    .development_plan
                    .as_ref()
                    .ok_or("missing required road-first development plan")?;
                let published: serde_json::Value =
                    serde_json::from_slice(&fs::read(folder.join("development-plan.json"))?)?;
                let canonical: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(plan)?)?;
                if published != canonical {
                    return Err("development-plan.json differs from scene plan".into());
                }
            } else {
                if scene.development_plan.is_some()
                    || scene.landcover_plan.is_some()
                    || scene.landcover_surfaces.is_some()
                {
                    return Err("development/landcover plan has no road source".into());
                }
                let plan = scene
                    .parcel_plan
                    .as_ref()
                    .ok_or("missing parcel plan; rebake required")?;
                glyphweave_core::settlement::verify_parcel_plan(plan, zones, &scene.entities)?;
                for (file, expected) in [
                    ("parcel-plan.json", serde_json::to_value(plan)?),
                    (
                        "infrastructure-plan.json",
                        serde_json::to_value(&plan.infrastructure)?,
                    ),
                ] {
                    let published: serde_json::Value =
                        serde_json::from_slice(&fs::read(folder.join(file))?)?;
                    if published != expected {
                        return Err(format!(
                            "{file} differs from independently checked scene plan"
                        )
                        .into());
                    }
                }
            }
            let budget = glyphweave_core::worldgen::plan_landuse_budget(&scene, profile);
            if !budget_values_match(
                &serde_json::to_value(&scene.land_use_budget)?,
                &serde_json::to_value(&budget)?,
            ) {
                return Err(
                    "persisted land-use budget differs from rebuilt entity and zone areas".into(),
                );
            }
            for (file, expected) in [("land-use-zones.json", serde_json::to_value(zones)?)] {
                let published: serde_json::Value =
                    serde_json::from_slice(&fs::read(folder.join(file))?)?;
                if published != expected {
                    return Err(
                        format!("{file} differs from independently checked scene plan").into(),
                    );
                }
            }
        }

        let mut terrain = BakedTerrain::new(spec)?;
        for chunk in &scene.chunks {
            let bytes = fs::read(
                root.join("scenes")
                    .join(&scene.scene_id)
                    .join(&chunk.height_file),
            )?;
            terrain.insert_chunk(
                chunk.world_x,
                chunk.world_z,
                chunk.valid_width_m,
                chunk.valid_depth_m,
                &bytes,
            )?;
        }
        terrain.require_complete()?;
        if terrain.encoding()
            != Some(glyphweave_core::worldgen::height_encoding_from_style(
                &manifest.style,
            )?)
        {
            return Err("final height encoding differs from manifest".into());
        }
        let mut natural_terrain = BakedTerrain::new(spec)?;
        for chunk in &scene.chunks {
            let natural_file = chunk.natural_height_file.as_ref().ok_or_else(|| {
                format!("{} missing natural heightfield reference", scene.scene_id)
            })?;
            let bytes = fs::read(root.join("scenes").join(&scene.scene_id).join(natural_file))?;
            natural_terrain.insert_chunk(
                chunk.world_x,
                chunk.world_z,
                chunk.valid_width_m,
                chunk.valid_depth_m,
                &bytes,
            )?;
        }
        natural_terrain.require_complete()?;
        if natural_terrain.encoding() != terrain.encoding() {
            return Err("natural/final height precision differs".into());
        }
        if scene.natural_terrain.seed != manifest.world.seed ^ spec.seed_offset {
            return Err(format!("{} natural snapshot seed mismatch", scene.scene_id).into());
        }
        let recomputed_natural_audit =
            verify_snapshot(&scene.natural_terrain, &natural_terrain, spec)?;
        let published_natural: glyphweave_core::natural::NaturalTerrainSnapshot =
            serde_json::from_slice(&fs::read(
                root.join("scenes")
                    .join(&scene.scene_id)
                    .join("natural-terrain.json"),
            )?)?;
        if serde_json::to_value(&published_natural)?
            != serde_json::to_value(&scene.natural_terrain)?
        {
            return Err(format!("{} natural snapshot copies differ", scene.scene_id).into());
        }
        let water = water_geometry(
            water_kind(&manifest.style),
            &scene.landmarks,
            spec,
            &manifest.style,
        );
        if glyphweave_core::worldgen::road_network::enabled(&manifest.style, water)
            && scene.road_network_plan.is_none()
        {
            return Err("missing required natural road network plan".into());
        }
        if let Some(profile) = &profile {
            // The one independent re-derivation of the scene's land-use plan. It is a pure function
            // of the manifest style, the scene envelope, the natural raster and the water geometry -
            // every input the published world carries - so a bake cannot smuggle in a plan its own
            // inputs do not state, and an opted-in axis-and-node plan is reproduced from scratch
            // (including the road network it grows along) rather than trusted.
            let program = glyphweave_core::worldgen::services::from_style(&effective_style)?;
            let source = glyphweave_core::worldgen::declared_natural_source(&manifest, spec)?;
            let expected = glyphweave_core::worldgen::plan_landuse_zones_for_style(
                spec,
                profile,
                &effective_style,
                &natural_terrain,
                water,
                program.as_ref(),
                source,
            )?;
            if serde_json::to_value(&expected.zones)?
                != serde_json::to_value(&scene.land_use_zones)?
            {
                return Err("persisted zones differ from manifest planning inputs".into());
            }
        }
        let road_network_audit = if let Some(plan) = &scene.road_network_plan {
            let published: glyphweave_core::worldgen::road_network::RoadNetworkPlan =
                serde_json::from_slice(&fs::read(
                    root.join("scenes")
                        .join(&scene.scene_id)
                        .join("road-network-plan.json"),
                )?)?;
            if serde_json::to_value(&published)? != serde_json::to_value(plan)? {
                return Err("road network plan copies differ".into());
            }
            let service_program =
                glyphweave_core::worldgen::services::from_style(&effective_style)?;
            // The independent audit derives its natural source from the trusted
            // original manifest (the same rule generation uses) instead of the
            // undeclared legacy wrapper, so a version-3 supply program can be
            // re-planned and compared here.
            let audit = glyphweave_core::worldgen::road_network::verify_with_manifest(
                plan,
                &manifest,
                spec,
                &natural_terrain,
                &terrain,
                water,
                &scene.entities,
                Some(&scene.land_use_zones),
                service_program.as_ref(),
                &effective_style,
            )?;
            if audit.status != "pass" {
                warnings.push(format!(
                    "{} road network differs from design: {:?}",
                    scene.scene_id, audit.failures
                ));
            }
            Some(audit)
        } else {
            None
        };
        // OPT-IN `style.terrainPolicy.entryCorridor`: one independent recomputation
        // of every published graded entry corridor, from the plan's own pad records,
        // the natural raster and the published raster. Absent for a world without a
        // corridor, so its report shape is unchanged.
        let entry_corridor_audit = scene.road_network_plan.as_ref().and_then(|plan| {
            if !plan.entry_corridor_policy.enabled()
                && !plan
                    .roadside_sites
                    .iter()
                    .any(|site| site.entry_corridor.is_some())
            {
                return None;
            }
            let audit = glyphweave_core::worldgen::entry_corridor::verify(
                &plan.roadside_sites,
                &natural_terrain,
                &terrain,
                spec,
                Some(&scene.land_use_zones),
                water,
                plan.max_roadbed_fill_m,
            );
            if audit.status != "pass" {
                warnings.push(format!(
                    "{} graded entry corridor differs from its plan: {:?}",
                    scene.scene_id, audit.failures
                ));
            }
            Some(audit)
        });
        let protection_audit = if scene.road_network_plan.is_some() {
            let a = glyphweave_core::worldgen::zoning::audit_protection(
                spec,
                &scene.land_use_zones,
                &natural_terrain,
                &terrain,
                &scene.entities,
            )?;
            if a.status != "pass" {
                warnings.push(format!(
                    "{} reserved land changed or occupied: {:?}",
                    scene.scene_id, a
                ));
            }
            Some(a)
        } else {
            None
        };
        let access = build_access_graph(&scene.entities, &terrain, water);
        let joint_layout_audit = scene
            .road_network_plan
            .as_ref()
            .filter(|p| p.joint_layout.is_some())
            .map(|p| glyphweave_core::worldgen::joint_frontage::audit(p, &scene.entities, &access));
        if let Some(a) = &joint_layout_audit {
            if a.status != "pass" {
                warnings.push(format!(
                    "{} joint housing minimum remains unmet: {:?}",
                    scene.scene_id, a
                ));
            }
        }
        let service_program_audit = scene
            .road_network_plan
            .as_ref()
            .filter(|p| !p.service_outcomes.is_empty())
            .map(|p| glyphweave_core::worldgen::services::audit(p, &scene.entities, &access));
        if let Some(audit) = &service_program_audit {
            if audit.status != "pass" {
                warnings.push(format!(
                    "{} requested services incomplete: {:?}",
                    scene.scene_id, audit
                ));
            }
        }
        let mut development_audit = if let Some(plan) = &scene.development_plan {
            let roads = scene
                .road_network_plan
                .as_ref()
                .ok_or("development missing road source")?;
            let audit = glyphweave_core::settlement::development::verify(
                plan,
                spec,
                &scene.land_use_zones,
                roads,
                &scene.entities,
                &access,
                &scene.rule_building_candidates,
            )?;
            let graph =
                glyphweave_core::settlement::development::settlement_graph(plan, &scene.entities);
            if serde_json::to_value(&scene.settlement_graph)? != serde_json::to_value(Some(&graph))?
            {
                return Err("settlement graph differs from road-first parentage".into());
            }
            let published: glyphweave_core::settlement::SettlementGraph =
                serde_json::from_slice(&fs::read(
                    root.join("scenes")
                        .join(&scene.scene_id)
                        .join("settlement-graph.json"),
                )?)?;
            if serde_json::to_value(&published)? != serde_json::to_value(&graph)? {
                return Err("settlement-graph.json differs from road-first parentage".into());
            }
            Some(audit)
        } else {
            None
        };
        let landcover_audit = if let Some(development) = &scene.development_plan {
            let plan = scene
                .landcover_plan
                .as_ref()
                .ok_or("missing required landcover plan")?;
            let published: serde_json::Value = serde_json::from_slice(&fs::read(
                root.join("scenes")
                    .join(&scene.scene_id)
                    .join("landcover-plan.json"),
            )?)?;
            if published != serde_json::to_value(plan)? {
                return Err("landcover-plan.json differs from scene plan".into());
            }
            glyphweave_core::settlement::farm_program::audit_source(
                &plan.farm_pattern,
                &effective_style,
                spec,
            )?;
            glyphweave_core::settlement::residual_land::validate_manifest(
                &effective_style,
                &manifest.scenes,
            )?;
            let expected_residual =
                glyphweave_core::settlement::residual_land::audit_source(
                    plan.residual_program.as_ref(),
                    &effective_style,
                    spec,
                )?;
            glyphweave_core::settlement::development::audit_landcover(
                plan,
                spec,
                development,
                expected_residual.as_ref(),
            )?;
            let surfaces = scene
                .landcover_surfaces
                .as_ref()
                .ok_or("missing required landcover surfaces")?;
            let published: glyphweave_core::settlement::landcover::SurfaceRealization =
                serde_json::from_slice(&fs::read(
                    root.join("scenes")
                        .join(&scene.scene_id)
                        .join("landcover-surfaces.json"),
                )?)?;
            let audit = glyphweave_core::settlement::landcover::verify(
                surfaces,
                spec,
                development,
                plan,
                &terrain,
                water,
                &scene.entities,
            )
            .map_err(|e| format!("scene landcover independent realization: {e}"))?;
            glyphweave_core::settlement::landcover::verify(
                &published,
                spec,
                development,
                plan,
                &terrain,
                water,
                &scene.entities,
            )
            .map_err(|e| format!("published landcover independent realization: {e}"))?;
            if audit.ecology_unfilled_plants > 0 {
                warnings.push(format!(
                    "{} ecology has {} / {} planned plants without legal placement",
                    scene.scene_id, audit.ecology_unfilled_plants, audit.ecology_planned_plants
                ));
            }
            if audit.unresolved_area_m2 > 1e-7 {
                warnings.push(format!(
                    "{} landcover has {} m2 without surface realization",
                    scene.scene_id, audit.unresolved_area_m2
                ));
            }
            Some(audit)
        } else {
            None
        };
        if let (Some(d), Some(s)) = (&mut development_audit, &landcover_audit) {
            glyphweave_core::settlement::landcover::apply_evidence(d, s);
            if d.status != "pass" {
                warnings.push(format!(
                    "{} road-first land-use development is {}: {:?} {:?}",
                    scene.scene_id, d.status, d.failures, d.incomplete_reasons
                ));
            }
        }
        let surface_audit = validate_access_surfaces(&access, &terrain, &scene.access_surfaces);
        if let Err(error) = &surface_audit {
            access_surface_failed = true;
            warnings.push(format!("{}: {error}", scene.scene_id));
        }
        if access.disconnected_entities > 0 {
            warnings.push(format!(
                "{}: {} buildings have no verified walkable entrance route",
                scene.scene_id, access.disconnected_entities
            ));
        }
        // Only verified source geometry enters this ledger. The published
        // land-use-ledger.json is a convenience report, never input authority.
        let land_use_ledger = if scene.development_plan.is_some() {
            let ledger = glyphweave_core::settlement::landuse_ledger::analyze(
                &scene,
                profile
                    .as_ref()
                    .ok_or("road-first ledger missing profile")?,
            )?;
            for (purpose, row) in &ledger.by_purpose {
                if row.unresolved_m2 > 1e-6 {
                    warnings.push(format!("{}: {purpose} land realization {:.1}/{:.1} m2; unresolved {:.1} m2 (whole-scene realized ratio {:.4}, target {:.4})",
                        scene.scene_id, row.realized_m2, row.planned_m2, row.unresolved_m2,
                        row.realized_scene_ratio, row.target_ratio));
                }
                if (row.planned_scene_ratio - row.target_ratio).abs() > 0.05 + 1e-6 {
                    warnings.push(format!("{}: {purpose} zoning ratio {:.4} differs from target {:.4} by more than 0.05",
                        scene.scene_id, row.planned_scene_ratio, row.target_ratio));
                }
            }
            Some(ledger)
        } else {
            None
        };
        // The shoreline public passage band: independently recomputed from the
        // plan, the zoning, the natural and final rasters, the published surfaces
        // and the ledger. Only an opted-in waterfront world carries one, so no
        // other report grows a key or a warning.
        let shore_band_audit = if water.shore_band() {
            let audit = glyphweave_core::worldgen::shore_band::audit(
                spec,
                water,
                &natural_terrain,
                &terrain,
                scene.road_network_plan.as_ref(),
                &scene.land_use_zones,
                &scene.entities,
                scene.landcover_surfaces.as_ref(),
                land_use_ledger.as_ref(),
            )
            .map_err(|e| format!("scene shore band independent audit: {e}"))?;
            if audit.status != "pass" {
                warnings.push(format!(
                    "{} shoreline public passage band failed its independent audit: {:?}",
                    scene.scene_id, audit.failures
                ));
            }
            Some(audit)
        } else {
            None
        };
        // An explicit along-road conversion moves purpose realization, so it must be provable from the
        // plan's own geometry and the water authority, and the PUBLISHED ledger's rows must be exactly
        // the plan's records. A hand-edited ledger inflating urban realization therefore fails here by
        // name instead of becoming the realization a reader sees.
        let conversion_audit = match &scene.road_network_plan {
            Some(plan)
                if land_use_ledger.is_some()
                    && (plan
                        .frontage
                        .as_ref()
                        .is_some_and(|report| !report.conversions.is_empty())
                        || !plan.pad_conversions.is_empty()
                        // A pad that DECLARED a purpose must be audited even when it converted
                        // nothing: the audit is what proves no earned conversion was left out.
                        || plan
                            .roadside_sites
                            .iter()
                            .any(|site| site.purpose.is_some())) =>
            {
                // The published ledger is the document a reader sees, so that is what must agree with
                // the plan; a missing one carries no claim at all and therefore fails.
                let published = fs::read(
                    root.join("scenes")
                        .join(&scene.scene_id)
                        .join("land-use-ledger.json"),
                )
                .ok()
                .and_then(|bytes| {
                    serde_json::from_slice::<
                        glyphweave_core::settlement::landuse_ledger::LandUseLedger,
                    >(&bytes)
                    .ok()
                });
                let audit = glyphweave_core::settlement::landuse_ledger::audit_conversions(
                    plan,
                    &scene.land_use_zones,
                    expected_water,
                    published.as_ref(),
                );
                if audit.status != "pass" {
                    conversion_failed = true;
                    for failure in &audit.failures {
                        warnings.push(format!("{}: {failure}", scene.scene_id));
                    }
                }
                Some(audit)
            }
            _ => None,
        };
        if profile_applied
            && land_use_ledger.is_none()
            && !has_structured_water
            && !matches!(intent_settlement, Some("pastoral"))
        {
            let profile = profile.as_ref().expect("profile_applied requires profile");
            // The profile ratios describe the intended land-use mix. Compare
            // the *normalized* shares (urban/rural/nature summing to 1) so a
            // sparse scene with large unassigned terrain still reports the
            // mix correctly instead of failing on absolute coverage.
            let used = areas.urban_ratio + areas.rural_ratio + areas.nature_ratio;
            if used > 0.0 {
                let share = |value: f64| value / used;
                let target_sum =
                    profile.urban_target() + profile.rural_target() + profile.nature_target();
                if target_sum > 0.0 {
                    let tolerance = 0.25;
                    let target = [
                        (
                            "urban",
                            share(areas.urban_ratio),
                            profile.urban_target() / target_sum,
                        ),
                        (
                            "rural",
                            share(areas.rural_ratio),
                            profile.rural_target() / target_sum,
                        ),
                        (
                            "nature",
                            share(areas.nature_ratio),
                            profile.nature_target() / target_sum,
                        ),
                    ];
                    for (label, actual, expected_target) in target {
                        if (actual - expected_target).abs() > tolerance {
                            warnings.push(format!(
                                "{}: {label} area share {actual:.3} is outside profile target {expected_target:.3} ±{tolerance}",
                                scene.scene_id
                            ));
                        }
                    }
                }
            }
        }
        let purpose_area = |kind: &str, legacy: f64| {
            land_use_ledger
                .as_ref()
                .map_or(legacy, |l| l.by_purpose[kind].realized_m2)
        };
        let urban_area = purpose_area("urban", areas.urban_m2);
        let rural_area = purpose_area("rural", areas.rural_m2);
        let nature_area = purpose_area("nature", areas.nature_m2);
        area_reports.push(serde_json::json!({
            "sceneId": scene.scene_id,
            "areaM2": areas.scene_area_m2,
            "accounting": if land_use_ledger.is_some() { "disjoint-purpose-cover-v1" } else { "legacy-entity-proxy" },
            "urbanM2": urban_area,
            "ruralM2": rural_area,
            "natureM2": nature_area,
            "urbanRatio": urban_area / areas.scene_area_m2,
            "ruralRatio": rural_area / areas.scene_area_m2,
            "natureRatio": nature_area / areas.scene_area_m2,
            "entityFootprintDiagnostic": {
                "accounting": "legacy-overlapping-entity-proxy-not-realization",
                "urbanM2": areas.urban_m2, "ruralM2": areas.rural_m2,
                "natureM2": areas.nature_m2, "byKindM2": areas.by_kind,
            },
            "landUseBudget": scene.land_use_budget,
            "landUseBudgetStatus": scene.land_use_budget.as_ref().map(|budget| budget.status.clone()),
            "settlementGraph": scene.settlement_graph.as_ref().map(|graph| serde_json::json!({
                "settlements": graph.settlements.len(),
                "districts": graph.districts.len(),
                "parcels": graph.parcels.len(),
                "publicSpaces": graph.public_spaces.len(),
                "buildingRelations": graph.building_relations.len(),
                "status": graph.status,
            })),
        }));
        let mut scene_report = serde_json::json!({
            "sceneId": scene.scene_id, "sizeM": [scene.width_m, scene.depth_m],
            "chunks": scene.chunks.len(), "expectedChunks": expected,
            "landmarks": scene.landmarks.len(), "entities": scene.entities.len(),
            "naturalTerrainAudit": recomputed_natural_audit,
            "roadNetworkAudit": road_network_audit,
            "automaticRoads":scene.road_network_plan.as_ref().and_then(|p|p.automatic_extensions.as_ref()),
            "frontageAudit":scene.road_network_plan.as_ref().filter(|p|p.frontage.is_some()).map(|p|glyphweave_core::worldgen::frontage::audit(p,&scene.entities,&access)),
            "jointLayoutAudit": joint_layout_audit,
            "landProtectionAudit": protection_audit,
            "landUseLedger": land_use_ledger,
            "serviceProgramAudit": service_program_audit,
            "developmentAudit": development_audit,
            "landcoverAudit": landcover_audit,
            "accessGraph": access,
            "accessSurfaces": { "count": scene.access_surfaces.len(), "status": if surface_audit.is_ok() { "pass" } else { "fail" } },
            "landUseBudget": scene.land_use_budget,
            "parcelRealization": scene.parcel_plan.as_ref().map(|p| serde_json::json!({
                "status": p.status, "planned": p.parcels.len(),
                "accepted": p.parcels.iter().filter(|p|p.realization_status=="accepted").count(),
                "unrealized": p.parcels.iter().filter(|p|p.realization_status!="accepted").count(),
                "evidence": "independently-rebuilt-plan-and-origin-ids",
            })),
            "entityKinds": kinds,
            "landUseArea": area_reports.last(),
            "engineeringPlacement": engineering_placement_audit,
        });
        // Only worlds that actually planned a corridor carry the key, so a world
        // without the switch keeps the exact report shape it always had.
        if let Some(audit) = &entry_corridor_audit {
            scene_report["entryCorridorAudit"] = serde_json::json!(audit);
        }
        // Only worlds that actually converted land carry the key, so a manifest without the explicit
        // switch keeps the exact report shape it always had.
        if let Some(audit) = &conversion_audit {
            scene_report["purposeConversionAudit"] = serde_json::json!(audit);
        }
        // Same rule for the opt-in shoreline band: absent unless the world
        // claimed the band by opting into road-first over structured water.
        if let Some(audit) = &shore_band_audit {
            scene_report["shoreBandAudit"] = serde_json::json!(audit);
        }
        scene_reports.push(scene_report);
    }
    let report = serde_json::json!({
        "format": "glyphweave.quality-report", "version": 2,
        "status": if scale_failed || access_surface_failed || conversion_failed || engineering_placement_failed { "fail" } else if warnings.is_empty() { "pass" } else { "warn" },
        "worldRevision": world.revision, "sceneCount": world.scenes.len(),
        "chunks": chunks, "entities": entities, "landmarks": landmarks,
        "engineeringPlacementSkips": engineering_placement_skips,
        "warnings": warnings, "scenes": scene_reports,
        "landUseProfileApplied": profile_applied,
        "landUseProfile": profile.map(|profile| serde_json::json!({
            "theme": profile.theme.as_str(),
            "urbanTarget": profile.urban_target(),
            "ruralTarget": profile.rural_target(),
            "natureTarget": profile.nature_target(),
        })),
        "visualFeedback": visual_feedback,
        "scaleAudit": scale_audit,
        "agentNextAction": "inspect HTML screenshot and create a Patch when visual quality is insufficient",
    });
    fs::write(
        root.join("quality-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

/// Read a style asset contract's size limits for `kind`, falling back to the
/// given procedural defaults when the contract or field is absent.
fn contract_limits(
    contracts: Option<&serde_json::Map<String, serde_json::Value>>,
    kind: &str,
    default_w_lo: f32,
    default_w_hi: f32,
    default_h_lo: f32,
    default_h_hi: f32,
) -> (f32, f32, f32, f32) {
    let Some(c) = contracts.and_then(|m| m.get(kind)) else {
        return (default_w_lo, default_w_hi, default_h_lo, default_h_hi);
    };
    let num = |f: &str, dflt: f32| -> f32 {
        c.get(f)
            .and_then(serde_json::Value::as_f64)
            .map(|v| v as f32)
            .unwrap_or(dflt)
    };
    (
        num("minWidthM", default_w_lo),
        num("maxWidthM", default_w_hi),
        num("minHeightM", default_h_lo),
        num("maxHeightM", default_h_hi),
    )
}

/// BakedTerrain::height already returns metres, not quarter-metre integers.
fn road_is_below_ground(entity: &glyphweave_core::worldgen::EntityInstance, ground_m: f32) -> bool {
    if let Some(profile)=&entity.grounding.road_profile {
        if !ground_m.is_finite() || profile.validate_entity(entity).is_err(){return true;}
        // Both the profile and its required raster use integer millimetres.
        // Reconstruct that integer, not (-1 + 0.904f32) < -0.096f32.
        // No geometric tolerance is added: a one-millimetre burial still fails.
        let ground_mm=(f64::from(ground_m)*1000.).round() as i64;
        return profile.elevations_mm[profile.elevations_mm.len()/2] < ground_mm;
    }
    entity.ground_height_m() < ground_m
}

fn contract_for_entity<'a>(
    contracts: Option<&'a serde_json::Map<String, serde_json::Value>>,
    entity: &glyphweave_core::worldgen::EntityInstance,
    authored: bool,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    let contracts = contracts?;
    // Authored assets get an exact asset contract first. This prevents a large
    // bridge, fence, GIS building, or water object from being forced through a
    // generic procedural kind contract. The manifest can declare:
    // assetContracts["asset:<assetId>"] = {...}.
    if authored {
        let key = format!("asset:{}", entity.asset_id);
        if let Some(contract) = contracts.get(&key).and_then(serde_json::Value::as_object) {
            return Some(contract);
        }
    }
    contracts
        .get(&entity.kind)
        .or_else(|| {
            if matches!(entity.kind.as_str(), "urban_building" | "building_tower") {
                contracts.get("building")
            } else {
                None
            }
        })
        .and_then(serde_json::Value::as_object)
}

fn scale_audit_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("scale-audit", args)?);
    let manifest: WorldManifest =
        serde_json::from_slice(&fs::read(root.join("glyphweave.manifest.json"))?)?;
    manifest.validate()?;
    let mut failures = Vec::new();
    let style = &manifest.style;
    if !manifest.landmarks.is_empty()
        && style
            .get("referenceData")
            .and_then(serde_json::Value::as_object)
            .is_none()
    {
        failures.push("missing style.referenceData".to_owned());
    }
    if let Some(water) = style.get("water").and_then(serde_json::Value::as_object) {
        for field in ["waterType", "levelPolicy", "shoreProfile", "waveModel"] {
            if !water.contains_key(field) {
                failures.push(format!("style.water missing {field}"));
            }
        }
    } else if manifest
        .landmarks
        .iter()
        .any(|landmark| matches!(landmark.entity_type.as_str(), "lake" | "river"))
    {
        failures.push("missing structured style.water model".to_owned());
    }
    let contracts = style
        .get("assetContracts")
        .and_then(serde_json::Value::as_object);
    if contracts.is_none() {
        failures.push("missing style.assetContracts".to_owned());
    }
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let mut audited_entities = 0_usize;
    let mut tree_count = 0_usize;
    let mut authored_count = 0_usize;
    let mut authored_contract_count = 0_usize;
    let authored_kind_fallback_count = 0_usize;
    let mut expected_chunk_total = 0_usize;
    let mut actual_chunk_total = 0_usize;
    let mut max_boundary_step = 0_f64;
    for scene_path in &world.scenes {
        let scene: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
        // Load each scene heightfield once. The old implementation reread a
        // chunk file for every boundary cell and every entity, turning a
        // 60-million-cell audit into thousands of filesystem calls.
        let scene_spec = glyphweave_core::worldgen::SceneSpec {
            scene_id: scene.scene_id.clone(),
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            seed_offset: 0,
        };
        let mut baked_terrain = BakedTerrain::new(&scene_spec)?;
        for chunk in &scene.chunks {
            let bytes = fs::read(
                root.join("scenes")
                    .join(&scene.scene_id)
                    .join(&chunk.height_file),
            )?;
            baked_terrain.insert_chunk(
                chunk.world_x,
                chunk.world_z,
                chunk.valid_width_m,
                chunk.valid_depth_m,
                &bytes,
            )?;
        }
        baked_terrain.require_complete()?;
        let expected_chunks = (scene.chunk_count_x * scene.chunk_count_z) as usize;
        expected_chunk_total += expected_chunks;
        actual_chunk_total += scene.chunks.len();
        let mut covered_chunks = BTreeSet::new();
        if scene.chunks.len() != expected_chunks {
            failures.push(format!(
                "{} has {} chunks, expected {}",
                scene.scene_id,
                scene.chunks.len(),
                expected_chunks
            ));
        }
        for chunk in &scene.chunks {
            covered_chunks.insert((chunk.chunk_x, chunk.chunk_z));
            if chunk.chunk_x >= scene.chunk_count_x || chunk.chunk_z >= scene.chunk_count_z {
                failures.push(format!(
                    "{} chunk ({},{}) is outside declared grid",
                    scene.scene_id, chunk.chunk_x, chunk.chunk_z
                ));
            }
            let chunk_root = root.join("scenes").join(&scene.scene_id);
            let encoding = glyphweave_core::worldgen::height_encoding_from_style(&manifest.style)?;
            let expected_height_bytes = u64::from(chunk.valid_width_m)
                * u64::from(chunk.valid_depth_m)
                * if encoding == HeightEncoding::Millimetre {
                    4
                } else {
                    2
                }
                + if encoding == HeightEncoding::Millimetre {
                    16
                } else {
                    0
                };
            let expected_surface_bytes =
                u64::from(chunk.valid_width_m) * u64::from(chunk.valid_depth_m);
            let lod_width = chunk.valid_width_m.div_ceil(64);
            let lod_depth = chunk.valid_depth_m.div_ceil(64);
            let expected_lod2_bytes = u64::from(lod_width) * u64::from(lod_depth) * 3;
            let mut payload_files = vec![
                (&chunk.height_file, expected_height_bytes),
                (&chunk.surface_file, expected_surface_bytes),
                (&chunk.lod2_file, expected_lod2_bytes),
            ];
            if let Some(terrain_file) = &chunk.terrain_file {
                payload_files.push((
                    terrain_file,
                    u64::from(chunk.valid_width_m) * u64::from(chunk.valid_depth_m) * 4,
                ));
            }
            for (name, expected) in &payload_files {
                match fs::metadata(chunk_root.join(name)) {
                    Ok(metadata) if metadata.len() == *expected => {}
                    Ok(metadata) => failures.push(format!(
                        "{} chunk ({},{}) file {} has {} bytes, expected {}",
                        scene.scene_id,
                        chunk.chunk_x,
                        chunk.chunk_z,
                        name,
                        metadata.len(),
                        expected
                    )),
                    Err(_) => failures.push(format!(
                        "{} chunk ({},{}) file {} is missing",
                        scene.scene_id, chunk.chunk_x, chunk.chunk_z, name
                    )),
                }
            }
            let mut payload = Vec::new();
            for (name, _) in &payload_files {
                match fs::read(chunk_root.join(name)) {
                    Ok(bytes) => payload.extend_from_slice(&bytes),
                    Err(_) => payload.clear(),
                }
                if payload.is_empty() {
                    break;
                }
            }
            if !payload.is_empty() && blake3::hash(&payload).to_hex().to_string() != chunk.hash {
                failures.push(format!(
                    "{} chunk ({},{}) hash mismatch",
                    scene.scene_id, chunk.chunk_x, chunk.chunk_z
                ));
            }
        }
        for chunk_z in 0..scene.chunk_count_z {
            for chunk_x in 0..scene.chunk_count_x {
                if !covered_chunks.contains(&(chunk_x, chunk_z)) {
                    failures.push(format!(
                        "{} missing chunk ({},{})",
                        scene.scene_id, chunk_x, chunk_z
                    ));
                }
            }
        }
        if covered_chunks.len() != scene.chunks.len() {
            failures.push(format!(
                "{} contains duplicate chunk coordinates",
                scene.scene_id
            ));
        }
        for boundary_x in (1..scene.chunk_count_x)
            .map(|value| scene.origin_x + (value * scene.chunk_size_m) as i32)
        {
            for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
                if let (Some(left), Some(right)) = (
                    baked_terrain.height(boundary_x - 1, z).map(f64::from),
                    baked_terrain.height(boundary_x, z).map(f64::from),
                ) {
                    max_boundary_step = max_boundary_step.max((right - left).abs());
                } else {
                    failures.push(format!(
                        "{} missing height at x boundary {}",
                        scene.scene_id, boundary_x
                    ));
                    break;
                }
            }
        }
        for boundary_z in (1..scene.chunk_count_z)
            .map(|value| scene.origin_z + (value * scene.chunk_size_m) as i32)
        {
            for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                if let (Some(top), Some(bottom)) = (
                    baked_terrain.height(x, boundary_z - 1).map(f64::from),
                    baked_terrain.height(x, boundary_z).map(f64::from),
                ) {
                    max_boundary_step = max_boundary_step.max((bottom - top).abs());
                } else {
                    failures.push(format!(
                        "{} missing height at z boundary {}",
                        scene.scene_id, boundary_z
                    ));
                    break;
                }
            }
        }
        if max_boundary_step > 4.0 {
            failures.push(format!(
                "{} boundary height step {} metres exceeds 4",
                scene.scene_id, max_boundary_step
            ));
        }
        let water = water_geometry(
            water_kind(&manifest.style),
            &scene.landmarks,
            &scene_spec,
            &manifest.style,
        );
        let authored_ids: BTreeSet<&str> = manifest
            .landmarks
            .iter()
            .filter(|landmark| landmark.scene_id == scene.scene_id)
            .map(|landmark| landmark.entity_id.as_str())
            .collect();
        for entity in &scene.entities {
            audited_entities += 1;
            let authored = authored_ids.contains(entity.entity_id.as_str());
            if authored {
                authored_count += 1;
            }
            if authored
                && contracts
                    .and_then(|map| map.get(&format!("asset:{}", entity.asset_id)))
                    .is_some()
            {
                authored_contract_count += 1;
            }
            let Some(contract) = contract_for_entity(contracts, entity, authored) else {
                failures.push(format!(
                    "{} has no asset contract for kind {}",
                    entity.entity_id, entity.kind
                ));
                continue;
            };
            for field in ["type", "placement", "allowedSurfaces", "forbiddenSurfaces"] {
                if !contract.contains_key(field) {
                    failures.push(format!("{} contract missing {field}", entity.kind));
                }
            }
            if entity.width_m <= 0.0 || entity.depth_m <= 0.0 || entity.height_m <= 0.0 {
                failures.push(format!("{} has non-positive dimensions", entity.entity_id));
            }
            if matches!(
                entity.kind.as_str(),
                "tree"
                    | "bush"
                    | "rock"
                    | "building"
                    | "urban_building"
                    | "building_tower"
                    | "storefront"
                    | "bench"
                    | "lamp"
                    | "grass_clump"
                    | "fallen_log"
                    | "building_cluster"
            ) {
                let margin = if entity.kind == "reed" {
                    0.0
                } else {
                    // One constant for the published clearance, the lot rule and the
                    // shoreline band: it used to be this bare literal while the
                    // generation side asked for 1.0 m, so a legal bake could produce
                    // objects this audit then refused.
                    glyphweave_core::worldgen::WATER_SHORE_CLEARANCE_M
                };
                if footprint_intersects_water(entity, water, margin) {
                    failures.push(format!(
                        "{} {} is inside water geometry",
                        entity.entity_id, entity.kind
                    ));
                }
            }
            for (field, value, min_field, max_field) in [
                ("widthM", entity.width_m, "minWidthM", "maxWidthM"),
                ("depthM", entity.depth_m, "minDepthM", "maxDepthM"),
                ("heightM", entity.height_m, "minHeightM", "maxHeightM"),
            ] {
                let min = contract
                    .get(min_field)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let max = contract
                    .get(max_field)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(f32::MAX as f64) as f32;
                if value < min || value > max {
                    failures.push(format!(
                        "{} {}={value:.2} outside {min:.2}..{max:.2}",
                        entity.entity_id, field
                    ));
                }
            }
            if entity.kind == "urban_building" {
                for tower in scene
                    .entities
                    .iter()
                    .filter(|candidate| candidate.kind == "building_tower")
                {
                    let overlap_x = (f64::from(entity.world_x - tower.world_x)).abs()
                        < f64::from(entity.width_m + tower.width_m) * 0.5;
                    let overlap_z = (f64::from(entity.world_z - tower.world_z)).abs()
                        < f64::from(entity.depth_m + tower.depth_m) * 0.5;
                    if overlap_x && overlap_z {
                        failures.push(format!(
                            "{} overlaps tower {}",
                            entity.entity_id, tower.entity_id
                        ));
                    }
                }
            }
            match entity.kind.as_str() {
                "tree" => {
                    tree_count += 1;
                    // Use the style asset contract range when present (GIS /
                    // authored trees may legitimately differ from the
                    // procedural default), else fall back to defaults.
                    let (w_lo, w_hi, h_lo, h_hi) =
                        contract_limits(contracts, "tree", 2.0, 5.5, 4.0, 9.0);
                    if !(w_lo..=w_hi).contains(&entity.width_m)
                        || !(h_lo..=h_hi).contains(&entity.height_m)
                    {
                        failures.push(format!(
                            "{} tree dimensions {:.2}x{:.2}x{:.2}m outside range",
                            entity.entity_id, entity.width_m, entity.depth_m, entity.height_m
                        ));
                    }
                }
                "building" => {
                    // Real footprints (GIS) span far wider than the procedural
                    // default; honour the style asset contract when declared.
                    let (w_lo, w_hi, h_lo, h_hi) =
                        contract_limits(contracts, "building", 8.0, 30.0, 8.0, 24.0);
                    if !(w_lo..=w_hi).contains(&entity.width_m)
                        || !(h_lo..=h_hi).contains(&entity.height_m)
                    {
                        failures.push(format!(
                            "{} building dimensions {:.2}x{:.2}x{:.2}m outside range",
                            entity.entity_id, entity.width_m, entity.depth_m, entity.height_m
                        ));
                    }
                }
                "road" => {
                    if let Some(profile)=&entity.grounding.road_profile {
                        if let Err(error)=profile.validate_entity(entity){failures.push(error.to_string());continue;}
                    }
                    // Only procedurally generated roads must sit on the ground
                    // heightfield. GIS/landmark roads carry author-specified
                    // heights (bridges, viaducts) and are exempt.
                    if entity.entity_id.starts_with("generated.") {
                        if let Some(ground_y) = baked_terrain.height(entity.world_x, entity.world_z)
                        {
                            if road_is_below_ground(entity, ground_y) {
                                failures.push(format!(
                                    "{} road y={} is below ground y={}",
                                    entity.entity_id, entity.ground_height_m(), ground_y
                                ));
                            }
                        } else {
                            failures.push(format!(
                                "{} road center is outside baked heightfield",
                                entity.entity_id
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        for landmark in &scene.landmarks {
            if landmark.entity_type == "lake" && landmark.world_y != 0 {
                failures.push(format!(
                    "{} lake is not on local horizontal datum",
                    landmark.entity_id
                ));
            }
        }
    }
    let report = serde_json::json!({
        "format": "glyphweave.scale-audit",
        "version": 1,
        "status": if failures.is_empty() { "pass" } else { "fail" },
        "worldRevision": world.revision,
        "auditedEntities": audited_entities,
        "treeCount": tree_count,
        "authoredCount": authored_count,
        "authoredAssetContractCount": authored_contract_count,
        "authoredKindFallbackCount": authored_kind_fallback_count,
        "chunkCoverage": {
            "scenes": world.scenes.len(),
            "expectedChunks": expected_chunk_total,
            "actualChunks": actual_chunk_total
        },
        "continuity": { "maxBoundaryHeightStepM": max_boundary_step, "maxBoundaryHeightStepQuarterM": max_boundary_step*4.0 },
        "failures": failures,
    });
    fs::write(
        root.join("scale-audit.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report["status"] == "fail" {
        return Err("scale audit failed".into());
    }
    Ok(())
}

fn convert_command(args: &[String]) -> CliResult<()> {
    let mut mode = MigrationMode::Flatten;
    let mut paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--mode" => {
                let value = args.get(index + 1).ok_or("--mode requires a value")?;
                mode = parse_mode(value)?;
                index += 2;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown convert option {option:?}").into());
            }
            path => {
                paths.push(PathBuf::from(path));
                index += 1;
            }
        }
    }
    if paths.len() != 2 {
        return Err("convert requires INPUT and OUTPUT paths".into());
    }

    let input = fs::read(&paths[0])?;
    if input.starts_with(b"PK") {
        return Err(
            "convert expects a legacy JSON .gemap; input is already a ZIP container".into(),
        );
    }
    let migrated = migrate_legacy_json(&input, mode)?;
    let metadata = BTreeMap::from([(
        "migration".to_owned(),
        serde_json::json!({
            "sourceFormat": format!("gemap-v{}", migrated.report.source_version),
            "mode": migrated.report.mode,
            "layerZ": migrated.layer_z,
            "report": migrated.report,
        }),
    )]);
    let encoded = encode_world_with_metadata(&migrated.world, Some(metadata.clone()))?;
    write_atomic(&paths[1], &encoded)?;
    println!("{}", serde_json::to_string_pretty(&metadata["migration"])?);
    Ok(())
}

fn dump_chunk_command(args: &[String]) -> CliResult<()> {
    let mut selector = None;
    let mut limit = Some(DEFAULT_DUMP_LIMIT);
    let mut paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--coord" => {
                if selector.is_some() {
                    return Err("dump-chunk accepts only one selector".into());
                }
                let value = args.get(index + 1).ok_or("--coord requires z,x,y")?;
                selector = Some(DumpSelector::Coord(parse_world_coord(value)?));
                index += 2;
            }
            "--section" => {
                if selector.is_some() {
                    return Err("dump-chunk accepts only one selector".into());
                }
                let value = args
                    .get(index + 1)
                    .ok_or("--section requires cz,rx,ry,rcx,rcy")?;
                selector = Some(DumpSelector::Section(parse_section_selector(value)?));
                index += 2;
            }
            "--limit" => {
                let value = args.get(index + 1).ok_or("--limit requires a value")?;
                limit = Some(parse_limit(value)?);
                index += 2;
            }
            "--all" => {
                limit = None;
                index += 1;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown dump-chunk option {option:?}").into());
            }
            path => {
                paths.push(PathBuf::from(path));
                index += 1;
            }
        }
    }
    if paths.len() != 1 {
        return Err("dump-chunk requires exactly one FILE path".into());
    }
    let selector = selector.ok_or("dump-chunk requires --coord or --section")?;
    dump_chunk(&paths[0], dump_target(selector), limit)
}

fn inspect_command(args: &[String]) -> CliResult<()> {
    let path = one_path("inspect", args)?;
    let world = decode_world(&fs::read(path)?, ArchiveLimits::default())?;
    println!("name: {}", world.name);
    println!("voxels: {}", world.len());
    println!("regions: {}", world.region_count());
    println!("chunks: {}", world.chunk_count());
    println!("blocks: {}", world.registry().len());
    match world.bounds() {
        Some(bounds) => println!(
            "bounds: ({},{},{})..({},{},{})",
            bounds.min.z, bounds.min.x, bounds.min.y, bounds.max.z, bounds.max.x, bounds.max.y
        ),
        None => println!("bounds: empty"),
    }
    for (_, name) in world.registry().iter() {
        println!("block: {name}");
    }
    Ok(())
}

fn validate_command(args: &[String]) -> CliResult<()> {
    let path = one_path("validate", args)?;
    let world = decode_world(&fs::read(path)?, ArchiveLimits::default())?;
    println!(
        "valid .gemap v3: {} voxels, {} chunks",
        world.len(),
        world.chunk_count()
    );
    Ok(())
}

fn compact_command(args: &[String]) -> CliResult<()> {
    let path = one_path("compact", args)?;
    let decoded = decode_world_with_metadata(&fs::read(path)?, ArchiveLimits::default())?;
    let compacted = encode_world_with_metadata(&decoded.world, decoded.metadata)?;
    write_atomic(path, &compacted)?;
    println!("compacted {}", path.display());
    Ok(())
}

fn dump_chunk(path: &Path, target: DumpTarget, limit: Option<usize>) -> CliResult<()> {
    let bytes = fs::read(path)?;
    let entries = read_entries(Cursor::new(bytes), ArchiveLimits::default())?;
    let manifest_entry = entries
        .get("manifest.json")
        .ok_or("archive is missing manifest.json")?;
    let manifest: Manifest = serde_json::from_slice(manifest_entry)?;
    manifest.validate()?;

    let region_key = format!("{},{}", target.region.x, target.region.y);
    let section_key = section_key(target.section);
    let chunk_coord = ChunkCoord::from_region_local(target.region, target.section);
    let Some(region_path) = manifest.regions.get(&region_key) else {
        return print_absent_chunk(path, &target, "region is absent; chunk is all air");
    };
    let region_entry = entries
        .get(region_path)
        .ok_or_else(|| format!("archive is missing {region_path}"))?;
    let region_manifest: RegionManifest = serde_json::from_slice(region_entry)?;
    region_manifest.validate()?;
    if region_manifest.region != (target.region.x, target.region.y) {
        return Err(format!(
            "{region_path} declares region {:?}, expected ({},{})",
            region_manifest.region, target.region.x, target.region.y
        )
        .into());
    }

    let Some(chunk_id_value) = region_manifest.sections.get(&section_key) else {
        return print_absent_chunk(path, &target, "section is absent; chunk is all air");
    };
    let record = region_manifest.chunks.get(chunk_id_value).ok_or_else(|| {
        format!("section {section_key} references missing chunk {chunk_id_value}")
    })?;
    for block_id in &record.palette {
        if !manifest.block_registry.contains_key(block_id) {
            return Err(
                format!("chunk {chunk_id_value} uses unregistered block ID {block_id}").into(),
            );
        }
    }
    let region_dir = region_path
        .strip_suffix("region.json")
        .ok_or_else(|| format!("region path is not canonical: {region_path}"))?;
    let binary_path = format!("{region_dir}{}", record.data);
    let data = entries
        .get(&binary_path)
        .ok_or_else(|| format!("archive is missing {binary_path}"))?;
    let actual_id = chunk_id(&record.palette, record.bits, data);
    if actual_id != *chunk_id_value {
        return Err(format!("chunk {chunk_id_value} canonical ID is {actual_id}").into());
    }
    let indices = unpack_indices(data, record.bits, record.palette.len(), CHUNK_VOLUME)?;

    let selected_voxel = target.selected_local.map(|local| {
        let index = local.index();
        let palette_index = indices[index] as usize;
        let block_id = record.palette[palette_index];
        serde_json::json!({
            "coord": coord_json(target.selected_coord.expect("local implies selected coord")),
            "local": local_json(local),
            "index": index,
            "paletteIndex": palette_index,
            "blockId": block_id,
            "block": block_name(&manifest, block_id),
        })
    });

    let mut non_air_count = 0_usize;
    let mut non_air_voxels = Vec::new();
    for (index, palette_index) in indices.iter().enumerate() {
        let block_id = record.palette[*palette_index as usize];
        if block_id == 0 {
            continue;
        }
        non_air_count += 1;
        if limit.is_none_or(|max| non_air_voxels.len() < max) {
            let local = LocalVoxelCoord::from_index(index)
                .expect("indices below CHUNK_VOLUME are valid local coordinates");
            let coord = VoxelCoord::from_chunk_local(chunk_coord, local);
            non_air_voxels.push(serde_json::json!({
                "coord": coord_json(coord),
                "local": local_json(local),
                "index": index,
                "paletteIndex": palette_index,
                "blockId": block_id,
                "block": block_name(&manifest, block_id),
            }));
        }
    }

    let palette: Vec<_> = record
        .palette
        .iter()
        .map(|&block_id| {
            serde_json::json!({
                "id": block_id,
                "block": block_name(&manifest, block_id),
            })
        })
        .collect();
    let output = serde_json::json!({
        "kind": "glyphweave.gemap.chunkDump",
        "file": path.display().to_string(),
        "region": {
            "key": region_key,
            "coord": [target.region.x, target.region.y],
            "path": region_path,
        },
        "section": {
            "key": section_key,
            "coord": [target.section.z(), target.section.x(), target.section.y()],
        },
        "chunk": {
            "coord": [chunk_coord.z, chunk_coord.x, chunk_coord.y],
            "id": chunk_id_value,
            "path": binary_path,
            "bits": record.bits,
            "binaryBytes": data.len(),
            "palette": palette,
        },
        "selectedVoxel": selected_voxel,
        "nonAirVoxelCount": non_air_count,
        "nonAirVoxelsShown": non_air_voxels.len(),
        "truncated": limit.is_some_and(|max| non_air_count > max),
        "nonAirVoxels": non_air_voxels,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn parse_mode(value: &str) -> CliResult<MigrationMode> {
    match value {
        "flatten" => Ok(MigrationMode::Flatten),
        "preserve-layers" => Ok(MigrationMode::PreserveLayers),
        _ => Err(format!("unsupported migration mode {value:?}").into()),
    }
}

fn parse_world_coord(value: &str) -> CliResult<VoxelCoord> {
    let parts = parse_i32_list(value, 3, "world coordinate")?;
    Ok(VoxelCoord::new(parts[0], parts[1], parts[2]))
}

fn parse_section_selector(value: &str) -> CliResult<DumpSectionSelector> {
    let parts = parse_i32_list(value, 5, "section selector")?;
    let rcx = u8::try_from(parts[3])
        .ok()
        .and_then(|value| (value < 32).then_some(value))
        .ok_or("--section rcx must be in 0..31")?;
    let rcy = u8::try_from(parts[4])
        .ok()
        .and_then(|value| (value < 32).then_some(value))
        .ok_or("--section rcy must be in 0..31")?;
    let section =
        RegionChunkCoord::new(parts[0], rcx, rcy).ok_or("--section rcx/rcy must be in 0..31")?;
    Ok(DumpSectionSelector {
        section,
        region: RegionCoord::new(parts[1], parts[2]),
    })
}

fn parse_i32_list(value: &str, expected_len: usize, label: &str) -> CliResult<Vec<i32>> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != expected_len {
        return Err(format!("{label} must have {expected_len} comma-separated integers").into());
    }
    parts
        .iter()
        .map(|part| {
            part.parse::<i32>()
                .map_err(|_| format!("{label} contains invalid integer {part:?}").into())
        })
        .collect()
}

fn parse_limit(value: &str) -> CliResult<usize> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| format!("--limit must be a non-negative integer, got {value:?}"))?;
    Ok(limit)
}

fn dump_target(selector: DumpSelector) -> DumpTarget {
    match selector {
        DumpSelector::Coord(coord) => {
            let (chunk, local) = coord.split();
            let (region, section) = chunk.split_region();
            DumpTarget {
                region,
                section,
                selected_coord: Some(coord),
                selected_local: Some(local),
            }
        }
        DumpSelector::Section(selector) => DumpTarget {
            region: selector.region,
            section: selector.section,
            selected_coord: None,
            selected_local: None,
        },
    }
}

fn section_key(section: RegionChunkCoord) -> String {
    format!("{},{},{}", section.z(), section.x(), section.y())
}

fn block_name(manifest: &Manifest, block_id: u32) -> String {
    manifest
        .block_registry
        .get(&block_id)
        .cloned()
        .unwrap_or_else(|| format!("<unregistered:{block_id}>"))
}

fn coord_json(coord: VoxelCoord) -> [i32; 3] {
    [coord.z, coord.x, coord.y]
}

fn local_json(local: LocalVoxelCoord) -> [u8; 3] {
    [local.z(), local.x(), local.y()]
}

fn print_absent_chunk(path: &Path, target: &DumpTarget, reason: &str) -> CliResult<()> {
    let chunk = ChunkCoord::from_region_local(target.region, target.section);
    let output = serde_json::json!({
        "kind": "glyphweave.gemap.chunkDump",
        "file": path.display().to_string(),
        "absent": true,
        "reason": reason,
        "region": {
            "key": format!("{},{}", target.region.x, target.region.y),
            "coord": [target.region.x, target.region.y],
            "path": format!("regions/{}.{}/region.json", target.region.x, target.region.y),
        },
        "section": {
            "key": section_key(target.section),
            "coord": [target.section.z(), target.section.x(), target.section.y()],
        },
        "chunk": {
            "coord": [chunk.z, chunk.x, chunk.y],
        },
        "selectedVoxel": target.selected_coord.map(|coord| serde_json::json!({
            "coord": coord_json(coord),
            "local": local_json(target.selected_local.expect("coord implies local")),
            "blockId": 0,
            "block": "glyphweave:air",
        })),
        "nonAirVoxelCount": 0,
        "nonAirVoxelsShown": 0,
        "truncated": false,
        "nonAirVoxels": [],
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn one_path<'a>(command: &str, args: &'a [String]) -> CliResult<&'a Path> {
    if args.len() != 1 {
        return Err(format!("{command} requires exactly one path").into());
    }
    Ok(Path::new(&args[0]))
}

fn write_atomic(target: &Path, bytes: &[u8]) -> CliResult<()> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("output path must have a UTF-8 file name")?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));

    let result = (|| -> std::io::Result<()> {
        fs::write(&temporary, bytes)?;
        let file = fs::OpenOptions::new().write(true).open(&temporary)?;
        file.sync_all()?;
        fs::rename(&temporary, target)?;
        if let Ok(directory) = fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage:\n  glyphweave init-world MANIFEST.json\n  glyphweave generate-demo-world OUTPUT_DIR\n  glyphweave generate-procedural-world OUTPUT_DIR WIDTH_M DEPTH_M [SEED] [THEME] [URBAN_RATIO] [PROFILE]\n  glyphweave generate-world MANIFEST.json OUTPUT_DIR\n  glyphweave apply-patch MANIFEST.json PATCH.json OUTPUT_DIR\n  glyphweave apply-patch-preserving BASE_WORLD MANIFEST.json PATCH.json APPROVALS.json OUTPUT_DIR\n  glyphweave quality-report WORLD_DIR\n  glyphweave scale-audit WORLD_DIR\n  glyphweave export-preview-stream WORLD_DIR\n  glyphweave design-road-corridor WORLD_DIR REQUEST.json DESIGN.json\n  glyphweave verify-road-corridor WORLD_DIR REQUEST.json DESIGN.json\n  glyphweave build-engineering-connection --input INPUT.json --out BUNDLE.json --views DIR --report REPORT.json\n  glyphweave verify-engineering-connection --input INPUT.json --bundle BUNDLE.json --report REPORT.json
  glyphweave export-engineering-preview --input INPUT.json --bundle BUNDLE.json --out NEW_DIR
  glyphweave audit-engineering-preview --preview DIR --report NEW_REPORT.json
  glyphweave export-engineering-context --world WORLD_DIR --window X,Z,W,D --out NEW_DIR [--scene SCENE_ID]
  glyphweave audit-engineering-context --world WORLD_DIR --context DIR --report NEW_REPORT.json [--window X,Z,W,D]
  glyphweave export-context-preview --world WORLD_DIR --context CONTEXT_DIR --out NEW_DIR [--window X,Z,W,D]
  glyphweave audit-context-preview --world WORLD_DIR --context CONTEXT_DIR --preview DIR --report NEW_REPORT.json [--window X,Z,W,D]
  glyphweave saved-context-preview --preview PREVIEW_DIR [--port N] [--report NEW_FILE.json] [--expect-scene SCENE_ID]
  glyphweave export-context-preview3d --world WORLD_DIR --context CONTEXT_DIR --preview V1_DIR --out NEW_DIR [--context-audit REPORT.json]
  glyphweave audit-context-preview3d --world WORLD_DIR --context CONTEXT_DIR --preview V1_DIR --preview3d DIR --report NEW_REPORT.json [--context-audit REPORT.json]
  glyphweave saved-context-preview-handoff --preview PREVIEW_DIR [--port N] [--report NEW_FILE.json] [--expect-scene SCENE_ID]\n  glyphweave preview WORLD_DIR [PORT]\n  glyphweave convert [--mode flatten|preserve-layers] INPUT OUTPUT\n  glyphweave dump-chunk (--coord z,x,y | --section cz,rx,ry,rcx,rcy) [--limit N|--all] FILE\n  glyphweave inspect FILE\n  glyphweave validate FILE\n  glyphweave rules list DIR\n  glyphweave rules validate FILE\n  glyphweave rules check-dir DIR\n  glyphweave rules audit WORLD_DIR --rules DIR [--report PATH]\n  glyphweave intent draft INPUT.txt OUTPUT.json\n  glyphweave intent validate INTENT.json\n  glyphweave intent compile INTENT.json MANIFEST.json\n  glyphweave compact FILE"
    );
}

/// `rules` subcommands: load/validate the declarative object rules.
///   glyphweave rules list DIR                — list objects in a rules dir
///   glyphweave rules validate FILE           — validate one .object.toml
///   glyphweave rules check-dir DIR           — load + validate a whole dir
fn rules_command(args: &[String]) -> CliResult<()> {
    let Some(sub) = args.first().map(String::as_str) else {
        return Err("rules requires a subcommand: list | validate | check-dir | audit".into());
    };
    match sub {
        "list" => {
            let dir = args.get(1).ok_or("rules list requires DIR")?;
            let registry = ObjectRegistry::load_dir(Path::new(dir))?;
            for (id, desc) in &registry.descriptors {
                println!(
                    "{id:<20} kind={:?} phase={:?} p={}",
                    desc.kind, desc.placement.phase, desc.placement.priority
                );
            }
            println!("total: {}", registry.descriptors.len());
            Ok(())
        }
        "validate" => {
            let file = args.get(1).ok_or("rules validate requires FILE")?;
            let desc = load_descriptor(Path::new(file))?;
            println!("OK {}", desc.id);
            Ok(())
        }
        "check-dir" => {
            let dir = args.get(1).ok_or("rules check-dir requires DIR")?;
            let registry = load_dir(Path::new(dir))?;
            println!("validated {} object(s) in {dir}", registry.len());
            Ok(())
        }
        // glyphweave rules check-assets DIR ASSET_ROOT
        // Verify every non-empty `asset` path in the descriptors exists.
        "check-assets" => {
            let dir = args.get(1).ok_or("rules check-assets requires DIR")?;
            let root = args
                .get(2)
                .ok_or("rules check-assets requires ASSET_ROOT")?;
            let registry = ObjectRegistry::load_dir_with_assets(Path::new(dir), Path::new(root))?;
            println!(
                "validated {} object(s) with assets under {root}",
                registry.descriptors.len()
            );
            Ok(())
        }
        // glyphweave rules audit WORLD_DIR [--rules DIR] [--report PATH]
        // Audits an existing baked world against the object rules, writing a
        // JSON report. Ground height is read from the BAKED heightfield so the
        // audit matches what was actually baked (carves included).
        "audit" => {
            let world_dir = args.get(1).ok_or("rules audit requires WORLD_DIR")?;
            let world_dir = Path::new(world_dir);
            let rules_dir = args
                .iter()
                .position(|a| a == "--rules")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from)
                .ok_or("rules audit requires --rules DIR")?;
            let report_path = args
                .iter()
                .position(|a| a == "--report")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from)
                .unwrap_or_else(|| world_dir.join("rules-audit.json"));
            let manifest_path = world_dir.join("glyphweave.manifest.json");
            let manifest: WorldManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;

            manifest.validate()?;
            let mut total = glyphweave_core::rules::ValidationReport::default();
            let mut unruled: Vec<String> = Vec::new();
            let mut checked = 0usize;
            for scene in &manifest.scenes {
                let scene_json = world_dir
                    .join("scenes")
                    .join(&scene.scene_id)
                    .join("scene.json");
                let index: SceneIndex = serde_json::from_slice(&fs::read(&scene_json)?)?;
                if index.scene_id != scene.scene_id
                    || index.width_m != scene.width_m
                    || index.depth_m != scene.depth_m
                    || index.origin_x != scene.origin_x
                    || index.origin_z != scene.origin_z
                {
                    return Err(
                        format!("scene index {} disagrees with manifest", scene.scene_id).into(),
                    );
                }
                let landmarks: Vec<_> = manifest
                    .landmarks
                    .iter()
                    .filter(|l| l.scene_id == scene.scene_id)
                    .cloned()
                    .collect();
                let water = water_geometry(
                    glyphweave_core::worldgen::water_kind(&manifest.style),
                    &landmarks,
                    scene,
                    &manifest.style,
                );
                // Core validates coverage and units; no per-landmark full-map copies.
                let mut baked = BakedTerrain::new(scene)?;
                for chunk in &index.chunks {
                    let bytes = fs::read(
                        world_dir
                            .join("scenes")
                            .join(&scene.scene_id)
                            .join(&chunk.height_file),
                    )?;
                    baked.insert_chunk(
                        chunk.world_x,
                        chunk.world_z,
                        chunk.valid_width_m,
                        chunk.valid_depth_m,
                        &bytes,
                    )?;
                }
                if baked.encoding()
                    != Some(glyphweave_core::worldgen::height_encoding_from_style(
                        &manifest.style,
                    )?)
                {
                    return Err("rules height encoding differs from manifest".into());
                }
                let access = build_access_graph(&index.entities, &baked, water);
                validate_access_surfaces(&access, &baked, &index.access_surfaces)?;
                let report = audit_baked_scene(
                    manifest.world.seed ^ scene.seed_offset,
                    scene,
                    &landmarks,
                    &index.entities,
                    water,
                    &rules_dir,
                    &baked,
                )?;
                total.buildings += report.buildings;
                total.roads += report.roads;
                total.floating_items += report.floating_items;
                total.submerged_items += report.submerged_items;
                total.forbidden_biomes += report.forbidden_biomes;
                total.forbidden_hazards += report.forbidden_hazards;
                total.slope_too_high += report.slope_too_high;
                total.out_of_bounds += report.out_of_bounds;
                total.blocked_entrances += report.blocked_entrances;
                total.geometry_collisions += report.geometry_collisions;
                total.disconnected_roads += report.disconnected_roads;
                total.rejects.extend(report.rejects);
                checked += report.checked_items;
                total.checked_items += report.checked_items;
                total.passed_items += report.passed_items;
                total.rejected_items += report.rejected_items;
                unruled.extend(report.unruled_items.iter().cloned());
            }
            total.seed = manifest.world.seed;
            total.checked_items = checked;
            total.unruled_items = unruled.clone();
            fs::write(&report_path, serde_json::to_vec_pretty(&total)?)?;
            println!(
                "audited {} scene(s); checked={} passed={} rejected={} buildings={} roads={} floating={} submerged={} biome={} hazards={} slope={} out_of_bounds={} blocked_entrances={} collisions={} disconnected={} unruled={}; report={}",
                manifest.scenes.len(),
                checked,
                total.passed_items,
                total.rejected_items,
                total.buildings,
                total.roads,
                total.floating_items,
                total.submerged_items,
                total.forbidden_biomes,
                total.forbidden_hazards,
                total.slope_too_high,
                total.out_of_bounds,
                total.blocked_entrances,
                total.geometry_collisions,
                total.disconnected_roads,
                unruled.len(),
                report_path.display(),
            );
            if !unruled.is_empty() {
                eprintln!(
                    "warn: {} entities had no matching rule (unruled)",
                    unruled.len()
                );
            }
            if total.rejected_items > 0 {
                return Err(format!(
                    "rules audit rejected {} of {} checked entities; report={}",
                    total.rejected_items,
                    total.checked_items,
                    report_path.display()
                )
                .into());
            }
            Ok(())
        }
        other => Err(format!("unknown rules subcommand {other:?}").into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn road_grounding_compares_metres_without_fourfold_conversion() {
        let mut road: glyphweave_core::worldgen::EntityInstance =
            serde_json::from_value(serde_json::json!({
                "entityId":"generated.road","assetId":"prop.road","kind":"road",
                "worldX":10,"worldZ":10,"worldY":6,"widthM":20,"depthM":8,"heightM":0.2,"scale":1
            }))
            .unwrap();
        assert!(!road_is_below_ground(&road, 6.));
        assert!(!road_is_below_ground(&road, 5.75));
        assert!(road_is_below_ground(&road, 6.25));
        road.grounding.roadbed_offset_m = 0.25;
        assert!(!road_is_below_ground(&road, 6.25));
    }

    #[test]
    fn graded_road_grounding_compares_exact_millimetres_without_cancellation() {
        for mm in [-31999_i64,-322,-96,0,96,2049,5861,255999] {
            let level=mm as f64/1000.;
            let mut road:glyphweave_core::worldgen::EntityInstance=serde_json::from_value(serde_json::json!({
                "entityId":"generated.road-auto.test","assetId":"prop.road","kind":"road","worldX":0,"worldZ":0,"worldY":level.floor() as i32,"widthM":64,"depthM":8,"heightM":0.24,"scale":1,
                "grounding":{"bottomOffsetM":level-level.floor(),"roadProfile":{"version":1,"start":[-32,0],"end":[32,0],"elevationsMm":vec![mm;65],"maxFillMm":6000}}
            })).unwrap();
            assert!(!road_is_below_ground(&road,level as f32),"{mm}mm must not fail from f32 cancellation");
            assert!(road_is_below_ground(&road,(level+0.001) as f32),"1mm burial must fail");
            road.grounding.bottom_offset_m+=0.01;
            assert!(road_is_below_ground(&road,level as f32),"invalid profile/entity centre must not bypass checks");
        }
    }
    #[test]
    fn authored_asset_contract_takes_precedence_over_kind_contract() {
        let contracts = serde_json::json!({
            "building": {"type":"generic"},
            "asset:bridge.custom": {"type":"bridge", "minWidthM":100}
        });
        let map = contracts.as_object().unwrap();
        let entity = glyphweave_core::worldgen::EntityInstance {
            entity_id: "custom-landmark".into(),
            asset_id: "bridge.custom".into(),
            kind: "building".into(),
            world_x: 0,
            world_z: 0,
            world_y: 0,
            scale: 1.,
            width_m: 120.,
            depth_m: 12.,
            height_m: 20.,
            rotation_y_deg: 0.,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        assert_eq!(
            contract_for_entity(Some(map), &entity, true).unwrap()["type"],
            "bridge"
        );
        assert_eq!(
            contract_for_entity(Some(map), &entity, false).unwrap()["type"],
            "generic"
        );
    }

    use super::*;

    #[test]
    fn parses_migration_modes() {
        assert_eq!(parse_mode("flatten").unwrap(), MigrationMode::Flatten);
        assert_eq!(
            parse_mode("preserve-layers").unwrap(),
            MigrationMode::PreserveLayers
        );
        assert!(parse_mode("layers-as-height").is_err());
    }

    #[test]
    fn validates_single_path_commands() {
        let paths = vec!["world.gemap".to_owned()];
        assert_eq!(
            one_path("validate", &paths).unwrap(),
            Path::new("world.gemap")
        );
        assert!(one_path("validate", &[]).is_err());
    }

    #[test]
    fn parses_dump_world_coordinate_to_region_section_and_local() {
        let target = dump_target(DumpSelector::Coord(
            parse_world_coord("-1,-1,-513").unwrap(),
        ));

        assert_eq!(target.region, RegionCoord::new(-1, -2));
        assert_eq!(target.section.z(), -1);
        assert_eq!(target.section.x(), 31);
        assert_eq!(target.section.y(), 31);
        assert_eq!(
            target.selected_local,
            Some(LocalVoxelCoord::new(15, 15, 15).unwrap())
        );
    }

    #[test]
    fn parses_dump_section_selector() {
        let selector = parse_section_selector("-2,3,-4,31,0").unwrap();

        assert_eq!(selector.region, RegionCoord::new(3, -4));
        assert_eq!(selector.section.z(), -2);
        assert_eq!(selector.section.x(), 31);
        assert_eq!(selector.section.y(), 0);
        assert!(parse_section_selector("0,0,0,32,0").is_err());
    }
}
