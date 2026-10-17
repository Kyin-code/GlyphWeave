//! Read-only engineering context export and its independent audit.
//!
//! `export-engineering-context` claims a brand-new output directory atomically,
//! writes every payload with `create_new`, and publishes the completion marker
//! last. `audit-engineering-context` first validates the package format against
//! an independently required contract, then rebuilds the source once from the
//! caller-given world root and compares every declared output field against it.
//! A dependency that could not run is `notEvaluated`; a proven failure always
//! outranks it; only a fully evaluated, fully matching package passes.

use glyphweave_core::engineering_context::{
    build_context, resolve_within_root, BoundScene, WindowRect, CONTEXT_CHECK_CONTRACT,
    FORBIDDEN_CLAIM_PHRASES, MANDATED_DRIFT_LIMITATION_PREFIX, MANDATED_LIMITATION_PREFIXES,
};
use glyphweave_core::engineering_preview::sha256_hex;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

type CliResult<T> = Result<T, Box<dyn Error>>;

const CONTEXT_FILE: &str = "engineering-context.json";
const MANIFEST_FILE: &str = "manifest.json";
const MARKER_FILE: &str = "COMPLETE.json";
/// The three required package files. This set is required independently of any
/// declaration inside the package: deleting a declaration cannot remove it.
const PACKAGE_FILES: [&str; 3] = [CONTEXT_FILE, MANIFEST_FILE, MARKER_FILE];
const PACKAGE_SCHEMA: &str = "engineering-context-audit-v1";
const SUPPORTED_CONTEXT_VERSIONS: [&str; 1] = ["engineering-context-v3"];
/// Package reads are bounded per file and cumulatively.
const MAX_PACKAGE_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const ACCESS_SCOPE: &str = "sceneWideWithReferenceFlags";
const STAGES: [&str; 4] = [
    "sourceBinding",
    "terrainParity",
    "entityAndRoadParity",
    "contextCompleteness",
];

/// Every output field of the context, the stage that audits it, and how it is
/// checked. Unknown root fields are rejected rather than ignored, so a field
/// can never appear without an owner.
#[derive(Debug, Clone, Copy)]
struct FieldRule {
    field: &'static str,
    stage: &'static str,
    rule: &'static str,
}

const FIELD_RULES: [FieldRule; 16] = [
    FieldRule { field: "schemaVersion", stage: "contextCompleteness", rule: "supportedVersion" },
    FieldRule { field: "readOnly", stage: "contextCompleteness", rule: "mustBeTrue" },
    FieldRule { field: "checks", stage: "contextCompleteness", rule: "formalStagesNotEvaluated" },
    FieldRule { field: "limitations", stage: "contextCompleteness", rule: "nonEmptyAndDriftDisclosed" },
    FieldRule { field: "sceneBinding", stage: "sourceBinding", rule: "sourceRebuildEquality" },
    FieldRule { field: "sourceBindings", stage: "sourceBinding", rule: "sourceRebuildEquality" },
    FieldRule { field: "heightFiles", stage: "sourceBinding", rule: "sourceRebuildEquality" },
    FieldRule { field: "requiredFileSet", stage: "sourceBinding", rule: "sourceRebuildEquality" },
    FieldRule { field: "window", stage: "terrainParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "terrainComparison", stage: "terrainParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "crop", stage: "terrainParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "entities", stage: "entityAndRoadParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "roads", stage: "entityAndRoadParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "planEdgeReferences", stage: "entityAndRoadParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "derived", stage: "entityAndRoadParity", rule: "sourceRebuildEquality" },
    FieldRule { field: "accessRecords", stage: "entityAndRoadParity", rule: "sourceRebuildEquality" },
];

fn field_stage(field: &str) -> Option<&'static str> {
    FIELD_RULES
        .iter()
        .find(|rule| rule.field == field)
        .map(|rule| rule.stage)
}

/// `accessScope` and the two count maps belong to the entity stage as well.
fn field_stage_extended(field: &str) -> Option<&'static str> {
    match field {
        "accessScope" | "entityKindCounts" | "unmodelledKinds" => Some("entityAndRoadParity"),
        other => field_stage(other),
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

/// Numeric comparison without loose tolerance: integers and integral floats
/// compare equal (`8` and `8.0` are the same number), everything else must
/// match exactly. No epsilon is applied, so a real geometric change is still a
/// difference, and millimetre samples and ids are never converted.
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            if a == b {
                return true;
            }
            match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => is_integral(x) && is_integral(y) && x == y,
                _ => false,
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, x)| b.get(key).map(|y| values_equal(x, y)).unwrap_or(false))
        }
        _ => left == right,
    }
}

fn is_integral(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value.abs() < 9.0e15
}

/// First difference between the package and the rebuilt source, with its path
/// and the stage that owns it.
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
            if values_equal(left, right) {
                None
            } else {
                Some(format!("{path} differs: source {left} vs package {right}"))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Stage {
    pub(super) name: &'static str,
    pub(super) state: &'static str,
    pub(super) detail: String,
}

impl Stage {
    pub(super) fn pass(name: &'static str, detail: String) -> Self {
        Self { name, state: "pass", detail }
    }
    pub(super) fn fail(name: &'static str, detail: String) -> Self {
        Self { name, state: "fail", detail }
    }
    pub(super) fn not_evaluated(name: &'static str, detail: String) -> Self {
        Self { name, state: "notEvaluated", detail }
    }
}

pub(super) fn compare_stage(
    name: &'static str,
    fresh: &Value,
    package: &Value,
    keys: &[&str],
) -> Stage {
    for key in keys {
        let (Some(fresh_value), Some(package_value)) = (fresh.get(*key), package.get(*key)) else {
            return Stage::fail(
                name,
                format!("{key} is missing from the rebuilt source or the package"),
            );
        };
        if let Some(difference) = first_difference(fresh_value, package_value, key) {
            return Stage::fail(name, difference);
        }
    }
    Stage::pass(
        name,
        format!("{} assigned field group(s) match the rebuilt source", keys.len()),
    )
}

// ---------------------------------------------------------------------------
// Package contract
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Contract {
    failures: Vec<String>,
    context: Option<Value>,
    /// False when the declared schema version is unsupported or unreadable, in
    /// which case no stage may partially compare against the current schema.
    context_valid: bool,
    window: Option<WindowRect>,
    context_sha256: String,
    manifest_sha256: String,
    marker_sha256: String,
    declared_required: Vec<String>,
    declared_source_bindings: Vec<Value>,
}

/// Reads one required package file through the same root-confined resolver the
/// source reader uses: canonical relative path only, no traversal, no absolute
/// paths, no symlink or reparse-point component, regular file only, and both a
/// per-file and a cumulative byte cap.
fn read_package_file(
    package_root: &Path,
    name: &str,
    budget: &mut u64,
    failures: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let resolved = match resolve_within_root(package_root, name) {
        Ok(path) => path,
        Err(error) => {
            failures.push(format!(
                "required package file {name} is not a confined path: {error}"
            ));
            return None;
        }
    };
    let metadata = match fs::metadata(&resolved) {
        Ok(metadata) => metadata,
        Err(error) => {
            failures.push(format!("required package file {name} is unreadable: {error}"));
            return None;
        }
    };
    if !metadata.is_file() {
        failures.push(format!(
            "required package file {name} is not a regular file"
        ));
        return None;
    }
    if metadata.len() > MAX_PACKAGE_FILE_BYTES {
        failures.push(format!(
            "required package file {name} is {} bytes, over the {MAX_PACKAGE_FILE_BYTES} byte cap",
            metadata.len()
        ));
        return None;
    }
    *budget = budget.saturating_add(metadata.len());
    if *budget > MAX_PACKAGE_TOTAL_BYTES {
        failures.push(format!(
            "the audited package exceeds the cumulative {MAX_PACKAGE_TOTAL_BYTES} byte cap"
        ));
        return None;
    }
    match fs::read(&resolved) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            failures.push(format!("required package file {name} is unreadable: {error}"));
            None
        }
    }
}

fn parse_object(bytes: &[u8], name: &str, failures: &mut Vec<String>) -> Option<Value> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value @ Value::Object(_)) => Some(value),
        Ok(_) => {
            failures.push(format!("{name} is not a JSON object"));
            None
        }
        Err(error) => {
            failures.push(format!("{name} is not readable JSON: {error}"));
            None
        }
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

/// Validates the package format independently of the source: required files,
/// versions, fixed references, uniqueness, hash relations, declared types and
/// the semantic honesty of the context itself.
fn validate_contract(package: &Path) -> Contract {
    let mut contract = Contract::default();
    // The package root is canonicalised first, so no alias can bypass the
    // confinement rules below.
    let package_root = match fs::canonicalize(package) {
        Ok(root) if root.is_dir() => root,
        Ok(root) => {
            contract.failures.push(format!(
                "audited package {} is not a directory",
                root.display()
            ));
            return contract;
        }
        Err(error) => {
            contract.failures.push(format!(
                "audited package {} is unusable: {error}",
                package.display()
            ));
            return contract;
        }
    };
    let mut budget = 0u64;
    let context_bytes = read_package_file(
        &package_root,
        CONTEXT_FILE,
        &mut budget,
        &mut contract.failures,
    );
    let manifest_bytes = read_package_file(
        &package_root,
        MANIFEST_FILE,
        &mut budget,
        &mut contract.failures,
    );
    let marker_bytes = read_package_file(
        &package_root,
        MARKER_FILE,
        &mut budget,
        &mut contract.failures,
    );

    let context = context_bytes
        .as_ref()
        .and_then(|bytes| parse_object(bytes, CONTEXT_FILE, &mut contract.failures));
    let manifest = manifest_bytes
        .as_ref()
        .and_then(|bytes| parse_object(bytes, MANIFEST_FILE, &mut contract.failures));
    let marker = marker_bytes
        .as_ref()
        .and_then(|bytes| parse_object(bytes, MARKER_FILE, &mut contract.failures));

    if let Some(bytes) = &context_bytes {
        contract.context_sha256 = sha256_hex(bytes);
    }
    if let Some(bytes) = &manifest_bytes {
        contract.manifest_sha256 = sha256_hex(bytes);
    }
    if let Some(bytes) = &marker_bytes {
        contract.marker_sha256 = sha256_hex(bytes);
    }

    // Extra files are refused: the package declares a closed file set. A
    // directory-entry error is a structured refusal, never silently skipped,
    // and a non-regular entry (directory, link, special file) cannot stand in
    // for a required file.
    match fs::read_dir(&package_root) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !PACKAGE_FILES.contains(&name.as_str()) {
                            contract
                                .failures
                                .push(format!("undeclared package file {name} is present"));
                            continue;
                        }
                        match entry.file_type() {
                            Ok(file_type) if file_type.is_file() => {}
                            Ok(_) => contract.failures.push(format!(
                                "required package entry {name} is not a regular file"
                            )),
                            Err(error) => contract.failures.push(format!(
                                "cannot inspect package entry {name}: {error}"
                            )),
                        }
                    }
                    Err(error) => contract
                        .failures
                        .push(format!("cannot read a package directory entry: {error}")),
                }
            }
        }
        Err(error) => contract
            .failures
            .push(format!("package directory is unreadable: {error}")),
    }

    // Each declaration is validated independently and the window is taken from
    // the context alone, so a missing marker cannot stop the source rebuild.
    if let Some(context) = &context {
        // An unsupported or unreadable version stops every schema-dependent
        // comparison instead of partially comparing against the current one.
        contract.context_valid = context
            .get("schemaVersion")
            .and_then(Value::as_str)
            .map(|version| SUPPORTED_CONTEXT_VERSIONS.contains(&version))
            .unwrap_or(false);
        validate_context_semantics(context, &mut contract);
        match context.get("window") {
            Some(window) => match checked_window(window) {
                Ok(parsed) => contract.window = Some(parsed),
                Err(detail) => contract.failures.push(detail),
            },
            None => contract
                .failures
                .push("context declares no window".to_string()),
        }
        contract.context = Some(context.clone());
    }
    let context_sha = contract.context_sha256.clone();
    let manifest_sha = contract.manifest_sha256.clone();
    if let Some(manifest) = &manifest {
        validate_manifest(manifest, &context_sha, &mut contract.failures);
        if let Some(context) = &context {
            validate_manifest_consistency(manifest, context, &mut contract.failures);
        }
        contract.declared_required = manifest
            .get("requiredFileSet")
            .and_then(Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        contract.declared_source_bindings = manifest
            .get("sourceBindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
    }
    if let Some(marker) = &marker {
        validate_marker(marker, &context_sha, &manifest_sha, &mut contract.failures);
    }
    contract
}

fn validate_context_semantics(context: &Value, contract: &mut Contract) {
    let failures = &mut contract.failures;
    let Some(object) = context.as_object() else {
        failures.push("context is not an object".to_string());
        return;
    };
    // Closed field set: a field with no assigned stage would otherwise be
    // silently ignored, so it is refused instead.
    for key in object.keys() {
        if field_stage_extended(key).is_none() {
            failures.push(format!("undocumented root field {key} is present"));
        }
    }
    for rule in FIELD_RULES.iter() {
        if !object.contains_key(rule.field) {
            failures.push(format!("required root field {} is missing", rule.field));
        }
    }
    for key in ["accessScope", "entityKindCounts", "unmodelledKinds"] {
        if !object.contains_key(key) {
            failures.push(format!("required root field {key} is missing"));
        }
    }
    match string_field(context, "schemaVersion") {
        Some(version) => {
            if !SUPPORTED_CONTEXT_VERSIONS.contains(&version.as_str()) {
                failures.push(format!(
                    "unsupported context schemaVersion {version}; supported: {SUPPORTED_CONTEXT_VERSIONS:?}"
                ));
            }
        }
        None => failures.push("context schemaVersion is missing or not a string".to_string()),
    }
    match context.get("readOnly") {
        Some(Value::Bool(true)) => {}
        Some(other) => failures.push(format!("readOnly must be true, found {other}")),
        None => failures.push("readOnly is missing".to_string()),
    }
    if string_field(context, "accessScope").as_deref() != Some(ACCESS_SCOPE) {
        failures.push(format!("accessScope must be {ACCESS_SCOPE}"));
    }
    validate_limitations(context, failures);
    validate_checks(context, failures);
    // The scene identity lives inside sceneBinding; the root field set is the
    // producer's own shape, so a field it does not emit is never demanded.
    match context.get("sceneBinding") {
        Some(Value::Object(binding)) => match binding.get("sceneId").and_then(Value::as_str) {
            Some(id) if !id.trim().is_empty() => {}
            Some(_) => failures.push("sceneBinding.sceneId must not be empty".to_string()),
            None => failures.push("sceneBinding.sceneId is missing or not a string".to_string()),
        },
        Some(_) => failures.push("sceneBinding must be an object".to_string()),
        None => failures.push("sceneBinding is missing".to_string()),
    }
}

fn validate_manifest(manifest: &Value, context_sha256: &str, failures: &mut Vec<String>) {
    match string_field(manifest, "schemaVersion").as_deref() {
        Some(PACKAGE_SCHEMA) => {}
        Some(other) => failures.push(format!("unsupported manifest schemaVersion {other}")),
        None => failures.push("manifest schemaVersion is missing".to_string()),
    }
    match string_field(manifest, "contextFile").as_deref() {
        Some(CONTEXT_FILE) => {}
        Some(other) => failures.push(format!("manifest contextFile must be {CONTEXT_FILE}, found {other}")),
        None => failures.push("manifest contextFile is missing".to_string()),
    }
    match string_field(manifest, "markerFile").as_deref() {
        Some(MARKER_FILE) => {}
        Some(other) => failures.push(format!("manifest markerFile must be {MARKER_FILE}, found {other}")),
        None => failures.push("manifest markerFile is missing".to_string()),
    }
    match string_field(manifest, "contextSha256") {
        Some(declared) => {
            if !context_sha256.is_empty() && declared != context_sha256 {
                failures.push(format!(
                    "manifest contextSha256 {declared} does not match the context file {context_sha256}"
                ));
            }
        }
        None => failures.push("manifest contextSha256 is missing".to_string()),
    }
    match manifest.get("packageFiles").and_then(Value::as_array) {
        Some(array) => {
            let names: Vec<&str> = array.iter().filter_map(Value::as_str).collect();
            if names.len() != array.len() {
                failures.push("manifest packageFiles must be an array of strings".to_string());
            }
            let mut sorted = names.clone();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() != names.len() {
                failures.push("manifest packageFiles contains duplicate entries".to_string());
            }
            let mut expected = PACKAGE_FILES.to_vec();
            expected.sort_unstable();
            if sorted != expected {
                failures.push(format!(
                    "manifest packageFiles must be exactly {expected:?}, found {names:?}"
                ));
            }
        }
        None => failures.push("manifest packageFiles is missing".to_string()),
    }
    match manifest.get("requiredFileSet").and_then(Value::as_array) {
        Some(array) if !array.is_empty() && array.iter().all(Value::is_string) => {
            let mut sorted: Vec<&str> = array.iter().filter_map(Value::as_str).collect();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() != array.len() {
                failures.push("manifest requiredFileSet contains duplicate entries".to_string());
            }
        }
        Some(_) => failures.push("manifest requiredFileSet must be a non-empty string array".to_string()),
        None => failures.push("manifest requiredFileSet is missing".to_string()),
    }
    match manifest.get("sourceBindings").and_then(Value::as_array) {
        Some(array) if !array.is_empty() => {
            for (index, binding) in array.iter().enumerate() {
                for field in ["path", "role", "sha256"] {
                    if binding.get(field).and_then(Value::as_str).is_none() {
                        failures.push(format!(
                            "manifest sourceBindings[{index}].{field} is missing or not a string"
                        ));
                    }
                }
                if binding.get("bytes").and_then(Value::as_u64).is_none() {
                    failures.push(format!(
                        "manifest sourceBindings[{index}].bytes is missing or not an unsigned integer"
                    ));
                }
            }
        }
        Some(_) => failures.push("manifest sourceBindings must be a non-empty array".to_string()),
        None => failures.push("manifest sourceBindings is missing".to_string()),
    }
    if manifest.as_object().map(Map::is_empty).unwrap_or(true) {
        failures.push("manifest is empty".to_string());
    }
}

fn validate_marker(
    marker: &Value,
    context_sha256: &str,
    manifest_sha256: &str,
    failures: &mut Vec<String>,
) {
    match string_field(marker, "status").as_deref() {
        Some("complete") => {}
        Some(other) => failures.push(format!("completion marker status must be complete, found {other}")),
        None => failures.push("completion marker status is missing".to_string()),
    }
    match string_field(marker, "schemaVersion").as_deref() {
        Some(PACKAGE_SCHEMA) => {}
        Some(other) => failures.push(format!("unsupported marker schemaVersion {other}")),
        None => failures.push("marker schemaVersion is missing".to_string()),
    }
    match string_field(marker, "contextFile").as_deref() {
        Some(CONTEXT_FILE) => {}
        Some(other) => failures.push(format!("marker contextFile must be {CONTEXT_FILE}, found {other}")),
        None => failures.push("marker contextFile is missing".to_string()),
    }
    match string_field(marker, "manifestFile").as_deref() {
        Some(MANIFEST_FILE) => {}
        Some(other) => failures.push(format!("marker manifestFile must be {MANIFEST_FILE}, found {other}")),
        None => failures.push("marker manifestFile is missing".to_string()),
    }
    match string_field(marker, "contextSha256") {
        Some(declared) => {
            if !context_sha256.is_empty() && declared != context_sha256 {
                failures.push(format!(
                    "marker contextSha256 {declared} does not hash the context file {context_sha256}"
                ));
            }
        }
        None => failures.push("marker contextSha256 is missing".to_string()),
    }
    match string_field(marker, "manifestSha256") {
        Some(declared) => {
            if !manifest_sha256.is_empty() && declared != manifest_sha256 {
                failures.push(format!(
                    "marker manifestSha256 {declared} does not hash the manifest file {manifest_sha256}"
                ));
            }
        }
        None => failures.push("marker manifestSha256 is missing".to_string()),
    }
}

/// The check set is a closed contract: complete, unique, known names only, the
/// declared state, and a detail that starts with the declared text and never
/// asserts a verification this pipeline does not perform.
fn validate_checks(context: &Value, failures: &mut Vec<String>) {
    let Some(checks) = context.get("checks") else {
        failures.push("checks is missing".to_string());
        return;
    };
    let Some(array) = checks.as_array() else {
        failures.push("checks must be an array".to_string());
        return;
    };
    for (index, entry) in array.iter().enumerate() {
        let Some(object) = entry.as_object() else {
            failures.push(format!("checks[{index}] is not an object"));
            continue;
        };
        for key in object.keys() {
            if !["name", "state", "detail"].contains(&key.as_str()) {
                failures.push(format!("checks[{index}] has an undocumented key {key}"));
            }
        }
        let Some(name) = object.get("name").and_then(Value::as_str) else {
            failures.push(format!("checks[{index}].name is missing or not a string"));
            continue;
        };
        let known = CONTEXT_CHECK_CONTRACT
            .iter()
            .find(|(declared, _, _)| *declared == name);
        let Some((_, required_state, required_detail)) = known else {
            failures.push(format!("checks[{index}] declares the unknown check {name}"));
            continue;
        };
        match object.get("state").and_then(Value::as_str) {
            Some(state) if state == *required_state => {}
            Some(state) => failures.push(format!(
                "check {name} must be {required_state}, found {state}"
            )),
            None => failures.push(format!("check {name} state is missing or not a string")),
        }
        match object.get("detail").and_then(Value::as_str) {
            Some(detail) => {
                if !detail.starts_with(required_detail) {
                    failures.push(format!(
                        "check {name} detail must start with {required_detail:?}, found {detail:?}"
                    ));
                }
                reject_forbidden_claims(&format!("check {name} detail"), detail, failures);
            }
            None => failures.push(format!("check {name} detail is missing or not a string")),
        }
    }
    for (name, _, _) in CONTEXT_CHECK_CONTRACT {
        let count = array
            .iter()
            .filter(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
            .count();
        match count {
            1 => {}
            0 => failures.push(format!("check {name} is missing")),
            other => failures.push(format!("check {name} appears {other} times")),
        }
    }
    if array.len() != CONTEXT_CHECK_CONTRACT.len() {
        failures.push(format!(
            "checks must contain exactly {} entries, found {}",
            CONTEXT_CHECK_CONTRACT.len(),
            array.len()
        ));
    }
}

fn reject_forbidden_claims(place: &str, text: &str, failures: &mut Vec<String>) {
    let lowered = text.to_lowercase();
    for phrase in FORBIDDEN_CLAIM_PHRASES {
        if lowered.contains(phrase) {
            failures.push(format!(
                "{place} asserts a verification this pipeline does not perform: {phrase:?}"
            ));
        }
    }
}

/// Limitations are recomputable facts, not free text: the mandated facts must
/// be present, nothing may repeat, and no entry may assert a verification.
fn validate_limitations(context: &Value, failures: &mut Vec<String>) {
    let Some(limitations) = context.get("limitations") else {
        failures.push("limitations is missing".to_string());
        return;
    };
    let Some(array) = limitations.as_array() else {
        failures.push("limitations must be an array".to_string());
        return;
    };
    if array.is_empty() {
        failures.push("limitations must not be empty".to_string());
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, entry) in array.iter().enumerate() {
        let Some(text) = entry.as_str() else {
            failures.push(format!("limitations[{index}] is not a string"));
            continue;
        };
        if text.trim().is_empty() {
            failures.push(format!("limitations[{index}] is empty"));
        }
        *seen.entry(text).or_insert(0) += 1;
        reject_forbidden_claims(&format!("limitations[{index}]"), text, failures);
    }
    for (text, count) in &seen {
        if *count > 1 {
            failures.push(format!("limitation {text:?} appears {count} times"));
        }
    }
    for prefix in MANDATED_LIMITATION_PREFIXES {
        let present = array.iter().any(|entry| {
            entry
                .as_str()
                .map(|text| text.starts_with(prefix))
                .unwrap_or(false)
        });
        if !present {
            failures.push(format!("mandated limitation fact is missing: {prefix:?}"));
        }
    }
    let drift = context
        .get("sceneBinding")
        .and_then(|binding| binding.get("sceneSchemaDrift"))
        .map(|value| !value.is_null())
        .unwrap_or(false);
    if drift {
        let disclosed = array.iter().any(|entry| {
            entry
                .as_str()
                .map(|text| text.starts_with(MANDATED_DRIFT_LIMITATION_PREFIX))
                .unwrap_or(false)
        });
        if !disclosed {
            failures.push(format!(
                "schema drift is present but the mandated disclosure is missing: {MANDATED_DRIFT_LIMITATION_PREFIX:?}"
            ));
        }
    }
}

/// Manifest declarations must agree with the context they describe. These are
/// package-format failures and belong to contextCompleteness.
fn validate_manifest_consistency(manifest: &Value, context: &Value, failures: &mut Vec<String>) {
    let declared_scene = string_field(manifest, "sceneId");
    let context_scene = context
        .get("sceneBinding")
        .and_then(|binding| binding.get("sceneId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    match (declared_scene, context_scene) {
        (Some(declared), Some(actual)) if declared == actual => {}
        (Some(declared), Some(actual)) => failures.push(format!(
            "manifest sceneId {declared} does not match the context scene {actual}"
        )),
        _ => failures.push("manifest sceneId or the context scene identity is unusable".to_string()),
    }
    let declared_window = manifest.get("window").and_then(Value::as_array);
    let context_window = context.get("window");
    match (declared_window, context_window) {
        (Some(array), Some(window)) => {
            let declared: Vec<i64> = array.iter().filter_map(Value::as_i64).collect();
            let expected: Option<Vec<i64>> = (|| {
                Some(vec![
                    window.get("minX")?.as_i64()?,
                    window.get("minZ")?.as_i64()?,
                    window.get("widthM")?.as_i64()?,
                    window.get("depthM")?.as_i64()?,
                ])
            })();
            match expected {
                Some(expected) if array.len() == 4 && declared == expected => {}
                Some(expected) => failures.push(format!(
                    "manifest window {declared:?} does not match the context window {expected:?}"
                )),
                None => failures.push("the context window is not four integers".to_string()),
            }
        }
        _ => failures.push("manifest window or the context window is unusable".to_string()),
    }
}

/// Manifest declarations versus the independent rebuild: the recorded source
/// bindings and the required source file set must match exactly.
fn manifest_matches_rebuilt(contract: &Contract, bound: &BoundScene) -> Option<String> {
    let describe = |value: &Value| {
        format!(
            "{}|{}|{}|{}",
            value.get("path").and_then(Value::as_str).unwrap_or_default(),
            value.get("role").and_then(Value::as_str).unwrap_or_default(),
            value.get("bytes").and_then(Value::as_u64).unwrap_or_default(),
            value.get("sha256").and_then(Value::as_str).unwrap_or_default()
        )
    };
    let mut declared: Vec<String> = contract
        .declared_source_bindings
        .iter()
        .map(describe)
        .collect();
    let mut rebuilt: Vec<String> = bound
        .bindings()
        .iter()
        .map(|binding| {
            format!(
                "{}|{}|{}|{}",
                binding.path, binding.role, binding.bytes, binding.sha256
            )
        })
        .collect();
    declared.sort();
    rebuilt.sort();
    if declared != rebuilt {
        return Some(format!(
            "the manifest records {} source binding(s) that do not match the {} recorded by the rebuilt source",
            declared.len(),
            rebuilt.len()
        ));
    }
    let required = match bound.required_file_set() {
        Ok(required) => required,
        Err(error) => return Some(format!("required set could not be derived: {error}")),
    };
    let mut declared_required = contract.declared_required.clone();
    let mut required_sorted = required.clone();
    declared_required.sort();
    required_sorted.sort();
    declared_required.dedup();
    if declared_required != required_sorted {
        return Some(format!(
            "the manifest declares {} required source file(s) that do not match the source's {}",
            declared_required.len(),
            required_sorted.len()
        ));
    }
    for relative in &required {
        if !bound.world_dir().join(relative).is_file() {
            return Some(format!("required source file {relative} is absent"));
        }
    }
    None
}

/// Checked window conversion: no `as i32` truncation, no negative sides, and
/// the same 128 m cap the core enforces.
fn checked_window(window: &Value) -> Result<WindowRect, String> {
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

// ---------------------------------------------------------------------------
// Publication ownership
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(super) struct Owned {
    dir: PathBuf,
    files: Vec<PathBuf>,
}

/// Claims the output directory atomically. A pre-existing target is never
/// adopted, so nothing that this run did not create is ever removed.
pub(super) fn claim_directory(dir: &Path) -> CliResult<Owned> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(Owned { dir: dir.to_path_buf(), files: Vec::new() }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(format!("target {} already exists; refusing to reuse it", dir.display()).into())
        }
        Err(error) => Err(format!("cannot claim target {}: {error}", dir.display()).into()),
    }
}

/// Removes only the files this run created. The directory itself is removed
/// with a non-recursive call, so a foreign file appearing later stops the
/// cleanup and is reported instead of being deleted.
pub(super) fn release(owned: Owned) -> Result<(), String> {
    let mut removed = 0;
    for path in &owned.files {
        match fs::remove_file(path) {
            Ok(()) => removed += 1,
            Err(error) => {
                return Err(format!(
                    "cannot clean up {}: {error}; incomplete package kept for inspection",
                    path.display()
                ))
            }
        }
    }
    match fs::remove_dir(&owned.dir) {
        Ok(()) => Ok(()),
        Err(error) => Err(format!(
            "removed {removed} owned file(s) but {} is not empty ({error}); incomplete package kept and must not be audited as complete",
            owned.dir.display()
        )),
    }
}

/// Writes every artifact with `create_new` and syncs it. `fail_after` lets a
/// deterministic test stop between real file primitives, so cleanup is
/// exercised against the filesystem rather than simulated before any write.
pub(super) fn publish(
    owned: &mut Owned,
    artifacts: &[(String, Vec<u8>)],
    fail_after: Option<usize>,
) -> Result<(), String> {
    for (index, (name, bytes)) in artifacts.iter().enumerate() {
        if Some(index) == fail_after {
            return Err(format!("injected failure before writing {name}"));
        }
        let path = owned.dir.join(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        owned.files.push(path.clone());
        file.write_all(bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync {}: {error}", path.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Report path rules
// ---------------------------------------------------------------------------

fn normalized(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn path_inside(child: &Path, root: &Path) -> bool {
    let child = normalized(child);
    let root = normalized(root);
    child == root || child.starts_with(&format!("{root}/"))
}

/// The report must be a brand-new file outside the source root and outside the
/// audited package. Every destructive experiment stays on this round's own
/// copies, never on the fixed source.
pub(super) fn check_report_path(report: &Path, world: &Path, package: &Path) -> CliResult<()> {
    if report.exists() {
        return Err(format!(
            "report {} already exists; refusing to overwrite it",
            report.display()
        )
        .into());
    }
    let parent = report
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let parent = fs::canonicalize(&parent)
        .map_err(|error| format!("report directory {} is unusable: {error}", parent.display()))?;
    let file_name = report
        .file_name()
        .ok_or_else(|| "report path has no file name".to_string())?;
    let resolved = parent.join(file_name);
    // Both roots are canonicalised so that `\\?\` prefixes, links and platform
    // aliases cannot make an inside path look like an outside one.
    let package_canonical = fs::canonicalize(package).map_err(|error| {
        format!("audited package {} is unusable: {error}", package.display())
    })?;
    if PACKAGE_FILES.contains(&file_name.to_string_lossy().as_ref())
        && path_inside(&resolved, &package_canonical)
    {
        return Err(format!(
            "report {} would overwrite the audited package file {}",
            report.display(),
            file_name.to_string_lossy()
        )
        .into());
    }
    if path_inside(&resolved, &package_canonical) {
        return Err(format!(
            "report {} is inside the audited package {}",
            report.display(),
            package.display()
        )
        .into());
    }
    let world_canonical = fs::canonicalize(world)
        .map_err(|error| format!("source root {} is unusable: {error}", world.display()))?;
    if path_inside(&resolved, &world_canonical) {
        return Err(format!(
            "report {} is inside the source root {}",
            report.display(),
            world.display()
        )
        .into());
    }
    if resolved.exists() {
        return Err(format!(
            "report {} already exists after resolution; refusing to overwrite it",
            resolved.display()
        )
        .into());
    }
    Ok(())
}

pub(super) fn write_report(path: &Path, report: &Value) -> CliResult<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create report {}: {error}", path.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

fn export_context(args: &[String]) -> CliResult<()> {
    let world = PathBuf::from(required(args, "--world")?);
    let window_text = required(args, "--window")?;
    let out = PathBuf::from(required(args, "--out")?);
    let expected_scene = flag(args, "--scene");

    let window = WindowRect::parse(&window_text)?;
    let bound = BoundScene::read(&world, window)?;
    if let Some(expected) = &expected_scene {
        if expected != &bound.scene().scene_id {
            return Err(format!(
                "--scene {expected} does not match the declared scene {}",
                bound.scene().scene_id
            )
            .into());
        }
    }
    let context = build_context(&bound)?;

    let mut owned = claim_directory(&out)?;
    let outcome = (|| -> Result<(String, String), String> {
        let context_bytes = serde_json::to_vec_pretty(&context)
            .map_err(|error| format!("cannot serialise the context: {error}"))?;
        let context_sha = sha256_hex(&context_bytes);

        let mut bindings: Vec<Value> = Vec::new();
        for binding in bound.bindings() {
            bindings.push(json!({
                "path": binding.path,
                "role": binding.role,
                "bytes": binding.bytes,
                "sha256": binding.sha256,
            }));
        }
        let manifest = json!({
            "schemaVersion": PACKAGE_SCHEMA,
            "sceneId": context.scene_binding.scene_id,
            "window": [context.window.min_x, context.window.min_z, context.window.width_m, context.window.depth_m],
            "sourceBindings": bindings,
            "requiredFileSet": context.required_file_set,
            "contextFile": CONTEXT_FILE,
            "contextSha256": context_sha,
            "markerFile": MARKER_FILE,
            "packageFiles": PACKAGE_FILES,
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("cannot serialise the manifest: {error}"))?;
        let manifest_sha = sha256_hex(&manifest_bytes);

        let marker = json!({
            "status": "complete",
            "schemaVersion": PACKAGE_SCHEMA,
            "contextFile": CONTEXT_FILE,
            "contextSha256": context_sha,
            "manifestFile": MANIFEST_FILE,
            "manifestSha256": manifest_sha,
        });
        let marker_bytes = serde_json::to_vec_pretty(&marker)
            .map_err(|error| format!("cannot serialise the marker: {error}"))?;

        // The completion marker is the last artifact published.
        let artifacts = vec![
            (CONTEXT_FILE.to_string(), context_bytes),
            (MANIFEST_FILE.to_string(), manifest_bytes),
            (MARKER_FILE.to_string(), marker_bytes),
        ];
        publish(&mut owned, &artifacts, None)?;
        Ok((manifest_sha, context_sha))
    })();

    match outcome {
        Ok((manifest_sha, context_sha)) => {
            println!(
                "exported engineering context: out={} window={} scene={} context={} sha256={} manifest={} marker={}",
                out.display(),
                window_text,
                bound.scene().scene_id,
                CONTEXT_FILE,
                context_sha,
                manifest_sha,
                MARKER_FILE
            );
            Ok(())
        }
        Err(detail) => {
            let cleanup = release(owned);
            match cleanup {
                Ok(()) => Err(format!("export failed ({detail}); created objects were cleaned up").into()),
                Err(cleanup_detail) => Err(format!(
                    "export failed ({detail}); cleanup also failed: {cleanup_detail}"
                )
                .into()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

fn audit_context(args: &[String]) -> CliResult<()> {
    let world = PathBuf::from(required(args, "--world")?);
    let package_dir = PathBuf::from(required(args, "--context")?);
    let report_path = PathBuf::from(required(args, "--report")?);
    let expected_window = flag(args, "--window");

    // Refused before anything is read or written.
    check_report_path(&report_path, &world, &package_dir)?;

    let contract = validate_contract(&package_dir);
    let mut stages: Vec<Stage> = Vec::new();

    // One independent rebuild, reused by every dependent stage.
    let rebuilt = match contract.window {
        Some(window) => match BoundScene::read(&world, window) {
            Ok(bound) => match build_context(&bound) {
                Ok(context) => Ok((bound, context)),
                Err(error) => Err(format!("the rebuilt source could not produce a context: {error}")),
            },
            Err(error) => Err(format!("the caller-given source could not be rebuilt: {error}")),
        },
        None => Err("the package declares no usable window, so the source was not rebuilt".to_string()),
    };

    // sourceBinding
    let mut source_binding_detail = Vec::new();
    if let Some(expected) = &expected_window {
        let declared = contract.window.map(|window| {
            format!("{},{},{},{}", window.min_x, window.min_z, window.width_m, window.depth_m)
        });
        if declared.as_deref() != Some(expected.as_str()) {
            source_binding_detail.push(format!(
                "the package window {declared:?} is not the expected {expected}"
            ));
        }
    }
    let fresh_value = match (&rebuilt, &contract.context) {
        (Ok((_, context)), Some(_)) if contract.context_valid => {
            // Same serialisation path as the published package, so f32 fields
            // compare like for like without any tolerance.
            match serde_json::to_vec(context)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            {
                Some(value) => Some(value),
                None => {
                    source_binding_detail.push("the rebuilt context could not be serialised".to_string());
                    None
                }
            }
        }
        _ => None,
    };
    let mut source_binding_failed = !source_binding_detail.is_empty();
    if let Err(detail) = &rebuilt {
        source_binding_failed = true;
        source_binding_detail.push(detail.clone());
    }
    if let (Some(fresh), Some(package)) = (&fresh_value, &contract.context) {
        let stage = compare_stage(
            "sourceBinding",
            fresh,
            package,
            &["sceneBinding", "sourceBindings", "heightFiles", "requiredFileSet"],
        );
        if stage.state == "fail" {
            source_binding_failed = true;
            source_binding_detail.push(stage.detail);
        }
    }
    // The manifest's own record of the source must match the rebuild, so a
    // package cannot merely restate its own header and pass.
    if let Ok((bound, _)) = &rebuilt {
        if let Some(detail) = manifest_matches_rebuilt(&contract, bound) {
            source_binding_failed = true;
            source_binding_detail.push(detail);
        }
    }
    stages.push(if !contract.context_valid {
        Stage::not_evaluated(
            "sourceBinding",
            "the declared schema version is unsupported, so no schema-dependent comparison ran"
                .to_string(),
        )
    } else if let Err(detail) = &rebuilt {
        Stage::fail("sourceBinding", detail.clone())
    } else if source_binding_failed {
        Stage::fail("sourceBinding", source_binding_detail.join("; "))
    } else {
        Stage::pass(
            "sourceBinding",
            "sceneBinding, sourceBindings, heightFiles and requiredFileSet match the rebuilt source"
                .to_string(),
        )
    });

    // The parity stages run only on a successful rebuild.
    match (&fresh_value, &contract.context) {
        (Some(fresh), Some(package)) => {
            stages.push(compare_stage(
                "terrainParity",
                fresh,
                package,
                &["crop", "terrainComparison", "window"],
            ));
            stages.push(compare_stage(
                "entityAndRoadParity",
                fresh,
                package,
                &[
                    "entities",
                    "roads",
                    "planEdgeReferences",
                    "derived",
                    "accessRecords",
                    "accessScope",
                    "entityKindCounts",
                    "unmodelledKinds",
                ],
            ));
        }
        _ => {
            for name in ["terrainParity", "entityAndRoadParity"] {
                stages.push(Stage::not_evaluated(
                    name,
                    "the source could not be rebuilt, so this stage did not run".to_string(),
                ));
            }
        }
    }

    // contextCompleteness: package format and honest declarations first, then
    // the source-dependent declaration cross-check. Control flow uses explicit
    // results only: a proven failure outranks a stage that could not run, and
    // only a fully evaluated stage can pass.
    let mut completeness_detail = contract.failures.clone();
    if completeness_detail.is_empty() && contract.context_valid {
        if let (Some(fresh), Some(package)) = (&fresh_value, &contract.context) {
            let stage = compare_stage(
                "contextCompleteness",
                fresh,
                package,
                &["checks", "limitations", "readOnly", "schemaVersion", "accessScope"],
            );
            if stage.state == "fail" {
                completeness_detail.push(stage.detail);
            }
        }
    }
    let completeness = if !completeness_detail.is_empty() {
        Stage::fail("contextCompleteness", completeness_detail.join("; "))
    } else if fresh_value.is_none() {
        Stage::not_evaluated(
            "contextCompleteness",
            "the source could not be rebuilt or the declared schema version is unsupported, so the source-dependent completeness checks did not run"
                .to_string(),
        )
    } else {
        Stage::pass(
            "contextCompleteness",
            "package format, declarations and the rebuilt context agree".to_string(),
        )
    };
    stages.push(completeness);

    finish(report_path, package_dir, world, stages, &contract)
}

fn finish(
    report_path: PathBuf,
    package_dir: PathBuf,
    world: PathBuf,
    stages: Vec<Stage>,
    contract: &Contract,
) -> CliResult<()> {
    let total_pass = stages.len() == STAGES.len() && stages.iter().all(|stage| stage.state == "pass");
    let evaluated: Vec<String> = stages
        .iter()
        .filter(|stage| stage.state != "notEvaluated")
        .map(|stage| stage.name.to_string())
        .collect();
    let not_evaluated: Vec<String> = stages
        .iter()
        .filter(|stage| stage.state == "notEvaluated")
        .map(|stage| stage.name.to_string())
        .collect();
    let coverage: Vec<Value> = FIELD_RULES
        .iter()
        .map(|rule| json!({"field": rule.field, "stage": rule.stage, "rule": rule.rule}))
        .collect();
    let report = json!({
        "schemaVersion": PACKAGE_SCHEMA,
        "sourceRoot": world.display().to_string(),
        "package": package_dir.display().to_string(),
        "packageContractFailures": contract.failures,
        "inputHashes": {
            "contextSha256": contract.context_sha256,
            "manifestSha256": contract.manifest_sha256,
            "markerSha256": contract.marker_sha256,
        },
        "fieldCoverage": coverage,
        "stages": stages
            .iter()
            .map(|stage| json!({"name": stage.name, "state": stage.state, "detail": stage.detail}))
            .collect::<Vec<Value>>(),
        "totalPass": total_pass,
        "evaluatedStages": evaluated,
        "notEvaluatedStages": not_evaluated,
    });
    write_report(&report_path, &report)?;
    for stage in &stages {
        println!("stage {}: {} ({})", stage.name, stage.state, stage.detail);
    }
    println!(
        "audit {}: totalPass={} report={}",
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
        Err(format!("audit failed at {}", failing.join(", ")).into())
    }
}

pub(super) fn run(args: &[String], audit: bool) -> CliResult<()> {
    if audit {
        audit_context(args)
    } else {
        export_context(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("gwc-cli-{}-{}-{}", std::process::id(), tag, nanos));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn marker_for(context_sha: &str, manifest_sha: &str) -> Value {
        json!({
            "status": "complete",
            "schemaVersion": PACKAGE_SCHEMA,
            "contextFile": CONTEXT_FILE,
            "contextSha256": context_sha,
            "manifestFile": MANIFEST_FILE,
            "manifestSha256": manifest_sha,
        })
    }

    /// A minimal but fully well-formed package, used as the positive control.
    fn write_package(dir: &Path, context_mutate: impl FnOnce(&mut Value)) -> (String, String) {
        let mut map = Map::new();
        map.insert("schemaVersion".into(), json!(SUPPORTED_CONTEXT_VERSIONS[0]));
        map.insert("readOnly".into(), json!(true));
        map.insert(
            "sceneBinding".into(),
            json!({"sceneId": "scene-0", "sceneSchemaDrift": null}),
        );
        map.insert("window".into(), json!({"minX": 0, "minZ": 0, "widthM": 4, "depthM": 4}));
        map.insert("sourceBindings".into(), json!([]));
        map.insert("heightFiles".into(), json!([]));
        map.insert("requiredFileSet".into(), json!(["world.json"]));
        map.insert("terrainComparison".into(), json!({"samples": 1}));
        map.insert("crop".into(), json!({"finalMm": [1], "naturalMm": [1]}));
        map.insert("entities".into(), json!([]));
        map.insert("roads".into(), json!([]));
        map.insert("planEdgeReferences".into(), json!([]));
        map.insert("derived".into(), json!({"windowSamples": 1}));
        map.insert("accessRecords".into(), json!([]));
        map.insert("accessScope".into(), json!(ACCESS_SCOPE));
        map.insert("entityKindCounts".into(), json!({}));
        map.insert("unmodelledKinds".into(), json!({}));
        map.insert(
            "limitations".into(),
            json!([
                MANDATED_LIMITATION_PREFIXES[0],
                MANDATED_LIMITATION_PREFIXES[1],
                "the saved scene index is the authority for entity and road identity",
                MANDATED_LIMITATION_PREFIXES[2],
                "height payloads for 0 non-intersecting chunk(s) were never opened"
            ]),
        );
        map.insert(
            "checks".into(),
            Value::Array(
                CONTEXT_CHECK_CONTRACT
                    .iter()
                    .map(|(name, state, detail)| {
                        json!({"name": name, "state": state, "detail": detail})
                    })
                    .collect::<Vec<Value>>(),
            ),
        );
        let mut context = Value::Object(map);
        context_mutate(&mut context);
        let context_bytes = serde_json::to_vec_pretty(&context).expect("context");
        let context_sha = sha256_hex(&context_bytes);
        fs::write(dir.join(CONTEXT_FILE), &context_bytes).expect("context write");

        let manifest = json!({
            "schemaVersion": PACKAGE_SCHEMA,
            "sceneId": "scene-0",
            "window": [0, 0, 4, 4],
            "sourceBindings": [{"path": "world.json", "role": "world-index", "bytes": 1, "sha256": "00"}],
            "requiredFileSet": ["world.json"],
            "contextFile": CONTEXT_FILE,
            "contextSha256": context_sha,
            "markerFile": MARKER_FILE,
            "packageFiles": PACKAGE_FILES,
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest");
        let manifest_sha = sha256_hex(&manifest_bytes);
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("manifest write");
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &manifest_sha)).expect("marker"),
        )
        .expect("marker write");
        (context_sha, manifest_sha)
    }

    #[test]
    fn wellformed_package_passes_the_contract() {
        let dir = temp_dir("ok");
        write_package(&dir, |_| {});
        let contract = validate_contract(&dir);
        assert!(
            contract.failures.is_empty(),
            "unexpected failures: {:?}",
            contract.failures
        );
        assert!(contract.window.is_some());
    }

    #[test]
    fn empty_manifest_and_synced_marker_hash_is_refused() {
        let dir = temp_dir("empty-manifest");
        let (context_sha, _) = write_package(&dir, |_| {});
        let empty = b"{}".to_vec();
        let empty_sha = sha256_hex(&empty);
        fs::write(dir.join(MANIFEST_FILE), &empty).expect("manifest write");
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &empty_sha)).expect("marker"),
        )
        .expect("marker write");
        let contract = validate_contract(&dir);
        assert!(!contract.failures.is_empty());
        assert!(contract.failures.iter().any(|line| line.contains("manifest")));
    }

    #[test]
    fn wrong_marker_manifest_hash_is_refused() {
        let dir = temp_dir("marker-hash");
        let (context_sha, _) = write_package(&dir, |_| {});
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &"0".repeat(64))).expect("marker"),
        )
        .expect("marker write");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("manifestSha256")));
    }

    #[test]
    fn unknown_context_version_is_refused_before_comparison() {
        let dir = temp_dir("version");
        let (context_sha, manifest_sha) = write_package(&dir, |context| {
            context["schemaVersion"] = json!("engineering-context-v999");
        });
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &manifest_sha)).expect("marker"),
        )
        .expect("marker write");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("unsupported context schemaVersion")));
    }

    #[test]
    fn undeclared_extra_package_file_is_refused() {
        let dir = temp_dir("extra");
        write_package(&dir, |_| {});
        fs::write(dir.join("undeclared.txt"), b"x").expect("extra write");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("undeclared package file")));
    }

    #[test]
    fn missing_required_package_file_is_refused() {
        let dir = temp_dir("missing-file");
        write_package(&dir, |_| {});
        fs::remove_file(dir.join(CONTEXT_FILE)).expect("remove context");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("required package file")));
    }

    #[test]
    fn forged_status_claims_are_refused_field_by_field() {
        let cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
            (
                "readonly",
                Box::new(|context: &mut Value| context["readOnly"] = json!(false)),
            ),
            (
                "checks",
                Box::new(|context: &mut Value| {
                    context["checks"] = json!(STAGES
                        .iter()
                        .map(|stage| json!({"name": stage, "state": "pass"}))
                        .collect::<Vec<Value>>())
                }),
            ),
            (
                "limitations",
                Box::new(|context: &mut Value| context["limitations"] = json!([])),
            ),
            (
                "unknown-field",
                Box::new(|context: &mut Value| {
                    context["safetyVerified"] = json!(true);
                }),
            ),
            (
                "access-scope",
                Box::new(|context: &mut Value| context["accessScope"] = json!("onlyThisWindow")),
            ),
            (
                "scene-binding-identity",
                Box::new(|context: &mut Value| {
                    context["sceneBinding"] = json!({"sceneSchemaDrift": null})
                }),
            ),
        ];
        for (tag, mutate) in cases {
            let dir = temp_dir(tag);
            let (context_sha, manifest_sha) = write_package(&dir, |context| mutate(context));
            fs::write(
                dir.join(MARKER_FILE),
                serde_json::to_vec_pretty(&marker_for(&context_sha, &manifest_sha)).expect("marker"),
            )
            .expect("marker write");
            let contract = validate_contract(&dir);
            assert!(
                !contract.failures.is_empty(),
                "forged {tag} package was accepted"
            );
        }
    }

    #[test]
    fn schema_drift_without_disclosure_is_refused() {
        let dir = temp_dir("drift");
        let (context_sha, manifest_sha) = write_package(&dir, |context| {
            context["sceneBinding"] = json!({"sceneSchemaDrift": "missing field `accessMethod`"});
        });
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &manifest_sha)).expect("marker"),
        )
        .expect("marker write");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("mandated disclosure is missing")));
    }

    #[test]
    fn numeric_representation_equivalence_has_no_tolerance() {
        assert!(values_equal(&json!(8), &json!(8.0)));
        assert!(values_equal(&json!([1, 2.0]), &json!([1.0, 2])));
        assert!(values_equal(&json!({"m": 64.0}), &json!({"m": 64})));
        assert!(!values_equal(&json!(8), &json!(8.5)));
        assert!(!values_equal(&json!({"m": 64.0}), &json!({"m": 64.5})));
        // No epsilon: a real change in a non-integral value is still a change.
        assert!(!values_equal(&json!(0.24), &json!(0.25)));
    }

    #[test]
    fn window_conversion_is_checked() {
        assert!(checked_window(&json!({"minX": 0, "minZ": 0, "widthM": 64, "depthM": 64})).is_ok());
        assert!(checked_window(&json!({"minX": 0, "minZ": 0, "widthM": 0, "depthM": 64})).is_err());
        assert!(checked_window(&json!({"minX": 0, "minZ": 0, "widthM": -4, "depthM": 64})).is_err());
        assert!(checked_window(&json!({"minX": 0, "minZ": 0, "widthM": 4096, "depthM": 64})).is_err());
        assert!(
            checked_window(&json!({"minX": 9_000_000_000i64, "minZ": 0, "widthM": 4, "depthM": 4}))
                .is_err(),
            "i64 values outside i32 must be refused rather than truncated"
        );
        assert!(checked_window(&json!({"minX": 0, "minZ": 0, "widthM": 4.5, "depthM": 4})).is_err());
    }

    /// A genuine package built from the fixed source, so manifest declarations
    /// can be compared against an independent rebuild.
    fn real_package(dir: &Path, mutate_manifest: impl FnOnce(&mut Value)) {
        let window = WindowRect::parse("256,32,64,64").expect("window");
        let bound = BoundScene::read(&fixed_source_dir(), window).expect("source binds");
        let context = build_context(&bound).expect("context builds");
        let context_bytes = serde_json::to_vec_pretty(&context).expect("context bytes");
        let context_sha = sha256_hex(&context_bytes);
        fs::write(dir.join(CONTEXT_FILE), &context_bytes).expect("context write");
        let bindings: Vec<Value> = bound
            .bindings()
            .iter()
            .map(|binding| {
                json!({
                    "path": binding.path,
                    "role": binding.role,
                    "bytes": binding.bytes,
                    "sha256": binding.sha256,
                })
            })
            .collect();
        let mut manifest = json!({
            "schemaVersion": PACKAGE_SCHEMA,
            "sceneId": context.scene_binding.scene_id,
            "window": [context.window.min_x, context.window.min_z, context.window.width_m, context.window.depth_m],
            "sourceBindings": bindings,
            "requiredFileSet": context.required_file_set,
            "contextFile": CONTEXT_FILE,
            "contextSha256": context_sha,
            "markerFile": MARKER_FILE,
            "packageFiles": PACKAGE_FILES,
        });
        mutate_manifest(&mut manifest);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        let manifest_sha = sha256_hex(&manifest_bytes);
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("manifest write");
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker_for(&context_sha, &manifest_sha)).expect("marker"),
        )
        .expect("marker write");
    }

    fn fixed_source_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/joint-r27-verified-baseline")
    }

    fn fixed_window() -> WindowRect {
        WindowRect::parse("256,32,64,64").expect("window")
    }

    /// Failures recorded for a self-consistent fixture whose context was
    /// mutated before the declaration chain was sealed.
    fn contract_failures_after(tag: &str, mutate: impl FnOnce(&mut Value)) -> Vec<String> {
        let dir = temp_dir(tag);
        write_package(&dir, |context| mutate(context));
        validate_contract(&dir).failures
    }

    #[test]
    fn real_package_passes_the_contract_and_the_rebuild_comparison() {
        let dir = temp_dir("real-ok");
        real_package(&dir, |_| {});
        let contract = validate_contract(&dir);
        assert!(
            contract.failures.is_empty(),
            "unexpected failures: {:?}",
            contract.failures
        );
        assert!(contract.context_valid);
        let bound = BoundScene::read(&fixed_source_dir(), fixed_window()).expect("rebuild");
        assert!(manifest_matches_rebuilt(&contract, &bound).is_none());
        let window = contract.window.expect("window");
        assert_eq!((window.min_x, window.width_m), (256, 64));
    }

    #[test]
    fn faked_source_bindings_are_refused_by_the_rebuild_comparison() {
        let dir = temp_dir("fake-bindings");
        real_package(&dir, |manifest| {
            let faked: Vec<Value> = (0..4)
                .map(|index| {
                    json!({"path": format!("fake/{index}"), "role": "final-height", "bytes": 1, "sha256": "00"})
                })
                .collect();
            manifest["sourceBindings"] = Value::Array(faked);
        });
        let contract = validate_contract(&dir);
        assert!(
            contract.failures.is_empty(),
            "the forged bindings are shape-valid, so only the rebuild may catch them: {:?}",
            contract.failures
        );
        let bound = BoundScene::read(&fixed_source_dir(), fixed_window()).expect("rebuild");
        assert!(manifest_matches_rebuilt(&contract, &bound).is_some());
    }

    #[test]
    fn shortened_bindings_and_a_wrong_required_set_are_refused() {
        let shortened = temp_dir("short-bindings");
        real_package(&shortened, |manifest| {
            let kept = manifest["sourceBindings"]
                .as_array()
                .expect("bindings")[0]
                .clone();
            manifest["sourceBindings"] = Value::Array(vec![kept]);
        });
        let contract = validate_contract(&shortened);
        assert!(contract.failures.is_empty());
        let bound = BoundScene::read(&fixed_source_dir(), fixed_window()).expect("rebuild");
        assert!(manifest_matches_rebuilt(&contract, &bound).is_some());

        let wrong_set = temp_dir("wrong-required");
        real_package(&wrong_set, |manifest| {
            manifest["requiredFileSet"] = json!(["world.json"]);
        });
        let contract = validate_contract(&wrong_set);
        assert!(contract.failures.is_empty());
        assert!(manifest_matches_rebuilt(&contract, &bound).is_some());
    }

    #[test]
    fn manifest_declarations_must_match_the_context() {
        let dir = temp_dir("manifest-mismatch");
        real_package(&dir, |manifest| {
            manifest["sceneId"] = json!("scene-9");
            manifest["window"] = json!([0, 0, 8, 8]);
        });
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("does not match the context scene")));
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("does not match the context window")));
    }

    #[test]
    fn non_formal_check_states_cannot_be_flipped_to_pass() {
        for name in ["accessRouteReverification", "engineeringFeasibility"] {
            let failures = contract_failures_after("flip-check", |context| {
                for check in context["checks"].as_array_mut().expect("checks") {
                    if check["name"] == json!(name) {
                        check["state"] = json!("pass");
                    }
                }
            });
            assert!(
                failures
                    .iter()
                    .any(|line| line.contains(name) && line.contains("must be notEvaluated")),
                "{name}: {failures:?}"
            );
        }
    }

    #[test]
    fn duplicate_check_entries_are_refused() {
        let failures = contract_failures_after("dup-check", |context| {
            let checks = context["checks"].as_array_mut().expect("checks");
            let copy = checks[6].clone();
            checks.push(copy);
        });
        assert!(
            failures.iter().any(|line| line.contains("appears 2 times")),
            "{failures:?}"
        );
    }

    #[test]
    fn missing_and_unknown_check_entries_are_refused() {
        let missing = contract_failures_after("missing-check", |context| {
            let checks = context["checks"].as_array_mut().expect("checks");
            checks.retain(|check| check["name"] != json!("terrainComparison"));
        });
        assert!(
            missing
                .iter()
                .any(|line| line.contains("terrainComparison") && line.contains("missing")),
            "{missing:?}"
        );

        let unknown = contract_failures_after("unknown-check", |context| {
            for check in context["checks"].as_array_mut().expect("checks") {
                if check["name"] == json!("engineeringFeasibility") {
                    check["name"] = json!("routeVerified");
                }
            }
        });
        assert!(
            unknown
                .iter()
                .any(|line| line.contains("unknown check routeVerified")),
            "{unknown:?}"
        );
    }

    #[test]
    fn removed_replaced_or_duplicated_limitations_are_refused() {
        let removed = contract_failures_after("drop-limitation", |context| {
            let lines = context["limitations"].as_array_mut().expect("limitations");
            lines.retain(|line| line.as_str() != Some(MANDATED_LIMITATION_PREFIXES[1]));
        });
        assert!(
            removed
                .iter()
                .any(|line| line.contains("mandated limitation fact is missing")),
            "{removed:?}"
        );

        let replaced = contract_failures_after("keyword-limitations", |context| {
            context["limitations"] = json!(["unrelated text that mentions are not verified"]);
        });
        assert!(
            replaced
                .iter()
                .any(|line| line.contains("mandated limitation fact is missing")),
            "{replaced:?}"
        );

        let duplicated = contract_failures_after("dup-limitation", |context| {
            let lines = context["limitations"].as_array_mut().expect("limitations");
            let first = lines[0].clone();
            lines.push(first);
        });
        assert!(
            duplicated.iter().any(|line| line.contains("appears 2 times")),
            "{duplicated:?}"
        );
    }

    #[test]
    fn claims_of_verification_are_refused() {
        let failures = contract_failures_after("contradiction", |context| {
            let lines = context["limitations"].as_array_mut().expect("limitations");
            lines.push(json!(
                "the route is verified and the design is engineering feasible"
            ));
        });
        assert!(
            failures
                .iter()
                .any(|line| line.contains("asserts a verification")),
            "{failures:?}"
        );
    }

    #[test]
    fn unsupported_version_invalidates_schema_dependent_comparison() {
        let dir = temp_dir("version-invalid");
        write_package(&dir, |context| {
            context["schemaVersion"] = json!("engineering-context-v999")
        });
        let contract = validate_contract(&dir);
        assert!(!contract.context_valid);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("unsupported context schemaVersion")));
    }

    #[test]
    fn package_reads_are_confined_to_the_package_root() {
        let dir = temp_dir("confine-package");
        write_package(&dir, |_| {});
        let mut budget = 0u64;
        let mut failures = Vec::new();
        assert!(read_package_file(&dir, "../escape.json", &mut budget, &mut failures).is_none());
        assert!(failures
            .iter()
            .any(|line| line.contains("not a confined path")));
        let mut failures = Vec::new();
        assert!(
            read_package_file(&dir, "C:/Windows/win.ini", &mut budget, &mut failures).is_none()
        );
        assert!(failures
            .iter()
            .any(|line| line.contains("not a confined path")));
        let mut failures = Vec::new();
        assert!(read_package_file(&dir, "nested/context.json", &mut budget, &mut failures).is_none());
        assert!(!failures.is_empty());
    }

    #[test]
    fn extra_and_non_regular_package_entries_are_refused() {
        let dir = temp_dir("extra-entry");
        write_package(&dir, |_| {});
        fs::write(dir.join("undeclared.txt"), b"x").expect("extra");
        fs::create_dir(dir.join("subdir")).expect("subdir");
        let contract = validate_contract(&dir);
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("undeclared package file undeclared.txt")));
        assert!(contract
            .failures
            .iter()
            .any(|line| line.contains("undeclared package file subdir")));
    }

    #[test]
    fn report_path_rules_refuse_inputs_and_accept_a_fresh_file() {
        let root = temp_dir("report");
        let world = root.join("world");
        let package = root.join("package");
        fs::create_dir_all(&world).expect("world");
        fs::create_dir_all(&package).expect("package");
        fs::write(world.join("world.json"), b"{}").expect("world file");
        fs::write(package.join(CONTEXT_FILE), b"{}").expect("context file");

        let existing = root.join("existing.json");
        fs::write(&existing, b"sentinel").expect("existing");
        assert!(check_report_path(&existing, &world, &package).is_err());
        assert_eq!(fs::read(&existing).expect("read"), b"sentinel");

        assert!(check_report_path(&package.join(CONTEXT_FILE), &world, &package).is_err());
        assert!(check_report_path(&package.join("report.json"), &world, &package).is_err());
        assert!(check_report_path(&world.join("report.json"), &world, &package).is_err());
        assert!(check_report_path(&root.join("report.json"), &world, &package).is_ok());
    }

    #[test]
    fn publication_claims_a_new_directory_and_refuses_an_existing_one() {
        let root = temp_dir("claim");
        let target = root.join("out");
        let owned = claim_directory(&target).expect("claim succeeds");
        assert!(target.is_dir());
        release(owned).expect("cleanup removes the owned empty directory");
        assert!(!target.exists());

        fs::create_dir(&target).expect("foreign dir");
        fs::write(target.join("sentinel.txt"), b"keep").expect("sentinel");
        assert!(claim_directory(&target).is_err(), "existing target must be refused");
        assert_eq!(fs::read(target.join("sentinel.txt")).expect("read"), b"keep");
    }

    #[test]
    fn mid_write_failure_cleans_only_owned_objects() {
        let root = temp_dir("midfail");
        let target = root.join("out");
        let mut owned = claim_directory(&target).expect("claim");
        let artifacts = vec![
            (CONTEXT_FILE.to_string(), b"a".to_vec()),
            (MANIFEST_FILE.to_string(), b"b".to_vec()),
            (MARKER_FILE.to_string(), b"c".to_vec()),
        ];
        let error = publish(&mut owned, &artifacts, Some(2)).expect_err("injected failure");
        assert!(
            error.contains(MARKER_FILE),
            "the injected failure stops before the marker, got: {error}"
        );
        assert!(target.join(CONTEXT_FILE).is_file(), "written file exists before cleanup");
        assert!(!target.join(MARKER_FILE).exists(), "the marker was never published");
        release(owned).expect("cleanup");
        assert!(!target.exists(), "owned files and directory removed");
    }

    #[test]
    fn foreign_file_appearing_during_publication_stops_cleanup_and_is_preserved() {
        let root = temp_dir("foreign");
        let target = root.join("out");
        let mut owned = claim_directory(&target).expect("claim");
        let artifacts = vec![
            (CONTEXT_FILE.to_string(), b"a".to_vec()),
            (MANIFEST_FILE.to_string(), b"b".to_vec()),
        ];
        publish(&mut owned, &artifacts, None).expect("publish first two");
        fs::write(target.join("foreign.txt"), b"keep").expect("foreign");
        let error = release(owned).expect_err("cleanup must refuse to delete a foreign file");
        assert!(error.contains("not empty"), "got: {error}");
        assert_eq!(fs::read(target.join("foreign.txt")).expect("read"), b"keep");
        assert!(!target.join(MARKER_FILE).exists(), "no completion marker was published");
    }
}
