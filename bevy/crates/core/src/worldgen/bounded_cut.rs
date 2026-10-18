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
//! Exactly the FRONTAGE PAD: the entity `frontage::append` accepted for a
//! `style.frontageProgram` request, its entrance apron and its frontage-point
//! apron (`worldgen::plan_terrain_carves` marks those three carves with the
//! entity's own cap). Roads, their graded bands, roadside pads of the settlement
//! / service / joint programmes, the shoreline quay row, landmarks and every
//! authored entity never carry a cap and therefore never cut.
//!
//! What may never be cut, per cell
//! -------------------------------
//! * protected land (`zoning::ZoningMask::protected_at`),
//! * the structured water body (`WaterGeometry::contains_point`),
//! * the shoreline public passage band (`WaterGeometry::shore_band_cell`),
//! * a cell any already accepted entity occupies,
//! * a cell a road's own graded band fully covers (`priority == 10`),
//! * a cell inside another accepted pad's [`CutFootprint`]: two benches may not
//!   share a footprint, because the ledger books every pad's excavation from the
//!   same cells and an overlap would count one cell twice.
//!
//! The generator guards those cells one by one before a pad is accepted
//! ([`CellGuards`]) and the audit re-derives them from the published bytes
//! ([`verify`]), so neither side can be satisfied by the other's promise.
//!
//! The ONE cap
//! -----------
//! [`MAX_CUT_MM`] is read by the style parser, by `frontage::support_check`, by
//! `worldgen::apply_terrain_carves` (through [`clamp_cap`]) and by this module's
//! audit. There is no second number.

use super::road_network::RoadsideSite;
use super::zoning::ZoningMask;
use super::{EntityInstance, WaterGeometry, spatial_half_extents, support_pad_half_extents};
use crate::baked_terrain::BakedTerrain;
use serde::{Deserialize, Serialize};

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
        if self.water.contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5) {
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

/// A cell-footprint predicate over a list of entities, in their published
/// geometry. Rounded the way `frontage::bounds` rounds, so the occupancy test is
/// the same window the placement preflight already refuses.
pub fn entity_occupies(entities: &[EntityInstance]) -> impl Fn(i32, i32) -> bool + '_ {
    move |x: i32, z: i32| {
        entities.iter().any(|e| {
            let (hw, hd) = spatial_half_extents(e.width_m * e.scale, e.depth_m * e.scale, e.rotation_y_deg);
            (f64::from(x) - f64::from(e.world_x)).abs() <= hw
                && (f64::from(z) - f64::from(e.world_z)).abs() <= hd
        })
    }
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

/// What the cut did to one world, per pad and in total.
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
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
        }
    }

    pub fn totals(&mut self) {
        self.cut_cells = self.pads.iter().map(|pad| u64::from(pad.cut_cells)).sum();
        self.cut_depth_sum_mm = self.pads.iter().map(|pad| pad.cut_depth_sum_mm).sum();
        self.cut_volume_m3 = self.cut_depth_sum_mm as f64 / 1000.;
        self.max_cut_depth_mm = self.pads.iter().map(|pad| pad.max_cut_depth_mm).max().unwrap_or(0);
    }

    pub fn pad(&self, pad_id: &str) -> Option<&CutPad> {
        self.pads.iter().find(|pad| pad.pad_id == pad_id)
    }
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
        let footprint = CutFootprint::of(&entity, cap);
        let (mut cells, mut sum, mut max) = (0_u32, 0_i64, 0_i32);
        for z in footprint.top..=footprint.bottom {
            for x in footprint.left..=footprint.right {
                if !footprint.contains(x, z) {
                    continue;
                }
                let Some(base) = natural.height_millimetres(x, z) else {
                    continue;
                };
                let depth = cut_depth_mm(
                    f64::from(base) / 1000.,
                    footprint.target_m,
                    footprint.outside_m(x, z),
                    footprint.cap_mm,
                );
                if depth <= 0 {
                    continue;
                }
                cells += 1;
                sum += depth;
                max = max.max(depth as i32);
            }
        }
        pads.push(CutPad {
            pad_id: site.id.clone(),
            entity_id: site.id.clone(),
            design_level_mm: level,
            max_cut_mm: footprint.cap_mm,
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
        let footprint = CutFootprint::of(&entity, cap);
        let (mut cells, mut sum, mut max) = (0_u32, 0_i64, 0_i32);
        for z in footprint.top..=footprint.bottom {
            for x in footprint.left..=footprint.right {
                if !footprint.contains(x, z) {
                    continue;
                }
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
                cells += 1;
                sum += depth;
                max = max.max(depth as i32);
            }
        }
        pads.push(CutPad {
            pad_id: site.id.clone(),
            entity_id: site.id.clone(),
            design_level_mm: level,
            max_cut_mm: footprint.cap_mm,
            cut_cells: cells,
            cut_depth_sum_mm: sum,
            max_cut_depth_mm: max,
            cut_volume_m3: sum as f64 / 1000.,
        });
    }
    Ok(pads)
}

/// The audit of one world's bounded cut, recomputed from published bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutAudit {
    pub status: String,
    pub enabled: bool,
    pub pads: usize,
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
    let mut audit = CutAudit::new(policy.enabled());
    let sites: &[RoadsideSite] = plan.map_or(&[], |plan| plan.roadside_sites.as_slice());
    // 1. The policy and its record agree.
    match (policy.enabled(), ledger) {
        (true, None) => {
            audit.failures.push(format!(
                "{LEDGER_MISSING}: the policy is enabled but no bounded-cut ledger was published"
            ));
        }
        (false, Some(published)) if published.enabled || !published.pads.is_empty() => {
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
        match sites.iter().find(|site| site.id == pad.pad_id) {
            None => audit
                .failures
                .push(format!("{UNKNOWN_PAD}: {} is not a pad of this plan", pad.pad_id)),
            Some(site) => {
                if site.bounded_cut_mm != Some(pad.max_cut_mm) {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: pad {} publishes a {} mm cap but the plan declares {:?}",
                        pad.pad_id, pad.max_cut_mm, site.bounded_cut_mm
                    ));
                }
                if site.support_level_mm != Some(pad.design_level_mm) {
                    audit.failures.push(format!(
                        "{LEDGER_MISMATCH}: pad {} publishes design level {} mm but the plan \
                         declares {:?}",
                        pad.pad_id, pad.design_level_mm, site.support_level_mm
                    ));
                }
            }
        }
    }
    // 3. The plan's own pads all have a record.
    for site in sites.iter().filter(|site| site.bounded_cut_mm.is_some()) {
        if !published_pads.iter().any(|pad| pad.pad_id == site.id) {
            audit.failures.push(format!(
                "{PAD_NOT_IN_LEDGER}: pad {} carries a bounded-cut cap but no published record",
                site.id
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
    // 4a-pre. No two pads may share a cut footprint. The per-pad measurement below
    //     is what the ledger TOTAL is made of, so a shared cell would be booked once
    //     per pad and the total would exceed the excavation the published rasters
    //     actually hold. The placement preflight refuses such a pair by name; this
    //     re-derives the same rule from the plan a reader holds, so neither side can
    //     pass alone.
    let mut cut_footprints: Vec<(&str, CutFootprint)> = Vec::new();
    for site in sites.iter().filter(|site| site.bounded_cut_mm.is_some()) {
        let cap = site.bounded_cut_mm.unwrap_or(0);
        cut_footprints.push((site.id.as_str(), CutFootprint::of(&site.entity(), cap)));
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
    for site in sites {
        let Some(cap) = site.bounded_cut_mm else {
            continue;
        };
        let footprint = CutFootprint::of(&site.entity(), cap);
        let slot = footprints.len() as i32;
        footprints.push(footprint);
        for z in footprint.top..=footprint.bottom {
            for x in footprint.left..=footprint.right {
                if !footprint.contains(x, z) {
                    continue;
                }
                let Some(at) = index(x, z) else { continue };
                let Some(base) = natural.height_millimetres(x, z) else {
                    continue;
                };
                let expected = footprint.depth_mm(f64::from(base) / 1000., x, z);
                if expected > owner_depth_mm[at] {
                    owner_depth_mm[at] = expected;
                    owner[at] = slot;
                }
            }
        }
    }
    // The measured per-pad record, straight from the two rasters.
    let measured = measure_from_rasters(sites, natural, final_terrain)?;
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
            ("designLevelMm", pad.design_level_mm == truth.design_level_mm),
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
                audit.failures.push(format!(
                    "{OUTSIDE_PAD}: cell ({x},{z}) was lowered by {cut_mm} mm but no \
                     bounded-cut pad owns it"
                ));
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
            let expected = cut_depth_mm(
                f64::from(base) / 1000.,
                footprint.target_m,
                footprint.outside_m(x, z),
                footprint.cap_mm,
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
    //    is what keeps the 8 % / 12 % footprint contracts satisfied.
    for site in sites.iter().filter(|site| site.bounded_cut_mm.is_some()) {
        let Some(level) = site.support_level_mm else {
            continue;
        };
        let footprint = CutFootprint::of(&site.entity(), site.bounded_cut_mm.unwrap_or(0));
        let (hw, hd) = spatial_half_extents(site.width_m as f32, site.depth_m as f32, site.rotation_y_deg);
        let (hw, hd) = (hw.floor() as i32, hd.floor() as i32);
        let mut bad = 0_u32;
        for z in site.world_z - hd..=site.world_z + hd {
            for x in site.world_x - hw..=site.world_x + hw {
                let _ = &footprint;
                match final_terrain.height_millimetres(x, z) {
                    Some(actual) if actual == level => {}
                    _ => bad += 1,
                }
            }
        }
        if bad > 0 {
            audit.failures.push(format!(
                "{PAD_TOP_NOT_FLAT}: pad {} holds {bad} footprint cells away from its declared \
                 {} mm top",
                site.id, level
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
                            .find(|site| CutFootprint::of(&site.entity(), site.bounded_cut_mm.unwrap_or(0)) == *footprint)
                            .map(|site| site.id.as_str())
                            .unwrap_or("<unknown>")
                    ));
                }
            }
        }
    }
    audit.pads = measured.len();
    audit.cut_cells = measured.iter().map(|pad| u64::from(pad.cut_cells)).sum();
    audit.cut_depth_sum_mm = measured.iter().map(|pad| pad.cut_depth_sum_mm).sum();
    audit.cut_volume_m3 = audit.cut_depth_sum_mm as f64 / 1000.;
    audit.max_cut_depth_mm = measured.iter().map(|pad| pad.max_cut_depth_mm).max().unwrap_or(0);
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
            rotation_y_deg: 0.,
            natural_min_m: 2.,
            natural_max_m: 2.,
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
            .insert_chunk(0, 0, 64, 64, &crate::height_payload::encode_millimetres(64, 64, values).unwrap())
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
            .insert_chunk(0, 0, 64, 64, &crate::height_payload::encode_millimetres(64, 64, final_values).unwrap())
            .unwrap();
        let plan = vec![site.clone()];
        let pads = measure_from_rasters(&plan, &natural, &final_terrain).unwrap();
        let mut ledger = CutLedger {
            requested: Some(POLICY_VALUE.into()),
            enabled: true,
            not_enabled: None,
            max_cut_mm: Some(MAX_CUT_MM),
            pads,
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
        // (support_pad_half_extents = 8 m) at the whole 2000 mm, then seven graded
        // rings of 72/80/88/96/104/112/120 cells at 1750/1500/.../250 mm.
        //   289 cells * 2000 mm                      =   578 000 mm
        //   rings 126+120+110+96+78+56+30 (thousand) =   616 000 mm
        //   total                                    = 1 194 000 mm over 961 cells
        assert_eq!(ledger.pads.len(), 1);
        let pad = &ledger.pads[0];
        assert_eq!(pad.pad_id, site.id);
        assert_eq!(pad.design_level_mm, 0);
        assert_eq!(pad.max_cut_mm, MAX_CUT_MM);
        assert_eq!(pad.max_cut_depth_mm, 2000, "the bank is exactly the cap deep");
        assert_eq!(pad.cut_cells, 961, "289 flat-top cells plus 672 ring cells");
        assert_eq!(pad.cut_depth_sum_mm, 1_194_000);
        assert_eq!(pad.cut_volume_m3, 1194.0, "961 cells of one square metre");
        assert_eq!(ledger.cut_cells, 961);
        assert_eq!(ledger.cut_depth_sum_mm, 1_194_000);
        assert_eq!(ledger.max_cut_depth_mm, 2000);
        let audit = verify(
            &scene,
            Some(&super::super::road_network::RoadNetworkPlan {
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
            CutPolicy::Bounded { max_cut_mm: MAX_CUT_MM },
        )
        .unwrap();
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.pads, 1);
        assert_eq!(audit.cut_cells, 961);
        assert_eq!(audit.cut_depth_sum_mm, 1_194_000);
        assert_eq!(audit.cut_volume_m3, 1194.0);
        assert_eq!(audit.max_cut_depth_mm, MAX_CUT_MM);
        assert_eq!(audit.protected_cells_changed, 0);
        assert_eq!(audit.water_cells_changed, 0);
        assert_eq!(audit.shore_band_cells_cut, 0);
        // The plan + natural recomputation reproduces the measured record exactly.
        let predicted = recompute_from_plan(&[site], &natural).unwrap();
        assert_eq!(predicted, ledger.pads);
        // Two runs, identical bytes (the ledger is derived, not accumulated).
        let (.., again, _) = {
            let second = cut_world();
            (second.0, second.1, second.2, second.3, second.4, second.5, second.6)
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
        let policy = CutPolicy::Bounded { max_cut_mm: MAX_CUT_MM };
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
            audit.failures.iter().any(|f| f.starts_with(LEDGER_MISMATCH)),
            "{:?}",
            audit.failures
        );
        // Over-report: a bigger sum than the rasters hold.
        let mut over = ledger.clone();
        over.pads[0].cut_depth_sum_mm += 4000;
        over.pads[0].cut_volume_m3 += 4.0;
        let audit = run(&over);
        assert!(audit.failures.iter().any(|f| f.starts_with(LEDGER_MISMATCH)), "{:?}", audit.failures);
        // A cap above the maximum is refused even when every number matches.
        let mut greedy = ledger.clone();
        greedy.pads[0].max_cut_mm = 5000;
        let audit = run(&greedy);
        assert!(
            audit.failures.iter().any(|f| f.starts_with(CAP_ABOVE_MAXIMUM)),
            "{:?}",
            audit.failures
        );
        // The pad carries a cap but publishes no record.
        let empty = CutLedger {
            pads: vec![],
            cut_cells: 0,
            cut_depth_sum_mm: 0,
            cut_volume_m3: 0.,
            max_cut_depth_mm: 0,
            ..ledger.clone()
        };
        let audit = run(&empty);
        assert!(
            audit.failures.iter().any(|f| f.starts_with(PAD_NOT_IN_LEDGER)),
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
            .insert_chunk(0, 0, 64, 64, &crate::height_payload::encode_millimetres(64, 64, values).unwrap())
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
            .insert_chunk(0, 0, 64, 64, &crate::height_payload::encode_millimetres(64, 64, values).unwrap())
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
                values.push((super::super::apply_terrain_carves(base, x, z, &carves) * 1000.).round() / 1000.);
            }
        }
        final_terrain
            .insert_chunk(0, 0, 64, 64, &crate::height_payload::encode_millimetres(64, 64, values).unwrap())
            .unwrap();
        let plan = super::super::road_network::RoadNetworkPlan {
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
        let pads = measure_from_rasters(&plan.roadside_sites, &natural, &final_terrain).unwrap();
        let mut ledger = CutLedger {
            requested: Some(POLICY_VALUE.into()),
            enabled: true,
            not_enabled: None,
            max_cut_mm: Some(MAX_CUT_MM),
            pads,
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
            CutPolicy::Bounded { max_cut_mm: MAX_CUT_MM },
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
}
