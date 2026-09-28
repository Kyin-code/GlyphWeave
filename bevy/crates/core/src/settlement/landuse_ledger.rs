//! Area ledger for independently verified road-first geometry. Zoning purpose
//! and physical cover are orthogonal: a rural road is transport IN rural land.
//! No normalized entity proxy or reserved candidate earns realization credit.
//! Call after road/development/landcover reconstruction; this is not a safety
//! or ecology validator. Published reports are never acceptance authority.
use super::{
    development::{self, LandRole},
    landcover::{self, Material},
};
use crate::worldgen::{LandUseProfile, SceneIndex, SceneSpec};
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
    pub allocation_status: String,
    pub realization_status: String,
    pub status: String,
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
        }
    }
    for (id, size) in source_totals {
        if size > sources[id].boundary.area() as f64 + EPS {
            return Err("ledger source overcredited".into());
        }
    }
    for row in result.by_purpose.values_mut() {
        row.planned_scene_ratio = row.planned_m2 / area;
        row.realized_m2 = row.physical.total();
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
        let developed =
            ledger.by_purpose["urban"].realized_m2 + ledger.by_purpose["rural"].realized_m2;
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
    #[test]
    fn area_source_and_geometry_forgery_fail_closed() {
        let (scene, profile) = fixture();
        for mode in 0..4 {
            let mut bad = scene.clone();
            let patches = &mut bad.landcover_surfaces.as_mut().unwrap().patches;
            match mode {
                0 => patches[0].area_m2 += 1.,
                1 => patches[0].zone_kind = "nature".into(),
                2 => patches[0].source_cell_id = "forged".into(),
                _ => patches.push(patches[0].clone()),
            }
            assert!(analyze(&bad, &profile).is_err());
        }
        let mut missing = scene.clone();
        missing.landcover_surfaces = None;
        assert!(analyze(&missing, &profile).is_err());
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
