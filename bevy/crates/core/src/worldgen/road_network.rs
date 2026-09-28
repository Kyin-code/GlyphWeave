//! Bounded fill-only, level lowland road network. This deliberately does not
//! claim to model mountain passes, bridges or an optimized highway alignment.
use super::LandUseZone;
use super::zoning::{ZoningMask, ZoningRejection};
use super::{
    EntityInstance, GroundingSpec, SceneSpec, SpatialAnchor, WaterGeometry, WorldManifest,
};
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
    pub parcel_insets: Option<super::parcel_geometry::ParcelInsets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_level_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage_road_point: Option<[i32; 2]>,
    pub rotation_y_deg: f32,
    pub natural_min_m: f32,
    pub natural_max_m: f32,
}
impl RoadsideSite {
    pub(crate) fn entity(&self) -> EntityInstance {
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
            world_y: s
                .support_level_mm
                .map_or(s.support_level_m, |mm| mm.div_euclid(1000)),
            width_m: s.width_m as f32,
            depth_m: s.depth_m as f32,
            height_m: s.program.as_ref().map_or(8., |p| p.height_m as f32),
            scale: 1.,
            rotation_y_deg: s.rotation_y_deg,
            grounding: GroundingSpec {
                frontage_road_point: s.frontage_road_point,
                bottom_offset_m: s
                    .support_level_mm
                    .map_or(0., |mm| mm.rem_euclid(1000) as f32 / 1000.),
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
                let road_width_m = profile
                    .as_ref()
                    .map_or(8., |p| p.road_width_m as f32);
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
                    depth_m: road_width_m,
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
                    parcel_insets: None,
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
    plan_with_style_declared(
        scene,
        terrain,
        water,
        zones,
        program,
        style,
        super::NaturalSource::Undeclared,
    )
}

/// The same planning entry with an explicit declaration of where the scene's
/// authoritative natural surface comes from. Version-3 supply needs it: the
/// demand-led phase must never guess the source, so a caller that does not
/// declare one is refused rather than defaulted to a zero seed or a raster
/// fallback.
pub(super) fn plan_with_style_declared(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
    source: super::NaturalSource,
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
        super::auto_roads::append_declared(
            &mut result,
            scene,
            terrain,
            water,
            zones.ok_or_else(|| TerrainError("automatic roads require zones".into()))?,
            style,
            source,
        )?;
    }
    super::frontage::append(&mut result, scene, terrain, water, zones, style)?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Test-only view of the ungraded base domain (same rule as the append
    /// phase's trusted contract derivation).
    pub(super) fn base_only_plan_for_test(plan: &RoadNetworkPlan) -> RoadNetworkPlan {
        let entities = plan.entities();
        let mut base = plan.clone();
        base.edges.retain(|edge| {
            entities
                .iter()
                .find(|entity| entity.entity_id == edge.id)
                .is_some_and(|entity| entity.grounding.road_profile.is_none())
        });
        let keep: std::collections::BTreeSet<usize> =
            base.edges.iter().flat_map(|e| [e.from, e.to]).collect();
        base.nodes.retain(|node| keep.contains(&node.id));
        base.extensions.clear();
        base
    }
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
    fn the_legacy_undeclared_entry_and_unknown_sources_are_refused() {
        use crate::height_payload::encode_millimetres;
        let (scene, _flat, water) = fixture();
        let zones = vec![LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: scene.origin_x,
            world_z: scene.origin_z,
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            source: "test".into(),
        }];
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let values: Vec<f64> = (0..scene.depth_m as i32)
            .flat_map(|z| (0..scene.width_m as i32).map(move |x| (x, z)))
            .map(|(x, z)| {
                super::super::terrain_height_metres(
                    0,
                    scene.origin_x + x,
                    scene.origin_z + z,
                    water,
                    &[],
                )
            })
            .collect();
        natural
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        let plan = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        // The old undeclared audit wrapper must still refuse a version-3 input
        // rather than silently falling back to a zero seed or a sampled source.
        let v3_style = serde_json::json!({
            "frontageProgram": {"version": 1, "requests": [{
                "id": "homes", "sceneId": "roads", "purpose": "urban",
                "kind": "residential_home", "count": 1, "widthM": 12, "depthM": 10,
                "heightM": 8, "stationSpacingM": 24, "setbackM": 8,
                "maxCandidateAttempts": 8,
                "parcelInsets": {"frontM": 4, "sideM": 2, "rearM": 4}
            }]},
            "automaticRoadExtensions": {"version": 3, "scenes": [{
                "sceneId": "roads", "maxNewSegments": 1, "maxCandidateAttempts": 8,
                "segmentLengthsM": [64], "priorityPurposes": ["urban"],
                "reachDistanceM": 32,
                "hierarchy": {"tiers": [
                    {"id": "collector-16", "role": "collector", "widthM": 16, "purposes": ["urban"]},
                    {"id": "local-12", "role": "local", "widthM": 12, "purposes": ["urban"]}
                ]},
                "supply": {"version": 1, "maxCandidateStates": 1, "preflightBudgetMs": 180000, "mode": "append"}
            }]},
        });
        let error = verify_with_style(
            &plan,
            &scene,
            &natural,
            &natural,
            water,
            &entities,
            Some(&zones),
            None,
            &v3_style,
        )
        .unwrap_err();
        assert!(
            error.0.contains("declared natural source"),
            "{}",
            error.0
        );
        // The manifest-bound derivation is the shared rule, and an authored
        // source this build cannot reproduce is refused instead of guessed onto
        // `SuppliedRaster`.
        let mut manifest = super::super::WorldManifest::default_demo();
        manifest.scenes = vec![scene.clone()];
        manifest.style["heightPrecision"] = serde_json::json!("millimeter");
        let declared = super::super::declared_natural_source(&manifest, &scene).unwrap();
        assert_eq!(
            declared,
            super::super::NaturalSource::AuthoredSeed {
                seed: manifest.world.seed ^ scene.seed_offset,
                precise: true,
            }
        );
        manifest.style["terrain"] = serde_json::json!("imported-raster");
        let error = super::super::declared_natural_source(&manifest, &scene).unwrap_err();
        assert!(
            error.to_string().contains("unsupported authored terrain source"),
            "{error}"
        );
    }

    #[test]
    fn shared_final_support_leaf_is_the_production_audit_body() {
        use crate::height_payload::encode_millimetres;
        let (scene, _flat, water) = fixture();
        let zones = vec![LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: scene.origin_x,
            world_z: scene.origin_z,
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            source: "test".into(),
        }];
        // The authoritative natural source is the authored seed field, so the
        // production encoder and the carve path agree cell for cell.
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let values: Vec<f64> = (0..scene.depth_m as i32)
            .flat_map(|z| (0..scene.width_m as i32).map(move |x| (x, z)))
            .map(|(x, z)| {
                super::super::terrain_height_metres(
                    0,
                    scene.origin_x + x,
                    scene.origin_z + z,
                    water,
                    &[],
                )
            })
            .collect();
        natural
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        let plan = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let carves = super::super::plan_terrain_carves(&entities);
        let mask = super::super::zoning::ZoningMask::new(&scene, &zones).unwrap();
        let (height, _s, _l, _t) = super::super::generate_chunk_encoded(
            0,
            scene.origin_x,
            scene.origin_z,
            scene.width_m,
            scene.depth_m,
            water,
            &carves,
            true,
            Some(&mask),
        );
        let mut final_terrain = BakedTerrain::new(&scene).unwrap();
        final_terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &height,
            )
            .unwrap();
        let empty_style = serde_json::json!({});
        let production = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &empty_style,
        )
        .unwrap();
        let leaf = audit_final_support_for_record(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &empty_style,
        )
        .unwrap();
        assert_eq!(production.realized_edges, leaf.realized_edges);
        assert_eq!(production.planned_edges, leaf.planned_edges);
        assert_eq!(production.realized_sites, leaf.realized_sites);
        assert_eq!(production.failures, leaf.failures);
        assert!(plan.edges.len() > 0);
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

    type GradedFixture = (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
        i32,
    );

    /// A gentle cone around (160,160): non-flat in both axes, so a legal graded
    /// corridor exists in every cardinal direction while the shoulder is
    /// genuinely blended. Production planning/carve/audit are used unchanged.
    fn graded_fixture() -> GradedFixture {
        use crate::height_payload::encode_millimetres;
        let scene = SceneSpec {
            scene_id: "graded-audit".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut values = Vec::with_capacity(512 * 512);
        for z in 0..512 {
            for x in 0..512 {
                let fall = (f64::from(x) - 160.).abs() + (f64::from(z) - 160.).abs();
                values.push((8. - 0.05 * fall).max(0.));
            }
        }
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, values).unwrap(),
            )
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "graded.urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let base = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let level_mm = base.level_m.expect("resolved base level") * 1000;
        (scene, natural, water, zones, base, level_mm)
    }

    /// A style with one graded corridor of `width_m`, along the axis implied by
    /// `rotation` (0 = x, 90 = z), starting from a real base node. The search
    /// only accepts candidates the production planner itself accepts.
    fn single_corridor_style(fixture: &GradedFixture, width_m: u32, rotation: i32) -> Option<serde_json::Value> {
        let (scene, natural, water, zones, base, level_mm) = fixture;
        let directions: [(i32, i32); 2] = if rotation == 90 {
            [(0, 1), (0, -1)]
        } else {
            [(1, 0), (-1, 0)]
        };
        for node in &base.nodes {
            for (dx, dz) in directions {
                for length in [64_i32, 96, 48] {
                    let start = [node.world_x, node.world_z];
                    let end = [start[0] + dx * length, start[1] + dz * length];
                    if end[0] < 12 || end[1] < 12
                        || end[0] + 12 > scene.width_m as i32
                        || end[1] + 12 > scene.depth_m as i32
                    {
                        continue;
                    }
                    let candidate = serde_json::json!({
                        "roadExtensions": {"version": 1, "corridors": [{
                            "sceneId": "graded-audit",
                            "start": start,
                            "end": end,
                            "roadWidthM": width_m,
                            "shoulderWidthM": 3,
                            "maxFillMm": 6000,
                            "maxRiseMmPerM": 80,
                            "startLevelMm": level_mm,
                            "endLevelMm": level_mm,
                            "junctionLengthM": 8
                        }]}
                    });
                    let mut probe = base.clone();
                    if super::super::road_extensions::append(
                        &mut probe,
                        scene,
                        natural,
                        *water,
                        zones,
                        &candidate,
                    )
                    .is_ok()
                    {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// A 16m collector feeding a 12m local road: the shared node is pinned to
    /// the base level, so the two corridors must agree there.
    fn collector_with_local_style(fixture: &GradedFixture) -> Option<serde_json::Value> {
        let (scene, natural, water, zones, base, level_mm) = fixture;
        let directions: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        // Deterministic bounded search: straight continuations first, then
        // L-shaped junctions, shortest first.
        let mut attempts = 0_u32;
        for node in &base.nodes {
            for (dx, dz) in directions {
                for (ex, ez) in directions {
                    if (dx + ex, dz + ez) == (0, 0) {
                        continue;
                    }
                    for (first, second) in [
                        (48_i32, 48_i32),
                        (64, 48),
                        (48, 64),
                        (64, 64),
                        (32, 48),
                        (48, 32),
                    ] {
                    if attempts >= 600 {
                        return None;
                    }
                    attempts += 1;
                    let start = [node.world_x, node.world_z];
                    let mid = [start[0] + dx * first, start[1] + dz * first];
                    let end = [mid[0] + ex * second, mid[1] + ez * second];
                    if start[0] < 12 || start[1] < 12
                        || mid[0] < 12 || mid[1] < 12
                        || end[0] < 12 || end[1] < 12
                        || end[0] + 12 > scene.width_m as i32
                        || end[1] + 12 > scene.depth_m as i32
                    {
                        continue;
                    }
                    let candidate = serde_json::json!({
                        "roadExtensions": {"version": 1, "corridors": [
                            {"sceneId": "graded-audit", "start": start, "end": mid,
                             "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                             "maxRiseMmPerM": 80, "startLevelMm": level_mm,
                             "endLevelMm": level_mm, "junctionLengthM": 8},
                            {"sceneId": "graded-audit", "start": mid, "end": end,
                             "roadWidthM": 12, "shoulderWidthM": 3, "maxFillMm": 6000,
                             "maxRiseMmPerM": 80, "startLevelMm": level_mm,
                             "endLevelMm": level_mm, "junctionLengthM": 8}
                        ]}
                    });
                    let mut probe = base.clone();
                    if super::super::road_extensions::append(
                        &mut probe,
                        scene,
                        natural,
                        *water,
                        zones,
                        &candidate,
                    )
                    .is_ok()
                    {
                        return Some(candidate);
                    }
                    }
                }
            }
        }
        None
    }

    /// Build the production final terrain (carve over the planned entities).
    fn carved_final(
        scene: &SceneSpec,
        natural: &BakedTerrain,
        entities: &[EntityInstance],
    ) -> BakedTerrain {
        use crate::height_payload::encode_millimetres;
        let carves = super::super::plan_terrain_carves(entities);
        let mut values = Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
        for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
            for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                values.push(super::super::apply_terrain_carves(
                    f64::from(natural.height(x, z).unwrap()),
                    x,
                    z,
                    &carves,
                ));
            }
        }
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        terrain
    }

    /// The graded band of one profiled road as world-axis halves.
    fn band_of(entity: &EntityInstance) -> (i32, i32) {
        super::super::graded_flat_top_half_extents(entity)
    }

    /// Worst |final - design| inside a road's graded band.
    fn worst_band_error(
        entity: &EntityInstance,
        final_terrain: &BakedTerrain,
    ) -> f32 {
        let profile = entity.grounding.road_profile.as_ref().unwrap();
        let (band_x, band_z) = band_of(entity);
        let mut worst = 0_f32;
        for dz in -band_z..=band_z {
            for dx in -band_x..=band_x {
                let (x, z) = (entity.world_x + dx, entity.world_z + dz);
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                worst = worst.max((actual - target).abs());
            }
        }
        worst
    }

    #[test]
    fn graded_road_audit_accepts_legal_engineering_and_rejects_real_damage() {
        let fixture = graded_fixture();
        let style = single_corridor_style(&fixture, 12, 0)
            .expect("the control terrain admits a legal graded corridor");
        let (scene, natural, water, zones, _base, _level_mm) = fixture;
        let plan = plan_with_style(&scene, &natural, water, Some(&zones), None, &style).unwrap();
        assert_eq!(plan.extensions.len(), 1, "the control corridor must be built");
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        // Exactly the production final terrain: the carve over the plan.
        let final_terrain = carved_final(&scene, &natural, &entities);
        use crate::height_payload::encode_millimetres;
        let audit = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        assert_eq!(
            audit.status, "pass",
            "legal graded engineering must pass: {:?}",
            audit.failures
        );
        assert_eq!(audit.realized_edges, plan.edges.len());

        // W1 band rule: max(7, pavement/2 + 1) keeps the 8m and 12m values
        // bit-identical at 7m and gives a 16m collector 9m, so every contract
        // width covers its pavement and the structural guard stays silent.
        for (width, band) in [(8.0_f32, 7), (12.0, 7), (16.0, 9)] {
            let probe: EntityInstance = serde_json::from_value(serde_json::json!({
                "entityId": "probe", "assetId": "prop.road", "kind": "road",
                "worldX": 0, "worldZ": 0, "worldY": 0, "scale": 1.0,
                "widthM": 128.0, "depthM": width, "heightM": 0.2, "rotationYDeg": 0.0
            }))
            .unwrap();
            let (along, transverse) = super::super::graded_flat_top_half_extents(&probe);
            let (transport_along, transport_transverse) =
                super::super::graded_transport_half_extents(&probe);
            let (_, pavement_transverse) = super::super::graded_pavement_half_extents(&probe);
            assert_eq!(along, 68);
            assert_eq!(transport_along, 68);
            assert_eq!(transport_transverse, width as i32 / 2 + 3);
            assert_eq!(pavement_transverse, width as i32 / 2);
            assert_eq!(transverse, band, "graded band for a {width}m road");
            assert!(
                transverse >= pavement_transverse,
                "the graded band must cover the {width}m pavement"
            );
            assert_eq!(
                super::super::graded_band_shortfall_m(pavement_transverse, transverse),
                0
            );
        }
        // Narrow-band regression: the pre-W1 constant band (7m) under a 16m
        // pavement (8m half) is exactly one structural shortfall, so a road in
        // that state is refused instead of being silently counted as built.
        assert_eq!(super::super::graded_band_shortfall_m(8, 7), 1);
        assert_eq!(super::super::graded_band_shortfall_m(6, 7), 0);
        assert_eq!(super::super::graded_band_shortfall_m(4, 7), 0);
        assert_eq!(super::super::graded_band_shortfall_m(12, 7), 5);

        // The whole graded footprint, shoulder included, holds the design
        // level: that is what the independent audit requires.
        let graded = entities
            .iter()
            .find(|entity| entity.grounding.road_profile.is_some())
            .expect("a profiled road");
        let profile = graded.grounding.road_profile.as_ref().unwrap();
        // The design level is required across the earthwork's graded band.
        let (along_half, transverse_half) = super::super::graded_flat_top_half_extents(graded);
        // A road at rotation 0 runs along x, so its transverse axis is z.
        let transverse_is_z = graded.rotation_y_deg != 90.;
        let mut worst = 0_f32;
        for dz in -transverse_half..=transverse_half {
            for dx in -along_half..=along_half {
                let (x, z) = if transverse_is_z {
                    (graded.world_x + dx, graded.world_z + dz)
                } else {
                    (graded.world_x + dz, graded.world_z + dx)
                };
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                worst = worst.max((actual - target).abs());
            }
        }
        assert!(
            worst <= 0.00005,
            "the graded footprint must hold the design level, worst {worst}"
        );

        // The transition band is judged by its own contract: it is blended and
        // never required to equal the design level, so the audit still passes
        // while that ring genuinely deviates from the design height.
        let transport_half = if transverse_is_z {
            super::super::graded_transport_half_extents(graded).1
        } else {
            super::super::graded_transport_half_extents(graded).0
        };
        assert!(transport_half > transverse_half);
        let mut worst_transition = 0_f32;
        for dz in -transport_half..=transport_half {
            if dz.abs() <= transverse_half {
                continue;
            }
            for dx in -along_half..=along_half {
                let (x, z) = if transverse_is_z {
                    (graded.world_x + dx, graded.world_z + dz)
                } else {
                    (graded.world_x + dz, graded.world_z + dx)
                };
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                let natural_h = natural.height(x, z).unwrap();
                assert!(
                    actual >= natural_h - 0.0005,
                    "the shoulder must never excavate: actual {actual}, natural {natural_h}"
                );
                assert!(
                    actual - natural_h <= 6.0 + 0.0005,
                    "the shoulder fill must stay inside the road's cap: actual {actual}, natural {natural_h}"
                );
                worst_transition = worst_transition.max((actual - target).abs());
            }
        }
        assert!(
            worst_transition > 0.001,
            "the shoulder must really deviate, got {worst_transition}"
        );

        let pavement_half = if transverse_is_z {
            super::super::graded_pavement_half_extents(graded).1
        } else {
            super::super::graded_pavement_half_extents(graded).0
        };
        let at = |transverse: i32, along: i32| {
            if transverse_is_z {
                (graded.world_x + along, graded.world_z + transverse)
            } else {
                (graded.world_x + transverse, graded.world_z + along)
            }
        };
        let perturb = |cell: (i32, i32), delta: f64| -> BakedTerrain {
            let mut terrain = BakedTerrain::new(&scene).unwrap();
            let mut values = Vec::with_capacity(512 * 512);
            for zz in 0..512 {
                for xx in 0..512 {
                    let base = f64::from(final_terrain.height(xx, zz).unwrap());
                    values.push(if (xx, zz) == cell { base + delta } else { base });
                }
            }
            terrain
                .insert_chunk(
                    0,
                    0,
                    512,
                    512,
                    &encode_millimetres(512, 512, values).unwrap(),
                )
                .unwrap();
            terrain
        };
        let audit_of = |terrain: &BakedTerrain| {
            verify_with_style(
                &plan,
                &scene,
                &natural,
                terrain,
                water,
                &entities,
                Some(&zones),
                None,
                &style,
            )
            .unwrap()
        };

        // Pavement edge +1mm: one design-error failure, and the failing road is
        // not counted as realized.
        let pavement_edge = audit_of(&perturb(at(pavement_half, 0), 0.001));
        assert_eq!(pavement_edge.status, "fail");
        assert_eq!(
            pavement_edge.failures.len(),
            1,
            "{:?}",
            pavement_edge.failures
        );
        assert!(pavement_edge.failures[0].contains("changed after planning"));
        assert_eq!(pavement_edge.realized_edges, plan.edges.len() - 1);

        // Graded-band edge +1mm: still inside the band the earthwork flattens,
        // so it is a design error even though it is outside the pavement.
        let band_edge = audit_of(&perturb(at(transverse_half, 0), 0.001));
        assert_eq!(band_edge.status, "fail");
        assert_eq!(band_edge.failures.len(), 1, "{:?}", band_edge.failures);
        assert!(band_edge.max_design_height_error_m > 0.0009);

        // Unauthorized excavation: lowering the pavement 0.2m below the natural
        // ground fails on the cut criterion (not merely as a design error).
        let excavated = {
            let cell = at(pavement_half, 0);
            let natural_m = f64::from(natural.height(cell.0, cell.1).unwrap());
            let final_m = f64::from(final_terrain.height(cell.0, cell.1).unwrap());
            audit_of(&perturb(cell, natural_m - final_m - 0.2))
        };
        assert_eq!(excavated.status, "fail");
        assert!(
            excavated.failures[0].contains("cut 0.2"),
            "{:?}",
            excavated.failures
        );

        // Over-fill: raising a transport-footprint cell above the road's own
        // fill cap fails on the fill criterion.
        let overfilled = {
            let cell = at(pavement_half, 0);
            let natural_mm = natural.height_millimetres(cell.0, cell.1).unwrap() as f64 / 1000.;
            audit_of(&perturb(cell, natural_mm + 6.5 - f64::from(
                final_terrain.height(cell.0, cell.1).unwrap(),
            )))
        };
        assert_eq!(overfilled.status, "fail");
        assert!(
            overfilled.max_final_roadbed_fill_m > 6.0,
            "fill {} must exceed the cap",
            overfilled.max_final_roadbed_fill_m
        );
    }

    /// B02-W1: 8m, 12m and 16m graded roads, each in two orthogonal directions,
    /// on non-flat natural ground, through production plan -> carve -> verify.
    #[test]
    fn graded_width_and_direction_matrix_is_supported() {
        let fixture = graded_fixture();
        let (scene, natural, water, zones, _base, _level_mm) = &fixture;
        let mut matrix = Vec::new();
        for width in [8_u32, 12, 16] {
            for rotation in [0_i32, 90] {
                let style = single_corridor_style(&fixture, width, rotation).unwrap_or_else(|| {
                    panic!("no legal {width}m corridor was found at rotation {rotation}")
                });
                let plan = plan_with_style(scene, natural, *water, Some(zones), None, &style)
                    .unwrap_or_else(|error| panic!("{width}m/{rotation}: {error}"));
                assert_eq!(plan.extensions.len(), 1, "{width}m/{rotation}");
                let mut entities = plan.entities();
                entities.extend(plan.site_entities());
                let final_terrain = carved_final(scene, natural, &entities);
                let audit = verify_with_style(
                    &plan,
                    scene,
                    natural,
                    &final_terrain,
                    *water,
                    &entities,
                    Some(zones),
                    None,
                    &style,
                )
                .unwrap_or_else(|error| panic!("{width}m/{rotation}: {error}"));
                assert_eq!(
                    audit.status, "pass",
                    "{width}m/{rotation} must pass: {:?}",
                    audit.failures
                );
                assert_eq!(audit.realized_edges, plan.edges.len());
                // Every profiled road's graded band covers its pavement, holds
                // the design level exactly, and keeps the shoulder as a fill.
                for entity in entities
                    .iter()
                    .filter(|entity| entity.grounding.road_profile.is_some())
                {
                    let (band_x, band_z) = band_of(entity);
                    let (pavement_x, pavement_z) =
                        super::super::graded_pavement_half_extents(entity);
                    let transverse_band = if entity.rotation_y_deg == 90. {
                        band_x
                    } else {
                        band_z
                    };
                    let transverse_pavement = if entity.rotation_y_deg == 90. {
                        pavement_x
                    } else {
                        pavement_z
                    };
                    assert!(
                        transverse_band >= transverse_pavement,
                        "{width}m/{rotation}: band {transverse_band} < pavement {transverse_pavement}"
                    );
                    assert_eq!(transverse_band, (entity.depth_m as i32 / 2 + 1).max(7));
                    let worst = worst_band_error(entity, &final_terrain);
                    assert!(
                        worst <= 0.00005,
                        "{width}m/{rotation}: graded band deviation {worst}"
                    );
                    matrix.push((width, rotation, entity.depth_m as u32, transverse_band));
                }
            }
        }
        assert_eq!(matrix.len(), 6, "one profiled road per combination");
        assert!(matrix.iter().any(|(_, _, width, band)| *width == 16 && *band == 9));
        assert!(matrix.iter().all(|(_, _, width, band)| *band == (*width as i32 / 2 + 1).max(7)));
    }

    /// B02-W1: a real 16m collector feeding a 12m local road, both pinned to
    /// the base level, with the shared junction held to one elevation.
    #[test]
    fn wide_collector_with_local_connection_is_supported() {
        let fixture = graded_fixture();
        let style = collector_with_local_style(&fixture)
            .expect("the control terrain admits a 16m collector feeding a 12m local road");
        let (scene, natural, water, zones, _base, level_mm) = fixture;
        let plan = plan_with_style(&scene, &natural, water, Some(&zones), None, &style)
            .unwrap_or_else(|error| panic!("collector+local planning failed: {error}"));
        assert_eq!(plan.extensions.len(), 2, "collector + local must both be built");
        let widths: Vec<u32> = plan
            .extensions
            .iter()
            .map(|extension| extension.design.request.road_width_m)
            .collect();
        assert_eq!(widths, vec![16, 12]);
        // The junction is shared and pinned to the base level in both designs.
        let shared = plan.extensions[0].design.request.end;
        assert_eq!(plan.extensions[1].design.request.start, shared);
        for extension in &plan.extensions {
            let start_level = extension.design.request.start_level_mm.unwrap();
            let end_level = extension.design.request.end_level_mm.unwrap();
            assert_eq!(start_level, level_mm);
            assert_eq!(end_level, level_mm);
        }
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        let final_terrain = carved_final(&scene, &natural, &entities);
        let audit = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.realized_edges, plan.edges.len());
        // The 16m pavement is fully supported inside its 9m graded band.
        let collector = entities
            .iter()
            .find(|entity| {
                entity
                    .grounding
                    .road_profile
                    .as_ref()
                    .is_some_and(|profile| profile.road_width_m == 16)
            })
            .expect("the collector entity");
        let collector_band = band_of(collector);
        let collector_transverse = if collector.rotation_y_deg == 90. {
            collector_band.0
        } else {
            collector_band.1
        };
        assert_eq!(collector_transverse, 9, "16m pavement needs a 9m graded band");
        assert!(worst_band_error(collector, &final_terrain) <= 0.00005);
        assert_eq!(
            collector
                .grounding
                .road_profile
                .as_ref()
                .unwrap()
                .height_at(
                    f64::from(shared[0]),
                    f64::from(shared[1])
                )
                .map(|value| (value * 1000.).round() as i32),
            Some(level_mm)
        );
    }

    /// Manual diagnostic (ignored): searches the published C1 compact terrain
    /// for a 16m collector feeding a 12m local road that production planning
    /// accepts, and audits the best candidate. Used once to author the W1 CLI
    /// control; it needs a historical published world, so it is not part of the
    /// normal suite. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "needs bevy/work/collaboration/ds-b04-c1-r1/gen-compact-mixed-block; prints W1 corridor candidates"]
    fn w1_cli_control_corridor_search() {
        use crate::height_payload::encode_millimetres;
        use crate::worldgen::WaterKind;
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = repo.join("bevy/work/collaboration/ds-b04-c1-r1/gen-compact-mixed-block");
        if !root.is_dir() {
            eprintln!("SKIP missing {}", root.display());
            return;
        }
        let manifest: crate::worldgen::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).unwrap(),
        )
        .unwrap();
        let spec = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&spec.scene_id);
        let scene: crate::worldgen::SceneIndex =
            serde_json::from_slice(&std::fs::read(dir.join("scene.json")).unwrap()).unwrap();
        let mut natural = BakedTerrain::new(&spec).unwrap();
        for chunk in &scene.chunks {
            natural
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &std::fs::read(dir.join(chunk.natural_height_file.as_ref().unwrap()))
                        .unwrap(),
                )
                .unwrap();
        }
        let water =
            super::super::water_geometry(WaterKind::None, &[], &spec, &manifest.style);
        let profile = crate::worldgen::LandUseProfile::from_style(&manifest.style).unwrap();
        let zones = crate::worldgen::plan_landuse_zones(&spec, &profile);
        let base = plan_with_zones(&spec, &natural, water, Some(&zones)).unwrap();
        let level_mm = base.level_m.unwrap() * 1000;
        let alignment =
            super::super::road_alignment::AlignmentContext::new(&spec, &natural, water, &zones)
                .unwrap();
        let solve_end = |start: [i32; 2], end: [i32; 2], width: u32, start_level: i32| {
            let request = super::super::road_alignment::CorridorRequest {
                scene_id: spec.scene_id.clone(),
                start,
                end,
                road_width_m: width,
                shoulder_width_m: 3,
                max_fill_mm: 6000,
                max_rise_mm_per_m: 80,
                start_level_mm: Some(start_level),
                end_level_mm: None,
                junction_length_m: 8,
            };
            alignment.design(&request).ok().map(|design| {
                let solved = *design.profile_mm.last().unwrap() as i32;
                let mut levels = vec![solved];
                for delta in [-3000, -2000, -1000, -500, 500, 1000, 2000, 3000] {
                    levels.push(solved + delta);
                }
                levels
            })
        };
        let try_append = |corridors: Vec<serde_json::Value>| -> bool {
            let style = serde_json::json!({
                "roadExtensions": {"version": 1, "corridors": corridors}
            });
            let mut probe = base.clone();
            super::super::road_extensions::append(
                &mut probe,
                &spec,
                &natural,
                water,
                &zones,
                &style,
            )
            .is_ok()
        };
        let mut found = Vec::new();
        let mut attempts = 0_u32;
        'search: for node in &base.nodes {
            for (dx, dz) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                for (ex, ez) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                    if (dx + ex, dz + ez) == (0, 0) {
                        continue;
                    }
                    for (first, second) in [
                        (48_i32, 48_i32),
                        (64, 48),
                        (48, 64),
                        (32, 48),
                        (64, 96),
                        (96, 64),
                    ] {
                        if attempts >= 4000 {
                            break 'search;
                        }
                        attempts += 1;
                        let start = [node.world_x, node.world_z];
                        let mid = [start[0] + dx * first, start[1] + dz * first];
                        let end = [mid[0] + ex * second, mid[1] + ez * second];
                        if [start, mid, end].iter().any(|point| {
                            point[0] < 12
                                || point[1] < 12
                                || point[0] + 12 > spec.width_m as i32
                                || point[1] + 12 > spec.depth_m as i32
                        }) {
                            continue;
                        }
                        let Some(first_levels) = solve_end(start, mid, 16, level_mm) else {
                            continue;
                        };
                        for first_level in first_levels {
                            let Some(second_levels) = solve_end(mid, end, 12, first_level) else {
                                continue;
                            };
                            for second_level in second_levels {
                                let corridors = vec![
                                    serde_json::json!({
                                        "sceneId": spec.scene_id, "start": start, "end": mid,
                                        "roadWidthM": 16, "shoulderWidthM": 3,
                                        "maxFillMm": 6000, "maxRiseMmPerM": 80,
                                        "startLevelMm": level_mm, "endLevelMm": first_level,
                                        "junctionLengthM": 8
                                    }),
                                    serde_json::json!({
                                        "sceneId": spec.scene_id, "start": mid, "end": end,
                                        "roadWidthM": 12, "shoulderWidthM": 3,
                                        "maxFillMm": 6000, "maxRiseMmPerM": 80,
                                        "startLevelMm": first_level, "endLevelMm": second_level,
                                        "junctionLengthM": 8
                                    }),
                                ];
                                if try_append(corridors.clone()) {
                                    found.push((
                                        start,
                                        mid,
                                        end,
                                        first,
                                        second,
                                        level_mm,
                                        first_level,
                                        second_level,
                                    ));
                                    break;
                                }
                            }
                            if found.len() >= 6 {
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        eprintln!("W1 attempts={attempts} candidates={found:?}");
        let Some((start, mid, end, first, second, start_level, first_level, second_level)) =
            found.first().copied()
        else {
            eprintln!("W1 no candidate found");
            return;
        };
        let style = serde_json::json!({
            "roadExtensions": {"version": 1, "corridors": [
                {"sceneId": spec.scene_id, "start": start, "end": mid,
                 "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                 "maxRiseMmPerM": 80, "startLevelMm": start_level,
                 "endLevelMm": first_level, "junctionLengthM": 8},
                {"sceneId": spec.scene_id, "start": mid, "end": end,
                 "roadWidthM": 12, "shoulderWidthM": 3, "maxFillMm": 6000,
                 "maxRiseMmPerM": 80, "startLevelMm": first_level,
                 "endLevelMm": second_level, "junctionLengthM": 8}
            ]}
        });
        let plan = plan_with_style(&spec, &natural, water, Some(&zones), None, &style).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        let carves = super::super::plan_terrain_carves(&entities);
        let mut values = Vec::with_capacity((spec.width_m * spec.depth_m) as usize);
        for z in spec.origin_z..spec.origin_z + spec.depth_m as i32 {
            for x in spec.origin_x..spec.origin_x + spec.width_m as i32 {
                values.push(super::super::apply_terrain_carves(
                    f64::from(natural.height(x, z).unwrap()),
                    x,
                    z,
                    &carves,
                ));
            }
        }
        let mut final_terrain = BakedTerrain::new(&spec).unwrap();
        final_terrain
            .insert_chunk(
                spec.origin_x,
                spec.origin_z,
                spec.width_m,
                spec.depth_m,
                &encode_millimetres(spec.width_m, spec.depth_m, values).unwrap(),
            )
            .unwrap();
        let audit = verify_with_style(
            &plan,
            &spec,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        eprintln!(
            "W1 best start={start:?} mid={mid:?} end={end:?} lengths=({first},{second}) levels=({start_level},{first_level},{second_level}) audit={:?} realized={}/{}",
            audit.status, audit.realized_edges, audit.planned_edges
        );
        for failure in &audit.failures {
            eprintln!("W1 FAILURE {failure}");
        }
        eprintln!(
            "W1 manifest corridors: {}",
            serde_json::to_string(&style["roadExtensions"]["corridors"]).unwrap()
        );
    }

    /// A corridor whose fixed start level does not match its parent node is
    /// refused by production planning, not silently re-levelled.
    #[test]
    fn fixed_node_level_mismatch_is_refused_by_planning() {
        let fixture = graded_fixture();
        let (scene, natural, water, zones, base, level_mm) = &fixture;
        let node = base.nodes.first().expect("a base node");
        let mut style = serde_json::json!({
            "roadExtensions": {"version": 1, "corridors": [{
                "sceneId": "graded-audit",
                "start": [node.world_x, node.world_z],
                "end": [node.world_x + 64, node.world_z],
                "roadWidthM": 16,
                "shoulderWidthM": 3,
                "maxFillMm": 6000,
                "maxRiseMmPerM": 80,
                "startLevelMm": level_mm + 250,
                "endLevelMm": level_mm + 250,
                "junctionLengthM": 8
            }]}
        });
        let error = plan_with_style(scene, natural, *water, Some(zones), None, &style)
            .expect_err("a mismatched fixed junction must be refused");
        assert!(
            error.to_string().contains("start junction height mismatch")
                || error.to_string().contains("corridor infeasible"),
            "{error}"
        );
        // The same corridor at the node's own level is acceptable, proving the
        // refusal is about the mismatch and not about the geometry.
        style["roadExtensions"]["corridors"][0]["startLevelMm"] = serde_json::json!(level_mm);
        style["roadExtensions"]["corridors"][0]["endLevelMm"] = serde_json::json!(level_mm);
        let _ = plan_with_style(scene, natural, *water, Some(zones), None, &style);
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
        false,
        super::NaturalSource::Undeclared,
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
    verify_with_style_declared(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        style,
        super::NaturalSource::Undeclared,
    )
}
/// The production audit with an explicit natural-source declaration, so the
/// source rebuild that the audit performs for comparison cannot enter the
/// version-3 planner with a different seed than the candidate was built with.
pub(crate) fn verify_with_style_declared(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
    source: super::NaturalSource,
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
        false,
        source,
    )
}

/// The independent audit entry for callers outside the generator (the CLI).
///
/// It derives the scene's declared natural source from the *trusted original
/// manifest* with the same rule generation uses, then runs the full production
/// audit: the source rebuild re-plans the scene, compares the rebuilt plan with
/// the record and audits the final support. It never reads the source back out
/// of the published product and never takes the trusted-record shortcut, so a
/// record that its own manifest cannot reproduce is refused here. A manifest
/// whose declared terrain source this build cannot author is refused too,
/// instead of being guessed onto `SuppliedRaster`.
#[allow(clippy::too_many_arguments)]
pub fn verify_with_manifest(
    record: &RoadNetworkPlan,
    manifest: &WorldManifest,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkAudit, TerrainError> {
    let source = super::declared_natural_source(manifest, scene)
        .map_err(|error| TerrainError(error.to_string()))?;
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
        false,
        source,
    )
}

/// The same production support/entity audit as [`verify_with_style`], but for a
/// plan that is already trusted by its caller: it is not re-planned and not
/// compared for equality, so a demand-led append candidate can be audited with
/// the production rules, thresholds and coverage instead of a second, weaker
/// check. The caller is responsible for the provenance of `record` (the
/// accepted append path) and for [`audit_extension_provenance`].
pub(crate) fn audit_final_support_for_record(
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
        true,
        // A trusted record is never re-planned here, so no source is needed to
        // rebuild it; the caller has already asserted its provenance.
        super::NaturalSource::Undeclared,
    )
}

/// Independent source rebuild of every graded extension in a plan. The
/// production audit gets this for free by re-planning and comparing; a trusted
/// candidate must assert it explicitly. Read-only, no re-planning.
pub(crate) fn audit_extension_provenance(
    plan: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
) -> Result<Vec<String>, TerrainError> {
    let mut failures = vec![];
    for extension in &plan.extensions {
        if let Err(error) = super::road_alignment::verify(
            &extension.design,
            &extension.design.request,
            scene,
            natural,
            water,
            zones,
        ) {
            failures.push(format!(
                "extension {} source rebuild failed: {error}",
                extension.entity_id
            ));
        }
    }
    Ok(failures)
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
    trusted_record: bool,
    source: super::NaturalSource,
) -> Result<RoadNetworkAudit, TerrainError> {
    let expected = if trusted_record {
        record.clone()
    } else if let Some(style) = style {
        plan_with_style_declared(scene, natural, water, zones, program, style, source)?
    } else {
        plan_with_program(scene, natural, water, zones, program)?
    };

    if !trusted_record
        && serde_json::to_value(&expected).map_err(|e| TerrainError(e.to_string()))?
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
        // Four ranges, each with its own contract:
        //  * pavement     - the declared road width; held exactly;
        //  * graded band  - what `plan_terrain_carves` flattens; held exactly,
        //                   and it must cover the pavement;
        //  * transport    - pavement + the corridor's 3m shoulder + the end
        //    footprint      platforms; bounded by fill and no-excavation;
        //  * transition   - the carve's blend beyond it; only the fill envelope
        //                   applies, never the design level.
        let (hw, hd) = super::graded_transport_half_extents(e);
        // A structural shortfall is one failure per road, recorded before any
        // sampling, and such a road is never counted as realized.
        if planned.grounding.road_profile.is_some() {
            let (flat_hw, flat_hd) = super::graded_flat_top_half_extents(&planned);
            let flat_transverse = if planned.rotation_y_deg == 90. {
                flat_hw
            } else {
                flat_hd
            };
            let (pavement_hw, pavement_hd) = super::graded_pavement_half_extents(&planned);
            let pavement_transverse = if planned.rotation_y_deg == 90. {
                pavement_hw
            } else {
                pavement_hd
            };
            let shortfall =
                super::graded_band_shortfall_m(pavement_transverse, flat_transverse);
            if shortfall > 0 {
                audit.failures.push(format!(
                    "road {} graded band is {}m narrower than its declared pavement half",
                    planned.entity_id, shortfall
                ));
                continue;
            }
        }
        let mut error = 0_f32;
        let mut error_at = (e.world_x, e.world_z);
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
                    // The design level is required exactly where the earthwork
                    // flattens the road (`graded_flat_top_half_extents`); the
                    // rest of the transport footprint is audited as earthwork
                    // through the fill cap and the no-excavation rule below.
                    // Requiring the design level across the whole padded
                    // footprint graded the blend/apron as pavement, which no
                    // legal graded road on sloping ground can satisfy.
                    let (flat_hw, flat_hd) = super::graded_flat_top_half_extents(&planned);
                    (flat_hw, flat_hd)
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
                    let sample_error = (f - target as f32).abs();
                    if sample_error > error {
                        error = sample_error;
                        error_at = (x, z);
                    }
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
                "road {} changed after planning: design error {error}m at ({},{}), fill {fill}m, cut {cut}m",
                e.entity_id, error_at.0, error_at.1
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
        let precise = expected
            .roadside_sites
            .iter()
            .find(|s| s.id == planned.entity_id)
            .and_then(|s| s.support_level_mm);
        let mismatch_mm = precise.and_then(|level| {
            (e.world_z - hd - 2..=e.world_z + hd + 2).find_map(|z| {
                (e.world_x - hw - 2..=e.world_x + hw + 2).find_map(|x| {
                    let actual = final_terrain.height_millimetres(x, z);
                    (actual != Some(level)).then_some((x, z, level, actual))
                })
            })
        });
        let precision = if precise.is_some() { 0.0001 } else { 0.26 };
        if mismatch_mm.is_some()
            || error > precision
            || fill > expected.max_roadbed_fill_m + precision
            || cut > precision
        {
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
