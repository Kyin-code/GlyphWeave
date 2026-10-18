//! Round 30: an explicit rural-to-urban conversion is capped by the world's urban purpose
//! allocation, at the placement stage, instead of being discovered by the land-use ledger.
//!
//! The round 29 probe measured the defect this file locks: a 1 km world converted 48,548 m2 of
//! farmland into "urban" purpose on top of 9,755 m2 of urban physical cover, against a plan that
//! allocated 40,898 m2 of urban land. Every conversion was individually legal - each parcel's
//! buildable core lies wholly in rural land - and jointly impossible, so the ledger refused the
//! whole scene with `ledger purpose overcredited`.
//!
//! The fixture below is the same defect at 512 m: the authored waterfront sample with the explicit
//! conversion switched on, its urban profile squeezed to 5 % and a demand-led frontage programme far
//! larger than that allocation. What the capped planner must produce instead:
//!
//!  * the world bakes (the ledger closes: `urban physical + conversions <= planned urban`),
//!  * the overflow is refused BY NAME, with the area it needed and the area that was left,
//!  * every refused pad is withdrawn from the plan, from the conversions and from the world,
//!  * the published allocation is exactly the sum of the conversions the plan kept, and
//!  * two bakes of the same input publish the same bytes.
use glyphweave_core::settlement::landuse_ledger::LandUseLedger;
use glyphweave_core::worldgen::{
    SceneIndex, WorldManifest, bake_world,
    conversion_budget::{self, CONVERSION_EXCEEDS_URBAN_ALLOCATION},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the fixture cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");

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

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The authored waterfront sample with a 5 % urban profile and a frontage programme whose
/// conversions (up to 80 lots) can never fit that allocation.
fn oversized() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["landUseProfile"] = json!({
        "theme": "river-delta",
        "urbanCoreRatio": 0.05,
        "suburbanRatio": 0.0,
        "greenRatio": 0.05,
        "farmRatio": 0.6,
        "forestRatio": 0.1,
        "pastureRatio": 0.1,
        "reserveRatio": 0.1
    });
    manifest.style["frontageProgram"]["ruralConversion"] = json!("explicit");
    manifest.style["frontageProgram"]["requests"] = json!([
        {
            "id": "bank-homes", "sceneId": "scene-0", "purpose": "urban",
            "kind": "residential_home", "count": 60, "widthM": 24, "depthM": 24, "heightM": 8,
            "stationSpacingM": 16, "setbackM": 8, "maxCandidateAttempts": 8192
        },
        {
            "id": "bank-shops", "sceneId": "scene-0", "purpose": "urban",
            "kind": "storefront", "count": 20, "widthM": 12, "depthM": 10, "heightM": 5,
            "stationSpacingM": 16, "setbackM": 8, "maxCandidateAttempts": 8192
        }
    ]);
    manifest
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap()
}

/// blake3 of every regular file in a published world, keyed by its relative path.
fn tree(root: &PathBuf) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string());
            }
        }
    }
    out
}

#[test]
fn the_oversubscribed_conversion_world_spends_its_allocation_and_closes_its_ledger() {
    let dir = TestDir::new("conversion-allocation");
    let manifest = oversized();
    manifest.validate().expect("the over-subscribed sample validates");
    assert_eq!(
        manifest.style["frontageProgram"]["ruralConversion"],
        json!("explicit")
    );
    // The defect this replaces: the same input against an UNCAPPED planner converts more rural land
    // than the world plans as urban, and the ledger refuses the whole scene. It bakes here.
    bake_world(&manifest, &dir.0).expect("a capped world must bake instead of overcrediting");
    let scene = scene_index(&dir);
    let plan = scene
        .road_network_plan
        .as_ref()
        .expect("the switch authorizes the road network");
    let zones = &scene.land_use_zones;

    // ---- the allocation is the plan's own geometry, re-derived from the published bytes ----------
    let planned = conversion_budget::planned_urban_m2(zones);
    let committed = conversion_budget::committed_urban_m2(plan, zones);
    let allocation = plan
        .conversion_allocation
        .clone()
        .expect("the cap refused conversions, so the plan publishes the allocation it spent against");
    assert_eq!(allocation.planned_urban_m2, planned);
    assert_eq!(allocation.committed_urban_m2, committed);
    assert!(planned > 0);
    assert!(committed > 0, "roads and parcels stand in the urban zone");
    // The conversions the plan KEPT are inside what the allocation granted, and both fit it.
    let kept = conversion_budget::plan_conversion_records(plan);
    let kept_m2: i64 = kept.iter().map(|record| record.area_m2).sum();
    assert!(
        kept_m2 <= allocation.converted_m2,
        "the plan kept {kept_m2} m2 of conversions but granted only {}",
        allocation.converted_m2
    );
    assert!(
        allocation.converted_m2 <= allocation.allowance_m2(),
        "the allocation granted {} m2 out of an allowance of {} m2",
        allocation.converted_m2,
        allocation.allowance_m2()
    );
    assert!(
        kept_m2 <= allocation.allowance_m2(),
        "{kept_m2} m2 of conversions must fit an allowance of {} m2",
        allocation.allowance_m2()
    );
    // The audit re-derives the same verdict from the plan and the zoning alone.
    assert_eq!(
        conversion_budget::allocation_failures(plan, zones, kept_m2),
        Vec::<String>::new()
    );

    // ---- the overflow was refused BY NAME, with both areas --------------------------------
    assert!(
        !allocation.refusals.is_empty(),
        "a programme this size against a 5 % urban profile must overflow"
    );
    for refusal in &allocation.refusals {
        assert!(
            refusal.required_m2 > refusal.remaining_m2,
            "{} asked for {} m2 with {} m2 left",
            refusal.parcel_id,
            refusal.required_m2,
            refusal.remaining_m2
        );
        assert!(
            !kept.iter().any(|record| record.parcel_id == refusal.parcel_id),
            "a refused conversion may not also be recorded"
        );
    }
    let refused_pads: Vec<&str> = plan
        .roadside_site_rejections
        .iter()
        .filter(|rejection| rejection.category == CONVERSION_EXCEEDS_URBAN_ALLOCATION)
        .map(|rejection| rejection.edge_id.as_str())
        .collect();
    // Every pad refused for the allocation is gone from the plan, from the conversions and from the
    // published world - the triangle the round 28 machinery established, now for this refusal too.
    for id in &refused_pads {
        assert!(
            !plan.roadside_sites.iter().any(|site| site.id == *id),
            "pad {id} is refused by the allocation and still planned"
        );
        assert!(
            !plan.pad_conversions.iter().any(|record| record.parcel_id == *id),
            "pad {id} is refused by the allocation and still converted"
        );
        assert!(
            !scene.entities.iter().any(|entity| entity.entity_id == *id),
            "pad {id} is refused by the allocation and still published"
        );
    }

    // ---- the land-use ledger closes ---------------------------------------------------------
    let ledger: LandUseLedger = serde_json::from_slice(
        &fs::read(dir.0.join("scenes/scene-0/land-use-ledger.json")).unwrap(),
    )
    .expect("the ledger is published");
    let urban = &ledger.by_purpose["urban"];
    let rural = &ledger.by_purpose["rural"];
    assert_eq!(urban.planned_m2, planned as f64);
    assert!(
        urban.realized_m2 <= urban.planned_m2 + 1e-6,
        "the urban row is overcredited: {} > {}",
        urban.realized_m2,
        urban.planned_m2
    );
    assert!(
        (urban.realized_m2 - (urban.physical.total() + kept_m2 as f64)).abs() < 1e-6,
        "the urban row must be exactly physical cover plus the conversions the plan kept"
    );
    assert!(
        (rural.realized_m2 - (rural.physical.total() - kept_m2 as f64)).abs() < 1e-6,
        "the rural row must be exactly physical cover minus the same conversions"
    );
    assert!(
        (ledger.realized_m2 + ledger.unresolved_m2 - ledger.scene_area_m2).abs() < 1e-6,
        "the ledger must still conserve the scene area"
    );
    assert_eq!(
        ledger.conversions, kept,
        "the ledger applies exactly the conversions the plan records"
    );
    // The physical cover the ledger books is inside the commitment the allocation claimed for it.
    assert!(
        urban.physical.total() <= (allocation.committed_urban_m2 + allocation.residual_landcover_m2) as f64 + 1e-6,
        "the allocation's urban commitment must bound the realized urban cover"
    );
}

#[test]
fn two_bakes_of_the_same_world_publish_identical_bytes() {
    let first = TestDir::new("conversion-allocation-repeat-a");
    let second = TestDir::new("conversion-allocation-repeat-b");
    let manifest = oversized();
    bake_world(&manifest, &first.0).unwrap();
    bake_world(&manifest, &second.0).unwrap();
    let a = tree(&first.0);
    let b = tree(&second.0);
    assert!(!a.is_empty(), "a bake publishes files");
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "two bakes of one input must publish the same file list"
    );
    for (path, digest) in &a {
        assert_eq!(digest, &b[path], "{path} differs between two bakes");
    }
    // And the allocation's own record is one of the frozen bytes, not a diagnostic that drifts.
    let scene = scene_index(&first);
    let recorded = scene
        .road_network_plan
        .as_ref()
        .and_then(|plan| plan.conversion_allocation.clone())
        .expect("the allocation record is published");
    let repeated = read_json(first.0.join("scenes/scene-0/road-network-plan.json"));
    let from_json: Value = serde_json::to_value(&recorded).unwrap();
    assert_eq!(repeated["conversionAllocation"], from_json);
}
