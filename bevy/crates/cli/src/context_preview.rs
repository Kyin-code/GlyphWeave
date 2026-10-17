//! `engineering-context-preview`: a read-only preview consumer of an audited
//! saved-scene context (G24).
//!
//! The export re-runs an independent rebuild of the caller-given world root and
//! refuses unless the audited context still matches it, then writes a render
//! model, a self-contained viewer, a description, a manifest and a completion
//! marker. The audit re-reads from the source root again and compares the model
//! field by field in four stages. Nothing here builds an `EngineeringInput`,
//! routes, grades terrain, or promotes a saved observation into a verified
//! route: every access record keeps `sourceObservationOnly` and
//! `notReverified`, and the viewer can only display source facts.

use super::engineering_context::{
    check_report_path, claim_directory, compare_stage, publish, release, write_report, Stage,
};
use glyphweave_core::engineering_context::{
    build_context, resolve_within_root, BoundScene, WindowRect,
};
use glyphweave_core::engineering_preview::sha256_hex;
use serde_json::{json, Map, Value};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

type CliResult<T> = Result<T, Box<dyn Error>>;

const PREVIEW_SCHEMA: &str = "engineering-context-preview-v1";
const PACKAGE_SCHEMA: &str = "context-preview-audit-v1";
const SUPPORTED_CONTEXT_VERSIONS: [&str; 1] = ["engineering-context-v3"];
const MODEL_FILE: &str = "render-model.json";
const VIEWER_FILE: &str = "viewer.js";
const HTML_FILE: &str = "preview.html";
const README_FILE: &str = "README.md";
const MANIFEST_FILE: &str = "manifest.json";
const MARKER_FILE: &str = "COMPLETE.json";
/// Required preview files. This set is required independently of any
/// declaration inside the package.
const REQUIRED_FILES: [&str; 6] = [
    MODEL_FILE,
    VIEWER_FILE,
    HTML_FILE,
    README_FILE,
    MANIFEST_FILE,
    MARKER_FILE,
];
/// Closed field set of the render model: a field with no rule cannot appear.
const MODEL_FIELDS: [&str; 17] = [
    "schemaVersion",
    "contextSchemaVersion",
    "sources",
    "window",
    "coordinateSystem",
    "heightUnit",
    "surfaces",
    "entities",
    "roads",
    "planEdgeReferences",
    "access",
    "unknownKinds",
    "unmodelledKinds",
    "schemaDrift",
    "droppedUnconsumedKeys",
    "limitations",
    "status",
];
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn run(args: &[String], audit: bool) -> CliResult<()> {
    if audit {
        audit_preview(args)
    } else {
        export_preview(args)
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == name {
            return args.get(index + 1).cloned();
        }
        index += 1;
    }
    None
}

fn required(args: &[String], name: &str) -> CliResult<String> {
    flag(args, name).ok_or_else(|| format!("missing required flag {name}").into())
}

fn read_confined(root: &Path, relative: &str, budget: &mut u64) -> Result<Vec<u8>, String> {
    let resolved = resolve_within_root(root, relative).map_err(|error| {
        format!("{relative} is not a confined path inside {}: {error}", root.display())
    })?;
    let metadata = fs::metadata(&resolved).map_err(|error| format!("{relative} is unreadable: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{relative} is not a regular file"));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("{relative} exceeds the per-file byte cap"));
    }
    *budget = budget.saturating_add(metadata.len());
    if *budget > MAX_TOTAL_BYTES {
        return Err("the package exceeds the cumulative byte cap".to_string());
    }
    fs::read(&resolved).map_err(|error| format!("{relative} is unreadable: {error}"))
}

fn parse_object(bytes: &[u8], name: &str) -> Result<Value, String> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(_) => Err(format!("{name} is not a JSON object")),
        Err(error) => Err(format!("{name} is not readable JSON: {error}")),
    }
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::to_string)
}

/// The audited G24 context package: three confined files whose hash chain and
/// schema version must hold before anything is previewed.
pub(super) struct ContextPackage {
    pub(super) context: Value,
    context_sha256: String,
    manifest_sha256: String,
    marker_sha256: String,
    context_bytes: u64,
}

pub(super) fn load_context_package(package: &Path) -> Result<ContextPackage, String> {
    let root = fs::canonicalize(package)
        .map_err(|error| format!("context package {} is unusable: {error}", package.display()))?;
    if !root.is_dir() {
        return Err("the context package is not a directory".to_string());
    }
    let mut budget = 0u64;
    let context_bytes = read_confined(&root, "engineering-context.json", &mut budget)?;
    let manifest_bytes = read_confined(&root, "manifest.json", &mut budget)?;
    let marker_bytes = read_confined(&root, "COMPLETE.json", &mut budget)?;
    let context = parse_object(&context_bytes, "engineering-context.json")?;
    let manifest = parse_object(&manifest_bytes, "manifest.json")?;
    let marker = parse_object(&marker_bytes, "COMPLETE.json")?;
    let context_sha256 = sha256_hex(&context_bytes);
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let marker_sha256 = sha256_hex(&marker_bytes);
    if marker.get("status").and_then(Value::as_str) != Some("complete") {
        return Err("the context package has no completion marker".to_string());
    }
    if string_at(&marker, &["contextSha256"]).as_deref() != Some(context_sha256.as_str()) {
        return Err("the context marker does not hash the context file".to_string());
    }
    if string_at(&marker, &["manifestSha256"]).as_deref() != Some(manifest_sha256.as_str()) {
        return Err("the context marker does not hash the manifest".to_string());
    }
    if string_at(&manifest, &["contextSha256"]).as_deref() != Some(context_sha256.as_str()) {
        return Err("the context manifest does not hash the context file".to_string());
    }
    let version = string_at(&context, &["schemaVersion"]).unwrap_or_default();
    if !SUPPORTED_CONTEXT_VERSIONS.contains(&version.as_str()) {
        return Err(format!(
            "unsupported context schemaVersion {version}; supported: {SUPPORTED_CONTEXT_VERSIONS:?}"
        ));
    }
    Ok(ContextPackage {
        context,
        context_sha256,
        manifest_sha256,
        marker_sha256,
        context_bytes: context_bytes.len() as u64,
    })
}

pub(super) fn context_window(context: &Value) -> Result<WindowRect, String> {
    let window = context
        .get("window")
        .ok_or_else(|| "the context declares no window".to_string())?;
    let read = |field: &str| -> Result<i32, String> {
        let raw = window
            .get(field)
            .and_then(Value::as_i64)
            .ok_or_else(|| format!("window.{field} is missing or not an integer"))?;
        i32::try_from(raw).map_err(|_| format!("window.{field} {raw} does not fit in i32"))
    };
    let parsed = WindowRect {
        min_x: read("minX")?,
        min_z: read("minZ")?,
        width_m: read("widthM")?,
        depth_m: read("depthM")?,
    };
    parsed
        .check_shape()
        .map_err(|error| format!("window is not a bounded input: {error}"))?;
    Ok(parsed)
}

/// A bounded, independent rebuild of the caller-given world root.
pub(super) struct Rebuild {
    pub(super) context: Value,
    world_path: String,
    world_sha256: String,
    world_bytes: u64,
    scene_path: String,
    scene_sha256: String,
    scene_bytes: u64,
}

pub(super) fn rebuild(world: &Path, window: WindowRect, expected_context: &Value) -> Result<Rebuild, String> {
    let bound = BoundScene::read(world, window)
        .map_err(|error| format!("the caller-given source could not be rebuilt: {error}"))?;
    let context = build_context(&bound)
        .map_err(|error| format!("the rebuilt source could not produce a context: {error}"))?;
    let fresh: Value = serde_json::from_slice(
        &serde_json::to_vec(&context).map_err(|error| format!("cannot serialise: {error}"))?,
    )
    .map_err(|error| format!("cannot re-read the rebuilt context: {error}"))?;
    if let Some(difference) = first_difference(&fresh, expected_context, "context") {
        return Err(format!(
            "the audited context no longer matches an independent rebuild: {difference}"
        ));
    }
    let world_binding = bound
        .bindings()
        .iter()
        .find(|binding| binding.role == "world-index")
        .ok_or_else(|| "the rebuild recorded no world binding".to_string())?;
    let scene_binding = bound
        .bindings()
        .iter()
        .find(|binding| binding.role == "scene-index")
        .ok_or_else(|| "the rebuild recorded no scene binding".to_string())?;
    Ok(Rebuild {
        context: fresh,
        world_path: world_binding.path.clone(),
        world_sha256: world_binding.sha256.clone(),
        world_bytes: world_binding.bytes,
        scene_path: scene_binding.path.clone(),
        scene_sha256: scene_binding.sha256.clone(),
        scene_bytes: scene_binding.bytes,
    })
}

/// First differing path between two values, using the shared comparison so that
/// numeric representation never counts as a change.
fn first_difference(fresh: &Value, package: &Value, path: &str) -> Option<String> {
    match (fresh, package) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, left_value) in left {
                let child = format!("{path}.{key}");
                match right.get(key) {
                    Some(right_value) => {
                        if let Some(found) = first_difference(left_value, right_value, &child) {
                            return Some(found);
                        }
                    }
                    None => return Some(format!("{child} is missing from the package")),
                }
            }
            for key in right.keys() {
                if !left.contains_key(key) {
                    return Some(format!("{path}.{key} is not produced by the source"));
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path} length differs: source {} vs package {}",
                    left.len(),
                    right.len()
                ));
            }
            for (index, (left_value, right_value)) in left.iter().zip(right.iter()).enumerate() {
                if let Some(found) =
                    first_difference(left_value, right_value, &format!("{path}[{index}]"))
                {
                    return Some(found);
                }
            }
            None
        }
        (left, right) => {
            let probe = json!({"v": left});
            let other = json!({"v": right});
            if compare_stage("probe", &probe, &other, &["v"]).state == "pass" {
                None
            } else {
                Some(format!("{path} differs: source {left} vs package {right}"))
            }
        }
    }
}

/// Builds the read-only render model from a verified context. Every listed
/// source fact is copied verbatim; nothing is interpolated, invented or
/// summarised away.
pub(super) fn build_model(
    context: &Value,
    package: &ContextPackage,
    rebuilt: &Rebuild,
    window: WindowRect,
) -> Value {
    let crop = context.get("crop").cloned().unwrap_or(Value::Null);
    let access_records = context
        .get("accessRecords")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let entities = context
        .get("entities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let roads = context
        .get("roads")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let access: Vec<Value> = access_records
        .iter()
        .map(|record| {
            json!({
                "entityId": record.get("entityId"),
                "anchorId": record.get("anchorId"),
                "roadId": record.get("roadId"),
                "distanceM": record.get("distanceM"),
                "distanceMmDerived": record.get("distanceMmDerived"),
                "connectedInSource": record.get("connectedInSource"),
                "requiredInSource": record.get("requiredInSource"),
                "status": record.get("status"),
                "path": record.get("path").cloned().unwrap_or(Value::Array(Vec::new())),
                "inWindow": record.get("inWindow"),
                "referenceOnly": record.get("referenceOnly"),
                "sourceObservationOnly": true,
                "notReverified": true,
            })
        })
        .collect();
    let unmodelled = context
        .get("unmodelledKinds")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let schema_drift = context
        .get("sceneBinding")
        .and_then(|binding| binding.get("sceneSchemaDrift"))
        .cloned()
        .unwrap_or(Value::Null);
    let natural_equals_final = crop
        .get("naturalEqualsFinal")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "schemaVersion": PREVIEW_SCHEMA,
        "contextSchemaVersion": context.get("schemaVersion"),
        "sources": {
            "world": {"path": rebuilt.world_path, "sha256": rebuilt.world_sha256, "bytes": rebuilt.world_bytes},
            "scene": {"path": rebuilt.scene_path, "sha256": rebuilt.scene_sha256, "bytes": rebuilt.scene_bytes},
            "context": {"sha256": package.context_sha256, "bytes": package.context_bytes},
            "manifest": {"sha256": package.manifest_sha256},
            "marker": {"sha256": package.marker_sha256},
            "declaredSceneId": context.get("sceneBinding").and_then(|binding| binding.get("declaredSceneId")),
            "sceneId": context.get("sceneBinding").and_then(|binding| binding.get("sceneId")),
        },
        "window": {"minX": window.min_x, "minZ": window.min_z, "widthM": window.width_m, "depthM": window.depth_m},
        "coordinateSystem": "worldMetresXEastZSouthYUp",
        "heightUnit": "millimetre",
        "surfaces": {
            "natural": {
                "sourceField": "crop.naturalMm",
                "widthM": crop.get("widthM"),
                "depthM": crop.get("depthM"),
                "samples": crop.get("naturalMm").cloned().unwrap_or(Value::Array(Vec::new())),
            },
            "final": {
                "sourceField": "crop.finalMm",
                "widthM": crop.get("widthM"),
                "depthM": crop.get("depthM"),
                "samples": crop.get("finalMm").cloned().unwrap_or(Value::Array(Vec::new())),
            },
            "naturalEqualsFinal": natural_equals_final,
            "differingSamples": crop.get("differingSamples"),
        },
        "entities": entities,
        "roads": roads,
        "planEdgeReferences": context.get("planEdgeReferences").cloned().unwrap_or(Value::Array(Vec::new())),
        "access": {"scope": context.get("accessScope"), "records": access},
        "unknownKinds": unmodelled,
        "unmodelledKinds": context.get("unmodelledKinds").cloned().unwrap_or_else(|| json!({})),
        "schemaDrift": schema_drift,
        "droppedUnconsumedKeys": context.get("sceneBinding").and_then(|binding| binding.get("droppedUnconsumedKeys")).cloned().unwrap_or(Value::Array(Vec::new())),
        "limitations": context.get("limitations").cloned().unwrap_or(Value::Array(Vec::new())),
        "status": {
            "entityCount": context.get("entities").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "roadCount": context.get("roads").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "accessRecordCount": access_records.len(),
            "accessRecordsInWindow": context.get("derived").and_then(|derived| derived.get("accessRecordsInWindow")).cloned().unwrap_or(json!(0)),
            "referenceOnlyAccessRecords": context.get("derived").and_then(|derived| derived.get("referenceOnlyAccessRecords")).cloned().unwrap_or(json!(0)),
            "accessEmpty": access_records.is_empty(),
            "naturalEqualsFinal": natural_equals_final,
            "schemaDriftPresent": !schema_drift.is_null(),
            "unmodelledKindCount": unmodelled.as_object().map(Map::len).unwrap_or(0),
        }
    })
}

const MODEL_MARK_BEGIN: &str = "/*RENDER-MODEL-BEGIN*/";
const MODEL_MARK_END: &str = "/*RENDER-MODEL-END*/";

fn preview_html(model_json: &str) -> String {
    PREVIEW_HTML.replace(
        "__RENDER_MODEL__",
        &format!("{MODEL_MARK_BEGIN}{model_json}{MODEL_MARK_END}"),
    )
}

fn readme(model: &Value) -> String {
    format!(
        "# Saved-scene context preview (read-only)\n\n\
This package displays an audited G24 `engineering-context.json` for one window. \
It is a source-fact viewer only: it builds no EngineeringInput, computes no route, \
grades no terrain and certifies nothing.\n\n\
* preview schema: {}\n* context schema: {}\n* scene: {}\n* window: {},{},{},{}\n\
* world sha256: {}\n* scene sha256: {}\n* context sha256: {}\n\
* coordinates: world metres, x east, z south, y up; heights in millimetres\n\
* surfaces: natural and final are separate layers and are never merged\n\
* access records: source observations, marked sourceObservationOnly / notReverified\n\
* unknown kinds and schema drift are displayed as source facts, not as safe ground\n",
        PREVIEW_SCHEMA,
        model.get("contextSchemaVersion").cloned().unwrap_or(Value::Null),
        model.get("sources").and_then(|sources| sources.get("sceneId")).cloned().unwrap_or(Value::Null),
        model.get("window").and_then(|w| w.get("minX")).cloned().unwrap_or(Value::Null),
        model.get("window").and_then(|w| w.get("minZ")).cloned().unwrap_or(Value::Null),
        model.get("window").and_then(|w| w.get("widthM")).cloned().unwrap_or(Value::Null),
        model.get("window").and_then(|w| w.get("depthM")).cloned().unwrap_or(Value::Null),
        model.get("sources").and_then(|sources| sources.get("world")).and_then(|world| world.get("sha256")).cloned().unwrap_or(Value::Null),
        model.get("sources").and_then(|sources| sources.get("scene")).and_then(|scene| scene.get("sha256")).cloned().unwrap_or(Value::Null),
        model.get("sources").and_then(|sources| sources.get("context")).and_then(|context| context.get("sha256")).cloned().unwrap_or(Value::Null),
    )
}

fn package_manifest(model: &Value, model_sha256: &str) -> Value {
    json!({
        "schemaVersion": PACKAGE_SCHEMA,
        "modelFile": MODEL_FILE,
        "modelSha256": model_sha256,
        "contextSha256": model.get("sources").and_then(|sources| sources.get("context")).and_then(|context| context.get("sha256")),
        "sceneId": model.get("sources").and_then(|sources| sources.get("sceneId")),
        "window": [model["window"]["minX"], model["window"]["minZ"], model["window"]["widthM"], model["window"]["depthM"]],
        "requiredFiles": REQUIRED_FILES,
        "markerFile": MARKER_FILE,
    })
}

fn package_marker(model_sha256: &str, manifest_sha256: &str) -> Value {
    json!({
        "status": "complete",
        "schemaVersion": PACKAGE_SCHEMA,
        "modelFile": MODEL_FILE,
        "modelSha256": model_sha256,
        "manifestFile": MANIFEST_FILE,
        "manifestSha256": manifest_sha256,
    })
}

fn export_preview(args: &[String]) -> CliResult<()> {
    let world = PathBuf::from(required(args, "--world")?);
    let context_dir = PathBuf::from(required(args, "--context")?);
    let out = PathBuf::from(required(args, "--out")?);
    let expected_window = flag(args, "--window");

    let package = load_context_package(&context_dir).map_err(|detail| -> Box<dyn Error> {
        format!("the audited context package is not usable: {detail}").into()
    })?;
    let window = context_window(&package.context).map_err(|detail| -> Box<dyn Error> {
        format!("the context window is unusable: {detail}").into()
    })?;
    if let Some(expected) = &expected_window {
        let declared = format!(
            "{},{},{},{}",
            window.min_x, window.min_z, window.width_m, window.depth_m
        );
        if expected != &declared {
            return Err(format!("--window {expected} is not the context window {declared}").into());
        }
    }
    let rebuilt = rebuild(&world, window, &package.context).map_err(|detail| -> Box<dyn Error> {
        format!("independent rebuild refused: {detail}").into()
    })?;
    let model = build_model(&rebuilt.context, &package, &rebuilt, window);
    let model_json = serde_json::to_string_pretty(&model)?;
    let model_sha256 = sha256_hex(model_json.as_bytes());
    let manifest = package_manifest(&model, &model_sha256);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let marker = package_marker(&model_sha256, &manifest_sha256);

    let mut owned = claim_directory(&out)?;
    let outcome = (|| -> Result<(), String> {
        let artifacts = vec![
            (MODEL_FILE.to_string(), model_json.clone().into_bytes()),
            (VIEWER_FILE.to_string(), VIEWER_JS.as_bytes().to_vec()),
            (HTML_FILE.to_string(), preview_html(&model_json).into_bytes()),
            (README_FILE.to_string(), readme(&model).into_bytes()),
            (MANIFEST_FILE.to_string(), manifest_bytes.clone()),
            (MARKER_FILE.to_string(), serde_json::to_vec_pretty(&marker).unwrap_or_default()),
        ];
        publish(&mut owned, &artifacts, None)
    })();
    match outcome {
        Ok(()) => {
            println!(
                "exported context preview: out={} window={},{},{},{} scene={} entities={} roads={} access={} model={} sha256={}",
                out.display(),
                window.min_x,
                window.min_z,
                window.width_m,
                window.depth_m,
                model["sources"]["sceneId"],
                model["status"]["entityCount"],
                model["status"]["roadCount"],
                model["status"]["accessRecordCount"],
                MODEL_FILE,
                model_sha256
            );
            Ok(())
        }
        Err(detail) => {
            let cleanup = release(owned);
            match cleanup {
                Ok(()) => {
                    Err(format!("export failed ({detail}); created objects were cleaned up").into())
                }
                Err(cleanup_detail) => Err(format!(
                    "export failed ({detail}); cleanup also failed: {cleanup_detail}"
                )
                .into()),
            }
        }
    }
}

/// Extract the model embedded in the viewer HTML, so the audit can prove the
/// page displays exactly the model the package declares.
fn embedded_model(html: &str) -> Result<Value, String> {
    let start = html
        .find(MODEL_MARK_BEGIN)
        .ok_or_else(|| "the viewer HTML embeds no render model".to_string())?;
    let end = html
        .find(MODEL_MARK_END)
        .ok_or_else(|| "the embedded render model is not terminated".to_string())?;
    if end <= start {
        return Err("the embedded render model markers are inverted".to_string());
    }
    let body = &html[start + MODEL_MARK_BEGIN.len()..end];
    serde_json::from_str::<Value>(body).map_err(|error| format!("the embedded model is invalid: {error}"))
}

fn audit_preview(args: &[String]) -> CliResult<()> {
    let world = PathBuf::from(required(args, "--world")?);
    let context_dir = PathBuf::from(required(args, "--context")?);
    let preview_dir = PathBuf::from(required(args, "--preview")?);
    let report_path = PathBuf::from(required(args, "--report")?);
    let expected_window = flag(args, "--window");
    check_report_path(&report_path, &world, &preview_dir)?;

    let mut stages: Vec<Stage> = Vec::new();
    let mut completeness_failures: Vec<String> = Vec::new();
    let mut source_failures: Vec<String> = Vec::new();

    // ---- preview package format -------------------------------------------------
    let preview_root = match fs::canonicalize(&preview_dir) {
        Ok(root) if root.is_dir() => root,
        Ok(root) => {
            completeness_failures.push(format!("preview {} is not a directory", root.display()));
            PathBuf::new()
        }
        Err(error) => {
            completeness_failures.push(format!(
                "preview {} is unusable: {error}",
                preview_dir.display()
            ));
            PathBuf::new()
        }
    };
    let mut model: Option<Value> = None;
    let mut model_bytes: Option<Vec<u8>> = None;
    let mut manifest_bytes: Option<Vec<u8>> = None;
    let mut html: Option<String> = None;
    let mut viewer_bytes = 0u64;
    let mut readme_bytes = 0u64;
    let mut manifest: Option<Value> = None;
    let mut marker: Option<Value> = None;
    if !preview_root.as_os_str().is_empty() {
        let mut budget = 0u64;
        match fs::read_dir(&preview_root) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !REQUIRED_FILES.contains(&name.as_str()) {
                                completeness_failures
                                    .push(format!("undeclared preview file {name} is present"));
                                continue;
                            }
                            match entry.file_type() {
                                Ok(file_type) if file_type.is_file() => {}
                                Ok(_) => completeness_failures
                                    .push(format!("preview entry {name} is not a regular file")),
                                Err(error) => completeness_failures
                                    .push(format!("cannot inspect preview entry {name}: {error}")),
                            }
                        }
                        Err(error) => completeness_failures
                            .push(format!("cannot read a preview directory entry: {error}")),
                    }
                }
            }
            Err(error) => completeness_failures
                .push(format!("the preview directory is unreadable: {error}")),
        }
        for name in REQUIRED_FILES {
            match read_confined(&preview_root, name, &mut budget) {
                Ok(bytes) => match name {
                    MODEL_FILE => {
                        model_bytes = Some(bytes.clone());
                        match parse_object(&bytes, MODEL_FILE) {
                            Ok(value) => model = Some(value),
                            Err(detail) => completeness_failures.push(detail),
                        }
                    }
                    HTML_FILE => match String::from_utf8(bytes) {
                        Ok(text) => html = Some(text),
                        Err(_) => completeness_failures
                            .push("preview.html is not valid UTF-8".to_string()),
                    },
                    VIEWER_FILE => viewer_bytes = bytes.len() as u64,
                    README_FILE => readme_bytes = bytes.len() as u64,
                    MANIFEST_FILE => {
                        manifest_bytes = Some(bytes.clone());
                        match parse_object(&bytes, MANIFEST_FILE) {
                            Ok(value) => manifest = Some(value),
                            Err(detail) => completeness_failures.push(detail),
                        }
                    }
                    MARKER_FILE => match parse_object(&bytes, MARKER_FILE) {
                        Ok(value) => marker = Some(value),
                        Err(detail) => completeness_failures.push(detail),
                    },
                    _ => {}
                },
                Err(detail) => completeness_failures.push(detail),
            }
        }
        if viewer_bytes == 0 {
            completeness_failures.push("the viewer script is empty".to_string());
        }
        if readme_bytes == 0 {
            completeness_failures.push("the preview description is empty".to_string());
        }
        if let (Some(model), Some(manifest), Some(marker)) = (&model, &manifest, &marker) {
            // Hashes are taken over the file bytes, so a re-formatted model
            // still matches its own declaration while edited values do not.
            let model_sha256 = sha256_hex(model_bytes.as_deref().unwrap_or(&[]));
            match string_at(manifest, &["schemaVersion"]).as_deref() {
                Some(PACKAGE_SCHEMA) => {}
                Some(other) => completeness_failures
                    .push(format!("unsupported preview manifest schemaVersion {other}")),
                None => completeness_failures
                    .push("the preview manifest declares no schemaVersion".to_string()),
            }
            if string_at(manifest, &["modelSha256"]).as_deref() != Some(model_sha256.as_str()) {
                completeness_failures
                    .push("the preview manifest does not hash the render model".to_string());
            }
            match manifest.get("requiredFiles").and_then(Value::as_array) {
                Some(array) => {
                    let mut names: Vec<String> = array
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect();
                    names.sort();
                    names.dedup();
                    let mut expected: Vec<String> =
                        REQUIRED_FILES.iter().map(|name| name.to_string()).collect();
                    expected.sort();
                    if names != expected {
                        completeness_failures.push(format!(
                            "the preview manifest must declare exactly {expected:?}"
                        ));
                    }
                }
                None => completeness_failures
                    .push("the preview manifest declares no requiredFiles".to_string()),
            }
            if marker.get("status").and_then(Value::as_str) != Some("complete") {
                completeness_failures.push("the preview marker is not complete".to_string());
            }
            if string_at(marker, &["modelSha256"]).as_deref() != Some(model_sha256.as_str()) {
                completeness_failures
                    .push("the preview marker does not hash the render model".to_string());
            }
            let manifest_sha256 = sha256_hex(manifest_bytes.as_deref().unwrap_or(&[]));
            if string_at(marker, &["manifestSha256"]).as_deref() != Some(manifest_sha256.as_str()) {
                completeness_failures
                    .push("the preview marker does not hash the preview manifest".to_string());
            }
        } else {
            completeness_failures.push("the preview package is incomplete".to_string());
        }
        if let (Some(model), Some(html)) = (&model, &html) {
            for field in MODEL_FIELDS {
                if model.get(field).is_none() {
                    completeness_failures.push(format!("render model field {field} is missing"));
                }
            }
            if let Some(object) = model.as_object() {
                for key in object.keys() {
                    if !MODEL_FIELDS.contains(&key.as_str()) {
                        completeness_failures
                            .push(format!("render model declares an unknown field {key}"));
                    }
                }
            }
            match string_at(model, &["schemaVersion"]).as_deref() {
                Some(PREVIEW_SCHEMA) => {}
                Some(other) => completeness_failures.push(format!(
                    "unsupported render model schemaVersion {other}; supported: {PREVIEW_SCHEMA}"
                )),
                None => completeness_failures
                    .push("the render model declares no schemaVersion".to_string()),
            }
            match embedded_model(html) {
                Ok(embedded) => {
                    if let Some(difference) = first_difference(model, &embedded, "embeddedModel") {
                        completeness_failures.push(format!(
                            "the viewer HTML does not embed the declared render model: {difference}"
                        ));
                    }
                }
                Err(detail) => completeness_failures.push(detail),
            }
        }
    }
    stages.push(if completeness_failures.is_empty() {
        Stage::pass(
            "previewCompleteness",
            format!(
                "{} required preview file(s) present, unique and hash-bound",
                REQUIRED_FILES.len()
            ),
        )
    } else {
        Stage::fail("previewCompleteness", completeness_failures.join("; "))
    });

    // ---- audited context package + independent rebuild --------------------------
    let package = match load_context_package(&context_dir) {
        Ok(package) => Some(package),
        Err(detail) => {
            source_failures.push(format!("the audited context package is not usable: {detail}"));
            None
        }
    };
    let window = package
        .as_ref()
        .and_then(|package| context_window(&package.context).ok());
    if package.is_some() && window.is_none() {
        source_failures.push("the context window is unusable".to_string());
    }
    if let (Some(expected), Some(window)) = (&expected_window, window) {
        let declared = format!(
            "{},{},{},{}",
            window.min_x, window.min_z, window.width_m, window.depth_m
        );
        if expected != &declared {
            source_failures.push(format!(
                "the context window {declared} is not the expected {expected}"
            ));
        }
    }
    let rebuilt = match (package.as_ref(), window) {
        (Some(package), Some(window)) => match rebuild(&world, window, &package.context) {
            Ok(rebuilt) => Some(rebuilt),
            Err(detail) => {
                source_failures.push(detail);
                None
            }
        },
        _ => None,
    };

    // sourceBinding: what the model claims about its own sources.
    if let (Some(model), Some(package), Some(rebuilt)) = (&model, &package, &rebuilt) {
        let expectations: [(&str, String); 5] = [
            (
                "sources.world.sha256",
                rebuilt.world_sha256.clone(),
            ),
            (
                "sources.scene.sha256",
                rebuilt.scene_sha256.clone(),
            ),
            (
                "sources.context.sha256",
                package.context_sha256.clone(),
            ),
            (
                "sources.manifest.sha256",
                package.manifest_sha256.clone(),
            ),
            (
                "sources.marker.sha256",
                package.marker_sha256.clone(),
            ),
        ];
        for (path, expected) in expectations {
            let actual = model
                .get("sources")
                .and_then(|sources| {
                    let mut current = sources;
                    let mut parts = path.split('.').skip(1).peekable();
                    while let Some(part) = parts.next() {
                        current = current.get(part)?;
                    }
                    Some(current)
                })
                .and_then(Value::as_str)
                .unwrap_or_default();
            if actual != expected {
                source_failures.push(format!("{path} does not match the rebuilt source"));
            }
        }
        let model_window = model.get("window");
        let declared = window.expect("window");
        let model_window_matches = model_window
            .map(|value| {
                value.get("minX").and_then(Value::as_i64) == Some(i64::from(declared.min_x))
                    && value.get("minZ").and_then(Value::as_i64) == Some(i64::from(declared.min_z))
                    && value.get("widthM").and_then(Value::as_i64)
                        == Some(i64::from(declared.width_m))
                    && value.get("depthM").and_then(Value::as_i64)
                        == Some(i64::from(declared.depth_m))
            })
            .unwrap_or(false);
        if !model_window_matches {
            source_failures.push("the render model window does not match the context window".to_string());
        }
        let model_scene = string_at(model, &["sources", "sceneId"]);
        let context_scene = string_at(&package.context, &["sceneBinding", "sceneId"]);
        if model_scene != context_scene {
            source_failures.push("the render model scene identity does not match the context".to_string());
        }
        let model_version = string_at(model, &["contextSchemaVersion"]);
        let context_version = string_at(&package.context, &["schemaVersion"]);
        if model_version != context_version {
            source_failures.push("the render model context version does not match the context".to_string());
        }
    }
    // The render model is compared only when it declares the version this audit
    // knows; a closed field set is enforced in the completeness stage, which is
    // where an unsupported model is rejected without partial comparison.
    let model_supported = model
        .as_ref()
        .map(|model| string_at(model, &["schemaVersion"]).as_deref() == Some(PREVIEW_SCHEMA))
        .unwrap_or(false);
    stages.push(if !model_supported && model.is_some() {
        Stage::not_evaluated(
            "sourceBinding",
            "the render model declares an unsupported version, so no schema-dependent comparison ran"
                .to_string(),
        )
    } else if let Some(detail) = source_failures.first() {
        Stage::fail("sourceBinding", source_failures.join("; "))
    } else if model.is_none() || rebuilt.is_none() {
        Stage::not_evaluated(
            "sourceBinding",
            "the render model or the independent rebuild was unavailable".to_string(),
        )
    } else {
        Stage::pass(
            "sourceBinding",
            "world, scene, context, manifest and marker hashes match the rebuilt source".to_string(),
        )
    });

    // ---- terrain and entity/road parity ----------------------------------------
    let fresh_model = match (&rebuilt, &package) {
        (Some(rebuilt), Some(package)) => Some(build_model(
            &rebuilt.context,
            package,
            rebuilt,
            window.expect("window"),
        )),
        _ => None,
    };
    match (&model, &fresh_model) {
        (Some(model), Some(fresh)) if model_supported => {
            stages.push(compare_stage(
                "terrainParity",
                fresh,
                model,
                &["surfaces"],
            ));
            stages.push(compare_stage(
                "entityAndRoadParity",
                fresh,
                model,
                &[
                    "entities",
                    "roads",
                    "planEdgeReferences",
                    "access",
                    "unknownKinds",
                    "unmodelledKinds",
                    "schemaDrift",
                    "droppedUnconsumedKeys",
                    "limitations",
                    "status",
                ],
            ));
        }
        _ => {
            for name in ["terrainParity", "entityAndRoadParity"] {
                stages.push(Stage::not_evaluated(
                    name,
                    "the render model or the rebuilt source was unavailable".to_string(),
                ));
            }
        }
    }

    finish_preview(
        report_path,
        preview_dir,
        world,
        stages,
        &model,
        model_bytes.as_deref().map(sha256_hex),
    )
}

fn finish_preview(
    report_path: PathBuf,
    preview_dir: PathBuf,
    world: PathBuf,
    mut stages: Vec<Stage>,
    model: &Option<Value>,
    model_sha256: Option<String>,
) -> CliResult<()> {
    let order = [
        "sourceBinding",
        "terrainParity",
        "entityAndRoadParity",
        "previewCompleteness",
    ];
    let mut ordered: Vec<Stage> = Vec::new();
    for name in order {
        match stages.iter().position(|stage| stage.name == name) {
            Some(index) => ordered.push(stages.remove(index)),
            None => ordered.push(Stage::not_evaluated(name, "the stage did not run".to_string())),
        }
    }
    let stages = ordered;
    let total_pass = stages.iter().all(|stage| stage.state == "pass");
    let report = json!({
        "schemaVersion": PACKAGE_SCHEMA,
        "sourceRoot": world.display().to_string(),
        "preview": preview_dir.display().to_string(),
        "modelSha256": model_sha256,
        "statusFlags": model.as_ref().and_then(|model| model.get("status")).cloned(),
        "stages": stages
            .iter()
            .map(|stage| json!({"name": stage.name, "state": stage.state, "detail": stage.detail}))
            .collect::<Vec<Value>>(),
        "totalPass": total_pass,
        "evaluatedStages": stages.iter().filter(|stage| stage.state != "notEvaluated").map(|stage| stage.name).collect::<Vec<&str>>(),
        "notEvaluatedStages": stages.iter().filter(|stage| stage.state == "notEvaluated").map(|stage| stage.name).collect::<Vec<&str>>(),
    });
    write_report(&report_path, &report)?;
    for stage in &stages {
        println!("stage {}: {} ({})", stage.name, stage.state, stage.detail);
    }
    println!(
        "preview audit {}: totalPass={} report={}",
        if total_pass { "passed" } else { "failed" },
        total_pass,
        report_path.display()
    );
    if total_pass {
        Ok(())
    } else {
        let failing: Vec<&str> = stages
            .iter()
            .filter(|stage| stage.state != "pass")
            .map(|stage| stage.name)
            .collect();
        Err(format!("preview audit failed at {}", failing.join(", ")).into())
    }
}

const PREVIEW_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Saved-scene context preview (read-only)</title>
<style>
  :root { color-scheme: dark; }
  body { margin: 0; font: 13px/1.4 ui-monospace, Consolas, monospace; background: #14171c; color: #d8dee9; display: flex; }
  #side { width: 340px; padding: 10px 12px; border-right: 1px solid #2b3038; height: 100vh; overflow: auto; }
  #stage { flex: 1; position: relative; }
  canvas { display: block; width: 100%; height: 100vh; }
  h1 { font-size: 14px; margin: 0 0 8px; }
  h2 { font-size: 12px; margin: 12px 0 4px; color: #8fa1b3; text-transform: uppercase; letter-spacing: .04em; }
  label { display: block; margin: 2px 0; }
  .badge { display: inline-block; padding: 1px 6px; border-radius: 8px; background: #2b3038; margin-right: 4px; }
  .warn { color: #f0b849; }
  .err { color: #ff6b6b; }
  .ok { color: #7fd18b; }
  ul { margin: 4px 0; padding-left: 18px; }
  li { margin: 1px 0; }
  .row { display: flex; justify-content: space-between; gap: 8px; }
</style>
</head>
<body>
<div id="side">
  <h1>Saved-scene context preview</h1>
  <div id="binding"></div>
  <h2>Surfaces</h2>
  <label><input type="radio" name="surface" value="final" checked> final / engineered</label>
  <label><input type="radio" name="surface" value="natural"> natural</label>
  <h2>Layers</h2>
  <label><input type="checkbox" id="showEntities" checked> entities</label>
  <label><input type="checkbox" id="showRoads" checked> roads and nodes</label>
  <label><input type="checkbox" id="showAccess" checked> saved access paths (observation only)</label>
  <h2>Status</h2>
  <div id="status"></div>
  <h2>Selected entity</h2>
  <div id="selection">nothing selected</div>
  <h2>Entities in window</h2>
  <div id="entities"></div>
  <h2>Access observations</h2>
  <div id="accessList"></div>
</div>
<div id="stage"><canvas id="view"></canvas></div>
<script type="application/json" id="render-model">__RENDER_MODEL__</script>
<script src="viewer.js"></script>
</body>
</html>
"##;

const VIEWER_JS: &str = r##"(function () {
  "use strict";
  var state = {
    modelLoaded: false,
    errorMessage: null,
    surface: "final",
    surfaceSampleSource: null,
    entityCount: 0,
    roadCount: 0,
    nodeCount: 0,
    accessPathCount: 0,
    accessObservationLabels: 0,
    referenceOnlyAccessCount: 0,
    selectedEntityId: null,
    selectedFacts: null,
    renderedPixelVariety: 0,
    canvasWidth: 0,
    canvasHeight: 0,
    statusFlags: {}
  };
  window.__PREVIEW_STATE__ = state;
  var model = null;
  var selectedId = null;

  function setError(message) {
    state.errorMessage = message;
    var status = document.getElementById("status");
    if (status) { status.innerHTML = '<div class="err">' + message + '</div>'; }
  }

  function text(value) {
    return value === null || value === undefined ? "null" : String(value);
  }

  function bounds() {
    var w = model.window;
    return { minX: w.minX, minZ: w.minZ, maxX: w.minX + w.widthM, maxZ: w.minZ + w.depthM, widthM: w.widthM, depthM: w.depthM };
  }

  function project(wx, wz, b, canvas) {
    var pad = 12;
    var scale = Math.min((canvas.width - pad * 2) / b.widthM, (canvas.height - pad * 2) / b.depthM);
    return { x: pad + (wx - b.minX) * scale, y: pad + (wz - b.minZ) * scale, scale: scale };
  }

  function heightColor(mm, min, max) {
    var t = max > min ? (mm - min) / (max - min) : 0.5;
    var r = Math.round(40 + 150 * t);
    var g = Math.round(90 + 90 * (1 - Math.abs(0.5 - t) * 2));
    var bl = Math.round(190 - 140 * t);
    return "rgb(" + r + "," + g + "," + bl + ")";
  }

  function surfaceStats(surface) {
    var samples = surface.samples || [];
    var min = Infinity, max = -Infinity;
    for (var i = 0; i < samples.length; i++) {
      if (samples[i] < min) { min = samples[i]; }
      if (samples[i] > max) { max = samples[i]; }
    }
    if (!samples.length) { min = 0; max = 1; }
    return { min: min, max: max };
  }

  function drawSurface(ctx, canvas, b) {
    var surface = model.surfaces[state.surface];
    var samples = surface.samples || [];
    var width = surface.widthM || b.widthM;
    var depth = surface.depthM || b.depthM;
    var stats = surfaceStats(surface);
    var cellW = (canvas.width - 24) / width;
    var cellH = (canvas.height - 24) / depth;
    for (var z = 0; z < depth; z++) {
      for (var x = 0; x < width; x++) {
        var index = z * width + x;
        if (index >= samples.length) { continue; }
        ctx.fillStyle = heightColor(samples[index], stats.min, stats.max);
        ctx.fillRect(12 + x * cellW, 12 + z * cellH, Math.ceil(cellW) + 1, Math.ceil(cellH) + 1);
      }
    }
    state.surfaceSampleSource = surface.sourceField;
  }

  function drawEntities(ctx, canvas, b) {
    var entities = model.entities || [];
    ctx.lineWidth = 1;
    for (var i = 0; i < entities.length; i++) {
      var entity = entities[i];
      var outline = entity.outline || {};
      var p1 = project(outline.minX, outline.minZ, b, canvas);
      var p2 = project(outline.maxX, outline.maxZ, b, canvas);
      var isSelected = entity.entityId === selectedId;
      ctx.strokeStyle = isSelected ? "#ffd166" : (entity.kind === "road" ? "#7fb2ff" : "#7fd18b");
      ctx.lineWidth = isSelected ? 2 : 1;
      ctx.strokeRect(p1.x, p1.y, Math.max(2, p2.x - p1.x), Math.max(2, p2.y - p1.y));
      if (isSelected) {
        ctx.fillStyle = "#ffd166";
        ctx.fillText(entity.entityId, p1.x + 2, p1.y - 3);
      }
    }
  }

  function nodeOf(road, key) {
    var node = road[key];
    if (!node || node.worldX === null || node.worldX === undefined) { return null; }
    return { x: node.worldX, z: node.worldZ, id: node.id, referenceOnly: true };
  }

  function drawRoads(ctx, canvas, b) {
    var roads = model.roads || [];
    var nodes = {};
    ctx.lineWidth = 2;
    for (var i = 0; i < roads.length; i++) {
      var road = roads[i];
      var from = nodeOf(road, "fromNodeFact");
      var to = nodeOf(road, "toNodeFact");
      ctx.strokeStyle = "#7fb2ff";
      if (from && to) {
        var p1 = project(from.x, from.z, b, canvas);
        var p2 = project(to.x, to.z, b, canvas);
        ctx.beginPath(); ctx.moveTo(p1.x, p1.y); ctx.lineTo(p2.x, p2.y); ctx.stroke();
        nodes[from.id] = from; nodes[to.id] = to;
      } else {
        var geometry = road.geometry || {};
        var centre = project((geometry.minX + geometry.maxX) / 2, (geometry.minZ + geometry.maxZ) / 2, b, canvas);
        ctx.beginPath(); ctx.arc(centre.x, centre.y, 2, 0, Math.PI * 2); ctx.stroke();
      }
    }
    state.nodeCount = Object.keys(nodes).length;
    for (var key in nodes) {
      var p = project(nodes[key].x, nodes[key].z, b, canvas);
      ctx.fillStyle = "#c9d6e4";
      ctx.beginPath(); ctx.arc(p.x, p.y, 3, 0, Math.PI * 2); ctx.fill();
      ctx.fillText("n" + key, p.x + 4, p.y - 4);
    }
  }

  function drawAccess(ctx, canvas, b) {
    var records = (model.access && model.access.records) || [];
    ctx.lineWidth = 2;
    ctx.setLineDash([5, 4]);
    ctx.strokeStyle = "#f0b849";
    for (var i = 0; i < records.length; i++) {
      var path = records[i].path || [];
      if (!path.length) { continue; }
      ctx.beginPath();
      for (var j = 0; j < path.length; j++) {
        var p = project(path[j].x, path[j].z, b, canvas);
        if (j === 0) { ctx.moveTo(p.x, p.y); } else { ctx.lineTo(p.x, p.y); }
      }
      ctx.stroke();
    }
    ctx.setLineDash([]);
  }

  function pixelVariety(ctx, canvas) {
    var seen = {};
    var count = 0;
    for (var x = 0; x < canvas.width; x += Math.max(1, Math.floor(canvas.width / 20))) {
      for (var y = 0; y < canvas.height; y += Math.max(1, Math.floor(canvas.height / 20))) {
        var data = ctx.getImageData(x, y, 1, 1).data;
        var key = data[0] + "," + data[1] + "," + data[2];
        if (!seen[key]) { seen[key] = true; count++; }
      }
    }
    return count;
  }

  function render() {
    var canvas = document.getElementById("view");
    if (!canvas || !model) { return; }
    var ctx = canvas.getContext("2d");
    canvas.width = canvas.clientWidth || 800;
    canvas.height = canvas.clientHeight || 600;
    state.canvasWidth = canvas.width;
    state.canvasHeight = canvas.height;
    var b = bounds();
    ctx.fillStyle = "#0f1216";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    drawSurface(ctx, canvas, b);
    if (document.getElementById("showAccess").checked) { drawAccess(ctx, canvas, b); }
    if (document.getElementById("showRoads").checked) { drawRoads(ctx, canvas, b); }
    if (document.getElementById("showEntities").checked) { drawEntities(ctx, canvas, b); }
    state.renderedPixelVariety = pixelVariety(ctx, canvas);
  }

  function selectEntity(id) {
    selectedId = id;
    state.selectedEntityId = id;
    var entity = null;
    for (var i = 0; i < (model.entities || []).length; i++) {
      if (model.entities[i].entityId === id) { entity = model.entities[i]; }
    }
    if (entity) {
      state.selectedFacts = {
        entityId: entity.entityId,
        kind: entity.kind,
        assetId: entity.assetId,
        worldX: entity.worldX, worldZ: entity.worldZ, worldY: entity.worldY,
        rotationYDeg: entity.rotationYDeg,
        widthM: entity.widthM, depthM: entity.depthM, heightM: entity.heightM,
        scale: entity.scale,
        savedBounds: entity.savedBounds,
        inclusion: entity.inclusion
      };
      document.getElementById("selection").innerHTML =
        "<div>entityId: " + text(entity.entityId) + "</div>" +
        "<div>kind: " + text(entity.kind) + " / asset: " + text(entity.assetId) + "</div>" +
        "<div>world: (" + text(entity.worldX) + ", " + text(entity.worldZ) + ", " + text(entity.worldY) + ")</div>" +
        "<div>rotation: " + text(entity.rotationYDeg) + " deg, size: " + text(entity.widthM) + " x " + text(entity.depthM) + " x " + text(entity.heightM) + "</div>" +
        "<div>saved bounds: " + JSON.stringify(entity.savedBounds) + "</div>" +
        "<div>source fact only; no engineering identity is created</div>";
    } else {
      state.selectedFacts = null;
      document.getElementById("selection").textContent = "no entity with that identity in this window";
    }
    render();
  }
  window.__selectEntity = selectEntity;
  window.__setSurface = function (value) {
    state.surface = value;
    var radios = document.getElementsByName("surface");
    for (var i = 0; i < radios.length; i++) { radios[i].checked = radios[i].value === value; }
    render();
  };
  window.__clickAt = function (x, y) {
    var canvas = document.getElementById("view");
    var b = bounds();
    var best = null, bestDistance = Infinity;
    for (var i = 0; i < (model.entities || []).length; i++) {
      var entity = model.entities[i];
      var outline = entity.outline || {};
      var p = project((outline.minX + outline.maxX) / 2, (outline.minZ + outline.maxZ) / 2, b, canvas);
      var distance = Math.hypot(p.x - x, p.y - y);
      if (distance < bestDistance) { bestDistance = distance; best = entity; }
    }
    if (best && bestDistance < 40) { selectEntity(best.entityId); }
    return state.selectedEntityId;
  };

  function fillPanels() {
    var status = model.status || {};
    state.statusFlags = {
      schemaDriftPresent: !!status.schemaDriftPresent,
      unmodelledKindCount: status.unmodelledKindCount || 0,
      accessEmpty: !!status.accessEmpty,
      naturalEqualsFinal: !!status.naturalEqualsFinal
    };
    var limitations = model.limitations || [];
    var html = "";
    html += '<div>scene: <span class="badge">' + text(model.sources.sceneId) + '</span></div>';
    html += '<div>window: ' + text(model.window.minX) + ',' + text(model.window.minZ) + ',' + text(model.window.widthM) + ',' + text(model.window.depthM) + '</div>';
    html += '<div>entities: ' + text(status.entityCount) + ' roads: ' + text(status.roadCount) + ' access: ' + text(status.accessRecordCount) + '</div>';
    html += '<div>natural == final: ' + text(status.naturalEqualsFinal) + '</div>';
    html += '<div class="' + (status.schemaDriftPresent ? "warn" : "ok") + '">schema drift: ' + text(model.schemaDrift) + '</div>';
    html += '<div class="' + ((status.unmodelledKindCount || 0) > 0 ? "warn" : "ok") + '">source fact / not converted kinds: ' + text(status.unmodelledKindCount) + '</div>';
    html += '<div class="' + (status.accessEmpty ? "warn" : "ok") + '">access observations: ' + text(status.accessRecordCount) + (status.accessEmpty ? " (none in this context)" : "") + '</div>';
    html += '<ul>' + limitations.map(function (line) { return "<li>" + line + "</li>"; }).join("") + '</ul>';
    document.getElementById("status").innerHTML = html;
    document.getElementById("binding").innerHTML =
      '<div>world sha256: ' + text(model.sources.world.sha256).slice(0, 12) + '</div>' +
      '<div>scene sha256: ' + text(model.sources.scene.sha256).slice(0, 12) + '</div>' +
      '<div>context sha256: ' + text(model.sources.context.sha256).slice(0, 12) + '</div>' +
      '<div>model schema: ' + text(model.schemaVersion) + '</div>';

    var entities = model.entities || [];
    var list = document.getElementById("entities");
    list.innerHTML = entities.map(function (entity) {
      return '<div class="row"><a href="#" data-entity="' + entity.entityId + '">' + entity.entityId + '</a><span>' + text(entity.kind) + '</span></div>';
    }).join("");
    var links = list.getElementsByTagName("a");
    for (var i = 0; i < links.length; i++) {
      links[i].addEventListener("click", function (event) {
        event.preventDefault();
        selectEntity(this.getAttribute("data-entity"));
      });
    }

    var records = (model.access && model.access.records) || [];
    var observationLabels = 0;
    var referenceOnly = 0;
    document.getElementById("accessList").innerHTML = records.map(function (record) {
      if (record.sourceObservationOnly && record.notReverified) { observationLabels++; }
      if (record.referenceOnly) { referenceOnly++; }
      return '<div><span class="badge">sourceObservationOnly</span><span class="badge">notReverified</span> ' +
        text(record.entityId) + ' status=' + text(record.status) + ' connected=' + text(record.connectedInSource) +
        ' pathPoints=' + text((record.path || []).length) + (record.referenceOnly ? ' <span class="warn">referenceOnly</span>' : '') + '</div>';
    }).join("");
    state.entityCount = entities.length;
    state.roadCount = (model.roads || []).length;
    state.accessPathCount = records.length;
    state.accessObservationLabels = observationLabels;
    state.referenceOnlyAccessCount = referenceOnly;
  }

  function boot() {
    var node = document.getElementById("render-model");
    try {
      var raw = node.textContent
        .replace(/\/\*RENDER-MODEL-BEGIN\*\//g, "")
        .replace(/\/\*RENDER-MODEL-END\*\//g, "");
      model = JSON.parse(raw);
      if (!model || typeof model !== "object" || !model.window || !model.surfaces) {
        throw new Error("render model is missing window or surfaces");
      }
      state.modelLoaded = true;
      fillPanels();
      var radios = document.getElementsByName("surface");
      for (var i = 0; i < radios.length; i++) {
        radios[i].addEventListener("change", function () {
          state.surface = this.value;
          render();
        });
      }
      ["showEntities", "showRoads", "showAccess"].forEach(function (id) {
        document.getElementById(id).addEventListener("change", render);
      });
      document.getElementById("view").addEventListener("click", function (event) {
        var rect = this.getBoundingClientRect();
        window.__clickAt(event.clientX - rect.left, event.clientY - rect.top);
      });
      render();
      window.addEventListener("resize", render);
    } catch (error) {
      state.modelLoaded = false;
      model = null;
      setError("preview refused: " + error.message);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("gwc-preview-{}-{}-{}", std::process::id(), tag, nanos));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn fixed_source_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/joint-r27-verified-baseline")
    }

    fn context_package() -> (PathBuf, ContextPackage, WindowRect, Rebuild) {
        let dir = temp_dir("context");
        let window = WindowRect::parse("256,32,64,64").expect("window");
        let bound = BoundScene::read(&fixed_source_dir(), window).expect("source binds");
        let context = build_context(&bound).expect("context builds");
        let context_bytes = serde_json::to_vec_pretty(&context).expect("context bytes");
        let context_sha = sha256_hex(&context_bytes);
        fs::write(dir.join("engineering-context.json"), &context_bytes).expect("context");
        let manifest = json!({"schemaVersion": "engineering-context-audit-v1", "contextSha256": context_sha});
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest");
        let manifest_sha = sha256_hex(&manifest_bytes);
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("manifest");
        let marker = json!({
            "status": "complete",
            "schemaVersion": "engineering-context-audit-v1",
            "contextSha256": context_sha,
            "manifestSha256": manifest_sha,
        });
        fs::write(dir.join("COMPLETE.json"), serde_json::to_vec_pretty(&marker).expect("marker"))
            .expect("marker");
        let package = load_context_package(&dir).expect("package loads");
        let rebuilt = rebuild(&fixed_source_dir(), window, &package.context).expect("rebuild");
        (dir, package, window, rebuilt)
    }

    #[test]
    fn model_keeps_both_surfaces_entities_roads_and_access_observations() {
        let (_dir, package, window, rebuilt) = context_package();
        let model = build_model(&rebuilt.context, &package, &rebuilt, window);
        let natural = model["surfaces"]["natural"]["samples"].as_array().expect("natural");
        let final_samples = model["surfaces"]["final"]["samples"].as_array().expect("final");
        assert_eq!(natural.len(), 4096);
        assert_eq!(final_samples.len(), 4096);
        assert_eq!(model["surfaces"]["natural"]["sourceField"], json!("crop.naturalMm"));
        assert_eq!(model["surfaces"]["final"]["sourceField"], json!("crop.finalMm"));
        assert_ne!(natural[0], final_samples[0], "the two surfaces are separate facts");
        let entities = model["entities"].as_array().expect("entities");
        assert!(!entities.is_empty());
        let named = entities
            .iter()
            .find(|entity| entity["entityId"] == json!("generated.road-network.scene-0.4-11"))
            .expect("the named road entity");
        assert!(named.get("savedBounds").is_some(), "original bounds are kept");
        assert!(named.get("rotationYDeg").is_some());
        assert!(named.get("scale").is_some());
        let roads = model["roads"].as_array().expect("roads");
        let road = roads
            .iter()
            .find(|road| road["roadId"] == json!("generated.road-network.scene-0.4-11"))
            .expect("the named road");
        assert_eq!(road["fromNodeFact"]["id"], json!(4));
        assert_eq!(road["toNodeFact"]["id"], json!(11));
        let records = model["access"]["records"].as_array().expect("access");
        assert_eq!(records.len(), 18);
        assert!(records.iter().all(|record| record["sourceObservationOnly"] == json!(true)
            && record["notReverified"] == json!(true)));
        let with_path = records
            .iter()
            .find(|record| record["path"].as_array().map(|path| path.len() == 3).unwrap_or(false))
            .expect("a saved polyline is kept in full");
        assert_eq!(with_path["path"][0]["x"], json!(276.0));
        assert_eq!(model["status"]["entityCount"], json!(entities.len()));
        assert_eq!(model["status"]["accessEmpty"], json!(false));
    }

    #[test]
    fn model_declares_drift_unknown_kinds_and_closed_fields() {
        let (_dir, package, window, rebuilt) = context_package();
        let model = build_model(&rebuilt.context, &package, &rebuilt, window);
        for field in MODEL_FIELDS {
            assert!(model.get(field).is_some(), "missing field {field}");
        }
        assert!(
            model["unknownKinds"].as_object().map(|map| !map.is_empty()).unwrap_or(false),
            "unmodelled kinds are kept as source facts"
        );
        assert!(model["schemaDrift"].is_string(), "schema drift is disclosed");
        assert!(model["status"]["schemaDriftPresent"].as_bool().unwrap_or(false));
        assert!(!model["limitations"].as_array().expect("limitations").is_empty());
    }

    #[test]
    fn viewer_html_embeds_the_declared_model_and_stays_self_contained() {
        let (_dir, package, window, rebuilt) = context_package();
        let model = build_model(&rebuilt.context, &package, &rebuilt, window);
        let model_json = serde_json::to_string_pretty(&model).expect("json");
        let html = preview_html(&model_json);
        assert!(html.contains("viewer.js"), "the viewer is a separate asset");
        assert!(html.contains("sourceObservationOnly") || VIEWER_JS.contains("sourceObservationOnly"));
        assert!(!html.contains("fetch("), "the page must not need a server");
        let embedded = embedded_model(&html).expect("embedded model");
        assert_eq!(embedded, model);
        assert!(embedded_model("<html></html>").is_err());
    }

    #[test]
    fn context_package_requires_the_declaration_chain_and_version() {
        let (dir, _package, _window, _rebuilt) = context_package();
        let mut broken = dir.clone();
        broken.push("engineering-context.json");
        let original = fs::read(&broken).expect("read");
        let mut value: Value = serde_json::from_slice(&original).expect("parse");
        value["schemaVersion"] = json!("engineering-context-v999");
        fs::write(&broken, serde_json::to_vec_pretty(&value).expect("json")).expect("write");
        assert!(load_context_package(&dir).is_err(), "unknown version must be refused");

        fs::write(&broken, &original).expect("restore");
        let marker = dir.join("COMPLETE.json");
        fs::write(&marker, br#"{"status":"complete","schemaVersion":"x","contextSha256":"00","manifestSha256":"00"}"#)
            .expect("marker");
        assert!(load_context_package(&dir).is_err(), "a broken hash chain must be refused");
    }

    #[test]
    fn tampering_with_the_model_is_detected_by_the_matching_stage() {
        let (_dir, package, window, rebuilt) = context_package();
        let model = build_model(&rebuilt.context, &package, &rebuilt, window);
        let fresh = build_model(&rebuilt.context, &package, &rebuilt, window);

        let mut surface = model.clone();
        surface["surfaces"]["natural"]["samples"][0] = json!(987654);
        assert_eq!(
            compare_stage("terrainParity", &fresh, &surface, &["surfaces"]).state,
            "fail"
        );

        let mut removed = model.clone();
        removed["entities"] = json!([]);
        assert_eq!(
            compare_stage(
                "entityAndRoadParity",
                &fresh,
                &removed,
                &["entities", "roads", "planEdgeReferences", "access"]
            )
            .state,
            "fail"
        );

        let mut path = model.clone();
        path["access"]["records"][0]["path"] = json!([]);
        assert_eq!(
            compare_stage("entityAndRoadParity", &fresh, &path, &["access"]).state,
            "fail"
        );

        let mut verified = model.clone();
        verified["access"]["records"][0]["sourceObservationOnly"] = json!(false);
        assert_eq!(
            compare_stage("entityAndRoadParity", &fresh, &verified, &["access"]).state,
            "fail"
        );

        let mut limitations = model.clone();
        limitations["limitations"] = json!([]);
        assert_eq!(
            compare_stage("entityAndRoadParity", &fresh, &limitations, &["limitations"]).state,
            "fail"
        );

        let mut sources = model.clone();
        sources["sources"]["context"]["sha256"] = json!("00");
        assert_ne!(sources["sources"]["context"]["sha256"], model["sources"]["context"]["sha256"]);
    }

    #[test]
    fn final_binary_exports_and_audits_a_package_end_to_end() {
        let (context_dir, _package, _window, _rebuilt) = context_package();
        let out = temp_dir("e2e").join("package");
        let report = temp_dir("e2e-report").join("report.json");
        let world = fixed_source_dir().display().to_string();
        let context = context_dir.display().to_string();
        let export_args: Vec<String> = vec![
            "--world".into(),
            world.clone(),
            "--context".into(),
            context.clone(),
            "--out".into(),
            out.display().to_string(),
            "--window".into(),
            "256,32,64,64".into(),
        ];
        export_preview(&export_args).expect("the export must succeed");
        for name in REQUIRED_FILES {
            assert!(out.join(name).is_file(), "{name} must be written");
        }
        let audit_args: Vec<String> = vec![
            "--world".into(),
            world,
            "--context".into(),
            context,
            "--preview".into(),
            out.display().to_string(),
            "--report".into(),
            report.display().to_string(),
            "--window".into(),
            "256,32,64,64".into(),
        ];
        audit_preview(&audit_args).expect("the four-stage audit must pass");
        let report_value: Value =
            serde_json::from_slice(&fs::read(&report).expect("report")).expect("report json");
        assert_eq!(report_value["totalPass"], json!(true));
        let stages = report_value["stages"].as_array().expect("stages");
        let names: Vec<&str> = stages
            .iter()
            .map(|stage| stage["name"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(
            names,
            vec![
                "sourceBinding",
                "terrainParity",
                "entityAndRoadParity",
                "previewCompleteness"
            ]
        );
        assert!(stages.iter().all(|stage| stage["state"] == json!("pass")));
        assert!(report_value["notEvaluatedStages"]
            .as_array()
            .expect("notEvaluated")
            .is_empty());
    }

    #[test]
    fn rebuild_refuses_a_context_that_no_longer_matches_the_source() {
        let (_dir, mut package, window, _rebuilt) = context_package();
        package.context["entities"] = json!([]);
        assert!(
            rebuild(&fixed_source_dir(), window, &package.context).is_err(),
            "an edited context must not pass the independent rebuild"
        );
    }
}
