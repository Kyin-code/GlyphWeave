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

fn footprint_for(
    desc: &ObjectDescriptor,
    source: Option<&PlacementSource>,
    cx: f32,
    cz: f32,
) -> Footprint {
    let (width, depth) = source
        .map(|source| (source.width_m * source.scale, source.depth_m * source.scale))
        .unwrap_or((desc.geometry.footprint[0], desc.geometry.footprint[1]));
    Footprint {
        cx,
        cz,
        half_w: width * 0.5 + desc.geometry.clearance,
        half_d: depth * 0.5 + desc.geometry.clearance,
    }
}

fn build_entity(
    desc: &ObjectDescriptor,
    source: Option<&PlacementSource>,
    cand: Candidate,
    rotated: bool,
    world_y: i32,
) -> EntityInstance {
    let (entity_id, asset_id, kind, scale, width, depth, height) = source
        .map(|source| {
            (
                source.entity_id.clone(),
                source.asset_id.clone(),
                source.kind.clone(),
                source.scale,
                source.width_m,
                source.depth_m,
                source.height_m,
            )
        })
        .unwrap_or_else(|| {
            (
                format!("{}_{}", desc.id, cand.x.abs() * 1000 + cand.z.abs()),
                if desc.asset.is_empty() {
                    desc.id.clone()
                } else {
                    desc.asset.clone()
                },
                kind_to_str(desc.kind),
                1.0,
                desc.geometry.footprint[0],
                desc.geometry.footprint[1],
                desc.geometry.height,
            )
        });
    EntityInstance {
        entity_id,
        asset_id,
        kind,
        world_x: cand.x,
        world_z: cand.z,
        world_y,
        scale,
        width_m: if rotated { depth } else { width },
        depth_m: if rotated { width } else { depth },
        height_m: height,
        rotation_y_deg: source.map(|source| source.rotation_y_deg).unwrap_or(0.0)
            + if rotated { 90.0 } else { 0.0 },
        grounding: source
            .map(|source| source.grounding.clone())
            .unwrap_or_default(),
        anchors: source
            .map(|source| source.anchors.clone())
            .unwrap_or_default(),
        bounds: source.and_then(|source| source.bounds),
    }
}

/// Shared committed occupancy index for the rule placement pipeline.
///
/// The index is intentionally independent of the renderer: it stores the same
/// footprint records used by hard constraints and provides a checkpoint API so
/// speculative placement can be rolled back without leaving stale occupancy.
#[derive(Debug, Clone, Default)]
pub struct PlacementIndex {
    entries: Vec<PlacedKind>,
}

impl PlacementIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: Vec<PlacedKind>) -> Self {
        Self { entries }
    }

    pub fn as_slice(&self) -> &[PlacedKind] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(&mut self, placed: PlacedKind) {
        self.entries.push(placed);
    }

    pub fn into_entries(self) -> Vec<PlacedKind> {
        self.entries
    }

    pub fn checkpoint(&self) -> usize {
        self.entries.len()
    }

    pub fn rollback(&mut self, checkpoint: usize) {
        self.entries.truncate(checkpoint.min(self.entries.len()));
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
    let mut index = PlacementIndex::from_entries(std::mem::take(placed));
    let outcome = place_one_with_source(desc, centre, ctx, &mut index, seed, None);
    *placed = index.into_entries();
    outcome
}

fn place_one_with_source(
    desc: &ObjectDescriptor,
    centre: Candidate,
    ctx: &PlacementContext<'_>,
    placed: &mut PlacementIndex,
    seed: u64,
    source: Option<&PlacementSource>,
) -> PlacementOutcome {
    let mut outcome = PlacementOutcome::default();
    let checkpoint = placed.checkpoint();
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
            let mut fp = footprint_for(desc, source, cand.x as f32, cand.z as f32);
            if rot {
                fp = fp.rotated();
            }
            match check_hard(desc, &fp, ctx, &hard, placed.as_slice()) {
                Ok(()) => {
                    let s = score_soft(&fp, &soft, placed.as_slice());
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
                let mut fp = footprint_for(desc, source, cand.x as f32, cand.z as f32);
                if rot {
                    fp = fp.rotated();
                }
                match check_hard(desc, &fp, ctx, &hard, placed.as_slice()) {
                    Ok(()) => {
                        let score = score_soft(&fp, &soft, placed.as_slice());
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
        let mut fp = footprint_for(desc, source, cand.x as f32, cand.z as f32);
        if rot {
            fp = fp.rotated();
        }
        // Preserve the source entity's identity, asset and geometry while
        // applying the descriptor's placement constraints. The descriptor is a
        // rule/constraint source, not an implicit geometry replacement.
        let entity = build_entity(
            desc,
            source,
            cand,
            rot,
            ctx.height(cand.x, cand.z).round() as i32,
        );
        let placed_id = entity.entity_id.clone();
        outcome.placed.push(entity);
        placed.push(PlacedKind {
            id: Some(placed_id),
            // Relations/collision are based on the runtime entity kind, not
            // the descriptor's category. This matters for specialised
            // descriptors such as civic_building, which govern several
            // concrete building kinds while retaining the source identity.
            kind: source
                .map(|source| super::audit::kind_from_str(&source.kind))
                .unwrap_or(desc.kind),
            cx: cand.x as f32,
            cz: cand.z as f32,
            half_w: fp.half_w,
            half_d: fp.half_d,
            tags: desc.tags.clone(),
        });
    } else {
        placed.rollback(checkpoint);
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

/// Source geometry and render identity carried through a rule re-placement.
#[derive(Debug, Clone)]
pub struct PlacementSource {
    pub entity_id: String,
    pub asset_id: String,
    pub kind: String,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    pub scale: f32,
    pub rotation_y_deg: f32,
    pub grounding: crate::worldgen::GroundingSpec,
    pub anchors: Vec<crate::worldgen::SpatialAnchor>,
    pub bounds: Option<crate::worldgen::Bounds2d>,
}

/// A batch placement request: place `count` instances of the object whose
/// descriptor id is `descriptor_id`, centred near (x, z).
#[derive(Debug, Clone)]
pub struct PlacementRequest {
    pub descriptor_id: String,
    pub x: i32,
    pub z: i32,
    pub count: u32,
    pub source: Option<PlacementSource>,
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
    let mut placed = PlacementIndex::new();

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
            let one = place_one_with_source(
                desc,
                Candidate { x: r.x, z: r.z },
                ctx,
                &mut placed,
                seed_rot,
                r.source.as_ref(),
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
    fn placement_index_checkpoint_rolls_back_speculative_entries() {
        let mut index = PlacementIndex::new();
        index.push(PlacedKind {
            id: Some("committed".into()),
            kind: super::super::schema::ItemKind::Building,
            cx: 10.0,
            cz: 10.0,
            half_w: 2.0,
            half_d: 2.0,
            tags: vec![],
        });
        let checkpoint = index.checkpoint();
        index.push(PlacedKind {
            id: Some("speculative".into()),
            kind: super::super::schema::ItemKind::Tree,
            cx: 10.0,
            cz: 10.0,
            half_w: 1.0,
            half_d: 1.0,
            tags: vec![],
        });
        index.rollback(checkpoint);
        assert_eq!(index.len(), 1);
        assert_eq!(index.as_slice()[0].id.as_deref(), Some("committed"));
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
            slope_at_footprint: None,
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
    fn place_all_preserves_source_entity_geometry_and_identity() {
        let desc = building_desc();
        let mut registry = ObjectRegistry::default();
        registry.descriptors.insert(desc.id.clone(), desc.clone());
        let ctx = PlacementContext {
            height_at: &|_, _| 3.0,
            water_level: &|_, _| None,
            slope_at: &|_, _| 0.0,
            slope_at_footprint: None,
            bounds: (0, 0, 500, 500),
            grounding_tolerance: 0.5,
            biome_at: None,
            hazard_at: None,
        };
        let request = PlacementRequest {
            descriptor_id: desc.id.clone(),
            x: 100,
            z: 100,
            count: 1,
            source: Some(PlacementSource {
                entity_id: "generated.school.7".into(),
                asset_id: "prop.school".into(),
                kind: "school".into(),
                width_m: 80.0,
                depth_m: 60.0,
                height_m: 10.0,
                scale: 1.25,
                rotation_y_deg: 0.0,
                grounding: crate::worldgen::GroundingSpec::default(),
                anchors: Vec::new(),
                bounds: None,
            }),
        };
        let outcome = place_all(&registry, &[request], &ctx, 7);
        assert_eq!(outcome.placed.len(), 1);
        let entity = &outcome.placed[0];
        assert_eq!(entity.entity_id, "generated.school.7");
        assert_eq!(entity.asset_id, "prop.school");
        assert_eq!(entity.kind, "school");
        assert_eq!(entity.width_m, 80.0);
        assert_eq!(entity.depth_m, 60.0);
        assert_eq!(entity.height_m, 10.0);
        assert_eq!(entity.scale, 1.25);
    }

    #[test]
    fn place_rejects_water() {
        let desc = building_desc();
        let c = PlacementContext {
            height_at: &|_, _| 0.0,
            water_level: &|_, _| Some(1.0), // everything underwater
            slope_at: &|_, _| 2.0,
            slope_at_footprint: None,
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
