//! Population-led programme derivation for urban settlement planning.
//!
//! This module converts a resident/daytime-worker baseline into housing,
//! employment, retail and civic-facility demand. It does not invent a building
//! count: every facility still needs a legal, accessible lot in the settlement
//! planner.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PopulationProgram {
    pub version: u32,
    pub residents: u32,
    pub daytime_workers: u32,
    pub household_size: f32,
    pub retail_floor_area_m2_per_resident: f32,
    pub facilities: Vec<FacilityStandard>,
    #[serde(default)]
    pub inferred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FacilityStandard {
    pub kind: String,
    /// `residents`, `workers`, or `daytime-population`.
    pub basis: String,
    pub population_per_facility: u32,
    pub max_count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PopulationSummary {
    pub residents: u32,
    pub daytime_workers: u32,
    pub daytime_population: u64,
    pub households: u32,
    pub retail_floor_area_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FacilityRequirement {
    pub kind: String,
    pub basis: String,
    pub basis_population: u64,
    pub population_per_facility: u32,
    pub required_count: u32,
    pub delivered_count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
}

pub fn from_settlement_value(value: &Value) -> Result<Option<PopulationProgram>, String> {
    let Some(value) = value.get("populationProgram") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| format!("invalid populationProgram: {error}"))
}

pub fn validate(program: &PopulationProgram) -> Result<(), String> {
    if program.version != 1
        || program.residents == 0
        || !(1.0..=8.0).contains(&program.household_size)
        || !program.retail_floor_area_m2_per_resident.is_finite()
        || !(0.0..=30.0).contains(&program.retail_floor_area_m2_per_resident)
        || program.facilities.len() > 24
    {
        return Err("invalid populationProgram population or intensity".into());
    }
    if program.inferred
        && (program
            .model
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
            || program
                .inference_reason
                .as_deref()
                .is_none_or(|value| value.trim().is_empty()))
    {
        return Err("inferred populationProgram requires model and inferenceReason".into());
    }
    let mut kinds = BTreeSet::new();
    for facility in &program.facilities {
        if !matches!(
            facility.kind.as_str(),
            "school"
                | "clinic"
                | "hospital"
                | "church"
                | "community_center"
                | "sports_center"
                | "stadium"
        ) || !matches!(
            facility.basis.as_str(),
            "residents" | "workers" | "daytime-population"
        ) || !kinds.insert(facility.kind.as_str())
            || facility.population_per_facility == 0
            || facility.max_count == 0
            || facility.max_count > 32
            || !(12..=240).contains(&facility.width_m)
            || !(12..=200).contains(&facility.depth_m)
            || !(4..=80).contains(&facility.height_m)
            || facility.width_m % 2 != 0
            || facility.depth_m % 2 != 0
        {
            return Err("invalid populationProgram facility standard".into());
        }
    }
    Ok(())
}

pub fn derive(program: &PopulationProgram) -> (PopulationSummary, Vec<FacilityRequirement>) {
    let daytime_population = u64::from(program.residents) + u64::from(program.daytime_workers);
    let households =
        (f64::from(program.residents) / f64::from(program.household_size)).ceil() as u32;
    let retail_floor_area_m2 =
        f64::from(program.residents) * f64::from(program.retail_floor_area_m2_per_resident);
    let facilities = program
        .facilities
        .iter()
        .map(|facility| {
            let basis_population = match facility.basis.as_str() {
                "residents" => u64::from(program.residents),
                "workers" => u64::from(program.daytime_workers),
                _ => daytime_population,
            };
            let required_count = basis_population
                .div_ceil(u64::from(facility.population_per_facility))
                .min(u64::from(facility.max_count)) as u32;
            FacilityRequirement {
                kind: facility.kind.clone(),
                basis: facility.basis.clone(),
                basis_population,
                population_per_facility: facility.population_per_facility,
                required_count,
                delivered_count: 0,
                width_m: facility.width_m,
                depth_m: facility.depth_m,
                height_m: facility.height_m,
            }
        })
        .collect();
    (
        PopulationSummary {
            residents: program.residents,
            daytime_workers: program.daytime_workers,
            daytime_population,
            households,
            retail_floor_area_m2,
        },
        facilities,
    )
}

pub fn dimensions(program: &PopulationProgram, kind: &str) -> Option<(u32, u32, u32)> {
    program
        .facilities
        .iter()
        .find(|facility| facility.kind == kind)
        .map(|facility| (facility.width_m, facility.depth_m, facility.height_m))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PopulationProgram {
        PopulationProgram {
            version: 1,
            residents: 45_212,
            daytime_workers: 80_000,
            household_size: 2.825_75,
            retail_floor_area_m2_per_resident: 8.847_4,
            facilities: vec![
                FacilityStandard {
                    kind: "school".into(),
                    basis: "residents".into(),
                    population_per_facility: 25_000,
                    max_count: 6,
                    width_m: 64,
                    depth_m: 44,
                    height_m: 14,
                },
                FacilityStandard {
                    kind: "hospital".into(),
                    basis: "daytime-population".into(),
                    population_per_facility: 150_000,
                    max_count: 2,
                    width_m: 88,
                    depth_m: 64,
                    height_m: 36,
                },
            ],
            inferred: false,
            model: None,
            inference_reason: None,
        }
    }

    #[test]
    fn derives_demand_instead_of_treating_building_count_as_target() {
        let (summary, facilities) = derive(&fixture());
        assert_eq!(summary.households, 16_000);
        assert_eq!(summary.daytime_population, 125_212);
        assert_eq!(facilities[0].required_count, 2);
        assert_eq!(facilities[1].required_count, 1);
        assert_eq!(facilities[0].delivered_count, 0);
    }

    #[test]
    fn validation_rejects_duplicate_or_under_sized_facilities() {
        let mut program = fixture();
        program.facilities.push(program.facilities[0].clone());
        assert!(validate(&program).unwrap_err().contains("facility"));
        let mut program = fixture();
        program.facilities[0].width_m = 2;
        assert!(validate(&program).is_err());
    }
}
