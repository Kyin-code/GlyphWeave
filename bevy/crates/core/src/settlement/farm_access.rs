//! Bounded, deterministic farm-lane search. GlyphWeave-original integration:
//! 2m centre grid, caller-validated full 4m footprint, unchanged length budget.
use super::development::Rect;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};
type Point = [i64; 2];
#[derive(Debug, Clone)]
pub(super) struct Target {
    pub road_id: String,
    pub boundary: Rect,
}
impl Target {
    pub(super) fn distance(&self, p: Point) -> u64 {
        let r = self.boundary;
        let x = p[0].clamp(r.x + 2, r.x + i64::from(r.width_m) - 2);
        let z = p[1].clamp(r.z + 2, r.z + i64::from(r.depth_m) - 2);
        p[0].abs_diff(x) + p[1].abs_diff(z)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Search {
    pub road_id: Option<String>,
    pub path: Vec<Point>,
    pub expanded: usize,
    pub reason: &'static str,
}
fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut result: Vec<Point> = vec![];
    for p in points {
        while result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            if a[0] == b[0] && b[0] == p[0] || a[1] == b[1] && b[1] == p[1] {
                result.pop();
            } else {
                break;
            }
        }
        result.push(p);
    }
    result
}
pub(super) fn length(path: &[Point]) -> u64 {
    path.windows(2)
        .map(|p| p[0][0].abs_diff(p[1][0]) + p[0][1].abs_diff(p[1][1]))
        .sum()
}
pub(super) fn find(
    starts: &[Point],
    targets: &[Target],
    max_length: u64,
    max_expanded: usize,
    mut legal: impl FnMut(Point) -> bool,
) -> Search {
    let mut result = Search {
        road_id: None,
        path: vec![],
        expanded: 0,
        reason: "no_verified_detour",
    };
    if targets.is_empty() {
        result.reason = "no_target_road";
        return result;
    }
    if targets
        .iter()
        .any(|t| t.boundary.width_m < 4 || t.boundary.depth_m < 4)
    {
        result.reason = "invalid_target_road_width";
        return result;
    }
    let heuristic = |p| targets.iter().map(|t| t.distance(p)).min().unwrap();
    let mut nodes = BTreeMap::<Point, (u64, Option<Point>)>::new();
    let mut frontier = BinaryHeap::new();
    let mut starts = starts.to_vec();
    starts.sort();
    starts.dedup();
    for p in starts {
        let h = heuristic(p);
        if h <= max_length && legal(p) {
            nodes.insert(p, (0, None));
            frontier.push(Reverse((h, 0, p)));
        }
    }
    let mut feasibility = BTreeMap::new();
    while let Some(Reverse((_, g, p))) = frontier.pop() {
        if nodes[&p].0 != g {
            continue;
        }
        if result.expanded >= max_expanded {
            result.reason = "detour_expansion_budget_exhausted";
            return result;
        }
        result.expanded += 1;
        if let Some(target) = targets
            .iter()
            .filter(|t| t.distance(p) == 0)
            .min_by(|a, b| a.road_id.cmp(&b.road_id))
        {
            let mut chain = vec![p];
            let mut cursor = p;
            while let Some(parent) = nodes[&cursor].1 {
                chain.push(parent);
                cursor = parent;
            }
            chain.reverse();
            result.path = simplify(chain);
            result.road_id = Some(target.road_id.clone());
            result.reason = "verified_detour";
            debug_assert_eq!(length(&result.path), g);
            return result;
        }
        let next_cost = g + 2;
        if next_cost > max_length {
            continue;
        }
        for [dx, dz] in [[-2, 0], [0, -2], [0, 2], [2, 0]] {
            let (Some(x), Some(z)) = (p[0].checked_add(dx), p[1].checked_add(dz)) else {
                continue;
            };
            let next = [x, z];
            let h = heuristic(next);
            if h > max_length - next_cost || nodes.get(&next).is_some_and(|n| n.0 <= next_cost) {
                continue;
            }
            if !*feasibility.entry(next).or_insert_with(|| legal(next)) {
                continue;
            }
            nodes.insert(next, (next_cost, Some(p)));
            frontier.push(Reverse((next_cost + h, next_cost, next)));
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> Target {
        Target {
            road_id: "road".into(),
            boundary: Rect {
                x: 36,
                z: 0,
                width_m: 8,
                depth_m: 40,
            },
        }
    }
    #[test]
    fn detours_around_barrier_and_preserves_metric_length_and_determinism() {
        let allowed = |[x, z]: Point| {
            (0..=40).contains(&x)
                && (2..=38).contains(&z)
                && !(x >= 14 && x <= 24 && z >= 6 && z <= 30)
        };
        let a = find(&[[4, 20]], &[target()], 80, 4096, allowed);
        assert_eq!(a.road_id.as_deref(), Some("road"));
        assert!(a.path.len() > 3);
        assert!(length(&a.path) <= 80);
        assert!(
            a.path
                .windows(2)
                .all(|p| p[0][0] == p[1][0] || p[0][1] == p[1][1])
        );
        assert_eq!(a, find(&[[4, 20]], &[target()], 80, 4096, allowed));
        assert!(
            find(&[[4, 20]], &[target()], 34, 4096, allowed)
                .road_id
                .is_none()
        );
    }
    #[test]
    fn barriers_and_expansion_budget_never_report_a_fake_route() {
        let blocked = find(&[[4, 20]], &[target()], 80, 4096, |[x, _]| x < 14);
        assert!(blocked.road_id.is_none());
        assert!(blocked.path.is_empty());
        let exhausted = find(&[[4, 20]], &[target()], 80, 1, |_| true);
        assert_eq!(exhausted.reason, "detour_expansion_budget_exhausted");
        assert_eq!(exhausted.expanded, 1);
        assert!(exhausted.road_id.is_none());
    }
    #[test]
    fn target_identity_ties_and_large_origins_are_stable() {
        for origin in [-1_000_000_000, 0, 1_000_000_000] {
            let mut a = target();
            a.boundary.x += origin;
            a.boundary.z += origin;
            let mut b = a.clone();
            b.road_id = "a-road".into();
            let p = [origin + 4, origin + 20];
            let x = find(&[p], &[a.clone(), b.clone()], 80, 4096, |_| true);
            let y = find(&[p, p], &[b, a], 80, 4096, |_| true);
            assert_eq!(x, y);
            assert_eq!(x.road_id.as_deref(), Some("a-road"));
            assert_eq!(length(&x.path), 34);
        }
    }
}
