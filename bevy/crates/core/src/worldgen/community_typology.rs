//! Regional community typologies used by capacity-led settlement planning.
//!
//! These are reusable morphological priors, not per-city scripts. They change
//! lot geometry, floor-area intensity, household/job area assumptions, retail
//! share, parking pressure and income-band labels while leaving all physical
//! terrain/access contracts untouched.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Modifiers {
    pub profile: String,
    pub lot_width_scale: f32,
    pub lot_depth_scale: f32,
    pub far_scale: f32,
    pub floors_scale: f32,
    pub household_area_m2: f64,
    pub job_area_m2: f64,
    pub retail_share: f64,
    pub parking_pressure: f64,
    pub enclosed: bool,
}

pub fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("communityProfile") else {
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

pub fn resolve(style: &Value) -> Result<Option<Modifiers>, String> {
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    Ok(Some(match program.profile.as_str() {
        "china-gated" => Modifiers {
            profile: program.profile,
            lot_width_scale: 1.45,
            lot_depth_scale: 1.55,
            far_scale: 0.95,
            floors_scale: 1.0,
            household_area_m2: 92.0,
            job_area_m2: 42.0,
            retail_share: 0.08,
            parking_pressure: 0.75,
            enclosed: true,
        },
        "china-urban-village" => Modifiers {
            profile: program.profile,
            lot_width_scale: 0.48,
            lot_depth_scale: 0.62,
            far_scale: 1.25,
            floors_scale: 1.0,
            household_area_m2: 68.0,
            job_area_m2: 38.0,
            retail_share: 0.24,
            parking_pressure: 0.18,
            enclosed: false,
        },
        "us-suburban" => Modifiers {
            profile: program.profile,
            lot_width_scale: 1.65,
            lot_depth_scale: 1.5,
            far_scale: 0.45,
            floors_scale: 0.42,
            household_area_m2: 185.0,
            job_area_m2: 55.0,
            retail_share: 0.055,
            parking_pressure: 2.0,
            enclosed: false,
        },
        "us-mixed-income" => Modifiers {
            profile: program.profile,
            lot_width_scale: 0.82,
            lot_depth_scale: 0.9,
            far_scale: 0.92,
            floors_scale: 0.82,
            household_area_m2: 112.0,
            job_area_m2: 36.0,
            retail_share: 0.14,
            parking_pressure: 1.15,
            enclosed: false,
        },
        "europe-perimeter" => Modifiers {
            profile: program.profile,
            lot_width_scale: 0.9,
            lot_depth_scale: 0.92,
            far_scale: 1.18,
            floors_scale: 0.9,
            household_area_m2: 88.0,
            job_area_m2: 38.0,
            retail_share: 0.15,
            parking_pressure: 0.45,
            enclosed: false,
        },
        "japan-machiya" => Modifiers {
            profile: program.profile,
            lot_width_scale: 0.52,
            lot_depth_scale: 0.8,
            far_scale: 1.05,
            floors_scale: 0.75,
            household_area_m2: 64.0,
            job_area_m2: 32.0,
            retail_share: 0.22,
            parking_pressure: 0.16,
            enclosed: false,
        },
        "hong-kong-estate" => Modifiers {
            profile: program.profile,
            lot_width_scale: 1.08,
            lot_depth_scale: 0.88,
            far_scale: 1.8,
            floors_scale: 1.5,
            household_area_m2: 54.0,
            job_area_m2: 28.0,
            retail_share: 0.1,
            parking_pressure: 0.22,
            enclosed: true,
        },
        _ => unreachable!("validated profile"),
    }))
}

pub fn income_band(profile: &str, center_distance_m: f64) -> &'static str {
    match profile {
        "us-mixed-income" if center_distance_m < 130.0 => "upper-income",
        "us-mixed-income" if center_distance_m < 280.0 => "middle-income",
        "us-mixed-income" => "workforce",
        "china-gated" | "hong-kong-estate" => "mixed-market-rate",
        "china-urban-village" => "local-resident-and-migrant",
        "us-suburban" => "homeowner",
        _ => "mixed",
    }
}

fn validate(program: &Program) -> Result<(), String> {
    const PROFILES: [&str; 7] = [
        "china-gated",
        "china-urban-village",
        "us-suburban",
        "us-mixed-income",
        "europe-perimeter",
        "japan-machiya",
        "hong-kong-estate",
    ];
    if program.version != 1 || !PROFILES.contains(&program.profile.as_str()) {
        return Err("unsupported communityProfile".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_gated_and_us_suburban_have_distinct_priors() {
        let china = resolve(&serde_json::json!({
            "communityProfile": {"version": 1, "profile": "china-gated"}
        }))
        .unwrap()
        .unwrap();
        let us = resolve(&serde_json::json!({
            "communityProfile": {"version": 1, "profile": "us-suburban"}
        }))
        .unwrap()
        .unwrap();
        assert!(china.enclosed && !us.enclosed);
        assert!(china.lot_width_scale < us.lot_width_scale);
        assert!(china.far_scale > us.far_scale);
        assert!(china.household_area_m2 < us.household_area_m2);
        assert!(us.parking_pressure > china.parking_pressure);
    }

    #[test]
    fn mixed_income_assigns_bands_by_center_distance() {
        assert_eq!(income_band("us-mixed-income", 80.0), "upper-income");
        assert_eq!(income_band("us-mixed-income", 200.0), "middle-income");
        assert_eq!(income_band("us-mixed-income", 400.0), "workforce");
    }

    #[test]
    fn unknown_profile_fails_closed() {
        assert!(
            validate_style(&serde_json::json!({
                "communityProfile": {"version": 1, "profile": "unknown"}
            }))
            .is_err()
        );
    }
}
