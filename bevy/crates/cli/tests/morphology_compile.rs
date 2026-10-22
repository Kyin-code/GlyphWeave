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
    let compiled =
        morphology_compile::audit_published(&published, &manifest.style, &manifest.scenes)
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
    assert_eq!(
        compiled.style["automaticRoadExtensions"]["version"],
        json!(2)
    );
    // Public space is not pasture: the profile fills the urban open-space share
    // and the pasture share stays unstated, so the residual-land consumer's
    // named default (0) applies to it.
    assert_eq!(
        compiled.style["residualLandProgram"],
        json!({"version": 1, "scenes": {"scene-0": {"urbanOpenSpaceRatio": 0.10}}})
    );
    assert!(compiled.report.notes.iter().any(|note| {
        note.contains("residualLandProgram.scenes.scene-0.ruralPastureRatio is unstated")
    }));

    // Tampering with the published document is rejected.
    let mut tampered = published.clone();
    tampered["effectiveStyle"]["frontageProgram"]["requests"][0]["parcelInsets"]["sideM"] =
        json!(10);
    assert!(
        morphology_compile::audit_published(&tampered, &manifest.style, &manifest.scenes).is_err()
    );
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

fn read(root: &std::path::Path, path: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(path)).unwrap()).unwrap()
}

/// B-04 closeout: the profile's public-space share on its own now realizes
/// urban public open space. The numbers are the published ones (512 m fixture),
/// the ledger keeps both conservation identities, and the generation side can
/// re-derive the same area from the plan and the terrain — a tampered
/// publication is refused by the real binary.
#[test]
fn profile_public_space_share_realizes_ledger_accounted_urban_open_space() {
    let dir = TempDir::new("morphology-public-space");
    let manifest = control_manifest("compact-mixed-block");
    manifest.validate().expect("control manifest validates");
    bake_world(&manifest, &dir.0).expect("control world bakes");

    let report = |command: &str| {
        Command::new(glyphweave())
            .arg(command)
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let quality = report("quality-report");
    assert!(
        quality.status.success(),
        "{}",
        String::from_utf8_lossy(&quality.stderr)
    );
    let quality: Value = serde_json::from_slice(&quality.stdout).unwrap();
    let scene = &quality["scenes"][0];

    // Before B-04: no residual program was compiled, so this scene realized
    // 0 m2 of public open space. It now states only the profile's share.
    let plan = read(&dir.0, "scenes/scene-0/landcover-plan.json");
    assert_eq!(
        plan["residualProgram"],
        json!({"urbanOpenSpaceRatio": 0.1}),
        "only the public-space share is derived from the profile"
    );
    let audit = &scene["landcoverAudit"];
    assert_eq!(audit["publicOpenSpaceAreaM2"], json!(16443.0));
    assert_eq!(
        audit["pastureAreaM2"],
        json!(0.0),
        "no pasture share was stated"
    );
    assert_eq!(
        audit["residualDefaultedRatios"],
        json!(["ruralPastureRatio"]),
        "the applied default must be named in the report"
    );
    // The open space is real surface area in the urban row, and the row and the
    // scene both conserve.
    let urban = &scene["landUseLedger"]["byPurpose"]["urban"];
    assert_eq!(urban["physical"]["publicOpenSpaceM2"], json!(16443.0));
    assert_eq!(urban["plannedM2"], json!(180224.0));
    assert_eq!(urban["realizedM2"], json!(20963.0));
    assert_eq!(urban["unresolvedM2"], json!(159261.0));
    assert_eq!(
        urban["realizedM2"].as_f64().unwrap() + urban["unresolvedM2"].as_f64().unwrap(),
        urban["plannedM2"].as_f64().unwrap(),
        "urban realized + unresolved must equal the urban plan"
    );
    let ledger = &scene["landUseLedger"];
    assert_eq!(ledger["sceneAreaM2"], json!(262144.0));
    assert_eq!(ledger["realizedM2"], json!(53731.0));
    assert_eq!(ledger["unresolvedM2"], json!(208413.0));
    assert_eq!(
        ledger["realizedM2"].as_f64().unwrap() + ledger["unresolvedM2"].as_f64().unwrap(),
        ledger["sceneAreaM2"].as_f64().unwrap(),
        "the whole scene area must be realized or explicitly unresolved"
    );
    // The generation side can rebuild the same area from plan + terrain; the
    // audit's independent realization equals the published surface sum, and
    // every open-space patch belongs to an urban future-development cell.
    let surfaces = read(&dir.0, "scenes/scene-0/landcover-surfaces.json");
    let cells: std::collections::BTreeMap<String, String> = plan["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cell| {
            (
                cell["id"].as_str().unwrap().to_owned(),
                cell["role"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut published_open_space = 0.0;
    for patch in surfaces["patches"].as_array().unwrap() {
        if patch["material"] != json!("public_open_space") {
            continue;
        }
        published_open_space += patch["areaM2"].as_f64().unwrap();
        assert_eq!(patch["zoneKind"], json!("urban"));
        assert_eq!(
            cells[patch["sourceCellId"].as_str().unwrap()],
            "future_development"
        );
    }
    assert_eq!(published_open_space, audit["publicOpenSpaceAreaM2"]);
    assert_eq!(
        audit["unresolvedAreaM2"],
        json!(208053.0),
        "the public space moved area out of unresolved, it did not fake completion"
    );
    // The share is a per-cell target, not a claim: the realized area can never
    // exceed the summed floor(cell area * share), and every metre it did not
    // realize is a refusal recorded by name in the published document.
    let target: f64 = plan["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|cell| cell["role"] == json!("future_development"))
        .map(|cell| {
            let boundary = &cell["boundary"];
            let area = boundary["widthM"].as_f64().unwrap() * boundary["depthM"].as_f64().unwrap();
            (area * 0.1).floor()
        })
        .sum();
    assert_eq!(target, 17467.0, "the per-cell public-space target");
    assert!(
        published_open_space <= target,
        "open space {published_open_space} m2 exceeds the {target} m2 target"
    );
    assert_eq!(
        target - published_open_space,
        1024.0,
        "the part of the target with no flat, clear, unclaimed metre left stays unresolved"
    );
    // Whatever was scanned and refused is recorded by name; hillside squares are
    // not silently dropped, and the refusals belong to the future districts.
    let residual_refusals = surfaces["rejections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|rejection| {
            cells
                .get(rejection["sourceCellId"].as_str().unwrap())
                .map(String::as_str)
                == Some("future_development")
        })
        .count();
    assert!(residual_refusals > 1_000, "{residual_refusals}");
    assert!(
        surfaces["rejections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rejection| rejection["reason"] == json!("residual_grade_exceeds_25_percent"))
    );

    // Tampering the published share, or deleting it, is refused.
    let effective_path = dir.0.join("effective-programs.json");
    let published: Value = read(&dir.0, "effective-programs.json");
    let mut changed = published.clone();
    changed["effectiveStyle"]["residualLandProgram"]["scenes"]["scene-0"]["urbanOpenSpaceRatio"] =
        json!(0.05);
    fs::write(
        &effective_path,
        serde_json::to_vec_pretty(&changed).unwrap(),
    )
    .unwrap();
    let rejected = report("validate-world");
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("effective-programs.json differs from the recompiled manifest+profile"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    let mut removed = published.clone();
    removed["effectiveStyle"]["residualLandProgram"]["scenes"]["scene-0"]
        .as_object_mut()
        .unwrap()
        .remove("urbanOpenSpaceRatio");
    fs::write(
        &effective_path,
        serde_json::to_vec_pretty(&removed).unwrap(),
    )
    .unwrap();
    assert!(!report("validate-world").status.success());
    fs::write(
        &effective_path,
        serde_json::to_vec_pretty(&published).unwrap(),
    )
    .unwrap();
    let restored = report("validate-world");
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );

    // The published *plan* is bound to the authoritative style too: editing the
    // realized policy there is refused by the quality gate.
    let plan_path = dir.0.join("scenes/scene-0/landcover-plan.json");
    let mut forged_plan = plan.clone();
    forged_plan["residualProgram"] = json!({"urbanOpenSpaceRatio": 0.2});
    fs::write(&plan_path, serde_json::to_vec_pretty(&forged_plan).unwrap()).unwrap();
    assert!(!report("quality-report").status.success());
    fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    assert!(report("quality-report").status.success());
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
    tampered["effectiveStyle"]["frontageProgram"]["requests"][1]["parcelInsets"]["sideM"] =
        json!(10);
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
