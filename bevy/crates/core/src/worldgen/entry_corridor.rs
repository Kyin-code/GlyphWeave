//! OPT-IN graded entry corridor: the walkable approach a roadside pad needs when
//! the DOOR and the STREET it is assigned to are separated by nothing but the
//! street's own fill slope.
//!
//! ## The defect this answers
//!
//! A pad without a `frontageRoadPoint` (every service pad, and every roadside pad
//! the programme plans on a level rather than a street station) publishes two
//! earthworks between its door and its assigned street: its own flat top with its
//! blended ring, and the street's graded band. The band is a hard, priority-10
//! rectangle at the road's authored surface, and a blend ring is a cubic
//! smoothstep whose maximum gradient is `1.5 x` its average. Where the street
//! stands above the lot - an embankment on a natural bank - the only surface
//! between the two is that transition, and the checked pedestrian route samples it
//! every 0.5 m against [`GRADE_CONTRACT`]. A roadbed fill spread over the band's
//! ring measures well over 25 % at its steepest point, so the route is refused
//! (`assigned_road_not_reachable_at_acceptance`), the pad is withdrawn, and the
//! service or the dwelling it would have carried is never realized.
//!
//! ## What this module adds (and what it does not)
//!
//! One straight, axis-aligned, FILL-ONLY ramp per pad, laid from the edge of the
//! pad's own `+-2 m` support window to the edge of the assigned street's graded
//! band, at a design grade of [`DESIGN_GRADE`] (`24 %`, deliberately below the
//! `25 %` contract so that the raster's millimetre quantization cannot push a
//! checked 0.5 m sample over the contract). The ramp REPLACES the pad's own fill
//! inside its footprint - it is the pad's own earthwork, re-graded - and it never
//! goes below the natural ground: the published surface of a corridor cell is
//! `max(natural, ramp)`, which is exactly what the ledger records and what the
//! audit recomputes from the plan and the natural raster.
//!
//! Nothing is relaxed. Every bound is the project's existing number, read from its
//! one definition (never copied):
//!
//!  * [`GRADE_CONTRACT`] = `access::WALKABLE_GRADE` (25 %) - the route contract,
//!    untouched; the corridor is DESIGNED strictly below it;
//!  * [`MAX_LENGTH_M`] = `access::MAX_LENGTH` (120 m) - the budget the checked
//!    route already has;
//!  * [`WIDTH_M`] = 2 m - the project's existing minimum reserved passage: the
//!    2 m-wide entrance corridor `services::entrance_corridor` reserves before a
//!    service pad is accepted (half extent 1). The checked route's own clearance
//!    is `access::HALF_WIDTH` = 0.75 m either side of the walked line, so a 2 m
//!    pavement holds the line and both its edges;
//!  * [`MAX_FILL_M`] = 6 m - the roadbed fill contract the pads are already
//!    measured against, and the zoning mask's own `fill_capacity_at` taper;
//!  * protected zoning, the water body and the shoreline public passage band are
//!    refused cell by cell, exactly as `published_surface_from_raster_core`
//!    refuses to lower them;
//!  * no cell of another entity's support surface or `+-2 m` audit window may be
//!    taken: the corridor is a notch through THIS pad's fill, never through a
//!    neighbour's platform or a roadbed.
//!
//! ## Off by default
//!
//! `style.terrainPolicy.entryCorridor = "graded"` is the switch. Absent (or any
//! other value) the policy is OFF, [`Policy::enabled`] is false, no corridor is
//! ever planned, the plan record, the entity grounding, the carve list and every
//! published byte are exactly the historical ones, and the acceptance gates stay
//! byte-for-byte the gates they were.
//!
//! ## Auditability
//!
//! The record ([`EntryCorridor`]) lives in the same pad record the plan publishes
//! (`RoadsideSite::entry_corridor`) and is carried into the pad's entity
//! `grounding.entryCorridor`, so the bake composes it. It publishes the length, the
//! two end points, both end levels, the steepest station-to-station grade, and the
//! per-cell `(x, z, natural, target, fill)` ledger. [`verify`] rebuilds every one
//! of those numbers from the record's own geometry and the natural raster, and
//! checks the published raster cell by cell, so the corridor is provable from
//! `plan + natural + published`, not from a second generator run.

use super::road_network::RoadsideSite;
use super::zoning::ZoningMask;
use super::*;
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};

/// The authored switch key, inside `style.terrainPolicy`.
pub const POLICY_KEY: &str = "entryCorridor";
/// The one value that enables the corridor.
pub const POLICY_VALUE: &str = "graded";
/// The named reason the policy is off when the key is absent.
pub const NOT_DECLARED: &str = "not_declared";
/// The named reason the policy is off when the key holds anything else.
pub const UNRECOGNISED: &str = "unrecognised_entry_corridor_value";
/// The record's own version.
pub const VERSION: u32 = 1;
/// The corridor's pavement width in cells (1 m per cell): the project's existing
/// minimum reserved passage (`services::entrance_corridor`, half extent 1).
pub const WIDTH_M: u32 = 2;
/// The side ring, in metres, of the ramp's own fill transition.
pub const RING_M: f64 = 2.0;
/// The grade the ramp is LAID at, as a rise over run. Strictly below
/// [`GRADE_CONTRACT`] so that a millimetre-quantized raster cannot push a checked
/// 0.5 m sample over the contract: `0.24 x 0.5 m = 120 mm <= 125 mm`.
pub const DESIGN_GRADE: f64 = 0.24;
/// The walkable grade the checked route allows. The project's one contract.
pub const GRADE_CONTRACT: f64 = super::access::WALKABLE_GRADE;
/// The checked route's own length budget, reused as the corridor's.
pub const MAX_LENGTH_M: f64 = super::access::MAX_LENGTH;
/// The roadbed fill contract every pad is already measured against.
pub const MAX_FILL_M: f64 = 6.0;
/// The pad's `+-2 m` support window, in metres. The corridor starts outside it, so
/// the pad's declared top keeps every cell the road audit measures.
pub const SUPPORT_WINDOW_M: i32 = 2;
/// The carve priority: above a building's own pad (1) and its apron (2), below a
/// road's band (10), so the ramp's claim is only ever applied outside a roadbed.
pub const PRIORITY: i32 = 3;

/// The authored policy of one world, read from `style.terrainPolicy.entryCorridor`.
///
/// It is also carried on the plan (`RoadNetworkPlan::entry_corridor_policy`), so
/// every pad generator reads ONE decision instead of re-parsing the style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    /// Absent or unrecognised: no corridor is planned and no byte moves.
    #[default]
    Off,
    /// `"graded"`: a pad whose checked entrance fails gets one graded ramp.
    Graded,
}

impl Policy {
    /// Read the policy. The second value is the named reason the policy is off,
    /// so a report can say WHY rather than "false".
    pub fn from_style(style: &serde_json::Value) -> (Self, Option<&'static str>) {
        let Some(value) = style
            .get("terrainPolicy")
            .and_then(|policy| policy.get(POLICY_KEY))
        else {
            return (Self::Off, Some(NOT_DECLARED));
        };
        match value.as_str() {
            Some(text) if text == POLICY_VALUE => (Self::Graded, None),
            _ => (Self::Off, Some(UNRECOGNISED)),
        }
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::Graded)
    }

    /// Serde helper: an OFF policy is never serialized, so a plan of a world that
    /// did not opt in keeps its exact bytes.
    pub fn is_off(&self) -> bool {
        !self.enabled()
    }

    /// Was the key declared at all (whatever its value)? A report publishes the
    /// authored value verbatim when it was, like the bounded-cut ledger does.
    pub fn declared(style: &serde_json::Value) -> bool {
        style
            .get("terrainPolicy")
            .and_then(|policy| policy.get(POLICY_KEY))
            .is_some()
    }

    /// The authored value, verbatim, for the report.
    pub fn requested(style: &serde_json::Value) -> Option<String> {
        style
            .get("terrainPolicy")
            .and_then(|policy| policy.get(POLICY_KEY))
            .map(|value| match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            })
    }
}

/// The axis a corridor runs along. The strip is two cells WIDE across it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Z,
}

/// One corridor cell: what the natural ground held, what the ramp publishes, and
/// the fill between the two. One cell is one square metre, so the sum of
/// `fill_mm` is the corridor's fill volume in litres (`/ 1000` = m3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorridorCell {
    pub x: i32,
    pub z: i32,
    pub natural_mm: i32,
    pub target_mm: i32,
    pub fill_mm: i32,
}

/// One pad's graded entry corridor, published in the pad record and carried into
/// the pad's entity grounding.
///
/// `axis` is the direction the ramp RUNS: the strip is two cells wide across that
/// axis and `stations + 1` cells long. `from` is the near (door-side) end on the
/// low lateral cell and `to` the far (street-side) end on the high one, so each
/// coordinate pair differs by the width across and the run along.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryCorridor {
    pub version: u32,
    pub pad_id: String,
    pub road_id: String,
    pub axis: Axis,
    pub from: [i32; 2],
    pub to: [i32; 2],
    pub width_m: u32,
    /// The run between the two end cells, in metres (one cell = one metre).
    pub length_m: f64,
    /// The pad's declared top at the near end, whole millimetres.
    pub start_mm: i32,
    /// The assigned street's published surface at the far end, whole millimetres.
    pub end_mm: i32,
    /// The steepest station-to-station grade of the PUBLISHED profile, in
    /// per-mille (the natural ground's own rises are included), positive = climbs
    /// toward the street.
    pub grade_permille: i32,
    pub cell_count: u32,
    pub fill_sum_mm: i64,
    pub max_fill_mm: i32,
    pub cells: Vec<CorridorCell>,
}

impl EntryCorridor {
    /// Does the ramp run along world x? (`false` = along world z.)
    pub fn along_x(&self) -> bool {
        self.axis == Axis::X
    }

    /// The number of one-metre stations between the two ends.
    pub fn stations(&self) -> i32 {
        if self.along_x() {
            (self.to[0] - self.from[0]).abs()
        } else {
            (self.to[1] - self.from[1]).abs()
        }
    }

    /// The two lateral cells the strip covers, low and high.
    pub fn across_range(&self) -> (i32, i32) {
        if self.along_x() {
            (self.from[1].min(self.to[1]), self.from[1].max(self.to[1]))
        } else {
            (self.from[0].min(self.to[0]), self.from[0].max(self.to[0]))
        }
    }

    /// The near (door-side) and far (street-side) coordinates along the axis.
    pub fn axis_range(&self) -> (i32, i32) {
        if self.along_x() {
            (self.from[0], self.to[0])
        } else {
            (self.from[1], self.to[1])
        }
    }

    /// The linear ramp at one station, whole millimetres. The ONE definition the
    /// planner, the carve and the audit read.
    pub fn linear_mm(&self, station: i32) -> i64 {
        ramp_mm(self.start_mm, self.end_mm, station, self.stations())
    }

    /// Is the cell inside the strip's pavement?
    pub fn holds(&self, x: i32, z: i32) -> bool {
        let (across_lo, across_hi) = self.across_range();
        let (near, far) = self.axis_range();
        let across = if self.along_x() { z } else { x };
        let along = if self.along_x() { x } else { z };
        across >= across_lo
            && across <= across_hi
            && along >= near.min(far)
            && along <= near.max(far)
    }

    /// The linear ramp (millimetres) at a cell of the strip, or `None` outside it.
    pub fn linear_at(&self, x: i32, z: i32) -> Option<i64> {
        if !self.holds(x, z) {
            return None;
        }
        let (near, far) = self.axis_range();
        let along = if self.along_x() { x } else { z };
        let station = (along - near) * (far - near).signum();
        Some(self.linear_mm(station))
    }

    /// The compact geometry the terrain carve reads.
    pub(crate) fn ramp(&self) -> EntryRamp {
        EntryRamp {
            axis_x: self.along_x(),
            from: [self.from[0] as f64, self.from[1] as f64],
            to: [self.to[0] as f64, self.to[1] as f64],
            half_width: f64::from(self.width_m) * 0.5,
            start_mm: self.start_mm,
            end_mm: self.end_mm,
        }
    }
}

/// The one ramp formula: `start + (end - start) * station / stations`, whole
/// millimetres, rounded once.
pub fn ramp_mm(start_mm: i32, end_mm: i32, station: i32, stations: i32) -> i64 {
    if stations <= 0 {
        return i64::from(start_mm);
    }
    let delta = f64::from(end_mm) - f64::from(start_mm);
    let t = f64::from(station) / f64::from(stations);
    (f64::from(start_mm) + (delta * t).round()) as i64
}

/// The ramp as the terrain composition sees it: a strip whose target height is a
/// function of one axis only. `from`/`to` are the strip's two end cells (in cell
/// coordinates) and the strip is `2 x half_width` cells wide across.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EntryRamp {
    /// The ramp runs along world x (`true`) or world z (`false`).
    pub axis_x: bool,
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub half_width: f64,
    pub start_mm: i32,
    pub end_mm: i32,
}

impl EntryRamp {
    fn along_x(&self) -> bool {
        self.axis_x
    }

    /// The ramp's run, in cells.
    pub(crate) fn stations(&self) -> f64 {
        if self.along_x() {
            (self.to[0] - self.from[0]).abs()
        } else {
            (self.to[1] - self.from[1]).abs()
        }
    }

    /// The distance along the ramp from its near end, in cells. Negative before
    /// the near end, greater than [`Self::stations`] past the far end.
    pub(crate) fn along(&self, x: f64, z: f64) -> f64 {
        if self.along_x() {
            (x - self.from[0]) * (self.to[0] - self.from[0]).signum()
        } else {
            (z - self.from[1]) * (self.to[1] - self.from[1]).signum()
        }
    }

    /// The distance across the ramp from its centre line, in cells. The strip
    /// covers two cell centres, so its line runs half a cell outside `from`.
    pub(crate) fn across(&self, x: f64, z: f64) -> f64 {
        if self.along_x() {
            (z - (self.from[1] + self.half_width - 0.5)).abs()
        } else {
            (x - (self.from[0] + self.half_width - 0.5)).abs()
        }
    }

    /// The ramp's target surface at one point, in metres. The continuous form of
    /// [`ramp_mm`], so a bilinear sample between two stations reads the
    /// interpolation of the same two targets the cells publish.
    pub(crate) fn target_m(&self, x: f64, z: f64) -> f64 {
        let stations = self.stations().max(1.);
        let along = self.along(x, z);
        (f64::from(self.start_mm)
            + (f64::from(self.end_mm) - f64::from(self.start_mm)) * (along / stations))
            / 1000.
    }

    /// Is this point inside the pavement (the ramp's flat top)?
    pub(crate) fn holds(&self, x: f64, z: f64) -> bool {
        let along = self.along(x, z);
        along >= -0.5 && along <= self.stations() + 0.5 && self.across(x, z) <= self.half_width
    }
}

/// One pad's request to the planner. Everything the corridor must respect.
pub struct Request<'a> {
    pub site: &'a RoadsideSite,
    pub road: &'a EntityInstance,
    pub neighbours: &'a [EntityInstance],
    pub natural: &'a BakedTerrain,
    pub mask: Option<&'a ZoningMask>,
    pub water: WaterGeometry,
    pub max_fill_m: f64,
}

/// The planner's verdict: a corridor, or the MEASURED reason none exists.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Corridor(EntryCorridor),
    Infeasible(String),
}

/// The pad's door cell and the unit direction its door faces, exactly as
/// `RoadsideSite::front_anchor` publishes them.
fn door_cell(site: &RoadsideSite) -> ([i32; 2], [i32; 2]) {
    let front = site.depth_m as i32 / 2 + 1;
    match site.rotation_y_deg as i32 {
        90 => ([site.world_x - front, site.world_z], [-1, 0]),
        180 => ([site.world_x, site.world_z + front], [0, 1]),
        270 => ([site.world_x + front, site.world_z], [1, 0]),
        _ => ([site.world_x, site.world_z - front], [0, -1]),
    }
}

/// The two whole-cell strips that hold the door's lateral cell. A 2 m pavement is
/// two cells and the door stands on a cell of its own, so exactly two placements
/// contain it; the planner tries the lower one first and keeps the first that
/// holds.
fn lateral_placements(lateral: i32) -> [[i32; 2]; 2] {
    [[lateral, lateral + 1], [lateral - 1, lateral]]
}

/// Plan one corridor for one pad whose checked entrance failed.
///
/// The geometry is fixed by the two endpoints: the first cell outside the pad's
/// `+-2 m` support window, and the first cell outside the assigned street's carve
/// rectangle (its graded band). There is no detour and no search: the corridor is
/// the checked route's own direct segment, re-graded, so the route that the gate
/// runs is the route the corridor was designed for.
pub fn plan(request: &Request<'_>) -> Result<Verdict, TerrainError> {
    let site = request.site;
    let road = request.road;
    let ([door_x, door_z], dir) = door_cell(site);
    let start_mm = (f64::from(site.entity().ground_height_m()) * 1000.).round() as i32;
    // The near end: the first cell outside the pad's own `+-2 m` support window.
    let near_x = door_x + dir[0] * SUPPORT_WINDOW_M;
    let near_z = door_z + dir[1] * SUPPORT_WINDOW_M;
    // The far end: the first cell outside the street's own carve rectangle. The
    // rect holds every cell whose centre stands within `ceil(half)` cells of the
    // street's axis (the composition's own `outside <= 0` test), so the first cell
    // it does NOT hold is `floor(half) + 1` away - which is exactly the band's edge,
    // the last walkable surface the street itself publishes.
    let (road_half_w, road_half_d) = super::carve_flat_top_half_extents(road);
    let along_z = dir[1] != 0;
    let road_axis = if along_z { road.world_z } else { road.world_x };
    let road_half = if along_z { road_half_d } else { road_half_w };
    let step = if along_z { dir[1] } else { dir[0] };
    // The pad stands on the street's other side, so the near cell is reached by
    // walking `-step` from the street's axis.
    let far_axis = road_axis - step * (road_half.floor() as i32 + 1);
    let near_axis = if along_z { near_z } else { near_x };
    let stations = (far_axis - near_axis).abs();
    if stations < 1 {
        return Ok(Verdict::Infeasible(format!(
            "the pad's {SUPPORT_WINDOW_M} m support window reaches {}'s graded band, so no cell \
             lies between its door and the street",
            road.entity_id
        )));
    }
    let far_x = if along_z { near_x } else { far_axis };
    let far_z = if along_z { far_axis } else { near_z };
    // The street's published surface at the far end: a profiled road's authored
    // elevation is what the bake publishes inside its band; a flat road publishes
    // its own level.
    let end_mm = match &road.grounding.road_profile {
        Some(profile) => profile
            .height_at(f64::from(far_x), f64::from(far_z))
            .map_or_else(
                || (f64::from(road.ground_height_m()) * 1000.).round() as i32,
                |h| (h * 1000.).round() as i32,
            ),
        None => (f64::from(road.ground_height_m()) * 1000.).round() as i32,
    };
    let length_m = f64::from(stations);
    if length_m > MAX_LENGTH_M {
        return Ok(Verdict::Infeasible(format!(
            "the run from its door to {} is {length_m:.0} m, over the {MAX_LENGTH_M:.0} m budget \
             the checked route uses",
            road.entity_id
        )));
    }
    let rise_m = (f64::from(end_mm) - f64::from(start_mm)).abs() / 1000.;
    let measured_grade = rise_m / length_m;
    if measured_grade > DESIGN_GRADE {
        let need = rise_m / DESIGN_GRADE;
        return Ok(Verdict::Infeasible(format!(
            "the {rise_m:.3} m between the pad's declared top ({:.3} m) and {}'s published surface \
             ({:.3} m) needs {need:.2} m of run at the 25% contract, but only {length_m:.0} m lie \
             between its {SUPPORT_WINDOW_M} m support window and the street's band (measured \
             minimum grade {:.1}%)",
            f64::from(start_mm) / 1000.,
            road.entity_id,
            f64::from(end_mm) / 1000.,
            measured_grade * 100.,
        )));
    }
    let lateral = if along_z { door_x } else { door_z };
    let axis = if along_z { Axis::Z } else { Axis::X };
    let mut first_reason: Option<String> = None;
    for columns in lateral_placements(lateral) {
        let corridor = if along_z {
            EntryCorridor {
                version: VERSION,
                pad_id: site.id.clone(),
                road_id: site.road_id.clone(),
                axis,
                from: [columns[0], near_z],
                to: [columns[1], far_z],
                width_m: WIDTH_M,
                length_m,
                start_mm,
                end_mm,
                grade_permille: 0,
                cell_count: 0,
                fill_sum_mm: 0,
                max_fill_mm: 0,
                cells: Vec::new(),
            }
        } else {
            EntryCorridor {
                version: VERSION,
                pad_id: site.id.clone(),
                road_id: site.road_id.clone(),
                axis,
                from: [near_x, columns[0]],
                to: [far_x, columns[1]],
                width_m: WIDTH_M,
                length_m,
                start_mm,
                end_mm,
                grade_permille: 0,
                cell_count: 0,
                fill_sum_mm: 0,
                max_fill_mm: 0,
                cells: Vec::new(),
            }
        };
        match evaluate(request, &corridor) {
            Ok(measured) => return Ok(Verdict::Corridor(measured)),
            Err(reason) => {
                if first_reason.is_none() {
                    first_reason = Some(reason);
                }
            }
        }
    }
    Ok(Verdict::Infeasible(first_reason.unwrap_or_else(|| {
        format!("no 2 m strip toward {} is free", road.entity_id)
    })))
}

/// Measure one candidate strip cell by cell, then return it with its ledger.
///
/// Every rule is a refusal BY NAME; nothing is clipped or clamped into place.
fn evaluate(request: &Request<'_>, corridor: &EntryCorridor) -> Result<EntryCorridor, String> {
    let stations = corridor.stations();
    let (across_lo, across_hi) = corridor.across_range();
    let (near, far) = corridor.axis_range();
    let sign = (far - near).signum();
    let mut cells = Vec::with_capacity(((stations + 1) * 2) as usize);
    let mut fill_sum = 0_i64;
    let mut max_fill = 0_i32;
    let mut profiles: Vec<Vec<i64>> = Vec::new();
    for across in across_lo..=across_hi {
        let mut profile = Vec::with_capacity((stations + 1) as usize);
        for station in 0..=stations {
            let along = near + sign * station;
            let (x, z) = if corridor.along_x() {
                (along, across)
            } else {
                (across, along)
            };
            if let Some(reason) = blocked(request, x, z) {
                return Err(reason);
            }
            let natural_mm = request
                .natural
                .height_millimetres(x, z)
                .ok_or_else(|| format!("missing natural sample ({x},{z})"))?;
            let linear = corridor.linear_mm(station);
            let target = linear.max(i64::from(natural_mm));
            let fill = target - i64::from(natural_mm);
            let cap_mm = fill_cap_mm(request.mask, request.max_fill_m, x, z);
            if fill > cap_mm {
                return Err(format!(
                    "it would place {:.2} m of fill at ({x},{z}), above the {:.2} m contract",
                    fill as f64 / 1000.,
                    cap_mm as f64 / 1000.,
                ));
            }
            profile.push(target);
            fill_sum += fill;
            max_fill = max_fill.max(fill as i32);
            cells.push(CorridorCell {
                x,
                z,
                natural_mm,
                target_mm: target as i32,
                fill_mm: fill as i32,
            });
        }
        profiles.push(profile);
    }
    // The grade of the PUBLISHED profile, column by column: the natural ground
    // under one lane may rise through the strip on its own. The contract is never
    // lowered and nothing is benched away; a natural rise this steep is refused.
    let mut worst_mm = 0_i64;
    for (index, profile) in profiles.iter().enumerate() {
        for (station, pair) in profile.windows(2).enumerate() {
            let rise = (pair[1] - pair[0]).abs();
            if f64::from(rise as i32) / 1000. > GRADE_CONTRACT + 1e-9 {
                let across = across_lo + index as i32;
                let along = near + sign * (station as i32 + 1);
                let (x, z) = if corridor.along_x() {
                    (along, across)
                } else {
                    (across, along)
                };
                return Err(format!(
                    "its own profile reaches {:.1}% at ({x},{z}): the natural ground rises through \
                     the strip faster than the 25% contract",
                    rise as f64 / 10.,
                ));
            }
            worst_mm = worst_mm.max(rise);
        }
    }
    Ok(EntryCorridor {
        grade_permille: worst_mm as i32,
        cell_count: cells.len() as u32,
        fill_sum_mm: fill_sum,
        max_fill_mm: max_fill,
        cells,
        ..corridor.clone()
    })
}

/// Why a cell may not be taken, or `None` when it may.
fn blocked(request: &Request<'_>, x: i32, z: i32) -> Option<String> {
    if let Some(mask) = request.mask {
        if mask.protected_at(x, z) {
            return Some(format!("reserved (protected) land at ({x},{z})"));
        }
    }
    if request.water.contains_point(f64::from(x), f64::from(z)) {
        return Some(format!("the water body at ({x},{z})"));
    }
    if request.water.shore_band_cell(x, z) {
        return Some(format!("the reserved shore band at ({x},{z})"));
    }
    // Another entity's own support surface, or the window its audit measures: the
    // corridor is a notch through THIS pad's fill only.
    let own = request.site.id.as_str();
    for other in request.neighbours {
        if other.entity_id == own {
            continue;
        }
        // Another pad's graded corridor is territory too: two ramps may not claim
        // the same cell, or each would audit the other's surface.
        if let Some(corridor) = &other.grounding.entry_corridor {
            if corridor.holds(x, z) {
                return Some(format!(
                    "the entry corridor of {} at ({x},{z})",
                    other.entity_id
                ));
            }
        }
        let (half_w, half_d) = if super::carves_as_road(other.kind.as_str()) {
            // A road owns its graded band: the corridor stops at the band's edge
            // and never claims a cell of it.
            super::carve_flat_top_half_extents(other)
        } else {
            let bounds = super::frontage::bounds(other, i64::from(SUPPORT_WINDOW_M));
            let half_w = (bounds[2] - bounds[0]) as f64 * 0.5;
            let half_d = (bounds[3] - bounds[1]) as f64 * 0.5;
            (half_w, half_d)
        };
        if (f64::from(x) - f64::from(other.world_x)).abs() <= half_w + 1e-9
            && (f64::from(z) - f64::from(other.world_z)).abs() <= half_d + 1e-9
        {
            return Some(format!(
                "the {} support surface of {} at ({x},{z})",
                other.kind, other.entity_id
            ));
        }
    }
    None
}

/// The fill one corridor cell may place: the roadbed contract, narrowed by the
/// zoning mask's own taper away from protected land.
fn fill_cap_mm(mask: Option<&ZoningMask>, max_fill_m: f64, x: i32, z: i32) -> i64 {
    let contract = (max_fill_m.min(MAX_FILL_M) * 1000.).round() as i64;
    match mask {
        Some(mask) => contract.min((mask.fill_capacity_at(x, z) * 1000.).floor() as i64),
        None => contract,
    }
}

/// The entrance contract, with ONE graded corridor attempted before a pad is
/// refused.
///
/// The gate itself is unchanged: an existing walkable route is never re-graded,
/// and the corridor is accepted only when the SAME gate - the same search, the
/// same 120 m budget, the same 25 % grade - passes on the surface the corridor
/// makes the bake publish. A corridor that cannot open the entrance is withdrawn
/// again and the refusal quotes the gate's own words plus the measured reason.
#[allow(clippy::too_many_arguments)]
pub fn gate_or_corridor(
    site: &mut RoadsideSite,
    roads: &[EntityInstance],
    neighbours: &[EntityInstance],
    natural: &BakedTerrain,
    mask: Option<&ZoningMask>,
    cut_mm: i32,
    water: WaterGeometry,
    policy: Policy,
    max_fill_m: f32,
) -> Result<Option<(String, String)>, TerrainError> {
    let refused = super::road_network::assigned_road_gate(
        site, roads, neighbours, natural, mask, cut_mm, water,
    )?;
    let Some((category, reason)) = refused else {
        return Ok(None);
    };
    if !policy.enabled() {
        return Ok(Some((category.into(), reason)));
    }
    let Some(road) = roads.iter().find(|road| road.entity_id == site.road_id) else {
        return Ok(Some((category.into(), reason)));
    };
    let request = Request {
        site,
        road,
        neighbours,
        natural,
        mask,
        water,
        max_fill_m: f64::from(max_fill_m),
    };
    match plan(&request)? {
        Verdict::Infeasible(measured) => Ok(Some((
            category.into(),
            format!("{reason}; a graded entry corridor could not open it: {measured}"),
        ))),
        Verdict::Corridor(corridor) => {
            let reached = format!(
                "{} m corridor from ({},{}) to ({},{}) at {:.1}%",
                corridor.length_m,
                corridor.from[0],
                corridor.from[1],
                corridor.to[0],
                corridor.to[1],
                f64::from(corridor.grade_permille) / 10.,
            );
            site.entry_corridor = Some(corridor);
            let again = super::road_network::assigned_road_gate(
                site, roads, neighbours, natural, mask, cut_mm, water,
            )?;
            match again {
                None => Ok(None),
                Some((category, reason)) => {
                    site.entry_corridor = None;
                    Ok(Some((
                        category.into(),
                        format!(
                            "{reason}; its graded entry corridor ({reached}) still does not reach its own street"
                        ),
                    )))
                }
            }
        }
    }
}

/// One corridor's independently recomputed verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorridorAudit {
    pub pad_id: String,
    pub road_id: String,
    pub length_m: f64,
    pub cells: u32,
    pub fill_sum_mm: i64,
    pub max_fill_mm: i32,
    pub max_grade_permille: i32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
}

/// What the corridors of one scene add up to, recomputed from the plan and the two
/// published rasters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub status: String,
    pub evidence: String,
    pub grade_contract: f64,
    pub length_budget_m: f64,
    pub corridor_count: u32,
    pub cells: u64,
    pub fill_sum_mm: i64,
    pub max_fill_mm: i32,
    pub max_grade_permille: i32,
    pub longest_m: f64,
    pub corridors: Vec<CorridorAudit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
}

/// Recompute every published corridor from the plan's own records, the natural
/// raster and the published raster, cell for cell and millimetre for millimetre.
///
/// The record is the SUBJECT, not the evidence: the geometry is re-derived from
/// `from`/`to`/`startMm`/`endMm`, the natural ground is read from the natural
/// payload, the expected surface is `max(natural, ramp)`, and every recorded cell
/// and total must equal that recomputation. Nothing here calls the planner.
#[allow(clippy::too_many_arguments)]
pub fn verify(
    sites: &[RoadsideSite],
    natural: &BakedTerrain,
    published: &BakedTerrain,
    scene: &SceneSpec,
    zones: Option<&[LandUseZone]>,
    water: WaterGeometry,
    max_fill_m: f32,
) -> Audit {
    let mask = zones
        .map(|zones| ZoningMask::new(scene, zones))
        .transpose()
        .ok()
        .flatten();
    let mask = mask.as_ref();
    let mut audit = Audit {
        status: "pass".into(),
        evidence: "plan-record + natural-raster + published-raster".into(),
        grade_contract: GRADE_CONTRACT,
        length_budget_m: MAX_LENGTH_M,
        corridor_count: 0,
        cells: 0,
        fill_sum_mm: 0,
        max_fill_mm: 0,
        max_grade_permille: 0,
        longest_m: 0.,
        corridors: Vec::new(),
        failures: Vec::new(),
    };
    for site in sites {
        let Some(corridor) = &site.entry_corridor else {
            continue;
        };
        let mut failures = Vec::new();
        let mut cells = 0_u32;
        let mut fill_sum = 0_i64;
        let mut max_fill = 0_i32;
        let mut max_grade = 0_i64;
        if corridor.version != VERSION {
            failures.push(format!(
                "{}: corridor version {}",
                corridor.pad_id, corridor.version
            ));
        }
        if corridor.pad_id != site.id {
            failures.push(format!(
                "corridor names pad {} but is published on {}",
                corridor.pad_id, site.id
            ));
        }
        if corridor.road_id != site.road_id {
            failures.push(format!(
                "{}: corridor names road {} but the pad fronts {}",
                corridor.pad_id, corridor.road_id, site.road_id
            ));
        }
        let stations = corridor.stations();
        let (across_lo, across_hi) = corridor.across_range();
        let (near, far) = corridor.axis_range();
        let width = across_hi - across_lo + 1;
        if width != WIDTH_M as i32 {
            failures.push(format!(
                "{}: corridor is {width} cells across, not the {WIDTH_M} m passage",
                corridor.pad_id
            ));
        }
        if (corridor.length_m - f64::from(stations)).abs() > 1e-6 {
            failures.push(format!(
                "{}: recorded length {} m but the end points are {stations} m apart",
                corridor.pad_id, corridor.length_m
            ));
        }
        if corridor.length_m > MAX_LENGTH_M + 1e-9 {
            failures.push(format!(
                "{}: corridor is {:.1} m long, over the {MAX_LENGTH_M:.0} m budget",
                corridor.pad_id, corridor.length_m
            ));
        }
        if stations < 1 {
            failures.push(format!("{}: corridor has no run", corridor.pad_id));
        }
        let sign = (far - near).signum();
        let mut profiles: Vec<Vec<i64>> = Vec::new();
        for across in across_lo..=across_hi {
            let mut profile = Vec::new();
            for station in 0..=stations {
                let along = near + sign * station;
                let (x, z) = if corridor.along_x() {
                    (along, across)
                } else {
                    (across, along)
                };
                let Some(natural_mm) = natural.height_millimetres(x, z) else {
                    failures.push(format!("{}: no natural sample ({x},{z})", corridor.pad_id));
                    continue;
                };
                let linear = corridor.linear_mm(station);
                let target = linear.max(i64::from(natural_mm));
                let fill = target - i64::from(natural_mm);
                profile.push(target);
                cells += 1;
                fill_sum += fill;
                max_fill = max_fill.max(fill as i32);
                if fill < 0 {
                    failures.push(format!(
                        "{}: corridor EXCAVATES ({x},{z}) by {} mm",
                        corridor.pad_id, -fill
                    ));
                }
                let cap = fill_cap_mm(mask, f64::from(max_fill_m), x, z);
                if fill > cap {
                    failures.push(format!(
                        "{}: {:.2} m of fill at ({x},{z}), above the {:.2} m contract",
                        corridor.pad_id,
                        fill as f64 / 1000.,
                        cap as f64 / 1000.
                    ));
                }
                if let Some(mask) = mask {
                    if mask.protected_at(x, z) {
                        failures.push(format!(
                            "{}: corridor enters protected land at ({x},{z})",
                            corridor.pad_id
                        ));
                    }
                }
                if water.contains_point(f64::from(x), f64::from(z)) {
                    failures.push(format!(
                        "{}: corridor enters the water body at ({x},{z})",
                        corridor.pad_id
                    ));
                }
                if water.shore_band_cell(x, z) {
                    failures.push(format!(
                        "{}: corridor enters the shore band at ({x},{z})",
                        corridor.pad_id
                    ));
                }
                match published.height_millimetres(x, z) {
                    Some(actual) if i64::from(actual) != target => failures.push(format!(
                        "{}: published ({x},{z}) is {actual} mm, the corridor says {target} mm",
                        corridor.pad_id
                    )),
                    None => failures.push(format!(
                        "{}: no published sample ({x},{z})",
                        corridor.pad_id
                    )),
                    _ => {}
                }
                match corridor
                    .cells
                    .iter()
                    .find(|cell| cell.x == x && cell.z == z)
                {
                    Some(cell) => {
                        if i64::from(cell.natural_mm) != i64::from(natural_mm)
                            || i64::from(cell.target_mm) != target
                            || i64::from(cell.fill_mm) != fill
                        {
                            failures.push(format!(
                                "{}: cell ({x},{z}) records ({},{},{}), recomputed ({natural_mm},{target},{fill})",
                                corridor.pad_id, cell.natural_mm, cell.target_mm, cell.fill_mm
                            ));
                        }
                    }
                    None => failures.push(format!(
                        "{}: cell ({x},{z}) is not in the record",
                        corridor.pad_id
                    )),
                }
            }
            profiles.push(profile);
        }
        for profile in &profiles {
            for pair in profile.windows(2) {
                let rise = (pair[1] - pair[0]).abs();
                max_grade = max_grade.max(rise);
                if f64::from(rise as i32) / 1000. > GRADE_CONTRACT + 1e-9 {
                    failures.push(format!(
                        "{}: station-to-station grade {:.1}% above the {:.0}% contract",
                        corridor.pad_id,
                        rise as f64 / 10.,
                        GRADE_CONTRACT * 100.
                    ));
                }
            }
        }
        if cells != corridor.cell_count {
            failures.push(format!(
                "{}: record counts {} cells, recomputed {cells}",
                corridor.pad_id, corridor.cell_count
            ));
        }
        if fill_sum != corridor.fill_sum_mm {
            failures.push(format!(
                "{}: record sums {} mm of fill, recomputed {fill_sum}",
                corridor.pad_id, corridor.fill_sum_mm
            ));
        }
        if max_fill != corridor.max_fill_mm {
            failures.push(format!(
                "{}: record's deepest fill {} mm, recomputed {max_fill} mm",
                corridor.pad_id, corridor.max_fill_mm
            ));
        }
        if max_grade as i32 != corridor.grade_permille {
            failures.push(format!(
                "{}: record's steepest grade {} per-mille, recomputed {max_grade}",
                corridor.pad_id, corridor.grade_permille
            ));
        }
        audit.cells += u64::from(cells);
        audit.fill_sum_mm += fill_sum;
        audit.max_fill_mm = audit.max_fill_mm.max(max_fill);
        audit.max_grade_permille = audit.max_grade_permille.max(max_grade as i32);
        audit.longest_m = audit.longest_m.max(corridor.length_m);
        audit.corridors.push(CorridorAudit {
            pad_id: corridor.pad_id.clone(),
            road_id: corridor.road_id.clone(),
            length_m: corridor.length_m,
            cells,
            fill_sum_mm: fill_sum,
            max_fill_mm: max_fill,
            max_grade_permille: max_grade as i32,
            status: if failures.is_empty() {
                "pass".into()
            } else {
                "fail".into()
            },
            failures: failures.clone(),
        });
        audit.corridor_count += 1;
        for failure in failures {
            audit.failures.push(failure);
        }
    }
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    audit
}

#[cfg(test)]
#[path = "entry_corridor_tests.rs"]
mod tests;
