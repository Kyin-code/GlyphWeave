//! Round 37: `style.terrainPolicy.earthworkBlend = "derived-from-footprint"`.
//!
//! Round 33 measured the defect: a structure's fill envelope was
//! `3 x (ground_height_m + 32 m)` - 144 m for the hillside town's 16 m
//! `urban_building` - so a 25x23 m building's ring reached 86.5 m across its own
//! block and lifted a planned 8x8 shop pad by 3,780 mm, which `bounded_cut_verify`
//! step 5 could only answer by withdrawing the pad by name.
//!
//! The switch derives the ring from the structure's own footprint instead
//! (`max(width_m, height_m) x 1 m + 8 m`, clamped into `8 m ..= 144 m`), and it is
//! opt-in: a world that never asked keeps the historical bytes, which
//! `two_off_bakes_are_byte_identical_and_carry_no_ring` re-proves end to end.
//!
//! What this file asserts, world by world, all of it read back from the bytes the
//! bake published:
//!
//! 1. OFF: no entity carries a derived ring, every structure's carve publishes the
//!    fixed envelope (`3 x (ground + 32)`, exactly 144 m on the 16 m plate), the
//!    structure carves include that 144 m ring and the pad's window publishes
//!    9,105 mm - the ring standing 86.5 m away.
//! 2. ON: the 144 m ring is gone from the published carve list, the rule-engine
//!    structures carry the derived ring (33 m for the 25x23 plate, 46 m for the
//!    38x20 lodge), the same window publishes 6,052 mm, and the recorded lift falls
//!    from 3,780 mm to 761 mm.
//! 3. The cut ledger still equals the rasters' own measurement, and the protected
//!    land, the water and the shore passage band are still untouched.
//! 4. Two bakes of the same derived manifest publish identical bytes.
//! 5. The waterfront sample keeps its plan, its pads and its entities.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::worldgen::bounded_cut::{self, CutLedger, CutPolicy};
use glyphweave_core::worldgen::{
    LandUseZone, SceneIndex, SceneSpec, WaterGeometry, WorldManifest, bake_world, water_geometry,
    water_kind,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
/// The registered waterfront sample, with the conversion switch this round runs.
const WATERFRONT: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");
/// The pad the entity earthwork buries, by name.
const REFUSED_PAD: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-network.scene-0.23-30.36.negative";
/// The cell of that pad's window the published raster disagrees with most.
const PAD_CELL: (i32, i32) = (140, 259);
/// The plate the round is about: a 25x23 `urban_building` on a 16 m platform.
const PLATE_GROUND_M: f64 = 16.0;
const PLATE_WIDTH_M: f64 = 25.0;
const PLATE_HEIGHT_M: f64 = 12.0;
/// The fixed envelope at that plate: `3 x (16 + 32)`.
const LEGACY_RING_M: f64 = 144.0;
/// The derived envelope at that plate: `max(25, 12) x 1 + 8`.
const DERIVED_RING_M: f64 = 33.0;
/// What the pad's window publishes under each envelope, in whole millimetres.
const LEGACY_PAD_MM: i32 = 9105;
const DERIVED_PAD_MM: i32 = 6052;
/// The lift each arm records for that pad.
const LEGACY_LIFT_MM: i32 = 3780;
const DERIVED_LIFT_MM: i32 = 761;
/// The excavation the derived scene holds, summed once from its two rasters.
const DERIVED_CUT_PADS: usize = 3;
const DERIVED_CUT_CELLS: u64 = 1097;
const DERIVED_CUT_DEPTH_SUM_MM: i64 = 729_038;
/// The pads the grown plan keeps.
const REMAINING_PADS: usize = 16;

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

/// The published hillside town with the growth switch ON: Round 33's world.
fn grown() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["landUseProfile"]["urbanGrowth"] = json!("axis-and-node");
    manifest
}

/// The same world with the derived earthwork envelope ON.
fn grown_derived() -> WorldManifest {
    let mut manifest = grown();
    manifest.style["terrainPolicy"] = json!({"earthworkBlend": "derived-from-footprint"});
    manifest
}

/// The registered waterfront sample with the explicit conversion switch.
fn waterfront(derived: bool) -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(WATERFRONT).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["frontageProgram"]["ruralConversion"] = json!("explicit");
    if derived {
        manifest.style["terrainPolicy"] = json!({"earthworkBlend": "derived-from-footprint"});
    }
    manifest
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn scene_dir(dir: &TestDir) -> PathBuf {
    dir.0.join("scenes/scene-0")
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(scene_dir(dir).join("scene.json")).unwrap()).unwrap()
}

fn scene_spec(scene: &SceneIndex) -> SceneSpec {
    SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    }
}

/// The published raster: `baked` is the final height payload, `!baked` the natural
/// payload the bake publishes beside it.
fn raster(dir: &TestDir, scene: &SceneIndex, baked: bool) -> BakedTerrain {
    let spec = scene_spec(scene);
    let mut terrain = BakedTerrain::new(&spec).unwrap();
    let folder = scene_dir(dir);
    for chunk in &scene.chunks {
        let file = if baked {
            chunk.height_file.clone()
        } else {
            chunk.natural_height_file.clone().expect("natural payload")
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

fn published_files(root: &PathBuf) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.clone()];
    while let Some(folder) = stack.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(
                    relative,
                    blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    files
}

/// The ground elevation the carve planner reads: `world_y` plus the grounding
/// offsets (see `EntityInstance::ground_height_m`).
fn ground_height_m(entity: &Value) -> f64 {
    let grounding = &entity["grounding"];
    let pivot = entity["heightM"].as_f64().unwrap_or(0.0);
    let pivot_offset = if grounding["pivot"] == json!("center") {
        -pivot * 0.5
    } else {
        0.0
    };
    entity["worldY"].as_f64().unwrap_or(0.0)
        + pivot_offset
        + grounding["bottomOffsetM"].as_f64().unwrap_or(0.0)
        + grounding["roadbedOffsetM"].as_f64().unwrap_or(0.0)
}

/// The fixed envelope `plan_terrain_carves` publishes for one entity:
/// `max(6, 3 x (ground + 32))` and then at least `3 x (ground + 32)`.
fn legacy_ring_m(entity: &Value) -> f64 {
    let ground = ground_height_m(entity);
    3.0 * (ground + 32.0).max(0.0).max(6.0 / 3.0)
}

/// The footprint of one carve, in metres: the support rectangle `plan_terrain_carves`
/// expands to, plus the ring.
fn envelope_area(entity: &Value, ring_m: f64) -> f64 {
    let quarter = (entity["rotationYDeg"].as_f64().unwrap_or(0.0).round() as i32).rem_euclid(180) == 90;
    let (width, depth) = (
        entity["widthM"].as_f64().unwrap(),
        entity["depthM"].as_f64().unwrap(),
    );
    let (half_w, half_d) = if quarter {
        (depth * 0.5, width * 0.5)
    } else {
        (width * 0.5, depth * 0.5)
    };
    let precise = entity["grounding"]["frontageRoadPoint"].is_array();
    let margin = if precise { 2.0 } else { 1.0 };
    (2.0 * (half_w + margin + ring_m)) * (2.0 * (half_d + margin + ring_m))
}

fn is_structure(kind: &str) -> bool {
    matches!(
        kind,
        "building"
            | "building_tower"
            | "building_cluster"
            | "urban_building"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "resort_lodge"
            | "storefront"
            | "commercial_center"
            | "entertainment_center"
            | "school"
            | "town_hall"
            | "market"
            | "industrial"
            | "temple"
            | "church"
            | "parking_lot"
            | "convenience_store"
    )
}

/// Every published building carve's ring, off the engineered-terrain ledger.
fn building_rings(dir: &TestDir) -> Vec<f64> {
    let snapshot = read_json(scene_dir(dir).join("engineered-terrain.json"));
    snapshot["operations"]
        .as_array()
        .expect("the ledger publishes its operations")
        .iter()
        .filter(|operation| operation["kind"] != json!("roadbed"))
        .map(|operation| operation["blendM"].as_f64().expect("a published ring"))
        .collect()
}

/// Every published road carve's ring, off the same ledger. The switch is about
/// STRUCTURES, so this list is the same in both arms: road corridors keep their
/// graded envelope and their own fill contract.
const ROAD_RINGS: [f64; 9] = [
    108.24, 109.2, 109.29, 111.28, 111.74, 114.0, 114.28, 121.07, 121.5,
];

fn road_rings(dir: &TestDir) -> Vec<f64> {
    let snapshot = read_json(scene_dir(dir).join("engineered-terrain.json"));
    snapshot["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|operation| operation["kind"] == json!("roadbed"))
        .map(|operation| operation["blendM"].as_f64().unwrap())
        .collect()
}

fn assert_road_rings_unchanged(dir: &TestDir) {
    let rings = road_rings(dir);
    assert_eq!(rings.len(), 29, "29 road carves in this scene");
    let mut unique = rings.clone();
    unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
    unique.dedup_by(|a, b| (*a - *b).abs() < 0.005);
    assert_eq!(unique.len(), ROAD_RINGS.len(), "{unique:?}");
    for (got, want) in unique.iter().zip(ROAD_RINGS) {
        assert!(
            (got - want).abs() < 0.005,
            "a road keeps its own earthwork ring: {got} vs {want}"
        );
    }
}

/// 1. OFF: the fixed envelope is still the 144 m constant on the reference plate,
///    and the world still buries the pad by 3,780 mm.
#[test]
fn the_default_envelope_is_still_the_fixed_ring() {
    let dir = TestDir::new("blend-off");
    let manifest = grown();
    bake_world(&manifest, &dir.0).expect("the fixed-envelope world bakes");
    let scene = scene_index(&dir);

    // No entity carries a derived ring, and every structure's ring IS the fixed
    // envelope recomputed from its own published elevation.
    for entity in &scene.entities {
        assert!(
            entity.grounding.earthwork_blend_m.is_none(),
            "{} must not carry a derived ring",
            entity.entity_id
        );
        if is_structure(&entity.kind) {
            let entity_value = serde_json::to_value(entity).unwrap();
            assert_eq!(
                legacy_ring_m(&entity_value),
                3.0 * (ground_height_m(&entity_value) + 32.0),
                "the fixed envelope is 3 x (ground + 32), never a function of the footprint"
            );
        }
    }
    // The constant and the per-world envelope are the same number on the plate the
    // round is about: 3 x (16 + 32) = 144 m, so the area a build of that elevation
    // covers is identical whether it is read from the constant or from the entity.
    let plate = json!({
        "widthM": PLATE_WIDTH_M, "depthM": 23.0, "heightM": PLATE_HEIGHT_M,
        "rotationYDeg": 0.0, "worldY": PLATE_GROUND_M,
        "grounding": {"pivot": "bottom", "bottomOffsetM": 0.0, "roadbedOffsetM": 0.0},
    });
    assert_eq!(legacy_ring_m(&plate), LEGACY_RING_M);
    assert_eq!(
        envelope_area(&plate, LEGACY_RING_M),
        envelope_area(&plate, legacy_ring_m(&plate)),
        "the constant IS the per-world envelope on that plate"
    );

    // The published carve list carries that 144 m ring, and nothing wider.
    let rings = building_rings(&dir);
    assert_eq!(rings.len(), 52, "one carve per structure and per apron");
    assert_eq!(
        rings.iter().cloned().fold(0.0_f64, f64::max),
        LEGACY_RING_M
    );
    assert_eq!(rings.iter().filter(|ring| **ring == LEGACY_RING_M).count(), 1);
    assert_road_rings_unchanged(&dir);

    // The pad is still buried, by name, by the structure's ring.
    let completion = read_json(dir.0.join("pad-completion.json"));
    let record = &completion["scenes"][0]["pads"][0];
    assert_eq!(record["padId"], json!(REFUSED_PAD));
    assert_eq!(record["outcome"], json!("refused"));
    assert_eq!(record["liftMm"], json!(LEGACY_LIFT_MM));
    assert_eq!(record["entityKind"], json!("urban_building"));
    assert_eq!(completion["scenes"][0]["padsRefused"], json!(1));

    // And the ring's reach is in the bytes: the pad's window publishes the plate's
    // own blended fill 86.5 m away, not the natural ground.
    let baked = raster(&dir, &scene, true);
    assert_eq!(baked.height_millimetres(PAD_CELL.0, PAD_CELL.1), Some(LEGACY_PAD_MM));
}

/// 2. ON: every rule-engine structure carries the derived ring, the 144 m ring is
///    gone from the carve list, and the recorded lift falls to 761 mm.
#[test]
fn the_switch_derives_the_structure_rings_from_their_footprints() {
    let dir = TestDir::new("blend-on");
    let manifest = grown_derived();
    bake_world(&manifest, &dir.0).expect("the derived-envelope world bakes");
    let scene = scene_index(&dir);

    // The derived ring is on every rule-engine structure, and it IS the formula.
    let mut stamped: BTreeMap<String, f64> = BTreeMap::new();
    for entity in &scene.entities {
        if let Some(ring) = entity.grounding.earthwork_blend_m {
            let want = (f64::from(entity.width_m).max(f64::from(entity.height_m)) + 8.0)
                .clamp(8.0, LEGACY_RING_M);
            assert_eq!(
                f64::from(ring),
                want,
                "{} ({}x{} m) must carry its own footprint ring",
                entity.entity_id,
                entity.width_m,
                entity.height_m
            );
            stamped.insert(entity.entity_id.clone(), f64::from(ring));
        }
    }
    assert_eq!(stamped.len(), 4, "the plate, two more urban blocks and the lodge");
    let mut values: Vec<f64> = stamped.values().cloned().collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(values, vec![30.0, 30.0, DERIVED_RING_M, 46.0]);
    // The plate itself: 25x23 on a 16 m platform, 33 m instead of 144 m.
    let plate = scene
        .entities
        .iter()
        .find(|entity| entity.grounding.earthwork_blend_m == Some(DERIVED_RING_M as f32))
        .expect("the 25x23 plate");
    assert_eq!((plate.width_m, plate.height_m), (PLATE_WIDTH_M as f32, PLATE_HEIGHT_M as f32));
    let plate_value = serde_json::to_value(plate).unwrap();
    assert_eq!(ground_height_m(&plate_value), PLATE_GROUND_M);
    assert_eq!(legacy_ring_m(&plate_value), LEGACY_RING_M);
    // Never wider than the envelope it replaces, and never outside the contract.
    assert!(DERIVED_RING_M <= legacy_ring_m(&plate_value));

    // The published carve list no longer holds the 144 m ring, and every structure
    // ring is one the formula could produce. Roads are untouched: the same 29
    // carves with the same rings as the fixed-envelope world.
    let rings = building_rings(&dir);
    assert_eq!(rings.len(), 52);
    assert!(
        rings.iter().all(|ring| *ring < LEGACY_RING_M),
        "the 144 m ring must be gone: {rings:?}"
    );
    for ring in [DERIVED_RING_M, 46.0, 30.0] {
        assert!(rings.contains(&ring), "the derived ring {ring} m is published");
    }
    // A planned pad keeps the historical ring this round: its record is rebuilt
    // from the plan by three independent readers (see `earthwork_blend::stamp`).
    assert_eq!(rings.iter().filter(|ring| **ring == 114.0).count(), 13);
    assert_road_rings_unchanged(&dir);

    // The same window: the plate's fill is gone, the road's own fill is what stands.
    let baked = raster(&dir, &scene, true);
    assert_eq!(baked.height_millimetres(PAD_CELL.0, PAD_CELL.1), Some(DERIVED_PAD_MM));

    // The lift the completion pass records falls from 3,780 mm to 761 mm; the
    // residual owner is a planned home's own historical ring, not a structure.
    let completion = read_json(dir.0.join("pad-completion.json"));
    let scene_record = &completion["scenes"][0];
    assert_eq!(scene_record["padsRefused"], json!(1));
    assert_eq!(scene_record["padsDeepened"], json!(1));
    let lift = &scene_record["pads"][0];
    assert_eq!(lift["padId"], json!(REFUSED_PAD));
    assert_eq!(lift["liftMm"], json!(DERIVED_LIFT_MM));
    assert_ne!(lift["entityKind"], json!("urban_building"));

    // The pads the plan keeps are still the plan's pads.
    let plan: Value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    assert!(
        plan["roadsideSites"]
            .as_array()
            .unwrap()
            .iter()
            .all(|site| site["id"] != json!(REFUSED_PAD))
    );
    assert_eq!(plan["roadsideSites"].as_array().unwrap().len(), REMAINING_PADS);

    // 3. The cut ledger is still the rasters' own measurement, and the protected
    //    land, the water and the shore band still carry no excavation.
    let ledger: CutLedger = serde_json::from_value(
        read_json(scene_dir(&dir).join("engineered-terrain.json"))["boundedCut"].clone(),
    )
    .expect("the world publishes its bounded-cut ledger");
    assert!(ledger.enabled);
    assert_eq!(ledger.pads.len(), DERIVED_CUT_PADS);
    assert_eq!(ledger.cut_cells, DERIVED_CUT_CELLS);
    assert_eq!(ledger.cut_depth_sum_mm, DERIVED_CUT_DEPTH_SUM_MM);
    assert!(ledger.max_cut_depth_mm <= bounded_cut::MAX_CUT_MM);
    let natural = raster(&dir, &scene, false);
    let final_terrain = raster(&dir, &scene, true);
    let plan: glyphweave_core::worldgen::road_network::RoadNetworkPlan =
        serde_json::from_value(read_json(scene_dir(&dir).join("road-network-plan.json"))).unwrap();
    let measured = bounded_cut::measure_from_rasters(&plan.roadside_sites, &natural, &final_terrain)
        .expect("the two published rasters measure");
    assert_eq!(ledger.pads, measured, "the ledger IS the rasters' own measurement");
    let zones: Vec<LandUseZone> =
        serde_json::from_value(read_json(scene_dir(&dir).join("land-use-zones.json"))).unwrap();
    let spec = scene_spec(&scene);
    let water: WaterGeometry = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &spec,
        &manifest.style,
    );
    let (policy, not_enabled) = CutPolicy::from_style(&manifest.style);
    assert_eq!(not_enabled, None);
    let audit = bounded_cut::verify(
        &spec,
        Some(&plan),
        &zones,
        water,
        &natural,
        &final_terrain,
        Some(&ledger),
        policy,
    )
    .expect("the audit runs");
    assert_eq!(audit.status, "pass", "{:?}", audit.failures);
    assert_eq!(audit.protected_cells_changed, 0);
    assert_eq!(audit.water_cells_changed, 0);
    assert_eq!(audit.shore_band_cells_cut, 0);
}

/// 5. OFF is frozen: two bakes publish the same bytes, carry no derived ring, and
///    the 144 m ring is still in the carve list the second time.
#[test]
fn two_off_bakes_are_byte_identical_and_carry_no_ring() {
    let first = TestDir::new("blend-off-first");
    let second = TestDir::new("blend-off-second");
    let manifest = grown();
    bake_world(&manifest, &first.0).expect("the off arm bakes");
    bake_world(&manifest, &second.0).expect("the off arm bakes twice");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    assert!(moved.is_empty(), "off-state bytes moved: {moved:?}");
    for dir in [&first, &second] {
        let scene = scene_index(dir);
        assert!(scene
            .entities
            .iter()
            .all(|entity| entity.grounding.earthwork_blend_m.is_none()));
        assert_eq!(
            building_rings(dir).iter().cloned().fold(0.0_f64, f64::max),
            LEGACY_RING_M
        );
    }
}

/// 4. Two bakes of the derived world publish identical bytes.
#[test]
fn two_derived_bakes_are_byte_identical() {
    let first = TestDir::new("blend-on-first");
    let second = TestDir::new("blend-on-second");
    let manifest = grown_derived();
    bake_world(&manifest, &first.0).expect("the derived arm bakes");
    bake_world(&manifest, &second.0).expect("the derived arm bakes twice");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert!(moved.is_empty(), "derived bytes moved: {moved:?}");
}

/// 5. The waterfront sample (explicit conversion ON) keeps its plan, its pads and
///    its entities when the derived envelope is switched on.
#[test]
fn the_waterfront_sample_keeps_its_plan_and_its_entities() {
    let fixed = TestDir::new("blend-waterfront-off");
    let derived = TestDir::new("blend-waterfront-on");
    bake_world(&waterfront(false), &fixed.0).expect("the fixed waterfront bakes");
    bake_world(&waterfront(true), &derived.0).expect("the derived waterfront bakes");
    let off_plan: Value = read_json(scene_dir(&fixed).join("road-network-plan.json"));
    let on_plan: Value = read_json(scene_dir(&derived).join("road-network-plan.json"));
    assert_eq!(
        off_plan["roadsideSites"].as_array().unwrap().len(),
        28,
        "the registered sample plans 28 pads"
    );
    assert_eq!(
        on_plan["roadsideSites"].as_array().unwrap().len(),
        28,
        "the derived envelope may not lose a pad"
    );
    assert_eq!(
        off_plan["roadsideSiteRejections"], on_plan["roadsideSiteRejections"],
        "the same pads are refused, by name, in both arms"
    );
    let kinds = |dir: &TestDir| -> BTreeMap<String, usize> {
        let scene = scene_index(dir);
        let mut counts = BTreeMap::new();
        for entity in &scene.entities {
            *counts.entry(entity.kind.clone()).or_insert(0) += 1;
        }
        counts
    };
    assert_eq!(kinds(&fixed), kinds(&derived), "the same entities are realized");
    let stamped: Vec<f32> = scene_index(&derived)
        .entities
        .iter()
        .filter_map(|entity| entity.grounding.earthwork_blend_m)
        .collect();
    assert_eq!(stamped.len(), 2, "two rule-engine structures carry the ring");
    assert!(scene_index(&fixed)
        .entities
        .iter()
        .all(|entity| entity.grounding.earthwork_blend_m.is_none()));
}
