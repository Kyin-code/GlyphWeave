use glyphweave_core::worldgen::{
    LandmarkSpec, PatchOperation, WorldManifest, WorldPatch, bake_world, world_manifest_revision,
};
use serde_json::json;
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

fn manifest() -> WorldManifest {
    let mut manifest = WorldManifest::default_demo();
    manifest.scenes[0].width_m = 512;
    manifest.scenes[0].depth_m = 512;
    manifest.style["naturalOnly"] = json!(true);
    let landmark: LandmarkSpec = serde_json::from_value(json!({
        "entityId":"generated.inventory-rock",
        "name":"inventory rock",
        "type":"rock",
        "purpose":"inventory test",
        "description":"stable authored object",
        "sceneId":manifest.scenes[0].scene_id,
        "worldX":256,
        "worldZ":256,
        "widthM":2,
        "depthM":2,
        "heightM":1,
        "assetId":"prop.rock"
    }))
    .unwrap();
    manifest.landmarks = vec![landmark];
    manifest
}

fn move_patch(manifest: &WorldManifest) -> WorldPatch {
    WorldPatch {
        format: "glyphweave-world-patch".into(),
        version: 2,
        patch_id: "inventory-move".into(),
        base_revision: world_manifest_revision(manifest).unwrap(),
        parent_patch_hash: Some(manifest.patch_chain_hash.clone()),
        sequence: 1,
        operations: vec![PatchOperation::MoveLandmark {
            entity_id: "generated.inventory-rock".into(),
            world_x: 257,
            world_z: 256,
            world_y: -2,
        }],
    }
}

fn run(base: &PathBuf, manifest: &PathBuf, patch: &PathBuf, approvals: &PathBuf, out: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("apply-patch-preserving")
        .arg(base)
        .arg(manifest)
        .arg(patch)
        .arg(approvals)
        .arg(out)
        .output()
        .unwrap()
}

#[test]
fn preserving_patch_requires_exact_approval_and_publishes_only_after_audit() {
    let root = TestDir::new("inventory-preserving");
    let base_dir = root.0.join("base");
    let manifest = manifest();
    bake_world(&manifest, &base_dir).unwrap();
    let manifest_path = root.0.join("manifest.json");
    let patch_path = root.0.join("patch.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    fs::write(
        &patch_path,
        serde_json::to_vec_pretty(&move_patch(&manifest)).unwrap(),
    )
    .unwrap();

    let denied_approvals = root.0.join("denied-approvals.json");
    fs::write(
        &denied_approvals,
        serde_json::to_vec_pretty(&json!({
            "format":"glyphweave.entity-change-approvals",
            "version":1,
            "approvals":[]
        }))
        .unwrap(),
    )
    .unwrap();
    let denied_out = root.0.join("denied-world");
    let denied = run(
        &base_dir,
        &manifest_path,
        &patch_path,
        &denied_approvals,
        &denied_out,
    );
    assert!(
        !denied.status.success(),
        "unapproved move must fail: {}",
        String::from_utf8_lossy(&denied.stderr)
    );
    assert!(!denied_out.exists(), "failed inventory audit must not publish");

    let approved_approvals = root.0.join("approved-approvals.json");
    fs::write(
        &approved_approvals,
        serde_json::to_vec_pretty(&json!({
            "format":"glyphweave.entity-change-approvals",
            "version":1,
            "approvals":[
                {"entityId":"generated.inventory-rock","change":"moved"},
                {"entityId":"generated.inventory-rock","change":"updated"}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let approved_out = root.0.join("approved-world");
    let approved = run(
        &base_dir,
        &manifest_path,
        &patch_path,
        &approved_approvals,
        &approved_out,
    );
    assert!(
        approved.status.success(),
        "approved move must publish: {}",
        String::from_utf8_lossy(&approved.stderr)
    );
    let audit: serde_json::Value = serde_json::from_slice(
        &fs::read(approved_out.join("entity-inventory-audit.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(audit["status"], "pass");
    assert!(audit["entries"].as_array().unwrap().iter().any(|entry| {
        entry["entityId"] == "generated.inventory-rock"
            && entry["changes"]
                .as_array()
                .is_some_and(|changes| {
                    changes.contains(&json!("moved")) && changes.contains(&json!("updated"))
                })
    }));
}
