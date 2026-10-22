//! Opt-in urban land growth along the road network: around its transport nodes and along its main
//! roads, instead of the radial density field.
//!
//! # Why this exists
//!
//! The default plan (`plan_landuse_zones`) ranks the scene's zoning cells by distance to the scene
//! CENTRE and spends the urban budget outward from there. That is a density prior, not a city. On a
//! 1 km natural-settlement probe it planned 4 % of the scene as urban in two 143 m cells around the
//! geometric centre, and those two cells happened to sit where almost no street could carry a
//! building: the frontage programme placed 3 of 120 requested homes, and the published world held 92
//! entities against 146 before the urban-edge policy confined conversions to the city's rim. The
//! measurement named the lever - not the conversion band, but the shape of the zoning plan itself.
//!
//! # The evidence in this repository
//!
//! `docs/modern-mainland-morphology.zh-CN.md` states the stable common structure of modern mainland
//! cities: high-density cores sit at **transport convergences** (交通汇聚点), and the ordinary urban
//! districts form a **400-900 m living circle** around those cores and along the arterial corridors.
//! The density gradient is "distance to the nearest core".
//! `docs/20260906-human-settlement-morphology-research.zh-CN.md` adds the same rule from the
//! settlement side: a city grows along its arterial roads and public-transport nodes first, and the
//! blocks and lots follow the street.
//!
//! # The plan it produces
//!
//! ```text
//! budget    = the urban area the RADIAL plan already allocated (urbanCoreRatio & co unchanged)
//! nodes     = k road nodes, k = floor(budget / (pi * R^2)) at least 1; a node is ranked by the
//!             main-road metres its own living circle reaches (a transport convergence first)
//! axes      = the longest/widest main-road runs, as many as the budget needs
//! tier 0    = the AXIS_WIDTH_M corridor along each axis, inside the node's living circle
//! tier 1    = the rest of a node's living circle
//! tier 2    = the radial remainder, spent only when tiers 0 and 1 cannot hold the budget
//! urban     = the nearest pieces first, cut to EXACTLY the budget
//! rural     = every non-reserved square metre the urban land does not take
//! nature    = the radial plan's reserved cells, untouched
//! ```
//!
//! # Why a corridor and not "the cells the road crosses"
//!
//! The corridor is a real width, not a cell filter. A frontage parcel is placed `setbackM` from the
//! road's own station and is `depthM` deep: the probe's homes are 24 m deep with an 8 m setback, so a
//! built lot needs 32 m on each side of the road. The world's own road hierarchy tops out at 16 m
//! (`auto_roads::RoadHierarchy::validate` accepts 8/12/16), and
//! `docs/modern-mainland-morphology.zh-CN.md` gives 18-32 m as a mainland arterial's pavement width,
//! so 16 m of pavement plus two 32 m frontage rows is 80 m. A 64 m corridor cannot hold a 24 m home
//! with an 8 m setback on both sides; a 120 m corridor is almost as deep as the 128-143 m zoning cell
//! a town scene is planned on, which would make the width meaningless. 80 m is the narrowest round
//! corridor that holds the plan's own deepest frontage lot on both sides.
//!
//! # Determinism
//!
//! Every choice is an exact integer order over published geometry: node scores are whole metres,
//! distances are Chebyshev metres, ties break on coordinates and cell index, and no clock, hash map
//! or floating-point summation reaches a decision. Two runs of one input publish the same bytes.
use super::road_network::RoadNetworkPlan;
use super::{LandUseProfile, LandUseZone, SceneSpec, WaterGeometry};
use std::collections::BTreeMap;

/// The only accepted value of `style.landUseProfile.urbanGrowth`.
pub const OPT_IN: &str = "axis-and-node";

/// Radius of one node's living circle, in metres.
///
/// `docs/modern-mainland-morphology.zh-CN.md` gives the ordinary urban band as "围绕核心和轨道/主干路
/// 形成 400-900m 生活圈" and the school/service radius as 400-900 m. The chosen value is that range's
/// midpoint: 600 m is 7-8 minutes on foot at the accepted 75-80 m/min walking speed, it is the radius
/// at which two cores 1.2 km apart have exactly touching circles (the observed spacing of mainland
/// cores), and one 600 m circle is 1.13 km2 - the size of one 街道 district. The 400 m end would leave
/// the corridor outside a single neighbourhood; the 900 m end would let one circle swallow a whole
/// 1 km probe and make the node choice untestable.
pub const NODE_RADIUS_M: i32 = 600;

/// The width of the corridor one main road claims, in metres (the street plus one built frontage row
/// on each side - see the module documentation for the 16 m + 2 x 32 m derivation).
pub const AXIS_WIDTH_M: i32 = 80;

/// The closest two PARALLEL main roads may be and still be different roads, in metres.
///
/// `docs/modern-mainland-morphology.zh-CN.md` puts the ordinary urban district on a 400-900 m living
/// circle around its core and along its arterials, so two arterials closer together than the
/// smallest living circle serve the same district: taking both would weld their corridors into one
/// slab wider than a district and turn the axis into a block. 400 m is that lower bound.
pub const AXIS_SPACING_M: i32 = 400;

/// The shortest road run that counts as a main road, in metres.
///
/// Two corridor widths: a run shorter than that is a junction, not a road, and growing a city along
/// it would plan land around a crossroads. A network with no run this long falls back to its longest
/// run.
pub const AXIS_MIN_LENGTH_M: i32 = 160;

/// The published `source` of every zone this policy plans.
pub const SOURCE: &str = "axis-and-node-v1";

/// The published `source` when the style opts in but the scene has no usable road network: the plan
/// then IS the radial plan, and says so. Named so a reader cannot mistake it for a grown plan.
pub const SOURCE_NO_NETWORK: &str = "axis-and-node-v1-no-road-network";

/// The published `source` when the corridor and the living circles cannot hold the whole urban
/// budget and the radial ranking had to place the rest: the remainder is honest, and named.
pub const SOURCE_RADIAL_REMAINDER: &str = "axis-and-node-v1+radial-remainder";

/// Is the axis-and-node policy declared by this style?
///
/// The value must be exactly [`OPT_IN`]: a near miss is not silently reinterpreted, because the
/// switch decides where a world's urban land is.
pub fn enabled(style: &serde_json::Value) -> bool {
    style
        .get("landUseProfile")
        .and_then(|profile| profile.get("urbanGrowth"))
        .and_then(serde_json::Value::as_str)
        == Some(OPT_IN)
}

/// One maximal straight run of contiguous road segments: a main road, as the network already builds
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Axis {
    /// `[x, z]` of the run's low end.
    pub from: [i32; 2],
    /// `[x, z]` of the run's high end.
    pub to: [i32; 2],
    /// The widest pavement the run's own road profiles declare, in metres. `0` for a run of
    /// profile-less segments (a lane the alignment solver never gave a street profile to).
    pub width_m: u32,
}

impl Axis {
    pub fn horizontal(&self) -> bool {
        self.from[1] == self.to[1]
    }

    pub fn vertical(&self) -> bool {
        !self.horizontal()
    }

    pub fn length_m(&self) -> i32 {
        if self.horizontal() {
            self.to[0] - self.from[0]
        } else {
            self.to[1] - self.from[1]
        }
    }

    /// The corridor this axis claims, as `[x0, z0, x1, z1]` in whole metres, clipped to `radius`
    /// metres (Chebyshev) of `node` so a corridor never runs past the living circle it belongs to.
    fn corridor(&self, node: [i32; 2], radius: i32) -> [i64; 4] {
        let half = i64::from(AXIS_WIDTH_M) / 2;
        let rect = if self.horizontal() {
            [
                i64::from(self.from[0]),
                i64::from(self.from[1]) - half,
                i64::from(self.to[0]),
                i64::from(self.from[1]) + half,
            ]
        } else {
            [
                i64::from(self.from[0]) - half,
                i64::from(self.from[1]),
                i64::from(self.from[0]) + half,
                i64::from(self.to[1]),
            ]
        };
        clip_to_square(rect, node, radius)
    }
}

/// One node of the network the plan grows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node {
    pub at: [i32; 2],
    /// Road segments meeting at this node (4 is a crossing in the planner's own grid).
    pub degree: u32,
    /// True when one of the node's roads is a registered water crossing: a bridge head.
    pub bridgehead: bool,
}

/// The existing road network, as the growth policy reads it: its nodes and its main-road runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Network {
    pub nodes: Vec<Node>,
    pub axes: Vec<Axis>,
}

impl Network {
    /// Read a planned road network into its nodes and its maximal straight runs.
    ///
    /// A run is merged from contiguous collinear segments of one orientation. Segments that carry a
    /// street profile are merged on their own first, so a run of profiled road is a run the world can
    /// actually front; runs of profile-less lanes are the named fallback for a network whose
    /// alignment solver refused every base edge.
    pub fn from_plan(plan: &RoadNetworkPlan) -> Network {
        let mut degree = BTreeMap::<usize, u32>::new();
        let mut bridgehead = BTreeMap::<usize, bool>::new();
        for edge in &plan.edges {
            *degree.entry(edge.from).or_default() += 1;
            *degree.entry(edge.to).or_default() += 1;
            if plan.water_crossings.iter().any(|c| c.road_id == edge.id) {
                bridgehead.insert(edge.from, true);
                bridgehead.insert(edge.to, true);
            }
        }
        let mut nodes: Vec<Node> = plan
            .nodes
            .iter()
            .map(|node| Node {
                at: [node.world_x, node.world_z],
                degree: degree.get(&node.id).copied().unwrap_or(0),
                bridgehead: bridgehead.get(&node.id).copied().unwrap_or(false),
            })
            .collect();
        nodes.sort_by_key(|node| node.at);
        let mut axes = merge_runs(
            plan.edges
                .iter()
                .filter(|edge| edge.profile.is_some())
                .map(segment_of),
        );
        if axes.is_empty() {
            axes = merge_runs(plan.edges.iter().map(segment_of));
        }
        Network { nodes, axes }
    }
}

/// One road segment, decoupled from the plan record so the run merge is a pure function.
#[derive(Debug, Clone, Copy)]
struct Segment {
    world_x: i32,
    world_z: i32,
    length_m: u32,
    vertical: bool,
    width_m: u32,
}

fn segment_of(edge: &super::road_network::RoadEdge) -> Segment {
    Segment {
        world_x: edge.world_x,
        world_z: edge.world_z,
        length_m: edge.length_m,
        vertical: (edge.rotation_y_deg as i32).rem_euclid(180) != 0,
        width_m: edge
            .profile
            .as_ref()
            .map_or(0, |profile| profile.road_width_m),
    }
}

/// Merge collinear contiguous segments into maximal straight runs.
fn merge_runs(segments: impl Iterator<Item = Segment>) -> Vec<Axis> {
    // (orientation, fixed coordinate) -> intervals of (low, high, widest profile)
    let mut lines = BTreeMap::<(u8, i32), Vec<(i32, i32, u32)>>::new();
    for segment in segments {
        let half = (segment.length_m / 2) as i32;
        if !segment.vertical {
            lines.entry((0, segment.world_z)).or_default().push((
                segment.world_x - half,
                segment.world_x + half,
                segment.width_m,
            ));
        } else {
            lines.entry((1, segment.world_x)).or_default().push((
                segment.world_z - half,
                segment.world_z + half,
                segment.width_m,
            ));
        }
    }
    let mut axes = Vec::new();
    for ((orientation, fixed), mut spans) in lines {
        spans.sort();
        let (mut lo, mut hi, mut width) = spans[0];
        for (next_lo, next_hi, next_width) in spans.into_iter().skip(1) {
            if next_lo <= hi {
                hi = hi.max(next_hi);
                width = width.max(next_width);
                continue;
            }
            axes.push(axis_of(orientation, fixed, lo, hi, width));
            (lo, hi, width) = (next_lo, next_hi, next_width);
        }
        axes.push(axis_of(orientation, fixed, lo, hi, width));
    }
    axes
}

fn axis_of(orientation: u8, fixed: i32, lo: i32, hi: i32, width: u32) -> Axis {
    if orientation == 0 {
        Axis {
            from: [lo, fixed],
            to: [hi, fixed],
            width_m: width,
        }
    } else {
        Axis {
            from: [fixed, lo],
            to: [fixed, hi],
            width_m: width,
        }
    }
}

/// Which nodes and axes one growth plan used, with the constants its geometry comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub nodes: Vec<[i32; 2]>,
    pub axes: Vec<Axis>,
    pub radius_m: i32,
    pub axis_width_m: i32,
}

impl Selection {
    fn corridors(&self) -> Vec<[i64; 4]> {
        self.axes
            .iter()
            .filter_map(|axis| {
                nearest_node(&axis_rect(axis), &self.nodes)
                    .map(|node| axis.corridor(node, self.radius_m))
            })
            .collect()
    }

    /// Chebyshev metres from `rect` to the nearest axis corridor (`0` inside or touching one).
    pub fn distance_to_axis(&self, rect: &[i64; 4]) -> i64 {
        self.corridors()
            .iter()
            .map(|corridor| rect_distance(rect, corridor))
            .min()
            .unwrap_or(i64::MAX / 4)
    }

    /// Chebyshev metres from `rect` to the nearest node point.
    pub fn distance_to_node(&self, rect: &[i64; 4]) -> i64 {
        self.nodes
            .iter()
            .map(|node| point_distance(rect, *node))
            .min()
            .unwrap_or(i64::MAX / 4)
    }
}

/// One growth plan: the zones, the geometry they came from, and the published source.
#[derive(Debug, Clone)]
pub struct GrowthPlan {
    pub zones: Vec<LandUseZone>,
    pub selection: Option<Selection>,
    pub source: String,
}

/// One rectangle of land the plan may spend its budget on. Every piece of one plan is disjoint from
/// every other, so the budget is spent exactly once.
#[derive(Debug, Clone, Copy)]
struct Piece {
    /// 0 = axis corridor, 1 = living circle, 2 = radial remainder.
    tier: u8,
    /// Over open water. Dry land always comes first inside a tier.
    water: bool,
    /// Chebyshev metres to the nearest node (tiers 0 and 1), or the radial rank (tier 2).
    rank: i64,
    /// The zoning cell the piece was cut from, in plan order.
    cell: usize,
    /// Which of that cell's pieces this is, so ordering never depends on a hash order.
    order: usize,
    /// `[x0, z0, x1, z1]`, half-open.
    rect: [i64; 4],
}

impl Piece {
    fn area(&self) -> i64 {
        rect_area(&self.rect)
    }
}

/// Plan one scene's urban land under the axis-and-node policy.
///
/// `network` is the road network the world builds under its own (radial) land-use plan - the plan it
/// already had before this policy could move a cell. `None` degrades to the radial plan, by name.
///
/// The urban, rural and nature TOTALS are those of the radial plan, exactly: the budget is its urban
/// area, the reserved cells are its nature cells, and rural land is whatever is left. Only the
/// placement of the urban land changes.
pub fn plan(
    scene: &SceneSpec,
    profile: &LandUseProfile,
    network: Option<&Network>,
    water: WaterGeometry,
) -> GrowthPlan {
    let radial = super::plan_landuse_zones(scene, profile);
    let budget: i64 = radial
        .iter()
        .filter(|zone| zone.kind == "urban")
        .map(zone_area)
        .sum();
    // Protected land is EVERY kind the plan does not develop: `nature` and `unassigned`. That is
    // the same rule `ZoningMask` applies to roads and earthwork, so a cell the radial plan reserved
    // or left unassigned can never become urban land here, and its total cannot move.
    let reserved: Vec<&LandUseZone> = radial
        .iter()
        .filter(|zone| !matches!(zone.kind.as_str(), "urban" | "rural"))
        .collect();
    let cells = super::zoning_cells(scene);
    let Some(network) = network.filter(|network| !network.axes.is_empty()) else {
        return GrowthPlan {
            zones: with_source(radial, SOURCE_NO_NETWORK),
            selection: None,
            source: SOURCE_NO_NETWORK.into(),
        };
    };
    if cells.is_empty() || budget <= 0 {
        return GrowthPlan {
            zones: with_source(radial, SOURCE),
            selection: None,
            source: SOURCE.into(),
        };
    }
    let Some(selection) = choose(scene, network, budget) else {
        return GrowthPlan {
            zones: with_source(radial, SOURCE_NO_NETWORK),
            selection: None,
            source: SOURCE_NO_NETWORK.into(),
        };
    };
    // Reserved (nature) cells are the plan's protected land: they never become urban, so the urban
    // land can never encroach on it, and the nature total cannot move.
    let mut reserved_cells = vec![false; cells.len()];
    for zone in &reserved {
        for (index, cell) in cells.iter().enumerate() {
            if cell_holds(zone, cell) {
                reserved_cells[index] = true;
            }
        }
    }
    let corridors = selection.corridors();
    let circles: Vec<[i64; 4]> = selection
        .nodes
        .iter()
        .map(|node| square(*node, selection.radius_m))
        .collect();
    // Every piece the policy may spend its budget on. The pieces tile every non-reserved cell, so
    // the budget can always be reached exactly and no square metre is ever counted twice.
    let mut pieces = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        if reserved_cells[index] {
            continue;
        }
        let box_ = cell_rect(cell);
        let mut order = 0;
        let mut here: Vec<[i64; 4]> = Vec::new();
        for corridor in &corridors {
            if let Some(rect) = intersect(&box_, corridor) {
                here.push(rect);
            }
        }
        let (axis_land, rest) = decompose(&box_, &here);
        for rect in axis_land {
            pieces.push(Piece {
                tier: 0,
                water: water.covers(rect),
                rank: node_rank(&rect, &selection),
                cell: index,
                order,
                rect,
            });
            order += 1;
        }
        let in_circle = circles.iter().any(|circle| rect_inside(&box_, circle));
        if here.is_empty() {
            pieces.push(Piece {
                tier: if in_circle { 1 } else { 2 },
                water: water.covers(box_),
                rank: if in_circle {
                    node_rank(&box_, &selection)
                } else {
                    radial_rank(scene, cell)
                },
                cell: index,
                order,
                rect: box_,
            });
            continue;
        }
        for rect in rest {
            let tier = if in_circle && circles.iter().any(|circle| rect_inside(&rect, circle)) {
                1
            } else {
                2
            };
            pieces.push(Piece {
                tier,
                water: water.covers(rect),
                rank: if tier == 1 {
                    node_rank(&rect, &selection)
                } else {
                    radial_rank(scene, cell)
                },
                cell: index,
                order,
                rect,
            });
            order += 1;
        }
    }
    pieces.sort_by_key(|piece| (piece.tier, piece.water, piece.rank, piece.cell, piece.order));
    // Spend the budget from the nearest piece outward, and cut the piece that crosses the budget so
    // the total is exact instead of "as close as the grid allows".
    let mut spent = 0_i64;
    let mut remainder = false;
    let mut urban: BTreeMap<usize, Vec<[i64; 4]>> = BTreeMap::new();
    for piece in &pieces {
        if spent >= budget {
            break;
        }
        let area = piece.area();
        if area <= 0 {
            continue;
        }
        if piece.tier == 2 {
            remainder = true;
        }
        let room = budget - spent;
        if area <= room {
            urban.entry(piece.cell).or_default().push(piece.rect);
            spent += area;
            continue;
        }
        for rect in cut(piece, room) {
            urban.entry(piece.cell).or_default().push(rect);
            spent += rect_area(&rect);
        }
        break;
    }
    debug_assert_eq!(spent, budget, "the growth budget must be spent exactly");
    let source = if remainder {
        SOURCE_RADIAL_REMAINDER
    } else {
        SOURCE
    };
    let mut zones = Vec::with_capacity(cells.len());
    for (index, cell) in cells.iter().enumerate() {
        let box_ = cell_rect(cell);
        if reserved_cells[index] {
            let zone = reserved
                .iter()
                .find(|zone| cell_holds(zone, cell))
                .expect("a reserved cell has its radial zone");
            zones.push(LandUseZone {
                source: source.into(),
                ..(*zone).clone()
            });
            continue;
        }
        let covered = urban.get(&index).cloned().unwrap_or_default();
        let (built, rest) = decompose(&box_, &covered);
        let mut parts = 0;
        for rect in built {
            zones.push(zone_of(scene, "urban", cell, parts, rect, source));
            parts += 1;
        }
        for rect in rest {
            zones.push(zone_of(scene, "rural", cell, parts, rect, source));
            parts += 1;
        }
    }
    GrowthPlan {
        zones,
        selection: Some(selection),
        source: source.into(),
    }
}

fn zone_of(
    scene: &SceneSpec,
    kind: &str,
    cell: &super::ZoningCell,
    part: usize,
    rect: [i64; 4],
    source: &str,
) -> LandUseZone {
    let suffix = if part == 0 {
        String::new()
    } else {
        format!(".{part}")
    };
    LandUseZone {
        id: format!("{}.zone.{kind}.{}{suffix}", scene.scene_id, cell.index),
        kind: kind.into(),
        world_x: rect[0] as i32,
        world_z: rect[1] as i32,
        width_m: (rect[2] - rect[0]) as u32,
        depth_m: (rect[3] - rect[1]) as u32,
        source: source.into(),
    }
}

fn zone_area(zone: &LandUseZone) -> i64 {
    i64::from(zone.width_m) * i64::from(zone.depth_m)
}

fn rect_area(rect: &[i64; 4]) -> i64 {
    (rect[2] - rect[0]).max(0) * (rect[3] - rect[1]).max(0)
}

fn cell_rect(cell: &super::ZoningCell) -> [i64; 4] {
    [
        i64::from(cell.world_x),
        i64::from(cell.world_z),
        i64::from(cell.world_x) + i64::from(cell.width_m),
        i64::from(cell.world_z) + i64::from(cell.depth_m),
    ]
}

fn cell_holds(zone: &LandUseZone, cell: &super::ZoningCell) -> bool {
    zone.world_x == cell.world_x
        && zone.world_z == cell.world_z
        && zone.width_m == cell.width_m
        && zone.depth_m == cell.depth_m
}

/// The radial rank of a cell: four times its squared distance from the scene centre, in whole
/// units. Integer, and the same order the radial plan sorts by.
fn radial_rank(scene: &SceneSpec, cell: &super::ZoningCell) -> i64 {
    let cx = i64::from(scene.origin_x) * 2 + i64::from(scene.width_m);
    let cz = i64::from(scene.origin_z) * 2 + i64::from(scene.depth_m);
    let x2 = i64::from(cell.world_x) * 2 + i64::from(cell.width_m);
    let z2 = i64::from(cell.world_z) * 2 + i64::from(cell.depth_m);
    (x2 - cx) * (x2 - cx) + (z2 - cz) * (z2 - cz)
}

fn node_rank(rect: &[i64; 4], selection: &Selection) -> i64 {
    selection.distance_to_node(rect)
}

/// Chebyshev metres from a rectangle to a point (`0` when the point is on the rectangle).
fn point_distance(rect: &[i64; 4], at: [i32; 2]) -> i64 {
    let x = i64::from(at[0]);
    let z = i64::from(at[1]);
    let dx = (rect[0] - x).max(x - rect[2]).max(0);
    let dz = (rect[1] - z).max(z - rect[3]).max(0);
    dx.max(dz)
}

/// Chebyshev metres between two rectangles (`0` when they touch or overlap).
fn rect_distance(a: &[i64; 4], b: &[i64; 4]) -> i64 {
    let dx = (b[0] - a[2]).max(a[0] - b[2]).max(0);
    let dz = (b[1] - a[3]).max(a[1] - b[3]).max(0);
    dx.max(dz)
}

/// Is `inner` wholly inside `outer`?
fn rect_inside(inner: &[i64; 4], outer: &[i64; 4]) -> bool {
    inner[0] >= outer[0] && inner[1] >= outer[1] && inner[2] <= outer[2] && inner[3] <= outer[3]
}

fn nearest_node(rect: &[i64; 4], nodes: &[[i32; 2]]) -> Option<[i32; 2]> {
    nodes
        .iter()
        .copied()
        .min_by_key(|node| (point_distance(rect, *node), *node))
}

/// The square of Chebyshev radius `radius` around `node`: a circle in the plan's own block metric,
/// and the reason every piece of this plan stays an axis-aligned rectangle.
fn square(node: [i32; 2], radius: i32) -> [i64; 4] {
    let (x, z, r) = (i64::from(node[0]), i64::from(node[1]), i64::from(radius));
    [x - r, z - r, x + r, z + r]
}

fn clip_to_square(rect: [i64; 4], node: [i32; 2], radius: i32) -> [i64; 4] {
    let square = square(node, radius);
    [
        rect[0].max(square[0]),
        rect[1].max(square[1]),
        rect[2].min(square[2]),
        rect[3].min(square[3]),
    ]
}

fn axis_rect(axis: &Axis) -> [i64; 4] {
    let half = i64::from(AXIS_WIDTH_M) / 2;
    if axis.horizontal() {
        [
            i64::from(axis.from[0]),
            i64::from(axis.from[1]) - half,
            i64::from(axis.to[0]),
            i64::from(axis.from[1]) + half,
        ]
    } else {
        [
            i64::from(axis.from[0]) - half,
            i64::from(axis.from[1]),
            i64::from(axis.from[0]) + half,
            i64::from(axis.to[1]),
        ]
    }
}

/// How many metres of `axis` lie within `radius` metres (Chebyshev) of `node`.
fn axis_span_within(axis: &Axis, node: [i32; 2], radius: i32) -> i32 {
    let r = i64::from(radius);
    let n = [i64::from(node[0]), i64::from(node[1])];
    if axis.horizontal() {
        if (n[1] - i64::from(axis.from[1])).abs() > r {
            return 0;
        }
        let lo = i64::from(axis.from[0]).max(n[0] - r);
        let hi = i64::from(axis.to[0]).min(n[0] + r);
        (hi - lo).max(0) as i32
    } else {
        if (n[0] - i64::from(axis.from[0])).abs() > r {
            return 0;
        }
        let lo = i64::from(axis.from[1]).max(n[1] - r);
        let hi = i64::from(axis.to[1]).min(n[1] + r);
        (hi - lo).max(0) as i32
    }
}

/// The chosen nodes and axes, or `None` when the network offers nothing to grow along.
fn choose(scene: &SceneSpec, network: &Network, budget: i64) -> Option<Selection> {
    if network.nodes.is_empty() || network.axes.is_empty() {
        return None;
    }
    let mut runs: Vec<Axis> = network
        .axes
        .iter()
        .copied()
        .filter(|axis| axis.length_m() >= AXIS_MIN_LENGTH_M)
        .collect();
    if runs.is_empty() {
        runs = network.axes.clone();
    }
    runs.sort_by(|a, b| {
        b.width_m
            .cmp(&a.width_m)
            .then(b.length_m().cmp(&a.length_m()))
            .then(a.from.cmp(&b.from))
            .then(a.to.cmp(&b.to))
    });
    // k: the living circles the EXISTING budget pays for, at least one. One 600 m circle holds
    // pi * R^2 of land, so a budget can describe at most that many cores - and the world always has
    // its first core.
    let circle_m2 =
        (std::f64::consts::PI * f64::from(NODE_RADIUS_M) * f64::from(NODE_RADIUS_M)).round() as i64;
    let k = ((budget / circle_m2.max(1)).max(1) as usize).min(network.nodes.len());
    let centre = [
        i64::from(scene.origin_x) * 2 + i64::from(scene.width_m),
        i64::from(scene.origin_z) * 2 + i64::from(scene.depth_m),
    ];
    // A transport convergence is the node whose own living circle reaches the most main-road metres.
    let mut scored: Vec<(i64, i64, [i32; 2], bool)> = network
        .nodes
        .iter()
        .map(|node| {
            let metres: i64 = runs
                .iter()
                .map(|axis| i64::from(axis_span_within(axis, node.at, NODE_RADIUS_M)))
                .sum();
            let dx = i64::from(node.at[0]) * 2 - centre[0];
            let dz = i64::from(node.at[1]) * 2 - centre[1];
            (
                metres,
                dx * dx + dz * dz,
                node.at,
                node.bridgehead || node.degree >= 3,
            )
        })
        .collect();
    scored.sort_by_key(|entry| (std::cmp::Reverse(entry.0), entry.1, entry.2, !entry.3));
    let nodes: Vec<[i32; 2]> = scored.iter().take(k).map(|entry| entry.2).collect();
    // As many axes as the budget needs: each corridor is its length inside the living circle of the
    // node it belongs to, times the corridor width, and the loop stops as soon as that land can hold
    // the whole budget.
    let mut usable: Vec<Axis> = runs
        .into_iter()
        .filter(|axis| {
            nearest_node(&axis_rect(axis), &nodes)
                .is_some_and(|node| axis_span_within(axis, node, NODE_RADIUS_M) > 0)
        })
        .collect();
    if usable.is_empty() {
        usable = network.axes.clone();
    }
    let mut axes: Vec<Axis> = Vec::new();
    let mut skipped: Vec<Axis> = Vec::new();
    let mut band = 0_i64;
    // Two parallel arterials closer than a living circle serve one district: taking both would weld
    // their corridors into a slab wider than the corridor the policy promises. A skipped axis is kept
    // for the degenerate case where the world has nothing else, and reached only when the spacing rule
    // would otherwise leave the budget homeless.
    for axis in usable {
        if axes.iter().any(|chosen| parallel_and_close(chosen, &axis)) {
            skipped.push(axis);
            continue;
        }
        if let Some(node) = nearest_node(&axis_rect(&axis), &nodes) {
            band += i64::from(axis_span_within(&axis, node, NODE_RADIUS_M).max(0))
                * i64::from(AXIS_WIDTH_M);
        }
        axes.push(axis);
        if band >= budget {
            break;
        }
    }
    // A skipped axis is NEVER reached to make up a budget: when the world's main roads are all one
    // congested strip, the honest plan is one corridor plus the living circles (tier 1), not a slab
    // welded out of parallel roads. Only a scene with no usable axis at all falls back to one.
    let _ = skipped;
    if axes.is_empty() {
        return None;
    }
    Some(Selection {
        nodes,
        axes,
        radius_m: NODE_RADIUS_M,
        axis_width_m: AXIS_WIDTH_M,
    })
}

/// Are these two runs parallel and closer together than one living circle?
fn parallel_and_close(a: &Axis, b: &Axis) -> bool {
    if a.horizontal() != b.horizontal() {
        return false;
    }
    let fixed = |axis: &Axis| {
        if axis.horizontal() {
            i64::from(axis.from[1])
        } else {
            i64::from(axis.from[0])
        }
    };
    (fixed(a) - fixed(b)).abs() < i64::from(AXIS_SPACING_M)
}

fn intersect(a: &[i64; 4], b: &[i64; 4]) -> Option<[i64; 4]> {
    let rect = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];
    (rect[2] > rect[0] && rect[3] > rect[1]).then_some(rect)
}

/// Cut `room` square metres out of `piece`.
///
/// The cut runs across the piece's longer side, which for a corridor slice is the corridor's
/// length: a clipped corridor is a shorter corridor at its full width, never a narrowed one. The
/// square metres left over after whole rows are a one-row strip at most as wide as the piece.
fn cut(piece: &Piece, room: i64) -> Vec<[i64; 4]> {
    let [x0, z0, x1, z1] = piece.rect;
    let (w, d) = (x1 - x0, z1 - z0);
    let mut out = Vec::new();
    if w >= d {
        let length = (room / d).clamp(0, w);
        let rest = room - length * d;
        if length > 0 {
            out.push([x0, z0, x0 + length, z1]);
        }
        if rest > 0 {
            out.push([x0 + length, z0, x0 + length + 1, z0 + rest]);
        }
    } else {
        let depth = (room / w).clamp(0, d);
        let rest = room - depth * w;
        if depth > 0 {
            out.push([x0, z0, x1, z0 + depth]);
        }
        if rest > 0 {
            out.push([x0, z0 + depth, x0 + rest, z0 + depth + 1]);
        }
    }
    out
}

/// Split a cell's box into the part `urban` rectangles cover and the part they do not.
///
/// Both lists are pairwise disjoint, stay inside `box`, and together tile it exactly: that property
/// IS the zoning conservation, mutual-exclusion and uniqueness contract the audits check.
fn decompose(box_: &[i64; 4], urban: &[[i64; 4]]) -> (Vec<[i64; 4]>, Vec<[i64; 4]>) {
    let [x0, z0, x1, z1] = *box_;
    let mut edges: Vec<i64> = vec![z0, z1];
    for rect in urban {
        edges.push(rect[1].clamp(z0, z1));
        edges.push(rect[3].clamp(z0, z1));
    }
    edges.sort();
    edges.dedup();
    let mut built = Vec::new();
    let mut rest = Vec::new();
    for pair in edges.windows(2) {
        let (za, zb) = (pair[0], pair[1]);
        if zb <= za {
            continue;
        }
        // The x intervals of every urban rectangle covering this row band.
        let mut spans: Vec<(i64, i64)> = urban
            .iter()
            .filter(|rect| rect[1] <= za && rect[3] >= zb)
            .map(|rect| (rect[0].max(x0), rect[2].min(x1)))
            .filter(|(a, b)| b > a)
            .collect();
        spans.sort();
        let mut merged: Vec<(i64, i64)> = Vec::new();
        for (a, b) in spans {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        let mut cursor = x0;
        for (a, b) in merged {
            if a > cursor {
                rest.push([cursor, za, a, zb]);
            }
            built.push([a, za, b, zb]);
            cursor = b;
        }
        if cursor < x1 {
            rest.push([cursor, za, x1, zb]);
        }
    }
    (built, rest)
}

fn with_source(zones: Vec<LandUseZone>, source: &str) -> Vec<LandUseZone> {
    zones
        .into_iter()
        .map(|mut zone| {
            zone.source = source.into();
            zone
        })
        .collect()
}

impl WaterGeometry {
    /// Does this rectangle touch the scene's water body? Sampled at the corners and the centre - a
    /// planning allocation, not an earthwork probe.
    fn covers(self, rect: [i64; 4]) -> bool {
        // The samples are INSET half a metre, so a band that only touches the waterline (its edge is
        // the river's own centre line, which happens when a cell boundary lands there) is dry land,
        // while a band that stands over the water is caught. A planning allocation, not an earthwork
        // probe.
        let (x0, z0, x1, z1) = (
            rect[0] as f64 + 0.5,
            rect[1] as f64 + 0.5,
            rect[2] as f64 - 0.5,
            rect[3] as f64 - 0.5,
        );
        for x in [x0, (x0 + x1) * 0.5, x1] {
            for z in [z0, (z0 + z1) * 0.5, z1] {
                if self.contains_point(x, z) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{LandUseProfile, WaterKind, water_geometry};

    fn scene_at(width_m: u32, depth_m: u32) -> SceneSpec {
        SceneSpec {
            scene_id: "scene-0".into(),
            origin_x: 0,
            origin_z: 0,
            width_m,
            depth_m,
            seed_offset: 0,
        }
    }

    fn profile(urban: f64, rural: f64, nature: f64) -> LandUseProfile {
        LandUseProfile::from_style(&serde_json::json!({
            "landUseProfile": {
                "urbanCoreRatio": urban,
                "suburbanRatio": 0.0,
                "farmRatio": rural,
                "pastureRatio": 0.0,
                "forestRatio": nature,
                "reserveRatio": 0.0,
                "greenRatio": 0.0,
            }
        }))
        .expect("profile")
    }

    /// A crossed pair of main roads through the middle of the scene, with their own nodes.
    fn cross_network(scene: &SceneSpec) -> Network {
        let mid_x = (scene.width_m / 2) as i32;
        let mid_z = (scene.depth_m / 2) as i32;
        let reach = (scene.width_m.min(scene.depth_m) / 2) as i32;
        Network {
            nodes: vec![
                Node {
                    at: [mid_x - 128, mid_z],
                    degree: 2,
                    bridgehead: false,
                },
                Node {
                    at: [mid_x, mid_z],
                    degree: 4,
                    bridgehead: false,
                },
                Node {
                    at: [mid_x + 128, mid_z],
                    degree: 2,
                    bridgehead: false,
                },
                Node {
                    at: [mid_x, mid_z - 128],
                    degree: 3,
                    bridgehead: false,
                },
                Node {
                    at: [mid_x, mid_z + 128],
                    degree: 3,
                    bridgehead: true,
                },
            ],
            axes: vec![
                Axis {
                    from: [mid_x - reach, mid_z],
                    to: [mid_x + reach, mid_z],
                    width_m: 16,
                },
                Axis {
                    from: [mid_x, mid_z - reach],
                    to: [mid_x, mid_z + reach],
                    width_m: 12,
                },
            ],
        }
    }

    fn waterless(scene: &SceneSpec) -> WaterGeometry {
        water_geometry(WaterKind::None, &[], scene, &serde_json::Value::Null)
    }

    fn totals(zones: &[LandUseZone]) -> (i64, i64, i64) {
        let mut sum = (0, 0, 0);
        for zone in zones {
            let area = zone_area(zone);
            match zone.kind.as_str() {
                "urban" => sum.0 += area,
                "rural" => sum.1 += area,
                _ => sum.2 += area,
            }
        }
        sum
    }

    fn hash(zones: &[LandUseZone]) -> String {
        blake3::hash(&serde_json::to_vec(zones).expect("zones serialize"))
            .to_hex()
            .to_string()
    }

    fn rect_of(zone: &LandUseZone) -> [i64; 4] {
        [
            i64::from(zone.world_x),
            i64::from(zone.world_z),
            i64::from(zone.world_x) + i64::from(zone.width_m),
            i64::from(zone.world_z) + i64::from(zone.depth_m),
        ]
    }

    #[test]
    fn the_switch_is_the_exact_string_and_nothing_else() {
        let style = |value: serde_json::Value| serde_json::json!({ "landUseProfile": value });
        assert!(enabled(&style(
            serde_json::json!({"urbanGrowth": "axis-and-node"})
        )));
        for other in [
            serde_json::json!({"urbanGrowth": "axis-and-node "}),
            serde_json::json!({"urbanGrowth": "Axis-and-Node"}),
            serde_json::json!({"urbanGrowth": true}),
            serde_json::json!({"urbanGrowth": "radial"}),
            serde_json::json!({}),
        ] {
            assert!(!enabled(&style(other.clone())), "{other} must not opt in");
        }
        assert!(!enabled(&serde_json::json!({})));
    }

    #[test]
    fn off_the_plan_is_the_radial_plan_byte_for_byte() {
        let scene = scene_at(1000, 1000);
        let land_use = profile(0.02, 0.86, 0.12);
        let water = waterless(&scene);
        let radial = crate::worldgen::plan_landuse_zones(&scene, &land_use);
        // An EMPTY raster: the off path must never read the natural field at all.
        let terrain = crate::baked_terrain::BakedTerrain::new(&scene).expect("terrain");
        let off = crate::worldgen::plan_landuse_zones_for_style(
            &scene,
            &land_use,
            &serde_json::json!({"landUseProfile": {"urbanCoreRatio": 0.02}}),
            &terrain,
            water,
            None,
            crate::worldgen::NaturalSource::Undeclared,
        )
        .expect("the off path plans without touching the natural field");
        assert_eq!(off.source, crate::worldgen::RADIAL_ZONE_SOURCE);
        assert_eq!(
            serde_json::to_vec(&off.zones).expect("bytes"),
            serde_json::to_vec(&radial).expect("bytes")
        );
        assert_eq!(hash(&off.zones), hash(&radial));
        // The policy module itself cannot move an input that did not opt in either: with no network
        // it republishes the radial geometry under a name that says so.
        let grown = plan(&scene, &land_use, None, water);
        assert_eq!(grown.source, SOURCE_NO_NETWORK);
        assert_eq!(
            grown.zones.iter().map(zone_area).collect::<Vec<_>>(),
            radial.iter().map(zone_area).collect::<Vec<_>>()
        );
    }

    #[test]
    fn grown_urban_area_equals_the_radial_budget_exactly() {
        for (width, depth, urban) in [
            (512_u32, 512_u32, 0.25),
            (512, 512, 0.35),
            (1000, 1000, 0.04),
            (1000, 1000, 0.3),
            (2000, 2000, 0.22),
        ] {
            let scene = scene_at(width, depth);
            let land_use = profile(urban, 0.6, 0.15);
            let water = waterless(&scene);
            let radial = crate::worldgen::plan_landuse_zones(&scene, &land_use);
            let grown = plan(&scene, &land_use, Some(&cross_network(&scene)), water);
            let (radial_urban, radial_rural, radial_nature) = totals(&radial);
            let (grown_urban, grown_rural, grown_nature) = totals(&grown.zones);
            assert!(
                radial_urban > 0,
                "{width}: the radial plan must plan urban land"
            );
            assert_eq!(
                grown_urban, radial_urban,
                "{width}x{depth} at {urban}: the grown plan must spend the radial budget to the \
                 square metre"
            );
            assert_eq!(
                grown_nature, radial_nature,
                "{width}: the nature total must not move"
            );
            assert_eq!(
                grown_rural, radial_rural,
                "{width}: the rural total must not move"
            );
            assert_eq!(
                grown.zones.iter().map(zone_area).sum::<i64>(),
                i64::from(width) * i64::from(depth),
                "{width}: the plan must still cover the whole scene"
            );
        }
    }

    #[test]
    fn the_urban_land_sits_on_the_axes_and_inside_the_living_circle() {
        let scene = scene_at(1000, 1000);
        let land_use = profile(0.04, 0.86, 0.10);
        let water = waterless(&scene);
        let grown = plan(&scene, &land_use, Some(&cross_network(&scene)), water);
        let selection = grown.selection.as_ref().expect("a grown plan has geometry");
        assert_eq!(selection.radius_m, NODE_RADIUS_M);
        assert_eq!(selection.axis_width_m, AXIS_WIDTH_M);
        let corridors = selection.corridors();
        let mut urban_area = 0_i64;
        let mut on_axis = 0_i64;
        let mut in_circle = 0_i64;
        let mut over_corridor = 0_i64;
        for zone in grown.zones.iter().filter(|zone| zone.kind == "urban") {
            let rect = rect_of(zone);
            let area = zone_area(zone);
            urban_area += area;
            if selection.distance_to_axis(&rect) <= i64::from(AXIS_WIDTH_M) / 2 {
                on_axis += area;
            }
            if selection.distance_to_node(&rect) <= i64::from(NODE_RADIUS_M) {
                in_circle += area;
            }
            if corridors
                .iter()
                .any(|corridor| intersect(&rect, corridor).is_some())
            {
                over_corridor += area;
            }
        }
        assert!(urban_area > 0);
        assert_eq!(
            in_circle, urban_area,
            "urban land outside every living circle"
        );
        assert_eq!(on_axis, urban_area, "urban land off every axis corridor");
        assert_eq!(
            over_corridor, urban_area,
            "urban land must stand ON the corridor, not near it"
        );
        // The corridor is a real width: the urban land cannot be wider across the axis than
        // AXIS_WIDTH_M plus the cell cut, and it is much longer along the axis than across it.
        let mut longest = 0_i64;
        let mut across = 0_i64;
        for zone in grown.zones.iter().filter(|zone| zone.kind == "urban") {
            let rect = rect_of(zone);
            longest = longest.max((rect[2] - rect[0]).max(rect[3] - rect[1]));
            across = across.max((rect[2] - rect[0]).min(rect[3] - rect[1]));
        }
        assert!(
            across <= i64::from(AXIS_WIDTH_M),
            "no urban piece may be deeper than the corridor; deepest {across} m"
        );
        assert!(
            longest > across,
            "the plan must be a band: longest {longest} m, deepest {across} m"
        );
    }

    #[test]
    fn the_grown_plan_tiles_the_scene_and_never_touches_reserved_land() {
        let scene = scene_at(1000, 1000);
        let land_use = profile(0.15, 0.70, 0.15);
        let water = waterless(&scene);
        let radial = crate::worldgen::plan_landuse_zones(&scene, &land_use);
        let grown = plan(&scene, &land_use, Some(&cross_network(&scene)), water);
        let width = scene.width_m as usize;
        let depth = scene.depth_m as usize;
        let mut grid = vec![0_u8; width * depth];
        let mut ids = std::collections::BTreeSet::new();
        const WHITE: u8 = u8::MAX;
        for zone in &grown.zones {
            assert!(
                zone.width_m > 0 && zone.depth_m > 0,
                "empty zone {}",
                zone.id
            );
            assert!(ids.insert(zone.id.clone()), "duplicate zone id {}", zone.id);
            assert!(zone.world_x >= scene.origin_x && zone.world_z >= scene.origin_z);
            assert!(zone.world_x + zone.width_m as i32 <= scene.origin_x + scene.width_m as i32);
            assert!(zone.world_z + zone.depth_m as i32 <= scene.origin_z + scene.depth_m as i32);
            for z in zone.world_z..zone.world_z + zone.depth_m as i32 {
                for x in zone.world_x..zone.world_x + zone.width_m as i32 {
                    let cell = &mut grid
                        [(z - scene.origin_z) as usize * width + (x - scene.origin_x) as usize];
                    assert_eq!(
                        *cell, 0,
                        "zones overlap at ({x},{z}); the later one is {}",
                        zone.id
                    );
                    *cell = if zone.kind == "urban" { 1 } else { 2 };
                }
            }
        }
        assert!(
            grid.iter().all(|value| *value != 0),
            "the plan leaves scene cells unassigned"
        );
        assert!(grid.iter().all(|value| *value != WHITE));
        // Reserved (nature) land is the radial plan's, cell for cell: the urban plan may not take a
        // single square metre of it.
        for zone in grown.zones.iter().filter(|zone| zone.kind == "urban") {
            let rect = rect_of(zone);
            for reserved in radial.iter().filter(|zone| zone.kind == "nature") {
                assert!(
                    intersect(&rect, &rect_of(reserved)).is_none(),
                    "urban zone {} encroaches on reserved land",
                    zone.id
                );
            }
        }
    }

    #[test]
    fn open_water_is_the_last_land_the_city_takes() {
        // A river down the middle of the scene, a main road across it, and a budget small enough
        // that the dry corridor reaches it: no urban square metre may stand over the water.
        let scene = scene_at(512, 512);
        let style = serde_json::json!({"water": {
            "waterType": "river",
            "levelPolicy": "horizontal-datum",
            "levelM": 0,
            "footprint": {"shape": "band", "centerXM": 256, "halfWidthM": 20, "axis": "world_z"},
        }});
        let water = water_geometry(WaterKind::River, &[], &scene, &style);
        let mid_x = (scene.width_m / 2) as i32;
        let mid_z = (scene.depth_m / 2) as i32;
        let network = Network {
            nodes: vec![
                Node {
                    at: [mid_x - 128, mid_z],
                    degree: 2,
                    bridgehead: false,
                },
                Node {
                    at: [mid_x, mid_z],
                    degree: 4,
                    bridgehead: true,
                },
                Node {
                    at: [mid_x + 128, mid_z],
                    degree: 2,
                    bridgehead: true,
                },
            ],
            axes: vec![Axis {
                from: [0, mid_z],
                to: [512, mid_z],
                width_m: 16,
            }],
        };
        let land_use = profile(0.05, 0.85, 0.10);
        let grown = plan(&scene, &land_use, Some(&network), water);
        let (urban, _, _) = totals(&grown.zones);
        assert_eq!(
            urban, 16_384,
            "the 512 m probe's 5 % budget is one 128 m cell"
        );
        for zone in grown.zones.iter().filter(|zone| zone.kind == "urban") {
            assert!(
                !water.covers(rect_of(zone)),
                "urban zone {} stands over open water",
                zone.id
            );
        }
    }

    #[test]
    fn the_same_input_plans_the_same_bytes_twice() {
        let scene = scene_at(1000, 1000);
        let land_use = profile(0.10, 0.80, 0.10);
        let water = waterless(&scene);
        let network = cross_network(&scene);
        let first = plan(&scene, &land_use, Some(&network), water);
        let second = plan(&scene, &land_use, Some(&network), water);
        assert_eq!(hash(&first.zones), hash(&second.zones));
        assert_eq!(first.source, second.source);
        assert_eq!(first.selection, second.selection);
    }

    #[test]
    fn a_missing_or_unusable_network_degrades_by_name_and_deterministically() {
        let scene = scene_at(512, 512);
        let land_use = profile(0.25, 0.65, 0.10);
        let water = waterless(&scene);
        let radial = crate::worldgen::plan_landuse_zones(&scene, &land_use);
        for network in [
            None,
            Some(Network::default()),
            Some(Network {
                nodes: vec![Node {
                    at: [256, 256],
                    degree: 4,
                    bridgehead: false,
                }],
                axes: Vec::new(),
            }),
        ] {
            let plan = plan(&scene, &land_use, network.as_ref(), water);
            assert_eq!(plan.source, SOURCE_NO_NETWORK);
            assert!(plan.selection.is_none());
            assert_eq!(
                plan.zones.iter().map(zone_area).collect::<Vec<_>>(),
                radial.iter().map(zone_area).collect::<Vec<_>>(),
                "the degraded plan must be the radial plan's geometry"
            );
            assert!(
                plan.zones
                    .iter()
                    .all(|zone| zone.source == SOURCE_NO_NETWORK)
            );
        }
    }

    #[test]
    fn a_budget_larger_than_the_grown_land_is_named_and_exact() {
        // One short corridor in a large scene: the corridor and the living circles cannot hold a
        // 40 % urban budget, so the radial remainder must place the rest - and the total must still
        // be the radial budget, to the square metre.
        let scene = scene_at(4000, 4000);
        let land_use = profile(0.40, 0.50, 0.10);
        let water = waterless(&scene);
        let network = Network {
            nodes: vec![
                Node {
                    at: [1000, 1000],
                    degree: 2,
                    bridgehead: false,
                },
                Node {
                    at: [1256, 1000],
                    degree: 2,
                    bridgehead: false,
                },
            ],
            axes: vec![Axis {
                from: [1000, 1000],
                to: [1256, 1000],
                width_m: 16,
            }],
        };
        let radial = crate::worldgen::plan_landuse_zones(&scene, &land_use);
        let grown = plan(&scene, &land_use, Some(&network), water);
        assert_eq!(grown.source, SOURCE_RADIAL_REMAINDER);
        let (urban, _, _) = totals(&grown.zones);
        let (budget, _, _) = totals(&radial);
        assert_eq!(urban, budget);
        let again = plan(&scene, &land_use, Some(&network), water);
        assert_eq!(hash(&grown.zones), hash(&again.zones));
    }

    #[test]
    fn runs_merge_only_while_they_stay_collinear_and_contiguous() {
        let segment =
            |world_x: i32, world_z: i32, length_m: u32, vertical: bool, width_m: u32| Segment {
                world_x,
                world_z,
                length_m,
                vertical,
                width_m,
            };
        let axes = merge_runs(
            [
                segment(64, 0, 64, false, 8),
                segment(128, 0, 64, false, 12),
                segment(256, 0, 64, false, 8),
                segment(64, 200, 64, true, 8),
            ]
            .into_iter(),
        );
        let lengths: Vec<i32> = axes.iter().map(Axis::length_m).collect();
        // The first two touch and merge; the third stands alone across a gap.
        assert_eq!(lengths, vec![128, 64, 64]);
        assert_eq!(axes[0].from, [32, 0]);
        assert_eq!(axes[0].to, [160, 0]);
        assert_eq!(axes[0].width_m, 12, "the widest profile of the run wins");
        assert!(axes[2].vertical());
    }
}
