//! Fresh-food market accessibility over the route graph.
//!
//! This module never uses straight-line radius as service evidence. Demand is
//! attached to the settlement plan, markets are placed on route nodes, and
//! travel time is measured through the actual route polyline.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::route_evolution::Plan as RoutePlan;
use super::settlement_evolution::Plan as SettlementPlan;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_beta")]
    pub beta: f64,
    #[serde(default = "default_max_walk")]
    pub max_travel_minutes: f64,
    #[serde(default = "default_walk_speed")]
    pub walk_speed_mps: f64,
    #[serde(default = "default_capacity")]
    pub market_capacity: f64,
    #[serde(default = "default_quality")]
    pub market_quality: f64,
    #[serde(default = "default_coverage")]
    pub coverage_target: f64,
    #[serde(default = "default_max_markets")]
    pub max_markets: usize,
    #[serde(default = "default_spacing")]
    pub candidate_spacing_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DemandPoint {
    pub id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub demand: f64,
    pub access_walk_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub capacity: f64,
    pub quality: f64,
    pub load: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Accessibility {
    pub demand_id: String,
    pub nearest_market_id: Option<String>,
    pub travel_minutes: f64,
    pub assigned_share: f64,
    pub local_supply_density: f64,
    pub deficit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub model: String,
    pub beta: f64,
    pub total_demand: f64,
    pub served_demand: f64,
    pub coverage_ratio: f64,
    pub markets: Vec<Market>,
    pub demand_points: Vec<DemandPoint>,
    pub accessibility: Vec<Accessibility>,
    pub failures: Vec<String>,
}

const fn default_true() -> bool {
    true
}
const fn default_beta() -> f64 {
    0.08
}
const fn default_max_walk() -> f64 {
    15.0
}
const fn default_walk_speed() -> f64 {
    1.3
}
const fn default_capacity() -> f64 {
    280.0
}
const fn default_quality() -> f64 {
    1.0
}
const fn default_coverage() -> f64 {
    0.85
}
const fn default_max_markets() -> usize {
    4
}
const fn default_spacing() -> f64 {
    80.0
}

pub fn enabled(style: &Value) -> bool {
    from_style(style).is_ok_and(|program| program.is_some_and(|program| program.enabled))
}

pub fn validate_style(style: &Value) -> Result<(), String> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    if !program.enabled {
        return Ok(());
    }
    if program.version != 1
        || !(0.005..=0.5).contains(&program.beta)
        || !(3.0..=60.0).contains(&program.max_travel_minutes)
        || !(0.5..=2.5).contains(&program.walk_speed_mps)
        || !(20.0..=5000.0).contains(&program.market_capacity)
        || !(0.2..=3.0).contains(&program.market_quality)
        || !(0.1..=1.0).contains(&program.coverage_target)
        || program.max_markets == 0
        || program.max_markets > 32
        || !(24.0..=1000.0).contains(&program.candidate_spacing_m)
    {
        return Err("invalid marketCatchment limits".into());
    }
    Ok(())
}

fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("marketCatchment") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn plan(
    routes: &RoutePlan,
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
    let graph = Graph::from_routes(routes);
    if graph.nodes.is_empty() {
        return Err("marketCatchment requires at least one route node".into());
    }
    let demand_points: Vec<DemandPoint> = settlement
        .buildings
        .iter()
        .map(|building| {
            let (node, access_walk_m) =
                graph.nearest_node(f64::from(building.world_x), f64::from(building.world_z));
            let mut point = DemandPoint {
                id: building.id.clone(),
                world_x: building.world_x,
                world_z: building.world_z,
                demand: f64::from(building.households)
                    + f64::from(building.jobs) / 3.0
                    + building.retail_floor_area_m2 / 120.0
                    + building.service_budget * 25.0,
                access_walk_m,
            };
            point.demand = point.demand.max(1.0);
            let _ = node;
            point
        })
        .collect();
    if demand_points.is_empty() {
        return Err("marketCatchment requires settlement demand".into());
    }
    let demand_nodes: Vec<usize> = settlement
        .buildings
        .iter()
        .map(|building| {
            graph
                .nearest_node(f64::from(building.world_x), f64::from(building.world_z))
                .0
        })
        .collect();
    let candidates = graph.candidates(program.candidate_spacing_m);
    let mut selected = Vec::<usize>::new();
    let total_demand: f64 = demand_points.iter().map(|point| point.demand).sum();
    let mut served = vec![0.0_f64; demand_points.len()];
    while selected.len() < program.max_markets {
        let mut best = None;
        for &candidate in &candidates {
            if selected.contains(&candidate) {
                continue;
            }
            let times = graph.times_from(candidate, program.walk_speed_mps);
            let reachable: f64 = demand_points
                .iter()
                .enumerate()
                .filter(|(index, point)| served[*index] < point.demand)
                .map(|(index, point)| {
                    let minutes = times[demand_nodes[index]]
                        + point.access_walk_m / program.walk_speed_mps / 60.0;
                    if minutes <= program.max_travel_minutes {
                        (point.demand - served[index]).max(0.0)
                    } else {
                        0.0
                    }
                })
                .sum();
            if best.is_none_or(|(_, score)| reachable > score) {
                best = Some((candidate, reachable));
            }
        }
        let Some((candidate, _)) = best else {
            break;
        };
        selected.push(candidate);
        let times = graph.times_from(candidate, program.walk_speed_mps);
        let remaining = program.market_capacity;
        let mut capacity = remaining;
        let mut ranked: Vec<(usize, f64)> = demand_points
            .iter()
            .enumerate()
            .filter_map(|(index, point)| {
                let minutes = times[demand_nodes[index]]
                    + point.access_walk_m / program.walk_speed_mps / 60.0;
                (minutes <= program.max_travel_minutes && served[index] < point.demand)
                    .then_some((index, minutes))
            })
            .collect();
        ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        for (index, _) in ranked {
            if capacity <= 1e-9 {
                break;
            }
            let need = (demand_points[index].demand - served[index]).max(0.0);
            let grant = need.min(capacity);
            served[index] += grant;
            capacity -= grant;
        }
        let covered: f64 = served.iter().sum();
        if covered / total_demand >= program.coverage_target {
            break;
        }
    }
    let mut markets = Vec::new();
    let mut all_times = Vec::new();
    for (index, &node) in selected.iter().enumerate() {
        let times = graph.times_from(node, program.walk_speed_mps);
        markets.push(Market {
            id: format!("market.{index}"),
            world_x: graph.nodes[node].x,
            world_z: graph.nodes[node].z,
            capacity: program.market_capacity,
            quality: program.market_quality,
            load: 0.0,
        });
        all_times.push(times);
    }
    let mut accessibility = Vec::new();
    let mut market_loads = vec![0.0_f64; markets.len()];
    for (demand_index, point) in demand_points.iter().enumerate() {
        let mut weights = Vec::new();
        for (market_index, market) in markets.iter().enumerate() {
            let minutes = all_times[market_index][demand_nodes[demand_index]]
                + point.access_walk_m / program.walk_speed_mps / 60.0;
            let attraction = if minutes <= program.max_travel_minutes {
                market.capacity * market.quality * (-program.beta * minutes).exp()
            } else {
                0.0
            };
            weights.push((market_index, minutes, attraction));
        }
        let total_weight: f64 = weights.iter().map(|(_, _, value)| value).sum();
        let mut best_market = None;
        let mut best_minutes = f64::INFINITY;
        for (market_index, minutes, attraction) in &weights {
            if *attraction <= 0.0 {
                continue;
            }
            let share = attraction / total_weight;
            market_loads[*market_index] += point.demand * share;
            if *minutes < best_minutes {
                best_minutes = *minutes;
                best_market = Some(*market_index);
            }
        }
        let density: f64 = weights.iter().map(|(_, _, value)| value).sum();
        accessibility.push(Accessibility {
            demand_id: point.id.clone(),
            nearest_market_id: best_market.map(|index| markets[index].id.clone()),
            travel_minutes: if best_market.is_some() {
                best_minutes
            } else {
                f64::INFINITY
            },
            assigned_share: if total_weight > 0.0 { 1.0 } else { 0.0 },
            local_supply_density: density,
            deficit: (point.demand - density).max(0.0),
        });
    }
    for (market, load) in markets.iter_mut().zip(market_loads) {
        market.load = load;
    }
    let served_demand: f64 = served.iter().sum();
    let coverage_ratio = served_demand / total_demand.max(1e-9);
    let mut failures = Vec::new();
    if coverage_ratio < program.coverage_target {
        failures.push(format!(
            "coverage {:.3} below target {:.3}",
            coverage_ratio, program.coverage_target
        ));
    }
    Ok(Some(Plan {
        format: "glyphweave.market-catchment-plan".into(),
        version: 1,
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "incomplete".into()
        },
        model: "capacity-quality-exp-decay-over-route-network".into(),
        beta: program.beta,
        total_demand,
        served_demand,
        coverage_ratio,
        markets,
        demand_points,
        accessibility,
        failures,
    }))
}

#[derive(Clone)]
struct Node {
    x: i32,
    z: i32,
}

struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Vec<(usize, f64)>>,
}

impl Graph {
    fn from_routes(routes: &RoutePlan) -> Self {
        let mut nodes = Vec::<Node>::new();
        let mut edges = Vec::<Vec<(usize, f64)>>::new();
        for route in routes.routes.iter().filter(|route| route.status == "pass") {
            let mut previous: Option<usize> = None;
            for point in &route.polyline {
                let node = Self::insert_or_find(&mut nodes, &mut edges, point.x, point.z);
                if let Some(previous) = previous {
                    if previous != node {
                        let length = f64::from(nodes[previous].x - nodes[node].x)
                            .hypot(f64::from(nodes[previous].z - nodes[node].z));
                        edges[previous].push((node, length));
                        edges[node].push((previous, length));
                    }
                }
                previous = Some(node);
            }
        }
        Self { nodes, edges }
    }

    fn insert_or_find(
        nodes: &mut Vec<Node>,
        edges: &mut Vec<Vec<(usize, f64)>>,
        x: i32,
        z: i32,
    ) -> usize {
        if let Some(index) = nodes
            .iter()
            .position(|node| (node.x - x).abs() <= 2 && (node.z - z).abs() <= 2)
        {
            return index;
        }
        let index = nodes.len();
        nodes.push(Node { x, z });
        edges.push(Vec::new());
        index
    }

    fn nearest_node(&self, x: f64, z: f64) -> (usize, f64) {
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (index, (x - f64::from(node.x)).hypot(z - f64::from(node.z))))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .unwrap_or((0, 0.0))
    }

    fn times_from(&self, source: usize, speed_mps: f64) -> Vec<f64> {
        let mut times = vec![f64::INFINITY; self.nodes.len()];
        let mut heap = BinaryHeap::new();
        times[source] = 0.0;
        heap.push(TimeEntry {
            minutes: 0.0,
            node: source,
        });
        while let Some(entry) = heap.pop() {
            if entry.minutes > times[entry.node] {
                continue;
            }
            for &(next, distance) in &self.edges[entry.node] {
                let candidate = entry.minutes + distance / speed_mps / 60.0;
                if candidate + 1e-9 < times[next] {
                    times[next] = candidate;
                    heap.push(TimeEntry {
                        minutes: candidate,
                        node: next,
                    });
                }
            }
        }
        times
    }

    fn candidates(&self, spacing_m: f64) -> Vec<usize> {
        let mut result = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if result.iter().all(|prior: &usize| {
                f64::from(self.nodes[*prior].x - node.x)
                    .hypot(f64::from(self.nodes[*prior].z - node.z))
                    >= spacing_m
            }) {
                result.push(index);
            }
        }
        result
    }
}

#[derive(Clone, Copy)]
struct TimeEntry {
    minutes: f64,
    node: usize,
}
impl Eq for TimeEntry {}
impl PartialEq for TimeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.minutes == other.minutes && self.node == other.node
    }
}
impl Ord for TimeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .minutes
            .partial_cmp(&self.minutes)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.node.cmp(&self.node))
    }
}
impl PartialOrd for TimeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::route_evolution::{Anchor, Route, RoutePoint};
    use crate::worldgen::settlement_evolution::{
        Building, Capacity, Demand, Plan as SettlementPlan,
    };

    fn route_plan() -> RoutePlan {
        let polyline = (0..=30)
            .map(|index| RoutePoint {
                x: 40 + index * 12,
                z: 256,
                natural_height_m: 0.0,
                design_height_m: 0.0,
                water: false,
            })
            .collect();
        RoutePlan {
            format: "route".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 12,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.5,
            max_fill_m: 6.0,
            anchors: vec![
                Anchor {
                    id: "a".into(),
                    x: 40,
                    z: 256,
                    role: String::new(),
                },
                Anchor {
                    id: "b".into(),
                    x: 400,
                    z: 256,
                    role: String::new(),
                },
            ],
            routes: vec![Route {
                id: "main".into(),
                from: "a".into(),
                to: "b".into(),
                status: "pass".into(),
                road_width_m: 8,
                polyline,
                length_m: 360.0,
                direct_length_m: 360.0,
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
        let buildings = (0..8)
            .map(|index| Building {
                id: format!("building.{index}"),
                kind: "residential_home".into(),
                route_id: "main".into(),
                side: "left".into(),
                world_x: 60 + index * 40,
                world_z: 278,
                width_m: 20.0,
                depth_m: 24.0,
                height_m: 10.0,
                floors: 3,
                floor_area_m2: 300.0,
                households: 20,
                jobs: 0,
                retail_floor_area_m2: 0.0,
                service_budget: 0.0,
                access_distance_m: 10.0,
                center_distance_m: index as f64 * 40.0,
                community_profile: "generic".into(),
                income_band: "mixed".into(),
            })
            .collect();
        SettlementPlan {
            format: "settlement".into(),
            version: 1,
            status: "pass".into(),
            demand: Demand {
                households: 160,
                jobs: 0,
                retail_floor_area_m2: 0.0,
                service_budget: 0.0,
            },
            delivered: Demand {
                households: 160,
                jobs: 0,
                retail_floor_area_m2: 0.0,
                service_budget: 0.0,
            },
            unmet_demand: Demand {
                households: 0,
                jobs: 0,
                retail_floor_area_m2: 0.0,
                service_budget: 0.0,
            },
            capacity: Capacity {
                candidate_lots: 8,
                accepted_lots: 8,
                rejected_lots: 0,
                floor_area_m2: 2400.0,
            },
            buildings,
            rejections: Vec::new(),
            community_profile: None,
        }
    }

    #[test]
    fn markets_are_placed_on_network_and_cover_demand() {
        let style = serde_json::json!({
            "marketCatchment": {
                "version": 1,
                "beta": 0.08,
                "maxTravelMinutes": 15,
                "walkSpeedMps": 1.3,
                "marketCapacity": 160,
                "marketQuality": 1,
                "coverageTarget": 0.85,
                "maxMarkets": 2,
                "candidateSpacingM": 100
            }
        });
        let plan = plan(&route_plan(), &settlement(), &style).unwrap().unwrap();
        assert_eq!(plan.status, "pass");
        assert!(!plan.markets.is_empty());
        assert!(plan.coverage_ratio >= 0.85);
        assert!(plan
            .accessibility
            .iter()
            .all(|point| point.nearest_market_id.is_some()));
    }

    #[test]
    fn invalid_decay_and_capacity_fail_closed() {
        let style = serde_json::json!({
            "marketCatchment": {
                "version": 1,
                "beta": 0,
                "maxTravelMinutes": 15,
                "walkSpeedMps": 1.3,
                "marketCapacity": 0,
                "marketQuality": 1,
                "coverageTarget": 0.85,
                "maxMarkets": 2,
                "candidateSpacingM": 100
            }
        });
        assert!(validate_style(&style).is_err());
    }
}
