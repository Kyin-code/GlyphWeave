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
        Footprint {
            cx,
            cz,
            half_w: hw,
            half_d: hd,
        }
    }

    /// Footprint with a 90° rotation applied (width/depth swap).
    pub fn rotated(&self) -> Footprint {
        Footprint {
            cx: self.cx,
            cz: self.cz,
            half_w: self.half_d,
            half_d: self.half_w,
        }
    }

    /// True if this footprint (AABB) overlaps `other`'s expanded box.
    pub fn overlaps(&self, ox: f32, oz: f32, o_half_w: f32, o_half_d: f32) -> bool {
        (self.cx - ox).abs() < self.half_w + o_half_w
            && (self.cz - oz).abs() < self.half_d + o_half_d
    }

    /// Centre, corners and edge midpoints for footprint-aware environment checks.
    pub fn sample_points(&self) -> [(i32, i32); 9] {
        let x0 = (self.cx - self.half_w).round() as i32;
        let x1 = (self.cx + self.half_w).round() as i32;
        let z0 = (self.cz - self.half_d).round() as i32;
        let z1 = (self.cz + self.half_d).round() as i32;
        let cx = self.cx.round() as i32;
        let cz = self.cz.round() as i32;
        [
            (cx, cz),
            (x0, z0),
            (x1, z0),
            (x0, z1),
            (x1, z1),
            (cx, z0),
            (cx, z1),
            (x0, cz),
            (x1, cz),
        ]
    }
}

/// Terrain / world queries the validator needs. Closures let the caller plug
/// in the existing generator's data without exposing internals.
pub struct PlacementContext<'a> {
    /// Ground height at (x, z) in metres (not metres/4 like the raw DataView).
    pub height_at: &'a dyn Fn(i32, i32) -> f32,
    /// Water surface height in metres; None = no water body nearby.
    pub water_level: &'a dyn Fn(i32, i32) -> Option<f32>,
    /// Local slope in metres per 100m at (x, z), used as a fallback when
    /// no footprint-aware query is supplied.
    pub slope_at: &'a dyn Fn(i32, i32) -> f32,
    /// Optional slope query receiving the candidate's actual half extents.
    /// This keeps large/rotated source entities from being judged by a fixed
    /// 8m sampling window.
    pub slope_at_footprint: Option<&'a dyn Fn(i32, i32, i32, i32) -> f32>,
    /// Map bounds (min_x, min_z, max_x, max_z).
    pub bounds: (i32, i32, i32, i32),
    /// Grounding tolerance (m) for the 4-corner height test.
    pub grounding_tolerance: f32,
    /// Biome at (x, z). None ⇒ biome constraints are not checked.
    pub biome_at: Option<&'a dyn Fn(i32, i32) -> super::schema::Biome>,
    /// Hazards at (x, z). None ⇒ hazard constraints are not checked.
    pub hazard_at: Option<&'a dyn Fn(i32, i32) -> Vec<super::schema::HazardKind>>,
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
    pub fn slope_for(&self, fp: &Footprint) -> f32 {
        if let Some(query) = self.slope_at_footprint {
            query(
                fp.cx.round() as i32,
                fp.cz.round() as i32,
                fp.half_w.round().max(1.0) as i32,
                fp.half_d.round().max(1.0) as i32,
            )
        } else {
            fp.sample_points()
                .into_iter()
                .map(|(x, z)| self.slope(x, z))
                .fold(0.0_f32, f32::max)
        }
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
    /// Half extents INCLUDING clearance (the effective occupied box).
    pub half_w: f32,
    pub half_d: f32,
    /// Semantic tags (e.g. "flammable", "public_access") for tag-based rules.
    pub tags: Vec<String>,
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
            Constraint::InsideBounds => {
                // No-op: the 4-corner bounds test above already covers this.
            }
            Constraint::NoGeometryCollision => {
                // Overlap policy is symmetric: inspecting A against B produces
                // the same answer as inspecting B against A. Roads, bridges
                // and sidewalks are the only explicit shared transport surface
                // group; they may overlap one another at crossings, but no
                // other pair shares footprint space by default.
                let transport = |kind: ItemKind| {
                    matches!(kind, ItemKind::Road | ItemKind::Bridge | ItemKind::Sidewalk)
                };
                let conflict = placed.iter().find(|p| {
                    fp.overlaps(p.cx, p.cz, p.half_w, p.half_d)
                        && !(transport(desc.kind) || transport(p.kind))
                });
                if let Some(conflict) = conflict {
                    return Err(RejectReason::GeometryCollision {
                        conflict_kind: conflict.kind,
                        conflict_id: conflict.id.clone(),
                    });
                }
            }
            Constraint::OnGround => {
                // Placement happens before the generator bakes support pads.
                // Do not mistake natural relief for a floating object: the
                // worldgen grounding pass will place the bottom at the highest
                // footprint sample and terrain carving will flatten the pad.
                // The rule still rejects missing/non-finite terrain samples.
                if fp
                    .sample_points()
                    .iter()
                    .any(|(x, z)| !ctx.height(*x, *z).is_finite())
                {
                    return Err(RejectReason::NotGrounded);
                }
            }
            Constraint::NotInWater => {
                for (x, z) in fp.sample_points() {
                    if let Some(w) = ctx.water(x, z) {
                        if ctx.height(x, z) <= w {
                            return Err(RejectReason::InWater);
                        }
                    }
                }
            }
            Constraint::MaxSlope(max) => {
                let s = ctx.slope_for(fp);
                if s > *max {
                    return Err(RejectReason::SlopeTooHigh {
                        slope: s,
                        max: *max,
                    });
                }
            }
            Constraint::AllowedBiome(allowed) => {
                if let Some(biome_at) = ctx.biome_at {
                    for (x, z) in fp.sample_points() {
                        let biome = biome_at(x, z);
                        if !allowed.contains(&biome) {
                            return Err(RejectReason::ForbiddenBiome { biome });
                        }
                    }
                }
            }
            Constraint::ForbiddenHazard(forbidden) => {
                if let Some(hazard_at) = ctx.hazard_at {
                    for (x, z) in fp.sample_points() {
                        if let Some(hazard) = hazard_at(x, z)
                            .into_iter()
                            .find(|hazard| forbidden.contains(hazard))
                        {
                            return Err(RejectReason::ForbiddenHazard { hazard });
                        }
                    }
                }
            }
            Constraint::AvoidKind { kind, distance } => {
                let d = *distance;
                if let Some(conflict) = placed.iter().find(|p| {
                    p.kind == *kind && fp.overlaps(p.cx, p.cz, p.half_w + d, p.half_d + d)
                }) {
                    return Err(RejectReason::GeometryCollision {
                        conflict_kind: *kind,
                        conflict_id: conflict.id.clone(),
                    });
                }
            }
            Constraint::AvoidTag { tag, distance } => {
                let d = *distance;
                let blocked = placed.iter().any(|p| {
                    p.tags.iter().any(|t| t == tag)
                        && fp.overlaps(p.cx, p.cz, p.half_w + d, p.half_d + d)
                });
                if blocked {
                    return Err(RejectReason::GeometryCollision {
                        conflict_kind: ItemKind::Other,
                        conflict_id: placed
                            .iter()
                            .find(|p| {
                                p.tags.iter().any(|t| t == tag)
                                    && fp.overlaps(p.cx, p.cz, p.half_w + d, p.half_d + d)
                            })
                            .and_then(|p| p.id.clone()),
                    });
                }
            }
            Constraint::RequireNear { kind, distance } => {
                let d = *distance;
                let ok = placed.iter().any(|p| {
                    p.kind == *kind && ((p.cx - fp.cx).powi(2) + (p.cz - fp.cz).powi(2)).sqrt() <= d
                });
                if !ok {
                    return Err(RejectReason::MissingRequiredRelation { kind: *kind });
                }
            }
            Constraint::ClearAnchor {
                anchor,
                side,
                radius,
                must_face,
            } => {
                let r = radius.max(0.0);
                let side = side.as_str();
                let raw_hw = (fp.half_w - desc.geometry.clearance).max(0.0);
                let raw_hd = (fp.half_d - desc.geometry.clearance).max(0.0);
                // Model the entrance as an outward rectangular corridor, not a
                // center-point circle. This catches a tree/building whose
                // footprint clips the doorway while its center is outside.
                let (corridor_cx, corridor_cz, corridor_hw, corridor_hd, dir_x, dir_z) = match side
                {
                    "front" | "north" => {
                        (fp.cx, fp.cz - raw_hd - r * 0.5, r * 0.5, r * 0.5, 0.0, -1.0)
                    }
                    "back" | "south" => {
                        (fp.cx, fp.cz + raw_hd + r * 0.5, r * 0.5, r * 0.5, 0.0, 1.0)
                    }
                    "left" | "west" => {
                        (fp.cx - raw_hw - r * 0.5, fp.cz, r * 0.5, r * 0.5, -1.0, 0.0)
                    }
                    "right" | "east" => {
                        (fp.cx + raw_hw + r * 0.5, fp.cz, r * 0.5, r * 0.5, 1.0, 0.0)
                    }
                    _ => (fp.cx, fp.cz - raw_hd - r * 0.5, r * 0.5, r * 0.5, 0.0, -1.0),
                };
                let target_matches = |p: &PlacedKind, target: &str| {
                    kind_name(p.kind) == target || p.tags.iter().any(|tag| tag == target)
                };
                let blocked = placed.iter().any(|p| {
                    let is_target = must_face
                        .as_deref()
                        .map(|t| target_matches(p, t))
                        .unwrap_or(false);
                    !is_target
                        && (p.cx - corridor_cx).abs() < corridor_hw + p.half_w
                        && (p.cz - corridor_cz).abs() < corridor_hd + p.half_d
                });
                if blocked {
                    return Err(RejectReason::BlockedEntrance {
                        anchor: anchor.clone(),
                    });
                }
                if let Some(target) = must_face {
                    let faces_target = placed.iter().any(|p| {
                        if !target_matches(p, target) {
                            return false;
                        }
                        let rel_x = p.cx - corridor_cx;
                        let rel_z = p.cz - corridor_cz;
                        let ahead = rel_x * dir_x + rel_z * dir_z;
                        let lateral = (rel_x * dir_z - rel_z * dir_x).abs();
                        ahead >= -p.half_w.max(p.half_d)
                            && ahead <= r + p.half_w.max(p.half_d)
                            && lateral <= corridor_hw + corridor_hd
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
        if let Constraint::PreferNear {
            kind,
            distance,
            weight,
        } = c
        {
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
            heights
                .iter()
                .find(|(hx, hz, _)| *hx == x && *hz == z)
                .map(|(_, _, h)| *h)
                .unwrap_or(0.0)
        };
        let water_level = move |x: i32, z: i32| {
            waters
                .iter()
                .find(|(wx, wz, _)| *wx == x && *wz == z)
                .map(|(_, _, w)| *w)
        };
        let slope_at = move |x: i32, z: i32| {
            slopes
                .iter()
                .find(|(sx, sz, _)| *sx == x && *sz == z)
                .map(|(_, _, s)| *s)
                .unwrap_or(0.0)
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
            slope_at_footprint: None,
            bounds,
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
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
            slope_at_footprint: None,
            bounds: (0, 0, 100, 100),
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        };
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        let r = check_hard(&desc, &fp, &c, &hard, &[]).unwrap_err();
        assert_eq!(r, RejectReason::InWater);
    }

    #[test]
    fn footprint_slope_uses_actual_candidate_extents() {
        let mut desc = tree_desc();
        desc.geometry.footprint = [40.0, 20.0];
        let (hard, _) = compile(&desc);
        let c = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| None,
            slope_at: &|_, _| 0.0,
            slope_at_footprint: Some(&|_, _, half_w, _| {
                if half_w >= 20 { 45.0 } else { 0.0 }
            }),
            bounds: (0, 0, 100, 100),
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        };
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        let r = check_hard(&desc, &fp, &c, &hard, &[]).unwrap_err();
        assert!(matches!(r, RejectReason::SlopeTooHigh { .. }));
    }

    #[test]
    fn reject_steep_slope() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(
            vec![(5, 5, 0.0)],
            vec![],
            vec![(5, 5, 40.0)],
            (0, 0, 100, 100),
        );
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
            tags: vec![],
        }];
        let fp = Footprint::from_descriptor(&desc, 5.0, 5.0);
        let r = check_hard(&desc, &fp, &c, &hard, &placed).unwrap_err();
        assert!(matches!(r, RejectReason::GeometryCollision { .. }));
    }

    #[test]
    fn accept_clear_position() {
        let desc = tree_desc();
        let (hard, _) = compile(&desc);
        let c = ctx(
            vec![(5, 5, 0.0)],
            vec![],
            vec![(5, 5, 5.0)],
            (0, 0, 100, 100),
        );
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
            PlacedKind {
                id: None,
                kind: ItemKind::Road,
                cx: 50.0,
                cz: 44.0,
                half_w: 4.0,
                half_d: 4.0,
                tags: vec![],
            },
            PlacedKind {
                id: None,
                kind: ItemKind::Tree,
                cx: 50.0,
                cz: 44.0,
                half_w: 1.0,
                half_d: 1.0,
                tags: vec![],
            },
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
        let placed = vec![PlacedKind {
            id: None,
            kind: ItemKind::Road,
            cx: 50.0,
            cz: 58.0,
            half_w: 4.0,
            half_d: 4.0,
            tags: vec![],
        }];
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        let r = check_hard(&desc, &fp, &c, &hard, &placed).unwrap_err();
        assert!(matches!(r, RejectReason::BlockedEntrance { .. }));
    }

    #[test]
    fn hard_and_soft_collision_is_symmetric() {
        let building: ObjectDescriptor = toml::from_str(
            r#"
id = "building"
kind = "building"
[geometry]
footprint = [8.0, 8.0]
height = 4.0
[placement]
phase = "building"
"#,
        )
        .unwrap();
        let tree: ObjectDescriptor = toml::from_str(
            r#"
id = "tree"
kind = "tree"
[geometry]
footprint = [4.0, 4.0]
height = 8.0
[placement]
phase = "vegetation"
"#,
        )
        .unwrap();
        let ctx = ctx(vec![], vec![], vec![], (0, 0, 200, 200));
        let building_fp = Footprint::from_descriptor(&building, 50.0, 50.0);
        let tree_fp = Footprint::from_descriptor(&tree, 50.0, 50.0);
        let building_placed = PlacedKind {
            id: Some("building-1".into()),
            kind: ItemKind::Building,
            cx: 50.0,
            cz: 50.0,
            half_w: building_fp.half_w,
            half_d: building_fp.half_d,
            tags: vec![],
        };
        let tree_placed = PlacedKind {
            id: Some("tree-1".into()),
            kind: ItemKind::Tree,
            cx: 50.0,
            cz: 50.0,
            half_w: tree_fp.half_w,
            half_d: tree_fp.half_d,
            tags: vec!["flammable".into()],
        };
        let (building_hard, _) = compile(&building);
        let (tree_hard, _) = compile(&tree);
        assert!(matches!(
            check_hard(&building, &building_fp, &ctx, &building_hard, &[tree_placed.clone()]),
            Err(RejectReason::GeometryCollision { conflict_id: Some(id), .. }) if id == "tree-1"
        ));
        assert!(matches!(
            check_hard(&tree, &tree_fp, &ctx, &tree_hard, &[building_placed]),
            Err(RejectReason::GeometryCollision { conflict_id: Some(id), .. }) if id == "building-1"
        ));
    }

    #[test]
    fn avoid_tag_and_footprint_biome_are_enforced() {
        let desc: ObjectDescriptor = toml::from_str(
            r#"
id = "tagged_tree"
kind = "tree"
[geometry]
footprint = [4.0, 4.0]
height = 8.0
[environment]
allowed_biomes = ["grassland"]
[[relations.avoid_tag]]
tag = "flammable"
distance = 0.0
[placement]
phase = "vegetation"
"#,
        )
        .unwrap();
        let (hard, _) = compile(&desc);
        let context = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| None,
            slope_at: &|_, _| 0.0,
            slope_at_footprint: None,
            bounds: (0, 0, 100, 100),
            grounding_tolerance: 0.5,
            biome_at: Some(&|x, _| {
                if x > 50 {
                    super::super::schema::Biome::Desert
                } else {
                    super::super::schema::Biome::Grassland
                }
            }),
            hazard_at: None,
        };
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        assert!(matches!(
            check_hard(&desc, &fp, &context, &hard, &[]),
            Err(RejectReason::ForbiddenBiome { .. })
        ));
        let safe_context = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| None,
            slope_at: &|_, _| 0.0,
            slope_at_footprint: None,
            bounds: (0, 0, 100, 100),
            grounding_tolerance: 0.5,
            biome_at: Some(&|_, _| super::super::schema::Biome::Grassland),
            hazard_at: None,
        };
        let tagged = PlacedKind {
            id: Some("tree-2".into()),
            kind: ItemKind::Tree,
            cx: 50.0,
            cz: 50.0,
            half_w: 2.0,
            half_d: 2.0,
            tags: vec!["flammable".into()],
        };
        assert!(matches!(
            check_hard(&desc, &fp, &safe_context, &hard, &[tagged]),
            Err(RejectReason::GeometryCollision { conflict_id: Some(id), .. }) if id == "tree-2"
        ));
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
            tags: vec![],
        }];
        let fp = Footprint::from_descriptor(&desc, 50.0, 50.0);
        assert!(check_hard(&desc, &fp, &c, &hard, &placed).is_ok());
    }
}
