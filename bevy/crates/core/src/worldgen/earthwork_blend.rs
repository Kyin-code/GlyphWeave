//! OPT-IN: derive each structure's fill-transition ring from its own size.
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
//! # What this module derives instead
//!
//! `style.terrainPolicy.earthworkBlend = "derived-from-footprint"` (the only value
//! that turns it on) replaces that ring, per structure, with
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
//! # Planned pads: the ring travels in the plan record
//!
//! A pad the development plan owns is not stamped on its entity alone. Its
//! record carries the ring ([`stamp_plan`] -> [`RoadsideSite::earthwork_blend_m`])
//! and the entity reads it back out ([`RoadsideSite::entity`]), because three
//! independent readers rebuild that entity from the road plan alone and compare
//! it with the published one byte for byte
//! (`settlement::development::verify`, the plan re-derivation inside the
//! completion pass, and the rules-mode ownership audit). One ring, written once,
//! in the one place both sides read.
//!
//! Nothing else changes: the pad's flat top, the 6 m fill cap, the 8 %/25 % grades,
//! the 2,000 mm cut cap, the setbacks, the shore band and the protected land and
//! water are all untouched, and every carve that does not carry a derived ring
//! (roads, and every entity of a world that did not opt in) keeps the historical
//! bytes.

use super::bounded_cut;

/// The authored switch key inside `style.terrainPolicy`.
pub const POLICY_KEY: &str = "earthworkBlend";
/// The one authored value that turns the derivation on.
pub const POLICY_VALUE: &str = "derived-from-footprint";

/// Metres of transition run per metre of the structure's largest dimension.
///
/// `access::WALKABLE_GRADE` (the yard grade) over `bounded_cut::MAX_CUT_RING_GRADE`
/// (the ring grade): two of the project's three existing slope contracts, both
/// 0.25, so the ring is as wide as the structure is large.
pub const BLEND_RUN_PER_METRE: f64 = super::access::WALKABLE_GRADE / bounded_cut::MAX_CUT_RING_GRADE;

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

/// Is `style.terrainPolicy.earthworkBlend` the one accepted value?
///
/// Only `"derived-from-footprint"` opts in. Absent, `null`, a non-string or any
/// other spelling is the historical fixed-envelope earthwork, never an error: a
/// world that never asked must keep its published bytes exactly.
pub fn enabled(style: &serde_json::Value) -> bool {
    style
        .get("terrainPolicy")
        .and_then(|policy| policy.get(POLICY_KEY))
        .and_then(serde_json::Value::as_str)
        == Some(POLICY_VALUE)
}

/// The derived fill-transition ring of one structure, in metres.
///
/// `max(width_m, height_m) * K + M`, clamped into
/// `BLEND_FLOOR_M ..= BLEND_CEILING_M`.
pub fn derived_blend_m(width_m: f32, height_m: f32) -> f64 {
    let largest = f64::from(width_m).max(f64::from(height_m));
    (largest * BLEND_RUN_PER_METRE + BLEND_APRON_M).clamp(BLEND_FLOOR_M, BLEND_CEILING_M)
}

/// Stamp every structure of one world with its derived ring, and answer how many
/// were stamped.
///
/// Inert (and byte-inert) unless the style asked for it. Only the kinds
/// `plan_terrain_carves` actually carves as a BUILDING are stamped: a road keeps
/// its graded corridor envelope (`6 m` flat, `18 m` graded, and its own fill
/// contract), and a structure deck is carried by an engineering structure and gets
/// no carve at all.
///
/// `planned` is the set of entity ids the DEVELOPMENT PLAN owns. Those are
/// stamped by [`stamp_plan`] instead, before the plan record is published: their
/// record carries the ring, so the rebuild that derives the entity from the road
/// plan alone (`settlement::development::plan`) reads the same number and the
/// plan/entity byte comparison stays exact. A planned pad that the world carries
/// WITHOUT a record ring (a world that did not opt in) gets none here either,
/// because `stamp` never touches a planned id. Every other structure - the rule
/// engine's `urban_building`, `resort_lodge`, and the rest of
/// `carves_as_building` - is stamped here.
pub fn stamp(
    entities: &mut [super::EntityInstance],
    style: &serde_json::Value,
    planned: &std::collections::BTreeSet<String>,
) -> usize {
    let enabled = enabled(style);
    let mut stamped = 0;
    for entity in entities.iter_mut() {
        if planned.contains(&entity.entity_id) {
            continue;
        }
        if stamp_entity(entity, enabled) {
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

/// One entity, once. Answers whether it carries a derived ring now.
pub fn stamp_entity(entity: &mut super::EntityInstance, enabled: bool) -> bool {
    if !enabled || entity.grounding.structure_deck || !super::carves_as_building(&entity.kind) {
        return false;
    }
    entity.grounding.earthwork_blend_m =
        Some(derived_blend_m(entity.width_m, entity.height_m) as f32);
    true
}

/// The derived fill-transition ring of ONE PUBLISHED PAD, in metres.
///
/// The same formula, read off the two numbers the pad's realized entity carries
/// (`RoadsideSite::entity`: `width_m` and `height_m`), so a pad's record and its
/// entity can never publish two different rings.
pub fn site_ring_m(site: &super::road_network::RoadsideSite) -> f32 {
    derived_blend_m(site.width_m as f32, site.height_m()) as f32
}

/// Stamp every pad of one plan with the ring its own footprint derives, and
/// answer how many were stamped.
///
/// This is the PLAN-RECORD half of [`stamp_entity`], and the reason the opt-in
/// can cover a planned pad at all. `stamp` above deliberately skips the pads the
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
/// The predicate is `stamp_entity`'s, applied to the pad's own kind: a pad whose
/// kind `plan_terrain_carves` does not carve as a building would never read the
/// number, so it does not gain one.
pub fn stamp_plan(
    plan: &mut super::road_network::RoadNetworkPlan,
    style: &serde_json::Value,
) -> usize {
    if !enabled(style) {
        return 0;
    }
    let mut stamped = 0;
    for site in plan.roadside_sites.iter_mut() {
        if !super::carves_as_building(site.kind()) {
            continue;
        }
        site.earthwork_blend_m = Some(site_ring_m(site));
        stamped += 1;
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
        ] {
            assert!(!enabled(&style), "{style} must not opt in");
        }
        assert!(enabled(
            &json!({"terrainPolicy": {"earthworkBlend": "derived-from-footprint"}})
        ));
    }

    /// 2. The derived ring IS the formula, for the sizes this project authors.
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
        assert_eq!(BLEND_FLOOR_M, bounded_cut::ring_width_m(bounded_cut::MAX_CUT_MM));
        assert_eq!(BLEND_CEILING_M, 3.0 * (16.0 + 32.0));
    }

    /// 4. The same world buries fewer pad cells after the switch: the hillside
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
        assert!(stamp_entity(&mut derived, true));
        assert_eq!(derived.grounding.earthwork_blend_m, Some(BLEND_APRON_M as f32 + 25.));
        assert_eq!(bury(&plan_terrain_carves(std::slice::from_ref(&derived))), (0, 0));
        // And the switch is inert when it is off: nothing is stamped, and the
        // carve is the legacy one, bit for bit.
        let mut off = building.clone();
        assert!(!stamp_entity(&mut off, false));
        assert!(off.grounding.earthwork_blend_m.is_none());
        assert_eq!(
            plan_terrain_carves(std::slice::from_ref(&off))[0].blend_m,
            BLEND_CEILING_M
        );
    }
}
