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
    /// C-E1 retaining-wall and culvert records. Present only when the scene
    /// opted into `style.terrainWorks.retainingWalls/drainageStructures`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retaining_works: Option<crate::engineering_works::TerrainWorksPlan>,
    /// C-E2: the natural-vs-engineered sampled routing comparison. Present only
    /// when a world opts into drainage feedback, so every existing snapshot keeps
    /// its exact bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drainage_impact: Option<crate::engineering_drainage::EngineeringDrainageImpact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeredTerrainOperation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_profile: Option<crate::worldgen::road_profile::RoadProfile>,
    /// Arbitrary-yaw longitudinal grade of a route-evolution segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_grade: Option<crate::worldgen::RouteGrade>,
    pub id: String,
    pub kind: String,
    pub entity_id: String,
    pub center_x: f32,
    pub center_z: f32,
    pub half_width_m: f32,
    pub half_depth_m: f32,
    /// Local footprint yaw. Older snapshots omit it and interpret the half
    /// extents as world-axis values, preserving their published geometry.
    #[serde(default, skip_serializing_if = "rotation_is_zero")]
    pub rotation_y_deg: f32,
    pub target_height_m: f32,
    pub blend_m: f32,
    pub priority: i32,
    pub status: String,
}

fn rotation_is_zero(rotation: &f32) -> bool {
    *rotation == 0.0
}

impl EngineeredTerrainSnapshot {
    pub fn new(operations: Vec<EngineeredTerrainOperation>) -> Self {
        let graded = operations.iter().any(|op| op.road_profile.is_some());
        let route_graded = operations.iter().any(|op| op.route_grade.is_some());
        Self {
            format: "glyphweave.engineered-terrain".into(),
            version: if route_graded {
                3
            } else if graded {
                2
            } else {
                1
            },
            status: if graded {
                "profile-and-footprint-fill-surfaces"
            } else {
                "ordered-footprint-carves-only"
            }
            .into(),
            source_heightfield: "natural-terrain-snapshot".into(),
            operations,
            bounded_cut: None,
            retaining_works: None,
            drainage_impact: None,
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

    /// Attach the C-E2 comparison and bump the format only for an opted-in
    /// world. The impact itself remains a separate, independently auditable
    /// document.
    pub fn with_drainage_impact(
        mut self,
        impact: crate::engineering_drainage::EngineeringDrainageImpact,
    ) -> Self {
        if self.version < 4 {
            self.version = 4;
        }
        self.status = format!("{}-with-drainage-feedback", self.status);
        self.drainage_impact = Some(impact);
        self
    }

    /// Attach the C-E1 retaining-wall / culvert plan. The snapshot version is
    /// bumped only for a world that actually opted in.
    pub fn with_terrain_works(
        mut self,
        works: crate::engineering_works::TerrainWorksPlan,
    ) -> Self {
        if self.version < 5 {
            self.version = 5;
        }
        self.status = format!("{}-with-terrain-works", self.status);
        self.retaining_works = Some(works);
        self
    }
}
