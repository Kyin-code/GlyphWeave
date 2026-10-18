//! GlyphWeave-original final-surface realization. Managed yards are actual
//! terrain-conforming meshes, partitioned against independently rebuilt access
//! triangles; agriculture and conservation are delegated/planned against the
//! same final terrain. Reserved area is never credited without surface meshes.
use super::development::{self, DevelopmentPlan, LandcoverPlan, LandcoverRole, Rect};
use crate::{
    baked_terrain::BakedTerrain,
    worldgen::{
        AccessSurface, EntityInstance, SceneSpec, WaterGeometry, bake_access_surfaces,
        build_access_graph,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Test-only accounting of real narrow-phase calls, not elapsed-time estimates.
#[cfg(test)]
std::thread_local! {
    pub(super) static FOOTPRINT_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

const EPS: f64 = 1e-9;
const MAX_YARD_GRADE: f64 = 0.25;
type Point = [f64; 2];
type Polygon = Vec<Point>;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceRealization {
    pub version: u32,
    pub scene_id: String,
    pub final_heightfield_hash: String,
    pub patches: Vec<SurfacePatch>,
    pub rejections: Vec<SurfaceRejection>,
    pub agriculture: super::agriculture::AgricultureRealization,
    #[serde(default)]
    pub ecology: EcologyRealization,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfacePatch {
    pub id: String,
    pub source_cell_id: String,
    pub parcel_id: String,
    pub zone_kind: String,
    pub material: Material,
    pub area_m2: f64,
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    ManagedGrass,
    AccessPavement,
    PublicOpenSpace,
    PastureGround,
    CultivatedSoil,
    FieldHeadland,
    FarmTrack,
    ConservationGround,
    ConservationForestFloor,
    ConservationWetland,
}
// Preserve the public type paths while moving ecology ownership out of surfaces.
pub use super::ecology::{EcologyClassSummary, EcologyCommunity, EcologyPlant, EcologyRealization};
use super::ecology::{matches as ecology_matches, verify_contract as ecology_plant_contract};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceRejection {
    pub source_cell_id: String,
    pub x: i64,
    pub z: i64,
    pub area_m2: u64,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceAudit {
    pub status: String,
    pub planned_area_m2: u64,
    pub planned_yard_m2: u64,
    pub realized_area_m2: f64,
    pub grass_area_m2: f64,
    pub pavement_area_m2: f64,
    pub agriculture_fields: usize,
    pub cultivated_area_m2: f64,
    pub headland_area_m2: f64,
    pub farm_track_area_m2: f64,
    #[serde(default)]
    pub public_open_space_area_m2: f64,
    #[serde(default)]
    pub pasture_area_m2: f64,
    #[serde(default)]
    pub conservation_ground_area_m2: f64,
    /// Residual ratios the source left unstated, named in a fixed order. The
    /// residual-land consumer's default (`residual_land::ABSENT_RATIO_DEFAULT`,
    /// zero) applied to each of them, so the effective policy is fully readable
    /// from the report. Additive evidence only: documents written before this
    /// field still deserialize, and no area above changes because of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residual_defaulted_ratios: Vec<String>,
    pub ecology_planned_plants: usize,
    pub ecology_realized_plants: usize,
    pub ecology_unfilled_plants: usize,
    pub unresolved_yard_m2: f64,
    /// Unresolved yard area per owning parcel. Additive evidence only: the
    /// totals, thresholds and ownership rules above are unchanged, and
    /// documents written before this field still deserialize.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unresolved_yard_by_parcel: BTreeMap<String, f64>,
    pub unresolved_area_m2: f64,
    pub patches: usize,
    pub failures: Vec<String>,
}
fn cross(a: Point, b: Point, p: Point) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
pub(super) fn signed_area(p: &[Point]) -> f64 {
    if p.len() < 3 {
        return 0.;
    }
    let a = p[0];
    (1..p.len() - 1)
        .map(|i| cross(a, p[i], p[i + 1]) * 0.5)
        .sum()
}
fn clip(poly: &[Point], a: Point, b: Point, inside: bool) -> Polygon {
    if poly.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let dp = cross(a, b, p);
        let dq = cross(a, b, q);
        let pin = if inside { dp >= 0. } else { dp <= 0. };
        let qin = if inside { dq >= 0. } else { dq <= 0. };
        if pin {
            out.push(p);
        }
        if pin != qin {
            let t = dp / (dp - dq);
            out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
        }
    }
    out.dedup_by(|a, b| (a[0] - b[0]).abs() < EPS && (a[1] - b[1]).abs() < EPS);
    if out.len() > 1
        && (out[0][0] - out[out.len() - 1][0]).abs() < EPS
        && (out[0][1] - out[out.len() - 1][1]).abs() < EPS
    {
        out.pop();
    }
    if signed_area(&out).abs() < EPS {
        vec![]
    } else {
        out
    }
}
/// Convex triangle subtraction: emitted outside pieces are disjoint; the
/// remaining inside polygon is the exact intersection with the pavement.
pub(super) fn subtract_triangle(poly: Polygon, triangle: [Point; 3]) -> (Vec<Polygon>, Polygon) {
    let mut outside = vec![];
    let mut current = poly;
    for i in 0..3 {
        let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
        let part = clip(&current, a, b, false);
        if !part.is_empty() {
            outside.push(part);
        }
        current = clip(&current, a, b, true);
        if current.is_empty() {
            break;
        }
    }
    (outside, current)
}
#[derive(Clone)]
struct PavementTriangle {
    points: [Point; 3],
    heights: [f64; 3],
    bounds: [f64; 4],
}
impl PavementTriangle {
    fn height(&self, p: Point) -> f64 {
        let [a, b, c] = self.points;
        let area = cross(a, b, c);
        let u = cross(b, c, p) / area;
        let v = cross(c, a, p) / area;
        self.heights[0] * u + self.heights[1] * v + self.heights[2] * (1. - u - v)
    }
    fn intersects(&self, r: Rect) -> bool {
        self.bounds[0] < (r.x + i64::from(r.width_m)) as f64
            && self.bounds[2] > r.x as f64
            && self.bounds[1] < (r.z + i64::from(r.depth_m)) as f64
            && self.bounds[3] > r.z as f64
    }
}
fn pavement_triangles(surfaces: &[AccessSurface]) -> Vec<PavementTriangle> {
    let mut result = vec![];
    for s in surfaces {
        for indices in &s.triangles {
            let mut v = indices.map(|i| s.vertices[i as usize]);
            let mut p = v.map(|v| [v[0], v[2]]);
            if signed_area(&p).abs() < EPS {
                continue;
            }
            if signed_area(&p) < 0. {
                p.swap(1, 2);
                v.swap(1, 2);
            }
            result.push(PavementTriangle {
                points: p,
                heights: v.map(|v| v[1]),
                bounds: [
                    p.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
                    p.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
                    p.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
                    p.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
                ],
            });
        }
    }
    result
}
pub(super) fn terrain_height(t: &BakedTerrain, p: Point) -> Result<f64, String> {
    let x = p[0].floor();
    let z = p[1].floor();
    if x < i32::MIN as f64 || z < i32::MIN as f64 || x >= i32::MAX as f64 || z >= i32::MAX as f64 {
        return Err("surface coordinate not representable".into());
    }
    let (x, z) = (x as i32, z as i32);
    let (fx, fz) = (p[0] - f64::from(x), p[1] - f64::from(z));
    let h = |x, z| {
        t.height_clamped(x, z)
            .map(f64::from)
            .ok_or("surface outside complete terrain".to_owned())
    };
    let a = h(x, z)?;
    let b = if fx > EPS { h(x + 1, z)? } else { a };
    let c = if fz > EPS { h(x, z + 1)? } else { a };
    let d = if fx > EPS && fz > EPS {
        h(x + 1, z + 1)?
    } else if fx > EPS {
        b
    } else {
        c
    };
    Ok((a * (1. - fx) + b * fx) * (1. - fz) + (c * (1. - fx) + d * fx) * fz)
}
pub(super) fn append(
    patch: &mut SurfacePatch,
    polygon: &[Point],
    height: impl Fn(Point) -> Result<f64, String>,
) -> Result<(), String> {
    let area = signed_area(polygon).abs();
    if area < EPS {
        return Ok(());
    }
    let base = patch.vertices.len() as u32;
    for &p in polygon {
        patch.vertices.push([p[0], height(p)?, p[1]]);
    }
    // Positive XZ polygon has downward XYZ winding; reverse for Y-up.
    for i in 1..polygon.len() - 1 {
        patch
            .triangles
            .push([base, base + i as u32 + 1, base + i as u32]);
    }
    patch.area_m2 += area;
    Ok(())
}
pub(super) fn ecology_obstacle(e: &EntityInstance) -> bool {
    solid(e)
        || matches!(
            e.kind.as_str(),
            "road" | "sidewalk" | "railway" | "canal" | "river" | "lake" | "water"
        )
}

// Retain dimensions, ownership and all required layers. Prefer the original
// anchor, then a bounded deterministic 1m search within the (<=16m) patch.
// Exhaustion is an explicit error, never silent deletion or a smaller asset.
pub(super) fn choose_ecology_position(
    preferred: [f64; 2],
    bounds: [f64; 4],
    width: f64,
    water: WaterGeometry,
    obstacles: &[&EntityInstance],
    occupied: &[EcologyPlant],
) -> Result<[f64; 2], String> {
    let [min_x, max_x, min_z, max_z] = bounds;
    let r = width * 0.5;
    if !bounds
        .iter()
        .chain(preferred.iter())
        .chain([width].iter())
        .all(|v| v.is_finite())
        || width <= 0.
        || max_x - min_x > 16. + EPS
        || max_z - min_z > 16. + EPS
    {
        return Err("invalid ecology candidate domain".into());
    }
    let clear = |[x, z]: [f64; 2]| {
        x - r >= min_x - EPS
            && x + r <= max_x + EPS
            && z - r >= min_z - EPS
            && z + r <= max_z + EPS
            && !water.intersects_axis_aligned(x, z, r, r)
            && !obstacles
                .iter()
                .any(|e| footprint_intersects_entity(e, x, z, r, r))
            && !occupied.iter().any(|p| {
                let half = r + p.width_m * 0.5;
                (x - p.world_x).abs() < half - EPS && (z - p.world_z).abs() < half - EPS
            })
    };
    if clear(preferred) {
        return Ok(preferred);
    }
    for iz in 0..=16 {
        for ix in 0..=16 {
            let point = [min_x + r + f64::from(ix), min_z + r + f64::from(iz)];
            if clear(point) {
                return Ok(point);
            }
        }
    }
    Err("no clear full-size ecology anchor in source patch".into())
}

// Exact separating-axis overlap for an axis-aligned footprint and rotated
// entity rectangle. Checking five probe points misses thin crossings and
// enclosed obstacles. Touching is not positive-area overlap (EPS tolerance).
pub(super) fn footprint_intersects_entity(
    e: &EntityInstance,
    x: f64,
    z: f64,
    rx: f64,
    rz: f64,
) -> bool {
    #[cfg(test)]
    FOOTPRINT_CHECKS.with(|count| count.set(count.get() + 1));
    let (sin, cos) = f64::from(e.rotation_y_deg).to_radians().sin_cos();
    let hw = f64::from(e.width_m) * f64::from(e.scale) * 0.5;
    let hd = f64::from(e.depth_m) * f64::from(e.scale) * 0.5;
    if ![x, z, rx, rz, sin, cos, hw, hd]
        .iter()
        .all(|v| v.is_finite())
        || rx <= 0.
        || rz <= 0.
        || hw <= 0.
        || hd <= 0.
    {
        return true;
    }
    let dx = x - f64::from(e.world_x);
    let dz = z - f64::from(e.world_z);
    dx.abs() < rx + hw * cos.abs() + hd * sin.abs() - EPS
        && dz.abs() < rz + hw * sin.abs() + hd * cos.abs() - EPS
        && (dx * cos - dz * sin).abs() < hw + rx * cos.abs() + rz * sin.abs() - EPS
        && (dx * sin + dz * cos).abs() < hd + rx * sin.abs() + rz * cos.abs() - EPS
}
pub(super) fn obstacle_intersects(e: &EntityInstance, x: i64, z: i64) -> bool {
    footprint_intersects_entity(e, x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5)
}
pub(super) fn solid(e: &EntityInstance) -> bool {
    super::is_building(&e.kind)
        || matches!(
            e.kind.as_str(),
            "tree"
                | "bush"
                | "rock"
                | "boulder"
                | "wall"
                | "fence"
                | "guardrail"
                | "fallen_log"
                | "lamp"
                | "street_lamp"
                | "water_well"
        )
}
fn exact_building(a: &EntityInstance, b: &EntityInstance) -> bool {
    let (mut a, mut b) = (a.clone(), b.clone());
    a.bounds = None;
    b.bounds = None;
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}
/// Only exact planned buildings with independently walkable assigned-road
/// access can acquire landscaping. Raster isn't altered: this is an explicit
/// surface mesh layer above the final engineered terrain.
pub fn realize(
    scene: &SceneSpec,
    development: &DevelopmentPlan,
    plan: &LandcoverPlan,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<SurfaceRealization, String> {
    let tracing = std::env::var_os("GLYPHWEAVE_BAKE_TRACE").is_some();
    let started = std::time::Instant::now();
    let mut previous = started;
    let mut trace = |stage:&str| {
        if tracing {
            let now=std::time::Instant::now();
            eprintln!("{}",serde_json::json!({"format":"glyphweave.landcover-trace","sceneId":scene.scene_id,"stage":stage,"durationMs":now.duration_since(previous).as_millis(),"elapsedMs":now.duration_since(started).as_millis(),"entities":entities.len(),"sourceCells":plan.cells.len()}));
            previous=now;
        }
    };
    development::validate_landcover_plan(plan, scene, development)?;
    if !terrain.matches_scene(scene) {
        return Err("landcover surface terrain mismatch".into());
    }
    terrain.require_complete().map_err(|e| e.to_string())?;
    let access = build_access_graph(entities, terrain, water);
    let pavement = bake_access_surfaces(&access, terrain).map_err(|e| e.to_string())?;
    let triangles = pavement_triangles(&pavement);
    trace("source-and-pedestrian-access");
    let source: BTreeMap<_, _> = development.cells.iter().map(|c| (&c.id, c)).collect();
    let mut result = SurfaceRealization {
        version: 2,
        scene_id: scene.scene_id.clone(),
        final_heightfield_hash: terrain.content_hash().map_err(|e| e.to_string())?,
        patches: vec![],
        rejections: vec![],
        agriculture: Default::default(),
        ecology: EcologyRealization {
            model: "terrain-semantic-surface-classes-v1".into(),
            ..Default::default()
        },
    };
    for cell in plan
        .cells
        .iter()
        .filter(|c| c.role == LandcoverRole::ManagedYard)
    {
        let parcel_id = source[&cell.source_cell_id]
            .owner_id
            .as_ref()
            .ok_or("yard missing parcel source")?;
        let building = development
            .buildings
            .iter()
            .find(|b| &b.parcel_id == parcel_id)
            .ok_or("yard missing building plan")?;
        let found: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_id == building.entity.entity_id)
            .collect();
        let valid = found.len() == 1
            && exact_building(found[0], &building.entity)
            && access.links.iter().any(|l| {
                l.required
                    && l.connected
                    && l.entity_id == building.entity.entity_id
                    && l.road_id.as_deref() == Some(building.road_id.as_str())
            });
        if !valid {
            result.rejections.push(SurfaceRejection {
                source_cell_id: cell.id.clone(),
                x: cell.boundary.x,
                z: cell.boundary.z,
                area_m2: cell.boundary.area(),
                reason: "building_or_assigned_access_unrealized".into(),
            });
            continue;
        }
        let mut grass = SurfacePatch {
            id: format!("{}.grass", cell.id),
            source_cell_id: cell.id.clone(),
            parcel_id: parcel_id.clone(),
            zone_kind: cell.zone_kind.clone(),
            material: Material::ManagedGrass,
            area_m2: 0.,
            vertices: vec![],
            triangles: vec![],
        };
        let mut paved = grass.clone();
        paved.id = format!("{}.pavement", cell.id);
        paved.material = Material::AccessPavement;
        let obstacles: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_id != building.entity.entity_id && solid(e))
            .collect();
        let nearby: Vec<_> = triangles
            .iter()
            .filter(|t| t.intersects(cell.boundary))
            .collect();
        for z in cell.boundary.z..cell.boundary.z + i64::from(cell.boundary.depth_m) {
            for x in cell.boundary.x..cell.boundary.x + i64::from(cell.boundary.width_m) {
                let r = Rect {
                    x,
                    z,
                    width_m: 1,
                    depth_m: 1,
                };
                let poly = vec![
                    [x as f64, z as f64],
                    [x as f64 + 1., z as f64],
                    [x as f64 + 1., z as f64 + 1.],
                    [x as f64, z as f64 + 1.],
                ];
                let heights: Vec<_> = poly
                    .iter()
                    .map(|&p| terrain_height(terrain, p))
                    .collect::<Result<_, _>>()?;
                // Upper bound of bilinear gradient over the whole metre cell,
                // not merely a one-axis slope that misses diagonal steepness.
                let dx = (heights[1] - heights[0])
                    .abs()
                    .max((heights[2] - heights[3]).abs());
                let dz = (heights[3] - heights[0])
                    .abs()
                    .max((heights[2] - heights[1]).abs());
                let grade = dx.hypot(dz);
                let reason =
                    if water.intersects_axis_aligned(x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5) {
                        Some("water_intersection")
                    } else if obstacles.iter().any(|e| obstacle_intersects(e, x, z)) {
                        Some("solid_object_occupies_yard")
                    } else if grade > MAX_YARD_GRADE + EPS {
                        Some("yard_grade_exceeds_25_percent")
                    } else {
                        None
                    };
                if let Some(reason) = reason {
                    result.rejections.push(SurfaceRejection {
                        source_cell_id: cell.id.clone(),
                        x,
                        z,
                        area_m2: 1,
                        reason: reason.into(),
                    });
                    continue;
                }
                let mut remaining = vec![poly];
                for triangle in nearby.iter().filter(|t| t.intersects(r)) {
                    let mut next = vec![];
                    for polygon in remaining {
                        let (outside, inside) = subtract_triangle(polygon, triangle.points);
                        if !inside.is_empty() {
                            append(&mut paved, &inside, |p| Ok(triangle.height(p)))?;
                        }
                        next.extend(outside);
                    }
                    remaining = next;
                    if remaining.is_empty() {
                        break;
                    }
                }
                for polygon in remaining {
                    append(&mut grass, &polygon, |p| {
                        terrain_height(terrain, p).map(|h| h + 0.025)
                    })?;
                }
            }
        }
        for patch in [grass, paved] {
            if patch.area_m2 > EPS {
                result.patches.push(patch);
            }
        }
    }
    trace("yard-clipping");
    // ---- Shoreline public passage band (opt-in waterfront only) -------------
    //
    // The band is the strip of DRY ground within `WATER_SHORE_CLEARANCE_M` of the
    // water body: it is an offset of the waterline, so it is continuous along
    // every shore by construction rather than by luck (see
    // `WaterGeometry::shore_band_cell`). It is realized here as ONE surface layer
    // on the existing `PublicOpenSpace` physical column, so the promenade is real
    // geometry with a ledger row instead of a comment, and a lot may not claim it:
    // the roadside pad rule dilates the pad by the same constant, which is why a
    // pad that keeps the shore clearance owns no band cell at all.
    //
    // Natural detail (reeds, trees, benches) does NOT break the band: an object
    // standing on a public green is not a second surface, so no area is credited
    // twice. Only two things may keep the band out of this pass, and both are
    // named: protected land (it keeps its conservation surface) and a building
    // PARCEL standing in the band, which is a planning defect the audit must see
    // rather than something this pass may quietly cover with pavement.
    if water.shore_band() {
        for cell in &plan.cells {
            let width = i64::from(cell.boundary.width_m);
            let depth = i64::from(cell.boundary.depth_m);
            if width == 0 || depth == 0 {
                continue;
            }
            let protected_cell = !matches!(cell.zone_kind.as_str(), "urban" | "rural");
            for z in cell.boundary.z..cell.boundary.z + depth {
                for x in cell.boundary.x..cell.boundary.x + width {
                    let (xi, zi) = (x as i32, z as i32);
                    if !water.shore_band_cell(xi, zi) {
                        continue;
                    }
                    let reason = if protected_cell {
                        Some("shore_band_in_protected_land")
                    } else if cell.role == LandcoverRole::ManagedYard {
                        Some("shore_band_inside_building_parcel")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        result.rejections.push(SurfaceRejection {
                            source_cell_id: cell.id.clone(),
                            x,
                            z,
                            area_m2: 1,
                            reason: reason.into(),
                        });
                        continue;
                    }
                    let square = [
                        [x as f64, z as f64],
                        [x as f64 + 1., z as f64],
                        [x as f64 + 1., z as f64 + 1.],
                        [x as f64, z as f64 + 1.],
                    ];
                    let mut patch = SurfacePatch {
                        id: format!(
                            "{}.shore.{}.{}",
                            cell.id,
                            x - cell.boundary.x,
                            z - cell.boundary.z
                        ),
                        source_cell_id: cell.id.clone(),
                        parcel_id: cell.id.clone(),
                        zone_kind: cell.zone_kind.clone(),
                        material: Material::PublicOpenSpace,
                        area_m2: 0.,
                        vertices: vec![],
                        triangles: vec![],
                    };
                    append(&mut patch, &square, |p| {
                        terrain_height(terrain, p).map(|h| h + 0.025)
                    })?;
                    if patch.area_m2 > EPS {
                        result.patches.push(patch);
                    }
                }
            }
        }
    }
    trace("shore-band");
    let (agriculture, patches) =
        super::agriculture::realize(scene, plan, terrain, water, entities, &pavement)?;
    result.patches.extend(patches);
    trace("agriculture");
    // Nature cells receive a real, terrain-conforming conservation surface.
    // This is deliberately coarse (one quad per source cell), but it is a
    // source-bound mesh rather than a green-area claim or browser texture.
    for cell in plan
        .cells
        .iter()
        .filter(|c| c.role == LandcoverRole::ConservationCandidate)
    {
        // Subdivide large nature ownership cells so a rolling natural surface
        // is sampled rather than represented by one stretched planar quad.
        // The 16m grid is a deterministic geometry/detail compromise; it does
        // not invent ecology or alter the protected heightfield.
        const TILE: i64 = 16;
        for tz in (cell.boundary.z..cell.boundary.z + i64::from(cell.boundary.depth_m))
            .step_by(TILE as usize)
        {
            for tx in (cell.boundary.x..cell.boundary.x + i64::from(cell.boundary.width_m))
                .step_by(TILE as usize)
            {
                let right = (tx + TILE).min(cell.boundary.x + i64::from(cell.boundary.width_m));
                let bottom = (tz + TILE).min(cell.boundary.z + i64::from(cell.boundary.depth_m));
                let x = tx as f64;
                let z = tz as f64;
                let r = right as f64;
                let b = bottom as f64;
                let corners = [(x, z), (x, b), (r, b), (r, z)];
                if corners
                    .iter()
                    .any(|(xx, zz)| water.intersects_axis_aligned(*xx, *zz, 0., 0.))
                {
                    continue;
                }
                let h0 = terrain_height(terrain, [x, z])?;
                let h1 = terrain_height(terrain, [x, b])?;
                let h2 = terrain_height(terrain, [r, b])?;
                let h3 = terrain_height(terrain, [r, z])?;
                let local_grade =
                    ((h0 - h2).abs().max((h1 - h3).abs())) / ((right - tx).max(1) as f64);
                let wet = [(x + r) * 0.5, (z + b) * 0.5];
                let material = if water.intersects_axis_aligned(wet[0], wet[1], 0., 0.)
                    || local_grade < 0.04 && h0.min(h1).min(h2).min(h3) < 1.0
                {
                    Material::ConservationWetland
                } else if local_grade < 0.18 {
                    Material::ConservationForestFloor
                } else {
                    Material::ConservationGround
                };
                let mut patch = SurfacePatch {
                    id: format!(
                        "{}.conservation.{}.{}",
                        cell.id,
                        tx - cell.boundary.x,
                        tz - cell.boundary.z
                    ),
                    source_cell_id: cell.id.clone(),
                    parcel_id: cell.id.clone(),
                    zone_kind: cell.zone_kind.clone(),
                    material,
                    area_m2: (right - tx) as f64 * (bottom - tz) as f64,
                    vertices: Vec::new(),
                    triangles: vec![[0, 1, 2], [0, 2, 3]],
                };
                for (xx, zz) in corners {
                    let h = terrain_height(terrain, [xx, zz])?;
                    patch.vertices.push([xx, h, zz]);
                }
                result.patches.push(patch);
            }
        }
    }
    trace("conservation-surfaces");
    if let Some(program) = &plan.residual_program {
        program.validate()?;
        let mut occupied = std::collections::BTreeSet::<(i64, i64)>::new();
        for patch in &result.patches {
            for vertex in &patch.vertices {
                occupied.insert((vertex[0].floor() as i64, vertex[2].floor() as i64));
            }
        }
        let residual_obstacles: Vec<_> = entities.iter().filter(|e| solid(e)).collect();
        // A ratio the source left unstated consumes nothing: the named default
        // is `residual_land::ABSENT_RATIO_DEFAULT`, so a program that states
        // only the public-space share realizes public open space and no pasture.
        // Neither ratio is ever derived from the other.
        for (role, ratio, material, label) in [
            (
                LandcoverRole::FutureDevelopment,
                program.urban_open_space_ratio(),
                Material::PublicOpenSpace,
                "urban-open-space",
            ),
            (
                LandcoverRole::AgricultureCandidate,
                program.rural_pasture_ratio(),
                Material::PastureGround,
                "rural-pasture",
            ),
        ] {
            for cell in plan.cells.iter().filter(|c| c.role == role) {
                let target = (cell.boundary.area() as f64 * ratio).floor() as u64;
                if target == 0 {
                    continue;
                }
                let mut patch = SurfacePatch {
                    id: format!("{}.{}", cell.id, label),
                    source_cell_id: cell.id.clone(),
                    parcel_id: cell.id.clone(),
                    zone_kind: cell.zone_kind.clone(),
                    material,
                    area_m2: 0.,
                    vertices: vec![],
                    triangles: vec![],
                };
                let mut realized = 0_u64;
                for z in cell.boundary.z..cell.boundary.z + i64::from(cell.boundary.depth_m) {
                    for x in cell.boundary.x..cell.boundary.x + i64::from(cell.boundary.width_m)
                    {
                        if realized >= target {
                            break;
                        }
                        if occupied.contains(&(x, z)) {
                            continue;
                        }
                        let square = [
                            [x as f64, z as f64],
                            [x as f64 + 1., z as f64],
                            [x as f64 + 1., z as f64 + 1.],
                            [x as f64, z as f64 + 1.],
                        ];
                        let heights: Vec<_> = square
                            .iter()
                            .map(|&p| terrain_height(terrain, p))
                            .collect::<Result<_, _>>()?;
                        let dx = (heights[1] - heights[0])
                            .abs()
                            .max((heights[2] - heights[3]).abs());
                        let dz = (heights[3] - heights[0])
                            .abs()
                            .max((heights[2] - heights[1]).abs());
                        let reason = if water
                            .intersects_axis_aligned(x as f64 + 0.5, z as f64 + 0.5, 0.5, 0.5)
                        {
                            Some("water_intersection")
                        } else if residual_obstacles
                            .iter()
                            .any(|e| obstacle_intersects(e, x, z))
                        {
                            Some("solid_object_occupies_residual_land")
                        } else if dx.hypot(dz) > MAX_YARD_GRADE + EPS {
                            Some("residual_grade_exceeds_25_percent")
                        } else {
                            None
                        };
                        if let Some(reason) = reason {
                            result.rejections.push(SurfaceRejection {
                                source_cell_id: cell.id.clone(),
                                x,
                                z,
                                area_m2: 1,
                                reason: reason.into(),
                            });
                            continue;
                        }
                        append(&mut patch, &square, |p| {
                            terrain_height(terrain, p).map(|h| h + 0.025)
                        })?;
                        occupied.insert((x, z));
                        realized += 1;
                    }
                }
                if patch.area_m2 > EPS {
                    result.patches.push(patch);
                }
            }
        }
    }
    trace("residual-land");
    result.ecology = super::ecology::realize(scene, &result.patches, terrain, water, entities)?;
    trace("ecology");

    result.agriculture = agriculture;
    Ok(result)
}
pub(super) fn mesh_area(p: &SurfacePatch) -> Result<f64, String> {
    let mut area = 0.;
    for t in &p.triangles {
        let mut v = [[0.; 3]; 3];
        for i in 0..3 {
            v[i] = *p
                .vertices
                .get(t[i] as usize)
                .ok_or("invalid landcover triangle index")?;
            if !v[i].iter().all(|v| v.is_finite()) {
                return Err("nonfinite landcover vertex".into());
            }
        }
        let signed = cross([v[0][0], v[0][2]], [v[1][0], v[1][2]], [v[2][0], v[2][2]]);
        if signed > EPS {
            return Err("downward landcover triangle".into());
        }
        area += signed.abs() * 0.5;
    }
    Ok(area)
}
pub fn verify(
    actual: &SurfaceRealization,
    scene: &SceneSpec,
    development: &DevelopmentPlan,
    plan: &LandcoverPlan,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<SurfaceAudit, String> {
    let expected = realize(scene, development, plan, terrain, water, entities)?;
    // Field shapes are re-derived from the published rectangles and the final
    // terrain before any byte comparison: a contour strip that drifts off its
    // cell's contour axis, out of the size domain, off the 12% grade or across a
    // protected feature fails by name instead of only as "differs from the
    // independent realization".
    super::agriculture::verify_field_shapes(&actual.agriculture, plan, terrain, water, entities)?;
    ecology_plant_contract(&actual.ecology, &actual.patches, water, entities)?;
    if !ecology_matches(&actual.ecology, &expected.ecology) {
        return Err("landcover ecology differs from independent realization".into());
    }
    if actual.version != expected.version
        || actual.scene_id != expected.scene_id
        || actual.final_heightfield_hash != expected.final_heightfield_hash
        || actual.rejections != expected.rejections
        || actual.agriculture != expected.agriculture
        || actual.patches.len() != expected.patches.len()
    {
        return Err("landcover surface differs from independent realization".into());
    }
    for (a, e) in actual.patches.iter().zip(&expected.patches) {
        if a.id != e.id
            || a.source_cell_id != e.source_cell_id
            || a.parcel_id != e.parcel_id
            || a.zone_kind != e.zone_kind
            || a.material != e.material
            || a.triangles != e.triangles
            || a.vertices.len() != e.vertices.len()
            || !a.area_m2.is_finite()
            || (a.area_m2 - e.area_m2).abs() > 1e-7
            || a.vertices.iter().zip(&e.vertices).any(|(a, e)| {
                a.iter()
                    .zip(e)
                    .any(|(a, e)| !a.is_finite() || (a - e).abs() > 1e-8)
            })
        {
            return Err("landcover patch differs from independent realization".into());
        }
        if (mesh_area(a)? - e.area_m2).abs() > 1e-7 {
            return Err("landcover mesh area mismatch".into());
        }
    }
    // The farm-track network is re-derived from the published lanes: a square
    // belongs to exactly one lane, the lane that published it must be the one
    // the access graph says owns it, and a join may not invent a shorter way
    // back to a road. Checking only the generator would make this circular.
    let track_owners = super::agriculture::verify_access_network(&actual.agriculture, entities)?;
    let mut published_tracks = BTreeMap::<(i64, i64), &str>::new();
    for patch in actual
        .patches
        .iter()
        .filter(|p| p.material == Material::FarmTrack)
    {
        for square in patch.vertices.chunks_exact(4) {
            let cell = (square[0][0] as i64, square[0][2] as i64);
            if published_tracks
                .insert(cell, patch.parcel_id.as_str())
                .is_some()
            {
                return Err("farm track square is laid by two lanes".into());
            }
            if track_owners
                .get(&cell)
                .map(|owner| owner.field_id.as_str())
                != Some(patch.parcel_id.as_str())
            {
                return Err("farm track square is not owned by its realized lane".into());
            }
        }
    }
    if published_tracks.len() != track_owners.len() {
        return Err("realized farm track squares differ from the access network".into());
    }
    let planned = plan.cells.iter().map(|c| c.boundary.area()).sum::<u64>();
    let yard = plan
        .cells
        .iter()
        .filter(|c| c.role == LandcoverRole::ManagedYard)
        .map(|c| c.boundary.area())
        .sum::<u64>();
    let mut grass = 0.;
    let mut paved = 0.;
    let (mut cultivated, mut headland, mut tracks, mut conservation) = (0., 0., 0., 0.);
    let (mut public_open_space, mut pasture) = (0., 0.);
    for p in &actual.patches {
        match p.material {
            Material::ManagedGrass => grass += mesh_area(p)?,
            Material::AccessPavement => paved += mesh_area(p)?,
            Material::PublicOpenSpace => public_open_space += mesh_area(p)?,
            Material::PastureGround => pasture += mesh_area(p)?,
            Material::CultivatedSoil => cultivated += mesh_area(p)?,
            Material::FieldHeadland => headland += mesh_area(p)?,
            Material::FarmTrack => tracks += mesh_area(p)?,
            Material::ConservationGround
            | Material::ConservationForestFloor
            | Material::ConservationWetland => conservation += mesh_area(p)?,
        }
    }
    if (cultivated
        - actual
            .agriculture
            .fields
            .iter()
            .map(|f| f.cultivated.area() as f64)
            .sum::<f64>())
    .abs()
        > 1e-7
        || (headland
            - actual
                .agriculture
                .fields
                .iter()
                .map(|f| f.boundary.area() as f64 - f.cultivated.area() as f64)
                .sum::<f64>())
        .abs()
            > 1e-7
    {
        return Err("agricultural field preparation area mismatch".into());
    }
    let yard_realized = grass + paved;
    // Keep realizedAreaM2 compatible with the pre-r28 yard/agriculture
    // contract. Nature coverage is reported separately and accounted by the
    // purpose ledger; it must not change yard-area assertions.
    let realized =
        yard_realized + cultivated + headland + tracks + public_open_space + pasture;
    let yard_source_ids: BTreeMap<_, _> = plan
        .cells
        .iter()
        .filter(|cell| cell.role == LandcoverRole::ManagedYard)
        .map(|cell| (cell.id.as_str(), ()))
        .collect();
    let yard_rejected = actual
        .rejections
        .iter()
        .filter(|rejection| yard_source_ids.contains_key(rejection.source_cell_id.as_str()))
        .map(|r| r.area_m2 as f64)
        .sum::<f64>();
    if (yard_realized + yard_rejected - yard as f64).abs() > 1e-6 {
        return Err("yard realization area is not conserved".into());
    }
    // Remove only sub-square-millimetre floating point residue, not missing
    // whole cells or geometric deficits. Triangle sums remain unrounded.
    let residual = |v: f64| if v.abs() < 1e-7 { 0. } else { v.max(0.) };
    if realized + conservation > planned as f64 + 1e-7 {
        return Err("landcover realized area exceeds candidate ownership".into());
    }
    let unresolved = residual(planned as f64 - realized - conservation);
    let unresolved_yard = residual(yard as f64 - yard_realized);
    // Same rejection evidence, attributed to the owning parcel so new parcels
    // can never spend an old parcel's shortfall.
    let mut unresolved_yard_by_parcel: BTreeMap<String, f64> = BTreeMap::new();
    let owner_by_cell: BTreeMap<&str, &str> = development
        .cells
        .iter()
        .filter_map(|cell| {
            cell.owner_id
                .as_deref()
                .map(|owner| (cell.id.as_str(), owner))
        })
        .collect();
    for rejection in actual
        .rejections
        .iter()
        .filter(|rejection| yard_source_ids.contains_key(rejection.source_cell_id.as_str()))
    {
        let parcel_id = plan
            .cells
            .iter()
            .find(|cell| cell.id == rejection.source_cell_id)
            .and_then(|cell| owner_by_cell.get(cell.source_cell_id.as_str()).copied())
            .map(str::to_string)
            .unwrap_or_else(|| rejection.source_cell_id.clone());
        *unresolved_yard_by_parcel.entry(parcel_id).or_default() +=
            residual(rejection.area_m2 as f64);
    }
    Ok(SurfaceAudit {
        status: if unresolved > 1e-7 || !actual.ecology.rejections.is_empty() {
            "incomplete"
        } else {
            "pass"
        }
        .into(),
        planned_area_m2: planned,
        planned_yard_m2: yard,
        realized_area_m2: realized,
        grass_area_m2: grass,
        pavement_area_m2: paved,
        agriculture_fields: actual.agriculture.fields.len(),
        cultivated_area_m2: cultivated,
        headland_area_m2: headland,
        farm_track_area_m2: tracks,
        public_open_space_area_m2: public_open_space,
        pasture_area_m2: pasture,
        conservation_ground_area_m2: conservation,
        residual_defaulted_ratios: plan
            .residual_program
            .as_ref()
            .map(|program| {
                program
                    .unstated_ratios()
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        ecology_planned_plants: actual
            .ecology
            .communities
            .iter()
            .map(|c| c.planned_plant_count)
            .sum(),
        ecology_realized_plants: actual.ecology.plants.len(),
        ecology_unfilled_plants: actual.ecology.rejections.len(),
        unresolved_yard_m2: unresolved_yard,
        unresolved_yard_by_parcel,
        unresolved_area_m2: unresolved,
        patches: actual.patches.len(),
        failures: if actual.ecology.rejections.is_empty() {
            vec![]
        } else {
            vec!["ecology density budget remains unrealized".into()]
        },
    })
}
/// Read only geometry-derived areas; caller MUST independently verify patches
/// before using this diagnostic as release evidence.
pub fn area_by_zone(s: &SurfaceRealization, kind: &str) -> f64 {
    s.patches
        .iter()
        .filter(|p| p.zone_kind == kind && p.material != Material::FarmTrack)
        .filter_map(|p| mesh_area(p).ok())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{
        LandUseZone,
        road_network::{RoadEdge, RoadNetworkPlan, RoadsideSite},
        water_geometry, water_kind,
    };
    fn fixture() -> (
        SceneSpec,
        DevelopmentPlan,
        LandcoverPlan,
        BakedTerrain,
        WaterGeometry,
        Vec<EntityInstance>,
    ) {
        let scene = SceneSpec {
            scene_id: "yard".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = vec![LandUseZone {
            id: "zone".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 128,
            depth_m: 128,
            source: "test".into(),
        }];
        let road = RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: "yard".into(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 1,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            pad_conversions: vec![],
            conversion_allocation: None,
        conversion_fringe: None,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: Vec::new(),
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
            nodes: vec![],
            edges: vec![RoadEdge {
                id: "road".into(),
                from: 0,
                to: 1,
                world_x: 64,
                world_z: 64,
                length_m: 64,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
                // B01-T2 mechanical sync: unprofiled legacy edge in this fixture
                profile: None,
            }],
            roadside_sites: vec![RoadsideSite {
                program: None,
                purpose: None,
                id: "generated.home".into(),
                road_id: "road".into(),
                world_x: 64,
                world_z: 88,
                width_m: 24,
                depth_m: 24,
                support_level_m: 0,
                parcel_insets: None,
                support_level_mm: None,
                    frontage_road_point: None,
                    bounded_cut_mm: None,
                    earthwork_blend_m: None,
                rotation_y_deg: 0.,
                natural_min_m: 0.,
                natural_max_m: 0.,
            }],
        };
        let development = development::plan(&scene, &zones, &road).unwrap();
        let plan = development::plan_landcover(&scene, &development).unwrap();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 128, 128, &0_i16.to_le_bytes().repeat(128 * 128))
            .unwrap();
        let style = serde_json::Value::Null;
        let water = water_geometry(water_kind(&style), &[], &scene, &style);
        let mut entities = road.entities();
        entities.extend(development.building_entities());
        (scene, development, plan, terrain, water, entities)
    }
    #[test]
    fn ecology_relocates_full_size_anchors_without_deleting_layers() {
        let (_, _, _, _, water, entities) = fixture();
        let mut obstacle = entities[0].clone();
        obstacle.world_x = 8;
        obstacle.world_z = 6;
        obstacle.width_m = 6.;
        obstacle.depth_m = 0.1;
        obstacle.scale = 1.;
        obstacle.rotation_y_deg = 0.;
        let obstacles = vec![&obstacle];
        let bounds = [0., 16., 0., 16.];
        let preferred = [8., 8.];
        let chosen =
            choose_ecology_position(preferred, bounds, 8., water, &obstacles, &[]).unwrap();
        assert_ne!(chosen, preferred);
        assert!(!footprint_intersects_entity(
            &obstacle, chosen[0], chosen[1], 4., 4.
        ));
        assert_eq!(
            chosen,
            choose_ecology_position(preferred, bounds, 8., water, &obstacles, &[]).unwrap()
        );
        assert_eq!(
            choose_ecology_position(preferred, bounds, 8., water, &[], &[]).unwrap(),
            preferred
        );
        obstacle.width_m = 32.;
        obstacle.depth_m = 32.;
        assert!(choose_ecology_position(preferred, bounds, 8., water, &[&obstacle], &[]).is_err());
        assert!(
            choose_ecology_position(preferred, bounds, 17., water, &[], &[]).is_err(),
            "must not shrink to fit"
        );
    }

    #[test]
    fn footprint_sat_catches_thin_crossings_and_enclosed_obstacles() {
        let (_, _, _, _, _, entities) = fixture();
        let mut e = entities[0].clone();
        e.world_x = 8;
        e.world_z = 6;
        e.width_m = 6.;
        e.depth_m = 0.1;
        e.rotation_y_deg = 0.;
        e.scale = 1.;
        // A strip crosses between the centre and the corners, so the old five
        // one-metre probes miss it entirely.
        for [x, z] in [[8, 8], [4, 4], [4, 12], [12, 4], [12, 12]] {
            assert!(!obstacle_intersects(&e, x, z));
        }
        assert!(footprint_intersects_entity(&e, 8., 8., 4., 4.));
        e.world_x = 10;
        e.world_z = 9;
        e.width_m = 0.1;
        assert!(footprint_intersects_entity(&e, 8., 8., 4., 4.));
        e.world_x = 13;
        e.world_z = 8;
        e.width_m = 2.;
        e.depth_m = 2.;
        assert!(
            !footprint_intersects_entity(&e, 8., 8., 4., 4.),
            "touching only"
        );
        e.world_x = 14;
        assert!(!footprint_intersects_entity(&e, 8., 8., 4., 4.), "disjoint");
    }

    #[test]
    fn footprint_sat_respects_rotation_scale_and_large_origins() {
        let (_, _, _, _, _, entities) = fixture();
        for offset in [-1_000_000_000, 0, 1_000_000_000] {
            let mut e = entities[0].clone();
            e.world_x = offset + 12;
            e.world_z = offset + 12;
            e.width_m = 12.;
            e.depth_m = 0.1;
            e.scale = 1.;
            e.rotation_y_deg = 45.;
            assert!(
                !footprint_intersects_entity(&e, offset as f64 + 8., offset as f64 + 8., 1., 1.),
                "AABB corner is not rotated footprint"
            );
            assert!(footprint_intersects_entity(
                &e,
                offset as f64 + 12.,
                offset as f64 + 12.,
                1.,
                1.
            ));
            e.rotation_y_deg = 0.;
            e.width_m = 2.;
            e.depth_m = 2.;
            assert!(!footprint_intersects_entity(
                &e,
                offset as f64 + 9.,
                offset as f64 + 12.,
                1.,
                1.
            ));
            e.scale = 3.;
            assert!(footprint_intersects_entity(
                &e,
                offset as f64 + 9.,
                offset as f64 + 12.,
                1.,
                1.
            ));
            e.width_m = f32::NAN;
            assert!(
                footprint_intersects_entity(&e, 0., 0., 1., 1.),
                "invalid geometry fails closed"
            );
        }
    }

    #[test]
    fn conservation_patch_is_terrain_bound_and_forgery_is_rejected() {
        let (mut s, _old_d, _old_p, _old_t, w, _old_e) = fixture();
        s.scene_id = "nature".into();
        let nature = LandUseZone {
            id: "nature".into(),
            kind: "nature".into(),
            world_x: 0,
            world_z: 0,
            width_m: 128,
            depth_m: 128,
            source: "test".into(),
        };
        let roads = RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "test".into(),
            scene_id: "nature".into(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: None,
            candidates: 0,
            rejected_water: 0,
            rejected_relief: 0,
            unselected_candidates: 0,
            nodes: vec![],
            edges: vec![],
            roadside_sites: vec![],
            pad_conversions: vec![],
            conversion_allocation: None,
        conversion_fringe: None,
            zoning_hash: None,
            zoning_rejections: vec![],
            service_outcomes: vec![],
            joint_layout: None,
            water_crossings: Vec::new(),
            water_crossing_rejections: Vec::new(),
        };
        let d = development::plan(&s, &[nature], &roads).unwrap();
        let p = development::plan_landcover(&s, &d).unwrap();
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(0, 0, 128, 128, &0_i16.to_le_bytes().repeat(128 * 128))
            .unwrap();
        let out = realize(&s, &d, &p, &t, w, &[]).unwrap();
        let conservation = out
            .patches
            .iter()
            .find(|p| {
                matches!(
                    p.material,
                    Material::ConservationGround
                        | Material::ConservationForestFloor
                        | Material::ConservationWetland
                )
            })
            .unwrap();
        assert_eq!(conservation.area_m2, 256.);
        assert_eq!(conservation.vertices.len(), 4);
        // Rejection-only tests hide validators that reject every valid world.
        verify(&out, &s, &d, &p, &t, w, &[]).expect("valid conservation must pass");
        let decoded: SurfaceRealization =
            serde_json::from_slice(&serde_json::to_vec(&out).unwrap()).unwrap();
        verify(&decoded, &s, &d, &p, &t, w, &[]).expect("JSON roundtrip must pass");
        assert!(out.ecology.communities.len() > 0);
        assert!(!out.ecology.plants.is_empty());
        assert_eq!(
            out.ecology.plants.len(),
            out.ecology
                .communities
                .iter()
                .map(|c| c.planned_plant_count)
                .sum::<usize>()
        );
        for mode in 0..15 {
            let mut bad = out.clone();
            match mode {
                0 => {
                    bad.ecology.plants.remove(0);
                }
                1 => bad.ecology.plants.push(bad.ecology.plants[0].clone()),
                2 => bad.ecology.plants[0].source_patch_id = "missing".into(),
                3 => bad.ecology.plants[0].kind = "tree".into(), // flat 0m fixture is wetland
                4 => bad.ecology.plants[0].layer = "canopy".into(),
                5 => bad.ecology.plants[0].world_y += 1.,
                6 => bad.ecology.plants[0].world_x += 0.25,
                7 => bad.ecology.plants[0].width_m = 1000.,
                8 => bad.ecology.communities[0].planned_plant_count += 1,
                9 => bad.ecology.communities[0].density_per_ha += 1.,
                10 => bad.ecology.communities[0].density_per_ha = f64::NAN,
                11 => bad.ecology.communities[0].density_per_ha = f64::INFINITY,
                12 => {
                    bad.ecology.communities.remove(0);
                }
                13 => bad
                    .ecology
                    .communities
                    .push(bad.ecology.communities[0].clone()),
                _ => bad.ecology.total_area_m2 += 1.,
            }
            assert!(
                verify(&bad, &s, &d, &p, &t, w, &[]).is_err(),
                "forgery mode {mode}"
            );
        }
        // Exercise collision validation itself, not just deterministic mismatch.
        let mut clearance = out.ecology.clone();
        clearance.plants[0].world_x = 8.;
        clearance.plants[0].world_z = 8.;
        clearance.plants[0].width_m = 8.;
        let clearance_patch = out
            .patches
            .iter()
            .find(|p| p.id == clearance.plants[0].source_patch_id)
            .unwrap();
        super::super::ecology::verify_plant_clearance(
            &clearance.plants[0],
            clearance_patch,
            w,
            &[],
        )
        .unwrap();
        let mut obstacle = fixture().5[0].clone();
        obstacle.world_x = 8;
        obstacle.world_z = 6;
        obstacle.width_m = 6.;
        obstacle.depth_m = 0.1;
        obstacle.rotation_y_deg = 0.;
        obstacle.scale = 1.;
        for kind in ["road", "railway", "canal", "wall", "rock", "fence", "tree"] {
            obstacle.kind = kind.into();
            assert_eq!(
                super::super::ecology::verify_plant_clearance(
                    &clearance.plants[0],
                    clearance_patch,
                    w,
                    &[obstacle.clone()]
                )
                .unwrap_err(),
                "ecology plant intersects hard entity",
                "{kind}"
            );
        }

        // Physical surface completion must not hide an unmet ecology budget.
        let mut blocked = fixture().5[0].clone();
        blocked.kind = "rock".into();
        blocked.world_x = 64;
        blocked.world_z = 64;
        blocked.width_m = 256.;
        blocked.depth_m = 256.;
        blocked.scale = 1.;
        blocked.rotation_y_deg = 0.;
        let short = realize(&s, &d, &p, &t, w, &[blocked.clone()]).unwrap();
        let audit = verify(&short, &s, &d, &p, &t, w, &[blocked]).unwrap();
        assert_eq!(audit.conservation_ground_area_m2, 16384.);
        assert_eq!(audit.unresolved_area_m2, 0.);
        assert_eq!(audit.ecology_planned_plants, 459);
        assert_eq!(audit.ecology_realized_plants, 0);
        assert_eq!(audit.ecology_unfilled_plants, 459);
        assert_eq!(audit.status, "incomplete");
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("ecology density budget"))
        );
        let mut bad_community = out.clone();
        bad_community.ecology.communities[0].community_kind = "forged-community".into();
        assert!(verify(&bad_community, &s, &d, &p, &t, w, &[]).is_err());
        let mut bad = out.clone();
        bad.patches
            .iter_mut()
            .find(|p| {
                matches!(
                    p.material,
                    Material::ConservationGround
                        | Material::ConservationForestFloor
                        | Material::ConservationWetland
                )
            })
            .unwrap()
            .source_cell_id = "forged".into();
        assert!(verify(&bad, &s, &d, &p, &t, w, &[]).is_err());
        let mut bad = out.clone();
        let patch = bad
            .patches
            .iter_mut()
            .find(|p| {
                matches!(
                    p.material,
                    Material::ConservationGround
                        | Material::ConservationForestFloor
                        | Material::ConservationWetland
                )
            })
            .unwrap();
        patch.material = Material::ManagedGrass;
        assert!(verify(&bad, &s, &d, &p, &t, w, &[]).is_err());
    }
    #[test]
    fn flat_yard_meshes_conserve_area_and_do_not_cover_access_twice() {
        let (s, d, p, t, w, e) = fixture();
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        let a = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert_eq!(a.status, "incomplete");
        assert!((a.realized_area_m2 - 208.).abs() < 1e-7);
        assert!(a.pavement_area_m2 > 0. && a.pavement_area_m2 < 10.);
        assert!(a.grass_area_m2 > 198.);
        assert!(a.unresolved_yard_m2 < 1e-7);
        assert!(a.unresolved_area_m2 > 10000.);
        assert!(out.rejections.is_empty());
        for patch in &out.patches {
            assert!(!patch.vertices.is_empty());
            assert!(!patch.triangles.is_empty());
            for v in &patch.vertices {
                assert!(
                    (v[1]
                        - if patch.material == Material::ManagedGrass {
                            0.025
                        } else {
                            0.04
                        })
                    .abs()
                        < 1e-8
                );
                assert!(
                    v[0] <= 52. + EPS
                        || v[0] >= 76. - EPS
                        || v[2] <= 76. + EPS
                        || v[2] >= 100. - EPS
                );
            }
        }
        let decoded: SurfaceRealization =
            serde_json::from_slice(&serde_json::to_vec(&out).unwrap()).unwrap();
        verify(&decoded, &s, &d, &p, &t, w, &e).unwrap();
    }
    #[test]
    fn convex_clipping_handles_diagonal_boundary_overlap_and_large_world_origin() {
        for origin in [0., -2_000_000_000., 2_000_000_000.] {
            let square = vec![
                [origin, origin],
                [origin + 1., origin],
                [origin + 1., origin + 1.],
                [origin, origin + 1.],
            ];
            for t in [
                [[0., 0.], [1., 0.], [0., 1.]],
                [[-1., -1.], [3., -1.], [-1., 3.]],
                [[2., 2.], [3., 2.], [2., 3.]],
                [[0.5, -1.], [0.5, 2.], [-1., 0.5]],
            ] {
                let triangle = t.map(|p| [p[0] + origin, p[1] + origin]);
                let (out, inside) = subtract_triangle(square.clone(), triangle);
                let total = out.iter().map(|p| signed_area(p).abs()).sum::<f64>()
                    + signed_area(&inside).abs();
                assert!((total - 1.).abs() < 1e-6, "{origin}: {total}");
            }
        }
    }
    #[test]
    fn coincident_access_triangles_do_not_double_count_pavement() {
        let square = vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        let t = [[0., 0.], [1., 0.], [0., 1.]];
        let mut remaining = vec![square];
        let mut covered = 0.;
        for _ in 0..3 {
            let mut next = vec![];
            for p in remaining {
                let (out, inside) = subtract_triangle(p, t);
                covered += signed_area(&inside).abs();
                next.extend(out);
            }
            remaining = next;
        }
        assert!((covered - 0.5).abs() < EPS);
        assert!((remaining.iter().map(|p| signed_area(p).abs()).sum::<f64>() - 0.5).abs() < EPS);
    }

    #[test]
    fn serialized_mesh_forgery_and_missing_layers_fail_independent_rebuild() {
        let (s, d, p, t, w, e) = fixture();
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        for mutation in 0..7 {
            let mut bad = out.clone();
            match mutation {
                0 => bad.patches[0].vertices[0][1] += 0.25,
                1 => bad.patches[0].vertices[0][0] += 0.25,
                2 => bad.patches[0].area_m2 += 1.,
                3 => bad.patches[0].source_cell_id = "fake".into(),
                4 => {
                    bad.patches.pop();
                }
                5 => bad.patches[0].triangles[0][0] = u32::MAX,
                _ => bad.patches[0].vertices[0][1] = f64::NAN,
            }
            assert!(verify(&bad, &s, &d, &p, &t, w, &e).is_err());
        }
        let mut bad = out.clone();
        bad.final_heightfield_hash = "forged".into();
        assert!(verify(&bad, &s, &d, &p, &t, w, &e).is_err());
    }
    #[test]
    fn missing_house_or_blocked_access_cannot_receive_landscaping_credit() {
        let (s, d, p, t, w, mut e) = fixture();
        e.retain(|e| e.kind == "road");
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        assert!(out.patches.is_empty());
        let a = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert_eq!(a.realized_area_m2, 0.);
        assert_eq!(a.unresolved_yard_m2, 208.);
        assert!(
            out.rejections
                .iter()
                .all(|r| r.reason == "building_or_assigned_access_unrealized")
        );
        let (s, d, p, t, w, mut e) = fixture();
        e.retain(|e| e.kind != "road");
        assert!(realize(&s, &d, &p, &t, w, &e).unwrap().patches.is_empty());
    }
    #[test]
    fn occupied_yard_cells_are_not_landscaped_or_deleted() {
        let (s, d, p, t, w, mut e) = fixture();
        let mut rock = e[1].clone();
        rock.entity_id = "existing-rock".into();
        rock.kind = "rock".into();
        rock.asset_id = "prop.rock".into();
        rock.world_x = 64;
        rock.world_z = 101;
        rock.width_m = 1.;
        rock.depth_m = 1.;
        rock.height_m = 1.;
        rock.rotation_y_deg = 45.;
        rock.anchors.clear();
        assert!(obstacle_intersects(&rock, 63, 100));
        assert!(!obstacle_intersects(&rock, 70, 100));
        e.push(rock);
        let before = serde_json::to_value(&e).unwrap();
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        let a = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert!(a.realized_area_m2 > 0. && a.realized_area_m2 < 208.);
        assert!(
            out.rejections
                .iter()
                .any(|r| r.reason == "solid_object_occupies_yard")
        );
        assert_eq!(before, serde_json::to_value(&e).unwrap());
    }

    #[test]
    fn steep_backyard_is_rejected_without_changing_the_heightfield() {
        let (s, d, p, _, w, e) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        let mut bytes = 0_i16.to_le_bytes().repeat(128 * 128);
        let at = (101 * 128 + 64) * 2;
        bytes[at..at + 2].copy_from_slice(&8_i16.to_le_bytes());
        t.insert_chunk(0, 0, 128, 128, &bytes).unwrap();
        let before = t.content_hash().unwrap();
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        assert!(
            out.rejections
                .iter()
                .any(|r| r.reason == "yard_grade_exceeds_25_percent")
        );
        let a = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert!(a.realized_area_m2 > 0. && a.realized_area_m2 < 208.);
        assert_eq!(before, t.content_hash().unwrap());
        assert!(
            (a.realized_area_m2 + out.rejections.iter().map(|r| r.area_m2 as f64).sum::<f64>()
                - 208.)
                .abs()
                < 1e-7
        );
    }
    #[test]
    fn residual_program_realizes_only_the_explicit_fraction() {
        let (s, d, mut p, t, w, e) = fixture();
        p.residual_program = Some(super::super::residual_land::SceneResidualProgram {
            urban_open_space_ratio: Some(0.1),
            rural_pasture_ratio: None,
        });
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        let open: f64 = out
            .patches
            .iter()
            .filter(|patch| patch.material == Material::PublicOpenSpace)
            .map(|patch| patch.area_m2)
            .sum();
        assert!(open > 0.);
        let audit = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert!((audit.public_open_space_area_m2 - open).abs() < 1e-7);
        assert!(audit.unresolved_area_m2 > 0.);
    }

    /// The public-space share on its own is enough: no pasture share is stated,
    /// the named default (0) applies, and the realized public open space enters
    /// the same area accounting as every other material.
    #[test]
    fn an_unstated_pasture_share_is_named_and_realizes_no_pasture() {
        let (s, d, mut p, t, w, e) = fixture();
        p.residual_program = Some(super::super::residual_land::SceneResidualProgram {
            urban_open_space_ratio: Some(0.1),
            rural_pasture_ratio: None,
        });
        let out = realize(&s, &d, &p, &t, w, &e).unwrap();
        let areas = |material: Material| -> f64 {
            out.patches
                .iter()
                .filter(|patch| patch.material == material)
                .map(|patch| patch.area_m2)
                .sum()
        };
        let open = areas(Material::PublicOpenSpace);
        assert!(open > 0., "the public-space share must realize open space");
        assert!(
            (open - open.floor()).abs() < 1e-7,
            "residual squares are whole metres: {open}"
        );
        assert_eq!(areas(Material::PastureGround), 0., "no pasture was stated");
        let audit = verify(&out, &s, &d, &p, &t, w, &e).unwrap();
        assert_eq!(audit.public_open_space_area_m2, open);
        assert_eq!(audit.pasture_area_m2, 0.);
        assert_eq!(
            audit.residual_defaulted_ratios,
            vec!["ruralPastureRatio".to_string()],
            "the applied default must be named in the report"
        );
        // The open space is public-ground mesh inside urban remainder, never
        // inside the nature/conservation role or on top of water.
        let cells: std::collections::BTreeMap<_, _> = p
            .cells
            .iter()
            .map(|cell| (cell.id.as_str(), cell.role))
            .collect();
        for patch in out
            .patches
            .iter()
            .filter(|patch| patch.material == Material::PublicOpenSpace)
        {
            assert_eq!(
                cells[patch.source_cell_id.as_str()],
                LandcoverRole::FutureDevelopment
            );
            assert_eq!(patch.zone_kind, "urban");
            for square in patch.vertices.chunks_exact(4) {
                let (x, z) = (square[0][0], square[0][2]);
                assert!(
                    !w.intersects_axis_aligned(x + 0.5, z + 0.5, 0.5, 0.5),
                    "public open space may not enter water at {x},{z}"
                );
            }
        }
        // Conservation of the scene-level identity, not only the yard identity.
        assert!(
            (audit.realized_area_m2
                + audit.conservation_ground_area_m2
                + audit.unresolved_area_m2
                - audit.planned_area_m2 as f64)
                .abs()
                < 1e-7
        );
        // The stated ratio is an upper bound on the target, cell by cell.
        let target: u64 = p
            .cells
            .iter()
            .filter(|cell| cell.role == LandcoverRole::FutureDevelopment)
            .map(|cell| (cell.boundary.area() as f64 * 0.1).floor() as u64)
            .sum();
        assert_eq!(open, target as f64, "every targetable metre was realized");
        // Omitting the whole policy leaves both shares at the named default.
        let (s2, d2, p2, t2, w2, e2) = fixture();
        let plain = realize(&s2, &d2, &p2, &t2, w2, &e2).unwrap();
        assert!(plain.patches.iter().all(|patch| patch.material
            != Material::PublicOpenSpace
            && patch.material != Material::PastureGround));
        let plain_audit = verify(&plain, &s2, &d2, &p2, &t2, w2, &e2).unwrap();
        assert!(plain_audit.residual_defaulted_ratios.is_empty());
    }

    /// The public-space share is a surface, not a claim: it may not enter water,
    /// a solid entity, a conservation cell or an already realized patch, and each
    /// refusal is named. This is the negative case for the open-space path.
    #[test]
    fn public_open_space_refuses_water_solids_and_conservation() {
        let (s, d, mut p, t, _old_water, mut e) = fixture();
        p.residual_program = Some(super::super::residual_land::SceneResidualProgram {
            urban_open_space_ratio: Some(0.1),
            rural_pasture_ratio: None,
        });
        // A lake over the west half of the scene. The first open-space square
        // the generator would otherwise claim is then occupied by a solid tree,
        // so both refusals are exercised on a location the scan really reaches.
        let lake = crate::worldgen::LandmarkSpec {
            entity_id: "lake".into(),
            name: "lake".into(),
            entity_type: "lake".into(),
            purpose: "water".into(),
            description: String::new(),
            scene_id: s.scene_id.clone(),
            world_x: 32,
            world_z: 64,
            world_y: 0,
            width_m: 64,
            depth_m: 128,
            height_m: 0,
            asset_id: "lake".into(),
            rotation_y_deg: 0.,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        let water = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::Lake,
            &[lake],
            &s,
            &serde_json::Value::Null,
        );
        let control = realize(&s, &d, &p, &t, water, &e).unwrap();
        let first = control
            .patches
            .iter()
            .find(|patch| patch.material == Material::PublicOpenSpace)
            .expect("open space must be realized on the control")
            .vertices
            .chunks_exact(4)
            .next()
            .unwrap();
        let (tx, tz) = (first[0][0] as i32, first[0][2] as i32);
        let mut tree = e[0].clone();
        tree.entity_id = "tree".into();
        tree.kind = "tree".into();
        tree.world_x = tx + 4;
        tree.world_z = tz + 4;
        tree.width_m = 8.;
        tree.depth_m = 8.;
        tree.scale = 1.;
        tree.rotation_y_deg = 0.;
        e.push(tree);
        let out = realize(&s, &d, &p, &t, water, &e).unwrap();
        let open: Vec<_> = out
            .patches
            .iter()
            .filter(|patch| patch.material == Material::PublicOpenSpace)
            .collect();
        assert!(!open.is_empty(), "open space must still be realized");
        for patch in &open {
            for square in patch.vertices.chunks_exact(4) {
                let (x, z) = (square[0][0], square[0][2]);
                assert!(
                    !water.intersects_axis_aligned(x + 0.5, z + 0.5, 0.5, 0.5),
                    "open space entered water at {x},{z}"
                );
                assert!(
                    (x as i32 - tx).abs() >= 4 || (z as i32 - tz).abs() >= 4,
                    "open space entered the solid tree at {x},{z}"
                );
            }
        }
        let reasons: Vec<&str> = out
            .rejections
            .iter()
            .map(|rejection| rejection.reason.as_str())
            .collect();
        assert!(
            reasons.contains(&"water_intersection"),
            "the water refusal must be recorded: {reasons:?}"
        );
        let refused_square = out.rejections.iter().any(|rejection| {
            rejection.reason == "solid_object_occupies_residual_land"
                && rejection.x == i64::from(tx)
                && rejection.z == i64::from(tz)
        });
        assert!(
            refused_square,
            "the square the tree occupies must be refused by name"
        );
        // Nowhere did the public-space share claim a conservation cell, and no
        // square is owned by two realized patches at once.
        let conservation_ids: std::collections::BTreeSet<&str> = p
            .cells
            .iter()
            .filter(|cell| cell.role == LandcoverRole::ConservationCandidate)
            .map(|cell| cell.id.as_str())
            .collect();
        let mut claimed = std::collections::BTreeSet::new();
        for patch in &out.patches {
            if patch.material == Material::PublicOpenSpace {
                assert!(!conservation_ids.contains(patch.source_cell_id.as_str()));
            }
            for square in patch.vertices.chunks_exact(4) {
                let cell = (square[0][0] as i64, square[0][2] as i64);
                assert!(
                    claimed.insert(cell),
                    "a metre square is realized by two patches at once: {cell:?}"
                );
            }
        }
        verify(&out, &s, &d, &p, &t, water, &e).expect("the refused open space still audits");
    }
}

/// Merge an independently verified surface result into the development status.
/// Remaining future districts/agriculture/conservation still prevent success.
pub fn apply_evidence(development: &mut development::DevelopmentAudit, surface: &SurfaceAudit) {
    if surface.unresolved_yard_m2 <= 1e-7 {
        development
            .incomplete_reasons
            .retain(|r| r != "yard_reserved_but_landcover_not_realized");
    }
    development.status = if !development.failures.is_empty() {
        "fail"
    } else if !development.incomplete_reasons.is_empty() {
        "incomplete"
    } else {
        "pass"
    }
    .into();
}

/// Farm service tracks count as transport, regardless of their source zoning.
pub fn farm_track_area(s: &SurfaceRealization) -> f64 {
    s.patches
        .iter()
        .filter(|p| p.material == Material::FarmTrack)
        .filter_map(|p| mesh_area(p).ok())
        .sum()
}
