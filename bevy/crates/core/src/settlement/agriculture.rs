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
    /// Present only when the lane ends on an already realized farm track
    /// instead of a road footprint. Absent (and therefore absent from the
    /// published bytes) for every world that never enabled shared tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_track: Option<SharedTrackJoin>,
}
/// One lane that joins an already realized farm track. `road_distance_m` is the
/// length that track already spends between the join square and the nearest
/// real road, so `access_length_m + road_distance_m` is the whole unchanged
/// budget this field consumes: a chain is bounded in metres, never in hops.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedTrackJoin {
    /// Field whose realized lane is reused.
    pub field_id: String,
    /// The 1m farm-track square this lane ends on.
    pub cell: [i64; 2],
    /// Metres that square already spends to reach a real road.
    pub road_distance_m: u64,
    /// How many realized lanes stand between this field and that road. The
    /// metre budget above is what actually bounds the chain.
    pub chain_lanes: u32,
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
/// Ownership of one realized 1m farm-track square. The road distance is
/// cumulative, so a chain of lanes can never hide metres behind a hop count,
/// and the owning field is unique because a square is only ever lain once.
#[derive(Debug, Clone)]
struct TrackClaim {
    field_id: String,
    road_id: String,
    road_distance_m: u64,
    chain_lanes: u32,
}
/// Metres from every square covered by `path` back to its far end. Taking the
/// minimum over the path vertices keeps the number exact on the centreline and
/// over-counts by at most the 2m corridor half-width beside it, so a lane is
/// never credited with a shorter chain than the one it physically travels.
fn lane_distances(path: &[[i64; 2]]) -> BTreeMap<(i64, i64), u64> {
    let mut remaining = vec![0_u64; path.len()];
    for i in (0..path.len().saturating_sub(1)).rev() {
        remaining[i] = remaining[i + 1]
            + path[i][0].abs_diff(path[i + 1][0])
            + path[i][1].abs_diff(path[i + 1][1]);
    }
    let mut out = BTreeMap::new();
    for cell in path_cells(path) {
        let distance = path
            .iter()
            .zip(&remaining)
            .map(|(v, r)| r + cell.0.abs_diff(v[0]) + cell.1.abs_diff(v[1]))
            .min()
            .unwrap_or(0);
        out.insert(cell, distance);
    }
    out
}
/// Cheapest whole path from these field edges onto any already realized farm
/// track: the straight line to a square plus the metres that square already
/// spends back to a real road. It is a lower bound on the real lane, so when it
/// overruns the budget the chain is over budget however it is laid.
fn nearest_shared_track(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
) -> Option<u64> {
    let mut nearest = None::<u64>;
    for (&(x, zz), claim) in claimed_tracks {
        for start in starts {
            let straight = start[0].abs_diff(x) + start[1].abs_diff(zz);
            if straight > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            let bound = straight + claim.road_distance_m;
            nearest = Some(nearest.map_or(bound, |n: u64| n.min(bound)));
        }
    }
    nearest
}
/// The realized squares a spur is worth aiming at: the cheapest whole paths
/// from these field edges. Aiming at a bounded offer set keeps the spur search
/// as cheap as the road search it complements, and a blocked first choice still
/// has neighbours to fall back on instead of failing the whole field.
const SHARED_TRACK_TERMINALS: usize = 32;
fn shared_track_terminals(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
) -> Vec<([i64; 2], u64)> {
    // The road search steps 2m at a time from field edges that all share one
    // parity, so a square on the other parity could never be a goal: it is not
    // offered, and the offer set stays honest about what the search can reach.
    let Some(parity) = starts.first() else {
        return vec![];
    };
    let mut bounds = BTreeMap::<[i64; 2], u64>::new();
    for (&(x, zz), claim) in claimed_tracks {
        if x.rem_euclid(2) != parity[0].rem_euclid(2)
            || zz.rem_euclid(2) != parity[1].rem_euclid(2)
        {
            continue;
        }
        for start in starts {
            let straight = start[0].abs_diff(x) + start[1].abs_diff(zz);
            if straight > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            let bound = straight + claim.road_distance_m;
            // Squares already over the budget cannot become a legal goal, so
            // they never enter the offer set.
            if bound > MAX_ACCESS_LENGTH as u64 {
                continue;
            }
            bounds
                .entry([x, zz])
                .and_modify(|best| *best = (*best).min(bound))
                .or_insert(bound);
        }
    }
    let mut ordered: Vec<([i64; 2], u64)> = bounds.into_iter().collect();
    ordered.sort_by_key(|&(cell, bound)| (bound, cell));
    ordered.truncate(SHARED_TRACK_TERMINALS);
    ordered
        .into_iter()
        .map(|(cell, _)| (cell, claimed_tracks[&(cell[0], cell[1])].road_distance_m))
        .collect()
}
/// Joins an already realized farm track instead of demanding a fresh lane all
/// the way to a road, which is what a real field road network does: one lane
/// serves several fields. The whole path back to the nearest real road stays on
/// the clock, so the chain is bounded in metres, never in hops.
fn join_realized_track(
    claimed_tracks: &BTreeMap<(i64, i64), TrackClaim>,
    starts: &[[i64; 2]],
    max_expanded: usize,
    failures: &mut BTreeSet<&'static str>,
    check_cell: &mut impl FnMut(i64, i64) -> Result<(), &'static str>,
) -> Result<
    Option<(
        String,
        Vec<[i64; 2]>,
        BTreeSet<(i64, i64)>,
        SharedTrackJoin,
        usize,
    )>,
    String,
> {
    let terminals = shared_track_terminals(claimed_tracks, starts);
    let spur = farm_access::find_spur(
        starts,
        &terminals,
        MAX_ACCESS_LENGTH as u64,
        max_expanded,
        |[cx, cz]| {
            // A centre is legal only when its entire 4m x 4m support is legal,
            // exactly the rule the road search applies to its own centres.
            let mut legal = true;
            for zz in cz - 2..cz + 2 {
                for xx in cx - 2..cx + 2 {
                    if let Err(reason) = check_cell(xx, zz) {
                        failures.insert(reason);
                        legal = false;
                    }
                }
            }
            legal
        },
    );
    if spur.path.is_empty() {
        match nearest_shared_track(claimed_tracks, starts) {
            // No realized lane is even in range: keep the older evidence.
            None => {}
            // Near, but every whole path back to a road overruns the budget.
            Some(nearest) if nearest > MAX_ACCESS_LENGTH as u64 => {
                failures.insert("access_via_existing_track_exceeds_budget");
            }
            Some(_) => {
                failures.insert(spur.reason);
            }
        }
        return Ok(None);
    }
    let centre = *spur.path.last().expect("a spur path is nonempty");
    let join = spur
        .join
        .ok_or("shared farm track spur ended without a join")?;
    // Extend the centreline onto the square it joins, so the published lane
    // ends exactly there and its length is the whole path it travels.
    for bend in [[join[0], centre[1]], [centre[0], join[1]]] {
        let mut path = spur.path.clone();
        path.push(bend);
        path.push(join);
        path.dedup();
        let claim = &claimed_tracks[&(join[0], join[1])];
        if farm_access::length(&path) + claim.road_distance_m > MAX_ACCESS_LENGTH as u64 {
            continue;
        }
        let corridor = path_cells(&path);
        let mut failure = None;
        for &(xx, zz) in &corridor {
            // Recheck the swept corridor, not just the search centres.
            if let Err(reason) = check_cell(xx, zz) {
                failure = Some(reason);
                break;
            }
        }
        if let Some(reason) = failure {
            failures.insert(reason);
            continue;
        }
        return Ok(Some((
            claim.road_id.clone(),
            path,
            corridor,
            SharedTrackJoin {
                field_id: claim.field_id.clone(),
                cell: join,
                road_distance_m: claim.road_distance_m,
                chain_lanes: claim.chain_lanes + 1,
            },
            spur.expanded,
        )));
    }
    Ok(None)
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
    let track_network = plan
        .farm_pattern
        .as_ref()
        .and_then(|p| p.track_network.as_ref());
    let mut result = AgricultureRealization {
        // The model states which access rule produced this scene, so a world
        // that shares realized tracks cannot claim the road-only model.
        model: if track_network.is_some() {
            "explicit-rectangular-field-access-v4-shared-tracks"
        } else {
            "explicit-rectangular-field-access-v3"
        }
        .into(),
        ..Default::default()
    };
    let mut patches = vec![];
    let mut claimed_fields = BTreeSet::new();
    let mut claimed_tracks = BTreeMap::<(i64, i64), TrackClaim>::new();
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
        let mut accepted: Option<(
            String,
            Vec<[i64; 2]>,
            BTreeSet<(i64, i64)>,
            Option<SharedTrackJoin>,
        )> = None;
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
            accepted = Some((road_id, path, track, None));
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
                accepted = Some((road_id, search.path, track, None));
            } else {
                route_failures.insert(search.reason);
                // A road route must be proven impossible first, so every field
                // this rule already accepted keeps its exact lane geometry and
                // shared tracks can only add fields, never rewrite one.
                if track_network.is_some() {
                    match join_realized_track(
                        &claimed_tracks,
                        &starts,
                        4096,
                        &mut route_failures,
                        &mut check_cell,
                    )? {
                        Some((road_id, path, corridor, join, expanded)) => {
                            accepted = Some((road_id, path, corridor, Some(join)));
                            // The accepted lane's own search is the evidence the
                            // expansion budget bounds, as for every other lane.
                            access_search_expanded = expanded;
                        }
                        None => {}
                    }
                }
            }
        }
        let Some((road_id, path, track, join)) = accepted else {
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
        // A lane that joins an existing track inherits everything that track
        // already spent, so the square it lays must carry the whole sum.
        let spent_m = join.as_ref().map_or(0, |j| j.road_distance_m);
        // Squares laid by this lane are one shared join further from the road
        // than the join itself, so the next lane's chain grows by exactly one.
        let chain_lanes = join.as_ref().map_or(0, |j| j.chain_lanes);
        let lane = lane_distances(&path);
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
            claimed_tracks.insert(
                (xx, zz),
                TrackClaim {
                    field_id: id.clone(),
                    road_id: road_id.clone(),
                    road_distance_m: spent_m
                        + lane
                            .get(&(xx, zz))
                            .copied()
                            .ok_or("accepted agricultural lane lost its own distance")?,
                    chain_lanes,
                },
            );
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
            access_method: if join.is_some() {
                "shared_farm_track_spur"
            } else if detour_pass {
                "bounded_grid_detour"
            } else {
                "direct_axis_route"
            }
            .into(),
            access_search_expanded,
            preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
            shared_track: join,
        });
    }
    Ok((result, patches))
}

/// Ownership of one realized 1m farm-track square, rebuilt from the published
/// lanes alone. `road_distance_m` is the whole distance that square spends back
/// to a real road, so an auditor can re-check a chain in metres without
/// trusting the generator's own bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOwnership {
    pub field_id: String,
    pub road_id: String,
    pub road_distance_m: u64,
    pub chain_lanes: u32,
}

/// Rebuilds the realized farm-track ownership from the published lanes alone
/// and re-checks every access budget, join and chain. Nothing here trusts a
/// recorded number: the claim map, the metres each square already spends and
/// the chain length are recomputed from the plan's own paths, so a lane that
/// quietly counted one hop instead of the whole way to a road fails by name.
pub fn verify_access_network(
    realization: &AgricultureRealization,
    entities: &[EntityInstance],
) -> Result<BTreeMap<(i64, i64), TrackOwnership>, String> {
    let mut roads: Vec<_> = entities.iter().filter_map(road_rect).collect();
    roads.sort_by_key(|r| (r.x, r.z, r.width_m, r.depth_m));
    let mut claims = BTreeMap::<(i64, i64), TrackClaim>::new();
    let position: BTreeMap<&str, usize> = realization
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| (field.id.as_str(), index))
        .collect();
    if position.len() != realization.fields.len() {
        return Err("agricultural fields do not have unique ids".into());
    }
    for field in &realization.fields {
        if field.access_path.is_empty() {
            return Err("agricultural lane has no access path".into());
        }
        if field.access_length_m != farm_access::length(&field.access_path) {
            return Err("agricultural lane length differs from its own path".into());
        }
        let lane = lane_distances(&field.access_path);
        let spent_m = field
            .shared_track
            .as_ref()
            .map_or(0, |join| join.road_distance_m);
        // The whole path back to a real road is on the clock. One hop is not a
        // unit of this contract, so a chain cannot tunnel past the budget.
        if field.access_length_m + spent_m > MAX_ACCESS_LENGTH as u64 {
            return Err("agricultural access exceeds the unchanged 160m budget".into());
        }
        let end = *field
            .access_path
            .last()
            .expect("the access path is nonempty");
        match &field.shared_track {
            None => {
                if !matches!(
                    field.access_method.as_str(),
                    "direct_axis_route" | "bounded_grid_detour"
                ) {
                    return Err("agricultural access method differs from its evidence".into());
                }
                if !roads.iter().any(|r| contains(*r, end[0], end[1])) {
                    return Err("agricultural lane does not reach a road footprint".into());
                }
            }
            Some(join) => {
                if field.access_method != "shared_farm_track_spur" {
                    return Err("shared farm track lane has the wrong access method".into());
                }
                if join.cell != end {
                    return Err("shared farm track lane does not end on its join".into());
                }
                if roads.iter().any(|r| contains(*r, end[0], end[1])) {
                    return Err("shared farm track join sits in a road footprint".into());
                }
                let owner = claims
                    .get(&(end[0], end[1]))
                    .ok_or("shared farm track lane joins a track that was never laid")?;
                if owner.field_id != join.field_id
                    || owner.road_distance_m != join.road_distance_m
                    || owner.road_id != field.road_id
                    || owner.chain_lanes + 1 != join.chain_lanes
                {
                    return Err(
                        "shared farm track join differs from the realized network".into(),
                    );
                }
                match position.get(owner.field_id.as_str()) {
                    Some(&owner_index) if owner_index < position[field.id.as_str()] => {}
                    _ => {
                        return Err(
                            "shared farm track lane joins a track that is not older".into(),
                        )
                    }
                }
            }
        }
        for (cell, own_m) in &lane {
            if contains(field.boundary, cell.0, cell.1)
                || roads.iter().any(|r| contains(*r, cell.0, cell.1))
            {
                continue;
            }
            claims.entry(*cell).or_insert(TrackClaim {
                field_id: field.id.clone(),
                road_id: field.road_id.clone(),
                road_distance_m: own_m + spent_m,
                chain_lanes: field
                    .shared_track
                    .as_ref()
                    .map_or(0, |join| join.chain_lanes),
            });
        }
    }
    Ok(claims
        .into_iter()
        .map(|(cell, claim)| {
            (
                cell,
                TrackOwnership {
                    field_id: claim.field_id,
                    road_id: claim.road_id,
                    road_distance_m: claim.road_distance_m,
                    chain_lanes: claim.chain_lanes,
                },
            )
        })
        .collect())
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
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
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
                // B01-T2 mechanical sync: this fixture builds an unprofiled legacy base edge
                profile: None,
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
    /// A 4m-wide road footprint plus a rock wall with one 4m gap. The road is
    /// narrow on purpose: its search inset is a single odd row, which the
    /// established 2m centre lattice can never stand on, so the road search
    /// cannot terminate there while a realized lane still can.
    fn barrier_fixture_at(depth_m: u32) -> (
        SceneSpec,
        development::DevelopmentPlan,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "farms".into(),
            width_m: 160,
            depth_m,
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
            depth_m,
            source: "fixture".into(),
        }];
        let roads = RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
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
                profile: None,
            }],
            roadside_sites: vec![],
        };
        let development_plan = development::plan(&scene, &zones, &roads).unwrap();
        let mut plan = development::plan_landcover(&scene, &development_plan).unwrap();
        let template = plan.cells[0].clone();
        plan.cells = [Rect {
            x: 0,
            z: 0,
            width_m: 160,
            depth_m,
        }]
        .into_iter()
        .enumerate()
        .map(|(index, boundary)| {
            let mut cell = template.clone();
            cell.id = format!("barrier.cell.{index}");
            cell.boundary = boundary;
            cell.role = LandcoverRole::AgricultureCandidate;
            cell
        })
        .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        let chunk = crate::worldgen::STREAM_CHUNK_METERS;
        let mut z = 0_u32;
        while z < depth_m {
            let tall = (depth_m - z).min(chunk);
            terrain
                .insert_chunk(
                    0,
                    z as i32,
                    160,
                    tall,
                    &0_i16.to_le_bytes().repeat((160 * tall) as usize),
                )
                .unwrap();
            z += chunk;
        }
        let water = water_geometry(WaterKind::None, &[], &scene, &serde_json::Value::Null);
        let road = EntityInstance {
            entity_id: "road".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 80,
            world_z: 41,
            world_y: 0,
            width_m: 64.,
            depth_m: 4.,
            height_m: 0.24,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        };
        let mut entities = vec![road];
        for (index, (world_x, width_m)) in [(34, 68.), (118, 84.)].into_iter().enumerate() {
            entities.push(EntityInstance {
                entity_id: format!("wall.{index}"),
                asset_id: "prop.rock".into(),
                kind: "rock".into(),
                world_x,
                world_z: 44,
                world_y: 0,
                width_m,
                depth_m: 2.,
                height_m: 2.,
                scale: 1.,
                rotation_y_deg: 0.,
                grounding: GroundingSpec::default(),
                anchors: vec![],
                bounds: None,
            });
        }
        (scene, development_plan, plan, terrain, water, entities)
    }

    fn barrier_fixture() -> (
        SceneSpec,
        development::DevelopmentPlan,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        barrier_fixture_at(160)
    }

    fn with_track_network(plan: &LandcoverPlan, shared: bool) -> LandcoverPlan {
        use super::super::farm_program::{FarmPattern, TrackNetwork};
        let mut plan = plan.clone();
        let mut pattern = plan.farm_pattern.clone().unwrap_or(FarmPattern {
            default_field: Default::default(),
            zone_fields: BTreeMap::new(),
            track_network: None,
        });
        pattern.track_network = shared.then(TrackNetwork::shared);
        plan.farm_pattern = Some(pattern);
        plan
    }

        #[test]
    fn shared_track_terminals_realize_more_fields_on_the_same_input() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let plain = with_track_network(&plan, false);
        let shared = with_track_network(&plan, true);
        // Same terrain, same plan geometry, same final entities: only the
        // authored policy differs, which is exactly what the manifest sets.
        let (before, before_patches) =
            realize(&scene, &plain, &terrain, water, &entities, &[]).unwrap();
        let (after, after_patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(before.model, "explicit-rectangular-field-access-v3");
        assert_eq!(
            after.model,
            "explicit-rectangular-field-access-v4-shared-tracks"
        );
        // The measured delta on this input: 6 fields before, 15 after.
        assert_eq!(before.fields.len(), 6);
        assert_eq!(after.fields.len(), 15);
        let joined: Vec<_> = after
            .fields
            .iter()
            .filter(|field| field.shared_track.is_some())
            .collect();
        assert_eq!(joined.len(), 9);
        // Every field the road-only rule accepted survives with identical lane
        // geometry: shared tracks can only add fields, they never rewrite one.
        for field in &before.fields {
            let kept = after
                .fields
                .iter()
                .find(|other| other.id == field.id)
                .expect("a realized field must not be lost");
            assert_eq!(kept.access_path, field.access_path);
            assert_eq!(kept.access_length_m, field.access_length_m);
            assert_eq!(kept.access_method, field.access_method);
            assert_eq!(kept.shared_track, None);
        }
        let mut budgets = BTreeSet::new();
        let mut chains = BTreeSet::new();
        for field in &joined {
            assert_eq!(field.access_method, "shared_farm_track_spur");
            assert!(before.fields.iter().all(|other| other.id != field.id));
            let join = field.shared_track.as_ref().unwrap();
            assert_eq!(
                field.access_length_m,
                farm_access::length(&field.access_path)
            );
            // The published lane ends exactly on the square it joins.
            assert_eq!(*field.access_path.last().unwrap(), join.cell);
            // The whole path back to a real road is the budget, not one hop.
            assert!(field.access_length_m + join.road_distance_m <= 160);
            budgets.insert(field.access_length_m + join.road_distance_m);
            chains.insert(join.chain_lanes);
            // A joined square is reused, never dug again: it stays owned by the
            // earlier lane and out of this field's own track surface.
            assert!(after_patches.iter().all(|patch| {
                !(patch.parcel_id == field.id
                    && patch.vertices.chunks_exact(4).any(|square| {
                        square[0][0] as i64 == join.cell[0]
                            && square[0][2] as i64 == join.cell[1]
                    }))
            }));
        }
        // Chains of one and two shared lanes both occur, and every one of them
        // is bounded by metres of path to a road rather than by hops.
        assert_eq!(chains, BTreeSet::from([1, 2]));
        assert_eq!(
            budgets,
            BTreeSet::from([47, 75, 87, 103, 131])
        );
        // Determinism and unique area ownership survive the new rule.
        let (again, again_patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(
            serde_json::to_vec(&after).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&after_patches).unwrap(),
            serde_json::to_vec(&again_patches).unwrap()
        );
        let mut owned = BTreeSet::new();
        for patch in after_patches
            .iter()
            .filter(|patch| patch.material == Material::FarmTrack)
        {
            for square in patch.vertices.chunks_exact(4) {
                assert!(
                    owned.insert((square[0][0] as i64, square[0][2] as i64)),
                    "a farm-track square was laid twice"
                );
            }
        }
        // Both realizations satisfy the independent access-network audit, and
        // the audit's square count is exactly the published track surface.
        assert_eq!(
            verify_access_network(&before, &entities).unwrap().len(),
            before_patches
                .iter()
                .filter(|patch| patch.material == Material::FarmTrack)
                .map(|patch| patch.vertices.len() / 4)
                .sum::<usize>()
        );
        assert_eq!(
            verify_access_network(&after, &entities).unwrap().len(),
            owned.len()
        );
    }

    #[test]
    fn chain_over_the_unchanged_budget_is_rejected_by_name() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture_at(200);
        let plain = with_track_network(&plan, false);
        let shared = with_track_network(&plan, true);
        let (before, _) = realize(&scene, &plain, &terrain, water, &entities, &[]).unwrap();
        let (after, _) = realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(before.fields.len(), 6);
        assert_eq!(after.fields.len(), 18);
        let joined = after
            .fields
            .iter()
            .filter_map(|field| {
                field
                    .shared_track
                    .as_ref()
                    .map(|join| field.access_length_m + join.road_distance_m)
            })
            .max()
            .unwrap();
        // The rule accepts up to the unchanged 160m and not one metre more.
        assert_eq!(joined, 159);
        let token = "access_via_existing_track_exceeds_budget";
        assert!(
            before
                .rejected_fields
                .iter()
                .all(|rejection| !rejection.reason.contains(token)),
            "the road-only rule cannot name a shared-track budget"
        );
        let over: Vec<_> = after
            .rejected_fields
            .iter()
            .filter(|rejection| rejection.reason.contains(token))
            .collect();
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].boundary.z, 170);
        // Recompute that refusal from the published realization alone: the
        // cheapest realized square plus its whole spend back to a road is more
        // than the budget, which is why it is named and not left generic.
        let owners = verify_access_network(&after, &entities).unwrap();
        let b = over[0].boundary;
        let starts = [
            [b.x + 12, b.z + 2],
            [b.x + 22, b.z + 12],
            [b.x + 12, b.z + 22],
            [b.x + 2, b.z + 12],
        ];
        let cheapest = owners
            .iter()
            .map(|(&(x, z), owner)| {
                let straight = starts
                    .iter()
                    .map(|start| start[0].abs_diff(x) + start[1].abs_diff(z))
                    .min()
                    .unwrap();
                straight + owner.road_distance_m
            })
            .min()
            .unwrap();
        assert!(cheapest > 160);
        assert_eq!(cheapest, 167);
    }

    #[test]
    fn audit_rebuilds_the_shared_track_network_from_plan_and_terrain() {
        let (scene, _, plan, terrain, water, entities) = barrier_fixture();
        let shared = with_track_network(&plan, true);
        let (realization, patches) =
            realize(&scene, &shared, &terrain, water, &entities, &[]).unwrap();
        // `landcover::verify` runs exactly this function over the published
        // surfaces, so auditing it here audits the reconstruction rule itself.
        let owners = verify_access_network(&realization, &entities).unwrap();
        let published = patches
            .iter()
            .filter(|patch| patch.material == Material::FarmTrack)
            .map(|patch| patch.vertices.len() / 4)
            .sum::<usize>();
        assert_eq!(owners.len(), published);
        // Every shared lane's join is genuinely owned by an older lane, and the
        // recomputed metres agree with what the lane recorded.
        let mut shared_lanes = 0;
        for field in realization
            .fields
            .iter()
            .filter(|field| field.shared_track.is_some())
        {
            shared_lanes += 1;
            let join = field.shared_track.as_ref().unwrap();
            let owner = &owners[&(join.cell[0], join.cell[1])];
            assert_eq!(owner.field_id, join.field_id);
            assert_eq!(owner.road_distance_m, join.road_distance_m);
            assert_eq!(owner.chain_lanes + 1, join.chain_lanes);
            assert_eq!(owner.road_id, field.road_id);
        }
        assert_eq!(shared_lanes, 9);
        // The metres an audit attributes to a square are the whole distance back
        // to a road along its own lane, not one hop: the deepest square of a
        // chained lane still reports the full spend.
        let deepest = owners.values().map(|owner| owner.road_distance_m).max();
        assert_eq!(deepest, Some(129));
        let chains = owners
            .values()
            .map(|owner| owner.chain_lanes)
            .collect::<BTreeSet<_>>();
        assert_eq!(chains, BTreeSet::from([0, 1, 2]));
        // A falsified join cannot pass: the audit recomputes from the lanes.
        let mut shrunk = realization.clone();
        let tampered = shrunk
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .expect("the fixture realizes shared lanes");
        tampered.shared_track.as_mut().unwrap().road_distance_m += 1;
        assert_eq!(
            verify_access_network(&shrunk, &entities).unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        let mut lengthened = realization.clone();
        let tampered = lengthened
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .expect("the fixture realizes shared lanes");
        tampered.shared_track.as_mut().unwrap().chain_lanes = 9;
        assert_eq!(
            verify_access_network(&lengthened, &entities).unwrap_err(),
            "shared farm track join differs from the realized network"
        );
        // A lane whose own path no longer reaches the budget fails by name too.
        // A lane whose recorded length no longer matches its own path fails.
        let mut over = realization.clone();
        let field = over
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .unwrap();
        field.access_length_m = 161;
        assert_eq!(
            verify_access_network(&over, &entities).unwrap_err(),
            "agricultural lane length differs from its own path"
        );
        // A chain that spends the whole path plus one metre is over budget even
        // though both lanes are individually short: the audit counts the whole
        // way back to a real road, never the last hop.
        let road = entities[0].entity_id.clone();
        let lane = |spur: i64| {
            let mut realization = AgricultureRealization {
                model: "explicit-rectangular-field-access-v4-shared-tracks".into(),
                ..Default::default()
            };
            // The older lane runs 139m from the join back to the road footprint.
            realization.fields.push(FarmField {
                id: "owner".into(),
                source_cell_id: "cell".into(),
                boundary: Rect {
                    x: 48,
                    z: 156,
                    width_m: 24,
                    depth_m: 24,
                },
                cultivated: Rect {
                    x: 52,
                    z: 160,
                    width_m: 16,
                    depth_m: 16,
                },
                road_id: road.clone(),
                access_path: vec![[60, 180], [60, 41]],
                access_method: "bounded_grid_detour".into(),
                access_length_m: 139,
                access_search_expanded: 0,
                preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
                shared_track: None,
            });
            realization.fields.push(FarmField {
                id: "joined".into(),
                source_cell_id: "cell".into(),
                boundary: Rect {
                    x: 48,
                    z: 156,
                    width_m: 24,
                    depth_m: 24,
                },
                cultivated: Rect {
                    x: 52,
                    z: 160,
                    width_m: 16,
                    depth_m: 16,
                },
                road_id: road.clone(),
                access_path: vec![[60, 180 + spur], [60, 180]],
                access_method: "shared_farm_track_spur".into(),
                access_length_m: spur as u64,
                access_search_expanded: 0,
                preparation: "rainfed-prepared-soil-no-irrigation-or-yield-claim".into(),
                shared_track: Some(SharedTrackJoin {
                    field_id: "owner".into(),
                    cell: [60, 180],
                    road_distance_m: 139,
                    chain_lanes: 1,
                }),
            });
            realization
        };
        assert!(
            verify_access_network(&lane(21), &entities).is_ok(),
            "a whole path of exactly 160m must be accepted"
        );
        assert_eq!(
            verify_access_network(&lane(22), &entities).unwrap_err(),
            "agricultural access exceeds the unchanged 160m budget"
        );
        // A lane that forgets its join is no longer allowed to end on a track.
        let mut orphan = realization.clone();
        let field = orphan
            .fields
            .iter_mut()
            .find(|field| field.shared_track.is_some())
            .unwrap();
        field.shared_track = None;
        field.access_method = "bounded_grid_detour".into();
        assert_eq!(
            verify_access_network(&orphan, &entities).unwrap_err(),
            "agricultural lane does not reach a road footprint"
        );
    }
    #[test]
    fn legacy_agriculture_output_is_byte_identical_and_policy_defaults_off() {
        use super::super::farm_program::FarmPattern;
        let (scene, plan, terrain, water, entities) = fixture();
        // The fixture carries no agriculture program at all: the pre-existing
        // rule stays in force and the published bytes cannot move.
        assert!(plan.farm_pattern.is_none());
        let (realization, patches) =
            realize(&scene, &plan, &terrain, water, &entities, &[]).unwrap();
        assert_eq!(realization.model, "explicit-rectangular-field-access-v3");
        assert!(realization
            .fields
            .iter()
            .all(|field| field.shared_track.is_none()));
        let bytes = serde_json::to_vec(&(realization.clone(), patches.clone())).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("sharedTrack"),
            "the legacy surface must not grow a shared-track field"
        );
        assert_eq!(
            blake3::hash(&bytes).to_hex().to_string(),
            LEGACY_AGRICULTURE_HASH
        );
        // An old agricultureProgram without the policy still resolves to the
        // road-only rule, so published manifests deserialize unchanged.
        let legacy: FarmPattern = serde_json::from_value(serde_json::json!({
            "defaultField": {"widthM": 48, "depthM": 32},
            "zoneFields": {}
        }))
        .unwrap();
        assert_eq!(legacy.track_network, None);
        legacy.validate().unwrap();
        let mut with_program = plan.clone();
        with_program.farm_pattern = Some(legacy);
        let (unchanged, _) =
            realize(&scene, &with_program, &terrain, water, &entities, &[]).unwrap();
        assert!(unchanged
            .fields
            .iter()
            .all(|field| field.shared_track.is_none()));
        assert_eq!(
            serde_json::to_value(&unchanged.model).unwrap(),
            serde_json::to_value("explicit-rectangular-field-access-v3").unwrap()
        );
    }

    /// Locked bytes of the road-only agriculture surface on the flat fixture.
    /// Any change to the legacy path breaks this constant on purpose.
    const LEGACY_AGRICULTURE_HASH: &str =
    "57268eb9f4126518ce6d15a479ff6ceb5d37f6a0bd8f8024d8fc5fe138727f82";

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
            track_network: None,
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
            track_network: None,
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
