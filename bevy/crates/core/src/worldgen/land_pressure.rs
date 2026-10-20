//! Explainable land-pressure and macro-engineering intensity.
//!
//! Pressure combines urban scale, network-centre proximity, terrain scarcity,
//! protection and special regimes. It funds larger explicit engineering
//! projects; it never relaxes local road, pad, shoreline or walkability limits.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::route_evolution::Plan as RoutePlan;
use super::{natural_geography, urban_case_priors, urban_slice};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    pub urban_scale: f32,
    pub centre_pressure: f32,
    pub terrain_scarcity: f32,
    pub protection_resistance: f32,
    pub special_regime: f32,
    pub tourism_seasonality: f32,
    pub sample_prior_blend: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlement_scale: String,
    pub focus_feature: Option<String>,
    pub case_prior_id: Option<String>,
    pub network_centrality: f32,
    pub factors: Breakdown,
    pub land_pressure_index: f32,
    pub engineering_scale: f32,
    pub hard_contracts_unchanged: bool,
    pub notes: Vec<String>,
}

pub fn plan(style: &Value, network_centrality: Option<f32>) -> Result<Option<Plan>, String> {
    let Some(slice) = urban_slice::compile(style)? else {
        return Ok(None);
    };
    let geography = natural_geography::controls_for_style(style)?;
    let centrality = network_centrality.unwrap_or(0.5).clamp(0.0, 1.0);
    let urban_scale = slice.scale_factor;
    let centre_pressure = 0.88 + centrality * 0.32;
    let relief_pressure = (geography.relief_scale - 0.7).max(0.0) * 0.22;
    let coast_pressure = geography.coastality * 0.18;
    let wetland_pressure = geography.wetland_bias * 0.10;
    let hazard_pressure =
        ((geography.mass_wasting + geography.fluvial_incision + geography.coastal_erosion) / 3.0)
            * 0.16;
    let terrain_scarcity =
        (1.0 + relief_pressure + coast_pressure + wetland_pressure + hazard_pressure)
            .clamp(0.85, 1.8);
    let historic = slice
        .special_factors
        .iter()
        .any(|factor| factor == "historic-core");
    let protected_nature = geography.wetland_bias * 0.18
        + geography.karst.clamp(0.0, 1.0) * 0.12
        + geography.glacial.clamp(0.0, 1.0) * 0.08;
    let protection_resistance =
        (if historic { 0.68 } else { 0.38 } + protected_nature).clamp(0.2, 1.0);
    let mut special_regime = 0.0_f32;
    for factor in &slice.special_factors {
        special_regime += match factor.as_str() {
            "special-economic-zone" => 0.22,
            "financial-center" => 0.12,
            "political-capital" => 0.10,
            "port-industrial" => 0.12,
            "coastal-reclamation" => 0.18,
            "island" => 0.10,
            "mountainous" => 0.20,
            "historic-core" => 0.08,
            "tourism" => 0.06,
            _ => 0.0,
        };
    }
    special_regime = (1.0 + special_regime).clamp(1.0, 1.65);
    let tourism = slice
        .special_factors
        .iter()
        .any(|factor| factor == "tourism");
    let tourism_seasonality = if tourism {
        1.0 + geography.seasonality.clamp(0.0, 1.0) * 0.18
    } else {
        1.0
    };

    let case_prior_id = case_id(style);
    let sample_prior_blend = case_prior_id
        .as_deref()
        .and_then(|id| urban_case_priors::case(id).ok().flatten())
        .map_or(0.0, |prior| {
            let p = prior.land_pressure_prior;
            0.65 * p.terrain_scarcity as f32
                + 0.20 * p.special_regime as f32
                + 0.15 * p.protection_resistance as f32
        });
    let modelled = urban_scale
        * centre_pressure
        * terrain_scarcity
        * (1.0 + protection_resistance * 0.18)
        * special_regime
        * tourism_seasonality;
    let blended = if sample_prior_blend > 0.0 {
        modelled * 0.62 + sample_prior_blend * urban_scale * 0.58
    } else {
        modelled
    };
    let land_pressure_index = blended.clamp(0.15, 2.4);
    let engineering_scale = (0.45 + land_pressure_index * 0.90).clamp(0.45, 2.4);
    Ok(Some(Plan {
        format: "glyphweave.land-pressure-plan".into(),
        version: 1,
        status: "pass".into(),
        settlement_scale: slice.settlement_scale,
        focus_feature: slice.focus_feature,
        case_prior_id,
        network_centrality: centrality,
        factors: Breakdown {
            urban_scale,
            centre_pressure,
            terrain_scarcity,
            protection_resistance,
            special_regime,
            tourism_seasonality,
            sample_prior_blend,
        },
        land_pressure_index,
        engineering_scale,
        hard_contracts_unchanged: true,
        notes: vec![
            "land pressure changes project priority, candidate competition and macro budget"
                .into(),
            "6m fill, 8% road grade, 25% walk grade, 2m local pad cut and shoreline clearance remain unchanged"
                .into(),
            "large cut, fill, reclamation, tunnels and terraces require explicit engineering operations"
                .into(),
        ],
    }))
}

pub fn route_centrality(routes: &RoutePlan) -> f32 {
    let passing = routes
        .routes
        .iter()
        .filter(|route| route.status == "pass")
        .collect::<Vec<_>>();
    if passing.is_empty() {
        return 0.0;
    }
    let route_factor = (passing.len() as f32 / 6.0).clamp(0.0, 1.0);
    let anchor_factor = (routes.anchors.len() as f32 / 8.0).clamp(0.0, 1.0);
    let total_network_distance: f64 = passing.iter().map(|route| route.length_m).sum();
    let total_direct_distance: f64 = passing.iter().map(|route| route.direct_length_m).sum();
    let detour_factor = if total_direct_distance > 1.0 {
        ((total_network_distance / total_direct_distance - 1.0) / 0.5).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    (0.20 + route_factor * 0.38 + anchor_factor * 0.24 + detour_factor * 0.18).clamp(0.0, 1.0)
}

fn case_id(style: &Value) -> Option<String> {
    style
        .get("referenceData")
        .and_then(|value| value.get("caseIds"))
        .and_then(Value::as_array)
        .and_then(|cases| {
            cases
                .iter()
                .filter_map(Value::as_str)
                .find(|id| urban_case_priors::case(id).ok().flatten().is_some())
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(scale: &str, factors: &[&str], setting: &str, relief: f32) -> Value {
        serde_json::json!({
            "urbanSlice": {
                "version": 1,
                "settlementScale": scale,
                "mode": if scale == "town" { "whole-settlement" } else { "feature-cut" },
                "focusFeature": if scale == "town" { Value::Null } else { Value::String("bay-core".into()) },
                "cropReason": "land pressure test",
                "contextRadiusM": 2500,
                "offMapContinuity": ["roads", "river"],
                "specialFactors": factors
            },
            "naturalGeography": {
                "version": 1,
                "mode": "derived",
                "setting": setting,
                "regionalReliefM": relief,
                "derivationSeed": 42
            }
        })
    }

    #[test]
    fn metropolis_pressure_exceeds_a_small_town() {
        let town = plan(&style("town", &[], "inland-basin", 180.0), None)
            .unwrap()
            .unwrap();
        let metropolis = plan(&style("metropolis", &[], "inland-basin", 180.0), None)
            .unwrap()
            .unwrap();
        assert!(metropolis.land_pressure_index > town.land_pressure_index);
        assert!(metropolis.engineering_scale > town.engineering_scale);
    }

    #[test]
    fn mountain_and_island_scarcity_raise_pressure() {
        let plain = plan(&style("city", &[], "inland-basin", 120.0), None)
            .unwrap()
            .unwrap();
        let mountain = plan(
            &style("city", &["mountainous"], "fold-mountain", 900.0),
            None,
        )
        .unwrap()
        .unwrap();
        assert!(mountain.factors.terrain_scarcity > plain.factors.terrain_scarcity);
        assert!(mountain.land_pressure_index > plain.land_pressure_index);
    }

    #[test]
    fn special_zone_and_tourism_have_separate_coefficients() {
        let sez = plan(
            &style("city", &["special-economic-zone"], "coastal-bay", 100.0),
            None,
        )
        .unwrap()
        .unwrap();
        let tourism = plan(&style("city", &["tourism"], "coastal-bay", 100.0), None)
            .unwrap()
            .unwrap();
        assert!(sez.factors.special_regime > tourism.factors.special_regime);
        assert!(tourism.factors.tourism_seasonality > 1.0);
    }

    #[test]
    fn historic_and_wetland_protection_increase_resistance() {
        let open = plan(&style("city", &[], "inland-basin", 100.0), None)
            .unwrap()
            .unwrap();
        let protected = plan(
            &style("city", &["historic-core"], "river-delta", 100.0),
            None,
        )
        .unwrap()
        .unwrap();
        assert!(protected.factors.protection_resistance > open.factors.protection_resistance);
    }

    #[test]
    fn deterministic_and_local_contracts_are_declared_unchanged() {
        let value = style(
            "metropolis",
            &["financial-center", "coastal-reclamation"],
            "coastal-mountain",
            700.0,
        );
        let first = plan(&value, Some(0.8)).unwrap().unwrap();
        let second = plan(&value, Some(0.8)).unwrap().unwrap();
        assert_eq!(first, second);
        assert!(first.hard_contracts_unchanged);
        assert!(first.notes.iter().any(|note| note.contains("6m fill")));
    }

    #[test]
    fn route_centrality_uses_network_resources_not_a_fixed_constant() {
        use crate::worldgen::route_evolution::{Anchor, Route, RoutePoint};
        let mut routes = RoutePlan {
            format: "route".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.55,
            max_fill_m: 6.0,
            anchors: vec![Anchor {
                id: "a".into(),
                x: 0,
                z: 0,
                role: "junction".into(),
            }],
            routes: vec![Route {
                id: "main".into(),
                from: "a".into(),
                to: "a".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline: vec![RoutePoint {
                    x: 0,
                    z: 0,
                    natural_height_m: 0.0,
                    design_height_m: 0.0,
                    water: false,
                }],
                length_m: 100.0,
                direct_length_m: 100.0,
                curve_deviation_m: 0.0,
                maximum_grade: 0.0,
                maximum_fill_m: 0.0,
                maximum_terrain_fill_m: 0.0,
                fill_volume_m3: 0.0,
                bridge_spans: Vec::new(),
                failure: None,
            }],
            failures: Vec::new(),
        };
        let low = route_centrality(&routes);
        routes.anchors.extend((0..6).map(|index| Anchor {
            id: format!("a{index}"),
            x: index * 100,
            z: 50,
            role: "junction".into(),
        }));
        routes.routes[0].length_m = 160.0;
        let high = route_centrality(&routes);
        assert!(high > low);
    }

    #[test]
    fn invalid_missing_or_bad_geography_fails_closed() {
        let mut value = style("city", &[], "coastal-bay", 100.0);
        value["naturalGeography"]["regionalReliefM"] = serde_json::json!(-5);
        assert!(plan(&value, None).is_err());
        assert!(plan(&serde_json::json!({}), None).unwrap().is_none());
    }
}
