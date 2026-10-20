//! Explicit macro-engineering budget derived from urban slice pressure.
//!
//! Scale factors may fund larger structures or earthwork programmes. They never
//! relax local pad, road-grade, shoreline or walkability contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::route_evolution::Plan as RoutePlan;
use super::urban_slice;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub route_id: String,
    pub kind: String,
    pub start_index: usize,
    pub end_index: usize,
    pub length_m: f64,
    pub width_m: u32,
    pub maximum_clearance_m: f64,
    pub required_structure_volume_m3: f64,
    pub budget_m3: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalContracts {
    pub max_fill_m: f64,
    pub max_road_grade: f64,
    pub max_walk_grade: f64,
    pub max_entry_apron_m: f64,
    pub setback_range_m: [f64; 2],
    pub shoreline_clearance_m: f64,
    pub max_local_pad_cut_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlement_scale: String,
    pub land_pressure_index: f64,
    pub engineering_scale: f64,
    pub local_contracts: LocalContracts,
    pub operations: Vec<Operation>,
    pub failures: Vec<String>,
}

pub fn plan(routes: &RoutePlan, style: &Value) -> Result<Option<Plan>, String> {
    let Some(slice) = urban_slice::compile(style)? else {
        return Ok(None);
    };
    let engineering_scale = f64::from(slice.engineering_scale);
    let land_pressure_index = f64::from(slice.land_pressure_index);
    let mut operations = Vec::new();
    for route in routes.routes.iter().filter(|route| route.status == "pass") {
        if route.fill_volume_m3 > 0.0 {
            let required = route.fill_volume_m3;
            let budget = required * 2.0 * engineering_scale.max(0.1);
            operations.push(Operation {
                id: format!("{}.embankment", route.id),
                route_id: route.id.clone(),
                kind: "embankment".into(),
                start_index: 0,
                end_index: route.polyline.len().saturating_sub(1),
                length_m: route.length_m,
                width_m: route.road_width_m,
                maximum_clearance_m: route.maximum_terrain_fill_m,
                required_structure_volume_m3: required,
                budget_m3: budget,
                status: if route.maximum_terrain_fill_m <= 6.0 + 1e-6
                    && required <= budget + 1e-6
                {
                    "approved-within-local-fill-contract".into()
                } else {
                    "rejected-local-fill-contract".into()
                },
            });
        }
        for (index, span) in route.bridge_spans.iter().enumerate() {
            let points = &route.polyline[span.start_index..=span.end_index];
            let clearance = points
                .iter()
                .map(|point| (point.design_height_m - point.natural_height_m).max(0.0))
                .fold(0.0, f64::max);
            let required = span.length_m
                * f64::from(route.road_width_m)
                * clearance.max(1.0)
                * 0.32;
            let budget = span.length_m
                * f64::from(route.road_width_m)
                * 12.0
                * engineering_scale.max(0.1);
            operations.push(Operation {
                id: format!("{}.{}.{}", route.id, span.kind, index),
                route_id: route.id.clone(),
                kind: span.kind.clone(),
                start_index: span.start_index,
                end_index: span.end_index,
                length_m: span.length_m,
                width_m: route.road_width_m,
                maximum_clearance_m: clearance,
                required_structure_volume_m3: required,
                budget_m3: budget,
                status: if required <= budget + 1e-6 {
                    "approved".into()
                } else {
                    "rejected-budget".into()
                },
            });
        }
    }
    let mut failures = Vec::new();
    for operation in &operations {
        if operation.status.starts_with("rejected") {
            failures.push(format!("{}: {}", operation.id, operation.status));
        }
    }
    Ok(Some(Plan {
        format: "glyphweave.engineering-scale-plan".into(),
        version: 1,
        status: if failures.is_empty() { "pass".into() } else { "fail".into() },
        settlement_scale: slice.settlement_scale,
        land_pressure_index,
        engineering_scale,
        local_contracts: LocalContracts {
            max_fill_m: 6.0,
            max_road_grade: 0.08,
            max_walk_grade: 0.25,
            max_entry_apron_m: 2.0,
            setback_range_m: [8.0, 32.0],
            shoreline_clearance_m: 4.0,
            max_local_pad_cut_m: 2.0,
        },
        operations,
        failures,
    }))
}

pub fn require_pass(plan: &Plan) -> Result<(), String> {
    if plan.status == "pass" {
        Ok(())
    } else {
        Err(format!(
            "engineering-scale plan failed: {}",
            plan.failures.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::route_evolution::{
        Anchor, BridgeSpan, Route, RoutePoint,
    };

    fn route_plan() -> RoutePlan {
        RoutePlan {
            format: "route".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.55,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor { id: "a".into(), x: 0, z: 0, role: String::new() },
                Anchor { id: "b".into(), x: 100, z: 0, role: String::new() },
            ],
            routes: vec![Route {
                id: "main".into(),
                from: "a".into(),
                to: "b".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline: vec![
                    RoutePoint {
                        x: 0,
                        z: 0,
                        natural_height_m: 0.0,
                        design_height_m: 1.0,
                        water: false,
                    },
                    RoutePoint {
                        x: 100,
                        z: 0,
                        natural_height_m: 0.0,
                        design_height_m: 1.0,
                        water: false,
                    },
                ],
                length_m: 100.0,
                direct_length_m: 100.0,
                curve_deviation_m: 0.0,
                maximum_grade: 0.0,
                maximum_fill_m: 1.0,
                maximum_terrain_fill_m: 1.0,
                fill_volume_m3: 800.0,
                bridge_spans: vec![BridgeSpan {
                    start_index: 0,
                    end_index: 1,
                    length_m: 100.0,
                    kind: "viaduct".into(),
                }],
                failure: None,
            }],
            failures: Vec::new(),
        }
    }

    fn style(scale: &str) -> Value {
        let (mode, focus, factors) = if scale == "town" {
            (
                "whole-settlement",
                Value::Null,
                serde_json::json!([]),
            )
        } else {
            (
                "feature-cut",
                serde_json::json!("financial-core"),
                serde_json::json!(["financial-center", "mountainous"]),
            )
        };
        serde_json::json!({
            "urbanSlice": {
                "version": 1,
                "settlementScale": scale,
                "mode": mode,
                "focusFeature": focus,
                "cropReason": "engineering scale test",
                "contextRadiusM": 2500,
                "offMapContinuity": ["roads"],
                "specialFactors": factors
            }
        })
    }

    #[test]
    fn metropolitan_pressure_funds_more_than_a_small_town() {
        let town = plan(&route_plan(), &style("town")).unwrap().unwrap();
        let metropolis = plan(&route_plan(), &style("metropolis")).unwrap().unwrap();
        assert!(metropolis.engineering_scale > town.engineering_scale);
        assert!(metropolis.operations.iter().all(|operation| operation.status == "approved" || operation.status.starts_with("approved")));
        assert_eq!(metropolis.local_contracts.max_fill_m, 6.0);
        assert_eq!(metropolis.local_contracts.max_road_grade, 0.08);
    }
}
