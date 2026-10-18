//! Explicit B residual-land policy. Ratios select real, bounded sub-use
//! surfaces inside the original purpose; the remainder stays unresolved.
//!
//! Each ratio is an independent statement about one sub-use. A scene may state
//! only the public open-space share: the residual-land consumer's named default
//! for an unstated ratio is [`ABSENT_RATIO_DEFAULT`] (zero), so an absent
//! pasture share means "no pasture is planned", never "someone else's ratio
//! applies here". The default is visible in the value domain (absent == 0) and
//! in the audit report ([`SceneResidualProgram::unstated_ratios`] names every
//! ratio the source left unstated), so a reader never has to infer it.
//!
//! Presence is preserved rather than flattened to zero: `Some(0.)` is a stated
//! policy and `None` is an unstated one, so an audit can tell a deleted field
//! from a field the source never wrote. Both consume the same area.
use crate::worldgen::SceneSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Named default the consumer applies to a ratio the source never stated.
pub const ABSENT_RATIO_DEFAULT: f64 = 0.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneResidualProgram {
    /// Public open-space share inside the scene's urban remainder. Unstated
    /// means [`ABSENT_RATIO_DEFAULT`]; it is the only field a profile's
    /// `publicSpaceRatio` may fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urban_open_space_ratio: Option<f64>,
    /// Pasture share inside the scene's rural remainder. Unstated means
    /// [`ABSENT_RATIO_DEFAULT`]; no profile field supplies this value because
    /// a public-space share is not a pasture share.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rural_pasture_ratio: Option<f64>,
}
impl SceneResidualProgram {
    /// Effective urban share: the stated value, or the named default.
    pub fn urban_open_space_ratio(&self) -> f64 {
        self.urban_open_space_ratio.unwrap_or(ABSENT_RATIO_DEFAULT)
    }
    /// Effective rural pasture share: the stated value, or the named default.
    pub fn rural_pasture_ratio(&self) -> f64 {
        self.rural_pasture_ratio.unwrap_or(ABSENT_RATIO_DEFAULT)
    }
    /// Ratios this policy leaves unstated, in a fixed order. Reports name them
    /// together with [`ABSENT_RATIO_DEFAULT`]; nothing is silently zeroed.
    pub fn unstated_ratios(&self) -> Vec<&'static str> {
        [
            (self.urban_open_space_ratio.is_none(), "urbanOpenSpaceRatio"),
            (self.rural_pasture_ratio.is_none(), "ruralPastureRatio"),
        ]
        .into_iter()
        .filter_map(|(unstated, name)| unstated.then_some(name))
        .collect()
    }
    pub fn validate(&self) -> Result<(), String> {
        // An unstated ratio consumes nothing, so it can never be out of bounds.
        for (value, bound, field) in [
            (self.urban_open_space_ratio, 0.25, "urbanOpenSpaceRatio"),
            (self.rural_pasture_ratio, 0.50, "ruralPastureRatio"),
        ] {
            let Some(value) = value else { continue };
            if !value.is_finite() || !(0. ..=bound).contains(&value) {
                return Err(format!(
                    "residual {field} must be finite within 0..={bound}"
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidualLandProgram {
    pub version: u32,
    pub scenes: BTreeMap<String, SceneResidualProgram>,
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<ResidualLandProgram>, String> {
    let Some(value) = style.get("residualLandProgram") else {
        return Ok(None);
    };
    let program: ResidualLandProgram = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid residualLandProgram: {error}"))?;
    if program.version != 1 || program.scenes.is_empty() || program.scenes.len() > 64 {
        return Err("residualLandProgram requires version1 and 1..=64 scenes".into());
    }
    for (scene_id, scene) in &program.scenes {
        if scene_id.is_empty() {
            return Err("empty residualLandProgram scene id".into());
        }
        scene.validate()?;
    }
    Ok(Some(program))
}
pub fn for_scene(
    style: &serde_json::Value,
    scene: &SceneSpec,
) -> Result<Option<SceneResidualProgram>, String> {
    Ok(from_style(style)?.and_then(|p| p.scenes.get(&scene.scene_id).cloned()))
}
/// Bind a published/scene residual policy to the frozen manifest source.
/// The expected value is never copied from the object being audited.
pub fn audit_source(
    actual: Option<&SceneResidualProgram>,
    style: &serde_json::Value,
    scene: &SceneSpec,
) -> Result<Option<SceneResidualProgram>, String> {
    let expected = for_scene(style, scene)?;
    if actual != expected.as_ref() {
        return Err("residualLandProgram differs from authoritative manifest/scene".into());
    }
    Ok(expected)
}
pub fn validate_manifest(style: &serde_json::Value, scenes: &[SceneSpec]) -> Result<(), String> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    for id in program.scenes.keys() {
        if !scenes.iter().any(|scene| &scene.scene_id == id) {
            return Err(format!("residualLandProgram refers to unknown scene {id}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "s".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }
    fn urban_only(ratio: f64) -> serde_json::Value {
        serde_json::json!({
            "residualLandProgram": {
                "version": 1,
                "scenes": {"s": {"urbanOpenSpaceRatio": ratio}}
            }
        })
    }
    #[test]
    fn ratios_are_explicit_bounded_and_legacy_default_is_off() {
        assert!(from_style(&serde_json::json!({})).unwrap().is_none());
        assert!(from_style(&serde_json::json!({"residualLandProgram":{"version":1,"scenes":{"s":{"urbanOpenSpaceRatio":0.1,"ruralPastureRatio":0.2}}}})).unwrap().is_some());
        for value in [-0.1, 0.26, f64::NAN] {
            assert!(SceneResidualProgram {
                urban_open_space_ratio: Some(value),
                rural_pasture_ratio: Some(0.),
            }
            .validate()
            .is_err());
        }
        assert!(SceneResidualProgram {
            urban_open_space_ratio: Some(0.25),
            rural_pasture_ratio: Some(0.5),
        }
        .validate()
        .is_ok());
    }
    /// A public-space-only policy is legal and consumes exactly one share: the
    /// generator's named default (0) covers the ratio the source never stated.
    #[test]
    fn a_public_space_only_policy_defaults_the_unstated_pasture_share_to_zero() {
        let scene = scene();
        let program = for_scene(&urban_only(0.1), &scene).unwrap().unwrap();
        assert_eq!(program.urban_open_space_ratio(), 0.1);
        assert_eq!(program.rural_pasture_ratio(), 0.0);
        assert_eq!(program.rural_pasture_ratio, None, "unstated, not stated as 0");
        assert_eq!(program.unstated_ratios(), vec!["ruralPastureRatio"]);
        program.validate().unwrap();
        // The unstated default is part of the document: only the stated share
        // is serialized, so a reader sees the gap instead of a synthetic 0.
        let value = serde_json::to_value(&program).unwrap();
        assert_eq!(value, serde_json::json!({"urbanOpenSpaceRatio": 0.1}));
        // A stated zero is a different statement from an unstated one.
        let stated_zero = SceneResidualProgram {
            urban_open_space_ratio: Some(0.1),
            rural_pasture_ratio: Some(0.0),
        };
        assert!(stated_zero.unstated_ratios().is_empty());
        assert_eq!(stated_zero.rural_pasture_ratio(), 0.0);
        assert_ne!(stated_zero, program);
        // Bounds still fail closed on the stated share alone. JSON cannot carry
        // a non-finite number, so `validate` is the only guard for those and it
        // is exercised through the struct directly above.
        for bad in [-0.01, 0.2501, 1.5] {
            assert!(for_scene(&urban_only(bad), &scene).is_err(), "{bad}");
        }
        assert!(SceneResidualProgram {
            urban_open_space_ratio: Some(f64::INFINITY),
            rural_pasture_ratio: None,
        }
        .validate()
        .is_err());
    }
    #[test]
    fn source_binding_is_exact_and_does_not_default_missing_policy() {
        let scene = scene();
        let style = serde_json::json!({
            "residualLandProgram": {
                "version": 1,
                "scenes": {"s": {"urbanOpenSpaceRatio": 0.1, "ruralPastureRatio": 0.2}}
            }
        });
        let expected = for_scene(&style, &scene).unwrap().unwrap();
        assert_eq!(
            audit_source(Some(&expected), &style, &scene).unwrap(),
            Some(expected.clone())
        );
        assert!(audit_source(None, &style, &scene).is_err());
        assert!(audit_source(
            Some(&SceneResidualProgram {
                urban_open_space_ratio: Some(0.0),
                rural_pasture_ratio: Some(0.0),
            }),
            &style,
            &scene,
        )
        .is_err());
        // Deleting a stated field is a publication change even when the value
        // it carried was the equal of the default: presence is audited too.
        let mut deleted = expected.clone();
        deleted.rural_pasture_ratio = None;
        assert!(audit_source(Some(&deleted), &style, &scene).is_err());
        let mut deleted_urban = expected.clone();
        deleted_urban.urban_open_space_ratio = None;
        assert!(audit_source(Some(&deleted_urban), &style, &scene).is_err());
        let no_program = serde_json::json!({});
        assert!(audit_source(None, &no_program, &scene).unwrap().is_none());
        assert!(audit_source(Some(&expected), &no_program, &scene).is_err());
    }
}
