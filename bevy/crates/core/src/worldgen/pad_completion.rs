//! Completion: does the pad's declared PLATFORM still stand after the ENTITY
//! earthwork has been composed on top of the plan's?
//!
//! # The coupling this closes
//!
//! Round 32 measured it on the hillside town. Planning decides a pad's
//! buildability from the plan's OWN earthwork, because at planning time the
//! rule-engine entities do not exist yet: `road_network::refuse_unrealizable_pads`
//! composes the pad list's carves and asks whether every declared flat top is
//! held. Then the entity stage places a 38x20 `resort_lodge` on a 6.0 m platform
//! 12 m away, and that building's support envelope (its blend ring is 114 m wide,
//! the 1:2 bound `plan_terrain_carves` gives a 6 m plate) stands over a planned
//! `storefront` pad's 8x8 top. The plan was right about its own earthwork and the
//! world was still wrong: 81 of 81 footprint cells published 6,000-9,105 mm
//! instead of the declared 5,325 mm, and the whole scene was refused by
//! `bounded_cut_pad_top_not_flat` with no way to say WHICH pad and WHY.
//!
//! # What this module asks
//!
//! One question per planned pad, in the audit's own window
//! ([`bounded_cut::flat_top_bounds`]) and at the audit's own precision (the
//! published millimetre samples):
//!
//! > does the surface the bake will publish still hold this pad's declared top,
//! > cell for cell?
//!
//! The answer is [`bounded_cut::pad_top_holds`] - the same function
//! `bounded_cut::verify` step 5 uses, so the bake and the audit cannot disagree
//! about what "holding the top" means. It is computed on the composition the encoder
//! actually PUBLISHES (`worldgen::PadPlatform::Active`): inside a cap-carrying pad's
//! declared flat top the pad's own platform ([`bounded_cut::platform_surface_m`]) is
//! the surface, both above a neighbour's blended fill and above the pad's own bench.
//!
//! # Round 36: the probe used to read a surface the bake never publishes
//!
//! The probe was wired to the FILL-FIRST composition (`PadPlatform::Dormant`), which
//! is the planning-time reading and deliberately does NOT contain the pad's own
//! platform face. On the grown hillside town that composition published 6,033 mm at
//! the worst cell of a shop pad declared at 5,325 mm, the probe called it a 708 mm
//! lift and decided `Deepen` - and because a deepening changes nothing in a
//! composition that has no platform in it, the second round read the same 708 mm and
//! refused the pad as `Unstable`. The pad's declared platform was never the problem:
//! the encoder publishes it cell for cell inside the pad's own flat top, and
//! `bounded_cut::verify` step 5 (which reads the published raster) finds nothing
//! wrong there. The judgement therefore reads the published composition, so "this
//! pad's top does not hold" means the raster the world ships really does not hold it.
//!
//! # The two answers, in priority order
//!
//! 1. **Deepen.** The pad carries a cap and its own bench can reach the declared
//!    top within that cap, but the published composition still stands over the top:
//!    the pad's platform wins inside its own flat top
//!    ([`bounded_cut::platform_surface_m`]) and the excavation the raster shows is
//!    exactly the excavation `bounded_cut::cut_surface_m` already describes. The
//!    per-cell prohibitions ([`bounded_cut::CellGuards`]) and the footprint
//!    exclusion still apply, because a bench may not cut protected land, water,
//!    the shore passage band, an occupied cell or another accepted bench. Reading the
//!    published composition makes this the RARE arm: a cell the platform cannot own
//!    (a graded road band crossing the pad, a cell the zoning clips) is the only
//!    thing a deepening can still be asked for, and a second round that finds it
//!    unchanged refuses it as `Unstable` instead of publishing it.
//! 2. **Refuse by name.** Everything else - no cap at all, a bench its own cap
//!    cannot deepen far enough, a fill deeper than the one cap, a guarded cell, a
//!    footprint that would share a cell - is refused under
//!    [`super::road_network::PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM`] with the
//!    entity id, the cell count and the lift, and withdrawn from the plan, the
//!    service outcomes, the joint receipt, the conversions and the entity list.
//!    A refused pad is not a parcel of this world any more, exactly as if the
//!    plan had never accepted it.
//!
//! # Bounded
//!
//! [`MAX_COMPLETION_ROUNDS`] is two. A pad can be deepened at most once (a second
//! breach after a deepening is a refusal, not another round), and a refused pad
//! leaves the checked set, so every round that changes anything removes a
//! conflict for good. The loop is a bound, not a schedule: a world whose pads all
//! hold their published platforms - now the normal case, because the probe reads the
//! surface the encoder publishes - spends exactly one round and publishes no
//! completion record at all.

use super::bounded_cut::{self, CellGuards};
use super::road_network::{RoadNetworkPlan, RoadsideSite};
use super::EntityInstance;

/// The fixed number of convergence rounds. Never unbounded: see the module doc.
pub const MAX_COMPLETION_ROUNDS: usize = 2;

/// The deepest lift one repair may remove, in whole millimetres.
///
/// THE bounded-cut cap (`bounded_cut::MAX_CUT_MM`), not a second number: a pad
/// whose declared platform needs more than one cap's worth of another entity's
/// fill taken back is refused instead of quietly excavating past the contract.
pub const MAX_LIFT_MM: i32 = bounded_cut::MAX_CUT_MM;

/// One planned pad whose declared flat top the composed surface does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadTopConflict {
    pub pad_id: String,
    pub road_id: String,
    pub declared_mm: i32,
    /// The cap the pad was accepted under. `probe` only judges cap-carrying pads,
    /// so this is always `Some` in a probe result; it stays optional because a
    /// caller may judge any site, and a fill-only pad can never be deepened.
    pub cap_mm: Option<i32>,
    /// Cells of the declared flat top that do not publish the declared level.
    pub cells: u32,
    /// Cells of the declared flat top.
    pub total_cells: u32,
    /// The highest sample of the window above the declared top, whole millimetres.
    pub lift_mm: i32,
    /// The lowest sample of the window below the declared top, whole millimetres.
    pub shortfall_mm: i32,
    /// The deepest excavation the pad's OWN bench needs from the natural surface
    /// to reach the declared top, whole millimetres.
    pub needed_cut_mm: i32,
    /// The cell that explains the conflict (the worst deviation).
    pub at: Option<(i32, i32, i32)>,
    /// The highest natural sample of the flat top, whole millimetres.
    pub natural_max_mm: i32,
    /// Attribution only: the entity whose own platform stands highest over the
    /// declared top, and that platform's height.
    pub owner: Option<EntityPlatform>,
}

/// One entity platform standing over a pad's declared top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityPlatform {
    pub entity_id: String,
    pub kind: String,
    /// The surface this entity's own support envelope places at the cell, whole
    /// millimetres: its platform height inside its own pad, its blended taper
    /// outside it.
    pub top_mm: i32,
    pub at: (i32, i32),
}

/// What the completion judgement decides about one pad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The pad keeps its platform: the lift is taken back by its own bench.
    Deepen {
        /// The fill the repair removes, whole millimetres (the lift).
        lift_mm: i32,
        /// The excavation the published raster will show from the natural
        /// surface, whole millimetres - the number the ledger measures.
        cut_depth_mm: i32,
    },
    /// The pad leaves the world, by name.
    Refuse { cause: Refusal },
}

/// Why a pad could not be completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No cap at all: a fill-only pad may not excavate, so it cannot take a
    /// neighbour's fill back. `probe` never reports such a pad (a fill-only pad
    /// carries no platform obligation under this capability), so this arm is the
    /// defensive answer for a caller that judges one directly.
    NoCap { lift_mm: i32 },
    /// A cap, but the fill over the declared top is deeper than one cap.
    LiftAboveCap { lift_mm: i32, cap_mm: i32 },
    /// A cap, but the pad's own bench already needs more than the cap from the
    /// natural surface: its declared top was never reachable.
    BenchBeyondCap { needed_cut_mm: i32, cap_mm: i32 },
    /// A cell of the declared flat top stands BELOW it, and a cut cannot fill.
    Shortfall { shortfall_mm: i32 },
    /// A cell of the declared flat top is protected land, water, the shore
    /// passage band or an occupied cell: the per-cell prohibition still holds.
    Guarded { reason: &'static str, at: (i32, i32) },
    /// The pad's own footprint would share a cell with another accepted bench.
    FootprintOverlap {
        other_pad_id: String,
        at: (i32, i32),
    },
    /// A deepening round left the platform still off its declared top.
    Unstable { lift_mm: i32 },
}

impl Refusal {
    /// The published spelling of the reason (the refusal category is the caller's).
    pub fn detail(&self) -> String {
        match self {
            Self::NoCap { lift_mm } => format!(
                "the pad carries no bounded-cut cap, so a fill-only platform may not excavate \
                 {lift_mm} mm of fill away"
            ),
            Self::LiftAboveCap { lift_mm, cap_mm } => format!(
                "{lift_mm} mm of fill stands over the declared top and the pad's own cap is \
                 {cap_mm} mm"
            ),
            Self::BenchBeyondCap {
                needed_cut_mm,
                cap_mm,
            } => format!(
                "the pad's own bench needs {needed_cut_mm} mm from the natural surface to reach \
                 the declared top, above its {cap_mm} mm cap"
            ),
            Self::Shortfall { shortfall_mm } => format!(
                "{shortfall_mm} mm of the declared flat top stands BELOW the declaration, which \
                 a cut cannot restore"
            ),
            Self::Guarded { reason, at } => {
                format!("cell ({}, {}) of the flat top is {reason}", at.0, at.1)
            }
            Self::FootprintOverlap { other_pad_id, at } => format!(
                "the pad's cut footprint would share cell ({}, {}) with pad {other_pad_id}, and \
                 the ledger books one cell once",
                at.0, at.1
            ),
            Self::Unstable { lift_mm } => format!(
                "the pad was deepened once and {lift_mm} mm of fill still stands over the \
                 declared top"
            ),
        }
    }
}

impl Decision {
    pub fn refused(&self) -> bool {
        matches!(self, Self::Refuse { .. })
    }
}

/// Sample the PUBLISHED composition and report every planned pad whose declared
/// flat top it does not hold.
///
/// `published_mm` is the surface the bake publishes - the composition with the
/// bounded-cut platform rule ACTIVE (`worldgen::PadPlatform::Active`), which is what
/// the chunk encoder writes and what `bounded_cut::verify` step 5 re-reads. Inside a
/// cap-carrying pad's own flat top that composition is the pad's declared platform,
/// so a neighbour's blended fill can no longer make a pad look buried when the world
/// ships it flat. A caller that hands in the fill-first composition instead is asking
/// a different question (what the earthwork would compose without the pad's own
/// platform) and will report pads the raster holds - the Round 36 phantom.
/// `natural_mm` is the natural datum the cut is measured from. Both are the bake's own
/// functions, so this probe asks the encoder's question rather than a model of it.
pub fn probe(
    plan: &RoadNetworkPlan,
    world: &[EntityInstance],
    published_mm: &dyn Fn(i32, i32) -> Option<i32>,
    natural_mm: &dyn Fn(i32, i32) -> Option<i32>,
) -> Vec<PadTopConflict> {
    let mut conflicts = Vec::new();
    for site in &plan.roadside_sites {
        // Only a pad that DECLARED an excavation carries the platform obligation
        // this judgement enforces. A fill-only pad's window keeps its historical
        // treatment (the road audit reports it and the bake proceeds), which is what
        // makes a world that never opted into `bounded-cut` byte-identical.
        if site.bounded_cut_mm.is_none() {
            continue;
        }
        let Some(verdict) = bounded_cut::pad_top_holds(site, published_mm) else {
            // A pad with no millimetre declaration makes no platform promise.
            continue;
        };
        if verdict.holds() {
            continue;
        }
        let window = bounded_cut::flat_top_bounds(site);
        let mut needed_cut_mm = 0_i32;
        let mut natural_max_mm = i32::MIN;
        let mut owner = None;
        for z in window[1]..=window[3] {
            for x in window[0]..=window[2] {
                let Some(natural) = natural_mm(x, z) else {
                    continue;
                };
                natural_max_mm = natural_max_mm.max(natural);
                needed_cut_mm = needed_cut_mm.max(natural - verdict.level_mm);
            }
        }
        if natural_max_mm == i32::MIN {
            continue;
        }
        if let Some((x, z, actual)) = verdict.worst_cell {
            if let Some(actual) = actual {
                owner = attribute(
                    world,
                    &site.id,
                    x,
                    z,
                    verdict.level_mm,
                    actual,
                    natural_mm,
                );
            }
        }
        conflicts.push(PadTopConflict {
            pad_id: site.id.clone(),
            road_id: site.road_id.clone(),
            declared_mm: verdict.level_mm,
            cap_mm: site.bounded_cut_mm.map(bounded_cut::clamp_cap),
            cells: verdict.mismatched,
            total_cells: verdict.cells,
            lift_mm: verdict.lift_mm(),
            shortfall_mm: verdict.shortfall_mm(),
            needed_cut_mm: needed_cut_mm.max(0),
            at: verdict.worst_cell.map(|(x, z, actual)| (x, z, actual.unwrap_or(0))),
            natural_max_mm,
            owner,
        });
    }
    conflicts
}

/// Which entity's own support envelope stands over this cell.
///
/// ATTRIBUTION ONLY, never the decision: the decision is made on the composed
/// surface (see [`probe`]). This names the entity whose own fill envelope
/// ([`super::plan_terrain_carves`] of that entity alone, evaluated the way the
/// encoder evaluates it) puts the highest surface on the cell, so a refusal can
/// quote an id and a height instead of a bare number. A blended taper counts: the
/// fill that buries a bench is usually the neighbour's ring, not its platform.
fn attribute(
    world: &[EntityInstance],
    pad_id: &str,
    x: i32,
    z: i32,
    level_mm: i32,
    actual_mm: i32,
    natural_mm: &dyn Fn(i32, i32) -> Option<i32>,
) -> Option<EntityPlatform> {
    // Attribution is only attempted for a cell that actually stands ABOVE the
    // declared top; a cell below it is not another platform's doing.
    if actual_mm <= level_mm {
        return None;
    }
    let Some(natural_mm) = natural_mm(x, z) else {
        return None;
    };
    let base = f64::from(natural_mm) / 1000.;
    // A natural bank is not a platform: the entity must RAISE the ground here.
    let (xf, zf) = (f64::from(x), f64::from(z));
    let mut best: Option<(i32, i32, usize)> = None;
    for (index, entity) in world.iter().enumerate() {
        if entity.entity_id == pad_id {
            continue;
        }
        // The entity's own envelope, evaluated with the encoder's own formula (see
        // `worldgen::apply_terrain_carves`), so "its fill stands here" means the
        // same thing to the attribution and to the bake.
        let mut surface = f64::NEG_INFINITY;
        for carve in super::plan_terrain_carves(std::slice::from_ref(entity)) {
            // The encoder's own distance rule, term by term.
            let outside = super::carve_outside_m(&carve, xf, zf);
            let target = carve
                .profile
                .as_ref()
                .and_then(|profile| profile.height_at(xf, zf))
                .unwrap_or(carve.target_h_m);
            if outside <= 0. {
                surface = surface.max(target);
            } else if outside < carve.blend_m && target > base {
                let t = (outside / carve.blend_m).clamp(0.0, 1.0);
                let smooth_t = t * t * (3.0 - 2.0 * t);
                surface = surface.max(target + (base - target) * smooth_t);
            }
        }
        if !surface.is_finite() {
            continue;
        }
        let top_mm = (surface * 1000.).round() as i32;
        if top_mm <= level_mm {
            continue;
        }
        let distance = (x - entity.world_x).abs() + (z - entity.world_z).abs();
        let better = best.is_none_or(|(known_top, known_distance, known_index)| {
            top_mm > known_top
                || (top_mm == known_top
                    && (distance < known_distance
                        || (distance == known_distance
                            && entity.entity_id < world[known_index].entity_id)))
        });
        if better {
            best = Some((top_mm, distance, index));
        }
    }
    best.map(|(top_mm, _, index)| EntityPlatform {
        entity_id: world[index].entity_id.clone(),
        kind: world[index].kind.clone(),
        top_mm,
        at: (x, z),
    })
}

/// What the world does about one conflict: deepen it, or refuse it by name.
///
/// The order is the priority the contract asks for - the cheapest legal answer
/// first, and a refusal only when no legal repair exists:
///
/// 1. a cell below the declared top cannot be repaired by a cut, ever;
/// 2. the pad's own bench must already reach the declared top within its cap,
///    because a plan that needs more than its own record is refused at planning;
/// 3. the fill standing over the top must fit inside ONE cap
///    ([`MAX_LIFT_MM`]) - the number is the same 2,000 mm every other reader uses;
/// 4. every cell of the flat top must still pass the per-cell prohibitions;
/// 5. the pad's footprint must still be disjoint from every other bench's.
pub fn decide(
    conflict: &PadTopConflict,
    site: &RoadsideSite,
    guards: &CellGuards<'_>,
    deepened_once: bool,
) -> Decision {
    if conflict.shortfall_mm > 0 {
        return Decision::Refuse {
            cause: Refusal::Shortfall {
                shortfall_mm: conflict.shortfall_mm,
            },
        };
    }
    let Some(cap) = conflict.cap_mm else {
        return Decision::Refuse {
            cause: Refusal::NoCap {
                lift_mm: conflict.lift_mm,
            },
        };
    };
    if conflict.needed_cut_mm > cap {
        return Decision::Refuse {
            cause: Refusal::BenchBeyondCap {
                needed_cut_mm: conflict.needed_cut_mm,
                cap_mm: cap,
            },
        };
    }
    if conflict.lift_mm > MAX_LIFT_MM {
        return Decision::Refuse {
            cause: Refusal::LiftAboveCap {
                lift_mm: conflict.lift_mm,
                cap_mm: cap,
            },
        };
    }
    if deepened_once {
        return Decision::Refuse {
            cause: Refusal::Unstable {
                lift_mm: conflict.lift_mm,
            },
        };
    }
    let window = bounded_cut::flat_top_bounds(site);
    for z in window[1]..=window[3] {
        for x in window[0]..=window[2] {
            if let Some(reason) = guards.blocks(x, z) {
                // The pad's OWN entity is the one occupant of its own platform,
                // so a cell it occupies is not an occupied cell for this test.
                if reason == bounded_cut::ON_OCCUPIED {
                    continue;
                }
                return Decision::Refuse {
                    cause: Refusal::Guarded { reason, at: (x, z) },
                };
            }
        }
    }
    let Some(footprint) = site.cut_footprint() else {
        return Decision::Refuse {
            cause: Refusal::NoCap {
                lift_mm: conflict.lift_mm,
            },
        };
    };
    for other in guards
        .cut_footprints
        .iter()
        .filter(|other| other.pad_id != site.id)
    {
        if let Some((x, z)) = bounded_cut::footprint_intersection(&footprint, &other.footprint) {
            return Decision::Refuse {
                cause: Refusal::FootprintOverlap {
                    other_pad_id: other.pad_id.to_string(),
                    at: (x, z),
                },
            };
        }
    }
    Decision::Deepen {
        lift_mm: conflict.lift_mm,
        cut_depth_mm: conflict.needed_cut_mm,
    }
}

/// The published reason string of one refusal, carrying the entity, the cell count
/// and the lift the contract asks for.
pub fn refusal_reason(conflict: &PadTopConflict, cause: &Refusal) -> String {
    let owner = conflict.owner.as_ref().map_or_else(
        || "an entity platform".to_string(),
        |owner| {
            format!(
                "{} ({}) standing {:.3} m at ({}, {})",
                owner.entity_id,
                owner.kind,
                f64::from(owner.top_mm) / 1000.,
                owner.at.0,
                owner.at.1
            )
        },
    );
    format!(
        "pad {} on road {} declares its top at {:.3} m and cannot hold it after the entity \
         earthwork: {owner} puts the surface at {:.3} m, {:.3} m over that declared top across {} \
         of {} footprint cells, and {} - so the pad is refused here by name instead of leaving \
         the cut ledger over a platform the plan cannot reach",
        conflict.pad_id,
        conflict.road_id,
        f64::from(conflict.declared_mm) / 1000.,
        f64::from(conflict.natural_max_mm.max(conflict.declared_mm + conflict.lift_mm)) / 1000.,
        f64::from(conflict.lift_mm) / 1000.,
        conflict.cells,
        conflict.total_cells,
        cause.detail(),
    )
}

/// One record of what the completion pass did, published as
/// `pad-completion.json` so a reader can see the coupling and its verdict.
pub fn record(
    conflict: &PadTopConflict,
    decision: &Decision,
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "padId": conflict.pad_id,
        "roadId": conflict.road_id,
        "declaredMm": conflict.declared_mm,
        "maxCutMm": conflict.cap_mm,
        "footprintCells": conflict.total_cells,
        "mismatchedCells": conflict.cells,
        "liftMm": conflict.lift_mm,
        "shortfallMm": conflict.shortfall_mm,
        "neededCutMm": conflict.needed_cut_mm,
        "naturalMaxMm": conflict.natural_max_mm,
    });
    if let Some(owner) = &conflict.owner {
        value["entityId"] = serde_json::json!(owner.entity_id);
        value["entityKind"] = serde_json::json!(owner.kind);
        value["entityTopMm"] = serde_json::json!(owner.top_mm);
        value["at"] = serde_json::json!([owner.at.0, owner.at.1]);
    } else if let Some((x, z, _)) = conflict.at {
        value["at"] = serde_json::json!([x, z]);
    }
    match decision {
        Decision::Deepen {
            lift_mm,
            cut_depth_mm,
        } => {
            value["outcome"] = serde_json::json!("deepened");
            value["fillRemovedMm"] = serde_json::json!(lift_mm);
            value["cutDepthMm"] = serde_json::json!(cut_depth_mm);
        }
        Decision::Refuse { cause } => {
            value["outcome"] = serde_json::json!("refused");
            value["reason"] = serde_json::json!(cause.detail());
        }
    }
    value
}

#[cfg(test)]
mod tests {
// The entity-stage pad completion: a pad's declared platform either holds, is
// healed by its own bench inside the one cap, or is refused BY NAME and taken out
// of every record that claims it.
//
// What these tests pin, in the order the bake decides it:
//
//  1. an entity platform standing over a pad's declared top refuses that pad by
//     name - category, entity id, cell count and lift - and withdrawing it moves
//     the plan, the joint receipt AND the conversions together;
//  2. a lift the pad's own bench can take back inside one cap is DEEPENED first:
//     the published platform is the declared top, the fill the world gives up is
//     exactly the lift, and the excavation the raster shows is exactly the bench;
//  3. a refused residential pad is charged to the housing minimum as UNFILLED -
//     the declared minimum never moves;
//  4. the other arms (no cap, a bench beyond the cap, a cell below the top, a
//     second round after a deepening) each refuse for their own named reason, so a
//     pad that cannot be completed is never the whole scene's failure.
use super::*;
use crate::worldgen::{GroundingSpec, SceneSpec};

/// One pad, with the fields the completion judgement reads.
fn pad(id: &str, x: i32, z: i32, level_mm: i32, cap: Option<i32>) -> RoadsideSite {
    RoadsideSite {
        program: None,
        purpose: None,
        id: id.into(),
        road_id: "road.0".into(),
        world_x: x,
        world_z: z,
        width_m: 24,
        depth_m: 24,
        support_level_m: level_mm.div_euclid(1000),
        parcel_insets: None,
        support_level_mm: Some(level_mm),
        frontage_road_point: None,
        bounded_cut_mm: cap,
        earthwork_blend_m: None,
        rotation_y_deg: 0.,
        natural_min_m: 5.,
        natural_max_m: 5.4,
    
        entry_corridor: None,
    }
}

/// A building whose own support envelope is the thing that buries a pad.
fn building(id: &str, kind: &str, x: i32, z: i32, size: f32, level_m: f32) -> EntityInstance {
    EntityInstance {
        entity_id: id.into(),
        asset_id: format!("prop.{kind}"),
        kind: kind.into(),
        world_x: x,
        world_z: z,
        world_y: level_m.floor() as i32,
        width_m: size,
        depth_m: size,
        height_m: 8.,
        scale: 1.,
        rotation_y_deg: 0.,
        grounding: GroundingSpec {
            bottom_offset_m: level_m - level_m.floor(),
            ..Default::default()
        },
        anchors: vec![],
        bounds: None,
    }
}

fn plan(sites: Vec<RoadsideSite>) -> RoadNetworkPlan {
    serde_json::from_value(serde_json::json!({
        "model": "test",
        "naturalHeightfieldHash": "hash",
        "sceneId": "scene-0",
        "status": "planned",
        "gridSpacingM": 64,
        "maxRoadbedFillM": 6.0,
        "levelM": 0,
        "candidates": 1,
        "rejectedWater": 0,
        "rejectedRelief": 0,
        "unselectedCandidates": 0,
        "nodes": [],
        "edges": [],
        "roadsideSites": serde_json::to_value(&sites).unwrap(),
    }))
    .expect("the fixture plan round-trips")
}

/// The surface the encoder publishes for a flat natural datum, with the pad
/// platform rule either ACTIVE (what the bake writes) or DORMANT (what the
/// fill-first composition leaves). Both are the production functions.
fn surface(
    entities: &[EntityInstance],
    base_m: f64,
    active: bool,
) -> impl Fn(i32, i32) -> Option<i32> {
    let carves = super::super::plan_terrain_carves(entities);
    move |x: i32, z: i32| {
        let mode = if active {
            super::super::PadPlatform::Active
        } else {
            super::super::PadPlatform::Dormant
        };
        Some(
            (super::super::apply_terrain_carves_with(base_m, x, z, &carves, mode) * 1000.).round()
                as i32,
        )
    }
}

fn natural(base_m: f64) -> impl Fn(i32, i32) -> Option<i32> {
    move |_, _| Some((base_m * 1000.).round() as i32)
}

fn scene() -> SceneSpec {
    SceneSpec {
        scene_id: "scene-0".into(),
        origin_x: 0,
        origin_z: 0,
        width_m: 256,
        depth_m: 256,
        seed_offset: 0,
    }
}

fn zoning() -> crate::worldgen::zoning::ZoningMask {
    let zones = vec![crate::worldgen::LandUseZone {
        id: "urban".into(),
        kind: "urban".into(),
        world_x: 0,
        world_z: 0,
        width_m: 256,
        depth_m: 256,
        source: "test".into(),
    }];
    crate::worldgen::zoning::ZoningMask::new(&scene(), &zones).unwrap()
}

fn guards<'a>(mask: &'a crate::worldgen::zoning::ZoningMask) -> CellGuards<'a> {
    CellGuards {
        mask,
        water: crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::None,
            &[],
            &scene(),
            &serde_json::Value::Null,
        ),
        occupied: &|_, _| false,
        cut_footprints: &[],
    }
}

/// Requirement 2b: an entity platform standing over a pad's declared top refuses
/// that pad BY NAME, and the withdrawal reaches the plan, the joint receipt and the
/// conversions - one operation, every view.
#[test]
fn an_entity_platform_over_a_pad_refuses_it_by_name_and_withdraws_it_everywhere() {
    // Natural ground 5.0 m; the pad declares 5.325 m; a 16 m landuse building with a
    // very wide blend ring raises the surface over the pad's whole 25x25 window.
    let base = 5.0_f64;
    let site = pad("pad.shops.negative", 100, 100, 5_325, Some(2_000));
    let mut plan = plan(vec![site.clone()]);
    plan.pad_conversions = vec![crate::worldgen::frontage::Conversion::rural_to_urban(
        "pad.shops.negative",
        484,
    )];
    plan.joint_layout = Some(crate::worldgen::joint_frontage::JointLayout {
        model: "test".into(),
        residential_minimum: 3,
        baseline_residential_count: 2,
        retained_residential_ids: vec!["pad.shops.negative".into()],
        displaced_residential_ids: vec![],
        replacement_residential_ids: vec![],
        unfilled_residential: 1,
        recovery_rejections: Default::default(),
    });
    let burier = building(
        "generated.urban-building.0",
        "urban_building",
        100,
        190,
        27.,
        16.,
    );
    let mut entities = vec![site.entity(), burier];
    let dormant = surface(&entities, base, false);
    let conflicts = probe(&plan, &entities, &dormant, &natural(base));
    assert_eq!(
        conflicts.len(),
        1,
        "the pad's platform is raised: {conflicts:?}"
    );
    let conflict = &conflicts[0];
    assert_eq!(conflict.pad_id, "pad.shops.negative");
    assert_eq!(conflict.declared_mm, 5_325);
    assert_eq!(conflict.total_cells, 25 * 25, "the audit's own window");
    assert_eq!(
        conflict.cells, conflict.total_cells,
        "every cell of the declared top is off it"
    );
    assert_eq!(conflict.needed_cut_mm, 0, "the pad's own bench needs no cut");
    assert!(
        conflict.lift_mm > MAX_LIFT_MM,
        "the lift is deeper than the one cap can take back: {} mm",
        conflict.lift_mm
    );
    let owner = conflict.owner.as_ref().expect("the refusal names its entity");
    assert_eq!(owner.entity_id, "generated.urban-building.0");
    assert_eq!(owner.kind, "urban_building");
    assert!(
        owner.top_mm > conflict.declared_mm + MAX_LIFT_MM,
        "the named platform stands above one cap over the declaration: {} mm",
        owner.top_mm
    );
    let mask = zoning();
    let decision = decide(conflict, &site, &guards(&mask), false);
    assert_eq!(
        decision,
        Decision::Refuse {
            cause: Refusal::LiftAboveCap {
                lift_mm: conflict.lift_mm,
                cap_mm: 2_000
            }
        }
    );
    let reason = refusal_reason(
        conflict,
        &Refusal::LiftAboveCap {
            lift_mm: conflict.lift_mm,
            cap_mm: 2_000,
        },
    );
    assert!(reason.contains("generated.urban-building.0"), "{reason}");
    assert!(
        reason.contains(&format!("of {} footprint cells", conflict.total_cells)),
        "the refusal counts cells: {reason}"
    );
    // The withdrawal: plan, joint receipt, conversions, and the named category.
    assert!(crate::worldgen::road_network::refuse_pad_top_conflict(
        &mut plan,
        "pad.shops.negative",
        reason
    ));
    assert!(plan.roadside_sites.is_empty(), "the pad leaves the plan");
    assert!(
        plan.pad_conversions.is_empty(),
        "the conversion it earned leaves with it"
    );
    let joint = plan.joint_layout.as_ref().expect("the receipt survives");
    assert_eq!(
        joint.unfilled_residential, 3,
        "the housing minimum is charged, never lowered"
    );
    assert_eq!(
        joint.residential_minimum, 3,
        "the declared minimum never moves"
    );
    assert!(joint.retained_residential_ids.is_empty());
    let rejected = plan
        .roadside_site_rejections
        .iter()
        .find(|rejection| rejection.edge_id == "pad.shops.negative")
        .expect("the refusal is published in the plan");
    assert_eq!(
        rejected.category,
        crate::worldgen::road_network::PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM
    );
    assert!(
        crate::worldgen::road_network::refused_pad_ids(&plan).contains("pad.shops.negative")
    );
    assert!(crate::worldgen::road_network::entity_refused_pad_ids(&plan)
        .contains(&"pad.shops.negative"));
    // Refusing the same pad twice is a no-op: the bounded loop cannot count one pad
    // as two refusals.
    assert!(!crate::worldgen::road_network::refuse_pad_top_conflict(
        &mut plan,
        "pad.shops.negative",
        "again".into()
    ));
    assert_eq!(plan.roadside_site_rejections.len(), 1);
    // The caller filters the entity list with the same id (see
    // `worldgen::complete_pad_tops`), so the pad's own entity cannot outlive it.
    assert!(entities
        .iter()
        .any(|entity| entity.entity_id == "pad.shops.negative"));
}

/// Requirement 2a: a lift the pad's own bench can take back inside the ONE cap is
/// DEEPENED first, with the exact numbers the published raster then holds.
#[test]
fn a_lift_inside_the_one_cap_is_deepened_and_the_platform_holds() {
    // Natural 5.0 m, declared top 4.0 m: the pad's own bench excavates 1000 mm. A
    // neighbour platform legitimately stands at 6.0 m over the pad's window -
    // exactly the 2000 mm the one cap is allowed to take back.
    let base = 5.0_f64;
    let site = pad("pad.bench", 100, 100, 4_000, Some(2_000));
    let plan = plan(vec![site.clone()]);
    let neighbour = building("generated.resort-lodge.0", "resort_lodge", 100, 118, 24., 6.);
    let entities = vec![site.entity(), neighbour];
    let dormant = surface(&entities, base, false);
    let conflicts = probe(&plan, &entities, &dormant, &natural(base));
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    let conflict = &conflicts[0];
    assert_eq!(conflict.declared_mm, 4_000);
    assert_eq!(conflict.lift_mm, 2_000, "the neighbour stands one cap over");
    assert_eq!(conflict.needed_cut_mm, 1_000, "the pad's own bench depth");
    let mask = zoning();
    assert_eq!(
        decide(conflict, &site, &guards(&mask), false),
        Decision::Deepen {
            lift_mm: 2_000,
            cut_depth_mm: 1_000
        },
        "the cheapest legal answer is the pad's bench, not a refusal"
    );
    // What the world then publishes: the platform rule puts the pad's own surface
    // inside its declared flat top, so every cell of it is the declared top...
    let active = surface(&entities, base, true);
    let verdict = bounded_cut::pad_top_holds(&site, &active).expect("the pad has a declaration");
    assert!(verdict.holds(), "the platform holds: {verdict:?}");
    assert_eq!(verdict.highest_mm, 4_000);
    assert_eq!(verdict.lowest_mm, 4_000);
    assert_eq!(verdict.cells, 25 * 25);
    // ...and the excavation the raster shows is exactly the bench: 1000 mm at the
    // centre, never the 2000 mm the cap allows and not the neighbour's 6000 mm.
    assert_eq!(active(100, 100), Some(4_000));
    assert_eq!(5_000 - active(100, 100).unwrap(), 1_000);
    // The published record carries both numbers: the fill it gives back and the
    // excavation it publishes.
    let record = record(conflict, &decide(conflict, &site, &guards(&mask), false));
    assert_eq!(record["outcome"], serde_json::json!("deepened"));
    assert_eq!(record["fillRemovedMm"], serde_json::json!(2_000));
    assert_eq!(record["cutDepthMm"], serde_json::json!(1_000));
    // A lift one millimetre past the cap refuses instead of excavating deeper.
    let mut over = conflict.clone();
    over.lift_mm = 2_001;
    assert_eq!(
        decide(&over, &site, &guards(&mask), false),
        Decision::Refuse {
            cause: Refusal::LiftAboveCap {
                lift_mm: 2_001,
                cap_mm: 2_000
            }
        }
    );
}

/// Round 39, requirement 1: the probe judges the PUBLISHED composition, so a pad
/// whose own platform stands is not reported as buried by the fill that platform
/// overrules - the Round 36 phantom, with the grown hillside town's own numbers.
///
/// The fixture is that world's pad: an 8x8 storefront whose declared top is 5.325 m
/// on ground standing 5.417 m, under a neighbour platform at 6.033 m whose fill
/// covers the pad's whole 81-cell window.
#[test]
fn the_probe_reads_the_published_platform_not_the_fill_it_overrules() {
    let base = 5.417_f64;
    let mut site = pad("pad.shops.negative", 100, 100, 5_325, Some(2_000));
    site.width_m = 8;
    site.depth_m = 8;
    let plan = plan(vec![site.clone()]);
    let entities = vec![
        site.entity(),
        building(
            "generated.urban-building.0",
            "urban_building",
            100,
            116,
            24.,
            6.033,
        ),
    ];
    let mask = zoning();
    // 1. The FILL-FIRST composition has no pad platform in it, so the neighbour's
    //    6.033 m fill reads as a platform conflict the pad can never resolve: 81 of
    //    81 cells off the declared top, a 708 mm lift over ground that stands at
    //    5.417 m, and a bench that only has to excavate 92 mm.
    let fill_first = surface(&entities, base, false);
    let phantom = probe(&plan, &entities, &fill_first, &natural(base));
    assert_eq!(phantom.len(), 1, "{phantom:?}");
    let conflict = &phantom[0];
    assert_eq!(conflict.pad_id, "pad.shops.negative");
    assert_eq!(conflict.declared_mm, 5_325);
    assert_eq!(conflict.cells, 81, "the audit's own 9x9 window");
    assert_eq!(conflict.total_cells, 81);
    assert_eq!(conflict.lift_mm, 708);
    assert_eq!(conflict.needed_cut_mm, 92);
    assert_eq!(conflict.natural_max_mm, 5_417);
    assert_eq!(conflict.shortfall_mm, 0);
    let owner = conflict.owner.as_ref().expect("the phantom names an entity");
    assert_eq!(owner.kind, "urban_building");
    assert_eq!(owner.top_mm, 6_033);
    assert_eq!(conflict.lift_mm, owner.top_mm - conflict.declared_mm);
    // On that reading the repair is a deepening - which changes nothing in a
    // composition that has no platform to deepen, so a second round reads the same
    // 708 mm and the pad is refused as `Unstable`.
    assert_eq!(
        decide(conflict, &site, &guards(&mask), false),
        Decision::Deepen {
            lift_mm: 708,
            cut_depth_mm: 92
        }
    );
    assert_eq!(
        decide(conflict, &site, &guards(&mask), true),
        Decision::Refuse {
            cause: Refusal::Unstable { lift_mm: 708 }
        }
    );
    // 2. The PUBLISHED composition - the one the encoder writes - contains the pad's
    //    own platform, so the very same world reports nothing at all and the first
    //    round converges on the declaration itself.
    let published = surface(&entities, base, true);
    assert!(
        probe(&plan, &entities, &published, &natural(base)).is_empty(),
        "the published raster already holds the declared top"
    );
    let verdict =
        bounded_cut::pad_top_holds(&site, &published).expect("the pad has a declaration");
    assert!(verdict.holds(), "{verdict:?}");
    assert_eq!(verdict.highest_mm, 5_325);
    assert_eq!(verdict.lowest_mm, 5_325);
    assert_eq!(verdict.cells, 81);
    assert_eq!(verdict.lift_mm(), 0);
    assert_eq!(published(100, 100), Some(5_325));
    assert_eq!(published(96, 96), Some(5_325), "the window's far corner");
    assert_eq!(published(104, 104), Some(5_325), "the window's far corner");
    // The excavation that publishes is the pad's own bench, 92 mm, not the 708 mm
    // of neighbour fill above it.
    assert_eq!(5_417 - published(100, 100).unwrap(), 92);
}

/// Requirement 2b, the other arms: no cap, a bench its own cap cannot reach, a cell
/// below the declaration, and a second round after a deepening are all refusals.
#[test]
fn a_pad_that_cannot_be_completed_is_refused_for_the_named_reason() {
    let mask = zoning();
    let g = guards(&mask);
    let site = pad("pad.nocap", 100, 100, 5_325, None);
    let mut conflict = PadTopConflict {
        pad_id: site.id.clone(),
        road_id: site.road_id.clone(),
        declared_mm: 5_325,
        cap_mm: None,
        cells: 625,
        total_cells: 625,
        lift_mm: 675,
        shortfall_mm: 0,
        needed_cut_mm: 0,
        at: Some((88, 88, 6_000)),
        natural_max_mm: 5_417,
        owner: None,
    };
    assert_eq!(
        decide(&conflict, &site, &g, false),
        Decision::Refuse {
            cause: Refusal::NoCap { lift_mm: 675 }
        }
    );
    let capped = pad("pad.deep", 100, 100, 5_325, Some(500));
    conflict.cap_mm = Some(500);
    conflict.needed_cut_mm = 900;
    assert_eq!(
        decide(&conflict, &capped, &g, false),
        Decision::Refuse {
            cause: Refusal::BenchBeyondCap {
                needed_cut_mm: 900,
                cap_mm: 500
            }
        }
    );
    conflict.needed_cut_mm = 0;
    conflict.shortfall_mm = 40;
    assert_eq!(
        decide(&conflict, &capped, &g, false),
        Decision::Refuse {
            cause: Refusal::Shortfall { shortfall_mm: 40 }
        }
    );
    conflict.shortfall_mm = 0;
    assert_eq!(
        decide(&conflict, &capped, &g, true),
        Decision::Refuse {
            cause: Refusal::Unstable { lift_mm: 675 }
        }
    );
    // The bound itself: two rounds, and the bound is the bounded-cut cap, not a
    // second number.
    assert_eq!(MAX_COMPLETION_ROUNDS, 2);
    assert_eq!(MAX_LIFT_MM, 2_000);
    assert_eq!(MAX_LIFT_MM, bounded_cut::MAX_CUT_MM);
}

/// A pad whose window already publishes its declared top is not a conflict at all,
/// so the completion pass publishes nothing and moves nothing.
#[test]
fn a_pad_that_already_holds_its_platform_is_not_a_conflict() {
    let base = 5.0_f64;
    let site = pad("pad.quiet", 100, 100, 5_000, Some(2_000));
    let plan = plan(vec![site.clone()]);
    let entities = vec![site.entity()];
    let dormant = surface(&entities, base, false);
    let conflicts = probe(&plan, &entities, &dormant, &natural(base));
    assert!(
        conflicts.is_empty(),
        "the platform is already the declared top: {conflicts:?}"
    );
    let active = surface(&entities, base, true);
    let verdict = bounded_cut::pad_top_holds(&site, &active).expect("the pad has a declaration");
    assert!(verdict.holds());
    assert_eq!(verdict.lift_mm(), 0);
    assert_eq!(verdict.shortfall_mm(), 0);
}
}
