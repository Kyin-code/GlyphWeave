//! Deterministic large-world planning and sidecar baking for engine adapters.
//!
//! Public world coordinates use named `world_x`, `world_z`, and `world_y`
//! fields. The `.gemap` v3 codec keeps its frozen `(z, x, y)` protocol order.

use crate::baked_terrain::{BakedTerrain, TerrainError};
use crate::engineering::{EngineeredTerrainOperation, EngineeredTerrainSnapshot};
use crate::height_payload::{HeightEncoding, HeightPayload, encode_millimetres};
use crate::natural::{NaturalTerrainSnapshot, audit_drainage_graph};
use crate::settlement::development::{self, DevelopmentPlan, LandcoverPlan};
use crate::settlement::landcover::{self, SurfaceRealization};
use crate::settlement::proposals::{self, DevelopmentProposal};
use crate::settlement::{
    ParcelPlan, SettlementGraph, derive_settlement_graph, plan_parcels,
    realize_infrastructure_plan, realize_parcel_plan, validate_parcel_plan,
};
mod access;
pub mod bounded_cut;
/// OPT-IN `style.terrainPolicy.entryCorridor = "graded"`: the graded, fill-only
/// walking corridor a pad needs when the street it is assigned to stands above it
/// and only the street's own fill slope lies between the door and the pavement
/// (see the module docs for the contract and the ledger).
pub mod entry_corridor;
/// The graded road's bounded-cut support contract: the plan-stage probe that
/// refuses a road BY NAME when its own cut still cannot bring the roadbed inside
/// the 6 m fill contract (see the module docs).
pub mod road_support;
/// OPT-IN `style.terrainPolicy.earthworkBlend`: derive each structure's
/// fill-transition ring from its own footprint instead of the fixed envelope
/// (see the module docs for the constants and their derivation).
pub mod earthwork_blend;
/// The entity-stage completion of a planned pad's declared platform: the loop
/// between "the plan accepted a pad" and "the rule-engine earthwork stands on top
/// of it" (see the module docs).
pub mod pad_completion;
#[path = "worldgen/entity_identity.rs"]
mod entity_identity;
pub use entity_identity::{ENTITY_ID_SCHEME, EntityCandidateKey, EntityChangeKind, EntityChangeApproval, EntityInventoryMode, EntityInventoryChange, EntityInventoryAudit, audit_entity_inventory};
use entity_identity::CandidateIds;
mod access_surface;
pub use access::{AccessGraph, AccessLink, AccessPoint, build_access_graph};
pub use access_surface::{AccessSurface, bake_access_surfaces, validate_access_surfaces};
mod engineered_placement;
pub use engineered_placement::{
    PlacementReportAudit, verify_published_placement,
};
pub mod joint_frontage;
pub mod road_network;
pub mod road_alignment;
pub mod road_profile;
pub mod road_extensions;
pub mod auto_roads;
pub mod engineering_connections;
pub mod frontage;
pub mod conversion_budget;
pub mod conversion_fringe;
mod frontage_supply;
pub mod parcel_geometry;
pub mod road_parcel_feasibility;
pub mod services;
pub mod natural_geography;
pub mod natural_only;
pub mod route_evolution;
pub mod urban_slice;
mod shorefront;
mod terrain_context;
mod water_query;
pub mod shore_band;
pub mod urban_growth;
pub mod zoning;
#[cfg(test)]
mod assigned_road_tests;
#[cfg(test)]
mod entrance_recheck_tests;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use terrain_context::{RasterSurface, TerrainContext};
pub use water_query::{WATER_SHORE_CLEARANCE_M, footprint_intersects_water};

use crate::storage::codec::encode_world_with_metadata;
use crate::voxel::{VoxelCoord, VoxelWorld};
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub const WORLD_FORMAT: &str = "glyphweave-world";
pub const WORLD_VERSION: u32 = 1;
/// Bump when the serialized world semantics or deterministic bake algorithm changes.
pub const WORLD_GENERATOR_REVISION: &str = "worldgen-2026-09-08-r51-stable-v1-transport-identity";
pub const STREAM_CHUNK_METERS: u32 = 512;
pub const MIN_SCENE_METERS: u32 = 512;
pub const MAX_SCENE_WIDTH_METERS: u32 = 6_000;
pub const MAX_SCENE_DEPTH_METERS: u32 = 10_000;
const SHORE_RADIUS: f64 = 1.9;
/// Packed terrain-semantic sidecar payload: slope, curvature, wetness and
/// human disturbance at the same one-metre raster as the baked heightfield.
const TERRAIN_SEMANTIC_BYTES_PER_CELL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    None,
    Lake,
    River,
}

#[derive(Debug, Clone, Copy)]
pub struct WaterGeometry {
    kind: WaterKind,
    center_x: f64,
    center_z: f64,
    half_width: f64,
    half_depth: f64,
    /// Scene footprint used by landform classification. Kept on the geometry
    /// struct so every terrain / surface call can derive the same world-space
    /// landform bands without threading extra scene state through the API.
    scene_width_m: u32,
    scene_depth_m: u32,
    terrain_origin_x: i32,
    terrain_origin_z: i32,
    /// Smooth-rolling terrain (steppe / prairie): soft rolling hills instead
    /// of sharp ridges, matching wgen's "Hills" hemispheric generator.
    smooth_rolling: bool,
    /// Opt-in road-first over structured water
    /// (`automaticRoadExtensions.allowStructuredWater = "bridge-or-causeway"`).
    ///
    /// When set, no earthwork may be placed inside the water body: the raster
    /// keeps the natural bed, banks and surface exactly, because every road that
    /// reaches the water is carried by a registered structure deck instead of by
    /// fill. Off for every input that did not opt in, so the raster path is
    /// byte-identical for them.
    road_water_crossings: bool,
    /// Authored macro-geography expanded by `natural_geography`. Defaults are
    /// byte-compatible with the legacy noise terrain.
    geography: natural_geography::GeographyControls,
}

/// A terrain carve: a flat rectangular pad under a road or building footprint.
/// Roads and buildings are generated ON TOP of a flattened pad so a building
/// on a slope reads as standing on level ground instead of draping over the
/// hillside (the approach used by symbios-ground-lab's terrain carving).
/// `blend_m` is the width of the smooth transition ring around the pad.
#[derive(Debug, Clone, PartialEq)]
struct TerrainCarve {
    profile: Option<std::sync::Arc<road_profile::RoadProfile>>,
    cx: f64,
    cz: f64,
    half_w: f64,
    half_d: f64,
    target_h_m: f64,
    blend_m: f64,
    /// Roads are carved before buildings so a pad under a street is not
    /// re-flattened by a neighbouring lot; higher = wins.
    priority: i32,
    /// Render/analysis mask: 0 natural, 160 building/parcel pad, 255 roadbed.
    disturbance: u8,
    /// OPT-IN bounded cut: the deepest excavation this carve may take from the
    /// natural surface, in whole millimetres. `None` - every carve of every input
    /// that did not opt in - is the historical fill-only contract, and
    /// `apply_terrain_carves` never lowers the ground for it.
    cut_cap_mm: Option<i32>,
    /// OPT-IN `style.terrainPolicy.entryCorridor`: this carve is a graded walking
    /// ramp (see [`entry_corridor`]). Its target height varies along one axis, and
    /// inside its own pavement it OWNS the published surface - it replaces the
    /// pad's own fill rather than competing with it - because the whole point of
    /// the ramp is to remove a fill bulge the pad's ring placed. It never lowers
    /// the natural ground (the target is always `>= natural`), so it is not an
    /// excavation, and a road's flat top still wins over it.
    ///
    /// `None` for every other carve of every other input, so the historical
    /// composition is bit-for-bit unchanged when the policy is off.
    ramp: Option<std::sync::Arc<entry_corridor::EntryRamp>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroundingPivot {
    #[default]
    Bottom,
    Center,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroundingSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage_road_point: Option<[i32;2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_profile: Option<road_profile::RoadProfile>,
    /// Authored road-tier provenance for an automatic/extension road. Base
    /// roads and every non-road entity leave it absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_tier: Option<road_network::RoadTierRef>,
    /// Optional settlement-stage provenance carried from an expanded district
    /// frontage request. Absent for every non-district entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_cohort: Option<String>,
    #[serde(default)]
    pub pivot: GroundingPivot,
    #[serde(default)]
    pub bottom_offset_m: f32,
    #[serde(default)]
    pub roadbed_offset_m: f32,
    #[serde(default = "default_grounding_tolerance_m")]
    pub tolerance_m: f32,
    /// A STRUCTURE deck: this roadbed is carried by an engineering structure
    /// (a registered bridge over water), not by an earthwork fill, so
    /// `plan_terrain_carves` refuses to carve it and the ground under it stays
    /// exactly as the natural raster holds it.
    ///
    /// Only ever true for a road that `road_network::WaterCrossing` registers,
    /// and it is not serialized when false, so every existing entity keeps its
    /// published bytes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub structure_deck: bool,
    /// OPT-IN bounded cut: the deepest excavation, in whole millimetres, this
    /// entity's own support pad may take from the natural surface.
    ///
    /// Only `frontage::append` ever sets it, and only when
    /// `style.frontageProgram.terrainPolicy = "bounded-cut"` is declared with a
    /// legal `maxCutMm` AND the accepted pad actually needs a cut. Absent (and
    /// therefore not serialized) for every other entity, so an input that never
    /// asked keeps its published bytes exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_cut_mm: Option<i32>,
    /// OPT-IN, `style.terrainPolicy.earthworkBlend`: the width of this structure's
    /// own fill-transition ring, in metres - `"derived-from-footprint"` derives it
    /// from the structure's footprint ([`earthwork_blend::derived_blend_m`]),
    /// `"derived-from-fill"` from the fill depth it actually places
    /// ([`earthwork_blend::derived_fill_blend_m`]).
    ///
    /// Only [`earthwork_blend::stamp`] ever sets it on a rule-engine structure or a
    /// road, once, for every entity of a world that opted in; a PLANNED pad's ring
    /// arrives through its own plan record instead
    /// ([`earthwork_blend::stamp_plan`] -> `RoadsideSite::earthwork_blend_m`), so
    /// the rebuild of that entity from the plan carries the same number. It is the
    /// ring `plan_terrain_carves` publishes, so the carve, the raster, the
    /// completion pass and every audit read one number.
    /// Absent (and therefore not serialized) for every other entity, so a world
    /// that never asked - and every road of a world that asked only for the
    /// footprint value, which is about structures - keeps its historical bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earthwork_blend_m: Option<f32>,
    /// OPT-IN `style.terrainPolicy.entryCorridor = "graded"`: this pad's own
    /// graded walking corridor toward the street it is assigned to, when the
    /// checked entrance needed one.
    ///
    /// Only [`entry_corridor::gate_or_corridor`] ever sets it, and only after the
    /// SAME acceptance gate failed on the surface the bake would have published
    /// and then passed on the surface the corridor makes it publish. Absent (and
    /// therefore not serialized) for every other entity of every other input, so
    /// a world that never asked keeps its published bytes exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_corridor: Option<entry_corridor::EntryCorridor>,
}

fn default_grounding_tolerance_m() -> f32 {
    1.0
}

impl Default for GroundingSpec {
    fn default() -> Self {
        Self {
            frontage_road_point: None,
            road_profile: None,
            road_tier: None,
            district_id: None,
            construction_cohort: None,
            pivot: GroundingPivot::Bottom,
            bottom_offset_m: 0.0,
            roadbed_offset_m: 0.0,
            tolerance_m: default_grounding_tolerance_m(),
            structure_deck: false,
            bounded_cut_mm: None,
            earthwork_blend_m: None,
            entry_corridor: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bounds2d {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialAnchor {
    pub id: String,
    pub world_x: i32,
    pub world_z: i32,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneGraph {
    #[serde(default)]
    pub transitions: Vec<SceneTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneTransition {
    pub id: String,
    pub source_scene: String,
    pub target_scene: String,
    pub source_world_x: i32,
    pub source_world_z: i32,
    pub target_world_x: i32,
    pub target_world_z: i32,
    pub direction: String,
    #[serde(default)]
    pub kind: String,
}

/// Declares the derived terrain raster carried beside the authoritative
/// heightfield. Height and land-cover remain in their existing payloads; this
/// contract makes the shared render/placement inputs explicit instead of
/// asking every adapter to invent independent noise fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainSemanticsContract {
    pub format: String,
    pub version: u32,
    pub bytes_per_cell: u32,
    /// Byte order per raster cell: slope degrees, signed curvature, wetness,
    /// and disturbance. Values are normalized u8 unless noted otherwise.
    pub channels: Vec<String>,
}

impl Default for TerrainSemanticsContract {
    fn default() -> Self {
        Self {
            format: "glyphweave-terrain-semantics".into(),
            version: 1,
            bytes_per_cell: TERRAIN_SEMANTIC_BYTES_PER_CELL as u32,
            channels: vec![
                "slope_degrees_u8".into(),
                "curvature_signed_u8".into(),
                "wetness_u8".into(),
                "disturbance_u8".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarContract {
    pub format: String,
    pub version: u32,
    pub authoritative_for_terrain: bool,
    pub gemap_role: String,
    pub scene_index_root: String,
    pub height_precision_m: f32,
    #[serde(default = "default_height_encoding_name")]
    pub height_encoding: String,
    #[serde(default = "default_lod_height_precision")]
    pub lod_height_precision_m: f32,
    #[serde(default)]
    pub terrain_semantics: TerrainSemanticsContract,
}

fn default_height_encoding_name() -> String {
    "i16le-quarter-meter-raw".into()
}
fn default_lod_height_precision() -> f32 {
    0.25
}
impl SidecarContract {
    pub fn for_style(style: &serde_json::Value) -> WorldgenResult<Self> {
        let mut c = Self::default();
        if height_encoding_from_style(style)? == HeightEncoding::Millimetre {
            c.height_precision_m = 0.001;
            c.height_encoding = "gwhm-v1-i32le-millimeter".into();
        }
        Ok(c)
    }
}
impl Default for SidecarContract {
    fn default() -> Self {
        Self {
            format: "glyphweave-sidecar".into(),
            version: 1,
            authoritative_for_terrain: true,
            gemap_role: "identity-anchor".into(),
            scene_index_root: "scenes/".into(),
            height_precision_m: 0.25,
            height_encoding: default_height_encoding_name(),
            lod_height_precision_m: 0.25,
            terrain_semantics: TerrainSemanticsContract::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldManifest {
    pub format: String,
    pub version: u32,
    pub world: WorldSpec,
    #[serde(default)]
    pub scenes: Vec<SceneSpec>,
    #[serde(default)]
    pub style: serde_json::Value,
    #[serde(default)]
    pub landmarks: Vec<LandmarkSpec>,
    #[serde(default)]
    pub scene_graph: SceneGraph,
    /// Hash of the immutable patch chain applied to this manifest.
    #[serde(default)]
    pub patch_chain_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldSpec {
    pub name: String,
    pub seed: u64,
    #[serde(default = "default_render_mode")]
    pub render_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneSpec {
    pub scene_id: String,
    pub width_m: u32,
    pub depth_m: u32,
    #[serde(default)]
    pub origin_x: i32,
    #[serde(default)]
    pub origin_z: i32,
    #[serde(default)]
    pub seed_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandmarkSpec {
    pub entity_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub purpose: String,
    pub description: String,
    pub scene_id: String,
    pub world_x: i32,
    pub world_z: i32,
    #[serde(default)]
    pub world_y: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
    pub asset_id: String,
    #[serde(default)]
    pub rotation_y_deg: f32,
    #[serde(default)]
    pub grounding: GroundingSpec,
    #[serde(default)]
    pub anchors: Vec<SpatialAnchor>,
    #[serde(default)]
    pub bounds: Option<Bounds2d>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EdgeSummary {
    pub min_quarter_m: i16,
    pub max_quarter_m: i16,
    pub dominant_surface: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChunkBoundarySummary {
    pub west: EdgeSummary,
    pub east: EdgeSummary,
    pub north: EdgeSummary,
    pub south: EdgeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDescriptor {
    pub chunk_x: u32,
    pub chunk_z: u32,
    pub world_x: i32,
    pub world_z: i32,
    pub valid_width_m: u32,
    pub valid_depth_m: u32,
    pub height_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural_height_file: Option<String>,
    pub surface_file: String,
    pub lod2_file: String,
    /// Optional for worlds baked before terrain semantics; new bakes include it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_file: Option<String>,
    pub hash: String,
    /// Edge statistics allow adjacent chunks and multi-scene boundaries to be
    /// checked without decoding the full raster first.
    #[serde(default)]
    pub boundary: ChunkBoundarySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneIndex {
    #[serde(default)]
    pub land_use_budget: Option<LandUseBudget>,
    #[serde(default)]
    pub land_use_zones: Vec<LandUseZone>,
    #[serde(default)]
    pub settlement_graph: Option<SettlementGraph>,
    #[serde(default)]
    pub parcel_plan: Option<ParcelPlan>,
    #[serde(default)]
    pub road_network_plan: Option<road_network::RoadNetworkPlan>,
    #[serde(default)]
    pub development_plan: Option<DevelopmentPlan>,
    #[serde(default)]
    pub landcover_plan: Option<LandcoverPlan>,
    #[serde(default)]
    pub landcover_surfaces: Option<SurfaceRealization>,
    #[serde(default)]
    pub development_proposals: Vec<DevelopmentProposal>,
    #[serde(default)]
    pub natural_terrain: NaturalTerrainSnapshot,
    #[serde(default)]
    pub engineered_terrain: EngineeredTerrainSnapshot,
    pub scene_id: String,
    pub width_m: u32,
    pub depth_m: u32,
    pub origin_x: i32,
    pub origin_z: i32,
    pub chunk_size_m: u32,
    pub chunk_count_x: u32,
    pub chunk_count_z: u32,
    pub chunks: Vec<ChunkDescriptor>,
    pub landmarks: Vec<LandmarkSpec>,
    pub entities: Vec<EntityInstance>,
    /// Provenance for the buildings the RULE ENGINE placed for itself: the
    /// generator's own candidate keys, recomputable into the very entity ids in
    /// `entities`. Source authority, not a report - the development audit needs
    /// it to tell a rule structure from a stray one, and nothing else knows.
    ///
    /// Published only for a scene that also publishes a development plan, so a
    /// world without a road-first ledger keeps its historical bytes, and only
    /// when there is something to say.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_building_candidates: Vec<EntityCandidateKey>,
    #[serde(default)]
    pub access_graph: AccessGraph,
    #[serde(default)]
    pub access_surfaces: Vec<AccessSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityInstance {
    pub entity_id: String,
    pub asset_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub world_y: i32,
    pub scale: f32,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    #[serde(default)]
    pub rotation_y_deg: f32,
    #[serde(default)]
    pub grounding: GroundingSpec,
    #[serde(default)]
    pub anchors: Vec<SpatialAnchor>,
    #[serde(default)]
    pub bounds: Option<Bounds2d>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldIndex {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub seed: u64,
    pub render_mode: String,
    pub revision: String,
    #[serde(default)]
    pub generator_revision: String,
    #[serde(default)]
    pub patch_chain_hash: String,
    pub scenes: Vec<String>,
    #[serde(default)]
    pub scene_graph: SceneGraph,
    #[serde(default)]
    pub sidecar: SidecarContract,
    /// Horizontal datum used by preview adapters for water cells.
    #[serde(default)]
    pub water_level_m: Option<f32>,
    /// Road geometry revision the baked earthwork used. Absent on legacy
    /// products that cannot use the 16m graded band.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_geometry_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPatch {
    pub format: String,
    pub version: u32,
    pub patch_id: String,
    /// Optional for legacy v1 patches; strict workflows should provide it.
    #[serde(default)]
    pub base_revision: String,
    #[serde(default)]
    pub parent_patch_hash: Option<String>,
    #[serde(default)]
    pub sequence: u64,
    pub operations: Vec<PatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum PatchOperation {
    MoveLandmark {
        entity_id: String,
        world_x: i32,
        world_z: i32,
        world_y: i32,
    },
    SetLandmark {
        landmark: LandmarkSpec,
    },
    RemoveLandmark {
        entity_id: String,
    },
}

#[derive(Debug, Error)]
pub enum WorldgenError {
    #[error("world manifest format must be {WORLD_FORMAT}, got {0:?}")]
    InvalidFormat(String),
    #[error("unsupported world manifest version {0}")]
    UnsupportedVersion(u32),
    #[error("world name must not be empty")]
    EmptyWorldName,
    #[error("renderMode must be 2d or 2.5d, got {0:?}")]
    InvalidRenderMode(String),
    #[error("scene {0:?} has an invalid or duplicate ID")]
    InvalidSceneId(String),
    #[error("invalid scene transition: {0}")]
    InvalidTransition(String),
    #[error(
        "scene {scene_id:?} dimensions {width_m}x{depth_m}m are outside {MIN_SCENE_METERS}m..{MAX_SCENE_WIDTH_METERS}m by {MIN_SCENE_METERS}m..{MAX_SCENE_DEPTH_METERS}m"
    )]
    InvalidSceneSize {
        scene_id: String,
        width_m: u32,
        depth_m: u32,
    },
    #[error("landmark {0:?} is missing required narrative or asset fields")]
    InvalidLandmark(String),
    #[error("landmark {landmark:?} references missing scene {scene:?}")]
    MissingLandmarkScene { landmark: String, scene: String },
    #[error(
        "landmark {landmark:?} footprint intersects water in scene {scene:?}; move it to land or declare an explicit water-compatible kind"
    )]
    LandmarkIntersectsWater { landmark: String, scene: String },
    #[error("landmark {landmark:?} is outside scene {scene:?}")]
    LandmarkOutsideScene { landmark: String, scene: String },
    #[error("output directory already contains files: {0}")]
    OutputNotEmpty(String),
    #[error("rules mode could not load descriptors: {0}")]
    InvalidRules(String),
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error(
        "rules-mode baked audit failed for scene {scene_id}: {rejected} of {checked} checked entities rejected"
    )]
    RulesAuditFailed {
        scene_id: String,
        checked: usize,
        rejected: usize,
    },
    #[error(transparent)]
    Terrain(#[from] TerrainError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type WorldgenResult<T> = Result<T, WorldgenError>;

fn default_render_mode() -> String {
    "2.5d".to_owned()
}

/// Urban morphology family abstracted from the 36-city baseline in
/// `docs/modern-mainland-morphology.zh-CN.md`. A profile only tunes the
/// distribution priors; it never changes the hard constraints (determinism,
/// footprint vs water, chunk continuity, asset contracts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CityForm {
    DenseCore,
    RiverDelta,
    CoastalBay,
    MountainValley,
    TemperatePlain,
    LowDensitySuburban,
}

impl CityForm {
    pub fn parse(value: &str) -> CityForm {
        match value {
            "dense-core" | "dense_core" => CityForm::DenseCore,
            "river-delta" | "river_delta" => CityForm::RiverDelta,
            "coastal-bay" | "coastal_bay" => CityForm::CoastalBay,
            "mountain-valley" | "mountain_valley" => CityForm::MountainValley,
            "low-density-suburban" | "low_density_suburban" => CityForm::LowDensitySuburban,
            _ => CityForm::TemperatePlain,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CityForm::DenseCore => "dense-core",
            CityForm::RiverDelta => "river-delta",
            CityForm::CoastalBay => "coastal-bay",
            CityForm::MountainValley => "mountain-valley",
            CityForm::TemperatePlain => "temperate-plain",
            CityForm::LowDensitySuburban => "low-density-suburban",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoadGrid {
    Grid,
    Radial,
    RiverAxis,
    ValleyAxis,
    Loose,
}

#[derive(Debug, Clone, Copy)]
struct CityFormParams {
    form: CityForm,
    core_count: usize,
    road_grid: RoadGrid,
    block_scale: f32,
    core_density: f64,
    suburban_density: f64,
    road_spacing_m: i32,
    canal_probability: f64,
}

impl CityFormParams {
    fn resolve(style: &serde_json::Value) -> CityFormParams {
        let form = style
            .get("landUseProfile")
            .and_then(|profile| profile.get("theme"))
            .and_then(serde_json::Value::as_str)
            .map(CityForm::parse)
            .unwrap_or(CityForm::TemperatePlain);
        let params = match form {
            CityForm::DenseCore => CityFormParams {
                form,
                core_count: 1,
                road_grid: RoadGrid::Grid,
                block_scale: 1.0,
                core_density: 0.95,
                suburban_density: 0.62,
                road_spacing_m: 260,
                canal_probability: 0.0,
            },
            CityForm::RiverDelta => CityFormParams {
                form,
                core_count: 3,
                road_grid: RoadGrid::RiverAxis,
                block_scale: 1.1,
                core_density: 0.78,
                suburban_density: 0.5,
                road_spacing_m: 320,
                canal_probability: 0.55,
            },
            CityForm::CoastalBay => CityFormParams {
                form,
                core_count: 2,
                road_grid: RoadGrid::Radial,
                block_scale: 0.9,
                core_density: 0.88,
                suburban_density: 0.55,
                road_spacing_m: 300,
                canal_probability: 0.15,
            },
            CityForm::MountainValley => CityFormParams {
                form,
                core_count: 2,
                road_grid: RoadGrid::ValleyAxis,
                block_scale: 0.8,
                core_density: 0.66,
                suburban_density: 0.4,
                road_spacing_m: 380,
                canal_probability: 0.25,
            },
            CityForm::LowDensitySuburban => CityFormParams {
                form,
                core_count: 1,
                road_grid: RoadGrid::Loose,
                block_scale: 1.5,
                core_density: 0.42,
                suburban_density: 0.28,
                road_spacing_m: 460,
                canal_probability: 0.05,
            },
            CityForm::TemperatePlain => CityFormParams {
                form,
                core_count: 1,
                road_grid: RoadGrid::Grid,
                block_scale: 1.0,
                core_density: 0.75,
                suburban_density: 0.5,
                road_spacing_m: 360,
                canal_probability: 0.2,
            },
        };
        params
    }

    fn is_multi_core(&self) -> bool {
        self.core_count > 1
    }
}

/// Structured land-use ratios parsed from `style.landUseProfile`. The Rust
/// generator treats these as area targets, not as hard quotas; the audit layer
/// reports achieved area against the target interval.
#[derive(Debug, Clone, Copy)]
pub struct LandUseProfile {
    pub theme: CityForm,
    pub urban_core_ratio: f64,
    pub suburban_ratio: f64,
    pub green_ratio: f64,
    pub farm_ratio: f64,
    pub forest_ratio: f64,
    pub pasture_ratio: f64,
    pub reserve_ratio: f64,
}

impl LandUseProfile {
    pub fn from_style(style: &serde_json::Value) -> Option<LandUseProfile> {
        let profile = style.get("landUseProfile")?;
        Some(LandUseProfile {
            theme: profile
                .get("theme")
                .and_then(serde_json::Value::as_str)
                .map(CityForm::parse)
                .unwrap_or(CityForm::TemperatePlain),
            urban_core_ratio: ratio_of(profile, "urbanCoreRatio", 0.22),
            suburban_ratio: ratio_of(profile, "suburbanRatio", 0.28),
            green_ratio: ratio_of(profile, "greenRatio", 0.18),
            farm_ratio: ratio_of(profile, "farmRatio", 0.20),
            forest_ratio: ratio_of(profile, "forestRatio", 0.20),
            pasture_ratio: ratio_of(profile, "pastureRatio", 0.12),
            reserve_ratio: ratio_of(profile, "reserveRatio", 0.10),
        })
    }

    pub fn default_demo() -> LandUseProfile {
        LandUseProfile {
            theme: CityForm::TemperatePlain,
            urban_core_ratio: 0.22,
            suburban_ratio: 0.28,
            green_ratio: 0.18,
            farm_ratio: 0.20,
            forest_ratio: 0.20,
            pasture_ratio: 0.12,
            reserve_ratio: 0.10,
        }
    }

    pub fn urban_target(&self) -> f64 {
        (self.urban_core_ratio + self.suburban_ratio).clamp(0.0, 1.0)
    }

    pub fn rural_target(&self) -> f64 {
        (self.farm_ratio + self.pasture_ratio).clamp(0.0, 1.0)
    }

    pub fn nature_target(&self) -> f64 {
        (self.green_ratio + self.forest_ratio + self.reserve_ratio).clamp(0.0, 1.0)
    }
}

fn ratio_of(profile: &serde_json::Value, key: &str, fallback: f64) -> f64 {
    profile
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(fallback)
        .clamp(0.0, 1.0)
}

/// Planning allocation, NOT achieved area. Realization is accounted by the
/// disjoint purpose/physical-cover ledger after geometry reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandUseZone {
    pub id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub source: String,
}

pub fn plan_landuse_zones(scene: &SceneSpec, profile: &LandUseProfile) -> Vec<LandUseZone> {
    if scene.width_m == 0 || scene.depth_m == 0 {
        return Vec::new();
    }
    let total =
        (profile.urban_target() + profile.rural_target() + profile.nature_target()).max(1.0);
    let targets = [
        profile.urban_target() / total,
        profile.rural_target() / total,
        profile.nature_target() / total,
    ];
    let cx = f64::from(scene.origin_x) + f64::from(scene.width_m) * 0.5;
    let cz = f64::from(scene.origin_z) + f64::from(scene.depth_m) * 0.5;
    let mut cells: Vec<(f64, ZoningCell)> = zoning_cells(scene)
        .into_iter()
        .map(|cell| {
            let radius = (f64::from(cell.world_x) + f64::from(cell.width_m) * 0.5 - cx).powi(2)
                + (f64::from(cell.world_z) + f64::from(cell.depth_m) * 0.5 - cz).powi(2);
            (radius, cell)
        })
        .collect();
    cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.index.cmp(&b.1.index)));
    let area = f64::from(scene.width_m) * f64::from(scene.depth_m);
    let boundaries = [
        targets[0] * area,
        (targets[0] + targets[1]) * area,
        (targets[0] + targets[1] + targets[2]) * area,
    ];
    let names = ["urban", "rural", "nature", "unassigned"];
    let mut consumed = 0.0;
    let mut category = 0;
    cells
        .into_iter()
        .map(|(_, cell)| {
            let size = f64::from(cell.width_m) * f64::from(cell.depth_m);
            // Nearest cumulative area boundary, not rounded cell count. Zero
            // targets remain zero and an unspecified remainder stays unassigned.
            while category < 3
                && (consumed + size - boundaries[category]).abs()
                    >= (consumed - boundaries[category]).abs()
            {
                category += 1;
            }
            let kind = names[category];
            consumed += size;
            LandUseZone {
                id: format!("{}.zone.{kind}.{}", scene.scene_id, cell.index),
                kind: kind.into(),
                world_x: cell.world_x,
                world_z: cell.world_z,
                width_m: cell.width_m,
                depth_m: cell.depth_m,
                source: RADIAL_ZONE_SOURCE.into(),
            }
        })
        .collect()
}

/// The published `source` of the default, radial land-use plan.
pub const RADIAL_ZONE_SOURCE: &str = "balanced-area-radial-v3";

/// One cell of the land-use planning grid: the unit every zoning rectangle is an exact cut of.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ZoningCell {
    /// Row-major index of the cell in the plan, which is the suffix of its zone ids.
    pub index: usize,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
}

/// The planning grid one scene is zoned on: the same cells the radial plan and the opt-in
/// axis-and-node growth both cut, so the two plans are directly comparable square metre for square
/// metre.
pub(crate) fn zoning_cells(scene: &SceneSpec) -> Vec<ZoningCell> {
    if scene.width_m == 0 || scene.depth_m == 0 {
        return Vec::new();
    }
    // The zoning cell is 240 m for scenes whose longest side is at least 5 km and 160 m otherwise.
    //
    // A round 12 experiment replaced this with `(longest / 4).clamp(160, 240)` on the theory that a
    // fixed 160 m cell was too small at 1 km, where it produces 143 m cells. The theory was measured and
    // REFUTED on the same input: at 1 km the rescale collapsed the urban zoning to a single 200 m cell
    // and cut the number of station-carrying roads that touch urban zoning from 6 to 2, so realization
    // went from 0 of 120 homes and 1 of 24 shops to 0 of 120 and 0 of 24. The experiment was reverted
    // rather than kept, because a change that helps nothing measured should not sit in the tree.
    //
    // The real B01 limit, measured separately, is that frontage stations exist only on profile-bearing
    // roads and only 6 of those 16 roads touch an urban zone at all.
    let cell = if scene.width_m.max(scene.depth_m) >= 5000 {
        240
    } else {
        160
    };
    let cols = scene.width_m.div_ceil(cell);
    let rows = scene.depth_m.div_ceil(cell);
    let mut cells = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            // Redistribute the partial edge rather than emitting a 32m sliver.
            let x = (u64::from(col) * u64::from(scene.width_m) / u64::from(cols)) as u32;
            let z = (u64::from(row) * u64::from(scene.depth_m) / u64::from(rows)) as u32;
            let right = (u64::from(col + 1) * u64::from(scene.width_m) / u64::from(cols)) as u32;
            let bottom = (u64::from(row + 1) * u64::from(scene.depth_m) / u64::from(rows)) as u32;
            let wx = i64::from(scene.origin_x) + i64::from(x);
            let wz = i64::from(scene.origin_z) + i64::from(z);
            let (Ok(wx), Ok(wz)) = (i32::try_from(wx), i32::try_from(wz)) else {
                continue;
            };
            cells.push(ZoningCell {
                index: (row * cols + col) as usize,
                world_x: wx,
                world_z: wz,
                width_m: right - x,
                depth_m: bottom - z,
            });
        }
    }
    cells
}

/// The land-use plan one scene's style states.
///
/// With the opt-in switch off this is [`plan_landuse_zones`] and nothing else, so no existing world
/// can move by a byte. With `style.landUseProfile.urbanGrowth = "axis-and-node"` the plan grows along
/// the road network the world builds under its OWN (radial) land-use plan - the network it already
/// had - and the urban, rural and nature totals stay the radial plan's, exactly.
///
/// `style` must be the EFFECTIVE style the bake plans with (the compiled one: the morphology
/// compiler fills in the frontage program's derived setbacks), never the raw manifest style.
pub fn plan_landuse_zones_for_style(
    scene: &SceneSpec,
    profile: &LandUseProfile,
    style: &serde_json::Value,
    natural: &BakedTerrain,
    water: WaterGeometry,
    program: Option<&services::ServiceProgram>,
    source: NaturalSource,
) -> Result<urban_growth::GrowthPlan, TerrainError> {
    if !urban_growth::enabled(style) {
        return Ok(urban_growth::GrowthPlan {
            zones: plan_landuse_zones(scene, profile),
            selection: None,
            source: RADIAL_ZONE_SOURCE.into(),
        });
    }
    // The network the world already builds: the one the RADIAL plan produces, planned exactly as the
    // bake plans it (same zones, same style, same natural source). It is a pure function of inputs
    // the audit also holds, so the grown plan is reproducible without trusting the bake.
    let network = if road_network::enabled(style, water) {
        let radial = plan_landuse_zones(scene, profile);
        let skeleton = road_network::plan_with_style_declared(
            scene,
            natural,
            water,
            Some(&radial),
            program,
            style,
            source,
        )?;
        Some(urban_growth::Network::from_plan(&skeleton))
    } else {
        None
    };
    Ok(urban_growth::plan(scene, profile, network.as_ref(), water))
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandUseBudget {
    pub scene_area_m2: f64,
    #[serde(default)]
    pub realization_accounting: String,
    pub target_urban_ratio: f64,
    pub target_rural_ratio: f64,
    pub target_nature_ratio: f64,
    pub target_urban_m2: f64,
    pub target_rural_m2: f64,
    pub target_nature_m2: f64,
    pub planned_urban_m2: f64,
    pub planned_rural_m2: f64,
    pub planned_nature_m2: f64,
    pub entity_urban_m2: f64,
    pub entity_rural_m2: f64,
    pub entity_nature_m2: f64,
    pub actual_urban_m2: f64,
    pub actual_rural_m2: f64,
    pub actual_nature_m2: f64,
    #[serde(default)]
    pub actual_transport_m2: f64,
    pub assigned_m2: f64,
    pub unassigned_m2: f64,
    pub status: String,
    pub entity_coverage_status: String,
}

pub fn plan_landuse_budget(scene: &SceneIndex, profile: &LandUseProfile) -> LandUseBudget {
    let areas = analyze_landuse_areas(scene);
    let zone_area = |kind: &str| -> f64 {
        scene
            .land_use_zones
            .iter()
            .filter(|zone| zone.kind == kind)
            .map(|zone| f64::from(zone.width_m) * f64::from(zone.depth_m))
            .sum()
    };
    let planned_urban = zone_area("urban");
    let planned_rural = zone_area("rural");
    let planned_nature = zone_area("nature");
    let entity_urban = areas.urban_m2;
    let entity_rural = areas.rural_m2;
    let entity_nature = areas.nature_m2;
    let (actual_urban, actual_rural, actual_nature, actual_transport, accounting) =
        if let (Some(plan), Some(roads)) = (&scene.development_plan, &scene.road_network_plan) {
            let owned = development::realized_footprint_areas(plan, roads, &scene.entities);
            (
                owned.urban_m2 as f64
                    + scene
                        .landcover_surfaces
                        .as_ref()
                        .map_or(0., |s| landcover::area_by_zone(s, "urban")),
                owned.rural_m2 as f64
                    + scene
                        .landcover_surfaces
                        .as_ref()
                        .map_or(0., |s| landcover::area_by_zone(s, "rural")),
                scene
                    .landcover_surfaces
                    .as_ref()
                    .map_or(0., |s| landcover::area_by_zone(s, "nature")),
                owned.road_m2 as f64
                    + scene
                        .landcover_surfaces
                        .as_ref()
                        .map_or(0., landcover::farm_track_area),
                "disjoint-owned-surfaces-v3",
            )
        } else {
            (
                entity_urban,
                entity_rural,
                entity_nature,
                0.,
                "legacy-entity-proxy",
            )
        };
    let norm = (profile.urban_target() + profile.rural_target() + profile.nature_target()).max(1.0);
    let urban = profile.urban_target() / norm;
    let rural = profile.rural_target() / norm;
    let nature = profile.nature_target() / norm;
    let target_urban_m2 = areas.scene_area_m2 * urban;
    let target_rural_m2 = areas.scene_area_m2 * rural;
    let target_nature_m2 = areas.scene_area_m2 * nature;
    let planned_within = [
        (planned_urban / areas.scene_area_m2.max(1.0) - urban).abs(),
        (planned_rural / areas.scene_area_m2.max(1.0) - rural).abs(),
        (planned_nature / areas.scene_area_m2.max(1.0) - nature).abs(),
    ]
    .into_iter()
    .all(|delta| delta <= 0.05);
    let entity_coverage = actual_urban + actual_rural + actual_nature + actual_transport;
    let entity_coverage_status =
        if entity_coverage >= (planned_urban + planned_rural + planned_nature) * 0.9 {
            "realized"
        } else {
            "underfilled"
        };
    LandUseBudget {
        scene_area_m2: areas.scene_area_m2,
        realization_accounting: accounting.into(),
        target_urban_ratio: urban,
        target_rural_ratio: rural,
        target_nature_ratio: nature,
        target_urban_m2,
        target_rural_m2,
        target_nature_m2,
        planned_urban_m2: planned_urban,
        planned_rural_m2: planned_rural,
        planned_nature_m2: planned_nature,
        entity_urban_m2: entity_urban,
        entity_rural_m2: entity_rural,
        entity_nature_m2: entity_nature,
        actual_urban_m2: actual_urban,
        actual_rural_m2: actual_rural,
        actual_nature_m2: actual_nature,
        actual_transport_m2: actual_transport,
        assigned_m2: entity_coverage,
        unassigned_m2: (areas.scene_area_m2 - entity_coverage).max(0.0),
        status: if planned_within {
            "planned_within_tolerance"
        } else {
            "planned_mismatched"
        }
        .into(),
        entity_coverage_status: entity_coverage_status.into(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandUseAreaReport {
    pub scene_area_m2: f64,
    pub urban_m2: f64,
    pub rural_m2: f64,
    pub nature_m2: f64,
    pub urban_ratio: f64,
    pub rural_ratio: f64,
    pub nature_ratio: f64,
    pub by_kind: BTreeMap<String, f64>,
}

/// Legacy diagnostic: overlapping entity footprint proxies, not zoning or
/// realized land cover. Kept for compatibility, never road-first gate authority.
pub fn analyze_landuse_areas(scene: &SceneIndex) -> LandUseAreaReport {
    let scene_area_m2 = f64::from(scene.width_m.max(1)) * f64::from(scene.depth_m.max(1));
    let mut by_kind = BTreeMap::<String, f64>::new();
    let mut urban_m2 = 0.0;
    let mut rural_m2 = 0.0;
    let mut nature_m2 = 0.0;
    for entity in &scene.entities {
        let area = f64::from(entity.width_m.max(0.0)) * f64::from(entity.depth_m.max(0.0));
        *by_kind.entry(entity.kind.clone()).or_insert(0.0) += area;
        match entity.kind.as_str() {
            "commercial_center"
            | "entertainment_center"
            | "school"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "parking_lot"
            | "temple"
            | "church"
            | "road"
            | "building"
            | "building_tower"
            | "storefront"
            | "town_hall"
            | "market"
            | "industrial"
            | "water_well"
            | "street_lamp"
            | "road_sign"
            | "shopping_mall"
            | "office_tower"
            | "convenience_store"
            | "night_market"
            | "community_plaza" => urban_m2 += area,
            "farmland" | "pasture" | "canal" | "building_cluster" | "hill" | "mountain"
            | "rock_mountain" => rural_m2 += area,
            "green_space" | "mountain_forest" | "nature_reserve" | "tree" | "bush" | "rock"
            | "grass_clump" | "reed" | "fallen_log" | "scenic_belt" | "mudflat" | "mangrove" => {
                nature_m2 += area
            }
            _ => {}
        }
    }
    let norm = scene_area_m2.max(1.0);
    LandUseAreaReport {
        scene_area_m2,
        urban_m2,
        rural_m2,
        nature_m2,
        urban_ratio: urban_m2 / norm,
        rural_ratio: rural_m2 / norm,
        nature_ratio: nature_m2 / norm,
        by_kind,
    }
}

pub(crate) fn default_asset_contracts() -> serde_json::Value {
    let mut contracts = serde_json::Map::new();
    let entries = [
        ("road", "road", 8.0, 10000.0, 6.0, 32.0, 0.1, 1.0),
        ("building", "building", 8.0, 80.0, 8.0, 80.0, 4.0, 30.0),
        ("tree", "tree", 2.0, 5.5, 2.0, 5.5, 4.0, 9.0),
        ("bush", "bush", 0.5, 3.0, 0.5, 3.0, 0.3, 3.0),
        ("rock", "rock", 0.2, 4.0, 0.2, 4.0, 0.1, 4.0),
        ("parking_lot", "parking", 20.0, 240.0, 12.0, 160.0, 0.1, 2.0),
        (
            "commercial_center",
            "commercial",
            40.0,
            300.0,
            30.0,
            240.0,
            4.0,
            80.0,
        ),
        (
            "entertainment_center",
            "entertainment",
            40.0,
            300.0,
            30.0,
            240.0,
            4.0,
            100.0,
        ),
        ("school", "school", 30.0, 140.0, 25.0, 120.0, 4.0, 30.0),
        (
            "residential_block",
            "residential",
            20.0,
            120.0,
            20.0,
            120.0,
            4.0,
            60.0,
        ),
        (
            "residential_tower",
            "residential",
            20.0,
            120.0,
            20.0,
            120.0,
            18.0,
            60.0,
        ),
        (
            "residential_home",
            "residential",
            24.0,
            140.0,
            24.0,
            130.0,
            4.0,
            14.0,
        ),
        ("green_space", "green", 20.0, 240.0, 20.0, 240.0, 0.1, 4.0),
        ("canal", "waterway", 6.0, 40.0, 50.0, 10000.0, 0.1, 6.0),
        ("farmland", "farmland", 40.0, 500.0, 40.0, 500.0, 0.1, 3.0),
        (
            "mountain_forest",
            "forest",
            40.0,
            800.0,
            40.0,
            800.0,
            1.0,
            200.0,
        ),
        ("temple", "religious", 12.0, 80.0, 12.0, 80.0, 3.0, 30.0),
        ("church", "religious", 12.0, 80.0, 12.0, 80.0, 3.0, 40.0),
        ("pasture", "pasture", 60.0, 800.0, 60.0, 800.0, 0.1, 4.0),
        (
            "nature_reserve",
            "reserve",
            80.0,
            2000.0,
            80.0,
            2000.0,
            0.1,
            10.0,
        ),
        ("town_hall", "civic", 30.0, 160.0, 24.0, 120.0, 8.0, 60.0),
        ("market", "market", 36.0, 200.0, 30.0, 160.0, 3.0, 20.0),
        (
            "industrial",
            "industrial",
            40.0,
            260.0,
            40.0,
            200.0,
            6.0,
            40.0,
        ),
        ("water_well", "amenity", 2.0, 12.0, 2.0, 12.0, 1.0, 4.0),
        (
            "street_lamp",
            "street-furniture",
            0.3,
            2.0,
            0.3,
            2.0,
            4.0,
            8.0,
        ),
        (
            "road_sign",
            "street-furniture",
            0.4,
            3.0,
            0.2,
            1.5,
            2.0,
            4.0,
        ),
        ("sidewalk", "pavement", 2.0, 10000.0, 2.0, 10.0, 0.1, 1.0),
        ("bench", "street-furniture", 0.2, 6.0, 0.2, 6.0, 0.1, 4.0),
        ("lamp", "street-furniture", 0.2, 3.0, 0.2, 3.0, 0.5, 10.0),
        ("reed", "vegetation", 0.1, 2.0, 0.1, 2.0, 0.1, 4.0),
        ("grass_clump", "vegetation", 0.1, 3.0, 0.1, 3.0, 0.1, 4.0),
        ("fallen_log", "vegetation", 0.2, 8.0, 0.2, 4.0, 0.1, 3.0),
        ("bridge", "bridge", 6.0, 10000.0, 6.0, 200.0, 0.1, 80.0),
        (
            "mountain", "landform", 20.0, 2000.0, 20.0, 2000.0, 1.0, 500.0,
        ),
        ("hill", "landform", 20.0, 1200.0, 20.0, 1200.0, 0.5, 180.0),
        (
            "rock_mountain",
            "landform",
            20.0,
            2000.0,
            20.0,
            2000.0,
            1.0,
            500.0,
        ),
        (
            "shopping_mall",
            "commercial",
            40.0,
            400.0,
            30.0,
            300.0,
            4.0,
            60.0,
        ),
        (
            "office_tower",
            "commercial",
            20.0,
            160.0,
            20.0,
            160.0,
            12.0,
            400.0,
        ),
        (
            "convenience_store",
            "commercial",
            6.0,
            40.0,
            6.0,
            30.0,
            2.5,
            12.0,
        ),
        ("fence", "barrier", 1.0, 10000.0, 0.1, 4.0, 0.5, 4.0),
        ("guardrail", "barrier", 1.0, 10000.0, 0.1, 4.0, 0.5, 3.0),
        ("boardwalk", "pavement", 1.0, 10000.0, 1.0, 20.0, 0.1, 3.0),
        (
            "scenic_belt",
            "landscape",
            10.0,
            2000.0,
            10.0,
            500.0,
            0.1,
            30.0,
        ),
        (
            "mudflat",
            "coastal_ecology",
            10.0,
            2000.0,
            10.0,
            1000.0,
            0.1,
            8.0,
        ),
        (
            "night_market",
            "market",
            20.0,
            300.0,
            12.0,
            220.0,
            0.1,
            20.0,
        ),
        (
            "community_plaza",
            "civic",
            20.0,
            400.0,
            20.0,
            400.0,
            0.1,
            8.0,
        ),
        (
            "mangrove",
            "coastal_ecology",
            10.0,
            1200.0,
            10.0,
            1200.0,
            0.1,
            20.0,
        ),
        (
            "building_cluster",
            "building",
            20.0,
            500.0,
            20.0,
            500.0,
            1.0,
            100.0,
        ),
        (
            "urban_building",
            "building",
            8.0,
            300.0,
            8.0,
            300.0,
            2.0,
            100.0,
        ),
        (
            "resort_lodge",
            "building",
            8.0,
            120.0,
            8.0,
            120.0,
            2.0,
            40.0,
        ),
        ("storefront", "commercial", 6.0, 120.0, 6.0, 80.0, 2.0, 30.0),
        ("road_exit", "road", 2.0, 10000.0, 2.0, 32.0, 0.1, 2.0),
    ];
    for (kind, contract_type, min_width, max_width, min_depth, max_depth, min_height, max_height) in
        entries
    {
        contracts.insert(
            kind.to_owned(),
            serde_json::json!({
                "type": contract_type,
                "placement": "surface-grounded",
                "allowedSurfaces": ["grass", "forest", "soil", "stone", "shore"],
                "forbiddenSurfaces": ["water", "deep_water", "underground"],
                "minWidthM": min_width,
                "maxWidthM": max_width,
                "minDepthM": min_depth,
                "maxDepthM": max_depth,
                "minHeightM": min_height,
                "maxHeightM": max_height,
            }),
        );
    }
    serde_json::Value::Object(contracts)
}

pub fn water_kind(style: &serde_json::Value) -> WaterKind {
    let Some(water) = style.get("water").and_then(serde_json::Value::as_object) else {
        return WaterKind::None;
    };
    if water
        .get("levelPolicy")
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return WaterKind::None;
    }
    match water.get("waterType").and_then(serde_json::Value::as_str) {
        Some("lake") | Some("pond") => WaterKind::Lake,
        Some("river") | Some("stream") => WaterKind::River,
        _ => WaterKind::None,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainDomain {
    pub origin_x: i32,
    pub origin_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
}
fn terrain_domain(style: &serde_json::Value) -> WorldgenResult<Option<TerrainDomain>> {
    let Some(value) = style.get("terrainDomain") else {
        return Ok(None);
    };
    let d: TerrainDomain = serde_json::from_value(value.clone())?;
    if d.width_m < 1
        || d.depth_m < 1
        || d.width_m > 6000
        || d.depth_m > 10000
        || d.origin_x.checked_add(d.width_m as i32).is_none()
        || d.origin_z.checked_add(d.depth_m as i32).is_none()
    {
        return Err(WorldgenError::InvalidRules(
            "invalid terrainDomain bounds".into(),
        ));
    }
    Ok(Some(d))
}
pub fn water_geometry(
    kind: WaterKind,
    landmarks: &[LandmarkSpec],
    scene: &SceneSpec,
    style: &serde_json::Value,
) -> WaterGeometry {
    let landmark = landmarks.iter().find(|item| match kind {
        WaterKind::Lake => item.entity_type == "lake",
        WaterKind::River => item.entity_type == "river",
        WaterKind::None => false,
    });
    let fallback_x = f64::from(scene.origin_x) + f64::from(scene.width_m) * 0.5;
    let fallback_z = f64::from(scene.origin_z) + f64::from(scene.depth_m) * 0.5;
    let (fallback_half_width, fallback_half_depth) = match kind {
        WaterKind::Lake => (
            f64::from(scene.width_m) * 0.38,
            f64::from(scene.depth_m) * 0.26,
        ),
        WaterKind::River => (
            f64::from(scene.width_m) * 0.14,
            f64::from(scene.depth_m) * 0.5,
        ),
        WaterKind::None => (0.0, 0.0),
    };
    let (center_x, center_z, half_width, half_depth) = landmark.map_or(
        (
            fallback_x,
            fallback_z,
            fallback_half_width,
            fallback_half_depth,
        ),
        |item| {
            (
                f64::from(item.world_x),
                f64::from(item.world_z),
                f64::from(item.width_m) * 0.5,
                f64::from(item.depth_m) * 0.5,
            )
        },
    );
    // "steppe" / "prairie" / "grassland" terrainProfile -> soft rolling hills
    // (wgen Hills style) instead of sharp mountain ridges.
    let smooth_rolling = style
        .get("terrainProfile")
        .and_then(serde_json::Value::as_str)
        .map(|s| matches!(s, "steppe" | "prairie" | "grassland" | "plains"))
        .unwrap_or(false);
    // Manifest validation checks optional shared domain before generation.
    let domain = terrain_domain(style)
        .ok()
        .flatten()
        .unwrap_or(TerrainDomain {
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            width_m: scene.width_m,
            depth_m: scene.depth_m,
        });
    let mut geography = natural_geography::controls_for_style(style).unwrap_or_default();
    if let Ok(Some(slice)) = urban_slice::compile(style) {
        geography.land_pressure = slice.land_pressure_index;
        geography.engineering_scale = slice.engineering_scale;
    }
    WaterGeometry {
        kind,
        center_x,
        center_z,
        half_width,
        half_depth,
        scene_width_m: domain.width_m,
        scene_depth_m: domain.depth_m,
        terrain_origin_x: domain.origin_x,
        terrain_origin_z: domain.origin_z,
        smooth_rolling,
        // The raster-side half of the same authorization `road_network::enabled`
        // reads from the style. Both come from the one string, so generation and
        // audit cannot disagree about whether earthwork in the water is legal.
        road_water_crossings: road_network::allows_structured_water(style),
        geography,
    }
}

fn legacy_water_geometry(
    kind: WaterKind,
    scene_width: u32,
    scene_depth: u32,
    river_half_m: f64,
) -> WaterGeometry {
    match kind {
        WaterKind::Lake => WaterGeometry {
            kind,
            center_x: f64::from(scene_width) * 0.52,
            center_z: f64::from(scene_depth) * 0.47,
            half_width: f64::from(scene_width) * 0.38,
            half_depth: f64::from(scene_depth) * 0.26,
            scene_width_m: scene_width,
            scene_depth_m: scene_depth,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        },
        WaterKind::River => WaterGeometry {
            kind,
            center_x: f64::from(scene_width) * 0.5,
            center_z: f64::from(scene_depth) * 0.5,
            half_width: river_half_m,
            half_depth: f64::from(scene_depth) * 0.5,
            scene_width_m: scene_width,
            scene_depth_m: scene_depth,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        },
        WaterKind::None => WaterGeometry {
            kind,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: scene_width,
            scene_depth_m: scene_depth,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        },
    }
}

impl GroundingSpec {
    pub fn validate(&self, owner: &str) -> WorldgenResult<()> {
        if let Some(p)=&self.road_profile {p.validate_shape()?;}
        if !self.bottom_offset_m.is_finite()
            || !self.roadbed_offset_m.is_finite()
            || !self.tolerance_m.is_finite()
            || self.tolerance_m < 0.0
        {
            return Err(WorldgenError::InvalidLandmark(owner.to_owned()));
        }
        Ok(())
    }
}

impl Bounds2d {
    pub fn from_center(x: f32, z: f32, width: f32, depth: f32) -> Self {
        let half_w = width.max(0.0) * 0.5;
        let half_d = depth.max(0.0) * 0.5;
        Self {
            min_x: x - half_w,
            min_z: z - half_d,
            max_x: x + half_w,
            max_z: z + half_d,
        }
    }

    pub fn is_finite_and_ordered(&self) -> bool {
        self.min_x.is_finite()
            && self.min_z.is_finite()
            && self.max_x.is_finite()
            && self.max_z.is_finite()
            && self.min_x <= self.max_x
            && self.min_z <= self.max_z
    }
}

impl EntityInstance {
    pub fn computed_bounds(&self) -> Bounds2d {
        let (half_w, half_d) =
            spatial_half_extents(self.width_m, self.depth_m, self.rotation_y_deg);
        Bounds2d {
            min_x: self.world_x as f32 - half_w as f32,
            min_z: self.world_z as f32 - half_d as f32,
            max_x: self.world_x as f32 + half_w as f32,
            max_z: self.world_z as f32 + half_d as f32,
        }
    }

    pub fn normalize_spatial_semantics(&mut self) {
        // The procedural road convention stores a long strip in width/depth,
        // while historical generators used the entity id to distinguish the
        // north/south variant. Normalize that legacy convention once into the
        // explicit rotation field consumed by adapters and future assets.
        if self.rotation_y_deg == 0.0
            && (self.entity_id.contains("road-ns")
                || self.entity_id.contains("north-south")
                || self.entity_id.contains("sidewalk-ns"))
        {
            self.rotation_y_deg = 90.0;
        }
        if self.bounds.is_none() {
            self.bounds = Some(self.computed_bounds());
        }
        // Buildings without an authored entrance still expose a deterministic
        // front anchor. This is deliberately conservative: authored anchors
        // always win, while generated anchors make road-access checks and
        // renderer placement explicit instead of relying on model defaults.
        let is_building = matches!(
            self.kind.as_str(),
            "building"
                | "building_tower"
                | "building_cluster"
                | "urban_building"
                | "residential_block"
                | "residential_tower"
                | "residential_home"
                | "resort_lodge"
                | "storefront"
                | "commercial_center"
                | "entertainment_center"
                | "school"
                | "town_hall"
                | "market"
                | "industrial"
                | "temple"
                | "church"
        );
        if is_building && self.anchors.is_empty() {
            // Local front is -Z. Transform continuously with the same yaw
            // convention used by terrain footprints and renderer meshes.
            let (sin, cos) = self.rotation_y_deg.to_radians().sin_cos();
            let offset = self.depth_m * 0.5 + 1.0;
            let dx = -sin * offset;
            let dz = -cos * offset;
            self.anchors.push(SpatialAnchor {
                id: "front".into(),
                world_x: (self.world_x as f32 + dx).round() as i32,
                world_z: (self.world_z as f32 + dz).round() as i32,
                direction: if dx.abs() > dz.abs() {
                    if dx > 0. { "east" } else { "west" }
                } else if dz > 0. {
                    "south"
                } else {
                    "north"
                }
                .into(),
                target: Some("road".into()),
            });
        }
    }

    pub fn ground_height_m(&self) -> f32 {
        let pivot_offset = match self.grounding.pivot {
            GroundingPivot::Bottom => 0.0,
            GroundingPivot::Center => -self.height_m * 0.5,
        };
        self.world_y as f32
            + pivot_offset
            + self.grounding.bottom_offset_m
            + self.grounding.roadbed_offset_m
    }
}

impl LandmarkSpec {
    pub fn computed_bounds(&self) -> Bounds2d {
        let (half_w, half_d) = spatial_half_extents(
            self.width_m as f32,
            self.depth_m as f32,
            self.rotation_y_deg,
        );
        Bounds2d {
            min_x: self.world_x as f32 - half_w as f32,
            min_z: self.world_z as f32 - half_d as f32,
            max_x: self.world_x as f32 + half_w as f32,
            max_z: self.world_z as f32 + half_d as f32,
        }
    }

    pub fn normalize_spatial_semantics(&mut self) {
        if self.bounds.is_none() {
            self.bounds = Some(self.computed_bounds());
        }
    }
}

impl SceneGraph {
    fn validate(&self, scenes: &[SceneSpec]) -> WorldgenResult<()> {
        let mut ids = BTreeMap::new();
        for transition in &self.transitions {
            if transition.id.trim().is_empty() || ids.insert(&transition.id, true).is_some() {
                return Err(WorldgenError::InvalidTransition(format!(
                    "invalid or duplicate id {:?}",
                    transition.id
                )));
            }
            let source = scenes
                .iter()
                .find(|scene| scene.scene_id == transition.source_scene)
                .ok_or_else(|| {
                    WorldgenError::InvalidTransition(format!(
                        "missing source scene {:?}",
                        transition.source_scene
                    ))
                })?;
            let target = scenes
                .iter()
                .find(|scene| scene.scene_id == transition.target_scene)
                .ok_or_else(|| {
                    WorldgenError::InvalidTransition(format!(
                        "missing target scene {:?}",
                        transition.target_scene
                    ))
                })?;
            if transition.direction.trim().is_empty() {
                return Err(WorldgenError::InvalidTransition(format!(
                    "transition {:?} has empty direction",
                    transition.id
                )));
            }
            let in_scene = |scene: &SceneSpec, x: i32, z: i32| {
                x >= scene.origin_x
                    && x <= scene.origin_x + scene.width_m as i32
                    && z >= scene.origin_z
                    && z <= scene.origin_z + scene.depth_m as i32
            };
            if !in_scene(source, transition.source_world_x, transition.source_world_z) {
                return Err(WorldgenError::InvalidTransition(format!(
                    "{} source coordinate is outside {}",
                    transition.id, source.scene_id
                )));
            }
            if !in_scene(target, transition.target_world_x, transition.target_world_z) {
                return Err(WorldgenError::InvalidTransition(format!(
                    "{} target coordinate is outside {}",
                    transition.id, target.scene_id
                )));
            }
        }
        Ok(())
    }
}

impl WorldManifest {
    pub fn default_demo() -> Self {
        Self {
            format: WORLD_FORMAT.to_owned(),
            version: WORLD_VERSION,
            world: WorldSpec {
                name: "GlyphWeave World".to_owned(),
                seed: 42,
                render_mode: default_render_mode(),
            },
            scenes: vec![SceneSpec {
                scene_id: "scene-0".to_owned(),
                width_m: 1_000,
                depth_m: 1_000,
                origin_x: 0,
                origin_z: 0,
                seed_offset: 0,
            }],
            style: serde_json::json!({
                "family":"procedural-natural-settlement",
                "terrain":"continuous-heightfield",
                "assetContracts": default_asset_contracts(),
                "landUseProfile": {
                    "theme": "temperate-plain",
                    "urbanCoreRatio": 0.10,
                    "suburbanRatio": 0.10,
                    "greenRatio": 0.06,
                    "farmRatio": 0.32,
                    "forestRatio": 0.14,
                    "pastureRatio": 0.26,
                    "reserveRatio": 0.04
                }
            }),
            landmarks: Vec::new(),
            scene_graph: SceneGraph::default(),
            patch_chain_hash: String::new(),
        }
    }

    pub fn validate(&self) -> WorldgenResult<()> {
        // A morphology profile compiles once into the effective programs that
        // both generation and independent audit consume. The authored style is
        // never rewritten, so the original demand survives publication.
        let compiled = crate::settlement::morphology_compile::compile(&self.style, &self.scenes)
            .map_err(WorldgenError::InvalidRules)?;
        let effective_manifest = compiled.as_ref().map(|compiled| {
            let mut manifest = self.clone();
            manifest.style = compiled.style.clone();
            manifest
        });
        let effective = effective_manifest.as_ref().unwrap_or(self);
        road_extensions::validate_manifest(effective)?;
        auto_roads::validate_manifest(effective)?;
        frontage::validate_manifest(effective)?;
        height_encoding_from_style(&self.style)?;
        natural_geography::validate_style(&self.style).map_err(WorldgenError::InvalidRules)?;
        natural_only::validate_style(&self.style).map_err(WorldgenError::InvalidRules)?;
        route_evolution::validate_style(&self.style).map_err(WorldgenError::InvalidRules)?;
        urban_slice::validate_style(&self.style).map_err(WorldgenError::InvalidRules)?;
        crate::settlement::farm_program::validate_manifest(&effective.style,&effective.scenes).map_err(WorldgenError::InvalidRules)?;
        crate::settlement::residual_land::validate_manifest(&effective.style,&effective.scenes)
            .map_err(WorldgenError::InvalidRules)?;
        crate::settlement::morphology::validate_manifest(&self.style)
            .map_err(WorldgenError::InvalidRules)?;
        let service_program =
            services::from_style(&effective.style).map_err(WorldgenError::InvalidRules)?;
        if let Some(program) = &service_program {
            for id in program.housing_minimum.keys() {
                if !self.scenes.iter().any(|s| &s.scene_id == id) {
                    return Err(WorldgenError::InvalidRules(
                        "housing minimum refers to unknown scene".into(),
                    ));
                }
            }
            for request in &program.requests {
                if !self.scenes.iter().any(|s| s.scene_id == request.scene_id) {
                    return Err(WorldgenError::InvalidRules(
                        "service request refers to unknown scene".into(),
                    ));
                }
            }
        }
        if let Some(domain) = terrain_domain(&self.style)? {
            for scene in &self.scenes {
                if i64::from(scene.origin_x) < i64::from(domain.origin_x)
                    || i64::from(scene.origin_z) < i64::from(domain.origin_z)
                    || i64::from(scene.origin_x) + i64::from(scene.width_m)
                        > i64::from(domain.origin_x) + i64::from(domain.width_m)
                    || i64::from(scene.origin_z) + i64::from(scene.depth_m)
                        > i64::from(domain.origin_z) + i64::from(domain.depth_m)
                {
                    return Err(WorldgenError::InvalidRules(
                        "scene escapes shared terrainDomain".into(),
                    ));
                }
            }
        }
        if self.format != WORLD_FORMAT {
            return Err(WorldgenError::InvalidFormat(self.format.clone()));
        }
        if self.version != WORLD_VERSION {
            return Err(WorldgenError::UnsupportedVersion(self.version));
        }
        if self.world.name.trim().is_empty() {
            return Err(WorldgenError::EmptyWorldName);
        }
        if self.world.render_mode != "2d" && self.world.render_mode != "2.5d" {
            return Err(WorldgenError::InvalidRenderMode(
                self.world.render_mode.clone(),
            ));
        }
        if self.scenes.is_empty() {
            return Err(WorldgenError::InvalidSceneId(
                "at least one scene is required".into(),
            ));
        }
        let mut scene_ids = BTreeMap::new();
        for scene in &self.scenes {
            if scene.scene_id.trim().is_empty() || scene_ids.insert(&scene.scene_id, true).is_some()
            {
                return Err(WorldgenError::InvalidSceneId(scene.scene_id.clone()));
            }
            if !(MIN_SCENE_METERS..=MAX_SCENE_WIDTH_METERS).contains(&scene.width_m)
                || !(MIN_SCENE_METERS..=MAX_SCENE_DEPTH_METERS).contains(&scene.depth_m)
                || scene.origin_x.checked_add(scene.width_m as i32).is_none()
                || scene.origin_z.checked_add(scene.depth_m as i32).is_none()
            {
                return Err(WorldgenError::InvalidSceneSize {
                    scene_id: scene.scene_id.clone(),
                    width_m: scene.width_m,
                    depth_m: scene.depth_m,
                });
            }
        }
        self.scene_graph.validate(&self.scenes)?;
        let mut landmark_ids = std::collections::BTreeSet::new();
        for landmark in &self.landmarks {
            if landmark.entity_id.trim().is_empty()
                || !landmark_ids.insert(&landmark.entity_id)
                || !landmark.rotation_y_deg.is_finite()
                || landmark.name.trim().is_empty()
                || landmark.entity_type.trim().is_empty()
                || landmark.purpose.trim().is_empty()
                || landmark.description.trim().is_empty()
                || landmark.asset_id.trim().is_empty()
                || landmark.width_m == 0
                || landmark.depth_m == 0
                || landmark.height_m == 0
            {
                return Err(WorldgenError::InvalidLandmark(landmark.entity_id.clone()));
            }
            landmark.grounding.validate(&landmark.entity_id)?;
            if let Some(bounds) = landmark.bounds {
                if !bounds.is_finite_and_ordered() {
                    return Err(WorldgenError::InvalidLandmark(landmark.entity_id.clone()));
                }
            }
            let Some(scene) = self
                .scenes
                .iter()
                .find(|scene| scene.scene_id == landmark.scene_id)
            else {
                return Err(WorldgenError::MissingLandmarkScene {
                    landmark: landmark.entity_id.clone(),
                    scene: landmark.scene_id.clone(),
                });
            };
            let local_x = i64::from(landmark.world_x) - i64::from(scene.origin_x);
            let local_z = i64::from(landmark.world_z) - i64::from(scene.origin_z);
            let (half_width, half_depth) = spatial_half_extents(
                landmark.width_m as f32,
                landmark.depth_m as f32,
                landmark.rotation_y_deg,
            );
            let half_width = half_width.ceil() as i64;
            let half_depth = half_depth.ceil() as i64;
            let may_be_clipped_by_scene = matches!(landmark.entity_type.as_str(), "lake" | "river");
            if !may_be_clipped_by_scene
                && (local_x - half_width < 0
                    || local_z - half_depth < 0
                    || local_x + half_width >= i64::from(scene.width_m)
                    || local_z + half_depth >= i64::from(scene.depth_m))
            {
                return Err(WorldgenError::LandmarkOutsideScene {
                    landmark: landmark.entity_id.clone(),
                    scene: scene.scene_id.clone(),
                });
            }
        }
        Ok(())
    }
}

fn edge_summary(
    height: &[u8],
    surface: &[u8],
    width: u32,
    depth: u32,
    horizontal: bool,
    index: u32,
) -> EdgeSummary {
    let payload = HeightPayload::parse(height, width, depth).expect("generated height payload");
    let length = if horizontal { width } else { depth };
    let mut min = i16::MAX;
    let mut max = i16::MIN;
    let mut counts = [0_usize; 256];
    for step in 0..length {
        let x = if horizontal { step } else { index };
        let z = if horizontal { index } else { step };
        let offset = (z * width + x) as usize;
        let h = (payload.metres(offset) * 4.).clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        min = min.min(h);
        max = max.max(h);
        counts[surface[offset] as usize] += 1;
    }
    let dominant_surface = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(value, _)| value as u8)
        .unwrap_or(0);
    EdgeSummary {
        min_quarter_m: min,
        max_quarter_m: max,
        dominant_surface,
    }
}

fn chunk_boundary_summary(
    height: &[u8],
    surface: &[u8],
    width: u32,
    depth: u32,
) -> ChunkBoundarySummary {
    ChunkBoundarySummary {
        west: edge_summary(height, surface, width, depth, false, 0),
        east: edge_summary(height, surface, width, depth, false, width - 1),
        north: edge_summary(height, surface, width, depth, true, 0),
        south: edge_summary(height, surface, width, depth, true, depth - 1),
    }
}

fn bake_natural_heightfield(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    scene_dir: &Path,
    precise: bool,
) -> WorldgenResult<BakedTerrain> {
    let mut terrain = BakedTerrain::new(scene)?;
    for chunk_z in 0..scene.depth_m.div_ceil(STREAM_CHUNK_METERS) {
        for chunk_x in 0..scene.width_m.div_ceil(STREAM_CHUNK_METERS) {
            let dx = chunk_x * STREAM_CHUNK_METERS;
            let dz = chunk_z * STREAM_CHUNK_METERS;
            let width = (scene.width_m - dx).min(STREAM_CHUNK_METERS);
            let depth = (scene.depth_m - dz).min(STREAM_CHUNK_METERS);
            let x = scene.origin_x + dx as i32;
            let z = scene.origin_z + dz as i32;
            let bytes = if precise {
                encode_millimetres(
                    width,
                    depth,
                    (0..depth).flat_map(|row| {
                        (0..width).map(move |col| {
                            terrain_height_metres(seed, x + col as i32, z + row as i32, water, &[])
                        })
                    }),
                )?
            } else {
                let mut bytes = Vec::with_capacity((width * depth * 2) as usize);
                for row in 0..depth {
                    for col in 0..width {
                        bytes.extend_from_slice(
                            &terrain_height_with_geometry(
                                seed,
                                x + col as i32,
                                z + row as i32,
                                water,
                            )
                            .to_le_bytes(),
                        );
                    }
                }
                bytes
            };
            terrain.insert_chunk(x, z, width, depth, &bytes)?;
            fs::write(
                scene_dir.join(format!("chunk-{chunk_x}-{chunk_z}.natural.height.bin")),
                bytes,
            )?;
        }
    }
    terrain.require_complete()?;
    Ok(terrain)
}

pub fn height_encoding_from_style(style: &serde_json::Value) -> WorldgenResult<HeightEncoding> {
    if style.get("heightPrecision").is_some_and(|v| !v.is_string()) {
        return Err(WorldgenError::InvalidRules(
            "heightPrecision must be a string".into(),
        ));
    }
    match style
        .get("heightPrecision")
        .and_then(serde_json::Value::as_str)
    {
        None | Some("quarter-meter") => Ok(HeightEncoding::QuarterMetre),
        Some("millimeter") => Ok(HeightEncoding::Millimetre),
        Some(value) => Err(WorldgenError::InvalidRules(format!(
            "unsupported heightPrecision {value:?}"
        ))),
    }
}
/// Where a scene's authoritative *natural* surface comes from.
///
/// The owner of the manifest declares this once, and it travels unchanged down
/// the same path the supply phase already uses (`plan_with_style` ->
/// `auto_roads` -> `frontage_supply::realize_candidate` -> the final encoder).
/// The demand-led supply phase must never infer the source from raster samples:
/// an explicit declaration is the only thing that authorises regenerating a
/// field, and a missing declaration refuses instead of guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalSource {
    /// The authored procedural field. `seed` is the scene's effective seed
    /// (`world.seed ^ scene.seedOffset`) and `precise` the style's height
    /// encoding. The production encoder may re-derive the surface from exactly
    /// these inputs, which is what the bake itself does.
    AuthoredSeed { seed: u64, precise: bool },
    /// The supplied raster is authoritative and is consumed exactly as handed
    /// over: imported rasters, edited rasters and synthetic fixtures. It is
    /// never regenerated, so an edit outside any sample point survives.
    SuppliedRaster,
    /// No declaration was made. Every path that needs an authoritative source
    /// refuses rather than silently using a zero seed or a raster fallback.
    Undeclared,
}

/// The effective seed of one scene, exactly as generation derives it.
pub fn effective_scene_seed(world_seed: u64, scene: &SceneSpec) -> u64 {
    world_seed ^ scene.seed_offset
}

/// Derive a scene's authoritative natural source from the trusted original
/// inputs alone.
///
/// Generation and the independent audit both call this, so one rule produces
/// the effective seed (`world.seed ^ scene.seedOffset`), the precision and the
/// source kind on both sides. The declaration is never read back out of the
/// published product and never guessed from raster samples. The generator only
/// authors a procedural field, so a manifest that declares any other terrain
/// source is refused rather than silently mapped onto `SuppliedRaster`.
pub fn declared_natural_source(
    manifest: &WorldManifest,
    scene: &SceneSpec,
) -> WorldgenResult<NaturalSource> {
    if !manifest
        .scenes
        .iter()
        .any(|declared| declared.scene_id == scene.scene_id)
    {
        return Err(WorldgenError::InvalidRules(format!(
            "scene {} is not declared by the manifest",
            scene.scene_id
        )));
    }
    match manifest
        .style
        .get("terrain")
        .and_then(serde_json::Value::as_str)
    {
        None | Some("continuous-heightfield") => {}
        Some(other) => {
            return Err(WorldgenError::InvalidRules(format!(
                "unsupported authored terrain source {other:?}: this build has no declared non-authored source for the independent audit"
            )))
        }
    }
    Ok(NaturalSource::AuthoredSeed {
        seed: effective_scene_seed(manifest.world.seed, scene),
        precise: height_encoding_from_style(&manifest.style)? == HeightEncoding::Millimetre,
    })
}

// Opt-in stderr telemetry is observational, never part of world identity or
// release acceptance. The external runner measures process resources.
struct BakeTrace { enabled: bool, started: std::time::Instant }
impl BakeTrace {
    fn new()->Self { Self { enabled:std::env::var_os("GLYPHWEAVE_BAKE_TRACE").is_some_and(|v|v=="1"),started:std::time::Instant::now() } }
    fn stage(&self,scene:&str,stage:&str) {
        if self.enabled {eprintln!("{}",serde_json::json!({"format":"glyphweave.bake-trace","event":"begin","sceneId":scene,"stage":stage,"elapsedMs":self.started.elapsed().as_millis()}));}
    }
}
/// The published bounded-cut ledger of ONE scene, or `None` when the style never
/// declared the policy.
///
/// The ledger is WRITTEN from the published rasters (the excavation the world
/// actually made) and then verified against the plan and the natural raster by
/// the same `bounded_cut::verify` a reader runs, so a bake that cannot prove its
/// own excavation fails instead of publishing it. A declared-but-illegal policy
/// publishes `enabled: false` plus the named reason - never a silent downgrade.
#[allow(clippy::too_many_arguments)]
fn bounded_cut_ledger_for(
    manifest: &WorldManifest,
    scene: &SceneSpec,
    plan: &road_network::RoadNetworkPlan,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    zones: Option<&[LandUseZone]>,
    water: WaterGeometry,
) -> WorldgenResult<Option<bounded_cut::CutLedger>> {
    if !bounded_cut::CutPolicy::declared(&manifest.style) {
        return Ok(None);
    }
    let (policy, not_enabled) = bounded_cut::CutPolicy::from_style(&manifest.style);
    if !policy.enabled() {
        return Ok(Some(bounded_cut::CutLedger::off(
            &manifest.style,
            not_enabled.unwrap_or(bounded_cut::NotEnabled::NotDeclared),
        )));
    }
    // A graded road's own flat top belongs to the road's ledger, so the pad
    // records are measured with those cells excluded (see `RoadFlatTop`).
    let mut cut_entities = plan.entities();
    cut_entities.extend(plan.site_entities());
    let road_tops = bounded_cut::road_flat_tops(&cut_entities);
    let pads = bounded_cut::measure_from_rasters(
        &plan.roadside_sites,
        natural,
        final_terrain,
        &road_tops,
    )
    .map_err(WorldgenError::InvalidRules)?;
    let mut ledger = bounded_cut::CutLedger {
        requested: Some(
            manifest
                .style
                .get("frontageProgram")
                .and_then(|program| program.get(bounded_cut::POLICY_KEY))
                .and_then(|value| value.as_str())
                .unwrap_or(bounded_cut::POLICY_VALUE)
                .to_string(),
        ),
        enabled: true,
        not_enabled: None,
        max_cut_mm: Some(policy.cap_mm()),
        pads,
        roads: bounded_cut::measure_road_cuts(scene, plan, natural, final_terrain, policy)
            .map_err(WorldgenError::InvalidRules)?,
        cut_cells: 0,
        cut_depth_sum_mm: 0,
        cut_volume_m3: 0.,
        max_cut_depth_mm: 0,
    };
    ledger.totals();
    if let Some(zones) = zones {
        let audit = bounded_cut::verify(
            scene,
            Some(plan),
            zones,
            water,
            natural,
            final_terrain,
            Some(&ledger),
            policy,
        )
        .map_err(WorldgenError::InvalidRules)?;
        if audit.status != "pass" {
            return Err(WorldgenError::InvalidRules(format!(
                "bounded cut ledger does not verify for {}: {}",
                scene.scene_id,
                audit.failures.join("; ")
            )));
        }
    } else if !ledger.pads.is_empty() || !ledger.roads.is_empty() {
        return Err(WorldgenError::InvalidRules(
            "bounded cut requires the scene's zoning to verify its pads and roads".into(),
        ));
    }
    Ok(Some(ledger))
}

pub fn bake_world(manifest: &WorldManifest, output: &Path) -> WorldgenResult<WorldIndex> {
    let trace=BakeTrace::new();
    trace.stage("", "manifest-validation");
    manifest.validate()?;
    // Compile the single effective program set for this bake. Roads, frontage,
    // services, agriculture and residual land all consume this one value; no
    // module fills its own default.
    let compiled_style =
        crate::settlement::morphology_compile::compile(&manifest.style, &manifest.scenes)
            .map_err(WorldgenError::InvalidRules)?;
    let effective_style = compiled_style
        .as_ref()
        .map(|compiled| compiled.style.clone())
        .unwrap_or_else(|| manifest.style.clone());
    let precise = height_encoding_from_style(&manifest.style)? == HeightEncoding::Millimetre;
    let rules_mode = manifest
        .style
        .get("rulesMode")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|mode| mode == "rules");
    let generation_profile = manifest
        .style
        .get("generationProfile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("legacy");
    if !matches!(
        generation_profile,
        "legacy" | "prototype" | "strict" | "release"
    ) {
        return Err(WorldgenError::InvalidRules(format!(
            "unsupported generationProfile {generation_profile:?}"
        )));
    }
    if matches!(generation_profile, "strict" | "release") && !rules_mode {
        return Err(WorldgenError::InvalidRules(
            "generationProfile strict/release requires style.rulesMode=rules".into(),
        ));
    }
    let rules_dir = rules_mode.then(|| rules_dir_from_style(&manifest.style));
    if let Some(dir) = &rules_dir {
        crate::rules::ObjectRegistry::load_dir(dir)
            .map_err(|error| WorldgenError::InvalidRules(error.to_string()))?;
    }
    if output.exists() && fs::read_dir(output)?.next().is_some() {
        return Err(WorldgenError::OutputNotEmpty(output.display().to_string()));
    }
    fs::create_dir_all(output)?;
    let geography_plan = natural_geography::compile(&manifest.style)
        .map_err(WorldgenError::InvalidRules)?;
    if let Some(plan) = &geography_plan {
        fs::write(
            output.join("natural-geography-plan.json"),
            serde_json::to_vec_pretty(plan)?,
        )?;
    }
    let slice_plan =
        urban_slice::compile(&manifest.style).map_err(WorldgenError::InvalidRules)?;
    if let Some(plan) = &slice_plan {
        fs::write(
            output.join("urban-slice-plan.json"),
            serde_json::to_vec_pretty(plan)?,
        )?;
    }
    if let Some(compiled) = &compiled_style {
        fs::write(
            output.join("effective-programs.json"),
            serde_json::to_vec_pretty(&compiled.report)?,
        )?;
    }
    let revision = world_manifest_revision(manifest)?;
    let mut scene_paths = Vec::new();
    let mut baked_rules_report = crate::rules::ValidationReport::default();
    let mut placement_reports = Vec::<serde_json::Value>::new();
    let mut pad_completion_reports = Vec::<serde_json::Value>::new();
    let mut engineering_reports = Vec::new();
    let mut natural_only_reports = Vec::<natural_only::SceneAudit>::new();
    for scene in &manifest.scenes {
        let scene_dir = output.join("scenes").join(&scene.scene_id);
        fs::create_dir_all(&scene_dir)?;
        let chunk_count_x = scene.width_m.div_ceil(STREAM_CHUNK_METERS);
        let chunk_count_z = scene.depth_m.div_ceil(STREAM_CHUNK_METERS);
        // Authored water establishes the legacy natural boundary. Bake that
        // layer first; generated infrastructure modifies only the final layer.
        let landmarks: Vec<LandmarkSpec> = manifest
            .landmarks
            .iter()
            .filter(|item| item.scene_id == scene.scene_id)
            .cloned()
            .map(|mut landmark| {
                landmark.normalize_spatial_semantics();
                landmark
            })
            .collect();
        let water = water_geometry(
            water_kind(&manifest.style),
            &landmarks,
            scene,
            &manifest.style,
        );
        let service_program =
            services::from_style(&effective_style).map_err(WorldgenError::InvalidRules)?;
        if service_program.as_ref().is_some_and(|p| {
            p.requests.iter().any(|r| r.scene_id == scene.scene_id)
                || p.housing_minimum.contains_key(&scene.scene_id)
        }) && !road_network::enabled(&manifest.style, water)
        {
            return Err(WorldgenError::InvalidRules(
                "explicit service program requires supported road-first scene".into(),
            ));
        }
        if crate::settlement::farm_program::for_scene(&effective_style,scene).map_err(WorldgenError::InvalidRules)?.is_some()
            && !road_network::enabled(&manifest.style,water) {
            return Err(WorldgenError::InvalidRules("explicit agriculture program requires supported road-first scene".into()));
        }
        // Establish and publish the natural fact layer before any human
        // candidate exists. Only heights are baked here, not unused LOD or
        // derivative payloads; final engineering has its own later bake.
        trace.stage(&scene.scene_id,"natural-heightfield");
        let natural_samples = bake_natural_heightfield(
            manifest.world.seed ^ scene.seed_offset,
            scene,
            water,
            &scene_dir,
            precise,
        )?;
        natural_samples.require_complete()?;

        trace.stage(&scene.scene_id,"natural-drainage");
        let natural_terrain = {
            let kind = match water.kind {
                WaterKind::None => "none",
                WaterKind::Lake => "lake",
                WaterKind::River => "river",
            };
            let snapshot = NaturalTerrainSnapshot::authored_water(
                manifest.world.seed ^ scene.seed_offset,
                &scene.scene_id,
                scene.width_m,
                scene.depth_m,
                kind,
                water.center_x as f32,
                water.center_z as f32,
            )
            .with_derived_drainage(&natural_samples, scene)?;
            fs::write(
                scene_dir.join("natural-terrain.json"),
                serde_json::to_vec_pretty(&snapshot)?,
            )?;
            let drainage_audit = audit_drainage_graph(&snapshot.drainage);
            fs::write(
                scene_dir.join("natural-terrain-audit.json"),
                serde_json::to_vec_pretty(&drainage_audit)?,
            )?;
            snapshot
        };
        let route_evolution_plan =
            route_evolution::plan(scene, &natural_samples, water, &effective_style)?;
        if let Some(route_plan) = &route_evolution_plan {
            fs::write(
                scene_dir.join("route-evolution-plan.json"),
                serde_json::to_vec_pretty(&route_plan)?,
            )?;
            route_evolution::require_pass(&route_plan)
                .map_err(|error| WorldgenError::InvalidRules(error.to_string()))?;
        }
        trace.stage(&scene.scene_id,"road-and-service-planning");
        let planned_zones = match LandUseProfile::from_style(&manifest.style) {
            Some(profile) => Some(
                plan_landuse_zones_for_style(
                    scene,
                    &profile,
                    &effective_style,
                    &natural_samples,
                    water,
                    service_program.as_ref(),
                    declared_natural_source(manifest, scene)?,
                )?
                .zones,
            ),
            None => None,
        };
        let mut road_plan = if road_network::enabled(&manifest.style, water) {
            Some(road_network::plan_with_style_declared(
                scene,
                &natural_samples,
                water,
                planned_zones.as_deref(),
                service_program.as_ref(),
                &effective_style,
                // The bake owns the manifest, so it is the one place that may
                // declare the authored seed as the authoritative natural
                // source. Everything downstream receives this unchanged, and
                // the independent audit derives it with the same rule.
                declared_natural_source(manifest, scene)?,
            )?)
        } else {
            None
        };
        if let Some(plan) = &road_plan {
            fs::write(
                scene_dir.join("road-network-plan.json"),
                serde_json::to_vec_pretty(plan)?,
            )?;
        }
        trace.stage(&scene.scene_id,"development-partition");
        let mut development_plan = if let Some(roads) = &road_plan {
            let zones = planned_zones.as_deref().ok_or_else(|| {
                WorldgenError::InvalidRules("road development requires land-use profile".into())
            })?;
            Some(
                development::plan(scene, zones, roads)
                    .map_err(WorldgenError::InvalidRules)?,
            )
        } else {
            None
        };
        // Authored landmarks are rendered even when the procedural entity list
        // filters them out. Validate them before generation so a building cannot
        // silently disappear from the Rust audit and then reappear on top of the
        // water in the HTML adapter.
        let zoning_mask = if road_plan.is_some() {
            planned_zones
                .as_deref()
                .map(|z| zoning::ZoningMask::new(scene, z))
                .transpose()?
        } else {
            None
        };
        let terrain_context = TerrainContext {
            seed: manifest.world.seed ^ scene.seed_offset,
            water,
            precise,
            zoning: zoning_mask.as_ref(),
        };
        validate_authored_landmarks_clear_water(&landmarks, scene, water)?;
        trace.stage(&scene.scene_id,"entity-generation-and-preflight");
        let (mut entities, placement_outcome, development_proposals, candidate_claims) =
            generate_entities_with_profile_report(
                manifest.world.seed ^ scene.seed_offset,
                scene,
                &landmarks,
                water,
                &effective_style,
                road_plan.as_ref(),
                development_plan.as_ref(),
                planned_zones.as_deref(),
            )?;
        if let Some(route_plan) = &route_evolution_plan {
            let removed = route_evolution::retain_clearance(&mut entities, route_plan, 2.0);
            engineering_reports.push(serde_json::json!({
                "sceneId": scene.scene_id,
                "phase": "route-evolution-natural-clearance",
                "removedNaturalEntities": removed,
            }));
        }
        // OPT-IN `style.terrainPolicy.earthworkBlend`: every structure of this
        // scene gets the fill-transition ring its own footprint
        // (`"derived-from-footprint"`) or its own measured fill depth
        // (`"derived-from-fill"`) derives, once, before any carve is planned, so the
        // carve list, the rasters, the completion pass and every audit read the same
        // number. The fill rule measures the natural ground under each entity's own
        // flat top, which is why the natural raster is still alive here; it is
        // dropped immediately afterwards, before the bake allocates its own.
        // A world that never asked is untouched: `stamp` returns 0 and no entity
        // gains the field, so its published bytes are the historical ones.
        earthwork_blend::stamp(
            &mut entities,
            &manifest.style,
            &earthwork_blend::planned_ids(development_plan.as_ref()),
            &natural_samples,
        );
        if let Some((proposed_count, outcome)) = placement_outcome {
            let mut by_reason = BTreeMap::<String, usize>::new();
            for reject in &outcome.rejected {
                *by_reason.entry(reject.reason.clone()).or_default() += 1;
            }
            placement_reports.push(serde_json::json!({
                "sceneId": scene.scene_id,
                "proposedItems": proposed_count,
                "placedItems": outcome.placed.len(),
                "rejectedCandidates": outcome.rejected.len(),
                "rejectedByReason": by_reason,
                "rejects": outcome.rejected,
            }));
            fs::write(
                output.join("rules-placement.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "format": "glyphweave.rules-placement",
                    "version": 1,
                    "scenes": placement_reports,
                }))?,
            )?;
        }
        let mut entities = entities;
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        sanitize_generated_transport_collisions(&mut entities);
        for e in &entities { if let Some(p)=&e.grounding.road_profile { p.validate_entity(e)?; } frontage::validate_entity(e)?; }
        let mut carves = plan_terrain_carves(&entities);
        // Reconcile grounded entities against the preliminary carved surface
        // so roadbeds cannot make a nearby object fail final baked audit.
        for _ in 0..2 {
            rebase_grounded_entities_to_carved_surface(
                manifest.world.seed ^ scene.seed_offset,
                water,
                &mut entities,
                &carves,
            );
            carves = plan_terrain_carves(&entities);
        }
        // The plan accepted its pads from its OWN earthwork, because the rule-engine
        // entities did not exist yet. They do now: a building platform standing over
        // a planned pad's declared top is the coupling this closes. Bounded, and by
        // name - a pad that cannot be completed is taken out of the plan, never left
        // to blow the whole scene's cut ledger up (see `pad_completion`).
        if let (Some(plan), Some(zones), Some(mask)) = (
            road_plan.as_mut(),
            planned_zones.as_deref(),
            zoning_mask.as_ref(),
        ) {
            let done = complete_pad_tops(
                manifest.world.seed ^ scene.seed_offset,
                scene,
                plan,
                &mut entities,
                &mut carves,
                zones,
                mask,
                water,
                &mut development_plan,
            )?;
            if !done.records.is_empty() {
                // The plan's own record is what an auditor reads; the early write
                // above predates the refusals, so republish it.
                fs::write(
                    scene_dir.join("road-network-plan.json"),
                    serde_json::to_vec_pretty(&*plan)?,
                )?;
                pad_completion_reports.push(serde_json::json!({
                    "sceneId": scene.scene_id,
                    "rounds": done.rounds,
                    "padsChecked": done.checked,
                    "padsRefused": done.refused,
                    "padsDeepened": done.deepened,
                    "pads": done.records,
                }));
            }
        }
        if let Some(dir) = &rules_dir {
            let (repaired, final_carves, mut report) = engineered_placement::reconcile(
                manifest.world.seed ^ scene.seed_offset,
                scene,
                &landmarks,
                &entities,
                water,
                dir,
                &terrain_context,
            )?;
            entities = repaired;
            carves = final_carves;
            // Rule-engine `fallback = "skip"` may remove a planned frontage pad
            // after the first completion pass. Run the same bounded completion
            // transaction again so the plan, conversions, development plan,
            // carves and entity list cannot diverge after placement.
            if let (Some(plan), Some(zones), Some(mask)) = (
                road_plan.as_mut(),
                planned_zones.as_deref(),
                zoning_mask.as_ref(),
            ) {
                let done = complete_pad_tops(
                    manifest.world.seed ^ scene.seed_offset,
                    scene,
                    plan,
                    &mut entities,
                    &mut carves,
                    zones,
                    mask,
                    water,
                    &mut development_plan,
                )?;
                if !done.records.is_empty() {
                    fs::write(
                        scene_dir.join("road-network-plan.json"),
                        serde_json::to_vec_pretty(&*plan)?,
                    )?;
                    pad_completion_reports.push(serde_json::json!({
                        "sceneId": scene.scene_id,
                        "phase": "post-rules-reconcile",
                        "rounds": done.rounds,
                        "padsChecked": done.checked,
                        "padsRefused": done.refused,
                        "padsDeepened": done.deepened,
                        "pads": done.records,
                    }));
                }
            }
            // A pad the ROAD PLAN planned must be accounted for end to end, and a
            // pad it REFUSED by name must be nowhere. The reconcile report is the
            // one artifact that publishes both id lists, so both claims travel with
            // it and `verify_published_placement` can recompute them from the
            // published bytes. This is the triangle the defect fell through: a pad
            // stayed in the plan and in the ledger while its entity was deleted, and
            // no audit could see all three at once.
            if let Some(plan) = &road_plan {
                let planned: Vec<&str> = plan
                    .roadside_sites
                    .iter()
                    .map(|site| site.id.as_str())
                    .collect();
                if !planned.is_empty() {
                    report["plannedPadIds"] = serde_json::json!(planned);
                }
                // A pad removed by a post-reconcile rules-engine `skip` is already
                // named by the report's input/output/skip triangle. It is not a
                // planner refusal that existed before reconcile, so publishing it
                // in `refusedEntityIds` would contradict `inputEntityIds`.
                let input_ids: BTreeSet<&str> = report
                    .get("inputEntityIds")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                let refused: Vec<&str> = road_network::refused_pad_ids(plan)
                    .into_iter()
                    .filter(|id| !input_ids.contains(id))
                    .collect();
                if !refused.is_empty() {
                    report["refusedEntityIds"] = serde_json::json!(refused);
                }
            }
            engineering_reports.push(report);
            fs::write(
                output.join("engineering-placement.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "format":"glyphweave.engineering-placement", "version":1,
                    "scenes":engineering_reports,
                }))?,
            )?;
        }
        let mut chunks = Vec::new();
        trace.stage(&scene.scene_id,"final-heightfield-and-semantics");
        let mut baked_samples = BakedTerrain::new(scene)?;
        for chunk_z in 0..chunk_count_z {
            for chunk_x in 0..chunk_count_x {
                let valid_width_m =
                    (scene.width_m - chunk_x * STREAM_CHUNK_METERS).min(STREAM_CHUNK_METERS);
                let valid_depth_m =
                    (scene.depth_m - chunk_z * STREAM_CHUNK_METERS).min(STREAM_CHUNK_METERS);
                let base_x = scene.origin_x + (chunk_x * STREAM_CHUNK_METERS) as i32;
                let base_z = scene.origin_z + (chunk_z * STREAM_CHUNK_METERS) as i32;
                let stem = format!("chunk-{chunk_x}-{chunk_z}");
                let height_file = format!("{stem}.height.bin");
                let natural_height_file = format!("{stem}.natural.height.bin");
                let surface_file = format!("{stem}.surface.bin");
                let lod2_file = format!("{stem}.lod2.bin");
                let terrain_file = format!("{stem}.terrain.bin");
                // A large scene may contain thousands of pads. Restrict the
                // per-cell terrain walk to carves whose expanded bounds touch
                // this chunk; unrelated city districts no longer cost O(N)
                // for every raster cell.
                let chunk_carves: Vec<TerrainCarve> = carves
                    .iter()
                    .cloned()
                    .filter(|carve| {
                        carve_intersects_chunk(carve, base_x, base_z, valid_width_m, valid_depth_m)
                    })
                    .collect();
                let (height, surface, lod2, terrain) = generate_chunk_encoded(
                    manifest.world.seed ^ scene.seed_offset,
                    base_x,
                    base_z,
                    valid_width_m,
                    valid_depth_m,
                    water,
                    &chunk_carves,
                    precise,
                    zoning_mask.as_ref(),
                );
                fs::write(scene_dir.join(&height_file), &height)?;
                fs::write(scene_dir.join(&surface_file), &surface)?;
                fs::write(scene_dir.join(&terrain_file), &terrain)?;
                baked_samples.insert_chunk(
                    base_x,
                    base_z,
                    valid_width_m,
                    valid_depth_m,
                    &height,
                )?;
                fs::write(scene_dir.join(&lod2_file), &lod2)?;
                // Stream the four payloads into BLAKE3 instead of allocating
                // one additional concatenated buffer per 512m chunk.
                let mut hasher = blake3::Hasher::new();
                hasher.update(&height);
                hasher.update(&surface);
                hasher.update(&lod2);
                hasher.update(&terrain);
                let hash = hasher.finalize().to_hex().to_string();
                let descriptor = ChunkDescriptor {
                    chunk_x,
                    chunk_z,
                    world_x: base_x,
                    world_z: base_z,
                    valid_width_m,
                    valid_depth_m,
                    height_file,
                    natural_height_file: Some(natural_height_file),
                    surface_file,
                    lod2_file,
                    terrain_file: Some(terrain_file),
                    hash,
                    boundary: chunk_boundary_summary(
                        &height,
                        &surface,
                        valid_width_m,
                        valid_depth_m,
                    ),
                };
                fs::write(
                    scene_dir.join(format!("{stem}.json")),
                    serde_json::to_vec_pretty(&descriptor)?,
                )?;
                chunks.push(descriptor);
            }
        }
        // One compact raster and one audit preparation path for bake and CLI.
        trace.stage(&scene.scene_id,"final-entity-access-and-audit");
        baked_samples.require_complete()?;
        rebase_entities_to_baked_surface(&mut entities, &baked_samples);
        if water.road_water_crossings() {
            // Opt-in waterfront: a roadside pad's declared top is the plan's
            // obligation, and the audit holds the entity to it exactly. In a
            // waterfront block a legally higher neighbouring lot can raise the
            // raster around the pad, and the raster-driven rebase would then move
            // the pad off its own declaration - a plan/entity disagreement the
            // audit can only refuse. Restore the planned transform instead. This
            // is inert for every world that did not opt in, and inert in a correct
            // world even here: a pad whose raster exceeds its declaration already
            // fails the audit by name.
            if let Some(plan) = &road_plan {
                for source in plan.site_entities() {
                    if let Some(entity) = entities
                        .iter_mut()
                        .find(|entity| entity.entity_id == source.entity_id)
                    {
                        *entity = source;
                    }
                }
            }
            // The restored entity IS the plan's own record, which carries the
            // derived earthwork ring this round publishes; the entity must carry
            // the same number or the development audit reads a plan/entity split
            // that the world does not have. The natural raster this round measured
            // the fill rings against is gone by now, and it is not needed again: a
            // ring is measured once, and this pass may only CONFIRM one.
            earthwork_blend::stamp_restored(
                &mut entities,
                &manifest.style,
                &earthwork_blend::planned_ids(development_plan.as_ref()),
            );
        }
        rebase_generated_transport_to_baked_surface(
            &mut entities,
            &landmarks,
            &baked_samples,
            Some(water),
        );
        // The graph and persisted access surfaces are derived from this final
        // entity slice. Fixed frontage is protected inside
        // `place_generated_entrances`; do not restore an entity after graph
        // construction, because that would make the graph stale.
        let mut access_graph =
            access::place_generated_entrances(&mut entities, &landmarks, &baked_samples, water);
        // Final planned/authored obligations cannot be deleted to improve a gate.
        let protected_access_ids: std::collections::BTreeSet<String> = landmarks
            .iter()
            .map(|l| l.entity_id.clone())
            .chain(
                development_plan
                    .iter()
                    .flat_map(|p| p.buildings.iter().map(|b| b.entity.entity_id.clone())),
            )
            .collect();
        let rejected_access = access::rejectable_candidates(&access_graph, &protected_access_ids);
        if !rejected_access.is_empty() {
            entities.retain(|entity| !rejected_access.contains(&entity.entity_id));
            access_graph =
                access::place_generated_entrances(&mut entities, &landmarks, &baked_samples, water);
            engineering_reports.push(serde_json::json!({
                "sceneId": scene.scene_id,
                "phase": "access-candidate-rejection",
                "rejectedGeneratedBuildings": rejected_access,
                "reason": "no generated facade produced a verified route within the strict access contract"
            }));
        }
        let access_surfaces = bake_access_surfaces(&access_graph, &baked_samples)?;
        if let Some(rules_dir) = &rules_dir {
            let audit = audit_baked_scene(
                manifest.world.seed ^ scene.seed_offset,
                scene,
                &landmarks,
                &entities,
                water,
                rules_dir,
                &baked_samples,
            )?;
            merge_validation_report(&mut baked_rules_report, &audit);
            if audit.rejected_items > 0 {
                let _ = fs::write(
                    output.join("rules-audit.json"),
                    serde_json::to_vec_pretty(&baked_rules_report)?,
                );
                return Err(WorldgenError::RulesAuditFailed {
                    scene_id: scene.scene_id.clone(),
                    checked: audit.checked_items,
                    rejected: audit.rejected_items,
                });
            }
        }

        let mut bounded_cut_ledger: Option<bounded_cut::CutLedger> = None;
        if let Some(plan) = &road_plan {
            let mut natural = BakedTerrain::new(scene)?;
            for chunk in &chunks {
                let file = chunk
                    .natural_height_file
                    .as_ref()
                    .ok_or_else(|| TerrainError("missing natural chunk for road audit".into()))?;
                natural.insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &fs::read(scene_dir.join(file))?,
                )?;
            }
            let audit = road_network::verify_with_style_declared(
                plan,
                scene,
                &natural,
                &baked_samples,
                water,
                &entities,
                planned_zones.as_deref(),
                service_program.as_ref(),
                &effective_style,
                declared_natural_source(manifest, scene)?,
            )?;
            let protection = zoning::audit_protection(
                scene,
                planned_zones.as_deref().expect("zoned road source"),
                &natural,
                &baked_samples,
                &entities,
            )?;
            fs::write(
                scene_dir.join("land-protection-audit.json"),
                serde_json::to_vec_pretty(&protection)?,
            )?;
            if let Some(joint) = &plan.joint_layout {
                let audit = joint_frontage::audit(plan, &entities, &access_graph);
                fs::write(
                    scene_dir.join("joint-layout.json"),
                    serde_json::to_vec_pretty(joint)?,
                )?;
                fs::write(
                    scene_dir.join("joint-layout-audit.json"),
                    serde_json::to_vec_pretty(&audit)?,
                )?;
            }
            if !plan.service_outcomes.is_empty() {
                let service_audit = services::audit(plan, &entities, &access_graph);
                fs::write(
                    scene_dir.join("service-program-outcomes.json"),
                    serde_json::to_vec_pretty(&plan.service_outcomes)?,
                )?;
                fs::write(
                    scene_dir.join("service-program-audit.json"),
                    serde_json::to_vec_pretty(&service_audit)?,
                )?;
            }
            // The OPT-IN bounded-cut ledger, from what the world ACTUALLY
            // excavated (natural -> final raster), published beside the earthwork
            // operations. `None` whenever the style never declared
            // `frontageProgram.terrainPolicy`, which is why every other world's
            // `engineered-terrain.json` keeps its exact bytes.
            bounded_cut_ledger = bounded_cut_ledger_for(
                manifest,
                scene,
                plan,
                &natural,
                &baked_samples,
                planned_zones.as_deref(),
                water,
            )?;
            fs::write(
                scene_dir.join("road-network-audit.json"),
                serde_json::to_vec_pretty(&audit)?,
            )?;
            if let Some(district_audit) =
                crate::settlement::district_program::audit(&effective_style, scene, plan)
                    .map_err(WorldgenError::InvalidRules)?
            {
                fs::write(
                    scene_dir.join("urban-district-audit.json"),
                    serde_json::to_vec_pretty(&district_audit)?,
                )?;
            }
        }
        // A style that declared the policy but produced no road plan has no pad
        // that could have been cut: the record says so instead of vanishing.
        if bounded_cut_ledger.is_none() && bounded_cut::CutPolicy::declared(&manifest.style) {
            let (policy, not_enabled) = bounded_cut::CutPolicy::from_style(&manifest.style);
            bounded_cut_ledger = Some(if policy.enabled() {
                let mut ledger = bounded_cut::CutLedger {
                    requested: Some(bounded_cut::POLICY_VALUE.into()),
                    enabled: true,
                    not_enabled: None,
                    max_cut_mm: Some(policy.cap_mm()),
                    pads: vec![],
                    roads: vec![],
                    cut_cells: 0,
                    cut_depth_sum_mm: 0,
                    cut_volume_m3: 0.,
                    max_cut_depth_mm: 0,
                };
                ledger.totals();
                ledger
            } else {
                bounded_cut::CutLedger::off(
                    &manifest.style,
                    not_enabled.unwrap_or(bounded_cut::NotEnabled::NotDeclared),
                )
            });
        }
        let engineered_terrain = {
            let operations: Vec<EngineeredTerrainOperation> = carves
                .iter()
                .enumerate()
                .map(|(index, carve)| EngineeredTerrainOperation {
                    id: format!("{}.carve.{index}", scene.scene_id),
                    kind: if carve.disturbance == 255 {
                        "roadbed"
                    } else {
                        "building_or_parcel_pad"
                    }
                    .into(),
                    entity_id: carve.profile.as_deref().and_then(|p|entities.iter().find(|e|e.grounding.road_profile.as_ref()==Some(p))).map(|e|e.entity_id.clone()).unwrap_or_else(||format!("generated.terrain-carve.{index}")),
                    center_x: carve.cx as f32,
                    center_z: carve.cz as f32,
                    half_width_m: carve.half_w as f32,
                    half_depth_m: carve.half_d as f32,
                    target_height_m: carve.target_h_m as f32,
                    road_profile: carve.profile.as_deref().cloned(),
                    blend_m: carve.blend_m as f32,
                    priority: carve.priority,
                    status: "applied-to-final-heightfield".into(),
                })
                .collect();
            let works_policy = crate::engineering_works::policy_from_style(&manifest.style)
                .map_err(WorldgenError::InvalidRules)?;
            let crossings: &[road_network::WaterCrossing] = road_plan
                .as_ref()
                .map(|plan| plan.water_crossings.as_slice())
                .unwrap_or(&[]);
            let terrain_works = crate::engineering_works::plan(
                &operations,
                &natural_samples,
                &baked_samples,
                crossings,
                works_policy,
            );
            let snapshot = EngineeredTerrainSnapshot::new(operations);
            let snapshot = match bounded_cut_ledger.take() {
                Some(ledger) => snapshot.with_bounded_cut(ledger),
                None => snapshot,
            };
            let snapshot = match terrain_works {
                Some(works) => {
                    fs::write(
                        scene_dir.join("terrain-works.json"),
                        serde_json::to_vec_pretty(&works)?,
                    )?;
                    snapshot.with_terrain_works(works)
                }
                None => snapshot,
            };
            let drainage_feedback = manifest
                .style
                .get("engineeringDrainage")
                .and_then(serde_json::Value::as_str)
                == Some("audit");
            let snapshot = if drainage_feedback {
                let engineered_graph = crate::natural::derive_drainage_graph(&baked_samples, scene)
                    .map_err(|e| WorldgenError::InvalidRules(e.to_string()))?;
                let impact = crate::engineering_drainage::compare(
                    &natural_terrain.drainage,
                    &engineered_graph,
                );
                fs::write(
                    scene_dir.join("engineered-drainage.json"),
                    serde_json::to_vec_pretty(&impact)?,
                )?;
                snapshot.with_drainage_impact(impact)
            } else {
                snapshot
            };
            fs::write(
                scene_dir.join("engineered-terrain.json"),
                serde_json::to_vec_pretty(&snapshot)?,
            )?;
            snapshot
        };
        // The rule engine's own provenance for the buildings it placed. Kept out
        // of every world that publishes no development plan: with no ledger to
        // audit there is no rule building to distinguish, and those worlds keep
        // their historical bytes.
        let rule_building_candidates = development_plan
            .as_ref()
            .map(|plan| {
                development::rule_building_candidates(
                    scene,
                    plan,
                    &entities,
                    &candidate_claims,
                )
            })
            .unwrap_or_default();
        let mut index = SceneIndex {
            natural_terrain,
            engineered_terrain,
            scene_id: scene.scene_id.clone(),
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            chunk_size_m: STREAM_CHUNK_METERS,
            chunk_count_x,
            chunk_count_z,
            chunks,
            access_graph,
            access_surfaces,
            landmarks,
            entities,
            rule_building_candidates,
            land_use_budget: None,
            land_use_zones: Vec::new(),
            settlement_graph: None,
            parcel_plan: None,
            road_network_plan: road_plan,
            development_plan,
            landcover_plan: None,
            landcover_surfaces: None,
            development_proposals,
        };
        if let Some(profile) = LandUseProfile::from_style(&manifest.style) {
            // The plan the bake decided, not a second derivation of it: a scene whose style opted
            // into the axis-and-node growth publishes the zones its road plan, development plan and
            // ledger were built from.
            let zones = planned_zones
                .clone()
                .unwrap_or_else(|| plan_landuse_zones(scene, &profile));
            fs::write(
                scene_dir.join("land-use-zones.json"),
                serde_json::to_vec_pretty(&zones)?,
            )?;
            index.land_use_zones = zones;
            if let Some(plan) = &index.development_plan {
                let landcover = development::plan_landcover_with_program(scene, plan, &effective_style)
                    .map_err(WorldgenError::InvalidRules)?;
                trace.stage(&scene.scene_id,"landcover-realization");
                let surfaces = landcover::realize(
                    scene,
                    plan,
                    &landcover,
                    &baked_samples,
                    water,
                    &index.entities,
                )
                .map_err(WorldgenError::InvalidRules)?;
                trace.stage(&scene.scene_id,"landcover-independent-reconstruction");
                let landcover_audit = landcover::verify(
                    &surfaces,
                    scene,
                    plan,
                    &landcover,
                    &baked_samples,
                    water,
                    &index.entities,
                )
                .map_err(WorldgenError::InvalidRules)?;
                trace.stage(&scene.scene_id,"landcover-serialization");
                fs::write(
                    scene_dir.join("landcover-surfaces.json"),
                    serde_json::to_vec_pretty(&surfaces)?,
                )?;
                index.landcover_surfaces = Some(surfaces);
                fs::write(
                    scene_dir.join("landcover-plan.json"),
                    serde_json::to_vec_pretty(&landcover)?,
                )?;
                fs::write(
                    scene_dir.join("landcover-audit.json"),
                    serde_json::to_vec_pretty(&landcover_audit)?,
                )?;
                index.landcover_plan = Some(landcover);
                let mut audit = development::verify(
                    plan,
                    scene,
                    &index.land_use_zones,
                    index
                        .road_network_plan
                        .as_ref()
                        .expect("development has road source"),
                    &index.entities,
                    &index.access_graph,
                    &index.rule_building_candidates,
                )
                .map_err(WorldgenError::InvalidRules)?;
                landcover::apply_evidence(&mut audit, &landcover_audit);
                fs::write(
                    scene_dir.join("development-plan.json"),
                    serde_json::to_vec_pretty(plan)?,
                )?;
                fs::write(
                    scene_dir.join("development-audit.json"),
                    serde_json::to_vec_pretty(&audit)?,
                )?;
            } else {
                let mut parcel_plan = plan_parcels(&index.land_use_zones);
                realize_parcel_plan(&mut parcel_plan, &index.entities);
                realize_infrastructure_plan(&mut parcel_plan.infrastructure, &index.entities);
                validate_parcel_plan(&parcel_plan).map_err(WorldgenError::InvalidRules)?;
                fs::write(
                    scene_dir.join("parcel-plan.json"),
                    serde_json::to_vec_pretty(&parcel_plan)?,
                )?;
                fs::write(
                    scene_dir.join("infrastructure-plan.json"),
                    serde_json::to_vec_pretty(&parcel_plan.infrastructure)?,
                )?;
                index.parcel_plan = Some(parcel_plan);
            }
            trace.stage(&scene.scene_id,"landuse-accounting");
            let budget = plan_landuse_budget(&index, &profile);
            fs::write(
                scene_dir.join("land-use-budget.json"),
                serde_json::to_vec_pretty(&budget)?,
            )?;
            index.land_use_budget = Some(budget);
            if index.development_plan.is_some() {
                let ledger = crate::settlement::landuse_ledger::analyze(&index, &profile)
                    .map_err(WorldgenError::InvalidRules)?;
                fs::write(
                    scene_dir.join("land-use-ledger.json"),
                    serde_json::to_vec_pretty(&ledger)?,
                )?;
            }
        }
        if index.development_plan.is_some() {
            fs::write(
                scene_dir.join("development-proposals.json"),
                serde_json::to_vec_pretty(&index.development_proposals)?,
            )?;
        }
        // SettlementGraph is written after land-use planning so its final
        // entity relations observe the same parcel plan that was persisted.
        let settlement_graph = if let Some(plan) = &index.development_plan {
            development::settlement_graph(plan, &index.entities)
        } else {
            derive_settlement_graph(scene, &index.entities)
        };
        fs::write(
            scene_dir.join("settlement-graph.json"),
            serde_json::to_vec_pretty(&settlement_graph)?,
        )?;
        index.settlement_graph = Some(settlement_graph);
        let service_coverage_requested = effective_style
            .get("serviceCoverage")
            .and_then(serde_json::Value::as_str)
            == Some("audit");
        if service_coverage_requested {
            let profile = super::settlement::morphology::resolve(&effective_style)
                .map_err(WorldgenError::InvalidRules)?
                .ok_or_else(|| {
                    WorldgenError::InvalidRules(
                        "serviceCoverage=audit requires a morphologyProfile".into(),
                    )
                })?;
            let coverage = super::settlement::service_coverage::audit(
                &index.entities,
                f64::from(profile.service_radius_m),
            )
            .map_err(WorldgenError::InvalidRules)?;
            fs::write(
                scene_dir.join("service-coverage-audit.json"),
                serde_json::to_vec_pretty(&coverage)?,
            )?;
        }
        let morphology_coverage_requested = effective_style
            .get("morphologyCoverage")
            .and_then(serde_json::Value::as_str)
            == Some("audit");
        if morphology_coverage_requested {
            let profile = super::settlement::morphology::resolve(&effective_style)
                .map_err(WorldgenError::InvalidRules)?
                .ok_or_else(|| {
                    WorldgenError::InvalidRules(
                        "morphologyCoverage=audit requires a morphologyProfile".into(),
                    )
                })?;
            let development = index.development_plan.as_ref().ok_or_else(|| {
                WorldgenError::InvalidRules(
                    "morphologyCoverage=audit requires a realized development plan".into(),
                )
            })?;
            let coverage =
                super::settlement::morphology_coverage::audit(development, &profile)
                    .map_err(WorldgenError::InvalidRules)?;
            fs::write(
                scene_dir.join("morphology-coverage-audit.json"),
                serde_json::to_vec_pretty(&coverage)?,
            )?;
        }
        if natural_only::enabled(&effective_style) {
            let audit = natural_only::audit_scene(
                &scene.scene_id,
                &index.entities,
                index.engineered_terrain.operations.len(),
                index.road_network_plan.is_some(),
                index.development_plan.is_some(),
            );
            fs::write(
                scene_dir.join("natural-audit.json"),
                serde_json::to_vec_pretty(&audit)?,
            )?;
            natural_only::require_pass(&audit).map_err(WorldgenError::InvalidRules)?;
            natural_only_reports.push(audit);
        }
        trace.stage(&scene.scene_id,"scene-serialization");
        fs::write(
            scene_dir.join("scene.json"),
            serde_json::to_vec_pretty(&index)?,
        )?;
        crate::preview_stream::write(&index,&revision,&scene_dir).map_err(|e|WorldgenError::InvalidRules(format!("preview projection: {e}")))?;
        scene_paths.push(format!("scenes/{}/scene.json", scene.scene_id));
    }
    if natural_only::enabled(&effective_style) {
        let audit = natural_only::aggregate(natural_only_reports);
        fs::write(
            output.join("natural-audit.json"),
            serde_json::to_vec_pretty(&audit)?,
        )?;
        if audit.status != "pass" {
            return Err(WorldgenError::InvalidRules(format!(
                "natural-only audit failed: {}",
                audit.failures.join("; ")
            )));
        }
    }
    if rules_mode {
        fs::write(
            output.join("rules-audit.json"),
            serde_json::to_vec_pretty(&baked_rules_report)?,
        )?;
    }
    if !placement_reports.is_empty() {
        fs::write(
            output.join("rules-placement.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "format": "glyphweave.rules-placement",
                "version": 1,
                "scenes": placement_reports,
            }))?,
        )?;
    }
    // The entity-stage pad completion, published only when it had something to
    // settle: a world whose planned pads all hold their platforms publishes no new
    // file at all, so every world that never hit the coupling keeps its exact bytes.
    if !pad_completion_reports.is_empty() {
        fs::write(
            output.join("pad-completion.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "format": "glyphweave.pad-completion",
                "version": 1,
                "policy": "pad-top-conflicts-with-entity-platform",
                "maxRounds": pad_completion::MAX_COMPLETION_ROUNDS,
                "maxLiftMm": pad_completion::MAX_LIFT_MM,
                "scenes": pad_completion_reports,
            }))?,
        )?;
    }
    trace.stage("","world-publication");
    write_gemap_anchor(output, manifest, &revision)?;
    let index = WorldIndex {
        format: WORLD_FORMAT.to_owned(),
        version: WORLD_VERSION,
        name: manifest.world.name.clone(),
        seed: manifest.world.seed,
        render_mode: manifest.world.render_mode.clone(),
        revision,
        generator_revision: WORLD_GENERATOR_REVISION.to_owned(),
        patch_chain_hash: manifest.patch_chain_hash.clone(),
        scenes: scene_paths,
        scene_graph: manifest.scene_graph.clone(),
        sidecar: SidecarContract::for_style(&manifest.style)?,
        water_level_m: manifest
            .style
            .get("water")
            .and_then(|water| water.get("levelM"))
            .and_then(serde_json::Value::as_f64)
            .map(|level| level as f32),
        road_geometry_revision: road_geometry_revision_for_style(
            compiled_style
                .as_ref()
                .map(|compiled| &compiled.style)
                .unwrap_or(&manifest.style),
        )
        .map(str::to_owned),
    };
    fs::write(
        output.join("world.json"),
        serde_json::to_vec_pretty(&index)?,
    )?;
    fs::write(
        output.join("sidecar.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "contract": index.sidecar,
            "worldIndex": "world.json",
            "scenes": index.scenes,
        }))?,
    )?;
    fs::write(
        output.join("glyphweave.manifest.json"),
        serde_json::to_vec_pretty(manifest)?,
    )?;
    write_adapter_templates(output)?;
    Ok(index)
}

/// What one scene's entity-stage pad completion did.
struct PadCompletion {
    records: Vec<serde_json::Value>,
    rounds: usize,
    checked: usize,
    refused: usize,
    deepened: usize,
}

/// Hold every planned pad's declared flat top against the surface the whole
/// earthwork produces - the plan's carves AND the rule-engine entities' platforms -
/// and settle what cannot hold.
///
/// The loop is `pad_completion::MAX_COMPLETION_ROUNDS` long, never unbounded: a pad
/// may be deepened once (a second breach is a refusal) and a refused pad leaves the
/// checked set, so every round that changes anything removes a conflict for good. A
/// round that must refuse pads re-derives everything the pad list feeds - the
/// development plan's parcels and buildings, the carves, and the grounded entities'
/// rebase - so nothing downstream reads a plan the world no longer carries.
#[allow(clippy::too_many_arguments)]
fn complete_pad_tops(
    seed: u64,
    scene: &SceneSpec,
    plan: &mut road_network::RoadNetworkPlan,
    entities: &mut Vec<EntityInstance>,
    carves: &mut Vec<TerrainCarve>,
    zones: &[LandUseZone],
    mask: &zoning::ZoningMask,
    water: WaterGeometry,
    development_plan: &mut Option<DevelopmentPlan>,
) -> WorldgenResult<PadCompletion> {
    let mut out = PadCompletion {
        records: Vec::new(),
        rounds: 0,
        checked: 0,
        refused: 0,
        deepened: 0,
    };
    let mut deepened: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for round in 0..pad_completion::MAX_COMPLETION_ROUNDS {
        out.rounds = round + 1;
        // Round 39, the TRIANGLE: a cap-carrying pad the plan keeps must be an
        // entity of this world. Without one there is no carve, no platform and no
        // declaration to publish - and a probe that only composes surfaces can miss
        // it entirely whenever the ground under the pad happens to stand on the
        // declared level already, which is exactly the case where the plan, the
        // development plan, the cut ledger and the conversions would all go on
        // claiming a pad the world does not carry and no audit would see it. The
        // entity stage is asked directly, and a pad with no entity is refused BY
        // NAME through the SAME five-record withdrawal as any other completion
        // refusal (plan, service outcomes, joint receipt, conversions, entity list).
        let mut evicted: Vec<String> = plan
            .roadside_sites
            .iter()
            .filter(|site| site.bounded_cut_mm.is_some())
            .filter(|site| !entities.iter().any(|entity| entity.entity_id == site.id))
            .map(|site| site.id.clone())
            .collect();
        evicted.sort();
        if !evicted.is_empty() {
            for pad_id in &evicted {
                let reason = format!(
                    "pad {pad_id} is a pad of this plan and the entity stage published no entity \
                     for it, so its declared flat top has no platform to stand on; the pad is \
                     refused by name instead of leaving the plan, the development plan and the \
                     cut ledger claiming a parcel the world does not carry"
                );
                road_network::refuse_pad_top_conflict(plan, pad_id, reason.clone());
                entities.retain(|entity| entity.entity_id != *pad_id);
                out.refused += 1;
                out.records.push(serde_json::json!({
                    "padId": pad_id,
                    "outcome": "refused",
                    "reason": reason,
                    "entityStage": "no-entity",
                }));
            }
            // Everything the pad list feeds is re-derived from the plan that is left.
            if let Some(record) = development_plan.as_mut() {
                if let Ok(rebuilt) = development::plan(scene, zones, plan) {
                    *record = rebuilt;
                }
            }
            *carves = plan_terrain_carves(entities);
            for _ in 0..2 {
                rebase_grounded_entities_to_carved_surface(seed, water, entities, carves);
                *carves = plan_terrain_carves(entities);
            }
            continue;
        }
        // THE probe: the surface the bake PUBLISHES over each pad's declared flat
        // top, at the audit's own window and precision - the composition with the
        // bounded-cut platform rule ACTIVE (`PadPlatform::Active`, the same
        // `published_terrain_height_zoned` the chunk encoder calls), so a pad whose
        // own declared platform stands is never reported as buried by the fill that
        // platform overrules. Reading the fill-first composition here was the
        // Round 36 phantom: a pad whose platform already published its declared top
        // was deepened, re-read on the same fill-first surface and refused as
        // unstable, while `bounded_cut::verify` step 5 - the audit that reads the
        // published raster - would have found nothing wrong.
        let conflicts = {
            let published = |x: i32, z: i32| {
                Some(
                    (published_terrain_height_zoned(seed, x, z, water, carves, Some(mask)) * 1000.)
                        .round() as i32,
                )
            };
            let natural = |x: i32, z: i32| {
                Some((terrain_height_metres(seed, x, z, water, &[]) * 1000.).round() as i32)
            };
            pad_completion::probe(plan, entities, &published, &natural)
        };
        out.checked = plan
            .roadside_sites
            .iter()
            .filter(|site| site.bounded_cut_mm.is_some())
            .count();
        if conflicts.is_empty() {
            break;
        }
        // The accepted benches, as the per-cell guard and the footprint exclusion
        // read them: Round 28's one predicate, not a second one. Owned ids, because a
        // refusal below takes the pad back out of the plan.
        let benches: Vec<(String, bounded_cut::CutFootprint)> = plan
            .roadside_sites
            .iter()
            .filter_map(|site| {
                site.cut_footprint()
                    .map(|footprint| (site.id.clone(), footprint))
            })
            .collect();
        let mut changed = false;
        for conflict in conflicts {
            let Some(site) = plan
                .roadside_sites
                .iter()
                .find(|site| site.id == conflict.pad_id)
                .cloned()
            else {
                continue;
            };
            let accepted: Vec<bounded_cut::AcceptedFootprint<'_>> = benches
                .iter()
                .map(|(pad_id, footprint)| bounded_cut::AcceptedFootprint {
                    pad_id: pad_id.as_str(),
                    footprint: *footprint,
                })
                .collect();
            let occupied = |x: i32, z: i32| {
                entities.iter().any(|entity| {
                    entity.entity_id != site.id && bounded_cut::entity_covers(entity, x, z)
                })
            };
            let guards = bounded_cut::CellGuards {
                mask,
                water,
                occupied: &occupied,
                cut_footprints: &accepted,
            };
            let deepened_once = deepened.contains(&conflict.pad_id);
            let decision = pad_completion::decide(&conflict, &site, &guards, deepened_once);
            match &decision {
                pad_completion::Decision::Deepen { .. } => {
                    // The platform rule in `apply_terrain_carves` is what deepens:
                    // inside the pad's declared flat top its own bench stands, and
                    // the next round's probe confirms it from the composition.
                    deepened.insert(conflict.pad_id.clone());
                    out.deepened += 1;
                }
                pad_completion::Decision::Refuse { cause } => {
                    let reason = pad_completion::refusal_reason(&conflict, cause);
                    road_network::refuse_pad_top_conflict(plan, &conflict.pad_id, reason);
                    // The pad's own entity leaves the world with it: the plan and
                    // the entity list may not disagree about which pads exist.
                    entities.retain(|entity| entity.entity_id != conflict.pad_id);
                    out.refused += 1;
                    changed = true;
                }
            }
            out.records
                .push(pad_completion::record(&conflict, &decision));
        }
        if !changed {
            // A repair-only round changes no plan record; the next round's probe is
            // what confirms it, and past the last round `bounded_cut::verify` step 5
            // refuses any ledger that is wrong.
            continue;
        }
        // Everything the pad list feeds is re-derived, never patched by hand.
        if let Some(record) = development_plan.as_mut() {
            if let Ok(rebuilt) = development::plan(scene, zones, plan) {
                *record = rebuilt;
            }
        }
        *carves = plan_terrain_carves(entities);
        for _ in 0..2 {
            rebase_grounded_entities_to_carved_surface(seed, water, entities, carves);
            *carves = plan_terrain_carves(entities);
        }
    }
    Ok(out)
}

fn merge_validation_report(
    total: &mut crate::rules::ValidationReport,
    report: &crate::rules::ValidationReport,
) {
    total.seed = report.seed;
    total.buildings += report.buildings;
    total.roads += report.roads;
    total.floating_items += report.floating_items;
    total.submerged_items += report.submerged_items;
    total.forbidden_biomes += report.forbidden_biomes;
    total.forbidden_hazards += report.forbidden_hazards;
    total.slope_too_high += report.slope_too_high;
    total.out_of_bounds += report.out_of_bounds;
    total.blocked_entrances += report.blocked_entrances;
    total.geometry_collisions += report.geometry_collisions;
    total.disconnected_roads += report.disconnected_roads;
    total.rejects.extend(report.rejects.clone());
    total.checked_items += report.checked_items;
    total.passed_items += report.passed_items;
    total.rejected_items += report.rejected_items;
    total.unruled_items.extend(report.unruled_items.clone());
}

fn write_gemap_anchor(
    output: &Path,
    manifest: &WorldManifest,
    revision: &str,
) -> WorldgenResult<()> {
    let mut world = VoxelWorld::new(&manifest.world.name);
    let anchor = world
        .intern_block("glyphweave:world_anchor")
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    world
        .set(VoxelCoord::new(0, 0, 0), anchor)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let metadata = BTreeMap::from([(
        "world".to_owned(),
        serde_json::json!({
            "revision": revision,
            "sidecar": "sidecar.json",
            "gemapRole": "identity-anchor",
            "terrainAuthority": "sidecar",
        }),
    )]);
    let bytes = encode_world_with_metadata(&world, Some(metadata))
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    fs::write(output.join("world.gemap"), bytes)?;
    Ok(())
}

pub fn world_manifest_revision(manifest: &WorldManifest) -> WorldgenResult<String> {
    // A legacy manifest that cannot use any graded band keeps the historical
    // payload so frozen evidence keeps its revision. Profile-bearing manifests
    // bind the compiled effective programs, and any manifest whose programs can
    // produce a graded road binds the road geometry revision, so a change in
    // the earthwork rule can never silently reuse an old world revision.
    let compiled =
        crate::settlement::morphology_compile::compile(&manifest.style, &manifest.scenes)
            .map_err(WorldgenError::InvalidRules)?;
    let effective = compiled
        .as_ref()
        .map(|compiled| &compiled.style)
        .unwrap_or(&manifest.style);
    let geometry = road_geometry_revision_for_style(effective);
    world_manifest_revision_with_geometry(manifest, compiled.as_ref(), geometry)
}

/// Revision payload/hash with an explicit road geometry revision. Kept separate
/// so a unit test can prove that two geometry revisions produce different
/// identities without touching morphology versions.
pub fn world_manifest_revision_with_geometry(
    manifest: &WorldManifest,
    compiled: Option<&crate::settlement::morphology_compile::CompiledStyle>,
    geometry: Option<&str>,
) -> WorldgenResult<String> {
    let payload = match &compiled {
        Some(compiled) => {
            let mut payload = serde_json::json!({
                "generator": WORLD_GENERATOR_REVISION,
                "manifest": manifest,
                "effectiveProgramsRevision":
                    crate::settlement::morphology_compile::EFFECTIVE_PROGRAMS_REVISION,
                "effectiveStyleHash": compiled.report.effective_style_hash,
                "effectiveProgramSources": compiled.report.field_sources,
            });
            if let Some(geometry) = geometry {
                payload["roadGeometryRevision"] = serde_json::json!(geometry);
            }
            payload
        }
        None => {
            let mut payload = serde_json::json!({
                "generator": WORLD_GENERATOR_REVISION,
                "manifest": manifest,
            });
            if let Some(geometry) = geometry {
                payload["roadGeometryRevision"] = serde_json::json!(geometry);
            }
            payload
        }
    };
    Ok(blake3::hash(&serde_json::to_vec(&payload)?)
        .to_hex()
        .to_string())
}

/// Which road geometry revision the given (effective) style can actually use.
///
/// The graded band rule changed only for roads wider than 12m, so an input is
/// bound to the new revision when it can produce a 16m graded road:
///
/// * `roadExtensions` with an explicit 16m corridor;
/// * an `automaticRoadExtensions` hierarchy with a 16m tier whose purposes
///   intersect that scene's priority purposes;
/// * a morphology profile, whose compiled hierarchy always carries the 16m
///   collector tier.
///
/// Inputs that cannot use the rule (8/12m roads only, or no graded roads at
/// all) keep the historical identity, so frozen legacy evidence is untouched.
pub fn road_geometry_revision_for_style(style: &serde_json::Value) -> Option<&'static str> {
    if style.get("morphologyProfile").is_some() {
        return Some(GRADED_ROAD_GEOMETRY_REVISION);
    }
    if let Some(corridors) = style
        .get("roadExtensions")
        .and_then(|program| program.get("corridors"))
        .and_then(serde_json::Value::as_array)
    {
        if corridors
            .iter()
            .any(|corridor| corridor.get("roadWidthM").and_then(serde_json::Value::as_u64) == Some(16))
        {
            return Some(GRADED_ROAD_GEOMETRY_REVISION);
        }
    }
    if let Some(scenes) = style
        .get("automaticRoadExtensions")
        .and_then(|program| program.get("scenes"))
        .and_then(serde_json::Value::as_array)
    {
        for scene in scenes {
            let Some(tiers) = scene
                .get("hierarchy")
                .and_then(|hierarchy| hierarchy.get("tiers"))
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            let purposes: Vec<&str> = scene
                .get("priorityPurposes")
                .and_then(serde_json::Value::as_array)
                .map(|purposes| {
                    purposes
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect()
                })
                .unwrap_or_default();
            let picks_16m = tiers.iter().any(|tier| {
                tier.get("widthM").and_then(serde_json::Value::as_u64) == Some(16)
                    && tier
                        .get("purposes")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|tier_purposes| {
                            tier_purposes
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .any(|purpose| purposes.contains(&purpose))
                        })
            });
            if picks_16m {
                return Some(GRADED_ROAD_GEOMETRY_REVISION);
            }
        }
    }
    None
}

pub fn apply_patch(manifest: &WorldManifest, patch: &WorldPatch) -> WorldgenResult<WorldManifest> {
    manifest.validate()?;
    if patch.format != "glyphweave-world-patch" || !(patch.version == 1 || patch.version == 2) {
        return Err(WorldgenError::InvalidFormat(patch.format.clone()));
    }
    if patch.patch_id.trim().is_empty() {
        return Err(WorldgenError::InvalidPatch(
            "patch_id must not be empty".into(),
        ));
    }
    let current_revision = world_manifest_revision(manifest)?;
    if !patch.base_revision.is_empty() && patch.base_revision != current_revision {
        return Err(WorldgenError::InvalidPatch(format!(
            "base_revision {} does not match manifest revision {}",
            patch.base_revision, current_revision
        )));
    }
    if let Some(parent) = &patch.parent_patch_hash {
        if parent != &manifest.patch_chain_hash {
            return Err(WorldgenError::InvalidPatch(format!(
                "parent_patch_hash {} does not match manifest patch chain {}",
                parent, manifest.patch_chain_hash
            )));
        }
    }
    let mut result = manifest.clone();
    for operation in &patch.operations {
        match operation {
            PatchOperation::MoveLandmark {
                entity_id,
                world_x,
                world_z,
                world_y,
            } => {
                let landmark = result
                    .landmarks
                    .iter_mut()
                    .find(|item| item.entity_id == *entity_id)
                    .ok_or_else(|| WorldgenError::InvalidLandmark(entity_id.clone()))?;
                landmark.world_x = *world_x;
                landmark.world_z = *world_z;
                landmark.world_y = *world_y;
            }
            PatchOperation::SetLandmark { landmark } => {
                if let Some(existing) = result
                    .landmarks
                    .iter_mut()
                    .find(|item| item.entity_id == landmark.entity_id)
                {
                    *existing = landmark.clone();
                } else {
                    result.landmarks.push(landmark.clone());
                }
            }
            PatchOperation::RemoveLandmark { entity_id } => {
                let before = result.landmarks.len();
                result.landmarks.retain(|item| item.entity_id != *entity_id);
                if result.landmarks.len() == before {
                    return Err(WorldgenError::InvalidLandmark(entity_id.clone()));
                }
            }
        }
    }
    let chain_payload = serde_json::json!({
        "parent": manifest.patch_chain_hash,
        "patch": patch,
        "sequence": patch.sequence,
    });
    result.patch_chain_hash = blake3::hash(&serde_json::to_vec(&chain_payload)?)
        .to_hex()
        .to_string();
    result.validate()?;
    Ok(result)
}

/// Refresh only HTML adapter modules, never authoritative generation files.
pub fn refresh_html_adapter(output:&Path)->WorldgenResult<()> {
    let preview=output.join("preview");fs::create_dir_all(&preview)?;
    fs::write(preview.join("index.html"),include_str!("../../../../adapters/html/index.html"))?;
    fs::write(preview.join("app.js"),include_str!("../../../../adapters/html/app.js"))?;
    write_html_preview_modules(&preview)?;write_preview_assets(output)?;Ok(())
}

fn write_adapter_templates(output: &Path) -> WorldgenResult<()> {
    let preview = output.join("preview");
    let godot = output.join("godot");
    fs::create_dir_all(&preview)?;
    fs::create_dir_all(&godot)?;
    fs::write(
        preview.join("index.html"),
        include_str!("../../../../adapters/html/index.html"),
    )?;
    fs::write(
        preview.join("app.js"),
        include_str!("../../../../adapters/html/app.js"),
    )?;
    write_html_preview_modules(&preview)?;
    fs::write(
        godot.join("project.godot"),
        include_str!("../../../../adapters/godot/project.godot"),
    )?;
    fs::write(
        godot.join("main.tscn"),
        include_str!("../../../../adapters/godot/main.tscn"),
    )?;
    fs::write(
        godot.join("main.gd"),
        include_str!("../../../../adapters/godot/main.gd"),
    )?;
    write_preview_assets(output)?;
    Ok(())
}

fn write_html_preview_modules(preview: &Path) -> WorldgenResult<()> {
    // The generated preview is a standalone static adapter. Copy its complete
    // ES-module dependency tree rather than only app.js; otherwise browsers
    // load the shell but fail before rendering on missing ./core and ./render
    // imports. Three.js is vendored so the exported world works offline.
    let files: &[(&str, &[u8])] = &[
        (
            "core/height.js",
            include_bytes!("../../../../adapters/html/core/height.js"),
        ),
        (
            "core/camera.js",
            include_bytes!("../../../../adapters/html/core/camera.js"),
        ),
        (
            "core/shared.js",
            include_bytes!("../../../../adapters/html/core/shared.js"),
        ),
        (
            "core/state.js",
            include_bytes!("../../../../adapters/html/core/state.js"),
        ),
        (
            "core/three.js",
            include_bytes!("../../../../adapters/html/core/three.js"),
        ),
        (
            "core/render-stream.js",
            include_bytes!("../../../../adapters/html/core/render-stream.js"),
        ),
        (
            "core/world.js",
            include_bytes!("../../../../adapters/html/core/world.js"),
        ),
        (
            "render/road-profile.js",
            include_bytes!("../../../../adapters/html/render/road-profile.js"),
        ),
        (
            "render/ecology.js",
            include_bytes!("../../../../adapters/html/render/ecology.js"),
        ),
        (
            "render/buildings.js",
            include_bytes!("../../../../adapters/html/render/buildings.js"),
        ),
        (
            "render/grass.js",
            include_bytes!("../../../../adapters/html/render/grass.js"),
        ),
        (
            "render/postfx.js",
            include_bytes!("../../../../adapters/html/render/postfx.js"),
        ),
        (
            "render/props.js",
            include_bytes!("../../../../adapters/html/render/props.js"),
        ),
        (
            "render/sky.js",
            include_bytes!("../../../../adapters/html/render/sky.js"),
        ),
        (
            "render/terrain.js",
            include_bytes!("../../../../adapters/html/render/terrain.js"),
        ),
        (
            "render/vegetation.js",
            include_bytes!("../../../../adapters/html/render/vegetation.js"),
        ),
        (
            "render/wind.js",
            include_bytes!("../../../../adapters/html/render/wind.js"),
        ),
        (
            "render/water.js",
            include_bytes!("../../../../adapters/html/render/water.js"),
        ),
        (
            "render/presets/materials.js",
            include_bytes!("../../../../adapters/html/render/presets/materials.js"),
        ),
        (
            "vendor/three/build/three.core.js",
            include_bytes!("../../../../adapters/html/vendor/three/build/three.core.js"),
        ),
        (
            "vendor/three/LICENSE",
            include_bytes!("../../../../adapters/html/vendor/three/LICENSE"),
        ),
        (
            "vendor/three/build/three.module.js",
            include_bytes!("../../../../adapters/html/vendor/three/build/three.module.js"),
        ),
        (
            "vendor/three/examples/jsm/controls/OrbitControls.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/controls/OrbitControls.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/loaders/GLTFLoader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/loaders/GLTFLoader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/loaders/OBJLoader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/loaders/OBJLoader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/BokehPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/BokehPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/EffectComposer.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/EffectComposer.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/MaskPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/MaskPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/OutputPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/OutputPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/Pass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/Pass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/RenderPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/RenderPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/ShaderPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/ShaderPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/postprocessing/UnrealBloomPass.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/postprocessing/UnrealBloomPass.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/shaders/BokehShader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/shaders/BokehShader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/shaders/CopyShader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/shaders/CopyShader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/shaders/LuminosityHighPassShader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/shaders/LuminosityHighPassShader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/shaders/OutputShader.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/shaders/OutputShader.js"
            ),
        ),
        (
            "vendor/three/examples/jsm/utils/BufferGeometryUtils.js",
            include_bytes!(
                "../../../../adapters/html/vendor/three/examples/jsm/utils/BufferGeometryUtils.js"
            ),
        ),
    ];
    for (relative, data) in files {
        let path = preview.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)?;
    }
    Ok(())
}

fn write_preview_assets(output: &Path) -> WorldgenResult<()> {
    let assets = output.join("assets");
    fs::create_dir_all(&assets)?;
    let files: &[(&str, &[u8])] = &[
        (
            "CommonTree_1.gltf",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/CommonTree_1.gltf"
            ),
        ),
        (
            "CommonTree_1.bin",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/CommonTree_1.bin"
            ),
        ),
        (
            "Bark_NormalTree_Normal.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Bark_NormalTree_Normal.png"
            ),
        ),
        (
            "Bark_NormalTree.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Bark_NormalTree.png"
            ),
        ),
        (
            "Leaves_NormalTree_C.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Leaves_NormalTree_C.png"
            ),
        ),
        (
            "Leaves_NormalTree.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Leaves_NormalTree.png"
            ),
        ),
        (
            "Bush_Common.gltf",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Bush_Common.gltf"
            ),
        ),
        (
            "Bush_Common.bin",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Bush_Common.bin"
            ),
        ),
        (
            "Leaves_TwistedTree_C.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Leaves_TwistedTree_C.png"
            ),
        ),
        (
            "Pebble_Round_1.gltf",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Pebble_Round_1.gltf"
            ),
        ),
        (
            "Pebble_Round_1.bin",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/Pebble_Round_1.bin"
            ),
        ),
        (
            "PathRocks_Diffuse.png",
            include_bytes!(
                "../../../../assets/third_party/quaternius/stylized-nature/PathRocks_Diffuse.png"
            ),
        ),
        (
            "2Story_GableRoof.obj",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/2Story_GableRoof.obj"
            ),
        ),
        (
            "2Story_GableRoof.mtl",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/2Story_GableRoof.mtl"
            ),
        ),
        (
            "1Story_GableRoof.obj",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/1Story_GableRoof.obj"
            ),
        ),
        (
            "1Story_GableRoof.mtl",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/1Story_GableRoof.mtl"
            ),
        ),
        (
            "2Story_Wide.obj",
            include_bytes!("../../../../assets/third_party/quaternius/buildings/2Story_Wide.obj"),
        ),
        (
            "2Story_Wide.mtl",
            include_bytes!("../../../../assets/third_party/quaternius/buildings/2Story_Wide.mtl"),
        ),
        (
            "2Story_RoundRoof.obj",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/2Story_RoundRoof.obj"
            ),
        ),
        (
            "2Story_RoundRoof.mtl",
            include_bytes!(
                "../../../../assets/third_party/quaternius/buildings/2Story_RoundRoof.mtl"
            ),
        ),
    ];
    for (name, data) in files {
        fs::write(assets.join(name), data)?;
    }
    fs::write(
        assets.join("LICENSE.txt"),
        b"GlyphWeave preview asset notices\n\nThis bundle includes third-party assets.\nSee THIRD_PARTY_NOTICES.md and licenses/ for the component notices.\n",
    )?;
    fs::create_dir_all(assets.join("licenses"))?;
    fs::write(
        assets.join("licenses").join("quaternius-stylized-nature-CC0.txt"),
        include_bytes!(
            "../../../../assets/third_party/quaternius/stylized-nature/License_Standard.txt"
        ),
    )?;
    fs::write(
        assets
            .join("licenses")
            .join("quaternius-buildings-NOTICE.txt"),
        include_bytes!("../../../../assets/third_party/quaternius/buildings/NOTICE.txt"),
    )?;
    fs::write(
        assets.join("THIRD_PARTY_NOTICES.md"),
        include_bytes!("../../../../THIRD_PARTY_NOTICES.md"),
    )?;
    // Rewrite source-relative registry paths to the self-contained published
    // preview asset bundle. The generated preview must not point back into the
    // repository checkout that produced it.
    let mut registry: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../assets/glyphweave.registry.json"
    ))?;
    if let Some(entries) = registry
        .get_mut("assets")
        .and_then(serde_json::Value::as_object_mut)
    {
        for entry in entries.values_mut() {
            if let Some(path) = entry.get("path").and_then(serde_json::Value::as_str) {
                let file = path.rsplit('/').next().unwrap_or(path);
                entry["path"] = serde_json::json!(format!("assets/{file}"));
            }
            if let Some(path) = entry.get("material").and_then(serde_json::Value::as_str) {
                let file = path.rsplit('/').next().unwrap_or(path);
                entry["material"] = serde_json::json!(format!("assets/{file}"));
            }
        }
    }
    registry["publishedRoot"] = serde_json::json!("assets/");
    fs::write(
        assets.join("glyphweave.registry.json"),
        serde_json::to_vec_pretty(&registry)?,
    )?;
    Ok(())
}

pub fn write_demo_manifest(path: &Path) -> WorldgenResult<()> {
    fs::write(
        path,
        serde_json::to_vec_pretty(&WorldManifest::default_demo())?,
    )?;
    Ok(())
}

#[cfg(test)]
fn generate_chunk(
    seed: u64,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
    water: WaterKind,
    scene_width: u32,
    scene_depth: u32,
    river_half_m: f64,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let geometry = legacy_water_geometry(water, scene_width, scene_depth, river_half_m);
    let (height, surface, lod2, _) =
        generate_chunk_with_geometry(seed, base_x, base_z, width, depth, geometry, &[]);
    (height, surface, lod2)
}

/// Include the one-metre finite-difference halo used by terrain semantics.
/// Filtering only the chunk interior changes edge normals/curvature/wetness
/// even when every encoded height sample is identical.
fn carve_intersects_chunk(
    carve: &TerrainCarve,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
) -> bool {
    let min_x = f64::from(base_x) - 1.;
    let max_x = f64::from(base_x) + f64::from(width);
    let min_z = f64::from(base_z) - 1.;
    let max_z = f64::from(base_z) + f64::from(depth);
    carve.cx + carve.half_w + carve.blend_m >= min_x
        && carve.cx - carve.half_w - carve.blend_m <= max_x
        && carve.cz + carve.half_d + carve.blend_m >= min_z
        && carve.cz - carve.half_d - carve.blend_m <= max_z
}

#[cfg(test)]
fn generate_chunk_with_geometry(
    seed: u64,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    generate_chunk_encoded(
        seed, base_x, base_z, width, depth, water, carves, false, None,
    )
}
fn generate_chunk_encoded(
    seed: u64,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    precise: bool,
    zoning: Option<&zoning::ZoningMask>,
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let cell_count = (width * depth) as usize;
    let mut height = Vec::with_capacity(cell_count * 2);
    let mut surface = Vec::with_capacity(cell_count);
    let mut terrain = Vec::with_capacity(cell_count * TERRAIN_SEMANTIC_BYTES_PER_CELL);
    let mut lod2 = Vec::with_capacity((width.div_ceil(64) * depth.div_ceil(64)) as usize * 3);
    let mut samples = vec![0_f64; cell_count];
    // ONE index for the whole chunk: the per-cell walk below visits only the
    // carves whose own window can reach that cell, and answers exactly what the
    // linear walk answered.
    let carve_index = CarveIndex::new(carves);
    for z in 0..depth {
        for x in 0..width {
            let world_x = base_x + x as i32;
            let world_z = base_z + z as i32;
            let source = published_terrain_height_zoned_indexed(
                seed,
                world_x,
                world_z,
                water,
                carves,
                &carve_index,
                zoning,
            );
            let quarter_meters = (source * 4.).clamp(-128., 1024.) as i16;
            let index = (z * width + x) as usize;
            samples[index] = if precise {
                (source * 1000.).round() / 1000.
            } else {
                f64::from(quarter_meters) / 4.
            };
            if !precise {
                height.extend_from_slice(&quarter_meters.to_le_bytes());
            }
            surface.push(surface_kind_with_geometry(
                seed,
                world_x,
                world_z,
                quarter_meters,
                water,
            ));
        }
    }
    for z in 0..depth {
        for x in 0..width {
            terrain.extend_from_slice(&terrain_semantics_at(
                seed,
                base_x,
                base_z,
                width,
                depth,
                x,
                z,
                &samples,
                water,
                carves,
                &carve_index,
                precise,
                zoning,
            ));
        }
    }
    debug_assert_eq!(terrain.len(), cell_count * TERRAIN_SEMANTIC_BYTES_PER_CELL);
    for block_z in (0..depth).step_by(64) {
        for block_x in (0..width).step_by(64) {
            let mut total = 0_i32;
            let mut count = 0_i32;
            for z in block_z..(block_z + 64).min(depth) {
                for x in block_x..(block_x + 64).min(width) {
                    total += (samples[(z * width + x) as usize] * 4.) as i32;
                    count += 1;
                }
            }
            let average = (total / count) as i16;
            lod2.extend_from_slice(&average.to_le_bytes());
            lod2.push(surface_kind_with_geometry(
                seed,
                base_x + block_x as i32,
                base_z + block_z as i32,
                average,
                water,
            ));
        }
    }
    if precise {
        height = encode_millimetres(width, depth, samples)
            .expect("bounded metre source produces valid millimetre payload");
    }
    (height, surface, lod2, terrain)
}

/// Derive shared render/analysis attributes from the exact baked terrain.
///
/// Height lives in the height payload and land-cover lives in the surface
/// payload. This compact four-byte payload carries values that must be
/// consistent across renderers, vegetation placement and later terrain-aware
/// rules, without letting each consumer generate unrelated visual noise.
#[allow(clippy::too_many_arguments)]
fn terrain_semantics_at(
    seed: u64,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
    x: u32,
    z: u32,
    samples: &[f64],
    water: WaterGeometry,
    carves: &[TerrainCarve],
    carve_index: &CarveIndex,
    precise: bool,
    zoning: Option<&zoning::ZoningMask>,
) -> [u8; TERRAIN_SEMANTIC_BYTES_PER_CELL] {
    let sample = |sx: i32, sz: i32| -> f64 {
        if sx >= 0 && sx < width as i32 && sz >= 0 && sz < depth as i32 {
            samples[(sz as u32 * width + sx as u32) as usize]
        } else {
            let h = published_terrain_height_zoned_indexed(
                seed,
                base_x + sx,
                base_z + sz,
                water,
                carves,
                carve_index,
                zoning,
            );
            if precise {
                (h * 1000.).round() / 1000.
            } else {
                (h * 4.).clamp(-128., 1024.) as i16 as f64 / 4.
            }
        }
    };
    let x = x as i32;
    let z = z as i32;
    let centre = sample(x, z);
    let west = sample(x - 1, z);
    let east = sample(x + 1, z);
    let north = sample(x, z - 1);
    let south = sample(x, z + 1);

    // Central differences over two metres, encoded in degrees. This is the
    // baked slope, so a renderer never has to infer it from a coarser LOD.
    let gradient_x = (east - west) * 0.5;
    let gradient_z = (south - north) * 0.5;
    let slope_deg = gradient_x
        .hypot(gradient_z)
        .atan()
        .to_degrees()
        .clamp(0.0, 90.0);
    let slope_u8 = (slope_deg / 90.0 * 255.0).round() as u8;

    // Laplacian-like local curvature in metres. Valleys are positive and
    // ridges negative; clamp deliberately because this field is a material /
    // placement signal, not a replacement heightmap.
    let curvature_m = (west + east + north + south - 4.0 * centre) * 0.25;
    let curvature_u8 = (((curvature_m.clamp(-2.0, 2.0) + 2.0) / 4.0) * 255.0).round() as u8;

    let world_x = base_x + x;
    let world_z = base_z + z;
    let wetness = (humidity_field(seed, world_x, world_z, water) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let disturbance = if zoning.is_some_and(|m| m.protected_at(world_x, world_z)) {
        0
    } else {
        terrain_disturbance_at(world_x, world_z, carves)
    };
    [slope_u8, curvature_u8, wetness, disturbance]
}

/// Return the dominant human terrain intervention at a baked cell. Terrain
/// pads are already authoritative for height; this is only the semantic mask
/// that lets adapters vary cover/roughness without independently guessing
/// where roads and lots have altered the ground.
fn terrain_disturbance_at(x: i32, z: i32, carves: &[TerrainCarve]) -> u8 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    carves
        .iter()
        .filter_map(|carve| {
            let outside =
                ((xf - carve.cx).abs() - carve.half_w).max((zf - carve.cz).abs() - carve.half_d);
            (outside <= 0.0).then_some(carve.disturbance)
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
fn river_half_width_m(landmarks: &[LandmarkSpec], scene_width_m: u32) -> f64 {
    landmarks
        .iter()
        .find(|landmark| landmark.entity_type == "river")
        .map(|landmark| f64::from(landmark.width_m) * 0.5)
        .unwrap_or_else(|| f64::from(scene_width_m) * 0.14)
}

fn river_half_width_at(z: f64, scene_depth_m: u32, base_half_width_m: f64) -> f64 {
    if scene_depth_m < 3_000 {
        return base_half_width_m.max(1.0);
    }
    let t = (z / f64::from(scene_depth_m)).clamp(0.0, 1.0);
    let broad_bend = (t * std::f64::consts::TAU * 1.15).sin() * 0.08;
    let harbour_bay = ((t - 0.28) * std::f64::consts::TAU * 3.0).sin().max(0.0) * 0.045;
    (base_half_width_m * (1.0 + broad_bend + harbour_bay)).max(1.0)
}

/// Smooth value noise in `[-1, 1]`. Uses a lattice of hashed corner values
/// with bilinear interpolation, so the field is continuous across chunk
/// boundaries and depends only on world coordinates.
fn value_noise2d(seed: u64, x: f64, z: f64) -> f64 {
    let xi = x.floor();
    let zi = z.floor();
    let xf = x - xi;
    let zf = z - zi;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = zf * zf * (3.0 - 2.0 * zf);
    let lattice = |cx: i64, cz: i64| -> f64 {
        let mut h = seed
            .wrapping_add(cx as u64)
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(cz as u64)
            .wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
        h ^= h >> 30;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 27;
        h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        (h as i64 % 100_003) as f64 / 50_001.5 - 1.0
    };
    let x0 = xi as i64;
    let z0 = zi as i64;
    let a = lattice(x0, z0);
    let b = lattice(x0 + 1, z0);
    let c = lattice(x0, z0 + 1);
    let d = lattice(x0 + 1, z0 + 1);
    let top = a + (b - a) * u;
    let bottom = c + (d - c) * u;
    top + (bottom - top) * v
}

/// Fractal value noise (3 octaves) for stable, chunk-continuous landform
/// detail. Amplitudes are kept low so adjacent 1m cells never step more than
/// a couple of metres, which is what keeps water shores and chunk seams
/// continuous.
fn fbm2d(seed: u64, x: f64, z: f64) -> f64 {
    let base = value_noise2d(seed.wrapping_add(0x517c_c1b7), x / 520.0, z / 520.0);
    let detail = value_noise2d(seed.wrapping_add(0x33ab_1103), x / 150.0, z / 150.0) * 0.35;
    let fine = value_noise2d(seed.wrapping_add(0x2209_1f11), x / 64.0, z / 64.0) * 0.12;
    base + detail + fine
}

/// Mid-scale relief that makes slopes, swells and shallow draws legible at
/// walking and district scale (roughly 80–360m) without adding fake renderer
/// displacement. The field is deterministic in world space, chunk-continuous,
/// and becomes part of the baked height contract used by roads and buildings.
fn mesoscale_relief(seed: u64, x: i32, z: i32, water: WaterGeometry) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    // A low-frequency domain warp breaks the regularity of plain value noise.
    let warp_x = value_noise2d(seed.wrapping_add(0x243f_6a88), xf / 360.0, zf / 360.0) * 85.0;
    let warp_z = value_noise2d(seed.wrapping_add(0x85a3_08d3), xf / 360.0, zf / 360.0) * 85.0;
    let swell = value_noise2d(
        seed.wrapping_add(0x1319_8a2e),
        (xf + warp_x) / 310.0,
        (zf + warp_z) / 310.0,
    );
    // Fold a second field into soft shoulders and shallow draws. It gives a
    // traveller something to read between kilometre-scale landforms and the
    // 64m detail ripple, instead of a single featureless colour ramp.
    let fold = 1.0
        - value_noise2d(
            seed.wrapping_add(0x0370_7344),
            (xf - warp_z * 0.45) / 145.0,
            (zf + warp_x * 0.45) / 145.0,
        )
        .abs();
    let signed_fold = (fold - 0.5) * 2.0;
    let amplitude = if water.smooth_rolling { 6.5 } else { 9.0 };
    (swell * 0.68 + signed_fold * 0.32) * amplitude
}

/// Approximate valley-shaped noise carving, NOT a water/sediment simulation.
///
/// This legacy pointwise function has no water or sediment transport state.
/// It carves near warped-noise zero contours, not measured drainage paths.
/// A deterministic staged simulation can replace it later; simulation does
/// not inherently prevent reproducibility or chunked output.
///
/// It is masked to zero inside settled land so cities and their surroundings
/// stay flat — erosion only reshapes the wild valley / hill ground.
fn approximate_valley_carve(seed: u64, x: i32, z: i32, water: WaterGeometry) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    // Domain-warped field: warping makes the zero lines meander like rivers
    // instead of straight grid lines.
    let warp_x = fbm2d(seed.wrapping_add(0x9e37_79b9), xf / 240.0, zf / 240.0) * 60.0;
    let warp_z = fbm2d(seed.wrapping_add(0xbf58_476d), xf / 240.0, zf / 240.0) * 60.0;
    // Two river scales: major channels (broad valleys) + minor streams.
    let major = value_noise2d(
        seed.wrapping_add(0x6a09_e667),
        (xf + warp_x) / 640.0,
        (zf + warp_z) / 640.0,
    );
    let minor = value_noise2d(
        seed.wrapping_add(0xbb67_ae85),
        (xf + warp_x * 1.4) / 190.0,
        (zf + warp_z * 1.4) / 190.0,
    );
    // Distance to the nearest channel (field near zero) - closer = stronger flow.
    let d_major = (major.abs() * 640.0).min(90.0) / 90.0;
    let d_minor = (minor.abs() * 190.0).min(46.0) / 46.0;
    let near_channel = (1.0 - d_major) * 0.62 + (1.0 - d_minor) * 0.38;
    // Flow strength: 0 on ridges, 1 in valleys where drainage concentrates.
    let landform = landform_field(seed, x, z, water);
    let low_ground = (1.0 - (landform / 90.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    // Settled land stays untouched: cities sit on flat, erosion-free pads.
    let urban = urbanization_field(seed, x, z, water, 0.5);
    let wild = (1.0 - urban * 1.8).clamp(0.0, 1.0);
    let strength = near_channel * low_ground * wild;
    // Smoothstep so channel banks ease out instead of a hard V cut.
    let strength_s = strength * strength * (3.0 - 2.0 * strength);
    let geography = water.geography;
    let incision = if geography.authored {
        30.0 * (0.5 + f64::from(geography.incised_valleys))
    } else {
        30.0
    };
    let relict = if geography.authored && geography.historic_channel_shift_m > 0.0 {
        let shift = f64::from(geography.historic_channel_shift_m);
        let historical = value_noise2d(
            seed.wrapping_add(0x9e37_79b9),
            (xf + shift + warp_x * 0.8) / 760.0,
            (zf - shift * 0.35 + warp_z * 0.8) / 760.0,
        );
        let d_historical = (historical.abs() * 760.0).min(120.0) / 120.0;
        let floodplain = (1.0 - d_historical).powi(2)
            * (shift / 1000.0).clamp(0.0, 0.75)
            * (1.0 - urban * 1.8).clamp(0.0, 1.0);
        -(floodplain * 9.0)
    } else {
        0.0
    };
    -(strength_s * incision) + relict
}

/// Deterministic terrain skeleton (peak / valley structure) in world space.
///
/// A plain fractal smear gives no structure; a Worley peak field is
/// discontinuous at Voronoi edges. The robust choice for a pure function is
/// ridge noise: `|fbm|` inverted so low fbm = ridge (high ground) and high
/// fbm = basin (low ground), producing continuous, branching ridge-and-valley
/// structure like real fold mountains. It is chunk-continuous because it only
/// depends on world coordinates and the seed.
fn mountain_skeleton(seed: u64, x: i32, z: i32) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    // Broad mountain-scale folds.
    let base = fbm2d(seed.wrapping_add(0x3c6e_f372), xf / 480.0, zf / 480.0);
    let mid = fbm2d(seed.wrapping_add(0xa54f_f53a), xf / 200.0, zf / 200.0);
    // Ridge: |fbm| so folds create sharp ridge lines; invert so low = peak.
    let ridge = (1.0 - base.abs()) * 0.7 + (1.0 - mid.abs()) * 0.3;
    ridge.clamp(0.0, 1.0)
}

/// Steppe rolling hills (wgen "Hills" style): a deterministic field of stacked
/// hemispherical paraboloids. Each hill contributes height*(1 - dist²/r²)
/// inside its radius, so adjacent bumps blend into smooth rounded terrain —
/// a grassy plain with gentle swells instead of sharp ridges.
fn steppe_hills_field(seed: u64, xf: f64, zf: f64) -> f64 {
    const CELL: f64 = 240.0;
    const NEIGHBORS: [(i64, i64); 9] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (0, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let cx = (xf / CELL).floor();
    let cz = (zf / CELL).floor();
    let mut h = 0.0;
    for (ox, oz) in NEIGHBORS {
        let gx = cx + ox as f64;
        let gz = cz + oz as f64;
        // Hill centre with strong random offset so hills are irregularly
        // scattered, not a neat grid (wgen scatters them randomly).
        let rx = value_noise2d(seed, gx * 0.31, gz * 0.23);
        let rz = value_noise2d(seed.wrapping_add(0x517c_c1b7), gx * 0.19, gz * 0.11);
        let hx = (gx + rx * 1.4) * CELL;
        let hz = (gz + rz * 1.4) * CELL;
        let rh = value_noise2d(seed.wrapping_add(0x6a09_e667), gx * 0.41, gz * 0.37);
        let radius = 120.0 + rh * 110.0;
        let hv = value_noise2d(seed.wrapping_add(0xbb67_ae85), gx * 0.53, gz * 0.47);
        let height = 6.0 + hv * 12.0;
        let dx = xf - hx;
        let dz = zf - hz;
        let d2 = dx * dx + dz * dz;
        let r2 = radius * radius;
        if d2 < r2 {
            // Smooth rounded hill (paraboloid crown): quadratic falloff keeps
            // the crest round and eases to zero at the edge.
            let t = 1.0 - d2 / r2;
            h += height * t * t;
        }
    }
    h - 2.0 // start below datum so the plain can dip to shallow low spots
}

/// Continuous landform field in world space. Returns a scalar that rises
/// smoothly from valley floors through plains and hills into mountains, so
/// the terrain elevation is a continuous function with no banding artifacts
/// at landform boundaries.
fn landform_field(seed: u64, x: i32, z: i32, water: WaterGeometry) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    let width = f64::from(water.scene_width_m.max(1));
    let depth = f64::from(water.scene_depth_m.max(1));
    let cx = f64::from(water.terrain_origin_x) + width * 0.5;
    let cz = f64::from(water.terrain_origin_z) + depth * 0.5;
    // Radial skirt: normalised distance from the scene centre, but the term is
    // an even function that reaches its max at the scene edge and does NOT
    // grow outside it (terrain_height is sampled past the edge by continuity
    // checks, and an unbounded quadratic there would create fake cliffs).
    let scale = width.max(depth) * 0.62;
    let dx = ((xf - cx) / scale).abs().min(1.0);
    let dz = ((zf - cz) / scale).abs().min(1.0);
    let radial = (dx * dx + dz * dz).sqrt();
    let base = if water.smooth_rolling {
        // Steppe / prairie: soft rolling hills (wgen "Hills" generator).
        // Random hemispherical bumps (paraboloids) are stacked, each adding
        // height*(1 - dist²/radius²) inside its footprint, so the terrain is a
        // smooth field of rounded swells — exactly wgen's approach. No sharp
        // ridges, low amplitude so it stays a grassy plain.
        steppe_hills_field(seed, xf, zf) + radial * radial * 20.0
    } else {
        // Terrain skeleton: mountain peaks are distinct rounded high points from
        // the Worley distance field, then a fractal disturbance adds ridges and
        // valleys. A coastal skirt pulls the outer edge down toward the water so
        // land rises from the shore instead of starting mid-plateau.
        let skeleton = mountain_skeleton(seed, x, z);
        let ridges = fbm2d(seed, xf, zf) * 0.5
            + fbm2d(seed.wrapping_add(0x33ab_1103), xf * 2.3, zf * 2.3) * 0.25;
        // 0 at a peak core, rising to ~1 far from peaks; blend skeleton with the
        // fractal so both structure and texture are present.
        let rugged = skeleton * 0.72 + (1.0 - skeleton) * (0.5 + ridges) * 0.28;
        rugged.mul_add(60.0, -16.0) + radial * radial * 90.0
    };
    let geography = water.geography;
    if !geography.authored {
        return base;
    }
    let theta = f64::from(geography.orogeny_axis_rad);
    let across = (xf - cx) * theta.cos() + (zf - cz) * theta.sin();
    let axis_band = (-(across / (scale * 0.42)).powi(2)).exp();
    let uplift = axis_band * (12.0 + f64::from(geography.uplift_bias) * 42.0);
    let glacial_trough = f64::from(geography.glacial)
        * (-(across / (scale * 0.24)).powi(2)).exp()
        * 13.0;
    let karst = if geography.karst > 0.0 {
        let sink = value_noise2d(
            seed.wrapping_add(0x6a09_e667),
            xf / 135.0,
            zf / 135.0,
        )
        .abs();
        f64::from(geography.karst) * (1.0 - sink).powi(2) * 7.0
    } else {
        0.0
    };
    let dune = if geography.aeolian > 0.0 {
        let wind = f64::from(geography.wind_rad);
        let along = xf * wind.cos() + zf * wind.sin();
        f64::from(geography.aeolian)
            * ((along / 145.0).sin() * 1.8
                + value_noise2d(seed.wrapping_add(0xbb67_ae85), xf / 260.0, zf / 260.0) * 2.2)
    } else {
        0.0
    };
    (base * f64::from(geography.relief_scale) + uplift + radial * radial
        * f64::from(geography.coastality)
        * 26.0
        - glacial_trough
        - karst
        + dune)
        .clamp(-32.0, 512.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Landform {
    Valley,
    Plain,
    Hill,
    Mountain,
}

impl Landform {
    fn classify(seed: u64, x: i32, z: i32, water: WaterGeometry) -> Landform {
        let elevation = landform_field(seed, x, z, water);
        if elevation < -10.0 {
            Landform::Valley
        } else if elevation < 52.0 {
            Landform::Plain
        } else if elevation < 82.0 {
            Landform::Hill
        } else {
            Landform::Mountain
        }
    }
}

/// Global urbanisation field. Unlike per-chunk local density, this is a
/// continuous world-space scalar (0 = wild, 1 = dense city core) derived from
/// radial distance to the scene centre plus a fractal disturbance. Every chunk
/// samples the same field, so a mainland reads as "central city fading to
/// wild edges" instead of every chunk doing its own mini city.
fn urbanization_field(seed: u64, x: i32, z: i32, water: WaterGeometry, urban_scale: f64) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    let width = f64::from(water.scene_width_m.max(1));
    let depth = f64::from(water.scene_depth_m.max(1));
    let cx = f64::from(water.terrain_origin_x) + width * 0.5;
    let cz = f64::from(water.terrain_origin_z) + depth * 0.5;
    // City radius is primarily scene-driven so every profile keeps a
    // comparable urban heart; the urban+suburban share only nudges the
    // envelope. Density is expressed by `core_density`/`suburban_density`
    // inside each region, not by shrinking the whole city.
    let pressure = f64::from(water.geography.land_pressure).clamp(0.15, 1.8);
    let envelope = (0.75 + (urban_scale * 0.25)) * (0.82 + pressure * 0.18);
    let city_radius = width.max(depth) * 0.5 * envelope;
    let dist = ((xf - cx).powi(2) + (zf - cz).powi(2)).sqrt();
    let t = (dist / city_radius).clamp(0.0, 1.0);
    // Gentler quadratic falloff: a wide urban heart, then a long suburban /
    // rural skirt out to the wild rim.
    let base = 1.0 - t;
    let smooth = base * base;
    // Organic boundary so the city edge is not a perfect circle. The noise is
    // kept small and blended as a fraction of the smooth value so it cannot
    // push a mid-field point to zero.
    let noise = (fbm2d(seed, xf, zf) * 0.35).clamp(-1.0, 1.0);
    let urban = smooth * (1.0 + noise * 0.25) * (0.88 + pressure * 0.12);
    urban.clamp(0.0, 1.0)
}

/// Land-use region at a world position. Combines the global urbanisation
/// field with landform so regions are coherent across chunk seams and match
/// the terrain they sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RegionType {
    UrbanCore,
    Urban,
    Suburban,
    Rural,
    Forest,
    Mountain,
}

impl RegionType {
    fn classify(seed: u64, x: i32, z: i32, water: WaterGeometry, urban_scale: f64) -> RegionType {
        let urban = urbanization_field(seed, x, z, water, urban_scale);
        let landform = Landform::classify(seed, x, z, water);
        // Thresholds are profile-aware. The previous fixed 0.72/0.50/0.28
        // bands made a 6km x 10km mainland overwhelmingly rural even when
        // the manifest requested a city. Keep a compact core, but widen the
        // urban and suburban bands according to the requested urban target.
        let urban_target = urban_scale.clamp(0.05, 0.9);
        let core_threshold = (0.66 - urban_target * 0.20).clamp(0.48, 0.66);
        let urban_threshold = (0.42 - urban_target * 0.35).clamp(0.18, 0.42);
        let suburban_threshold = (0.20 - urban_target * 0.25).clamp(0.05, 0.20);
        if urban > core_threshold {
            RegionType::UrbanCore
        } else if urban > urban_threshold {
            RegionType::Urban
        } else if urban > suburban_threshold {
            RegionType::Suburban
        } else {
            match landform {
                Landform::Valley | Landform::Plain => RegionType::Rural,
                Landform::Hill => RegionType::Forest,
                Landform::Mountain => RegionType::Mountain,
            }
        }
    }
}

/// Deterministic footprint-occupancy grid used to keep generated land-use
/// entities from overlapping. Roads, buildings, parcels and vegetation all
/// check a coarse grid cell before placement, so a tree never spawns on a
/// road, a building never cuts into a hillside, and greenspace never covers a
/// house — without per-kind hardcoded rules.
///
/// Cell size is a fixed 10m; each cell tracks a small bitmask of what has been
/// placed there. This is order-independent, deterministic and cheap.
struct OccupancyGrid {
    /// The legacy generator and the rules placement pipeline share this
    /// footprint index. The legacy path still uses a coarse policy (hard vs
    /// soft), while the rules path can apply the full descriptor constraints.
    index: crate::rules::PlacementIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OccLayer {
    Hard,
    Soft,
}

impl OccupancyGrid {
    fn new(_width_m: u32, _depth_m: u32) -> OccupancyGrid {
        OccupancyGrid {
            index: crate::rules::PlacementIndex::new(),
        }
    }

    /// Mark a footprint (axis-aligned box) as occupied by a layer.
    fn mark(&mut self, x: i32, z: i32, half_w: i32, half_d: i32, layer: OccLayer) {
        let kind = match layer {
            OccLayer::Hard => crate::rules::ItemKind::Building,
            OccLayer::Soft => crate::rules::ItemKind::Tree,
        };
        self.index.push(crate::rules::PlacedKind {
            clearance: 0.,
            id: None,
            kind,
            cx: x as f32,
            cz: z as f32,
            half_w: half_w as f32,
            half_d: half_d as f32,
            tags: Vec::new(),
        });
    }

    /// True if the footprint touches a hard (road / building) reservation.
    fn collides_hard(&self, x: i32, z: i32, half_w: i32, half_d: i32) -> bool {
        let fp = crate::rules::Footprint {
            rotation_y_deg: 0.,
            cx: x as f32,
            cz: z as f32,
            half_w: half_w as f32,
            half_d: half_d as f32,
        };
        self.index.as_slice().iter().any(|placed| {
            placed.kind.is_hard() && fp.overlaps(placed.cx, placed.cz, placed.half_w, placed.half_d)
        })
    }
}

/// Local slope under a footprint, in metres of rise per 100m of run. Buildings
/// and roads reject steep ground so they never clip into a mountainside.
fn local_slope(
    seed: u64,
    x: i32,
    z: i32,
    width_m: u32,
    depth_m: u32,
    half_w: i32,
    half_d: i32,
) -> f64 {
    let base = f64::from(terrain_height(
        seed,
        x,
        z,
        WaterKind::None,
        width_m,
        depth_m,
        0.0,
    ));
    let corners = [
        (x - half_w, z - half_d),
        (x + half_w, z - half_d),
        (x - half_w, z + half_d),
        (x + half_w, z + half_d),
    ];
    let max_delta = corners
        .iter()
        .map(|(cx, cz)| {
            (f64::from(terrain_height(
                seed,
                *cx,
                *cz,
                WaterKind::None,
                width_m,
                depth_m,
                0.0,
            )) - base)
                .abs()
        })
        .fold(0.0_f64, f64::max);
    // Slope percent: quarter-metres to metres, per 100m of footprint span.
    let span = (half_w.max(half_d) * 2).max(1) as f64;
    max_delta / 4.0 / span * 100.0
}

fn terrain_height(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterKind,
    scene_width: u32,
    scene_depth: u32,
    river_half_m: f64,
) -> i16 {
    terrain_height_with_geometry(
        seed,
        x,
        z,
        legacy_water_geometry(water, scene_width, scene_depth, river_half_m),
    )
}

fn terrain_height_with_geometry(seed: u64, x: i32, z: i32, water: WaterGeometry) -> i16 {
    terrain_height_with_geometry_carved(seed, x, z, water, &[])
}

/// Build the flat pads that flatten terrain under roads and buildings.
///
/// Every generated structure gets a level pad at the height its own `world_y`
/// was sampled from the natural terrain. Roads are strips (long along one
/// axis), buildings are rectangles. Pads blend back to the natural terrain
/// over a short ring so a lot meets the neighbouring ground without a cliff.
/// B02-W1 graded geometry version. Recorded so a world baked before the wider
/// 16m band is distinguishable from one baked after it; the band rule itself is
/// `graded_flat_top_half_extents`.
pub const GRADED_ROAD_GEOMETRY_REVISION: &str = "b02-w1-graded-band-v2";

/// The band the earthwork flattens to the authored longitudinal profile, in
/// world axes: the corridor's end platforms along the axis and the graded
/// roadbed across it. Earthwork and the independent road audit read this one
/// definition, so the audited band cannot drift from what is actually built.
///
/// Transverse half = `max(7, pavement/2 + 1)`. The 7m floor keeps the existing
/// 8m and 12m behaviour bit-identical (4+3 and 6+1 both resolve to 7) while a
/// 16m collector gets 9m, which covers its 8m pavement half plus the same 1m
/// graded margin the 12m case already had. A band narrower than the pavement
/// is refused by `graded_band_shortfall_m`, never silently treated as flat.
pub(crate) fn graded_flat_top_half_extents(entity: &EntityInstance) -> (i32, i32) {
    let along = entity.width_m as i32 / 2 + 4;
    let transverse = (entity.depth_m as i32 / 2 + 1).max(7);
    if entity.rotation_y_deg == 90. {
        (transverse, along)
    } else {
        (along, transverse)
    }
}

/// Shortfall, in metres, between the graded band and the declared pavement.
/// Zero means the earthwork covers every metre the audit requires exactly; a
/// positive value is one structural failure per road. Kept as a pure function
/// of the two halves so a regression of the band rule can be tested directly
/// (a 7m band under an 8m pavement half is exactly the pre-W1 defect).
pub(crate) fn graded_band_shortfall_m(pavement_half: i32, band_half: i32) -> u32 {
    (pavement_half - band_half).max(0) as u32
}

/// The corridor's graded transport footprint in world axes: the declared
/// pavement plus the corridor contract's fixed 3m shoulder, plus the end
/// platforms. This is the earthwork envelope the audit bounds with the fill
/// cap and the no-excavation rule.
///
/// Public because it is the ONE definition an independent reader (and the
/// hillside acceptance test) uses to measure a graded road's support band; the
/// carve, the road audit and the road cut footprint all read it.
pub fn graded_transport_half_extents(entity: &EntityInstance) -> (i32, i32) {
    let along = entity.width_m as i32 / 2 + 4;
    let transverse = entity.depth_m as i32 / 2 + 3;
    if entity.rotation_y_deg == 90. {
        (transverse, along)
    } else {
        (along, transverse)
    }
}

/// The declared paved roadbed in world axes: what the corridor contract calls
/// the road width, which must be held exactly and which the graded band has to
/// cover.
pub(crate) fn graded_pavement_half_extents(entity: &EntityInstance) -> (i32, i32) {
    let along = entity.width_m as i32 / 2 + 4;
    let transverse = entity.depth_m as i32 / 2;
    if entity.rotation_y_deg == 90. {
        (transverse, along)
    } else {
        (along, transverse)
    }
}

/// The entrance apron's run in metres: the distance over which the frontage
/// corridor carve spends the walkable rise between a pad's flat top and the
/// graded band edge of the road it is assigned. The corridor carve's blend is
/// this value, and the placement pre-filter allows a rise of
/// `access::WALKABLE_GRADE * ENTRANCE_APRON_RUN_M` because the audited route has
/// that much run to climb - never a single cell's worth.
pub(crate) const ENTRANCE_APRON_RUN_M: f64 = 2.;

/// B01-T2 R9-1: the support-pad half extents used by the CARVE, by the road
/// planner's rule-A' scans and by the AUDIT, derived once here so the three
/// cannot drift (they did: A' scanned +-12 while the carve and the audit use the
/// pad half width plus a 2 m margin, i.e. +-14 for a 24x24 site).
pub(crate) fn support_pad_half_extents(
    width_m: f32,
    depth_m: f32,
    rotation_y_deg: f32,
    precise: bool,
) -> (f64, f64) {
    let (half_w, half_d) = spatial_half_extents(width_m, depth_m, rotation_y_deg);
    let margin = if precise { 2.0 } else { 1.0 };
    (half_w + margin, half_d + margin)
}

/// The earthwork geometry of one FRONTAGE pad's own support carve, derived once
/// and read by `plan_terrain_carves`, by `bounded_cut::CutFootprint` and by the
/// independent audit. One definition, so "the cells the ledger describes",
/// "the cells the raster carves" and "the cells the audit recomputes" cannot
/// drift.
pub(crate) fn frontage_pad_carve_geometry(entity: &EntityInstance) -> (f64, f64, f64) {
    let (half_w, half_d) = support_pad_half_extents(
        entity.width_m,
        entity.depth_m,
        entity.rotation_y_deg,
        entity.grounding.frontage_road_point.is_some(),
    );
    let blend = 6.0_f64.max(3.0 * (f64::from(entity.ground_height_m()) + 32.0).max(0.0));
    (half_w, half_d, blend)
}

/// Does `kind` get a building support carve from [`plan_terrain_carves`]?
///
/// One list, two readers: the carve planner and
/// [`earthwork_blend::stamp`], so "this entity has a building ring" cannot mean
/// two different sets of entities. A road (`road`, `sidewalk`, `canal`,
/// `causeway`) is the other branch and keeps its graded corridor envelope.
pub(crate) fn carves_as_building(kind: &str) -> bool {
    matches!(
        kind,
        "building"
            | "building_tower"
            | "building_cluster"
            | "urban_building"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "resort_lodge"
            | "storefront"
            | "commercial_center"
            | "entertainment_center"
            | "school"
            | "town_hall"
            | "market"
            | "industrial"
            | "temple"
            | "church"
            | "parking_lot"
            | "convenience_store"
    )
}

/// Does `kind` get a graded CORRIDOR carve from [`plan_terrain_carves`]?
///
/// One list, two readers: the carve planner and
/// [`earthwork_blend::stamp`]'s fill rule, so "this entity's fill is a roadbed"
/// cannot mean two different sets of entities.
pub(crate) fn carves_as_road(kind: &str) -> bool {
    matches!(kind, "road" | "sidewalk" | "canal" | "causeway")
}

/// The earthwork rectangle and the base ring of ONE entity's own support carve,
/// derived once and read by [`plan_terrain_carves`] and by
/// [`earthwork_blend`]'s fill measurement, so "the cells the ring is measured over"
/// and "the cells the carve flattens" cannot drift.
///
/// The third number is the ring the carve publishes when the entity carries NO
/// derived one ([`carve_base_ring_m`]).
fn carve_geometry(entity: &EntityInstance) -> (f64, f64, f64) {
    let is_road = carves_as_road(entity.kind.as_str());
    // Roads are authored in local X/Z dimensions and may be rotated.
    // Convert that local footprint into world-space before carving: a
    // zero-degree road with width=1440 is long on X, while a road-ns with
    // rotation=90 is long on Z. The old branch swapped these extents for
    // the zero-degree case, causing an east-west road to carve a huge
    // north-south trench through houses and vegetation.
    if entity.grounding.road_profile.is_some() {
        // Exactly the band the independent audit checks for the design
        // level (see `graded_flat_top_half_extents`).
        let (half_w, half_d) = graded_flat_top_half_extents(entity);
        (half_w as f64, half_d as f64, 18.0)
    } else if is_road {
        let width = f64::from(entity.width_m);
        let depth = f64::from(entity.depth_m);
        let rotated_quarter_turn = (entity.rotation_y_deg.round() as i32).rem_euclid(180) == 90;
        if rotated_quarter_turn {
            // Local width becomes world depth; local depth becomes world
            // width after a quarter turn.
            (depth * 0.5 + 1.5, width * 0.5 + 4.0, 6.0)
        } else {
            // Local width remains world X; local depth remains world Z.
            (width * 0.5 + 4.0, depth * 0.5 + 1.5, 6.0)
        }
    } else {
        // Buildings keep local width/depth in the entity contract; rotate
        // those dimensions into world axes before baking their support pad.
        frontage_pad_carve_geometry(entity)
    }
}

/// The rectangle one entity's own carve flattens, in world axes: half width and
/// half depth of its flat top.
///
/// This is the entity's OWN RANGE for the OPT-IN fill ring
/// ([`earthwork_blend::entity_fill_depth_m`]): the fill depth that decides the ring
/// is the deepest fill inside exactly this rectangle, and it is the same rectangle
/// the carve flattens and the bounded-cut ledger books.
pub(crate) fn carve_flat_top_half_extents(entity: &EntityInstance) -> (f64, f64) {
    let (half_w, half_d, _) = carve_geometry(entity);
    (half_w, half_d)
}

/// The ring one entity's carve publishes when the entity carries NO derived one:
/// a road's own corridor ring (`6 m` flat top, `18 m` graded band), and the
/// historical fixed envelope for a structure.
///
/// The fill rule takes it as a FLOOR for a road only: a road's corridor transition
/// is structural, so [`earthwork_blend`] may only narrow the part of the ring the
/// fill actually spends. A structure's own ring is its size, which the fill rule
/// replaces outright.
pub(crate) fn carve_base_ring_m(entity: &EntityInstance) -> f64 {
    let (_, _, ring) = carve_geometry(entity);
    ring
}

fn plan_terrain_carves(entities: &[EntityInstance]) -> Vec<TerrainCarve> {
    let mut carves = Vec::new();
    for entity in entities {
        let kind = entity.kind.as_str();
        let is_road = carves_as_road(kind);
        let is_building = carves_as_building(kind);
        if !is_road && !is_building {
            continue;
        }
        // A structure deck (a registered water crossing) is carried by an
        // engineering structure, not by earthwork: it gets NO carve at all, so
        // the ground under the span - banks and the water bed - is never raised
        // and the water surface cannot move. Every other entity is carved exactly
        // as before; the flag defaults to false.
        if entity.grounding.structure_deck {
            continue;
        }
        let (half_w, half_d, blend) = carve_geometry(entity);

        // OPT-IN bounded cut: two engineered supports may excavate, each only
        // when its OWN published entity carries a cap:
        //
        //  * a FRONTAGE pad (`frontage::append` sets the cap after the policy and
        //    the per-cell guards held). The pad's flat top, its entrance apron and
        //    its frontage-point apron belong to the same support surface, so all
        //    three carry the cap;
        //  * a GRADED ROAD: the road plan stamps the cap on every road that
        //    carries a longitudinal profile (`RoadNetworkPlan::road_cut_mm`), and
        //    the road's support band is cut on its uphill side and filled on its
        //    downhill side - the real hillside road. A flat road without a
        //    profile has no authored surface to cut to and keeps the historical
        //    fill-only contract, so it carries no cap.
        let cut_cap_mm = if is_road {
            entity
                .grounding
                .road_profile
                .as_ref()
                .and(entity.grounding.bounded_cut_mm)
        } else if is_building {
            entity.grounding.bounded_cut_mm
        } else {
            None
        };
        carves.push(TerrainCarve {
            profile: entity.grounding.road_profile.clone().map(std::sync::Arc::new),
            cx: f64::from(entity.world_x),
            cz: f64::from(entity.world_z),
            half_w,
            half_d,
            target_h_m: f64::from(entity.ground_height_m()),
            // A cubic smoothstep has maximum derivative 1.5. Use the
            // representable raster floor (-32m) as a conservative fill bound
            // so this pad contributes at most a 1:2 transition gradient.
            // This does not certify the natural terrain or road longitudinal grade.
            //
            // OPT-IN `style.terrainPolicy.earthworkBlend`: a stamped entity
            // carries the ring its own footprint (`derived-from-footprint`) or its
            // own fill depth (`derived-from-fill`) derived - one definition each, in
            // `earthwork_blend` - and that ring already carries a road's own
            // corridor ring as its floor. It is still never wider than the fixed
            // envelope above, so either value can only narrow a ring. Every other
            // carve - and every entity of a world that never asked - takes the
            // historical expression unchanged, bit for bit.
            blend_m: match entity.grounding.earthwork_blend_m {
                Some(derived) => f64::from(derived)
                    .min(blend.max(3.0 * (f64::from(entity.ground_height_m()) + 32.0).max(0.0))),
                None => blend.max(3.0 * (f64::from(entity.ground_height_m()) + 32.0).max(0.0)),
            },
            priority: if is_road { 10 } else { 1 },
            disturbance: if is_road { 255 } else { 160 },
            cut_cap_mm,
        
            ramp: None,
});
        if is_building {
            if let Some([rx,rz]) = entity.grounding.frontage_road_point {
                if let Some(anchor) = entity.anchors.first() {
                    carves.push(TerrainCarve {
                        profile: None,
                        cx: (f64::from(anchor.world_x)+f64::from(rx))*0.5,
                        cz: (f64::from(anchor.world_z)+f64::from(rz))*0.5,
                        half_w: (f64::from(anchor.world_x)-f64::from(rx)).abs()*0.5+2.,
                        half_d: (f64::from(anchor.world_z)-f64::from(rz)).abs()*0.5+2.,
                        target_h_m: f64::from(entity.ground_height_m()),
                        blend_m: ENTRANCE_APRON_RUN_M, priority: 2, disturbance:160,
                        // The aprons stay fill-only. Scoping the cap to the
                        // pad's OWN flat top keeps one auditable footprint per
                        // pad: the ledger, the raster and the audit all describe
                        // exactly the same rectangle plus its graded ring, and a
                        // walkable 25% approach over the 20 m setback carries the
                        // same rise the cut takes out of the platform.
                        cut_cap_mm: None,
                    
                        ramp: None,
});
                }
            }
            // Generated entrance aprons are part of the engineered support
            // contract, so the final raster can meet the door without a
            // graph-only promise.
            for anchor in &entity.anchors {
                carves.push(TerrainCarve {
                    profile: None,
                    cx: f64::from(anchor.world_x),
                    cz: f64::from(anchor.world_z),
                    half_w: 2.0,
                    half_d: 2.0,
                    target_h_m: f64::from(entity.ground_height_m()),
                    blend_m: 2.0,
                    priority: 1,
                    disturbance: 160,
                    cut_cap_mm: None,
                    ramp: None,
                });
            }
            // OPT-IN `style.terrainPolicy.entryCorridor`: the pad's own graded
            // walking corridor, when the acceptance gate needed one. It is the
            // pad's OWN earthwork re-graded - a notch through this pad's fill -
            // and its carve box covers the strip plus its 2 m side ring so the
            // spatial index visits every cell it can claim.
            if let Some(corridor) = &entity.grounding.entry_corridor {
                let (across_lo, across_hi) = corridor.across_range();
                let (near, far) = corridor.axis_range();
                let (across_c, along_c) = (
                    (f64::from(across_lo) + f64::from(across_hi)) * 0.5,
                    (f64::from(near) + f64::from(far)) * 0.5,
                );
                let half_across = (f64::from(across_hi) - f64::from(across_lo)) * 0.5
                    + entry_corridor::RING_M;
                let half_along =
                    (f64::from(far) - f64::from(near)).abs() * 0.5 + entry_corridor::RING_M;
                let (cx, cz, half_w, half_d) = if corridor.along_x() {
                    (along_c, across_c, half_along, half_across)
                } else {
                    (across_c, along_c, half_across, half_along)
                };
                carves.push(TerrainCarve {
                    profile: None,
                    cx,
                    cz,
                    half_w,
                    half_d,
                    target_h_m: f64::from(entity.ground_height_m()),
                    blend_m: entry_corridor::RING_M,
                    priority: entry_corridor::PRIORITY,
                    disturbance: 160,
                    cut_cap_mm: None,
                    ramp: Some(std::sync::Arc::new(corridor.ramp())),
                });
            }
        }
    }
    // Keep deterministic ordering for metadata; geometry is the maximum of
    // independent, fill-only support envelopes, not an order-dependent blend.
    carves.sort_by(|a, b| a.priority.cmp(&b.priority));
    carves
}

/// Same as [`terrain_height_with_geometry`] but first flattens any road /
/// building pads that cover this cell. Natural (uncarved) calls use the plain
/// function; chunk baking passes the carve list so baked heightfields carry
/// the flat pads that entities then sit on.
/// The surface the BAKE publishes at one cell, composed from the natural ground and
/// the entity carves with the bounded-cut PLATFORM rule active.
///
/// The encoder, the terrain-semantics halo and the rebase all call this, so "the
/// surface the world publishes" has one definition. Every planning probe keeps
/// reading [`terrain_height_zoned`], which is fill-first: a candidate's own bench
/// must be judged by whether its cap can REACH its top, not by a surface that
/// assumes it already did.
fn published_terrain_height_metres(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> f64 {
    terrain_height_metres_with(seed, x, z, water, carves, PadPlatform::Active)
}

/// Same as [`terrain_height_zoned`] with the platform rule active: the one
/// composition the published raster is made of.
fn published_terrain_height_zoned(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    zoning: Option<&zoning::ZoningMask>,
) -> f64 {
    terrain_height_zoned_core(seed, x, z, water, carves, None, zoning, PadPlatform::Active)
}

/// [`published_terrain_height_zoned`] over an indexed carve list: the same cells,
/// the same answers, only the carves that can reach each cell are visited. The
/// bake's per-chunk encoder builds ONE index for the whole chunk and walks it cell
/// by cell.
fn published_terrain_height_zoned_indexed(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    index: &CarveIndex,
    zoning: Option<&zoning::ZoningMask>,
) -> f64 {
    terrain_height_zoned_core(
        seed,
        x,
        z,
        water,
        carves,
        Some(index),
        zoning,
        PadPlatform::Active,
    )
}

fn terrain_height_with_geometry_carved(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> i16 {
    (published_terrain_height_metres(seed, x, z, water, carves) * 4.).clamp(-128., 1024.) as i16
}

/// Unquantized metre evaluator. Quantization belongs at the storage boundary,
/// not in the natural/engineering source function.
fn terrain_height_zoned(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    zoning: Option<&zoning::ZoningMask>,
) -> f64 {
    terrain_height_zoned_with(seed, x, z, water, carves, zoning, PadPlatform::Dormant)
}

/// Whether a bounded-cut pad's declared flat top is an authoritative PLATFORM in
/// this composition.
///
/// * [`PadPlatform::Dormant`] is the historical composition, fill first: a cell
///   any carve raises is never excavated, so a pad's declared top is whatever the
///   whole earthwork happens to leave there. Every planning probe uses it, because
///   a candidate must be judged on whether its own cap can reach its top.
/// * [`PadPlatform::Active`] is what the bake PUBLISHES: inside a cap-carrying
///   pad's declared flat top the pad's own platform
///   (`bounded_cut::platform_surface_m`) is the answer, both above a neighbour's
///   blended fill and above its own bench. It is inert wherever the fill-first
///   composition already stands on the declared top, which is every world whose
///   cut audit passes, so it moves no byte that was already correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PadPlatform {
    Dormant,
    Active,
}

impl PadPlatform {
    fn active(self) -> bool {
        self == Self::Active
    }
}

fn terrain_height_zoned_with(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    zoning: Option<&zoning::ZoningMask>,
    platform: PadPlatform,
) -> f64 {
    terrain_height_zoned_core(seed, x, z, water, carves, None, zoning, platform)
}

fn terrain_height_zoned_core(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    index: Option<&CarveIndex>,
    zoning: Option<&zoning::ZoningMask>,
    platform: PadPlatform,
) -> f64 {
    let Some(mask) = zoning else {
        return terrain_height_metres_core(seed, x, z, water, carves, index, platform);
    };
    let natural = terrain_height_metres(seed, x, z, water, &[]);
    // The zoning path composes the carve itself, so the waterfront exclusion has
    // to be restated here; otherwise an opted-in scene would still fill its water
    // wherever a land-use zone exists.
    if carves.is_empty()
        || mask.protected_at(x, z)
        || water.forbids_earthwork(f64::from(x), f64::from(z))
    {
        return natural;
    }
    let engineered = apply_carves_select(natural, x, z, carves, index, platform);
    // The same two prohibitions `terrain_height_metres_with` states: a cut never
    // enters the structured water body or the shoreline public passage band.
    if engineered < natural
        && (water.contains_point(f64::from(x), f64::from(z)) || water.shore_band_cell(x, z))
    {
        return natural;
    }
    if engineered <= natural {
        // Below the natural surface is a bounded cut - the only earthwork that
        // can lower ground, and only from an OPT-IN frontage pad's own carve. The
        // zoning fill taper is a FILL bound and has nothing to say about it; the
        // cut's own per-cell guards (protected land, water, shore band, occupied
        // cells, the millimetre cap) already decided every cell. Equal is the
        // historical "no earthwork here" answer and is returned unchanged.
        return engineered;
    }
    engineered.min(natural + mask.fill_capacity_at(x, z))
}

/// The surface the PUBLISHED raster holds at one cell, composed from a natural
/// raster and the final carve list exactly as `terrain_height_zoned_with` composes
/// it (bounded-cut platform rule active, protected land and the waterfront
/// exclusion intact, the zoning fill taper applied).
///
/// The bake's chunk encoder reads the same function through
/// `published_terrain_height_zoned`; this raster-backed form exists so a PLANNING
/// stage that only holds the natural raster - the graded road's fill-contract
/// probe, `worldgen::road_support` - judges the same surface the world will
/// publish instead of a second approximation of it.
pub(super) fn published_surface_from_raster(
    natural: &BakedTerrain,
    mask: Option<&zoning::ZoningMask>,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    x: i32,
    z: i32,
) -> Option<f64> {
    published_surface_from_raster_core(natural, mask, water, carves, None, x, z)
}

/// [`published_surface_from_raster`] over an indexed carve list. The index is a
/// candidate filter only: every answer is still decided by the same composition
/// body.
#[allow(clippy::too_many_arguments)]
pub(super) fn published_surface_from_raster_indexed(
    natural: &BakedTerrain,
    mask: Option<&zoning::ZoningMask>,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    index: &CarveIndex,
    x: i32,
    z: i32,
) -> Option<f64> {
    published_surface_from_raster_core(natural, mask, water, carves, Some(index), x, z)
}

#[allow(clippy::too_many_arguments)]
fn published_surface_from_raster_core(
    natural: &BakedTerrain,
    mask: Option<&zoning::ZoningMask>,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    index: Option<&CarveIndex>,
    x: i32,
    z: i32,
) -> Option<f64> {
    let base = f64::from(natural.height_millimetres(x, z)?) / 1000.;
    let forbidden = water.contains_point(f64::from(x), f64::from(z)) || water.shore_band_cell(x, z);
    let Some(mask) = mask else {
        let engineered = apply_carves_select(base, x, z, carves, index, PadPlatform::Active);
        return Some(if engineered < base && forbidden {
            base
        } else {
            engineered
        });
    };
    if carves.is_empty()
        || mask.protected_at(x, z)
        || water.forbids_earthwork(f64::from(x), f64::from(z))
    {
        return Some(base);
    }
    let engineered = apply_carves_select(base, x, z, carves, index, PadPlatform::Active);
    if engineered < base && forbidden {
        return Some(base);
    }
    if engineered <= base {
        return Some(engineered);
    }
    Some(engineered.min(base + mask.fill_capacity_at(x, z)))
}

/// Compose one cell through the plain or the indexed body. Both read the same
/// `carves`; the index only chooses which of them are visited.
#[inline]
fn apply_carves_select(
    terrain: f64,
    x: i32,
    z: i32,
    carves: &[TerrainCarve],
    index: Option<&CarveIndex>,
    platform: PadPlatform,
) -> f64 {
    match index {
        Some(index) => apply_terrain_carves_indexed(terrain, x, z, carves, index, platform),
        None => apply_terrain_carves_with(terrain, x, z, carves, platform),
    }
}

fn terrain_height_metres(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> f64 {
    terrain_height_metres_with(seed, x, z, water, carves, PadPlatform::Dormant)
}

fn terrain_height_metres_with(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    platform: PadPlatform,
) -> f64 {
    terrain_height_metres_core(seed, x, z, water, carves, None, platform)
}

fn terrain_height_metres_core(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
    index: Option<&CarveIndex>,
    platform: PadPlatform,
) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    // Landform field is continuous in world space: valley -> plain -> hill ->
    // mountain as the scalar rises. Elevation is a smooth function of it, so
    // terrain never band-jumps and stays continuous across chunk boundaries.
    let landform = landform_field(seed, x, z, water);
    let detail = fbm2d(seed, xf, zf);
    // Gentle slope: base landform plus a modest detail ripple. The detail
    // amplitude stays small enough that adjacent 1m cells never step more
    // than ~4m, keeping both water shores and chunk seams continuous.
    // Three spatial bands are baked into one authoritative heightfield:
    // macro landform, readable district-scale relief, then restrained local
    // detail. Renderers may add only shading micro-detail, never a competing
    // geometric displacement that would detach roads/buildings from the map.
    let natural = landform
        + mesoscale_relief(seed, x, z, water)
        + detail * (3.0 + landform.max(0.0) * 0.18)
        + 14.0;
    // Approximate valley carving shapes wild ground; this is not an erosion
    // or sediment-transport solver. The water branches below
    // then clamp the lake/river bed, and settled land is masked to zero.
    let natural = natural + approximate_valley_carve(seed, x, z, water);
    let terrain = match water.kind {
        WaterKind::Lake => {
            let radius = lake_radius_at(xf, zf, water);
            // The natural height is capped near the waterline so a steep
            // mountain-backed bank can't form a cliff at the lake edge: the
            // shore stays low and wide, then climbs back to full natural
            // height well away from the water.
            let low_shore = natural.min(10.0).max(1.5);
            let shore_target = if radius < SHORE_RADIUS {
                low_shore
            } else if radius < SHORE_RADIUS + 3.0 {
                let t = ((radius - SHORE_RADIUS) / 3.0).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);
                low_shore + (natural - low_shore) * s
            } else {
                natural
            };
            if radius < 1.0 {
                let depth = (-5.0 + radius * 4.0).max(-7.0);
                depth.min(shore_target * 0.2)
            } else if radius < SHORE_RADIUS {
                let t = ((radius - 1.0) / (SHORE_RADIUS - 1.0)).clamp(0.0, 1.0);
                let smooth_t = t * t * (3.0 - 2.0 * t);
                -4.0 + (shore_target + 4.0) * smooth_t
            } else {
                shore_target
            }
        }
        WaterKind::River => {
            let min_z = water.center_z - water.half_depth;
            let max_z = water.center_z + water.half_depth;
            if !(min_z..=max_z).contains(&zf) {
                // The authored river footprint is finite in world Z.
                natural
            } else {
                let half_width = river_half_width_at(
                    zf - min_z,
                    (water.half_depth * 2.0) as u32,
                    water.half_width,
                );
                let distance = (xf - water.center_x).abs();
                let shore_target = natural.max(1.5);
                if distance < half_width {
                    let depth_t = (distance / half_width.max(1.0)).clamp(0.0, 1.0);
                    let bed = -5.0 + depth_t * depth_t * 4.0;
                    bed.max(shore_target * 0.2 - 6.0).max(-7.0)
                } else if distance < half_width * 1.35 {
                    let t = ((distance - half_width) / (half_width * 0.35)).clamp(0.0, 1.0);
                    let smooth_t = t * t * (3.0 - 2.0 * t);
                    -4.0 + (shore_target + 4.0) * smooth_t
                } else {
                    shore_target
                }
            }
        }
        WaterKind::None => natural,
    };
    // Combine independent fill envelopes. Repeatedly blending the previous
    // result stacks unrelated ramps and makes the result order-dependent.
    // Raster clamping is part of the natural datum for engineering as well.
    //
    // Opt-in waterfront: NO earthwork may enter the water body. Carve blend
    // rings reach well past their pads (6 m for a flat road, 18 m for a graded
    // one), so a fill that merely starts on the bank would still raise the bed
    // and shrink the water surface. The exclusion is therefore a property of the
    // CELL, not of the carve: the natural raster is the final answer inside the
    // water, and roads that reach it are structure decks. Off by default, and
    // when off this branch is not even evaluated.
    if water.forbids_earthwork(xf, zf) {
        return terrain.clamp(-32., 256.);
    }
    let composed = apply_carves_select(terrain, x, z, carves, index, platform);
    // The bounded-cut contract's two natural prohibitions, re-derived here for
    // EVERY carve that can excavate (a frontage pad's placement guards already
    // refuse them, a graded road has no placement step): no cut ever enters the
    // structured water body or the shoreline public passage band. A fill is not
    // this clause's business - the historical world fills banks - so only a cell
    // that actually stands BELOW its natural datum is reset.
    if composed < terrain
        && (water.contains_point(xf, zf) || water.shore_band_cell(x, z))
    {
        return terrain.clamp(-32., 256.);
    }
    composed
}
fn apply_terrain_carves(terrain: f64, x: i32, z: i32, carves: &[TerrainCarve]) -> f64 {
    apply_terrain_carves_with(terrain, x, z, carves, PadPlatform::Dormant)
}

/// Is `has_road_cut`'s question true of this list? A property of the LIST, not of
/// a cell, so the indexed path answers it once per list instead of once per cell.
fn carve_list_has_road_cut(carves: &[TerrainCarve]) -> bool {
    carves
        .iter()
        .any(|carve| carve.priority == 10 && carve.cut_cap_mm.is_some() && carve.profile.is_some())
}

/// The metres around a carve's own rectangle within which it can still change any
/// cell, so a spatial index may omit it everywhere else. Read off the composition
/// body itself, whose every term is gated on one of
///
///  * `outside < blend_m` (the transition fill and the graded influence),
///  * `outside < ring_width_m(cap)` (the bounded cut, which never leaves its ring),
///  * `outside <= 2.0` (the audit window counts and the graded road's 2 m shoulder).
///
/// A carve further away than the widest of those enters no term, so leaving it out
/// cannot change a single answer.
fn carve_influence_m(carve: &TerrainCarve) -> f64 {
    let ring = carve
        .cut_cap_mm
        .map_or(2.0, |cap| bounded_cut::ring_width_m(cap));
    carve.blend_m.max(ring).max(2.0)
}

/// Bucket edge of the carve grid, in metres. A fixed coarse size: the grid is a
/// candidate filter only, and the composition body still applies its own exact
/// window test to every candidate it is handed.
const CARVE_BUCKET_M: f64 = 32.0;
/// Refuse to build a grid finer than this many buckets; such a list falls back to
/// the plain linear walk, which is always correct.
const MAX_CARVE_BUCKETS: f64 = 1_046_576.0;

/// A uniform bucket grid over one carve list, so one composition cell visits the
/// carves that can reach it instead of the whole list.
///
/// `active(x, z)` answers a SUPERSET of the carves whose own window holds the cell:
/// a carve is filed in every bucket its rectangle expanded by `carve_influence_m`
/// touches, so a cell it can affect always finds it. The composition body re-tests
/// each candidate exactly as before, in the list's own order, which is why pruning
/// cannot move an answer.
pub(super) struct CarveIndex {
    cell: f64,
    min_x: f64,
    min_z: f64,
    cols: i32,
    rows: i32,
    buckets: Vec<Vec<u32>>,
    /// Every index, in order, used when the grid is refused.
    all: Vec<u32>,
    has_road_cut: bool,
    indexed: bool,
}

impl CarveIndex {
    pub(super) fn new(carves: &[TerrainCarve]) -> Self {
        let has_road_cut = carve_list_has_road_cut(carves);
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_z = f64::INFINITY;
        let mut max_z = f64::NEG_INFINITY;
        for carve in carves {
            let reach = carve_influence_m(carve);
            min_x = min_x.min(carve.cx - carve.half_w - reach);
            max_x = max_x.max(carve.cx + carve.half_w + reach);
            min_z = min_z.min(carve.cz - carve.half_d - reach);
            max_z = max_z.max(carve.cz + carve.half_d + reach);
        }
        let refused = || Self {
            cell: CARVE_BUCKET_M,
            min_x: 0.,
            min_z: 0.,
            cols: 0,
            rows: 0,
            buckets: Vec::new(),
            all: (0..carves.len() as u32).collect(),
            has_road_cut,
            indexed: false,
        };
        if carves.is_empty() || !min_x.is_finite() || !min_z.is_finite() {
            return refused();
        }
        let cols = ((max_x - min_x) / CARVE_BUCKET_M).floor() + 1.;
        let rows = ((max_z - min_z) / CARVE_BUCKET_M).floor() + 1.;
        if cols <= 0. || rows <= 0. || cols * rows > MAX_CARVE_BUCKETS {
            return refused();
        }
        let (cols, rows) = (cols as i32, rows as i32);
        let mut buckets = vec![Vec::new(); (cols * rows) as usize];
        let bucket_of = |v: f64, min: f64, n: i32| -> i32 {
            (((v - min) / CARVE_BUCKET_M).floor() as i64).clamp(0, i64::from(n) - 1) as i32
        };
        for (i, carve) in carves.iter().enumerate() {
            let reach = carve_influence_m(carve);
            let x0 = bucket_of(carve.cx - carve.half_w - reach, min_x, cols);
            let x1 = bucket_of(carve.cx + carve.half_w + reach, min_x, cols);
            let z0 = bucket_of(carve.cz - carve.half_d - reach, min_z, rows);
            let z1 = bucket_of(carve.cz + carve.half_d + reach, min_z, rows);
            for bz in z0..=z1 {
                for bx in x0..=x1 {
                    buckets[(bz * cols + bx) as usize].push(i as u32);
                }
            }
        }
        Self {
            cell: CARVE_BUCKET_M,
            min_x,
            min_z,
            cols,
            rows,
            buckets,
            all: Vec::new(),
            has_road_cut,
            indexed: true,
        }
    }

    fn has_road_cut(&self) -> bool {
        self.has_road_cut
    }

    /// Carves that can reach `(x, z)`, in the list's own order. A cell outside the
    /// grid is reached by nothing (the grid covers every carve's whole reach).
    #[inline]
    fn active(&self, x: i32, z: i32) -> &[u32] {
        if !self.indexed {
            return &self.all;
        }
        let bx = ((f64::from(x) - self.min_x) / self.cell).floor();
        let bz = ((f64::from(z) - self.min_z) / self.cell).floor();
        if !(0.0..f64::from(self.cols)).contains(&bx) || !(0.0..f64::from(self.rows)).contains(&bz) {
            return &[];
        }
        &self.buckets[bz as usize * self.cols as usize + bx as usize]
    }
}

/// A dense memo of the PUBLISHED composition for the cells of ONE rectangle.
///
/// # Key
///
/// `(the rectangle, the carve set)`. The rectangle IS the storage - a row-major
/// `Vec` addressed by the cell's own offset, so a hit is two subtractions and an
/// array read, never a hash - and the owner fills it from ONE carve list, which it
/// passes unchanged for the memo's whole life. `reset` is the only writer of the
/// key and drops every value, so two different carve lists cannot share a memo.
///
/// # Why a window memo and not a per-cell hash map
///
/// Measured on the 1 km gate: a `HashMap<(i32, i32), f64>` memo keyed by carve set
/// saved 1.0 M of 21.6 M compositions and still made the gate 89.2 s against 67.9 s
/// without it - hashing, probing and rehashing ~20 M quiet cells in a debug build
/// costs far more than the composition it replaces. A dense window costs one bounds
/// check per cell, so this reuse is a win and not a tax.
///
/// # Exactness
///
/// A hit returns the value composed for the SAME rectangle and the SAME carve list
/// by `published_surface_from_raster_indexed`; a miss composes with that same
/// function. The memo cannot introduce a second definition of the surface.
pub(super) struct WindowMemo {
    x0: i32,
    z0: i32,
    cols: i32,
    rows: i32,
    values: Vec<f64>,
}

impl WindowMemo {
    pub(super) fn new() -> Self {
        Self {
            x0: 0,
            z0: 0,
            cols: 0,
            rows: 0,
            values: Vec::new(),
        }
    }

    /// Has any rectangle been taken yet?
    pub(super) fn is_empty(&self) -> bool {
        self.cols <= 0 || self.rows <= 0
    }

    /// Forget every value and take the rectangle of the next window. Every cell
    /// starts UNCOMPOSED, marked by `f64::NAN` (a published surface is always a
    /// finite metre value), so a read before the first write is a miss and can never
    /// be answered with a made-up zero.
    pub(super) fn reset(&mut self, x0: i32, z0: i32, cols: i32, rows: i32) {
        self.x0 = x0;
        self.z0 = z0;
        self.cols = cols;
        self.rows = rows;
        self.values.clear();
        self.values
            .resize((i64::from(cols) * i64::from(rows)) as usize, f64::NAN);
    }

    #[inline]
    fn index(&self, x: i32, z: i32) -> Option<usize> {
        let dx = i64::from(x) - i64::from(self.x0);
        let dz = i64::from(z) - i64::from(self.z0);
        if dx < 0 || dz < 0 || dx >= i64::from(self.cols) || dz >= i64::from(self.rows) {
            return None;
        }
        Some((dz * i64::from(self.cols) + dx) as usize)
    }

    #[inline]
    pub(super) fn get(&self, x: i32, z: i32) -> Option<f64> {
        let value = self.index(x, z).map(|i| self.values[i])?;
        (!value.is_nan()).then_some(value)
    }

    #[inline]
    pub(super) fn set(&mut self, x: i32, z: i32, value: f64) {
        if let Some(i) = self.index(x, z) {
            self.values[i] = value;
        }
    }
}

fn apply_terrain_carves_with(
    terrain: f64,
    x: i32,
    z: i32,
    carves: &[TerrainCarve],
    platform: PadPlatform,
) -> f64 {
    apply_terrain_carves_generic(
        terrain,
        x,
        z,
        carves.len(),
        |i| &carves[i],
        platform,
        carve_list_has_road_cut(carves),
    )
}

fn apply_terrain_carves_indexed(
    terrain: f64,
    x: i32,
    z: i32,
    carves: &[TerrainCarve],
    index: &CarveIndex,
    platform: PadPlatform,
) -> f64 {
    let active = index.active(x, z);
    apply_terrain_carves_generic(
        terrain,
        x,
        z,
        active.len(),
        |i| &carves[active[i] as usize],
        platform,
        index.has_road_cut(),
    )
}

/// The one composition body; `at` addresses the carves the caller wants visited.
/// `has_road_cut` is a property of the WHOLE list (`carve_list_has_road_cut`), so a
/// pruned list passes it in rather than answering it from its own members.
#[allow(clippy::too_many_arguments)]
fn apply_terrain_carves_generic<'a, F>(
    terrain: f64,
    x: i32,
    z: i32,
    count: usize,
    at: F,
    platform: PadPlatform,
    has_road_cut: bool,
) -> f64
where
    F: Fn(usize) -> &'a TerrainCarve,
{
    let xf = f64::from(x);
    let zf = f64::from(z);
    let base_terrain = terrain.clamp(-32.0, 256.0);
    let mut terrain = base_terrain;
    // A road owns its band. This used to fire only for PROFILE-bearing carves,
    // which left every unprofiled (flat, `level_m`) road open to being raised by
    // a neighbouring pad's fill: the roadside apron that carries a pad's small
    // entrance rise reaches into the pavement, so the baked roadbed came out
    // above the level its own plan declares, the post-bake transport rebase then
    // moved the road entity by a whole metre (`ceil` of the raster), and the
    // frozen plan no longer matched - measured as one 64x8 road cell missing from
    // the realized transport ledger in the 1 km fixture. Roads are the carves
    // `plan_terrain_carves` gives priority 10.
    let mut graded_influence=false;
    let mut full_priority=i32::MIN;
    let mut full_target=base_terrain;
    // The deepest bounded cut any carve asks for at this cell (a strictly lower
    // surface than the natural one). `None` unless an OPT-IN frontage pad's carve
    // or one of its aprons reaches here, so every fill-only input leaves it
    // untouched and the answer is the historical one.
    let mut cut_floor: Option<f64> = None;
    // The platform of the cap-carrying pad whose DECLARED FLAT TOP holds this
    // cell, when the platform rule is active. Only such a pad has an audited
    // "declared top is the surface, cell for cell" obligation; every other carve
    // stays fill-first.
    let mut pad_platform: Option<f64> = None;
    // OPT-IN `style.terrainPolicy.entryCorridor`: the graded walking ramp whose
    // pavement or side ring holds this cell. A ramp is the pad's own fill re-graded,
    // so inside its pavement it owns the published surface (it replaces the pad's
    // ring instead of competing with it - a `max` would keep the very bulge the ramp
    // exists to remove), and in its side band it replaces blended fill for the same
    // reason: the bank of a graded ramp is a shape, not a competing pad. It is always
    // at or above the natural ground, so it is never an excavation; a road's own
    // band, a road cut and a cap-carrying pad's platform still win over it. `None`
    // for every cell of every input that did not opt in.
    let mut ramp_claim: Option<f64> = None;
    // The support surface of a GRADED ROAD whose transport band (graded flat top +
    // the corridor's shoulder) holds this cell, and which no other entity's
    // support window claims. Inside that band the road's own surface - its
    // authored profile, filled downhill and cut uphill within the one cap - is
    // what the world publishes, so a neighbouring pad's transition ring can no
    // longer bury the roadbed above its 6 m fill contract. `None` everywhere else,
    // which keeps every fill-only input byte-identical.
    let mut road_support: Option<f64> = None;
    // The deepest unblocked ROAD excavation at this cell. Unlike a frontage pad's
    // cut - which the historical fill-first resolution keeps under any fill - a
    // graded road's cut is the road's own support surface, so it is authoritative
    // over a neighbour's transition fill exactly as the band's fill is. It is
    // still never inside another entity's support window.
    let mut road_cut_floor: Option<f64> = None;
    let mut road_owns_cell = false;
    // Another entity's own support window. TWO questions are asked per cell,
    // because the road answers two different things there:
    //
    //  * a road CUT may not enter a cell another entity's own earthwork claims -
    //    a pad's carve rectangle plus its cut footprint (or its 2 m audit margin)
    //    and another road's graded band. Those are the cells whose surface that
    //    entity's ledger and audit answer for, so a road excavation there would be
    //    booked against the wrong owner;
    //  * the road's FILL authority - its authored surface is the answer inside its
    //    transport band - is blocked only by another entity's SUPPORT RECTANGLE
    //    (the cells its own platform/fill actually stands on). A pad's graded ring
    //    is fill the road may legitimately replace with its own lower band; the
    //    pad's own platform, its audit margin and another road's band are not.
    //
    // Both counts include the asking road's own window; it subtracts that one when
    // it asks, so a road is never blocked by itself. The pre-pass exists because a
    // blocking entity may sort after the road it blocks.
    let mut cut_windows = 0_i32;
    let mut support_windows = 0_i32;
    // The same two counts restricted to OTHER ROADS' bands, which are the only
    // claims that can take a cell away from a graded road's own flat top.
    let mut road_cut_windows = 0_i32;
    let mut road_support_windows = 0_i32;
    // Is this cell inside SOME graded road's own flat top? When it is, the road
    // OWNS the surface there: a building's carve rectangle, its entrance apron or
    // its 8 m graded ring may not decide what the roadbed is. That is not a new
    // claim - the pad's platform stands at least its own half width away from the
    // band, and a pad whose bench would reach into the band is refused by
    // `frontage::support_check` - but it is now the difference between a roadbed
    // that is its authored profile and one a neighbour's ring buries.
    let mut road_flat_here = false;
    if has_road_cut {
        // One pass answers both the flat-top question and the two blocking
        // counts; the former used to be its own walk over the same list.
        for i in 0..count {
            let other = at(i);
            let out = ((xf - other.cx).abs() - other.half_w)
                .max((zf - other.cz).abs() - other.half_d);
            if other.priority == 10 && out <= 0.0 {
                road_flat_here = true;
            }
            let margin = match (other.priority == 10, other.cut_cap_mm) {
                (false, Some(cap)) => bounded_cut::ring_width_m(cap),
                _ => 2.0,
            };
            if out < margin {
                cut_windows += 1;
                if other.priority == 10 {
                    road_cut_windows += 1;
                }
            }
            if out <= 0.0 {
                support_windows += 1;
                if other.priority == 10 {
                    road_support_windows += 1;
                }
            }
        }
    }
    for i in 0..count {
        let carve = at(i);
        // OPT-IN `style.terrainPolicy.entryCorridor`: a graded walking ramp has
        // no flat top and no blend ring of its own. Its pavement claims the cell
        // (see `ramp_claim`), and its 2 m side ring is ordinary FILL, exactly the
        // shape a pad's ring has - so a ramp never excavates and never competes
        // with a road's band.
        if let Some(ramp) = &carve.ramp {
            let along = ramp.along(xf, zf);
            if along >= -0.5 && along <= ramp.stations() + 0.5 {
                let across = ramp.across(xf, zf);
                let target = ramp.target_m(xf, zf);
                if across <= ramp.half_width {
                    ramp_claim =
                        Some(ramp_claim.map_or(target, |known: f64| known.max(target)));
                } else if across <= ramp.half_width + carve.blend_m && target > base_terrain {
                    let t =
                        ((across - ramp.half_width) / carve.blend_m).clamp(0.0, 1.0);
                    let smooth_t = t * t * (3.0 - 2.0 * t);
                    let bank = target + (base_terrain - target) * smooth_t;
                    ramp_claim = Some(ramp_claim.map_or(bank, |known: f64| known.max(bank)));
                }
            }
            continue;
        }
        let dx = (xf - carve.cx).abs();
        let dz = (zf - carve.cz).abs();
        let outside_x = dx - carve.half_w;
        let outside_z = dz - carve.half_d;
        let outside = outside_x.max(outside_z);
        let target = carve.profile.as_ref().and_then(|p|p.height_at(xf,zf)).unwrap_or(carve.target_h_m);
        let limit = carve.profile.as_ref().map_or(f64::INFINITY, |p|base_terrain + f64::from(p.max_fill_mm)/1000.);
        if carve.priority == 10 && outside < carve.blend_m { graded_influence = true; }
        if carve.priority == 10 && outside <= 0. { road_owns_cell = true; }
        if outside <= 0. && carve.priority >= full_priority {
            if carve.priority > full_priority { full_target=base_terrain; }
            full_priority=carve.priority; full_target=full_target.max(target);
        }
        // A generated pad is an earthwork fill, not an implicit excavation.
        // Never lower existing terrain to an entity's center sample: doing so
        // creates pits when a footprint crosses a higher contour and is
        // especially wrong near the water datum. Explicit terrain edits may
        // excavate later; generated roads/buildings may only raise or gently
        // meet the natural surface.
        if outside <= 0.0 {
            terrain = terrain.max(target.min(limit));
        } else if outside < carve.blend_m && target > base_terrain {
            let t = (outside / carve.blend_m).clamp(0.0, 1.0);
            let smooth_t = t * t * (3.0 - 2.0 * t);
            let fill = target + (base_terrain - target) * smooth_t;
            terrain = terrain.max(fill.min(limit));
        }
        // OPT-IN bounded cut. The carve's own cap is the ONLY way a cell can be
        // lowered: a carve without one cannot excavate, however high the natural
        // ground stands. `bounded_cut::cut_surface_m` is the one formula the
        // ledger and the audit also read, so the published depth is provable
        // from the plan and the natural raster alone.
        //
        // A cap-carrying GRADED ROAD is the other reader of that formula, and its
        // cut is clipped by every other entity's support window (`support_windows`
        // minus its own). It also owns its transport band's FILL: the road's
        // authored surface is the answer there, not a neighbour's blend.
        if carve.priority == 10 && carve.cut_cap_mm.is_some() {
            if let (Some(cap), Some(profile)) = (carve.cut_cap_mm, carve.profile.as_deref()) {
                let own_cut_window = outside < 2.0;
                // Inside the road's own graded flat top only ANOTHER ROAD can take
                // the cell away; outside it every entity's support window still
                // clips the excavation exactly as before.
                let blocking_cut = if road_flat_here {
                    road_cut_windows
                } else {
                    cut_windows
                };
                let blocking_support = if road_flat_here {
                    road_support_windows
                } else {
                    support_windows
                };
                if blocking_cut - i32::from(own_cut_window) <= 0 {
                    let cut =
                        bounded_cut::cut_surface_m(base_terrain, target, outside, cap);
                    if cut < base_terrain {
                        road_cut_floor =
                            Some(road_cut_floor.map_or(cut, |known: f64| known.min(cut)));
                    }
                }
                let own_support = outside <= 0.0;
                if blocking_support - i32::from(own_support) <= 0 {
                    let cut =
                        bounded_cut::cut_surface_m(base_terrain, target, outside, cap);
                    let margin_m = road_transport_margin_m(profile);
                    let along_x = profile.start[1] == profile.end[1];
                    let support_outside = if along_x {
                        outside_x.max(dz - (carve.half_d + margin_m))
                    } else {
                        (dx - (carve.half_w + margin_m)).max(outside_z)
                    };
                    if support_outside <= 0.0 {
                        let fill = target.min(limit);
                        // Inside its own graded flat top the road's authored
                        // surface IS the answer - cut, fill, or exactly the natural
                        // ground. The boundary case used to fall through to the
                        // fill-first resolution and let a neighbour's transition
                        // ring stand in the roadbed (measured as 5.788 m of fill on
                        // a road whose own surface was the natural ground).
                        //
                        // Outside the flat top - the transport band's shoulder
                        // margin - the road is a FILL authority only: it keeps its
                        // own band from being buried, and never excavates there
                        // because the graded ring beyond the flat top is what the
                        // ledger's per-cell rule reserves for the neighbouring
                        // pads.
                        let authored = fill.max(cut);
                        let surface = if own_support {
                            authored
                        } else {
                            authored.max(base_terrain)
                        };
                        road_support =
                            Some(road_support.map_or(surface, |known: f64| known.min(surface)));
                    }
                }
            }
        } else if let Some(cap) = carve.cut_cap_mm {
            let surface = bounded_cut::cut_surface_m(base_terrain, target.min(limit), outside, cap);
            if surface < base_terrain {
                cut_floor = Some(cut_floor.map_or(surface, |known: f64| known.min(surface)));
            }
            // The PLATFORM rule: inside the pad's own declared flat top the pad's
            // own platform stands, so the declared millimetre top is what the
            // world publishes there - not whatever a neighbour's blended fill
            // happens to leave. `bounded_cut::platform_surface_m` is
            // `max(target, cut_surface_m(base, target, 0, cap))`: the same two
            // numbers the ledger and the audit already hold, so no new formula.
            // It is inert wherever the fill-first answer is already the declared
            // top, and it never survives a bench its cap cannot reach.
            if platform.active() && outside <= 0.0 {
                let surface =
                    bounded_cut::platform_surface_m(base_terrain, target.min(limit), cap);
                pad_platform = Some(pad_platform.map_or(surface, |known: f64| known.max(surface)));
            }
        }
    }
    // A neighbouring blended fill cannot overwrite a validated hard support
    // surface. New graded worlds resolve overlaps by road/building priority;
    // legacy worlds without a graded influence retain their old envelopes.
    if graded_influence && full_priority!=i32::MIN { terrain=full_target.max(base_terrain); }
    // Fill always wins over an implicit excavation: a cell that any carve raises
    // above the natural surface is a fill cell and no pad's cut may trench it. The
    // exceptions are audited surfaces, each blocked by every other entity's support
    // window:
    //
    //  * a GRADED ROAD's transport band publishes its authored profile. The road's
    //    own fill is the answer there (a neighbour's transition ring may not bury
    //    the roadbed above its 6 m contract) and its own unblocked excavation is
    //    the answer outside it (the uphill grading ring);
    //  * a bounded-cut pad's own declared flat top publishes its platform
    //    (`bounded_cut::verify` step 5).
    //
    // Everything else keeps the historical resolution: a road's flat top wins over
    // a neighbour's fill, a fill wins over a pad's cut, and a pad's cut is the
    // answer only when nothing raised the cell.
    let resolved = if let Some(support) = road_support {
        support
    } else if let Some(cut) = road_cut_floor {
        cut
    } else if road_owns_cell {
        terrain
    } else if let Some(platform) = pad_platform {
        platform
    } else if let Some(claim) = ramp_claim {
        // The graded entry corridor's own pavement. It replaces the pad's fill
        // here (that is what makes the walk 25 % instead of the ring's 37 %), and
        // it can never publish below the natural ground.
        claim.max(base_terrain)
    } else if terrain > base_terrain {
        terrain
    } else {
        cut_floor.unwrap_or(terrain)
    };
    resolved.clamp(-32., 256.)
}

/// The transverse margin from a graded road's flat top to its transport band:
/// the corridor's fixed 3 m shoulder beyond the pavement, minus the graded
/// margin the flat top already adds. Derived from the profile width, so the
/// composition, the carve and the audit's `graded_transport_half_extents` read
/// one number (`0` for an 8 m road, `2` for 12/16 m).
fn road_transport_margin_m(profile: &road_profile::RoadProfile) -> f64 {
    let pavement_half = f64::from(profile.road_width_m) * 0.5;
    (pavement_half + 3.0) - (pavement_half + 1.0).max(7.0)
}

/// Climate moisture field in world space (0 = arid, 1 = humid).
///
/// Moisture comes from distance to the nearest water surface plus a fractal
/// climate band: coastal and lakeside land is humid (forest), inland and
/// leeward ground dries out toward steppe / desert. This is the mapgen4
/// "wind → rainfall → biome" idea expressed as a continuous pure function, so
/// biomes are climate-driven rather than random noise classification.
fn humidity_field(seed: u64, x: i32, z: i32, water: WaterGeometry) -> f64 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    // Distance to the nearest water body (lake centre or river axis).
    let dist_water = match water.kind {
        WaterKind::Lake => {
            let radius = lake_radius_at(xf, zf, water);
            radius.sqrt().max(0.0) * water.half_width.max(1.0)
        }
        WaterKind::River => (xf - water.center_x).abs() - water.half_width,
        WaterKind::None => f64::MAX,
    };
    let shore_damp = (1.0 - (dist_water / 500.0).clamp(0.0, 1.0)) * 0.45;
    // Broad climate bands: large-scale wet / dry regions.
    let climate = fbm2d(seed.wrapping_add(0xa5b0_cb4f), xf / 1800.0, zf / 1800.0);
    let band = (climate * 0.5 + 0.5).clamp(0.0, 1.0) * 0.55;
    // Moisture is pulled down on high ridges (rain shadow) and boosted in
    // valleys.
    let landform = landform_field(seed, x, z, water);
    let ridge = (landform / 90.0).clamp(0.0, 1.0);
    let elevation_effect = (1.0 - ridge) * 0.15;
    let geography = water.geography;
    let authored = if geography.authored {
        (f64::from(geography.rainfall_bias) - 0.5) * 0.42
            + f64::from(geography.wetland_bias) * 0.18
            + f64::from(geography.seasonality) * 0.06
    } else {
        0.0
    };
    (shore_damp + band + elevation_effect + authored).clamp(0.0, 1.0)
}

fn lake_radius_at(x: f64, z: f64, water: WaterGeometry) -> f64 {
    let dx = (x - water.center_x) / water.half_width.max(1.0);
    let dz = (z - water.center_z) / water.half_depth.max(1.0);
    let radial = (dx * dx + dz * dz).sqrt();
    if radial <= f64::EPSILON {
        return 0.0;
    }
    if !water.geography.authored {
        return radial * radial;
    }
    // Authored lakes are landforms, not scaled discs. Three low-frequency
    // harmonics bend bays and headlands while keeping the shoreline continuous
    // and deterministic; the orogeny and wind axes make different regions
    // produce different lake plans. The inverse-square form preserves the
    // existing radius < 1 water and SHORE_RADIUS transition semantics.
    let angle = dz.atan2(dx);
    let tectonic = f64::from(water.geography.orogeny_axis_rad);
    let wind = f64::from(water.geography.wind_rad);
    let shore_scale = 1.0
        + 0.105 * (3.0 * angle + tectonic).sin()
        + 0.065 * (7.0 * angle - tectonic * 1.7 + wind * 0.4).sin()
        + 0.035 * (13.0 * angle + wind * 0.9).sin();
    (radial / shore_scale.max(0.72)).powi(2)
}

fn surface_kind_with_geometry(seed: u64, x: i32, z: i32, height: i16, water: WaterGeometry) -> u8 {
    match water.kind {
        WaterKind::Lake => {
            let radius = lake_radius_at(f64::from(x), f64::from(z), water);
            if radius < 1.0 {
                return 3;
            }
            if radius < SHORE_RADIUS {
                return 4;
            }
        }
        WaterKind::River => {
            let z = f64::from(z);
            let min_z = water.center_z - water.half_depth;
            let max_z = water.center_z + water.half_depth;
            if (min_z..=max_z).contains(&z) {
                let distance = (f64::from(x) - water.center_x).abs();
                let half_width = river_half_width_at(
                    z - min_z,
                    (water.half_depth * 2.0) as u32,
                    water.half_width,
                );
                if distance < half_width {
                    return 3;
                }
                if distance < half_width * 1.35 {
                    return 4;
                }
            }
        }
        WaterKind::None => {}
    }
    // Eroded valley channels (rivers on land): where the hydrological carve
    // has cut the terrain down near the water datum and the flow is strong,
    // mark the channel as wet mud so rivers read as water courses on the
    // ground instead of grass grooves.
    if water.kind != WaterKind::None
        && height < 8
        && approximate_valley_carve(seed, x, z, water) < -14.0
    {
        return 4;
    }
    if height < -12 {
        // Below datum. The channel itself (distance < half_width) is already
        // marked water above. Beyond the channel, low ground must read as mud
        // or tidal flat, NOT as standing water: random puddles marching up the
        // shore look broken. Only a defined wetland band right at the waterline
        // may keep a hint of wet mud, and even that stays a mud surface (5/7)
        // so the eye reads one continuous water body instead of scattered pools.
        match water.kind {
            WaterKind::River => {
                let distance = (f64::from(x) - water.center_x).abs();
                if distance >= water.half_width && distance <= water.half_width + 90.0 {
                    // Tidal mudflat / reed bed surface (dark wet soil), still
                    // walkable-looking and clearly NOT water.
                    5
                } else {
                    7
                }
            }
            WaterKind::Lake => 7,
            WaterKind::None => 7,
        }
    } else if height > 700 {
        2
    } else {
        // Landform decides the base surface so terrain reads coherently.
        match Landform::classify(seed, x, z, water) {
            Landform::Valley => {
                // Fertile valley floor: soil with grass patches.
                match signed_noise(seed.rotate_left(31), x, z) {
                    value if value > 70 => 1,
                    value if value < -70 => 6,
                    _ => 5,
                }
            }
            Landform::Plain => {
                // Settlement plain: biome follows moisture — humid forest,
                // temperate grass, arid steppe.
                let moisture = humidity_field(seed, x, z, water);
                if moisture > 0.62 {
                    // Humid plain: forest / tall grass.
                    match signed_noise(seed.rotate_left(31), x, z) {
                        value if value > 40 => 6,
                        _ => 1,
                    }
                } else if moisture > 0.38 {
                    // Temperate grassland.
                    match signed_noise(seed.rotate_left(31), x, z) {
                        value if value > 82 => 1,
                        value if value < -82 => 7,
                        _ => 0,
                    }
                } else {
                    // Arid steppe: dry grass / dirt.
                    match signed_noise(seed.rotate_left(31), x, z) {
                        value if value > 60 => 7,
                        value if value < -60 => 5,
                        _ => 1,
                    }
                }
            }
            Landform::Hill => {
                // Rolling hills: forest on humid slopes, grass / rock on dry.
                let moisture = humidity_field(seed, x, z, water);
                if moisture > 0.55 {
                    match signed_noise(seed.rotate_left(31), x, z) {
                        value if value > 30 => 6,
                        value if value < -60 => 2,
                        _ => 0,
                    }
                } else {
                    match signed_noise(seed.rotate_left(31), x, z) {
                        value if value > 70 => 6,
                        value if value < -70 => 2,
                        _ => 0,
                    }
                }
            }
            Landform::Mountain => {
                // Mountain: rock and scree with highland grass.
                match signed_noise(seed.rotate_left(31), x, z) {
                    value if value > 64 => 2,
                    value if value < -64 => 5,
                    _ => 0,
                }
            }
        }
    }
}

#[cfg(test)]
fn generate_entities(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    water: WaterKind,
) -> Vec<EntityInstance> {
    let geometry = legacy_water_geometry(
        water,
        scene.width_m,
        scene.depth_m,
        river_half_width_m(landmarks, scene.width_m),
    );
    generate_entities_with_geometry(seed, scene, landmarks, geometry)
}

#[cfg(test)]
fn generate_entities_with_geometry(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
) -> Vec<EntityInstance> {
    generate_entities_with_profile_report(
        seed,
        scene,
        landmarks,
        geometry,
        &serde_json::Value::Null,
        None,
        None,
        None,
    )
    .expect("valid candidate identity")
    .0
}

#[cfg(test)]
fn generate_entities_with_profile(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
    style: &serde_json::Value,
) -> Vec<EntityInstance> {
    generate_entities_with_profile_report(seed, scene, landmarks, geometry, style, None, None, None)
        .expect("valid candidate identity").0
}

/// `zones` is the land-use plan the bake ALREADY decided (see
/// `plan_landuse_zones_for_style`): a scene whose style opted into the axis-and-node growth must be
/// generated under the plan it was planned with, not under a second, radial derivation of it. A
/// caller that passes `None` keeps the historical recomputation, so every input that does not opt in
/// is byte-identical.
fn generate_entities_with_profile_report(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
    style: &serde_json::Value,
    road_plan: Option<&road_network::RoadNetworkPlan>,
    development_plan: Option<&DevelopmentPlan>,
    zones: Option<&[LandUseZone]>,
) -> WorldgenResult<(
    Vec<EntityInstance>,
    Option<(usize, crate::rules::PlacementOutcome)>,
    Vec<DevelopmentProposal>,
    Vec<EntityCandidateKey>,
)> {
    let ids = CandidateIds::new(&scene.scene_id);
    let mut development_proposals = Vec::new();
    // The bayfront fixture band may only be replaced by the plan's own shorefront row when both
    // halves of that row's contract exist: a real road plan, a stated street morphology (a frontage
    // programme or a morphology profile), AND a row the plan could actually realize. The last
    // conjunct is what keeps this change non-regressive: where the quay frontage the blueprint
    // offers cannot hold a single shop (see `worldgen::shorefront`, whose refusals are published by
    // name), the world keeps the commerce it already had instead of trading it for nothing.
    let shorefront_row = road_plan.is_some_and(|plan| {
        frontage::shorefront_row_contract(style).is_some()
            && plan
                .service_outcomes
                .iter()
                .any(|outcome| {
                    outcome.request.id == shorefront::SHOREFRONT_ROW
                        && !outcome.planned_site_ids.is_empty()
                })
    });
    let mut entities = generate_entities_template(
        seed,
        scene,
        landmarks,
        geometry,
        &ids,
        shorefront_row,
        natural_only::enabled(style) || route_evolution::enabled(style),
    );
    ids.finish(&entities).map_err(WorldgenError::InvalidRules)?;
    // When a normalized settlement intent is present for a water scene, the
    // generic intent planner owns ordinary buildings. Remove only the legacy
    // generated building filler; authored/GIS landmarks and water/transport
    // infrastructure remain authoritative inputs.
    if geometry.kind != WaterKind::None
        && matches!(generation_settlement(style), Some("city" | "town"))
    {
        entities.retain(|entity| {
            !(entity.entity_id.starts_with("generated.") && is_building_kind(&entity.kind))
        });
    }
    // Apply the final footprint test after all deterministic jitter and template placement.
    // This is deliberately independent of the renderer, so a bad placement cannot be hidden.
    // Bridges claim their full span: any generated building or roadside prop that
    // overlaps a bridge deck/approach must be dropped so nothing clips the crossing.
    let bridge_spans: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| entity.kind == "bridge")
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1
                    + 30.0,
            )
        })
        .collect();
    // Tree canopies must not overlap buildings: a tree pushed against a shop
    // front or a resort wall reads as clipping, not as street planting. We
    // collect every building footprint once, then drop any tree whose crown
    // reaches into one.
    let road_footprints: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| matches!(entity.kind.as_str(), "road" | "sidewalk" | "boardwalk"))
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0 + 0.5,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1 + 0.5,
            )
        })
        .collect();
    let building_footprints: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| {
            matches!(
                entity.kind.as_str(),
                "building"
                    | "building_tower"
                    | "building_cluster"
                    | "urban_building"
                    | "residential_block"
                    | "residential_tower"
                    | "residential_home"
                    | "resort_lodge"
                    | "storefront"
                    | "commercial_center"
                    | "entertainment_center"
                    | "school"
                    | "town_hall"
                    | "market"
                    | "industrial"
                    | "temple"
                    | "church"
                    | "parking_lot"
                    | "green_space"
                    | "shopping_mall"
                    | "office_tower"
                    | "convenience_store"
                    | "night_market"
                    | "community_plaza"
            )
        })
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0 + 0.5,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1 + 0.5,
            )
        })
        .collect();
    entities.retain(|entity| {
        // Natural details must clear hard infrastructure. A tree crown on a
        // carriageway is not a street tree; it is a collision. Trees may still
        // sit beside roads once their full canopy clears the road footprint.
        if matches!(
            entity.kind.as_str(),
            "tree" | "bush" | "grass_clump" | "rock" | "fallen_log"
        ) {
            let detail_r =
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg)
                    .0
                    .max(
                        spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg)
                            .1,
                    );
            let overlaps_road = road_footprints.iter().any(|(rx, rz, rhx, rhz)| {
                f64::from((entity.world_x - rx).abs()) < detail_r + rhx
                    && f64::from((entity.world_z - rz).abs()) < detail_r + rhz
            });
            if overlaps_road {
                return false;
            }
            if entity.kind == "tree" {
                let overlaps_building = building_footprints.iter().any(|(bx, bz, bhx, bhz)| {
                    f64::from((entity.world_x - bx).abs()) < detail_r + bhx
                        && f64::from((entity.world_z - bz).abs()) < detail_r + bhz
                });
                if overlaps_building {
                    return false;
                }
            }
        }
        // Only volumetric buildings must clear every bridge corridor: a house
        // under the deck or overlapping an approach ramp would clip the
        // crossing. Roads, sidewalks, cars, pedestrians and vegetation are
        // road-adjacent and may pass beneath or beside the span.
        let is_building_kind = matches!(
            entity.kind.as_str(),
            "building"
                | "building_tower"
                | "building_cluster"
                | "urban_building"
                | "residential_block"
                | "residential_tower"
                | "residential_home"
                | "resort_lodge"
                | "storefront"
                | "commercial_center"
                | "entertainment_center"
                | "school"
                | "town_hall"
                | "market"
                | "industrial"
                | "temple"
                | "church"
                | "parking_lot"
                | "green_space"
        );
        if is_building_kind {
            // Buildings must clear every bridge corridor.
            let overlaps_bridge = bridge_spans.iter().any(|(bx, bz, bhx, bhz)| {
                f64::from((entity.world_x - bx).abs())
                    < spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0
                        + bhx
                    && f64::from((entity.world_z - bz).abs())
                        < spatial_half_extents(
                            entity.width_m,
                            entity.depth_m,
                            entity.rotation_y_deg,
                        )
                        .1 + bhz
            });
            if overlaps_bridge {
                return false;
            }
        }
        // This pass only clears hard infrastructure. The water body is judged by
        // `retain_shore_clearance` immediately below, with the audit's own constant.
        true
    });
    // The shore clearance is its own named filter so that the one constant, the
    // one predicate and the published audit are visibly the same rule.
    retain_shore_clearance(&mut entities, geometry);
    let explicit_settlement = generation_settlement(style);
    if natural_only::enabled(style) || route_evolution::enabled(style) {
        append_natural_content(seed, scene, geometry, &mut entities, &ids);
        retain_shore_clearance(&mut entities, geometry);
    } else if geometry.kind == WaterKind::None {
        // An explicit intent selects a generic planner. Fixture-free generation
        // is important here: the planner consumes normalized intent/profile
        // fields and never branches on a place name.
        match explicit_settlement {
            Some("wilderness") => {
                append_natural_content(seed, scene, geometry, &mut entities, &ids)
            }
            Some("pastoral") => append_pastoral_intent_content(seed, scene, &mut entities, &ids),
            Some("city" | "town") if style.get("landUseProfile").is_some() => {
                append_modern_landuse_content(
                    seed,
                    scene,
                    geometry,
                    &mut entities,
                    style,
                    road_plan,
                    development_plan,
                    &mut development_proposals,
                    &ids,
                    zones,
                )
            }
            Some("city" | "town") => append_generic_land_content(seed, scene, &mut entities, &ids),
            Some(_) => append_generic_land_content(seed, scene, &mut entities, &ids),
            None => {
                // Legacy manifests retain their established behavior.
                if style.get("landUseProfile").is_some() {
                    append_modern_landuse_content(
                        seed,
                        scene,
                        geometry,
                        &mut entities,
                        style,
                        road_plan,
                        development_plan,
                        &mut development_proposals,
                        &ids,
                        zones,
                    );
                } else {
                    append_generic_land_content(seed, scene, &mut entities, &ids);
                }
            }
        }
    } else if road_plan.is_some() {
        // Waterfront road-first: the road network is the PARENT of the
        // settlement, so the same land-use generator the waterless profile path
        // uses has to run here too - it is the one that consumes
        // `development_plan` (which the bake derived from this very road plan)
        // and injects the planned roads and roadside sites. The water-aware
        // generator below places a legacy shoreline settlement from its own
        // fixture coordinates and never consumes a plan, which is why a
        // waterfront scene used to come out with no planned roads at all.
        //
        // Reachable only for a scene whose manifest opted in
        // (`allowStructuredWater`): without the opt-in `road_network::enabled` is
        // false for a water scene, so `road_plan` is `None` and every existing
        // waterfront world keeps the shoreline generator byte-for-byte.
        append_modern_landuse_content(
            seed,
            scene,
            geometry,
            &mut entities,
            style,
            road_plan,
            development_plan,
            &mut development_proposals,
            &ids,
            zones,
        )
    } else if matches!(explicit_settlement, Some("city" | "town")) {
        // Water geometry is already present in the template. Add the settlement
        // on its derived banks/shoreline rather than using fixture coordinates.
        append_waterfront_intent_settlement(seed, scene, geometry, &mut entities, style, &ids);
    }
    ids.finish(&entities).map_err(WorldgenError::InvalidRules)?;
    // Content planners run after the template retain pass, so enforce the
    // final road/building clearance once all generated details exist.
    sanitize_natural_detail_clearance(&mut entities);
    // User-authored identity is defined by Manifest membership, never by a
    // string prefix. Restore authored records filtered by procedural-only
    // sanitation so rules audit fails explicitly instead of hiding a subject.
    for landmark in landmarks {
        if !entities.iter().any(|e| e.entity_id == landmark.entity_id) {
            entities.push(landmark_to_entity(landmark));
        }
    }
    // rulesMode = "rules": the declarative rules engine drives final placement.
    // The template generator still proposes positions, but every proposed
    // entity is re-validated by place_all(): violations are retried (move /
    // rotation) or dropped, never force-placed. Legacy mode (default) keeps
    // the existing post-hoc retain filters for A/B comparison.
    let rules_mode = style
        .get("rulesMode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("legacy");
    // Normalize before rule placement as well as before carving: a road-ns
    // proposal must occupy its north/south footprint while other candidates
    // are being tested, not only during the final audit.
    for entity in &mut entities {
        entity.normalize_spatial_semantics();
    }
    let mut placement_outcome = None;
    if rules_mode == "rules" {
        let proposed_count = entities.len();
        let (rule_entities, outcome) =
            apply_rules_mode(
                seed,
                scene,
                geometry,
                style,
                landmarks,
                entities,
                road_plan,
                zones,
            );
        entities = rule_entities;
        placement_outcome = Some((proposed_count, outcome));
        sanitize_natural_detail_clearance(&mut entities);
    }
    // Rule placement may have rotated/moved an entity, so refresh derived
    // bounds/anchors once after its final transform is known.
    for entity in &mut entities {
        entity.normalize_spatial_semantics();
    }
    rebase_buildings_to_surface(seed, geometry, &mut entities);
    rebase_generated_surfaces_to_ground(seed, geometry, &mut entities);
    if let Some(plan) = road_plan {
        let planned: BTreeMap<_, _> = plan
            .entities()
            .into_iter()
            .chain(
                development_plan
                    .map(DevelopmentPlan::building_entities)
                    .unwrap_or_default(),
            )
            .map(|e| (e.entity_id.clone(), e))
            .collect();
        for e in &mut entities {
            if let Some(source) = planned.get(&e.entity_id) {
                *e = source.clone();
                e.normalize_spatial_semantics();
            }
        }
    }
    ids.finish(&entities).map_err(WorldgenError::InvalidRules)?;
    if natural_only::enabled(style) {
        let audit = natural_only::audit_scene(
            &scene.scene_id,
            &entities,
            0,
            road_plan.is_some(),
            development_plan.is_some(),
        );
        natural_only::require_pass(&audit).map_err(WorldgenError::InvalidRules)?;
    }
    Ok((
        entities,
        placement_outcome,
        development_proposals,
        ids.keys(),
    ))
}

/// Re-validate / re-place the template-generated entities through the rules
/// engine. Each proposed entity becomes a PlacementRequest; `place_all` keeps
/// only positions that pass every hard constraint (same-kind collision, water,
/// slope, ground, entrance, biome/hazard), retrying with `move` when enabled.
/// Returns the rule-accepted entities (the rule-clean subset).
fn apply_rules_mode(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    style: &serde_json::Value,
    landmarks: &[LandmarkSpec],
    proposed: Vec<EntityInstance>,
    road_plan: Option<&road_network::RoadNetworkPlan>,
    zones: Option<&[LandUseZone]>,
) -> (Vec<EntityInstance>, crate::rules::PlacementOutcome) {
    // Object rules location: <repo>/assets/objects (resolved from the caller).
    let rules_dir = rules_dir_from_style(style);
    // bake_world() validates the same directory before generation. Keep this
    // second load fail-closed as well: a rules-mode generation must never
    // silently downgrade to legacy placement if the registry changes or
    // becomes unreadable between validation and use.
    let registry = crate::rules::ObjectRegistry::load_dir(&rules_dir)
        .unwrap_or_else(|error| panic!("rules mode registry became unavailable: {error}"));

    // Authored landmarks are fixed spatial anchors, not candidates for the
    // procedural placer. Keep them in the output, and remove only generated
    // volumetric entities that would enter their authored footprint. This is a
    // generic manifest rule: it protects any authored landform, building,
    // bridge or other landmark without naming a Shenzhen coordinate.
    let authored_ids: std::collections::BTreeSet<_> =
        landmarks.iter().map(|l| l.entity_id.as_str()).collect();
    let authored: Vec<EntityInstance> = proposed
        .iter()
        .filter(|e| authored_ids.contains(e.entity_id.as_str()))
        .cloned()
        .collect();
    let mut requests = Vec::new();
    let mut passthrough = Vec::new();
    // Opt-in: a planned roadside pad is an OBLIGATION, and a legacy
    // template filler that stands on one blocks the entrance the pad was designed
    // around - the rules engine then refuses the PAD, the road-network audit
    // reports a missing planned site, and a city loses a home to scenery. The
    // filler is refused BY NAME here (it reaches `rules-placement.json` like every
    // other named refusal) so the plan's site keeps its ground. Off for every
    // world that did not opt in, and the pads themselves are exempt.
    //
    // The axis-and-node growth policy opts in too, for the same reason and one more: a pad whose
    // order the RADIAL plan did not inherit stands where a filler could not have stood before, and a
    // filler platform that merely reaches into the pad's audit window buries the pad's declared flat
    // top - the bake then fails its bounded-cut ledger with `bounded_cut_pad_top_not_flat` on a pad
    // the planner was right to accept. The window is the same one the pad's own flat-top audit uses.
    //
    // Round 39: that window guard is not enough on its own. A filler can be collided
    // with the pad by a margin the window never sees - the grown hillside town's 38x20
    // `resort_lodge` is proposed 20 m away, is ROTATED 90 degrees by `place_all`'s
    // fallback, and then reaches 3.5 m into an 8x8 shop pad - and the pad's one
    // placement request is rejected while the plan goes on claiming the parcel. The
    // guard below is therefore paired with the attributed re-placement after
    // `place_all` (see the "planned pad is an OBLIGATION" block further down), which
    // refuses the structure the ENGINE blamed and places the scene again.
    let planned_pads: Vec<EntityInstance> =
        if water.road_water_crossings() || urban_growth::enabled(style) {
            road_plan
                .map(|plan| plan.site_entities())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    let planned_pad_ids: std::collections::BTreeSet<&str> = planned_pads
        .iter()
        .map(|pad| pad.entity_id.as_str())
        .collect();
    let mut refused_fillers = Vec::new();
    for e in proposed {
        let is_authored = authored_ids.contains(e.entity_id.as_str());
        if is_authored {
            passthrough.push(e);
            continue;
        }
        if !planned_pad_ids.contains(e.entity_id.as_str())
            && crate::settlement::is_building(&e.kind)
        {
            let (filler_hw, filler_hd) =
                spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
            if let Some(pad) = planned_pads.iter().find(|pad| {
                // The pad's PUBLISHED AUDIT WINDOW: the pad plus the 2 m apron the
                // carve, the flat-top audit and the descriptor clearance all work
                // on. A filler that merely touches that window already collides
                // with the pad's apron, so it is inside the pad's ground.
                let (hw, hd) = road_network::pad_audit_half_extents(
                    pad.width_m,
                    pad.depth_m,
                    pad.rotation_y_deg,
                );
                f64::from((pad.world_x - e.world_x).abs()) < hw + filler_hw
                    && f64::from((pad.world_z - e.world_z).abs()) < hd + filler_hd
            }) {
                refused_fillers.push(crate::rules::RejectRecord {
                    item_id: e.kind.clone(),
                    candidate_x: e.world_x,
                    candidate_z: e.world_z,
                    reason: format!(
                        "occupied by the planned roadside pad {} the waterfront plan owes",
                        pad.entity_id
                    ),
                    conflict_with: Some(pad.entity_id.clone()),
                    rule: "plan".into(),
                });
                continue;
            }
        }
        // A structure deck is not a terrain-grounded object: its pavement is a
        // span whose support is the registered bridge, and the rules engine's
        // surface/grounding model has no way to describe one (the road descriptor
        // forbids water by design). It is passed through instead of being
        // force-placed or deleted, and its own contract -
        // `road_network::audit_water_crossings`, which is stricter: no earthwork
        // anywhere in the water, the deck must clear the datum, and the span must
        // be registered - governs it. The flag is false for every other entity,
        // so no existing placement decision changes.
        if e.grounding.structure_deck {
            passthrough.push(e);
            continue;
        }
        let authored_collision = authored.iter().any(|fixed| {
            let fixed_is_mass = matches!(
                fixed.kind.as_str(),
                "building"
                    | "urban_building"
                    | "building_tower"
                    | "shopping_mall"
                    | "office_tower"
                    | "convenience_store"
                    | "hill"
                    | "mountain"
                    | "rock_mountain"
            );
            // Authored landforms own their complete footprint. Do not let
            // procedural lamps, trees, benches or other details be generated
            // inside the mountain/hill volume and then make the fixed landmark
            // fail its own audit. Buildings also clear authored building masses.
            let fixed_landform =
                matches!(fixed.kind.as_str(), "hill" | "mountain" | "rock_mountain");
            let generated_kind = !authored_ids.contains(e.entity_id.as_str());
            let blocked_by_landform = fixed_landform && generated_kind;
            let blocked_by_building = fixed_is_mass
                && matches!(
                    e.kind.as_str(),
                    "building"
                        | "urban_building"
                        | "building_tower"
                        | "residential_block"
                        | "residential_tower"
                        | "residential_home"
                        | "resort_lodge"
                        | "commercial_center"
                        | "entertainment_center"
                        | "school"
                        | "town_hall"
                        | "market"
                        | "industrial"
                        | "shopping_mall"
                        | "office_tower"
                        | "convenience_store"
                );
            (blocked_by_landform || blocked_by_building)
                && e.computed_bounds().min_x < fixed.computed_bounds().max_x
                && e.computed_bounds().max_x > fixed.computed_bounds().min_x
                && e.computed_bounds().min_z < fixed.computed_bounds().max_z
                && e.computed_bounds().max_z > fixed.computed_bounds().min_z
        });
        if authored_collision {
            continue;
        }
        let desc = registry
            .descriptors
            .values()
            .find(|d| d.matches_kind(&e.kind));
        match desc {
            Some(d) => requests.push(crate::rules::PlacementRequest {
                descriptor_id: d.id.clone(),
                x: e.world_x,
                z: e.world_z,
                count: 1,
                source: Some(crate::rules::PlacementSource {
                    entity_id: e.entity_id.clone(),
                    asset_id: e.asset_id.clone(),
                    kind: e.kind.clone(),
                    width_m: e.width_m,
                    depth_m: e.depth_m,
                    height_m: e.height_m,
                    scale: e.scale,
                    rotation_y_deg: e.rotation_y_deg,
                    grounding: e.grounding.clone(),
                    anchors: e.anchors.clone(),
                    bounds: e.bounds,
                }),
            }),
            None => passthrough.push(e),
        }
    }

    // The bake hands its own land-use plan down; a caller that passes none keeps the historical
    // recomputation from the style, byte for byte.
    let computed;
    let source_zones: Option<&[LandUseZone]> = match (road_plan, zones) {
        (None, _) => None,
        (Some(_), Some(zones)) => Some(zones),
        (Some(_), None) => match LandUseProfile::from_style(style) {
            Some(profile) => {
                computed = plan_landuse_zones(scene, &profile);
                Some(&computed)
            }
            None => None,
        },
    };
    let source_zoning = source_zones
        .map(|z| zoning::ZoningMask::new(scene, z).expect("computed planning zones are valid"));
    let terrain_context = TerrainContext {
        seed,
        water,
        precise: height_encoding_from_style(style).expect("validated height precision")
            == HeightEncoding::Millimetre,
        zoning: source_zoning.as_ref(),
    };
    let road_carves = road_plan
        .map(|p| {
            let mut entities = p.entities();
            entities.extend(p.site_entities());
            for e in &mut entities {
                e.normalize_spatial_semantics();
            }
            plan_terrain_carves(&entities)
        })
        .unwrap_or_default();
    let designed_height = |x, z| terrain_context.height(x, z, &road_carves);
    let place = |requests: &[crate::rules::PlacementRequest]| {
        with_height_placement_context(seed, scene, water, &designed_height, |ctx| {
            crate::rules::place_all(&registry, requests, ctx, seed)
        })
    };
    let mut outcome = place(&requests);
    // Round 39: a planned pad is an OBLIGATION, and `place_all` may not delete one.
    //
    // The engine has no way to know a request is a plan obligation: when the pad's one
    // candidate collides with an already placed structure it records a reject whose
    // `item_id` is the DESCRIPTOR ("storefront") and whose `conflict_with` is the
    // structure, and the pad's entity simply is not in the output. The plan, the
    // development plan, the cut ledger and the conversions went on claiming it - the
    // grown hillside town lost a shop exactly this way to a 38x20 `resort_lodge` whose
    // rotated (90 degree) footprint reached 3.5 m into the pad.
    //
    // The repair uses the engine's own attribution instead of a second geometry
    // predicate: for every planned pad the engine did not place, ask WHICH already
    // placed structure it blamed at the pad's own request position, and when that
    // structure is a building the carve planner knows (`carves_as_building` - never a
    // road, a canal or an authored landmark), refuse THAT structure by name and place
    // the scene again. The pad's request is re-evaluated against the world without the
    // structure that displaced it, and the structure cannot silently win a spot the
    // plan had already given the pad. Bounded: every re-placement drops at least one
    // request, so the loop can run at most once per planned pad.
    //
    // A pad the engine still cannot place after that is left to the entity-stage
    // completion pass, which now reads the PUBLISHED composition: a pad with no
    // platform standing publishes no declared top, so it is refused BY NAME through
    // `pad_completion` (plan, services, joint receipt, conversions and entity list
    // withdrawn together) instead of vanishing here.
    let mut displaced = Vec::new();
    for _ in 0..planned_pads.len() {
        if planned_pads.is_empty() {
            break;
        }
        let evicted: Vec<&EntityInstance> = planned_pads
            .iter()
            .filter(|pad| {
                !outcome.placed.iter().any(|e| e.entity_id == pad.entity_id)
                    && !passthrough.iter().any(|e| e.entity_id == pad.entity_id)
            })
            .collect();
        if evicted.is_empty() {
            break;
        }
        let mut blamed: Vec<(String, String)> = Vec::new();
        for pad in &evicted {
            for record in &outcome.rejected {
                if record.candidate_x != pad.world_x || record.candidate_z != pad.world_z {
                    continue;
                }
                let Some(other) = record.conflict_with.as_deref() else {
                    continue;
                };
                if planned_pad_ids.contains(other) || authored_ids.contains(other) {
                    continue;
                }
                let Some(source) = requests
                    .iter()
                    .filter_map(|request| request.source.as_ref())
                    .find(|source| source.entity_id == other)
                else {
                    continue;
                };
                if !carves_as_building(&source.kind) {
                    continue;
                }
                // The pad this reject belongs to, for the published record.
                if !blamed.iter().any(|(id, _)| id == other) {
                    blamed.push((other.to_string(), source.kind.clone()));
                }
            }
        }
        if blamed.is_empty() {
            break;
        }
        for (entity_id, kind) in &blamed {
            let Some(position) = requests
                .iter()
                .find(|request| {
                    request
                        .source
                        .as_ref()
                        .is_some_and(|source| source.entity_id == *entity_id)
                })
                .map(|request| (request.x, request.z))
            else {
                continue;
            };
            let pad = evicted
                .iter()
                .find(|pad| {
                    outcome.rejected.iter().any(|record| {
                        record.candidate_x == pad.world_x
                            && record.candidate_z == pad.world_z
                            && record.conflict_with.as_deref() == Some(entity_id.as_str())
                    })
                })
                .map(|pad| pad.entity_id.clone())
                .unwrap_or_default();
            displaced.push(crate::rules::RejectRecord {
                item_id: kind.clone(),
                candidate_x: position.0,
                candidate_z: position.1,
                reason: format!(
                    "stands on the planned roadside pad {pad} the plan owes: {entity_id} and the \
                     pad collide, so the pad keeps its ground and this structure is refused by name"
                ),
                conflict_with: Some(pad),
                rule: "plan".into(),
            });
            requests.retain(|request| {
                request
                    .source
                    .as_ref()
                    .is_none_or(|source| source.entity_id != *entity_id)
            });
        }
        outcome = place(&requests);
    }
    refused_fillers.extend(displaced);
    // A move fallback can relocate a generated object into an authored mass even
    // when its proposal was clear. Apply the same fixed-landmark reservation to
    // the committed result before merging passthrough landmarks.
    let mut merged = outcome.placed.clone();
    merged.retain(|e| {
        let generated_kind = !authored_ids.contains(e.entity_id.as_str());
        !authored.iter().any(|fixed| {
            (matches!(fixed.kind.as_str(), "hill" | "mountain" | "rock_mountain")
                || (matches!(
                    fixed.kind.as_str(),
                    "building"
                        | "building_tower"
                        | "shopping_mall"
                        | "office_tower"
                        | "convenience_store"
                ) && matches!(
                    e.kind.as_str(),
                    "building"
                        | "urban_building"
                        | "building_tower"
                        | "residential_block"
                        | "residential_tower"
                        | "residential_home"
                        | "resort_lodge"
                        | "commercial_center"
                        | "entertainment_center"
                        | "school"
                        | "town_hall"
                        | "market"
                        | "industrial"
                        | "shopping_mall"
                        | "office_tower"
                        | "convenience_store"
                )))
                && generated_kind
                && e.computed_bounds().min_x < fixed.computed_bounds().max_x
                && e.computed_bounds().max_x > fixed.computed_bounds().min_x
                && e.computed_bounds().min_z < fixed.computed_bounds().max_z
                && e.computed_bounds().max_z > fixed.computed_bounds().min_z
        })
    });
    merged.extend(passthrough);
    outcome.rejected.extend(refused_fillers);
    (merged, outcome)
}

/// Resolve the object-rules directory: `style.rulesDir` if set, else the
/// default `assets/objects` relative to the current directory.
fn rules_dir_from_style(style: &serde_json::Value) -> std::path::PathBuf {
    if let Some(dir) = style.get("rulesDir").and_then(serde_json::Value::as_str) {
        return std::path::PathBuf::from(dir);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let candidates = [
        cwd.join("assets").join("objects"),
        cwd.join("..").join("assets").join("objects"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| cwd.join("assets").join("objects"))
}

/// Scope environment closures to the consumer; repeated generation must not
/// leak one heap allocation per query via Box::leak.
#[cfg(test)]
fn with_rules_placement_context<T>(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    run: impl FnOnce(&crate::rules::PlacementContext<'_>) -> T,
) -> T {
    let height = |x, z| terrain_height_with_geometry(seed, x, z, water) as f32 / 4.0;
    with_height_placement_context(seed, scene, water, &height, run)
}

fn with_height_placement_context<T>(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    height: &dyn Fn(i32, i32) -> f32,
    run: impl FnOnce(&crate::rules::PlacementContext<'_>) -> T,
) -> T {
    let water_point = |x, z| {
        water
            .contains_point(f64::from(x), f64::from(z))
            .then_some(f32::MAX)
    };
    let water_footprint = |fp: &crate::rules::Footprint| {
        water.intersects_axis_aligned(
            f64::from(fp.cx),
            f64::from(fp.cz),
            f64::from(fp.half_w),
            f64::from(fp.half_d),
        )
    };
    let slope = |x, z| slope_from_height(&height, x, z, 8, 8);
    let footprint_slope = |x, z, hw, hd| slope_from_height(&height, x, z, hw, hd);
    let biome = |x, z| {
        let h = humidity_field(seed, x, z, water);
        if h > 0.55 {
            crate::rules::Biome::Forest
        } else if h < -0.2 {
            crate::rules::Biome::Desert
        } else {
            crate::rules::Biome::Grassland
        }
    };
    let hazard = |x, z| {
        if slope(x, z) > 30. {
            vec![crate::rules::HazardKind::Cliff]
        } else {
            vec![]
        }
    };
    run(&crate::rules::PlacementContext {
        height_at: &height,
        water_level: &water_point,
        water_intersects: Some(&water_footprint),
        slope_at: &slope,
        slope_at_footprint: Some(&footprint_slope),
        bounds: (
            scene.origin_x,
            scene.origin_z,
            scene.origin_x + scene.width_m as i32,
            scene.origin_z + scene.depth_m as i32,
        ),
        grounding_tolerance: 0.5,
        biome_at: Some(&biome),
        hazard_at: Some(&hazard),
    })
}

/// Same slope metric as local_slope, but derived from the caller's actual
/// terrain. A baked audit must not silently resample unrelated natural hills.
fn slope_from_height(height: &dyn Fn(i32, i32) -> f32, x: i32, z: i32, hw: i32, hd: i32) -> f32 {
    let base = height(x, z);
    let corners = [
        (x.saturating_sub(hw), z.saturating_sub(hd)),
        (x.saturating_add(hw), z.saturating_sub(hd)),
        (x.saturating_sub(hw), z.saturating_add(hd)),
        (x.saturating_add(hw), z.saturating_add(hd)),
    ];
    corners
        .into_iter()
        .map(|(x, z)| (height(x, z) - base).abs())
        .fold(0., f32::max)
        / (hw.max(hd).saturating_mul(2)).max(1) as f32
        * 100.
}

/// Audit already-generated entities against the declarative rules engine
/// (`rules::audit_entities`) without changing their placement. Environment
/// queries (height / water / slope) use this module's own terrain functions
/// so the audit matches what the bake actually produced.
///
/// Returns a `rules::ValidationReport`. `rules_dir` points at a directory of
/// `*.object.toml` descriptors; if it is empty/missing, the report is all
/// zeros (nothing to check against).
/// Audit options: how the rules engine queries the world. `height_at` may be
/// provided by the caller (e.g. reading the baked heightfield) so the audit
/// matches what the bake actually produced; when `None` the natural terrain
/// function is used (an approximation, not the carved heightfield).
#[derive(Default)]
pub struct AuditOptions<'a> {
    /// Ground height in metres at (x, z). If None, natural (uncarved) terrain
    /// is used.
    pub height_at: Option<&'a dyn Fn(i32, i32) -> f32>,
    /// Half-extents used for footprint-level slope sampling. When None, the
    /// entity's own width/depth are used (falling back to 8m).
    pub slope_half: Option<(i32, i32)>,
}

/// Audit already-generated entities against the declarative rules engine.
/// Returns a `ValidationReport`, or an error if the rules directory cannot be
/// loaded (missing / malformed / duplicate descriptors) — a missing rules dir
/// must NOT silently produce an all-zero "clean" report.
pub fn audit_scene(
    seed: u64,
    scene: &SceneSpec,
    _landmarks: &[LandmarkSpec],
    entities: &[EntityInstance],
    water: WaterGeometry,
    rules_dir: &Path,
    opts: AuditOptions<'_>,
) -> Result<crate::rules::ValidationReport, crate::rules::RuleLoadError> {
    let registry = crate::rules::ObjectRegistry::load_dir(rules_dir)?;
    // Default height: natural (uncarved) terrain — an approximation; callers
    // that audit a baked world should pass the baked heightfield instead.
    let natural_height =
        |x: i32, z: i32| terrain_height_with_geometry(seed, x, z, water) as f32 / 4.;
    let height_at = opts.height_at.unwrap_or(&natural_height);
    let slope_half = opts.slope_half.unwrap_or((8, 8));
    let ctx = crate::rules::PlacementContext {
        height_at,
        water_level: &|x, z| {
            water
                .contains_point(f64::from(x), f64::from(z))
                .then_some(f32::MAX)
        },
        water_intersects: Some(&|fp| {
            water.intersects_axis_aligned(
                f64::from(fp.cx),
                f64::from(fp.cz),
                f64::from(fp.half_w),
                f64::from(fp.half_d),
            )
        }),
        slope_at: &|x, z| slope_from_height(height_at, x, z, slope_half.0, slope_half.1),
        slope_at_footprint: Some(&|x, z, hw, hd| {
            let (hw, hd) = opts.slope_half.unwrap_or((hw, hd));
            slope_from_height(height_at, x, z, hw, hd)
        }),
        bounds: (
            scene.origin_x,
            scene.origin_z,
            scene.origin_x + scene.width_m as i32,
            scene.origin_z + scene.depth_m as i32,
        ),
        grounding_tolerance: 0.5,
        // Biome from the same humidity field the surface classification uses.
        biome_at: Some(&|x, z| {
            let h = humidity_field(seed, x, z, water);
            if h > 0.55 {
                crate::rules::Biome::Forest
            } else if h < -0.2 {
                crate::rules::Biome::Desert
            } else {
                crate::rules::Biome::Grassland
            }
        }),
        // Hazard: very steep ground = cliff (reuse the slope query).
        hazard_at: Some(&|x, z| {
            let s = slope_from_height(height_at, x, z, slope_half.0, slope_half.1);
            if s > 30.0 {
                vec![crate::rules::HazardKind::Cliff]
            } else {
                Vec::new()
            }
        }),
    };
    Ok(crate::rules::audit_entities(
        entities, &registry, &ctx, seed,
    ))
}

/// Audit final baked data without repairing persisted entities. Only authored
/// landmarks absent from the index are converted from pre-bake intent. Both
/// bake and CLI must use this entry point so their coverage cannot drift.
pub fn audit_baked_scene(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    entities: &[EntityInstance],
    water: WaterGeometry,
    rules_dir: &Path,
    terrain: &BakedTerrain,
) -> WorldgenResult<crate::rules::ValidationReport> {
    if !terrain.matches_scene(scene) {
        return Err(TerrainError("heightfield does not match scene".into()).into());
    }
    terrain.require_complete()?;
    let mut entity_ids = std::collections::BTreeSet::new();
    for entity in entities {
        if entity.entity_id.trim().is_empty()
            || !entity_ids.insert(&entity.entity_id)
            || ![
                entity.width_m,
                entity.depth_m,
                entity.height_m,
                entity.scale,
            ]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
            || !entity.rotation_y_deg.is_finite()
            || entity.width_m > 100_000.
            || entity.depth_m > 100_000.
        {
            return Err(
                TerrainError(format!("invalid/duplicate entity {}", entity.entity_id)).into(),
            );
        }
        entity.grounding.validate(&entity.entity_id)?;
    }
    let mut audit_entities = entities.to_vec();
    for landmark in landmarks {
        if !audit_entities
            .iter()
            .any(|e| e.entity_id == landmark.entity_id)
        {
            let mut authored = landmark_to_entity(landmark);
            rebase_entities_to_baked_surface(std::slice::from_mut(&mut authored), terrain);
            audit_entities.push(authored);
        }
    }
    // Water landmarks may extend beyond the cropped scene. Land entities may
    // not rely on clamped lookup to hide a missing/out-of-scene centre.
    for entity in &audit_entities {
        if !matches!(entity.kind.as_str(), "lake" | "river")
            && terrain.height(entity.world_x, entity.world_z).is_none()
        {
            return Err(TerrainError(format!(
                "entity {} has no baked centre sample",
                entity.entity_id
            ))
            .into());
        }
    }
    let height_at = |x, z| {
        terrain
            .height_clamped(x, z)
            .expect("complete baked terrain")
    };
    audit_scene(
        seed,
        scene,
        landmarks,
        &audit_entities,
        water,
        rules_dir,
        AuditOptions {
            height_at: Some(&height_at),
            slope_half: None,
        },
    )
    .map_err(|error| WorldgenError::InvalidRules(error.to_string()))
}

/// Return the highest natural/banked terrain sample under an object's footprint.
///
/// This deliberately samples every integer heightfield cell covered by the
/// footprint rather than only the centre and four corners.  The renderer puts
/// the building bottom at `world_y`, so a missed interior peak would still
/// make the baked terrain intersect the house even though the corners looked
/// grounded.  A lot can receive fill under its low side, but map generation
/// must not excavate the high side merely to match the centre sample or the
/// water datum.
fn footprint_surface_ceiling_m(seed: u64, entity: &EntityInstance, water: WaterGeometry) -> i32 {
    let (half_w, half_d) =
        spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
    let half_w = half_w.ceil().max(0.0) as i32;
    let half_d = half_d.ceil().max(0.0) as i32;
    let mut ceiling_m = f64::NEG_INFINITY;
    for dz in -half_d..=half_d {
        for dx in -half_w..=half_w {
            let sample_m = f64::from(terrain_height_with_geometry(
                seed,
                entity.world_x + dx,
                entity.world_z + dz,
                water,
            )) / 4.0;
            ceiling_m = ceiling_m.max(sample_m);
        }
    }
    ceiling_m.ceil() as i32
}

/// Rebase volume-bearing buildings onto the highest terrain sample inside their
/// footprint before terrain pads are baked. Pads may add fill under the rest of
/// the lot, but may never dig the landscape down to sea level or to a low centre
/// sample. This establishes the data-side grounding contract; renderers only
/// consume the resulting world_y/heightfield pair.
fn is_grounded_entity_kind(kind: &str) -> bool {
    is_building_kind(kind)
        || matches!(
            kind,
            "tree"
                | "rock"
                | "bush"
                | "grass_clump"
                | "fallen_log"
                | "fence"
                | "guardrail"
                | "boardwalk"
                | "scenic_belt"
                | "mudflat"
                | "mangrove"
        )
}

fn rebase_buildings_to_surface(seed: u64, water: WaterGeometry, entities: &mut [EntityInstance]) {
    for entity in entities {
        // Buildings and natural details use the same highest-footprint sample
        // contract. Roads/bridges/water keep their own roadbed/deck semantics.
        if is_grounded_entity_kind(&entity.kind) && entity.grounding.frontage_road_point.is_none() {
            entity.world_y = entity
                .world_y
                .max(footprint_surface_ceiling_m(seed, entity, water));
        }
    }
}

/// Keep generated road/sidewalk anchors at or above the terrain that will be
/// baked at their centre.  Roads are rendered as terrain-following strips, but
/// their `world_y` is still the baseline used by adapters and by scale-audit.
/// A neighbouring building pad can raise that centre after the road's initial
/// terrain sample, so account for overlapping building pads before planning
/// the final carves. Authored/GIS roads are left untouched because their
/// elevation may intentionally describe a viaduct or bridge approach.
fn rebase_grounded_entities_to_carved_surface(
    seed: u64,
    water: WaterGeometry,
    entities: &mut [EntityInstance],
    carves: &[TerrainCarve],
) {
    for entity in entities
        .iter_mut()
        .filter(|entity| is_grounded_entity_kind(&entity.kind) && entity.grounding.frontage_road_point.is_none())
    {
        let (half_w, half_d) =
            spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
        let mut ceiling = f64::NEG_INFINITY;
        for dz in -(half_d.ceil() as i32)..=(half_d.ceil() as i32) {
            for dx in -(half_w.ceil() as i32)..=(half_w.ceil() as i32) {
                let sample = f64::from(terrain_height_with_geometry_carved(
                    seed,
                    entity.world_x + dx,
                    entity.world_z + dz,
                    water,
                    carves,
                )) / 4.0;
                ceiling = ceiling.max(sample);
            }
        }
        let anchor_offset = entity.ground_height_m() - entity.world_y as f32;
        entity.world_y = entity
            .world_y
            .max((ceiling - f64::from(anchor_offset)).ceil() as i32);
    }
}

fn rebase_generated_surfaces_to_ground(
    seed: u64,
    water: WaterGeometry,
    entities: &mut [EntityInstance],
) {
    let building_supports: Vec<(i32, i32, f64, f64, f32)> = entities
        .iter()
        .filter(|entity| is_building_kind(&entity.kind))
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0 + 1.0,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1 + 1.0,
                entity.ground_height_m(),
            )
        })
        .collect();

    for entity in entities.iter_mut().filter(|entity| {
        entity.entity_id.starts_with("generated.")
            && matches!(entity.kind.as_str(), "road" | "sidewalk" | "boardwalk")
    }) {
        let natural_m = f64::from(terrain_height_with_geometry(
            seed,
            entity.world_x,
            entity.world_z,
            water,
        )) / 4.0;
        let mut ground_m = natural_m;
        for (bx, bz, half_w, half_d, building_ground_m) in &building_supports {
            if f64::from((entity.world_x - bx).abs()) <= *half_w
                && f64::from((entity.world_z - bz).abs()) <= *half_d
            {
                ground_m = ground_m.max(f64::from(*building_ground_m));
            }
        }
        entity.world_y = entity.world_y.max(ground_m.ceil() as i32).max(1);
    }
}

/// Reconcile unplanned grounded entities against the authoritative baked heightfield.
/// Explicit frontage platforms remain fixed: audit a mismatch, never ceil/rebase
/// the source obligation to conceal it.
/// Post-bake CLI audits use this same contract as `bake_world`.
pub fn rebase_entities_to_baked_surface(
    entities: &mut [EntityInstance],
    baked_samples: &BakedTerrain,
) {
    for entity in entities
        .iter_mut()
        .filter(|entity| is_grounded_entity_kind(&entity.kind) && entity.grounding.frontage_road_point.is_none())
    {
        let (half_w, half_d) =
            spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
        let mut ceiling = f32::NEG_INFINITY;
        for dz in -(half_d.ceil() as i32)..=(half_d.ceil() as i32) {
            for dx in -(half_w.ceil() as i32)..=(half_w.ceil() as i32) {
                if let Some(height) = baked_samples.height(entity.world_x + dx, entity.world_z + dz)
                {
                    ceiling = ceiling.max(height);
                }
            }
        }
        if ceiling.is_finite() {
            let offset = entity.ground_height_m() - entity.world_y as f32;
            entity.world_y = entity.world_y.max((ceiling - offset).ceil() as i32);
        }
    }
}

/// Generation-only baseline reconciliation for terrain-following transport.
/// Keep authored elevations fixed, and never feed this metadata update back
/// into another carve pass (that would cause an ever-rising fill feedback loop).
fn rebase_generated_transport_to_baked_surface(
    entities: &mut [EntityInstance],
    landmarks: &[LandmarkSpec],
    terrain: &BakedTerrain,
    water: Option<WaterGeometry>,
) {
    for entity in entities.iter_mut().filter(|e| {
        e.entity_id.starts_with("generated.")
            && matches!(e.kind.as_str(), "road" | "sidewalk")
            // B01-T2 R11 G2: a PROFILED road already carries its authoritative
            // longitudinal surface - `world_y` is the profile's floor and
            // `bottom_offset_m` its fraction, which is exactly what
            // `RoadProfile::validate_entity` requires. Re-deriving the anchor
            // from the raster is both unnecessary (the road's own carve, at
            // priority 10, is what places the centre cell on the profile) and
            // wrong at the rounding boundary: when the baked centre sample
            // equals the profile, `(h - offset).ceil()` lands one metre high
            // whenever f32 rounds the exact integer upward, which is how
            // `generated.road-network.scene-0.129-130` and
            // `generated.road-auto.scene-0.5` came to fail the scale audit.
            // A genuinely buried profiled road is still caught, by name, by
            // the audit's own below-ground clause.
            && e.grounding.road_profile.is_none()
            && !landmarks.iter().any(|l| l.entity_id == e.entity_id)
            // A registered span owns its plan-authored deck level even when its
            // centre station is on the dry bank. Re-grounding by the centre
            // sample moved such bridges after planning and made the road audit
            // report a zero-error "changed after planning" failure.
            && !e.grounding.structure_deck
            // Opt-in waterfront: a road whose centre cell is inside the water
            // body is a structure deck. Its published `world_y` IS its deck level
            // (the plan's chosen level and the audit's `deckLevelMm`); re-deriving
            // it from the raster would sink the deck to the natural BED and break
            // the plan/entity agreement the audit checks. No existing input
            // reaches this: without the opt-in no road plan can place a road over
            // water at all.
            && !water.is_some_and(|water| {
                water.road_water_crossings()
                    && water.contains_point(f64::from(e.world_x), f64::from(e.world_z))
            })
    }) {
        if let Some(h) = terrain.height(entity.world_x, entity.world_z) {
            let offset = entity.ground_height_m() - entity.world_y as f32;
            entity.world_y = (h - offset).ceil() as i32;
        }
    }
}

fn sanitize_generated_transport_collisions(entities: &mut Vec<EntityInstance>) {
    let buildings: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| is_building_kind(&entity.kind))
        .map(|entity| {
            let (half_w, half_d) =
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
            (entity.world_x, entity.world_z, half_w + 4.0, half_d + 4.0)
        })
        .collect();
    entities.retain(|entity| {
        if !entity.entity_id.starts_with("generated.") || entity.kind != "sidewalk" {
            return true;
        }
        let (half_w, half_d) =
            spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
        !buildings.iter().any(|(bx, bz, bhw, bhd)| {
            f64::from((entity.world_x - bx).abs()) < half_w + bhw
                && f64::from((entity.world_z - bz).abs()) < half_d + bhd
        })
    });
}

fn sanitize_natural_detail_clearance(entities: &mut Vec<EntityInstance>) {
    let road_footprints: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| matches!(entity.kind.as_str(), "road" | "sidewalk" | "boardwalk"))
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0 + 6.0,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1 + 6.0,
            )
        })
        .collect();
    let building_footprints: Vec<(i32, i32, f64, f64)> = entities
        .iter()
        .filter(|entity| is_building_kind(&entity.kind))
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).0 + 0.5,
                spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg).1 + 0.5,
            )
        })
        .collect();
    entities.retain(|entity| {
        if !matches!(
            entity.kind.as_str(),
            "tree" | "bush" | "grass_clump" | "rock" | "fallen_log"
        ) {
            return true;
        }
        let detail_r = f64::from(entity.width_m.max(entity.depth_m)) * 0.5;
        let overlaps_road = road_footprints.iter().any(|(rx, rz, rhx, rhz)| {
            f64::from((entity.world_x - rx).abs()) < detail_r + rhx
                && f64::from((entity.world_z - rz).abs()) < detail_r + rhz
        });
        if overlaps_road {
            return false;
        }
        !building_footprints.iter().any(|(bx, bz, bhx, bhz)| {
            f64::from((entity.world_x - bx).abs()) < detail_r + bhx
                && f64::from((entity.world_z - bz).abs()) < detail_r + bhz
        })
    });
}

fn generation_settlement(style: &serde_json::Value) -> Option<&str> {
    style
        .get("generationIntent")
        .and_then(|intent| intent.get("settlement"))
        .and_then(serde_json::Value::as_str)
}

fn generated_intent_entity(
    id: String,
    kind: &str,
    x: i32,
    z: i32,
    y: i32,
    width_m: f32,
    depth_m: f32,
    height_m: f32,
) -> EntityInstance {
    EntityInstance {
        rotation_y_deg: if id.contains("road-ns") { 90.0 } else { 0.0 },
        grounding: GroundingSpec::default(),
        anchors: Vec::new(),
        bounds: None,
        entity_id: id,
        asset_id: format!("prop.{kind}"),
        kind: kind.into(),
        world_x: x,
        world_z: z,
        world_y: y,
        scale: 1.0,
        width_m,
        depth_m,
        height_m,
    }
}

pub(crate) fn spatial_half_extents(width_m: f32, depth_m: f32, rotation_y_deg: f32) -> (f64, f64) {
    let quarter_turn = (rotation_y_deg.round() as i32).rem_euclid(180) == 90;
    if quarter_turn {
        (f64::from(depth_m) * 0.5, f64::from(width_m) * 0.5)
    } else {
        (f64::from(width_m) * 0.5, f64::from(depth_m) * 0.5)
    }
}

fn is_building_kind(kind: &str) -> bool {
    matches!(
        kind,
        "building"
            | "building_tower"
            | "building_cluster"
            | "urban_building"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "resort_lodge"
            | "storefront"
            | "commercial_center"
            | "entertainment_center"
            | "school"
            | "town_hall"
            | "market"
            | "industrial"
            | "temple"
            | "church"
            | "parking_lot"
            | "green_space"
            | "shopping_mall"
            | "office_tower"
            | "convenience_store"
            | "night_market"
            | "community_plaza"
    )
}

fn intent_candidate_is_free(
    scene: &SceneSpec,
    water: WaterGeometry,
    entities: &[EntityInstance],
    x: i32,
    z: i32,
    width_m: f32,
    depth_m: f32,
    kind: &str,
) -> bool {
    let half_w = width_m as f64 * 0.5;
    let half_d = depth_m as f64 * 0.5;
    if x as f64 - half_w < scene.origin_x as f64
        || x as f64 + half_w >= (scene.origin_x + scene.width_m as i32) as f64
        || z as f64 - half_d < scene.origin_z as f64
        || z as f64 + half_d >= (scene.origin_z + scene.depth_m as i32) as f64
    {
        return false;
    }
    let candidate = EntityInstance {
        rotation_y_deg: 0.0,
        grounding: GroundingSpec::default(),
        anchors: Vec::new(),
        bounds: None,
        entity_id: "candidate".into(),
        asset_id: "candidate".into(),
        kind: kind.into(),
        world_x: x,
        world_z: z,
        world_y: 0,
        scale: 1.0,
        width_m,
        depth_m,
        height_m: 1.0,
    };
    if !keeps_shore_clearance(&candidate, water) {
        return false;
    }
    if !is_building_kind(kind) {
        return true;
    }
    !entities
        .iter()
        .filter(|entity| is_building_kind(&entity.kind))
        .any(|entity| {
            ((entity.world_x - x).abs() as f64) < entity.width_m as f64 * 0.5 + half_w
                && ((entity.world_z - z).abs() as f64) < entity.depth_m as f64 * 0.5 + half_d
        })
}

/// Generic settlement planner for confirmed city/town intents beside a river
/// or lake. It derives cores from WaterGeometry and scene bounds; it never
/// reads a scenario name or a fixture-specific coordinate.
fn append_waterfront_intent_settlement(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    entities: &mut Vec<EntityInstance>,
    style: &serde_json::Value,
    ids: &CandidateIds,
) {
    let Some(settlement) = generation_settlement(style) else {
        return;
    };
    if !matches!(settlement, "city" | "town") || water.kind == WaterKind::None {
        return;
    }
    let profile = LandUseProfile::from_style(style).unwrap_or_else(LandUseProfile::default_demo);
    let density = (profile.urban_target() + 0.15).clamp(0.25, 0.75);
    let inset = 110_i32;
    let mut cores: Vec<(i32, i32, bool)> = match water.kind {
        WaterKind::River => {
            let shore = water.half_width.ceil() as i32 + inset;
            vec![
                (
                    water.center_x.round() as i32 - shore,
                    scene.origin_z + scene.depth_m as i32 / 3,
                    false,
                ),
                (
                    water.center_x.round() as i32 + shore,
                    scene.origin_z + scene.depth_m as i32 * 2 / 3,
                    true,
                ),
            ]
        }
        WaterKind::Lake => {
            let shore = water.half_depth.ceil() as i32 + inset;
            vec![
                (
                    scene.origin_x + scene.width_m as i32 / 2,
                    water.center_z.round() as i32 - shore,
                    false,
                ),
                (
                    scene.origin_x + scene.width_m as i32 / 2,
                    water.center_z.round() as i32 + shore,
                    true,
                ),
            ]
        }
        WaterKind::None => Vec::new(),
    };
    cores.retain(|(x, z, _)| {
        intent_candidate_is_free(
            scene,
            water,
            entities,
            *x,
            *z,
            80.0,
            60.0,
            "commercial_center",
        )
    });
    for (core_x, core_z, positive_bank) in cores {
        let road_is_vertical = water.kind == WaterKind::River;
        let (road_w, road_d, road_id) = if road_is_vertical {
            (
                (scene.depth_m.saturating_sub(80)) as f32,
                10.0,
                ids.claim(
                    "intent-road-ns",
                    "road",
                    &[i64::from(core_x), i64::from(core_z)],
                ),
            )
        } else {
            (
                (scene.width_m.saturating_sub(80)) as f32,
                10.0,
                ids.claim(
                    "intent-road-ew",
                    "road",
                    &[i64::from(core_x), i64::from(core_z)],
                ),
            )
        };
        let road_y =
            (i32::from(terrain_height_with_geometry(seed, core_x, core_z, water)) / 4).max(1);
        entities.push(generated_intent_entity(
            road_id, "road", core_x, core_z, road_y, road_w, road_d, 0.24,
        ));
        // Civic parcels sit beside the access road, never on its centreline.
        // The side is derived from the bank, so this remains generic for any
        // river/lake geometry instead of depending on fixture coordinates.
        let bank_sign = if positive_bank { 1 } else { -1 };
        let buildings = if road_is_vertical {
            [
                ("commercial_center", 80.0, 60.0, 18.0, bank_sign * 75, 0),
                ("school", 60.0, 48.0, 10.0, bank_sign * 180, 20),
                ("green_space", 30.0, 30.0, 0.2, bank_sign * 75, 100),
            ]
        } else {
            [
                ("commercial_center", 80.0, 60.0, 18.0, 0, bank_sign * 75),
                ("school", 60.0, 48.0, 10.0, 20, bank_sign * 180),
                ("green_space", 30.0, 30.0, 0.2, 100, bank_sign * 75),
            ]
        };
        for (offset, (kind, width, depth, height, ox, oz)) in buildings.into_iter().enumerate() {
            let x = core_x + ox;
            let z = core_z + oz;
            if !intent_candidate_is_free(scene, water, entities, x, z, width, depth, kind) {
                continue;
            }
            let y = (i32::from(terrain_height_with_geometry(seed, x, z, water)) / 4).max(1);
            entities.push(generated_intent_entity(
                ids.claim(&format!("intent-{kind}"), kind, &[i64::from(core_x), i64::from(core_z), offset as i64]),
                kind,
                x,
                z,
                y,
                width,
                depth,
                height,
            ));
        }
        let homes = (density * 10.0).round() as i32;
        for row in 0..homes {
            let lateral = bank_sign * (150 + row * 36);
            let along = (row % 3 - 1) * 65;
            let (x, z) = if road_is_vertical {
                (core_x + lateral, core_z + along)
            } else {
                (core_x + along, core_z + lateral)
            };
            if !intent_candidate_is_free(
                scene,
                water,
                entities,
                x,
                z,
                30.0,
                24.0,
                "residential_block",
            ) {
                continue;
            }
            let y = (i32::from(terrain_height_with_geometry(seed, x, z, water)) / 4).max(1);
            entities.push(generated_intent_entity(
                ids.claim("intent-residential", "residential_block", &[i64::from(core_x), i64::from(core_z), i64::from(row)]),
                "residential_block",
                x,
                z,
                y,
                30.0,
                24.0,
                12.0,
            ));
        }
    }
}

/// Generic pastoral planner. Unlike wilderness it deliberately creates rural
/// land-use, access and service objects; it is driven by SettlementIntent,
/// not by a named steppe fixture.
fn append_pastoral_intent_content(
    seed: u64,
    scene: &SceneSpec,
    entities: &mut Vec<EntityInstance>,
    ids: &CandidateIds,
) {
    let land = WaterGeometry {
        kind: WaterKind::None,
        center_x: 0.0,
        center_z: 0.0,
        half_width: 0.0,
        half_depth: 0.0,
        scene_width_m: scene.width_m,
        scene_depth_m: scene.depth_m,
        terrain_origin_x: scene.origin_x,
        terrain_origin_z: scene.origin_z,
        smooth_rolling: false,
        road_water_crossings: false,
        geography: natural_geography::GeographyControls::default(),
    };
    let mut serial = 750_000_u32;
    for z in (scene.origin_z + 140..scene.origin_z + scene.depth_m as i32 - 140).step_by(180) {
        for x in (scene.origin_x + 140..scene.origin_x + scene.width_m as i32 - 140).step_by(180) {
            let kind = if signed_noise(seed.rotate_left(301), x, z) > 0 {
                "pasture"
            } else {
                "farmland"
            };
            if intent_candidate_is_free(scene, land, entities, x, z, 100.0, 80.0, kind) {
                let y = i32::from(terrain_height(
                    seed,
                    x,
                    z,
                    WaterKind::None,
                    scene.width_m,
                    scene.depth_m,
                    0.0,
                )) / 4;
                entities.push(generated_intent_entity(
                    ids.claim("intent-rural", kind, &[i64::from(x), i64::from(z)]),
                    kind,
                    x,
                    z,
                    y,
                    100.0,
                    80.0,
                    0.2,
                ));
                if serial % 3 == 0 {
                    let well_x = x + 35;
                    let well_z = z + 25;
                    if intent_candidate_is_free(
                        scene,
                        land,
                        entities,
                        well_x,
                        well_z,
                        5.0,
                        5.0,
                        "water_well",
                    ) {
                        entities.push(generated_intent_entity(
                            ids.claim("intent-well", "water_well", &[i64::from(x), i64::from(z)]),
                            "water_well",
                            well_x,
                            well_z,
                            y,
                            5.0,
                            5.0,
                            2.4,
                        ));
                    }
                    let home_x = x - 35;
                    let home_z = z - 20;
                    if intent_candidate_is_free(
                        scene,
                        land,
                        entities,
                        home_x,
                        home_z,
                        24.0,
                        24.0,
                        "residential_home",
                    ) {
                        entities.push(generated_intent_entity(
                            ids.claim("intent-home", "residential_home", &[i64::from(x), i64::from(z)]),
                            "residential_home",
                            home_x,
                            home_z,
                            y,
                            24.0,
                            24.0,
                            6.0,
                        ));
                    }
                }
            }
            serial += 1;
        }
    }
}

/// Natural-only filler for wild scenes (steppe / nature): sparse trees,
/// bushes, rocks and grass clumps over a deterministic grid, with jitter.
/// No roads, no buildings, no paths — just open wild land.
fn append_natural_content(
    seed: u64,
    scene: &SceneSpec,
    geometry: WaterGeometry,
    entities: &mut Vec<EntityInstance>,
    ids: &CandidateIds,
) {
    for z in (20..scene.depth_m.saturating_sub(8)).step_by(16) {
        for x in (20..scene.width_m.saturating_sub(8)).step_by(16) {
            let jx = signed_noise(seed.rotate_left(211), x as i32, z as i32);
            let jz = signed_noise(seed.rotate_left(223), x as i32, z as i32);
            let wx = scene.origin_x + x as i32 + (jx.rem_euclid(19) - 9);
            let wz = scene.origin_z + z as i32 + (jz.rem_euclid(19) - 9);
            let world_y = i32::from(terrain_height_with_geometry(seed, wx, wz, geometry)) / 4;
            let roll = signed_noise(seed.rotate_left(239), wx, wz);
            // Steppe: grass dominant, sparse trees, occasional rocks/bushes.
            let kind = if roll < -84 {
                "tree"
            } else if roll < -52 {
                "bush"
            } else if roll > 88 {
                "rock"
            } else {
                "grass_clump"
            };
            let w = match kind {
                "tree" => 2.5 + (roll.rem_euclid(20) as f32) / 10.0,
                "bush" => 1.2 + (roll.rem_euclid(14) as f32) / 10.0,
                "rock" => 0.8 + (roll.rem_euclid(18) as f32) / 10.0,
                _ => 1.0 + (roll.rem_euclid(16) as f32) / 10.0,
            };
            let h = match kind {
                "tree" => 4.0 + (roll.rem_euclid(26) as f32) / 10.0,
                "bush" => 0.6 + (roll.rem_euclid(14) as f32) / 10.0,
                "rock" => 0.5 + (roll.rem_euclid(18) as f32) / 10.0,
                _ => 0.5 + (roll.rem_euclid(12) as f32) / 10.0,
            };
            entities.push(EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: ids.claim("natural", kind, &[i64::from(x), i64::from(z)]),
                asset_id: format!("prop.{kind}"),
                kind: kind.to_owned(),
                world_x: wx,
                world_z: wz,
                world_y,
                scale: 1.0,
                width_m: w,
                depth_m: w,
                height_m: h,
            });
        }
    }
}

fn append_generic_land_content(seed: u64, scene: &SceneSpec, entities: &mut Vec<EntityInstance>, ids: &CandidateIds) {
    let center_x = scene.origin_x + (scene.width_m / 2) as i32;
    let center_z = scene.origin_z + (scene.depth_m / 2) as i32;
    let road_y = i32::from(terrain_height(
        seed,
        center_x,
        center_z,
        WaterKind::None,
        scene.width_m,
        scene.depth_m,
        0.0,
    )) / 4;
    entities.push(EntityInstance {
        rotation_y_deg: 0.0,
        grounding: GroundingSpec::default(),
        anchors: Vec::new(),
        bounds: None,
        entity_id: "generated.auto-road-east-west".to_owned(),
        asset_id: "prop.road".to_owned(),
        kind: "road".to_owned(),
        world_x: center_x,
        world_z: center_z,
        world_y: road_y,
        scale: 1.0,
        width_m: (scene.width_m.saturating_sub(48)) as f32,
        depth_m: 12.0,
        height_m: 0.24,
    });
    entities.push(EntityInstance {
        rotation_y_deg: 0.0,
        grounding: GroundingSpec::default(),
        anchors: Vec::new(),
        bounds: None,
        entity_id: "generated.auto-road-north-south".to_owned(),
        asset_id: "prop.road".to_owned(),
        kind: "road".to_owned(),
        world_x: center_x,
        world_z: center_z,
        world_y: road_y,
        scale: 1.0,
        width_m: (scene.depth_m.saturating_sub(48)) as f32,
        depth_m: 12.0,
        height_m: 0.24,
    });
    for z in (scene.origin_z + 40..scene.origin_z + scene.depth_m as i32 - 40).step_by(40) {
        for x in (scene.origin_x + 40..scene.origin_x + scene.width_m as i32 - 40).step_by(40) {
            if (x - center_x).abs() < 30 || (z - center_z).abs() < 30 {
                continue;
            }
            let roll = signed_noise(seed.rotate_left(151), x, z);
            let (kind, asset_id, width, depth, height) = if roll > 42 {
                (
                    "building",
                    if roll.rem_euclid(3) == 0 {
                        "prop.building.small"
                    } else if roll.rem_euclid(3) == 1 {
                        "prop.building.wide"
                    } else {
                        "prop.building.round"
                    },
                    18.0,
                    16.0,
                    8.0 + roll.rem_euclid(10) as f32,
                )
            } else if roll < -65 {
                (
                    "tree",
                    "prop.tree",
                    3.0 + roll.rem_euclid(20) as f32 / 10.0,
                    3.0 + roll.rem_euclid(20) as f32 / 10.0,
                    4.5 + roll.rem_euclid(35) as f32 / 10.0,
                )
            } else {
                ("bush", "prop.bush", 1.8, 1.8, 1.2)
            };
            let world_y = i32::from(terrain_height(
                seed,
                x,
                z,
                WaterKind::None,
                scene.width_m,
                scene.depth_m,
                0.0,
            )) / 4;
            entities.push(EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: ids.claim(&format!("auto-{kind}"), kind, &[i64::from(x), i64::from(z)]),
                asset_id: asset_id.to_owned(),
                kind: kind.to_owned(),
                world_x: x,
                world_z: z,
                world_y,
                scale: 1.0,
                width_m: width,
                depth_m: depth,
                height_m: height,
            });
        }
    }
}

fn projected_road_corridor_intersects(
    params: &CityFormParams,
    scene: &SceneSpec,
    x: i32,
    z: i32,
    half_w: i32,
    half_d: i32,
) -> bool {
    let center_x = scene.origin_x + (scene.width_m / 2) as i32;
    let center_z = scene.origin_z + (scene.depth_m / 2) as i32;
    let min_x = scene.origin_x + 80;
    let max_x = scene.origin_x + scene.width_m as i32 - 80;
    let min_z = scene.origin_z + 80;
    let max_z = scene.origin_z + scene.depth_m as i32 - 80;
    let hit_ew = |road_z: i32, cross: i32| {
        (z - road_z).abs() <= half_d + cross && x + half_w >= min_x && x - half_w <= max_x
    };
    let hit_ns = |road_x: i32, cross: i32| {
        (x - road_x).abs() <= half_w + cross && z + half_d >= min_z && z - half_d <= max_z
    };
    match params.road_grid {
        RoadGrid::Grid | RoadGrid::Loose => {
            if hit_ew(center_z, 6) || hit_ns(center_x, 6) {
                return true;
            }
            let spacing = if scene.width_m.max(scene.depth_m) >= 5_000 {
                params.road_spacing_m.min(120).max(32) as usize
            } else {
                params.road_spacing_m.max(32) as usize
            };
            for road_z in (min_z..max_z).step_by(spacing) {
                if (road_z - center_z).abs() >= 24 && hit_ew(road_z, 5) {
                    return true;
                }
            }
            for road_x in (min_x..max_x).step_by(spacing) {
                if (road_x - center_x).abs() >= 24 && hit_ns(road_x, 5) {
                    return true;
                }
            }
        }
        RoadGrid::RiverAxis | RoadGrid::ValleyAxis => {
            if hit_ew(center_z, 6) {
                return true;
            }
            let spacing = if scene.width_m.max(scene.depth_m) >= 5_000 {
                params.road_spacing_m.min(120).max(32) as usize
            } else {
                params.road_spacing_m.max(32) as usize
            };
            for road_x in (min_x..max_x).step_by(spacing) {
                if (road_x - center_x).abs() >= 120 && hit_ns(road_x, 5) {
                    return true;
                }
            }
            for road_z in (min_z..max_z).step_by(spacing.saturating_mul(2)) {
                if (road_z - center_z).abs() >= 120 && hit_ew(road_z, 5) {
                    return true;
                }
            }
        }
        RoadGrid::Radial => {
            // Radial rings are generated around the core; use the two axial
            // spines as a conservative reservation. Ring-specific collision
            // is handled by the final rules audit and bounded engineering.
            if hit_ew(center_z, 6) || hit_ns(center_x, 6) {
                return true;
            }
        }
    }
    false
}

fn append_modern_landuse_content(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
    entities: &mut Vec<EntityInstance>,
    style: &serde_json::Value,
    road_plan: Option<&road_network::RoadNetworkPlan>,
    development_plan: Option<&DevelopmentPlan>,
    development_proposals: &mut Vec<DevelopmentProposal>,
    ids: &CandidateIds,
    zones: Option<&[LandUseZone]>,
) {
    let params = CityFormParams::resolve(style);
    let min_x = scene.origin_x + 80;
    let max_x = scene.origin_x + scene.width_m as i32 - 80;
    let min_z = scene.origin_z + 80;
    let max_z = scene.origin_z + scene.depth_m as i32 - 80;
    let scene_min_x = scene.origin_x;
    let scene_max_x = scene.origin_x + scene.width_m as i32;
    let scene_min_z = scene.origin_z;
    let scene_max_z = scene.origin_z + scene.depth_m as i32;
    let center_x = scene.origin_x + (scene.width_m / 2) as i32;
    let center_z = scene.origin_z + (scene.depth_m / 2) as i32;
    let urban_ratio = profile_ratio(style, "urbanCoreRatio", 0.22);
    let suburban_ratio = profile_ratio(style, "suburbanRatio", 0.28);
    let farm_ratio = profile_ratio(style, "farmRatio", 0.20);
    let pasture_ratio = profile_ratio(style, "pastureRatio", 0.12);
    let forest_ratio = profile_ratio(style, "forestRatio", 0.20);
    let green_ratio = profile_ratio(style, "greenRatio", 0.18);
    let reserve_ratio = profile_ratio(style, "reserveRatio", 0.10);
    let block_w = (52.0 * params.block_scale).clamp(20.0, 120.0);
    let block_d = (48.0 * params.block_scale).clamp(20.0, 120.0);
    let mut serial = 200_000_u32;
    // One occupancy grid for the whole scene keeps land-use coherent: roads
    // claim their strips first, buildings then avoid them, and vegetation /
    // greenspace only fills leftover soft ground. This prevents trees on
    // roads, houses cutting into hillsides, and lawns covering buildings.
    let planned_roads = road_plan.map(|p| p.entities()).unwrap_or_default();
    let mut occ = OccupancyGrid::new(scene.width_m, scene.depth_m);
    for road in &planned_roads {
        let (hw, hd) = spatial_half_extents(road.width_m, road.depth_m, road.rotation_y_deg);
        occ.mark(
            road.world_x,
            road.world_z,
            hw.ceil() as i32 + 3,
            hd.ceil() as i32 + 3,
            OccLayer::Hard,
        );
    }
    let planned_sites = development_plan
        .map(DevelopmentPlan::building_entities)
        .unwrap_or_default();
    for site in &planned_sites {
        let (hw, hd) = spatial_half_extents(site.width_m, site.depth_m, site.rotation_y_deg);
        occ.mark(
            site.world_x,
            site.world_z,
            hw as i32 + 2,
            hd as i32 + 2,
            OccLayer::Hard,
        );
    }
    entities.extend(planned_roads.iter().cloned());
    entities.extend(planned_sites.iter().cloned());
    // The bake hands its own land-use plan down; a caller that passes none keeps the historical
    // recomputation from the style, byte for byte.
    let computed;
    let source_zones: Option<&[LandUseZone]> = match (road_plan, zones) {
        (None, _) => None,
        (Some(_), Some(zones)) => Some(zones),
        (Some(_), None) => match LandUseProfile::from_style(style) {
            Some(profile) => {
                computed = plan_landuse_zones(scene, &profile);
                Some(&computed)
            }
            None => None,
        },
    };
    let source_zoning = source_zones
        .map(|z| zoning::ZoningMask::new(scene, z).expect("computed planning zones are valid"));
    // The scene's OWN water geometry, landmarks included. For a waterless scene
    // this is the same empty body the old landmark-free derivation produced, so
    // every existing world is byte-identical; for a waterfront road-first scene it
    // is the river the plan was actually designed against, so this generator's
    // water checks cannot read a different river than the planner did.
    let source_water = water;
    let terrain_context = TerrainContext {
        seed,
        water: source_water,
        precise: height_encoding_from_style(style).expect("validated height precision")
            == HeightEncoding::Millimetre,
        zoning: source_zoning.as_ref(),
    };
    // A planned parcel's fill envelope must not flood an already designed
    // roadside pad - the pads are flat surfaces whose top the road audit holds
    // exactly, so a lot that needs a higher platform buries its neighbour and the
    // pad then fails the baked audit. A PAD that would do exactly this is no
    // longer removed from the world here: the road plan refuses it by name
    // (`road_network::PAD_FILL_CONFLICT_CATEGORY`) before any conversion, parcel
    // or entity is derived from it, so every pair the plan kept is compatible by
    // the time generation runs and a silent `retain` over the published entities
    // has nothing left to delete - which is why it is gone. Land-use CANDIDATES
    // still ask the same question per candidate below (`pad_fill_conflicts`).
    let candidate_identity = std::cell::RefCell::new(None::<String>);
    // Optional planner-name override for fixed singleton placements that must
    // not share a stable-ID slot with a grid proposal of the same kind.
    let candidate_planner = std::cell::RefCell::new(None::<String>);
    let mut add = |kind: &str, x: i32, z: i32, width: f32, depth: f32, height: f32| -> bool {
        let planned_id = candidate_identity.borrow_mut().take();
        let planned_planner = candidate_planner.borrow_mut().take();
        // Claim the original proposal slot before constraints or engine movement.
        // Planned IDs remain authoritative. Transport kinds use planner names
        // that retain their direction suffix (`landuse-road-ns`), because
        // downstream constructors and access ordering still consume it.
        let optional_id = planned_id.is_none().then(|| {
            let planner = planned_planner.unwrap_or_else(|| format!("landuse-{kind}"));
            ids.claim(&planner, kind, &[i64::from(x), i64::from(z)])
        });
        if development_plan.is_some()
            && (crate::settlement::is_building(kind)
                || matches!(kind, "canal" | "railway" | "parking_lot"))
        {
            development_proposals.push(proposals::deferred(
                &scene.scene_id,
                development_proposals.len(),
                kind,
                x,
                z,
                width,
                depth,
                height,
            ));
            return false; // A request without a parcel cannot emit an entity.
        }
        // Agriculture now comes from verified landcover fields, never a
        // renderer-sized box that has no road/soil ownership contract.
        if development_plan.is_some() && matches!(kind, "farmland" | "pasture") {
            return false;
        }
        if road_plan.is_some() && (kind == "road-ew" || kind == "road-ns" || kind == "sidewalk") {
            return false;
        }
        if !planned_roads.is_empty()
            && occ.collides_hard(
                x,
                z,
                (width * 0.5).ceil() as i32,
                (depth * 0.5).ceil() as i32,
            )
        {
            return false;
        }
        let entity_kind = kind
            .strip_suffix("-ew")
            .or_else(|| kind.strip_suffix("-ns"))
            .unwrap_or(kind);
        let is_road = entity_kind == "road" || entity_kind == "sidewalk" || entity_kind == "canal";
        let is_building = matches!(
            entity_kind,
            "residential_block"
                | "residential_tower"
                | "residential_home"
                | "commercial_center"
                | "entertainment_center"
                | "school"
                | "town_hall"
                | "market"
                | "industrial"
                | "temple"
                | "church"
                | "building_cluster"
                | "storefront"
        );
        // Bulk land-use entities generated by the density loop. These are the
        // ones that must respect slopes and keep off roads; fixed civic
        // landmarks are placed deterministically and always kept.
        let is_bulk = matches!(
            entity_kind,
            "residential_block"
                | "residential_tower"
                | "residential_home"
                | "farmland"
                | "pasture"
                | "mountain_forest"
                | "tree"
                | "bush"
        );
        let is_soft = false;
        let _ = is_soft;
        // Roads are strips that span the full scene; they may touch the scene
        // edge, while buildings and land-use parcels stay inset. A road's
        // `width` is its running length (along the strip) and `depth` its
        // cross-section, so the boundary check only constrains the cross
        // direction and lets the length reach the scene edge.
        if scene.width_m.max(scene.depth_m) >= 5_000
            && is_building
            && projected_road_corridor_intersects(
                &params,
                scene,
                x,
                z,
                (width * 0.5).ceil() as i32,
                (depth * 0.5).ceil() as i32,
            )
        {
            // Roads are authored later in this planner, so reserve their
            // deterministic corridors before civic/land-use objects exist.
            // This is the road-first invariant without a second placement pass.
            return false;
        }
        if is_road {
            // For both EW and NS strips, `depth` is the cross-section width
            // and `width` is the running length. Only constrain the cross
            // direction to the scene; the length may reach the edge.
            let is_ns = kind.ends_with("-ns");
            let half_cross = (depth * 0.5).ceil() as i32;
            let inside_cross = if is_ns {
                x - half_cross >= scene_min_x
                    && x + half_cross <= scene_max_x
                    && z >= scene_min_z
                    && z <= scene_max_z
            } else {
                z - half_cross >= scene_min_z
                    && z + half_cross <= scene_max_z
                    && x >= scene_min_x
                    && x <= scene_max_x
            };
            if !inside_cross {
                return false;
            }
        } else {
            let half_width = (width * 0.5).ceil() as i32;
            let half_depth = (depth * 0.5).ceil() as i32;
            if x - half_width < min_x
                || x + half_width > max_x
                || z - half_depth < min_z
                || z + half_depth > max_z
            {
                return false;
            }
        }
        // Slope guard: bulk parcels and roads must not cut into a mountainside.
        // Fixed civic landmarks and small props tolerate their given spot.
        if is_bulk || is_road {
            let slope = local_slope(
                seed,
                x,
                z,
                scene.width_m,
                scene.depth_m,
                (width * 0.5).ceil() as i32,
                (depth * 0.5).ceil() as i32,
            );
            let max_slope = if is_road {
                // Keep the authored road contract strict. A road that cannot
                // be graded within 8% must be rejected before it is fixed.
                8.0
            } else if is_building {
                30.0
            } else if matches!(entity_kind, "mountain_forest" | "tree" | "bush") {
                70.0
            } else {
                30.0
            };
            if slope > max_slope {
                return false;
            }
        }
        // Collision guard against already-placed hard surfaces. Roads claim
        // their corridors first; buildings must not overlap roads or other
        // buildings; soft parcels avoid hard surfaces.
        let half_w = (width * 0.5).ceil() as i32;
        let half_d = (depth * 0.5).ceil() as i32;
        // The occupancy grid reserves roads and water channels as "hard"
        // corridors. Civic buildings are placed deterministically at the core
        // and must not be dropped, so they skip the collision check; bulk
        // parcels and vegetation must stay off roads and existing hard space.
        if is_road {
            // Roads claim corridors and may cross each other.
        } else if is_bulk && occ.collides_hard(x, z, half_w, half_d) {
            return false;
        }
        // Every building kind - not only the ones the density loop emits - is asked the same
        // question while the axis-and-node growth policy is on: its fill envelope must not bury a
        // planned pad, a planned parcel or a road. Off for every other input, so no existing world
        // gains or loses a single placement.
        let fill_guard = is_building
            || entity_kind == "parking_lot"
            || (urban_growth::enabled(style) && crate::settlement::is_building(entity_kind));
        if !planned_roads.is_empty() && fill_guard {
            let water = legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.);
            let mut proposed = generated_intent_entity(
                "generated.preflight".into(),
                entity_kind,
                x,
                z,
                0,
                width,
                depth,
                height,
            );
            proposed.world_y = footprint_surface_ceiling_m(seed, &proposed, water);
            proposed.normalize_spatial_semantics();
            // A roadside pad is an already designed flat surface, exactly like a
            // planned parcel: a higher building whose fill envelope floods the
            // pad would bury it in the final raster. Opt-in waterfront only, so
            // every existing world keeps the placements it already publishes.
            //
            // The axis-and-node growth policy opts in for the same reason: its pads do not inherit
            // the radial plan's order, so a land-use building can stand where no pad was before and
            // its fill envelope can bury a pad's declared flat top - which the bake then reports as
            // `bounded_cut_pad_top_not_flat` on a pad the frontage programme was right to accept.
            let planned_roadside = if source_water.road_water_crossings()
                || urban_growth::enabled(style)
            {
                road_plan.map(|plan| plan.site_entities()).unwrap_or_default()
            } else {
                Vec::new()
            };
            if road_network::building_fill_conflicts(&terrain_context, &planned_roads, &proposed)
                || road_network::building_fill_conflicts(
                    &terrain_context,
                    &planned_sites,
                    &proposed,
                )
                || road_network::pad_fill_conflicts(
                    &terrain_context,
                    &planned_roadside,
                    &proposed,
                )
            {
                return false;
            }
        }
        // Claim this footprint: roads reserve hard space; everything else is
        // soft and does not block neighbours.
        let layer = if is_road {
            OccLayer::Hard
        } else {
            OccLayer::Soft
        };
        occ.mark(x, z, half_w, half_d, layer);
        let world_y = i32::from(terrain_height(
            seed,
            x,
            z,
            WaterKind::None,
            scene.width_m,
            scene.depth_m,
            0.0,
        )) / 4;
        entities.push(EntityInstance {
            rotation_y_deg: if kind.ends_with("-ns") { 90.0 } else { 0.0 },
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: planned_id.or(optional_id).unwrap_or_else(|| format!("generated.landuse-{kind}.{serial}")),
            asset_id: format!("prop.{entity_kind}"),
            kind: entity_kind.to_owned(),
            world_x: x,
            world_z: z,
            world_y,
            scale: 1.0,
            width_m: width,
            depth_m: depth,
            height_m: height,
        });
        serial += 1;
        true
    };
    // Multi-core layout: place a full commercial/school cluster at the primary
    // core and lighter civic clusters at each secondary core so the road
    // network has a purpose instead of a single center.
    let cores = city_core_positions(&params, scene, seed);
    let primary = cores[0];
    add(
        "commercial_center",
        primary.0,
        primary.1 - 180,
        160.0,
        110.0,
        24.0,
    );
    add(
        "entertainment_center",
        primary.0 + 190,
        primary.1 - 20,
        130.0,
        100.0,
        18.0,
    );
    add("school", primary.0 - 190, primary.1 + 100, 80.0, 60.0, 10.0);
    add(
        "parking_lot",
        primary.0 + 100,
        primary.1 - 180,
        70.0,
        45.0,
        0.3,
    );
    add("temple", primary.0 - 300, primary.1 + 230, 34.0, 34.0, 14.0);
    add("church", primary.0 + 300, primary.1 + 230, 30.0, 42.0, 18.0);
    add(
        "green_space",
        primary.0 + 150,
        primary.1 + 150,
        30.0,
        30.0,
        0.2,
    );
    // Civic quarter: town hall anchors the administrative centre with a
    // market square beside it, as in a real urban core.
    add("town_hall", primary.0, primary.1 + 20, 88.0, 64.0, 26.0);
    add("market", primary.0 + 130, primary.1 + 40, 96.0, 72.0, 9.0);
    add("water_well", primary.0 - 60, primary.1 - 40, 5.0, 5.0, 2.4);
    // Industrial belt: placed on the far side of the ring road so factories
    // and workshops stay off the residential core but close to transport.
    let industrial_arm = match params.form {
        CityForm::RiverDelta => (scene.width_m as i32 / 5, scene.depth_m as i32 / 3),
        CityForm::CoastalBay => (scene.width_m as i32 / 5, scene.depth_m as i32 / 4),
        CityForm::MountainValley => (scene.width_m as i32 / 4, scene.depth_m as i32 / 3),
        _ => (scene.width_m as i32 / 4, scene.depth_m as i32 / 4),
    };
    for index in 0..2 {
        let ix = primary.0 - industrial_arm.0 + index * 150;
        let iz = primary.1 + industrial_arm.1 + index * 120;
        add(
            "industrial",
            ix,
            iz,
            120.0 + index as f32 * 40.0,
            80.0,
            18.0,
        );
        add("parking_lot", ix - 90, iz + 60, 46.0, 34.0, 0.3);
    }
    for (index, core) in cores.iter().enumerate().skip(1) {
        add("commercial_center", core.0, core.1 - 120, 110.0, 80.0, 18.0);
        add("school", core.0 - 110, core.1 + 70, 60.0, 50.0, 8.0);
        add("parking_lot", core.0 + 60, core.1 - 120, 48.0, 34.0, 0.3);
        add("green_space", core.0 + 90, core.1 + 90, 26.0, 26.0, 0.2);
        add("water_well", core.0 - 40, core.1 + 20, 5.0, 5.0, 2.4);
        let _ = index;
    }
    let canal_probability = params.canal_probability;
    let roll_canal = signed_noise(seed.rotate_left(191), center_x, center_z);
    let canal_offset = (scene.width_m / 4) as i32;
    // Temperate plains keep the historical always-on single canal so existing
    // tests and examples stay deterministic; other forms opt in via their
    // canal probability (river deltas and valleys get more canals).
    let always_canal = matches!(params.form, CityForm::TemperatePlain);
    let canal_trigger =
        always_canal || (roll_canal as f64).abs() / 128.0 < canal_probability + 0.15;
    if canal_trigger {
        add(
            "canal",
            scene.origin_x + canal_offset,
            center_z,
            12.0,
            (scene.depth_m.saturating_sub(160)) as f32,
            0.4,
        );
        if canal_probability > 0.4 || matches!(params.form, CityForm::RiverDelta) {
            add(
                "canal",
                scene.origin_x + scene.width_m as i32 - canal_offset,
                center_z,
                10.0,
                (scene.depth_m.saturating_sub(240)) as f32,
                0.4,
            );
        }
    }
    // Road grid depends on the city form. Grid forms keep a full orthogonal
    // network; radial forms emphasize rings around the primary core; valley /
    // river forms add an axial spine; loose suburban forms use wider spacing.
    match params.road_grid {
        RoadGrid::Grid | RoadGrid::Loose => {
            let spacing = if scene.width_m.max(scene.depth_m) >= 5_000 {
                params.road_spacing_m.min(120)
            } else {
                params.road_spacing_m
            };
            // Always lay a primary cross through the scene so every profile
            // keeps a connected core even on small test scenes.
            add(
                "road-ew",
                center_x,
                center_z,
                (scene.width_m.saturating_sub(160)) as f32,
                10.0,
                0.24,
            );
            add(
                "road-ns",
                center_x,
                center_z,
                (scene.depth_m.saturating_sub(160)) as f32,
                10.0,
                0.24,
            );
            for z in (min_z..max_z).step_by(spacing as usize) {
                if (z - center_z).abs() < 24 {
                    continue;
                }
                add(
                    "road-ew",
                    center_x,
                    z,
                    (scene.width_m.saturating_sub(160)) as f32,
                    9.0,
                    0.24,
                );
            }
            for x in (min_x..max_x).step_by(spacing as usize) {
                if (x - center_x).abs() < 24 {
                    continue;
                }
                add(
                    "road-ns",
                    x,
                    center_z,
                    (scene.depth_m.saturating_sub(160)) as f32,
                    9.0,
                    0.24,
                );
            }
        }
        RoadGrid::Radial => {
            let ring_radii = [
                scene.width_m.min(scene.depth_m) as f32 * 0.16,
                scene.width_m.min(scene.depth_m) as f32 * 0.30,
                scene.width_m.min(scene.depth_m) as f32 * 0.44,
            ];
            for (index, radius) in ring_radii.iter().enumerate() {
                let radius = *radius;
                let thickness = 8.0 - index as f32;
                add(
                    "road-ew",
                    primary.0,
                    primary.1 - radius as i32,
                    radius * 2.0,
                    thickness,
                    0.24,
                );
                add(
                    "road-ew",
                    primary.0,
                    primary.1 + radius as i32,
                    radius * 2.0,
                    thickness,
                    0.24,
                );
                add(
                    "road-ns",
                    primary.0 - radius as i32,
                    primary.1,
                    radius * 2.0,
                    thickness,
                    0.24,
                );
                add(
                    "road-ns",
                    primary.0 + radius as i32,
                    primary.1,
                    radius * 2.0,
                    thickness,
                    0.24,
                );
            }
            let spine_length = scene.depth_m.max(scene.width_m) as f32;
            add("road-ew", primary.0, primary.1, spine_length, 10.0, 0.24);
            add("road-ns", primary.0, primary.1, spine_length, 10.0, 0.24);
        }
        RoadGrid::RiverAxis | RoadGrid::ValleyAxis => {
            let spacing = if scene.width_m.max(scene.depth_m) >= 5_000 {
                params.road_spacing_m.min(120)
            } else {
                params.road_spacing_m
            };
            add(
                "road-ew",
                center_x,
                center_z,
                (scene.width_m.saturating_sub(160)) as f32,
                10.0,
                0.24,
            );
            for x in (min_x..max_x).step_by(spacing as usize) {
                if (x - center_x).abs() < 120 {
                    continue;
                }
                add(
                    "road-ns",
                    x,
                    center_z,
                    (scene.depth_m.saturating_sub(160)) as f32,
                    8.0,
                    0.24,
                );
            }
            for z in (min_z..max_z).step_by((spacing * 2) as usize) {
                if (z - center_z).abs() < 120 {
                    continue;
                }
                add(
                    "road-ew",
                    center_x,
                    z,
                    (scene.width_m.saturating_sub(160)) as f32,
                    8.0,
                    0.24,
                );
            }
        }
    }
    // Street furniture: lamps line the primary roads and a sign marks each
    // ring-road crossing so the street view reads as a real thoroughfare.
    let primary_road_z = center_z;
    let primary_road_x = center_x;
    // Sidewalks flank the primary roads so street view shows a real
    // pedestrian pavement between the carriageway and the buildings.
    for side in [-1_i32, 1_i32] {
        add(
            "sidewalk",
            center_x,
            center_z + side * 13,
            (scene.width_m.saturating_sub(160)) as f32,
            5.0,
            0.35,
        );
        add(
            "sidewalk",
            center_x + side * 13,
            center_z,
            (scene.depth_m.saturating_sub(160)) as f32,
            5.0,
            0.35,
        );
    }
    for offset in (min_z..max_z).step_by(180) {
        let roll = signed_noise(seed.rotate_left(211), center_x, offset);
        if roll < -20 {
            continue;
        }
        let lamp_x = center_x + 12;
        add("street_lamp", lamp_x, offset, 0.8, 0.8, 6.0);
        if (offset - primary_road_z).abs() < 160 {
            add("road_sign", center_x - 14, offset, 1.6, 0.9, 3.0);
        }
    }
    for offset in (min_x..max_x).step_by(180) {
        let roll = signed_noise(seed.rotate_left(217), offset, center_z);
        if roll < -20 {
            continue;
        }
        add("street_lamp", offset, center_z + 12, 0.8, 0.8, 6.0);
        if (offset - primary_road_x).abs() < 160 {
            add("road_sign", offset, center_z - 14, 1.6, 0.9, 3.0);
        }
    }
    let parcel_offset = if scene.width_m.max(scene.depth_m) >= 5_000 {
        60
    } else {
        0
    };
    let parcel_min_x = min_x + parcel_offset;
    let parcel_min_z = min_z + parcel_offset;
    let parcel_step = if scene.width_m.max(scene.depth_m) >= 5_000 {
        240
    } else {
        120
    };
    for z in (parcel_min_z..max_z).step_by(parcel_step) {
        for x in (parcel_min_x..max_x).step_by(parcel_step) {
            let roll = signed_noise(seed.rotate_left(163), x, z);
            let urban_scale = (urban_ratio + suburban_ratio).clamp(0.1, 1.0);
            let region = RegionType::classify(
                seed,
                x,
                z,
                WaterGeometry {
                    kind: WaterKind::None,
                    center_x: 0.0,
                    center_z: 0.0,
                    half_width: 0.0,
                    half_depth: 0.0,
                    scene_width_m: scene.width_m,
                    scene_depth_m: scene.depth_m,
                    terrain_origin_x: scene.origin_x,
                    terrain_origin_z: scene.origin_z,
                    smooth_rolling: false,
                    road_water_crossings: false,
                    geography: natural_geography::GeographyControls::default(),
                },
                urban_scale,
            );
            // Urbanisation is a global field, so the same region logic runs in
            // every chunk: dense core, mid-rise urban, detached suburban,
            // farmland on the plain, forest on the hills. No per-chunk random
            // scatter.
            match region {
                RegionType::UrbanCore => {
                    // Chinese-style gated compound: high-rise tower cluster
                    // around a shared court, denser than the rest of the map.
                    // Higher `core_density` forms fill more of the core block.
                    let fill_roll = (params.core_density * 100.0 - 55.0) as i32;
                    if roll > fill_roll {
                        let tower_w = (block_w * 0.9).clamp(24.0, 100.0);
                        let tower_d = (block_d * 0.9).clamp(24.0, 100.0);
                        let tower_h = 18.0 + roll.rem_euclid(26) as f32;
                        add("residential_tower", x, z, tower_w, tower_d, tower_h);
                    }
                    if roll.rem_euclid(3) == 0 {
                        add("parking_lot", x + 40, z + 30, 26.0, 20.0, 0.3);
                    }
                    if roll.rem_euclid(4) == 0 {
                        add("green_space", x - 34, z + 30, 24.0, 24.0, 0.2);
                    }
                    if roll.rem_euclid(12) == 0 {
                        add("street_lamp", x + 16, z - 16, 0.8, 0.8, 6.0);
                    }
                }
                RegionType::Urban => {
                    // Mid-rise urban blocks with retail ground floors.
                    let w = block_w;
                    let d = block_d;
                    let h = 9.0 + roll.rem_euclid(14) as f32;
                    add("residential_block", x, z, w, d, h);
                    if roll.rem_euclid(4) == 0 {
                        add("parking_lot", x + 32, z + 26, 26.0, 20.0, 0.3);
                    }
                    if roll.rem_euclid(100) < (green_ratio * 100.0) as i32 {
                        add("green_space", x - 28, z + 30, 26.0, 26.0, 0.2);
                    }
                }
                RegionType::Suburban => {
                    // American-style detached homes with yards: lower-rise,
                    // spread out, yards and street trees between lots.
                    // Higher `suburban_density` fills more lots.
                    let fill_roll = (params.suburban_density * 100.0 - 52.0) as i32;
                    let lot_w = (52.0 * params.block_scale * 1.2).clamp(30.0, 140.0);
                    let lot_d = (48.0 * params.block_scale * 1.2).clamp(30.0, 130.0);
                    if roll > fill_roll {
                        let h = 5.0 + roll.rem_euclid(7) as f32;
                        add("residential_home", x, z, lot_w, lot_d, h);
                    }
                    // Front yard
                    if roll.rem_euclid(2) == 0 {
                        add(
                            "green_space",
                            x,
                            z + (lot_d / 2.0 + 14.0) as i32,
                            22.0,
                            22.0,
                            0.2,
                        );
                    }
                    if roll.rem_euclid(3) == 0 {
                        add("tree", x + 18, z - 18, 3.0, 3.0, 6.0);
                    }
                }
                RegionType::Rural => {
                    // Fertile plain: farmland and pasture with the odd well.
                    if roll.rem_euclid(100) < (farm_ratio * 100.0) as i32 {
                        add("farmland", x, z, 92.0, 82.0, 0.2);
                    } else if roll.rem_euclid(100) < (pasture_ratio * 100.0) as i32 {
                        add("pasture", x, z, 100.0, 90.0, 0.2);
                    } else {
                        add("pasture", x, z, 100.0, 90.0, 0.2);
                    }
                    if roll.rem_euclid(60) == 0 {
                        add("water_well", x + 30, z + 30, 5.0, 5.0, 2.4);
                    }
                    if roll.rem_euclid(90) == 0 {
                        add("tree", x + 40, z + 40, 3.0, 3.0, 6.0);
                    }
                }
                RegionType::Forest | RegionType::Mountain => {
                    // Forest on hills, denser woodland on mountains.
                    let forest_p = if region == RegionType::Mountain {
                        (forest_ratio * 1.6).min(0.8)
                    } else {
                        forest_ratio
                    };
                    if roll.rem_euclid(100) < (forest_p * 100.0) as i32 {
                        add(
                            "mountain_forest",
                            x,
                            z,
                            100.0,
                            100.0,
                            12.0 + roll.rem_euclid(30) as f32,
                        );
                    } else if roll.rem_euclid(100) < (pasture_ratio * 100.0) as i32 {
                        add("pasture", x, z, 100.0, 90.0, 0.2);
                    } else {
                        add("tree", x, z, 3.0, 3.0, 5.0 + roll.rem_euclid(4) as f32);
                    }
                }
            }
        }
    }
    // Generate from the precomputed land-use zones. This is the first
    // generation-before-entities parcel pass: each accepted footprint consumes
    // a zone of the requested kind, while `add` remains the hard placement
    // gate for roads, slope, water and collision.
    let planning_profile = LandUseProfile {
        theme: params.form,
        urban_core_ratio: urban_ratio,
        suburban_ratio,
        green_ratio,
        farm_ratio,
        forest_ratio,
        pasture_ratio,
        reserve_ratio,
    };
    // Road-first scenes consume BuildingPlan; never generate zone-sized
    // building/green-space proxies from an unrelated legacy parcel layout.
    if development_plan.is_none() {
        let planned_zones = plan_landuse_zones(scene, &planning_profile);
        let parcel_plan = plan_parcels(&planned_zones);
        // Entity generation consumes the same ParcelPlan that is persisted later;
        // it must not independently reinterpret zone geometry.
        for parcel in &parcel_plan.parcels {
            let (kind, height) = match parcel.land_use.as_str() {
                "urban" if params.form == CityForm::LowDensitySuburban => {
                    ("residential_home", 8.0_f32)
                }
                "urban" => ("residential_block", 9.0 + params.core_density as f32 * 16.0),
                "rural" => ("farmland", 0.2_f32),
                "nature" => ("green_space", 0.2_f32),
                _ => continue,
            };
            let candidate_x = parcel.world_x + (parcel.width_m / 2) as i32;
            let candidate_z = parcel.world_z + (parcel.depth_m / 2) as i32;
            if projected_road_corridor_intersects(
                &params,
                scene,
                candidate_x,
                candidate_z,
                (parcel.width_m as f32 * 0.5).ceil() as i32 + 2,
                (parcel.depth_m as f32 * 0.5).ceil() as i32 + 2,
            ) {
                continue;
            }
            *candidate_identity.borrow_mut() =
                Some(crate::settlement::planned_entity_id(&parcel.id));
            let _ = add(
                kind,
                candidate_x,
                candidate_z,
                parcel.width_m as f32,
                parcel.depth_m as f32,
                height,
            );
        }
        for public_space in &parcel_plan.public_spaces {
            *candidate_identity.borrow_mut() =
                Some(crate::settlement::planned_entity_id(&public_space.id));
            let _ = add(
                "green_space",
                public_space.world_x + (public_space.width_m / 2) as i32,
                public_space.world_z + (public_space.depth_m / 2) as i32,
                public_space.width_m as f32,
                public_space.depth_m as f32,
                0.2,
            );
        }
    }
    // Fixed corner pasture guarantees a pasture on small scenes; use a
    // dedicated planner so its stable ID cannot collide with a grid pasture
    // that happens to land on the same cell.
    *candidate_planner.borrow_mut() = Some("landuse-pasture-corner".into());
    add("pasture", max_x - 120, max_z - 120, 100.0, 90.0, 0.2);
    let reserve_count = ((reserve_ratio * 10.0).ceil() as i32).max(1);
    for index in 0..reserve_count {
        add(
            "nature_reserve",
            min_x + 220 + index * 220,
            min_z + 220,
            180.0,
            180.0,
            0.4,
        );
    }
    // A forest belt always forms on the far rural side (the high ground
    // corner) so the map keeps wooded land even on small scenes with little
    // interior highland. Placing it at the far corner reads as mountain
    // forest rather than a flat tree farm. These are forced landmarks and
    // bypass the occupancy grid (they sit on wild ground away from roads).
    let belt_z = max_z - (scene.depth_m / 8) as i32;
    let belt_x1 = min_x + (scene.width_m / 10) as i32;
    let belt_x2 = max_x - (scene.width_m / 10) as i32;
    if belt_z > min_z && belt_z < max_z && belt_x1 < belt_x2 {
        for (bx, bw, bd) in [(belt_x1, 240.0_f32, 120.0_f32), (belt_x2, 200.0, 100.0)] {
            let bz = if bx == belt_x1 { belt_z } else { belt_z - 60 };
            let by = i32::from(terrain_height(
                seed,
                bx,
                bz,
                WaterKind::None,
                scene.width_m,
                scene.depth_m,
                0.0,
            )) / 4;
            entities.push(EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: ids.claim("landuse-forest-belt", "mountain_forest", &[i64::from(bx), i64::from(bz)]),
                asset_id: "prop.mountain_forest".to_owned(),
                kind: "mountain_forest".to_owned(),
                world_x: bx,
                world_z: bz,
                world_y: by,
                scale: 1.0,
                width_m: bw,
                depth_m: bd,
                height_m: 16.0,
            });
        }
    }
}

fn city_core_positions(params: &CityFormParams, scene: &SceneSpec, seed: u64) -> Vec<(i32, i32)> {
    let center_x = scene.origin_x + (scene.width_m / 2) as i32;
    let center_z = scene.origin_z + (scene.depth_m / 2) as i32;
    let span_x = (scene.width_m / 6) as i32;
    let span_z = (scene.depth_m / 6) as i32;
    let mut cores = vec![(center_x, center_z)];
    if !params.is_multi_core() {
        return cores;
    }
    let count = params.core_count - 1;
    for index in 0..count {
        // Linear forms (river delta, mountain valley) grow secondary cores
        // along a primary axis; the other multi-core forms spread them in a
        // golden-angle fan so the exact angle stays deterministic per seed.
        let (x, z) = match params.form {
            CityForm::RiverDelta => {
                let spacing = (index as f64 + 1.0) * 0.62;
                let z = center_z + (spacing * span_z as f64).round() as i32;
                (
                    center_x
                        + (signed_noise(seed.rotate_left(53), index as i32, 7) as f64
                            * span_x as f64
                            / 128.0) as i32,
                    z,
                )
            }
            CityForm::MountainValley => {
                let spacing = (index as f64 + 1.0) * 0.7;
                let z = center_z - (spacing * span_z as f64).round() as i32;
                (
                    center_x
                        + (signed_noise(seed.rotate_left(97), 3, index as i32) as f64
                            * span_x as f64
                            / 256.0) as i32,
                    z,
                )
            }
            _ => {
                let theta =
                    (seed as f64) * 0.618_033_988_749_894_9 + index as f64 * 2.399_963_229_728_653;
                let theta = theta.rem_euclid(std::f64::consts::TAU);
                let (sin_theta, cos_theta) = theta.sin_cos();
                (
                    center_x + (cos_theta * span_x as f64) as i32,
                    center_z + (sin_theta * span_z as f64) as i32,
                )
            }
        };
        cores.push((x, z));
    }
    cores
}

fn profile_ratio(style: &serde_json::Value, key: &str, fallback: f64) -> f64 {
    style
        .get("landUseProfile")
        .and_then(|profile| profile.get(key))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(fallback)
        .clamp(0.0, 1.0)
}

fn is_water_compatible_entity_kind(kind: &str) -> bool {
    matches!(
        kind,
        "lake" | "river" | "canal" | "bridge" | "causeway" | "mudflat" | "mangrove" | "reed"
    )
}

/// May this generated candidate be published at all, given the water body?
///
/// This is the GENERATION-side reader of the published shore clearance, and it
/// reads the same constant the audit reads: [`WATER_SHORE_CLEARANCE_M`]. The
/// two sides used to disagree (generation asked for 1.0 m, the published
/// `scale-audit` refuses at 4.0 m), so a legal bake could publish objects the
/// audit then refused - the hillside town published three trees on its banks
/// that no bake may produce. One number, one distance: a candidate the audit
/// would refuse is never generated.
///
/// Water-compatible kinds are exempt because they are the water's own
/// furniture (a reed grows in it, a bridge spans it) and the audit exempts them
/// through the same list; that is a stated kind rule, not a looser distance.
fn keeps_shore_clearance(entity: &EntityInstance, water: WaterGeometry) -> bool {
    is_water_compatible_entity_kind(&entity.kind)
        || !footprint_intersects_water(entity, water, WATER_SHORE_CLEARANCE_M)
}

/// Drop every generated candidate that does not keep the published shore
/// clearance. This is the filter every template/detail proposal passes through
/// before the bake publishes it, so "the audit would refuse it" and "generation
/// never produced it" are one statement.
fn retain_shore_clearance(entities: &mut Vec<EntityInstance>, water: WaterGeometry) {
    entities.retain(|entity| keeps_shore_clearance(entity, water));
}

fn landmark_requires_land(kind: &str) -> bool {
    !matches!(
        kind,
        "river" | "lake" | "canal" | "bridge" | "causeway" | "mudflat" | "mangrove" | "reed"
    )
}

/// Fail closed before baking: authored landmarks are rendered directly by
/// the HTML adapter, so they must be checked even when procedural placement
/// later filters or omits their entity record. Water-compatible infrastructure
/// and wetland kinds are explicitly exempted by `landmark_requires_land`.
fn validate_authored_landmarks_clear_water(
    landmarks: &[LandmarkSpec],
    scene: &SceneSpec,
    water: WaterGeometry,
) -> WorldgenResult<()> {
    for landmark in landmarks {
        if landmark_requires_land(&landmark.entity_type)
            && footprint_intersects_water(&landmark_to_entity(landmark), water, 0.0)
        {
            return Err(WorldgenError::LandmarkIntersectsWater {
                landmark: landmark.entity_id.clone(),
                scene: scene.scene_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn landmark_to_entity(landmark: &LandmarkSpec) -> EntityInstance {
    EntityInstance {
        rotation_y_deg: landmark.rotation_y_deg,
        grounding: landmark.grounding.clone(),
        anchors: landmark.anchors.clone(),
        bounds: landmark.bounds,
        entity_id: landmark.entity_id.clone(),
        asset_id: landmark.asset_id.clone(),
        kind: landmark.entity_type.clone(),
        world_x: landmark.world_x,
        world_z: landmark.world_z,
        world_y: landmark.world_y,
        scale: 1.0,
        width_m: landmark.width_m as f32,
        depth_m: landmark.depth_m as f32,
        height_m: landmark.height_m as f32,
    }
}

fn generate_entities_template(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
    ids: &CandidateIds,
    // True when this world has a real road plan AND states the street morphology a
    // shoreline commercial row is planned from (`frontage::shorefront_row_contract`).
    // The bayfront fixture band then belongs to the plan's own shorefront row instead of
    // to this generator; for every other world it stays exactly as it was.
    shorefront_row: bool,
    natural_only: bool,
) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for landmark in landmarks {
        entities.push(EntityInstance {
            rotation_y_deg: landmark.rotation_y_deg,
            grounding: landmark.grounding.clone(),
            anchors: landmark.anchors.clone(),
            bounds: landmark.bounds,
            entity_id: landmark.entity_id.clone(),
            asset_id: landmark.asset_id.clone(),
            kind: landmark.entity_type.clone(),
            world_x: landmark.world_x,
            world_z: landmark.world_z,
            world_y: landmark.world_y,
            scale: 1.0,
            width_m: landmark.width_m as f32,
            depth_m: landmark.depth_m as f32,
            height_m: landmark.height_m as f32,
        });
    }
    // The natural stage is an authored landmark slice plus natural content only.
    // In particular, a lake must not silently bring the legacy shore road,
    // bridge and village template back into the "no human trace" snapshot.
    if natural_only {
        return entities;
    }
    // On a waterless mainland the modern profile owns all ordinary land; the
    // legacy river/lake street grids below are water-scene specials. If there
    // is no water landmark, return landmarks only so nothing legacy is layered
    // on top of the modern profile (and no tree grows on a modern road).
    if geometry.kind == WaterKind::None {
        return entities;
    }
    // GIS / real-data scenes: when the manifest already supplies explicit
    // buildings and roads as landmarks, the legacy water street grid must not
    // layer procedural towers onto the real footprints. Anything with an
    // authored `road`/`building` landmark is treated as data-driven.
    let has_real_city = landmarks.iter().any(|l| {
        matches!(
            l.entity_type.as_str(),
            "road"
                | "building"
                | "building_tower"
                | "storefront"
                | "shopping_mall"
                | "office_tower"
                | "convenience_store"
                | "community_plaza"
                | "night_market"
        )
    });
    if has_real_city {
        return entities;
    }
    let water = geometry.kind;
    let river_center = geometry.center_x;
    let river_half_m = geometry.half_width;
    let in_river = |world_x: f64, world_z: f64| {
        (world_x - river_center).abs()
            < river_half_width_at(
                world_z - f64::from(scene.origin_z),
                scene.depth_m,
                river_half_m,
            )
    };
    if water == WaterKind::Lake {
        let lake_x = geometry.center_x.round() as i32;
        let north_shore_z = (geometry.center_z - geometry.half_depth - 30.0).round() as i32;
        let bridge_z = (geometry.center_z - geometry.half_depth * 0.45).round() as i32;
        let village_x = (geometry.center_x + geometry.half_width + 80.0).round() as i32;
        let village_z = (geometry.center_z - geometry.half_depth - 100.0).round() as i32;
        entities.extend([
            EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: "generated.north-shore-road".to_owned(),
                asset_id: "prop.road".to_owned(),
                kind: "road".to_owned(),
                world_x: lake_x,
                world_z: north_shore_z,
                world_y: (i32::from(terrain_height_with_geometry(
                    seed,
                    lake_x,
                    north_shore_z,
                    geometry,
                )) / 4)
                    .max(1),
                scale: 1.0,
                width_m: (geometry.half_width * 2.0 + 90.0) as f32,
                depth_m: 14.0,
                height_m: 0.24,
            },
            EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: "generated.lake-bridge".to_owned(),
                asset_id: "prop.bridge".to_owned(),
                kind: "bridge".to_owned(),
                world_x: lake_x,
                world_z: bridge_z,
                world_y: 0,
                scale: 1.0,
                width_m: (geometry.half_width * 0.32).max(20.0) as f32,
                depth_m: 24.0,
                height_m: 8.0,
            },
            EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: "generated.west-village".to_owned(),
                asset_id: "prop.building-cluster".to_owned(),
                kind: "building_cluster".to_owned(),
                world_x: village_x,
                world_z: village_z,
                world_y: i32::from(terrain_height_with_geometry(
                    seed, village_x, village_z, geometry,
                )) / 4,
                scale: 1.0,
                width_m: 120.0,
                depth_m: 100.0,
                height_m: 32.0,
            },
        ]);
    }
    if water == WaterKind::River {
        let road_offsets = [-(river_half_m as i32 + 130), river_half_m as i32 + 130];
        for (index, offset) in road_offsets.iter().enumerate() {
            let road_x = scene.origin_x + (scene.width_m / 2) as i32 + offset;
            let road_z = scene.origin_z + (scene.depth_m / 2) as i32;
            let road_y = (i32::from(terrain_height_with_geometry(seed, road_x, road_z, geometry))
                / 4)
            .max(1);
            entities.push(EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: format!("generated.riverbank-road-{index}"),
                asset_id: "prop.road".to_owned(),
                kind: "road".to_owned(),
                world_x: road_x,
                world_z: road_z,
                world_y: road_y,
                scale: 1.0,
                width_m: (f64::from(scene.depth_m) * 0.9) as f32,
                depth_m: 14.0,
                height_m: 0.24,
            });
            for side in [-1_i32, 1_i32] {
                let sidewalk_x = road_x + side * 11;
                let sidewalk_y = (i32::from(terrain_height_with_geometry(
                    seed, sidewalk_x, road_z, geometry,
                )) / 4)
                    .max(1);
                entities.push(EntityInstance {
                    rotation_y_deg: 0.0,
                    grounding: GroundingSpec::default(),
                    anchors: Vec::new(),
                    bounds: None,
                    entity_id: format!("generated.riverbank-sidewalk-{index}-{side}"),
                    asset_id: "prop.sidewalk".to_owned(),
                    kind: "sidewalk".to_owned(),
                    world_x: sidewalk_x,
                    world_z: road_z,
                    world_y: sidewalk_y,
                    scale: 1.0,
                    width_m: (f64::from(scene.depth_m) * 0.9) as f32,
                    depth_m: 5.0,
                    height_m: 0.35,
                });
            }
        }
        let bridge_center = entities
            .iter()
            .find(|entity| entity.kind == "bridge" && entity.width_m > 1000.0)
            .map(|entity| (entity.world_x, entity.world_z, entity.width_m));
        let _ = bridge_center;
        let center_x = scene.origin_x + (scene.width_m / 2) as i32;
        let half_width = river_half_m as i32;
        // Riverbank roads sit at centre ±(half_width + 130) with a 7m half-width
        // (their corridor spans ±(half_width+123) .. ±(half_width+137)). Keep
        // tower blocks outside that corridor so no building sits on a road.
        let left_lo = 140;
        let left_hi = (center_x - half_width - 90).max(left_lo + 60);
        let right_lo = center_x + half_width + 90;
        let right_hi = scene.width_m as i32 - 140;
        for (band_lo, band_hi) in [(left_lo, left_hi), (right_lo, right_hi)] {
            for z in (220..scene.depth_m.saturating_sub(220)).step_by(60) {
                for x in (band_lo..band_hi).step_by(46) {
                    let world_x = scene.origin_x + x;
                    let world_z = scene.origin_z + z as i32;
                    if in_river(f64::from(world_x), f64::from(world_z)) {
                        continue;
                    }
                    // Skip the road corridor itself (road sits at
                    // centre ±(half_width + 130), half width 7m).
                    let road_x = center_x
                        + if band_lo < center_x { -1 } else { 1 } * (half_width as i32 + 130);
                    if (world_x - road_x).abs() < 24 {
                        continue;
                    }
                    let roll = signed_noise(seed.rotate_left(73), world_x, world_z);
                    if roll < -70 {
                        continue;
                    }
                    let world_y = i32::from(terrain_height_with_geometry(
                        seed, world_x, world_z, geometry,
                    )) / 4;
                    // A resort never sits in the water or on a flooded mudflat:
                    // skip spots whose terrain is below datum (they are wet
                    // shore, not building ground). This keeps the waterfront
                    // band on dry land instead of raising plinths in a lagoon.
                    if world_y < 1 {
                        continue;
                    }
                    // Waterfront band reads as a low-rise holiday resort zone
                    // (white walls, wood louvers, flat roofs) while the outer
                    // band carries the skyline towers. This mirrors a bayfront
                    // resort district: low and open near the water, taller
                    // towers stepping up inland.
                    let dist_from_water = (world_x - center_x).abs() - half_width as i32;
                    let resort = dist_from_water < 320;
                    if resort {
                        let lodge_w = 26.0 + roll.rem_euclid(18) as f32;
                        let lodge_h = 6.0 + roll.rem_euclid(10) as f32;
                        entities.push(EntityInstance {
                            rotation_y_deg: 0.0,
                            grounding: GroundingSpec::default(),
                            anchors: Vec::new(),
                            bounds: None,
                            entity_id: ids.claim("resort-lodge", "resort_lodge", &[i64::from(band_lo), i64::from(x), i64::from(z)]),
                            asset_id: "prop.resort-lodge".to_owned(),
                            kind: "resort_lodge".to_owned(),
                            world_x,
                            world_z,
                            world_y,
                            scale: 1.0,
                            width_m: lodge_w,
                            depth_m: 20.0,
                            height_m: lodge_h,
                        });
                    } else {
                        let tower_h = (24 + roll.rem_euclid(26)) as f32;
                        entities.push(EntityInstance {
                            rotation_y_deg: 0.0,
                            grounding: GroundingSpec::default(),
                            anchors: Vec::new(),
                            bounds: None,
                            entity_id: ids.claim("block-tower", "building_tower", &[i64::from(band_lo), i64::from(x), i64::from(z)]),
                            asset_id: "prop.building-tower".to_owned(),
                            kind: "building_tower".to_owned(),
                            world_x,
                            world_z,
                            world_y,
                            scale: 1.0,
                            width_m: 20.0,
                            depth_m: 18.0,
                            height_m: tower_h,
                        });
                    }
                }
            }
        }
        for side in [-1_i32, 1_i32] {
            let frontage_x = center_x + side * (half_width + 150);
            // Skip the whole shopfront row if the road-side lot would fall
            // outside the scene (narrow bayfront slices).
            if frontage_x < 40 || frontage_x > scene.width_m as i32 - 40 {
                continue;
            }
            // The fixed bayfront commercial band is a FIXTURE coordinate: `centre ±
            // (half_width + 150)` is 150 m from the water, which in the registered
            // river-delta sample is three blocks behind the quay and wholly inside the
            // rural zoning. A road-first world plans its own streets, so its commerce is
            // the shoreline row `frontage::append_shore_row` plans on the plan's first
            // street landward of the shore (same stations, same gates, same conversion
            // accounting). This band is kept ONLY for a world with no plan to plan
            // against, which is why every such world is byte-identical.
            if !shorefront_row {
                for z in (160..scene.depth_m.saturating_sub(120)).step_by(18) {
                    let world_z = scene.origin_z + z as i32;
                    let roll = signed_noise(seed.rotate_left(91), frontage_x, world_z);
                    if roll < -105 {
                        continue;
                    }
                    let width = 12.0 + roll.rem_euclid(9) as f32;
                    let height = 5.0 + roll.rem_euclid(7) as f32;
                    let world_y = (i32::from(terrain_height_with_geometry(
                        seed, frontage_x, world_z, geometry,
                    )) / 4)
                        .max(1);
                    entities.push(EntityInstance {
                        rotation_y_deg: 0.0,
                        grounding: GroundingSpec::default(),
                        anchors: Vec::new(),
                        bounds: None,
                        entity_id: ids.claim("storefront", "storefront", &[i64::from(side), 0, i64::from(z)]),
                        asset_id: "prop.storefront".to_owned(),
                        kind: "storefront".to_owned(),
                        world_x: frontage_x,
                        world_z,
                        world_y,
                        scale: 1.0,
                        width_m: width,
                        depth_m: 12.0,
                        height_m: height,
                    });
                }
            }
            // Street trees line the sidewalk between the shopfront and the
            // road: a shop sits on the pavement inner edge, the tree row runs
            // along the pavement centre, and the road is on the far side. This
            // mirrors a real commercial street instead of planting trees on
            // the wrong side of the carriageway.
            let road_axis = center_x + side * (half_width + 130);
            // Storefronts are 12m deep; their street-facing wall sits 6m from
            // the lot centre. Trees go 2m beyond that wall (away from the
            // shop) so the canopy never grows into the facade.
            let shop_wall = frontage_x - side * 6;
            let tree_x = shop_wall - side * 2;
            for z in (176..scene.depth_m.saturating_sub(120)).step_by(30) {
                let world_z = scene.origin_z + z as i32;
                let roll = signed_noise(seed.rotate_left(109), tree_x, world_z);
                let crown = 3.2 + roll.rem_euclid(18) as f32 / 10.0;
                let height = 5.5 + roll.rem_euclid(28) as f32 / 10.0;
                let world_y = (i32::from(terrain_height_with_geometry(
                    seed, tree_x, world_z, geometry,
                )) / 4)
                    .max(1);
                entities.push(EntityInstance {
                    rotation_y_deg: 0.0,
                    grounding: GroundingSpec::default(),
                    anchors: Vec::new(),
                    bounds: None,
                    entity_id: ids.claim("street-tree", "tree", &[i64::from(side), 0, i64::from(z)]),
                    asset_id: "prop.tree".to_owned(),
                    kind: "tree".to_owned(),
                    world_x: tree_x,
                    world_z,
                    world_y,
                    scale: 1.0,
                    width_m: crown,
                    depth_m: crown,
                    height_m: height,
                });
            }
            let second_tree_x = road_axis - side * 18;
            for z in (190..scene.depth_m.saturating_sub(120)).step_by(30) {
                let world_z = scene.origin_z + z as i32;
                let roll = signed_noise(seed.rotate_left(131), second_tree_x, world_z);
                let crown = 3.0 + roll.rem_euclid(16) as f32 / 10.0;
                let height = 5.0 + roll.rem_euclid(25) as f32 / 10.0;
                let world_y = (i32::from(terrain_height_with_geometry(
                    seed,
                    second_tree_x,
                    world_z,
                    geometry,
                )) / 4)
                    .max(1);
                entities.push(EntityInstance {
                    rotation_y_deg: 0.0,
                    grounding: GroundingSpec::default(),
                    anchors: Vec::new(),
                    bounds: None,
                    entity_id: ids.claim("street-tree", "tree", &[i64::from(side), 1, i64::from(z)]),
                    asset_id: "prop.tree".to_owned(),
                    kind: "tree".to_owned(),
                    world_x: second_tree_x,
                    world_z,
                    world_y,
                    scale: 1.0,
                    width_m: crown,
                    depth_m: crown,
                    height_m: height,
                });
            }
            let second_row_x = frontage_x + side * 72;
            // Only a second row of shops if there is actually room for it on
            // the scene side — in a narrow bayfront slice (like Shenzhen Bay)
            // the frontage may already sit near the scene edge, so pushing a
            // second row past it would place shops outside the world.
            if !shorefront_row && second_row_x > 40 && second_row_x < scene.width_m as i32 - 40 {
                for z in (170..scene.depth_m.saturating_sub(130)).step_by(28) {
                    let world_z = scene.origin_z + z as i32;
                    let roll = signed_noise(seed.rotate_left(97), second_row_x, world_z);
                    if roll < -92 {
                        continue;
                    }
                    let world_y = (i32::from(terrain_height_with_geometry(
                        seed,
                        second_row_x,
                        world_z,
                        geometry,
                    )) / 4)
                        .max(1);
                    entities.push(EntityInstance {
                        rotation_y_deg: 0.0,
                        grounding: GroundingSpec::default(),
                        anchors: Vec::new(),
                        bounds: None,
                        entity_id: ids.claim("storefront", "storefront", &[i64::from(side), 1, i64::from(z)]),
                        asset_id: "prop.storefront".to_owned(),
                        kind: "storefront".to_owned(),
                        world_x: second_row_x,
                        world_z,
                        world_y,
                        scale: 1.0,
                        width_m: 14.0,
                        depth_m: 14.0,
                        height_m: 7.0 + roll.rem_euclid(6) as f32,
                    });
                }
            }
            // Inland city band: on the far side of the waterfront road from the
            // bay, urban buildings form the denser city core while the
            // bayfront resort lodges stay low near the water. The band runs
            // from the road edge to the scene boundary on each shore.
            let road_axis_x = center_x + side * (half_width + 130);
            let urban_lo = if side < 0 { 40 } else { road_axis_x + 30 };
            let urban_hi = if side < 0 {
                road_axis_x - 30
            } else {
                scene.width_m as i32 - 40
            };
            if urban_lo < urban_hi {
                for z in (220..scene.depth_m.saturating_sub(220)).step_by(36) {
                    for x in (urban_lo..urban_hi).step_by(30) {
                        let world_x = scene.origin_x + x;
                        let world_z = scene.origin_z + z as i32;
                        if in_river(f64::from(world_x), f64::from(world_z)) {
                            continue;
                        }
                        if (world_x - road_axis_x).abs() < 30 {
                            continue;
                        }
                        if entities.iter().any(|existing| {
                            (existing.kind == "building_tower" || existing.kind == "resort_lodge")
                                && f64::from((existing.world_x - world_x).abs())
                                    < f64::from(existing.width_m) * 0.5 + 18.0
                                && f64::from((existing.world_z - world_z).abs())
                                    < f64::from(existing.depth_m) * 0.5 + 18.0
                        }) {
                            continue;
                        }
                        let roll = signed_noise(seed.rotate_left(103), world_x, world_z);
                        if roll < -96 {
                            continue;
                        }
                        let world_y = (i32::from(terrain_height_with_geometry(
                            seed, world_x, world_z, geometry,
                        )) / 4)
                            .max(1);
                        entities.push(EntityInstance {
                            rotation_y_deg: 0.0,
                            grounding: GroundingSpec::default(),
                            anchors: Vec::new(),
                            bounds: None,
                            entity_id: ids.claim("urban-building", "urban_building", &[i64::from(side), i64::from(x), i64::from(z)]),
                            asset_id: "prop.building".to_owned(),
                            kind: "urban_building".to_owned(),
                            world_x,
                            world_z,
                            world_y,
                            scale: 1.0,
                            width_m: 22.0 + roll.rem_euclid(9) as f32,
                            depth_m: 20.0 + roll.rem_euclid(9) as f32,
                            height_m: 8.0 + roll.rem_euclid(19) as f32,
                        });
                    }
                }
            }
        }
    }
    for z in (24..scene.depth_m.saturating_sub(16)).step_by(32) {
        for x in (24..scene.width_m.saturating_sub(16)).step_by(32) {
            let world_x = scene.origin_x + x as i32;
            let world_z = scene.origin_z + z as i32;
            if water != WaterKind::None {
                let in_water = match water {
                    WaterKind::Lake => {
                        lake_radius_at(f64::from(world_x), f64::from(world_z), geometry)
                            < SHORE_RADIUS
                    }
                    WaterKind::River => in_river(f64::from(world_x), f64::from(world_z)),
                    WaterKind::None => false,
                };
                if in_water {
                    continue;
                }
            }
            let roll = signed_noise(seed.rotate_left(13), world_x, world_z);
            let kind = if roll > 112 {
                "building"
            } else if roll > -12 {
                "tree"
            } else if roll < -104 {
                "rock"
            } else if roll < 58 {
                "bush"
            } else {
                continue;
            };
            if water == WaterKind::River && kind == "building" {
                let city_center_x = scene.origin_x + (scene.width_m / 2) as i32;
                if (world_x - city_center_x).abs() < river_half_m as i32 + 650 {
                    continue;
                }
            }
            let jitter_x = signed_noise(seed.rotate_left(7), world_x, world_z).rem_euclid(13) - 6;
            let jitter_z = signed_noise(seed.rotate_left(19), world_x, world_z).rem_euclid(13) - 6;
            let world_x = world_x + jitter_x;
            let world_z = world_z + jitter_z;
            if water == WaterKind::River && in_river(f64::from(world_x), f64::from(world_z)) {
                continue;
            }
            let world_y = i32::from(terrain_height_with_geometry(
                seed, world_x, world_z, geometry,
            )) / 4;
            let (width_m, depth_m, height_m) = match kind {
                "building" => (12.0, 12.0, 8.0),
                "tree" => {
                    let crown = 2.6
                        + (signed_noise(seed.rotate_left(61), world_x, world_z) + 128) as f32
                            / 256.0
                            * 2.2;
                    let height = 5.0
                        + (signed_noise(seed.rotate_left(67), world_x, world_z) + 128) as f32
                            / 256.0
                            * 3.0;
                    (crown, crown, height)
                }
                "bush" => (1.5, 1.5, 1.2),
                _ => (1.0, 1.0, 1.0),
            };
            entities.push(EntityInstance {
                rotation_y_deg: 0.0,
                grounding: GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
                entity_id: ids.claim(&format!("template-{kind}"), kind, &[i64::from(x), i64::from(z)]),
                asset_id: format!("prop.{kind}"),
                kind: kind.to_owned(),
                world_x,
                world_z,
                world_y,
                scale: if kind == "building" { 1.0 } else { 1.0 },
                width_m,
                depth_m,
                height_m,
            });
        }
    }
    if water != WaterKind::None {
        for z in (12..scene.depth_m.saturating_sub(12)).step_by(20) {
            for x in (12..scene.width_m.saturating_sub(12)).step_by(20) {
                let world_x = scene.origin_x + x as i32;
                let world_z = scene.origin_z + z as i32;
                let radius = match water {
                    WaterKind::Lake => {
                        lake_radius_at(f64::from(world_x), f64::from(world_z), geometry)
                    }
                    WaterKind::River => 0.0,
                    WaterKind::None => f64::MAX,
                };
                let roll = signed_noise(seed.rotate_left(43), world_x, world_z);
                let river_shore = water == WaterKind::River && {
                    let distance = (f64::from(world_x) - river_center).abs();
                    let half_width = river_half_width_at(
                        f64::from(world_z - scene.origin_z),
                        scene.depth_m,
                        river_half_m,
                    );
                    distance >= half_width + 40.0 && distance < half_width + 150.0
                };
                let in_water_now =
                    water == WaterKind::River && in_river(f64::from(world_x), f64::from(world_z));
                let kind = if ((1.0..SHORE_RADIUS).contains(&radius) || river_shore) && roll > -64 {
                    "reed"
                } else if !in_water_now
                    && (radius >= SHORE_RADIUS || water == WaterKind::River)
                    && roll > 92
                {
                    match roll.rem_euclid(4) {
                        0 => "grass_clump",
                        1 => "bench",
                        2 => "lamp",
                        _ => "fallen_log",
                    }
                } else {
                    continue;
                };
                let jitter_x =
                    signed_noise(seed.rotate_left(47), world_x, world_z).rem_euclid(9) - 4;
                let jitter_z =
                    signed_noise(seed.rotate_left(53), world_x, world_z).rem_euclid(9) - 4;
                let world_x = world_x + jitter_x;
                let world_z = world_z + jitter_z;
                if water == WaterKind::River
                    && in_river(f64::from(world_x), f64::from(world_z))
                    && kind != "reed"
                {
                    continue;
                }
                let world_y = i32::from(terrain_height_with_geometry(
                    seed, world_x, world_z, geometry,
                )) / 4;
                entities.push(EntityInstance {
                    rotation_y_deg: 0.0,
                    grounding: GroundingSpec::default(),
                    anchors: Vec::new(),
                    bounds: None,
                    entity_id: ids.claim(&format!("shore-{kind}"), kind, &[i64::from(x), i64::from(z)]),
                    asset_id: format!("prop.{kind}"),
                    kind: kind.to_owned(),
                    world_x,
                    world_z,
                    world_y,
                    scale: 1.0,
                    width_m: match kind {
                        "reed" => 0.5,
                        "grass_clump" => 1.8,
                        "bench" => 3.4,
                        "lamp" => 0.5,
                        "fallen_log" => 3.2,
                        _ => 1.0,
                    },
                    depth_m: match kind {
                        "reed" => 0.5,
                        "grass_clump" => 1.8,
                        "bench" => 1.2,
                        "lamp" => 0.5,
                        "fallen_log" => 0.8,
                        _ => 1.0,
                    },
                    height_m: match kind {
                        "reed" => 3.8,
                        "grass_clump" => 3.5,
                        "bench" => 1.1,
                        "lamp" => 4.8,
                        "fallen_log" => 1.0,
                        _ => 1.0,
                    },
                });
            }
        }
    }
    entities
}

fn signed_noise(seed: u64, x: i32, z: i32) -> i32 {
    let mut value = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value as i32).rem_euclid(257) - 128
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Round 40: generation and the published audit share ONE shore-clearance
    /// number (`water_query::WATER_SHORE_CLEARANCE_M`).
    ///
    /// The two sides used to disagree - generation asked for 1.0 m while the
    /// published `scale-audit` refuses at 4.0 m - so the hillside town could
    /// publish three trees on its banks that no bake is allowed to produce. The
    /// fixtures are two trees whose crowns stop 3.9 m and 4.1 m short of the
    /// waterline: the first must NOT survive generation, the second must.
    #[test]
    fn the_generated_shore_clearance_is_the_published_constant() {
        // A 40 m channel down the middle of a 512 m scene: the east waterline is
        // x = 276, so a crown centred at 282 stops at 282 - width/2.
        let water = WaterGeometry {
            kind: WaterKind::River,
            center_x: 256.,
            center_z: 256.,
            half_width: 20.,
            half_depth: 256.,
            scene_width_m: 512,
            scene_depth_m: 512,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let tree = |id: &str, width_m: f32| -> EntityInstance {
            serde_json::from_value(serde_json::json!({
                "entityId": id, "assetId": "prop.tree", "kind": "tree",
                "worldX": 282, "worldZ": 100, "worldY": 0, "scale": 1.0,
                "widthM": width_m, "depthM": width_m, "heightM": 8.0
            }))
            .unwrap()
        };
        // 4.2 m crown -> 279.9: 3.9 m of dry ground from the waterline.
        let three_point_nine = tree("tree-3.9", 4.2);
        // 3.8 m crown -> 280.1: 4.1 m.
        let four_point_one = tree("tree-4.1", 3.8);
        assert_eq!(WATER_SHORE_CLEARANCE_M, 4.0);
        // The published audit refuses the 3.9 m sample and keeps the 4.1 m one...
        assert!(footprint_intersects_water(
            &three_point_nine,
            water,
            WATER_SHORE_CLEARANCE_M
        ));
        assert!(!footprint_intersects_water(
            &four_point_one,
            water,
            WATER_SHORE_CLEARANCE_M
        ));
        // ...and generation drops exactly the sample the audit would refuse.
        let mut proposed = vec![three_point_nine, four_point_one];
        retain_shore_clearance(&mut proposed, water);
        assert_eq!(proposed.len(), 1, "only the 4.1 m tree may survive generation");
        assert_eq!(proposed[0].entity_id, "tree-4.1");
    }

    #[test]
    fn authored_identity_does_not_require_landmark_prefix() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let authored: LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId":"my-city-hall","name":"City hall","type":"building",
            "purpose":"civic","description":"Required subject","sceneId":"scene-0",
            "worldX":200,"worldZ":200,"worldY":50,"widthM":20,"depthM":10,
            "heightM":10,"assetId":"prop.building"
        }))
        .unwrap();
        let mut style = serde_json::json!({});
        style["rulesDir"] = serde_json::json!(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
        );
        let water = water_geometry(WaterKind::None, &[], &scene, &style);
        let (entities, _) = apply_rules_mode(
            42,
            &scene,
            water,
            &style,
            std::slice::from_ref(&authored),
            vec![landmark_to_entity(&authored)],
            None,
            None,
        );
        assert_eq!(entities.len(), 1);
        assert_eq!(
            (
                entities[0].world_x,
                entities[0].world_z,
                entities[0].world_y
            ),
            (200, 200, 50)
        );
    }

    #[test]
    fn rules_reject_pond_entirely_enclosed_between_sample_points() {
        let scene = SceneSpec {
            scene_id: "pond".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let water = WaterGeometry {
            kind: WaterKind::Lake,
            center_x: 203.,
            center_z: 203.,
            half_width: 1.,
            half_depth: 1.,
            scene_width_m: 512,
            scene_depth_m: 512,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let registry = crate::rules::ObjectRegistry::load_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects"),
        )
        .unwrap();
        let descriptor = registry
            .descriptors
            .values()
            .find(|d| d.matches_kind("building"))
            .unwrap();
        let fp = crate::rules::Footprint {
            rotation_y_deg: 0.,
            cx: 200.,
            cz: 200.,
            half_w: 20.,
            half_d: 20.,
        };
        assert!(
            fp.sample_points()
                .iter()
                .all(|(x, z)| !water.contains_point(f64::from(*x), f64::from(*z)))
        );
        with_rules_placement_context(7, &scene, water, |ctx| {
            assert!(matches!(
                crate::rules::check_hard(
                    descriptor,
                    &fp,
                    ctx,
                    &[crate::rules::constraint::Constraint::NotInWater],
                    &[]
                ),
                Err(crate::rules::RejectReason::InWater)
            ));
        });
    }

    #[test]
    fn manifest_rejects_duplicate_landmarks_rotation_overflow_and_empty_scenes() {
        let mut m: WorldManifest = serde_json::from_str(include_str!(
            "../../../../examples/shenzhen-bay-minimal.manifest.json"
        ))
        .unwrap();
        let original = m.clone();
        m.landmarks.push(m.landmarks[0].clone());
        assert!(m.validate().is_err());
        m = original.clone();
        m.landmarks[0].rotation_y_deg = f32::NAN;
        assert!(m.validate().is_err());
        m = original.clone();
        m.scenes[0].origin_x = i32::MAX;
        assert!(m.validate().is_err());
        m = original;
        m.scenes.clear();
        assert!(m.validate().is_err());
    }

    #[test]
    fn baked_rebase_respects_center_pivot_and_offsets() {
        let scene = SceneSpec {
            scene_id: "pivot".into(),
            width_m: 16,
            depth_m: 16,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 16, 16, &12_i16.to_le_bytes().repeat(256))
            .unwrap();
        let mut entity = generated_intent_entity("pivot".into(), "building", 8, 8, 0, 4., 4., 10.);
        entity.grounding.pivot = GroundingPivot::Center;
        entity.grounding.bottom_offset_m = 0.25;
        rebase_entities_to_baked_surface(std::slice::from_mut(&mut entity), &terrain);
        assert_eq!(entity.world_y, 8);
        assert!(entity.ground_height_m() >= 3.);
    }

    #[test]
    fn default_world_is_valid() {
        WorldManifest::default_demo().validate().unwrap();
    }

    #[test]
    fn water_generation_requires_structured_water_model() {
        assert_eq!(
            water_kind(&serde_json::json!({"layoutReference": "西湖"})),
            WaterKind::None
        );
        assert_eq!(
            water_kind(
                &serde_json::json!({"water": {"waterType": "lake", "levelPolicy": "horizontal-datum"}})
            ),
            WaterKind::Lake
        );
    }

    #[test]
    fn patch_chain_rejects_stale_base_revision_and_changes_revision() {
        let manifest = WorldManifest::default_demo();
        let base = world_manifest_revision(&manifest).unwrap();
        let patch = WorldPatch {
            format: "glyphweave-world-patch".into(),
            version: 2,
            patch_id: "patch-1".into(),
            base_revision: base.clone(),
            parent_patch_hash: Some(String::new()),
            sequence: 1,
            operations: Vec::new(),
        };
        let patched = apply_patch(&manifest, &patch).unwrap();
        assert_ne!(patched.patch_chain_hash, manifest.patch_chain_hash);
        assert_ne!(world_manifest_revision(&patched).unwrap(), base);
        let stale = WorldPatch {
            base_revision: "stale".into(),
            ..patch
        };
        assert!(matches!(
            apply_patch(&manifest, &stale),
            Err(WorldgenError::InvalidPatch(_))
        ));
    }

    #[test]
    fn strict_profile_requires_rules_mode() {
        let mut manifest = WorldManifest::default_demo();
        manifest.style["generationProfile"] = serde_json::json!("strict");
        let output =
            std::env::temp_dir().join(format!("glyphweave-strict-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output);
        let result = bake_world(&manifest, &output);
        assert!(
            matches!(result, Err(WorldgenError::InvalidRules(message)) if message.contains("requires style.rulesMode"))
        );
        let _ = std::fs::remove_dir_all(output);
    }

    #[test]
    fn terrain_domain_and_precision_are_validated_before_bake() {
        let mut m = WorldManifest::default_demo();
        m.style["heightPrecision"] = serde_json::json!(false);
        assert!(m.validate().is_err());
        m.style["heightPrecision"] = serde_json::json!("mysterious");
        assert!(m.validate().is_err());
        m.style["heightPrecision"] = serde_json::json!("millimeter");
        assert!(m.validate().is_ok());
        m.style["terrainDomain"] =
            serde_json::json!({"originX":0,"originZ":0,"widthM":999,"depthM":1000});
        assert!(m.validate().is_err());
        m.style["terrainDomain"] =
            serde_json::json!({"originX":0,"originZ":0,"widthM":1000,"depthM":1000,"ignored":true});
        assert!(m.validate().is_err());
        m.style["terrainDomain"] =
            serde_json::json!({"originX":0,"originZ":0,"widthM":1000,"depthM":1000});
        assert!(m.validate().is_ok());
        let mut scene = m.scenes[0].clone();
        scene.origin_x = -200;
        scene.origin_z = 500;
        let geometry = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        assert_eq!(geometry.terrain_origin_x, -200);
        assert_eq!(geometry.terrain_origin_z, 500);
    }

    #[test]
    fn precise_height_is_from_continuous_source_not_rescaled_legacy_steps() {
        let water = legacy_water_geometry(WaterKind::None, 512, 512, 0.);
        let (bytes, _, _, _) = generate_chunk_encoded(7, 0, 0, 64, 64, water, &[], true, None);
        let payload = HeightPayload::parse(&bytes, 64, 64).unwrap();
        assert!((0..payload.count).any(|i| payload.raw(i).rem_euclid(250) != 0));
        for z in 0..64 {
            for x in 0..64 {
                let source = terrain_height_metres(7, x, z, water, &[]);
                let actual = payload.metres((z * 64 + x) as usize);
                assert!((actual - source).abs() <= 0.000500001);
                let legacy = terrain_height_with_geometry(7, x, z, water) as f64 / 4.;
                assert!((legacy - source).abs() < 0.25 + 1e-9);
            }
        }
    }

    #[test]
    fn same_seed_produces_same_chunk() {
        assert_eq!(
            generate_chunk(42, 0, 0, 32, 32, WaterKind::None, 1_000, 1_000, 0.0),
            generate_chunk(42, 0, 0, 32, 32, WaterKind::None, 1_000, 1_000, 0.0)
        );
    }

    #[test]
    fn invalid_scene_is_rejected() {
        let mut world = WorldManifest::default_demo();
        world.scenes[0].width_m = 511;
        assert!(matches!(
            world.validate(),
            Err(WorldgenError::InvalidSceneSize { .. })
        ));
    }

    #[test]
    fn scene_graph_transition_is_typed_and_validated() {
        let mut world = WorldManifest::default_demo();
        world.scenes.push(SceneSpec {
            scene_id: "scene-1".into(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 1_000,
            origin_z: 0,
            seed_offset: 1,
        });
        world.scene_graph.transitions.push(SceneTransition {
            id: "east-gate".into(),
            source_scene: "scene-0".into(),
            target_scene: "scene-1".into(),
            source_world_x: 999,
            source_world_z: 500,
            target_world_x: 1_000,
            target_world_z: 500,
            direction: "east".into(),
            kind: "road_exit".into(),
        });
        world.validate().unwrap();
        let json = serde_json::to_value(&world).unwrap();
        assert_eq!(
            json["sceneGraph"]["transitions"][0]["targetScene"],
            "scene-1"
        );
    }

    #[test]
    fn manifest_rejects_unknown_top_level_fields() {
        let mut json = serde_json::to_value(WorldManifest::default_demo()).unwrap();
        json["unexpected"] = serde_json::json!(true);
        let err = serde_json::from_value::<WorldManifest>(json).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn grounding_and_bounds_are_normalized_for_entities() {
        let mut entity = EntityInstance {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "test".into(),
            asset_id: "prop.test".into(),
            kind: "building".into(),
            world_x: 10,
            world_z: 20,
            world_y: 4,
            scale: 1.0,
            width_m: 8.0,
            depth_m: 6.0,
            height_m: 10.0,
        };
        entity.normalize_spatial_semantics();
        assert_eq!(entity.bounds.unwrap().min_x, 6.0);
        assert_eq!(entity.ground_height_m(), 4.0);
        entity.grounding.pivot = GroundingPivot::Center;
        assert_eq!(entity.ground_height_m(), -1.0);
    }

    #[test]
    fn generated_spatial_semantics_infer_road_rotation_and_building_front() {
        let mut road = EntityInstance {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "generated.landuse-road-ns.1".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 50,
            world_z: 50,
            world_y: 0,
            scale: 1.0,
            width_m: 80.0,
            depth_m: 8.0,
            height_m: 0.25,
        };
        road.normalize_spatial_semantics();
        assert_eq!(road.rotation_y_deg, 90.0);

        let mut building = EntityInstance {
            rotation_y_deg: 90.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "generated.house.1".into(),
            asset_id: "prop.house".into(),
            kind: "building".into(),
            world_x: 10,
            world_z: 20,
            world_y: 0,
            scale: 1.0,
            width_m: 12.0,
            depth_m: 8.0,
            height_m: 6.0,
        };
        building.normalize_spatial_semantics();
        assert_eq!(building.anchors[0].direction, "west");
        assert_eq!(building.anchors[0].target.as_deref(), Some("road"));
    }

    #[test]
    fn rules_mode_is_deterministic_across_seed_set() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 2_000,
            depth_m: 2_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let rules_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("assets/objects");
        let style = serde_json::json!({
            "rulesMode": "rules",
            "rulesDir": rules_dir,
            "landUseProfile": { "theme": "temperate-plain", "urbanCoreRatio": 0.3 }
        });
        for seed in [1_u64, 42, 99, 20260902] {
            let a = generate_entities_with_profile(
                seed,
                &scene,
                &[],
                region_geometry(2_000, 2_000),
                &style,
            );
            let b = generate_entities_with_profile(
                seed,
                &scene,
                &[],
                region_geometry(2_000, 2_000),
                &style,
            );
            assert_eq!(
                serde_json::to_vec(&a).unwrap(),
                serde_json::to_vec(&b).unwrap()
            );
            let mut ids = std::collections::BTreeSet::new();
            assert!(a.iter().all(|entity| ids.insert(entity.entity_id.clone())));
        }
    }

    #[test]
    fn explicit_intent_uses_generic_settlement_planners() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 2_000,
            depth_m: 1_600,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let river_style = serde_json::json!({
            "terrainProfile": "plains",
            "water": { "waterType": "river", "levelPolicy": "horizontal-datum", "levelM": 0 },
            "generationIntent": { "terrain": "river", "settlement": "city", "water": "river" },
            "landUseProfile": { "theme": "river-delta", "urbanCoreRatio": 0.4 }
        });
        let river_geometry = water_geometry(WaterKind::River, &[], &scene, &river_style);
        let river_entities =
            generate_entities_with_profile(7, &scene, &[], river_geometry, &river_style);
        assert!(
            river_entities
                .iter()
                .any(|entity| entity.kind == "commercial_center")
        );
        assert!(
            river_entities
                .iter()
                .any(|entity| entity.entity_id.starts_with("generated.intent-"))
        );
        for entity in river_entities
            .iter()
            .filter(|entity| is_building_kind(&entity.kind))
        {
            assert!(
                entity.world_y >= footprint_surface_ceiling_m(7, entity, river_geometry),
                "{} is below its footprint surface",
                entity.entity_id
            );
        }

        let pastoral_style = serde_json::json!({
            "terrainProfile": "steppe",
            "generationIntent": { "terrain": "steppe", "settlement": "pastoral", "water": "none" }
        });
        let pastoral_entities = generate_entities_with_profile(
            7,
            &scene,
            &[],
            legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
            &pastoral_style,
        );
        assert!(
            pastoral_entities
                .iter()
                .any(|entity| entity.kind == "pasture")
        );
        assert!(
            pastoral_entities
                .iter()
                .any(|entity| entity.kind == "farmland")
        );
        assert!(
            pastoral_entities
                .iter()
                .any(|entity| entity.kind == "residential_home")
        );
    }

    #[test]
    fn fallback_lake_geometry_keeps_shore_roads_inside_scene() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 1_500,
            depth_m: 1_200,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "terrainProfile": "plains",
            "water": { "waterType": "lake", "levelPolicy": "horizontal-datum" }
        });
        let geometry = water_geometry(WaterKind::Lake, &[], &scene, &style);
        assert!(geometry.center_z - geometry.half_depth > scene.origin_z as f64);
        assert!(
            geometry.center_z + geometry.half_depth
                < (scene.origin_z + scene.depth_m as i32) as f64
        );
        let entities = generate_entities_with_profile(5, &scene, &[], geometry, &style);
        let road = entities
            .iter()
            .find(|entity| entity.entity_id == "generated.north-shore-road")
            .expect("lake template should provide a shore road");
        assert!(road.world_z >= scene.origin_z);
        assert!(road.world_z < scene.origin_z + scene.depth_m as i32);
        assert!(road.world_y >= 1);
    }

    #[test]
    fn explicit_wilderness_intent_does_not_add_urban_fabric() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "terrainProfile": "grassland",
            "naturalOnly": true,
            "generationIntent": { "terrain": "plain", "settlement": "wilderness", "water": "none" }
        });
        let entities = generate_entities_with_profile(
            11,
            &scene,
            &[],
            legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
            &style,
        );
        assert!(!entities.iter().any(|entity| is_building_kind(&entity.kind)));
        assert!(!entities.iter().any(|entity| entity.kind == "road"));
        assert!(entities.iter().any(|entity| entity.kind == "grass_clump"));
    }

    #[test]
    fn natural_only_lake_excludes_the_legacy_human_template() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "terrainProfile": "mountain-foot",
            "naturalOnly": true,
            "water": {"waterType": "lake", "levelPolicy": "horizontal-datum"}
        });
        let geometry = water_geometry(WaterKind::Lake, &[], &scene, &style);
        let entities = generate_entities_with_profile(11, &scene, &[], geometry, &style);
        assert!(entities.iter().all(|entity| natural_only::is_natural_kind(&entity.kind)));
        assert!(!entities.iter().any(|entity| {
            matches!(
                entity.kind.as_str(),
                "road" | "sidewalk" | "bridge" | "building_cluster"
            )
        }));
        assert!(entities.iter().any(|entity| entity.kind == "tree"));
    }

    #[test]
    fn natural_only_rejects_an_authored_human_landmark() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "terrainProfile": "steppe",
            "naturalOnly": true
        });
        let geometry = legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0);
        let cabin: LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId": "authored.cabin",
            "name": "cabin",
            "type": "building",
            "purpose": "test",
            "description": "must not enter a natural-only stage",
            "sceneId": scene.scene_id,
            "worldX": 128,
            "worldZ": 128,
            "worldY": 0,
            "widthM": 8,
            "depthM": 8,
            "heightM": 4,
            "assetId": "prop.building"
        }))
        .unwrap();
        let error = generate_entities_with_profile_report(
            11,
            &scene,
            &[cabin],
            geometry,
            &style,
            None,
            None,
            None,
        )
        .expect_err("a human landmark must fail closed");
        assert!(error.to_string().contains("natural-only audit failed"));
    }

    #[test]
    fn waterfront_does_not_place_land_entities_in_water() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        for entity in generate_entities(42, &scene, &[], WaterKind::Lake) {
            if !matches!(entity.kind.as_str(), "tree" | "bush" | "rock") {
                continue;
            }
            let dx = (f64::from(entity.world_x) - 520.0) / 380.0;
            let dz = (f64::from(entity.world_z) - 470.0) / 260.0;
            assert!(
                dx * dx + dz * dz >= 1.0,
                "{} spawned in water",
                entity.entity_id
            );
        }
    }

    #[test]
    fn waterfront_detail_entities_stay_on_valid_surfaces() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let entities = generate_entities(42, &scene, &[], WaterKind::Lake);
        assert!(entities.iter().any(|entity| entity.kind == "reed"));
        assert!(entities.iter().any(|entity| entity.kind == "bench"));
        for entity in entities {
            let dx = (f64::from(entity.world_x) - 520.0) / 380.0;
            let dz = (f64::from(entity.world_z) - 470.0) / 260.0;
            let radius = dx * dx + dz * dz;
            if entity.kind == "reed" {
                assert!(
                    radius > 0.9 && radius < SHORE_RADIUS + 0.05,
                    "{} left the shoreline",
                    entity.entity_id
                );
            } else if matches!(
                entity.kind.as_str(),
                "grass_clump" | "bench" | "lamp" | "fallen_log"
            ) {
                assert!(radius >= 1.22, "{} spawned in water", entity.entity_id);
            }
        }
    }

    #[test]
    fn building_grounding_uses_the_highest_footprint_sample() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let mut entity = EntityInstance {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "test.building".into(),
            asset_id: "prop.building".into(),
            kind: "building".into(),
            world_x: 400,
            world_z: 400,
            world_y: -50,
            scale: 1.0,
            width_m: 80.0,
            depth_m: 60.0,
            height_m: 12.0,
        };
        let expected = footprint_surface_ceiling_m(20260829, &entity, water);
        rebase_buildings_to_surface(20260829, water, std::slice::from_mut(&mut entity));
        assert_eq!(entity.world_y, expected);
        let carve = plan_terrain_carves(&[entity.clone()]);
        let half_w = (f64::from(entity.width_m) * 0.5).ceil() as i32;
        let half_d = (f64::from(entity.depth_m) * 0.5).ceil() as i32;
        let mut highest_baked = f64::NEG_INFINITY;
        for dz in -half_d..=half_d {
            for dx in -half_w..=half_w {
                let baked = f64::from(terrain_height_with_geometry_carved(
                    20260829,
                    entity.world_x + dx,
                    entity.world_z + dz,
                    water,
                    &carve,
                )) / 4.0;
                highest_baked = highest_baked.max(baked);
                assert!(
                    baked <= f64::from(expected) + 0.25,
                    "building terrain intersects the footprint at ({dx},{dz}): {baked} > {expected}"
                );
            }
        }
        assert!(
            highest_baked >= f64::from(expected) - 0.25,
            "building grounding did not reach the highest footprint sample"
        );
    }

    #[test]
    fn rotated_building_uses_world_footprint_for_grounding_and_pad() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let mut entity = EntityInstance {
            rotation_y_deg: 90.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "test.rotated-building".into(),
            asset_id: "prop.building".into(),
            kind: "building".into(),
            world_x: 400,
            world_z: 400,
            world_y: -50,
            scale: 1.0,
            width_m: 80.0,
            depth_m: 20.0,
            height_m: 12.0,
        };
        let mut expected = f64::NEG_INFINITY;
        for dz in -40..=40 {
            for dx in -10..=10 {
                expected = expected.max(
                    f64::from(terrain_height_with_geometry(
                        20260903,
                        entity.world_x + dx,
                        entity.world_z + dz,
                        water,
                    )) / 4.0,
                );
            }
        }
        rebase_buildings_to_surface(20260903, water, std::slice::from_mut(&mut entity));
        assert_eq!(entity.world_y, expected.ceil() as i32);
        let carves = plan_terrain_carves(&[entity.clone()]);
        assert_eq!(
            terrain_height_with_geometry_carved(20260903, 400, 439, water, &carves),
            entity.world_y as i16 * 4,
            "rotated building pad did not extend along world Z"
        );
        assert_eq!((carves[0].half_w, carves[0].half_d), (11., 41.));
        assert_eq!(
            terrain_disturbance_at(439, 400, &carves),
            0,
            "world X must be an embankment, not the rotated building support pad"
        );
        // Adaptive earthwork now legitimately extends beyond the old 6m ring.
        // Outside its computed support radius, the natural terrain is unchanged.
        let far_x = 400 + (carves[0].half_w + carves[0].blend_m).ceil() as i32 + 1;
        assert_eq!(
            terrain_height_with_geometry_carved(20260903, far_x, 400, water, &carves),
            terrain_height_with_geometry_carved(20260903, far_x, 400, water, &[]),
            "rotated building fill altered its far field"
        );
    }

    #[test]
    fn terrain_semantics_match_the_baked_raster_contract() {
        let water = legacy_water_geometry(WaterKind::None, 128, 128, 0.0);
        let carve = TerrainCarve {
            profile: None,
            cx: 32.0,
            cz: 32.0,
            half_w: 12.0,
            half_d: 8.0,
            target_h_m: 18.0,
            blend_m: 4.0,
            priority: 10,
            disturbance: 255,
            cut_cap_mm: None,
        
            ramp: None,
};
        let (height, surface, lod2, terrain) =
            generate_chunk_with_geometry(20260903, 0, 0, 64, 64, water, std::slice::from_ref(&carve));
        assert_eq!(height.len(), 64 * 64 * 2);
        assert_eq!(surface.len(), 64 * 64);
        assert_eq!(lod2.len(), 3);
        assert_eq!(terrain.len(), 64 * 64 * TERRAIN_SEMANTIC_BYTES_PER_CELL);
        let centre = (32 * 64 + 32) * TERRAIN_SEMANTIC_BYTES_PER_CELL;
        assert_eq!(
            terrain[centre + 3],
            255,
            "roadbed disturbance was not serialized"
        );
        assert_ne!(terrain[centre], 255, "flat roadbed slope was not encoded");
        assert!(
            terrain[centre + 2] > 0,
            "wetness encoding must be populated"
        );
        let outside = (63 * 64 + 63) * TERRAIN_SEMANTIC_BYTES_PER_CELL;
        assert_eq!(
            terrain[outside + 3],
            0,
            "natural ground was marked disturbed"
        );
    }

    #[test]
    fn mesoscale_relief_is_continuous_and_nontrivial() {
        let water = legacy_water_geometry(WaterKind::None, 2_000, 2_000, 0.0);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut max_adjacent_step = 0.0_f64;
        for z in (0..=512).step_by(8) {
            for x in (0..=512).step_by(8) {
                let here = mesoscale_relief(20260903, x, z, water);
                min = min.min(here);
                max = max.max(here);
                max_adjacent_step = max_adjacent_step
                    .max((here - mesoscale_relief(20260903, x + 1, z, water)).abs());
                max_adjacent_step = max_adjacent_step
                    .max((here - mesoscale_relief(20260903, x, z + 1, water)).abs());
            }
        }
        assert!(
            max - min > 3.0,
            "mid-scale terrain relief is visually negligible"
        );
        assert!(
            max_adjacent_step < 0.5,
            "mid-scale terrain introduced a discontinuity"
        );
    }

    #[test]
    fn filtered_carves_keep_all_payload_bytes_including_derivative_halo() {
        for (base_x, base_z) in [(0, 0), (-100, 200)] {
            let water = legacy_water_geometry(WaterKind::None, 512, 512, 0.);
            let carves = vec![
                TerrainCarve {
                    profile: None,
                    cx: f64::from(base_x) + 34.,
                    cz: f64::from(base_z) + 16.,
                    half_w: 1.,
                    half_d: 8.,
                    target_h_m: 100.,
                    blend_m: 1.5,
                    priority: 1,
                    disturbance: 160,
                    cut_cap_mm: None,
                
                    ramp: None,
},
                TerrainCarve {
                    profile: None,
                    cx: f64::from(base_x) - 2000.,
                    cz: f64::from(base_z) - 2000.,
                    half_w: 1.,
                    half_d: 1.,
                    target_h_m: 100.,
                    blend_m: 1.5,
                    priority: 1,
                    disturbance: 160,
                    cut_cap_mm: None,
                
                    ramp: None,
},
            ];
            let filtered: Vec<_> = carves
                .iter()
                .cloned()
                .filter(|c| carve_intersects_chunk(c, base_x, base_z, 32, 32))
                .collect();
            assert_eq!(filtered.len(), 1);
            assert_eq!(
                generate_chunk_with_geometry(7, base_x, base_z, 32, 32, water, &carves),
                generate_chunk_with_geometry(7, base_x, base_z, 32, 32, water, &filtered)
            );
        }
    }

    #[test]
    fn chunk_carve_filter_excludes_distant_pads_and_keeps_blend_ring() {
        let near = TerrainCarve {
            profile: None,
            cx: 520.,
            cz: 520.,
            half_w: 8.,
            half_d: 8.,
            target_h_m: 2.,
            blend_m: 6.,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,
        
            ramp: None,
};
        let far = TerrainCarve {
            profile: None,
            cx: 2_000.,
            cz: 2_000.,
            half_w: 8.,
            half_d: 8.,
            target_h_m: 2.,
            blend_m: 6.,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,
        
            ramp: None,
};
        assert!(carve_intersects_chunk(&near, 0, 0, 512, 512));
        assert!(!carve_intersects_chunk(&far, 0, 0, 512, 512));
        assert!(carve_intersects_chunk(&near, 512, 512, 16, 16));
    }

    #[test]
    fn planned_fill_does_not_make_the_observed_chunk_boundary_cliff() {
        let water = legacy_water_geometry(WaterKind::None, 1800, 1800, 0.0);
        let building = generated_intent_entity(
            "generated.test".into(),
            "residential_home",
            1520,
            842,
            1,
            22.,
            22.,
            0.2,
        );
        let carves = plan_terrain_carves(&[building]);
        assert!(carves[0].blend_m >= 96.);
        for z in 830..870 {
            let left = terrain_height_with_geometry_carved(20260905, 1535, z, water, &carves);
            let right = terrain_height_with_geometry_carved(20260905, 1536, z, water, &carves);
            assert!(
                (left - right).abs() <= 16,
                "quarter-metre seam step at {z}: {}",
                (left - right).abs()
            );
        }
    }

    #[test]
    fn overlapping_fill_envelopes_are_order_independent() {
        let water = legacy_water_geometry(WaterKind::None, 1000, 1000, 0.);
        let a = TerrainCarve {
            profile: None,
            cx: 400.,
            cz: 400.,
            half_w: 10.,
            half_d: 10.,
            target_h_m: 12.,
            blend_m: 80.,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,
        
            ramp: None,
};
        let b = TerrainCarve {
            profile: None,
            cx: 410.,
            cz: 412.,
            target_h_m: 16.,
            ..a.clone()
        };
        for z in (380..490).step_by(3) {
            for x in (380..490).step_by(3) {
                assert_eq!(
                    terrain_height_with_geometry_carved(42, x, z, water, &[a.clone(), b.clone()]),
                    terrain_height_with_geometry_carved(42, x, z, water, &[b.clone(), a.clone()])
                );
            }
        }
    }

    #[test]
    fn generated_pads_never_excavate_higher_natural_ground() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let natural = terrain_height_with_geometry_carved(20260829, 400, 400, water, &[]);
        let carve = TerrainCarve {
            profile: None,
            cx: 400.0,
            cz: 400.0,
            half_w: 50.0,
            half_d: 40.0,
            target_h_m: f64::from(natural) / 4.0 - 20.0,
            blend_m: 6.0,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,
        
            ramp: None,
};
        assert_eq!(
            terrain_height_with_geometry_carved(20260829, 400, 400, water, std::slice::from_ref(&carve)),
            natural,
            "generated pad excavated natural ground"
        );
    }

    #[test]
    fn terrain_carve_flattens_pad_and_blends_to_natural() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let carve = TerrainCarve {
            profile: None,
            cx: 400.0,
            cz: 400.0,
            half_w: 50.0,
            half_d: 40.0,
            target_h_m: 12.0,
            blend_m: 4.0,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,
        
            ramp: None,
};
        // Inside the pad the height is the flat target, independent of the
        // underlying natural terrain (which varies with position).
        for (x, z) in [(350, 360), (400, 400), (449, 439), (370, 425)] {
            assert_eq!(
                terrain_height_with_geometry_carved(20260829, x, z, water, std::slice::from_ref(&carve)),
                12 * 4,
                "pad at ({x},{z}) not flattened to target"
            );
        }
        // Far outside the pad the natural terrain is untouched.
        let natural_at = terrain_height_with_geometry_carved(20260829, 700, 700, water, &[]);
        assert_eq!(
            terrain_height_with_geometry_carved(20260829, 700, 700, water, std::slice::from_ref(&carve)),
            natural_at,
            "far field altered by carve"
        );
        // Inside the pad the terrain is flat (zero local slope).
        let a = terrain_height_with_geometry_carved(20260829, 380, 400, water, std::slice::from_ref(&carve));
        let b = terrain_height_with_geometry_carved(20260829, 420, 400, water, std::slice::from_ref(&carve));
        let c = terrain_height_with_geometry_carved(20260829, 400, 380, water, std::slice::from_ref(&carve));
        assert_eq!(a, b, "pad not flat along X");
        assert_eq!(a, c, "pad not flat along Z");
        // The blend ring is continuous (each step bounded) so no cliff appears.
        let mut max_step = 0_i16;
        for z in 430..450 {
            for x in 440..460 {
                let here = terrain_height_with_geometry_carved(20260829, x, z, water, std::slice::from_ref(&carve));
                max_step = max_step.max(
                    (here
                        - terrain_height_with_geometry_carved(20260829, x - 1, z, water, std::slice::from_ref(&carve)))
                    .abs(),
                );
                max_step = max_step.max(
                    (here
                        - terrain_height_with_geometry_carved(20260829, x, z - 1, water, std::slice::from_ref(&carve)))
                    .abs(),
                );
            }
        }
        // The blend ring must stay monotonic and bounded: over a steep natural
        // valley the smoothstep can legitimately drop many metres per cell,
        // but never a vertical cliff (which would be tens of metres in one
        // step). Quarter-m units: 30m/step is the absolute worst-case bound.
        assert!(
            max_step <= 120,
            "carve blend ring has a discontinuity of {max_step} quarter-meters"
        );
    }

    #[test]
    fn approximate_valley_carves_valleys_but_keeps_city_flat() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        // City centre: high urbanisation => erosion must be zero so the built
        // area and its terrain stay flat (a city sits on a level pad).
        let mut urban_cells = 0;
        let mut carved_sum = 0.0;
        for z in (450..550).step_by(10) {
            for x in (450..550).step_by(10) {
                let c = approximate_valley_carve(20260829, x, z, water);
                if c != 0.0 {
                    urban_cells += 1;
                    carved_sum += c;
                }
            }
        }
        // Some cells in the very heart may still be rural by noise, but the
        // majority of the central 100x100 must be untouched.
        assert!(carved_sum <= 0.0, "erosion must not raise terrain");
        assert!(
            urban_cells <= 40,
            "erosion touched {urban_cells} central city cells (expected <= 40)"
        );
        // Erosion never raises terrain, only carves down.
        for z in (100..900).step_by(23) {
            for x in (100..900).step_by(23) {
                assert!(
                    approximate_valley_carve(20260829, x, z, water) <= 0.0,
                    "erosion raised terrain at ({x},{z})"
                );
            }
        }
        // It carves somewhere in wild land (valleys exist off the centre).
        let wild_carved = (100..900)
            .step_by(7)
            .flat_map(|z| {
                (100..900)
                    .step_by(7)
                    .map(move |x| approximate_valley_carve(20260829, x, z, water))
            })
            .filter(|c| *c < -2.0)
            .count();
        assert!(
            wild_carved > 100,
            "erosion barely carved wild terrain ({wild_carved} deep cells)"
        );
        // Erosion stays continuous across chunk boundaries (it is a pure
        // function of world coordinates).
        let a = approximate_valley_carve(20260829, 511, 200, water);
        let b = approximate_valley_carve(20260829, 512, 200, water);
        let c = approximate_valley_carve(20260829, 200, 511, water);
        let d = approximate_valley_carve(20260829, 200, 512, water);
        assert!((a - b).abs() < 4.0, "erosion jumps across X chunk seam");
        assert!((c - d).abs() < 4.0, "erosion jumps across Z chunk seam");
    }

    #[test]
    fn waterfront_water_is_level_and_terrain_is_continuous() {
        // The water surface is the fixed 0 datum; the lake bed must sit below
        // it (negative height) so the water has depth and never floats above
        // the shore. Every cell in the lake interior must be below datum.
        for z in 300..340 {
            for x in 500..540 {
                assert!(
                    terrain_height(1208, x, z, WaterKind::Lake, 1_000, 1_000, 0.0) < 0,
                    "lake bed at ({x},{z}) is not below the water datum"
                );
            }
        }
        // Shoreline must rise above datum so the sea cannot flood dry land.
        // Lake: centre (520,470), half width 380 / half depth 260, so the
        // waterline sits at radius 1.0 and dry shore past radius 1.25.
        for (x, z) in [(990, 470), (520, 806), (50, 470), (520, 134)] {
            let r = {
                let dx = (x as f64 - 520.0) / 380.0;
                let dz = (z as f64 - 470.0) / 260.0;
                dx * dx + dz * dz
            };
            assert!(r > 1.25, "test point ({x},{z}) r={r:.2} not on dry shore");
            assert!(
                terrain_height(1208, x, z, WaterKind::Lake, 1_000, 1_000, 0.0) > 0,
                "shore at ({x},{z}) is below the water datum"
            );
        }
        let mut max_step = 0_i16;
        for z in 1..999 {
            for x in 1..999 {
                let here = terrain_height(1208, x, z, WaterKind::Lake, 1_000, 1_000, 0.0);
                max_step = max_step.max(
                    (here - terrain_height(1208, x - 1, z, WaterKind::Lake, 1_000, 1_000, 0.0))
                        .abs(),
                );
                max_step = max_step.max(
                    (here - terrain_height(1208, x, z - 1, WaterKind::Lake, 1_000, 1_000, 0.0))
                        .abs(),
                );
            }
        }
        assert!(
            max_step <= 16,
            "terrain has a discontinuity of {max_step} quarter-meters"
        );
    }

    #[test]
    fn water_crossing_is_detected_between_legacy_samples() {
        let water = WaterGeometry {
            kind: WaterKind::River,
            center_x: 100.0,
            center_z: 100.0,
            half_width: 8.0,
            half_depth: 100.0,
            scene_width_m: 200,
            scene_depth_m: 200,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let entity = EntityInstance {
            entity_id: "edge-crossing".into(),
            asset_id: "test".into(),
            kind: "building".into(),
            world_x: 130,
            world_z: 100,
            world_y: 0,
            scale: 1.0,
            width_m: 100.0,
            depth_m: 400.0,
            height_m: 10.0,
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        assert!(footprint_intersects_water(&entity, water, 0.0));
    }

    #[test]
    fn river_landmark_keeps_entities_out_of_water() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 5_000,
            depth_m: 2_400,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let river = LandmarkSpec {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "landmark.yangtze-river".to_owned(),
            asset_id: "water.yangtze-river".to_owned(),
            entity_type: "river".to_owned(),
            scene_id: "scene-0".to_owned(),
            world_x: 2_500,
            world_z: 1_200,
            world_y: 0,
            width_m: 1_400,
            depth_m: 2_400,
            height_m: 1,
            name: "长江河道".to_owned(),
            purpose: "提供主河道水面".to_owned(),
            description: "沿世界Z 轴贯穿场景的长江河道".to_owned(),
        };
        let half = river_half_width_m(&[river.clone()], scene.width_m);
        assert_eq!(half, 700.0);
        for entity in generate_entities(42, &scene, &[river], WaterKind::River) {
            if matches!(
                entity.kind.as_str(),
                "tree"
                    | "bush"
                    | "rock"
                    | "building"
                    | "bench"
                    | "lamp"
                    | "grass_clump"
                    | "fallen_log"
            ) {
                let distance = (f64::from(entity.world_x) - 2_500.0).abs();
                assert!(
                    distance >= 700.0,
                    "{} spawned inside river water at distance {distance}",
                    entity.entity_id
                );
            }
        }
    }

    #[test]
    fn authored_land_building_intersecting_river_fails_closed() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let river = LandmarkSpec {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "landmark.river".into(),
            asset_id: "water.river".into(),
            entity_type: "river".into(),
            scene_id: "scene-0".into(),
            world_x: 500,
            world_z: 500,
            world_y: 0,
            width_m: 300,
            depth_m: 600,
            height_m: 1,
            name: "test river".into(),
            purpose: "water".into(),
            description: "finite test river".into(),
        };
        let building = LandmarkSpec {
            entity_type: "shopping_mall".into(),
            entity_id: "landmark.mall-on-water".into(),
            asset_id: "building.mall".into(),
            name: "bad mall".into(),
            purpose: "negative test".into(),
            description: "must be rejected".into(),
            scene_id: "scene-0".into(),
            world_x: 500,
            world_z: 500,
            world_y: 100,
            width_m: 80,
            depth_m: 60,
            height_m: 20,
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        let water = water_geometry(WaterKind::River, &[river], &scene, &serde_json::Value::Null);
        assert!(matches!(
            validate_authored_landmarks_clear_water(&[building], &scene, water),
            Err(WorldgenError::LandmarkIntersectsWater { .. })
        ));
    }

    #[test]
    fn river_membership_is_finite_in_world_z_for_placement() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let water = WaterGeometry {
            kind: WaterKind::River,
            center_x: 500.0,
            center_z: 500.0,
            half_width: 100.0,
            half_depth: 200.0,
            scene_width_m: 1_000,
            scene_depth_m: 1_000,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        with_rules_placement_context(42, &scene, water, |ctx| {
            assert!(ctx.water(500, 500).is_some());
            assert!(ctx.water(500, 100).is_none());
            assert!(ctx.water(500, 900).is_none());
        });
    }

    #[test]
    fn exported_preview_contains_all_static_module_dependencies() {
        let root =
            std::env::temp_dir().join(format!("glyphweave-preview-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write_adapter_templates(&root).expect("preview templates should export");
        for relative in [
            "preview/index.html",
            "preview/app.js",
            "preview/core/shared.js",
            "preview/render/terrain.js",
            "preview/render/wind.js",
            "preview/render/presets/materials.js",
            "preview/vendor/three/LICENSE",
            "preview/vendor/three/build/three.module.js",
            "preview/vendor/three/examples/jsm/loaders/GLTFLoader.js",
            "assets/LICENSE.txt",
            "assets/THIRD_PARTY_NOTICES.md",
            "assets/licenses/quaternius-stylized-nature-CC0.txt",
            "assets/licenses/quaternius-buildings-NOTICE.txt",
        ] {
            assert!(
                root.join(relative).is_file(),
                "missing preview file: {relative}"
            );
        }
        let three_license =
            std::fs::read_to_string(root.join("preview/vendor/three/LICENSE")).unwrap();
        assert!(three_license.contains("The MIT License"));
        assert!(three_license.contains("Copyright © 2010-2025 three.js authors"));
        let building_notice =
            std::fs::read_to_string(root.join("assets/licenses/quaternius-buildings-NOTICE.txt"))
                .unwrap();
        assert!(building_notice.contains("exact local file-to-pack"));
        assert!(building_notice.contains("has not been proven"));
        std::fs::remove_dir_all(root).expect("test preview directory should be removable");
    }

    #[test]
    fn generated_natural_details_clear_road_footprints() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 2_000,
            depth_m: 1_600,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let entities = generate_entities(20260902, &scene, &[], WaterKind::River);
        let roads: Vec<_> = entities
            .iter()
            .filter(|entity| entity.kind == "road")
            .collect();
        for detail in entities.iter().filter(|entity| {
            matches!(
                entity.kind.as_str(),
                "tree" | "bush" | "grass_clump" | "rock" | "fallen_log"
            )
        }) {
            let detail_r = f64::from(detail.width_m.max(detail.depth_m)) * 0.5;
            assert!(
                !roads.iter().any(|road| {
                    f64::from((detail.world_x - road.world_x).abs())
                        < detail_r + f64::from(road.width_m) * 0.5 + 0.5
                        && f64::from((detail.world_z - road.world_z).abs())
                            < detail_r + f64::from(road.depth_m) * 0.5 + 0.5
                }),
                "{} overlaps a road",
                detail.entity_id
            );
        }
    }

    #[test]
    fn road_carve_respects_world_rotation() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let mut east_west = generated_intent_entity(
            "generated.test-road-ew".into(),
            "road",
            400,
            400,
            100,
            100.0,
            10.0,
            0.24,
        );
        east_west.rotation_y_deg = 0.0;
        let ew_carve = plan_terrain_carves(&[east_west]);
        assert_eq!(
            terrain_height_with_geometry_carved(20260903, 450, 400, water, &ew_carve),
            400,
            "east-west road did not carve along world X"
        );
        assert_ne!(
            terrain_height_with_geometry_carved(20260903, 400, 450, water, &ew_carve),
            400,
            "east-west road carve leaked along world Z"
        );

        let mut north_south = generated_intent_entity(
            "generated.test-road-ns".into(),
            "road",
            400,
            400,
            100,
            100.0,
            10.0,
            0.24,
        );
        north_south.rotation_y_deg = 90.0;
        let ns_carve = plan_terrain_carves(&[north_south]);
        assert_eq!(
            terrain_height_with_geometry_carved(20260903, 400, 450, water, &ns_carve),
            400,
            "north-south road did not carve along world Z"
        );
        assert_ne!(
            terrain_height_with_geometry_carved(20260903, 450, 400, water, &ns_carve),
            400,
            "north-south road carve leaked along world X"
        );
    }

    #[test]
    fn waterfront_civic_buildings_clear_their_access_roads() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 2_000,
            depth_m: 1_600,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "generationIntent": { "settlement": "city" }
        });
        let geometry = legacy_water_geometry(WaterKind::River, scene.width_m, scene.depth_m, 0.0);
        let entities = generate_entities_with_profile(20260903, &scene, &[], geometry, &style);
        let roads: Vec<_> = entities.iter().filter(|e| e.kind == "road").collect();
        let civic: Vec<_> = entities
            .iter()
            .filter(|e| matches!(e.kind.as_str(), "commercial_center" | "school"))
            .collect();
        assert!(!roads.is_empty());
        assert!(!civic.is_empty());
        for building in civic {
            assert!(
                !roads.iter().any(|road| {
                    f64::from((building.world_x - road.world_x).abs())
                        < spatial_half_extents(
                            building.width_m,
                            building.depth_m,
                            building.rotation_y_deg,
                        )
                        .0 + spatial_half_extents(road.width_m, road.depth_m, road.rotation_y_deg)
                            .0
                        && f64::from((building.world_z - road.world_z).abs())
                            < spatial_half_extents(
                                building.width_m,
                                building.depth_m,
                                building.rotation_y_deg,
                            )
                            .1 + spatial_half_extents(
                                road.width_m,
                                road.depth_m,
                                road.rotation_y_deg,
                            )
                            .1
                }),
                "{} overlaps its access road",
                building.entity_id
            );
        }
    }

    #[test]
    fn generated_road_uses_ground_height() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let road = generate_entities(1208, &scene, &[], WaterKind::Lake)
            .into_iter()
            .find(|entity| entity.kind == "road")
            .unwrap();
        assert_eq!(
            road.world_y,
            (i32::from(terrain_height(
                1208,
                road.world_x,
                road.world_z,
                WaterKind::Lake,
                1_000,
                1_000,
                0.0
            )) / 4)
                .max(1)
        );
    }

    #[test]
    fn water_geometry_uses_landmark_footprint_not_scene_constants() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let lake = LandmarkSpec {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: "landmark.lake".to_owned(),
            name: "Procedural lake".to_owned(),
            entity_type: "lake".to_owned(),
            purpose: "water test".to_owned(),
            description: "arbitrary lake placement".to_owned(),
            scene_id: "scene-0".to_owned(),
            world_x: 210,
            world_z: 760,
            world_y: 0,
            width_m: 300,
            depth_m: 180,
            height_m: 1,
            asset_id: "water.lake".to_owned(),
        };
        let geometry = water_geometry(WaterKind::Lake, &[lake], &scene, &serde_json::Value::Null);
        assert_eq!((geometry.center_x, geometry.center_z), (210.0, 760.0));
        assert_eq!((geometry.half_width, geometry.half_depth), (150.0, 90.0));
        assert!(lake_radius_at(210.0, 760.0, geometry) < 1.0);
        assert!(lake_radius_at(520.0, 470.0, geometry) > 1.0);
    }

    #[test]
    fn authored_geography_gives_lakes_irregular_shorelines() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let style = serde_json::json!({
            "water": {"waterType": "lake", "levelPolicy": "horizontal-datum"},
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
        });
        let geometry = water_geometry(WaterKind::Lake, &[], &scene, &style);
        let east = lake_radius_at(800.0, 500.0, geometry);
        let north_east = lake_radius_at(500.0 + 300.0 * 0.55, 500.0 - 300.0 * 0.55, geometry);
        assert!(
            (east - north_east).abs() > 0.08,
            "an authored lake must not have a perfect elliptical shoreline"
        );
    }

    #[test]
    fn generic_world_has_structured_content_without_landmarks() {
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: 1_000,
            depth_m: 1_000,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let entities = generate_entities_with_geometry(
            42,
            &scene,
            &[],
            legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
        );
        assert!(entities.iter().any(|entity| entity.kind == "road"));
        assert!(entities.iter().any(|entity| entity.kind == "building"));
        assert!(entities.iter().any(|entity| entity.kind == "tree"));
        assert!(
            entities
                .iter()
                .any(|entity| entity.asset_id == "prop.building.small")
        );
    }

    #[test]
    fn modern_profile_generates_reusable_landuse_categories() {
        let manifest = WorldManifest::default_demo();
        let scene = &manifest.scenes[0];
        let entities = generate_entities_with_profile(
            manifest.world.seed,
            scene,
            &[],
            legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
            &manifest.style,
        );
        for kind in [
            "road",
            "parking_lot",
            "commercial_center",
            "entertainment_center",
            "school",
            "residential_block",
            "green_space",
            "canal",
            "farmland",
            "mountain_forest",
            "temple",
            "church",
            "pasture",
            "nature_reserve",
        ] {
            assert!(
                entities.iter().any(|entity| entity.kind == kind),
                "modern profile did not generate {kind}"
            );
        }
        assert!(
            entities
                .iter()
                .filter(|entity| entity.kind == "road")
                .count()
                > 2
        );
    }

    fn profile_style(theme: &str) -> serde_json::Value {
        serde_json::json!({
            "landUseProfile": {
                "theme": theme,
                "urbanCoreRatio": 0.22,
                "suburbanRatio": 0.28,
                "greenRatio": 0.18,
                "farmRatio": 0.20,
                "forestRatio": 0.20,
                "pastureRatio": 0.12,
                "reserveRatio": 0.10
            }
        })
    }

    fn profile_entities(theme: &str, seed: u64, width: u32, depth: u32) -> Vec<EntityInstance> {
        let style = profile_style(theme);
        let scene = SceneSpec {
            scene_id: "scene-0".to_owned(),
            width_m: width,
            depth_m: depth,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        generate_entities_with_profile(
            seed,
            &scene,
            &[],
            legacy_water_geometry(WaterKind::None, width, depth, 0.0),
            &style,
        )
    }

    fn distance(a: (i32, i32), b: (i32, i32)) -> f64 {
        let dx = f64::from(a.0 - b.0);
        let dz = f64::from(a.1 - b.1);
        (dx * dx + dz * dz).sqrt()
    }

    #[test]
    fn city_form_profile_same_seed_is_deterministic() {
        for theme in [
            "dense-core",
            "river-delta",
            "coastal-bay",
            "mountain-valley",
            "temperate-plain",
            "low-density-suburban",
        ] {
            let a = profile_entities(theme, 20260829, 1_200, 1_200);
            let b = profile_entities(theme, 20260829, 1_200, 1_200);
            assert_eq!(a.len(), b.len(), "{theme} entity count differs");
            for (index, (left, right)) in a.iter().zip(b.iter()).enumerate() {
                assert_eq!(
                    left.entity_id, right.entity_id,
                    "{theme} id mismatch at {index}"
                );
                assert_eq!(left.world_x, right.world_x, "{theme} x mismatch at {index}");
                assert_eq!(left.world_z, right.world_z, "{theme} z mismatch at {index}");
            }
        }
    }

    #[test]
    fn every_city_form_generates_the_core_landuse_set() {
        for theme in [
            "dense-core",
            "river-delta",
            "coastal-bay",
            "mountain-valley",
            "low-density-suburban",
        ] {
            let entities = profile_entities(theme, 20260829, 1_600, 1_600);
            for kind in [
                "road",
                "commercial_center",
                "school",
                "residential_block",
                "parking_lot",
                "green_space",
                "farmland",
                "mountain_forest",
                "pasture",
                "nature_reserve",
            ] {
                assert!(
                    entities.iter().any(|entity| entity.kind == kind),
                    "{theme} did not generate {kind}"
                );
            }
        }
    }

    #[test]
    fn multi_core_profiles_place_civic_centers_near_cores() {
        for theme in ["river-delta", "coastal-bay", "mountain-valley"] {
            let style = profile_style(theme);
            let scene = SceneSpec {
                scene_id: "scene-0".to_owned(),
                width_m: 2_400,
                depth_m: 2_400,
                origin_x: 0,
                origin_z: 0,
                seed_offset: 0,
            };
            let entities = generate_entities_with_profile(
                99,
                &scene,
                &[],
                legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
                &style,
            );
            let params = CityFormParams::resolve(&style);
            let cores = city_core_positions(&params, &scene, 99);
            let commercials: Vec<_> = entities
                .iter()
                .filter(|entity| entity.kind == "commercial_center")
                .collect();
            assert!(
                commercials.len() >= 2,
                "{theme} should place >=2 commercial centers, got {}",
                commercials.len()
            );
            for commercial in commercials {
                let center = (commercial.world_x, commercial.world_z);
                let closest = cores
                    .iter()
                    .map(|core| distance(center, *core))
                    .fold(f64::MAX, f64::min);
                assert!(
                    closest < 700.0,
                    "{theme} commercial at {center:?} is {closest:.0}m from nearest core"
                );
            }
        }
    }

    #[test]
    fn schools_sit_near_residential_blocks() {
        for theme in ["dense-core", "temperate-plain", "low-density-suburban"] {
            let entities = profile_entities(theme, 20260829, 1_600, 1_600);
            let schools: Vec<_> = entities
                .iter()
                .filter(|entity| entity.kind == "school")
                .map(|entity| (entity.world_x, entity.world_z))
                .collect();
            let residentials: Vec<_> = entities
                .iter()
                .filter(|entity| entity.kind == "residential_block")
                .map(|entity| (entity.world_x, entity.world_z))
                .collect();
            assert!(!schools.is_empty(), "{theme} has no school");
            assert!(!residentials.is_empty(), "{theme} has no residential block");
            for school in &schools {
                let nearest_residential = residentials
                    .iter()
                    .map(|residential| distance(*school, *residential))
                    .fold(f64::MAX, f64::min);
                assert!(
                    nearest_residential < 500.0,
                    "{theme} school {school:?} is {nearest_residential:.0}m from nearest residence"
                );
            }
        }
    }

    #[test]
    fn farmland_appears_outside_the_urban_core() {
        for theme in ["temperate-plain", "river-delta", "low-density-suburban"] {
            let entities = profile_entities(theme, 20260829, 1_600, 1_600);
            let center_x = 800.0_f64;
            let center_z = 800.0_f64;
            let farmland: Vec<_> = entities
                .iter()
                .filter(|entity| entity.kind == "farmland")
                .collect();
            assert!(
                farmland.len() >= 3,
                "{theme} should place several farmland parcels"
            );
            let max_radius = farmland
                .iter()
                .map(|parcel| {
                    let dx = f64::from(parcel.world_x) - center_x;
                    let dz = f64::from(parcel.world_z) - center_z;
                    (dx * dx + dz * dz).sqrt()
                })
                .fold(0.0_f64, f64::max);
            assert!(
                max_radius > 300.0,
                "{theme} farmland never leaves the urban core (max radius {max_radius:.0}m)"
            );
        }
    }

    #[test]
    fn landuse_area_breakdown_is_consistent_and_deterministic() {
        let manifest = WorldManifest::default_demo();
        let scene = &manifest.scenes[0];
        let make_index = |seed: u64| {
            let entities = generate_entities_with_profile(
                seed,
                scene,
                &[],
                legacy_water_geometry(WaterKind::None, scene.width_m, scene.depth_m, 0.0),
                &manifest.style,
            );
            SceneIndex {
                land_use_budget: None,
                land_use_zones: Vec::new(),
                settlement_graph: None,
                parcel_plan: None,
                road_network_plan: None,
                development_plan: None,
                landcover_plan: None,
                landcover_surfaces: None,
                development_proposals: Vec::new(),
                natural_terrain: NaturalTerrainSnapshot::authored_water(
                    42,
                    &scene.scene_id,
                    scene.width_m,
                    scene.depth_m,
                    "none",
                    0.0,
                    0.0,
                ),
                engineered_terrain: EngineeredTerrainSnapshot::default(),
                scene_id: scene.scene_id.clone(),
                width_m: scene.width_m,
                depth_m: scene.depth_m,
                origin_x: scene.origin_x,
                origin_z: scene.origin_z,
                chunk_size_m: STREAM_CHUNK_METERS,
                chunk_count_x: scene.width_m.div_ceil(STREAM_CHUNK_METERS),
                chunk_count_z: scene.depth_m.div_ceil(STREAM_CHUNK_METERS),
                chunks: Vec::new(),
                landmarks: Vec::new(),
                access_graph: AccessGraph::default(),
                access_surfaces: Vec::new(),
                entities,
                rule_building_candidates: Vec::new(),
            }
        };
        let first = analyze_landuse_areas(&make_index(42));
        let second = analyze_landuse_areas(&make_index(42));
        assert_eq!(first.urban_m2, second.urban_m2);
        assert_eq!(first.rural_m2, second.rural_m2);
        assert_eq!(first.nature_m2, second.nature_m2);
        let scene_area = f64::from(scene.width_m) * f64::from(scene.depth_m);
        assert!((first.scene_area_m2 - scene_area).abs() < 1.0);
        // Every land-use category is present and the sums stay inside the
        // scene area (allowing a small overlap margin).
        assert!(first.urban_m2 > 0.0);
        assert!(first.rural_m2 > 0.0);
        assert!(first.nature_m2 > 0.0);
        assert!(
            (first.urban_m2 + first.rural_m2 + first.nature_m2) <= scene_area * 1.25,
            "land-use areas overflow the scene: urban={} rural={} nature={} scene={}",
            first.urban_m2,
            first.rural_m2,
            first.nature_m2,
            scene_area
        );
        // The urban band must not be a rounding artifact: it should cover a
        // meaningful share of the scene on a normal profile.
        assert!(
            first.urban_ratio > 0.03,
            "urban ratio {:.3} is too small to be meaningful",
            first.urban_ratio
        );
    }

    #[test]
    fn loose_suburban_profile_uses_larger_blocks_and_sparser_roads() {
        // Use a large scene so the urban core actually materialises instead of
        // being swallowed by the wild rim.
        let dense = profile_entities("dense-core", 7, 3_000, 3_000);
        let loose = profile_entities("low-density-suburban", 7, 3_000, 3_000);
        let dense_roads = dense.iter().filter(|e| e.kind == "road").count();

        let loose_roads = loose.iter().filter(|e| e.kind == "road").count();
        assert!(
            loose_roads < dense_roads,
            "low-density-suburban should have fewer roads ({loose_roads} vs {dense_roads})"
        );
        // A city form produces a vertical gradient: high-rise core towers and
        // low-rise suburban homes coexist, so the dense core must be clearly
        // taller than its own suburban fringe.
        let dense_heights: Vec<f32> = dense
            .iter()
            .filter(|e| e.kind == "residential_tower" || e.kind == "residential_block")
            .map(|e| e.height_m)
            .collect();
        let loose_heights: Vec<f32> = loose
            .iter()
            .filter(|e| e.kind == "residential_home" || e.kind == "residential_block")
            .map(|e| e.height_m)
            .collect();
        let dense_max = dense_heights.iter().cloned().fold(0.0_f32, f32::max);
        let low_share = |heights: &[f32]| {
            if heights.is_empty() {
                return 0.0_f32;
            }
            heights.iter().filter(|h| **h <= 12.0).count() as f32 / heights.len() as f32
        };
        assert!(
            dense_max >= 16.0,
            "dense-core should build mid/high-rise towers, got max {dense_max:.1}m"
        );
        // The suburban profile should be dominated by low-rise detached homes.
        assert!(
            low_share(&loose_heights) > 0.5,
            "low-density-suburban should be mostly low-rise, got low-share {:.2}",
            low_share(&loose_heights)
        );
        // The dense core should have fewer low-rise homes than the loose form.
        assert!(
            low_share(&dense_heights) < low_share(&loose_heights),
            "dense-core low-share {:.2} should be below low-density {:.2}",
            low_share(&dense_heights),
            low_share(&loose_heights)
        );
        // The suburban profile should spread more yards / greenspace.
        let dense_green = dense.iter().filter(|e| e.kind == "green_space").count();
        let loose_green = loose.iter().filter(|e| e.kind == "green_space").count();
        assert!(
            loose_green >= dense_green,
            "low-density-suburban should have at least as much greenspace ({loose_green} vs {dense_green})"
        );
    }

    #[test]
    fn landform_field_is_continuous_across_chunk_boundaries() {
        let geometry = WaterGeometry {
            kind: WaterKind::None,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: 6_000,
            scene_depth_m: 10_000,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let mut max_step = 0_i16;
        // Walk across a chunk seam at x=512 and confirm heights stay continuous.
        for z in 200..220 {
            for x in 500..524 {
                let here = terrain_height_with_geometry(20260829, x, z, geometry);
                max_step = max_step
                    .max((here - terrain_height_with_geometry(20260829, x - 1, z, geometry)).abs());
                max_step = max_step
                    .max((here - terrain_height_with_geometry(20260829, x, z - 1, geometry)).abs());
            }
        }
        assert!(
            max_step <= 16,
            "landform terrain steps {max_step} quarter-meters across a chunk seam (limit 16)"
        );
    }

    #[test]
    fn landform_produces_more_than_one_zone_on_a_large_scene() {
        let geometry = WaterGeometry {
            kind: WaterKind::None,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: 6_000,
            scene_depth_m: 10_000,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let mut zones = BTreeSet::new();
        for z in (500..9_500).step_by(400) {
            for x in (500..5_500).step_by(400) {
                zones.insert(Landform::classify(20260829, x, z, geometry));
            }
        }
        assert!(
            zones.len() >= 2,
            "a 6km x 10km scene should contain at least two landform zones, got {zones:?}"
        );
    }

    #[test]
    fn modern_profile_generates_realistic_civic_and_industrial_types() {
        let entities = profile_entities("temperate-plain", 20260829, 2_400, 2_400);
        for kind in ["town_hall", "market", "industrial", "water_well"] {
            assert!(
                entities.iter().any(|entity| entity.kind == kind),
                "modern profile did not generate {kind}"
            );
        }
        // Town hall sits in the core, industrial sits well away from it.
        let town = entities
            .iter()
            .find(|entity| entity.kind == "town_hall")
            .unwrap();
        let center_x = 1_200.0_f64;
        let center_z = 1_200.0_f64;
        let dx = f64::from(town.world_x) - center_x;
        let dz = f64::from(town.world_z) - center_z;
        assert!(
            (dx * dx + dz * dz).sqrt() < 900.0,
            "town hall at ({},{}) should be near the core",
            town.world_x,
            town.world_z
        );
        for industrial in entities.iter().filter(|entity| entity.kind == "industrial") {
            let dix = f64::from(industrial.world_x) - center_x;
            let diz = f64::from(industrial.world_z) - center_z;
            assert!(
                (dix * dix + diz * diz).sqrt() > 300.0,
                "industrial at ({},{}) should be off the core",
                industrial.world_x,
                industrial.world_z
            );
        }
    }

    #[test]
    fn farmland_avoids_high_land_and_forest_prefers_high_land() {
        let entities = profile_entities("mountain-valley", 20260829, 3_000, 3_000);
        let geometry = WaterGeometry {
            kind: WaterKind::None,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: 3_000,
            scene_depth_m: 3_000,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        };
        let mut farmland_on_mountain = 0_usize;
        let mut farmland_total = 0_usize;
        for entity in &entities {
            if entity.kind != "farmland" {
                continue;
            }
            farmland_total += 1;
            let landform = Landform::classify(20260829, entity.world_x, entity.world_z, geometry);
            if matches!(landform, Landform::Hill | Landform::Mountain) {
                farmland_on_mountain += 1;
            }
        }
        assert!(
            farmland_total >= 5,
            "mountain-valley should still grow farmland on its plains, got {farmland_total}"
        );
        assert!(
            farmland_on_mountain <= farmland_total / 3,
            "too much farmland on high ground ({farmland_on_mountain}/{farmland_total})"
        );
        // Forest density per landform area: highland cells should be forest
        // at a higher rate than lowland cells. Count sampled cells per zone.
        let mut lowland_cells = 0_usize;
        let mut highland_cells = 0_usize;
        for z in (80..2_920).step_by(240) {
            for x in (80..2_920).step_by(240) {
                let lf = Landform::classify(20260829, x, z, geometry);
                match lf {
                    Landform::Valley | Landform::Plain => lowland_cells += 1,
                    Landform::Hill | Landform::Mountain => highland_cells += 1,
                }
            }
        }
        let mut forest_lowland = 0_usize;
        let mut forest_highland = 0_usize;
        for entity in &entities {
            if entity.kind != "mountain_forest" {
                continue;
            }
            let lf = Landform::classify(20260829, entity.world_x, entity.world_z, geometry);
            match lf {
                Landform::Valley | Landform::Plain => forest_lowland += 1,
                Landform::Hill | Landform::Mountain => forest_highland += 1,
            }
        }
        let lowland_density = forest_lowland as f64 / lowland_cells.max(1) as f64;
        let highland_density = forest_highland as f64 / highland_cells.max(1) as f64;
        assert!(
            highland_density >= lowland_density,
            "highland forest density {highland_density:.3} should be at least lowland {lowland_density:.3}"
        );
    }

    fn region_geometry(width: u32, depth: u32) -> WaterGeometry {
        WaterGeometry {
            kind: WaterKind::None,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: width,
            scene_depth_m: depth,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: natural_geography::GeographyControls::default(),
        }
    }

    #[test]
    fn profile_aware_region_bands_do_not_collapse_large_city_to_rural_only() {
        let water = legacy_water_geometry(WaterKind::None, 6_000, 10_000, 0.0);
        let mut urbanish = 0usize;
        let mut ruralish = 0usize;
        for z in (0..10_000).step_by(120) {
            for x in (0..6_000).step_by(120) {
                match RegionType::classify(20260905, x, z, water, 0.264) {
                    RegionType::UrbanCore | RegionType::Urban | RegionType::Suburban => {
                        urbanish += 1
                    }
                    RegionType::Rural | RegionType::Forest | RegionType::Mountain => ruralish += 1,
                }
            }
        }
        assert!(
            urbanish > 500,
            "large city profile produced too few urban cells: {urbanish}"
        );
        assert!(
            ruralish > 100,
            "large scene lost rural transition: {ruralish}"
        );
    }

    #[test]
    fn urbanisation_field_fades_from_centre_to_rim() {
        let geometry = region_geometry(6_000, 6_000);
        let centre = urbanization_field(20260829, 3_000, 3_000, geometry, 0.5);
        let near_mid = urbanization_field(20260829, 3_000, 4_200, geometry, 0.5);
        let edge = urbanization_field(20260829, 300, 300, geometry, 0.5);
        assert!(
            centre > near_mid && near_mid > edge,
            "urbanisation should fade from centre ({centre:.3}) through mid ({near_mid:.3}) to rim ({edge:.3})"
        );
        assert!(
            centre > 0.7,
            "scene centre should be strongly urbanised ({centre:.3})"
        );
        assert!(edge < 0.3, "scene corner should be near-wild ({edge:.3})");
    }

    #[test]
    fn region_types_differ_by_position_and_drive_landuse() {
        let geometry = region_geometry(4_000, 4_000);
        let centre_region = RegionType::classify(20260829, 2_000, 2_000, geometry, 0.5);
        let edge_region = RegionType::classify(20260829, 3800, 3800, geometry, 0.5);
        assert!(
            matches!(centre_region, RegionType::UrbanCore | RegionType::Urban),
            "scene centre should be urban, got {centre_region:?}"
        );
        assert!(
            matches!(
                edge_region,
                RegionType::Rural | RegionType::Forest | RegionType::Mountain
            ),
            "scene corner should be wild, got {edge_region:?}"
        );
        // The same region field drives generation, so an urban-core cell must
        // produce residential while a forest cell produces woodland.
        let urban_entities = profile_entities("temperate-plain", 20260829, 4_000, 4_000);
        let residential_count = urban_entities
            .iter()
            .filter(|e| e.kind == "residential_block")
            .count();
        let forest_count = urban_entities
            .iter()
            .filter(|e| e.kind == "mountain_forest")
            .count();
        assert!(residential_count > 0, "urban core should build homes");
        assert!(forest_count > 0, "wild rim should grow forest");
    }

    #[test]
    fn region_field_is_consistent_across_chunk_seams() {
        let geometry = region_geometry(6_000, 10_000);
        // The field is a pure world-space function: sampling left and right of
        // a chunk seam (x=512) must not jump between unrelated region types.
        let mut seen = BTreeSet::new();
        for z in [2_000, 5_000, 8_000] {
            let left = RegionType::classify(20260829, 511, z, geometry, 0.5);
            let right = RegionType::classify(20260829, 512, z, geometry, 0.5);
            seen.insert(left);
            seen.insert(right);
        }
        // A seam can be at a region boundary, but it should never flip between
        // urban and wild without passing through the intermediate band.
        let left = RegionType::classify(20260829, 511, 5_000, geometry, 0.5);
        let right = RegionType::classify(20260829, 512, 5_000, geometry, 0.5);
        let jump = (left, right);
        let is_gradual = left == right
            || matches!(
                jump,
                (RegionType::Urban, RegionType::UrbanCore)
                    | (RegionType::UrbanCore, RegionType::Urban)
                    | (RegionType::Suburban, RegionType::Urban)
                    | (RegionType::Urban, RegionType::Suburban)
                    | (RegionType::Rural, RegionType::Suburban)
                    | (RegionType::Suburban, RegionType::Rural)
            );
        assert!(
            is_gradual,
            "chunk seam at (511/512, 5000) jumped {left:?} -> {right:?}"
        );
        let _ = seen;
    }
    #[test]
    fn landuse_zones_follow_core_to_rim_spatial_prior() {
        let scene = SceneSpec {
            scene_id: "spatial".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let profile = LandUseProfile::default_demo();
        let zones = plan_landuse_zones(&scene, &profile);
        let center_x = scene.width_m as f64 * 0.5;
        let center_z = scene.depth_m as f64 * 0.5;
        let radius = |zone: &LandUseZone| {
            let x = zone.world_x as f64 + zone.width_m as f64 * 0.5;
            let z = zone.world_z as f64 + zone.depth_m as f64 * 0.5;
            ((x - center_x).powi(2) + (z - center_z).powi(2)).sqrt()
        };
        let urban = zones
            .iter()
            .filter(|zone| zone.kind == "urban")
            .map(radius)
            .sum::<f64>()
            / zones.iter().filter(|zone| zone.kind == "urban").count() as f64;
        let nature = zones
            .iter()
            .filter(|zone| zone.kind == "nature")
            .map(radius)
            .sum::<f64>()
            / zones.iter().filter(|zone| zone.kind == "nature").count() as f64;
        assert!(urban < nature, "urban zones should be closer to the core");
    }
    #[test]
    fn balanced_zone_areas_match_budget_without_edge_sliver_bias() {
        let m = WorldManifest::default_demo();
        let profile = LandUseProfile::default_demo();
        for (w, d) in [(512, 512), (1000, 713), (6000, 10000)] {
            let mut scene = m.scenes[0].clone();
            scene.width_m = w;
            scene.depth_m = d;
            let zones = plan_landuse_zones(&scene, &profile);
            let total = f64::from(w) * f64::from(d);
            assert_eq!(
                zones
                    .iter()
                    .map(|z| u64::from(z.width_m) * u64::from(z.depth_m))
                    .sum::<u64>(),
                u64::from(w) * u64::from(d)
            );
            let norm =
                (profile.urban_target() + profile.rural_target() + profile.nature_target()).max(1.);
            for (kind, target) in [
                ("urban", profile.urban_target() / norm),
                ("rural", profile.rural_target() / norm),
                ("nature", profile.nature_target() / norm),
            ] {
                let area = zones
                    .iter()
                    .filter(|z| z.kind == kind)
                    .map(|z| f64::from(z.width_m) * f64::from(z.depth_m))
                    .sum::<f64>();
                assert!(
                    (area / total - target).abs() < 0.05,
                    "{w}x{d} {kind}: {} vs {target}",
                    area / total
                );
            }
        }
        let mut scene = m.scenes[0].clone();
        scene.width_m = 512;
        scene.depth_m = 512;
        let profile = LandUseProfile {
            urban_core_ratio: 0.,
            suburban_ratio: 0.,
            farm_ratio: 0.,
            pasture_ratio: 0.,
            green_ratio: 0.,
            forest_ratio: 0.,
            reserve_ratio: 0.,
            ..profile
        };
        let zones = plan_landuse_zones(&scene, &profile);
        assert!(zones.iter().all(|z| z.kind == "unassigned"));
        let plan = plan_parcels(&zones);
        assert!(plan.parcels.is_empty());
        validate_parcel_plan(&plan).unwrap();
    }
    #[test]
    fn ordinary_green_space_does_not_raise_neighboring_terrain() {
        let green = generated_intent_entity(
            "generated.green".into(),
            "green_space",
            406,
            406,
            56,
            30.,
            30.,
            0.2,
        );
        assert!(
            plan_terrain_carves(&[green]).is_empty(),
            "landcover is not a building support pad"
        );
    }
    #[test]
    fn stable_v1_optional_generation_keeps_ids_when_prior_candidates_are_removed() {
        let scene = SceneSpec {scene_id:"stable-scene".into(),width_m:512,depth_m:512,origin_x:-256,origin_z:384,seed_offset:7};
        let style=serde_json::json!({"generationIntent":{"settlement":"wilderness"}});
        let water=water_geometry(WaterKind::None,&[],&scene,&style);
        let base=generate_entities_with_profile(7,&scene,&[],water,&style);
        let repeat=generate_entities_with_profile(7,&scene,&[],water,&style);
        assert_eq!(serde_json::to_vec(&base).unwrap(),serde_json::to_vec(&repeat).unwrap());
        assert!(!base.is_empty());
        assert!(base.iter().all(|e|e.entity_id.contains(".stable-v1.")));
        // Explicit IDs are determined by membership, even with a generated prefix.
        let far:LandmarkSpec=serde_json::from_value(serde_json::json!({
            "entityId":"generated.authored-keep-exact","name":"far","type":"rock","purpose":"test",
            "description":"unrelated authored input","sceneId":scene.scene_id,"worldX":10000,"worldZ":10000,
            "widthM":1,"depthM":1,"heightM":1,"assetId":"prop.rock"
        })).unwrap();
        let with_far=generate_entities_with_profile(7,&scene,&[far.clone()],water,&style);
        assert!(with_far.iter().any(|e|e.entity_id==far.entity_id));
        let generated:Vec<_>=with_far.iter().filter(|e|e.entity_id!=far.entity_id).collect();
        assert_eq!(serde_json::to_vec(&base).unwrap(),serde_json::to_vec(&generated).unwrap());
        let first=base.iter().find(|e|matches!(e.kind.as_str(),"tree"|"rock"|"bush"|"grass_clump")).unwrap();
        let mut blocker=far.clone();
        blocker.entity_id="authored.blocker".into();blocker.entity_type="building".into();
        blocker.world_x=first.world_x;blocker.world_z=first.world_z;
        blocker.width_m=24;blocker.depth_m=24;blocker.height_m=8;blocker.asset_id="prop.building".into();
        let filtered=generate_entities_with_profile(7,&scene,&[blocker.clone()],water,&style);
        assert!(!filtered.iter().any(|e|e.entity_id==first.entity_id));
        let old:BTreeMap<_,_>=base.iter().map(|e|(&e.entity_id,serde_json::to_value(e).unwrap())).collect();
        let mut retained=0;
        for e in filtered.iter().filter(|e|e.entity_id!=blocker.entity_id) {
            assert_eq!(old.get(&e.entity_id),Some(&serde_json::to_value(e).unwrap()),"survivors must not be renumbered or moved");
            retained+=1;
        }
        assert!(retained>10 && retained<base.len());
        let mut other_scene=scene.clone();other_scene.scene_id="stable-other-scene".into();
        let other=generate_entities_with_profile(7,&other_scene,&[],water,&style);
        assert_eq!(other.len(),base.len());
        for (a,b) in base.iter().zip(&other) {
            assert_ne!(a.entity_id,b.entity_id);
            let mut value=serde_json::to_value(b).unwrap();value["entityId"]=serde_json::json!(a.entity_id);
            assert_eq!(serde_json::to_value(a).unwrap(),value,"scene namespace must not alter geometry");
        }
    }

    #[test]
    fn stable_v1_candidate_keys_are_bound_before_movement_and_planned_ids_stay_explicit() {
        let scene=SceneSpec {scene_id:"slots".into(),width_m:512,depth_m:512,origin_x:0,origin_z:0,seed_offset:0};
        let geometry=legacy_water_geometry(WaterKind::None,scene.width_m,scene.depth_m,0.0);
        let a=CandidateIds::new(&scene.scene_id);
        let mut base=vec![];
        append_natural_content(7,&scene,geometry,&mut base,&a);
        a.finish(&base).unwrap();
        let b=CandidateIds::new(&scene.scene_id);
        let extra=b.claim("unrelated-request","rock",&[1000,1000]);
        let mut expanded=vec![generated_intent_entity(extra,"rock",1000,1000,0,1.,1.,1.)];
        append_natural_content(7,&scene,geometry,&mut expanded,&b);
        b.finish(&expanded).unwrap();
        assert_eq!(serde_json::to_vec(&base).unwrap(),serde_json::to_vec(&expanded[1..]).unwrap());
        let mut moved=base[0].clone();moved.world_x+=1;
        assert_eq!(moved.entity_id,base[0].entity_id);
        let denied=audit_entity_inventory(&base[..1],&[moved],&[],EntityInventoryMode::Preserving{old_scheme:ENTITY_ID_SCHEME.into(),new_scheme:ENTITY_ID_SCHEME.into()});
        assert_eq!(denied.status,"fail");assert!(denied.entries[0].changes.contains(&EntityChangeKind::Moved));
        let site:road_network::RoadsideSite=serde_json::from_value(serde_json::json!({
            "id":"generated.frontage.request.exact","roadId":"planned.road","worldX":64,"worldZ":64,
            "widthM":24,"depthM":24,"supportLevelM":-2,"supportLevelMm":-1354,
            "frontageRoadPoint":[64,40],"rotationYDeg":0,"naturalMinM":-2.,"naturalMaxM":-1.
        })).unwrap();
        let planned=site.entity();
        assert_eq!(planned.entity_id,site.id);assert_eq!(planned.grounding.frontage_road_point,Some([64,40]));
        let mut inventory=base.clone();inventory.push(planned.clone());
        a.finish(&inventory).unwrap();
        assert_eq!(serde_json::to_value(&inventory.last().unwrap()).unwrap(),serde_json::to_value(&planned).unwrap());
        let duplicate=vec![planned.clone(),planned];
        assert!(a.finish(&duplicate).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn stable_v1_legacy_transport_identity_coupling_is_explicit_not_blindly_migrated() {
        let e=generated_intent_entity("generated.landuse-road-ns.200000".into(),"road",0,0,0,80.,8.,0.2);
        assert_eq!(e.rotation_y_deg,90.);
        let renamed=generated_intent_entity("generated.opaque.stable-v1.hash".into(),"road",0,0,0,80.,8.,0.2);
        assert_eq!(renamed.rotation_y_deg,0.,"minimal witness: blind ID replacement changes road geometry");
        let scene=SceneSpec {scene_id:"legacy-transport-boundary".into(),width_m:512,depth_m:512,origin_x:0,origin_z:0,seed_offset:0};
        let ids=CandidateIds::new(&scene.scene_id);let mut entities=vec![];
        append_generic_land_content(7,&scene,&mut entities,&ids);
        assert_eq!(entities[0].entity_id,"generated.auto-road-east-west");
        assert_eq!(entities[1].entity_id,"generated.auto-road-north-south");
        assert!(entities[2..].iter().all(|e|e.entity_id.contains(".stable-v1.")));
        ids.finish(&entities).unwrap();
    }

    #[test]
    fn stable_v1_transport_ids_preserve_direction_and_survive_unrelated_input() {
        let scene=SceneSpec {scene_id:"stable-transport".into(),width_m:1000,depth_m:1000,origin_x:-500,origin_z:-500,seed_offset:0};
        let style=serde_json::json!({"generationIntent":{"settlement":"city"},"landUseProfile":{"theme":"temperate-plain","urbanCoreRatio":0.25}});
        let water=water_geometry(WaterKind::None,&[],&scene,&style);
        let base=generate_entities_with_profile(7,&scene,&[],water,&style);
        let transport:Vec<_>=base.iter().filter(|e|matches!(e.kind.as_str(),"road"|"sidewalk"|"canal"|"railway")).collect();
        assert!(!transport.is_empty());
        for entity in &transport {
            if entity.entity_id.starts_with("generated.landuse-") {
                assert!(entity.entity_id.contains(".stable-v1."),"transport serial ID remained: {}",entity.entity_id);
            }
        }
        let ns=transport.iter().find(|e|e.entity_id.contains("landuse-road-ns.stable-v1.")).expect("stable NS road");
        assert_eq!(ns.rotation_y_deg,90.0,"NS direction must remain encoded after stable-ID migration");
        let ew=transport.iter().find(|e|e.entity_id.contains("landuse-road-ew.stable-v1.")).expect("stable EW road");
        assert_eq!(ew.rotation_y_deg,0.0,"EW direction must remain encoded after stable-ID migration");
        let far:LandmarkSpec=serde_json::from_value(serde_json::json!({
            "entityId":"generated.transport-unrelated-authored","name":"far","type":"rock","purpose":"test",
            "description":"unrelated authored input","sceneId":scene.scene_id,"worldX":10000,"worldZ":10000,
            "widthM":1,"depthM":1,"heightM":1,"assetId":"prop.rock"
        })).unwrap();
        let with_far=generate_entities_with_profile(7,&scene,&[far.clone()],water,&style);
        let generated:Vec<_>=with_far.iter().filter(|e|e.entity_id!=far.entity_id).collect();
        assert_eq!(serde_json::to_vec(&base).unwrap(),serde_json::to_vec(&generated).unwrap());
    }

    #[test]
    fn stable_v1_small_modern_and_pastoral_planners_keep_determinism_and_unique_ids() {
        let scene=SceneSpec {scene_id:"small-modern".into(),width_m:512,depth_m:512,origin_x:0,origin_z:0,seed_offset:0};
        for style in [serde_json::json!({"generationIntent":{"settlement":"pastoral"}}),
            serde_json::json!({"generationIntent":{"settlement":"city"},"landUseProfile":{"theme":"temperate-plain","urbanCoreRatio":0.3}})] {
            let water=water_geometry(WaterKind::None,&[],&scene,&style);
            let a=generate_entities_with_profile(7,&scene,&[],water,&style);
            let b=generate_entities_with_profile(7,&scene,&[],water,&style);
            assert_eq!(serde_json::to_vec(&a).unwrap(),serde_json::to_vec(&b).unwrap());
            assert!(a.iter().any(|e|e.entity_id.contains(".stable-v1.")));
            let ids:std::collections::BTreeSet<_>=a.iter().map(|e|&e.entity_id).collect();assert_eq!(ids.len(),a.len());
        }
    }
    #[test]
    fn stable_v1_manifest_patch_keeps_explicit_operations_without_claiming_inventory_preservation() {
        let mut manifest=WorldManifest::default_demo();
        let landmark:LandmarkSpec=serde_json::from_value(serde_json::json!({
            "entityId":"generated.authored-explicit","name":"kept","type":"rock","purpose":"test",
            "description":"explicit membership","sceneId":manifest.scenes[0].scene_id,
            "worldX":300,"worldZ":300,"widthM":2,"depthM":2,"heightM":1,"assetId":"prop.rock"
        })).unwrap();
        manifest.landmarks=vec![landmark.clone()];
        let make_patch=|base:&WorldManifest,operations| WorldPatch {
            format:"glyphweave-world-patch".into(),version:2,patch_id:"inventory-scope-test".into(),
            base_revision:world_manifest_revision(base).unwrap(),parent_patch_hash:Some(base.patch_chain_hash.clone()),sequence:1,operations,
        };
        let moved=apply_patch(&manifest,&make_patch(&manifest,vec![PatchOperation::MoveLandmark{
            entity_id:landmark.entity_id.clone(),world_x:301,world_z:302,world_y:-2,
        }])).unwrap();
        assert_eq!(moved.landmarks[0].entity_id,landmark.entity_id);
        assert_eq!((moved.landmarks[0].world_x,moved.landmarks[0].world_z,moved.landmarks[0].world_y),(301,302,-2));
        let mut update=moved.landmarks[0].clone();update.height_m=3;
        let set=apply_patch(&moved,&make_patch(&moved,vec![PatchOperation::SetLandmark{landmark:update}])).unwrap();
        assert_eq!(set.landmarks[0].height_m,3);
        let removed=apply_patch(&set,&make_patch(&set,vec![PatchOperation::RemoveLandmark{entity_id:landmark.entity_id.clone()}])).unwrap();
        assert!(removed.landmarks.is_empty());
        let unknown=make_patch(&manifest,vec![PatchOperation::RemoveLandmark{entity_id:"not-authored".into()}]);
        assert!(apply_patch(&manifest,&unknown).is_err());
        // Patch success alone is not an approval to alter stored generated objects.
        let old=vec![landmark_to_entity(&landmark)];
        let new=vec![landmark_to_entity(&moved.landmarks[0])];
        assert_eq!(audit_entity_inventory(&old,&new,&[],EntityInventoryMode::Preserving{
            old_scheme:"legacy".into(),new_scheme:"legacy".into(),
        }).status,"fail");
    }

    /// B02-W1 closeout: the graded road geometry revision is bound only into
    /// the identity of inputs that can actually use the 16m band.
    #[test]
    fn road_geometry_revision_applies_only_to_inputs_that_can_use_the_16m_band() {
        let auto = |tiers: serde_json::Value, purposes: serde_json::Value| {
            serde_json::json!({
                "automaticRoadExtensions": {"version": 2, "scenes": [{
                    "sceneId": "scene-0", "maxNewSegments": 8, "maxCandidateAttempts": 256,
                    "segmentLengthsM": [128], "priorityPurposes": purposes,
                    "reachDistanceM": 96, "hierarchy": {"tiers": tiers}
                }]}
            })
        };
        let corridor = |width: u64| {
            serde_json::json!({
                "roadExtensions": {"version": 1, "corridors": [{
                    "sceneId": "scene-0", "start": [0, 0], "end": [64, 0],
                    "roadWidthM": width, "shoulderWidthM": 3, "maxFillMm": 6000,
                    "maxRiseMmPerM": 80, "startLevelMm": 0, "endLevelMm": 0,
                    "junctionLengthM": 8
                }]}
            })
        };
        // Legacy input: automatic roads, no hierarchy, no explicit corridors.
        assert!(road_geometry_revision_for_style(&auto(
            serde_json::Value::Null,
            serde_json::json!(["urban", "rural"])
        ))
        .is_none());
        // Default world: no graded program at all.
        assert!(road_geometry_revision_for_style(&WorldManifest::default_demo().style).is_none());
        // Explicit 16m corridor: bound. 8m/12m only: not bound.
        assert_eq!(
            road_geometry_revision_for_style(&corridor(16)),
            Some(GRADED_ROAD_GEOMETRY_REVISION)
        );
        assert!(road_geometry_revision_for_style(&corridor(12)).is_none());
        assert!(road_geometry_revision_for_style(&corridor(8)).is_none());
        // Automatic hierarchy with a 16m tier the scene's purposes can select.
        let tiers = serde_json::json!([
            {"id": "collector", "role": "collector", "widthM": 16, "purposes": ["urban"]},
            {"id": "local", "role": "local", "widthM": 8, "purposes": ["urban", "rural"]}
        ]);
        assert_eq!(
            road_geometry_revision_for_style(&auto(tiers.clone(), serde_json::json!(["urban"]))),
            Some(GRADED_ROAD_GEOMETRY_REVISION)
        );
        // Same tiers, but only rural is prioritised and the 16m tier is urban.
        assert!(road_geometry_revision_for_style(&auto(
            tiers,
            serde_json::json!(["rural"])
        ))
        .is_none());
        // A morphology profile always compiles in the 16m collector tier.
        assert_eq!(
            road_geometry_revision_for_style(&serde_json::json!({
                "morphologyProfile": "village-field"
            })),
            Some(GRADED_ROAD_GEOMETRY_REVISION)
        );
    }

    /// The same manifest gets a different identity under two road geometry
    /// revisions, while a legacy input keeps the historical identity.
    #[test]
    fn world_identity_binds_the_road_geometry_revision_deterministically() {
        let mut manifest = WorldManifest::default_demo();
        manifest.scenes[0].width_m = 512;
        manifest.scenes[0].depth_m = 512;
        // Legacy input: the derived revision is unchanged from the historical
        // payload (no geometry key at all).
        assert!(road_geometry_revision_for_style(&manifest.style).is_none());
        let legacy = world_manifest_revision(&manifest).unwrap();
        assert_eq!(
            legacy,
            world_manifest_revision_with_geometry(&manifest, None, None).unwrap()
        );
        // Adding an explicit 16m corridor changes the identity, and it matches
        // the payload built with the geometry tag explicitly.
        manifest.style["roadExtensions"] = serde_json::json!({
            "version": 1, "corridors": [{
                "sceneId": "scene-0", "start": [32, 96], "end": [96, 96],
                "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                "maxRiseMmPerM": 80, "startLevelMm": 8000, "endLevelMm": 8000,
                "junctionLengthM": 8
            }]
        });
        let bound = world_manifest_revision(&manifest).unwrap();
        assert_ne!(bound, legacy);
        assert_eq!(
            bound,
            world_manifest_revision_with_geometry(
                &manifest,
                None,
                Some(GRADED_ROAD_GEOMETRY_REVISION)
            )
            .unwrap()
        );
        // Deterministic.
        assert_eq!(bound, world_manifest_revision(&manifest).unwrap());
        // The geometry revision alone changes the identity: v1 vs v2.
        let v1 = world_manifest_revision_with_geometry(&manifest, None, Some("graded-road-band-v1"))
            .unwrap();
        let v2 = bound.clone();
        assert_ne!(v1, v2);
        // 12m corridors carry no geometry binding: their identity is exactly
        // the payload without a geometry key (the manifest content itself still
        // differs from a manifest with no extension program at all).
        let mut narrow = WorldManifest::default_demo();
        narrow.scenes[0].width_m = 512;
        narrow.scenes[0].depth_m = 512;
        narrow.style["roadExtensions"] = manifest.style["roadExtensions"].clone();
        narrow.style["roadExtensions"]["corridors"][0]["roadWidthM"] = serde_json::json!(12);
        assert!(road_geometry_revision_for_style(&narrow.style).is_none());
        assert_eq!(
            world_manifest_revision(&narrow).unwrap(),
            world_manifest_revision_with_geometry(&narrow, None, None).unwrap()
        );
        assert_ne!(world_manifest_revision(&narrow).unwrap(), legacy);
    }
}

/// B04-C1 real-function integration. Uses the independently verified R4/R6
/// feasible flat control (two graded road levels plus an explicit 16-home and
/// 8-shop demand) and drives the production planning functions with the
/// compiled effective programs, never with a second placement implementation.
#[cfg(test)]
mod morphology_profile_integration {
    use super::*;
    use crate::height_payload::encode_millimetres;
    use crate::settlement::morphology_compile::{self, SOURCE_PROFILE};
    use road_network::RoadNetworkPlan;

    const SCENE: &str = "profile-integration-control";
    const HOMES: u32 = 16;
    const SHOPS: u32 = 8;

    fn fixture() -> (SceneSpec, BakedTerrain, WaterGeometry, Vec<LandUseZone>) {
        let scene = SceneSpec {
            scene_id: SCENE.into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, vec![0.0; 512 * 512]).unwrap(),
            )
            .unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let zones = vec![LandUseZone {
            id: "control.urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "integration_fixture:flat-authored".into(),
        }];
        (scene, terrain, water, zones)
    }

    fn base_plan(natural_hash: &str) -> RoadNetworkPlan {
        serde_json::from_value(serde_json::json!({
            "model": "fixture",
            "naturalHeightfieldHash": natural_hash,
            "sceneId": SCENE,
            "status": "planned_connected_network",
            "gridSpacingM": 64,
            "maxRoadbedFillM": 6.0,
            "levelM": 0,
            "candidates": 1,
            "rejectedWater": 0,
            "rejectedRelief": 0,
            "unselectedCandidates": 0,
            "nodes": [
                {"id": 0, "worldX": 32, "worldZ": 160},
                {"id": 1, "worldX": 96, "worldZ": 160}
            ],
            "edges": [{
                "id": "fixture.base", "from": 0, "to": 1,
                "worldX": 64, "worldZ": 160, "lengthM": 64,
                "rotationYDeg": 0.0, "naturalMinM": 0.0, "naturalMaxM": 0.0
            }],
            "roadsideSites": []
        }))
        .unwrap()
    }

    /// Demand is authored; building geometry is authored; parcel insets are
    /// left to the profile. This is the shape the CLI control uses.
    fn control_style(profile: Option<&str>) -> serde_json::Value {
        let mut style = serde_json::json!({
            "landUseProfile": {
                "theme": "temperate-plain",
                "urbanCoreRatio": 0.10,
                "suburbanRatio": 0.10,
                "greenRatio": 0.06,
                "farmRatio": 0.32,
                "forestRatio": 0.14,
                "pastureRatio": 0.26,
                "reserveRatio": 0.04
            },
            "roadExtensions": {
                "version": 1,
                "corridors": [
                    {"sceneId": SCENE, "start": [96, 160], "end": [126, 160],
                     "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                     "maxRiseMmPerM": 80, "startLevelMm": 0, "endLevelMm": 0,
                     "junctionLengthM": 8},
                    {"sceneId": SCENE, "start": [126, 160], "end": [480, 160],
                     "roadWidthM": 12, "shoulderWidthM": 3, "maxFillMm": 6000,
                     "maxRiseMmPerM": 80, "startLevelMm": 0, "endLevelMm": 0,
                     "junctionLengthM": 8}
                ]
            },
            "frontageProgram": {
                "version": 1,
                "requests": [
                    {"id": "control-homes", "sceneId": SCENE, "purpose": "urban",
                     "kind": "residential_home", "count": HOMES,
                     "widthM": 12, "depthM": 10, "heightM": 8,
                     "stationSpacingM": 20, "setbackM": 32, "maxCandidateAttempts": 2048},
                    {"id": "control-shops", "sceneId": SCENE, "purpose": "urban",
                     "kind": "storefront", "count": SHOPS,
                     "widthM": 12, "depthM": 10, "heightM": 6,
                     "stationSpacingM": 20, "setbackM": 32, "maxCandidateAttempts": 2048}
                ]
            }
        });
        if let Some(profile) = profile {
            style["morphologyProfile"] = serde_json::json!(profile);
        }
        style
    }

    fn run(profile: Option<&str>) -> (serde_json::Value, RoadNetworkPlan) {
        let (scene, terrain, water, zones) = fixture();
        let raw = control_style(profile);
        let effective = match profile {
            Some(_) => morphology_compile::compile(&raw, std::slice::from_ref(&scene))
                .unwrap()
                .expect("profile compiles")
                .style,
            None => raw.clone(),
        };
        let mut plan = base_plan(&terrain.content_hash().unwrap());
        road_extensions::append(&mut plan, &scene, &terrain, water, &zones, &effective).unwrap();
        frontage::append(&mut plan, &scene, &terrain, water, Some(&zones), &effective).unwrap();
        (effective, plan)
    }

    fn parcel_size(site: &road_network::RoadsideSite) -> (i64, i64) {
        let boundary =
            parcel_geometry::boundary_for_entity(&site.entity(), site.parcel_insets.unwrap_or_default())
                .unwrap();
        (boundary[2] - boundary[0], boundary[3] - boundary[1])
    }

    fn planned(plan: &RoadNetworkPlan) -> u32 {
        let report = plan.frontage.as_ref().expect("frontage report");
        report
            .outcomes
            .iter()
            .map(|outcome| outcome.site_ids.len() as u32)
            .sum()
    }

    #[test]
    fn two_profiles_change_real_parcel_geometry_on_the_feasible_control() {
        let (legacy_style, legacy_plan) = run(None);
        assert!(legacy_style.get("morphologyProfile").is_none());
        let (compact_style, compact_plan) = run(Some("compact-mixed-block"));
        let (cluster_style, cluster_plan) = run(Some("clustered-residential"));

        // The same authored demand is planned by production code for all three
        // inputs; only the compiled effective programs differ.
        assert_eq!(legacy_plan.roadside_sites.len(), (HOMES + SHOPS) as usize);
        assert_eq!(compact_plan.roadside_sites.len(), (HOMES + SHOPS) as usize);
        assert_eq!(cluster_plan.roadside_sites.len(), (HOMES + SHOPS) as usize);
        assert_eq!(planned(&legacy_plan), HOMES + SHOPS);
        assert_eq!(planned(&compact_plan), HOMES + SHOPS);
        assert_eq!(planned(&cluster_plan), HOMES + SHOPS);
        for plan in [&legacy_plan, &compact_plan, &cluster_plan] {
            let report = plan.frontage.as_ref().unwrap();
            assert!(
                report.outcomes.iter().all(|outcome| outcome.unfilled == 0
                    && outcome.stop_reason == "demand_satisfied"),
                "unsatisfied demand must be reported, never resized: {report:?}"
            );
        }

        let first = |plan: &RoadNetworkPlan| parcel_size(&plan.roadside_sites[0]);
        let legacy_size = first(&legacy_plan);
        let compact_size = first(&compact_plan);
        let cluster_size = first(&cluster_plan);
        assert_eq!(legacy_size, (16, 14), "legacy insets stay 2/2/2 metres");
        assert_eq!(compact_size, (16, 18), "compact-mixed-block insets 4/2/4");
        assert_eq!(cluster_size, (20, 32), "clustered-residential insets 6/4/16");
        assert_ne!(legacy_size, compact_size);
        assert_ne!(compact_size, cluster_size);

        // Profiles also change the stored parcel policy, not only the numbers
        // this test computes.
        assert!(compact_plan
            .roadside_sites
            .iter()
            .all(|site| site.parcel_insets.is_some()));
        assert!(legacy_plan
            .roadside_sites
            .iter()
            .all(|site| site.parcel_insets.is_none()));
        assert_ne!(
            compact_plan.roadside_sites[0].parcel_insets,
            cluster_plan.roadside_sites[0].parcel_insets
        );

        // The difference is visible in the published plans themselves.
        assert_ne!(
            serde_json::to_value(&compact_plan).unwrap(),
            serde_json::to_value(&cluster_plan).unwrap()
        );
        // Source binding: the insets the plans carry came from the profile, and
        // the two profiles really compiled to different effective styles.
        assert_ne!(compact_style, cluster_style);
        for (style, expected) in [
            (&compact_style, "compact-mixed-block"),
            (&cluster_style, "clustered-residential"),
        ] {
            assert_eq!(style["morphologyProfile"], serde_json::json!(expected));
            assert_eq!(
                style["frontageProgram"]["requests"][0]["count"],
                serde_json::json!(HOMES)
            );
        }
        assert!(morphology_compile::compile(
            &control_style(Some("compact-mixed-block")),
            std::slice::from_ref(&fixture().0)
        )
        .unwrap()
        .unwrap()
        .report
        .field_sources
        .iter()
        .any(|entry| entry.source == SOURCE_PROFILE && entry.profile_field == "sideInsetM"));
    }

    #[test]
    fn the_feasible_control_is_deterministic_and_the_profile_binds_the_road_plan() {
        let (_, first) = run(Some("village-field"));
        let (_, second) = run(Some("village-field"));
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        // A different profile must not silently reuse the same document.
        let (_, other) = run(Some("perimeter-block"));
        assert_ne!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&other).unwrap()
        );
    }

    /// The derived street setback is not only a published number: it moves the
    /// real frontage candidate and puts the parcel's front boundary exactly on
    /// the road's protected line. Same control, same road, same station, same
    /// side; only the compiled setback differs, so the difference is the wiring.
    #[test]
    fn the_derived_street_setback_moves_the_real_frontage_candidate() {
        let (scene, terrain, water, zones) = fixture();
        let raw = control_style(Some("compact-mixed-block"));
        let mut derived_style = raw.clone();
        let mut authored_style = raw.clone();
        for request in derived_style["frontageProgram"]["requests"]
            .as_array_mut()
            .unwrap()
        {
            request.as_object_mut().unwrap().remove("setbackM");
        }
        for request in authored_style["frontageProgram"]["requests"]
            .as_array_mut()
            .unwrap()
        {
            request["setbackM"] = serde_json::json!(12);
        }
        let compiled_derived =
            morphology_compile::compile(&derived_style, std::slice::from_ref(&scene))
                .unwrap()
                .unwrap();
        let compiled_authored =
            morphology_compile::compile(&authored_style, std::slice::from_ref(&scene))
                .unwrap()
                .unwrap();
        // compact-mixed-block: front inset 4m + 4m road band = 8m setback.
        assert_eq!(
            compiled_derived.style["frontageProgram"]["requests"][0]["setbackM"],
            serde_json::json!(8)
        );
        assert_eq!(
            compiled_authored.style["frontageProgram"]["requests"][0]["setbackM"],
            serde_json::json!(12)
        );

        // A real planned road from the production network.
        let mut plan = base_plan(&terrain.content_hash().unwrap());
        road_extensions::append(
            &mut plan,
            &scene,
            &terrain,
            water,
            &zones,
            &compiled_derived.style,
        )
        .unwrap();
        let road = plan
            .entities()
            .into_iter()
            .find(|entity| {
                entity.kind == "road"
                    && entity.rotation_y_deg == 0.
                    && entity.grounding.road_profile.is_some()
            })
            .expect("a real graded road entity");
        let request = |style: &serde_json::Value| {
            frontage::from_style(style).unwrap().unwrap().requests[0].clone()
        };
        let derived_request = request(&compiled_derived.style);
        let authored_request = request(&compiled_authored.style);
        let station = 18_usize;
        let derived_site =
            frontage::candidate_site(&road, &derived_request, SCENE, station, 1).unwrap();
        let authored_site =
            frontage::candidate_site(&road, &authored_request, SCENE, station, 1).unwrap();
        assert_eq!(derived_site.rotation_y_deg, 0.);
        assert_eq!(derived_site.world_x, authored_site.world_x);
        assert_eq!(
            authored_site.world_z - derived_site.world_z,
            4,
            "the 12m authored setback is 4m further from the road than the derived 8m one"
        );
        // The parcel's near boundary is exactly the road's protected edge: the
        // corridor half width plus `ROAD_PROTECTION_M`, and not one metre less.
        // The control's first graded road is the 16 m collector corridor, so the
        // concrete numbers are 8 + 4 = 12 m (derived) and 8 + 4 + 4 = 16 m
        // (authored 12 m setback).
        assert_eq!(road.depth_m, 16., "control collector width");
        let road_half = (road.depth_m / 2.) as i64;
        let band = frontage::ROAD_PROTECTION_M;
        assert_eq!(road_half, 8);
        assert_eq!(band, 4);
        let derived_parcel = frontage::parcel_boundary(&derived_site).unwrap();
        let authored_parcel = frontage::parcel_boundary(&authored_site).unwrap();
        let road_center_z = i64::from(road.world_z);
        // Building centre from the road centreline: half collector 8 + setback
        // 8 or 12 + half building depth 5.
        assert_eq!(derived_site.world_z - road.world_z, 21);
        assert_eq!(authored_site.world_z - road.world_z, 25);
        assert_eq!(derived_parcel[1] - road_center_z, 12);
        assert_eq!(authored_parcel[1] - road_center_z, 16);
        assert_eq!(derived_parcel[1] - road_center_z, road_half + band);
        assert_eq!(authored_parcel[1] - road_center_z, road_half + band + 4);
        assert_eq!(
            derived_parcel[3] - derived_parcel[1],
            i64::from(derived_request.depth_m)
                + i64::from(derived_request.parcel_insets.unwrap().front_m)
                + i64::from(derived_request.parcel_insets.unwrap().rear_m)
        );
        // The production placement loop accepts the derived default: every
        // authored home and shop is really placed, so the default is usable and
        // not merely arithmetically valid.
        let mut placed = base_plan(&terrain.content_hash().unwrap());
        road_extensions::append(
            &mut placed,
            &scene,
            &terrain,
            water,
            &zones,
            &compiled_derived.style,
        )
        .unwrap();
        frontage::append(
            &mut placed,
            &scene,
            &terrain,
            water,
            Some(&zones),
            &compiled_derived.style,
        )
        .unwrap();
        assert_eq!(planned(&placed), HOMES + SHOPS);
    }

    /// Planning-stage pre-flight of the two CLI controls. It runs the same
    /// production planning functions the bake uses (base network, automatic
    /// road extension, local frontage) on the authored 512m control scene and
    /// shows what the profile actually changes before a world is written.
    #[test]
    fn cli_control_planning_stage_shows_profile_driven_road_hierarchy() {
        let scene = SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let seed = 42_u64;
        let values: Vec<f64> = (0..512)
            .flat_map(|z| (0..512).map(move |x| terrain_height_metres(seed, x, z, water, &[])))
            .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 512, 512, &encode_millimetres(512, 512, values).unwrap())
            .unwrap();
        let mut reports = Vec::new();
        for (raw, profile_id) in [
            (
                include_str!(
                    "../../../../examples/morphology/b04-c1-compact-mixed-block.manifest.json"
                ),
                "compact-mixed-block",
            ),
            (
                include_str!("../../../../examples/morphology/b04-c1-village-field.manifest.json"),
                "village-field",
            ),
        ] {
            let manifest: serde_json::Value = serde_json::from_str(raw).unwrap();
            let style = crate::settlement::morphology_compile::compile(
                &manifest["style"],
                std::slice::from_ref(&scene),
            )
            .unwrap()
            .unwrap()
            .style;
            let profile = LandUseProfile::from_style(&style).unwrap();
            let zones = plan_landuse_zones(&scene, &profile);
            let mut plan =
                road_network::plan_with_zones(&scene, &terrain, water, Some(&zones)).unwrap();
            let base_edges = plan.edges.len();
            auto_roads::append(&mut plan, &scene, &terrain, water, &zones, &style).unwrap();
            frontage::append(&mut plan, &scene, &terrain, water, Some(&zones), &style).unwrap();
            let hierarchy = plan
                .automatic_extensions
                .as_ref()
                .map(|report| serde_json::to_value(&report.program.hierarchy).unwrap());
            let widths: Vec<u32> = plan
                .entities()
                .iter()
                .filter(|entity| entity.kind == "road")
                .map(|entity| entity.depth_m as u32)
                .collect();
            let outcomes: Vec<(String, usize, u32)> = plan
                .frontage
                .as_ref()
                .unwrap()
                .outcomes
                .iter()
                .map(|outcome| {
                    (
                        outcome.request.id.clone(),
                        outcome.site_ids.len(),
                        outcome.unfilled,
                    )
                })
                .collect();
            let development = development::plan(&scene, &zones, &plan).unwrap();
            let landcover =
                development::plan_landcover_with_program(&scene, &development, &style).unwrap();
            reports.push((
                profile_id,
                hierarchy,
                widths,
                base_edges,
                plan.edges.len(),
                outcomes,
                serde_json::to_value(&landcover).unwrap(),
                landcover.cells.len(),
            ));
        }
        // Both controls share the same authored demand and the same authored
        // terrain; the profile is the only difference. The injected hierarchy
        // must actually reach the automatic road program.
        let (compact_id, compact_hierarchy, ..) = &reports[0];
        let (village_id, village_hierarchy, ..) = &reports[1];
        assert_eq!(*compact_id, "compact-mixed-block");
        assert_eq!(*village_id, "village-field");
        let compact_tiers: Vec<&str> = compact_hierarchy.as_ref().unwrap()["tiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tier| tier["role"].as_str().unwrap())
            .collect();
        let village_tiers: Vec<&str> = village_hierarchy.as_ref().unwrap()["tiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tier| tier["role"].as_str().unwrap())
            .collect();
        assert_eq!(compact_tiers, vec!["collector", "local"]);
        assert_eq!(village_tiers, vec!["collector", "local", "farm_access"]);
        assert_ne!(
            reports[0].6, reports[1].6,
            "the two profiles must compile different landcover execution programs"
        );
        for (profile_id, _, widths, base_edges, edges, outcomes, _, patches) in &reports {
            eprintln!(
                "PREFLIGHT {profile_id} baseEdges={base_edges} edges={edges} widths={widths:?} patches={patches} frontage={outcomes:?}"
            );
        }
    }

    #[test]
    fn structure_deck_keeps_plan_level_when_centre_station_is_on_dry_bank() {
        let scene = SceneSpec {
            scene_id: "deck-rebase".into(),
            width_m: 32,
            depth_m: 32,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        let payload =
            crate::height_payload::encode_millimetres(32, 32, vec![6.; 32 * 32]).unwrap();
        terrain.insert_chunk(0, 0, 32, 32, &payload).unwrap();
        let mut deck: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId": "generated.road.deck",
            "assetId": "prop.road",
            "kind": "road",
            "worldX": 16,
            "worldZ": 16,
            "worldY": 8,
            "scale": 1.0,
            "widthM": 64,
            "depthM": 8,
            "heightM": 0.24,
            "rotationYDeg": 0.0,
            "grounding": {
                "structureDeck": true,
                "roadbedOffsetM": 0.0
            }
        }))
        .unwrap();
        rebase_generated_transport_to_baked_surface(
            std::slice::from_mut(&mut deck),
            &[],
            &terrain,
            None,
        );
        assert_eq!(
            deck.world_y, 8,
            "a bridge deck must keep the level its water-span audit measured"
        );
    }
}
