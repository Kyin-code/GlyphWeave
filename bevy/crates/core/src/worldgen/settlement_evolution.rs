//! Demand- and capacity-led settlement growth.
//!
//! Building counts are outputs, never authored completion targets. The planner
//! walks terrain routes, creates legal candidate lots and spends their floor
//! area against household, job, retail and service demand. Candidates that fail
//! slope, water or access contracts are named in the report.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::{BakedTerrain, TerrainError};

use super::community_typology;
use super::epoch_evolution;
use super::route_evolution::Plan as RoutePlan;
use super::water_query::WATER_SHORE_CLEARANCE_M;
use super::{
    oriented_footprints_overlap, urban_slice, EntityInstance, OrientedFootprint, SceneSpec,
    WaterGeometry, STREAM_CHUNK_METERS,
};
use crate::settlement::building_archetypes;

/// Candidate parcels keep a conservative margin from streaming seams. Once a
/// building body has been selected, its exact approach corridor is checked
/// again below, so this early guard does not have to discard the whole block.
// Candidate and fill-transition work must stay clear of a streaming seam.
// The final approach check is narrower than the earthwork ring and therefore
// cannot substitute for this guard.
const SETTLEMENT_SEAM_GUARD_M: f64 = 10.5;
const SETTLEMENT_APPROACH_HALF_WIDTH_M: f64 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub households_target: u32,
    pub jobs_target: u32,
    pub retail_floor_area_m2: f64,
    #[serde(default)]
    pub service_budget: f64,
    /// Opt-in low-rise infill inside blocks already bounded by the road
    /// network. Candidates still pass the same placement and access contracts.
    #[serde(default)]
    pub interior_infill: bool,
    #[serde(default = "default_lot_width")]
    pub lot_width_m: f32,
    #[serde(default = "default_lot_depth")]
    pub lot_depth_m: f32,
    #[serde(default = "default_road_step")]
    pub road_step_m: f32,
    #[serde(default = "default_setback")]
    pub setback_m: f32,
    #[serde(default = "default_max_slope")]
    pub max_slope: f32,
    #[serde(default = "default_far")]
    pub floor_area_ratio: f32,
    #[serde(default = "default_max_floors")]
    pub max_floors: u32,
    #[serde(default = "default_access_height_tolerance")]
    pub access_height_tolerance_m: f32,
    /// Weak local pad-cut allowance. Feature cuts may declare a larger value in
    /// the manifest; candidates whose natural hillside exceeds it are named
    /// rather than silently turned into a deep trench.
    #[serde(default = "default_max_pad_cut")]
    pub max_pad_cut_m: f32,
    /// Optional epoch-level demand reservation. The first selection pass may
    /// spend only a stage's reserved budget; unused reservations are released
    /// to a second, global pass. This preserves a real old-core/renewal/new-growth
    /// cross-section without turning district share into a building count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district_demand: Option<DistrictDemandProgram>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistrictDemandProgram {
    pub version: u32,
    pub shares: Vec<DistrictDemandShare>,
    #[serde(default = "default_true")]
    pub allow_spill: bool,
    /// Provenance for budgets composed from an urban-case prior. Authored
    /// budgets may omit these fields; inferred budgets must identify their
    /// model and rationale so the allocation remains auditable downstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub inferred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistrictDemandShare {
    pub stage: String,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Demand {
    pub households: u32,
    pub jobs: u32,
    pub retail_floor_area_m2: f64,
    pub service_budget: f64,
}

impl Default for Demand {
    fn default() -> Self {
        Self {
            households: 0,
            jobs: 0,
            retail_floor_area_m2: 0.0,
            service_budget: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Capacity {
    pub candidate_lots: usize,
    pub accepted_lots: usize,
    pub rejected_lots: usize,
    pub floor_area_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DistrictAllocation {
    pub stage: String,
    pub share: f64,
    pub planned: Demand,
    pub delivered: Demand,
    pub remaining: Demand,
    pub eligible_lots: usize,
    pub accepted_lots: usize,
    pub spill_accepted_lots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreetFrontageRun {
    pub route_id: String,
    pub side: String,
    pub district_stage: String,
    pub buildings: usize,
    pub frontage_span_m: f64,
    pub facade_length_m: f64,
    pub uncovered_length_m: f64,
    pub longest_gap_m: f64,
    pub coverage_ratio: f64,
    pub continuous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreetFrontageAudit {
    pub status: String,
    pub model: String,
    pub old_core_run_count: usize,
    pub old_core_continuous_runs: usize,
    pub old_core_min_coverage_ratio: f64,
    pub runs: Vec<StreetFrontageRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Building {
    pub id: String,
    pub kind: String,
    pub route_id: String,
    pub side: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    pub floors: u32,
    pub floor_area_m2: f64,
    pub households: u32,
    pub jobs: u32,
    pub retail_floor_area_m2: f64,
    pub service_budget: f64,
    pub access_distance_m: f32,
    pub center_distance_m: f64,
    #[serde(default)]
    pub community_profile: String,
    #[serde(default)]
    pub income_band: String,
    #[serde(default)]
    pub archetype_id: String,
    #[serde(default)]
    pub recommended_count: u32,
    #[serde(default)]
    pub lot_area_m2: f64,
    #[serde(default)]
    pub coverage_ratio: f64,
    #[serde(default)]
    pub frontage_road_point: Option<[i32; 2]>,
    #[serde(default)]
    pub frontage_position_m: f64,
    #[serde(default)]
    pub district_stage: String,
    #[serde(default)]
    pub frontage_level_m: Option<f64>,
    #[serde(default)]
    pub max_pad_cut_mm: Option<i32>,
    #[serde(default)]
    pub rotation_y_deg: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub candidate_id: String,
    pub route_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub demand: Demand,
    pub delivered: Demand,
    pub unmet_demand: Demand,
    pub capacity: Capacity,
    pub buildings: Vec<Building>,
    pub rejections: Vec<Rejection>,
    #[serde(default)]
    pub community_profile: Option<String>,
    #[serde(default)]
    pub district_allocation: Vec<DistrictAllocation>,
    #[serde(default)]
    pub street_frontage: StreetFrontageAudit,
}

#[derive(Debug, Clone)]
struct Candidate {
    id: String,
    route_id: String,
    side: String,
    world_x: i32,
    world_z: i32,
    width_m: f32,
    depth_m: f32,
    access_distance_m: f32,
    center_distance_m: f64,
    pressure: f64,
    frontage_road_point: [i32; 2],
    frontage_position_m: f64,
    district_stage: String,
    road_design_m: f64,
    required_cut_mm: i32,
    rotation_y_deg: f32,
}

#[derive(Debug, Clone)]
struct SelectionRules {
    community_profile: String,
    household_area_m2: f64,
    job_area_m2: f64,
    retail_share: f64,
    floor_area_ratio: f64,
    max_floors: u32,
}

struct AcceptedCandidate {
    building: Building,
    spent: Demand,
    floor_area_m2: f64,
}

const fn default_true() -> bool {
    true
}
const fn default_lot_width() -> f32 {
    24.0
}
const fn default_lot_depth() -> f32 {
    28.0
}
const fn default_road_step() -> f32 {
    36.0
}
const fn default_setback() -> f32 {
    12.0
}
const fn default_max_slope() -> f32 {
    0.25
}
const fn default_far() -> f32 {
    1.6
}
const fn default_max_floors() -> u32 {
    18
}
const fn default_access_height_tolerance() -> f32 {
    6.0
}
const fn default_max_pad_cut() -> f32 {
    2.0
}

pub fn enabled(style: &Value) -> bool {
    from_style(style).is_ok_and(|program| program.is_some_and(|program| program.enabled))
}

pub fn validate_style(style: &Value) -> Result<(), String> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    if !program.enabled {
        return Ok(());
    }
    if program.version != 1
        || program.households_target == 0 && program.jobs_target == 0
        || !(8.0..=80.0).contains(&program.lot_width_m)
        || !(8.0..=80.0).contains(&program.lot_depth_m)
        || !(8.0..=120.0).contains(&program.road_step_m)
        || !(2.0..=32.0).contains(&program.setback_m)
        || !(0.02..=0.25).contains(&program.max_slope)
        || !(0.2..=8.0).contains(&program.floor_area_ratio)
        || !(1..=60).contains(&program.max_floors)
        || !(1.0..=30.0).contains(&program.access_height_tolerance_m)
        || !(0.1..=30.0).contains(&program.max_pad_cut_m)
        || !program.retail_floor_area_m2.is_finite()
        || program.retail_floor_area_m2 < 0.0
        || !program.service_budget.is_finite()
        || program.service_budget < 0.0
    {
        return Err("invalid settlementEvolution demand or lot limits".into());
    }
    if style
        .get("frontageProgram")
        .and_then(|p| p.get("requests"))
        .is_some()
    {
        return Err(
            "settlementEvolution cannot be combined with frontageProgram.requests.count".into(),
        );
    }
    if let Some(districts) = &program.district_demand {
        if districts.version != 1 || districts.shares.is_empty() {
            return Err("invalid settlementEvolution districtDemand".into());
        }
        if districts.inferred
            && (districts
                .model
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || districts
                    .inference_reason
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty()))
        {
            return Err(
                "inferred settlementEvolution districtDemand requires model and inferenceReason"
                    .into(),
            );
        }
        let mut stages = std::collections::BTreeSet::new();
        let mut sum = 0.0;
        for entry in &districts.shares {
            if !matches!(
                entry.stage.as_str(),
                "old-core" | "renewal" | "new-growth" | "periphery"
            ) || !stages.insert(entry.stage.as_str())
                || !entry.share.is_finite()
                || !(0.0..=1.0).contains(&entry.share)
                || entry.share <= 0.0
            {
                return Err("invalid settlementEvolution districtDemand shares".into());
            }
            sum += entry.share;
        }
        if (sum - 1.0).abs() > 1e-6 {
            return Err("settlementEvolution districtDemand shares must sum to 1".into());
        }
    }
    Ok(())
}

fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("settlementEvolution") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

fn nearest_route_station(routes: &RoutePlan, x: f64, z: f64) -> Option<([i32; 2], f64, f64)> {
    let mut best: Option<([i32; 2], f64, f64)> = None;
    for route in routes.routes.iter().filter(|route| route.status == "pass") {
        for segment in route.polyline.windows(2) {
            let ax = f64::from(segment[0].x);
            let az = f64::from(segment[0].z);
            let bx = f64::from(segment[1].x);
            let bz = f64::from(segment[1].z);
            let dx = bx - ax;
            let dz = bz - az;
            let length_squared = dx * dx + dz * dz;
            if length_squared < 1e-9 {
                continue;
            }
            let t = (((x - ax) * dx + (z - az) * dz) / length_squared).clamp(0.0, 1.0);
            let px = ax + dx * t;
            let pz = az + dz * t;
            let distance = (x - px).hypot(z - pz);
            if best.is_none_or(|(_, prior, _)| distance < prior) {
                let design_m =
                    segment[0].design_height_m + (segment[1].design_height_m - segment[0].design_height_m) * t;
                best = Some(([px.round() as i32, pz.round() as i32], distance, design_m));
            }
        }
    }
    best
}

pub fn plan(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    routes: &RoutePlan,
    style: &Value,
) -> Result<Option<Plan>, TerrainError> {
    validate_style(style).map_err(TerrainError)?;
    let Some(program) = from_style(style).map_err(TerrainError)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    let focus_bias = if program
        .district_demand
        .as_ref()
        .is_some_and(|districts| districts.inferred)
    {
        urban_slice::compile(style)
            .map_err(TerrainError)?
            .map(|slice| slice.focus_bias)
    } else {
        None
    };
    terrain.require_complete()?;
    let typology = community_typology::resolve(style).map_err(TerrainError)?;
    let base_lot_width_m = (program.lot_width_m
        * typology.as_ref().map_or(1.0, |value| value.lot_width_scale))
    .clamp(8.0, 80.0);
    let base_lot_depth_m = (program.lot_depth_m
        * typology.as_ref().map_or(1.0, |value| value.lot_depth_scale))
    .clamp(8.0, 80.0);
    let floor_area_ratio =
        program.floor_area_ratio * typology.as_ref().map_or(1.0, |value| value.far_scale);
    let max_floors = ((program.max_floors as f32
        * typology.as_ref().map_or(1.0, |value| value.floors_scale))
    .round() as u32)
        .max(1);
    let household_area_m2 = typology
        .as_ref()
        .map_or(85.0, |value| value.household_area_m2);
    let job_area_m2 = typology.as_ref().map_or(38.0, |value| value.job_area_m2);
    let retail_share = typology.as_ref().map_or(0.12, |value| value.retail_share);
    let center = route_center(routes);
    let terrace_authorized = style
        .get("urbanSlice")
        .and_then(|slice| slice.get("mode"))
        .and_then(Value::as_str)
        == Some("feature-cut");
    let target_demand = Demand {
        households: program.households_target,
        jobs: program.jobs_target,
        retail_floor_area_m2: program.retail_floor_area_m2,
        service_budget: program.service_budget,
    };
    let mut demand = Demand {
        households: program.households_target,
        jobs: program.jobs_target,
        retail_floor_area_m2: program.retail_floor_area_m2,
        service_budget: program.service_budget,
    };
    let mut delivered = Demand {
        households: 0,
        jobs: 0,
        retail_floor_area_m2: 0.0,
        service_budget: 0.0,
    };
    let mut buildings = Vec::new();
    let mut rejections = Vec::new();
    let mut accepted_lots = 0;
    let mut candidate_lots = 0;
    let mut total_floor_area = 0.0;
    let mut candidates = Vec::<Candidate>::new();
    let materialized_roads = super::route_evolution::materialize(routes);
    let mut road_reservations = materialized_roads.clone();
    for road in &mut road_reservations {
        if road.kind == "road" {
            // The published road owns its pavement plus the graded transport
            // shoulder. Buildings must clear the shoulder as well as the visible
            // carriageway or the road's support resolution can raise terrain
            // through the neighbouring building pad.
            road.depth_m += 6.0;
        }
    }
    // Old-core frontage is sampled at the narrow-lot cadence, not the coarse
    // district lot pitch. Demand still decides how many buildings land; this
    // only removes the artificial 10-20m holes between otherwise legal row
    // houses. Other stages keep their larger parcels because the overlap gate
    // below still uses each stage's own lot dimensions.
    let old_core_lot_width = (base_lot_width_m * 0.875).clamp(7.0, 80.0);
    let frontage_step_m = program.road_step_m.min(old_core_lot_width + 1.0).max(6.0);
    // The old-core sample is a tighter walking grain, not a global minimum lot
    // size. A five-metre cadence is legal for the four-metre row-house
    // archetype and is confined to the innermost 125 m, leaving renewal and
    // new-growth parcels at their coarser cadence.
    let core_frontage_step_m = 6.6_f64;
    for route in routes.routes.iter().filter(|route| route.status == "pass") {
        // Frontage cadence is measured along the whole route, not restarted at
        // every terrain-grid vertex. Route polylines commonly have 4-8 m
        // segments; sampling each segment independently used to place several
        // overlapping lots at the same bend and made 86% of candidates fail the
        // overlap gate before demand could be evaluated.
        let mut segment_lengths = Vec::with_capacity(route.polyline.len().saturating_sub(1));
        let mut route_length_m = 0.0_f64;
        for segment in route.polyline.windows(2) {
            let length = f64::from(segment[1].x - segment[0].x)
                .hypot(f64::from(segment[1].z - segment[0].z));
            segment_lengths.push(length);
            route_length_m += length;
        }
        let standard_step = f64::from(frontage_step_m);
        let mut cursor_m = 0.0_f64;
        let mut segment_index = 0_usize;
        let mut segment_start_m = 0.0_f64;
        while cursor_m + standard_step * 0.5 < route_length_m {
            let probe_m = cursor_m + standard_step * 0.5;
            while segment_index + 1 < segment_lengths.len()
                && probe_m > segment_start_m + segment_lengths[segment_index]
            {
                segment_start_m += segment_lengths[segment_index];
                segment_index += 1;
            }
            let probe_length = segment_lengths[segment_index];
            if probe_length < 1.0 {
                cursor_m += standard_step;
                continue;
            }
            let probe_t = ((probe_m - segment_start_m) / probe_length).clamp(0.0, 1.0);
            let probe_segment = &route.polyline[segment_index..=segment_index + 1];
            let probe_x = f64::from(probe_segment[0].x)
                + f64::from(probe_segment[1].x - probe_segment[0].x) * probe_t;
            let probe_z = f64::from(probe_segment[0].z)
                + f64::from(probe_segment[1].z - probe_segment[0].z) * probe_t;
            let step = if (probe_x - center.0).hypot(probe_z - center.1) <= 125.0 {
                core_frontage_step_m
            } else {
                standard_step
            };
            let probe_stage =
                district_stage(style, (probe_x - center.0).hypot(probe_z - center.1));
            // A street wall is made of parcels, not identical 7-9m slots. The
            // module count changes the frontage lot width and the advance along
            // the route together, so wider blocks do not create overlapping
            // candidates while narrow infill remains possible.
            let modules = frontage_module_count(route.id.as_str(), probe_m, probe_stage);
            let parcel_step_m = step * f64::from(modules);
            let route_position_m = cursor_m + parcel_step_m * 0.5;
            let (segment_index, segment_start_m, length, local_t) = {
                let mut walked = 0.0_f64;
                let mut found = None;
                for (index, segment_length) in segment_lengths.iter().copied().enumerate() {
                    if route_position_m <= walked + segment_length {
                        found = Some((
                            index,
                            walked,
                            segment_length,
                            ((route_position_m - walked) / segment_length).clamp(0.0, 1.0),
                        ));
                        break;
                    }
                    walked += segment_length;
                }
                let Some(sample) = found else {
                    break;
                };
                sample
            };
            if length < 1.0 {
                cursor_m += parcel_step_m;
                continue;
            }
            let segment = &route.polyline[segment_index..=segment_index + 1];
            let dx = f64::from(segment[1].x - segment[0].x);
            let dz = f64::from(segment[1].z - segment[0].z);
            let normal_x = -dz / length;
            let normal_z = dx / length;
            // Keep the parcel's local X axis on the road tangent. In the
            // serialized/Three frame +Y rotation maps +X toward -Z, so the
            // tangent angle must be negated relative to atan2(dz, dx).
            let rotation_y_deg = (-dz).atan2(dx).to_degrees() as f32;
            let road_x = f64::from(segment[0].x) + dx * local_t;
            let road_z = f64::from(segment[0].z) + dz * local_t;
            for (side_index, side) in [-1.0_f64, 1.0].into_iter().enumerate() {
                candidate_lots += 1;
                let candidate_id = format!(
                    "{}.lot.{}.{}",
                    route.id,
                    (route_position_m * 1000.0).round() as i64,
                    side_index
                );
                let access =
                    f64::from(route.road_width_m) * 0.5 + 3.0 + f64::from(program.setback_m);
                let mut center_x =
                    road_x + normal_x * side * (access + f64::from(base_lot_depth_m) * 0.5);
                let mut center_z =
                    road_z + normal_z * side * (access + f64::from(base_lot_depth_m) * 0.5);
                let mut x = center_x.round() as i32;
                let mut z = center_z.round() as i32;
                let mut center_distance = (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                let district_stage = district_stage(style, center_distance);
                let (base_stage_width_m, lot_depth_m) =
                    staged_lot_size(district_stage, base_lot_width_m, base_lot_depth_m);
                let lot_width_m = base_stage_width_m * f32::from(modules);
                center_x = road_x + normal_x * side * (access + f64::from(lot_depth_m) * 0.5);
                center_z = road_z + normal_z * side * (access + f64::from(lot_depth_m) * 0.5);
                x = center_x.round() as i32;
                z = center_z.round() as i32;
                center_distance = (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                let lot_slope =
                    match lot_is_legal(scene, terrain, water, x, z, lot_width_m, lot_depth_m) {
                        Ok(slope) => slope,
                        Err(reason) => {
                            rejections.push(Rejection {
                                candidate_id,
                                route_id: route.id.clone(),
                                world_x: x,
                                world_z: z,
                                reason,
                            });
                            continue;
                        }
                    };
                let road_design_m = segment[0].design_height_m
                    + (segment[1].design_height_m - segment[0].design_height_m) * local_t;
                let foundation_m = road_design_m.max(nearby_road_surface_m(
                    &materialized_roads,
                    x,
                    z,
                    lot_width_m,
                    lot_depth_m,
                    rotation_y_deg,
                ));
                let lot_ground_m = terrain.height(x, z).unwrap_or(f32::INFINITY);
                if (road_design_m - f64::from(lot_ground_m)).abs()
                    > f64::from(program.access_height_tolerance_m)
                {
                    rejections.push(Rejection {
                        candidate_id,
                        route_id: route.id.clone(),
                        world_x: x,
                        world_z: z,
                        reason: "road_access_height_mismatch".into(),
                    });
                    continue;
                }
                let natural_ceiling_m =
                    lot_ground_ceiling_m(terrain, x, z, lot_width_m, lot_depth_m, rotation_y_deg)
                        .unwrap_or(f32::INFINITY);
                let entrance_ceiling_m = facade_anchor(
                    x,
                    z,
                    lot_width_m,
                    lot_depth_m,
                    rotation_y_deg,
                    road_x,
                    road_z,
                )
                .and_then(|(anchor_x, anchor_z, _)| {
                    lot_ground_ceiling_m(terrain, anchor_x, anchor_z, 4.0, 4.0, 0.0)
                })
                .unwrap_or(f32::NEG_INFINITY);
                let required_cut_m =
                    (f64::from(natural_ceiling_m.max(entrance_ceiling_m)) - foundation_m).max(0.0);
                if required_cut_m > f64::from(program.max_pad_cut_m) {
                    rejections.push(Rejection {
                        candidate_id,
                        route_id: route.id.clone(),
                        world_x: x,
                        world_z: z,
                        reason: "road_support_cut_exceeds_contract".into(),
                    });
                    continue;
                }
                if lot_slope > program.max_slope && !terrace_authorized {
                    rejections.push(Rejection {
                        candidate_id,
                        route_id: route.id.clone(),
                        world_x: x,
                        world_z: z,
                        reason: "slope_exceeds_contract".into(),
                    });
                    continue;
                }
                let lot_footprint = OrientedFootprint {
                    center_x: f64::from(x),
                    center_z: f64::from(z),
                    width_m: f64::from(lot_width_m),
                    depth_m: f64::from(lot_depth_m),
                    rotation_y_deg: f64::from(rotation_y_deg),
                };
                // A platform edge exactly on a streaming seam is not
                // interoperable: each side bakes its own half of the pad and
                // the assembled 1 m surface can develop a one-cell cliff.
                // Keep a narrow seam guard until a cross-seam entity has an
                // explicit shared-support contract.
                let (half_x, half_z) = lot_footprint.aabb_half_extents();
                // Access spurs and their fill transition extend roughly one
                // short building depth beyond the lot. Keep that whole working
                // envelope off a streaming seam so the two chunks never bake
                // opposite halves of the same sloped approach independently.
                let seam_guard = SETTLEMENT_SEAM_GUARD_M;
                let crosses_x = (1..scene.width_m.div_ceil(STREAM_CHUNK_METERS))
                    .map(|chunk| f64::from(scene.origin_x + (chunk * STREAM_CHUNK_METERS) as i32))
                    .any(|seam| (lot_footprint.center_x - seam).abs() < half_x + seam_guard);
                let crosses_z = (1..scene.depth_m.div_ceil(STREAM_CHUNK_METERS))
                    .map(|chunk| f64::from(scene.origin_z + (chunk * STREAM_CHUNK_METERS) as i32))
                    .any(|seam| (lot_footprint.center_z - seam).abs() < half_z + seam_guard);
                if crosses_x || crosses_z {
                    rejections.push(Rejection {
                        candidate_id,
                        route_id: route.id.clone(),
                        world_x: x,
                        world_z: z,
                        reason: "straddles_chunk_seam_without_shared_support".into(),
                    });
                    continue;
                }
                candidates.push(Candidate {
                    id: candidate_id,
                    route_id: route.id.clone(),
                    side: if side < 0.0 { "left" } else { "right" }.into(),
                    world_x: x,
                    world_z: z,
                    width_m: lot_width_m,
                    depth_m: lot_depth_m,
                    access_distance_m: access as f32,
                    center_distance_m: center_distance,
                    pressure: (1.0 - center_distance / 650.0).clamp(0.15, 1.0),
                    frontage_road_point: [road_x.round() as i32, road_z.round() as i32],
                    frontage_position_m: route_position_m,
                    district_stage: district_stage.into(),
                    road_design_m: foundation_m,
                    required_cut_mm: (required_cut_m * 1000.0 + 64.0).ceil() as i32,
                    rotation_y_deg,
                });
            }
            cursor_m += parcel_step_m;
        }
    }
    if program.interior_infill {
        // Dense street walls alone leave the middle of every 60-100m block
        // empty. Sample a deterministic low-rise infill lattice inside that
        // envelope. These are not road lots: the nearest legal road is still
        // the access target and every physical/engineering gate remains live.
        // Interior rows are part of the block, not a second uniform grid laid
        // over the city. A 12m cadence with deterministic per-cell dimensions
        // admits courtyard infill, workshops and rear apartment rows while the
        // exact road/building OBB checks still reject every real conflict.
        let spacing = 12_i32;
        let radius = 400.0_f64;
        let first_x = (center.0 - radius).ceil() as i32;
        let last_x = (center.0 + radius).floor() as i32;
        let first_z = (center.1 - radius).ceil() as i32;
        let last_z = (center.1 + radius).floor() as i32;
        for grid_x in (first_x..=last_x).step_by(spacing as usize) {
            for grid_z in (first_z..=last_z).step_by(spacing as usize) {
                let hash = blake3::hash(format!("interior-infill:{grid_x}:{grid_z}").as_bytes());
                let jitter = |offset: usize| {
                    let value = u32::from_le_bytes(
                        hash.as_bytes()[offset..offset + 4]
                            .try_into()
                            .expect("four-byte infill hash"),
                    );
                    f64::from(value) / f64::from(u32::MAX) * 8.0 - 4.0
                };
                let x = (f64::from(grid_x) + jitter(0)).round() as i32;
                let z = (f64::from(grid_z) + jitter(4)).round() as i32;
                let center_distance = (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                if center_distance > radius {
                    continue;
                }
                let district_stage = district_stage(style, center_distance);
                if district_stage == "periphery" {
                    continue;
                }
                let Some((road_point, road_distance, road_design_m)) =
                    nearest_route_station(routes, f64::from(x), f64::from(z))
                else {
                    continue;
                };
                if !(12.0..=48.0).contains(&road_distance) {
                    continue;
                }
                let (stage_width, stage_depth) =
                    staged_lot_size(district_stage, base_lot_width_m, base_lot_depth_m);
                // Keep small infill for tight gaps, but let roughly one in
                // four positions become a genuine combined residential block.
                // A single uniform grain was still producing a wall of tiny
                // boxes and under-reporting the real mid-block floorspace.
                let large_block = hash.as_bytes()[8] % 4 == 0;
                let (lot_width_m, lot_depth_m) = if large_block {
                    (
                        (stage_width * 1.20).clamp(10.0, 20.0),
                        (stage_depth * 0.80).clamp(16.0, 28.0),
                    )
                } else {
                    (
                        (stage_width * 0.75).clamp(6.0, 10.0),
                        (stage_depth * 0.50).clamp(8.0, 16.0),
                    )
                };
                let rotation_y_deg = (-f64::from(road_point[0] - x))
                    .atan2(-f64::from(road_point[1] - z))
                    .to_degrees() as f32;
                if lot_is_legal(scene, terrain, water, x, z, lot_width_m, lot_depth_m).is_err() {
                    continue;
                }
                let lot_footprint = OrientedFootprint {
                    center_x: f64::from(x),
                    center_z: f64::from(z),
                    width_m: f64::from(lot_width_m),
                    depth_m: f64::from(lot_depth_m),
                    rotation_y_deg: f64::from(rotation_y_deg),
                };
                if materialized_roads.iter().any(|road| {
                    road.kind == "road"
                        && oriented_footprints_overlap(
                            &lot_footprint,
                            &OrientedFootprint::from_entity(road),
                            0.5,
                        )
                }) {
                    continue;
                }
                let lot_ground_m = terrain.height(x, z).unwrap_or(f32::INFINITY);
                if (road_design_m - f64::from(lot_ground_m)).abs()
                    > f64::from(program.access_height_tolerance_m)
                {
                    continue;
                }
                let natural_ceiling_m =
                    lot_ground_ceiling_m(terrain, x, z, lot_width_m, lot_depth_m, rotation_y_deg)
                        .unwrap_or(f32::INFINITY);
                let required_cut_m =
                    (f64::from(natural_ceiling_m) - road_design_m).max(0.0);
                if required_cut_m > f64::from(program.max_pad_cut_m) {
                    continue;
                }
                let (half_x, half_z) = lot_footprint.aabb_half_extents();
                // Infill platforms have a larger symmetric fill ring than a
                // normal frontage pad because they are not backed by a road
                // platform. Keep that complete earthwork away from the seam.
                let seam_guard = 32.0;
                let crosses_x = (1..scene.width_m.div_ceil(STREAM_CHUNK_METERS))
                    .map(|chunk| {
                        f64::from(scene.origin_x + (chunk * STREAM_CHUNK_METERS) as i32)
                    })
                    .any(|seam| (lot_footprint.center_x - seam).abs() < half_x + seam_guard);
                let crosses_z = (1..scene.depth_m.div_ceil(STREAM_CHUNK_METERS))
                    .map(|chunk| {
                        f64::from(scene.origin_z + (chunk * STREAM_CHUNK_METERS) as i32)
                    })
                    .any(|seam| (lot_footprint.center_z - seam).abs() < half_z + seam_guard);
                if crosses_x || crosses_z {
                    continue;
                }
                candidates.push(Candidate {
                    id: format!("zz-infill.lot.{x}.{z}"),
                    route_id: "zz-infill".into(),
                    side: "inner".into(),
                    world_x: x,
                    world_z: z,
                    width_m: lot_width_m,
                    depth_m: lot_depth_m,
                    access_distance_m: road_distance as f32,
                    center_distance_m: center_distance,
                    pressure: (1.0 - center_distance / 650.0).clamp(0.15, 1.0),
                    frontage_road_point: road_point,
                    frontage_position_m: road_distance,
                    district_stage: district_stage.into(),
                    road_design_m,
                    required_cut_mm: (required_cut_m * 1000.0 + 64.0).ceil() as i32,
                    rotation_y_deg,
                });
            }
        }
    }
    let rules = SelectionRules {
        community_profile: typology
            .as_ref()
            .map_or_else(|| "generic".into(), |value| value.profile.clone()),
        household_area_m2,
        job_area_m2,
        retail_share,
        floor_area_ratio: f64::from(floor_area_ratio),
        max_floors,
    };
    let mut accepted = vec![false; candidates.len()];
    let mut stage_planned = BTreeMap::<String, Demand>::new();
    let mut stage_delivered = BTreeMap::<String, Demand>::new();
    let mut stage_remaining = BTreeMap::<String, Demand>::new();
    let mut stage_eligible = BTreeMap::<String, usize>::new();
    let mut stage_accepted = BTreeMap::<String, usize>::new();
    let mut stage_spill = BTreeMap::<String, usize>::new();
    let mut district_allocation = Vec::new();
    if let Some(districts) = &program.district_demand {
        let shares = effective_district_shares(&districts.shares, focus_bias.as_ref());
        let planned_by_stage = allocate_district_demands(&target_demand, &shares);
        for entry in &shares {
            let planned = planned_by_stage
                .get(&entry.stage)
                .cloned()
                .unwrap_or_default();
            stage_planned.insert(entry.stage.clone(), planned.clone());
            stage_delivered.insert(entry.stage.clone(), Demand::default());
            stage_remaining.insert(entry.stage.clone(), planned);
        }
        for candidate in &candidates {
            *stage_eligible
                .entry(candidate.district_stage.clone())
                .or_default() += 1;
        }
        for entry in &shares {
            let mut stage_candidates: Vec<usize> = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.district_stage == entry.stage)
                .map(|(index, _)| index)
                .collect();
            if entry.stage == "old-core" {
                let mut coherent = Vec::new();
                let mut by_front = BTreeMap::<(&str, &str), Vec<usize>>::new();
                for index in stage_candidates {
                    let candidate = &candidates[index];
                    by_front
                        .entry((candidate.route_id.as_str(), candidate.side.as_str()))
                        .or_default()
                        .push(index);
                }
                for (_, mut indices) in by_front {
                    indices.sort_by(|left, right| {
                        candidates[*left]
                            .frontage_position_m
                            .total_cmp(&candidates[*right].frontage_position_m)
                            .then(candidates[*left].id.cmp(&candidates[*right].id))
                    });
                    let mut cluster_start = 0;
                    while cluster_start < indices.len() {
                        let mut cluster_end = cluster_start + 1;
                        while cluster_end < indices.len() {
                            let left = &candidates[indices[cluster_end - 1]];
                            let right = &candidates[indices[cluster_end]];
                            let allowed_gap = f64::from(left.width_m.max(right.width_m)) + 8.0;
                            if right.frontage_position_m - left.frontage_position_m > allowed_gap {
                                break;
                            }
                            cluster_end += 1;
                        }
                        let cluster = &indices[cluster_start..cluster_end];
                        if cluster.len() >= 2 {
                            coherent.extend_from_slice(cluster);
                        } else {
                            let candidate = &candidates[cluster[0]];
                            rejections.push(Rejection {
                                candidate_id: candidate.id.clone(),
                                route_id: candidate.route_id.clone(),
                                world_x: candidate.world_x,
                                world_z: candidate.world_z,
                                reason: "discontinuous_old_core_frontage".into(),
                            });
                        }
                        cluster_start = cluster_end;
                    }
                }
                stage_candidates = coherent;
            }
            // Spend one epoch's reserved demand along coherent street fronts
            // before moving to the next stage. This keeps old-core rows and
            // renewal frontages contiguous instead of scattering a small budget
            // across alternating sides of every segment.
            stage_candidates.sort_by(|left, right| {
                let left = &candidates[*left];
                let right = &candidates[*right];
                left.route_id
                    .cmp(&right.route_id)
                    .then(left.side.cmp(&right.side))
                    .then(
                        left.frontage_position_m
                            .total_cmp(&right.frontage_position_m),
                    )
                    .then(left.id.cmp(&right.id))
            });
            for index in stage_candidates {
                let candidate = &candidates[index];
                let limit = stage_remaining
                    .get(&candidate.district_stage)
                    .cloned()
                    .unwrap_or_default();
                if demand_is_empty(&limit) {
                    break;
                }
                let accepted_candidate = match try_build_candidate(
                    candidate,
                    &limit,
                    &rules,
                    scene,
                    terrain,
                    water,
                    program.max_pad_cut_m,
                    program.max_slope,
                    terrace_authorized,
                    &road_reservations,
                    &buildings,
                ) {
                    Ok(accepted) => accepted,
                    Err(reason) => {
                        rejections.push(Rejection {
                            candidate_id: candidate.id.clone(),
                            route_id: candidate.route_id.clone(),
                            world_x: candidate.world_x,
                            world_z: candidate.world_z,
                            reason: reason.into(),
                        });
                        continue;
                    }
                };
                commit_candidate(
                    index,
                    accepted_candidate,
                    candidate,
                    &mut demand,
                    &mut delivered,
                    &mut buildings,
                    &mut accepted,
                    &mut total_floor_area,
                    &mut accepted_lots,
                    &mut stage_remaining,
                    &mut stage_delivered,
                    &mut stage_accepted,
                    false,
                    &mut stage_spill,
                );
            }
        }
        if districts.allow_spill && !demand_is_empty(&demand) {
            for (index, candidate) in candidates.iter().enumerate() {
                if accepted[index] {
                    continue;
                }
                if candidate.district_stage == "old-core"
                    && !old_core_has_frontage_neighbor(&candidates, index)
                {
                    continue;
                }
                let accepted_candidate = match try_build_candidate(
                    candidate,
                    &demand,
                    &rules,
                    scene,
                    terrain,
                    water,
                    program.max_pad_cut_m,
                    program.max_slope,
                    terrace_authorized,
                    &road_reservations,
                    &buildings,
                ) {
                    Ok(accepted) => accepted,
                    Err(reason) => {
                        rejections.push(Rejection {
                            candidate_id: candidate.id.clone(),
                            route_id: candidate.route_id.clone(),
                            world_x: candidate.world_x,
                            world_z: candidate.world_z,
                            reason: reason.into(),
                        });
                        continue;
                    }
                };
                commit_candidate(
                    index,
                    accepted_candidate,
                    candidate,
                    &mut demand,
                    &mut delivered,
                    &mut buildings,
                    &mut accepted,
                    &mut total_floor_area,
                    &mut accepted_lots,
                    &mut stage_remaining,
                    &mut stage_delivered,
                    &mut stage_accepted,
                    true,
                    &mut stage_spill,
                );
            }
        }
        for entry in &shares {
            district_allocation.push(DistrictAllocation {
                stage: entry.stage.clone(),
                share: entry.share,
                planned: stage_planned.get(&entry.stage).cloned().unwrap_or_default(),
                delivered: stage_delivered
                    .get(&entry.stage)
                    .cloned()
                    .unwrap_or_default(),
                remaining: stage_remaining
                    .get(&entry.stage)
                    .cloned()
                    .unwrap_or_default(),
                eligible_lots: *stage_eligible.get(&entry.stage).unwrap_or(&0),
                accepted_lots: *stage_accepted.get(&entry.stage).unwrap_or(&0),
                spill_accepted_lots: *stage_spill.get(&entry.stage).unwrap_or(&0),
            });
        }
    } else {
        for (index, candidate) in candidates.iter().enumerate() {
            let accepted_candidate = match try_build_candidate(
                candidate,
                &demand,
                &rules,
                scene,
                terrain,
                water,
                program.max_pad_cut_m,
                program.max_slope,
                terrace_authorized,
                &road_reservations,
                &buildings,
            ) {
                Ok(accepted) => accepted,
                Err(reason) => {
                    rejections.push(Rejection {
                        candidate_id: candidate.id.clone(),
                        route_id: candidate.route_id.clone(),
                        world_x: candidate.world_x,
                        world_z: candidate.world_z,
                        reason: reason.into(),
                    });
                    continue;
                }
            };
            commit_candidate(
                index,
                accepted_candidate,
                candidate,
                &mut demand,
                &mut delivered,
                &mut buildings,
                &mut accepted,
                &mut total_floor_area,
                &mut accepted_lots,
                &mut stage_remaining,
                &mut stage_delivered,
                &mut stage_accepted,
                false,
                &mut stage_spill,
            );
            if demand_is_empty(&demand) {
                break;
            }
        }
    }
    let unmet_demand = demand.clone();
    let complete = demand_is_empty(&unmet_demand);
    let street_frontage = audit_street_frontage(&buildings);
    Ok(Some(Plan {
        format: "glyphweave.settlement-evolution-plan".into(),
        version: 1,
        status: if complete {
            "pass".into()
        } else {
            "incomplete".into()
        },
        demand: target_demand,
        delivered,
        unmet_demand,
        capacity: Capacity {
            candidate_lots,
            accepted_lots,
            rejected_lots: rejections.len(),
            floor_area_m2: total_floor_area,
        },
        buildings,
        rejections,
        community_profile: typology.as_ref().map(|value| value.profile.clone()),
        district_allocation,
        street_frontage,
    }))
}

fn allocate_district_demands(
    target: &Demand,
    shares: &[DistrictDemandShare],
) -> BTreeMap<String, Demand> {
    let mut allocated = BTreeMap::new();
    let mut households = 0_u32;
    let mut jobs = 0_u32;
    let mut retail = 0.0_f64;
    let mut services = 0.0_f64;
    for (index, entry) in shares.iter().enumerate() {
        let last = index + 1 == shares.len();
        let stage_households = if last {
            target.households.saturating_sub(households)
        } else {
            (f64::from(target.households) * entry.share).round() as u32
        }
        .min(target.households.saturating_sub(households));
        let stage_jobs = if last {
            target.jobs.saturating_sub(jobs)
        } else {
            (f64::from(target.jobs) * entry.share).round() as u32
        }
        .min(target.jobs.saturating_sub(jobs));
        let stage_retail = if last {
            (target.retail_floor_area_m2 - retail).max(0.0)
        } else {
            target.retail_floor_area_m2 * entry.share
        };
        let stage_services = if last {
            (target.service_budget - services).max(0.0)
        } else {
            target.service_budget * entry.share
        };
        households += stage_households;
        jobs += stage_jobs;
        retail += stage_retail;
        services += stage_services;
        allocated.insert(
            entry.stage.clone(),
            Demand {
                households: stage_households,
                jobs: stage_jobs,
                retail_floor_area_m2: stage_retail,
                service_budget: stage_services,
            },
        );
    }
    allocated
}

fn effective_district_shares(
    shares: &[DistrictDemandShare],
    focus_bias: Option<&urban_slice::FocusBias>,
) -> Vec<DistrictDemandShare> {
    let Some(focus_bias) = focus_bias else {
        return shares.to_vec();
    };
    let multipliers = &focus_bias.district_multipliers;
    let weighted = shares
        .iter()
        .map(|entry| {
            let multiplier = match entry.stage.as_str() {
                "old-core" => multipliers.old_core,
                "renewal" => multipliers.renewal,
                "new-growth" => multipliers.new_growth,
                "periphery" => multipliers.periphery,
                _ => 1.0,
            };
            (entry, f64::from(entry.share) * f64::from(multiplier))
        })
        .collect::<Vec<_>>();
    let total: f64 = weighted.iter().map(|(_, weight)| *weight).sum();
    if !total.is_finite() || total <= f64::EPSILON {
        return shares.to_vec();
    }
    let mut normalized = Vec::with_capacity(shares.len());
    let mut assigned = 0.0_f64;
    for (index, (entry, weight)) in weighted.iter().enumerate() {
        let share = if index + 1 == weighted.len() {
            (1.0 - assigned).max(0.0)
        } else {
            (weight / total).clamp(0.0, 1.0)
        };
        assigned += share;
        normalized.push(DistrictDemandShare {
            stage: entry.stage.clone(),
            share,
        });
    }
    normalized
}

#[allow(clippy::too_many_arguments)]
fn try_build_candidate(
    candidate: &Candidate,
    limit: &Demand,
    rules: &SelectionRules,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    max_pad_cut_m: f32,
    max_slope: f32,
    terrace_authorized: bool,
    roads: &[EntityInstance],
    buildings: &[Building],
) -> Result<AcceptedCandidate, &'static str> {
    let wants_retail = limit.retail_floor_area_m2 > 0.0 && candidate.pressure > 0.20;
    // Office demand is not a centre-only switch. Outlying and renewal
    // frontages can host clinics, workshops, studios and local employment;
    // center pressure still chooses the archetype, while the demand ledger
    // decides whether that capacity is consumed.
    let wants_jobs = limit.jobs > 0;
    let selection = building_archetypes::SelectionInput {
        community_profile: rules.community_profile.clone(),
        district_stage: candidate.district_stage.clone(),
        center_pressure: candidate.pressure,
        lot_width_m: candidate.width_m,
        lot_depth_m: candidate.depth_m,
        use_mix: parcel_use_mix(&candidate.id),
        needs_homes: limit.households > 0,
        needs_jobs: wants_jobs,
        needs_retail: wants_retail,
    };
    let archetype = building_archetypes::choose(&selection);
    let mut form = building_archetypes::form(&candidate.id, &selection, &archetype);
    form.floors = form.floors.min(rules.max_floors);
    let (household_grain, job_grain, retail_grain_m2, service_grain) =
        match candidate.district_stage.as_str() {
            // A parcel's programme is a sample of the district demand, not a
            // full building-size target. Smaller grains let a dense street
            // wall respond to the same household/job demand with varied
            // buildings instead of repeatedly cloning a handful of towers.
            "old-core" => (28, 100, 1_600.0, 0.60),
            "renewal" => (70, 240, 2_400.0, 0.80),
            "new-growth" => (120, 360, 3_500.0, 1.00),
            _ => (72, 220, 3_000.0, 0.90),
        };
    let mut rejected = "candidate_has_no_deliverable_program";
    // A planned envelope is a maximum, not a fixed asset size. In tight
    // corners, retaining the street wall by reducing depth/width is how real
    // infill responds to the block. Every variant still runs the complete road,
    // building and later access/grounding audits; this is not collision
    // tolerance relaxation.
    for scale in [1.0_f32, 0.85, 0.70, 0.55] {
        let width_m = (form.width_m * scale).max(6.0);
        let depth_m = (form.depth_m * scale).max(6.0);
        let footprint_m2 = width_m * depth_m * form.coverage_ratio;
        let floor_area =
            f64::from(footprint_m2) * f64::from(form.floors) * rules.floor_area_ratio;
        let household_capacity = (floor_area / rules.household_area_m2).floor() as u32;
        let job_capacity = (floor_area / rules.job_area_m2).floor() as u32;
        let retail_capacity = floor_area * rules.retail_share;
        let service_capacity = floor_area / 700.0;
        let spent = Demand {
            // One lot may provide a plausible mixed programme, but it may not
            // consume an entire district's demand. The grain cap is a
            // per-epoch planning coefficient, not a target building count:
            // empty or inaccessible lots still produce no building.
            households: household_capacity
                .min(limit.households)
                .min(household_grain),
            jobs: job_capacity.min(limit.jobs).min(job_grain),
            retail_floor_area_m2: retail_capacity
                .min(retail_grain_m2)
                .min(limit.retail_floor_area_m2),
            service_budget: service_capacity.min(service_grain).min(limit.service_budget),
        };
        if demand_is_empty(&spent) {
            continue;
        }
        let frontage_dx = f64::from(candidate.frontage_road_point[0])
            - f64::from(candidate.world_x);
        let frontage_dz = f64::from(candidate.frontage_road_point[1])
            - f64::from(candidate.world_z);
        let frontage_length = frontage_dx.hypot(frontage_dz);
        // A block narrowed by a second road is not necessarily unbuildable:
        // the frontage can keep its street edge while the building body moves
        // through the legal depth of the lot. Search the whole parcel section
        // instead of collapsing the entire lot to a six-metre shed.
        let depth_retreat = f64::from((candidate.depth_m - depth_m).max(0.0) * 0.5);
        let depth_search_m = f64::from((candidate.depth_m - depth_m).max(0.0));
        for placement in [0.0_f64, 0.25, 0.5, 0.75, 1.0] {
            let local_retreat = depth_retreat + placement * depth_search_m;
            let world_x = if frontage_length > 1e-6 {
                (f64::from(candidate.world_x)
                    + frontage_dx / frontage_length * local_retreat)
                    .round() as i32
            } else {
                candidate.world_x
            };
            let world_z = if frontage_length > 1e-6 {
                (f64::from(candidate.world_z)
                    + frontage_dz / frontage_length * local_retreat)
                    .round() as i32
            } else {
                candidate.world_z
            };
            // The placement search may move the body rearward inside the
            // parsed lot. Re-run the physical envelope checks at that exact
            // footprint: a valid frontage does not imply the rear of the lot
            // is dry, level or within the declared cut contract.
            let actual_slope =
                match lot_is_legal(scene, terrain, water, world_x, world_z, width_m, depth_m) {
                    Ok(slope) => slope,
                    Err(reason) => {
                        rejected = match reason.as_str() {
                            "intersects_water" => "intersects_water",
                            "outside_scene" => "outside_scene",
                            "missing_terrain" => "missing_terrain",
                            _ => "actual_footprint_outside_contract",
                        };
                        continue;
                    }
                };
            let actual_ceiling = lot_ground_ceiling_m(
                terrain,
                world_x,
                world_z,
                width_m,
                depth_m,
                candidate.rotation_y_deg,
            )
            .map(f64::from)
            .unwrap_or(f64::INFINITY);
            if actual_ceiling - candidate.road_design_m > f64::from(max_pad_cut_m) {
                rejected = "road_support_cut_exceeds_contract";
                continue;
            }
            if actual_slope > max_slope && !terrace_authorized {
                rejected = "slope_exceeds_contract";
                continue;
            }
            let actual_footprint = OrientedFootprint {
                center_x: f64::from(world_x),
                center_z: f64::from(world_z),
                width_m: f64::from(width_m),
                depth_m: f64::from(depth_m),
                rotation_y_deg: f64::from(candidate.rotation_y_deg),
            };
            if water.intersects_footprint(
                f64::from(world_x),
                f64::from(world_z),
                f64::from(width_m),
                f64::from(depth_m),
                f64::from(candidate.rotation_y_deg),
                WATER_SHORE_CLEARANCE_M,
            ) {
                rejected = "intersects_water";
                continue;
            }
            let (half_x, half_z) = actual_footprint.aabb_half_extents();
            let [road_x, road_z] = candidate.frontage_road_point;
            let approach_min_x = (actual_footprint.center_x - half_x)
                .min(f64::from(road_x) - SETTLEMENT_APPROACH_HALF_WIDTH_M);
            let approach_max_x = (actual_footprint.center_x + half_x)
                .max(f64::from(road_x) + SETTLEMENT_APPROACH_HALF_WIDTH_M);
            let approach_min_z = (actual_footprint.center_z - half_z)
                .min(f64::from(road_z) - SETTLEMENT_APPROACH_HALF_WIDTH_M);
            let approach_max_z = (actual_footprint.center_z + half_z)
                .max(f64::from(road_z) + SETTLEMENT_APPROACH_HALF_WIDTH_M);
            let crosses_x = (1..scene.width_m.div_ceil(STREAM_CHUNK_METERS))
                .map(|chunk| f64::from(scene.origin_x + (chunk * STREAM_CHUNK_METERS) as i32))
                .any(|seam| approach_min_x < seam && approach_max_x > seam);
            let crosses_z = (1..scene.depth_m.div_ceil(STREAM_CHUNK_METERS))
                .map(|chunk| f64::from(scene.origin_z + (chunk * STREAM_CHUNK_METERS) as i32))
                .any(|seam| approach_min_z < seam && approach_max_z > seam);
            if crosses_x || crosses_z {
                rejected = "straddles_chunk_seam_without_shared_support";
                continue;
            }
            let building = Building {
                id: candidate.id.clone(),
                kind: form.kind.clone(),
                route_id: candidate.route_id.clone(),
                side: candidate.side.clone(),
                world_x,
                world_z,
                width_m,
                depth_m,
                height_m: 4.0 + form.floors as f32 * 3.2,
                floors: form.floors,
                floor_area_m2: floor_area,
                households: spent.households,
                jobs: spent.jobs,
                retail_floor_area_m2: spent.retail_floor_area_m2,
                service_budget: spent.service_budget,
                access_distance_m: candidate.access_distance_m,
                center_distance_m: candidate.center_distance_m,
                community_profile: rules.community_profile.clone(),
                income_band: community_typology::income_band(
                    &rules.community_profile,
                    candidate.center_distance_m,
                )
                .into(),
                archetype_id: form.archetype_id.clone(),
                recommended_count: form.recommended_count,
                lot_area_m2: f64::from(candidate.width_m) * f64::from(candidate.depth_m),
                coverage_ratio: f64::from(form.coverage_ratio),
                frontage_road_point: Some(candidate.frontage_road_point),
                frontage_position_m: candidate.frontage_position_m,
                district_stage: candidate.district_stage.clone(),
                frontage_level_m: Some(candidate.road_design_m),
                // A generated building always owns a declared platform, even when
                // its required cut rounds to zero. Without an active platform a
                // neighbour's higher fill ring can raise terrain through its base.
                max_pad_cut_mm: Some(candidate.required_cut_mm.max(1)),
                rotation_y_deg: candidate.rotation_y_deg,
            };
            let footprint = actual_footprint;
            let conflicts_with_road = roads.iter().any(|road| {
                road.kind == "road"
                    && !road
                        .entity_id
                        .starts_with("generated.settlement-evolution.approach.")
                    && oriented_footprints_overlap(
                        &footprint,
                        &OrientedFootprint::from_entity(road),
                        0.5,
                    )
            });
            if conflicts_with_road {
                rejected = "building_conflicts_with_road_support";
                continue;
            }
            let conflicts_with_building = buildings.iter().any(|other| {
                oriented_footprints_overlap(
                    &footprint,
                    &OrientedFootprint {
                        center_x: f64::from(other.world_x),
                        center_z: f64::from(other.world_z),
                        width_m: f64::from(other.width_m),
                        depth_m: f64::from(other.depth_m),
                        rotation_y_deg: f64::from(other.rotation_y_deg),
                    },
                    0.25,
                )
            });
            if conflicts_with_building {
                rejected = "building_conflicts_with_prior_building";
                continue;
            }
            return Ok(AcceptedCandidate {
                building,
                spent,
                floor_area_m2: floor_area,
            });
        }
    }
    Err(rejected)
}

fn parcel_use_mix(id: &str) -> f64 {
    let hash = blake3::hash(format!("{id}:building-use-mix").as_bytes());
    f64::from(u32::from_le_bytes(
        hash.as_bytes()[..4]
            .try_into()
            .expect("four-byte use hash"),
    )) / f64::from(u32::MAX)
}

#[allow(clippy::too_many_arguments)]
fn commit_candidate(
    index: usize,
    accepted_candidate: AcceptedCandidate,
    candidate: &Candidate,
    demand: &mut Demand,
    delivered: &mut Demand,
    buildings: &mut Vec<Building>,
    accepted: &mut [bool],
    total_floor_area: &mut f64,
    accepted_lots: &mut usize,
    stage_remaining: &mut BTreeMap<String, Demand>,
    stage_delivered: &mut BTreeMap<String, Demand>,
    stage_accepted: &mut BTreeMap<String, usize>,
    spill: bool,
    stage_spill: &mut BTreeMap<String, usize>,
) {
    subtract_demand(demand, &accepted_candidate.spent);
    if let Some(limit) = stage_remaining.get_mut(&candidate.district_stage) {
        subtract_demand(limit, &accepted_candidate.spent);
    }
    let stage_total = stage_delivered
        .entry(candidate.district_stage.clone())
        .or_default();
    stage_total.households += accepted_candidate.spent.households;
    stage_total.jobs += accepted_candidate.spent.jobs;
    stage_total.retail_floor_area_m2 += accepted_candidate.spent.retail_floor_area_m2;
    stage_total.service_budget += accepted_candidate.spent.service_budget;
    delivered.households += accepted_candidate.spent.households;
    delivered.jobs += accepted_candidate.spent.jobs;
    delivered.retail_floor_area_m2 += accepted_candidate.spent.retail_floor_area_m2;
    delivered.service_budget += accepted_candidate.spent.service_budget;
    *total_floor_area += accepted_candidate.floor_area_m2;
    *accepted_lots += 1;
    accepted[index] = true;
    buildings.push(accepted_candidate.building);
    *stage_accepted
        .entry(candidate.district_stage.clone())
        .or_default() += 1;
    if spill {
        *stage_spill
            .entry(candidate.district_stage.clone())
            .or_default() += 1;
    }
}

fn subtract_demand(remaining: &mut Demand, spent: &Demand) {
    remaining.households = remaining.households.saturating_sub(spent.households);
    remaining.jobs = remaining.jobs.saturating_sub(spent.jobs);
    remaining.retail_floor_area_m2 =
        (remaining.retail_floor_area_m2 - spent.retail_floor_area_m2).max(0.0);
    remaining.service_budget = (remaining.service_budget - spent.service_budget).max(0.0);
}

fn route_center(plan: &RoutePlan) -> (f64, f64) {
    if plan.anchors.is_empty() {
        return (0.0, 0.0);
    }
    let count = plan.anchors.len() as f64;
    (
        plan.anchors
            .iter()
            .map(|anchor| f64::from(anchor.x))
            .sum::<f64>()
            / count,
        plan.anchors
            .iter()
            .map(|anchor| f64::from(anchor.z))
            .sum::<f64>()
            / count,
    )
}

fn district_stage(style: &Value, center_distance_m: f64) -> &'static str {
    let radius = |field: &str, fallback: f64| {
        style
            .get("epochProgram")
            .and_then(|program| program.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(fallback)
    };
    let old_core = radius("oldCoreRadiusM", 130.0);
    let expansion = radius("expansionRadiusM", 210.0);
    let modern = radius("modernRadiusM", 340.0);
    if center_distance_m <= old_core {
        "old-core"
    } else if center_distance_m <= expansion {
        "renewal"
    } else if center_distance_m <= modern {
        "new-growth"
    } else {
        "periphery"
    }
}

fn staged_lot_size(stage: &str, width_m: f32, depth_m: f32) -> (f32, f32) {
    let (width_scale, depth_scale) = match stage {
        "old-core" => (0.72, 1.25),
        "renewal" => (0.95, 1.15),
        "new-growth" => (1.20, 1.05),
        _ => (1.30, 0.95),
    };
    (
        (width_m * width_scale).clamp(if stage == "old-core" { 7.0 } else { 8.0 }, 80.0),
        (depth_m * depth_scale).clamp(8.0, 80.0),
    )
}

/// Frontage module count is a deterministic morphology coefficient. It is not
/// a building-count target: demand and the overlap gate still decide whether a
/// parcel builds. Wider assemblies are concentrated in newer districts, while
/// the historic core keeps both one-module infill and two-module block faces.
fn frontage_module_count(route_id: &str, position_m: f64, stage: &str) -> u8 {
    let hash = blake3::hash(
        format!(
            "{route_id}:frontage-module:{:.3}:{stage}",
            position_m.round()
        )
        .as_bytes(),
    );
    let unit = f64::from(u32::from_le_bytes(
        hash.as_bytes()[..4]
            .try_into()
            .expect("four-byte module hash"),
    )) / f64::from(u32::MAX);
    // Historic streets preserve small shop-house frontages and occasionally
    // join two neighbours. Renewal and new-town blocks use larger assemblies.
    // The selected module count advances the parcel cursor by the same number
    // of frontage units, so a wider building never overlaps its neighbour.
    match stage {
        // The historic core keeps its narrow shop-house rhythm. Its variation
        // comes from depth, roof form and use rather than erasing adjoining
        // frontages into a uniform large block.
        "old-core" => 1,
        "renewal" => {
            if unit < 0.05 {
                2
            } else {
                1
            }
        }
        "new-growth" => {
            if unit < 0.03 {
                3
            } else if unit < 0.18 {
                2
            } else {
                1
            }
        }
        _ => {
            if unit < 0.10 {
                3
            } else if unit < 0.60 {
                2
            } else {
                1
            }
        }
    }
}

fn demand_is_empty(demand: &Demand) -> bool {
    demand.households == 0
        && demand.jobs == 0
        && demand.retail_floor_area_m2 <= 1e-6
        && demand.service_budget <= 1e-6
}

fn old_core_has_frontage_neighbor(candidates: &[Candidate], index: usize) -> bool {
    let candidate = &candidates[index];
    candidates.iter().enumerate().any(|(other_index, other)| {
        other_index != index
            && other.route_id == candidate.route_id
            && other.side == candidate.side
            && other.district_stage == "old-core"
            && (other.frontage_position_m - candidate.frontage_position_m).abs()
                <= f64::from(candidate.width_m.max(other.width_m)) + 8.0
    })
}

fn audit_street_frontage(buildings: &[Building]) -> StreetFrontageAudit {
    let mut groups = BTreeMap::<(String, String, String), Vec<(f64, f64)>>::new();
    for building in buildings {
        let half_width = f64::from(building.width_m) * 0.5;
        let start = building.frontage_position_m - half_width;
        let end = building.frontage_position_m + half_width;
        groups
            .entry((
                building.route_id.clone(),
                building.side.clone(),
                building.district_stage.clone(),
            ))
            .or_default()
            .push((start.min(end), start.max(end)));
    }
    let mut runs = Vec::with_capacity(groups.len());
    for ((route_id, side, district_stage), mut intervals) in groups {
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut cluster_start = 0;
        while cluster_start < intervals.len() {
            let mut cluster_end = cluster_start + 1;
            while cluster_end < intervals.len()
                && intervals[cluster_end].0 <= intervals[cluster_end - 1].1 + 5.0
            {
                cluster_end += 1;
            }
            let cluster = &intervals[cluster_start..cluster_end];
            let (span_start, span_end) = (
                cluster.first().map_or(0.0, |interval| interval.0),
                cluster.last().map_or(0.0, |interval| interval.1),
            );
            let frontage_span_m = (span_end - span_start).max(0.0);
            let mut merged_end = span_start;
            let mut facade_length_m = 0.0;
            let mut longest_gap_m = 0.0_f64;
            for (start, end) in cluster {
                if *start > merged_end {
                    longest_gap_m = longest_gap_m.max(*start - merged_end);
                }
                let covered_start = (*start).max(merged_end);
                if *end > covered_start {
                    facade_length_m += *end - covered_start;
                    merged_end = *end;
                }
            }
            let uncovered_length_m = (frontage_span_m - facade_length_m).max(0.0);
            let coverage_ratio = if frontage_span_m > 1e-6 {
                (facade_length_m / frontage_span_m).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let building_count = buildings
                .iter()
                .filter(|building| {
                    building.route_id == route_id
                        && building.side == side
                        && building.district_stage == district_stage
                        && building.frontage_position_m >= span_start - 0.5
                        && building.frontage_position_m <= span_end + 0.5
                })
                .count();
            runs.push(StreetFrontageRun {
                route_id: route_id.clone(),
                side: side.clone(),
                district_stage: district_stage.clone(),
                buildings: building_count,
                frontage_span_m,
                facade_length_m,
                uncovered_length_m,
                longest_gap_m,
                coverage_ratio,
                continuous: building_count >= 2 && coverage_ratio >= 0.62 && longest_gap_m <= 5.0,
            });
            cluster_start = cluster_end;
        }
    }
    let old_core: Vec<&StreetFrontageRun> = runs
        .iter()
        .filter(|run| run.district_stage == "old-core")
        .collect();
    let old_core_continuous_runs = old_core.iter().filter(|run| run.continuous).count();
    let old_core_min_coverage_ratio = old_core
        .iter()
        .map(|run| run.coverage_ratio)
        .fold(1.0_f64, f64::min);
    let status = if old_core.is_empty() {
        "not_applicable"
    } else if old_core_continuous_runs == old_core.len() {
        "pass"
    } else {
        "warn"
    };
    StreetFrontageAudit {
        status: status.into(),
        model: "street-frontage-runs-v1".into(),
        old_core_run_count: old_core.len(),
        old_core_continuous_runs,
        old_core_min_coverage_ratio,
        runs,
    }
}

fn lot_is_legal(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    x: i32,
    z: i32,
    width: f32,
    depth: f32,
) -> Result<f32, String> {
    let hw = width as f64 * 0.5;
    let hd = depth as f64 * 0.5;
    if f64::from(x) - hw < f64::from(scene.origin_x)
        || f64::from(z) - hd < f64::from(scene.origin_z)
        || f64::from(x) + hw >= f64::from(scene.origin_x + scene.width_m as i32)
        || f64::from(z) + hd >= f64::from(scene.origin_z + scene.depth_m as i32)
    {
        return Err("outside_scene".into());
    }
    let mut lowest = f32::INFINITY;
    let mut highest = f32::NEG_INFINITY;
    for [sx, sz] in [[-hw, -hd], [hw, -hd], [-hw, hd], [hw, hd], [0.0, 0.0]] {
        let px = x + sx.round() as i32;
        let pz = z + sz.round() as i32;
        if water.contains_point(f64::from(px), f64::from(pz)) {
            return Err("intersects_water".into());
        }
        let height = terrain
            .height(px, pz)
            .ok_or_else(|| "missing_terrain".to_string())?;
        lowest = lowest.min(height);
        highest = highest.max(height);
    }
    let slope = (highest - lowest) / width.min(depth).max(1.0);
    Ok(slope)
}

fn lot_ground_ceiling_m(
    terrain: &BakedTerrain,
    x: i32,
    z: i32,
    width: f32,
    depth: f32,
    rotation_y_deg: f32,
) -> Option<f32> {
    let mut ceiling = f32::NEG_INFINITY;
    let footprint = OrientedFootprint {
        center_x: f64::from(x),
        center_z: f64::from(z),
        width_m: f64::from(width),
        depth_m: f64::from(depth),
        rotation_y_deg: f64::from(rotation_y_deg),
    };
    for (sample_x, sample_z) in footprint.grid_cells() {
        ceiling = ceiling.max(terrain.height(sample_x, sample_z)?);
    }
    ceiling.is_finite().then_some(ceiling)
}

pub fn materialize(
    plan: &Plan,
    terrain: &BakedTerrain,
    epoch: Option<&epoch_evolution::Plan>,
) -> Vec<EntityInstance> {
    let epoch_by_building = epoch
        .map(|plan| {
            plan.assigned_buildings
                .iter()
                .map(|assigned| (assigned.building_id.as_str(), assigned))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut target_grounds = plan
        .buildings
        .iter()
        .map(|building| {
            let terrain_support = f64::from(building_support_height_m(building, terrain));
            building
                .frontage_level_m
                .map_or(terrain_support, |frontage| frontage.max(terrain_support))
        })
        .collect::<Vec<_>>();
    // Building support carves are axis-aligned envelopes around the (possibly
    // rotated) footprint. Where two envelopes overlap, the higher declared pad
    // must level the whole connected tier; otherwise its fill ring raises
    // terrain through the lower neighbour's base even though the two building
    // OBBs are legally separated.
    let pads = plan
        .buildings
        .iter()
        .map(|building| {
            let (half_w, half_d) = super::spatial_half_extents(
                building.width_m,
                building.depth_m,
                building.rotation_y_deg,
            );
            (
                f64::from(building.world_x),
                f64::from(building.world_z),
                half_w + 2.0,
                half_d + 2.0,
            )
        })
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for left in 0..pads.len() {
            for right in left + 1..pads.len() {
                let a = pads[left];
                let b = pads[right];
                if (a.0 - b.0).abs() >= a.2 + b.2 || (a.1 - b.1).abs() >= a.3 + b.3 {
                    continue;
                }
                let level = target_grounds[left].max(target_grounds[right]);
                // A shared platform may bridge a terrace step. The bound is
                // the same 12m allowance used for a single pad cut: beyond it
                // the pair needs separate retaining structures, not one level.
                // A 6m cap let 6-7m street-to-platform steps slip through and
                // left the final audit with terrain through a building base.
                if level - target_grounds[left].min(target_grounds[right]) > 9.0 {
                    continue;
                }
                if (target_grounds[left] - level).abs() > 1e-9
                    || (target_grounds[right] - level).abs() > 1e-9
                {
                    target_grounds[left] = level;
                    target_grounds[right] = level;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    plan.buildings
        .iter()
        .enumerate()
        .map(|(building_index, building)| {
            let epoch_assignment = epoch_by_building.get(building.id.as_str()).copied();
            // A settlement building may sit on a terrace cut to the street, but
            // the natural high side of its own footprint is still an authoritative
            // lower bound: publishing the building below that contour buries one
            // facade and manufactures a false entrance step. Take the higher of
            // the two authored surfaces so the foundation meets both the street
            // and the uphill terrain; the strict access audit still decides
            // whether the resulting relationship is walkable.
            let target_ground = target_grounds[building_index];
            let world_y = target_ground.floor() as i32;
            let bottom_offset = (target_ground - f64::from(world_y)) as f32;
            let mut anchors = Vec::new();
            let mut frontage_road_point = None;
            let mut stair_connection = None;
            if let Some([road_x, road_z]) = building.frontage_road_point {
                if let Some((anchor_x, anchor_z, direction)) = facade_anchor(
                    building.world_x,
                    building.world_z,
                    building.width_m,
                    building.depth_m,
                    building.rotation_y_deg,
                    f64::from(road_x),
                    f64::from(road_z),
                ) {
                    frontage_road_point = Some(match direction {
                        "east" => [anchor_x + 16, anchor_z],
                        "west" => [anchor_x - 16, anchor_z],
                        "south" => [anchor_x, anchor_z + 16],
                        _ => [anchor_x, anchor_z - 16],
                    });
                    anchors.push(super::SpatialAnchor {
                        id: "front".into(),
                        world_x: anchor_x,
                        world_z: anchor_z,
                        direction: direction.to_owned(),
                        target: Some("road".into()),
                    });
                    // Dense rows can leave the authored front door hard against
                    // the next building even though a genuine side alley exists.
                    // Publish both side-wall candidates; AccessGraph accepts one
                    // only if its complete walk can reach the road. The visual
                    // street facade and the front anchor remain authoritative.
                    let (side_sin, side_cos) =
                        f64::from(building.rotation_y_deg).to_radians().sin_cos();
                    let side_half = (f64::from(building.width_m) * 0.5 - 0.5).max(0.5);
                    for (side_id, sign) in [("side-left", -1.0_f64), ("side-right", 1.0)] {
                        let side_x = (f64::from(building.world_x)
                            + side_cos * sign * side_half)
                            .round() as i32;
                        let side_z = (f64::from(building.world_z)
                            - side_sin * sign * side_half)
                            .round() as i32;
                        let side_direction = if side_cos.abs() >= side_sin.abs() {
                            if side_cos * sign >= 0.0 {
                                "east"
                            } else {
                                "west"
                            }
                        } else if -side_sin * sign >= 0.0 {
                            "south"
                        } else {
                            "north"
                        };
                        anchors.push(super::SpatialAnchor {
                            id: side_id.into(),
                            world_x: side_x,
                            world_z: side_z,
                            direction: side_direction.into(),
                            target: Some("road".into()),
                        });
                    }
                    // The supporting road network can place a higher street or
                    // platform behind a frontage lot. Publish the exact rear
                    // facade opposite the authored door as a real service
                    // entrance; the front anchor remains first for rendering and
                    // the remaining side options stay available to AccessGraph.
                    let rear_direction = match direction {
                        "east" => "west",
                        "west" => "east",
                        "south" => "north",
                        _ => "south",
                    };
                    anchors.push(super::SpatialAnchor {
                        id: "rear".into(),
                        world_x: building.world_x * 2 - anchor_x,
                        world_z: building.world_z * 2 - anchor_z,
                        direction: rear_direction.into(),
                        target: Some("road".into()),
                    });
                    let [out_x, out_z] = outward_delta(direction);
                    let landing = [anchor_x + out_x * 2, anchor_z + out_z * 2];
                    if let Some(landing_m) = terrain.height(landing[0], landing[1]) {
                        let delta_m = f64::from(landing_m) - target_ground;
                        if (0.30..=2.40).contains(&delta_m.abs()) {
                            let low_m = target_ground.min(f64::from(landing_m));
                            let high_m = target_ground.max(f64::from(landing_m));
                            stair_connection = Some(super::StairConnection {
                                version: 1,
                                stair_id: format!(
                                    "generated.settlement-evolution.stairs.{}",
                                    building.id
                                ),
                                from: [anchor_x, anchor_z],
                                to: landing,
                                from_mm: (target_ground * 1000.0).round() as i32,
                                to_mm: (f64::from(landing_m) * 1000.0).round() as i32,
                                low_mm: (low_m * 1000.0).round() as i32,
                                high_mm: (high_m * 1000.0).round() as i32,
                                step_count: ((delta_m.abs() / 0.18).ceil() as u32).max(1),
                                width_m: 2.4,
                            });
                        }
                    }
                }
            }
            EntityInstance {
                entity_id: format!("generated.settlement-evolution.{}", building.id),
                asset_id: format!("building.{}", building.kind),
                kind: building.kind.clone(),
                world_x: building.world_x,
                world_z: building.world_z,
                world_y,
                scale: 1.0,
                width_m: building.width_m,
                depth_m: building.depth_m,
                height_m: building.height_m,
                rotation_y_deg: building.rotation_y_deg,
                grounding: super::GroundingSpec {
                    bottom_offset_m: bottom_offset,
                    bounded_cut_mm: building.max_pad_cut_mm.filter(|cut| *cut > 0),
                    district_id: Some(format!("district.{}", building.district_stage)),
                    construction_cohort: Some(building.district_stage.clone()),
                    building_operation: epoch_assignment.map(|assigned| assigned.operation.clone()),
                    building_parent_id: epoch_assignment
                        .and_then(|assigned| assigned.parent_id.clone()),
                    frontage_road_point,
                    stair_connection,
                    ..super::GroundingSpec::default()
                },
                anchors,
                bounds: None,
            }
        })
        .collect()
}

pub fn materialize_stairs(
    buildings: &[EntityInstance],
    terrain: &BakedTerrain,
) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for building in buildings {
        if let Some(primary) = building.grounding.stair_connection.as_ref() {
            entities.extend(stair_entities(primary));
        }
        for anchor in &building.anchors {
            let already_published = building
                .grounding
                .stair_connection
                .as_ref()
                .is_some_and(|stairs| stairs.from == [anchor.world_x, anchor.world_z]);
            if already_published {
                continue;
            }
            let [out_x, out_z] = outward_delta(&anchor.direction);
            let landing = [anchor.world_x + out_x * 2, anchor.world_z + out_z * 2];
            let near_seam = |value: i32| {
                let rem = value.rem_euclid(STREAM_CHUNK_METERS as i32);
                rem < 4 || rem > STREAM_CHUNK_METERS as i32 - 4
            };
            if near_seam(anchor.world_x)
                || near_seam(anchor.world_z)
                || near_seam(landing[0])
                || near_seam(landing[1])
            {
                continue;
            }
            let Some(landing_m) = terrain.height(landing[0], landing[1]) else {
                continue;
            };
            let start_m = f64::from(building.ground_height_m());
            let delta_m = f64::from(landing_m) - start_m;
            if !(0.30..=2.40).contains(&delta_m.abs()) {
                continue;
            }
            let low_m = start_m.min(f64::from(landing_m));
            let high_m = start_m.max(f64::from(landing_m));
            entities.extend(stair_entities(&super::StairConnection {
                version: 1,
                stair_id: format!("{}.stairs.anchor.{}", building.entity_id, anchor.id),
                from: [anchor.world_x, anchor.world_z],
                to: landing,
                from_mm: (start_m * 1000.0).round() as i32,
                to_mm: (f64::from(landing_m) * 1000.0).round() as i32,
                low_mm: (low_m * 1000.0).round() as i32,
                high_mm: (high_m * 1000.0).round() as i32,
                step_count: ((delta_m.abs() / 0.18).ceil() as u32).max(1),
                width_m: 2.0,
            }));
        }
    }
    entities
}

pub fn attach_frontage_stairs(
    building: &mut EntityInstance,
    terrain: &BakedTerrain,
    to: [i32; 2],
) -> Option<Vec<EntityInstance>> {
    let anchor = building.anchors.first()?;
    if let Some(existing) = &building.grounding.stair_connection {
        if existing.to == to {
            return None;
        }
    }
    // A promoted frontage stair starts at the building's declared door
    // threshold, not at the baked terrain beside it. The terrain at the facade
    // may already have been lowered by the adjoining road carve; using that
    // number erased the very step the stair was supposed to resolve.
    let start_height_m = f64::from(building.ground_height_m());
    let end_height_m = f64::from(terrain.height(to[0], to[1])?);
    let delta_m = (end_height_m - start_height_m).abs();
    let run_m = f64::from(to[0] - anchor.world_x)
        .hypot(f64::from(to[1] - anchor.world_z));
    // Long stairs are published as <=22m flights. Each flight must still fit
    // the 8.5m stair contract; the whole connection may therefore step through
    // more than one legal flight on a steep block.
    let flights = (run_m / 22.0).ceil().max(1.0) as f64;
    let max_total_rise_m = 8.0 * flights;
    if !(0.10..=max_total_rise_m).contains(&delta_m) {
        return None;
    }
    let low_m = start_height_m.min(end_height_m);
    let high_m = start_height_m.max(end_height_m);
    let (stair_namespace, building_suffix) = if let Some(suffix) = building
        .entity_id
        .strip_prefix("generated.settlement-evolution.")
    {
        ("generated.settlement-evolution", suffix)
    } else if let Some(suffix) = building
        .entity_id
        .strip_prefix("generated.market-catchment.")
    {
        ("generated.market-catchment", suffix)
    } else {
        return None;
    };
    let stairs = super::StairConnection {
        version: 1,
        stair_id: format!("{stair_namespace}.stairs.{building_suffix}.long"),
        from: [anchor.world_x, anchor.world_z],
        to,
        from_mm: (start_height_m * 1000.0).round() as i32,
        to_mm: (end_height_m * 1000.0).round() as i32,
        low_mm: (low_m * 1000.0).round() as i32,
        high_mm: (high_m * 1000.0).round() as i32,
        step_count: ((delta_m / 0.18).ceil() as u32).max(1),
        width_m: 2.4,
    };
    building.grounding.stair_connection = Some(stairs.clone());
    Some(stair_entities(&stairs))
}

fn stair_entity(stairs: &super::StairConnection) -> EntityInstance {
    let dx = stairs.to[0] - stairs.from[0];
    let dz = stairs.to[1] - stairs.from[1];
    let run_m = f64::from(dx).hypot(f64::from(dz)) as f32;
    let world_y = stairs.low_mm.div_euclid(1000);
    let bottom_offset_m = stairs.low_mm.rem_euclid(1000) as f32 / 1000.0;
    EntityInstance {
        entity_id: stairs.stair_id.clone(),
        asset_id: "architecture.stairs".into(),
        kind: "stairs".into(),
        world_x: (stairs.from[0] + stairs.to[0]) / 2,
        world_z: (stairs.from[1] + stairs.to[1]) / 2,
        world_y,
        scale: 1.0,
        width_m: run_m + 0.4,
        depth_m: stairs.width_m,
        height_m: (stairs.high_mm - stairs.low_mm) as f32 / 1000.0 + 0.2,
        rotation_y_deg: f64::from(-dz).atan2(f64::from(dx)).to_degrees() as f32,
        grounding: super::GroundingSpec {
            bottom_offset_m,
            stair_connection: Some(stairs.clone()),
            ..super::GroundingSpec::default()
        },
        anchors: Vec::new(),
        bounds: None,
    }
}

fn stair_entities(stairs: &super::StairConnection) -> Vec<EntityInstance> {
    let dx = stairs.to[0] - stairs.from[0];
    let dz = stairs.to[1] - stairs.from[1];
    let run_m = f64::from(dx).hypot(f64::from(dz));
    if run_m + 0.4 <= 24.0 {
        return vec![stair_entity(stairs)];
    }
    let segment_run_m = 22.0;
    let segment_count = (run_m / segment_run_m).ceil().max(2.0) as usize;
    let mut segments = Vec::with_capacity(segment_count);
    for index in 0..segment_count {
        let start_t = index as f64 / segment_count as f64;
        let end_t = (index + 1) as f64 / segment_count as f64;
        let from = [
            (f64::from(stairs.from[0]) + f64::from(dx) * start_t).round() as i32,
            (f64::from(stairs.from[1]) + f64::from(dz) * start_t).round() as i32,
        ];
        let to = [
            (f64::from(stairs.from[0]) + f64::from(dx) * end_t).round() as i32,
            (f64::from(stairs.from[1]) + f64::from(dz) * end_t).round() as i32,
        ];
        let from_mm = (f64::from(stairs.from_mm)
            + f64::from(stairs.to_mm - stairs.from_mm) * start_t)
            .round() as i32;
        let to_mm = (f64::from(stairs.from_mm) + f64::from(stairs.to_mm - stairs.from_mm) * end_t)
            .round() as i32;
        let low_mm = from_mm.min(to_mm);
        let high_mm = from_mm.max(to_mm);
        let segment = super::StairConnection {
            version: stairs.version,
            stair_id: format!("{}.part-{index}", stairs.stair_id),
            from,
            to,
            from_mm,
            to_mm,
            low_mm,
            high_mm,
            step_count: stairs.step_count.div_ceil(segment_count as u32).max(1),
            width_m: stairs.width_m,
        };
        segments.push(stair_entity(&segment));
    }
    segments
}

/// Short, explicit ground-level approaches from each planned building facade to
/// the road station that generated the lot. They are road-class static geometry
/// so access search, terrain carving and rendering all consume the same object.
pub fn materialize_approaches(plan: &Plan, terrain: &BakedTerrain) -> Vec<EntityInstance> {
    plan.buildings
        .iter()
        .filter_map(|building| {
            let [road_x, road_z] = building.frontage_road_point?;
            let dx = f64::from(road_x - building.world_x);
            let dz = f64::from(road_z - building.world_z);
            let length = dx.hypot(dz);
            if length < 0.5 {
                return None;
            }
            let nx = dx / length;
            let nz = dz / length;
            let reach = (f64::from(building.width_m) * 0.5 / nx.abs().max(1e-9))
                .min(f64::from(building.depth_m) * 0.5 / nz.abs().max(1e-9));
            let edge_x = f64::from(building.world_x) + nx * reach;
            let edge_z = f64::from(building.world_z) + nz * reach;
            let center_x = (edge_x + f64::from(road_x)) * 0.5;
            let center_z = (edge_z + f64::from(road_z)) * 0.5;
            let approach_length = (f64::from(road_x) - edge_x).hypot(f64::from(road_z) - edge_z);
            if approach_length < 0.5 {
                return None;
            }
            let terrain_support = f64::from(building_support_height_m(building, terrain));
            let target_ground = building
                .frontage_level_m
                .map_or(terrain_support, |frontage| frontage.max(terrain_support));
            if let Some(road_level) = building.frontage_level_m {
                if (target_ground - road_level).abs() > 2.4 {
                    // This threshold needs stairs, not a long flat road that
                    // imports the uphill pad elevation into the street.
                    return None;
                }
            }
            let world_y = target_ground.floor() as i32;
            let bottom_offset = (target_ground - f64::from(world_y)) as f32;
            Some(EntityInstance {
                entity_id: format!("generated.settlement-evolution.approach.{}", building.id),
                asset_id: "road.generated".into(),
                kind: "road".into(),
                world_x: center_x.round() as i32,
                world_z: center_z.round() as i32,
                world_y,
                scale: 1.0,
                width_m: (approach_length + 4.0) as f32,
                // Access spurs are pedestrian-scale connectors, not twelve-
                // metre carriageways. The previous width extended across a
                // streaming seam even when the building itself stayed clear.
                depth_m: 4.0,
                height_m: 0.2,
                rotation_y_deg: (-dz).atan2(dx).to_degrees() as f32,
                grounding: super::GroundingSpec {
                    bottom_offset_m: bottom_offset,
                    ..super::GroundingSpec::default()
                },
                anchors: Vec::new(),
                bounds: None,
            })
        })
        .collect()
}

fn building_support_height_m(building: &Building, terrain: &BakedTerrain) -> f32 {
    let mut highest = f32::NEG_INFINITY;
    let footprint = OrientedFootprint {
        center_x: f64::from(building.world_x),
        center_z: f64::from(building.world_z),
        width_m: f64::from(building.width_m),
        depth_m: f64::from(building.depth_m),
        rotation_y_deg: f64::from(building.rotation_y_deg),
    };
    for (x, z) in footprint.grid_cells() {
        if let Some(height) = terrain.height(x, z) {
            highest = highest.max(height);
        }
    }
    highest
}

fn facade_anchor(
    world_x: i32,
    world_z: i32,
    width_m: f32,
    depth_m: f32,
    rotation_y_deg: f32,
    road_x: f64,
    road_z: f64,
) -> Option<(i32, i32, &'static str)> {
    let dx = road_x - f64::from(world_x);
    let dz = road_z - f64::from(world_z);
    let length = dx.hypot(dz);
    if length <= 1e-9 {
        return None;
    }
    let (sin, cos) = f64::from(rotation_y_deg).to_radians().sin_cos();
    let local_x = dx * cos - dz * sin;
    let local_z = dx * sin + dz * cos;
    let (out_x, out_z, half) = if local_z.abs() >= local_x.abs() {
        let sign = local_z.signum();
        (sin * sign, cos * sign, f64::from(depth_m) * 0.5)
    } else {
        let sign = local_x.signum();
        (cos * sign, -sin * sign, f64::from(width_m) * 0.5)
    };
    let anchor_x = (f64::from(world_x) + out_x * (half - 0.5)).round() as i32;
    let anchor_z = (f64::from(world_z) + out_z * (half - 0.5)).round() as i32;
    let direction = if out_x.abs() >= out_z.abs() {
        if out_x >= 0.0 {
            "east"
        } else {
            "west"
        }
    } else if out_z >= 0.0 {
        "south"
    } else {
        "north"
    };
    Some((anchor_x, anchor_z, direction))
}

fn outward_delta(direction: &str) -> [i32; 2] {
    match direction {
        "east" => [1, 0],
        "west" => [-1, 0],
        "south" => [0, 1],
        _ => [0, -1],
    }
}

fn nearby_road_surface_m(
    roads: &[EntityInstance],
    world_x: i32,
    world_z: i32,
    width_m: f32,
    depth_m: f32,
    rotation_y_deg: f32,
) -> f64 {
    let footprint = OrientedFootprint {
        center_x: f64::from(world_x),
        center_z: f64::from(world_z),
        width_m: f64::from(width_m),
        depth_m: f64::from(depth_m),
        rotation_y_deg: f64::from(rotation_y_deg),
    };
    let mut highest = f64::NEG_INFINITY;
    for road in roads {
        let blend = f64::from(road.grounding.earthwork_blend_m.unwrap_or(0.0)).max(0.0);
        let influence = OrientedFootprint {
            center_x: f64::from(road.world_x),
            center_z: f64::from(road.world_z),
            // Match the planner's `road + 6m` graded transport shoulder.
            // Without this envelope, a perpendicular street 15m away can
            // raise finished terrain through a building that never touches
            // the carriageway OBB.
            width_m: f64::from(road.width_m) + blend * 2.0 + 12.0,
            depth_m: f64::from(road.depth_m) + blend * 2.0 + 12.0,
            rotation_y_deg: f64::from(road.rotation_y_deg),
        };
        if oriented_footprints_overlap(&footprint, &influence, 0.5) {
            highest = highest.max(f64::from(road.ground_height_m()));
        }
    }
    highest
}

pub fn require_complete(plan: &Plan) -> Result<(), TerrainError> {
    if plan.status == "pass" {
        Ok(())
    } else {
        Err(TerrainError(format!(
            "settlement demand unmet: {} households, {} jobs, {:.1}m2 retail",
            plan.unmet_demand.households,
            plan.unmet_demand.jobs,
            plan.unmet_demand.retail_floor_area_m2
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;
    use crate::worldgen::route_evolution::{Anchor, Plan as RoutePlan, Route, RoutePoint};
    use crate::worldgen::{water_geometry, WaterKind};

    fn fixture() -> (SceneSpec, BakedTerrain, WaterGeometry) {
        let scene = SceneSpec {
            scene_id: "settlement".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let payload =
            encode_millimetres(512, 512, (0..512).flat_map(|_| (0..512).map(|_| 10.0))).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain.insert_chunk(0, 0, 512, 512, &payload).unwrap();
        let style = serde_json::json!({"water": {"levelM": 0}});
        let water = water_geometry(WaterKind::None, &[], &scene, &style);
        (scene, terrain, water)
    }

    fn route_plan() -> RoutePlan {
        let polyline = (0..20)
            .map(|index| RoutePoint {
                x: 40 + index * 20,
                z: 256,
                natural_height_m: 10.0,
                design_height_m: 10.0,
                water: false,
            })
            .collect();
        RoutePlan {
            format: "glyphweave.route-evolution-plan".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.55,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor {
                    id: "a".into(),
                    x: 40,
                    z: 256,
                    role: "test".into(),
                },
                Anchor {
                    id: "b".into(),
                    x: 420,
                    z: 256,
                    role: "test".into(),
                },
            ],
            candidate_nodes: Vec::new(),
            routes: vec![Route {
                id: "main".into(),
                from: "a".into(),
                to: "b".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline,
                length_m: 380.0,
                direct_length_m: 380.0,
                curve_deviation_m: 0.0,
                maximum_grade: 0.0,
                maximum_fill_m: 0.0,
                maximum_terrain_fill_m: 0.0,
                fill_volume_m3: 0.0,
                bridge_spans: Vec::new(),
                failure: None,
            }],
            failures: Vec::new(),
        }
    }

    fn diagonal_route_plan() -> RoutePlan {
        let polyline = (0..20)
            .map(|index| RoutePoint {
                x: 40 + index * 16,
                z: 40 + index * 16,
                natural_height_m: 10.0,
                design_height_m: 10.0,
                water: false,
            })
            .collect();
        RoutePlan {
            format: "glyphweave.route-evolution-plan".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.55,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor {
                    id: "a".into(),
                    x: 40,
                    z: 40,
                    role: "test".into(),
                },
                Anchor {
                    id: "b".into(),
                    x: 344,
                    z: 344,
                    role: "test".into(),
                },
            ],
            candidate_nodes: Vec::new(),
            routes: vec![Route {
                id: "diagonal".into(),
                from: "a".into(),
                to: "b".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline,
                length_m: 430.12,
                direct_length_m: 430.12,
                curve_deviation_m: 0.0,
                maximum_grade: 0.0,
                maximum_fill_m: 0.0,
                maximum_terrain_fill_m: 0.0,
                fill_volume_m3: 0.0,
                bridge_spans: Vec::new(),
                failure: None,
            }],
            failures: Vec::new(),
        }
    }

    fn style(households: u32, jobs: u32, retail: f64) -> Value {
        serde_json::json!({
            "settlementEvolution": {
                "version": 1,
                "householdsTarget": households,
                "jobsTarget": jobs,
                "retailFloorAreaM2": retail,
                "serviceBudget": 1,
                "lotWidthM": 24,
                "lotDepthM": 28,
                "roadStepM": 36,
                "setbackM": 12,
                "maxSlope": 0.25,
                "floorAreaRatio": 1.6,
                "maxFloors": 8
            }
        })
    }

    #[test]
    fn building_count_is_a_result_of_demand_and_capacity() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let low = plan(&scene, &terrain, water, &routes, &style(20, 5, 100.0))
            .unwrap()
            .unwrap();
        let high = plan(&scene, &terrain, water, &routes, &style(2_000, 900, 0.0))
            .unwrap()
            .unwrap();
        assert_eq!(low.status, "pass", "{:?}", low.unmet_demand);
        assert!(high.buildings.len() > low.buildings.len());
        assert!(
            high.buildings
                .iter()
                .map(|building| building.kind.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        );
        assert_eq!(
            high.status, "incomplete",
            "the one-street fixture must expose unmet demand once its physical frontage is exhausted"
        );
        assert!(high.unmet_demand.households > 0);
        assert!(high
            .buildings
            .iter()
            .all(|building| building.recommended_count > 0));
        let entities = materialize(&low, &terrain, None);
        assert_eq!(entities.len(), low.buildings.len());
        assert!(entities.iter().all(|entity| {
            entity
                .entity_id
                .starts_with("generated.settlement-evolution.")
                && entity.width_m > 0.0
                && entity.depth_m > 0.0
                && entity.height_m > 0.0
        }));
    }

    #[test]
    fn frontage_sampling_is_independent_of_polyline_segment_length() {
        let (scene, terrain, water) = fixture();
        let coarse = route_plan();
        let mut fine = route_plan();
        let original = &fine.routes[0].polyline;
        let mut densified = Vec::new();
        for pair in original.windows(2) {
            densified.push(pair[0].clone());
            densified.push(RoutePoint {
                x: (pair[0].x + pair[1].x) / 2,
                z: (pair[0].z + pair[1].z) / 2,
                natural_height_m: 10.0,
                design_height_m: 10.0,
                water: false,
            });
        }
        densified.push(original.last().unwrap().clone());
        fine.routes[0].polyline = densified;

        let style = style(20, 5, 100.0);
        let coarse = plan(&scene, &terrain, water, &coarse, &style)
            .unwrap()
            .unwrap();
        let fine = plan(&scene, &terrain, water, &fine, &style)
            .unwrap()
            .unwrap();
        assert_eq!(coarse.capacity.candidate_lots, fine.capacity.candidate_lots);
        assert_eq!(coarse.capacity.accepted_lots, fine.capacity.accepted_lots);
        assert!(coarse.capacity.candidate_lots >= 20);
    }

    #[test]
    fn street_frontage_audit_distinguishes_rows_from_scattered_lots() {
        let building = |id: &str, position: f64, width: f32| Building {
            id: id.into(),
            route_id: "main".into(),
            side: "left".into(),
            width_m: width,
            frontage_position_m: position,
            district_stage: "old-core".into(),
            ..Building::default()
        };
        let row = audit_street_frontage(&[
            building("a", 10.0, 9.0),
            building("b", 20.0, 9.0),
            building("c", 30.0, 9.0),
        ]);
        assert_eq!(row.status, "pass");
        assert_eq!(row.old_core_continuous_runs, 1);
        assert!(row.old_core_min_coverage_ratio > 0.8);

        let scattered =
            audit_street_frontage(&[building("a", 10.0, 5.0), building("b", 40.0, 5.0)]);
        assert_eq!(scattered.status, "warn");
        assert_eq!(scattered.runs.len(), 2);
        assert!(scattered.runs.iter().all(|run| !run.continuous));
    }

    #[test]
    fn frontage_modules_are_deterministic_and_era_specific() {
        let key = frontage_module_count("old-town-core-north", 123.5, "old-core");
        assert_eq!(
            key,
            frontage_module_count("old-town-core-north", 123.5, "old-core")
        );
        assert!((1..=2).contains(&key));

        let samples = (0..512)
            .map(|index| {
                frontage_module_count(
                    "urban-grid-v260",
                    f64::from(index * 11),
                    "renewal",
                )
            })
            .collect::<Vec<_>>();
        let values = samples
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert!(values.contains(&1));
        assert!(values.contains(&2));
        assert!(values.iter().all(|value| (1..=3).contains(value)));
        assert!(
            samples.iter().any(|value| *value > 1),
            "renewal blocks must contain at least some joined frontages"
        );
    }

    #[test]
    fn materialization_never_sinks_a_building_below_its_terrain_support() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let mut planned = plan(&scene, &terrain, water, &routes, &style(20, 5, 100.0))
            .unwrap()
            .unwrap();
        for building in &mut planned.buildings {
            building.frontage_level_m = Some(4.0);
        }

        let entities = materialize(&planned, &terrain, None);

        assert!(!entities.is_empty());
        assert!(entities.iter().all(|entity| {
            (f64::from(entity.world_y) + f64::from(entity.grounding.bottom_offset_m) - 10.0).abs()
                < 1e-6
        }));
        assert!(entities.iter().all(|entity| {
            let Some(front) = entity.anchors.iter().find(|anchor| anchor.id == "front") else {
                return true;
            };
            let Some(rear) = entity.anchors.iter().find(|anchor| anchor.id == "rear") else {
                return false;
            };
            rear.world_x == entity.world_x * 2 - front.world_x
                && rear.world_z == entity.world_z * 2 - front.world_z
                && rear.direction != front.direction
        }));
    }

    #[test]
    fn promoted_frontage_stair_starts_at_the_declared_door_threshold() {
        let (_, terrain, _) = fixture();
        let mut building = EntityInstance {
            entity_id: "generated.settlement-evolution.test.door".into(),
            asset_id: "building.residential_block".into(),
            kind: "residential_block".into(),
            world_x: 100,
            world_z: 100,
            world_y: 22,
            width_m: 10.0,
            depth_m: 10.0,
            height_m: 20.0,
            scale: 1.0,
            rotation_y_deg: 0.0,
            grounding: super::super::GroundingSpec::default(),
            anchors: vec![super::super::SpatialAnchor {
                id: "front".into(),
                world_x: 100,
                world_z: 95,
                direction: "north".into(),
                target: Some("road".into()),
            }],
            bounds: None,
        };
        let stairs = attach_frontage_stairs(&mut building, &terrain, [128, 100])
            .expect("the 12 m facade step must be repaired by two legal flights");
        assert_eq!(stairs.len(), 2);
        assert!(stairs.iter().all(|stair| stair.height_m <= 8.5));
        let contract = building
            .grounding
            .stair_connection
            .as_ref()
            .expect("the stair connection is attached to the building");
        assert_eq!(contract.from_mm, 22_000);
        assert_eq!(contract.to_mm, 10_000);
        assert_eq!(contract.low_mm, 10_000);
        assert_eq!(contract.high_mm, 22_000);
    }

    #[test]
    fn materialized_epoch_operations_and_parents_reach_the_final_entity() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let mut value = style(300, 80, 600.0);
        value["epochProgram"] = serde_json::json!({
            "version": 1,
            "oldCoreRadiusM": 40,
            "expansionRadiusM": 90,
            "modernRadiusM": 180
        });
        let planned = plan(&scene, &terrain, water, &routes, &value)
            .unwrap()
            .unwrap();
        let epoch = epoch_evolution::plan(&routes, &planned, &value)
            .unwrap()
            .unwrap();
        let entities = materialize(&planned, &terrain, Some(&epoch));
        assert_eq!(entities.len(), planned.buildings.len());
        for assigned in &epoch.assigned_buildings {
            let entity = entities
                .iter()
                .find(|entity| entity.entity_id.ends_with(&assigned.building_id))
                .expect("materialized epoch building");
            assert_eq!(
                entity.grounding.construction_cohort.as_deref(),
                Some(assigned.cohort.as_str())
            );
            assert_eq!(
                entity.grounding.building_operation.as_deref(),
                Some(assigned.operation.as_str())
            );
            assert_eq!(
                entity.grounding.building_parent_id.as_deref(),
                assigned.parent_id.as_deref()
            );
        }
    }

    #[test]
    fn unmet_demand_is_named_not_backfilled() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let result = plan(&scene, &terrain, water, &routes, &style(100_000, 0, 0.0))
            .unwrap()
            .unwrap();
        assert_eq!(result.status, "incomplete");
        assert!(result.unmet_demand.households > 0);
        assert!(require_complete(&result).is_err());
    }

    #[test]
    fn authored_frontage_counts_cannot_coexist_with_demand_mode() {
        let mut value = style(10, 0, 0.0);
        value["frontageProgram"] = serde_json::json!({"requests": [{"id": "fixed"}]});
        assert!(validate_style(&value).is_err());
    }

    #[test]
    fn district_budgets_reserve_epochs_and_spill_only_unused_demand() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let mut value = style(300, 80, 600.0);
        value["epochProgram"] = serde_json::json!({
            "oldCoreRadiusM": 50,
            "expansionRadiusM": 100,
            "modernRadiusM": 160
        });
        value["settlementEvolution"]["districtDemand"] = serde_json::json!({
            "version": 1,
            "allowSpill": true,
            "shares": [
                {"stage": "old-core", "share": 0.15},
                {"stage": "renewal", "share": 0.35},
                {"stage": "new-growth", "share": 0.40},
                {"stage": "periphery", "share": 0.10}
            ]
        });
        let planned = plan(&scene, &terrain, water, &routes, &value)
            .unwrap()
            .unwrap();
        assert_eq!(planned.status, "pass", "{:?}", planned.unmet_demand);
        assert_eq!(planned.district_allocation.len(), 4);
        assert_eq!(
            planned
                .district_allocation
                .iter()
                .map(|entry| entry.planned.households)
                .sum::<u32>(),
            planned.demand.households
        );
        let occupied = planned
            .buildings
            .iter()
            .map(|building| building.district_stage.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(occupied.contains("old-core"));
        assert!(occupied.contains("renewal"));
        assert!(occupied.contains("new-growth"));
        assert_eq!(
            planned
                .district_allocation
                .iter()
                .map(|entry| entry.accepted_lots)
                .sum::<usize>(),
            planned.buildings.len()
        );
        let entities = materialize(&planned, &terrain, None);
        assert_eq!(entities.len(), planned.buildings.len());
        for (entity, building) in entities.iter().zip(&planned.buildings) {
            let expected_district = format!("district.{}", building.district_stage);
            assert_eq!(
                entity.grounding.district_id.as_deref(),
                Some(expected_district.as_str())
            );
            assert_eq!(
                entity.grounding.construction_cohort.as_deref(),
                Some(building.district_stage.as_str())
            );
        }
    }

    #[test]
    fn district_budget_shares_must_be_unique_and_complete() {
        let mut value = style(10, 0, 0.0);
        value["settlementEvolution"]["districtDemand"] = serde_json::json!({
            "version": 1,
            "shares": [
                {"stage": "old-core", "share": 0.6},
                {"stage": "renewal", "share": 0.2}
            ]
        });
        assert!(validate_style(&value).is_err());
    }

    #[test]
    fn feature_focus_reweights_only_inferred_district_budgets() {
        let shares = vec![
            DistrictDemandShare {
                stage: "old-core".into(),
                share: 0.24,
            },
            DistrictDemandShare {
                stage: "renewal".into(),
                share: 0.34,
            },
            DistrictDemandShare {
                stage: "new-growth".into(),
                share: 0.32,
            },
            DistrictDemandShare {
                stage: "periphery".into(),
                share: 0.10,
            },
        ];
        let mut waterfront = style(10, 0, 0.0);
        waterfront["urbanSlice"] = serde_json::json!({
            "version": 1,
            "settlementScale": "metropolis",
            "mode": "feature-cut",
            "focusFeature": "reclaimed-financial-waterfront",
            "cropReason": "focus allocation test",
            "contextRadiusM": 2200,
            "offMapContinuity": ["roads", "coast"],
            "specialFactors": ["financial-center", "coastal-reclamation"]
        });
        let waterfront_bias = urban_slice::compile(&waterfront)
            .unwrap()
            .unwrap()
            .focus_bias;

        let mut historic = style(10, 0, 0.0);
        historic["urbanSlice"] = serde_json::json!({
            "version": 1,
            "settlementScale": "metropolis",
            "mode": "feature-cut",
            "focusFeature": "historic-market-lanes",
            "cropReason": "focus allocation test",
            "contextRadiusM": 2200,
            "offMapContinuity": ["roads", "urban-fabric"],
            "specialFactors": ["historic-core"]
        });
        let historic_bias = urban_slice::compile(&historic).unwrap().unwrap().focus_bias;

        let waterfront_shares = effective_district_shares(&shares, Some(&waterfront_bias));
        let historic_shares = effective_district_shares(&shares, Some(&historic_bias));
        let share = |values: &[DistrictDemandShare], stage: &str| {
            values
                .iter()
                .find(|entry| entry.stage == stage)
                .unwrap()
                .share
        };
        assert!(share(&waterfront_shares, "new-growth") > share(&historic_shares, "new-growth"));
        assert!(share(&historic_shares, "old-core") > share(&waterfront_shares, "old-core"));
        for values in [&waterfront_shares, &historic_shares] {
            let total: f64 = values.iter().map(|entry| entry.share).sum();
            assert!((total - 1.0).abs() < 1e-12);
        }
        assert_eq!(effective_district_shares(&shares, None), shares);
    }

    #[test]
    fn regional_typologies_change_capacity_outcomes() {
        let (scene, terrain, water) = fixture();
        let routes = route_plan();
        let mut china = style(180, 20, 300.0);
        china["communityProfile"] = serde_json::json!({"version": 1, "profile": "china-gated"});
        let mut us = style(180, 20, 300.0);
        us["communityProfile"] = serde_json::json!({"version": 1, "profile": "us-suburban"});
        let china = plan(&scene, &terrain, water, &routes, &china)
            .unwrap()
            .unwrap();
        let us = plan(&scene, &terrain, water, &routes, &us)
            .unwrap()
            .unwrap();
        assert_eq!(china.community_profile.as_deref(), Some("china-gated"));
        assert_eq!(us.community_profile.as_deref(), Some("us-suburban"));
        let footprint = |plan: &Plan| {
            plan.buildings
                .iter()
                .map(|building| building.width_m * building.depth_m)
                .sum::<f32>()
        };
        assert!(
            (footprint(&china) - footprint(&us)).abs() > 1.0,
            "regional typologies must change the realized building-floor capacity"
        );
        assert!(us
            .buildings
            .iter()
            .all(|building| building.income_band == "homeowner"));
    }

    #[test]
    fn diagonal_frontage_buildings_keep_oriented_separation_from_each_other_and_roads() {
        let (scene, terrain, water) = fixture();
        let routes = diagonal_route_plan();
        let planned = plan(&scene, &terrain, water, &routes, &style(80, 20, 300.0))
            .unwrap()
            .unwrap();
        assert!(!planned.buildings.is_empty());
        let footprints = planned
            .buildings
            .iter()
            .map(|building| OrientedFootprint {
                center_x: f64::from(building.world_x),
                center_z: f64::from(building.world_z),
                width_m: f64::from(building.width_m),
                depth_m: f64::from(building.depth_m),
                rotation_y_deg: f64::from(building.rotation_y_deg),
            })
            .collect::<Vec<_>>();
        for (index, left) in footprints.iter().enumerate() {
            for right in footprints.iter().skip(index + 1) {
                assert!(!oriented_footprints_overlap(left, right, 0.05));
            }
        }
        let roads = super::super::route_evolution::materialize(&routes);
        for building in &footprints {
            for road in &roads {
                assert!(!oriented_footprints_overlap(
                    building,
                    &OrientedFootprint::from_entity(road),
                    0.25
                ));
            }
        }
    }
}
