//! Conservative broad phase for ecology only. A bucket hit is never a collision;
//! callers retain the original SAT/water/plant predicates and candidate order.
use super::landcover;
use crate::worldgen::EntityInstance;
use std::collections::BTreeMap;

const CELL_M: f64 = 32.;
const MAX_ENTITY_CELLS: i64 = 256;
const MAX_QUERY_CELLS: i64 = 4096;
// Stay well inside saturating f64 -> i64 casts and bounded range arithmetic.
const MAX_CELL: f64 = 1_000_000_000.;

type Cells = (i64, i64, i64, i64);

fn cells([x0, x1, z0, z1]: [f64; 4], limit: i64) -> Option<Cells> {
    if ![x0, x1, z0, z1].iter().all(|v| v.is_finite()) || x0 > x1 || z0 > z1 {
        return None;
    }
    // Pad outward for floating-point addition at large origins and the caller's
    // patch-edge EPS. Inclusive cells deliberately keep touching candidates.
    let pad = 1e-9 + x0.abs().max(x1.abs()).max(z0.abs()).max(z1.abs()) * f64::EPSILON * 8.;
    let values = [(x0 - pad) / CELL_M, (x1 + pad) / CELL_M,
                  (z0 - pad) / CELL_M, (z1 + pad) / CELL_M];
    if values.iter().any(|v| !v.is_finite() || v.abs() > MAX_CELL) {
        return None;
    }
    let [a, b, c, d] = values.map(|v| v.floor() as i64);
    if (b - a + 1).checked_mul(d - c + 1)? > limit {
        return None;
    }
    Some((a, b, c, d))
}

fn entity_cells(e: &EntityInstance) -> Option<Cells> {
    // Same rotation and scale expressions as footprint_intersects_entity.
    let (sin, cos) = f64::from(e.rotation_y_deg).to_radians().sin_cos();
    let hw = f64::from(e.width_m) * f64::from(e.scale) * 0.5;
    let hd = f64::from(e.depth_m) * f64::from(e.scale) * 0.5;
    if ![sin, cos, hw, hd].iter().all(|v| v.is_finite()) || hw <= 0. || hd <= 0. {
        // Invalid geometry is a blocker in the narrow phase, even far away.
        return None;
    }
    let rx = hw * cos.abs() + hd * sin.abs();
    let rz = hw * sin.abs() + hd * cos.abs();
    let (x, z) = (f64::from(e.world_x), f64::from(e.world_z));
    cells([x - rx, x + rx, z - rz, z + rz], MAX_ENTITY_CELLS)
}

pub(super) struct ObstacleQuery<'a> {
    obstacles: Vec<&'a EntityInstance>,
    buckets: BTreeMap<(i64, i64), Vec<usize>>,
    spill: Vec<usize>,
    #[cfg(test)]
    linear: bool,
    #[cfg(test)]
    pub(super) candidate_visits: std::cell::Cell<usize>,
}

impl<'a> ObstacleQuery<'a> {
    pub(super) fn indexed(entities: &'a [EntityInstance]) -> Self {
        let mut result = Self::empty(entities);
        for (i, entity) in result.obstacles.iter().enumerate() {
            if let Some((x0, x1, z0, z1)) = entity_cells(entity) {
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        result.buckets.entry((x, z)).or_default().push(i);
                    }
                }
            } else {
                // Long roads/huge AABBs cannot allocate unbounded buckets.
                result.spill.push(i);
            }
        }
        result
    }

    fn empty(entities: &'a [EntityInstance]) -> Self {
        Self {
            obstacles: entities.iter().filter(|e| landcover::ecology_obstacle(e)).collect(),
            buckets: BTreeMap::new(),
            spill: vec![],
            #[cfg(test)]
            linear: false,
            #[cfg(test)]
            candidate_visits: std::cell::Cell::new(0),
        }
    }

    #[cfg(test)]
    pub(super) fn linear(entities: &'a [EntityInstance]) -> Self {
        // Genuine unindexed reference: no spatial filtering or index build.
        let mut result = Self::empty(entities);
        result.linear = true;
        result
    }

    pub(super) fn query(&self, bounds: [f64; 4]) -> Vec<&'a EntityInstance> {
        #[cfg(test)]
        if self.linear {
            self.candidate_visits.set(self.candidate_visits.get() + self.obstacles.len());
            return self.obstacles.clone();
        }
        let result = if let Some((x0, x1, z0, z1)) = cells(bounds, MAX_QUERY_CELLS) {
            let mut indices = self.spill.clone();
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if let Some(bucket) = self.buckets.get(&(x, z)) {
                        indices.extend_from_slice(bucket);
                    }
                }
            }
            // Restore input entity order and remove repeated bucket entries.
            // Never let hash/bucket traversal order change short-circuiting.
            indices.sort_unstable();
            indices.dedup();
            indices.into_iter().map(|i| self.obstacles[i]).collect()
        } else {
            self.obstacles.clone()
        };
        #[cfg(test)]
        self.candidate_visits.set(self.candidate_visits.get() + result.len());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: &str, x: i32, z: i32, width: f32, depth: f32, yaw: f32, scale: f32) -> EntityInstance {
        let mut e: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId":id,"assetId":"test","kind":"road","worldX":x,"worldZ":z,
            "worldY":4,"widthM":1.,"depthM":1.,"heightM":1.,"scale":1.
        })).unwrap();
        e.width_m = width;
        e.depth_m = depth;
        e.rotation_y_deg = yaw;
        e.scale = scale;
        e
    }

    #[test]
    fn conservative_candidates_preserve_sat_order_edges_rotation_and_spill() {
        let entities = vec![
            entity("rotated", -32, -32, 40., 0.1, 45., 1.5),
            entity("long-road", -10, 0, 100_000., 8., 17., 1.),
            entity("bucket-edge", 32, -64, 2., 2., 90., 1.),
            entity("huge-aabb", 500_000, 0, 200_000., 200_000., 0., 1.),
            entity("negative", -65, 31, 4., 12., 270., 2.),
            entity("near-not-overlap", 0, 0, 2., 2., 0., 1.),
        ];
        let indexed = ObstacleQuery::indexed(&entities);
        let reference = ObstacleQuery::linear(&entities);
        assert_eq!(indexed.spill, vec![1, 3]);
        assert!(indexed.buckets.len() < 100);
        for x in [-100., -65., -64., -32., -31., -1., 0., 1., 2., 31., 32., 33., 65., 500_000.] {
            for z in [-65., -64., -32., -1., 0., 2., 31., 32., 65.] {
                for r in [0.4, 0.75, 1.25, 2., 8., 16., 33.] {
                    let bounds = [x-r, x+r, z-r, z+r];
                    let candidates = indexed.query(bounds);
                    let ordinal: Vec<_> = candidates.iter().map(|e| {
                        entities.iter().position(|original| std::ptr::eq(original, *e)).unwrap()
                    }).collect();
                    assert!(ordinal.windows(2).all(|w| w[0] < w[1]));
                    let hits = |list: Vec<&EntityInstance>| -> Vec<String> {
                        list.into_iter().filter(|e| landcover::footprint_intersects_entity(e,x,z,r,r))
                            .map(|e| e.entity_id.clone()).collect()
                    };
                    assert_eq!(hits(candidates), hits(reference.query(bounds)), "x={x} z={z} r={r}");
                }
            }
        }
        // Touching the unit half-width rectangle is not SAT overlap; just inside is.
        let single = vec![entities[5].clone()];
        let query = ObstacleQuery::indexed(&single);
        for (x, expected) in [(2., false), (2.-1e-7, true), (2.+1e-7, false)] {
            assert_eq!(query.query([x-1.,x+1.,-1.,1.]).iter()
                .any(|e| landcover::footprint_intersects_entity(e,x,0.,1.,1.)), expected);
        }
        // Unsupported query domains conservatively visit all entries.
        assert_eq!(indexed.query([f64::NAN, 1., 0., 1.]).len(), entities.len());
        assert_eq!(indexed.query([-1e20, 1e20, -1e20, 1e20]).len(), entities.len());
    }

    #[test]
    fn invalid_obstacles_remain_global_blockers_and_large_origins_are_conservative() {
        for bad in [
            entity("nan", 1_000_000, 1_000_000, f32::NAN, 1., 0., 1.),
            entity("zero", 1_000_000, 1_000_000, 0., 1., 0., 1.),
            entity("infinite-yaw", 1_000_000, 1_000_000, 1., 1., f32::INFINITY, 1.),
            entity("negative-scale", 1_000_000, 1_000_000, 1., 1., 0., -1.),
        ] {
            let entities = [bad];
            let query = ObstacleQuery::indexed(&entities);
            assert_eq!(query.spill, vec![0]);
            assert!(query.query([-1.,1.,-1.,1.]).iter()
                .any(|e| landcover::footprint_intersects_entity(e,0.,0.,1.,1.)));
        }
        for offset in [i32::MIN + 256, -32, i32::MAX - 256] {
            let entities = [entity("large-origin", offset, offset, 30., 0.1, 45., 2.)];
            let query = ObstacleQuery::indexed(&entities);
            for delta in [-32., -16., -1e-7, 0., 1e-7, 16., 32.] {
                let (x, z) = (f64::from(offset) + delta, f64::from(offset) - delta);
                assert_eq!(query.query([x-0.4,x+0.4,z-0.4,z+0.4]).iter()
                    .any(|e| landcover::footprint_intersects_entity(e,x,z,0.4,0.4)),
                    landcover::footprint_intersects_entity(&entities[0],x,z,0.4,0.4));
            }
        }
    }
}
