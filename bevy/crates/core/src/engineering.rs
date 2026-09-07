//! Human terrain engineering operations preserved separately from natural terrain.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineeredTerrainSnapshot {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub source_heightfield: String,
    pub operations: Vec<EngineeredTerrainOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeredTerrainOperation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_profile: Option<crate::worldgen::road_profile::RoadProfile>,
    pub id: String,
    pub kind: String,
    pub entity_id: String,
    pub center_x: f32,
    pub center_z: f32,
    pub half_width_m: f32,
    pub half_depth_m: f32,
    pub target_height_m: f32,
    pub blend_m: f32,
    pub priority: i32,
    pub status: String,
}

impl EngineeredTerrainSnapshot {
    pub fn new(operations: Vec<EngineeredTerrainOperation>) -> Self {
        let graded = operations.iter().any(|op| op.road_profile.is_some());
        Self {
            format: "glyphweave.engineered-terrain".into(),
            version: if graded { 2 } else { 1 },
            status: if graded {
                "profile-and-footprint-fill-surfaces"
            } else {
                "ordered-footprint-carves-only"
            }
            .into(),
            source_heightfield: "natural-terrain-snapshot".into(),
            operations,
        }
    }
}
