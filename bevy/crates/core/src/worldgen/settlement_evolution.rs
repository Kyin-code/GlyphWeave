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
use super::route_evolution::Plan as RoutePlan;
use super::{EntityInstance, SceneSpec, WaterGeometry};
use crate::settlement::building_archetypes;

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
    pub district_stage: String,
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
    district_stage: String,
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
        || !(12.0..=120.0).contains(&program.road_step_m)
        || !(2.0..=32.0).contains(&program.setback_m)
        || !(0.02..=0.25).contains(&program.max_slope)
        || !(0.2..=8.0).contains(&program.floor_area_ratio)
        || !(1..=60).contains(&program.max_floors)
        || !(1.0..=30.0).contains(&program.access_height_tolerance_m)
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
    let mut accepted_lot_centers = Vec::<(i32, i32)>::new();
    let mut candidates = Vec::<Candidate>::new();
    for route in routes.routes.iter().filter(|route| route.status == "pass") {
        for (segment_index, segment) in route.polyline.windows(2).enumerate() {
            let dx = f64::from(segment[1].x - segment[0].x);
            let dz = f64::from(segment[1].z - segment[0].z);
            let length = dx.hypot(dz);
            if length < 1.0 {
                continue;
            }
            let normal_x = -dz / length;
            let normal_z = dx / length;
            let step = f64::from(program.road_step_m);
            let samples = (length / step).floor().max(1.0) as usize;
            for sample in 0..samples {
                let t = (sample as f64 + 0.5) / samples as f64;
                let road_x = f64::from(segment[0].x) + dx * t;
                let road_z = f64::from(segment[0].z) + dz * t;
                for (side_index, side) in [-1.0_f64, 1.0].into_iter().enumerate() {
                    candidate_lots += 1;
                    let candidate_id = format!(
                        "{}.lot.{}.{}.{}",
                        route.id, segment_index, sample, side_index
                    );
                    let access = route.road_width_m as f64 * 0.5 + f64::from(program.setback_m);
                    let mut center_x =
                        road_x + normal_x * side * (access + f64::from(base_lot_depth_m) * 0.5);
                    let mut center_z =
                        road_z + normal_z * side * (access + f64::from(base_lot_depth_m) * 0.5);
                    let mut x = center_x.round() as i32;
                    let mut z = center_z.round() as i32;
                    let mut center_distance =
                        (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                    let district_stage = district_stage(style, center_distance);
                    let (lot_width_m, lot_depth_m) =
                        staged_lot_size(district_stage, base_lot_width_m, base_lot_depth_m);
                    center_x = road_x + normal_x * side * (access + f64::from(lot_depth_m) * 0.5);
                    center_z = road_z + normal_z * side * (access + f64::from(lot_depth_m) * 0.5);
                    x = center_x.round() as i32;
                    z = center_z.round() as i32;
                    center_distance = (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                    if let Err(reason) = lot_is_legal(
                        scene,
                        terrain,
                        water,
                        x,
                        z,
                        lot_width_m,
                        lot_depth_m,
                        program.max_slope,
                    ) {
                        rejections.push(Rejection {
                            candidate_id,
                            route_id: route.id.clone(),
                            world_x: x,
                            world_z: z,
                            reason,
                        });
                        continue;
                    }
                    let road_design_m = segment[0].design_height_m
                        + (segment[1].design_height_m - segment[0].design_height_m) * t;
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
                    if accepted_lot_centers.iter().any(|(lot_x, lot_z)| {
                        (f64::from(*lot_x) - f64::from(x)).hypot(f64::from(*lot_z) - f64::from(z))
                            < f64::from(lot_width_m.min(lot_depth_m)) * 0.9
                    }) {
                        rejections.push(Rejection {
                            candidate_id,
                            route_id: route.id.clone(),
                            world_x: x,
                            world_z: z,
                            reason: "overlaps_prior_candidate".into(),
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
                        district_stage: district_stage.into(),
                    });
                    accepted_lot_centers.push((x, z));
                }
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
        let planned_by_stage = allocate_district_demands(&target_demand, &districts.shares);
        for entry in &districts.shares {
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
        for (index, candidate) in candidates.iter().enumerate() {
            let limit = stage_remaining
                .get(&candidate.district_stage)
                .cloned()
                .unwrap_or_default();
            if demand_is_empty(&limit) {
                continue;
            }
            let Some(accepted_candidate) = try_build_candidate(candidate, &limit, &rules) else {
                continue;
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
        if districts.allow_spill && !demand_is_empty(&demand) {
            for (index, candidate) in candidates.iter().enumerate() {
                if accepted[index] {
                    continue;
                }
                let Some(accepted_candidate) = try_build_candidate(candidate, &demand, &rules)
                else {
                    continue;
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
        for entry in &districts.shares {
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
            let Some(accepted_candidate) = try_build_candidate(candidate, &demand, &rules) else {
                continue;
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

fn try_build_candidate(
    candidate: &Candidate,
    limit: &Demand,
    rules: &SelectionRules,
) -> Option<AcceptedCandidate> {
    let wants_retail = limit.retail_floor_area_m2 > 0.0 && candidate.pressure > 0.28;
    let wants_jobs = limit.jobs > 0 && (candidate.pressure > 0.55 || limit.households == 0);
    let selection = building_archetypes::SelectionInput {
        community_profile: rules.community_profile.clone(),
        district_stage: candidate.district_stage.clone(),
        center_pressure: candidate.pressure,
        lot_width_m: candidate.width_m,
        lot_depth_m: candidate.depth_m,
        needs_homes: limit.households > 0,
        needs_jobs: wants_jobs,
        needs_retail: wants_retail,
    };
    let archetype = building_archetypes::choose(&selection);
    let mut form = building_archetypes::form(&candidate.id, &selection, &archetype);
    form.floors = form.floors.min(rules.max_floors);
    let floor_area = f64::from(form.footprint_m2) * f64::from(form.floors) * rules.floor_area_ratio;
    let household_capacity = (floor_area / rules.household_area_m2).floor() as u32;
    let job_capacity = (floor_area / rules.job_area_m2).floor() as u32;
    let retail_capacity = floor_area * rules.retail_share;
    let service_capacity = floor_area / 700.0;
    let spent = Demand {
        households: household_capacity.min(limit.households),
        jobs: job_capacity.min(limit.jobs),
        retail_floor_area_m2: retail_capacity.min(limit.retail_floor_area_m2),
        service_budget: service_capacity.min(limit.service_budget),
    };
    if demand_is_empty(&spent) {
        return None;
    }
    let height = 4.0 + form.floors as f32 * 3.2;
    let building = Building {
        id: candidate.id.clone(),
        kind: form.kind.clone(),
        route_id: candidate.route_id.clone(),
        side: candidate.side.clone(),
        world_x: candidate.world_x,
        world_z: candidate.world_z,
        width_m: form.width_m,
        depth_m: form.depth_m,
        height_m: height,
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
        archetype_id: form.archetype_id,
        recommended_count: form.recommended_count,
        lot_area_m2: f64::from(candidate.width_m) * f64::from(candidate.depth_m),
        coverage_ratio: f64::from(form.coverage_ratio),
        frontage_road_point: Some(candidate.frontage_road_point),
        district_stage: candidate.district_stage.clone(),
    };
    Some(AcceptedCandidate {
        building,
        spent,
        floor_area_m2: floor_area,
    })
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
        "old-core" => (0.72, 0.88),
        "renewal" => (0.95, 0.95),
        "new-growth" => (1.20, 1.05),
        _ => (1.30, 0.95),
    };
    (
        (width_m * width_scale).clamp(8.0, 80.0),
        (depth_m * depth_scale).clamp(8.0, 80.0),
    )
}

fn demand_is_empty(demand: &Demand) -> bool {
    demand.households == 0
        && demand.jobs == 0
        && demand.retail_floor_area_m2 <= 1e-6
        && demand.service_budget <= 1e-6
}

fn lot_is_legal(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    x: i32,
    z: i32,
    width: f32,
    depth: f32,
    max_slope: f32,
) -> Result<(), String> {
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
    if slope > max_slope {
        return Err("slope_exceeds_contract".into());
    }
    Ok(())
}

pub fn materialize(plan: &Plan, terrain: &BakedTerrain) -> Vec<EntityInstance> {
    plan.buildings
        .iter()
        .map(|building| {
            let world_y = building_support_height_m(building, terrain).ceil() as i32;
            let anchors = building
                .frontage_road_point
                .map_or_else(Vec::new, |[road_x, road_z]| {
                    let dx = f64::from(road_x - building.world_x);
                    let dz = f64::from(road_z - building.world_z);
                    let length = dx.hypot(dz).max(1.0);
                    let nx = dx / length;
                    let nz = dz / length;
                    let reach = (f64::from(building.width_m) * 0.5 / nx.abs().max(1e-9))
                        .min(f64::from(building.depth_m) * 0.5 / nz.abs().max(1e-9))
                        - 0.5;
                    let reach = reach.max(0.0);
                    vec![super::SpatialAnchor {
                        id: "front".into(),
                        world_x: (f64::from(building.world_x) + nx * reach).round() as i32,
                        world_z: (f64::from(building.world_z) + nz * reach).round() as i32,
                        direction: if dx.abs() > dz.abs() {
                            if dx > 0.0 {
                                "east"
                            } else {
                                "west"
                            }
                        } else if dz > 0.0 {
                            "south"
                        } else {
                            "north"
                        }
                        .into(),
                        target: Some("road".into()),
                    }]
                });
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
                rotation_y_deg: 0.0,
                grounding: super::GroundingSpec::default(),
                anchors,
                bounds: None,
            }
        })
        .collect()
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
            let world_y = building_support_height_m(building, terrain).ceil() as i32;
            Some(EntityInstance {
                entity_id: format!("generated.settlement-evolution.approach.{}", building.id),
                asset_id: "road.generated".into(),
                kind: "road".into(),
                world_x: center_x.round() as i32,
                world_z: center_z.round() as i32,
                world_y,
                scale: 1.0,
                width_m: (approach_length + 4.0) as f32,
                depth_m: 12.0,
                height_m: 0.2,
                rotation_y_deg: dz.atan2(dx).to_degrees() as f32,
                grounding: super::GroundingSpec {
                    roadbed_offset_m: 0.5,
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
    let half_width = (building.width_m * 0.5).ceil() as i32;
    let half_depth = (building.depth_m * 0.5).ceil() as i32;
    for dz in -half_depth..=half_depth {
        for dx in -half_width..=half_width {
            let x = building.world_x + dx;
            let z = building.world_z + dz;
            if let Some(height) = terrain.height(x, z) {
                highest = highest.max(height);
            }
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
        let high = plan(&scene, &terrain, water, &routes, &style(300, 120, 0.0))
            .unwrap()
            .unwrap();
        assert_eq!(low.status, "pass", "{:?}", low.unmet_demand);
        assert!(high.buildings.len() > low.buildings.len());
        assert!(high
            .buildings
            .iter()
            .any(|building| building.kind == "office_tower"));
        let entities = materialize(&low, &terrain);
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
        assert_ne!(china.buildings.len(), us.buildings.len());
        assert!(us
            .buildings
            .iter()
            .all(|building| building.income_band == "homeowner"));
    }
}
