//! Default-off freeze for the waterfront opt-in.
//!
//! `style.automaticRoadExtensions.allowStructuredWater` is the only new switch,
//! and `road_network::enabled` still disables road-first for every scene that
//! declares a structured water body unless the manifest states that one string.
//! This fixture is a waterfront world that does NOT state it: nothing about the
//! opt-in may reach it.
//!
//! The evidence is a byte freeze, not a shape assertion: blake3 over the
//! published scene index plus every baked height payload, pinned to the value the
//! pre-opt-in generator produced for the same manifest. A single changed byte in
//! the raster, the entity list or the plan would move it.
use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
use std::{fs, path::PathBuf};

/// blake3 of `scene.json` followed by every `chunk-*.height.bin` payload, in
/// scene-index order. Measured on `11728e0` BEFORE the opt-in existed and again
/// on the opt-in tree: identical, so the default-off waterfront world is
/// byte-identical across the change.
const FROZEN_WATERFRONT_DIGEST: &str =
    "11f9a7179abde1fa480a41479db3ab5afaa446019df2bac39a69a31e8f3e2d7a";

/// The same freeze for a WATERLESS world: the round-21 waterfront pad rule, the
/// shoreline public passage band and their audits are all gated on
/// `allowStructuredWater`, so a scene that declares no water at all has to keep
/// every byte it published before them. Measured on `c9db3e6` (the commit before
/// the pad rule) and again on the round-21 tree: identical.
const FROZEN_WATERLESS_DIGEST: &str =
    "d95e8697be92f38bafa7eacd3b6550c1ece62e4034d6248f13f2b70b7770fb3d";

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

/// A 512 m waterfront town with a structured river through its middle and NO
/// road-first opt-in: the historical waterfront world, unchanged.
fn waterfront_without_opt_in() -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = 42;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.scenes[0].origin_x = 0;
    m.scenes[0].origin_z = 0;
    m.style["generationProfile"] = serde_json::json!("strict");
    m.style["rulesMode"] = serde_json::json!("rules");
    m.style["rulesDir"] = serde_json::json!(rules());
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["water"] = serde_json::json!({
        "waterType": "river",
        "levelPolicy": "horizontal-datum",
        "levelM": 0
    });
    m.landmarks = vec![serde_json::from_value(serde_json::json!({
        "entityId": "landmark.waterfront-river",
        "name": "waterfront river",
        "type": "river",
        "purpose": "structured water the scene must live with",
        "description": "north-south river through the middle of the town",
        "sceneId": "scene-0",
        "worldX": 256,
        "worldZ": 256,
        "worldY": 0,
        "widthM": 16,
        "depthM": 512,
        "heightM": 1,
        "assetId": "water.waterfront-river"
    }))
    .unwrap()];
    m
}

/// A 512 m town that declares NO water at all: the fixture that proves the
/// waterfront work reaches nothing it was not invited into.
fn waterless_town() -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = 42;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.scenes[0].origin_x = 0;
    m.scenes[0].origin_z = 0;
    m.style["generationProfile"] = serde_json::json!("strict");
    m.style["rulesMode"] = serde_json::json!("rules");
    m.style["rulesDir"] = serde_json::json!(rules());
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["water"] = serde_json::json!({
        "waterType": "none",
        "levelPolicy": "horizontal-datum",
        "levelM": 0
    });
    m.landmarks.clear();
    m
}

fn published_digest(dir: &TestDir, scene: &Value) -> String {
    let folder = dir.0.join("scenes/scene-0");
    let mut hasher = blake3::Hasher::new();
    hasher.update(&fs::read(folder.join("scene.json")).unwrap());
    for chunk in scene["chunks"].as_array().unwrap() {
        hasher.update(&fs::read(folder.join(chunk["heightFile"].as_str().unwrap())).unwrap());
    }
    hasher.finalize().to_hex().to_string()
}

#[test]
fn a_waterless_world_is_byte_identical_across_the_waterfront_pad_rule() {
    let dir = TestDir::new("waterless-freeze");
    let manifest = waterless_town();
    bake_world(&manifest, &dir.0).unwrap();
    let scene: Value =
        serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap();
    assert!(
        scene["roadNetworkPlan"].is_object(),
        "a waterless strict world still plans its road network"
    );
    // No band, no pad refusal, no pad level: nothing of the opt-in exists here.
    assert!(
        scene["roadNetworkPlan"]["roadsideSiteRejections"]
            .as_array()
            .is_none_or(|rejections| rejections.iter().all(|rejection| {
                rejection["category"] != "pad_touches_water_or_shore_band"
            })),
        "a waterless world cannot refuse a pad for touching water"
    );
    let digest = published_digest(&dir, &scene);
    assert_eq!(
        digest, FROZEN_WATERLESS_DIGEST,
        "the waterless world changed bytes; every waterfront rule is gated on \
         allowStructuredWater and may not reach a scene without water"
    );
}

#[test]
fn a_waterfront_world_without_the_opt_in_is_byte_identical() {
    let dir = TestDir::new("waterfront-freeze");
    let manifest = waterfront_without_opt_in();
    bake_world(&manifest, &dir.0).unwrap();
    let scene: Value =
        serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap();
    // The gate has not moved: a structured water body still plans no road network
    // and publishes no plan (and therefore no crossing record).
    assert!(
        scene["roadNetworkPlan"].is_null(),
        "a water scene without the opt-in must not carry a road plan"
    );
    assert!(
        !dir.0.join("scenes/scene-0/road-network-plan.json").exists(),
        "a water scene without the opt-in must not publish a road plan"
    );
    // The shoreline world itself still bakes: its entities are the authored
    // landmark set plus the water-compatible detail the legacy generator places.
    assert!(!scene["entities"].as_array().unwrap().is_empty());
    assert!(scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entity| entity["kind"] != "road"));
    let digest = published_digest(&dir, &scene);
    assert_eq!(
        digest, FROZEN_WATERFRONT_DIGEST,
        "the default-off waterfront world changed bytes; nothing about the opt-in may reach an input that did not state it"
    );
}
