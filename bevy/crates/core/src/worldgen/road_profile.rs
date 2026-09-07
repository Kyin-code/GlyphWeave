//! Authored road longitudinal geometry shared by planning, earthwork and audit.
use super::EntityInstance;
use crate::baked_terrain::TerrainError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoadProfile {
    pub version: u32,
    pub start: [i32; 2],
    pub end: [i32; 2],
    pub elevations_mm: Vec<i64>,
    pub max_fill_mm: u32,
}
impl RoadProfile {
    pub fn validate_shape(&self) -> Result<(), TerrainError> {
        let dx = i64::from(self.end[0]) - i64::from(self.start[0]);
        let dz = i64::from(self.end[1]) - i64::from(self.start[1]);
        let length = dx.abs() + dz.abs();
        if self.version != 1
            || (dx == 0) == (dz == 0)
            || !(16..=10000).contains(&length)
            || length % 2 != 0
            || self.elevations_mm.len() != length as usize + 1
            || self.max_fill_mm > 6000
            || self
                .elevations_mm
                .iter()
                .any(|h| !(-32000..=256000).contains(h))
            || self
                .elevations_mm
                .windows(2)
                .any(|p| (p[0] - p[1]).abs() > 80)
        {
            return Err(TerrainError("invalid road profile samples/domain".into()));
        }
        Ok(())
    }
    pub fn height_at(&self, x: f64, z: f64) -> Option<f64> {
        let dx = i64::from(self.end[0]) - i64::from(self.start[0]);
        let dz = i64::from(self.end[1]) - i64::from(self.start[1]);
        let n = self.elevations_mm.len().checked_sub(1)?;
        if n == 0 || !x.is_finite() || !z.is_finite() {
            return None;
        }
        let t = if dx != 0 {
            (x - f64::from(self.start[0])) * dx.signum() as f64
        } else {
            (z - f64::from(self.start[1])) * dz.signum() as f64
        };
        let t = t.clamp(0., n as f64);
        let i = t.floor() as usize;
        let j = (i + 1).min(n);
        Some(
            (self.elevations_mm[i] as f64
                + (self.elevations_mm[j] as f64 - self.elevations_mm[i] as f64) * (t - i as f64))
                / 1000.,
        )
    }
    pub fn validate_entity(&self, e: &EntityInstance) -> Result<(), TerrainError> {
        self.validate_shape()?;
        let dx = i64::from(self.end[0]) - i64::from(self.start[0]);
        let dz = i64::from(self.end[1]) - i64::from(self.start[1]);
        let length = dx.abs() + dz.abs();
        if self.version != 1
            || e.kind != "road"
            || (dx == 0) == (dz == 0)
            || length < 16
            || length > 10000
            || length % 2 != 0
            || self.elevations_mm.len() != length as usize + 1
            || self.max_fill_mm > 6000
            || self
                .elevations_mm
                .iter()
                .any(|h| !(-32000..=256000).contains(h))
            || self
                .elevations_mm
                .windows(2)
                .any(|p| (p[0] - p[1]).abs() > 80)
            || e.width_m != length as f32
            || e.depth_m != 8.
            || e.scale != 1.
            || e.rotation_y_deg != if dx == 0 { 90. } else { 0. }
            || i64::from(e.world_x) * 2 != i64::from(self.start[0]) + i64::from(self.end[0])
            || i64::from(e.world_z) * 2 != i64::from(self.start[1]) + i64::from(self.end[1])
            || (f64::from(e.ground_height_m())
                - self
                    .height_at(f64::from(e.world_x), f64::from(e.world_z))
                    .unwrap_or(f64::INFINITY))
            .abs()
                > 0.00005
        {
            return Err(TerrainError(format!(
                "invalid road longitudinal contract {}",
                e.entity_id
            )));
        }
        Ok(())
    }
}
