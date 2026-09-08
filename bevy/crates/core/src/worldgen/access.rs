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
                if (h - old).abs() > 0.25 * (length / n as f64) + 1e-6 {
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
    let mut goals: Vec<_> = roads
        .iter()
        .filter_map(|road| {
            let end = closest(road, start);
            let d = distance(start, end);
            (d <= MAX_LENGTH).then_some((*road, end, d))
        })
        .collect();
    goals.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.entity_id.cmp(&b.0.entity_id)));
    for (road, end, d) in &goals {
        if segment_clear(start, *end, obstacles, terrain, water) {
            return Some((road.entity_id.clone(), vec![start, *end], *d));
        }
    }
    if goals.is_empty() {
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
        if done.len() > MAX_EXPANSIONS {
            return None;
        }
        let p = coord(x, z);
        let g = cost[&(x, z)];
        for (road, _, _) in &goals {
            let end = closest(road, p);
            let d = distance(p, end);
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
                continue;
            }
            if !segment_clear(p, q, obstacles, terrain, water) {
                continue;
            }
            cost.insert(next, ng);
            parent.insert(next, (x, z));
            queue.push(Reverse(((estimate * 1000.).round() as u64, next.0, next.1)));
        }
    }
    None
}

pub(super) fn access_for_entity(
    e: &EntityInstance,
    entities: &[EntityInstance],
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> Vec<AccessLink> {
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
        rebase_generated_transport_to_baked_surface(&mut entities, &[], &terrain);
        assert_eq!(entities[1].world_y, 0);
        entities[1].entity_id = "authored.road".into();
        entities[1].world_y = 100;
        rebase_generated_transport_to_baked_surface(&mut entities, &[], &terrain);
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
            id: "generated.frontage.cardinal-fixed".into(),
            road_id: "road.cardinal".into(),
            world_x: x,
            world_z: z,
            // A non-square footprint makes a missing rotation observable.
            width_m: 14,
            depth_m: 10,
            support_level_m: support_mm.div_euclid(1000),
            support_level_mm: Some(support_mm),
            frontage_road_point: Some(road_point),
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
}
