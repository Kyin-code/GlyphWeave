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

fn manifest() -> WorldManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/b02-road-supply.manifest.json");
    let mut manifest: WorldManifest = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    manifest
}

fn read(path: &PathBuf) -> Value {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap()
}

fn bake(label: &str, max_segments: Option<u64>) -> (TestDir, Value) {
    let mut manifest = manifest();
    match max_segments {
        Some(max) => {
            manifest.style["automaticRoadExtensions"]["scenes"][0]["maxNewSegments"] =
                serde_json::json!(max);
        }
        None => {
            manifest
                .style
                .as_object_mut()
                .unwrap()
                .remove("automaticRoadExtensions");
        }
    }
    let dir = TestDir::new(label);
    bake_world(&manifest, &dir.0).unwrap();
    let plan_path = dir.0.join("scenes/scene-0/road-network-plan.json");
    let plan = if plan_path.exists() {
        read(&plan_path)
    } else {
        serde_json::json!(null)
    };
    (dir, plan)
}

fn cli(args: &[&str], dir: &TestDir) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_glyphweave"));
    command.args(args).arg(&dir.0);
    command.output().unwrap()
}

#[test]
fn accepted_extensions_publish_tier_provenance_and_a_reconstructable_parent_chain() {
    let (with_dir, with_plan) = bake("road-tier-with", Some(64));
    let (without_dir, without_plan) = bake("road-tier-without", None);

    let report = &with_plan["automaticExtensions"];
    assert!(
        report["acceptedSegments"].as_u64().unwrap() > 0,
        "control must accept at least one automatic road"
    );
    assert_eq!(report["unservedTargets"], 0);
    let improved = report["targets"].as_array().unwrap().iter().any(|target| {
        let before_road = target["beforeHasRoad"].as_bool().unwrap_or(false);
        let after_road = target["afterHasRoad"].as_bool().unwrap_or(false);
        let before = target["beforeDistanceM"].as_u64();
        let after = target["afterDistanceM"].as_u64();
        (!before_road && after_road)
            || matches!((before, after), (Some(before), Some(after)) if after < before)
    });
    assert!(
        improved,
        "an accepted extension must improve real zone access, not just add a road"
    );
    assert!(without_plan["automaticExtensions"].is_null());

    let generated: Vec<&str> = report["generatedRoadIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    let mut roles = std::collections::BTreeSet::new();
    let mut saw_base_parent = false;
    for id in &generated {
        let edge = with_plan["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edge| edge["id"] == *id)
            .expect("generated road must have an edge");
        let tier = &edge["tier"];
        assert!(!tier["id"].as_str().unwrap().is_empty());
        let role = tier["role"].as_str().unwrap();
        roles.insert(role.to_string());
        assert!(matches!(role, "collector" | "local" | "farm_access"));

        let extension = with_plan["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|extension| extension["entityId"] == *id)
            .expect("generated road must have an extension record");
        assert_eq!(extension["tier"], *tier);
        let parents = extension["parentRoadIds"].as_array().unwrap();
        assert!(!parents.is_empty());
        for parent in parents {
            let parent = parent.as_str().unwrap();
            assert!(
                with_plan["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|edge| edge["id"] == parent),
                "parent {parent} must exist"
            );
            saw_base_parent |= !generated.contains(&parent);
        }
    }
    assert!(
        roles.contains("collector"),
        "collector tier must be published"
    );
    assert!(saw_base_parent, "the chain must reach a base road");

    let with_quality = cli(&["quality-report"], &with_dir);
    assert!(with_quality.status.success());
    let quality = read(&with_dir.0.join("quality-report.json"));
    assert_eq!(
        quality["scenes"][0]["roadNetworkAudit"]["status"], "pass",
        "the final road audit must pass on the accepted control"
    );

    // The parent chain cannot be forged by editing the published plan.
    let plan_path = with_dir.0.join("scenes/scene-0/road-network-plan.json");
    let mut tampered = with_plan.clone();
    tampered["extensions"][0]["parentRoadIds"][0] = serde_json::json!("missing-parent");
    fs::write(&plan_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
    let validation = cli(&["world-quality-gate"], &with_dir);
    assert!(
        !validation.status.success(),
        "a forged tier parent must fail the authoritative quality gate"
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&validation.stdout),
        String::from_utf8_lossy(&validation.stderr)
    );
    assert!(
        output.contains("road tier")
            || output.contains("parent")
            || output.contains("road network plan"),
        "the refusal must name the tier/parent/plan difference: {output}"
    );
    let _ = without_dir;
}
