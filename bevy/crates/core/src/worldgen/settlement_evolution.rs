//! Demand- and capacity-led settlement growth.
//!
//! Building counts are outputs, never authored completion targets. The planner
//! walks terrain routes, creates legal candidate lots and spends their floor
//! area against household, job, retail and service demand. Candidates that fail
//! slope, water or access contracts are named in the report.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::{BakedTerrain, TerrainError};

use super::community_typology;
use super::route_evolution::Plan as RoutePlan;
use super::{EntityInstance, SceneSpec, WaterGeometry};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Demand {
    pub households: u32,
    pub jobs: u32,
    pub retail_floor_area_m2: f64,
    pub service_budget: f64,
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
    let lot_width_m = (program.lot_width_m
        * typology.as_ref().map_or(1.0, |value| value.lot_width_scale))
    .clamp(8.0, 80.0);
    let lot_depth_m = (program.lot_depth_m
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
                    let center_x =
                        road_x + normal_x * side * (access + f64::from(lot_depth_m) * 0.5);
                    let center_z =
                        road_z + normal_z * side * (access + f64::from(lot_depth_m) * 0.5);
                    let x = center_x.round() as i32;
                    let z = center_z.round() as i32;
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
                    if buildings.iter().any(|building: &Building| {
                        (f64::from(building.world_x) - f64::from(x))
                            .hypot(f64::from(building.world_z) - f64::from(z))
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
                    let center_distance = (f64::from(x) - center.0).hypot(f64::from(z) - center.1);
                    let pressure = (1.0 - center_distance / 650.0).clamp(0.15, 1.0);
                    let floors =
                        ((1.0 + pressure * f64::from(max_floors - 1)).round() as u32).max(1);
                    let footprint = f64::from(lot_width_m) * f64::from(lot_depth_m) * 0.58;
                    let floor_area = footprint * f64::from(floors) * f64::from(floor_area_ratio);
                    let household_capacity = (floor_area / household_area_m2).floor() as u32;
                    let job_capacity = (floor_area / job_area_m2).floor() as u32;
                    let retail_capacity = floor_area * retail_share;
                    let service_capacity = floor_area / 700.0;
                    let wants_retail = demand.retail_floor_area_m2 > 0.0 && pressure > 0.28;
                    let wants_jobs = demand.jobs > 0 && (pressure > 0.55 || demand.households == 0);
                    let homes = household_capacity.min(demand.households);
                    let jobs = job_capacity.min(demand.jobs);
                    let retail = retail_capacity.min(demand.retail_floor_area_m2);
                    let services = service_capacity.min(demand.service_budget);
                    if homes == 0 && jobs == 0 && retail <= 0.0 && services <= 0.0 {
                        rejections.push(Rejection {
                            candidate_id,
                            route_id: route.id.clone(),
                            world_x: x,
                            world_z: z,
                            reason: "no_remaining_demand".into(),
                        });
                        continue;
                    }
                    let (kind, width, depth) = if wants_retail {
                        ("storefront", lot_width_m.min(24.0), lot_depth_m.min(20.0))
                    } else if wants_jobs {
                        ("office_tower", lot_width_m.min(28.0), lot_depth_m.min(26.0))
                    } else {
                        (
                            "residential_home",
                            lot_width_m.min(24.0),
                            lot_depth_m.min(26.0),
                        )
                    };
                    let height = 4.0 + floors as f32 * 3.2;
                    let building = Building {
                        id: candidate_id.clone(),
                        kind: kind.into(),
                        route_id: route.id.clone(),
                        side: if side < 0.0 {
                            "left".into()
                        } else {
                            "right".into()
                        },
                        world_x: x,
                        world_z: z,
                        width_m: width,
                        depth_m: depth,
                        height_m: height,
                        floors,
                        floor_area_m2: floor_area,
                        households: homes,
                        jobs,
                        retail_floor_area_m2: retail,
                        service_budget: services,
                        access_distance_m: access as f32,
                        center_distance_m: center_distance,
                        community_profile: typology
                            .as_ref()
                            .map_or_else(|| "generic".into(), |value| value.profile.clone()),
                        income_band: typology.as_ref().map_or_else(
                            || "mixed".into(),
                            |value| {
                                community_typology::income_band(&value.profile, center_distance)
                                    .into()
                            },
                        ),
                    };
                    demand.households -= homes;
                    demand.jobs -= jobs;
                    demand.retail_floor_area_m2 = (demand.retail_floor_area_m2 - retail).max(0.0);
                    demand.service_budget = (demand.service_budget - services).max(0.0);
                    delivered.households += homes;
                    delivered.jobs += jobs;
                    delivered.retail_floor_area_m2 += retail;
                    delivered.service_budget += services;
                    total_floor_area += floor_area;
                    accepted_lots += 1;
                    buildings.push(building);
                    if demand.households == 0
                        && demand.jobs == 0
                        && demand.retail_floor_area_m2 <= 1e-6
                        && demand.service_budget <= 1e-6
                    {
                        break;
                    }
                }
                if demand.households == 0
                    && demand.jobs == 0
                    && demand.retail_floor_area_m2 <= 1e-6
                    && demand.service_budget <= 1e-6
                {
                    break;
                }
            }
            if demand.households == 0
                && demand.jobs == 0
                && demand.retail_floor_area_m2 <= 1e-6
                && demand.service_budget <= 1e-6
            {
                break;
            }
        }
    }
    let complete = demand.households == 0
        && demand.jobs == 0
        && demand.retail_floor_area_m2 <= 1e-6
        && demand.service_budget <= 1e-6;
    Ok(Some(Plan {
        format: "glyphweave.settlement-evolution-plan".into(),
        version: 1,
        status: if complete {
            "pass".into()
        } else {
            "incomplete".into()
        },
        demand: Demand {
            households: program.households_target,
            jobs: program.jobs_target,
            retail_floor_area_m2: program.retail_floor_area_m2,
            service_budget: program.service_budget,
        },
        delivered,
        unmet_demand: demand,
        capacity: Capacity {
            candidate_lots,
            accepted_lots,
            rejected_lots: rejections.len(),
            floor_area_m2: total_floor_area,
        },
        buildings,
        rejections,
        community_profile: typology.as_ref().map(|value| value.profile.clone()),
    }))
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
            let world_y = terrain
                .height(building.world_x, building.world_z)
                .map_or(0.0, |height| height * 4.0)
                .round() as i32;
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
                anchors: Vec::new(),
                bounds: None,
            }
        })
        .collect()
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
        assert_eq!(low.status, "pass");
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
