//! Round 34: the derived ring now lives in the PLAN RECORD, so a planned pad
//! carries it too - `style.terrainPolicy.earthworkBlend = "derived-from-footprint"`.
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
//! Round 34 closes the half of the envelope the first cut left behind. A PLANNED
//! pad's entity is rebuilt from the road plan alone (`settlement::development::plan`
//! -> `RoadsideSite::entity`), and three readers compare that rebuild with the
//! published entity byte for byte, so a ring that only the entity carried was a
//! plan/entity split rather than an earthwork decision - and the thirteen planned
//! pads of the grown hillside town kept the historical 114 m ring while every
//! rule-engine structure was already derived. The ring is now written into the pad's
//! own plan record (`earthworkBlendM`), the entity reads it out of that record, and
//! the rebuild reads it out of that record too: one ring, one place, both sides.
//!
//! Round 39 changes what the two arms PUBLISH over the pad's own flat top. The ring
//! is still the thing the carve list carries (144 m -> 48 m at the widest), but the
//! bounded-cut platform rule now publishes the pad's declared 5,325 mm on all 81 cells
//! in BOTH arms, and the pad keeps its entity in both arms. The numbers below were
//! re-measured on the same fixtures: the pad's window reads 5,325 mm where it used to
//! read 9,105 mm (off) and 6,016 mm (derived), the plan keeps 17 pads instead of 16,
//! and the completion pass publishes no record at all because it has nothing to
//! settle. The ring's own effect is still visible where it always was measurable - in
//! the carve list and in the structures' envelopes - which is what the assertions
//! below hold.
//!
//! What this file asserts, world by world, all of it read back from the bytes the
//! bake published:
//!
//! 1. OFF: no entity AND no plan pad carries a derived ring, the published plan has
//!    no `earthworkBlendM` key at all, and two bakes are byte-identical. Every
//!    structure's carve publishes the fixed envelope (`3 x (ground + 32)`, exactly
//!    144 m on the 16 m plate), the structure carves include that 144 m ring, and the
//!    pad's window publishes its own declared 5,325 mm.
//! 2. ON: the 144 m ring is gone from the published carve list, the rule-engine
//!    structures carry the derived ring (33 m for the 25x23 plate, 48 m for the 40x30
//!    school), every one of the 17 planned pads carries the ring its own width/height
//!    derives (16/20/32/48 m instead of 114 m), and the same window publishes the
//!    same declared 5,325 mm.
//! 3. The plan's pads and the world's entities are the SAME BYTES: the development
//!    plan rebuilt from the published road plan equals the published
//!    `development-plan.json`, and every pad's published entity equals its rebuilt
//!    entity, `earthworkBlendM` included.
//! 4. The cut ledger still equals the rasters' own measurement, and the protected
//!    land, the water and the shore passage band are still untouched.
//! 5. Two bakes of the same derived manifest publish identical bytes.
//! 6. The waterfront sample keeps its plan, its pads and its entities.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::settlement::development;
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
/// The pad the entity earthwork used to bury, by name.
const THE_PAD: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-network.scene-0.23-30.36.negative";
/// The cell of that pad's window the round reads its published top on.
const PAD_CELL: (i32, i32) = (140, 259);
/// The plate the round is about: a 25x23 `urban_building` on a 16 m platform.
const PLATE_GROUND_M: f64 = 16.0;
const PLATE_WIDTH_M: f64 = 25.0;
const PLATE_HEIGHT_M: f64 = 12.0;
/// The fixed envelope at that plate: `3 x (16 + 32)`.
const LEGACY_RING_M: f64 = 144.0;
/// The derived envelope at that plate: `max(25, 12) x 1 + 8`.
const DERIVED_RING_M: f64 = 33.0;
/// The historical ring of a PLANNED pad on a 6 m plate: `3 x (6 + 32)`.
const LEGACY_PAD_RING_M: f64 = 114.0;
/// What the pad's window publishes in BOTH arms now: its own declared top. Before
/// Round 39 the same cell published 9,105 mm (off) and 6,016 mm (derived), because the
/// pad's entity had been deleted and its platform could not be published at all.
const PAD_MM: i32 = 5_325;
/// The excavation the derived scene holds, summed once from its two rasters.
const DERIVED_CUT_PADS: usize = 4;
const DERIVED_CUT_CELLS: u64 = 1_231;
const DERIVED_CUT_DEPTH_SUM_MM: i64 = 737_286;
/// The excavation the fixed-envelope scene holds.
const OFF_CUT_PADS: usize = 4;
const OFF_CUT_CELLS: u64 = 911;
const OFF_CUT_DEPTH_SUM_MM: i64 = 530_566;
/// The pads the grown plan keeps: every one of them.
const PLANNED_PADS: usize = 17;
/// The published bytes of the two fixtures THIS ROUND MUST NOT MOVE, as one blake3
/// digest each over every published file except the path-bearing generation report.
/// Both were taken from the HEAD binary's own output (`glyphweave-r36.exe`), and the
/// round's binary reproduces them file for file.
///
/// Round 40 re-measured both: the hillside digest moves because the town no longer
/// publishes the three trees its own `scale-audit` refused (generation now reads the
/// published 4 m shore clearance), and the waterfront digest moves because its river
/// is now CHECKED by `river.object.toml` instead of being disclosed as unruled - a
/// report-only change, the 432 published entities are byte-identical.
///
/// Round 41 moves the HILLSIDE digest only, and for a reason that is the round's
/// point: the committed hillside sample opts into `bounded-cut`, so its two graded
/// auto roads now publish their own transport band (3.389 m / 5.788 m of fill instead
/// of 6.705 m / 7.150 m), the road audit passes, and the cut ledger books the twelve
/// graded roads by id. The pad ledger, the 22 pads, the 15 homes and the 6 shops are
/// unchanged. The WATERFRONT fixture never opted into the policy and is byte-frozen:
/// its constant does not move.
const FROZEN_HILLSIDE_DIGEST: &str =
    "3db90edbd5a8fc2fc8aa8937dfc2318a1c5f246022783b5a70acbf3ffe797c2b";
const FROZEN_WATERFRONT_DIGEST: &str =
    "5a43f6a08567a764c42b8cb82767a2de722ce276475d3a284c92133a428793b2";
/// The ring every planned pad of the grown hillside town derives, by kind:
/// `(width_m, depth_m, height_m, ring_m, count)`. A pad's ring reads its OWN
/// published width and height - never its depth, never its elevation.
const GROWN_PAD_RINGS: [(u32, u32, u32, f64, usize); 4] = [
    (12, 12, 6, 20.0, 3),
    (8, 8, 6, 16.0, 3),
    (24, 24, 8, 32.0, 10),
    (40, 30, 10, 48.0, 1),
];

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
    assert_eq!(rings.len(), 53, "one carve per structure and per apron");
    assert_eq!(
        rings.iter().cloned().fold(0.0_f64, f64::max),
        LEGACY_RING_M
    );
    assert_eq!(rings.iter().filter(|ring| **ring == LEGACY_RING_M).count(), 1);
    assert_road_rings_unchanged(&dir);

    // Round 39: the ring reaches across the pad's window, but the window publishes
    // the pad's OWN declared top, and the completion pass has nothing to settle - no
    // record file, and the pad is still a pad of the plan.
    assert!(
        !dir.0.join("pad-completion.json").exists(),
        "a world whose platforms hold publishes no completion record"
    );
    let plan: Value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let sites = plan["roadsideSites"].as_array().unwrap();
    assert_eq!(sites.len(), PLANNED_PADS);
    assert!(sites.iter().any(|site| site["id"] == json!(THE_PAD)));
    let baked = raster(&dir, &scene, true);
    assert_eq!(
        baked.height_millimetres(PAD_CELL.0, PAD_CELL.1),
        Some(PAD_MM),
        "the pad's own platform is the published surface, whatever the ring around it"
    );
}

/// 2. ON: every rule-engine structure AND every planned pad carries the derived
///    ring, the 144 m and 114 m rings are gone from the carve list, and the recorded
///    lift falls to 708 mm.
#[test]
fn the_switch_derives_the_structure_rings_from_their_footprints() {
    let dir = TestDir::new("blend-on");
    let manifest = grown_derived();
    bake_world(&manifest, &dir.0).expect("the derived-envelope world bakes");
    let scene = scene_index(&dir);

    // The derived ring is on every structure and every planned pad, and it IS the
    // formula, read off each entity's own published width and height.
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
    // 17 planned pads + the plate and two more urban blocks. The 38x20 lodge that
    // used to stand 3.5 m inside the shop pad is refused by name, so the pad it
    // displaced carries the 16 m ring instead of the lodge's 46 m.
    assert_eq!(stamped.len(), 20, "every structure of this scene, planned pads included");
    let mut values: Vec<f64> = stamped.values().cloned().collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut want = vec![30.0, 30.0, DERIVED_RING_M];
    for (_, _, _, ring, count) in GROWN_PAD_RINGS {
        for _ in 0..count {
            want.push(ring);
        }
    }
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(values, want);
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

    // The published carve list no longer holds the 144 m ring NOR the planned pads'
    // 114 m ring, and every ring is one the formula could produce. Roads are
    // untouched: the same 29 carves with the same rings as the fixed-envelope world.
    let rings = building_rings(&dir);
    assert_eq!(rings.len(), 53, "one carve per structure and per apron");
    assert!(
        rings.iter().all(|ring| *ring < LEGACY_PAD_RING_M),
        "the 144 m and 114 m rings must be gone: {rings:?}"
    );
    for ring in [DERIVED_RING_M, 30.0, 48.0, 32.0, 20.0, 16.0] {
        assert!(rings.contains(&ring), "the derived ring {ring} m is published");
    }
    assert_eq!(
        rings.iter().filter(|ring| **ring == LEGACY_PAD_RING_M).count(),
        0,
        "no planned pad may keep the historical 114 m ring"
    );
    assert_eq!(rings.iter().cloned().fold(0.0_f64, f64::max), 48.0);
    assert_road_rings_unchanged(&dir);

    // The same window: the pad's own declared top, in this arm too. The ring no
    // longer decides what stands over a pad's declared top - the pad's platform does.
    let baked = raster(&dir, &scene, true);
    assert_eq!(
        baked.height_millimetres(PAD_CELL.0, PAD_CELL.1),
        Some(PAD_MM)
    );

    // The completion pass has nothing to settle, so it publishes no record at all and
    // every planned pad - the shop the entity stage used to lose included - is still
    // a pad of the plan.
    assert!(
        !dir.0.join("pad-completion.json").exists(),
        "a world whose platforms hold publishes no completion record"
    );
    let plan: Value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    assert!(
        plan["roadsideSites"]
            .as_array()
            .unwrap()
            .iter()
            .any(|site| site["id"] == json!(THE_PAD))
    );
    assert_eq!(plan["roadsideSites"].as_array().unwrap().len(), PLANNED_PADS);
    assert!(
        plan["roadsideSiteRejections"]
            .as_array()
            .is_none_or(|rejections| rejections
                .iter()
                .all(|rejection| rejection["category"]
                    != json!("pad_top_conflicts_with_entity_platform"))),
        "no pad top is refused in this arm"
    );

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
    assert_eq!(
        stamped.len(),
        30,
        "28 planned pads and two rule-engine structures carry the ring"
    );
    let mut histogram: BTreeMap<String, usize> = BTreeMap::new();
    for ring in &stamped {
        *histogram.entry(format!("{ring}")).or_insert(0) += 1;
    }
    // 22 homes 24x24 (32 m), four shops 12x10 (20 m), the church 20x28 (28 m), the
    // school 40x30 (48 m), plus the two rule-engine structures at 35 m and 37 m.
    assert_eq!(
        histogram,
        BTreeMap::from([
            ("20".to_string(), 4),
            ("28".to_string(), 1),
            ("32".to_string(), 22),
            ("35".to_string(), 1),
            ("37".to_string(), 1),
            ("48".to_string(), 1),
        ])
    );
    // The same 82 structure carves, and no ring left above 48 m.
    let off_rings = building_rings(&fixed);
    let on_rings = building_rings(&derived);
    assert_eq!((off_rings.len(), on_rings.len()), (82, 82));
    assert_eq!(off_rings.iter().cloned().fold(0.0_f64, f64::max), 114.0);
    assert_eq!(on_rings.iter().cloned().fold(0.0_f64, f64::max), 48.0);
    assert!(scene_index(&fixed)
        .entities
        .iter()
        .all(|entity| entity.grounding.earthwork_blend_m.is_none()));
    // And the record carries it, not just the entity: every one of the 28 pads.
    for site in on_plan["roadsideSites"].as_array().unwrap() {
        assert!(
            site["earthworkBlendM"].as_f64().is_some(),
            "{} must publish the ring it derives",
            site["id"]
        );
    }
    assert!(off_plan["roadsideSites"]
        .as_array()
        .unwrap()
        .iter()
        .all(|site| site.get("earthworkBlendM").is_none()));
}

/// One digest over every published file of a world, so a fixture's bytes can be
/// frozen as ONE constant. `generation-report.json` is excluded because it records the
/// output directory, which the test's temporary folder changes every run; nothing else
/// in a world embeds a path.
fn world_digest(files: &BTreeMap<String, String>) -> String {
    let mut hasher = blake3::Hasher::new();
    for (path, digest) in files.iter().filter(|(path, _)| !path.contains("generation-report")) {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(digest.as_bytes());
        hasher.update(b"\0");
    }
    hasher.finalize().to_hex().to_string()
}

/// 6. OFF is FROZEN against the previous binary: the committed hillside sample and the
///    registered waterfront sample publish exactly the bytes Round 36 published, file
///    for file (the digests were taken from the HEAD binary's own output). This is the
///    cross-binary freeze the round's contract asks for, in a form a test can hold: if
///    a later change moves a byte of either fixture, one digest changes.
#[test]
fn the_committed_fixtures_publish_the_frozen_bytes() {
    let hillside_dir = TestDir::new("frozen-hillside");
    let mut hillside: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    hillside.style["rulesDir"] = json!(rules());
    bake_world(&hillside, &hillside_dir.0).expect("the committed hillside bakes");
    let hillside_digest = world_digest(&published_files(&hillside_dir.0));

    let waterfront_dir = TestDir::new("frozen-waterfront");
    bake_world(&waterfront(false), &waterfront_dir.0).expect("the waterfront bakes");
    let waterfront_digest = world_digest(&published_files(&waterfront_dir.0));

    assert_eq!(
        hillside_digest, FROZEN_HILLSIDE_DIGEST,
        "the committed hillside town moved: {}",
        hillside_digest
    );
    assert_eq!(
        waterfront_digest, FROZEN_WATERFRONT_DIGEST,
        "the registered waterfront sample moved: {}",
        waterfront_digest
    );
}

/// 1. OFF is byte-inert in the PLAN too, not only on the entity: no pad record gains
///    the key, the published plan never contains the string, and two bakes of the
///    same manifest publish the same bytes - fixture by fixture.
#[test]
fn the_off_plan_publishes_no_ring_key_and_is_byte_frozen() {
    for (manifest, label) in [(grown(), "grown"), (waterfront(false), "waterfront")] {
        let a = TestDir::new(&format!("blend-off-{label}-a"));
        let b = TestDir::new(&format!("blend-off-{label}-b"));
        bake_world(&manifest, &a.0).expect("the off arm bakes");
        bake_world(&manifest, &b.0).expect("the off arm bakes twice");
        let plan_bytes = fs::read(scene_dir(&a).join("road-network-plan.json")).unwrap();
        let scene_bytes = fs::read(scene_dir(&a).join("scene.json")).unwrap();
        for bytes in [&plan_bytes, &scene_bytes] {
            assert!(
                !String::from_utf8_lossy(bytes).contains("earthworkBlend"),
                "{label}: the off arm must publish no ring key at all"
            );
        }
        let plan: Value = serde_json::from_slice(&plan_bytes).unwrap();
        let sites = plan["roadsideSites"].as_array().unwrap();
        assert!(!sites.is_empty(), "{label}: the fixture plans pads");
        for site in sites {
            assert!(
                site.get("earthworkBlendM").is_none(),
                "{label}: {} must not carry a ring",
                site["id"]
            );
        }
        let scene = scene_index(&a);
        assert!(
            scene
                .entities
                .iter()
                .all(|entity| entity.grounding.earthwork_blend_m.is_none()),
            "{label}: no entity of the off arm carries a ring"
        );
        // Byte-frozen: the two bakes of one manifest agree, file for file.
        assert_eq!(
            published_files(&a.0),
            published_files(&b.0),
            "{label}: the off arm is deterministic"
        );
    }
}

/// 2. ON: every planned pad's RECORD carries the ring its own published width and
///    height derive - 16/20/32/48 m instead of the historical 114 m - and the record
///    is the only place the number comes from.
#[test]
fn every_planned_pad_carries_the_ring_its_own_width_and_height_derive() {
    let dir = TestDir::new("blend-pad-rings");
    bake_world(&grown_derived(), &dir.0).expect("the derived-envelope world bakes");
    let plan: Value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let sites = plan["roadsideSites"].as_array().unwrap();
    assert_eq!(sites.len(), PLANNED_PADS);

    let mut histogram: BTreeMap<(u32, u32, u32, u32), usize> = BTreeMap::new();
    for site in sites {
        let width = site["widthM"].as_u64().unwrap() as u32;
        let depth = site["depthM"].as_u64().unwrap() as u32;
        let height = site["program"]["heightM"].as_u64().unwrap_or(8) as u32;
        let ring = site["earthworkBlendM"]
            .as_f64()
            .unwrap_or_else(|| panic!("{} must publish its ring", site["id"]));
        // The formula, on the two numbers the record itself publishes:
        // `clamp(max(width_m, height_m) * 1 + 8, 8, 144)`.
        let want = f64::from(width.max(height)) + 8.0;
        assert_eq!(ring, want, "{} ({width}x{depth} m, h {height})", site["id"]);
        assert_eq!(site["earthworkBlendM"], json!(want as f32));
        *histogram.entry((width, depth, height, ring as u32)).or_insert(0) += 1;
    }
    // (width, depth, height, ring) -> how many pads of the grown plan derive it.
    let want: BTreeMap<(u32, u32, u32, u32), usize> = GROWN_PAD_RINGS
        .iter()
        .map(|(w, d, h, ring, count)| ((*w, *d, *h, *ring as u32), *count))
        .collect();
    assert_eq!(histogram, want);
    // Not one pad keeps the historical envelope of a 6 m plate.
    assert!(sites
        .iter()
        .all(|site| site["earthworkBlendM"].as_f64() != Some(LEGACY_PAD_RING_M)));
    // And the entity that pad becomes publishes the same number, byte for byte.
    let scene = scene_index(&dir);
    for site in sites {
        let id = site["id"].as_str().unwrap();
        let entity = scene
            .entities
            .iter()
            .find(|entity| entity.entity_id == id)
            .unwrap_or_else(|| panic!("{id} is realized"));
        assert_eq!(
            entity.grounding.earthwork_blend_m,
            Some(site["earthworkBlendM"].as_f64().unwrap() as f32),
            "{id}: record and entity must carry one ring"
        );
    }
}

/// 3. The plan's pads and the world's entities are the SAME BYTES. The development
///    plan rebuilt from the PUBLISHED road plan equals the published
///    `development-plan.json`, and every pad's published entity equals the entity
///    that rebuild derives (`earthworkBlendM` included) - which is exactly what
///    `settlement::development::verify` requires of a world.
#[test]
fn the_planned_pad_entity_is_the_rebuilt_plan_entity_byte_for_byte() {
    let dir = TestDir::new("blend-plan-rebuild");
    bake_world(&grown_derived(), &dir.0).expect("the derived-envelope world bakes");
    let scene = scene_index(&dir);
    let spec = scene_spec(&scene);
    let roads: glyphweave_core::worldgen::road_network::RoadNetworkPlan =
        serde_json::from_value(read_json(scene_dir(&dir).join("road-network-plan.json"))).unwrap();
    let zones: Vec<LandUseZone> =
        serde_json::from_value(read_json(scene_dir(&dir).join("land-use-zones.json"))).unwrap();
    let rebuilt = development::plan(&spec, &zones, &roads).expect("the plan rebuilds");
    // 1. The rebuild IS the published record, byte for byte. (Bytes, not `Value`s:
    //    a `Value` round-trip widens every f32 `bottomOffsetM` to f64, which is a
    //    property of the comparison and not of the plan.)
    assert_eq!(
        serde_json::to_vec_pretty(&rebuilt).unwrap(),
        fs::read(scene_dir(&dir).join("development-plan.json")).unwrap(),
        "the plan rebuilt from the published road plan must equal the published plan"
    );
    // 2. Every planned pad's entity is the rebuild's own entity, ring included.
    let mut carrying = 0;
    for building in &rebuilt.buildings {
        let mut source = building.entity.clone();
        source.bounds = None;
        let found: Vec<_> = scene
            .entities
            .iter()
            .filter(|entity| entity.entity_id == source.entity_id)
            .collect();
        assert_eq!(found.len(), 1, "{} is realized exactly once", source.entity_id);
        let mut actual = found[0].clone();
        actual.bounds = None;
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&source).unwrap(),
            "{}: the published entity must be the plan's entity, byte for byte",
            source.entity_id
        );
        if let Some(ring) = source.grounding.earthwork_blend_m {
            carrying += 1;
            let value = serde_json::to_value(&actual).unwrap();
            assert_eq!(
                value["grounding"]["earthworkBlendM"],
                json!(ring),
                "{}: the rebuilt entity carries the record's ring",
                source.entity_id
            );
        }
    }
    assert_eq!(carrying, PLANNED_PADS, "every planned pad carries a ring");
}

/// 4. The same world, two arms: the ring no longer decides what stands over a pad's
///    declared top, and the cut ledger of each arm is the rasters' own measurement.
///
/// | | arm OFF | arm ON |
/// |---|---|---|
/// | published pads whose top the raster fails to hold | 0 of 17 | 0 of 17 |
/// | the pad's window | 5,325 mm (its own declared top) | 5,325 mm |
/// | widest structure ring in the carve list | 144 m | 48 m |
/// | completion records published | none | none |
/// | cut ledger (pads / cells / mm) | 4 / 911 / 530,566 | 4 / 1,231 / 737,286 |
#[test]
fn the_derived_ring_narrows_the_envelope_and_the_pad_top_holds_in_both_arms() {
    let off = TestDir::new("blend-bury-off");
    let on = TestDir::new("blend-bury-on");
    bake_world(&grown(), &off.0).expect("the fixed-envelope world bakes");
    bake_world(&grown_derived(), &on.0).expect("the derived-envelope world bakes");

    let arms = |dir: &TestDir| {
        let scene = scene_index(dir);
        let baked = raster(dir, &scene, true);
        let plan: glyphweave_core::worldgen::road_network::RoadNetworkPlan =
            serde_json::from_value(read_json(scene_dir(dir).join("road-network-plan.json")))
                .unwrap();
        // Every pad the plan keeps publishes its own declared top, cell for cell.
        let published_buried = plan
            .roadside_sites
            .iter()
            .filter(|site| {
                bounded_cut::pad_top_holds(site, |x, z| baked.height_millimetres(x, z))
                    .is_some_and(|verdict| !verdict.holds())
            })
            .count();
        let ledger: CutLedger = serde_json::from_value(
            read_json(scene_dir(dir).join("engineered-terrain.json"))["boundedCut"].clone(),
        )
        .expect("the arm publishes its bounded-cut ledger");
        (
            plan.roadside_sites.len(),
            published_buried,
            baked.height_millimetres(PAD_CELL.0, PAD_CELL.1),
            building_rings(dir).iter().cloned().fold(0.0_f64, f64::max),
            dir.0.join("pad-completion.json").exists(),
            (
                ledger.pads.len(),
                ledger.cut_cells,
                ledger.cut_depth_sum_mm,
            ),
        )
    };
    let before = arms(&off);
    let after = arms(&on);
    assert_eq!(before.0, PLANNED_PADS);
    assert_eq!(after.0, PLANNED_PADS, "the derived arm keeps every pad");
    assert_eq!(before.1, 0, "no kept pad's top may fail to hold");
    assert_eq!(after.1, 0, "no kept pad's top may fail to hold");
    assert_eq!(before.2, Some(PAD_MM));
    assert_eq!(after.2, Some(PAD_MM));
    assert_eq!(before.3, LEGACY_RING_M, "the fixed envelope is still the widest");
    assert_eq!(after.3, 48.0, "the derived envelope is the widest left");
    assert!(!before.4, "the fixed-envelope arm settles no pad top");
    assert!(!after.4, "the derived-envelope arm settles no pad top");
    assert_eq!(before.5, (OFF_CUT_PADS, OFF_CUT_CELLS, OFF_CUT_DEPTH_SUM_MM));
    assert_eq!(
        after.5,
        (DERIVED_CUT_PADS, DERIVED_CUT_CELLS, DERIVED_CUT_DEPTH_SUM_MM)
    );
}
