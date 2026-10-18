//! Bounded pedestrian route search against final baked terrain and obstacles.
//! A road-distance hint is not connectivity. Each accepted route carries its
//! checked polyline; absence of a route is reported, never deferred by origin.
use super::*;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
};
pub(super) const HALF_WIDTH: f64 = 0.75;
const STEP: f64 = 2.;
pub(super) const MAX_LENGTH: f64 = 120.;
/// The walkable longitudinal grade a checked route may use, as a rise over run.
/// One contract, shared by the route predicate and by the planner's entrance
/// apron gate (B01-T2 R11 G3) so the earthwork can never authorise a step the
/// route check then rejects.
pub(super) const WALKABLE_GRADE: f64 = 0.25;
const MAX_EXPANSIONS: usize = 6000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccessGraph {
    #[serde(default)]
    pub version: u32,
    pub links: Vec<AccessLink>,
    pub connected_entities: usize,
    pub disconnected_entities: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessLink {
    pub entity_id: String,
    pub anchor_id: String,
    pub road_id: Option<String>,
    /// Actual route length; null when no checked route exists (never Infinity).
    pub distance_m: Option<f64>,
    pub connected: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub path: Vec<AccessPoint>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessPoint {
    pub x: f64,
    pub z: f64,
}
type P = AccessPoint;
pub(super) fn point(x: f64, z: f64) -> P {
    P { x, z }
}
fn distance(a: P, b: P) -> f64 {
    (a.x - b.x).hypot(a.z - b.z)
}
fn local(e: &EntityInstance, p: P) -> P {
    let (s, c) = f64::from(e.rotation_y_deg).to_radians().sin_cos();
    let dx = p.x - f64::from(e.world_x);
    let dz = p.z - f64::from(e.world_z);
    point(dx * c - dz * s, dx * s + dz * c)
}
fn world(e: &EntityInstance, p: P) -> P {
    let (s, c) = f64::from(e.rotation_y_deg).to_radians().sin_cos();
    point(
        f64::from(e.world_x) + p.x * c + p.z * s,
        f64::from(e.world_z) - p.x * s + p.z * c,
    )
}
fn closest(e: &EntityInstance, p: P) -> P {
    let p = local(e, p);
    world(
        e,
        point(
            p.x.clamp(-f64::from(e.width_m) * 0.5, f64::from(e.width_m) * 0.5),
            p.z.clamp(-f64::from(e.depth_m) * 0.5, f64::from(e.depth_m) * 0.5),
        ),
    )
}
fn building(kind: &str) -> bool {
    (is_building_kind(kind) && !matches!(kind, "parking_lot" | "green_space" | "community_plaza"))
        || matches!(
            kind,
            "shopping_mall" | "office_tower" | "convenience_store" | "night_market"
        )
}
fn obstacle(kind: &str) -> bool {
    building(kind)
        || matches!(
            kind,
            "wall"
                | "fence"
                | "guardrail"
                | "rock"
                | "boulder"
                | "tree"
                | "bush"
                | "fallen_log"
                | "hill"
                | "mountain"
                | "rock_mountain"
        )
}
// Segment vs inflated oriented footprint, including thin walls between nodes.
fn crosses(e: &EntityInstance, a: P, b: P) -> bool {
    let a = local(e, a);
    let b = local(e, b);
    let mut lo: f64 = 0.;
    let mut hi: f64 = 1.;
    for (p, d, h) in [
        (a.x, b.x - a.x, f64::from(e.width_m) * 0.5 + HALF_WIDTH),
        (a.z, b.z - a.z, f64::from(e.depth_m) * 0.5 + HALF_WIDTH),
    ] {
        if d.abs() < 1e-10 {
            if p.abs() >= h {
                return false;
            }
        } else {
            let t0 = (-h - p) / d;
            let t1 = (h - p) / d;
            lo = lo.max(t0.min(t1));
            hi = hi.min(t0.max(t1));
        }
    }
    lo < hi && hi > 0. && lo < 1.
}
pub(super) fn height(terrain: &BakedTerrain, p: P) -> Option<f64> {
    let x = p.x.floor() as i32;
    let z = p.z.floor() as i32;
    let fx = p.x - f64::from(x);
    let fz = p.z - f64::from(z);
    let a = f64::from(terrain.height(x, z)?);
    let b = f64::from(terrain.height(x.checked_add(1)?, z)?);
    let c = f64::from(terrain.height(x, z.checked_add(1)?)?);
    let d = f64::from(terrain.height(x.checked_add(1)?, z.checked_add(1)?)?);
    Some((a * (1. - fx) + b * fx) * (1. - fz) + (c * (1. - fx) + d * fx) * fz)
}
fn segment_clear(
    a: P,
    b: P,
    obstacles: &[&EntityInstance],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> bool {
    if obstacles.iter().any(|e| crosses(e, a, b)) {
        return false;
    }
    if water.intersects_corridor(a.x, a.z, b.x, b.z, HALF_WIDTH) {
        return false;
    }
    terrain_segment_clear_xy(a.x, a.z, b.x, b.z, terrain)
}

/// Terrain-only half of the walkable-segment predicate, extracted without any
/// semantic change so the engineering-connections module reuses the production
/// sampling and grade contract instead of re-implementing it. Obstacle and water
/// checks stay in `segment_clear`; thresholds, batch size and return values are
/// unchanged.
pub(super) fn terrain_segment_clear_xy(
    ax: f64,
    az: f64,
    bx: f64,
    bz: f64,
    terrain: &BakedTerrain,
) -> bool {
    let a = point(ax, az);
    let b = point(bx, bz);
    let length = distance(a, b);
    let n = (length / 0.5).ceil().max(1.) as usize;
    let (nx, nz) = if length > 0. {
        (-(b.z - a.z) / length, (b.x - a.x) / length)
    } else {
        (1., 0.)
    };
    // Centre and both edges must have terrain and a walkable gradient.
    for lateral in [-HALF_WIDTH, 0., HALF_WIDTH] {
        let mut previous: Option<f64> = None;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let p = point(
                a.x + (b.x - a.x) * t + nx * lateral,
                a.z + (b.z - a.z) * t + nz * lateral,
            );
            let Some(h) = height(terrain, p) else {
                return false;
            };
            if let Some(old) = previous {
                if (h - old).abs() > WALKABLE_GRADE * (length / n as f64) + 1e-6 {
                    return false;
                }
            }
            previous = Some(h);
        }
    }
    true
}
fn exit_point(e: &EntityInstance, anchor: &SpatialAnchor) -> Option<P> {
    let p = local(
        e,
        point(f64::from(anchor.world_x), f64::from(anchor.world_z)),
    );
    let hw = f64::from(e.width_m) * 0.5;
    let hd = f64::from(e.depth_m) * 0.5;
    if !hw.is_finite() || !hd.is_finite() || hw <= 0. || hd <= 0. {
        return None;
    }
    let edge = (p.x.abs() - hw).abs().min((p.z.abs() - hd).abs());
    if edge > 2. || p.x.abs() > hw + 2. || p.z.abs() > hd + 2. {
        return None;
    }
    let q = if (p.x.abs() - hw).abs() < (p.z.abs() - hd).abs() {
        point(p.x.signum() * (hw + HALF_WIDTH + 0.1), p.z.clamp(-hd, hd))
    } else {
        point(p.x.clamp(-hw, hw), p.z.signum() * (hd + HALF_WIDTH + 0.1))
    };
    Some(world(e, q))
}
fn route(
    start: P,
    roads: &[&EntityInstance],
    obstacles: &[&EntityInstance],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> Option<(String, Vec<P>, f64)> {
    #[cfg(test)]
    observation::begin_route(start);
    let mut goals: Vec<_> = roads
        .iter()
        .filter_map(|road| {
            let end = closest(road, start);
            let d = distance(start, end);
            #[cfg(test)]
            if d > MAX_LENGTH {
                observation::goal_excluded(road, end, d);
            } else {
                observation::goal(road, end, d);
            }
            (d <= MAX_LENGTH).then_some((*road, end, d))
        })
        .collect();
    goals.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.entity_id.cmp(&b.0.entity_id)));
    for (road, end, d) in &goals {
        if segment_clear(start, *end, obstacles, terrain, water) {
            #[cfg(test)]
            observation::direct_attempt(road, *end, *d, true);
            #[cfg(test)]
            observation::terminated("direct", 0);
            return Some((road.entity_id.clone(), vec![start, *end], *d));
        }
        #[cfg(test)]
        {
            observation::direct_attempt(road, *end, *d, false);
            observation::segment_rejected(start, *end, obstacles, terrain, water, "direct");
        }
    }
    if goals.is_empty() {
        #[cfg(test)]
        observation::terminated("no_eligible_goals", 0);
        return None;
    }
    let heuristic = |p: P| {
        goals
            .iter()
            .map(|(r, _, _)| distance(p, closest(r, p)))
            .fold(f64::INFINITY, f64::min)
    };
    let coord =
        |x: i32, z: i32| point(start.x + f64::from(x) * STEP, start.z + f64::from(z) * STEP);
    let mut queue = BinaryHeap::new();
    let mut cost = BTreeMap::new();
    let mut parent = BTreeMap::new();
    let mut done = BTreeSet::new();
    queue.push(Reverse(((heuristic(start) * 1000.).round() as u64, 0, 0)));
    cost.insert((0, 0), 0.);
    while let Some(Reverse((_, x, z))) = queue.pop() {
        if !done.insert((x, z)) {
            continue;
        }
        #[cfg(test)]
        observation::expanded(done.len());
        if done.len() > MAX_EXPANSIONS {
            #[cfg(test)]
            observation::terminated("expansion_limit", done.len());
            return None;
        }
        let p = coord(x, z);
        let g = cost[&(x, z)];
        for (road, _, _) in &goals {
            let end = closest(road, p);
            let d = distance(p, end);
            #[cfg(test)]
            observation::search_goal_attempt(road, p, end, d, g, obstacles, terrain, water);
            if d <= STEP * 1.5
                && g + d <= MAX_LENGTH
                && segment_clear(p, end, obstacles, terrain, water)
            {
                let mut path = vec![end, p];
                let mut key = (x, z);
                while let Some(prev) = parent.get(&key) {
                    key = *prev;
                    path.push(coord(key.0, key.1));
                }
                path.reverse();
                #[cfg(test)]
                observation::terminated("search_goal_reached", done.len());
                return Some((road.entity_id.clone(), path, g + d));
            }
        }
        for (dx, dz) in [
            (0, -1),
            (-1, 0),
            (1, 0),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let next = (x + dx, z + dz);
            if done.contains(&next) {
                continue;
            }
            let q = coord(next.0, next.1);
            let ng = g + distance(p, q);
            let estimate = ng + heuristic(q);
            if estimate > MAX_LENGTH || cost.get(&next).is_some_and(|old| *old <= ng) {
                #[cfg(test)]
                observation::prune_estimate_or_cost(
                    estimate > MAX_LENGTH,
                    cost.get(&next).is_some_and(|old| *old <= ng),
                );
                continue;
            }
            #[cfg(test)]
            observation::queue_len(queue.len());
            if !segment_clear(p, q, obstacles, terrain, water) {
                #[cfg(test)]
                observation::segment_rejected(p, q, obstacles, terrain, water, "search_step");
                continue;
            }
            cost.insert(next, ng);
            parent.insert(next, (x, z));
            queue.push(Reverse(((estimate * 1000.).round() as u64, next.0, next.1)));
            #[cfg(test)]
            observation::queue_len(queue.len());
        }
    }
    #[cfg(test)]
    observation::terminated("queue_exhausted", done.len());
    None
}

pub(super) fn access_for_entity(
    e: &EntityInstance,
    entities: &[EntityInstance],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> Vec<AccessLink> {
    #[cfg(test)]
    observation::enter(&e.entity_id);
    let roads: Vec<_> = entities
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "road" | "sidewalk" | "boardwalk"))
        .collect();
    let obstacles: Vec<_> = entities
        .iter()
        .filter(|other| {
            obstacle(&other.kind)
                && distance(
                    point(f64::from(e.world_x), f64::from(e.world_z)),
                    closest(other, point(f64::from(e.world_x), f64::from(e.world_z))),
                ) <= MAX_LENGTH + f64::from(e.width_m.max(e.depth_m))
        })
        .collect();
    let mut links = vec![];
    for anchor in &e.anchors {
        let mut link = AccessLink {
            entity_id: e.entity_id.clone(),
            anchor_id: anchor.id.clone(),
            road_id: None,
            distance_m: None,
            connected: false,
            required: true,
            status: "no_walkable_route_within_budget".into(),
            path: vec![],
        };
        if let Some(start) = exit_point(e, anchor) {
            // Anchors may sit up to two metres off the facade. The actual
            // pavement must reach the wall, not stop at that routing marker.
            let facade_point = closest(e, start);
            let entrance_obstacles: Vec<_> = obstacles
                .iter()
                .copied()
                .filter(|o| o.entity_id != e.entity_id)
                .collect();
            #[cfg(test)]
            observation::entrance(
                e,
                start,
                facade_point,
                &entrance_obstacles,
                terrain,
                water,
            );
            if terrain.require_complete().is_err() {
                link.status = "incomplete_terrain".into();
            } else if [start, facade_point].into_iter().any(|p| {
                height(terrain, p).is_none_or(|h| (h - f64::from(e.ground_height_m())).abs() > 0.30)
            }) {
                link.status = "entrance_step_requires_engineering".into();
            } else if !segment_clear(facade_point, start, &entrance_obstacles, terrain, water) {
                link.status = "entrance_clearance_blocked".into();
            } else if let Some((road, mut path, length)) =
                route(start, &roads, &obstacles, terrain, water)
            {
                if length + distance(facade_point, start) > MAX_LENGTH {
                    links.push(link);
                    continue;
                }
                path.insert(0, facade_point);
                link.road_id = Some(road);
                link.path = path;
                link.distance_m = Some(length + distance(facade_point, start));
                link.connected = true;
                link.status = "walkable_route".into();
            }
        } else {
            link.status = "anchor_not_on_building_boundary".into();
        }
        links.push(link);
    }
    if links.is_empty() {
        links.push(AccessLink {
            entity_id: e.entity_id.clone(),
            anchor_id: String::new(),
            road_id: None,
            distance_m: None,
            connected: false,
            required: true,
            status: "missing_entrance".into(),
            path: vec![],
        });
    }
    #[cfg(test)]
    {
        for link in &links {
            observation::link(link);
        }
        observation::leave();
    }
    links
}

pub fn build_access_graph(
    entities: &[EntityInstance],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> AccessGraph {
    let mut graph = AccessGraph {
        version: 2,
        ..Default::default()
    };
    for e in entities.iter().filter(|e| building(&e.kind)) {
        let links = access_for_entity(e, entities, terrain, water);
        if links.iter().any(|l| l.connected) {
            graph.connected_entities += 1;
        } else {
            graph.disconnected_entities += 1;
        }
        graph.links.extend(links);
    }
    graph
}

/// Generation-only door selection. Keep authored/custom anchors unchanged;
/// select a facade for synthetic generated `front` entrances on final terrain.
/// Does not move buildings, edit terrain, remove obstacles or change thresholds.
pub(super) fn place_generated_entrances(
    entities: &mut [EntityInstance],
    landmarks: &[LandmarkSpec],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> AccessGraph {
    let authored: BTreeSet<_> = landmarks.iter().map(|l| l.entity_id.as_str()).collect();
    let mut graph = AccessGraph {
        version: 2,
        ..Default::default()
    };
    for i in 0..entities.len() {
        if !building(&entities[i].kind) {
            continue;
        }
        let original = &entities[i];
        let mut links = access_for_entity(original, entities, terrain, water);
        if !links.iter().any(|l| l.connected)
            && original.entity_id.starts_with("generated.")
            && !authored.contains(original.entity_id.as_str())
            // A frontage station owns its door, local platform and road point
            // as one engineering contract. If it cannot route, preserve that
            // failed obligation; do not turn it into a different building.
            && original.grounding.frontage_road_point.is_none()
            && (original.anchors.is_empty()
                || (original.anchors.len() == 1 && original.anchors[0].id == "front"))
        {
            let mut best: Option<(EntityInstance, Vec<AccessLink>)> = None;
            let hw = f64::from(original.width_m) * 0.5 + 1.;
            let hd = f64::from(original.depth_m) * 0.5 + 1.;
            for p in [point(0., -hd), point(hw, 0.), point(0., hd), point(-hw, 0.)] {
                let at = world(original, p);
                let mut candidate = original.clone();
                let dx = at.x - f64::from(original.world_x);
                let dz = at.z - f64::from(original.world_z);
                let direction = if dx.abs() > dz.abs() {
                    if dx > 0. { "east" } else { "west" }
                } else if dz > 0. {
                    "south"
                } else {
                    "north"
                };
                candidate.anchors = vec![SpatialAnchor {
                    id: "front".into(),
                    world_x: at.x.round() as i32,
                    world_z: at.z.round() as i32,
                    direction: direction.into(),
                    target: Some("road".into()),
                }];
                let candidate_links = access_for_entity(&candidate, entities, terrain, water);
                if candidate_links[0].connected
                    && best.as_ref().is_none_or(|(_, links)| {
                        candidate_links[0].distance_m < links[0].distance_m
                    })
                {
                    best = Some((candidate, candidate_links));
                }
            }
            if let Some((candidate, new_links)) = best {
                entities[i] = candidate;
                links = new_links;
            }
        }
        if links.iter().any(|l| l.connected) {
            graph.connected_entities += 1;
        } else {
            graph.disconnected_entities += 1;
        }
        graph.links.extend(links);
    }
    graph
}

/// Only unplanned procedural candidates can be rejected after route search.
/// Failed planned/authored obligations remain visible to independent audits.
pub(super) fn rejectable_candidates(
    graph: &AccessGraph,
    protected: &BTreeSet<String>,
) -> BTreeSet<String> {
    graph
        .links
        .iter()
        .filter(|l| {
            !l.connected
                && l.entity_id.starts_with("generated.")
                && !protected.contains(&l.entity_id)
        })
        .map(|l| l.entity_id.clone())
        .collect()
}
/// B01-G12 test-only passive observation. It reads the variables the production
/// solver already decided on; it never feeds a value back into a decision, never
/// changes a constant, order, threshold or return value, and it does not exist in
/// non-test builds. It is off by default, scoped to one thread and one target
/// entity by a guard, and drops every record for any other entity, so it cannot
/// pollute other tests.
#[cfg(test)]
pub(crate) mod observation {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// Mirrors the production literals inside `segment_clear`. The differential
    /// tests in this module assert that `classify_segment` agrees with the
    /// production predicate on every synthetic input they build.
    const SAMPLE_STEP_M: f64 = 0.5;
    const GRADE_LIMIT: f64 = 0.25;
    const SAMPLE_CAP_PER_CLASS: usize = 8;

    fn json_map(entries: Vec<(&str, serde_json::Value)>) -> serde_json::Value {
        serde_json::Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct Trace {
        pub target: String,
        pub counters: BTreeMap<String, i64>,
        pub samples: BTreeMap<String, Vec<serde_json::Value>>,
        pub truncated: BTreeMap<String, i64>,
        pub entrance: Vec<serde_json::Value>,
        pub goals: Vec<serde_json::Value>,
        pub goal_exclusions: Vec<serde_json::Value>,
        pub directAttempts: Vec<serde_json::Value>,
        pub searchGoalAttempts: Vec<serde_json::Value>,
        pub routes: Vec<serde_json::Value>,
        pub links: Vec<serde_json::Value>,
        pub queuePeak: usize,
        pub expandedPeak: usize,
    }

    struct State {
        trace: Trace,
        current: Option<String>,
        queue_peak: usize,
    }

    thread_local! {
        static ACTIVE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    /// Run `body` with the observer enabled for `target` on this thread only.
    pub(crate) fn observe<T>(target: &str, body: impl FnOnce() -> T) -> (T, Trace) {
        ACTIVE.with(|slot| {
            assert!(
                slot.borrow().is_none(),
                "the passive observer is already active on this thread"
            );
            *slot.borrow_mut() = Some(State {
                trace: Trace {
                    target: target.to_string(),
                    ..Trace::default()
                },
                current: None,
                queue_peak: 0,
            });
        });
        let value = body();
        let state = ACTIVE
            .with(|slot| slot.borrow_mut().take())
            .expect("observer state");
        (value, state.trace)
    }

    fn recording<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
        ACTIVE.with(|slot| {
            let mut guard = slot.borrow_mut();
            guard.as_mut().and_then(|state| {
                if state.current.as_deref() == Some(state.trace.target.as_str()) {
                    Some(f(state))
                } else {
                    None
                }
            })
        })
    }

    fn record(state: &mut State, class: &str, value: serde_json::Value) {
        *state.trace.counters.entry(class.to_string()).or_default() += 1;
        let list = state.trace.samples.entry(class.to_string()).or_default();
        if list.len() < SAMPLE_CAP_PER_CLASS {
            list.push(value);
        } else {
            *state.trace.truncated.entry(class.to_string()).or_default() += 1;
        }
    }

    pub(crate) fn enter(entity_id: &str) {
        ACTIVE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                state.current = Some(entity_id.to_string());
            }
        });
    }

    pub(crate) fn leave() {
        ACTIVE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                state.current = None;
            }
        });
    }

    pub(crate) fn entrance(
        e: &EntityInstance,
        start: P,
        facade: P,
        entrance_obstacles: &[&EntityInstance],
        terrain: &BakedTerrain,
        water: WaterGeometry,
    ) {
        let _ = recording(|state| {
            let ground = f64::from(e.ground_height_m());
            let start_h = height(terrain, start);
            let facade_h = height(terrain, facade);
            let delta = |h: Option<f64>| h.map(|value| (value - ground).abs());
            let record_value = json_map(vec![
                ("entityId", serde_json::json!(e.entity_id)),
                ("anchor", serde_json::json!([start.x, start.z])),
                ("facade", serde_json::json!([facade.x, facade.z])),
                ("groundM", serde_json::json!(ground)),
                ("startHeightM", serde_json::json!(start_h)),
                ("facadeHeightM", serde_json::json!(facade_h)),
                ("startDeltaM", serde_json::json!(delta(start_h))),
                ("facadeDeltaM", serde_json::json!(delta(facade_h))),
                ("stepLimitM", serde_json::json!(0.30)),
                (
                    "stepWithin",
                    serde_json::json!(
                        delta(start_h).is_some_and(|value| value <= 0.30)
                            && delta(facade_h).is_some_and(|value| value <= 0.30)
                    ),
                ),
                (
                    "entranceSegmentClear",
                    serde_json::json!(
                        start_h.is_some()
                            && facade_h.is_some()
                            && segment_clear(facade, start, entrance_obstacles, terrain, water)
                    ),
                ),
                (
                    "entranceObstacles",
                    serde_json::json!(entrance_obstacles.len()),
                ),
            ]);
            state.trace.entrance.push(record_value);
        });
    }

    pub(crate) fn link(link: &AccessLink) {
        let _ = recording(|state| {
            state.trace.links.push(json_map(vec![
                ("entityId", serde_json::json!(link.entity_id)),
                ("anchorId", serde_json::json!(link.anchor_id)),
                ("roadId", serde_json::json!(link.road_id)),
                ("connected", serde_json::json!(link.connected)),
                ("status", serde_json::json!(link.status)),
                ("distanceM", serde_json::json!(link.distance_m)),
                ("pathPoints", serde_json::json!(link.path.len())),
                (
                    "path",
                    serde_json::json!(link
                        .path
                        .iter()
                        .map(|p| serde_json::json!([p.x, p.z]))
                        .collect::<Vec<_>>()),
                ),
            ]));
        });
    }

    pub(crate) fn begin_route(start: P) {
        ACTIVE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                state.queue_peak = 0;
                state.trace.queuePeak = 0;
                state.trace.expandedPeak = 0;
            }
        });
        let _ = recording(|state| {
            *state.trace.counters.entry("routes.started".into()).or_default() += 1;
            let _ = start;
        });
    }

    pub(crate) fn goal(road: &EntityInstance, end: P, d: f64) {
        let _ = recording(|state| {
            state.trace.goals.push(json_map(vec![
                ("roadId", serde_json::json!(road.entity_id)),
                ("end", serde_json::json!([end.x, end.z])),
                ("distanceM", serde_json::json!(d)),
            ]));
            *state
                .trace
                .counters
                .entry("goals.eligible".into())
                .or_default() += 1;
        });
    }

    pub(crate) fn goal_excluded(road: &EntityInstance, end: P, d: f64) {
        let _ = recording(|state| {
            state.trace.goal_exclusions.push(json_map(vec![
                ("roadId", serde_json::json!(road.entity_id)),
                ("end", serde_json::json!([end.x, end.z])),
                ("distanceM", serde_json::json!(d)),
                ("reason", serde_json::json!("distance_exceeds_max_length")),
                ("maxLengthM", serde_json::json!(MAX_LENGTH)),
            ]));
            *state
                .trace
                .counters
                .entry("goals.excluded_distance".into())
                .or_default() += 1;
        });
    }

    pub(crate) fn direct_attempt(road: &EntityInstance, end: P, d: f64, clear: bool) {
        let _ = recording(|state| {
            state.trace.directAttempts.push(json_map(vec![
                ("roadId", serde_json::json!(road.entity_id)),
                ("end", serde_json::json!([end.x, end.z])),
                ("distanceM", serde_json::json!(d)),
                ("segmentClear", serde_json::json!(clear)),
            ]));
            *state
                .trace
                .counters
                .entry(if clear {
                    "direct.accepted".into()
                } else {
                    "direct.rejected".into()
                })
                .or_default() += 1;
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_goal_attempt(
        road: &EntityInstance,
        p: P,
        end: P,
        d: f64,
        g: f64,
        obstacles: &[&EntityInstance],
        terrain: &BakedTerrain,
        water: WaterGeometry,
    ) {
        // Recompute the production sub-conditions from the same values; the
        // production expression itself is untouched because this runs only in a
        // cfg(test) statement after the decision variables are read.
        let near = d <= STEP * 1.5;
        let within_length = g + d <= MAX_LENGTH;
        let clear = near && within_length && segment_clear(p, end, obstacles, terrain, water);
        let _ = recording(|state| {
            state.trace.searchGoalAttempts.push(json_map(vec![
                ("roadId", serde_json::json!(road.entity_id)),
                ("at", serde_json::json!([p.x, p.z])),
                ("end", serde_json::json!([end.x, end.z])),
                ("distanceM", serde_json::json!(d)),
                ("costM", serde_json::json!(g)),
                ("withinStep", serde_json::json!(near)),
                ("withinLengthBudget", serde_json::json!(within_length)),
                ("segmentClear", serde_json::json!(clear)),
            ]));
            let class = if clear {
                "search.goal_accepted"
            } else if !near {
                "search.goal_not_within_step"
            } else if !within_length {
                "search.goal_over_length_budget"
            } else {
                "search.goal_segment_blocked"
            };
            *state.trace.counters.entry(class.into()).or_default() += 1;
        });
        if near && within_length && !clear {
            segment_rejected(p, end, obstacles, terrain, water, "search_goal");
        }
    }

    pub(crate) fn expanded(count: usize) {
        let _ = recording(|state| {
            state.trace.expandedPeak = state.trace.expandedPeak.max(count);
        });
    }

    pub(crate) fn queue_len(len: usize) {
        ACTIVE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                state.queue_peak = state.queue_peak.max(len);
                state.trace.queuePeak = state.trace.queuePeak.max(len);
            }
        });
    }

    pub(crate) fn prune_estimate_or_cost(over_budget: bool, dominated: bool) {
        let _ = recording(|state| {
            if over_budget {
                *state
                    .trace
                    .counters
                    .entry("search.prune_length_or_estimate".into())
                    .or_default() += 1;
            }
            if dominated {
                *state
                    .trace
                    .counters
                    .entry("search.prune_cost_dominance".into())
                    .or_default() += 1;
            }
        });
    }

    pub(crate) fn terminated(reason: &str, expanded: usize) {
        let _ = recording(|state| {
            state.trace.routes.push(json_map(vec![
                ("termination", serde_json::json!(reason)),
                ("expanded", serde_json::json!(expanded)),
                ("maxExpansions", serde_json::json!(MAX_EXPANSIONS)),
                ("queuePeak", serde_json::json!(state.trace.queuePeak)),
                ("maxLengthM", serde_json::json!(MAX_LENGTH)),
                ("stepM", serde_json::json!(STEP)),
            ]));
            *state
                .trace
                .counters
                .entry(format!("termination.{reason}"))
                .or_default() += 1;
        });
    }

    pub(crate) fn segment_rejected(
        a: P,
        b: P,
        obstacles: &[&EntityInstance],
        terrain: &BakedTerrain,
        water: WaterGeometry,
        context: &str,
    ) {
        let Some(classification) = classify_segment(a, b, obstacles, terrain, water) else {
            // the production predicate refused for a reason this classifier did not
            // reproduce: record it as unknown rather than inventing a cause
            let _ = recording(|state| {
                record(
                    state,
                    "segment.unclassified",
                    json_map(vec![
                        ("context", serde_json::json!(context)),
                        ("segmentFrom", serde_json::json!([a.x, a.z])),
                        ("segmentTo", serde_json::json!([b.x, b.z])),
                    ]),
                );
            });
            return;
        };
        let class = classification
            .get("class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let _ = recording(|state| {
            let mut value = classification;
            if let Some(object) = value.as_object_mut() {
                object.insert("context".into(), serde_json::json!(context));
            }
            record(state, &format!("segment_reject.{class}"), value);
        });
    }

    /// Test-only mirror of the production `segment_clear` decision order, used only
    /// to name a reason for a segment the production predicate already refused.
    /// The differential tests assert `segment_clear(..) == classify_segment(..).is_none()`.
    pub(crate) fn classify_segment(
        a: P,
        b: P,
        obstacles: &[&EntityInstance],
        terrain: &BakedTerrain,
        water: WaterGeometry,
    ) -> Option<serde_json::Value> {
        // The whole segment is reported as `segmentFrom`/`segmentTo`; the actual
        // adjacent sampling pair that failed is reported separately as
        // `sampleFrom`/`sampleTo` with its index, lateral offset and step, so a
        // reader can re-interpolate the recorded coordinates and reproduce the
        // recorded heights instead of mistaking the segment end for the sample.
        let segment = |class: &str| {
            json_map(vec![
                ("class", serde_json::json!(class)),
                ("segmentFrom", serde_json::json!([a.x, a.z])),
                ("segmentTo", serde_json::json!([b.x, b.z])),
            ])
        };
        if let Some(hit) = obstacles.iter().find(|e| crosses(e, a, b)) {
            let mut value = segment("obstacle");
            if let Some(object) = value.as_object_mut() {
                object.insert("entityId".into(), serde_json::json!(hit.entity_id));
            }
            return Some(value);
        }
        if water.intersects_corridor(a.x, a.z, b.x, b.z, HALF_WIDTH) {
            let mut value = segment("water");
            if let Some(object) = value.as_object_mut() {
                object.insert("halfWidthM".into(), serde_json::json!(HALF_WIDTH));
            }
            return Some(value);
        }
        let length = distance(a, b);
        let n = (length / SAMPLE_STEP_M).ceil().max(1.) as usize;
        let (nx, nz) = if length > 0. {
            (-(b.z - a.z) / length, (b.x - a.x) / length)
        } else {
            (1., 0.)
        };
        for lateral in [-HALF_WIDTH, 0., HALF_WIDTH] {
            let mut previous: Option<(P, f64, usize)> = None;
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let p = point(
                    a.x + (b.x - a.x) * t + nx * lateral,
                    a.z + (b.z - a.z) * t + nz * lateral,
                );
                let Some(h) = height(terrain, p) else {
                    let mut value = segment("missing");
                    if let Some(object) = value.as_object_mut() {
                        object.insert("sampleAt".into(), serde_json::json!([p.x, p.z]));
                        object.insert("sampleIndex".into(), serde_json::json!(i));
                        object.insert("lateralM".into(), serde_json::json!(lateral));
                    }
                    return Some(value);
                };
                if let Some((old_p, old, old_index)) = previous {
                    let step = length / n as f64;
                    if (h - old).abs() > GRADE_LIMIT * step + 1e-6 {
                        let mut value = segment("grade");
                        if let Some(object) = value.as_object_mut() {
                            object.insert(
                                "sampleFrom".into(),
                                serde_json::json!([old_p.x, old_p.z]),
                            );
                            object.insert("sampleTo".into(), serde_json::json!([p.x, p.z]));
                            object.insert(
                                "sampleIndex".into(),
                                serde_json::json!(old_index),
                            );
                            object.insert("sampleIndexTo".into(), serde_json::json!(i));
                            object.insert("sampleCount".into(), serde_json::json!(n));
                            object.insert("heightFromM".into(), serde_json::json!(old));
                            object.insert("heightToM".into(), serde_json::json!(h));
                            object.insert("stepM".into(), serde_json::json!(step));
                            object.insert("deltaM".into(), serde_json::json!((h - old).abs()));
                            object.insert(
                                "limitM".into(),
                                serde_json::json!(GRADE_LIMIT * step),
                            );
                            object.insert("lateralM".into(), serde_json::json!(lateral));
                            object.insert(
                                "segmentLengthM".into(),
                                serde_json::json!(length),
                            );
                        }
                        return Some(value);
                    }
                }
                previous = Some((p, h, i));
            }
        }
        None
    }

    /// Test-only: the full three-line sampling profile of one segment, using the same
    /// bilinear lookup and 0.5m spacing as the production predicate. It reads only;
    /// it cannot change a decision.
    pub(crate) fn sample_profile(
        terrain: &BakedTerrain,
        a: P,
        b: P,
    ) -> serde_json::Value {
        let length = distance(a, b);
        let n = (length / SAMPLE_STEP_M).ceil().max(1.) as usize;
        let (nx, nz) = if length > 0. {
            (-(b.z - a.z) / length, (b.x - a.x) / length)
        } else {
            (1., 0.)
        };
        let step = length / n as f64;
        let mut lines: Vec<serde_json::Value> = vec![];
        for lateral in [-HALF_WIDTH, 0., HALF_WIDTH] {
            let mut samples: Vec<serde_json::Value> = vec![];
            let mut previous: Option<f64> = None;
            let mut first_failure: Option<serde_json::Value> = None;
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let p = point(
                    a.x + (b.x - a.x) * t + nx * lateral,
                    a.z + (b.z - a.z) * t + nz * lateral,
                );
                let h = height(terrain, p);
                let mut entry = json_map(vec![
                    ("index", serde_json::json!(i)),
                    ("at", serde_json::json!([p.x, p.z])),
                    ("heightM", serde_json::json!(h)),
                ]);
                if let (Some(previous_h), Some(h)) = (previous, h) {
                    let delta = (h - previous_h).abs();
                    if let Some(object) = entry.as_object_mut() {
                        object.insert("previousHeightM".into(), serde_json::json!(previous_h));
                        object.insert("deltaM".into(), serde_json::json!(delta));
                        object.insert(
                            "limitM".into(),
                            serde_json::json!(GRADE_LIMIT * step),
                        );
                        object.insert(
                            "gradeClear".into(),
                            serde_json::json!(delta <= GRADE_LIMIT * step + 1e-6),
                        );
                    }
                    if first_failure.is_none() && delta > GRADE_LIMIT * step + 1e-6 {
                        first_failure = Some(json_map(vec![
                            ("indexFrom", serde_json::json!(i - 1)),
                            ("indexTo", serde_json::json!(i)),
                            ("deltaM", serde_json::json!(delta)),
                        ]));
                    }
                }
                previous = h;
                samples.push(entry);
            }
            lines.push(json_map(vec![
                ("lateralM", serde_json::json!(lateral)),
                ("stepM", serde_json::json!(step)),
                ("sampleCount", serde_json::json!(n + 1)),
                ("firstFailure", first_failure.unwrap_or(serde_json::Value::Null)),
                ("samples", serde_json::json!(samples)),
            ]));
        }
        json_map(vec![
            ("segmentFrom", serde_json::json!([a.x, a.z])),
            ("segmentTo", serde_json::json!([b.x, b.z])),
            ("segmentLengthM", serde_json::json!(length)),
            ("lines", serde_json::json!(lines)),
        ])
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn planned_failed_routes_are_not_deleted_as_optional_candidates() {
        let mut graph = AccessGraph::default();
        for id in ["authored.home", "generated.planned", "generated.optional"] {
            graph.links.push(AccessLink {
                entity_id: id.into(),
                anchor_id: "front".into(),
                road_id: None,
                distance_m: None,
                connected: false,
                required: true,
                status: "failed".into(),
                path: vec![],
            });
        }
        let protected = BTreeSet::from(["authored.home".into(), "generated.planned".into()]);
        assert_eq!(
            rejectable_candidates(&graph, &protected),
            BTreeSet::from(["generated.optional".into()])
        );
        assert_eq!(graph.links.len(), 3);
    }

    use super::*;
    fn fixture() -> (Vec<EntityInstance>, BakedTerrain, WaterGeometry) {
        let scene = SceneSpec {
            scene_id: "access".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 128, 128, &[0; 128 * 128 * 2])
            .unwrap();
        let mut house =
            generated_intent_entity("house".into(), "building", 64, 64, 0, 10., 10., 8.);
        house.anchors.push(SpatialAnchor {
            id: "door".into(),
            world_x: 64,
            world_z: 58,
            direction: "north".into(),
            target: Some("road".into()),
        });
        let road = generated_intent_entity("road".into(), "road", 64, 25, 0, 100., 8., 0.2);
        (
            vec![house, road],
            terrain,
            water_geometry(WaterKind::None, &[], &scene, &serde_json::json!({})),
        )
    }
    #[test]
    fn pavement_reaches_the_facade_not_the_offset_anchor() {
        let (entities, terrain, water) = fixture();
        let graph = build_access_graph(&entities, &terrain, water);
        assert_eq!(graph.connected_entities, 1);
        let link = &graph.links[0];
        assert_eq!(entities[0].anchors[0].world_z, 58);
        assert_eq!(link.path[0], point(64., 59.)); // Actual 10m building facade.
        assert!((link.distance_m.unwrap() - 30.).abs() < 1e-8);
        let surfaces = bake_access_surfaces(&graph, &terrain).unwrap();
        assert!(
            surfaces[0]
                .vertices
                .iter()
                .any(|v| (v[2] - 59.).abs() < 1e-8)
        );
    }

    #[test]
    fn persisted_pavement_is_exact_geometry_and_cannot_prove_its_own_route() {
        let (mut entities, terrain, water) = fixture();
        entities.push(generated_intent_entity(
            "obstacle".into(),
            "wall",
            64,
            43,
            0,
            12.,
            1.,
            3.,
        ));
        let graph = build_access_graph(&entities, &terrain, water);
        assert_eq!(graph.connected_entities, 1);
        assert!(
            graph.links[0].path.len() > 2,
            "fixture must actually route around a wall"
        );
        let surfaces = bake_access_surfaces(&graph, &terrain).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert!(!surfaces[0].triangles.is_empty());
        assert!(
            surfaces[0]
                .vertices
                .iter()
                .any(|v| v[0].fract().abs() > 1e-6 || v[2].fract().abs() > 1e-6)
        );
        assert!(
            surfaces[0]
                .vertices
                .iter()
                .all(|v| (v[1] - 0.04).abs() < 1e-9)
        );
        for tri in &surfaces[0].triangles {
            let [a, b, c] = tri.map(|i| surfaces[0].vertices[i as usize]);
            assert!(
                (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]) > 0.,
                "pavement winding must face up"
            );
        }
        let decoded: Vec<AccessSurface> =
            serde_json::from_slice(&serde_json::to_vec(&surfaces).unwrap()).unwrap();
        validate_access_surfaces(&graph, &terrain, &decoded).unwrap();
        let mut corrupted = surfaces.clone();
        corrupted[0].vertices[0][1] += 0.25;
        assert!(validate_access_surfaces(&graph, &terrain, &corrupted).is_err());
        assert!(validate_access_surfaces(&graph, &terrain, &[]).is_err());
        // Removing the only real road invalidates existing pavement. It is not
        // injected as a new road target to make the scene validate itself.
        entities.retain(|e| e.kind != "road");
        let disconnected = build_access_graph(&entities, &terrain, water);
        assert_eq!(disconnected.connected_entities, 0);
        assert!(
            bake_access_surfaces(&disconnected, &terrain)
                .unwrap()
                .is_empty()
        );
        assert!(validate_access_surfaces(&disconnected, &terrain, &surfaces).is_err());
    }

    #[test]
    fn pavement_rejects_incomplete_terrain_and_oversized_route_before_allocating() {
        let (entities, terrain, water) = fixture();
        let mut graph = build_access_graph(&entities, &terrain, water);
        graph.links[0].path[1].x = 1e30;
        assert!(bake_access_surfaces(&graph, &terrain).is_err());
        let s = SceneSpec {
            scene_id: "empty".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let empty = BakedTerrain::new(&s).unwrap();
        assert!(bake_access_surfaces(&AccessGraph::default(), &empty).is_err());
    }

    #[test]
    fn final_road_baseline_uses_metres_without_moving_authored_infrastructure() {
        let (mut entities, terrain, _) = fixture();
        entities[1].entity_id = "generated.road".into();
        entities[1].world_y = 100;
        rebase_generated_transport_to_baked_surface(&mut entities, &[], &terrain, None);
        assert_eq!(entities[1].world_y, 0);
        entities[1].entity_id = "authored.road".into();
        entities[1].world_y = 100;
        rebase_generated_transport_to_baked_surface(&mut entities, &[], &terrain, None);
        assert_eq!(entities[1].world_y, 100);
    }

    #[test]
    fn generation_selects_another_facade_but_never_rewrites_authored_door() {
        let (mut entities, terrain, water) = fixture();
        entities[0].entity_id = "generated.house".into();
        entities[0].anchors[0].id = "front".into();
        // Blocking strip directly outside the original facade, alternate doors
        // remain accessible by routes around the owner's footprint.
        entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            57,
            0,
            14.,
            1.,
            3.,
        ));
        let original = entities[0].anchors.clone();
        let graph = place_generated_entrances(&mut entities, &[], &terrain, water);
        assert_eq!(graph.connected_entities, 1);
        assert_ne!(
            (
                entities[0].anchors[0].world_x,
                entities[0].anchors[0].world_z
            ),
            (original[0].world_x, original[0].world_z)
        );
        assert_eq!(
            build_access_graph(&entities, &terrain, water).connected_entities,
            1
        );
        entities[0].anchors = original.clone();
        let landmark: LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId":"generated.house","name":"Authored","type":"building","purpose":"civic",
            "description":"Fixed doorway","sceneId":"access","worldX":64,"worldZ":64,
            "widthM":10,"depthM":10,"heightM":8,"assetId":"house"
        }))
        .unwrap();
        let graph = place_generated_entrances(&mut entities, &[landmark], &terrain, water);
        assert_eq!(graph.disconnected_entities, 1);
        assert_eq!(
            (
                entities[0].anchors[0].world_x,
                entities[0].anchors[0].world_z
            ),
            (original[0].world_x, original[0].world_z)
        );
        assert!(graph.links[0].required);
    }

    #[test]
    fn fixed_frontage_entrance_is_never_reassigned_even_when_another_facade_is_reachable() {
        let (mut entities, terrain, water) = fixture();
        entities[0].entity_id = "generated.frontage.fixed".into();
        entities[0].anchors[0].id = "front".into();
        entities[0].grounding.frontage_road_point = Some([64, 25]);
        // This blocks the fixed north-side doorway. The generic generated
        // building test above proves that another facade remains reachable.
        entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            57,
            0,
            14.,
            1.,
            3.,
        ));
        let original = entities[0].anchors.clone();
        let graph = place_generated_entrances(&mut entities, &[], &terrain, water);
        assert_eq!(
            (
                entities[0].anchors[0].world_x,
                entities[0].anchors[0].world_z
            ),
            (original[0].world_x, original[0].world_z),
        );
        assert_eq!(graph.connected_entities, 0);
        assert_eq!(graph.disconnected_entities, 1);
        assert_eq!(
            serde_json::to_value(&graph).unwrap(),
            serde_json::to_value(build_access_graph(&entities, &terrain, water)).unwrap(),
            "the returned graph must describe the same final fixed entity state"
        );
    }
    // A-T1-R1: rotate the entire local layout, not just the building yaw.
    // Expected cardinal vectors are explicit, independent of access::world().
    fn fixed_frontage_cardinal_fixture(
        yaw: i32,
        origin: (i32, i32),
        support_mm: i32,
        blocked: bool,
    ) -> (Vec<EntityInstance>, BakedTerrain, WaterGeometry) {
        let (nx, nz, direction) = match yaw {
            0 => (0, -1, "north"),
            90 => (-1, 0, "west"),
            180 => (0, 1, "south"),
            270 => (1, 0, "east"),
            _ => panic!("fixture requires a cardinal yaw"),
        };
        let scene = SceneSpec {
            scene_id: "fixed-frontage-cardinal".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: origin.0,
            origin_z: origin.1,
            seed_offset: 0,
        };
        let (x, z) = (origin.0 + 64, origin.1 + 64);
        let road_point = [x + nx * 39, z + nz * 39];
        let site = super::super::road_network::RoadsideSite {
            program: None,
            purpose: None,
            id: "generated.frontage.cardinal-fixed".into(),
            road_id: "road.cardinal".into(),
            world_x: x,
            world_z: z,
            // A non-square footprint makes a missing rotation observable.
            width_m: 14,
            depth_m: 10,
            support_level_m: support_mm.div_euclid(1000),
            parcel_insets: None,
            support_level_mm: Some(support_mm),
            frontage_road_point: Some(road_point),
            bounded_cut_mm: None,
            earthwork_blend_m: None,
            rotation_y_deg: yaw as f32,
            natural_min_m: support_mm as f32 / 1000.,
            natural_max_m: support_mm as f32 / 1000.,
        };
        let house = site.entity();
        assert_eq!((house.world_x, house.world_z), (x, z));
        assert_eq!(house.rotation_y_deg, yaw as f32);
        assert_eq!(house.world_y, support_mm.div_euclid(1000));
        assert_eq!(
            house.grounding.bottom_offset_m,
            support_mm.rem_euclid(1000) as f32 / 1000.
        );
        assert!((f64::from(house.ground_height_m()) - f64::from(support_mm) / 1000.).abs() < 1e-6);
        assert_eq!(house.grounding.frontage_road_point, Some(road_point));
        assert_eq!(house.anchors.len(), 1);
        let anchor = &house.anchors[0];
        assert_eq!(anchor.id, "front");
        assert_eq!((anchor.world_x, anchor.world_z), (x + nx * 6, z + nz * 6));
        assert_eq!(anchor.direction, direction);
        assert_eq!(anchor.target.as_deref(), Some("road"));

        let mut road = generated_intent_entity(
            site.road_id.clone(), "road", road_point[0], road_point[1],
            house.world_y, 100., 8., 0.2,
        );
        road.rotation_y_deg = yaw as f32;
        road.grounding.bottom_offset_m = house.grounding.bottom_offset_m;
        let mut entities = vec![house, road];
        if blocked {
            let mut wall = generated_intent_entity(
                "wall.cardinal".into(), "wall", x + nx * 7, z + nz * 7,
                support_mm.div_euclid(1000), 18., 1., 3.,
            );
            wall.rotation_y_deg = yaw as f32;
            entities.push(wall);
        }
        let payload = encode_millimetres(
            128, 128, vec![f64::from(support_mm) / 1000.; 128 * 128],
        ).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain.insert_chunk(origin.0, origin.1, 128, 128, &payload).unwrap();
        assert_eq!(terrain.encoding(), Some(HeightEncoding::Millimetre));
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::json!({}));
        (entities, terrain, water)
    }

    fn assert_fixed_frontage_final_graph(
        entities: &[EntityInstance],
        graph: &AccessGraph,
        terrain: &BakedTerrain,
        water: WaterGeometry,
        case: &str,
    ) {
        // Reconstruct from final serialized entities, never from saved links.
        let final_entities: Vec<EntityInstance> =
            serde_json::from_slice(&serde_json::to_vec(entities).unwrap()).unwrap();
        let rebuilt = build_access_graph(&final_entities, terrain, water);
        assert_eq!(
            serde_json::to_value(graph).unwrap(),
            serde_json::to_value(&rebuilt).unwrap(),
            "{case}: returned graph must describe the final entity state"
        );
        let surfaces = bake_access_surfaces(graph, terrain).unwrap();
        validate_access_surfaces(&rebuilt, terrain, &surfaces).unwrap();
        assert_eq!(
            serde_json::to_value(&surfaces).unwrap(),
            serde_json::to_value(bake_access_surfaces(&rebuilt, terrain).unwrap()).unwrap(),
            "{case}: access pavement must derive from that same final graph and raster"
        );
    }

    #[test]
    fn fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_reachable_contract() {
        for yaw in [0, 90, 180, 270] {
            for origin in [(-256, 384), (512, -384)] {
                for mm in [-1354, 1354] {
                    let case = format!("yaw={yaw} origin={origin:?} mm={mm}");
                    let (mut entities, terrain, water) =
                        fixed_frontage_cardinal_fixture(yaw, origin, mm, false);
                    let before = serde_json::to_value(&entities).unwrap();
                    let terrain_hash = terrain.content_hash().unwrap();
                    let graph = place_generated_entrances(&mut entities, &[], &terrain, water);
                    assert_eq!(serde_json::to_value(&entities).unwrap(), before, "{case}");
                    assert_eq!((graph.connected_entities, graph.disconnected_entities), (1, 0), "{case}");
                    assert_eq!(graph.links.len(), 1, "{case}");
                    let link = &graph.links[0];
                    assert!(link.connected && link.required, "{case}");
                    assert_eq!(link.status, "walkable_route", "{case}");
                    assert_eq!(link.entity_id, entities[0].entity_id, "{case}");
                    assert_eq!(link.anchor_id, "front", "{case}");
                    assert_eq!(link.road_id.as_deref(), Some("road.cardinal"), "{case}");
                    let (nx, nz) = match yaw {
                        0 => (0., -1.), 90 => (-1., 0.),
                        180 => (0., 1.), 270 => (1., 0.), _ => unreachable!(),
                    };
                    let facade = point(f64::from(origin.0 + 64) + nx * 5., f64::from(origin.1 + 64) + nz * 5.);
                    let road_edge = point(f64::from(origin.0 + 64) + nx * 35., f64::from(origin.1 + 64) + nz * 35.);
                    assert!(distance(link.path[0], facade) < 1e-8, "{case}");
                    assert!(distance(*link.path.last().unwrap(), road_edge) < 1e-8, "{case}");
                    assert!((link.distance_m.unwrap() - 30.).abs() < 1e-8, "{case}");
                    assert_fixed_frontage_final_graph(&entities, &graph, &terrain, water, &case);
                    // This covers only the complete 128m fixture raster, not global natural/protected land.
                    assert_eq!(terrain.content_hash().unwrap(), terrain_hash, "{case}");
                }
            }
        }
    }

    #[test]
    fn fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_blocked_obligation() {
        for yaw in [0, 90, 180, 270] {
            for origin in [(-256, 384), (512, -384)] {
                for mm in [-1354, 1354] {
                    let case = format!("yaw={yaw} origin={origin:?} mm={mm}");
                    let (mut entities, terrain, water) =
                        fixed_frontage_cardinal_fixture(yaw, origin, mm, true);
                    let before = serde_json::to_value(&entities).unwrap();
                    let terrain_hash = terrain.content_hash().unwrap();

                    // Same geometry and entry point: remove only the fixed-contract marker.
                    // This positive control must actually find a different facade for EVERY case.
                    let mut generic = entities.clone();
                    generic[0].grounding.frontage_road_point = None;
                    let generic_before = serde_json::to_value(&generic).unwrap();
                    let generic_graph = place_generated_entrances(&mut generic, &[], &terrain, water);
                    assert_eq!((generic_graph.connected_entities, generic_graph.disconnected_entities), (1, 0), "{case}: alternate facade must be reachable");
                    assert_ne!(
                        (generic[0].anchors[0].world_x, generic[0].anchors[0].world_z),
                        (entities[0].anchors[0].world_x, entities[0].anchors[0].world_z),
                        "{case}: generic control must really exercise fallback"
                    );
                    let mut generic_after = serde_json::to_value(&generic).unwrap();
                    generic_after[0]["anchors"] = generic_before[0]["anchors"].clone();
                    assert_eq!(generic_after, generic_before, "{case}: fallback changes only anchors");
                    assert_fixed_frontage_final_graph(&generic, &generic_graph, &terrain, water, &case);

                    let graph = place_generated_entrances(&mut entities, &[], &terrain, water);
                    assert_eq!(entities.len(), 3, "{case}: keep house, road and blocking wall");
                    assert_eq!(serde_json::to_value(&entities).unwrap(), before, "{case}: keep anchors, position, yaw, roadPoint and all grounding fields");
                    assert_eq!((graph.connected_entities, graph.disconnected_entities), (0, 1), "{case}");
                    assert_eq!(graph.links.len(), 1, "{case}: keep the failed obligation");
                    let link = &graph.links[0];
                    assert!(!link.connected && link.required, "{case}");
                    assert_eq!(link.entity_id, entities[0].entity_id, "{case}");
                    assert_eq!(link.anchor_id, "front", "{case}");
                    assert_eq!(link.status, "entrance_clearance_blocked", "{case}: not an arbitrary routing failure");
                    assert!(link.road_id.is_none() && link.distance_m.is_none() && link.path.is_empty(), "{case}");
                    assert_fixed_frontage_final_graph(&entities, &graph, &terrain, water, &case);
                    assert!(bake_access_surfaces(&graph, &terrain).unwrap().is_empty(), "{case}");

                    // A stale graph for a substituted, reachable doorway is observably different.
                    let mut changed_door = entities.clone();
                    changed_door[0].anchors = generic[0].anchors.clone();
                    let changed_graph = build_access_graph(&changed_door, &terrain, water);
                    assert_eq!(changed_graph.connected_entities, 1, "{case}");
                    assert_ne!(serde_json::to_value(&graph).unwrap(), serde_json::to_value(&changed_graph).unwrap(), "{case}: reconstruction must notice a changed doorway");
                    assert_eq!(terrain.content_hash().unwrap(), terrain_hash, "{case}");
                }
            }
        }
    }

    #[test]
    fn two_entrances_do_not_double_count_one_building() {
        let (mut entities, terrain, water) = fixture();
        let mut door = entities[0].anchors[0].clone();
        door.id = "second".into();
        entities[0].anchors.push(door);
        let graph = build_access_graph(&entities, &terrain, water);
        assert_eq!(graph.links.len(), 2);
        assert_eq!(graph.connected_entities, 1);
    }

    #[test]
    fn route_is_serialized_finite_and_counts_buildings_not_anchors() {
        let (entities, terrain, water) = fixture();
        let graph = build_access_graph(&entities, &terrain, water);
        assert_eq!(graph.connected_entities, 1);
        assert_eq!(graph.disconnected_entities, 0);
        assert!(graph.links[0].path.len() >= 2);
        assert!(graph.links[0].distance_m.unwrap() > 20.);
        let json = serde_json::to_string(&graph).unwrap();
        let restored: AccessGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.links[0].path, graph.links[0].path);
    }
    #[test]
    fn no_road_or_missing_anchor_is_not_connected_and_round_trips() {
        let (mut entities, terrain, water) = fixture();
        entities.pop();
        let graph = build_access_graph(&entities, &terrain, water);
        assert_eq!(graph.disconnected_entities, 1);
        assert!(graph.links[0].distance_m.is_none());
        let _: AccessGraph = serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        entities[0].anchors.clear();
        assert_eq!(
            build_access_graph(&entities, &terrain, water).links[0].status,
            "missing_entrance"
        );
    }
    #[test]
    fn stale_authored_anchor_is_rejected_not_deferred() {
        let (mut entities, terrain, water) = fixture();
        entities[0].entity_id = "landmark.city-hall".into();
        entities[0].anchors[0].world_z = 25;
        let g = build_access_graph(&entities, &terrain, water);
        assert_eq!(g.disconnected_entities, 1);
        assert!(g.links[0].required);
        assert_eq!(g.links[0].status, "anchor_not_on_building_boundary");
    }
    #[test]
    fn thin_wall_blocks_route_and_tree_blocks_door() {
        let (mut entities, terrain, water) = fixture();
        entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            43,
            0,
            128.,
            0.1,
            2.,
        ));
        assert_eq!(
            build_access_graph(&entities, &terrain, water).disconnected_entities,
            1
        );
        entities.pop();
        entities.push(generated_intent_entity(
            "tree".into(),
            "tree",
            64,
            57,
            0,
            2.,
            2.,
            8.,
        ));
        assert_eq!(
            build_access_graph(&entities, &terrain, water).disconnected_entities,
            1
        );
    }
    #[test]
    fn search_routes_around_wall_instead_of_assuming_line_of_sight() {
        let (mut entities, terrain, water) = fixture();
        entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            43,
            0,
            10.,
            0.1,
            2.,
        ));
        let g = build_access_graph(&entities, &terrain, water);
        assert_eq!(g.connected_entities, 1);
        assert!(g.links[0].path.iter().any(|p| (p.x - 64.).abs() > 5.));
        let obstacles: Vec<_> = entities.iter().filter(|e| obstacle(&e.kind)).collect();
        for segment in g.links[0].path[1..].windows(2) {
            assert!(segment_clear(
                segment[0], segment[1], &obstacles, &terrain, water
            ));
        }
    }
    #[test]
    fn river_between_close_building_and_road_is_not_connected() {
        let (entities, terrain, mut water) = fixture();
        water.kind = WaterKind::River;
        water.center_x = 64.;
        water.center_z = 43.;
        water.half_width = 128.;
        water.half_depth = 0.1;
        assert_eq!(
            build_access_graph(&entities, &terrain, water).disconnected_entities,
            1
        );
    }
    #[test]
    fn steep_final_raster_and_incomplete_terrain_are_not_walkable() {
        let (entities, _, water) = fixture();
        let scene = SceneSpec {
            scene_id: "access".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        assert_eq!(
            build_access_graph(&entities, &terrain, water).links[0].status,
            "incomplete_terrain"
        );
        let mut bytes = vec![];
        for z in 0..128 {
            for _x in 0..128 {
                bytes.extend_from_slice(&(if z < 44 { 80_i16 } else { 0_i16 }).to_le_bytes());
            }
        }
        terrain.insert_chunk(0, 0, 128, 128, &bytes).unwrap();
        assert_eq!(
            build_access_graph(&entities, &terrain, water).disconnected_entities,
            1
        );
    }
    #[test]
    fn rotated_obstacle_check_uses_actual_rectangle_not_aabb() {
        let mut wall = generated_intent_entity("wall".into(), "wall", 50, 50, 0, 30., 1., 2.);
        wall.rotation_y_deg = 45.;
        assert!(!crosses(&wall, point(60., 58.), point(60., 62.)));
        assert!(crosses(&wall, point(60., 38.), point(60., 42.)));
    }

    // ---- B01-G12: passive observer synthetic differential --------------------

    fn g12_obstacles(entities: &[EntityInstance]) -> Vec<&EntityInstance> {
        entities.iter().filter(|e| obstacle(&e.kind)).collect()
    }

    fn g12_target_link(graph: &AccessGraph, target: &str) -> AccessLink {
        graph
            .links
            .iter()
            .find(|link| link.entity_id == target)
            .expect("the target link")
            .clone()
    }

    fn g12_scene() -> SceneSpec {
        SceneSpec {
            scene_id: "access".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    /// B01-G12 item B: with and without the observer the production graph must be
    /// identical field by field on the same input, and the observer must leave no
    /// residue for a different target on the same thread.
    #[test]
    fn g12_observer_result_equivalence_and_no_residue() {
        let fixtures: Vec<(
            &str,
            Box<dyn Fn() -> (Vec<EntityInstance>, BakedTerrain, WaterGeometry)>,
        )> = vec![
            ("passable", Box::new(fixture)),
            (
                "wall_detour",
                Box::new(|| {
                    let (mut entities, terrain, water) = fixture();
                    entities.push(generated_intent_entity(
                        "wall".into(),
                        "wall",
                        64,
                        43,
                        0,
                        10.,
                        0.1,
                        2.,
                    ));
                    (entities, terrain, water)
                }),
            ),
            (
                "blocked_entrance",
                Box::new(|| {
                    let (mut entities, terrain, water) = fixture();
                    entities.push(generated_intent_entity(
                        "tree".into(),
                        "tree",
                        64,
                        57,
                        0,
                        2.,
                        2.,
                        8.,
                    ));
                    (entities, terrain, water)
                }),
            ),
            (
                "river",
                Box::new(|| {
                    let (entities, terrain, mut water) = fixture();
                    water.kind = WaterKind::River;
                    water.center_x = 64.;
                    water.center_z = 43.;
                    water.half_width = 128.;
                    water.half_depth = 0.1;
                    (entities, terrain, water)
                }),
            ),
        ];
        for (name, make) in fixtures {
            let (entities, terrain, water) = make();
            let plain = build_access_graph(&entities, &terrain, water);
            let target = entities[0].entity_id.clone();
            let (observed, trace) =
                observation::observe(&target, || build_access_graph(&entities, &terrain, water));
            assert_eq!(
                serde_json::to_value(&plain).unwrap(),
                serde_json::to_value(&observed).unwrap(),
                "{name}: the observer must not change the production graph"
            );
            let link = g12_target_link(&observed, &target);
            assert!(
                trace.links.iter().any(|record| {
                    record["status"] == serde_json::json!(link.status)
                        && record["pathPoints"] == serde_json::json!(link.path.len())
                        && record["connected"] == serde_json::json!(link.connected)
                }),
                "{name}: the observer must record the target link"
            );
            assert!(
                trace
                    .links
                    .iter()
                    .all(|record| record["entityId"] == serde_json::json!(target)),
                "{name}: only the target link may be recorded"
            );
            let (_, other) = observation::observe("g12.someone.else", || {
                build_access_graph(&entities, &terrain, water)
            });
            assert!(
                other.links.is_empty() && other.goals.is_empty() && other.counters.is_empty(),
                "{name}: a different target must collect nothing"
            );
        }
    }

    /// B01-G12 item B: the observer's classifier must agree with the production
    /// predicate on every synthetic segment, and the exercised classes must include
    /// a pass, an obstacle, water, a missing sample and a grade.
    #[test]
    fn g12_differential_classifier_matches_production_predicate() {
        let (flat_entities, flat_terrain, flat_water) = fixture();
        let (mut wall_entities, _, _) = fixture();
        wall_entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            43,
            0,
            10.,
            0.1,
            2.,
        ));
        let (river_entities, _, mut river) = fixture();
        river.kind = WaterKind::River;
        river.center_x = 64.;
        river.center_z = 43.;
        river.half_width = 128.;
        river.half_depth = 0.1;
        let scene = g12_scene();
        let mut steep_bytes = vec![];
        for z in 0..128 {
            for _x in 0..128 {
                steep_bytes
                    .extend_from_slice(&(if z < 44 { 80_i16 } else { 0_i16 }).to_le_bytes());
            }
        }
        let mut steep = BakedTerrain::new(&scene).unwrap();
        steep.insert_chunk(0, 0, 128, 128, &steep_bytes).unwrap();
        let incomplete = BakedTerrain::new(&scene).unwrap();
        let segments = [
            (point(60., 50.), point(68., 50.)),
            (point(64., 58.), point(64., 20.)),
            (point(0., 0.), point(64., 64.)),
        ];
        let mut classes: BTreeSet<String> = BTreeSet::new();
        let cases: Vec<(&str, &[EntityInstance], &BakedTerrain, WaterGeometry)> = vec![
            ("flat", &flat_entities, &flat_terrain, flat_water),
            ("wall", &wall_entities, &flat_terrain, flat_water),
            ("river", &river_entities, &flat_terrain, river),
            ("steep", &flat_entities, &steep, flat_water),
            ("incomplete", &flat_entities, &incomplete, flat_water),
        ];
        for (name, entities, terrain, water) in cases {
            let obstacles = g12_obstacles(entities);
            for (a, b) in segments {
                let production = segment_clear(a, b, &obstacles, terrain, water);
                let classified = observation::classify_segment(a, b, &obstacles, terrain, water);
                assert_eq!(
                    production,
                    classified.is_none(),
                    "{name}: {a:?} -> {b:?} must be classified as the production predicate decided"
                );
                classes.insert(
                    classified
                        .and_then(|value| {
                            value["class"].as_str().map(std::string::ToString::to_string)
                        })
                        .unwrap_or_else(|| "pass".to_string()),
                );
            }
        }
        for required in ["pass", "obstacle", "water", "missing", "grade"] {
            assert!(
                classes.contains(required),
                "the differential set must exercise {required}: {classes:?}"
            );
        }
    }

    /// B01-G12 item B: the observer must name the branch that actually refused, using
    /// the production status for the link itself, and must name the search
    /// termination branch.
    #[test]
    fn g12_observer_names_entrance_obstacle_water_and_termination() {
        let scene = g12_scene();
        // entrance step: the paving in front of the door sits a metre too high
        let (entities, _, water) = fixture();
        let mut stepped_bytes = vec![];
        for z in 0..128 {
            for _x in 0..128 {
                stepped_bytes
                    .extend_from_slice(&(if z >= 57 { 4_i16 } else { 0_i16 }).to_le_bytes());
            }
        }
        let mut stepped = BakedTerrain::new(&scene).unwrap();
        stepped.insert_chunk(0, 0, 128, 128, &stepped_bytes).unwrap();
        let target = entities[0].entity_id.clone();
        let (graph, trace) =
            observation::observe(&target, || build_access_graph(&entities, &stepped, water));
        assert_eq!(
            g12_target_link(&graph, &target).status,
            "entrance_step_requires_engineering"
        );
        assert!(trace
            .entrance
            .iter()
            .any(|row| row["stepWithin"] == serde_json::json!(false)));
        // the entrance clearance field must mirror the production branch: true when
        // the pavement reaches the door, false when a real obstacle blocks it
        let (plain_entities, plain_terrain, plain_water) = fixture();
        let plain_target = plain_entities[0].entity_id.clone();
        let (plain_graph, plain_trace) = observation::observe(&plain_target, || {
            build_access_graph(&plain_entities, &plain_terrain, plain_water)
        });
        assert!(plain_graph.links[0].connected);
        assert_eq!(
            plain_trace.entrance[0]["entranceSegmentClear"],
            serde_json::json!(true)
        );
        assert!(
            plain_trace
                .counters
                .get("routes.started")
                .copied()
                .unwrap_or(0)
                >= 1,
            "the route stage must be counted: {:?}",
            plain_trace.counters
        );
        let (mut door_entities, door_terrain, door_water) = fixture();
        door_entities.push(generated_intent_entity(
            "tree".into(),
            "tree",
            64,
            57,
            0,
            2.,
            2.,
            8.,
        ));
        let door_target = door_entities[0].entity_id.clone();
        let (door_graph, door_trace) = observation::observe(&door_target, || {
            build_access_graph(&door_entities, &door_terrain, door_water)
        });
        assert_eq!(
            g12_target_link(&door_graph, &door_target).status,
            "entrance_clearance_blocked"
        );
        assert_eq!(
            door_trace.entrance[0]["entranceSegmentClear"],
            serde_json::json!(false)
        );
        // incomplete terrain can never be walkable
        let empty = BakedTerrain::new(&scene).unwrap();
        let (graph, _) =
            observation::observe(&target, || build_access_graph(&entities, &empty, water));
        assert_eq!(
            g12_target_link(&graph, &target).status,
            "incomplete_terrain"
        );
        // obstacle: the direct segment is refused by the real wall entity
        let (mut wall_entities, flat, flat_water) = fixture();
        wall_entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            43,
            0,
            10.,
            0.1,
            2.,
        ));
        let target = wall_entities[0].entity_id.clone();
        let (graph, trace) = observation::observe(&target, || {
            build_access_graph(&wall_entities, &flat, flat_water)
        });
        assert_eq!(graph.connected_entities, 1);
        let obstacle_samples = trace
            .samples
            .get("segment_reject.obstacle")
            .expect("the obstacle reason must be named");
        assert!(obstacle_samples
            .iter()
            .any(|row| row["entityId"] == serde_json::json!("wall")));
        assert!(obstacle_samples.len() <= 8);
        // water: the river fixture must be named as water, not as something else
        let (river_entities, _, mut river) = fixture();
        river.kind = WaterKind::River;
        river.center_x = 64.;
        river.center_z = 43.;
        river.half_width = 128.;
        river.half_depth = 0.1;
        let target = river_entities[0].entity_id.clone();
        let (_graph, trace) = observation::observe(&target, || {
            build_access_graph(&river_entities, &flat, river)
        });
        assert!(trace.samples.contains_key("segment_reject.water"));
        // queue exhaustion: a wall spanning the scene leaves no detour
        let (mut blocked_entities, blocked_terrain, blocked_water) = fixture();
        blocked_entities.push(generated_intent_entity(
            "wall".into(),
            "wall",
            64,
            43,
            0,
            128.,
            0.1,
            2.,
        ));
        let target = blocked_entities[0].entity_id.clone();
        let (graph, trace) = observation::observe(&target, || {
            build_access_graph(&blocked_entities, &blocked_terrain, blocked_water)
        });
        assert!(!g12_target_link(&graph, &target).connected);
        assert!(
            trace
                .counters
                .get("termination.queue_exhausted")
                .copied()
                .unwrap_or(0)
                >= 1,
            "queue exhaustion must be named: {:?}",
            trace.counters
        );
        // expansion limit: a sealed 200m pocket with a road ring just outside keeps
        // the heuristic small, so the bounded search expands past its cap
        let big_scene = SceneSpec {
            scene_id: "access-big".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut big = BakedTerrain::new(&big_scene).unwrap();
        big.insert_chunk(0, 0, 512, 512, &[0; 512 * 512 * 2])
            .unwrap();
        let mut pocket_entities = vec![];
        let mut house = generated_intent_entity("house.big".into(), "building", 256, 256, 0, 10., 10., 8.);
        house.anchors.push(SpatialAnchor {
            id: "door".into(),
            world_x: 256,
            world_z: 250,
            direction: "north".into(),
            target: Some("road".into()),
        });
        pocket_entities.push(house);
        for (id, x, z, w, d) in [
            ("wall.w".to_string(), 156, 256, 1.0, 220.0),
            ("wall.e".to_string(), 356, 256, 1.0, 220.0),
            ("wall.n".to_string(), 256, 156, 220.0, 1.0),
            ("wall.s".to_string(), 256, 356, 220.0, 1.0),
            ("road.w".to_string(), 152, 256, 4.0, 260.0),
            ("road.e".to_string(), 360, 256, 4.0, 260.0),
            ("road.n".to_string(), 256, 152, 260.0, 4.0),
            ("road.s".to_string(), 256, 360, 260.0, 4.0),
        ] {
            let kind = if id.starts_with("wall") { "wall" } else { "road" };
            pocket_entities.push(generated_intent_entity(id, kind, x, z, 0, w, d, 2.));
        }
        let target = "house.big".to_string();
        let (graph, trace) = observation::observe(&target, || {
            build_access_graph(&pocket_entities, &big, flat_water)
        });
        assert!(!g12_target_link(&graph, &target).connected);
        let expansion_limit = trace
            .counters
            .get("termination.expansion_limit")
            .copied()
            .unwrap_or(0);
        let queue_exhausted = trace
            .counters
            .get("termination.queue_exhausted")
            .copied()
            .unwrap_or(0);
        assert!(
            expansion_limit >= 1 || queue_exhausted >= 1,
            "the bounded search must end through a named branch: routes={:?} counters={:?}",
            trace.routes,
            trace.counters
        );
        let termination = serde_json::json!({
            "expansionLimitHits": expansion_limit,
            "queueExhaustedHits": queue_exhausted,
            "routes": trace.routes,
            "counters": trace.counters,
        });
        println!("{}", serde_json::to_string_pretty(&termination).unwrap());
    }

    /// B01-G13 item A: a rejection record must expose the whole segment and the
    /// actual failing sample pair separately, and re-interpolating the recorded
    /// sample coordinates must reproduce the recorded heights. The fixture is built
    /// so the first failure happens on the -0.75 lateral band and after the first
    /// sample step, so a centre/first-step-only test could not pass by accident.
    #[test]
    fn g13_segment_record_keeps_both_coordinate_pairs() {
        let scene = SceneSpec {
            scene_id: "g13".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut bytes = vec![];
        for z in 0..128 {
            for x in 0..128 {
                let value: i16 = if (62..66).contains(&z) && x < 64 { 8 } else { 0 };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain.insert_chunk(0, 0, 128, 128, &bytes).unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::json!({}));
        let obstacles: Vec<&EntityInstance> = vec![];
        let a = point(64.0, 58.0);
        let b = point(64.0, 70.0);
        assert!(!segment_clear(a, b, &obstacles, &terrain, water));
        let record = observation::classify_segment(a, b, &obstacles, &terrain, water)
            .expect("a named rejection");
        assert_eq!(record["class"], serde_json::json!("grade"));
        assert_eq!(record["segmentFrom"], serde_json::json!([64.0, 58.0]));
        assert_eq!(record["segmentTo"], serde_json::json!([64.0, 70.0]));
        assert_ne!(
            record["sampleTo"], record["segmentTo"],
            "the failing sample must not be reported as the segment end"
        );
        assert_ne!(record["sampleFrom"], record["segmentFrom"]);
        assert!(
            record["sampleIndexTo"].as_i64().unwrap() > 1,
            "the first failure must not sit on the first sample step"
        );
        // the production normal turns "lateral +0.75" into the x = 63.25 band here
        assert_eq!(record["lateralM"].as_f64().unwrap(), 0.75);
        let at = |value: &serde_json::Value| {
            point(
                value[0].as_f64().unwrap(),
                value[1].as_f64().unwrap(),
            )
        };
        let h_from = height(&terrain, at(&record["sampleFrom"])).unwrap();
        let h_to = height(&terrain, at(&record["sampleTo"])).unwrap();
        assert!((h_from - record["heightFromM"].as_f64().unwrap()).abs() < 1e-9);
        assert!((h_to - record["heightToM"].as_f64().unwrap()).abs() < 1e-9);
        assert!(record["deltaM"].as_f64().unwrap() > record["limitM"].as_f64().unwrap());
        assert_eq!(record["sampleIndex"].as_i64().unwrap() + 1, record["sampleIndexTo"].as_i64().unwrap());
        let profile = observation::sample_profile(&terrain, a, b);
        let lines = profile["lines"].as_array().unwrap();
        let left = lines
            .iter()
            .find(|line| line["lateralM"] == serde_json::json!(0.75))
            .expect("the failing band");
        assert!(left["firstFailure"]["indexTo"].as_i64().unwrap() > 1);
        let other = lines
            .iter()
            .find(|line| line["lateralM"] == serde_json::json!(-0.75))
            .expect("the other band");
        assert!(
            other["firstFailure"].is_null(),
            "the other band must stay clear in this fixture"
        );
        let payload = serde_json::json!({"record": record, "profile": profile});
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    }
    /// B01-G19 regression for the extraction that let the engineering-connections
    /// module reuse the production terrain predicate: with no obstacles and no water
    /// the two entry points must agree exactly, including the degenerate segment.
    #[test]
    fn g19_terrain_predicate_extraction_is_semantically_identical() {
        let (_, terrain, mut water) = fixture();
        water.kind = WaterKind::None;
        for (a, b) in [
            (point(60., 50.), point(68., 50.)),
            (point(64., 58.), point(64., 20.)),
            (point(0., 0.), point(64., 64.)),
            (point(40., 40.), point(40., 40.)),
            (point(-5., 3.5), point(70.25, 61.75)),
        ] {
            assert_eq!(
                segment_clear(a, b, &[], &terrain, water),
                terrain_segment_clear_xy(a.x, a.z, b.x, b.z, &terrain),
                "extraction changed the verdict for {a:?} -> {b:?}"
            );
        }
    }
}
