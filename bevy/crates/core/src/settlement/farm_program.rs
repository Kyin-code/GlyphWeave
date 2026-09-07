//! Explicit field dimensions from manifest intent. No automatic fallback to
//! smaller fields, and no changes to grade/access/earthwork limits.
use crate::worldgen::{LandUseProfile, SceneSpec, plan_landuse_zones};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldTemplate {
    pub width_m: u32,
    pub depth_m: u32,
}
impl Default for FieldTemplate {
    fn default() -> Self {
        Self {
            width_m: 24,
            depth_m: 24,
        }
    }
}
impl FieldTemplate {
    pub fn validate(self) -> Result<(), String> {
        if ![self.width_m, self.depth_m]
            .iter()
            .all(|v| (16..=128).contains(v) && v % 4 == 0)
        {
            return Err("field dimensions must be multiples of 4 in 16..=128 metres".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmPattern {
    pub default_field: FieldTemplate,
    #[serde(default)]
    pub zone_fields: BTreeMap<String, FieldTemplate>,
}
impl FarmPattern {
    pub fn validate(&self) -> Result<(), String> {
        self.default_field.validate()?;
        if self.zone_fields.len() > 256 {
            return Err("too many agriculture zone overrides".into());
        }
        for (id, t) in &self.zone_fields {
            if id.is_empty() {
                return Err("empty agriculture zone id".into());
            }
            t.validate()?;
        }
        Ok(())
    }
    pub fn for_zone(&self, id: &str) -> FieldTemplate {
        self.zone_fields
            .get(id)
            .copied()
            .unwrap_or(self.default_field)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmProgram {
    pub version: u32,
    pub scenes: BTreeMap<String, FarmPattern>,
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<FarmProgram>, String> {
    let Some(value) = style.get("agricultureProgram") else {
        return Ok(None);
    };
    let p: FarmProgram = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid agricultureProgram: {e}"))?;
    if p.version != 1 || p.scenes.is_empty() || p.scenes.len() > 64 {
        return Err("agricultureProgram requires version1 and 1..=64 scenes".into());
    }
    for (id, pattern) in &p.scenes {
        if id.is_empty() {
            return Err("empty agriculture scene id".into());
        }
        pattern.validate()?;
    }
    Ok(Some(p))
}
pub fn for_scene(
    style: &serde_json::Value,
    scene: &SceneSpec,
) -> Result<Option<FarmPattern>, String> {
    Ok(from_style(style)?.and_then(|p| p.scenes.get(&scene.scene_id).cloned()))
}
pub fn validate_manifest(style: &serde_json::Value, scenes: &[SceneSpec]) -> Result<(), String> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    let profile = LandUseProfile::from_style(style)
        .ok_or("agricultureProgram requires explicit landUseProfile")?;
    if style.get("naturalOnly").and_then(|v| v.as_bool()) == Some(true) {
        return Err("agricultureProgram conflicts with naturalOnly".into());
    }
    for (id, p) in &program.scenes {
        let scene = scenes
            .iter()
            .find(|s| &s.scene_id == id)
            .ok_or("agricultureProgram refers to unknown scene")?;
        let zones = plan_landuse_zones(scene, &profile);
        if !zones.iter().any(|z| z.kind == "rural") {
            return Err("agricultureProgram scene has no rural zone".into());
        }
        for zone_id in p.zone_fields.keys() {
            if !zones.iter().any(|z| &z.id == zone_id && z.kind == "rural") {
                return Err(format!(
                    "agricultureProgram zone {zone_id} is not a rural zone in scene {id}"
                ));
            }
        }
    }
    Ok(())
}
pub fn audit_source(
    pattern: &Option<FarmPattern>,
    style: &serde_json::Value,
    scene: &SceneSpec,
) -> Result<(), String> {
    if pattern != &for_scene(style, scene)? {
        return Err("agriculture program differs from authoritative manifest".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensions_and_unknown_fields_fail_closed() {
        for n in [0, 12, 17, 130, u32::MAX] {
            assert!(
                FieldTemplate {
                    width_m: n,
                    depth_m: 24
                }
                .validate()
                .is_err()
            );
        }
        for n in [16, 24, 48, 128] {
            FieldTemplate {
                width_m: n,
                depth_m: 32,
            }
            .validate()
            .unwrap();
        }
        for v in [
            serde_json::json!(null),
            serde_json::json!({"version":2,"scenes":{}}),
            serde_json::json!({"version":1,"scenes":{"scene":{"defaultField":{"widthM":32,"depthM":48,"ignored":true}}}}),
        ] {
            assert!(from_style(&serde_json::json!({"agricultureProgram":v})).is_err());
        }
    }
    #[test]
    fn manifest_scene_zone_and_natural_only_obligations_are_checked() {
        let mut m: crate::worldgen::WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/service-program-joint-plains.manifest.json"
        ))
        .unwrap();
        let profile = LandUseProfile::from_style(&m.style).unwrap();
        let zones = plan_landuse_zones(&m.scenes[0], &profile);
        let rural = zones.iter().find(|z| z.kind == "rural").unwrap().id.clone();
        m.style["agricultureProgram"] = serde_json::json!({"version":1,"scenes":{"scene-0":{"defaultField":{"widthM":48,"depthM":32},"zoneFields":{rural.clone():{"widthM":24,"depthM":24}}}}});
        validate_manifest(&m.style, &m.scenes).unwrap();
        let pattern = for_scene(&m.style, &m.scenes[0]).unwrap();
        audit_source(&pattern, &m.style, &m.scenes[0]).unwrap();
        assert!(audit_source(&None, &m.style, &m.scenes[0]).is_err());
        let mut bad = m.style.clone();
        bad["agricultureProgram"]["scenes"]["unknown"] =
            bad["agricultureProgram"]["scenes"]["scene-0"].clone();
        assert!(validate_manifest(&bad, &m.scenes).is_err());
        let mut bad = m.style.clone();
        bad["agricultureProgram"]["scenes"]["scene-0"]["zoneFields"]["unknown"] =
            serde_json::json!({"widthM":24,"depthM":24});
        assert!(validate_manifest(&bad, &m.scenes).is_err());
        let mut bad = m.style.clone();
        bad["naturalOnly"] = serde_json::json!(true);
        assert!(validate_manifest(&bad, &m.scenes).is_err());
    }

    #[test]
    fn overrides_are_explicit_and_missing_program_keeps_legacy_size() {
        assert_eq!(
            FieldTemplate::default(),
            FieldTemplate {
                width_m: 24,
                depth_m: 24
            }
        );
        let p = FarmPattern {
            default_field: FieldTemplate {
                width_m: 48,
                depth_m: 32,
            },
            zone_fields: BTreeMap::from([("small".into(), FieldTemplate::default())]),
        };
        p.validate().unwrap();
        assert_eq!(p.for_zone("small"), FieldTemplate::default());
        assert_eq!(p.for_zone("other"), p.default_field);
    }
}
