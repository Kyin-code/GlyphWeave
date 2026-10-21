//! Round 19: the waterfront opt-in world must bake END TO END, and the one way
//! reconciliation may remove a feature must be auditable.
//!
//! The fixture is the Round 19 waterfront scene: a 512 m strict/rules world with
//! a 16 m structured river through its middle and
//! `style.automaticRoadExtensions.allowStructuredWater = "bridge-or-causeway"`.
//! Before this test existed, the full bake refused the whole scene with
//!
//! ```text
//! engineering placement stalled in scene-0 at round 1;
//! unresolved=[{ item_id: "generated.storefront.stable-v1.9c5082…",
//!               reason: "slope 8.571427% > max 8%", rule: "storefront.object.toml" }]
//! ```
//!
//! because a *movable* generated feature whose bounded nearby search ran out had
//! no `fallback`-aware exit. Both refused storefronts declare `fallback = "skip"`,
//! so they are now REMOVED BY NAME instead of failing the world; every other
//! numeric contract (8% slope, 6 m roadbed fill, setbacks, entrance aprons,
//! reserve land, water) is untouched, and the water surface is still exactly the
//! natural one (no earthwork in the water).
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::rules::ObjectRegistry;
use glyphweave_core::worldgen::{
    SceneIndex, SceneSpec, WaterGeometry, WorldManifest, bake_world, verify_published_placement,
    water_geometry, water_kind,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::{fs, path::PathBuf, process::Command};

const SCENE_SIZE_M: u32 = 512;
/// The 16 m river the scene has to live with.
const RIVER_WIDTH_M: u32 = 16;

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

fn glyphweave() -> &'static str {
    env!("CARGO_BIN_EXE_glyphweave")
}

/// The Round 19 fixture: 512 m, seed 42, strict + rules, a 16 m river through
/// the middle, and the ONLY switch the waterfront opt-in adds.
fn waterfront_opt_in() -> WorldManifest {
    let mut manifest = WorldManifest::default_demo();
    manifest.world.seed = 42;
    manifest.scenes[0].width_m = SCENE_SIZE_M;
    manifest.scenes[0].depth_m = SCENE_SIZE_M;
    manifest.scenes[0].origin_x = 0;
    manifest.scenes[0].origin_z = 0;
    manifest.style["generationProfile"] = json!("strict");
    manifest.style["rulesMode"] = json!("rules");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["terrainProfile"] = json!("plains");
    manifest.style["heightPrecision"] = json!("millimeter");
    manifest.style["water"] = json!({
        "waterType": "river",
        "levelPolicy": "horizontal-datum",
        "levelM": 0
    });
    manifest.style["automaticRoadExtensions"] = json!({
        "version": 1,
        "allowStructuredWater": "bridge-or-causeway",
        "scenes": [{
            "sceneId": "scene-0",
            "maxNewSegments": 16,
            "maxCandidateAttempts": 4096,
            "segmentLengthsM": [128, 64],
            "priorityPurposes": ["urban", "rural"],
            "reachDistanceM": 96
        }]
    });
    manifest.landmarks = vec![serde_json::from_value(json!({
        "entityId": "landmark.waterfront-river",
        "name": "waterfront river",
        "type": "river",
        "purpose": "structured water the plan has to span",
        "description": "north-south river through the middle of the town",
        "sceneId": "scene-0",
        "worldX": 256,
        "worldZ": 256,
        "worldY": 0,
        "widthM": RIVER_WIDTH_M,
        "depthM": SCENE_SIZE_M,
        "heightM": 1,
        "assetId": "water.waterfront-river"
    }))
    .unwrap()];
    manifest
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap()
}

/// The published raster: `baked` is the final height payload, `natural` is the
/// payload the bake published beside it (no pad, no roadbed, no fill).
fn raster(dir: &TestDir, scene: &SceneIndex, baked: bool) -> BakedTerrain {
    let spec = SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    let mut terrain = BakedTerrain::new(&spec).unwrap();
    let folder = dir.0.join("scenes").join(&scene.scene_id);
    for chunk in &scene.chunks {
        let file = if baked {
            chunk.height_file.clone()
        } else {
            chunk
                .natural_height_file
                .clone()
                .expect("every baked chunk publishes its natural payload")
        };
        let bytes = fs::read(folder.join(file)).unwrap();
        terrain
            .insert_chunk(
                chunk.world_x,
                chunk.world_z,
                chunk.valid_width_m,
                chunk.valid_depth_m,
                &bytes,
            )
            .unwrap();
    }
    terrain
}

/// Wet cells: inside the water body and strictly below the datum. One cell is
/// one square metre, so this count IS the water surface area.
fn wet_area(terrain: &BakedTerrain, water: WaterGeometry, scene: &SceneIndex) -> u64 {
    let mut wet = 0;
    for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
        for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
            if water.contains_point(f64::from(x), f64::from(z))
                && terrain.height(x, z).is_some_and(|h| h < 0.)
            {
                wet += 1;
            }
        }
    }
    wet
}

fn quality_report(dir: &TestDir) -> Value {
    let output = Command::new(glyphweave())
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "quality-report failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The full bake: roads, registered crossings, the unchanged water surface, and
/// building placement that treats those roads as hard occupancy.
#[test]
fn the_waterfront_opt_in_world_bakes_with_named_skips_and_no_water_earthwork() {
    let dir = TestDir::new("waterfront-optin-bake");
    let manifest = waterfront_opt_in();
    manifest.validate().expect("fixture validates");
    bake_world(&manifest, &dir.0).expect("the opt-in waterfront world must bake end to end");

    let scene = scene_index(&dir);

    // Roads: the opt-in authorizes the network, so the scene carries a road.
    let roads = scene
        .entities
        .iter()
        .filter(|entity| entity.kind == "road")
        .count();
    assert_eq!(roads, 38, "the waterfront world's road entities");
    assert!(
        scene
            .road_network_plan
            .as_ref()
            .is_some_and(|plan| !plan.water_crossings.is_empty()),
        "the opt-in world must publish its registered water crossings"
    );
    // One registered structure per road that touches the water, and nothing is
    // filled in the water: every deck sits 7 m above the datum, the span is 15 m
    // of the 16 m channel, and the pavement band holds 225 wet cells.
    let plan = scene.road_network_plan.as_ref().unwrap();
    assert_eq!(plan.water_crossings.len(), 5, "registered water crossings");
    for crossing in &plan.water_crossings {
        assert_eq!(crossing.kind, "bridge");
        assert_eq!(crossing.water_id, "water.scene-0.river");
        assert_eq!(crossing.water_kind, "river");
        assert_eq!(crossing.width_m, 8);
        assert_eq!(crossing.length_m, 15);
        assert_eq!(crossing.wet_cells, 225);
        assert_eq!(crossing.deck_level_mm, 7000);
        assert_eq!(crossing.water_level_mm, 0);
        assert!(
            crossing.bed_max_mm <= crossing.water_level_mm,
            "no earthwork in the water: the bed stays below the datum"
        );
    }

    // The water surface is EXACTLY the natural one: the bake publishes both
    // payloads and the wet cells must be identical.
    let water = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &manifest.scenes[0],
        &manifest.style,
    );
    let baked_wet = wet_area(&raster(&dir, &scene, true), water, &scene);
    let natural_wet = wet_area(&raster(&dir, &scene, false), water, &scene);
    assert_eq!(
        natural_wet, BAKED_WATER_SURFACE_M2,
        "the natural water surface of this fixture"
    );
    assert_eq!(
        baked_wet, natural_wet,
        "the baked water surface must equal the natural one"
    );

    // Roads are part of placement occupancy now, so the road-colliding
    // storefronts never reach the reconciliation stage as publications.
    let placement = read_json(dir.0.join("engineering-placement.json"));
    let entry = placement["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry.get("inputItems").is_some())
        .expect("the reconciliation report is published among the phase records");
    assert_eq!(entry["sceneId"], json!("scene-0"));
    assert_eq!(entry["rounds"], json!(2));
    assert_eq!(entry["inputItems"], json!(421));
    assert_eq!(entry["outputItems"], json!(420));
    assert_eq!(entry["removedItems"], json!(1));
    let skips = entry["skips"].as_array().unwrap();
    assert_eq!(skips.len(), 1, "exactly one exhausted storefront");
    let ids: Vec<&str> = skips
        .iter()
        .map(|skip| skip["entityId"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["generated.storefront.stable-v1.adce32212580c8037185d3fe07721a8ff8eb1ad465efd8d13f0f57afb5c7c5da"]
    );
    let reasons: Vec<&str> = skips
        .iter()
        .map(|skip| skip["reason"].as_str().unwrap())
        .collect();
    assert_eq!(
        reasons,
        vec!["slope 10.644444% > max 8%"]
    );
    for skip in skips {
        assert_eq!(skip["kind"], json!("storefront"));
        assert_eq!(skip["rule"], json!("storefront.object.toml"));
        assert_eq!(skip["descriptorFallback"], json!("skip"));
        assert_eq!(skip["exit"], json!("search-exhausted"));
        assert_eq!(skip["round"], json!(2));
    }

    // Auditability: the published reported removals recompute against the
    // published scene and the real descriptors, and the number the quality flow
    // will report is the same two.
    let published: BTreeSet<String> = scene
        .entities
        .iter()
        .map(|entity| entity.entity_id.clone())
        .collect();
    let authored: BTreeSet<String> = scene
        .landmarks
        .iter()
        .map(|landmark| landmark.entity_id.clone())
        .collect();
    let registry = ObjectRegistry::load_dir(&rules()).unwrap();
    let audit =
        verify_published_placement(entry, &published, &authored, Some(&registry)).expect(
            "the published skips must recompute from the plan ids and the descriptors",
        );
    assert_eq!(audit.plan_items, 421);
    assert_eq!(audit.output_items, 420);
    assert_eq!(audit.skipped_items, 1);
    assert_eq!(audit.skipped_by_rule["storefront.object.toml"], 1);
    for id in ids {
        assert!(!published.contains(id));
    }

    // The quality flow agrees with the independent recomputation.
    let quality = quality_report(&dir);
    assert_eq!(quality["engineeringPlacementSkips"], json!(1));
    assert_eq!(
        quality["scenes"][0]["engineeringPlacement"]["status"],
        json!("pass")
    );
    assert_eq!(
        quality["scenes"][0]["engineeringPlacement"]["skippedByRule"],
        json!({ "storefront.object.toml": 1 })
    );
}

/// The water surface of the Round 19 fixture, in square metres (one raster cell
/// is one square metre): 15 wet metres across the 16 m channel over 512 m.
const BAKED_WATER_SURFACE_M2: u64 = 7_680;

/// Two bakes of the same input publish identical reconciliation records.
#[test]
fn two_bakes_publish_identical_skip_records() {
    let first = TestDir::new("waterfront-determinism-a");
    let second = TestDir::new("waterfront-determinism-b");
    let manifest = waterfront_opt_in();
    bake_world(&manifest, &first.0).expect("first bake");
    bake_world(&manifest, &second.0).expect("second bake");
    let first_report = fs::read(first.0.join("engineering-placement.json")).unwrap();
    let second_report = fs::read(second.0.join("engineering-placement.json")).unwrap();
    assert_eq!(
        first_report, second_report,
        "the published reconciliation report must be byte-identical across bakes"
    );
    let skips: Value = serde_json::from_slice(&first_report).unwrap();
    let entry = skips["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry.get("inputItems").is_some())
        .unwrap();
    let skips = entry["skips"].as_array().unwrap();
    assert_eq!(skips.len(), 1);
    assert_eq!(
        skips[0]["entityId"],
        json!("generated.storefront.stable-v1.adce32212580c8037185d3fe07721a8ff8eb1ad465efd8d13f0f57afb5c7c5da")
    );
}

/// Forging a removal for an entity that is still published is visible to the
/// quality flow: the report's plan ids no longer conserve.
#[test]
fn the_quality_flow_refuses_a_forged_skip_record() {
    let dir = TestDir::new("waterfront-skip-tamper");
    let manifest = waterfront_opt_in();
    bake_world(&manifest, &dir.0).expect("the opt-in waterfront world must bake");
    let path = dir.0.join("engineering-placement.json");
    let clean = read_json(path.clone());
    let clean_entry = clean["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry.get("inputItems").is_some())
        .unwrap();
    assert_eq!(clean_entry["removedItems"], json!(1));

    // Forging a removal for an entity that is still published is refused too.
    let mut forged = clean.clone();
    let reports = forged["scenes"].as_array_mut().unwrap();
    let entry = reports
        .iter_mut()
        .find(|entry| entry.get("inputItems").is_some())
        .unwrap();
    let still_published = entry["outputEntityIds"][0].clone();
    entry["skips"] = json!([{
        "entityId": still_published,
        "kind": "storefront",
        "reason": "forged removal",
        "rule": "storefront.object.toml",
        "descriptorFallback": "skip",
        "exit": "fixed",
        "round": 0
    }]);
    entry["removedItems"] = json!(1);
    fs::write(&path, serde_json::to_vec_pretty(&forged).unwrap()).unwrap();
    let forged_report = quality_report(&dir);
    assert_eq!(forged_report["status"], json!("fail"));
    assert_eq!(
        forged_report["scenes"][0]["engineeringPlacement"]["status"],
        json!("fail")
    );
}
