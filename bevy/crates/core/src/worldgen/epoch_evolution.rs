//! Static construction-epoch ledger for a settlement plan.
//!
//! The final buildings remain the P3 capacity result. This module adds the
//! planning history: historic cores, expansion, modern additions, preservation,
//! modification, demolition and replacements with explicit parent identities.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::route_evolution::Plan as RoutePlan;
use super::settlement_evolution::Plan as SettlementPlan;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_old_core")]
    pub old_core_radius_m: f64,
    #[serde(default = "default_expansion")]
    pub expansion_radius_m: f64,
    #[serde(default = "default_modern")]
    pub modern_radius_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoricBuilding {
    pub id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: f32,
    pub depth_m: f32,
    pub floors: u32,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EpochOperation {
    pub stage: String,
    pub operation: String,
    pub entity_id: String,
    pub parent_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssignedBuilding {
    pub building_id: String,
    pub cohort: String,
    pub operation: String,
    pub parent_id: Option<String>,
    pub center_distance_m: f64,
    pub functional_area: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub center: [f64; 2],
    pub epochs: Vec<String>,
    pub historic_buildings: Vec<HistoricBuilding>,
    pub operations: Vec<EpochOperation>,
    pub assigned_buildings: Vec<AssignedBuilding>,
    pub failures: Vec<String>,
}

const fn default_true() -> bool {
    true
}
const fn default_old_core() -> f64 {
    160.0
}
const fn default_expansion() -> f64 {
    320.0
}
const fn default_modern() -> f64 {
    560.0
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
        || !(40.0..=600.0).contains(&program.old_core_radius_m)
        || !(program.old_core_radius_m..=1500.0).contains(&program.expansion_radius_m)
        || !(program.expansion_radius_m..=4000.0).contains(&program.modern_radius_m)
    {
        return Err("invalid epochProgram radii".into());
    }
    Ok(())
}

fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("epochProgram") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn plan(
    routes: &RoutePlan,
    settlement: &SettlementPlan,
    style: &Value,
) -> Result<Option<Plan>, String> {
    validate_style(style)?;
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    let center = route_center(routes);
    let mut historic_buildings = Vec::new();
    let mut operations = Vec::new();
    let mut assigned_buildings = Vec::new();
    for building in &settlement.buildings {
        let distance = (f64::from(building.world_x) - center.0)
            .hypot(f64::from(building.world_z) - center.1);
        let functional_area = if building.kind == "storefront" {
            "commercial-street"
        } else if building.kind == "office_tower" {
            "employment-centre"
        } else {
            "residential-neighbourhood"
        }
        .to_string();
        if distance <= program.old_core_radius_m {
            let historic_id = format!("historic.{}", building.id);
            let historic_floors = building.floors.saturating_sub(2).max(1);
            historic_buildings.push(HistoricBuilding {
                id: historic_id.clone(),
                world_x: building.world_x,
                world_z: building.world_z,
                width_m: building.width_m * 0.82,
                depth_m: building.depth_m * 0.82,
                floors: historic_floors,
                kind: building.kind.clone(),
            });
            operations.push(EpochOperation {
                stage: "old-core".into(),
                operation: "new".into(),
                entity_id: historic_id.clone(),
                parent_id: None,
                reason: "historic core seed before modern redevelopment".into(),
            });
            if building.kind == "residential_home" {
                operations.push(EpochOperation {
                    stage: "renewal".into(),
                    operation: "demolish".into(),
                    entity_id: historic_id.clone(),
                    parent_id: None,
                    reason: "old housing replaced by capacity-led redevelopment".into(),
                });
                operations.push(EpochOperation {
                    stage: "renewal".into(),
                    operation: "new".into(),
                    entity_id: building.id.clone(),
                    parent_id: Some(historic_id),
                    reason: "replacement housing and local services".into(),
                });
                assigned_buildings.push(AssignedBuilding {
                    building_id: building.id.clone(),
                    cohort: "renewal".into(),
                    operation: "replace".into(),
                    parent_id: operations.last().and_then(|op| op.parent_id.clone()),
                    center_distance_m: distance,
                    functional_area,
                });
            } else if building.floors > historic_floors {
                operations.push(EpochOperation {
                    stage: "modern-new".into(),
                    operation: "modify".into(),
                    entity_id: building.id.clone(),
                    parent_id: Some(historic_id),
                    reason: "vertical addition / mixed-use intensification".into(),
                });
                assigned_buildings.push(AssignedBuilding {
                    building_id: building.id.clone(),
                    cohort: "old-core".into(),
                    operation: "modify".into(),
                    parent_id: operations.last().and_then(|op| op.parent_id.clone()),
                    center_distance_m: distance,
                    functional_area,
                });
            } else {
                operations.push(EpochOperation {
                    stage: "renewal".into(),
                    operation: "preserve".into(),
                    entity_id: building.id.clone(),
                    parent_id: Some(historic_id),
                    reason: "retain historic street frontage".into(),
                });
                assigned_buildings.push(AssignedBuilding {
                    building_id: building.id.clone(),
                    cohort: "old-core".into(),
                    operation: "preserve".into(),
                    parent_id: operations.last().and_then(|op| op.parent_id.clone()),
                    center_distance_m: distance,
                    functional_area,
                });
            }
        } else {
            let cohort = if distance <= program.expansion_radius_m {
                "expansion"
            } else if distance <= program.modern_radius_m {
                "modern-new"
            } else {
                "outer-growth"
            };
            operations.push(EpochOperation {
                stage: cohort.into(),
                operation: "new".into(),
                entity_id: building.id.clone(),
                parent_id: None,
                reason: "capacity-led growth outside the historic core".into(),
            });
            assigned_buildings.push(AssignedBuilding {
                building_id: building.id.clone(),
                cohort: cohort.into(),
                operation: "new".into(),
                parent_id: None,
                center_distance_m: distance,
                functional_area,
            });
        }
    }
    let known: std::collections::BTreeSet<_> = settlement
        .buildings
        .iter()
        .map(|building| building.id.as_str())
        .collect();
    let mut failures = Vec::new();
    for assigned in &assigned_buildings {
        if !known.contains(assigned.building_id.as_str()) {
            failures.push(format!(
                "epoch assignment references unknown building {}",
                assigned.building_id
            ));
        }
        if assigned.operation == "replace" && assigned.parent_id.is_none() {
            failures.push(format!(
                "replacement {} has no parent identity",
                assigned.building_id
            ));
        }
    }
    if assigned_buildings.len() != settlement.buildings.len() {
        failures.push("not every final building has an epoch assignment".into());
    }
    Ok(Some(Plan {
        format: "glyphweave.epoch-evolution-plan".into(),
        version: 1,
        status: if failures.is_empty() { "pass".into() } else { "fail".into() },
        center: [center.0, center.1],
        epochs: vec![
            "founding".into(),
            "old-core".into(),
            "expansion".into(),
            "industrial".into(),
            "modern-new".into(),
            "renewal".into(),
        ],
        historic_buildings,
        operations,
        assigned_buildings,
        failures,
    }))
}

fn route_center(plan: &RoutePlan) -> (f64, f64) {
    if plan.anchors.is_empty() {
        return (0.0, 0.0);
    }
    let count = plan.anchors.len() as f64;
    (
        plan.anchors.iter().map(|anchor| f64::from(anchor.x)).sum::<f64>() / count,
        plan.anchors.iter().map(|anchor| f64::from(anchor.z)).sum::<f64>() / count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::route_evolution::{Anchor, Route, RoutePoint};
    use crate::worldgen::settlement_evolution::{
        Building, Capacity, Demand, Plan as SettlementPlan,
    };

    fn routes() -> RoutePlan {
        RoutePlan {
            format: "route".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.5,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor { id: "a".into(), x: 100, z: 100, role: String::new() },
                Anchor { id: "b".into(), x: 500, z: 100, role: String::new() },
            ],
            routes: vec![Route {
                id: "main".into(),
                from: "a".into(),
                to: "b".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline: vec![RoutePoint {
                    x: 100,
                    z: 100,
                    natural_height_m: 0.0,
                    design_height_m: 0.0,
                    water: false,
                }],
                length_m: 400.0,
                direct_length_m: 400.0,
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

    fn settlement() -> SettlementPlan {
        let make = |id: &str, x: i32, kind: &str, floors: u32| Building {
            id: id.into(),
            kind: kind.into(),
            route_id: "main".into(),
            side: "left".into(),
            world_x: x,
            world_z: 120,
            width_m: 20.0,
            depth_m: 24.0,
            height_m: 12.0,
            floors,
            floor_area_m2: 300.0,
            households: 20,
            jobs: 5,
            retail_floor_area_m2: 20.0,
            service_budget: 0.0,
            access_distance_m: 10.0,
            center_distance_m: 0.0,
            community_profile: "generic".into(),
            income_band: "mixed".into(),
        };
        SettlementPlan {
            format: "settlement".into(),
            version: 1,
            status: "pass".into(),
            demand: Demand { households: 60, jobs: 15, retail_floor_area_m2: 60.0, service_budget: 0.0 },
            delivered: Demand { households: 60, jobs: 15, retail_floor_area_m2: 60.0, service_budget: 0.0 },
            unmet_demand: Demand { households: 0, jobs: 0, retail_floor_area_m2: 0.0, service_budget: 0.0 },
            capacity: Capacity { candidate_lots: 3, accepted_lots: 3, rejected_lots: 0, floor_area_m2: 900.0 },
            buildings: vec![
                make("shop", 280, "storefront", 4),
                make("home", 300, "residential_home", 3),
                make("outer", 500, "residential_home", 3),
            ],
            rejections: Vec::new(),
            community_profile: None,
        }
    }

    #[test]
    fn epochs_preserve_modify_and_replace_with_parent_ids() {
        let style = serde_json::json!({
            "epochProgram": {
                "version": 1,
                "oldCoreRadiusM": 160,
                "expansionRadiusM": 320,
                "modernRadiusM": 560
            }
        });
        let plan = plan(&routes(), &settlement(), &style).unwrap().unwrap();
        assert_eq!(plan.status, "pass");
        assert!(plan.operations.iter().any(|op| op.operation == "modify"));
        assert!(plan.operations.iter().any(|op| op.operation == "demolish"));
        assert!(plan
            .assigned_buildings
            .iter()
            .any(|building| building.operation == "replace"));
        assert!(plan
            .assigned_buildings
            .iter()
            .filter(|building| building.operation == "replace")
            .all(|building| building.parent_id.is_some()));
    }

    #[test]
    fn non_increasing_radii_fail_closed() {
        let style = serde_json::json!({
            "epochProgram": {
                "version": 1,
                "oldCoreRadiusM": 300,
                "expansionRadiusM": 200,
                "modernRadiusM": 500
            }
        });
        assert!(validate_style(&style).is_err());
    }
}
