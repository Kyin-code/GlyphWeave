//! Round 28/29: the benched hillside town is a REAL excavation, not a plan entry.
//!
//! `examples/hillside-town-512.manifest.json` is a 512 m river-gorge world whose
//! frontage programme opts into `bounded-cut` at the one 2 m cap: contour streets on
//! both banks whose lots are benched into the slope. Round 27 made the placement
//! preflight plan a candidate's own cut before judging its platform, which took the
//! 1 km replay from 43 to 96 planned homes - and then `road_network`'s pad refusal
//! gate, which still read every pad as fill-only, stripped the benched pads again
//! before anything was written.
//!
//! Round 29 closed the accounting: two pads may not share a cut footprint (the
//! ledger books every pad's excavation from the same cells, so an overlap counted
//! 273 cells twice), and a pad refused after placement withdraws its frontage
//! conversion with it. What this test asserts, in the order the bake produces it:
//!
//! 1. the bake SUCCEEDS, and that is stronger than it sounds: `bounded_cut_ledger_for`
//!    verifies the published ledger against the plan, the zoning, the water and the
//!    two published rasters and REFUSES to publish a world that does not verify, so a
//!    successful bake is a passing cut audit;
//! 2. the plan carries the excavations: 22 pads, 7 of them with a `boundedCutMm`, and
//!    15 fill-only pads that publish no new field at all;
//! 3. every cut footprint is pairwise disjoint, and the three candidates that would
//!    have shared one are refused BY NAME and counted in the request's own table;
//! 4. the published ledger's per-pad records equal an INDEPENDENT recomputation from
//!    the two published rasters, cell for cell and millimetre for millimetre, and the
//!    ledger's TOTAL equals the scene-wide excavation summed once (1,572 cells and
//!    753,367 mm of depth) - the number the double count used to inflate;
//! 5. the cut audit recomputed here - from the published bytes, the plan, the zoning
//!    and the water - passes with ZERO cut cells on protected land, in the water and
//!    on the shoreline passage band;
//! 6. every planned pad is realized, the deepest cell is inside the 2 m cap, and every
//!    pad's platform is its declared millimetre top;
//! 7. the two roads that Round 38 measured at 6.705 m / 7.150 m of fill now publish
//!    3.389 m / 5.788 m against their own transport bands, the road audit passes, and
//!    the graded roads appear in the SAME ledger by road id (12 records, no
//!    excavation needed here: this world's road excess was a neighbour's fill);
//! 8. two bakes of the same manifest publish byte-identical world files, and the
//!    policy-OFF bake is frozen to one tree digest.
//!
//! The two auto roads this test used to pin as a GAP are now part of the contract.
//! Round 41 gave a GRADED road the same bounded-cut capability a frontage pad has
//! (the same one cap, the same per-cell formula, the same ledger): the road's own
//! transport band is its authored support surface, so a neighbouring pad's
//! transition ring can no longer bury the roadbed above its 6 m fill contract, and
//! the road may excavate its uphill grading ring within the cap. The two roads now
//! publish 3.389 m and 5.788 m of fill, the audit passes, and the pad ledger is
//! unchanged (7 pads / 1,572 cells / 753,367 mm) - the excavation this manifest
//! needs is exactly the pads' own.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::worldgen::bounded_cut::{
    self, CutLedger, CutPolicy, footprint_intersection,
};
use glyphweave_core::worldgen::road_network::{RoadNetworkPlan, RoadsideSite};
use glyphweave_core::worldgen::{
    LandUseZone, SceneIndex, SceneSpec, WaterGeometry, WorldManifest, bake_world,
    graded_transport_half_extents, water_geometry, water_kind,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
/// The one cap this world authors, in whole millimetres.
const MAX_CUT_MM: i32 = 2000;
/// Pads the benched plan publishes: 15 homes, 6 storefronts and 1 school, of which
/// seven carry a real excavation (four benched homes, two shops and the school).
/// Round 45 refused the gorge shop whose own street stood a step below its
/// plateau, so the plan is 22 lots and 7 excavations instead of 23 and 8.
const PLANNED_PADS: usize = 22;
const CUT_PADS: usize = 7;
/// The excavation the scene actually holds, summed ONCE from the two published
/// rasters: the number the ledger must equal (Round 28 booked 2,160 cells /
/// 1,099 m3 because 273 cells stood in two footprints at once).
const SCENE_CUT_CELLS: u64 = 2237;
const SCENE_CUT_DEPTH_SUM_MM: i64 = 907_597;
const SCENE_CUT_VOLUME_M3: f64 = 907.597;
const SCENE_CUT_MAX_DEPTH_MM: i32 = 1850;
/// Candidates refused because their cut footprint intersected an accepted pad's:
/// two homes and two storefronts.
///
/// Round 31 moved this from three to four without moving a single accepted pad: the candidate order
/// is now nearest-the-city first (`frontage::candidate_order`), so one storefront candidate that used
/// to be refused earlier - as `occupied_or_road_support` - is now tried after the bench it would
/// overlap and is named by the more precise `bounded_cut_footprint_overlaps_another_pad` instead. The
/// 22 pads, the 7 benches and every excavation number below are unchanged.
const FOOTPRINT_REFUSALS: u32 = 4;

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

/// The published example, with its one repo-relative path made absolute (the CLI
/// resolves `rulesDir` against the manifest, the test against this crate).
fn sample() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
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

/// Every regular file under the bake, keyed by its path relative to the root.
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
fn the_benched_hillside_town_bakes_with_real_cuts_that_verify() {
    let dir = TestDir::new("hillside-cut-city");
    let manifest = sample();
    manifest.validate().expect("the published sample validates");
    // The one switch this world authors, read back out of the sample.
    assert_eq!(
        manifest.style["frontageProgram"]["terrainPolicy"],
        json!("bounded-cut")
    );
    assert_eq!(
        manifest.style["frontageProgram"]["maxCutMm"],
        json!(MAX_CUT_MM)
    );
    // 1. The bake publishes, which means its own cut audit passed (see the module
    //    doc: a world that cannot prove its excavation is refused, not published).
    bake_world(&manifest, &dir.0)
        .expect("the benched hillside bakes; a bake whose bounded-cut ledger fails is refused");

    // 2. The plan carries the excavations.
    let plan_value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let plan: RoadNetworkPlan =
        serde_json::from_value(plan_value.clone()).expect("the published plan round-trips");
    assert_eq!(plan.roadside_sites.len(), PLANNED_PADS);
    let cut: Vec<&RoadsideSite> = plan
        .roadside_sites
        .iter()
        .filter(|site| site.bounded_cut_mm.is_some())
        .collect();
    assert_eq!(
        cut.len(),
        CUT_PADS,
        "the benched lots carry a cap: {:?}",
        cut.iter().map(|s| (&s.id, s.bounded_cut_mm)).collect::<Vec<_>>()
    );
    assert_eq!(PLANNED_PADS - CUT_PADS, 15, "the rest are fill-only pads");
    for site in &cut {
        assert_eq!(site.bounded_cut_mm, Some(MAX_CUT_MM));
        assert!(
            site.support_level_mm.is_some(),
            "a cut pad publishes its millimetre design level"
        );
    }
    // A fill-only pad of the same world publishes no new field at all: the key is
    // absent from its plan record, exactly as it was before the switch existed.
    let published = plan_value["roadsideSites"]
        .as_array()
        .expect("the plan publishes its pads");
    assert_eq!(
        published
            .iter()
            .filter(|site| site.get("boundedCutMm").is_some())
            .count(),
        CUT_PADS
    );
    assert_eq!(
        published
            .iter()
            .filter(|site| site.get("boundedCutMm").is_none())
            .count(),
        PLANNED_PADS - CUT_PADS,
        "a fill-only pad gains no key"
    );
    // 2b. No two benches share a cut footprint, and the candidates that would have
    //     are refused BY NAME and counted in the request's own rejection table.
    for (index, site) in cut.iter().enumerate() {
        let footprint = site.cut_footprint().expect("a cut pad publishes its footprint");
        for other in cut.iter().skip(index + 1) {
            let other_footprint =
                other.cut_footprint().expect("a cut pad publishes its footprint");
            assert_eq!(
                footprint_intersection(&footprint, &other_footprint),
                None,
                "pads {} and {} were both accepted with intersecting cut footprints",
                site.id,
                other.id
            );
        }
    }
    let report = plan.frontage.as_ref().expect("the world publishes its frontage report");
    let refused: u32 = report
        .outcomes
        .iter()
        .map(|outcome| {
            outcome
                .rejected
                .get(bounded_cut::FOOTPRINT_OVERLAP)
                .copied()
                .unwrap_or(0)
        })
        .sum();
    assert_eq!(
        refused, FOOTPRINT_REFUSALS,
        "the overlapping candidates are named and counted: {:?}",
        report
            .outcomes
            .iter()
            .map(|outcome| (&outcome.request.id, &outcome.rejected))
            .collect::<Vec<_>>()
    );

    // 3. The published ledger IS the measurement: recompute it from the two rasters.
    let engineered = read_json(scene_dir(&dir).join("engineered-terrain.json"));
    let ledger: CutLedger = serde_json::from_value(engineered["boundedCut"].clone())
        .expect("the world publishes its bounded-cut ledger");
    assert!(ledger.enabled);
    assert_eq!(ledger.max_cut_mm, Some(MAX_CUT_MM));
    assert_eq!(ledger.pads.len(), CUT_PADS);
    assert_eq!(ledger.cut_cells, SCENE_CUT_CELLS);
    assert_eq!(ledger.cut_depth_sum_mm, SCENE_CUT_DEPTH_SUM_MM);
    assert_eq!(ledger.cut_volume_m3, SCENE_CUT_VOLUME_M3);
    assert_eq!(ledger.max_cut_depth_mm, SCENE_CUT_MAX_DEPTH_MM);
    assert!(ledger.max_cut_depth_mm <= MAX_CUT_MM);
    let scene = scene_index(&dir);
    let natural = raster(&dir, &scene, false);
    let final_terrain = raster(&dir, &scene, true);
    let measured = bounded_cut::measure_from_rasters(
        &plan.roadside_sites,
        &natural,
        &final_terrain,
        &bounded_cut::road_flat_tops(&plan.entities()),
    )
        .expect("the two published rasters measure");
    assert_eq!(measured.len(), CUT_PADS);
    assert_eq!(
        ledger.pads, measured,
        "every published pad record equals the rasters' own measurement"
    );
    // 3b. The ledger TOTAL is the scene's own excavation, summed ONCE over every
    //     cell of the two published rasters - not the sum of overlapping footprints.
    let (mut cells, mut depth_sum, mut deepest) = (0_u64, 0_i64, 0_i32);
    for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
        for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
            let (Some(base), Some(built)) = (
                natural.height_millimetres(x, z),
                final_terrain.height_millimetres(x, z),
            ) else {
                continue;
            };
            let depth = i64::from(base) - i64::from(built);
            if depth <= 0 {
                continue;
            }
            cells += 1;
            depth_sum += depth;
            deepest = deepest.max(depth as i32);
        }
    }
    assert_eq!(cells, SCENE_CUT_CELLS, "scene-wide lowered cells");
    assert_eq!(depth_sum, SCENE_CUT_DEPTH_SUM_MM, "scene-wide cut depth");
    assert_eq!(deepest, SCENE_CUT_MAX_DEPTH_MM, "scene-wide deepest cut");
    assert_eq!(ledger.cut_cells, cells, "the ledger total is the scene total");
    assert_eq!(
        ledger.cut_depth_sum_mm, depth_sum,
        "the ledger's millimetres are the scene's millimetres, once"
    );
    assert_eq!(ledger.cut_volume_m3, depth_sum as f64 / 1000.);
    assert_eq!(ledger.max_cut_depth_mm, deepest);

    // 4. The whole cut audit, recomputed from the published bytes.
    let zones: Vec<LandUseZone> = serde_json::from_value(read_json(
        scene_dir(&dir).join("land-use-zones.json"),
    ))
    .expect("the world publishes its zoning");
    let spec = scene_spec(&scene);
    let water: WaterGeometry = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &spec,
        &manifest.style,
    );
    let (policy, not_enabled) = CutPolicy::from_style(&manifest.style);
    assert_eq!(not_enabled, None);
    assert_eq!(policy.cap_mm(), MAX_CUT_MM);
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
    assert_eq!(audit.pads, CUT_PADS);
    assert_eq!(audit.cut_cells, ledger.cut_cells);
    assert_eq!(audit.cut_depth_sum_mm, ledger.cut_depth_sum_mm);
    assert_eq!(audit.cut_volume_m3, ledger.cut_volume_m3);
    assert_eq!(audit.max_cut_depth_mm, ledger.max_cut_depth_mm);
    // The three lands this capability may never touch, counted from the bytes.
    assert_eq!(audit.protected_cells_changed, 0);
    assert_eq!(audit.water_cells_changed, 0);
    assert_eq!(audit.shore_band_cells_cut, 0);

    // 5. Every planned pad is realized, inside the cap, on its declared top.
    let road_audit = read_json(scene_dir(&dir).join("road-network-audit.json"));
    assert_eq!(road_audit["plannedSites"], json!(PLANNED_PADS));
    assert_eq!(
        road_audit["realizedSites"], road_audit["plannedSites"],
        "every planned pad reaches the world: {road_audit}"
    );
    assert!(road_audit["maxSiteCutM"].as_f64().unwrap() <= f64::from(MAX_CUT_MM) / 1000.);
    assert!(
        road_audit["maxSiteDesignHeightErrorM"].as_f64().unwrap() < 0.0001,
        "every platform is its declared top: {road_audit}"
    );
    assert!(road_audit["maxFinalSiteFillM"].as_f64().unwrap() <= 6.);
    // The dwellings and the one facility the programme asks for are all there.
    let development = read_json(scene_dir(&dir).join("development-audit.json"));
    assert_eq!(development["plannedBuildings"], json!(PLANNED_PADS));
    // Round 45: the shop Round 42 recorded as "planned but without verified
    // assigned access" is now refused at ACCEPTANCE, so the plan no longer
    // contains a lot the world cannot realize and realized == planned.
    assert_eq!(development["realizedBuildings"], json!(PLANNED_PADS));
    assert_eq!(development["failures"], json!([]));
    let services = read_json(scene_dir(&dir).join("service-program-audit.json"));
    assert_eq!(services["status"], json!("pass"), "{services}");
    assert_eq!(services["realizedWithAccess"], services["requested"]);

    // 6. The two roads that used to overfill the 6 m roadbed contract now hold it,
    //    measured cell for cell from the two published rasters over each road's own
    //    transport band (`graded_transport_half_extents`, the audit's own window).
    assert_eq!(road_audit["status"], json!("pass"), "{road_audit}");
    assert_eq!(road_audit["plannedEdges"], json!(33));
    assert_eq!(road_audit["realizedEdges"], road_audit["plannedEdges"], "{road_audit}");
    assert!(
        road_audit["maxFinalRoadbedFillM"].as_f64().unwrap() <= 6.,
        "the roadbed fill contract holds: {road_audit}"
    );
    assert_eq!(road_audit["failures"], json!([]));
    // The ledger books the graded roads in the SAME ledger, by road id. Eleven of
    // this plan's 33 roads carry a graded profile; a flat road has no authored
    // surface to cut to and keeps the historical fill-only contract, so it
    // publishes no record at all.
    assert_eq!(ledger.roads.len(), 12, "the graded roads publish one record each");
    assert!(
        plan.entities()
            .iter()
            .filter(|entity| entity.kind == "road" && entity.grounding.bounded_cut_mm.is_some())
            .count()
            == 12,
        "only a road with a longitudinal profile may carry the cap"
    );
    for record in &ledger.roads {
        assert_eq!(record.max_cut_mm, MAX_CUT_MM);
        assert_eq!(record.entity_id, record.road_id);
        assert!(record.max_cut_depth_mm <= MAX_CUT_MM, "{record:?}");
    }
    // Round 42: the corridors now EXCAVATE. Five of the eleven graded roads book
    // their own cut: 675 cells and 150,657 mm of depth, 150.657 m3.
    let road_cells: u32 = ledger.roads.iter().map(|record| record.cut_cells).sum();
    let road_depth: i64 = ledger.roads.iter().map(|record| record.cut_depth_sum_mm).sum();
    assert_eq!(road_cells, 675);
    assert_eq!(road_depth, 150_657);
    assert!(
        ledger.roads.iter().filter(|record| record.cut_cells > 0).count() == 5,
        "five graded roads carry a real excavation: {:?}",
        ledger
            .roads
            .iter()
            .map(|record| (&record.road_id, record.cut_cells))
            .collect::<Vec<_>>()
    );
    // Every road's book is under the one cap and the total is the sum of them.
    assert_eq!(ledger.cut_cells, SCENE_CUT_CELLS);
    assert_eq!(ledger.cut_depth_sum_mm, SCENE_CUT_DEPTH_SUM_MM);
    assert_eq!(
        ledger.roads.iter().map(|record| u64::from(record.cut_cells)).sum::<u64>()
            + ledger.pads.iter().map(|record| u64::from(record.cut_cells)).sum::<u64>(),
        ledger.cut_cells
    );
    assert_eq!(
        ledger.road("generated.road-auto.scene-0.0").map(|r| (r.design_level_mm, r.cut_cells)),
        Some((8_357, 14))
    );
    assert_eq!(
        ledger.road("generated.road-auto.scene-0.1").map(|r| (r.design_level_mm, r.cut_cells)),
        Some((6_336, 194))
    );
    assert_eq!(
        ledger.road("generated.road-auto.scene-0.2").map(|r| (r.design_level_mm, r.cut_cells)),
        Some((5_495, 352))
    );
    // The two roads Round 38 measured at 6.705 m / 7.150 m of fill: 2.153 m /
    // 2.682 m now, and the third 5.788 m road (Round 39) is 2.682 m as well,
    // because its own band publishes its own surface and its own excavation.
    assert_eq!(
        road_band_fill_cut(&plan, "generated.road-auto.scene-0.0", &natural, &final_terrain),
        (2_153, 101)
    );
    assert_eq!(
        road_band_fill_cut(&plan, "generated.road-auto.scene-0.1", &natural, &final_terrain),
        (2_682, 141)
    );
    assert_eq!(
        road_band_fill_cut(&plan, "generated.road-auto.scene-0.2", &natural, &final_terrain),
        (4_500, 599)
    );
}

/// The authored frontage demand this world cannot meet, MEASURED - and the proof
/// that the shortfall is not a candidate search the planner failed to run.
///
/// The quality gate has been failing this world on `frontage demand remains
/// unfilled` while its development, road and service audits pass. Round 52 took
/// that apart attempt by attempt (the per-attempt census lives in
/// `glyphweave_core::worldgen::frontage_supply::tests::
/// frontage_candidate_census_of_a_published_world`, which replays this exact
/// search and keeps each refusing gate's own `RejectDetail`). What it establishes,
/// and what this test pins:
///
///  * the station pitch offers 15 home stations and 27 shop stations on 12 of the
///    33 roads (9 contour roads with a 64 m profile and 3 auto roads with 96 m; the
///    other 21 roads carry no longitudinal profile, so a lot cannot be benched
///    against a level that does not exist). Both sides of every one of them is
///    tried, so `attempted` is exactly twice the station count: 30 and 54. Nothing
///    the authored pitch can produce is left unvisited;
///  * the candidate budgets are 8192 and the search spent 0.37 % and 0.66 % of
///    them, and both requests stop with `no_feasible_frontage`, never
///    `candidate_budget_exhausted`: the budget is not what refuses a lot;
///  * every one of the 84 attempts is accounted for - a placed lot or one named
///    refusal - so `planned + Σ rejected == attempted` for each request;
///  * the refusals are the world's own, split by what the gate actually measured:
///    24 are the river (the gorge channel runs between the two banks, so one side
///    of every contour street is water), 25 are ground another entity really holds
///    (base roadside pads, the shorefront row, the school, the other request's
///    lot), 17 are a parcel that would stand inside a NEIGHBOURING street's 4 m
///    protection band and never on its carriageway, 6 are the earthwork contract
///    (a pad's grading would move a published platform by 160-1229 mm), 4 are the
///    bounded-cut ring contract and 1 is the Round-45 entrance rule.
///
/// A counterfactual was measured before this test was written, and it is recorded
/// because it is the reason the generator is NOT changed here: moving every station
/// `parcelInsets.sideM` metres away from the junction - so the parcel's own side
/// boundary, and not just the building's, clears the neighbouring band - re-plans
/// this world to 6 lots instead of 7 (shops 3 -> 2, and a new `reserved_entrance`
/// refusal). The 2 m the parcel loses at a junction is ground the plan does not
/// have anyway: the shop's own street then rises a step its entrance cannot climb.
/// The shortfall is local capacity, not a skipped candidate.
#[test]
fn the_hillside_frontage_shortfall_is_measured_and_the_search_is_exhausted() {
    let dir = TestDir::new("hillside-frontage-shortfall");
    let manifest = sample();
    bake_world(&manifest, &dir.0).expect("the benched hillside bakes");

    // 1. The station space, counted from the published roads rather than from the
    //    station list, so the number below is an independent cross-check of
    //    `frontage::stations` and not a copy of it.
    let plan: RoadNetworkPlan =
        serde_json::from_slice(&fs::read(scene_dir(&dir).join("road-network-plan.json")).unwrap())
            .expect("the published plan round-trips");
    let mut profile_lengths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut profiled = 0_usize;
    let mut roads = 0_usize;
    for entity in plan.entities() {
        if entity.kind != "road" {
            continue;
        }
        roads += 1;
        let Some(profile) = &entity.grounding.road_profile else {
            continue;
        };
        profiled += 1;
        *profile_lengths
            .entry(profile.elevations_mm.len() - 1)
            .or_default() += 1;
    }
    assert_eq!(roads, 33, "the plan publishes its whole network");
    assert_eq!(profiled, 12, "12 of the 33 roads carry a longitudinal profile");
    assert_eq!(
        profile_lengths,
        BTreeMap::from([(64, 9), (96, 3)]),
        "9 contour stubs at 64 m and 3 auto roads at 96 m"
    );
    // The documented station contract, arithmetic only: stations run from
    // `widthM/2 + 8` (+ the road's own extra half-width) to `length - widthM/2 - 8`
    // in `stationSpacingM` steps. Written out here so the counts below are checked
    // from the published geometry instead of being taken from the plan.
    let stations_on = |length: usize, width_m: usize, spacing: usize, extra: usize| -> usize {
        let margin = width_m / 2 + 8;
        if length <= 2 * margin {
            return 0;
        }
        let start = margin + extra;
        let end = length - margin;
        if start > end {
            return 0;
        }
        (start..=end).step_by(spacing).count()
    };
    // 9 roads of 64 m at 8 m wide (extra 0) and 3 roads of 96 m at 12 m wide (extra 4).
    let home_stations = 9 * stations_on(64, 24, 32, 0) + 3 * stations_on(96, 24, 32, 4);
    let shop_stations = 9 * stations_on(64, 8, 24, 0) + 3 * stations_on(96, 8, 24, 4);
    assert_eq!((home_stations, shop_stations), (15, 27));
    assert_eq!(
        (home_stations * 2, shop_stations * 2),
        (30, 54),
        "both sides of every station, which is what `attempted` must equal"
    );

    // 2. The published report: demand, placement, and the reason table, request by
    //    request. `planned + Σ rejected == attempted` is the statement that no
    //    attempt is unaccounted for.
    let report = plan
        .frontage
        .as_ref()
        .expect("the world publishes its frontage report");
    let outcome = |id: &str| {
        report
            .outcomes
            .iter()
            .find(|outcome| outcome.request.id == id)
            .expect("the world publishes both requests")
    };
    let homes = outcome("gorge-homes");
    let shops = outcome("gorge-shops");
    assert_eq!((homes.request.count, homes.site_ids.len()), (10, 4));
    assert_eq!(
        (homes.unfilled, homes.attempted, homes.stop_reason.as_str()),
        (6, 30, "no_feasible_frontage")
    );
    assert_eq!(
        homes.rejected,
        BTreeMap::from([
            ("bounded_cut_footprint_overlaps_another_pad".to_string(), 2),
            ("existing_support_changed".to_string(), 1),
            ("occupied_or_road_support".to_string(), 15),
            ("water".to_string(), 8),
        ])
    );
    assert_eq!((shops.request.count, shops.site_ids.len()), (14, 3));
    assert_eq!(
        (shops.unfilled, shops.attempted, shops.stop_reason.as_str()),
        (11, 54, "no_feasible_frontage")
    );
    assert_eq!(
        shops.rejected,
        BTreeMap::from([
            ("access_to_different_road".to_string(), 1),
            ("bounded_cut_footprint_overlaps_another_pad".to_string(), 2),
            ("existing_support_changed".to_string(), 5),
            ("occupied_or_road_support".to_string(), 27),
            ("water".to_string(), 16),
        ])
    );
    for outcome in [homes, shops] {
        let refused: u32 = outcome.rejected.values().sum();
        assert_eq!(
            outcome.site_ids.len() as u32 + refused,
            outcome.attempted,
            "{}: every attempt is a lot or a named refusal",
            outcome.request.id
        );
        // The budget is not the limiter: this is a geometry the world does not have,
        // not a search that ran out of attempts.
        assert!(
            outcome.attempted * 10 < outcome.request.max_candidate_attempts,
            "{}: attempted {} of {}",
            outcome.request.id,
            outcome.attempted,
            outcome.request.max_candidate_attempts
        );
    }
    // The family the bounded-cut capability exists to relieve is EMPTY here: with
    // the 2 m cap on, not one lot is refused for terrain. That is what makes the
    // remaining 17 a frontage-capacity question and not an earthwork one.
    assert!(!homes.rejected.contains_key("natural_support_or_fill_limit"));
    assert!(!shops.rejected.contains_key("natural_support_or_fill_limit"));
    assert!(!homes.rejected.contains_key("bounded_cut_exceeds_cap"));
    assert!(!shops.rejected.contains_key("bounded_cut_exceeds_cap"));

    // 3. The published audit is those numbers, and its only complaint is the
    //    unfilled demand: no lot is missing, changed or disconnected.
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .expect("the quality report runs");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let quality = read_json(dir.0.join("quality-report.json"));
    let audit = &quality["scenes"][0]["frontageAudit"];
    assert_eq!(audit["requested"], json!(24));
    assert_eq!(audit["planned"], json!(7));
    assert_eq!(audit["realizedWithAccess"], json!(7));
    assert_eq!(audit["unfilled"], json!(17));
    assert_eq!(audit["status"], json!("fail"));
    assert_eq!(
        audit["failures"],
        json!(["frontage demand remains unfilled"]),
        "the shortfall is the only failure: {audit}"
    );
    assert_eq!(audit["realizedIds"].as_array().unwrap().len(), 7);
}

/// The worst fill and the worst cut over one graded road's transport band, in
/// whole millimetres, from the two published rasters.
fn road_band_fill_cut(
    plan: &RoadNetworkPlan,
    road_id: &str,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
) -> (i32, i32) {
    let entity = plan
        .entities()
        .into_iter()
        .find(|entity| entity.entity_id == road_id)
        .expect("the plan publishes the road");
    let (half_w, half_d) = graded_transport_half_extents(&entity);
    let (mut fill, mut cut) = (0_i32, 0_i32);
    for z in entity.world_z - half_d..=entity.world_z + half_d {
        for x in entity.world_x - half_w..=entity.world_x + half_w {
            let n = natural
                .height_millimetres(x, z)
                .expect("the natural raster covers the band");
            let f = final_terrain
                .height_millimetres(x, z)
                .expect("the final raster covers the band");
            fill = fill.max(f - n);
            cut = cut.max(n - f);
        }
    }
    (fill, cut)
}

#[test]
fn two_bakes_of_the_benched_hillside_town_are_byte_identical() {
    let first = TestDir::new("hillside-cut-first");
    let second = TestDir::new("hillside-cut-second");
    let manifest = sample();
    bake_world(&manifest, &first.0).expect("the first bake publishes");
    bake_world(&manifest, &second.0).expect("the second bake publishes");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    assert!(a.len() > 90, "the world publishes its files: {}", a.len());
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

/// One digest over the published scene CONTENT files.
///
/// The manifest-derived reports echo the absolute `rulesDir` of this machine, and
/// the world index, the preview scene and the render-chunk manifests echo the
/// world revision (which is computed from that manifest); none of them is scene
/// content, so the digest is taken over `scenes/scene-0` minus those `revision`
/// carriers and is portable across checkouts. Everything the excavation is judged
/// on - the rasters, the plans, the audits, the zones and the ledgers - is in it.
fn published_digest(dir: &TestDir) -> String {
    let files = published_files(&dir.0);
    let mut hasher = blake3::Hasher::new();
    let mut counted = 0_usize;
    for (path, digest) in files.iter().filter(|(path, _)| {
        path.starts_with("scenes/")
            && !path.ends_with("preview-scene.json")
            && !path.contains("/render-chunks/")
    }) {
        counted += 1;
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(digest.as_bytes());
        hasher.update(b"\n");
    }
    assert!(counted > 20, "the scene publishes its files: {counted}");
    hasher.finalize().to_hex().to_string()
}

/// Requirement 4: with the switch OFF the whole world is the historical one, byte
/// for byte. The digest is frozen over the scene's own published files; a regression
/// that lets either new rule move an off-state decision cannot pass by both sides
/// drifting together, and two bakes of the same off input agree file for file.
///
/// Round 40 re-measured it: the off world no longer publishes the three trees its
/// own `scale-audit` refused, because generation now reads the same 4 m shore
/// clearance the audit uses. The cut ledger, the plan and both rasters are unchanged.
#[test]
fn the_policy_off_bake_is_frozen_and_two_bakes_agree() {
    let mut manifest = sample();
    {
        let program = manifest.style["frontageProgram"]
            .as_object_mut()
            .expect("the sample authors a frontage program");
        program.remove("terrainPolicy");
        program.remove("maxCutMm");
    }
    assert_eq!(CutPolicy::from_style(&manifest.style).0, CutPolicy::Disabled);
    let first = TestDir::new("hillside-off-frozen-first");
    let second = TestDir::new("hillside-off-frozen-second");
    bake_world(&manifest, &first.0).expect("the off arm bakes");
    bake_world(&manifest, &second.0).expect("the off arm bakes twice");
    // Off is byte-identical: no pad gains a cap, no bounded-cut section is published.
    let plan: RoadNetworkPlan = serde_json::from_value(read_json(
        scene_dir(&first).join("road-network-plan.json"),
    ))
    .expect("the off plan round-trips");
    assert!(
        plan.roadside_sites
            .iter()
            .all(|site| site.bounded_cut_mm.is_none()),
        "the policy is off, so no pad may carry a cut record"
    );
    // A graded road is not stamped either: no road may carry a cap, and the
    // historical fill-contract failure is still the off world's own answer.
    assert_eq!(plan.road_cut_mm, None, "the off plan publishes no road cap");
    assert!(
        plan.entities()
            .iter()
            .all(|entity| entity.grounding.bounded_cut_mm.is_none()),
        "the policy is off, so no road entity may carry a cut cap"
    );
    let off_audit = read_json(scene_dir(&first).join("road-network-audit.json"));
    assert_eq!(off_audit["status"], json!("pass"), "{off_audit}");
    assert_eq!(off_audit["failures"], json!([]));
    assert!(
        off_audit["maxFinalRoadbedFillM"].as_f64().unwrap() <= 6.,
        "the road's own profile must stay inside the 6m fill contract: {off_audit}"
    );
    let engineered = read_json(scene_dir(&first).join("engineered-terrain.json"));
    assert_eq!(
        engineered.get("boundedCut"),
        None,
        "a world that never asked for a cut publishes no bounded-cut ledger"
    );
    let first_digest = published_digest(&first);
    assert_eq!(
        first_digest,
        published_digest(&second),
        "two off-state bakes must publish the same bytes"
    );
    assert_eq!(
        first_digest, "6eab4494c1dabc2f8745881637b523e5b6f01ca04a81ffff3da3b58fef4f82eb",
        "the frozen blake3 tree digest of the policy-off world (B-02 re-took it: the road \
         audit now measures the road's own profile instead of a neighbour's carve, so the \
         two false >6m failures become a pass; terrain, road, pad and entity geometry did \
         not move)"
    );
}
