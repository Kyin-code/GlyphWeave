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
use crate::worldgen::{EntityInstance, spatial_half_extents};

/// Convert an existing entity into the rules engine's placed-item shape.
/// Uses the entity's own kind string 鈫?ItemKind (best effort; unknown kinds
/// become `Other` and are skipped by audits that only care about real items).
pub fn entity_to_placed(e: &EntityInstance) -> PlacedKind {
    entity_to_placed_with_descriptor(e, None)
}

pub(crate) fn entity_to_placed_with_descriptor(
    e: &EntityInstance,
    descriptor: Option<&super::schema::ObjectDescriptor>,
) -> PlacedKind {
    let clearance = descriptor.map(|d| d.geometry.clearance).unwrap_or(0.0);
    let (half_w, half_d) = spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
    PlacedKind {
        clearance,
        id: if e.entity_id.is_empty() {
            None
        } else {
            Some(e.entity_id.clone())
        },
        kind: kind_from_str(&e.kind),
        cx: e.world_x as f32,
        cz: e.world_z as f32,
        half_w: half_w as f32 + clearance,
        half_d: half_d as f32 + clearance,
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
        "road" | "road-ew" | "road-ns" => ItemKind::Road,
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
        "mountain" | "rock_mountain" => ItemKind::Mountain,
        "hill" => ItemKind::Hill,
        "shopping_mall" => ItemKind::ShoppingMall,
        "office_tower" => ItemKind::OfficeTower,
        "convenience_store" => ItemKind::ConvenienceStore,
        "fence" => ItemKind::Fence,
        "guardrail" => ItemKind::Guardrail,
        "boardwalk" => ItemKind::Boardwalk,
        "scenic_belt" => ItemKind::ScenicBelt,
        "mudflat" => ItemKind::Mudflat,
        "night_market" => ItemKind::NightMarket,
        "community_plaza" => ItemKind::CommunityPlaza,
        "mangrove" => ItemKind::Mangrove,
        "tree" => ItemKind::Tree,
        "rock" => ItemKind::Rock,
        "water" | "lake" | "river" | "canal" => ItemKind::Water,
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
        let (half_w, half_d) = spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
        let fp = Footprint {
            rotation_y_deg: e.rotation_y_deg,
            cx: e.world_x as f32,
            cz: e.world_z as f32,
            half_w: half_w as f32 + desc.geometry.clearance,
            half_d: half_d as f32 + desc.geometry.clearance,
        };

        // Exclude the entity itself by stable id / index (not float equality).
        // Authored landforms own their complete footprint; procedural detail
        // inside a terrain mass must not make that fixed landmark fail audit.
        let authored_landform = e.entity_id.starts_with("landmark.")
            && matches!(desc.kind, ItemKind::Mountain | ItemKind::Hill);
        let others: Vec<PlacedKind> = all_placed
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                *i != idx
                    && !(p.id.is_some() && p.id.as_deref() == Some(e.entity_id.as_str()))
                    && !authored_landform
            })
            .map(|(_, p)| p.clone())
            .collect();

        let ground_fp = Footprint {
            rotation_y_deg: 0.,
            cx: fp.cx,
            cz: fp.cz,
            half_w: half_w as f32,
            half_d: half_d as f32,
        };
        // Grounding uses every integer raster sample, not nine probes: an
        // interior peak can pierce a building without touching its corners.
        let (min_x, min_z, max_x, max_z) = ctx.bounds;
        let x0 = (ground_fp.cx - ground_fp.half_w).floor().max(min_x as f32) as i32;
        let x1 = (ground_fp.cx + ground_fp.half_w).ceil().min(max_x as f32) as i32;
        let z0 = (ground_fp.cz - ground_fp.half_d).floor().max(min_z as f32) as i32;
        let z1 = (ground_fp.cz + ground_fp.half_d).ceil().min(max_z as f32) as i32;
        let footprint_ground = (z0..=z1)
            .flat_map(|z| (x0..=x1).map(move |x| (x, z)))
            .map(|(x, z)| ctx.height(x, z))
            .fold(f32::NEG_INFINITY, f32::max);
        // A filled/raised pad may leave the entity slightly above the
        // surrounding raster. The invalid case is only the inverse: the
        // baked ground rises through the object bottom.
        let ground_delta = footprint_ground - e.ground_height_m();
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
pub(crate) fn descriptor_for<'a>(
    registry: &'a ObjectRegistry,
    e: &EntityInstance,
) -> Option<&'a super::schema::ObjectDescriptor> {
    // Identity is not a rule override. A road-ns ID still has kind `road`
    // and must not inherit a more permissive sidewalk/transport descriptor.
    registry
        .descriptors
        .values()
        .find(|d| d.matches_kind(&e.kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::GroundingSpec;

    #[test]
    fn audit_footprint_uses_entity_rotation() {
        let entity = EntityInstance {
            entity_id: "rotated".into(),
            asset_id: "prop.building".into(),
            kind: "building".into(),
            world_x: 100,
            world_z: 200,
            world_y: 8,
            scale: 1.0,
            width_m: 80.0,
            depth_m: 20.0,
            height_m: 10.0,
            rotation_y_deg: 90.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        let placed = entity_to_placed(&entity);
        assert_eq!(placed.half_w, 10.0);
        assert_eq!(placed.half_d, 40.0);
    }

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
            water_intersects: None,
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        }
    }

    #[test]
    fn interior_baked_peak_is_not_hidden_between_nine_probes() {
        let mut entity: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId":"interior-peak", "assetId":"prop.building", "kind":"building",
            "worldX":100, "worldZ":200, "worldY":0,
            "scale":1.0,"widthM":20.0,"depthM":20.0,"heightM":10.0
        }))
        .unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects");
        let registry = ObjectRegistry::load_dir(&dir).unwrap();
        let height = |x, z| if x == 103 && z == 203 { 5.0 } else { 0.0 };
        let mut ctx = ctx_flat((0, 0, 512, 512));
        ctx.height_at = &height;
        let report = audit_entities(&[entity.clone()], &registry, &ctx, 42);
        assert_eq!(report.floating_items, 1);
        assert_eq!(report.rejected_items, 1);
        entity.world_y = 5;
        assert_eq!(
            audit_entities(&[entity], &registry, &ctx, 42).rejected_items,
            0
        );
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

#[cfg(test)]
mod descriptor_identity_tests {
    use super::*;

    #[test]
    fn road_like_id_cannot_select_a_more_permissive_descriptor() {
        let registry = ObjectRegistry::load_dir(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects"),
        )
        .unwrap();
        let road: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId":"generated.landuse-road-ns.1", "assetId":"prop.road", "kind":"road",
            "worldX":200,"worldZ":200,"worldY":0,"widthM":100,"depthM":8,"heightM":0.5,"scale":1
        }))
        .unwrap();
        assert_eq!(descriptor_for(&registry, &road).unwrap().id, "road");
        let mut spoof = road.clone();
        spoof.kind = "tree".into();
        assert_eq!(descriptor_for(&registry, &spoof).unwrap().id, "tree_common");
    }
}
