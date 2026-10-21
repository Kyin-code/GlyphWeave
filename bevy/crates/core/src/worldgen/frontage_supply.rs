//! Bounded, read-only reuse of the production frontage candidate and
//! engineering preflight path.
//!
//! This module never places anything itself: it calls the same
//! `frontage::stations`, `frontage::candidate_site`, `frontage::envelope_reason`
//! and `frontage::PlacementState::assess` the bake uses, so a measured supply
//! can never drift from the placement it predicts. Two consumers exist: a
//! bounded diagnosis of an already-planned network, and the real-supply score
//! used by the explicitly enabled demand-led road selection.
use super::{
    EntityInstance, LandUseZone, SceneSpec, WaterGeometry,
    frontage::{self, PlacementState, RejectDetail, Request, RequestBudget},
    road_network::{RoadNetworkPlan, RoadsideSite},
    zoning::ZoningMask,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Test-only switch: corrupt the candidate's own engineered final raster under
/// one planned parcel of the named scene, so a *real* realization audit
/// (landcover yard / access graph) refuses it and the transaction can be
/// observed rolling back. Compiled out of production builds, and scoped to one
/// scene id so parallel tests are unaffected.
#[cfg(test)]
pub(super) static TEST_CORRUPT_SCENE: std::sync::Mutex<Option<String>> =
    std::sync::Mutex::new(None);

/// Test-only observation of what the production supply entry actually handed to
/// the final encoder. Recorded from inside `candidate_final_terrain`, so a test
/// observes the real call path instead of passing the value itself; appended per
/// call so the baseline realization and every candidate measurement are visible.
/// It observes only and never influences a result.
#[cfg(test)]
pub(super) static OBSERVED_FINAL_CONTEXT: std::sync::Mutex<Vec<(String, u64, bool, &'static str)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(test)]
pub(super) fn observe_final_context(
    scene_id: &str,
    seed: u64,
    precise: bool,
    source: &'static str,
) {
    if let Ok(mut observed) = OBSERVED_FINAL_CONTEXT.lock() {
        observed.push((scene_id.to_string(), seed, precise, source));
    }
}

/// Outstanding demand for one scene, with the cumulative per-request budgets
/// production placement uses. The baseline is derived from the plan's trusted
/// realized set (recorded outcome plus program sites that already exist), never
/// hardcoded to zero, and every road in one pass shares these budgets so demand
/// and station attempts accumulate across roads instead of restarting.
#[derive(Debug, Clone)]
pub(super) struct SupplySession {
    pub budgets: Vec<RequestBudget>,
    /// Rolling commitments: the real geometry of every currently realized
    /// frontage site, updated atomically after each verified success so later
    /// rounds protect the identities this same phase already committed.
    pub committed: Vec<RoadsideSite>,
    pub commitments: BTreeMap<String, serde_json::Value>,
    /// Evidence only: the last verified frontage report of this session.
    pub verified_report: Option<frontage::Report>,
    /// Every parcel that existed before this phase, with the yard shortfall the
    /// trusted baseline realization already carried on it. This is the full
    /// pre-existing asset set — all development parcels, not only the frontage
    /// requests — so the authored obligations are protected without being
    /// counted as new supply. Values never rise once recorded.
    pub protected_parcels: BTreeMap<String, f64>,
    /// Entity id -> owning parcel id for the same pre-existing assets. The
    /// first recorded owner wins, so a later round cannot silently re-own a
    /// baseline asset.
    pub protected_owners: BTreeMap<String, String>,
}

impl SupplySession {
    pub(super) fn empty() -> Self {
        Self {
            budgets: vec![],
            committed: vec![],
            commitments: BTreeMap::new(),
            verified_report: None,
            protected_parcels: BTreeMap::new(),
            protected_owners: BTreeMap::new(),
        }
    }

    /// Adopt exactly the realized-with-access identities of one verified plan.
    /// Ownership comes from the structured `realized_ids_by_request` mapping —
    /// never from parsing the id string — and every id must be named by exactly
    /// the request its own program claims. The update is atomic: any error
    /// leaves the previous commitments untouched.
    pub(super) fn adopt(
        &mut self,
        verified: &RoadNetworkPlan,
        realization: &Realization,
    ) -> Result<(), String> {
        let mut next_committed: Vec<RoadsideSite> = vec![];
        let mut next_commitments: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        let mut next_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for id in &realization.realized_ids {
            let owner = realization
                .realized_ids_by_request
                .iter()
                .find(|(_, ids)| ids.iter().any(|value| value == id))
                .map(|(request, _)| request.clone())
                .ok_or_else(|| format!("realized id {id} is not attributed to any request"))?;
            let site = verified
                .roadside_sites
                .iter()
                .find(|site| &site.id == id)
                .ok_or_else(|| format!("realized id {id} has no site in the verified plan"))?;
            let program = site
                .program
                .as_ref()
                .ok_or_else(|| format!("realized id {id} has no program"))?;
            if program.request_id != owner {
                return Err(format!(
                    "realized id {id} is attributed to {owner} but belongs to {}",
                    program.request_id
                ));
            }
            let mut entity = site.entity();
            entity.normalize_spatial_semantics();
            entity.bounds = None;
            next_commitments.insert(
                id.clone(),
                serde_json::to_value(&entity).map_err(|e| e.to_string())?,
            );
            next_committed.push(site.clone());
            next_ids.entry(owner).or_default().push(id.clone());
        }
        for budget in self.budgets.iter_mut() {
            budget.site_ids = next_ids
                .get(budget.request.id.as_str())
                .cloned()
                .unwrap_or_default();
        }
        // Protect the full asset set of this verified stage: every parcel it
        // realizes, with its own shortfall, and the ownership that produced it.
        // `or_insert` keeps the earliest (entering-baseline) owner and allowance,
        // so a later round can neither re-own a parcel nor enlarge the
        // shortfall it is already allowed to carry.
        for (entity_id, parcel) in &realization.parcel_by_entity {
            self.protected_owners
                .entry(entity_id.clone())
                .or_insert_with(|| parcel.clone());
            let shortfall = realization
                .yard_unresolved_by_parcel
                .get(parcel)
                .copied()
                .unwrap_or(0.);
            self.protected_parcels
                .entry(parcel.clone())
                .or_insert(shortfall);
        }
        self.committed = next_committed;
        self.commitments = next_commitments;
        self.verified_report = verified.frontage.clone();
        Ok(())
    }

    /// The real geometry later candidates must treat as occupied ground.
    pub(super) fn occupied(&self) -> Vec<RoadsideSite> {
        self.committed.clone()
    }

    pub(super) fn for_scene(
        style: &serde_json::Value,
        scene_id: &str,
        planned: &RoadNetworkPlan,
    ) -> Result<Self, TerrainError> {
        let Some(program) = frontage::from_style(style)? else {
            return Ok(Self::empty());
        };
        let mut budgets = vec![];
        for request in program
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene_id)
        {
            let mut budget = RequestBudget::new(&request);
            let mut realized: BTreeSet<String> = BTreeSet::new();
            if let Some(report) = &planned.frontage {
                for outcome in report
                    .outcomes
                    .iter()
                    .filter(|o| o.request.id == request.id)
                {
                    realized.extend(outcome.site_ids.iter().cloned());
                }
            }
            for site in &planned.roadside_sites {
                if site
                    .program
                    .as_ref()
                    .is_some_and(|p| p.request_id == request.id)
                {
                    realized.insert(site.id.clone());
                }
            }
            for id in realized {
                budget.accept(&id);
            }
            budgets.push(budget);
        }
        Ok(Self {
            budgets,
            committed: vec![],
            commitments: BTreeMap::new(),
            verified_report: None,
            protected_parcels: BTreeMap::new(),
            protected_owners: BTreeMap::new(),
        })
    }

    /// Realized totals per request id, in program order.
    pub(super) fn realized_totals(&self) -> Vec<(String, usize)> {
        self.budgets
            .iter()
            .map(|b| (b.request.id.clone(), b.site_ids.len()))
            .collect()
    }

    pub(super) fn remaining_total(&self) -> usize {
        self.budgets.iter().map(|b| b.remaining() as usize).sum()
    }
}

/// One tried station/side with the reason it did or did not become a parcel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CandidateSample {
    pub request_id: String,
    pub road_id: String,
    pub station: usize,
    pub side: i32,
    pub world_x: i32,
    pub world_z: i32,
    pub parcel: [i64; 4],
    pub purpose: String,
    pub design_level_mm: Option<i64>,
    pub natural_min_mm: Option<i32>,
    pub natural_max_mm: Option<i32>,
    pub outcome: String,
}

/// The bounded per-(request, road) evidence of one production candidate run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RoadSupply {
    pub request_id: String,
    pub road_id: String,
    pub road_width_m: u32,
    pub profile_length_m: usize,
    pub margin_m: u32,
    pub station_start: usize,
    pub station_count: usize,
    pub attempts_budget: u32,
    pub attempted: u32,
    pub accepted: usize,
    /// Accepted site identities in acceptance order, so a diagnosis can be
    /// compared with the production selection identity-for-identity.
    pub accepted_ids: Vec<String>,
    pub geometry_exhausted: bool,
    pub rejected: BTreeMap<String, u32>,
    pub rejection_details: Vec<RejectDetail>,
    pub samples: Vec<CandidateSample>,
}

/// A bounded measurement of ONE road for ONE request: the production
/// enumeration driven by that request's shared cumulative budget, plus the
/// accepted sites in acceptance order for the caller to commit.
#[derive(Debug, Clone)]
pub(super) struct RoadMeasurement {
    pub trace: RoadSupply,
    pub accepted: Vec<RoadsideSite>,
}

fn natural_range(
    n: &BakedTerrain,
    parcel: [i64; 4],
) -> Result<(Option<i32>, Option<i32>), TerrainError> {
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for z in parcel[1]..parcel[3] {
        for x in parcel[0]..parcel[2] {
            // A parcel that reaches past the scene edge has no source sample;
            // the envelope stage already refuses it, so evidence sampling must
            // tolerate the gap instead of failing the whole phase.
            let Some(h) = n.height_millimetres(x as i32, z as i32) else {
                continue;
            };
            min = min.min(h);
            max = max.max(h);
        }
    }
    Ok(if min > max {
        (None, None)
    } else {
        (Some(min), Some(max))
    })
}

/// The accepted joint realization of one candidate plan, measured on the
/// candidate's own engineered final raster. Every step is an existing
/// production leaf (`generate_chunk_encoded`, `plan_terrain_carves`,
/// `road_parcel_feasibility::audit_joint_plan`, `frontage::audit`); nothing
/// here is a second, weaker audit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Realization {
    pub frontage_requested: u32,
    pub planned: usize,
    pub realized_with_access: usize,
    pub realized_ids: Vec<String>,
    pub realized_by_request: Vec<(String, usize)>,
    /// Realized-with-access ids grouped by the request that owns them.
    pub realized_ids_by_request: Vec<(String, Vec<String>)>,
    pub yard_unresolved_m2: f64,
    /// Additive per-parcel yard shortfall from the accepted landcover audit, so
    /// new parcels cannot spend an old parcel's shortfall.
    pub yard_unresolved_by_parcel: BTreeMap<String, f64>,
    /// Entity id -> owning parcel id, for the same accounting.
    pub parcel_by_entity: BTreeMap<String, String>,
    pub unresolved_area_m2: f64,
    pub parcels: usize,
    pub buildings_realized: usize,
    pub planned_buildings: usize,
    pub road_edges: usize,
    pub extension_count: usize,
    pub frontage_status: String,
    pub development_status: String,
    pub landcover_status: String,
    pub program_landcover_status: String,
    pub frontage_failures: Vec<String>,
    pub development_failures: Vec<String>,
    pub landcover_failures: Vec<String>,
    /// Per-road engineering status on the candidate's own final raster, using
    /// the production bands, fill cap and no-excavation rule.
    pub road_status: String,
    pub road_failures: Vec<String>,
    pub graded_roads: usize,
    pub node_status: String,
    pub required_unconnected: usize,
    pub required_unconnected_ids: Vec<String>,
    /// Which authoritative source produced the audited final raster, taken from
    /// the caller's declaration rather than inferred: `production-encoder` (the
    /// bake's own chunk encoder on the declared authored seed and precision) or
    /// `supplied-raster-carve` (the identical carve composition on the raster
    /// handed over by the caller).
    pub final_source: String,
    pub seconds: f64,
}

/// Build the candidate's own engineered final raster exactly the way the bake
/// does: the production chunk encoder on the authored seed, scene origin and
/// zoning mask, with the candidate's own carve list and millimetre precision.
/// One construction, one quantization order, shared with the production audit.
/// The authoritative source is a *declared* input ([`super::NaturalSource`]);
/// nothing here samples the raster to guess where it came from. An
/// `AuthoredSeed` declaration regenerates the field through the production
/// encoder, a `SuppliedRaster` declaration consumes the handed-over raster
/// exactly as given, and a missing declaration is refused.
pub(super) fn candidate_final_terrain(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    plan: &RoadNetworkPlan,
    source: super::NaturalSource,
) -> Result<(BakedTerrain, &'static str), String> {
    #[cfg(test)]
    let corrupt = TEST_CORRUPT_SCENE
        .lock()
        .map(|guard| guard.as_deref() == Some(scene.scene_id.as_str()))
        .unwrap_or(false);
    let mut entities = plan.entities();
    entities.extend(plan.site_entities());
    for entity in &mut entities {
        entity.normalize_spatial_semantics();
    }
    let carves = super::plan_terrain_carves(&entities);
    let (height, source_label) = match source {
        super::NaturalSource::AuthoredSeed { seed, precise } => {
            #[cfg(test)]
            observe_final_context(&scene.scene_id, seed, precise, "production-encoder");
            let mask = ZoningMask::new(scene, zones).map_err(|e| e.to_string())?;
            let height = super::generate_chunk_encoded(
                seed,
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                water,
                &carves,
                precise,
                Some(&mask),
            )
            .0;
            (height, "production-encoder")
        }
        super::NaturalSource::SuppliedRaster => {
            #[cfg(test)]
            observe_final_context(&scene.scene_id, 0, false, "supplied-raster-carve");
            let mut values = Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
            for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
                for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                    let base = f64::from(
                        natural
                            .height(x, z)
                            .ok_or_else(|| "candidate natural sample missing".to_string())?,
                    );
                    values.push(super::apply_terrain_carves(base, x, z, &carves));
                }
            }
            (
                crate::height_payload::encode_millimetres(scene.width_m, scene.depth_m, values)
                    .map_err(|e| e.to_string())?,
                "supplied-raster-carve",
            )
        }
        super::NaturalSource::Undeclared => {
            return Err(
                "candidate final terrain has no declared natural source (authored seed or supplied raster)"
                    .to_string(),
            )
        }
    };
    let mut final_terrain = BakedTerrain::new(scene).map_err(|e| e.to_string())?;
    let height = {
        #[cfg(test)]
        if corrupt {
            // Bury one planned parcel's pad: the accepted landcover yard check
            // and the access graph must then refuse the candidate.
            let victim = plan
                .roadside_sites
                .iter()
                .filter(|site| site.program.is_some())
                .max_by_key(|site| site.id.clone());
            if let Some(victim) = victim {
                let mut values: Vec<f64> =
                    Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
                for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
                    for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                        let base = f64::from(
                            natural
                                .height(x, z)
                                .ok_or_else(|| "candidate natural sample missing".to_string())?,
                        );
                        let carved = super::apply_terrain_carves(base, x, z, &carves);
                        let inside =
                            (x - victim.world_x).abs() <= 3 && (z - victim.world_z).abs() <= 3;
                        values.push(if inside { carved + 1.5 } else { carved });
                    }
                }
                crate::height_payload::encode_millimetres(scene.width_m, scene.depth_m, values)
                    .map_err(|e| e.to_string())?
            } else {
                height
            }
        } else {
            height
        }
        #[cfg(not(test))]
        height
    };
    final_terrain
        .insert_chunk(
            scene.origin_x,
            scene.origin_z,
            scene.width_m,
            scene.depth_m,
            &height,
        )
        .map_err(|e| e.to_string())?;
    Ok((final_terrain, source_label))
}

/// Realized-with-access stable IDs of the plan's frontage sites, derived with
/// the production access graph on the candidate's final raster and the
/// production audit identity rule. Only these may earn supply credit.
pub(super) fn realized_frontage_ids(
    scene: &SceneSpec,
    water: WaterGeometry,
    plan: &RoadNetworkPlan,
    final_terrain: &BakedTerrain,
) -> Result<
    (
        Vec<String>,
        Vec<(String, usize)>,
        Vec<(String, Vec<String>)>,
    ),
    String,
> {
    let mut entities = plan.entities();
    entities.extend(plan.site_entities());
    let access = super::build_access_graph(&entities, final_terrain, water);
    let audit = frontage::audit(plan, &entities, &access);
    let mut ids = vec![];
    let mut by_request: Vec<(String, usize)> = vec![];
    let mut ids_by_request: Vec<(String, Vec<String>)> = vec![];
    if let Some(report) = &plan.frontage {
        for outcome in &report.outcomes {
            let mut realized = 0_usize;
            let mut realized_ids_for_request = vec![];
            for id in &outcome.site_ids {
                if audit.realized_ids.iter().any(|value| value == id) {
                    ids.push(id.clone());
                    realized_ids_for_request.push(id.clone());
                    realized += 1;
                }
            }
            by_request.push((outcome.request.id.clone(), realized));
            ids_by_request.push((outcome.request.id.clone(), realized_ids_for_request));
        }
    }
    let _ = scene;
    Ok((ids, by_request, ids_by_request))
}

/// Full realized evidence for one candidate plan: engineered final raster,
/// the accepted joint audit (road support, development, yard surfaces, access
/// graph) and the production program-bound landcover consumer.
#[allow(clippy::too_many_arguments)]
pub(super) fn realize_candidate(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    plan: &RoadNetworkPlan,
    source: super::NaturalSource,
    trusted: &super::road_parcel_feasibility::TrustedBaseContract,
    style: &serde_json::Value,
) -> Result<Realization, String> {
    let started = std::time::Instant::now();
    let (final_terrain, final_source) =
        candidate_final_terrain(scene, natural, water, zones, plan, source)?;
    let mut entities = plan.entities();
    entities.extend(plan.site_entities());
    for entity in &mut entities {
        entity.normalize_spatial_semantics();
    }
    let access = super::build_access_graph(&entities, &final_terrain, water);
    let frontage_audit = frontage::audit(plan, &entities, &access);
    let (realized_ids, realized_by_request, realized_ids_by_request) =
        realized_frontage_ids(scene, water, plan, &final_terrain)?;
    // Development ownership and its audit.
    let (development_status, development_failures, planned_buildings, buildings_realized) =
        match crate::settlement::development::plan(scene, zones, plan) {
            Ok(development) => {
                match crate::settlement::development::verify(
                    &development,
                    scene,
                    zones,
                    plan,
                    &entities,
                    &access,
                    &[],
                ) {
                    Ok(audit) => (
                        audit.status.clone(),
                        audit.failures.clone(),
                        audit.planned_buildings,
                        audit.realized_buildings,
                    ),
                    Err(error) => (format!("development_verify_rejected:{error}"), vec![], 0, 0),
                }
            }
            Err(error) => (format!("development_plan_rejected:{error}"), vec![], 0, 0),
        };
    // Landcover: the program-bound plan the bake consumes, then the real
    // surface realization and its independent verification (yard shortfall).
    let mut yard_unresolved_m2 = 0.0;
    let mut unresolved_area_m2 = 0.0;
    let mut yard_unresolved_by_parcel: BTreeMap<String, f64> = BTreeMap::new();
    let mut parcel_by_entity: BTreeMap<String, String> = BTreeMap::new();
    let mut landcover_status = "unplanned".to_string();
    let mut landcover_failures = vec![];
    let mut program_landcover_status = "unplanned".to_string();
    let mut parcels = 0_usize;
    if let Ok(development) = crate::settlement::development::plan(scene, zones, plan) {
        parcels = development.parcels.len();
        for building in &development.buildings {
            parcel_by_entity.insert(
                building.entity.entity_id.clone(),
                building.parcel_id.clone(),
            );
        }
        match crate::settlement::development::plan_landcover_with_program(
            scene,
            &development,
            style,
        ) {
            Ok(landcover) => {
                program_landcover_status = "pass".into();
                match crate::settlement::landcover::realize(
                    scene,
                    &development,
                    &landcover,
                    &final_terrain,
                    water,
                    &entities,
                ) {
                    Ok(surfaces) => {
                        match crate::settlement::landcover::verify(
                            &surfaces,
                            scene,
                            &development,
                            &landcover,
                            &final_terrain,
                            water,
                            &entities,
                        ) {
                            Ok(audit) => {
                                landcover_status = audit.status.clone();
                                yard_unresolved_m2 = audit.unresolved_yard_m2;
                                unresolved_area_m2 = audit.unresolved_area_m2;
                                yard_unresolved_by_parcel = audit.unresolved_yard_by_parcel.clone();
                                landcover_failures = audit.failures.clone();
                            }
                            Err(error) => {
                                landcover_status = format!("landcover_verify_rejected:{error}")
                            }
                        }
                    }
                    Err(error) => landcover_status = format!("landcover_realize_rejected:{error}"),
                }
            }
            Err(error) => program_landcover_status = format!("program_landcover_rejected:{error}"),
        }
    }
    // Node elevations and per-road engineering on the candidate's final raster.
    // The road audit is the *production* audit body (`verify_with_style`'s own
    // implementation, entered without re-planning because the caller commits to
    // the recorded plan), plus the explicit extension source rebuild the
    // production re-plan would otherwise perform.
    let node_status = match super::road_parcel_feasibility::audit_node_heights(plan, trusted) {
        Ok(_) => "pass".to_string(),
        Err(error) => format!("node_height_rejected:{error}"),
    };
    let mut road_failures =
        super::road_network::audit_extension_provenance(plan, scene, natural, water, zones)
            .map_err(|e| e.to_string())?;
    let road_audit = super::road_network::audit_final_support_for_record(
        plan,
        scene,
        natural,
        &final_terrain,
        water,
        &entities,
        Some(zones),
        None,
        style,
    )
    .map_err(|e| e.to_string())?;
    let graded_roads = plan
        .entities()
        .iter()
        .filter(|e| e.grounding.road_profile.is_some())
        .count();
    road_failures.extend(road_audit.failures.iter().cloned());
    if road_audit.realized_edges != plan.edges.len() {
        road_failures.push(format!(
            "road audit realized {} of {} planned edges",
            road_audit.realized_edges,
            plan.edges.len()
        ));
    }
    let road_status = if road_failures.is_empty() {
        "pass".to_string()
    } else {
        "fail".to_string()
    };
    Ok(Realization {
        frontage_requested: frontage_audit.requested,
        planned: frontage_audit.planned,
        realized_with_access: frontage_audit.realized_with_access,
        realized_ids,
        realized_by_request,
        realized_ids_by_request,
        yard_unresolved_m2,
        yard_unresolved_by_parcel,
        parcel_by_entity,
        unresolved_area_m2,
        parcels,
        buildings_realized,
        planned_buildings,
        road_edges: plan.edges.len(),
        extension_count: plan.extensions.len(),
        frontage_status: frontage_audit.status,
        development_status,
        landcover_status,
        program_landcover_status,
        frontage_failures: frontage_audit.failures,
        development_failures,
        landcover_failures,
        road_status,
        road_failures,
        graded_roads,
        node_status,
        required_unconnected: access
            .links
            .iter()
            .filter(|l| l.required && !l.connected)
            .count(),
        required_unconnected_ids: access
            .links
            .iter()
            .filter(|l| l.required && !l.connected)
            .map(|l| l.entity_id.clone())
            .collect(),
        final_source: final_source.to_string(),
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Engineering state of one plan version: roads, already-obligated sites and
/// their fixed platforms. Everything measured through it is read-only.
pub(super) struct SupplyContext<'a> {
    pub scene: &'a SceneSpec,
    pub terrain: &'a BakedTerrain,
    pub mask: &'a ZoningMask,
    pub water: WaterGeometry,
    pub zones: &'a [LandUseZone],
    /// The world's OPT-IN bounded-cut policy, read from the same authored style
    /// `frontage::append` reads it from, so a read-only measurement predicts the
    /// production decision instead of a policy the bake never used.
    pub policy: super::bounded_cut::CutPolicy,
}

/// The per-(road, request) evidence header of one measurement. The station
/// bounds come from the shared schedule, so the reported geometry is the one the
/// candidate search actually visits.
fn road_supply_header(
    road: &EntityInstance,
    r: &Request,
    stations: Option<&[usize]>,
) -> RoadSupply {
    RoadSupply {
        request_id: r.id.clone(),
        road_id: road.entity_id.clone(),
        road_width_m: road.depth_m as u32,
        profile_length_m: road
            .grounding
            .road_profile
            .as_ref()
            .map(|p| p.elevations_mm.len() - 1)
            .unwrap_or(0),
        margin_m: r.width_m / 2 + 8,
        station_start: stations.and_then(|list| list.first().copied()).unwrap_or(0),
        station_count: stations.map_or(0, <[usize]>::len),
        attempts_budget: r.max_candidate_attempts,
        attempted: 0,
        accepted: 0,
        accepted_ids: vec![],
        geometry_exhausted: true,
        rejected: BTreeMap::new(),
        rejection_details: vec![],
        samples: vec![],
    }
}

impl<'a> SupplyContext<'a> {
    pub(super) fn state(&self, plan: &RoadNetworkPlan) -> PlacementState<'a> {
        // Production builds its engineering state from `profile_roads`, which
        // stable-sorts wider roads before local roads. Replaying an unsorted
        // `plan.entities()` changes which existing support a candidate reaches
        // first and can therefore rename a rejection without changing the
        // candidate geometry. Keep the diagnosis on the exact production order.
        let mut existing = frontage::profile_roads(plan);
        existing.extend(plan.site_entities());
        PlacementState::new(self.terrain, self.mask, self.water, existing, self.policy)
    }

    /// Try ONE candidate turn and record the production decision on the request's
    /// cumulative budget and on its evidence trace. The accepted site is returned
    /// so the caller can commit it to its own plan view; `state` is advanced here,
    /// exactly as the placement loop does, so every later candidate sees the new
    /// footprint. `sample_cap` only limits how much evidence is kept.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn attempt_turn(
        &self,
        plan: &RoadNetworkPlan,
        state: &mut PlacementState<'_>,
        road: &EntityInstance,
        r: &Request,
        station: usize,
        side: i32,
        budget: &mut RequestBudget,
        trace: &mut RoadSupply,
        sample_cap: usize,
    ) -> Result<Option<RoadsideSite>, TerrainError> {
        if !budget.claim_attempt() {
            return Ok(None);
        }
        trace.attempted += 1;
        let mut site = frontage::candidate_site(road, r, &self.scene.scene_id, station, side)?;
        let parcel = frontage::parcel_boundary(&site)?;
        // The same gate production uses, including this request's along-road purpose
        // conversion, so a measured supply can never disagree with the placement it predicts.
        // The converted AREA is discarded here: this measurement is read-only and the bake's
        // own `append` is the only authority that may publish a conversion record.
        let (envelope, _converted_m2) = frontage::envelope_verdict(
            parcel,
            r.parcel_insets.unwrap_or_default(),
            site.rotation_y_deg,
            self.scene,
            self.zones,
            &r.purpose,
            r.rural_conversion,
        );
        let (reason, detail) = match envelope {
            Some(reason) => (Some(reason), None),
            None => state.assess_detailed(&site, plan)?,
        };
        let (natural_min_mm, natural_max_mm) = natural_range(self.terrain, parcel)?;
        let mut accepted = false;
        let outcome = match reason {
            Some(reason) => {
                budget.reject(reason);
                *trace.rejected.entry(reason.into()).or_default() += 1;
                if trace.rejection_details.len() < sample_cap {
                    trace
                        .rejection_details
                        .push(detail.unwrap_or_else(|| RejectDetail::envelope(reason, parcel)));
                }
                reason.to_string()
            }
            None => {
                // Production records the pad's own cut cap BEFORE committing it (`frontage::
                // append`), because the cap is what publishes the footprint every later candidate is
                // checked against. A replay that committed the parcel without it would silently stop
                // applying the bench-overlap gate, refuse the same candidates under a different name
                // and report a search the bake never performed.
                //
                // Sampling the parcel is part of the committed record, not reporting
                // metadata: production writes `naturalMinM/naturalMaxM` before the
                // entity is committed, and those grounding fields are read by later
                // engineering checks.
                state.sample_natural(&mut site, parcel)?;
                site.bounded_cut_mm = state.cut_cap_for(&site);
                state.commit(&site);
                budget.accept(&site.id);
                trace.accepted += 1;
                trace.accepted_ids.push(site.id.clone());
                accepted = true;
                "accepted".into()
            }
        };
        if trace.samples.len() < sample_cap {
            trace.samples.push(CandidateSample {
                request_id: r.id.clone(),
                road_id: road.entity_id.clone(),
                station,
                side,
                world_x: site.world_x,
                world_z: site.world_z,
                parcel,
                purpose: r.purpose.clone(),
                // R7-1.4: fall back to the integer-metre support instead of
                // handing the supply chain a bare None (which read as 0 mm).
                design_level_mm: Some(i64::from(
                    site.support_level_mm
                        .unwrap_or_else(|| site.support_level_m * 1000),
                )),
                natural_min_mm,
                natural_max_mm,
                outcome,
            });
        }
        Ok(accepted.then_some(site))
    }

    /// Re-run the production candidate path for ONE road and ONE request on the
    /// state `plan` currently commits to. The request's budget is shared across
    /// roads by the caller, so remaining demand and `maxCandidateAttempts` are
    /// cumulative exactly as in `frontage::append`; `sample_cap` only limits how
    /// much evidence is kept. Nothing in `plan` is modified.
    pub(super) fn measure_road_for_request(
        &self,
        plan: &RoadNetworkPlan,
        road: &EntityInstance,
        budget: &mut RequestBudget,
        sample_cap: usize,
    ) -> Result<RoadMeasurement, TerrainError> {
        let r = budget.request.clone();
        let mut state = self.state(plan);
        // One request: the shared schedule degenerates to this request's own
        // stations × sides, in the order placement would try them.
        let schedule = frontage::StationSchedule::new(road, std::slice::from_ref(&r))?;
        let mut trace = road_supply_header(road, &r, schedule.stations_of(0));
        let mut accepted = vec![];
        for turn in schedule.turns() {
            if budget.satisfied() || !budget.budget_left() {
                trace.geometry_exhausted = false;
                break;
            }
            if let Some(site) = self.attempt_turn(
                plan,
                &mut state,
                road,
                &r,
                turn.station,
                turn.side,
                budget,
                &mut trace,
                sample_cap,
            )? {
                accepted.push(site);
            }
        }
        Ok(RoadMeasurement { trace, accepted })
    }
}

#[allow(dead_code)] // read-only diagnosis entry point
/// Per-request summary of one diagnosis pass.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(super) struct RequestSummary {
    pub request_id: String,
    pub kind: String,
    pub purpose: String,
    pub count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub station_spacing_m: u32,
    pub setback_m: u32,
    pub max_candidate_attempts: u32,
    pub joint_accepted: usize,
    pub joint_attempted: u32,
    pub standalone_accepted: usize,
    pub rejected: BTreeMap<String, u32>,
    pub stop_reason: String,
}

/// Per-request, per-road diagnosis of one already-planned network.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(super) struct Diagnosis {
    pub model: String,
    pub scene_id: String,
    pub profile_roads: Vec<String>,
    pub requests: Vec<RequestSummary>,
    pub joint: Vec<RoadSupply>,
    pub standalone: Vec<RoadSupply>,
    pub stop_reason: String,
    /// Aggregates over every request of the joint pass, never just the first
    /// trace with evidence.
    pub joint_station_attempts: u32,
    pub joint_rejected: BTreeMap<String, u32>,
    pub joint_accepted: usize,
}

/// Reproduce the production frontage search on an already-planned network and
/// report where every candidate went. `joint` accumulates accepted parcels the
/// way the bake does; `standalone` measures each road alone against the
/// untouched committed state, so per-road capacity is never presented as a
/// joint witness.
#[allow(dead_code, clippy::too_many_arguments)]
pub(super) fn diagnose(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    mask: &ZoningMask,
    water: WaterGeometry,
    zones: &[LandUseZone],
    plan: &RoadNetworkPlan,
    requests: &[Request],
    sample_cap: usize,
    policy: super::bounded_cut::CutPolicy,
) -> Result<Diagnosis, TerrainError> {
    let context = SupplyContext {
        scene,
        terrain,
        mask,
        water,
        zones,
        policy,
    };
    let roads = frontage::profile_roads(plan);
    let usable: Vec<&EntityInstance> = roads
        .iter()
        .filter(|road| road.grounding.road_profile.is_some())
        .collect();
    // The diagnosis owns its accumulation copy; the frozen plan is untouched.
    //
    // Only the parcels of the DIAGNOSED requests are stripped. Production keeps
    // every other program site (service and joint-layout parcels) standing while
    // it places frontage, so a replay that cleared all of them would judge
    // candidates against ground the bake never offered.
    let diagnosed: BTreeSet<&str> = requests.iter().map(|r| r.id.as_str()).collect();
    let mut joint_plan = plan.clone();
    // The production pipeline runs `frontage::append` before publishing the
    // plan-wide road-cut allowance. A replay from the final plan must therefore
    // clear that post-frontage field: carrying it backwards gives the roads a
    // cutting authority the original search did not have and can rename or
    // replace a candidate without changing its geometry.
    joint_plan.road_cut_mm = None;
    joint_plan.roadside_sites.retain(|site| {
        !site
            .program
            .as_ref()
            .is_some_and(|program| diagnosed.contains(program.request_id.as_str()))
    });
    let mut joint = vec![];
    let mut standalone = vec![];
    let mut summaries: Vec<RequestSummary> = requests
        .iter()
        .map(|r| RequestSummary {
            request_id: r.id.clone(),
            kind: r.kind.clone(),
            purpose: r.purpose.clone(),
            count: r.count,
            width_m: r.width_m,
            depth_m: r.depth_m,
            station_spacing_m: r.station_spacing_m,
            setback_m: r.setback_m,
            max_candidate_attempts: r.max_candidate_attempts,
            joint_accepted: 0,
            joint_attempted: 0,
            standalone_accepted: 0,
            rejected: BTreeMap::new(),
            stop_reason: String::new(),
        })
        .collect();
    let without_sites = {
        let mut base = joint_plan.clone();
        base.frontage = None;
        base
    };
    // Road-major, station-major, side-minor and rotated over the requests: the
    // exact placement order of `frontage::append`, from the same shared
    // `StationSchedule` and driven by the same cumulative budgets. The plan view
    // and the engineering state are owned once for the whole pass, as the bake
    // owns them, so every accepted parcel is already occupied ground for the
    // candidate that follows it.
    //
    // The ORDER itself comes from `frontage::candidate_order`, the very function `append` walks: when
    // the world's explicit conversion policy is on, that order is nearest-the-city first, and a
    // diagnosis that re-derived the historical order would publish a search the bake never performs.
    let mut budgets: Vec<RequestBudget> = requests.iter().map(RequestBudget::new).collect();
    let mut state = context.state(&joint_plan);
    let nearest_first = requests.iter().any(|r| r.rural_conversion.is_explicit());
    let ordered = frontage::candidate_order(
        &usable,
        requests,
        &scene.scene_id,
        nearest_first.then_some(zones),
    )?;
    for (road_index, turns) in ordered {
        let road = usable[road_index];
        let schedule = frontage::StationSchedule::new(road, requests)?;
        // One trace per request still in play on this road, in program order.
        let mut slots: Vec<Option<usize>> = vec![None; requests.len()];
        for (index, r) in requests.iter().enumerate() {
            if budgets[index].satisfied() || !budgets[index].budget_left() {
                continue;
            }
            slots[index] = Some(joint.len());
            joint.push(road_supply_header(road, r, schedule.stations_of(index)));
        }
        for turn in turns {
            let Some(slot) = slots[turn.request] else {
                continue;
            };
            let budget = &mut budgets[turn.request];
            if budget.satisfied() || !budget.budget_left() {
                // The request met its demand or spent its budget on an earlier
                // diagonal, so this road was not searched to the end for it.
                joint[slot].geometry_exhausted = false;
                continue;
            }
            let accepted = context.attempt_turn(
                &joint_plan,
                &mut state,
                road,
                &requests[turn.request],
                turn.station,
                turn.side,
                budget,
                &mut joint[slot],
                sample_cap,
            )?;
            if let Some(site) = accepted {
                joint_plan.roadside_sites.push(site);
            }
        }
    }
    for (index, r) in requests.iter().enumerate() {
        summaries[index].joint_attempted = joint
            .iter()
            .filter(|trace| trace.request_id == r.id)
            .map(|trace| trace.attempted)
            .sum();
        summaries[index].joint_accepted = budgets[index].site_ids.len();
        summaries[index].rejected = budgets[index].rejected.clone();
        summaries[index].stop_reason = budgets[index].stop_reason();
    }
    // Standalone pass: one road, untouched committed state, never shared.
    for (index, r) in requests.iter().enumerate() {
        let mut standalone_accepted = 0;
        for road in &usable {
            let mut fresh = RequestBudget::new(r);
            let measured =
                context.measure_road_for_request(&without_sites, road, &mut fresh, sample_cap)?;
            standalone_accepted += measured.trace.accepted;
            standalone.push(measured.trace);
        }
        summaries[index].standalone_accepted = standalone_accepted;
    }
    let stop = summaries
        .iter()
        .find(|s| s.stop_reason != "demand_satisfied")
        .map(|s| s.stop_reason.clone())
        .unwrap_or_else(|| "demand_satisfied".into());
    let mut joint_rejected: BTreeMap<String, u32> = BTreeMap::new();
    for trace in &joint {
        for (reason, count) in &trace.rejected {
            *joint_rejected.entry(reason.clone()).or_default() += *count;
        }
    }
    Ok(Diagnosis {
        model: "production-frontage-candidate-diagnosis-v1".into(),
        scene_id: scene.scene_id.clone(),
        profile_roads: usable.iter().map(|r| r.entity_id.clone()).collect(),
        requests: summaries,
        joint_station_attempts: joint.iter().map(|t| t.attempted).sum(),
        joint_rejected,
        joint_accepted: joint.iter().map(|t| t.accepted).sum(),
        joint,
        standalone,
        stop_reason: stop,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{
        LandUseProfile, SceneSpec, plan_landuse_zones, terrain_height_metres, water_geometry,
        water_kind,
    };
    use super::*;
    use crate::height_payload::encode_millimetres;
    use crate::settlement::morphology_compile;

    fn frozen_path() -> std::path::PathBuf {
        std::env::var("GLYPHWEAVE_W1_MANIFEST")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                    "../../work/collaboration/ds-b01-l1-supply/w1-collector-local.manifest.json",
                )
            })
    }

    struct Stage {
        scene: SceneSpec,
        terrain: BakedTerrain,
        water: WaterGeometry,
        zones: Vec<LandUseZone>,
        style: serde_json::Value,
        plan: RoadNetworkPlan,
        base_edges: usize,
        frozen_plan: serde_json::Value,
        frozen_report: serde_json::Value,
        effective_report: serde_json::Value,
    }

    /// Rebuild the frozen planning stage from the immutable seed and the
    /// authored manifest. `version` may raise the automatic road program to the
    /// demand-led version; nothing else about the input changes.
    fn frozen_w1_stage(version: u32, supply_states: usize) -> Stage {
        let raw = std::fs::read(frozen_path()).expect("frozen manifest");
        let manifest: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let scene = SceneSpec {
            scene_id: manifest["scenes"][0]["sceneId"].as_str().unwrap().into(),
            width_m: manifest["scenes"][0]["widthM"].as_u64().unwrap() as u32,
            depth_m: manifest["scenes"][0]["depthM"].as_u64().unwrap() as u32,
            origin_x: manifest["scenes"][0]["originX"].as_i64().unwrap() as i32,
            origin_z: manifest["scenes"][0]["originZ"].as_i64().unwrap() as i32,
            seed_offset: manifest["scenes"][0]["seedOffset"].as_u64().unwrap_or(0),
        };
        let seed = manifest["world"]["seed"].as_u64().unwrap();
        let mut authored = manifest["style"].clone();
        if version != 1 {
            authored["automaticRoadExtensions"]["version"] = serde_json::json!(version);
            if version == 3 {
                authored["automaticRoadExtensions"]["scenes"][0]["supply"] = serde_json::json!({
                    "version": 1,
                    "maxCandidateStates": supply_states,
                    "preflightBudgetMs": 180000,
                    "mode": std::env::var("GLYPHWEAVE_W1_SUPPLY_MODE")
                        .unwrap_or_else(|_| "append".to_string()),
                });
            }
        }
        let water = water_geometry(water_kind(&authored), &[], &scene, &authored);
        let compiled = morphology_compile::compile(&authored, std::slice::from_ref(&scene))
            .expect("morphology compile")
            .expect("profile");
        let effective = compiled.style;
        let effective_report = serde_json::to_value(&compiled.report).unwrap();
        let profile = LandUseProfile::from_style(&effective).unwrap();
        let zones = plan_landuse_zones(&scene, &profile);
        let values: Vec<f64> = (0..scene.depth_m as i32)
            .flat_map(|z| (0..scene.width_m as i32).map(move |x| (x, z)))
            .map(|(x, z)| terrain_height_metres(seed, x, z, water, &[]))
            .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        let mut plan =
            super::super::road_network::plan_with_zones(&scene, &terrain, water, Some(&zones))
                .unwrap();
        let base_edges = plan.edges.len();
        super::super::road_extensions::append(
            &mut plan, &scene, &terrain, water, &zones, &effective,
        )
        .unwrap();
        // This reconstruction rebuilds the authored natural field verbatim from
        // the manifest seed and millimetre encoding, so the declaration below is
        // the same one the bake itself makes.
        super::super::auto_roads::append_declared(
            &mut plan,
            &scene,
            &terrain,
            water,
            &zones,
            &effective,
            super::super::NaturalSource::AuthoredSeed {
                seed,
                precise: true,
            },
        )
        .unwrap();
        let published = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../work/collaboration/ds-b04-c1-r2-b02-w1/gen-w1-collector-local/scenes/scene-0",
        );
        let frozen_plan: serde_json::Value = serde_json::from_slice(
            &std::fs::read(published.join("road-network-plan.json")).unwrap(),
        )
        .unwrap();
        let frozen_report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(published.join("scene.json")).unwrap())
                .map(|scene_json: serde_json::Value| {
                    scene_json["roadNetworkPlan"]["frontage"].clone()
                })
                .unwrap();
        Stage {
            scene,
            terrain,
            water,
            zones,
            style: effective,
            plan,
            base_edges,
            frozen_plan,
            frozen_report,
            effective_report,
        }
    }

    /// Read-only reconstruction of the frozen B02-W1 planning stage, then the
    /// bounded per-request/per-road diagnosis. Asserts the reconstruction is
    /// bit-identical to the published world before reporting anything.
    #[test]
    #[ignore = "bounded read-only check against the frozen baseline"]
    fn diagnosis_replays_production_and_carries_budgets_across_roads() {
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style,
            plan,
            ..
        } = frozen_w1_stage(1, 0);
        // A deliberately small cumulative budget: four profile roads share it.
        let mut requests: Vec<Request> = frontage::from_style(&style)
            .unwrap()
            .unwrap()
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene.scene_id)
            .map(|mut r| {
                r.count = 4;
                r.max_candidate_attempts = 10;
                r
            })
            .collect();
        requests.sort_by(|a, b| a.id.cmp(&b.id));
        let mut limited = style.clone();
        limited["frontageProgram"]["requests"] = serde_json::to_value(&requests).unwrap();
        let mask = ZoningMask::new(&scene, &zones).unwrap();
        let before = serde_json::to_value(&plan).unwrap();
        let diagnosis = diagnose(
            &scene,
            &terrain,
            &mask,
            water,
            &zones,
            &plan,
            &requests,
            4,
            super::super::bounded_cut::CutPolicy::Disabled,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&plan).unwrap(), before);
        // The diagnosis is production's own selection, identity for identity.
        let mut produced = plan.clone();
        frontage::append(
            &mut produced,
            &scene,
            &terrain,
            water,
            Some(&zones),
            &limited,
        )
        .unwrap();
        for outcome in &produced.frontage.as_ref().unwrap().outcomes {
            let diagnosed: Vec<String> = diagnosis
                .joint
                .iter()
                .filter(|trace| trace.request_id == outcome.request.id)
                .flat_map(|trace| trace.accepted_ids.clone())
                .collect();
            assert_eq!(
                diagnosed, outcome.site_ids,
                "diagnosis and production must agree for {}",
                outcome.request.id
            );
        }
        // Budgets are cumulative per request across every road, never per road.
        for summary in &diagnosis.requests {
            assert!(
                summary.joint_attempted <= summary.max_candidate_attempts,
                "{} attempted {} of {}",
                summary.request_id,
                summary.joint_attempted,
                summary.max_candidate_attempts
            );
            assert!(summary.joint_accepted <= summary.count as usize);
        }
        // Reported totals cover every request, not only the first trace.
        assert_eq!(
            diagnosis.joint_station_attempts,
            diagnosis.joint.iter().map(|t| t.attempted).sum::<u32>()
        );
        let mut aggregated: BTreeMap<String, u32> = BTreeMap::new();
        for trace in &diagnosis.joint {
            for (reason, count) in &trace.rejected {
                *aggregated.entry(reason.clone()).or_default() += *count;
            }
        }
        assert_eq!(diagnosis.joint_rejected, aggregated);
        assert!(diagnosis.joint_station_attempts > 0);
    }

    #[test]
    #[ignore = "diagnostic: reads the frozen W1 manifest, writes nothing"]
    fn frozen_w1_supply_diagnosis_is_bounded_and_reproduces_the_published_plan() {
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style: effective,
            plan,
            base_edges,
            frozen_plan,
            frozen_report,
            effective_report,
        } = frozen_w1_stage(1, 0);
        let seed = 42_u64;
        let mask = ZoningMask::new(&scene, &zones).unwrap();
        let mut nodes: Vec<(usize, i32, i32)> = plan
            .nodes
            .iter()
            .map(|n| (n.id, n.world_x, n.world_z))
            .collect();
        let mut frozen_nodes: Vec<(usize, i32, i32)> = frozen_plan["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                (
                    n["id"].as_u64().unwrap() as usize,
                    n["worldX"].as_i64().unwrap() as i32,
                    n["worldZ"].as_i64().unwrap() as i32,
                )
            })
            .collect();
        nodes.sort();
        frozen_nodes.sort();
        let mut edges: Vec<(String, usize, usize, u32, i64)> = plan
            .edges
            .iter()
            .map(|e| {
                (
                    e.id.clone(),
                    e.from,
                    e.to,
                    e.length_m,
                    (e.rotation_y_deg * 1000.).round() as i64,
                )
            })
            .collect();
        let mut frozen_edges: Vec<(String, usize, usize, u32, i64)> = frozen_plan["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["id"].as_str().unwrap().to_string(),
                    e["from"].as_u64().unwrap() as usize,
                    e["to"].as_u64().unwrap() as usize,
                    e["lengthM"].as_u64().unwrap() as u32,
                    (e["rotationYDeg"].as_f64().unwrap() * 1000.).round() as i64,
                )
            })
            .collect();
        edges.sort();
        frozen_edges.sort();
        let requests = frontage::from_style(&effective)
            .unwrap()
            .unwrap()
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene.scene_id)
            .collect::<Vec<_>>();
        let before = serde_json::to_value(&plan).unwrap();
        let diagnosis = diagnose(
            &scene,
            &terrain,
            &mask,
            water,
            &zones,
            &plan,
            &requests,
            64,
            super::super::bounded_cut::CutPolicy::Disabled,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            before,
            "diagnosis must not modify the plan"
        );
        let heightfield_hash = terrain.content_hash().unwrap();
        // Coarse context only: obligated sites that already existed before any
        // frontage selection, and a 32m height grid of the immutable source.
        let obligated: Vec<serde_json::Value> = plan
            .site_entities()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.entity_id,
                    "kind": e.kind,
                    "x": e.world_x,
                    "z": e.world_z,
                    "widthM": e.width_m,
                    "depthM": e.depth_m,
                })
            })
            .collect();
        let height_grid: Vec<Vec<i32>> = (0..16)
            .map(|row| {
                (0..16)
                    .map(|col| {
                        terrain
                            .height_millimetres(16 + col * 32, 16 + row * 32)
                            .unwrap()
                    })
                    .collect()
            })
            .collect();
        // Which already-committed entity's support envelope actually controls a
        // reported sample. Read-only attribution over the same carve planner.
        let mut contributors = vec![];
        'attribute: for trace in diagnosis.joint.iter() {
            for detail in trace.rejection_details.iter() {
                if contributors.len() >= 24 {
                    break 'attribute;
                }
                let Some([x, z]) = detail.at else {
                    continue;
                };
                let natural = terrain.height_millimetres(x, z).unwrap();
                let mut full_cover = vec![];
                for e in plan.entities().iter().chain(plan.site_entities().iter()) {
                    let carves = super::super::plan_terrain_carves(std::slice::from_ref(e));
                    for carve in &carves {
                        let outside = super::super::carve_outside_m(
                            carve,
                            f64::from(x),
                            f64::from(z),
                        );
                        if outside <= 0. {
                            full_cover.push(serde_json::json!({
                                "entity": e.entity_id,
                                "kind": e.kind,
                                "targetMm": (carve.target_h_m * 1000.).round() as i64,
                                "priority": carve.priority,
                                "half": [carve.half_w, carve.half_d],
                                "blendM": carve.blend_m,
                            }));
                        }
                    }
                    let value = super::super::apply_terrain_carves(
                        f64::from(natural) / 1000.,
                        x,
                        z,
                        &carves,
                    );
                    if (value * 1000.).round() as i32 > natural + 500 {
                        full_cover.push(serde_json::json!({
                            "entity": e.entity_id,
                            "kind": e.kind,
                            "aloneBlendedMm": (value * 1000.).round() as i32,
                        }));
                    }
                }
                contributors.push(serde_json::json!({
                    "roadId": trace.road_id,
                    "requestId": trace.request_id,
                    "reason": detail.reason,
                    "at": [x, z],
                    "naturalMm": natural,
                    "beforeMm": detail.before_mm,
                    "afterMm": detail.after_mm,
                    "designMm": detail.design_mm,
                    "conflictingEntity": detail.conflicting_entity,
                    "fullCoverOrBlend": full_cover,
                }));
            }
        }
        let report = serde_json::json!({
            "reconstruction": {
                "manifest": frozen_path().display().to_string(),
                "seed": seed,
                "scene": serde_json::to_value(&scene).unwrap(),
                "heightfieldHash": heightfield_hash,
                "frozenHeightfieldHash": frozen_plan["naturalHeightfieldHash"],
                "heightfieldMatches": heightfield_hash == frozen_plan["naturalHeightfieldHash"],
                "baseEdges": base_edges,
                "nodesMatch": nodes == frozen_nodes,
                "edgesMatch": edges == frozen_edges,
                "nodeCount": nodes.len(),
                "edgeCount": edges.len(),
                "effectiveFrontageProgram": effective["frontageProgram"].clone(),
                "effectiveAutomaticRoads": effective["automaticRoadExtensions"].clone(),
                "frozenFrontageOutcomes": frozen_report["outcomes"].clone(),
                "maxRoadbedFillM": plan.max_roadbed_fill_m,
                "obligatedSiteCount": obligated.len(),
                "obligatedSites": obligated,
                "heightGrid16x16At32m": height_grid,
                "sampleAttribution": contributors,
                "recompiledEffectiveReport": effective_report,
            },
            "diagnosis": serde_json::to_value(&diagnosis).unwrap(),
        });
        assert_eq!(heightfield_hash, frozen_plan["naturalHeightfieldHash"]);
        assert_eq!(nodes, frozen_nodes, "reconstructed nodes differ");
        assert_eq!(edges, frozen_edges, "reconstructed edges differ");
        // The frozen world recorded these exact per-request outcomes.
        for (outcome, summary) in frozen_report["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&diagnosis.requests)
        {
            assert_eq!(
                outcome["request"]["id"].as_str().unwrap(),
                summary.request_id
            );
            // B01-T2 R3 V4-3: the frozen report was produced BEFORE base roads
            // carried profiles, so its per-request numbers are PRE-T2 facts - with
            // no profiled base road there was no base-road frontage and therefore
            // zero sites. The pre-T2 reference and the T2 baseline are both pinned
            // exactly here (so the change is visible and attributable rather than
            // hidden), the demand ids still must match, and T2 must not LOSE site
            // capacity. The isolated pre-T2 copy is what proves the delta is T2.
            let frozen_sites = outcome["siteIds"].as_array().map_or(0, |a| a.len());
            let pre_t2_reference = match summary.request_id.as_str() {
                "b04c1-homes" => (24_u32, 16_u32),
                "b04c1-shops" => (24_u32, 8_u32),
                other => panic!("unexpected frozen request {other}"),
            };
            assert_eq!(
                (
                    outcome["attempted"].as_u64().unwrap() as u32,
                    outcome["unfilled"].as_u64().unwrap() as u32,
                ),
                pre_t2_reference,
                "the frozen world is the documented PRE-T2 reference"
            );
            let t2_baseline = match summary.request_id.as_str() {
                "b04c1-homes" => (148_u32, 4_usize),
                "b04c1-shops" => (148_u32, 0_usize),
                other => panic!("unexpected request {other}"),
            };
            assert_eq!(
                (summary.joint_attempted, summary.joint_accepted),
                t2_baseline,
                "the T2 diagnosis baseline for {}",
                summary.request_id
            );
            assert!(
                summary.joint_accepted >= frozen_sites,
                "T2 must never lose realized site capacity: {summary:?}"
            );
        }
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    /// One bounded measurement of the accepted joint realization chain on the
    /// frozen 35-edge baseline: the engineered final raster, road support,
    /// development, yard surfaces and the access graph, all in memory.
    /// The entering baseline's *full development parcel set* is protected
    /// assets, not new supply: a bounded production-derived fixture for the C3
    /// acceptance cases. Reads the frozen world read-only; writes nothing.
    #[test]
    #[ignore = "bounded C3 fixture: derives the real parcel map from the frozen baseline"]
    fn frozen_baseline_parcels_are_protected_and_new_parcels_must_be_resolved() {
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style: v1_style,
            plan,
            ..
        } = frozen_w1_stage(1, 0);
        let seed = 42_u64;
        // The same demand-led program, with zero candidate states so the phase
        // itself is not what is under test here.
        let mut v3_style = v1_style.clone();
        v3_style["automaticRoadExtensions"]["version"] = serde_json::json!(3);
        v3_style["automaticRoadExtensions"]["scenes"][0]["supply"] = serde_json::json!({
            "version": 1,
            "maxCandidateStates": 0,
            "preflightBudgetMs": 180000,
            "mode": "append",
        });
        let base =
            super::super::road_network::plan_with_zones(&scene, &terrain, water, Some(&zones))
                .unwrap();
        let trusted = super::super::road_parcel_feasibility::trusted_base_contract(
            &base,
            &terrain.content_hash().unwrap(),
        )
        .unwrap();
        let mut verified = plan;
        frontage::append(
            &mut verified,
            &scene,
            &terrain,
            water,
            Some(&zones),
            &v3_style,
        )
        .unwrap();
        // The real baseline realization: production development + landcover
        // realize/verify produce the parcel ownership and its yard shortfalls.
        let baseline = realize_candidate(
            &scene,
            &terrain,
            water,
            &zones,
            &verified,
            super::super::NaturalSource::AuthoredSeed {
                seed,
                precise: true,
            },
            &trusted,
            &v3_style,
        )
        .unwrap();
        assert_eq!(baseline.final_source, "production-encoder");
        // B01-T2 R3 V4-3: the pre-T2 baseline was two shortfalls summing 13 m2.
        // With profiled base roads the ENTERING baseline itself changes, because
        // the base-road frontage/roadside parcels now carry yard obligations.
        // The pre-T2 value is kept as the documented reference and the T2
        // baseline is pinned exactly, with an added conservation identity.
        const PRE_T2_YARD_UNRESOLVED_M2: f64 = 13.0;
        assert_eq!(
            // R8-1: A' + the re-based selection gates took effect on this scene for
            // the first time: one 208 m2 roadside pad now realises and one site is
            // refused by name (its natural ground sits above the road's surface), so
            // 4419 -> 4211 m2 ( = the same two frontage parcels 51 + 20 roadside pads
            // x 208). The 13 m2 PRE-T2 and 4419 m2 R4/T2 references are kept in the
            // message below; the ~51 m2 R8 target is NOT reached yet.
            // R11 G1: the 2 m legacy parcel-inset ring is unrealizable where it sits
            // on a road's blend ramp, so roadside pads now carry an explicit zero
            // inset and the yard's required area IS the flat pad. 4211 -> 51.0 m2,
            // i.e. exactly the two frontage parcels; the 13 m2 (pre-T2) and 4211 m2
            // (R8/R10) references are kept here per the established rule.
            baseline.yard_unresolved_m2,
            51.0,
            "the T2 baseline total (was {PRE_T2_YARD_UNRESOLVED_M2} before T2)"
        );
        let mut session = SupplySession::for_scene(&v3_style, &scene.scene_id, &verified).unwrap();
        session.adopt(&verified, &baseline).unwrap();
        // The protected set is the *whole* pre-existing development asset set,
        // not the frontage requests.
        assert_eq!(
            session.protected_parcels.len(),
            baseline
                .parcel_by_entity
                .values()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            "every entering parcel must be protected"
        );
        let shorted: Vec<(String, f64)> = session
            .protected_parcels
            .iter()
            .filter(|(_, shortfall)| **shortfall > 0.)
            .map(|(parcel, shortfall)| (parcel.clone(), *shortfall))
            .collect();
        // The two pre-T2 frontage shortfalls are still exactly the same, and the
        // T2-only roadside obligations are named explicitly.
        let mut frontage_shorts: Vec<f64> = shorted
            .iter()
            .filter(|(parcel, _)| parcel.starts_with("generated.frontage."))
            .map(|(_, shortfall)| *shortfall)
            .collect();
        frontage_shorts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            frontage_shorts,
            vec![21.0, 30.0],
            "the two pre-T2 frontage shortfalls are unchanged by T2"
        );
        let roadside_shorts: Vec<f64> = shorted
            .iter()
            .filter(|(parcel, _)| parcel.starts_with("generated.roadside."))
            .map(|(_, shortfall)| *shortfall)
            .collect();
        assert!(
            roadside_shorts.is_empty(),
            "R11 G1: no roadside yard may remain unresolved: {roadside_shorts:?}"
        );
        assert!(
            roadside_shorts.iter().all(|s| (*s - 208.0).abs() < 1e-9),
            "{roadside_shorts:?}"
        );
        assert_eq!(
            shorted.len(),
            2,
            "only the two frontage parcels after R11 G1"
        );
        assert_eq!(
            shorted.iter().map(|(_, shortfall)| *shortfall).sum::<f64>(),
            baseline.yard_unresolved_m2,
            "the per-parcel shortfalls must conserve the baseline total"
        );
        // Protecting the entering obligations is not frontage credit.
        //
        // B01-T2 R3 V4-3: pre-T2 the entering baseline realized NOTHING (no
        // profiled base road could carry frontage), so "no invented credit" was
        // observable as an all-zero total. The T2 baseline does realize base-road
        // frontage, so the invariant that matters - and is asserted here in both
        // directions - is that the session credits EXACTLY the baseline's own
        // realized set per request: nothing invented, nothing dropped.
        let credited = session.realized_totals();
        for (request_id, count) in &credited {
            let baseline_ids = baseline
                .realized_ids_by_request
                .iter()
                .find(|(id, _)| id == request_id)
                .map_or(0, |(_, ids)| ids.len());
            assert_eq!(
                *count, baseline_ids,
                "credit for {request_id} must be the baseline's own realized set, never invented demand"
            );
        }
        for (request_id, ids) in &baseline.realized_ids_by_request {
            assert!(
                credited.iter().any(|(id, _)| id == request_id),
                "request {request_id} realizes {} sites in the entering baseline and must stay credited",
                ids.len()
            );
        }

        let new_entity = "fixture.candidate.home".to_string();
        let new_parcel = "fixture.candidate.parcel".to_string();
        let gain = |mut realization: Realization, shortfall: f64| -> Realization {
            // B01-T2 R3 V4-3: the candidate is the entering baseline PLUS one
            // resolved home. Pre-T2 the baseline realized nothing, so replacing
            // these vectors was equivalent to extending them; with a T2 baseline
            // that already realizes base-road frontage, replacing them would drop
            // the baseline's own realizations and trip
            // `realization_lost_existing_access`.
            realization.realized_ids.push(new_entity.clone());
            match realization
                .realized_by_request
                .iter_mut()
                .find(|(id, _)| id == "b04c1-homes")
            {
                Some(entry) => entry.1 += 1,
                None => realization
                    .realized_by_request
                    .push(("b04c1-homes".into(), 1)),
            }
            match realization
                .realized_ids_by_request
                .iter_mut()
                .find(|(id, _)| id == "b04c1-homes")
            {
                Some(entry) => entry.1.push(new_entity.clone()),
                None => realization
                    .realized_ids_by_request
                    .push(("b04c1-homes".into(), vec![new_entity.clone()])),
            }
            realization.planned += 1;
            realization.realized_with_access += 1;
            // The guard refuses any candidate whose development audit is "fail".
            // Pre-T2 this fixture inherited a "pass" baseline; the T2 baseline is
            // "fail" (recorded in r3/frontage-supply-diagnosis.json), so the
            // fixture must model a candidate that genuinely resolves development
            // to exercise the C3 acceptance cases at all. The baseline's own
            // failure list is carried through unchanged, so nothing is hidden.
            realization.development_status = "pass".to_string();
            realization
                .parcel_by_entity
                .insert(new_entity.clone(), new_parcel.clone());
            if shortfall > 0. {
                realization
                    .yard_unresolved_by_parcel
                    .insert(new_parcel.clone(), shortfall);
            }
            realization
        };
        // (1) Old shortfalls unchanged, the new parcel fully resolved: accept.
        let candidate = gain(baseline.clone(), 0.);
        super::super::auto_roads::commit_decision(&session, &candidate, &baseline, &verified)
            .expect("an unchanged old baseline plus a resolved new parcel must be accepted");
        // (2) Every old parcel repaired while the new parcel carries 13 m2: the
        //     old repair may not be spent on the new parcel.
        let mut repaired = gain(baseline.clone(), 13.);
        repaired.yard_unresolved_by_parcel =
            std::collections::BTreeMap::from([(new_parcel.clone(), 13.0)]);
        repaired.yard_unresolved_m2 = 13.0;
        let error =
            super::super::auto_roads::commit_decision(&session, &repaired, &baseline, &verified)
                .unwrap_err();
        assert!(error.contains("new_parcel_yard_unresolved"), "{error}");
        // (3) One old parcel repaired, another old parcel grows by the same
        //     amount: the total is unchanged, so only a per-parcel comparison
        //     can catch it.
        let mut moved = gain(baseline.clone(), 0.);
        let mut by_parcel = baseline.yard_unresolved_by_parcel.clone();
        let first = shorted[0].0.clone();
        let second = shorted[1].0.clone();
        by_parcel.remove(&first);
        let moved_amount = shorted[0].1 + shorted[1].1;
        by_parcel.insert(second.clone(), moved_amount);
        moved.yard_unresolved_by_parcel = by_parcel;
        // V4-3: the move is total-NEUTRAL, so the scalar total must stay the
        // baseline's; setting it to `moved_amount` (the old 13 m2 world total)
        // would make the candidate an outright regression and mask the
        // per-parcel check this case exists to prove.
        moved.yard_unresolved_m2 = baseline.yard_unresolved_m2;
        let error =
            super::super::auto_roads::commit_decision(&session, &moved, &baseline, &verified)
                .unwrap_err();
        assert!(
            error.contains("existing_parcel_yard_grew"),
            "a total-neutral move must still name the growing parcel: {error}"
        );
        assert!(
            error.contains(&second),
            "the growing parcel must be named: {error}"
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "glyphweave.b01-supply-r3.c3-parcel-map",
                "sceneId": scene.scene_id,
                "finalSource": baseline.final_source,
                "baselineYardUnresolvedM2": baseline.yard_unresolved_m2,
                "protectedParcelCount": session.protected_parcels.len(),
                "protectedEntityCount": session.protected_owners.len(),
                "shortfallParcels": shorted.iter().map(|(parcel, shortfall)| {
                    serde_json::json!({"parcel": parcel, "shortfallM2": shortfall})
                }).collect::<Vec<_>>(),
                "requestCredit": session.realized_totals(),
                "unresolvedAreaM2": baseline.unresolved_area_m2,
                "cases": {
                    "oldUnchangedPlusResolvedNewParcel": "accepted",
                    "oldRepairedPlusNewParcelShort13": "refused:new_parcel_yard_unresolved",
                    "totalNeutralMove": "refused:existing_parcel_yard_grew",
                },
            }))
            .unwrap()
        );
    }

    #[test]
    #[ignore = "bounded probe: recomputes the frozen baseline final raster in memory"]
    fn frozen_w1_joint_realization_baseline_is_measured_once() {
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style,
            plan,
            ..
        } = frozen_w1_stage(1, 0);
        let seed = 42_u64;
        let base =
            super::super::road_network::plan_with_zones(&scene, &terrain, water, Some(&zones))
                .unwrap();
        let trusted = super::super::road_parcel_feasibility::trusted_base_contract(
            &base,
            &terrain.content_hash().unwrap(),
        )
        .unwrap();
        assert_eq!(plan.edges.len(), 35, "the frozen 35-edge baseline");
        // Trust evidence: the plan's own ungraded base domain must equal a fresh
        // production rebuild of the authored base network.
        let mut rebuilt_edges: Vec<(String, usize, usize, u32)> = base
            .edges
            .iter()
            .map(|e| (e.id.clone(), e.from, e.to, e.length_m))
            .collect();
        let mut plan_base_edges: Vec<(String, usize, usize, u32)> = plan
            .edges
            .iter()
            .filter(|edge| {
                plan.extensions
                    .iter()
                    .all(|extension| extension.entity_id != edge.id)
            })
            .map(|e| (e.id.clone(), e.from, e.to, e.length_m))
            .collect();
        rebuilt_edges.sort();
        plan_base_edges.sort();
        assert_eq!(
            rebuilt_edges, plan_base_edges,
            "the baseline's ungraded domain must be the production rebuild"
        );
        let mut rebuilt_nodes: Vec<(usize, i32, i32)> = base
            .nodes
            .iter()
            .map(|n| (n.id, n.world_x, n.world_z))
            .collect();
        let mut plan_nodes: Vec<(usize, i32, i32)> = plan
            .nodes
            .iter()
            .filter(|node| {
                plan_base_edges
                    .iter()
                    .any(|(_, from, to, _)| node.id == *from || node.id == *to)
            })
            .map(|n| (n.id, n.world_x, n.world_z))
            .collect();
        rebuilt_nodes.sort();
        plan_nodes.sort();
        assert_eq!(rebuilt_nodes, plan_nodes, "the base node set must match");
        // The production pipeline runs the frontage stage before its audits, so
        // audit the same stage sequence here.
        let mut plan = plan;
        frontage::append(&mut plan, &scene, &terrain, water, Some(&zones), &style).unwrap();
        let started = std::time::Instant::now();
        let realization = realize_candidate(
            &scene,
            &terrain,
            water,
            &zones,
            &plan,
            super::super::NaturalSource::AuthoredSeed {
                seed,
                precise: true,
            },
            &trusted,
            &style,
        )
        .unwrap();
        let total_seconds = started.elapsed().as_secs_f64();
        // Before/after equivalence of the shared production audit: the same
        // final raster and entities audited by the whole production entry point
        // (which re-plans and compares, no recursion because this is the frozen
        // v1/v2 style) and by the non-recursive leaf the supply phase uses.
        let final_terrain = candidate_final_terrain(
            &scene,
            &terrain,
            water,
            &zones,
            &plan,
            super::super::NaturalSource::AuthoredSeed {
                seed,
                precise: true,
            },
        )
        .unwrap()
        .0;
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let production = super::super::road_network::verify_with_style(
            &plan,
            &scene,
            &terrain,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        let leaf = super::super::road_network::audit_final_support_for_record(
            &plan,
            &scene,
            &terrain,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        assert_eq!(
            production.realized_edges, leaf.realized_edges,
            "shared leaf must realize exactly the production audit's edges"
        );
        assert_eq!(
            production.failures, leaf.failures,
            "shared leaf failures must match the production audit"
        );
        assert_eq!(production.realized_edges, plan.edges.len());
        // C1 acceptance on the real baseline: +1mm on a graded road's flat-top
        // edge and +1mm inside its transport footprint must each be refused by
        // the shared production leaf, with the offending road named.
        let graded = plan
            .entities()
            .into_iter()
            .find(|entity| entity.grounding.road_profile.is_some())
            .expect("a graded road exists");
        let (flat_w, _) = super::super::graded_flat_top_half_extents(&graded);
        let (transport_w, _) = super::super::graded_transport_half_extents(&graded);
        let perturbed = |x: i32, z: i32| -> BakedTerrain {
            let mut values = Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
            for zz in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
                for xx in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                    let mut value = f64::from(final_terrain.height(xx, zz).unwrap());
                    if xx == x && zz == z {
                        value += 0.001;
                    }
                    values.push(value);
                }
            }
            let mut terrain = BakedTerrain::new(&scene).unwrap();
            terrain
                .insert_chunk(
                    scene.origin_x,
                    scene.origin_z,
                    scene.width_m,
                    scene.depth_m,
                    &crate::height_payload::encode_millimetres(
                        scene.width_m,
                        scene.depth_m,
                        values,
                    )
                    .unwrap(),
                )
                .unwrap();
            terrain
        };
        let mut perturbed_results = vec![];
        for (label, offset) in [
            ("flat_top_edge", flat_w),
            ("transport_footprint", transport_w),
        ] {
            let damaged = perturbed(graded.world_x + offset, graded.world_z);
            let audit = super::super::road_network::audit_final_support_for_record(
                &plan,
                &scene,
                &terrain,
                &damaged,
                water,
                &entities,
                Some(&zones),
                None,
                &style,
            )
            .unwrap();
            assert!(
                audit.realized_edges < plan.edges.len(),
                "{label}: +1mm must not be realized ({:?})",
                audit.failures.first()
            );
            perturbed_results.push(serde_json::json!({
                "case": label,
                "damagedAt": [graded.world_x + offset, graded.world_z],
                "realizedEdges": audit.realized_edges,
                "failures": audit.failures,
            }));
        }
        // A deleted fixed base edge and a forged extension source each hit their
        // own rejection class.
        let base = super::super::auto_roads::base_only_plan(&plan).unwrap();
        let trusted_again = super::super::road_parcel_feasibility::trusted_base_contract(
            &base,
            &terrain.content_hash().unwrap(),
        )
        .unwrap();
        let mut deleted = plan.clone();
        let position = deleted
            .edges
            .iter()
            .position(|edge| edge.id == base.edges[0].id)
            .unwrap();
        deleted.edges.remove(position);
        let deleted_status =
            super::super::road_parcel_feasibility::audit_node_heights(&deleted, &trusted_again)
                .is_err();
        assert!(deleted_status, "a deleted fixed base edge must be refused");
        let mut forged = plan.clone();
        forged.extensions[0].design.profile_mm[10] += 25;
        let forged_failures = super::super::road_network::audit_extension_provenance(
            &forged, &scene, &terrain, water, &zones,
        )
        .unwrap();
        assert!(
            !forged_failures.is_empty(),
            "a forged design must be refused"
        );
        let production_audit = serde_json::json!({
            "productionRealizedEdges": production.realized_edges,
            "productionFailures": production.failures,
            "leafRealizedEdges": leaf.realized_edges,
            "leafFailures": leaf.failures,
            "equivalent": true,
            "perturbations": perturbed_results,
            "deletedFixedBaseEdgeRefused": deleted_status,
            "forgedExtensionSourceRefused": forged_failures,
        });
        let report = serde_json::json!({
            "baseline": {
                "edges": plan.edges.len(),
                "nodes": plan.nodes.len(),
                "extensions": plan.extensions.len(),
                "sites": plan.roadside_sites.len(),
                "frontagePlanned": realization.planned,
                "realizedWithAccess": realization.realized_with_access,
                "realizedById": realization.realized_by_request,
                "yardUnresolvedM2": realization.yard_unresolved_m2,
                "parcels": realization.parcels,
                "buildingsRealized": realization.buildings_realized,
                "frontageStatus": realization.frontage_status,
                "developmentStatus": realization.development_status,
                "landcoverStatus": realization.landcover_status,
                "programLandcoverStatus": realization.program_landcover_status,
                "finalSource": realization.final_source,
                "roadStatus": realization.road_status,
                "roadFailures": realization.road_failures,
                "gradedRoads": realization.graded_roads,
                "nodeStatus": realization.node_status,
                "developmentFailures": realization.development_failures,
                "landcoverFailures": realization.landcover_failures,
                "requiredUnconnected": realization.required_unconnected,
                "requiredUnconnectedIds": realization.required_unconnected_ids,
                "realizationSeconds": realization.seconds,
                "totalSeconds": total_seconds,
            },
            "trusted": {
                "naturalHash": trusted.natural_hash,
                "baseLevelMm": trusted.base_level_mm,
                "fixedNodes": trusted.fixed_nodes.len(),
                "baseEdges": trusted.base_edges.len(),
            },
            "productionAuditEquivalence": production_audit,
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    /// B01-G1: bounded feasibility experiment on the frozen W1 input.
    ///
    /// The candidate table is frozen in
    /// `bevy/work/collaboration/ds-b01-g1-feasible-increment/candidate-table.json`
    /// before this test was written: six near-miss corridors (production
    /// design/append passed but 0 real parcels were earned) times four states,
    /// where a state is {the live `infer_end` plateau, the fixed start
    /// junction's level} times {the authorized 12 m local tier, the authorized
    /// 16 m collector tier} = 24 states. The production leaves make every
    /// judgement: corridor design+verify, the frontage measurement, the full
    /// realization chain and the commit gate. Nothing here re-implements
    /// placement, audit or carving, and the frozen natural field, the 35
    /// baseline roads, the fixed node levels and the 24 authored demand are
    /// never changed.
    #[test]
    #[ignore = "B01-G1 bounded feasibility experiment: reads the frozen W1 input, writes nothing"]
    fn frozen_near_miss_corridors_try_a_second_legal_endpoint_plateau() {
        use super::super::road_alignment::CorridorRequest;

        let Stage {
            scene,
            terrain,
            water,
            zones,
            style,
            plan,
            ..
        } = frozen_w1_stage(1, 0);
        assert_eq!(plan.edges.len(), 35, "the frozen 35-edge baseline");
        let seed = 42_u64;
        let base =
            super::super::road_network::plan_with_zones(&scene, &terrain, water, Some(&zones))
                .unwrap();
        let trusted = super::super::road_parcel_feasibility::trusted_base_contract(
            &base,
            &terrain.content_hash().unwrap(),
        )
        .unwrap();
        let mut verified = plan.clone();
        frontage::append(&mut verified, &scene, &terrain, water, Some(&zones), &style).unwrap();
        let baseline = realize_candidate(
            &scene,
            &terrain,
            water,
            &zones,
            &verified,
            super::super::NaturalSource::AuthoredSeed {
                seed,
                precise: true,
            },
            &trusted,
            &style,
        )
        .unwrap();
        assert_eq!(
            baseline.frontage_requested, 24,
            "the authored demand stays 24"
        );
        let obligation_ids: Vec<String> = verified
            .roadside_sites
            .iter()
            .filter(|site| site.program.is_none())
            .map(|site| site.id.clone())
            .collect();
        // R8-1: 20 - one site is now refused by the re-based selection gate.
        // R11 (note 2): 21 - the pad's STATION is now chosen along the road
        // (the smallest rise between the road's required surface over the pad's
        // window and the road's own surface at that station), so one more site
        // of this frozen baseline is placed somewhere its own frontage is
        // reachable instead of being refused at the edge's centre.
        assert_eq!(
            obligation_ids.len(),
            21,
            "the entering obligations after R11"
        );
        let mut session = SupplySession::for_scene(&style, &scene.scene_id, &verified).unwrap();
        session.adopt(&verified, &baseline).unwrap();
        let ctx =
            super::super::road_alignment::AlignmentContext::new(&scene, &terrain, water, &zones)
                .unwrap();
        let mask = ZoningMask::new(&scene, &zones).unwrap();
        let supply = SupplyContext {
            scene: &scene,
            terrain: &terrain,
            mask: &mask,
            water,
            zones: &zones,
            policy: super::super::bounded_cut::CutPolicy::from_style(&style).0,
        };
        let node_levels =
            super::super::auto_roads::levels(&verified, &verified.entities()).unwrap();

        // The frozen table. Widths come from the frozen hierarchy (profile-local
        // 12 m, profile-collector 16 m), not from an invented tier.
        let corridors: [(&str, [i32; 2], [i32; 2]); 6] = [
            ("c1", [416, 160], [480, 160]),
            ("c2", [288, 224], [224, 224]),
            ("c3", [288, 224], [288, 96]),
            ("c4", [288, 96], [288, 160]),
            ("c5", [224, 288], [224, 224]),
            ("c6", [160, 416], [160, 480]),
        ];
        let widths = [12_u32, 16_u32];
        let started = std::time::Instant::now();
        let mut states = 0_usize;
        let mut realizations = 0_usize;
        let mut rows: Vec<serde_json::Value> = vec![];
        let mut best: Option<serde_json::Value> = None;
        for (corridor, start, end) in corridors {
            let start_level = node_levels.get(&start).copied();
            // The live search pins an endpoint that is already a node and only
            // lets `infer_end` choose a plateau for a genuinely new endpoint.
            // Reproduce exactly that, and only propose a second plateau for a
            // free endpoint.
            let end_is_node = node_levels.contains_key(&end);
            let control_level = node_levels.get(&end).copied();
            for width in widths {
                let mut variants: Vec<(&str, Option<i32>)> = vec![("live_control", control_level)];
                if !end_is_node {
                    variants.push(("start_junction_level", start_level));
                }
                for (variant, level_mm) in variants {
                    states += 1;
                    let r = CorridorRequest {
                        scene_id: scene.scene_id.clone(),
                        start,
                        end,
                        road_width_m: width,
                        shoulder_width_m: 3,
                        max_fill_mm: 6000,
                        max_rise_mm_per_m: 80,
                        start_level_mm: start_level,
                        end_level_mm: level_mm,
                        junction_length_m: 8,
                    };
                    let mut row = serde_json::json!({
                        "corridor": corridor,
                        "start": start,
                        "end": end,
                        "widthM": width,
                        "endpointPlateau": variant,
                        "endIsExistingNode": end_is_node,
                        "proposedEndLevelMm": level_mm,
                        "startLevelMm": start_level,
                    });
                    // (1) production corridor design + append.
                    let id = format!("generated.road-supply.g1.{corridor}.{width}.{variant}");
                    // Exactly like the live search: the candidate is staged on the
                    // committed *roads-only* plan, so the frontage parcels are
                    // re-derived at realization time instead of being carried into
                    // the candidate plan as obstacles.
                    let mut staged = plan.clone();
                    let appended = (|| -> Result<i32, crate::baked_terrain::TerrainError> {
                        let request = ctx.infer_end(&r)?;
                        let level = request.end_level_mm.unwrap_or_default();
                        super::super::road_extensions::append_one(
                            &mut staged,
                            &ctx,
                            &request,
                            id.clone(),
                        )?;
                        Ok(level)
                    })();
                    match appended {
                        Err(error) => {
                            row["stage"] = serde_json::json!("design_refused");
                            row["reason"] = serde_json::json!(error.0);
                            rows.push(row);
                            continue;
                        }
                        Ok(inferred_level) => {
                            row["inferredEndLevelMm"] = serde_json::json!(inferred_level);
                        }
                    }
                    let (staged_road_ids, staged_edges) = (
                        staged
                            .extensions
                            .iter()
                            .map(|e| e.entity_id.clone())
                            .collect::<Vec<_>>(),
                        staged.edges.len(),
                    );
                    assert_eq!(staged_edges, 36, "the candidate adds exactly one road");
                    // (2) existing frontage measurement, demand-budget shared.
                    let mut probe = staged.clone();
                    probe.roadside_sites.extend(session.occupied());
                    let Some(road) = probe
                        .entities()
                        .into_iter()
                        .find(|entity| entity.entity_id == id)
                    else {
                        row["stage"] = serde_json::json!("measurement_unavailable");
                        rows.push(row);
                        continue;
                    };
                    let mut trial = session.clone();
                    let before = trial.realized_totals();
                    let mut first_reason = String::new();
                    let mut station_attempts = 0_u32;
                    for index in 0..trial.budgets.len() {
                        if trial.budgets[index].satisfied() || !trial.budgets[index].budget_left() {
                            continue;
                        }
                        let measured = supply
                            .measure_road_for_request(
                                &probe,
                                &road,
                                &mut trial.budgets[index],
                                super::super::auto_roads::SUPPLY_SAMPLE_CAP,
                            )
                            .unwrap();
                        station_attempts += measured.trace.attempted;
                        if first_reason.is_empty() {
                            first_reason = measured
                                .trace
                                .rejection_details
                                .first()
                                .map(|detail| detail.reason.clone())
                                .unwrap_or_else(|| "no_station".into());
                        }
                    }
                    let gains: Vec<usize> = trial
                        .realized_totals()
                        .iter()
                        .zip(&before)
                        .map(|((_, after), (_, prior))| after.saturating_sub(*prior))
                        .collect();
                    let total: usize = gains.iter().sum();
                    row["stage"] = serde_json::json!("measured");
                    row["predictedGains"] = serde_json::json!(gains);
                    row["predictedTotal"] = serde_json::json!(total);
                    row["firstReason"] = serde_json::json!(first_reason);
                    row["stationAttempts"] = serde_json::json!(station_attempts);
                    row["stagedRoadIds"] = serde_json::json!(staged_road_ids);
                    // (3) the full realization chain only for predicted gains.
                    if total == 0 {
                        rows.push(row);
                        continue;
                    }
                    if realizations >= 4 || started.elapsed().as_secs_f64() >= 180.0 {
                        row["stage"] = serde_json::json!("realization_budget_exhausted");
                        rows.push(row);
                        continue;
                    }
                    realizations += 1;
                    let mut verification = staged.clone();
                    let outcome = (|| -> Result<Realization, String> {
                        frontage::append(
                            &mut verification,
                            &scene,
                            &terrain,
                            water,
                            Some(&zones),
                            &style,
                        )
                        .map_err(|error| error.0)?;
                        realize_candidate(
                            &scene,
                            &terrain,
                            water,
                            &zones,
                            &verification,
                            super::super::NaturalSource::AuthoredSeed {
                                seed,
                                precise: true,
                            },
                            &trusted,
                            &style,
                        )
                    })();
                    match outcome {
                        Err(reason) => {
                            row["stage"] = serde_json::json!("realize_refused");
                            row["reason"] = serde_json::json!(reason);
                        }
                        Ok(realization) => {
                            let decision = super::super::auto_roads::commit_decision(
                                &session,
                                &realization,
                                &baseline,
                                &verification,
                            );
                            let new_parcels: Vec<String> = realization
                                .parcel_by_entity
                                .iter()
                                .filter(|(entity, _)| {
                                    !baseline.parcel_by_entity.contains_key(*entity)
                                })
                                .map(|(entity, parcel)| format!("{entity} -> {parcel}"))
                                .collect();
                            row["stage"] = serde_json::json!("realized");
                            row["finalSource"] = serde_json::json!(realization.final_source);
                            row["realizedWithAccess"] =
                                serde_json::json!(realization.realized_with_access);
                            row["realizedByRequest"] =
                                serde_json::json!(realization.realized_by_request);
                            row["newParcels"] = serde_json::json!(new_parcels);
                            row["yardUnresolvedM2"] =
                                serde_json::json!(realization.yard_unresolved_m2);
                            row["roadStatus"] = serde_json::json!(realization.road_status);
                            row["nodeStatus"] = serde_json::json!(realization.node_status);
                            row["requiredUnconnected"] =
                                serde_json::json!(realization.required_unconnected);
                            row["commit"] = match &decision {
                                Ok(()) => serde_json::json!("accepted"),
                                Err(error) => serde_json::json!(format!("refused:{error}")),
                            };
                            if decision.is_ok() && best.is_none() {
                                let baseline_ids: Vec<String> = baseline.realized_ids.clone();
                                let kept = baseline_ids
                                    .iter()
                                    .all(|id| realization.realized_ids.contains(id));
                                best = Some(serde_json::json!({
                                    "corridor": corridor,
                                    "widthM": width,
                                    "endpointPlateau": variant,
                                    "proposedEndLevelMm": level_mm,
                                    "inferredEndLevelMm": row["inferredEndLevelMm"],
                                    "newRoadId": id,
                                    "baselineEdges": 35,
                                    "candidateEdges": verification.edges.len(),
                                    "baselineObligations": obligation_ids.len(),
                                    "baselineRequested": baseline.frontage_requested,
                                    "baselineRealizedWithAccess": baseline.realized_with_access,
                                    "realizedWithAccess": realization.realized_with_access,
                                    "realizedByRequest": realization.realized_by_request,
                                    "newParcels": new_parcels,
                                    "yardUnresolvedM2": realization.yard_unresolved_m2,
                                    "yardUnresolvedByParcel": realization.yard_unresolved_by_parcel,
                                    "baselineYardUnresolvedByParcel": baseline.yard_unresolved_by_parcel,
                                    "baselineIdentitiesKept": kept,
                                    "finalSource": realization.final_source,
                                    "roadStatus": realization.road_status,
                                    "nodeStatus": realization.node_status,
                                    "generalNodeStatus": realization.node_status,
                                    "requiredUnconnected": realization.required_unconnected,
                                }));
                            }
                        }
                    }
                    rows.push(row);
                }
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "glyphweave.b01-g1.feasibility-run",
                "table": "bevy/work/collaboration/ds-b01-g1-feasible-increment/candidate-table.json",
                "baselineEdges": 35,
                "baselineObligations": obligation_ids.len(),
                "baselineRequested": baseline.frontage_requested,
                "baselineRealizedWithAccess": baseline.realized_with_access,
                "statesRun": states,
                "fullRealizations": realizations,
                "seconds": started.elapsed().as_secs_f64(),
                "bestLegalCandidate": best,
                "rows": rows,
            }))
            .unwrap()
        );
    }

    /// The bounded demand-led joint solve on the frozen W1 input. The authored
    /// demand, seed, terrain, zoning and fixed roots are untouched: only the
    /// automatic road program is raised to version 3 by an explicit policy.
    /// One deterministic reproduction of the located carve conflict inside the
    /// frozen 35-edge baseline, with the geometry corrected and every
    /// contributing carve listed. Read-only: no rule is changed.
    #[test]
    #[ignore = "carve diagnosis: reads the frozen W1 baseline, writes nothing"]
    fn frozen_w1_carve_conflict_is_reproduced_and_located() {
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style,
            plan,
            ..
        } = frozen_w1_stage(1, 0);
        let point = [168_i32, 220_i32];
        let road = plan
            .entities()
            .into_iter()
            .find(|e| e.entity_id == "generated.road-auto.scene-0.0")
            .expect("the frozen auto collector");
        let profile = road.grounding.road_profile.clone().unwrap();
        let lateral = (point[0] - road.world_x).abs();
        let (flat_w, _flat_d) = super::super::graded_flat_top_half_extents(&road);
        let (transport_w, _transport_d) = super::super::graded_transport_half_extents(&road);
        let (pavement_w, _) = super::super::graded_pavement_half_extents(&road);
        let layer = if lateral <= pavement_w {
            "pavement"
        } else if lateral <= flat_w {
            "graded_flat_top"
        } else if lateral <= transport_w {
            "transport_shield_outside_flat_top"
        } else {
            "outer_transition"
        };
        // The candidate that was refused: the production station 66, side +1
        // parcel of the same road.
        let request = frontage::from_style(&style)
            .unwrap()
            .unwrap()
            .requests
            .into_iter()
            .find(|r| r.id == "b04c1-homes")
            .unwrap();
        let site = frontage::candidate_site(&road, &request, &scene.scene_id, 66, 1).unwrap();
        let mut baseline_plan = plan.clone();
        baseline_plan.frontage = None;
        let mut candidate_plan = baseline_plan.clone();
        candidate_plan.roadside_sites.push(site.clone());
        let sample = |p: &RoadNetworkPlan| -> i32 {
            let mut entities = p.entities();
            entities.extend(p.site_entities());
            for e in &mut entities {
                e.normalize_spatial_semantics();
            }
            let carves = super::super::plan_terrain_carves(&entities);
            let base = f64::from(terrain.height(point[0], point[1]).unwrap());
            (super::super::apply_terrain_carves(base, point[0], point[1], &carves) * 1000.).round()
                as i32
        };
        let natural = terrain.height_millimetres(point[0], point[1]).unwrap();
        let before = sample(&baseline_plan);
        let after = sample(&candidate_plan);
        assert_eq!(
            sample(&baseline_plan),
            before,
            "the sample is deterministic"
        );
        assert_eq!(
            sample(&candidate_plan),
            after,
            "the sample is deterministic"
        );
        // Every carve whose own envelope covers or blends the point.
        let mut contributors = vec![];
        for entity in baseline_plan
            .entities()
            .iter()
            .chain(baseline_plan.site_entities().iter())
            .chain(std::iter::once(&site.entity()))
        {
            for carve in super::super::plan_terrain_carves(std::slice::from_ref(entity)) {
                let outside = super::super::carve_outside_m(
                    &carve,
                    f64::from(point[0]),
                    f64::from(point[1]),
                );
                if outside < carve.blend_m {
                    contributors.push(serde_json::json!({
                        "owner": entity.entity_id,
                        "kind": entity.kind,
                        "priority": carve.priority,
                        "flatHalf": [carve.half_w, carve.half_d],
                        "blendM": carve.blend_m,
                        "outsideM": outside,
                        "targetMm": (carve.target_h_m * 1000.).round() as i64,
                        "hasProfile": carve.profile.is_some(),
                        "inCandidate": entity.entity_id == site.id,
                    }));
                }
            }
        }
        let report = serde_json::json!({
            "model": "carve-conflict-located-in-frozen-baseline-v1",
            "point": point,
            "road": {
                "id": road.entity_id,
                "centre": [road.world_x, road.world_z],
                "widthM": road.depth_m,
                "pavementHalfM": pavement_w,
                "gradedFlatTopHalfM": flat_w,
                "transportHalfM": transport_w,
                "lateralDistanceM": lateral,
                "layer": layer,
            },
            "values": { "naturalMm": natural, "finalWithoutCandidateMm": before, "finalWithCandidateMm": after },
            "contributingCarves": contributors,
            "designLevelAtPointMm": (profile.height_at(f64::from(point[0]), f64::from(point[1])).unwrap() * 1000.).round() as i64,
            "note": "no rule is changed here; this is the minimal reproduction the follow-up design proposal refers to",
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    /// The bounded demand-led joint solve on the frozen W1 input. The authored
    /// demand, seed, terrain, zoning and fixed roots are untouched: only the
    /// automatic road program is raised to version 3 by an explicit policy.
    #[test]
    #[ignore = "bounded joint solve: reads the frozen W1 manifest, writes nothing"]
    fn frozen_w1_demand_led_solve_reports_real_verified_parcels() {
        let states = std::env::var("GLYPHWEAVE_W1_SUPPLY_STATES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(128);
        let started = std::time::Instant::now();
        let Stage {
            scene,
            terrain,
            water,
            zones,
            style,
            mut plan,
            base_edges,
            frozen_plan,
            ..
        } = frozen_w1_stage(3, states);
        let planning_seconds = started.elapsed().as_secs_f64();
        let base_nodes = frozen_plan["nodes"].as_array().unwrap().len();
        // The append phase must not disturb the frozen 35-edge baseline: same
        // node set, same base/explicit/auto edges, same obligations.
        let mut baseline_nodes: Vec<(usize, i32, i32)> = plan
            .nodes
            .iter()
            .map(|n| (n.id, n.world_x, n.world_z))
            .collect();
        baseline_nodes.sort();
        let mut frozen_nodes: Vec<(usize, i32, i32)> = frozen_plan["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                (
                    n["id"].as_u64().unwrap() as usize,
                    n["worldX"].as_i64().unwrap() as i32,
                    n["worldZ"].as_i64().unwrap() as i32,
                )
            })
            .collect();
        frozen_nodes.sort();
        assert_eq!(baseline_nodes, frozen_nodes, "the frozen node set is fixed");
        // R8-1: 20 - see the refusal record in the plan's roadside_site_rejections.
        // R11 (note 2): 21 - the station scan places the pad where its own
        // frontage is walkable (see the assertion above).
        assert_eq!(plan.roadside_sites.len(), 21, "the obligations are fixed");
        let mut baseline_edges: Vec<(String, usize, usize, u32, i64)> = plan
            .edges
            .iter()
            .map(|e| {
                (
                    e.id.clone(),
                    e.from,
                    e.to,
                    e.length_m,
                    (e.rotation_y_deg * 1000.).round() as i64,
                )
            })
            .collect();
        baseline_edges.sort();
        let mut frozen_edges: Vec<(String, usize, usize, u32, i64)> = frozen_plan["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["id"].as_str().unwrap().to_string(),
                    e["from"].as_u64().unwrap() as usize,
                    e["to"].as_u64().unwrap() as usize,
                    e["lengthM"].as_u64().unwrap() as u32,
                    (e["rotationYDeg"].as_f64().unwrap() * 1000.).round() as i64,
                )
            })
            .collect();
        frozen_edges.sort();
        for edge in &frozen_edges {
            assert!(
                baseline_edges.contains(edge),
                "the frozen edge {} must survive the append phase",
                edge.0
            );
        }
        assert!(
            baseline_edges.len() >= frozen_edges.len(),
            "only additions are allowed on top of the frozen baseline"
        );
        let roads_before = plan.edges.len();
        assert!(
            roads_before > base_edges,
            "version 3 must still start from the frozen base plan"
        );
        let automatic = plan.automatic_extensions.clone();
        let before_frontage = plan.clone();
        let frontage_started = std::time::Instant::now();
        frontage::append(&mut plan, &scene, &terrain, water, Some(&zones), &style).unwrap();
        let frontage_seconds = frontage_started.elapsed().as_secs_f64();
        let outcomes: Vec<serde_json::Value> = plan
            .frontage
            .as_ref()
            .unwrap()
            .outcomes
            .iter()
            .map(|outcome| {
                serde_json::json!({
                    "requestId": outcome.request.id,
                    "count": outcome.request.count,
                    "planned": outcome.site_ids.len(),
                    "unfilled": outcome.unfilled,
                    "attempted": outcome.attempted,
                    "rejected": outcome.rejected,
                })
            })
            .collect();
        let planned: usize = outcomes
            .iter()
            .map(|o| o["planned"].as_u64().unwrap() as usize)
            .sum();
        let requested: usize = outcomes
            .iter()
            .map(|o| o["count"].as_u64().unwrap() as usize)
            .sum();
        // The authored demand must survive the strategy untouched: same ids,
        // counts and exact dimensions, and the same effective program the
        // frozen world was built from.
        let requests: Vec<(String, u32, u32, u32, u32)> = frontage::from_style(&style)
            .unwrap()
            .unwrap()
            .requests
            .into_iter()
            .map(|r| (r.id, r.count, r.width_m, r.depth_m, r.station_spacing_m))
            .collect();
        assert_eq!(
            requests,
            vec![
                ("b04c1-homes".into(), 16, 12, 10, 24),
                ("b04c1-shops".into(), 8, 12, 10, 24),
            ],
            "author demand must be preserved exactly"
        );
        for site in plan
            .roadside_sites
            .iter()
            .filter(|site| site.program.is_some())
        {
            assert_eq!(site.width_m, 12);
            assert_eq!(site.depth_m, 10);
            let parcel = frontage::parcel_boundary(site).unwrap();
            assert_eq!((parcel[2] - parcel[0]) * (parcel[3] - parcel[1]), 16 * 18);
        }
        // Every planned parcel carries a real entrance: the production
        // preflight only accepts a site when the pedestrian solver connects it
        // to its own road on the same engineered platform.
        let entrances = plan
            .roadside_sites
            .iter()
            .filter(|site| site.program.is_some())
            .filter(|site| site.frontage_road_point.is_some() && site.support_level_mm.is_some())
            .count();
        let report = serde_json::json!({
            "input": {
                "manifest": frozen_path().display().to_string(),
                "supplyStates": states,
                "frozenBaseEdges": base_edges,
                "frozenEdges": frozen_edges.len(),
                "phaseInputEdges": roads_before,
                "phaseInputNodes": frozen_nodes.len(),
                "phaseInputSites": 21,
                "baseNodesFrozen": base_nodes,
            },
            "automatic": {
                "mode": "append",
                "baselinePreserved": true,
                "baselineRoadIds": automatic
                    .as_ref()
                    .and_then(|a| a.base_road_program.as_ref())
                    .map(|b| b.generated_road_ids.clone()),
                "creditedRoadIds": automatic.as_ref().map(|a| a.generated_road_ids.clone()),
                "stopReason": automatic.as_ref().map(|a| a.stop_reason.clone()),
                "attemptedCandidates": automatic.as_ref().map(|a| a.attempted_candidates),
                "acceptedSegments": automatic.as_ref().map(|a| a.accepted_segments),
                "rejectedByReason": automatic.as_ref().map(|a| a.rejected_by_reason.clone()),
                "rejectionExamples": automatic.as_ref().map(|a| a.rejection_examples.clone()),
                "supply": automatic.as_ref().map(|a| a.supply.clone()),
                "sweep": automatic.as_ref().map(|a| a.sweep.clone()),
                "sweepExhausted": automatic.as_ref().map(|a| a.sweep_exhausted),
                "incomplete": automatic.as_ref().map(|a| a.incomplete),
                "sweepWidthsM": automatic
                    .as_ref()
                    .map(|a| a.sweep.iter().map(|e| e.width_m).collect::<BTreeSet<_>>()),
                "sweepLengthsM": automatic.as_ref().map(|a| {
                    a.sweep
                        .iter()
                        .map(|e| (e.start[0] - e.end[0]).abs().max((e.start[1] - e.end[1]).abs()) as u32)
                        .collect::<BTreeSet<_>>()
                }),
                "sweepNodes": automatic
                    .as_ref()
                    .map(|a| a.sweep.iter().map(|e| e.start).collect::<BTreeSet<_>>().len()),
            },
            "frontage": outcomes,
            "planned": planned,
            "requested": requested,
            "parcelsWithEntrance": entrances,
            "edgesAfterSolve": plan.edges.len(),
            "edgesBeforeFrontage": before_frontage.edges.len(),
            "roadsBeforeFrontage": roads_before,
            "planningSeconds": planning_seconds,
            "frontageSeconds": frontage_seconds,
            "witness24": planned == requested && requested == 24 && entrances >= 24,
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    /// The read-only diagnosis rechecks ONE PUBLISHED world under the current
    /// exact support geometry. It must preserve the production search order and
    /// reject legacy parcels that the older AABB test let stand on road support.
    ///
    /// This is the regression gate for the candidate order shared with
    /// `frontage::append`. While the diagnosis walked request-major it reported
    /// 50 homes / 317 shop attempts on this world against the published
    /// 43 / 176, so a downstream judgement could trust a search the bake never
    /// performed. The world is a `bevy/work` artifact and the suite must not
    /// depend on it, so the test skips when the directory is absent.
    ///
    /// Round 31 moved the fixture one directory along, to the world the round's own 1 km probe baked
    /// (`bevy/work/r31/1km-b8-cut`). The current fixture is r32l. Replay reconstructs the
    /// pre-frontage boundary by clearing the post-frontage `road_cut_mm` field and restores the
    /// committed natural range on accepted parcels. The previous artifact was baked under the
    /// round-30 policy, whose
    /// conversions were free to stand anywhere in the fields; this build refuses that land by name, so
    /// a stale world can only disagree with its own replay about conversions it no longer permits. The
    /// assertions are unchanged - what moved is which published report they are held against.
    #[test]
    fn diagnosis_rechecks_a_published_world_under_exact_support_geometry() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../work/r31/1km-b8-cut-r32l-current");
        if !root.exists() {
            return;
        }
        let manifest: super::super::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).expect("published manifest"),
        )
        .expect("manifest json");
        let scene = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&scene.scene_id);
        let plan: RoadNetworkPlan = serde_json::from_slice(
            &std::fs::read(dir.join("road-network-plan.json")).expect("published road plan"),
        )
        .expect("plan json");
        let zones: Vec<LandUseZone> = serde_json::from_slice(
            &std::fs::read(dir.join("land-use-zones.json")).expect("published zoning"),
        )
        .expect("zoning json");
        let mask = ZoningMask::new(&scene, &zones).expect("zoning mask");
        let mut natural = BakedTerrain::new(&scene).expect("raster");
        for chunk_z in 0..scene.depth_m.div_ceil(512) {
            for chunk_x in 0..scene.width_m.div_ceil(512) {
                let dx = chunk_x * 512;
                let dz = chunk_z * 512;
                let width = (scene.width_m - dx).min(512);
                let depth = (scene.depth_m - dz).min(512);
                let bytes = std::fs::read(
                    dir.join(format!("chunk-{chunk_x}-{chunk_z}.natural.height.bin")),
                )
                .expect("published natural chunk");
                natural
                    .insert_chunk(
                        scene.origin_x + dx as i32,
                        scene.origin_z + dz as i32,
                        width,
                        depth,
                        &bytes,
                    )
                    .expect("chunk insert");
            }
        }
        natural.require_complete().expect("complete raster");
        assert_eq!(
            natural.content_hash().expect("raster hash"),
            plan.natural_heightfield_hash,
            "the replay must use the published natural surface"
        );
        assert_eq!(
            plan.zoning_hash.as_deref(),
            Some(mask.hash.as_str()),
            "the replay must use the published zoning"
        );
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            &scene,
            &manifest.style,
        );
        let style = baked_style(&manifest.style, &scene);
        let requests: Vec<Request> = frontage::from_style(&style)
            .expect("frontage program")
            .expect("frontage program present")
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene.scene_id)
            .collect();
        let published = plan.frontage.as_ref().expect("published frontage report");
        // The replay starts from the plan `append` saw: strip exactly the parcels
        // this report placed, and keep every other pre-existing program site.
        let placed: BTreeSet<String> = published
            .outcomes
            .iter()
            .flat_map(|outcome| outcome.site_ids.iter().cloned())
            .collect();
        let mut stripped = plan.clone();
        stripped
            .roadside_sites
            .retain(|site| !placed.contains(&site.id));
        stripped.frontage = None;
        stripped.road_cut_mm = None;
        // The bounded-cut policy is part of the search, not a diagnostic setting: a lot the authored
        // style may bench is accepted by the bake and refused by a replay that assumed no cut. The
        // policy is therefore read from the SAME style the world was baked from, exactly as `append`
        // reads it, so the replay cannot disagree with the bake about which lots are buildable.
        let (policy, _not_enabled) = super::super::bounded_cut::CutPolicy::from_style(&style);
        let diagnosis = diagnose(
            &scene, &natural, &mask, water, &zones, &stripped, &requests, 0, policy,
        )
        .expect("diagnosis");
        assert_eq!(
            diagnosis.requests.len(),
            published.outcomes.len(),
            "one summary per published request"
        );
        assert_eq!(
            published.outcomes.len(),
            2,
            "this world carries the homes and shops requests"
        );
        for (summary, outcome) in diagnosis.requests.iter().zip(&published.outcomes) {
            assert_eq!(summary.request_id, outcome.request.id);
            assert_eq!(
                summary.joint_attempted, outcome.attempted,
                "candidate attempts for {}",
                outcome.request.id
            );
            assert_eq!(
                summary.stop_reason, outcome.stop_reason,
                "stop reason for {}",
                outcome.request.id
            );
        }
        // The concrete numbers this world was published with, asserted directly so
        // the regression cannot pass by both sides drifting together.
        //
        // Round 31 re-pinned them: the fixture is the round's own 1 km probe, and its urban edge now
        // decides how much of the authored demand can land. Exact oriented support geometry replaced
        // the old AABB acceptance order: one legacy home and one legacy shop stood on road support,
        // so current replay refuses them and spends those slots on the exact-geometry candidates
        // below. Both requests still stop with no feasible frontage left on the city's edge.
        let by_id: BTreeMap<&str, &RequestSummary> = diagnosis
            .requests
            .iter()
            .map(|summary| (summary.request_id.as_str(), summary))
            .collect();
        let homes = by_id["urban-homes"];
        assert_eq!(
            (homes.joint_accepted, homes.joint_attempted),
            (3, 380),
            "homes: planned / attempted"
        );
        assert_eq!(
            homes.rejected,
            BTreeMap::from([
                ("bounded_cut_exceeds_cap".to_string(), 10),
                ("bounded_cut_footprint_overlaps_another_pad".to_string(), 2),
                ("bounded_cut_on_occupied_cell".to_string(), 2),
                ("conversion_outside_the_urban_fringe".to_string(), 314),
                ("existing_support_changed".to_string(), 7),
                ("no_local_walkable_frontage".to_string(), 2),
                ("occupied_or_road_support".to_string(), 22),
                ("wrong_purpose_or_protected".to_string(), 18),
            ])
        );
        assert_eq!(homes.stop_reason, "no_feasible_frontage");
        let shops = by_id["urban-shops"];
        assert_eq!(
            (shops.joint_accepted, shops.joint_attempted),
            (17, 422),
            "shops: planned / attempted"
        );
        assert_eq!(
            shops.rejected,
            BTreeMap::from([
                ("bounded_cut_exceeds_cap".to_string(), 4),
                ("bounded_cut_footprint_overlaps_another_pad".to_string(), 10),
                ("bounded_cut_on_occupied_cell".to_string(), 4),
                ("conversion_outside_the_urban_fringe".to_string(), 352),
                ("existing_support_changed".to_string(), 13),
                ("no_local_walkable_frontage".to_string(), 3),
                ("occupied_or_road_support".to_string(), 15),
                ("wrong_purpose_or_protected".to_string(), 4),
            ])
        );
        assert_eq!(shops.stop_reason, "no_feasible_frontage");
        let accepted: BTreeSet<&str> = diagnosis
            .joint
            .iter()
            .flat_map(|trace| trace.accepted_ids.iter().map(String::as_str))
            .collect();
        for legacy_road_overlap in [
            "generated.frontage.scene-0.urban-homes.generated.road-auto.scene-0.4.20.positive",
            "generated.frontage.scene-0.urban-shops.generated.road-auto.scene-0.0.30.positive",
        ] {
            assert!(
                !accepted.contains(legacy_road_overlap),
                "exact support geometry must refuse the legacy road-overlap parcel {legacy_road_overlap}"
            );
        }
        for exact_geometry_parcel in [
            "generated.frontage.scene-0.urban-shops.generated.road-auto.scene-0.4.14.positive",
            "generated.frontage.scene-0.urban-shops.generated.road-auto.scene-0.4.46.positive",
        ] {
            assert!(
                accepted.contains(exact_geometry_parcel),
                "the current exact geometry accepts {exact_geometry_parcel}"
            );
        }
        assert_eq!(diagnosis.joint_accepted, 3 + 17);
        assert_eq!(diagnosis.joint_station_attempts, 380 + 422);
        assert_eq!(diagnosis.stop_reason, "no_feasible_frontage");
    }

    /// Read-only replay of the SAME published world under the OPT-IN bounded
    /// cut: how many frontage pads the world could plan if a frontage pad were
    /// allowed to excavate up to `maxCutMm`, and which named rejections it lost.
    ///
    /// Nothing is regenerated and nothing is written: the natural raster, the
    /// zoning and the plan come from the published bytes, and the production
    /// `frontage::append` is re-run on the stripped plan exactly as the support
    /// audit does. The only difference between the two arms is the authored
    /// `style.frontageProgram.terrainPolicy` pair, so the delta is the policy's.
    /// Prints one `BOUNDED-CUT-REPLAY-JSON:` line.
    #[test]
    #[ignore = "reads one published world from GLYPHWEAVE_SUPPORT_AUDIT_WORLD; writes nothing"]
    fn bounded_cut_offline_replay_of_a_published_world() {
        let Ok(root) = std::env::var("GLYPHWEAVE_SUPPORT_AUDIT_WORLD") else {
            eprintln!("SKIP: set GLYPHWEAVE_SUPPORT_AUDIT_WORLD to a published world directory");
            return;
        };
        let root = std::path::PathBuf::from(root);
        let manifest: super::super::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).expect("published manifest"),
        )
        .expect("manifest json");
        let scene = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&scene.scene_id);
        let plan: RoadNetworkPlan = serde_json::from_slice(
            &std::fs::read(dir.join("road-network-plan.json")).expect("published road plan"),
        )
        .expect("plan json");
        let zones: Vec<LandUseZone> = serde_json::from_slice(
            &std::fs::read(dir.join("land-use-zones.json")).expect("published zoning"),
        )
        .expect("zoning json");
        let mask = ZoningMask::new(&scene, &zones).expect("zoning mask");
        let mut natural = BakedTerrain::new(&scene).expect("raster");
        for chunk_z in 0..scene.depth_m.div_ceil(512) {
            for chunk_x in 0..scene.width_m.div_ceil(512) {
                let dx = chunk_x * 512;
                let dz = chunk_z * 512;
                let width = (scene.width_m - dx).min(512);
                let depth = (scene.depth_m - dz).min(512);
                let bytes = std::fs::read(
                    dir.join(format!("chunk-{chunk_x}-{chunk_z}.natural.height.bin")),
                )
                .expect("published natural chunk");
                natural
                    .insert_chunk(
                        scene.origin_x + dx as i32,
                        scene.origin_z + dz as i32,
                        width,
                        depth,
                        &bytes,
                    )
                    .expect("chunk insert");
            }
        }
        natural.require_complete().expect("complete raster");
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            &scene,
            &manifest.style,
        );
        let baked = baked_style(&manifest.style, &scene);
        let published = plan.frontage.as_ref().expect("published frontage report");
        let placed: BTreeSet<String> = published
            .outcomes
            .iter()
            .flat_map(|outcome| outcome.site_ids.iter().cloned())
            .collect();
        let mut stripped = plan.clone();
        stripped
            .roadside_sites
            .retain(|site| !placed.contains(&site.id));
        stripped.frontage = None;
        stripped.road_cut_mm = None;
        let mut arms: Vec<serde_json::Value> = vec![];
        let mut arm_state: Vec<(super::super::bounded_cut::CutPolicy, frontage::Report)> = vec![];
        for (label, policy) in [
            ("off", super::super::bounded_cut::CutPolicy::Disabled),
            (
                "bounded-cut",
                super::super::bounded_cut::CutPolicy::Bounded {
                    max_cut_mm: super::super::bounded_cut::MAX_CUT_MM,
                },
            ),
        ] {
            let mut style = baked.clone();
            if policy.enabled() {
                style["frontageProgram"]["terrainPolicy"] =
                    serde_json::json!(super::super::bounded_cut::POLICY_VALUE);
                style["frontageProgram"]["maxCutMm"] =
                    serde_json::json!(super::super::bounded_cut::MAX_CUT_MM);
            } else if let Some(program) = style["frontageProgram"].as_object_mut() {
                // The OFF arm must actually BE off: the compiled style carries the
                // authored `terrainPolicy`/`maxCutMm` pair, so merely leaving it alone
                // ran the bounded-cut policy in both arms and reported a delta of zero
                // for every request - a measurement that could never show the policy's
                // effect. The pair is removed here, exactly as an author removes it from
                // the manifest to switch the capability off.
                program.remove("terrainPolicy");
                program.remove("maxCutMm");
            }
            let mut produced = stripped.clone();
            frontage::append(&mut produced, &scene, &natural, water, Some(&zones), &style)
                .expect("production frontage append");
            let report = produced.frontage.clone().expect("replayed report");
            let rows: Vec<serde_json::Value> = report
                .outcomes
                .iter()
                .map(|outcome| {
                    serde_json::json!({
                        "requestId": outcome.request.id,
                        "demand": outcome.request.count,
                        "planned": outcome.site_ids.len(),
                        "attempted": outcome.attempted,
                        "unfilled": outcome.unfilled,
                        "rejected": outcome.rejected,
                        "stopReason": outcome.stop_reason,
                    })
                })
                .collect();
            let cut_pads = produced
                .roadside_sites
                .iter()
                .filter(|site| site.bounded_cut_mm.is_some())
                .count();
            arms.push(serde_json::json!({
                "arm": label,
                "enabled": policy.enabled(),
                "maxCutMm": policy.cap_mm(),
                "padsWithACut": cut_pads,
                "requests": rows,
            }));
            arm_state.push((policy, report));
        }
        // The delta, per request, in the family the capability exists to relieve.
        let family = "natural_support_or_fill_limit";
        let mut deltas: Vec<serde_json::Value> = vec![];
        for (was, now) in arm_state[0].1.outcomes.iter().zip(&arm_state[1].1.outcomes) {
            let off = u64::from(*was.rejected.get(family).unwrap_or(&0));
            let on = u64::from(*now.rejected.get(family).unwrap_or(&0));
            deltas.push(serde_json::json!({
                "requestId": was.request.id,
                "plannedOff": was.site_ids.len(),
                "plannedOn": now.site_ids.len(),
                "familyRejectedOff": off,
                "familyRejectedOn": on,
                "familyDrop": off.saturating_sub(on),
                "capRejectedOn": now.rejected.get("bounded_cut_exceeds_cap").copied().unwrap_or(0),
                "rejectedOn": now.rejected,
            }));
        }
        let report = serde_json::json!({
            "world": root.display().to_string(),
            "scene": scene.scene_id,
            "maxCutMm": super::super::bounded_cut::MAX_CUT_MM,
            "arms": arms,
            "deltas": deltas,
        });
        println!(
            "BOUNDED-CUT-REPLAY-JSON: {}",
            serde_json::to_string(&report).unwrap()
        );
    }

    /// Read-only support-gate audit of ONE PUBLISHED world.
    ///
    /// It loads the published natural raster chunks, the published zoning and the
    /// published road plan, replays the production frontage search through
    /// `diagnose` (which shares every candidate constructor and every preflight
    /// decision with `frontage::append`), and then splits the
    /// `natural_support_or_fill_limit` family by the sub-condition each candidate
    /// actually violates:
    ///
    /// * `natural_above_design_level`  - a sample is above the pad's design level (a cut)
    /// * `fill_exceeds_roadbed_limit`  - fill > `maxRoadbedFillM` (6 m)
    /// * `fill_exceeds_zoning_capacity`- fill > the zoning taper capacity at the sample
    ///
    /// Nothing is regenerated and nothing is written: the raster is the published
    /// chunk set (its content hash must equal `naturalHeightfieldHash`), and the
    /// zoning hash must equal `zoningHash`, so the replay provably uses the same
    /// inputs the published plan used. Prints one `SUPPORT-AUDIT-JSON:` line.
    #[test]
    #[ignore = "reads one published world from GLYPHWEAVE_SUPPORT_AUDIT_WORLD; writes nothing"]
    fn support_gate_audit_of_a_published_world() {
        let Ok(root) = std::env::var("GLYPHWEAVE_SUPPORT_AUDIT_WORLD") else {
            eprintln!("SKIP: set GLYPHWEAVE_SUPPORT_AUDIT_WORLD to a published world directory");
            return;
        };
        let root = std::path::PathBuf::from(root);
        let manifest: super::super::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).expect("published manifest"),
        )
        .expect("manifest json");
        let scene = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&scene.scene_id);
        let plan: RoadNetworkPlan = serde_json::from_slice(
            &std::fs::read(dir.join("road-network-plan.json")).expect("published road plan"),
        )
        .expect("plan json");
        let zones: Vec<LandUseZone> = serde_json::from_slice(
            &std::fs::read(dir.join("land-use-zones.json")).expect("published zoning"),
        )
        .expect("zoning json");
        let mask = ZoningMask::new(&scene, &zones).expect("zoning mask");
        let mut natural = BakedTerrain::new(&scene).expect("raster");
        for chunk_z in 0..scene.depth_m.div_ceil(512) {
            for chunk_x in 0..scene.width_m.div_ceil(512) {
                let dx = chunk_x * 512;
                let dz = chunk_z * 512;
                let width = (scene.width_m - dx).min(512);
                let depth = (scene.depth_m - dz).min(512);
                let bytes = std::fs::read(
                    dir.join(format!("chunk-{chunk_x}-{chunk_z}.natural.height.bin")),
                )
                .expect("published natural chunk");
                natural
                    .insert_chunk(
                        scene.origin_x + dx as i32,
                        scene.origin_z + dz as i32,
                        width,
                        depth,
                        &bytes,
                    )
                    .expect("chunk insert");
            }
        }
        natural.require_complete().expect("complete raster");
        let natural_hash = natural.content_hash().expect("raster hash");
        let natural_hash_matches = natural_hash == plan.natural_heightfield_hash;
        let zoning_hash_matches = plan.zoning_hash.as_deref() == Some(mask.hash.as_str());
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            &scene,
            &manifest.style,
        );
        let style = baked_style(&manifest.style, &scene);
        let requests: Vec<Request> = frontage::from_style(&style)
            .expect("frontage program")
            .expect("frontage program present")
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene.scene_id)
            .collect();
        let published = plan.frontage.as_ref().expect("published frontage report");
        // The requests the published plan actually ran must be the ones this style
        // declares, identity for identity; otherwise the replay would audit a
        // different demand than the world was built from.
        let request_identity_matches = requests.len() == published.outcomes.len()
            && requests.iter().zip(&published.outcomes).all(|(r, o)| {
                serde_json::to_value(r).unwrap() == serde_json::to_value(&o.request).unwrap()
            });
        // 1. Production replay: reconstruct the exact PRE-frontage plan from the
        //    published receipts and call the same `frontage::append` entry point the
        //    bake calls. `plan_with_program` runs `joint_frontage::plan` (service
        //    program version 2) BEFORE frontage, so the base roadside sites, the
        //    service sites and the joint layout's housing-recovery sites are already
        //    present when `append` starts; the only parcels to remove are the ones the
        //    frontage report itself placed. Identical per-request outcomes prove the
        //    stripped plan is exactly that state. Nothing is regenerated or written.
        let placed: BTreeSet<String> = published
            .outcomes
            .iter()
            .flat_map(|outcome| outcome.site_ids.iter().cloned())
            .collect();
        let mut stripped = plan.clone();
        stripped
            .roadside_sites
            .retain(|site| !placed.contains(&site.id));
        stripped.frontage = None;
        stripped.road_cut_mm = None;
        let mut produced = stripped.clone();
        frontage::append(&mut produced, &scene, &natural, water, Some(&zones), &style)
            .expect("production frontage append");
        let produced_report = produced.frontage.as_ref().expect("replayed report");
        let mut production_matches = published.outcomes.len() == produced_report.outcomes.len();
        let mut production_rows: Vec<serde_json::Value> = vec![];
        for (was, now) in published.outcomes.iter().zip(&produced_report.outcomes) {
            let identical = was.request.id == now.request.id
                && was.attempted == now.attempted
                && was.unfilled == now.unfilled
                && was.rejected == now.rejected
                && was.site_ids == now.site_ids
                && was.stop_reason == now.stop_reason;
            production_matches &= identical;
            production_rows.push(serde_json::json!({
                "requestId": now.request.id,
                "publishedAttempted": was.attempted,
                "replayedAttempted": now.attempted,
                "publishedAccepted": was.site_ids.len(),
                "replayedAccepted": now.site_ids.len(),
                "publishedRejected": was.rejected,
                "replayedRejected": now.rejected,
                "identical": identical,
            }));
        }

        // 2. Shadow replay: the SAME loops and the SAME functions as `frontage::append`,
        //    with the structured rejection evidence kept. `append` itself discards that
        //    evidence, and it must not grow a published field (the plan is contracted),
        //    so the audit duplicates the loop here and proves below that it reproduces
        //    the production selection identity for identity.
        #[derive(Clone, Default)]
        struct SupportTruth {
            cut: bool,
            fill_roadbed: bool,
            fill_capacity: bool,
            cut_mm: i64,
            fill_mm: i64,
            natural_min_mm: i64,
            natural_max_mm: i64,
            raise_delta_mm: i64,
            raise_legal: bool,
            /// Fill the pad would need if it were raised to the highest ground it
            /// covers, and whether that raised pad still fits the zoning taper.
            raise_fill_mm: i64,
            raise_capacity_ok: bool,
            /// Natural ground above the design level AT the frontage point on the
            /// road: non-zero would mean the road's own datum is in cut.
            road_point_cut_mm: i64,
            /// The same at the pad's entrance anchor, 20 m off the centreline.
            entrance_cut_mm: i64,
        }
        struct Attempt {
            request: String,
            reason: String,
            note: String,
            side: i32,
            truth: SupportTruth,
        }
        let roads = frontage::profile_roads(&stripped);
        let mut state = PlacementState::new(
            &natural,
            &mask,
            water,
            {
                let mut existing = roads.clone();
                existing.extend(stripped.site_entities());
                existing
            },
            super::super::bounded_cut::CutPolicy::from_style(&style).0,
        );
        let mut staged = stripped.clone();
        let mut budgets: Vec<RequestBudget> = requests.iter().map(RequestBudget::new).collect();
        let mut attempts: Vec<Attempt> = vec![];
        let max_fill_mm = (plan.max_roadbed_fill_m * 1000.).round() as i64;
        let apron_rise_mm =
            (super::super::access::WALKABLE_GRADE * super::super::ENTRANCE_APRON_RUN_M * 1000.)
                .round() as i64;
        // The ORDER is the bake's own, taken from the function `append` walks rather
        // than re-derived here. With an explicit rural conversion `append` re-sorts
        // both the roads and the turns inside each road nearest-the-city first
        // (`frontage::candidate_order`), and a shadow pass that rebuilt the historical
        // schedule would measure a search the bake never performs. Measured on the
        // hillside town before this used `candidate_order`: the shadow pass planned
        // 5 homes + 2 shops against the published 4 + 3, and this audit's own
        // "the shadow replay must reproduce the production selection" assertion
        // failed on every world that states the conversion policy.
        let nearest_first = requests.iter().any(|r| r.rural_conversion.is_explicit());
        let ordered = frontage::candidate_order(
            &roads.iter().collect::<Vec<_>>(),
            &requests,
            &scene.scene_id,
            nearest_first.then_some(zones.as_slice()),
        )
        .expect("candidate order");
        for (road_index, turns) in ordered {
            let road = &roads[road_index];
            for turn in turns {
                let index = turn.request;
                let (station, side) = (turn.station, turn.side);
                let budget = &mut budgets[index];
                if budget.satisfied() || !budget.budget_left() {
                    continue;
                }
                if !budget.claim_attempt() {
                    continue;
                }
                let r = &requests[index];
                let mut site = frontage::candidate_site(road, r, &scene.scene_id, station, side)
                    .expect("candidate site");
                let parcel = frontage::parcel_boundary(&site).expect("parcel");
                let (mut reason, _converted) = frontage::envelope_verdict(
                    parcel,
                    r.parcel_insets.unwrap_or_default(),
                    site.rotation_y_deg,
                    &scene,
                    &zones,
                    &r.purpose,
                    r.rural_conversion,
                );
                let mut detail = None;
                if reason.is_none() {
                    let (found, evidence) =
                        state.assess_detailed(&site, &staged).expect("preflight");
                    reason = found;
                    detail = evidence;
                }
                if let Some(reason) = reason {
                    budget.reject(reason);
                    let level = i64::from(site.support_level_mm.expect("precise"));
                    let (half_w, half_d) = crate::worldgen::support_pad_half_extents(
                        site.width_m as f32,
                        site.depth_m as f32,
                        site.rotation_y_deg,
                        site.frontage_road_point.is_some(),
                    );
                    let (x0, z0, x1, z1) = (
                        site.world_x - half_w as i32,
                        site.world_z - half_d as i32,
                        site.world_x + half_w as i32,
                        site.world_z + half_d as i32,
                    );
                    let mut truth = SupportTruth {
                        natural_min_mm: i64::MAX,
                        natural_max_mm: i64::MIN,
                        ..SupportTruth::default()
                    };
                    for z in z0..=z1 {
                        for x in x0..=x1 {
                            let Some(h) = natural.height_millimetres(x, z) else {
                                continue;
                            };
                            let h = i64::from(h);
                            truth.natural_min_mm = truth.natural_min_mm.min(h);
                            truth.natural_max_mm = truth.natural_max_mm.max(h);
                            if h > level {
                                truth.cut = true;
                                truth.cut_mm = truth.cut_mm.max(h - level);
                            } else {
                                let fill = level - h;
                                truth.fill_mm = truth.fill_mm.max(fill);
                                if fill > max_fill_mm {
                                    truth.fill_roadbed = true;
                                }
                                if fill as f64 > mask.fill_capacity_at(x, z) * 1000. {
                                    truth.fill_capacity = true;
                                }
                            }
                        }
                    }
                    if truth.natural_max_mm < truth.natural_min_mm {
                        truth.natural_min_mm = level;
                        truth.natural_max_mm = level;
                    }
                    // Where does the excess come from: is the road's own datum
                    // already below natural ground, or does the ground rise
                    // ACROSS the setback between the road and the pad?
                    if let Some([rx, rz]) = site.frontage_road_point {
                        truth.road_point_cut_mm = natural
                            .height_millimetres(rx, rz)
                            .map_or(0, |h| (i64::from(h) - level).max(0));
                    }
                    let anchor = site.entity().anchors[0].clone();
                    truth.entrance_cut_mm = natural
                        .height_millimetres(anchor.world_x, anchor.world_z)
                        .map_or(0, |h| (i64::from(h) - level).max(0));
                    // What-if: the pad raised to the highest ground it must
                    // cover, so the cut disappears and the pad becomes fill-only.
                    // That is only legal while the rise stays inside the walkable
                    // entrance run and the added fill stays inside the roadbed
                    // and zoning fill budgets. `familyRaiseLegalByRampMm` below
                    // reports how many pads each authorised ramp run would unlock.
                    let raised = level.max(truth.natural_max_mm);
                    truth.raise_delta_mm = raised - level;
                    truth.raise_fill_mm = raised - truth.natural_min_mm;
                    let mut capacity_ok = true;
                    for z in z0..=z1 {
                        for x in x0..=x1 {
                            let Some(h) = natural.height_millimetres(x, z) else {
                                continue;
                            };
                            let fill = raised - i64::from(h);
                            if fill as f64 > mask.fill_capacity_at(x, z) * 1000. {
                                capacity_ok = false;
                            }
                        }
                    }
                    truth.raise_capacity_ok = capacity_ok;
                    truth.raise_legal = truth.raise_delta_mm <= apron_rise_mm
                        && truth.raise_fill_mm <= max_fill_mm
                        && capacity_ok;
                    attempts.push(Attempt {
                        request: r.id.clone(),
                        reason: reason.into(),
                        note: detail.and_then(|d| d.note).unwrap_or_default(),
                        side,
                        truth,
                    });
                } else {
                    // The cap the pad will PUBLISH has to be on the parcel before it
                    // is committed: it is the footprint every later candidate is
                    // measured against (`frontage::append` does the same, and
                    // `attempt_turn` above says why). A shadow pass that committed
                    // the parcel without it silently stopped applying the bench
                    // overlap gate, and its selection diverged from production on
                    // every world whose pads excavate.
                    site.bounded_cut_mm = state.cut_cap_for(&site);
                    state
                        .sample_natural(&mut site, parcel)
                        .expect("natural range");
                    state.commit(&site);
                    budget.accept(&site.id);
                    staged.roadside_sites.push(site);
                }
            }
        }
        let mut shadow_matches = true;
        for (index, r) in requests.iter().enumerate() {
            let Some(now) = produced_report
                .outcomes
                .iter()
                .find(|outcome| outcome.request.id == r.id)
            else {
                shadow_matches = false;
                continue;
            };
            shadow_matches &= budgets[index].attempted == now.attempted
                && budgets[index].site_ids == now.site_ids
                && budgets[index].rejected == now.rejected;
        }
        // 3. Split the family by the sub-condition each candidate actually violates.
        let bucket = |mm: i64| -> &'static str {
            if mm <= 0 {
                "none"
            } else if mm <= 500 {
                "<=0.5m"
            } else if mm <= 1000 {
                "<=1m"
            } else if mm <= 2000 {
                "<=2m"
            } else if mm <= 4000 {
                "<=4m"
            } else {
                ">4m"
            }
        };
        let mut rows: Vec<serde_json::Value> = vec![];
        let mut totals = BTreeMap::<String, i64>::new();
        for outcome in &published.outcomes {
            let family: Vec<&Attempt> = attempts
                .iter()
                .filter(|a| a.request == outcome.request.id)
                .filter(|a| a.reason == "natural_support_or_fill_limit")
                .collect();
            let mut combos = BTreeMap::<String, u32>::new();
            let mut notes = BTreeMap::<String, u32>::new();
            let mut cut_buckets = BTreeMap::<&'static str, u32>::new();
            let mut fill_buckets = BTreeMap::<&'static str, u32>::new();
            let mut raise_buckets = BTreeMap::<&'static str, u32>::new();
            let mut side_split = BTreeMap::<&'static str, u32>::new();
            // Always present, so a zero is reported as a zero instead of vanishing.
            let mut conditions = BTreeMap::<&'static str, u32>::from([
                ("cut", 0),
                ("fill_roadbed", 0),
                ("fill_capacity", 0),
            ]);
            let mut max_cut = 0_i64;
            let mut max_fill = 0_i64;
            let mut raise_legal = 0_u32;
            let mut road_point_cut = 0_u32;
            let mut entrance_cut = 0_u32;
            let mut entrance_cut_buckets = BTreeMap::<&'static str, u32>::new();
            for attempt in &family {
                let truth = &attempt.truth;
                let mut needs: Vec<&str> = vec![];
                if truth.cut {
                    needs.push("cut");
                }
                if truth.fill_roadbed {
                    needs.push("fill_roadbed");
                }
                if truth.fill_capacity {
                    needs.push("fill_capacity");
                }
                let key = needs.join("+");
                let key = if key.is_empty() { "none".into() } else { key };
                *combos.entry(key).or_default() += 1;
                if truth.cut {
                    *conditions.entry("cut").or_default() += 1;
                }
                if truth.fill_roadbed {
                    *conditions.entry("fill_roadbed").or_default() += 1;
                }
                if truth.fill_capacity {
                    *conditions.entry("fill_capacity").or_default() += 1;
                }
                *notes.entry(attempt.note.clone()).or_default() += 1;
                *cut_buckets.entry(bucket(truth.cut_mm)).or_default() += 1;
                *fill_buckets.entry(bucket(truth.fill_mm)).or_default() += 1;
                *raise_buckets
                    .entry(bucket(truth.raise_delta_mm))
                    .or_default() += 1;
                *side_split
                    .entry(if attempt.side < 0 {
                        "negative"
                    } else {
                        "positive"
                    })
                    .or_default() += 1;
                max_cut = max_cut.max(truth.cut_mm);
                max_fill = max_fill.max(truth.fill_mm);
                if truth.raise_legal {
                    raise_legal += 1;
                }
                if truth.road_point_cut_mm > 0 {
                    road_point_cut += 1;
                }
                if truth.entrance_cut_mm > 0 {
                    entrance_cut += 1;
                }
                *entrance_cut_buckets
                    .entry(bucket(truth.entrance_cut_mm))
                    .or_default() += 1;
            }
            // Candidates whose support truth is ALSO violated but whose first failing
            // gate was something else: the cut/fill envelope blocks more frontage than
            // the family counter shows.
            let mut hidden = BTreeMap::<String, u32>::new();
            for attempt in attempts
                .iter()
                .filter(|a| a.request == outcome.request.id)
                .filter(|a| a.reason != "natural_support_or_fill_limit")
            {
                let truth = &attempt.truth;
                if truth.cut || truth.fill_roadbed || truth.fill_capacity {
                    *hidden.entry(attempt.reason.clone()).or_default() += 1;
                }
            }
            let family_total = family.len() as i64;
            // Authorisation table: how many of this request's family rejections a
            // fill-only pad raised off the road datum would unlock, per walkable
            // entrance run the project lead might authorise. The 0.5 m row is the
            // current 2 m apron at the 25 % walkable grade.
            let mut raise_by_ramp = BTreeMap::<&'static str, u32>::new();
            for (label, rise_mm) in [
                ("0.5m", 500_i64),
                ("1m", 1000),
                ("2m", 2000),
                ("3m", 3000),
                ("4m", 4000),
                ("6m", 6000),
            ] {
                let count = family
                    .iter()
                    .filter(|attempt| {
                        let truth = &attempt.truth;
                        truth.raise_delta_mm <= rise_mm
                            && truth.raise_fill_mm <= max_fill_mm
                            && truth.raise_capacity_ok
                    })
                    .count() as u32;
                raise_by_ramp.insert(label, count);
            }
            totals.insert(format!("{}.family", outcome.request.id), family_total);
            totals.insert(
                format!("{}.cut", outcome.request.id),
                i64::from(*conditions.get("cut").unwrap_or(&0)),
            );
            totals.insert(
                format!("{}.fillRoadbed", outcome.request.id),
                i64::from(*conditions.get("fill_roadbed").unwrap_or(&0)),
            );
            totals.insert(
                format!("{}.fillCapacity", outcome.request.id),
                i64::from(*conditions.get("fill_capacity").unwrap_or(&0)),
            );
            totals.insert(
                format!("{}.raiseLegal", outcome.request.id),
                i64::from(raise_legal),
            );
            rows.push(serde_json::json!({
                "requestId": outcome.request.id,
                "demand": outcome.request.count,
                "attempted": outcome.attempted,
                "accepted": outcome.site_ids.len(),
                "unfilled": outcome.unfilled,
                "publishedRejected": outcome.rejected,
                "familyRejected": family_total,
                "familySubconditions": combos,
                "familyConditionCounts": conditions,
                "familyFirstSampleNotes": notes,
                "familyCutDepthBuckets": cut_buckets,
                "familyFillDepthBuckets": fill_buckets,
                "familyRaiseToNaturalMaxLegal": raise_legal,
                "familyRaiseLegalByRampMm": raise_by_ramp,
                "familyRaiseDeltaBuckets": raise_buckets,
                "familyRoadPointCut": road_point_cut,
                "familyEntranceCut": entrance_cut,
                "familyEntranceCutBuckets": entrance_cut_buckets,
                "familySideSplit": side_split,
                "familyMaxCutMm": max_cut,
                "familyMaxFillMm": max_fill,
                "supportInfeasibleButBlockedEarlier": hidden,
            }));
        }
        let report = serde_json::json!({
            "world": root.display().to_string(),
            "scene": scene.scene_id,
            "naturalHashMatches": natural_hash_matches,
            "zoningHashMatches": zoning_hash_matches,
            "requestIdentityMatches": request_identity_matches,
            "maxRoadbedFillM": plan.max_roadbed_fill_m,
            "apronRiseBudgetMm": apron_rise_mm,
            "profileRoads": roads.len(),
            "productionReproducesPublished": production_matches,
            "productionReplay": production_rows,
            "shadowReproducesProduction": shadow_matches,
            "requests": rows,
            "totals": totals,
        });
        println!(
            "SUPPORT-AUDIT-JSON: {}",
            serde_json::to_string(&report).unwrap()
        );
        assert!(
            natural_hash_matches && zoning_hash_matches && request_identity_matches,
            "the audit must run on the published inputs"
        );
        assert!(
            production_matches,
            "stripping the frontage sites and re-running `frontage::append` must reproduce the \
             published outcome exactly"
        );
        assert!(
            shadow_matches,
            "the shadow replay must reproduce the production selection exactly"
        );
    }

    /// The style a published world was actually baked from.
    ///
    /// The bake runs the MORPHOLOGY-COMPILED programme, not the authored one: a
    /// frontage request's `parcelInsets` and its derived `setbackM` only exist
    /// after `morphology_compile::compile` has filled them in from
    /// `style.morphologyProfile`. A read-only replay that reads `manifest.style`
    /// therefore searches a DIFFERENT parcel than the published plan did - no side
    /// inset, no rear inset - and it is the replay, not the bake, that is wrong.
    /// Measured on the hillside town before this helper existed: the replay planned
    /// 5 homes + 2 shops against the published 4 + 3, so
    /// `support_gate_audit_of_a_published_world` failed its own
    /// "the replay must reproduce the published report" assertion and the
    /// per-subcondition support census was computed over the wrong rectangles.
    ///
    /// A style that states no `morphologyProfile` compiles to `None` and is
    /// returned verbatim, so a world that never used a profile replays exactly as
    /// before.
    fn baked_style(style: &serde_json::Value, scene: &SceneSpec) -> serde_json::Value {
        match crate::settlement::morphology_compile::compile(style, std::slice::from_ref(scene)) {
            Ok(Some(compiled)) => compiled.style,
            _ => style.clone(),
        }
    }

    /// One published world, reloaded from its own bytes for a read-only replay:
    /// the published natural chunk set, the published zoning and the published
    /// road plan. Nothing is regenerated, so a census can never describe a field
    /// (or a network) the bake did not use.
    struct PublishedWorld {
        scene: SceneSpec,
        natural: BakedTerrain,
        water: WaterGeometry,
        zones: Vec<LandUseZone>,
        plan: RoadNetworkPlan,
        requests: Vec<Request>,
        style: serde_json::Value,
    }

    fn load_published_world(root: &std::path::Path) -> PublishedWorld {
        let manifest: super::super::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).expect("published manifest"),
        )
        .expect("manifest json");
        let scene = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&scene.scene_id);
        let plan: RoadNetworkPlan = serde_json::from_slice(
            &std::fs::read(dir.join("road-network-plan.json")).expect("published road plan"),
        )
        .expect("plan json");
        let zones: Vec<LandUseZone> = serde_json::from_slice(
            &std::fs::read(dir.join("land-use-zones.json")).expect("published zoning"),
        )
        .expect("zoning json");
        let mut natural = BakedTerrain::new(&scene).expect("raster");
        for chunk_z in 0..scene.depth_m.div_ceil(512) {
            for chunk_x in 0..scene.width_m.div_ceil(512) {
                let dx = chunk_x * 512;
                let dz = chunk_z * 512;
                let width = (scene.width_m - dx).min(512);
                let depth = (scene.depth_m - dz).min(512);
                let bytes = std::fs::read(
                    dir.join(format!("chunk-{chunk_x}-{chunk_z}.natural.height.bin")),
                )
                .expect("published natural chunk");
                natural
                    .insert_chunk(
                        scene.origin_x + dx as i32,
                        scene.origin_z + dz as i32,
                        width,
                        depth,
                        &bytes,
                    )
                    .expect("chunk insert");
            }
        }
        natural.require_complete().expect("complete raster");
        let water = water_geometry(
            water_kind(&manifest.style),
            &manifest.landmarks,
            &scene,
            &manifest.style,
        );
        // The bake runs the MORPHOLOGY-COMPILED programme, not the authored one:
        // `parcelInsets` in a published request is derived from `style.morphologyProfile`,
        // so a replay from the raw manifest would search a different parcel than the
        // published plan did. The compiled style is what both the gates and the policy
        // must be read from.
        let effective = baked_style(&manifest.style, &scene);
        let requests: Vec<Request> = frontage::from_style(&effective)
            .expect("frontage program")
            .expect("frontage program present")
            .requests
            .into_iter()
            .filter(|r| r.scene_id == scene.scene_id)
            .collect();
        PublishedWorld {
            scene,
            natural,
            water,
            zones,
            plan,
            requests,
            style: effective,
        }
    }

    /// Read-only per-CANDIDATE census of one published world's frontage search.
    ///
    /// `diagnose` shares every candidate constructor, every gate and the exact
    /// production order (`frontage::candidate_order`) with `frontage::append`, so
    /// what it reports is the search the bake performs, attempt by attempt: the
    /// road, station and side, the verdict, and the structured `RejectDetail` the
    /// refusing gate recorded (which entity it collided with, at which cell).
    /// The census also carries the `standalone` pass, the same per-road search run
    /// against the committed state WITHOUT any competing frontage parcel, so the
    /// failed demand can be split into "no such frontage exists" and "another lot
    /// took it".
    ///
    /// Prints one `FRONTAGE-CENSUS-JSON:` line. Nothing is regenerated or written.
    #[test]
    #[ignore = "reads one published world from GLYPHWEAVE_SUPPORT_AUDIT_WORLD; writes nothing"]
    fn frontage_candidate_census_of_a_published_world() {
        let Ok(root) = std::env::var("GLYPHWEAVE_SUPPORT_AUDIT_WORLD") else {
            eprintln!("SKIP: set GLYPHWEAVE_SUPPORT_AUDIT_WORLD to a published world directory");
            return;
        };
        let world = load_published_world(std::path::Path::new(&root));
        let mask = ZoningMask::new(&world.scene, &world.zones).expect("zoning mask");
        let policy = super::super::bounded_cut::CutPolicy::from_style(&world.style).0;
        let published = world
            .plan
            .frontage
            .as_ref()
            .expect("published frontage report");
        let diagnosis = diagnose(
            &world.scene,
            &world.natural,
            &mask,
            world.water,
            &world.zones,
            &world.plan,
            &world.requests,
            512,
            policy,
        )
        .expect("census");
        // The census must be the published search, not a similar one.
        let production_matches = published.outcomes.len() == diagnosis.requests.len()
            && published
                .outcomes
                .iter()
                .zip(&diagnosis.requests)
                .all(|(outcome, summary)| {
                    summary.request_id == outcome.request.id
                        && summary.joint_accepted == outcome.site_ids.len()
                        && summary.joint_attempted == outcome.attempted
                        && summary.rejected == outcome.rejected
                });
        let standalone: Vec<serde_json::Value> = diagnosis
            .requests
            .iter()
            .map(|summary| {
                let traces: Vec<&RoadSupply> = diagnosis
                    .standalone
                    .iter()
                    .filter(|trace| trace.request_id == summary.request_id)
                    .collect();
                let mut rejected: BTreeMap<String, u32> = BTreeMap::new();
                for trace in &traces {
                    for (reason, count) in &trace.rejected {
                        *rejected.entry(reason.clone()).or_default() += *count;
                    }
                }
                serde_json::json!({
                    "requestId": summary.request_id,
                    "acceptedWithoutCompetingLots": traces.iter().map(|t| t.accepted).sum::<usize>(),
                    "attemptedWithoutCompetingLots": traces.iter().map(|t| t.attempted).sum::<u32>(),
                    "rejectedWithoutCompetingLots": rejected,
                })
            })
            .collect();
        let report = serde_json::json!({
            "world": root,
            "scene": world.scene.scene_id,
            "maxCutMm": policy.cap_mm(),
            "productionMatches": production_matches,
            "totals": {
                "attempted": diagnosis.joint_station_attempts,
                "accepted": diagnosis.joint_accepted,
                "rejected": diagnosis.joint_rejected,
                "stopReason": diagnosis.stop_reason,
            },
            "requests": diagnosis.requests,
            "standalone": standalone,
            "joint": diagnosis.joint,
        });
        println!(
            "FRONTAGE-CENSUS-JSON: {}",
            serde_json::to_string(&report).unwrap()
        );
        assert!(
            production_matches,
            "the census must reproduce the published frontage report attempt for attempt"
        );
    }
}
