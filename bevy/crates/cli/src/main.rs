use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

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
    LandUseProfile, SceneIndex, WorldManifest, WorldPatch, analyze_landuse_areas, apply_patch,
    audit_scene, bake_world, water_geometry, write_demo_manifest,
};

type CliResult<T> = Result<T, Box<dyn Error>>;
const DEFAULT_DUMP_LIMIT: usize = 64;

fn river_half_width_at(z: f64, scene: &glyphweave_core::worldgen::SceneIndex, base: f64) -> f64 {
    if scene.depth_m < 3_000 {
        return base.max(1.0);
    }
    let t = ((z - f64::from(scene.origin_z)) / f64::from(scene.depth_m)).clamp(0.0, 1.0);
    let broad_bend = (t * std::f64::consts::TAU * 1.15).sin() * 0.08;
    let harbour_bay = ((t - 0.28) * std::f64::consts::TAU * 3.0).sin().max(0.0) * 0.045;
    (base * (1.0 + broad_bend + harbour_bay)).max(1.0)
}

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
        "quality-report" => quality_report_command(&args[1..]),
        "validate-world" => validate_world_command(&args[1..]),
        "check-baseline" => check_baseline_command(&args[1..]),
        "scale-audit" => scale_audit_command(&args[1..]),
        "preview" => preview_command(&args[1..]),
        "convert" => convert_command(&args[1..]),
        "dump-chunk" => dump_chunk_command(&args[1..]),
        "inspect" => inspect_command(&args[1..]),
        "validate" => validate_command(&args[1..]),
        "rules" => rules_command(&args[1..]),
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
        let request = read_http_request(&mut stream)?;
        let line = String::from_utf8_lossy(&request);
        let request_line = line.lines().next().unwrap_or("");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or("GET");
        let requested = request_parts.next().unwrap_or("/");
        if method == "POST" && requested == "/api/feedback" {
            let body = line.split_once("\r\n\r\n").map_or("", |(_, body)| body);
            let feedback: serde_json::Value = serde_json::from_str(body)?;
            fs::write(
                root.join("visual-feedback.json"),
                serde_json::to_vec_pretty(&feedback)?,
            )?;
            let response = serde_json::to_vec(
                &serde_json::json!({"ok": true, "path": "visual-feedback.json"}),
            )?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            )?;
            stream.write_all(&response)?;
            continue;
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
    }
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
    if !(3..=6).contains(&args.len()) {
        return Err(
            "generate-procedural-world requires OUTPUT_DIR WIDTH_M DEPTH_M [SEED] [THEME] [URBAN_RATIO]"
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
    let report = serde_json::json!({
        "format": "glyphweave.generation-report",
        "version": 1,
        "status": "baked",
        "nextGate": "scale-audit",
        "worldRevision": index.revision,
        "sceneCount": index.scenes.len(),
        "chunkCount": chunk_count,
        "entityCount": entity_count,
        "landmarkCount": landmark_count,
        "renderMode": index.render_mode,
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

fn validate_world_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("validate-world", args)?);
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
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

fn quality_report_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("quality-report", args)?);
    let world: glyphweave_core::worldgen::WorldIndex =
        serde_json::from_slice(&fs::read(root.join("world.json"))?)?;
    let mut scene_reports = Vec::new();
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
    let profile = LandUseProfile::from_style(&manifest.style);
    let mut area_reports = Vec::new();
    for scene_path in &world.scenes {
        let scene: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
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
        if let Some(profile) = &profile {
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
        area_reports.push(serde_json::json!({
            "sceneId": scene.scene_id,
            "areaM2": areas.scene_area_m2,
            "urbanM2": areas.urban_m2,
            "ruralM2": areas.rural_m2,
            "natureM2": areas.nature_m2,
            "urbanRatio": areas.urban_ratio,
            "ruralRatio": areas.rural_ratio,
            "natureRatio": areas.nature_ratio,
            "byKindM2": areas.by_kind,
        }));
        scene_reports.push(serde_json::json!({
            "sceneId": scene.scene_id, "sizeM": [scene.width_m, scene.depth_m],
            "chunks": scene.chunks.len(), "expectedChunks": expected,
            "landmarks": scene.landmarks.len(), "entities": scene.entities.len(),
            "entityKinds": kinds,
            "landUseArea": area_reports.last(),
        }));
    }
    let report = serde_json::json!({
        "format": "glyphweave.quality-report", "version": 1,
        "status": if scale_failed { "fail" } else if warnings.is_empty() { "pass" } else { "warn" },
        "worldRevision": world.revision, "sceneCount": world.scenes.len(),
        "chunks": chunks, "entities": entities, "landmarks": landmarks,
        "warnings": warnings, "scenes": scene_reports,
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

fn scale_audit_command(args: &[String]) -> CliResult<()> {
    let root = Path::new(one_path("scale-audit", args)?);
    let manifest: WorldManifest =
        serde_json::from_slice(&fs::read(root.join("glyphweave.manifest.json"))?)?;
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
    let mut expected_chunk_total = 0_usize;
    let mut actual_chunk_total = 0_usize;
    let mut max_boundary_step = 0_i32;
    for scene_path in &world.scenes {
        let scene: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_slice(&fs::read(root.join(scene_path))?)?;
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
            let expected_height_bytes =
                u64::from(chunk.valid_width_m) * u64::from(chunk.valid_depth_m) * 2;
            let expected_surface_bytes =
                u64::from(chunk.valid_width_m) * u64::from(chunk.valid_depth_m);
            let lod_width = chunk.valid_width_m.div_ceil(64);
            let lod_depth = chunk.valid_depth_m.div_ceil(64);
            let expected_lod2_bytes = u64::from(lod_width) * u64::from(lod_depth) * 3;
            for (name, expected) in [
                (&chunk.height_file, expected_height_bytes),
                (&chunk.surface_file, expected_surface_bytes),
                (&chunk.lod2_file, expected_lod2_bytes),
            ] {
                match fs::metadata(chunk_root.join(name)) {
                    Ok(metadata) if metadata.len() == expected => {}
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
            for name in [&chunk.height_file, &chunk.surface_file, &chunk.lod2_file] {
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
                    scene_height_at(&root, &scene, boundary_x - 1, z),
                    scene_height_at(&root, &scene, boundary_x, z),
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
                    scene_height_at(&root, &scene, x, boundary_z - 1),
                    scene_height_at(&root, &scene, x, boundary_z),
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
        if max_boundary_step > 16 {
            failures.push(format!(
                "{} boundary height step {} quarter-meters exceeds 16",
                scene.scene_id, max_boundary_step
            ));
        }
        let water_zones: Vec<(f64, f64, f64, f64, String)> = scene
            .landmarks
            .iter()
            .filter(|landmark| matches!(landmark.entity_type.as_str(), "river" | "lake"))
            .map(|landmark| {
                let half_w = f64::from(landmark.width_m) * 0.5;
                let half_d = f64::from(landmark.depth_m) * 0.5;
                (
                    f64::from(landmark.world_x),
                    f64::from(landmark.world_z),
                    half_w,
                    half_d,
                    landmark.entity_type.clone(),
                )
            })
            .collect();
        for entity in &scene.entities {
            audited_entities += 1;
            let Some(contract) = contracts
                .and_then(|items| {
                    items.get(&entity.kind).or_else(|| {
                        if matches!(entity.kind.as_str(), "urban_building" | "building_tower") {
                            items.get("building")
                        } else {
                            None
                        }
                    })
                })
                .and_then(serde_json::Value::as_object)
            else {
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
                // GIS / real-data footprints are positioned by the source map,
                // including legitimate waterfront structures; the procedural
                // water-zone check only applies to generated entities.
                if entity.entity_id.starts_with("gis.") {
                    continue;
                }
                for (zone_x, zone_z, half_w, half_d, zone_kind) in &water_zones {
                    let margin = match entity.kind.as_str() {
                        "reed" => 0.0,
                        _ => 4.0,
                    };
                    let half_x = f64::from(entity.width_m) * 0.5 + margin;
                    let half_z = f64::from(entity.depth_m) * 0.5 + margin;
                    let samples = [
                        (f64::from(entity.world_x), f64::from(entity.world_z)),
                        (
                            f64::from(entity.world_x) - half_x,
                            f64::from(entity.world_z) - half_z,
                        ),
                        (
                            f64::from(entity.world_x) - half_x,
                            f64::from(entity.world_z) + half_z,
                        ),
                        (
                            f64::from(entity.world_x) + half_x,
                            f64::from(entity.world_z) - half_z,
                        ),
                        (
                            f64::from(entity.world_x) + half_x,
                            f64::from(entity.world_z) + half_z,
                        ),
                    ];
                    let inside = samples.iter().any(|(x, z)| {
                        if zone_kind == "river" {
                            let effective_half_w = river_half_width_at(*z, &scene, *half_w);
                            (*x - zone_x).abs() < effective_half_w - margin
                        } else {
                            ((*x - zone_x) / half_w).powi(2) + ((*z - zone_z) / half_d).powi(2)
                                < 1.0
                        }
                    });
                    if inside {
                        failures.push(format!(
                            "{} {} is inside {zone_kind} water zone",
                            entity.entity_id, entity.kind
                        ));
                    }
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
                    // Only procedurally generated roads must sit on the ground
                    // heightfield. GIS/landmark roads carry author-specified
                    // heights (bridges, viaducts) and are exempt.
                    if entity.entity_id.starts_with("generated.") {
                        if let Some(ground_y) =
                            scene_height_at(&root, &scene, entity.world_x, entity.world_z)
                        {
                            if entity.world_y < ground_y {
                                failures.push(format!(
                                    "{} road y={} is below ground y={}",
                                    entity.entity_id, entity.world_y, ground_y
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
        "chunkCoverage": {
            "scenes": world.scenes.len(),
            "expectedChunks": expected_chunk_total,
            "actualChunks": actual_chunk_total
        },
        "continuity": { "maxBoundaryHeightStepQuarterM": max_boundary_step },
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

fn scene_height_at(
    root: &Path,
    scene: &glyphweave_core::worldgen::SceneIndex,
    world_x: i32,
    world_z: i32,
) -> Option<i32> {
    let chunk = scene.chunks.iter().find(|chunk| {
        world_x >= chunk.world_x
            && world_x < chunk.world_x + chunk.valid_width_m as i32
            && world_z >= chunk.world_z
            && world_z < chunk.world_z + chunk.valid_depth_m as i32
    })?;
    let width = chunk.valid_width_m as usize;
    let x = (world_x - chunk.world_x) as usize;
    let z = (world_z - chunk.world_z) as usize;
    let bytes = fs::read(
        root.join("scenes")
            .join(&scene.scene_id)
            .join(&chunk.height_file),
    )
    .ok()?;
    let offset = (z * width + x) * 2;
    let value = i16::from_le_bytes([*bytes.get(offset)?, *bytes.get(offset + 1)?]);
    Some(i32::from(value) / 4)
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
        "Usage:\n  glyphweave init-world MANIFEST.json\n  glyphweave generate-demo-world OUTPUT_DIR\n  glyphweave generate-procedural-world OUTPUT_DIR WIDTH_M DEPTH_M [SEED] [THEME] [URBAN_RATIO]\n  glyphweave generate-world MANIFEST.json OUTPUT_DIR\n  glyphweave apply-patch MANIFEST.json PATCH.json OUTPUT_DIR\n  glyphweave quality-report WORLD_DIR\n  glyphweave scale-audit WORLD_DIR\n  glyphweave preview WORLD_DIR [PORT]\n  glyphweave convert [--mode flatten|preserve-layers] INPUT OUTPUT\n  glyphweave dump-chunk (--coord z,x,y | --section cz,rx,ry,rcx,rcy) [--limit N|--all] FILE\n  glyphweave inspect FILE\n  glyphweave validate FILE\n  glyphweave rules list DIR\n  glyphweave rules validate FILE\n  glyphweave rules check-dir DIR\n  glyphweave rules audit WORLD_DIR --rules DIR [--report PATH]\n  glyphweave compact FILE"
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

            let mut total = glyphweave_core::rules::ValidationReport::default();
            let mut unruled: Vec<String> = Vec::new();
            let mut checked = 0usize;
            for scene in &manifest.scenes {
                let scene_json = world_dir
                    .join("scenes")
                    .join(&scene.scene_id)
                    .join("scene.json");
                let index: SceneIndex = serde_json::from_slice(&fs::read(&scene_json)?)?;
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
                // Load the baked heightfield for this scene into a query map.
                let mut baked = BTreeMap::<i64, i16>::new();
                for chunk in &index.chunks {
                    let hfile = world_dir
                        .join("scenes")
                        .join(&scene.scene_id)
                        .join(&chunk.height_file);
                    let bytes = fs::read(&hfile)?;
                    let width = chunk.valid_width_m as usize;
                    let depth = chunk.valid_depth_m as usize;
                    let expected = width * depth * 2;
                    if bytes.len() != expected {
                        return Err(format!(
                            "heightfield {} has {} bytes, expected {expected} ({}x{}x2); refusing to audit with possibly missing terrain",
                            chunk.height_file, bytes.len(), width, depth
                        )
                        .into());
                    }
                    for (i, pair) in bytes.chunks_exact(2).enumerate() {
                        let raw = i16::from_le_bytes([pair[0], pair[1]]);
                        let lx = i as i32 % width as i32;
                        let lz = i as i32 / width as i32;
                        let wx = chunk.world_x + lx;
                        let wz = chunk.world_z + lz;
                        baked.insert(i64::from(wx) << 32 | (wz as u32 as i64), raw);
                    }
                }
                // Verify every entity's centre has a baked height sample; a
                // missing point must be an error, not a silent height-0.
                for e in &index.entities {
                    let key = (i64::from(e.world_x) << 32) | (e.world_z as u32 as i64);
                    if !baked.contains_key(&key) {
                        return Err(format!(
                            "entity {} at ({},{}) has no baked height sample; refusing to audit",
                            e.entity_id, e.world_x, e.world_z
                        )
                        .into());
                    }
                }
                let height_at = |x: i32, z: i32| -> f32 {
                    baked
                        .get(&((i64::from(x) << 32) | (z as u32 as i64)))
                        .map(|v| f32::from(*v) / 4.0)
                        .unwrap_or(0.0)
                };
                let opts = glyphweave_core::worldgen::AuditOptions {
                    height_at: Some(&height_at),
                    slope_half: Some((8, 8)),
                };
                let report = audit_scene(
                    manifest.world.seed ^ scene.seed_offset,
                    scene,
                    &landmarks,
                    &index.entities,
                    water,
                    &rules_dir,
                    opts,
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
            Ok(())
        }
        other => Err(format!("unknown rules subcommand {other:?}").into()),
    }
}

#[cfg(test)]
mod tests {
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
