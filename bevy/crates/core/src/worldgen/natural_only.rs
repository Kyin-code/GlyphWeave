//! Fail-closed contract for the natural-only stage.
//!
//! A natural-only scene may contain natural terrain, water, vegetation, rocks
//! and other explicitly natural entities. It may not contain roads, buildings,
//! bridges, street furniture, agricultural works or terrain engineering. The
//! audit is deliberately a positive allow-list: unknown kinds fail instead of
//! silently entering the natural stage.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::EntityInstance;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub entity_id: String,
    pub kind: String,
    pub asset_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SceneAudit {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub status: String,
    pub human_trace_count: usize,
    pub terrain_operation_count: usize,
    pub road_plan_present: bool,
    pub development_plan_present: bool,
    pub traces: Vec<Trace>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldAudit {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub human_trace_count: usize,
    pub scenes: Vec<SceneAudit>,
    pub failures: Vec<String>,
}

pub fn enabled(style: &Value) -> bool {
    style
        .get("naturalOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// A natural stage must name its macro-geography; a seed alone is not a
/// sufficient description of regional relief, climate, coast or hydrology.
pub fn validate_style(style: &Value) -> Result<(), String> {
    if enabled(style) && style.get("naturalGeography").is_none() {
        return Err(
            "naturalOnly requires style.naturalGeography so the natural stage has an authored macro-geography"
                .into(),
        );
    }
    Ok(())
}

pub fn is_natural_kind(kind: &str) -> bool {
    matches!(
        kind,
        "tree"
            | "bush"
            | "rock"
            | "grass_clump"
            | "reed"
            | "fallen_log"
            | "water"
            | "lake"
            | "river"
            | "sea"
            | "pond"
            | "spring"
            | "waterfall"
            | "wetland"
            | "shore"
            | "cliff"
            | "cave"
            | "sand_dune"
            | "snowfield"
            | "glacier"
            | "natural_landmark"
    )
}

pub fn audit_scene(
    scene_id: &str,
    entities: &[EntityInstance],
    terrain_operation_count: usize,
    road_plan_present: bool,
    development_plan_present: bool,
) -> SceneAudit {
    let traces: Vec<Trace> = entities
        .iter()
        .filter(|entity| !is_natural_kind(&entity.kind))
        .map(|entity| Trace {
            entity_id: entity.entity_id.clone(),
            kind: entity.kind.clone(),
            asset_id: entity.asset_id.clone(),
            reason: "kind is outside the natural-only allow-list".into(),
        })
        .collect();
    let mut failures = Vec::new();
    if !traces.is_empty() {
        failures.push(format!(
            "{} human or unknown entities remain in the natural-only stage",
            traces.len()
        ));
    }
    if terrain_operation_count > 0 {
        failures.push(format!(
            "{} terrain engineering operations exist in the natural-only stage",
            terrain_operation_count
        ));
    }
    if road_plan_present {
        failures.push("a road plan exists in the natural-only stage".into());
    }
    if development_plan_present {
        failures.push("a development plan exists in the natural-only stage".into());
    }
    SceneAudit {
        format: "glyphweave.natural-only-audit.scene".into(),
        version: 1,
        scene_id: scene_id.into(),
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        human_trace_count: traces.len(),
        terrain_operation_count,
        road_plan_present,
        development_plan_present,
        traces,
        failures,
    }
}

pub fn aggregate(scenes: Vec<SceneAudit>) -> WorldAudit {
    let human_trace_count = scenes.iter().map(|scene| scene.human_trace_count).sum();
    let failures: Vec<String> = scenes
        .iter()
        .flat_map(|scene| {
            scene
                .failures
                .iter()
                .map(move |failure| format!("{}: {failure}", scene.scene_id))
        })
        .collect();
    WorldAudit {
        format: "glyphweave.natural-only-audit".into(),
        version: 1,
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        human_trace_count,
        scenes,
        failures,
    }
}

pub fn require_pass(audit: &SceneAudit) -> Result<(), String> {
    if audit.status == "pass" {
        Ok(())
    } else {
        Err(format!(
            "natural-only audit failed for {}: {}",
            audit.scene_id,
            audit.failures.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{EntityInstance, GroundingSpec};

    fn entity(kind: &str) -> EntityInstance {
        EntityInstance {
            entity_id: format!("test.{kind}"),
            asset_id: format!("test.{kind}"),
            kind: kind.into(),
            world_x: 0,
            world_z: 0,
            world_y: 0,
            scale: 1.0,
            width_m: 1.0,
            depth_m: 1.0,
            height_m: 1.0,
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        }
    }

    #[test]
    fn natural_allow_list_passes_and_aggregates_zero_human_traces() {
        let scene = audit_scene(
            "scene-0",
            &[
                entity("tree"),
                entity("bush"),
                entity("rock"),
                entity("lake"),
            ],
            0,
            false,
            false,
        );
        require_pass(&scene).unwrap();
        let world = aggregate(vec![scene]);
        assert_eq!(world.status, "pass");
        assert_eq!(world.human_trace_count, 0);
    }

    #[test]
    fn roads_buildings_unknown_kinds_and_engineering_fail_closed() {
        let scene = audit_scene(
            "scene-0",
            &[
                entity("tree"),
                entity("road"),
                entity("building"),
                entity("future-unclassified-object"),
            ],
            2,
            true,
            true,
        );
        assert_eq!(scene.status, "fail");
        assert_eq!(scene.human_trace_count, 3);
        assert_eq!(scene.terrain_operation_count, 2);
        assert!(require_pass(&scene).is_err());
    }

    #[test]
    fn natural_only_requires_authored_macro_geography() {
        assert!(validate_style(&serde_json::json!({"naturalOnly": true})).is_err());
        assert!(
            validate_style(&serde_json::json!({
                "naturalOnly": true,
                "naturalGeography": {
                    "version": 1,
                    "setting": "coastal-mountain",
                    "regionalReliefM": 900,
                    "orogenyAxisDeg": 28,
                    "upliftBias": 0.72,
                    "coast": {"type": "ria-bay", "waveExposure": "moderate", "trench": false},
                    "climate": {
                        "band": "humid-subtropical",
                        "prevailingWindDeg": 125,
                        "monsoon": true,
                        "rainfallBias": 0.65,
                        "seasonality": 0.48
                    },
                    "hydrology": {
                        "regime": "perennial",
                        "historicChannelShiftM": 260,
                        "wetlandBias": 0.3
                    },
                    "landform": {
                        "karst": false,
                        "aeolian": false,
                        "glacial": false,
                        "incisedValleys": 0.68
                    },
                    "hazards": ["flood"]
                }
            }))
            .is_ok()
        );
    }
}
