//! Deterministic large-world planning and sidecar baking for engine adapters.
//!
//! Public world coordinates use named `world_x`, `world_z`, and `world_y`
//! fields. The `.gemap` v3 codec keeps its frozen `(z, x, y)` protocol order.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
                let (height, surface, lod2) = generate_chunk(manifest.world.seed ^ scene.seed_offset, base_x, base_z, valid_width_m, valid_depth_m);
                fs::write(scene_dir.join(&height_file), &height)?;
                fs::write(scene_dir.join(&surface_file), &surface)?;
                fs::write(scene_dir.join(&lod2_file), &lod2)?;
                let hash = blake3::hash(&[height.as_slice(), surface.as_slice(), lod2.as_slice()].concat()).to_hex().to_string();
                let descriptor = ChunkDescriptor { chunk_x, chunk_z, world_x: base_x, world_z: base_z, valid_width_m, valid_depth_m, height_file, surface_file, lod2_file, hash };
                fs::write(scene_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&descriptor)?)?;
                chunks.push(descriptor);
            }
        }
        let landmarks = manifest.landmarks.iter().filter(|item| item.scene_id == scene.scene_id).cloned().collect();
        let index = SceneIndex { scene_id: scene.scene_id.clone(), width_m: scene.width_m, depth_m: scene.depth_m, origin_x: scene.origin_x, origin_z: scene.origin_z, chunk_size_m: STREAM_CHUNK_METERS, chunk_count_x, chunk_count_z, chunks, landmarks };
        fs::write(scene_dir.join("scene.json"), serde_json::to_vec_pretty(&index)?)?;
        scene_paths.push(format!("scenes/{}/scene.json", scene.scene_id));
    }
    let index = WorldIndex { format: WORLD_FORMAT.to_owned(), version: WORLD_VERSION, name: manifest.world.name.clone(), seed: manifest.world.seed, render_mode: manifest.world.render_mode.clone(), revision, scenes: scene_paths };
    fs::write(output.join("world.json"), serde_json::to_vec_pretty(&index)?)?;
    fs::write(output.join("glyphweave.manifest.json"), serde_json::to_vec_pretty(manifest)?)?;
    write_adapter_templates(output)?;
    Ok(index)
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
    Ok(())
}

pub fn write_demo_manifest(path: &Path) -> WorldgenResult<()> {
    fs::write(path, serde_json::to_vec_pretty(&WorldManifest::default_demo())?)?;
    Ok(())
}

fn generate_chunk(seed: u64, base_x: i32, base_z: i32, width: u32, depth: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let cell_count = (width * depth) as usize;
    let mut height = Vec::with_capacity(cell_count * 2);
    let mut surface = Vec::with_capacity(cell_count);
    let mut lod2 = Vec::with_capacity((width.div_ceil(64) * depth.div_ceil(64)) as usize * 3);
    let mut samples = vec![0_i16; cell_count];
    for z in 0..depth {
        for x in 0..width {
            let world_x = base_x + x as i32;
            let world_z = base_z + z as i32;
            let quarter_meters = terrain_height(seed, world_x, world_z);
            let index = (z * width + x) as usize;
            samples[index] = quarter_meters;
            height.extend_from_slice(&quarter_meters.to_le_bytes());
            surface.push(surface_kind(seed, world_x, world_z, quarter_meters));
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
            lod2.push(surface_kind(seed, base_x + block_x as i32, base_z + block_z as i32, average));
        }
    }
    (height, surface, lod2)
}

fn terrain_height(seed: u64, x: i32, z: i32) -> i16 {
    let broad = signed_noise(seed, x.div_euclid(32), z.div_euclid(32));
    let detail = signed_noise(seed.rotate_left(17), x.div_euclid(7), z.div_euclid(7));
    (broad * 24 + detail * 4).clamp(-128, 1_024) as i16
}

fn surface_kind(seed: u64, x: i32, z: i32, height: i16) -> u8 {
    if height < -12 { 3 } else if height > 700 { 2 } else if signed_noise(seed.rotate_left(31), x, z) > 70 { 1 } else { 0 }
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
        assert_eq!(generate_chunk(42, 0, 0, 32, 32), generate_chunk(42, 0, 0, 32, 32));
    }

    #[test]
    fn invalid_scene_is_rejected() {
        let mut world = WorldManifest::default_demo();
        world.scenes[0].width_m = 511;
        assert!(matches!(world.validate(), Err(WorldgenError::InvalidSceneSize { .. })));
    }
}
