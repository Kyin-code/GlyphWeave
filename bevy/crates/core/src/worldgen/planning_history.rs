//! City-planning history derived from the route network and capacity settlement.
//!
//! This is the bridge between "buildings exist" and "a city evolved". It records
//! network-based centres, road widening, redevelopment, preservation and public
//! park demand. The actions are planning evidence; they do not bypass the final
//! terrain, access, water or zoning contracts.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::route_evolution::Plan as RoutePlan;
use super::settlement_evolution::Plan as SettlementPlan;
use super::{
    BakedTerrain, EntityInstance, OrientedFootprint, SceneSpec, is_building_kind,
    oriented_footprints_overlap, urban_slice,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_old_core")]
    pub old_core_radius_m: f64,
    #[serde(default = "default_park_area")]
    pub park_area_m2_per_person: f64,
    #[serde(default = "default_renewal_share")]
    pub renewal_share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Centre {
    pub id: String,
    pub role: String,
    pub world_x: i32,
    pub world_z: i32,
    pub network_degree: usize,
    pub influence: f64,
    pub source: String,
    #[serde(default)]
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    pub stage: String,
    pub kind: String,
    pub target_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub length_m: f64,
    pub area_m2: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub building_id: String,
    pub centre_distance_m: f64,
    pub centrality: f64,
    pub district: String,
    pub planning_action: String,
    pub typology: NeighbourhoodRule,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NeighbourhoodRule {
    pub typology_id: String,
    pub tenure_mix: String,
    pub parking_strategy: String,
    pub frontage_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlement_scale: String,
    pub community_profile: String,
    pub centres: Vec<Centre>,
    pub actions: Vec<Action>,
    pub assignments: Vec<Assignment>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Materialization {
    pub entities: Vec<EntityInstance>,
    pub skipped: Vec<SkippedAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedAction {
    pub action_id: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
struct Graph {
    points: Vec<[i32; 2]>,
    adjacency: Vec<Vec<(usize, u64)>>,
    distance_to_centres_mm: Vec<u64>,
}

impl Graph {
    fn from_route_plan(plan: &RoutePlan) -> Self {
        let mut points = Vec::<[i32; 2]>::new();
        let mut index = BTreeMap::<[i32; 2], usize>::new();
        let mut edges = Vec::<([i32; 2], [i32; 2], u64)>::new();
        for route in plan.routes.iter().filter(|route| route.status == "pass") {
            for pair in route.polyline.windows(2) {
                let a = [pair[0].x, pair[0].z];
                let b = [pair[1].x, pair[1].z];
                for point in [a, b] {
                    if !index.contains_key(&point) {
                        let next = points.len();
                        index.insert(point, next);
                        points.push(point);
                    }
                }
                let dx = f64::from(b[0] - a[0]);
                let dz = f64::from(b[1] - a[1]);
                edges.push((a, b, (dx.hypot(dz) * 1000.0).round() as u64));
            }
        }
        let mut adjacency = vec![Vec::<(usize, u64)>::new(); points.len()];
        for (a, b, length) in edges {
            let ai = index[&a];
            let bi = index[&b];
            adjacency[ai].push((bi, length));
            adjacency[bi].push((ai, length));
        }
        for neighbours in &mut adjacency {
            neighbours.sort_unstable();
            neighbours.dedup_by_key(|(node, _)| *node);
        }
        let mut graph = Self {
            points,
            adjacency,
            distance_to_centres_mm: Vec::new(),
        };
        let sources = plan
            .anchors
            .iter()
            .filter_map(|anchor| graph.nearest_node(anchor.x, anchor.z))
            .collect::<Vec<_>>();
        graph.distance_to_centres_mm = graph.multi_source_distances(&sources);
        graph
    }

    fn nearest_node(&self, x: i32, z: i32) -> Option<usize> {
        self.points
            .iter()
            .enumerate()
            .min_by_key(|(index, point)| {
                let dx = i64::from((*point)[0] - x);
                let dz = i64::from((*point)[1] - z);
                (dx * dx + dz * dz, *index)
            })
            .map(|(index, _)| index)
    }

    fn multi_source_distances(&self, sources: &[usize]) -> Vec<u64> {
        let mut distances = vec![u64::MAX; self.points.len()];
        let mut heap = BinaryHeap::<Reverse<(u64, usize)>>::new();
        for source in sources {
            if distances[*source] != 0 {
                distances[*source] = 0;
                heap.push(Reverse((0, *source)));
            }
        }
        while let Some(Reverse((distance, node))) = heap.pop() {
            if distance != distances[node] {
                continue;
            }
            for (next, length) in &self.adjacency[node] {
                let candidate = distance.saturating_add(*length);
                if candidate < distances[*next] {
                    distances[*next] = candidate;
                    heap.push(Reverse((candidate, *next)));
                }
            }
        }
        distances
    }

    fn building_distance_m(&self, x: i32, z: i32) -> f64 {
        let Some(node) = self.nearest_node(x, z) else {
            return f64::INFINITY;
        };
        let point = self.points[node];
        let snap = f64::from(point[0] - x).hypot(f64::from(point[1] - z));
        let network = if self.distance_to_centres_mm[node] == u64::MAX {
            f64::INFINITY
        } else {
            self.distance_to_centres_mm[node] as f64 / 1000.0
        };
        snap + network
    }

    fn reachable_node_count(&self, source: usize) -> usize {
        let mut seen = vec![false; self.points.len()];
        let mut stack = vec![source];
        seen[source] = true;
        let mut count = 0;
        while let Some(node) = stack.pop() {
            count += 1;
            for (next, _) in &self.adjacency[node] {
                if !seen[*next] {
                    seen[*next] = true;
                    stack.push(*next);
                }
            }
        }
        count
    }
}

const fn default_true() -> bool {
    true
}
const fn default_old_core() -> f64 {
    260.0
}
const fn default_park_area() -> f64 {
    8.0
}
const fn default_renewal_share() -> f64 {
    0.22
}

pub fn validate_style(style: &Value) -> Result<(), String> {
    let Some(value) = style.get("planningHistory") else {
        return Ok(());
    };
    let program: Program =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    if program.version != 1
        || !(40.0..=1200.0).contains(&program.old_core_radius_m)
        || !(0.0..=40.0).contains(&program.park_area_m2_per_person)
        || !(0.0..=1.0).contains(&program.renewal_share)
    {
        return Err("invalid planningHistory values".into());
    }
    Ok(())
}

fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("planningHistory") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn plan(
    route: &RoutePlan,
    settlement: &SettlementPlan,
    style: &Value,
) -> Result<Option<Plan>, String> {
    validate_style(style)?;
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    let graph = Graph::from_route_plan(route);
    let slice = urban_slice::compile(style)?;
    let settlement_scale = slice
        .as_ref()
        .map_or_else(|| "town".to_string(), |plan| plan.settlement_scale.clone());
    let factors = slice
        .as_ref()
        .map_or_else(Vec::new, |plan| plan.special_factors.clone());
    let focus_bias = slice.as_ref().map(|plan| &plan.focus_bias);
    let centres = derive_centres(route, &graph, &factors, &settlement_scale, focus_bias);
    let scale_distance = match settlement_scale.as_str() {
        "metropolis" | "megacity" => 1800.0,
        "city" => 1100.0,
        _ => 700.0,
    };
    let profile = settlement.community_profile.as_deref().unwrap_or("generic");

    let mut assignments = Vec::new();
    let mut actions = Vec::new();
    for building in &settlement.buildings {
        let distance = graph.building_distance_m(building.world_x, building.world_z);
        let centrality = if distance.is_finite() {
            (-distance / scale_distance).exp().clamp(0.0, 1.0)
        } else {
            0.0
        };
        let district = if distance <= program.old_core_radius_m {
            "old-core"
        } else if distance <= program.old_core_radius_m * 2.2 {
            "renewal-or-expansion"
        } else {
            "new-city-or-edge"
        };
        let planning_action = if district == "old-core"
            && building.kind == "residential_home"
            && centrality >= 0.52
        {
            "evaluate-redevelopment"
        } else if district == "old-core" && building.kind == "storefront" {
            "preserve-street-frontage"
        } else if district == "renewal-or-expansion" {
            "connect-and-infilling"
        } else {
            "new-growth"
        };
        assignments.push(Assignment {
            building_id: building.id.clone(),
            centre_distance_m: distance,
            centrality,
            district: district.into(),
            planning_action: planning_action.into(),
            typology: neighbourhood_rule(profile, district),
        });
    }

    actions.extend(centre_actions(&centres, &settlement_scale, route));
    actions.extend(road_actions(route, &settlement_scale, focus_bias));
    let park_target = (f64::from(
        settlement
            .delivered
            .households
            .saturating_add(settlement.delivered.jobs / 2),
    ) * 3.0
        * program.park_area_m2_per_person
        * focus_bias.map_or(1.0, |bias| f64::from(bias.park_bias)))
    .min(250_000.0);
    if park_target > 1.0 {
        let focus_anchor = focus_bias
            .filter(|bias| bias.park_bias > 1.05)
            .and_then(|_| {
                centres
                    .iter()
                    .find(|centre| matches!(centre.role.as_str(), "tourism" | "port"))
            });
        let anchor = focus_anchor
            .or_else(|| centres.get(1))
            .or_else(|| centres.first())
            .map_or_else(
                || {
                    route
                        .anchors
                        .first()
                        .map_or([0, 0], |anchor| [anchor.x, anchor.z])
                },
                |centre| [centre.world_x, centre.world_z],
            );
        actions.push(Action {
            id: "park.target".into(),
            stage: "modern-new".into(),
            kind: "public-park".into(),
            target_id: "district.park".into(),
            world_x: anchor[0],
            world_z: anchor[1],
            length_m: 0.0,
            area_m2: park_target,
            reason: "population, jobs and walking access create public-space demand".into(),
        });
    }
    let renewal_count = ((assignments.len() as f64) * program.renewal_share).round() as usize;
    for assignment in assignments
        .iter()
        .filter(|assignment| assignment.planning_action == "evaluate-redevelopment")
        .take(renewal_count)
    {
        if let Some(building) = settlement
            .buildings
            .iter()
            .find(|building| building.id == assignment.building_id)
        {
            actions.push(Action {
                id: format!("renewal.{}", building.id),
                stage: "renewal".into(),
                kind: "parcel-consolidation".into(),
                target_id: building.id.clone(),
                world_x: building.world_x,
                world_z: building.world_z,
                length_m: 0.0,
                area_m2: f64::from(building.width_m * building.depth_m),
                reason: "high network centrality and old housing justify a redevelopment study"
                    .into(),
            });
        }
    }

    actions.sort_by(|left, right| {
        left.stage
            .cmp(&right.stage)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut failures = Vec::new();
    if settlement.buildings.is_empty() {
        failures.push("settlement has no buildings to assign to planning history".into());
    }
    if !settlement.buildings.is_empty() && assignments.len() != settlement.buildings.len() {
        failures.push("not every building received a planning-history assignment".into());
    }
    if centres.is_empty() && !settlement.buildings.is_empty() {
        failures.push("route network has no usable centre for the settlement".into());
    }
    Ok(Some(Plan {
        format: "glyphweave.planning-history-plan".into(),
        version: 1,
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        settlement_scale,
        community_profile: settlement
            .community_profile
            .clone()
            .unwrap_or_else(|| "generic".into()),
        centres,
        actions,
        assignments,
        failures,
    }))
}

fn neighbourhood_rule(profile: &str, district: &str) -> NeighbourhoodRule {
    let (typology_id, tenure_mix, parking_strategy, frontage_strategy) = match (profile, district) {
        ("china-gated", "old-core") => (
            "china-historic-mixed-lane",
            "market-ownership-and-rental",
            "curbside-and-shared",
            "continuous-small-shopfront",
        ),
        ("china-gated", "renewal-or-expansion") => (
            "china-gated-redevelopment",
            "market-housing-mixed-block",
            "underground-or-podium",
            "perimeter-retail-with-gated-interior",
        ),
        ("china-gated", _) => (
            "china-gated-residential-enclave",
            "owner-occupied-superblock",
            "underground-parking",
            "enclosed-residential-with-entrance-services",
        ),
        ("china-urban-village", "old-core") => (
            "urban-village-fine-grain",
            "local-resident-and-migrant-rental",
            "informal-shared",
            "mixed-lane-frontage",
        ),
        ("china-urban-village", "renewal-or-expansion") => (
            "urban-village-renewal",
            "resettlement-and-market-housing",
            "shared-podium",
            "retained-lane-with-new-block-frontage",
        ),
        ("china-urban-village", _) => (
            "transit-superblock",
            "market-housing",
            "podium-and-underground",
            "large-block-with-transit-commercial-edge",
        ),
        ("us-suburban", "old-core") => (
            "us-small-town-grid",
            "owner-and-renter",
            "surface-lot",
            "main-street-commercial",
        ),
        ("us-suburban", "renewal-or-expansion") => (
            "us-commercial-corridor-infill",
            "mixed-tenure",
            "surface-lot",
            "strip-commercial-and-apartment-infill",
        ),
        ("us-suburban", _) => (
            "us-income-segregated-suburb",
            "homeowner-dominant",
            "surface-lot-and-garage",
            "cul-de-sac-and-arterial-retail",
        ),
        ("us-mixed-income", "old-core") => (
            "us-mixed-tenure-old-grid",
            "income-diverse-rental-and-owner",
            "curbside-and-structured",
            "continuous-mixed-use-street",
        ),
        ("us-mixed-income", "renewal-or-expansion") => (
            "us-mixed-income-transit-infill",
            "inclusionary-and-market-rate",
            "structured-and-unbundled",
            "transit-oriented-mixed-use",
        ),
        ("us-mixed-income", _) => (
            "us-income-diverse-corridor",
            "mixed-income",
            "parking-podium",
            "commercial-corridor-with-multi-family-blocks",
        ),
        ("europe-perimeter", "old-core") => (
            "european-historic-perimeter",
            "mixed-tenure",
            "curbside-and-courtyard",
            "continuous-street-wall",
        ),
        ("europe-perimeter", "renewal-or-expansion") => (
            "european-perimeter-infill",
            "social-and-market-housing",
            "courtyard-and-transit",
            "perimeter-block-repair",
        ),
        ("europe-perimeter", _) => (
            "european-perimeter-extension",
            "mixed-housing",
            "transit-first",
            "street-square-transition",
        ),
        ("japan-machiya", "old-core") => (
            "japan-machiya-row",
            "small-owner-and-renter",
            "minimal-curbside",
            "shop-house-row",
        ),
        ("japan-machiya", "renewal-or-expansion") => (
            "japan-station-renewal",
            "mixed-rental-and-owner",
            "station-podium",
            "arcade-to-station-frontage",
        ),
        ("japan-machiya", _) => (
            "japan-transit-residential",
            "owner-and-rental",
            "station-podium",
            "station-commercial-with-quiet-side-streets",
        ),
        ("hong-kong-estate", "old-core") => (
            "hong-kong-street-market-block",
            "mixed-rental-and-owner",
            "minimal-curbside",
            "podium-shop-and-market-frontage",
        ),
        ("hong-kong-estate", "renewal-or-expansion") => (
            "hong-kong-redevelopment-podium",
            "public-private-mix",
            "podium-and-transit",
            "vertical-mixed-podium",
        ),
        ("hong-kong-estate", _) => (
            "hong-kong-estate-cluster",
            "public-rental-and-sale",
            "transit-and-podium",
            "estate-centre-and-connected-podium",
        ),
        (_, "old-core") => (
            "generic-mixed-old-core",
            "mixed-tenure",
            "shared-curbside",
            "mixed-street-frontage",
        ),
        (_, "renewal-or-expansion") => (
            "generic-redevelopment",
            "mixed-tenure",
            "structured",
            "mixed-use-frontage",
        ),
        _ => (
            "generic-new-neighbourhood",
            "mixed-tenure",
            "district-parking",
            "connected-frontage",
        ),
    };
    NeighbourhoodRule {
        typology_id: typology_id.into(),
        tenure_mix: tenure_mix.into(),
        parking_strategy: parking_strategy.into(),
        frontage_strategy: frontage_strategy.into(),
    }
}

pub fn materialize_public_space(
    planning: &Plan,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    occupied: &[EntityInstance],
) -> Materialization {
    let mut entities = Vec::new();
    let mut skipped = Vec::new();
    for action in planning
        .actions
        .iter()
        .filter(|action| action.kind == "public-park")
    {
        let side_m = action.area_m2.max(256.0).sqrt().clamp(16.0, 240.0) as i32;
        let mut placed = None;
        for ring in 0..=10 {
            let radius = ring * 16;
            for (dx, dz) in [
                (0, 0),
                (radius, 0),
                (-radius, 0),
                (0, radius),
                (0, -radius),
                (radius, radius),
                (radius, -radius),
                (-radius, radius),
                (-radius, -radius),
            ] {
                let x = action.world_x + dx;
                let z = action.world_z + dz;
                let half = f64::from(side_m) * 0.5 + 6.0;
                if f64::from(x) - half < f64::from(scene.origin_x)
                    || f64::from(z) - half < f64::from(scene.origin_z)
                    || f64::from(x) + half >= f64::from(scene.origin_x + scene.width_m as i32)
                    || f64::from(z) + half >= f64::from(scene.origin_z + scene.depth_m as i32)
                {
                    continue;
                }
                let candidate = OrientedFootprint {
                    center_x: f64::from(x),
                    center_z: f64::from(z),
                    width_m: f64::from(side_m),
                    depth_m: f64::from(side_m),
                    rotation_y_deg: 0.0,
                };
                let free = occupied
                    .iter()
                    .chain(entities.iter())
                    .filter(|entity| {
                        matches!(
                            entity.kind.as_str(),
                            "road"
                                | "sidewalk"
                                | "storefront"
                                | "office_tower"
                                | "residential_home"
                                | "residential_block"
                                | "park"
                        )
                    })
                    .all(|entity| {
                        !oriented_footprints_overlap(
                            &candidate,
                            &OrientedFootprint::from_entity(entity),
                            0.5,
                        )
                    });
                if free {
                    placed = Some((x, z, side_m));
                    break;
                }
            }
            if placed.is_some() {
                break;
            }
        }
        let Some((x, z, side_m)) = placed else {
            skipped.push(SkippedAction {
                action_id: action.id.clone(),
                kind: action.kind.clone(),
                reason: "no park footprint clear of roads, buildings and scene edge".into(),
            });
            continue;
        };
        let world_y = terrain
            .height(x, z)
            .map_or(0.0, |height| height * 4.0)
            .round() as i32;
        entities.push(EntityInstance {
            entity_id: format!("generated.planning-history.{}", action.id),
            asset_id: "landscape.park".into(),
            kind: "park".into(),
            world_x: x,
            world_z: z,
            world_y,
            scale: 1.0,
            width_m: side_m as f32,
            depth_m: side_m as f32,
            height_m: 0.15,
            rotation_y_deg: 0.0,
            grounding: super::GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        });
    }
    Materialization { entities, skipped }
}

pub fn materialize_centres(
    planning: &Plan,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    occupied: &[EntityInstance],
    water: super::WaterGeometry,
) -> Materialization {
    let mut entities = Vec::new();
    let mut skipped = Vec::new();
    for action in &planning.actions {
        let Some((kind, width, depth, height, asset_id)) = centre_entity_spec(action.kind.as_str())
        else {
            continue;
        };
        let mut placed = None;
        for ring in 0..=16 {
            let radius = ring * 16;
            for (dx, dz) in [
                (0, 0),
                (radius, 0),
                (-radius, 0),
                (0, radius),
                (0, -radius),
                (radius, radius),
                (radius, -radius),
                (-radius, radius),
                (-radius, -radius),
            ] {
                let x = action.world_x + dx;
                let z = action.world_z + dz;
                let half_x = f64::from(width) * 0.5 + 6.0;
                let half_z = f64::from(depth) * 0.5 + 6.0;
                if water.intersects_footprint(
                    f64::from(x),
                    f64::from(z),
                    f64::from(width),
                    f64::from(depth),
                    0.0,
                    2.0,
                ) {
                    continue;
                }
                if f64::from(x) - half_x < f64::from(scene.origin_x)
                    || f64::from(z) - half_z < f64::from(scene.origin_z)
                    || f64::from(x) + half_x >= f64::from(scene.origin_x + scene.width_m as i32)
                    || f64::from(z) + half_z >= f64::from(scene.origin_z + scene.depth_m as i32)
                {
                    continue;
                }
                let candidate = OrientedFootprint {
                    center_x: f64::from(x),
                    center_z: f64::from(z),
                    width_m: f64::from(width),
                    depth_m: f64::from(depth),
                    rotation_y_deg: 0.0,
                };
                let free = occupied
                    .iter()
                    .chain(entities.iter())
                    .filter(|entity| {
                        matches!(
                            entity.kind.as_str(),
                            "road"
                                | "sidewalk"
                                | "storefront"
                                | "office_tower"
                                | "residential_home"
                                | "residential_block"
                                | "park"
                                | "market"
                                | "town_hall"
                                | "industrial"
                                | "resort_lodge"
                                | "community_plaza"
                                | "commercial_center"
                        )
                    })
                    .all(|entity| {
                        !oriented_footprints_overlap(
                            &candidate,
                            &OrientedFootprint::from_entity(entity),
                            0.5,
                        )
                    });
                if !free {
                    continue;
                }
                let mut low = f64::INFINITY;
                let mut high = f64::NEG_INFINITY;
                let mut complete = true;
                for (sx, sz) in [
                    (-half_x, -half_z),
                    (half_x, -half_z),
                    (-half_x, half_z),
                    (half_x, half_z),
                ] {
                    if let Some(height) =
                        terrain.height(x + sx.round() as i32, z + sz.round() as i32)
                    {
                        low = low.min(f64::from(height));
                        high = high.max(f64::from(height));
                    } else {
                        complete = false;
                    }
                }
                if complete && (high - low) / f64::from(width.min(depth)) <= 0.16 {
                    placed = Some((x, z));
                    break;
                }
            }
            if placed.is_some() {
                break;
            }
        }
        let Some((x, z)) = placed else {
            skipped.push(SkippedAction {
                action_id: action.id.clone(),
                kind: action.kind.clone(),
                reason: "no centre footprint clear of roads/buildings with a buildable grade"
                    .into(),
            });
            continue;
        };
        let nearest_road = occupied
            .iter()
            .filter(|entity| entity.kind == "road")
            .min_by_key(|entity| {
                let dx = i64::from(entity.world_x - x);
                let dz = i64::from(entity.world_z - z);
                dx * dx + dz * dz
            });
        let world_y = nearest_road.map_or_else(
            || {
                terrain
                    .height(x, z)
                    .map_or(0.0, |height| height * 4.0)
                    .round() as i32
            },
            |road| road.world_y,
        );
        let mut anchors = nearest_road
            .map(|road| {
                let (anchor_x, anchor_z, direction) =
                    if (road.world_x - x).abs() >= (road.world_z - z).abs() {
                        if road.world_x >= x {
                            (x + width / 2, z, "east")
                        } else {
                            (x - width / 2, z, "west")
                        }
                    } else if road.world_z >= z {
                        (x, z + depth / 2, "south")
                    } else {
                        (x, z - depth / 2, "north")
                    };
                vec![super::SpatialAnchor {
                    id: "front".into(),
                    world_x: anchor_x,
                    world_z: anchor_z,
                    direction: direction.into(),
                    target: Some("road".into()),
                }]
            })
            .unwrap_or_default();
        if !anchors.is_empty() {
            let side_half = (width as f64 * 0.5 - 0.5).max(0.5);
            anchors.push(super::SpatialAnchor {
                id: "side-left".into(),
                world_x: (f64::from(x) - side_half).round() as i32,
                world_z: z,
                direction: "west".into(),
                target: Some("road".into()),
            });
            anchors.push(super::SpatialAnchor {
                id: "side-right".into(),
                world_x: (f64::from(x) + side_half).round() as i32,
                world_z: z,
                direction: "east".into(),
                target: Some("road".into()),
            });
        }
        entities.push(EntityInstance {
            entity_id: format!("generated.planning-history.centre.{}", action.id),
            asset_id: asset_id.into(),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            world_y,
            scale: 1.0,
            width_m: width as f32,
            depth_m: depth as f32,
            height_m: height as f32,
            rotation_y_deg: 0.0,
            grounding: super::GroundingSpec::default(),
            anchors,
            bounds: None,
        });
    }
    Materialization { entities, skipped }
}

fn centre_entity_spec(kind: &str) -> Option<(&'static str, i32, i32, i32, &'static str)> {
    match kind {
        "office-and-transit-cluster" => Some(("office_tower", 20, 20, 72, "building.office_tower")),
        "civic-axis" => Some(("town_hall", 36, 28, 18, "building.town_hall")),
        "port-logistics-district" => Some(("industrial", 60, 40, 16, "building.industrial")),
        "scenic-gateway" => Some(("community_plaza", 48, 36, 1, "landscape.tourism-gateway")),
        "mixed-use-centre" => Some(("community_plaza", 48, 36, 1, "landscape.community-plaza")),
        _ => None,
    }
}

pub struct RoadWideningResult {
    pub applied: Vec<String>,
    pub skipped: Vec<SkippedAction>,
}

pub fn apply_road_widening(entities: &mut [EntityInstance], planning: &Plan) -> RoadWideningResult {
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    let buildings = entities
        .iter()
        .filter(|entity| is_building_kind(entity.kind.as_str()))
        .map(OrientedFootprint::from_entity)
        .collect::<Vec<_>>();
    for action in planning
        .actions
        .iter()
        .filter(|action| action.kind == "widen-and-rebuild-street")
    {
        if action.length_m <= 0.0 {
            continue;
        }
        let target_width = (action.area_m2 / action.length_m).clamp(8.0, 32.0) as f32;
        let prefix = format!("generated.route-evolution.{}.", action.target_id);
        for entity in entities
            .iter_mut()
            .filter(|entity| entity.kind == "road" && entity.entity_id.starts_with(&prefix))
        {
            // Route-evolution width is a route-level contract consumed by
            // HTML and collision. A late per-segment entity widening changes
            // terrain and collision without changing the visible road. Keep
            // the published route width and record the refusal by name until
            // segment-local widths can be applied to all three consumers.
            if entity.entity_id.starts_with("generated.route-evolution.") {
                skipped.push(SkippedAction {
                    action_id: format!("{}:{}", action.id, entity.entity_id),
                    kind: action.kind.clone(),
                    reason: "route-level width is authoritative; segment entity widening would diverge from preview and collision".into(),
                });
                continue;
            }
            if entity.depth_m < target_width && !near_stream_chunk_boundary(entity, 48) {
                let widened = OrientedFootprint {
                    center_x: f64::from(entity.world_x),
                    center_z: f64::from(entity.world_z),
                    width_m: f64::from(entity.width_m),
                    depth_m: f64::from(target_width),
                    rotation_y_deg: f64::from(entity.rotation_y_deg),
                };
                if let Some(building) = buildings
                    .iter()
                    .find(|building| oriented_footprints_overlap(&widened, building, 0.25))
                {
                    skipped.push(SkippedAction {
                        action_id: format!("{}:{}", action.id, entity.entity_id),
                        kind: action.kind.clone(),
                        reason: format!(
                            "widening to {target_width:.1}m would intersect a building near ({:.0},{:.0})",
                            building.center_x, building.center_z
                        ),
                    });
                    continue;
                }
                entity.depth_m = target_width;
                changed.push(entity.entity_id.clone());
            }
        }
    }
    changed.sort();
    changed.dedup();
    RoadWideningResult {
        applied: changed,
        skipped,
    }
}

fn near_stream_chunk_boundary(entity: &EntityInstance, margin_m: i32) -> bool {
    const STREAM_CHUNK_M: i32 = 512;
    let near = |value: i32| {
        let rem = value.rem_euclid(STREAM_CHUNK_M);
        rem < margin_m || rem > STREAM_CHUNK_M - margin_m
    };
    near(entity.world_x) || near(entity.world_z)
}

fn derive_centres(
    route: &RoutePlan,
    graph: &Graph,
    factors: &[String],
    settlement_scale: &str,
    focus_bias: Option<&urban_slice::FocusBias>,
) -> Vec<Centre> {
    let mut anchors = route
        .anchors
        .iter()
        .filter_map(|anchor| {
            graph.nearest_node(anchor.x, anchor.z).map(|node| {
                let degree = graph.adjacency[node].len();
                (anchor, node, degree, graph.points[node])
            })
        })
        .collect::<Vec<_>>();
    anchors.sort_by_key(|(anchor, _, degree, point)| {
        (Reverse(*degree), point[1], point[0], anchor.id.clone())
    });
    let mut roles = focus_bias
        .map(|bias| {
            bias.centre_roles
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .filter(|roles| !roles.is_empty())
        .unwrap_or_else(|| {
            if factors.iter().any(|factor| factor == "political-capital") {
                vec!["political", "historic", "mixed"]
            } else if factors.iter().any(|factor| factor == "financial-center") {
                vec!["financial", "mixed", "historic"]
            } else if factors.iter().any(|factor| factor == "port-industrial") {
                vec!["port", "mixed", "historic"]
            } else if factors.iter().any(|factor| factor == "tourism") {
                vec!["tourism", "historic", "mixed"]
            } else {
                vec!["mixed"]
            }
        });
    roles.truncate(match settlement_scale {
        "metropolis" | "megacity" => 3,
        "city" => 2,
        _ => 1,
    });
    let scale = if matches!(settlement_scale, "metropolis" | "megacity") {
        1.25
    } else {
        1.0
    };
    anchors
        .into_iter()
        .take(roles.len())
        .enumerate()
        .map(|(index, (anchor, node, degree, point))| {
            let reachable = graph.reachable_node_count(node);
            let degree_signal = degree as f64 / (degree as f64 + 2.0);
            let reach_signal = reachable as f64 / graph.points.len().max(1) as f64;
            let role = roles[index].as_ref();
            let role_signal = match role {
                "historic" => 0.18,
                "financial" | "political" => 0.15,
                "port" | "tourism" => 0.10,
                _ => 0.0,
            };
            let influence =
                (0.22 + degree_signal * 0.24 + reach_signal * 0.26 + role_signal) * scale;
            let mut signals = vec!["road-network".into(), "route-reachability".into()];
            if !anchor.role.is_empty() {
                signals.push(format!("anchor:{}", anchor.role));
            }
            if role_signal > 0.0 {
                signals.push(format!("special-role:{role}"));
            }
            Centre {
                id: format!("centre.{index}.{}", anchor.id),
                role: role.into(),
                world_x: point[0],
                world_z: point[1],
                network_degree: degree,
                influence,
                source: format!("route-anchor:{}", anchor.role),
                signals,
            }
        })
        .collect()
}

fn centre_actions(centres: &[Centre], settlement_scale: &str, route: &RoutePlan) -> Vec<Action> {
    centres
        .iter()
        .map(|centre| {
            let kind = match centre.role.as_str() {
                "financial" => "office-and-transit-cluster",
                "political" => "civic-axis",
                "port" => "port-logistics-district",
                "tourism" => "scenic-gateway",
                "historic" => "historic-core-frame",
                _ => "mixed-use-centre",
            };
            Action {
                id: format!("centre.{}.{}", centre.id, kind),
                stage: if centre.role == "historic" {
                    "old-core"
                } else {
                    "modern-new"
                }
                .into(),
                kind: kind.into(),
                target_id: centre.id.clone(),
                world_x: centre.world_x,
                world_z: centre.world_z,
                length_m: if matches!(settlement_scale, "metropolis" | "megacity") {
                    route
                        .routes
                        .iter()
                        .map(|route| route.length_m)
                        .fold(0.0_f64, f64::max)
                        .min(1800.0)
                } else {
                    0.0
                },
                area_m2: 0.0,
                reason: format!(
                    "{} role is derived from settlement scale and network influence",
                    centre.role
                ),
            }
        })
        .collect()
}

fn road_actions(
    route: &RoutePlan,
    scale: &str,
    focus_bias: Option<&urban_slice::FocusBias>,
) -> Vec<Action> {
    let target_width = urban_slice::road_target_width_m(
        scale,
        focus_bias.map_or(1.0, |bias| bias.road_widening_bias),
    ) as i32;
    route
        .routes
        .iter()
        .filter(|route| route.status == "pass" && route.length_m >= 120.0)
        .enumerate()
        .map(|(index, route)| {
            let point = route
                .polyline
                .get(route.polyline.len() / 2)
                .map_or([0, 0], |point| [point.x, point.z]);
            Action {
                id: format!("road.{}.widen-{}", route.id, index),
                stage: if index == 0 { "expansion" } else { "modern-new" }.into(),
                kind: "widen-and-rebuild-street".into(),
                target_id: route.id.clone(),
                world_x: point[0],
                world_z: point[1],
                length_m: route.length_m.min(320.0),
                area_m2: route.length_m.min(320.0) * f64::from(target_width),
                reason: format!(
                    "network growth raises the target pavement width to {target_width}m; final cross-section still passes local contracts"
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::route_evolution::{Anchor, Route, RoutePoint};
    use crate::worldgen::settlement_evolution::{Building, Capacity, Demand};

    fn route_plan(town: bool) -> RoutePlan {
        let polyline = (0..12)
            .map(|index| RoutePoint {
                x: 100 + index * 40,
                z: if town { 260 } else { 260 + (index % 3) * 12 },
                natural_height_m: 10.0,
                design_height_m: 10.0,
                water: false,
            })
            .collect();
        RoutePlan {
            format: "glyphweave.route-evolution-plan".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.55,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor {
                    id: "old".into(),
                    x: 100,
                    z: 260,
                    role: "valley-floor".into(),
                },
                Anchor {
                    id: "new".into(),
                    x: 540,
                    z: 260,
                    role: "junction".into(),
                },
            ],
            candidate_nodes: Vec::new(),
            routes: vec![Route {
                id: "main".into(),
                from: "old".into(),
                to: "new".into(),
                status: "pass".into(),
                road_width_m: 8,
                road_class: "township".into(),
                alignment: "terrain-following".into(),
                polyline,
                length_m: 440.0,
                direct_length_m: 440.0,
                curve_deviation_m: 0.0,
                maximum_grade: 0.0,
                maximum_fill_m: 0.0,
                maximum_terrain_fill_m: 0.0,
                fill_volume_m3: 0.0,
                bridge_spans: Vec::new(),
                failure: None,
            }],
            failures: Vec::new(),
        }
    }

    fn settlement() -> SettlementPlan {
        let make = |id: &str, x: i32, kind: &str| Building {
            id: id.into(),
            kind: kind.into(),
            route_id: "main".into(),
            side: "left".into(),
            world_x: x,
            world_z: 280,
            width_m: 20.0,
            depth_m: 24.0,
            height_m: 12.0,
            floors: 3,
            floor_area_m2: 300.0,
            households: 20,
            jobs: 5,
            retail_floor_area_m2: 20.0,
            service_budget: 0.0,
            access_distance_m: 10.0,
            center_distance_m: 0.0,
            community_profile: "china-gated".into(),
            income_band: "mixed-market-rate".into(),
            ..Building::default()
        };
        SettlementPlan {
            format: "settlement".into(),
            version: 1,
            status: "pass".into(),
            demand: Demand {
                households: 100,
                jobs: 25,
                retail_floor_area_m2: 100.0,
                service_budget: 0.0,
            },
            delivered: Demand {
                households: 100,
                jobs: 25,
                retail_floor_area_m2: 100.0,
                service_budget: 0.0,
            },
            unmet_demand: Demand {
                households: 0,
                jobs: 0,
                retail_floor_area_m2: 0.0,
                service_budget: 0.0,
            },
            capacity: Capacity {
                candidate_lots: 5,
                accepted_lots: 5,
                rejected_lots: 0,
                floor_area_m2: 1500.0,
            },
            buildings: vec![
                make("old-home", 140, "residential_home"),
                make("old-shop", 240, "storefront"),
                make("outer-home", 500, "residential_home"),
            ],
            rejections: Vec::new(),
            community_profile: Some("china-gated".into()),
            district_allocation: Vec::new(),
            street_frontage: Default::default(),
            density_curve: Vec::new(),
            population: None,
            facility_requirements: Vec::new(),
        }
    }

    fn style(scale: &str, factors: &[&str]) -> Value {
        serde_json::json!({
            "planningHistory": {
                "version": 1,
                "oldCoreRadiusM": 240,
                "parkAreaM2PerPerson": 9,
                "renewalShare": 1.0
            },
            "urbanSlice": {
                "version": 1,
                "settlementScale": scale,
                "mode": if scale == "town" { "whole-settlement" } else { "feature-cut" },
                "focusFeature": if scale == "town" { Value::Null } else { Value::String("financial-waterfront".into()) },
                "cropReason": "planning history test",
                "contextRadiusM": 2500,
                "offMapContinuity": ["roads", "urban-fabric"],
                "specialFactors": factors
            }
        })
    }

    #[test]
    fn network_centres_drive_redevelopment_and_road_widening() {
        let plan = plan(&route_plan(true), &settlement(), &style("town", &[]))
            .unwrap()
            .unwrap();
        assert_eq!(plan.status, "pass");
        assert_eq!(plan.centres.len(), 1);
        assert!(
            plan.actions
                .iter()
                .any(|action| action.kind == "widen-and-rebuild-street")
        );
        assert!(
            plan.actions
                .iter()
                .any(|action| action.kind == "public-park")
        );
        assert!(
            plan.actions
                .iter()
                .any(|action| action.kind == "parcel-consolidation")
        );
    }

    #[test]
    fn metropolis_special_centres_create_distinct_roles() {
        let plan = plan(
            &route_plan(false),
            &settlement(),
            &style("metropolis", &["financial-center", "special-economic-zone"]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.centres[0].role, "financial");
        assert!(plan.centres.len() >= 2);
        assert!(
            plan.actions
                .iter()
                .any(|action| action.kind == "office-and-transit-cluster")
        );
        assert!(
            plan.actions
                .iter()
                .find(|action| action.kind == "widen-and-rebuild-street")
                .is_some_and(|action| action.area_m2 > 5_000.0)
        );
    }

    #[test]
    fn focus_feature_changes_centre_priority_and_road_target() {
        let waterfront = plan(
            &route_plan(false),
            &settlement(),
            &style("metropolis", &["financial-center", "coastal-reclamation"]),
        )
        .unwrap()
        .unwrap();
        let mut historic_style = style("metropolis", &["historic-core"]);
        historic_style["urbanSlice"]["focusFeature"] = serde_json::json!("historic-market-lanes");
        historic_style["urbanSlice"]["requiredAnchors"] = serde_json::json!([
            {"id": "market", "kind": "market", "role": "historic-focus"}
        ]);
        let historic = plan(&route_plan(false), &settlement(), &historic_style)
            .unwrap()
            .unwrap();
        assert_eq!(waterfront.centres[0].role, "financial");
        assert_eq!(historic.centres[0].role, "historic");
        let width = |plan: &Plan| {
            let action = plan
                .actions
                .iter()
                .find(|action| action.kind == "widen-and-rebuild-street")
                .unwrap();
            action.area_m2 / action.length_m
        };
        assert!(width(&waterfront) > width(&historic));
    }

    #[test]
    fn invalid_policy_fails_closed_and_same_input_is_stable() {
        let mut bad = style("town", &[]);
        bad["planningHistory"]["renewalShare"] = serde_json::json!(2.0);
        assert!(plan(&route_plan(true), &settlement(), &bad).is_err());
        let first = plan(
            &route_plan(false),
            &settlement(),
            &style("metropolis", &["political-capital"]),
        )
        .unwrap()
        .unwrap();
        let second = plan(
            &route_plan(false),
            &settlement(),
            &style("metropolis", &["political-capital"]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn route_evolution_widening_is_refused_instead_of_creating_a_second_width_source() {
        let planning = plan(&route_plan(true), &settlement(), &style("town", &[]))
            .unwrap()
            .unwrap();
        let action = planning
            .actions
            .iter()
            .find(|action| action.kind == "widen-and-rebuild-street")
            .unwrap();
        let mut roads = vec![EntityInstance {
            entity_id: format!("generated.route-evolution.{}.0", action.target_id),
            asset_id: "road.generated".into(),
            kind: "road".into(),
            world_x: 300,
            world_z: 260,
            world_y: 40,
            scale: 1.0,
            width_m: 8.0,
            depth_m: 8.0,
            height_m: 0.45,
            rotation_y_deg: 0.0,
            grounding: super::super::GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        }];
        let changed = apply_road_widening(&mut roads, &planning);
        assert!(changed.applied.is_empty());
        assert_eq!(roads[0].depth_m, 8.0);
        assert!(
            changed
                .skipped
                .iter()
                .any(|skip| { skip.reason.contains("route-level width is authoritative") })
        );
    }

    #[test]
    fn neighbourhood_rules_encode_regional_traditions() {
        let china = neighbourhood_rule("china-gated", "new-city-or-edge");
        let us = neighbourhood_rule("us-mixed-income", "new-city-or-edge");
        let europe = neighbourhood_rule("europe-perimeter", "new-city-or-edge");
        let hong_kong = neighbourhood_rule("hong-kong-estate", "new-city-or-edge");
        assert_eq!(china.typology_id, "china-gated-residential-enclave");
        assert_eq!(us.typology_id, "us-income-diverse-corridor");
        assert_eq!(europe.frontage_strategy, "street-square-transition");
        assert_eq!(hong_kong.parking_strategy, "transit-and-podium");
        assert_ne!(china.typology_id, us.typology_id);
    }
}
