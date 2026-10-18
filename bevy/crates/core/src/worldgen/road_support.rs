//! The graded road's bounded-cut support contract.
//!
//! A real hillside road is cut on its uphill side and filled on its downhill
//! side. [`super::bounded_cut`] owns the one cap and the per-cell formula; this
//! module owns the ONE decision the cap cannot make by itself: whether a graded
//! road's own support band can hold the 6 m roadbed fill contract after that cut.
//!
//! The probe runs at PLAN time, on the plan's own earthwork, through the SAME
//! composition the bake publishes (`worldgen::published_surface_from_raster` ->
//! `apply_terrain_carves_with` with the platform rule active). Every cell of the
//! road's transport band (`graded_transport_half_extents`: the pavement plus the
//! corridor's fixed shoulder) is measured against the natural raster:
//!
//! * a graded road owns its band's fill (its authored profile), so the historical
//!   overfill caused by a neighbouring pad's transition ring is gone;
//! * where the natural ground stands above the profile the road excavates, at
//!   most the one cap, and the surface is still the road's own profile/ring.
//!
//! A road that is still more than its `maxFillMm` above the natural ground after
//! that cut is refused BY NAME
//! ([`super::bounded_cut::ROAD_FILL_EXCEEDS_AFTER_CUT`]) and withdrawn from the
//! plan - its edge, its extension design, its registered water crossings and any
//! roadside pad that named it - so the published plan, the entity list, the
//! development plan and the cut ledger cannot disagree about a roadbed the
//! contract does not hold. The refusal record carries the measured fill and the
//! worst cell, the same numbers the road audit would otherwise report as a
//! generic failure.
//!
//! The probe is the PLAN's earthwork. A rule-engine or development structure is
//! placed against the roadbed by its own conflict gates
//! (`road_network::building_fill_conflicts`) and cannot stand a platform inside a
//! road band; a later structure can only *add* a support window, and the road
//! audit re-reads the published raster and fails by name if one ever did. The
//! probe therefore refuses every road the plan itself owns the means to judge.

use super::road_network::{BaseProfileRejection, RoadNetworkPlan};
use super::zoning::ZoningMask;
use super::{
    LandUseZone, SceneSpec, WaterGeometry, plan_terrain_carves,
    published_surface_from_raster,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};

/// How many refusals one probe may write before it stops: every round removes at
/// least one road, so the loop is bounded by the plan's own road count. The
/// constant only keeps a damaged plan from looping; it is never a policy number.
const MAX_ROAD_SUPPORT_ROUNDS: usize = 32;

/// The tolerance between the probe's composition and the millimetre raster: one
/// millimetre is the storage step, so a fill that lands exactly on the contract
/// (`6.000 m`) must not be refused by a rounding difference.
const FILL_MM_TOLERANCE: f64 = 0.001;

/// Refuse every graded road whose own bounded-cut support band still exceeds its
/// fill contract, and withdraw it from the plan by name.
pub(super) fn refuse_overfilled_roads(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    style: &serde_json::Value,
) -> Result<(), TerrainError> {
    let (policy, _) = super::bounded_cut::CutPolicy::from_style(style);
    if !policy.enabled() {
        return Ok(());
    }
    let mask = match zones {
        Some(zones) => Some(ZoningMask::new(scene, zones).map_err(|e| TerrainError(e.0))?),
        None => None,
    };
    let cap = policy.cap_mm();
    for _ in 0..MAX_ROAD_SUPPORT_ROUNDS {
        // The probe owns the cap so `plan.entities()` stamps it onto every graded
        // road; the published plan gains the same cap only after the frontage
        // programme has accepted its pads (see `plan_with_style_declared`).
        let mut probe = plan.clone();
        probe.road_cut_mm = Some(cap);
        let mut entities = probe.entities();
        entities.extend(probe.site_entities());
        let carves = plan_terrain_carves(&entities);
        let mut refused: Vec<(String, f64, [i32; 2], f64)> = Vec::new();
        for entity in probe.entities() {
            if entity.grounding.bounded_cut_mm != Some(cap) {
                continue;
            }
            let Some(profile) = entity.grounding.road_profile.as_ref() else {
                continue;
            };
            let max_fill_m = f64::from(profile.max_fill_mm) / 1000.;
            let (half_w, half_d) = super::graded_transport_half_extents(&entity);
            let mut worst = f64::NEG_INFINITY;
            let mut worst_at = [entity.world_x, entity.world_z];
            for z in entity.world_z - half_d..=entity.world_z + half_d {
                for x in entity.world_x - half_w..=entity.world_x + half_w {
                    let Some(natural) = terrain.height_millimetres(x, z) else {
                        continue;
                    };
                    let base = f64::from(natural) / 1000.;
                    let Some(surface) =
                        published_surface_from_raster(terrain, mask.as_ref(), water, &carves, x, z)
                    else {
                        continue;
                    };
                    let fill = surface - base;
                    if fill > worst {
                        worst = fill;
                        worst_at = [x, z];
                    }
                }
            }
            if worst > max_fill_m + FILL_MM_TOLERANCE {
                refused.push((
                    entity.entity_id.clone(),
                    worst,
                    worst_at,
                    max_fill_m,
                ));
            }
        }
        if refused.is_empty() {
            return Ok(());
        }
        for (road_id, fill_m, at, max_fill_m) in refused {
            withdraw_road(plan, &road_id);
            plan.base_profile_rejections.push(BaseProfileRejection {
                edge_id: road_id.clone(),
                category: super::bounded_cut::ROAD_FILL_EXCEEDS_AFTER_CUT.into(),
                reason: format!(
                    "road {road_id} is refused by name: after its own bounded cut its transport \
                     band still stands {:.3} m above the natural ground at ({},{}), above the \
                     {:.3} m roadbed fill contract (cap {} mm); the road, its extension design and \
                     any roadside pad that named it are withdrawn from the plan instead of \
                     publishing a roadbed the contract cannot hold",
                    fill_m, at[0], at[1], max_fill_m, cap
                ),
            });
        }
    }
    Err(TerrainError(
        "road support refusal did not converge; refusing to publish a half-withdrawn road plan"
            .into(),
    ))
}

/// Take one road out of every part of the plan that names it: its edge, the
/// extension design that carries its profile, its registered water crossing, the
/// roadside pads that named it and their service-program accounting.
fn withdraw_road(plan: &mut RoadNetworkPlan, road_id: &str) {
    plan.edges.retain(|edge| edge.id != road_id);
    plan.extensions
        .retain(|extension| extension.entity_id != road_id);
    plan.water_crossings
        .retain(|crossing| crossing.road_id != road_id);
    let withdrawn_sites: Vec<String> = plan
        .roadside_sites
        .iter()
        .filter(|site| site.road_id == road_id)
        .map(|site| site.id.clone())
        .collect();
    plan.roadside_sites
        .retain(|site| site.road_id != road_id);
    if withdrawn_sites.is_empty() {
        return;
    }
    for outcome in &mut plan.service_outcomes {
        let before = outcome.planned_site_ids.len();
        outcome
            .planned_site_ids
            .retain(|id| !withdrawn_sites.contains(id));
        let lost = before - outcome.planned_site_ids.len();
        if lost > 0 {
            outcome.unfilled_count = outcome.unfilled_count.saturating_add(lost as u32);
            outcome.status = "refused-road-withdrawn".into();
        }
    }
    if let Some(joint) = plan.joint_layout.as_mut() {
        joint
            .retained_residential_ids
            .retain(|id| !withdrawn_sites.contains(id));
        joint
            .replacement_residential_ids
            .retain(|id| !withdrawn_sites.contains(id));
        joint
            .displaced_residential_ids
            .retain(|id| !withdrawn_sites.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;

    /// One graded road (flat 10 m profile, 56 m long, 12 m wide) and ONE fill-only
    /// roadside structure whose support rectangle reaches into the road's transport
    /// band. `level_m` decides whether that structure's fill is inside the 6 m
    /// contract (`14`) or beyond it (`19`), which is exactly the refusal boundary.
    fn fixture(level_m: f64) -> (SceneSpec, Vec<LandUseZone>, RoadNetworkPlan, BakedTerrain, WaterGeometry) {
        let scene = SceneSpec {
            scene_id: "road-support".into(),
            origin_x: 0,
            origin_z: 0,
            width_m: 96,
            depth_m: 96,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 96,
            depth_m: 96,
            source: "test".into(),
        }];
        let plan: RoadNetworkPlan = serde_json::from_value(serde_json::json!({
            "model": "fixture",
            "naturalHeightfieldHash": "source",
            "sceneId": "road-support",
            "status": "planned",
            "gridSpacingM": 96,
            "maxRoadbedFillM": 6,
            "levelM": 10,
            "candidates": 1,
            "rejectedWater": 0,
            "rejectedRelief": 0,
            "unselectedCandidates": 0,
            "nodes": [
                {"id": 0, "worldX": 16, "worldZ": 40},
                {"id": 1, "worldX": 72, "worldZ": 40}
            ],
            "edges": [{
                "id": "road.0",
                "from": 0,
                "to": 1,
                "worldX": 44,
                "worldZ": 40,
                "lengthM": 56,
                "rotationYDeg": 0.0,
                "naturalMinM": 9.0,
                "naturalMaxM": 12.0,
                "profile": {
                    "version": 1,
                    "roadWidthM": 12,
                    "start": [16, 40],
                    "end": [72, 40],
                    "elevationsMm": vec![10_000_i64; 57],
                    "maxFillMm": 6000
                }
            }],
            "roadsideSites": [{
                "id": "sit-in-the-band",
                "roadId": "road.0",
                "worldX": 44,
                "worldZ": 24,
                "widthM": 12,
                "depthM": 12,
                "supportLevelM": level_m as i32,
                "supportLevelMm": (level_m * 1000.) as i32,
                "rotationYDeg": 0.0,
                "naturalMinM": 9.0,
                "naturalMaxM": 12.0
            }]
        }))
        .expect("the fixture plan parses");
        let mut values = vec![9.0_f64; 96 * 96];
        for z in 30..=34 {
            for x in 0..96 {
                values[z * 96 + x] = 12.0;
            }
        }
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(0, 0, 96, 96, &encode_millimetres(96, 96, values).unwrap())
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        (scene, zones, plan, natural, water)
    }

    fn style() -> serde_json::Value {
        serde_json::json!({
            "frontageProgram": {"terrainPolicy": "bounded-cut", "maxCutMm": 2000}
        })
    }

    /// Requirement 2: a road whose own band still stands above its 6 m fill
    /// contract after its bounded cut is refused BY NAME and withdrawn from the
    /// plan - edge, entity and the roadside pad that named it - not published.
    #[test]
    fn a_road_that_still_overfills_after_its_cut_is_refused_by_name() {
        let (scene, zones, mut plan, natural, water) = fixture(19.0);
        assert_eq!(plan.edges.len(), 1);
        refuse_overfilled_roads(
            &mut plan,
            &scene,
            &natural,
            water,
            Some(&zones),
            &style(),
        )
        .expect("the probe converges");
        assert!(plan.edges.is_empty(), "the road leaves the plan");
        assert!(plan.extensions.is_empty());
        assert!(plan.entities().is_empty(), "and its entity leaves with it");
        assert!(
            plan.roadside_sites.is_empty(),
            "the roadside pad that named the road is withdrawn too"
        );
        let refusal = plan
            .base_profile_rejections
            .iter()
            .find(|record| record.category == super::super::bounded_cut::ROAD_FILL_EXCEEDS_AFTER_CUT)
            .expect("the refusal is named and persisted");
        assert_eq!(refusal.edge_id, "road.0");
        assert!(
            refusal.reason.contains("7.000 m") && refusal.reason.contains("(37,31)"),
            "the refusal carries the measured fill and the worst cell: {}",
            refusal.reason
        );
        assert!(refusal.reason.contains("6.000 m roadbed fill contract"));
    }

    /// The same road with a neighbour whose fill is inside the contract is NOT
    /// refused: the named refusal is specific, not a blanket refusal of graded
    /// roads.
    #[test]
    fn a_road_whose_band_stays_inside_the_fill_contract_is_kept() {
        let (scene, zones, mut plan, natural, water) = fixture(14.0);
        refuse_overfilled_roads(
            &mut plan,
            &scene,
            &natural,
            water,
            Some(&zones),
            &style(),
        )
        .expect("the probe converges");
        assert_eq!(plan.edges.len(), 1, "the road stays");
        assert_eq!(plan.entities().len(), 1);
        assert_eq!(plan.roadside_sites.len(), 1);
        assert!(plan.base_profile_rejections.is_empty());
    }
}
