//! Round 23: the registered waterfront sample WITH the explicit along-road purpose
//! conversion (`style.frontageProgram.ruralConversion = "explicit"`) bakes end to
//! end, and the pad that used to be deleted from the world is REALIZED.
//!
//! What the defect was, and what this test locks:
//!
//!  * Round 23 measured 28 planned pads, 27 realized: the church pad was planned,
//!    declared `urban` and converted 468 m2, and then its entity was removed from
//!    the world by `append_modern_landuse_content`'s `retain`, which asked
//!    `pad_fill_conflicts` whether the pad's own blend ring reached a neighbour's
//!    declared top. It did - by the rule's own arithmetic - so the pad was dropped
//!    without a record. The plan kept it, the ledger kept booking its 468 m2, and
//!    nothing in the world carried it: the road audit failed 27 of 28 pads and
//!    dragged the service and joint audits down with it.
//!  * The rule was wrong, not the pad: a neighbouring blended fill cannot overwrite
//!    a validated hard support surface whose road band reaches the cell
//!    (`apply_terrain_carves`), which is what the encoder actually produces. The
//!    independently recomputed audit proves it here: 28 of 28 pads hold their
//!    declared tops with a design error of 0 m.
//!  * So the pad is not refused; it is REALIZED, and the three views agree: 28
//!    planned pads, 28 published pad entities, every conversion record belongs to a
//!    pad that exists, and the ledger's 4 692 m2 of pad conversion is booked against
//!    standing buildings.
//!  * The refusal machinery still exists for pads that genuinely cannot hold their
//!    declared top (`pad_fill_conflicts_with_neighbour` / `pad_flat_top_unrealizable`,
//!    locked by `road_network::pad_refusal_tests`), and it can never leave the plan,
//!    the ledger and the world disagreeing again.

use glyphweave_core::worldgen::{SceneIndex, WorldManifest, bake_world, verify_published_placement};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::PathBuf, process::Command};

/// The published sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");
const SEED: u64 = 7;
const SCENE_SIZE_M: u32 = 512;

struct TestDir(PathBuf);
impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glyphweave-{label}-{}-{nonce}",
            std::process::id()
        ));
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

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The registered sample with the one authored switch this round is about.
fn sample() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["frontageProgram"]["ruralConversion"] = json!("explicit");
    manifest
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap()
}

fn count_kind(scene: &SceneIndex, kind: &str) -> usize {
    scene
        .entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .count()
}

fn quality_gate(dir: &TestDir) -> Value {
    Command::new(glyphweave())
        .arg("world-quality-gate")
        .arg(&dir.0)
        .output()
        .unwrap();
    read_json(dir.0.join("world-quality-gate.json"))
}

#[test]
fn the_explicit_conversion_sample_realizes_its_church_and_every_pad_agrees_with_the_ledger() {
    let dir = TestDir::new("waterfront-conversion");
    let manifest = sample();
    manifest.validate().expect("the switched sample validates");
    assert_eq!(
        manifest.style["frontageProgram"]["ruralConversion"],
        json!("explicit"),
        "only the exact authored value enables the conversion"
    );
    assert_eq!(manifest.scenes[0].width_m, SCENE_SIZE_M);
    assert_eq!(manifest.world.seed, SEED);

    bake_world(&manifest, &dir.0).expect("the switched waterfront sample must bake end to end");
    let scene = scene_index(&dir);
    let plan = scene
        .road_network_plan
        .as_ref()
        .expect("the opt-in authorizes the road network");

    // ---------------------------------------------------------------------
    // 1. THE CHURCH IS BACK. 28 pads planned, 28 realized, and the far end of
    //    the pipeline (the independent road audit) says every declared top is
    //    held with a design error of 0 m.
    // ---------------------------------------------------------------------
    assert_eq!(
        plan.roadside_sites.len(),
        28,
        "23 retained dwellings + 4 services + the one converted frontage shop"
    );
    let rejections: BTreeMap<&str, u64> = plan
        .roadside_site_rejections
        .iter()
        .fold(BTreeMap::new(), |mut map, rejection| {
            *map.entry(rejection.category.as_str()).or_default() += 1;
            map
        });
    assert_eq!(
        rejections.get("pad_fill_conflicts_with_neighbour"),
        None,
        "the church's fill ring does not bury the shop: the bake pins the shop's validated \
         top against a neighbouring blend, so no pad has to be refused here: {rejections:?}"
    );
    assert_eq!(rejections.get("pad_flat_top_unrealizable"), None, "{rejections:?}");
    assert_eq!(count_kind(&scene, "church"), 1, "the church is a building in the world");

    // ---------------------------------------------------------------------
    // 2. THE TRIANGLE: plan == entities == ledger. Every planned pad id is
    //    published exactly once, and no pad is on the ledger without a plan.
    // ---------------------------------------------------------------------
    let mut published: BTreeMap<&str, usize> = BTreeMap::new();
    for entity in &scene.entities {
        *published.entry(entity.entity_id.as_str()).or_default() += 1;
    }
    let mut missing = Vec::new();
    for site in &plan.roadside_sites {
        match published.get(site.id.as_str()) {
            Some(1) => {}
            Some(count) => panic!("pad {} is published {count} times", site.id),
            None => missing.push(site.id.clone()),
        }
    }
    assert!(
        missing.is_empty(),
        "every planned pad must be published; these are planned and absent: {missing:?}"
    );
    assert_eq!(
        plan.roadside_sites.len(),
        plan.site_entities().len(),
        "the plan's own entity list is exactly its pad list"
    );
    // THE IDENTITY the defect broke, in both directions: the pads the world
    // carries plus the pads the plan refused by name account for the planner's pad
    // list, and no pad is a refusal and a published entity at the same time.
    let refused: Vec<&str> = plan
        .roadside_site_rejections
        .iter()
        .filter(|rejection| {
            glyphweave_core::worldgen::road_network::PAD_REFUSED_CATEGORIES
                .contains(&rejection.category.as_str())
        })
        .map(|rejection| rejection.edge_id.as_str())
        .collect();
    let realized_pads = plan
        .roadside_sites
        .iter()
        .filter(|site| published.get(site.id.as_str()) == Some(&1))
        .count();
    assert_eq!(
        realized_pads + refused.len(),
        plan.roadside_sites.len(),
        "pad count = published entities + named refusals ({realized_pads} + {} != {})",
        refused.len(),
        plan.roadside_sites.len()
    );
    for refusal in &refused {
        assert!(
            !published.contains_key(refusal),
            "a refused pad may not be published: {refusal}"
        );
    }

    // ---------------------------------------------------------------------
    // 3. THE LEDGER BOOKS LAND FOR PADS THAT EXIST. 11 pad conversions
    //    (5 080 m2) plus the frontage parcel's 80 m2 = 12 records, 5 160 m2,
    //    and the church's own 468 m2 is one of them.
    // ---------------------------------------------------------------------
    let pad_converted: i64 = plan.pad_conversions.iter().map(|record| record.area_m2).sum();
    assert_eq!(plan.pad_conversions.len(), 11);
    assert_eq!(pad_converted, 5_080);
    let church = plan
        .pad_conversions
        .iter()
        .find(|record| record.parcel_id == "generated.service.scene-0.bank-church.0")
        .expect("the church pad declared urban purpose on rural land and converted its core");
    assert_eq!((church.from.as_str(), church.to.as_str()), ("rural", "urban"));
    assert_eq!(church.area_m2, 468);
    for record in &plan.pad_conversions {
        assert!(
            plan.roadside_sites.iter().any(|site| site.id == record.parcel_id),
            "conversion record {} belongs to no planned pad",
            record.parcel_id
        );
    }
    let ledger = read_json(dir.0.join("scenes/scene-0/land-use-ledger.json"));
    let ledger_records = ledger["conversions"].as_array().expect("the ledger applied them");
    assert_eq!(ledger_records.len(), 12);
    let ledger_converted: i64 = ledger_records
        .iter()
        .map(|record| record["areaM2"].as_i64().unwrap())
        .sum();
    assert_eq!(ledger_converted, 5_160);
    assert!(
        ledger_records
            .iter()
            .any(|record| record["parcelId"] == json!("generated.service.scene-0.bank-church.0")
                && record["areaM2"] == json!(468)),
        "the ledger must book the church's 468 m2 - and the church exists, so the booking is honest"
    );

    // ---------------------------------------------------------------------
    // 4. The release gate: the switched sample carries the SAME four pre-existing
    //    failures as the unswitched one - no road-network, service or joint
    //    failure, which is exactly what the deleted pad used to cause.
    // ---------------------------------------------------------------------
    let gate = quality_gate(&dir);
    assert_eq!(gate["status"], json!("fail"));
    let failures: Vec<&str> = gate["failures"]
        .as_array()
        .expect("the gate published its failure list")
        .iter()
        .map(|failure| failure.as_str().unwrap())
        .collect();
    assert_eq!(
        failures,
        [
            "\"scene-0\" local-height frontage demand or realization remains incomplete",
            "\"scene-0\" purpose-area allocation or realization remains incomplete",
            "\"scene-0\" landcover remains unrealized",
            "strict profile has unruled entity kinds: river",
        ],
        "the conversion switch may not add a network, service or joint failure"
    );
    let quality = read_json(dir.0.join("quality-report.json"));
    let row = &quality["scenes"][0];

    // 4a. The road audit holds every pad's declared top.
    assert_eq!(row["roadNetworkAudit"]["status"], json!("pass"));
    assert_eq!(row["roadNetworkAudit"]["plannedEdges"], json!(60));
    assert_eq!(row["roadNetworkAudit"]["realizedEdges"], json!(60));
    assert_eq!(row["roadNetworkAudit"]["plannedSites"], json!(28));
    assert_eq!(row["roadNetworkAudit"]["realizedSites"], json!(28));
    assert_eq!(row["roadNetworkAudit"]["failures"], json!([]));
    assert_eq!(
        row["roadNetworkAudit"]["maxSiteDesignHeightErrorM"].as_f64(),
        Some(0.0),
        "every pad's declared top is held with no design error at all"
    );

    // 4b. All four requested services stand with verified access.
    assert_eq!(row["serviceProgramAudit"]["status"], json!("pass"));
    assert_eq!(row["serviceProgramAudit"]["requested"], json!(4));
    assert_eq!(row["serviceProgramAudit"]["planned"], json!(4));
    assert_eq!(row["serviceProgramAudit"]["realizedWithAccess"], json!(4));
    assert_eq!(row["serviceProgramAudit"]["unfilled"], json!(0));
    assert_eq!(count_kind(&scene, "school"), 1);
    assert_eq!(count_kind(&scene, "convenience_store"), 2);

    // 4c. The joint layout keeps its housing minimum and reports no unfilled service.
    assert_eq!(row["jointLayoutAudit"]["status"], json!("pass"));
    assert_eq!(row["jointLayoutAudit"]["residentialMinimum"], json!(12));
    assert_eq!(row["jointLayoutAudit"]["plannedResidential"], json!(23));
    assert_eq!(row["jointLayoutAudit"]["realizedResidentialWithAccess"], json!(23));
    assert_eq!(row["jointLayoutAudit"]["missingResidential"], json!(0));
    assert_eq!(row["jointLayoutAudit"]["unfilledServices"], json!(0));
    assert_eq!(count_kind(&scene, "residential_home"), 23);

    // 4d. The conversion audit passes: the plan's 12 records ARE the ledger's, the
    //     refused-pad count is 0, and no record is booked for a pad that is not planned.
    assert_eq!(row["purposeConversionAudit"]["status"], json!("pass"));
    assert_eq!(row["purposeConversionAudit"]["planned"], json!(12));
    assert_eq!(row["purposeConversionAudit"]["applied"], json!(12));
    assert_eq!(row["purposeConversionAudit"]["convertedM2"], json!(5_160));
    assert_eq!(row["purposeConversionAudit"]["refusedPads"], json!(0));
    assert_eq!(row["purposeConversionAudit"]["failures"], json!([]));

    // 4e. The ledger closes per purpose and for the scene, with the conversion area
    //     on the urban side: 34 306 m2 realized urban, 18 866 m2 realized rural.
    let ledger_row = &row["landUseLedger"];
    let mut planned_total = 0.0;
    for (purpose, account) in ledger_row["byPurpose"].as_object().unwrap() {
        let planned = account["plannedM2"].as_f64().unwrap();
        let realized = account["realizedM2"].as_f64().unwrap();
        let unresolved = account["unresolvedM2"].as_f64().unwrap();
        assert_eq!(
            realized + unresolved,
            planned,
            "{purpose}: realized {realized} + unresolved {unresolved} != planned {planned}"
        );
        planned_total += planned;
    }
    assert_eq!(
        planned_total,
        ledger_row["sceneAreaM2"].as_f64().unwrap(),
        "the purposes cover the scene"
    );
    let urban = &ledger_row["byPurpose"]["urban"];
    assert_eq!(urban["realizedM2"].as_f64(), Some(34_306.0));
    let rural = &ledger_row["byPurpose"]["rural"];
    assert_eq!(rural["realizedM2"].as_f64(), Some(18_866.0));

    // ---------------------------------------------------------------------
    // 5. The reconciliation publishes the pad triangle. Every planned pad is
    //    accounted for, nothing was refused, nothing was removed, and the
    //    independent recomputation agrees.
    // ---------------------------------------------------------------------
    let placement = read_json(dir.0.join("engineering-placement.json"));
    let entry = &placement["scenes"][0];
    assert_eq!(entry["inputItems"], json!(436));
    assert_eq!(entry["outputItems"], json!(436));
    assert_eq!(entry["removedItems"], json!(0));
    assert_eq!(entry["skips"], json!([]));
    assert_eq!(
        entry["plannedPadIds"].as_array().unwrap().len(),
        28,
        "the report publishes the plan's pad list so the triangle is recomputable"
    );
    assert!(
        entry.get("refusedEntityIds").is_none(),
        "no pad was refused on this sample"
    );
    let published_ids: BTreeSet<String> =
        scene.entities.iter().map(|entity| entity.entity_id.clone()).collect();
    let authored: BTreeSet<String> = scene
        .landmarks
        .iter()
        .map(|landmark| landmark.entity_id.clone())
        .collect();
    let audit = verify_published_placement(entry, &published_ids, &authored, None)
        .expect("the published pad triangle must recompute");
    assert_eq!(audit.plan_items, 436);
    assert_eq!(audit.output_items, 436);
    assert_eq!(audit.skipped_items, 0);

    // ---------------------------------------------------------------------
    // 6. Two bakes of the same input publish identical bytes.
    // ---------------------------------------------------------------------
    let second = TestDir::new("waterfront-conversion-again");
    bake_world(&manifest, &second.0).expect("second bake");
    for file in ["engineering-placement.json", "scenes/scene-0/scene.json"] {
        assert_eq!(
            fs::read(dir.0.join(file)).unwrap(),
            fs::read(second.0.join(file)).unwrap(),
            "{file} must be byte-identical across two bakes of the same input"
        );
    }
}
