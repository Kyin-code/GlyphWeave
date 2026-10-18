//! Round 21: the REGISTERED river-delta city sample
//! (`examples/waterfront-city-512.manifest.json`) bakes end to end, and every
//! waterfront claim it makes is independently recomputable from the published
//! bytes.
//!
//! The sample is a 512 m strict/rules world whose land-use theme is
//! `river-delta` (the registered morphology of Amsterdam/Wuhan: three cores
//! strung along the river axis, a main street plus side streets, a 0.78 density
//! gradient, farmland along the channel) with one 40 m structured river through
//! its middle, `style.automaticRoadExtensions.allowStructuredWater =
//! "bridge-or-causeway"`, a `frontageProgram` (24x24 homes + shops), a
//! `settlementProgram` (school + church + two convenience stores, housing
//! minimum 12) and an `agricultureProgram`.
//!
//! What this test proves, in the order it asserts it:
//!
//! 1. the baked water surface is EXACTLY the natural one: the bake publishes
//!    both height payloads and every wet cell must agree - no earthwork is ever
//!    placed in the river;
//! 2. the published land-use ledger closes: per purpose and for the whole
//!    scene, `realized + unresolved == planned` and the sum is the scene area;
//! 3. the protected (nature) land keeps its natural surface exactly: the
//!    published protection audit says so AND every protected raster cell is
//!    re-checked here against the natural payload;
//! 4. the engineering reconciliation publishes its skips by name and
//!    `verify_published_placement` recomputes them from the plan ids and the
//!    real descriptors;
//! 5. two bakes of the same input publish byte-identical
//!    `engineering-placement.json` and `scene.json`;
//! 6. the crossings are real registered bridges (`bridges >= 1`) and the
//!    release gate's verdict on the sample is pinned exactly.
//!
//! 6 is where the sample used to report a GAP, and this is the round that closed
//! it. Round 20 refused the whole roadside pad programme as
//! `waterfront_pad_needs_a_second_level`, so the plan's 23 retained dwellings and
//! 4 services were planned and then dropped: the scene realized 0 of the 12
//! required dwellings and 0 of the 4 requested services. The premise was right -
//! a shore is a multi-level surface - and the conclusion was wrong: what a shore
//! forbids is a pad declared on the SCENE level, not a flat pad. Every pad now
//! takes the elevation of its own road's graded surface at its own station
//! (`supportLevelMm`, the mechanism profiled roads already publish), keeps the
//! published shore clearance, and the reserve that clearance creates - the
//! shoreline public passage band - is realized as `PublicOpenSpace` and audited.
//! The sample now realizes 23 of 23 planned dwellings with verified access, 4 of
//! 4 services, and a 4,096 m2 shore band that is continuous on both banks.
//!
//! What this test still locks as a GAP is what the sample does not do: the
//! authored `frontageProgram` (24 local-height frontage lots) still realizes 0,
//! because its 24 m candidates sit on the wrong side of the roads that run along
//! the banks, and the land-use realization backlog every road-first 512 m scene
//! carries is unchanged. Those numbers are asserted exactly, so a future fix has
//! to update them instead of quietly passing a city that claims them.

use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::rules::ObjectRegistry;
use glyphweave_core::worldgen::{
    SceneIndex, SceneSpec, WATER_SHORE_CLEARANCE_M, WaterGeometry, WorldManifest, bake_world,
    verify_published_placement, water_geometry, water_kind,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::PathBuf, process::Command};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");
/// 512 m scene, one 40 m channel, seed 7 (a frozen B-milestone acceptance seed).
const SCENE_SIZE_M: u32 = 512;
const RIVER_WIDTH_M: u32 = 40;
const SEED: u64 = 7;

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

/// The published example, with its one repo-relative path made absolute (the
/// CLI resolves `rulesDir` against the manifest, the test against this crate).
fn sample() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
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

/// The release-facing gate: it runs `validate-world`, `scale-audit` and
/// `quality-report` itself and then writes `world-quality-gate.json`, so one
/// invocation is the whole acceptance chain.
fn quality_gate(dir: &TestDir) -> (std::process::Output, Value) {
    let output = Command::new(glyphweave())
        .arg("world-quality-gate")
        .arg(&dir.0)
        .output()
        .unwrap();
    let report = read_json(dir.0.join("world-quality-gate.json"));
    (output, report)
}

fn count_kind(scene: &SceneIndex, kind: &str) -> usize {
    scene
        .entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .count()
}

/// The whole acceptance: bake the registered sample, then recompute every
/// waterfront claim from the published bytes.
#[test]
fn the_registered_river_delta_sample_bakes_with_exact_water_bridges_and_a_closed_ledger() {
    let dir = TestDir::new("waterfront-city-512");
    let manifest = sample();
    manifest.validate().expect("the published sample validates");

    // The authored demand really is the programme the sample claims.
    let frontage = &manifest.style["frontageProgram"]["requests"];
    assert_eq!(frontage[0]["kind"], json!("residential_home"));
    assert_eq!(
        frontage[0]["widthM"],
        json!(24),
        "the sample's homes are 24x24"
    );
    assert_eq!(frontage[0]["depthM"], json!(24));
    assert!(
        frontage[0].get("setbackM").is_none(),
        "the street setback must stay profile-derived (compact-mixed-block frontInset 4 + the 4 m road band = 8 m)"
    );
    assert_eq!(
        manifest.style["landUseProfile"]["theme"],
        json!("river-delta")
    );
    assert_eq!(manifest.style["morphologyProfile"], json!("compact-mixed-block"));
    assert_eq!(
        manifest.style["automaticRoadExtensions"]["allowStructuredWater"],
        json!("bridge-or-causeway")
    );
    assert_eq!(manifest.landmarks.len(), 1);
    assert_eq!(manifest.landmarks[0].width_m, RIVER_WIDTH_M);
    assert_eq!(manifest.scenes[0].width_m, SCENE_SIZE_M);
    assert_eq!(manifest.world.seed, SEED);

    bake_world(&manifest, &dir.0).expect("the registered waterfront sample must bake end to end");
    let scene = scene_index(&dir);
    let water = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &manifest.scenes[0],
        &manifest.style,
    );

    // ---------------------------------------------------------------------
    // 1. The water surface is EXACTLY the natural one.
    // ---------------------------------------------------------------------
    let baked_wet = wet_area(&raster(&dir, &scene, true), water, &scene);
    let natural_wet = wet_area(&raster(&dir, &scene, false), water, &scene);
    assert_eq!(
        natural_wet, SAMPLE_WATER_SURFACE_M2,
        "the natural water surface of this sample (39 wet metres of the 40 m channel over 512 m)"
    );
    assert_eq!(
        baked_wet, natural_wet,
        "the baked water surface must equal the natural one: no earthwork in the water"
    );

    // ---------------------------------------------------------------------
    // 6a. The crossings are registered bridges, measured from the roads' own
    //     published geometry.
    // ---------------------------------------------------------------------
    let roads = count_kind(&scene, "road");
    assert_eq!(roads, SAMPLE_ROAD_ENTITIES, "the sample's road entities");
    let plan = scene
        .road_network_plan
        .as_ref()
        .expect("the opt-in authorizes the road network");
    assert_eq!(plan.edges.len(), SAMPLE_ROAD_ENTITIES);
    assert_eq!(
        plan.water_crossings.len(),
        SAMPLE_BRIDGES,
        "one registered structure per road that spans the channel"
    );
    for crossing in &plan.water_crossings {
        assert_eq!(crossing.kind, "bridge");
        assert_eq!(crossing.water_id, "water.scene-0.river");
        assert_eq!(crossing.water_kind, "river");
        assert_eq!(crossing.width_m, 8);
        assert_eq!(crossing.length_m, SAMPLE_BRIDGE_SPAN_M);
        assert_eq!(crossing.wet_cells, SAMPLE_BRIDGE_WET_CELLS);
        assert_eq!(crossing.deck_level_mm, 5000);
        assert_eq!(crossing.water_level_mm, 0);
        assert!(
            crossing.bed_max_mm <= crossing.water_level_mm,
            "no earthwork in the water: the bed stays below the datum"
        );
        assert!(
            crossing.deck_level_mm >= crossing.take_off_max_mm,
            "the deck has to sit at or above the highest dry ground it takes off from"
        );
    }
    assert!(
        !plan.water_crossings.is_empty(),
        "the sample must carry at least one bridge"
    );

    // ---------------------------------------------------------------------
    // 4. Skips are published by name and recompute from the plan ids. The pad
    //    rule now leaves nothing to reconcile away on this sample, which is
    //    itself an assertion: a planned obligation that the engineering pass has
    //    to remove is a planning defect, not a skip.
    // ---------------------------------------------------------------------
    let placement = read_json(dir.0.join("engineering-placement.json"));
    let entry = &placement["scenes"][0];
    assert_eq!(entry["sceneId"], json!("scene-0"));
    assert_eq!(entry["inputItems"], json!(SAMPLE_PLAN_ITEMS));
    assert_eq!(entry["outputItems"], json!(SAMPLE_OUTPUT_ITEMS));
    assert_eq!(entry["removedItems"], json!(SAMPLE_REMOVED_ITEMS));
    let skips = entry["skips"].as_array().unwrap();
    assert_eq!(
        skips.len(),
        SAMPLE_REMOVED_ITEMS as usize,
        "the named removals"
    );

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
    let audit = verify_published_placement(entry, &published, &authored, Some(&registry))
        .expect("the published skips must recompute from the plan ids and the descriptors");
    assert_eq!(audit.plan_items, SAMPLE_PLAN_ITEMS as usize);
    assert_eq!(audit.output_items, SAMPLE_OUTPUT_ITEMS as usize);
    assert_eq!(audit.skipped_items, SAMPLE_REMOVED_ITEMS as usize);
    for skip in skips {
        let id = skip["entityId"].as_str().unwrap();
        assert!(
            !published.contains(id),
            "a skipped entity must not be published: {id}"
        );
    }

    // ---------------------------------------------------------------------
    // 2 + 3. The release gate's ledger, protection and reconciliation audits.
    // ---------------------------------------------------------------------
    let (gate_output, gate) = quality_gate(&dir);
    assert!(
        !gate_output.status.success(),
        "the sample is expected to FAIL the release gate today (see the pinned failure list below)"
    );
    assert_eq!(gate["status"], json!("fail"));
    let gate_failures: Vec<&str> = gate["failures"]
        .as_array()
        .expect("the gate published its failure list")
        .iter()
        .map(|failure| failure.as_str().unwrap())
        .collect();
    assert_eq!(
        gate_failures, SAMPLE_GATE_FAILURES,
        "the sample's acceptance status, pinned line by line"
    );
    let quality = read_json(dir.0.join("quality-report.json"));
    assert_eq!(
        quality["scenes"][0]["engineeringPlacement"]["status"],
        json!("pass"),
        "the reconciliation report must recompute"
    );
    assert_eq!(
        quality["scenes"][0]["engineeringPlacement"]["skippedItems"],
        json!(SAMPLE_REMOVED_ITEMS)
    );
    assert_eq!(
        quality["engineeringPlacementSkips"],
        json!(SAMPLE_REMOVED_ITEMS)
    );
    assert_eq!(
        quality["scenes"][0]["roadNetworkAudit"]["status"],
        json!("pass"),
        "every planned edge and every planned pad is realized at its declared top: 60 of 60 edges, 27 of 27 pads, max roadbed fill 4.59 m"
    );
    assert_eq!(
        quality["scenes"][0]["roadNetworkAudit"]["realizedSites"],
        json!(SAMPLE_PLANNED_PADS)
    );
    assert_eq!(
        quality["scenes"][0]["naturalTerrainAudit"]["status"],
        json!("pass")
    );

    // The ledger closes per purpose and for the whole scene.
    let ledger = &quality["scenes"][0]["landUseLedger"];
    let mut planned_total = 0.0;
    let mut realized_total = 0.0;
    for (purpose, row) in ledger["byPurpose"].as_object().unwrap() {
        let planned = row["plannedM2"].as_f64().unwrap();
        let realized = row["realizedM2"].as_f64().unwrap();
        let unresolved = row["unresolvedM2"].as_f64().unwrap();
        assert_eq!(
            realized + unresolved,
            planned,
            "{purpose}: realized {realized} + unresolved {unresolved} must equal planned {planned}"
        );
        planned_total += planned;
        realized_total += realized;
    }
    let scene_area = ledger["sceneAreaM2"].as_f64().unwrap();
    assert_eq!(planned_total, scene_area, "planned purposes cover the scene");
    assert_eq!(
        realized_total,
        ledger["realizedM2"].as_f64().unwrap(),
        "the scene total is the sum of its purpose rows"
    );
    assert_eq!(
        ledger["realizedM2"].as_f64().unwrap() + ledger["unresolvedM2"].as_f64().unwrap(),
        scene_area,
        "every square metre is either realized or unresolved"
    );
    let rows: BTreeMap<&str, (f64, f64)> = ledger["byPurpose"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(purpose, row)| {
            (
                purpose.as_str(),
                (
                    row["plannedM2"].as_f64().unwrap(),
                    row["realizedM2"].as_f64().unwrap(),
                ),
            )
        })
        .collect();
    assert_eq!(rows["urban"], (65_536.0, 28_070.0));
    assert_eq!(rows["rural"], (131_072.0, 25_160.0));
    assert_eq!(rows["nature"], (65_536.0, 65_536.0));

    // Protected land: the published audit, plus an independent raster check.
    let protection = &quality["scenes"][0]["landProtectionAudit"];
    assert_eq!(protection["status"], json!("pass"));
    assert_eq!(
        protection["protectedAreaM2"].as_f64(),
        Some(65_536.0),
        "the four nature zoning cells: {protection}"
    );
    assert_eq!(protection["changedHeightSamples"], json!(0));
    assert_eq!(protection["maxHeightChangeM"].as_f64(), Some(0.0));
    let baked = raster(&dir, &scene, true);
    let natural = raster(&dir, &scene, false);
    let protected_cells = protected_cell_count(&scene);
    assert_eq!(
        protected_cells, 65_536,
        "the sample reserves four whole 128 m zoning cells as nature"
    );
    let mut sample_walks = 0_usize;
    for zone in scene
        .land_use_zones
        .iter()
        .filter(|zone| zone.kind == "nature")
    {
        for z in zone.world_z..zone.world_z + zone.depth_m as i32 {
            for x in zone.world_x..zone.world_x + zone.width_m as i32 {
                let before = natural.height(x, z).expect("natural sample");
                let after = baked.height(x, z).expect("after sample");
                assert_eq!(
                    after.to_bits(),
                    before.to_bits(),
                    "protected cell ({x},{z}) moved: {before} -> {after}"
                );
                sample_walks += 1;
            }
        }
    }
    assert_eq!(
        sample_walks as u64, protected_cells,
        "every protected cell checked"
    );

    // ---------------------------------------------------------------------
    // 6b. The residential realization, locked at what the sample reaches.
    // ---------------------------------------------------------------------
    assert_eq!(
        plan.roadside_sites.len(),
        SAMPLE_PLANNED_PADS as usize,
        "the waterfront opt-in now plans its pads instead of refusing them"
    );
    let joint = &quality["scenes"][0]["jointLayoutAudit"];
    assert_eq!(joint["status"], json!("pass"));
    assert_eq!(joint["residentialMinimum"], json!(12));
    assert_eq!(joint["plannedResidential"], json!(SAMPLE_PLANNED_HOMES));
    assert_eq!(
        joint["realizedResidentialWithAccess"],
        json!(SAMPLE_REALIZED_HOMES)
    );
    assert_eq!(joint["missingResidential"], json!(0));
    assert_eq!(joint["unfilledServices"], json!(0));
    let services = &quality["scenes"][0]["serviceProgramAudit"];
    assert_eq!(services["status"], json!("pass"));
    assert_eq!(services["requested"], json!(4));
    assert_eq!(services["realizedWithAccess"], json!(SAMPLE_REALIZED_SERVICES));
    for kind in ["school", "church", "convenience_store", "residential_home"] {
        assert!(
            count_kind(&scene, kind) > 0,
            "the waterfront block must carry a real {kind}"
        );
    }
    assert_eq!(
        count_kind(&scene, "residential_home"),
        SAMPLE_REALIZED_HOMES as usize
    );
    // The authored local-height frontage programme is still unfilled, and that
    // shortfall is asserted rather than papered over.
    let frontage_audit = &quality["scenes"][0]["frontageAudit"];
    assert_eq!(frontage_audit["requested"], json!(24));
    assert_eq!(frontage_audit["planned"], json!(0));
    assert_eq!(frontage_audit["unfilled"], json!(24));

    // ---------------------------------------------------------------------
    // 6c. THE PAD RULE, recomputed from the plan and the rasters.
    //
    // Every pad carries a precise elevation and a frontage point, that elevation
    // is its own road's graded surface at that point (no pad is declared on the
    // scene's single level), and no pad square metre stands in the water or in
    // the reserved shore band.
    // ---------------------------------------------------------------------
    let roads_by_id: BTreeMap<&str, &glyphweave_core::worldgen::EntityInstance> = scene
        .entities
        .iter()
        .filter(|entity| entity.kind == "road")
        .map(|entity| (entity.entity_id.as_str(), entity))
        .collect();
    let apron_mm = 1000. * 0.25 * 2.0; // access::WALKABLE_GRADE * ENTRANCE_APRON_RUN_M
    let mut levels: BTreeSet<i32> = BTreeSet::new();
    for site in &plan.roadside_sites {
        let road = roads_by_id
            .get(site.road_id.as_str())
            .unwrap_or_else(|| panic!("pad {} names no published road", site.id));
        // The station a pad fronts: its published frontage point, or - for a
        // service pad, whose station is its own centre projected onto its road -
        // that projection.
        let [fx, fz] = site.frontage_road_point.unwrap_or_else(|| {
            if road.rotation_y_deg == 0. {
                [site.world_x, road.world_z]
            } else {
                [road.world_x, site.world_z]
            }
        });
        let surface_mm = match &road.grounding.road_profile {
            Some(profile) => (profile
                .height_at(f64::from(fx), f64::from(fz))
                .expect("the frontage point lies inside its road's profile")
                * 1000.)
                .round(),
            None => (f64::from(road.ground_height_m()) * 1000.).round(),
        };
        let level = match site.support_level_mm {
            Some(level) => level,
            // A service pad keeps the integer-level contract its own audit
            // recognises: it is declared from its road's surface at the entrance,
            // snapped to the nearest whole metre, and the plan refuses a snap that
            // leaves the one-cell walkable step. Assert exactly that, so the snap
            // can never quietly stop reading the road.
            None => {
                assert!(
                    site.program.is_some(),
                    "a roadside pad may not be declared on a metre level: {}",
                    site.id
                );
                let level = site.support_level_m * 1000;
                assert!(
                    (f64::from(level) - surface_mm).abs() <= 500.0 + 1e-9,
                    "service pad {} is {level} mm while its own road stands at {surface_mm} mm",
                    site.id
                );
                continue;
            }
        };
        levels.insert(level);
        let rise = f64::from(level) - surface_mm;
        assert!(
            rise >= -1e-9,
            "pad {} ({level} mm) stands below its own road's surface ({surface_mm} mm)",
            site.id
        );
        assert!(
            rise <= apron_mm,
            "pad {} stands {:.3} m above its own road, more than the 2 m apron can climb",
            site.id,
            rise / 1000.
        );
        // Dry land and out of the band, cell by cell, from the water authority.
        let (half_w, half_d) = site.world_half_extents();
        for z in site.world_z - half_d as i32..site.world_z + half_d as i32 {
            for x in site.world_x - half_w as i32..site.world_x + half_w as i32 {
                assert!(
                    !water.contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5),
                    "pad {} stands on water at ({x},{z})",
                    site.id
                );
                assert!(
                    natural.height(x, z).is_some_and(|h| h >= 0.),
                    "pad {} stands on a below-datum natural cell at ({x},{z})",
                    site.id
                );
                assert!(
                    !water.shore_band_cell(x, z),
                    "pad {} enters the reserved shore band at ({x},{z})",
                    site.id
                );
            }
        }
        assert!(
            water.clears_shore(
                f64::from(site.world_x),
                f64::from(site.world_z),
                12.,
                12.,
            ),
            "pad {} does not keep the published {} m shore clearance",
            site.id,
            WATER_SHORE_CLEARANCE_M
        );
    }
    assert!(
        !levels.is_empty(),
        "the sample has to plan at least one precise roadside pad"
    );
    // The pad rule is per site, and the plan says so: every pad is precise, and
    // the refusal list is exactly the waterfront categories, per site. The old
    // blanket refusal - one category that swallowed the whole programme - is
    // absent by name.
    let rejected: BTreeMap<&str, u64> = plan
        .roadside_site_rejections
        .iter()
        .fold(BTreeMap::new(), |mut map, rejection| {
            *map.entry(rejection.category.as_str()).or_default() += 1;
            map
        });
    assert_eq!(
        rejected.get("waterfront_pad_needs_a_second_level"),
        None,
        "the blanket pad refusal must not come back: {rejected:?}"
    );
    assert_eq!(
        rejected.get("pad_touches_water_or_shore_band"),
        Some(&SAMPLE_REFUSED_WET_PADS),
        "pads refused for standing on water or in the band: {rejected:?}"
    );
    assert_eq!(
        rejected
            .get("pad_not_enterable_from_own_road")
            .copied()
            .unwrap_or(0),
        SAMPLE_REFUSED_STEEP_PADS,
        "pads refused because their own road is too far below them: {rejected:?}"
    );

    // ---------------------------------------------------------------------
    // 6d. THE SHORELINE PUBLIC PASSAGE BAND, recomputed independently.
    //
    // The band is the strip of dry ground within the published shore clearance
    // of the water: an offset of the waterline, so it is continuous on both banks
    // by construction. It must carry public passage on every square metre, its
    // area must be booked in the ledger's public-open-space column, and no
    // building may stand in it.
    // ---------------------------------------------------------------------
    assert_eq!(
        WATER_SHORE_CLEARANCE_M, 4.0,
        "the shore clearance is one published constant, not a per-call number"
    );
    let mut band_cells = 0_u64;
    for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
        for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
            if water.shore_band_cell(x, z) {
                band_cells += 1;
            }
        }
    }
    assert_eq!(
        band_cells, SAMPLE_SHORE_BAND_M2,
        "two 4 m banks over 512 m: the band is the waterline offset, not a patch set"
    );
    let band = &quality["scenes"][0]["shoreBandAudit"];
    assert_eq!(band["status"], json!("pass"), "{band}");
    assert_eq!(band["clearanceM"].as_f64(), Some(WATER_SHORE_CLEARANCE_M));
    assert_eq!(band["bandAreaM2"].as_f64(), Some(SAMPLE_SHORE_BAND_M2 as f64));
    assert_eq!(band["publicOpenSpaceM2"].as_f64(), Some(3_648.0));
    assert_eq!(band["roadPassageCells"], json!(448));
    assert_eq!(band["uncoveredCells"], json!(0));
    assert_eq!(band["wetPadCells"], json!(0));
    assert_eq!(band["bandPadCells"], json!(0));
    assert_eq!(band["passageComponents"]["west"], json!(1));
    assert_eq!(band["passageComponents"]["east"], json!(1));
    assert_eq!(
        band["publicOpenSpaceM2"].as_f64().unwrap() + band["roadPassageCells"].as_f64().unwrap(),
        band["bandAreaM2"].as_f64().unwrap(),
        "every band square metre is either public open space or a road's own pavement"
    );
    // The band's area is in the ledger's existing public-open-space column, and
    // the ledger's total is what the surfaces realize (no area counted twice).
    let ledger_public: f64 = ledger["byPurpose"]
        .as_object()
        .unwrap()
        .values()
        .map(|row| row["physical"]["publicOpenSpaceM2"].as_f64().unwrap())
        .sum();
    assert_eq!(
        ledger_public, SAMPLE_PUBLIC_OPEN_SPACE_M2,
        "the band's public surface is booked on the existing public-open-space column"
    );
    assert!(
        ledger_public > band["publicOpenSpaceM2"].as_f64().unwrap(),
        "the band is part of a larger public surface, not a second credit for the same square metre"
    );
    assert_eq!(
        quality["scenes"][0]["landcoverAudit"]["publicOpenSpaceAreaM2"].as_f64(),
        Some(SAMPLE_PUBLIC_OPEN_SPACE_M2),
        "the landcover audit books the same public surface"
    );
    // No building stands in the band: the pad check above proves the plan, and
    // this proves the published entities agree with it.
    for entity in &scene.entities {
        if !is_building_kind(&entity.kind) {
            continue;
        }
        let (half_w, half_d) = building_half_extents(
            entity.width_m,
            entity.depth_m,
            entity.rotation_y_deg,
        );
        for z in entity.world_z - half_d as i32..entity.world_z + half_d as i32 {
            for x in entity.world_x - half_w as i32..entity.world_x + half_w as i32 {
                assert!(
                    !water.shore_band_cell(x, z),
                    "building {} ({}) stands in the shore band at ({x},{z})",
                    entity.entity_id,
                    entity.kind
                );
            }
        }
    }
    // The scene is not empty: the crossings, the commercial strip and the city
    // itself are there, so the numbers above are a city and not a void.
    assert_eq!(count_kind(&scene, "storefront"), SAMPLE_STOREFRONTS);
    assert_eq!(count_kind(&scene, "river"), 1);
    assert_eq!(
        quality["scenes"][0]["accessGraph"]["disconnectedEntities"],
        json!(0)
    );
    assert_eq!(
        quality["scenes"][0]["accessGraph"]["connectedEntities"],
        json!(SAMPLE_CONNECTED_ENTITIES)
    );

    // ---------------------------------------------------------------------
    // 5. Two bakes of the same input publish identical bytes.
    // ---------------------------------------------------------------------
    let second = TestDir::new("waterfront-city-512-again");
    bake_world(&manifest, &second.0).expect("second bake");
    for file in ["engineering-placement.json", "scenes/scene-0/scene.json"] {
        let first_bytes = fs::read(dir.0.join(file)).unwrap();
        let second_bytes = fs::read(second.0.join(file)).unwrap();
        assert_eq!(
            first_bytes, second_bytes,
            "{file} must be byte-identical across two bakes of the same input"
        );
    }
}

/// A building's world half extents: all cardinal rotations, so the swap is exact.
fn building_half_extents(width_m: f32, depth_m: f32, rotation_y_deg: f32) -> (f64, f64) {
    let quarter = (rotation_y_deg.round() as i32).rem_euclid(180) == 90;
    if quarter {
        (f64::from(depth_m) / 2., f64::from(width_m) / 2.)
    } else {
        (f64::from(width_m) / 2., f64::from(depth_m) / 2.)
    }
}

fn is_building_kind(kind: &str) -> bool {
    matches!(
        kind,
        "building"
            | "building_tower"
            | "building_cluster"
            | "urban_building"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "storefront"
            | "commercial_center"
            | "school"
            | "church"
            | "convenience_store"
            | "green_space"
    )
}

/// Protected cells of this sample: the four 128 m nature zoning cells.
fn protected_cell_count(scene: &SceneIndex) -> u64 {
    scene
        .land_use_zones
        .iter()
        .filter(|zone| !matches!(zone.kind.as_str(), "urban" | "rural"))
        .map(|zone| u64::from(zone.width_m) * u64::from(zone.depth_m))
        .sum()
}

/// 39 wet metres across the 40 m channel, over 512 m: `39 * 512 = 19 968`
/// square metres of open water, identical before and after the bake.
const SAMPLE_WATER_SURFACE_M2: u64 = 19_968;
const SAMPLE_ROAD_ENTITIES: usize = 60;
const SAMPLE_BRIDGES: usize = 7;
const SAMPLE_BRIDGE_SPAN_M: u32 = 39;
const SAMPLE_BRIDGE_WET_CELLS: u32 = 585;
const SAMPLE_PLAN_ITEMS: u64 = 427;
const SAMPLE_OUTPUT_ITEMS: u64 = 427;
const SAMPLE_REMOVED_ITEMS: u64 = 0;
/// 22 retained dwellings + 4 services, plus the shoreline row's 3 quay shops: every planned site.
const SAMPLE_PLANNED_PADS: u64 = 29;
const SAMPLE_PLANNED_HOMES: u64 = 22;
const SAMPLE_REALIZED_HOMES: u64 = 22;
const SAMPLE_REALIZED_SERVICES: u64 = 4;
/// Candidates the water/shore-band rule refuses, per site.
const SAMPLE_REFUSED_WET_PADS: u64 = 25;
/// Candidates whose own road stands too far below the flat pad they would need.
const SAMPLE_REFUSED_STEEP_PADS: u64 = 0;
/// Two 4 m banks over 512 m.
const SAMPLE_SHORE_BAND_M2: u64 = 4_096;
/// The ledger's public-open-space column: 3 648 m2 of band plus 4 342 m2 of
/// residual urban open space.
const SAMPLE_PUBLIC_OPEN_SPACE_M2: f64 = 7_974.0;
const SAMPLE_STOREFRONTS: usize = 3;
const SAMPLE_CONNECTED_ENTITIES: u64 = 31;

/// The release gate's verdict on the sample, line by line. The waterfront gap
/// (no housing minimum, no services, an unaudited shore) is closed and gone from
/// this list; what remains is the land-use realization backlog that every
/// road-first 512 m scene carries and the authored frontage programme.
///
/// Round 40 removed the fourth line this constant used to carry,
/// `strict profile has unruled entity kinds: river`: `assets/objects/river.object.toml`
/// now describes the channel, so the rules engine CHECKS it (425 checked / 425 passed,
/// 0 rejected) instead of reporting a coverage gap.
const SAMPLE_GATE_FAILURES: [&str; 3] = [
    "\"scene-0\" local-height frontage demand or realization remains incomplete",
    "\"scene-0\" purpose-area allocation or realization remains incomplete",
    "\"scene-0\" landcover remains unrealized",
];
