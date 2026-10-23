//! Terrain-led route planning for the first human stage.
//!
//! Routes are searched on the natural heightfield with slope, curvature, water
//! and bridge penalties. The result is an auditable polyline with a
//! grade-limited longitudinal profile; it is not a straight urban axis and it
//! does not depend on the legacy 64 m settlement grid.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::{BakedTerrain, TerrainError};

use super::{EntityInstance, SceneSpec, WaterGeometry};

const MAX_GRADE: f64 = 0.08;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchor {
    pub id: String,
    pub x: i32,
    pub z: i32,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteRequest {
    pub id: String,
    pub from: String,
    pub to: String,
    /// Optional topological waypoint, for example a ford, pass or bridge
    /// crossing. It is a location constraint, not a straight-line override.
    #[serde(default)]
    pub via: Option<[i32; 2]>,
    #[serde(default)]
    pub road_width_m: Option<u32>,
    #[serde(default)]
    pub allow_bridge: Option<bool>,
    /// Authored hierarchy: expressway, national, county, township, local, lane.
    /// A missing value preserves legacy width-derived sections.
    #[serde(default)]
    pub road_class: Option<String>,
    /// `terrain-following` (default), `straight`, or `straight-preferred`.
    #[serde(default)]
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_grid_step")]
    pub grid_step_m: u32,
    #[serde(default = "default_road_width")]
    pub road_width_m: u32,
    #[serde(default = "default_max_grade")]
    pub max_grade: f64,
    #[serde(default = "default_max_terrain_slope")]
    pub max_terrain_slope: f64,
    #[serde(default = "default_max_fill_m")]
    pub max_fill_m: f64,
    #[serde(default = "default_true")]
    pub allow_bridge: bool,
    #[serde(default = "default_max_bridge_span")]
    pub max_bridge_span_m: f64,
    #[serde(default = "default_turn_penalty")]
    pub turn_penalty: f64,
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub routes: Vec<RouteRequest>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutePoint {
    pub x: i32,
    pub z: i32,
    pub natural_height_m: f64,
    pub design_height_m: f64,
    pub water: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSpan {
    pub start_index: usize,
    pub end_index: usize,
    pub length_m: f64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NaturalNode {
    pub id: String,
    pub x: i32,
    pub z: i32,
    pub quadrant: String,
    pub elevation_m: f64,
    pub slope: f64,
    pub flatness: f64,
    pub valley_score: f64,
    pub water_access: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub id: String,
    pub from: String,
    pub to: String,
    pub status: String,
    pub road_width_m: u32,
    #[serde(default)]
    pub road_class: String,
    #[serde(default)]
    pub alignment: String,
    pub polyline: Vec<RoutePoint>,
    pub length_m: f64,
    pub direct_length_m: f64,
    pub curve_deviation_m: f64,
    pub maximum_grade: f64,
    pub maximum_fill_m: f64,
    pub maximum_terrain_fill_m: f64,
    pub fill_volume_m3: f64,
    pub bridge_spans: Vec<BridgeSpan>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub grid_step_m: u32,
    pub road_width_m: u32,
    pub max_grade: f64,
    pub max_terrain_slope: f64,
    pub max_fill_m: f64,
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub candidate_nodes: Vec<NaturalNode>,
    pub routes: Vec<Route>,
    pub failures: Vec<String>,
}

const fn default_true() -> bool {
    true
}
const fn default_grid_step() -> u32 {
    16
}
const fn default_road_width() -> u32 {
    8
}
const fn default_max_grade() -> f64 {
    MAX_GRADE
}
const fn default_max_terrain_slope() -> f64 {
    0.35
}
const fn default_max_fill_m() -> f64 {
    6.0
}
const fn default_max_bridge_span() -> f64 {
    300.0
}
const fn default_turn_penalty() -> f64 {
    1.4
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
    if style.get("naturalGeography").is_none() {
        return Err("routeEvolution requires style.naturalGeography".into());
    }
    if !(8..=64).contains(&program.grid_step_m)
        || !matches!(program.road_width_m, 4..=32)
        || program.road_width_m % 2 != 0
        || !(0.01..=MAX_GRADE).contains(&program.max_grade)
        || !(0.03..=0.6).contains(&program.max_terrain_slope)
        || !(0.5..=120.0).contains(&program.max_fill_m)
        || !(40.0..=2_000.0).contains(&program.max_bridge_span_m)
        || !(0.0..=8.0).contains(&program.turn_penalty)
    {
        return Err("invalid routeEvolution limits".into());
    }
    if program.max_fill_m > 6.0 && !macro_engineering_authorized(style) {
        return Err(
            "routeEvolution.fill above 6m requires an urbanSlice feature-cut authority".into(),
        );
    }
    if program.anchors.len() < 2 && !program.anchors.is_empty() {
        return Err(
            "routeEvolution anchors must be empty for auto mode or contain at least two anchors"
                .into(),
        );
    }
    let mut ids = std::collections::BTreeSet::new();
    for anchor in &program.anchors {
        if anchor.id.trim().is_empty() || !ids.insert(anchor.id.as_str()) {
            return Err("routeEvolution anchor ids must be non-empty and unique".into());
        }
    }
    for route in &program.routes {
        if route.id.trim().is_empty()
            || !ids.contains(route.from.as_str())
            || !ids.contains(route.to.as_str())
            || route.from == route.to
        {
            return Err("routeEvolution route endpoints must name two different anchors".into());
        }
        if route
            .road_width_m
            .is_some_and(|width| !(4..=32).contains(&width) || width % 2 != 0)
        {
            return Err("routeEvolution route width must be even and 4..=32m".into());
        }
        if let Some(class) = route.road_class.as_deref() {
            let width = route.road_width_m.unwrap_or(program.road_width_m) as f32;
            if super::RoadDesign::for_class(class, width).is_none() {
                return Err(format!(
                    "routeEvolution route {} class {class} does not fit {width}m right-of-way",
                    route.id
                ));
            }
        }
        if route.alignment.as_deref().is_some_and(|alignment| {
            !matches!(
                alignment,
                "terrain-following" | "straight" | "straight-preferred"
            )
        }) {
            return Err(format!(
                "routeEvolution route {} has unsupported alignment",
                route.id
            ));
        }
    }
    Ok(())
}

fn macro_engineering_authorized(style: &Value) -> bool {
    style
        .get("urbanSlice")
        .and_then(|slice| slice.get("mode"))
        .and_then(Value::as_str)
        == Some("feature-cut")
}

fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("routeEvolution") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn plan(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    style: &Value,
) -> Result<Option<Plan>, TerrainError> {
    validate_style(style).map_err(TerrainError)?;
    let Some(program) = from_style(style).map_err(TerrainError)? else {
        return Ok(None);
    };
    if !program.enabled {
        return Ok(None);
    }
    if !terrain.matches_scene(scene) {
        return Err(TerrainError("routeEvolution scene/terrain mismatch".into()));
    }
    terrain.require_complete()?;

    let margin = (program.road_width_m / 2 + 12).max(24) as i32;
    let candidate_nodes = natural_nodes(scene, terrain, water, margin);
    let anchors = if program.anchors.is_empty() {
        auto_anchors(scene, terrain, water, margin, &candidate_nodes)?
    } else {
        program.anchors.clone()
    };
    if anchors.len() < 2 {
        return Err(TerrainError(
            "routeEvolution needs at least two usable anchors".into(),
        ));
    }
    let requests = if program.routes.is_empty() {
        auto_route_requests(&anchors)
    } else {
        program.routes.clone()
    };
    let water_level_m = style
        .get("water")
        .and_then(|water| water.get("levelM"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let grid = Grid::new(scene, terrain, water, &program, margin);
    let mut routes = Vec::with_capacity(requests.len());
    let mut failures = Vec::new();
    for request in requests {
        let from = anchors
            .iter()
            .find(|anchor| anchor.id == request.from)
            .ok_or_else(|| TerrainError(format!("missing route anchor {}", request.from)))?;
        let to = anchors
            .iter()
            .find(|anchor| anchor.id == request.to)
            .ok_or_else(|| TerrainError(format!("missing route anchor {}", request.to)))?;
        let route = solve_route(&request, from, to, &grid, &program, water_level_m);
        if route.status != "pass" {
            failures.push(format!(
                "{}: {}",
                route.id,
                route.failure.as_deref().unwrap_or("route failed")
            ));
        }
        routes.push(route);
    }
    resolve_grade_crossings(&mut routes, program.max_bridge_span_m);
    Ok(Some(Plan {
        format: "glyphweave.route-evolution-plan".into(),
        version: 1,
        status: if failures.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        grid_step_m: program.grid_step_m,
        road_width_m: program.road_width_m,
        max_grade: program.max_grade,
        max_terrain_slope: program.max_terrain_slope,
        max_fill_m: program.max_fill_m,
        anchors,
        candidate_nodes,
        routes,
        failures,
    }))
}

const GRADE_SEPARATION_THRESHOLD_M: f64 = 1.2;

/// Resolve route intersections that cannot form a sane at-grade junction.
///
/// A hillside local street and a contour road may meet with more than ten metres
/// of vertical separation. Publishing both as ground-level ribbons made the
/// terrain composer take the higher surface and bury the lower street. The
/// higher route now publishes an explicit `overpass` span instead: it remains a
/// real elevated deck, while the lower route keeps its own cut, pavement and
/// collision chain.
fn resolve_grade_crossings(routes: &mut [Route], max_span_m: f64) {
    let mut additions = Vec::<(usize, BridgeSpan)>::new();
    for left in 0..routes.len() {
        for right in left + 1..routes.len() {
            if routes[left].status != "pass" || routes[right].status != "pass" {
                continue;
            }
            for left_index in 1..routes[left].polyline.len() {
                for right_index in 1..routes[right].polyline.len() {
                    let left_from = routes[left].polyline[left_index - 1];
                    let left_to = routes[left].polyline[left_index];
                    let right_from = routes[right].polyline[right_index - 1];
                    let right_to = routes[right].polyline[right_index];
                    let (left_t, right_t) = if let Some((_, _, t, u)) =
                        segment_intersection(left_from, left_to, right_from, right_to)
                    {
                        (t, u)
                    } else if let Some((overlap_lo, overlap_hi)) =
                        collinear_overlap(left_from, left_to, right_from, right_to)
                    {
                        let left_length = f64::from(left_to.x - left_from.x)
                            .hypot(f64::from(left_to.z - left_from.z));
                        let midpoint_t = (overlap_lo + overlap_hi) * 0.5 / left_length;
                        let point_x = f64::from(left_from.x)
                            + f64::from(left_to.x - left_from.x) * midpoint_t;
                        let point_z = f64::from(left_from.z)
                            + f64::from(left_to.z - left_from.z) * midpoint_t;
                        (
                            midpoint_t,
                            projection_fraction(right_from, right_to, point_x, point_z),
                        )
                    } else if let Some((t, u, distance)) =
                        segment_closest(left_from, left_to, right_from, right_to)
                    {
                        // `plan_terrain_carves` uses the wider published flat
                        // top, including the one-metre transverse margin and the
                        // seven-metre minimum band for narrow streets.
                        let half_band = |width: u32| (f64::from(width) * 0.5 + 1.0).max(7.0);
                        let footprint_overlap = half_band(routes[left].road_width_m)
                            + half_band(routes[right].road_width_m);
                        if distance >= footprint_overlap {
                            continue;
                        }
                        (t, u)
                    } else {
                        continue;
                    };
                    let left_y = interpolate_height(left_from, left_to, left_t);
                    let right_y = interpolate_height(right_from, right_to, right_t);
                    if (left_y - right_y).abs() <= GRADE_SEPARATION_THRESHOLD_M {
                        continue;
                    }
                    let (high, low, segment) = if left_y > right_y {
                        (left, right, left_index)
                    } else {
                        (right, left, right_index)
                    };
                    let half_run =
                        (f64::from(routes[left].road_width_m.max(routes[right].road_width_m))
                            * 1.75
                            + 12.0)
                            .min(max_span_m * 0.5);
                    let (start_index, end_index) =
                        span_around(&routes[high].polyline, segment, half_run);
                    let length_m = path_length(&routes[high].polyline[start_index..=end_index]);
                    if length_m > max_span_m {
                        continue;
                    }
                    additions.push((
                        high,
                        BridgeSpan {
                            start_index,
                            end_index,
                            length_m,
                            kind: format!("overpass-over-{}", routes[low].id),
                        },
                    ));
                }
            }
        }
    }
    for (route_index, span) in additions {
        routes[route_index].bridge_spans.push(span);
    }
    for route in routes {
        route.bridge_spans.sort_by_key(|span| span.start_index);
        let mut merged = Vec::<BridgeSpan>::new();
        for span in route.bridge_spans.drain(..) {
            if let Some(previous) = merged.last_mut() {
                if span.start_index <= previous.end_index.saturating_add(1) {
                    previous.end_index = previous.end_index.max(span.end_index);
                    previous.length_m =
                        path_length(&route.polyline[previous.start_index..=previous.end_index]);
                    if span.kind.starts_with("rejected-") {
                        previous.kind = span.kind;
                    } else if previous.kind != span.kind && span.kind.starts_with("overpass") {
                        previous.kind = "overpass".into();
                    }
                    continue;
                }
            }
            merged.push(span);
        }
        route.bridge_spans = merged;
    }
}

fn segment_intersection(
    a: RoutePoint,
    b: RoutePoint,
    c: RoutePoint,
    d: RoutePoint,
) -> Option<(f64, f64, f64, f64)> {
    let rx = f64::from(b.x - a.x);
    let rz = f64::from(b.z - a.z);
    let sx = f64::from(d.x - c.x);
    let sz = f64::from(d.z - c.z);
    let denominator = rx * sz - rz * sx;
    if denominator.abs() < 1e-9 {
        return None;
    }
    let qx = f64::from(c.x - a.x);
    let qz = f64::from(c.z - a.z);
    let t = (qx * sz - qz * sx) / denominator;
    let u = (qx * rz - qz * rx) / denominator;
    if !(-1e-6..=1.0 + 1e-6).contains(&t) || !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    Some((f64::from(a.x) + rx * t, f64::from(a.z) + rz * t, t, u))
}

/// Positive-length overlap of two collinear route segments, in metres along the
/// first segment. Near-collinear centerlines within half a lane are accepted so
/// independently smoothed route spellings of the same physical road converge.
fn collinear_overlap(
    a: RoutePoint,
    b: RoutePoint,
    c: RoutePoint,
    d: RoutePoint,
) -> Option<(f64, f64)> {
    let ax = f64::from(b.x - a.x);
    let az = f64::from(b.z - a.z);
    let length = ax.hypot(az);
    if length < 1e-6 {
        return None;
    }
    let ux = ax / length;
    let uz = az / length;
    let distance =
        |point: RoutePoint| (f64::from(point.x - a.x) * -uz + f64::from(point.z - a.z) * ux).abs();
    if distance(c).max(distance(d)) > 1.0 {
        return None;
    }
    let project = |point: RoutePoint| f64::from(point.x - a.x) * ux + f64::from(point.z - a.z) * uz;
    let c_project = project(c);
    let d_project = project(d);
    let lo = c_project.min(d_project).max(0.0);
    let hi = c_project.max(d_project).min(length);
    (hi - lo > 1.0).then_some((lo, hi))
}

fn projection_fraction(a: RoutePoint, b: RoutePoint, x: f64, z: f64) -> f64 {
    let dx = f64::from(b.x - a.x);
    let dz = f64::from(b.z - a.z);
    let length_sq = dx * dx + dz * dz;
    if length_sq <= f64::EPSILON {
        return 0.0;
    }
    (((x - f64::from(a.x)) * dx + (z - f64::from(a.z)) * dz) / length_sq).clamp(0.0, 1.0)
}

/// Fractions of the closest points on two segments and their centreline
/// distance. Used to detect wide carriageways whose footprints overlap even
/// when neither centreline actually crosses the other.
fn segment_closest(
    a: RoutePoint,
    b: RoutePoint,
    c: RoutePoint,
    d: RoutePoint,
) -> Option<(f64, f64, f64)> {
    let ax = f64::from(a.x);
    let az = f64::from(a.z);
    let dx = f64::from(b.x) - ax;
    let dz = f64::from(b.z) - az;
    let cx = f64::from(c.x);
    let cz = f64::from(c.z);
    let ex = f64::from(d.x) - cx;
    let ez = f64::from(d.z) - cz;
    let rx = ax - cx;
    let rz = az - cz;
    let aa = dx * dx + dz * dz;
    let ee = ex * ex + ez * ez;
    let b = dx * ex + dz * ez;
    let ef = ex * rx + ez * rz;
    let denominator = aa * ee - b * b;
    if aa <= f64::EPSILON || ee <= f64::EPSILON {
        return None;
    }
    let f = dx * rx + dz * rz;
    let (mut t, mut u) = if denominator.abs() < 1e-12 {
        let mut t = (f / aa).clamp(0.0, 1.0);
        let mut u = (((ax + dx * t) - cx) * ex + ((az + dz * t) - cz) * ez) / ee;
        u = u.clamp(0.0, 1.0);
        t = (((cx + ex * u) - ax) * dx + ((cz + ez * u) - az) * dz) / aa;
        (t.clamp(0.0, 1.0), u)
    } else {
        (
            ((b * ef - aa * f) / denominator).clamp(0.0, 1.0),
            ((ee * f - b * ef) / denominator).clamp(0.0, 1.0),
        )
    };
    if u < 0.0 {
        u = 0.0;
        t = (f / aa).clamp(0.0, 1.0);
    } else if u > 1.0 {
        u = 1.0;
        t = ((dx * (cx + ex - ax) + dz * (cz + ez - az)) / aa).clamp(0.0, 1.0);
    }
    let px = ax + dx * t;
    let pz = az + dz * t;
    let qx = cx + ex * u;
    let qz = cz + ez * u;
    Some((t, u, (px - qx).hypot(pz - qz)))
}

fn interpolate_height(a: RoutePoint, b: RoutePoint, t: f64) -> f64 {
    a.design_height_m + (b.design_height_m - a.design_height_m) * t
}

fn span_around(points: &[RoutePoint], center: usize, half_run_m: f64) -> (usize, usize) {
    let mut start = center;
    let mut travelled = 0.0;
    while start > 0 {
        let step = f64::from(points[start - 1].x - points[start].x)
            .hypot(f64::from(points[start - 1].z - points[start].z));
        if travelled + step > half_run_m {
            break;
        }
        travelled += step;
        start -= 1;
    }
    let mut end = center;
    travelled = 0.0;
    while end + 1 < points.len() {
        let step = f64::from(points[end + 1].x - points[end].x)
            .hypot(f64::from(points[end + 1].z - points[end].z));
        if travelled + step > half_run_m {
            break;
        }
        travelled += step;
        end += 1;
    }
    (start, end)
}

struct Grid<'a> {
    scene: &'a SceneSpec,
    terrain: &'a BakedTerrain,
    water: WaterGeometry,
    step: i32,
    margin: i32,
    cols: i32,
    rows: i32,
    max_terrain_slope: f64,
    turn_penalty: f64,
}

impl<'a> Grid<'a> {
    fn new(
        scene: &'a SceneSpec,
        terrain: &'a BakedTerrain,
        water: WaterGeometry,
        program: &Program,
        margin: i32,
    ) -> Self {
        let usable_width = scene.width_m as i32 - margin * 2;
        let usable_depth = scene.depth_m as i32 - margin * 2;
        Self {
            scene,
            terrain,
            water,
            step: program.grid_step_m as i32,
            margin,
            cols: usable_width / program.grid_step_m as i32 + 1,
            rows: usable_depth / program.grid_step_m as i32 + 1,
            max_terrain_slope: program.max_terrain_slope,
            turn_penalty: program.turn_penalty,
        }
    }

    fn point(&self, node: usize) -> (i32, i32) {
        let col = node as i32 % self.cols;
        let row = node as i32 / self.cols;
        (
            self.scene.origin_x + self.margin + col * self.step,
            self.scene.origin_z + self.margin + row * self.step,
        )
    }

    fn nearest_node(&self, x: i32, z: i32) -> usize {
        let col = ((x - self.scene.origin_x - self.margin) as f64 / f64::from(self.step))
            .round()
            .clamp(0.0, f64::from(self.cols - 1)) as i32;
        let row = ((z - self.scene.origin_z - self.margin) as f64 / f64::from(self.step))
            .round()
            .clamp(0.0, f64::from(self.rows - 1)) as i32;
        (row * self.cols + col) as usize
    }

    fn water_at(&self, x: i32, z: i32) -> bool {
        self.water.contains_point(f64::from(x), f64::from(z)) || self.water.shore_band_cell(x, z)
    }
}

#[derive(Clone, Copy)]
struct QueueEntry {
    cost: f64,
    state: usize,
}
impl Eq for QueueEntry {}
impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.state == other.state
    }
}
impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.state.cmp(&self.state))
    }
}
impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

fn direction_angle(direction: usize) -> f64 {
    let (dx, dz) = DIRECTIONS[direction];
    f64::from(dz).atan2(f64::from(dx))
}

fn turn_angle(a: usize, b: usize) -> f64 {
    let mut angle = (direction_angle(a) - direction_angle(b)).abs();
    if angle > std::f64::consts::PI {
        angle = std::f64::consts::TAU - angle;
    }
    angle
}

fn solve_route(
    request: &RouteRequest,
    from: &Anchor,
    to: &Anchor,
    grid: &Grid<'_>,
    program: &Program,
    water_level_m: f64,
) -> Route {
    let direct_length_m = f64::from(from.x - to.x).hypot(f64::from(from.z - to.z));
    let width = request.road_width_m.unwrap_or(program.road_width_m);
    let allow_bridge = request.allow_bridge.unwrap_or(program.allow_bridge);
    let alignment = request
        .alignment
        .clone()
        .unwrap_or_else(|| "terrain-following".into());
    let road_class = request.road_class.clone().unwrap_or_else(|| {
        super::RoadDesign::from_width(width as f32)
            .road_class
            .to_owned()
    });
    let failure = |message: String| Route {
        id: request.id.clone(),
        from: request.from.clone(),
        to: request.to.clone(),
        status: "fail".into(),
        road_width_m: width,
        road_class: road_class.clone(),
        alignment: alignment.clone(),
        polyline: Vec::new(),
        length_m: 0.0,
        direct_length_m,
        curve_deviation_m: 0.0,
        maximum_grade: 0.0,
        maximum_fill_m: 0.0,
        maximum_terrain_fill_m: 0.0,
        fill_volume_m3: 0.0,
        bridge_spans: Vec::new(),
        failure: Some(message),
    };
    let start = grid.nearest_node(from.x, from.z);
    let end = grid.nearest_node(to.x, to.z);
    if start == end {
        return failure("start and end resolve to the same grid node".into());
    }
    let nodes = if let Some([via_x, via_z]) = request.via {
        let via = grid.nearest_node(via_x, via_z);
        let first = match find_path_nodes(grid, start, via, allow_bridge) {
            Ok(nodes) => nodes,
            Err(error) => return failure(format!("cannot reach route waypoint: {error}")),
        };
        let second = match find_path_nodes(grid, via, end, allow_bridge) {
            Ok(nodes) => nodes,
            Err(error) => return failure(format!("cannot leave route waypoint: {error}")),
        };
        let mut nodes = first;
        nodes.extend(second.into_iter().skip(1));
        nodes
    } else {
        match find_path_nodes(grid, start, end, allow_bridge) {
            Ok(nodes) => nodes,
            Err(error) => return failure(error),
        }
    };
    let mut points: Vec<RoutePoint> = nodes
        .into_iter()
        .map(|node| {
            let (x, z) = grid.point(node);
            let natural_height_m = grid.terrain.height(x, z).unwrap_or(0.0) as f64;
            RoutePoint {
                x,
                z,
                natural_height_m,
                design_height_m: natural_height_m,
                water: grid.water_at(x, z),
            }
        })
        .collect();
    if let Some(first) = points.first_mut() {
        if (first.x, first.z) != (from.x, from.z) {
            let height = grid
                .terrain
                .height(from.x, from.z)
                .map(f64::from)
                .unwrap_or(first.natural_height_m);
            points.insert(
                0,
                RoutePoint {
                    x: from.x,
                    z: from.z,
                    natural_height_m: f64::from(height),
                    design_height_m: f64::from(height),
                    water: grid.water_at(from.x, from.z),
                },
            );
        }
    }
    if let Some(last) = points.last_mut() {
        if (last.x, last.z) != (to.x, to.z) {
            let height = grid
                .terrain
                .height(to.x, to.z)
                .map(f64::from)
                .unwrap_or(last.natural_height_m);
            points.push(RoutePoint {
                x: to.x,
                z: to.z,
                natural_height_m: f64::from(height),
                design_height_m: f64::from(height),
                water: grid.water_at(to.x, to.z),
            });
        }
    }
    if matches!(alignment.as_str(), "straight" | "straight-preferred") {
        let waypoints = request.via.map_or_else(
            || vec![[from.x, from.z], [to.x, to.z]],
            |via| vec![[from.x, from.z], via, [to.x, to.z]],
        );
        match straight_points(&waypoints, grid, program, allow_bridge) {
            Ok(straight) => points = straight,
            Err(error) if alignment == "straight" => {
                return failure(format!("straight alignment is infeasible: {error}"));
            }
            Err(_) => points = smooth_polyline(&points, grid, 2),
        }
    } else {
        points = smooth_polyline(&points, grid, 2);
    }
    grade_profile(&mut points, program, water_level_m);
    let fill_point = points
        .iter()
        .max_by(|a, b| {
            (a.design_height_m - a.natural_height_m)
                .partial_cmp(&(b.design_height_m - b.natural_height_m))
                .unwrap_or(Ordering::Equal)
        })
        .map(|point| {
            (
                point.x,
                point.z,
                point.design_height_m - point.natural_height_m,
            )
        })
        .unwrap_or((0, 0, 0.0));
    let maximum_fill_m = fill_point.2.max(0.0);
    let maximum_grade = points
        .windows(2)
        .map(|pair| {
            let distance = f64::from(pair[1].x - pair[0].x).hypot(f64::from(pair[1].z - pair[0].z));
            (pair[1].design_height_m - pair[0].design_height_m).abs() / distance.max(1e-9)
        })
        .fold(0.0, f64::max);
    if maximum_grade > program.max_grade + 1e-6 {
        return failure(format!(
            "route grade {:.4} exceeds {:.4}",
            maximum_grade, program.max_grade
        ));
    }
    let length_m = path_length(&points);
    let curve_deviation_m = maximum_deviation(&points, from, to);
    let mut bridge_spans = bridge_spans(&points, width, program.max_bridge_span_m);
    if points.iter().any(|point| point.water) && bridge_spans.is_empty() {
        return failure("route enters water without an emitted bridge span".into());
    }
    bridge_spans.extend(viaduct_spans(
        &points,
        width,
        program.max_fill_m,
        program.max_bridge_span_m,
    ));
    if bridge_spans
        .iter()
        .any(|span| span.kind.starts_with("rejected-"))
    {
        return failure(format!(
            "a bridge or viaduct exceeds the {}m maximum structure span",
            program.max_bridge_span_m
        ));
    }
    let structure = structure_mask(&points, &bridge_spans);
    let maximum_terrain_fill_m = points
        .iter()
        .enumerate()
        .filter(|(index, _)| !structure[*index])
        .map(|(_, point)| (point.design_height_m - point.natural_height_m).max(0.0))
        .fold(0.0, f64::max);
    if maximum_terrain_fill_m > program.max_fill_m + 1e-6 {
        return failure(format!(
            "terrain-supported profile needs {:.3}m fill at {},{} above the {}m contract",
            maximum_terrain_fill_m, fill_point.0, fill_point.1, program.max_fill_m
        ));
    }
    let fill_volume_m3 = points
        .windows(2)
        .enumerate()
        .map(|pair| {
            let (index, pair) = pair;
            if structure[index] || structure[index + 1] {
                return 0.0;
            }
            let distance = f64::from(pair[1].x - pair[0].x).hypot(f64::from(pair[1].z - pair[0].z));
            let fill = ((pair[0].design_height_m - pair[0].natural_height_m)
                + (pair[1].design_height_m - pair[1].natural_height_m))
                .max(0.0)
                * 0.5;
            fill * distance * f64::from(width)
        })
        .sum();
    Route {
        id: request.id.clone(),
        from: request.from.clone(),
        to: request.to.clone(),
        status: "pass".into(),
        road_width_m: width,
        road_class,
        alignment,
        polyline: points,
        length_m,
        direct_length_m,
        curve_deviation_m,
        maximum_grade,
        maximum_fill_m,
        maximum_terrain_fill_m,
        fill_volume_m3,
        bridge_spans,
        failure: None,
    }
}

fn straight_points(
    waypoints: &[[i32; 2]],
    grid: &Grid<'_>,
    program: &Program,
    allow_bridge: bool,
) -> Result<Vec<RoutePoint>, String> {
    if waypoints.len() < 2 {
        return Err("straight alignment needs at least two points".into());
    }
    let mut points = Vec::<RoutePoint>::new();
    for pair in waypoints.windows(2) {
        let [ax, az] = pair[0];
        let [bx, bz] = pair[1];
        let distance = f64::from(bx - ax).hypot(f64::from(bz - az));
        if distance < 1.0 {
            continue;
        }
        let sample_count = (distance / f64::from(program.grid_step_m)).ceil().max(1.0) as usize;
        for index in 0..=sample_count {
            let t = index as f64 / sample_count as f64;
            let x = (f64::from(ax) + f64::from(bx - ax) * t).round() as i32;
            let z = (f64::from(az) + f64::from(bz - az) * t).round() as i32;
            if points
                .last()
                .is_some_and(|point| point.x == x && point.z == z)
            {
                continue;
            }
            let natural_height_m = grid
                .terrain
                .height(x, z)
                .ok_or_else(|| format!("straight alignment has no terrain at {x},{z}"))?
                as f64;
            let water = grid.water_at(x, z);
            if water && !allow_bridge {
                return Err(format!("straight alignment enters water at {x},{z}"));
            }
            points.push(RoutePoint {
                x,
                z,
                natural_height_m,
                design_height_m: natural_height_m,
                water,
            });
        }
    }
    for pair in points.windows(2) {
        let distance = f64::from(pair[1].x - pair[0].x).hypot(f64::from(pair[1].z - pair[0].z));
        if distance < 1.0 {
            continue;
        }
        let slope = (pair[1].natural_height_m - pair[0].natural_height_m).abs() / distance;
        if slope > program.max_terrain_slope + 1e-6 {
            return Err(format!(
                "straight alignment terrain slope {slope:.3} exceeds {:.3}",
                program.max_terrain_slope
            ));
        }
    }
    Ok(points)
}

fn find_path_nodes(
    grid: &Grid<'_>,
    start: usize,
    end: usize,
    allow_bridge: bool,
) -> Result<Vec<usize>, String> {
    if start == end {
        return Ok(vec![start]);
    }
    let states = grid.cols as usize * grid.rows as usize * DIRECTIONS.len();
    let mut cost = vec![f64::INFINITY; states];
    let mut previous = vec![usize::MAX; states];
    let mut heap = BinaryHeap::new();
    for direction in 0..DIRECTIONS.len() {
        let state = start * DIRECTIONS.len() + direction;
        cost[state] = 0.0;
        heap.push(QueueEntry { cost: 0.0, state });
    }
    let mut best_end = None;
    while let Some(entry) = heap.pop() {
        if entry.cost > cost[entry.state] {
            continue;
        }
        let node = entry.state / DIRECTIONS.len();
        let direction = entry.state % DIRECTIONS.len();
        if node == end {
            best_end = Some(entry.state);
            break;
        }
        let (x0, z0) = grid.point(node);
        let h0 = grid
            .terrain
            .height(x0, z0)
            .ok_or_else(|| format!("missing terrain at route grid point {x0},{z0}"))?;
        let col = node as i32 % grid.cols;
        let row = node as i32 / grid.cols;
        for next_direction in 0..DIRECTIONS.len() {
            let (dx, dz) = DIRECTIONS[next_direction];
            let next_col = col + dx;
            let next_row = row + dz;
            if next_col < 0 || next_row < 0 || next_col >= grid.cols || next_row >= grid.rows {
                continue;
            }
            let next = (next_row * grid.cols + next_col) as usize;
            let (x1, z1) = grid.point(next);
            let water = grid.water_at(x1, z1);
            if water && !allow_bridge {
                continue;
            }
            let Some(h1) = grid.terrain.height(x1, z1) else {
                continue;
            };
            let distance = f64::from(grid.step).hypot(if dx != 0 && dz != 0 {
                f64::from(grid.step)
            } else {
                0.0
            });
            let terrain_slope = f64::from(h1 - h0).abs() / distance;
            if terrain_slope > grid.max_terrain_slope {
                continue;
            }
            let turn = turn_angle(direction, next_direction);
            let step_cost = distance
                * if water { 1.5 } else { 1.0 }
                * (1.0 + terrain_slope * 18.0 + grid.turn_penalty * turn);
            let next_state = next * DIRECTIONS.len() + next_direction;
            let candidate = entry.cost + step_cost;
            if candidate + 1e-9 < cost[next_state] {
                cost[next_state] = candidate;
                previous[next_state] = entry.state;
                heap.push(QueueEntry {
                    cost: candidate,
                    state: next_state,
                });
            }
        }
    }
    let Some(mut state) = best_end else {
        return Err("no terrain route satisfies the slope and crossing limits".into());
    };
    let mut nodes = Vec::new();
    loop {
        nodes.push(state / DIRECTIONS.len());
        let prior = previous[state];
        if prior == usize::MAX {
            break;
        }
        state = prior;
    }
    nodes.reverse();
    Ok(nodes)
}

fn sample_route_point(grid: &Grid<'_>, x: i32, z: i32) -> RoutePoint {
    let natural_height_m = grid.terrain.height(x, z).map(f64::from).unwrap_or(0.0);
    RoutePoint {
        x,
        z,
        natural_height_m,
        design_height_m: natural_height_m,
        water: grid.water_at(x, z),
    }
}

fn smooth_polyline(points: &[RoutePoint], grid: &Grid<'_>, iterations: usize) -> Vec<RoutePoint> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut current = points.to_vec();
    for _ in 0..iterations {
        let mut next = Vec::with_capacity(current.len() * 2);
        next.push(current[0].clone());
        for pair in current.windows(2) {
            let qx = (pair[0].x as f64 * 0.75 + pair[1].x as f64 * 0.25).round() as i32;
            let qz = (pair[0].z as f64 * 0.75 + pair[1].z as f64 * 0.25).round() as i32;
            let rx = (pair[0].x as f64 * 0.25 + pair[1].x as f64 * 0.75).round() as i32;
            let rz = (pair[0].z as f64 * 0.25 + pair[1].z as f64 * 0.75).round() as i32;
            let q = sample_route_point(grid, qx, qz);
            let r = sample_route_point(grid, rx, rz);
            if next
                .last()
                .is_none_or(|point: &RoutePoint| (point.x, point.z) != (q.x, q.z))
            {
                next.push(q);
            }
            if next
                .last()
                .is_none_or(|point: &RoutePoint| (point.x, point.z) != (r.x, r.z))
            {
                next.push(r);
            }
        }
        next.push(current.last().unwrap().clone());
        current = next;
    }
    current
}

fn grade_profile(points: &mut [RoutePoint], program: &Program, water_level_m: f64) {
    for point in points.iter_mut() {
        if point.water {
            point.design_height_m = point
                .design_height_m
                .max(point.natural_height_m)
                .max(water_level_m + 4.0);
        }
    }
    for index in 1..points.len() {
        let distance = f64::from(points[index].x - points[index - 1].x)
            .hypot(f64::from(points[index].z - points[index - 1].z));
        let minimum = points[index - 1].design_height_m - program.max_grade * distance;
        points[index].design_height_m = points[index].design_height_m.max(minimum);
    }
    for index in (0..points.len().saturating_sub(1)).rev() {
        let distance = f64::from(points[index + 1].x - points[index].x)
            .hypot(f64::from(points[index + 1].z - points[index].z));
        let minimum = points[index + 1].design_height_m - program.max_grade * distance;
        points[index].design_height_m = points[index].design_height_m.max(minimum);
    }
}

fn path_length(points: &[RoutePoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| f64::from(pair[1].x - pair[0].x).hypot(f64::from(pair[1].z - pair[0].z)))
        .sum()
}

fn maximum_deviation(points: &[RoutePoint], from: &Anchor, to: &Anchor) -> f64 {
    let ax = f64::from(from.x);
    let az = f64::from(from.z);
    let dx = f64::from(to.x - from.x);
    let dz = f64::from(to.z - from.z);
    let length = dx.hypot(dz).max(1e-9);
    points
        .iter()
        .map(|point| {
            ((f64::from(point.x) - ax) * dz - (f64::from(point.z) - az) * dx).abs() / length
        })
        .fold(0.0, f64::max)
}

fn bridge_spans(points: &[RoutePoint], width: u32, max_span_m: f64) -> Vec<BridgeSpan> {
    let mut spans = Vec::new();
    let mut start = None;
    for index in 0..=points.len() {
        let water = index < points.len() && points[index].water;
        match (start, water) {
            (None, true) => start = Some(index),
            (Some(first), false) => {
                let last = index.saturating_sub(1);
                let length = if first < last {
                    path_length(&points[first..=last])
                } else {
                    f64::from(width)
                };
                spans.push(BridgeSpan {
                    start_index: first.saturating_sub(1),
                    end_index: index.min(points.len() - 1),
                    length_m: length,
                    kind: if length > max_span_m {
                        "rejected-bridge-span-too-long".into()
                    } else {
                        "bridge".into()
                    },
                });
                start = None;
            }
            _ => {}
        }
    }
    spans
}

fn viaduct_spans(
    points: &[RoutePoint],
    width: u32,
    max_fill_m: f64,
    max_span_m: f64,
) -> Vec<BridgeSpan> {
    let mut spans = Vec::new();
    let mut start = None;
    for index in 0..=points.len() {
        let needs_structure = index < points.len()
            && !points[index].water
            && points[index].design_height_m - points[index].natural_height_m > max_fill_m + 1e-6;
        match (start, needs_structure) {
            (None, true) => start = Some(index),
            (Some(first), false) => {
                let last = index.saturating_sub(1);
                let length = if first < last {
                    path_length(&points[first..=last]).max(f64::from(width))
                } else {
                    f64::from(width)
                };
                spans.push(BridgeSpan {
                    start_index: first.saturating_sub(1),
                    end_index: index.min(points.len() - 1),
                    length_m: length,
                    kind: if length > max_span_m {
                        "rejected-viaduct-span-too-long".into()
                    } else {
                        "viaduct".into()
                    },
                });
                start = None;
            }
            _ => {}
        }
    }
    spans
}

fn structure_mask(points: &[RoutePoint], spans: &[BridgeSpan]) -> Vec<bool> {
    let mut mask = vec![false; points.len()];
    for span in spans {
        for value in &mut mask[span.start_index..=span.end_index] {
            *value = true;
        }
    }
    mask
}

fn auto_anchors(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    margin: i32,
    candidates: &[NaturalNode],
) -> Result<Vec<Anchor>, TerrainError> {
    let quadrants = ["northwest", "northeast", "southwest", "southeast"];
    let mut anchors = Vec::new();
    for quadrant in quadrants {
        let selected = candidates
            .iter()
            .filter(|node| node.quadrant == quadrant)
            .max_by(|left, right| {
                left.score
                    .partial_cmp(&right.score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| right.z.cmp(&left.z))
                    .then_with(|| right.x.cmp(&left.x))
            });
        if let Some(node) = selected {
            anchors.push(Anchor {
                id: format!("natural-{quadrant}"),
                x: node.x,
                z: node.z,
                role: "natural-valley-or-water-node".into(),
            });
        }
    }
    if anchors.len() >= 2 {
        return Ok(anchors);
    }
    fallback_anchors(scene, terrain, water, margin)
}

fn auto_route_requests(anchors: &[Anchor]) -> Vec<RouteRequest> {
    let mut connected = vec![false; anchors.len()];
    connected[0] = true;
    let mut requests = Vec::with_capacity(anchors.len().saturating_sub(1));
    while connected.iter().filter(|value| **value).count() < anchors.len() {
        let mut best: Option<(i64, usize, usize)> = None;
        for left in 0..anchors.len() {
            if !connected[left] {
                continue;
            }
            for (right, anchor) in anchors.iter().enumerate() {
                if connected[right] {
                    continue;
                }
                let dx = i64::from(anchors[left].x) - i64::from(anchor.x);
                let dz = i64::from(anchors[left].z) - i64::from(anchor.z);
                let distance_squared = dx * dx + dz * dz;
                let candidate = (distance_squared, left, right);
                if best.is_none_or(|current| {
                    (candidate.0, candidate.1, candidate.2) < (current.0, current.1, current.2)
                }) {
                    best = Some(candidate);
                }
            }
        }
        let Some((_, left, right)) = best else {
            break;
        };
        connected[right] = true;
        requests.push(RouteRequest {
            id: format!("route-auto-{}", requests.len()),
            from: anchors[left].id.clone(),
            to: anchors[right].id.clone(),
            via: None,
            road_width_m: None,
            allow_bridge: None,
            road_class: None,
            alignment: None,
        });
    }
    requests
}

fn fallback_anchors(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    margin: i32,
) -> Result<Vec<Anchor>, TerrainError> {
    let positions = [
        (
            "auto-west",
            scene.origin_x + scene.width_m as i32 / 5,
            scene.origin_z + scene.depth_m as i32 / 2,
        ),
        (
            "auto-east",
            scene.origin_x + scene.width_m as i32 * 4 / 5,
            scene.origin_z + scene.depth_m as i32 / 2,
        ),
        (
            "auto-north",
            scene.origin_x + scene.width_m as i32 / 2,
            scene.origin_z + scene.depth_m as i32 / 5,
        ),
        (
            "auto-south",
            scene.origin_x + scene.width_m as i32 / 2,
            scene.origin_z + scene.depth_m as i32 * 4 / 5,
        ),
    ];
    let mut anchors = Vec::new();
    for (id, x, z) in positions {
        if let Some((x, z)) = find_land_near(terrain, water, x, z, margin) {
            anchors.push(Anchor {
                id: id.into(),
                x,
                z,
                role: "auto-terrain-seed".into(),
            });
        }
    }
    Ok(anchors)
}

fn natural_nodes(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    margin: i32,
) -> Vec<NaturalNode> {
    let (origin_x, origin_z) = terrain.scene_origin();
    let (width, depth) = terrain.scene_dimensions();
    let min_x = origin_x + margin;
    let min_z = origin_z + margin;
    let max_x = origin_x + width as i32 - margin;
    let max_z = origin_z + depth as i32 - margin;
    let mid_x = origin_x + width as i32 / 2;
    let mid_z = origin_z + depth as i32 / 2;
    let mut nodes = Vec::new();
    for z in (min_z..max_z).step_by(64) {
        for x in (min_x..max_x).step_by(64) {
            if water.contains_point(f64::from(x), f64::from(z)) || water.shore_band_cell(x, z) {
                continue;
            }
            let Some(elevation) = terrain.height(x, z) else {
                continue;
            };
            let offsets = [(16, 0), (-16, 0), (0, 16), (0, -16)];
            let mut neighbour_sum = 0.0;
            let mut max_slope: f64 = 0.0;
            let mut valid = true;
            for (dx, dz) in offsets {
                let Some(neighbour) = terrain.height(x + dx, z + dz) else {
                    valid = false;
                    break;
                };
                neighbour_sum += f64::from(neighbour);
                max_slope =
                    max_slope.max((f64::from(neighbour) - f64::from(elevation)).abs() / 16.0);
            }
            if !valid {
                continue;
            }
            let neighbour_mean = neighbour_sum / 4.0;
            let valley_score = ((neighbour_mean - f64::from(elevation)) / 6.0).clamp(-1.0, 1.0);
            let flatness = (1.0 - max_slope / 0.35).clamp(0.0, 1.0);
            let water_access = water_access_score(water, x, z);
            let score = flatness * 0.52 + water_access * 0.28 + (valley_score + 1.0) * 0.10;
            let quadrant = match (x < mid_x, z < mid_z) {
                (true, true) => "northwest",
                (false, true) => "northeast",
                (true, false) => "southwest",
                (false, false) => "southeast",
            };
            nodes.push(NaturalNode {
                id: format!("natural.{x}.{z}"),
                x,
                z,
                quadrant: quadrant.into(),
                elevation_m: f64::from(elevation),
                slope: max_slope,
                flatness,
                valley_score,
                water_access,
                score,
            });
        }
    }
    nodes
}

fn water_access_score(water: WaterGeometry, x: i32, z: i32) -> f64 {
    for radius in [8_i32, 16, 32, 64, 96, 128] {
        for dz in (-radius..=radius).step_by(8) {
            for dx in (-radius..=radius).step_by(8) {
                if dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                if water.contains_point(f64::from(x + dx), f64::from(z + dz))
                    || water.shore_band_cell(x + dx, z + dz)
                {
                    return 1.0 - f64::from(radius) / 160.0;
                }
            }
        }
    }
    0.0
}

fn find_land_near(
    terrain: &BakedTerrain,
    water: WaterGeometry,
    x: i32,
    z: i32,
    margin: i32,
) -> Option<(i32, i32)> {
    let (origin_x, origin_z) = terrain.scene_origin();
    let (width, depth) = terrain.scene_dimensions();
    for radius in (0_i32..=256).step_by(16) {
        for dz in (-radius..=radius).step_by(16) {
            for dx in (-radius..=radius).step_by(16) {
                if radius > 0 && dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                let px = x + dx;
                let pz = z + dz;
                if px < origin_x + margin
                    || pz < origin_z + margin
                    || px >= origin_x + width as i32 - margin
                    || pz >= origin_z + depth as i32 - margin
                    || water.contains_point(f64::from(px), f64::from(pz))
                    || water.shore_band_cell(px, pz)
                {
                    continue;
                }
                if terrain.height(px, pz).is_some() {
                    return Some((px, pz));
                }
            }
        }
    }
    None
}

pub fn require_pass(plan: &Plan) -> Result<(), TerrainError> {
    if plan.status == "pass" {
        Ok(())
    } else {
        Err(TerrainError(format!(
            "route evolution failed: {}",
            plan.failures.join("; ")
        )))
    }
}

/// Remove surface plants and rocks that would intersect a route footprint.
/// Authored or generated structures are not silently moved; only natural
/// decor is cleared, and the route plan remains the source of the corridor.
pub fn retain_clearance(entities: &mut Vec<EntityInstance>, plan: &Plan, extra_m: f64) -> usize {
    let before = entities.len();
    entities.retain(|entity| {
        if !matches!(
            entity.kind.as_str(),
            "tree" | "bush" | "grass_clump" | "rock" | "fallen_log"
        ) {
            return true;
        }
        let entity_radius_m = entity.width_m.max(entity.depth_m) as f64 * 0.5;
        !plan.routes.iter().any(|route| {
            if route.status != "pass" {
                return false;
            }
            let clearance = route.road_width_m as f64 * 0.5 + extra_m + entity_radius_m;
            route.polyline.windows(2).any(|segment| {
                point_segment_distance(
                    f64::from(entity.world_x),
                    f64::from(entity.world_z),
                    f64::from(segment[0].x),
                    f64::from(segment[0].z),
                    f64::from(segment[1].x),
                    f64::from(segment[1].z),
                ) <= clearance
            })
        })
    });
    before - entities.len()
}

/// Materialize a passing route plan as static road segments.
///
/// The planner already produced a terrain-following, grade-limited polyline.
/// Each consecutive pair becomes an oriented road segment using the same width
/// and design height, so HTML and downstream audits can consume the result
/// without inventing a second road geometry.
pub fn materialize(plan: &Plan) -> Vec<EntityInstance> {
    let mut entities = Vec::new();
    for route in plan.routes.iter().filter(|route| route.status == "pass") {
        // The storage contract treats a road's widthM as its long axis and
        // depthM as pavement width. Keep enough subdivisions to follow the
        // terrain curve without creating sub-contract slivers.
        let minimum_segment = f64::from(route.road_width_m).mul_add(0.5, 0.25).max(4.25);
        let simplified = simplify_for_materialization(&route.polyline, minimum_segment);
        for (index, pair) in simplified.windows(2).enumerate() {
            let dx = pair[1].x - pair[0].x;
            let dz = pair[1].z - pair[0].z;
            let length = f64::from(dx).hypot(f64::from(dz));
            if length < 0.5 {
                continue;
            }
            // Entity Y rotation follows the same Three-style frame consumed by
            // Rust OBBs: local +X is (cos(yaw), -sin(yaw)) in world X/Z.
            // Using atan2(dz, dx) mirrored every diagonal segment across X and
            // put its collision box on the opposite diagonal of the pavement.
            let rotation_y_deg = f64::from(-dz).atan2(f64::from(dx)).to_degrees() as f32;
            let start_height_mm = (pair[0].design_height_m * 1000.).round() as i32;
            let end_height_mm = (pair[1].design_height_m * 1000.).round() as i32;
            let design_height_m = (pair[0].design_height_m + pair[1].design_height_m) * 0.5;
            let structure_deck = route_segment_is_structure(route, &pair[0], &pair[1]);
            let route_grade = super::RouteGrade {
                version: 1,
                start: [pair[0].x, pair[0].z],
                end: [pair[1].x, pair[1].z],
                start_height_mm,
                end_height_mm,
            };
            entities.push(EntityInstance {
                asset_id: "road.generated".into(),
                entity_id: format!("generated.route-evolution.{}.{}", route.id, index),
                kind: "road".into(),
                world_x: ((pair[0].x + pair[1].x) as f64 * 0.5).round() as i32,
                world_z: ((pair[0].z + pair[1].z) as f64 * 0.5).round() as i32,
                world_y: design_height_m.round() as i32,
                scale: 1.0,
                width_m: length as f32,
                depth_m: route.road_width_m as f32,
                height_m: 0.45,
                rotation_y_deg,
                grounding: super::GroundingSpec {
                    structure_deck,
                    roadbed_offset_m: if structure_deck { 0.0 } else { 0.5 },
                    route_grade: Some(route_grade),
                    road_design: Some(
                        super::RoadDesign::for_class(&route.road_class, route.road_width_m as f32)
                            .unwrap_or_else(|| {
                                super::RoadDesign::from_width(route.road_width_m as f32)
                            }),
                    ),
                    ..super::GroundingSpec::default()
                },
                anchors: Vec::new(),
                bounds: None,
            });
        }
    }
    entities
}

fn route_segment_is_structure(route: &Route, from: &RoutePoint, to: &RoutePoint) -> bool {
    let first = route
        .polyline
        .iter()
        .position(|point| (point.x, point.z) == (from.x, from.z));
    let last = route
        .polyline
        .iter()
        .rposition(|point| (point.x, point.z) == (to.x, to.z));
    let (Some(first), Some(last)) = (first, last) else {
        return false;
    };
    let (first, last) = if first <= last {
        (first, last)
    } else {
        (last, first)
    };
    route.bridge_spans.iter().any(|span| {
        !span.kind.starts_with("rejected-") && span.start_index <= last && span.end_index >= first
    })
}

fn simplify_for_materialization(
    polyline: &[RoutePoint],
    minimum_segment_m: f64,
) -> Vec<RoutePoint> {
    let Some(first) = polyline.first() else {
        return Vec::new();
    };
    let mut kept = vec![first.clone()];
    for point in polyline
        .iter()
        .skip(1)
        .take(polyline.len().saturating_sub(2))
    {
        let last = kept.last().expect("first point is always present");
        if f64::from(point.x - last.x).hypot(f64::from(point.z - last.z)) >= minimum_segment_m {
            kept.push(point.clone());
        }
    }
    if let Some(last) = polyline.last() {
        let current = kept.last().expect("first point is always present");
        let remaining = f64::from(last.x - current.x).hypot(f64::from(last.z - current.z));
        if remaining > 0.5 {
            if remaining < minimum_segment_m && kept.len() >= 2 {
                *kept.last_mut().expect("kept has at least two points") = last.clone();
            } else {
                kept.push(last.clone());
            }
        }
    }
    kept
}

fn point_segment_distance(px: f64, pz: f64, ax: f64, az: f64, bx: f64, bz: f64) -> f64 {
    let dx = bx - ax;
    let dz = bz - az;
    let length_sq = dx * dx + dz * dz;
    if length_sq <= f64::EPSILON {
        return (px - ax).hypot(pz - az);
    }
    let t = (((px - ax) * dx + (pz - az) * dz) / length_sq).clamp(0.0, 1.0);
    (px - (ax + dx * t)).hypot(pz - (az + dz * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;
    use crate::worldgen::{WaterKind, water_geometry};

    fn terrain(scene: &SceneSpec) -> BakedTerrain {
        let width = scene.width_m as usize;
        let depth = scene.depth_m as usize;
        let values = (0..depth).flat_map(|z| {
            (0..width).map(move |x| {
                let ridge = ((f64::from(x as i32 - 256) / 70.0).abs()).min(1.0);
                let valley = if (170..=340).contains(&z) { 2.0 } else { 8.0 };
                valley + ridge * 4.0
            })
        });
        let payload = encode_millimetres(scene.width_m, scene.depth_m, values).unwrap();
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &payload,
            )
            .unwrap();
        terrain
    }

    fn style(allow_bridge: bool) -> Value {
        serde_json::json!({
            "naturalGeography": {
                "version": 1,
                "setting": "coastal-mountain",
                "regionalReliefM": 900,
                "orogenyAxisDeg": 28,
                "upliftBias": 0.72,
                "coast": {"type": "ria-bay", "waveExposure": "moderate", "trench": false},
                "climate": {
                    "band": "humid-subtropical",
                    "prevailingWindDeg": 125,
                    "monsoon": true,
                    "rainfallBias": 0.65,
                    "seasonality": 0.48
                },
                "hydrology": {
                    "regime": "perennial",
                    "historicChannelShiftM": 260,
                    "wetlandBias": 0.3
                },
                "landform": {
                    "karst": false,
                    "aeolian": false,
                    "glacial": false,
                    "incisedValleys": 0.68
                },
                "hazards": ["flood"]
            },
            "water": {"waterType": "lake", "levelPolicy": "horizontal-datum", "levelM": 0},
            "routeEvolution": {
                "version": 1,
                "enabled": true,
                "gridStepM": 16,
                "roadWidthM": 8,
                "maxGrade": 0.08,
                "maxTerrainSlope": 0.45,
                "maxFillM": 6,
                "allowBridge": allow_bridge,
                "maxBridgeSpanM": 320,
                "turnPenalty": 1.4,
                "anchors": [
                    {"id": "west", "x": 96, "z": 256},
                    {"id": "east", "x": 416, "z": 256}
                ],
                "routes": [{"id": "crossing", "from": "west", "to": "east", "via": [256, 256]}]
            }
        })
    }

    fn lake(scene: &SceneSpec, style: &Value) -> WaterGeometry {
        let lake: crate::worldgen::LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId": "lake",
            "name": "lake",
            "type": "lake",
            "purpose": "test",
            "description": "test lake",
            "sceneId": scene.scene_id,
            "worldX": 256,
            "worldZ": 256,
            "worldY": 0,
            "widthM": 120,
            "depthM": 90,
            "heightM": 1,
            "assetId": "water.lake"
        }))
        .unwrap();
        water_geometry(WaterKind::Lake, &[lake], scene, style)
    }

    #[test]
    fn route_follows_terrain_with_a_bridge_and_grade_limited_profile() {
        let scene = SceneSpec {
            scene_id: "route".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let terrain = terrain(&scene);
        let style = style(true);
        let water = lake(&scene, &style);
        let plan = plan(&scene, &terrain, water, &style).unwrap().unwrap();
        require_pass(&plan).unwrap();
        let route = &plan.routes[0];
        assert!(route.polyline.len() > 3);
        assert!(!route.bridge_spans.is_empty());
        assert!(route.maximum_grade <= 0.08 + 1e-6);
        assert!(route.maximum_terrain_fill_m <= 6.0 + 1e-6);
        assert!(
            route
                .bridge_spans
                .iter()
                .all(|span| !span.kind.starts_with("rejected-"))
        );
    }

    #[test]
    fn auto_anchors_come_from_dry_natural_nodes_in_multiple_quadrants() {
        let scene = SceneSpec {
            scene_id: "natural-nodes".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let terrain = terrain(&scene);
        let style = style(true);
        let water = lake(&scene, &style);
        let candidates = natural_nodes(&scene, &terrain, water, 40);
        assert!(candidates.len() >= 16);
        assert!(candidates.iter().all(|node| {
            node.score.is_finite()
                && node.slope >= 0.0
                && (0.0..=1.0).contains(&node.flatness)
                && (0.0..=1.0).contains(&node.water_access)
                && !water.contains_point(f64::from(node.x), f64::from(node.z))
        }));
        let anchors = auto_anchors(&scene, &terrain, water, 40, &candidates).unwrap();
        assert!(anchors.len() >= 3);
        assert!(anchors.iter().all(|anchor| {
            anchor.role == "natural-valley-or-water-node"
                && anchor.id.starts_with("natural-")
                && !water.contains_point(f64::from(anchor.x), f64::from(anchor.z))
        }));
        assert!(
            anchors
                .iter()
                .map(|anchor| anchor.id.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 3
        );

        let requests = auto_route_requests(&anchors);
        assert_eq!(requests.len(), anchors.len() - 1);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.id.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            requests.len()
        );
        let names = anchors
            .iter()
            .map(|anchor| anchor.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for request in &requests {
            assert!(names.contains(request.from.as_str()));
            assert!(names.contains(request.to.as_str()));
            assert_ne!(request.from, request.to);
        }
    }

    #[test]
    fn materialized_roads_keep_curve_orientation_and_auditable_segments() {
        let scene = SceneSpec {
            scene_id: "route-materialize".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let terrain = terrain(&scene);
        let style = style(true);
        let water = lake(&scene, &style);
        let plan = plan(&scene, &terrain, water, &style).unwrap().unwrap();
        let entities = materialize(&plan);
        assert!(!entities.is_empty());
        assert!(entities.iter().all(|entity| entity.kind == "road"));
        assert!(entities.iter().all(|entity| entity.width_m >= 4.25));
        assert!(
            entities
                .iter()
                .any(|entity| entity.rotation_y_deg.abs() > 1.0)
        );
        let ids = entities
            .iter()
            .map(|entity| entity.entity_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), entities.len());
        for entity in &entities {
            let grade = entity
                .grounding
                .route_grade
                .as_ref()
                .expect("every route segment must publish its design line");
            grade.validate_shape().unwrap();
            let midpoint = [
                (grade.start[0] + grade.end[0]) as f64 * 0.5,
                (grade.start[1] + grade.end[1]) as f64 * 0.5,
            ];
            let expected =
                (f64::from(grade.start_height_mm) + f64::from(grade.end_height_mm)) * 0.0005;
            assert!(
                (grade
                    .height_at(midpoint[0], midpoint[1])
                    .expect("valid segment grade")
                    - expected)
                    .abs()
                    < 1e-9
            );
        }
    }

    #[test]
    fn conflicting_route_crossings_become_explicit_overpasses() {
        let route = |id: &str, points: &[(i32, i32, f64)]| Route {
            id: id.into(),
            from: "a".into(),
            to: "b".into(),
            status: "pass".into(),
            road_width_m: 8,
            road_class: "township".into(),
            alignment: "terrain-following".into(),
            polyline: points
                .iter()
                .map(|(x, z, y)| RoutePoint {
                    x: *x,
                    z: *z,
                    natural_height_m: *y,
                    design_height_m: *y,
                    water: false,
                })
                .collect(),
            length_m: 20.0,
            direct_length_m: 20.0,
            curve_deviation_m: 0.0,
            maximum_grade: 0.0,
            maximum_fill_m: 0.0,
            maximum_terrain_fill_m: 0.0,
            fill_volume_m3: 0.0,
            bridge_spans: Vec::new(),
            failure: None,
        };
        let mut routes = vec![
            route("low", &[(0, 10, 2.0), (10, 10, 2.0), (20, 10, 2.0)]),
            route("high", &[(10, 0, 12.0), (10, 10, 12.0), (10, 20, 12.0)]),
            route(
                "parallel-high",
                &[(0, 10, 5.0), (10, 10, 5.0), (20, 10, 5.0)],
            ),
            route("offset-high", &[(0, 15, 5.0), (10, 15, 5.0), (20, 15, 5.0)]),
        ];
        resolve_grade_crossings(&mut routes, 420.0);
        assert!(routes[0].bridge_spans.is_empty());
        assert!(
            routes[1]
                .bridge_spans
                .iter()
                .any(|span| span.kind.starts_with("overpass"))
        );
        assert!(
            routes[2]
                .bridge_spans
                .iter()
                .any(|span| span.kind.starts_with("overpass"))
        );
        assert!(
            routes[3]
                .bridge_spans
                .iter()
                .any(|span| span.kind.starts_with("overpass"))
        );
        let high = materialize(&Plan {
            format: "glyphweave.route-evolution-plan".into(),
            version: 1,
            status: "pass".into(),
            grid_step_m: 16,
            road_width_m: 8,
            max_grade: 0.08,
            max_terrain_slope: 0.6,
            max_fill_m: 6.0,
            anchors: Vec::new(),
            candidate_nodes: Vec::new(),
            routes,
            failures: Vec::new(),
        });
        assert!(
            high.iter()
                .filter(|entity| entity.entity_id.contains(".high."))
                .any(|entity| entity.grounding.structure_deck)
        );
    }

    #[test]
    fn wide_sloped_road_footprints_without_centreline_crossing_grade_separate() {
        let route = |id: &str, points: &[(i32, i32, f64)]| Route {
            id: id.into(),
            from: "a".into(),
            to: "b".into(),
            status: "pass".into(),
            road_width_m: 8,
            road_class: "township".into(),
            alignment: "terrain-following".into(),
            polyline: points
                .iter()
                .map(|(x, z, y)| RoutePoint {
                    x: *x,
                    z: *z,
                    natural_height_m: *y,
                    design_height_m: *y,
                    water: false,
                })
                .collect(),
            length_m: 20.0,
            direct_length_m: 20.0,
            curve_deviation_m: 0.0,
            maximum_grade: 0.25,
            maximum_fill_m: 0.0,
            maximum_terrain_fill_m: 0.0,
            fill_volume_m3: 0.0,
            bridge_spans: Vec::new(),
            failure: None,
        };
        let mut routes = vec![
            route(
                "local",
                &[(744, 566, 2.0), (744, 570, 1.9), (744, 574, 1.8)],
            ),
            route(
                "hillside",
                &[
                    (722, 568, 17.3),
                    (726, 567, 17.0),
                    (730, 565, 16.6),
                    (734, 562, 16.2),
                    (738, 558, 15.8),
                    (741, 554, 15.4),
                    (743, 550, 15.0),
                ],
            ),
        ];
        routes[1].road_width_m = 32;
        let distance = segment_closest(
            routes[0].polyline[0],
            routes[0].polyline[1],
            routes[1].polyline[0],
            routes[1].polyline[1],
        )
        .unwrap()
        .2;
        assert!(distance < 20.0, "test geometry must overlap: {distance}");
        resolve_grade_crossings(&mut routes, 420.0);
        assert!(
            routes[1]
                .bridge_spans
                .iter()
                .any(|span| span.kind == "overpass-over-local")
        );
    }

    #[test]
    fn crossing_at_materialized_segment_endpoints_is_grade_separated() {
        let route = |id: &str, width: u32, points: &[(i32, i32, f64)]| Route {
            id: id.into(),
            from: "a".into(),
            to: "b".into(),
            status: "pass".into(),
            road_width_m: width,
            road_class: "township".into(),
            alignment: "terrain-following".into(),
            polyline: points
                .iter()
                .map(|(x, z, y)| RoutePoint {
                    x: *x,
                    z: *z,
                    natural_height_m: *y,
                    design_height_m: *y,
                    water: false,
                })
                .collect(),
            length_m: 24.0,
            direct_length_m: 24.0,
            curve_deviation_m: 0.0,
            maximum_grade: 0.08,
            maximum_fill_m: 0.0,
            maximum_terrain_fill_m: 0.0,
            fill_volume_m3: 0.0,
            bridge_spans: Vec::new(),
            failure: None,
        };
        let mut routes = vec![
            route(
                "low",
                16,
                &[(550, 488, 33.75), (562, 488, 34.38), (574, 488, 35.34)],
            ),
            route(
                "high",
                10,
                &[(568, 486, 41.96), (568, 490, 41.64), (568, 494, 41.32)],
            ),
        ];
        resolve_grade_crossings(&mut routes, 420.0);
        assert!(
            routes[1]
                .bridge_spans
                .iter()
                .any(|span| span.kind == "overpass-over-low")
        );
    }

    #[test]
    fn water_is_refused_when_crossing_is_disabled() {
        let scene = SceneSpec {
            scene_id: "route".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let terrain = terrain(&scene);
        let mut style = style(false);
        style["routeEvolution"]["routes"][0]
            .as_object_mut()
            .unwrap()
            .remove("via");
        let water = lake(&scene, &style);
        let plan = plan(&scene, &terrain, water, &style).unwrap().unwrap();
        require_pass(&plan).unwrap();
        assert!(plan.routes[0].bridge_spans.is_empty());
    }

    #[test]
    fn fill_above_six_metres_requires_feature_cut_authority() {
        let mut style = style(true);
        style["routeEvolution"]["maxFillM"] = serde_json::json!(20);
        assert!(validate_style(&style).is_err());

        style["urbanSlice"] = serde_json::json!({
            "version": 1,
            "mode": "feature-cut",
            "settlementScale": "metropolis",
            "focusFeature": "central-ridge-cut"
        });
        validate_style(&style).unwrap();
    }
}
