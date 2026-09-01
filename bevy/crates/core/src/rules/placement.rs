//! Unified placement pipeline: generate deterministic candidates → hard
//! constraint check → soft score → commit (mark occupancy + register placed).

use super::constraint::compile;
use super::errors::{PlacementOutcome, RejectRecord};
use super::schema::ObjectDescriptor;
use super::validator::{check_hard, score_soft, Footprint, PlacementContext, PlacedKind};
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
                out.push(Candidate { x: x + dx * 2 + jx, z: z + dy * 2 + jz });
            }
        }
        ring += 1;
    }
    out.truncate(attempts as usize);
    out
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

    let mut best: Option<(f32, Candidate)> = None;
    let mut best_reject: Option<RejectRecord> = None;

    for cand in candidates {
        let fp = Footprint::from_descriptor(desc, cand.x as f32, cand.z as f32);
        match check_hard(desc, &fp, ctx, &hard, placed) {
            Ok(()) => {
                let s = score_soft(&fp, &soft, placed);
                if best.as_ref().map(|(bs, _)| s > *bs).unwrap_or(true) {
                    best = Some((s, cand));
                }
            }
            Err(reason) => {
                if best_reject.is_none() {
                    best_reject = Some(RejectRecord {
                        item_id: desc.id.clone(),
                        candidate_x: cand.x,
                        candidate_z: cand.z,
                        reason: reason.to_string(),
                        conflict_with: None,
                        rule: "hard".into(),
                    });
                }
            }
        }
    }

    if let Some((_, cand)) = best {
        // Commit: register into placed list (occupancy is tracked by caller
        // via PlacedKind). Build the EntityInstance for the map.
        let fp = Footprint::from_descriptor(desc, cand.x as f32, cand.z as f32);
        let mut entity = EntityInstance {
            entity_id: format!("{}_{}", desc.id, cand.x.abs() * 1000 + cand.z.abs()),
            asset_id: if desc.asset.is_empty() { desc.id.clone() } else { desc.asset.clone() },
            kind: kind_to_str(desc.kind),
            world_x: cand.x,
            world_z: cand.z,
            world_y: ctx.height(cand.x, cand.z).round() as i32,
            scale: 1.0,
            width_m: desc.geometry.footprint[0],
            depth_m: desc.geometry.footprint[1],
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
        });
    } else if let Some(rec) = best_reject {
        outcome.rejected.push(rec);
    }

    outcome
}

/// Map ItemKind → the string kind used by EntityInstance / scene entities.
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
        };
        let mut placed: Vec<PlacedKind> = Vec::new();
        let out = place_one(&desc, Candidate { x: 100, z: 100 }, &c, &mut placed, 42);
        assert_eq!(out.placed.len(), 0, "should reject all in water");
        assert!(!out.rejected.is_empty());
    }
}
