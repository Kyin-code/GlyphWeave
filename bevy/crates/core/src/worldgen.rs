//! Deterministic large-world planning and sidecar baking for engine adapters.
//!
//! Public world coordinates use named `world_x`, `world_z`, and `world_y`
//! fields. The `.gemap` v3 codec keeps its frozen `(z, x, y)` protocol order.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::storage::codec::encode_world_with_metadata;
use crate::voxel::{VoxelCoord, VoxelWorld};
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub const WORLD_FORMAT: &str = "glyphweave-world";
pub const WORLD_VERSION: u32 = 1;
pub const STREAM_CHUNK_METERS: u32 = 512;
pub const MIN_SCENE_METERS: u32 = 512;
pub const MAX_SCENE_WIDTH_METERS: u32 = 6_000;
pub const MAX_SCENE_DEPTH_METERS: u32 = 10_000;
const SHORE_RADIUS: f64 = 1.9;

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
    /// Smooth-rolling terrain (steppe / prairie): soft rolling hills instead
    /// of sharp ridges, matching wgen's "Hills" hemispheric generator.
    smooth_rolling: bool,
}

/// A terrain carve: a flat rectangular pad under a road or building footprint.
/// Roads and buildings are generated ON TOP of a flattened pad so a building
/// on a slope reads as standing on level ground instead of draping over the
/// hillside (the approach used by symbios-ground-lab's terrain carving).
/// `blend_m` is the width of the smooth transition ring around the pad.
#[derive(Debug, Clone, Copy)]
struct TerrainCarve {
    cx: f64,
    cz: f64,
    half_w: f64,
    half_d: f64,
    target_h_m: f64,
    blend_m: f64,
    /// Roads are carved before buildings so a pad under a street is not
    /// re-flattened by a neighbouring lot; higher = wins.
    priority: i32,
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
    #[serde(default)]
    pub pivot: GroundingPivot,
    #[serde(default)]
    pub bottom_offset_m: f32,
    #[serde(default)]
    pub roadbed_offset_m: f32,
    #[serde(default = "default_grounding_tolerance_m")]
    pub tolerance_m: f32,
}

fn default_grounding_tolerance_m() -> f32 {
    1.0
}

impl Default for GroundingSpec {
    fn default() -> Self {
        Self {
            pivot: GroundingPivot::Bottom,
            bottom_offset_m: 0.0,
            roadbed_offset_m: 0.0,
            tolerance_m: default_grounding_tolerance_m(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarContract {
    pub format: String,
    pub version: u32,
    pub authoritative_for_terrain: bool,
    pub gemap_role: String,
    pub scene_index_root: String,
    pub height_precision_m: f32,
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
    pub surface_file: String,
    pub lod2_file: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneIndex {
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
    pub scenes: Vec<String>,
    #[serde(default)]
    pub scene_graph: SceneGraph,
    #[serde(default)]
    pub sidecar: SidecarContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPatch {
    pub format: String,
    pub version: u32,
    pub patch_id: String,
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
    #[error("landmark {landmark:?} is outside scene {scene:?}")]
    LandmarkOutsideScene { landmark: String, scene: String },
    #[error("output directory already contains files: {0}")]
    OutputNotEmpty(String),
    #[error("rules mode could not load descriptors: {0}")]
    InvalidRules(String),
    #[error(
        "rules-mode baked audit failed for scene {scene_id}: {rejected} of {checked} checked entities rejected"
    )]
    RulesAuditFailed {
        scene_id: String,
        checked: usize,
        rejected: usize,
    },
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

/// Achieved land-use area breakdown over a baked scene. Areas are computed as
/// `width_m * depth_m` per entity and grouped into urban / rural / nature so
/// the audit compares against `LandUseProfile` targets instead of entity counts.
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
            | "road_sign" => urban_m2 += area,
            "farmland" | "pasture" | "canal" | "building_cluster" => rural_m2 += area,
            "green_space" | "mountain_forest" | "nature_reserve" | "tree" | "bush" | "rock"
            | "grass_clump" | "reed" | "fallen_log" => nature_m2 += area,
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

fn default_asset_contracts() -> serde_json::Value {
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
    let (center_x, center_z, half_width, half_depth) = landmark.map_or(
        (
            fallback_x,
            fallback_z,
            f64::from(scene.width_m) * 0.14,
            f64::from(scene.depth_m) * 0.5,
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
    WaterGeometry {
        kind,
        center_x,
        center_z,
        half_width,
        half_depth,
        scene_width_m: scene.width_m,
        scene_depth_m: scene.depth_m,
        smooth_rolling,
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
            smooth_rolling: false,
        },
        WaterKind::River => WaterGeometry {
            kind,
            center_x: f64::from(scene_width) * 0.5,
            center_z: f64::from(scene_depth) * 0.5,
            half_width: river_half_m,
            half_depth: f64::from(scene_depth) * 0.5,
            scene_width_m: scene_width,
            scene_depth_m: scene_depth,
            smooth_rolling: false,
        },
        WaterKind::None => WaterGeometry {
            kind,
            center_x: 0.0,
            center_z: 0.0,
            half_width: 0.0,
            half_depth: 0.0,
            scene_width_m: scene_width,
            scene_depth_m: scene_depth,
            smooth_rolling: false,
        },
    }
}

impl GroundingSpec {
    pub fn validate(&self, owner: &str) -> WorldgenResult<()> {
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
        Bounds2d::from_center(
            self.world_x as f32,
            self.world_z as f32,
            self.width_m,
            self.depth_m,
        )
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
            let (dx, dz) = match (self.rotation_y_deg.round() as i32).rem_euclid(360) {
                90 => (self.width_m * 0.5 + 1.0, 0.0),
                180 => (0.0, self.depth_m * 0.5 + 1.0),
                270 => (-self.width_m * 0.5 - 1.0, 0.0),
                _ => (0.0, -self.depth_m * 0.5 - 1.0),
            };
            self.anchors.push(SpatialAnchor {
                id: "front".into(),
                world_x: (self.world_x as f32 + dx).round() as i32,
                world_z: (self.world_z as f32 + dz).round() as i32,
                direction: match (self.rotation_y_deg.round() as i32).rem_euclid(360) {
                    90 => "east",
                    180 => "south",
                    270 => "west",
                    _ => "north",
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
        Bounds2d::from_center(
            self.world_x as f32,
            self.world_z as f32,
            self.width_m as f32,
            self.depth_m as f32,
        )
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
        }
    }

    pub fn validate(&self) -> WorldgenResult<()> {
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
        let mut scene_ids = BTreeMap::new();
        for scene in &self.scenes {
            if scene.scene_id.trim().is_empty() || scene_ids.insert(&scene.scene_id, true).is_some()
            {
                return Err(WorldgenError::InvalidSceneId(scene.scene_id.clone()));
            }
            if !(MIN_SCENE_METERS..=MAX_SCENE_WIDTH_METERS).contains(&scene.width_m)
                || !(MIN_SCENE_METERS..=MAX_SCENE_DEPTH_METERS).contains(&scene.depth_m)
            {
                return Err(WorldgenError::InvalidSceneSize {
                    scene_id: scene.scene_id.clone(),
                    width_m: scene.width_m,
                    depth_m: scene.depth_m,
                });
            }
        }
        self.scene_graph.validate(&self.scenes)?;
        for landmark in &self.landmarks {
            if landmark.entity_id.trim().is_empty()
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
            let local_x = landmark.world_x - scene.origin_x;
            let local_z = landmark.world_z - scene.origin_z;
            let half_width = landmark.width_m.div_ceil(2) as i32;
            let half_depth = landmark.depth_m.div_ceil(2) as i32;
            let may_be_clipped_by_scene = matches!(landmark.entity_type.as_str(), "lake" | "river");
            if !may_be_clipped_by_scene
                && (local_x - half_width < 0
                    || local_z - half_depth < 0
                    || local_x + half_width >= scene.width_m as i32
                    || local_z + half_depth >= scene.depth_m as i32)
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

pub fn bake_world(manifest: &WorldManifest, output: &Path) -> WorldgenResult<WorldIndex> {
    manifest.validate()?;
    let rules_mode = manifest
        .style
        .get("rulesMode")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|mode| mode == "rules");
    let rules_dir = rules_mode.then(|| rules_dir_from_style(&manifest.style));
    if let Some(dir) = &rules_dir {
        crate::rules::ObjectRegistry::load_dir(dir)
            .map_err(|error| WorldgenError::InvalidRules(error.to_string()))?;
    }
    if output.exists() && fs::read_dir(output)?.next().is_some() {
        return Err(WorldgenError::OutputNotEmpty(output.display().to_string()));
    }
    fs::create_dir_all(output)?;
    let revision = blake3::hash(&serde_json::to_vec(manifest)?)
        .to_hex()
        .to_string();
    let mut scene_paths = Vec::new();
    let mut baked_rules_report = crate::rules::ValidationReport::default();
    for scene in &manifest.scenes {
        let scene_dir = output.join("scenes").join(&scene.scene_id);
        fs::create_dir_all(&scene_dir)?;
        let chunk_count_x = scene.width_m.div_ceil(STREAM_CHUNK_METERS);
        let chunk_count_z = scene.depth_m.div_ceil(STREAM_CHUNK_METERS);
        // Generate entities BEFORE baking the heightfields so we can carve
        // their footprints into the terrain. The world_y of every entity is
        // read from the natural (uncarved) terrain, then each road / building
        // pad is flattened into the heightfield so the baked ground carries a
        // level lot under every structure (symbios-ground-lab's approach).
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
        let entities = generate_entities_with_profile(
            manifest.world.seed ^ scene.seed_offset,
            scene,
            &landmarks,
            water,
            &manifest.style,
        );
        let mut entities = entities;
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let carves = plan_terrain_carves(&entities);
        let mut chunks = Vec::new();
        let mut baked_samples = BTreeMap::<i64, f32>::new();
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
                let surface_file = format!("{stem}.surface.bin");
                let lod2_file = format!("{stem}.lod2.bin");
                let (height, surface, lod2) = generate_chunk_with_geometry(
                    manifest.world.seed ^ scene.seed_offset,
                    base_x,
                    base_z,
                    valid_width_m,
                    valid_depth_m,
                    water,
                    &carves,
                );
                fs::write(scene_dir.join(&height_file), &height)?;
                fs::write(scene_dir.join(&surface_file), &surface)?;
                for (i, pair) in height.chunks_exact(2).enumerate() {
                    let raw = i16::from_le_bytes([pair[0], pair[1]]);
                    let lx = i as i32 % valid_width_m as i32;
                    let lz = i as i32 / valid_width_m as i32;
                    let wx = base_x + lx;
                    let wz = base_z + lz;
                    let key = (i64::from(wx) << 32) | (wz as u32 as i64);
                    baked_samples.insert(key, f32::from(raw) / 4.0);
                }
                fs::write(scene_dir.join(&lod2_file), &lod2)?;
                let hash = blake3::hash(
                    &[height.as_slice(), surface.as_slice(), lod2.as_slice()].concat(),
                )
                .to_hex()
                .to_string();
                let descriptor = ChunkDescriptor {
                    chunk_x,
                    chunk_z,
                    world_x: base_x,
                    world_z: base_z,
                    valid_width_m,
                    valid_depth_m,
                    height_file,
                    surface_file,
                    lod2_file,
                    hash,
                };
                fs::write(
                    scene_dir.join(format!("{stem}.json")),
                    serde_json::to_vec_pretty(&descriptor)?,
                )?;
                chunks.push(descriptor);
            }
        }
        if let Some(rules_dir) = &rules_dir {
            // The generator validates candidates against natural terrain before
            // carving. Re-run the same rules against the actual baked height
            // samples so terrain-carve and placement semantics cannot drift.
            for entity in &entities {
                let key = (i64::from(entity.world_x) << 32) | (entity.world_z as u32 as i64);
                if !baked_samples.contains_key(&key) {
                    return Err(WorldgenError::InvalidRules(format!(
                        "baked heightfield has no sample for entity {} at ({},{})",
                        entity.entity_id, entity.world_x, entity.world_z
                    )));
                }
            }
            let height_query = |x: i32, z: i32| {
                baked_samples
                    .get(&((i64::from(x) << 32) | (z as u32 as i64)))
                    .copied()
                    .expect("validated baked entity/footprint sample")
            };
            let audit = audit_scene(
                manifest.world.seed ^ scene.seed_offset,
                scene,
                &landmarks,
                &entities,
                water,
                rules_dir,
                AuditOptions {
                    height_at: Some(&height_query),
                    slope_half: None,
                },
            )
            .map_err(|error| WorldgenError::InvalidRules(error.to_string()))?;
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

        let index = SceneIndex {
            scene_id: scene.scene_id.clone(),
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            chunk_size_m: STREAM_CHUNK_METERS,
            chunk_count_x,
            chunk_count_z,
            chunks,
            landmarks,
            entities,
        };
        fs::write(
            scene_dir.join("scene.json"),
            serde_json::to_vec_pretty(&index)?,
        )?;
        scene_paths.push(format!("scenes/{}/scene.json", scene.scene_id));
    }
    if rules_mode {
        fs::write(
            output.join("rules-audit.json"),
            serde_json::to_vec_pretty(&baked_rules_report)?,
        )?;
    }
    write_gemap_anchor(output, manifest, &revision)?;
    let index = WorldIndex {
        format: WORLD_FORMAT.to_owned(),
        version: WORLD_VERSION,
        name: manifest.world.name.clone(),
        seed: manifest.world.seed,
        render_mode: manifest.world.render_mode.clone(),
        revision,
        scenes: scene_paths,
        scene_graph: manifest.scene_graph.clone(),
        sidecar: SidecarContract::default(),
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

pub fn apply_patch(manifest: &WorldManifest, patch: &WorldPatch) -> WorldgenResult<WorldManifest> {
    manifest.validate()?;
    if patch.format != "glyphweave-world-patch" || patch.version != 1 {
        return Err(WorldgenError::InvalidFormat(patch.format.clone()));
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
        }
    }
    result.validate()?;
    Ok(result)
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
        include_bytes!(
            "../../../../assets/third_party/quaternius/stylized-nature/License_Standard.txt"
        ),
    )?;
    fs::write(
        assets.join("glyphweave.registry.json"),
        include_bytes!("../../../../assets/glyphweave.registry.json"),
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
    generate_chunk_with_geometry(seed, base_x, base_z, width, depth, geometry, &[])
}

fn generate_chunk_with_geometry(
    seed: u64,
    base_x: i32,
    base_z: i32,
    width: u32,
    depth: u32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let cell_count = (width * depth) as usize;
    let mut height = Vec::with_capacity(cell_count * 2);
    let mut surface = Vec::with_capacity(cell_count);
    let mut lod2 = Vec::with_capacity((width.div_ceil(64) * depth.div_ceil(64)) as usize * 3);
    let mut samples = vec![0_i16; cell_count];
    for z in 0..depth {
        for x in 0..width {
            let world_x = base_x + x as i32;
            let world_z = base_z + z as i32;
            let quarter_meters =
                terrain_height_with_geometry_carved(seed, world_x, world_z, water, carves);
            let index = (z * width + x) as usize;
            samples[index] = quarter_meters;
            height.extend_from_slice(&quarter_meters.to_le_bytes());
            surface.push(surface_kind_with_geometry(
                seed,
                world_x,
                world_z,
                quarter_meters,
                water,
            ));
        }
    }
    for block_z in (0..depth).step_by(64) {
        for block_x in (0..width).step_by(64) {
            let mut total = 0_i32;
            let mut count = 0_i32;
            for z in block_z..(block_z + 64).min(depth) {
                for x in block_x..(block_x + 64).min(width) {
                    total += i32::from(samples[(z * width + x) as usize]);
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
    (height, surface, lod2)
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

/// Lightweight, deterministic erosion carving for natural terrain.
///
/// This is NOT a particle simulation (which would break the pure-function
/// heightfield). It mimics the *result* of hydrological erosion: valley
/// networks are cut deeper where drainage concentrates, while ridges stay
/// high. The carve is a smooth function of world position so it is
/// chunk-continuous and matches the entity world_y samples exactly.
///
/// It is masked to zero inside settled land so cities and their surroundings
/// stay flat — erosion only reshapes the wild valley / hill ground.
fn erosion_carve(seed: u64, x: i32, z: i32, water: WaterGeometry) -> f64 {
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
    -(strength_s * 30.0)
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
    let cx = width * 0.5;
    let cz = depth * 0.5;
    // Radial skirt: normalised distance from the scene centre, but the term is
    // an even function that reaches its max at the scene edge and does NOT
    // grow outside it (terrain_height is sampled past the edge by continuity
    // checks, and an unbounded quadratic there would create fake cliffs).
    let scale = width.max(depth) * 0.62;
    let dx = ((xf - cx) / scale).abs().min(1.0);
    let dz = ((zf - cz) / scale).abs().min(1.0);
    let radial = (dx * dx + dz * dz).sqrt();
    if water.smooth_rolling {
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
    }
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
    let cx = width * 0.5;
    let cz = depth * 0.5;
    // City radius is primarily scene-driven so every profile keeps a
    // comparable urban heart; the urban+suburban share only nudges the
    // envelope. Density is expressed by `core_density`/`suburban_density`
    // inside each region, not by shrinking the whole city.
    let envelope = 0.75 + (urban_scale * 0.25);
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
    let urban = smooth * (1.0 + noise * 0.25);
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
        if urban > 0.72 {
            RegionType::UrbanCore
        } else if urban > 0.5 {
            RegionType::Urban
        } else if urban > 0.28 {
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
fn plan_terrain_carves(entities: &[EntityInstance]) -> Vec<TerrainCarve> {
    let mut carves = Vec::new();
    for entity in entities {
        let kind = entity.kind.as_str();
        let width = f64::from(entity.width_m);
        let depth = f64::from(entity.depth_m);
        let is_road = matches!(kind, "road" | "sidewalk" | "canal" | "causeway");
        let is_building = matches!(
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
        );
        if !is_road && !is_building {
            continue;
        }
        // Roads are long strips: the running length is `width` and the cross
        // section is `depth`. The pad uses the running dimension along the
        // longer axis so the strip stays flat across its cross-section.
        let (half_w, half_d, blend) = if is_road {
            if width >= depth {
                // Strip runs along X: cross-section (half) is on Z.
                (depth * 0.5 + 1.5, width * 0.5 + 4.0, 6.0)
            } else {
                // Strip runs along Z: cross-section is on X.
                (width * 0.5 + 4.0, depth * 0.5 + 1.5, 6.0)
            }
        } else {
            (width * 0.5 + 1.0, depth * 0.5 + 1.0, 6.0)
        };
        carves.push(TerrainCarve {
            cx: f64::from(entity.world_x),
            cz: f64::from(entity.world_z),
            half_w,
            half_d,
            target_h_m: f64::from(entity.ground_height_m()),
            blend_m: blend,
            priority: if is_road { 10 } else { 1 },
        });
    }
    // Higher-priority pads (roads) must win when several overlap a cell, so
    // sort ascending and apply in that order — the last write overwrites, so
    // the road pad ends up on top of any neighbouring building pad.
    carves.sort_by(|a, b| a.priority.cmp(&b.priority));
    carves
}

/// Same as [`terrain_height_with_geometry`] but first flattens any road /
/// building pads that cover this cell. Natural (uncarved) calls use the plain
/// function; chunk baking passes the carve list so baked heightfields carry
/// the flat pads that entities then sit on.
fn terrain_height_with_geometry_carved(
    seed: u64,
    x: i32,
    z: i32,
    water: WaterGeometry,
    carves: &[TerrainCarve],
) -> i16 {
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
    let natural = landform + detail * (3.0 + landform.max(0.0) * 0.18) + 14.0;
    // Erosion carves valleys into wild ground only; the water branches below
    // then clamp the lake/river bed, and settled land is masked to zero.
    let natural = natural + erosion_carve(seed, x, z, water);
    let mut terrain = match water.kind {
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
            let half_width = river_half_width_at(
                zf - water.center_z + water.half_depth,
                (water.half_depth * 2.0) as u32,
                water.half_width,
            );
            let distance = (xf - water.center_x).abs();
            // The water surface is the fixed 0 datum. Inside the channel the
            // river bed sits below it (a shallow channel of a few metres so
            // the water is never a zero-thickness disc). On the shore the
            // terrain rises smoothly from the bed to a height that is ALWAYS
            // above the water surface: a floodplain dip must never let the
            // sea climb onto dry land, so the far-shore height is clamped to
            // at least one metre above datum. This mirrors mapgen4's rule
            // that elevation 0 is the shoreline and land is elevation > 0.
            let shore_target = natural.max(1.5);
            if distance < half_width {
                let depth_t = (distance / half_width.max(1.0)).clamp(0.0, 1.0);
                let bed = -5.0 + depth_t * depth_t * 4.0;
                bed.max(shore_target * 0.2 - 6.0).max(-7.0)
            } else if distance < half_width * 1.35 {
                let t = ((distance - half_width) / (half_width * 0.35)).clamp(0.0, 1.0);
                let smooth_t = t * t * (3.0 - 2.0 * t);
                // Start from the channel-bed level at the waterline and ease
                // into the (above-water) shore target.
                -4.0 + (shore_target + 4.0) * smooth_t
            } else {
                shore_target
            }
        }
        WaterKind::None => natural,
    };
    // Apply terrain carving: a pad flattens its footprint to the road/building
    // target height and blends back to the natural terrain over a smooth ring.
    // Higher-priority pads (roads) win when several overlap the same cell.
    for carve in carves {
        let dx = (xf - carve.cx).abs();
        let dz = (zf - carve.cz).abs();
        let outside_x = dx - carve.half_w;
        let outside_z = dz - carve.half_d;
        let outside = outside_x.max(outside_z);
        if outside <= 0.0 {
            terrain = carve.target_h_m;
        } else if outside < carve.blend_m {
            let t = (outside / carve.blend_m).clamp(0.0, 1.0);
            let smooth_t = t * t * (3.0 - 2.0 * t);
            terrain = carve.target_h_m + (terrain - carve.target_h_m) * smooth_t;
        }
    }
    (terrain.mul_add(4.0, 0.0)).clamp(-128.0, 1_024.0) as i16
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
    (shore_damp + band + elevation_effect).clamp(0.0, 1.0)
}

#[cfg(test)]
fn lake_radius(x: f64, z: f64, scene_width: u32, scene_depth: u32) -> f64 {
    lake_radius_at(
        x,
        z,
        legacy_water_geometry(WaterKind::Lake, scene_width, scene_depth, 0.0),
    )
}

fn lake_radius_at(x: f64, z: f64, water: WaterGeometry) -> f64 {
    let dx = (x - water.center_x) / water.half_width.max(1.0);
    let dz = (z - water.center_z) / water.half_depth.max(1.0);
    dx * dx + dz * dz
}

#[cfg(test)]
fn surface_kind(
    seed: u64,
    x: i32,
    z: i32,
    height: i16,
    water: WaterKind,
    scene_width: u32,
    scene_depth: u32,
    river_half_m: f64,
) -> u8 {
    surface_kind_with_geometry(
        seed,
        x,
        z,
        height,
        legacy_water_geometry(water, scene_width, scene_depth, river_half_m),
    )
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
            let distance = (f64::from(x) - water.center_x).abs();
            let half_width = river_half_width_at(
                f64::from(z) - water.center_z + water.half_depth,
                (water.half_depth * 2.0) as u32,
                water.half_width,
            );
            if distance < half_width {
                return 3;
            }
            if distance < half_width * 1.35 {
                // Shore transition: wet sand / tidal mud at the waterline,
                // easing into the above-water surface further out. Kept as a
                // mud/sand surface (not water) so the bay reads as one body
                // with a natural wet edge instead of scattered puddles.
                return 4;
            }
        }
        WaterKind::None => {}
    }
    // Eroded valley channels (rivers on land): where the hydrological carve
    // has cut the terrain down near the water datum and the flow is strong,
    // mark the channel as wet mud so rivers read as water courses on the
    // ground instead of grass grooves.
    if water.kind != WaterKind::None && height < 8 && erosion_carve(seed, x, z, water) < -14.0 {
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
    generate_entities_with_profile(seed, scene, landmarks, geometry, &serde_json::Value::Null)
}

fn generate_entities_with_profile(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
    style: &serde_json::Value,
) -> Vec<EntityInstance> {
    let mut entities = generate_entities_template(seed, scene, landmarks, geometry);
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
                f64::from(entity.width_m) * 0.5,
                f64::from(entity.depth_m) * 0.5 + 30.0,
            )
        })
        .collect();
    // Tree canopies must not overlap buildings: a tree pushed against a shop
    // front or a resort wall reads as clipping, not as street planting. We
    // collect every building footprint once, then drop any tree whose crown
    // reaches into one.
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
            )
        })
        .map(|entity| {
            (
                entity.world_x,
                entity.world_z,
                f64::from(entity.width_m) * 0.5 + 0.5,
                f64::from(entity.depth_m) * 0.5 + 0.5,
            )
        })
        .collect();
    entities.retain(|entity| {
        // A tree growing out of a building wall is a placement error: the
        // crown must clear every building footprint (roads are fine — street
        // trees line the carriageway).
        if entity.kind == "tree" {
            let crown_r = f64::from(entity.width_m) * 0.5;
            let overlaps_building = building_footprints.iter().any(|(bx, bz, bhx, bhz)| {
                f64::from((entity.world_x - bx).abs()) < crown_r + bhx
                    && f64::from((entity.world_z - bz).abs()) < crown_r + bhz
            });
            if overlaps_building {
                return false;
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
                f64::from((entity.world_x - bx).abs()) < f64::from(entity.width_m) * 0.5 + bhx
                    && f64::from((entity.world_z - bz).abs())
                        < f64::from(entity.depth_m) * 0.5 + bhz
            });
            if overlaps_bridge {
                return false;
            }
        }
        matches!(
            entity.kind.as_str(),
            "lake" | "river" | "bridge" | "causeway" | "reed"
        ) || entity.entity_id.starts_with("gis.")
            || !footprint_intersects_water(entity, geometry, 1.0)
    });
    if geometry.kind == WaterKind::None {
        // naturalOnly: a pure wild scene (steppe / nature) — skip urban land-use
        // generation entirely, keep only the natural entities (pasture, trees,
        // rocks, grass) so the world stays a wild plain with no city fabric.
        let natural_only = style
            .get("naturalOnly")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !natural_only {
            if style.get("landUseProfile").is_some() {
                append_modern_landuse_content(seed, scene, &mut entities, style);
            } else {
                append_generic_land_content(seed, scene, &mut entities);
            }
        } else {
            append_natural_content(seed, scene, &mut entities);
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
    if rules_mode == "rules" {
        entities = apply_rules_mode(seed, scene, geometry, style, entities);
    }
    entities
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
    proposed: Vec<EntityInstance>,
) -> Vec<EntityInstance> {
    // Object rules location: <repo>/assets/objects (resolved from the caller).
    let rules_dir = rules_dir_from_style(style);
    // bake_world() validates the same directory before generation. Keep this
    // second load fail-closed as well: a rules-mode generation must never
    // silently downgrade to legacy placement if the registry changes or
    // becomes unreadable between validation and use.
    let registry = crate::rules::ObjectRegistry::load_dir(&rules_dir)
        .unwrap_or_else(|error| panic!("rules mode registry became unavailable: {error}"));

    let ctx = rules_placement_context(seed, scene, water);
    // Every proposed entity that has a matching descriptor becomes one request.
    // Entities without a descriptor (landmarks, water bodies) pass through.
    let mut requests = Vec::new();
    let mut passthrough = Vec::new();
    for e in proposed {
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

    let outcome = crate::rules::place_all(&registry, &requests, &ctx, seed);
    // Merge: rule-placed entities + descriptor-less passthrough.
    let mut merged = outcome.placed;
    merged.extend(passthrough);
    merged
}

/// Resolve the object-rules directory: `style.rulesDir` if set, else the
/// default `assets/objects` relative to the current directory.
fn rules_dir_from_style(style: &serde_json::Value) -> std::path::PathBuf {
    if let Some(dir) = style.get("rulesDir").and_then(serde_json::Value::as_str) {
        return std::path::PathBuf::from(dir);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("assets")
        .join("objects")
}

/// Build the rules PlacementContext for a scene (natural terrain queries).
/// Closures are leaked to 'static so the context can be passed around freely
/// (audit / place_all both take borrowed contexts).
fn rules_placement_context(
    seed: u64,
    scene: &SceneSpec,
    water: WaterGeometry,
) -> crate::rules::PlacementContext<'static> {
    let scene_w = scene.width_m;
    let scene_d = scene.depth_m;
    let natural_height = move |x: i32, z: i32| {
        terrain_height(seed, x, z, WaterKind::None, scene_w, scene_d, 0.0) as f32 / 4.0
    };
    let height_leak: &'static dyn Fn(i32, i32) -> f32 = Box::leak(Box::new(natural_height));
    let water_level = move |x: i32, z: i32| match water.kind {
        WaterKind::None => None,
        WaterKind::Lake => {
            if lake_radius_at(x as f64, z as f64, water) < 1.0 {
                Some(f32::MAX)
            } else {
                None
            }
        }
        WaterKind::River => {
            let half_width = river_half_width_at(
                z as f64 - water.center_z + water.half_depth,
                (water.half_depth * 2.0) as u32,
                water.half_width,
            );
            if (x as f64 - water.center_x).abs() < half_width {
                Some(f32::MAX)
            } else {
                None
            }
        }
    };
    let water_leak: &'static dyn Fn(i32, i32) -> Option<f32> = Box::leak(Box::new(water_level));
    let slope_at = move |x: i32, z: i32| local_slope(seed, x, z, scene_w, scene_d, 8, 8) as f32;
    let slope_leak: &'static dyn Fn(i32, i32) -> f32 = Box::leak(Box::new(slope_at));
    let slope_at_footprint = move |x: i32, z: i32, half_w: i32, half_d: i32| {
        local_slope(seed, x, z, scene_w, scene_d, half_w, half_d) as f32
    };
    let slope_footprint_leak: &'static dyn Fn(i32, i32, i32, i32) -> f32 =
        Box::leak(Box::new(slope_at_footprint));
    let biome_at = move |x: i32, z: i32| {
        let h = humidity_field(seed, x, z, water);
        if h > 0.55 {
            crate::rules::Biome::Forest
        } else if h < -0.2 {
            crate::rules::Biome::Desert
        } else {
            crate::rules::Biome::Grassland
        }
    };
    let biome_leak: &'static dyn Fn(i32, i32) -> crate::rules::Biome =
        Box::leak(Box::new(biome_at));
    let hazard_at = move |x: i32, z: i32| {
        if local_slope(seed, x, z, scene_w, scene_d, 8, 8) > 30.0 {
            vec![crate::rules::HazardKind::Cliff]
        } else {
            Vec::new()
        }
    };
    let hazard_leak: &'static dyn Fn(i32, i32) -> Vec<crate::rules::HazardKind> =
        Box::leak(Box::new(hazard_at));
    crate::rules::PlacementContext {
        height_at: height_leak,
        water_level: water_leak,
        slope_at: slope_leak,
        slope_at_footprint: Some(slope_footprint_leak),
        bounds: (
            scene.origin_x,
            scene.origin_z,
            scene.origin_x + scene.width_m as i32,
            scene.origin_z + scene.depth_m as i32,
        ),
        grounding_tolerance: 0.5,
        biome_at: Some(biome_leak),
        hazard_at: Some(hazard_leak),
    }
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
    let natural_height = |x: i32, z: i32| {
        terrain_height(
            seed,
            x,
            z,
            WaterKind::None,
            scene.width_m,
            scene.depth_m,
            0.0,
        ) as f32
            / 4.0
    };
    let height_at = opts.height_at.unwrap_or(&natural_height);
    let slope_half = opts.slope_half.unwrap_or((8, 8));
    let ctx = crate::rules::PlacementContext {
        height_at,
        water_level: &|x, z| match water.kind {
            WaterKind::None => None,
            // A point "in water" means its surface kind is water (lake radius
            // < 1.0 or within the river half-width). Report a very high water
            // level so the rules engine's height<=water test fires.
            WaterKind::Lake => {
                if lake_radius_at(x as f64, z as f64, water) < 1.0 {
                    Some(f32::MAX)
                } else {
                    None
                }
            }
            WaterKind::River => {
                let half_width = river_half_width_at(
                    z as f64 - water.center_z + water.half_depth,
                    (water.half_depth * 2.0) as u32,
                    water.half_width,
                );
                if (x as f64 - water.center_x).abs() < half_width {
                    Some(f32::MAX)
                } else {
                    None
                }
            }
        },
        slope_at: &|x, z| {
            local_slope(
                seed,
                x,
                z,
                scene.width_m,
                scene.depth_m,
                slope_half.0,
                slope_half.1,
            ) as f32
        },
        slope_at_footprint: Some(&|x, z, half_w, half_d| {
            local_slope(seed, x, z, scene.width_m, scene.depth_m, half_w, half_d) as f32
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
            let s = local_slope(
                seed,
                x,
                z,
                scene.width_m,
                scene.depth_m,
                slope_half.0,
                slope_half.1,
            );
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

/// Natural-only filler for wild scenes (steppe / nature): sparse trees,
/// bushes, rocks and grass clumps over a deterministic grid, with jitter.
/// No roads, no buildings, no paths — just open wild land.
fn append_natural_content(seed: u64, scene: &SceneSpec, entities: &mut Vec<EntityInstance>) {
    let mut serial = 0_u32;
    for z in (20..scene.depth_m.saturating_sub(8)).step_by(16) {
        for x in (20..scene.width_m.saturating_sub(8)).step_by(16) {
            let jx = signed_noise(seed.rotate_left(211), x as i32, z as i32);
            let jz = signed_noise(seed.rotate_left(223), x as i32, z as i32);
            let wx = scene.origin_x + x as i32 + (jx.rem_euclid(19) - 9);
            let wz = scene.origin_z + z as i32 + (jz.rem_euclid(19) - 9);
            let world_y = i32::from(terrain_height(
                seed,
                wx,
                wz,
                WaterKind::None,
                scene.width_m,
                scene.depth_m,
                0.0,
            )) / 4;
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
                entity_id: format!("generated.natural.{serial}"),
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
            serial += 1;
        }
    }
}

fn append_generic_land_content(seed: u64, scene: &SceneSpec, entities: &mut Vec<EntityInstance>) {
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
    let mut serial = 100_000_u32;
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
                entity_id: format!("generated.auto-{kind}.{serial}"),
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
            serial += 1;
        }
    }
}

fn append_modern_landuse_content(
    seed: u64,
    scene: &SceneSpec,
    entities: &mut Vec<EntityInstance>,
    style: &serde_json::Value,
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
    let mut occ = OccupancyGrid::new(scene.width_m, scene.depth_m);
    let mut add = |kind: &str, x: i32, z: i32, width: f32, depth: f32, height: f32| {
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
                return;
            }
        } else {
            let half_width = (width * 0.5).ceil() as i32;
            let half_depth = (depth * 0.5).ceil() as i32;
            if x - half_width < min_x
                || x + half_width > max_x
                || z - half_depth < min_z
                || z + half_depth > max_z
            {
                return;
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
            let max_slope = if is_building {
                30.0
            } else if matches!(entity_kind, "mountain_forest" | "tree" | "bush") {
                70.0
            } else {
                30.0
            };
            if slope > max_slope {
                return;
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
            return;
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
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
            entity_id: format!("generated.landuse-{kind}.{serial}"),
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
            let spacing = params.road_spacing_m;
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
            let spacing = params.road_spacing_m;
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
    for z in (min_z..max_z).step_by(120) {
        for x in (min_x..max_x).step_by(120) {
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
                    smooth_rolling: false,
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
                entity_id: format!("generated.landuse-mountain_forest.{serial}"),
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
            serial += 1;
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

fn footprint_intersects_water(entity: &EntityInstance, water: WaterGeometry, margin: f64) -> bool {
    if water.kind == WaterKind::None {
        return false;
    }
    let half_x = f64::from(entity.width_m) * 0.5 + margin;
    let half_z = f64::from(entity.depth_m) * 0.5 + margin;
    let samples = [
        (f64::from(entity.world_x), f64::from(entity.world_z)),
        (
            f64::from(entity.world_x) - half_x,
            f64::from(entity.world_z) - half_z,
        ),
        (
            f64::from(entity.world_x) - half_x,
            f64::from(entity.world_z) + half_z,
        ),
        (
            f64::from(entity.world_x) + half_x,
            f64::from(entity.world_z) - half_z,
        ),
        (
            f64::from(entity.world_x) + half_x,
            f64::from(entity.world_z) + half_z,
        ),
    ];
    samples.iter().any(|(x, z)| match water.kind {
        WaterKind::Lake => lake_radius_at(*x, *z, water) < 1.0,
        WaterKind::River => {
            let half_width = river_half_width_at(
                *z - water.center_z + water.half_depth,
                (water.half_depth * 2.0) as u32,
                water.half_width,
            );
            (*x - water.center_x).abs() < half_width
        }
        WaterKind::None => false,
    })
}

fn generate_entities_template(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    geometry: WaterGeometry,
) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for landmark in landmarks {
        entities.push(EntityInstance {
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
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
            "road" | "building" | "building_tower" | "storefront"
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
                world_y: i32::from(terrain_height_with_geometry(
                    seed,
                    lake_x,
                    north_shore_z,
                    geometry,
                )) / 4,
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
        let mut block_serial = 40_000_u32;
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
                            entity_id: format!("generated.resort-lodge.{block_serial}"),
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
                            entity_id: format!("generated.block-tower.{block_serial}"),
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
                    block_serial += 1;
                }
            }
        }
        let mut frontage_serial = 50_000_u32;
        for side in [-1_i32, 1_i32] {
            let frontage_x = center_x + side * (half_width + 150);
            // Skip the whole shopfront row if the road-side lot would fall
            // outside the scene (narrow bayfront slices).
            if frontage_x < 40 || frontage_x > scene.width_m as i32 - 40 {
                continue;
            }
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
                    entity_id: format!("generated.storefront.{frontage_serial}"),
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
                frontage_serial += 1;
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
            let mut tree_serial = 70_000_u32 + if side < 0 { 0 } else { 10_000 };
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
                    entity_id: format!("generated.street-tree.{tree_serial}"),
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
                tree_serial += 1;
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
                    entity_id: format!("generated.street-tree.{tree_serial}"),
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
                tree_serial += 1;
            }
            let second_row_x = frontage_x + side * 72;
            // Only a second row of shops if there is actually room for it on
            // the scene side — in a narrow bayfront slice (like Shenzhen Bay)
            // the frontage may already sit near the scene edge, so pushing a
            // second row past it would place shops outside the world.
            if second_row_x > 40 && second_row_x < scene.width_m as i32 - 40 {
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
                        entity_id: format!("generated.storefront.{frontage_serial}"),
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
                    frontage_serial += 1;
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
                let mut urban_serial = 60_000_u32 + if side < 0 { 0 } else { 20_000 };
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
                            entity_id: format!("generated.urban-building.{urban_serial}"),
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
                        urban_serial += 1;
                    }
                }
            }
        }
    }
    let mut serial = 0_u32;
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
                entity_id: format!("generated.{kind}.{serial}"),
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
            serial += 1;
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
                    entity_id: format!("generated.{kind}.{serial}"),
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
                serial += 1;
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
        assert_eq!(building.anchors[0].direction, "east");
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
    fn terrain_carve_flattens_pad_and_blends_to_natural() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        let carve = TerrainCarve {
            cx: 400.0,
            cz: 400.0,
            half_w: 50.0,
            half_d: 40.0,
            target_h_m: 12.0,
            blend_m: 4.0,
            priority: 1,
        };
        // Inside the pad the height is the flat target, independent of the
        // underlying natural terrain (which varies with position).
        for (x, z) in [(350, 360), (400, 400), (449, 439), (370, 425)] {
            assert_eq!(
                terrain_height_with_geometry_carved(20260829, x, z, water, &[carve]),
                12 * 4,
                "pad at ({x},{z}) not flattened to target"
            );
        }
        // Far outside the pad the natural terrain is untouched.
        let natural_at = terrain_height_with_geometry_carved(20260829, 700, 700, water, &[]);
        assert_eq!(
            terrain_height_with_geometry_carved(20260829, 700, 700, water, &[carve]),
            natural_at,
            "far field altered by carve"
        );
        // Inside the pad the terrain is flat (zero local slope).
        let a = terrain_height_with_geometry_carved(20260829, 380, 400, water, &[carve]);
        let b = terrain_height_with_geometry_carved(20260829, 420, 400, water, &[carve]);
        let c = terrain_height_with_geometry_carved(20260829, 400, 380, water, &[carve]);
        assert_eq!(a, b, "pad not flat along X");
        assert_eq!(a, c, "pad not flat along Z");
        // The blend ring is continuous (each step bounded) so no cliff appears.
        let mut max_step = 0_i16;
        for z in 430..450 {
            for x in 440..460 {
                let here = terrain_height_with_geometry_carved(20260829, x, z, water, &[carve]);
                max_step = max_step.max(
                    (here
                        - terrain_height_with_geometry_carved(20260829, x - 1, z, water, &[carve]))
                    .abs(),
                );
                max_step = max_step.max(
                    (here
                        - terrain_height_with_geometry_carved(20260829, x, z - 1, water, &[carve]))
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
    fn erosion_carves_valleys_but_keeps_city_flat() {
        let water = legacy_water_geometry(WaterKind::None, 1_000, 1_000, 0.0);
        // City centre: high urbanisation => erosion must be zero so the built
        // area and its terrain stay flat (a city sits on a level pad).
        let mut urban_cells = 0;
        let mut carved_sum = 0.0;
        for z in (450..550).step_by(10) {
            for x in (450..550).step_by(10) {
                let c = erosion_carve(20260829, x, z, water);
                if c != 0.0 {
                    urban_cells += 1;
                    carved_sum += c;
                }
            }
        }
        // Some cells in the very heart may still be rural by noise, but the
        // majority of the central 100x100 must be untouched.
        assert!(
            urban_cells <= 40,
            "erosion touched {urban_cells} central city cells (expected <= 40)"
        );
        // Erosion never raises terrain, only carves down.
        for z in (100..900).step_by(23) {
            for x in (100..900).step_by(23) {
                assert!(
                    erosion_carve(20260829, x, z, water) <= 0.0,
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
                    .map(move |x| erosion_carve(20260829, x, z, water))
            })
            .filter(|c| *c < -2.0)
            .count();
        assert!(
            wild_carved > 100,
            "erosion barely carved wild terrain ({wild_carved} deep cells)"
        );
        // Erosion stays continuous across chunk boundaries (it is a pure
        // function of world coordinates).
        let a = erosion_carve(20260829, 511, 200, water);
        let b = erosion_carve(20260829, 512, 200, water);
        let c = erosion_carve(20260829, 200, 511, water);
        let d = erosion_carve(20260829, 200, 512, water);
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
            i32::from(terrain_height(
                1208,
                road.world_x,
                road.world_z,
                WaterKind::Lake,
                1_000,
                1_000,
                0.0
            )) / 4
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
                entities,
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
            smooth_rolling: false,
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
            smooth_rolling: false,
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
            smooth_rolling: false,
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
            smooth_rolling: false,
        }
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
}
