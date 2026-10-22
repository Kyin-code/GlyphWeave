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
fn write(path: &PathBuf, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn run_quality(dir: &TestDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .args(["quality-report"])
        .arg(&dir.0)
        .output()
        .unwrap()
}

#[test]
fn residual_policy_is_bound_to_the_frozen_manifest() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/b03-land-execution.manifest.json");
    let mut manifest: WorldManifest = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    let dir = TestDir::new("residual-source");
    bake_world(&manifest, &dir.0).unwrap();
    assert!(run_quality(&dir).status.success());

    let scene_path = dir.0.join("scenes/scene-0/scene.json");
    let plan_path = dir.0.join("scenes/scene-0/landcover-plan.json");
    let manifest_path = dir.0.join("glyphweave.manifest.json");
    let scene = read(&scene_path);
    let plan = read(&plan_path);
    let published_manifest = read(&manifest_path);

    let mut changed_scene = scene.clone();
    changed_scene["landcoverPlan"]["residualProgram"] =
        serde_json::json!({"urbanOpenSpaceRatio":0.0,"ruralPastureRatio":0.0});
    let mut changed_plan = plan.clone();
    changed_plan["residualProgram"] =
        serde_json::json!({"urbanOpenSpaceRatio":0.0,"ruralPastureRatio":0.0});
    write(&scene_path, &changed_scene);
    write(&plan_path, &changed_plan);
    assert!(!run_quality(&dir).status.success());

    write(&scene_path, &scene);
    write(&plan_path, &plan);
    let mut removed_scene = scene.clone();
    let mut removed_plan = plan.clone();
    removed_scene["landcoverPlan"]
        .as_object_mut()
        .unwrap()
        .remove("residualProgram");
    removed_plan
        .as_object_mut()
        .unwrap()
        .remove("residualProgram");
    write(&scene_path, &removed_scene);
    write(&plan_path, &removed_plan);
    assert!(!run_quality(&dir).status.success());

    write(&scene_path, &scene);
    write(&plan_path, &plan);
    let mut wrong_version = published_manifest.clone();
    wrong_version["style"]["residualLandProgram"]["version"] = serde_json::json!(2);
    write(&manifest_path, &wrong_version);
    assert!(!run_quality(&dir).status.success());

    write(&manifest_path, &published_manifest);
    let mut unknown_scene = published_manifest.clone();
    unknown_scene["style"]["residualLandProgram"]["scenes"]["missing"] =
        serde_json::json!({"urbanOpenSpaceRatio":0.1,"ruralPastureRatio":0.2});
    write(&manifest_path, &unknown_scene);
    assert!(!run_quality(&dir).status.success());
}
