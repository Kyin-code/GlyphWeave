//! Transactional reconciliation after infrastructure pads, before raster bake.
//! Never relax a descriptor, resize/delete an entity, or move an authored
//! landmark to make a gate green. Generated features try bounded nearby sites;
//! the whole local snapshot is returned only after another global audit passes.
use super::*;
use crate::rules::audit::entity_to_placed_with_descriptor;
use crate::rules::{Footprint, ObjectRegistry, check_hard};

const MAX_ROUNDS: usize = 4;
const SEARCH_RADII_M: [i32; 5] = [2, 4, 8, 16, 32];

pub(super) fn reconcile(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    base: &[EntityInstance],
    water: WaterGeometry,
    rules_dir: &Path,
    terrain_context: &TerrainContext<'_>,
) -> WorldgenResult<(Vec<EntityInstance>, Vec<TerrainCarve>, serde_json::Value)> {
    let registry = ObjectRegistry::load_dir(rules_dir)
        .map_err(|e| WorldgenError::InvalidRules(e.to_string()))?;
    let mut entities = base.to_vec();
    let mut moves = Vec::new();
    let mut skips: Vec<serde_json::Value> = Vec::new();
    let mut original_positions = BTreeMap::new();
    for entity in base {
        original_positions.insert(entity.entity_id.clone(), (entity.world_x, entity.world_z));
    }
    for round in 0..=MAX_ROUNDS {
        let carves = plan_terrain_carves(&entities);
        let height = |x, z| terrain_context.height(x, z, &carves);
        let audit = audit_scene(
            seed,
            scene,
            landmarks,
            &entities,
            water,
            rules_dir,
            AuditOptions {
                height_at: Some(&height),
                slope_half: None,
            },
        )
        .map_err(|e| WorldgenError::InvalidRules(e.to_string()))?;
        if audit.rejected_items == 0 {
            let output_items = entities.len();
            return Ok((
                entities,
                carves,
                serde_json::json!({
                    "sceneId":scene.scene_id,"rounds":round,"moves":moves,
                    "inputItems":base.len(),"outputItems":output_items,"removedItems":skips.len(),
                    "skips":skips,
                    "checkedItems":audit.checked_items,"rejectedItems":0,
                }),
            ));
        }
        if round == MAX_ROUNDS {
            return Err(WorldgenError::InvalidRules(format!(
                "engineering reconciliation exhausted in {}: {}",
                scene.scene_id,
                serde_json::to_string(&audit.rejects)?
            )));
        }
        let pending_rejects = serde_json::to_string(&audit.rejects)?;
        let mut progress = false;
        for reject in audit.rejects {
            let Some(index) = entities.iter().position(|e| e.entity_id == reject.item_id) else {
                continue;
            };
            let source = entities[index].clone();
            // Infrastructure layout and user-authored entities are fixed. Moving
            // an entire road could silently break its graph / bridge connections.
            if source.grounding.frontage_road_point.is_some()
                || !source.entity_id.starts_with("generated.")
                || landmarks.iter().any(|l| l.entity_id == source.entity_id)
                || matches!(
                    source.kind.as_str(),
                    "road" | "bridge" | "railway" | "canal" | "causeway" | "sidewalk"
                )
            {
                // T-0013 (ii): a GENERATED frontage object whose own object descriptor
                // declares `fallback = skip` may be SKIPPED rather than failing the whole
                // bake. Its contract already says "if you cannot satisfy me, do not place
                // me"; before this the reconciliation treated every frontage-bearing
                // entity as fixed and returned `fixed entity ... fails engineered terrain`,
                // so one shop whose authored setback (12 m) cannot satisfy its own 8 m
                // road-distance contract made a whole world unbuildable (measured on
                // `morphology_compile`). Authored entities, landmarks, infrastructure
                // layout and objects that ask to move/shrink stay hard errors, and every
                // skip is NAMED and COUNTED - never silent.
                let skippable = source.entity_id.starts_with("generated.")
                    && !landmarks.iter().any(|l| l.entity_id == source.entity_id)
                    && !matches!(
                        source.kind.as_str(),
                        "road" | "bridge" | "railway" | "canal" | "causeway" | "sidewalk"
                    )
                    && crate::rules::audit::descriptor_for(&registry, &source).is_some_and(|d| {
                        d.placement.fallback == crate::rules::schema::Fallback::Skip
                    });
                if skippable {
                    entities.remove(index);
                    skips.push(serde_json::json!({
                        "entityId": source.entity_id,
                        "kind": source.kind,
                        "round": round + 1,
                        "reason": reject.reason,
                        "descriptorFallback": "skip",
                    }));
                    progress = true;
                    continue;
                }
                return Err(WorldgenError::InvalidRules(format!(
                    "fixed entity {} fails engineered terrain: {}",
                    source.entity_id, reject.reason
                )));
            }
            let Some(desc) = crate::rules::audit::descriptor_for(&registry, &source) else {
                continue;
            };
            let (hard, _) = crate::rules::constraint::compile(desc);
            let others: Vec<_> = entities
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, e)| {
                    let descriptor = crate::rules::audit::descriptor_for(&registry, e);
                    entity_to_placed_with_descriptor(e, descriptor)
                })
                .collect();
            let other_entities: Vec<_> = entities
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, e)| e.clone())
                .collect();
            let other_carves = plan_terrain_carves(&other_entities);
            let (origin_x, origin_z) = original_positions[&source.entity_id];
            let mut offsets = vec![(0, 0)];
            if matches!(
                desc.placement.fallback,
                crate::rules::schema::Fallback::Move
            ) {
                for radius in SEARCH_RADII_M {
                    for (dx, dz) in [
                        (0, -1),
                        (1, 0),
                        (0, 1),
                        (-1, 0),
                        (1, -1),
                        (1, 1),
                        (-1, 1),
                        (-1, -1),
                    ] {
                        offsets.push((dx * radius, dz * radius));
                    }
                }
            }
            for (dx, dz) in offsets {
                let mut candidate = source.clone();
                candidate.world_x = origin_x.saturating_add(dx);
                candidate.world_z = origin_z.saturating_add(dz);
                let (hw, hd) = spatial_half_extents(
                    candidate.width_m,
                    candidate.depth_m,
                    candidate.rotation_y_deg,
                );
                let fp = Footprint {
                    rotation_y_deg: candidate.rotation_y_deg,
                    cx: candidate.world_x as f32,
                    cz: candidate.world_z as f32,
                    half_w: hw as f32 + desc.geometry.clearance,
                    half_d: hd as f32 + desc.geometry.clearance,
                };
                if fp.cx - fp.half_w < scene.origin_x as f32
                    || fp.cz - fp.half_d < scene.origin_z as f32
                    || fp.cx + fp.half_w > scene.origin_x as f32 + scene.width_m as f32
                    || fp.cz + fp.half_d > scene.origin_z as f32 + scene.depth_m as f32
                {
                    continue;
                }
                // Do not carry the old site's elevation or support pad to the new
                // site. Sample its full footprint against other committed works.
                let mut ceiling = f32::NEG_INFINITY;
                for z in -(hd.ceil() as i32)..=hd.ceil() as i32 {
                    for x in -(hw.ceil() as i32)..=hw.ceil() as i32 {
                        ceiling = ceiling.max(terrain_context.height(
                            candidate.world_x + x,
                            candidate.world_z + z,
                            &other_carves,
                        ));
                    }
                }
                let offset = candidate.ground_height_m() - candidate.world_y as f32;
                candidate.world_y = (ceiling - offset).ceil() as i32;
                let mut trial_carves = other_carves.clone();
                trial_carves.extend(plan_terrain_carves(std::slice::from_ref(&candidate)));
                trial_carves.sort_by_key(|c| c.priority);
                let height = |x, z| terrain_context.height(x, z, &trial_carves);
                let valid = with_height_placement_context(seed, scene, water, &height, |ctx| {
                    check_hard(desc, &fp, ctx, &hard, &others).is_ok()
                });
                if !valid {
                    continue;
                }
                let delta_x = candidate.world_x - source.world_x;
                let delta_z = candidate.world_z - source.world_z;
                for anchor in &mut candidate.anchors {
                    anchor.world_x = anchor.world_x.saturating_add(delta_x);
                    anchor.world_z = anchor.world_z.saturating_add(delta_z);
                }
                candidate.bounds = Some(candidate.computed_bounds());
                if (source.world_x, source.world_z, source.world_y)
                    == (candidate.world_x, candidate.world_z, candidate.world_y)
                {
                    continue;
                }
                moves.push(serde_json::json!({"entityId":source.entity_id,"round":round+1,"reason":reject.reason,
                    "from":{"x":source.world_x,"z":source.world_z,"y":source.world_y},
                    "to":{"x":candidate.world_x,"z":candidate.world_z,"y":candidate.world_y}}));
                entities[index] = candidate;
                progress = true;
                break;
            }
        }
        if !progress {
            return Err(WorldgenError::InvalidRules(format!(
                "engineering placement stalled in {} at round {}; unresolved={}",
                scene.scene_id, round, pending_rejects
            )));
        }
    }
    unreachable!("bounded engineering loop returns success or failure")
}
