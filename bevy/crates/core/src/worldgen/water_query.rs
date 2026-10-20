//! Continuous footprint queries. No finite list of sample points can prove
//! absence of an intersection (a whole pond may fit between the samples).
use super::{EntityInstance, WaterGeometry, WaterKind, lake_radius_at, river_half_width_at};

/// The frozen water-shore clearance band, in metres.
///
/// It is ONE number with THREE readers, which is the whole point of naming it:
///
///  * the published scale audit refuses any object whose footprint comes closer
///    than this to the water geometry (`footprint_intersects_water(entity, water,
///    margin)`); it used to be the bare literal `4.0` in the CLI;
///  * a waterfront roadside pad has to keep this much dry ground between itself
///    and the water body (`WaterGeometry::clears_shore`), because the strip is
///    reserved public passage and a lot may not claim it;
///  * the shoreline public passage band IS this strip (`WaterGeometry::shore_band_cell`).
///
/// The generation side used to ask for 1.0 m while the audit asked for 4.0 m, so
/// a legal bake could produce objects the audit then refused. A profile may read
/// this constant; it may never widen it.
pub const WATER_SHORE_CLEARANCE_M: f64 = 4.0;

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    z: f64,
    hx: f64,
    hz: f64,
    c: f64,
    s: f64,
}
impl Rect {
    fn corners(self) -> [(f64, f64); 4] {
        [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)].map(|(x, z)| {
            (
                self.x + x * self.hx * self.c + z * self.hz * self.s,
                self.z - x * self.hx * self.s + z * self.hz * self.c,
            )
        })
    }
    // Separating axes: this rectangle's two axes and world X/Z.
    fn overlaps_box(self, x: f64, z: f64, hx: f64, hz: f64) -> bool {
        let dx = x - self.x;
        let dz = z - self.z;
        dx.abs() < self.hx * self.c.abs() + self.hz * self.s.abs() + hx
            && dz.abs() < self.hx * self.s.abs() + self.hz * self.c.abs() + hz
            && (dx * self.c - dz * self.s).abs() < self.hx + hx * self.c.abs() + hz * self.s.abs()
            && (dx * self.s + dz * self.c).abs() < self.hz + hx * self.s.abs() + hz * self.c.abs()
    }
    fn contains(self, x: f64, z: f64) -> bool {
        let dx = x - self.x;
        let dz = z - self.z;
        (dx * self.c - dz * self.s).abs() <= self.hx && (dx * self.s + dz * self.c).abs() <= self.hz
    }
}

impl WaterGeometry {
    /// The raster-side waterfront opt-in. `false` for every input that did not
    /// state `automaticRoadExtensions.allowStructuredWater`.
    pub fn road_water_crossings(self) -> bool {
        self.road_water_crossings
    }

    /// May an earthwork fill be placed at this cell?
    ///
    /// Only ever `false` for an opted-in structured-water scene: the water bed,
    /// the banks and the surface are then the natural raster exactly, so a road
    /// that reaches the water has to be carried by a registered structure deck.
    pub fn forbids_earthwork(self, x: f64, z: f64) -> bool {
        self.road_water_crossings && self.contains_point(x, z)
    }

    pub fn contains_point(self, x: f64, z: f64) -> bool {
        match self.kind {
            WaterKind::None => false,
            WaterKind::Lake => lake_radius_at(x, z, self) < 1.,
            WaterKind::River => {
                let min_z = self.center_z - self.half_depth;
                (min_z..=self.center_z + self.half_depth).contains(&z)
                    && (x - self.center_x).abs()
                        < river_half_width_at(
                            z - min_z,
                            (self.half_depth * 2.) as u32,
                            self.half_width,
                        )
            }
        }
    }

    /// Does this scene carry the shoreline public passage band at all?
    ///
    /// Only the opt-in road-first-over-water authorization (`style.automatic
    /// RoadExtensions.allowStructuredWater`) opens it, so every world that did
    /// not state that switch keeps its published surfaces, its ledger and its
    /// audits byte for byte.
    pub fn shore_band(self) -> bool {
        self.road_water_crossings && self.kind != WaterKind::None
    }

    /// Is the 1 m cell `(x, z)` part of the shoreline public passage band?
    ///
    /// The band is the strip of DRY ground that lies within
    /// [`WATER_SHORE_CLEARANCE_M`] of the water body, measured from the water's
    /// own published geometry. A cell is in it when its centre is outside the
    /// water body and the body itself comes within the clearance of that centre.
    ///
    /// Two consequences follow from using one constant for the strip and for the
    /// lots, and both are audited rather than assumed:
    ///
    ///  * a pad that keeps the clearance (its footprint dilated by the clearance
    ///    does not meet the water) contains no band cell, so "no building in the
    ///    band" and "the lot keeps the shore clearance" are the same geometric
    ///    fact and cannot disagree;
    ///  * the band is a continuous strip along every shore by construction - it
    ///    is an offset of the waterline, not a collected set of patches.
    pub fn shore_band_cell(self, x: i32, z: i32) -> bool {
        if !self.shore_band() {
            return false;
        }
        let (cx, cz) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
        !self.contains_point(cx, cz)
            && self.intersects_axis_aligned(
                cx,
                cz,
                WATER_SHORE_CLEARANCE_M,
                WATER_SHORE_CLEARANCE_M,
            )
    }

    /// Does an axis-aligned footprint at `(x, z)` keep the shore clearance?
    ///
    /// The footprint is dilated by [`WATER_SHORE_CLEARANCE_M`] and tested against
    /// the water body: `false` means the water comes closer than the clearance
    /// (and therefore that the footprint also touches the band). The dilation is
    /// a square, which CONTAINS the Euclidean disc of the same radius, so a
    /// `true` here proves the clearance in metres, never merely in the other
    /// direction.
    pub fn clears_shore(self, x: f64, z: f64, half_w: f64, half_d: f64) -> bool {
        !self.intersects_axis_aligned(
            x,
            z,
            half_w + WATER_SHORE_CLEARANCE_M,
            half_d + WATER_SHORE_CLEARANCE_M,
        )
    }

    fn intersects_rect(self, rect: Rect) -> bool {
        match self.kind {
            WaterKind::None => false,
            WaterKind::Lake => {
                if rect.contains(self.center_x, self.center_z) {
                    return true;
                }
                // Scale ellipse to unit circle. Minimum distance to each polygon
                // edge catches edges crossing water and enclosed water bodies.
                let corners = rect.corners().map(|(x, z)| {
                    (
                        (x - self.center_x) / self.half_width.max(1.),
                        (z - self.center_z) / self.half_depth.max(1.),
                    )
                });
                (0..4).any(|i| {
                    let a = corners[i];
                    let b = corners[(i + 1) % 4];
                    let d = (b.0 - a.0, b.1 - a.1);
                    let length = d.0 * d.0 + d.1 * d.1;
                    let t = if length > 0. {
                        (-(a.0 * d.0 + a.1 * d.1) / length).clamp(0., 1.)
                    } else {
                        0.
                    };
                    (a.0 + t * d.0).powi(2) + (a.1 + t * d.1).powi(2) < 1.
                })
            }
            WaterKind::River => {
                let half_z = rect.hx * rect.s.abs() + rect.hz * rect.c.abs();
                let lo = (rect.z - half_z).max(self.center_z - self.half_depth);
                let hi = (rect.z + half_z).min(self.center_z + self.half_depth);
                if hi <= lo {
                    return false;
                }
                self.river_interval(rect, lo, hi, 0)
            }
        }
    }

    fn river_interval(self, rect: Rect, lo: f64, hi: f64, depth: u8) -> bool {
        let mid = (lo + hi) * 0.5;
        let full_depth = (self.half_depth * 2.) as u32;
        let width = river_half_width_at(
            mid - (self.center_z - self.half_depth),
            full_depth,
            self.half_width,
        );
        // |sin'| <= 1; max(sin,0) and max(width,1) preserve this Lipschitz bound.
        let lipschitz = if full_depth < 3000 {
            0.
        } else {
            self.half_width.abs() * std::f64::consts::TAU * (0.08 * 1.15 + 0.045 * 3.)
                / f64::from(full_depth)
        };
        let error = lipschitz * (hi - lo) * 0.5;
        if !rect.overlaps_box(self.center_x, mid, width + error, (hi - lo) * 0.5) {
            return false;
        }
        if rect.overlaps_box(self.center_x, mid, (width - error).max(1.), (hi - lo) * 0.5) {
            return true;
        }
        // Unresolved contact within 0.1mm is conservatively occupied. This is a
        // bounded numerical tolerance, not a sampling gap proportional to size.
        if error <= 0.0001 || depth >= 24 {
            return true;
        }
        self.river_interval(rect, lo, mid, depth + 1)
            || self.river_interval(rect, mid, hi, depth + 1)
    }

    /// Continuous swept pedestrian rectangle, not discrete water probes.
    pub(crate) fn intersects_corridor(
        self,
        ax: f64,
        az: f64,
        bx: f64,
        bz: f64,
        half_width: f64,
    ) -> bool {
        let dx = bx - ax;
        let dz = bz - az;
        let length = dx.hypot(dz);
        let (c, s) = if length > 1e-9 {
            (dx / length, -dz / length)
        } else {
            (1., 0.)
        };
        self.intersects_rect(Rect {
            x: (ax + bx) * 0.5,
            z: (az + bz) * 0.5,
            hx: length * 0.5 + half_width,
            hz: half_width,
            c,
            s,
        })
    }

    pub(crate) fn intersects_axis_aligned(self, x: f64, z: f64, hx: f64, hz: f64) -> bool {
        self.intersects_rect(Rect {
            x,
            z,
            hx,
            hz,
            c: 1.,
            s: 0.,
        })
    }
}

pub fn footprint_intersects_water(
    entity: &EntityInstance,
    water: WaterGeometry,
    margin: f64,
) -> bool {
    if ![entity.width_m, entity.depth_m, entity.rotation_y_deg]
        .iter()
        .all(|v| v.is_finite())
        || entity.width_m <= 0.
        || entity.depth_m <= 0.
        || !margin.is_finite()
        || margin < 0.
    {
        return true;
    }
    let angle = f64::from(entity.rotation_y_deg).to_radians();
    water.intersects_rect(Rect {
        x: f64::from(entity.world_x),
        z: f64::from(entity.world_z),
        hx: f64::from(entity.width_m) * 0.5 + margin,
        hz: f64::from(entity.depth_m) * 0.5 + margin,
        c: angle.cos(),
        s: angle.sin(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn water(kind: WaterKind) -> WaterGeometry {
        WaterGeometry {
            kind,
            center_x: 0.,
            center_z: 0.,
            half_width: 0.1,
            half_depth: 10.,
            scene_width_m: 512,
            scene_depth_m: 512,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
            geography: crate::worldgen::natural_geography::GeographyControls::default(),
        }
    }
    fn rect(x: f64, z: f64, hx: f64, hz: f64, angle: f64) -> Rect {
        Rect {
            x,
            z,
            hx,
            hz,
            c: angle.to_radians().cos(),
            s: angle.to_radians().sin(),
        }
    }
    #[test]
    fn enclosed_pond_and_submetre_river_are_not_sampling_gaps() {
        let r = rect(30., 0., 50., 50., 0.);
        assert!(water(WaterKind::Lake).intersects_rect(r));
        assert!(water(WaterKind::River).intersects_rect(r));
    }
    #[test]
    fn rotated_corner_aabb_does_not_reject_dry_footprint() {
        let mut w = water(WaterKind::Lake);
        w.center_x = 6.;
        w.center_z = 6.;
        w.half_depth = 1.;
        assert!(!w.intersects_rect(rect(0., 0., 10., 1., 45.)));
        w.center_z = -6.;
        assert!(w.intersects_rect(rect(0., 0., 10., 1., 45.)));
    }
    #[test]
    fn finite_river_end_and_tangent_are_dry() {
        let w = water(WaterKind::River);
        assert!(!w.intersects_rect(rect(0., 12., 4., 1., 0.)));
        assert!(!w.intersects_rect(rect(3., 0., 2., 1., 0.)));
        assert!(w.intersects_rect(rect(2.99, 0., 2., 1., 0.)));
    }
    #[test]
    fn curved_river_matches_interior_points_across_width_extrema() {
        let mut w = water(WaterKind::River);
        w.half_depth = 5000.;
        w.half_width = 300.;
        for z in (-4999..5000).step_by(71) {
            let width = river_half_width_at(f64::from(z) + 5000., 10000, 300.);
            assert!(w.intersects_rect(rect(width - 0.01, f64::from(z), 0.001, 0.001, 23.)));
            assert!(!w.intersects_rect(rect(width + 0.1, f64::from(z), 0.001, 0.001, 23.)));
        }
    }

    /// A waterfront scene: a 40 m channel down the middle of a 512 m scene.
    fn river_scene() -> WaterGeometry {
        let mut w = water(WaterKind::River);
        w.center_x = 256.;
        w.center_z = 256.;
        w.half_width = 20.;
        w.half_depth = 256.;
        w.road_water_crossings = true;
        w
    }

    /// The clearance is ONE number for the audit margin, the lot rule and the
    /// band width. If somebody loosens it here, the published scale audit's
    /// margin (which reads this constant) loosens with it - so pin the value.
    #[test]
    fn the_shore_clearance_is_one_named_constant() {
        assert_eq!(WATER_SHORE_CLEARANCE_M, 4.0);
    }

    /// The band is dry, hugs the water, and is absent without the opt-in.
    #[test]
    fn the_band_is_dry_hugs_the_water_and_needs_the_opt_in() {
        let mut w = river_scene();
        // The waterline at z = 256 is |x - 256| < 20, so the wet cells are
        // x = 236..=275 and the 4 m band of dry cell centres is
        // x = 232..=235 (west) and x = 276..=279 (east).
        for x in 236..276 {
            assert!(!w.shore_band_cell(x, 256), "x={x} is in the water body");
        }
        for x in 276..280 {
            assert!(w.shore_band_cell(x, 256), "x={x} is in the 4 m band");
        }
        assert!(!w.shore_band_cell(280, 256), "x=280 is past the band");
        for x in 232..236 {
            assert!(w.shore_band_cell(x, 256), "x={x} is in the west band");
        }
        assert!(!w.shore_band_cell(231, 256));
        // No opt-in, no band and no pad exemption: the switch is the only gate.
        w.road_water_crossings = false;
        assert!(!w.shore_band());
        assert!(!w.shore_band_cell(277, 256));
    }

    /// The lot rule and the band are the same geometric fact: a pad that keeps
    /// the clearance owns no band cell, and the band is a continuous strip on
    /// each bank rather than a collection of lucky cells.
    #[test]
    fn a_pad_that_clears_the_shore_owns_no_band_cell_and_the_band_is_continuous() {
        let w = river_scene();
        let (mut band_w, mut band_e) = (0_u32, 0_u32);
        for z in 0..512 {
            // Each bank's band must be one uninterrupted run along the shore.
            let west: Vec<i32> = (232..236).filter(|x| w.shore_band_cell(*x, z)).collect();
            let east: Vec<i32> = (276..280).filter(|x| w.shore_band_cell(*x, z)).collect();
            assert_eq!(west.len(), 4, "west band broken at z={z}: {west:?}");
            assert_eq!(east.len(), 4, "east band broken at z={z}: {east:?}");
            band_w += west.len() as u32;
            band_e += east.len() as u32;
        }
        assert_eq!(band_w, band_e);
        for z in (16..512).step_by(48) {
            for x in (16..512).step_by(32) {
                // A 24x24 lot centred here covers x-12..=x+11 (cell centres
                // x-11.5..=x+11.5), which is the footprint `clears_shore` dilates.
                let clears = w.clears_shore(f64::from(x), f64::from(z), 12., 12.);
                let mut owns_band = false;
                for zz in z - 12..=z + 11 {
                    for xx in x - 12..=x + 11 {
                        owns_band |= w.shore_band_cell(xx, zz);
                    }
                }
                assert_eq!(
                    clears, !owns_band,
                    "lot at ({x},{z}): clearing the shore and owning no band cell must agree"
                );
            }
        }
    }
}
