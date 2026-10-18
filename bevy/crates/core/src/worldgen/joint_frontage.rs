//! Service reservation plus residential supply recovery, before entity creation.
//! This is bounded deterministic planning, not a global optimum or an edit of
//! an existing baked world. A minimum that cannot be recovered stays unmet.
use super::{
    AccessGraph, EntityInstance, LandUseZone, SceneSpec, WaterGeometry,
    road_network::{RoadNetworkPlan, RoadsideSite},
    services::{self, ServiceProgram, ServiceRequest},
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointLayout {
    pub model: String,
    pub residential_minimum: usize,
    pub baseline_residential_count: usize,
    pub retained_residential_ids: Vec<String>,
    pub displaced_residential_ids: Vec<String>,
    pub replacement_residential_ids: Vec<String>,
    pub unfilled_residential: usize,
    pub recovery_rejections: BTreeMap<String, u32>,
}
fn overlap(a: (i64, i64, i64, i64), b: (i64, i64, i64, i64)) -> bool {
    (a.0 - b.0).abs() < a.2 + b.2 && (a.1 - b.1).abs() < a.3 + b.3
}
fn parcel(s: &RoadsideSite) -> (i64, i64, i64, i64) {
    let (w, d) = s.world_half_extents();
    (
        i64::from(s.world_x),
        i64::from(s.world_z),
        i64::from(w) + 2,
        i64::from(d) + 2,
    )
}
pub(super) fn incompatible(a: &RoadsideSite, b: &RoadsideSite, p: &RoadNetworkPlan) -> bool {
    overlap(parcel(a), parcel(b))
        || services::entrance_corridor(a, p).is_some_and(|c| overlap(c, parcel(b)))
        || services::entrance_corridor(b, p).is_some_and(|c| overlap(c, parcel(a)))
}
pub(super) fn plan(
    baseline: RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    program: &ServiceProgram,
    pad_conversion: super::frontage::RuralConversion,
) -> Result<RoadNetworkPlan, TerrainError> {
    if !program
        .requests
        .iter()
        .any(|r| r.scene_id == scene.scene_id)
        && !program.housing_minimum.contains_key(&scene.scene_id)
    {
        return Ok(baseline);
    }
    let minimum = program
        .housing_minimum
        .get(&scene.scene_id)
        .map_or(baseline.roadside_sites.len(), |v| *v as usize);
    let mut joint = baseline.clone();
    // Discard only a clone's provisional selections. The original candidate
    // identities remain available below; no final entity exists at this point.
    joint.roadside_sites.clear();
    // The shoreline row's outcome was published by `worldgen::shorefront` BEFORE this layout, and this
    // layout does not re-plan it: it is not a service of the settlement programme and its refusals are
    // the only record of why a quay station did not land. The service outcomes are cleared only to be
    // re-derived below, so the row's own record is carried across and kept first, in plan order.
    let shorefront_outcomes: Vec<super::services::ServiceOutcome> = joint
        .service_outcomes
        .iter()
        .filter(|outcome| outcome.request.id == super::shorefront::SHOREFRONT_ROW)
        .cloned()
        .collect();
    joint.service_outcomes.clear();
    // A baseline site that is NOT a dwelling is not part of this layout at all. The shoreline
    // commercial row (`worldgen::shorefront`) plans its shops on the road network before the pad
    // programme runs, and they are baseline sites too: they must keep their frontage through the
    // service reservation (the services below then reserve AROUND them) and they must never be
    // counted as dwellings, which is what `retained/displaced/retained_residential_ids` and the
    // housing minimum account. Attaching them to this phase costs neither: they are simply already
    // in place when the services and the baseline homes are considered.
    let mut dwellings: Vec<RoadsideSite> = Vec::new();
    for site in &baseline.roadside_sites {
        if site.kind() == "residential_home" {
            dwellings.push(site.clone());
        } else {
            joint.roadside_sites.push(site.clone());
        }
    }
    services::append(&mut joint, scene, natural, water, zones, program, pad_conversion)?;
    joint.service_outcomes.splice(0..0, shorefront_outcomes);
    let service_sites = joint.roadside_sites.clone();
    let mut receipt = JointLayout {
        model: "service-reservation-and-housing-recovery-v1".into(),
        residential_minimum: minimum,
        baseline_residential_count: dwellings.len(),
        retained_residential_ids: vec![],
        displaced_residential_ids: vec![],
        replacement_residential_ids: vec![],
        unfilled_residential: 0,
        recovery_rejections: BTreeMap::new(),
    };
    for home in &dwellings {
        if service_sites
            .iter()
            .any(|s| incompatible(home, s, &baseline))
        {
            receipt.displaced_residential_ids.push(home.id.clone());
        } else {
            receipt.retained_residential_ids.push(home.id.clone());
            joint.roadside_sites.push(home.clone());
        }
    }
    let required = minimum.saturating_sub(receipt.retained_residential_ids.len());
    if required > 0 {
        let start = joint.roadside_sites.len();
        // Reuse exactly the same support, zoning, road and entrance checks as
        // facilities. This internal request is NOT serialized as user demand.
        let internal = ServiceProgram {
            version: 1,
            housing_minimum: BTreeMap::new(),
            requests: vec![ServiceRequest {
                id: "housing-recovery".into(),
                scene_id: scene.scene_id.clone(),
                kind: "residential_home".into(),
                count: required as u32,
                width_m: 24,
                depth_m: 24,
                height_m: 8,
            }],
        };
        let mut recovery = joint.clone();
        services::append(
            &mut recovery,
            scene,
            natural,
            water,
            zones,
            &internal,
            pad_conversion,
        )?;
        receipt.recovery_rejections = recovery
            .service_outcomes
            .last()
            .expect("internal request outcome")
            .rejected_candidates
            .clone();
        for (i, mut site) in recovery.roadside_sites.into_iter().skip(start).enumerate() {
            site.id = format!("generated.residential-recovery.{}.{}", scene.scene_id, i);
            site.program = None;
            receipt.replacement_residential_ids.push(site.id.clone());
            joint.roadside_sites.push(site);
        }
    }
    receipt.unfilled_residential = minimum.saturating_sub(
        joint
            .roadside_sites
            .iter()
            .filter(|s| s.program.is_none())
            .count(),
    );
    joint.joint_layout = Some(receipt);
    joint.model = "joint-frontage-residential-minimum-v2".into();
    Ok(joint)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JointAudit {
    pub status: String,
    pub residential_minimum: usize,
    pub planned_residential: usize,
    pub realized_residential_with_access: usize,
    pub missing_residential: usize,
    pub unfilled_services: u32,
    pub failures: Vec<String>,
}
/// The CLI first reconstructs the whole plan from manifest + natural raster;
/// the saved receipt and saved reports cannot override the following result.
pub fn audit(
    plan: &RoadNetworkPlan,
    entities: &[EntityInstance],
    access: &AccessGraph,
) -> JointAudit {
    let minimum = plan
        .joint_layout
        .as_ref()
        .map_or(0, |p| p.residential_minimum);
    let mut out = JointAudit {
        status: "pass".into(),
        residential_minimum: minimum,
        planned_residential: 0,
        realized_residential_with_access: 0,
        missing_residential: 0,
        unfilled_services: 0,
        failures: vec![],
    };
    for (site, mut expected) in plan.roadside_sites.iter().zip(plan.site_entities()) {
        if site.program.is_some() {
            continue;
        }
        out.planned_residential += 1;
        expected.normalize_spatial_semantics();
        expected.bounds = None;
        let found: Vec<_> = entities.iter().filter(|e| e.entity_id == site.id).collect();
        let exact = found.len() == 1 && {
            let mut e = found[0].clone();
            e.bounds = None;
            serde_json::to_value(&e).ok() == serde_json::to_value(&expected).ok()
        };
        let connected = access.links.iter().any(|l| {
            l.entity_id == site.id
                && l.connected
                && l.required
                && l.road_id.as_deref() == Some(site.road_id.as_str())
        });
        if exact && connected {
            out.realized_residential_with_access += 1;
        } else {
            out.failures.push(format!(
                "{} housing missing, altered or without assigned access",
                site.id
            ));
        }
    }
    let service = services::audit(plan, entities, access);
    out.unfilled_services = service
        .requested
        .saturating_sub(service.realized_with_access as u32);
    out.missing_residential = minimum.saturating_sub(out.realized_residential_with_access);
    out.failures.extend(service.failures);
    if out.missing_residential > 0 || out.unfilled_services > 0 {
        out.status = "incomplete".into();
    }
    if !out.failures.is_empty() {
        out.status = "fail".into();
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (
        RoadNetworkPlan,
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
    ) {
        let scene = SceneSpec {
            scene_id: "joint".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: -256,
            origin_z: -256,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(-256, -256, 512, 512, &vec![0; 512 * 512 * 2])
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: -256,
            world_z: -256,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let roads=serde_json::from_value(serde_json::json!({"sceneId":"joint","model":"test","naturalHeightfieldHash":"test","status":"planned_connected_network",
            "gridSpacingM":64,"maxRoadbedFillM":6.,"levelM":0,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,
            "nodes":[],"edges":[{"id":"road","from":0,"to":1,"worldX":0,"worldZ":0,"lengthM":64,"rotationYDeg":0.,"naturalMinM":0.,"naturalMaxM":0.}],
            "roadsideSites":[{"id":"original-home","roadId":"road","worldX":0,"worldZ":-24,"widthM":24,"depthM":24,"supportLevelM":0,"rotationYDeg":180.,"naturalMinM":0.,"naturalMaxM":0.}]})).unwrap();
        (roads, scene, terrain, water, zones)
    }
    #[test]
    fn reserves_school_and_recovers_displaced_home_without_lowering_minimum() {
        let (base, s, t, w, z) = fixture();
        let program=services::from_style(&serde_json::json!({"settlementProgram":{"version":2,"housingMinimum":{"joint":1},"requests":[
            {"id":"school","sceneId":"joint","kind":"school","count":1,"widthM":40,"depthM":30,"heightM":8}]}})).unwrap().unwrap();
        let joint = plan(base.clone(), &s, &t, w, &z, &program, super::super::frontage::RuralConversion::Disabled).unwrap();
        let receipt = joint.joint_layout.as_ref().unwrap();
        assert_eq!(receipt.residential_minimum, 1);
        assert_eq!(receipt.unfilled_residential, 0);
        assert_eq!(receipt.displaced_residential_ids, vec!["original-home"]);
        assert_eq!(receipt.replacement_residential_ids.len(), 1);
        assert_eq!(joint.roadside_sites.len(), 2);
        assert_eq!(
            serde_json::to_value(&base.edges).unwrap(),
            serde_json::to_value(&joint.edges).unwrap()
        );
        assert!(!incompatible(
            &joint.roadside_sites[0],
            &joint.roadside_sites[1],
            &joint
        ));
        let mut entities = joint.entities();
        entities.extend(joint.site_entities());
        for e in &mut entities {
            e.normalize_spatial_semantics();
        }
        let access = super::super::build_access_graph(&entities, &t, w);
        assert_eq!(audit(&joint, &entities, &access).status, "pass");
        crate::settlement::development::plan(&s, &z, &joint).unwrap();
        let school = entities.iter().find(|e| e.kind == "school").unwrap();
        assert_eq!((school.width_m, school.depth_m), (40., 30.));
        let second = plan(base, &s, &t, w, &z, &program, super::super::frontage::RuralConversion::Disabled).unwrap();
        assert_eq!(
            serde_json::to_value(&joint).unwrap(),
            serde_json::to_value(second).unwrap()
        );
    }
    #[test]
    fn parcel_separation_does_not_allow_either_entrance_to_be_blocked() {
        let (base, _, _, _, _) = fixture();
        let mut a = base.roadside_sites[0].clone();
        let mut b = a.clone();
        a.world_z = 80;
        a.rotation_y_deg = 0.;
        b.world_z = 24;
        b.rotation_y_deg = 0.;
        assert!(!overlap(parcel(&a), parcel(&b)));
        assert!(incompatible(&a, &b, &base));
        assert!(incompatible(&b, &a, &base));
        a.world_x = -24;
        a.world_z = 24;
        b.world_x = 24;
        assert!(!incompatible(&a, &b, &base));
    }
    #[test]
    fn unavailable_housing_is_explicit_and_saved_shortfall_cannot_create_supply() {
        let (mut base, s, t, w, z) = fixture();
        base.edges.clear();
        base.roadside_sites.clear();
        base.level_m = None;
        let program=services::from_style(&serde_json::json!({"settlementProgram":{"version":2,"housingMinimum":{"joint":3},"requests":[]}})).unwrap().unwrap();
        let mut joint = plan(base, &s, &t, w, &z, &program, super::super::frontage::RuralConversion::Disabled).unwrap();
        assert_eq!(joint.joint_layout.as_ref().unwrap().unfilled_residential, 3);
        assert_eq!(
            audit(&joint, &[], &AccessGraph::default()).missing_residential,
            3
        );
        joint.joint_layout.as_mut().unwrap().unfilled_residential = 0;
        assert_eq!(
            audit(&joint, &[], &AccessGraph::default()).status,
            "incomplete"
        );
        assert!(services::from_style(&serde_json::json!({"settlementProgram":{"version":1,"housingMinimum":{"joint":3},"requests":[]}})).is_err());
    }
}
