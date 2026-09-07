//! Bounded fill-only, level lowland road network. This deliberately does not
//! claim to model mountain passes, bridges or an optimized highway alignment.
use super::LandUseZone;
use super::zoning::{ZoningMask, ZoningRejection};
use super::{EntityInstance, GroundingSpec, SceneSpec, SpatialAnchor, WaterGeometry};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNetworkPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage: Option<super::frontage::Report>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_extensions: Option<super::auto_roads::Report>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<super::road_extensions::RoadExtension>,
    pub model: String,
    pub natural_heightfield_hash: String,
    pub scene_id: String,
    pub status: String,
    pub grid_spacing_m: u32,
    pub max_roadbed_fill_m: f32,
    pub level_m: Option<i32>,
    pub candidates: usize,
    pub rejected_water: usize,
    pub rejected_relief: usize,
    pub unselected_candidates: usize,
    pub nodes: Vec<RoadNode>,
    pub edges: Vec<RoadEdge>,
    pub roadside_sites: Vec<RoadsideSite>,
    #[serde(default)]
    pub zoning_hash: Option<String>,
    #[serde(default)]
    pub zoning_rejections: Vec<ZoningRejection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_outcomes: Vec<super::services::ServiceOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint_layout: Option<super::joint_frontage::JointLayout>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNode {
    pub id: usize,
    pub world_x: i32,
    pub world_z: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadEdge {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub world_x: i32,
    pub world_z: i32,
    pub length_m: u32,
    pub rotation_y_deg: f32,
    pub natural_min_m: f32,
    pub natural_max_m: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadsideSite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<super::services::SiteProgram>,
    pub id: String,
    pub road_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub support_level_m: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_level_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage_road_point: Option<[i32;2]>,
    pub rotation_y_deg: f32,
    pub natural_min_m: f32,
    pub natural_max_m: f32,
}
impl RoadsideSite {
    pub(super) fn entity(&self) -> EntityInstance {
        let s = self;
        EntityInstance {
                entity_id: s.id.clone(),
                asset_id: format!(
                    "prop.{}",
                    s.program
                        .as_ref()
                        .map_or("residential_home", |p| p.kind.as_str())
                ),
                kind: s
                    .program
                    .as_ref()
                    .map_or("residential_home", |p| p.kind.as_str())
                    .into(),
                world_x: s.world_x,
                world_z: s.world_z,
                world_y: s.support_level_mm.map_or(s.support_level_m, |mm| mm.div_euclid(1000)),
                width_m: s.width_m as f32,
                depth_m: s.depth_m as f32,
                height_m: s.program.as_ref().map_or(8., |p| p.height_m as f32),
                scale: 1.,
                rotation_y_deg: s.rotation_y_deg,
                grounding: GroundingSpec {
                    frontage_road_point: s.frontage_road_point,
                    bottom_offset_m: s.support_level_mm.map_or(0., |mm| mm.rem_euclid(1000) as f32 / 1000.),
                    ..Default::default()
                },
                anchors: vec![s.front_anchor()],
                bounds: None,
            }
    }
    pub fn world_half_extents(&self) -> (u32, u32) {
        if matches!(self.rotation_y_deg as i32, 90 | 270) {
            (self.depth_m / 2, self.width_m / 2)
        } else {
            (self.width_m / 2, self.depth_m / 2)
        }
    }
    pub(super) fn front_anchor(&self) -> SpatialAnchor {
        // Cardinal frontage is calculated in integer metres. Do not round a
        // large world coordinate through f32 to place an entrance.
        let front = self.depth_m as i32 / 2 + 1;
        let (dx, dz, direction) = match self.rotation_y_deg as i32 {
            90 => (-front, 0, "west"),
            180 => (0, front, "south"),
            270 => (front, 0, "east"),
            _ => (0, -front, "north"),
        };
        SpatialAnchor {
            id: "front".into(),
            world_x: self.world_x + dx,
            world_z: self.world_z + dz,
            direction: direction.into(),
            target: Some("road".into()),
        }
    }
}
impl RoadNetworkPlan {
    pub fn site_entities(&self) -> Vec<EntityInstance> {
        self.roadside_sites
            .iter()
            .map(RoadsideSite::entity)
            .collect()
    }
    pub fn entities(&self) -> Vec<EntityInstance> {
        let Some(level) = self.level_m else {
            return Vec::new();
        };
        self.edges
            .iter()
            .map(|e| {
                let profile = self
                    .extensions
                    .iter()
                    .find(|p| p.entity_id == e.id)
                    .map(|p| super::road_extensions::profile(&p.design));
                let height = profile
                    .as_ref()
                    .and_then(|p| p.height_at(f64::from(e.world_x), f64::from(e.world_z)))
                    .unwrap_or(f64::from(level));
                EntityInstance {
                    entity_id: e.id.clone(),
                    asset_id: "prop.road".into(),
                    kind: "road".into(),
                    world_x: e.world_x,
                    world_z: e.world_z,
                    world_y: height.floor() as i32,
                    width_m: e.length_m as f32,
                    depth_m: 8.,
                    height_m: 0.24,
                    rotation_y_deg: e.rotation_y_deg,
                    scale: 1.,
                    grounding: GroundingSpec {
                        road_profile: profile,
                        bottom_offset_m: (height - height.floor()) as f32,
                        ..Default::default()
                    },
                    anchors: Vec::new(),
                    bounds: None,
                }
            })
            .collect()
    }
}

pub fn enabled(style: &serde_json::Value, water: WaterGeometry) -> bool {
    matches!(
        style
            .get("generationProfile")
            .and_then(serde_json::Value::as_str),
        Some("strict" | "release")
    ) && water.kind == super::WaterKind::None
        && style.get("landUseProfile").is_some()
        && style
            .get("naturalOnly")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        && !matches!(
            super::generation_settlement(style),
            Some("wilderness" | "pastoral")
        )
}

#[cfg(test)]
pub(super) fn plan(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> Result<RoadNetworkPlan, TerrainError> {
    plan_with_zones(scene, terrain, water, None)
}
pub(super) fn plan_with_zones(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
) -> Result<RoadNetworkPlan, TerrainError> {
    let zoning = zones.map(|z| ZoningMask::new(scene, z)).transpose()?;
    terrain.require_complete()?;
    if !terrain.matches_scene(scene) {
        return Err(TerrainError("road plan terrain mismatch".into()));
    }
    let step = 64;
    let max_fill = 6.;
    let mut result = RoadNetworkPlan {
        automatic_extensions: None,
            frontage: None,
        extensions: vec![],
        model: if zoning.is_some() {
            "zoned-fill-envelope-lowland-frontage-v4"
        } else {
            "connected-level-lowland-fill-frontage-v2"
        }
        .into(),
        natural_heightfield_hash: terrain.content_hash()?,
        scene_id: scene.scene_id.clone(),
        status: "no_feasible_network".into(),
        grid_spacing_m: step,
        max_roadbed_fill_m: max_fill,
        level_m: None,
        candidates: 0,
        rejected_water: 0,
        rejected_relief: 0,
        unselected_candidates: 0,
        nodes: Vec::new(),
        edges: Vec::new(),
        roadside_sites: Vec::new(),
        zoning_hash: zoning.as_ref().map(|z| z.hash.clone()),
        zoning_rejections: Vec::new(),
        service_outcomes: Vec::new(),
        joint_layout: None,
    };
    let xs: Vec<i32> = (32..scene.width_m.saturating_sub(32))
        .step_by(step as usize)
        .map(|x| scene.origin_x + x as i32)
        .collect();
    let zs: Vec<i32> = (32..scene.depth_m.saturating_sub(32))
        .step_by(step as usize)
        .map(|z| scene.origin_z + z as i32)
        .collect();
    let mut all_nodes = Vec::new();
    for &z in &zs {
        for &x in &xs {
            all_nodes.push(RoadNode {
                id: all_nodes.len(),
                world_x: x,
                world_z: z,
            });
        }
    }
    let mut edges = Vec::new();
    let mut level_caps = Vec::new();
    for row in 0..zs.len() {
        for col in 0..xs.len() {
            let from = row * xs.len() + col;
            for (exists, to, rot) in [
                (col + 1 < xs.len(), from + 1, 0.),
                (row + 1 < zs.len(), from + xs.len(), 90.),
            ] {
                if !exists {
                    continue;
                }
                result.candidates += 1;
                let a = &all_nodes[from];
                let b = &all_nodes[to];
                let x = ((i64::from(a.world_x) + i64::from(b.world_x)) / 2) as i32;
                let z = ((i64::from(a.world_z) + i64::from(b.world_z)) / 2) as i32;
                let (hw, hd) = if rot == 0. {
                    (step as i32 / 2 + 4, 7)
                } else {
                    (7, step as i32 / 2 + 4)
                };
                if let Some(reject) = zoning.as_ref().and_then(|mask| {
                    mask.check(
                        format!("generated.road-network.{}.{}-{}", scene.scene_id, from, to),
                        "road",
                        x,
                        z,
                        hw,
                        hd,
                    )
                }) {
                    result.zoning_rejections.push(reject);
                    continue;
                }
                if water.intersects_axis_aligned(
                    f64::from(x),
                    f64::from(z),
                    f64::from(hw),
                    f64::from(hd),
                ) {
                    result.rejected_water += 1;
                    continue;
                }
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                let mut protected_cap = f64::INFINITY;
                // Full authoritative raster, not nine probes that miss ridges.
                for zz in z - hd..=z + hd {
                    for xx in x - hw..=x + hw {
                        let h = terrain.height(xx, zz).ok_or_else(|| {
                            TerrainError("road footprint missing natural sample".into())
                        })?;
                        min = min.min(h);
                        max = max.max(h);
                        if let Some(mask) = &zoning {
                            protected_cap =
                                protected_cap.min(f64::from(h) + mask.fill_capacity_at(xx, zz));
                        }
                    }
                }
                if max.ceil() - min > max_fill || f64::from(max.ceil()) > protected_cap {
                    result.rejected_relief += 1;
                    continue;
                }
                level_caps.push(protected_cap);
                edges.push(RoadEdge {
                    id: format!("generated.road-network.{}.{}-{}", scene.scene_id, from, to),
                    from,
                    to,
                    world_x: x,
                    world_z: z,
                    length_m: step,
                    rotation_y_deg: rot,
                    natural_min_m: min,
                    natural_max_m: max,
                });
            }
        }
    }
    let levels: BTreeSet<i32> = edges
        .iter()
        .map(|e| e.natural_max_m.ceil() as i32)
        .collect();
    let mut best: Vec<usize> = Vec::new();
    let mut best_distance = f64::INFINITY;
    for level in levels {
        let mut adjacency = vec![Vec::new(); all_nodes.len()];
        for (i, e) in edges.iter().enumerate() {
            if level as f32 >= e.natural_max_m
                && level as f32 - e.natural_min_m <= max_fill
                && f64::from(level) <= level_caps[i]
            {
                adjacency[e.from].push((e.to, i));
                adjacency[e.to].push((e.from, i));
            }
        }
        let mut seen = vec![false; all_nodes.len()];
        for start in 0..all_nodes.len() {
            if seen[start] || adjacency[start].is_empty() {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            seen[start] = true;
            let mut component = BTreeSet::new();
            let mut distance = f64::INFINITY;
            while let Some(n) = queue.pop_front() {
                let node = &all_nodes[n];
                distance = distance.min(
                    (f64::from(node.world_x)
                        - f64::from(scene.origin_x)
                        - f64::from(scene.width_m) * 0.5)
                        .powi(2)
                        + (f64::from(node.world_z)
                            - f64::from(scene.origin_z)
                            - f64::from(scene.depth_m) * 0.5)
                            .powi(2),
                );
                for &(neighbor, edge) in &adjacency[n] {
                    component.insert(edge);
                    if !seen[neighbor] {
                        seen[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
            if component.len() > best.len()
                || (component.len() == best.len() && distance < best_distance)
            {
                best = component.into_iter().collect();
                best_distance = distance;
                result.level_m = Some(level);
            }
        }
    }
    result.unselected_candidates = edges.len() - best.len();
    let mut used = BTreeMap::new();
    for i in best {
        let e = edges[i].clone();
        used.insert(e.from, all_nodes[e.from].clone());
        used.insert(e.to, all_nodes[e.to].clone());
        result.edges.push(e);
    }
    result.nodes = used.into_values().collect();
    if !result.edges.is_empty() {
        result.status = "planned_connected_network".into();
    }

    if let Some(level) = result.level_m {
        for e in &result.edges {
            for side in [-1, 1] {
                let (x, z) = if e.rotation_y_deg == 0. {
                    (i64::from(e.world_x), i64::from(e.world_z) + 24 * side)
                } else {
                    (i64::from(e.world_x) + 24 * side, i64::from(e.world_z))
                };
                if x - 14 < i64::from(scene.origin_x)
                    || z - 14 < i64::from(scene.origin_z)
                    || x + 14 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
                    || z + 14 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
                {
                    continue;
                }
                let (x, z) = (x as i32, z as i32); // Bounds checked in i64 above.
                if let Some(reject) = zoning.as_ref().and_then(|mask| {
                    mask.check(
                        format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        "residential_home",
                        x,
                        z,
                        14,
                        14,
                    )
                }) {
                    result.zoning_rejections.push(reject);
                    continue;
                }
                if water.intersects_axis_aligned(f64::from(x), f64::from(z), 14., 14.) {
                    continue;
                }
                if result.edges.iter().any(|r| {
                    let (hw, hd) = if r.rotation_y_deg == 0. {
                        (r.length_m as i32 / 2 + 3, 7)
                    } else {
                        (7, r.length_m as i32 / 2 + 3)
                    };
                    (r.world_x - x).abs() < hw + 14 && (r.world_z - z).abs() < hd + 14
                }) {
                    continue;
                }
                if result
                    .roadside_sites
                    .iter()
                    .any(|s| (s.world_x - x).abs() < 28 && (s.world_z - z).abs() < 28)
                {
                    continue;
                }
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                let mut protected_cap = f64::INFINITY;
                for zz in z - 14..=z + 14 {
                    for xx in x - 14..=x + 14 {
                        let h = terrain.height(xx, zz).ok_or_else(|| {
                            TerrainError("missing roadside natural sample".into())
                        })?;
                        min = min.min(h);
                        max = max.max(h);
                        if let Some(mask) = &zoning {
                            protected_cap =
                                protected_cap.min(f64::from(h) + mask.fill_capacity_at(xx, zz));
                        }
                    }
                }
                if max > level as f32
                    || level as f32 - min > max_fill
                    || f64::from(level) > protected_cap
                {
                    continue;
                }
                result.roadside_sites.push(RoadsideSite {
                    program: None,
                    id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                    road_id: e.id.clone(),
                    world_x: x,
                    world_z: z,
                    width_m: 24,
                    depth_m: 24,
                    support_level_m: level,
                    support_level_mm: None,
                    frontage_road_point: None,
                    rotation_y_deg: if e.rotation_y_deg == 0. {
                        if side < 0 { 180. } else { 0. }
                    } else if side < 0 {
                        270.
                    } else {
                        90.
                    },
                    natural_min_m: min,
                    natural_max_m: max,
                });
            }
        }
    }
    Ok(result)
}

pub(super) fn plan_with_program(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
) -> Result<RoadNetworkPlan, TerrainError> {
    let mut result = plan_with_zones(scene, terrain, water, zones)?;
    let Some(program) = program else {
        return Ok(result);
    };
    let zones = zones.ok_or_else(|| TerrainError("service program requires zoning".into()))?;
    if program.version == 2 {
        return super::joint_frontage::plan(result, scene, terrain, water, zones, program);
    }
    super::services::append(&mut result, scene, terrain, water, zones, program)?;
    Ok(result)
}

pub(super) fn plan_with_style(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkPlan, TerrainError> {
    let mut result = plan_with_program(scene, terrain, water, zones, program)?;
    if super::road_extensions::from_style(style)?.is_some() {
        super::road_extensions::append(
            &mut result,
            scene,
            terrain,
            water,
            zones.ok_or_else(|| TerrainError("extensions require zones".into()))?,
            style,
        )?;
    }
    if super::auto_roads::from_style(style)?.is_some() {
        super::auto_roads::append(
            &mut result,
            scene,
            terrain,
            water,
            zones.ok_or_else(|| TerrainError("automatic roads require zones".into()))?,
            style,
        )?;
    }
    super::frontage::append(&mut result, scene, terrain, water, zones, style)?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (SceneSpec, BakedTerrain, WaterGeometry) {
        let scene = SceneSpec {
            scene_id: "roads".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: -256,
            origin_z: -256,
            seed_offset: 0,
        };
        let mut raster = BakedTerrain::new(&scene).unwrap();
        raster
            .insert_chunk(-256, -256, 512, 512, &0_i16.to_le_bytes().repeat(512 * 512))
            .unwrap();
        let water =
            super::super::legacy_water_geometry(super::super::WaterKind::None, 512, 512, 0.);
        (scene, raster, water)
    }
    #[test]
    fn flat_network_is_connected_nonempty_and_deterministic() {
        let (s, t, w) = fixture();
        let a = plan(&s, &t, w).unwrap();
        let b = plan(&s, &t, w).unwrap();
        assert!(!a.edges.is_empty());
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
        let mut visited = BTreeSet::from([a.nodes[0].id]);
        loop {
            let n = visited.len();
            for e in &a.edges {
                if visited.contains(&e.from) || visited.contains(&e.to) {
                    visited.insert(e.from);
                    visited.insert(e.to);
                }
            }
            if n == visited.len() {
                break;
            }
        }
        assert_eq!(visited.len(), a.nodes.len());
        assert_eq!(a.level_m, Some(0));
    }

    #[test]
    fn final_audit_detects_deleted_road_and_forged_plan() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        let mut entities = planned.site_entities();
        entities.extend(planned.entities());
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "pass"
        );
        entities.pop();
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "fail"
        );
        let mut forged = planned.clone();
        forged.edges.pop();
        assert!(verify(&forged, &scene, &natural, &natural, water, &entities).is_err());
    }
    #[test]
    fn roadside_audit_rejects_missing_duplicate_moved_and_lifted_sites() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        assert!(!planned.roadside_sites.is_empty());
        let mut entities = planned.entities();
        entities.extend(planned.site_entities());
        let valid = verify(&planned, &scene, &natural, &natural, water, &entities).unwrap();
        assert_eq!(valid.status, "pass");
        assert_eq!(valid.realized_sites, planned.roadside_sites.len());
        let site_index = planned.edges.len();
        let mut missing = entities.clone();
        missing.remove(site_index);
        assert!(
            verify(&planned, &scene, &natural, &natural, water, &missing)
                .unwrap()
                .failures
                .iter()
                .any(|f| f.contains("missing planned roadside site"))
        );
        let mut duplicate = entities.clone();
        duplicate.push(entities[site_index].clone());
        assert!(
            verify(&planned, &scene, &natural, &natural, water, &duplicate)
                .unwrap()
                .failures
                .iter()
                .any(|f| f.contains("duplicate roadside site"))
        );
        let mut moved = entities.clone();
        moved[site_index].world_x += 1;
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &moved)
                .unwrap()
                .status,
            "fail"
        );
        let site = &planned.roadside_sites[0];
        let mut bytes = 0_i16.to_le_bytes().repeat(512 * 512);
        let offset =
            (((site.world_z - scene.origin_z) * 512 + site.world_x - scene.origin_x) * 2) as usize;
        bytes[offset..offset + 2].copy_from_slice(&4_i16.to_le_bytes());
        let mut lifted = BakedTerrain::new(&scene).unwrap();
        lifted
            .insert_chunk(scene.origin_x, scene.origin_z, 512, 512, &bytes)
            .unwrap();
        let audit = verify(&planned, &scene, &natural, &lifted, water, &entities).unwrap();
        assert_eq!(audit.status, "fail");
        assert_eq!(audit.realized_edges, planned.edges.len());
        assert_eq!(audit.max_site_design_height_error_m, 1.);
    }

    #[test]
    fn roadside_front_doors_face_the_assigned_road() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        for (site, entity) in planned.roadside_sites.iter().zip(planned.site_entities()) {
            let road = planned.edges.iter().find(|e| e.id == site.road_id).unwrap();
            let a = &entity.anchors[0];
            let original = (i64::from(entity.world_x) - i64::from(road.world_x)).abs()
                + (i64::from(entity.world_z) - i64::from(road.world_z)).abs();
            let door = (i64::from(a.world_x) - i64::from(road.world_x)).abs()
                + (i64::from(a.world_z) - i64::from(road.world_z)).abs();
            assert_eq!(original, 24);
            assert_eq!(door, 11);
            assert_eq!(a.target.as_deref(), Some("road"));
        }
        let mut entities = planned.entities();
        entities.extend(planned.site_entities());
        entities[planned.edges.len()].anchors[0].world_x += 1;
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "fail"
        );
    }

    #[test]
    fn roadside_bounds_are_checked_without_i32_overflow() {
        let (mut scene, _, water) = fixture();
        for origin in [i32::MIN, i32::MAX - 512] {
            scene.origin_x = origin;
            scene.origin_z = origin;
            let mut raster = BakedTerrain::new(&scene).unwrap();
            raster
                .insert_chunk(
                    origin,
                    origin,
                    512,
                    512,
                    &0_i16.to_le_bytes().repeat(512 * 512),
                )
                .unwrap();
            let planned = plan(&scene, &raster, water).unwrap();
            assert!(!planned.edges.is_empty());
            assert!(!planned.roadside_sites.is_empty());
            for site in &planned.roadside_sites {
                assert!(i64::from(site.world_x) - 14 >= i64::from(origin));
                assert!(i64::from(site.world_z) + 14 < i64::from(origin) + 512);
            }
        }
    }

    #[test]
    fn full_raster_peak_is_rejected_and_water_blocks_network() {
        let (scene, _, water) = fixture();
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let mut bytes = 0_i16.to_le_bytes().repeat(512 * 512);
        let idx = (32 * 512 + 48) * 2;
        bytes[idx..idx + 2].copy_from_slice(&160_i16.to_le_bytes());
        natural.insert_chunk(-256, -256, 512, 512, &bytes).unwrap();
        let planned = plan(&scene, &natural, water).unwrap();
        assert!(planned.rejected_relief > 0);
        assert!(!planned.edges.iter().any(|e| e.from == 0 && e.to == 1));
        let wet = WaterGeometry {
            kind: super::super::WaterKind::Lake,
            center_x: 0.,
            center_z: 0.,
            half_width: 2000.,
            half_depth: 2000.,
            scene_width_m: 512,
            scene_depth_m: 512,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
        };
        let planned = plan(&scene, &natural, wet).unwrap();
        assert!(planned.edges.is_empty());
        assert_eq!(planned.rejected_water, planned.candidates);
    }
    #[test]
    fn zoned_planning_rejects_reserved_candidates_and_rebuilds_their_evidence() {
        let (scene, natural, water) = fixture();
        let zones = vec![
            LandUseZone {
                id: "buildable".into(),
                kind: "rural".into(),
                world_x: -256,
                world_z: -256,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "protected".into(),
                kind: "nature".into(),
                world_x: 0,
                world_z: -256,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
        ];
        let p = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        assert!(!p.edges.is_empty());
        assert!(!p.roadside_sites.is_empty());
        assert!(p.zoning_hash.is_some());
        assert!(!p.zoning_rejections.is_empty());
        assert!(p.roadside_sites.iter().all(|s| s.world_x + 14 < 0));
        let mut entities = p.entities();
        entities.extend(p.site_entities());
        assert_eq!(
            verify_with_zones(
                &p,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&zones)
            )
            .unwrap()
            .status,
            "pass"
        );
        assert!(verify(&p, &scene, &natural, &natural, water, &entities).is_err());
        let mut forged = p.clone();
        forged.zoning_rejections.clear();
        assert!(
            verify_with_zones(
                &forged,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&zones)
            )
            .is_err()
        );
        let mut changed = zones.clone();
        changed[1].kind = "urban".into();
        assert!(
            verify_with_zones(
                &p,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&changed)
            )
            .is_err()
        );
    }

    #[test]
    fn missing_raster_cannot_be_silently_treated_as_flat() {
        let (s, _, w) = fixture();
        assert!(plan(&s, &BakedTerrain::new(&s).unwrap(), w).is_err());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNetworkAudit {
    pub status: String,
    pub planned_edges: usize,
    pub realized_edges: usize,
    pub planned_sites: usize,
    pub realized_sites: usize,
    pub max_final_site_fill_m: f32,
    pub max_site_design_height_error_m: f32,
    pub max_final_roadbed_fill_m: f32,
    pub max_design_height_error_m: f32,
    pub failures: Vec<String>,
}

/// Rebuild from natural raster and inspect every final roadbed sample.
/// Does not repair missing roads or accept a saved success flag as evidence.
pub fn verify(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_with_zones(record, scene, natural, final_terrain, water, entities, None)
}
pub fn verify_with_zones(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_with_program(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        None,
    )
}
pub fn verify_with_program(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        None,
    )
}
pub fn verify_with_style(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        Some(style),
    )
}
fn verify_impl(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: Option<&serde_json::Value>,
) -> Result<RoadNetworkAudit, TerrainError> {
    let expected = if let Some(style) = style {
        plan_with_style(scene, natural, water, zones, program, style)?
    } else {
        plan_with_program(scene, natural, water, zones, program)?
    };

    if serde_json::to_value(&expected).map_err(|e| TerrainError(e.to_string()))?
        != serde_json::to_value(record).map_err(|e| TerrainError(e.to_string()))?
    {
        return Err(TerrainError(
            "road plan differs from natural raster reconstruction".into(),
        ));
    }
    if !final_terrain.matches_scene(scene) {
        return Err(TerrainError("road audit final terrain mismatch".into()));
    }
    final_terrain.require_complete()?;
    let mut audit = RoadNetworkAudit {
        status: "pass".into(),
        planned_edges: expected.edges.len(),
        realized_edges: 0,
        planned_sites: expected.roadside_sites.len(),
        realized_sites: 0,
        max_final_site_fill_m: 0.,
        max_site_design_height_error_m: 0.,
        max_final_roadbed_fill_m: 0.,
        max_design_height_error_m: 0.,
        failures: Vec::new(),
    };
    if expected.edges.is_empty() {
        audit
            .failures
            .push("no feasible connected road network".into());
    }
    let mut ids = BTreeMap::<&str, Vec<&EntityInstance>>::new();
    for e in entities {
        ids.entry(&e.entity_id).or_default().push(e);
    }
    for planned in expected.entities() {
        let Some(found) = ids.get(planned.entity_id.as_str()) else {
            audit
                .failures
                .push(format!("missing planned road {}", planned.entity_id));
            continue;
        };
        if found.len() != 1 {
            audit
                .failures
                .push(format!("duplicate road id {}", planned.entity_id));
            continue;
        }
        let e = found[0];
        if e.kind != "road"
            || e.asset_id != planned.asset_id
            || e.height_m != planned.height_m
            || e.world_x != planned.world_x
            || e.world_z != planned.world_z
            || e.width_m != planned.width_m
            || e.depth_m != planned.depth_m
            || e.rotation_y_deg != planned.rotation_y_deg
            || e.scale != 1.
            || serde_json::to_value(&e.grounding).ok()
                != serde_json::to_value(&planned.grounding).ok()
        {
            audit
                .failures
                .push(format!("road transform differs from plan {}", e.entity_id));
            continue;
        }
        let (hw, hd) = if e.rotation_y_deg == 0. {
            (e.width_m as i32 / 2 + 4, 7)
        } else {
            (7, e.width_m as i32 / 2 + 4)
        };
        let mut error = 0_f32;
        let mut fill = 0_f32;
        let mut cut = 0_f32;
        for z in e.world_z - hd..=e.world_z + hd {
            for x in e.world_x - hw..=e.world_x + hw {
                let n = natural
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing natural roadbed sample".into()))?;
                let f = final_terrain
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing final roadbed sample".into()))?;
                let (paved_hw, paved_hd) = if planned.grounding.road_profile.is_some() {
                    (hw, hd)
                } else if e.rotation_y_deg == 0. {
                    (e.width_m as i32 / 2, 5)
                } else {
                    (5, e.width_m as i32 / 2)
                };
                if (x - e.world_x).abs() <= paved_hw && (z - e.world_z).abs() <= paved_hd {
                    let target = planned
                        .grounding
                        .road_profile
                        .as_ref()
                        .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
                        .unwrap_or(f64::from(planned.world_y));
                    error = error.max((f - target as f32).abs());
                }
                fill = fill.max(f - n);
                cut = cut.max(n - f);
            }
        }
        audit.max_final_roadbed_fill_m = audit.max_final_roadbed_fill_m.max(fill);
        audit.max_design_height_error_m = audit.max_design_height_error_m.max(error);
        let tolerance = if planned.grounding.road_profile.is_some() {
            0.00005
        } else {
            0.26
        };
        let max_fill = planned
            .grounding
            .road_profile
            .as_ref()
            .map_or(expected.max_roadbed_fill_m, |p| {
                p.max_fill_mm as f32 / 1000.
            });
        if error > tolerance
            || fill > max_fill + tolerance
            || cut > tolerance
            || e.world_y != planned.world_y
        {
            audit.failures.push(format!(
                "road {} changed after planning: design error {error}m, fill {fill}m, cut {cut}m",
                e.entity_id
            ));
        } else {
            audit.realized_edges += 1;
        }
    }
    // Planned sites are obligations, not decorative hints. Audit their final
    // geometry and the full support pad independently of serialized reports.
    for planned in expected.site_entities() {
        let Some(found) = ids.get(planned.entity_id.as_str()) else {
            audit.failures.push(format!(
                "missing planned roadside site {}",
                planned.entity_id
            ));
            continue;
        };
        if found.len() != 1 {
            audit
                .failures
                .push(format!("duplicate roadside site id {}", planned.entity_id));
            continue;
        }
        let e = found[0];
        let anchors_match = e.anchors.len() == planned.anchors.len()
            && e.anchors.iter().zip(&planned.anchors).all(|(a, b)| {
                a.id == b.id
                    && a.world_x == b.world_x
                    && a.world_z == b.world_z
                    && a.direction == b.direction
                    && a.target == b.target
            });
        if !anchors_match
            || e.kind != planned.kind
            || e.asset_id != planned.asset_id
            || e.world_x != planned.world_x
            || e.world_z != planned.world_z
            || e.world_y != planned.world_y
            || e.grounding.bottom_offset_m != planned.grounding.bottom_offset_m
            || e.grounding.roadbed_offset_m != planned.grounding.roadbed_offset_m
            || e.width_m != planned.width_m
            || e.depth_m != planned.depth_m
            || e.height_m != planned.height_m
            || e.rotation_y_deg != planned.rotation_y_deg
            || e.scale != 1.
        {
            audit.failures.push(format!(
                "roadside site transform differs from plan {}",
                e.entity_id
            ));
            continue;
        }
        let mut error = 0_f32;
        let mut fill = 0_f32;
        let mut cut = 0_f32;
        let (hw, hd) = super::spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
        let (hw, hd) = (hw as i32, hd as i32);
        for z in e.world_z - hd - 2..=e.world_z + hd + 2 {
            for x in e.world_x - hw - 2..=e.world_x + hw + 2 {
                let n = natural
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing natural site sample".into()))?;
                let f = final_terrain
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing final site sample".into()))?;
                if (x - e.world_x).abs() <= hw && (z - e.world_z).abs() <= hd {
                    error = error.max((f - planned.ground_height_m()).abs());
                }
                fill = fill.max(f - n);
                cut = cut.max(n - f);
            }
        }
        audit.max_final_site_fill_m = audit.max_final_site_fill_m.max(fill);
        audit.max_site_design_height_error_m = audit.max_site_design_height_error_m.max(error);
        let precise = expected.roadside_sites.iter().find(|s|s.id==planned.entity_id).and_then(|s|s.support_level_mm);
        let mismatch_mm = precise.and_then(|level| {
            (e.world_z-hd-2..=e.world_z+hd+2).find_map(|z| (e.world_x-hw-2..=e.world_x+hw+2).find_map(|x| {let actual=final_terrain.height_millimetres(x,z);(actual!=Some(level)).then_some((x,z,level,actual))}))
        });
        let precision = if precise.is_some() {0.0001} else {0.26};
        if mismatch_mm.is_some() || error > precision || fill > expected.max_roadbed_fill_m + precision || cut > precision {
            audit.failures.push(format!("roadside site {} changed after planning: design error {error}m, fill {fill}m, cut {cut}m, support sample {mismatch_mm:?}", e.entity_id));
        } else {
            audit.realized_sites += 1;
        }
    }
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    Ok(audit)
}

/// Prevent a proposed high building platform from flooding an already designed
/// roadbed with its fill transition. Checks natural+candidate engineering,
/// not a looser collision descriptor. Authored assets still fail final audit.
pub(super) fn building_fill_conflicts(
    terrain_context: &super::TerrainContext<'_>,
    roads: &[EntityInstance],
    candidate: &EntityInstance,
) -> bool {
    let carves = super::plan_terrain_carves(std::slice::from_ref(candidate));
    if carves.is_empty() {
        return false;
    }
    for road in roads {
        let (hw, hd) = super::spatial_half_extents(road.width_m, road.depth_m, road.rotation_y_deg);
        let (hw, hd) = (hw.ceil() as i32 + 4, hd.ceil() as i32 + 4);
        if !carves.iter().any(|c| {
            (c.cx - f64::from(road.world_x)).abs() <= c.half_w + c.blend_m + f64::from(hw)
                && (c.cz - f64::from(road.world_z)).abs() <= c.half_d + c.blend_m + f64::from(hd)
        }) {
            continue;
        }
        for z in road.world_z - hd..=road.world_z + hd {
            for x in road.world_x - hw..=road.world_x + hw {
                let h = terrain_context.height(x, z, &carves);
                let target = road
                    .grounding
                    .road_profile
                    .as_ref()
                    .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
                    .unwrap_or(f64::from(road.world_y));
                if f64::from(h) > target + 0.01 {
                    return true;
                }
            }
        }
    }
    false
}
