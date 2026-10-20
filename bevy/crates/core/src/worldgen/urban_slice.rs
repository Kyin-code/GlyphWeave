//! Urban scale and slice semantics.
//!
//! A generated scene is normally a cut through a larger settlement, not a
//! miniature copy of an entire metropolis. This module validates that intent
//! and publishes the scale/special-factor coefficients consumed by later route,
//! epoch, land-pressure and engineering stages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub settlement_scale: String,
    pub mode: String,
    #[serde(default)]
    pub focus_feature: Option<String>,
    pub crop_reason: String,
    pub context_radius_m: f32,
    #[serde(default)]
    pub off_map_continuity: Vec<String>,
    #[serde(default)]
    pub special_factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlement_scale: String,
    pub mode: String,
    pub focus_feature: Option<String>,
    pub crop_reason: String,
    pub context_radius_m: f32,
    pub off_map_continuity: Vec<String>,
    pub special_factors: Vec<String>,
    pub scale_factor: f32,
    pub land_pressure_index: f32,
    pub engineering_scale: f32,
    pub allow_whole_settlement: bool,
    pub notes: Vec<String>,
}

pub fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("urbanSlice") else {
        return Ok(None);
    };
    let program: Program =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    validate(&program)?;
    Ok(Some(program))
}

pub fn validate_style(style: &Value) -> Result<(), String> {
    from_style(style).map(|_| ())
}

pub fn compile(style: &Value) -> Result<Option<Plan>, String> {
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    let scale_factor = match program.settlement_scale.as_str() {
        "village" => 0.20,
        "town" => 0.35,
        "county-city" => 0.52,
        "city" => 0.70,
        "metropolis" => 0.92,
        "megacity" => 1.08,
        _ => unreachable!("validated settlement scale"),
    };
    let factor_sum: f32 = program
        .special_factors
        .iter()
        .map(|factor| match factor.as_str() {
            "special-economic-zone" => 0.20,
            "tourism" => 0.08,
            "mountainous" => 0.24,
            "historic-core" => 0.16,
            "coastal-reclamation" => 0.22,
            "island" => 0.20,
            "political-capital" => 0.12,
            "financial-center" => 0.20,
            "port-industrial" => 0.16,
            _ => 0.0,
        })
        .sum();
    let land_pressure_index = (scale_factor + factor_sum).clamp(0.15, 1.80);
    let engineering_scale = (0.45 + land_pressure_index * 0.85).clamp(0.45, 2.0);
    let allow_whole_settlement = matches!(
        program.settlement_scale.as_str(),
        "village" | "town" | "county-city"
    );
    let mut notes = vec![
        "landPressureIndex scales demand and candidate competition; it does not relax local pad, road, water or protection contracts"
            .into(),
        "engineeringScale sizes explicit macro projects and budget pressure; every project still records footprint, volume, hazard and hydrology"
            .into(),
    ];
    if !allow_whole_settlement {
        notes.push(
            "metropolitan scales are represented as a feature/detail cut with off-map continuity, never a closed miniature city"
                .into(),
        );
    }
    Ok(Some(Plan {
        format: "glyphweave.urban-slice-plan".into(),
        version: 1,
        status: "compiled".into(),
        settlement_scale: program.settlement_scale,
        mode: program.mode,
        focus_feature: program.focus_feature,
        crop_reason: program.crop_reason,
        context_radius_m: program.context_radius_m,
        off_map_continuity: program.off_map_continuity,
        special_factors: program.special_factors,
        scale_factor,
        land_pressure_index,
        engineering_scale,
        allow_whole_settlement,
        notes,
    }))
}

fn validate(program: &Program) -> Result<(), String> {
    if program.version != 1
        || !matches!(
            program.settlement_scale.as_str(),
            "village" | "town" | "county-city" | "city" | "metropolis" | "megacity"
        )
        || !matches!(
            program.mode.as_str(),
            "whole-settlement" | "district-cut" | "feature-cut"
        )
        || program.crop_reason.trim().is_empty()
        || !(64.0..=10_000.0).contains(&program.context_radius_m)
    {
        return Err("invalid urbanSlice scale/mode/context".into());
    }
    if !matches!(
        program.settlement_scale.as_str(),
        "village" | "town" | "county-city"
    ) && program.mode == "whole-settlement"
    {
        return Err(
            "city/metropolis/megacity cannot claim whole-settlement generation".into(),
        );
    }
    if program.mode == "feature-cut"
        && program
            .focus_feature
            .as_deref()
            .is_none_or(|feature| feature.trim().is_empty())
    {
        return Err("feature-cut requires focusFeature".into());
    }
    let continuity = ["roads", "rail", "river", "urban-fabric", "coast"];
    if program
        .off_map_continuity
        .iter()
        .any(|item| !continuity.contains(&item.as_str()))
    {
        return Err("unsupported offMapContinuity item".into());
    }
    let factors = [
        "special-economic-zone",
        "tourism",
        "mountainous",
        "historic-core",
        "coastal-reclamation",
        "island",
        "political-capital",
        "financial-center",
        "port-industrial",
    ];
    if program
        .special_factors
        .iter()
        .any(|factor| !factors.contains(&factor.as_str()))
    {
        return Err("unsupported urbanSlice special factor".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn style() -> Value {
        json!({
            "urbanSlice": {
                "version": 1,
                "settlementScale": "metropolis",
                "mode": "feature-cut",
                "focusFeature": "bay-waterfront",
                "cropReason": "deep bay and financial core relation",
                "contextRadiusM": 2500,
                "offMapContinuity": ["roads", "rail", "river", "urban-fabric"],
                "specialFactors": [
                    "financial-center",
                    "coastal-reclamation",
                    "special-economic-zone"
                ]
            }
        })
    }

    #[test]
    fn metropolis_slice_compiles_pressure_and_engineering_coefficients() {
        let plan = compile(&style()).unwrap().unwrap();
        assert!(!plan.allow_whole_settlement);
        assert_eq!(plan.focus_feature.as_deref(), Some("bay-waterfront"));
        assert!(plan.land_pressure_index > 1.0);
        assert!(plan.engineering_scale > 1.0);
        assert!(plan.notes.iter().any(|note| note.contains("off-map")));
    }

    #[test]
    fn metropolis_cannot_claim_a_whole_city() {
        let mut bad = style();
        bad["urbanSlice"]["mode"] = json!("whole-settlement");
        assert!(compile(&bad).is_err());
        let mut no_feature = style();
        no_feature["urbanSlice"]
            .as_object_mut()
            .unwrap()
            .remove("focusFeature");
        assert!(compile(&no_feature).is_err());
    }
}
