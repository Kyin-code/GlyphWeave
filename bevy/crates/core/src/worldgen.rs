//! Deterministic large-world planning and sidecar baking for engine adapters.
//!
//! Public world coordinates use named `world_x`, `world_z`, and `world_y`
//! fields. The `.gemap` v3 codec keeps its frozen `(z, x, y)` protocol order.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::storage::codec::encode_world_with_metadata;
use crate::voxel::{VoxelCoord, VoxelWorld};

pub const WORLD_FORMAT: &str = "glyphweave-world";
pub const WORLD_VERSION: u32 = 1;
pub const STREAM_CHUNK_METERS: u32 = 512;
pub const MIN_SCENE_METERS: u32 = 512;
pub const MAX_SCENE_WIDTH_METERS: u32 = 6_000;
pub const MAX_SCENE_DEPTH_METERS: u32 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSpec {
    pub name: String,
    pub seed: u64,
    #[serde(default = "default_render_mode")]
    pub render_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldPatch {
    pub format: String,
    pub version: u32,
    pub patch_id: String,
    pub operations: Vec<PatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum PatchOperation {
    MoveLandmark { entity_id: String, world_x: i32, world_z: i32, world_y: i32 },
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
    #[error("scene {scene_id:?} dimensions {width_m}x{depth_m}m are outside {MIN_SCENE_METERS}m..{MAX_SCENE_WIDTH_METERS}m by {MIN_SCENE_METERS}m..{MAX_SCENE_DEPTH_METERS}m")]
    InvalidSceneSize { scene_id: String, width_m: u32, depth_m: u32 },
    #[error("landmark {0:?} is missing required narrative or asset fields")]
    InvalidLandmark(String),
    #[error("landmark {landmark:?} references missing scene {scene:?}")]
    MissingLandmarkScene { landmark: String, scene: String },
    #[error("landmark {landmark:?} is outside scene {scene:?}")]
    LandmarkOutsideScene { landmark: String, scene: String },
    #[error("output directory already contains files: {0}")]
    OutputNotEmpty(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type WorldgenResult<T> = Result<T, WorldgenError>;

fn default_render_mode() -> String { "2.5d".to_owned() }

impl WorldManifest {
    pub fn default_demo() -> Self {
        Self {
            format: WORLD_FORMAT.to_owned(),
            version: WORLD_VERSION,
            world: WorldSpec { name: "GlyphWeave World".to_owned(), seed: 42, render_mode: default_render_mode() },
            scenes: vec![SceneSpec { scene_id: "scene-0".to_owned(), width_m: 1_000, depth_m: 1_000, origin_x: 0, origin_z: 0, seed_offset: 0 }],
            style: serde_json::json!({"family":"user-confirmed", "terrain":"stylized-low-poly"}),
            landmarks: Vec::new(),
        }
    }

    pub fn validate(&self) -> WorldgenResult<()> {
        if self.format != WORLD_FORMAT { return Err(WorldgenError::InvalidFormat(self.format.clone())); }
        if self.version != WORLD_VERSION { return Err(WorldgenError::UnsupportedVersion(self.version)); }
        if self.world.name.trim().is_empty() { return Err(WorldgenError::EmptyWorldName); }
        if self.world.render_mode != "2d" && self.world.render_mode != "2.5d" { return Err(WorldgenError::InvalidRenderMode(self.world.render_mode.clone())); }
        let mut scene_ids = BTreeMap::new();
        for scene in &self.scenes {
            if scene.scene_id.trim().is_empty() || scene_ids.insert(&scene.scene_id, true).is_some() {
                return Err(WorldgenError::InvalidSceneId(scene.scene_id.clone()));
            }
            if !(MIN_SCENE_METERS..=MAX_SCENE_WIDTH_METERS).contains(&scene.width_m)
                || !(MIN_SCENE_METERS..=MAX_SCENE_DEPTH_METERS).contains(&scene.depth_m) {
                return Err(WorldgenError::InvalidSceneSize { scene_id: scene.scene_id.clone(), width_m: scene.width_m, depth_m: scene.depth_m });
            }
        }
        for landmark in &self.landmarks {
            if landmark.entity_id.trim().is_empty() || landmark.name.trim().is_empty() || landmark.entity_type.trim().is_empty()
                || landmark.purpose.trim().is_empty() || landmark.description.trim().is_empty() || landmark.asset_id.trim().is_empty()
                || landmark.width_m == 0 || landmark.depth_m == 0 || landmark.height_m == 0 {
                return Err(WorldgenError::InvalidLandmark(landmark.entity_id.clone()));
            }
            let Some(scene) = self.scenes.iter().find(|scene| scene.scene_id == landmark.scene_id) else {
                return Err(WorldgenError::MissingLandmarkScene { landmark: landmark.entity_id.clone(), scene: landmark.scene_id.clone() });
            };
            let local_x = landmark.world_x - scene.origin_x;
            let local_z = landmark.world_z - scene.origin_z;
            if local_x < 0 || local_z < 0 || local_x as u32 >= scene.width_m || local_z as u32 >= scene.depth_m {
                return Err(WorldgenError::LandmarkOutsideScene { landmark: landmark.entity_id.clone(), scene: scene.scene_id.clone() });
            }
        }
        Ok(())
    }
}

pub fn bake_world(manifest: &WorldManifest, output: &Path) -> WorldgenResult<WorldIndex> {
    manifest.validate()?;
    if output.exists() && fs::read_dir(output)?.next().is_some() {
        return Err(WorldgenError::OutputNotEmpty(output.display().to_string()));
    }
    fs::create_dir_all(output)?;
    let revision = blake3::hash(&serde_json::to_vec(manifest)?).to_hex().to_string();
    let mut scene_paths = Vec::new();
    for scene in &manifest.scenes {
        let scene_dir = output.join("scenes").join(&scene.scene_id);
        fs::create_dir_all(&scene_dir)?;
        let chunk_count_x = scene.width_m.div_ceil(STREAM_CHUNK_METERS);
        let chunk_count_z = scene.depth_m.div_ceil(STREAM_CHUNK_METERS);
        let mut chunks = Vec::new();
        for chunk_z in 0..chunk_count_z {
            for chunk_x in 0..chunk_count_x {
                let valid_width_m = (scene.width_m - chunk_x * STREAM_CHUNK_METERS).min(STREAM_CHUNK_METERS);
                let valid_depth_m = (scene.depth_m - chunk_z * STREAM_CHUNK_METERS).min(STREAM_CHUNK_METERS);
                let base_x = scene.origin_x + (chunk_x * STREAM_CHUNK_METERS) as i32;
                let base_z = scene.origin_z + (chunk_z * STREAM_CHUNK_METERS) as i32;
                let stem = format!("chunk-{chunk_x}-{chunk_z}");
                let height_file = format!("{stem}.height.bin");
                let surface_file = format!("{stem}.surface.bin");
                let lod2_file = format!("{stem}.lod2.bin");
                let waterfront = manifest.style.to_string().contains("西湖");
                let (height, surface, lod2) = generate_chunk(manifest.world.seed ^ scene.seed_offset, base_x, base_z, valid_width_m, valid_depth_m, waterfront);
                fs::write(scene_dir.join(&height_file), &height)?;
                fs::write(scene_dir.join(&surface_file), &surface)?;
                fs::write(scene_dir.join(&lod2_file), &lod2)?;
                let hash = blake3::hash(&[height.as_slice(), surface.as_slice(), lod2.as_slice()].concat()).to_hex().to_string();
                let descriptor = ChunkDescriptor { chunk_x, chunk_z, world_x: base_x, world_z: base_z, valid_width_m, valid_depth_m, height_file, surface_file, lod2_file, hash };
                fs::write(scene_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&descriptor)?)?;
                chunks.push(descriptor);
            }
        }
        let landmarks: Vec<LandmarkSpec> = manifest.landmarks.iter().filter(|item| item.scene_id == scene.scene_id).cloned().collect();
        let waterfront = manifest.style.to_string().contains("西湖");
        let entities = generate_entities(manifest.world.seed ^ scene.seed_offset, scene, &landmarks, waterfront);
        let index = SceneIndex { scene_id: scene.scene_id.clone(), width_m: scene.width_m, depth_m: scene.depth_m, origin_x: scene.origin_x, origin_z: scene.origin_z, chunk_size_m: STREAM_CHUNK_METERS, chunk_count_x, chunk_count_z, chunks, landmarks, entities };
        fs::write(scene_dir.join("scene.json"), serde_json::to_vec_pretty(&index)?)?;
        scene_paths.push(format!("scenes/{}/scene.json", scene.scene_id));
    }
    write_gemap_anchor(output, manifest, &revision)?;
    let index = WorldIndex { format: WORLD_FORMAT.to_owned(), version: WORLD_VERSION, name: manifest.world.name.clone(), seed: manifest.world.seed, render_mode: manifest.world.render_mode.clone(), revision, scenes: scene_paths };
    fs::write(output.join("world.json"), serde_json::to_vec_pretty(&index)?)?;
    fs::write(output.join("glyphweave.manifest.json"), serde_json::to_vec_pretty(manifest)?)?;
    write_adapter_templates(output)?;
    Ok(index)
}

fn write_gemap_anchor(output: &Path, manifest: &WorldManifest, revision: &str) -> WorldgenResult<()> {
    let mut world = VoxelWorld::new(&manifest.world.name);
    let anchor = world.intern_block("glyphweave:world_anchor").map_err(|error| std::io::Error::other(error.to_string()))?;
    world.set(VoxelCoord::new(0, 0, 0), anchor).map_err(|error| std::io::Error::other(error.to_string()))?;
    let metadata = BTreeMap::from([("world".to_owned(), serde_json::json!({"revision": revision, "sidecar": "world.json"}))]);
    let bytes = encode_world_with_metadata(&world, Some(metadata)).map_err(|error| std::io::Error::other(error.to_string()))?;
    fs::write(output.join("world.gemap"), bytes)?;
    Ok(())
}

pub fn apply_patch(manifest: &WorldManifest, patch: &WorldPatch) -> WorldgenResult<WorldManifest> {
    manifest.validate()?;
    if patch.format != "glyphweave-world-patch" || patch.version != 1 { return Err(WorldgenError::InvalidFormat(patch.format.clone())); }
    let mut result = manifest.clone();
    for operation in &patch.operations {
        match operation {
            PatchOperation::MoveLandmark { entity_id, world_x, world_z, world_y } => {
                let landmark = result.landmarks.iter_mut().find(|item| item.entity_id == *entity_id)
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
    fs::write(preview.join("index.html"), include_str!("../../../../adapters/html/index.html"))?;
    fs::write(preview.join("app.js"), include_str!("../../../../adapters/html/app.js"))?;
    fs::write(godot.join("project.godot"), include_str!("../../../../adapters/godot/project.godot"))?;
    fs::write(godot.join("main.tscn"), include_str!("../../../../adapters/godot/main.tscn"))?;
    fs::write(godot.join("main.gd"), include_str!("../../../../adapters/godot/main.gd"))?;
    write_preview_assets(output)?;
    Ok(())
}

fn write_preview_assets(output: &Path) -> WorldgenResult<()> {
    let assets = output.join("assets");
    fs::create_dir_all(&assets)?;
    let files: &[(&str, &[u8])] = &[
        ("CommonTree_1.gltf", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/CommonTree_1.gltf")),
        ("CommonTree_1.bin", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/CommonTree_1.bin")),
        ("Bark_NormalTree_Normal.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Bark_NormalTree_Normal.png")),
        ("Bark_NormalTree.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Bark_NormalTree.png")),
        ("Leaves_NormalTree_C.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Leaves_NormalTree_C.png")),
        ("Leaves_NormalTree.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Leaves_NormalTree.png")),
        ("Bush_Common.gltf", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Bush_Common.gltf")),
        ("Bush_Common.bin", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Bush_Common.bin")),
        ("Leaves_TwistedTree_C.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Leaves_TwistedTree_C.png")),
        ("Pebble_Round_1.gltf", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Pebble_Round_1.gltf")),
        ("Pebble_Round_1.bin", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/Pebble_Round_1.bin")),
        ("PathRocks_Diffuse.png", include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/PathRocks_Diffuse.png")),
    ];
    for (name, data) in files { fs::write(assets.join(name), data)?; }
    fs::write(assets.join("LICENSE.txt"), include_bytes!("../../../../assets/third_party/quaternius/stylized-nature/License_Standard.txt"))?;
    fs::write(assets.join("glyphweave.registry.json"), include_bytes!("../../../../assets/glyphweave.registry.json"))?;
    Ok(())
}

pub fn write_demo_manifest(path: &Path) -> WorldgenResult<()> {
    fs::write(path, serde_json::to_vec_pretty(&WorldManifest::default_demo())?)?;
    Ok(())
}

fn generate_chunk(seed: u64, base_x: i32, base_z: i32, width: u32, depth: u32, waterfront: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let cell_count = (width * depth) as usize;
    let mut height = Vec::with_capacity(cell_count * 2);
    let mut surface = Vec::with_capacity(cell_count);
    let mut lod2 = Vec::with_capacity((width.div_ceil(64) * depth.div_ceil(64)) as usize * 3);
    let mut samples = vec![0_i16; cell_count];
    for z in 0..depth {
        for x in 0..width {
            let world_x = base_x + x as i32;
            let world_z = base_z + z as i32;
            let quarter_meters = terrain_height(seed, world_x, world_z, waterfront);
            let index = (z * width + x) as usize;
            samples[index] = quarter_meters;
            height.extend_from_slice(&quarter_meters.to_le_bytes());
            surface.push(surface_kind(seed, world_x, world_z, quarter_meters, waterfront));
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
            lod2.push(surface_kind(seed, base_x + block_x as i32, base_z + block_z as i32, average, waterfront));
        }
    }
    (height, surface, lod2)
}

fn terrain_height(seed: u64, x: i32, z: i32, waterfront: bool) -> i16 {
    let xf = f64::from(x);
    let zf = f64::from(z);
    let hills = (xf / 190.0).sin() * 36.0 + (zf / 240.0).cos() * 30.0;
    let detail = ((xf + zf) / 47.0).sin() * 8.0;
    let lake = if waterfront {
        let dx = (xf - 520.0) / 380.0;
        let dz = (zf - 470.0) / 260.0;
        if dx * dx + dz * dz < 1.0 { -42.0 } else { 0.0 }
    } else { 0.0 };
    let variation = f64::from(signed_noise(seed, x.div_euclid(64), z.div_euclid(64))) * 0.08;
    ((hills + detail + lake + variation).mul_add(4.0, 40.0)).clamp(-128.0, 1_024.0) as i16
}

fn surface_kind(seed: u64, x: i32, z: i32, height: i16, waterfront: bool) -> u8 {
    if waterfront {
        let dx = (f64::from(x) - 520.0) / 380.0;
        let dz = (f64::from(z) - 470.0) / 260.0;
        let radius = dx * dx + dz * dz;
        if radius < 1.0 { return 3; }
        if radius < 1.12 { return 4; }
    }
    if height < -12 { 3 } else if height > 700 { 2 } else if signed_noise(seed.rotate_left(31), x, z) > 70 { 1 } else { 0 }
}

fn generate_entities(seed: u64, scene: &SceneSpec, landmarks: &[LandmarkSpec], waterfront: bool) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for landmark in landmarks {
        entities.push(EntityInstance { entity_id: landmark.entity_id.clone(), asset_id: landmark.asset_id.clone(), kind: landmark.entity_type.clone(), world_x: landmark.world_x, world_z: landmark.world_z, world_y: landmark.world_y, scale: 1.0 });
    }
    if waterfront {
        entities.extend([
            EntityInstance { entity_id: "generated.north-shore-road".to_owned(), asset_id: "prop.road".to_owned(), kind: "road".to_owned(), world_x: scene.origin_x + 500, world_z: scene.origin_z + 96, world_y: 18, scale: 1.0 },
            EntityInstance { entity_id: "generated.lake-bridge".to_owned(), asset_id: "prop.bridge".to_owned(), kind: "bridge".to_owned(), world_x: scene.origin_x + 520, world_z: scene.origin_z + 245, world_y: 4, scale: 1.0 },
            EntityInstance { entity_id: "generated.west-village".to_owned(), asset_id: "prop.building-cluster".to_owned(), kind: "building_cluster".to_owned(), world_x: scene.origin_x + 790, world_z: scene.origin_z + 180, world_y: 18, scale: 1.0 },
        ]);
    }
    let mut serial = 0_u32;
    for z in (24..scene.depth_m.saturating_sub(16)).step_by(32) {
        for x in (24..scene.width_m.saturating_sub(16)).step_by(32) {
            let world_x = scene.origin_x + x as i32;
            let world_z = scene.origin_z + z as i32;
            if waterfront {
                let lake_x = (f64::from(world_x) - 520.0) / 380.0;
                let lake_z = (f64::from(world_z) - 470.0) / 260.0;
                if lake_x * lake_x + lake_z * lake_z < 1.15 { continue; }
            }
            let roll = signed_noise(seed.rotate_left(13), world_x, world_z);
            let kind = if roll > 112 { "building" } else if roll > -12 { "tree" } else if roll < -104 { "rock" } else if roll < 58 { "bush" } else { continue };
            let jitter_x = signed_noise(seed.rotate_left(7), world_x, world_z).rem_euclid(13) - 6;
            let jitter_z = signed_noise(seed.rotate_left(19), world_x, world_z).rem_euclid(13) - 6;
            let world_x = world_x + jitter_x;
            let world_z = world_z + jitter_z;
            let world_y = i32::from(terrain_height(seed, world_x, world_z, waterfront)) / 4;
            entities.push(EntityInstance { entity_id: format!("generated.{kind}.{serial}"), asset_id: format!("prop.{kind}"), kind: kind.to_owned(), world_x, world_z, world_y, scale: if kind == "building" { 1.8 } else { 1.0 } });
            serial += 1;
        }
    }
    entities
}

fn signed_noise(seed: u64, x: i32, z: i32) -> i32 {
    let mut value = seed ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (z as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
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

    #[test]
    fn default_world_is_valid() { WorldManifest::default_demo().validate().unwrap(); }

    #[test]
    fn same_seed_produces_same_chunk() {
        assert_eq!(generate_chunk(42, 0, 0, 32, 32, false), generate_chunk(42, 0, 0, 32, 32, false));
    }

    #[test]
    fn invalid_scene_is_rejected() {
        let mut world = WorldManifest::default_demo();
        world.scenes[0].width_m = 511;
        assert!(matches!(world.validate(), Err(WorldgenError::InvalidSceneSize { .. })));
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
        for entity in generate_entities(42, &scene, &[], true) {
            if !matches!(entity.kind.as_str(), "tree" | "bush" | "rock") { continue; }
            let dx = (f64::from(entity.world_x) - 520.0) / 380.0;
            let dz = (f64::from(entity.world_z) - 470.0) / 260.0;
            assert!(dx * dx + dz * dz >= 1.0, "{} spawned in water", entity.entity_id);
        }
    }
}
