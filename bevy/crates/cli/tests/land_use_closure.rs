use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
use std::{fs, path::PathBuf};

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

fn bake(manifest_name: &str, label: &str) -> (TestDir, Value) {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples")
        .join(manifest_name);
    let mut manifest: WorldManifest =
        serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    let dir = TestDir::new(label);
    bake_world(&manifest, &dir.0).unwrap();
    let scene = read(&dir.0.join("scenes/scene-0/scene.json"));
    (dir, scene)
}

#[test]
fn complete_ground_closes_the_ledger_and_its_independent_surface_audit() {
    let (dir, scene) = bake("b03-complete-ground-512.manifest.json", "land-closure");
    let landcover = read(&dir.0.join("scenes/scene-0/landcover-audit.json"));
    let ledger = read(&dir.0.join("scenes/scene-0/land-use-ledger.json"));
    let budget = read(&dir.0.join("scenes/scene-0/land-use-budget.json"));
    let development = read(&dir.0.join("scenes/scene-0/development-audit.json"));

    assert_eq!(landcover["status"], "pass", "{landcover}");
    assert_eq!(landcover["unresolvedAreaM2"], 0.0);
    assert!(
        landcover["groundCoverAreaM2"].as_f64().unwrap() > 0.,
        "the baseline must realize real ground, not merely flip a status"
    );
    assert_eq!(ledger["status"], "pass", "{ledger}");
    assert_eq!(ledger["unresolvedM2"], 0.0);
    assert_eq!(ledger["residual"]["status"], "pass", "{ledger}");
    assert_eq!(budget["entityCoverageStatus"], "realized", "{budget}");
    assert_eq!(budget["unassignedM2"], 0.0);
    assert!(
        budget["actualNatureM2"].as_f64().unwrap() > 0.,
        "nature coverage must be measured, not hardcoded to zero"
    );
    assert_eq!(development["status"], "pass", "{development}");
    assert!(
        development["incompleteReasons"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // The scene, not only the report, carries the opt-in statement.
    assert_eq!(
        scene["landcoverPlan"]["residualProgram"]["coverage"],
        "complete-ground-v1"
    );
}

