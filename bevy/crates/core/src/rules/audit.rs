//! Audit mode: run the rules engine over already-generated entities without
//! changing their placement. Produces a `ValidationReport` so the generator
//! can surface violations without yet switching to rule-driven placement.
//!
//! This bridges `worldgen` (which owns terrain/water/slope queries) and the
//! rules engine (which owns constraints): `worldgen` supplies environment
//! closures through `PlacementContext`, and each existing entity is treated
//! as a candidate and checked against its descriptor's hard constraints.

use super::constraint::compile;
use super::errors::{RejectRecord, ValidationReport};
use super::loader::ObjectRegistry;
use super::schema::ItemKind;
use super::validator::{check_hard, Footprint, PlacementContext, PlacedKind};
use crate::worldgen::EntityInstance;

/// Convert an existing entity into the rules engine's placed-item shape.
/// Uses the entity's own kind string → ItemKind (best effort; unknown kinds
/// become `Other` and are skipped by audits that only care about real items).
pub fn entity_to_placed(e: &EntityInstance) -> PlacedKind {
    PlacedKind {
        kind: kind_from_str(&e.kind),
        cx: e.world_x as f32,
        cz: e.world_z as f32,
        half_w: e.width_m * 0.5 + e.height_m * 0.0, // footprint half-width
        half_d: e.depth_m * 0.5,
    }
}

/// Map an entity kind string to the rules `ItemKind`.
pub fn kind_from_str(kind: &str) -> super::schema::ItemKind {
    use super::schema::ItemKind;
    match kind {
        "road" => ItemKind::Road,
        "railway" => ItemKind::Railway,
        "building" | "building_tower" | "building_cluster" | "urban_building"
        | "residential_block" | "residential_tower" | "residential_home" | "resort_lodge"
        | "commercial_center" | "entertainment_center" | "school" | "town_hall" | "market"
        | "industrial" | "temple" | "church" | "parking_lot" | "green_space" => {
            ItemKind::Building
        }
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
/// `ValidationReport`. `descriptors` may be empty → nothing is checked and the
/// report is all zeros (the caller can decide to skip).
pub fn audit_entities(
    entities: &[EntityInstance],
    descriptors: &ObjectRegistry,
    ctx: &PlacementContext<'_>,
    seed: u64,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.seed = seed;

    // Build the placed list once (all entities are "already placed").
    let placed: Vec<PlacedKind> = entities.iter().map(entity_to_placed).collect();

    // Index placed entities by kind for relation queries.
    let all_placed = placed.clone();

    for e in entities {
        // Find a descriptor for this kind.
        let Some(desc) = descriptor_for(descriptors, e) else {
            continue;
        };
        let (hard, _soft) = compile(desc);

        // Treat the entity's centre as a candidate; check its real footprint.
        let fp = Footprint {
            cx: e.world_x as f32,
            cz: e.world_z as f32,
            half_w: e.width_m * 0.5 + desc.geometry.clearance,
            half_d: e.depth_m * 0.5 + desc.geometry.clearance,
        };

        // Exclude the entity itself from collision checks.
        let others: Vec<PlacedKind> = all_placed
            .iter()
            .filter(|p| {
                !(p.cx == fp.cx && p.cz == fp.cz && p.half_w == fp.half_w && p.half_d == fp.half_d)
            })
            .cloned()
            .collect();

        if let Err(reason) = check_hard(desc, &fp, ctx, &hard, &others) {
            report.count_reason(&reason);
            report.rejects.push(RejectRecord {
                item_id: e.entity_id.clone(),
                candidate_x: e.world_x,
                candidate_z: e.world_z,
                reason: reason.to_string(),
                conflict_with: None,
                rule: format!("{}.object.toml", desc.id),
            });
            continue;
        }
        // Count by category for the summary (only kinds with a descriptor).
        match desc.kind {
            ItemKind::Building | ItemKind::Storefront => report.buildings += 1,
            ItemKind::Road => report.roads += 1,
            _ => {}
        }
    }

    report
}

/// Find the descriptor whose kind matches the entity's kind. Matching is by
/// the **exact kind string** (TOML `kind = "building"` matches entity
/// `kind == "building"`). Entities whose kind string has no descriptor (e.g.
/// `commercial_center`) are skipped rather than force-fitted onto another
/// building descriptor — a mis-fitting rule would report false violations.
fn descriptor_for<'a>(
    registry: &'a ObjectRegistry,
    e: &EntityInstance,
) -> Option<&'a super::schema::ObjectDescriptor> {
    registry.descriptors.values().find(|d| {
        // Match the descriptor's kind against the entity kind string via the
        // same string table, but only when the descriptor id == the kind string
        // (i.e. the descriptor was written FOR this exact kind).
        descriptor_kind_str(d) == Some(e.kind.as_str())
    })
}

/// The canonical kind string a descriptor declares (its `kind` field, mapped
/// back to the string form used in entity kinds).
fn descriptor_kind_str(d: &super::schema::ObjectDescriptor) -> Option<&'static str> {
    Some(match d.kind {
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
        ItemKind::Other => return None,
    })
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
            bounds,
            grounding_tolerance: 0.5,
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
