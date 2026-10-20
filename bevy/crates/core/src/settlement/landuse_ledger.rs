//! Area ledger for independently verified road-first geometry. Zoning purpose
//! and physical cover are orthogonal: a rural road is transport IN rural land.
//! No normalized entity proxy or reserved candidate earns realization credit.
//! Call after road/development/landcover reconstruction; this is not a safety
//! or ecology validator. Published reports are never acceptance authority.
use super::{
    development::{self, LandRole},
    landcover::{self, Material},
};
use crate::worldgen::{
    LandUseProfile, LandUseZone, SceneIndex, SceneSpec, WaterGeometry,
    frontage::{self, Conversion},
    road_network::RoadNetworkPlan,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const EPS: f64 = 1e-6;
const PLAN_TOLERANCE: f64 = 0.05;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicalCover {
    pub buildings_m2: f64,
    pub roads_m2: f64,
    pub managed_grass_m2: f64,
    pub access_pavement_m2: f64,
    pub public_open_space_m2: f64,
    pub pasture_m2: f64,
    pub cultivated_soil_m2: f64,
    pub field_headland_m2: f64,
    pub farm_tracks_m2: f64,
    pub conservation_ground_m2: f64,
    /// Opt-in complete-ground background surfaces. Absent (and therefore not
    /// serialized) for legacy worlds, so their ledger bytes stay unchanged.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ground_cover_m2: f64,
    /// Terrain bed owned by authored water bodies in the complete-ground pass.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub water_surface_m2: f64,
}
fn is_zero_f64(value: &f64) -> bool {
    *value == 0.
}
impl PhysicalCover {
    pub fn total(&self) -> f64 {
        self.buildings_m2
            + self.roads_m2
            + self.managed_grass_m2
            + self.access_pavement_m2
            + self.public_open_space_m2
            + self.pasture_m2
            + self.cultivated_soil_m2
            + self.field_headland_m2
            + self.farm_tracks_m2
            + self.conservation_ground_m2
            + self.ground_cover_m2
            + self.water_surface_m2
    }
    pub fn transport(&self) -> f64 {
        self.roads_m2 + self.farm_tracks_m2
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PurposeAccount {
    pub target_ratio: f64,
    pub target_m2: f64,
    pub planned_m2: f64,
    pub planned_scene_ratio: f64,
    pub realized_m2: f64,
    pub realized_scene_ratio: f64,
    pub unresolved_m2: f64,
    pub target_deficit_m2: f64,
    pub target_excess_m2: f64,
    pub physical: PhysicalCover,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandUseLedger {
    pub version: u32,
    pub accounting: String,
    pub scene_id: String,
    pub scene_area_m2: f64,
    pub by_purpose: BTreeMap<String, PurposeAccount>,
    pub realized_m2: f64,
    pub unresolved_m2: f64,
    /// Subset of realized_m2, NOT another area to add to the purpose rows.
    pub transport_subset_m2: f64,
    /// The purpose conversions this ledger applied, in plan order: the along-road frontage
    /// programme's parcels first, then the roadside PAD programme's, because both move the same two
    /// rows. Empty (and therefore absent) for every world that never enabled the explicit along-road
    /// switch, so an old ledger round-trips unchanged. Each row moved exactly its `area_m2` from rural
    /// to urban realization and from nowhere else, which is what `audit_conversions` re-checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversions: Vec<Conversion>,
    /// Explicit residual-land accounting. Absent for worlds that never declared
    /// a residual program, so legacy ledgers keep their serialized shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual: Option<ResidualAudit>,
    pub allocation_status: String,
    pub realization_status: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidualRejection {
    pub reason: String,
    pub area_m2: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidualAccount {
    pub source_zone_id: String,
    pub sub_use: String,
    pub candidate_m2: f64,
    pub target_m2: f64,
    pub realized_m2: f64,
    /// Candidate land the policy deliberately leaves outside this sub-use.
    /// Not an additional area; it may exceed unresolved when realized exceeds
    /// target (for example cultivated soil inside a pasture cell).
    pub policy_remainder_m2: f64,
    /// Rejected attempts inside unresolved_m2, never an additional area.
    pub rejected_attempt_m2: f64,
    pub unresolved_m2: f64,
    pub rejections: Vec<ResidualRejection>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidualAudit {
    pub status: String,
    pub candidate_m2: f64,
    pub target_m2: f64,
    pub realized_m2: f64,
    pub policy_remainder_m2: f64,
    pub rejected_attempt_m2: f64,
    pub unresolved_m2: f64,
    pub accounts: Vec<ResidualAccount>,
    pub failures: Vec<String>,
}
/// Independent residual accounting over the published source cells and their
/// realized mesh geometry. Rejected attempts are a subset of unresolved.
/// Policy remainder is not additive and may exceed unresolved when realized
/// exceeds target.
pub fn residual_audit(
    plan: &development::LandcoverPlan,
    surfaces: &landcover::SurfaceRealization,
) -> Result<ResidualAudit, String> {
    let program = plan
        .residual_program
        .as_ref()
        .ok_or("residual audit requires a residual program")?;
    let mut patches_by_source: BTreeMap<&str, Vec<&landcover::SurfacePatch>> = BTreeMap::new();
    for patch in &surfaces.patches {
        let size = landcover::mesh_area(patch)?;
        if !patch.area_m2.is_finite() || (size - patch.area_m2).abs() > EPS {
            return Err("residual surface declared area differs from geometry".into());
        }
        patches_by_source
            .entry(patch.source_cell_id.as_str())
            .or_default()
            .push(patch);
    }
    let mut rejections_by_source: BTreeMap<&str, Vec<&landcover::SurfaceRejection>> =
        BTreeMap::new();
    for rejection in &surfaces.rejections {
        rejections_by_source
            .entry(rejection.source_cell_id.as_str())
            .or_default()
            .push(rejection);
    }
    let mut accounts: BTreeMap<(String, String), ResidualAccount> = BTreeMap::new();
    for cell in &plan.cells {
        let (sub_use, ratio) = match cell.role {
            development::LandcoverRole::ManagedYard => ("managed_yard", 1.),
            development::LandcoverRole::FutureDevelopment => (
                "urban_public_open_space",
                program.urban_open_space_ratio(),
            ),
            development::LandcoverRole::AgricultureCandidate => (
                "rural_pasture",
                program.rural_pasture_ratio(),
            ),
            development::LandcoverRole::ConservationCandidate => ("conservation", 1.),
            development::LandcoverRole::Unmanaged => ("unmanaged", 1.),
        };
        let candidate = cell.boundary.area() as f64;
        let target = (candidate * ratio).floor();
        let mut realized = 0.;
        for patch in patches_by_source.get(cell.id.as_str()).into_iter().flatten() {
            let counts = match (cell.role, patch.material) {
                (
                    development::LandcoverRole::ManagedYard,
                    landcover::Material::ManagedGrass | landcover::Material::AccessPavement,
                ) => true,
                (
                    development::LandcoverRole::FutureDevelopment,
                    landcover::Material::PublicOpenSpace
                    | landcover::Material::UrbanVacantGround
                    | landcover::Material::WaterSurface,
                ) => true,
                (
                    development::LandcoverRole::AgricultureCandidate,
                    landcover::Material::PastureGround
                    | landcover::Material::CultivatedSoil
                    | landcover::Material::FieldHeadland
                    | landcover::Material::FarmTrack
                    | landcover::Material::RuralMeadow
                    | landcover::Material::RuralWoodlandFloor
                    | landcover::Material::RuralRoughGround
                    | landcover::Material::WaterSurface,
                ) => true,
                (
                    development::LandcoverRole::ConservationCandidate,
                    landcover::Material::ConservationGround
                    | landcover::Material::ConservationForestFloor
                    | landcover::Material::ConservationWetland
                    | landcover::Material::WaterSurface,
                ) => true,
                (
                    development::LandcoverRole::Unmanaged,
                    landcover::Material::UnassignedGround | landcover::Material::WaterSurface,
                ) => true,
                _ => false,
            };
            if counts {
                realized += landcover::mesh_area(patch)?;
            }
        }
        let mut rejected_by_reason: BTreeMap<String, f64> = BTreeMap::new();
        let mut rejected = 0.;
        for rejection in rejections_by_source.get(cell.id.as_str()).into_iter().flatten() {
            let area = rejection.area_m2 as f64;
            rejected += area;
            *rejected_by_reason.entry(rejection.reason.clone()).or_default() += area;
        }
        let unresolved = (candidate - realized).max(0.);
        let account = accounts
            .entry((cell.zone_id.clone(), sub_use.into()))
            .or_insert_with(|| ResidualAccount {
                source_zone_id: cell.zone_id.clone(),
                sub_use: sub_use.into(),
                candidate_m2: 0.,
                target_m2: 0.,
                realized_m2: 0.,
                policy_remainder_m2: 0.,
                rejected_attempt_m2: 0.,
                unresolved_m2: 0.,
                rejections: vec![],
            });
        account.candidate_m2 += candidate;
        account.target_m2 += target;
        account.realized_m2 += realized;
        account.policy_remainder_m2 += (candidate - target).max(0.);
        account.rejected_attempt_m2 += rejected.min(unresolved);
        account.unresolved_m2 += unresolved;
        for (reason, area) in rejected_by_reason {
            if let Some(row) = account.rejections.iter_mut().find(|row| row.reason == reason) {
                row.area_m2 += area;
            } else {
                account.rejections.push(ResidualRejection { reason, area_m2: area });
            }
        }
    }
    let accounts: Vec<_> = accounts.into_values().collect();
    let candidate_m2: f64 = accounts.iter().map(|a| a.candidate_m2).sum();
    let target_m2: f64 = accounts.iter().map(|a| a.target_m2).sum();
    let realized_m2: f64 = accounts.iter().map(|a| a.realized_m2).sum();
    let policy_remainder_m2: f64 = accounts.iter().map(|a| a.policy_remainder_m2).sum();
    let rejected_attempt_m2: f64 = accounts.iter().map(|a| a.rejected_attempt_m2).sum();
    let unresolved_m2: f64 = accounts.iter().map(|a| a.unresolved_m2).sum();
    let mut failures = Vec::new();
    if (candidate_m2 - realized_m2 - unresolved_m2).abs() > EPS {
        failures.push("residual candidate does not equal realized plus unresolved".into());
    }
    if rejected_attempt_m2 > unresolved_m2 + EPS {
        failures.push("residual rejected attempts exceed unresolved".into());
    }
    Ok(ResidualAudit {
        status: if unresolved_m2 <= EPS && failures.is_empty() {
            "pass".into()
        } else {
            "incomplete".into()
        },
        candidate_m2,
        target_m2,
        realized_m2,
        policy_remainder_m2,
        rejected_attempt_m2,
        unresolved_m2,
        accounts,
        failures,
    })
}

/// Independent verdict on the conversion accounting of one scene.
///
/// Nothing here is taken on trust: every record is re-derived from the plan's own parcel geometry,
/// the zoning and the water authority, and the ledger's rows must be exactly the plan's records.
/// A hand-edited ledger therefore fails by name instead of inflating urban realization.
///
/// It also closes the triangle a planned pad can fall through. A pad the road plan REFUSED by name
/// (`worldgen::road_network::PAD_FILL_CONFLICT_CATEGORY`) is no longer a parcel of this world, so
/// booking its converted land would credit an area to an entity that cannot exist. The plan may not
/// keep the record, the ledger may not apply it, and the plan may not silently plan the pad again -
/// each of those is a named failure, not a count that happens to agree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversionAudit {
    pub status: String,
    /// Records the plan claims.
    pub planned: usize,
    /// Records the ledger applied.
    pub applied: usize,
    /// Sum of the verified conversion areas; the exact amount the ledger must have moved.
    pub converted_m2: i64,
    /// Pads the plan refused by name on a fill conflict with a neighbour (see
    /// `worldgen::road_network::PAD_FILL_CONFLICT_CATEGORY`). None of them may be booked.
    pub refused_pads: usize,
    pub failures: Vec<String>,
}

/// Every conversion record the PLAN claims, frontage programme first, then the roadside pad
/// programme, each in plan order.
///
/// A pad is not a frontage request: its record lives on the plan, not inside the frontage report
/// (a world may author a `settlementProgram` with no `frontageProgram` at all). Both lists move the
/// same accounting, so the ledger reads them as ONE list and `audit_conversions` compares against
/// the same union instead of trusting either half.
fn plan_conversions(plan: &RoadNetworkPlan) -> Vec<Conversion> {
    let mut records = plan
        .frontage
        .as_ref()
        .map(|report| report.conversions.clone())
        .unwrap_or_default();
    records.extend(plan.pad_conversions.iter().cloned());
    records
}

/// The plan's conversion records, re-derived from their own parcels before any area moves.
///
/// A record is refused by name when it is not rural-to-urban, when its area is not a positive whole
/// area, when the same parcel is counted twice, or when the parcel's buildable core is not wholly
/// rural land. Water has no authority at this layer; the quality gate re-runs the same derivation
/// WITH the water geometry.
fn planned_conversions(
    plan: &RoadNetworkPlan,
    zones: &[LandUseZone],
) -> Result<Vec<Conversion>, String> {
    let records = plan_conversions(plan);
    let mut counted = BTreeSet::new();
    for record in &records {
        if !counted.insert(record.parcel_id.clone()) {
            return Err(format!(
                "ledger conversion parcel {} is counted twice",
                record.parcel_id
            ));
        }
        let recomputed = frontage::verify_conversion(plan, zones, None, record)?;
        if recomputed != record.area_m2 {
            return Err(format!(
                "ledger conversion parcel {} declares {} m2 but its buildable core is {} m2",
                record.parcel_id, record.area_m2, recomputed
            ));
        }
    }
    Ok(records)
}

/// Re-check the conversion accounting a PUBLISHED ledger claims against the plan it was built from.
///
/// The published ledger is the document a reader sees, so it is the one that must agree with the
/// plan: a recomputed ledger alone would only prove the code agrees with itself. `None` means no
/// published ledger carries the claim, which is itself a failure — a conversion that exists only in
/// the plan earns no realization credit anywhere.
pub fn audit_conversions(
    plan: &RoadNetworkPlan,
    zones: &[LandUseZone],
    water: WaterGeometry,
    ledger: Option<&LandUseLedger>,
) -> ConversionAudit {
    let claimed = plan_conversions(plan);
    let records = claimed.as_slice();
    let mut failures = Vec::new();
    let mut converted_m2 = 0_i64;
    let mut duplicated = BTreeSet::new();
    // Pads the plan refused BY NAME because their declared flat top cannot be held (see
    // `worldgen::road_network::PAD_REFUSED_CATEGORIES`: the only roadside categories whose `edgeId`
    // names a pad that WAS planned and was taken back out again). They are not parcels of this
    // world any more, so `triangle_failures` refuses any record that still books one.
    let refused: BTreeSet<&str> = crate::worldgen::road_network::refused_pad_ids(plan);
    for record in records {
        if !duplicated.insert(record.parcel_id.as_str()) {
            failures.push(format!(
                "plan conversion parcel {} is recorded twice",
                record.parcel_id
            ));
            continue;
        }
        match frontage::verify_conversion(plan, zones, Some(water), record) {
            Ok(recomputed) if recomputed == record.area_m2 => converted_m2 += record.area_m2,
            Ok(recomputed) => failures.push(format!(
                "conversion parcel {} claims {} m2 but its buildable core is {} m2",
                record.parcel_id, record.area_m2, recomputed
            )),
            Err(error) => failures.push(error),
        }
    }
    // A pad that DECLARED an urban purpose on wholly-rural land has earned a conversion; leaving the
    // record out would silently keep its area in the rural row, so the claim is completed here by
    // name instead of being trusted to whoever wrote the record list.
    let mut recorded: BTreeSet<&str> = records.iter().map(|r| r.parcel_id.as_str()).collect();
    for site in &plan.roadside_sites {
        if site.purpose.as_deref() != Some(frontage::Conversion::URBAN) {
            continue;
        }
        if frontage::pad_conversion_area(site, zones).is_some()
            && !recorded.remove(site.id.as_str())
        {
            failures.push(format!(
                "pad conversion parcel {} is not recorded in the plan",
                site.id
            ));
        }
    }
    triangle_failures(plan, records, &refused, ledger, &mut failures);
    // The purpose allocation: the verified conversion area may not exceed the urban land the plan
    // allocated but did not commit, and every published refusal must name an area it could not
    // afford. This is the half of the contract the LEDGER cannot re-derive on its own - the ledger
    // sees the after state and refuses an overcredited row, while this refuses the plan that would
    // produce it, by name, from the plan and the zoning alone. It runs before the "is there a
    // published ledger" question, because a plan that over-spends the allocation is already wrong.
    failures.extend(crate::worldgen::conversion_budget::allocation_failures(
        plan,
        zones,
        converted_m2,
    ));
    // WHERE the conversions happened: every kept record's buildable core must lie inside the urban
    // edge the world's explicit conversion policy allows, and a pad that declared an urban purpose on
    // wholly-rural land may not stand outside that edge either. Re-derived from the plan and the
    // zoning alone (`worldgen::conversion_fringe::audit_failures`), so a forged record or a pad kept
    // without its conversion fails here by name with the distance it stands.
    failures.extend(crate::worldgen::conversion_fringe::audit_failures(
        plan, zones,
    ));
    let Some(ledger) = ledger else {
        failures.push(format!(
            "no published ledger carries the {} conversion(s) the plan claims",
            records.len()
        ));
        return ConversionAudit {
            status: "fail".into(),
            planned: records.len(),
            applied: 0,
            converted_m2,
            refused_pads: refused.len(),
            failures,
        };
    };
    // The ledger rows must be exactly the plan's records: same count, same parcel ids, same areas.
    let mut applied_by_parcel = BTreeMap::new();
    for applied in &ledger.conversions {
        if applied_by_parcel
            .insert(applied.parcel_id.as_str(), applied)
            .is_some()
        {
            failures.push(format!(
                "ledger counts conversion parcel {} twice",
                applied.parcel_id
            ));
        }
    }
    if ledger.conversions.len() != records.len() {
        failures.push(format!(
            "ledger applied {} conversions while the plan records {}",
            ledger.conversions.len(),
            records.len()
        ));
    }
    for record in records {
        match applied_by_parcel.get(record.parcel_id.as_str()) {
            Some(applied) if *applied == record => {}
            Some(applied) => failures.push(format!(
                "ledger conversion parcel {} is {}-to-{} {} m2 but the plan records {} m2",
                record.parcel_id, applied.from, applied.to, applied.area_m2, record.area_m2
            )),
            None => failures.push(format!(
                "ledger is missing conversion parcel {}",
                record.parcel_id
            )),
        }
    }
    // And the ledger's own rows must have moved exactly what its own conversion list claims, once.
    // This is the `sum(conversions) == urban delta == rural delta` identity, checked against the
    // ledger's OWN records so it stays meaningful even when the plan's claim could not be verified.
    let claimed = ledger
        .conversions
        .iter()
        .map(|record| record.area_m2)
        .sum::<i64>() as f64;
    match (
        ledger.by_purpose.get("urban"),
        ledger.by_purpose.get("rural"),
    ) {
        (Some(urban), Some(rural)) => {
            let moved_in = urban.realized_m2 - urban.physical.total();
            let moved_out = rural.physical.total() - rural.realized_m2;
            if (moved_in - claimed).abs() > EPS || (moved_out - claimed).abs() > EPS {
                failures.push(format!(
                    "ledger moved {moved_in} m2 into urban and {moved_out} m2 out of rural, but \
                     its conversions account for {claimed} m2"
                ));
            }
        }
        _ => failures.push("ledger is missing its urban or rural purpose row".into()),
    }
    let status = if failures.is_empty() { "pass" } else { "fail" };
    ConversionAudit {
        status: status.into(),
        planned: records.len(),
        applied: ledger.conversions.len(),
        converted_m2,
        refused_pads: refused.len(),
        failures,
    }
}

/// The categories a refused pad can name, for a failure message.
fn refused_categories() -> String {
    crate::worldgen::road_network::PAD_REFUSED_CATEGORIES.join(" or ")
}

/// Every record the plan PUBLISHES must name a pad the plan still carries.
///
/// This is the structural half of "a refused pad may not leave a record behind": a
/// record is joined to the plan's own pad list by identity, never to the text of a
/// refusal, so a record left in the frontage report (or in the pad table) by any
/// future removal path fails by name here. `forget_refused_pad` withdraws the pad
/// from both tables in one place; this is the audit that the withdrawal was whole.
fn unplanned_record_failures(plan: &RoadNetworkPlan, failures: &mut Vec<String>) {
    let planned: BTreeSet<&str> = plan
        .roadside_sites
        .iter()
        .map(|site| site.id.as_str())
        .collect();
    if let Some(report) = &plan.frontage {
        for record in &report.conversions {
            if !planned.contains(record.parcel_id.as_str()) {
                failures.push(format!(
                    "frontage conversion parcel {} is not a planned pad; the frontage report still \
                     books {} m2",
                    record.parcel_id, record.area_m2
                ));
            }
        }
        for outcome in &report.outcomes {
            for id in &outcome.site_ids {
                if !planned.contains(id.as_str()) {
                    failures.push(format!(
                        "frontage outcome {} names pad {} which the plan does not carry",
                        outcome.request.id, id
                    ));
                }
            }
        }
    }
    for record in &plan.pad_conversions {
        if !planned.contains(record.parcel_id.as_str()) {
            failures.push(format!(
                "pad conversion parcel {} is not a planned pad; the plan still books {} m2",
                record.parcel_id, record.area_m2
            ));
        }
    }
}

/// The plan/pad/ledger triangle: a refused pad may not be re-planned, a record may not survive for a
/// pad the plan does not plan, and the ledger may not apply a record the plan does not carry.
///
/// This is the half of the check that needs no raster and no entity list: everything here is
/// recomputable from the plan and the published ledger alone, which is exactly what makes it cheap
/// enough to run on every scene.
fn triangle_failures(
    plan: &RoadNetworkPlan,
    records: &[Conversion],
    refused: &BTreeSet<&str>,
    ledger: Option<&LandUseLedger>,
    failures: &mut Vec<String>,
) {
    unplanned_record_failures(plan, failures);
    for site in &plan.roadside_sites {
        if refused.contains(site.id.as_str()) {
            failures.push(format!(
                "pad {} is refused by the plan ({}) and still planned",
                site.id,
                refused_categories()
            ));
        }
    }
    for record in records {
        if refused.contains(record.parcel_id.as_str()) {
            failures.push(format!(
                "conversion parcel {} was refused by the plan ({}) yet the plan still books {} m2",
                record.parcel_id,
                refused_categories(),
                record.area_m2
            ));
        }
    }
    let Some(ledger) = ledger else {
        return;
    };
    for applied in &ledger.conversions {
        if refused.contains(applied.parcel_id.as_str()) {
            failures.push(format!(
                "ledger books conversion parcel {} which the plan refused ({}); the pad has no entity",
                applied.parcel_id,
                refused_categories()
            ));
            continue;
        }
        if !plan
            .roadside_sites
            .iter()
            .any(|site| site.id == applied.parcel_id)
        {
            failures.push(format!(
                "ledger books conversion parcel {} which the plan does not plan; \
                 the pad has no plan record and no entity",
                applied.parcel_id
            ));
        }
    }
}

pub fn analyze(scene: &SceneIndex, profile: &LandUseProfile) -> Result<LandUseLedger, String> {
    let development = scene
        .development_plan
        .as_ref()
        .ok_or("ledger requires development plan")?;
    let roads = scene
        .road_network_plan
        .as_ref()
        .ok_or("ledger requires road plan")?;
    let landcover_plan = scene
        .landcover_plan
        .as_ref()
        .ok_or("ledger requires landcover plan")?;
    let surfaces = scene
        .landcover_surfaces
        .as_ref()
        .ok_or("ledger requires realized surface contract")?;
    let spec = SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    development::validate(development, &spec, &scene.land_use_zones)?;
    development::validate_landcover(landcover_plan, &spec, development)?;
    let area = f64::from(scene.width_m) * f64::from(scene.depth_m);
    if area <= 0. {
        return Err("ledger requires nonempty scene".into());
    }
    // Explicit along-road purpose conversions are resolved BEFORE the rows are closed, so every
    // conservation identity below is computed on the AFTER state instead of being patched up after
    // the fact. Each record is re-derived from its own parcel, so a forged entry cannot move area.
    let conversions = planned_conversions(roads, &scene.land_use_zones)?;
    let converted_m2 = conversions
        .iter()
        .map(|record| record.area_m2)
        .sum::<i64>() as f64;
    let norm = (profile.urban_target() + profile.rural_target() + profile.nature_target()).max(1.);
    let targets = [
        profile.urban_target() / norm,
        profile.rural_target() / norm,
        profile.nature_target() / norm,
    ];
    let mut result = LandUseLedger {
        version: 1,
        accounting: "disjoint-purpose-cover-v1".into(),
        scene_id: scene.scene_id.clone(),
        scene_area_m2: area,
        by_purpose: BTreeMap::new(),
        realized_m2: 0.,
        unresolved_m2: 0.,
        transport_subset_m2: 0.,
        conversions,
        residual: None,
        allocation_status: "pass".into(),
        realization_status: "incomplete".into(),
        status: "incomplete".into(),
    };
    for (kind, ratio) in [
        ("urban", targets[0]),
        ("rural", targets[1]),
        ("nature", targets[2]),
        ("unassigned", (1. - targets.iter().sum::<f64>()).max(0.)),
    ] {
        result.by_purpose.insert(
            kind.into(),
            PurposeAccount {
                target_ratio: ratio,
                target_m2: area * ratio,
                planned_m2: 0.,
                planned_scene_ratio: 0.,
                realized_m2: 0.,
                realized_scene_ratio: 0.,
                unresolved_m2: 0.,
                target_deficit_m2: 0.,
                target_excess_m2: 0.,
                physical: PhysicalCover::default(),
            },
        );
    }
    for zone in &scene.land_use_zones {
        let row = result
            .by_purpose
            .get_mut(&zone.kind)
            .ok_or("unsupported ledger zone purpose")?;
        row.planned_m2 += f64::from(zone.width_m) * f64::from(zone.depth_m);
    }
    for cell in development::realized_cells(development, roads, &scene.entities) {
        if !matches!(cell.role, LandRole::Road | LandRole::Building) {
            continue;
        }
        if !matches!(cell.zone_kind.as_str(), "urban" | "rural") {
            return Err("hard development in reserved ledger purpose".into());
        }
        let row = result
            .by_purpose
            .get_mut(&cell.zone_kind)
            .ok_or("missing ledger purpose")?;
        match cell.role {
            LandRole::Road => row.physical.roads_m2 += cell.boundary.area() as f64,
            LandRole::Building => row.physical.buildings_m2 += cell.boundary.area() as f64,
            _ => unreachable!(),
        }
    }
    let sources: BTreeMap<_, _> = landcover_plan
        .cells
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect();
    let mut ids = BTreeSet::new();
    let mut source_totals = BTreeMap::<&str, f64>::new();
    for patch in &surfaces.patches {
        if !ids.insert(patch.id.as_str()) {
            return Err("duplicate ledger surface id".into());
        }
        let source = sources
            .get(patch.source_cell_id.as_str())
            .ok_or("ledger surface has no source cell")?;
        if patch.zone_kind != source.zone_kind {
            return Err("ledger surface purpose differs from source".into());
        }
        let r = source.boundary;
        if patch.vertices.iter().any(|v| {
            v[0] < r.x as f64 - EPS
                || v[2] < r.z as f64 - EPS
                || v[0] > r.x as f64 + f64::from(r.width_m) + EPS
                || v[2] > r.z as f64 + f64::from(r.depth_m) + EPS
        }) {
            return Err("ledger surface outside source boundary".into());
        }
        let size = landcover::mesh_area(patch)?;
        if !patch.area_m2.is_finite() || (size - patch.area_m2).abs() > EPS {
            return Err("ledger surface declared area differs from geometry".into());
        }
        *source_totals.entry(source.id.as_str()).or_default() += size;
        let row = result
            .by_purpose
            .get_mut(&source.zone_kind)
            .ok_or("missing surface purpose")?;
        match patch.material {
            Material::ManagedGrass => row.physical.managed_grass_m2 += size,
            Material::AccessPavement => row.physical.access_pavement_m2 += size,
            Material::PublicOpenSpace => row.physical.public_open_space_m2 += size,
            Material::PastureGround => row.physical.pasture_m2 += size,
            Material::CultivatedSoil => row.physical.cultivated_soil_m2 += size,
            Material::FieldHeadland => row.physical.field_headland_m2 += size,
            Material::FarmTrack => row.physical.farm_tracks_m2 += size,
            Material::ConservationGround
            | Material::ConservationForestFloor
            | Material::ConservationWetland => row.physical.conservation_ground_m2 += size,
            Material::RuralMeadow
            | Material::RuralWoodlandFloor
            | Material::RuralRoughGround
            | Material::UrbanVacantGround
            | Material::UnassignedGround => row.physical.ground_cover_m2 += size,
            Material::WaterSurface => row.physical.water_surface_m2 += size,
        }
    }
    for (id, size) in source_totals {
        if size > sources[id].boundary.area() as f64 + EPS {
            return Err("ledger source overcredited".into());
        }
    }
    if landcover_plan.residual_program.is_some() {
        result.residual = Some(residual_audit(landcover_plan, surfaces)?);
    }
    for (kind, row) in result.by_purpose.iter_mut() {
        row.planned_scene_ratio = row.planned_m2 / area;
        row.realized_m2 = row.physical.total();
        // The converted land is a PURPOSE transfer, not new physical cover: the converted parcel keeps
        // its buildings and roads in the rural row's physical totals (it still stands in a rural zone),
        // and only the purpose realization moves. Both sides move by the same amount, so the totals
        // below conserve exactly as before and nothing is counted twice.
        match kind.as_str() {
            "urban" => row.realized_m2 += converted_m2,
            "rural" => row.realized_m2 -= converted_m2,
            _ => {}
        }
        if row.realized_m2 < -EPS {
            return Err("ledger conversion exceeds realized rural area".into());
        }
        if row.realized_m2 > row.planned_m2 + EPS {
            return Err("ledger purpose overcredited".into());
        }
        row.realized_scene_ratio = row.realized_m2 / area;
        row.unresolved_m2 = (row.planned_m2 - row.realized_m2).max(0.);
        row.target_deficit_m2 = (row.target_m2 - row.realized_m2).max(0.);
        row.target_excess_m2 = (row.realized_m2 - row.target_m2).max(0.);
        if (row.planned_scene_ratio - row.target_ratio).abs() > PLAN_TOLERANCE + EPS {
            result.allocation_status = "mismatched".into();
        }
        result.realized_m2 += row.realized_m2;
        result.unresolved_m2 += row.unresolved_m2;
        result.transport_subset_m2 += row.physical.transport();
    }
    if (result.realized_m2 + result.unresolved_m2 - area).abs() > EPS {
        return Err("ledger does not conserve scene area".into());
    }
    // A plan that spent conversions against an urban allocation made a claim about the urban cover it
    // did not have to convert: roads, parcels and the programmed public open space. If what the world
    // actually realized inside the urban zones is larger, that claim was false and the conversion
    // decision was taken against a fiction - refused by name here instead of being quietly absorbed.
    if let Some(allocation) = roads.conversion_allocation.as_ref() {
        let physical = result
            .by_purpose
            .get("urban")
            .map_or(0., |row| row.physical.total());
        let bound = (allocation.committed_urban_m2 + allocation.residual_landcover_m2) as f64;
        if physical > bound + EPS {
            return Err(format!(
                "ledger urban cover {physical} m2 exceeds the conversion allocation's {bound} m2 \
                 commitment; the plan converted against land it had already spent"
            ));
        }
    }
    if result.unresolved_m2 <= EPS {
        result.realization_status = "pass".into();
    }
    if result.realization_status == "pass" && result.allocation_status == "pass" {
        result.status = "pass".into();
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        baked_terrain::BakedTerrain,
        worldgen::{LandUseZone, road_network::RoadNetworkPlan, water_geometry, water_kind},
    };
    fn fixture() -> (SceneIndex, LandUseProfile) {
        let spec = SceneSpec {
            scene_id: "ledger".into(),
            width_m: 384,
            depth_m: 256,
            origin_x: -128,
            origin_z: 40,
            seed_offset: 0,
        };
        let zones: Vec<_> = [("urban", -128), ("rural", 0), ("nature", 128)]
            .into_iter()
            .map(|(kind, x)| LandUseZone {
                id: kind.into(),
                kind: kind.into(),
                world_x: x,
                world_z: 40,
                width_m: 128,
                depth_m: 256,
                source: "test".into(),
            })
            .collect();
        let roads: RoadNetworkPlan = serde_json::from_value(serde_json::json!({
            "model":"test", "naturalHeightfieldHash":"test", "sceneId":"ledger", "status":"planned",
            "gridSpacingM":64, "maxRoadbedFillM":6, "levelM":0, "candidates":3, "rejectedWater":0,
            "rejectedRelief":0, "unselectedCandidates":0, "nodes":[],
            "edges": ([-64,0,64].into_iter().enumerate().map(|(i,x)| serde_json::json!({"id":format!("road{i}"), "from":i, "to":i+1, "worldX":x, "worldZ":168, "lengthM":64,
                "rotationYDeg":0., "naturalMinM":0., "naturalMaxM":0.})).collect::<Vec<_>>()),
            "roadsideSites":[{"id":"home", "roadId":"road2", "worldX":64, "worldZ":192,
                "widthM":24, "depthM":24, "supportLevelM":0, "rotationYDeg":0., "naturalMinM":0., "naturalMaxM":0.}]
        })).unwrap();
        let development = development::plan(&spec, &zones, &roads).unwrap();
        let plan = development::plan_landcover(&spec, &development).unwrap();
        let mut terrain = BakedTerrain::new(&spec).unwrap();
        terrain
            .insert_chunk(-128, 40, 384, 256, &vec![0; 384 * 256 * 2])
            .unwrap();
        let mut entities = roads.entities();
        entities.extend(development.building_entities());
        let style = serde_json::json!({"landUseProfile":{"urbanCoreRatio":0.1,"suburbanRatio":0.2,"farmRatio":0.3,"pastureRatio":0.1,"forestRatio":0.1,"reserveRatio":0.1,"greenRatio":0.1}});
        let water = water_geometry(
            water_kind(&serde_json::Value::Null),
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        let surfaces =
            landcover::realize(&spec, &development, &plan, &terrain, water, &entities).unwrap();
        landcover::verify(
            &surfaces,
            &spec,
            &development,
            &plan,
            &terrain,
            water,
            &entities,
        )
        .unwrap();
        let scene = serde_json::from_value(serde_json::json!({
            "sceneId":"ledger", "widthM":384, "depthM":256, "originX":-128, "originZ":40,
            "chunkSizeM":512,"chunkCountX":1,"chunkCountZ":1,"chunks":[],"landmarks":[],"entities":entities,
            "landUseZones":zones,"roadNetworkPlan":roads,"developmentPlan":development,
            "landcoverPlan":plan,"landcoverSurfaces":surfaces,
        })).unwrap();
        (scene, LandUseProfile::from_style(&style).unwrap())
    }
    #[test]
    fn purpose_and_cover_conserve_area_without_reclassifying_entity_kinds() {
        let (scene, profile) = fixture();
        let ledger = analyze(&scene, &profile).unwrap();
        let rural = &ledger.by_purpose["rural"];
        assert_eq!(rural.physical.buildings_m2, 576.);
        assert!(rural.physical.cultivated_soil_m2 > 0.);
        assert!(ledger.by_purpose["urban"].physical.roads_m2 > 0.);
        assert!(rural.physical.roads_m2 > 0.);
        assert_eq!(ledger.by_purpose["nature"].realized_m2, 32768.);
        assert_eq!(ledger.by_purpose["nature"].unresolved_m2, 0.);
        assert!((ledger.realized_m2 + ledger.unresolved_m2 - 384. * 256.).abs() < EPS);
        let old = crate::worldgen::plan_landuse_budget(&scene, &profile);
        let developed = ledger.by_purpose["urban"].realized_m2
            + ledger.by_purpose["rural"].realized_m2
            + ledger.by_purpose["nature"].realized_m2;
        assert!((old.assigned_m2 - developed).abs() < EPS);
        // Conservation coverage is a separate natural realization introduced
        // by this ledger revision and was intentionally absent from r25.
        assert!(ledger.by_purpose["nature"].realized_m2 > 0.);
        assert!((old.actual_transport_m2 - ledger.transport_subset_m2).abs() < EPS);
        assert!((rural.realized_scene_ratio - rural.realized_m2 / (384. * 256.)).abs() < EPS);
        assert_eq!(ledger.status, "incomplete");
        assert!(scene.entities.iter().any(|e| e.kind == "residential_home"));
        assert_eq!(crate::worldgen::analyze_landuse_areas(&scene).rural_m2, 0.);
    }
    /// B01 follow-up: an explicit along-road conversion must move EXACTLY the converted buildable
    /// core from rural realization to urban realization, conserve every area identity, and be
    /// auditable against the plan instead of trusting a published number.
    #[test]
    fn explicit_conversion_moves_exactly_the_buildable_core_and_forgery_fails() {
        let (mut scene, profile) = fixture();
        // The fixture's only frontage parcel is a 24x24 m home on a 28x28 m parcel with the legacy 2 m
        // insets, so the 1 m skin inside those insets leaves a 22x22 = 484 m2 buildable core. It stands
        // in the rural zone, which is exactly the land an explicit conversion may take.
        let site_id = scene.road_network_plan.as_ref().unwrap().roadside_sites[0]
            .id
            .clone();
        assert_eq!(site_id, "home");
        let record = Conversion::rural_to_urban(&site_id, 484);
        let baseline = analyze(&scene, &profile).unwrap();
        let request = frontage::Request {
            id: "homes".into(),
            scene_id: "ledger".into(),
            purpose: "urban".into(),
            kind: "residential_home".into(),
            count: 1,
            width_m: 24,
            depth_m: 24,
            height_m: 8,
            station_spacing_m: 16,
            setback_m: 8,
            max_candidate_attempts: 8,
            parcel_insets: None,
            center_distance_m: None,
            center_world: None,
            district_id: None,
            construction_cohort: None,
            rural_conversion: frontage::RuralConversion::RuralToUrban,
        };
        scene.road_network_plan.as_mut().unwrap().frontage = Some(frontage::Report {
            version: 1,
            model: "local-road-station-frontage-v1".into(),
            outcomes: vec![frontage::Outcome {
                request,
                site_ids: vec![site_id.clone()],
                attempted: 1,
                unfilled: 0,
                rejected: BTreeMap::new(),
                stop_reason: "demand_satisfied".into(),
            }],
            conversions: vec![record.clone()],
        });
        let zones = scene.land_use_zones.clone();
        let ledger = analyze(&scene, &profile).unwrap();
        assert_eq!(ledger.conversions, vec![record.clone()]);
        let urban = &ledger.by_purpose["urban"];
        let rural = &ledger.by_purpose["rural"];
        let base_urban = &baseline.by_purpose["urban"];
        let base_rural = &baseline.by_purpose["rural"];
        // 484 m2 moved, once, into urban and out of rural, and nowhere else
        assert_eq!(ledger.conversions.iter().map(|c| c.area_m2).sum::<i64>(), 484);
        assert!((urban.realized_m2 - base_urban.realized_m2 - 484.).abs() < EPS);
        assert!((base_rural.realized_m2 - rural.realized_m2 - 484.).abs() < EPS);
        assert!((urban.realized_m2 - urban.physical.total() - 484.).abs() < EPS);
        assert!((rural.physical.total() - rural.realized_m2 - 484.).abs() < EPS);
        assert_eq!(
            ledger.by_purpose["nature"].realized_m2,
            baseline.by_purpose["nature"].realized_m2
        );
        // the conservation identities still hold on the AFTER state
        assert!((ledger.realized_m2 + ledger.unresolved_m2 - 384. * 256.).abs() < EPS);
        assert!((ledger.realized_m2 - baseline.realized_m2).abs() < EPS);
        assert!((ledger.unresolved_m2 - baseline.unresolved_m2).abs() < EPS);
        for (kind, row) in &ledger.by_purpose {
            assert!(
                (row.realized_m2 + row.unresolved_m2 - row.planned_m2).abs() < EPS,
                "{kind} row does not conserve planned area"
            );
        }
        let spec = SceneSpec {
            scene_id: "ledger".into(),
            width_m: 384,
            depth_m: 256,
            origin_x: -128,
            origin_z: 40,
            seed_offset: 0,
        };
        let dry = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::None,
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        let plan = scene.road_network_plan.as_ref().unwrap();
        let audit = audit_conversions(plan, &zones, dry, Some(&ledger));
        assert_eq!(audit.status, "pass", "{audit:?}");
        assert_eq!(
            (audit.planned, audit.applied, audit.converted_m2),
            (1, 1, 484)
        );
        // a ledger that is not published at all cannot carry the claim
        let unpublished = audit_conversions(plan, &zones, dry, None);
        assert_eq!(unpublished.status, "fail");
        assert_eq!(
            unpublished.failures,
            vec!["no published ledger carries the 1 conversion(s) the plan claims".to_string()]
        );
        // A hand-edited ledger row cannot pass: the records must be the plan's records.
        let mut forged = ledger.clone();
        forged.conversions[0].area_m2 = 500;
        let forged_audit = audit_conversions(plan, &zones, dry, Some(&forged));
        assert_eq!(forged_audit.status, "fail");
        assert!(
            forged_audit
                .failures
                .iter()
                .any(|failure| failure.contains("but the plan records")),
            "{forged_audit:?}"
        );
        // and the ledger itself re-derives every claimed area, so a forged plan cannot move area
        let mut inflated = scene.clone();
        inflated
            .road_network_plan
            .as_mut()
            .unwrap()
            .frontage
            .as_mut()
            .unwrap()
            .conversions[0]
            .area_m2 = 500;
        assert_eq!(
            analyze(&inflated, &profile).unwrap_err(),
            "ledger conversion parcel home declares 500 m2 but its buildable core is 484 m2"
        );
        // a parcel the plan does not hold cannot be converted
        let mut ghost = scene.clone();
        ghost
            .road_network_plan
            .as_mut()
            .unwrap()
            .frontage
            .as_mut()
            .unwrap()
            .conversions[0]
            .parcel_id = "ghost".into();
        assert_eq!(
            analyze(&ghost, &profile).unwrap_err(),
            "conversion parcel ghost is not a planned frontage parcel"
        );
        // the same parcel may not be counted twice
        let mut twice = scene.clone();
        twice
            .road_network_plan
            .as_mut()
            .unwrap()
            .frontage
            .as_mut()
            .unwrap()
            .conversions
            .push(record.clone());
        assert_eq!(
            analyze(&twice, &profile).unwrap_err(),
            "ledger conversion parcel home is counted twice"
        );
        // water has no authority in the ledger, but the quality gate supplies it: the same honest
        // record fails the audit once a lake covers the parcel
        let lake = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::Lake,
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        let wet_audit = audit_conversions(plan, &zones, lake, Some(&ledger));
        assert_eq!(wet_audit.status, "fail");
        assert_eq!(
            wet_audit.failures,
            vec!["conversion parcel home touches water".to_string()]
        );
    }

    #[test]
    fn missing_duplicate_or_moved_owners_never_earn_footprint_credit() {
        let (scene, profile) = fixture();
        for mode in 0..3 {
            let mut bad = scene.clone();
            let i = bad
                .entities
                .iter()
                .position(|e| e.kind == "residential_home")
                .unwrap();
            match mode {
                0 => {
                    bad.entities.remove(i);
                }
                1 => bad.entities.push(bad.entities[i].clone()),
                _ => bad.entities[i].world_x += 1,
            }
            assert_eq!(
                analyze(&bad, &profile).unwrap().by_purpose["rural"]
                    .physical
                    .buildings_m2,
                0.
            );
        }
    }
    /// A ROADSIDE PAD converts land the same way an along-road request does, so the ledger must
    /// apply its records and `audit_conversions` must re-derive every one of them from the pad's own
    /// parcel. The pad programme is not a frontage request: its record lives on the plan, and this
    /// test is the reason the audit reads BOTH lists as one.
    #[test]
    fn a_roadside_pad_conversion_is_applied_and_every_forgery_fails_by_name() {
        let (mut scene, profile) = fixture();
        let spec = SceneSpec {
            scene_id: "ledger".into(),
            width_m: 384,
            depth_m: 256,
            origin_x: -128,
            origin_z: 40,
            seed_offset: 0,
        };
        // The fixture's single pad: a 24x24 home on the legacy 2/2/2 parcel, so its 22x22 = 484 m2
        // buildable core is what a conversion may take - and it stands in the rural zone.
        let site_id = scene.road_network_plan.as_ref().unwrap().roadside_sites[0]
            .id
            .clone();
        assert_eq!(site_id, "home");
        let record = Conversion::rural_to_urban(&site_id, 484);
        let baseline = analyze(&scene, &profile).unwrap();
        assert!(baseline.conversions.is_empty());
        // The pad DECLARES its purpose before it lands, and the plan records the conversion.
        {
            let plan = scene.road_network_plan.as_mut().unwrap();
            plan.roadside_sites[0].purpose = Some("urban".into());
            plan.pad_conversions = vec![record.clone()];
        }
        let zones = scene.land_use_zones.clone();
        let ledger = analyze(&scene, &profile).unwrap();
        assert_eq!(ledger.conversions, vec![record.clone()]);
        let urban = &ledger.by_purpose["urban"];
        let rural = &ledger.by_purpose["rural"];
        assert!((urban.realized_m2 - baseline.by_purpose["urban"].realized_m2 - 484.).abs() < EPS);
        assert!((baseline.by_purpose["rural"].realized_m2 - rural.realized_m2 - 484.).abs() < EPS);
        assert!((urban.realized_m2 - urban.physical.total() - 484.).abs() < EPS);
        assert!((rural.physical.total() - rural.realized_m2 - 484.).abs() < EPS);
        assert_eq!(
            ledger.by_purpose["nature"].realized_m2,
            baseline.by_purpose["nature"].realized_m2
        );
        for (kind, row) in &ledger.by_purpose {
            assert!(
                (row.realized_m2 + row.unresolved_m2 - row.planned_m2).abs() < EPS,
                "{kind} row does not conserve planned area"
            );
        }
        let dry = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::None,
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        let plan = scene.road_network_plan.as_ref().unwrap();
        // The audit covers the pad record exactly as it covers a frontage one.
        let audit = audit_conversions(plan, &zones, dry, Some(&ledger));
        assert_eq!(audit.status, "pass", "{audit:?}");
        assert_eq!((audit.planned, audit.applied, audit.converted_m2), (1, 1, 484));
        // A pad that earned a conversion but left the record out is named, not silently kept rural.
        let mut missing_record = scene.clone();
        missing_record
            .road_network_plan
            .as_mut()
            .unwrap()
            .pad_conversions
            .clear();
        let unpublished_claim = analyze(&missing_record, &profile).unwrap();
        assert!(unpublished_claim.conversions.is_empty());
        let missing_audit = audit_conversions(
            missing_record.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&unpublished_claim),
        );
        assert_eq!(missing_audit.status, "fail");
        assert_eq!(
            missing_audit.failures,
            vec!["pad conversion parcel home is not recorded in the plan".to_string()]
        );
        // A forged area is refused by BOTH layers: the ledger recomputes it, the audit compares it.
        let mut inflated = scene.clone();
        inflated
            .road_network_plan
            .as_mut()
            .unwrap()
            .pad_conversions[0]
            .area_m2 = 500;
        assert_eq!(
            analyze(&inflated, &profile).unwrap_err(),
            "ledger conversion parcel home declares 500 m2 but its buildable core is 484 m2"
        );
        let inflated_audit = audit_conversions(
            inflated.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&ledger),
        );
        assert_eq!(inflated_audit.status, "fail");
        assert_eq!(
            inflated_audit.failures,
            vec![
                "conversion parcel home claims 500 m2 but its buildable core is 484 m2".to_string(),
                // and the published ledger, which is honest, no longer agrees with the plan
                "ledger conversion parcel home is rural-to-urban 484 m2 but the plan records 500 m2"
                    .to_string(),
            ]
        );
        // The same parcel may be counted once, and the ledger rows must be exactly the plan's.
        let mut twice = scene.clone();
        twice
            .road_network_plan
            .as_mut()
            .unwrap()
            .pad_conversions
            .push(record.clone());
        assert_eq!(
            analyze(&twice, &profile).unwrap_err(),
            "ledger conversion parcel home is counted twice"
        );
        let twice_audit = audit_conversions(
            twice.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&ledger),
        );
        assert_eq!(
            twice_audit.failures,
            vec![
                "plan conversion parcel home is recorded twice".to_string(),
                "ledger applied 1 conversions while the plan records 2".to_string(),
            ]
        );
        // One record in the plan, none in the published ledger, is also a failure.
        let mut unapplied = scene.clone();
        unapplied
            .road_network_plan
            .as_mut()
            .unwrap()
            .pad_conversions
            .clear();
        let unapplied_audit = audit_conversions(
            unapplied.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&ledger),
        );
        assert_eq!(unapplied_audit.status, "fail");
        assert_eq!(
            unapplied_audit.failures,
            vec![
                "pad conversion parcel home is not recorded in the plan".to_string(),
                "ledger applied 1 conversions while the plan records 0".to_string(),
            ]
        );
        // A parcel that reaches protected land may not be converted: the pad's own zoning gate
        // refuses such a pad, and the audit refuses a record for one by name.
        let mut protected_zones = zones.clone();
        for zone in &mut protected_zones {
            if zone.kind == "rural" {
                zone.kind = "nature".into();
            }
        }
        assert_eq!(
            frontage::verify_conversion(plan, &protected_zones, None, &record).unwrap_err(),
            "conversion parcel home claims protected land"
        );
        // A pad whose declared purpose contradicts its own kind is refused by name too.
        let mut forged_purpose = scene.clone();
        let plan = forged_purpose.road_network_plan.as_mut().unwrap();
        plan.roadside_sites[0].program = Some(crate::worldgen::services::SiteProgram {
            request_id: "forged".into(),
            kind: "turnip_cart".into(),
            height_m: 4,
            district_id: None,
            construction_cohort: None,
        });
        assert_eq!(
            frontage::verify_conversion(
                forged_purpose.road_network_plan.as_ref().unwrap(),
                &zones,
                None,
                &record
            )
            .unwrap_err(),
            "conversion parcel home declares purpose urban but its kind turnip_cart is no settlement purpose"
        );
    }


    /// A pad the road plan REFUSED by name is not a parcel of this world any more. The ledger may not
    /// book it, the plan may not keep its record, and a plan that claims both a refusal and a pad is
    /// itself a failure - each named, so "planned, booked, deleted from the world" can never pass as
    /// an agreement between the plan and the ledger.
    #[test]
    fn a_ledger_that_still_books_a_refused_pad_is_refused_by_name() {
        let (mut scene, profile) = fixture();
        let spec = SceneSpec {
            scene_id: "ledger".into(),
            width_m: 384,
            depth_m: 256,
            origin_x: -128,
            origin_z: 40,
            seed_offset: 0,
        };
        let record = Conversion::rural_to_urban("home", 484);
        {
            let plan = scene.road_network_plan.as_mut().unwrap();
            plan.roadside_sites[0].purpose = Some("urban".into());
            plan.pad_conversions = vec![record.clone()];
        }
        let zones = scene.land_use_zones.clone();
        let honest_ledger = analyze(&scene, &profile).unwrap();
        assert_eq!(honest_ledger.conversions, vec![record.clone()]);
        let dry = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::None,
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        let categories =
            crate::worldgen::road_network::PAD_REFUSED_CATEGORIES.join(" or ");
        let categories = categories.as_str();
        // The refusal the sweep writes when the pad cannot hold its declared top.
        let refusal = crate::worldgen::road_network::BaseProfileRejection {
            edge_id: "home".into(),
            category: crate::worldgen::road_network::PAD_FILL_CONFLICT_CATEGORY.into(),
            reason: "pad home (24x24, declared top 0.000 m on road road2) cannot hold its flat top: \
                     pad.other stands 4.970 m at (64, 188)"
                .into(),
        };
        // 1. The honest ledger PLUS the refusal is a plan/ledger disagreement: the ledger books land
        //    for a pad the plan refused, and the plan still lists the pad.
        scene
            .road_network_plan
            .as_mut()
            .unwrap()
            .roadside_site_rejections = vec![refusal.clone()];
        let refused_audit = audit_conversions(
            scene.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&honest_ledger),
        );
        assert_eq!(refused_audit.status, "fail", "{refused_audit:?}");
        assert_eq!(refused_audit.refused_pads, 1);
        assert!(refused_audit.failures.iter().any(|failure| failure
            == &format!("pad home is refused by the plan ({categories}) and still planned")),
            "{refused_audit:?}");
        assert!(refused_audit.failures.iter().any(|failure| failure
            == &format!("conversion parcel home was refused by the plan ({categories}) yet the plan still books 484 m2")),
            "{refused_audit:?}");
        assert!(refused_audit.failures.iter().any(|failure| failure
            == &format!("ledger books conversion parcel home which the plan refused ({categories}); the pad has no entity")),
            "{refused_audit:?}");
        // 2. A plan that took the pad out but kept the record is refused by name as well: the record
        //    would move 484 m2 for a parcel the plan does not plan.
        let mut dropped_pad = scene.clone();
        dropped_pad
            .road_network_plan
            .as_mut()
            .unwrap()
            .roadside_sites
            .clear();
        let dropped_audit = audit_conversions(
            dropped_pad.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&honest_ledger),
        );
        assert_eq!(dropped_audit.status, "fail", "{dropped_audit:?}");
        assert!(
            dropped_audit
                .failures
                .contains(&"conversion parcel home is not a planned frontage parcel".to_string()),
            "{dropped_audit:?}"
        );
        assert!(dropped_audit.failures.iter().any(|failure| failure
            == &format!("conversion parcel home was refused by the plan ({categories}) yet the plan still books 484 m2")),
            "{dropped_audit:?}");
        assert!(dropped_audit.failures.iter().any(|failure| failure
            == &format!("ledger books conversion parcel home which the plan refused ({categories}); the pad has no entity")),
            "{dropped_audit:?}");
        // 3. The fix's own shape passes: the pad is out, the record is out, its area stays rural.
        let mut fixed = dropped_pad.clone();
        fixed
            .road_network_plan
            .as_mut()
            .unwrap()
            .pad_conversions
            .clear();
        let fixed_ledger = analyze(&fixed, &profile).unwrap();
        assert!(fixed_ledger.conversions.is_empty());
        let fixed_audit = audit_conversions(
            fixed.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&fixed_ledger),
        );
        assert_eq!(fixed_audit.status, "pass", "{fixed_audit:?}");
        assert_eq!(
            (
                fixed_audit.planned,
                fixed_audit.applied,
                fixed_audit.converted_m2
            ),
            (0, 0, 0)
        );
        assert_eq!(fixed_audit.refused_pads, 1);
        // 4. A published ledger that keeps the record the plan dropped is refused by name too.
        let mut stray_ledger = fixed_ledger.clone();
        stray_ledger.conversions = vec![record];
        let stray_audit = audit_conversions(
            fixed.road_network_plan.as_ref().unwrap(),
            &zones,
            dry,
            Some(&stray_ledger),
        );
        assert_eq!(stray_audit.status, "fail", "{stray_audit:?}");
        assert!(stray_audit.failures.iter().any(|failure| failure
            == &format!("ledger books conversion parcel home which the plan refused ({categories}); the pad has no entity")),
            "{stray_audit:?}");
    }

    /// A pad the road plan takes back out AFTER placement leaves no record in the
    /// frontage report behind, and the ledger cannot keep one.
    ///
    /// Round 28 measured the defect: a refused pad kept its rural-to-urban
    /// conversion in `plan.frontage.conversions`, so the ledger's union of plan
    /// records named a parcel the plan no longer carried and `analyze` refused the
    /// whole world with "conversion parcel ... is not a planned frontage parcel".
    /// `Report::withdraw_site` moves the outcome and the conversion with the pad;
    /// this test pins both the withdrawal and the audit that names a record left
    /// behind - the frontage table included.
    #[test]
    fn a_refused_frontage_pad_withdraws_its_conversion_and_the_ledger_cannot_keep_it() {
        let (mut scene, profile) = fixture();
        let spec = SceneSpec {
            scene_id: "ledger".into(),
            width_m: 384,
            depth_m: 256,
            origin_x: -128,
            origin_z: 40,
            seed_offset: 0,
        };
        // The fixture's home is served by the along-road programme: one accepted
        // parcel and the 484 m2 conversion its buildable core earned.
        let report: frontage::Report = serde_json::from_value(serde_json::json!({
            "version": 1,
            "model": "local-road-station-frontage-v1",
            "outcomes": [{
                "request": {
                    "id": "homes", "sceneId": "ledger", "purpose": "urban",
                    "kind": "residential_home", "count": 1, "widthM": 24, "depthM": 24,
                    "heightM": 8, "stationSpacingM": 16, "setbackM": 8,
                    "maxCandidateAttempts": 16
                },
                "siteIds": ["home"], "attempted": 1, "unfilled": 0,
                "rejected": {}, "stopReason": "demand_satisfied"
            }],
            "conversions": [{"parcelId": "home", "from": "rural", "to": "urban", "areaM2": 484}]
        }))
        .unwrap();
        scene.road_network_plan.as_mut().unwrap().frontage = Some(report);
        let zones = scene.land_use_zones.clone();
        let zones = zones.as_slice();
        let dry = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::None,
            &[],
            &spec,
            &serde_json::Value::Null,
        );
        // Before the refusal the plan is honest: one record, one applied conversion.
        let honest = analyze(&scene, &profile).unwrap();
        assert_eq!(honest.conversions.len(), 1);
        assert_eq!(
            audit_conversions(
                scene.road_network_plan.as_ref().unwrap(),
                zones,
                dry,
                Some(&honest)
            )
            .status,
            "pass"
        );
        // The refusal: exactly the three moves `refuse_unrealizable_pads` makes.
        let refusal = crate::worldgen::road_network::BaseProfileRejection {
            edge_id: "home".into(),
            category: crate::worldgen::road_network::PAD_FILL_CONFLICT_CATEGORY.into(),
            reason: "pad home cannot hold its flat top".into(),
        };
        {
            let plan = scene.road_network_plan.as_mut().unwrap();
            plan.roadside_sites.retain(|site| site.id != "home");
            plan.roadside_site_rejections = vec![refusal];
            let report = plan.frontage.as_mut().expect("frontage report");
            assert!(
                report.withdraw_site("home"),
                "the report must have accepted the pad, or the fixture proves nothing"
            );
            assert!(report.conversions.is_empty());
            assert!(report.outcomes[0].site_ids.is_empty());
            assert_eq!(report.outcomes[0].unfilled, 1);
            assert_eq!(report.outcomes[0].stop_reason, "no_feasible_frontage");
        }
        // The ledger no longer moves a square metre for the withdrawn parcel, and the
        // audit closes with the refusal recorded.
        let withdrawn = analyze(&scene, &profile).unwrap();
        assert!(withdrawn.conversions.is_empty());
        let audit = audit_conversions(
            scene.road_network_plan.as_ref().unwrap(),
            zones,
            dry,
            Some(&withdrawn),
        );
        assert_eq!(audit.status, "pass", "{audit:?}");
        assert_eq!((audit.planned, audit.applied, audit.converted_m2), (0, 0, 0));
        assert_eq!(audit.refused_pads, 1);
        // A record left in the FRONTAGE table - the Round 28 defect exactly - makes
        // the ledger fail by name, and the audit names the table that kept it.
        let mut stray = scene.clone();
        let plan = stray.road_network_plan.as_mut().unwrap();
        let report = plan.frontage.as_mut().unwrap();
        report.conversions = vec![Conversion::rural_to_urban("home", 484)];
        report.outcomes[0].site_ids = vec!["home".into()];
        assert_eq!(
            analyze(&stray, &profile).unwrap_err(),
            "conversion parcel home is not a planned frontage parcel"
        );
        let stray_audit = audit_conversions(
            stray.road_network_plan.as_ref().unwrap(),
            zones,
            dry,
            Some(&withdrawn),
        );
        assert_eq!(stray_audit.status, "fail");
        assert!(
            stray_audit.failures.contains(
                &"frontage conversion parcel home is not a planned pad; the frontage report \
                  still books 484 m2"
                    .to_string()
            ),
            "{:?}",
            stray_audit.failures
        );
        assert!(
            stray_audit.failures.contains(
                &"frontage outcome homes names pad home which the plan does not carry".to_string()
            ),
            "{:?}",
            stray_audit.failures
        );
    }

    #[test]
    fn area_source_and_geometry_forgery_fail_closed() {
        let (scene, profile) = fixture();
        // Each forgery must fail for its OWN reason. Asserting only `is_err()` would let a forgery pass
        // because some unrelated guard fired first, which is exactly the trap the reserved-purpose check
        // sits behind: landuse_ledger's "hard development in reserved ledger purpose" is UNREACHABLE
        // through analyze(), because the surface/source purpose mismatch is caught earlier at line 188
        // and development.rs already refuses non-urban, non-rural development cells upstream.
        let expected = [
            (0, "ledger surface declared area differs from geometry"),
            (1, "ledger surface purpose differs from source"),
            (2, "ledger surface has no source cell"),
            (3, "duplicate ledger surface id"),
        ];
        for (mode, reason) in expected {
            let mut bad = scene.clone();
            let patches = &mut bad.landcover_surfaces.as_mut().unwrap().patches;
            match mode {
                0 => patches[0].area_m2 += 1.,
                1 => patches[0].zone_kind = "nature".into(),
                2 => patches[0].source_cell_id = "forged".into(),
                _ => patches.push(patches[0].clone()),
            }
            match analyze(&bad, &profile) {
                Err(error) => assert_eq!(
                    error, reason,
                    "forgery mode {mode} must fail for its own reason, not for an unrelated guard"
                ),
                Ok(_) => panic!("forgery mode {mode} must not analyse cleanly"),
            }
        }
        let mut missing = scene.clone();
        missing.landcover_surfaces = None;
        assert!(analyze(&missing, &profile).is_err());
    }
    #[test]
    fn residual_audit_splits_policy_remainder_and_rejections_from_unresolved() {
        fn cell(
            id: &str,
            zone: &str,
            kind: &str,
            role: development::LandcoverRole,
            width_m: u32,
            depth_m: u32,
        ) -> development::LandcoverCell {
            development::LandcoverCell {
                id: id.into(),
                boundary: development::Rect {
                    x: 0,
                    z: 0,
                    width_m,
                    depth_m,
                },
                source_cell_id: id.into(),
                zone_id: zone.into(),
                zone_kind: kind.into(),
                role,
                realization_status: "planned".into(),
                realization_reason: Some("test".into()),
            }
        }
        fn patch(
            id: &str,
            source: &str,
            zone: &str,
            material: Material,
            width_m: u32,
            depth_m: u32,
        ) -> landcover::SurfacePatch {
            let w = width_m as f64;
            let d = depth_m as f64;
            landcover::SurfacePatch {
                id: id.into(),
                source_cell_id: source.into(),
                parcel_id: source.into(),
                zone_kind: zone.into(),
                material,
                area_m2: w * d,
                vertices: vec![[0., 0., 0.], [0., 0., d], [w, 0., d], [w, 0., 0.]],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            }
        }
        let plan = development::LandcoverPlan {
            version: 1,
            source: "test".into(),
            scene_id: "s".into(),
            scene_area_m2: 333,
            cells: vec![
                cell("u-future", "u", "urban", development::LandcoverRole::FutureDevelopment, 10, 10),
                cell("u-yard", "u", "urban", development::LandcoverRole::ManagedYard, 2, 2),
                cell("r-agri", "r", "rural", development::LandcoverRole::AgricultureCandidate, 20, 10),
                cell("n-cons", "n", "nature", development::LandcoverRole::ConservationCandidate, 5, 5),
                cell("x-unassigned", "x", "unassigned", development::LandcoverRole::Unmanaged, 2, 2),
            ],
            farm_pattern: None,
            residual_program: Some(super::super::residual_land::SceneResidualProgram {
                urban_open_space_ratio: Some(0.1),
                rural_pasture_ratio: Some(0.2),
                coverage: None,
            }),
        };
        let surfaces = landcover::SurfaceRealization {
            version: 1,
            scene_id: "s".into(),
            final_heightfield_hash: "h".into(),
            patches: vec![
                patch("u-open", "u-future", "urban", Material::PublicOpenSpace, 10, 1),
                patch("u-yard-grass", "u-yard", "urban", Material::ManagedGrass, 3, 1),
                patch("r-soil", "r-agri", "rural", Material::CultivatedSoil, 10, 6),
                patch("r-headland", "r-agri", "rural", Material::FieldHeadland, 10, 2),
                patch("r-track", "r-agri", "rural", Material::FarmTrack, 10, 1),
                patch("n-ground", "n-cons", "nature", Material::ConservationGround, 5, 5),
            ],
            rejections: vec![landcover::SurfaceRejection {
                source_cell_id: "u-yard".into(),
                x: 0,
                z: 0,
                area_m2: 1,
                reason: "residual_grade_exceeds_25_percent".into(),
            }],
            agriculture: Default::default(),
            ecology: Default::default(),
        };
        let audit = residual_audit(&plan, &surfaces).unwrap();
        assert_eq!(audit.status, "incomplete");
        assert_eq!(audit.candidate_m2, 333.);
        assert_eq!(audit.target_m2, 83.);
        assert_eq!(audit.realized_m2, 128.);
        assert_eq!(audit.policy_remainder_m2, 250.);
        assert_eq!(audit.rejected_attempt_m2, 1.);
        assert_eq!(audit.unresolved_m2, 205.);
        assert!(audit.failures.is_empty());
        let find = |zone: &str, sub_use: &str| {
            audit
                .accounts
                .iter()
                .find(|account| account.source_zone_id == zone && account.sub_use == sub_use)
                .unwrap()
        };
        let urban_open = find("u", "urban_public_open_space");
        assert_eq!(
            (
                urban_open.candidate_m2,
                urban_open.target_m2,
                urban_open.realized_m2,
                urban_open.policy_remainder_m2,
                urban_open.unresolved_m2,
            ),
            (100., 10., 10., 90., 90.)
        );
        let yard = find("u", "managed_yard");
        assert_eq!(
            (yard.realized_m2, yard.rejected_attempt_m2, yard.unresolved_m2),
            (3., 1., 1.)
        );
        let rural = find("r", "rural_pasture");
        assert_eq!(
            (rural.target_m2, rural.realized_m2, rural.unresolved_m2),
            (40., 90., 110.)
        );
        let conservation = find("n", "conservation");
        assert_eq!(
            (conservation.realized_m2, conservation.unresolved_m2),
            (25., 0.)
        );
        let unmanaged = find("x", "unmanaged");
        assert_eq!(unmanaged.unresolved_m2, 4.);
        let json = serde_json::to_value(&audit).unwrap();
        assert_eq!(json["unresolvedM2"], serde_json::json!(205.));
    }

    #[test]
    fn residual_audit_is_absent_from_a_legacy_ledger() {
        let (scene, profile) = fixture();
        let ledger = analyze(&scene, &profile).unwrap();
        assert!(ledger.residual.is_none());
        let json = serde_json::to_value(&ledger).unwrap();
        assert!(json.get("residual").is_none(), "legacy ledger grew a residual field");
    }

    #[test]
    fn reserved_candidates_and_small_balanced_output_are_not_completion() {
        let (mut scene, profile) = fixture();
        scene.entities.clear();
        scene.landcover_surfaces.as_mut().unwrap().patches.clear();
        let empty = analyze(&scene, &profile).unwrap();
        assert_eq!(empty.realized_m2, 0.);
        assert_eq!(empty.unresolved_m2, empty.scene_area_m2);
        assert_eq!(empty.status, "incomplete");
        assert!(empty.by_purpose["rural"].target_deficit_m2 > 0.);
        // The helper only accounts geometry; independent reconstruction must
        // separately reject missing meshes. Clearing them cannot earn credit.
    }
}
