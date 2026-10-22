// Exact 1m longitudinal design against an immutable, full-width natural raster.
// A design is not a built road: no terrain, entities, ownership or gates change.
//
// Two modes, one solver
// ---------------------
// OFF (the historical, default contract): the design surface may never stand
// below the natural ground, so `L(i) = max over the pavement/shoulder of
// natural` and the profile is the least 8 %-bounded majorant of it. Every road
// on a hillside is therefore an embankment on its downhill side.
//
// ON (`bounded-cut`, opt-in, [`super::bounded_cut::MAX_CUT_MM`]): the design may
// stand BELOW the natural ground by at most the one cap, per cell. The lower
// bound becomes `max(natural_max - allowance, guarded)` where `guarded` is the
// highest natural sample of a cell the road may never lower - protected land,
// the water body and the shoreline public passage band keep their own ground,
// cell by cell - and the profile is still the exact least slope-bounded majorant
// of those bounds. The two junction tie-in platforms, the 8 %/m grade, the fill
// cap (6 m per cell) and the flat transverse surface are unchanged.
//
// WHICH allowance: the profile is a real engineering choice, not a random
// excavation, so the solver searches the bounded interval `0..=cap` for the
// allowance whose profile moves the least earthwork in total
// (`sum |profile - natural|`: cut and fill weighed alike, the cut/fill balance of
// a real vertical alignment), then the least maximum embankment, then the
// smallest allowance. The search is a deterministic ascending sweep of whole
// millimetres with an exact integer objective, so two runs cannot disagree; see
// [`AlignmentContext::profile`]. `allowance = 0` reproduces the fill-only
// envelope bit for bit, which is why a world without the policy is unchanged.
use super::bounded_cut::clamp_cap;
use super::{LandUseZone, SceneSpec, WaterGeometry, zoning::ZoningMask};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
const MAX_FILL_MM: i64 = 6000;
const MAX_RISE_MM: i64 = 80; // 8% per one-metre station; project constraint, not a road code.
/// The published model of the fill-only design. Unchanged, so a world that never
/// opted into the bounded cut publishes the same design bytes it always did.
const FILL_ONLY_MODEL: &str = "full-width-fill-only-minimum-lipschitz-envelope-v1";
/// The published model of the cut-capable design.
const BOUNDED_CUT_MODEL: &str = "full-width-bounded-cut-and-fill-minimum-lipschitz-envelope-v1";
/// "this station has no bound of this kind", far below the raster's own domain
/// (the payload holds -32_000..=256_000 mm). `max` over it is the identity on a
/// real height, and the grade sweep cannot overflow it.
const UNBOUNDED: i64 = i64::MIN / 4;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorridorRequest {
    pub scene_id: String,
    pub start: [i32; 2],
    pub end: [i32; 2],
    pub road_width_m: u32,
    pub shoulder_width_m: u32,
    pub max_fill_mm: u32,
    pub max_rise_mm_per_m: u32,
    /// Optional tie-in platforms; both are authoritative constraints, never relaxed.
    pub start_level_mm: Option<i32>,
    pub end_level_mm: Option<i32>,
    pub junction_length_m: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorridorMesh {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorridorDesign {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub model: String,
    pub request: CorridorRequest,
    pub natural_heightfield_hash: String,
    pub zoning_hash: String,
    pub profile_mm: Vec<i64>,
    /// The cut allowance the profile actually used, in whole millimetres: `0`
    /// for every fill-only design, which is why the key is absent from them.
    #[serde(default, skip_serializing_if = "is_zero_allowance")]
    pub cut_allowance_mm: i32,
    pub road_mesh: CorridorMesh,
    pub support_mesh: CorridorMesh,
    pub proposed_fill_volume_m3: f64,
    pub maximum_fill_mm: i64,
    pub maximum_rise_mm_per_m: i64,
    pub support_area_m2: u64,
}
/// Serialization predicate: a fill-only design publishes no allowance key at all,
/// so its bytes are the historical ones.
fn is_zero_allowance(allowance: &i32) -> bool {
    *allowance == 0
}
struct Samples {
    length: usize,
    half: i32,
    points: Vec<[i32; 2]>,
    natural: Vec<Vec<i64>>,
    /// The fill-only lower bound before the junction pins: the highest natural
    /// sample of the row.
    natural_max: Vec<i64>,
    /// The highest natural sample of a cell this road may NEVER lower - protected
    /// land, the water body and the shoreline public passage band. Such a cell
    /// keeps its own ground, so it bounds the profile from below whatever the
    /// allowance is. [`UNBOUNDED`] where the row holds no such cell.
    guarded_max: Vec<i64>,
    /// The junction tie-in level of a pinned station ([`UNBOUNDED`] otherwise).
    pin: Vec<i64>,
    /// The per-cell fill upper bound, the tightest of the row, pins applied.
    upper: Vec<i64>,
    zoning_hash: String,
}
impl Samples {
    /// The exact per-station lower bound for one cut allowance, in millimetres.
    ///
    /// `allowance == 0` is the historical fill-only bound, bit for bit:
    /// `max(natural_max, pin)`. Any other allowance lowers that by at most the
    /// one cap, never below a cell the road may not cut and never below a
    /// junction tie-in.
    fn lower_at(&self, allowance: i64) -> Vec<i64> {
        (0..=self.length)
            .map(|i| {
                let fill_only = self.natural_max[i];
                let cut = if allowance == 0 {
                    fill_only
                } else {
                    fill_only.saturating_sub(allowance)
                };
                cut.max(self.guarded_max[i]).max(self.pin[i])
            })
            .collect()
    }

    /// The earthwork one candidate profile moves, in whole millimetres of depth
    /// summed over one-square-metre cells: `(sum |profile - natural|, highest
    /// fill above the natural ground)`. Both are exact integers, so the search
    /// below cannot depend on a rounding.
    fn earthwork(&self, profile: &[i64]) -> (i64, i64) {
        let mut moved = 0_i64;
        let mut fill = i64::MIN;
        for (i, row) in self.natural.iter().enumerate() {
            for &h in row {
                let difference = profile[i] - h;
                moved += difference.abs();
                fill = fill.max(difference);
            }
        }
        (moved, fill.max(0))
    }
}
fn error(s: impl Into<String>) -> TerrainError {
    TerrainError(s.into())
}
fn samples(
    r: &CorridorRequest,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    mask: &ZoningMask,
) -> Result<Samples, TerrainError> {
    if r.scene_id != scene.scene_id || !terrain.matches_scene(scene) {
        return Err(error("corridor scene mismatch"));
    }
    terrain.require_complete()?;
    let dx = i64::from(r.end[0]) - i64::from(r.start[0]);
    let dz = i64::from(r.end[1]) - i64::from(r.start[1]);
    if (dx == 0) == (dz == 0)
        || dx.unsigned_abs() + dz.unsigned_abs() > 10000
        || r.road_width_m < 4
        || r.road_width_m > 32
        || r.road_width_m % 2 != 0
        || r.shoulder_width_m > 8
        || r.max_fill_mm > MAX_FILL_MM as u32
        || r.max_rise_mm_per_m == 0
        || r.max_rise_mm_per_m > MAX_RISE_MM as u32
    {
        return Err(error(
            "invalid corridor axis, dimensions or engineering limits",
        ));
    }
    let length = (dx.abs() + dz.abs()) as usize;
    if r.junction_length_m as usize * 2 > length
        || ((r.start_level_mm.is_some() || r.end_level_mm.is_some()) && r.junction_length_m == 0)
    {
        return Err(error("invalid corridor junction interval"));
    }
    let half = (r.road_width_m / 2 + r.shoulder_width_m) as i32;
    // Continuous support rectangle: station-only checks could miss a narrow
    // water crossing between two integer samples. Bridges are out of scope.
    let center_x = (f64::from(r.start[0]) + f64::from(r.end[0])) * 0.5;
    let center_z = (f64::from(r.start[1]) + f64::from(r.end[1])) * 0.5;
    let (half_x, half_z) = if dx == 0 {
        (f64::from(half), length as f64 * 0.5)
    } else {
        (length as f64 * 0.5, f64::from(half))
    };
    if water.intersects_axis_aligned(center_x, center_z, half_x, half_z) {
        return Err(error(
            "corridor continuous support intersects natural water",
        ));
    }
    let mut result = Samples {
        length,
        half,
        points: vec![],
        natural: vec![],
        natural_max: vec![],
        guarded_max: vec![],
        pin: vec![],
        upper: vec![],
        zoning_hash: mask.hash.clone(),
    };
    for i in 0..=length {
        let x = i64::from(r.start[0]) + dx.signum() * i as i64;
        let z = i64::from(r.start[1]) + dz.signum() * i as i64;
        let point = [
            i32::try_from(x).map_err(|_| error("corridor coordinate overflow"))?,
            i32::try_from(z).map_err(|_| error("corridor coordinate overflow"))?,
        ];
        // Full lateral support, not centerline-only or endpoint terrain probes.
        let (hw, hd) = if dx == 0 { (half, 0) } else { (0, half) };
        if mask
            .check("corridor".into(), "road", point[0], point[1], hw, hd)
            .is_some()
        {
            return Err(error(format!(
                "corridor protected/outside zoning at station {i}"
            )));
        }
        if water.intersects_axis_aligned(x as f64, z as f64, hw as f64, hd as f64) {
            return Err(error(format!(
                "corridor crosses natural water at station {i}"
            )));
        }
        let mut row = vec![];
        let mut lo = i64::MIN;
        let mut guarded = UNBOUNDED;
        let mut hi = i64::MAX;
        for transverse in -half..=half {
            let xx = x + if dx == 0 { i64::from(transverse) } else { 0 };
            let zz = z + if dz == 0 { i64::from(transverse) } else { 0 };
            let xx = i32::try_from(xx).map_err(|_| error("corridor support overflow"))?;
            let zz = i32::try_from(zz).map_err(|_| error("corridor support overflow"))?;
            let h = terrain
                .height_millimetres(xx, zz)
                .ok_or_else(|| error(format!("corridor missing support sample {xx},{zz}")))?;
            let cap = mask
                .fill_capacity_at(xx, zz)
                .min(f64::from(r.max_fill_mm) / 1000.);
            let h = i64::from(h);
            lo = lo.max(h);
            hi = hi.min(h + (cap * 1000.).floor() as i64);
            // The three lands an excavation may never enter, taken cell by cell
            // from the natural surface they keep. `bounded_cut` and the
            // composition state the same prohibition; asking here is what makes
            // the profile itself respect it instead of leaving it to a later
            // clamp.
            if mask.protected_at(xx, zz)
                || water.contains_point(f64::from(xx), f64::from(zz))
                || water.shore_band_cell(xx, zz)
            {
                guarded = guarded.max(h);
            }
            row.push(h);
        }
        let mut pin = UNBOUNDED;
        for (active, level) in [
            (i <= r.junction_length_m as usize, r.start_level_mm),
            (length - i <= r.junction_length_m as usize, r.end_level_mm),
        ] {
            if active {
                if let Some(h) = level {
                    pin = pin.max(i64::from(h));
                    hi = hi.min(i64::from(h));
                }
            }
        }
        result.points.push(point);
        result.natural.push(row);
        result.natural_max.push(lo);
        result.guarded_max.push(guarded);
        result.pin.push(pin);
        result.upper.push(hi);
    }
    Ok(result)
}
/// The pointwise least slope-bounded majorant of lower bounds on a path.
/// Two exact integer sweeps: y[i] = max_j(lower[j] - rise * abs(i-j)).
/// If it exceeds an upper bound, no profile satisfying these fixed bounds exists.
fn solve(lower: &[i64], upper: &[i64], rise: i64) -> Result<Vec<i64>, TerrainError> {
    if lower.is_empty() || lower.len() != upper.len() || rise <= 0 {
        return Err(error("invalid alignment bounds"));
    }
    let mut y = lower.to_vec();
    for i in 1..y.len() {
        y[i] = y[i].max(y[i - 1] - rise);
    }
    for i in (0..y.len() - 1).rev() {
        y[i] = y[i].max(y[i + 1] - rise);
    }
    for (i, (&height, &cap)) in y.iter().zip(upper).enumerate() {
        if height > cap {
            return Err(error(format!(
                "corridor infeasible at station {i}: required {height}mm exceeds {cap}mm cap"
            )));
        }
    }
    Ok(y)
}
fn mesh(points: &[[i32; 2]], profile: &[i64], half: i32) -> CorridorMesh {
    let along_x = points[0][1] == points[1][1];
    let mut result = CorridorMesh {
        vertices: vec![],
        triangles: vec![],
    };
    for (p, h) in points.iter().zip(profile) {
        for d in [-half, half] {
            result.vertices.push([
                f64::from(p[0]) + if along_x { 0. } else { f64::from(d) },
                *h as f64 / 1000.,
                f64::from(p[1]) + if along_x { f64::from(d) } else { 0. },
            ]);
        }
    }
    for i in 0..points.len() - 1 {
        let k = i as u32 * 2;
        let mut a = [k, k + 1, k + 2];
        let mut b = [k + 1, k + 3, k + 2];
        // Ensure upward triangle winding for both axes and both directions.
        let p = a.map(|j| result.vertices[j as usize]);
        if (p[1][2] - p[0][2]) * (p[2][0] - p[0][0]) - (p[1][0] - p[0][0]) * (p[2][2] - p[0][2])
            < 0.
        {
            a.swap(1, 2);
            b.swap(1, 2);
        }
        result.triangles.extend([a, b]);
    }
    result
}
fn quantities(s: &Samples, profile: &[i64]) -> (f64, i64, i64) {
    let mut volume = 0.;
    let mut max_fill = 0;
    for (i, row) in s.natural.iter().enumerate() {
        for &h in row {
            max_fill = max_fill.max(profile[i] - h);
        }
    }
    // Integral over 1x1m cells with bilinear source and linear design heights.
    for i in 0..s.length {
        for j in 0..s.natural[i].len() - 1 {
            let sum = (profile[i] - s.natural[i][j])
                + (profile[i] - s.natural[i][j + 1])
                + (profile[i + 1] - s.natural[i + 1][j])
                + (profile[i + 1] - s.natural[i + 1][j + 1]);
            volume += sum as f64 / 4000.;
        }
    }
    (
        volume,
        max_fill,
        profile
            .windows(2)
            .map(|p| (p[1] - p[0]).abs())
            .max()
            .unwrap_or(0),
    )
}
/// Prepared per-scene source. Only compact candidate samples are allocated in
/// the search loop; the raster hash and zoning index are built once.
pub(super) struct AlignmentContext<'a> {
    pub scene: &'a SceneSpec,
    pub terrain: &'a BakedTerrain,
    pub water: WaterGeometry,
    pub mask: ZoningMask,
    natural_hash: String,
    /// The bounded-cut allowance in whole millimetres; `0` keeps the historical
    /// fill-only profile and is the default of every context.
    cut_mm: i32,
}
impl<'a> AlignmentContext<'a> {
    pub fn new(
        scene: &'a SceneSpec,
        terrain: &'a BakedTerrain,
        water: WaterGeometry,
        zones: &[LandUseZone],
    ) -> Result<Self, TerrainError> {
        terrain.require_complete()?;
        if !terrain.matches_scene(scene) {
            return Err(error("alignment context scene mismatch"));
        }
        Ok(Self {
            scene,
            terrain,
            water,
            mask: ZoningMask::new(scene, zones)?,
            natural_hash: terrain.content_hash()?,
            cut_mm: 0,
        })
    }

    /// OPT-IN `bounded-cut`: allow the longitudinal design to sit up to `cut_mm`
    /// millimetres BELOW the natural ground, cell by cell.
    ///
    /// The allowance is the policy's own cap, clamped by
    /// [`super::bounded_cut::clamp_cap`], so this file states no second number.
    /// Passing `0` - every context that does not opt in - leaves the solver
    /// exactly as it was.
    pub fn with_cut(mut self, cut_mm: i32) -> Self {
        self.cut_mm = clamp_cap(cut_mm);
        self
    }

    /// The bounded-cut allowance this context designs under, in whole
    /// millimetres; `0` for a fill-only context.
    ///
    /// Exported because a corridor's own PRE-FLIGHT probes ask the same question
    /// the design does - "how far below the natural ground may this road go" - and
    /// they must answer it with the design's own allowance, never with a second
    /// number (`road_extensions::append_one`'s padded-footprint envelope).
    pub fn cut_mm(&self) -> i32 {
        self.cut_mm
    }

    /// THE longitudinal solver.
    ///
    /// A bounded one-dimensional search over the cut allowance
    /// `a in 0..=cap`, and nothing else is invented: for every `a` the profile is
    /// the SAME exact least slope-bounded majorant [`solve`] computes, only with
    /// its lower bound [`Samples::lower_at`] lowered by `a` where no guarded cell
    /// forbids it. Larger `a` only ever lowers the profile (each lower bound is
    /// monotone in `a`, so is their majorant), so the fill can only fall and the
    /// excavation can only grow.
    ///
    /// WHAT IS CHOSEN, in this order:
    ///
    /// 1. the least TOTAL EARTHWORK `sum |profile - natural|` over the whole
    ///    corridor - cut and fill weighed alike. That is the cut/fill balance of
    ///    a real vertical alignment: on a cross-slope it settles the surface near
    ///    the middle of each transverse section (a bench cut into the uphill side
    ///    and an embankment on the downhill side) instead of the fill-only
    ///    envelope's `max`, and on ground that needs neither it takes nothing;
    /// 2. the least MAXIMUM EMBANKMENT, which is the number the 6 m roadbed
    ///    contract is about;
    /// 3. the smallest allowance, so a tie publishes the shallower excavation.
    ///
    /// Determinism: the sweep is `0..=cap` in whole millimetres over exact integer
    /// objectives, and the comparison is strict `<` on `(earthwork, fill)` - so
    /// the winner is unique, no floating point participates, and the same raster
    /// always yields the same profile and the same published allowance.
    fn profile(&self, s: &Samples, rise: i64) -> Result<(Vec<i64>, i32), TerrainError> {
        let cap = i64::from(self.cut_mm);
        let mut best: Option<(i64, i64, i32, Vec<i64>)> = None;
        let mut infeasible: Option<TerrainError> = None;
        for allowance in 0..=cap {
            let lower = s.lower_at(allowance);
            match solve(&lower, &s.upper, rise) {
                Ok(profile) => {
                    let (moved, fill) = s.earthwork(&profile);
                    let better = best.as_ref().is_none_or(|(known_moved, known_fill, _, _)| {
                        (moved, fill) < (*known_moved, *known_fill)
                    });
                    if better {
                        best = Some((moved, fill, allowance as i32, profile));
                    }
                }
                // The upper bounds are the fill contract and the junction pins;
                // a larger allowance can only help them, so the tightest failure
                // is the last one seen. With the policy off this is the only
                // candidate and the message is the historical one.
                Err(failure) => infeasible = Some(failure),
            }
        }
        match best {
            Some((_, _, allowance, profile)) => Ok((profile, allowance)),
            None => Err(infeasible.unwrap_or_else(|| error("invalid alignment bounds"))),
        }
    }

    pub fn design(&self, r: &CorridorRequest) -> Result<CorridorDesign, TerrainError> {
        let s = samples(r, self.scene, self.terrain, self.water, &self.mask)?;
        let (profile, allowance) = self.profile(&s, i64::from(r.max_rise_mm_per_m))?;
        let (volume, fill, rise) = quantities(&s, &profile);
        Ok(CorridorDesign {
            format: "glyphweave.road-corridor-design".into(),
            version: 1,
            status: "design_only_not_applied".into(),
            model: self.model().into(),
            request: r.clone(),
            natural_heightfield_hash: self.natural_hash.clone(),
            zoning_hash: s.zoning_hash.clone(),
            cut_allowance_mm: allowance,
            road_mesh: mesh(&s.points, &profile, (r.road_width_m / 2) as i32),
            support_mesh: mesh(&s.points, &profile, s.half),
            profile_mm: profile,
            proposed_fill_volume_m3: volume,
            maximum_fill_mm: fill,
            maximum_rise_mm_per_m: rise,
            support_area_m2: s.length as u64 * s.half as u64 * 2,
        })
    }

    /// The design model this context publishes. A world that never opted into the
    /// bounded cut keeps the historical string, byte for byte.
    fn model(&self) -> &'static str {
        if self.cut_mm == 0 {
            FILL_ONLY_MODEL
        } else {
            BOUNDED_CUT_MODEL
        }
    }

    /// Infer a constant endpoint platform without enumerating or rounding to
    /// whole metres. Any higher endpoint only raises the least profile, so it
    /// cannot rescue an upper-bound violation of this minimum feasible choice.
    ///
    /// The platform search stays FILL-ONLY even under the bounded cut: the
    /// inferred plateau is a junction others tie into, and it is the minimum
    /// feasible one already, so lowering it would trade a legal junction for a
    /// deeper excavation nobody asked for. A`design` may still cut the corridor
    /// between the two platforms.
    pub fn infer_end(&self, r: &CorridorRequest) -> Result<CorridorRequest, TerrainError> {
        if r.end_level_mm.is_some() {
            return Ok(r.clone());
        }
        let source = samples(r, self.scene, self.terrain, self.water, &self.mask)?;
        let [x, z] = r.end;
        let radius = (r.road_width_m / 2 + r.shoulder_width_m).max(r.junction_length_m) as i32;
        if radius > 32
            || self
                .mask
                .check("auto-end".into(), "road", x, z, radius, radius)
                .is_some()
        {
            return Err(error("automatic endpoint crosses protected/outside zoning"));
        }
        let mut low = i64::MIN;
        let mut high = i64::MAX;
        for zz in z - radius..=z + radius {
            for xx in x - radius..=x + radius {
                let h = i64::from(
                    self.terrain
                        .height_millimetres(xx, zz)
                        .ok_or_else(|| error("missing automatic endpoint sample"))?,
                );
                low = low.max(h);
                high = high.min(
                    h + (self
                        .mask
                        .fill_capacity_at(xx, zz)
                        .min(f64::from(r.max_fill_mm) / 1000.)
                        * 1000.)
                        .floor() as i64,
                );
            }
        }
        let begin = source.length - r.junction_length_m as usize;
        let mut lower = source.lower_at(0);
        let mut upper = source.upper.clone();
        for i in begin..=source.length {
            lower[i] = lower[i].max(low);
            upper[i] = upper[i].min(high);
        }
        let y = solve(&lower, &upper, i64::from(r.max_rise_mm_per_m))?;
        let level = *y[begin..]
            .iter()
            .max()
            .ok_or_else(|| error("missing automatic endpoint"))?;
        if level > high {
            return Err(error(
                "automatic endpoint has no feasible constant platform",
            ));
        }
        let mut pinned = r.clone();
        pinned.end_level_mm =
            Some(i32::try_from(level).map_err(|_| error("endpoint outside millimetre range"))?);
        // The returned request is checked by design/append, including the
        // effect of pinning the whole plateau on all upstream upper bounds.
        Ok(pinned)
    }
    /// Independent source/constraint verification, not replay of the solver.
    pub fn verify(&self, d: &CorridorDesign, r: &CorridorRequest) -> Result<(), TerrainError> {
        let s = samples(r, self.scene, self.terrain, self.water, &self.mask)?;
        let allowance = i64::from(d.cut_allowance_mm);
        if d.format != "glyphweave.road-corridor-design"
            || d.version != 1
            || d.status != "design_only_not_applied"
            || d.model != self.model()
            // The published allowance is a claim: it may not exceed the policy's
            // own cap, and a fill-only design may not claim any (its context's
            // cap is 0, so this rejects a forged non-zero value by name).
            || allowance < 0
            || allowance > i64::from(self.cut_mm)
            || d.request != *r
            || d.natural_heightfield_hash != self.natural_hash
            || d.zoning_hash != s.zoning_hash
            || d.profile_mm.len() != s.length + 1
        {
            return Err(error("corridor design provenance mismatch"));
        }
        let y = &d.profile_mm;
        let rise = i64::from(r.max_rise_mm_per_m);
        let lower = s.lower_at(allowance);
        for i in 0..y.len() {
            let mut expected = lower[i];
            if i > 0 {
                expected = expected.max(
                    y[i - 1]
                        .checked_sub(rise)
                        .ok_or_else(|| error("profile overflow"))?,
                );
            }
            if i + 1 < y.len() {
                expected = expected.max(
                    y[i + 1]
                        .checked_sub(rise)
                        .ok_or_else(|| error("profile overflow"))?,
                );
            }
            if y[i] < lower[i] || y[i] > s.upper[i] || y[i] != expected {
                return Err(error(format!(
                    "corridor profile violates source/grade/minimum at station {i}"
                )));
            }
        }
        // The per-cell cut this profile asks for: never deeper than the one cap,
        // and never on a cell the road may not lower.
        for (i, row) in s.natural.iter().enumerate() {
            for &h in row {
                let depth = h - y[i];
                if depth > allowance {
                    return Err(error(format!(
                        "corridor profile cuts station {i} by {depth}mm, above the {allowance}mm allowance"
                    )));
                }
            }
        }
        for (actual, half) in [
            (&d.road_mesh, (r.road_width_m / 2) as i32),
            (&d.support_mesh, s.half),
        ] {
            // Geometry must exactly realize all stations; no hidden clipping/scale.
            let expected = mesh(&s.points, y, half);
            if *actual != expected {
                return Err(error("corridor mesh differs from station geometry"));
            }
        }
        let (volume, fill, max_rise) = quantities(&s, y);
        if d.proposed_fill_volume_m3 != volume
            || d.maximum_fill_mm != fill
            || d.maximum_rise_mm_per_m != max_rise
            || d.support_area_m2 != s.length as u64 * s.half as u64 * 2
        {
            return Err(error("corridor earthwork quantities mismatch"));
        }
        Ok(())
    }
}
pub fn design(
    r: &CorridorRequest,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
) -> Result<CorridorDesign, TerrainError> {
    AlignmentContext::new(scene, terrain, water, zones)?.design(r)
}
pub fn verify(
    d: &CorridorDesign,
    r: &CorridorRequest,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
) -> Result<(), TerrainError> {
    // The design publishes the allowance it used, and THIS entry point has no
    // policy to compare it against (it is the source rebuild a reader of the
    // published bytes can run). `with_cut` clamps the claim into the one legal
    // range, so the check below still refuses any design that asks for more than
    // [`super::bounded_cut::MAX_CUT_MM`]; the policy's own gate is
    // `AlignmentContext::verify`, which the producer calls right after designing.
    AlignmentContext::new(scene, terrain, water, zones)?
        .with_cut(d.cut_allowance_mm)
        .verify(d, r)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        height_payload::encode_millimetres,
        worldgen::{WaterKind, water_geometry},
    };
    fn fixture(
        f: impl Fn(i32, i32) -> i32,
    ) -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        CorridorRequest,
    ) {
        let scene = SceneSpec {
            scene_id: "alignment".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: -1000,
            origin_z: 50,
            seed_offset: 0,
        };
        let values: Vec<i32> = (0..512)
            .flat_map(|z| (0..512).map(move |x| (x, z)))
            .map(|(x, z)| f(x, z))
            .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                -1000,
                50,
                512,
                512,
                &encode_millimetres(512, 512, values.iter().map(|h| f64::from(*h) / 1000.))
                    .unwrap(),
            )
            .unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: -1000,
            world_z: 50,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let r = CorridorRequest {
            scene_id: scene.scene_id.clone(),
            start: [-968, 178],
            end: [-712, 178],
            road_width_m: 8,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: None,
            end_level_mm: None,
            junction_length_m: 8,
        };
        (scene, terrain, water, zones, r)
    }
    #[test]
    fn exact_envelope_matches_quadratic_oracle_and_all_upper_bounds() {
        for seed in 0..50 {
            let lower: Vec<_> = (0..60)
                .map(|i| ((i * 997 + seed * 137) % 1733) - 700)
                .collect();
            let upper: Vec<_> = lower.iter().map(|h| h + 6000).collect();
            let actual = solve(&lower, &upper, 80).unwrap();
            let expected: Vec<_> = (0..lower.len())
                .map(|i| {
                    (0..lower.len())
                        .map(|j| lower[j] - 80 * i.abs_diff(j) as i64)
                        .max()
                        .unwrap()
                })
                .collect();
            assert_eq!(actual, expected);
            let mut cap = upper;
            cap[17] = expected[17] - 1;
            assert!(solve(&lower, &cap, 80).is_err());
        }
    }
    #[test]
    fn natural_rising_corridor_succeeds_where_one_flat_platform_cannot() {
        let (s, t, w, z, r) = fixture(|x, _| x * 40);
        let before = t.content_hash().unwrap();
        let d = design(&r, &s, &t, w, &z).unwrap();
        assert_eq!(d.profile_mm.first(), Some(&1280));
        assert_eq!(d.profile_mm.last(), Some(&11520));
        assert!(d.profile_mm.last().unwrap() - d.profile_mm.first().unwrap() > 6000);
        assert_eq!(d.maximum_rise_mm_per_m, 40);
        assert_eq!(d.maximum_fill_mm, 0);
        assert_eq!(d.proposed_fill_volume_m3, 0.);
        assert_eq!(d.support_area_m2, 256 * 14);
        verify(&d, &r, &s, &t, w, &z).unwrap();
        assert_eq!(before, t.content_hash().unwrap());
        assert_eq!(d.road_mesh.vertices.len(), 514);
        assert_eq!(d.road_mesh.triangles.len(), 512);
    }
    #[test]
    fn shoulder_ridge_is_not_missed_by_centerline_probes() {
        let (s, t, w, z, r) = fixture(|x, z| if x == 128 && z == 134 { 12000 } else { x * 40 });
        assert!(
            design(&r, &s, &t, w, &z)
                .unwrap_err()
                .0
                .contains("infeasible")
        );
    }
    #[test]
    fn exact_tie_in_platforms_and_grade_constraints_are_not_relaxed() {
        let (s, t, w, z, mut r) = fixture(|x, _| x * 20);
        r.start_level_mm = Some(2000);
        r.end_level_mm = Some(6500);
        let d = design(&r, &s, &t, w, &z).unwrap();
        assert!(d.profile_mm[..=8].iter().all(|h| *h == 2000));
        assert!(d.profile_mm[248..].iter().all(|h| *h == 6500));
        verify(&d, &r, &s, &t, w, &z).unwrap();
        r.start_level_mm = Some(0);
        assert!(design(&r, &s, &t, w, &z).is_err());
        r.start_level_mm = Some(2000);
        r.end_level_mm = Some(20000);
        assert!(design(&r, &s, &t, w, &z).is_err());
    }
    #[test]
    fn axes_directions_volume_and_upward_winding_are_explicit() {
        let (s, t, w, z, mut r) = fixture(|_, _| 3861);
        assert_eq!(t.height_millimetres(-900, 180), Some(3861));
        assert_eq!(t.height_millimetres(-1001, 180), None);
        r.start_level_mm = Some(4861);
        r.end_level_mm = Some(4861);
        r.junction_length_m = 128;
        for (a, b) in [
            ([-968, 178], [-712, 178]),
            ([-712, 178], [-968, 178]),
            ([-872, 82], [-872, 338]),
            ([-872, 338], [-872, 82]),
        ] {
            r.start = a;
            r.end = b;
            let d = design(&r, &s, &t, w, &z).unwrap();
            verify(&d, &r, &s, &t, w, &z).unwrap();
            assert_eq!(d.proposed_fill_volume_m3, 3584.);
            for tri in &d.support_mesh.triangles {
                let p = tri.map(|i| d.support_mesh.vertices[i as usize]);
                assert!(
                    (p[1][2] - p[0][2]) * (p[2][0] - p[0][0])
                        - (p[1][0] - p[0][0]) * (p[2][2] - p[0][2])
                        > 0.
                );
            }
        }
    }
    #[test]
    fn protected_land_invalid_requests_and_forged_designs_fail_closed() {
        let (s, t, w, z, r) = fixture(|_, _| 0);
        let d = design(&r, &s, &t, w, &z).unwrap();
        let mut protected = z.clone();
        protected[0].kind = "nature".into();
        assert!(design(&r, &s, &t, w, &protected).is_err());
        for mode in 0..5 {
            let mut bad = r.clone();
            match mode {
                0 => bad.road_width_m = 7,
                1 => bad.max_fill_mm = 6001,
                2 => bad.max_rise_mm_per_m = 81,
                3 => bad.end[1] += 1,
                _ => bad.start = [-1001, 178],
            };
            assert!(design(&bad, &s, &t, w, &z).is_err());
        }
        for mode in 0..8 {
            let mut bad = d.clone();
            match mode {
                0 => bad.profile_mm[20] = 1,
                1 => bad.road_mesh.vertices[20][1] += 1.,
                2 => bad.support_mesh.triangles[0] = [0, 0, 0],
                3 => bad.proposed_fill_volume_m3 = 123.,
                4 => bad.maximum_fill_mm = 7,
                5 => bad.status = "built".into(),
                6 => bad.natural_heightfield_hash = "forged".into(),
                _ => bad.request.max_fill_mm = 5000,
            };
            assert!(verify(&bad, &r, &s, &t, w, &z).is_err(), "mode {mode}");
        }
    }
    #[test]
    fn water_crossings_and_missing_raster_are_not_silently_bridged() {
        let (s, t, _, z, r) = fixture(|_, _| 0);
        let river = water_geometry(WaterKind::River, &[], &s, &serde_json::Value::Null);
        assert!(design(&r, &s, &t, river, &z).is_err());
        let empty = BakedTerrain::new(&s).unwrap();
        let none = water_geometry(WaterKind::None, &[], &s, &serde_json::Value::Null);
        assert!(design(&r, &s, &empty, none, &z).is_err());
    }
    #[test]
    fn prepared_context_and_inferred_endpoint_preserve_exact_millimetres() {
        let (s, t, w, z, mut r) = fixture(|_, _| 3861);
        r.start_level_mm = Some(4861);
        let ctx = AlignmentContext::new(&s, &t, w, &z).unwrap();
        let inferred = ctx.infer_end(&r).unwrap();
        assert_eq!(inferred.end_level_mm, Some(3861));
        let d = ctx.design(&inferred).unwrap();
        ctx.verify(&d, &inferred).unwrap();
        assert_eq!(d, design(&inferred, &s, &t, w, &z).unwrap());
        let mut too_low = inferred;
        too_low.end_level_mm = Some(3860);
        assert!(ctx.design(&too_low).is_err());
    }

    // ============ Round 42: the bounded-cut longitudinal profile =============

    /// One east-west corridor across a 40 % cross-slope that ALSO climbs 4 %
    /// along its axis:
    ///
    /// ```text
    /// natural(z) = 2000 - 400 * (z - 178) + 40 * (x - 968)   [mm]
    /// ```
    ///
    /// so at every station the 15 m transverse band spans 5600 mm from its
    /// uphill edge (`+2800`) to its downhill edge (`-2800`). The historical
    /// fill-only envelope is therefore flat at the uphillside maximum, i.e. the
    /// whole road stands on a 5600 mm embankment - the shape this round removes.
    fn cross_slope_fixture() -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        CorridorRequest,
    ) {
        // `fixture` hands the closure LOCAL indices: world x = x + 1000,
        // world z = z + 50, and the corridor runs at world z = 178 (local 128)
        // from world x = -968 (local 32).
        fixture(|x, z| 2000 - 400 * (z - 128) + 40 * (x - 32))
    }

    /// Requirement 1 + 2: under `bounded-cut` the design surface sits BELOW the
    /// natural ground, by at most the one cap per cell, and the choice is the
    /// cut/fill balance of the corridor - not an arbitrary excavation.
    #[test]
    fn a_bounded_cut_profile_sits_below_the_natural_ground_inside_the_one_cap() {
        let (s, t, w, z, r) = cross_slope_fixture();
        let fill_only = AlignmentContext::new(&s, &t, w, &z).unwrap();
        let off = fill_only.design(&r).unwrap();
        // OFF: the surface is the uphill edge of every row, raised 40 mm/m.
        assert_eq!(off.cut_allowance_mm, 0);
        assert_eq!(off.model, FILL_ONLY_MODEL);
        for (i, h) in off.profile_mm.iter().enumerate() {
            assert_eq!(*h, 4_800 + 40 * i as i64, "station {i}");
        }
        assert_eq!(off.maximum_fill_mm, 5_600);

        // ON: the same corridor, the same constraints, one 2000 mm allowance.
        let context = AlignmentContext::new(&s, &t, w, &z)
            .unwrap()
            .with_cut(super::super::bounded_cut::MAX_CUT_MM);
        let on = context.design(&r).unwrap();
        context.verify(&on, &r).unwrap();
        assert_eq!(on.cut_allowance_mm, super::super::bounded_cut::MAX_CUT_MM);
        assert_eq!(on.model, BOUNDED_CUT_MODEL);
        // The balanced surface: 2000 mm below the fill-only envelope, and the
        // longitudinal grade is untouched (40 mm/m on both arms).
        for (i, h) in on.profile_mm.iter().enumerate() {
            assert_eq!(*h, 2_800 + 40 * i as i64, "station {i}");
        }
        assert_eq!(on.maximum_rise_mm_per_m, 40);
        assert_eq!(
            on.maximum_fill_mm, 3_600,
            "5600 mm of embankment less the cap"
        );

        // Cell by cell, from the natural raster: every station is BOTH cut (its
        // uphill half) and filled (its downhill half), and no cell loses more
        // than the one cap.
        let samples = samples(&r, &s, &t, w, &context.mask).unwrap();
        let (mut cut_cells, mut half_cut_stations, mut deepest) = (0_u32, 0_u32, 0_i64);
        for (i, row) in samples.natural.iter().enumerate() {
            let cut = row.iter().filter(|h| **h > on.profile_mm[i]).count();
            let fill = row.iter().filter(|h| **h < on.profile_mm[i]).count();
            if cut > 0 && fill > 0 {
                half_cut_stations += 1;
            }
            assert_eq!(cut, 5, "station {i}: five uphill cells are excavated");
            assert_eq!(fill, 9, "station {i}: nine downhill cells are filled");
            cut_cells += cut as u32;
            for h in row {
                deepest = deepest.max(*h - on.profile_mm[i]);
            }
        }
        assert_eq!(half_cut_stations, 257, "the whole 256 m corridor");
        assert_eq!(cut_cells, 1_285);
        assert_eq!(deepest, super::super::bounded_cut::MAX_CUT_MM as i64);

        // Two runs of the same raster produce the same design, byte for byte.
        let again = AlignmentContext::new(&s, &t, w, &z)
            .unwrap()
            .with_cut(super::super::bounded_cut::MAX_CUT_MM)
            .design(&r)
            .unwrap();
        assert_eq!(on, again);
        assert_eq!(
            serde_json::to_vec(&on).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
    }

    /// Requirement 3: the same fixture WITHOUT the policy is the historical
    /// fill-only envelope - the row's maximum at every station, no cell cut.
    #[test]
    fn the_same_corridor_without_the_policy_is_still_the_fill_only_envelope() {
        let (s, t, w, z, r) = cross_slope_fixture();
        let context = AlignmentContext::new(&s, &t, w, &z).unwrap();
        let samples = samples(&r, &s, &t, w, &context.mask).unwrap();
        let off = context.design(&r).unwrap();
        for (i, h) in off.profile_mm.iter().enumerate() {
            assert_eq!(*h, samples.natural_max[i], "station {i}");
            assert!(
                samples.natural[i].iter().all(|natural| *natural <= *h),
                "station {i} is never excavated"
            );
        }
        assert_eq!(off.cut_allowance_mm, 0);
        assert!(
            off.proposed_fill_volume_m3 > 0.,
            "the fill-only arm is a fill, never a cut"
        );
        // The published bytes of the off arm carry no allowance key at all.
        let published = serde_json::to_value(&off).unwrap();
        assert!(
            published.get("cutAllowanceMm").is_none(),
            "a fill-only design publishes no cut record: {published}"
        );

        // Flat ground takes no excavation even with the policy ON: the objective
        // is the earthwork, so a corridor that needs no fill is not cut.
        let (s, t, w, z, r) = fixture(|_, _| 3_000);
        let flat = AlignmentContext::new(&s, &t, w, &z)
            .unwrap()
            .with_cut(super::super::bounded_cut::MAX_CUT_MM)
            .design(&r)
            .unwrap();
        assert_eq!(flat.cut_allowance_mm, 0);
        assert_eq!(flat.maximum_fill_mm, 0);
        assert_eq!(flat.proposed_fill_volume_m3, 0.);
    }

    /// Requirement 3, cell by cell: a cell the road may NEVER lower keeps its own
    /// natural height as a lower bound, whatever the allowance is. Protected
    /// land, the water body and the shoreline band are the three such cells;
    /// this asserts the bound itself, so no later clamp can hide a regression.
    #[test]
    fn no_guarded_cell_ever_has_its_lower_bound_lowered() {
        let row = vec![1_000, 4_000, 9_000, 2_000];
        let samples = Samples {
            length: 1,
            half: 1,
            points: vec![[0, 0], [1, 0]],
            natural: vec![row.clone(), row.clone()],
            natural_max: vec![9_000, 9_000],
            // The uphill cell of the row (9000 mm) is protected ground.
            guarded_max: vec![9_000, 9_000],
            pin: vec![UNBOUNDED, UNBOUNDED],
            upper: vec![i64::MAX, i64::MAX],
            zoning_hash: "guard".into(),
        };
        for allowance in [0, 1, 500, 1_999, 2_000] {
            let lower = samples.lower_at(allowance);
            for i in 0..lower.len() {
                assert!(
                    lower[i] >= samples.guarded_max[i],
                    "allowance {allowance} station {i} lowered a guarded bound"
                );
                assert!(lower[i] >= 9_000, "allowance {allowance} station {i}");
            }
        }
        // An unguarded row of the very same heights IS lowered by the allowance,
        // so the assertion above is about the guard, not about the fixture.
        let open = Samples {
            guarded_max: vec![UNBOUNDED, UNBOUNDED],
            ..samples
        };
        assert_eq!(open.lower_at(2_000), vec![7_000, 7_000]);
        assert_eq!(open.lower_at(0), vec![9_000, 9_000]);
        // A junction tie-in pins the station whatever the allowance offers.
        let pinned = Samples {
            pin: vec![8_500, UNBOUNDED],
            ..open
        };
        assert_eq!(pinned.lower_at(2_000), vec![8_500, 7_000]);
    }

    /// Requirement 4: the two numeric contracts survive the cut exactly - 8 %/m
    /// between adjacent stations, 6 m of fill per cell - and a tie-in elevation
    /// is never relaxed. A corridor that cannot hold them is still refused by
    /// name, not excavated past its allowance.
    #[test]
    fn the_grade_fill_and_tie_in_contracts_are_not_relaxed_by_the_cut() {
        let (s, t, w, z, mut r) = cross_slope_fixture();
        let context = AlignmentContext::new(&s, &t, w, &z)
            .unwrap()
            .with_cut(super::super::bounded_cut::MAX_CUT_MM);
        // Tie-ins: pin both platforms 1000 mm above the balanced surface.
        r.start_level_mm = Some(3_800);
        r.end_level_mm = Some(3_800 + 40 * 256);
        let s_ = samples(&r, &s, &t, w, &context.mask).unwrap();
        let pinned = context.design(&r).unwrap();
        context.verify(&pinned, &r).unwrap();
        assert_eq!(pinned.profile_mm[..=8].iter().all(|h| *h == 3_800), true);
        assert_eq!(
            pinned.profile_mm[248..].iter().all(|h| *h == 14_040),
            true,
            "the end platform keeps its tie-in elevation"
        );
        // 8 %/m between adjacent stations, everywhere, and 6 m of fill per cell.
        assert!(pinned.maximum_rise_mm_per_m <= 80);
        for (i, row) in s_.natural.iter().enumerate() {
            for h in row {
                assert!(
                    pinned.profile_mm[i] - h <= 6_000,
                    "station {i} fills {h} mm above the natural ground"
                );
            }
        }
        // A tie-in the corridor cannot reach is refused, not cut into place.
        let mut unreachable = r.clone();
        unreachable.start_level_mm = Some(0);
        assert!(context.design(&unreachable).is_err());
        // An allowance cannot be spent twice: a design claiming more than the
        // context's cap is refused by name.
        let mut forged = pinned.clone();
        forged.cut_allowance_mm = 2_001;
        assert!(context.verify(&forged, &r).is_err());
        // ...and a forged allowance still cannot publish a deeper cut.
        let mut forged = pinned;
        forged.cut_allowance_mm = 500;
        assert!(context.verify(&forged, &r).is_err());
    }
}
