// Exact 1m longitudinal design against an immutable, full-width natural raster.
// A design is not a built road: no terrain, entities, ownership or gates change.
use super::{LandUseZone, SceneSpec, WaterGeometry, zoning::ZoningMask};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
const MAX_FILL_MM: i64 = 6000;
const MAX_RISE_MM: i64 = 80; // 8% per one-metre station; project constraint, not a road code.
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
    pub road_mesh: CorridorMesh,
    pub support_mesh: CorridorMesh,
    pub proposed_fill_volume_m3: f64,
    pub maximum_fill_mm: i64,
    pub maximum_rise_mm_per_m: i64,
    pub support_area_m2: u64,
}
struct Samples {
    length: usize,
    half: i32,
    points: Vec<[i32; 2]>,
    natural: Vec<Vec<i64>>,
    lower: Vec<i64>,
    upper: Vec<i64>,
    zoning_hash: String,
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
        lower: vec![],
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
            row.push(h);
        }
        for (active, level) in [
            (i <= r.junction_length_m as usize, r.start_level_mm),
            (length - i <= r.junction_length_m as usize, r.end_level_mm),
        ] {
            if active {
                if let Some(h) = level {
                    lo = lo.max(i64::from(h));
                    hi = hi.min(i64::from(h));
                }
            }
        }
        result.points.push(point);
        result.natural.push(row);
        result.lower.push(lo);
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
        })
    }
    pub fn design(&self, r: &CorridorRequest) -> Result<CorridorDesign, TerrainError> {
        let s = samples(r, self.scene, self.terrain, self.water, &self.mask)?;
        let profile = solve(&s.lower, &s.upper, i64::from(r.max_rise_mm_per_m))?;
        let (volume, fill, rise) = quantities(&s, &profile);
        Ok(CorridorDesign {
            format: "glyphweave.road-corridor-design".into(),
            version: 1,
            status: "design_only_not_applied".into(),
            model: "full-width-fill-only-minimum-lipschitz-envelope-v1".into(),
            request: r.clone(),
            natural_heightfield_hash: self.natural_hash.clone(),
            zoning_hash: s.zoning_hash.clone(),
            road_mesh: mesh(&s.points, &profile, (r.road_width_m / 2) as i32),
            support_mesh: mesh(&s.points, &profile, s.half),
            profile_mm: profile,
            proposed_fill_volume_m3: volume,
            maximum_fill_mm: fill,
            maximum_rise_mm_per_m: rise,
            support_area_m2: s.length as u64 * s.half as u64 * 2,
        })
    }

    /// Infer a constant endpoint platform without enumerating or rounding to
    /// whole metres. Any higher endpoint only raises the least profile, so it
    /// cannot rescue an upper-bound violation of this minimum feasible choice.
    pub fn infer_end(&self, r: &CorridorRequest) -> Result<CorridorRequest, TerrainError> {
        if r.end_level_mm.is_some() {
            return Ok(r.clone());
        }
        let mut source = samples(r, self.scene, self.terrain, self.water, &self.mask)?;
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
        for i in begin..=source.length {
            source.lower[i] = source.lower[i].max(low);
            source.upper[i] = source.upper[i].min(high);
        }
        let y = solve(&source.lower, &source.upper, i64::from(r.max_rise_mm_per_m))?;
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
        if d.format != "glyphweave.road-corridor-design"
            || d.version != 1
            || d.status != "design_only_not_applied"
            || d.model != "full-width-fill-only-minimum-lipschitz-envelope-v1"
            || d.request != *r
            || d.natural_heightfield_hash != self.natural_hash
            || d.zoning_hash != s.zoning_hash
            || d.profile_mm.len() != s.length + 1
        {
            return Err(error("corridor design provenance mismatch"));
        }
        let y = &d.profile_mm;
        let rise = i64::from(r.max_rise_mm_per_m);
        for i in 0..y.len() {
            let mut expected = s.lower[i];
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
            if y[i] < s.lower[i] || y[i] > s.upper[i] || y[i] != expected {
                return Err(error(format!(
                    "corridor profile violates source/grade/minimum at station {i}"
                )));
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
    AlignmentContext::new(scene, terrain, water, zones)?.verify(d, r)
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
}
