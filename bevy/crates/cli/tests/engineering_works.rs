use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
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

#[test]
fn terrain_works_and_engineered_drainage_are_published_and_gated() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/c01-engineering-works-512.manifest.json");
    let mut manifest: WorldManifest =
        serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    let dir = TestDir::new("engineering-works");
    bake_world(&manifest, &dir.0).unwrap();

    let scene = read(&dir.0.join("scenes/scene-0/scene.json"));
    let works = &scene["engineeredTerrain"]["retainingWorks"];
    assert_eq!(works["status"], "pass", "{works}");
    assert!(
        works["retainingWalls"].as_array().unwrap().len() > 0,
        "the hillside control must produce real retaining walls"
    );
    assert!(
        works["culverts"].as_array().unwrap().len() > 0,
        "the structured-water control must produce culverts"
    );
    let impact = &scene["engineeredTerrain"]["drainageImpact"];
    assert_eq!(impact["status"], "pass", "{impact}");
    assert!(
        impact["changedFlows"].as_array().unwrap().len() > 0,
        "engineering must actually change some sampled flow path"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .args(["world-quality-gate"])
        .arg(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let gate = read(&dir.0.join("world-quality-gate.json"));
    let failures = gate["failures"].as_array().unwrap();
    assert!(
        !failures
            .iter()
            .any(|failure| failure.as_str().unwrap().contains("retaining works")
                || failure.as_str().unwrap().contains("engineered drainage")),
        "the published engineering evidence must pass its gate checks: {failures:?}"
    );
}
