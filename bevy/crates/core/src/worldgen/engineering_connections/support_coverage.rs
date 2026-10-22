//! Complete walking-band coverage for finite rectangular support parts.
//!
//! What is proved here is not a narrowest width at sampled stations. The object is the
//! closed walking band of one straight segment on one effective support layer:
//!
//! ```text
//!     P(t, s) = a + t*d + s*n,   t in [0, 1],   s in [-r, r]
//! ```
//!
//! Every finite rectangle of that layer becomes a convex polygon in the (t, s) frame.
//! The event set is: every polygon vertex, every crossing of a polygon boundary with
//! `s = -r`, `s = 0` and `s = r`, and every pairwise boundary crossing. Between two
//! consecutive events the polygons that own a section, the order of their boundary
//! functions and the way their intervals merge are all constant, so one representative
//! section decides the whole open sub-interval; every event section is inspected as well,
//! so the closed segment is decided by the certificate. A section certifies when the
//! union of the real intervals covers `[-r, r]` as one connected component (the union
//! bounds are `min(low)`, `max(high)`, never an intersection). Anything unproved stays a
//! gap with a witness point, or an explicit indeterminate, never `carried`.
//!
//! This module is pure 2D geometry over the frames it is given: the caller decides which
//! parts belong to the effective layer, and never merges different layers here.

use super::{MemberFrame, Vec3};
use serde::Serialize;

/// Hard cap on generated events for one segment, checked before generation.
pub const MAX_COVERAGE_EVENTS: usize = 65_536;

/// The event/section tolerance: only exact duplicates are merged with it, and it never
/// closes a geometric gap.
const EXACT_EPS: f64 = 1e-12;

/// The filter margin used when a part is excluded before the expensive event scan. A
/// part is dropped only when its own rectangle is provably clear of the closed band by
/// more than this margin; contact and floating-point ambiguity keeps the part.
const BAND_FILTER_EPS: f64 = 1e-9;

/// Bounded stations used only to decide a one-sided bearing identity when the coverage
/// evaluation itself cannot be afforded. A hit really does prove `structure` at a real
/// station of the segment; a miss proves nothing, so the identity stays `unknown`.
const PROBE_STATIONS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// The reason a bearing identity or a coverage result is not decisive. A failure reason
/// is recorded next to the state instead of being folded into "no structural support".
pub const REASON_DECIDED: &str = "decided";
pub const REASON_NO_CANDIDATE_PARTS_PROVEN: &str = "no_candidate_parts_proven";
pub const REASON_COVERAGE_BUDGET_EXHAUSTED: &str = "coverage_budget_exhausted";
pub const REASON_INVALID_FRAME: &str = "invalid_frame";
pub const REASON_NUMERICAL_UNCERTAIN: &str = "numerical_uncertain";
pub const REASON_OUTSIDE_CLOSED_BAND: &str = "outside_closed_walking_band";
/// The walking line itself could not be evaluated: it was empty, a single point, fully
/// degenerate, or it carried a non-finite vertex. Nothing was measured, so this is an
/// explicit invalid-path state and never a "no structure here" proof.
pub const REASON_INVALID_PATH: &str = "invalid_path";

/// Whether a support frame is usable at all: finite own geometry, unit and independent plan
/// axes, strictly positive halves.
///
/// The wrapper layers place a part into a layer by comparing `frame.top` with the walking
/// level, so a part whose own geometry cannot be trusted must be rejected **before** that
/// comparison: a non-finite `top` simply fails the comparison and the part would be dropped
/// in silence, turning "we cannot classify this part" into the false proof "no structure is
/// here". This predicate is the single source of that judgement for both the wrapper and
/// [`certify_band`].
pub fn frame_is_usable(frame: &MemberFrame) -> bool {
    let along = frame.along.xz_len();
    let across = frame.across.xz_len();
    let axes_unit = (along - 1.0).abs() <= 1e-6 && (across - 1.0).abs() <= 1e-6;
    let independent =
        (frame.along.x * frame.across.x + frame.along.z * frame.across.z).abs() <= 1e-6;
    frame.centre.finite()
        && frame.along.finite()
        && frame.across.finite()
        && frame.top.is_finite()
        && frame.half_length.is_finite()
        && frame.half_width.is_finite()
        && frame.half_length > 0.0
        && frame.half_width > 0.0
        && along > 1e-9
        && across > 1e-9
        && axes_unit
        && independent
}

/// The complete-band coverage result. It is one axis of the answer and never stands in
/// for the bearing identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverageVerdict {
    /// The required band is proved covered over the whole closed sub-interval.
    Certified,
    /// A real uncovered point inside the required band was found.
    Gap,
    /// Degenerate input, exhausted budget or unresolved ordering: nothing is claimed.
    Indeterminate,
    /// The interval was completely evaluated and holds no bearing candidate at all, so
    /// no structural certificate is owed here. Only this decisive result may use it: an
    /// invalid frame, an exhausted budget or a numerical doubt is never `NotApplicable`.
    NotApplicable,
}

impl CoverageVerdict {
    pub fn name(self) -> &'static str {
        match self {
            CoverageVerdict::Certified => "certified",
            CoverageVerdict::Gap => "gap",
            CoverageVerdict::Indeterminate => "indeterminate",
            CoverageVerdict::NotApplicable => "notApplicable",
        }
    }

    pub fn from_name(name: &str) -> Option<CoverageVerdict> {
        match name {
            "certified" => Some(CoverageVerdict::Certified),
            "gap" => Some(CoverageVerdict::Gap),
            "indeterminate" => Some(CoverageVerdict::Indeterminate),
            "notApplicable" => Some(CoverageVerdict::NotApplicable),
            _ => None,
        }
    }
}

/// Whether a real part of the effective layer bears the *centre line* of this interval.
/// It is independent of whether the full walking band is wide enough: a narrow deck
/// bears its centre line and is `Structure` with a `Gap`, never "no support needed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BearingIdentity {
    /// A real part of the effective layer carries the centre line here.
    Structure,
    /// A complete evaluation proved that no real part carries the centre line here.
    NoStructuralSupport,
    /// The identity could not be decided; nothing is claimed and nothing is exempted.
    Unknown,
}

impl BearingIdentity {
    pub fn name(self) -> &'static str {
        match self {
            BearingIdentity::Structure => "structure",
            BearingIdentity::NoStructuralSupport => "noStructuralSupport",
            BearingIdentity::Unknown => "unknown",
        }
    }

    pub fn from_name(name: &str) -> Option<BearingIdentity> {
        match name {
            "structure" => Some(BearingIdentity::Structure),
            "noStructuralSupport" => Some(BearingIdentity::NoStructuralSupport),
            "unknown" => Some(BearingIdentity::Unknown),
            _ => None,
        }
    }
}

/// One support candidate that was excluded before event generation, with the stable part
/// id, its own layer and the reason it cannot touch the closed walking band. The
/// exclusion is an optimisation: it is recorded and independently recomputable, and it
/// never turns a rejection into a pass.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredPart {
    pub segment: usize,
    pub part: String,
    pub level_m: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageWitness {
    pub t: f64,
    pub s: f64,
    pub covered_half_width_m: f64,
    pub required_half_width_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageSlice {
    pub from_t: f64,
    pub to_t: f64,
    pub effective_level_m: Option<f64>,
    pub adjacent_levels_m: [f64; 2],
    /// The real parts whose own interval lies inside the covering component.
    pub parts: Vec<String>,
    /// Whether a real member of this layer carries the centre line at the representative
    /// section. Kept as a diagnostic: the decision is taken on `bearing`, never on this
    /// boolean alone.
    pub centre_borne: bool,
    /// The bearing identity of this section: the independent first axis.
    pub bearing: BearingIdentity,
    /// Why the bearing identity is `Unknown`, or `decided`. Recorded, never inferred
    /// from a boolean.
    pub bearing_reason: String,
    /// Why the coverage result is `Indeterminate`, or `decided`.
    pub coverage_reason: String,
    pub verdict: CoverageVerdict,
    pub certified_band_m: f64,
    pub required_band_m: f64,
    pub witness: Option<CoverageWitness>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BandCertificate {
    pub segment: usize,
    pub required_band_m: f64,
    /// The coverage axis over the whole segment.
    pub verdict: CoverageVerdict,
    /// The bearing axis over the whole segment.
    pub bearing: BearingIdentity,
    pub bearing_reason: String,
    pub coverage_reason: String,
    pub certified_band_m: f64,
    pub slices: Vec<CoverageSlice>,
    pub event_count: usize,
    pub section_count: usize,
    pub numerical: String,
    /// Every candidate part this segment considered, in stable part-id order.
    pub parts_considered: Vec<String>,
    /// The candidates excluded before event generation, with their reason. Contributing
    /// parts are named on the slices and are never mixed into this list.
    pub parts_filtered: Vec<FilteredPart>,
    /// The event budget this evaluation ran under, so a saved fact can be recomputed.
    pub event_budget: usize,
}

/// One rectangular support part in the (t, s) frame:
/// `alpha[k] + t*beta[k] + s*gamma[k] in [-half[k], half[k]]` for its two own axes.
#[derive(Debug, Clone)]
struct Rect {
    part: String,
    level: f64,
    alpha: [f64; 2],
    beta: [f64; 2],
    gamma: [f64; 2],
    half: [f64; 2],
}

impl Rect {
    /// The interval of `s` this rectangle carries at `t`, if any.
    fn s_range(&self, t: f64) -> Option<(f64, f64)> {
        let mut low = f64::MIN;
        let mut high = f64::MAX;
        for k in 0..2 {
            let centre = self.alpha[k] + t * self.beta[k];
            let lo = -self.half[k] - centre;
            let hi = self.half[k] - centre;
            if self.gamma[k].abs() <= EXACT_EPS {
                if lo > EXACT_EPS || hi < -EXACT_EPS {
                    return None;
                }
            } else {
                let a = lo / self.gamma[k];
                let b = hi / self.gamma[k];
                low = low.max(a.min(b));
                high = high.min(a.max(b));
            }
        }
        if low > high + EXACT_EPS {
            return None;
        }
        Some((low, high))
    }

    /// The four corners of its parallelogram in (t, s), in boundary order.
    fn corners(&self) -> Vec<(f64, f64)> {
        let determinant = self.beta[0] * self.gamma[1] - self.beta[1] * self.gamma[0];
        if determinant.abs() <= EXACT_EPS {
            return vec![];
        }
        let mut out = vec![];
        for sign in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)] {
            let row0 = sign.0 * self.half[0] - self.alpha[0];
            let row1 = sign.1 * self.half[1] - self.alpha[1];
            let t = (row0 * self.gamma[1] - row1 * self.gamma[0]) / determinant;
            let s = (self.beta[0] * row1 - self.beta[1] * row0) / determinant;
            if t.is_finite() && s.is_finite() {
                out.push((t, s));
            }
        }
        out
    }
}

/// Where two boundary segments cross, if they do.
fn segment_crossing(
    p0: (f64, f64),
    p1: (f64, f64),
    q0: (f64, f64),
    q1: (f64, f64),
) -> Option<(f64, f64)> {
    let r = (p1.0 - p0.0, p1.1 - p0.1);
    let s = (q1.0 - q0.0, q1.1 - q0.1);
    let denominator = r.0 * s.1 - r.1 * s.0;
    if denominator.abs() <= EXACT_EPS {
        return None;
    }
    let qp = (q0.0 - p0.0, q0.1 - p0.1);
    let u = (qp.0 * r.1 - qp.1 * r.0) / denominator;
    let v = (qp.0 * s.1 - qp.1 * s.0) / denominator;
    if !(-EXACT_EPS..=1.0 + EXACT_EPS).contains(&u) || !(-EXACT_EPS..=1.0 + EXACT_EPS).contains(&v)
    {
        return None;
    }
    Some((p0.0 + v * r.0, p0.1 + v * r.1))
}

/// The coverage of one section: which real parts cover the required band, or the first
/// uncovered point inside it.
struct Section {
    certified: bool,
    centre_borne: bool,
    parts: Vec<String>,
    witness: Option<(f64, f64)>,
    covered_half_width_m: f64,
}

fn inspect_section(rects: &[Rect], members: &[usize], t: f64, half_width: f64) -> Section {
    let mut intervals: Vec<(f64, f64, usize)> = vec![];
    for index in members {
        if let Some((low, high)) = rects[*index].s_range(t) {
            if high - low > EXACT_EPS {
                intervals.push((low, high, *index));
            }
        }
    }
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    // One connected component at a time: a genuine gap never bridges two components.
    let mut components: Vec<(f64, f64, Vec<usize>)> = vec![];
    for (low, high, index) in intervals {
        match components.last_mut() {
            Some(last) if low <= last.1 + EXACT_EPS => {
                last.1 = last.1.max(high);
                last.2.push(index);
            }
            _ => components.push((low, high, vec![index])),
        }
    }
    for (low, high, owners) in &components {
        if *low <= -half_width + EXACT_EPS && *high >= half_width - EXACT_EPS {
            return Section {
                certified: true,
                centre_borne: true,
                parts: owners
                    .iter()
                    .map(|index| rects[*index].part.clone())
                    .collect(),
                witness: None,
                covered_half_width_m: (-low).min(*high),
            };
        }
    }
    let mut covered = 0.0_f64;
    let mut witness = None;
    for (low, high, _) in &components {
        if *low <= 0.0 && *high >= 0.0 {
            covered = (-low).min(*high);
        }
    }
    let mut cursor = -half_width;
    for (low, high, _) in &components {
        if *high < cursor {
            continue;
        }
        if *low > cursor + EXACT_EPS {
            witness = Some((t, 0.5 * (cursor + *low)));
            break;
        }
        cursor = cursor.max(*high);
        if cursor >= half_width - EXACT_EPS {
            break;
        }
    }
    if witness.is_none() && cursor < half_width - EXACT_EPS {
        witness = Some((t, 0.5 * (cursor + half_width)));
    }
    Section {
        certified: false,
        centre_borne: components
            .iter()
            .any(|(low, high, _)| *low <= EXACT_EPS && *high >= -EXACT_EPS),
        parts: vec![],
        witness,
        covered_half_width_m: covered,
    }
}

/// A decisive or failed evaluation that produced no slices. Both axes and both reasons
/// are carried explicitly: an exhausted budget is `Unknown`, never "no support needed".
#[allow(clippy::too_many_arguments)]
fn empty_certificate(
    segment: usize,
    required_band_m: f64,
    verdict: CoverageVerdict,
    bearing: BearingIdentity,
    bearing_reason: &str,
    coverage_reason: &str,
    note: &str,
    parts_considered: Vec<String>,
    parts_filtered: Vec<FilteredPart>,
    event_budget: usize,
) -> BandCertificate {
    BandCertificate {
        segment,
        required_band_m,
        verdict,
        bearing,
        bearing_reason: bearing_reason.to_string(),
        coverage_reason: coverage_reason.to_string(),
        certified_band_m: 0.0,
        slices: vec![],
        event_count: 0,
        section_count: 0,
        numerical: note.to_string(),
        parts_considered,
        parts_filtered,
        event_budget,
    }
}

/// Certify the closed walking band of one segment against one or more effective layers.
///
/// `layers` is the caller's layer decision: each entry is an effective level and the real
/// parts that belong to it. A sub-interval is certified only when one single layer covers
/// the whole required band there; different layers are never merged into one surface.
pub fn certify_band(
    layers: &[(f64, Vec<(String, MemberFrame)>)],
    segment: usize,
    a: Vec3,
    b: Vec3,
    required_band_m: f64,
    event_budget: usize,
) -> BandCertificate {
    let delta = b.sub(a);
    let length = delta.xz_len();
    let half_width = required_band_m / 2.0;
    // The candidate set the caller handed in, in the caller's stable order. It is
    // reported for every exit, so a failure names what it could not decide about.
    let all_ids: Vec<String> = layers
        .iter()
        .flat_map(|(_, frames)| frames.iter().map(|(part, _)| part.clone()))
        .collect();
    let unknown = |reason: &str, note: &str| {
        empty_certificate(
            segment,
            required_band_m,
            CoverageVerdict::Indeterminate,
            BearingIdentity::Unknown,
            reason,
            reason,
            note,
            all_ids.clone(),
            vec![],
            event_budget,
        )
    };
    if !length.is_finite()
        || length <= EXACT_EPS
        || !required_band_m.is_finite()
        || half_width <= 0.0
    {
        return unknown(REASON_NUMERICAL_UNCERTAIN, "degenerate segment or band");
    }
    let Ok(direction) = delta.normalize_xz() else {
        return unknown(REASON_NUMERICAL_UNCERTAIN, "degenerate segment direction");
    };
    let normal = direction.perp_xz();
    // Entrance contract: non-finite or degenerate geometry never reaches the scan, and a
    // frame whose boundary cannot be trusted is an indeterminate, not a certified cover
    // and not a licence to treat the section as bearing-free ground.
    for (level, frames) in layers {
        if !level.is_finite() {
            return unknown(REASON_INVALID_FRAME, "non-finite layer level");
        }
        for (part, frame) in frames {
            if !frame_is_usable(frame) {
                return empty_certificate(
                    segment,
                    required_band_m,
                    CoverageVerdict::Indeterminate,
                    BearingIdentity::Unknown,
                    REASON_INVALID_FRAME,
                    REASON_INVALID_FRAME,
                    &format!("invalid support frame for part {part}"),
                    all_ids.clone(),
                    vec![],
                    event_budget,
                );
            }
        }
    }
    // Candidate rectangles, with the layer each one belongs to. Layers are never merged.
    let mut rects: Vec<Rect> = vec![];
    let mut levels: Vec<f64> = vec![];
    for (level, frames) in layers {
        if !levels
            .iter()
            .any(|existing| (existing - level).abs() <= 1e-9)
        {
            levels.push(*level);
        }
        for (part, frame) in frames {
            let centre = frame.centre;
            rects.push(Rect {
                part: part.clone(),
                level: *level,
                alpha: [
                    (a.x - centre.x) * frame.along.x + (a.z - centre.z) * frame.along.z,
                    (a.x - centre.x) * frame.across.x + (a.z - centre.z) * frame.across.z,
                ],
                beta: [
                    length * (direction.x * frame.along.x + direction.z * frame.along.z),
                    length * (direction.x * frame.across.x + direction.z * frame.across.z),
                ],
                gamma: [
                    normal.x * frame.along.x + normal.z * frame.along.z,
                    normal.x * frame.across.x + normal.z * frame.across.z,
                ],
                half: [frame.half_length, frame.half_width],
            });
        }
    }
    if rects.is_empty() {
        // Nothing at the walking level at all: a complete evaluation that proves this
        // section owes no structural certificate. The ground consumer still applies the
        // authoritative terrain, grade, water, obstacle and sampling rules.
        return empty_certificate(
            segment,
            required_band_m,
            CoverageVerdict::NotApplicable,
            BearingIdentity::NoStructuralSupport,
            REASON_NO_CANDIDATE_PARTS_PROVEN,
            REASON_NO_CANDIDATE_PARTS_PROVEN,
            "no support part at any adjacent level: proven no structural bearing",
            all_ids,
            vec![],
            event_budget,
        );
    }
    // Safe band filtering, before the expensive event generation: a part is excluded
    // only when its own rectangle is provably clear of the closed walking band
    // (`t` in [0, 1], `|s| <= required/2`). Contact and floating-point ambiguity keep
    // the part, and a frame whose own corners cannot be computed is kept rather than
    // declared out of band. Filtering is an optimisation, never the only defence: the
    // hard cap is never raised, so an over-budget filtered set is still refused. Lateral
    // widening parts that miss the centre line but reach into the band are kept.
    let mut parts_filtered: Vec<FilteredPart> = vec![];
    let mut retained: Vec<Rect> = vec![];
    for rect in rects.into_iter() {
        let corners = rect.corners();
        let touching = if corners.is_empty() {
            true
        } else {
            let mut t_low = f64::MAX;
            let mut t_high = f64::MIN;
            let mut s_low = f64::MAX;
            let mut s_high = f64::MIN;
            for (t, s) in corners {
                t_low = t_low.min(t);
                t_high = t_high.max(t);
                s_low = s_low.min(s);
                s_high = s_high.max(s);
            }
            t_high >= -BAND_FILTER_EPS
                && t_low <= 1.0 + BAND_FILTER_EPS
                && s_high >= -half_width - BAND_FILTER_EPS
                && s_low <= half_width + BAND_FILTER_EPS
        };
        if touching {
            retained.push(rect);
        } else {
            parts_filtered.push(FilteredPart {
                segment,
                part: rect.part,
                level_m: rect.level,
                reason: REASON_OUTSIDE_CLOSED_BAND.to_string(),
            });
        }
    }
    let rects = retained;
    if rects.is_empty() {
        // Every candidate is provably clear of the closed band, so none of them can bear
        // the centre line of this segment either: still a complete proof, not a failure.
        return empty_certificate(
            segment,
            required_band_m,
            CoverageVerdict::NotApplicable,
            BearingIdentity::NoStructuralSupport,
            REASON_NO_CANDIDATE_PARTS_PROVEN,
            REASON_NO_CANDIDATE_PARTS_PROVEN,
            "every support candidate is provably outside the closed walking band",
            all_ids,
            parts_filtered,
            event_budget,
        );
    }
    let mut layer_members: Vec<Vec<usize>> = levels.iter().map(|_| vec![]).collect();
    for (index, rect) in rects.iter().enumerate() {
        let slot = levels
            .iter()
            .position(|existing| (existing - rect.level).abs() <= 1e-9)
            .expect("a layer was registered for every candidate part");
        layer_members[slot].push(index);
    }
    let edge_total = rects.len().saturating_mul(4);
    let estimated_events = edge_total
        .saturating_mul(edge_total.saturating_sub(1))
        .saturating_div(2)
        .saturating_add(edge_total.saturating_mul(4))
        .saturating_add(2);
    if estimated_events > event_budget || estimated_events > MAX_COVERAGE_EVENTS {
        // Over budget on the filtered set is still a refusal, and the two axes stay
        // separate: a station that really is carried proves `structure` and keeps that
        // identity under the failure, while an incomplete evaluation proves nothing and
        // stays `unknown`. Both refuse; neither becomes bearing-free ground.
        let probed = PROBE_STATIONS.iter().any(|t| {
            rects.iter().any(|rect| {
                rect.s_range(*t)
                    .is_some_and(|(low, high)| low <= EXACT_EPS && high >= -EXACT_EPS)
            })
        });
        let bearing = if probed {
            BearingIdentity::Structure
        } else {
            BearingIdentity::Unknown
        };
        return empty_certificate(
            segment,
            required_band_m,
            CoverageVerdict::Indeterminate,
            bearing,
            REASON_COVERAGE_BUDGET_EXHAUSTED,
            REASON_COVERAGE_BUDGET_EXHAUSTED,
            "coverage budget exhausted before event generation",
            all_ids,
            parts_filtered,
            event_budget,
        );
    }
    let corners: Vec<Vec<(f64, f64)>> = rects.iter().map(Rect::corners).collect();
    let mut events: Vec<f64> = vec![0.0, 1.0];
    for polygon in &corners {
        for corner in polygon {
            if corner.0.is_finite() {
                events.push(corner.0);
            }
        }
        for index in 0..polygon.len() {
            let p0 = polygon[index];
            let p1 = polygon[(index + 1) % polygon.len()];
            for line in [-half_width, 0.0, half_width] {
                let span = p1.1 - p0.1;
                if span.abs() <= EXACT_EPS {
                    continue;
                }
                let ratio = (line - p0.1) / span;
                if (-EXACT_EPS..=1.0 + EXACT_EPS).contains(&ratio) {
                    events.push(p0.0 + ratio * (p1.0 - p0.0));
                }
            }
        }
    }
    for i in 0..corners.len() {
        for j in (i + 1)..corners.len() {
            for pi in 0..corners[i].len() {
                let p0 = corners[i][pi];
                let p1 = corners[i][(pi + 1) % corners[i].len()];
                for pj in 0..corners[j].len() {
                    let q0 = corners[j][pj];
                    let q1 = corners[j][(pj + 1) % corners[j].len()];
                    if let Some((t, _)) = segment_crossing(p0, p1, q0, q1) {
                        events.push(t);
                    }
                }
            }
        }
    }
    events.retain(|t| t.is_finite() && *t >= -EXACT_EPS && *t <= 1.0 + EXACT_EPS);
    events.sort_by(f64::total_cmp);
    let event_count = events.len();
    let mut deduped: Vec<f64> = vec![];
    for t in events {
        let clipped = t.clamp(0.0, 1.0);
        if deduped
            .last()
            .is_none_or(|last| (clipped - last).abs() > EXACT_EPS)
        {
            deduped.push(clipped);
        }
    }
    let adjacent = [
        levels[0],
        if levels.len() > 1 {
            levels[1]
        } else {
            levels[0]
        },
    ];
    let mut slices: Vec<CoverageSlice> = vec![];
    let mut section_count = 0_usize;
    let mut all_certified = true;
    let mut any_indeterminate = false;
    let mut any_unknown = false;
    let mut decide = |t: f64, from_t: f64, to_t: f64, slices: &mut Vec<CoverageSlice>| {
        section_count += 1;
        let mut best: Option<(f64, Section)> = None;
        for (slot, level) in levels.iter().enumerate() {
            let section = inspect_section(&rects, &layer_members[slot], t, half_width);
            if section.certified {
                best = Some((*level, section));
                break;
            }
            if best.as_ref().is_none_or(|(_, previous)| {
                section.covered_half_width_m > previous.covered_half_width_m
            }) {
                best = Some((*level, section));
            }
        }
        match best {
            Some((level, section)) if section.certified => slices.push(CoverageSlice {
                from_t,
                to_t,
                effective_level_m: Some(level),
                adjacent_levels_m: adjacent,
                parts: section.parts,
                centre_borne: true,
                bearing: BearingIdentity::Structure,
                bearing_reason: REASON_DECIDED.to_string(),
                coverage_reason: REASON_DECIDED.to_string(),
                verdict: CoverageVerdict::Certified,
                certified_band_m: required_band_m,
                required_band_m,
                witness: None,
            }),
            Some((level, section)) => {
                all_certified = false;
                // The identity is the centre-line fact, decided independently of the
                // band: a real part that carries the centre line keeps `structure` even
                // when the full band is not certified, so a narrow deck is a gap and
                // never a licence to walk on terrain.
                slices.push(CoverageSlice {
                    from_t,
                    to_t,
                    effective_level_m: Some(level),
                    adjacent_levels_m: adjacent,
                    parts: vec![],
                    centre_borne: section.centre_borne,
                    bearing: if section.centre_borne {
                        BearingIdentity::Structure
                    } else {
                        BearingIdentity::NoStructuralSupport
                    },
                    bearing_reason: REASON_DECIDED.to_string(),
                    coverage_reason: REASON_DECIDED.to_string(),
                    verdict: CoverageVerdict::Gap,
                    certified_band_m: 0.0,
                    required_band_m,
                    witness: section.witness.map(|(wt, ws)| CoverageWitness {
                        t: wt,
                        s: ws,
                        covered_half_width_m: section.covered_half_width_m,
                        required_half_width_m: half_width,
                    }),
                });
            }
            None => {
                all_certified = false;
                any_indeterminate = true;
                any_unknown = true;
                slices.push(CoverageSlice {
                    from_t,
                    to_t,
                    effective_level_m: None,
                    adjacent_levels_m: adjacent,
                    parts: vec![],
                    centre_borne: false,
                    bearing: BearingIdentity::Unknown,
                    bearing_reason: REASON_NUMERICAL_UNCERTAIN.to_string(),
                    coverage_reason: REASON_NUMERICAL_UNCERTAIN.to_string(),
                    verdict: CoverageVerdict::Indeterminate,
                    certified_band_m: 0.0,
                    required_band_m,
                    witness: None,
                });
            }
        }
    };
    for event in &deduped {
        decide(*event, *event, *event, &mut slices);
    }
    for pair in deduped.windows(2) {
        if pair[1] - pair[0] <= EXACT_EPS {
            continue;
        }
        decide(0.5 * (pair[0] + pair[1]), pair[0], pair[1], &mut slices);
    }
    let verdict = if all_certified {
        CoverageVerdict::Certified
    } else if any_indeterminate {
        CoverageVerdict::Indeterminate
    } else {
        CoverageVerdict::Gap
    };
    let bearing = if any_unknown {
        BearingIdentity::Unknown
    } else if slices
        .iter()
        .any(|slice| slice.bearing == BearingIdentity::Structure)
    {
        BearingIdentity::Structure
    } else {
        BearingIdentity::NoStructuralSupport
    };
    let bearing_reason = if any_unknown {
        REASON_NUMERICAL_UNCERTAIN.to_string()
    } else {
        REASON_DECIDED.to_string()
    };
    let coverage_reason = if any_indeterminate {
        REASON_NUMERICAL_UNCERTAIN.to_string()
    } else {
        REASON_DECIDED.to_string()
    };
    BandCertificate {
        segment,
        required_band_m,
        verdict,
        bearing,
        bearing_reason,
        coverage_reason,
        certified_band_m: if all_certified { required_band_m } else { 0.0 },
        slices,
        event_count,
        section_count,
        numerical: if any_indeterminate {
            "some sections could not be assessed".to_string()
        } else {
            "floating-point f64 certificate; events merged only within 1e-12".to_string()
        },
        parts_considered: all_ids,
        parts_filtered,
        event_budget,
    }
}
