//! Compile an `ObjectDescriptor` into a flat list of `Constraint`s that the
//! validator can apply uniformly, plus soft scoring terms.

use super::schema::{AnchorSpec, ItemKind, ObjectDescriptor};

/// Hard / soft placement constraint (see plan §16.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    InsideBounds,
    OnGround,
    NotInWater,
    MaxSlope(f32),
    AvoidKind { kind: ItemKind, distance: f32 },
    AvoidTag { tag: String, distance: f32 },
    RequireNear { kind: ItemKind, distance: f32 },
    /// A public-access anchor that must keep a clear zone and face a target.
    ClearAnchor { anchor: String, side: String, radius: f32, must_face: Option<String> },
    /// Default geometry collision: an object must not overlap ANY other
    /// placed object (compiled for every descriptor, not just when an
    /// `avoid` rule is written). `avoid` adds extra distance margins.
    NoGeometryCollision,
    AllowedBiome(Vec<super::schema::Biome>),
    ForbiddenHazard(Vec<super::schema::HazardKind>),
    /// Soft scoring term (never rejects).
    PreferNear { kind: ItemKind, distance: f32, weight: f32 },
}

/// Compile a descriptor into (hard constraints, soft preferences).
pub fn compile(desc: &ObjectDescriptor) -> (Vec<Constraint>, Vec<Constraint>) {
    let mut hard = Vec::new();
    let mut soft = Vec::new();

    hard.push(Constraint::InsideBounds);
    // Default: never overlap any placed object (regardless of avoid rules).
    hard.push(Constraint::NoGeometryCollision);
    if desc.environment.on_ground {
        hard.push(Constraint::OnGround);
    }
    if desc.environment.not_in_water {
        hard.push(Constraint::NotInWater);
    }
    if desc.environment.max_slope > 0.0 {
        hard.push(Constraint::MaxSlope(desc.environment.max_slope));
    }
    if !desc.environment.allowed_biomes.is_empty() {
        hard.push(Constraint::AllowedBiome(desc.environment.allowed_biomes.clone()));
    }
    if !desc.environment.forbidden_hazards.is_empty() {
        hard.push(Constraint::ForbiddenHazard(desc.environment.forbidden_hazards.clone()));
    }
    for r in &desc.relations.avoid {
        hard.push(Constraint::AvoidKind {
            kind: r.kind,
            distance: r.distance,
        });
    }
    for r in &desc.relations.require {
        hard.push(Constraint::RequireNear {
            kind: r.kind,
            distance: r.distance,
        });
    }
    for a in &desc.anchors {
        if a.clear_radius > 0.0 {
            hard.push(Constraint::ClearAnchor {
                anchor: a.id.clone(),
                side: a.side.clone(),
                radius: a.clear_radius,
                must_face: a.must_face.clone(),
            });
        }
    }
    for p in &desc.relations.prefer {
        soft.push(Constraint::PreferNear {
            kind: p.kind,
            distance: p.distance,
            weight: p.weight,
        });
    }
    (hard, soft)
}

/// True if the descriptor needs a front anchor facing a specific kind.
pub fn front_anchor(desc: &ObjectDescriptor) -> Option<&AnchorSpec> {
    desc.anchors
        .iter()
        .find(|a| a.kind == super::schema::AnchorKind::PublicAccess && a.must_face.is_some())
}

/// Debug: print compiled constraints for a descriptor.
pub fn summarize(desc: &ObjectDescriptor) -> String {
    let (hard, soft) = compile(desc);
    format!(
        "{} ({}): hard={} soft={}",
        desc.id,
        desc.kind as u8,
        hard.len(),
        soft.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
kind = "storefront"
distance = 3.0

[[relations.prefer]]
kind = "park"
distance = 30.0
weight = 0.5

[[anchors]]
id = "front"
side = "front"
kind = "public_access"
clear_radius = 2.0

[placement]
phase = "vegetation"
"#,
        )
        .unwrap()
    }

    #[test]
    fn compile_tree_hard_and_soft() {
        let d = tree_desc();
        let (hard, soft) = compile(&d);
        assert!(hard.contains(&Constraint::InsideBounds));
        assert!(hard.contains(&Constraint::OnGround));
        assert!(hard.contains(&Constraint::NotInWater));
        assert!(hard.contains(&Constraint::MaxSlope(30.0)));
        assert!(hard.contains(&Constraint::AvoidKind {
            kind: ItemKind::Storefront,
            distance: 3.0,
        }));
        assert!(hard.contains(&Constraint::ClearAnchor {
            anchor: "front".into(),
            side: "front".into(),
            radius: 2.0,
            must_face: None,
        }));
        assert!(soft.contains(&Constraint::PreferNear {
            kind: ItemKind::Park,
            distance: 30.0,
            weight: 0.5,
        }));
    }
}
