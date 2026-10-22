//! Round 30: an explicit rural-to-urban conversion is capped by the world's urban purpose
//! allocation, at the placement stage, instead of being discovered by the land-use ledger.
//!
//! The round 29 probe measured the defect this file locks: a 1 km world converted 48,548 m2 of
//! farmland into "urban" purpose on top of 9,755 m2 of urban physical cover, against a plan that
//! allocated 40,898 m2 of urban land. Every conversion was individually legal - each parcel's
//! buildable core lies wholly in rural land - and jointly impossible, so the ledger refused the
//! whole scene with `ledger purpose overcredited`.
//!
//! The fixture is the same world at 512 m: the authored waterfront sample with the explicit
//! conversion switched on, its urban profile squeezed to 5 % and a demand-led frontage programme far
//! larger than that allocation.
//!
//! # What Round 31 changed here, and why the file is re-based rather than weakened
//!
//! Round 31 put a second bound in front of the same decision: a conversion may only take rural land
//! whose buildable core stands within `conversion_fringe::FRINGE_M` (64 m) of an existing urban zone.
//! On this fixture the urban edge turns out to be the TIGHTER bound - the 5 % profile plans one
//! 16,384 m2 urban block, the band around it holds 14 convertible pads (5,764 m2 of buildable core),
//! and the allocation still had 10,272 m2 of allowance when the programme ran out of candidates. The
//! planner therefore publishes no allocation record at all here any more, and asserting otherwise
//! would be asserting a scenario the world cannot produce.
//!
//! What is still asserted, end to end and by name:
//!
//!  * the world bakes, and the land-use ledger closes over the scene,
//!  * every conversion the plan keeps stands inside the urban edge (`conversion_fringe`), which is
//!    re-derived here from the published plan and zoning,
//!  * the purpose allocation is still a LIVE contract on THIS plan: the audit accepts exactly the
//!    area the plan kept and refuses one square metre more, by name, with both numbers,
//!  * the ledger's urban row is exactly physical cover plus the kept conversions and never exceeds
//!    the planned urban area,
//!  * two bakes of the same input publish the same bytes, including the published band record.
//!
//! The cap's own refusal path - a candidate that cannot afford the land it would convert, refused by
//! name with the area it needed and the area that was left - is asserted at the planner level in
//! `worldgen::frontage::tests::an_exhausted_urban_allocation_refuses_the_conversion_by_name_with_both_areas`
//! and `a_forged_conversion_above_the_urban_allocation_is_refused_by_the_audit`, where the fixture
//! can make the allowance bind without needing a fringe that does not exist.
use glyphweave_core::settlement::landuse_ledger::LandUseLedger;
use glyphweave_core::worldgen::{
    SceneIndex, WorldManifest, bake_world,
    conversion_budget::{self, CONVERSION_EXCEEDS_URBAN_ALLOCATION},
    conversion_fringe,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published sample, compiled in: the fixture cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");

/// What the capped planner keeps on this fixture: 14 convertible pads, all inside the urban edge.
const KEPT_RECORDS: usize = 14;
const KEPT_M2: i64 = 5_764;

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
                out.insert(
                    rel,
                    blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    out
}

#[test]
fn the_capped_conversion_world_spends_inside_its_allocation_and_inside_the_urban_edge() {
    let dir = TestDir::new("conversion-allocation");
    let manifest = oversized();
    manifest
        .validate()
        .expect("the over-subscribed sample validates");
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

    // ---- the conversions are inside the urban edge, re-derived from the published bytes -----------
    let planned = conversion_budget::planned_urban_m2(zones);
    let committed = conversion_budget::committed_urban_m2(plan, zones);
    assert!(planned > 0);
    assert!(committed > 0, "roads and parcels stand in the urban zone");
    let kept = conversion_budget::plan_conversion_records(plan);
    let kept_m2: i64 = kept.iter().map(|record| record.area_m2).sum();
    assert_eq!(kept.len(), KEPT_RECORDS, "{kept:?}");
    assert_eq!(kept_m2, KEPT_M2);
    // Every record's own geometry is re-verified against this zoning, and the whole list passes the
    // band audit: the urban purpose did not appear one square metre outside the city's edge.
    for record in &kept {
        assert_eq!(
            glyphweave_core::worldgen::frontage::verify_conversion(plan, zones, None, record)
                .expect("a kept record re-derives from the parcel's own geometry"),
            record.area_m2
        );
    }
    assert_eq!(
        conversion_fringe::audit_failures(plan, zones),
        Vec::<String>::new()
    );
    // The band refused far more than it allowed, and it says so with the distance it measured.
    let band = plan
        .conversion_fringe
        .clone()
        .expect("a plan the urban edge refused publishes the band it converted inside");
    assert_eq!(band.fringe_m, conversion_fringe::FRINGE_M);
    assert_eq!(band.fringe_m, 64);
    assert!(!band.refusals.is_empty());
    for refusal in &band.refusals {
        let nearest = refusal.nearest_m.expect("this world plans urban land");
        assert!(
            nearest > conversion_fringe::FRINGE_M,
            "request {} publishes a nearest refused candidate {nearest} m out, inside the band",
            refusal.request_id
        );
    }
    // The urban edge, not the allocation, is what bounded this world: with the conversions inside the
    // band there was still allowance left, so the plan publishes no allocation record at all.
    let allowance_left = planned - committed - kept_m2;
    assert!(
        allowance_left > 0,
        "the fixture must leave allowance unspent, or the argument here is empty: {planned} - \
         {committed} - {kept_m2}"
    );
    assert!(
        plan.conversion_allocation.is_none(),
        "a plan that never hit the cap publishes no allocation record"
    );

    // ---- the allocation is still a LIVE contract on THIS plan --------------------------------------
    // The audit's own bound, from the plan and the zoning alone: planned urban minus the urban land
    // the plan already committed to roads and parcels (no allocation record is published here, so the
    // residual-land share is not part of the claim). The area the plan kept fits it, and one square
    // metre more does not - which is the cap doing its job, asserted on the real plan.
    let bound = (planned - committed).max(0);
    assert!(
        kept_m2 <= bound,
        "{kept_m2} m2 of conversions vs a bound of {bound} m2"
    );
    assert_eq!(
        conversion_budget::allocation_failures(plan, zones, kept_m2),
        Vec::<String>::new(),
        "the area the plan kept fits what the zoning allows"
    );
    let forged = conversion_budget::allocation_failures(plan, zones, bound + 1);
    assert_eq!(forged.len(), 1, "{forged:?}");
    assert!(
        forged[0].contains(&format!("converts {} m2", bound + 1)),
        "the audit must name the area it cannot afford: {}",
        forged[0]
    );
    assert!(
        forged[0].contains(&format!("leaves only {bound} m2")),
        "the audit must publish the allowance it had: {}",
        forged[0]
    );

    // ---- the land-use ledger closes ---------------------------------------------------------------
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
    // No pad was refused for the allocation, and none for any other withdrawn-pad reason: on this
    // fixture the cap never had to withdraw a planned pad.
    let withdrawn: Vec<&str> = plan
        .roadside_site_rejections
        .iter()
        .filter(|rejection| rejection.category == CONVERSION_EXCEEDS_URBAN_ALLOCATION)
        .map(|rejection| rejection.edge_id.as_str())
        .collect();
    assert!(withdrawn.is_empty(), "{withdrawn:?}");
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
    // And the band's own record is one of the frozen bytes, not a diagnostic that drifts.
    let scene = scene_index(&first);
    let recorded = scene
        .road_network_plan
        .as_ref()
        .and_then(|plan| plan.conversion_fringe.clone())
        .expect("the urban edge refused candidates, so the band is published");
    let repeated = read_json(first.0.join("scenes/scene-0/road-network-plan.json"));
    let from_json: Value = serde_json::to_value(&recorded).unwrap();
    assert_eq!(repeated["conversionFringe"], from_json);
}
