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
use super::urban_slice;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    let centres = derive_centres(route, &graph, &factors, &settlement_scale);
    let scale_distance = match settlement_scale.as_str() {
        "metropolis" | "megacity" => 1800.0,
        "city" => 1100.0,
        _ => 700.0,
    };

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
        });
    }

    actions.extend(centre_actions(&centres, &settlement_scale, route));
    actions.extend(road_actions(route, &settlement_scale));
    let park_target = (f64::from(
        settlement
            .delivered
            .households
            .saturating_add(settlement.delivered.jobs / 2),
    ) * 3.0
        * program.park_area_m2_per_person)
        .min(250_000.0);
    if park_target > 1.0 {
        let anchor = centres.get(1).or_else(|| centres.first()).map_or_else(
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

fn derive_centres(
    route: &RoutePlan,
    graph: &Graph,
    factors: &[String],
    settlement_scale: &str,
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
    let roles = if factors.iter().any(|factor| factor == "political-capital") {
        vec!["political", "historic", "mixed"]
    } else if factors.iter().any(|factor| factor == "financial-center") {
        vec!["financial", "mixed", "historic"]
    } else if factors.iter().any(|factor| factor == "port-industrial") {
        vec!["port", "mixed", "historic"]
    } else if factors.iter().any(|factor| factor == "tourism") {
        vec!["tourism", "historic", "mixed"]
    } else {
        vec!["mixed"]
    };
    let scale = if matches!(settlement_scale, "metropolis" | "megacity") {
        1.25
    } else {
        1.0
    };
    anchors
        .into_iter()
        .take(roles.len())
        .enumerate()
        .map(|(index, (anchor, _, degree, point))| Centre {
            id: format!("centre.{index}.{}", anchor.id),
            role: roles[index].into(),
            world_x: point[0],
            world_z: point[1],
            network_degree: degree,
            influence: (0.45 + degree as f64 * 0.12) * scale,
            source: format!("route-anchor:{}", anchor.role),
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

fn road_actions(route: &RoutePlan, scale: &str) -> Vec<Action> {
    let target_width = if matches!(scale, "metropolis" | "megacity") {
        24
    } else if scale == "city" {
        16
    } else {
        12
    };
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
            routes: vec![Route {
                id: "main".into(),
                from: "old".into(),
                to: "new".into(),
                status: "pass".into(),
                road_width_m: 8,
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
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == "widen-and-rebuild-street"));
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == "public-park"));
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == "parcel-consolidation"));
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
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == "office-and-transit-cluster"));
        assert!(plan
            .actions
            .iter()
            .find(|action| action.kind == "widen-and-rebuild-street")
            .is_some_and(|action| action.area_m2 > 5_000.0));
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
}
