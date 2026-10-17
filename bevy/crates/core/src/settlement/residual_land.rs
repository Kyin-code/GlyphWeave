//! Explicit B residual-land policy. Ratios select real, bounded sub-use
//! surfaces inside the original purpose; the remainder stays unresolved.
use crate::worldgen::SceneSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneResidualProgram {
    pub urban_open_space_ratio: f64,
    pub rural_pasture_ratio: f64,
}
impl SceneResidualProgram {
    pub fn validate(&self) -> Result<(), String> {
        if !self.urban_open_space_ratio.is_finite()
            || !self.rural_pasture_ratio.is_finite()
            || !(0. ..=0.25).contains(&self.urban_open_space_ratio)
            || !(0. ..=0.50).contains(&self.rural_pasture_ratio)
        {
            return Err(
                "residual ratios must be finite with urban<=0.25 and rural<=0.50".into(),
            );
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
    #[test]
    fn ratios_are_explicit_bounded_and_legacy_default_is_off() {
        assert!(from_style(&serde_json::json!({})).unwrap().is_none());
        assert!(from_style(&serde_json::json!({"residualLandProgram":{"version":1,"scenes":{"s":{"urbanOpenSpaceRatio":0.1,"ruralPastureRatio":0.2}}}})).unwrap().is_some());
        for value in [-0.1, 0.26, f64::NAN] {
            assert!(SceneResidualProgram {
                urban_open_space_ratio: value,
                rural_pasture_ratio: 0.,
            }
            .validate()
            .is_err());
        }
        assert!(SceneResidualProgram {
            urban_open_space_ratio: 0.25,
            rural_pasture_ratio: 0.5,
        }
        .validate()
        .is_ok());
    }
    #[test]
    fn source_binding_is_exact_and_does_not_default_missing_policy() {
        let scene = SceneSpec {
            scene_id: "s".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
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
                urban_open_space_ratio: 0.0,
                rural_pasture_ratio: 0.0,
            }),
            &style,
            &scene,
        )
        .is_err());
        let no_program = serde_json::json!({});
        assert!(audit_source(None, &no_program, &scene).unwrap().is_none());
        assert!(audit_source(Some(&expected), &no_program, &scene).is_err());
    }
}
