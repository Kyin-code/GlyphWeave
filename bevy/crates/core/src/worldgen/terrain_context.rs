//! Query the exact encoded terrain before the payload has been materialized.
//! Reconciliation and final bake share zoning/fill bounds and quantization.
use super::{TerrainCarve, WaterGeometry, zoning::ZoningMask};
pub(super) struct TerrainContext<'a> {
    pub seed: u64,
    pub water: WaterGeometry,
    pub precise: bool,
    pub zoning: Option<&'a ZoningMask>,
}
impl TerrainContext<'_> {
    pub fn height(&self, x: i32, z: i32, carves: &[TerrainCarve]) -> f32 {
        let h = super::terrain_height_zoned(self.seed, x, z, self.water, carves, self.zoning);
        if self.precise {
            ((h * 1000.).round() as i32) as f32 / 1000.
        } else {
            (h * 4.).clamp(-128., 1024.) as i16 as f32 / 4.
        }
    }
}

/// The same surface as [`TerrainContext`], sampled from a PUBLISHED natural
/// raster instead of the procedural field.
///
/// The road programme is planned before any payload is materialized, and it is
/// planned against the natural raster it was handed (`BakedTerrain`) - the very
/// samples the independent audit re-reads afterwards. A planning gate therefore
/// has no seed to regenerate a field from, and must not invent one: it composes
/// the carve on top of the raster sample here, with exactly the composition
/// [`TerrainContext::height`] performs - protected cells and water keep their
/// natural surface, fill is clamped to the (lesser) of the zoning fill capacity
/// and the carve's own limit, and a carve can never excavate.
pub(super) struct RasterSurface<'a> {
    pub natural: &'a super::BakedTerrain,
    pub water: WaterGeometry,
    pub zoning: Option<&'a ZoningMask>,
}
impl RasterSurface<'_> {
    pub fn natural_at(&self, x: i32, z: i32) -> Option<f32> {
        self.natural.height(x, z)
    }
    pub fn height(&self, x: i32, z: i32, carves: &[TerrainCarve]) -> Option<f32> {
        let natural = self.natural.height(x, z)?;
        if carves.is_empty()
            || self.water.forbids_earthwork(f64::from(x), f64::from(z))
            || self.zoning.is_some_and(|mask| mask.protected_at(x, z))
        {
            return Some(natural);
        }
        let engineered = super::apply_terrain_carves(f64::from(natural), x, z, carves);
        if engineered <= f64::from(natural) {
            return Some(natural);
        }
        Some(match self.zoning {
            Some(mask) => engineered.min(f64::from(natural) + mask.fill_capacity_at(x, z)) as f32,
            None => engineered as f32,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        height_payload::HeightPayload,
        worldgen::{
            EntityInstance, GroundingSpec, LandUseZone, SceneSpec, WaterKind, water_geometry,
        },
    };
    #[test]
    fn preflight_encoded_query_matches_bake_and_protects_natural_boundary() {
        let scene = SceneSpec {
            scene_id: "context".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![
            LandUseZone {
                id: "build".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "preserve".into(),
                kind: "nature".into(),
                world_x: 256,
                world_z: 0,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
        ];
        let mask = ZoningMask::new(&scene, &zones).unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let e = EntityInstance {
            entity_id: "fixture.pad".into(),
            asset_id: "prop.building".into(),
            kind: "building".into(),
            world_x: 240,
            world_z: 110,
            world_y: 150,
            width_m: 24.,
            depth_m: 24.,
            height_m: 8.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        };
        let carves = super::super::plan_terrain_carves(&[e]);
        for precise in [false, true] {
            let context = TerrainContext {
                seed: 42,
                water,
                precise,
                zoning: Some(&mask),
            };
            let (height, _, _, semantics) = super::super::generate_chunk_encoded(
                42,
                0,
                100,
                512,
                4,
                water,
                &carves,
                precise,
                Some(&mask),
            );
            let p = HeightPayload::parse(&height, 512, 4).unwrap();
            for row in 0..4 {
                for x in 0..512 {
                    let h = context.height(x, 100 + row, &carves);
                    assert!((f64::from(h) - p.metres((row * 512 + x) as usize)).abs() < 0.00003);
                    let natural = context.height(x, 100 + row, &[]);
                    if x >= 256 {
                        assert_eq!(h, natural);
                        assert_eq!(semantics[((row * 512 + x) * 4 + 3) as usize], 0);
                    } else {
                        assert!(
                            f64::from(h - natural) <= mask.fill_capacity_at(x, 100 + row) + 0.251
                        );
                    }
                }
            }
        }
    }
}
