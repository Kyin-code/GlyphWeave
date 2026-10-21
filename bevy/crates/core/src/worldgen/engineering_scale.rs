//! Explicit macro-engineering budget derived from urban slice pressure.
//!
//! Scale factors may fund larger structures or earthwork programmes. They never
//! relax local pad, road-grade, shoreline or walkability contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::land_pressure;
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
pub struct MacroAuthority {
    pub mode: String,
    pub focus_feature: String,
    pub special_factors: Vec<String>,
    pub allows_local_fill_above_m: f64,
    pub authority_scope: String,
    pub no_absolute_earthwork_cap: bool,
    pub engineering_audit_required: bool,
    pub signals: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlement_scale: String,
    pub network_centrality: f32,
    pub land_pressure_index: f64,
    pub engineering_scale: f64,
    pub local_contracts: LocalContracts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_authority: Option<MacroAuthority>,
    pub operations: Vec<Operation>,
    pub failures: Vec<String>,
}

pub fn plan(routes: &RoutePlan, style: &Value) -> Result<Option<Plan>, String> {
    let Some(slice) = urban_slice::compile(style)? else {
        return Ok(None);
    };
    let network_centrality = land_pressure::route_centrality(routes);
    let pressure = land_pressure::plan(style, Some(network_centrality))?;
    let pressure_engineering_scale = f64::from(
        pressure
            .as_ref()
            .map_or(slice.engineering_scale, |plan| plan.engineering_scale),
    );
    let engineering_scale = (pressure_engineering_scale
        * (0.75 + 0.25 * f64::from(slice.focus_bias.engineering_bias)))
    .clamp(0.45, 2.4);
    let land_pressure_index = f64::from(
        pressure
            .as_ref()
            .map_or(slice.land_pressure_index, |plan| plan.land_pressure_index),
    );
    let macro_authority = macro_authority(&slice);
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
                status: if required > budget + 1e-6 {
                    "rejected-budget".into()
                } else if route.maximum_terrain_fill_m > 6.0 + 1e-6 {
                    if macro_authority.is_some() {
                        "approved-macro-authority".into()
                    } else {
                        "rejected-local-fill-contract".into()
                    }
                } else {
                    "approved-within-local-fill-contract".into()
                },
            });
        }
        for (index, span) in route.bridge_spans.iter().enumerate() {
            let points = &route.polyline[span.start_index..=span.end_index];
            let clearance = points
                .iter()
                .map(|point| (point.design_height_m - point.natural_height_m).max(0.0))
                .fold(0.0, f64::max);
            let required =
                span.length_m * f64::from(route.road_width_m) * clearance.max(1.0) * 0.32;
            let budget =
                span.length_m * f64::from(route.road_width_m) * 12.0 * engineering_scale.max(0.1);
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
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        settlement_scale: slice.settlement_scale,
        network_centrality,
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
        macro_authority,
        operations,
        failures,
    }))
}

fn macro_authority(slice: &urban_slice::Plan) -> Option<MacroAuthority> {
    if slice.mode != "feature-cut" {
        return None;
    }
    let authority_scope = if slice
        .special_factors
        .iter()
        .any(|factor| factor == "special-economic-zone")
    {
        "special-area-planning-led"
    } else if slice
        .special_factors
        .iter()
        .any(|factor| factor == "coastal-reclamation" || factor == "island")
    {
        "reclamation-planning-led"
    } else if slice
        .special_factors
        .iter()
        .any(|factor| factor == "mountainous")
    {
        "hillside-planning-led"
    } else {
        "feature-cut-planning-led"
    };
    Some(MacroAuthority {
        mode: "feature-cut".into(),
        focus_feature: slice.focus_feature.clone().unwrap_or("feature-cut".into()),
        special_factors: slice.special_factors.clone(),
        allows_local_fill_above_m: 120.0,
        authority_scope: authority_scope.into(),
        no_absolute_earthwork_cap: true,
        engineering_audit_required: true,
        signals: slice.focus_bias.signals.clone(),
        rationale: "feature and planning-led cuts may fund larger earthworks through the macro ledger without an absolute earthwork cap; ordinary lots, roads, shorelines and protected land keep their local contracts and every operation remains audited".into(),
    })
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
    use crate::worldgen::route_evolution::{Anchor, BridgeSpan, Route, RoutePoint};

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
                Anchor {
                    id: "a".into(),
                    x: 0,
                    z: 0,
                    role: String::new(),
                },
                Anchor {
                    id: "b".into(),
                    x: 100,
                    z: 0,
                    role: String::new(),
                },
            ],
            candidate_nodes: Vec::new(),
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
            ("whole-settlement", Value::Null, serde_json::json!([]))
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
        assert!(
            metropolis
                .operations
                .iter()
                .all(|operation| operation.status == "approved"
                    || operation.status.starts_with("approved"))
        );
        assert_eq!(metropolis.local_contracts.max_fill_m, 6.0);
        assert_eq!(metropolis.local_contracts.max_road_grade, 0.08);
    }

    #[test]
    fn feature_cut_can_use_macro_earthwork_beyond_the_local_fill_bound() {
        let mut route = route_plan();
        route.routes[0].maximum_terrain_fill_m = 24.0;
        route.routes[0].fill_volume_m3 = 96_000.0;

        let town = plan(&route, &style("town")).unwrap().unwrap();
        assert_eq!(town.status, "fail");
        assert!(
            town.operations
                .iter()
                .any(|operation| operation.status == "rejected-local-fill-contract")
        );
        assert!(town.macro_authority.is_none());

        let feature = plan(&route, &style("metropolis")).unwrap().unwrap();
        assert_eq!(feature.status, "pass");
        assert!(feature.macro_authority.is_some());
        assert!(
            feature
                .operations
                .iter()
                .any(|operation| operation.status == "approved-macro-authority")
        );
        assert_eq!(feature.local_contracts.max_fill_m, 6.0);
    }

    #[test]
    fn planning_led_reclamation_authority_is_explicit_and_audited() {
        let mut value = style("metropolis");
        value["urbanSlice"]["focusFeature"] =
            serde_json::json!("island-reclamation-and-financial-core");
        value["urbanSlice"]["specialFactors"] =
            serde_json::json!(["coastal-reclamation", "island", "financial-center"]);
        let plan = plan(&route_plan(), &value).unwrap().unwrap();
        let authority = plan.macro_authority.unwrap();
        assert_eq!(authority.authority_scope, "reclamation-planning-led");
        assert!(authority.no_absolute_earthwork_cap);
        assert!(authority.engineering_audit_required);
        assert!(authority
            .signals
            .iter()
            .any(|signal| signal.contains("coastal-reclamation")));
        assert!(authority
            .rationale
            .contains("without an absolute earthwork cap"));
        assert_eq!(plan.local_contracts.max_fill_m, 6.0);
        assert_eq!(plan.local_contracts.max_road_grade, 0.08);
    }
}
