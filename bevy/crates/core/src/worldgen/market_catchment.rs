//! Fresh-food market accessibility over the route graph.
//!
//! This module never uses straight-line radius as service evidence. Demand is
//! attached to the settlement plan, markets are placed on route nodes, and
//! travel time is measured through the actual route polyline.

use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::BakedTerrain;

use super::route_evolution::Plan as RoutePlan;
use super::settlement_evolution::Plan as SettlementPlan;
use super::{
    oriented_footprints_overlap, urban_slice, EntityInstance, OrientedFootprint, SpatialAnchor,
};

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
    /// Optional fresh-food hierarchy. When omitted, the legacy single market
    /// values above remain the exact fallback.
    #[serde(default)]
    pub tiers: Vec<Tier>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tier {
    pub id: String,
    pub kind: String,
    pub capacity: f64,
    pub quality: f64,
    pub max_travel_minutes: f64,
    pub max_count: usize,
    pub candidate_spacing_m: f64,
    #[serde(default = "default_preference")]
    pub preference: f64,
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
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub capacity: f64,
    pub quality: f64,
    pub max_travel_minutes: f64,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
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

struct Assessment {
    market_loads: Vec<f64>,
    accessibility: Vec<Accessibility>,
    served_demand: f64,
    coverage_ratio: f64,
    failures: Vec<String>,
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
const fn default_preference() -> f64 {
    1.0
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
    let mut tier_ids = std::collections::BTreeSet::new();
    for tier in &program.tiers {
        if tier.id.trim().is_empty()
            || tier.kind.trim().is_empty()
            || !tier_ids.insert(tier.id.as_str())
            || !(20.0..=5000.0).contains(&tier.capacity)
            || !(0.2..=3.0).contains(&tier.quality)
            || !(3.0..=60.0).contains(&tier.max_travel_minutes)
            || tier.max_count == 0
            || tier.max_count > 32
            || !(24.0..=1000.0).contains(&tier.candidate_spacing_m)
            || !(0.2..=2.0).contains(&tier.preference)
        {
            return Err("invalid marketCatchment tier".into());
        }
    }
    Ok(())
}

impl Program {
    fn effective_tiers(&self) -> Vec<Tier> {
        if self.tiers.is_empty() {
            vec![Tier {
                id: "market".into(),
                kind: "mixed-market".into(),
                capacity: self.market_capacity,
                quality: self.market_quality,
                max_travel_minutes: self.max_travel_minutes,
                max_count: self.max_markets,
                candidate_spacing_m: self.candidate_spacing_m,
                preference: 1.0,
            }]
        } else {
            self.tiers.clone()
        }
    }
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
    let road_width_m = f64::from(
        urban_slice::planned_road_width_m(style)?.unwrap_or(routes.road_width_m as f32),
    );
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
    let tiers = program.effective_tiers();
    let candidates = tiers
        .iter()
        .map(|tier| graph.candidates(tier.candidate_spacing_m))
        .collect::<Vec<_>>();
    let mut selected = Vec::<(usize, usize)>::new();
    let mut tier_counts = vec![0usize; tiers.len()];
    let total_demand: f64 = demand_points.iter().map(|point| point.demand).sum();
    let mut served = vec![0.0_f64; demand_points.len()];
    while selected.len() < program.max_markets {
        let mut best: Option<(f64, usize, usize)> = None;
        for (tier_index, tier) in tiers.iter().enumerate() {
            if tier_counts[tier_index] >= tier.max_count {
                continue;
            }
            for &candidate in &candidates[tier_index] {
                if selected.iter().any(|(node, _)| *node == candidate) {
                    continue;
                }
                let Some(candidate_site) = market_site(
                    routes,
                    graph.nodes[candidate].x,
                    graph.nodes[candidate].z,
                    tier,
                    settlement,
                    road_width_m,
                ) else {
                    continue;
                };
                if selected.iter().any(|(node, selected_tier)| {
                    market_site(
                        routes,
                        graph.nodes[*node].x,
                        graph.nodes[*node].z,
                        &tiers[*selected_tier],
                        settlement,
                        road_width_m,
                    )
                    .is_some_and(|other| {
                        footprints_overlap(candidate_site.footprint, other.footprint, 2.0)
                    })
                }) {
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
                        if minutes <= tier.max_travel_minutes {
                            (point.demand - served[index]).max(0.0)
                        } else {
                            0.0
                        }
                    })
                    .sum::<f64>()
                    * tier.preference;
                let order = (candidate, tier_index);
                if best.is_none_or(|(score, prior_candidate, prior_tier)| {
                    reachable > score
                        || (reachable == score && order < (prior_candidate, prior_tier))
                }) {
                    best = Some((reachable, candidate, tier_index));
                }
            }
        }
        let Some((reachable, candidate, tier_index)) = best else {
            break;
        };
        if reachable <= 0.0 {
            break;
        }
        selected.push((candidate, tier_index));
        tier_counts[tier_index] += 1;
        let tier = &tiers[tier_index];
        let times = graph.times_from(candidate, program.walk_speed_mps);
        let mut capacity = tier.capacity;
        let mut ranked: Vec<(usize, f64)> = demand_points
            .iter()
            .enumerate()
            .filter_map(|(index, point)| {
                let minutes = times[demand_nodes[index]]
                    + point.access_walk_m / program.walk_speed_mps / 60.0;
                (minutes <= tier.max_travel_minutes && served[index] < point.demand)
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
    for (index, &(node, tier_index)) in selected.iter().enumerate() {
        let tier = &tiers[tier_index];
        let times = graph.times_from(node, program.walk_speed_mps);
        let site = market_site(
            routes,
            graph.nodes[node].x,
            graph.nodes[node].z,
            tier,
            settlement,
            road_width_m,
        )
        .expect("a selected market site remains feasible");
        markets.push(Market {
            id: format!("market.{index}"),
            tier: tier.id.clone(),
            kind: tier.kind.clone(),
            world_x: site.center_x.round() as i32,
            world_z: site.center_z.round() as i32,
            capacity: tier.capacity,
            quality: tier.quality,
            max_travel_minutes: tier.max_travel_minutes,
            width_m: market_size(tier).0,
            depth_m: market_size(tier).1,
            height_m: market_size(tier).2,
            load: 0.0,
        });
        all_times.push(times);
    }
    let assessment = assess_markets(
        &program,
        &markets,
        &demand_points,
        &demand_nodes,
        &all_times,
    );
    for (market, load) in markets.iter_mut().zip(assessment.market_loads) {
        market.load = load;
    }
    Ok(Some(Plan {
        format: "glyphweave.market-catchment-plan".into(),
        version: 1,
        status: if assessment.failures.is_empty() {
            "pass".into()
        } else {
            "incomplete".into()
        },
        model: "capacity-quality-exp-decay-over-route-network".into(),
        beta: program.beta,
        total_demand,
        served_demand: assessment.served_demand,
        coverage_ratio: assessment.coverage_ratio,
        markets,
        demand_points,
        accessibility: assessment.accessibility,
        failures: assessment.failures,
    }))
}

fn assess_markets(
    program: &Program,
    markets: &[Market],
    demand_points: &[DemandPoint],
    demand_nodes: &[usize],
    all_times: &[Vec<f64>],
) -> Assessment {
    let total_demand: f64 = demand_points.iter().map(|point| point.demand).sum();
    let mut accessibility = Vec::new();
    let mut market_loads = vec![0.0_f64; markets.len()];
    let mut served_by_demand = vec![0.0_f64; demand_points.len()];
    let mut demand_order = (0..demand_points.len()).collect::<Vec<_>>();
    demand_order.sort_by(|left, right| {
        let best = |index: usize| {
            markets
                .iter()
                .enumerate()
                .map(|(market_index, market)| {
                    all_times[market_index][demand_nodes[index]]
                        + demand_points[index].access_walk_m / program.walk_speed_mps / 60.0
                        - market.max_travel_minutes
                })
                .fold(f64::INFINITY, f64::min)
        };
        best(*left)
            .partial_cmp(&best(*right))
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.cmp(right))
    });
    for demand_index in demand_order {
        let point = &demand_points[demand_index];
        let mut best_market = None;
        let mut best_minutes = f64::INFINITY;
        for (market_index, market) in markets.iter().enumerate() {
            let minutes = all_times[market_index][demand_nodes[demand_index]]
                + point.access_walk_m / program.walk_speed_mps / 60.0;
            if minutes <= market.max_travel_minutes && minutes < best_minutes {
                best_minutes = minutes;
                best_market = Some(market_index);
            }
        }
        let mut remaining = point.demand;
        while remaining > 1e-9 {
            let eligible = markets
                .iter()
                .enumerate()
                .filter_map(|(market_index, market)| {
                    let minutes = all_times[market_index][demand_nodes[demand_index]]
                        + point.access_walk_m / program.walk_speed_mps / 60.0;
                    let available = (market.capacity - market_loads[market_index]).max(0.0);
                    (minutes <= market.max_travel_minutes && available > 1e-9).then_some((
                        market_index,
                        market.capacity * market.quality * (-program.beta * minutes).exp(),
                        available,
                    ))
                })
                .collect::<Vec<_>>();
            let total_weight: f64 = eligible.iter().map(|(_, weight, _)| weight).sum();
            if eligible.is_empty() || total_weight <= 1e-9 {
                break;
            }
            let mut round_remaining = remaining;
            let mut granted = 0.0;
            for (market_index, weight, available) in eligible {
                let grant = (round_remaining * weight / total_weight)
                    .min(available)
                    .min(round_remaining);
                market_loads[market_index] += grant;
                round_remaining -= grant;
                granted += grant;
            }
            if granted <= 1e-9 {
                break;
            }
            remaining = round_remaining;
        }
        let assigned = point.demand - remaining;
        served_by_demand[demand_index] = assigned;
        let density: f64 = markets
            .iter()
            .enumerate()
            .map(|(market_index, market)| {
                let minutes = all_times[market_index][demand_nodes[demand_index]]
                    + point.access_walk_m / program.walk_speed_mps / 60.0;
                if minutes <= market.max_travel_minutes {
                    market.capacity * (-program.beta * minutes).exp()
                } else {
                    0.0
                }
            })
            .sum();
        accessibility.push(Accessibility {
            demand_id: point.id.clone(),
            nearest_market_id: best_market.map(|index| markets[index].id.clone()),
            travel_minutes: if best_market.is_some() {
                best_minutes
            } else {
                f64::INFINITY
            },
            assigned_share: (assigned / point.demand.max(1e-9)).clamp(0.0, 1.0),
            local_supply_density: density,
            deficit: remaining.max(0.0),
        });
    }
    let served_demand: f64 = served_by_demand.iter().sum();
    let coverage_ratio = served_demand / total_demand.max(1e-9);
    let mut failures = Vec::new();
    if coverage_ratio < program.coverage_target {
        failures.push(format!(
            "coverage {:.3} below target {:.3}",
            coverage_ratio, program.coverage_target
        ));
    }
    Assessment {
        market_loads,
        accessibility,
        served_demand,
        coverage_ratio,
        failures,
    }
}

pub fn reconcile_after_access(
    plan: &Plan,
    routes: &RoutePlan,
    style: &Value,
    realized_building_ids: &BTreeSet<String>,
) -> Result<Option<Plan>, String> {
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    let graph = Graph::from_routes(routes);
    let demand_points = plan
        .demand_points
        .iter()
        .filter(|point| realized_building_ids.contains(&point.id))
        .cloned()
        .collect::<Vec<_>>();
    let demand_nodes = demand_points
        .iter()
        .map(|point| {
            graph
                .nearest_node(f64::from(point.world_x), f64::from(point.world_z))
                .0
        })
        .collect::<Vec<_>>();
    let mut markets = plan.markets.clone();
    let all_times = markets
        .iter()
        .map(|market| {
            let node = graph
                .nearest_node(f64::from(market.world_x), f64::from(market.world_z))
                .0;
            graph.times_from(node, program.walk_speed_mps)
        })
        .collect::<Vec<_>>();
    let assessment = assess_markets(&program, &markets, &demand_points, &demand_nodes, &all_times);
    for (market, load) in markets.iter_mut().zip(assessment.market_loads) {
        market.load = load;
    }
    Ok(Some(Plan {
        format: plan.format.clone(),
        version: plan.version,
        status: if assessment.failures.is_empty() {
            "pass".into()
        } else {
            "incomplete".into()
        },
        model: format!("{}-post-access", plan.model),
        beta: program.beta,
        total_demand: demand_points.iter().map(|point| point.demand).sum(),
        served_demand: assessment.served_demand,
        coverage_ratio: assessment.coverage_ratio,
        markets,
        demand_points,
        accessibility: assessment.accessibility,
        failures: assessment.failures,
    }))
}

pub fn materialize(plan: &Plan, routes: &RoutePlan, terrain: &BakedTerrain) -> Vec<EntityInstance> {
    plan.markets
        .iter()
        .filter_map(|market| {
            let station = nearest_station(routes, market.world_x, market.world_z)?;
            // The plan already resolved a road-width-aware, collision-free site.
            // Re-deriving the old 12 m offset here silently moved the market
            // back onto the widened road after planning.
            let center_x = f64::from(market.world_x);
            let center_z = f64::from(market.world_z);
            let dx = station.x - center_x;
            let dz = station.z - center_z;
            let (sin, cos) = f64::from(station.rotation_y_deg).to_radians().sin_cos();
            let local_x = dx * cos - dz * sin;
            let local_z = dx * sin + dz * cos;
            let tx = if local_x.abs() > 1e-9 {
                f64::from(market.width_m) * 0.5 / local_x.abs()
            } else {
                f64::INFINITY
            };
            let tz = if local_z.abs() > 1e-9 {
                f64::from(market.depth_m) * 0.5 / local_z.abs()
            } else {
                f64::INFINITY
            };
            let edge_t = tx.min(tz).min(1.0);
            let anchor_x = center_x + dx * edge_t;
            let anchor_z = center_z + dz * edge_t;
            let direction = if dx.abs() > dz.abs() {
                if dx > 0.0 { "east" } else { "west" }
            } else if dz > 0.0 {
                "south"
            } else {
                "north"
            };
            let world_y = terrain
                .height(center_x.round() as i32, center_z.round() as i32)?
                .ceil() as i32;
            Some(EntityInstance {
                entity_id: format!("generated.market-catchment.{}", market.id),
                asset_id: format!("building.market.{}", market.tier),
                kind: "market".into(),
                world_x: center_x.round() as i32,
                world_z: center_z.round() as i32,
                world_y,
                scale: 1.0,
                width_m: market.width_m,
                depth_m: market.depth_m,
                height_m: market.height_m,
                rotation_y_deg: station.rotation_y_deg,
                grounding: super::GroundingSpec::default(),
                anchors: vec![SpatialAnchor {
                    id: "front".into(),
                    world_x: anchor_x.round() as i32,
                    world_z: anchor_z.round() as i32,
                    direction: direction.into(),
                    target: Some("road".into()),
                }],
                bounds: None,
            })
        })
        .collect()
}

fn market_size(tier: &Tier) -> (f32, f32, f32) {
    match tier.id.as_str() {
        "street-market" => (24.0, 16.0, 6.0),
        "district" => (72.0, 54.0, 15.0),
        _ => (48.0, 32.0, 10.0),
    }
}

#[derive(Clone, Copy)]
struct Footprint {
    min_x: f64,
    max_x: f64,
    min_z: f64,
    max_z: f64,
}

#[derive(Clone, Copy)]
struct MarketSite {
    center_x: f64,
    center_z: f64,
    footprint: Footprint,
}

fn market_site(
    routes: &RoutePlan,
    x: i32,
    z: i32,
    tier: &Tier,
    settlement: &SettlementPlan,
    road_width_m: f64,
) -> Option<MarketSite> {
    let station = nearest_station(routes, x, z)?;
    let (width, depth, _) = market_size(tier);
    let (sin, cos) = f64::from(station.rotation_y_deg).to_radians().sin_cos();
    let half_x = f64::from(width) * 0.5 * cos.abs() + f64::from(depth) * 0.5 * sin.abs();
    let half_z = f64::from(width) * 0.5 * sin.abs() + f64::from(depth) * 0.5 * cos.abs();
    for side in [1.0, -1.0] {
        let road_clearance = road_width_m * 0.5 + 3.0 + 2.0;
        let center_x =
            station.x + station.normal_x * side * (road_clearance + f64::from(depth) * 0.5);
        let center_z =
            station.z + station.normal_z * side * (road_clearance + f64::from(depth) * 0.5);
        let footprint = Footprint {
            min_x: center_x - half_x,
            max_x: center_x + half_x,
            min_z: center_z - half_z,
            max_z: center_z + half_z,
        };
        let oriented = OrientedFootprint {
            center_x,
            center_z,
            width_m: f64::from(width),
            depth_m: f64::from(depth),
            rotation_y_deg: f64::from(station.rotation_y_deg),
        };
        let collides_with_planned_building = settlement.buildings.iter().any(|building| {
            let building_footprint = OrientedFootprint {
                center_x: f64::from(building.world_x),
                center_z: f64::from(building.world_z),
                width_m: f64::from(building.width_m),
                depth_m: f64::from(building.depth_m),
                rotation_y_deg: f64::from(building.rotation_y_deg),
            };
            oriented_footprints_overlap(&oriented, &building_footprint, 2.0)
        });
        let collides_with_road = routes
            .routes
            .iter()
            .filter(|route| route.status == "pass")
            .flat_map(|route| route.polyline.windows(2))
            .any(|pair| {
                let dx = f64::from(pair[1].x - pair[0].x);
                let dz = f64::from(pair[1].z - pair[0].z);
                let length = dx.hypot(dz);
                if length <= f64::EPSILON {
                    return false;
                }
                let road = OrientedFootprint {
                    center_x: (f64::from(pair[0].x) + f64::from(pair[1].x)) * 0.5,
                    center_z: (f64::from(pair[0].z) + f64::from(pair[1].z)) * 0.5,
                    width_m: length,
                    depth_m: road_width_m + 6.0,
                    rotation_y_deg: dz.atan2(dx).to_degrees(),
                };
                oriented_footprints_overlap(&oriented, &road, 0.5)
            });
        if !collides_with_planned_building && !collides_with_road {
            return Some(MarketSite {
                center_x,
                center_z,
                footprint,
            });
        }
    }
    None
}

fn footprints_overlap(a: Footprint, b: Footprint, margin_m: f64) -> bool {
    a.min_x <= b.max_x + margin_m
        && a.max_x + margin_m >= b.min_x
        && a.min_z <= b.max_z + margin_m
        && a.max_z + margin_m >= b.min_z
}

struct Station {
    x: f64,
    z: f64,
    normal_x: f64,
    normal_z: f64,
    rotation_y_deg: f32,
}

fn nearest_station(routes: &RoutePlan, x: i32, z: i32) -> Option<Station> {
    let mut best: Option<(f64, Station)> = None;
    for route in routes.routes.iter().filter(|route| route.status == "pass") {
        for pair in route.polyline.windows(2) {
            let ax = f64::from(pair[0].x);
            let az = f64::from(pair[0].z);
            let bx = f64::from(pair[1].x);
            let bz = f64::from(pair[1].z);
            let dx = bx - ax;
            let dz = bz - az;
            let length_squared = dx * dx + dz * dz;
            if length_squared <= 1e-9 {
                continue;
            }
            let t = (((f64::from(x) - ax) * dx + (f64::from(z) - az) * dz) / length_squared)
                .clamp(0.0, 1.0);
            let station_x = ax + dx * t;
            let station_z = az + dz * t;
            let distance = (f64::from(x) - station_x).hypot(f64::from(z) - station_z);
            if best
                .as_ref()
                .is_some_and(|(prior_distance, _)| *prior_distance <= distance)
            {
                continue;
            }
            let length = length_squared.sqrt();
            best = Some((
                distance,
                Station {
                    x: station_x,
                    z: station_z,
                    normal_x: -dz / length,
                    normal_z: dx / length,
                    rotation_y_deg: dz.atan2(dx).to_degrees() as f32,
                },
            ));
        }
    }
    best.map(|(_, station)| station)
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
    use crate::height_payload::encode_millimetres;
    use crate::worldgen::route_evolution::{Anchor, Route, RoutePoint};
    use crate::worldgen::settlement_evolution::{
        Building, Capacity, Demand, Plan as SettlementPlan,
    };
    use crate::worldgen::SceneSpec;

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
            candidate_nodes: Vec::new(),
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
                ..Building::default()
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
            district_allocation: Vec::new(),
            street_frontage: Default::default(),
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
        assert!(plan.markets.iter().all(|market| market.tier == "market"));
        assert_eq!(
            plan.markets
                .iter()
                .map(|market| market.kind.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["mixed-market"])
        );
    }

    #[test]
    fn tiered_markets_compete_on_capacity_time_and_type() {
        let style = serde_json::json!({
            "marketCatchment": {
                "version": 1,
                "beta": 0.08,
                "maxTravelMinutes": 15,
                "walkSpeedMps": 1.3,
                "marketCapacity": 160,
                "marketQuality": 1,
                "coverageTarget": 0.95,
                "maxMarkets": 3,
                "candidateSpacingM": 100,
                "tiers": [
                    {
                        "id": "street-market",
                        "kind": "community-market-hall",
                        "capacity": 40,
                        "quality": 1.1,
                        "maxTravelMinutes": 8,
                        "maxCount": 1,
                        "candidateSpacingM": 80,
                        "preference": 1.2
                    },
                    {
                        "id": "neighbourhood",
                        "kind": "neighbourhood-supermarket",
                        "capacity": 180,
                        "quality": 1.0,
                        "maxTravelMinutes": 15,
                        "maxCount": 1,
                        "candidateSpacingM": 160,
                        "preference": 1.0
                    }
                ]
            }
        });
        let plan = plan(&route_plan(), &settlement(), &style).unwrap().unwrap();
        assert_eq!(plan.status, "pass");
        assert!(plan.markets.len() >= 2);
        let tiers = plan
            .markets
            .iter()
            .map(|market| market.tier.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(tiers.contains("street-market"));
        assert!(tiers.contains("neighbourhood"));
        assert!(plan
            .markets
            .iter()
            .all(|market| market.load <= market.capacity + 1e-6));
        let settlement = settlement();
        for (index, market) in plan.markets.iter().enumerate() {
            for other in plan.markets.iter().skip(index + 1) {
                let left = market_site(
                    &route_plan(),
                    market.world_x,
                    market.world_z,
                    &tiers_for_test(&market.tier),
                    &settlement,
                    8.0,
                )
                .unwrap()
                .footprint;
                let right = market_site(
                    &route_plan(),
                    other.world_x,
                    other.world_z,
                    &tiers_for_test(&other.tier),
                    &settlement,
                    8.0,
                )
                .unwrap()
                .footprint;
                assert!(!footprints_overlap(left, right, 2.0));
            }
        }
    }

    #[test]
    fn market_site_yields_to_a_planned_building_on_the_near_side() {
        let settlement = settlement();
        let tier = tiers_for_test("street-market");
        let site = market_site(&route_plan(), 100, 256, &tier, &settlement, 8.0).unwrap();

        assert!(site.center_z < 256.0);
        for building in &settlement.buildings {
            let building_footprint = Footprint {
                min_x: f64::from(building.world_x) - f64::from(building.width_m) * 0.5,
                max_x: f64::from(building.world_x) + f64::from(building.width_m) * 0.5,
                min_z: f64::from(building.world_z) - f64::from(building.depth_m) * 0.5,
                max_z: f64::from(building.world_z) + f64::from(building.depth_m) * 0.5,
            };
            assert!(!footprints_overlap(site.footprint, building_footprint, 2.0));
        }
    }

    #[test]
    fn post_access_reconciliation_removes_rejected_demand() {
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
        let planned = plan(&route_plan(), &settlement(), &style)
            .unwrap()
            .unwrap();
        let rejected = planned.demand_points[0].id.clone();
        let realized = planned
            .demand_points
            .iter()
            .filter(|point| point.id != rejected)
            .map(|point| point.id.clone())
            .collect::<BTreeSet<_>>();

        let reconciled =
            reconcile_after_access(&planned, &route_plan(), &style, &realized)
                .unwrap()
                .unwrap();

        assert_eq!(reconciled.demand_points.len(), planned.demand_points.len() - 1);
        assert_eq!(
            reconciled.accessibility.len(),
            reconciled.demand_points.len()
        );
        assert_eq!(reconciled.markets.len(), planned.markets.len());
        assert!(reconciled.model.ends_with("-post-access"));
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

    #[test]
    fn markets_preserve_planned_geometry_and_route_entrances() {
        let scene = SceneSpec {
            scene_id: "markets".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let payload =
            encode_millimetres(512, 512, (0..512).flat_map(|_| (0..512).map(|_| 0.0))).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain.insert_chunk(0, 0, 512, 512, &payload).unwrap();
        let routes = route_plan();
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
        let planned = plan(&routes, &settlement(), &style).unwrap().unwrap();
        let entities = materialize(&planned, &routes, &terrain);
        assert_eq!(entities.len(), planned.markets.len());
        assert!(entities.iter().all(|entity| {
            entity.kind == "market"
                && entity.anchors.len() == 1
                && entity.width_m > 0.0
                && entity.depth_m > 0.0
        }));
    }

    fn tiers_for_test(id: &str) -> Tier {
        Tier {
            id: id.into(),
            kind: match id {
                "street-market" => "community-market-hall",
                "district" => "large-supermarket",
                _ => "neighbourhood-supermarket",
            }
            .into(),
            capacity: 100.0,
            quality: 1.0,
            max_travel_minutes: 15.0,
            max_count: 1,
            candidate_spacing_m: 80.0,
            preference: 1.0,
        }
    }
}
