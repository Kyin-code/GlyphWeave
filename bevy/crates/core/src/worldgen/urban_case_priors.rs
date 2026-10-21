//! Machine-readable core-cut priors distilled from the 48-case survey.
//!
//! These priors do not copy a real city. They provide a geography sketch,
//! development timeline, district structure, centre roles and pressure factors
//! that an agent can combine with a selected crop.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PRIORS_JSON: &str = include_str!("../../../../../assets/data/urban-case-priors-48.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    pub format: String,
    pub version: u32,
    pub schema: String,
    pub generated_by: String,
    pub date: String,
    pub cases: Vec<CasePrior>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CasePrior {
    pub id: String,
    pub region: String,
    pub core_cut: String,
    pub natural_geography: NaturalGeographyPrior,
    pub history: Vec<HistoryStage>,
    pub urban_structure: UrbanStructurePrior,
    pub focus_feature: String,
    pub community_profile: String,
    pub special_factors: Vec<String>,
    pub land_pressure_prior: LandPressurePrior,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NaturalGeographyPrior {
    pub setting: String,
    pub regional_relief_m: f32,
    pub climate_band: String,
    pub geomorphic_processes: Vec<String>,
    pub hydrography: String,
    pub hazards: Vec<String>,
    pub evidence_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryStage {
    pub order: u32,
    pub era: String,
    pub change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UrbanStructurePrior {
    pub old_core: String,
    pub renewal: String,
    pub new_city: String,
    pub edge: String,
    pub centres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandPressurePrior {
    pub settlement_scale: String,
    pub terrain_scarcity: f64,
    pub protection_resistance: f64,
    pub special_regime: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub sources: Vec<String>,
    pub confidence: String,
}

pub fn load() -> Result<Registry, String> {
    let registry: Registry =
        serde_json::from_str(PRIORS_JSON).map_err(|error| error.to_string())?;
    validate(&registry)?;
    Ok(registry)
}

pub fn case(id: &str) -> Result<Option<CasePrior>, String> {
    Ok(load()?.cases.into_iter().find(|case| case.id == id))
}

pub fn validate(registry: &Registry) -> Result<(), String> {
    if registry.format != "glyphweave.urban-case-priors"
        || registry.version != 2
        || registry.schema != "core-cut-history-v2"
        || registry.cases.len() != 48
    {
        return Err("invalid 48-case prior registry envelope".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut regions = std::collections::BTreeMap::<&str, usize>::new();
    for case in &registry.cases {
        if case.id.trim().is_empty()
            || !ids.insert(case.id.as_str())
            || case.core_cut.trim().is_empty()
            || case.focus_feature.trim().is_empty()
            || case.natural_geography.setting.trim().is_empty()
            || case.natural_geography.geomorphic_processes.is_empty()
            || case.history.len() < 3
            || case.urban_structure.centres.len() < 2
        {
            return Err(format!("invalid or duplicate case prior {}", case.id));
        }
        if case
            .history
            .iter()
            .enumerate()
            .any(|(index, stage)| stage.order != index as u32 + 1)
        {
            return Err(format!("history order is invalid for {}", case.id));
        }
        if !(0.2..=2.5).contains(&case.land_pressure_prior.terrain_scarcity)
            || !(0.2..=2.5).contains(&case.land_pressure_prior.protection_resistance)
            || !(0.2..=2.5).contains(&case.land_pressure_prior.special_regime)
        {
            return Err(format!("pressure prior is invalid for {}", case.id));
        }
        *regions.entry(case.region.as_str()).or_default() += 1;
    }
    let expected = [
        ("china", 12),
        ("united-states", 12),
        ("europe", 12),
        ("japan", 6),
        ("hong-kong", 6),
    ];
    if expected
        .iter()
        .any(|(region, count)| regions.get(region).copied() != Some(*count))
    {
        return Err("regional case distribution is invalid".into());
    }
    Ok(())
}

pub fn natural_geography_sketch(case: &CasePrior) -> Value {
    let setting = core_natural_setting(&case.natural_geography.setting);
    json!({
        "version": 1,
        "mode": "derived",
        "setting": setting,
        "regionalReliefM": case.natural_geography.regional_relief_m,
        "climateBand": &case.natural_geography.climate_band,
        "derivationSeed": stable_seed(&case.id),
        "hints": {
            "caseId": &case.id,
            "caseSetting": &case.natural_geography.setting,
            "normalizedFromCasePrior": setting != case.natural_geography.setting,
            "geomorphicProcesses": &case.natural_geography.geomorphic_processes,
            "hydrography": &case.natural_geography.hydrography,
            "hazards": &case.natural_geography.hazards
        }
    })
}

fn core_natural_setting(case_setting: &str) -> &'static str {
    let value = case_setting.to_ascii_lowercase();
    if value.contains("delta")
        || (value.contains("alluvial")
            && (value.contains("river") || value.contains("lake") || value.contains("flood")))
    {
        "river-delta"
    } else if value.contains("tidal") {
        "tidal-flat"
    } else if value.contains("ria") {
        "ria-bay"
    } else if value.contains("volcan") {
        "volcanic-island"
    } else if value.contains("island") {
        "island"
    } else if value.contains("plateau") {
        "plateau-basin"
    } else if value.contains("rift") || value.contains("valley") {
        "rift-valley"
    } else if value.contains("fold") || value.contains("mountain") || value.contains("mass-wasting")
    {
        "coastal-mountain"
    } else if value.contains("bay") || value.contains("coast") || value.contains("harbour") {
        "coastal-bay"
    } else if value.contains("steppe")
        || value.contains("plain")
        || value.contains("desert")
        || value.contains("basin")
    {
        "inland-steppe"
    } else {
        "inland-basin"
    }
}

fn inferred_district_shares(case: &CasePrior) -> Value {
    let mut weights = [0.24_f64, 0.34, 0.32, 0.10];
    for (index, stage) in case.history.iter().enumerate() {
        let text = format!("{} {}", stage.era, stage.change).to_ascii_lowercase();
        if index == 0
            && ["water", "canal", "river", "port", "agri", "village", "fort"]
                .iter()
                .any(|word| text.contains(word))
        {
            weights[0] += 0.04;
        }
        if [
            "rail",
            "railway",
            "industrial",
            "factory",
            "warehouse",
            "manufactur",
            "depot",
        ]
        .iter()
        .any(|word| text.contains(word))
        {
            weights[1] += 0.04;
        }
        if [
            "renewal",
            "redevelop",
            "preserve",
            "conserv",
            "reconstruct",
            "infill",
            "urban village",
        ]
        .iter()
        .any(|word| text.contains(word))
        {
            weights[1] += 0.03;
        }
        if [
            "metro", "new town", "tram", "airport", "cbd", "tower", "suburb", "reform",
        ]
        .iter()
        .any(|word| text.contains(word))
        {
            weights[2] += 0.04;
        }
        if ["tourism", "country park", "reservoir"]
            .iter()
            .any(|word| text.contains(word))
        {
            weights[3] += 0.03;
        }
    }
    let total: f64 = weights.iter().sum();
    let ratios: Vec<f64> = weights.iter().map(|weight| weight / total).collect();
    json!({
        "version": 1,
        "model": "case-history-keyword-heuristic-v1",
        "inferred": true,
        "inferenceReason": format!(
            "normalized from the three documented development stages of {}",
            case.id
        ),
        "allowSpill": true,
        "shares": [
            {"stage": "old-core", "share": ratios[0]},
            {"stage": "renewal", "share": ratios[1]},
            {"stage": "new-growth", "share": ratios[2]},
            {"stage": "periphery", "share": ratios[3]}
        ]
    })
}

pub fn urban_slice(case: &CasePrior) -> Value {
    let whole = matches!(
        case.land_pressure_prior.settlement_scale.as_str(),
        "village" | "town" | "county-city"
    );
    json!({
        "version": 1,
        "settlementScale": &case.land_pressure_prior.settlement_scale,
        "mode": if whole { "whole-settlement" } else { "feature-cut" },
        "focusFeature": if whole { Value::Null } else { Value::String(case.focus_feature.clone()) },
        "cropReason": &case.core_cut,
        "contextRadiusM": if whole { 1200.0 } else { 2500.0 },
        "offMapContinuity": ["roads", "river", "urban-fabric"],
        "specialFactors": &case.special_factors
    })
}

pub fn pressure_prior(case: &CasePrior) -> Value {
    serde_json::to_value(&case.land_pressure_prior).unwrap_or(Value::Null)
}

/// Resolve a referenced 48-case prior into the style fields the generator
/// consumes. Authored fields always win; this fills only absent values.
pub fn compose_style(style: &Value) -> Result<Value, String> {
    let Some(case_ids) = style
        .get("referenceData")
        .and_then(|reference| reference.get("caseIds"))
        .and_then(Value::as_array)
    else {
        return Ok(style.clone());
    };
    let Some(case_id) = case_ids.first().and_then(Value::as_str) else {
        return Err("referenceData.caseIds must contain at least one string".into());
    };
    let prior = case(case_id)?
        .ok_or_else(|| format!("referenceData.caseIds references unknown case {case_id}"))?;
    let mut composed = style.clone();
    let object = composed
        .as_object_mut()
        .ok_or_else(|| "style must be an object".to_string())?;
    object
        .entry("naturalGeography")
        .or_insert_with(|| natural_geography_sketch(&prior));
    object
        .entry("urbanSlice")
        .or_insert_with(|| urban_slice(&prior));
    object.entry("communityProfile").or_insert_with(|| {
        json!({
            "version": 1,
            "profile": &prior.community_profile,
            "inferred": true,
            "inferenceReason": format!("community profile from urban case {}", prior.id)
        })
    });
    let inferred_shares = inferred_district_shares(&prior);
    if let Some(settlement) = object
        .get_mut("settlementEvolution")
        .and_then(Value::as_object_mut)
    {
        settlement
            .entry("districtDemand")
            .or_insert_with(|| inferred_shares.clone());
    }
    object.insert(
        "urbanCasePrior".into(),
        json!({
            "id": &prior.id,
            "region": &prior.region,
            "coreCut": &prior.core_cut,
            "focusFeature": &prior.focus_feature,
            "history": &prior.history,
            "inferredDistrictDemand": inferred_shares,
            "contextCaseIds": case_ids,
            "evidenceConfidence": &prior.evidence.confidence,
        }),
    );
    Ok(composed)
}

fn stable_seed(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_all_48_valid_core_cuts() {
        let registry = load().unwrap();
        assert_eq!(registry.cases.len(), 48);
        assert_eq!(
            registry
                .cases
                .iter()
                .filter(|case| case.region == "china")
                .count(),
            12
        );
        assert!(registry.cases.iter().all(|case| case.history.len() >= 3));
    }

    #[test]
    fn representative_geographies_are_distinct() {
        let chongqing = case("cn-chongqing").unwrap().unwrap();
        let amsterdam = case("eu-amsterdam").unwrap().unwrap();
        let las_vegas = case("us-las-vegas").unwrap().unwrap();
        assert_eq!(
            chongqing.natural_geography.setting,
            "fold-mountain-river-confluence"
        );
        assert_eq!(amsterdam.natural_geography.setting, "low-delta-wetland");
        assert_eq!(las_vegas.natural_geography.setting, "desert-basin");
        assert!(chongqing
            .natural_geography
            .geomorphic_processes
            .contains(&"mass-wasting".to_string()));
        assert!(las_vegas
            .natural_geography
            .geomorphic_processes
            .contains(&"aeolian-erosion".to_string()));
    }

    #[test]
    fn case_presets_are_ready_for_manifest_composition() {
        let shanghai = case("cn-shanghai").unwrap().unwrap();
        let natural = natural_geography_sketch(&shanghai);
        let slice = urban_slice(&shanghai);
        assert_eq!(natural["mode"], "derived");
        assert_eq!(natural["setting"], "river-delta");
        assert_eq!(slice["settlementScale"], "megacity");
        assert_eq!(slice["mode"], "feature-cut");
        assert_eq!(slice["focusFeature"], "lilong-to-pudong-waterfront");
        assert!(
            pressure_prior(&shanghai)["terrainScarcity"]
                .as_f64()
                .unwrap()
                > 1.0
        );
    }

    #[test]
    fn unknown_case_is_absent_not_silently_defaulted() {
        assert!(case("not-a-city").unwrap().is_none());
    }

    #[test]
    fn referenced_case_composes_absent_style_fields_and_keeps_authored_ones() {
        let style = json!({
            "referenceData": {"caseIds": ["cn-chongqing"]},
            "urbanSlice": {"version": 1, "settlementScale": "custom-authored"},
            "settlementEvolution": {
                "version": 1,
                "householdsTarget": 12,
                "jobsTarget": 4,
                "retailFloorAreaM2": 80.0
            }
        });
        let composed = compose_style(&style).unwrap();
        assert_eq!(composed["naturalGeography"]["setting"], "coastal-mountain");
        assert_eq!(
            composed["naturalGeography"]["hints"]["caseSetting"],
            "fold-mountain-river-confluence"
        );
        assert_eq!(composed["urbanSlice"]["settlementScale"], "custom-authored");
        assert_eq!(composed["communityProfile"]["profile"], "china-gated");
        assert_eq!(
            composed["urbanCasePrior"]["history"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let shares = composed["settlementEvolution"]["districtDemand"]["shares"]
            .as_array()
            .unwrap();
        assert_eq!(shares.len(), 4);
        let total: f64 = shares
            .iter()
            .map(|share| share["share"].as_f64().unwrap())
            .sum();
        assert!((total - 1.0).abs() < 1e-9);
        assert_eq!(
            composed["settlementEvolution"]["districtDemand"]["model"],
            "case-history-keyword-heuristic-v1"
        );
        crate::worldgen::settlement_evolution::validate_style(&composed)
            .expect("inferred case demand must be consumable by settlement evolution");
    }

    #[test]
    fn authored_district_demand_is_not_replaced_by_case_inference() {
        let authored = json!([
            {"stage": "old-core", "share": 0.10},
            {"stage": "renewal", "share": 0.20},
            {"stage": "new-growth", "share": 0.60},
            {"stage": "periphery", "share": 0.10}
        ]);
        let style = json!({
            "referenceData": {"caseIds": ["cn-shanghai"]},
            "settlementEvolution": {
                "version": 1,
                "householdsTarget": 12,
                "jobsTarget": 4,
                "retailFloorAreaM2": 80.0,
                "districtDemand": {
                    "version": 1,
                    "allowSpill": false,
                    "shares": authored
                }
            }
        });
        let composed = compose_style(&style).unwrap();
        assert_eq!(
            composed["settlementEvolution"]["districtDemand"]["shares"],
            authored
        );
        assert_eq!(
            composed["urbanCasePrior"]["inferredDistrictDemand"]["inferred"],
            true
        );
    }

    #[test]
    fn referenced_unknown_case_fails_closed() {
        let style = json!({"referenceData": {"caseIds": ["missing-case"]}});
        assert!(compose_style(&style).unwrap_err().contains("unknown case"));
    }
}
