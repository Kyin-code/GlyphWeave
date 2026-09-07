//! GlyphWeave-original agricultural surface execution. Small rain-fed field
//! blocks and 4m farm-access lanes are sampled against final terrain. This is
//! bounded deterministic land preparation, not crop/irrigation simulation.
use super::farm_access;
use super::{
    development::{LandcoverPlan, LandcoverRole, Rect},
    landcover::{self, Material, SurfacePatch},
};
use crate::{
    baked_terrain::BakedTerrain,
    worldgen::{AccessSurface, EntityInstance, SceneSpec, WaterGeometry},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
const HEADLAND: i64 = 4;
const FIELD_GRADE: f64 = 0.12;
const TRACK_GRADE: f64 = 0.08;
const MAX_ACCESS_LENGTH: i64 = 160;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmField {
    pub id: String,
    pub source_cell_id: String,
    pub boundary: Rect,
    pub cultivated: Rect,
    pub road_id: String,
    pub access_path: Vec<[i64; 2]>,
    pub access_method: String,
    pub access_length_m: u64,
    pub access_search_expanded: usize,
    pub preparation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FarmRejection {
    pub source_cell_id: String,
    pub boundary: Rect,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgricultureRealization {
    pub model: String,
    pub fields: Vec<FarmField>,
    pub rejected_fields: Vec<FarmRejection>,
}
struct Index<'a> {
    rows: BTreeMap<i64, Vec<(i64, i64, &'a super::development::LandcoverCell)>>,
}
impl<'a> Index<'a> {
    fn new(plan: &'a LandcoverPlan) -> Self {
        let mut rows = BTreeMap::<_, Vec<_>>::new();
        for c in &plan.cells {
            if !matches!(
                c.role,
                LandcoverRole::AgricultureCandidate | LandcoverRole::FutureDevelopment
            ) {
                continue;
            }
            for z in c.boundary.z..c.boundary.z + i64::from(c.boundary.depth_m) {
                rows.entry(z).or_default().push((
                    c.boundary.x,
                    c.boundary.x + i64::from(c.boundary.width_m),
                    c,
                ));
            }
        }
        for row in rows.values_mut() {
            row.sort_by_key(|p| p.0);
        }
        Self { rows }
    }
    fn at(&self, x: i64, z: i64) -> Option<&'a super::development::LandcoverCell> {
        let row = self.rows.get(&z)?;
        let i = row.partition_point(|p| p.0 <= x);
        if i == 0 {
            return None;
        }
        let p = &row[i - 1];
        (x < p.1).then_some(p.2)
    }
}
fn contains(r: Rect, x: i64, z: i64) -> bool {
    x >= r.x && x < r.x + i64::from(r.width_m) && z >= r.z && z < r.z + i64::from(r.depth_m)
}
fn cells(r: Rect) -> impl Iterator<Item = (i64, i64)> {
    (r.z..r.z + i64::from(r.depth_m))
        .flat_map(move |z| (r.x..r.x + i64::from(r.width_m)).map(move |x| (x, z)))
}
fn road_rect(e: &EntityInstance) -> Option<Rect> {
    if e.kind != "road" || !e.width_m.is_finite() || !e.depth_m.is_finite() || e.scale != 1. {
        return None;
    }
    let (w, d) = match e.rotation_y_deg {
        0. | 180. => (e.width_m, e.depth_m),
        90. | 270. => (e.depth_m, e.width_m),
        _ => return None,
    };
    if w < 4.
        || d < 4.
        || w.fract() != 0.
        || d.fract() != 0.
        || w as u32 % 2 != 0
        || d as u32 % 2 != 0
    {
        return None;
    }
    Some(Rect {
        x: i64::from(e.world_x) - w as i64 / 2,
        z: i64::from(e.world_z) - d as i64 / 2,
        width_m: w as u32,
        depth_m: d as u32,
    })
}
fn grade(t: &BakedTerrain, x: i64, z: i64) -> Option<f64> {
    let x = i32::try_from(x).ok()?;
    let z = i32::try_from(z).ok()?;
    let [a, b, c, d] = [
        t.height(x, z)?,
        t.height(x.checked_add(1)?, z)?,
        t.height(x, z.checked_add(1)?)?,
        t.height(x.checked_add(1)?, z.checked_add(1)?)?,
    ]
    .map(f64::from);
    Some(
        (b - a)
            .abs()
            .max((d - c).abs())
            .hypot((c - a).abs().max((d - b).abs())),
    )
}
fn append_cell(patch: &mut SurfacePatch, t: &BakedTerrain, x: i64, z: i64) -> Result<(), String> {
    // Shared indexed metre vertices are intentionally deferred until the
    // chunked landcover format; full samples are never replaced by probes.
    let square = vec![
        [x as f64, z as f64],
        [x as f64 + 1., z as f64],
        [x as f64 + 1., z as f64 + 1.],
        [x as f64, z as f64 + 1.],
    ];
    landcover::append(patch, &square, |p| {
        landcover::terrain_height(t, p).map(|h| h + 0.03)
    })
}
fn patch(
    id: String,
    source: String,
    owner: String,
    zone: String,
    material: Material,
) -> SurfacePatch {
    SurfacePatch {
        id,
        source_cell_id: source,
        parcel_id: owner,
        zone_kind: zone,
        material,
        area_m2: 0.,
        vertices: vec![],
        triangles: vec![],
    }
}
fn path_cells(path: &[[i64; 2]]) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    for pair in path.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let r = if a[0] == b[0] {
            Rect {
                x: a[0] - 2,
                z: a[1].min(b[1]) - 2,
                width_m: 4,
                depth_m: (a[1] - b[1]).unsigned_abs() as u32 + 4,
            }
        } else {
            Rect {
                x: a[0].min(b[0]) - 2,
                z: a[1] - 2,
                width_m: (a[0] - b[0]).unsigned_abs() as u32 + 4,
                depth_m: 4,
            }
        };
        out.extend(cells(r));
    }
    out
}
/// All roads supplied here are final actual entities. Callers additionally
/// enforce the RoadNetworkPlan/final-raster contract before release.
pub fn realize(
    scene: &SceneSpec,
    plan: &LandcoverPlan,
    t: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    access: &[AccessSurface],
) -> Result<(AgricultureRealization, Vec<SurfacePatch>), String> {
    let pedestrian = super::pedestrian_index::PedestrianIndex::new(access)?;
    realize_with_query(scene,plan,t,water,entities,|x,z|pedestrian.overlaps_cell(x,z))
}
fn realize_with_query(
    scene:&SceneSpec,plan:&LandcoverPlan,t:&BakedTerrain,water:WaterGeometry,
    entities:&[EntityInstance],mut pedestrian_contains:impl FnMut(i64,i64)->bool,
) -> Result<(AgricultureRealization, Vec<SurfacePatch>), String> {
    if !t.matches_scene(scene) {
        return Err("agriculture terrain scene mismatch".into());
    }
    t.require_complete().map_err(|e| e.to_string())?;
    if plan.scene_id != scene.scene_id
        || plan.scene_area_m2 != u64::from(scene.width_m) * u64::from(scene.depth_m)
    {
        return Err("agriculture plan scene mismatch".into());
    }
    if let Some(pattern) = &plan.farm_pattern {
        pattern.validate()?;
    }
    let mut source_ids = BTreeSet::new();
    for c in &plan.cells {
        let r = c.boundary;
        if !source_ids.insert(&c.id)
            || r.width_m == 0
            || r.depth_m == 0
            || r.width_m > scene.width_m
            || r.depth_m > scene.depth_m
            || r.x < i64::from(scene.origin_x)
            || r.z < i64::from(scene.origin_z)
            || r.x > i64::from(scene.origin_x) + i64::from(scene.width_m) - i64::from(r.width_m)
            || r.z > i64::from(scene.origin_z) + i64::from(scene.depth_m) - i64::from(r.depth_m)
        {
            return Err("invalid agriculture source cell bounds/id".into());
        }
    }
    let index = Index::new(plan);
    for row in index.rows.values() {
        if row.windows(2).any(|p| p[0].1 > p[1].0) {
            return Err("overlapping agriculture candidate sources".into());
        }
    }
    let mut roads: Vec<_> = entities
        .iter()
        .filter_map(|e| road_rect(e).map(|r| (e, r)))
        .collect();
    roads.sort_by(|(a, _), (b, _)| a.entity_id.cmp(&b.entity_id));
    if roads
        .windows(2)
        .any(|p| p[0].0.entity_id == p[1].0.entity_id)
    {
        return Err("ambiguous agricultural road identity".into());
    }
    let solids: Vec<_> = entities.iter().filter(|e| landcover::solid(e)).collect();
    // Structured natural water is not the only reserved feature: authored
    // canals, parks and existing fields remain authoritative entity footprints.
    let water_features: Vec<_> = entities
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "canal" | "river" | "lake" | "water"))
        .collect();
    let reserved_features: Vec<_> = entities
        .iter()
        .filter(|e| {
            matches!(
                e.kind.as_str(),
                "park"
                    | "green_space"
                    | "nature_reserve"
                    | "mountain_forest"
                    | "farmland"
                    | "pasture"
            )
        })
        .collect();
    // Cache whole-cell feasibility, reused by competing routes and fields.
    let mut facts = BTreeMap::<(i64, i64), Result<f64, &'static str>>::new();
    let mut inspect = |x: i64, z: i64| -> Result<f64, &'static str> {
        *facts.entry((x, z)).or_insert_with(|| {
            if water.intersects_axis_aligned(x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5) {
                return Err("water_intersection");
            }
            if water_features
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("existing_water_feature_intersection");
            }
            if reserved_features
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("existing_land_use_reserved");
            }
            if solids
                .iter()
                .any(|e| landcover::obstacle_intersects(e, x, z))
            {
                return Err("solid_object_intersection");
            }
            if pedestrian_contains(x,z)
            {
                return Err("pedestrian_access_reserved");
            }
            grade(t, x, z).ok_or("missing_final_terrain_sample")
        })
    };
    let mut result = AgricultureRealization {
        model: "explicit-rectangular-field-access-v3".into(),
        ..Default::default()
    };
    let mut patches = vec![];
    let mut claimed_fields = BTreeSet::new();
    let mut claimed_tracks = BTreeMap::<(i64, i64), String>::new();
    let mut candidates = vec![];
    for cell in plan
        .cells
        .iter()
        .filter(|c| c.role == LandcoverRole::AgricultureCandidate)
    {
        let template = plan
            .farm_pattern
            .as_ref()
            .map(|p| p.for_zone(&cell.zone_id))
            .unwrap_or_default();
        template.validate()?;
        let width = i64::from(template.width_m);
        let depth = i64::from(template.depth_m);
        if plan.farm_pattern.is_some()
            && (i64::from(cell.boundary.width_m) < width + 4
                || i64::from(cell.boundary.depth_m) < depth + 4)
        {
            result.rejected_fields.push(FarmRejection {
                source_cell_id: cell.id.clone(),
                boundary: cell.boundary,
                reason: "source_too_small_for_requested_field_and_margin".into(),
            });
        }
        // Leave at least 2m around source boundaries, and 4m between blocks.
        for z in (cell.boundary.z + 2
            ..cell.boundary.z + i64::from(cell.boundary.depth_m) - depth - 1)
            .step_by((depth + 4) as usize)
        {
            for x in (cell.boundary.x + 2
                ..cell.boundary.x + i64::from(cell.boundary.width_m) - width - 1)
                .step_by((width + 4) as usize)
            {
                let center = [x + width / 2, z + depth / 2];
                let distance = roads
                    .iter()
                    .map(|(_, r)| {
                        (center[0] - center[0].clamp(r.x, r.x + i64::from(r.width_m))).abs()
                            + (center[1] - center[1].clamp(r.z, r.z + i64::from(r.depth_m))).abs()
                    })
                    .min()
                    .unwrap_or(i64::MAX);
                candidates.push((distance, z, x, cell));
            }
        }
    }
    candidates.sort_by_key(|(distance, z, x, _)| (*distance, *z, *x));
    let targets: Vec<_> = roads
        .iter()
        .map(|(e, r)| farm_access::Target {
            road_id: e.entity_id.clone(),
            boundary: *r,
        })
        .collect();
    // Finish the existing direct-access pass first. Detours may add fields but
    // cannot displace already accepted fields or their access/ownership.
    let mut queue: VecDeque<_> = candidates
        .into_iter()
        .map(|(d, z, x, c)| (d, z, x, c, false))
        .collect();
    while let Some((distance, z, x, cell, detour_pass)) = queue.pop_front() {
        let template = plan
            .farm_pattern
            .as_ref()
            .map(|p| p.for_zone(&cell.zone_id))
            .unwrap_or_default();
        let width = i64::from(template.width_m);
        let depth = i64::from(template.depth_m);
        let bounds = Rect {
            x,
            z,
            width_m: template.width_m,
            depth_m: template.depth_m,
        };
        let reject = |reason: &str| FarmRejection {
            source_cell_id: cell.id.clone(),
            boundary: bounds,
            reason: reason.into(),
        };
        if distance > MAX_ACCESS_LENGTH + width.max(depth) {
            result
                .rejected_fields
                .push(reject("no_road_within_direct_access_budget"));
            continue;
        }
        let mut failure = None;
        for (xx, zz) in cells(bounds) {
            if claimed_tracks.contains_key(&(xx, zz)) || claimed_fields.contains(&(xx, zz)) {
                failure = Some("reserved_field_or_track");
                break;
            }
            match inspect(xx, zz) {
                Err(reason) => {
                    failure = Some(reason);
                    break;
                }
                Ok(g) if g > FIELD_GRADE + 1e-9 => {
                    failure = Some("field_grade_exceeds_12_percent");
                    break;
                }
                _ => {}
            }
        }
        if let Some(reason) = failure {
            result.rejected_fields.push(reject(reason));
            continue;
        }
        let cultivated = Rect {
            x: x + HEADLAND,
            z: z + HEADLAND,
            width_m: template.width_m - 2 * HEADLAND as u32,
            depth_m: template.depth_m - 2 * HEADLAND as u32,
        };
        let starts = [
            [x + width / 2, z + 2],
            [x + width - 2, z + depth / 2],
            [x + width / 2, z + depth - 2],
            [x + 2, z + depth / 2],
        ];
        let mut routes = vec![];
        for (e, r) in roads.iter().filter(|_| !detour_pass) {
            for start in starts {
                let end = [
                    start[0].clamp(r.x + 2, r.x + i64::from(r.width_m) - 2),
                    start[1].clamp(r.z + 2, r.z + i64::from(r.depth_m) - 2),
                ];
                let length = (start[0] - end[0]).abs() + (start[1] - end[1]).abs();
                if length == 0 || length > MAX_ACCESS_LENGTH {
                    continue;
                }
                for bend in [[end[0], start[1]], [start[0], end[1]]] {
                    let mut path = vec![start, bend, end];
                    path.dedup();
                    routes.push((length, e.entity_id.clone(), path));
                }
            }
        }
        routes.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut accepted = None;
        let mut route_failures = BTreeSet::new();
        let mut access_search_expanded = 0;
        let mut check_cell = |xx: i64, zz: i64| -> Result<(), &'static str> {
            if roads.iter().any(|(_, r)| contains(*r, xx, zz)) {
                return Ok(());
            }
            if !contains(bounds, xx, zz) && index.at(xx, zz).is_none() {
                return Err("route_crosses_reserved_or_protected_land");
            }
            if contains(cultivated, xx, zz) {
                return Err("access_crosses_cultivated_interior");
            }
            if claimed_fields.contains(&(xx, zz)) {
                return Err("route_crosses_existing_field");
            }
            match inspect(xx, zz) {
                Err(reason) => Err(reason),
                Ok(g) if g > TRACK_GRADE + 1e-9 => Err("farm_access_grade_exceeds_8_percent"),
                _ => Ok(()),
            }
        };
        for (_, road_id, path) in routes {
            let track = path_cells(&path);
            let mut failure = None;
            for &(xx, zz) in &track {
                if let Err(reason) = check_cell(xx, zz) {
                    failure = Some(reason);
                    break;
                }
            }
            if let Some(reason) = failure {
                route_failures.insert(reason);
                continue;
            }
            accepted = Some((road_id, path, track));
            break;
        }
        if accepted.is_none() && !detour_pass {
            queue.push_back((distance, z, x, cell, true));
            continue;
        }
        if detour_pass {
            // Distant roads cannot be reached within the unchanged metric
            // budget; prune them once, not in every A* heuristic evaluation.
            let nearby_targets: Vec<_> = targets
                .iter()
                .filter(|t| {
                    starts
                        .iter()
                        .any(|&p| t.distance(p) <= MAX_ACCESS_LENGTH as u64)
                })
                .cloned()
                .collect();
            let search = farm_access::find(
                &starts,
                &nearby_targets,
                MAX_ACCESS_LENGTH as u64,
                4096,
                |[cx, cz]| {
                    // A centre is legal only when its entire 4m x 4m support is legal.
                    for zz in cz - 2..cz + 2 {
                        for xx in cx - 2..cx + 2 {
                            if let Err(reason) = check_cell(xx, zz) {
                                route_failures.insert(reason);
                                return false;
                            }
                        }
                    }
                    true
                },
            );
            access_search_expanded = search.expanded;
            if let Some(road_id) = search.road_id {
                let track = path_cells(&search.path);
                // Recheck the swept corridor, not just the search centres.
                for &(xx, zz) in &track {
                    check_cell(xx, zz)
                        .map_err(|r| format!("detour swept footprint mismatch: {r}"))?;
                }
                accepted = Some((road_id, search.path, track));
            } else {
                route_failures.insert(search.reason);
            }
        }
        let Some((road_id, path, track)) = accepted else {
            let reasons = route_failures.into_iter().collect::<Vec<_>>().join("|");
            result
                .rejected_fields
                .push(reject(&format!("no_verified_access:{reasons}")));
            continue;
        };
        let id = format!("{}.field.{x}.{z}", cell.id);
        let mut soil = patch(
            format!("{id}.soil"),
            cell.id.clone(),
            id.clone(),
            cell.zone_kind.clone(),
            Material::CultivatedSoil,
        );
        let mut headland = patch(
            format!("{id}.headland"),
            cell.id.clone(),
            id.clone(),
            cell.zone_kind.clone(),
            Material::FieldHeadland,
        );
        for (xx, zz) in cells(bounds) {
            claimed_fields.insert((xx, zz));
            if contains(cultivated, xx, zz) {
                append_cell(&mut soil, t, xx, zz)?
            } else {
                append_cell(&mut headland, t, xx, zz)?
            }
        }
        patches.extend([soil, headland]);
        let mut track_patches = BTreeMap::<String, SurfacePatch>::new();
        for (xx, zz) in track {
            if contains(bounds, xx, zz)
                || roads.iter().any(|(_, r)| contains(*r, xx, zz))
                || claimed_tracks.contains_key(&(xx, zz))
            {
                continue;
            }
            let src = index
                .at(xx, zz)
                .ok_or("accepted agricultural route lost ownership")?;
            let p = track_patches.entry(src.id.clone()).or_insert_with(|| {
                patch(
                    format!("{id}.track.{}", src.id),
                    src.id.clone(),
                    id.clone(),
                    src.zone_kind.clone(),
                    Material::FarmTrack,
                )
            });
            append_cell(p, t, xx, zz)?;
            claimed_tracks.insert((xx, zz), id.clone());
        }
        patches.extend(track_patches.into_values());
        result.fields.push(FarmField {
            id,
            source_cell_id: cell.id.clone(),
            boundary: bounds,
            cultivated,
            road_id,
            access_length_m: farm_access::length(&path),
            access_path: path,
            access_method: if detour_pass {
                "bounded_grid_detour"
            } else {
                "direct_axis_route"
            }
            .into(),
            access_search_expanded,
            preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
        });
    }
    Ok((result, patches))
}

#[cfg(test)]
mod tests {
    use super::super::pedestrian_index::triangle_overlaps_cell;
    use super::super::development;
    use super::*;
    use crate::worldgen::{
        GroundingSpec, LandUseZone, WaterKind,
        road_network::{RoadEdge, RoadNetworkPlan},
        water_geometry,
    };
    fn fixture() -> (
        SceneSpec,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "farms".into(),
            width_m: 160,
            depth_m: 160,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m: 160,
            source: "fixture".into(),
        }];
        let roads = RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: scene.scene_id.clone(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: Vec::new(),
            joint_layout: None,
            nodes: vec![],
            edges: vec![RoadEdge {
                id: "road".into(),
                from: 0,
                to: 1,
                world_x: 80,
                world_z: 80,
                length_m: 64,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
            }],
            roadside_sites: vec![],
        };
        let development = development::plan(&scene, &zones, &roads).unwrap();
        let plan = development::plan_landcover(&scene, &development).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 160, 160, &0_i16.to_le_bytes().repeat(160 * 160))
            .unwrap();
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        (scene, plan, terrain, water, roads.entities())
    }
    #[test]
    fn flat_fields_have_four_metre_access_and_disjoint_surface_ownership() {
        let (s, p, t, w, e) = fixture();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(!a.fields.is_empty());
        assert!(patches.iter().any(|p| p.material == Material::FarmTrack));
        let mut owned = BTreeSet::new();
        let mut area = 0.;
        for p in &patches {
            assert_eq!(p.zone_kind, "rural");
            for v in p.vertices.chunks_exact(4) {
                let x = v[0][0] as i64;
                let z = v[0][2] as i64;
                assert!(owned.insert((x, z)), "overlapping field/track cell");
                assert!(!contains(road_rect(&e[0]).unwrap(), x, z));
                assert!(v.iter().all(|v| (v[1] - 0.03).abs() < 1e-8));
            }
            area += p.area_m2;
        }
        assert_eq!(owned.len() as f64, area);
        for f in &a.fields {
            assert_eq!(f.road_id, "road");
            assert_eq!(f.boundary.area(), 576);
            assert_eq!(f.cultivated.area(), 256);
            assert_eq!(f.access_length_m, farm_access::length(&f.access_path));
            assert!(f.access_length_m <= MAX_ACCESS_LENGTH as u64);
            assert!(f.access_search_expanded <= 4096);
            assert!(
                f.access_path
                    .windows(2)
                    .all(|p| p[0][0] == p[1][0] || p[0][1] == p[1][1])
            );
            for (x, z) in path_cells(&f.access_path) {
                assert!(!contains(f.cultivated, x, z));
                assert!(owned.contains(&(x, z)) || contains(road_rect(&e[0]).unwrap(), x, z));
            }
        }
        let again = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert_eq!(a, again.0);
        assert_eq!(
            serde_json::to_value(patches).unwrap(),
            serde_json::to_value(again.1).unwrap()
        );
    }
    #[test]
    fn blocked_direct_routes_gain_real_four_metre_detours_but_not_through_narrow_gaps() {
        let (s, mut p, t, w, mut e) = fixture();
        let mut field = p.cells[0].clone();
        field.id = "field-source".into();
        field.role = LandcoverRole::AgricultureCandidate;
        field.boundary = Rect {
            x: 60,
            z: 110,
            width_m: 28,
            depth_m: 28,
        };
        let mut connector = field.clone();
        connector.id = "connector".into();
        connector.role = LandcoverRole::FutureDevelopment;
        connector.boundary = Rect {
            x: 0,
            z: 84,
            width_m: 160,
            depth_m: 26,
        };
        p.cells = vec![field, connector];
        let mut barrier = e[0].clone();
        barrier.entity_id = "barrier".into();
        barrier.kind = "rock".into();
        barrier.world_x = 74;
        barrier.world_z = 96;
        barrier.width_m = 32.;
        barrier.depth_m = 8.;
        e.push(barrier.clone());
        let before = t.content_hash().unwrap();
        // All original straight/L candidates cross this barrier.
        for start in [[74, 114], [84, 124], [74, 134], [64, 124]] {
            assert!(
                path_cells(&[start, [start[0], 82]])
                    .iter()
                    .any(|&(x, z)| landcover::obstacle_intersects(&barrier, x, z))
            );
        }
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert_eq!(a.fields.len(), 1);
        let f = &a.fields[0];
        assert_eq!(f.access_method, "bounded_grid_detour");
        assert!(f.access_search_expanded > 0 && f.access_search_expanded <= 4096);
        assert!(f.access_path.len() > 2);
        assert_eq!(f.access_length_m, farm_access::length(&f.access_path));
        assert!(f.access_length_m <= 160);
        let mut covered = BTreeSet::new();
        for patch in &patches {
            for vertices in patch.vertices.chunks_exact(4) {
                let x = vertices[0][0] as i64;
                let z = vertices[0][2] as i64;
                assert!(covered.insert((x, z)), "overlapping published surfaces");
                assert!(!landcover::obstacle_intersects(&barrier, x, z));
            }
        }
        assert!(patches.iter().any(|p| p.material == Material::FarmTrack));
        for (x, z) in path_cells(&f.access_path) {
            assert!(!contains(f.cultivated, x, z));
            assert!(covered.contains(&(x, z)) || contains(road_rect(&e[0]).unwrap(), x, z));
        }
        assert_eq!(before, t.content_hash().unwrap());
        assert_eq!(a, realize(&s, &p, &t, w, &e, &[]).unwrap().0);
        e[1].world_x = 80;
        e[1].width_m = 160.;
        let blocked = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(blocked.0.fields.is_empty());
        assert!(blocked.1.is_empty());
        // A 2m gap is not a legal 4m machinery corridor.
        e[1].world_x = 38;
        e[1].width_m = 76.;
        let mut right = e[1].clone();
        right.entity_id = "right-barrier".into();
        right.world_x = 119;
        right.width_m = 82.;
        e.push(right);
        let narrow = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(narrow.0.fields.is_empty());
        assert!(narrow.1.is_empty());
    }

    #[test]
    fn explicit_rectangles_have_exact_area_and_never_fallback_to_smaller_fields() {
        use super::super::farm_program::{FarmPattern, FieldTemplate};
        let (s, mut plan, t, w, e) = fixture();
        plan.farm_pattern = Some(FarmPattern {
            default_field: FieldTemplate {
                width_m: 48,
                depth_m: 32,
            },
            zone_fields: BTreeMap::new(),
        });
        let (result, patches) = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(!result.fields.is_empty());
        for field in &result.fields {
            assert_eq!((field.boundary.width_m, field.boundary.depth_m), (48, 32));
            assert_eq!(
                (field.cultivated.width_m, field.cultivated.depth_m),
                (40, 24)
            );
            assert_eq!(field.cultivated.area(), 960);
            assert_eq!(field.boundary.area() - field.cultivated.area(), 576);
            assert_eq!(
                field.access_length_m,
                farm_access::length(&field.access_path)
            );
            assert!(field.access_length_m <= 160);
        }
        assert_eq!(
            patches
                .iter()
                .filter(|p| p.material == Material::CultivatedSoil)
                .map(|p| p.area_m2)
                .sum::<f64>(),
            result.fields.len() as f64 * 960.
        );
        let before = t.content_hash().unwrap();
        plan.farm_pattern.as_mut().unwrap().default_field = FieldTemplate {
            width_m: 128,
            depth_m: 128,
        };
        let blocked = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(blocked.0.fields.is_empty());
        assert!(blocked.1.is_empty());
        assert!(
            blocked
                .0
                .rejected_fields
                .iter()
                .any(|r| r.reason == "source_too_small_for_requested_field_and_margin")
        );
        assert_eq!(before, t.content_hash().unwrap());
        plan.farm_pattern.as_mut().unwrap().default_field.width_m = 17;
        assert!(realize(&s, &plan, &t, w, &e, &[]).is_err());
    }

    #[test]
    fn zone_field_overrides_do_not_change_other_zones_or_ignore_requested_sizes() {
        use super::super::farm_program::{FarmPattern, FieldTemplate};
        let (s, mut plan, t, w, e) = fixture();
        let mut small_zone = None;
        for cell in &mut plan.cells {
            if cell.boundary.z >= 84 {
                cell.zone_id = "south".into();
                small_zone = Some(cell.zone_id.clone());
            }
        }
        plan.farm_pattern = Some(FarmPattern {
            default_field: FieldTemplate {
                width_m: 48,
                depth_m: 32,
            },
            zone_fields: BTreeMap::from([(
                small_zone.unwrap(),
                FieldTemplate {
                    width_m: 24,
                    depth_m: 24,
                },
            )]),
        });
        let (result, _) = realize(&s, &plan, &t, w, &e, &[]).unwrap();
        assert!(result.fields.iter().any(|f| f.boundary.width_m == 48));
        assert!(result.fields.iter().any(|f| f.boundary.width_m == 24));
        for f in &result.fields {
            let source = plan
                .cells
                .iter()
                .find(|c| c.id == f.source_cell_id)
                .unwrap();
            let template = plan
                .farm_pattern
                .as_ref()
                .unwrap()
                .for_zone(&source.zone_id);
            assert_eq!(f.boundary.width_m, template.width_m);
            assert_eq!(f.boundary.depth_m, template.depth_m);
        }
    }

    #[test]
    fn absence_of_roads_does_not_become_a_fake_agricultural_access() {
        let (s, p, t, w, _) = fixture();
        let (a, patches) = realize(&s, &p, &t, w, &[], &[]).unwrap();
        assert!(a.fields.is_empty());
        assert!(patches.is_empty());
        assert!(!a.rejected_fields.is_empty());
    }
    #[test]
    fn steep_terrain_water_and_existing_solids_remain_unmodified() {
        let (s, p, _, w, e) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        let bytes: Vec<_> = (0..160)
            .flat_map(|_| (0..160).flat_map(|x| (x as i16).to_le_bytes()))
            .collect();
        t.insert_chunk(0, 0, 160, 160, &bytes).unwrap();
        let before = t.content_hash().unwrap();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "field_grade_exceeds_12_percent")
        );
        assert_eq!(before, t.content_hash().unwrap());
        let (s, p, t, _, e) = fixture();
        let lake:crate::worldgen::LandmarkSpec=serde_json::from_value(serde_json::json!({
            "entityId":"lake","name":"lake","type":"lake","purpose":"test","description":"water fixture","sceneId":"farms","worldX":80,"worldZ":80,"widthM":400,"depthM":400,"heightM":1,"assetId":"prop.lake"
        })).unwrap();
        let w = water_geometry(WaterKind::Lake, &[lake], &s, &serde_json::Value::Null);
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "water_intersection")
        );
        let (s, p, t, w, mut e) = fixture();
        e.push(EntityInstance {
            entity_id: "existing-rock".into(),
            asset_id: "prop.rock".into(),
            kind: "rock".into(),
            world_x: 80,
            world_z: 80,
            world_y: 0,
            width_m: 300.,
            depth_m: 300.,
            height_m: 4.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        });
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason == "solid_object_intersection")
        );
        assert_eq!(e.len(), 2);
    }
    #[test]
    fn protected_strip_cannot_be_crossed_to_reach_a_nearby_road() {
        let (s, mut p, t, w, e) = fixture();
        p.cells.retain(|c| c.boundary.z >= 84);
        for c in &mut p.cells {
            c.boundary.z += 10;
            c.boundary.depth_m -= 10;
        }
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason.contains("reserved_or_protected"))
        );
    }
    #[test]
    fn rejected_access_does_not_claim_flat_fields_behind_a_steep_barrier() {
        let (s, mut p, _, w, e) = fixture();
        p.cells.retain(|c| c.boundary.z >= 84);
        for c in &mut p.cells {
            c.boundary.z += 8;
            c.boundary.depth_m -= 8;
        }
        // A legal candidate connector strip, too steep for farm machinery.
        let mut strip = p.cells[0].clone();
        strip.id = "connector".into();
        strip.role = LandcoverRole::FutureDevelopment;
        strip.boundary = Rect {
            x: 0,
            z: 84,
            width_m: 160,
            depth_m: 8,
        };
        p.cells.push(strip);
        let mut bytes = 0_i16.to_le_bytes().repeat(160 * 160);
        for z in [86, 87] {
            for x in 0..160 {
                let i = (z * 160 + x) * 2;
                bytes[i..i + 2].copy_from_slice(&4_i16.to_le_bytes());
            }
        }
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(0, 0, 160, 160, &bytes).unwrap();
        let (a, patches) = realize(&s, &p, &t, w, &e, &[]).unwrap();
        assert!(a.fields.is_empty());
        assert!(patches.is_empty());
        assert!(
            a.rejected_fields
                .iter()
                .any(|r| r.reason.contains("farm_access_grade_exceeds_8_percent"))
        );
    }
    #[test]
    fn existing_canal_park_and_authored_farm_are_not_overwritten() {
        let (s, p, t, w, e) = fixture();
        for kind in ["canal", "green_space", "farmland", "pasture"] {
            let mut entities = e.clone();
            let mut reserved = e[0].clone();
            reserved.entity_id = format!("authored.{kind}");
            reserved.kind = kind.into();
            reserved.width_m = 160.;
            reserved.depth_m = 160.;
            entities.push(reserved);
            let (a, patches) = realize(&s, &p, &t, w, &entities, &[]).unwrap();
            assert!(patches.is_empty());
            assert!(a.fields.is_empty());
            assert!(a.rejected_fields.iter().any(|r| r.reason
                == if kind == "canal" {
                    "existing_water_feature_intersection"
                } else {
                    "existing_land_use_reserved"
                }));
        }
    }

    #[test]
    fn duplicate_roads_overlapping_sources_and_invalid_bounds_fail() {
        let (s, p, t, w, mut e) = fixture();
        e.push(e[0].clone());
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
        let (s, mut p, t, w, e) = fixture();
        let mut duplicate = p.cells[0].clone();
        duplicate.id = "another-id".into();
        p.cells.push(duplicate);
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
        let (s, mut p, t, w, e) = fixture();
        p.cells[0].boundary.x = i64::MAX;
        assert!(realize(&s, &p, &t, w, &e, &[]).is_err());
    }

    #[test]
    fn narrow_pedestrian_triangle_is_detected_between_cell_centres() {
        let t = [[0.9, 0., 0.], [1.1, 0., 0.], [0.9, 0., 1.]];
        assert!(triangle_overlaps_cell(t, 0, 0));
        assert!(triangle_overlaps_cell(t, 1, 0));
        assert!(!triangle_overlaps_cell(t, 2, 0));
    }
    #[test]
    fn indexed_and_linear_complete_farm_outputs_are_identical() {
        let(s,p,t,w,e)=fixture();
        let access=vec![AccessSurface{version:1,entity_id:"pedestrian".into(),anchor_id:"front".into(),road_id:"road".into(),width_m:1.5,
            vertices:vec![[0.,0.,44.],[159.,0.,44.75],[0.,0.,45.5],[32.,0.,0.],[33.,0.,159.],[34.,0.,0.]],triangles:vec![[0,1,2],[3,5,4]]}];
        let index=super::super::pedestrian_index::PedestrianIndex::new(&access).unwrap();
        let reference=realize_with_query(&s,&p,&t,w,&e,|x,z|index.linear_reference(x,z)).unwrap();
        let optimized=realize(&s,&p,&t,w,&e,&access).unwrap();
        assert!(serde_json::to_vec(&reference).unwrap()==serde_json::to_vec(&optimized).unwrap());
        assert!(!optimized.0.fields.is_empty());assert!(!optimized.0.rejected_fields.is_empty());
    }
    #[test]
    #[ignore = "explicit full-world same-input indexed/reference benchmark; requires completed world and fresh report path"]
    fn benchmark_pedestrian_index_on_published_world() {
        use crate::worldgen::{WorldManifest,SceneIndex,build_access_graph,bake_access_surfaces};
        let root=std::path::PathBuf::from(std::env::var_os("GLYPHWEAVE_BENCH_WORLD").expect("set completed world path"));
        assert!(root.join("world.json").is_file(),"partial bake is not a published benchmark source");
        let report=std::path::PathBuf::from(std::env::var_os("GLYPHWEAVE_BENCH_REPORT").expect("set fresh report path"));
        assert!(!report.exists(),"keep earlier evidence");
        let setup=std::time::Instant::now();
        let manifest:WorldManifest=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(root.join("glyphweave.manifest.json")).unwrap())).unwrap();
        let spec=&manifest.scenes[0];let folder=root.join("scenes").join(&spec.scene_id);
        let scene:SceneIndex=serde_json::from_reader(std::io::BufReader::with_capacity(1024*1024,std::fs::File::open(folder.join("scene.json")).unwrap())).unwrap();
        let mut terrain=BakedTerrain::new(spec).unwrap();
        for c in &scene.chunks {terrain.insert_chunk(c.world_x,c.world_z,c.valid_width_m,c.valid_depth_m,&std::fs::read(folder.join(&c.height_file)).unwrap()).unwrap();}
        terrain.require_complete().unwrap();
        let water=water_geometry(super::super::super::worldgen::water_kind(&manifest.style),&manifest.landmarks,spec,&manifest.style);
        let graph=build_access_graph(&scene.entities,&terrain,water);
        let access=bake_access_surfaces(&graph,&terrain).unwrap();
        let plan=scene.landcover_plan.as_ref().unwrap();
        let index=super::super::pedestrian_index::PedestrianIndex::new(&access).unwrap();
        let setup_seconds=setup.elapsed().as_secs_f64();
        eprintln!("reference benchmark inputs loaded in {setup_seconds}s; starting linear agriculture");
        let start=std::time::Instant::now();
        let old=realize_with_query(spec,plan,&terrain,water,&scene.entities,|x,z|index.linear_reference(x,z)).unwrap();
        let reference_seconds=start.elapsed().as_secs_f64();
        eprintln!("linear agriculture complete in {reference_seconds}s; starting indexed agriculture");
        let start=std::time::Instant::now();let new=realize(spec,plan,&terrain,water,&scene.entities,&access).unwrap();let indexed_seconds=start.elapsed().as_secs_f64();
        let old_bytes=serde_json::to_vec(&old).unwrap();let new_bytes=serde_json::to_vec(&new).unwrap();
        let saved=scene.landcover_surfaces.as_ref().unwrap();
        let saved_patches:Vec<_>=saved.patches.iter().filter(|p|matches!(p.material,Material::CultivatedSoil|Material::FieldHeadland|Material::FarmTrack)).collect();
        let saved_bytes=serde_json::to_vec(&(&saved.agriculture,saved_patches)).unwrap();
        let result=serde_json::json!({"format":"glyphweave.agriculture-index-reference","status":if old_bytes==new_bytes&&new_bytes==saved_bytes {"pass"} else {"fail"},"scope":"same complete agriculture fields/rejections/routes/meshes; not full-world time SLA","world":root,"sceneId":spec.scene_id,"sizeM":[spec.width_m,spec.depth_m],"entities":scene.entities.len(),"pedestrianTriangles":access.iter().map(|s|s.triangles.len()).sum::<usize>(),"fields":new.0.fields.len(),"rejections":new.0.rejected_fields.len(),"patches":new.1.len(),"setupSeconds":setup_seconds,"referenceSeconds":reference_seconds,"indexedSeconds":indexed_seconds,"serializedBytes":new_bytes.len(),"referenceHash":blake3::hash(&old_bytes).to_hex().to_string(),"indexedHash":blake3::hash(&new_bytes).to_hex().to_string(),"publishedHash":blake3::hash(&saved_bytes).to_hex().to_string(),"naturalHash":scene.road_network_plan.as_ref().unwrap().natural_heightfield_hash,"finalTerrainHash":terrain.content_hash().unwrap()});
        std::io::Write::write_all(&mut std::fs::OpenOptions::new().write(true).create_new(true).open(report).unwrap(),&serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        eprintln!("{}",result);
        assert!(old_bytes==new_bytes,"indexed results differ from original linear predicate");
        assert!(new_bytes==saved_bytes,"published agriculture differs from reconstruction");
    }

}
