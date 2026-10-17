//! B04-C1-R1 contract: the effective programs of a morphology profile are
//! recompiled from the original manifest and the real binary agrees.
//!
//! Every fixture here is created by the test itself inside its own temporary
//! directory. Nothing depends on a historical `work/collaboration` package, so
//! the checks are real in a clean checkout instead of silently skipping.
use glyphweave_core::settlement::morphology_compile;
use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

struct TempDir(PathBuf);
impl TempDir {
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
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn glyphweave() -> &'static str {
    env!("CARGO_BIN_EXE_glyphweave")
}

fn rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
}

/// A self-contained control: authored demand, authored building footprint, no
/// parcel policy. The profile fills only the parcel insets.
fn control_manifest(profile: &str) -> WorldManifest {
    let mut manifest = WorldManifest::default_demo();
    manifest.world.seed = 42;
    manifest.scenes[0].width_m = 512;
    manifest.scenes[0].depth_m = 512;
    manifest.landmarks.clear();
    manifest.style["heightPrecision"] = json!("millimeter");
    manifest.style["generationProfile"] = json!("strict");
    manifest.style["rulesMode"] = json!("rules");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["morphologyProfile"] = json!(profile);
    manifest.style["landUseProfile"] = json!({
        "theme": "temperate-plain",
        "urbanCoreRatio": 0.45,
        "suburbanRatio": 0.25,
        "greenRatio": 0.05,
        "farmRatio": 0.10,
        "forestRatio": 0.05,
        "pastureRatio": 0.05,
        "reserveRatio": 0.05
    });
    manifest.style["automaticRoadExtensions"] = json!({
        "version": 1,
        "scenes": [{
            "sceneId": "scene-0",
            "maxNewSegments": 16,
            "maxCandidateAttempts": 4096,
            "segmentLengthsM": [128, 64],
            "priorityPurposes": ["urban", "rural"],
            "reachDistanceM": 96
        }]
    });
    manifest.style["frontageProgram"] = json!({
        "version": 1,
        "requests": [
            {"id": "homes", "sceneId": "scene-0", "purpose": "urban",
             "kind": "residential_home", "count": 16,
             "widthM": 24, "depthM": 24, "heightM": 8,
             "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192},
            {"id": "shops", "sceneId": "scene-0", "purpose": "urban",
             "kind": "storefront", "count": 8,
             "widthM": 12, "depthM": 10, "heightM": 6,
             "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192}
        ]
    });
    manifest
}

fn compile_manifest(manifest: &WorldManifest) -> Value {
    let compiled = morphology_compile::compile(&manifest.style, &manifest.scenes)
        .expect("compiles")
        .expect("a profile-bearing manifest compiles effective programs");
    serde_json::to_value(&compiled.report).unwrap()
}

/// The published document is re-derived from the authored manifest: demand is
/// preserved, the profile supplies only unstated parameters, and a tampered or
/// recompiled-for-another-profile document is rejected.
#[test]
fn effective_programs_are_recompiled_from_the_authored_manifest() {
    let manifest = control_manifest("compact-mixed-block");
    manifest.validate().expect("control manifest validates");
    let published = compile_manifest(&manifest);
    let compiled = morphology_compile::audit_published(&published, &manifest.style, &manifest.scenes)
        .unwrap()
        .expect("profile publishes effective programs");
    assert_eq!(compiled.report.profile_id, "compact-mixed-block");
    let requests = &compiled.style["frontageProgram"]["requests"];
    // Authored demand and footprint survive exactly.
    assert_eq!(requests[0]["count"], json!(16));
    assert_eq!(requests[1]["count"], json!(8));
    assert_eq!(requests[0]["widthM"], json!(24));
    assert_eq!(requests[0]["depthM"], json!(24));
    // The profile fills the unstated parcel policy and the road hierarchy.
    assert!(requests[0]["parcelInsets"].is_object());
    assert_eq!(compiled.style["automaticRoadExtensions"]["version"], json!(2));
    // Public space is not pasture: no residual program is invented.
    assert!(compiled.style.get("residualLandProgram").is_none());
    assert!(compiled
        .report
        .unsupported
        .iter()
        .any(|entry| entry.profile_field == "publicSpaceRatio"));

    // Tampering with the published document is rejected.
    let mut tampered = published.clone();
    tampered["effectiveStyle"]["frontageProgram"]["requests"][0]["parcelInsets"]["sideM"] = json!(10);
    assert!(morphology_compile::audit_published(&tampered, &manifest.style, &manifest.scenes).is_err());
    // A document recompiled for another profile is rejected.
    let mut other = manifest.style.clone();
    other["morphologyProfile"] = json!("village-field");
    assert!(morphology_compile::audit_published(&published, &other, &manifest.scenes).is_err());
    // An unknown key never survives as a silent default.
    let mut unknown = manifest.style.clone();
    unknown["frontageProgram"]["unexpectedField"] = json!(true);
    let error = morphology_compile::compile(&unknown, &manifest.scenes).unwrap_err();
    assert!(error.contains("unknown field"), "{error}");
}

#[test]
fn the_real_binary_binds_and_rejects_a_tampered_effective_document() {
    let dir = TempDir::new("morphology-cli");
    let manifest = control_manifest("compact-mixed-block");
    bake_world(&manifest, &dir.0).expect("control world bakes");

    // The published document must equal an independent recompilation.
    let published: Value =
        serde_json::from_slice(&fs::read(dir.0.join("effective-programs.json")).unwrap()).unwrap();
    morphology_compile::audit_published(&published, &manifest.style, &manifest.scenes)
        .expect("published effective programs match an independent recompile");

    let output = Command::new(glyphweave())
        .arg("validate-world")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The program audits consume the same compiled input.
    let quality = Command::new(glyphweave())
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        quality.status.success(),
        "{}",
        String::from_utf8_lossy(&quality.stderr)
    );

    // Tamper in place, prove the rejection, then restore and prove the pass.
    let path = dir.0.join("effective-programs.json");
    let mut tampered = published.clone();
    tampered["effectiveStyle"]["frontageProgram"]["requests"][1]["parcelInsets"]["sideM"] = json!(10);
    fs::write(&path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
    let rejected = Command::new(glyphweave())
        .arg("validate-world")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("effective-programs.json differs from the recompiled manifest+profile"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    fs::write(&path, serde_json::to_vec_pretty(&published).unwrap()).unwrap();
    let restored = Command::new(glyphweave())
        .arg("validate-world")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "restoring the original document must restore acceptance: {}",
        String::from_utf8_lossy(&restored.stderr)
    );
}
