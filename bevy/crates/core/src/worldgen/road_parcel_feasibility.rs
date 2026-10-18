//! Bounded joint feasibility audit for a candidate road/parcel network.
//! This composes the production corridor, frontage, development, access and
//! landcover functions; it is not a second JS planner and it never mutates
//! natural terrain.
use super::{EntityInstance, LandUseZone, SceneSpec, WaterGeometry};
use crate::{
    baked_terrain::BakedTerrain,
    settlement::{development, landcover},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_JOINT_CANDIDATE_STATES: usize = 256;
pub const MAX_JOINT_EXTENSIONS: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeElevation {
    pub world_x: i32,
    pub world_z: i32,
    pub level_mm: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeElevationAudit {
    pub nodes: Vec<NodeElevation>,
}

#[derive(Debug, Clone)]
pub struct TrustedBaseContract {
    pub natural_hash: String,
    pub base_level_mm: i32,
    pub fixed_nodes: BTreeMap<[i32; 2], i32>,
    pub base_edges: Vec<BaseEdgeContract>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseEdgeContract {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub world_x: i32,
    pub world_z: i32,
    pub length_m: u32,
    pub rotation_y_deg: f32,
    /// B01-T2 C3: the base edge's own authoritative profile, captured from the
    /// already-built, already-validated base plan. A candidate must reproduce it
    /// byte for byte, so a graded base road cannot be silently flattened,
    /// re-graded or re-leveled when a candidate is evaluated.
    pub profile: Option<super::road_profile::RoadProfile>,
}

/// A base contract is derived only from a trusted, already-built base plan.
/// Candidate plans cannot generate their own fixed-node obligations.
pub fn trusted_base_contract(
    base: &super::road_network::RoadNetworkPlan,
    expected_natural_hash: &str,
) -> Result<TrustedBaseContract, String> {
    if base.natural_heightfield_hash != expected_natural_hash {
        return Err("trusted base natural hash differs from source".into());
    }
    let base_level_m = base
        .level_m
        .ok_or("trusted base plan has no fixed road level")?
        .checked_mul(1000)
        .ok_or("trusted base level overflow")?;
    let fixed_nodes = base
        .nodes
        .iter()
        .map(|node| ([node.world_x, node.world_z], base_level_m))
        .collect();
    let base_entities = base.entities();
    for edge in &base.edges {
        let entity = base_entities
            .iter()
            .find(|entity| entity.entity_id == edge.id)
            .ok_or("trusted base edge has no entity")?;
        // B01-T2 C3: identity is the plan's base provenance (this edge is part of
        // the base domain), never the presence or absence of a profile. A base
        // road may now carry its own authoritative graded profile, so accept it
        // and lock it into the contract. It must still pin both endpoints at the
        // base level so shared nodes and access platforms stay fixed; an
        // UNPROFILED base edge (a refused design) must stay flat at the base
        // level exactly as the pre-T2 contract required.
        match &entity.grounding.road_profile {
            Some(profile) => {
                profile.validate_shape().map_err(|error| {
                    format!("trusted base edge {} profile is invalid: {}", edge.id, error.0)
                })?;
                let from = base
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.from)
                    .ok_or("trusted base edge endpoint is not a node")?;
                let to = base
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.to)
                    .ok_or("trusted base edge endpoint is not a node")?;
                if profile.start != [from.world_x, from.world_z]
                    || profile.end != [to.world_x, to.world_z]
                    || profile.elevations_mm.first() != Some(&i64::from(base_level_m))
                    || profile.elevations_mm.last() != Some(&i64::from(base_level_m))
                {
                    return Err(format!(
                        "trusted base edge {} profile does not pin its endpoints at the base level",
                        edge.id
                    ));
                }
            }
            None => {
                if (entity.ground_height_m() * 1000.).round() as i32 != base_level_m {
                    return Err(
                        "trusted base supports only base roads at the base level".into(),
                    );
                }
            }
        }
    }
    Ok(TrustedBaseContract {
        natural_hash: expected_natural_hash.into(),
        base_level_mm: base_level_m,
        fixed_nodes,
        base_edges: base
            .edges
            .iter()
            .map(|edge| BaseEdgeContract {
                id: edge.id.clone(),
                from: edge.from,
                to: edge.to,
                world_x: edge.world_x,
                world_z: edge.world_z,
                length_m: edge.length_m,
                rotation_y_deg: edge.rotation_y_deg,
                profile: edge.profile.clone(),
            })
            .collect(),
    })
}

pub fn apply_fixed_endpoint_levels(
    style: &mut serde_json::Value,
    trusted: &TrustedBaseContract,
) -> Result<(), String> {
    let corridors = style
        .get_mut("roadExtensions")
        .and_then(|value| value.get_mut("corridors"))
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("roadExtensions.corridors is required")?;
    for corridor in corridors {
        for (point_field, level_field) in [("start", "startLevelMm"), ("end", "endLevelMm")] {
            let point = corridor
                .get(point_field)
                .and_then(serde_json::Value::as_array)
                .and_then(|values| {
                    Some([values.first()?.as_i64()? as i32, values.get(1)?.as_i64()? as i32])
                })
                .ok_or_else(|| format!("invalid corridor {point_field}"))?;
            if let Some(level) = trusted.fixed_nodes.get(&point) {
                corridor[level_field] = serde_json::json!(level);
            }
        }
    }
    Ok(())
}

pub fn audit_node_heights(
    plan: &super::road_network::RoadNetworkPlan,
    trusted: &TrustedBaseContract,
) -> Result<NodeElevationAudit, String> {
    if plan.natural_heightfield_hash != trusted.natural_hash {
        return Err("joint candidate natural source hash differs from trusted base".into());
    }
    if plan.level_m.map(|level| level * 1000) != Some(trusted.base_level_mm) {
        return Err("joint candidate base road level differs from trusted base".into());
    }
    for required in &trusted.base_edges {
        let actual = plan
            .edges
            .iter()
            .find(|edge| edge.id == required.id)
            .ok_or_else(|| format!("fixed base edge {} was deleted", required.id))?;
        if actual.from != required.from
            || actual.to != required.to
            || actual.world_x != required.world_x
            || actual.world_z != required.world_z
            || actual.length_m != required.length_m
            || actual.rotation_y_deg != required.rotation_y_deg
            || actual.profile != required.profile
        {
            return Err(format!("fixed base edge {} was changed", required.id));
        }
    }
    let mut levels = BTreeMap::<[i32; 2], i32>::new();
    let mut nodes = BTreeMap::<[i32; 2], ()>::new();
    for node in &plan.nodes {
        nodes.insert([node.world_x, node.world_z], ());
    }
    for (point, level) in &trusted.fixed_nodes {
        if !nodes.contains_key(point) {
            return Err(format!("fixed road node {point:?} is missing from candidate"));
        }
        levels.insert(*point, *level);
    }
    for extension in &plan.extensions {
        let request = &extension.design.request;
        for (point, level) in [
            (request.start, request.start_level_mm),
            (request.end, request.end_level_mm),
        ] {
            let Some(level) = level else {
                return Err("joint road candidate is missing an endpoint elevation".into());
            };
            if let Some(previous) = levels.insert(point, level) {
                if previous != level {
                    return Err(format!(
                        "shared road node {point:?} has inconsistent elevations {previous} and {level}"
                    ));
                }
            }
        }
    }
    Ok(NodeElevationAudit {
        nodes: levels
            .into_iter()
            .map(|([world_x, world_z], level_mm)| NodeElevation {
                world_x,
                world_z,
                level_mm,
            })
            .collect(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JointAudit {
    pub frontage_requested: u32,
    pub frontage_planned: usize,
    pub frontage_realized_with_access: usize,
    pub frontage_unfilled: u32,
    pub parcels: usize,
    pub buildings_realized: usize,
    pub yard_unresolved_m2: f64,
    pub road_edges: usize,
    pub road_widths: Vec<u32>,
    pub frontage_status: String,
    pub frontage_failures: Vec<String>,
    pub development_status: String,
    pub development_failures: Vec<String>,
    pub landcover_status: String,
    pub landcover_failures: Vec<String>,
    pub required_connected: usize,
    pub required_unconnected: usize,
    pub required_unconnected_ids: Vec<String>,
    pub extension_count: usize,
    pub node_elevations: Vec<NodeElevation>,
}

fn candidate_entities(plan: &super::road_network::RoadNetworkPlan) -> Vec<EntityInstance> {
    let mut entities = plan.entities();
    entities.extend(plan.site_entities());
    entities
}

fn audit_final_road_support(
    plan: &super::road_network::RoadNetworkPlan,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    zones: &[LandUseZone],
    scene: &SceneSpec,
) -> Result<(), String> {
    let mask = super::zoning::ZoningMask::new(scene, zones).map_err(|error| error.to_string())?;
    let plan_entities = plan.entities();
    let carves = super::plan_terrain_carves(&plan_entities);
    for entity in &plan_entities {
        let (hw, hd) = if entity.rotation_y_deg == 0. {
            (
                entity.width_m as i32 / 2 + 4,
                entity.depth_m as i32 / 2 + 3,
            )
        } else {
            (
                entity.depth_m as i32 / 2 + 3,
                entity.width_m as i32 / 2 + 4,
            )
        };
        for z in entity.world_z - hd..=entity.world_z + hd {
            for x in entity.world_x - hw..=entity.world_x + hw {
                let natural_height = natural
                    .height(x, z)
                    .ok_or("road support sample missing from natural terrain")?;
                let target = entity
                    .grounding
                    .road_profile
                    .as_ref()
                    .and_then(|profile| profile.height_at(f64::from(x), f64::from(z)))
                    .unwrap_or_else(|| f64::from(entity.ground_height_m()));
                let fill = target - f64::from(natural_height);
                if fill < -1e-9
                    || fill > f64::from(plan.max_roadbed_fill_m) + 1e-9
                    || fill > mask.fill_capacity_at(x, z) + 1e-9
                {
                    return Err(format!(
                        "road {} support violates natural fill envelope at {x},{z}",
                        entity.entity_id
                    ));
                }
                let expected = super::apply_terrain_carves(
                    f64::from(natural_height),
                    x,
                    z,
                    &carves,
                );
                let actual = final_terrain
                    .height(x, z)
                    .ok_or("road support sample missing from final terrain")?;
                if (f64::from(actual) - expected).abs() > 0.0005 {
                    return Err(format!(
                        "road {} final support differs from production engineering at {x},{z}: actual={actual}, expected={expected}",
                        entity.entity_id,
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Rebuild every downstream artifact from the same candidate road network and
/// audit it through the production contracts. A single design pass is not
/// sufficient: frontage, parcel competition, access graph and yard surfaces
/// are all checked here.
pub fn audit_joint_plan(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    plan: &super::road_network::RoadNetworkPlan,
    trusted: &TrustedBaseContract,
) -> Result<JointAudit, String> {
    audit_joint_plan_with_fixed_nodes(
        scene,
        natural,
        final_terrain,
        water,
        zones,
        plan,
        trusted,
    )
}

pub fn audit_joint_plan_with_fixed_nodes(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    plan: &super::road_network::RoadNetworkPlan,
    trusted: &TrustedBaseContract,
) -> Result<JointAudit, String> {
    if plan.extensions.len() > MAX_JOINT_EXTENSIONS {
        return Err("joint road candidate exceeds the bounded 256-state budget".into());
    }
    if natural
        .content_hash()
        .map_err(|error| error.to_string())?
        != trusted.natural_hash
    {
        return Err("joint candidate natural terrain differs from trusted source".into());
    }
    let node_audit = audit_node_heights(plan, trusted)?;
    for extension in &plan.extensions {
        super::road_alignment::verify(
            &extension.design,
            &extension.design.request,
            scene,
            natural,
            water,
            zones,
        )
        .map_err(|error| format!("extension {} source rebuild failed: {error}", extension.entity_id))?;
    }
    audit_final_road_support(plan, natural, final_terrain, zones, scene)?;
    let entities = candidate_entities(plan);
    let access = super::build_access_graph(&entities, final_terrain, water);
    let development_plan = development::plan(scene, zones, plan)?;
    let development_audit =
        development::verify(&development_plan, scene, zones, plan, &entities, &access)?;
    let landcover_plan = development::plan_landcover(scene, &development_plan)?;
    let surfaces = landcover::realize(
        scene,
        &development_plan,
        &landcover_plan,
        final_terrain,
        water,
        &entities,
    )?;
    let surface_audit = landcover::verify(
        &surfaces,
        scene,
        &development_plan,
        &landcover_plan,
        final_terrain,
        water,
        &entities,
    )?;
    let frontage = super::frontage::audit(plan, &entities, &access);
    let widths: Vec<u32> = plan
        .extensions
        .iter()
        .map(|extension| extension.design.request.road_width_m)
        .collect();
    let required_unconnected_ids: Vec<String> = access
        .links
        .iter()
        .filter(|link| link.required && !link.connected)
        .map(|link| link.entity_id.clone())
        .collect();
    Ok(JointAudit {
        frontage_requested: frontage.requested,
        frontage_planned: frontage.planned,
        frontage_realized_with_access: frontage.realized_with_access,
        frontage_unfilled: frontage.unfilled,
        parcels: development_plan.parcels.len(),
        buildings_realized: development_audit.realized_buildings,
        yard_unresolved_m2: surface_audit.unresolved_yard_m2,
        road_edges: plan.edges.len(),
        road_widths: widths,
        frontage_status: frontage.status.clone(),
        frontage_failures: frontage.failures.clone(),
        development_status: development_audit.status.clone(),
        development_failures: development_audit.failures.clone(),
        landcover_status: surface_audit.status.clone(),
        landcover_failures: surface_audit.failures.clone(),
        required_connected: access
            .links
            .iter()
            .filter(|link| link.required && link.connected)
            .count(),
        required_unconnected: required_unconnected_ids.len(),
        required_unconnected_ids,
        extension_count: plan.extensions.len(),
        node_elevations: node_audit.nodes,
    })
}

pub fn evaluate_style_candidate(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    style: &serde_json::Value,
    trusted: &TrustedBaseContract,
) -> Result<JointAudit, String> {
    let service_program =
        super::services::from_style(style).map_err(|error| error.to_string())?;
    let plan = super::road_network::plan_with_style(
        scene,
        natural,
        water,
        Some(zones),
        service_program.as_ref(),
        style,
    )
    .map_err(|error| error.to_string())?;
    audit_joint_plan(scene, natural, final_terrain, water, zones, &plan, trusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;

    fn fixture() -> (SceneSpec, BakedTerrain, WaterGeometry, Vec<LandUseZone>) {
        let scene = SceneSpec {
            scene_id: "b-r3-feasible-fixture".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, vec![0.0; 512 * 512]).unwrap(),
            )
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "fixture.urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "integration_fixture:flat-authored".into(),
        }];
        (scene, terrain, water, zones)
    }

    fn style() -> serde_json::Value {
        serde_json::json!({
            "frontageProgram": {
                "version": 1,
                "requests": [
                    {
                        "id": "fixture-homes",
                        "sceneId": "b-r3-feasible-fixture",
                        "purpose": "urban",
                        "kind": "residential_home",
                        "count": 16,
                        "widthM": 12,
                        "depthM": 10,
                        "heightM": 8,
                        "stationSpacingM": 20,
                        "setbackM": 8,
                        "maxCandidateAttempts": 2048,
                        "parcelInsets": {"frontM": 4, "sideM": 4, "rearM": 12}
                    },
                    {
                        "id": "fixture-shops",
                        "sceneId": "b-r3-feasible-fixture",
                        "purpose": "urban",
                        "kind": "storefront",
                        "count": 8,
                        "widthM": 12,
                        "depthM": 10,
                        "heightM": 6,
                        "stationSpacingM": 20,
                        "setbackM": 8,
                        "maxCandidateAttempts": 2048,
                        "parcelInsets": {"frontM": 4, "sideM": 4, "rearM": 12}
                    }
                ]
            }
        })
    }

    fn minimal_plan(natural_hash: &str) -> super::super::road_network::RoadNetworkPlan {
        super::super::road_network::RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "fixture".into(),
            natural_heightfield_hash: natural_hash.into(),
            scene_id: "b-r3-feasible-fixture".into(),
            status: "planned_connected_network".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            nodes: vec![
                super::super::road_network::RoadNode {
                    id: 0,
                    world_x: 32,
                    world_z: 160,
                },
                super::super::road_network::RoadNode {
                    id: 1,
                    world_x: 96,
                    world_z: 160,
                },
            ],
            edges: vec![super::super::road_network::RoadEdge {
                id: "fixture.base".into(),
                from: 0,
                to: 1,
                world_x: 64,
                world_z: 160,
                length_m: 64,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
                // B01-T2 mechanical sync: this fixture is an unprofiled legacy base edge. It stays
                // unprofiled on purpose, because the trusted-base contract path is exercised separately.
                profile: None,
            }],
            roadside_sites: vec![],
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: vec![],
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
        }
    }

    fn append_network(
        plan: &mut super::super::road_network::RoadNetworkPlan,
        scene: &SceneSpec,
        terrain: &BakedTerrain,
        water: WaterGeometry,
        zones: &[LandUseZone],
        collector_start_level_mm: i32,
        collector_end_level_mm: i32,
        local_level_mm: i32,
    ) {
        let context =
            super::super::road_alignment::AlignmentContext::new(scene, terrain, water, zones)
                .unwrap();
        for (id, start, end, width, start_level_mm, end_level_mm) in [
            (
                "fixture.collector.16",
                [96, 160],
                [126, 160],
                16,
                collector_start_level_mm,
                collector_end_level_mm,
            ),
            (
                "fixture.local.12",
                [126, 160],
                [480, 160],
                12,
                collector_end_level_mm,
                local_level_mm,
            ),
        ] {
            let request = super::super::road_alignment::CorridorRequest {
                scene_id: scene.scene_id.clone(),
                start,
                end,
                road_width_m: width,
                shoulder_width_m: 3,
                max_fill_mm: 6000,
                max_rise_mm_per_m: 80,
                start_level_mm: Some(start_level_mm),
                end_level_mm: Some(end_level_mm),
                junction_length_m: 8,
            };
            super::super::road_extensions::append_one(
                plan,
                &context,
                &request,
                id.into(),
            )
            .unwrap();
        }
        super::super::frontage::append(plan, scene, terrain, water, Some(zones), &style())
            .unwrap();
    }

    fn engineered_terrain(
        scene: &SceneSpec,
        natural: &BakedTerrain,
        entities: &[EntityInstance],
    ) -> BakedTerrain {
        let carves = super::super::plan_terrain_carves(entities);
        let mut values = Vec::with_capacity(512 * 512);
        for z in 0..512 {
            for x in 0..512 {
                let base = f64::from(natural.height(x, z).unwrap());
                values.push(super::super::apply_terrain_carves(base, x, z, &carves));
            }
        }
        let mut final_terrain = BakedTerrain::new(scene).unwrap();
        final_terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, values).unwrap(),
            )
            .unwrap();
        final_terrain
    }

    fn terrain_with_bump(
        scene: &SceneSpec,
        natural: &BakedTerrain,
        bump_x: i32,
        bump_z: i32,
        bump_m: f64,
    ) -> BakedTerrain {
        let mut values = Vec::with_capacity(512 * 512);
        for z in 0..512 {
            for x in 0..512 {
                let mut value = f64::from(natural.height(x, z).unwrap());
                if x == bump_x && z == bump_z {
                    value += bump_m;
                }
                values.push(value);
            }
        }
        let mut result = BakedTerrain::new(scene).unwrap();
        result
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, values).unwrap(),
            )
            .unwrap();
        result
    }

    fn built_artifacts() -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        TrustedBaseContract,
        super::super::road_network::RoadNetworkPlan,
        development::DevelopmentPlan,
        development::LandcoverPlan,
        landcover::SurfaceRealization,
        Vec<EntityInstance>,
    ) {
        let (scene, terrain, water, zones) = fixture();
        let natural_hash = terrain.content_hash().unwrap();
        let base = minimal_plan(&natural_hash);
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();
        let mut plan = base;
        append_network(&mut plan, &scene, &terrain, water, &zones, 0, 0, 0);
        let entities = candidate_entities(&plan);
        let development_plan = development::plan(&scene, &zones, &plan).unwrap();
        let landcover_plan = development::plan_landcover(&scene, &development_plan).unwrap();
        let surfaces = landcover::realize(
            &scene,
            &development_plan,
            &landcover_plan,
            &terrain,
            water,
            &entities,
        )
        .unwrap();
        (
            scene,
            terrain,
            water,
            zones,
            trusted,
            plan,
            development_plan,
            landcover_plan,
            surfaces,
            entities,
        )
    }

    #[test]
    fn synthetic_flat_control_has_24_parcels_two_levels_and_real_yards() {
        let (scene, terrain, water, zones) = fixture();
        let natural_hash = terrain.content_hash().unwrap();
        let base = minimal_plan(&natural_hash);
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();
        let mut plan = base;
        append_network(&mut plan, &scene, &terrain, water, &zones, 0, 0, 0);
        let audit = audit_joint_plan(
            &scene,
            &terrain,
            &terrain,
            water,
            &zones,
            &plan,
            &trusted,
        )
        .unwrap();
        assert_eq!(audit.frontage_requested, 24);
        assert_eq!(audit.frontage_planned, 24);
        assert_eq!(audit.frontage_realized_with_access, 24);
        assert_eq!(audit.frontage_unfilled, 0);
        assert_eq!(audit.parcels, 24);
        assert_eq!(audit.buildings_realized, 24);
        assert!(audit.yard_unresolved_m2 < 1e-7);
        assert!(audit.road_widths.contains(&12));
        assert!(audit.road_widths.contains(&16));
        assert_eq!(audit.required_unconnected, 0);
        assert!(audit.required_unconnected_ids.is_empty());
        assert_eq!(audit.frontage_status, "pass");
        assert_eq!(audit.extension_count, 2);
    }

    #[test]
    fn yard_entrance_and_overlap_mutations_are_rejected() {
        let (
            scene,
            terrain,
            water,
            zones,
            _trusted,
            plan,
            mut development_plan,
            landcover_plan,
            mut surfaces,
            entities,
        ) = built_artifacts();
        let access = super::super::build_access_graph(&entities, &terrain, water);

        let yard = surfaces
            .patches
            .iter()
            .position(|patch| patch.material == landcover::Material::ManagedGrass)
            .expect("yard patch");
        surfaces.patches.remove(yard);
        assert!(landcover::verify(
            &surfaces,
            &scene,
            &development_plan,
            &landcover_plan,
            &terrain,
            water,
            &entities,
        )
        .is_err());

        let mut bad_entry = development_plan.clone();
        bad_entry.buildings[0].entity.anchors[0].world_x += 1;
        assert!(development::verify(
            &bad_entry,
            &scene,
            &zones,
            &plan,
            &entities,
            &access,
        )
        .is_err());

        development_plan.parcels[1].boundary = development_plan.parcels[0].boundary;
        assert!(development::validate(&development_plan, &scene, &zones).is_err());
    }

    #[test]
    fn urban_boundary_outer_rows_are_rejected_by_geometry() {
        let (scene, _, _, _) = fixture();
        let insets = super::super::parcel_geometry::ParcelInsets {
            front_m: 4,
            side_m: 4,
            rear_m: 12,
        };
        let south_of_z224 =
            super::super::parcel_geometry::boundary(0, 243, 12, 10, 0., insets).unwrap();
        assert!(south_of_z224[3] > 256);
        let north_of_z160 =
            super::super::parcel_geometry::boundary(0, 141, 12, 10, 180., insets).unwrap();
        assert!(north_of_z160[1] < 128);
        assert_eq!(scene.width_m, 512);
    }

    #[test]
    fn shared_nodes_keep_one_elevation_and_fixed_nodes_cannot_move() {
        let (_, terrain, _, _, trusted, plan, ..) = built_artifacts();
        audit_node_heights(&plan, &trusted).unwrap();

        let mut bad_fixed = trusted.clone();
        bad_fixed.fixed_nodes.insert([96, 160], 8000);
        assert!(audit_node_heights(&plan, &bad_fixed).is_err());

        let mut missing = trusted.clone();
        missing.fixed_nodes.insert([99, 99], 0);
        assert!(audit_node_heights(&plan, &missing).is_err());

        let mut forged = trusted.clone();
        forged.natural_hash = "forged-natural".into();
        assert!(audit_node_heights(&plan, &forged).is_err());

        let mut inconsistent = plan.clone();
        inconsistent.extensions[1].design.request.start_level_mm = Some(1000);
        assert!(audit_node_heights(&inconsistent, &trusted).is_err());
    }

    #[test]
    fn fixed_roots_are_bound_by_coordinate_not_array_order() {
        let trusted = TrustedBaseContract {
            natural_hash: "fixture".into(),
            base_level_mm: 8000,
            fixed_nodes: BTreeMap::from([
                ([224, 96], 8000),
                ([288, 224], 8000),
            ]),
            base_edges: vec![],
        };
        let mut style = serde_json::json!({
            "roadExtensions": {
                "version": 1,
                "corridors": [
                    {"start":[288,224],"end":[288,192],"startLevelMm":7000,"endLevelMm":7000},
                    {"start":[224,96],"end":[224,128],"startLevelMm":7000,"endLevelMm":7000}
                ]
            }
        });
        apply_fixed_endpoint_levels(&mut style, &trusted).unwrap();
        assert_eq!(style["roadExtensions"]["corridors"][0]["startLevelMm"], 8000);
        assert_eq!(style["roadExtensions"]["corridors"][0]["endLevelMm"], 7000);
        assert_eq!(style["roadExtensions"]["corridors"][1]["startLevelMm"], 8000);
        assert_eq!(style["roadExtensions"]["corridors"][1]["endLevelMm"], 7000);
    }

    #[test]
    fn base_support_extension_design_and_base_edge_are_independently_audited() {
        let (scene, natural, water, zones, trusted, plan, ..) = built_artifacts();
        for (x, z) in [(64, 160), (65, 160), (64, 161), (96, 160), (28, 160), (100, 160)] {
            let bumped_final = terrain_with_bump(&scene, &natural, x, z, 0.001);
            let error = audit_joint_plan(
                &scene,
                &natural,
                &bumped_final,
                water,
                &zones,
                &plan,
                &trusted,
            )
            .unwrap_err();
            assert!(error.contains("support"), "{error}");
        }
        let unrelated = terrain_with_bump(&scene, &natural, 0, 0, 0.001);
        audit_joint_plan(
            &scene,
            &natural,
            &unrelated,
            water,
            &zones,
            &plan,
            &trusted,
        )
        .unwrap();

        let mut forged_design = plan.clone();
        forged_design.extensions[0].design.road_mesh.vertices[0][1] += 1.0;
        let error = audit_joint_plan(
            &scene,
            &natural,
            &natural,
            water,
            &zones,
            &forged_design,
            &trusted,
        )
        .unwrap_err();
        assert!(error.contains("source rebuild"));

        let mut deleted_base = plan.clone();
        deleted_base
            .edges
            .retain(|edge| edge.id != "fixture.base");
        let error = audit_joint_plan(
            &scene,
            &natural,
            &natural,
            water,
            &zones,
            &deleted_base,
            &trusted,
        )
        .unwrap_err();
        assert!(error.contains("fixed base edge"));
    }

    #[test]
    fn nonzero_engineering_control_keeps_natural_and_final_distinct() {
        let (scene, natural, water, zones) = fixture();
        let natural_hash = natural.content_hash().unwrap();
        let base = minimal_plan(&natural_hash);
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();
        let mut plan = base;
        append_network(
            &mut plan,
            &scene,
            &natural,
            water,
            &zones,
            0,
            100,
            100,
        );
        let entities = candidate_entities(&plan);
        let final_terrain = engineered_terrain(&scene, &natural, &entities);
        let final_hash = final_terrain.content_hash().unwrap();
        assert_ne!(natural_hash, final_hash);
        let max_difference = (0..512)
            .flat_map(|z| (0..512).map(move |x| (x, z)))
            .map(|(x, z)| {
                (f64::from(final_terrain.height(x, z).unwrap())
                    - f64::from(natural.height(x, z).unwrap()))
                .abs()
            })
            .fold(0.0_f64, f64::max);
        assert!(max_difference >= 0.099);
        let audit = audit_joint_plan(
            &scene,
            &natural,
            &final_terrain,
            water,
            &zones,
            &plan,
            &trusted,
        )
        .unwrap();
        assert_eq!(audit.frontage_realized_with_access, 24);
        assert_eq!(audit.parcels, 24);
        assert!(audit.yard_unresolved_m2 < 1e-7);
    }

    #[test]
    #[ignore = "writes and re-reads persisted R4 integration fixture artifacts"]
    fn persisted_positive_reloads_and_reaudits() {
        use std::{fs::{self, OpenOptions}, io::Write as _, path::PathBuf};

        let (scene, natural, water, zones) = fixture();
        let natural_hash = natural.content_hash().unwrap();
        let base = minimal_plan(&natural_hash);
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();
        let mut plan = base.clone();
        append_network(&mut plan, &scene, &natural, water, &zones, 0, 100, 100);
        let entities = candidate_entities(&plan);
        let final_terrain = engineered_terrain(&scene, &natural, &entities);
        let development_plan = development::plan(&scene, &zones, &plan).unwrap();
        let landcover_plan = development::plan_landcover(&scene, &development_plan).unwrap();
        let surfaces = landcover::realize(
            &scene,
            &development_plan,
            &landcover_plan,
            &final_terrain,
            water,
            &entities,
        )
        .unwrap();

        let root = std::env::var_os("GLYPHWEAVE_R6_PERSISTED_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../bevy/work/collaboration/ds-b-r6/persisted-nonzero-r6")
            });
        fs::create_dir_all(&root).unwrap();
        let write_new = |name: &str, bytes: &[u8]| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(name))
                .unwrap();
            file.write_all(bytes).unwrap();
        };
        let terrain_bytes = |terrain: &BakedTerrain| {
            encode_millimetres(
                512,
                512,
                (0..512)
                    .flat_map(|z| (0..512).map(move |x| f64::from(terrain.height(x, z).unwrap())))
                    .collect::<Vec<f64>>(),
            )
            .unwrap()
        };
        write_new("scene.json", &serde_json::to_vec_pretty(&scene).unwrap());
        write_new("zones.json", &serde_json::to_vec_pretty(&zones).unwrap());
        write_new("water.json", br#"{"kind":"none"}"#);
        write_new("base-plan.json", &serde_json::to_vec_pretty(&base).unwrap());
        write_new(
            "candidate-plan.json",
            &serde_json::to_vec_pretty(&plan).unwrap(),
        );
        write_new("natural.height.bin", &terrain_bytes(&natural));
        write_new("final.height.bin", &terrain_bytes(&final_terrain));
        write_new(
            "development-plan.json",
            &serde_json::to_vec_pretty(&development_plan).unwrap(),
        );
        write_new(
            "landcover-plan.json",
            &serde_json::to_vec_pretty(&landcover_plan).unwrap(),
        );
        write_new(
            "landcover-surfaces.json",
            &serde_json::to_vec_pretty(&surfaces).unwrap(),
        );
        write_new("entities.json", &serde_json::to_vec_pretty(&entities).unwrap());

        let scene: SceneSpec = serde_json::from_slice(&fs::read(root.join("scene.json")).unwrap()).unwrap();
        let zones: Vec<LandUseZone> =
            serde_json::from_slice(&fs::read(root.join("zones.json")).unwrap()).unwrap();
        let base: super::super::road_network::RoadNetworkPlan =
            serde_json::from_slice(&fs::read(root.join("base-plan.json")).unwrap()).unwrap();
        let plan: super::super::road_network::RoadNetworkPlan =
            serde_json::from_slice(&fs::read(root.join("candidate-plan.json")).unwrap()).unwrap();
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(0, 0, 512, 512, &fs::read(root.join("natural.height.bin")).unwrap())
            .unwrap();
        let mut final_terrain = BakedTerrain::new(&scene).unwrap();
        final_terrain
            .insert_chunk(0, 0, 512, 512, &fs::read(root.join("final.height.bin")).unwrap())
            .unwrap();
        let development_plan: development::DevelopmentPlan =
            serde_json::from_slice(&fs::read(root.join("development-plan.json")).unwrap()).unwrap();
        let landcover_plan: development::LandcoverPlan =
            serde_json::from_slice(&fs::read(root.join("landcover-plan.json")).unwrap()).unwrap();
        let surfaces: landcover::SurfaceRealization =
            serde_json::from_slice(&fs::read(root.join("landcover-surfaces.json")).unwrap()).unwrap();
        let entities: Vec<EntityInstance> =
            serde_json::from_slice(&fs::read(root.join("entities.json")).unwrap()).unwrap();
        let natural_hash = natural.content_hash().unwrap();
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );

        let audit = audit_joint_plan(
            &scene,
            &natural,
            &final_terrain,
            water,
            &zones,
            &plan,
            &trusted,
        )
        .unwrap();
        assert_eq!(audit.frontage_realized_with_access, 24);
        assert!(audit.yard_unresolved_m2 < 1e-7);
        landcover::verify(
            &surfaces,
            &scene,
            &development_plan,
            &landcover_plan,
            &final_terrain,
            water,
            &entities,
        )
        .unwrap();

        let mut broken = surfaces.clone();
        broken.patches.remove(0);
        assert!(landcover::verify(
            &broken,
            &scene,
            &development_plan,
            &landcover_plan,
            &final_terrain,
            water,
            &entities,
        )
        .is_err());

        let bumped = terrain_with_bump(&scene, &final_terrain, 64, 160, 0.001);
        assert!(audit_joint_plan(
            &scene,
            &natural,
            &bumped,
            water,
            &zones,
            &plan,
            &trusted,
        )
        .is_err());

        let mut wrong_base = base.clone();
        wrong_base.natural_heightfield_hash = "forged-natural".into();
        assert!(trusted_base_contract(&wrong_base, &natural_hash).is_err());

        let mut deleted_base = plan.clone();
        deleted_base.edges.retain(|edge| edge.id != "fixture.base");
        assert!(audit_joint_plan(
            &scene,
            &natural,
            &final_terrain,
            water,
            &zones,
            &deleted_base,
            &trusted,
        )
        .is_err());
    }

    #[test]
    #[ignore = "runs the bounded R4 evaluation against the existing frozen natural world"]
    fn original_natural_bounded_joint_evaluation() {
        use std::{fs, path::PathBuf};

        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world_root = repo.join("bevy/work/collaboration/ds-b-run/b01/control-e2e-1");
        let manifest_path = repo.join("examples/b-r1-control.manifest.json");
        let manifest: super::super::WorldManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let scene = manifest.scenes[0].clone();
        let scene_json: serde_json::Value =
            serde_json::from_slice(&fs::read(world_root.join("scenes/scene-0/scene.json")).unwrap())
                .unwrap();
        let mut natural = BakedTerrain::new(&scene).unwrap();
        for chunk in scene_json["chunks"].as_array().unwrap() {
            let file = chunk["naturalHeightFile"].as_str().unwrap();
            natural
                .insert_chunk(
                    chunk["worldX"].as_i64().unwrap() as i32,
                    chunk["worldZ"].as_i64().unwrap() as i32,
                    chunk["validWidthM"].as_u64().unwrap() as u32,
                    chunk["validDepthM"].as_u64().unwrap() as u32,
                    &fs::read(world_root.join("scenes/scene-0").join(file)).unwrap(),
                )
                .unwrap();
        }
        let natural_hash = natural.content_hash().unwrap();
        let water = super::super::water_geometry(
            super::super::water_kind(&manifest.style),
            &manifest.landmarks,
            &scene,
            &manifest.style,
        );
        let profile = super::super::LandUseProfile::from_style(&manifest.style).unwrap();
        let zones = super::super::plan_landuse_zones(&scene, &profile);
        let base = super::super::road_network::plan_with_zones(
            &scene,
            &natural,
            water,
            Some(&zones),
        )
        .unwrap();
        let trusted = trusted_base_contract(&base, &natural_hash).unwrap();

        let mut states = Vec::new();
        let mut best: Option<serde_json::Value> = None;
        let mut attempted_states = 0_usize;
        for level in [8000_i32, 7000, 6000, 5000] {
            attempted_states += 1;
        let mut style = manifest.style.clone();
            if let Some(corridors) = style["roadExtensions"]["corridors"].as_array_mut() {
                for corridor in corridors {
                    corridor["startLevelMm"] = serde_json::json!(level);
                    corridor["endLevelMm"] = serde_json::json!(level);
                }
            }
            apply_fixed_endpoint_levels(&mut style, &trusted).unwrap();
            let service_program = match super::super::services::from_style(&style) {
                Ok(program) => program,
                Err(error) => {
                    states.push(serde_json::json!({
                        "stateId": format!("level-{level}"),
                        "levelMm": level,
                        "fixedEntryMm": 8000,
                        "extensionCount": serde_json::Value::Null,
                        "status": "service_program_rejected",
                        "reason": error.to_string(),
                    }));
                    continue;
                }
            };
            let result = super::super::road_network::plan_with_style(
                &scene,
                &natural,
                water,
                Some(&zones),
                service_program.as_ref(),
                &style,
            );
            match result {
                Ok(plan) => {
                    let extension_count = plan.extensions.len();
                    let entities = candidate_entities(&plan);
                    let final_terrain = engineered_terrain(&scene, &natural, &entities);
                    let audit = audit_joint_plan(
                        &scene,
                        &natural,
                        &final_terrain,
                        water,
                        &zones,
                        &plan,
                        &trusted,
                    );
                    match audit {
                        Ok(audit) => {
                            let legal = audit.frontage_status == "pass"
                                && audit.frontage_planned == 24
                                && audit.frontage_unfilled == 0
                                && audit.required_unconnected == 0
                                && audit.yard_unresolved_m2 < 1e-7
                                && audit.landcover_failures.is_empty();
                            let value = serde_json::json!({
                                "stateId": format!("level-{level}"),
                                "levelMm": level,
                                "fixedEntryMm": 8000,
                                "extensionCount": extension_count,
                                "status": if legal { "legal-candidate" } else { "evaluated-partial" },
                                "frontagePlanned": audit.frontage_planned,
                                "frontageUnfilled": audit.frontage_unfilled,
                                "parcels": audit.parcels,
                                "requiredUnconnected": audit.required_unconnected,
                                "yardUnresolvedM2": audit.yard_unresolved_m2,
                            });
                            if legal {
                                best = Some(value.clone());
                            }
                            states.push(value);
                        }
                        Err(error) => states.push(serde_json::json!({
                            "stateId": format!("level-{level}"),
                            "levelMm": level,
                            "extensionCount": extension_count,
                            "status": "audit_rejected",
                            "reason": error,
                        })),
                    }
                }
                Err(error) => states.push(serde_json::json!({
                    "stateId": format!("level-{level}"),
                    "levelMm": level,
                    "fixedEntryMm": 8000,
                    "extensionCount": serde_json::Value::Null,
                    "status": "plan_rejected",
                    "reason": error.to_string(),
                })),
            }
        }
        let status = if best.is_some() {
            "legal-candidate"
        } else {
            "unresolved"
        };
        let result = serde_json::json!({
            "format": "glyphweave.natural-joint-evaluation",
            "version": 1,
            "attemptedStates": attempted_states,
            "maxStates": MAX_JOINT_CANDIDATE_STATES,
            "sourceNaturalHash": natural_hash,
            "bestLegalCandidate": best,
            "states": states,
            "status": status,
        });
        // B01-T2 R3 V4-2: never write into a historical evidence directory during
        // an offline re-run. The historical path stays the default only for a
        // deliberate historical write; an offline replay points this at a
        // throwaway location instead.
        let out = std::env::var_os("GLYPHWEAVE_R6_JOINT_EVAL_OUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                repo.join("bevy/work/collaboration/ds-b-r6/natural-joint-evaluation-r6.json")
            });
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(&out, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        assert_eq!(attempted_states, 4);
        assert_eq!(result["status"], "unresolved");
    }

    #[test]
    fn fixed_node_height_rejects_a_design_only_lower_endpoint() {
        let scene = SceneSpec {
            scene_id: "height-fixture".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, vec![7.0; 512 * 512]).unwrap(),
            )
            .unwrap();
        let zones = vec![LandUseZone {
            id: "height.urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "fixture".into(),
        }];
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let natural_hash = terrain.content_hash().unwrap();
        let mut plan = minimal_plan(&natural_hash);
        plan.level_m = Some(8);
        let context =
            super::super::road_alignment::AlignmentContext::new(&scene, &terrain, water, &zones)
                .unwrap();
        let request = super::super::road_alignment::CorridorRequest {
            scene_id: scene.scene_id.clone(),
            start: [96, 160],
            end: [126, 160],
            road_width_m: 16,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(7000),
            end_level_mm: Some(7000),
            junction_length_m: 8,
        };
        assert!(super::super::road_alignment::design(
            &request,
            &scene,
            &terrain,
            water,
            &zones
        )
        .is_ok());
        let error = super::super::road_extensions::append_one(
            &mut plan,
            &context,
            &request,
            "fixture.reject".into(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("height mismatch") || error.contains("junction"));
    }
}
