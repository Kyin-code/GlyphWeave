//! Round 32: the plan's pad platform and the ENTITY earthwork are now closed.
//!
//! `examples/hillside-town-512.manifest.json` with `landUseProfile.urbanGrowth =
//! "axis-and-node"` is a 512 m river-gorge town grown along its axes. Before this
//! round it baked NOTHING: planning accepted a 8x8 `storefront` pad at (144, 260)
//! on a declared 5.325 m top from the plan's OWN earthwork, the entity stage then
//! placed a 16 m `urban_building` whose 144 m support ring stands over that pad,
//! and `bounded_cut_verify` step 5 refused the whole scene with
//! `bounded_cut_pad_top_not_flat: pad ... holds 81 footprint cells away from its
//! declared 5325 mm top`. One pad, 81 cells, one wrong platform - and no way to say
//! which.
//!
//! What this test asserts, in the order the bake produces it:
//!
//! 1. the bake SUCCEEDS, and the whole cut audit verifies from the published bytes:
//!    3 benches, 777 cells, 522,318 mm of excavation, ZERO cut cells on protected
//!    land, in the water and on the shoreline passage band;
//! 2. the pad is refused BY NAME under
//!    `pad_top_conflicts_with_entity_platform`, quoting the entity that buried it,
//!    the 81 cells and the 3.780 m lift, and it is withdrawn from the PLAN, the
//!    frontage outcome, the JOINT receipt, the CONVERSIONS and the ENTITY list -
//!    five views, one verdict;
//! 3. the world still builds its city: 16 pads remain planned and every one of them
//!    is realized, the school is placed, and the housing minimum is unchanged;
//! 4. two bakes of the same manifest publish byte-identical bytes;
//! 5. with the cut policy OFF the completion pass publishes NOTHING - no rejection,
//!    no artifact, no moved byte - and two off-state bakes agree file for file.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::worldgen::bounded_cut::{self, CutLedger, CutPolicy};
use glyphweave_core::worldgen::road_network::{RoadNetworkPlan, PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM};
use glyphweave_core::worldgen::{
    LandUseZone, SceneIndex, SceneSpec, WaterGeometry, WorldManifest, bake_world, water_geometry,
    water_kind,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
/// The one cap this world authors, in whole millimetres.
const MAX_CUT_MM: i32 = 2000;
/// The pad the entity earthwork buries: an 8x8 storefront on a 5.325 m declared top.
const REFUSED_PAD: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-network.scene-0.23-30.36.negative";
/// Cells of that pad's declared flat top (`flat_top_bounds` of a 8x8 pad is 9x9).
const REFUSED_CELLS: u32 = 81;
/// The lift the entity platform puts over the declared top, whole millimetres.
const REFUSED_LIFT_MM: i32 = 3780;
/// The pads the grown plan keeps.
const REMAINING_PADS: usize = 16;
/// The excavation the scene holds, summed once from the two published rasters.
const SCENE_CUT_CELLS: u64 = 777;
const SCENE_CUT_DEPTH_SUM_MM: i64 = 522_318;
const SCENE_CUT_PADS: usize = 3;

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

/// The published example with the growth switch ON: the world Round 32 measured.
fn grown() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["landUseProfile"]["urbanGrowth"] = json!("axis-and-node");
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
            let entry = entry.unwrap();
            let path = entry.path();
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

#[test]
fn the_grown_hillside_town_refuses_the_buried_pad_by_name_and_still_bakes() {
    let dir = TestDir::new("pad-platform-grown");
    let manifest = grown();
    // 1. The bake publishes. `bounded_cut_ledger_for` refuses a world whose cut
    //    cannot be proven from the published bytes, so this is also a passing cut
    //    audit - the thing the Round 32 world could not reach at all.
    bake_world(&manifest, &dir.0).expect("the grown hillside bakes: no scene-wide cut refusal");

    let plan_value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let plan: RoadNetworkPlan =
        serde_json::from_value(plan_value.clone()).expect("the plan round-trips");

    // 2. The pad is refused BY NAME, with the entity, the cells and the lift.
    assert!(
        plan.roadside_sites.iter().all(|site| site.id != REFUSED_PAD),
        "the refused pad is not planned any more"
    );
    assert_eq!(plan.roadside_sites.len(), REMAINING_PADS);
    let refusal = plan
        .roadside_site_rejections
        .iter()
        .find(|rejection| rejection.edge_id == REFUSED_PAD)
        .expect("the refusal is published in the plan");
    assert_eq!(refusal.category, PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM);
    let published = read_json(dir.0.join("pad-completion.json"));
    assert_eq!(published["maxRounds"], json!(2));
    assert_eq!(published["maxLiftMm"], json!(MAX_CUT_MM));
    let record = &published["scenes"][0]["pads"][0];
    assert_eq!(record["padId"], json!(REFUSED_PAD));
    assert_eq!(record["outcome"], json!("refused"));
    assert_eq!(record["declaredMm"], json!(5_325));
    assert_eq!(record["mismatchedCells"], json!(REFUSED_CELLS));
    assert_eq!(record["footprintCells"], json!(REFUSED_CELLS));
    assert_eq!(record["liftMm"], json!(REFUSED_LIFT_MM));
    assert_eq!(record["maxCutMm"], json!(MAX_CUT_MM));
    assert_eq!(record["naturalMaxMm"], json!(5_417));
    assert_eq!(record["neededCutMm"], json!(92));
    let entity = record["entityId"].as_str().expect("the refusal names the entity");
    assert!(
        entity.starts_with("generated.urban-building."),
        "the entity that buried the pad: {entity}"
    );
    assert_eq!(record["entityKind"], json!("urban_building"));
    let entity_top = record["entityTopMm"].as_i64().unwrap();
    assert_eq!(
        entity_top - 5_325,
        i64::from(REFUSED_LIFT_MM),
        "the named platform IS the lift"
    );
    assert_eq!(published["scenes"][0]["padsRefused"], json!(1));
    assert_eq!(published["scenes"][0]["padsDeepened"], json!(0));
    assert_eq!(published["scenes"][0]["rounds"], json!(2));
    assert!(
        refusal.reason.contains(entity),
        "the published reason names the entity: {}",
        refusal.reason
    );
    assert!(
        refusal.reason.contains("3780 mm"),
        "the published reason quotes the lift: {}",
        refusal.reason
    );

    // 2b. Five views, one verdict: entity, conversions, frontage outcome, joint
    //     receipt, plan.
    let scene = scene_index(&dir);
    assert!(
        scene
            .entities
            .iter()
            .all(|entity| entity.entity_id != REFUSED_PAD),
        "the pad's own entity left the world"
    );
    assert!(
        plan.pad_conversions
            .iter()
            .all(|record| record.parcel_id != REFUSED_PAD),
        "the conversion the pad earned left with it"
    );
    let frontage = plan.frontage.as_ref().expect("the frontage report");
    assert!(
        frontage
            .conversions
            .iter()
            .all(|conversion| conversion.parcel_id != REFUSED_PAD),
        "the along-road frontage report does not book it either"
    );
    for outcome in &frontage.outcomes {
        assert!(
            outcome.site_ids.iter().all(|id| id != REFUSED_PAD),
            "no frontage outcome keeps the refused pad"
        );
        assert_eq!(
            outcome.unfilled,
            outcome.request.count.saturating_sub(outcome.site_ids.len() as u32),
            "the outcome's demand is recomputed, not decremented by hand"
        );
    }
    let joint = plan.joint_layout.as_ref().expect("the joint receipt");
    for ids in [
        &joint.retained_residential_ids,
        &joint.displaced_residential_ids,
        &joint.replacement_residential_ids,
    ] {
        assert!(ids.iter().all(|id| id != REFUSED_PAD));
    }
    assert_eq!(
        joint.unfilled_residential,
        joint
            .residential_minimum
            .saturating_sub(plan.roadside_sites.iter().filter(|site| site.program.is_none()).count()),
        "the joint receipt charges its own minimum against the pads that remain"
    );
    // The reconcile report agrees with both: the pad is named refused and is in
    // neither the reconcile plan nor its output.
    let engineering = read_json(dir.0.join("engineering-placement.json"));
    let entry = &engineering["scenes"][0];
    assert!(entry["refusedEntityIds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == REFUSED_PAD));
    for field in ["inputEntityIds", "outputEntityIds"] {
        assert!(
            entry[field]
                .as_array()
                .unwrap()
                .iter()
                .all(|id| id != REFUSED_PAD),
            "{field} does not carry the refused pad"
        );
    }
    assert_eq!(
        entry["plannedPadIds"].as_array().unwrap().len(),
        REMAINING_PADS
    );

    // 3. The world still builds its city: every remaining pad is realized on its
    //    declared top, and the school is placed.
    let road_audit = read_json(scene_dir(&dir).join("road-network-audit.json"));
    assert_eq!(road_audit["plannedSites"], json!(REMAINING_PADS));
    assert_eq!(
        road_audit["realizedSites"],
        road_audit["plannedSites"],
        "every planned pad reaches the world: {road_audit}"
    );
    assert!(
        road_audit["maxSiteDesignHeightErrorM"].as_f64().unwrap() < 0.0001,
        "every platform is its declared top: {road_audit}"
    );
    let services = read_json(scene_dir(&dir).join("service-program-audit.json"));
    assert_eq!(services["status"], json!("pass"), "{services}");
    let school = plan
        .service_outcomes
        .iter()
        .find(|outcome| outcome.request.id == "bench-terrace-school")
        .expect("the school request");
    assert_eq!(school.planned_site_ids.len(), 1, "the school is already placed");

    // 4. The cut audit, recomputed from the published bytes.
    let ledger: CutLedger = serde_json::from_value(
        read_json(scene_dir(&dir).join("engineered-terrain.json"))["boundedCut"].clone(),
    )
    .expect("the world publishes its bounded-cut ledger");
    assert!(ledger.enabled);
    assert_eq!(ledger.max_cut_mm, Some(MAX_CUT_MM));
    assert_eq!(ledger.pads.len(), SCENE_CUT_PADS);
    assert_eq!(ledger.cut_cells, SCENE_CUT_CELLS);
    assert_eq!(ledger.cut_depth_sum_mm, SCENE_CUT_DEPTH_SUM_MM);
    assert_eq!(ledger.cut_volume_m3, SCENE_CUT_DEPTH_SUM_MM as f64 / 1000.);
    assert!(ledger.max_cut_depth_mm <= MAX_CUT_MM);
    let natural = raster(&dir, &scene, false);
    let final_terrain = raster(&dir, &scene, true);
    let measured = bounded_cut::measure_from_rasters(&plan.roadside_sites, &natural, &final_terrain)
        .expect("the two published rasters measure");
    assert_eq!(
        ledger.pads, measured,
        "the ledger IS the rasters' own measurement"
    );
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
    assert_eq!(audit.pads, SCENE_CUT_PADS);
    assert_eq!(audit.cut_cells, SCENE_CUT_CELLS);
    assert_eq!(audit.cut_depth_sum_mm, SCENE_CUT_DEPTH_SUM_MM);
    assert_eq!(audit.protected_cells_changed, 0);
    assert_eq!(audit.water_cells_changed, 0);
    assert_eq!(audit.shore_band_cells_cut, 0);
}

#[test]
fn two_bakes_of_the_grown_hillside_town_are_byte_identical() {
    let first = TestDir::new("grown-first");
    let second = TestDir::new("grown-second");
    let manifest = grown();
    bake_world(&manifest, &first.0).expect("the first bake publishes");
    bake_world(&manifest, &second.0).expect("the second bake publishes");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "both bakes publish the same file set"
    );
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert!(
        moved.is_empty(),
        "two bakes of the same manifest published different bytes: {moved:?}"
    );
}

/// Requirement 5: with the capability OFF the completion pass is inert. The same
/// grown world without `terrainPolicy`/`maxCutMm` publishes no refusal and no
/// `pad-completion.json` at all, so every off-state byte is the historical one.
#[test]
fn the_completion_pass_publishes_nothing_when_the_capability_is_off() {
    let mut manifest = grown();
    {
        let program = manifest.style["frontageProgram"]
            .as_object_mut()
            .expect("the sample authors a frontage program");
        program.remove("terrainPolicy");
        program.remove("maxCutMm");
    }
    assert_eq!(CutPolicy::from_style(&manifest.style).0, CutPolicy::Disabled);
    let first = TestDir::new("grown-off-first");
    let second = TestDir::new("grown-off-second");
    bake_world(&manifest, &first.0).expect("the off arm bakes");
    bake_world(&manifest, &second.0).expect("the off arm bakes twice");
    let plan: RoadNetworkPlan =
        serde_json::from_value(read_json(scene_dir(&first).join("road-network-plan.json")))
            .expect("the off plan round-trips");
    assert!(
        plan.roadside_sites
            .iter()
            .all(|site| site.bounded_cut_mm.is_none()),
        "the policy is off, so no pad may carry a cut record"
    );
    assert!(
        plan.roadside_site_rejections
            .iter()
            .all(|rejection| rejection.category != PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM),
        "a world that never asked for a bench publishes no bench refusal"
    );
    assert!(
        !first.0.join("pad-completion.json").exists(),
        "a world that never asked for a bench publishes no completion record"
    );
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert!(
        moved.is_empty(),
        "two off-state bakes must publish the same bytes: {moved:?}"
    );
}
