//! Exact Chebyshev distance to protected integer raster rectangles.
//! Original GlyphWeave implementation; this indexes the existing metric,
//! without approximating it or introducing a per-pixel distance raster.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub x: i64,
    pub z: i64,
    pub right: i64,
    pub bottom: i64,
}
impl Bounds {
    fn distance(self, x: i64, z: i64) -> i64 {
        (self.x - x)
            .max(x - self.right)
            .max(0)
            .max((self.z - z).max(z - self.bottom).max(0))
    }
    fn union(self, b: Self) -> Self {
        Self {
            x: self.x.min(b.x),
            z: self.z.min(b.z),
            right: self.right.max(b.right),
            bottom: self.bottom.max(b.bottom),
        }
    }
}
#[derive(Debug)]
struct Node {
    bounds: Bounds,
    content: Content,
}
#[derive(Debug)]
enum Content {
    Leaf { start: usize, end: usize },
    Branch { left: usize, right: usize },
}
#[derive(Debug, Default)]
pub(super) struct DistanceIndex {
    rectangles: Vec<Bounds>,
    nodes: Vec<Node>,
}
impl DistanceIndex {
    pub fn new(rectangles: Vec<Bounds>) -> Self {
        let mut index = Self {
            rectangles,
            nodes: vec![],
        };
        if !index.rectangles.is_empty() {
            index.build(0, index.rectangles.len());
        }
        index
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let bounds = self.rectangles[start + 1..end]
            .iter()
            .fold(self.rectangles[start], |a, &b| a.union(b));
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            content: Content::Leaf { start, end },
        });
        if end - start > 4 {
            let x_axis = bounds.right - bounds.x >= bounds.bottom - bounds.z;
            let half = (end - start) / 2;
            self.rectangles[start..end].select_nth_unstable_by_key(half, |b| {
                if x_axis {
                    (b.x + b.right, b.z + b.bottom, b.x, b.z)
                } else {
                    (b.z + b.bottom, b.x + b.right, b.z, b.x)
                }
            });
            let left = self.build(start, start + half);
            let right = self.build(start + half, end);
            self.nodes[node].content = Content::Branch { left, right };
        }
        node
    }
    pub fn distance(&self, x: i64, z: i64) -> Option<i64> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut best = i64::MAX;
        self.visit::<false>(0, x, z, &mut best, &mut 0);
        Some(best)
    }
    // A containing node's distance is a lower bound for every child rectangle.
    // Near-first traversal improves pruning without altering tie semantics.
    fn visit<const COUNT: bool>(
        &self,
        node: usize,
        x: i64,
        z: i64,
        best: &mut i64,
        checks: &mut usize,
    ) {
        let n = &self.nodes[node];
        if n.bounds.distance(x, z) >= *best {
            return;
        }
        match n.content {
            Content::Leaf { start, end } => {
                for b in &self.rectangles[start..end] {
                    if COUNT {
                        *checks += 1;
                    }
                    *best = (*best).min(b.distance(x, z));
                    if *best == 0 {
                        return;
                    }
                }
            }
            Content::Branch { left, right } => {
                let (first, second) = if self.nodes[left].bounds.distance(x, z)
                    <= self.nodes[right].bounds.distance(x, z)
                {
                    (left, right)
                } else {
                    (right, left)
                };
                self.visit::<COUNT>(first, x, z, best, checks);
                self.visit::<COUNT>(second, x, z, best, checks);
            }
        }
    }
    #[cfg(test)]
    fn measured_distance(&self, x: i64, z: i64) -> (Option<i64>, usize) {
        if self.nodes.is_empty() {
            return (None, 0);
        }
        let mut best = i64::MAX;
        let mut checks = 0;
        self.visit::<true>(0, x, z, &mut best, &mut checks);
        (Some(best), checks)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn reference(rects: &[Bounds], x: i64, z: i64) -> Option<i64> {
        rects
            .iter()
            .map(|q| {
                let dx = (q.x - x).max(x - q.right).max(0);
                let dz = (q.z - z).max(z - q.bottom).max(0);
                dx.max(dz)
            })
            .min()
    }
    #[test]
    fn empty_single_cell_edges_and_extreme_coordinates_are_exact() {
        let empty = DistanceIndex::new(vec![]);
        assert_eq!(empty.distance(0, 0), None);
        let r = Bounds {
            x: -10,
            z: 20,
            right: -10,
            bottom: 20,
        };
        let index = DistanceIndex::new(vec![r]);
        for [x, z] in [
            [-10, 20],
            [-11, 20],
            [-10, 21],
            [0, 0],
            [i32::MIN as i64, i32::MAX as i64],
        ] {
            assert_eq!(index.distance(x, z), reference(&[r], x, z));
        }
        assert_eq!(index.distance(-10, 20), Some(0));
        assert_eq!(index.distance(-11, 21), Some(1));
    }
    #[test]
    fn randomized_rectangles_and_shuffled_input_match_full_scan() {
        let mut seed = 4192u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed >> 32
        };
        for count in [1, 4, 5, 16, 127, 1024] {
            let rects: Vec<_> = (0..count)
                .map(|_| {
                    let x = (rnd() % 8000) as i64 - 4000;
                    let z = (rnd() % 12000) as i64 - 6000;
                    Bounds {
                        x,
                        z,
                        right: x + (rnd() % 500) as i64,
                        bottom: z + (rnd() % 500) as i64,
                    }
                })
                .collect();
            let index = DistanceIndex::new(rects.clone());
            let mut reversed = rects.clone();
            reversed.reverse();
            let reversed = DistanceIndex::new(reversed);
            for _ in 0..2048 {
                let x = (rnd() % 12000) as i64 - 6000;
                let z = (rnd() % 16000) as i64 - 8000;
                let expected = reference(&rects, x, z);
                assert_eq!(index.distance(x, z), expected);
                assert_eq!(reversed.distance(x, z), expected);
            }
            assert!(index.nodes.len() < 2 * count);
        }
    }
    fn large_rectangles() -> Vec<Bounds> {
        let mut rects = vec![];
        for z in (0..10000).step_by(128) {
            for x in (0..6000).step_by(128) {
                if (x / 128 + z / 128) % 3 == 0 {
                    rects.push(Bounds {
                        x,
                        z,
                        right: (x + 128).min(6000) - 1,
                        bottom: (z + 128).min(10000) - 1,
                    });
                }
            }
        }
        rects
    }
    #[test]
    fn maximum_domain_partial_edges_keep_exact_answers_with_fewer_leaf_checks() {
        let rects = large_rectangles();
        let index = DistanceIndex::new(rects.clone());
        let mut checks = 0;
        let mut queries = 0;
        for z in (0..10000).step_by(83) {
            for x in (0..6000).step_by(79) {
                let (actual, n) = index.measured_distance(x, z);
                assert_eq!(actual, reference(&rects, x, z));
                checks += n;
                queries += 1;
            }
        }
        for [x, z] in [[5999, 9999], [6000, 10000], [-1, -1]] {
            assert_eq!(index.distance(x, z), reference(&rects, x, z));
        }
        assert!(
            checks < queries * rects.len() / 10,
            "index must substantially prune the benchmark layout"
        );
        eprintln!(
            "queries={queries} rectangles={} leaf_checks={checks} full_scan_checks={}",
            rects.len(),
            queries * rects.len()
        );
    }
    #[test]
    #[ignore = "explicit release performance comparison, not a whole-world bake benchmark"]
    fn benchmark_maximum_domain_protection_distance() {
        use std::{hint::black_box, time::Instant};
        let rects = large_rectangles();
        let start = Instant::now();
        let index = DistanceIndex::new(rects.clone());
        let build = start.elapsed();
        let points: Vec<_> = (0..200_000i64)
            .map(|i| [(i * 7919) % 6000, (i * 104729) % 10000])
            .collect();
        let start = Instant::now();
        let old: Vec<_> = points
            .iter()
            .map(|[x, z]| black_box(reference(black_box(&rects), *x, *z)))
            .collect();
        let scan = start.elapsed();
        let start = Instant::now();
        let new: Vec<_> = points
            .iter()
            .map(|[x, z]| black_box(index.distance(black_box(*x), black_box(*z))))
            .collect();
        let indexed = start.elapsed();
        assert_eq!(old, new);
        let bytes = index.rectangles.capacity() * std::mem::size_of::<Bounds>()
            + index.nodes.capacity() * std::mem::size_of::<Node>();
        println!(
            "r42 protection query benchmark: domain=6000x10000 queries={} rectangles={} build_us={} scan_us={} indexed_us={} index_capacity_bytes={} (not process peak memory)",
            points.len(),
            rects.len(),
            build.as_micros(),
            scan.as_micros(),
            indexed.as_micros(),
            bytes
        );
    }
}
