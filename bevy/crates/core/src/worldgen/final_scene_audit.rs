//! Final hard-geometry audit over the entities that will actually be published.
//!
//! Planning-stage checks are not sufficient: archetype sizes, road widening,
//! market placement and planning-history centres can all change the final
//! footprint. This audit consumes the published oriented rectangles and the
//! baked heightfield, then fails closed on floating buildings, terrain piercing
//! a base, building/building overlaps, building/main-road overlaps, missing
//! terrain support and invalid road profiles.

use serde::{Deserialize, Serialize};

use crate::baked_terrain::BakedTerrain;

use super::{
    is_building_kind, oriented_footprints_overlap, EntityInstance, OrientedFootprint, SceneSpec,
    WaterGeometry,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub entity_id: String,
    pub kind: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_with: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub scene_id: String,
    pub checked_buildings: usize,
    pub checked_roads: usize,
    pub checked_footprints: usize,
    pub building_building_collisions: usize,
    pub building_road_collisions: usize,
    pub unsupported_footprints: usize,
    pub submerged_buildings: usize,
    pub floating_buildings: usize,
    pub pierced_buildings: usize,
    pub invalid_road_profiles: usize,
    pub failures: Vec<Failure>,
}

pub fn audit(
    entities: &[EntityInstance],
    terrain: &BakedTerrain,
    scene: &SceneSpec,
    water: WaterGeometry,
) -> Report {
    let mut failures = Vec::new();
    let mut checked_buildings = 0;
    let mut checked_roads = 0;
    let mut checked_footprints = 0;
    let mut building_building_collisions = 0;
    let mut building_road_collisions = 0;
    let mut unsupported_footprints = 0;
    let mut submerged_buildings = 0;
    let mut floating_buildings = 0;
    let mut pierced_buildings = 0;
    let mut invalid_road_profiles = 0;

    let buildings = entities
        .iter()
        .filter(|entity| is_hard_building(entity.kind.as_str()))
        .map(|entity| (entity, OrientedFootprint::from_entity(entity)))
        .collect::<Vec<_>>();
    let roads = entities
        .iter()
        .filter(|entity| is_main_road(entity))
        .map(|entity| (entity, OrientedFootprint::from_entity(entity)))
        .collect::<Vec<_>>();

    for (entity, footprint) in &buildings {
        checked_buildings += 1;
        let samples = footprint.grid_cells();
        checked_footprints += samples.len();
        let mut heights = Vec::with_capacity(samples.len());
        let mut unsupported = samples.is_empty();
        for &(x, z) in &samples {
            if x < scene.origin_x
                || z < scene.origin_z
                || x >= scene.origin_x + scene.width_m as i32
                || z >= scene.origin_z + scene.depth_m as i32
            {
                unsupported = true;
                continue;
            }
            match terrain.height(x, z) {
                Some(height) if height.is_finite() => heights.push(height),
                _ => unsupported = true,
            }
        }
        if unsupported || heights.is_empty() {
            unsupported_footprints += 1;
            failures.push(Failure {
                entity_id: entity.entity_id.clone(),
                kind: entity.kind.clone(),
                reason: "building footprint has no complete baked terrain support".into(),
                conflict_with: None,
            });
            continue;
        }
        let submerged_cells = samples
            .iter()
            .filter(|(x, z)| water.contains_point(f64::from(*x), f64::from(*z)))
            .count();
        if submerged_cells > 0 {
            submerged_buildings += 1;
            failures.push(Failure {
                entity_id: entity.entity_id.clone(),
                kind: entity.kind.clone(),
                reason: format!("building footprint covers {submerged_cells} water cell(s)"),
                conflict_with: None,
            });
            continue;
        }
        let highest = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let base = entity.ground_height_m();
        let tolerance = entity.grounding.tolerance_m.max(0.5);
        if base - highest > tolerance {
            floating_buildings += 1;
            failures.push(Failure {
                entity_id: entity.entity_id.clone(),
                kind: entity.kind.clone(),
                reason: format!(
                    "building base {base:.3}m floats {:.3}m above support {highest:.3}m",
                    base - highest
                ),
                conflict_with: None,
            });
            continue;
        }
        if highest - base > tolerance {
            pierced_buildings += 1;
            failures.push(Failure {
                entity_id: entity.entity_id.clone(),
                kind: entity.kind.clone(),
                reason: format!(
                    "terrain {highest:.3}m rises {:.3}m through building base {base:.3}m",
                    highest - base
                ),
                conflict_with: None,
            });
        }
    }

    for (entity, footprint) in &roads {
        checked_roads += 1;
        let samples = footprint.grid_cells();
        checked_footprints += samples.len();
        let complete = !samples.is_empty()
            && samples
                .iter()
                .all(|(x, z)| terrain.height(*x, *z).is_some_and(f32::is_finite));
        if !complete {
            unsupported_footprints += 1;
            failures.push(Failure {
                entity_id: entity.entity_id.clone(),
                kind: entity.kind.clone(),
                reason: "road footprint has no complete baked terrain support".into(),
                conflict_with: None,
            });
        }
        if let Some(profile) = &entity.grounding.road_profile {
            if let Err(error) = profile.validate_entity(entity) {
                invalid_road_profiles += 1;
                failures.push(Failure {
                    entity_id: entity.entity_id.clone(),
                    kind: entity.kind.clone(),
                    reason: format!("invalid road profile: {error}"),
                    conflict_with: None,
                });
            }
        }
    }

    for (left_index, (left, left_footprint)) in buildings.iter().enumerate() {
        for (right, right_footprint) in buildings.iter().skip(left_index + 1) {
            if oriented_footprints_overlap(left_footprint, right_footprint, 0.05) {
                building_building_collisions += 1;
                failures.push(Failure {
                    entity_id: left.entity_id.clone(),
                    kind: left.kind.clone(),
                    reason: format!(
                        "building footprint overlaps {} with no legal separation",
                        right.entity_id
                    ),
                    conflict_with: Some(right.entity_id.clone()),
                });
            }
        }
        for (road, road_footprint) in &roads {
            if oriented_footprints_overlap(left_footprint, road_footprint, 0.25) {
                building_road_collisions += 1;
                failures.push(Failure {
                    entity_id: left.entity_id.clone(),
                    kind: left.kind.clone(),
                    reason: format!("building footprint intersects main road {}", road.entity_id),
                    conflict_with: Some(road.entity_id.clone()),
                });
            }
        }
    }

    Report {
        format: "glyphweave.final-scene-audit".into(),
        version: 1,
        status: if failures.is_empty() { "pass" } else { "fail" }.into(),
        scene_id: scene.scene_id.clone(),
        checked_buildings,
        checked_roads,
        checked_footprints,
        building_building_collisions,
        building_road_collisions,
        unsupported_footprints,
        submerged_buildings,
        floating_buildings,
        pierced_buildings,
        invalid_road_profiles,
        failures,
    }
}

fn is_hard_building(kind: &str) -> bool {
    is_building_kind(kind)
        && !matches!(
            kind,
            "park" | "green_space" | "parking_lot" | "community_plaza"
        )
}

fn is_main_road(entity: &EntityInstance) -> bool {
    entity.kind == "road"
        && !entity
            .entity_id
            .starts_with("generated.settlement-evolution.approach.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{legacy_water_geometry, WaterKind};

    fn flat_terrain(scene: &SceneSpec) -> BakedTerrain {
        let bytes = crate::height_payload::encode_millimetres(
            scene.width_m,
            scene.depth_m,
            std::iter::repeat_n(10.0, (scene.width_m * scene.depth_m) as usize),
        )
        .expect("height payload");
        let mut terrain = BakedTerrain::new(scene).expect("terrain");
        terrain
            .insert_chunk(0, 0, scene.width_m, scene.depth_m, &bytes)
            .expect("chunk");
        terrain
    }

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "final-audit".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn building(id: &str, x: i32, z: i32, rotation_y_deg: f32) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: "building.house".into(),
            kind: "residential_home".into(),
            world_x: x,
            world_z: z,
            world_y: 10,
            scale: 1.0,
            width_m: 20.0,
            depth_m: 12.0,
            height_m: 8.0,
            rotation_y_deg,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        }
    }

    #[test]
    fn oriented_overlap_is_not_hidden_by_axis_aligned_centres() {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        let report = audit(
            &[
                building("left", 50, 50, 45.0),
                building("right", 60, 50, 45.0),
            ],
            &terrain,
            &scene,
            legacy_water_geometry(WaterKind::None, 128, 128, 0.0),
        );
        assert_eq!(report.status, "fail");
        assert_eq!(report.building_building_collisions, 1);
    }

    #[test]
    fn floating_and_pierced_buildings_fail_with_named_reasons() {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        let mut floating = building("floating", 30, 30, 0.0);
        floating.world_y = 20;
        let mut pierced = building("pierced", 80, 80, 0.0);
        pierced.world_y = 5;
        let report = audit(
            &[floating, pierced],
            &terrain,
            &scene,
            legacy_water_geometry(WaterKind::None, 128, 128, 0.0),
        );
        assert_eq!(report.status, "fail");
        assert_eq!(report.floating_buildings, 1);
        assert_eq!(report.pierced_buildings, 1);
    }

    #[test]
    fn water_under_a_building_fails_even_when_terrain_is_complete() {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        let report = audit(
            &[building("in-water", 64, 50, 0.0)],
            &terrain,
            &scene,
            legacy_water_geometry(WaterKind::River, 128, 128, 4.0),
        );
        assert_eq!(report.status, "fail");
        assert_eq!(report.submerged_buildings, 1);
    }
}
