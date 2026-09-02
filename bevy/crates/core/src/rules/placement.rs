//! Unified placement pipeline: generate deterministic candidates 鈫?hard
//! constraint check 鈫?soft score 鈫?commit (mark occupancy + register placed).

use super::constraint::compile;
use super::errors::{PlacementOutcome, RejectReason, RejectRecord};
use super::loader::ObjectRegistry;
use super::schema::ObjectDescriptor;
use super::validator::{Footprint, PlacedKind, PlacementContext, check_hard, score_soft};
use crate::worldgen::EntityInstance;

/// A candidate position for one object.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    pub x: i32,
    pub z: i32,
}

/// Deterministic candidate list for a footprint centred at (x, z), derived
/// from a seed so the same input always yields the same order.
pub fn generate_candidates(x: i32, z: i32, attempts: u32, seed: u64) -> Vec<Candidate> {
    let mut out = Vec::with_capacity(attempts as usize);
    // Centre first (most likely valid), then a deterministic spiral of offsets.
    out.push(Candidate { x, z });
    let mut ring = 1_i32;
    while (out.len() as u32) < attempts {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs() != ring && dy.abs() != ring {
                    continue;
                }
                if out.len() as u32 >= attempts {
                    break;
                }
                // Jitter by seed for determinism.
                let jx = ((x.wrapping_add(dx * 7) as u64).wrapping_mul(seed) % 5) as i32 - 2;
                let jz = ((z.wrapping_add(dy * 13) as u64).wrapping_mul(seed) % 5) as i32 - 2;
                out.push(Candidate {
                    x: x + dx * 2 + jx,
                    z: z + dy * 2 + jz,
                });
            }
        }
        ring += 1;
    }
    out.truncate(attempts as usize);
    out
}

fn reject_record(
    desc: &ObjectDescriptor,
    candidate: Candidate,
    reason: RejectReason,
) -> RejectRecord {
    let conflict_with = match &reason {
        RejectReason::GeometryCollision { conflict_id, .. } => conflict_id.clone(),
        _ => None,
    };
    RejectRecord {
        item_id: desc.id.clone(),
        candidate_x: candidate.x,
        candidate_z: candidate.z,
        reason: reason.to_string(),
        conflict_with,
        rule: "hard".into(),
    }
}

/// Place one object: try candidates in order, first valid (hard constraints)
/// with the highest soft score wins. Returns placed entity or reject records.
pub fn place_one(
    desc: &ObjectDescriptor,
    centre: Candidate,
    ctx: &PlacementContext<'_>,
    placed: &mut Vec<PlacedKind>,
    seed: u64,
) -> PlacementOutcome {
    let mut outcome = PlacementOutcome::default();
    let (hard, soft) = compile(desc);
    let candidates = generate_candidates(centre.x, centre.z, desc.placement.attempts, seed);

    // A candidate is (position, rotated bool). Rotation only matters when
    // allow_rotate and the footprint is non-square.
    let mut best: Option<(f32, Candidate, bool)> = None;
    let mut rejected_candidates: Vec<RejectRecord> = Vec::new();
    let rotations: &[bool] = if desc.placement.allow_rotate
        && (desc.geometry.footprint[0] - desc.geometry.footprint[1]).abs() > 0.1
    {
        &[false, true]
    } else {
        &[false]
    };

    for cand in candidates {
        for &rot in rotations {
            let mut fp = Footprint::from_descriptor(desc, cand.x as f32, cand.z as f32);
            if rot {
                fp = fp.rotated();
            }
            match check_hard(desc, &fp, ctx, &hard, placed) {
                Ok(()) => {
                    let s = score_soft(&fp, &soft, placed);
                    if best.as_ref().map(|(bs, _, _)| s > *bs).unwrap_or(true) {
                        best = Some((s, cand, rot));
                    }
                }
                Err(reason) => rejected_candidates.push(reject_record(desc, cand, reason)),
            }
        }
    }

    // fallback=move: if no valid candidate, widen the search ring once more.
    let mut best = best;
    if best.is_none() && desc.placement.fallback == super::schema::Fallback::Move {
        let wider = generate_candidates(
            centre.x,
            centre.z,
            desc.placement.attempts.max(20),
            seed.wrapping_add(0xABCDEF),
        );
        for cand in wider {
            for &rot in rotations {
                let mut fp = Footprint::from_descriptor(desc, cand.x as f32, cand.z as f32);
                if rot {
                    fp = fp.rotated();
                }
                match check_hard(desc, &fp, ctx, &hard, placed) {
                    Ok(()) => {
                        let score = score_soft(&fp, &soft, placed);
                        if best
                            .as_ref()
                            .map(|(best_score, _, _)| score > *best_score)
                            .unwrap_or(true)
                        {
                            best = Some((score, cand, rot));
                        }
                        break;
                    }
                    Err(reason) => rejected_candidates.push(reject_record(desc, cand, reason)),
                }
            }
            if best.is_some() {
                break;
            }
        }
    }

    if let Some((_, cand, rot)) = best {
        // Commit: register into placed list (occupancy is tracked by caller
        // via PlacedKind). Build the EntityInstance for the map.
        let mut fp = Footprint::from_descriptor(desc, cand.x as f32, cand.z as f32);
        if rot {
            fp = fp.rotated();
        }
        // Build the entity BEFORE moving `entity` into outcome.placed.
        let entity_id = format!("{}_{}", desc.id, cand.x.abs() * 1000 + cand.z.abs());
        let mut entity = EntityInstance {
            entity_id,
            asset_id: if desc.asset.is_empty() {
                desc.id.clone()
            } else {
                desc.asset.clone()
            },
            kind: kind_to_str(desc.kind),
            world_x: cand.x,
            world_z: cand.z,
            world_y: ctx.height(cand.x, cand.z).round() as i32,
            scale: 1.0,
            // A 90掳 rotation swaps width/depth in the footprint.
            width_m: if rot {
                desc.geometry.footprint[1]
            } else {
                desc.geometry.footprint[0]
            },
            depth_m: if rot {
                desc.geometry.footprint[0]
            } else {
                desc.geometry.footprint[1]
            },
            height_m: desc.geometry.height,
        };
        // Surface-anchored kinds (not road) sit on the ground.
        if !desc.kind.is_hard() {
            entity.world_y = ctx.height(cand.x, cand.z).round() as i32;
        }
        let placed_id = entity.entity_id.clone();
        outcome.placed.push(entity);
        placed.push(PlacedKind {
            id: Some(placed_id),
            kind: desc.kind,
            cx: cand.x as f32,
            cz: cand.z as f32,
            half_w: fp.half_w,
            half_d: fp.half_d,
            tags: desc.tags.clone(),
        });
    } else {
        outcome.rejected.extend(rejected_candidates);
    }

    outcome
}

/// Map ItemKind 鈫?the string kind used by EntityInstance / scene entities.
pub fn kind_to_str(kind: super::schema::ItemKind) -> String {
    match kind {
        super::schema::ItemKind::Road => "road".into(),
        super::schema::ItemKind::Railway => "railway".into(),
        super::schema::ItemKind::Building => "building".into(),
        super::schema::ItemKind::Storefront => "storefront".into(),
        super::schema::ItemKind::Tree => "tree".into(),
        super::schema::ItemKind::Rock => "rock".into(),
        super::schema::ItemKind::Water => "water".into(),
        super::schema::ItemKind::Bridge => "bridge".into(),
        super::schema::ItemKind::Park => "park".into(),
        super::schema::ItemKind::Sidewalk => "sidewalk".into(),
        super::schema::ItemKind::Lamp => "lamp".into(),
        super::schema::ItemKind::Bench => "bench".into(),
        super::schema::ItemKind::BusStop => "bus_stop".into(),
        super::schema::ItemKind::FoodStall => "food_stall".into(),
        super::schema::ItemKind::Other => "other".into(),
    }
}

/// A batch placement request: place `count` instances of the object whose
/// descriptor id is `descriptor_id`, centred near (x, z).
#[derive(Debug, Clone)]
pub struct PlacementRequest {
    pub descriptor_id: String,
    pub x: i32,
    pub z: i32,
    pub count: u32,
}

/// Batch placement with a stable order: phase 鈫?priority 鈫?descriptor id 鈫?/// request index. This is the entry point the generator uses to drive
/// placement by rules (roads before lots before buildings before vegetation).
///
/// Returns one aggregated `PlacementOutcome` (all placed entities + all
/// rejects). Uses a single shared `placed` list so later phases avoid earlier
/// ones.
pub fn place_all(
    registry: &ObjectRegistry,
    requests: &[PlacementRequest],
    ctx: &PlacementContext<'_>,
    seed: u64,
) -> PlacementOutcome {
    let mut outcome = PlacementOutcome::default();
    let mut placed: Vec<PlacedKind> = Vec::new();

    // Stable order: phase 鈫?priority 鈫?descriptor id 鈫?request index.
    let mut order: Vec<(u8, u32, &str, usize)> = requests
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let d = registry.get(&r.descriptor_id);
            let phase = d.map(|d| d.placement.phase as u8).unwrap_or(255);
            let prio = d.map(|d| d.placement.priority).unwrap_or(u32::MAX);
            (phase, prio, r.descriptor_id.as_str(), idx)
        })
        .collect();
    order.sort_unstable();

    for (_, _, _, idx) in order {
        let r = &requests[idx];
        let Some(desc) = registry.get(&r.descriptor_id) else {
            // Unknown descriptor id: record a reject so it is visible.
            outcome.rejected.push(RejectRecord {
                item_id: r.descriptor_id.clone(),
                candidate_x: r.x,
                candidate_z: r.z,
                reason: "unknown descriptor id".to_string(),
                conflict_with: None,
                rule: "place_all".to_string(),
            });
            continue;
        };
        let mut seed_rot = seed;
        for _ in 0..r.count {
            let one = place_one(
                desc,
                Candidate { x: r.x, z: r.z },
                ctx,
                &mut placed,
                seed_rot,
            );
            outcome.placed.extend(one.placed);
            outcome.rejected.extend(one.rejected);
            // Vary the seed per instance for deterministic-but-different jitter.
            seed_rot = seed_rot.wrapping_add(0x9e3779b97f4a7c15);
        }
    }
    outcome
}

/// Shorthand: order descriptors of a registry by phase, then priority, then id.
pub fn sorted_descriptors(registry: &ObjectRegistry) -> Vec<&ObjectDescriptor> {
    let mut v: Vec<&ObjectDescriptor> = registry.descriptors.values().collect();
    v.sort_by(|a, b| {
        a.placement
            .phase
            .cmp(&b.placement.phase)
            .then(a.placement.priority.cmp(&b.placement.priority))
            .then(a.id.cmp(&b.id))
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building_desc() -> ObjectDescriptor {
        toml::from_str(
            r#"
id = "residential_house"
kind = "building"

[geometry]
footprint = [12.0, 10.0]
height = 7.0

[environment]
on_ground = true
not_in_water = true
max_slope = 12

[placement]
phase = "building"
"#,
        )
        .unwrap()
    }

    #[test]
    fn candidates_deterministic() {
        let a = generate_candidates(50, 50, 12, 12345);
        let b = generate_candidates(50, 50, 12, 12345);
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(x, y)| x.x == y.x && x.z == y.z));
    }

    #[test]
    fn place_on_flat_ground() {
        let desc = building_desc();
        let c = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| None,
            slope_at: &|_, _| 2.0,
            bounds: (0, 0, 200, 200),
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        };
        let mut placed: Vec<PlacedKind> = Vec::new();
        let out = place_one(&desc, Candidate { x: 100, z: 100 }, &c, &mut placed, 42);
        assert_eq!(out.placed.len(), 1, "should place on flat ground");
        assert_eq!(out.rejected.len(), 0);
        assert_eq!(placed.len(), 1);
    }

    #[test]
    fn place_rejects_water() {
        let desc = building_desc();
        let c = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| Some(1.0), // everything underwater
            slope_at: &|_, _| 2.0,
            bounds: (0, 0, 200, 200),
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        };
        let mut placed: Vec<PlacedKind> = Vec::new();
        let out = place_one(&desc, Candidate { x: 100, z: 100 }, &c, &mut placed, 42);
        assert_eq!(out.placed.len(), 0, "should reject all in water");
        assert!(!out.rejected.is_empty());
    }
}
