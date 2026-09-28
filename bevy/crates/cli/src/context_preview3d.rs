//! `context-preview3d`: versioned real WebGL2 consumer of an audited saved-scene
//! context preview package.
//!
//! Contract in force (G27B approval): schema `engineering-context-preview3d-v1`
//! is an independent 3D model version beside `engineering-context-preview-v1`; it
//! reuses the audited `sources` binding and the four audit stages, never rewrites
//! or renames the v1 package, and refuses unknown versions. World coordinates
//! are metres with X east, Y up, Z south and the window origin from G24/G25.
//! Height FACTS stay the saved integer millimetres; metres exist only as a
//! lossless display conversion. natural and final stay two independent layers
//! whose vertices come sample by sample from `crop.naturalMm` / `crop.finalMm`
//! (64x64 samples -> 63x63 quads, 2 triangles per cell). No height is padded, no
//! interpolation and no fallback surface is ever used. Entities and roads are
//! built as real oriented solids from their saved world position, their saved
//! size, `rotationYDeg` and their saved pivot/grounding; `savedBounds` stays a
//! world-AABB fact, is never rotated a second time and baked sizes are never
//! scaled a second time. Nodes and access records that carry only x/z are kept in
//! an explicit observation layer with no 3D primitive and no constant world Y;
//! nothing is snapped to a surface. Access records keep `sourceObservationOnly` /
//! `notReverified` forever, and `connected` / `walkable_route` are never promoted
//! to a verified route. Picking is real indexed-triangle intersection
//! (Moller-Trumbore) against the drawn geometry, never nearest-sample plus step.

use super::engineering_context::{check_report_path, claim_directory, compare_stage, publish, release, write_report, Stage};
use glyphweave_core::engineering_context::resolve_within_root;
use glyphweave_core::engineering_preview::sha256_hex;
use serde_json::{json, Value};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

type CliResult<T> = Result<T, Box<dyn Error>>;

const SCHEMA3D: &str = "engineering-context-preview3d-v1";
const SCHEMA3D_AUDIT: &str = "context-preview3d-audit-v1";
const CONTEXT_SCHEMA: &str = "engineering-context-preview-v1";
const MODEL_FILE: &str = "render-model-3d.json";
const VIEWER_FILE: &str = "viewer3d.js";
const HTML_FILE: &str = "preview3d.html";
const README_FILE: &str = "README.md";
const MANIFEST_FILE: &str = "manifest.json";
const MARKER_FILE: &str = "COMPLETE.json";
const REQUIRED_FILES: [&str; 6] = [MODEL_FILE, VIEWER_FILE, HTML_FILE, README_FILE, MANIFEST_FILE, MARKER_FILE];
/// Required files of the audited v1 (`engineering-context-preview-v1`) package.
const V1_REQUIRED_FILES: [&str; 6] =
    ["render-model.json", "viewer.js", "preview.html", "README.md", "manifest.json", "COMPLETE.json"];
const V1_MODEL_FILE: &str = "render-model.json";
const V1_MANIFEST_FILE: &str = "manifest.json";
const V1_MARKER_FILE: &str = "COMPLETE.json";
const AUDIT_SCHEMA: &str = "context-preview-audit-v1";
/// Closed field set of the 3D render model (`grid` and `heightDisplayConversion`
/// are members, never tolerated extras).
const MODEL_FIELDS: [&str; 17] = [
    "schemaVersion", "contextSchemaVersion", "sources", "coordinateSystem", "heightFactUnit", "window",
    "surfaces", "entities", "roads", "nodes", "access", "status", "unknownKinds", "schemaDrift",
    "limitations", "grid", "heightDisplayConversion",
];
/// Closed root field set of the input v1 render model. A v1 model that declares
/// anything else is not this version and is refused instead of partially walked.
const V1_MODEL_FIELDS: [&str; 17] = [
    "schemaVersion", "contextSchemaVersion", "sources", "window", "coordinateSystem", "heightUnit",
    "surfaces", "entities", "roads", "planEdgeReferences", "access", "unknownKinds", "unmodelledKinds",
    "schemaDrift", "droppedUnconsumedKeys", "limitations", "status",
];
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// An optional audit trace is read through a bounded, ordinary-file-only path.
const MAX_TRACE_BYTES: u64 = 8 * 1024 * 1024;

pub(super) fn run(args: &[String], audit: bool) -> CliResult<()> {
    if audit { audit_3d(args) } else { export_3d(args) }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == name { return args.get(index + 1).cloned(); }
        index += 1;
    }
    None
}
fn required(args: &[String], name: &str) -> CliResult<String> {
    flag(args, name).ok_or_else(|| format!("missing required flag {name}").into())
}

fn read_confined(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let resolved = resolve_within_root(root, relative)
        .map_err(|error| format!("{relative} is not a confined path: {error}"))?;
    let metadata = fs::metadata(&resolved).map_err(|e| format!("{relative} is unreadable: {e}"))?;
    if !metadata.is_file() { return Err(format!("{relative} is not a regular file")); }
    if metadata.len() > MAX_FILE_BYTES { return Err(format!("{relative} exceeds the per-file cap")); }
    fs::read(&resolved).map_err(|e| format!("{relative} is unreadable: {e}"))
}
fn object(bytes: &[u8], name: &str) -> Result<Value, String> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(_) => Err(format!("{name} is not a JSON object")),
        Err(e) => Err(format!("{name} is not readable JSON: {e}")),
    }
}
fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path { current = current.get(*key)?; }
    current.as_str().map(str::to_string)
}
fn number(value: &Value, key: &str) -> i64 { value.get(key).and_then(Value::as_i64).unwrap_or(0) }

/// The audited v1 package plus the rebuilt source chain. The external G25 audit
/// report is only an optional trace: it is never the trust root.
struct Inputs {
    v1: Value,
    v1_sha256: String,
    v1_manifest_sha256: String,
    v1_marker_sha256: String,
    audit_sha256: Option<String>,
}

/// Reads an optional trace through a bounded, ordinary-file-only path: the size
/// is taken from metadata BEFORE any bytes are read, so an oversized or linked
/// report is refused without being pulled into memory first.
fn read_bounded_regular(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, String> {
    let link = fs::symlink_metadata(path).map_err(|e| format!("{what} is unreadable: {e}"))?;
    if link.file_type().is_symlink() {
        return Err(format!("{what} is a symbolic link, not a regular file"));
    }
    let metadata = fs::metadata(path).map_err(|e| format!("{what} is unreadable: {e}"))?;
    if !metadata.is_file() {
        return Err(format!("{what} is not a regular file"));
    }
    if metadata.len() > cap {
        return Err(format!(
            "{what} is {} bytes, over the {cap} byte read cap",
            metadata.len()
        ));
    }
    fs::read(path).map_err(|e| format!("{what} is unreadable: {e}"))
}

/// The optional `--context-audit` trace must satisfy the real production report
/// contract (`context-preview-audit-v1`): a known version, a passing summary, the
/// four formal stages exactly once each and all passing, and `evaluatedStages` /
/// `notEvaluatedStages` that are present, correctly typed and consistent with
/// `stages`. Missing, duplicated, unknown, mistyped and contradictory summaries
/// are all refused; a trace is never the trust root.
fn check_trace(path: &Path, v1_sha256: &str) -> Result<String, String> {
    const FORMAL: [&str; 4] = ["sourceBinding", "terrainParity", "entityAndRoadParity", "previewCompleteness"];
    let bytes = read_bounded_regular(path, MAX_TRACE_BYTES, "context audit trace")?;
    let report = object(&bytes, "context audit trace")?;
    match string_at(&report, &["schemaVersion"]).as_deref() {
        Some(AUDIT_SCHEMA) => {}
        Some(other) => {
            return Err(format!(
                "unsupported context audit trace schemaVersion {other}; expected {AUDIT_SCHEMA}"
            ))
        }
        None => return Err("the context audit trace declares no schemaVersion".to_string()),
    }
    match report.get("totalPass") {
        Some(Value::Bool(true)) => {}
        Some(Value::Bool(false)) => return Err("the context audit trace does not summarise a pass".to_string()),
        _ => return Err("the context audit trace has no boolean totalPass".to_string()),
    }
    if string_at(&report, &["modelSha256"]).as_deref() != Some(v1_sha256) {
        return Err("the context audit trace binds a different render model".to_string());
    }
    let Some(stages) = report.get("stages").and_then(Value::as_array) else {
        return Err("the context audit trace carries no stage contract; totalPass alone is not accepted".to_string());
    };
    if stages.len() != FORMAL.len() {
        return Err(format!(
            "the context audit trace must carry exactly {} stages, found {}",
            FORMAL.len(),
            stages.len()
        ));
    }
    for stage in stages {
        let name = match stage.get("name").and_then(Value::as_str) {
            Some(name) => name,
            None => return Err("a context audit trace stage has no string name".to_string()),
        };
        if !FORMAL.contains(&name) {
            return Err(format!("the context audit trace declares an unknown stage {name}"));
        }
        if stage.get("detail").and_then(Value::as_str).is_none() {
            return Err(format!("the context audit trace stage {name} has no detail"));
        }
        match stage.get("state").and_then(Value::as_str) {
            Some("pass") => {}
            Some(other) => {
                return Err(format!("the context audit trace stage {name} is {other}, not a pass"))
            }
            None => return Err(format!("the context audit trace stage {name} has no string state")),
        }
    }
    for name in FORMAL {
        let count = stages
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some(name))
            .count();
        if count != 1 {
            return Err(format!("the context audit trace declares the stage {name} {count} times"));
        }
    }
    let Some(not_evaluated) = report.get("notEvaluatedStages").and_then(Value::as_array) else {
        return Err("the context audit trace has no notEvaluatedStages array".to_string());
    };
    if !not_evaluated.is_empty() {
        return Err("the context audit trace still declares notEvaluated stages".to_string());
    }
    let Some(evaluated) = report.get("evaluatedStages").and_then(Value::as_array) else {
        return Err("the context audit trace has no evaluatedStages array".to_string());
    };
    let mut evaluated_names: Vec<String> = Vec::new();
    for entry in evaluated {
        match entry.as_str() {
            Some(name) => evaluated_names.push(name.to_string()),
            None => return Err("evaluatedStages must be an array of stage names".to_string()),
        }
    }
    if evaluated_names.len() != FORMAL.len() {
        return Err(format!(
            "evaluatedStages must name exactly {} stages, found {}",
            FORMAL.len(),
            evaluated_names.len()
        ));
    }
    for name in FORMAL {
        if evaluated_names.iter().filter(|n| n.as_str() == name).count() != 1 {
            return Err(format!("evaluatedStages does not name the stage {name} exactly once"));
        }
    }
    let stage_names: Vec<String> = stages
        .iter()
        .filter_map(|s| s.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    if stage_names != evaluated_names {
        return Err("evaluatedStages contradicts the stage list".to_string());
    }
    Ok(sha256_hex(&bytes))
}

/// Independent source re-check: the caller-given world root and G24 context
/// package are rebuilt with the audited G25 rebuild function, the supplied v1
/// preview model must equal the model that rebuild produces, and only then may a
/// 3D package be produced or passed.
fn load_inputs(
    world: &Path,
    context_pkg: &Path,
    preview: &Path,
    audit_trace: Option<&Path>,
) -> Result<Inputs, String> {
    let package = super::context_preview::load_context_package(context_pkg)?;
    let window = super::context_preview::context_window(&package.context)?;
    let rebuilt = super::context_preview::rebuild(world, window, &package.context)?;
    let fresh = super::context_preview::build_model(&rebuilt.context, &package, &rebuilt, window);
    let (supplied, v1_sha256, v1_manifest_sha256, v1_marker_sha256) = read_v1_package(preview)?;
    let comparison = compare_stage(
        "independentSourceRebuild",
        &fresh,
        &supplied,
        // The v1 field set (nodes are derived in 3D and do not exist there).
        &[
            "schemaVersion", "contextSchemaVersion", "sources", "window", "coordinateSystem",
            "heightUnit", "surfaces", "entities", "roads", "planEdgeReferences", "access",
            "unknownKinds", "unmodelledKinds", "schemaDrift", "droppedUnconsumedKeys",
            "limitations", "status",
        ],
    );
    if comparison.state != "pass" {
        return Err(format!(
            "the supplied v1 model does not match an independent rebuild from the given source: {}",
            comparison.detail
        ));
    }
    let audit_sha256 = match audit_trace {
        None => None,
        Some(path) => Some(check_trace(path, &v1_sha256)?),
    };
    Ok(Inputs { v1: supplied, v1_sha256, v1_manifest_sha256, v1_marker_sha256, audit_sha256 })
}

/// Reads the supplied v1 preview package under the confinement and hash-chain
/// rules of the audited G25 package, and binds every declared v1 source fact:
/// the closed file set, the whole marker/manifest chain, the model's complete
/// root field set and the manifest's own scene/context/window declarations.
fn read_v1_package(preview: &Path) -> Result<(Value, String, String, String), String> {
    let root = fs::canonicalize(preview).map_err(|e| format!("preview {} is unusable: {e}", preview.display()))?;
    if !root.is_dir() { return Err("the preview package is not a directory".to_string()); }
    // Closed file set: no undeclared file may stand in the package.
    let mut present: Vec<String> = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| format!("preview is unreadable: {e}"))? {
        let entry = entry.map_err(|e| format!("cannot read a preview entry: {e}"))?;
        present.push(entry.file_name().to_string_lossy().to_string());
    }
    // The v1 package must carry its own six files and nothing else (the 3D file
    // names are only used for the 3D output package).
    for name in V1_REQUIRED_FILES {
        if !present.iter().any(|p| p == name) { return Err(format!("required v1 file {name} is missing")); }
    }
    for name in &present {
        if !V1_REQUIRED_FILES.contains(&name.as_str()) {
            return Err(format!("undeclared v1 file {name} is present"));
        }
    }
    let model_bytes = read_confined(&root, V1_MODEL_FILE)?;
    let manifest_bytes = read_confined(&root, V1_MANIFEST_FILE)?;
    let marker_bytes = read_confined(&root, V1_MARKER_FILE)?;
    let v1 = object(&model_bytes, V1_MODEL_FILE)?;
    let manifest = object(&manifest_bytes, V1_MANIFEST_FILE)?;
    let marker = object(&marker_bytes, V1_MARKER_FILE)?;
    let v1_sha256 = sha256_hex(&model_bytes);
    let v1_manifest_sha256 = sha256_hex(&manifest_bytes);
    let v1_marker_sha256 = sha256_hex(&marker_bytes);
    match string_at(&marker, &["schemaVersion"]).as_deref() {
        Some(AUDIT_SCHEMA) => {}
        Some(other) => return Err(format!("unsupported v1 marker schemaVersion {other}")),
        None => return Err("the v1 completion marker declares no schemaVersion".to_string()),
    }
    if marker.get("status").and_then(Value::as_str) != Some("complete") {
        return Err("the v1 completion marker is not complete".to_string());
    }
    if string_at(&marker, &["modelFile"]).as_deref() != Some(V1_MODEL_FILE) {
        return Err("the v1 marker does not name its render model".to_string());
    }
    if string_at(&marker, &["manifestFile"]).as_deref() != Some(V1_MANIFEST_FILE) {
        return Err("the v1 marker does not name its manifest".to_string());
    }
    if string_at(&marker, &["modelSha256"]).as_deref() != Some(v1_sha256.as_str()) {
        return Err("the v1 marker does not hash its render model".to_string());
    }
    if string_at(&marker, &["manifestSha256"]).as_deref() != Some(v1_manifest_sha256.as_str()) {
        return Err("the v1 marker does not hash its manifest".to_string());
    }
    match string_at(&manifest, &["schemaVersion"]).as_deref() {
        Some(AUDIT_SCHEMA) => {}
        Some(other) => return Err(format!("unsupported v1 manifest schemaVersion {other}")),
        None => return Err("the v1 manifest declares no schemaVersion".to_string()),
    }
    if string_at(&manifest, &["modelFile"]).as_deref() != Some(V1_MODEL_FILE) {
        return Err("the v1 manifest does not name its render model".to_string());
    }
    if string_at(&manifest, &["markerFile"]).as_deref() != Some(V1_MARKER_FILE) {
        return Err("the v1 manifest does not name its completion marker".to_string());
    }
    if string_at(&manifest, &["modelSha256"]).as_deref() != Some(v1_sha256.as_str()) {
        return Err("the v1 manifest does not hash its render model".to_string());
    }
    let Some(declared) = manifest.get("requiredFiles").and_then(Value::as_array) else {
        return Err("the v1 manifest declares no requiredFiles".to_string());
    };
    let mut declared_names: Vec<String> = Vec::new();
    for entry in declared {
        match entry.as_str() {
            Some(name) => declared_names.push(name.to_string()),
            None => return Err("a v1 requiredFiles entry is not a string".to_string()),
        }
    }
    let mut sorted_declared = declared_names.clone();
    sorted_declared.sort();
    let mut expected: Vec<String> = V1_REQUIRED_FILES.iter().map(|n| n.to_string()).collect();
    expected.sort();
    if sorted_declared != expected {
        return Err(format!("the v1 manifest must declare exactly {expected:?}"));
    }
    match string_at(&v1, &["schemaVersion"]).as_deref() {
        Some(CONTEXT_SCHEMA) => {}
        Some(other) => return Err(format!("unsupported v1 schemaVersion {other}")),
        None => return Err("the v1 model declares no schemaVersion".to_string()),
    }
    // The input v1 model is checked against its complete root field set: a model
    // that carries extra root declarations is not this version.
    for field in V1_MODEL_FIELDS {
        if v1.get(field).is_none() { return Err(format!("the v1 model field {field} is missing")); }
    }
    if let Some(map) = v1.as_object() {
        for key in map.keys() {
            if !V1_MODEL_FIELDS.contains(&key.as_str()) {
                return Err(format!("the v1 model declares an unknown root field {key}"));
            }
        }
    }
    if string_at(&manifest, &["sceneId"]) != string_at(&v1, &["sources", "sceneId"]) {
        return Err("the v1 manifest sceneId contradicts the v1 model sources".to_string());
    }
    if string_at(&manifest, &["contextSha256"]) != string_at(&v1, &["sources", "context", "sha256"]) {
        return Err("the v1 manifest context hash contradicts the v1 model sources".to_string());
    }
    let Some(window) = manifest.get("window").and_then(Value::as_array) else {
        return Err("the v1 manifest declares no window".to_string());
    };
    let declared_window: Vec<i64> = window.iter().filter_map(Value::as_i64).collect();
    let model_window = [
        number(v1.get("window").unwrap_or(&Value::Null), "minX"),
        number(v1.get("window").unwrap_or(&Value::Null), "minZ"),
        number(v1.get("window").unwrap_or(&Value::Null), "widthM"),
        number(v1.get("window").unwrap_or(&Value::Null), "depthM"),
    ];
    if declared_window.len() != 4 || declared_window.as_slice() != model_window.as_slice() {
        return Err("the v1 manifest window contradicts the v1 model window".to_string());
    }
    // Read-only rebinding is performed by the independent rebuild in the caller;
    // here the supplied package's own chain and closed file set are enforced.
    Ok((v1, v1_sha256, v1_manifest_sha256, v1_marker_sha256))
}

// ---------------------------------------------------------------------------
// Pure geometry (the model builder and the WebGL2 viewer use the same maths)
// ---------------------------------------------------------------------------

/// Real 3D geometry for the consumer. Every function is pure and none of them
/// writes back: derived metre coordinates are display geometry only and never
/// replace a saved integer-millimetre sample.
#[allow(dead_code)]
mod geom {
    /// Corner order: `index = (xMax << 2) | (yMax << 1) | zMax`.
    pub(super) const BOX_TRIANGLES: [u32; 36] = [
        0, 1, 5, 0, 5, 4, // -Y
        2, 6, 7, 2, 7, 3, // +Y
        0, 4, 6, 0, 6, 2, // -Z
        1, 3, 7, 1, 7, 5, // +Z
        0, 2, 3, 0, 3, 1, // -X
        4, 5, 7, 4, 7, 6, // +X
    ];

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
    fn finite(point: [f64; 3]) -> bool { point.iter().all(|value| value.is_finite()) }

    /// The authoritative transform, copied character by character from the
    /// approved contract:
    /// `worldX = world_x + localX*cos + localZ*sin`;
    /// `worldZ = world_z - localX*sin + localZ*cos`.
    pub(super) fn transform_point(
        world_x: f64, world_z: f64, rotation_y_deg: f64, local_x: f64, local_z: f64,
    ) -> [f64; 2] {
        let (sin, cos) = rotation_y_deg.to_radians().sin_cos();
        [world_x + local_x * cos + local_z * sin, world_z - local_x * sin + local_z * cos]
    }

    /// Vertical span from the saved height, the saved anchor and the saved pivot.
    /// An unrecognised or absent pivot is refused instead of being inferred.
    pub(super) fn vertical_span(
        world_y: f64, height_m: f64, pivot: &str, bottom_offset_m: f64,
    ) -> Option<(f64, f64)> {
        if !world_y.is_finite() || !height_m.is_finite() || height_m <= 0.0 { return None; }
        match pivot {
            "bottom" => { let base = world_y + bottom_offset_m; Some((base, base + height_m)) }
            "centre" | "center" => Some((world_y - height_m / 2.0, world_y + height_m / 2.0)),
            "top" => Some((world_y - height_m, world_y)),
            _ => None,
        }
    }

    /// Orientated solid from saved facts. Local X spans the saved width, local Z
    /// the saved depth, both centred on the saved world position. The baked size
    /// is never scaled a second time and `rotationYDeg` is applied exactly once.
    pub(super) fn oriented_box(
        world_x: f64, world_y: f64, world_z: f64, width_m: f64, depth_m: f64, height_m: f64,
        rotation_y_deg: f64, pivot: &str, bottom_offset_m: f64,
    ) -> Option<([[f64; 3]; 8], f64, f64)> {
        if !world_x.is_finite() || !world_z.is_finite() || !rotation_y_deg.is_finite() { return None; }
        if !width_m.is_finite() || !depth_m.is_finite() || width_m <= 0.0 || depth_m <= 0.0 { return None; }
        let (min_y, max_y) = vertical_span(world_y, height_m, pivot, bottom_offset_m)?;
        let half_x = width_m / 2.0;
        let half_z = depth_m / 2.0;
        let mut corners = [[0.0_f64; 3]; 8];
        for (index, corner) in corners.iter_mut().enumerate() {
            let sign_x = if index & 4 != 0 { 1.0 } else { -1.0 };
            let sign_z = if index & 1 != 0 { 1.0 } else { -1.0 };
            let [x, z] = transform_point(world_x, world_z, rotation_y_deg, sign_x * half_x, sign_z * half_z);
            *corner = [x, if index & 2 != 0 { max_y } else { min_y }, z];
        }
        if !corners.iter().all(|corner| finite(*corner)) { return None; }
        Some((corners, min_y, max_y))
    }

    /// World-AABB proxy: the saved world AABB stays axis aligned and is never
    /// rotated a second time.
    pub(super) fn aabb_box(
        min_x: f64, min_z: f64, max_x: f64, max_z: f64, min_y: f64, max_y: f64,
    ) -> Option<[[f64; 3]; 8]> {
        if !(min_x.is_finite() && min_z.is_finite() && max_x.is_finite() && max_z.is_finite()
            && min_y.is_finite() && max_y.is_finite())
        {
            return None;
        }
        let xs = [min_x, max_x];
        let ys = [min_y, max_y];
        let zs = [min_z, max_z];
        let mut corners = [[0.0_f64; 3]; 8];
        for (index, corner) in corners.iter_mut().enumerate() {
            *corner = [xs[(index >> 2) & 1], ys[(index >> 1) & 1], zs[index & 1]];
        }
        Some(corners)
    }

    /// `[min_x, min_z, max_x, max_z, min_y, max_y]` of a corner set.
    pub(super) fn corners_aabb(corners: &[[f64; 3]]) -> Option<[f64; 6]> {
        if corners.is_empty() { return None; }
        let mut aabb = [
            f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY,
            f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY,
        ];
        for corner in corners {
            if !finite(*corner) { return None; }
            aabb[0] = aabb[0].min(corner[0]);
            aabb[1] = aabb[1].min(corner[2]);
            aabb[2] = aabb[2].max(corner[0]);
            aabb[3] = aabb[3].max(corner[2]);
            aabb[4] = aabb[4].min(corner[1]);
            aabb[5] = aabb[5].max(corner[1]);
        }
        Some(aabb)
    }

    /// Expands indexed triangles. Out-of-range indices and non-triples are
    /// dropped, so a malformed topology can never be hit.
    pub(super) fn triangles_of(
        corners: &[[f64; 3]], indices: &[u32],
    ) -> Vec<([f64; 3], [f64; 3], [f64; 3])> {
        let mut out = Vec::new();
        if indices.len() % 3 != 0 { return out; }
        for triangle in indices.chunks(3) {
            let mut points = [[0.0_f64; 3]; 3];
            let mut complete = true;
            for (slot, index) in triangle.iter().enumerate() {
                match corners.get(*index as usize) {
                    Some(corner) => points[slot] = *corner,
                    None => complete = false,
                }
            }
            if complete { out.push((points[0], points[1], points[2])); }
        }
        out
    }

    /// Moller-Trumbore. `None` for a degenerate (zero-area) triangle, a parallel
    /// ray or a hit at or behind the origin.
    pub(super) fn ray_triangle(
        origin: [f64; 3], dir: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3],
    ) -> Option<f64> {
        if !finite(origin) || !finite(dir) || !finite(a) || !finite(b) || !finite(c) { return None; }
        let edge1 = sub(b, a);
        let edge2 = sub(c, a);
        let pvec = cross(dir, edge2);
        let det = dot(edge1, pvec);
        if det.abs() < 1e-12 { return None; }
        let inv_det = 1.0 / det;
        let tvec = sub(origin, a);
        let u = dot(tvec, pvec) * inv_det;
        if !(-1e-9..=1.0 + 1e-9).contains(&u) { return None; }
        let qvec = cross(tvec, edge1);
        let v = dot(dir, qvec) * inv_det;
        if v < -1e-9 || u + v > 1.0 + 1e-9 { return None; }
        let t = dot(edge2, qvec) * inv_det;
        if t > 1e-9 && t.is_finite() { Some(t) } else { None }
    }

    /// Barycentric weights of `point` on `(a, b, c)`, in `(a, b, c)` order.
    pub(super) fn barycentric(
        point: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3],
    ) -> [f64; 3] {
        let normal = cross(sub(b, a), sub(c, a));
        let area2 = dot(normal, normal);
        if area2 <= 0.0 { return [f64::NAN; 3]; }
        let wa = dot(cross(sub(b, point), sub(c, point)), normal) / area2;
        let wb = dot(cross(sub(c, point), sub(a, point)), normal) / area2;
        [wa, wb, 1.0 - wa - wb]
    }

    fn vertex_at(vertices: &[f64], index: usize) -> Option<[f64; 3]> {
        let base = index.checked_mul(3)?;
        let point = [*vertices.get(base)?, *vertices.get(base + 1)?, *vertices.get(base + 2)?];
        if finite(point) { Some(point) } else { None }
    }

    /// One terrain triangle of the drawn grid, with the viewer's `indicesFor`
    /// topology: cell `(x, z)` produces `(a, c, b)` then `(b, c, d)`.
    /// Out-of-range indices and out-of-range triangle numbers yield `None`.
    pub(super) fn terrain_triangle(
        vertices: &[f64], width: i64, depth: i64, triangle_index: usize,
    ) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        if width < 2 || depth < 2 { return None; }
        let columns = (width - 1) as usize;
        let cells = columns * (depth - 1) as usize;
        if triangle_index >= cells * 2 { return None; }
        let cell = triangle_index / 2;
        let second = triangle_index % 2 == 1;
        let x = cell % columns;
        let z = cell / columns;
        let a = z * width as usize + x;
        let b = a.checked_add(1)?;
        let c = a.checked_add(width as usize)?;
        let d = c.checked_add(1)?;
        let (i0, i1, i2) = if second { (b, c, d) } else { (a, c, b) };
        Some((vertex_at(vertices, i0)?, vertex_at(vertices, i1)?, vertex_at(vertices, i2)?))
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(super) enum HitLabel {
        Entity(usize),
        Road(usize),
        Terrain { triangle_index: usize, cell_index: usize },
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(super) struct Hit {
        pub(super) t: f64,
        pub(super) point: [f64; 3],
        pub(super) barycentric: [f64; 3],
        pub(super) label: HitLabel,
    }

    /// Nearest positive hit inside one layer of real triangles.
    pub(super) fn nearest_in(
        triangles: &[([f64; 3], [f64; 3], [f64; 3])],
        origin: [f64; 3],
        dir: [f64; 3],
        label: impl Fn(usize) -> HitLabel,
    ) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for (index, (a, b, c)) in triangles.iter().enumerate() {
            if let Some(t) = ray_triangle(origin, dir, *a, *b, *c) {
                if best.map(|hit| t < hit.t).unwrap_or(true) {
                    let point =
                        [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
                    best = Some(Hit {
                        t,
                        point,
                        barycentric: barycentric(point, *a, *b, *c),
                        label: label(index),
                    });
                }
            }
        }
        best
    }

    /// Visible-layer pick with the approved priority: an entity solid first, then
    /// a road solid, then the terrain triangles. A hidden layer is passed as an
    /// empty slice and can therefore never be hit.
    pub(super) fn pick(
        origin: [f64; 3],
        dir: [f64; 3],
        entities: &[([f64; 3], [f64; 3], [f64; 3])],
        roads: &[([f64; 3], [f64; 3], [f64; 3])],
        terrain: &[([f64; 3], [f64; 3], [f64; 3])],
    ) -> Option<Hit> {
        nearest_in(entities, origin, dir, HitLabel::Entity)
            .or_else(|| nearest_in(roads, origin, dir, HitLabel::Road))
            .or_else(|| {
                nearest_in(terrain, origin, dir, |index| HitLabel::Terrain {
                    triangle_index: index,
                    cell_index: index / 2,
                })
            })
    }

    /// Every drawn terrain triangle of one surface, skipping malformed indices.
    pub(super) fn terrain_triangles_from(
        vertices: &[f64], width: i64, depth: i64,
    ) -> Vec<([f64; 3], [f64; 3], [f64; 3])> {
        if width < 2 || depth < 2 { return Vec::new(); }
        let count = ((width - 1) * (depth - 1) * 2) as usize;
        (0..count).filter_map(|index| terrain_triangle(vertices, width, depth, index)).collect()
    }
}

/// Corner list as a JSON array of `[x, y, z]` triples.
fn corners_json(corners: &[[f64; 3]]) -> Value {
    Value::Array(corners.iter().map(|corner| json!([corner[0], corner[1], corner[2]])).collect())
}

/// The world AABB of a drawn solid as a JSON object.
fn aabb_json(aabb: [f64; 6]) -> Value {
    json!({"minX": aabb[0], "minZ": aabb[1], "maxX": aabb[2], "maxZ": aabb[3], "minY": aabb[4], "maxY": aabb[5]})
}

/// Geometry of one saved entity. Real oriented solid when the saved facts allow
/// it, otherwise an explicit observation-layer record. `savedBounds` is kept
/// verbatim as the world-AABB fact; baked sizes are never re-scaled and no height
/// or grounding is ever invented.
fn entity_geometry(e: &Value) -> Value {
    let saved_bounds = e.get("savedBounds").cloned().unwrap_or(Value::Null);
    let world_x = e.get("worldX").and_then(Value::as_f64);
    let world_y = e.get("worldY").and_then(Value::as_f64);
    let world_z = e.get("worldZ").and_then(Value::as_f64);
    let width = e.get("widthM").and_then(Value::as_f64);
    let depth = e.get("depthM").and_then(Value::as_f64);
    let height = e.get("heightM").and_then(Value::as_f64);
    let rotation = e.get("rotationYDeg").and_then(Value::as_f64);
    let pivot = string_at(e, &["grounding", "pivot"]);
    let bottom_offset = e.pointer("/grounding/bottomOffsetM").and_then(Value::as_f64).unwrap_or(0.0);
    let height_status = match height {
        Some(value) if value > 0.0 => "savedHeightFact",
        Some(_) => "no saved height",
        None => "no saved height fact",
    };
    let mut geometry = json!({
        "savedBounds": saved_bounds.clone(),
        "heightStatus": height_status,
        "rotationYDeg": e.get("rotationYDeg"),
        "scale": e.get("scale"),
        "scaleAppliedToBakedSize": false,
        "savedBoundsIsWorldAabb": true,
        "transformFormula": "worldX=e.worldX+localX*cos+localZ*sin; worldZ=e.worldZ-localX*sin+localZ*cos",
    });
    let oriented = match (world_x, world_y, world_z, width, depth, height, rotation) {
        (Some(wx), Some(wy), Some(wz), Some(w), Some(d), Some(h), Some(rot)) if pivot.is_some() => {
            geom::oriented_box(wx, wy, wz, w, d, h, rot, pivot.as_deref().unwrap_or_default(), bottom_offset)
        }
        _ => None,
    };
    if let Some((corners, min_y, max_y)) = oriented {
        let aabb = geom::corners_aabb(&corners).unwrap_or([0.0; 6]);
        geometry["representation"] = json!("orientedSavedBlock");
        geometry["renderLayer"] = json!("savedWorld3d");
        geometry["runsNo3dPrimitive"] = json!(false);
        geometry["sourceAabbProxy"] = json!(false);
        geometry["pivot"] = json!(pivot);
        geometry["bottomOffsetM"] = json!(bottom_offset);
        geometry["drawnAabb"] = aabb_json(aabb);
        geometry["bounds3d"] = aabb_json(aabb);
        geometry["verticalSpanM"] = json!([min_y, max_y]);
        geometry["cornersMetres"] = corners_json(&corners);
        geometry["triangleIndices"] = json!(geom::BOX_TRIANGLES.to_vec());
        geometry["triangleCount"] = json!(geom::BOX_TRIANGLES.len() / 3);
        geometry["heightFactMm"] = json!(height.map(|value| (value * 1000.0).round() as i64));
        return geometry;
    }
    // No orientable solid. A saved world AABB may still be shown, but only as an
    // explicitly named proxy, never as a precise oriented solid.
    let proxy = match (
        saved_bounds.get("minX").and_then(Value::as_f64),
        saved_bounds.get("minZ").and_then(Value::as_f64),
        saved_bounds.get("maxX").and_then(Value::as_f64),
        saved_bounds.get("maxZ").and_then(Value::as_f64),
    ) {
        (Some(min_x), Some(min_z), Some(max_x), Some(max_z)) => {
            let span = match (world_y, height, pivot.as_deref()) {
                (Some(wy), Some(h), Some(p)) => geom::vertical_span(wy, h, p, bottom_offset),
                _ => None,
            };
            let (min_y, max_y) = span.unwrap_or((0.0, 0.0));
            let flat = span.is_none();
            geom::aabb_box(min_x, min_z, max_x, max_z, min_y, max_y).map(|corners| (corners, flat))
        }
        _ => None,
    };
    match proxy {
        Some((corners, flat)) => {
            geometry["representation"] = json!("sourceAabbProxy");
            geometry["sourceAabbProxy"] = json!(true);
            geometry["renderLayer"] = if flat { json!("observation2d") } else { json!("savedWorld3d") };
            geometry["runsNo3dPrimitive"] = json!(flat);
            geometry["pivot"] = json!(pivot);
            geometry["bottomOffsetM"] = json!(bottom_offset);
            geometry["drawnAabb"] = geom::corners_aabb(&corners).map(aabb_json).unwrap_or(Value::Null);
            geometry["bounds3d"] = geometry["drawnAabb"].clone();
            geometry["cornersMetres"] = corners_json(&corners);
            geometry["triangleIndices"] = json!(geom::BOX_TRIANGLES.to_vec());
            geometry["triangleCount"] = json!(geom::BOX_TRIANGLES.len() / 3);
            if flat {
                geometry["heightStatus"] =
                    json!("no saved height or grounding; observation layer only, no 3D primitive and no constant world Y");
            } else {
                geometry["heightFactMm"] = json!(height.map(|value| (value * 1000.0).round() as i64));
            }
        }
        None => {
            geometry["representation"] = json!("unrepresentedSavedEntity");
            geometry["sourceAabbProxy"] = json!(false);
            geometry["renderLayer"] = json!("observation2d");
            geometry["runsNo3dPrimitive"] = json!(true);
            geometry["heightStatus"] =
                json!("no saved size, height or world AABB; observation layer only, no 3D primitive");
        }
    }
    geometry
}

/// Geometry of one saved road. Size and height are taken from the saved road
/// entity with the exact same identity (never guessed from a distance), and every
/// field records which saved field it came from.
fn road_geometry(r: &Value, entity: Option<&Value>) -> Value {
    let length = r.get("lengthM").and_then(Value::as_f64);
    let rotation = r.get("rotationYDeg").and_then(Value::as_f64);
    let world_x = r.get("worldX").and_then(Value::as_f64);
    let world_z = r.get("worldZ").and_then(Value::as_f64);
    let width = entity.and_then(|e| e.get("depthM")).and_then(Value::as_f64);
    let height = entity.and_then(|e| e.get("heightM")).and_then(Value::as_f64);
    let world_y = entity.and_then(|e| e.get("worldY")).and_then(Value::as_f64);
    let pivot = entity.and_then(|e| string_at(e, &["grounding", "pivot"]));
    let bottom_offset = entity.and_then(|e| e.pointer("/grounding/bottomOffsetM")).and_then(Value::as_f64);
    let entity_width = entity.and_then(|e| e.get("widthM")).and_then(Value::as_f64);
    let mut geometry = json!({
        "representation": "savedLengthRotationObservation",
        "renderLayer": "observation2d",
        "runsNo3dPrimitive": true,
        "snapping": "none",
        "lengthM": r.get("lengthM"),
        "widthM": width,
        "rotationYDeg": r.get("rotationYDeg"),
        "lengthFactSource": "savedRoad.lengthM",
        "widthFactSource": if width.is_some() { "savedRoadEntity.depthM" } else { "missing" },
        "heightFactSource": if height.is_some() { "savedRoadEntity.heightM" } else { "missing" },
        "worldYFactSource": if world_y.is_some() { "savedRoadEntity.worldY" } else { "missing" },
        "associatedRoadEntityId": entity.and_then(|e| e.get("entityId")).cloned().unwrap_or(Value::Null),
        "association": "exactIdentityMatch",
        "widthFactMissing": width.is_none(),
        "heightFactMissing": height.is_none(),
        "savedRoadEntityWidthM": entity_width,
        "lengthAgreesWithSavedEntityWidth": match (length, entity_width) {
            (Some(left), Some(right)) => Some((left - right).abs() < 1e-9),
            _ => None,
        },
        "scaleAppliedToBakedSize": false,
        "savedBoundsIsWorldAabb": true,
        "transformFormula": "worldX=e.worldX+localX*cos+localZ*sin; worldZ=e.worldZ-localX*sin+localZ*cos",
        "heightStatus": "no saved roadbed height; observation layer only, no 3D primitive and no constant world Y",
    });
    let block = match (world_x, world_y, world_z, length, width, height, rotation, pivot.as_deref()) {
        (Some(wx), Some(wy), Some(wz), Some(l), Some(w), Some(h), Some(rot), Some(p)) => {
            // The saved road entity uses local X for the road length and local Z
            // for the cross-road width, so the strip is a length x width block and
            // the saved height gives it its real thickness.
            geom::oriented_box(wx, wy, wz, l, w, h, rot, p, bottom_offset.unwrap_or(0.0))
        }
        _ => None,
    };
    if let Some((corners, min_y, max_y)) = block {
        let aabb = geom::corners_aabb(&corners).unwrap_or([0.0; 6]);
        geometry["representation"] = json!("savedOrientedRoadBedBlock");
        geometry["renderLayer"] = json!("savedWorld3d");
        geometry["runsNo3dPrimitive"] = json!(false);
        geometry["pivot"] = json!(pivot);
        geometry["bottomOffsetM"] = json!(bottom_offset);
        geometry["drawnAabb"] = aabb_json(aabb);
        geometry["bounds3d"] = aabb_json(aabb);
        geometry["verticalSpanM"] = json!([min_y, max_y]);
        geometry["cornersMetres"] = corners_json(&corners);
        geometry["triangleIndices"] = json!(geom::BOX_TRIANGLES.to_vec());
        geometry["triangleCount"] = json!(geom::BOX_TRIANGLES.len() / 3);
        // The horizontal band itself, for the strip observation contract.
        let half_length = length.unwrap_or(0.0) / 2.0;
        let half_width = width.unwrap_or(0.0) / 2.0;
        let band: Vec<[f64; 3]> = [[-1.0_f64, -1.0_f64], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .iter()
            .map(|signs| {
                let (sign_x, sign_z) = (signs[0], signs[1]);
                let [x, z] = geom::transform_point(
                    world_x.unwrap_or(0.0),
                    world_z.unwrap_or(0.0),
                    rotation.unwrap_or(0.0),
                    sign_x * half_length,
                    sign_z * half_width,
                );
                [x, max_y, z]
            })
            .collect();
        geometry["bandCornersMetres"] = corners_json(&band);
        geometry["bandTriangleIndices"] = json!([0, 1, 2, 0, 2, 3]);
        geometry["heightStatus"] = json!("saved roadbed height fact");
        geometry["heightFactMm"] = json!(height.map(|value| (value * 1000.0).round() as i64));
    }
    geometry
}

fn samples_of(v1: &Value, surface: &str) -> Vec<i64> {
    v1.get("surfaces").and_then(|s| s.get(surface)).and_then(|s| s.get("samples"))
        .and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default()
}

/// Builds the versioned 3D model from the audited v1 model. Pure: export and
/// audit both call it, so any tamper differs from the rebuilt model.
fn build_model(inputs: &Inputs) -> Value {
    let v1 = &inputs.v1;
    let window = v1.get("window").cloned().unwrap_or(Value::Null);
    let min_x = number(&window, "minX") as f64;
    let min_z = number(&window, "minZ") as f64;
    let width_m = number(&window, "widthM");
    let depth_m = number(&window, "depthM");
    let mut surfaces = serde_json::Map::new();
    let mut grid = serde_json::Map::new();
    for surface in ["natural", "final"] {
        let samples = samples_of(v1, surface);
        let vertex_count = samples.len();
        let quads = ((width_m - 1).max(0) * (depth_m - 1).max(0)) as usize;
        let mut vertices: Vec<f64> = Vec::with_capacity(vertex_count * 3);
        for index in 0..vertex_count {
            let ix = (index as i64 % width_m.max(1)) as f64;
            let iz = (index as i64 / width_m.max(1)) as f64;
            vertices.push(min_x + ix);
            vertices.push(samples[index] as f64 / 1000.0); // lossless display conversion only
            vertices.push(min_z + iz);
        }
        surfaces.insert(surface.to_string(), json!({
            "source": if surface == "natural" { "crop.naturalMm" } else { "crop.finalMm" },
            "heightsMm": samples,
            "verticesMetres": vertices,
            "vertexCount": vertex_count,
            "quads": quads,
            "triangles": quads * 2,
            "triangleTopology": "rowMajorQuadSplit2",
            "interpolation": "none",
            "fallbackSurface": "none",
            "snapping": "none",
        }));
        grid.insert(surface.to_string(), json!({"sampleWidth": width_m, "sampleDepth": depth_m, "quads": quads, "triangles": quads * 2, "vertexCount": vertex_count}));
    }
    let entity_values: Vec<Value> =
        v1.get("entities").and_then(Value::as_array).cloned().unwrap_or_default();
    let entities: Vec<Value> = entity_values
        .iter()
        .map(|e| {
            let mut geometry = entity_geometry(e);
            geometry["entityId"] = e.get("entityId").cloned().unwrap_or(Value::Null);
            geometry["kind"] = e.get("kind").cloned().unwrap_or(Value::Null);
            geometry["assetId"] = e.get("assetId").cloned().unwrap_or(Value::Null);
            geometry["worldX"] = e.get("worldX").cloned().unwrap_or(Value::Null);
            geometry["worldY"] = e.get("worldY").cloned().unwrap_or(Value::Null);
            geometry["worldZ"] = e.get("worldZ").cloned().unwrap_or(Value::Null);
            geometry["widthM"] = e.get("widthM").cloned().unwrap_or(Value::Null);
            geometry["depthM"] = e.get("depthM").cloned().unwrap_or(Value::Null);
            geometry["heightM"] = e.get("heightM").cloned().unwrap_or(Value::Null);
            geometry["anchors"] = e.get("anchors").cloned().unwrap_or(Value::Null);
            geometry["outline"] = e.get("outline").cloned().unwrap_or(Value::Null);
            geometry
        })
        .collect();
    // Roads take their cross-road width and their roadbed height from the saved
    // road entity with the exact same identity only; a distance heuristic is
    // never used.
    let entity_index: std::collections::BTreeMap<String, &Value> = entity_values
        .iter()
        .filter_map(|e| e.get("entityId").and_then(Value::as_str).map(|id| (id.to_string(), e)))
        .collect();
    let road_values: Vec<Value> = v1.get("roads").and_then(Value::as_array).cloned().unwrap_or_default();
    let roads: Vec<Value> = road_values
        .iter()
        .map(|r| {
            let entity = r.get("roadId").and_then(Value::as_str).and_then(|id| entity_index.get(id)).copied();
            let mut geometry = road_geometry(r, entity);
            geometry["roadId"] = r.get("roadId").cloned().unwrap_or(Value::Null);
            geometry["fromNode"] = r.get("fromNode").cloned().unwrap_or(Value::Null);
            geometry["toNode"] = r.get("toNode").cloned().unwrap_or(Value::Null);
            geometry["worldX"] = r.get("worldX").cloned().unwrap_or(Value::Null);
            geometry["worldZ"] = r.get("worldZ").cloned().unwrap_or(Value::Null);
            geometry["lengthM"] = r.get("lengthM").cloned().unwrap_or(Value::Null);
            geometry["rotationYDeg"] = r.get("rotationYDeg").cloned().unwrap_or(Value::Null);
            geometry["naturalMinM"] = r.get("naturalMinM").cloned().unwrap_or(Value::Null);
            geometry["naturalMaxM"] = r.get("naturalMaxM").cloned().unwrap_or(Value::Null);
            geometry["planEdgePresent"] = r.get("planEdgePresent").cloned().unwrap_or(Value::Null);
            geometry["sourceEntityPresent"] = r.get("sourceEntityPresent").cloned().unwrap_or(Value::Null);
            geometry["sourceEntityKind"] = r.get("sourceEntityKind").cloned().unwrap_or(Value::Null);
            geometry["nodeFactsReferenceOnly"] = r.get("nodeFactsReferenceOnly").cloned().unwrap_or(Value::Null);
            geometry["geometryAgreesWithPlan"] = r.get("geometryAgreesWithPlan").cloned().unwrap_or(Value::Null);
            geometry["geometry"] = r.get("geometry").cloned().unwrap_or(Value::Null);
            geometry
        })
        .collect();
    let mut nodes: Vec<Value> = Vec::new();
    let mut seen: Vec<i64> = Vec::new();
    for road in &road_values {
        for key in ["fromNodeFact", "toNodeFact"] {
            let Some(fact) = road.get(key).cloned() else { continue };
            if fact.is_null() { continue; }
            let id = fact.get("id").and_then(Value::as_i64).unwrap_or(-1);
            if seen.contains(&id) { continue; }
            seen.push(id);
            let has_x = fact.get("worldX").map(|v| !v.is_null()).unwrap_or(false);
            let has_z = fact.get("worldZ").map(|v| !v.is_null()).unwrap_or(false);
            nodes.push(json!({
                "id": id,
                "worldX": fact.get("worldX"), "worldZ": fact.get("worldZ"),
                "referenceOnly": true,
                "coordinatesPresent": has_x && has_z,
                "representation": "savedNodeReference",
                "renderLayer": "observation2d",
                "runsNo3dPrimitive": true,
                "snapping": "none",
                "heightStatus": "no saved node height; observation record only, not positioned and not snapped",
            }));
        }
    }
    let records = v1.get("access").and_then(|a| a.get("records")).and_then(Value::as_array).cloned().unwrap_or_default();
    let access: Vec<Value> = records.iter().map(|r| json!({
        "entityId": r.get("entityId"), "anchorId": r.get("anchorId"), "roadId": r.get("roadId"),
        "distanceM": r.get("distanceM"), "status": r.get("status"),
        "path": r.get("path").cloned().unwrap_or(Value::Array(Vec::new())),
        "pathPointCount": r.get("path").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "inWindow": r.get("inWindow"), "referenceOnly": r.get("referenceOnly"),
        "sourceObservationOnly": true, "notReverified": true,
        "representation": "savedAccessObservation",
        "renderLayer": "observation2d",
        "runsNo3dPrimitive": true,
        "snapping": "none",
        "heightStatus": "no saved access height; observation record only, not positioned and not snapped",
    })).collect();
    let runs_primitive =
        |value: &Value| value.get("runsNo3dPrimitive").and_then(Value::as_bool) == Some(false);
    let misses_height = |value: &Value| value.get("heightFactMm").is_none();
    let observation_only = access.iter().filter(|r| {
        r.get("sourceObservationOnly").and_then(Value::as_bool) == Some(true)
            && r.get("notReverified").and_then(Value::as_bool) == Some(true)
    }).count();
    let entity_solid_count = entities.iter().filter(|v| runs_primitive(v)).count();
    let road_solid_count = roads.iter().filter(|v| runs_primitive(v)).count();
    let heightless_count = entities.iter().filter(|v| misses_height(v)).count()
        + roads.iter().filter(|v| misses_height(v)).count();
    let observation_2d_count = entities.len() - entity_solid_count + roads.len() - road_solid_count
        + nodes.len()
        + access.len();
    // G28: split every fact label per category so the HUD can never present a
    // combined number (4 nodes + 18 access) under a single "nodes" label, and so
    // "no height", "not positioned" and "not snapped" are each reported per
    // category instead of only for entities and roads.
    let category = |total: usize, solid: usize, no_height: usize| -> Value {
        json!({
            "total": total,
            "solid": solid,
            "noHeight": no_height,
            "notPositioned": total - solid,
            "notSnapped": total,
            "snapped": 0,
        })
    };
    let entity_no_height = entities.iter().filter(|v| misses_height(v)).count();
    let road_no_height = roads.iter().filter(|v| misses_height(v)).count();
    let node_total = nodes.len();
    let access_total = access.len();
    let by_category = json!({
        "entities": category(entity_values.len(), entity_solid_count, entity_no_height),
        "roads": category(road_values.len(), road_solid_count, road_no_height),
        "nodes": category(node_total, 0, node_total),
        "access": category(access_total, 0, access_total),
    });
    let heightless_by_category = json!({
        "total": entity_no_height + road_no_height + node_total + access_total,
        "entities": entity_no_height, "roads": road_no_height,
        "nodes": node_total, "access": access_total,
    });
    let not_positioned_by_category = json!({
        "total": entities.len() - entity_solid_count + roads.len() - road_solid_count + node_total + access_total,
        "entities": entities.len() - entity_solid_count, "roads": roads.len() - road_solid_count,
        "nodes": node_total, "access": access_total,
    });
    let not_snapped_by_category = json!({
        "total": entities.len() + roads.len() + node_total + access_total,
        "entities": entities.len(), "roads": roads.len(),
        "nodes": node_total, "access": access_total,
    });
    json!({
        "schemaVersion": SCHEMA3D,
        "contextSchemaVersion": v1.get("schemaVersion"),
        "sources": {
            "world": v1.pointer("/sources/world"), "scene": v1.pointer("/sources/scene"),
            "context": v1.pointer("/sources/context"), "manifest": v1.pointer("/sources/manifest"),
            "marker": v1.pointer("/sources/marker"),
            "declaredSceneId": v1.pointer("/sources/declaredSceneId"), "sceneId": v1.pointer("/sources/sceneId"),
            "v1ModelSha256": inputs.v1_sha256,
            "v1ManifestSha256": inputs.v1_manifest_sha256,
            "v1MarkerSha256": inputs.v1_marker_sha256,
            "contextAuditReportSha256": inputs.audit_sha256,
        },
        "coordinateSystem": {
            "axes": "X east, Y up, Z south",
            "units": "metres",
            "windowOrigin": "G24/G25 window min corner, world metres",
        },
        "heightFactUnit": "millimetre",
        "heightDisplayConversion": "metres = millimetres / 1000 (lossless, display only)",
        "window": window,
        "surfaces": Value::Object(surfaces),
        "grid": Value::Object(grid),
        "entities": entities,
        "roads": roads,
        "nodes": nodes,
        "access": access,
        "status": {
            "entityCount": entity_values.len(),
            "entitySolidCount": entity_solid_count,
            "roadCount": road_values.len(),
            "roadSolidCount": road_solid_count,
            "nodeCount": node_total,
            "accessRecordCount": access_total,
            "observationOnlyCount": observation_only,
            "observation2dObjectCount": observation_2d_count,
            "observation2d": {
                "total": node_total + access_total,
                "nodes": node_total,
                "access": access_total,
                "note": "nodes and access are 2D observation records; they are never given a world Y",
            },
            "heightlessObjectCount": heightless_by_category.get("total").and_then(Value::as_u64).unwrap_or(0),
            "heightlessSolidObjectCount": heightless_count,
            "notPositionedCount": not_positioned_by_category.get("total").and_then(Value::as_u64).unwrap_or(0),
            "notSnappedCount": not_snapped_by_category.get("total").and_then(Value::as_u64).unwrap_or(0),
            "snappedCount": 0,
            "categoryCounts": by_category,
            "noHeight": heightless_by_category,
            "notPositioned": not_positioned_by_category,
            "notSnapped": not_snapped_by_category,
            "constantWorldYUsed": false,
            "accessEmpty": access.is_empty(),
            "naturalEqualsFinal": v1.pointer("/surfaces/naturalEqualsFinal").cloned().unwrap_or(json!(false)),
            "schemaDriftPresent": v1.get("schemaDrift").map(|d| !d.is_null()).unwrap_or(false),
            "unmodelledKindCount": v1.get("unknownKinds").and_then(Value::as_object).map(|m| m.len()).unwrap_or(0),
            "snapping": "none",
        },
        "unknownKinds": v1.get("unknownKinds").cloned().unwrap_or_else(|| json!({})),
        "schemaDrift": v1.get("schemaDrift").cloned().unwrap_or(Value::Null),
        "limitations": v1.get("limitations").cloned().unwrap_or(Value::Array(Vec::new())),
    })
}

fn readme(model: &Value) -> String {
    format!(
        "# Saved-scene context preview 3D (read-only WebGL2)\n\n\
Real WebGL2 consumer of an audited `engineering-context-preview-v1` package. It is a source-fact \
viewer: it builds no EngineeringInput, computes no route, grades no terrain and certifies nothing.\n\n\
* 3D schema: {SCHEMA3D} (independent from {CONTEXT_SCHEMA}; the v1 package is never rewritten or renamed)\n\
* axes: X east, Y up, Z south; window origin from G24/G25; height facts are integer millimetres, \
metres are a lossless display conversion only\n\
* natural and final are two independent switchable layers; vertices come sample by sample from \
`crop.naturalMm` / `crop.finalMm`; no height padding, no interpolation, no fallback surface\n\
* entities and roads are real oriented solids built from their saved world position, saved size, \
`rotationYDeg` and saved pivot/grounding; `savedBounds` stays a world-AABB fact and is neither \
rotated again nor re-scaled\n\
* nodes and access records carry only x/z, so they stay in an explicit observation layer with no 3D \
primitive, no constant world Y and no terrain snapping; the HUD reports them as not positioned / \
no height / not snapped\n\
* access records stay `sourceObservationOnly` / `notReverified`; `connected` and `walkable_route` are \
never promoted to a verified route\n\
* picking uses real mouse events and real indexed-triangle intersection: an entity solid first, then a \
road solid, then the drawn terrain triangles; hidden layers are never hit\n\
* G28 experience layer: a selection panel fed by the same object as the highlight and \
`window.__PREVIEW3D_STATE__` (saved facts and derived display values are labelled separately), per-category \
HUD counts, camera presets (default oblique / top / focus selection / reset) with pointer rotate and wheel \
zoom, a collapsible facts panel, and layered materials with flat-normal lighting, a mid-tone background and \
depth fog\n\
* nodes and access appear only in the explicitly labelled 2D observation overlay and in the HUD counts; \
no world Y, no 3D primitive, no verified route\n\
* entities: {} (solids {}) roads: {} (solids {}) nodes: {} access: {}\n",
        model.pointer("/status/entityCount").cloned().unwrap_or(json!(0)),
        model.pointer("/status/entitySolidCount").cloned().unwrap_or(json!(0)),
        model.pointer("/status/roadCount").cloned().unwrap_or(json!(0)),
        model.pointer("/status/roadSolidCount").cloned().unwrap_or(json!(0)),
        model.pointer("/status/nodeCount").cloned().unwrap_or(json!(0)),
        model.pointer("/status/accessRecordCount").cloned().unwrap_or(json!(0)),
    )
}

const MODEL_BEGIN: &str = "/*3D-MODEL-BEGIN*/";
const MODEL_END: &str = "/*3D-MODEL-END*/";

fn html_for(model_json: &str) -> String {
    HTML_TEMPLATE.replace("__MODEL3D__", &format!("{MODEL_BEGIN}{model_json}{MODEL_END}"))
}

fn embedded_model(html: &str) -> Result<Value, String> {
    let start = html.find(MODEL_BEGIN).ok_or("the page embeds no 3D model")?;
    let end = html.find(MODEL_END).ok_or("the embedded 3D model is not terminated")?;
    if end <= start { return Err("the embedded 3D model markers are inverted".to_string()); }
    serde_json::from_str::<Value>(&html[start + MODEL_BEGIN.len()..end])
        .map_err(|e| format!("the embedded 3D model is invalid: {e}"))
}

/// Producer fingerprint of the static viewer asset. The audit compares the
/// packaged script with the producer constant, so a damaged viewer cannot be
/// smuggled in by synchronising ordinary hash declarations. G31 changed the display
/// parameters (background, fog, ambient share and the material ramps) and nothing
/// else, so the identity moves with the candidate.
const VIEWER_BUILD_ID: &str = "g34-terrain-visibility-6";

fn artifacts_of(model_json: &str, model: &Value) -> Vec<(String, Vec<u8>)> {
    vec![
        (MODEL_FILE.to_string(), model_json.as_bytes().to_vec()),
        (VIEWER_FILE.to_string(), VIEWER_JS.as_bytes().to_vec()),
        (HTML_FILE.to_string(), html_for(model_json).into_bytes()),
        (README_FILE.to_string(), readme(model).into_bytes()),
    ]
}

/// Required-asset rule shared by export and audit: every declared artifact must
/// be present exactly once, and its declared bytes and sha256 must match the
/// bytes actually read. Nothing is inferred from the package under audit.
fn artifact_rule(declared: &[(String, Vec<u8>)]) -> Vec<Value> {
    declared
        .iter()
        .map(|(path, bytes)| {
            json!({
                "path": path,
                "bytes": bytes.len() as u64,
                "sha256": sha256_hex(bytes),
            })
        })
        .collect()
}

/// Checks the manifest's artifact declarations against the bytes on disk.
fn check_artifacts(manifest: &Value, on_disk: &[(String, Vec<u8>)]) -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let Some(declared) = manifest.get("artifacts").and_then(Value::as_array) else {
        return vec!["the manifest declares no artifact set".to_string()];
    };
    if declared.len() != on_disk.len() {
        failures.push(format!(
            "the manifest declares {} artifact(s) but {} were read",
            declared.len(),
            on_disk.len()
        ));
    }
    let mut seen: Vec<String> = Vec::new();
    for entry in declared {
        let path = match entry.get("path").and_then(Value::as_str) {
            Some(path) => path.to_string(),
            None => {
                failures.push("an artifact declaration has no string path".to_string());
                continue;
            }
        };
        if seen.contains(&path) {
            failures.push(format!("the artifact {path} is declared more than once"));
            continue;
        }
        seen.push(path.clone());
        let Some(actual) = on_disk.iter().find(|(name, _)| name == &path) else {
            failures.push(format!("the declared artifact {path} is not one of the required files"));
            continue;
        };
        match entry.get("bytes").and_then(Value::as_u64) {
            Some(bytes) if bytes == actual.1.len() as u64 => {}
            Some(bytes) => failures.push(format!(
                "artifact {path} declares {bytes} bytes but the file has {}",
                actual.1.len()
            )),
            None => failures.push(format!("artifact {path} has no unsigned bytes declaration")),
        }
        match entry.get("sha256").and_then(Value::as_str) {
            Some(sha) if sha == sha256_hex(&actual.1) => {}
            Some(_) => failures.push(format!("artifact {path} declares a different sha256")),
            None => failures.push(format!("artifact {path} has no string sha256 declaration")),
        }
    }
    for (name, _) in on_disk {
        if !seen.contains(name) {
            failures.push(format!("the required artifact {name} is not declared"));
        }
    }
    if string_at(manifest, &["viewerBuildId"]).as_deref() != Some(VIEWER_BUILD_ID) {
        failures.push(format!("the manifest does not declare the producer viewer build {VIEWER_BUILD_ID}"));
    }
    failures
}

/// The static viewer is a producer asset: the packaged script must equal the
/// script this binary produces, and the page must equal the page recomputed from
/// the packaged model, so a tampered script or a page carrying a different model
/// is refused even when every ordinary hash declaration is synchronised.
fn check_producer_assets(model: &Value, model_bytes: &[u8], viewer: Option<&[u8]>, html: Option<&str>) -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    match viewer {
        Some(bytes) if bytes == VIEWER_JS.as_bytes() => {}
        Some(_) => failures.push("the packaged viewer script is not the producer asset".to_string()),
        None => failures.push("the viewer script could not be read".to_string()),
    }
    let model_json = String::from_utf8_lossy(model_bytes).to_string();
    let expected_html = html_for(&model_json);
    match html {
        Some(text) if text == expected_html => {}
        Some(_) => failures.push(
            "the packaged page is not the page recomputed from the packaged model".to_string(),
        ),
        None => failures.push("the page could not be read".to_string()),
    }
    match html {
        Some(text) => match embedded_model(text) {
            Ok(embedded) => {
                // The whole model is compared, not only its geometry groups: the
                // page must consume the same bound model the manifest declares.
                let diff = compare_stage("embedded", model, &embedded, &MODEL_FIELDS);
                if diff.state == "fail" {
                    failures.push(format!("the embedded model differs from the packaged model: {}", diff.detail));
                }
            }
            Err(d) => failures.push(d),
        },
        None => {}
    }
    failures
}

/// Every artifact of a 3D package in write order. The completion marker is
/// always the last artifact, so an interrupted write can never leave a
/// complete-looking package behind.
fn package_artifacts(
    files: &[(String, Vec<u8>)],
    manifest_bytes: &[u8],
    marker_bytes: &[u8],
) -> Vec<(String, Vec<u8>)> {
    let mut artifacts = files.to_vec();
    artifacts.push((MANIFEST_FILE.to_string(), manifest_bytes.to_vec()));
    artifacts.push((MARKER_FILE.to_string(), marker_bytes.to_vec()));
    artifacts
}

/// Parses a JSON corner list into finite `[x, y, z]` triples.
fn corner_list(value: &Value) -> Option<Vec<[f64; 3]>> {
    let array = value.as_array()?;
    let mut corners = Vec::with_capacity(array.len());
    for corner in array {
        let triple = corner.as_array()?;
        if triple.len() != 3 { return None; }
        let point = [triple[0].as_f64()?, triple[1].as_f64()?, triple[2].as_f64()?];
        if !point.iter().all(|value| value.is_finite()) { return None; }
        corners.push(point);
    }
    Some(corners)
}

/// Structural contract of the 3D render model and of its input v1 model: closed
/// root field set, required types, nested array shapes, finite numbers,
/// dimensions and lengths, index range and triangle topology, units and
/// conversion rules. A model that is not this version is refused here; the
/// semantic fact groups are compared by the four audit stages.
fn model_shape_failures(model: &Value) -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    for field in MODEL_FIELDS {
        if model.get(field).is_none() { failures.push(format!("3D model field {field} is missing")); }
    }
    if let Some(object) = model.as_object() {
        for key in object.keys() {
            if !MODEL_FIELDS.contains(&key.as_str()) {
                failures.push(format!("3D model declares an unknown field {key}"));
            }
        }
    }
    match string_at(model, &["schemaVersion"]).as_deref() {
        Some(SCHEMA3D) => {}
        Some(other) => failures.push(format!("unsupported 3D schemaVersion {other}; expected {SCHEMA3D}")),
        None => failures.push("the 3D model declares no schemaVersion".to_string()),
    }
    if string_at(model, &["heightFactUnit"]).as_deref() != Some("millimetre") {
        failures.push("the 3D model heightFactUnit is not the saved millimetre fact".to_string());
    }
    match model.get("heightDisplayConversion").and_then(Value::as_str) {
        Some(text) if text.contains("millimetres / 1000") => {}
        Some(_) => failures.push("the 3D model heightDisplayConversion is not the lossless metre conversion".to_string()),
        None => failures.push("the 3D model declares no heightDisplayConversion".to_string()),
    }
    let axes = string_at(model, &["coordinateSystem", "axes"]).unwrap_or_default();
    if axes != "X east, Y up, Z south" {
        failures.push(format!("the 3D model coordinate axes are not the approved ones: {axes}"));
    }
    if string_at(model, &["coordinateSystem", "units"]).as_deref() != Some("metres") {
        failures.push("the 3D model coordinate units are not metres".to_string());
    }
    let window = model.get("window").cloned().unwrap_or(Value::Null);
    let width_m = number(&window, "widthM");
    let depth_m = number(&window, "depthM");
    if width_m <= 0 || depth_m <= 0 {
        failures.push("the 3D model window has no positive sample dimensions".to_string());
    }
    for surface in ["natural", "final"] {
        let entry = model.get("surfaces").and_then(|s| s.get(surface)).cloned().unwrap_or(Value::Null);
        let samples = match entry.get("heightsMm").and_then(Value::as_array) {
            Some(samples) => samples.clone(),
            None => {
                failures.push(format!("3D surface {surface} declares no saved sample array"));
                continue;
            }
        };
        let expected = (width_m * depth_m) as usize;
        if samples.len() != expected {
            failures.push(format!("3D surface {surface} has {} samples, expected {expected}", samples.len()));
        }
        for sample in &samples {
            match sample.as_i64() {
                Some(mm) if (i32::MIN as i64..=i32::MAX as i64).contains(&mm) => {}
                Some(mm) => {
                    failures.push(format!("3D surface {surface} sample {mm} is outside the saved millimetre range"));
                    break;
                }
                None => {
                    failures.push(format!("3D surface {surface} has a non-integer sample"));
                    break;
                }
            }
        }
        let heights = entry.get("heightsMm").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        if heights != samples.len() {
            failures.push(format!("3D surface {surface} heightsMm does not map every saved sample"));
        }        if entry.get("interpolation").and_then(Value::as_str) != Some("none") {
            failures.push(format!("3D surface {surface} does not declare interpolation none"));
        }
        if entry.get("fallbackSurface").and_then(Value::as_str) != Some("none") {
            failures.push(format!("3D surface {surface} declares a fallback surface"));
        }
        let vertices = entry.get("verticesMetres").and_then(Value::as_array).cloned().unwrap_or_default();
        if vertices.len() != samples.len() * 3 {
            failures.push(format!("3D surface {surface} vertex array does not match its sample count"));
        }
        for vertex in &vertices {
            match vertex.as_f64() {
                Some(value) if value.is_finite() => {}
                _ => {
                    failures.push(format!("3D surface {surface} has a non-finite or non-numeric vertex coordinate"));
                    break;
                }
            }
        }
        let grid = model.get("grid").and_then(|g| g.get(surface)).cloned().unwrap_or(Value::Null);
        if number(&grid, "sampleWidth") != width_m || number(&grid, "sampleDepth") != depth_m {
            failures.push(format!("3D grid {surface} does not carry the window sample dimensions"));
        }
        let quads = (width_m - 1).max(0) * (depth_m - 1).max(0);
        if number(&grid, "vertexCount") != samples.len() as i64 {
            failures.push(format!("3D grid {surface} vertexCount does not match its samples"));
        }
        if number(&grid, "quads") != quads {
            failures.push(format!("3D grid {surface} quads do not cover the window"));
        }
        if number(&grid, "triangles") != quads * 2 {
            failures.push(format!("3D grid {surface} triangle count is not two per quad"));
        }
    }
    for collection in ["entities", "roads"] {
        let Some(entries) = model.get(collection).and_then(Value::as_array) else {
            failures.push(format!("the 3D model declares no {collection} array"));
            continue;
        };
        for entry in entries {
            if entry.get("runsNo3dPrimitive").and_then(Value::as_bool) == Some(true) { continue; }
            let corners = match corner_list(entry.get("cornersMetres").unwrap_or(&Value::Null)) {
                Some(corners) if corners.len() == 8 => corners,
                _ => {
                    failures.push(format!("a 3D {collection} solid does not carry eight finite corners"));
                    continue;
                }
            };
            let indices = entry.get("triangleIndices").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut numeric: Vec<u32> = Vec::with_capacity(indices.len());
            let mut typed = true;
            for index in &indices {
                match index.as_u64() {
                    Some(value) if value < corners.len() as u64 => numeric.push(value as u32),
                    Some(value) => {
                        failures.push(format!("a 3D {collection} solid has an out-of-range corner index {value}"));
                        typed = false;
                        break;
                    }
                    None => {
                        failures.push(format!("a 3D {collection} solid has a non-integer corner index"));
                        typed = false;
                        break;
                    }
                }
            }
            if !typed { continue; }
            if numeric.len() != geom::BOX_TRIANGLES.len() {
                failures.push(format!("a 3D {collection} solid does not declare twelve triangles"));
                continue;
            }
            for triangle in numeric.chunks(3) {
                if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[0] == triangle[2] {
                    failures.push(format!("a 3D {collection} solid declares a degenerate triangle"));
                    break;
                }
            }
        }
    }
    // A node or access observation record must never carry a world Y that was
    // not saved: those layers have no vertical fact at all.
    for collection in ["nodes", "access"] {
        let Some(entries) = model.get(collection).and_then(Value::as_array) else { continue };
        for entry in entries {
            if entry.get("worldY").is_some() || entry.get("bounds3d").is_some() || entry.get("cornersMetres").is_some() {
                failures.push(format!("a 3D {collection} observation record carries a world Y fact that was never saved"));
                break;
            }
            if entry.get("runsNo3dPrimitive").and_then(Value::as_bool) != Some(true) {
                failures.push(format!("a 3D {collection} observation record must not claim a 3D primitive"));
                break;
            }
        }
    }
    if model.pointer("/status/constantWorldYUsed").and_then(Value::as_bool) != Some(false) {
        failures.push("the 3D model does not declare that no constant world Y was used".to_string());
    }
    failures
}

/// The 3D package's own chain: fixed file references, versioned schemas, typed
/// and unique declarations, the model/manifest/marker hash chain, the producer
/// viewer build and the audited v1 bindings.
fn check_package_chain(
    model: &Value,
    model_bytes: &[u8],
    manifest_bytes: &[u8],
    marker: &Value,
    on_disk: &[(String, Vec<u8>)],
    inputs: Option<&Inputs>,
) -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let v1_sha256 = inputs.map(|i| i.v1_sha256.as_str());
    let v1_manifest_sha256 = inputs.map(|i| i.v1_manifest_sha256.as_str());
    let v1_marker_sha256 = inputs.map(|i| i.v1_marker_sha256.as_str());
    let audit_sha256 = inputs.and_then(|i| i.audit_sha256.as_deref());
    let manifest = match object(manifest_bytes, MANIFEST_FILE) {
        Ok(value) => value,
        Err(detail) => return vec![detail],
    };
    failures.extend(check_artifacts(&manifest, on_disk));
    let model_sha = sha256_hex(model_bytes);
    let manifest_sha = sha256_hex(manifest_bytes);
    match string_at(&manifest, &["schemaVersion"]).as_deref() {
        Some(SCHEMA3D_AUDIT) => {}
        Some(other) => failures.push(format!("unsupported 3D manifest schemaVersion {other}")),
        None => failures.push("the 3D manifest declares no schemaVersion".to_string()),
    }
    if string_at(&manifest, &["modelFile"]).as_deref() != Some(MODEL_FILE) {
        failures.push("the 3D manifest does not name the fixed model file".to_string());
    }
    if string_at(&manifest, &["markerFile"]).as_deref() != Some(MARKER_FILE) {
        failures.push("the 3D manifest does not name the fixed completion marker".to_string());
    }
    if string_at(&manifest, &["modelSha256"]).as_deref() != Some(model_sha.as_str()) {
        failures.push("the 3D manifest does not hash the packaged model".to_string());
    }
    match manifest.get("requiredFiles").and_then(Value::as_array) {
        Some(array) => {
            let mut names: Vec<String> = Vec::new();
            let mut typed = true;
            for entry in array {
                match entry.as_str() {
                    Some(name) => names.push(name.to_string()),
                    None => {
                        failures.push("a 3D requiredFiles entry is not a string".to_string());
                        typed = false;
                        break;
                    }
                }
            }
            if typed {
                let mut sorted = names.clone();
                sorted.sort();
                let mut expected: Vec<String> = REQUIRED_FILES.iter().map(|n| n.to_string()).collect();
                expected.sort();
                if sorted != expected || names.len() != REQUIRED_FILES.len() {
                    failures.push(format!("the 3D manifest must declare exactly {expected:?}, once each"));
                }
            }
        }
        None => failures.push("the 3D manifest declares no requiredFiles".to_string()),
    }
    if string_at(&manifest, &["viewerBuildId"]).as_deref() != Some(VIEWER_BUILD_ID) {
        failures.push(format!("the 3D manifest does not declare the producer viewer build {VIEWER_BUILD_ID}"));
    }
    match v1_sha256 {
        Some(expected) if string_at(&manifest, &["contextModelSha256"]).as_deref() == Some(expected) => {}
        Some(_) => failures.push("the 3D manifest does not bind the audited v1 model".to_string()),
        None => failures.push("the audited v1 model was unavailable to bind".to_string()),
    }
    match v1_manifest_sha256 {
        Some(expected) if string_at(&manifest, &["contextManifestSha256"]).as_deref() == Some(expected) => {}
        Some(_) => failures.push("the 3D manifest does not bind the audited v1 manifest".to_string()),
        None => failures.push("the audited v1 manifest was unavailable to bind".to_string()),
    }
    match v1_marker_sha256 {
        Some(expected) if string_at(&manifest, &["contextMarkerSha256"]).as_deref() == Some(expected) => {}
        Some(_) => failures.push("the 3D manifest does not bind the audited v1 completion marker".to_string()),
        None => failures.push("the audited v1 completion marker was unavailable to bind".to_string()),
    }
    match audit_sha256 {
        Some(expected) => {
            if string_at(&manifest, &["contextAuditReportSha256"]).as_deref() != Some(expected) {
                failures.push("the 3D manifest does not bind the supplied context audit trace".to_string());
            }
        }
        None => {
            if !manifest.get("contextAuditReportSha256").map(Value::is_null).unwrap_or(false) {
                failures.push("the 3D manifest declares an audit trace that was not supplied".to_string());
            }
        }
    }
    if manifest.get("sceneId") != model.pointer("/sources/sceneId") {
        failures.push("the 3D manifest sceneId contradicts the packaged model sources".to_string());
    }
    match string_at(marker, &["schemaVersion"]).as_deref() {
        Some(SCHEMA3D_AUDIT) => {}
        Some(other) => failures.push(format!("unsupported 3D marker schemaVersion {other}")),
        None => failures.push("the 3D marker declares no schemaVersion".to_string()),
    }
    if marker.get("status").and_then(Value::as_str) != Some("complete") {
        failures.push("the 3D marker is not complete".to_string());
    }
    if string_at(marker, &["modelFile"]).as_deref() != Some(MODEL_FILE) {
        failures.push("the 3D marker does not name the fixed model file".to_string());
    }
    if string_at(marker, &["manifestFile"]).as_deref() != Some(MANIFEST_FILE) {
        failures.push("the 3D marker does not name the fixed manifest file".to_string());
    }
    if string_at(marker, &["modelSha256"]).as_deref() != Some(model_sha.as_str()) {
        failures.push("the 3D marker does not hash the packaged model".to_string());
    }
    if string_at(marker, &["manifestSha256"]).as_deref() != Some(manifest_sha.as_str()) {
        failures.push("the 3D marker does not hash the packaged manifest".to_string());
    }
    failures
}

fn export_3d(args: &[String]) -> CliResult<()> {
    let preview = PathBuf::from(required(args, "--preview")?);
    let world = PathBuf::from(required(args, "--world")?);
    let context_pkg = PathBuf::from(required(args, "--context")?);
    let audit_trace = flag(args, "--context-audit").map(PathBuf::from);
    let out = PathBuf::from(required(args, "--out")?);
    let inputs = load_inputs(&world, &context_pkg, &preview, audit_trace.as_deref())
        .map_err(|d| -> Box<dyn Error> { format!("context-preview3d refused: {d}").into() })?;
    let model = build_model(&inputs);
    let model_json = serde_json::to_string_pretty(&model)?;
    let model_sha = sha256_hex(model_json.as_bytes());
    let files = artifacts_of(&model_json, &model);
    let manifest = json!({
        "schemaVersion": SCHEMA3D_AUDIT,
        "modelFile": MODEL_FILE, "modelSha256": model_sha,
        "contextModelSha256": inputs.v1_sha256, "contextAuditReportSha256": inputs.audit_sha256,
        "contextManifestSha256": inputs.v1_manifest_sha256,
        "contextMarkerSha256": inputs.v1_marker_sha256,
        "sceneId": model.pointer("/sources/sceneId"),
        "requiredFiles": REQUIRED_FILES,
        "markerFile": MARKER_FILE,
        "viewerBuildId": VIEWER_BUILD_ID,
        "artifacts": artifact_rule(&files),
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let marker = json!({
        "status": "complete", "schemaVersion": SCHEMA3D_AUDIT,
        "modelFile": MODEL_FILE, "modelSha256": model_sha,
        "manifestFile": MANIFEST_FILE, "manifestSha256": sha256_hex(&manifest_bytes),
    });
    let marker_bytes = serde_json::to_vec_pretty(&marker)?;
    let mut owned = claim_directory(&out)?;
    let outcome = (|| -> Result<(), String> {
        publish(&mut owned, &package_artifacts(&files, &manifest_bytes, &marker_bytes), None)
    })();
    match outcome {
        Ok(()) => {
            println!(
                "exported 3D context preview: out={} schema={} entities={}({}) roads={}({}) nodes={} access={} model={} sha256={}",
                out.display(), SCHEMA3D,
                model.pointer("/status/entityCount").cloned().unwrap_or(json!(0)),
                model.pointer("/status/entitySolidCount").cloned().unwrap_or(json!(0)),
                model.pointer("/status/roadCount").cloned().unwrap_or(json!(0)),
                model.pointer("/status/roadSolidCount").cloned().unwrap_or(json!(0)),
                model.pointer("/status/nodeCount").cloned().unwrap_or(json!(0)),
                model.pointer("/status/accessRecordCount").cloned().unwrap_or(json!(0)),
                MODEL_FILE, model_sha
            );
            Ok(())
        }
        Err(detail) => match release(owned) {
            Ok(()) => Err(format!("export failed ({detail}); created objects were cleaned up").into()),
            Err(cleanup) => Err(format!("export failed ({detail}); cleanup also failed: {cleanup}").into()),
        },
    }
}

fn audit_3d(args: &[String]) -> CliResult<()> {
    let preview = PathBuf::from(required(args, "--preview")?);
    let world = PathBuf::from(required(args, "--world")?);
    let context_pkg = PathBuf::from(required(args, "--context")?);
    let audit_trace = flag(args, "--context-audit").map(PathBuf::from);
    let package = PathBuf::from(required(args, "--preview3d")?);
    let report_path = PathBuf::from(required(args, "--report")?);
    // The report must stay outside every protected root: the source world, the
    // audited v1 preview package and the 3D package under audit.
    check_report_path(&report_path, &world, &package)?;
    check_report_path(&report_path, &preview, &package)?;

    let mut completeness: Vec<String> = Vec::new();
    let root = match fs::canonicalize(&package) {
        Ok(r) if r.is_dir() => r,
        Ok(r) => { completeness.push(format!("{} is not a directory", r.display())); PathBuf::new() }
        Err(e) => { completeness.push(format!("preview3d {} is unusable: {e}", package.display())); PathBuf::new() }
    };
    let mut model: Option<Value> = None;
    let mut model_bytes: Option<Vec<u8>> = None;
    let mut manifest_bytes: Option<Vec<u8>> = None;
    let mut html: Option<String> = None;
    let mut viewer_bytes: Option<Vec<u8>> = None;
    let mut readme_bytes: Option<Vec<u8>> = None;
    let mut marker: Option<Value> = None;
    let mut on_disk: Vec<(String, Vec<u8>)> = Vec::new();
    if !root.as_os_str().is_empty() {
        let mut present: Vec<String> = Vec::new();
        match fs::read_dir(&root) {
            Ok(entries) => for entry in entries {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !REQUIRED_FILES.contains(&name.as_str()) {
                            completeness.push(format!("undeclared file {name} is present"));
                        }
                        present.push(name);
                    }
                    Err(e) => completeness.push(format!("cannot read a package entry: {e}")),
                }
            },
            Err(e) => completeness.push(format!("package is unreadable: {e}")),
        }
        for name in REQUIRED_FILES { if !present.iter().any(|p| p == name) { completeness.push(format!("required file {name} is missing")); } }
        match read_confined(&root, MODEL_FILE) {
            Ok(bytes) => { model_bytes = Some(bytes.clone()); match object(&bytes, MODEL_FILE) { Ok(v) => model = Some(v), Err(d) => completeness.push(d) } }
            Err(d) => completeness.push(d),
        }
        match read_confined(&root, MANIFEST_FILE) { Ok(b) => manifest_bytes = Some(b), Err(d) => completeness.push(d) }
        match read_confined(&root, MARKER_FILE) {
            Ok(b) => match object(&b, MARKER_FILE) { Ok(v) => marker = Some(v), Err(d) => completeness.push(d) },
            Err(d) => completeness.push(d),
        }
        match read_confined(&root, HTML_FILE) { Ok(b) => html = String::from_utf8(b).ok(), Err(d) => completeness.push(d) }
        if let Ok(bytes) = read_confined(&root, VIEWER_FILE) {
            if bytes.is_empty() { completeness.push("the WebGL2 viewer script is empty".to_string()); }
            viewer_bytes = Some(bytes);
        }
        match read_confined(&root, README_FILE) { Ok(b) => readme_bytes = Some(b), Err(d) => completeness.push(d) }
        on_disk = vec![
            (MODEL_FILE.to_string(), model_bytes.clone().unwrap_or_default()),
            (VIEWER_FILE.to_string(), viewer_bytes.clone().unwrap_or_default()),
            (HTML_FILE.to_string(), html.as_ref().map(|t| t.as_bytes().to_vec()).unwrap_or_default()),
            (README_FILE.to_string(), readme_bytes.clone().unwrap_or_default()),
        ];
    }

    // Independent source re-check first: the package is never its own trust root.
    let mut source: Vec<String> = Vec::new();
    let inputs = match load_inputs(&world, &context_pkg, &preview, audit_trace.as_deref()) {
        Ok(inputs) => Some(inputs),
        Err(d) => { source.push(d); None }
    };
    let fresh = inputs.as_ref().map(build_model);
    let inputs_available = inputs.is_some();

    if let (Some(model), Some(inputs), Some(fresh)) = (&model, &inputs, &fresh) {
        let diff = compare_stage(
            "sourceBinding",
            fresh,
            model,
            &["sources", "contextSchemaVersion", "window", "coordinateSystem", "heightFactUnit", "heightDisplayConversion"],
        );
        if diff.state == "fail" { source.push(diff.detail); }
        // Bind the audited v1 chain field by field, not only two hashes.
        for (label, value) in [
            ("v1ModelSha256", &inputs.v1_sha256),
            ("v1ManifestSha256", &inputs.v1_manifest_sha256),
            ("v1MarkerSha256", &inputs.v1_marker_sha256),
        ] {
            if string_at(model, &["sources", label]).as_deref() != Some(value.as_str()) {
                source.push(format!("the 3D model does not bind the audited v1 {label}"));
            }
        }
        if string_at(model, &["sources", "contextAuditReportSha256"]).as_deref()
            != inputs.audit_sha256.as_deref()
        {
            source.push("the 3D model does not bind the supplied context audit trace".to_string());
        }
        for key in ["world", "scene", "context", "manifest", "marker", "declaredSceneId", "sceneId"] {
            let expected = inputs.v1.pointer(&format!("/sources/{key}")).cloned().unwrap_or(Value::Null);
            if model.pointer(&format!("/sources/{key}")) != Some(&expected) {
                source.push(format!("the 3D model sources.{key} does not match the audited context"));
            }
        }
    }

    if let Some(model) = &model {
        completeness.extend(model_shape_failures(model));
    }
    if let (Some(model), Some(manifest_bytes), Some(marker)) = (&model, &manifest_bytes, &marker) {
        completeness.extend(check_package_chain(
            model,
            model_bytes.as_deref().unwrap_or(&[]),
            manifest_bytes,
            marker,
            &on_disk,
            inputs.as_ref(),
        ));
    } else {
        completeness.push("the 3D package is incomplete".to_string());
    }
    if let Some(model) = &model {
        completeness.extend(check_producer_assets(
            model,
            model_bytes.as_deref().unwrap_or(&[]),
            viewer_bytes.as_deref(),
            html.as_deref(),
        ));
    }

    // The version gate: a model whose declared version is not this one cannot be
    // interpreted, so no stage may partially compare it.
    let version_known = model
        .as_ref()
        .map(|value| string_at(value, &["schemaVersion"]).as_deref() == Some(SCHEMA3D))
        .unwrap_or(false);
    let comparable = model.is_some() && fresh.is_some() && version_known;

    let mut stages: Vec<Stage> = Vec::new();
    stages.push(match (source.is_empty(), model.is_some(), inputs_available) {
        (false, _, _) => Stage::fail("sourceBinding", source.join("; ")),
        (true, true, true) => Stage::pass(
            "sourceBinding",
            format!(
                "3D sources match the rebuilt audited context {} and its v1 model, manifest and marker",
                &inputs.as_ref().map(|i| i.v1_sha256[..12].to_string()).unwrap_or_default()
            ),
        ),
        _ => Stage::not_evaluated(
            "sourceBinding",
            "the 3D model or the independent source rebuild was unavailable".to_string(),
        ),
    });
    if comparable {
        let (fresh, model) = (fresh.as_ref().unwrap(), model.as_ref().unwrap());
        stages.push(compare_stage("terrainParity", fresh, model, &["surfaces", "grid", "heightFactUnit", "coordinateSystem"]));
        stages.push(compare_stage("entityAndRoadParity", fresh, model, &["entities", "roads", "nodes", "access", "status", "unknownKinds", "schemaDrift", "limitations"]));
    } else {
        let reason = if model.is_some() && !version_known {
            "the 3D model version is not interpretable, so no field group is partially compared".to_string()
        } else {
            "the 3D model or the rebuilt context was unavailable".to_string()
        };
        stages.push(Stage::not_evaluated("terrainParity", reason.clone()));
        stages.push(Stage::not_evaluated("entityAndRoadParity", reason));
    }
    stages.push(if completeness.is_empty() {
        Stage::pass("previewCompleteness", format!("{} required 3D files present, unique and hash-bound to the producer assets", REQUIRED_FILES.len()))
    } else {
        Stage::fail("previewCompleteness", completeness.join("; "))
    });

    let total_pass = stages.iter().all(|s| s.state == "pass");
    let report = json!({
        "schemaVersion": SCHEMA3D_AUDIT,
        "preview": preview.display().to_string(),
        "preview3d": package.display().to_string(),
        "modelSha256": model_bytes.as_deref().map(sha256_hex),
        "stages": stages.iter().map(|s| json!({"name": s.name, "state": s.state, "detail": s.detail})).collect::<Vec<Value>>(),
        "totalPass": total_pass,
        "evaluatedStages": stages.iter().filter(|s| s.state != "notEvaluated").map(|s| s.name).collect::<Vec<&str>>(),
        "notEvaluatedStages": stages.iter().filter(|s| s.state == "notEvaluated").map(|s| s.name).collect::<Vec<&str>>(),
    });
    write_report(&report_path, &report)?;
    for stage in &stages { println!("stage {}: {} ({})", stage.name, stage.state, stage.detail); }
    println!("3D audit {}: totalPass={} report={}", if total_pass { "passed" } else { "failed" }, total_pass, report_path.display());
    if total_pass { Ok(()) } else {
        let failing: Vec<&str> = stages.iter().filter(|s| s.state != "pass").map(|s| s.name).collect();
        Err(format!("3D audit failed at {}", failing.join(", ")).into())
    }
}

// G29: one viewer, one model, two information layouts. The page boots in browse
// mode (map first, compact toolbar, one concise selection card) and every fact
// the inspect mode shows is also in the DOM while browse mode is active - only
// the facts panel is hidden, never the values. Nothing overlays the map: the
// toolbar and the selection card are flex rows, so the measured interactive map
// rectangle is the canvas itself and no transparent panel can cover it.
// G33 adds exactly one declared exception to that rule: a fixed, opaque 140x140 svg
// overview pinned to the stage corner (g33-contract.zh-CN.md section 4). It is off the
// layout flow, so the measured canvas rectangle and the G32 area floors are unchanged;
// every other chrome element is still a flex row that never covers the map.
const HTML_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>保存场景 3D 只读预览 (read-only WebGL2)</title>
<style>
 html,body { margin:0; height:100%; font:13px/1.45 ui-monospace,Consolas,monospace; background:#0b0e12; color:#d8dee9; }
 body { display:flex; flex-direction:column; }
 /* G32-R2 layout closeout: the navigation surface was made compact in NORMAL flow so the
    real map rectangle reaches the 75% (960x640) and 80% (1280x800) area floors recorded in
    layout-closeout-contract.zh-CN.md. Only spacing and font size change: no control is
    removed or hidden, no text is shortened, no id changes, and no overlay is introduced.
    The reading-bar keeps its flex row, its nowrap summary line and its <=48px budget. */
 #topbar { flex:0 0 auto; display:flex; flex-wrap:wrap; align-items:center; gap:2px 6px; padding:3px 8px; border-bottom:1px solid #2b3038; box-sizing:border-box; }
 .group { display:flex; flex-wrap:wrap; align-items:center; gap:2px 5px; padding:1px 5px; border:1px solid #262c34; border-radius:6px; }
 .glabel { color:#8fa1b3; font-size:10px; white-space:nowrap; }
 #topbar label { display:inline-flex; align-items:center; gap:3px; cursor:pointer; white-space:nowrap; font-size:10px; }
 #topbar button, #hud-toggle { font:10px/1.15 ui-monospace,Consolas,monospace; color:#dbe3ee; background:#1b2129; border:1px solid #3a4350; border-radius:5px; padding:3px 5px; cursor:pointer; }
 #topbar button:hover, #hud-toggle:hover { background:#2c3644; }
 #topbar button[aria-pressed="true"] { background:#2e4a6b; border-color:#4b7fb5; color:#eaf2ff; }
 #more-camera { display:flex; align-items:center; gap:3px; }
 #more-camera summary { color:#8fa1b3; font-size:10px; cursor:pointer; }
 #more-camera[open] { flex-wrap:wrap; }
 #notice { white-space:nowrap; }
 #workspace { flex:1 1 auto; display:flex; min-height:0; }
 #stage { flex:1 1 auto; position:relative; min-width:0; min-height:0; }
 #gl { display:block; width:100%; height:100%; }
 #overlay { position:absolute; inset:0; width:100%; height:100%; pointer-events:none; }
 /* G33 navigation overview minimap. A fixed-size svg box pinned to the stage corner.
    It is off the layout flow (absolute), opaque, and adds no stacking-order rule, no
    viewport pinning and no clipping fallback, so the measured canvas rectangle and the
    G32 area floors are untouched. It is NOT a second WebGL surface: the page keeps
    exactly its two existing canvases. Declared deviation from the G30-R3 "nothing
    overlays the map" rule in g33-contract.zh-CN.md section 4. */
 #minimap { position:absolute; right:10px; bottom:10px; width:140px; height:140px; background:#11151b; pointer-events:auto; cursor:crosshair; }
 #minimap-window { fill:none; stroke:#3a4350; stroke-width:1; }
 #minimap-north { stroke:#8fa1b3; stroke-width:1; }
 #minimap-footprint { fill:rgba(124,196,255,0.12); stroke:rgba(124,196,255,0.95); stroke-width:1; }
 #minimap-target { fill:#f0b849; stroke:#11151b; stroke-width:1.2; }
/* G34 occlusion layer: only hidden cells are drawn. Warm, low-luminance and semi-transparent so
   the blue footprint stays visible underneath and the amber target dot stays the boldest mark on
   the map. Purely additive: it changes no existing rule above. */
.minimap-hidden-cell { fill:#c2803a; fill-opacity:0.42; stroke:none; }
/* The accessibility channel: the same states as text, so colour is never the only carrier.
   G34-R2: the short status sits INSIDE the 140x140 minimap box. #minimap is absolutely positioned
   at right:10px / bottom:10px with a 140x140 box, and this span mirrors exactly those two offsets at
   the same right/bottom edges, so it lands inside the box WITHOUT introducing a wrapper element that
   would change the minimap's own rect. It ignores pointer events so a drag started on the text still
   reaches the minimap, and it wraps rather than clipping so no text is ever lost. */
#minimap-visibility-status { position:absolute; right:10px; bottom:10px; width:140px; box-sizing:border-box; padding:1px 3px; background:rgba(17,21,27,0.88); color:#cbbcae; font:10px/1.3 ui-monospace, monospace; border-top:1px solid #3a4350; pointer-events:none; white-space:normal; word-break:break-all; }
 #hud { width:400px; flex:0 0 400px; padding:10px 12px; border-left:1px solid #2b3038; height:100%; overflow:auto; box-sizing:border-box; }
 body.hud-collapsed #hud { display:none; }
 body.view-browse #hud { display:none; }
 #selection-strip { flex:0 0 auto; max-height:34vh; overflow:auto; padding:5px 9px; border-top:1px solid #2b3038; box-sizing:border-box; }
 /* G30-R3 compact reading summary: one normal-flow row between the map and the
    selection card, never a floating panel over the canvas, so the measured map
    rectangle keeps its contract area. The full reading facts moved into the
    inspect sidebar behind the 读图详情 control; the summary carries only the short
    always-on facts. Nothing here clips content: the row is sized by its content
    and is expected to stay inside the reviewed single-row budget. */
 #reading-bar { flex:0 0 auto; display:flex; flex-wrap:wrap; align-items:center; gap:2px 8px; padding:1px 6px; border-top:1px solid #2b3038; box-sizing:border-box; font-size:11px; line-height:1.2; }
 .reading-group { display:flex; align-items:center; gap:5px; min-width:0; }
 .reading-group > .reading-label { color:#8fa1b3; white-space:nowrap; }
 #reading-direction { display:flex; align-items:center; gap:5px; min-width:0; }
 #reading-direction svg.dir { display:block; flex:0 0 auto; }
 .reading-note { color:#c3ccd8; white-space:nowrap; }
 /* G30-R4: the compact summary row keeps its single no-wrap line, and the rule is
    scoped to #reading-bar so it can never reach the sidebar detail legend again.
    The unscoped .legend-row/.legend-item nowrap pair used to clip the detail items
    at the panel edge. */
 #reading-bar .legend-row { display:flex; flex-wrap:nowrap; gap:2px 8px; }
 #reading-bar .legend-item { display:inline-flex; align-items:center; gap:3px; white-space:nowrap; }
 /* G30-R4: the detail legend wraps naturally inside the sidebar. Items may shrink
    and their long labels break internally; the swatches never shrink and no
    overflow clipping is used as a fallback. */
 #reading-details .legend-row { display:flex; flex-wrap:wrap; gap:3px 8px; }
 #reading-details .legend-item { display:inline-flex; align-items:flex-start; gap:3px; min-width:0; max-width:100%; white-space:normal; overflow-wrap:anywhere; }
 #reading-details .muted { overflow-wrap:anywhere; }
 .legend-item.is-hidden { opacity:0.55; text-decoration:line-through; }
 .swatch { display:inline-block; flex:0 0 auto; width:8px; height:8px; border:1px solid #3a4350; border-radius:2px; }
 .swatch.ramp { width:34px; }
 #reading-range { white-space:nowrap; }
 #reading-details-open { font:11px/1.2 ui-monospace,Consolas,monospace; color:#dbe3ee; background:#1b2129; border:1px solid #3a4350; border-radius:5px; padding:3px 7px; cursor:pointer; }
 #reading-details-open:hover { background:#2c3644; }
 .reading-detail h2 { font-size:12px; margin:8px 0 4px; color:#8fa1b3; text-transform:uppercase; }
 .reading-detail .dt-block { margin:0 0 4px; }
 h1 { font-size:13px; margin:0 0 6px; } h2 { font-size:12px; margin:10px 0 4px; color:#8fa1b3; text-transform:uppercase; }
 .badge { display:inline-block; padding:1px 6px; border-radius:8px; background:#2b3038; margin-right:4px; }
 .warn { color:#f0b849; } .ok { color:#7fd18b; } .err { color:#ff6b6b; } .muted { color:#8794a5; }
 ul { margin:4px 0; padding-left:18px; }
 details { margin:6px 0; } summary { cursor:pointer; color:#8fa1b3; }
 table.sel { width:100%; border-collapse:collapse; font-size:12px; }
 table.sel th { text-align:left; vertical-align:top; padding:2px 6px 2px 0; color:#93a1b3; font-weight:400; white-space:nowrap; }
 table.sel td { vertical-align:top; padding:2px 0; word-break:break-word; }
 table.sel td.fact { color:#cfe3ff; }
 table.sel td.derived { color:#f0b849; }
 .legend { margin-top:6px; font-size:11px; }
 .legend .fact { color:#cfe3ff; } .legend .derived { color:#f0b849; }
 #counts div { margin:1px 0; }
 .card { display:flex; flex-wrap:wrap; align-items:baseline; gap:3px 14px; }
 .card .k { color:#8fa1b3; }
 .card .v { color:#cfe3ff; }
 .card .note { color:#f0b849; }
</style></head><body class="view-browse hud-collapsed">
<header id="topbar">
 <div class="group" role="group" aria-label="显示模式">
  <span class="glabel">显示模式</span>
  <button id="mode-browse" type="button" aria-pressed="true">浏览（地图优先）</button>
  <button id="mode-inspect" type="button" aria-pressed="false">检查（完整事实）</button>
 </div>
 <div class="group" role="group" aria-label="地形层">
  <span class="glabel">地形层</span>
  <label><input type="radio" name="surface" value="final" checked> 最终 final</label>
  <label><input type="radio" name="surface" value="natural"> 自然 natural</label>
 </div>
 <div class="group" role="group" aria-label="图层显隐">
  <span class="glabel">图层</span>
  <label><input type="checkbox" id="showEntities" checked> 实体</label>
  <label><input type="checkbox" id="showRoads" checked> 道路</label>
  <label><input type="checkbox" id="showObservation" checked> 2D观测（无世界Y）</label>
 </div>
 <div class="group" id="camera-bar" role="group" aria-label="视角与选择">
  <span class="glabel">视角</span>
  <button id="cam-default" type="button">默认斜视</button>
  <button id="cam-top" type="button">俯视</button>
  <button id="cam-focus" type="button">聚焦选择</button>
  <button id="cam-reset" type="button">复位视角</button>
  <button id="cam-clear" type="button">清除选择</button>
  <details id="more-camera"><summary>更多视角</summary>
   <button id="cam-oblique" type="button">斜视</button>
   <button id="cam-rotate-left" type="button">左转</button>
   <button id="cam-rotate-right" type="button">右转</button>
   <button id="cam-zoom-in" type="button">放大</button>
   <button id="cam-zoom-out" type="button">缩小</button>
  </details>
 </div>
 <div class="group" role="group" aria-label="只读状态">
  <span class="glabel">状态</span>
  <span>load: <span id="load-status" class="ok">complete</span><span id="load-error" class="err"></span></span>
  <span id="notice" class="warn">只读3D预览 · 路线/工程未验证 · 观测记录无世界Y</span>
 </div>
</header>
<div id="workspace">
 <div id="stage">
  <canvas id="gl"></canvas>
  <canvas id="overlay"></canvas>
   <svg id="minimap" viewBox="0 0 140 140" role="img" aria-label="导航总览小地图（只读，点击或拖拽移动相机目标；遮挡层显示被地形挡住的采样格）">
    <defs>
     <!-- G34-R3: the drawn area must be cellRect ∩ footprint ∩ window. An SVG clipPath containing a
          polygon AND a rect is their UNION, not their intersection -- the R2 revision did that and the
          inspector measured painted pixels outside the triangle. The intersection is expressed with
          TWO NESTED clip groups instead: the outer group is clipped to the footprint polygon, and the
          window group inside it is clipped to the window rectangle. Each clipPath holds exactly one
          shape, so no union can occur. Both are in user space, in the same 0..140 units as the rects. -->
     <clipPath id="minimap-footprint-clip" clipPathUnits="userSpaceOnUse">
      <polygon id="minimap-domain-clip-poly" points=""></polygon>
     </clipPath>
     <clipPath id="minimap-window-clip" clipPathUnits="userSpaceOnUse">
      <rect id="minimap-domain-clip-window" x="0" y="0" width="140" height="140"></rect>
     </clipPath>
    </defs>
    <rect id="minimap-window" x="0.5" y="0.5" width="139" height="139"></rect>
    <line id="minimap-north" x1="70" y1="6" x2="70" y2="14"></line>
    <polygon id="minimap-footprint" points=""></polygon>
    <g id="minimap-footprint-clip-group" display="none" clip-path="url(#minimap-footprint-clip)">
     <g id="minimap-hidden" display="none" data-hidden-cells="0" clip-path="url(#minimap-window-clip)"></g>
    </g>
    <circle id="minimap-target" cx="70" cy="70" r="3"></circle>
   </svg>
   <span id="minimap-visibility-status" role="status" aria-live="polite" data-status="unavailable" data-visible="0" data-hidden="0" data-undefined="0" data-outside-domain="0" data-outside-view="0" data-unknown-data="0" data-numerical-degenerate="0" data-eligible="0" data-knowledge-complete="false" data-unresolved-count="0" data-resolution="16x16" data-reason="" data-reference-mode="terrain-surface" data-clip-polygon-points="0"></span>
 </div>
 <aside id="hud" aria-label="检查信息面板">
  <h1>Saved-scene context preview 3D</h1>
  <button id="hud-toggle" type="button" aria-expanded="true">收起检查信息</button>
  <div>contract: <span class="badge">independent-3d-readonly-consumer</span> real WebGL2 consumer of an audited saved-scene context package; no EngineeringInput, no route, no engineering build</div>
  <div id="gl-info"></div>
  <h2>Layer</h2>
  <div id="layer-info"></div>
  <h2>Counts (saved facts)</h2>
  <div id="counts"></div>
  <details id="evidence-details"><summary>source, schema drift and limitations</summary>
   <div id="evidence"></div>
   <h2>Limitations</h2><ul id="limitations"></ul>
  </details>
  <div class="warn">nodes and access records are observation-only: no 3D primitive, no constant world Y, no snapping, never a verified route</div>
  <details id="reading-details" class="reading-detail"><summary>读图详情（方向、图例与保存高程范围）</summary>
   <h2>方向</h2><div id="reading-detail-direction" class="dt-block"></div>
   <h2>图例</h2><div id="reading-detail-legend" class="dt-block"></div>
   <h2>保存高程范围</h2><div id="reading-detail-range" class="dt-block"></div>
  </details>
 </aside>
</div>
<div id="reading-bar" role="region" aria-label="读图摘要：方向、图例与保存高程范围">
 <div class="reading-group" aria-label="方向">
  <div id="reading-direction"></div>
 </div>
 <div class="reading-group" aria-label="图例">
  <div id="reading-legend"></div>
 </div>
 <div class="reading-group" aria-label="保存范围">
  <div id="reading-range"></div>
 </div>
 <button id="reading-details-open" type="button" aria-controls="reading-details" aria-expanded="false">读图详情</button>
</div>
<div id="selection-strip" role="region" aria-label="选择信息">
 <div id="selection" class="muted">nothing selected - click an entity solid, a road bed or the terrain (click the background to clear)</div>
</div>
<script type="application/json" id="render-model-3d">__MODEL3D__</script>
<script src="viewer3d.js"></script>
</body></html>
"##;

const VIEWER_JS: &str = include_str!("g28_viewer3d.js");

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("gwc-3d-{}-{}-{}", std::process::id(), tag, nanos));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }
    fn g25_package() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/collaboration/ds-b01-g25-context-preview-consumer/runs/p16-final-package/package")
    }
    fn g25_audit() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/collaboration/ds-b01-g25-context-preview-consumer/runs/p02-audit-preview/report.json")
    }
    fn copied_package(tag: &str) -> PathBuf {
        let target = temp_dir(tag);
        for name in ["render-model.json", "manifest.json", "COMPLETE.json", "preview.html", "viewer.js", "README.md"] {
            fs::copy(g25_package().join(name), target.join(name)).expect("copy");
        }
        target
    }
    fn world_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/joint-r27-verified-baseline")
    }
    fn g24_context_package() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../bevy/work/collaboration/ds-b01-g24-saved-scene-context/continuation-4/runs/c01-export-clean/package",
        )
    }
    fn inputs(tag: &str) -> Inputs {
        load_inputs(&world_dir(), &g24_context_package(), &copied_package(tag), Some(&g25_audit()))
            .expect("the audited package must rebuild from the given source")
    }

    fn fresh_3d(tag: &str) -> PathBuf {
        let out = temp_dir(tag).join("package");
        let args: Vec<String> = vec![
            "--world".into(), world_dir().display().to_string(),
            "--context".into(), g24_context_package().display().to_string(),
            "--preview".into(), g25_package().display().to_string(),
            "--out".into(), out.display().to_string(),
        ];
        export_3d(&args).expect("the clean export must succeed");
        out
    }
    fn fresh_3d_with_trace(tag: &str, trace: &Path) -> PathBuf {
        let out = temp_dir(tag).join("package");
        let args: Vec<String> = vec![
            "--world".into(), world_dir().display().to_string(),
            "--context".into(), g24_context_package().display().to_string(),
            "--preview".into(), g25_package().display().to_string(),
            "--context-audit".into(), trace.display().to_string(),
            "--out".into(), out.display().to_string(),
        ];
        export_3d(&args).expect("the clean export with a real trace must succeed");
        out
    }
    fn copied_3d(from: &Path, tag: &str) -> PathBuf {
        let target = temp_dir(tag);
        for name in REQUIRED_FILES { fs::copy(from.join(name), target.join(name)).expect("copy"); }
        target
    }
    fn audit_args(dir: &Path) -> Vec<String> {
        let report = temp_dir("audit-report").join("report.json");
        vec![
            "--world".into(), world_dir().display().to_string(),
            "--context".into(), g24_context_package().display().to_string(),
            "--preview".into(), g25_package().display().to_string(),
            "--preview3d".into(), dir.display().to_string(),
            "--report".into(), report.display().to_string(),
        ]
    }
    fn reseal(dir: &Path) {
        let model_bytes = fs::read(dir.join(MODEL_FILE)).expect("model");
        let model_sha = sha256_hex(&model_bytes);
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        manifest["modelSha256"] = json!(model_sha);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        let mut marker: Value = serde_json::from_slice(&fs::read(dir.join(MARKER_FILE)).expect("read")).expect("json");
        marker["modelSha256"] = json!(model_sha);
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join(MARKER_FILE), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
    }

    /// Makes a tampered 3D package fully self-consistent: the page is recomputed
    /// from the packaged model, the artifact declarations, the manifest and the
    /// marker are re-bound. This isolates one semantic tamper from a format error.
    fn reseal_self_consistent(dir: &Path) {
        let model_bytes = fs::read(dir.join(MODEL_FILE)).expect("model");
        let model_json = String::from_utf8_lossy(&model_bytes).to_string();
        let model: Value = serde_json::from_str(&model_json).expect("json");
        let files = artifacts_of(&model_json, &model);
        for (name, bytes) in &files {
            if name != MODEL_FILE { fs::write(dir.join(name), bytes).expect("write"); }
        }
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        manifest["modelSha256"] = json!(sha256_hex(&model_bytes));
        manifest["artifacts"] = json!(artifact_rule(&files));
        manifest["sceneId"] = model.pointer("/sources/sceneId").cloned().unwrap_or(Value::Null);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        let mut marker: Value =
            serde_json::from_slice(&fs::read(dir.join(MARKER_FILE)).expect("read")).expect("json");
        marker["modelSha256"] = json!(sha256_hex(&model_bytes));
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join(MARKER_FILE), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
    }

    /// Re-seals the v1 preview package chain after a deliberate edit.
    fn reseal_v1(dir: &Path) {
        let model_bytes = fs::read(dir.join("render-model.json")).expect("model");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("read")).expect("json");
        manifest["modelSha256"] = json!(sha256_hex(&model_bytes));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("write");
        let mut marker: Value =
            serde_json::from_slice(&fs::read(dir.join("COMPLETE.json")).expect("read")).expect("json");
        marker["modelSha256"] = json!(sha256_hex(&model_bytes));
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join("COMPLETE.json"), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
    }

    /// Runs an audit with a report path this test controls, so the written report
    /// can be inspected stage by stage.
    fn audit_with(dir: &Path, trace: Option<&Path>) -> (Result<(), String>, Value) {
        let report = temp_dir("audit-report").join("report.json");
        let mut args = vec![
            "--world".to_string(), world_dir().display().to_string(),
            "--context".to_string(), g24_context_package().display().to_string(),
            "--preview".to_string(), g25_package().display().to_string(),
            "--preview3d".to_string(), dir.display().to_string(),
            "--report".to_string(), report.display().to_string(),
        ];
        if let Some(trace) = trace {
            args.push("--context-audit".into());
            args.push(trace.display().to_string());
        }
        let outcome = audit_3d(&args).map_err(|error| error.to_string());
        let value: Value = serde_json::from_slice(
            &fs::read(&report).expect("the audit must always write its report"),
        )
        .expect("json");
        (outcome, value)
    }

    fn stage_state(report: &Value, name: &str) -> String {
        report["stages"]
            .as_array()
            .expect("stages")
            .iter()
            .find(|stage| stage["name"] == json!(name))
            .and_then(|stage| stage["state"].as_str())
            .unwrap_or("missing")
            .to_string()
    }

    /// One real G25 production report, copied and edited in one field.
    fn trace_case(tag: &str, edit: impl FnOnce(&mut Value)) -> PathBuf {
        let mut report: Value =
            serde_json::from_slice(&fs::read(g25_audit()).expect("the real G25 report must be readable"))
                .expect("json");
        edit(&mut report);
        let path = temp_dir("trace-cases").join(format!("{tag}.json"));
        fs::write(&path, serde_json::to_vec_pretty(&report).expect("json")).expect("write");
        path
    }

    fn named_entity(model: &Value, id: &str) -> Value {
        model["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["entityId"] == json!(id))
            .cloned()
            .unwrap_or_else(|| panic!("the named entity {id} must be present"))
    }

    fn named_road(model: &Value, id: &str) -> Value {
        model["roads"]
            .as_array()
            .expect("roads")
            .iter()
            .find(|road| road["roadId"] == json!(id))
            .cloned()
            .unwrap_or_else(|| panic!("the named road {id} must be present"))
    }

    #[test]
    fn a_clean_package_passes_the_shared_asset_rules_and_the_audit() {
        let dir = fresh_3d("clean-assets");
        let manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        let viewer = fs::read(dir.join(VIEWER_FILE)).expect("viewer");
        let html = fs::read_to_string(dir.join(HTML_FILE)).expect("html");
        let readme = fs::read(dir.join(README_FILE)).expect("readme");
        let model_bytes = fs::read(dir.join(MODEL_FILE)).expect("model");
        let model: Value = serde_json::from_slice(&model_bytes).expect("json");
        let on_disk = vec![
            (MODEL_FILE.to_string(), model_bytes.clone()),
            (VIEWER_FILE.to_string(), viewer.clone()),
            (HTML_FILE.to_string(), html.as_bytes().to_vec()),
            (README_FILE.to_string(), readme),
        ];
        assert!(check_artifacts(&manifest, &on_disk).is_empty(), "declared artifacts must match");
        assert!(check_producer_assets(&model, &model_bytes, Some(&viewer), Some(&html)).is_empty());
        audit_3d(&audit_args(&dir)).expect("the clean package must pass the four-stage audit");
    }

    #[test]
    fn a_damaged_viewer_is_refused_even_with_synchronised_declarations() {
        let dir = copied_3d(&fresh_3d("source-3d"), "damaged-viewer");
        let mut viewer = fs::read(dir.join(VIEWER_FILE)).expect("viewer");
        viewer.extend_from_slice(b"\n/*tampered*/\n");
        fs::write(dir.join(VIEWER_FILE), &viewer).expect("write");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        if let Some(artifacts) = manifest.get_mut("artifacts").and_then(Value::as_array_mut) {
            for entry in artifacts.iter_mut() {
                if entry.get("path").and_then(Value::as_str) == Some(VIEWER_FILE) {
                    entry["bytes"] = json!(viewer.len() as u64);
                    entry["sha256"] = json!(sha256_hex(&viewer));
                }
            }
        }
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        reseal(&dir);
        assert!(audit_3d(&audit_args(&dir)).is_err(), "a damaged viewer must be refused");
    }

    /// G30/G30-R3: the compact reading summary must be real, readable DOM and must
    /// derive its values from the live camera and the saved samples instead of a fixed
    /// screen-up assumption or a fabricated colour. The full facts moved into the
    /// inspect sidebar, so both layers are asserted instead of a CSS string.
    #[test]
    fn the_reading_aids_are_rendered_from_the_live_camera_and_the_saved_samples() {
        let html = html_for("{}");
        // The compact summary hosts (static template) and the detail hosts (static
        // template as well, filled by the viewer) must all exist exactly once.
        let ids = [
            "reading-bar", "reading-direction", "reading-legend", "reading-range", "reading-details-open",
            "reading-details", "reading-detail-direction", "reading-detail-legend", "reading-detail-range",
        ];
        for id in ids {
            let needle = format!("id=\"{id}\"");
            assert_eq!(html.matches(&needle).count(), 1, "the page must declare #{id} exactly once");
        }
        assert!(html.contains("#reading-bar"), "the reading bar must be styled");
        // The summary must be a normal-flow row, not chrome floating over the map.
        // (#overlay is the pointer-events:none observation content, not chrome.)
        assert!(html.contains("#reading-bar { flex:0 0 auto;"), "the reading bar must be a flex row in the layout");
        assert!(!html.contains("z-index"), "no chrome may float above the map");
        assert!(!html.contains("overflow:hidden"), "the compact summary must not clip its content to fit");
        // The details live in the inspect sidebar, not in a second canvas or an overlay.
        let hud_at = html.find("id=\"hud\"").expect("the facts panel exists");
        let details_at = html.find("id=\"reading-details\"").expect("the detail section exists");
        let workspace_at = html.find("id=\"selection-strip\"").expect("the selection strip exists");
        assert!(hud_at < details_at && details_at < workspace_at, "the reading details must live inside the facts panel");
        assert_eq!(html.matches("<canvas").count(), 2, "the page keeps exactly its two existing canvases");
        // The frame statement, the saved-sample wording and the degeneracy path
        assert!(VIEWER_JS.contains("本地世界坐标：X 东、Y 上、Z 南"), "the page must state the local frame explicitly");
        assert!(VIEWER_JS.contains("不是测绘真北"), "the page must say it is not a geodetic north");
        assert!(VIEWER_JS.contains("function directionReading()") && VIEWER_JS.contains("function viewBasis()"));
        assert!(VIEWER_JS.contains("lookAt(cameraEye(), state.camera.target"), "the direction must follow the live camera transform");
        assert!(VIEWER_JS.contains("AXIS_DEGENERATE_LENGTH") && VIEWER_JS.contains("方向退化"), "a collapsed projection must be reported as degenerate");
        assert!(VIEWER_JS.contains("function legendReading()") && VIEWER_JS.contains("MATERIALS.entity") && VIEWER_JS.contains("MATERIALS.road"), "the legend must read the render configuration");
        assert!(VIEWER_JS.contains("OBSERVATION_COLOURS"), "the overlay swatch must reuse the drawing colour");
        assert!(VIEWER_JS.contains("未贴高、未复核路线"), "the observation entry must keep its honesty wording");
        assert!(VIEWER_JS.contains("function rangeReading()") && VIEWER_JS.contains("heightsMm"), "the range must come from the saved millimetre samples");
        assert!(VIEWER_JS.contains("保存样本范围"), "the range must be labelled as a saved-sample range");
        assert!(VIEWER_JS.contains("no finite saved sample"), "an empty or non-finite layer must report why instead of zero");
        assert!(VIEWER_JS.contains("function rgbCss(") && VIEWER_JS.contains("rampColour(material.ramp, 0)"), "the ramp swatch must reuse the height mapping");
        assert!(VIEWER_JS.contains("window.__reading3d"), "the reading values must be readable for independent comparison");
        // the build identity moved to the G33 overview value and away from the older ones
        assert_eq!(VIEWER_BUILD_ID, "g34-terrain-visibility-6");
        assert_ne!(VIEWER_BUILD_ID, "g32-camera-pan-3-layout-1");
        assert_ne!(VIEWER_BUILD_ID, "g30-detail-legend-3");
        assert_ne!(VIEWER_BUILD_ID, "g30-compact-reading-2");
        assert_ne!(VIEWER_BUILD_ID, "g30-map-reading-1");
        assert_ne!(VIEWER_BUILD_ID, "g29-map-first-view-1");
    }

    /// G30-R4: the compact summary and the sidebar detail legend are separate style
    /// scopes. The summary keeps its single no-wrap line; the detail legend wraps so
    /// no item or swatch can be clipped at the panel edge, and no overflow clipping is
    /// used as a fallback. These guards are static: they do not replace the browser
    /// visibility measurement (which lives in the focused acceptance driver).
    #[test]
    fn the_summary_and_the_detail_legend_styles_are_separate_scopes() {
        let html = html_for("{}");
        // The summary rule is scoped and keeps the accepted single line.
        assert!(html.contains("#reading-bar .legend-row"), "the summary legend row rule must be scoped to #reading-bar");
        assert!(html.contains("#reading-bar .legend-item"), "the summary legend item rule must be scoped to #reading-bar");
        assert!(html.contains("#reading-bar .legend-row { display:flex; flex-wrap:nowrap;"), "the compact summary row must keep its single no-wrap line");
        assert!(html.contains("#reading-bar .legend-item { display:inline-flex; align-items:center; gap:3px; white-space:nowrap;"), "the compact summary item must keep its no-wrap text");
        // The unscoped pair that leaked into the details is gone.
        assert!(!html.contains("\n .legend-row { display:flex; flex-wrap:nowrap;"), "no unscoped .legend-row no-wrap rule may remain");
        assert!(!html.contains("\n .legend-item { display:inline-flex; align-items:center; gap:3px; white-space:nowrap;"), "no unscoped .legend-item no-wrap rule may remain");
        // The detail legend wraps and may break long labels internally.
        assert!(html.contains("#reading-details .legend-row { display:flex; flex-wrap:wrap;"), "the detail legend row must wrap");
        assert!(html.contains("#reading-details .legend-item {"), "the detail legend item rule must be scoped to #reading-details");
        assert!(html.contains("#reading-details .legend-item { display:inline-flex; align-items:flex-start; gap:3px; min-width:0; max-width:100%; white-space:normal;"), "the detail legend item must be shrinkable and wrap its text");
        assert!(html.contains("#reading-details .muted { overflow-wrap:anywhere; }"), "long detail strings must be allowed to break");
        // Swatches never shrink away, and no overflow clipping is used as a fallback.
        assert!(html.contains(".swatch { display:inline-block; flex:0 0 auto; width:8px; height:8px;"), "a swatch must never be squeezed away");
        assert!(!html.contains("overflow:hidden"), "the fix must not clip content to fit");
        assert!(!html.contains("text-overflow:ellipsis"), "the fix must not ellipsize content");
        // The two scopes are distinct selectors, so one cannot silently override the other.
        let scoped_summary = html.matches("#reading-bar .legend-item").count();
        let scoped_detail = html.matches("#reading-details .legend-item").count();
        assert_eq!(scoped_summary, 1, "exactly one scoped summary legend item rule");
        assert_eq!(scoped_detail, 1, "exactly one scoped detail legend item rule");
    }

    /// G30-R3: the display hierarchy is part of the contract, so it is guarded by
    /// meaning rather than by old CSS strings: the always-on summary keeps the short
    /// facts, the detail layer keeps the full facts, the entry point is a real control,
    /// and showing or hiding the details may not touch the camera, the model or the
    /// surface.
    #[test]
    fn the_compact_summary_and_the_expandable_details_keep_their_contract() {
        let html = html_for("{}");
        // The compact summary keeps the short direction note and the detail entry point.
        assert!(VIEWER_JS.contains("X东·Y上·Z南 / 局部坐标，非真北"), "the summary must carry the short frame note");
        assert!(VIEWER_JS.contains("读图详情"), "the summary must offer the details entry point");
        assert!(VIEWER_JS.contains("LEGEND_SUMMARY_ORDER"), "the summary must render a subset of the legend");
        assert!(VIEWER_JS.contains("summaryLabel"), "the summary legend must use short labels");
        assert!(VIEWER_JS.contains("2D观测（无世界Y·未复核）"), "the observation limit must stay briefly visible");
        assert!(VIEWER_JS.contains("保存样本 "), "the summary must show the current saved range");
        assert!(VIEWER_JS.contains("方向退化："), "the summary must keep a short degeneracy hint");
        // The detail layer keeps the full facts.
        for fact in [
            "本地世界坐标：X 东、Y 上、Z 南（局部坐标轴，不是测绘真北）",
            "相机 yaw ",
            "保存样本范围",
            "不是城市海拔基准",
            "未贴高、未复核路线",
            "基础材质色；实际像素还受光照与雾影响",
            "个保存样本",
            "来源 ",
        ] {
            assert!(VIEWER_JS.contains(fact), "the detail layer must keep the full fact: {fact}");
        }
        // The entry point is a real control wired to a guarded function.
        assert!(html.contains("id=\"reading-details-open\" type=\"button\""), "the entry point must be a real button");
        assert!(html.contains("aria-controls=\"reading-details\""), "the entry point must name the section it opens");
        assert!(VIEWER_JS.contains("function openReadingDetails()"), "the entry point must be wired to a guarded function");
        assert!(VIEWER_JS.contains("setViewMode(\"inspect\")"), "the entry point must use the existing inspect layout");
        assert!(VIEWER_JS.contains("readingDetailsOpen"), "the open flag must be published for independent checks");
        assert!(VIEWER_JS.contains("readingDetailSignature"), "the detail layer must not be rebuilt while its text is unchanged");
        assert!(VIEWER_JS.contains("addEventListener(\"toggle\""), "a manual toggle must stay in sync with the flag");
        // The details are a layout region, never a floating overlay or a second canvas.
        assert!(!html.contains("reading-overlay"), "the details must not introduce an overlay");
    }

    #[test]
    fn the_producer_identity_names_the_current_build_and_a_drifting_declaration_is_refused() {
        assert_eq!(VIEWER_BUILD_ID, "g34-terrain-visibility-6", "the producer constant must name the G34-R5 terrain-visibility build");
        assert_ne!(VIEWER_BUILD_ID, "g32-camera-pan-3-layout-1", "the build must not reuse the G32-R2 layout closeout identity");
        assert_ne!(VIEWER_BUILD_ID, "g30-detail-legend-3", "the build must not reuse the previous G30 identity");
        assert_ne!(VIEWER_BUILD_ID, "g30-compact-reading-2", "the build must not reuse the earlier G30 identity");
        assert_ne!(VIEWER_BUILD_ID, "g30-map-reading-1", "the build must not reuse the earlier G30 identity");
        assert_ne!(VIEWER_BUILD_ID, "g29-map-first-view-1", "the build must not reuse the G29 identity");
        assert_ne!(VIEWER_BUILD_ID, "g28r1h1-viewer-3d-4", "the build must not reuse the G28-R1-H1 identity");
        let dir = copied_3d(&fresh_3d("source-3d-g29-id"), "stale-build-id");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        assert_eq!(manifest["viewerBuildId"], json!(VIEWER_BUILD_ID), "the producer must write its own identity into the manifest");
        // A package whose own hashes agree but which still declares the previous
        // producer identity describes a different viewer and must be refused.
        manifest["viewerBuildId"] = json!("g29-map-first-view-1");
        fs::write(dir.join(MANIFEST_FILE), serde_json::to_vec_pretty(&manifest).expect("json")).expect("write");
        reseal(&dir);
        assert!(audit_3d(&audit_args(&dir)).is_err(), "a package that still declares the previous producer identity must be refused");
    }

    #[test]
    fn deleting_a_required_file_and_its_declaration_is_refused() {
        let dir = copied_3d(&fresh_3d("source-3d-del"), "deleted-readme");
        fs::remove_file(dir.join(README_FILE)).expect("remove");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        if let Some(artifacts) = manifest.get_mut("artifacts").and_then(Value::as_array_mut) {
            artifacts.retain(|entry| entry.get("path").and_then(Value::as_str) != Some(README_FILE));
        }
        if let Some(required) = manifest.get_mut("requiredFiles").and_then(Value::as_array_mut) {
            required.retain(|entry| entry.as_str() != Some(README_FILE));
        }
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        reseal(&dir);
        assert!(audit_3d(&audit_args(&dir)).is_err(), "a missing required file must be refused");
    }

    #[test]
    fn tampering_only_the_embedded_model_is_refused() {
        let dir = copied_3d(&fresh_3d("source-3d-html"), "embedded-only");
        let text = fs::read_to_string(dir.join(HTML_FILE)).expect("html");
        let start = text.find(MODEL_BEGIN).expect("begin") + MODEL_BEGIN.len();
        let end = text.find(MODEL_END).expect("end");
        let mut embedded: Value = serde_json::from_str(&text[start..end]).expect("json");
        embedded["status"]["entityCount"] = json!(999);
        embedded["sources"]["sceneId"] = json!("scene-9");
        let replaced = format!(
            "{}{}{}",
            &text[..start],
            serde_json::to_string_pretty(&embedded).expect("json"),
            &text[end..]
        );
        fs::write(dir.join(HTML_FILE), replaced).expect("write");
        assert!(audit_3d(&audit_args(&dir)).is_err(), "an embedded-only model change must be refused");
    }

    #[test]
    fn the_real_audit_trace_contract_is_enforced_field_by_field() {
        // The untouched real production report is a positive control, and it is
        // also the base of every negative case: no hand-reduced report is used to
        // claim the full contract.
        let dir = copied_package("trace-contract");
        assert!(
            load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&g25_audit())).is_ok(),
            "the real G25 report must satisfy the trace contract"
        );
        let cases: Vec<(&str, Box<dyn FnOnce(&mut Value)>)> = vec![
            ("unknown-schema-version", Box::new(|r: &mut Value| r["schemaVersion"] = json!("context-preview-audit-v999"))),
            ("missing-schema-version", Box::new(|r: &mut Value| { r.as_object_mut().expect("object").remove("schemaVersion"); })),
            ("total-pass-false", Box::new(|r: &mut Value| r["totalPass"] = json!(false))),
            ("total-pass-missing", Box::new(|r: &mut Value| { r.as_object_mut().expect("object").remove("totalPass"); })),
            ("wrong-model-hash", Box::new(|r: &mut Value| r["modelSha256"] = json!("00"))),
            ("duplicated-stage", Box::new(|r: &mut Value| {
                let stages = r["stages"].as_array_mut().expect("stages");
                let first = stages[0].clone();
                stages.push(first);
            })),
            ("unknown-stage", Box::new(|r: &mut Value| r["stages"][0]["name"] = json!("routeVerified"))),
            ("missing-detail", Box::new(|r: &mut Value| { r["stages"][0].as_object_mut().expect("stage").remove("detail"); })),
            ("stage-not-a-pass", Box::new(|r: &mut Value| r["stages"][1]["state"] = json!("fail"))),
            ("mistyped-not-evaluated", Box::new(|r: &mut Value| r["notEvaluatedStages"] = json!("terrainParity"))),
            ("missing-not-evaluated", Box::new(|r: &mut Value| { r.as_object_mut().expect("object").remove("notEvaluatedStages"); })),
            ("not-evaluated-left", Box::new(|r: &mut Value| r["notEvaluatedStages"] = json!(["terrainParity"]))),
            ("missing-evaluated", Box::new(|r: &mut Value| { r.as_object_mut().expect("object").remove("evaluatedStages"); })),
            ("empty-evaluated", Box::new(|r: &mut Value| r["evaluatedStages"] = json!([]))),
            ("short-evaluated", Box::new(|r: &mut Value| r["evaluatedStages"] = json!(["sourceBinding"]))),
            ("contradictory-evaluated", Box::new(|r: &mut Value| r["evaluatedStages"] = json!(["previewCompleteness", "entityAndRoadParity", "terrainParity", "sourceBinding"]))),
            ("missing-stages", Box::new(|r: &mut Value| { r.as_object_mut().expect("object").remove("stages"); })),
        ];
        for (label, edit) in cases {
            let path = trace_case(label, edit);
            assert!(
                load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&path)).is_err(),
                "a trace must be refused: {label}"
            );
        }
        // A report whose four stages and both summaries agree is still accepted.
        let good = trace_case("complete", |r: &mut Value| {
            r["notEvaluatedStages"] = json!([]);
            r["evaluatedStages"] = json!(["sourceBinding", "terrainParity", "entityAndRoadParity", "previewCompleteness"]);
        });
        assert!(
            load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&good)).is_ok(),
            "a complete four-stage trace must be accepted"
        );
    }

    #[test]
    fn a_trace_is_read_through_a_bounded_ordinary_file_path() {
        assert!(!read_bounded_regular(&g25_audit(), MAX_TRACE_BYTES, "trace").expect("the real report is a small regular file").is_empty());
        let big = temp_dir("oversized-trace").join("report.json");
        fs::write(&big, vec![b' '; 64]).expect("write");
        let error = read_bounded_regular(&big, 16, "trace").expect_err("the size must be checked before the bytes are read");
        assert!(error.contains("read cap"), "unexpected error: {error}");
        let error = read_bounded_regular(&temp_dir("trace-directory"), MAX_TRACE_BYTES, "trace")
            .expect_err("a directory is not an ordinary file");
        assert!(error.contains("not a regular file"), "unexpected error: {error}");
        assert!(read_bounded_regular(&temp_dir("missing-trace").join("nope.json"), MAX_TRACE_BYTES, "trace").is_err());
    }

    #[test]
    fn a_forged_audit_trace_cannot_authorize_export() {
        let dir = copied_package("forged-trace");
        let model_sha = sha256_hex(&fs::read(dir.join("render-model.json")).expect("read"));
        let forged = temp_dir("forged-report").join("report.json");
        fs::write(&forged, format!(r#"{{"totalPass":true,"modelSha256":"{model_sha}"}}"#)).expect("write");
        assert!(
            load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&forged)).is_err(),
            "totalPass alone must not authorize a 3D package"
        );
    }

    #[test]
    fn an_edited_source_model_is_refused_even_with_a_synced_chain_and_forged_trace() {
        let dir = copied_package("edited-source");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join("render-model.json")).expect("read")).expect("json");
        model["surfaces"]["final"]["samples"][0] = json!(123456);
        let bytes = serde_json::to_vec_pretty(&model).expect("json");
        fs::write(dir.join("render-model.json"), &bytes).expect("write");
        let model_sha = sha256_hex(&bytes);
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("read")).expect("json");
        manifest["modelSha256"] = json!(model_sha);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("write");
        let mut marker: Value = serde_json::from_slice(&fs::read(dir.join("COMPLETE.json")).expect("read")).expect("json");
        marker["modelSha256"] = json!(model_sha);
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join("COMPLETE.json"), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
        let forged = temp_dir("edited-report").join("report.json");
        fs::write(
            &forged,
            format!(
                r#"{{"totalPass":true,"modelSha256":"{model_sha}","stages":[{{"name":"sourceBinding","state":"pass"}},{{"name":"terrainParity","state":"pass"}},{{"name":"entityAndRoadParity","state":"pass"}},{{"name":"previewCompleteness","state":"pass"}}]}}"#
            ),
        )
        .expect("write");
        assert!(
            load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&forged)).is_err(),
            "an edited source model must fail the independent rebuild even with a forged trace"
        );
    }

    #[test]
    fn the_3d_model_is_versioned_and_reuses_the_audited_sources() {
        let model = build_model(&inputs("sources"));
        assert_eq!(model["schemaVersion"], json!(SCHEMA3D));
        assert_eq!(model["contextSchemaVersion"], json!(CONTEXT_SCHEMA));
        assert_eq!(model["heightFactUnit"], json!("millimetre"));
        assert_eq!(model["coordinateSystem"]["axes"], json!("X east, Y up, Z south"));
        assert!(model["sources"]["v1ModelSha256"].is_string());
        assert!(model["sources"]["contextAuditReportSha256"].is_string());
        assert_eq!(model["sources"]["sceneId"], json!("scene-0"));
    }

    #[test]
    fn both_surfaces_map_every_sample_without_interpolation_or_fallback() {
        let inputs = inputs("surfaces");
        let model = build_model(&inputs);
        for surface in ["natural", "final"] {
            let expected = samples_of(&inputs.v1, surface);
            let heights: Vec<i64> = model["surfaces"][surface]["heightsMm"].as_array().expect("heights").iter().filter_map(Value::as_i64).collect();
            assert_eq!(heights, expected, "{surface} must map every saved millimetre sample");
            assert_eq!(model["grid"][surface]["vertexCount"], json!(4096));
            assert_eq!(model["grid"][surface]["quads"], json!(3969));
            assert_eq!(model["grid"][surface]["triangles"], json!(7938));
            assert_eq!(model["surfaces"][surface]["interpolation"], json!("none"));
            assert_eq!(model["surfaces"][surface]["fallbackSurface"], json!("none"));
            assert_eq!(model["surfaces"][surface]["verticesMetres"].as_array().expect("vertices").len(), 4096 * 3);
        }
        let natural = &model["surfaces"]["natural"]["heightsMm"];
        let final_field = &model["surfaces"]["final"]["heightsMm"];
        assert_ne!(natural, final_field, "the two layers stay independent");
    }

    #[test]
    fn entities_roads_nodes_and_access_keep_their_saved_facts() {
        let model = build_model(&inputs("facts"));
        let entities = model["entities"].as_array().expect("entities");
        assert!(!entities.is_empty());
        assert!(entities.iter().all(|e| {
            e["representation"] == json!("orientedSavedBlock")
                && e["renderLayer"] == json!("savedWorld3d")
                && e["runsNo3dPrimitive"] == json!(false)
                && e["sourceAabbProxy"] == json!(false)
                && e["scaleAppliedToBakedSize"] == json!(false)
                && e["savedBounds"].is_object()
                && e["cornersMetres"].as_array().map(Vec::len) == Some(8)
                && e["triangleIndices"].as_array().map(Vec::len) == Some(36)
                && e["heightFactMm"].is_i64()
        }));
        let roads = model["roads"].as_array().expect("roads");
        assert!(!roads.is_empty());
        assert!(roads.iter().all(|r| {
            r["representation"] == json!("savedOrientedRoadBedBlock")
                && r["association"] == json!("exactIdentityMatch")
                && r["lengthFactSource"] == json!("savedRoad.lengthM")
                && r["widthFactSource"] == json!("savedRoadEntity.depthM")
                && r["snapping"] == json!("none")
        }));
        let nodes = model["nodes"].as_array().expect("nodes");
        assert!(!nodes.is_empty());
        assert!(nodes.iter().all(|n| {
            n["referenceOnly"] == json!(true)
                && n["renderLayer"] == json!("observation2d")
                && n["runsNo3dPrimitive"] == json!(true)
                && n.get("worldY").is_none()
                && n.get("cornersMetres").is_none()
        }));
        let access = model["access"].as_array().expect("access");
        assert_eq!(access.len(), 18);
        assert!(access.iter().all(|a| {
            a["sourceObservationOnly"] == json!(true)
                && a["notReverified"] == json!(true)
                && a["snapping"] == json!("none")
                && a["renderLayer"] == json!("observation2d")
                && a["runsNo3dPrimitive"] == json!(true)
                && a.get("worldY").is_none()
        }));
        assert_eq!(model["status"]["snapping"], json!("none"));
        assert_eq!(model["status"]["constantWorldYUsed"], json!(false));
        assert_eq!(model["status"]["notPositionedCount"], json!(nodes.len() + access.len()));
        assert!(model["status"]["observation2dObjectCount"].as_u64().unwrap() >= (nodes.len() + access.len()) as u64);
        assert_eq!(model["status"]["entitySolidCount"], json!(entities.len()));
        assert_eq!(model["status"]["roadSolidCount"], json!(roads.len()));
        // G28: the solid-object count stays separate from the true total, and the
        // combined observation count is split into nodes and access.
        assert_eq!(model["status"]["heightlessSolidObjectCount"], json!(0), "every saved entity and road in this context carries a saved height");
        assert_eq!(model["status"]["heightlessObjectCount"], json!(22), "nodes and access carry no saved height, so the true total is 22");
        assert_eq!(model["status"]["observation2d"]["total"], json!(22));
        assert_eq!(model["status"]["observation2d"]["nodes"], json!(4));
        assert_eq!(model["status"]["observation2d"]["access"], json!(18));
        assert_eq!(model["status"]["snappedCount"], json!(0));
    }

    /// G28: every fact label the HUD shows is split per category, so a combined
    /// count can never be presented under a single category's name.
    #[test]
    fn every_hud_fact_label_is_split_per_category() {
        let model = build_model(&inputs("category-counts"));
        let status = &model["status"];
        let categories = &status["categoryCounts"];
        for (name, total, solid, no_height) in [
            ("entities", 4_u64, 4_u64, 0_u64),
            ("roads", 3, 3, 0),
            ("nodes", 4, 0, 4),
            ("access", 18, 0, 18),
        ] {
            assert_eq!(categories[name]["total"], json!(total), "{name} total");
            assert_eq!(categories[name]["solid"], json!(solid), "{name} drawable solids");
            assert_eq!(categories[name]["noHeight"], json!(no_height), "{name} without a saved height");
            assert_eq!(categories[name]["notPositioned"], json!(total - solid), "{name} not positioned in 3D");
            assert_eq!(categories[name]["notSnapped"], json!(total), "{name} not snapped to a surface");
            assert_eq!(categories[name]["snapped"], json!(0), "{name} snapped objects");
        }
        assert_eq!(status["noHeight"]["entities"], json!(0));
        assert_eq!(status["noHeight"]["roads"], json!(0));
        assert_eq!(status["noHeight"]["nodes"], json!(4));
        assert_eq!(status["noHeight"]["access"], json!(18));
        assert_eq!(status["noHeight"]["total"], json!(22));
        assert_eq!(status["notPositioned"]["total"], json!(22));
        assert_eq!(status["notSnapped"]["total"], json!(29));
        assert_eq!(status["notSnapped"]["entities"], json!(4));
        assert_eq!(status["notSnapped"]["roads"], json!(3));
        // The node count must never be published as the nodes+access total.
        assert_ne!(status["nodeCount"], status["observation2d"]["total"]);
        assert_eq!(status["nodeCount"], json!(4));
        assert_eq!(status["accessRecordCount"], json!(18));
    }

    #[test]
    fn unknown_versions_and_a_failed_audit_report_are_refused() {
        let dir = copied_package("version");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join("render-model.json")).expect("read")).expect("json");
        model["schemaVersion"] = json!("engineering-context-preview-v999");
        let bytes = serde_json::to_vec_pretty(&model).expect("json");
        fs::write(dir.join("render-model.json"), &bytes).expect("write");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("read")).expect("json");
        manifest["modelSha256"] = json!(sha256_hex(&bytes));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("write");
        let mut marker: Value = serde_json::from_slice(&fs::read(dir.join("COMPLETE.json")).expect("read")).expect("json");
        marker["modelSha256"] = json!(sha256_hex(&bytes));
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join("COMPLETE.json"), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
        assert!(load_inputs(&world_dir(), &g24_context_package(), &dir, None).is_err(), "an unknown v1 version must be refused");

        let failed = temp_dir("failed-trace").join("report.json");
        fs::write(&failed, br#"{"totalPass":false,"modelSha256":"00"}"#).expect("write");
        assert!(
            load_inputs(&world_dir(), &g24_context_package(), &g25_package(), Some(&failed)).is_err(),
            "a non-passing trace must be refused"
        );
    }

    #[test]
    fn tampering_is_caught_by_the_matching_stage() {
        let inputs = inputs("tamper");
        let fresh = build_model(&inputs);
        let mut terrain = fresh.clone();
        terrain["surfaces"]["natural"]["heightsMm"][0] = json!(987654);
        assert_eq!(compare_stage("terrainParity", &fresh, &terrain, &["surfaces", "grid"]).state, "fail");
        let mut entity = fresh.clone();
        entity["entities"] = json!([]);
        assert_eq!(compare_stage("entityAndRoadParity", &fresh, &entity, &["entities"]).state, "fail");
        let mut access = fresh.clone();
        access["access"][0]["sourceObservationOnly"] = json!(false);
        assert_eq!(compare_stage("entityAndRoadParity", &fresh, &access, &["access"]).state, "fail");
        let mut nodes = fresh.clone();
        nodes["nodes"] = json!([]);
        assert_eq!(compare_stage("entityAndRoadParity", &fresh, &nodes, &["nodes"]).state, "fail");
    }

    #[test]
    fn the_page_embeds_the_declared_model_and_needs_no_server() {
        let model = build_model(&inputs("page"));
        let json = serde_json::to_string_pretty(&model).expect("json");
        let html = html_for(&json);
        assert!(html.contains("viewer3d.js"));
        assert!(!html.contains("fetch("), "the page must load without a server");
        assert_eq!(embedded_model(&html).expect("embedded"), model);
        assert!(embedded_model("<html></html>").is_err());
    }

    #[test]
    fn building_and_rendering_never_write_into_the_input_package() {
        let dir = copied_package("read-only");
        let before: Vec<String> = ["render-model.json", "manifest.json", "COMPLETE.json"].iter()
            .map(|n| sha256_hex(&fs::read(dir.join(n)).expect("read"))).collect();
        let inputs = load_inputs(&world_dir(), &g24_context_package(), &dir, Some(&g25_audit())).expect("loads");
        let model = build_model(&inputs);
        let html = html_for(&serde_json::to_string_pretty(&model).expect("json"));
        assert!(html.contains("WebGL2") || html.contains("3D-MODEL"));
        let after: Vec<String> = ["render-model.json", "manifest.json", "COMPLETE.json"].iter()
            .map(|n| sha256_hex(&fs::read(dir.join(n)).expect("read"))).collect();
        assert_eq!(before, after, "the audited input package must not be modified");
        assert_eq!(fs::read_dir(&dir).expect("dir").count(), 6);
    }

    // -----------------------------------------------------------------------
    // §4.7 named geometry and picking tests
    // -----------------------------------------------------------------------

    /// §4.7(1): the named saved road entity `generated.road-network.scene-0.4-11`
    /// (worldXYZ 288,1,64; 64x8x0.24; rotation 90; scale 1; bottom pivot) must
    /// transform to exactly its saved world AABB x284..292 / z32..96.
    #[test]
    fn the_named_90_degree_entity_transforms_to_its_saved_world_aabb() {
        let model = build_model(&inputs("named-entity"));
        let entity = named_entity(&model, "generated.road-network.scene-0.4-11");
        assert_eq!(entity["worldX"], json!(288));
        assert_eq!(entity["worldY"], json!(1));
        assert_eq!(entity["worldZ"], json!(64));
        assert_eq!(entity["widthM"], json!(64.0));
        assert_eq!(entity["depthM"], json!(8.0));
        assert_eq!(entity["heightM"], json!(0.24));
        assert_eq!(entity["rotationYDeg"], json!(90.0));
        assert_eq!(entity["scale"], json!(1.0));
        assert_eq!(entity["representation"], json!("orientedSavedBlock"));
        assert_eq!(entity["sourceAabbProxy"], json!(false));
        assert_eq!(entity["scaleAppliedToBakedSize"], json!(false));
        assert_eq!(entity["savedBoundsIsWorldAabb"], json!(true));
        let corners = corner_list(&entity["cornersMetres"]).expect("corners");
        assert_eq!(corners.len(), 8);
        let aabb = geom::corners_aabb(&corners).expect("aabb");
        let saved = &entity["savedBounds"];
        for (computed, expected) in [
            (aabb[0], saved["minX"].as_f64().expect("minX")),
            (aabb[1], saved["minZ"].as_f64().expect("minZ")),
            (aabb[2], saved["maxX"].as_f64().expect("maxX")),
            (aabb[3], saved["maxZ"].as_f64().expect("maxZ")),
        ] {
            assert!(
                (computed - expected).abs() < 1e-9,
                "the 90 degree oriented block must reproduce the saved world AABB: {computed} vs {expected}"
            );
        }
        assert!((aabb[4] - 1.0).abs() < 1e-12 && (aabb[5] - 1.24).abs() < 1e-12, "the bottom pivot anchors the block at the saved world Y");
        let indices: Vec<u32> = entity["triangleIndices"].as_array().expect("indices").iter().filter_map(Value::as_u64).map(|value| value as u32).collect();
        assert_eq!(indices.len(), 36);
        assert_eq!(geom::triangles_of(&corners, &indices).len(), 12, "a real solid has twelve triangles");
    }

    /// §4.7(1)/§4.4: the named road strip must take its width and height from the
    /// saved road entity with the exact same identity, never from a distance.
    #[test]
    fn the_named_road_strip_uses_the_saved_entity_by_exact_identity() {
        let model = build_model(&inputs("named-road"));
        let road = named_road(&model, "generated.road-network.scene-0.4-11");
        assert_eq!(road["representation"], json!("savedOrientedRoadBedBlock"));
        assert_eq!(road["renderLayer"], json!("savedWorld3d"));
        assert_eq!(road["runsNo3dPrimitive"], json!(false));
        assert_eq!(road["association"], json!("exactIdentityMatch"));
        assert_eq!(road["associatedRoadEntityId"], json!("generated.road-network.scene-0.4-11"));
        assert_eq!(road["lengthFactSource"], json!("savedRoad.lengthM"));
        assert_eq!(road["widthFactSource"], json!("savedRoadEntity.depthM"));
        assert_eq!(road["heightFactSource"], json!("savedRoadEntity.heightM"));
        assert_eq!(road["worldYFactSource"], json!("savedRoadEntity.worldY"));
        assert_eq!(road["lengthAgreesWithSavedEntityWidth"], json!(true));
        assert_eq!(road["widthFactMissing"], json!(false));
        assert_eq!(road["lengthM"], json!(64.0));
        assert_eq!(road["widthM"], json!(8.0));
        assert_eq!(road["rotationYDeg"], json!(90.0));
        let aabb = geom::corners_aabb(&corner_list(&road["cornersMetres"]).expect("corners")).expect("aabb");
        for (computed, expected) in [(aabb[0], 284.0), (aabb[1], 32.0), (aabb[2], 292.0), (aabb[3], 96.0)] {
            assert!((computed - expected).abs() < 1e-9, "the road bed must reproduce the saved bounds: {computed} vs {expected}");
        }
        assert!((aabb[4] - 1.0).abs() < 1e-12 && (aabb[5] - 1.24).abs() < 1e-12);
        let band = corner_list(&road["bandCornersMetres"]).expect("band");
        assert_eq!(band.len(), 4);
        assert!(band.iter().all(|corner| (corner[1] - 1.24).abs() < 1e-12), "the band lies on the saved roadbed top");
    }

    /// Every saved entity of the real context reproduces its own saved world AABB
    /// through the authoritative transform, at rotations 0, 90 and 270 degrees.
    #[test]
    fn every_saved_entity_solid_reproduces_its_saved_world_aabb() {
        let model = build_model(&inputs("entity-aabbs"));
        let entities = model["entities"].as_array().expect("entities");
        assert_eq!(entities.len(), 4);
        let mut rotations: Vec<f64> = Vec::new();
        for entity in entities {
            let corners = corner_list(&entity["cornersMetres"]).expect("corners");
            let aabb = geom::corners_aabb(&corners).expect("aabb");
            let saved = &entity["savedBounds"];
            for (computed, expected) in [
                (aabb[0], saved["minX"].as_f64().expect("minX")),
                (aabb[1], saved["minZ"].as_f64().expect("minZ")),
                (aabb[2], saved["maxX"].as_f64().expect("maxX")),
                (aabb[3], saved["maxZ"].as_f64().expect("maxZ")),
            ] {
                assert!((computed - expected).abs() < 1e-9, "entity {} must reproduce its saved AABB: {computed} vs {expected}", entity["entityId"]);
            }
            rotations.push(entity["rotationYDeg"].as_f64().expect("rotation"));
        }
        rotations.sort_by(|left, right| left.partial_cmp(right).expect("finite"));
        assert_eq!(rotations, vec![0.0, 90.0, 90.0, 270.0], "the real context covers 0, 90 and 270 degrees");
    }

    /// §4.7(2): non-zero and negative origins must transform without a hidden
    /// shift, and a rotated block must stay centred on its saved world position.
    #[test]
    fn non_zero_and_negative_origins_transform_without_a_hidden_shift() {
        let corners = geom::oriented_box(-7.0, -2.0, -3.0, 64.0, 8.0, 0.24, 90.0, "bottom", 0.0)
            .expect("box").0;
        let aabb = geom::corners_aabb(&corners).expect("aabb");
        for (computed, expected) in [(aabb[0], -11.0), (aabb[1], -35.0), (aabb[2], -3.0), (aabb[3], 29.0)] {
            assert!((computed - expected).abs() < 1e-9, "negative origin transform: {computed} vs {expected}");
        }
        assert!((aabb[4] + 2.0).abs() < 1e-12 && (aabb[5] + 1.76).abs() < 1e-12);
        let corners = geom::oriented_box(-7.0, 0.0, -3.0, 10.0, 10.0, 1.0, 45.0, "bottom", 0.0)
            .expect("box").0;
        let aabb = geom::corners_aabb(&corners).expect("aabb");
        assert!(((aabb[0] + aabb[2]) / 2.0 + 7.0).abs() < 1e-9, "a 45 degree block stays centred on the saved world X");
        assert!(((aabb[1] + aabb[3]) / 2.0 + 3.0).abs() < 1e-9, "a 45 degree block stays centred on the saved world Z");
        assert!((aabb[2] - aabb[0] - 10.0 * 2.0_f64.sqrt()).abs() < 1e-9, "a 45 degree square spans its diagonal");
        // A missing or unknown pivot must never be inferred.
        assert!(geom::oriented_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, "middle", 0.0).is_none());
        assert!(geom::vertical_span(0.0, 0.0, "bottom", 0.0).is_none(), "a zero height is not a vertical fact");
    }

    /// §4.7(3): a hidden layer contributes no triangle and can never be hit.
    #[test]
    fn hidden_layers_are_never_hit() {
        let corners = geom::oriented_box(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 0.0, "bottom", 0.0)
            .expect("box").0;
        let entity_triangles = geom::triangles_of(&corners, &geom::BOX_TRIANGLES);
        let terrain: Vec<([f64; 3], [f64; 3], [f64; 3])> =
            vec![([-5.0, -6.0, -5.0], [5.0, -6.0, -5.0], [0.0, -6.0, 5.0])];
        let origin = [2.5, 20.0, 0.0];
        let dir = [0.0, -1.0, 0.0];
        let visible = geom::pick(origin, dir, &entity_triangles, &[], &terrain).expect("a visible entity must be hit");
        assert!(matches!(visible.label, geom::HitLabel::Entity(_)), "the entity layer must win when it is visible");
        assert!((visible.point[1] - 10.0).abs() < 1e-9, "the hit must be on the drawn top face, not on the AABB");
        let hidden = geom::pick(origin, dir, &[], &[], &terrain).expect("the terrain must still be hit");
        assert!(matches!(hidden.label, geom::HitLabel::Terrain { .. }));
        assert!((hidden.point[1] + 6.0).abs() < 1e-9);
    }

    /// §4.7(4): the window samples x256..319 / z32..95 exactly, and no surface is
    /// invented outside the saved window.
    #[test]
    fn the_window_covers_exactly_x256_to_319_and_z32_to_95() {
        let model = build_model(&inputs("bounds"));
        let width = number(&model["window"], "widthM");
        let depth = number(&model["window"], "depthM");
        for surface in ["natural", "final"] {
            let vertices: Vec<f64> = model["surfaces"][surface]["verticesMetres"].as_array().expect("vertices").iter().filter_map(Value::as_f64).collect();
            let triangles = geom::terrain_triangles_from(&vertices, width, depth);
            assert_eq!(triangles.len(), ((width - 1) * (depth - 1) * 2) as usize);
            let mut limits = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
            for (a, b, c) in &triangles {
                for point in [a, b, c] {
                    limits[0] = limits[0].min(point[0]);
                    limits[1] = limits[1].min(point[2]);
                    limits[2] = limits[2].max(point[0]);
                    limits[3] = limits[3].max(point[2]);
                }
            }
            assert!((limits[0] - 256.0).abs() < 1e-9 && (limits[2] - 319.0).abs() < 1e-9, "samples stop at x=319, never 320");
            assert!((limits[1] - 32.0).abs() < 1e-9 && (limits[3] - 95.0).abs() < 1e-9, "samples stop at z=95, never 96");
            assert!(geom::pick([320.5, 500.0, 64.0], [0.0, -1.0, 0.0], &[], &[], &triangles).is_none(), "no sample may be invented outside the saved window");
            assert!(geom::pick([288.0, 500.0, 96.5], [0.0, -1.0, 0.0], &[], &[], &triangles).is_none(), "no sample may be invented outside the saved window");
        }
    }

    /// §4.7(5): degenerate triangles, invalid indices and non-finite inputs are
    /// refused instead of being hit or panicking.
    #[test]
    fn degenerate_triangles_and_invalid_indices_are_refused() {
        assert!(geom::ray_triangle([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).is_none(), "a zero-area triangle is not a surface");
        assert!(geom::ray_triangle([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], [f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "a non-finite vertex is refused");
        assert!(geom::ray_triangle([f64::NAN, 10.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "a non-finite ray is refused");
        assert!(geom::ray_triangle([0.0, -10.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "a hit behind the origin is refused");
        let vertices = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        let short = vec![0.0; 9];
        assert!(geom::terrain_triangle(&short, 2, 2, 1).is_none(), "an index beyond the vertex array is refused");
        assert!(geom::terrain_triangle(&vertices, 2, 2, 99).is_none(), "a triangle number outside the grid is refused");
        assert!(geom::terrain_triangle(&vertices, 1, 1, 0).is_none(), "a grid without a cell has no triangle");
        assert_eq!(geom::terrain_triangles_from(&vertices, 2, 2).len(), 2);
    }

    /// §4.7(6): on a 2x2 slope `[0, 1000, 0, 1000]` the plane is y = x, so a hit
    /// at x = .25/.75 must return 250/750 mm, never the nearest sample 0/1000 mm.
    #[test]
    fn a_2x2_slope_hits_the_real_plane_and_never_the_nearest_sample() {
        let vertices = vec![
            0.0, 0.0, 0.0,
            1.0, 1.0, 0.0,
            0.0, 0.0, 1.0,
            1.0, 1.0, 1.0,
        ];
        let triangles = geom::terrain_triangles_from(&vertices, 2, 2);
        assert_eq!(triangles.len(), 2);
        for query in [0.25_f64, 0.75] {
            let hit = geom::pick([query, 5.0, query], [0.0, -1.0, 0.0], &[], &[], &triangles).expect("the plane must be hit");
            assert!((hit.point[1] - query).abs() < 1e-12, "a y=x plane must be hit at y={query}, not at the nearest sample 0 or 1");
            assert!((hit.point[0] - query).abs() < 1e-12 && (hit.point[2] - query).abs() < 1e-12);
            assert!(matches!(hit.label, geom::HitLabel::Terrain { .. }));
        }
        // A ray that misses the cell entirely must not fall back to a sample.
        assert!(geom::pick([-3.0, 5.0, 0.5], [0.0, -1.0, 0.0], &[], &[], &triangles).is_none());
    }

    /// §4.7(7): the two triangles of one cell are not coplanar, the anti-diagonal
    /// split is locked, and barycentric interpolation reproduces the hit point.
    #[test]
    fn the_two_triangles_are_not_coplanar_and_barycentric_interpolation_is_locked() {
        let vertices = vec![
            0.0, 0.0, 0.0,   // a (x0,z0)  0 mm
            1.0, 0.0, 0.0,   // b (x1,z0)  0 mm
            0.0, 0.0, 1.0,   // c (x0,z1)  0 mm
            1.0, 1.0, 1.0,   // d (x1,z1)  1000 mm
        ];
        let first = geom::terrain_triangle(&vertices, 2, 2, 0).expect("first triangle");
        let second = geom::terrain_triangle(&vertices, 2, 2, 1).expect("second triangle");
        let normal = |t: ([f64; 3], [f64; 3], [f64; 3])| {
            let left = [t.1[0] - t.0[0], t.1[1] - t.0[1], t.1[2] - t.0[2]];
            let right = [t.2[0] - t.0[0], t.2[1] - t.0[1], t.2[2] - t.0[2]];
            [
                left[1] * right[2] - left[2] * right[1],
                left[2] * right[0] - left[0] * right[2],
                left[0] * right[1] - left[1] * right[0],
            ]
        };
        let flat = normal(first);
        let slope = normal(second);
        assert!(flat[0].abs() < 1e-12 && flat[2].abs() < 1e-12, "the a-c-b triangle must be the flat one");
        assert!(slope[0].abs() > 1e-12 || slope[2].abs() > 1e-12, "the b-c-d triangle must be the sloped one");
        let normalize = |vector: [f64; 3]| {
            let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
            [vector[0] / length, vector[1] / length, vector[2] / length]
        };
        let (unit_flat, unit_slope) = (normalize(flat), normalize(slope));
        let dot = unit_flat[0] * unit_slope[0] + unit_flat[1] * unit_slope[1] + unit_flat[2] * unit_slope[2];
        assert!(dot.abs() < 0.999, "the two triangles of the cell must not be coplanar");
        let triangles = geom::terrain_triangles_from(&vertices, 2, 2);
        let lower = geom::pick([0.25, 5.0, 0.25], [0.0, -1.0, 0.0], &[], &[], &triangles).expect("the flat side must be hit");
        assert!((lower.point[1]).abs() < 1e-12, "x + z < 1 is the flat triangle at y = 0");
        let upper = geom::pick([0.75, 5.0, 0.75], [0.0, -1.0, 0.0], &[], &[], &triangles).expect("the sloped side must be hit");
        assert!((upper.point[1] - 0.5).abs() < 1e-12, "x + z > 1 must slope to y = 0.5 m at x = z = 0.75");
        let triangle_index = match upper.label {
            geom::HitLabel::Terrain { triangle_index, cell_index } => {
                assert_eq!(cell_index, 0);
                triangle_index
            }
            other => panic!("unexpected hit label {other:?}"),
        };
        assert_eq!(triangle_index, 1, "the split must stay on the b-c anti-diagonal");
        let weights = upper.barycentric;
        assert!((weights[0] + weights[1] + weights[2] - 1.0).abs() < 1e-9, "barycentric weights must sum to one");
        assert!(weights.iter().all(|weight| (-1e-9..=1.0 + 1e-9).contains(weight)), "the hit must lie inside the triangle");
        let triangle = geom::terrain_triangle(&vertices, 2, 2, triangle_index).expect("triangle");
        let reconstructed = [
            weights[0] * triangle.0[0] + weights[1] * triangle.1[0] + weights[2] * triangle.2[0],
            weights[0] * triangle.0[1] + weights[1] * triangle.1[1] + weights[2] * triangle.2[1],
            weights[0] * triangle.0[2] + weights[1] * triangle.1[2] + weights[2] * triangle.2[2],
        ];
        for axis in 0..3 {
            assert!((reconstructed[axis] - upper.point[axis]).abs() < 1e-9, "barycentric interpolation must reproduce the hit point");
        }
    }

    #[test]
    fn the_viewer_uses_no_constant_world_y_and_picks_real_triangles() {
        assert!(!VIEWER_JS.contains("path[j].x, 0.6"), "access must not be drawn at a constant world Y");
        assert!(!VIEWER_JS.contains("0.7, model.nodes[n].worldZ"), "nodes must not be drawn at a constant world Y");
        assert!(!VIEWER_JS.contains("0.8, node.worldZ"), "node points must not be drawn at a constant world Y");
        assert!(!VIEWER_JS.contains("gl.LINES") && !VIEWER_JS.contains("gl.LINE_STRIP"), "solids must be drawn as faces, not as wireframes");
        assert!(!VIEWER_JS.contains("rectangleWire"), "the AABB wireframe must be gone");
        assert!(!VIEWER_JS.contains("surfaceHeightAt"), "the nearest-sample surface lookup must be gone");
        assert!(VIEWER_JS.contains("rayTriangle"), "picking must use real indexed-triangle intersection");
        assert!(VIEWER_JS.contains("surfacePickData"), "the pick set must come from the drawn index buffer");
        assert!(VIEWER_JS.contains("notSavedMillimetreFact"), "a derived hit coordinate must never be written back as a saved fact");
        assert!(VIEWER_JS.contains("constantWorldYUsed"), "the viewer must publish the no-constant-Y status");
    }

    // -----------------------------------------------------------------------
    // G28 experience tests: selection feedback, split counts, camera, materials
    // -----------------------------------------------------------------------

    /// The selection panel, the highlight and the published state must be fed by
    /// the same selection object, and the panel must show the approved entity
    /// fields with saved facts and derived values labelled apart.
    #[test]
    fn the_selection_panel_is_fed_by_the_same_object_as_the_highlight() {
        let html = html_for("{}");
        for id in ["selection", "counts", "layer-info", "evidence", "limitations", "overlay", "camera-bar"] {
            assert!(html.contains(&format!("id=\"{id}\"")), "the page must provide #{id}");
        }
        for id in ["cam-default", "cam-top", "cam-focus", "cam-rotate-left", "cam-rotate-right", "cam-zoom-in", "cam-zoom-out", "cam-reset", "cam-clear", "hud-toggle", "showObservation"] {
            assert!(html.contains(&format!("id=\"{id}\"")), "the page must provide the #{id} control");
        }
        for field in ["entityId", "kind", "assetId", "worldXYZ (saved)", "rotationYDeg (saved)", "scale (saved)", "savedBounds (saved world AABB)"] {
            assert!(VIEWER_JS.contains(field), "the selection panel must show {field}");
        }
        assert!(VIEWER_JS.contains("state.selection = { kind: \"entity\", label: \"entity solid\", facts: state.selectedFacts"));
        assert!(VIEWER_JS.contains("state.selectionIsSameObjectAsHighlight = true"), "the panel, the highlight and the state must reference one object");
        assert!(VIEWER_JS.contains("nothing selected - click an entity solid"), "clearing must restore an explicit empty state");
        assert!(VIEWER_JS.contains("class=\"derived\""), "derived display values must be labelled apart from saved facts");
        assert!(VIEWER_JS.contains("renderSelection();"), "the selection panel must be re-rendered on every pick");
        // The four required no-height / not-positioned / not-snapped lines are
        // rendered per category, never as a single combined figure.
        assert!(VIEWER_JS.contains("no saved height: entities "));
        assert!(VIEWER_JS.contains("not positioned in 3D: entities "));
        assert!(VIEWER_JS.contains("not snapped to a surface: entities "));
        assert!(VIEWER_JS.contains("total = "), "the observation2d total must be shown with its nodes/access split");
    }

    /// Camera presets, pointer interaction and the collapsible HUD must exist, and
    /// the HUD must be hideable so it cannot cover the map.
    #[test]
    fn the_viewer_offers_camera_presets_and_a_collapsible_hud() {
        for preset in ["\"default\"", "\"top\"", "\"focus\"", "\"oblique\""] {
            assert!(VIEWER_JS.contains(preset), "camera preset {preset} must exist");
        }
        assert!(VIEWER_JS.contains("applyCameraPreset"), "camera presets must be applied through one function");
        assert!(VIEWER_JS.contains("pointerdown") && VIEWER_JS.contains("wheel"), "rotate and zoom must come from real pointer input");
        assert!(VIEWER_JS.contains("cameraSnapshot"), "the camera state must be published");
        assert!(VIEWER_JS.contains("cameraPresetLog"), "camera preset changes must be observable");
        assert!(VIEWER_JS.contains("setHudCollapsed"), "the facts panel must be collapsible");
        assert!(html_for("{}").contains("body.hud-collapsed #hud { display:none; }"), "collapsing must really free the map area");
        assert!(VIEWER_JS.contains("resizeCanvas"), "the canvas must follow the stage size");
    }

    /// G29: the page boots map-first. The default layout must be browse mode, no
    /// panel may float above the map (nothing may cover the canvas the driver
    /// measures), every fact the inspect mode shows must already exist in the
    /// page, and the mode switch must be a layout change in the viewer instead of
    /// a second scene. The numeric 75 % map-share claim is measured by the
    /// acceptance driver in a real browser; this test only guards the structure it
    /// depends on, plus the JS wiring that makes the switch possible.
    #[test]
    fn the_page_boots_map_first_and_keeps_every_fact_reachable() {
        let html = html_for("{}");
        assert!(html.contains("<body class=\"view-browse hud-collapsed\">"), "the page must boot in browse mode");
        assert!(html.contains("body.view-browse #hud { display:none; }"), "browse mode must free the map area");
        assert!(html.contains("id=\"mode-browse\"") && html.contains("id=\"mode-inspect\""), "both display modes need a real control");
        assert!(html.contains("aria-pressed=\"true\""), "the active mode must be announced");
        assert!(html.contains("id=\"selection-strip\""), "browse mode needs one concise selection card");
        assert!(html.contains("id=\"topbar\"") && html.contains("flex-wrap:wrap"), "the toolbar must wrap instead of overflowing");
        assert!(html.contains("body { display:flex; flex-direction:column; }"), "map, toolbar and card must be flex rows, not overlays");
        assert!(!html.contains("z-index"), "no chrome may float above the map");
        assert!(!html.contains("http://") && !html.contains("https://"), "the page must stay offline and CDN-free");
        // every fact the inspect mode must keep
        for id in ["gl-info", "counts", "layer-info", "evidence", "limitations", "load-status", "load-error", "selection", "hud-toggle", "showEntities", "showRoads"] {
            assert!(html.contains(&format!("id=\"{id}\"")), "the page must always provide #{id}");
        }
        assert!(html.contains("independent-3d-readonly-consumer") && html.contains("只读3D预览"), "the read-only notice must stay visible in both modes");
        for label in ["显示模式", "地形层", "图层", "视角"] {
            assert!(html.contains(label), "the toolbar must carry the readable label {label}");
        }
        // the switch itself: layout only, shared state, no second scene
        assert!(VIEWER_JS.contains("viewMode: \"browse\""), "browse must be the default mode");
        assert!(VIEWER_JS.contains("function applyViewMode()") && VIEWER_JS.contains("function syncLayout()") && VIEWER_JS.contains("function measureLayout()"));
        assert!(VIEWER_JS.contains("function setViewMode(mode)"));
        assert!(VIEWER_JS.contains("viewModeSnapshot()"), "the mode switch must publish what it changed");
        assert!(VIEWER_JS.contains("state.modeSwitches++"), "mode switches must be countable");
        assert!(VIEWER_JS.contains("mapWidthFraction"), "the map share must be measured from the real layout box");
        assert!(VIEWER_JS.contains("bind(\"mode-browse\", function () { setViewMode(\"browse\"); })"));
        assert!(VIEWER_JS.contains("bind(\"mode-inspect\", function () { setViewMode(\"inspect\"); })"));
        assert!(VIEWER_JS.contains("selection-details"), "the full saved fields must open behind one control");
        assert!(VIEWER_JS.contains("非保存样本"), "a terrain card must mark derived values as not saved samples");
        assert!(VIEWER_JS.contains("state.selectionDetailsOpen"), "the card expansion must live on the state, not on the layout");
        // a mode switch must not be able to reset the camera, the surface or the selection
        assert!(!VIEWER_JS.contains("function setViewMode(mode) {\n    applyCameraPreset"), "switching modes must never reset the camera");
        assert!(VIEWER_JS.contains("state.pageLoads = (state.pageLoads || 0) + 1;"), "a model reload inside one page must be observable");
    }

    /// G32-R2: the layout closeout compresses the navigation surface in normal flow so the
    /// real map rectangle can reach the 75% (960x640) and 80% (1280x800) area floors. This
    /// guard is static only: it proves the compact spacing is present and that every
    /// control the checklist names still exists in the page. The area floors themselves
    /// are measured from real DOM rectangles in the one browser session, never here.
    #[test]
    fn the_layout_closeout_compresses_the_toolbar_without_removing_a_control() {
        let html = html_for("{}");
        // the compact rules are the ones written in layout-closeout-contract.zh-CN.md
        assert!(html.contains("#topbar { flex:0 0 auto; display:flex; flex-wrap:wrap; align-items:center; gap:2px 6px; padding:3px 8px;"), "the toolbar must use the compact padding and gap");
        assert!(html.contains(".group { display:flex; flex-wrap:wrap; align-items:center; gap:2px 5px; padding:1px 5px;"), "the control groups must use the compact padding and gap");
        assert!(html.contains("#topbar button, #hud-toggle { font:10px/1.15 ui-monospace,Consolas,monospace;"), "the buttons must use the compact font and line height");
        assert!(html.contains(".glabel { color:#8fa1b3; font-size:10px;"), "the group labels must use the compact font size");
        assert!(html.contains("#topbar label { display:inline-flex; align-items:center; gap:3px; cursor:pointer; white-space:nowrap; font-size:10px; }"), "the layer labels must use the compact font size");
        assert!(html.contains("#reading-bar { flex:0 0 auto; display:flex; flex-wrap:wrap; align-items:center; gap:2px 8px; padding:1px 6px;"), "the summary must keep its flex row with a compact padding");
        assert!(html.contains("#reading-bar .legend-row { display:flex; flex-wrap:nowrap;"), "the summary must keep its single no-wrap line");
        // no control may be dropped, hidden or renamed to buy area
        for id in [
            "mode-browse", "mode-inspect", "showEntities", "showRoads", "showObservation",
            "cam-default", "cam-top", "cam-focus", "cam-reset", "cam-clear",
            "cam-oblique", "cam-rotate-left", "cam-rotate-right", "cam-zoom-in", "cam-zoom-out",
            "more-camera", "reading-details-open", "hud-toggle",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")), "the layout closeout must keep #{id}");
        }
        for label in ["显示模式", "地形层", "图层", "视角", "更多视角", "读图详情", "只读3D预览"] {
            assert!(html.contains(label), "the layout closeout must keep the readable label {label}");
        }
        // compacting must never become hiding, clipping or floating above the map
        assert!(!html.contains("display:none; } /* compact"), "no compact rule may hide a control");
        assert!(!html.contains("overflow:hidden"), "area must not be bought by clipping content");
        assert!(!html.contains("position:sticky") && !html.contains("position:fixed"), "no toolbar may float above the map");
        assert!(!html.contains("z-index"), "no chrome may float above the map");
        // the frozen viewer is untouched by this round
        assert!(VIEWER_JS.contains("if (state.pointerGesture || event.button !== 0) { return; }"), "the repaired pointer gate must stay exactly as G32-R1 left it");
        assert!(VIEWER_JS.contains("if (gesture.mode === \"pan\" && gesture.panApplied === true) { state.panCount++; }"), "the pan counter semantics must stay exactly as G32-R1 left them");
    }

    /// G33: the navigation overview is a fixed-size, read-only svg box pinned to the
    /// stage corner. It must not become a second WebGL canvas, must not float above the
    /// map with a z-index or a fixed position, and must stay off the layout flow so the
    /// G32 area floors cannot move. The frozen pan gate and the pan counter semantics
    /// must survive the change, and the overview must expose pure mappings a driver can
    /// check against its own independent expectation.
    #[test]
    fn the_overview_minimap_is_a_fixed_readonly_svg_off_the_layout_flow() {
        let html = html_for("{}");
        assert!(html.contains("<svg id=\"minimap\""), "the overview must be an svg element");
        assert!(
            html.contains("#minimap { position:absolute; right:10px; bottom:10px; width:140px; height:140px;"),
            "the overview must be a fixed 140x140 box pinned to the stage corner"
        );
        for id in ["minimap-window", "minimap-footprint", "minimap-target", "minimap-north"] {
            assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "the overview must declare #{id} exactly once");
        }
        // an overlay of fixed size, never a second drawing surface and never a layer trick
        assert_eq!(html.matches("<canvas").count(), 2, "the page must keep exactly its two existing canvases");
        assert!(!html.contains("z-index"), "the overview must not float above the map with a z-index");
        assert!(!html.contains("position:fixed") && !html.contains("position:sticky"), "the overview must not be pinned to the viewport");
        // the overview is off the layout flow, so the G32 area floors cannot move
        assert!(html.contains("right:10px; bottom:10px;"), "the overview is anchored, not a flex row");
        // the frozen gesture gate and the pan counters survive the G33 change
        assert!(VIEWER_JS.contains("if (state.pointerGesture || event.button !== 0) { return; }"), "the repaired pointer gate must stay exactly as G32-R1 left it");
        assert!(VIEWER_JS.contains("if (gesture.mode === \"pan\" && gesture.panApplied === true) { state.panCount++; }"), "the pan counter semantics must stay exactly as G32-R1 left them");
        // the overview exposes pure functions and a read-only snapshot
        assert!(VIEWER_JS.contains("function minimapWorldToPx("), "the overview must expose a pure world-to-pixel mapping");
        assert!(VIEWER_JS.contains("function minimapPxToWorld("), "the overview must expose the inverse mapping");
        assert!(VIEWER_JS.contains("function minimapFootprint("), "the overview must derive its footprint from the same projection");
        assert!(VIEWER_JS.contains("function minimapTargetFromPx("), "the overview must expose a pure pixel-to-target mapping");
        assert!(VIEWER_JS.contains("window.__minimap3d"), "the overview must publish a read-only snapshot for independent comparison");
        assert!(VIEWER_JS.contains("minimapTargetFromPx(model.window, MINIMAP_SIZE, px, py, state.camera.target)"), "the overview must reuse the pure target mapping when it applies a jump");
        // G33-R1: north is up, the inverse mapping matches it, and invalid spans are refused
        // instead of being replaced by 1
        assert!(VIEWER_JS.contains("var py = ((z - win.minZ) / win.depthM) * size;"), "a larger z (south) must give a larger svg y: north stays up");
        assert!(VIEWER_JS.contains("var z = win.minZ + (py / size) * win.depthM;"), "the inverse mapping must use the same north-up convention");
        assert!(VIEWER_JS.contains("function minimapWindowValid(win)"), "the overview must validate the saved window before projecting");
        assert!(VIEWER_JS.contains("return win.widthM > 0 && win.depthM > 0;"), "a non-positive window span must be refused, never replaced by 1");
        assert!(VIEWER_JS.contains("var world = minimapPxToWorld(win, size, clamp(px, 0, size), clamp(py, 0, size));"), "interactive input must be clamped into the window while the projection stays untruncated");
        // G33-R1: the reference-plane footprint is a real half-space clipping, not a 60x point
        assert!(VIEWER_JS.contains("function minimapClipHalfSpace(poly, f)"), "the footprint must be produced by an explicit half-space clipper");
        assert!(VIEWER_JS.contains("minimapClipHalfSpace(poly, function (pt) { return 2000 - dot3(forward,"), "the footprint must be cut by the projection's own far plane");
        assert!(!VIEWER_JS.contains("60 * Math.max(win.widthM"), "the 60x virtual reach must be gone");
        assert!(VIEWER_JS.contains("if (!(eye[1] > planeY)) {"), "a plane at or behind the eye level must be reported empty, not filled");
        assert!(VIEWER_JS.contains("status: \"empty\""), "the empty intersection must be an explicit status");
        // G33-R1: one active pointer owns the overview drag
        assert!(VIEWER_JS.contains("function minimapOwns(event)"), "the overview must check pointer ownership on every event");
        assert!(VIEWER_JS.contains("if (state.minimapDrag) { return; }"), "a second pointer down must be ignored while the overview is dragging");
        assert!(VIEWER_JS.contains("if (!minimapOwns(event)) { return; }            // a foreign pointer can never end it"), "a foreign pointer up must not end the overview drag");
        assert!(VIEWER_JS.contains("endMinimapDrag(\"pointerup\");                    // clear the state FIRST"), "the drag state must be cleared before the capture is released");
        // G33-R2: clippedToWindow must be a measured change, not a constant. It may not be `true`
        // whenever the result is non-empty, the per-side change must be tracked by coordinate and
        // not by array length, and a fully contained frustum polygon must report false.
        assert!(!VIEWER_JS.contains("clippedToWindow: true }"), "clippedToWindow must never be returned as a constant true");
        assert!(VIEWER_JS.contains("var clipChanged = false;"), "the window clip must start from an unclipped state");
        assert!(VIEWER_JS.contains("if (next.length !== windowPoly.length) { clipChanged = true; }"), "a side clip that changes the vertex count is a change");
        assert!(VIEWER_JS.contains("if (!match) { clipChanged = true; }"), "a side clip that rewrites a coordinate must be detected even when the vertex count survives");
        assert!(VIEWER_JS.contains("var allInside = true;"), "the flag must be cross-checked against the geometric containment predicate");
        assert!(VIEWER_JS.contains("if (allInside) { clipChanged = false; }"), "a frustum polygon fully inside the window must report clippedToWindow=false");
        assert!(VIEWER_JS.contains("clippedToWindow: clipChanged === true, frustumInsideWindow: allInside === true"), "the footprint must publish both the measured flag and the containment predicate");
        assert!(VIEWER_JS.contains("clippedToWindow: false, reason: \"the visible ground region does not meet the saved window\""), "an empty result must not fabricate clippedToWindow=true");
    }

    /// G34: the overview gains a terrain-occlusion layer that reports visible/hidden/undefined on a
    /// fixed sampling grid. The layer is ADDITIVE: the G33 footprint, the target dot, the window
    /// frame and every navigation semantic stay exactly where they were, and the occluded state is
    /// carried by text as well as by colour.
    #[test]
    fn the_overview_occlusion_layer_is_additive_honest_and_not_colour_only() {
        let html = html_for("{}");
        // the layer and its accessibility channel exist exactly once
        for id in ["minimap-hidden", "minimap-visibility-status"] {
            assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "the occlusion layer must declare #{id} exactly once");
        }
        assert!(html.contains("<g id=\"minimap-footprint-clip-group\" display=\"none\" clip-path=\"url(#minimap-footprint-clip)\">"), "the occlusion layer must sit inside a footprint clip group that starts hidden");
        assert!(html.contains("<g id=\"minimap-hidden\" display=\"none\" data-hidden-cells=\"0\" clip-path=\"url(#minimap-window-clip)\">"), "the inner group must be clipped to the window, so the drawn area is cellRect ∩ footprint ∩ window");
        assert!(html.contains(".minimap-hidden-cell { fill:#c2803a; fill-opacity:0.42; stroke:none; }"), "hidden cells need their own low-luminance style");
        // G34-R1: the short status is INSIDE the 140x140 box and never clips its own text
        assert!(html.contains("#minimap-visibility-status { position:absolute; right:10px; bottom:10px; width:140px;"), "the short status must sit inside the minimap box at the same right/bottom offsets");
        // G34-R3: TWO clipPaths, each holding exactly ONE shape. A clipPath containing a polygon AND a
        // rect is their UNION, not their intersection, which is what the previous revision shipped and
        // what the inspector measured red pixels outside the triangle for.
        assert!(html.contains("<clipPath id=\"minimap-footprint-clip\" clipPathUnits=\"userSpaceOnUse\">"), "the footprint clip must be its own clipPath in user space");
        assert!(html.contains("<clipPath id=\"minimap-window-clip\" clipPathUnits=\"userSpaceOnUse\">"), "the window clip must be its own clipPath in user space");
        assert!(html.contains("<polygon id=\"minimap-domain-clip-poly\" points=\"\"></polygon>"), "the footprint clip must hold a POLYGON built from the footprint");
        assert!(html.contains("<rect id=\"minimap-domain-clip-window\""), "the window clip must hold the window rectangle");
        let poly_clip = html.find("<clipPath id=\"minimap-footprint-clip\"").expect("footprint clip");
        let window_clip = html.find("<clipPath id=\"minimap-window-clip\"").expect("window clip");
        assert!(window_clip > poly_clip, "both clips must be declared");
        // each clipPath must contain EXACTLY ONE shape element
        for (id, _name) in [("minimap-footprint-clip", "footprint"), ("minimap-window-clip", "window")] {
            let at = html.find(&format!("<clipPath id=\"{id}\"")).expect("clipPath");
            let end = html[at..].find("</clipPath>").expect("clipPath end");
            let body = &html[at..at + end];
            let shapes = body.matches("<polygon").count() + body.matches("<rect").count();
            assert_eq!(shapes, 1, "clipPath #{id} must contain exactly ONE shape; two shapes would union, not intersect");
        }
        assert!(VIEWER_JS.contains("minimap-domain-clip-poly") && VIEWER_JS.contains("minimap-domain-clip-window"), "the viewer must maintain both clip members");
        assert!(VIEWER_JS.contains("minimap-footprint-clip-group"), "the viewer must drive the footprint clip group's visibility");
        // the half-cell INSET of the clip window (size minus a cell) must be gone; the rect centring
        // offset (minus half a cell) is a different thing and is still required
        assert!(!VIEWER_JS.contains("size - stepXpx") && !VIEWER_JS.contains("size - stepZpx"), "the half-cell inset that had no contract basis must be gone, not merely renamed");
        assert!(VIEWER_JS.contains("clipWindow.setAttribute(\"width\", String(round6(Math.abs(wr.px - wl.px))))"), "the window clip must be the window's own rectangle, with no inset");
        assert!(VIEWER_JS.contains("minimapPointInPolygon(fp, world.x, world.z)"), "the cell's own domain membership must still be tested");
        // G34-R4: the ATOMIC visibility snapshot. One render builds ONE frozen object holding the
        // aggregate counts, the full 256-cell list and the set actually drawn, so a count and a cell
        // list can no longer come from different evaluations and be called a contradiction.
        for needle in [
            "g34-visibility-snapshot-v1",
            "var visibilitySnapshot = Object.freeze({",
            "state.minimapVisibility = visibilitySnapshot;",
            "visibilitySnapshotSerial++",
            "Object.freeze(snapCells)",
            "Object.freeze(drawnSorted)",
            "hiddenEqualsCells:",
            "drawnEqualsHiddenCells:",
            "selfConsistent: true",
            "snapshotConsistency.selfConsistent = false",
            "function visibilityFootprintHash(fp)",
            "dpr: window.devicePixelRatio",
            "structure: \"two-nested-single-shape-clipPaths\"",
        ] {
            assert!(VIEWER_JS.contains(needle), "the atomic snapshot must publish {needle}");
        }
        assert!(VIEWER_JS.contains("window.__minimapVisibility3d = function () { return state.minimapVisibility; };"), "the visibility hook must return the frozen snapshot itself, not reassemble it");
        assert!(!VIEWER_JS.contains("state.minimapVisibility = {\n      status:"), "the old hand-assembled visibility object must be gone");
        // G34-R4: a leftover reference to a REMOVED variable shipped once and only failed at runtime in
        // the browser, where the string assertions above could not see it. Any identifier the snapshot
        // area mentions must be declared somewhere.
        for name in ["snapshotOk", "drawnSortedCells", "snapCategoryTotal"] {
            let declared = VIEWER_JS.contains(&format!("var {name} =")) || VIEWER_JS.contains(&format!("let {name} =")) || VIEWER_JS.contains(&format!("{} =", name));
            assert!(!VIEWER_JS.contains(name) || declared || VIEWER_JS.contains(&format!("{} = false", name)), "the viewer references {name} but never declares it");
        }
        // the check above must not be vacuous: it has to be able to see an undeclared identifier, so
        // this synthetic case is asserted to fail the same predicate
        {
            let synthetic = "snapshotConsistency.selfConsistent = snapshotOkNotDeclared;";
            let name = "snapshotOkNotDeclared";
            let declared = synthetic.contains(&format!("var {name} =")) || synthetic.contains(&format!("let {name} =")) || synthetic.contains(&format!("{} =", name));
            let would_pass = !synthetic.contains(name) || declared;
            assert!(synthetic.contains(name) && !would_pass, "the dead-reference predicate must reject an undeclared identifier");
        }
        // the independent diagnostic hook must be LABELLED as such, so it cannot be quoted as the frame
        assert!(VIEWER_JS.contains("g.independent = true"), "the grid hook must be marked as an independent diagnostic, not the rendered frame");
        // the six mutually exclusive categories, the independent unknown counter, and the state words
        for needle in ["unknownCount", "categoryIdentityHolds", "outsideDomain", "outsideView", "numericalDegenerate", "outside-domain", "outside-view", "unknown-data", "numerical-degenerate", "partialKnowledge"] {
            assert!(VIEWER_JS.contains(needle), "the viewer must publish the category {needle}");
        }
        assert!(VIEWER_JS.contains("base.unknownCount = base.unknownData;"), "unknownCount must be its own quantity, not the undefined total");
        assert!(VIEWER_JS.contains("base.categorySum === base.sampleCount"), "the six mutually exclusive categories must be asserted to sum to the sample count");
        for attr in ["data-status", "data-visible", "data-hidden", "data-undefined", "data-outside-domain", "data-outside-view", "data-unknown-data", "data-numerical-degenerate", "data-eligible", "data-knowledge-complete", "data-resolution", "data-reason", "data-reference-mode"] {
            assert!(html.contains(attr), "the status element must publish {attr}");
        }
        // the visible short status must state the sampling restriction, not only encode it in data
        // attributes: the text is written by the viewer at runtime, so assert it on VIEWER_JS
        assert!(VIEWER_JS.contains("地表采样") && VIEWER_JS.contains("仅地形"), "the visible status must name the sampling method and the terrain-only limit");
        assert!(VIEWER_JS.contains("data-outside-view") && VIEWER_JS.contains("partialKnowledge"), "the viewer must publish the decomposed reasons and the partial-knowledge state");
        let status_rule = html.split('\n').find(|l| l.contains("#minimap-visibility-status")).expect("status rule");
        assert!(!status_rule.contains("overflow") && !status_rule.contains("text-overflow"), "the short status must not clip its own text to fit");
        // the target dot must stay the LAST mark inside the box so no layer paints over it
        let g = html.find("<g id=\"minimap-hidden\"").expect("occlusion group");
        let dot = html.find("<circle id=\"minimap-target\"").expect("target dot");
        assert!(g < dot, "the occlusion group must be drawn before the target dot");
        // the box itself and the accepted G33 elements are untouched
        assert!(html.contains("#minimap { position:absolute; right:10px; bottom:10px; width:140px; height:140px;"), "the box must stay fixed at 140x140");
        assert!(!html.contains("z-index"), "the overview must not float above the map with a z-index");
        assert_eq!(html.matches("<canvas").count(), 2, "the page must keep exactly its two existing canvases");
        // the product side: a separate occlusion implementation, an explicit three-state result and a
        // published read-only snapshot, with the claim kept honest about being a sampling grid
        for needle in [
            "function occlusionIndexFor(",
            "function occlusionTerrainHeight(",
            "function occlusionClassify(",
            "function occlusionSegmentHitMetres(",
            "function occlusionClassifyDetailed(",
            "function occlusionInsideFrustum(",
            "function occlusionObserverIsDegenerate(",
            "function minimapVisibilityGrid(",
            "var MINIMAP_VISIBILITY_RESOLUTION = 16;",
            "referenceMode: \"terrain-surface\"",
            // G34-R1 unit contract: the metre window is derived from the same epsilon at both ends
            "var maxMetres = len - MINIMAP_VISIBILITY_EPS_M;",
            // G34-R2: the intersection test is allocation-free and keeps the metre window
            "function occlusionSegmentHitMetres(o, dir, minMetres, maxMetres, tri)",
            "if (!(s > minMetres) || !(s < maxMetres)) { return null; }",
            // G34-R2: the conservative pre-filter and the spatial walk
            "function occlusionBuildHeightBounds(index)",
            "function occlusionLineClearsTerrain(index, bounds, eye, dir, len)",
            "function occlusionCandidates(index, x0, z0, x1, z1)",
            "var MINIMAP_VISIBILITY_BUCKET_M = 4;",
        ] {
            assert!(VIEWER_JS.contains(needle), "the occlusion module must expose every contracted function; a missing one is a delivery blocker");
        }
        assert!(VIEWER_JS.contains("SAMPLING GRID over the saved terrain triangles, NOT per-pixel"), "the source must state that this is a sampling grid, not per-pixel visibility");
        assert!(VIEWER_JS.contains("status: \"occluded\"") || VIEWER_JS.contains("base.status = \"occluded\""), "the occluded state must be explicit");
        assert!(VIEWER_JS.contains("base.status = \"unobstructed\""), "the unobstructed state must be explicit");
        assert!(VIEWER_JS.contains("base.status = \"unavailable\""), "the unavailable state must be explicit");
        // G34-R1: partial knowledge is its own state, and outside-view is not "unknown"
        assert!(VIEWER_JS.contains("base.status = \"partialKnowledge\""), "the partial-knowledge state must be explicit rather than being reported as unobstructed");
        assert!(VIEWER_JS.contains("observer-below-or-on-terrain"), "a degenerate observer must be refused with an explicit reason");
        assert!(VIEWER_JS.contains("\"outside-view\""), "a surface point outside the frustum must be categorised, not counted as visible");
        assert!(VIEWER_JS.contains("window.__minimapVisibility3d"), "the visibility snapshot must be published read-only");
        assert!(VIEWER_JS.contains("window.__minimapVisibilityGrid3d"), "the pure grid function must be published for an independent oracle");
        assert!(VIEWER_JS.contains("window.__minimapOcclusionProbe3d"), "the per-cell probe must be published for an independent oracle");
        // the G33 semantics this round must not disturb
        assert!(VIEWER_JS.contains("var py = ((z - win.minZ) / win.depthM) * size;"), "north-up must stay");
        assert!(VIEWER_JS.contains("function minimapClipHalfSpace(poly, f)"), "the six-half-space footprint must stay");
        assert!(VIEWER_JS.contains("clippedToWindow: clipChanged === true, frustumInsideWindow: allInside === true"), "the clippedToWindow semantics must stay");
        assert!(VIEWER_JS.contains("function minimapOwns(event)"), "the pointer ownership rules must stay");
        assert!(!VIEWER_JS.contains("60 * Math.max(win.widthM"), "the removed 60x reach must stay removed");
    }

    /// G32-R1: the gesture lifecycle repair keeps `state.pointerGesture` as the ONE
    /// source of truth (the old closure `pointerStart` was written once and never reset,
    /// so every later pointerdown was refused), counts completed pan gestures in
    /// `panCount` and applied steps in `panMoves`, and leaves the projection formula,
    /// the triangle intersection and the default framing untouched.
    #[test]
    fn the_camera_pan_contract_is_wired_without_touching_the_projection_or_the_picking() {
        assert!(VIEWER_JS.contains("function panTargetFromDrag(camera, canvasSize, startTarget, dx, dy)"), "the pan must be one pure function");
        assert!(VIEWER_JS.contains("window.__panTarget3d = function"), "the pure pan must be published for an independent expectation");
        assert!(VIEWER_JS.contains("var PAN_FOV = 0.9;"), "the pan must reuse the projection FOV");
        assert!(VIEWER_JS.contains("var PAN_DRAG_THRESHOLD = 3;"), "the pan must reuse the existing 3px drag threshold");
        assert!(VIEWER_JS.contains("function panBasis(yaw, pitch)"), "the horizontal basis must come from yaw/pitch only");
        assert!(VIEWER_JS.contains("mode: event.shiftKey ? \"pan\" : \"rotate\""), "the mode is decided once, at pointerdown");
        assert!(VIEWER_JS.contains("if (state.pointerGesture || event.button !== 0) { return; }"), "one published gesture object is the only gate on a new pointerdown");
        assert!(!VIEWER_JS.contains("var pointerStart"), "no second, uncleared closure gesture state may exist");
        assert!(VIEWER_JS.contains("canvas.addEventListener(\"pointercancel\"") && VIEWER_JS.contains("canvas.addEventListener(\"lostpointercapture\"") && VIEWER_JS.contains("window.addEventListener(\"blur\""), "cancel, lost capture and blur must all clear the gesture");
        assert!(VIEWER_JS.contains("function endPointerGesture(reason, event)"), "one exit clears every cancel path");
        assert!(VIEWER_JS.contains("if (gesture.mode === \"pan\" && gesture.panApplied === true) { state.panCount++; }"), "panCount must count completed pan gestures, once per gesture");
        assert!(VIEWER_JS.contains("state.panMoves++;"), "panMoves must count applied pan steps");
        assert!(VIEWER_JS.contains("if (gesture.rejected !== true) { gesture.rejected = true; state.panRejections++; }"), "a rejected pan must be counted at most once per gesture");
        assert!(VIEWER_JS.contains("if (gesture && gesture.mode === \"pan\") { gesture.picked = false; return; }"), "a pan gesture must never pick");
        assert!(VIEWER_JS.contains("if (gesture.moved <= PAN_DRAG_THRESHOLD) { return; }"), "a sub-threshold shift drag is not a pan and is not a pick");
        assert!(VIEWER_JS.contains("startTarget[1],"), "the pan must copy target.y through unchanged");
        assert!(VIEWER_JS.contains("if (!target.every(finite)) { return null; }"), "a non-finite pan must be rejected, not applied");
        assert!(VIEWER_JS.contains("panCount: 0, panMoves: 0, lastPan: null, panRejections: 0"), "the pan facts must be published on the state");
        assert!(VIEWER_JS.contains("c.mode = \"oblique\"; c.yaw = 0.6; c.pitch = 0.7; c.distance = limits.span * 1.4; c.target = windowCentre();"), "the default preset must restore the saved window centre, not only the angles");
        assert!(VIEWER_JS.contains("bind(\"cam-reset\", function () { applyCameraPreset(\"default\"); })"), "复位视角 must fully restore the default framing");
        assert!(VIEWER_JS.contains("拖动旋转；Shift+拖动平移；滚轮缩放。"), "the navigation help must live in the existing details block");
        assert!(VIEWER_JS.contains("mat4Multiply(perspective(0.9, canvas.width / canvas.height, 0.5, 2000)"), "the projection formula must stay untouched");
        assert!(VIEWER_JS.contains("function rayTriangle(origin, dir, a, b, c)"), "the picking intersection must stay untouched");
        assert!(!VIEWER_JS.contains("terrainRayPlane") && !VIEWER_JS.contains("rayGroundPlane"), "the pan must not intersect a ground plane");
    }

    /// The experience layer must not weaken any honesty rule: observation records
    /// stay 2D, picking stays real triangle intersection, and no height is invented.
    #[test]
    fn the_experience_layer_keeps_the_g27b_honesty_rules() {
        assert!(VIEWER_JS.contains("OBSERVATION_LABEL"), "the 2D overlay must be explicitly labelled");
        assert!(VIEWER_JS.contains("2D observation overlay - saved x/z only"), "the overlay label must state that only saved x/z is used");
        assert!(VIEWER_JS.contains("getContext(\"2d\")"), "observation records must be drawn in a 2D overlay");
        assert!(!VIEWER_JS.contains("model.nodes[n].worldY") && !VIEWER_JS.contains("access[i].worldY"), "observation records must never gain a world Y");
        assert!(VIEWER_JS.contains("sourceObservationOnly") && VIEWER_JS.contains("notReverified"), "the observation badges must stay visible");
        assert!(VIEWER_JS.contains("never verified routes"), "the page must not present an observation as a route");
        assert!(VIEWER_JS.contains("uFogRange") && VIEWER_JS.contains("uLightDir"), "lighting and fog must be real shader uniforms");
        assert!(VIEWER_JS.contains("#version 300 es"), "the viewer must use a real WebGL2 shading language version");
        assert!(VIEWER_JS.contains("MATERIALS"), "layer materials must be explicit");
        assert!(VIEWER_JS.contains("worldXYZ") || VIEWER_JS.contains("worldX"), "saved world coordinates must stay in the read-only path");
        assert!(VIEWER_JS.contains("builds no EngineeringInput"), "the viewer must keep its no-engineering-claim header");
        for forbidden in ["builds an EngineeringInput", "computes a route", "certifies the terrain", "walkable_route = true", "routeVerified: true", "gl.LINES"] {
            assert!(!VIEWER_JS.contains(forbidden), "the viewer must not claim {forbidden}");
        }
    }

    /// Every `status.<field>` the viewer reads must actually be published by the
    /// model. This is the check that catches a HUD label reading a field name the
    /// model never emits - the defect the first G28 browser run found (the counts
    /// rendered as "undefined" because the viewer read `status.byCategory` while
    /// the model publishes `status.categoryCounts`).
    #[test]
    fn every_status_field_the_viewer_reads_is_published_by_the_model() {
        let model = build_model(&inputs("status-fields"));
        let status = model["status"].as_object().expect("the model must publish a status object");
        let mut names: Vec<String> = Vec::new();
        let mut index = 0;
        while let Some(found) = VIEWER_JS[index..].find("status.") {
            let start = index + found + "status.".len();
            let name: String = VIEWER_JS[start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && !names.contains(&name) { names.push(name); }
            index = start;
        }
        assert!(names.len() >= 8, "the viewer must read the published status object: found {names:?}");
        for name in &names {
            assert!(
                status.contains_key(name.as_str()),
                "the viewer reads status.{name} but the model does not publish that field"
            );
        }
        assert!(names.contains(&"categoryCounts".to_string()), "the per-category counts must be read under their published name");
        assert!(!VIEWER_JS.contains("status.byCategory"), "the viewer must not read a status field the model never publishes");
    }

    /// G28-R1: the terrain draw and pick paths must consume one validated
    /// topology function, and the terrain must never fall back to a non-indexed
    /// draw. The executable viewer regression (`viewer-topology-record.json`)
    /// proves the runtime behaviour; this test keeps the call sites themselves
    /// from drifting apart again the way they did in G28, when the draw and pick
    /// calls still passed the surface object as the width.
    #[test]
    fn the_terrain_draw_and_pick_paths_share_one_validated_topology() {
        assert!(!VIEWER_JS.contains("indicesFor("), "the unvalidated two-argument topology helper must be gone");
        assert!(VIEWER_JS.contains("function terrainTopology(width, depth, vertexCount)"), "one validated topology function must own the grid");
        assert!(VIEWER_JS.contains("function terrainIndicesFor(name)"), "the terrain indices must be looked up by surface name");
        assert!(
            VIEWER_JS.contains("drawIndexedTriangles(surface.verticesMetres, surfaceColours(surface, material.ramp), indices, \"terrain \" + state.surface)"),
            "the terrain must be submitted with the indices that were just validated"
        );
        assert!(VIEWER_JS.contains("var data = surfacePickData(state.surface);"), "picking must name the surface instead of passing a surface object");
        assert_eq!(
            VIEWER_JS.matches("gl.drawArrays(gl.TRIANGLES").count(),
            1,
            "only the explicit solid path may be non-indexed"
        );
        assert!(VIEWER_JS.contains("the indexed terrain draw path requires a non-empty index buffer"));
        assert!(
            VIEWER_JS.contains("if (surface.triangles !== undefined && triangles !== surface.triangles)"),
            "the produced triangle count must be checked against the model declaration"
        );
        assert!(VIEWER_JS.contains("describe(width)"), "illegal dimensions must be described when rejected");
        // G28-R1-H1: the HUD must be rebuildable and refreshed after every render
        assert!(VIEWER_JS.contains("function refreshHud()"), "the HUD needs a repeatable refresh entry point");
        assert!(VIEWER_JS.contains("function hudCountsHtml(status)") && VIEWER_JS.contains("function hudLayerHtml(status)"), "the HUD must be split into refreshable pieces");
        assert!(VIEWER_JS.contains("refreshHud();\n  }"), "render() must refresh the HUD after its state updates");
        assert!(VIEWER_JS.contains("state.hud = {"), "the same strings must be published on the state for comparison");
        // G28-R1 consumer identity label
        assert!(html_for("{}").contains("independent-3d-readonly-consumer"));
        assert!(!html_for("{}").contains("path-A-fact-only"));
        assert!(html_for("{}").contains("no EngineeringInput, no route, no engineering build"));
        assert!(VIEWER_JS.contains("never verified routes"));
    }

    // -----------------------------------------------------------------------
    // §3.1 / §3.2 / §3.3 contract tests
    // -----------------------------------------------------------------------

    #[test]
    fn the_shared_format_rules_catch_broken_shape_and_topology() {
        let model = build_model(&inputs("shape"));
        assert!(model_shape_failures(&model).is_empty(), "the built model must satisfy its own shape contract: {:?}", model_shape_failures(&model));
        let mut broken = model.clone();
        broken["surfaces"]["final"]["heightsMm"][0] = json!("not-a-number");
        assert!(!model_shape_failures(&broken).is_empty());
        let mut broken = model.clone();
        broken["surfaces"]["final"]["heightsMm"][0] = json!(9_000_000_000_i64);
        assert!(!model_shape_failures(&broken).is_empty(), "a sample outside the saved millimetre range must be refused");
        let mut broken = model.clone();
        broken["entities"][0]["triangleIndices"] = json!([0, 1, 99]);
        assert!(!model_shape_failures(&broken).is_empty(), "an out-of-range corner index must be refused");
        let mut broken = model.clone();
        broken["entities"][0]["triangleIndices"] = json!([0, 0, 1]);
        assert!(!model_shape_failures(&broken).is_empty(), "a degenerate triangle must be refused");
        let mut broken = model.clone();
        broken["nodes"][0]["worldY"] = json!(0.7);
        assert!(!model_shape_failures(&broken).is_empty(), "a constant world Y on an observation record must be refused");
        let mut broken = model.clone();
        broken["status"]["constantWorldYUsed"] = json!(true);
        assert!(!model_shape_failures(&broken).is_empty());
        let mut broken = model.clone();
        broken["unexpectedRootField"] = json!(true);
        assert!(!model_shape_failures(&broken).is_empty(), "the root field set is closed and grid/heightDisplayConversion are members");
        let mut broken = model.clone();
        broken["schemaVersion"] = json!("engineering-context-preview3d-v999");
        assert!(!model_shape_failures(&broken).is_empty());
        let mut broken = model.clone();
        broken["heightDisplayConversion"] = json!("metres = millimetres");
        assert!(!model_shape_failures(&broken).is_empty());
        let mut broken = model.clone();
        broken["grid"]["final"]["triangles"] = json!(1);
        assert!(!model_shape_failures(&broken).is_empty(), "the index topology must be two triangles per quad");
        assert!(MODEL_FIELDS.contains(&"grid") && MODEL_FIELDS.contains(&"heightDisplayConversion"));
    }

    #[test]
    fn the_v1_input_model_root_field_set_is_closed_and_fully_bound() {
        // An extra root declaration on the input v1 model is not this version.
        let dir = copied_package("v1-extra-root");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join("render-model.json")).expect("read")).expect("json");
        model["unexpectedExtraRoot"] = json!(true);
        fs::write(dir.join("render-model.json"), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        reseal_v1(&dir);
        assert!(load_inputs(&world_dir(), &g24_context_package(), &dir, None).is_err(), "an unknown v1 root field must be refused");

        // A v1 manifest that contradicts the v1 model is refused.
        let dir = copied_package("v1-manifest-scene");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("read")).expect("json");
        manifest["sceneId"] = json!("scene-9");
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("write");
        let mut marker: Value = serde_json::from_slice(&fs::read(dir.join("COMPLETE.json")).expect("read")).expect("json");
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join("COMPLETE.json"), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
        assert!(load_inputs(&world_dir(), &g24_context_package(), &dir, None).is_err(), "a contradictory v1 manifest must be refused");

        // A self-consistent v1 sources tamper is caught by the independent rebuild.
        let dir = copied_package("v1-source-tamper");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join("render-model.json")).expect("read")).expect("json");
        model["sources"]["sceneId"] = json!("scene-9");
        model["sources"]["declaredSceneId"] = json!("scene-9");
        fs::write(dir.join("render-model.json"), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("read")).expect("json");
        manifest["modelSha256"] = json!(sha256_hex(&fs::read(dir.join("render-model.json")).expect("read")));
        manifest["sceneId"] = json!("scene-9");
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join("manifest.json"), &manifest_bytes).expect("write");
        reseal_v1(&dir);
        assert!(load_inputs(&world_dir(), &g24_context_package(), &dir, None).is_err(), "a v1 sources tamper must fail the independent rebuild");
    }

    #[test]
    fn a_self_consistent_geometry_tamper_fails_only_the_entity_stage() {
        let dir = copied_3d(&fresh_3d("tamper-geometry"), "tamper-geometry");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("model")).expect("json");
        let corner = model["entities"][0]["cornersMetres"][0][0].as_f64().expect("corner");
        model["entities"][0]["cornersMetres"][0][0] = json!(corner + 1.0);
        fs::write(dir.join(MODEL_FILE), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        reseal_self_consistent(&dir);
        let (outcome, report) = audit_with(&dir, None);
        assert!(outcome.is_err());
        assert_eq!(stage_state(&report, "entityAndRoadParity"), "fail");
        assert_eq!(stage_state(&report, "sourceBinding"), "pass");
        assert_eq!(stage_state(&report, "terrainParity"), "pass");
        assert_eq!(stage_state(&report, "previewCompleteness"), "pass");
    }

    #[test]
    fn a_promoted_access_observation_flag_fails_the_entity_stage() {
        let dir = copied_3d(&fresh_3d("tamper-access"), "tamper-access");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("model")).expect("json");
        model["access"][0]["sourceObservationOnly"] = json!(false);
        fs::write(dir.join(MODEL_FILE), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        reseal_self_consistent(&dir);
        let (outcome, report) = audit_with(&dir, None);
        assert!(outcome.is_err());
        assert_eq!(stage_state(&report, "entityAndRoadParity"), "fail");
        assert_eq!(stage_state(&report, "previewCompleteness"), "pass");
    }

    #[test]
    fn a_self_consistent_3d_source_tamper_fails_only_the_source_stage() {
        let dir = copied_3d(&fresh_3d("tamper-source"), "tamper-source");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("model")).expect("json");
        model["sources"]["sceneId"] = json!("scene-9");
        fs::write(dir.join(MODEL_FILE), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        reseal_self_consistent(&dir);
        let (outcome, report) = audit_with(&dir, None);
        assert!(outcome.is_err());
        assert_eq!(stage_state(&report, "sourceBinding"), "fail");
        assert_eq!(stage_state(&report, "previewCompleteness"), "pass");
        assert_eq!(stage_state(&report, "terrainParity"), "pass");
        assert_eq!(stage_state(&report, "entityAndRoadParity"), "pass");
    }

    #[test]
    fn an_embedded_only_model_change_fails_the_completeness_stage() {
        let dir = copied_3d(&fresh_3d("tamper-embedded"), "tamper-embedded");
        let model_bytes = fs::read(dir.join(MODEL_FILE)).expect("model");
        let mut embedded: Value = serde_json::from_slice(&model_bytes).expect("json");
        embedded["status"]["entityCount"] = json!(999);
        embedded["sources"]["sceneId"] = json!("scene-9");
        let page = html_for(&serde_json::to_string_pretty(&embedded).expect("json"));
        fs::write(dir.join(HTML_FILE), page.as_bytes()).expect("write");
        // The declaration is synchronised, so only the producer page check can see it.
        let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read")).expect("json");
        if let Some(artifacts) = manifest.get_mut("artifacts").and_then(Value::as_array_mut) {
            for entry in artifacts.iter_mut() {
                if entry.get("path").and_then(Value::as_str) == Some(HTML_FILE) {
                    entry["bytes"] = json!(page.len() as u64);
                    entry["sha256"] = json!(sha256_hex(page.as_bytes()));
                }
            }
        }
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        let mut marker: Value = serde_json::from_slice(&fs::read(dir.join(MARKER_FILE)).expect("read")).expect("json");
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(dir.join(MARKER_FILE), serde_json::to_vec_pretty(&marker).expect("json")).expect("write");
        let (outcome, report) = audit_with(&dir, None);
        assert!(outcome.is_err());
        assert_eq!(stage_state(&report, "previewCompleteness"), "fail");
        assert_eq!(stage_state(&report, "sourceBinding"), "pass");
    }

    #[test]
    fn an_unknown_3d_version_is_refused_without_a_partial_comparison() {
        let dir = copied_3d(&fresh_3d("unknown-3d-version"), "unknown-3d-version");
        let mut model: Value = serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("model")).expect("json");
        model["schemaVersion"] = json!("engineering-context-preview3d-v999");
        fs::write(dir.join(MODEL_FILE), serde_json::to_vec_pretty(&model).expect("json")).expect("write");
        reseal_self_consistent(&dir);
        let (outcome, report) = audit_with(&dir, None);
        assert!(outcome.is_err());
        assert_eq!(stage_state(&report, "previewCompleteness"), "fail");
        assert_eq!(stage_state(&report, "terrainParity"), "notEvaluated");
        assert_eq!(stage_state(&report, "entityAndRoadParity"), "notEvaluated");
        assert_eq!(stage_state(&report, "sourceBinding"), "pass");
        assert_eq!(report["totalPass"], json!(false));
        assert_eq!(report["notEvaluatedStages"], json!(["terrainParity", "entityAndRoadParity"]));
    }

    #[test]
    fn the_four_stages_appear_exactly_once_in_the_written_report() {
        let dir = fresh_3d_with_trace("report-stages", &g25_audit());
        let (outcome, report) = audit_with(&dir, Some(&g25_audit()));
        assert!(outcome.is_ok(), "the clean package must pass with a real trace: {}", outcome.err().unwrap_or_default());
        let stages = report["stages"].as_array().expect("stages");
        for name in ["sourceBinding", "terrainParity", "entityAndRoadParity", "previewCompleteness"] {
            assert_eq!(stages.iter().filter(|stage| stage["name"] == json!(name)).count(), 1, "stage {name} must appear exactly once");
            assert_eq!(stage_state(&report, name), "pass");
        }
        assert_eq!(report["notEvaluatedStages"], json!([]));
        assert_eq!(report["evaluatedStages"].as_array().expect("evaluated").len(), 4);
        assert_eq!(report["totalPass"], json!(true));
        assert_eq!(report["schemaVersion"], json!(SCHEMA3D_AUDIT));
    }

    #[test]
    fn a_preexisting_target_is_never_adopted_or_cleaned() {
        let target = temp_dir("preexisting-target").join("package");
        fs::create_dir(&target).expect("dir");
        let sentinel = target.join("foreign-sentinel.txt");
        fs::write(&sentinel, b"keep me").expect("write");
        let args: Vec<String> = vec![
            "--world".into(), world_dir().display().to_string(),
            "--context".into(), g24_context_package().display().to_string(),
            "--preview".into(), g25_package().display().to_string(),
            "--out".into(), target.display().to_string(),
        ];
        assert!(export_3d(&args).is_err(), "a pre-existing target must be refused");
        assert_eq!(fs::read(&sentinel).expect("read"), b"keep me");
        assert_eq!(fs::read_dir(&target).expect("dir").count(), 1, "nothing may be written into a pre-existing target");
    }

    #[test]
    fn a_failure_never_leaves_a_complete_package_or_deletes_a_foreign_file() {
        let target = temp_dir("injected-failure").join("package");
        let mut owned = claim_directory(&target).expect("claim the new output directory");
        let files = vec![
            (MODEL_FILE.to_string(), b"model".to_vec()),
            (VIEWER_FILE.to_string(), b"viewer".to_vec()),
        ];
        let artifacts = package_artifacts(&files, b"{}", b"{}");
        assert_eq!(
            artifacts.last().map(|(name, _)| name.as_str()),
            Some(MARKER_FILE),
            "the completion marker must be written last"
        );
        let foreign = target.join("foreign-sentinel.txt");
        fs::write(&foreign, b"keep me").expect("write");
        let failure = publish(&mut owned, &artifacts, Some(1)).expect_err("an injected failure must be reported");
        assert!(failure.contains("injected failure"), "unexpected failure: {failure}");
        assert!(!target.join(MARKER_FILE).exists(), "no completion marker may survive a failed write");
        let cleanup = release(owned).expect_err("a foreign file must stop the directory cleanup");
        assert!(cleanup.contains("not empty"), "unexpected cleanup message: {cleanup}");
        assert_eq!(fs::read(&foreign).expect("read"), b"keep me", "a foreign file must never be deleted");
        assert!(!target.join(MODEL_FILE).exists(), "only this run's own files may be removed");
    }

    #[test]
    fn a_pre_existing_or_protected_report_path_is_refused() {
        let dir = fresh_3d("report-guard");
        let before = sha256_hex(&fs::read(dir.join(MODEL_FILE)).expect("model"));
        let existing = temp_dir("existing-report").join("report.json");
        fs::write(&existing, b"sentinel").expect("write");
        let mut args = audit_args(&dir);
        let position = args.iter().position(|value| value == "--report").expect("report flag");
        args[position + 1] = existing.display().to_string();
        let error = audit_3d(&args).expect_err("a pre-existing report must never be overwritten");
        assert!(format!("{error}").contains("already exists"), "unexpected error: {error}");
        assert_eq!(fs::read(&existing).expect("read"), b"sentinel");
        assert_eq!(sha256_hex(&fs::read(dir.join(MODEL_FILE)).expect("model")), before, "the audited package must not change");

        let protected = world_dir().join("forbidden-report.json");
        let mut args = audit_args(&dir);
        let position = args.iter().position(|value| value == "--report").expect("report flag");
        args[position + 1] = protected.display().to_string();
        assert!(audit_3d(&args).is_err(), "a report inside the protected source root must be refused");
        assert!(!protected.exists(), "the protected source root must not be written");
    }
}
