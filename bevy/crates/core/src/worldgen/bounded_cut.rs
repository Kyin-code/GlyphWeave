//! Bounded cut: the OPT-IN earthwork capability that lets a frontage pad
//! excavate, and nothing else.
//!
//! `style.frontageProgram.terrainPolicy = "bounded-cut"` plus an integer
//! `maxCutMm` turns it on. Anything else - a missing key, another value, a
//! non-integer or a cap above [`MAX_CUT_MM`] - leaves it OFF and records why by
//! name ([`NotEnabled`]), so an input that asked for something the project does
//! not have is refused in writing instead of being silently approximated. Off is
//! the default, and off is byte-identical: no entity gains `boundedCutMm`, no
//! carve gains a cut cap, no artifact gains a `boundedCut` section.
//!
//! What may be cut
//! ---------------
//! Two engineered supports, and nothing else:
//!
//! * the FRONTAGE PAD: the entity `frontage::append` accepted for a
//!   `style.frontageProgram` request, its entrance apron and its frontage-point
//!   apron (`worldgen::plan_terrain_carves` marks those three carves with the
//!   entity's own cap);
//! * the GRADED ROAD: a road that carries a longitudinal [`RoadProfile`], whose
//!   support band is the graded flat top plus the corridor's shoulder. A real
//!   hillside road is cut on its uphill side and filled on its downhill side, so
//!   the same cap lets the road's own band excavate where the natural ground
//!   stands above the authored profile ([`RoadCutFootprint`]). An unprofiled
//!   (flat) road has no authored longitudinal surface to cut to and keeps the
//!   historical fill-only contract.
//!
//! Roadside pads of the settlement / service / joint programmes, the shoreline
//! quay row, landmarks and every authored entity never carry a cap and therefore
//! never cut.
//!
//! What may never be cut, per cell
//! -------------------------------
//! * protected land (`zoning::ZoningMask::protected_at`),
//! * the structured water body (`WaterGeometry::contains_point`),
//! * the shoreline public passage band (`WaterGeometry::shore_band_cell`),
//! * a cell inside another entity's own support window: a road's excavation
//!   stops at the edge of every pad's carve rectangle, every pad's cut
//!   footprint and every other road's graded band, so no road cut can be booked
//!   against an entity that owns that cell. The pads already state the same rule
//!   from their side ([`CellGuards`]), and
//! * a cell inside another accepted pad's [`CutFootprint`]: two benches may not
//!   share a footprint, because the ledger books every pad's excavation from the
//!   same cells and an overlap would count one cell twice. A road's ring is
//!   clipped by the same windows, so the road and the pad records are disjoint
//!   too.
//!
//! The generator guards those cells one by one before a pad is accepted
//! ([`CellGuards`]) and the audit re-derives them from the published bytes
//! ([`verify`]), so neither side can be satisfied by the other's promise.
//!
//! The ONE cap
//! -----------
//! [`MAX_CUT_MM`] is read by the style parser, by `frontage::support_check`, by
//! `worldgen::apply_terrain_carves` (through [`clamp_cap`]) and by this module's
//! audit. There is no second number - and a road reads the very same constant.

use super::road_network::RoadsideSite;
use super::road_profile::RoadProfile;
use super::zoning::ZoningMask;
use super::{EntityInstance, WaterGeometry, spatial_half_extents, support_pad_half_extents};
use crate::baked_terrain::BakedTerrain;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The authored switch key inside `style.frontageProgram`.
pub const POLICY_KEY: &str = "terrainPolicy";
/// The one authored value that turns the capability on.
pub const POLICY_VALUE: &str = "bounded-cut";
/// The authored cap key inside `style.frontageProgram`.
pub const MAX_CUT_KEY: &str = "maxCutMm";

/// THE hard cap, in whole millimetres. The style parser refuses a larger
/// declaration, `support_check` refuses a deeper cell, `apply_terrain_carves`
/// can never lower the ground further than this below the natural surface, and
/// the audit refuses a pad whose record claims more. One number, four readers.
pub const MAX_CUT_MM: i32 = 2_000;

/// The steepest face the cut's own taper may create, as a rise over run. This is
/// the WALKABLE grade, the loosest of the project's three slope contracts (8 %
/// storefront, 12 % building, 25 % walkable/yard), so a bounded-cut ring can
/// never be steeper than any use it passes through. Also the ring's width: a cut
/// of `d` metres needs `d / 0.25` metres of run.
pub const MAX_CUT_RING_GRADE: f64 = super::access::WALKABLE_GRADE;

/// Whole-millimetre tolerance for comparing a published raster sample against
/// the cut formula (metre source -> millimetre payload).
pub const CUT_MM_TOLERANCE: i64 = 1;

/// Named "not enabled" records. A style that asked for a cut gets one of these
/// instead of a silent downgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEnabled {
    /// The key is absent: the world never asked, which is not a defect.
    NotDeclared,
    /// The key is present but not the one accepted string.
    Unrecognised,
    /// `"bounded-cut"` without `maxCutMm`.
    CapMissing,
    /// `maxCutMm` is not a JSON integer.
    CapNotInteger,
    /// `maxCutMm` is zero or negative.
    CapOutOfRange,
    /// `maxCutMm` exceeds [`MAX_CUT_MM`].
    CapAboveMaximum,
}

impl NotEnabled {
    /// The published, machine-readable spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDeclared => "terrain-policy-not-declared",
            Self::Unrecognised => "terrain-policy-unrecognised",
            Self::CapMissing => "max-cut-mm-missing",
            Self::CapNotInteger => "max-cut-mm-not-an-integer",
            Self::CapOutOfRange => "max-cut-mm-out-of-range",
            Self::CapAboveMaximum => "max-cut-mm-above-maximum",
        }
    }
}

/// The typed policy of one world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutPolicy {
    /// The historical, fill-only earthwork contract.
    #[default]
    Disabled,
    /// Frontage pads may cut, at most `max_cut_mm` per cell.
    Bounded { max_cut_mm: i32 },
}

impl CutPolicy {
    /// Read the policy from the authored style, never trusting the value: only
    /// `"bounded-cut"` with an integer `maxCutMm` in `1..=MAX_CUT_MM` is on.
    pub fn from_style(style: &serde_json::Value) -> (Self, Option<NotEnabled>) {
        let Some(program) = style.get("frontageProgram") else {
            return (Self::Disabled, Some(NotEnabled::NotDeclared));
        };
        let Some(declared) = program.get(POLICY_KEY) else {
            return (Self::Disabled, Some(NotEnabled::NotDeclared));
        };
        if declared.as_str() != Some(POLICY_VALUE) {
            return (Self::Disabled, Some(NotEnabled::Unrecognised));
        }
        let Some(cap) = program.get(MAX_CUT_KEY) else {
            return (Self::Disabled, Some(NotEnabled::CapMissing));
        };
        let Some(cap) = cap.as_i64() else {
            return (Self::Disabled, Some(NotEnabled::CapNotInteger));
        };
        if cap <= 0 {
            return (Self::Disabled, Some(NotEnabled::CapOutOfRange));
        }
        if cap > i64::from(MAX_CUT_MM) {
            return (Self::Disabled, Some(NotEnabled::CapAboveMaximum));
        }
        (
            Self::Bounded {
                max_cut_mm: cap as i32,
            },
            None,
        )
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::Bounded { .. })
    }

    /// The cap in whole millimetres; `0` when the policy is off, which is what
    /// makes every cut formula below the identity.
    pub fn cap_mm(self) -> i32 {
        match self {
            Self::Disabled => 0,
            Self::Bounded { max_cut_mm } => clamp_cap(max_cut_mm),
        }
    }

    /// Is the authored key present at all? A declared-but-not-enabled style still
    /// publishes its named refusal, a style that never asked publishes nothing.
    pub fn declared(style: &serde_json::Value) -> bool {
        style
            .get("frontageProgram")
            .is_some_and(|program| program.get(POLICY_KEY).is_some())
    }
}

/// Clamp a declared cap into the legal range. THE single place the cap is
/// applied, so a forged record can never buy a deeper cut than [`MAX_CUT_MM`].
pub fn clamp_cap(cap_mm: i32) -> i32 {
    cap_mm.clamp(0, MAX_CUT_MM)
}

/// The cut surface of one cell, in metres.
///
/// `base_m` is the natural surface, `target_m` the pad's design level and
/// `outside_m` the cell's distance outside the pad's flat top (<= 0 inside).
///
/// * inside the flat top the surface is the design level itself, clamped so the
///   cut never removes more than `cap_mm` below the natural ground;
/// * outside it a ring rises from THAT flat-top surface - not from the design
///   level - at [`MAX_CUT_RING_GRADE`] until it meets the natural surface. Rising
///   from the design level instead leaves the difference between the flat top and
///   the cap standing as a wall at the ring's outer edge whenever the ground is
///   benched deeper than one whole cap, which is exactly the step the audit
///   refuses. The ring is therefore at most `cap / 0.25` metres wide, which is what
///   [`ring_width_m`] and [`CutFootprint`] already assume;
/// * the result is never below the natural surface minus the cap, and never
///   above the natural surface.
///
/// Where the bench is no deeper than the cap the two readings agree to the last
/// bit, so only the deep-bench case - a slope benched by more than the whole cap,
/// which the fill-only formula could never have expressed at all - moves.
pub fn cut_surface_m(base_m: f64, target_m: f64, outside_m: f64, cap_mm: i32) -> f64 {
    let cap_mm = clamp_cap(cap_mm);
    if cap_mm == 0 {
        return base_m;
    }
    let cap_m = f64::from(cap_mm) / 1000.;
    let loft = base_m - cap_m;
    // The flat top's own surface: the design level, but never deeper than the cap.
    let top = target_m.max(loft).min(base_m);
    if outside_m <= 0. {
        return top;
    }
    let depth = (base_m - top).max(0.);
    let ring = depth / MAX_CUT_RING_GRADE;
    if outside_m >= ring {
        return base_m;
    }
    (top + MAX_CUT_RING_GRADE * outside_m).min(base_m).max(loft)
}

/// The depth, in whole millimetres, that the cut removes at one cell.
pub fn cut_depth_mm(base_m: f64, target_m: f64, outside_m: f64, cap_mm: i32) -> i64 {
    let surface = cut_surface_m(base_m, target_m, outside_m, cap_mm);
    ((base_m - surface) * 1000.).round().max(0.) as i64
}

/// The width of the graded ring one cap needs, in metres.
pub fn ring_width_m(cap_mm: i32) -> f64 {
    f64::from(clamp_cap(cap_mm)) / 1000. / MAX_CUT_RING_GRADE
}

/// The surface a cap-carrying pad's own PLATFORM stands on, in metres.
///
/// This is the pad's declared flat top as the cut path already derives it: the
/// design level, EXCEPT where one whole cap cannot reach it, in which case the
/// bench stops at the deepest surface the cap permits
/// ([`cut_surface_m`] at `outside <= 0`). It is not a second formula - it is
/// `max(target, cut_surface_m(base, target, 0, cap))`, the same two numbers the
/// ledger, the encoder and the audit already hold - and it is the one answer the
/// platform rule in `worldgen::apply_terrain_carves` writes inside a pad's flat
/// top.
///
/// The value is always `>= cut_surface_m(base, target, 0, cap)` and always
/// `>= target` when the ground stands below the design level, so a platform is
/// never lower than either the bench or the fill the pad asks for.
pub fn platform_surface_m(base_m: f64, target_m: f64, cap_mm: i32) -> f64 {
    target_m.max(cut_surface_m(base_m, target_m, 0., cap_mm))
}

/// The cells of one pad's DECLARED FLAT TOP: the window the declared millimetre
/// level is an obligation over, cell for cell.
///
/// One derivation, read by the encoder-side platform rule, by the completion
/// judgement and by [`verify`]'s step 5, so "the cells the pad promises" cannot
/// mean two different rectangles. It is the rounded
/// [`worldgen::spatial_half_extents`] window `frontage::bounds(entity, 0)` uses,
/// in whole cells, which is what the published raster is sampled on.
pub fn flat_top_bounds(site: &RoadsideSite) -> [i32; 4] {
    let (half_w, half_d) = spatial_half_extents(
        site.width_m as f32,
        site.depth_m as f32,
        site.rotation_y_deg,
    );
    let (half_w, half_d) = (half_w.floor() as i32, half_d.floor() as i32);
    [
        site.world_x - half_w,
        site.world_z - half_d,
        site.world_x + half_w,
        site.world_z + half_d,
    ]
}

/// Does the published raster still hold this pad's declared top, cell for cell?
///
/// The audit's own verdict, in the audit's own window ([`flat_top_bounds`]): every
/// cell must publish exactly `site.support_level_mm`. A pad with no millimetre
/// declaration has no such obligation and answers `None`.
pub fn pad_top_holds(
    site: &RoadsideSite,
    height_millimetres: impl Fn(i32, i32) -> Option<i32>,
) -> Option<TopVerdict> {
    let level = site.support_level_mm?;
    let window = flat_top_bounds(site);
    let mut verdict = TopVerdict {
        level_mm: level,
        cells: 0,
        mismatched: 0,
        worst_cell: None,
        highest_mm: i32::MIN,
        lowest_mm: i32::MAX,
    };
    for z in window[1]..=window[3] {
        for x in window[0]..=window[2] {
            verdict.cells += 1;
            let actual = height_millimetres(x, z);
            let Some(actual) = actual else {
                verdict.mismatched += 1;
                verdict.worst_cell.get_or_insert((x, z, actual));
                continue;
            };
            verdict.highest_mm = verdict.highest_mm.max(actual);
            verdict.lowest_mm = verdict.lowest_mm.min(actual);
            if actual != level {
                verdict.mismatched += 1;
                let distance = actual.abs_diff(level);
                if verdict
                    .worst_cell
                    .is_none_or(|(_, _, known)| distance > known.unwrap_or(level).abs_diff(level))
                {
                    verdict.worst_cell = Some((x, z, Some(actual)));
                }
            }
        }
    }
    if verdict.highest_mm == i32::MIN {
        verdict.highest_mm = level;
    }
    if verdict.lowest_mm == i32::MAX {
        verdict.lowest_mm = level;
    }
    Some(verdict)
}

/// What one pad's declared flat top currently publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopVerdict {
    pub level_mm: i32,
    pub cells: u32,
    /// Cells that do not publish the declared level.
    pub mismatched: u32,
    /// The cell that deviates most, with the height it publishes.
    pub worst_cell: Option<(i32, i32, Option<i32>)>,
    /// The highest and lowest sample anywhere in the window, so a refusal can
    /// quote the lift it has to remove instead of a single cell.
    pub highest_mm: i32,
    pub lowest_mm: i32,
}

impl TopVerdict {
    pub fn holds(&self) -> bool {
        self.mismatched == 0
    }

    /// How far the highest cell of the window stands ABOVE the declared top, in
    /// whole millimetres (`0` when nothing does). This is the fill a declard
    /// platform would have to excavate away, and the number the one cap bounds.
    pub fn lift_mm(&self) -> i32 {
        (self.highest_mm - self.level_mm).max(0)
    }

    /// How far the lowest cell stands BELOW the declared top, in whole
    /// millimetres (`0` when nothing does). A fill cannot be rescued by a cut, so
    /// this is always a refusal when it is positive.
    pub fn shortfall_mm(&self) -> i32 {
        (self.level_mm - self.lowest_mm).max(0)
    }
}

/// One pad's bounded-cut footprint in world cells: the flat top it may cut plus
/// the graded ring around it. Both the generator's per-cell guard and the audit
/// walk exactly these cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutFootprint {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub center_x: f64,
    pub center_z: f64,
    pub half_w: f64,
    pub half_d: f64,
    pub target_m: f64,
    pub cap_mm: i32,
}

impl CutFootprint {
    /// The footprint of ONE frontage pad entity. The half extents are the same
    /// `support_pad_half_extents` the carve uses, so "the cells the ledger
    /// describes" and "the cells the raster carves" cannot drift.
    pub fn of(entity: &EntityInstance, cap_mm: i32) -> Self {
        let (half_w, half_d) = support_pad_half_extents(
            entity.width_m,
            entity.depth_m,
            entity.rotation_y_deg,
            entity.grounding.frontage_road_point.is_some(),
        );
        let ring = ring_width_m(cap_mm);
        let cx = f64::from(entity.world_x);
        let cz = f64::from(entity.world_z);
        Self {
            left: (cx - half_w - ring).floor() as i32,
            top: (cz - half_d - ring).floor() as i32,
            right: (cx + half_w + ring).ceil() as i32,
            bottom: (cz + half_d + ring).ceil() as i32,
            center_x: cx,
            center_z: cz,
            half_w,
            half_d,
            target_m: f64::from(entity.ground_height_m()),
            cap_mm: clamp_cap(cap_mm),
        }
    }

    /// The cell's distance outside the flat top, in metres (<= 0 inside).
    pub fn outside_m(&self, x: i32, z: i32) -> f64 {
        let dx = (f64::from(x) - self.center_x).abs() - self.half_w;
        let dz = (f64::from(z) - self.center_z).abs() - self.half_d;
        dx.max(dz)
    }

    /// The depth this footprint cuts at one cell, in whole millimetres.
    pub fn depth_mm(&self, base_m: f64, x: i32, z: i32) -> i64 {
        cut_depth_mm(base_m, self.target_m, self.outside_m(x, z), self.cap_mm)
    }

    /// True when the cell belongs to the footprint at all (inside or ring).
    pub fn contains(&self, x: i32, z: i32) -> bool {
        let outside = self.outside_m(x, z);
        outside <= 0. || outside < ring_width_m(self.cap_mm)
    }
}

/// The first cell two cut footprints SHARE, or `None` when the two sets are
/// disjoint.
///
/// This is the one predicate behind both the placement refusal and the audit:
/// "two benches share a footprint" means the same thing to the generator and to a
/// reader of the published plan. The bounding boxes are compared first and the
/// membership test is the footprint's own [`CutFootprint::contains`], so a ring
/// that merely touches a neighbour's bounding box is not a conflict.
pub fn footprint_intersection(a: &CutFootprint, b: &CutFootprint) -> Option<(i32, i32)> {
    let left = a.left.max(b.left);
    let right = a.right.min(b.right);
    let top = a.top.max(b.top);
    let bottom = a.bottom.min(b.bottom);
    if left > right || top > bottom {
        return None;
    }
    for z in top..=bottom {
        for x in left..=right {
            if a.contains(x, z) && b.contains(x, z) {
                return Some((x, z));
            }
        }
    }
    None
}

/// One accepted pad's cut footprint, with the id a refusal must name.
#[derive(Debug, Clone, Copy)]
pub struct AcceptedFootprint<'a> {
    pub pad_id: &'a str,
    pub footprint: CutFootprint,
}

/// The per-cell prohibitions. Returned reason is the audit's own spelling, so a
/// placement refusal and a published failure name the same defect.
pub const ON_PROTECTED: &str = "bounded_cut_on_protected_ground";
pub const IN_WATER: &str = "bounded_cut_in_water";
pub const ON_SHORE_BAND: &str = "bounded_cut_on_shore_band";
pub const ON_OCCUPIED: &str = "bounded_cut_on_occupied_cell";
pub const EXCEEDS_CAP: &str = "bounded_cut_exceeds_cap";
pub const OUTSIDE_PAD: &str = "bounded_cut_outside_pad";
pub const RING_TOO_STEEP: &str = "bounded_cut_ring_too_steep";
pub const PAD_TOP_NOT_FLAT: &str = "bounded_cut_pad_top_not_flat";
pub const CAP_ABOVE_MAXIMUM: &str = "bounded_cut_cap_above_maximum";
pub const LEDGER_MISMATCH: &str = "bounded_cut_ledger_mismatch";
pub const LEDGER_NOT_APPLIED: &str = "bounded_cut_ledger_not_applied";
pub const LEDGER_OVER_APPLIED: &str = "bounded_cut_ledger_over_applied";
pub const LEDGER_MISSING: &str = "bounded_cut_ledger_missing";
pub const LEDGER_WITHOUT_POLICY: &str = "bounded_cut_ledger_without_policy";
pub const UNKNOWN_PAD: &str = "bounded_cut_pad_not_in_plan";
pub const PAD_NOT_IN_LEDGER: &str = "bounded_cut_pad_missing_from_ledger";
/// Two pads' cut footprints share a cell (or may share one). The ledger books each
/// pad's excavation from those cells, so an overlap counts the shared cells twice
/// and the total stops being the scene's own excavation.
pub const FOOTPRINT_OVERLAP: &str = "bounded_cut_footprint_overlaps_another_pad";
/// A graded road whose own support band still stands more than its roadbed fill
/// contract above the natural ground after its bounded cut. The road is refused
/// BY NAME by `worldgen::road_support` and taken back out of the plan, with its
/// entity (and any roadside pad that named it), instead of publishing a roadbed
/// the 6 m contract cannot hold.
pub const ROAD_FILL_EXCEEDS_AFTER_CUT: &str = "road_fill_exceeds_contract_after_cut";
/// A published road cut record names a road the plan does not carry, or a road
/// without the graded profile the record's per-cell target needs.
pub const UNKNOWN_ROAD: &str = "bounded_cut_road_not_in_plan";
/// A graded road of the plan carries a cut cap but the ledger publishes no record
/// for it.
pub const ROAD_NOT_IN_LEDGER: &str = "bounded_cut_road_missing_from_ledger";

/// Everything one cell's refusal depends on.
pub struct CellGuards<'a> {
    pub mask: &'a ZoningMask,
    pub water: WaterGeometry,
    /// Footprint predicate of the already accepted entities.
    pub occupied: &'a dyn Fn(i32, i32) -> bool,
    /// Footprints of the already accepted CUT pads. A candidate that would cut may
    /// not share a single cell with any of them.
    pub cut_footprints: &'a [AcceptedFootprint<'a>],
}

impl CellGuards<'_> {
    /// Which named prohibition stops a cut at this cell, if any.
    pub fn blocks(&self, x: i32, z: i32) -> Option<&'static str> {
        if self.mask.protected_at(x, z) {
            return Some(ON_PROTECTED);
        }
        if self
            .water
            .contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5)
        {
            return Some(IN_WATER);
        }
        if self.water.shore_band_cell(x, z) {
            return Some(ON_SHORE_BAND);
        }
        if (self.occupied)(x, z) {
            return Some(ON_OCCUPIED);
        }
        None
    }

    /// The accepted cut footprint this candidate's own footprint intersects, with
    /// the accepted pad's id and the first shared cell.
    ///
    /// A footprint is the unit the ledger books and the audit re-derives, so the
    /// conflict is asked footprint to footprint: a candidate whose ring is clear of
    /// every accepted bench is legal even when its parcel stands next door.
    pub fn overlaps(&self, candidate: &CutFootprint) -> Option<(&str, i32, i32)> {
        self.cut_footprints.iter().find_map(|accepted| {
            footprint_intersection(candidate, &accepted.footprint)
                .map(|(x, z)| (accepted.pad_id, x, z))
        })
    }
}

/// Does one entity's published footprint cover this cell?
///
/// Rounded the way `frontage::bounds` rounds and `entity_occupies` rounds, so the
/// per-cell prohibition, the placement preflight and the audit agree on the window.
pub fn entity_covers(entity: &EntityInstance, x: i32, z: i32) -> bool {
    let (hw, hd) = spatial_half_extents(
        entity.width_m * entity.scale,
        entity.depth_m * entity.scale,
        entity.rotation_y_deg,
    );
    (f64::from(x) - f64::from(entity.world_x)).abs() <= hw
        && (f64::from(z) - f64::from(entity.world_z)).abs() <= hd
}

/// A cell-footprint predicate over a list of entities, in their published
/// geometry. Rounded the way `frontage::bounds` rounds, so the occupancy test is
/// the same window the placement preflight already refuses.
pub fn entity_occupies(entities: &[EntityInstance]) -> impl Fn(i32, i32) -> bool + '_ {
    move |x: i32, z: i32| entities.iter().any(|entity| entity_covers(entity, x, z))
}

/// One pad's published earthwork record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutPad {
    /// The plan's own pad id, so the record names something an auditor can find.
    pub pad_id: String,
    pub entity_id: String,
    /// The pad's declared millimetre design level, verbatim.
    pub design_level_mm: i32,
    /// The cap this pad was accepted under (`<= MAX_CUT_MM`).
    pub max_cut_mm: i32,
    pub cut_cells: u32,
    pub cut_depth_sum_mm: i64,
    pub max_cut_depth_mm: i32,
    /// `cut_depth_sum_mm / 1000`: one cell is one square metre.
    pub cut_volume_m3: f64,
}

/// One graded road's published earthwork record.
///
/// A road is not a pad: its declared surface is a longitudinal PROFILE, not one
/// millimetre level, so the record publishes the profile's own centre level
/// (`designLevelMm`) and the depth is re-derived per cell from the road's own
/// profile by [`RoadCutFootprint`]. The record is keyed by the road's id, the
/// same id the road plan, the scene and the road audit use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutRoad {
    /// The plan's own road id (`generated.road-auto.scene-0.0`, ...).
    pub road_id: String,
    /// The published entity id; always the road id.
    pub entity_id: String,
    /// The road's millimetre surface at its own centre station.
    pub design_level_mm: i32,
    /// The cap this road was accepted under (`<= MAX_CUT_MM`).
    pub max_cut_mm: i32,
    pub cut_cells: u32,
    pub cut_depth_sum_mm: i64,
    pub max_cut_depth_mm: i32,
    /// `cut_depth_sum_mm / 1000`: one cell is one square metre.
    pub cut_volume_m3: f64,
}

impl CutRoad {
    fn empty(footprint: &RoadCutFootprint) -> Self {
        Self {
            road_id: footprint.road_id.clone(),
            entity_id: footprint.entity_id.clone(),
            design_level_mm: footprint.design_level_mm,
            max_cut_mm: footprint.cap_mm,
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            max_cut_depth_mm: 0,
            cut_volume_m3: 0.,
        }
    }

    fn add(&mut self, depth_mm: i64) {
        self.cut_cells += 1;
        self.cut_depth_sum_mm += depth_mm;
        self.max_cut_depth_mm = self.max_cut_depth_mm.max(depth_mm as i32);
    }

    fn seal(&mut self) {
        self.cut_volume_m3 = self.cut_depth_sum_mm as f64 / 1000.;
    }
}

/// What the cut did to one world, per pad, per graded road and in total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutLedger {
    /// The authored value verbatim, so a wrong value is visible rather than
    /// normalised away.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<String>,
    pub enabled: bool,
    /// The named reason the policy is not enabled, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_enabled: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cut_mm: Option<i32>,
    pub pads: Vec<CutPad>,
    /// The graded roads' own excavation, one record per cap-carrying road, keyed
    /// by road id. Empty (and therefore absent) for every world whose roads keep
    /// the historical fill-only support, so those bytes do not move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roads: Vec<CutRoad>,
    pub cut_cells: u64,
    pub cut_depth_sum_mm: i64,
    pub cut_volume_m3: f64,
    pub max_cut_depth_mm: i32,
}

impl CutLedger {
    /// The empty ledger of a world that did not enable the policy, carrying its
    /// named reason.
    pub fn off(style: &serde_json::Value, reason: NotEnabled) -> Self {
        let requested = style
            .get("frontageProgram")
            .and_then(|program| program.get(POLICY_KEY))
            .map(|value| match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            });
        Self {
            requested,
            enabled: false,
            not_enabled: Some(reason.as_str().into()),
            max_cut_mm: None,
            pads: vec![],
            roads: vec![],
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
        }
    }

    pub fn totals(&mut self) {
        self.cut_cells = self
            .pads
            .iter()
            .map(|pad| u64::from(pad.cut_cells))
            .sum::<u64>()
            + self
                .roads
                .iter()
                .map(|road| u64::from(road.cut_cells))
                .sum::<u64>();
        self.cut_depth_sum_mm = self
            .pads
            .iter()
            .map(|pad| pad.cut_depth_sum_mm)
            .sum::<i64>()
            + self
                .roads
                .iter()
                .map(|road| road.cut_depth_sum_mm)
                .sum::<i64>();
        self.cut_volume_m3 = self.cut_depth_sum_mm as f64 / 1000.;
        self.max_cut_depth_mm = self
            .pads
            .iter()
            .map(|pad| pad.max_cut_depth_mm)
            .chain(self.roads.iter().map(|road| road.max_cut_depth_mm))
            .max()
            .unwrap_or(0);
    }

    pub fn pad(&self, pad_id: &str) -> Option<&CutPad> {
        self.pads.iter().find(|pad| pad.pad_id == pad_id)
    }

    pub fn road(&self, road_id: &str) -> Option<&CutRoad> {
        self.roads.iter().find(|road| road.road_id == road_id)
    }
}

/// The cells of ONE graded road's own flat top, in world axes.
///
/// A road's graded flat top is the road's: the raster publishes the road's own
/// profile there, and the ledger books that excavation under the road's id. A
/// pad's cut footprint therefore STOPS at a graded band - the pad was never
/// allowed to claim a cell the road owns, and once the road may excavate, a
/// shared cell would be counted twice and re-derived against the wrong formula.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadFlatTop {
    center_x: f64,
    center_z: f64,
    half_w: f64,
    half_d: f64,
    rotation_y_deg: f32,
}

impl RoadFlatTop {
    /// Does this road's graded flat top hold the cell? The predicate is the one
    /// [`RoadCutFootprint::outside_m`] uses, so "the road's flat top" means the
    /// same cells here, in the composition and in the audit.
    pub fn holds(&self, x: i32, z: i32) -> bool {
        super::oriented_outside_m(
            self.center_x,
            self.center_z,
            self.half_w,
            self.half_d,
            self.rotation_y_deg,
            f64::from(x),
            f64::from(z),
        ) <= 0.0
    }
}

/// Every graded road's own flat top of one entity list, in published geometry.
pub fn road_flat_tops(entities: &[EntityInstance]) -> Vec<RoadFlatTop> {
    entities
        .iter()
        .filter(|entity| entity.kind == "road" && entity.grounding.road_profile.is_some())
        .map(|entity| {
            let (half_w, half_d) = super::graded_flat_top_local_half_extents(entity);
            RoadFlatTop {
                center_x: f64::from(entity.world_x),
                center_z: f64::from(entity.world_z),
                half_w,
                half_d,
                rotation_y_deg: entity.rotation_y_deg,
            }
        })
        .collect()
}

/// Does any graded road's flat top hold this cell?
fn in_road_flat_top(tops: &[RoadFlatTop], x: i32, z: i32) -> bool {
    tops.iter().any(|top| top.holds(x, z))
}

/// Every cap-carrying carve of one bounded-cut pad, including the entity's
/// explicit door landing. They share one ledger owner but may occupy different
/// rectangles; the deepest carve at a cell is the surface the bake publishes.
fn entity_cut_carves(entity: &EntityInstance) -> Vec<super::TerrainCarve> {
    super::plan_terrain_carves(std::slice::from_ref(entity))
        .into_iter()
        .filter(|carve| carve.ramp.is_none() && carve.cut_cap_mm.is_some())
        .collect()
}

fn carve_cell_bounds(carve: &super::TerrainCarve) -> (i32, i32, i32, i32) {
    let ring = ring_width_m(carve.cut_cap_mm.unwrap_or(0));
    let (half_w, half_d) =
        super::oriented_aabb_half_extents(carve.half_w, carve.half_d, carve.rotation_y_deg);
    (
        (carve.cx - half_w - ring).floor() as i32,
        (carve.cz - half_d - ring).floor() as i32,
        (carve.cx + half_w + ring).ceil() as i32,
        (carve.cz + half_d + ring).ceil() as i32,
    )
}

fn carve_cut_depth_mm(
    carve: &super::TerrainCarve,
    base_m: f64,
    x: i32,
    z: i32,
) -> i64 {
    let Some(cap) = carve.cut_cap_mm else {
        return 0;
    };
    let outside = super::carve_outside_m(carve, f64::from(x), f64::from(z));
    if outside >= ring_width_m(cap) {
        return 0;
    }
    cut_depth_mm(base_m, carve.target_h_m, outside, cap)
}

fn owner_cut_depth_mm(
    carves: &[super::TerrainCarve],
    base_m: f64,
    x: i32,
    z: i32,
) -> i64 {
    carves
        .iter()
        .map(|carve| carve_cut_depth_mm(carve, base_m, x, z))
        .max()
        .unwrap_or(0)
}
/// The pad records the PLAN and the NATURAL raster alone predict.
///
/// This is the independent view requirement 3 asks for: it never reads the final
/// raster and never reads a stored number from the ledger. `level` comes from the
/// plan's `supportLevelMm`, the geometry from the pad entity, the depth from
/// [`cut_surface_m`] - the same three inputs the generator used.
pub fn recompute_from_plan(
    plan: &[RoadsideSite],
    natural: &BakedTerrain,
    road_tops: &[RoadFlatTop],
) -> Result<Vec<CutPad>, String> {
    let mut pads = vec![];
    for site in plan {
        let Some(cap) = site.bounded_cut_mm else {
            continue;
        };
        let Some(level) = site.support_level_mm else {
            continue;
        };
        let entity = site.entity();
        // One bounded-cut entity may own more than its support rectangle: a
        // generated door landing carries the same entity id and its own small
        // cap. The published raster and ledger include it, so the independent
        // plan-only prediction must include it too. Taking the deepest owned
        // carve at each cell keeps the two readers aligned without booking a
        // neighbour's fill as this pad's excavation.
        let entity_carves = entity_cut_carves(&entity);
        let mut depth_by_cell = BTreeMap::<(i32, i32), i64>::new();
        for carve in &entity_carves {
            let (left, top, right, bottom) = carve_cell_bounds(carve);
            for z in top..=bottom {
                for x in left..=right {
                    if in_road_flat_top(road_tops, x, z) {
                        continue;
                    }
                    let Some(base) = natural.height_millimetres(x, z) else {
                        continue;
                    };
                    let depth = carve_cut_depth_mm(carve, f64::from(base) / 1000., x, z);
                    if depth > 0 {
                        depth_by_cell
                            .entry((x, z))
                            .and_modify(|known| *known = (*known).max(depth))
                            .or_insert(depth);
                    }
                }
            }
        }
        let sum = depth_by_cell.values().sum::<i64>();
        let max = depth_by_cell
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        let cells = depth_by_cell.len() as u32;
        pads.push(CutPad {
            pad_id: site.id.clone(),
            entity_id: site.id.clone(),
            design_level_mm: level,
            max_cut_mm: clamp_cap(cap),
            cut_cells: cells,
            cut_depth_sum_mm: sum,
            max_cut_depth_mm: max,
            cut_volume_m3: sum as f64 / 1000.,
        });
    }
    Ok(pads)
}

/// The pad records the two PUBLISHED rasters alone measure.
///
/// This is the ground truth the ledger is written from: every cell of the pad's
/// footprint whose final sample stands below its natural sample is a cut cell,
/// and the depth is the published difference in whole millimetres.
pub fn measure_from_rasters(
    plan: &[RoadsideSite],
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    road_tops: &[RoadFlatTop],
) -> Result<Vec<CutPad>, String> {
    let entities: Vec<EntityInstance> = plan.iter().map(RoadsideSite::entity).collect();
    measure_cut_entities(&entities, natural, final_terrain, road_tops)
}

/// Measure every cap-carrying building entity, not only roadside pads. Settlement
/// evolution and other weak local benches publish the same `boundedCutMm`
/// contract and must be booked under the same owner instead of becoming an
/// untracked excavation.
pub fn measure_cut_entities(
    entities: &[EntityInstance],
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    road_tops: &[RoadFlatTop],
) -> Result<Vec<CutPad>, String> {
    let mut pads = vec![];
    let mut seen_ids = BTreeSet::<&str>::new();
    for entity in entities.iter().filter(|entity| {
        entity.kind != "road" && entity.grounding.bounded_cut_mm.is_some()
    }) {
        if !seen_ids.insert(entity.entity_id.as_str()) {
            continue;
        }
        let Some(cap) = entity.grounding.bounded_cut_mm else {
            continue;
        };
        let level = (f64::from(entity.ground_height_m()) * 1000.).round() as i32;
        let entity_carves = entity_cut_carves(entity);
        let mut depth_by_cell = BTreeMap::<(i32, i32), i64>::new();
        for carve in &entity_carves {
            let (left, top, right, bottom) = carve_cell_bounds(carve);
            for z in top..=bottom {
                for x in left..=right {
                    if in_road_flat_top(road_tops, x, z) {
                        continue;
                    }
                    let outside = super::carve_outside_m(carve, f64::from(x), f64::from(z));
                    if outside >= ring_width_m(carve.cut_cap_mm.unwrap_or(0)) {
                        continue;
                    }
                    let (Some(base), Some(final_mm)) = (
                        natural.height_millimetres(x, z),
                        final_terrain.height_millimetres(x, z),
                    ) else {
                        continue;
                    };
                    let depth = i64::from(base) - i64::from(final_mm);
                    if depth > 0 {
                        depth_by_cell
                            .entry((x, z))
                            .and_modify(|known| *known = (*known).max(depth))
                            .or_insert(depth);
                    }
                }
            }
        }
        let sum = depth_by_cell.values().sum::<i64>();
        let max = depth_by_cell
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        let cells = depth_by_cell.len() as u32;
        pads.push(CutPad {
            pad_id: entity.entity_id.clone(),
            entity_id: entity.entity_id.clone(),
            design_level_mm: level,
            max_cut_mm: clamp_cap(cap),
            cut_cells: cells,
            cut_depth_sum_mm: sum,
            max_cut_depth_mm: max,
            cut_volume_m3: sum as f64 / 1000.,
        });
    }
    Ok(pads)
}

/// One graded road's bounded-cut footprint: the graded flat top it may cut, the
/// transport band (flat top + the corridor's fixed shoulder) whose fill it owns,
/// and the ring that grades the excavation back to the natural surface.
///
/// One derivation, read by `worldgen::apply_terrain_carves` (through the carve
/// list `plan_terrain_carves` builds from the same entity), by the ledger and by
/// [`verify`], so "the cells the raster carves", "the cells the ledger books" and
/// "the cells the audit re-derives" cannot drift.
#[derive(Debug, Clone)]
pub struct RoadCutFootprint {
    pub road_id: String,
    pub entity_id: String,
    pub cap_mm: i32,
    /// The road's own millimetre surface at its centre station.
    pub design_level_mm: i32,
    /// The authored longitudinal contract the per-cell target comes from.
    pub profile: RoadProfile,
    pub center_x: f64,
    pub center_z: f64,
    pub rotation_y_deg: f32,
    /// The graded flat top (the band the earthwork flattens to the profile).
    pub flat_half_w: f64,
    pub flat_half_d: f64,
    /// The transport band: flat top plus the corridor's shoulder.
    pub transport_half_w: f64,
    pub transport_half_d: f64,
    /// Bounding box of the flat top plus the cut ring.
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RoadCutFootprint {
    /// The footprint of ONE graded road entity. `None` when the road carries no
    /// profile: a flat road has no authored longitudinal surface to cut to, so it
    /// keeps the historical fill-only contract.
    pub fn of(entity: &EntityInstance, cap_mm: i32) -> Option<Self> {
        let profile = entity.grounding.road_profile.clone()?;
        let (flat_half_w, flat_half_d) = super::graded_flat_top_local_half_extents(entity);
        let (transport_half_w, transport_half_d) =
            super::graded_transport_local_half_extents(entity);
        let cap_mm = clamp_cap(cap_mm);
        let ring = ring_width_m(cap_mm);
        let cx = f64::from(entity.world_x);
        let cz = f64::from(entity.world_z);
        let (aabb_half_w, aabb_half_d) =
            super::oriented_aabb_half_extents(flat_half_w, flat_half_d, entity.rotation_y_deg);
        Some(Self {
            road_id: entity.entity_id.clone(),
            entity_id: entity.entity_id.clone(),
            cap_mm,
            design_level_mm: (f64::from(entity.ground_height_m()) * 1000.).round() as i32,
            profile,
            center_x: cx,
            center_z: cz,
            rotation_y_deg: entity.rotation_y_deg,
            flat_half_w,
            flat_half_d,
            transport_half_w,
            transport_half_d,
            left: (cx - aabb_half_w - ring).floor() as i32,
            top: (cz - aabb_half_d - ring).floor() as i32,
            right: (cx + aabb_half_w + ring).ceil() as i32,
            bottom: (cz + aabb_half_d + ring).ceil() as i32,
        })
    }

    /// The authored surface at one cell, in metres.
    pub fn target_m(&self, x: i32, z: i32) -> f64 {
        self.profile
            .height_at(f64::from(x), f64::from(z))
            .unwrap_or(self.design_level_mm as f64 / 1000.)
    }

    /// The cell's distance outside the graded flat top, in metres (<= 0 inside).
    pub fn outside_m(&self, x: i32, z: i32) -> f64 {
        super::oriented_outside_m(
            self.center_x,
            self.center_z,
            self.flat_half_w,
            self.flat_half_d,
            self.rotation_y_deg,
            f64::from(x),
            f64::from(z),
        )
    }

    /// The cell's distance outside the transport band, in metres (<= 0 inside).
    pub fn transport_outside_m(&self, x: i32, z: i32) -> f64 {
        super::oriented_outside_m(
            self.center_x,
            self.center_z,
            self.transport_half_w,
            self.transport_half_d,
            self.rotation_y_deg,
            f64::from(x),
            f64::from(z),
        )
    }

    /// The depth this footprint cuts at one cell, in whole millimetres.
    pub fn depth_mm(&self, base_m: f64, x: i32, z: i32) -> i64 {
        cut_depth_mm(
            base_m,
            self.target_m(x, z),
            self.outside_m(x, z),
            self.cap_mm,
        )
    }

    /// True when the cell belongs to the footprint at all (inside or in the ring).
    pub fn contains(&self, x: i32, z: i32) -> bool {
        let outside = self.outside_m(x, z);
        outside <= 0. || outside < ring_width_m(self.cap_mm)
    }
}

/// Another entity's own support window, in world cells. A road's excavation and
/// its fill authority stop at the edge of every such window, so no road cut is
/// ever booked against an entity that owns the cell differently.
///
/// The window is the carve rectangle plus the margin that entity's own
/// earthwork/audit claims:
///
/// * a cap-carrying pad's whole [`CutFootprint`] (`margin = ring_width_m`),
/// * any other carve's support rectangle plus its 2 m audit margin
///   (`margin = 2 m`), which also covers a road's transport band (flat top + at
///   most 2 m of shoulder).
#[derive(Debug, Clone, Copy)]
pub struct SupportWindow {
    pub center_x: f64,
    pub center_z: f64,
    pub half_w: f64,
    pub half_d: f64,
    pub rotation_y_deg: f32,
    pub margin_m: f64,
    /// The road slot this window belongs to, when it belongs to a road: a road is
    /// never blocked by its own window. `None` for pads.
    pub road_slot: Option<usize>,
}

impl SupportWindow {
    pub fn pad(site: &RoadsideSite) -> Self {
        let entity = site.entity();
        let cap = site.bounded_cut_mm.map(clamp_cap);
        let (half_w, half_d) = support_pad_half_extents(
            entity.width_m,
            entity.depth_m,
            entity.rotation_y_deg,
            entity.grounding.frontage_road_point.is_some(),
        );
        Self {
            center_x: f64::from(entity.world_x),
            center_z: f64::from(entity.world_z),
            half_w,
            half_d,
            rotation_y_deg: 0.0,
            margin_m: cap.map_or(2.0, ring_width_m),
            road_slot: None,
        }
    }

    pub fn road(footprint: &RoadCutFootprint, slot: usize) -> Self {
        Self {
            center_x: footprint.center_x,
            center_z: footprint.center_z,
            half_w: footprint.flat_half_w,
            half_d: footprint.flat_half_d,
            rotation_y_deg: footprint.rotation_y_deg,
            margin_m: 2.0,
            road_slot: Some(slot),
        }
    }

    /// Does this window claim the cell? Strictly inside its margin, matching the
    /// per-cell test in `worldgen::apply_terrain_carves`.
    pub fn blocks(&self, x: i32, z: i32) -> bool {
        super::oriented_outside_m(
            self.center_x,
            self.center_z,
            self.half_w,
            self.half_d,
            self.rotation_y_deg,
            f64::from(x),
            f64::from(z),
        ) < self.margin_m - super::SUPPORT_WINDOW_EPS_M
    }
}

/// The plan's cap-carrying graded roads, their footprints and the per-cell owner
/// of every road excavation cell. One derivation shared by the ledger and the
/// audit: a cell is owned by the road whose declared cut is deepest there, and
/// every road's ring is clipped by the other entities' [`SupportWindow`]s, so no
/// cell is booked twice.
#[derive(Debug, Clone)]
pub struct RoadCutPlan {
    pub footprints: Vec<RoadCutFootprint>,
    pub windows: Vec<SupportWindow>,
    /// Scene cell -> footprint slot, or `-1` for "no road owns a cut here".
    pub owner: Vec<i32>,
}

impl RoadCutPlan {
    pub fn owner_at(&self, scene: &super::SceneSpec, x: i32, z: i32) -> Option<usize> {
        let dx = x - scene.origin_x;
        let dz = z - scene.origin_z;
        if dx < 0 || dz < 0 || dx >= scene.width_m as i32 || dz >= scene.depth_m as i32 {
            return None;
        }
        let slot = self.owner[(dz * scene.width_m as i32 + dx) as usize];
        (slot >= 0).then_some(slot as usize)
    }

    /// True when the cell is claimed by another entity's support window and may
    /// therefore not be cut by `slot` (the road's own window never blocks it).
    ///
    /// Inside `slot`'s OWN graded flat top only another road's band can take the
    /// cell away, exactly as in the composition and in [`road_cut_plan`]: a pad's
    /// carve rectangle, entrance apron or 8 m graded ring may not decide what the
    /// roadbed is.
    pub fn blocked_for(&self, slot: usize, x: i32, z: i32) -> Option<&'static str> {
        let inside_flat_top = self
            .footprints
            .get(slot)
            .is_none_or(|footprint| footprint.outside_m(x, z) <= 0.);
        if self.windows.iter().any(|window| {
            window.road_slot != Some(slot)
                && window.blocks(x, z)
                && !(inside_flat_top && window.road_slot.is_none())
        }) {
            return Some(ON_OCCUPIED);
        }
        None
    }
}

/// The road cut owners the plan and the natural raster alone predict.
///
/// `policy` supplies the one cap; a road without a profile is not a cut road and
/// contributes no footprint. The returned owner map is the ledger's and the
/// audit's common ground truth.
pub fn road_cut_plan(
    scene: &super::SceneSpec,
    plan: &super::road_network::RoadNetworkPlan,
    natural: &BakedTerrain,
    policy: CutPolicy,
) -> Result<RoadCutPlan, String> {
    road_cut_plan_with_entities(scene, plan, &[], natural, policy)
}

/// Road ownership with the final entity list included. Widening, reconciliation
/// and base-profile attachment can replace a plan edge's published geometry; the
/// final road entity is authoritative for carving and must therefore be authoritative
/// for the cut-owner map as well.
pub fn road_cut_plan_with_entities(
    scene: &super::SceneSpec,
    plan: &super::road_network::RoadNetworkPlan,
    entities: &[EntityInstance],
    natural: &BakedTerrain,
    policy: CutPolicy,
) -> Result<RoadCutPlan, String> {
    let mut footprints = Vec::new();
    if policy.enabled() {
        let cap = policy.cap_mm();
        let mut roads = BTreeMap::<String, EntityInstance>::new();
        for entity in entities.iter().filter(|entity| entity.kind == "road") {
            roads.insert(entity.entity_id.clone(), entity.clone());
        }
        for entity in plan.entities() {
            roads.entry(entity.entity_id.clone()).or_insert(entity);
        }
        for entity in roads.values() {
            if entity.grounding.bounded_cut_mm != Some(cap) {
                continue;
            }
            if let Some(footprint) = RoadCutFootprint::of(&entity, cap) {
                footprints.push(footprint);
            }
        }
    }
    // Every published pad owns its support rectangle plus its own cut footprint
    // (a cap pad's ring) or its 2 m audit margin (a fill-only pad).
    let mut windows: Vec<SupportWindow> =
        plan.roadside_sites.iter().map(SupportWindow::pad).collect();
    for (slot, footprint) in footprints.iter().enumerate() {
        windows.push(SupportWindow::road(footprint, slot));
    }
    let width = scene.width_m as i32;
    let depth = scene.depth_m as i32;
    let mut owner = vec![-1_i32; (width * depth) as usize];
    let mut owner_depth = vec![0_i64; (width * depth) as usize];
    for (slot, footprint) in footprints.iter().enumerate() {
        for z in footprint.top..=footprint.bottom {
            for x in footprint.left..=footprint.right {
                if !footprint.contains(x, z) {
                    continue;
                }
                let dx = x - scene.origin_x;
                let dz = z - scene.origin_z;
                if dx < 0 || dz < 0 || dx >= width || dz >= depth {
                    continue;
                }
                // Inside the road's OWN graded flat top only another road's band
                // can take the cell away: a pad's carve rectangle, entrance apron
                // and 8 m graded ring may not decide what the roadbed is (the
                // composition states the same rule). Outside the flat top every
                // window still clips the excavation.
                let inside_flat_top = footprint.outside_m(x, z) <= 0.;
                if windows.iter().any(|window| {
                    window.road_slot != Some(slot)
                        && window.blocks(x, z)
                        && !(inside_flat_top && window.road_slot.is_none())
                }) {
                    continue;
                }
                let Some(base) = natural.height_millimetres(x, z) else {
                    continue;
                };
                let expected = footprint.depth_mm(f64::from(base) / 1000., x, z);
                if expected <= 0 {
                    continue;
                }
                let at = (dz * width + dx) as usize;
                if expected > owner_depth[at] {
                    owner_depth[at] = expected;
                    owner[at] = slot as i32;
                }
            }
        }
    }
    Ok(RoadCutPlan {
        footprints,
        windows,
        owner,
    })
}

/// The road records the two PUBLISHED rasters alone measure.
///
/// Each road's cells are the cells it OWNS in [`RoadCutPlan`] - the deepest
/// declared cut wins, so two overlapping rings book one cell once - and every
/// cell the rasters lowered there is counted with the published difference.
pub fn measure_road_cuts(
    scene: &super::SceneSpec,
    plan: &super::road_network::RoadNetworkPlan,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    policy: CutPolicy,
) -> Result<Vec<CutRoad>, String> {
    measure_road_cuts_with_entities(scene, plan, &[], natural, final_terrain, policy)
}

pub fn measure_road_cuts_with_entities(
    scene: &super::SceneSpec,
    plan: &super::road_network::RoadNetworkPlan,
    entities: &[EntityInstance],
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    policy: CutPolicy,
) -> Result<Vec<CutRoad>, String> {
    let road_plan = road_cut_plan_with_entities(scene, plan, entities, natural, policy)?;
    let mut roads: Vec<CutRoad> = road_plan.footprints.iter().map(CutRoad::empty).collect();
    for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
        for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
            let Some(slot) = road_plan.owner_at(scene, x, z) else {
                continue;
            };
            let (Some(base), Some(final_mm)) = (
                natural.height_millimetres(x, z),
                final_terrain.height_millimetres(x, z),
            ) else {
                continue;
            };
            let depth = i64::from(base) - i64::from(final_mm);
            if depth <= 0 {
                continue;
            }
            roads[slot].add(depth);
        }
    }
    for road in &mut roads {
        road.seal();
    }
    Ok(roads)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutAudit {
    pub status: String,
    pub enabled: bool,
    pub pads: usize,
    /// The graded roads that publish a cut record.
    pub roads: usize,
    pub cut_cells: u64,
    pub cut_depth_sum_mm: i64,
    pub cut_volume_m3: f64,
    pub max_cut_depth_mm: i32,
    pub protected_cells_changed: u64,
    pub water_cells_changed: u64,
    pub shore_band_cells_cut: u64,
    pub failures: Vec<String>,
}

impl CutAudit {
    fn new(enabled: bool) -> Self {
        Self {
            status: "pass".into(),
            enabled,
            pads: 0,
            roads: 0,
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
            protected_cells_changed: 0,
            water_cells_changed: 0,
            shore_band_cells_cut: 0,
            failures: vec![],
        }
    }
}

/// Recompute and re-verify a world's bounded cut from the plan, the zoning, the
/// water geometry and the two published rasters - never from a stored pass flag.
///
/// `ledger` is the published record. `None` is legal only when the policy is off;
/// an enabled policy with no ledger fails by name, and so does a ledger published
/// by a world that never enabled the policy.
#[allow(clippy::too_many_arguments)]
pub fn verify(
    scene: &super::SceneSpec,
    plan: Option<&super::road_network::RoadNetworkPlan>,
    zones: &[super::LandUseZone],
    water: WaterGeometry,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    ledger: Option<&CutLedger>,
    policy: CutPolicy,
) -> Result<CutAudit, String> {
    verify_with_entities(
        scene,
        plan,
        zones,
        water,
        natural,
        final_terrain,
        ledger,
        policy,
        &[],
    )
}

/// [`verify`] with the full final entity list. This lets weak-cut settlement
/// buildings participate in the same ownership and ledger audit as roadside
/// pads, without pretending they are frontage sites.
#[allow(clippy::too_many_arguments)]
pub fn verify_with_entities(
    scene: &super::SceneSpec,
    plan: Option<&super::road_network::RoadNetworkPlan>,
    zones: &[super::LandUseZone],
    water: WaterGeometry,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    ledger: Option<&CutLedger>,
    policy: CutPolicy,
    extra_entities: &[EntityInstance],
) -> Result<CutAudit, String> {
    let mut audit = CutAudit::new(policy.enabled());
    let sites: &[RoadsideSite] = plan.map_or(&[], |plan| plan.roadside_sites.as_slice());
    let site_entities: Vec<EntityInstance> = sites.iter().map(RoadsideSite::entity).collect();
    let mut seen_pad_ids = BTreeSet::<&str>::new();
    let mut pad_entities: Vec<&EntityInstance> = Vec::new();
    for entity in extra_entities.iter().filter(|entity| {
        entity.kind != "road" && entity.grounding.bounded_cut_mm.is_some()
    }) {
        if seen_pad_ids.insert(entity.entity_id.as_str()) {
            pad_entities.push(entity);
        }
    }
    for entity in &site_entities {
        if entity.grounding.bounded_cut_mm.is_some()
            && seen_pad_ids.insert(entity.entity_id.as_str())
        {
            pad_entities.push(entity);
        }
    }
    // 1. The policy and its record agree.
    match (policy.enabled(), ledger) {
        (true, None) => {
            audit.failures.push(format!(
                "{LEDGER_MISSING}: the policy is enabled but no bounded-cut ledger was published"
            ));
        }
        (false, Some(published))
            if published.enabled || !published.pads.is_empty() || !published.roads.is_empty() =>
        {
            audit.failures.push(format!(
                "{LEDGER_WITHOUT_POLICY}: a bounded-cut ledger was published by a world whose \
                 policy is off"
            ));
        }
        _ => {}
    }
    let published_pads: &[CutPad] = ledger.map_or(&[], |ledger| ledger.pads.as_slice());
    // 2. Every pad's record and the plan agree, and the cap is legal.
    for pad in published_pads {
        if pad.max_cut_mm > MAX_CUT_MM || pad.max_cut_mm <= 0 {
            audit.failures.push(format!(
                "{CAP_ABOVE_MAXIMUM}: pad {} claims a {} mm cap; the maximum is {MAX_CUT_MM} mm",
                pad.pad_id, pad.max_cut_mm
            ));
        }
        match pad_entities
            .iter()
            .find(|entity| entity.entity_id == pad.pad_id)
        {
            None => audit.failures.push(format!(
                "{UNKNOWN_PAD}: {} is not a pad of this plan",
                pad.pad_id
            )),
            Some(entity) => {
                if entity.grounding.bounded_cut_mm != Some(pad.max_cut_mm) {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: pad {} publishes a {} mm cap but the plan declares {:?}",
                        pad.pad_id, pad.max_cut_mm, entity.grounding.bounded_cut_mm
                    ));
                }
                let level = (f64::from(entity.ground_height_m()) * 1000.).round() as i32;
                if level != pad.design_level_mm {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: pad {} publishes design level {} mm but the plan \
                         declares {level} mm",
                        pad.pad_id, pad.design_level_mm
                    ));
                }
            }
        }
    }
    // 3. The plan's own pads all have a record.
    for entity in &pad_entities {
        if !published_pads
            .iter()
            .any(|pad| pad.pad_id == entity.entity_id)
        {
            audit.failures.push(format!(
                "{PAD_NOT_IN_LEDGER}: pad {} carries a bounded-cut cap but no published record",
                entity.entity_id
            ));
        }
    }
    // 3b. A graded road is a cut support too: its record names the road, and the
    //     per-cell target is the road's own published profile.
    let published_roads: &[CutRoad] = ledger.map_or(&[], |ledger| ledger.roads.as_slice());
    let mut road_entities = BTreeMap::<String, EntityInstance>::new();
    for entity in extra_entities.iter().filter(|entity| entity.kind == "road") {
        road_entities.insert(entity.entity_id.clone(), entity.clone());
    }
    if let Some(plan) = plan {
        for entity in plan.entities() {
            road_entities.entry(entity.entity_id.clone()).or_insert(entity);
        }
    }
    let plan_roads: Vec<&EntityInstance> = road_entities
        .values()
        .filter(|entity| entity.kind == "road" && entity.grounding.bounded_cut_mm.is_some())
        .collect();
    for road in published_roads {
        if road.max_cut_mm > MAX_CUT_MM || road.max_cut_mm <= 0 {
            audit.failures.push(format!(
                "{CAP_ABOVE_MAXIMUM}: road {} claims a {} mm cap; the maximum is {MAX_CUT_MM} mm",
                road.road_id, road.max_cut_mm
            ));
        }
        match plan_roads
            .iter()
            .find(|entity| entity.entity_id == road.road_id)
        {
            None => audit.failures.push(format!(
                "{UNKNOWN_ROAD}: {} is not a cap-carrying graded road of this plan",
                road.road_id
            )),
            Some(entity) => {
                let level = (f64::from(entity.ground_height_m()) * 1000.).round() as i32;
                if road.design_level_mm != level {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: road {} publishes design level {} mm but its profile \
                         stands at {level} mm at the centre station",
                        road.road_id, road.design_level_mm
                    ));
                }
                if entity.grounding.bounded_cut_mm != Some(road.max_cut_mm) {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: road {} publishes a {} mm cap but the plan declares {:?}",
                        road.road_id, road.max_cut_mm, entity.grounding.bounded_cut_mm
                    ));
                }
            }
        }
    }
    for entity in &plan_roads {
        if !published_roads
            .iter()
            .any(|road| road.road_id == entity.entity_id)
        {
            audit.failures.push(format!(
                "{ROAD_NOT_IN_LEDGER}: road {} carries a bounded-cut cap but no published record",
                entity.entity_id
            ));
        }
    }
    if !policy.enabled() {
        audit.status = if audit.failures.is_empty() {
            "not_applicable".into()
        } else {
            "fail".into()
        };
        return Ok(audit);
    }
    // 4. Per cell: the cut is exactly the declared one, inside a pad, legal.
    let mask = ZoningMask::new(scene, zones).map_err(|e| e.0)?;
    // Every graded road's own flat top: those cells belong to the road's ledger
    // and no pad's footprint may book them (see [RoadFlatTop]).
    let road_tops = plan.map_or_else(Vec::new, |plan| {
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        road_flat_tops(&entities)
    });
    // 4a-pre. No two pads may share a cut footprint. The per-pad measurement below
    //     is what the ledger TOTAL is made of, so a shared cell would be booked once
    //     per pad and the total would exceed the excavation the published rasters
    //     actually hold. The placement preflight refuses such a pair by name; this
    //     re-derives the same rule from the plan a reader holds, so neither side can
    //     pass alone.
    let mut cut_footprints: Vec<(&str, CutFootprint)> = Vec::new();
    for entity in &pad_entities {
        let cap = entity.grounding.bounded_cut_mm.unwrap_or(0);
        cut_footprints.push((
            entity.entity_id.as_str(),
            CutFootprint::of(entity, cap),
        ));
    }
    for (index, (pad_id, footprint)) in cut_footprints.iter().enumerate() {
        for (other_id, other) in cut_footprints.iter().skip(index + 1) {
            if let Some((x, z)) = footprint_intersection(footprint, other) {
                audit.failures.push(format!(
                    "{FOOTPRINT_OVERLAP}: pads {pad_id} and {other_id} share the cut footprint \
                     cell ({x},{z}); the ledger would count that cell once per pad"
                ));
            }
        }
    }
    let width = scene.width_m as i32;
    let depth = scene.depth_m as i32;
    let index = |x: i32, z: i32| -> Option<usize> {
        let (dx, dz) = (x - scene.origin_x, z - scene.origin_z);
        (dx >= 0 && dz >= 0 && dx < width && dz < depth).then(|| (dz * width + dx) as usize)
    };
    // One owner per cell: the pad whose DECLARED BENCH is deepest there.
    //
    //  * `owner` holds the index of the FOOTPRINT, not the index of the site. The two
    //    lists have different lengths whenever the plan carries a pad without a cap
    //    (which is every plan: settlement pads never cut), and mixing them reported
    //    every cell of a cap-carrying pad past the first few sites as owned by nobody
    //    - `bounded_cut_outside_pad` for legitimate, declared excavations.
    //  * A shared cell is owned by the DEEPER footprint. Placement no longer accepts
    //    a pair whose footprints intersect (4a-pre above refuses it by name), so this
    //    is the defensive resolution for a plan that bypassed placement: the owner is
    //    the surface `apply_terrain_carves` publishes there (the minimum cut), and the
    //    overlap itself has already been reported.
    let mut owner: Vec<i32> = vec![-1; (width * depth) as usize];
    let mut owner_depth_mm: Vec<i64> = vec![0; (width * depth) as usize];
    let mut footprints: Vec<CutFootprint> = Vec::with_capacity(sites.len());
    let mut pad_carves: Vec<Vec<super::TerrainCarve>> = Vec::with_capacity(sites.len());
    for entity in &pad_entities {
        let Some(cap) = entity.grounding.bounded_cut_mm else {
            continue;
        };
        let footprint = CutFootprint::of(entity, cap);
        let slot = footprints.len() as i32;
        footprints.push(footprint);
        let carves = entity_cut_carves(entity);
        for carve in &carves {
            let (left, top, right, bottom) = carve_cell_bounds(carve);
            for z in top..=bottom {
                for x in left..=right {
                    if in_road_flat_top(&road_tops, x, z) {
                        continue;
                    }
                    let Some(at) = index(x, z) else { continue };
                    let Some(base) = natural.height_millimetres(x, z) else {
                        continue;
                    };
                    let expected = carve_cut_depth_mm(carve, f64::from(base) / 1000., x, z);
                    if expected > owner_depth_mm[at] {
                        owner_depth_mm[at] = expected;
                        owner[at] = slot;
                    }
                }
            }
        }
        pad_carves.push(carves);
    }
    // The measured per-pad record, straight from the two rasters.
    let measured_entities: Vec<EntityInstance> =
        pad_entities.iter().map(|entity| (*entity).clone()).collect();
    let measured =
        measure_cut_entities(&measured_entities, natural, final_terrain, &road_tops)?;
    // The road cut owners and the measured road records, from the plan and the
    // two rasters. A road's ring is clipped by every pad window and every other
    // road's graded band, and a shared road cell is owned by the deeper cut, so
    // no cell is booked twice.
    let road_plan = plan
        .map(|plan| {
            road_cut_plan_with_entities(scene, plan, extra_entities, natural, policy)
        })
        .transpose()?
        .unwrap_or(RoadCutPlan {
            footprints: vec![],
            windows: vec![],
            owner: vec![-1; (width * depth) as usize],
        });
    let measured_roads = match plan {
        Some(plan) => measure_road_cuts_with_entities(
            scene,
            plan,
            extra_entities,
            natural,
            final_terrain,
            policy,
        )?,
        None => vec![],
    };
    // 4a-road. The published road record IS the measurement (a forged number
    //     fails here, exactly as it does for a pad).
    for road in published_roads {
        let Some(truth) = measured_roads
            .iter()
            .find(|record| record.road_id == road.road_id)
        else {
            continue;
        };
        let fields: [(&str, bool); 5] = [
            ("cutCells", road.cut_cells == truth.cut_cells),
            (
                "cutDepthSumMm",
                road.cut_depth_sum_mm == truth.cut_depth_sum_mm,
            ),
            (
                "maxCutDepthMm",
                road.max_cut_depth_mm == truth.max_cut_depth_mm,
            ),
            (
                "designLevelMm",
                road.design_level_mm == truth.design_level_mm,
            ),
            (
                "cutVolumeM3",
                (road.cut_volume_m3 - truth.cut_volume_m3).abs() < 1e-9,
            ),
        ];
        for (field, ok) in fields {
            if !ok {
                audit.failures.push(format!(
                    "{LEDGER_MISMATCH}: road {} publishes {field} {:?} but the published rasters \
                     hold {:?}",
                    road.road_id, field, truth
                ));
                break;
            }
        }
        if road.cut_cells > truth.cut_cells {
            audit.failures.push(format!(
                "{LEDGER_OVER_APPLIED}: road {} claims {} cut cells, the rasters show {}",
                road.road_id, road.cut_cells, truth.cut_cells
            ));
        } else if road.cut_cells < truth.cut_cells {
            audit.failures.push(format!(
                "{LEDGER_NOT_APPLIED}: road {} reports {} cut cells, the rasters show {}",
                road.road_id, road.cut_cells, truth.cut_cells
            ));
        }
    }
    // 4a. The published record IS the measurement (a forged number fails here).
    for pad in published_pads {
        let Some(truth) = measured.iter().find(|record| record.pad_id == pad.pad_id) else {
            continue;
        };
        let fields: [(&str, bool); 5] = [
            ("cutCells", pad.cut_cells == truth.cut_cells),
            (
                "cutDepthSumMm",
                pad.cut_depth_sum_mm == truth.cut_depth_sum_mm,
            ),
            (
                "maxCutDepthMm",
                pad.max_cut_depth_mm == truth.max_cut_depth_mm,
            ),
            (
                "designLevelMm",
                pad.design_level_mm == truth.design_level_mm,
            ),
            (
                "cutVolumeM3",
                (pad.cut_volume_m3 - truth.cut_volume_m3).abs() < 1e-9,
            ),
        ];
        for (field, ok) in fields {
            if !ok {
                audit.failures.push(format!(
                    "{LEDGER_MISMATCH}: pad {} publishes {field} {:?} but the published rasters \
                     hold {:?}",
                    pad.pad_id, field, truth
                ));
                break;
            }
        }
        if pad.cut_cells > truth.cut_cells {
            audit.failures.push(format!(
                "{LEDGER_OVER_APPLIED}: pad {} claims {} cut cells, the rasters show {}",
                pad.pad_id, pad.cut_cells, truth.cut_cells
            ));
        } else if pad.cut_cells < truth.cut_cells {
            audit.failures.push(format!(
                "{LEDGER_NOT_APPLIED}: pad {} reports {} cut cells, the rasters show {}",
                pad.pad_id, pad.cut_cells, truth.cut_cells
            ));
        }
    }
    // 4b. Every lowered cell: owned by a pad, equal to the formula, within cap,
    //     never on protected land / water / the shore band.
    for z in scene.origin_z..scene.origin_z + depth {
        for x in scene.origin_x..scene.origin_x + width {
            let Some(at) = index(x, z) else {
                continue;
            };
            let (Some(base), Some(final_mm)) = (
                natural.height_millimetres(x, z),
                final_terrain.height_millimetres(x, z),
            ) else {
                continue;
            };
            let cut_mm = i64::from(base) - i64::from(final_mm);
            let protected = mask.protected_at(x, z);
            // The project's own water convention, the one `wet_area` and
            // `WaterGeometry::forbids_earthwork` use: a sample sits at the cell's
            // own integer coordinate, not at its centre.
            let wet = water.contains_point(f64::from(x), f64::from(z));
            let band = water.shore_band_cell(x, z);
            if protected && base != final_mm {
                audit.protected_cells_changed += 1;
                audit.failures.push(format!(
                    "{ON_PROTECTED}: protected cell ({x},{z}) changed from {base} mm to {final_mm} mm"
                ));
            }
            // A cut is what may not enter the water body: the bed, the banks and
            // the surface are never excavated. A fill in a non-opted-in water
            // scene is the historical behaviour and is not this capability's
            // business (the waterfront opt-in forbids it separately, which the
            // zero-change clause above covers).
            if wet && cut_mm > 0 {
                audit.water_cells_changed += 1;
                audit.failures.push(format!(
                    "{IN_WATER}: water cell ({x},{z}) was cut by {cut_mm} mm"
                ));
            }
            if cut_mm <= 0 {
                continue;
            }
            if band {
                audit.shore_band_cells_cut += 1;
                audit.failures.push(format!(
                    "{ON_SHORE_BAND}: shore band cell ({x},{z}) was cut by {cut_mm} mm"
                ));
            }
            let slot = owner[at];
            if slot < 0 || footprints.get(slot as usize).is_none() {
                // Not a pad cell. A graded road may own it: the road's own record
                // and the same per-cell formula are checked here, never by a
                // second audit.
                let Some(road_slot) = road_plan.owner_at(scene, x, z) else {
                    audit.failures.push(format!(
                        "{OUTSIDE_PAD}: cell ({x},{z}) was lowered by {cut_mm} mm but no \
                         bounded-cut pad or graded road owns it"
                    ));
                    continue;
                };
                let footprint = &road_plan.footprints[road_slot];
                if let Some(blocked) = road_plan.blocked_for(road_slot, x, z) {
                    audit.failures.push(format!(
                        "{blocked}: cell ({x},{z}) was cut by {cut_mm} mm inside another \
                         entity's support window, which road {} may not excavate",
                        footprint.road_id
                    ));
                }
                if cut_mm > i64::from(footprint.cap_mm) {
                    audit.failures.push(format!(
                        "{EXCEEDS_CAP}: cell ({x},{z}) was cut by {cut_mm} mm, above road {}'s \
                         {} mm cap",
                        footprint.road_id, footprint.cap_mm
                    ));
                }
                let expected = footprint.depth_mm(f64::from(base) / 1000., x, z);
                if (expected - cut_mm).abs() > CUT_MM_TOLERANCE {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: cell ({x},{z}) was cut by {cut_mm} mm but road {}'s \
                         own profile requires {expected} mm",
                        footprint.road_id
                    ));
                }
                continue;
            }
            let footprint = footprints[slot as usize];
            if cut_mm > i64::from(footprint.cap_mm) {
                audit.failures.push(format!(
                    "{EXCEEDS_CAP}: cell ({x},{z}) was cut by {cut_mm} mm, above the pad's \
                     {} mm cap",
                    footprint.cap_mm
                ));
            }
            let expected = owner_cut_depth_mm(
                &pad_carves[slot as usize],
                f64::from(base) / 1000.,
                x,
                z,
            );
            if (expected - cut_mm).abs() > CUT_MM_TOLERANCE {
                audit.failures.push(format!(
                    "{LEDGER_MISMATCH}: cell ({x},{z}) was cut by {cut_mm} mm but the pad's own \
                     geometry requires {expected} mm"
                ));
            }
        }
    }
    // 5. Every cut pad's flat top is exactly its declared level (grade 0), which
    //    is what keeps the 8 % / 12 % footprint contracts satisfied. The window is
    //    `bounded_cut::flat_top_bounds` - the SAME one the encoder's platform rule
    //    and the bake's completion judgement read, so the obligation, the surface
    //    and the verdict cannot describe three different rectangles.
    for site in sites.iter().filter(|site| site.bounded_cut_mm.is_some()) {
        let Some(verdict) = pad_top_holds(site, |x, z| final_terrain.height_millimetres(x, z))
        else {
            continue;
        };
        if !verdict.holds() {
            audit.failures.push(format!(
                "{PAD_TOP_NOT_FLAT}: pad {} holds {} footprint cells away from its declared \
                 {} mm top",
                site.id, verdict.mismatched, verdict.level_mm
            ));
        }
    }
    // 6. The ring's own face never exceeds the walkable grade.
    for (slot, footprint) in footprints.iter().enumerate() {
        let i = footprint.cap_mm;
        let _ = (slot, i);
        for z in footprint.top..=footprint.bottom {
            for x in footprint.left..=footprint.right {
                let outside = footprint.outside_m(x, z);
                if outside <= 0. {
                    continue;
                }
                let expected_outside = cut_depth_mm(
                    f64::from(natural.height_millimetres(x, z).unwrap_or(0)) / 1000.,
                    footprint.target_m,
                    outside - 1.,
                    footprint.cap_mm,
                );
                let expected_inside = cut_depth_mm(
                    f64::from(natural.height_millimetres(x, z).unwrap_or(0)) / 1000.,
                    footprint.target_m,
                    outside,
                    footprint.cap_mm,
                );
                let step = (expected_outside - expected_inside).abs() as f64 / 1000.;
                if step > MAX_CUT_RING_GRADE + 0.001 {
                    audit.failures.push(format!(
                        "{RING_TOO_STEEP}: pad {} steps {step} m at ({x},{z}) over one metre, \
                         above the {MAX_CUT_RING_GRADE} ring grade",
                        sites
                            .iter()
                            .find(|site| CutFootprint::of(
                                &site.entity(),
                                site.bounded_cut_mm.unwrap_or(0)
                            ) == *footprint)
                            .map(|site| site.id.as_str())
                            .unwrap_or("<unknown>")
                    ));
                }
            }
        }
    }
    audit.pads = measured.len();
    audit.roads = measured_roads.len();
    audit.cut_cells = measured
        .iter()
        .map(|pad| u64::from(pad.cut_cells))
        .sum::<u64>()
        + measured_roads
            .iter()
            .map(|road| u64::from(road.cut_cells))
            .sum::<u64>();
    audit.cut_depth_sum_mm = measured.iter().map(|pad| pad.cut_depth_sum_mm).sum::<i64>()
        + measured_roads
            .iter()
            .map(|road| road.cut_depth_sum_mm)
            .sum::<i64>();
    audit.cut_volume_m3 = audit.cut_depth_sum_mm as f64 / 1000.;
    audit.max_cut_depth_mm = measured
        .iter()
        .map(|pad| pad.max_cut_depth_mm)
        .chain(measured_roads.iter().map(|road| road.max_cut_depth_mm))
        .max()
        .unwrap_or(0);
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (super::super::SceneSpec, Vec<super::super::LandUseZone>) {
        let scene = super::super::SceneSpec {
            scene_id: "cut".into(),
            origin_x: 0,
            origin_z: 0,
            width_m: 64,
            depth_m: 64,
            seed_offset: 0,
        };
        let zones = vec![
            super::super::LandUseZone {
                id: "urban".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 32,
                depth_m: 64,
                source: "test".into(),
            },
            super::super::LandUseZone {
                id: "nature".into(),
                kind: "nature".into(),
                world_x: 32,
                world_z: 0,
                width_m: 32,
                depth_m: 64,
                source: "test".into(),
            },
        ];
        (scene, zones)
    }

    /// Requirement 4: one constant. A style declaring more than the cap is off.
    #[test]
    fn the_cap_is_one_constant_and_every_other_value_is_off_with_a_name() {
        let cases: [(serde_json::Value, Option<NotEnabled>); 6] = [
            (serde_json::json!({}), Some(NotEnabled::NotDeclared)),
            (
                serde_json::json!({"frontageProgram": {"terrainPolicy": "cut"}}),
                Some(NotEnabled::Unrecognised),
            ),
            (
                serde_json::json!({"frontageProgram": {"terrainPolicy": "bounded-cut"}}),
                Some(NotEnabled::CapMissing),
            ),
            (
                serde_json::json!({"frontageProgram": {"terrainPolicy": "bounded-cut", "maxCutMm": 5000}}),
                Some(NotEnabled::CapAboveMaximum),
            ),
            (
                serde_json::json!({"frontageProgram": {"terrainPolicy": "bounded-cut", "maxCutMm": 0}}),
                Some(NotEnabled::CapOutOfRange),
            ),
            (
                serde_json::json!({"frontageProgram": {"terrainPolicy": "bounded-cut", "maxCutMm": 1.5}}),
                Some(NotEnabled::CapNotInteger),
            ),
        ];
        for (style, expected) in cases {
            let (policy, reason) = CutPolicy::from_style(&style);
            assert_eq!(policy, CutPolicy::Disabled, "{}", style);
            assert_eq!(reason, expected, "{}", style);
            assert_eq!(policy.cap_mm(), 0);
        }
        let (policy, reason) = CutPolicy::from_style(
            &serde_json::json!({"frontageProgram": {"terrainPolicy": "bounded-cut", "maxCutMm": 2000}}),
        );
        assert_eq!(policy, CutPolicy::Bounded { max_cut_mm: 2000 });
        assert_eq!(reason, None);
        assert_eq!(MAX_CUT_MM, 2_000);
        // The generator's clamp and the audit's legality test read the same cap.
        assert_eq!(clamp_cap(i32::MAX), MAX_CUT_MM);
        assert_eq!(clamp_cap(-5), 0);
    }

    #[test]
    fn the_cut_never_exceeds_the_cap_and_never_touches_fill() {
        let (base, target) = (12.0, 10.0);
        for cap in [0, 1, 500, 2000] {
            let surface = cut_surface_m(base, target, 0., cap);
            let depth = (base - surface) * 1000.;
            assert!(depth <= f64::from(cap) + 1e-9, "cap {cap} depth {depth}");
            assert!(surface >= base - f64::from(cap) / 1000. - 1e-9);
            assert!(surface <= base);
        }
        // A five metre bank under a two metre cap is cut only to the cap.
        assert_eq!(cut_surface_m(15.0, 10.0, 0., 2000), 13.0);
        // Nothing is ever raised: a pad below the natural is the only cut.
        assert_eq!(cut_surface_m(9.0, 10.0, 0., 2000), 9.0);
        // Filling stays the other contract's job.
        assert_eq!(cut_surface_m(8.0, 10.0, 0., 0), 8.0);
    }

    #[test]
    fn the_ring_rises_at_the_walkable_grade_and_meets_the_natural_surface() {
        let (base, target) = (12.0, 10.0);
        let mut previous = cut_surface_m(base, target, 0., 2000);
        for step in 1..=8 {
            let surface = cut_surface_m(base, target, f64::from(step), 2000);
            let rise = surface - previous;
            assert!(
                rise <= MAX_CUT_RING_GRADE + 1e-9,
                "step {step} rose {rise} m"
            );
            previous = surface;
        }
        assert_eq!(previous, base, "eight metres is the 2 m cap's whole run");
        assert_eq!(ring_width_m(2000), 8.0);
    }

    #[test]
    fn the_guards_refuse_protected_water_band_and_occupied_cells_one_by_one() {
        let (scene, zones) = scene();
        let mask = ZoningMask::new(&scene, &zones).unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::River,
            &[],
            &scene,
            &serde_json::json!({"waterType": "river", "water": {"centerX": 48, "centerZ": 32, "widthM": 12, "depthM": 64}}),
        );
        let occupied = |x: i32, z: i32| (x, z) == (10, 10);
        let guards = CellGuards {
            mask: &mask,
            water,
            occupied: &occupied,
            cut_footprints: &[],
        };
        assert_eq!(guards.blocks(0, 0), None);
        assert_eq!(guards.blocks(40, 10), Some(ON_PROTECTED));
        assert_eq!(guards.blocks(10, 10), Some(ON_OCCUPIED));
        // Every cell of the scene is decided, never sampled: the protected strip
        // is refused for its whole height, the water body for its whole width.
        for z in 0..64 {
            for x in 32..64 {
                assert_eq!(guards.blocks(x, z), Some(ON_PROTECTED), "({x},{z})");
            }
        }
        let mut water_cells = 0;
        for z in 0..64 {
            for x in 0..32 {
                if guards.blocks(x, z) == Some(IN_WATER) {
                    water_cells += 1;
                }
            }
        }
        assert!(water_cells > 0, "the fixture must carry water cells");
    }

    /// One pad's bounded cut, composed into a real raster the way the bake
    /// composes it, then measured, then verified against the plan and the natural
    /// raster alone. Every number below is an integer millimetre sum, so a
    /// forged record cannot hide in a rounding.
    fn cut_world() -> (
        super::super::SceneSpec,
        Vec<super::super::LandUseZone>,
        RoadsideSite,
        BakedTerrain,
        BakedTerrain,
        CutLedger,
        WaterGeometry,
    ) {
        let scene = super::super::SceneSpec {
            scene_id: "cut-raster".into(),
            origin_x: 0,
            origin_z: 0,
            width_m: 64,
            depth_m: 64,
            seed_offset: 0,
        };
        let zones = vec![
            super::super::LandUseZone {
                id: "urban".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 48,
                depth_m: 64,
                source: "test".into(),
            },
            super::super::LandUseZone {
                id: "nature".into(),
                kind: "nature".into(),
                world_x: 48,
                world_z: 0,
                width_m: 16,
                depth_m: 64,
                source: "test".into(),
            },
        ];
        // A 12x12 pad on a 2 m bank: the natural ground stands 2000 mm above the
        // pad's declared level everywhere inside it, so the whole flat top is a
        // cut at exactly the cap.
        let site = RoadsideSite {
            program: None,
            purpose: None,
            id: "generated.frontage.cut-raster.homes.0.negative".into(),
            road_id: "road".into(),
            world_x: 16,
            world_z: 16,
            width_m: 12,
            depth_m: 12,
            support_level_m: 0,
            parcel_insets: None,
            support_level_mm: Some(0),
            frontage_road_point: Some([16, 28]),
            bounded_cut_mm: Some(MAX_CUT_MM),
            earthwork_blend_m: None,
            rotation_y_deg: 0.,
            natural_min_m: 2.,
            natural_max_m: 2.,

            entry_corridor: None,
        };
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let mut values = vec![2.0_f64; 64 * 64];
        // The protected strip east of x=48 keeps its own ground; the pad never
        // reaches it.
        for z in 0..64 {
            for x in 48..64 {
                values[z * 64 + x] = 1.0;
            }
        }
        natural
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, values).unwrap(),
            )
            .unwrap();
        // Compose the final raster exactly as the bake does: the pad's carve list,
        // then `apply_terrain_carves` per cell.
        let entities = vec![site.entity()];
        let carves = super::super::plan_terrain_carves(&entities);
        let mut final_terrain = BakedTerrain::new(&scene).unwrap();
        let mut final_values = Vec::with_capacity(64 * 64);
        for z in 0..64 {
            for x in 0..64 {
                let base = f64::from(natural.height_millimetres(x, z).unwrap()) / 1000.;
                let value = super::super::apply_terrain_carves(base, x, z, &carves);
                final_values.push((value * 1000.).round() / 1000.);
            }
        }
        final_terrain
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, final_values).unwrap(),
            )
            .unwrap();
        let plan = vec![site.clone()];
        let pads = measure_from_rasters(&plan, &natural, &final_terrain, &[]).unwrap();
        let mut ledger = CutLedger {
            requested: Some(POLICY_VALUE.into()),
            enabled: true,
            not_enabled: None,
            max_cut_mm: Some(MAX_CUT_MM),
            pads,
            roads: vec![],
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
        };
        ledger.totals();
        (scene, zones, site, natural, final_terrain, ledger, water)
    }

    /// Requirement 2 + 3 + 6: the excavation is real, it is at the cap, it is
    /// exactly the plan's, and two identical runs produce identical bytes.
    #[test]
    fn the_ledger_measures_the_cut_the_plan_dictains_and_verifies_from_rasters() {
        let (scene, zones, site, natural, final_terrain, ledger, water) = cut_world();
        // 12x12 footprint plus the graded ring: every cell inside the flat top is
        // cut by the whole 2000 mm the bank stands above the pad.
        assert_eq!(ledger.pads.len(), 1);
        let pad = &ledger.pads[0];
        assert_eq!(pad.pad_id, site.id);
        assert_eq!(pad.design_level_mm, 0);
        assert_eq!(pad.max_cut_mm, MAX_CUT_MM);
        assert_eq!(pad.max_cut_depth_mm, MAX_CUT_MM);
        // The exact arithmetic of this fixture, cell by cell: a 17x17 flat top
        // (support_pad_half_extents = 8 m) at the whole 2000 mm, seven graded
        // rings, and the entity-owned 2 m door landing. The landing is part of
        // the same published support, so its extra excavation is booked here
        // rather than disappearing from the ledger.
        //   289 cells * 2000 mm                      =   578 000 mm
        //   rings 126+120+110+96+78+56+30 (thousand) =   616 000 mm
        //   door landing and its ring                =     4 750 mm over 19 cells
        //   total                                    = 1 218 000 mm over 980 cells
        assert_eq!(ledger.pads.len(), 1);
        let pad = &ledger.pads[0];
        assert_eq!(pad.pad_id, site.id);
        assert_eq!(pad.design_level_mm, 0);
        assert_eq!(pad.max_cut_mm, MAX_CUT_MM);
        assert_eq!(
            pad.max_cut_depth_mm, 2000,
            "the bank is exactly the cap deep"
        );
        assert_eq!(
            pad.cut_cells, 980,
            "the pad footprint plus the entity-owned door landing"
        );
        assert_eq!(pad.cut_depth_sum_mm, 1_218_000);
        assert_eq!(pad.cut_volume_m3, 1218.0, "980 cells of one square metre");
        assert_eq!(ledger.cut_cells, 980);
        assert_eq!(ledger.cut_depth_sum_mm, 1_218_000);
        assert_eq!(ledger.max_cut_depth_mm, 2000);
        let audit = verify(
            &scene,
            Some(&super::super::road_network::RoadNetworkPlan {
                road_cut_mm: None,
                roadside_sites: vec![site.clone()],
                ..serde_json::from_value(serde_json::json!({
                    "model": "cut", "naturalHeightfieldHash": natural.content_hash().unwrap(),
                    "sceneId": "cut-raster", "status": "planned_connected_network",
                    "gridSpacingM": 64, "maxRoadbedFillM": 6, "levelM": 0, "candidates": 1,
                    "rejectedWater": 0, "rejectedRelief": 0, "unselectedCandidates": 0,
                    "nodes": [], "edges": [], "roadsideSites": []
                }))
                .unwrap()
            }),
            &zones,
            water,
            &natural,
            &final_terrain,
            Some(&ledger),
            CutPolicy::Bounded {
                max_cut_mm: MAX_CUT_MM,
            },
        )
        .unwrap();
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.pads, 1);
        assert_eq!(audit.cut_cells, 980);
        assert_eq!(audit.cut_depth_sum_mm, 1_218_000);
        assert_eq!(audit.cut_volume_m3, 1218.0);
        assert_eq!(audit.max_cut_depth_mm, MAX_CUT_MM);
        assert_eq!(audit.protected_cells_changed, 0);
        assert_eq!(audit.water_cells_changed, 0);
        assert_eq!(audit.shore_band_cells_cut, 0);
        // The plan + natural recomputation reproduces the measured record exactly.
        let predicted = recompute_from_plan(&[site], &natural, &[]).unwrap();
        assert_eq!(predicted, ledger.pads);
        // Two runs, identical bytes (the ledger is derived, not accumulated).
        let (.., again, _) = {
            let second = cut_world();
            (
                second.0, second.1, second.2, second.3, second.4, second.5, second.6,
            )
        };
        assert_eq!(
            serde_json::to_vec(&ledger).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
    }

    /// Requirement 3: a forged record fails by name, one defect at a time.
    #[test]
    fn a_forged_ledger_is_refused_by_name_and_protected_and_water_cells_never_cut() {
        let (scene, zones, site, natural, final_terrain, ledger, water) = cut_world();
        let plan = super::super::road_network::RoadNetworkPlan {
            road_cut_mm: None,
            roadside_sites: vec![site.clone()],
            ..serde_json::from_value(serde_json::json!({
                "model": "cut", "naturalHeightfieldHash": natural.content_hash().unwrap(),
                "sceneId": "cut-raster", "status": "planned_connected_network",
                "gridSpacingM": 64, "maxRoadbedFillM": 6, "levelM": 0, "candidates": 1,
                "rejectedWater": 0, "rejectedRelief": 0, "unselectedCandidates": 0,
                "nodes": [], "edges": [], "roadsideSites": []
            }))
            .unwrap()
        };
        let policy = CutPolicy::Bounded {
            max_cut_mm: MAX_CUT_MM,
        };
        let run = |ledger: &CutLedger| {
            verify(
                &scene,
                Some(&plan),
                &zones,
                water,
                &natural,
                &final_terrain,
                Some(ledger),
                policy,
            )
            .unwrap()
        };
        assert_eq!(run(&ledger).status, "pass");
        // Under-report: three cut cells hidden.
        let mut under = ledger.clone();
        under.pads[0].cut_cells -= 3;
        let audit = run(&under);
        assert_eq!(audit.status, "fail");
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.starts_with(LEDGER_MISMATCH)),
            "{:?}",
            audit.failures
        );
        // Over-report: a bigger sum than the rasters hold.
        let mut over = ledger.clone();
        over.pads[0].cut_depth_sum_mm += 4000;
        over.pads[0].cut_volume_m3 += 4.0;
        let audit = run(&over);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.starts_with(LEDGER_MISMATCH)),
            "{:?}",
            audit.failures
        );
        // A cap above the maximum is refused even when every number matches.
        let mut greedy = ledger.clone();
        greedy.pads[0].max_cut_mm = 5000;
        let audit = run(&greedy);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.starts_with(CAP_ABOVE_MAXIMUM)),
            "{:?}",
            audit.failures
        );
        // The pad carries a cap but publishes no record.
        let empty = CutLedger {
            pads: vec![],
            roads: vec![],
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
            ..ledger.clone()
        };
        let audit = run(&empty);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.starts_with(PAD_NOT_IN_LEDGER)),
            "{:?}",
            audit.failures
        );
        // A cell cut outside any pad's footprint.
        let mut stray = BakedTerrain::new(&scene).unwrap();
        let mut values: Vec<f64> = (0..64)
            .flat_map(|z| (0..64).map(move |x| (x, z)))
            .map(|(x, z)| f64::from(final_terrain.height_millimetres(x, z).unwrap()) / 1000.)
            .collect();
        values[40 * 64 + 40] -= 0.2;
        stray
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, values).unwrap(),
            )
            .unwrap();
        let audit = verify(
            &scene,
            Some(&plan),
            &zones,
            water,
            &natural,
            &stray,
            Some(&ledger),
            policy,
        )
        .unwrap();
        assert!(
            audit.failures.iter().any(|f| f.starts_with(OUTSIDE_PAD)),
            "{:?}",
            audit.failures
        );
        // A cut pressed into PROTECTED ground: the same cell is refused by name,
        // per cell, not "most likely".
        let mut on_protected = BakedTerrain::new(&scene).unwrap();
        let mut values: Vec<f64> = (0..64)
            .flat_map(|z| (0..64).map(move |x| (x, z)))
            .map(|(x, z)| f64::from(final_terrain.height_millimetres(x, z).unwrap()) / 1000.)
            .collect();
        values[10 * 64 + 50] -= 0.1;
        on_protected
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, values).unwrap(),
            )
            .unwrap();
        let audit = verify(
            &scene,
            Some(&plan),
            &zones,
            water,
            &natural,
            &on_protected,
            Some(&ledger),
            policy,
        )
        .unwrap();
        assert_eq!(audit.protected_cells_changed, 1);
        assert!(
            audit.failures.iter().any(|f| f.starts_with(ON_PROTECTED)),
            "{:?}",
            audit.failures
        );
    }

    #[test]
    fn non_frontage_buildings_share_the_same_bounded_cut_ledger() {
        let (_scene, _zones, site, natural, final_terrain, ledger, _water) = cut_world();
        let mut entity = site.entity();
        entity.entity_id = "generated.settlement-evolution.weak-cut".into();
        entity.kind = "residential_block".into();
        let pads = measure_cut_entities(&[entity], &natural, &final_terrain, &[]).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(
            pads[0].cut_cells,
            ledger.pads[0].cut_cells,
            "the same entity-owned support carves must be booked outside frontage too"
        );
        assert_eq!(
            pads[0].cut_depth_sum_mm,
            ledger.pads[0].cut_depth_sum_mm
        );
    }

    /// Two benches may not share a cut footprint: the ledger's total is the sum of
    /// the per-pad records, so a shared cell would be booked once per pad. The
    /// audit refuses the pair BY NAME from the plan alone, and the same predicate
    /// is what the placement preflight asks before it accepts a candidate.
    #[test]
    fn two_pads_that_share_a_cut_footprint_fail_the_audit_by_name() {
        let (scene, zones, site, natural, _final, _ledger, water) = cut_world();
        // The first pad's footprint reaches x = 32; the second one, 12 m away, starts
        // at x = 12, so the two footprints share 21 columns of flat top and ring.
        let mut second = site.clone();
        second.id = "generated.frontage.cut-raster.homes.1.negative".into();
        second.world_x = 28;
        second.frontage_road_point = Some([28, 28]);
        let first_footprint = CutFootprint::of(&site.entity(), MAX_CUT_MM);
        let second_footprint = CutFootprint::of(&second.entity(), MAX_CUT_MM);
        let shared = footprint_intersection(&first_footprint, &second_footprint)
            .expect("the fixture's two benches overlap");
        assert_eq!(
            Some(shared),
            footprint_intersection(&second_footprint, &first_footprint),
            "the predicate is symmetric"
        );
        // Compose the final raster the way the bake does: both pads' carves.
        let entities = vec![site.entity(), second.entity()];
        let carves = super::super::plan_terrain_carves(&entities);
        let mut final_terrain = BakedTerrain::new(&scene).unwrap();
        let mut values = Vec::with_capacity(64 * 64);
        for z in 0..64 {
            for x in 0..64 {
                let base = f64::from(natural.height_millimetres(x, z).unwrap()) / 1000.;
                values.push(
                    (super::super::apply_terrain_carves(base, x, z, &carves) * 1000.).round()
                        / 1000.,
                );
            }
        }
        final_terrain
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, values).unwrap(),
            )
            .unwrap();
        let plan = super::super::road_network::RoadNetworkPlan {
            road_cut_mm: None,
            roadside_sites: vec![site.clone(), second.clone()],
            ..serde_json::from_value(serde_json::json!({
                "model": "cut", "naturalHeightfieldHash": natural.content_hash().unwrap(),
                "sceneId": "cut-raster", "status": "planned_connected_network",
                "gridSpacingM": 64, "maxRoadbedFillM": 6, "levelM": 0, "candidates": 1,
                "rejectedWater": 0, "rejectedRelief": 0, "unselectedCandidates": 0,
                "nodes": [], "edges": [], "roadsideSites": []
            }))
            .unwrap()
        };
        let pads =
            measure_from_rasters(&plan.roadside_sites, &natural, &final_terrain, &[]).unwrap();
        let mut ledger = CutLedger {
            requested: Some(POLICY_VALUE.into()),
            enabled: true,
            not_enabled: None,
            max_cut_mm: Some(MAX_CUT_MM),
            pads,
            roads: vec![],
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
        };
        ledger.totals();
        let audit = verify(
            &scene,
            Some(&plan),
            &zones,
            water,
            &natural,
            &final_terrain,
            Some(&ledger),
            CutPolicy::Bounded {
                max_cut_mm: MAX_CUT_MM,
            },
        )
        .unwrap();
        assert_eq!(audit.status, "fail");
        assert!(
            audit
                .failures
                .iter()
                .any(|failure| failure.starts_with(FOOTPRINT_OVERLAP)
                    && failure.contains(&site.id)
                    && failure.contains(&second.id)
                    && failure.contains(&format!("({},{})", shared.0, shared.1))),
            "the overlap must be named with both pads and the shared cell: {:?}",
            audit.failures
        );
    }

    // ==================== Round 41: the graded road's own bounded cut =========

    /// One graded road across a synthetic raster, plus the structure whose fill
    /// transition ring used to bury it.
    ///
    /// * the road is a flat 10 m profile east-west at z = 40 (56 m long, 12 m
    ///   wide), so its graded flat top is `x 12..=76, z 33..=47` and its transport
    ///   band `z 31..=49`;
    /// * the natural ground under the band is 9 m, so the road's own design fill is
    ///   1 m - the road does NOT overfill by itself;
    /// * the ground rises to 12 m on the uphill strip `z 30..=34`, which the road's
    ///   grading ring must excavate;
    /// * a 26x24 building at (44,70) at 18 m pushes a transition ring of ~17.93 m
    ///   over the road's downhill edge - the exact shape that buried the hillside
    ///   roads at 6.705 m / 7.150 m.
    fn road_cut_world() -> (
        super::super::SceneSpec,
        Vec<super::super::LandUseZone>,
        super::super::road_network::RoadNetworkPlan,
        EntityInstance,
        BakedTerrain,
        WaterGeometry,
    ) {
        let scene = super::super::SceneSpec {
            scene_id: "road-cut".into(),
            origin_x: 0,
            origin_z: 0,
            width_m: 96,
            depth_m: 96,
            seed_offset: 0,
        };
        let zones = vec![super::super::LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 96,
            depth_m: 96,
            source: "test".into(),
        }];
        let plan: super::super::road_network::RoadNetworkPlan =
            serde_json::from_value(serde_json::json!({
                "model": "fixture",
                "naturalHeightfieldHash": "source",
                "sceneId": "road-cut",
                "status": "planned",
                "gridSpacingM": 96,
                "maxRoadbedFillM": 6,
                "levelM": 10,
                "candidates": 1,
                "rejectedWater": 0,
                "rejectedRelief": 0,
                "unselectedCandidates": 0,
                "nodes": [
                    {"id": 0, "worldX": 16, "worldZ": 40},
                    {"id": 1, "worldX": 72, "worldZ": 40}
                ],
                "edges": [{
                    "id": "road.0",
                    "from": 0,
                    "to": 1,
                    "worldX": 44,
                    "worldZ": 40,
                    "lengthM": 56,
                    "rotationYDeg": 0.0,
                    "naturalMinM": 9.0,
                    "naturalMaxM": 12.0,
                    "profile": {
                        "version": 1,
                        "roadWidthM": 12,
                        "start": [16, 40],
                        "end": [72, 40],
                        "elevationsMm": vec![10_000_i64; 57],
                        "maxFillMm": 6000
                    }
                }],
                "roadsideSites": [],
                "roadCutMm": MAX_CUT_MM
            }))
            .expect("the road plan fixture parses");
        let building = fill_building("building.0", 44, 70, 18.0, 26.0, 24.0);
        let mut values = vec![9.0_f64; 96 * 96];
        for z in 30..=34 {
            for x in 0..96 {
                values[z * 96 + x] = 12.0;
            }
        }
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(
                0,
                0,
                96,
                96,
                &crate::height_payload::encode_millimetres(96, 96, values).unwrap(),
            )
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        (scene, zones, plan, building, natural, water)
    }

    fn fill_building(
        id: &str,
        x: i32,
        z: i32,
        level_m: f32,
        width_m: f32,
        depth_m: f32,
    ) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: format!("prop.{id}"),
            kind: "urban_building".into(),
            world_x: x,
            world_z: z,
            world_y: level_m.floor() as i32,
            scale: 1.,
            width_m,
            depth_m,
            height_m: 10.,
            rotation_y_deg: 0.,
            grounding: super::super::GroundingSpec {
                bottom_offset_m: level_m - level_m.floor(),
                ..Default::default()
            },
            anchors: vec![],
            bounds: None,
        }
    }

    /// The raster the bake publishes for this plan and these extra structures:
    /// one cell at a time through the production composition
    /// (`published_surface_from_raster`), never a second model of it.
    fn publish_road_scene(
        scene: &super::super::SceneSpec,
        zones: &[super::super::LandUseZone],
        plan: &super::super::road_network::RoadNetworkPlan,
        extra: &[EntityInstance],
        natural: &BakedTerrain,
        water: WaterGeometry,
    ) -> BakedTerrain {
        let mask = ZoningMask::new(scene, zones).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        entities.extend(extra.iter().cloned());
        let carves = super::super::plan_terrain_carves(&entities);
        let mut values = Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
        for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
            for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                values.push(
                    super::super::published_surface_from_raster(
                        natural,
                        Some(&mask),
                        water,
                        &carves,
                        x,
                        z,
                    )
                    .expect("the natural raster covers the scene"),
                );
            }
        }
        let mut final_terrain = BakedTerrain::new(scene).unwrap();
        final_terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &crate::height_payload::encode_millimetres(scene.width_m, scene.depth_m, values)
                    .unwrap(),
            )
            .unwrap();
        final_terrain
    }

    fn road_ledger(
        scene: &super::super::SceneSpec,
        plan: &super::super::road_network::RoadNetworkPlan,
        natural: &BakedTerrain,
        final_terrain: &BakedTerrain,
        policy: CutPolicy,
    ) -> CutLedger {
        let pads = measure_from_rasters(&plan.roadside_sites, natural, final_terrain, &[]).unwrap();
        let roads = measure_road_cuts(scene, plan, natural, final_terrain, policy).unwrap();
        let mut ledger = CutLedger {
            requested: Some(POLICY_VALUE.into()),
            enabled: true,
            not_enabled: None,
            max_cut_mm: Some(MAX_CUT_MM),
            pads,
            roads,
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
        };
        ledger.totals();
        ledger
    }

    fn mm(terrain: &BakedTerrain, x: i32, z: i32) -> i32 {
        terrain.height_millimetres(x, z).expect("inside the scene")
    }

    /// Requirement 1 + 6: a graded road's cut clears the fill that used to stand
    /// above it, is bounded by the one cap, and is booked in the SAME ledger under
    /// the road's own id. The off arm is the historical fill-only road.
    #[test]
    fn a_graded_road_cut_clears_the_overfill_above_it_and_the_ledger_books_the_depth() {
        let (scene, zones, plan, building, natural, water) = road_cut_world();
        let policy = CutPolicy::Bounded {
            max_cut_mm: MAX_CUT_MM,
        };
        let on = publish_road_scene(
            &scene,
            &zones,
            &plan,
            std::slice::from_ref(&building),
            &natural,
            water,
        );
        // The road's own band: (44,49) is the transport edge the building's ring
        // used to bury; (44,33) is the graded flat top's uphill edge.
        assert_eq!(mm(&natural, 44, 49), 9_000);
        assert_eq!(mm(&on, 44, 49), 10_000, "the road's authored surface wins");
        assert_eq!(
            mm(&on, 44, 49) - mm(&natural, 44, 49),
            1_000,
            "1 m of the road's own design fill, inside its 6 m contract"
        );
        assert_eq!(mm(&natural, 44, 33), 12_000);
        assert_eq!(
            mm(&on, 44, 33),
            10_000,
            "the uphill edge is excavated to the profile"
        );
        // The off arm is the historical fill-only road: the same building ring
        // stands ~8.9 m above the natural ground at the transport edge (the
        // failure Round 38 measured as 6.705 m / 7.150 m on its own world), and no
        // cut is taken at the natural 12 m strip.
        let mut off_plan = plan.clone();
        off_plan.road_cut_mm = None;
        let off = publish_road_scene(
            &scene,
            &zones,
            &off_plan,
            std::slice::from_ref(&building),
            &natural,
            water,
        );
        let off_fill = mm(&off, 44, 49) - mm(&natural, 44, 49);
        assert!(
            off_fill > 6_000,
            "without the cap the neighbour's fill is the surface: {off_fill} mm"
        );
        assert_eq!(
            off_fill, 8_926,
            "the measured fill this road is rescued from"
        );
        assert_eq!(
            mm(&off, 44, 33),
            12_000,
            "no excavation at all when off; the natural strip stands"
        );
        assert!(
            measure_road_cuts(&scene, &off_plan, &natural, &off, policy)
                .unwrap()
                .is_empty()
        );

        let ledger = road_ledger(&scene, &plan, &natural, &on, policy);
        assert_eq!(ledger.pads.len(), 0, "this fixture has no frontage pad");
        assert_eq!(ledger.roads.len(), 1, "one record, keyed by the road id");
        let record = ledger.road("road.0").expect("the road is booked");
        assert_eq!(record.entity_id, "road.0");
        assert_eq!(record.max_cut_mm, MAX_CUT_MM);
        assert_eq!(record.design_level_mm, 10_000);
        assert_eq!(record.max_cut_depth_mm, MAX_CUT_MM);
        assert!(record.cut_cells > 0);
        assert!(record.cut_depth_sum_mm >= i64::from(record.max_cut_depth_mm));
        assert_eq!(record.cut_volume_m3, record.cut_depth_sum_mm as f64 / 1000.);
        assert_eq!(ledger.cut_cells, u64::from(record.cut_cells));
        assert_eq!(ledger.cut_depth_sum_mm, record.cut_depth_sum_mm);

        let audit = verify(
            &scene,
            Some(&plan),
            &zones,
            water,
            &natural,
            &on,
            Some(&ledger),
            policy,
        )
        .expect("the audit runs");
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.roads, 1);
        assert_eq!(audit.cut_cells, ledger.cut_cells);
        assert_eq!(audit.max_cut_depth_mm, MAX_CUT_MM);
        // The ledger total is the scene's own lowered cells, once.
        let mut scene_cells = 0_u64;
        for z in 0..96 {
            for x in 0..96 {
                if mm(&on, x, z) < mm(&natural, x, z) {
                    scene_cells += 1;
                }
            }
        }
        assert_eq!(ledger.cut_cells, scene_cells);
        assert_eq!(audit.protected_cells_changed, 0);
        assert_eq!(audit.water_cells_changed, 0);
        assert_eq!(audit.shore_band_cells_cut, 0);
    }

    /// Requirement 1: the per-cell prohibitions hold for a ROAD too - protected
    /// land, the water body, the shoreline band and another entity's support all
    /// keep their own surface, cell by cell, while the clean cells are excavated.
    #[test]
    fn a_graded_road_cut_never_enters_protected_water_shore_or_an_occupied_cell() {
        let (scene, _, plan, building, natural, _) = road_cut_world();
        let zones = vec![
            super::super::LandUseZone {
                id: "urban".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 80,
                depth_m: 96,
                source: "test".into(),
            },
            super::super::LandUseZone {
                id: "nature".into(),
                kind: "nature".into(),
                world_x: 80,
                world_z: 0,
                width_m: 16,
                depth_m: 96,
                source: "test".into(),
            },
        ];
        // A river through the middle of the scene (the 96 m fallback body: centre
        // x = 48, half width ~13.4 m) and a fill-only structure east of it whose
        // own support rectangle claims the middle of the uphill ring.
        let water = super::super::water_geometry(
            super::super::WaterKind::River,
            &[],
            &scene,
            &serde_json::json!({"waterType": "river"}),
        );
        assert!(
            water.contains_point(44., 33.),
            "the fixture's river covers x=44"
        );
        assert!(!water.contains_point(20., 33.), "and not x=20");
        let occupied = fill_building("occupied.0", 70, 24, 17.0, 12.0, 12.0);
        let on = publish_road_scene(
            &scene,
            &zones,
            &plan,
            &[building.clone(), occupied.clone()],
            &natural,
            water,
        );
        let policy = CutPolicy::Bounded {
            max_cut_mm: MAX_CUT_MM,
        };
        // A clean cell of the same uphill strip is excavated at the full cap.
        assert_eq!(mm(&natural, 20, 33), 12_000);
        assert_eq!(mm(&on, 20, 33), 10_000);
        assert_eq!(mm(&natural, 20, 33) - mm(&on, 20, 33), 2_000);
        // Protected (zoning) and water keep their natural surfaces.
        assert_eq!(mm(&on, 82, 33), mm(&natural, 82, 33));
        assert_eq!(mm(&on, 44, 33), mm(&natural, 44, 33));
        // The occupied support rectangle is not excavated either: the structure's
        // own fill stands (17 m), which is a fill, not a cut.
        assert_eq!(mm(&on, 70, 31), 17_000);
        assert!(mm(&on, 70, 31) > mm(&natural, 70, 31));

        let ledger = road_ledger(&scene, &plan, &natural, &on, policy);
        let audit = verify(
            &scene,
            Some(&plan),
            &zones,
            water,
            &natural,
            &on,
            Some(&ledger),
            policy,
        )
        .expect("the audit runs");
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.protected_cells_changed, 0);
        assert_eq!(audit.water_cells_changed, 0);
        assert_eq!(audit.shore_band_cells_cut, 0);
        assert!(audit.cut_cells > 0);
        // The audit's own road owner map names the clean cells and reports a
        // cap-carrying pad's support window as the occupied guard.
        let road_plan = road_cut_plan(&scene, &plan, &natural, policy).unwrap();
        let slot = road_plan
            .owner_at(&scene, 20, 33)
            .expect("the clean cut is road-owned");
        assert_eq!(road_plan.blocked_for(slot, 20, 33), None);
        let mut guarded = plan.clone();
        guarded.roadside_sites.push(
            serde_json::from_value(serde_json::json!({
                "id": "occupied.pad",
                "roadId": "road.0",
                "worldX": 70,
                "worldZ": 24,
                "widthM": 12,
                "depthM": 12,
                "supportLevelM": 17,
                "supportLevelMm": 17000,
                "boundedCutMm": MAX_CUT_MM,
                "rotationYDeg": 0.0,
                "naturalMinM": 9.0,
                "naturalMaxM": 12.0
            }))
            .unwrap(),
        );
        let guarded_plan = road_cut_plan(&scene, &guarded, &natural, policy).unwrap();
        let guarded_slot = guarded_plan
            .owner_at(&scene, 20, 33)
            .expect("the clean cut is still road-owned");
        assert_eq!(
            guarded_plan.blocked_for(guarded_slot, 70, 31),
            Some(ON_OCCUPIED),
            "the occupied support rectangle is a window, not a cut"
        );
    }
}
