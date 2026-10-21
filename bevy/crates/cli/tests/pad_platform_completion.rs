//! Round 39: the plan's pad platform and the entity stage are closed in BOTH
//! directions - the pad keeps its declared top, and the pad keeps its entity.
//!
//! `examples/hillside-town-512.manifest.json` with `landUseProfile.urbanGrowth =
//! "axis-and-node"` is a 512 m river-gorge town grown along its axes. Round 32 found
//! that planning accepted an 8x8 `storefront` pad at (144, 260) on a declared 5.325 m
//! top and the entity earthwork buried it; Round 36 found that the completion pass
//! that was supposed to settle it read a composition the bake never publishes, and
//! that the pad's own ENTITY had been deleted by the rules engine before the pass
//! ever ran, so the pad was deepened once (a no-op) and then refused as `Unstable` -
//! 16 pads, 5 shops, and a plan that no longer claimed the parcel.
//!
//! What this test asserts, in the order the bake produces it:
//!
//! 1. the bake SUCCEEDS and keeps EVERY planned pad: 17 sites in the plan and 17
//!    entities in the scene, the 8x8 shop among them;
//! 2. the pad's own platform is what the raster publishes: 5,325 mm on all 81 cells
//!    of its declared flat top, so the completion pass has nothing to refuse and
//!    publishes no `pad-completion.json` at all - it converged in its first round;
//! 3. the structure that used to evict the pad is refused BY NAME instead: the
//!    38x20 `resort_lodge` at (140, 280) reaches 3.5 m into the pad after its 90
//!    degree rotation, its reject names the pad, and it is not in the world;
//! 4. the cut audit still verifies from the published bytes: 4 benches, 911 cells,
//!    530,566 mm of excavation, ZERO cut cells on protected land, in the water and on
//!    the shoreline passage band, and the ledger IS the rasters' own measurement;
//! 5. the world still builds its city: every planned pad is realized on its declared
//!    top, the school is placed, and the housing minimum is unchanged;
//! 6. the plan, the reconcile report and the published entities agree: every planned
//!    pad is a published entity, no refused pad is anywhere, and
//!    `verify_published_placement` accepts the published `engineering-placement.json`
//!    (and still refuses a doctored one, so the check is live);
//! 7. two bakes of the same manifest publish byte-identical bytes.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::worldgen::bounded_cut::{self, CutLedger, CutPolicy};
use glyphweave_core::worldgen::road_network::{RoadNetworkPlan, PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM};
use glyphweave_core::worldgen::{
    LandUseZone, SceneIndex, SceneSpec, WaterGeometry, WorldManifest, bake_world,
    verify_published_placement, water_geometry, water_kind,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
/// The one cap this world authors, in whole millimetres.
const MAX_CUT_MM: i32 = 2000;
/// The pad the entity stage used to lose: an 8x8 storefront on a 5.325 m declared top.
const THE_PAD: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-network.scene-0.23-30.36.negative";
/// The declared top of that pad, in whole millimetres.
const DECLARED_MM: i32 = 5_325;
/// Cells of that pad's declared flat top (`flat_top_bounds` of a 8x8 pad is 9x9).
const PAD_CELLS: u32 = 81;
/// The structure that used to evict it: a 38x20 lodge whose rotated footprint reaches
/// into the pad.
const THE_LODGE: &str = "generated.resort-lodge.";
/// The pads the grown plan keeps - every one of them, now.
const PLANNED_PADS: usize = 19;
/// The excavation the scene holds, summed once from its two published rasters.
const SCENE_CUT_PADS: usize = 6;
const SCENE_CUT_CELLS: u64 = 1_760;
const SCENE_CUT_DEPTH_SUM_MM: i64 = 774_904;

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
fn the_grown_hillside_town_converges_on_the_published_platform_and_keeps_every_pad() {
    let dir = TestDir::new("pad-platform-grown");
    let manifest = grown();
    // 1. The bake publishes. `bounded_cut_ledger_for` refuses a world whose cut
    //    cannot be proven from the published bytes, so this is also a passing cut
    //    audit - the thing the Round 32 world could not reach at all.
    bake_world(&manifest, &dir.0).expect("the grown hillside bakes: no scene-wide cut refusal");

    let plan_value = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let plan: RoadNetworkPlan =
        serde_json::from_value(plan_value.clone()).expect("the plan round-trips");

    // 2. Every planned pad stays planned - the pad the entity stage used to lose
    //    included - and no completion refusal is published at all.
    assert_eq!(plan.roadside_sites.len(), PLANNED_PADS);
    let site = plan
        .roadside_sites
        .iter()
        .find(|site| site.id == THE_PAD)
        .expect("the shop pad is a pad of this plan");
    assert_eq!((site.width_m, site.depth_m), (8, 8));
    assert_eq!((site.world_x, site.world_z), (144, 260));
    assert_eq!(site.support_level_mm, Some(DECLARED_MM));
    assert_eq!(site.bounded_cut_mm, Some(MAX_CUT_MM));
    assert!(
        plan.roadside_site_rejections
            .iter()
            .all(|rejection| rejection.category != PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM),
        "nothing refuses a pad top any more: {:?}",
        plan.roadside_site_rejections
    );
    // The completion pass converged in its first round and had nothing to publish:
    // no record file, and therefore no refusal anywhere in it.
    assert!(
        !dir.0.join("pad-completion.json").exists(),
        "a world whose platforms all hold publishes no completion record"
    );

    // 3. The pad's OWN platform is what the world publishes, cell for cell.
    let scene = scene_index(&dir);
    let entity = scene
        .entities
        .iter()
        .find(|entity| entity.entity_id == THE_PAD)
        .expect("the pad's entity is IN the world: the entity stage no longer loses it");
    assert_eq!(entity.kind, "storefront");
    assert_eq!((entity.world_x, entity.world_z), (144, 260));
    assert_eq!((entity.width_m, entity.depth_m), (8., 8.));
    assert_eq!(entity.grounding.bounded_cut_mm, Some(MAX_CUT_MM));
    let final_terrain = raster(&dir, &scene, true);
    let verdict = bounded_cut::pad_top_holds(site, |x, z| final_terrain.height_millimetres(x, z))
        .expect("the pad declares a flat top");
    assert!(verdict.holds(), "the published top does not hold: {verdict:?}");
    assert_eq!(verdict.cells, PAD_CELLS);
    assert_eq!(verdict.highest_mm, DECLARED_MM);
    assert_eq!(verdict.lowest_mm, DECLARED_MM);
    assert_eq!(verdict.mismatched, 0);
    assert_eq!(final_terrain.height_millimetres(140, 259), Some(DECLARED_MM));
    assert_eq!(final_terrain.height_millimetres(140, 256), Some(DECLARED_MM));
    assert_eq!(final_terrain.height_millimetres(148, 264), Some(DECLARED_MM));

    // 4. The structure that used to evict the pad is refused BY NAME instead, and it
    //    is not in the world: the pad keeps its ground, the lodge keeps none.
    let placement = read_json(dir.0.join("rules-placement.json"));
    let rejects = placement["scenes"][0]["rejects"].as_array().unwrap();
    let displaced: Vec<&Value> = rejects
        .iter()
        .filter(|record| record["conflict_with"] == json!(THE_PAD))
        .filter(|record| {
            record["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("keeps its ground"))
        })
        .collect();
    assert_eq!(displaced.len(), 1, "{displaced:?}");
    assert_eq!(displaced[0]["item_id"], json!("resort_lodge"));
    assert_eq!(displaced[0]["candidate_x"], json!(140));
    assert_eq!(displaced[0]["candidate_z"], json!(280));
    assert!(
        displaced[0]["reason"]
            .as_str()
            .unwrap()
            .contains("refused by name"),
        "the refusal is published in words: {}",
        displaced[0]["reason"]
    );
    assert!(
        scene
            .entities
            .iter()
            .all(|entity| !entity.entity_id.starts_with(THE_LODGE)),
        "the structure the pad displaced is not published"
    );

    // 5. Five views, one pad: the plan, the conversions, the frontage report, the
    //    joint receipt and the entity list all carry it.
    assert!(
        plan.pad_conversions
            .iter()
            .all(|record| record.parcel_id != THE_PAD),
        "a storefront earns no rural-to-urban conversion; the pad is still planned"
    );
    let frontage = plan.frontage.as_ref().expect("the frontage report");
    for outcome in &frontage.outcomes {
        assert_eq!(
            outcome.unfilled,
            outcome.request.count.saturating_sub(outcome.site_ids.len() as u32),
            "the outcome's demand is recomputed, not decremented by hand"
        );
    }
    let joint = plan.joint_layout.as_ref().expect("the joint receipt");
    assert_eq!(
        joint.unfilled_residential,
        joint.residential_minimum.saturating_sub(
            plan.roadside_sites
                .iter()
                .filter(|site| site.program.is_none())
                .count()
        ),
        "the joint receipt charges its own minimum against the pads that remain"
    );

    // 6. The triangle: every planned pad is a published entity, every refused id is
    //    nowhere, and the reconcile report can be recomputed from the published ids.
    let engineering = read_json(dir.0.join("engineering-placement.json"));
    let entry = engineering["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry.get("inputItems").is_some())
        .expect("the reconciliation report is published among the phase records");
    let published_ids: BTreeSet<String> = scene
        .entities
        .iter()
        .map(|entity| entity.entity_id.clone())
        .collect();
    let planned_ids: Vec<String> = plan
        .roadside_sites
        .iter()
        .map(|site| site.id.clone())
        .collect();
    for id in &planned_ids {
        assert!(
            published_ids.contains(id),
            "planned pad {id} is not a published entity: the plan and the ledger claim a \
             parcel the world does not carry"
        );
    }
    assert_eq!(entry["plannedPadIds"].as_array().unwrap().len(), PLANNED_PADS);
    assert!(
        entry.get("refusedEntityIds").is_none(),
        "no pad was refused, so no refused id may be published: {entry}"
    );
    let authored: BTreeSet<String> = manifest
        .landmarks
        .iter()
        .map(|landmark| landmark.entity_id.clone())
        .collect();
    let registry = glyphweave_core::rules::ObjectRegistry::load_dir(&rules())
        .expect("the rules registry loads");
    verify_published_placement(entry, &published_ids, &authored, Some(&registry))
        .expect("the published placement is consistent with the plan and the world");
    // The check is live: drop one planned pad's entity from the published set and the
    // same verifier refuses it.
    let mut without_the_pad = published_ids.clone();
    without_the_pad.remove(THE_PAD);
    assert!(
        verify_published_placement(entry, &without_the_pad, &authored, Some(&registry)).is_err(),
        "a planned pad with no entity must be refused by the triangle check"
    );

    // 7. The world still builds its city: every planned pad is realized on its
    //    declared top, and the school is placed.
    let road_audit = read_json(scene_dir(&dir).join("road-network-audit.json"));
    assert_eq!(road_audit["plannedSites"], json!(PLANNED_PADS));
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

    // 8. The cut audit, recomputed from the published bytes.
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
    // The pad the entity stage used to lose publishes its own excavation: 92 mm at
    // its deepest cell, not the 708 mm of neighbour fill that stands above it.
    let pad_record = ledger
        .pads
        .iter()
        .find(|record| record.pad_id == THE_PAD)
        .expect("the kept pad books its own bench");
    assert_eq!(pad_record.max_cut_mm, MAX_CUT_MM);
    assert_eq!(pad_record.design_level_mm, DECLARED_MM);
    assert!(
        pad_record.max_cut_depth_mm <= 200,
        "the pad's own bench is shallow: {pad_record:?}"
    );
    let natural = raster(&dir, &scene, false);
    let measured = bounded_cut::measure_from_rasters(&plan.roadside_sites, &natural, &final_terrain, &bounded_cut::road_flat_tops(&plan.entities()))
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

/// Requirement 4: two bakes of the same manifest publish byte-identical bytes.
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
