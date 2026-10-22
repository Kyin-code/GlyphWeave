//! B02-W1 closeout: the graded road geometry revision is published as part of
//! the world identity, is rebuilt from the original manifest/effective programs
//! by the independent audit, and missing or tampered tags are rejected.
//!
//! The fixture is self-contained: one temporary 512m bake, no historical work
//! directory, and the temporary tree is removed on drop.
use glyphweave_core::settlement::morphology_compile;
use glyphweave_core::worldgen::{
    GRADED_ROAD_GEOMETRY_REVISION, WorldManifest, bake_world, road_geometry_revision_for_style,
};
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

/// A profile-bearing control: the compiled hierarchy always carries the 16m
/// collector tier, so the road geometry revision must be bound.
fn control_manifest() -> WorldManifest {
    let mut manifest = WorldManifest::default_demo();
    manifest.world.seed = 42;
    manifest.scenes[0].width_m = 512;
    manifest.scenes[0].depth_m = 512;
    manifest.landmarks.clear();
    manifest.style["heightPrecision"] = json!("millimeter");
    manifest.style["generationProfile"] = json!("strict");
    manifest.style["rulesMode"] = json!("rules");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["morphologyProfile"] = json!("compact-mixed-block");
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
            "maxNewSegments": 8,
            "maxCandidateAttempts": 1024,
            "segmentLengthsM": [128, 64],
            "priorityPurposes": ["urban", "rural"],
            "reachDistanceM": 96
        }]
    });
    manifest
}

#[test]
fn published_geometry_revision_is_rebuilt_and_missing_or_wrong_tags_are_rejected() {
    let dir = TempDir::new("graded-geometry-revision");
    let manifest = control_manifest();
    bake_world(&manifest, &dir.0).expect("control world bakes");

    // The tag is published, and an independent recompilation of the original
    // manifest predicts exactly that value.
    let world_path = dir.0.join("world.json");
    let mut world: Value = serde_json::from_slice(&fs::read(&world_path).unwrap()).unwrap();
    assert_eq!(
        world["roadGeometryRevision"],
        json!(GRADED_ROAD_GEOMETRY_REVISION)
    );
    let compiled = morphology_compile::compile(&manifest.style, &manifest.scenes)
        .unwrap()
        .expect("profile compiles");
    assert_eq!(
        road_geometry_revision_for_style(&compiled.style),
        Some(GRADED_ROAD_GEOMETRY_REVISION)
    );

    let validate = || {
        Command::new(glyphweave())
            .arg("validate-world")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = validate();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A wrong tag is rejected.
    world["roadGeometryRevision"] = json!("b02-w1-graded-band-v1");
    fs::write(&world_path, serde_json::to_vec_pretty(&world).unwrap()).unwrap();
    let rejected = validate();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("road geometry revision mismatch"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );

    // A missing tag is rejected too: an affected product may not silently claim
    // the historical identity.
    world
        .as_object_mut()
        .unwrap()
        .remove("roadGeometryRevision");
    fs::write(&world_path, serde_json::to_vec_pretty(&world).unwrap()).unwrap();
    let missing = validate();
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("road geometry revision mismatch"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    // Restoring the published tag restores acceptance.
    world["roadGeometryRevision"] = json!(GRADED_ROAD_GEOMETRY_REVISION);
    fs::write(&world_path, serde_json::to_vec_pretty(&world).unwrap()).unwrap();
    assert!(validate().status.success());
}

#[test]
fn a_legacy_input_without_graded_roads_keeps_the_historical_identity() {
    let mut manifest = WorldManifest::default_demo();
    manifest.scenes[0].width_m = 512;
    manifest.scenes[0].depth_m = 512;
    assert!(road_geometry_revision_for_style(&manifest.style).is_none());
    // No geometry binding, so the derived revision is exactly the payload
    // without a geometry key.
    let derived = glyphweave_core::worldgen::world_manifest_revision(&manifest).unwrap();
    let legacy =
        glyphweave_core::worldgen::world_manifest_revision_with_geometry(&manifest, None, None)
            .unwrap();
    assert_eq!(derived, legacy);
}
