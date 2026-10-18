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
    /// The OPT-IN bounded-cut ledger: what this world excavated, pad by pad.
    ///
    /// Present only when `style.frontageProgram.terrainPolicy` was declared at
    /// all, so every input that never asked publishes exactly the bytes it
    /// published before this field existed. A declared-but-illegal pair is
    /// published too, with `enabled: false` and the named reason, so a style that
    /// asked for a cut and did not get one is answered in writing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_cut: Option<super::worldgen::bounded_cut::CutLedger>,
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
            bounded_cut: None,
        }
    }

    /// Publish the bounded-cut ledger. The format version states the extension
    /// only for a world that actually carries one, so the version every existing
    /// world publishes is untouched.
    pub fn with_bounded_cut(mut self, ledger: super::worldgen::bounded_cut::CutLedger) -> Self {
        if ledger.enabled {
            if self.version < 3 {
                self.version = 3;
            }
            self.status = if self.version == 3 && self.status.contains("profile") {
                "profile-and-footprint-fill-surfaces-with-bounded-cut".into()
            } else {
                "ordered-footprint-carves-with-bounded-cut".into()
            };
        }
        self.bounded_cut = Some(ledger);
        self
    }
}
