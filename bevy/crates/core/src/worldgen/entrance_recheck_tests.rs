//! The entrance contract asked ONCE MORE, when the pad list is whole.
//!
//! Every pad generator refuses its own candidates on `assigned_road_gate`, but each of
//! them asks the question against the earthwork that exists at THAT moment, and the
//! pad list is not whole until the frontage programme has run. A lot accepted by an
//! earlier generator can therefore be buried by a later one: a fill-only pad's
//! declared flat top is the surface inside its own carve rectangles, and every cell
//! outside them is the MAXIMUM of the neighbours' fills. A neighbour that stands
//! higher - in the 1 km probe, the frontage pads planned up a road whose profile
//! climbs 11 m - raises a wall around the earlier lot's door.
//!
//! The fixture is that coupling in miniature: a lot on a flat street at the scene
//! datum, and a graded street one station to the north whose authored profile stands
//! `RIDGE_MM` above the natural ground. The graded street's own band is a hard
//! priority-10 rectangle, so its fill reaches the lot's door as a step of metres in
//! one cell - the same composition the bake publishes, not a special case.
//!
//! What is pinned here:
//!
//!  * the refusal is NAMED, published with the checked route's own status and the
//!    measured distance from the lot's door to the nearest street;
//!  * the lot leaves every record derived from the pad list (planned sites, joint
//!    receipt, conversion record, entity list) in ONE operation, and
//!    `refused_pad_ids` reports it, so the entity side cannot keep it;
//!  * no declaration inside the lot buys it back (a forged `supportLevelMm` and a
//!    forged `frontageRoadPoint` are inert - the gate reads the surface);
//!  * the control: the same lot, the same streets, with the ridge's profile on the
//!    natural ground instead of above it, keeps its walkable entrance and is NOT
//!    refused, so the refusal is the wall and not the rule;
//!  * a lot whose assigned street the completed plan does not publish is refused by
//!    name too - an unpublished street is never a dangling `roadId`;
//!  * the pass is deterministic: the same plan twice produces the same refusals.
use super::*;
use crate::baked_terrain::BakedTerrain;
use crate::worldgen::frontage::Conversion;
use crate::worldgen::joint_frontage::JointLayout;
use crate::worldgen::road_network::{PAD_ENTRANCE_LOST_AFTER_PAD_LIST, RoadNetworkPlan};
use serde_json::json;
use std::collections::BTreeMap;

const LOT: &str = "lot";
/// The graded street's published surface, above the natural ground.
const RIDGE_MM: i32 = 2_000;

fn scene() -> SceneSpec {
    SceneSpec {
        scene_id: "entrance-recheck".into(),
        width_m: 256,
        depth_m: 256,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    }
}

/// Flat ground one metre below the scene datum: the lot is a 1 m fill, exactly like
/// the 1 km probe's recovery home, so its own ring is a fill too.
fn natural() -> BakedTerrain {
    let scene = scene();
    let mut terrain = BakedTerrain::new(&scene).unwrap();
    let values = vec![-1.0_f64; 256 * 256];
    terrain
        .insert_chunk(
            0,
            0,
            256,
            256,
            &crate::height_payload::encode_millimetres(256, 256, values).unwrap(),
        )
        .unwrap();
    terrain
}

fn water() -> WaterGeometry {
    water_geometry(WaterKind::None, &[], &scene(), &serde_json::Value::Null)
}

fn zones() -> Vec<LandUseZone> {
    vec![LandUseZone {
        id: "rural".into(),
        kind: "rural".into(),
        world_x: 0,
        world_z: 0,
        width_m: 256,
        depth_m: 256,
        source: "fixture".into(),
    }]
}

/// One flat street 64 m long at z = 40, one GRADED street at z = 50 whose authored
/// profile stands `ridge_mm` above the datum, and a 24x24 lot at (128, 72) whose door
/// faces north at (128, 59) - 5 m from the graded band's edge.
fn plan(ridge_mm: i32) -> RoadNetworkPlan {
    let mut plan: RoadNetworkPlan = serde_json::from_value(json!({
        "sceneId": "entrance-recheck",
        "model": "fixture",
        "naturalHeightfieldHash": "source",
        "status": "planned_connected_network",
        "gridSpacingM": 64,
        "maxRoadbedFillM": 6.,
        "levelM": 0,
        "candidates": 1,
        "rejectedWater": 0,
        "rejectedRelief": 0,
        "unselectedCandidates": 0,
        "nodes": [
            {"id": 0, "worldX": 92, "worldZ": 40},
            {"id": 1, "worldX": 164, "worldZ": 40},
            {"id": 2, "worldX": 96, "worldZ": 50},
            {"id": 3, "worldX": 160, "worldZ": 50},
        ],
        "edges": [
            {
                "id": "street",
                "from": 0,
                "to": 1,
                "worldX": 128,
                "worldZ": 40,
                "lengthM": 64,
                "rotationYDeg": 0.,
                "naturalMinM": -1.,
                "naturalMaxM": -1.,
            },
            {
                "id": "ridge",
                "from": 2,
                "to": 3,
                "worldX": 128,
                "worldZ": 50,
                "lengthM": 64,
                "rotationYDeg": 0.,
                "naturalMinM": -1.,
                "naturalMaxM": -1.,
                "profile": {
                    "version": 1,
                    "roadWidthM": 8,
                    "start": [96, 50],
                    "end": [160, 50],
                    "elevationsMm": vec![ridge_mm as i64; 65],
                    "maxFillMm": 6000,
                },
            },
        ],
        "roadsideSites": [{
            "id": LOT,
            "roadId": "street",
            "worldX": 128,
            "worldZ": 72,
            "widthM": 24,
            "depthM": 24,
            "supportLevelM": 0,
            "rotationYDeg": 0.,
            "naturalMinM": -1.,
            "naturalMaxM": -1.,
            "purpose": "urban",
        }],
    }))
    .expect("the fixture plan parses");
    plan.joint_layout = Some(JointLayout {
        model: "service-reservation-and-housing-recovery-v1".into(),
        residential_minimum: 3,
        baseline_residential_count: 1,
        retained_residential_ids: vec![],
        displaced_residential_ids: vec![],
        replacement_residential_ids: vec![LOT.into()],
        unfilled_residential: 0,
        recovery_rejections: BTreeMap::new(),
    });
    plan.pad_conversions = vec![Conversion::rural_to_urban(LOT, 484)];
    plan
}

fn recheck(plan: &mut RoadNetworkPlan) -> Vec<super::road_network::BaseProfileRejection> {
    road_network::refuse_entrances_lost_after_pad_list(
        plan,
        &scene(),
        &natural(),
        water(),
        Some(&zones()),
    )
    .expect("the re-check composes a raster")
}

/// 1. The lot an EARLIER generator accepted is refused by name once a later street's
///    fill stands over the ground between its door and its street - with the measured
///    distance to the nearest street - and it leaves every record derived from the
///    pad list.
#[test]
fn a_lot_buried_by_a_later_streets_fill_is_refused_by_name() {
    let mut plan = plan(RIDGE_MM);
    let refusals = recheck(&mut plan);
    assert_eq!(refusals.len(), 1, "exactly the buried lot is refused");
    let refusal = &refusals[0];
    assert_eq!(refusal.edge_id, LOT);
    assert_eq!(refusal.category, PAD_ENTRANCE_LOST_AFTER_PAD_LIST);
    // The gate's own verdict, and the measurement this refusal adds: the checked
    // route reached no street at all, and the door stands 5.15 m from the graded
    // street's published rectangle (which ends at z = 54, the door at z = 59.15).
    assert!(
        refusal.reason.contains(
            "no checked pedestrian route to its own street (no_walkable_route_within_budget)"
        ),
        "{}",
        refusal.reason
    );
    assert!(
        refusal
            .reason
            .contains("its door is 5.15 m from the nearest street (ridge)"),
        "{}",
        refusal.reason
    );
    assert!(
        refusal.reason.contains("within the 120 m budget"),
        "{}",
        refusal.reason
    );
    assert!(
        refusal
            .reason
            .contains("pad lot (24x24, declared top 0.000 m on road street)"),
        "{}",
        refusal.reason
    );

    // Five-way withdrawal, from ONE operation.
    //  * the plan's own pad list;
    assert!(!plan.roadside_sites.iter().any(|site| site.id == LOT));
    //  * the entity list the plan derives (no entity can be published for it);
    assert!(!plan.site_entities().iter().any(|e| e.entity_id == LOT));
    //  * the joint receipt, with the shortfall recomputed, never decremented;
    let joint = plan.joint_layout.as_ref().unwrap();
    assert!(joint.replacement_residential_ids.is_empty());
    assert_eq!(joint.unfilled_residential, 3, "3 requested, none retained");
    //  * the conversion record it had earned;
    assert!(plan.pad_conversions.is_empty());
    //  * and the published refusal, which is what the refused-id readers use.
    assert!(
        road_network::refused_pad_ids(&plan).contains(LOT),
        "the refusal is published under a category the refused-pad readers know"
    );
    assert!(
        plan.roadside_site_rejections
            .iter()
            .any(|r| r.edge_id == LOT && r.category == PAD_ENTRANCE_LOST_AFTER_PAD_LIST)
    );
    // The streets are untouched: this refuses a lot, never a road.
    assert_eq!(plan.edges.len(), 2);
}

/// 2. The control: the same lot, the same two streets, the same distance - but the
///    graded street's profile stands ON the natural ground, so it places no fill over
///    the lot's door and the door keeps its walkable entrance. Nothing is refused.
#[test]
fn the_same_lot_is_accepted_when_no_later_fill_stands_over_its_door() {
    let mut plan = plan(-1_000);
    let refusals = recheck(&mut plan);
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(plan.roadside_sites.len(), 1);
    assert_eq!(plan.roadside_sites[0].id, LOT);
    assert_eq!(
        plan.joint_layout
            .as_ref()
            .unwrap()
            .replacement_residential_ids,
        vec![LOT.to_string()]
    );
}

/// 3. A forged record cannot buy the lot back. Declaring the street's own level to
///    the millimetre and publishing a frontage point on the street's axis is exactly
///    what a lot buried by a neighbour's fill would want to claim; the gate reads the
///    composed surface, never the declaration, so both are inert.
#[test]
fn a_forged_level_and_frontage_point_cannot_buy_the_buried_lot_back() {
    let mut plan = plan(RIDGE_MM);
    {
        let site = plan
            .roadside_sites
            .iter_mut()
            .find(|site| site.id == LOT)
            .expect("the lot is planned");
        site.support_level_mm = Some(0);
        site.frontage_road_point = Some([128, 40]);
        site.support_level_m = 0;
    }
    let refusals = recheck(&mut plan);
    assert_eq!(refusals.len(), 1, "a forged declaration is not access");
    assert_eq!(refusals[0].edge_id, LOT);
    assert_eq!(refusals[0].category, PAD_ENTRANCE_LOST_AFTER_PAD_LIST);
}

/// 4. A lot whose assigned street the completed plan no longer publishes is refused
///    by name as well: it fronts nothing, and a dangling `roadId` is never published.
#[test]
fn a_lot_whose_street_was_withdrawn_is_refused_by_name() {
    let mut plan = plan(-1_000);
    plan.edges.retain(|edge| edge.id != "street");
    let refusals = recheck(&mut plan);
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].edge_id, LOT);
    assert_eq!(refusals[0].category, PAD_ENTRANCE_LOST_AFTER_PAD_LIST);
    assert!(
        refusals[0].reason.contains(
            "road street is not published by the completed plan, so the lot fronts no street it can enter"
        ),
        "{}",
        refusals[0].reason
    );
    assert!(plan.roadside_sites.is_empty());
}

/// 5. The pass is deterministic: the same plan, twice, produces the same refusal
///    bytes - the property the bake's own determinism rests on.
#[test]
fn the_recheck_is_deterministic() {
    let mut first = plan(RIDGE_MM);
    let mut second = plan(RIDGE_MM);
    let a = recheck(&mut first);
    let b = recheck(&mut second);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
}
