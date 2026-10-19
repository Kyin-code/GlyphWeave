//! The shoreline commercial row: the FIRST street landward of the shore.
//!
//! Round 21/23 measured the defect this replaces. The legacy bayfront band placed its shops at
//! `centre ± (half_width + 150)`, a fixture coordinate that in the registered river-delta sample is
//! 150 m from the water: all twelve shops landed in the rural zoning three blocks behind the quay,
//! while the two streets that actually run along the shore carried no shopfront at all. A waterfront
//! city (Amsterdam, Wuhan, Qingdao) puts its commerce on the first street behind the quay, on the
//! landward side, so that is what this row is.
//!
//! It is planned as a PAD PROGRAMME for three reasons, each of which is the existing code:
//!
//!  * a pad is a flat, fill-only platform whose declared top is what the ROADS already require
//!    inside its window (`worldgen::services`, rule A'). A shoreline parcel is exactly that: a
//!    frontage strip on a graded quay street, one building deep, whose platform must meet the road
//!    rather than the natural ground. The frontage-property path cannot place one there (its own
//!    preflight refuses a footprint whose fill blend reaches the water, measured on this sample), and
//!    inventing a third engineering model for a shop row is the defect, not the fix;
//!  * the pad programme already declares a purpose on the land a pad takes and books the rural to
//!    urban conversion of its buildable core (`frontage::pad_purpose`, `frontage::pad_conversions`,
//!    `settlement::landuse_ledger::audit_conversions`). That is Round 23's accounting, inherited
//!    whole: with `style.frontageProgram.ruralConversion` off nothing converts and a rural station is
//!    refused by name; with it on, every converted square metre is re-derivable from the plan;
//!  * a pad publishes its own named refusals (`ServiceOutcome::rejected_candidates`), so a station
//!    that cannot land says WHY in the plan instead of silently missing from the world.
//!
//! The row is planned BEFORE the roadside pad programme's own station scan
//! (`road_network::plan_with_zones_demand`), because the quay street's frontage belongs to the
//! commerce: planned after it, the dwellings have already taken the whole block behind the street in
//! 24 m lots and `joint_frontage` keeps every parcel 2 m clear of its neighbour, which left 2 of 28
//! urban stations free. Planned first, the pad scan reserves around the row instead.
use super::{
    EntityInstance, LandUseZone, SceneSpec, WaterGeometry,
    frontage::RuralConversion,
    road_network::RoadNetworkPlan,
    services::{self, ServiceProgram, ServiceRequest},
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use std::collections::{BTreeMap, BTreeSet};

/// The service request id the row publishes itself under, and the key its outcome is read by.
pub(super) const SHOREFRONT_ROW: &str = "shorefront-row";
/// The shopfront: 12 m along the street, 12 m deep, one storey. Inside the `storefront` asset
/// contract (6..120 wide, 6..80 deep, 2..30 high) and the legacy band's own dimensions.
const SHOREFRONT_WIDTH_M: u32 = 12;
const SHOREFRONT_DEPTH_M: u32 = 12;
const SHOREFRONT_HEIGHT_M: u32 = 6;
/// One shop per 18 m of frontage: the legacy bayfront band's own rhythm.
const SHOREFRONT_STATION_SPACING_M: u32 = 18;
/// Bounded demand: a 512 m scene can carry at most this many shops on two banks.
const SHOREFRONT_MAX_SHOPS: u32 = 128;
/// The candidate frontage rungs, in order: closest to the quay first.
const SHOREFRONT_SETBACKS: [i64; 2] = [10, 24];

/// Plan the row onto an already planned road network.
///
/// Every early return here is a world that publishes the plan it always did: no structured river,
/// no opt-in shoreline band (which is what makes the 4 m strip a reserved public passage at all), no
/// zoning to judge purpose on, or no street morphology stated (`frontage::shorefront_row_contract`,
/// the same gate the legacy band's retirement reads).
pub(super) fn plan(
    result: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    conversion: RuralConversion,
    road_cut_mm: i32,
) -> Result<(), TerrainError> {
    if water.kind != super::WaterKind::River || !water.shore_band() {
        return Ok(());
    }
    let Some(zones) = zones else {
        return Ok(());
    };
    let Some((road_ids, frontage_m)) = first_shore_streets(result, water) else {
        return Ok(());
    };
    if road_ids.is_empty() || result.level_m.is_none() {
        return Ok(());
    }
    let count = (frontage_m / i64::from(SHOREFRONT_STATION_SPACING_M))
        .clamp(0, i64::from(SHOREFRONT_MAX_SHOPS)) as u32;
    if count == 0 {
        return Ok(());
    }
    let program = ServiceProgram {
        version: 1,
        housing_minimum: BTreeMap::new(),
        requests: vec![ServiceRequest {
            id: SHOREFRONT_ROW.into(),
            scene_id: scene.scene_id.clone(),
            kind: "storefront".into(),
            count,
            width_m: SHOREFRONT_WIDTH_M,
            depth_m: SHOREFRONT_DEPTH_M,
            height_m: SHOREFRONT_HEIGHT_M,
        }],
    };
    services::append_shorefront(
        result,
        scene,
        natural,
        water,
        zones,
        &program,
        conversion,
        &road_ids,
        // The row stands as close to its own pavement as the pad contract allows: the
        // road-clearance rule refuses a pad nearer its street's axis than `depth/2 + 9`, so the
        // first rung is 10 - the pad's near edge exactly on the 4 m road-protection line. Closer is
        // not a preference here, it is what makes the checked route end on the ROW's own street
        // instead of the parallel one 32 m away. The second rung is the fallback for a frontage
        // the first row cannot take.
        &SHOREFRONT_SETBACKS,
        road_cut_mm,
    )
}

/// The first street landward of the shore on each bank: the road ids to plan the row on, and the
/// total frontage length those streets offer.
///
/// "Landward" is the published shore clearance, not an offset: a street whose own corridor cannot
/// keep the 4 m clearance is not a quay street, it is a road over the water. Among the streets that
/// do keep it, the one NEAREST the water is the first row; the bank street is one continuous street
/// laid in 64 m segments, so all of its segments belong to the same row.
fn first_shore_streets(
    plan: &RoadNetworkPlan,
    water: WaterGeometry,
) -> Option<(BTreeSet<String>, i64)> {
    let roads: Vec<EntityInstance> = plan.entities();
    let mut picked: BTreeSet<String> = BTreeSet::new();
    let mut frontage_m = 0_i64;
    for sign in [-1_i32, 1] {
        let mut by_axis: BTreeMap<i32, Vec<&EntityInstance>> = BTreeMap::new();
        for road in &roads {
            if road.rotation_y_deg != 90. {
                continue;
            }
            if (f64::from(road.world_x) - water.center_x) * f64::from(sign) <= 0. {
                continue;
            }
            let (half_w, half_d) =
                super::spatial_half_extents(road.width_m, road.depth_m, road.rotation_y_deg);
            if !water.clears_shore(
                f64::from(road.world_x),
                f64::from(road.world_z),
                half_w,
                half_d,
            ) {
                continue;
            }
            by_axis.entry(road.world_x).or_default().push(road);
        }
        let nearest = if sign < 0 {
            by_axis.keys().next_back().copied()
        } else {
            by_axis.keys().next().copied()
        };
        let Some(axis) = nearest else {
            continue;
        };
        for road in &by_axis[&axis] {
            picked.insert(road.entity_id.clone());
            // A north-south segment runs along world Z, so its frontage length is its width.
            frontage_m += i64::from(road.width_m as u32);
        }
    }
    (!picked.is_empty()).then_some((picked, frontage_m))
}
