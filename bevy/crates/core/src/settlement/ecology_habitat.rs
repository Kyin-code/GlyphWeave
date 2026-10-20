//! C-N1 habitat hook: landform and terrain influence ecology suitability.
//!
//! This is deliberately a pure, independent interface. It does not mutate a
//! world and is not yet the final biome model; the existing conservation
//! material pass can call it when a future profile opts into habitat-driven
//! layers. Keeping it pure makes the rule testable and lets the Rust core,
//! HTML preview and Godot adapter share one explanation of why a patch is
//! forest, shrubland, wetland or meadow.
use serde::{Deserialize, Serialize};

const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandformClass {
    Plain,
    Valley,
    Hill,
    Mountain,
    Coast,
    Wetland,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HabitatContext {
    /// Rise over run, 0 = flat.
    pub slope: f64,
    /// Elevation in metres above the scene datum.
    pub elevation_m: f64,
    /// Distance to the nearest authored water body, metres.
    pub water_distance_m: f64,
    pub landform: LandformClass,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HabitatKind {
    Forest,
    Shrubland,
    Wetland,
    Meadow,
    Rock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HabitatSuitability {
    pub forest: f64,
    pub shrubland: f64,
    pub wetland: f64,
    pub meadow: f64,
    pub rock: f64,
    pub dominant: HabitatKind,
}

impl HabitatContext {
    fn validate(self) -> Result<(), String> {
        if !self.slope.is_finite()
            || !self.elevation_m.is_finite()
            || !self.water_distance_m.is_finite()
            || self.slope < 0.
            || self.water_distance_m < 0.
        {
            return Err("habitat context requires finite non-negative slope/water distance".into());
        }
        Ok(())
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0., 1.)
}

/// Deterministic first-order suitability. The rules are intentionally simple
/// and visible: steep ground cannot be wetland, near-water flat ground favours
/// wetland, and high/mountain ground shifts away from forest toward shrub/rock.
pub fn classify(context: HabitatContext) -> Result<HabitatSuitability, String> {
    context.validate()?;
    let slope = context.slope;
    let elevation = context.elevation_m.max(0.);
    let water = context.water_distance_m;
    let mut forest = clamp01(1. - slope / 0.85);
    let mut shrubland = clamp01(0.12 + slope / 0.45);
    let mut rock = clamp01(-0.15 + slope / 0.55 + elevation / 5_000.);
    let mut meadow = clamp01(1. - (slope - 0.08).abs() / 0.45);
    let mut wetland = if slope > 0.05 || water > 60. {
        0.
    } else {
        clamp01(1. - water / 60. - slope / 0.05)
    };
    match context.landform {
        LandformClass::Plain => {
            meadow += 0.18;
            forest += 0.04;
        }
        LandformClass::Valley => {
            meadow += 0.10;
            wetland += if water < 80. { 0.20 } else { 0. };
            rock -= 0.10;
        }
        LandformClass::Hill => {
            shrubland += 0.12;
            forest -= 0.06;
        }
        LandformClass::Mountain => {
            rock += 0.28;
            shrubland += 0.14;
            forest -= 0.18;
            meadow -= 0.16;
        }
        LandformClass::Coast => {
            wetland += if water < 40. { 0.18 } else { 0. };
            meadow += 0.06;
        }
        LandformClass::Wetland => {
            wetland += 0.35;
            meadow += 0.05;
            rock = 0.;
        }
    }
    // A deep-water or very high site should not produce an impossible
    // dominant class merely because all scores clamp to 1.
    if slope > 0.05 || water > 60. {
        wetland = 0.;
    }
    let scores = [
        (HabitatKind::Forest, clamp01(forest)),
        (HabitatKind::Shrubland, clamp01(shrubland)),
        (HabitatKind::Wetland, clamp01(wetland)),
        (HabitatKind::Meadow, clamp01(meadow)),
        (HabitatKind::Rock, clamp01(rock)),
    ];
    let tie_rank = |kind: HabitatKind| match kind {
        HabitatKind::Wetland => 5,
        HabitatKind::Meadow => 4,
        HabitatKind::Forest => 3,
        HabitatKind::Shrubland => 2,
        HabitatKind::Rock => 1,
    };
    let dominant = scores
        .iter()
        .max_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| tie_rank(a.0).cmp(&tie_rank(b.0)))
        })
        .map(|(kind, _)| *kind)
        .ok_or("empty habitat scores")?;
    Ok(HabitatSuitability {
        forest: scores[0].1,
        shrubland: scores[1].1,
        wetland: scores[2].1,
        meadow: scores[3].1,
        rock: scores[4].1,
        dominant,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(slope: f64, elevation: f64, water: f64, landform: LandformClass) -> HabitatContext {
        HabitatContext {
            slope,
            elevation_m: elevation,
            water_distance_m: water,
            landform,
        }
    }

    #[test]
    fn steep_ground_rejects_wetland() {
        let out = classify(context(0.5, 20., 2., LandformClass::Wetland)).unwrap();
        assert_eq!(out.wetland, 0.);
        assert_ne!(out.dominant, HabitatKind::Wetland);
    }

    #[test]
    fn near_water_flat_ground_favours_wetland() {
        let out = classify(context(0.01, 5., 4., LandformClass::Wetland)).unwrap();
        assert!(out.wetland > 0.5);
        assert_eq!(out.dominant, HabitatKind::Wetland);
    }

    #[test]
    fn mountain_steep_site_prefers_shrub_or_rock_over_forest() {
        let out = classify(context(0.8, 1_200., 500., LandformClass::Mountain)).unwrap();
        assert!(out.forest <= out.shrubland);
        assert!(out.forest <= out.rock);
        assert!(matches!(
            out.dominant,
            HabitatKind::Shrubland | HabitatKind::Rock
        ));
    }

    #[test]
    fn classification_is_deterministic() {
        let ctx = context(0.2, 300., 30., LandformClass::Hill);
        assert_eq!(classify(ctx), classify(ctx));
    }

    #[test]
    fn invalid_context_is_refused() {
        for ctx in [
            context(f64::NAN, 0., 0., LandformClass::Plain),
            context(-0.1, 0., 0., LandformClass::Plain),
            context(0., 0., -1., LandformClass::Plain),
        ] {
            assert!(classify(ctx).is_err());
        }
    }
}
