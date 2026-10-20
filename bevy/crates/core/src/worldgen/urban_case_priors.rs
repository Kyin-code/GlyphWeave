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
    json!({
        "version": 1,
        "mode": "derived",
        "setting": &case.natural_geography.setting,
        "regionalReliefM": case.natural_geography.regional_relief_m,
        "climateBand": &case.natural_geography.climate_band,
        "derivationSeed": stable_seed(&case.id),
        "hints": {
            "caseId": &case.id,
            "geomorphicProcesses": &case.natural_geography.geomorphic_processes,
            "hydrography": &case.natural_geography.hydrography,
            "hazards": &case.natural_geography.hazards
        }
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
}
