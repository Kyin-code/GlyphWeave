//! Continuous footprint queries. No finite list of sample points can prove
//! absence of an intersection (a whole pond may fit between the samples).
use super::{EntityInstance, WaterGeometry, WaterKind, lake_radius_at, river_half_width_at};

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
}
