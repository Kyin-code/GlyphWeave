//! Fresh-food market accessibility over the route graph.
//!
//! This module never uses straight-line radius as service evidence. Demand is
//! attached to the settlement plan, markets are placed on route nodes, and
//! travel time is measured through the actual route polyline.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::BakedTerrain;

use super::route_evolution::Plan as RoutePlan;
use super::settlement_evolution::Plan as SettlementPlan;
use super::{
    oriented_footprints_overlap, urban_slice, EntityInstance, OrientedFootprint, SpatialAnchor,
    WaterGeometry,
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
    #[serde(default)]
    pub rotation_y_deg: f32,
    /// Route point chosen by the market planner. Recomputing it from a rounded
    /// market centre may snap to a different segment and create a false long
    /// stair or a collision that was not present in the plan.
    #[serde(default)]
    pub frontage_road_point: Option<[i32; 2]>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarketAllocation {
    pub market_id: String,
    pub minutes: f64,
    pub weight: f64,
    pub demand: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceBand {
    pub minutes: f64,
    pub accessible_capacity: f64,
    pub competing_demand: f64,
    pub assigned_demand: f64,
    pub coverage_ratio: f64,
    pub capacity_per_demand: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DemandRadiation {
    pub demand_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub demand: f64,
    pub served_demand: f64,
    pub deficit: f64,
    pub nearest_market_id: Option<String>,
    pub allocations: Vec<MarketAllocation>,
    pub bands: Vec<ServiceBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandSummary {
    pub minutes: f64,
    pub demand: f64,
    pub assigned_demand: f64,
    pub coverage_ratio: f64,
    pub accessible_capacity: f64,
    pub capacity_per_demand: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarketReach {
    pub minutes: f64,
    pub demand_points: usize,
    pub demand: f64,
    pub assigned_demand: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarketRadiation {
    pub market_id: String,
    pub tier: String,
    pub capacity: f64,
    pub load: f64,
    pub utilization: f64,
    pub assigned_demand: f64,
    pub bands: Vec<MarketReach>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RadiationReport {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub status: String,
    pub model: String,
    pub beta: f64,
    pub coverage_target: f64,
    pub total_demand: f64,
    pub served_demand: f64,
    pub coverage_ratio: f64,
    pub bands: Vec<BandSummary>,
    pub demand_fields: Vec<DemandRadiation>,
    pub markets: Vec<MarketRadiation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DemandDeficit {
    pub demand_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub demand: f64,
    pub served_demand: f64,
    pub deficit: f64,
    pub nearest_market_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeficitReport {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub status: String,
    pub model: String,
    pub coverage_target: f64,
    pub total_demand: f64,
    pub total_deficit: f64,
    pub records: Vec<DemandDeficit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifacts {
    pub plan: Plan,
    pub radiation: RadiationReport,
    pub deficits: DeficitReport,
}

struct Assessment {
    market_loads: Vec<f64>,
    accessibility: Vec<Accessibility>,
    allocations: Vec<Vec<MarketAllocation>>,
    served_demand: f64,
    coverage_ratio: f64,
    failures: Vec<String>,
}

const SERVICE_BANDS_MINUTES: [f64; 4] = [5.0, 10.0, 15.0, 30.0];

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
    Ok(plan_with_artifacts(routes, settlement, style, "")?.map(|artifacts| artifacts.plan))
}

pub fn plan_with_artifacts(
    routes: &RoutePlan,
    settlement: &SettlementPlan,
    style: &Value,
    scene_id: &str,
) -> Result<Option<Artifacts>, String> {
    plan_with_artifacts_inner(routes, settlement, style, scene_id, None)
}

pub fn plan_with_water(
    routes: &RoutePlan,
    settlement: &SettlementPlan,
    style: &Value,
    scene_id: &str,
    water: WaterGeometry,
) -> Result<Option<Artifacts>, String> {
    plan_with_artifacts_inner(routes, settlement, style, scene_id, Some(water))
}

fn plan_with_artifacts_inner(
    routes: &RoutePlan,
    settlement: &SettlementPlan,
    style: &Value,
    scene_id: &str,
    water: Option<WaterGeometry>,
) -> Result<Option<Artifacts>, String> {
    validate_style(style)?;
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    let road_width_m =
        f64::from(urban_slice::planned_road_width_m(style)?.unwrap_or(routes.road_width_m as f32));
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
    // The post-access pass can remove demand footprints whose entrances fail
    // or whose nearest graph node is no longer reachable. Stopping exactly at
    // the authored target leaves no spare market coverage for that second
    // topology, so plan a small deployment reserve without weakening the
    // published target itself.
    let selection_target = (program.coverage_target + 0.02).min(0.96);
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
                if near_stream_seam(graph.nodes[candidate].x, graph.nodes[candidate].z, 48) {
                    continue;
                }
                let Some(candidate_site) = market_site(
                    routes,
                    graph.nodes[candidate].x,
                    graph.nodes[candidate].z,
                    tier,
                    settlement,
                    road_width_m,
                    water,
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
                        water,
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
        if covered / total_demand >= selection_target {
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
            water,
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
            rotation_y_deg: site.rotation_y_deg,
            frontage_road_point: Some([
                site.station_x.round() as i32,
                site.station_z.round() as i32,
            ]),
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
    let plan = Plan {
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
    };
    Ok(Some(build_artifacts(
        routes,
        &program,
        plan,
        &demand_nodes,
        &all_times,
        &assessment.allocations,
        scene_id,
    )))
}

fn build_artifacts(
    _routes: &RoutePlan,
    program: &Program,
    plan: Plan,
    demand_nodes: &[usize],
    all_times: &[Vec<f64>],
    allocations: &[Vec<MarketAllocation>],
    scene_id: &str,
) -> Artifacts {
    let nearest_by_id: BTreeMap<&str, &Accessibility> = plan
        .accessibility
        .iter()
        .map(|accessibility| (accessibility.demand_id.as_str(), accessibility))
        .collect();
    let mut demand_fields = Vec::with_capacity(plan.demand_points.len());
    let mut band_totals = SERVICE_BANDS_MINUTES
        .iter()
        .map(|minutes| (*minutes, 0.0_f64, 0.0_f64, 0.0_f64))
        .collect::<Vec<_>>();

    for (demand_index, point) in plan.demand_points.iter().enumerate() {
        let point_allocations = allocations.get(demand_index).cloned().unwrap_or_default();
        let served_demand = point_allocations
            .iter()
            .map(|allocation| allocation.demand)
            .sum::<f64>()
            .min(point.demand);
        let deficit = (point.demand - served_demand).max(0.0);
        let nearest_market_id = nearest_by_id
            .get(point.id.as_str())
            .and_then(|accessibility| accessibility.nearest_market_id.clone());
        let mut bands = Vec::with_capacity(SERVICE_BANDS_MINUTES.len());
        for (band_index, band_minutes) in SERVICE_BANDS_MINUTES.iter().enumerate() {
            let reachable = plan
                .markets
                .iter()
                .enumerate()
                .filter_map(|(market_index, market)| {
                    let minutes = all_times[market_index][demand_nodes[demand_index]]
                        + point.access_walk_m / program.walk_speed_mps / 60.0;
                    (minutes <= *band_minutes && minutes <= market.max_travel_minutes)
                        .then_some((market_index, minutes))
                })
                .collect::<Vec<_>>();
            let accessible_capacity = reachable
                .iter()
                .map(|(market_index, minutes)| {
                    let market = &plan.markets[*market_index];
                    market.capacity * market.quality * (-program.beta * minutes).exp()
                })
                .sum::<f64>();
            let assigned_demand = point_allocations
                .iter()
                .filter(|allocation| allocation.minutes <= *band_minutes)
                .map(|allocation| allocation.demand)
                .sum::<f64>();
            let competing_demand = reachable
                .iter()
                .map(|(market_index, _)| {
                    plan.demand_points
                        .iter()
                        .enumerate()
                        .filter(|(other_index, other)| {
                            *other_index != demand_index
                                && all_times[*market_index][demand_nodes[*other_index]]
                                    + other.access_walk_m / program.walk_speed_mps / 60.0
                                    <= plan.markets[*market_index].max_travel_minutes
                        })
                        .map(|(_, other)| other.demand)
                        .sum::<f64>()
                })
                .sum::<f64>();
            let coverage_ratio = (assigned_demand / point.demand.max(1e-9)).clamp(0.0, 1.0);
            let capacity_per_demand = accessible_capacity / point.demand.max(1e-9);
            bands.push(ServiceBand {
                minutes: *band_minutes,
                accessible_capacity,
                competing_demand,
                assigned_demand,
                coverage_ratio,
                capacity_per_demand,
            });
            band_totals[band_index].1 += point.demand;
            band_totals[band_index].2 += assigned_demand;
            band_totals[band_index].3 += accessible_capacity;
        }
        demand_fields.push(DemandRadiation {
            demand_id: point.id.clone(),
            world_x: point.world_x,
            world_z: point.world_z,
            demand: point.demand,
            served_demand,
            deficit,
            nearest_market_id,
            allocations: point_allocations,
            bands,
        });
    }

    let bands = band_totals
        .into_iter()
        .map(
            |(minutes, demand, assigned_demand, accessible_capacity)| BandSummary {
                minutes,
                demand,
                assigned_demand,
                coverage_ratio: (assigned_demand / demand.max(1e-9)).clamp(0.0, 1.0),
                accessible_capacity,
                capacity_per_demand: accessible_capacity / demand.max(1e-9),
            },
        )
        .collect::<Vec<_>>();

    let market_radiation = plan
        .markets
        .iter()
        .enumerate()
        .map(|(market_index, market)| {
            let assigned_demand = allocations
                .iter()
                .flatten()
                .filter(|allocation| allocation.market_id == market.id)
                .map(|allocation| allocation.demand)
                .sum::<f64>();
            let reach = SERVICE_BANDS_MINUTES
                .iter()
                .map(|minutes| {
                    let points = (0..plan.demand_points.len())
                        .filter(|demand_index| {
                            all_times[market_index][demand_nodes[*demand_index]]
                                + plan.demand_points[*demand_index].access_walk_m
                                    / program.walk_speed_mps
                                    / 60.0
                                <= *minutes
                        })
                        .collect::<Vec<_>>();
                    MarketReach {
                        minutes: *minutes,
                        demand_points: points.len(),
                        demand: points
                            .iter()
                            .map(|index| plan.demand_points[*index].demand)
                            .sum(),
                        assigned_demand: allocations
                            .iter()
                            .flatten()
                            .filter(|allocation| {
                                allocation.market_id == market.id && allocation.minutes <= *minutes
                            })
                            .map(|allocation| allocation.demand)
                            .sum(),
                    }
                })
                .collect();
            MarketRadiation {
                market_id: market.id.clone(),
                tier: market.tier.clone(),
                capacity: market.capacity,
                load: market.load,
                utilization: (market.load / market.capacity.max(1e-9)).clamp(0.0, 1.0),
                assigned_demand,
                bands: reach,
            }
        })
        .collect::<Vec<_>>();

    let deficits = demand_fields
        .iter()
        .filter(|point| point.deficit > 1e-6)
        .map(|point| DemandDeficit {
            demand_id: point.demand_id.clone(),
            world_x: point.world_x,
            world_z: point.world_z,
            demand: point.demand,
            served_demand: point.served_demand,
            deficit: point.deficit,
            nearest_market_id: point.nearest_market_id.clone(),
            reason: if point.nearest_market_id.is_none() {
                "no_market_within_travel_limit".into()
            } else {
                "eligible_market_capacity_exhausted".into()
            },
        })
        .collect::<Vec<_>>();

    Artifacts {
        radiation: RadiationReport {
            format: "glyphweave.market-radiation".into(),
            version: 1,
            scene_id: scene_id.to_string(),
            status: plan.status.clone(),
            model: "network-time-bands-capacity-competition-v1".into(),
            beta: program.beta,
            coverage_target: program.coverage_target,
            total_demand: plan.total_demand,
            served_demand: plan.served_demand,
            coverage_ratio: plan.coverage_ratio,
            bands,
            demand_fields,
            markets: market_radiation,
        },
        deficits: DeficitReport {
            format: "glyphweave.market-deficit".into(),
            version: 1,
            scene_id: scene_id.to_string(),
            status: plan.status.clone(),
            model: "network-time-capacity-gap-v1".into(),
            coverage_target: program.coverage_target,
            total_demand: plan.total_demand,
            total_deficit: deficits.iter().map(|record| record.deficit).sum(),
            records: deficits,
        },
        plan,
    }
}

pub fn audit_artifacts(artifacts: &Artifacts) -> Result<(), String> {
    if artifacts.plan.format != "glyphweave.market-catchment-plan"
        || artifacts.radiation.format != "glyphweave.market-radiation"
        || artifacts.deficits.format != "glyphweave.market-deficit"
    {
        return Err("market artifact format does not match the published contract".into());
    }
    if artifacts.plan.status != artifacts.radiation.status
        || artifacts.plan.status != artifacts.deficits.status
    {
        return Err("market plan, radiation and deficit statuses disagree".into());
    }
    if artifacts.radiation.scene_id != artifacts.deficits.scene_id {
        return Err("market radiation and deficit reports name different scenes".into());
    }
    if artifacts.radiation.bands.len() != SERVICE_BANDS_MINUTES.len()
        || artifacts
            .radiation
            .bands
            .iter()
            .zip(SERVICE_BANDS_MINUTES)
            .any(|(band, expected)| band.minutes != expected)
    {
        return Err("market radiation time bands are missing or out of order".into());
    }
    let demand: f64 = artifacts
        .radiation
        .demand_fields
        .iter()
        .map(|field| field.demand)
        .sum();
    let served: f64 = artifacts
        .radiation
        .demand_fields
        .iter()
        .map(|field| field.served_demand)
        .sum();
    let deficit: f64 = artifacts
        .radiation
        .demand_fields
        .iter()
        .map(|field| field.deficit)
        .sum();
    if !close(demand, artifacts.radiation.total_demand)
        || !close(served, artifacts.radiation.served_demand)
        || !close(deficit, artifacts.deficits.total_deficit)
    {
        return Err("market radiation demand totals do not conserve".into());
    }
    for field in &artifacts.radiation.demand_fields {
        let allocated: f64 = field
            .allocations
            .iter()
            .map(|allocation| allocation.demand)
            .sum();
        if !close(allocated, field.served_demand)
            || !close(field.served_demand + field.deficit, field.demand)
        {
            return Err(format!(
                "market allocation conservation failed for {}",
                field.demand_id
            ));
        }
        if field
            .allocations
            .iter()
            .any(|allocation| !allocation.minutes.is_finite() || allocation.demand < 0.0)
        {
            return Err(format!(
                "market allocation contains invalid evidence for {}",
                field.demand_id
            ));
        }
    }
    for market in &artifacts.radiation.markets {
        if market.load > market.capacity + 1e-6 || !(0.0..=1.0 + 1e-9).contains(&market.utilization)
        {
            return Err(format!("market load is invalid for {}", market.market_id));
        }
        let allocated: f64 = artifacts
            .radiation
            .demand_fields
            .iter()
            .flat_map(|field| &field.allocations)
            .filter(|allocation| allocation.market_id == market.market_id)
            .map(|allocation| allocation.demand)
            .sum();
        if !close(allocated, market.assigned_demand) {
            return Err(format!(
                "market assigned demand does not match allocations for {}",
                market.market_id
            ));
        }
    }
    let published_deficits = artifacts
        .radiation
        .demand_fields
        .iter()
        .filter(|field| field.deficit > 1e-6)
        .count();
    if published_deficits != artifacts.deficits.records.len() {
        return Err("market deficit records do not match the radiation field".into());
    }
    Ok(())
}

fn near_stream_seam(x: i32, z: i32, margin_m: i32) -> bool {
    const STREAM_CHUNK_M: i32 = 512;
    let near = |value: i32| {
        let rem = value.rem_euclid(STREAM_CHUNK_M);
        rem < margin_m || rem > STREAM_CHUNK_M - margin_m
    };
    near(x) || near(z)
}

fn close(left: f64, right: f64) -> bool {
    // Demand is accumulated through weighted, capacity-constrained shares.
    // A few micro-units can survive even after the final allocation is posted;
    // this floor rejects real discrepancies while accepting floating-point
    // residue far below one person per market.
    (left - right).abs() <= (1e-5_f64).max(1e-6 * left.abs().max(right.abs()))
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
    let mut allocations = vec![Vec::<MarketAllocation>::new(); demand_points.len()];
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
                if grant > 1e-9 {
                    let minutes = all_times[market_index][demand_nodes[demand_index]]
                        + point.access_walk_m / program.walk_speed_mps / 60.0;
                    let market_id = markets[market_index].id.clone();
                    if let Some(allocation) = allocations[demand_index]
                        .iter_mut()
                        .find(|allocation| allocation.market_id == market_id)
                    {
                        allocation.demand += grant;
                    } else {
                        allocations[demand_index].push(MarketAllocation {
                            market_id,
                            minutes,
                            weight,
                            demand: grant,
                        });
                    }
                }
                round_remaining -= grant;
                granted += grant;
            }
            if granted <= 1e-9 {
                break;
            }
            remaining = round_remaining;
        }
        // Weighted allocation can leave a few nanograms of demand after the
        // final eligible market has effectively absorbed it. Preserve the
        // conservation contract by posting that remainder to the same market
        // instead of emitting a contradictory near-zero deficit.
        if remaining > 0.0 && remaining <= 1e-6 {
            if let Some((market_index, _)) = markets
                .iter()
                .enumerate()
                .find(|(market_index, market)| {
                    let minutes = all_times[*market_index][demand_nodes[demand_index]]
                        + point.access_walk_m / program.walk_speed_mps / 60.0;
                    minutes <= market.max_travel_minutes
                        && market.capacity - market_loads[*market_index] + 1e-6 >= remaining
                })
            {
                let grant = remaining;
                market_loads[market_index] += grant;
                let minutes = all_times[market_index][demand_nodes[demand_index]]
                    + point.access_walk_m / program.walk_speed_mps / 60.0;
                let market_id = markets[market_index].id.clone();
                if let Some(allocation) = allocations[demand_index]
                    .iter_mut()
                    .find(|allocation| allocation.market_id == market_id)
                {
                    allocation.demand += grant;
                } else {
                    allocations[demand_index].push(MarketAllocation {
                        market_id,
                        minutes,
                        weight: 1.0,
                        demand: grant,
                    });
                }
                remaining = 0.0;
            }
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
        allocations,
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
    let realized_market_ids = plan
        .markets
        .iter()
        .map(|market| market.id.clone())
        .collect::<BTreeSet<_>>();
    Ok(reconcile_after_access_with_artifacts(
        plan,
        routes,
        style,
        realized_building_ids,
        &realized_market_ids,
        "",
    )?
    .map(|artifacts| artifacts.plan))
}

pub fn reconcile_after_access_with_artifacts(
    plan: &Plan,
    routes: &RoutePlan,
    style: &Value,
    realized_building_ids: &BTreeSet<String>,
    realized_market_ids: &BTreeSet<String>,
    scene_id: &str,
) -> Result<Option<Artifacts>, String> {
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
    let mut markets = plan
        .markets
        .iter()
        .filter(|market| realized_market_ids.contains(&market.id))
        .cloned()
        .collect::<Vec<_>>();
    let all_times = markets
        .iter()
        .map(|market| {
            let node = graph
                .nearest_node(f64::from(market.world_x), f64::from(market.world_z))
                .0;
            graph.times_from(node, program.walk_speed_mps)
        })
        .collect::<Vec<_>>();
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
    let reconciled = Plan {
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
    };
    Ok(Some(build_artifacts(
        routes,
        &program,
        reconciled,
        &demand_nodes,
        &all_times,
        &assessment.allocations,
        scene_id,
    )))
}

pub fn materialize(plan: &Plan, routes: &RoutePlan, terrain: &BakedTerrain) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for market in &plan.markets {
        let Some(station) = nearest_station(routes, market.world_x, market.world_z) else {
            continue;
        };
        // The plan already resolved a road-width-aware, collision-free site.
        // Re-deriving the old 12 m offset here silently moved the market
        // back onto the widened road after planning.
        let center_x = f64::from(market.world_x);
        let center_z = f64::from(market.world_z);
        let [frontage_x, frontage_z] = market
            .frontage_road_point
            .unwrap_or([station.x.round() as i32, station.z.round() as i32]);
        let dx = f64::from(frontage_x) - center_x;
        let dz = f64::from(frontage_z) - center_z;
        let market_yaw = if market.rotation_y_deg.is_finite() {
            market.rotation_y_deg
        } else {
            station.rotation_y_deg
        };
        let (sin, cos) = f64::from(market_yaw).to_radians().sin_cos();
        // Inverse of the serialized positive-Y rotation:
        // world = (localX*cos + localZ*sin, -localX*sin + localZ*cos).
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
            if dx > 0.0 {
                "east"
            } else {
                "west"
            }
        } else if dz > 0.0 {
            "south"
        } else {
            "north"
        };
        let Some(ground_height_m) = terrain
            .height(center_x.round() as i32, center_z.round() as i32)
            .map(f64::from)
        else {
            continue;
        };
        let world_y = ground_height_m.floor() as i32;
        let bottom_offset_m = (ground_height_m - f64::from(world_y)) as f32;
        let mut building = EntityInstance {
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
            rotation_y_deg: market_yaw,
            grounding: super::GroundingSpec {
                bottom_offset_m,
                ..super::GroundingSpec::default()
            },
            anchors: {
                let mut anchors = vec![SpatialAnchor {
                    id: "front".into(),
                    world_x: anchor_x.round() as i32,
                    world_z: anchor_z.round() as i32,
                    direction: direction.into(),
                    target: Some("road".into()),
                }];
                let side_half = (f64::from(market.width_m) * 0.5 - 0.5).max(0.5);
                for (id, sign) in [("side-left", -1.0_f64), ("side-right", 1.0)] {
                    let side_x = (center_x + cos * sign * side_half).round() as i32;
                    let side_z = (center_z - sin * sign * side_half).round() as i32;
                    let direction = if cos.abs() >= sin.abs() {
                        if cos * sign >= 0.0 { "east" } else { "west" }
                    } else if -sin * sign >= 0.0 {
                        "south"
                    } else {
                        "north"
                    };
                    anchors.push(SpatialAnchor {
                        id: id.into(),
                        world_x: side_x,
                        world_z: side_z,
                        direction: direction.into(),
                        target: Some("road".into()),
                    });
                }
                anchors
            },
            bounds: None,
        };
        let anchor_grid = [anchor_x.round() as i32, anchor_z.round() as i32];
        let station_grid = [frontage_x, frontage_z];
        if let (Some(edge_height), Some(station_height)) = (
            terrain.height(anchor_grid[0], anchor_grid[1]),
            terrain.height(station_grid[0], station_grid[1]),
        ) {
            let delta_m = f64::from(station_height - edge_height).abs();
            if (0.30..=8.0).contains(&delta_m) {
                let low_m = edge_height.min(station_height);
                let high_m = edge_height.max(station_height);
                building.grounding.stair_connection = Some(super::StairConnection {
                    version: 1,
                    stair_id: format!("generated.market-catchment.stairs.{}", market.id),
                    from: anchor_grid,
                    to: station_grid,
                    from_mm: (f64::from(edge_height) * 1000.0).round() as i32,
                    to_mm: (f64::from(station_height) * 1000.0).round() as i32,
                    low_mm: (f64::from(low_m) * 1000.0).round() as i32,
                    high_mm: (f64::from(high_m) * 1000.0).round() as i32,
                    step_count: ((delta_m / 0.18).ceil() as u32).max(1),
                    width_m: 2.4,
                });
            }
        }
        let approach_dx = f64::from(frontage_x) - anchor_x;
        let approach_dz = f64::from(frontage_z) - anchor_z;
        let approach_length = approach_dx.hypot(approach_dz);
        let approach_y = building.ground_height_m();
        entities.push(building);
        if approach_length >= 0.5 {
            entities.push(EntityInstance {
                entity_id: format!("generated.market-catchment.approach.{}", market.id),
                asset_id: "road.generated".into(),
                kind: "road".into(),
                world_x: ((anchor_x + f64::from(frontage_x)) * 0.5).round() as i32,
                world_z: ((anchor_z + f64::from(frontage_z)) * 0.5).round() as i32,
                world_y: approach_y.floor() as i32,
                scale: 1.0,
                width_m: (approach_length + 2.0) as f32,
                depth_m: 4.0,
                height_m: 0.2,
                rotation_y_deg: (-approach_dz).atan2(approach_dx).to_degrees() as f32,
                grounding: super::GroundingSpec {
                    bottom_offset_m: (approach_y - approach_y.floor()) as f32,
                    ..super::GroundingSpec::default()
                },
                anchors: Vec::new(),
                bounds: None,
            });
        }
    }
    let stairs = super::settlement_evolution::materialize_stairs(&entities, terrain);
    entities.extend(stairs);
    entities
}

fn market_size(tier: &Tier) -> (f32, f32, f32) {
    match tier.id.as_str() {
        // A neighbourhood fresh-food hall is a row-scale building, not a
        // suburban supermarket. A 24x16m box consumed two narrow frontages
        // and made dense centres unable to host the required network.
        "street-market" => (16.0, 10.0, 6.0),
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
    station_x: f64,
    station_z: f64,
    footprint: Footprint,
    rotation_y_deg: f32,
}

fn market_site(
    routes: &RoutePlan,
    x: i32,
    z: i32,
    tier: &Tier,
    settlement: &SettlementPlan,
    road_width_m: f64,
    water: Option<WaterGeometry>,
) -> Option<MarketSite> {
    let station = nearest_station(routes, x, z)?;
    let (width, depth, _) = market_size(tier);
    let (sin, cos) = f64::from(station.rotation_y_deg).to_radians().sin_cos();
    let half_x = f64::from(width) * 0.5 * cos.abs() + f64::from(depth) * 0.5 * sin.abs();
    let half_z = f64::from(width) * 0.5 * sin.abs() + f64::from(depth) * 0.5 * cos.abs();
    let route_roads = super::route_evolution::materialize(routes);
    let tangent_x = station.normal_z;
    let tangent_z = -station.normal_x;
    let lateral_step = f64::from(width) + 3.0;
    // Legacy/no-water callers keep the original nearest-lot search exactly. A
    // water-aware scene may step back from a blocked shoreline frontage to
    // find a dry replacement instead of losing that part of the market network.
    let depth_offsets: &[f64] = if water.is_some() {
        &[0.0, 6.0, 12.0, 18.0]
    } else {
        &[0.0]
    };
    // Search across the street, not tens of metres along it. A large tangential
    // shift can step past the route segment and manufacture a 40m stair that the
    // access graph correctly rejects.
    let along_offsets: &[f64] = &[0.0];
    for side in [1.0, -1.0] {
        for extra_depth in depth_offsets.iter().copied() {
            for along_step in along_offsets.iter().copied() {
                let along = along_step * lateral_step;
                let road_clearance = road_width_m * 0.5 + 3.0 + 2.0 + extra_depth;
                let center_x = station.x
                    + tangent_x * along
                    + station.normal_x * side * (road_clearance + f64::from(depth) * 0.5);
                let center_z = station.z
                    + tangent_z * along
                    + station.normal_z * side * (road_clearance + f64::from(depth) * 0.5);
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
                if water.is_some_and(|water| {
                    water.intersects_footprint(
                        center_x,
                        center_z,
                        f64::from(width),
                        f64::from(depth),
                        f64::from(station.rotation_y_deg),
                        2.0,
                    )
                }) {
                    continue;
                }
                let collides_with_planned_building = settlement.buildings.iter().any(|building| {
                    let building_footprint = OrientedFootprint {
                        center_x: f64::from(building.world_x),
                        center_z: f64::from(building.world_z),
                        width_m: f64::from(building.width_m),
                        depth_m: f64::from(building.depth_m),
                        rotation_y_deg: f64::from(building.rotation_y_deg),
                    };
                    oriented_footprints_overlap(&oriented, &building_footprint, 4.0)
                });
                // Check the exact segments that will be published. The planner used
                // raw 1m vertices while the final audit consumed simplified segment
                // OBBs; their chord geometry differed just enough to accept a market
                // that later intersected the baked road.
                let collides_with_road =
                    route_roads
                        .iter()
                        .filter(|road| road.kind == "road")
                        .any(|road| {
                            let influence = OrientedFootprint {
                                center_x: f64::from(road.world_x),
                                center_z: f64::from(road.world_z),
                                width_m: f64::from(road.width_m),
                                depth_m: f64::from(road.depth_m),
                                rotation_y_deg: f64::from(road.rotation_y_deg),
                            };
                    oriented_footprints_overlap(&oriented, &influence, 2.0)
                        });
                if !collides_with_planned_building && !collides_with_road {
                    return Some(MarketSite {
                        center_x,
                        center_z,
                station_x: station.x,
                station_z: station.z,
                        footprint,
                rotation_y_deg: station.rotation_y_deg,
                    });
                }
            }
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
                    rotation_y_deg: (-dz).atan2(dx).to_degrees() as f32,
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
    fn radiation_bands_match_network_allocations_and_expose_deficits() {
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
        let artifacts = plan_with_artifacts(&route_plan(), &settlement(), &style, "market-test")
            .unwrap()
            .unwrap();
        audit_artifacts(&artifacts).expect("the published market artifacts conserve");
        let radiation = &artifacts.radiation;
        assert_eq!(radiation.status, "pass");
        assert_eq!(
            radiation
                .bands
                .iter()
                .map(|band| band.minutes)
                .collect::<Vec<_>>(),
            SERVICE_BANDS_MINUTES.to_vec()
        );
        for pair in radiation.bands.windows(2) {
            assert!(
                pair[1].assigned_demand + 1e-9 >= pair[0].assigned_demand,
                "assigned demand must not shrink as the time band widens"
            );
            assert!(
                pair[1].accessible_capacity + 1e-9 >= pair[0].accessible_capacity,
                "accessible capacity must not shrink as the time band widens"
            );
        }
        assert_eq!(radiation.demand_fields.len(), 8);
        for field in &radiation.demand_fields {
            let allocated: f64 = field.allocations.iter().map(|a| a.demand).sum();
            assert!((allocated - field.served_demand).abs() < 1e-9);
            assert!((field.served_demand + field.deficit - field.demand).abs() < 1e-9);
            assert!(field
                .bands
                .iter()
                .all(|band| band.capacity_per_demand >= 0.0));
        }
        assert!(
            (artifacts.deficits.total_deficit
                - radiation
                    .demand_fields
                    .iter()
                    .map(|field| field.deficit)
                    .sum::<f64>())
            .abs()
                < 1e-9
        );
        let mut forged = artifacts.clone();
        forged.radiation.served_demand += 10.0;
        assert!(
            audit_artifacts(&forged).is_err(),
            "a forged coverage total must not pass the market artifact audit"
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
                    None,
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
                    None,
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
        let site = market_site(&route_plan(), 100, 256, &tier, &settlement, 8.0, None).unwrap();

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
        let planned = plan(&route_plan(), &settlement(), &style).unwrap().unwrap();
        let rejected = planned.demand_points[0].id.clone();
        let realized = planned
            .demand_points
            .iter()
            .filter(|point| point.id != rejected)
            .map(|point| point.id.clone())
            .collect::<BTreeSet<_>>();

        let reconciled = reconcile_after_access(&planned, &route_plan(), &style, &realized)
            .unwrap()
            .unwrap();

        assert_eq!(
            reconciled.demand_points.len(),
            planned.demand_points.len() - 1
        );
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
        let markets = entities
            .iter()
            .filter(|entity| entity.kind == "market")
            .collect::<Vec<_>>();
        let approaches = entities
            .iter()
            .filter(|entity| {
                entity.kind == "road"
                    && entity
                        .entity_id
                        .starts_with("generated.market-catchment.approach.")
            })
            .collect::<Vec<_>>();
        assert_eq!(markets.len(), planned.markets.len());
        assert_eq!(approaches.len(), planned.markets.len());
        assert!(markets.iter().all(|entity| {
            entity.kind == "market"
                && entity.anchors.len() == 3
                && entity
                    .anchors
                    .iter()
                    .any(|anchor| anchor.id == "front")
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
