//! OPT-IN: derive each structure's fill-transition ring from its own size, or
//! from the fill it actually places.
//!
//! # What the legacy ring is, and what it was trying to say
//!
//! Every structure carve is published with a `blend_m` ring: the width over which
//! the pad's flat top ramps back to the natural ground (`worldgen::TerrainCarve`,
//! `worldgen::plan_terrain_carves`). Until this module the ring was
//! `max(6 m, 3 m x (ground_height_m + 32 m))` for a building and then, in
//! `plan_terrain_carves`, widened again to at least `3 m x (ground + 32 m)`. The
//! `32 m` is the representable raster floor (`apply_terrain_carves` clamps every
//! sample into `-32 m .. 256 m`) and the `3` is `1.5 / 0.5`: a cubic smoothstep
//! has a peak derivative of 1.5, so a pad filled to its own top from the raster
//! floor needs `1.5 x (top + 32) / 0.5 = 3 x (top + 32)` metres of run to keep the
//! transition at a 1:2 gradient. The ring says, in other words:
//!
//! > if this plate were carried down to the lowest representable ground, this is
//! > the run over which it may spend the fill.
//!
//! That is a worst case over a hypothetical world, and it is not a function of the
//! structure at all: it reads the platform's ELEVATION (`ground_height_m`, e.g.
//! 16 m on the hillside town -> exactly 144 m) and never its footprint. A 25x23 m
//! `urban_building` therefore carries a 144 m ring - 71 m past every neighbour -
//! and its fill envelops a planned shop pad 86.5 m away, lifting it by 3,780 mm
//! (Round 33's named refusal).
//!
//! # The footprint rule: `derived-from-footprint`
//!
//! `style.terrainPolicy.earthworkBlend = "derived-from-footprint"` replaces that
//! ring, per structure, with
//!
//! ```text
//! blend_m = clamp( max(width_m, height_m) * K + M , FLOOR, CEILING )
//! ```
//!
//! * `K = BLEND_RUN_PER_METRE = 1` metre of run per metre of structure. The
//!   platform stands on the HIGH side of its own footprint (the rebase samples the
//!   highest ground under it), so the ground may fall away beneath it at the yard
//!   grade and the fill at the platform's edge is `yard_grade x size`; the ring
//!   re-spends that fill at the project's own ring grade. Both grades are the
//!   project's existing 25 % contracts (`access::WALKABLE_GRADE`, which is
//!   `bounded_cut::MAX_CUT_RING_GRADE`, and `settlement::landcover::MAX_YARD_GRADE`),
//!   so `K = 0.25 / 0.25 = 1`.
//! * `M = BLEND_APRON_M = 8 m`: the narrowest ring the project ALREADY certifies as
//!   enough to grade a legal bench - `bounded_cut::ring_width_m(2,000 mm)`, i.e. the
//!   whole cap at the 25 % ring grade. It is also the smallest street setback the
//!   frontage request band accepts (`morphology_compile`'s `8..=32`), so a
//!   structure's ring never reaches inside its own required front yard.
//! * `FLOOR = BLEND_FLOOR_M = 8 m` - the same number as `M`: the ring can never be
//!   narrower than the project's minimum feasible ring.
//! * `CEILING = BLEND_CEILING_M = 144 m`: the legacy ring on the plate that this
//!   rule was introduced for (`3 x (16 m + 32 m)`, the hillside town's
//!   `urban_building` platform), and the derived ring is additionally capped by the
//!   legacy ring of the very entity it replaces, so the opt-in can only ever make
//!   an envelope NARROWER, never wider.
//!
//! # The fill rule: `derived-from-fill`
//!
//! The footprint rule stops reading the platform's elevation, but it still has
//! nothing to do with the earthwork: a 24x24 m pad derives `24 + 8 = 32 m` whether
//! it stands on 6 m of fill or on solid rock. The engineering question a fill
//! slope actually answers is
//!
//! > how much run does THIS embankment need before it meets the natural ground?
//!
//! which is `fill depth / side slope`, plus the apron the project already certifies
//! for its graded rings:
//!
//! ```text
//! blend_m = clamp( fill_depth_m / SIDE_SLOPE_GRADE + BLEND_APRON_M , FLOOR, CEILING )
//! ```
//!
//! * `fill_depth_m = max(0, design - natural_min)` - the DEEPEST fill the pad
//!   places inside its own flat top, in metres: its own declared top
//!   (`RoadsideSite::support_level_mm` for a planned pad, `ground_height_m` - or
//!   the longitudinal profile of a graded road - for an entity) minus the lowest
//!   natural ground under it. That is not a new measurement: the project already
//!   bounds every roadbed and pad by exactly this difference
//!   (`road_network`'s `level - natural_min <= max_roadbed_fill_m`, the 6 m fill
//!   contract; `RoadsideSite::natural_min_m` is the same number, published), so the
//!   depth the ring spends is the depth that contract already reads.
//! * `SIDE_SLOPE_GRADE = 0.5` is the 1:2 (H:V) fill batter of a real embankment. It
//!   is NOT a new number: the legacy ring's own `3` is `1.5 / 0.5` (see the top of
//!   this module), so 0.5 - the 1:2 face that formula was written to keep - has
//!   been the project's fill batter since the ring existed. This round only names
//!   the half of that ratio that was an anonymous literal.
//! * `BLEND_APRON_M`, `BLEND_FLOOR_M` and `BLEND_CEILING_M` are reused unchanged.
//!
//! A 6 m fill therefore carries `6 / 0.5 + 8 = 20 m` of ring instead of the
//! `3 x (6 + 32) = 114 m` of the historical formula. The value replaces the ring of
//! a fill-only pad, of a cap-carrying pad (the ring is where the fill tapers, and a
//! capped pad still fills), and of a ROAD, whose depth is measured on its own
//! roadbed (its authored profile where it has one). A road keeps its corridor's own
//! structural ring (`6 m` flat top / `18 m` graded band) as a FLOOR: that transition
//! is structural, and the fill formula may only narrow the part of the ring the
//! fill actually spends.
//!
//! Nothing relaxes: the pad's flat top, the 6 m fill cap, the 8 %/25 % grades, the
//! 2,000 mm cut cap, the setbacks, the shore band and the protected land and water
//! are untouched by both values, and every carve that does not carry a derived ring
//! (every entity of a world that did not opt in) keeps the historical bytes.
//!
//! # Planned pads: the ring travels in the plan record
//!
//! A pad the development plan owns is not stamped on its entity alone. Its
//! record carries the ring ([`stamp_plan`] -> [`RoadsideSite::earthwork_blend_m`])
//! and the entity reads it back out ([`RoadsideSite::entity`]), because three
//! independent readers rebuild that entity from the road plan alone and compare
//! it with the published one byte for byte
//! (`settlement::development::verify`, the plan re-derivation inside the
//! completion pass, and the rules-mode ownership audit). One ring, written once,
//! in the one place both sides read. The record's ring is AUTHORITATIVE: [`stamp`]
//! never re-derives a ring an entity already carries, and a pad rebuilt from its
//! record ([`stamp_restored`]) is never measured a second time.

use super::bounded_cut;
use crate::baked_terrain::BakedTerrain;

/// The authored switch key inside `style.terrainPolicy`.
pub const POLICY_KEY: &str = "earthworkBlend";
/// The authored value that derives the ring from the structure's own footprint.
pub const POLICY_VALUE: &str = "derived-from-footprint";
/// The authored value that derives the ring from the fill the pad actually places.
pub const POLICY_VALUE_FILL: &str = "derived-from-fill";

/// Metres of transition run per metre of the structure's largest dimension.
///
/// `access::WALKABLE_GRADE` (the yard grade) over `bounded_cut::MAX_CUT_RING_GRADE`
/// (the ring grade): two of the project's three existing slope contracts, both
/// 0.25, so the ring is as wide as the structure is large.
pub const BLEND_RUN_PER_METRE: f64 =
    super::access::WALKABLE_GRADE / bounded_cut::MAX_CUT_RING_GRADE;

/// The fixed apron every structure ring keeps, in metres.
///
/// `bounded_cut::ring_width_m(bounded_cut::MAX_CUT_MM)`: the whole 2,000 mm cut cap
/// at the 25 % ring grade, which is the narrowest graded ring the project already
/// requires. Also the lowest street setback the frontage request band accepts.
pub const BLEND_APRON_M: f64 = 8.0;

/// The narrowest ring this rule may publish, in metres.
pub const BLEND_FLOOR_M: f64 = BLEND_APRON_M;

/// The widest ring this rule may publish, in metres.
///
/// The legacy envelope on the reference plate: `3 x (16 m + 32 m) = 144 m`.
pub const BLEND_CEILING_M: f64 = 144.0;

/// The fill batter a real embankment is built to, as a rise over run: a 1:2
/// (horizontal : vertical) side slope, i.e. `0.5`.
///
/// This constant names a number the project already works to. The legacy ring in
/// `worldgen::plan_terrain_carves` is `3 x (ground + 32)`, and that `3` is
/// `1.5 / 0.5`: a cubic smoothstep's peak derivative is 1.5, and 0.5 is the 1:2
/// face the ring was written to preserve.
///
/// It is deliberately STEEPER than the project's cut ring grade
/// (`bounded_cut::MAX_CUT_RING_GRADE = 0.25`, the walkable 25 %): a cut may be
/// crossed on foot, so its taper is held to 25 %, while a fill batter is a built
/// 1:2 face. A 6 m fill therefore spends 12 m of run where a 6 m cut would need
/// 24 m - the fill ring is narrower than a cut ring of the same depth and is still
/// a legal 1:2 face.
pub const SIDE_SLOPE_GRADE: f64 = 0.5;

/// Which derivation, if any, this world asked for.
///
/// Absent, `null`, a non-string or any other spelling is [`Policy::Off`] - the
/// historical fixed-envelope earthwork - and never an error: a world that never
/// asked must keep its published bytes exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Policy {
    /// The historical `3 x (ground + 32)` envelope.
    #[default]
    Off,
    /// The ring the structure's own footprint derives.
    Footprint,
    /// The ring the fill the pad actually places derives.
    Fill,
}

impl Policy {
    /// Does this policy publish a derived ring at all?
    pub fn enabled(self) -> bool {
        self != Self::Off
    }
}

/// The declared policy of one world, read from `style.terrainPolicy.earthworkBlend`.
pub fn policy(style: &serde_json::Value) -> Policy {
    match style
        .get("terrainPolicy")
        .and_then(|policy| policy.get(POLICY_KEY))
        .and_then(serde_json::Value::as_str)
    {
        Some(POLICY_VALUE) => Policy::Footprint,
        Some(POLICY_VALUE_FILL) => Policy::Fill,
        _ => Policy::Off,
    }
}

/// Did this world ask for either derivation?
///
/// Only the two declared strings opt in. Absent, `null`, a non-string or any other
/// spelling is the historical fixed-envelope earthwork.
pub fn enabled(style: &serde_json::Value) -> bool {
    policy(style).enabled()
}

/// The footprint-derived fill-transition ring of one structure, in metres.
///
/// `max(width_m, height_m) * K + M`, clamped into
/// `BLEND_FLOOR_M ..= BLEND_CEILING_M`.
pub fn derived_blend_m(width_m: f32, height_m: f32) -> f64 {
    let largest = f64::from(width_m).max(f64::from(height_m));
    (largest * BLEND_RUN_PER_METRE + BLEND_APRON_M).clamp(BLEND_FLOOR_M, BLEND_CEILING_M)
}

/// The deepest fill the pad places over its own flat top, in metres.
///
/// `max(0, design - natural_min)`: the pad's declared top minus the LOWEST natural
/// ground under it, which is where an embankment is deepest and where its batter
/// has to run the furthest. The project already reads exactly this difference as
/// the fill depth of a roadbed (`road_network`'s 6 m contract), so the ring spends
/// the same number the fill contract bounds.
pub fn fill_depth_m(design_m: f64, natural_min_m: f64) -> f64 {
    (design_m - natural_min_m).max(0.0)
}

/// The fill-derived fill-transition ring of one pad, in metres.
///
/// `fill_depth_m / SIDE_SLOPE_GRADE + BLEND_APRON_M`, clamped into
/// `BLEND_FLOOR_M ..= BLEND_CEILING_M`. A 6 m fill publishes `20 m`; the same pad
/// publishes `8 m` with no fill at all and the ceiling binds above `68 m` of fill.
pub fn derived_fill_blend_m(fill_depth_m: f64) -> f64 {
    (fill_depth_m.max(0.0) / SIDE_SLOPE_GRADE + BLEND_APRON_M).clamp(BLEND_FLOOR_M, BLEND_CEILING_M)
}

/// The declared top of one published pad, in metres.
fn site_design_m(site: &super::road_network::RoadsideSite) -> f64 {
    site.support_level_mm
        .map_or(f64::from(site.support_level_m), |mm| f64::from(mm) / 1000.)
}

/// The fill depth ONE PUBLISHED PAD places, in metres, or `None` when the record
/// carries no natural range (a pad whose natural ground was never sampled).
///
/// Read off the two numbers the record already publishes
/// (`support_level_mm`/`support_level_m` and `natural_min_m`), so an auditor
/// holding only the plan can re-derive it - and so the ring the plan publishes and
/// the ring the world carves cannot come from two different measurements.
pub fn site_fill_depth_m(site: &super::road_network::RoadsideSite) -> Option<f64> {
    site.natural_min_m
        .is_finite()
        .then(|| fill_depth_m(site_design_m(site), f64::from(site.natural_min_m)))
}

/// The fill-derived ring of ONE PUBLISHED PAD, in metres, or `None` when its fill
/// depth cannot be derived from the record.
pub fn site_fill_ring_m(site: &super::road_network::RoadsideSite) -> Option<f32> {
    site_fill_depth_m(site).map(|depth| derived_fill_blend_m(depth) as f32)
}

/// The design surface one entity's carve is built to, at one cell of its own flat
/// top: the authored longitudinal profile of a graded road where it has one, and
/// the entity's own declared top everywhere else.
fn entity_design_m(entity: &super::EntityInstance, x: i32, z: i32) -> f64 {
    entity
        .grounding
        .road_profile
        .as_ref()
        .and_then(|profile| profile.height_at(f64::from(x), f64::from(z)))
        .unwrap_or_else(|| f64::from(entity.ground_height_m()))
}

/// The fill depth ONE UNPLANNED ENTITY places over its own flat top, in metres,
/// measured against the natural raster, or `None` when no cell of that flat top is
/// inside the raster.
///
/// The rectangle is [`super::carve_flat_top_half_extents`] - the very cells the
/// carve flattens and the ledger books - so "the pad's own range" means one thing
/// for the ring, the raster and the audit.
pub fn entity_fill_depth_m(natural: &BakedTerrain, entity: &super::EntityInstance) -> Option<f64> {
    let (half_w, half_d) = super::carve_flat_top_half_extents(entity);
    let mut deepest = 0.0_f64;
    let mut sampled = false;
    for dz in -(half_d.ceil() as i32)..=(half_d.ceil() as i32) {
        for dx in -(half_w.ceil() as i32)..=(half_w.ceil() as i32) {
            let (x, z) = (entity.world_x + dx, entity.world_z + dz);
            let Some(natural_m) = natural.height(x, z) else {
                continue;
            };
            sampled = true;
            deepest = deepest.max(entity_design_m(entity, x, z) - f64::from(natural_m));
        }
    }
    sampled.then(|| deepest.max(0.0))
}

/// The fill-derived ring of one entity's carve, in metres.
///
/// A ROAD keeps its own corridor ring (`6 m` flat top, `18 m` graded band) as a
/// floor: that transition is structural, so the fill formula may only narrow the
/// part of the ring the fill actually spends. A STRUCTURE DECK gets no ring at all:
/// it places no embankment (its `plan_terrain_carves` carve is skipped entirely), so
/// a fill depth measured over a span would describe earthwork the world never puts
/// there.
fn entity_fill_ring_m(entity: &super::EntityInstance, fill_depth_m: f64) -> Option<f64> {
    if entity.grounding.structure_deck {
        return None;
    }
    let kind = entity.kind.as_str();
    if super::carves_as_road(kind) {
        Some(derived_fill_blend_m(fill_depth_m).max(super::carve_base_ring_m(entity)))
    } else if super::carves_as_building(kind) {
        Some(derived_fill_blend_m(fill_depth_m))
    } else {
        None
    }
}

/// Stamp every structure of one world with its derived ring, and answer how many
/// were stamped.
///
/// Inert (and byte-inert) unless the style asked for it. What is stampable depends
/// on the policy:
///
/// * the FOOTPRINT ring covers only the kinds `plan_terrain_carves` carves as a
///   BUILDING. A road keeps its graded corridor envelope (`6 m` flat, `18 m`
///   graded, and its own fill contract), and a structure deck is carried by an
///   engineering structure and gets no carve at all.
/// * the FILL ring covers those kinds AND every road: a road's own fill toward the
///   natural ground is a real embankment, measured on its roadbed (its authored
///   profile where it has one), floored by its corridor ring.
///
/// `planned` is the set of entity ids the DEVELOPMENT PLAN owns. Those are
/// stamped by [`stamp_plan`] instead, before the plan record is published: their
/// record carries the ring, so the rebuild that derives the entity from the road
/// plan alone (`settlement::development::plan`) reads the same number and the
/// plan/entity byte comparison stays exact. A planned pad that the world carries
/// WITHOUT a record ring (a world that did not opt in) gets none here either,
/// because `stamp` never touches a planned id. An entity that ALREADY carries a
/// ring keeps it: the record is the one place a pad's ring is decided, and this
/// pass may never publish a second number for the same pad.
///
/// `natural` is the natural raster the fill is measured against; the footprint
/// policy does not read it.
pub fn stamp(
    entities: &mut [super::EntityInstance],
    style: &serde_json::Value,
    planned: &std::collections::BTreeSet<String>,
    natural: &BakedTerrain,
) -> usize {
    let policy = policy(style);
    let mut stamped = 0;
    for entity in entities.iter_mut() {
        if planned.contains(&entity.entity_id) || entity.grounding.structure_deck {
            continue;
        }
        if entity.grounding.earthwork_blend_m.is_some() {
            continue;
        }
        let ring = match policy {
            Policy::Off => None,
            Policy::Footprint => super::carves_as_building(&entity.kind)
                .then(|| derived_blend_m(entity.width_m, entity.height_m)),
            Policy::Fill => entity_fill_depth_m(natural, entity)
                .and_then(|depth| entity_fill_ring_m(entity, depth)),
        };
        if let Some(ring) = ring {
            entity.grounding.earthwork_blend_m = Some(ring as f32);
            stamped += 1;
        }
    }
    stamped
}

/// Re-apply the rings a PLAN RECORD already carries after an entity has been
/// replaced by that record, and stamp nothing else.
///
/// A ring is MEASURED once, against the natural raster that was in scope at
/// [`stamp`]; this pass can only confirm what the record already publishes, so it
/// needs no raster. Under [`Policy::Fill`] it stamps nothing at all: every pad it
/// could reach carries its record's ring already, and the rule-engine entities
/// were measured by [`stamp`]. Under [`Policy::Footprint`] it is exactly the
/// historical stamp, which is the same number the record carries.
pub fn stamp_restored(
    entities: &mut [super::EntityInstance],
    style: &serde_json::Value,
    planned: &std::collections::BTreeSet<String>,
) -> usize {
    let policy = policy(style);
    if policy != Policy::Footprint {
        return 0;
    }
    let mut stamped = 0;
    for entity in entities.iter_mut() {
        if planned.contains(&entity.entity_id) {
            continue;
        }
        if entity.grounding.earthwork_blend_m.is_some() {
            continue;
        }
        if stamp_entity(entity, policy) {
            stamped += 1;
        }
    }
    stamped
}

/// The entity ids the development plan owns, as the stamp exclusion set.
pub fn planned_ids(
    plan: Option<&crate::settlement::development::DevelopmentPlan>,
) -> std::collections::BTreeSet<String> {
    plan.map(|plan| {
        plan.buildings
            .iter()
            .map(|building| building.entity.entity_id.clone())
            .collect()
    })
    .unwrap_or_default()
}

/// One entity, once, in the FOOTPRINT policy. Answers whether it carries a
/// derived ring now.
pub fn stamp_entity(entity: &mut super::EntityInstance, policy: Policy) -> bool {
    if policy != Policy::Footprint
        || entity.grounding.structure_deck
        || !super::carves_as_building(&entity.kind)
    {
        return false;
    }
    entity.grounding.earthwork_blend_m =
        Some(derived_blend_m(entity.width_m, entity.height_m) as f32);
    true
}

/// The footprint-derived fill-transition ring of ONE PUBLISHED PAD, in metres.
///
/// The same formula, read off the two numbers the pad's realized entity carries
/// (`RoadsideSite::entity`: `width_m` and `height_m`), so a pad's record and its
/// entity can never publish two different rings.
pub fn site_ring_m(site: &super::road_network::RoadsideSite) -> f32 {
    derived_blend_m(site.width_m as f32, site.height_m()) as f32
}

/// Stamp every pad and every road of one plan with the ring its own fill (or
/// footprint) derives, and answer how many records were stamped.
///
/// This is the PLAN-RECORD half of [`stamp`], and the reason either opt-in can
/// cover a planned pad at all. `stamp` above deliberately skips the pads the
/// development plan owns, because a planned pad's entity is rebuilt from the road
/// plan (`settlement::development::plan` -> `RoadsideSite::entity`) by three
/// independent readers that compare the rebuild with the published entity byte
/// for byte. A ring that lived only on the entity was therefore a plan/entity
/// split, and the hillside town's planned pads kept 114 m rings
/// (`3 x (6 m + 32 m)`) while every rule-engine structure was already derived.
///
/// Writing the ring into the RECORD closes that: the record carries it, the
/// entity reads it out of the record, and the rebuild reads it out of the record
/// too. Inert - and byte-inert - unless the style opted in: `None` serialises
/// nothing, so a world that never asked keeps its history exactly.
///
/// The predicate is the pad's own kind (`plan_terrain_carves` only carves those as
/// buildings), so a pad whose kind would never read the number does not gain one.
/// Under [`Policy::Fill`] the depth is the record's OWN published measurement
/// (`support_level` against `natural_min_m`) for a pad, and the roadbed's fill over
/// the plan's own road entity for a ROAD - measured here, once, against `natural` -
/// because the independent road audit rebuilds the published road from this plan
/// and refuses a `grounding` that differs from the record by name ("road transform
/// differs from plan"). A pad whose record carries no natural range keeps the
/// historical ring rather than inventing one.
pub fn stamp_plan(
    plan: &mut super::road_network::RoadNetworkPlan,
    style: &serde_json::Value,
    natural: &BakedTerrain,
) -> usize {
    let policy = policy(style);
    if !policy.enabled() {
        return 0;
    }
    let mut stamped = 0;
    if policy == Policy::Fill {
        // Roads first, off the entities this very plan publishes: the ring is the
        // record's, so `entities()` and the audit read one number.
        let rings: std::collections::BTreeMap<String, f32> = plan
            .entities()
            .into_iter()
            .filter_map(|entity| {
                let depth = entity_fill_depth_m(natural, &entity)?;
                let ring = entity_fill_ring_m(&entity, depth)?;
                Some((entity.entity_id, ring as f32))
            })
            .collect();
        for edge in plan.edges.iter_mut() {
            if let Some(ring) = rings.get(&edge.id) {
                edge.earthwork_blend_m = Some(*ring);
                stamped += 1;
            }
        }
    }
    for site in plan.roadside_sites.iter_mut() {
        if !super::carves_as_building(site.kind()) {
            continue;
        }
        let ring = match policy {
            Policy::Off => continue,
            Policy::Footprint => Some(site_ring_m(site)),
            Policy::Fill => site_fill_ring_m(site),
        };
        if let Some(ring) = ring {
            site.earthwork_blend_m = Some(ring);
            stamped += 1;
        }
    }
    stamped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 1. The default is the historical ring, and it is exactly the constant the
    ///    legacy formula yields: `3 x (16 + 32) = 144 m`.
    #[test]
    fn the_default_is_the_legacy_fixed_envelope() {
        for style in [
            json!({}),
            json!({"terrainPolicy": {}}),
            json!({"terrainPolicy": {"earthworkBlend": "fixed"}}),
            json!({"terrainPolicy": {"earthworkBlend": null}}),
            json!({"terrainPolicy": {"earthworkBlend": 144}}),
            json!({"frontageProgram": {"terrainPolicy": "derived-from-footprint"}}),
            json!({"terrainPolicy": "derived-from-footprint"}),
            json!({"terrainPolicy": {"earthworkBlend": "derived from fill"}}),
            json!({"terrainPolicy": {"earthworkBlend": "derived-from-fill "}}),
        ] {
            assert!(!enabled(&style), "{style} must not opt in");
            assert_eq!(policy(&style), Policy::Off, "{style}");
        }
        assert!(enabled(
            &json!({"terrainPolicy": {"earthworkBlend": "derived-from-footprint"}})
        ));
        assert_eq!(
            policy(&json!({"terrainPolicy": {"earthworkBlend": "derived-from-footprint"}})),
            Policy::Footprint
        );
        assert_eq!(
            policy(&json!({"terrainPolicy": {"earthworkBlend": "derived-from-fill"}})),
            Policy::Fill
        );
        assert!(enabled(
            &json!({"terrainPolicy": {"earthworkBlend": "derived-from-fill"}})
        ));
    }

    /// 2. The footprint-derived ring IS the formula, for the sizes this project
    ///    authors.
    #[test]
    fn the_derived_ring_is_the_footprint_formula() {
        // (width_m, height_m) -> max(w, h) * 1 + 8, clamped into 8..=144.
        for (width, height, want) in [
            (2.0_f32, 1.0_f32, 10.0_f64),
            (6.0, 2.0, 14.0),
            (8.0, 6.0, 16.0),
            (12.0, 5.0, 20.0),
            (16.0, 16.0, 24.0),
            (24.0, 8.0, 32.0),
            (25.0, 12.0, 33.0),
            (30.0, 10.0, 38.0),
            (38.0, 8.0, 46.0),
            (40.0, 30.0, 48.0),
            (100.0, 60.0, 108.0),
            (136.0, 8.0, 144.0),
            (300.0, 100.0, 144.0),
            (300.0, 400.0, 144.0),
        ] {
            assert_eq!(
                derived_blend_m(width, height),
                want,
                "a {width}x{height} m structure"
            );
        }
    }

    /// 3. Both clamps hold at the edges of the range.
    #[test]
    fn the_derived_ring_is_clamped_at_both_ends() {
        assert_eq!(derived_blend_m(0.0, 0.0), BLEND_FLOOR_M);
        assert_eq!(derived_blend_m(0.5, 0.0), BLEND_APRON_M + 0.5);
        assert_eq!(derived_blend_m(-40.0, -40.0), BLEND_FLOOR_M);
        assert_eq!(derived_blend_m(1e6, 1e6), BLEND_CEILING_M);
        // The apron alone never lands below the floor, and the ceiling only ever
        // binds above `(144 - 8) / 1 = 136 m` of structure.
        assert!(derived_blend_m(0.0, 0.0) >= BLEND_FLOOR_M);
        assert!(derived_blend_m(135.9, 0.0) < BLEND_CEILING_M);
        assert_eq!(derived_blend_m(136.0, 0.0), BLEND_CEILING_M);
        assert_eq!(
            BLEND_FLOOR_M,
            bounded_cut::ring_width_m(bounded_cut::MAX_CUT_MM)
        );
        assert_eq!(BLEND_CEILING_M, 3.0 * (16.0 + 32.0));
    }

    /// 3b. The fill-derived ring IS `depth / 0.5 + 8`, for the depths this project
    ///     publishes, and the 6 m fill the round is about publishes 20 m - the
    ///     `12 m` of 1:2 batter plus the 8 m apron.
    #[test]
    fn the_fill_ring_is_the_fill_formula() {
        // fill depth (m) -> ring width (m): 2 m of run per metre of fill, plus 8.
        // Dyadic depths only, so the equality is exact and not a rounding artefact.
        for (depth, want) in [
            (0.0_f64, 8.0_f64),
            (0.25, 8.5),
            (0.5, 9.0),
            (1.0, 10.0),
            (1.5, 11.0),
            (2.0, 12.0),
            (3.0, 14.0),
            (3.5, 15.0),
            (4.0, 16.0),
            (5.0, 18.0),
            (5.5, 19.0),
            (6.0, 20.0),
            (10.0, 28.0),
            (20.0, 48.0),
            (68.0, 144.0),
            (68.5, 144.0),
            (200.0, 144.0),
            (-3.0, 8.0),
        ] {
            assert_eq!(derived_fill_blend_m(depth), want, "a {depth} m fill");
        }
        // The one number the round quotes: 6 m of fill at 1:2 is 12 m of run plus
        // the 8 m apron, NOT the 114 m the elevation formula publishes.
        assert_eq!(derived_fill_blend_m(6.0), 12.0 + BLEND_APRON_M);
        assert!(
            derived_fill_blend_m(6.0) < 3.0 * (6.0 + 32.0),
            "the fill ring is narrower than the legacy ring at the same plate"
        );
        // The fill batter is the 1:2 face the legacy `3` was written to keep:
        // `1.5 / 0.5 == 3`.
        assert_eq!(1.5 / SIDE_SLOPE_GRADE, 3.0);
        // A fill spends half the run a cut of the same depth needs at the project's
        // 25 % cut ring grade, and is still a legal 1:2 face.
        let cut_run = 6.0 / bounded_cut::MAX_CUT_RING_GRADE;
        assert_eq!(cut_run, 2.0 * (6.0 / SIDE_SLOPE_GRADE));
        // Non-dyadic depths: the formula, to the last bit that matters (a published
        // ring is an f32 metre count).
        for depth in [2.4_f64, 3.57, 0.19, 4.83] {
            let ring = derived_fill_blend_m(depth);
            assert!((ring - (depth / SIDE_SLOPE_GRADE + BLEND_APRON_M)).abs() < 1e-12);
        }
    }

    /// 4. The fill depth is the difference the project's own fill contract reads.
    #[test]
    fn the_fill_depth_is_the_contract_difference() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(fill_depth_m(10.04, 8.79), 1.25));
        assert_eq!(fill_depth_m(6.0, 0.0), 6.0);
        assert_eq!(fill_depth_m(2.0, 5.0), 0.0, "a cut is never a fill");
        assert_eq!(fill_depth_m(-1.0, 3.5), 0.0);
        // The 1 km probe's deepest pad: a 6 m plate over natural ground at 0.
        assert_eq!(derived_fill_blend_m(fill_depth_m(6.0, 0.0)), 20.0);
    }

    /// 5. The same world buries fewer pad cells after the switch: the hillside
    ///    town's own coupling, in miniature. A 25x23 `urban_building` on a 16 m
    ///    plate stands 86.5 m from an 8x8 shop pad's window; the legacy ring buries
    ///    every one of the 81 cells by 3.7 m, the derived ring reaches none of them.
    #[test]
    fn the_derived_ring_stops_burying_the_neighbour_pad() {
        use super::super::{
            EntityInstance, GroundingSpec, PadPlatform, TerrainCarve, apply_terrain_carves_with,
            plan_terrain_carves,
        };
        let building = EntityInstance {
            entity_id: "generated.urban-building.test".into(),
            asset_id: "prop.building".into(),
            kind: "urban_building".into(),
            world_x: 40,
            world_z: 220,
            world_y: 16,
            width_m: 25.,
            depth_m: 23.,
            height_m: 12.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        };
        // The natural ground under the pad's window, from the published raster.
        const BASE_M: f64 = 5.376;
        let window: Vec<(i32, i32)> = (256..=264)
            .flat_map(|z| (140..=148).map(move |x| (x, z)))
            .collect();
        let bury = |carves: &[TerrainCarve]| -> (usize, i64) {
            let mut cells = 0;
            let mut deepest = 0_i64;
            for (x, z) in &window {
                let surface =
                    apply_terrain_carves_with(BASE_M, *x, *z, carves, PadPlatform::Dormant);
                let rise = ((surface - BASE_M) * 1000.).round() as i64;
                if rise > 0 {
                    cells += 1;
                    deepest = deepest.max(rise);
                }
            }
            (cells, deepest)
        };
        let legacy = plan_terrain_carves(std::slice::from_ref(&building));
        assert_eq!(legacy[0].blend_m, BLEND_CEILING_M);
        assert_eq!(
            bury(&legacy),
            (81, 3729),
            "the 144 m ring buries every cell of the pad"
        );
        let mut derived = building.clone();
        assert!(stamp_entity(&mut derived, Policy::Footprint));
        assert_eq!(
            derived.grounding.earthwork_blend_m,
            Some(BLEND_APRON_M as f32 + 25.)
        );
        assert_eq!(
            bury(&plan_terrain_carves(std::slice::from_ref(&derived))),
            (0, 0)
        );
        // And the switch is inert when it is off: nothing is stamped, and the
        // carve is the legacy one, bit for bit.
        let mut off = building.clone();
        assert!(!stamp_entity(&mut off, Policy::Off));
        assert!(off.grounding.earthwork_blend_m.is_none());
        assert_eq!(
            plan_terrain_carves(std::slice::from_ref(&off))[0].blend_m,
            BLEND_CEILING_M
        );
    }

    /// 6. The fill rule on the SAME 16 m plate: the ring is `(16 - 5.376) / 0.5 + 8`
    ///    metres, i.e. it reaches 29 m past the plate instead of 144 m, and it
    ///    still reaches none of the neighbour pad's cells.
    #[test]
    fn the_fill_rule_narrows_the_same_plate_without_moving_a_cell() {
        let ring = derived_fill_blend_m(fill_depth_m(16.0, 5.376));
        assert!((ring - (8.0 + 21.248)).abs() < 1e-9, "{ring}");
        assert!(ring < BLEND_CEILING_M);
        assert!(
            ring < derived_blend_m(25.0, 12.0),
            "the 33 m footprint ring"
        );
        // On a 6 m plain with the natural ground at zero, the round's own case.
        assert_eq!(derived_fill_blend_m(fill_depth_m(6.0, 0.0)), 20.0);
        assert_eq!(
            derived_fill_blend_m(fill_depth_m(6.0, 0.0)),
            6.0 / SIDE_SLOPE_GRADE + BLEND_APRON_M
        );
    }

    /// 7. A CONSTRUCTED WORLD, end to end: a 6 m platform on a flat 0 m plain,
    ///    and a shop whose door faces it 19 m from the platform's own edge.
    ///
    ///    The 114 m legacy ring spends the platform's 6 m of fill out to the
    ///    shop's door and buries it under 5.5 m of ground - a step no pedestrian
    ///    contract allows. The fill ring is 20 m (`6 / 0.5 + 8`), stops 1 m short
    ///    of the door, and the door publishes its own natural surface, so the step
    ///    into it is walkable again. This is the 1 km probe's round-53 defect in
    ///    miniature, and the shapes below are the ones the fixtures reproduce.
    #[test]
    fn a_buried_doorway_is_walkable_under_the_fill_ring() {
        use super::super::{
            EntityInstance, GroundingSpec, PadPlatform, SceneSpec, TerrainCarve,
            apply_terrain_carves_with, plan_terrain_carves,
        };
        const PLATE_TOP_M: f32 = 6.0;
        const SHOP_Z: i32 = 78;
        /// The shop's own front cell, one metre outside its flat top, facing the
        /// platform 32 m away.
        const DOOR: (i32, i32) = (48, 72);
        let scene = SceneSpec {
            scene_id: "blend-door".into(),
            origin_x: 0,
            origin_z: 0,
            width_m: 128,
            depth_m: 128,
            seed_offset: 0,
        };
        let mut natural = BakedTerrain::new(&scene).expect("the fixture raster");
        let values = vec![0.0_f64; 128 * 128];
        natural
            .insert_chunk(
                0,
                0,
                128,
                128,
                &crate::height_payload::encode_millimetres(128, 128, values).expect("encoded"),
            )
            .expect("one chunk covers the scene");
        let entity =
            |id: &str, kind: &str, z: i32, world_y: i32, width: f32, depth: f32| EntityInstance {
                entity_id: id.into(),
                asset_id: format!("prop.{kind}"),
                kind: kind.into(),
                world_x: 48,
                world_z: z,
                world_y,
                width_m: width,
                depth_m: depth,
                height_m: 8.,
                scale: 1.,
                rotation_y_deg: 0.,
                grounding: GroundingSpec::default(),
                anchors: vec![],
                bounds: None,
            };
        let platform = entity("plate", "urban_building", 40, PLATE_TOP_M as i32, 24., 24.);
        let shop = entity("shop", "storefront", SHOP_Z, 0, 12., 10.);
        // The published fill over the natural plain at the shop's door, in mm.
        let rise_at_door = |carves: &[TerrainCarve]| -> i64 {
            let surface =
                apply_terrain_carves_with(0.0, DOOR.0, DOOR.1, carves, PadPlatform::Dormant);
            (surface * 1000.).round() as i64
        };

        // --- the closed state: the elevation ring, and a doorstep 5.5 m in the air.
        let legacy = plan_terrain_carves(&[platform.clone(), shop.clone()]);
        assert_eq!(legacy[0].blend_m, 3.0 * (6.0 + 32.0), "the 114 m envelope");
        assert_eq!(legacy[1].blend_m, 3.0 * (0.0 + 32.0), "the 96 m envelope");
        assert_eq!(rise_at_door(&legacy), 5_556, "the doorstep is buried");
        assert!(
            rise_at_door(&legacy) as f64 / 1000. > super::super::access::WALKABLE_GRADE,
            "a step far outside the 25% walkable contract"
        );

        // --- the fill value: 20 m and 8 m, and the doorstep back on the ground.
        let mut entities = vec![platform, shop];
        let stamped = stamp(
            &mut entities,
            &json!({"terrainPolicy": {"earthworkBlend": "derived-from-fill"}}),
            &std::collections::BTreeSet::new(),
            &natural,
        );
        assert_eq!(stamped, 2, "both structures are measured");
        assert_eq!(entities[0].grounding.earthwork_blend_m, Some(20.0));
        assert_eq!(entities[1].grounding.earthwork_blend_m, Some(8.0));
        let fill = plan_terrain_carves(&entities);
        assert_eq!(
            fill[0].blend_m, 20.0,
            "a 6 m fill at 1:2 plus the 8 m apron"
        );
        assert_eq!(fill[1].blend_m, 8.0, "a pad with no fill at all");
        assert_eq!(
            rise_at_door(&fill),
            43,
            "the doorstep meets the natural ground"
        );
        assert!(
            (rise_at_door(&fill) as f64 / 1000.) <= super::super::access::WALKABLE_GRADE,
            "the step into the door is inside the 25% walkable contract"
        );

        // --- and the value is INERT without the switch: the very same world.
        let mut off = vec![
            entity("plate", "urban_building", 40, PLATE_TOP_M as i32, 24., 24.),
            entity("shop", "storefront", SHOP_Z, 0, 12., 10.),
        ];
        assert_eq!(
            stamp(
                &mut off,
                &json!({}),
                &std::collections::BTreeSet::new(),
                &natural
            ),
            0
        );
        assert!(off.iter().all(|e| e.grounding.earthwork_blend_m.is_none()));
        assert_eq!(rise_at_door(&plan_terrain_carves(&off)), 5_556);
    }
}
