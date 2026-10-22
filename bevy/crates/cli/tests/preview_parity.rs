//! A-06 contract: the independent `audit-preview-stream` parity must pass a
//! fresh bake and fail on tampered projection, header, or revision.
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
fn manifest(seed: u64) -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = seed;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.landmarks.clear();
    m.style["rulesDir"] = serde_json::json!(rules());
    m.style["rulesMode"] = serde_json::json!("rules");
    m.style["generationProfile"] = serde_json::json!("strict");
    m
}
fn run_audit(world: &PathBuf, out: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("audit-preview-stream")
        .arg(world)
        .arg(out)
        .output()
        .unwrap()
}
fn parity_status(out: &PathBuf) -> String {
    let parity: Value =
        serde_json::from_slice(&fs::read(out.join("parity.json")).unwrap()).unwrap();
    parity["status"].as_str().unwrap().to_string()
}
fn first_chunk(scene_dir: &PathBuf) -> PathBuf {
    let mut entries: Vec<PathBuf> = fs::read_dir(scene_dir.join("render-chunks"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    entries.into_iter().next().unwrap()
}

#[test]
fn preview_parity_passes_fresh_bake_and_rejects_projection_tampering() {
    let dir = TestDir::new("preview-parity");
    let out_root = TestDir::new("preview-parity-out");
    let m = manifest(7);
    bake_world(&m, &dir.0).unwrap();
    let scene_dir = dir.0.join(format!("scenes/{}", m.scenes[0].scene_id));

    // Positive: a fresh bake's projection must pass the independent audit.
    let positive = out_root.0.join("parity-positive");
    let output = run_audit(&dir.0, &positive);
    assert!(
        output.status.success(),
        "positive audit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(parity_status(&positive), "pass");

    // Negative 1: tamper a projected chunk payload. The content-addressed hash
    // no longer matches, and the source comparison must reject it.
    let chunk = first_chunk(&scene_dir);
    let mut value: Value = serde_json::from_slice(&fs::read(&chunk).unwrap()).unwrap();
    value["chunkX"] = serde_json::json!(999_u32);
    fs::write(&chunk, serde_json::to_vec(&value).unwrap()).unwrap();
    let tampered_chunk = out_root.0.join("parity-tampered-chunk");
    let output = run_audit(&dir.0, &tampered_chunk);
    assert!(!output.status.success());
    assert_eq!(parity_status(&tampered_chunk), "fail");

    // Restore the chunk, then tamper the projected header entities instead.
    let restored: Value = serde_json::from_slice(&fs::read(&chunk).unwrap()).unwrap();
    let mut restored = restored;
    restored["chunkX"] = serde_json::json!(0_u32);
    fs::write(&chunk, serde_json::to_vec(&restored).unwrap()).unwrap();

    let header_path = scene_dir.join("preview-scene.json");
    let mut header: Value = serde_json::from_slice(&fs::read(&header_path).unwrap()).unwrap();
    let entities = header["entities"].as_array_mut().unwrap();
    if let Some(first) = entities.first_mut() {
        first["worldX"] = serde_json::json!(123_456_i64);
    }
    fs::write(&header_path, serde_json::to_vec(&header).unwrap()).unwrap();
    let tampered_header = out_root.0.join("parity-tampered-header");
    let output = run_audit(&dir.0, &tampered_header);
    assert!(!output.status.success());
    assert_eq!(parity_status(&tampered_header), "fail");
}

#[test]
fn preview_parity_rejects_existing_output_and_writes_outside_world() {
    let dir = TestDir::new("preview-out-guard");
    let out_root = TestDir::new("preview-out-guard-out");
    let m = manifest(11);
    bake_world(&m, &dir.0).unwrap();

    // Existing output directory must be refused.
    let existing = out_root.0.join("parity-existing");
    fs::create_dir(&existing).unwrap();
    let output = run_audit(&dir.0, &existing);
    assert!(!output.status.success());

    // Output inside the world tree must be refused.
    let inside = dir.0.join("parity-inside");
    let output = run_audit(&dir.0, &inside);
    assert!(!output.status.success());
    assert!(
        !inside.exists(),
        "audit must not create output inside the world"
    );
}

#[test]
fn preview_parity_header_fields_are_independently_bound_to_source() {
    let dir = TestDir::new("preview-header-binding");
    let out_root = TestDir::new("preview-header-binding-out");
    let mut m = manifest(7);
    m.scenes[0].width_m = 1024;
    m.scenes[0].depth_m = 512;
    bake_world(&m, &dir.0).unwrap();
    let scene_dir = dir.0.join(format!("scenes/{}", m.scenes[0].scene_id));
    let header_path = scene_dir.join("preview-scene.json");
    let original = fs::read(&header_path).unwrap();

    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        (
            "sceneId",
            Box::new(|h| h["sceneId"] = serde_json::json!("tampered")),
        ),
        (
            "widthM",
            Box::new(|h| h["widthM"] = serde_json::json!(4097)),
        ),
        (
            "depthM",
            Box::new(|h| h["depthM"] = serde_json::json!(4097)),
        ),
        (
            "originX",
            Box::new(|h| h["originX"] = serde_json::json!(12345)),
        ),
        (
            "originZ",
            Box::new(|h| h["originZ"] = serde_json::json!(12345)),
        ),
        (
            "chunkSizeM",
            Box::new(|h| h["chunkSizeM"] = serde_json::json!(511)),
        ),
        (
            "chunkCountX",
            Box::new(|h| h["chunkCountX"] = serde_json::json!(99)),
        ),
        (
            "chunkCountZ",
            Box::new(|h| h["chunkCountZ"] = serde_json::json!(99)),
        ),
        (
            "chunks-heightFile",
            Box::new(|h| h["chunks"][0]["heightFile"] = serde_json::json!("missing.height.bin")),
        ),
        (
            "chunks-naturalHeightFile",
            Box::new(|h| {
                h["chunks"][0]["naturalHeightFile"] = serde_json::json!("missing.natural.bin")
            }),
        ),
        (
            "chunks-surfaceFile",
            Box::new(|h| h["chunks"][0]["surfaceFile"] = serde_json::json!("missing.surface.bin")),
        ),
        (
            "chunks-terrainFile",
            Box::new(|h| h["chunks"][0]["terrainFile"] = serde_json::json!("missing.terrain.bin")),
        ),
        (
            "chunks-worldX",
            Box::new(|h| h["chunks"][0]["worldX"] = serde_json::json!(99999)),
        ),
        (
            "chunks-worldZ",
            Box::new(|h| h["chunks"][0]["worldZ"] = serde_json::json!(99999)),
        ),
        (
            "chunks-validWidthM",
            Box::new(|h| h["chunks"][0]["validWidthM"] = serde_json::json!(1)),
        ),
        (
            "chunks-validDepthM",
            Box::new(|h| h["chunks"][0]["validDepthM"] = serde_json::json!(1)),
        ),
        (
            "chunks-order",
            Box::new(|h| h["chunks"].as_array_mut().unwrap().swap(0, 1)),
        ),
        (
            "chunks-remove",
            Box::new(|h| {
                h["chunks"].as_array_mut().unwrap().remove(0);
            }),
        ),
        (
            "chunks-unknown-field",
            Box::new(|h| h["chunks"][0]["forged"] = serde_json::json!(true)),
        ),
        (
            "renderStream-format",
            Box::new(|h| h["renderStream"]["format"] = serde_json::json!("forged")),
        ),
        (
            "renderStream-version",
            Box::new(|h| h["renderStream"]["version"] = serde_json::json!(2)),
        ),
        (
            "renderStream-worldRevision",
            Box::new(|h| h["renderStream"]["worldRevision"] = serde_json::json!("forged")),
        ),
        (
            "renderStream-totalPatches",
            Box::new(|h| h["renderStream"]["totalPatches"] = serde_json::json!(999999)),
        ),
        (
            "renderStream-totalPlants",
            Box::new(|h| h["renderStream"]["totalPlants"] = serde_json::json!(999999)),
        ),
        (
            "renderStream-totalEntities",
            Box::new(|h| h["renderStream"]["totalEntities"] = serde_json::json!(999999)),
        ),
        (
            "renderStream-unknown-field",
            Box::new(|h| h["renderStream"]["forged"] = serde_json::json!(true)),
        ),
        (
            "header-entities",
            Box::new(|h| h["entities"] = serde_json::json!([{"forged": true}])),
        ),
        (
            "header-accessSurfaces",
            Box::new(|h| h["accessSurfaces"] = serde_json::json!([{"forged": true}])),
        ),
        (
            "header-landmarks",
            Box::new(|h| h["landmarks"] = serde_json::json!([{"forged": true}])),
        ),
        (
            "header-developmentPlan",
            Box::new(|h| h["developmentPlan"] = serde_json::json!({"forged": true})),
        ),
        (
            "header-landcoverPlan",
            Box::new(|h| h["landcoverPlan"] = serde_json::json!({"forged": true})),
        ),
        (
            "header-landUseZones",
            Box::new(|h| h["landUseZones"] = serde_json::json!([{"forged": true}])),
        ),
        (
            "header-landUseBudget",
            Box::new(|h| h["landUseBudget"] = serde_json::json!({"forged": true})),
        ),
        (
            "header-roadNetworkPlan",
            Box::new(|h| h["roadNetworkPlan"] = serde_json::json!({"forged": true})),
        ),
        (
            "header-landcoverSurfaces",
            Box::new(|h| h["landcoverSurfaces"] = serde_json::json!({"forged": true})),
        ),
        (
            "header-unknown-field",
            Box::new(|h| h["forged"] = serde_json::json!(true)),
        ),
    ];

    for (name, mutate) in cases {
        let mut header: Value = serde_json::from_slice(&original).unwrap();
        mutate(&mut header);
        fs::write(&header_path, serde_json::to_vec(&header).unwrap()).unwrap();
        let out = out_root.0.join(format!("case-{name}"));
        let output = run_audit(&dir.0, &out);
        assert!(
            !output.status.success(),
            "{name} unexpectedly passed the independent audit"
        );
        assert_eq!(parity_status(&out), "fail", "{name} must fail parity");
    }
    fs::write(&header_path, original).unwrap();
}

#[test]
fn preview_parity_records_hash_bound_audit_input() {
    let dir = TestDir::new("preview-audit-input");
    let out_root = TestDir::new("preview-audit-input-out");
    let m = manifest(7);
    bake_world(&m, &dir.0).unwrap();
    let out = out_root.0.join("parity");
    let output = run_audit(&dir.0, &out);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parity: Value =
        serde_json::from_slice(&fs::read(out.join("parity.json")).unwrap()).unwrap();
    let scene = &parity["scenes"][0];
    let binding = &scene["auditInput"];
    let relative = binding["path"].as_str().unwrap();
    assert!(!relative.contains(".."));
    let bytes = fs::read(out.join(relative)).unwrap();
    assert_eq!(bytes.len() as u64, binding["bytes"].as_u64().unwrap());
    assert_eq!(binding["sha256"].as_str().unwrap().len(), 64);
    let source: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(source["sceneId"], scene["sceneId"]);
    assert_eq!(source["worldRevision"], parity["worldRevision"]);
}
