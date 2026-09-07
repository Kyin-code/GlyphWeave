//! Exact broad-phase acceleration for pedestrian triangles used by agriculture.
//! The original triangle/cell clipping predicate is unchanged. Buckets only
//! exclude provably disjoint triangles; oversized shapes use a global fallback.
use crate::worldgen::AccessSurface;
use std::collections::BTreeMap;
const BUCKET_M: i64 = 16;
const MAX_BUCKETS_PER_TRIANGLE: u64 = 4096;
type Triangle = [[f64; 3]; 3];
pub(super) struct PedestrianIndex {
    triangles: Vec<Triangle>,
    buckets: BTreeMap<(i64, i64), Vec<usize>>,
    global: Vec<usize>,
}
impl PedestrianIndex {
    pub(super) fn new(surfaces: &[AccessSurface]) -> Result<Self, String> {
        let mut triangles = vec![];
        for surface in surfaces {
            for indices in &surface.triangles {
                let mut t = [[0.; 3]; 3];
                for (v, &i) in t.iter_mut().zip(indices) {
                    *v = *surface
                        .vertices
                        .get(i as usize)
                        .ok_or("pedestrian triangle index outside vertices")?;
                    if v.iter().any(|x| !x.is_finite())
                        || [v[0], v[2]]
                            .iter()
                            .any(|x| *x < f64::from(i32::MIN) || *x > f64::from(i32::MAX))
                    {
                        return Err(
                            "pedestrian triangle coordinates outside finite world domain".into(),
                        );
                    }
                }
                triangles.push(t);
            }
        }
        let mut result = Self {
            triangles,
            buckets: BTreeMap::new(),
            global: vec![],
        };
        for (i, t) in result.triangles.iter().enumerate() {
            let min_x = t.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min).floor() as i64;
            let max_x = t
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max)
                .floor() as i64;
            let min_z = t.iter().map(|p| p[2]).fold(f64::INFINITY, f64::min).floor() as i64;
            let max_z = t
                .iter()
                .map(|p| p[2])
                .fold(f64::NEG_INFINITY, f64::max)
                .floor() as i64;
            let (x0, x1, z0, z1) = (
                min_x.div_euclid(BUCKET_M),
                max_x.div_euclid(BUCKET_M),
                min_z.div_euclid(BUCKET_M),
                max_z.div_euclid(BUCKET_M),
            );
            let count = (x1 - x0 + 1) as u64 * (z1 - z0 + 1) as u64;
            if count > MAX_BUCKETS_PER_TRIANGLE {
                result.global.push(i);
                continue;
            }
            for z in z0..=z1 {
                for x in x0..=x1 {
                    result.buckets.entry((x, z)).or_default().push(i);
                }
            }
        }
        Ok(result)
    }
    fn candidates(&self, x: i64, z: i64) -> impl Iterator<Item = usize> + '_ {
        self.buckets
            .get(&(x.div_euclid(BUCKET_M), z.div_euclid(BUCKET_M)))
            .into_iter()
            .flatten()
            .copied()
            .chain(self.global.iter().copied())
    }
    pub(super) fn overlaps_cell(&self, x: i64, z: i64) -> bool {
        self.candidates(x, z)
            .any(|i| triangle_overlaps_cell(self.triangles[i], x, z))
    }
    #[cfg(test)]
    pub(super) fn linear_reference(&self, x: i64, z: i64) -> bool {
        self.triangles
            .iter()
            .any(|t| triangle_overlaps_cell(*t, x, z))
    }
}
// Preserved verbatim in substance from agriculture.rs. Keep the positive-area
// threshold, winding normalization and polygon clipping as the narrow phase.
pub(super) fn triangle_overlaps_cell(t: Triangle, x: i64, z: i64) -> bool {
    let p = t.map(|p| [p[0], p[2]]);
    let [a, b, c] = p;
    let minx = a[0].min(b[0]).min(c[0]);
    let maxx = a[0].max(b[0]).max(c[0]);
    let minz = a[1].min(b[1]).min(c[1]);
    let maxz = a[1].max(b[1]).max(c[1]);
    if minx >= x as f64 + 1. || maxx <= x as f64 || minz >= z as f64 + 1. || maxz <= z as f64 {
        return false;
    }
    let mut p = p;
    if super::landcover::signed_area(&p) < 0. {
        p.swap(1, 2);
    }
    let square = vec![
        [x as f64, z as f64],
        [x as f64 + 1., z as f64],
        [x as f64 + 1., z as f64 + 1.],
        [x as f64, z as f64 + 1.],
    ];
    let (_, inside) = super::landcover::subtract_triangle(square, p);
    super::landcover::signed_area(&inside).abs() > 1e-8
}
#[cfg(test)]
mod tests {
    use super::*;
    fn surface(triangles: Vec<Triangle>) -> AccessSurface {
        let mut surface = AccessSurface {
            version: 1,
            entity_id: "test".into(),
            anchor_id: "front".into(),
            road_id: "road".into(),
            width_m: 1.5,
            vertices: vec![],
            triangles: vec![],
        };
        for t in triangles {
            let offset = surface.vertices.len() as u32;
            surface.vertices.extend(t);
            surface.triangles.push([offset, offset + 1, offset + 2]);
        }
        surface
    }
    fn verify(s: &[AccessSurface], min: i64, max: i64) {
        let index = PedestrianIndex::new(s).unwrap();
        for z in min..=max {
            for x in min..=max {
                assert_eq!(
                    index.overlaps_cell(x, z),
                    index.linear_reference(x, z),
                    "({x},{z})"
                );
            }
        }
    }
    #[test]
    fn bucket_boundaries_winding_slivers_and_degenerate_triangles_match_reference() {
        let triangles = vec![
            [[-16., 0., -16.], [16., 0., 16.], [-16., 0., 16.]],
            [[16., 1., 16.], [16.000001, 1., 16.], [16., 1., 17.]],
            [[0., 0., 0.], [0., 0., 0.], [0., 0., 1.]],
            [[31., 2., 1.], [32., 2., 0.], [32., 2., 1.]],
        ];
        let mut reversed = triangles.clone();
        for t in &mut reversed {
            t.swap(0, 2);
        }
        verify(&[surface(triangles), surface(reversed)], -34, 34);
        verify(&[], -17, 17);
    }
    #[test]
    fn randomized_negative_translated_and_permuted_inputs_match() {
        let mut seed = 17_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) % 16000) as f64 / 100. - 80.
        };
        let mut triangles = vec![];
        for _ in 0..80 {
            let x = next();
            let z = next();
            triangles.push([
                [x, 0., z],
                [x + next() / 5., 0., z + next() / 5.],
                [x + next() / 5., 0., z + next() / 5.],
            ]);
        }
        verify(&[surface(triangles.clone())], -90, 90);
        triangles.reverse();
        verify(&[surface(triangles.clone())], -90, 90);
        for t in &mut triangles {
            for p in t {
                p[0] -= 100000.;
                p[2] -= 100000.;
            }
        }
        verify(&[surface(triangles)], -100090, -99910);
    }
    #[test]
    fn oversized_global_fallback_and_invalid_data_are_bounded() {
        let huge = surface(vec![[
            [-1_000_000., 0., -1_000_000.],
            [1_000_000., 0., -1_000_000.],
            [0., 0., 1_000_000.],
        ]]);
        let index = PedestrianIndex::new(&[huge]).unwrap();
        assert!(index.buckets.is_empty());
        assert_eq!(index.global.len(), 1);
        for (x, z) in [(-10, 0), (0, 0), (1000001, 0)] {
            assert_eq!(index.overlaps_cell(x, z), index.linear_reference(x, z));
        }
        let mut bad = surface(vec![[[0.; 3]; 3]]);
        bad.triangles[0][1] = 30;
        assert!(PedestrianIndex::new(&[bad.clone()]).is_err());
        bad.triangles[0][1] = 0;
        bad.vertices[0][0] = f64::NAN;
        assert!(PedestrianIndex::new(&[bad]).is_err());
    }
    #[test]
    fn locality_reduces_exact_tests_without_removing_geometry() {
        let triangles = (0..10000)
            .map(|i| {
                let x = f64::from(i % 100) * 32.;
                let z = f64::from(i / 100) * 32.;
                [[x, 0., z], [x + 1., 0., z], [x, 0., z + 1.]]
            })
            .collect();
        let index = PedestrianIndex::new(&[surface(triangles)]).unwrap();
        assert_eq!(index.triangles.len(), 10000);
        assert!(index.candidates(0, 0).count() <= 1);
        assert_eq!(index.overlaps_cell(0, 0), index.linear_reference(0, 0));
        assert_eq!(index.candidates(20, 20).count(), 0);
        assert_eq!(index.overlaps_cell(20, 20), index.linear_reference(20, 20));
    }
}
