//! Unified candidate validator: applies compiled `Constraint`s to a candidate
//! position and returns the first reject reason, or Ok if all pass.
//!
//! The validator is deliberately decoupled from `worldgen` internals: it
//! receives terrain queries through `PlacementContext` closures, so the rules
//! engine can run against the existing generator without rewriting it.

use super::constraint::Constraint;
use super::errors::RejectReason;
use super::schema::{ItemKind, ObjectDescriptor};

/// ItemKind → canonical string (used for must_face matching).
pub fn kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Road => "road",
        ItemKind::Railway => "railway",
        ItemKind::Building => "building",
        ItemKind::Storefront => "storefront",
        ItemKind::Tree => "tree",
        ItemKind::Rock => "rock",
        ItemKind::Water => "water",
        ItemKind::Bridge => "bridge",
        ItemKind::Park => "park",
        ItemKind::Sidewalk => "sidewalk",
        ItemKind::Lamp => "lamp",
        ItemKind::Bench => "bench",
        ItemKind::BusStop => "bus_stop",
        ItemKind::FoodStall => "food_stall",
        ItemKind::Other => "other",
    }
}

/// Geometry of a candidate placement (footprint expanded by clearance).
#[derive(Debug, Clone, Copy)]
pub struct Footprint {
    /// World-space centre.
    pub cx: f32,
    pub cz: f32,
    /// Half extents of the collision box = footprint/2 + clearance.
    pub half_w: f32,
    pub half_d: f32,
}

impl Footprint {
    pub fn from_descriptor(desc: &ObjectDescriptor, cx: f32, cz: f32) -> Self {
        let hw = desc.geometry.footprint[0] * 0.5 + desc.geometry.clearance;
        let hd = desc.geometry.footprint[1] * 0.5 + desc.geometry.clearance;
        Footprint { cx, cz, half_w: hw, half_d: hd }
    }

    /// True if this footprint (AABB) overlaps `other`'s expanded box.
    pub fn overlaps(&self, ox: f32, oz: f32, o_half_w: f32, o_half_d: f32) -> bool {
        (self.cx - ox).abs() < self.half_w + o_half_w
            && (self.cz - oz).abs() < self.half_d + o_half_d
    }
}

/// Terrain / world queries the validator needs. Closures let the caller plug
/// in the existing generator's data without exposing internals.
pub struct PlacementContext<'a> {
    /// Ground height at (x, z) in metres (not metres/4 like the raw DataView).
    pub height_at: &'a dyn Fn(i32, i32) -> f32,
    /// Water surface height in metres; None = no water body nearby.
    pub water_level: &'a dyn Fn(i32, i32) -> Option<f32>,
    /// Local slope in metres per 100m at (x, z).
    pub slope_at: &'a dyn Fn(i32, i32) -> f32,
    /// Map bounds (min_x, min_z, max_x, max_z).
    pub bounds: (i32, i32, i32, i32),
    /// Grounding tolerance (m) for the 4-corner height test.
    pub grounding_tolerance: f32,
}

impl<'a> PlacementContext<'a> {
    pub fn height(&self, x: i32, z: i32) -> f32 {
        (self.height_at)(x, z)
    }
    pub fn water(&self, x: i32, z: i32) -> Option<f32> {
        (self.water_level)(x, z)
    }
    pub fn slope(&self, x: i32, z: i32) -> f32 {
        (self.slope_at)(x, z)
    }
    pub fn in_bounds(&self, x: f32, z: f32) -> bool {
        let (min_x, min_z, max_x, max_z) = self.bounds;
        x >= min_x as f32 && x <= max_x as f32 && z >= min_z as f32 && z <= max_z as f32
    }
}

/// A placed entity record the validator can consult for relation checks.
#[derive(Debug, Clone)]
pub struct PlacedKind {
    pub id: Option<String>,
    pub kind: super::schema::ItemKind,
    pub cx: f32,
    pub cz: f32,
    pub half_w: f32,
    pub half_d: f32,
}

/// Hard constraints only 鈥?returns the first violation.
pub fn check_hard(
    desc: &ObjectDescriptor,
    fp: &Footprint,
    ctx: &PlacementContext<'_>,
    hard: &[Constraint],
    placed: &[PlacedKind],
) -> Result<(), RejectReason> {
    // Bounds: sample the 4 corners of the collision box.
    let corners = [
        (fp.cx - fp.half_w, fp.cz - fp.half_d),
        (fp.cx + fp.half_w, fp.cz - fp.half_d),
        (fp.cx - fp.half_w, fp.cz + fp.half_d),
        (fp.cx + fp.half_w, fp.cz + fp.half_d),
    ];
    if !corners.iter().all(|(x, z)| ctx.in_bounds(*x, *z)) {
        return Err(RejectReason::OutOfBounds);
    }

    for c in hard {
        match c {
            Constraint::InsideBounds => {}
            Constraint::NoGeometryCollision => {
                // Overlap with ANY other placed object of the SAME kind (a
                // building cannot sit on another building, a road on a road).
                // Cross-kind adjacency (storefront flush against a road) is
                // governed by each descriptor's own `avoid` relations, so we
                // only forbid same-kind overlap here. Self is excluded by the
                // caller via stable ids.
                if placed.iter().any(|p| {
                    p.kind == desc.kind && fp.overlaps(p.cx, p.cz, p.half_w, p.half_d)
                }) {
                    return Err(RejectReason::GeometryCollision {
                        conflict_kind: desc.kind,
                    });
                }
            }
            Constraint::OnGround => {
                // Sample ground under the 4 corners; max deviation must be <= tol.
                let mut min_h = f32::MAX;
                let mut max_h = f32::MIN;
                for (x, z) in &corners {
                    let h = ctx.height(x.round() as i32, z.round() as i32);
                    min_h = min_h.min(h);
                    max_h = max_h.max(h);
                }
                if max_h - min_h > ctx.grounding_tolerance {
                    return Err(RejectReason::NotGrounded);
                }
            }
            Constraint::NotInWater => {
                // Any corner at/below water surface 鈬?in water.
                for (x, z) in &corners {
                    if let Some(w) = ctx.water(x.round() as i32, z.round() as i32) {
                        if ctx.height(x.round() as i32, z.round() as i32) <= w {
                            return Err(RejectReason::InWater);
                        }
                    }
                }
            }
            Constraint::MaxSlope(max) => {
                let s = ctx.slope(fp.cx.round() as i32, fp.cz.round() as i32);
                if s > *max {
                    return Err(RejectReason::SlopeTooHigh { slope: s, max: *max });
                }
            }
            Constraint::AvoidKind { kind, distance } => {
                let d = *distance;
                if placed.iter().any(|p| {
                    p.kind == *kind && p.cx == fp.cx && p.cz == fp.cz && p.half_w == fp.half_w && p.half_d == fp.half_d
                }) {
                    return Err(RejectReason::GeometryCollision { conflict_kind: *kind });
                }
                // Full geometry overlap + expanded-distance check.
                if placed.iter().any(|p| {
                    p.kind == *kind
                        && fp.overlaps(p.cx, p.cz, p.half_w + d, p.half_d + d)
                }) {
                    return Err(RejectReason::GeometryCollision { conflict_kind: *kind });
                }
            }
            Constraint::AvoidTag { tag, distance } => {
                let _ = (tag.as_str(), *distance);
                // MVP: tags require the placed list to carry tags; skip for now.
            }
            Constraint::RequireNear { kind, distance } => {
                let d = *distance;
                let ok = placed.iter().any(|p| {
                    p.kind == *kind
                        && ((p.cx - fp.cx).powi(2) + (p.cz - fp.cz).powi(2)).sqrt() <= d
                });
                if !ok {
                    return Err(RejectReason::MissingRequiredRelation { kind: *kind });
                }
            }
            Constraint::ClearAnchor { anchor, side, radius, must_face } => {
                let r = *radius;
                let side = side.as_str();
                // Anchor sits on a footprint edge, projected outward; ANY placed
                // entity (hard or soft: a tree can block a doorway too) inside
                // the anchor radius blocks the entrance.
                let a_hw = fp.half_w - desc.geometry.clearance;
                let a_hd = fp.half_d - desc.geometry.clearance;
                let (ax, az) = match side {
                    "front" | "north" => (fp.cx, fp.cz - a_hd - r * 0.5),
                    "back" | "south" => (fp.cx, fp.cz + a_hd + r * 0.5),
                    "left" | "west" => (fp.cx - a_hw - r * 0.5, fp.cz),
                    "right" | "east" => (fp.cx + a_hw + r * 0.5, fp.cz),
                    _ => (fp.cx, fp.cz - a_hd - r * 0.5),
                };
                let blocked = placed.iter().any(|p| {
                    // The entity the anchor must face is NOT a blocker (it's the
                    // desired frontage, e.g. a road); anything else in the zone is.
                    let is_target = must_face
                        .as_deref()
                        .map(|t| kind_name(p.kind) == t)
                        .unwrap_or(false);
                    !is_target
                        && ((p.cx - ax).powi(2) + (p.cz - az).powi(2)).sqrt() <= r
                });
                if blocked {
                    return Err(RejectReason::BlockedEntrance { anchor: anchor.clone() });
                }
                // must_face: the anchor direction must point at a placed entity
                // whose kind/tag matches the target string (e.g. "road").
                if let Some(target) = must_face {
                    let mut dir_x = 0.0_f32;
                    let mut dir_z = 0.0_f32;
                    match side {
                        "front" | "north" => dir_z = -1.0,
                        "back" | "south" => dir_z = 1.0,
                        "left" | "west" => dir_x = -1.0,
                        "right" | "east" => dir_x = 1.0,
                        _ => dir_z = -1.0,
                    }
                    // Look along the facing ray up to `r` metres for a matching
                    // entity (kind string or tag), measured from the anchor point.
                    let faces_target = placed.iter().any(|p| {
                        let rel_x = p.cx - ax;
                        let rel_z = p.cz - az;
                        // Dot with facing direction must be positive (ahead) and
                        // within the anchor radius.
                        let ahead = rel_x * dir_x + rel_z * dir_z;
                        ahead >= 0.0
                            && ahead <= r
                            && (kind_name(p.kind) == target.as_str())
                    });
                    if !faces_target {
                        return Err(RejectReason::BlockedEntrance {
                            anchor: anchor.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Soft scoring: sum of PreferNear weights for nearby matching entities.
/// Higher is better.
pub fn score_soft(fp: &Footprint, soft: &[Constraint], placed: &[PlacedKind]) -> f32 {
    let mut score = 0.0;
    for c in soft {
        if let Constraint::PreferNear { kind, distance, weight } = c {
            let best = placed
                .iter()
                .filter(|p| p.kind == *kind)
                .map(|p| ((p.cx - fp.cx).powi(2) + (p.cz - fp.cz).powi(2)).sqrt())
                .filter(|d| *d <= *distance)
                .fold(0.0_f32, |acc, d| acc.max(1.0 - d / distance));
            score += best * weight;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::constraint::compile;
    use crate::rules::schema::ItemKind;

    fn ctx(
        heights: Vec<(i32, i32, f32)>,
        waters: Vec<(i32, i32, f32)>,
        slopes: Vec<(i32, i32, f32)>,
        bounds: (i32, i32, i32, i32),
    ) -> PlacementContext<'static> {
        let height_at = move |x: i32, z: i32| {
            heights.iter().find(|(hx, hz, _)| *hx == x && *hz == z).map(|(_, _, h)| *h).unwrap_or(0.0)
        };
        let water_level = move |x: i32, z: i32| {
            waters.iter().find(|(wx, wz, _)| *wx == x && *wz == z).map(|(_, _, w)| *w)
        };
        let slope_at = move |x: i32, z: i32| {
            slopes.iter().find(|(sx, sz, _)| *sx == x && *sz == z).map(|(_, _, s)| *s).unwrap_or(0.0)
        };
        // Box the closures so we can return a 'static context.
        let height_box: Box<dyn Fn(i32, i32) -> f32> = Box::new(height_at);
        let water_box: Box<dyn Fn(i32, i32) -> Option<f32>> = Box::new(water_level);
        let slope_box: Box<dyn Fn(i32, i32) -> f32> = Box::new(slope_at);
        let height_leak: &'static dyn Fn(i32, i32) -> f32 = Box::leak(height_box);
        let water_leak: &'static dyn Fn(i32, i32) -> Option<f32> = Box::leak(water_box);
        let slope_leak: &'static dyn Fn(i32, i32) -> f32 = Box::leak(slope_box);
        PlacementContext {
            height_at: height_leak,
            water_level: water_leak,
            slope_at: slope_leak,
            bounds,
            grounding_tolerance: 0.5,
        }
    }

    fn tree_desc() -> ObjectDescriptor {
        toml::from_str(
            r#"
id = "tree_common"
kind = "tree"

[geometry]
footprint = [4.0, 4.0]
height = 8.0

[environment]
on_ground = true
not_in_water = true
max_slope = 30

[[relations.avoid]]
kind = "building"
distance = 1.0

[placement]
phase = "vegetation"
"#,
        )
        .unwrap()
    }

    #[test]
    fn reject_in_water() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        // Water surface at 1.0 everywhere, ground at 0.0 ⇒ submerged.
        let c = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| Some(1.0),
            slope_at: &|_, _| 0.0,
            bounds: (0, 0, 100, 100),
            grounding_tolerance: 0.5,
        };
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        let r = check_hard(&desc, &fp, &c, &hard, &[]).unwrap_err();
        assert_eq!(r, RejectReason::InWater);
    }

    #[test]
    fn reject_steep_slope() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![(5, 5, 0.0)], vec![], vec![(5, 5, 40.0)], (0, 0, 100, 100));
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        let r = check_hard(&desc, &fp, &c, &hard, &[]).unwrap_err();
        assert!(matches!(r, RejectReason::SlopeTooHigh { .. }));
    }

    #[test]
    fn reject_geometry_collision() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![], vec![], vec![], (0, 0, 100, 100));
        let placed = vec![PlacedKind {
            id: None,
            kind: ItemKind::Building,
            cx: 6.0,
            cz: 5.0,
            half_w: 6.0,
            half_d: 5.0,
        }];
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        let r = check_hard(&desc, &fp, &c, &hard, &placed).unwrap_err();
        assert!(matches!(r, RejectReason::GeometryCollision { .. }));
    }

    #[test]
    fn accept_clear_position() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![(5, 5, 0.0)], vec![], vec![(5, 5, 5.0)], (0, 0, 100, 100));
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        assert!(check_hard(&desc, &fp, &c, &hard, &[]).is_ok());
    }

    /// A tree must not block a storefront's public-access anchor (front).
    fn storefront_desc() -> ObjectDescriptor {
        toml::from_str(
            r#"
id = "storefront"
kind = "storefront"

[geometry]
footprint = [8.0, 6.0]
height = 4.0

[environment]
on_ground = true
not_in_water = true
max_slope = 8

[[relations.require]]
kind = "road"
distance = 8.0

[[anchors]]
id = "front"
side = "front"
kind = "public_access"
clear_radius = 3.0
must_face = "road"

[placement]
phase = "functional"
"#,
        )
        .unwrap()
    }

    #[test]
    fn soft_object_blocks_public_access() {
        let desc = storefront_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![], vec![], vec![], (0, 0, 200, 200));
        // A road ahead (satisfies require), but a TREE sits in the front zone.
        // The tree is just outside the storefront footprint (no geometry
        // collision) but inside the front anchor's clear radius.
        let placed = vec![
            PlacedKind { id: None, kind: ItemKind::Road, cx: 50.0, cz: 44.0, half_w: 4.0, half_d: 4.0 },
            PlacedKind { id: None, kind: ItemKind::Tree, cx: 50.0, cz: 43.0, half_w: 1.0, half_d: 1.0 },
        ];
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        let r = check_hard(&desc, &fp, &c, &hard, &placed).unwrap_err();
        assert!(matches!(r, RejectReason::BlockedEntrance { .. }));
    }

    #[test]
    fn storefront_front_must_face_road() {
        let desc = storefront_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![], vec![], vec![], (0, 0, 200, 200));
        // Road is behind (south), require passes but front must_face fails.
        let placed = vec![
            PlacedKind { id: None, kind: ItemKind::Road, cx: 50.0, cz: 58.0, half_w: 4.0, half_d: 4.0 },
        ];
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        let r = check_hard(&desc, &fp, &c, &hard, &placed).unwrap_err();
        assert!(matches!(r, RejectReason::BlockedEntrance { .. }));
    }

    #[test]
    fn storefront_front_faces_road_ok() {
        let desc = storefront_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(vec![], vec![], vec![], (0, 0, 200, 200));
        // Road placed ahead (north) of the front anchor → passes.
        let placed = vec![PlacedKind {
            id: None,
            kind: ItemKind::Road,
            cx: 50.0,
            cz: 44.0,
            half_w: 4.0,
            half_d: 4.0,
        }];
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        assert!(check_hard(&desc, &fp, &c, &hard, &placed).is_ok());
    }
}



