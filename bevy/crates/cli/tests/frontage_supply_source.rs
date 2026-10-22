//! B01-SUPPLY-R4: the independent CLI audit of a version-3 demand-led world
//! derives its natural source from the trusted original manifest and completes
//! the source rebuild.
//!
//! Everything here runs through the real binary: one bake, then the real
//! `validate-world` and `quality-report` subcommands on the same world, plus the
//! source-binding negatives. Nothing is proven by calling a core helper instead
//! of the CLI.
use glyphweave_core::worldgen::{
    NaturalSource, WorldManifest, bake_world, declared_natural_source,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

struct TestDir(PathBuf);
impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("glyphweave-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
}

fn read(path: &PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write(path: &PathBuf, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn glyphweave(dir: &TestDir, command: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg(command)
        .arg(&dir.0)
        .output()
        .unwrap()
}

fn streams(output: &std::process::Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// A legal version-3 authored input: non-zero world seed *and* non-zero scene
/// seed offset (they do not cancel: the effective seed is neither), millimetre
/// precision and a one-state supply budget, so the sweep is bounded.
fn supply_manifest() -> WorldManifest {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/b02-road-supply.manifest.json");
    let mut manifest: WorldManifest = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.world.seed = 0x00c0_ffee;
    manifest.scenes[0].seed_offset = 0x0000_beef;
    manifest.style["heightPrecision"] = json!("millimeter");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["automaticRoadExtensions"]["version"] = json!(3);
    manifest.style["automaticRoadExtensions"]["scenes"][0]["supply"] = json!({
        "version": 1,
        "maxCandidateStates": 1,
        "preflightBudgetMs": 180000,
        "mode": "append",
    });
    manifest
}

#[test]
fn a_version_three_world_audits_through_the_real_cli() {
    let manifest = supply_manifest();
    let scene = manifest.scenes[0].clone();
    assert_ne!(manifest.world.seed, 0);
    assert_ne!(scene.seed_offset, 0);
    // The rule generation and the independent audit share.
    let source = declared_natural_source(&manifest, &scene).unwrap();
    let effective_seed = match source {
        NaturalSource::AuthoredSeed { seed, precise } => {
            assert!(precise, "millimetre precision must be declared");
            seed
        }
        other => panic!("the authored v3 input must declare an authored seed, got {other:?}"),
    };
    assert_ne!(effective_seed, 0);
    assert_eq!(effective_seed, manifest.world.seed ^ scene.seed_offset);

    let dir = TestDir::new("r4-cli-source");
    bake_world(&manifest, &dir.0).unwrap();

    let validate = glyphweave(&dir, "validate-world");
    let (validate_out, validate_err) = streams(&validate);
    assert!(
        validate.status.success(),
        "validate-world failed: {validate_err}"
    );

    let quality = glyphweave(&dir, "quality-report");
    let (quality_out, quality_err) = streams(&quality);
    // The wiring error the old undeclared wrapper produced must be gone.
    assert!(
        !quality_out.contains("declared natural source")
            && !quality_err.contains("declared natural source"),
        "the independent audit must not fall back to an undeclared source: {quality_err}"
    );
    assert!(
        quality.status.success(),
        "quality-report failed: {quality_err}"
    );
    let report = read(&dir.0.join("quality-report.json"));
    let audit = report["scenes"][0]["roadNetworkAudit"].clone();
    assert!(
        audit.is_object(),
        "the report must carry the independent road audit: {}",
        report["scenes"][0]
    );
    println!(
        "R4-EVIDENCE {}",
        json!({
            "case": "positive-cli-round-trip",
            "worldSeed": manifest.world.seed,
            "sceneSeedOffset": scene.seed_offset,
            "effectiveSeed": effective_seed,
            "declaredSource": "AuthoredSeed",
            "validateWorldExit": validate.status.code(),
            "qualityReportExit": quality.status.code(),
            "roadNetworkAuditStatus": audit["status"].clone(),
            "qualityReportStatus": report["status"].clone(),
            "qualityWarnings": report["warnings"].clone(),
        })
    );

    // The release-facing gate calls quality-report itself, so it must not die on
    // the same wiring either. Its verdict may still be a content verdict.
    let gate = glyphweave(&dir, "world-quality-gate");
    let (_, gate_err) = streams(&gate);
    assert!(
        !gate_err.contains("declared natural source"),
        "the gate path must not fall back to an undeclared source: {gate_err}"
    );
    let gate_report = read(&dir.0.join("world-quality-gate.json"));
    println!(
        "R4-EVIDENCE {}",
        json!({
            "case": "gate-path",
            "gateExit": gate.status.code(),
            "gateMessage": gate_err.trim(),
            "gateStatus": gate_report["status"].clone(),
            "gateFailures": gate_report["failures"].clone(),
        })
    );

    // --- source-binding negative 1: the record is tampered consistently in
    // both published copies, so the copy check passes and the refusal has to
    // come from the audit's own source rebuild.
    let scene_path = dir.0.join("scenes/scene-0/scene.json");
    let plan_path = dir.0.join("scenes/scene-0/road-network-plan.json");
    let scene_index = read(&scene_path);
    let plan_copy = read(&plan_path);
    let original_hash = plan_copy["naturalHeightfieldHash"].clone();
    assert!(original_hash.is_string());
    let forged_hash = json!("0000000000000000000000000000000000000000000000000000000000000000");
    let mut forged_plan = plan_copy.clone();
    forged_plan["naturalHeightfieldHash"] = forged_hash.clone();
    let mut forged_scene = scene_index.clone();
    forged_scene["roadNetworkPlan"]["naturalHeightfieldHash"] = forged_hash;
    write(&plan_path, &forged_plan);
    write(&scene_path, &forged_scene);
    let forged = glyphweave(&dir, "quality-report");
    let (_, forged_err) = streams(&forged);
    assert!(
        !forged.status.success(),
        "a record that its own manifest cannot reproduce must be refused: {forged_err}"
    );
    assert!(
        forged_err.contains("road plan differs from natural raster reconstruction"),
        "the refusal must be the audit's own source rebuild, not an unrelated check: {forged_err}"
    );
    write(&plan_path, &plan_copy);
    write(&scene_path, &scene_index);
    assert!(
        glyphweave(&dir, "quality-report").status.success(),
        "restoring the published plan must restore the positive result"
    );
    println!(
        "R4-EVIDENCE {}",
        json!({
            "case": "negative-tampered-source-record",
            "tamperedField": "naturalHeightfieldHash (both published copies)",
            "qualityReportExit": forged.status.code(),
            "refusal": forged_err.trim(),
        })
    );

    // --- source-binding negative 2: a wrong seed on the trusted input side.
    let manifest_path = dir.0.join("glyphweave.manifest.json");
    let published_manifest = read(&manifest_path);
    let mut wrong_seed = published_manifest.clone();
    wrong_seed["world"]["seed"] = json!(manifest.world.seed ^ 0x1);
    write(&manifest_path, &wrong_seed);
    let wrong = glyphweave(&dir, "quality-report");
    let (_, wrong_err) = streams(&wrong);
    assert!(
        !wrong.status.success(),
        "a wrong effective seed must be refused: {wrong_err}"
    );
    write(&manifest_path, &published_manifest);
    println!(
        "R4-EVIDENCE {}",
        json!({
            "case": "negative-wrong-seed",
            "worldSeed": manifest.world.seed ^ 0x1,
            "qualityReportExit": wrong.status.code(),
            "refusal": wrong_err.trim(),
        })
    );
}
