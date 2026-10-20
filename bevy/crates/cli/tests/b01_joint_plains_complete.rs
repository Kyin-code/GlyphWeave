use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glyphweave-b01-complete-{}-{nonce}",
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

fn read(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_512m_joint_plains_slice_closes_housing_services_and_land_cover() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/b01-joint-plains-complete-512.manifest.json");
    let mut manifest: WorldManifest =
        serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
    );
    let dir = TestDir::new();
    bake_world(&manifest, &dir.0).unwrap();
    let scene_dir = dir.0.join("scenes/scene-0");
    let joint = read(scene_dir.join("joint-layout-audit.json"));
    let services = read(scene_dir.join("service-program-audit.json"));
    let landcover = read(scene_dir.join("landcover-audit.json"));
    let ledger = read(scene_dir.join("land-use-ledger.json"));
    let budget = read(scene_dir.join("land-use-budget.json"));
    assert_eq!(joint["status"], "pass", "{joint}");
    assert_eq!(joint["plannedResidential"], 18);
    assert_eq!(joint["realizedResidentialWithAccess"], 18);
    assert_eq!(joint["missingResidential"], 0);
    assert_eq!(services["status"], "pass", "{services}");
    assert_eq!(services["realizedWithAccess"], 4);
    assert_eq!(landcover["status"], "pass", "{landcover}");
    assert_eq!(landcover["unresolvedAreaM2"], 0.0);
    assert_eq!(ledger["status"], "pass", "{ledger}");
    assert_eq!(ledger["unresolvedM2"], 0.0);
    assert_eq!(budget["entityCoverageStatus"], "realized");

    let output = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .args(["world-quality-gate"])
        .arg(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let gate = read(dir.0.join("world-quality-gate.json"));
    assert_eq!(gate["status"], "pass", "{gate}");
    assert!(gate["failures"].as_array().unwrap().is_empty());
}
