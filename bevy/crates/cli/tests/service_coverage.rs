use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
use std::{fs, path::PathBuf};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glyphweave-service-coverage-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn village_profile_radius_is_consumed_by_a_real_coverage_audit() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/morphology/b04-village-field.manifest.json");
    let mut manifest: WorldManifest =
        serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
    );
    let dir = TestDir::new();
    bake_world(&manifest, &dir.0).unwrap();
    let coverage: Value = serde_json::from_slice(
        &fs::read(dir.0.join("scenes/scene-0/service-coverage-audit.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(coverage["status"], "pass", "{coverage}");
    assert_eq!(coverage["radiusM"], 800.0);
    assert_eq!(coverage["dwellings"], 18);
    assert_eq!(coverage["coveredDwellings"], 18);
    assert!(coverage["uncoveredEntityIds"].as_array().unwrap().is_empty());
    let services = coverage["services"].as_array().unwrap();
    assert_eq!(services.len(), 4);
    assert!(
        services
            .iter()
            .all(|service| service["dwellingsWithinRadius"].as_u64() == Some(18))
    );
}
