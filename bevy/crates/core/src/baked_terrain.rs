//! Compact authoritative metre raster shared by baking and offline audits.
//!
//! A raster stores one uniform encoding: legacy i16 quarter metres (120 MB)
//! or i32 millimetres (240 MB at 6x10km), never mixed units across chunks.
//! A small chunk-coverage table distinguishes missing data from real zero height.

use crate::height_payload::{HeightEncoding, HeightPayload};
use crate::worldgen::{STREAM_CHUNK_METERS, SceneSpec};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightComparison {
    pub cells: usize,
    pub unchanged_cells: usize,
    pub changed_cells: usize,
    pub raised_cells: usize,
    pub lowered_cells: usize,
    pub max_rise_mm: i32,
    pub max_lower_mm: i32,
    pub mean_absolute_change_m: f64,
}

#[derive(Debug, Error)]
#[error("invalid baked terrain: {0}")]
pub struct TerrainError(pub String);

pub struct BakedTerrain {
    origin_x: i32,
    origin_z: i32,
    width: u32,
    depth: u32,
    samples: Vec<i16>,
    millimetres: Option<Vec<i32>>,
    encoding: Option<HeightEncoding>,
    loaded: Vec<bool>,
}

impl BakedTerrain {
    pub fn new(scene: &SceneSpec) -> Result<Self, TerrainError> {
        if scene.width_m == 0
            || scene.depth_m == 0
            || scene.width_m > 6000
            || scene.depth_m > 10000
            || scene.origin_x.checked_add(scene.width_m as i32).is_none()
            || scene.origin_z.checked_add(scene.depth_m as i32).is_none()
        {
            return Err(TerrainError("invalid scene dimensions/origin".into()));
        }
        let count = scene.width_m as usize * scene.depth_m as usize;
        let chunks = scene.width_m.div_ceil(STREAM_CHUNK_METERS)
            * scene.depth_m.div_ceil(STREAM_CHUNK_METERS);
        Ok(Self {
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            width: scene.width_m,
            depth: scene.depth_m,
            samples: vec![0; count],
            millimetres: None,
            encoding: None,
            loaded: vec![false; chunks as usize],
        })
    }

    pub fn encoding(&self) -> Option<HeightEncoding> {
        self.encoding
    }

    pub fn matches_scene(&self, scene: &SceneSpec) -> bool {
        (self.origin_x, self.origin_z, self.width, self.depth)
            == (scene.origin_x, scene.origin_z, scene.width_m, scene.depth_m)
    }

    /// Raster origin `(x, z)`, for audits that have to walk the whole scene.
    pub fn scene_origin(&self) -> (i32, i32) {
        (self.origin_x, self.origin_z)
    }

    /// Raster size `(width, depth)` in metres.
    pub fn scene_dimensions(&self) -> (u32, u32) {
        (self.width, self.depth)
    }

    /// Validate an entire chunk before writing anything. Reject overlaps, gaps,
    /// truncated payloads and dimensions inconsistent with the scene grid.
    pub fn insert_chunk(
        &mut self,
        x: i32,
        z: i32,
        width: u32,
        depth: u32,
        bytes: &[u8],
    ) -> Result<(), TerrainError> {
        let dx = i64::from(x) - i64::from(self.origin_x);
        let dz = i64::from(z) - i64::from(self.origin_z);
        let size = i64::from(STREAM_CHUNK_METERS);
        if dx < 0
            || dz < 0
            || dx >= i64::from(self.width)
            || dz >= i64::from(self.depth)
            || dx % size != 0
            || dz % size != 0
        {
            return Err(TerrainError(format!(
                "misaligned/out-of-bounds chunk ({x},{z})"
            )));
        }
        let dx = dx as u32;
        let dz = dz as u32;
        if width != (self.width - dx).min(STREAM_CHUNK_METERS)
            || depth != (self.depth - dz).min(STREAM_CHUNK_METERS)
        {
            return Err(TerrainError(format!(
                "chunk ({x},{z}) dimensions/payload mismatch"
            )));
        }
        let chunk_index = (dz / STREAM_CHUNK_METERS * self.width.div_ceil(STREAM_CHUNK_METERS)
            + dx / STREAM_CHUNK_METERS) as usize;
        if self.loaded[chunk_index] {
            return Err(TerrainError(format!("duplicate chunk ({x},{z})")));
        }
        let payload = HeightPayload::parse(bytes, width, depth)?;
        if self.encoding.is_some_and(|e| e != payload.encoding) {
            return Err(TerrainError("mixed height encodings within raster".into()));
        }
        if payload.encoding == HeightEncoding::Millimetre && self.millimetres.is_none() {
            self.samples = Vec::new();
            self.millimetres = Some(vec![0; self.width as usize * self.depth as usize]);
        }
        self.encoding = Some(payload.encoding);
        for row in 0..depth as usize {
            let offset = (dz as usize + row) * self.width as usize + dx as usize;
            for col in 0..width as usize {
                let value = payload.raw(row * width as usize + col);
                if let Some(samples) = &mut self.millimetres {
                    samples[offset + col] = value
                } else {
                    self.samples[offset + col] = value as i16;
                }
            }
        }
        self.loaded[chunk_index] = true;
        Ok(())
    }

    pub fn require_complete(&self) -> Result<(), TerrainError> {
        if self.loaded.iter().all(|loaded| *loaded) {
            Ok(())
        } else {
            Err(TerrainError(
                "missing heightfield chunks; refusing to invent terrain".into(),
            ))
        }
    }

    /// Content identity includes every sample, dimensions, origin and units.
    /// Never hashes an incomplete raster as if missing cells were zero.
    pub fn content_hash(&self) -> Result<String, TerrainError> {
        self.require_complete()?;
        let mut hash = blake3::Hasher::new();
        hash.update(if self.millimetres.is_some() {
            b"glyphweave-heightfield-i32le-millimetre-v1".as_slice()
        } else {
            b"glyphweave-heightfield-i16le-quarter-metre-v1".as_slice()
        });
        hash.update(&self.origin_x.to_le_bytes());
        hash.update(&self.origin_z.to_le_bytes());
        hash.update(&self.width.to_le_bytes());
        hash.update(&self.depth.to_le_bytes());
        if let Some(samples) = &self.millimetres {
            let mut buffer = [0u8; 8192];
            for chunk in samples.chunks(2048) {
                for (i, s) in chunk.iter().enumerate() {
                    buffer[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
                }
                hash.update(&buffer[..chunk.len() * 4]);
            }
            return Ok(hash.finalize().to_hex().to_string());
        }
        let mut buffer = [0u8; 8192];
        for chunk in self.samples.chunks(4096) {
            for (i, sample) in chunk.iter().enumerate() {
                buffer[2 * i..2 * i + 2].copy_from_slice(&sample.to_le_bytes());
            }
            hash.update(&buffer[..chunk.len() * 2]);
        }
        Ok(hash.finalize().to_hex().to_string())
    }

    /// Strict lookup: a missing chunk or an outside point is not sea level.
    pub fn height(&self, x: i32, z: i32) -> Option<f32> {
        let dx = i64::from(x) - i64::from(self.origin_x);
        let dz = i64::from(z) - i64::from(self.origin_z);
        if dx < 0 || dz < 0 || dx >= i64::from(self.width) || dz >= i64::from(self.depth) {
            return None;
        }
        let (dx, dz) = (dx as u32, dz as u32);
        let chunk = dz / STREAM_CHUNK_METERS * self.width.div_ceil(STREAM_CHUNK_METERS)
            + dx / STREAM_CHUNK_METERS;
        self.loaded[chunk as usize].then(|| {
            let i = (dz * self.width + dx) as usize;
            self.millimetres
                .as_ref()
                .map_or_else(|| f32::from(self.samples[i]) / 4., |s| s[i] as f32 / 1000.)
        })
    }

    /// Exact storage-unit lookup for engineering solvers; avoids f32 round-off
    /// when constructing conservative millimetre lower and upper bounds.
    pub fn height_millimetres(&self, x: i32, z: i32) -> Option<i32> {
        let dx = i64::from(x) - i64::from(self.origin_x);
        let dz = i64::from(z) - i64::from(self.origin_z);
        if dx < 0 || dz < 0 || dx >= i64::from(self.width) || dz >= i64::from(self.depth) {
            return None;
        }
        let (dx, dz) = (dx as u32, dz as u32);
        let chunk = dz / STREAM_CHUNK_METERS * self.width.div_ceil(STREAM_CHUNK_METERS)
            + dx / STREAM_CHUNK_METERS;
        self.loaded[chunk as usize].then(|| {
            let i = (dz * self.width + dx) as usize;
            self.millimetres
                .as_ref()
                .map_or_else(|| i32::from(self.samples[i]) * 250, |s| s[i])
        })
    }

    /// Exact per-cell comparison between two complete rasters with the same
    /// scene identity. Values are converted to millimetres before comparison,
    /// so quarter-metre and millimetre rasters remain comparable.
    pub fn compare_heightfields(&self, other: &Self) -> Result<HeightComparison, TerrainError> {
        self.require_complete()?;
        other.require_complete()?;
        if (self.origin_x, self.origin_z, self.width, self.depth)
            != (other.origin_x, other.origin_z, other.width, other.depth)
        {
            return Err(TerrainError(
                "cannot compare heightfields with different scene identity".into(),
            ));
        }
        let mut changed_cells = 0usize;
        let mut raised_cells = 0usize;
        let mut lowered_cells = 0usize;
        let mut max_rise_mm = 0i32;
        let mut max_lower_mm = 0i32;
        let mut total_absolute_change_mm = 0i128;
        for row in 0..self.depth as i32 {
            for col in 0..self.width as i32 {
                let x = self.origin_x + col;
                let z = self.origin_z + row;
                let from = self
                    .height_millimetres(x, z)
                    .ok_or_else(|| TerrainError("missing source height sample".into()))?;
                let to = other
                    .height_millimetres(x, z)
                    .ok_or_else(|| TerrainError("missing target height sample".into()))?;
                let delta = to - from;
                if delta != 0 {
                    changed_cells += 1;
                    total_absolute_change_mm += i128::from(delta.unsigned_abs());
                    if delta > 0 {
                        raised_cells += 1;
                        max_rise_mm = max_rise_mm.max(delta);
                    } else {
                        lowered_cells += 1;
                        max_lower_mm = max_lower_mm.max(-delta);
                    }
                }
            }
        }
        let cells = self.width as usize * self.depth as usize;
        Ok(HeightComparison {
            cells,
            unchanged_cells: cells - changed_cells,
            changed_cells,
            raised_cells,
            lowered_cells,
            max_rise_mm,
            max_lower_mm,
            mean_absolute_change_m: total_absolute_change_mm as f64 / cells.max(1) as f64 / 1000.,
        })
    }

    /// Edge extension for slope/clearance sampling ONLY. Entity bounds are
    /// validated independently. Never fills holes inside the scene.
    pub fn height_clamped(&self, x: i32, z: i32) -> Option<f32> {
        self.height(
            x.clamp(self.origin_x, self.origin_x + self.width as i32 - 1),
            z.clamp(self.origin_z, self.origin_z + self.depth as i32 - 1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "terrain".into(),
            width_m: 513,
            depth_m: 2,
            origin_x: -100,
            origin_z: 50,
            seed_offset: 0,
        }
    }
    fn payload(width: usize, height: i16) -> Vec<u8> {
        height.to_le_bytes().repeat(width * 2)
    }
    #[test]
    fn millimetre_raster_keeps_small_gradients_and_rejects_mixed_units() {
        let spec = scene();
        let mut raster = BakedTerrain::new(&spec).unwrap();
        let mm = crate::height_payload::encode_millimetres(
            512,
            2,
            (0..2).flat_map(|_| (0..512).map(|x| -1. + x as f64 * 0.04)),
        )
        .unwrap();
        raster.insert_chunk(-100, 50, 512, 2, &mm).unwrap();
        assert!((raster.height(-99, 50).unwrap() + 0.96).abs() < 1e-6);
        assert!(raster.insert_chunk(412, 50, 1, 2, &payload(1, 0)).is_err());
        assert!(raster.height(412, 50).is_none());
        let tail = crate::height_payload::encode_millimetres(1, 2, [19.48, 19.48]).unwrap();
        raster.insert_chunk(412, 50, 1, 2, &tail).unwrap();
        raster.require_complete().unwrap();
        assert!(
            (raster.height(412, 50).unwrap() - raster.height(411, 50).unwrap() - 0.04).abs() < 1e-5
        );
        let hash = raster.content_hash().unwrap();
        assert!(!hash.is_empty());
        let mut a = BakedTerrain::new(&spec).unwrap();
        let mut corrupt = mm.clone();
        corrupt[8] = 1;
        assert!(a.insert_chunk(-100, 50, 512, 2, &corrupt).is_err());
        assert!(a.encoding().is_none());
        assert!(a.height(-100, 50).is_none());
    }

    #[test]
    fn negative_origin_partial_chunk_and_units() {
        let mut terrain = BakedTerrain::new(&scene()).unwrap();
        assert_eq!(terrain.height(-100, 50), None);
        terrain
            .insert_chunk(-100, 50, 512, 2, &payload(512, -3))
            .unwrap();
        assert!(terrain.require_complete().is_err());
        assert_eq!(terrain.height(411, 51), Some(-0.75));
        assert_eq!(terrain.height(412, 50), None);
        terrain.insert_chunk(412, 50, 1, 2, &payload(1, 9)).unwrap();
        terrain.require_complete().unwrap();
        assert_eq!(terrain.height(412, 51), Some(2.25));
        assert_eq!(terrain.height(413, 51), None);
        assert_eq!(terrain.height_clamped(413, 52), Some(2.25));
    }
    #[test]
    fn malformed_insert_is_atomic_and_duplicate_rejected() {
        let mut terrain = BakedTerrain::new(&scene()).unwrap();
        assert!(terrain.insert_chunk(-100, 50, 512, 2, &[1, 2]).is_err());
        assert_eq!(terrain.height(-100, 50), None);
        assert!(
            terrain
                .insert_chunk(-99, 50, 512, 2, &payload(512, 1))
                .is_err()
        );
        terrain
            .insert_chunk(-100, 50, 512, 2, &payload(512, 4))
            .unwrap();
        assert!(
            terrain
                .insert_chunk(-100, 50, 512, 2, &payload(512, 8))
                .is_err()
        );
        assert_eq!(terrain.height(-100, 50), Some(1.0));
    }
    #[test]
    fn rejects_invalid_dimensions_and_overflow_before_allocating() {
        for (width, depth, x) in [(6001, 2, 0), (1, 10001, 0), (0, 2, 0), (2, 2, i32::MAX)] {
            let mut s = scene();
            s.width_m = width;
            s.depth_m = depth;
            s.origin_x = x;
            assert!(BakedTerrain::new(&s).is_err());
        }
    }

    #[test]
    fn heightfield_comparison_is_exact_and_scene_bound() {
        let spec = SceneSpec {
            scene_id: "compare".into(),
            width_m: 2,
            depth_m: 2,
            origin_x: 10,
            origin_z: -4,
            seed_offset: 0,
        };
        let mut natural = BakedTerrain::new(&spec).unwrap();
        let mut final_terrain = BakedTerrain::new(&spec).unwrap();
        natural
            .insert_chunk(
                10,
                -4,
                2,
                2,
                &crate::height_payload::encode_millimetres(2, 2, [0., 0., 0., 0.]).unwrap(),
            )
            .unwrap();
        final_terrain
            .insert_chunk(
                10,
                -4,
                2,
                2,
                &crate::height_payload::encode_millimetres(2, 2, [0., 1.5, -0.5, 0.]).unwrap(),
            )
            .unwrap();
        let comparison = natural.compare_heightfields(&final_terrain).unwrap();
        assert_eq!(comparison.cells, 4);
        assert_eq!(comparison.changed_cells, 2);
        assert_eq!(comparison.raised_cells, 1);
        assert_eq!(comparison.lowered_cells, 1);
        assert_eq!(comparison.max_rise_mm, 1500);
        assert_eq!(comparison.max_lower_mm, 500);
        assert!((comparison.mean_absolute_change_m - 0.5).abs() < 1e-9);

        let mut other_scene = spec.clone();
        other_scene.origin_x += 1;
        let other = BakedTerrain::new(&other_scene).unwrap();
        assert!(natural.compare_heightfields(&other).is_err());
    }
}
