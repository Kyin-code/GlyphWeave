//! GlyphWeave-original agricultural surface execution. Small rain-fed field
//! blocks and 4m farm-access lanes are sampled against final terrain. This is
//! bounded deterministic land preparation, not crop/irrigation simulation.
use super::farm_access;
use super::farm_program::{CONTOUR_ELONGATED, FieldTemplate};
use super::{
    development::{LandcoverPlan, LandcoverRole, Rect},
    landcover::{self, Material, SurfacePatch},
};
use crate::{
    baked_terrain::BakedTerrain,
    worldgen::{AccessSurface, EntityInstance, SceneSpec, WaterGeometry},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
const HEADLAND: i64 = 4;
const FIELD_GRADE: f64 = 0.12;
const TRACK_GRADE: f64 = 0.08;
const MAX_ACCESS_LENGTH: i64 = 160;
/// How many metres of NEW lane a shared-track join must save before it may
/// replace a road route that the bounded search already proved. Round 15
/// measured the older trigger ("only when the road search is impossible") as a
/// no-op on real terrain, because road-first planning reaches a road inside the
/// 160 m budget everywhere, so the trigger is now a cost comparison - and a
/// cost comparison needs a floor, or two routes of the same length would trade
/// places on a rounding difference and the published network would churn
/// without laying one metre less surface.
///
/// 40 m is derived from the constants this module already enforces rather than
/// picked freely: it is exactly a quarter of the unchanged 160 m access budget
/// (`MAX_ACCESS_LENGTH / 4`), ten times the 4 m lane width (`HEADLAND` is the
/// same 4 m module) so a join must avoid laying at least 160 m2 of new track,
/// and twenty steps of the farm-lane search's 2 m centre lattice. A saving
/// below that cannot pay for replacing a self-contained lane with a chain: the
/// field would spend another field's metres to save an arm no longer than the
/// headland it already has to keep clear.
const JOIN_MIN_SAVING_M: u64 = 40;
/// The two published reasons a lane may end on an already realized farm track.
/// `CHEAPER` is the round-16 cost rule: a road route exists and the join lays at
/// least `JOIN_MIN_SAVING_M` fewer metres of new lane. `NO_ROAD` is the round-11
/// rule: the bounded road search proved no route inside the 160 m budget, so the
/// join is the field's only access. The basis is published because an auditor
/// can only recompute a saving when the generator states that it beat something.
const JOIN_BASIS_CHEAPER: &str = "cheaper_than_road";
const JOIN_BASIS_NO_ROAD: &str = "no_road_route_within_budget";
/// Distance kept between a field and the border of its own source cell. This is
/// the authored 2 m margin; the contour strategy may reshape a field but never
/// trims this, which is why it is a shared constant instead of a literal.
const FIELD_MARGIN: i64 = 2;
/// Every side of every field, square or strip, stays on the 4 m lattice inside
/// this domain. These mirror `FieldTemplate::validate` on purpose: the audit
/// re-checks the published shape against its own copy of the domain rather than
/// trusting the generator's ladder.
const FIELD_MIN_SIDE: u32 = 16;
const FIELD_MAX_SIDE: u32 = 128;
/// Upper bound on long:short for a contour strip. A strip may not be unboundedly
/// long: past this ratio the shape is spanning land on the strength of its
/// length rather than following the terrain, and the source cell rarely offers
/// it anyway.
const FIELD_ASPECT_MAX: u32 = 3;
/// Bounded ladder size. Each extra short side multiplies the anchor scan of the
/// cell, and the measured gain on the 1 km fixture comes from the first three
/// steps (the authored width and the way down to the 16 m minimum), so the
/// ladder stops after eight offers instead of walking a 128 m template.
const MAX_STRIP_SHAPES: usize = 8;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContourAxis {
    /// Long side runs east-west, i.e. the contours are aligned with x.
    #[serde(rename = "x")]
    X,
    /// Long side runs north-south.
    #[serde(rename = "z")]
    Z,
}
impl ContourAxis {
    fn label(self) -> &'static str {
        match self {
            ContourAxis::X => "x",
            ContourAxis::Z => "z",
        }
    }
}
/// The published shape decision for one contour strip. Every number here is
/// also recomputable from `boundary`, which is exactly how the audit cross
/// checks a forged record: the long side, its axis and the cell's own contour
/// axis must all agree or the field fails by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContourFieldShape {
    pub long_axis: String,
    pub long_side_m: u32,
    pub short_side_m: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmField {
    pub id: String,
    pub source_cell_id: String,
    pub boundary: Rect,
    pub cultivated: Rect,
    pub road_id: String,
    pub access_path: Vec<[i64; 2]>,
    pub access_method: String,
    pub access_length_m: u64,
    pub access_search_expanded: usize,
    pub preparation: String,
    /// Present only when the lane ends on an already realized farm track
    /// instead of a road footprint. Absent (and therefore absent from the
    /// published bytes) for every world that never enabled shared tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_track: Option<SharedTrackJoin>,
    /// Present only when the opt-in contour strategy shaped this field. Absent
    /// for every requested-shape field and for every world that never enabled
    /// the strategy, so those bytes cannot move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour_shape: Option<ContourFieldShape>,
}
/// One lane that joins an already realized farm track. `road_distance_m` is the
/// length that track already spends between the join square and the nearest
/// real road, so `access_length_m + road_distance_m` is the whole unchanged
/// budget this field consumes: a chain is bounded in metres, never in hops.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedTrackJoin {
    /// Field whose realized lane is reused.
    pub field_id: String,
    /// The 1m farm-track square this lane ends on.
    pub cell: [i64; 2],
    /// Metres that square already spends to reach a real road.
    pub road_distance_m: u64,
    /// How many realized lanes stand between this field and that road. The
    /// metre budget above is what actually bounds the chain.
    pub chain_lanes: u32,
    /// Which trigger accepted this join: `JOIN_BASIS_CHEAPER` or
    /// `JOIN_BASIS_NO_ROAD`. Publishing it is what makes "this join is cheaper"
    /// a claim an audit can re-check instead of a private decision of the
    /// generator.
    pub basis: String,
    /// Metres of new lane the road-only rule would have laid for this field,
    /// i.e. the route this join replaced. `0` means there was no bounded road
    /// route at all (`JOIN_BASIS_NO_ROAD`). The audit re-derives the saving from
    /// this number and the published lane length, and refuses a join that does
    /// not reach the unchanged threshold - a chain may never be published as
    /// "cheaper" on the generator's word alone.
    pub road_path_length_m: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmRejection {
    pub source_cell_id: String,
    pub boundary: Rect,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgricultureRealization {
    pub model: String,
    pub fields: Vec<FarmField>,
    pub rejected_fields: Vec<FarmRejection>,
    /// Which field-shape rule produced these fields, so an audit can demand the
    /// matching evidence. Absent unless the opt-in strategy ran, which keeps
    /// the legacy realization byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_shapes: Option<String>,
}
struct Index<'a> {
    rows: BTreeMap<i64, Vec<(i64, i64, &'a super::development::LandcoverCell)>>,
}
impl<'a> Index<'a> {
    fn new(plan: &'a LandcoverPlan) -> Self {
        let mut rows = BTreeMap::<_, Vec<_>>::new();
        for c in &plan.cells {
            if !matches!(
                c.role,
                LandcoverRole::AgricultureCandidate | LandcoverRole::FutureDevelopment
            ) {
                continue;
            }
            for z in c.boundary.z..c.boundary.z + i64::from(c.boundary.depth_m) {
                rows.entry(z).or_default().push((
                    c.boundary.x,
                    c.boundary.x + i64::from(c.boundary.width_m),
                    c,
                ));
            }
        }
        for row in rows.values_mut() {
            row.sort_by_key(|p| p.0);
        }
        Self { rows }
    }
    fn at(&self, x: i64, z: i64) -> Option<&'a super::development::LandcoverCell> {
        let row = self.rows.get(&z)?;
        let i = row.partition_point(|p| p.0 <= x);
        if i == 0 {
            return None;
        }
        let p = &row[i - 1];
        (x < p.1).then_some(p.2)
    }
}
fn contains(r: Rect, x: i64, z: i64) -> bool {
    x >= r.x && x < r.x + i64::from(r.width_m) && z >= r.z && z < r.z + i64::from(r.depth_m)
}
fn cells(r: Rect) -> impl Iterator<Item = (i64, i64)> {
    (r.z..r.z + i64::from(r.depth_m))
        .flat_map(move |z| (r.x..r.x + i64::from(r.width_m)).map(move |x| (x, z)))
}
fn road_rect(e: &EntityInstance) -> Option<Rect> {
    if e.kind != "road" || !e.width_m.is_finite() || !e.depth_m.is_finite() || e.scale != 1. {
        return None;
    }
    let (w, d) = match e.rotation_y_deg {
        0. | 180. => (e.width_m, e.depth_m),
        90. | 270. => (e.depth_m, e.width_m),
        _ => return None,
    };
    if w < 4.
        || d < 4.
        || w.fract() != 0.
        || d.fract() != 0.
        || w as u32 % 2 != 0
        || d as u32 % 2 != 0
    {
        return None;
    }
    Some(Rect {
        x: i64::from(e.world_x) - w as i64 / 2,
        z: i64::from(e.world_z) - d as i64 / 2,
        width_m: w as u32,
        depth_m: d as u32,
    })
}
fn grade(t: &BakedTerrain, x: i64, z: i64) -> Option<f64> {
    let x = i32::try_from(x).ok()?;
    let z = i32::try_from(z).ok()?;
    let [a, b, c, d] = [
        t.height(x, z)?,
        t.height(x.checked_add(1)?, z)?,
        t.height(x, z.checked_add(1)?)?,
        t.height(x.checked_add(1)?, z.checked_add(1)?)?,
    ]
    .map(f64::from);
    Some(
        (b - a)
            .abs()
            .max((d - c).abs())
            .hypot((c - a).abs().max((d - b).abs())),
    )
}
/// The axis a cell's contours run along, i.e. perpendicular to its dominant
/// gradient. Height differences are summed as exact integers of storage units
/// over a bounded stride grid, so the answer cannot depend on floating-point
/// rounding, on cell size or on any hash order: the same cell on the same final
/// terrain always yields the same axis. That determinism is what lets the audit
/// recompute it instead of trusting the published label.
fn contour_axis(t: &BakedTerrain, r: Rect) -> ContourAxis {
    // At most 32x32 probes per cell: an orientation decision needs a trend, not
    // every sample, and the cell may be 143 m wide.
    const PROBES: i64 = 32;
    let step_x = (i64::from(r.width_m) / PROBES).max(1);
    let step_z = (i64::from(r.depth_m) / PROBES).max(1);
    let (mut sx, mut sz) = (0_i64, 0_i64);
    let mut z = r.z;
    while z + 1 < r.z + i64::from(r.depth_m) {
        let mut x = r.x;
        while x + 1 < r.x + i64::from(r.width_m) {
            let sample = |dx: i64, dz: i64| -> Option<i64> {
                let x = i32::try_from(x + dx).ok()?;
                let z = i32::try_from(z + dz).ok()?;
                Some(i64::from(t.height_millimetres(x, z)?))
            };
            if let (Some(here), Some(toward_x), Some(toward_z)) =
                (sample(0, 0), sample(1, 0), sample(0, 1))
            {
                sx += toward_x - here;
                sz += toward_z - here;
            }
            x += step_x;
        }
        z += step_z;
    }
    if sx == 0 && sz == 0 {
        // Flat and unreadable cells have no contour to follow: fall back to the
        // cell's own longer dimension, and an exactly square cell to z.
        return if r.width_m > r.depth_m {
            ContourAxis::X
        } else {
            ContourAxis::Z
        };
    }
    // Gradient (sx, sz); contours are perpendicular to it. A dominant z
    // gradient means the slope runs north-south, so the contours and the long
    // side run along x. A tie resolves to z so the choice is total.
    if sz.abs() > sx.abs() {
        ContourAxis::X
    } else {
        ContourAxis::Z
    }
}
/// Deterministic contour-strip ladder for one cell, in preference order.
///
/// The authored template supplies the requested short side (across the
/// contour). The ladder walks that side down to the 16 m minimum, and for each
/// short side the long side is the largest multiple of 4 that both respects
/// `FIELD_ASPECT_MAX` and fits inside the cell's unchanged 2 m margin along the
/// contour. When the cell cannot span an elongated strip the ladder still emits
/// the largest legal contour-aligned rectangle rather than dropping the cell, so
/// a source that is too small for the authored square is not automatically
/// unusable. Nothing here reads a hash or an iterator order.
fn contour_shapes(cell: Rect, template: FieldTemplate, axis: ContourAxis) -> Vec<(u32, u32)> {
    let (along, across) = match axis {
        ContourAxis::X => (cell.width_m, cell.depth_m),
        ContourAxis::Z => (cell.depth_m, cell.width_m),
    };
    let margin = 2 * u32::try_from(FIELD_MARGIN).unwrap_or(2);
    let across_fit = across.saturating_sub(margin);
    let along_fit = along.saturating_sub(margin) / 4 * 4;
    let mut shapes: Vec<(u32, u32)> = Vec::new();
    if across_fit < FIELD_MIN_SIDE || along_fit < FIELD_MIN_SIDE {
        return shapes;
    }
    let requested = template
        .width_m
        .min(template.depth_m)
        .min(FIELD_MAX_SIDE)
        .max(FIELD_MIN_SIDE)
        .min(across_fit);
    let mut short = requested - requested % 4;
    while short >= FIELD_MIN_SIDE && shapes.len() < MAX_STRIP_SHAPES {
        let long = (FIELD_ASPECT_MAX * short)
            .min(FIELD_MAX_SIDE)
            .min(along_fit);
        let long = long - long % 4;
        if long >= FIELD_MIN_SIDE {
            // The short side never exceeds what fits across the contour, and an
            // over-long short side collapses onto the length that does fit: the
            // shape is then a square, which is still a legal field in the domain.
            let short_side = short.min(long);
            let (width_m, depth_m) = match axis {
                ContourAxis::X => (long, short_side),
                ContourAxis::Z => (short_side, long),
            };
            if !shapes.contains(&(width_m, depth_m)) {
                shapes.push((width_m, depth_m));
            }
        }
        if short == FIELD_MIN_SIDE {
            break;
        }
        short = short.saturating_sub(4).max(FIELD_MIN_SIDE);
    }
    shapes
}
fn append_cell(patch: &mut SurfacePatch, t: &BakedTerrain, x: i64, z: i64) -> Result<(), String> {
    // Shared indexed metre vertices are intentionally deferred until the
    // chunked landcover format; full samples are never replaced by probes.
    let square = vec![
        [x as f64, z as f64],
        [x as f64 + 1., z as f64],
        [x as f64 + 1., z as f64 + 1.],
        [x as f64, z as f64 + 1.],
    ];
    landcover::append(patch, &square, |p| {
        landcover::terrain_height(t, p).map(|h| h + 0.03)
    })
}
fn patch(
    id: String,
    source: String,
    owner: String,
    zone: String,
    material: Material,
) -> SurfacePatch {
    SurfacePatch {
        id,
        source_cell_id: source,
        parcel_id: owner,
        zone_kind: zone,
        material,
        area_m2: 0.,
        vertices: vec![],
        triangles: vec![],
    }
}
fn path_cells(path: &[[i64; 2]]) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    for pair in path.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let r = if a[0] == b[0] {
            Rect {
                x: a[0] - 2,
                z: a[1].min(b[1]) - 2,
                width_m: 4,
                depth_m: (a[1] - b[1]).unsigned_abs() as u32 + 4,
            }
        } else {
            Rect {
                x: a[0].min(b[0]) - 2,
                z: a[1] - 2,
                width_m: (a[0] - b[0]).unsigned_abs() as u32 + 4,
                depth_m: 4,
            }
        };
        out.extend(cells(r));
    }
    out
}
/// Ownership of one realized 1m farm-track square. The road distance is
/// cumulative, so a chain of lanes can never hide metres behind a hop count,
/// and the owning field is unique because a square is only ever lain once.
#[derive(Debug, Clone)]
struct TrackClaim {
    field_id: String,
    road_id: String,
    road_distance_m: u64,
    chain_lanes: u32,
}
/// Metres from every square covered by `path` back to its far end. Taking the
/// minimum over the path vertices keeps the number exact on the centreline and
/// over-counts by at most the 2m corridor half-width beside it, so a lane is
/// never credited with a shorter chain than the one it physically travels.
fn lane_distances(path: &[[i64; 2]]) -> BTreeMap<(i64, i64), u64> {
    let mut remaining = vec![0_u64; path.len()];
    for i in (0..path.len().saturating_sub(1)).rev() {
        remaining[i] = remaining[i + 1]
            + path[i][0].abs_diff(path[i + 1][0])
            + path[i][1].abs_diff(path[i + 1][1]);
    }
    let mut out = BTreeMap::new();
    for cell in path_cells(path) {
        let distance = path
            .iter()
            .zip(&remaining)
            .map(|(v, r)| r + cell.0.abs_diff(v[0]) + cell.1.abs_diff(v[1]))
            .min()
            .unwrap_or(0);
        out.insert(cell, distance);
    }
    out
}
/// Cheapest whole path from these field edges onto any already realized farm
/// track: the straight line to a square plus the metres that square already
/// spends back to a real road. It is a lower bound on the real lane, so when it
/// overruns the budget the chain is over budget however it is laid.
fn nearest_shared_track(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
) -> Option<u64> {
    let mut nearest = None::<u64>;
    for (&(x, zz), claim) in claimed_tracks {
        for start in starts {
            let straight = start[0].abs_diff(x) + start[1].abs_diff(zz);
            if straight > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            let bound = straight + claim.road_distance_m;
            nearest = Some(nearest.map_or(bound, |n: u64| n.min(bound)));
        }
    }
    nearest
}
/// The realized squares a spur is worth aiming at: the cheapest whole paths
/// from these field edges. Aiming at a bounded offer set keeps the spur search
/// as cheap as the road search it complements, and a blocked first choice still
/// has neighbours to fall back on instead of failing the whole field.
const SHARED_TRACK_TERMINALS: usize = 32;
fn shared_track_terminals(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
) -> Vec<([i64; 2], u64)> {
    // The road search steps 2m at a time from field edges that all share one
    // parity, so a square on the other parity could never be a goal: it is not
    // offered, and the offer set stays honest about what the search can reach.
    let Some(parity) = starts.first() else {
        return vec![];
    };
    let mut bounds = BTreeMap::<[i64; 2], u64>::new();
    for (&(x, zz), claim) in claimed_tracks {
        if x.rem_euclid(2) != parity[0].rem_euclid(2)
            || zz.rem_euclid(2) != parity[1].rem_euclid(2)
        {
            continue;
        }
        for start in starts {
            let straight = start[0].abs_diff(x) + start[1].abs_diff(zz);
            if straight > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            let bound = straight + claim.road_distance_m;
            // Squares already over the budget cannot become a legal goal, so
            // they never enter the offer set.
            if bound > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            bounds
                .entry([x, zz])
                .and_modify(|best| *best = (*best).min(bound))
                .or_insert(bound);
        }
    }
    let mut ordered: Vec<([i64; 2], u64)> = bounds.into_iter().collect();
    ordered.sort_by_key(|&(cell, bound)| (bound, cell));
    ordered.truncate(SHARED_TRACK_TERMINALS);
    ordered
        .into_iter()
        .map(|(cell, _)| (cell, claimed_tracks[&(cell[0], cell[1])].road_distance_m))
        .collect()
}
/// Joins an already realized farm track instead of demanding a fresh lane all
/// the way to a road, which is what a real field road network does: one lane
/// serves several fields. The whole path back to the nearest real road stays on
/// the clock, so the chain is bounded in metres, never in hops.
///
/// `road_path_length_m` is the length of the road-only route this field also
/// has, or `None` when the bounded road search proved no such route; the join
/// record publishes it so the cost comparison is auditable rather than implied.
fn join_realized_track(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
    road_path_length_m: Option<u64>,
    max_expanded: usize,
    failures: &mut BTreeSet<&'static str>,
    check_cell: &mut impl FnMut(i64, i64) -> Result<(), &'static str>,
) -> Result<
    Option<(
        String,
        Vec<[i64; 2]>,
        BTreeSet<(i64, i64)>,
        SharedTrackJoin,
        usize,
    )>,
    String,
> {
    let terminals = shared_track_terminals(claimed_tracks, starts);
    let spur = farm_access::find_spur(
        starts,
        &terminals,
        MAX_ACCESS_LENGTH as u64,
        max_expanded,
        |[cx, cz]| {
            // A centre is legal only when its entire 4m x 4m support is legal,
            // exactly the rule the road search applies to its own centres.
            let mut legal = true;
            for zz in cz - 2..cz + 2 {
                for xx in cx - 2..cx + 2 {
                    if let Err(reason) = check_cell(xx, zz) {
                        failures.insert(reason);
                        legal = false;
                    }
                }
            }
            legal
        },
    );
    if spur.path.is_empty() {
        match nearest_shared_track(claimed_tracks, starts) {
            // No realized lane is even in range: keep the older evidence.
            None => {}
            // Near, but every whole path back to a road overruns the budget.
            Some(nearest) if nearest > MAX_ACCESS_LENGTH as u64 => {
                failures.insert("access_via_existing_track_exceeds_budget");
            }
            Some(_) => {
                failures.insert(spur.reason);
            }
        }
        return Ok(None);
    }
    let centre = *spur.path.last().expect("a spur path is nonempty");
    let join = spur
        .join
        .ok_or("shared farm track spur ended without a join")?;
    // Extend the centreline onto the square it joins, so the published lane
    // ends exactly there and its length is the whole path it travels.
    for bend in [[join[0], centre[1]], [centre[0], join[1]]] {
        let mut path = spur.path.clone();
        path.push(bend);
        path.push(join);
        path.dedup();
        let claim = &claimed_tracks[&(join[0], join[1])];
        if farm_access::length(&path) + claim.road_distance_m > MAX_ACCESS_LENGTH as u64 {
            continue;
        }
        let corridor = path_cells(&path);
        let mut failure = None;
        for &(xx, zz) in &corridor {
            // Recheck the swept corridor, not just the search centres.
            if let Err(reason) = check_cell(xx, zz) {
                failure = Some(reason);
                break;
            }
        }
        if let Some(reason) = failure {
            failures.insert(reason);
            continue;
        }
        return Ok(Some((
            claim.road_id.clone(),
            path,
            corridor,
            SharedTrackJoin {
                field_id: claim.field_id.clone(),
                cell: join,
                road_distance_m: claim.road_distance_m,
                chain_lanes: claim.chain_lanes + 1,
                // Provisional: the caller only publishes this record when the
                // comparison below accepts it, and the flags say which side of
                // the cost rule it came from.
                basis: if road_path_length_m.is_some() {
                    JOIN_BASIS_CHEAPER
                } else {
                    JOIN_BASIS_NO_ROAD
                }
                .into(),
                road_path_length_m: road_path_length_m.unwrap_or(0),
            },
            spur.expanded,
        )));
    }
    Ok(None)
}
/// One enumerated field placement. `shape` is `None` for the authored template
/// and `Some((width, depth), axis)` for a contour strip, which keeps the two
/// rules in the same queue without letting a strip borrow the square rule's
/// geometry.
#[derive(Clone, Copy)]
struct FieldCandidate<'a> {
    distance: i64,
    z: i64,
    x: i64,
    cell: &'a super::development::LandcoverCell,
    detour: bool,
    shape: Option<((u32, u32), ContourAxis)>,
}
/// Authored canals, rivers and lakes are authoritative feature footprints even
/// when the scene's water geometry is empty, so both the generator and the audit
/// classify them through this one predicate.
fn water_feature(e: &EntityInstance) -> bool {
    matches!(e.kind.as_str(), "canal" | "river" | "lake" | "water")
}
/// Parks, reserves and already authored farmland/pasture are reserved land use.
/// A field may never be prepared on them, whatever shape the strategy picks.
fn reserved_feature(e: &EntityInstance) -> bool {
    matches!(
        e.kind.as_str(),
        "park" | "green_space" | "nature_reserve" | "mountain_forest" | "farmland" | "pasture"
    )
}
/// Cheapest Manhattan distance from a field centre to any road footprint. It is
/// only ever used to order the candidate queues, so both the square and the
/// strip enumeration share this one definition of "nearer the road".
fn road_distance(roads: &[(&EntityInstance, Rect)], center: [i64; 2]) -> i64 {
    roads
        .iter()
        .map(|(_, r)| {
            (center[0].clamp(r.x, r.x + i64::from(r.width_m)) - center[0]).abs()
                + (center[1].clamp(r.z, r.z + i64::from(r.depth_m)) - center[1]).abs()
        })
        .min()
        .unwrap_or(i64::MAX)
}
/// All roads supplied here are final actual entities. Callers additionally
/// enforce the RoadNetworkPlan/final-raster contract before release.
pub fn realize(
    scene: &SceneSpec,
    plan: &LandcoverPlan,
    t: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    access: &[AccessSurface],
) -> Result<(AgricultureRealization, Vec<SurfacePatch>), String> {
    let pedestrian = super::pedestrian_index::PedestrianIndex::new(access)?;
    realize_with_query(
        scene,
        plan,
        t,
        water,
        entities,
        JOIN_MIN_SAVING_M,
        |x, z| pedestrian.overlaps_cell(x, z),
    )
}
/// `min_saving_m` is the cost rule's threshold. Production always passes
/// `JOIN_MIN_SAVING_M`; it is a parameter only so the tests can prove what a
/// zero (or, under saturation, "negative") threshold is allowed to change and
/// what it still may not: no geometry, grade, ownership or budget contract
/// depends on it.
fn realize_with_query(
    scene:&SceneSpec,plan:&LandcoverPlan,t:&BakedTerrain,water:WaterGeometry,
    entities:&[EntityInstance],min_saving_m:u64,
    mut pedestrian_contains:impl FnMut(i64,i64)->bool,
) -> Result<(AgricultureRealization, Vec<SurfacePatch>), String> {
    if !t.matches_scene(scene) {
        return Err("agriculture terrain scene mismatch".into());
    }
    t.require_complete().map_err(|e| e.to_string())?;
    if plan.scene_id != scene.scene_id
        || plan.scene_area_m2 != u64::from(scene.width_m) * u64::from(scene.depth_m)
    {
        return Err("agriculture plan scene mismatch".into());
    }
    if let Some(pattern) = &plan.farm_pattern {
        pattern.validate()?;
    }
    let mut source_ids = BTreeSet::new();
    for c in &plan.cells {
        let r = c.boundary;
        if !source_ids.insert(&c.id)
            || r.width_m == 0
            || r.depth_m == 0
            || r.width_m > scene.width_m
            || r.depth_m > scene.depth_m
            || r.x < i64::from(scene.origin_x)
            || r.z < i64::from(scene.origin_z)
            || r.x > i64::from(scene.origin_x) + i64::from(scene.width_m) - i64::from(r.width_m)
            || r.z > i64::from(scene.origin_z) + i64::from(scene.depth_m) - i64::from(r.depth_m)
        {
            return Err("invalid agriculture source cell bounds/id".into());
        }
    }
    let index = Index::new(plan);
    for row in index.rows.values() {
        if row.windows(2).any(|p| p[0].1 > p[1].0) {
            return Err("overlapping agriculture candidate sources".into());
        }
    }
    let mut roads: Vec<_> = entities
        .iter()
        .filter_map(|e| road_rect(e).map(|r| (e, r)))
        .collect();
    roads.sort_by(|(a, _), (b, _)| a.entity_id.cmp(&b.entity_id));
    if roads
        .windows(2)
        .any(|p| p[0].0.entity_id == p[1].0.entity_id)
    {
        return Err("ambiguous agricultural road identity".into());
    }
    let solids: Vec<_> = entities.iter().filter(|e| landcover::solid(e)).collect();
    // Structured natural water is not the only reserved feature: authored
    // canals, parks and existing fields remain authoritative entity footprints.
    let water_features: Vec<_> = entities.iter().filter(|e| water_feature(e)).collect();
    let reserved_features: Vec<_> = entities.iter().filter(|e| reserved_feature(e)).collect();
    // Cache whole-cell feasibility, reused by competing routes and fields.
    let mut facts = BTreeMap::<(i64, i64), Result<f64, &'static str>>::new();
    let mut inspect = |x: i64, z: i64| -> Result<f64, &'static str> {
        *facts.entry((x, z)).or_insert_with(|| {
            if water.intersects_axis_aligned(x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5) {
                return Err("water_intersection");
            }
            // The shoreline public passage band is reserved public ground with
            // its own realized surface (opt-in waterfront only), so a field may
            // never claim it: two surfaces on one square metre would be credited
            // twice in the ledger.
            if water.shore_band_cell(x as i32, z as i32) {
                return Err("shore_public_band_reserved");
            }
            if water_features
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("existing_water_feature_intersection");
            }
            if reserved_features
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("existing_land_use_reserved");
            }
            if solids
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("solid_object_intersection");
            }
            if pedestrian_contains(x,z)
            {
                return Err("pedestrian_access_reserved");
            }
            grade(t, x, z).ok_or("missing_final_terrain_sample")
        })
    };
    let track_network = plan
        .farm_pattern
        .as_ref()
        .and_then(|p| p.track_network.as_ref());
    let contour_enabled = plan
        .farm_pattern
        .as_ref()
        .is_some_and(|p| p.contour_elongated());
    let mut result = AgricultureRealization {
        // The model states which access rule produced this scene, so a world
        // that shares realized tracks cannot claim the road-only model.
        model: if track_network.is_some() {
            "explicit-rectangular-field-access-v4-shared-tracks"
        } else {
            "explicit-rectangular-field-access-v3"
        }
        .into(),
        // The shape rule is a separate axis from the access rule: a contour
        // world still reaches its fields along roads or shared tracks.
        field_shapes: contour_enabled.then(|| CONTOUR_ELONGATED.to_string()),
        ..Default::default()
    };
    let mut patches = vec![];
    let mut claimed_fields = BTreeSet::new();
    let mut claimed_tracks = BTreeMap::<(i64, i64), TrackClaim>::new();
    let mut candidates = vec![];
    // The opt-in strategy only ever ADDS this second queue: the first queue is
    // built exactly as before, so every field the square rule accepts keeps its
    // geometry and its access, and the published bytes of a world that never
    // enables the strategy cannot move.
    let mut strips = vec![];
    for cell in plan
        .cells
        .iter()
        .filter(|c| c.role == LandcoverRole::AgricultureCandidate)
    {
        let template = plan
            .farm_pattern
            .as_ref()
            .map(|p| p.for_zone(&cell.zone_id))
            .unwrap_or_default();
        template.validate()?;
        let width = i64::from(template.width_m);
        let depth = i64::from(template.depth_m);
        let axis = contour_enabled.then(|| contour_axis(t, cell.boundary));
        let ladder = match axis {
            Some(axis) => contour_shapes(cell.boundary, template, axis),
            None => vec![],
        };
        if plan.farm_pattern.is_some()
            && (i64::from(cell.boundary.width_m) < width + 4
                || i64::from(cell.boundary.depth_m) < depth + 4)
            && ladder.is_empty()
        {
            // With the strategy on this only fires when no legal strip fits the
            // cell and its unchanged 2 m margin either, so the named reason now
            // means "no legal field of any offered shape fits here".
            result.rejected_fields.push(FarmRejection {
                source_cell_id: cell.id.clone(),
                boundary: cell.boundary,
                reason: "source_too_small_for_requested_field_and_margin".into(),
            });
        }
        // Leave at least 2m around source boundaries, and 4m between blocks.
        for z in (cell.boundary.z + FIELD_MARGIN
            ..cell.boundary.z + i64::from(cell.boundary.depth_m) - depth - 1)
            .step_by((depth + 4) as usize)
        {
            for x in (cell.boundary.x + FIELD_MARGIN
                ..cell.boundary.x + i64::from(cell.boundary.width_m) - width - 1)
                .step_by((width + 4) as usize)
            {
                let center = [x + width / 2, z + depth / 2];
                let distance = road_distance(&roads, center);
                candidates.push((distance, z, x, cell));
            }
        }
        // The ladder is only built for a cell whose contour axis is known, so a
        // strip candidate always carries the axis its long side follows.
        if let Some(axis) = axis {
            for (rank, (strip_w, strip_d)) in ladder.iter().copied().enumerate() {
                for z in (cell.boundary.z + FIELD_MARGIN
                    ..cell.boundary.z + i64::from(cell.boundary.depth_m)
                        - i64::from(strip_d)
                        - 1)
                    .step_by((i64::from(strip_d) + 4) as usize)
                {
                    for x in (cell.boundary.x + FIELD_MARGIN
                        ..cell.boundary.x + i64::from(cell.boundary.width_m)
                            - i64::from(strip_w)
                            - 1)
                        .step_by((i64::from(strip_w) + 4) as usize)
                    {
                        let center = [x + i64::from(strip_w) / 2, z + i64::from(strip_d) / 2];
                        let distance = road_distance(&roads, center);
                        strips.push((rank, distance, z, x, cell, ((strip_w, strip_d), axis)));
                    }
                }
            }
        }
    }
    candidates.sort_by_key(|(distance, z, x, _)| (*distance, *z, *x));
    // Widest contour strip first, then nearer the road, then north-west: a total
    // and stable order that never depends on how the cells were collected.
    strips.sort_by_key(|(rank, distance, z, x, _, _)| (*rank, *distance, *z, *x));
    let targets: Vec<_> = roads
        .iter()
        .map(|(e, r)| farm_access::Target {
            road_id: e.entity_id.clone(),
            boundary: *r,
        })
        .collect();
    // Finish the existing direct-access pass first. Detours may add fields but
    // cannot displace already accepted fields or their access/ownership. The
    // contour queue is drained only once the authored queue is empty (detours
    // included), which is what keeps every square field identical.
    let mut queue: VecDeque<FieldCandidate> = candidates
        .into_iter()
        .map(|(distance, z, x, cell)| FieldCandidate {
            distance,
            z,
            x,
            cell,
            detour: false,
            shape: None,
        })
        .collect();
    let mut strip_queue: VecDeque<FieldCandidate> = strips
        .into_iter()
        .map(|(_, distance, z, x, cell, shape)| FieldCandidate {
            distance,
            z,
            x,
            cell,
            detour: false,
            shape: Some(shape),
        })
        .collect();
    loop {
        let (from_strips, candidate) = match queue.pop_front() {
            Some(candidate) => (false, candidate),
            None => match strip_queue.pop_front() {
                Some(candidate) => (true, candidate),
                None => break,
            },
        };
        let FieldCandidate {
            distance,
            z,
            x,
            cell,
            detour: detour_pass,
            shape,
        } = candidate;
        let template = plan
            .farm_pattern
            .as_ref()
            .map(|p| p.for_zone(&cell.zone_id))
            .unwrap_or_default();
        // A contour strip carries its own legal shape; everything else is the
        // authored template, exactly as before.
        let (shape_width, shape_depth) = shape.map_or(
            (template.width_m, template.depth_m),
            |((width_m, depth_m), _)| (width_m, depth_m),
        );
        let width = i64::from(shape_width);
        let depth = i64::from(shape_depth);
        let bounds = Rect {
            x,
            z,
            width_m: shape_width,
            depth_m: shape_depth,
        };
        let reject = |reason: &str| FarmRejection {
            source_cell_id: cell.id.clone(),
            boundary: bounds,
            reason: reason.into(),
        };
        if distance > MAX_ACCESS_LENGTH + width.max(depth) {
            result
                .rejected_fields
                .push(reject("no_road_within_direct_access_budget"));
            continue;
        }
        let mut failure = None;
        for (xx, zz) in cells(bounds) {
            if claimed_tracks.contains_key(&(xx, zz)) || claimed_fields.contains(&(xx, zz)) {
                failure = Some("reserved_field_or_track");
                break;
            }
            match inspect(xx, zz) {
                Err(reason) => {
                    failure = Some(reason);
                    break;
                }
                Ok(g) if g > FIELD_GRADE + 1e-9 => {
                    failure = Some("field_grade_exceeds_12_percent");
                    break;
                }
                _ => {}
            }
        }
        if let Some(reason) = failure {
            result.rejected_fields.push(reject(reason));
            continue;
        }
        let cultivated = Rect {
            x: x + HEADLAND,
            z: z + HEADLAND,
            width_m: shape_width - 2 * HEADLAND as u32,
            depth_m: shape_depth - 2 * HEADLAND as u32,
        };
        let starts = [
            [x + width / 2, z + 2],
            [x + width - 2, z + depth / 2],
            [x + width / 2, z + depth - 2],
            [x + 2, z + depth / 2],
        ];
        let mut routes = vec![];
        for (e, r) in roads.iter().filter(|_| !detour_pass) {
            for start in starts {
                let end = [
                    start[0].clamp(r.x + 2, r.x + i64::from(r.width_m) - 2),
                    start[1].clamp(r.z + 2, r.z + i64::from(r.depth_m) - 2),
                ];
                let length = (start[0] - end[0]).abs() + (start[1] - end[1]).abs();
                if length == 0 || length > MAX_ACCESS_LENGTH {
                    continue;
                }
                for bend in [[end[0], start[1]], [start[0], end[1]]] {
                    let mut path = vec![start, bend, end];
                    path.dedup();
                    routes.push((length, e.entity_id.clone(), path));
                }
            }
        }
        routes.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut accepted: Option<(
            String,
            Vec<[i64; 2]>,
            BTreeSet<(i64, i64)>,
            Option<SharedTrackJoin>,
        )> = None;
        let mut route_failures = BTreeSet::new();
        let mut access_search_expanded = 0;
        let mut check_cell = |xx: i64, zz: i64| -> Result<(), &'static str> {
            if roads.iter().any(|(_, r)| contains(*r, xx, zz)) {
                return Ok(());
            }
            if !contains(bounds, xx, zz) && index.at(xx, zz).is_none() {
                return Err("route_crosses_reserved_or_protected_land");
            }
            if contains(cultivated, xx, zz) {
                return Err("access_crosses_cultivated_interior");
            }
            if claimed_fields.contains(&(xx, zz)) {
                return Err("route_crosses_existing_field");
            }
            match inspect(xx, zz) {
                Err(reason) => Err(reason),
                Ok(g) if g > TRACK_GRADE + 1e-9 => Err("farm_access_grade_exceeds_8_percent"),
                _ => Ok(()),
            }
        };
        for (_, road_id, path) in routes {
            let track = path_cells(&path);
            let mut failure = None;
            for &(xx, zz) in &track {
                if let Err(reason) = check_cell(xx, zz) {
                    failure = Some(reason);
                    break;
                }
            }
            if let Some(reason) = failure {
                route_failures.insert(reason);
                continue;
            }
            accepted = Some((road_id, path, track, None));
            break;
        }
        if accepted.is_none() && !detour_pass {
            // Re-enter the SAME queue: a contour strip's detour must not jump
            // ahead of the authored fields still waiting in the first queue.
            let retry = FieldCandidate {
                detour: true,
                ..candidate
            };
            if from_strips {
                strip_queue.push_back(retry);
            } else {
                queue.push_back(retry);
            }
            continue;
        }
        if detour_pass {
            // Distant roads cannot be reached within the unchanged metric
            // budget; prune them once, not in every A* heuristic evaluation.
            let nearby_targets: Vec<_> = targets
                .iter()
                .filter(|t| {
                    starts
                        .iter()
                        .any(|&p| t.distance(p) <= MAX_ACCESS_LENGTH as u64)
                })
                .cloned()
                .collect();
            let search = farm_access::find(
                &starts,
                &nearby_targets,
                MAX_ACCESS_LENGTH as u64,
                4096,
                |[cx, cz]| {
                    // A centre is legal only when its entire 4m x 4m support is legal.
                    for zz in cz - 2..cz + 2 {
                        for xx in cx - 2..cx + 2 {
                            if let Err(reason) = check_cell(xx, zz) {
                                route_failures.insert(reason);
                                return false;
                            }
                        }
                    }
                    true
                },
            );
            access_search_expanded = search.expanded;
            // The bounded road route, when the A* proves one. It stays the
            // default access: a shared track may only replace it on cost.
            let mut road_route = None;
            if let Some(road_id) = search.road_id {
                let track = path_cells(&search.path);
                // Recheck the swept corridor, not just the search centres.
                for &(xx, zz) in &track {
                    check_cell(xx, zz)
                        .map_err(|r| format!("detour swept footprint mismatch: {r}"))?;
                }
                road_route = Some((road_id, search.path, track));
            } else {
                route_failures.insert(search.reason);
            }
            if track_network.is_some() {
                // Round 11 gated this branch on the road route being proven
                // impossible, and Round 15 measured that gate as a no-op on
                // real terrain: road-first planning reaches a road inside the
                // 160 m budget almost everywhere, so the branch never ran and
                // three seeds produced zero shared-track joins. The trigger is
                // now the COST rule this policy was always meant to express,
                // and it is still opt-in: with the switch off this block does
                // not execute and the published bytes cannot move.
                //
                // Both conditions must hold, exactly as for every other lane:
                //  * `road_len - shared_len >= min_saving_m`, i.e. the join
                //    lays at least that many metres FEWER of new lane. The
                //    saving is measured on new lane, which is what joining an
                //    existing track actually buys; the field's whole way back
                //    to a real road is a separate, unchanged limit.
                //  * `shared_len + join.road_distance_m <= 160`, enforced
                //    inside `join_realized_track` for every chain, so a cheap
                //    spur can never buy its way past the access budget.
                let road_len = road_route
                    .as_ref()
                    .map(|(_, path, _)| farm_access::length(path));
                // A join can never lay fewer than zero metres, so a road route
                // already shorter than the threshold cannot be beaten and the
                // spur search is skipped. This is a strict prune: it never
                // changes which field wins, only how often the search runs.
                let join = if road_len.is_none_or(|len| len >= min_saving_m) {
                    join_realized_track(
                        &claimed_tracks,
                        &starts,
                        road_len,
                        4096,
                        &mut route_failures,
                        &mut check_cell,
                    )?
                } else {
                    None
                };
                let join_wins = match &join {
                    // No road route: the round-11 trigger, the join is the only
                    // access this field can have.
                    Some(_) if road_len.is_none() => true,
                    // Both exist: only a saving of at least the threshold may
                    // displace the lane the search already proved. The saving is
                    // the roadway this field does not build, which is exactly
                    // what the published join record calls
                    // `road_path_length_m - access_length_m`.
                    Some((_, shared_path, _, _, _)) => {
                        let saving = road_len
                            .unwrap_or(0)
                            .saturating_sub(farm_access::length(shared_path));
                        saving >= min_saving_m
                    }
                    None => false,
                };
                accepted = if join_wins {
                    join.map(|(road_id, path, corridor, join, expanded)| {
                        // The accepted lane's own search is the evidence the
                        // expansion budget bounds, as for every other lane.
                        access_search_expanded = expanded;
                        (road_id, path, corridor, Some(join))
                    })
                } else {
                    road_route.map(|(road_id, path, track)| (road_id, path, track, None))
                };
            } else {
                accepted = road_route.map(|(road_id, path, track)| (road_id, path, track, None));
            }
        }
        let Some((road_id, path, track, join)) = accepted else {
            let reasons = route_failures.into_iter().collect::<Vec<_>>().join("|");
            result
                .rejected_fields
                .push(reject(&format!("no_verified_access:{reasons}")));
            continue;
        };
        let id = format!("{}.field.{x}.{z}", cell.id);
        let mut soil = patch(
            format!("{id}.soil"),
            cell.id.clone(),
            id.clone(),
            cell.zone_kind.clone(),
            Material::CultivatedSoil,
        );
        let mut headland = patch(
            format!("{id}.headland"),
            cell.id.clone(),
            id.clone(),
            cell.zone_kind.clone(),
            Material::FieldHeadland,
        );
        for (xx, zz) in cells(bounds) {
            claimed_fields.insert((xx, zz));
            if contains(cultivated, xx, zz) {
                append_cell(&mut soil, t, xx, zz)?
            } else {
                append_cell(&mut headland, t, xx, zz)?
            }
        }
        patches.extend([soil, headland]);
        let mut track_patches = BTreeMap::<String, SurfacePatch>::new();
        // A lane that joins an existing track inherits everything that track
        // already spent, so the square it lays must carry the whole sum.
        let spent_m = join.as_ref().map_or(0, |j| j.road_distance_m);
        // Squares laid by this lane are one shared join further from the road
        // than the join itself, so the next lane's chain grows by exactly one.
        let chain_lanes = join.as_ref().map_or(0, |j| j.chain_lanes);
        let lane = lane_distances(&path);
        for (xx, zz) in track {
            if contains(bounds, xx, zz)
                || roads.iter().any(|(_, r)| contains(*r, xx, zz))
                || claimed_tracks.contains_key(&(xx, zz))
            {
                continue;
            }
            let src = index
                .at(xx, zz)
                .ok_or("accepted agricultural route lost ownership")?;
            let p = track_patches.entry(src.id.clone()).or_insert_with(|| {
                patch(
                    format!("{id}.track.{}", src.id),
                    src.id.clone(),
                    id.clone(),
                    src.zone_kind.clone(),
                    Material::FarmTrack,
                )
            });
            append_cell(p, t, xx, zz)?;
            claimed_tracks.insert(
                (xx, zz),
                TrackClaim {
                    field_id: id.clone(),
                    road_id: road_id.clone(),
                    road_distance_m: spent_m
                        + lane
                            .get(&(xx, zz))
                            .copied()
                            .ok_or("accepted agricultural lane lost its own distance")?,
                    chain_lanes,
                },
            );
        }
        patches.extend(track_patches.into_values());
        result.fields.push(FarmField {
            id,
            source_cell_id: cell.id.clone(),
            boundary: bounds,
            cultivated,
            road_id,
            access_length_m: farm_access::length(&path),
            access_path: path,
            access_method: if join.is_some() {
                "shared_farm_track_spur"
            } else if detour_pass {
                "bounded_grid_detour"
            } else {
                "direct_axis_route"
            }
            .into(),
            access_search_expanded,
            preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
            shared_track: join,
            // Published only for fields the contour strategy shaped, so an
            // auditor can recompute the axis and the ratio instead of inferring
            // them from a rectangle that also fits a square rule. A square strip
            // has no unique long side, so it publishes the cell's own contour
            // axis.
            contour_shape: shape.map(|((width_m, depth_m), axis)| ContourFieldShape {
                long_axis: axis.label().into(),
                long_side_m: width_m.max(depth_m),
                short_side_m: width_m.min(depth_m),
            }),
        });
    }
    Ok((result, patches))
}

/// Ownership of one realized 1m farm-track square, rebuilt from the published
/// lanes alone. `road_distance_m` is the whole distance that square spends back
/// to a real road, so an auditor can re-check a chain in metres without
/// trusting the generator's own bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOwnership {
    pub field_id: String,
    pub road_id: String,
    pub road_distance_m: u64,
    pub chain_lanes: u32,
}

/// Rebuilds the realized farm-track ownership from the published lanes alone
/// and re-checks every access budget, join and chain. Nothing here trusts a
/// recorded number: the claim map, the metres each square already spends and
/// the chain length are recomputed from the plan's own paths, so a lane that
/// quietly counted one hop instead of the whole way to a road fails by name.
pub fn verify_access_network(
    realization: &AgricultureRealization,
    entities: &[EntityInstance],
) -> Result<BTreeMap<(i64, i64), TrackOwnership>, String> {
    let mut roads: Vec<_> = entities.iter().filter_map(road_rect).collect();
    roads.sort_by_key(|r| (r.x, r.z, r.width_m, r.depth_m));
    let mut claims = BTreeMap::<(i64, i64), TrackClaim>::new();
    let position: BTreeMap<&str, usize> = realization
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| (field.id.as_str(), index))
        .collect();
    if position.len() != realization.fields.len() {
        return Err("agricultural fields do not have unique ids".into());
    }
    for field in &realization.fields {
        if field.access_path.is_empty() {
            return Err("agricultural lane has no access path".into());
        }
        if field.access_length_m != farm_access::length(&field.access_path) {
            return Err("agricultural lane length differs from its own path".into());
        }
        let lane = lane_distances(&field.access_path);
        let spent_m = field
            .shared_track
            .as_ref()
            .map_or(0, |join| join.road_distance_m);
        // The whole path back to a real road is on the clock. One hop is not a
        // unit of this contract, so a chain cannot tunnel past the budget.
        if field.access_length_m + spent_m > MAX_ACCESS_LENGTH as u64 {
            return Err("agricultural access exceeds the unchanged 160m budget".into());
        }
        let end = *field
            .access_path
            .last()
            .expect("the access path is nonempty");
        match &field.shared_track {
            None => {
                if !matches!(
                    field.access_method.as_str(),
                    "direct_axis_route" | "bounded_grid_detour"
                ) {
                    return Err("agricultural access method differs from its evidence".into());
                }
                if !roads.iter().any(|r| contains(*r, end[0], end[1])) {
                    return Err("agricultural lane does not reach a road footprint".into());
                }
            }
            Some(join) => {
                if field.access_method != "shared_farm_track_spur" {
                    return Err("shared farm track lane has the wrong access method".into());
                }
                if join.cell != end {
                    return Err("shared farm track lane does not end on its join".into());
                }
                if roads.iter().any(|r| contains(*r, end[0], end[1])) {
                    return Err("shared farm track join sits in a road footprint".into());
                }
                let owner = claims
                    .get(&(end[0], end[1]))
                    .ok_or("shared farm track lane joins a track that was never laid")?;
                if owner.field_id != join.field_id
                    || owner.road_distance_m != join.road_distance_m
                    || owner.road_id != field.road_id
                    || owner.chain_lanes + 1 != join.chain_lanes
                {
                    return Err(
                        "shared farm track join differs from the realized network".into(),
                    );
                }
                match position.get(owner.field_id.as_str()) {
                    Some(&owner_index) if owner_index < position[field.id.as_str()] => {}
                    _ => {
                        return Err(
                            "shared farm track lane joins a track that is not older".into(),
                        )
                    }
                }
                // The round-16 trigger is a cost comparison, so the audit
                // re-derives it from the published record instead of trusting
                // the generator's decision. `road_path_length_m` is the length
                // of the road-only lane this join replaced; the road search is
                // itself bounded by the unchanged 160 m, so a longer claim is
                // impossible, and the saving must reach the unchanged
                // threshold. A join that only exists because no road route was
                // found publishes 0 and is the round-11 trigger, which this
                // branch can only record - the road it lacked is, by
                // construction, not in the published network.
                match join.basis.as_str() {
                    JOIN_BASIS_NO_ROAD => {
                        if join.road_path_length_m != 0 {
                            return Err(
                                "shared farm track join without a road route claims a road lane"
                                    .into(),
                            );
                        }
                    }
                    JOIN_BASIS_CHEAPER => {
                        if join.road_path_length_m > MAX_ACCESS_LENGTH as u64 {
                            return Err(
                                "shared farm track road alternative exceeds the unchanged 160m budget"
                                    .into(),
                            );
                        }
                        if join.road_path_length_m
                            < field.access_length_m + JOIN_MIN_SAVING_M
                        {
                            return Err(
                                "shared farm track join does not save the required lane metres"
                                    .into(),
                            );
                        }
                    }
                    _ => {
                        return Err("shared farm track join has an unknown cost basis".into());
                    }
                }
            }
        }
        for (cell, own_m) in &lane {
            if contains(field.boundary, cell.0, cell.1)
                || roads.iter().any(|r| contains(*r, cell.0, cell.1))
            {
                continue;
            }
            claims.entry(*cell).or_insert(TrackClaim {
                field_id: field.id.clone(),
                road_id: field.road_id.clone(),
                road_distance_m: own_m + spent_m,
                chain_lanes: field
                    .shared_track
                    .as_ref()
                    .map_or(0, |join| join.chain_lanes),
            });
        }
    }
    Ok(claims
        .into_iter()
        .map(|(cell, claim)| {
            (
                cell,
                TrackOwnership {
                    field_id: claim.field_id,
                    road_id: claim.road_id,
                    road_distance_m: claim.road_distance_m,
                    chain_lanes: claim.chain_lanes,
                },
            )
        })
        .collect())
}

/// Recomputes every field's shape and slope legality from the published
/// realization alone. Nothing here trusts a recorded number: the source cell,
/// the 2 m margin, the 4 m headland inset, the long/short sides, the contour
/// axis, the aspect ratio and the 12% grade are all re-derived from the final
/// terrain and the plan, which is what makes the opt-in shape strategy auditable
/// rather than merely generated. `landcover::verify` calls exactly this.
pub fn verify_field_shapes(
    realization: &AgricultureRealization,
    plan: &LandcoverPlan,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<(), String> {
    let contour_enabled = plan
        .farm_pattern
        .as_ref()
        .is_some_and(|p| p.contour_elongated());
    if realization.field_shapes.as_deref() != contour_enabled.then_some(CONTOUR_ELONGATED) {
        return Err("agricultural field-shape policy differs from the manifest".into());
    }
    let water_features: Vec<_> = entities.iter().filter(|e| water_feature(e)).collect();
    let reserved_features: Vec<_> = entities.iter().filter(|e| reserved_feature(e)).collect();
    let solids: Vec<_> = entities.iter().filter(|e| landcover::solid(e)).collect();
    for field in &realization.fields {
        let source = plan
            .cells
            .iter()
            .find(|cell| cell.id == field.source_cell_id)
            .ok_or("agricultural field has no planned source cell")?;
        if source.role != LandcoverRole::AgricultureCandidate {
            return Err("agricultural field is not on a candidate source cell".into());
        }
        let template = plan
            .farm_pattern
            .as_ref()
            .map(|p| p.for_zone(&source.zone_id))
            .unwrap_or_default();
        let boundary = field.boundary;
        if boundary.width_m == 0 || boundary.depth_m == 0 {
            return Err("agricultural field has an empty boundary".into());
        }
        // The 2 m margin is the contract the strategy may reshape around but
        // never trim.
        if boundary.x < source.boundary.x + FIELD_MARGIN
            || boundary.z < source.boundary.z + FIELD_MARGIN
            || boundary.x + i64::from(boundary.width_m)
                > source.boundary.x + i64::from(source.boundary.width_m) - FIELD_MARGIN
            || boundary.z + i64::from(boundary.depth_m)
                > source.boundary.z + i64::from(source.boundary.depth_m) - FIELD_MARGIN
        {
            return Err("agricultural field breaks the unchanged 2m source margin".into());
        }
        let (width_m, depth_m) = (boundary.width_m, boundary.depth_m);
        match &field.contour_shape {
            None => {
                if (width_m, depth_m) != (template.width_m, template.depth_m) {
                    return Err("agricultural field is not the authored field template".into());
                }
            }
            Some(shape) => {
                if !contour_enabled {
                    return Err("contour field shape published without the enabling policy".into());
                }
                let (long, short) = (width_m.max(depth_m), width_m.min(depth_m));
                if shape.long_side_m != long || shape.short_side_m != short {
                    return Err("contour field shape differs from its own boundary".into());
                }
                // A strip's long side must be the side the label names; a square
                // strip has no unique long side and is judged by the cell's axis
                // alone (below).
                if width_m != depth_m {
                    let axis = if width_m > depth_m {
                        ContourAxis::X
                    } else {
                        ContourAxis::Z
                    };
                    if shape.long_axis != axis.label() {
                        return Err("contour field long side is not on the published axis".into());
                    }
                }
                if !(FIELD_MIN_SIDE..=FIELD_MAX_SIDE).contains(&long)
                    || !(FIELD_MIN_SIDE..=FIELD_MAX_SIDE).contains(&short)
                    || long % 4 != 0
                    || short % 4 != 0
                {
                    return Err("contour field side leaves the unchanged 16..=128m domain".into());
                }
                if long > FIELD_ASPECT_MAX * short {
                    return Err("contour field aspect ratio exceeds its unchanged cap".into());
                }
                if short > template.width_m.min(template.depth_m).min(FIELD_MAX_SIDE) {
                    return Err(
                        "contour field is wider across the contour than the requested field".into(),
                    );
                }
                // Recompute the cell's contour axis from the final terrain and
                // require the strip to follow it.
                let cell_axis = contour_axis(terrain, source.boundary);
                if shape.long_axis != cell_axis.label() {
                    return Err("contour field does not follow its cell's contour axis".into());
                }
                // And the shape must be one the ladder may legally offer here.
                let ladder = contour_shapes(source.boundary, template, cell_axis);
                if !ladder.contains(&(width_m, depth_m)) {
                    return Err("contour field shape is not a legal strip for its source".into());
                }
            }
        }
        let cultivated = Rect {
            x: boundary.x + HEADLAND,
            z: boundary.z + HEADLAND,
            width_m: width_m - 2 * HEADLAND as u32,
            depth_m: depth_m - 2 * HEADLAND as u32,
        };
        if field.cultivated != cultivated {
            return Err("agricultural field headland is not the unchanged 4m inset".into());
        }
        // The slope and protection contract is re-sampled per metre from the
        // published rectangle, not read from the generator's own cache.
        for (x, z) in cells(boundary) {
            if water.intersects_axis_aligned(x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5) {
                return Err("agricultural field intersects water".into());
            }
            if water_features
                .iter()
                .chain(reserved_features.iter())
                .chain(solids.iter())
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("agricultural field intersects a reserved feature".into());
            }
            match grade(terrain, x, z) {
                None => return Err("agricultural field has no final terrain sample".into()),
                Some(g) if g > FIELD_GRADE + 1e-9 => {
                    return Err("agricultural field exceeds the unchanged 12 percent grade".into())
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::pedestrian_index::triangle_overlaps_cell;
    use super::super::development;
    use super::super::farm_program::{FarmPattern, FieldTemplate, TrackNetwork};
    use super::*;
    use crate::worldgen::{
        GroundingSpec, LandUseZone, WaterKind,
        road_network::{RoadEdge, RoadNetworkPlan},
        water_geometry,
    };
    fn fixture() -> (
        SceneSpec,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "farms".into(),
            width_m: 160,
            depth_m: 160,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m: 160,
            source: "fixture".into(),
        }];
        let roads = RoadNetworkPlan {
            road_cut_mm: None,
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: scene.scene_id.clone(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            pad_conversions: vec![],
            conversion_allocation: None,
        conversion_fringe: None,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: Vec::new(),
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
            nodes: vec![],
            edges: vec![RoadEdge {
                id: "road".into(),
                from: 0,
                to: 1,
                world_x: 80,
                world_z: 80,
                length_m: 64,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
                // B01-T2 mechanical sync: this fixture builds an unprofiled legacy base edge
                profile: None,
                earthwork_blend_m: None,
            }],
            roadside_sites: vec![],
        
            entry_corridor_policy: Default::default(),
};
        let development = development::plan(&scene, &zones, &roads).unwrap();
        let plan = development::plan_landcover(&scene, &development).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 160, 160, &0_i16.to_le_bytes().repeat(160 * 160))
            .unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        (scene, plan, terrain, water, roads.entities())
    }
    /// A 4m-wide road footprint plus a rock wall with one 4m gap. The road is
    /// narrow on purpose: its search inset is a single odd row, which the
    /// established 2m centre lattice can never stand on, so the road search
    /// cannot terminate there while a realized lane still can.
    fn barrier_fixture_at(depth_m: u32) -> (
        SceneSpec,
        development::DevelopmentPlan,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "farms".into(),
            width_m: 160,
            depth_m,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m,
            source: "fixture".into(),
        }];
        let roads = RoadNetworkPlan {
            road_cut_mm: None,
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: scene.scene_id.clone(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            pad_conversions: vec![],
            conversion_allocation: None,
        conversion_fringe: None,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: Vec::new(),
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
            nodes: vec![],
            edges: vec![RoadEdge {
                id: "road".into(),
                from: 0,
                to: 1,
                world_x: 80,
                world_z: 80,
                length_m: 64,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
                profile: None,
                earthwork_blend_m: None,
            }],
            roadside_sites: vec![],
        
            entry_corridor_policy: Default::default(),
};
        let development_plan = development::plan(&scene, &zones, &roads).unwrap();
        let mut plan = development::plan_landcover(&scene, &development_plan).unwrap();
        let template = plan.cells[0].clone();
        plan.cells = [Rect {
            x: 0,
            z: 0,
            width_m: 160,
            depth_m,
        }]
        .into_iter()
        .enumerate()
        .map(|(index, boundary)| {
            let mut cell = template.clone();
            cell.id = format!("barrier.cell.{index}");
            cell.boundary = boundary;
            cell.role = LandcoverRole::AgricultureCandidate;
            cell
        })
        .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        let chunk = crate::worldgen::STREAM_CHUNK_METERS;
        let mut z = 0_u32;
        while z < depth_m {
            let tall = (depth_m - z).min(chunk);
            terrain
                .insert_chunk(
                    0,
                    z as i32,
                    160,
                    tall,
                    &0_i16.to_le_bytes().repeat((160 * tall) as usize),
                )
                .unwrap();
            z += chunk;
        }
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let road = EntityInstance {
            entity_id: "road".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 80,
            world_z: 41,
            world_y: 0,
            width_m: 64.,
            depth_m: 4.,
            height_m: 0.24,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        };
        let mut entities = vec![road];
        for (index, (world_x, width_m)) in [(34, 68.), (118, 84.)].into_iter().enumerate() {
            entities.push(EntityInstance {
                entity_id: format!("wall.{index}"),
                asset_id: "prop.rock".into(),
                kind: "rock".into(),
                world_x,
                world_z: 44,
                world_y: 0,
                width_m,
                depth_m: 2.,
                height_m: 2.,
                scale: 1.,
                rotation_y_deg: 0.,
                grounding: GroundingSpec::default(),
                anchors: vec![],
                bounds: None,
            });
        }
        (scene, development_plan, plan, terrain, water, entities)
    }

    fn barrier_fixture() -> (
        SceneSpec,
        development::DevelopmentPlan,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        barrier_fixture_at(160)
    }

    fn with_track_network(plan: &LandcoverPlan, shared: bool) -> LandcoverPlan {
        use super::super::farm_program::{FarmPattern, TrackNetwork};
        let mut plan = plan.clone();
        let mut pattern = plan.farm_pattern.clone().unwrap_or(FarmPattern {
            default_field: Default::default(),
            zone_fields: BTreeMap::new(),
            track_network: None,
            field_shapes: None,
        });
        pattern.track_network = shared.then(TrackNetwork::shared);
        plan.farm_pattern = Some(pattern);
        plan
    }

        #[test]
    fn shared_track_terminals_realize_more_fields_on_the_same_input() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let plain = with_track_network(&plan, false);
        let shared = with_track_network(&plan, true);
        // Same terrain, same plan geometry, same final entities: only the
        // authored policy differs, which is exactly what the manifest sets.
        let (before, before_patches) =
            realize(&scene, &plain, &terrain, water, &entities, &[]).unwrap();
        let (after, after_patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(before.model, "explicit-rectangular-field-access-v3");
        assert_eq!(
            after.model,
            "explicit-rectangular-field-access-v4-shared-tracks"
        );
        // The measured delta on this input: 6 fields before, 15 after.
        assert_eq!(before.fields.len(), 6);
        assert_eq!(after.fields.len(), 15);
        let joined: Vec<_> = after
            .fields
            .iter()
            .filter(|field| field.shared_track.is_some())
            .collect();
        assert_eq!(joined.len(), 9);
        // Every field the road-only rule accepted survives with identical lane
        // geometry: shared tracks can only add fields, they never rewrite one.
        for field in &before.fields {
            let kept = after
                .fields
                .iter()
                .find(|other| other.id == field.id)
                .expect("a realized field must not be lost");
            assert_eq!(kept.access_path, field.access_path);
            assert_eq!(kept.access_length_m, field.access_length_m);
            assert_eq!(kept.access_method, field.access_method);
            assert_eq!(kept.shared_track, None);
        }
        let mut budgets = BTreeSet::new();
        let mut chains = BTreeSet::new();
        for field in &joined {
            assert_eq!(field.access_method, "shared_farm_track_spur");
            assert!(before.fields.iter().all(|other| other.id != field.id));
            let join = field.shared_track.as_ref().unwrap();
            assert_eq!(
                field.access_length_m,
                farm_access::length(&field.access_path)
            );
            // The published lane ends exactly on the square it joins.
            assert_eq!(*field.access_path.last().unwrap(), join.cell);
            // The whole path back to a real road is the budget, not one hop.
            assert!(field.access_length_m + join.road_distance_m <= 160);
            budgets.insert(field.access_length_m + join.road_distance_m);
            chains.insert(join.chain_lanes);
            // A joined square is reused, never dug again: it stays owned by the
            // earlier lane and out of this field's own track surface.
            assert!(after_patches.iter().all(|patch| {
                !(patch.parcel_id == field.id
                    && patch.vertices.chunks_exact(4).any(|square| {
                        square[0][0] as i64 == join.cell[0]
                            && square[0][2] as i64 == join.cell[1]
                    }))
            }));
        }
        // Chains of one and two shared lanes both occur, and every one of them
        // is bounded by metres of path to a road rather than by hops.
        assert_eq!(chains, BTreeSet::from([1, 2]));
        assert_eq!(
            budgets,
            BTreeSet::from([47, 75, 87, 103, 131])
        );
        // Determinism and unique area ownership survive the new rule.
        let (again, again_patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(
            serde_json::to_vec(&after).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&after_patches).unwrap(),
            serde_json::to_vec(&again_patches).unwrap()
        );
        let mut owned = BTreeSet::new();
        for patch in after_patches
            .iter()
            .filter(|patch| patch.material == Material::FarmTrack)
        {
            for square in patch.vertices.chunks_exact(4) {
                assert!(
                    owned.insert((square[0][0] as i64, square[0][2] as i64)),
                    "a farm-track square was laid twice"
                );
            }
        }
        // Both realizations satisfy the independent access-network audit, and
        // the audit's square count is exactly the published track surface.
        assert_eq!(
            verify_access_network(&before, &entities).unwrap().len(),
            before_patches
                .iter()
                .filter(|patch| patch.material == Material::FarmTrack)
                .map(|patch| patch.vertices.len() / 4)
                .sum::<usize>()
        );
        assert_eq!(
            verify_access_network(&after, &entities).unwrap().len(),
            owned.len()
        );
    }

    #[test]
    fn chain_over_the_unchanged_budget_is_rejected_by_name() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture_at(200);
        let plain = with_track_network(&plan, false);
        let shared = with_track_network(&plan, true);
        let (before, _) = realize(&scene, &plain, &terrain, water, &entities, &[]).unwrap();
        let (after, _) = realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(before.fields.len(), 6);
        assert_eq!(after.fields.len(), 18);
        let joined = after
            .fields
            .iter()
            .filter_map(|field| {
                field
                    .shared_track
                    .as_ref()
                    .map(|join| field.access_length_m + join.road_distance_m)
            })
            .max()
            .unwrap();
        // The rule accepts up to the unchanged 160m and not one metre more.
        assert_eq!(joined, 159);
        let token = "access_via_existing_track_exceeds_budget";
        assert!(
            before
                .rejected_fields
                .iter()
                .all(|rejection| !rejection.reason.contains(token)),
            "the road-only rule cannot name a shared-track budget"
        );
        let over: Vec<_> = after
            .rejected_fields
            .iter()
            .filter(|rejection| rejection.reason.contains(token))
            .collect();
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].boundary.z, 170);
        // Recompute that refusal from the published realization alone: the
        // cheapest realized square plus its whole spend back to a road is more
        // than the budget, which is why it is named and not left generic.
        let owners = verify_access_network(&after, &entities).unwrap();
        let b = over[0].boundary;
        let starts = [
            [b.x + 12, b.z + 2],
            [b.x + 22, b.z + 12],
            [b.x + 12, b.z + 22],
            [b.x + 2, b.z + 12],
        ];
        let cheapest = owners
            .iter()
            .map(|(&(x, z), owner)| {
                let straight = starts
                    .iter()
                    .map(|start| start[0].abs_diff(x) + start[1].abs_diff(z))
                    .min()
                    .unwrap();
                straight + owner.road_distance_m
            })
            .min()
            .unwrap();
        assert!(cheapest > 160);
        assert_eq!(cheapest, 167);
    }

    #[test]
    fn audit_rebuilds_the_shared_track_network_from_plan_and_terrain() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let shared = with_track_network(&plan, true);
        let (realization, patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        // `landcover::verify` runs exactly this function over the published
        // surfaces, so auditing it here audits the reconstruction rule itself.
        let owners = verify_access_network(&realization, &entities).unwrap();
        let published = patches
            .iter()
            .filter(|patch| patch.material == Material::FarmTrack)
            .map(|patch| patch.vertices.len() / 4)
            .sum::<usize>();
        assert_eq!(owners.len(), published);
        // Every shared lane's join is genuinely owned by an older lane, and the
        // recomputed metres agree with what the lane recorded.
        let mut shared_lanes = 0;
        for field in realization
            .fields
            .iter()
            .filter(|field| field.shared_track.is_some())
        {
            shared_lanes += 1;
            let join = field.shared_track.as_ref().unwrap();
            let owner = &owners[&(join.cell[0], join.cell[1])];
            assert_eq!(owner.field_id, join.field_id);
            assert_eq!(owner.road_distance_m, join.road_distance_m);
            assert_eq!(owner.chain_lanes + 1, join.chain_lanes);
            assert_eq!(owner.road_id, field.road_id);
        }
        assert_eq!(shared_lanes, 9);
        // The metres an audit attributes to a square are the whole distance back
        // to a road along its own lane, not one hop: the deepest square of a
        // chained lane still reports the full spend.
        let deepest = owners.values().map(|owner| owner.road_distance_m).max();
        assert_eq!(deepest, Some(129));
        let chains = owners
            .values()
            .map(|owner| owner.chain_lanes)
            .collect::<BTreeSet<_>>();
        assert_eq!(chains, BTreeSet::from([0, 1, 2]));
        // A falsified join cannot pass: the audit recomputes from the lanes.
        let mut shrunk = realization.clone();
        let tampered = shrunk
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .expect("the fixture realizes shared lanes");
        tampered.shared_track.as_mut().unwrap().road_distance_m += 1;
        assert_eq!(
            verify_access_network(&shrunk, &entities).unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        let mut lengthened = realization.clone();
        let tampered = lengthened
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .expect("the fixture realizes shared lanes");
        tampered.shared_track.as_mut().unwrap().chain_lanes = 9;
        assert_eq!(
            verify_access_network(&lengthened, &entities).unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        // A lane whose own path no longer reaches the budget fails by name too.
        // A lane whose recorded length no longer matches its own path fails.
        let mut over = realization.clone();
        let field = over
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .unwrap();
        field.access_length_m = 161;
        assert_eq!(
            verify_access_network(&over, &entities).unwrap_err(),
            "agricultural lane length differs from its own path"
        );
        // A chain that spends the whole path plus one metre is over budget even
        // though both lanes are individually short: the audit counts the whole
        // way back to a real road, never the last hop.
        let road = entities[0].entity_id.clone();
        let lane = |spur: i64| {
            let mut realization = AgricultureRealization {
                model: "explicit-rectangular-field-access-v4-shared-tracks".into(),
                ..Default::default()
            };
            // The older lane runs 139m from the join back to the road footprint.
            realization.fields.push(FarmField {
                id: "owner".into(),
                source_cell_id: "cell".into(),
                boundary: Rect {
                    x: 48,
                    z: 156,
                    width_m: 24,
                    depth_m: 24,
                },
                cultivated: Rect {
                    x: 52,
                    z: 160,
                    width_m: 16,
                    depth_m: 16,
                },
                road_id: road.clone(),
                access_path: vec![[60, 180], [60, 41]],
                access_method: "bounded_grid_detour".into(),
                access_length_m: 139,
                access_search_expanded: 0,
                preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
                shared_track: None,
                contour_shape: None,
            });
            realization.fields.push(FarmField {
                id: "joined".into(),
                source_cell_id: "cell".into(),
                boundary: Rect {
                    x: 48,
                    z: 156,
                    width_m: 24,
                    depth_m: 24,
                },
                cultivated: Rect {
                    x: 52,
                    z: 160,
                    width_m: 16,
                    depth_m: 16,
                },
                road_id: road.clone(),
                access_path: vec![[60, 180 + spur], [60, 180]],
                access_method: "shared_farm_track_spur".into(),
                access_length_m: spur as u64,
                access_search_expanded: 0,
                preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
                shared_track: Some(SharedTrackJoin {
                    field_id: "owner".into(),
                    cell: [60, 180],
                    road_distance_m: 139,
                    chain_lanes: 1,
                    // The cost rule's evidence. This lane replaces a 160 m road
                    // route, so it saves 160 - 21 = 139 m and is legal while the
                    // whole path back to the road stays inside the budget.
                    basis: JOIN_BASIS_CHEAPER.into(),
                    road_path_length_m: 160,
                }),
                contour_shape: None,
            });
            realization
        };
        assert!(
            verify_access_network(&lane(21), &entities).is_ok(),
            "a whole path of exactly 160m must be accepted"
        );
        assert_eq!(
            verify_access_network(&lane(22), &entities).unwrap_err(),
            "agricultural access exceeds the unchanged 160m budget"
        );
        // A lane that forgets its join is no longer allowed to end on a track.
        let mut orphan = realization.clone();
        let field = orphan
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .unwrap();
        field.shared_track = None;
        field.access_method = "bounded_grid_detour".into();
        assert_eq!(
            verify_access_network(&orphan, &entities).unwrap_err(),
            "agricultural lane does not reach a road footprint"
        );
    }
    #[test]
    fn legacy_agriculture_output_is_byte_identical_and_policy_defaults_off() {
        use super::super::farm_program::FarmPattern;
        let (scene, plan, terrain, water, entities) = fixture();
        // The fixture carries no agriculture program at all: the pre-existing
        // rule stays in force and the published bytes cannot move.
        assert!(plan.farm_pattern.is_none());
        let (realization, patches) =
            realize(&scene, &plan, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(realization.model, "explicit-rectangular-field-access-v3");
        assert!(realization
            .fields
            .iter()
            .all(|field| field.shared_track.is_none()));
        let bytes = serde_json::to_vec(&(realization.clone(), patches.clone())).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("sharedTrack"),
            "the legacy surface must not grow a shared-track field"
        );
        assert_eq!(
            blake3::hash(&bytes).to_hex().to_string(),
            LEGACY_AGRICULTURE_HASH
        );
        // An old agricultureProgram without the policy still resolves to the
        // road-only rule, so published manifests deserialize unchanged.
        let legacy: FarmPattern = serde_json::from_value(serde_json::json!({
            "defaultField": {"widthM": 48, "depthM": 32},
            "zoneFields": {}
        }))
        .unwrap();
        assert_eq!(legacy.track_network, None);
        legacy.validate().unwrap();
        let mut with_program = plan.clone();
        with_program.farm_pattern = Some(legacy);
        let (unchanged, _) =
            realize(&scene, &with_program, &terrain, water, &entities, &[]).unwrap();
        assert!(unchanged
            .fields
            .iter()
            .all(|field| field.shared_track.is_none()));
        assert_eq!(
            serde_json::to_value(&unchanged.model).unwrap(),
            serde_json::to_value("explicit-rectangular-field-access-v3").unwrap()
        );
    }

    /// Locked bytes of the road-only agriculture surface on the flat fixture.
    /// Any change to the legacy path breaks this constant on purpose.
    const LEGACY_AGRICULTURE_HASH: &str =
    "57268eb9f4126518ce6d15a479ff6ceb5d37f6a0bd8f8024d8fc5fe138727f82";

#[test]
    fn flat_fields_have_four_metre_access_and_disjoint_surface_ownership() {
        let (s, p, t, w, e) = fixture();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(!a.fields.is_empty());
        assert!(patches.iter().any(|p| p.material == Material::FarmTrack));
        let mut owned = BTreeSet::new();
        let mut area = 0.;
        for p in &patches {
            assert_eq!(p.zone_kind, "rural");
            for v in p.vertices.chunks_exact(4) {
                let x = v[0][0] as i64;
                let z = v[0][2] as i64;
                assert!(owned.insert((x, z)), "overlapping field/track cell");
                assert!(!contains(road_rect(&e[0]).unwrap(), x, z));
                assert!(v.iter().all(|v| (v[1] - 0.03).abs() < 1e-8));
            }
            area += p.area_m2;
        }
        assert_eq!(owned.len() as f64, area);
        for f in &a.fields {
            assert_eq!(f.road_id, "road");
            assert_eq!(f.boundary.area(), 576);
            assert_eq!(f.cultivated.area(), 256);
            assert_eq!(f.access_length_m, farm_access::length(&f.access_path));
            assert!(f.access_length_m <= MAX_ACCESS_LENGTH as u64);
            assert!(f.access_search_expanded <= 4096);
            assert!(
                f.access_path
                    .windows(2)
                    .all(|p| p[0][0] == p[1][0] || p[0][1] == p[1][1])
            );
            for (x, z) in path_cells(&f.access_path) {
                assert!(!contains(f.cultivated, x, z));
                assert!(owned.contains(&(x, z)) || contains(road_rect(&e[0]).unwrap(), x, z));
            }
        }
        let again = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert_eq!(a, again.0);
        assert_eq!(
            serde_json::to_value(patches).unwrap(),
            serde_json::to_value(again.1).unwrap()
        );
    }
    #[test]
    fn blocked_direct_routes_gain_real_four_metre_detours_but_not_through_narrow_gaps() {
        let (s, mut p, t, w, mut e) = fixture();
        let mut field = p.cells[0].clone();
        field.id = "field-source".into();
        field.role = LandcoverRole::AgricultureCandidate;
        field.boundary = Rect {
            x: 60,
            z: 110,
            width_m: 28,
            depth_m: 28,
        };
        let mut connector = field.clone();
        connector.id = "connector".into();
        connector.role = LandcoverRole::FutureDevelopment;
        connector.boundary = Rect {
            x: 0,
            z: 84,
            width_m: 160,
            depth_m: 26,
        };
        p.cells = vec![field, connector];
        let mut barrier = e[0].clone();
        barrier.entity_id = "barrier".into();
        barrier.kind = "rock".into();
        barrier.world_x = 74;
        barrier.world_z = 96;
        barrier.width_m = 32.;
        barrier.depth_m = 8.;
        e.push(barrier.clone());
        let before = t.content_hash().unwrap();
        // All original straight/L candidates cross this barrier.
        for start in [[74, 114], [84, 124], [74, 134], [64, 124]] {
            assert!(
                path_cells(&[start, [start[0], 82]])
                    .iter()
                    .any(|&(x, z)| landcover::obstacle_intersects(&barrier, x, z))
            );
        }
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert_eq!(a.fields.len(), 1);
        let f = &a.fields[0];
        assert_eq!(f.access_method, "bounded_grid_detour");
        assert!(f.access_search_expanded > 0 && f.access_search_expanded <= 4096);
        assert!(f.access_path.len() > 2);
        assert_eq!(f.access_length_m, farm_access::length(&f.access_path));
        assert!(f.access_length_m <= 160);
        let mut covered = BTreeSet::new();
        for patch in &patches {
            for vertices in patch.vertices.chunks_exact(4) {
                let x = vertices[0][0] as i64;
                let z = vertices[0][2] as i64;
                assert!(covered.insert((x, z)), "overlapping published surfaces");
                assert!(!landcover::obstacle_intersects(&barrier, x, z));
            }
        }
        assert!(patches.iter().any(|p| p.material == Material::FarmTrack));
        for (x, z) in path_cells(&f.access_path) {
            assert!(!contains(f.cultivated, x, z));
            assert!(covered.contains(&(x, z)) || contains(road_rect(&e[0]).unwrap(), x, z));
        }
        assert_eq!(before, t.content_hash().unwrap());
        assert_eq!(a, realize(&s, &p, &t, w, &e, &[]).unwrap().0);
        e[1].world_x = 80;
        e[1].width_m = 160.;
        let blocked = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(blocked.0.fields.is_empty());
        assert!(blocked.1.is_empty());
        // A 2m gap is not a legal 4m machinery corridor.
        e[1].world_x = 38;
        e[1].width_m = 76.;
        let mut right = e[1].clone();
        right.entity_id = "right-barrier".into();
        right.world_x = 119;
        right.width_m = 82.;
        e.push(right);
        let narrow = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(narrow.0.fields.is_empty());
        assert!(narrow.1.is_empty());
    }

    #[test]
    fn explicit_rectangles_have_exact_area_and_never_fallback_to_smaller_fields() {
        use super::super::farm_program::{FarmPattern, FieldTemplate};
        let (s, mut plan, t, w, e) = fixture();
        plan.farm_pattern = Some(FarmPattern {
            default_field: FieldTemplate {
                width_m: 48,
                depth_m: 32,
            },
            zone_fields: BTreeMap::new(),
            track_network: None,

            field_shapes: None,
        });
        let (result, patches) = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(!result.fields.is_empty());
        for field in &result.fields {
            assert_eq!((field.boundary.width_m, field.boundary.depth_m), (48, 32));
            assert_eq!(
                (field.cultivated.width_m, field.cultivated.depth_m),
                (40, 24)
            );
            assert_eq!(field.cultivated.area(), 960);
            assert_eq!(field.boundary.area() - field.cultivated.area(), 576);
            assert_eq!(
                field.access_length_m,
                farm_access::length(&field.access_path)
            );
            assert!(field.access_length_m <= 160);
        }
        assert_eq!(
            patches
                .iter()
                .filter(|p| p.material == Material::CultivatedSoil)
                .map(|p| p.area_m2)
                .sum::<f64>(),
            result.fields.len() as f64 * 960.
        );
        let before = t.content_hash().unwrap();
        plan.farm_pattern.as_mut().unwrap().default_field = FieldTemplate {
            width_m: 128,
            depth_m: 128,
        };
        let blocked = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(blocked.0.fields.is_empty());
        assert!(blocked.1.is_empty());
        assert!(
            blocked
                .0
                .rejected_fields
                .iter()
                .any(|r| r.reason == "source_too_small_for_requested_field_and_margin")
        );
        assert_eq!(before, t.content_hash().unwrap());
        plan.farm_pattern.as_mut().unwrap().default_field.width_m = 17;
        assert!(realize(&s, &plan, &t, w, &e, &[]).is_err());
    }

    #[test]
    fn zone_field_overrides_do_not_change_other_zones_or_ignore_requested_sizes() {
        use super::super::farm_program::{FarmPattern, FieldTemplate};
        let (s, mut plan, t, w, e) = fixture();
        let mut small_zone = None;
        for cell in &mut plan.cells {
            if cell.boundary.z >= 84 {
                cell.zone_id = "south".into();
                small_zone = Some(cell.zone_id.clone());
            }
        }
        plan.farm_pattern = Some(FarmPattern {
            default_field: FieldTemplate {
                width_m: 48,
                depth_m: 32,
            },
            zone_fields: BTreeMap::from([(
                small_zone.unwrap(),
                FieldTemplate {
                    width_m: 24,
                    depth_m: 24,
                },
            )]),
            track_network: None,

            field_shapes: None,
        });
        let (result, _) = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(result.fields.iter().any(|f| f.boundary.width_m == 48));
        assert!(result.fields.iter().any(|f| f.boundary.width_m == 24));
        for f in &result.fields {
            let source = plan
                .cells
                .iter()
                .find(|c| c.id == f.source_cell_id)
                .unwrap();
            let template = plan
                .farm_pattern
                .as_ref()
                .unwrap()
                .for_zone(&source.zone_id);
            assert_eq!(f.boundary.width_m, template.width_m);
            assert_eq!(f.boundary.depth_m, template.depth_m);
        }
    }

    #[test]
    fn absence_of_roads_does_not_become_a_fake_agricultural_access() {
        let (s, p, t, w, _) = fixture();
        let (a, patches) = realize(&s, &p, &t, w, &[], &[]).unwrap();
        assert!(a.fields.is_empty());
        assert!(patches.is_empty());
        assert!(!a.rejected_fields.is_empty());
    }
    #[test]
    fn steep_terrain_water_and_existing_solids_remain_unmodified() {
        let (s, p, _, w, e) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        let bytes: Vec<_> = (0..160)
            .flat_map(|_| (0..160).flat_map(|x| (x as i16).to_le_bytes()))
            .collect();
        t.insert_chunk(0, 0, 160, 160, &bytes).unwrap();
        let before = t.content_hash().unwrap();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "field_grade_exceeds_12_percent")
        );
        assert_eq!(before, t.content_hash().unwrap());
        let (s, p, t, _, e) = fixture();
        let lake:crate::worldgen::LandmarkSpec=serde_json::from_value(serde_json::json!({
            "entityId":"lake","name":"lake","type":"lake","purpose":"test","description":"water fixture","sceneId":"farms","worldX":80,"worldZ":80,"widthM":400,"depthM":400,"heightM":1,"assetId":"prop.lake"
        })).unwrap();
        let w = water_geometry(WaterKind::Lake, &[lake], &s, &serde_json::Value::Null);
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "water_intersection")
        );
        let (s, p, t, w, mut e) = fixture();
        e.push(EntityInstance {
            entity_id: "existing-rock".into(),
            asset_id: "prop.rock".into(),
            kind: "rock".into(),
            world_x: 80,
            world_z: 80,
            world_y: 0,
            width_m: 300.,
            depth_m: 300.,
            height_m: 4.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        });
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "solid_object_intersection")
        );
        assert_eq!(e.len(), 2);
    }
    #[test]
    fn protected_strip_cannot_be_crossed_to_reach_a_nearby_road() {
        let (s, mut p, t, w, e) = fixture();
        p.cells.retain(|c| c.boundary.z >= 84);
        for c in &mut p.cells {
            c.boundary.z += 10;
            c.boundary.depth_m -= 10;
        }
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason.contains("reserved_or_protected"))
        );
    }
    #[test]
    fn rejected_access_does_not_claim_flat_fields_behind_a_steep_barrier() {
        let (s, mut p, _, w, e) = fixture();
        p.cells.retain(|c| c.boundary.z >= 84);
        for c in &mut p.cells {
            c.boundary.z += 8;
            c.boundary.depth_m -= 8;
        }
        // A legal candidate connector strip, too steep for farm machinery.
        let mut strip = p.cells[0].clone();
        strip.id = "connector".into();
        strip.role = LandcoverRole::FutureDevelopment;
        strip.boundary = Rect {
            x: 0,
            z: 84,
            width_m: 160,
            depth_m: 8,
        };
        p.cells.push(strip);
        let mut bytes = 0_i16.to_le_bytes().repeat(160 * 160);
        for z in [86, 87] {
            for x in 0..160 {
                let i = (z * 160 + x) * 2;
                bytes[i..i + 2].copy_from_slice(&4_i16.to_le_bytes());
            }
        }
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(0, 0, 160, 160, &bytes).unwrap();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(a.fields.is_empty());
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason.contains("farm_access_grade_exceeds_8_percent"))
        );
    }
    #[test]
    fn existing_canal_park_and_authored_farm_are_not_overwritten() {
        let (s, p, t, w, e) = fixture();
        for kind in ["canal", "green_space", "farmland", "pasture"] {
            let mut entities = e.clone();
            let mut reserved = e[0].clone();
            reserved.entity_id = format!("authored.{kind}");
            reserved.kind = kind.into();
            reserved.width_m = 160.;
            reserved.depth_m = 160.;
            entities.push(reserved);
            let (a, patches) = realize(&s, &p, &t, w, &entities, &[]).unwrap();
            assert!(patches.is_empty());
            assert!(a.fields.is_empty());
            assert!(a.rejected_fields.iter().any(|r| r.reason
                == if kind == "canal" {
                    "existing_water_feature_intersection"
                } else {
                    "existing_land_use_reserved"
                }));
        }
    }

    #[test]
    fn duplicate_roads_overlapping_sources_and_invalid_bounds_fail() {
        let (s, p, t, w, mut e) = fixture();
        e.push(e[0].clone());
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
        let (s, mut p, t, w, e) = fixture();
        let mut duplicate = p.cells[0].clone();
        duplicate.id = "another-id".into();
        p.cells.push(duplicate);
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
        let (s, mut p, t, w, e) = fixture();
        p.cells[0].boundary.x = i64::MAX;
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
    }

    #[test]
    fn narrow_pedestrian_triangle_is_detected_between_cell_centres() {
        let t = [[0.9, 0., 0.], [1.1, 0., 0.], [0.9, 0., 1.]];
        assert!(triangle_overlaps_cell(t, 0, 0));
        assert!(triangle_overlaps_cell(t, 1, 0));
        assert!(!triangle_overlaps_cell(t, 2, 0));
    }
    /// Deterministic millimetre terrain, so a 10% and a 20% slope are exact and
    /// the grade gate is tested with numbers rather than with luck.
    fn terrain_from_metres(s: &SceneSpec, h: impl Fn(i64, i64) -> f64) -> BakedTerrain {
        let metres = (0..i64::from(s.depth_m))
            .flat_map(|z| (0..i64::from(s.width_m)).map(move |x| (x, z)))
            .map(|(x, z)| h(x, z));
        let bytes = crate::height_payload::encode_millimetres(s.width_m, s.depth_m, metres)
            .expect("fixture terrain must encode");
        let mut t = BakedTerrain::new(s).unwrap();
        t.insert_chunk(s.origin_x, s.origin_z, s.width_m, s.depth_m, &bytes)
            .unwrap();
        t
    }
    fn pattern(default_field: (u32, u32), shapes: Option<&str>, shared: bool) -> FarmPattern {
        FarmPattern {
            default_field: FieldTemplate {
                width_m: default_field.0,
                depth_m: default_field.1,
            },
            zone_fields: BTreeMap::new(),
            track_network: shared.then(TrackNetwork::shared),
            field_shapes: shapes.map(str::to_string),
        }
    }
    fn with_pattern(plan: &LandcoverPlan, pattern: FarmPattern) -> LandcoverPlan {
        let mut plan = plan.clone();
        plan.farm_pattern = Some(pattern);
        plan
    }

    #[test]
    fn the_contour_axis_is_the_cells_own_terrain_trend() {
        let s = SceneSpec {
            scene_id: "axis".into(),
            width_m: 64,
            depth_m: 64,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let square = Rect {
            x: 0,
            z: 0,
            width_m: 64,
            depth_m: 64,
        };
        // Rising eastwards: the gradient runs along x, so the contours (and the
        // long side) run along z.
        let east = terrain_from_metres(&s, |x, _| x as f64 * 0.1);
        assert_eq!(contour_axis(&east, square), ContourAxis::Z);
        // Rising northwards: contours run along x.
        let north = terrain_from_metres(&s, |_, z| z as f64 * 0.1);
        assert_eq!(contour_axis(&north, square), ContourAxis::X);
        // Flat: no contour to follow, so the cell's longer side decides and an
        // exactly square flat cell resolves to z.
        let flat = terrain_from_metres(&s, |_, _| 0.);
        assert_eq!(contour_axis(&flat, square), ContourAxis::Z);
        assert_eq!(
            contour_axis(
                &flat,
                Rect {
                    x: 0,
                    z: 0,
                    width_m: 64,
                    depth_m: 32
                }
            ),
            ContourAxis::X
        );
        // An exactly diagonal ramp is a tie and resolves to z, so the choice is
        // total rather than arbitrary.
        let diagonal = terrain_from_metres(&s, |x, z| (x + z) as f64 * 0.05);
        assert_eq!(contour_axis(&diagonal, square), ContourAxis::Z);
    }

    #[test]
    fn the_contour_ladder_stays_inside_the_unchanged_domain_and_aspect() {
        let template = FieldTemplate {
            width_m: 24,
            depth_m: 24,
        };
        let big = Rect {
            x: 0,
            z: 0,
            width_m: 143,
            depth_m: 143,
        };
        assert_eq!(
            contour_shapes(big, template, ContourAxis::X),
            vec![(72, 24), (60, 20), (48, 16)]
        );
        // The same cell offers the transposed strips on the other contour axis.
        assert_eq!(
            contour_shapes(big, template, ContourAxis::Z),
            vec![(24, 72), (20, 60), (16, 48)]
        );
        // A cell only 24 m deep cannot hold a 24 m short side plus the 2 m
        // margin, but a narrower strip still fits: this is the small-source case
        // the strategy exists for. The long side shrinks with the short side
        // because the aspect cap is applied to the short side that fits.
        assert_eq!(
            contour_shapes(
                Rect {
                    x: 0,
                    z: 0,
                    width_m: 143,
                    depth_m: 24
                },
                template,
                ContourAxis::X
            ),
            vec![(60, 20), (48, 16)]
        );
        // 16 m deep is 4 m short of the minimum strip plus its 2 m margins.
        assert!(contour_shapes(
            Rect {
                x: 0,
                z: 0,
                width_m: 143,
                depth_m: 16
            },
            template,
            ContourAxis::X
        )
        .is_empty());
        // Every offered shape honours the domain, the lattice, the aspect cap
        // and the cell's own 2 m margin.
        for cell in [
            big,
            Rect {
                x: 0,
                z: 0,
                width_m: 143,
                depth_m: 24,
            },
            Rect {
                x: 0,
                z: 0,
                width_m: 31,
                depth_m: 39,
            },
        ] {
            for axis in [ContourAxis::X, ContourAxis::Z] {
                for (w, d) in contour_shapes(cell, template, axis) {
                    let (long, short) = (w.max(d), w.min(d));
                    assert!((FIELD_MIN_SIDE..=FIELD_MAX_SIDE).contains(&long));
                    assert!((FIELD_MIN_SIDE..=FIELD_MAX_SIDE).contains(&short));
                    assert_eq!(long % 4, 0);
                    assert_eq!(short % 4, 0);
                    assert!(long <= FIELD_ASPECT_MAX * short);
                    assert!(w + 2 * FIELD_MARGIN as u32 <= cell.width_m);
                    assert!(d + 2 * FIELD_MARGIN as u32 <= cell.depth_m);
                    // A square shape may sit on either axis; a strip may not.
                    if w != d {
                        assert_eq!(w > d, axis == ContourAxis::X);
                    }
                }
            }
        }
    }


    /// A 160 x 24 m rural source whose requested 24 x 24 field plus its 2 m
    /// margin needs 28 m of depth. A 24 m deep connector bridges it to the road,
    /// so access is never the reason this source fails: it is exactly the
    /// "source_too_small_for_requested_field_and_margin" shape the contour
    /// ladder exists to recover.
    fn shallow_fixture() -> (
        SceneSpec,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let (scene, mut plan, terrain, water, entities) = fixture();
        let template = plan.cells[0].clone();
        let mut cell = template.clone();
        cell.id = "shallow.cell".into();
        cell.zone_id = "shallow.zone".into();
        cell.zone_kind = "rural".into();
        cell.role = LandcoverRole::AgricultureCandidate;
        cell.boundary = Rect {
            x: 0,
            z: 40,
            width_m: 160,
            depth_m: 24,
        };
        let mut connector = template;
        connector.id = "connector".into();
        connector.zone_id = "shallow.zone".into();
        connector.zone_kind = "rural".into();
        connector.role = LandcoverRole::FutureDevelopment;
        connector.boundary = Rect {
            x: 0,
            z: 64,
            width_m: 160,
            depth_m: 24,
        };
        plan.cells = vec![cell, connector];
        (scene, plan, terrain, water, entities)
    }
    /// Flat ground with a single ramp inside z = 44..48, well above the access
    /// lanes. A 10% ramp must pass and a 13% ramp must fail: that is the
    /// unchanged 12% gate, exercised with exact numbers.
    fn ramp_terrain(s: &SceneSpec, slope: f64) -> BakedTerrain {
        terrain_from_metres(s, |_, z| {
            let rise = (z - 44).clamp(0, 4) as f64;
            slope * rise
        })
    }

    #[test]
    fn the_contour_strategy_off_keeps_the_published_bytes() {
        let (s, p, t, w, e) = fixture();
        let base = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert_eq!(
            blake3::hash(&serde_json::to_vec(&base).unwrap())
                .to_hex()
                .to_string(),
            LEGACY_AGRICULTURE_HASH
        );
        // The off state is the program WITHOUT the key: every other spelling of
        // the strategy must produce exactly those bytes, including the
        // small-source rejections that only a present program can emit.
        let off_plan = with_pattern(&p, pattern((24, 24), None, false));
        let (off, off_patches) = realize(&s, &off_plan, &t, w, &e, &[]).unwrap();
        assert_eq!(off.field_shapes, None);
        let off_hash = blake3::hash(&serde_json::to_vec(&off).unwrap())
            .to_hex()
            .to_string();
        assert!(off
            .rejected_fields
            .iter()
            .any(|r| r.reason == "source_too_small_for_requested_field_and_margin"));
        for shapes in [
            Some(""),
            Some("contour"),
            Some("square"),
            Some("contour-elongated-ish"),
            Some("CONTOUR-ELONGATED"),
        ] {
            let program = with_pattern(&p, pattern((24, 24), shapes, false));
            let (out, patches) = realize(&s, &program, &t, w, &e, &[]).unwrap();
            assert_eq!(
                blake3::hash(&serde_json::to_vec(&out).unwrap())
                    .to_hex()
                    .to_string(),
                off_hash,
                "{shapes:?} must not move the realization bytes"
            );
            assert_eq!(
                serde_json::to_vec(&patches).unwrap(),
                serde_json::to_vec(&off_patches).unwrap(),
                "{shapes:?} must not move the surface bytes"
            );
        }
        // The same holds on the shared-track model, so the off switch is
        // independent of every other policy.
        let shared = with_pattern(&p, pattern((24, 24), None, true));
        let (shared_off, shared_patches) = realize(&s, &shared, &t, w, &e, &[]).unwrap();
        let shared_hash = blake3::hash(&serde_json::to_vec(&shared_off).unwrap())
            .to_hex()
            .to_string();
        let shared_other = with_pattern(&p, pattern((24, 24), Some("square"), true));
        let (shared_out, shared_out_patches) =
            realize(&s, &shared_other, &t, w, &e, &[]).unwrap();
        assert_eq!(
            blake3::hash(&serde_json::to_vec(&shared_out).unwrap())
                .to_hex()
                .to_string(),
            shared_hash
        );
        assert_eq!(
            serde_json::to_vec(&shared_out_patches).unwrap(),
            serde_json::to_vec(&shared_patches).unwrap()
        );
    }

    #[test]
    fn flat_fixture_on_realizes_the_same_squares_and_passes_the_audit() {
        let (s, p, t, w, e) = fixture();
        let on_plan = with_pattern(&p, pattern((24, 24), Some(CONTOUR_ELONGATED), false));
        let (on, _) = realize(&s, &on_plan, &t, w, &e, &[]).unwrap();
        // The flat 160 m fixture is already fully tiled by 24 x 24 squares, so
        // the ladder finds no legal strip and adds no field. Turning the
        // strategy on may only add fields, never replace one.
        assert_eq!(on.fields.len(), 20);
        assert_eq!(
            on.fields
                .iter()
                .filter(|f| f.contour_shape.is_some())
                .count(),
            0
        );
        assert_eq!(on.field_shapes.as_deref(), Some(CONTOUR_ELONGATED));
        assert!(verify_field_shapes(&on, &on_plan, &t, w, &e).is_ok());
        // The square rule's rejections survive verbatim, and the strips that
        // lost their cells to a square say so by name.
        assert!(on
            .rejected_fields
            .iter()
            .any(|r| r.reason == "source_too_small_for_requested_field_and_margin"));
        assert!(on
            .rejected_fields
            .iter()
            .any(|r| r.reason == "reserved_field_or_track"));
    }

    #[test]
    fn contour_strips_realize_a_source_too_shallow_for_the_requested_field() {
        let (s, plan, t, w, e) = shallow_fixture();
        let off_plan = with_pattern(&plan, pattern((24, 24), None, false));
        let (off, off_patches) = realize(&s, &off_plan, &t, w, &e, &[]).unwrap();
        // Before: the 24 m deep source is refused by name and prepares nothing.
        assert!(off.fields.is_empty());
        assert!(off_patches.is_empty());
        assert_eq!(
            off.rejected_fields
                .iter()
                .filter(|r| r.reason == "source_too_small_for_requested_field_and_margin")
                .count(),
            1
        );
        let on_plan = with_pattern(&plan, pattern((24, 24), Some(CONTOUR_ELONGATED), false));
        let (on, on_patches) = realize(&s, &on_plan, &t, w, &e, &[]).unwrap();
        // After: two 60 x 20 strips, the widest shape 20 m of usable depth holds.
        assert_eq!(on.fields.len(), 2);
        let mut strips = on.fields.iter().collect::<Vec<_>>();
        strips.sort_by_key(|f| (f.boundary.x, f.boundary.z));
        assert_eq!(
            strips
                .iter()
                .map(|f| (f.boundary.x, f.boundary.z, f.boundary.width_m, f.boundary.depth_m))
                .collect::<Vec<_>>(),
            vec![(2, 42, 60, 20), (66, 42, 60, 20)]
        );
        for field in &strips {
            assert_eq!(field.source_cell_id, "shallow.cell");
            // The authored 2 m margin and the unchanged 4 m headland inset.
            assert!(field.boundary.x >= 0 + 2 && field.boundary.x + 60 <= 158);
            assert!(field.boundary.z >= 42 && field.boundary.z + 20 <= 62);
            assert_eq!(
                (field.cultivated.width_m, field.cultivated.depth_m),
                (52, 12)
            );
            assert_eq!(field.cultivated.area(), 624);
            assert_eq!(field.boundary.area() - field.cultivated.area(), 576);
            assert_eq!(
                field.contour_shape,
                Some(ContourFieldShape {
                    long_axis: "x".into(),
                    long_side_m: 60,
                    short_side_m: 20,
                })
            );
            assert!(field.access_length_m <= 160);
        }
        // The small-source refusal is gone because a legal field now fits.
        assert!(on
            .rejected_fields
            .iter()
            .all(|r| r.reason != "source_too_small_for_requested_field_and_margin"));
        // Every square the road-only rule would have accepted is untouched: on
        // this fixture there are none, and the strips are pure addition.
        assert!(on.fields.len() >= off.fields.len());
        // Area accounting is the unchanged headland decomposition.
        let cultivated = on_patches
            .iter()
            .filter(|p| p.material == Material::CultivatedSoil)
            .map(|p| p.area_m2)
            .sum::<f64>();
        let headland = on_patches
            .iter()
            .filter(|p| p.material == Material::FieldHeadland)
            .map(|p| p.area_m2)
            .sum::<f64>();
        assert_eq!(cultivated, 2. * 624.);
        assert_eq!(headland, 2. * 576.);
        // The audit recomputes all of it from the published plan and terrain.
        verify_field_shapes(&on, &on_plan, &t, w, &e).unwrap();
        // Determinism: same input, same serialized bytes.
        let (again, again_patches) = realize(&s, &on_plan, &t, w, &e, &[]).unwrap();
        assert_eq!(
            serde_json::to_vec(&on).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&on_patches).unwrap(),
            serde_json::to_vec(&again_patches).unwrap()
        );
    }

    #[test]
    fn the_unchanged_twelve_percent_grade_still_refuses_a_strip() {
        let (s, plan, _, w, e) = shallow_fixture();
        let on_plan = with_pattern(&plan, pattern((24, 24), Some(CONTOUR_ELONGATED), false));
        // 10% passes: the same strips realize on the gentler ramp.
        let gentle = ramp_terrain(&s, 0.10);
        let (pass, _) = realize(&s, &on_plan, &gentle, w, &e, &[]).unwrap();
        assert_eq!(pass.fields.len(), 2);
        assert!(pass
            .rejected_fields
            .iter()
            .all(|r| r.reason != "field_grade_exceeds_12_percent"));
        verify_field_shapes(&pass, &on_plan, &gentle, w, &e).unwrap();
        // 12.5% fails: the gate is not relaxed for strips. 0.125 is exact in
        // binary, so this is a slope decision and not a rounding artefact.
        let steep = ramp_terrain(&s, 0.125);
        let (fail, fail_patches) = realize(&s, &on_plan, &steep, w, &e, &[]).unwrap();
        assert!(fail.fields.is_empty());
        assert!(fail_patches.is_empty());
        // Every strip candidate in the cell is refused by name: two 60x20
        // anchors plus three 48x16 anchors.
        assert_eq!(
            fail.rejected_fields
                .iter()
                .filter(|r| r.reason == "field_grade_exceeds_12_percent")
                .count(),
            5
        );
    }

    #[test]
    fn the_shape_audit_recomputes_margin_domain_aspect_axis_grade_and_protection() {
        let (s, plan, t, w, e) = shallow_fixture();
        let on_plan = with_pattern(&plan, pattern((24, 24), Some(CONTOUR_ELONGATED), false));
        let (on, _) = realize(&s, &on_plan, &t, w, &e, &[]).unwrap();
        assert!(verify_field_shapes(&on, &on_plan, &t, w, &e).is_ok());
        // The field nearest the road wins the claim order, so pick the western
        // strip by position rather than by index.
        let field_index = on
            .fields
            .iter()
            .position(|f| f.boundary.x == 2)
            .expect("the shallow fixture realizes the western strip");
        let tamper = |field: usize, edit: &dyn Fn(&mut FarmField)| {
            let mut copy = on.clone();
            edit(&mut copy.fields[field]);
            verify_field_shapes(&copy, &on_plan, &t, w, &e)
        };
        // A published shape that no longer matches its own rectangle.
        assert_eq!(
            tamper(field_index, &|f| f.contour_shape.as_mut().unwrap().long_side_m += 4)
                .unwrap_err(),
            "contour field shape differs from its own boundary"
        );
        // A strip on the wrong axis for its cell's contours.
        assert_eq!(
            tamper(field_index, &|f| {
                f.contour_shape.as_mut().unwrap().long_axis = "z".into();
            })
            .unwrap_err(),
            "contour field long side is not on the published axis"
        );
        // A shape that grew past its source cell's 2 m margin.
        assert_eq!(
            tamper(field_index, &|f| f.boundary.x -= 4).unwrap_err(),
            "agricultural field breaks the unchanged 2m source margin"
        );
        // A tampered rectangular field on a world that never enabled the rule.
        let mut legacy = on.clone();
        legacy.field_shapes = None;
        assert_eq!(
            verify_field_shapes(&legacy, &on_plan, &t, w, &e).unwrap_err(),
            "agricultural field-shape policy differs from the manifest"
        );
        // Synthetic fields, so the illegal cases a generator must never publish
        // are still provable by the audit alone. The source is 160 x 128 m and
        // flat, so its contour axis is x and the ladder offers 72x24, 60x20 and
        // 48x16.
        let (big_scene, big_plan, _, big_water, big_entities) = shallow_fixture();
        let mut wide_plan = big_plan.clone();
        for cell in &mut wide_plan.cells {
            cell.boundary = Rect {
                x: 0,
                z: 0,
                width_m: 160,
                depth_m: 128,
            };
            cell.role = LandcoverRole::AgricultureCandidate;
        }
        let wide = with_pattern(&wide_plan, pattern((24, 24), Some(CONTOUR_ELONGATED), false));
        let field = |x: i64, z: i64, width_m: u32, depth_m: u32, shape: ContourFieldShape| FarmField {
            id: format!("probe.field.{x}.{z}"),
            source_cell_id: "shallow.cell".into(),
            boundary: Rect {
                x,
                z,
                width_m,
                depth_m,
            },
            cultivated: Rect {
                x: x + HEADLAND,
                z: z + HEADLAND,
                width_m: width_m - 8,
                depth_m: depth_m - 8,
            },
            road_id: "road".into(),
            access_path: vec![[x + 2, z + 2], [x + 2, 80]],
            access_method: "direct_axis_route".into(),
            access_length_m: 1,
            access_search_expanded: 0,
            preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
            shared_track: None,
            contour_shape: Some(shape),
        };
        let realization = |f: FarmField| AgricultureRealization {
            model: "explicit-rectangular-field-access-v3".into(),
            fields: vec![f],
            rejected_fields: vec![],
            field_shapes: Some(CONTOUR_ELONGATED.into()),
        };
        let strip = |w: u32, d: u32, axis: &str| ContourFieldShape {
            long_axis: axis.into(),
            long_side_m: w.max(d),
            short_side_m: w.min(d),
        };
        let flat = terrain_from_metres(&big_scene, |_, _| 0.);
        // Outside the source cell altogether.
        let outside = realization(field(-8, 42, 60, 20, strip(60, 20, "x")));
        assert_eq!(
            verify_field_shapes(&outside, &wide, &flat, big_water, &big_entities).unwrap_err(),
            "agricultural field breaks the unchanged 2m source margin"
        );
        // Inside the source but outside the 16..=128 m side domain.
        let oversized = realization(field(2, 2, 140, 16, strip(140, 16, "x")));
        assert_eq!(
            verify_field_shapes(&oversized, &wide, &flat, big_water, &big_entities).unwrap_err(),
            "contour field side leaves the unchanged 16..=128m domain"
        );
        // Inside the domain but past the aspect cap.
        let elongated = realization(field(2, 2, 128, 32, strip(128, 32, "x")));
        assert_eq!(
            verify_field_shapes(&elongated, &wide, &flat, big_water, &big_entities).unwrap_err(),
            "contour field aspect ratio exceeds its unchanged cap"
        );
        // A legal contour rectangle the ladder may not offer here: 56x20 is in
        // the domain, under the aspect cap and on the cell's contour axis, but
        // the ladder only offers 60x20 for that width.
        let invented = realization(field(2, 42, 56, 20, strip(56, 20, "x")));
        assert_eq!(
            verify_field_shapes(&invented, &wide, &flat, big_water, &big_entities).unwrap_err(),
            "contour field shape is not a legal strip for its source"
        );
        // The legal control: the narrowest strip the ladder offers, placed so a
        // ramp inside the cell can be laid over it.
        let legal = || realization(field(2, 42, 48, 16, strip(48, 16, "x")));
        assert!(verify_field_shapes(&legal(), &wide, &flat, big_water, &big_entities).is_ok());
        // Reserved land use under an otherwise legal strip.
        let mut reserved = big_entities.clone();
        let mut park = reserved[0].clone();
        park.entity_id = "authored.park".into();
        park.kind = "park".into();
        park.world_x = 20;
        park.world_z = 44;
        park.width_m = 16.;
        park.depth_m = 16.;
        reserved.push(park);
        assert_eq!(
            verify_field_shapes(&legal(), &wide, &flat, big_water, &reserved).unwrap_err(),
            "agricultural field intersects a reserved feature"
        );
        // And the slope gate, re-sampled from the published rectangle. 0.125 is
        // exact in binary, so this is a slope decision and not a rounding
        // artefact; 0.10 must still pass.
        let steep = ramp_terrain(&big_scene, 0.125);
        assert_eq!(
            verify_field_shapes(&legal(), &wide, &steep, big_water, &big_entities).unwrap_err(),
            "agricultural field exceeds the unchanged 12 percent grade"
        );
        let gentle = ramp_terrain(&big_scene, 0.10);
        assert!(verify_field_shapes(&legal(), &wide, &gentle, big_water, &big_entities).is_ok());
    }

    /// A 160 m corridor whose only road is a reachable 8 m x 64 m edge strip on
    /// its east side, with one authored agricultural source covering the whole
    /// corridor. Every candidate can reach that road by itself, so nothing here
    /// can be justified by "no road route exists": the only reason a lane may
    /// end on an already realized track is the round-16 cost rule. The fields
    /// nearest the road lay their lanes first; the fields behind them sit a few
    /// metres from those lanes and tens of metres from the road, which is the
    /// configuration a real field-track network exists for.
    fn lane_corridor_fixture() -> (
        SceneSpec,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "corridor".into(),
            width_m: 160,
            depth_m: 160,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m: 160,
            source: "fixture".into(),
        }];
        let roads = RoadNetworkPlan {
            road_cut_mm: None,
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: scene.scene_id.clone(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            pad_conversions: vec![],
            conversion_allocation: None,
        conversion_fringe: None,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: Vec::new(),
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
            nodes: vec![],
            edges: vec![RoadEdge {
                id: "road".into(),
                from: 0,
                to: 1,
                world_x: 156,
                world_z: 80,
                length_m: 64,
                rotation_y_deg: 90.,
                natural_min_m: 0.,
                natural_max_m: 0.,
                profile: None,
                earthwork_blend_m: None,
            }],
            roadside_sites: vec![],
        
            entry_corridor_policy: Default::default(),
};
        let development_plan = development::plan(&scene, &zones, &roads).unwrap();
        let mut plan = development::plan_landcover(&scene, &development_plan).unwrap();
        let template = plan.cells[0].clone();
        plan.cells = [Rect {
            x: 0,
            z: 0,
            width_m: 160,
            depth_m: 160,
        }]
        .into_iter()
        .enumerate()
        .map(|(index, boundary)| {
            let mut cell = template.clone();
            cell.id = format!("corridor.cell.{index}");
            cell.boundary = boundary;
            cell.role = LandcoverRole::AgricultureCandidate;
            cell
        })
        .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        let chunk = crate::worldgen::STREAM_CHUNK_METERS;
        let mut z = 0_u32;
        while z < scene.depth_m {
            let tall = (scene.depth_m - z).min(chunk);
            terrain
                .insert_chunk(
                    0,
                    z as i32,
                    160,
                    tall,
                    &0_i16.to_le_bytes().repeat((160 * tall) as usize),
                )
                .unwrap();
            z += chunk;
        }
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        (scene, plan, terrain, water, roads.entities())
    }

    /// Locked bytes of the road-only agriculture surface on the corridor
    /// fixture. Measured on the tree that still had the round-11 trigger, so
    /// this constant is the pre-round-16 output itself, not a snapshot taken
    /// after the change: the opt-in rule may only add keys inside
    /// `sharedTrack`, never move a byte of a world that did not enable it.
    const LEGACY_CORRIDOR_HASH: &str =
        "5c190930018a7e431c6517cefddfadce3139bc4bfe14c738bfcb055ec20c65a3";
    /// The same freeze on the barrier fixture, also measured before the cost
    /// rule existed: 6 fields, all on direct axis routes.
    const LEGACY_BARRIER_HASH: &str =
        "8640b13d32c179799db63f5ff21fef8bd3e30fdb4ecdd8db0ba902beb0dc38dc";

    fn joins(realization: &AgricultureRealization) -> Vec<(u64, u64, u64, String, u64)> {
        realization
            .fields
            .iter()
            .filter_map(|field| {
                field.shared_track.as_ref().map(|join| {
                    (
                        field.access_length_m,
                        join.road_path_length_m,
                        join.road_path_length_m
                            .saturating_sub(field.access_length_m),
                        join.basis.clone(),
                        field.access_length_m + join.road_distance_m,
                    )
                })
            })
            .collect()
    }

    /// Requirement 1: with the switch off the published bytes are the
    /// pre-round-16 bytes, on both fixtures, and the new cost rule cannot even
    /// run: the corridor hands every field a real road route, so a trigger that
    /// only fires on cost has nothing to compare and the output stays exactly as
    /// it was.
    #[test]
    fn the_shared_track_switch_off_keeps_the_published_bytes_frozen() {
        let (scene, plan, terrain, water, entities) = lane_corridor_fixture();
        let off = with_track_network(&plan, false);
        let (realization, patches) =
            realize(&scene, &off, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(realization.model, "explicit-rectangular-field-access-v3");
        assert_eq!(realization.fields.len(), 24);
        assert!(joins(&realization).is_empty());
        // 2120 m of new lane and 16832 m2 of field/track surface: the road-only
        // cost of this corridor, frozen.
        assert_eq!(
            realization
                .fields
                .iter()
                .map(|field| field.access_length_m)
                .sum::<u64>(),
            2120
        );
        assert_eq!(
            patches
                .iter()
                .map(|patch| patch.vertices.len() / 4)
                .sum::<usize>(),
            16832
        );
        let bytes = serde_json::to_vec(&(realization.clone(), patches.clone())).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("sharedTrack"),
            "the off switch must not publish a shared-track field"
        );
        assert_eq!(blake3::hash(&bytes).to_hex().to_string(), LEGACY_CORRIDOR_HASH);
        // A second fixture, frozen independently: proving the off path is
        // untouched is only meaningful on more than one input.
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let (barrier, barrier_patches) = realize(
            &scene,
            &with_track_network(&plan, false),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        assert_eq!(barrier.fields.len(), 6);
        assert!(barrier
            .fields
            .iter()
            .all(|field| field.access_method == "direct_axis_route"));
        assert_eq!(
            blake3::hash(&serde_json::to_vec(&(barrier, barrier_patches)).unwrap())
                .to_hex()
                .to_string(),
            LEGACY_BARRIER_HASH
        );
    }

    /// Requirement 2: with the switch on, the cost rule fires on a fixture where
    /// every field already has a road route, and every join it accepts publishes
    /// the comparison it won on: at least `JOIN_MIN_SAVING_M` metres of new lane
    /// fewer than the road route, and a whole path back to a real road inside
    /// the unchanged 160 m. On the barrier fixture the same switch still adds
    /// fields, so the two effects are measured side by side.
    #[test]
    fn a_cheaper_join_lays_fewer_new_lane_metres_than_the_road_route() {
        let (scene, plan, terrain, water, entities) = lane_corridor_fixture();
        let (off, _) = realize(
            &scene,
            &with_track_network(&plan, false),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        let (on, on_patches) = realize(
            &scene,
            &with_track_network(&plan, true),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        assert_eq!(on.model, "explicit-rectangular-field-access-v4-shared-tracks");
        // Same fields, different access: this fixture cannot justify a join by
        // "no road route", so all nine are cost decisions.
        assert_eq!(on.fields.len(), off.fields.len());
        assert_eq!(on.fields.len(), 24);
        let mut savings = vec![];
        let mut totals = vec![];
        for (len, road, saving, basis, total) in joins(&on) {
            assert_eq!(basis, JOIN_BASIS_CHEAPER);
            // The first condition of the new rule, on published numbers.
            assert!(saving >= JOIN_MIN_SAVING_M, "join saved only {saving} m");
            assert_eq!(road, len + saving);
            // The second condition, unchanged: the whole way back to a road.
            assert!(total <= MAX_ACCESS_LENGTH as u64);
            savings.push(saving);
            totals.push(total);
        }
        savings.sort_unstable();
        totals.sort_unstable();
        assert_eq!(savings, vec![44, 46, 80, 86, 86, 108, 114, 114, 142]);
        assert_eq!(totals, vec![60, 88, 88, 110, 116, 116, 138, 144, 144]);
        // What the rule buys, measured over the whole fixture: 2120 -> 1300 m of
        // new lane and 16832 -> 16632 m2 of laid surface, with the same fields.
        assert_eq!(
            on.fields
                .iter()
                .map(|field| field.access_length_m)
                .sum::<u64>(),
            1300
        );
        assert_eq!(
            on_patches
                .iter()
                .map(|patch| patch.vertices.len() / 4)
                .sum::<usize>(),
            16632
        );
        // The switch still only ever ADDS on a source where the road search
        // cannot finish: 6 -> 15 fields, all nine joins declared as having no
        // road alternative rather than as a saving.
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let (plain, _) = realize(
            &scene,
            &with_track_network(&plan, false),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        let (shared, _) = realize(
            &scene,
            &with_track_network(&plan, true),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        assert_eq!(plain.fields.len(), 6);
        assert_eq!(shared.fields.len(), 15);
        assert!(shared.fields.len() > plain.fields.len());
        let barrier_joins = joins(&shared);
        assert_eq!(barrier_joins.len(), 9);
        for (_, road, _, basis, total) in barrier_joins {
            assert_eq!(basis, JOIN_BASIS_NO_ROAD);
            assert_eq!(road, 0, "a join with no road route claims none");
            assert!(total <= MAX_ACCESS_LENGTH as u64);
        }
    }

    /// Requirement 3: an access whose whole path back to a real road overruns
    /// the unchanged 160 m is refused by name, and the refusal is recomputable:
    /// the corridor publishes exactly one such field, and a forged join that
    /// hides the overrun inside `road_distance_m` fails the same named check.
    #[test]
    fn a_join_over_the_unchanged_budget_is_refused_by_name() {
        let (scene, plan, terrain, water, entities) = lane_corridor_fixture();
        let (on, _) = realize(
            &scene,
            &with_track_network(&plan, true),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        let over: Vec<_> = on
            .rejected_fields
            .iter()
            .filter(|rejection| {
                rejection
                    .reason
                    .contains("access_via_existing_track_exceeds_budget")
            })
            .collect();
        assert_eq!(over.len(), 1);
        assert_eq!(
            over[0].reason,
            "no_verified_access:access_via_existing_track_exceeds_budget|no_target_road"
        );
        // No published access, joined or not, may spend more than 160 m.
        for field in &on.fields {
            let spent = field
                .shared_track
                .as_ref()
                .map_or(0, |join| join.road_distance_m);
            assert!(field.access_length_m + spent <= MAX_ACCESS_LENGTH as u64);
        }
        // Forge the overrun on a real published join: the lane is still 14 m and
        // its join is still the owner's, but the claim now totals 161 m.
        let victim = on
            .fields
            .iter()
            .find(|field| field.access_length_m == 14 && field.shared_track.is_some())
            .expect("the corridor publishes a 14 m joined lane");
        let mut forged = on.clone();
        let field = forged
            .fields
            .iter_mut()
            .find(|other| other.id == victim.id)
            .unwrap();
        assert_eq!(
            field.access_length_m
                + field.shared_track.as_ref().unwrap().road_distance_m,
            60
        );
        field.shared_track.as_mut().unwrap().road_distance_m = 161 - field.access_length_m;
        assert_eq!(
            verify_access_network(&forged, &entities).unwrap_err(),
            "agricultural access exceeds the unchanged 160m budget"
        );
        // And the same for a chain that only overruns once the inherited metres
        // are counted: the budget is cumulative metres, never hops.
        let mut chain = on.clone();
        let field = chain
            .fields
            .iter_mut()
            .find(|other| other.id == victim.id)
            .unwrap();
        field.shared_track.as_mut().unwrap().road_distance_m = 160;
        assert_eq!(
            verify_access_network(&chain, &entities).unwrap_err(),
            "agricultural access exceeds the unchanged 160m budget"
        );
    }

    /// Requirement 4: the threshold is a cost rule, never a licence. With it
    /// driven to zero (a "negative" threshold saturates to the same thing: a
    /// lane can never save fewer than zero metres) the generator does admit
    /// joins it would otherwise refuse, and every hard limit still holds - same
    /// fields, same geometry, and every published path still inside the 160 m
    /// budget. The audit then refuses those records by name, so the threshold is
    /// enforced independently of whoever ran the generator.
    #[test]
    fn a_zero_saving_threshold_changes_no_hard_limit() {
        let (scene, plan, terrain, water, entities) = lane_corridor_fixture();
        let on = with_track_network(&plan, true);
        let (strict, _) = realize_with_query(
            &scene,
            &on,
            &terrain,
            water,
            &entities,
            JOIN_MIN_SAVING_M,
            |_, _| false,
        )
        .unwrap();
        let (loose, loose_patches) =
            realize_with_query(&scene, &on, &terrain, water, &entities, 0, |_, _| false).unwrap();
        assert!(verify_access_network(&strict, &entities).is_ok());
        // The lowered threshold admits two more joins, and they are cheaper by
        // 24 m and 6 m: exactly the records the threshold exists to refuse.
        assert_eq!(joins(&strict).len(), 9);
        assert_eq!(joins(&loose).len(), 10);
        assert_eq!(
            joins(&loose)
                .iter()
                .map(|(_, _, saving, _, _)| *saving)
                .min(),
            Some(6)
        );
        // No hard limit moved: the fields are identical, every join is still a
        // real reuse of an older lane, and every whole path stays inside 160 m.
        assert_eq!(loose.fields.len(), strict.fields.len());
        assert_eq!(loose.rejected_fields, strict.rejected_fields);
        for (left, right) in loose.fields.iter().zip(&strict.fields) {
            assert_eq!(left.boundary, right.boundary);
            assert_eq!(left.cultivated, right.cultivated);
        }
        for (_, _, _, basis, total) in joins(&loose) {
            assert_eq!(basis, JOIN_BASIS_CHEAPER);
            assert!(total <= MAX_ACCESS_LENGTH as u64, "a lane over budget");
        }
        // Measured: the loosest threshold still publishes totals in 62..=144 m.
        let mut loose_totals: Vec<u64> = joins(&loose).iter().map(|row| row.4).collect();
        loose_totals.sort_unstable();
        assert_eq!(
            loose_totals,
            vec![62, 66, 82, 88, 110, 116, 116, 138, 144, 144]
        );
        let loose_cells = loose_patches
            .iter()
            .map(|patch| patch.vertices.len() / 4)
            .sum::<usize>();
        assert_eq!(loose_cells, 16572);
        // The audit refuses the same realization by name: a threshold of zero is
        // not allowed to publish a join that saves less than the contract.
        assert_eq!(
            verify_access_network(&loose, &entities).unwrap_err(),
            "shared farm track join does not save the required lane metres"
        );
    }

    /// Requirement 5: every published number a join rests on is re-derived, and
    /// a forged one fails by name: the join square, the metres the chain spends
    /// back to a road, the chain length, the road alternative it claims to beat
    /// and the basis that says which trigger accepted it.
    #[test]
    fn the_access_audit_detects_a_forged_join_record() {
        let (scene, plan, terrain, water, entities) = lane_corridor_fixture();
        let (on, _) = realize(
            &scene,
            &with_track_network(&plan, true),
            &terrain,
            water,
            &entities,
            &[],
        )
        .unwrap();
        assert!(verify_access_network(&on, &entities).is_ok());
        let id = on
            .fields
            .iter()
            .find(|field| field.shared_track.is_some())
            .unwrap()
            .id
            .clone();
        let forged = |edit: &dyn Fn(&mut FarmField)| {
            let mut copy = on.clone();
            edit(copy.fields.iter_mut().find(|field| field.id == id).unwrap());
            verify_access_network(&copy, &entities)
        };
        // The square the lane claims to join must be the square it ends on.
        assert_eq!(
            forged(&|field| field.shared_track.as_mut().unwrap().cell = [0, 0]).unwrap_err(),
            "shared farm track lane does not end on its join"
        );
        // The metres the joined square already spends are recomputed from the
        // older lane, never read from the record.
        assert_eq!(
            forged(&|field| field.shared_track.as_mut().unwrap().road_distance_m += 2)
                .unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        assert_eq!(
            forged(&|field| field.shared_track.as_mut().unwrap().chain_lanes = 7).unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        // A saving that is smaller than the threshold is a forged cost claim.
        assert_eq!(
            forged(&|field| {
                let join = field.shared_track.as_mut().unwrap();
                join.road_path_length_m = field.access_length_m + JOIN_MIN_SAVING_M - 1;
            })
            .unwrap_err(),
            "shared farm track join does not save the required lane metres"
        );
        // A road route longer than the road search's own budget is impossible.
        assert_eq!(
            forged(&|field| {
                field.shared_track.as_mut().unwrap().road_path_length_m = 161;
            })
            .unwrap_err(),
            "shared farm track road alternative exceeds the unchanged 160m budget"
        );
        // A join that says it had no road route may not claim one.
        assert_eq!(
            forged(&|field| {
                field.shared_track.as_mut().unwrap().basis = JOIN_BASIS_NO_ROAD.into();
            })
            .unwrap_err(),
            "shared farm track join without a road route claims a road lane"
        );
        // An unknown basis is not a third trigger.
        assert_eq!(
            forged(&|field| {
                field.shared_track.as_mut().unwrap().basis = "cheapest".into();
            })
            .unwrap_err(),
            "shared farm track join has an unknown cost basis"
        );
        // The published lane length is the length of the published path.
        assert_eq!(
            forged(&|field| field.access_length_m += 2).unwrap_err(),
            "agricultural lane length differs from its own path"
        );
        // Dropping the join leaves a lane that ends on a track, not on a road.
        assert_eq!(
            forged(&|field| {
                field.shared_track = None;
                field.access_method = "bounded_grid_detour".into();
            })
            .unwrap_err(),
            "agricultural lane does not reach a road footprint"
        );
    }

    /// Round-16 measurement, on an already published 1 km world and with no
    /// world generation at all: replay `agriculture::realize` over the same
    /// terrain, entities and access surfaces, changing nothing but the opt-in
    /// policy and the threshold it compares against. `GLYPHWEAVE_AUDIT_WORLD`
    /// names the published world and `GLYPHWEAVE_AUDIT_REPORT` a fresh report
    /// path; the first run must reproduce the published agriculture bytes.
    #[test]
    #[ignore = "explicit 1 km published-world replay; requires GLYPHWEAVE_AUDIT_WORLD"]
    fn shared_track_cost_rule_replay_on_a_published_world() {
        use super::super::landuse_ledger;
        use crate::worldgen::{
            LandUseProfile, SceneIndex, WorldManifest, bake_access_surfaces, build_access_graph,
            water_kind,
        };
        let root = std::path::PathBuf::from(
            std::env::var_os("GLYPHWEAVE_AUDIT_WORLD").expect("set a completed world path"),
        );
        let manifest: WorldManifest = serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(root.join("glyphweave.manifest.json")).unwrap(),
        ))
        .unwrap();
        let spec = &manifest.scenes[0];
        let folder = root.join("scenes").join(&spec.scene_id);
        let mut scene: SceneIndex = serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(folder.join("scene.json")).unwrap(),
        ))
        .unwrap();
        let mut terrain = BakedTerrain::new(spec).unwrap();
        for chunk in &scene.chunks {
            terrain
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &std::fs::read(folder.join(&chunk.height_file)).unwrap(),
                )
                .unwrap();
        }
        terrain.require_complete().unwrap();
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            spec,
            &manifest.style,
        );
        let access = bake_access_surfaces(
            &build_access_graph(&scene.entities, &terrain, water),
            &terrain,
        )
        .unwrap_or_default();
        let plan = scene.landcover_plan.as_ref().unwrap().clone();
        let profile = LandUseProfile::from_style(&manifest.style).expect("land use profile");
        let published = scene.landcover_surfaces.as_ref().unwrap().clone();
        let published_agriculture = (
            published.agriculture.clone(),
            published
                .patches
                .iter()
                .filter(|patch| {
                    matches!(
                        patch.material,
                        Material::CultivatedSoil | Material::FieldHeadland | Material::FarmTrack
                    )
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let mut runs = vec![];
        let pedestrian = super::super::pedestrian_index::PedestrianIndex::new(&access).unwrap();
        let mut configs = vec![
            ("published", plan.clone(), JOIN_MIN_SAVING_M),
            (
                "trackNetworkOff",
                with_pattern(&plan, pattern((24, 24), None, false)),
                JOIN_MIN_SAVING_M,
            ),
        ];
        // The threshold sweep the round-16 report needs: the same policy, the
        // same inputs, only the number the trigger compares against moves.
        for min_saving in [0_u64, 20, 30, 40, 60, 80, 120] {
            configs.push((
                Box::leak(format!("trackNetworkOn{min_saving}").into_boxed_str()),
                with_pattern(&plan, pattern((24, 24), None, true)),
                min_saving,
            ));
        }
        for (name, config, min_saving) in configs {
            let started = std::time::Instant::now();
            let (realization, patches) = realize_with_query(
                spec,
                &config,
                &terrain,
                water,
                &scene.entities,
                min_saving,
                |x, z| pedestrian.overlaps_cell(x, z),
            )
            .unwrap();
            let seconds = started.elapsed().as_secs_f64();
            let audit = verify_access_network(&realization, &scene.entities);
            if name == "published" {
                // The control: the replay must reproduce the published bytes.
                assert_eq!(
                    blake3::hash(&serde_json::to_vec(&(realization.clone(), patches.clone())).unwrap())
                        .to_hex()
                        .to_string(),
                    blake3::hash(&serde_json::to_vec(&published_agriculture).unwrap())
                        .to_hex()
                        .to_string(),
                    "the published world no longer reproduces its own agriculture"
                );
            }
            {
                let surfaces = scene.landcover_surfaces.as_mut().unwrap();
                surfaces.agriculture = realization.clone();
                surfaces.patches.retain(|patch| {
                    !matches!(
                        patch.material,
                        Material::CultivatedSoil | Material::FieldHeadland | Material::FarmTrack
                    )
                });
                surfaces.patches.extend(patches.iter().cloned());
            }
            let ledger = landuse_ledger::analyze(&scene, &profile).expect("ledger");
            let rural = &ledger.by_purpose["rural"];
            let mut reasons = BTreeMap::<String, usize>::new();
            for rejection in &realization.rejected_fields {
                let token = rejection
                    .reason
                    .split('|')
                    .map(|part| part.trim_start_matches("no_verified_access:"))
                    .collect::<BTreeSet<_>>();
                for part in token {
                    *reasons.entry(part.to_string()).or_default() += 1;
                }
            }
            let joined: Vec<_> = realization
                .fields
                .iter()
                .filter_map(|field| {
                    field.shared_track.as_ref().map(|join| {
                        (
                            field.access_length_m,
                            join.road_path_length_m,
                            join.road_path_length_m
                                .saturating_sub(field.access_length_m),
                            join.basis.clone(),
                            field.access_length_m + join.road_distance_m,
                        )
                    })
                })
                .collect();
            let by_basis = |basis: &str| joined.iter().filter(|row| row.3 == basis).count();
            let saving = |text: &str| {
                reasons
                    .iter()
                    .filter(|(reason, _)| reason.contains(text))
                    .map(|(_, count)| *count)
                    .sum::<usize>()
            };
            runs.push(serde_json::json!({
                "config": name,
                "model": realization.model,
                "fields": realization.fields.len(),
                "joins": joined.len(),
                "joinsCheaper": by_basis(JOIN_BASIS_CHEAPER),
                "joinsNoRoad": by_basis(JOIN_BASIS_NO_ROAD),
                "savings": joined.iter().map(|row| row.2).collect::<Vec<_>>(),
                "totals": joined.iter().map(|row| row.4).collect::<Vec<_>>(),
                "newLaneMetres": realization.fields.iter().map(|f| f.access_length_m).sum::<u64>(),
                "farmTrackM2": patches
                    .iter()
                    .filter(|patch| patch.material == Material::FarmTrack)
                    .map(|patch| patch.vertices.len() / 4)
                    .sum::<usize>(),
                "rejections": realization.rejected_fields.len(),
                "reasons": reasons,
                "accessViaExistingTrackExceedsBudget": saving(
                    "access_via_existing_track_exceeds_budget"
                ),
                "noSharedTrackWithinBudget": saving("no_shared_track_within_budget"),
                "noVerifiedDetour": saving("no_verified_detour"),
                "noRoadWithinDirectAccessBudget": saving("no_road_within_direct_access_budget"),
                "accessAudit": audit.as_ref().err().cloned(),
                "ruralRealizedM2": rural.realized_m2,
                "ruralPlannedM2": rural.planned_m2,
                "ruralUnresolvedM2": rural.unresolved_m2,
                "seconds": seconds,
            }));
        }
        // Turning the policy on must not cost a field the road-only rule could
        // realize, and every join it does publish must be inside the budget.
        for run in &runs {
            for total in run["totals"].as_array().into_iter().flatten() {
                assert!(total.as_u64().unwrap() <= MAX_ACCESS_LENGTH as u64);
            }
        }
        let report = serde_json::json!({
            "format": "glyphweave.shared-farm-track-cost-replay",
            "world": root,
            "sceneId": spec.scene_id,
            "sizeM": [spec.width_m, spec.depth_m],
            "entities": scene.entities.len(),
            "runs": runs,
        });
        if let Some(path) = std::env::var_os("GLYPHWEAVE_AUDIT_REPORT") {
            std::io::Write::write_all(
                &mut std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(std::path::PathBuf::from(path))
                    .expect("the audit report path must not already exist"),
                &serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    #[test]
    #[ignore = "explicit 1 km published-world measurement; requires GLYPHWEAVE_AUDIT_WORLD"]
    fn contour_shape_audit_of_a_published_world() {
        use super::super::landuse_ledger;
        use crate::worldgen::{
            LandUseProfile, SceneIndex, WorldManifest, bake_access_surfaces, build_access_graph,
            water_kind,
        };
        let root = std::path::PathBuf::from(
            std::env::var_os("GLYPHWEAVE_AUDIT_WORLD").expect("set a completed world path"),
        );
        let manifest: WorldManifest = serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(root.join("glyphweave.manifest.json")).unwrap(),
        ))
        .unwrap();
        let spec = &manifest.scenes[0];
        let folder = root.join("scenes").join(&spec.scene_id);
        let mut scene: SceneIndex = serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(folder.join("scene.json")).unwrap(),
        ))
        .unwrap();
        let mut terrain = BakedTerrain::new(spec).unwrap();
        for chunk in &scene.chunks {
            terrain
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &std::fs::read(folder.join(&chunk.height_file)).unwrap(),
                )
                .unwrap();
        }
        terrain.require_complete().unwrap();
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            spec,
            &manifest.style,
        );
        let access = bake_access_surfaces(&build_access_graph(&scene.entities, &terrain, water), &terrain)
            .unwrap_or_default();
        let plan = scene.landcover_plan.as_ref().unwrap().clone();
        let profile = LandUseProfile::from_style(&manifest.style).expect("land use profile");
        let published = scene.landcover_surfaces.as_ref().unwrap().clone();
        let published_agriculture = (
            published.agriculture.clone(),
            published
                .patches
                .iter()
                .filter(|patch| {
                    matches!(
                        patch.material,
                        Material::CultivatedSoil | Material::FieldHeadland | Material::FarmTrack
                    )
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let mut runs = vec![];
        for (name, pattern) in [
            ("published", None),
            ("sharedEnvelope", Some(pattern((24, 24), None, true))),
            (
                "contourElongated",
                Some(pattern((24, 24), Some(CONTOUR_ELONGATED), true)),
            ),
        ] {
            let mut config = plan.clone();
            config.farm_pattern = pattern;
            let started = std::time::Instant::now();
            let (realization, patches) =
                realize(spec, &config, &terrain, water, &scene.entities, &access).unwrap();
            let seconds = started.elapsed().as_secs_f64();
            verify_field_shapes(&realization, &config, &terrain, water, &scene.entities)
                .expect("published contour fields must satisfy the independent shape audit");
            if name == "published" {
                // The control: recomputing the published planet must reproduce
                // the stored agriculture bytes exactly.
                assert_eq!(
                    blake3::hash(&serde_json::to_vec(&(realization.clone(), patches.clone())).unwrap())
                        .to_hex()
                        .to_string(),
                    blake3::hash(&serde_json::to_vec(&published_agriculture).unwrap())
                        .to_hex()
                        .to_string(),
                    "the published world no longer reproduces its own agriculture"
                );
            }
            {
                let surfaces = scene.landcover_surfaces.as_mut().unwrap();
                surfaces.agriculture = realization.clone();
                surfaces.patches.retain(|patch| {
                    !matches!(
                        patch.material,
                        Material::CultivatedSoil | Material::FieldHeadland | Material::FarmTrack
                    )
                });
                surfaces.patches.extend(patches.iter().cloned());
            }
            let ledger = landuse_ledger::analyze(&scene, &profile).expect("ledger");
            let rural = &ledger.by_purpose["rural"];
            let mut reasons = BTreeMap::<String, usize>::new();
            for rejection in &realization.rejected_fields {
                *reasons.entry(rejection.reason.clone()).or_default() += 1;
            }
            let area = |material: Material| {
                patches
                    .iter()
                    .filter(|patch| patch.material == material)
                    .map(|patch| patch.area_m2)
                    .sum::<f64>()
            };
            let strips = realization
                .fields
                .iter()
                .filter(|field| field.contour_shape.is_some())
                .count();
            runs.push(serde_json::json!({
                "config": name,
                "policy": realization.field_shapes,
                "fields": realization.fields.len(),
                "strips": strips,
                "rejections": realization.rejected_fields.len(),
                "reasons": reasons,
                "cultivatedM2": area(Material::CultivatedSoil),
                "headlandM2": area(Material::FieldHeadland),
                "farmTrackM2": area(Material::FarmTrack),
                "ruralRealizedM2": rural.realized_m2,
                "ruralPlannedM2": rural.planned_m2,
                "ruralUnresolvedM2": rural.unresolved_m2,
                "verifySeconds": seconds,
                "contourFields": realization
                    .fields
                    .iter()
                    .filter(|field| field.contour_shape.is_some())
                    .map(|field| [
                        field.boundary.width_m as i64,
                        field.boundary.depth_m as i64,
                        field.contour_shape.as_ref().unwrap().long_side_m as i64,
                        field.contour_shape.as_ref().unwrap().short_side_m as i64,
                    ])
                    .collect::<Vec<_>>(),
            }));
            if name == "sharedEnvelope" {
                // Every field the pre-existing rule accepts survives verbatim;
                // the strategy may only add.
                for field in &realization.fields {
                    assert!(field.contour_shape.is_none());
                }
            }
        }
        // Monotonicity: the contour run must contain every shared-envelope field
        // with identical geometry and access.
        let mut config = plan.clone();
        config.farm_pattern = Some(pattern((24, 24), None, true));
        let (envelope, _) = realize(spec, &config, &terrain, water, &scene.entities, &access).unwrap();
        config.farm_pattern = Some(pattern((24, 24), Some(CONTOUR_ELONGATED), true));
        let (contour, _) = realize(spec, &config, &terrain, water, &scene.entities, &access).unwrap();
        for field in &envelope.fields {
            let kept = contour
                .fields
                .iter()
                .find(|other| other.id == field.id)
                .expect("the contour rule must not lose a square field");
            assert_eq!(kept.boundary, field.boundary);
            assert_eq!(kept.cultivated, field.cultivated);
            assert_eq!(kept.access_path, field.access_path);
            assert_eq!(kept.access_method, field.access_method);
            assert_eq!(kept.shared_track, field.shared_track);
            assert_eq!(kept.contour_shape, None);
        }
        assert!(contour.fields.len() >= envelope.fields.len());
        // Determinism on the same input.
        let (again, _) = realize(spec, &config, &terrain, water, &scene.entities, &access).unwrap();
        assert_eq!(
            serde_json::to_vec(&contour).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        let report = serde_json::json!({
            "format": "glyphweave.contour-elongated-field-audit",
            "world": root,
            "sceneId": spec.scene_id,
            "sizeM": [spec.width_m, spec.depth_m],
            "entities": scene.entities.len(),
            "runs": runs,
        });
        if let Some(path) = std::env::var_os("GLYPHWEAVE_AUDIT_REPORT") {
            let path = std::path::PathBuf::from(path);
            std::io::Write::write_all(
                &mut std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .expect("the audit report path must not already exist"),
                &serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    #[test]
    fn indexed_and_linear_complete_farm_outputs_are_identical() {
        let(s,p,t,w,e)=fixture();
        let access=vec![AccessSurface{version:1,entity_id:"pedestrian".into(),anchor_id:"front".into(),road_id:"road".into(),width_m:1.5,
            vertices:vec![[0.,0.,44.],[159.,0.,44.75],[0.,0.,45.5],[32.,0.,0.],[33.,0.,159.],[34.,0.,0.]],triangles:vec![[0,1,2],[3,5,4]]}];
        let index=super::super::pedestrian_index::PedestrianIndex::new(&access).unwrap();
        let reference=realize_with_query(&s,&p,&t,w,&e,JOIN_MIN_SAVING_M,|x,z|index.linear_reference(x,z)).unwrap();
        let optimized=realize(&s,&p,&t,w,&e,&access).unwrap();
        assert!(serde_json::to_vec(&reference).unwrap()==serde_json::to_vec(&optimized).unwrap());
        assert!(!optimized.0.fields.is_empty());assert!(!optimized.0.rejected_fields.is_empty());
    }
    #[test]
    #[ignore = "explicit full-world same-input indexed/reference benchmark; requires completed world and fresh report path"]
    fn benchmark_pedestrian_index_on_published_world() {
        use crate::worldgen::{WorldManifest,SceneIndex,build_access_graph,bake_access_surfaces};
        let root=std::path::PathBuf::from(std::env::var_os("GLYPHWEAVE_BENCH_WORLD").expect("set completed world path"));
        assert!(root.join("world.json").is_file(),"partial bake is not a published benchmark source");
        let report=std::path::PathBuf::from(std::env::var_os("GLYPHWEAVE_BENCH_REPORT").expect("set fresh report path"));
        assert!(!report.exists(),"keep earlier evidence");
        let setup=std::time::Instant::now();
        let manifest:WorldManifest=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(root.join("glyphweave.manifest.json")).unwrap())).unwrap();
        let spec=&manifest.scenes[0];let folder=root.join("scenes").join(&spec.scene_id);
        let scene:SceneIndex=serde_json::from_reader(std::io::BufReader::with_capacity(1024*1024,std::fs::File::open(folder.join("scene.json")).unwrap())).unwrap();
        let mut terrain=BakedTerrain::new(spec).unwrap();
        for c in &scene.chunks {terrain.insert_chunk(c.world_x,c.world_z,c.valid_width_m,c.valid_depth_m,&std::fs::read(folder.join(&c.height_file)).unwrap()).unwrap();}
        terrain.require_complete().unwrap();
        let water=water_geometry(super::super::super::worldgen::water_kind(&manifest.style),&manifest.landmarks,spec,&manifest.style);
        let graph=build_access_graph(&scene.entities,&terrain,water);
        let access=bake_access_surfaces(&graph,&terrain).unwrap();
        let plan=scene.landcover_plan.as_ref().unwrap();
        let index=super::super::pedestrian_index::PedestrianIndex::new(&access).unwrap();
        let setup_seconds=setup.elapsed().as_secs_f64();
        eprintln!("reference benchmark inputs loaded in {setup_seconds}s; starting linear agriculture");
        let start=std::time::Instant::now();
        let old=realize_with_query(spec,plan,&terrain,water,&scene.entities,JOIN_MIN_SAVING_M,|x,z|index.linear_reference(x,z)).unwrap();
        let reference_seconds=start.elapsed().as_secs_f64();
        eprintln!("linear agriculture complete in {reference_seconds}s; starting indexed agriculture");
        let start=std::time::Instant::now();let new=realize(spec,plan,&terrain,water,&scene.entities,&access).unwrap();let indexed_seconds=start.elapsed().as_secs_f64();
        let old_bytes=serde_json::to_vec(&old).unwrap();let new_bytes=serde_json::to_vec(&new).unwrap();
        let saved=scene.landcover_surfaces.as_ref().unwrap();
        let saved_patches:Vec<_>=saved.patches.iter().filter(|p|matches!(p.material,Material::CultivatedSoil|Material::FieldHeadland|Material::FarmTrack)).collect();
        let saved_bytes=serde_json::to_vec(&(&saved.agriculture,saved_patches)).unwrap();
        let result=serde_json::json!({"format":"glyphweave.agriculture-index-reference","status":if old_bytes==new_bytes&&new_bytes==saved_bytes {"pass"} else {"fail"},"scope":"same complete agriculture fields/rejections/routes/meshes; not full-world time SLA","world":root,"sceneId":spec.scene_id,"sizeM":[spec.width_m,spec.depth_m],"entities":scene.entities.len(),"pedestrianTriangles":access.iter().map(|s|s.triangles.len()).sum::<usize>(),"fields":new.0.fields.len(),"rejections":new.0.rejected_fields.len(),"patches":new.1.len(),"setupSeconds":setup_seconds,"referenceSeconds":reference_seconds,"indexedSeconds":indexed_seconds,"serializedBytes":new_bytes.len(),"referenceHash":blake3::hash(&old_bytes).to_hex().to_string(),"indexedHash":blake3::hash(&new_bytes).to_hex().to_string(),"publishedHash":blake3::hash(&saved_bytes).to_hex().to_string(),"naturalHash":scene.road_network_plan.as_ref().unwrap().natural_heightfield_hash,"finalTerrainHash":terrain.content_hash().unwrap()});
        std::io::Write::write_all(&mut std::fs::OpenOptions::new().write(true).create_new(true).open(report).unwrap(),&serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        eprintln!("{}",result);
        assert!(old_bytes==new_bytes,"indexed results differ from original linear predicate");
        assert!(new_bytes==saved_bytes,"published agriculture differs from reconstruction");
    }

}
