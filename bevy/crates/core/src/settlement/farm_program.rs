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
    fn a_zone_id_valid_at_one_scene_size_is_not_valid_at_another() {
        // G34-R5-B01 round 24 root cause. plan_landuse_zones builds ids as
        // {scene_id}.zone.{kind}.{row * cols + col}, where the grid size comes from the fixed 160 m cell
        // constant. The same zone index therefore denotes different zones at different scene sizes.
        //
        // The real input examples/b01-parcel-program.manifest.json hard-codes the zoneFields key
        // "scene-0.zone.rural.6". Measured against the produced worlds: at 512 m (16 zones of 128x128,
        // a 4x4 grid) that id EXISTS and IS rural, and is even the first rural zone. At 1 km (49 zones of
        // about 143x143, a 7x7 grid) index 6 is a NATURE zone, so the id "scene-0.zone.rural.6" is ABSENT
        // entirely and the first rural zone is scene-0.zone.rural.31. farm_program validates both that the
        // id exists and that its kind is rural, which is why the 1 km manifest derived from the 512 m
        // control failed with
        // "agricultureProgram zone scene-0.zone.rural.6 is not a rural zone in scene scene-0".
        //
        // This pins the mechanism so the constraint is visible to whoever next reuses an agricultureProgram
        // across scene sizes.
        let style = serde_json::json!({
            "landUseProfile": {
                "urbanCoreRatio": 0.02, "suburbanRatio": 0.02,
                "farmRatio": 0.7, "pastureRatio": 0.1, "forestRatio": 0.05,
                "reserveRatio": 0.05, "greenRatio": 0.05
            }
        });
        let profile = LandUseProfile::from_style(&style).unwrap();
        let scene_at = |size: u32| SceneSpec {
            scene_id: "scene-0".into(),
            width_m: size,
            depth_m: size,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let small = plan_landuse_zones(&scene_at(512), &profile);
        let large = plan_landuse_zones(&scene_at(1000), &profile);
        let kind_of = |zones: &[crate::worldgen::LandUseZone], id: &str| {
            zones.iter().find(|z| z.id == id).map(|z| z.kind.clone())
        };
        let id = "scene-0.zone.rural.6";
        let small_kind = kind_of(&small, id);
        let large_kind = kind_of(&large, id);
        // The 512 m scene must have this index as rural, otherwise the control would not bake.
        assert_eq!(
            small_kind.as_deref(),
            Some("rural"),
            "the 512 m control requires index 6 to be a rural zone; grid was {} cells",
            small.len()
        );
        // And at 1 km the same index must NOT be rural, which is the defect this test exists for.
        // Measured: index 6 there is a NATURE zone, so the id is absent from the rural set altogether.
        assert!(
            large_kind.as_deref() != Some("rural"),
            "if index 6 were rural at 1 km too, the derived manifest would not have failed; \
             grid was {} cells",
            large.len()
        );
        // Both scenes must still contain SOME rural zone, so the failure is the stale reference and not a
        // missing rural zone.
        assert!(small.iter().any(|z| z.kind == "rural"));
        assert!(large.iter().any(|z| z.kind == "rural"));
        // A size-agnostic program: defaultField only, no scene-specific zoneFields key. This must validate
        // at BOTH sizes, which is the fix the derived 1 km manifest needs.
        for size in [512_u32, 1000] {
            let scenes = vec![scene_at(size)];
            let style = serde_json::json!({
                "landUseProfile": {
                    "urbanCoreRatio": 0.02, "suburbanRatio": 0.02,
                    "farmRatio": 0.7, "pastureRatio": 0.1, "forestRatio": 0.05,
                    "reserveRatio": 0.05, "greenRatio": 0.05
                },
                "agricultureProgram": {
                    "version": 1,
                    "scenes": { "scene-0": { "defaultField": { "widthM": 32, "depthM": 24 } } }
                }
            });
            validate_manifest(&style, &scenes).unwrap_or_else(|e| {
                panic!("a defaultField-only agricultureProgram must validate at {size} m: {e}")
            });
        }
        // and the stale reference must be rejected at 1 km, which is the product behaving correctly
        let scenes = vec![scene_at(1000)];
        let mut style = serde_json::json!({
            "landUseProfile": {
                "urbanCoreRatio": 0.02, "suburbanRatio": 0.02,
                "farmRatio": 0.7, "pastureRatio": 0.1, "forestRatio": 0.05,
                "reserveRatio": 0.05, "greenRatio": 0.05
            },
            "agricultureProgram": {
                "version": 1,
                "scenes": {
                    "scene-0": {
                        "defaultField": { "widthM": 32, "depthM": 24 },
                        "zoneFields": { "scene-0.zone.rural.6": { "widthM": 48, "depthM": 32 } }
                    }
                }
            }
        });
        let rejected = validate_manifest(&style, &scenes).is_err();
        if let Some(kind) = large_kind.as_deref() {
            assert_eq!(
                rejected,
                kind != "rural",
                "the stale zone reference must be rejected exactly when index 6 is not rural"
            );
        } else {
            assert!(rejected, "a reference to a zone that does not exist must be rejected");
        }
        let _ = &mut style;
    }
    #[test]
    fn the_derived_1km_b01_manifest_validates_after_dropping_the_scene_specific_zone_field() {
        // G34-R5-B01 round 26. The derived 1 km B01 manifest used to carry a hard-coded
        // agricultureProgram zoneFields key that only exists at 512 m, so it failed before any world was
        // generated. It has been regenerated in the size-agnostic form. This loads the ACTUAL file and
        // validates it, so the repair is verified by executing the product's own obligation check rather
        // than by reasoning about it.
        let m: crate::worldgen::WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/b01-parcel-program-1km.manifest.json"
        ))
        .expect("the regenerated 1 km manifest must parse");
        assert_eq!(m.scenes[0].width_m, 1000, "the derived scene must be 1 km");
        let ap = m.style.get("agricultureProgram").expect("agricultureProgram must be present");
        let rendered = serde_json::to_string(ap).unwrap();
        assert!(
            !rendered.contains("zoneFields"),
            "the scene-size-specific zoneFields key must be gone; it is what made the manifest invalid at 1 km"
        );
        validate_manifest(&m.style, &m.scenes)
            .expect("the regenerated 1 km manifest must satisfy the agricultureProgram obligations");
        // and the pattern must resolve for the scene, so the program is usable and not merely tolerated
        let pattern = for_scene(&m.style, &m.scenes[0])
            .expect("the scene must resolve a farm pattern")
            .expect("the pattern must be present for scene-0");
        assert!(
            pattern.zone_fields.is_empty(),
            "no zone-specific fields should remain; got {:?}",
            pattern.zone_fields.keys().collect::<Vec<_>>()
        );
        // the 512 m control must STILL carry its zone field and still validate, so the repair did not
        // touch the protected input
        let control: crate::worldgen::WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/b01-parcel-program.manifest.json"
        ))
        .expect("the 512 m control must parse");
        assert!(
            serde_json::to_string(&control.style["agricultureProgram"])
                .unwrap()
                .contains("zoneFields"),
            "the 512 m control must keep its zone field; the task book forbids changing it"
        );
        validate_manifest(&control.style, &control.scenes)
            .expect("the 512 m control must still validate");
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
