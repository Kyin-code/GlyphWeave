//! Audit mode: run the rules engine over already-generated entities without
//! changing their placement. Produces a `ValidationReport` so the generator
//! can surface violations without yet switching to rule-driven placement.
//!
//! This bridges `worldgen` (which owns terrain/water/slope queries) and the
//! rules engine (which owns constraints): `worldgen` supplies environment
//! closures through `PlacementContext`, and each existing entity is treated
//! as a candidate and checked against its descriptor's hard constraints.

use super::constraint::compile;
use super::errors::{RejectReason, RejectRecord, ValidationReport};
use super::loader::ObjectRegistry;
use super::schema::ItemKind;
use super::validator::{Footprint, PlacedKind, PlacementContext, check_hard};
use crate::worldgen::EntityInstance;

/// Convert an existing entity into the rules engine's placed-item shape.
/// Uses the entity's own kind string 鈫?ItemKind (best effort; unknown kinds
/// become `Other` and are skipped by audits that only care about real items).
pub fn entity_to_placed(e: &EntityInstance) -> PlacedKind {
    entity_to_placed_with_descriptor(e, None)
}

fn entity_to_placed_with_descriptor(
    e: &EntityInstance,
    descriptor: Option<&super::schema::ObjectDescriptor>,
) -> PlacedKind {
    let clearance = descriptor.map(|d| d.geometry.clearance).unwrap_or(0.0);
    PlacedKind {
        id: if e.entity_id.is_empty() {
            None
        } else {
            Some(e.entity_id.clone())
        },
        kind: kind_from_str(&e.kind),
        cx: e.world_x as f32,
        cz: e.world_z as f32,
        half_w: e.width_m * 0.5 + clearance,
        half_d: e.depth_m * 0.5 + clearance,
        tags: descriptor.map(|d| d.tags.clone()).unwrap_or_default(),
    }
}

fn conflict_id(reason: &super::errors::RejectReason) -> Option<String> {
    match reason {
        super::errors::RejectReason::GeometryCollision { conflict_id, .. } => conflict_id.clone(),
        _ => None,
    }
}

/// Map an entity kind string to the rules `ItemKind`.
pub fn kind_from_str(kind: &str) -> super::schema::ItemKind {
    use super::schema::ItemKind;
    match kind {
        "road" => ItemKind::Road,
        "railway" => ItemKind::Railway,
        "building"
        | "building_tower"
        | "building_cluster"
        | "urban_building"
        | "residential_block"
        | "residential_tower"
        | "residential_home"
        | "resort_lodge"
        | "commercial_center"
        | "entertainment_center"
        | "school"
        | "town_hall"
        | "market"
        | "industrial"
        | "temple"
        | "church"
        | "parking_lot"
        | "green_space" => ItemKind::Building,
        "storefront" => ItemKind::Storefront,
        "tree" => ItemKind::Tree,
        "rock" => ItemKind::Rock,
        "water" | "lake" | "river" => ItemKind::Water,
        "bridge" | "causeway" => ItemKind::Bridge,
        "park" => ItemKind::Park,
        "sidewalk" => ItemKind::Sidewalk,
        "lamp" => ItemKind::Lamp,
        "bench" => ItemKind::Bench,
        "bus_stop" => ItemKind::BusStop,
        "food_stall" => ItemKind::FoodStall,
        _ => ItemKind::Other,
    }
}

/// Run an audit over a full entity list. Each entity is checked against the
/// descriptor for its kind (if one exists); violations are aggregated into a
/// `ValidationReport`. `descriptors` may be empty 鈫?nothing is checked and the
/// report is all zeros (the caller can decide to skip).
pub fn audit_entities(
    entities: &[EntityInstance],
    descriptors: &ObjectRegistry,
    ctx: &PlacementContext<'_>,
    seed: u64,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.seed = seed;

    // Build the placed list once (all entities are "already placed"). Each
    // record inherits the matching descriptor's effective clearance and tags so
    // audit mode evaluates AvoidTag and collision margins the same way as placement.
    let placed: Vec<PlacedKind> = entities
        .iter()
        .map(|entity| entity_to_placed_with_descriptor(entity, descriptor_for(descriptors, entity)))
        .collect();

    let all_placed = placed.clone();

    for (idx, e) in entities.iter().enumerate() {
        // Find a descriptor for this entity's kind string.
        let Some(desc) = descriptor_for(descriptors, e) else {
            // No rule governs this kind 鈥?record it as "unruled" so the report
            // can show coverage gaps instead of silently skipping.
            if !e.kind.is_empty() {
                report.unruled_items.push(e.kind.clone());
            }
            continue;
        };
        let (hard, _soft) = compile(desc);
        report.checked_items += 1;
        // Count buildings/roads as the number of entities whose kind has a
        // matching descriptor (matched BEFORE pass/fail), so the report field
        // reflects "how many buildings were audited", not "how many passed".
        match desc.kind {
            ItemKind::Building | ItemKind::Storefront => report.buildings += 1,
            ItemKind::Road => report.roads += 1,
            _ => {}
        }

        // Treat the entity's centre as a candidate; check its real footprint.
        let fp = Footprint {
            cx: e.world_x as f32,
            cz: e.world_z as f32,
            half_w: e.width_m * 0.5 + desc.geometry.clearance,
            half_d: e.depth_m * 0.5 + desc.geometry.clearance,
        };

        // Exclude the entity itself by stable id / index (not float equality).
        let others: Vec<PlacedKind> = all_placed
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                *i != idx && !(p.id.is_some() && p.id.as_deref() == Some(e.entity_id.as_str()))
            })
            .map(|(_, p)| p.clone())
            .collect();

        let ground_delta = (ctx.height(e.world_x, e.world_z) - e.ground_height_m()).abs();
        if desc.environment.on_ground
            && ground_delta > ctx.grounding_tolerance.max(e.grounding.tolerance_m)
        {
            report.rejected_items += 1;
            report.count_reason(&RejectReason::NotGrounded);
            report.rejects.push(RejectRecord {
                item_id: e.entity_id.clone(),
                candidate_x: e.world_x,
                candidate_z: e.world_z,
                reason: RejectReason::NotGrounded.to_string(),
                conflict_with: None,
                rule: format!("{}.object.toml", desc.id),
            });
            continue;
        }
        if let Err(reason) = check_hard(desc, &fp, ctx, &hard, &others) {
            report.rejected_items += 1;
            report.count_reason(&reason);
            report.rejects.push(RejectRecord {
                item_id: e.entity_id.clone(),
                candidate_x: e.world_x,
                candidate_z: e.world_z,
                reason: reason.to_string(),
                conflict_with: conflict_id(&reason),
                rule: format!("{}.object.toml", desc.id),
            });
            continue;
        }
        report.passed_items += 1;
    }

    report
}

/// Find the descriptor whose kind matches the entity's kind. Matching is by
/// the **exact kind string** (TOML `kind = "building"` matches entity
/// `kind == "building"`). Entities whose kind string has no descriptor (e.g.
/// `commercial_center`) are skipped rather than force-fitted onto another
/// building descriptor 鈥?a mis-fitting rule would report false violations.
fn descriptor_for<'a>(
    registry: &'a ObjectRegistry,
    e: &EntityInstance,
) -> Option<&'a super::schema::ObjectDescriptor> {
    registry
        .descriptors
        .values()
        .find(|d| d.matches_kind(&e.kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::schema::ItemKind;

    fn ctx_flat(bounds: (i32, i32, i32, i32)) -> PlacementContext<'static> {
        let height = Box::leak(Box::new(|_: i32, _: i32| 0.0_f32));
        let water = Box::leak(Box::new(|_: i32, _: i32| None));
        let slope = Box::leak(Box::new(|_: i32, _: i32| 0.0_f32));
        PlacementContext {
            height_at: height,
            water_level: water,
            slope_at: slope,
            slope_at_footprint: None,
            bounds,
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        }
    }

    #[test]
    fn kind_mapping() {
        assert_eq!(kind_from_str("tree"), ItemKind::Tree);
        assert_eq!(kind_from_str("building"), ItemKind::Building);
        assert_eq!(kind_from_str("residential_home"), ItemKind::Building);
        assert_eq!(kind_from_str("storefront"), ItemKind::Storefront);
        assert_eq!(kind_from_str("mystery"), ItemKind::Other);
    }

    #[test]
    fn audit_empty_descriptors_is_clean() {
        let reg = ObjectRegistry::default();
        let ctx = ctx_flat((0, 0, 100, 100));
        let r = audit_entities(&[], &reg, &ctx, 42);
        assert_eq!(r.buildings, 0);
        assert!(r.rejects.is_empty());
        assert!(r.geometry_collisions == 0);
    }
}
