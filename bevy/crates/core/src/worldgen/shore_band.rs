//! Independent audit of the shoreline public passage band.
//!
//! The band is the strip of dry ground within
//! [`super::water_query::WATER_SHORE_CLEARANCE_M`] of the water body: it is an
//! offset of the waterline, so it is continuous along every shore by
//! construction. The opt-in waterfront path keeps it for public passage - no lot
//! may be declared inside it, and its surface is realized as `PublicOpenSpace`
//! on the existing physical column - and this module is what stops that from
//! being a claim.
//!
//! Everything here is RECOMPUTED from published artifacts, never read from a
//! report:
//!
//!  * the band itself from the water geometry (the same predicate generation
//!    uses, so the two cannot drift);
//!  * every planned roadside pad's cells from the plan, checked for water and for
//!    band membership against the natural and final rasters;
//!  * every published surface patch's geometry and its cell, checked for
//!    continued band coverage, for a purpose that matches the zoning under it and
//!    for a square metre credited twice;
//!  * the ledger's public-open-space and access-pavement totals against the
//!    surfaces, because "in the ledger" is what makes an area count.
//!
//! A forgery fails BY NAME: a pad standing on water or on the band, a building
//! footprint inside the band, a band cell credited twice, a surface that
//! straddles the band boundary, a ledger total that disagrees with the geometry,
//! or a band with a hole in it.
use super::road_network::{RoadNetworkPlan, WATER_DATUM_MM};
use super::water_query::WATER_SHORE_CLEARANCE_M;
use super::{EntityInstance, LandUseZone, SceneSpec, WaterGeometry};
use crate::baked_terrain::BakedTerrain;
use crate::settlement::landcover::{Material, SurfacePatch, SurfaceRealization};
use crate::settlement::landuse_ledger::LandUseLedger;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Refusals a band cell may legally carry. Both keep the cell out of the
/// public promenade for a reason a reader can check:
///
///  * protected land keeps its conservation surface exactly as the protection
///    audit requires, and is still public green;
///  * a building PARCEL in the band is NOT an excuse - it is reported as a
///    failure by the audit, because a lot may never claim the reserved strip.
///    It is listed here only so the audit can name it instead of treating the
///    cell as a silent hole.
pub const SHORE_BAND_PROTECTED_REJECTION: &str = "shore_band_in_protected_land";
pub const SHORE_BAND_PARCEL_REJECTION: &str = "shore_band_inside_building_parcel";

/// The audit result, published beside the other scene audits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShoreBandAudit {
    pub status: String,
    /// the width of the reserved strip, metres (the one shore-clearance constant)
    pub clearance_m: f64,
    /// dry cells within the clearance of the water body
    pub band_cells: u64,
    pub band_area_m2: f64,
    /// realized public passage inside the band, by material
    pub public_open_space_m2: f64,
    pub access_pavement_m2: f64,
    pub passage_cells: u64,
    /// band cells covered by a road's own footprint: public passage too
    pub road_passage_cells: u64,
    pub protected_cells: u64,
    pub uncovered_cells: u64,
    /// planned roadside pad cells (all sites), and the violations among them
    pub pad_cells: u64,
    pub wet_pad_cells: u64,
    pub band_pad_cells: u64,
    /// connected public passage components per shore ("west"/"east" of a river,
    /// "shore" for a lake). Exactly one per shore that carries a band.
    pub passage_components: BTreeMap<String, u64>,
    pub failures: Vec<String>,
}

impl ShoreBandAudit {
    fn new() -> Self {
        Self {
            status: "pass".into(),
            clearance_m: WATER_SHORE_CLEARANCE_M,
            band_cells: 0,
            band_area_m2: 0.,
            public_open_space_m2: 0.,
            access_pavement_m2: 0.,
            passage_cells: 0,
            road_passage_cells: 0,
            protected_cells: 0,
            uncovered_cells: 0,
            pad_cells: 0,
            wet_pad_cells: 0,
            band_pad_cells: 0,
            passage_components: BTreeMap::new(),
            failures: vec![],
        }
    }
}

/// The whole audit. `surfaces` and `ledger` are optional so a caller that only
/// has the plan and the rasters can still prove the lots are dry and out of the
/// band; a published world always supplies both.
#[allow(clippy::too_many_arguments)]
pub fn audit(
    scene: &SceneSpec,
    water: WaterGeometry,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    plan: Option<&RoadNetworkPlan>,
    zones: &[LandUseZone],
    entities: &[EntityInstance],
    surfaces: Option<&SurfaceRealization>,
    ledger: Option<&LandUseLedger>,
) -> Result<ShoreBandAudit, String> {
    let mut audit = ShoreBandAudit::new();
    if !water.shore_band() {
        audit.status = "not_applicable".into();
        return Ok(audit);
    }
    if !natural.matches_scene(scene) {
        return Err("shore band natural raster does not match the scene".into());
    }
    if !final_terrain.matches_scene(scene) {
        return Err("shore band final raster does not match the scene".into());
    }
    // ---------------------------------------------------------------------
    // 1. The band, from the water geometry alone.
    // ---------------------------------------------------------------------
    let mut band: BTreeSet<(i32, i32)> = BTreeSet::new();
    for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
        for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
            if water.shore_band_cell(x, z) {
                band.insert((x, z));
            }
        }
    }
    audit.band_cells = band.len() as u64;
    audit.band_area_m2 = band.len() as f64; // one cell is one square metre
    if band.is_empty() {
        audit
            .failures
            .push("an opted-in waterfront scene carries no shore band at all".into());
    }
    let protected = |x: i32, z: i32| -> bool {
        zones.iter().any(|zone| {
            !matches!(zone.kind.as_str(), "urban" | "rural")
                && x >= zone.world_x
                && z >= zone.world_z
                && x < zone.world_x + zone.width_m as i32
                && z < zone.world_z + zone.depth_m as i32
        })
    };
    let zone_kind = |x: i32, z: i32| -> Option<&str> {
        zones
            .iter()
            .find(|zone| {
                x >= zone.world_x
                    && z >= zone.world_z
                    && x < zone.world_x + zone.width_m as i32
                    && z < zone.world_z + zone.depth_m as i32
            })
            .map(|zone| zone.kind.as_str())
    };
    // ---------------------------------------------------------------------
    // 2. Published band refusals: a legal excuse, or a named violation.
    // ---------------------------------------------------------------------
    let mut excused: BTreeSet<(i32, i32)> = BTreeSet::new();
    if let Some(surfaces) = surfaces {
        for rejection in &surfaces.rejections {
            let (Ok(x), Ok(z)) = (i32::try_from(rejection.x), i32::try_from(rejection.z)) else {
                continue;
            };
            match rejection.reason.as_str() {
                SHORE_BAND_PROTECTED_REJECTION => {
                    if !band.contains(&(x, z)) {
                        audit.failures.push(format!(
                            "shore band rejection at ({x},{z}) names a cell outside the band"
                        ));
                    } else if !protected(x, z) {
                        audit.failures.push(format!(
                            "shore band cell ({x},{z}) claims protection but stands in {} zoning",
                            zone_kind(x, z).unwrap_or("unassigned")
                        ));
                    } else {
                        excused.insert((x, z));
                    }
                }
                SHORE_BAND_PARCEL_REJECTION => {
                    audit.failures.push(format!(
                        "a building parcel stands inside the shore public band at ({x},{z})"
                    ));
                }
                _ => {}
            }
        }
    }
    // ---------------------------------------------------------------------
    // 3. Published surfaces: geometry, cell, purpose and double credit.
    // ---------------------------------------------------------------------
    let mut covered: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    let mut public_total = 0_f64;
    let mut pavement_total = 0_f64;
    if let Some(surfaces) = surfaces {
        let mut ids = BTreeSet::new();
        for patch in &surfaces.patches {
            if !ids.insert(patch.id.as_str()) {
                audit
                    .failures
                    .push(format!("duplicate surface id {}", patch.id));
            }
            let area = patch_area(patch).map_err(|e| format!("surface {}: {e}", patch.id))?;
            if !patch.area_m2.is_finite() || (area - patch.area_m2).abs() > 1e-6 {
                audit.failures.push(format!(
                    "surface {} declares {} m2 but its geometry covers {area} m2",
                    patch.id, patch.area_m2
                ));
            }
            match patch.material {
                Material::PublicOpenSpace => public_total += area,
                Material::AccessPavement => pavement_total += area,
                _ => {}
            }
            let Some(cell) = patch_cell(patch) else {
                // A patch that spans more than one square metre may not realize the
                // band: its area cannot be attributed to one band cell. It is refused
                // by name only when its own TRIANGLES actually cover a band cell, so
                // a multi-square public-open-space patch elsewhere in the scene is
                // never accused of claiming the shore.
                let claimed = band_claims(patch, &band);
                if let Some((bx, bz)) = claimed.first() {
                    audit.failures.push(format!(
                        "surface {} (material {:?}) covers the shore band cell ({bx},{bz}) without being realized per band cell, so its area cannot be attributed",
                        patch.id, patch.material
                    ));
                }
                continue;
            };
            if !band.contains(&cell) {
                continue;
            }
            let public = matches!(
                patch.material,
                Material::PublicOpenSpace | Material::AccessPavement
            );
            if !public {
                audit.failures.push(format!(
                    "surface {} claims shore band cell ({},{}) with material {:?} instead of public passage",
                    patch.id, cell.0, cell.1, patch.material
                ));
                continue;
            }
            if let Some(zone) = zone_kind(cell.0, cell.1) {
                if zone != patch.zone_kind {
                    audit.failures.push(format!(
                        "surface {} books the band cell ({},{}) against purpose {} but it stands in {}",
                        patch.id, cell.0, cell.1, patch.zone_kind, zone
                    ));
                }
            }
            *covered.entry(cell).or_insert(0) += 1;
            match patch.material {
                Material::PublicOpenSpace => audit.public_open_space_m2 += area,
                Material::AccessPavement => audit.access_pavement_m2 += area,
                _ => {}
            }
        }
    }
    for (cell, count) in &covered {
        if *count > 1 {
            audit.failures.push(format!(
                "shore band cell ({},{}) is credited {count} times: one square metre may carry one public surface",
                cell.0, cell.1
            ));
        }
    }
    // ---------------------------------------------------------------------
    // 4. Road pavement inside the band is public passage too.
    // ---------------------------------------------------------------------
    let mut road_passage: BTreeSet<(i32, i32)> = BTreeSet::new();
    for cell in &band {
        if covered.contains_key(cell) {
            continue;
        }
        if entities
            .iter()
            .filter(|entity| matches!(entity.kind.as_str(), "road" | "sidewalk" | "causeway"))
            .any(|entity| footprint_overlaps_cell(entity, cell.0, cell.1))
        {
            road_passage.insert(*cell);
        }
    }
    audit.road_passage_cells = road_passage.len() as u64;
    // ---------------------------------------------------------------------
    // 5. Every band cell is public passage, protected land, or a named hole.
    // ---------------------------------------------------------------------
    let mut holes: Vec<(i32, i32)> = Vec::new();
    for cell in &band {
        if protected(cell.0, cell.1) || excused.contains(cell) {
            audit.protected_cells += 1;
        } else if covered.contains_key(cell) || road_passage.contains(cell) {
            audit.passage_cells += 1;
        } else {
            holes.push(*cell);
        }
    }
    audit.uncovered_cells = holes.len() as u64;
    if let Some(cell) = holes.first() {
        audit.failures.push(format!(
            "the shore public band carries no public passage surface on {} cell(s), starting at ({},{})",
            holes.len(),
            cell.0,
            cell.1
        ));
    }
    // ---------------------------------------------------------------------
    // 6. Continuity: exactly one connected public passage component per shore.
    // ---------------------------------------------------------------------
    let mut passage: BTreeSet<(i32, i32)> = covered.keys().copied().collect();
    passage.extend(road_passage.iter().copied());
    passage.extend(
        band.iter()
            .filter(|cell| protected(cell.0, cell.1) || excused.contains(*cell))
            .copied(),
    );
    let shore_of = |cell: (i32, i32)| -> String {
        if water.kind == super::WaterKind::River {
            if f64::from(cell.0) + 0.5 < water.center_x {
                "west".into()
            } else {
                "east".into()
            }
        } else {
            "shore".into()
        }
    };
    let mut shore_has_band: BTreeMap<String, bool> = BTreeMap::new();
    for cell in &band {
        shore_has_band.insert(shore_of(*cell), true);
    }
    let mut seen: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut components: BTreeMap<String, u64> = BTreeMap::new();
    for start in &passage {
        if seen.contains(start) {
            continue;
        }
        let shore = shore_of(*start);
        *components.entry(shore).or_insert(0) += 1;
        let mut queue = VecDeque::from([*start]);
        seen.insert(*start);
        while let Some((x, z)) = queue.pop_front() {
            for next in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                if passage.contains(&next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }
    for (shore, _) in shore_has_band {
        let count = components.get(&shore).copied().unwrap_or(0);
        audit.passage_components.insert(shore.clone(), count);
        if count != 1 {
            audit.failures.push(format!(
                "the shore public band on the {shore} shore is not continuous: {count} public passage component(s) instead of 1"
            ));
        }
    }
    // ---------------------------------------------------------------------
    // 7. Planned pads: dry ground, out of the band, and never a building in it.
    // ---------------------------------------------------------------------
    if let Some(plan) = plan {
        for site in &plan.roadside_sites {
            let (half_w, half_d) = site.world_half_extents();
            let (mut wet, mut in_band) = (0_u64, 0_u64);
            for (x, z) in cells_between(
                site.world_x - half_w as i32,
                site.world_z - half_d as i32,
                half_w as i32,
                half_d as i32,
            ) {
                audit.pad_cells += 1;
                let blocked = water.contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5)
                    || natural.height(x, z).is_some_and(|h| {
                        h < WATER_DATUM_MM as f32 / 1000.
                    });
                if blocked {
                    wet += 1;
                }
                if band.contains(&(x, z)) {
                    in_band += 1;
                }
            }
            audit.wet_pad_cells += wet;
            audit.band_pad_cells += in_band;
            if wet > 0 {
                audit.failures.push(format!(
                    "pad {} stands on water at {wet} of its own cells",
                    site.id
                ));
            }
            if in_band > 0 {
                audit.failures.push(format!(
                    "pad {} enters the reserved shore public band at {in_band} of its own cells",
                    site.id
                ));
            }
        }
    }
    for entity in entities {
        if !crate::settlement::is_building(&entity.kind) {
            continue;
        }
        let (half_w, half_d) = super::spatial_half_extents(
            entity.width_m,
            entity.depth_m,
            entity.rotation_y_deg,
        );
        let cells: Vec<(i32, i32)> = cells_between(
            entity.world_x - half_w as i32,
            entity.world_z - half_d as i32,
            half_w as i32,
            half_d as i32,
        )
        .collect();
        if let Some(cell) = cells.iter().find(|cell| band.contains(cell)) {
            audit.failures.push(format!(
                "building {} ({}) stands inside the shore public band at ({},{})",
                entity.entity_id, entity.kind, cell.0, cell.1
            ));
        }
    }
    // ---------------------------------------------------------------------
    // 8. The area is booked: the ledger and the published geometry must agree.
    // ---------------------------------------------------------------------
    if let Some(ledger) = ledger {
        let mut ledger_public = 0_f64;
        let mut ledger_pavement = 0_f64;
        for row in ledger.by_purpose.values() {
            ledger_public += row.physical.public_open_space_m2;
            ledger_pavement += row.physical.access_pavement_m2;
        }
        if (ledger_public - public_total).abs() > 1e-6 {
            audit.failures.push(format!(
                "the ledger books {ledger_public} m2 of public open space while the published surfaces realize {public_total} m2"
            ));
        }
        if (ledger_pavement - pavement_total).abs() > 1e-6 {
            audit.failures.push(format!(
                "the ledger books {ledger_pavement} m2 of access pavement while the published surfaces realize {pavement_total} m2"
            ));
        }
        if audit.public_open_space_m2 + audit.access_pavement_m2 <= 0. {
            audit
                .failures
                .push("the shore public band carries no public passage surface".into());
        }
    }
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    Ok(audit)
}

/// The 1 m cells of a half-open rectangle: `x0..x0 + 2 * half_w`.
fn cells_between(
    x0: i32,
    z0: i32,
    half_w: i32,
    half_d: i32,
) -> impl Iterator<Item = (i32, i32)> {
    (0..2 * half_d.max(0)).flat_map(move |dz| {
        (0..2 * half_w.max(0)).map(move |dx| (x0 + dx, z0 + dz))
    })
}

/// Every band cell a patch's own triangles cover, judged at the cell centre.
///
/// The band's surface has to be realized per 1 m cell, so a patch that is not one
/// cell wide cannot be the band's surface. This asks the geometry directly instead
/// of the patch's bounding box: a multi-square public-open-space patch elsewhere in
/// the scene usually has the river inside its bounding box and must not be accused.
fn band_claims(patch: &SurfacePatch, band: &BTreeSet<(i32, i32)>) -> Vec<(i32, i32)> {
    let (mut min_x, mut min_z) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_z) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for vertex in &patch.vertices {
        if !vertex.iter().all(|value| value.is_finite()) {
            continue;
        }
        min_x = min_x.min(vertex[0]);
        min_z = min_z.min(vertex[2]);
        max_x = max_x.max(vertex[0]);
        max_z = max_z.max(vertex[2]);
    }
    if !min_x.is_finite() || !min_z.is_finite() {
        return vec![];
    }
    let mut claims = Vec::new();
    for (bx, bz) in band {
        let cx = f64::from(*bx) + 0.5;
        let cz = f64::from(*bz) + 0.5;
        if cx < min_x - 0.5 || cx > max_x + 0.5 || cz < min_z - 0.5 || cz > max_z + 0.5 {
            continue;
        }
        let covered = patch.triangles.iter().any(|triangle| {
            let mut points = [[0_f64; 2]; 3];
            let mut valid = true;
            for (index, corner) in triangle.iter().enumerate() {
                match patch.vertices.get(*corner as usize) {
                    Some(vertex) if vertex.iter().all(|value| value.is_finite()) => {
                        points[index] = [vertex[0], vertex[2]];
                    }
                    _ => valid = false,
                }
            }
            valid && inside_triangle(points, [cx, cz])
        });
        if covered {
            claims.push((*bx, *bz));
        }
    }
    claims
}

/// Is `point` inside (or on) the 2D triangle `points`?
fn inside_triangle(points: [[f64; 2]; 3], point: [f64; 2]) -> bool {
    let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| -> f64 {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    let d0 = edge(points[0], points[1], point);
    let d1 = edge(points[1], points[2], point);
    let d2 = edge(points[2], points[0], point);
    // Accept the triangle in either winding, and a point exactly on an edge.
    let positive = d0 >= -1e-9 && d1 >= -1e-9 && d2 >= -1e-9;
    let negative = d0 <= 1e-9 && d1 <= 1e-9 && d2 <= 1e-9;
    positive || negative
}

/// The single 1 m cell a patch covers, or `None` when it spans more than one cell
/// (the band's surface must be realized per band cell, so its area can be
/// attributed and never double credited).
///
/// The test is on the patch's own bounding box, not on its vertex floors: a 1 m
/// patch has vertices on the CELL CORNERS, so its four corners floor into two
/// different cells while the patch itself lies in exactly one.
fn patch_cell(patch: &SurfacePatch) -> Option<(i32, i32)> {
    let (mut min_x, mut min_z) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_z) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for vertex in &patch.vertices {
        if !vertex.iter().all(|value| value.is_finite()) {
            return None;
        }
        min_x = min_x.min(vertex[0]);
        min_z = min_z.min(vertex[2]);
        max_x = max_x.max(vertex[0]);
        max_z = max_z.max(vertex[2]);
    }
    if !min_x.is_finite() || !min_z.is_finite() {
        return None;
    }
    let x = min_x.floor();
    let z = min_z.floor();
    if max_x > x + 1. + 1e-9 || max_z > z + 1. + 1e-9 {
        return None;
    }
    Some((x as i32, z as i32))
}

/// A patch's area from its own triangles, in square metres. Recomputed rather
/// than trusted, because the ledger credits what the geometry covers.
fn patch_area(patch: &SurfacePatch) -> Result<f64, String> {
    let mut area = 0.;
    for triangle in &patch.triangles {
        let mut points = [[0_f64; 2]; 3];
        for (index, corner) in triangle.iter().enumerate() {
            let vertex = patch
                .vertices
                .get(*corner as usize)
                .ok_or_else(|| "triangle index outside the vertex list".to_string())?;
            if !vertex.iter().all(|value| value.is_finite()) {
                return Err("non-finite vertex".into());
            }
            points[index] = [vertex[0], vertex[2]];
        }
        let signed = (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
            - (points[1][1] - points[0][1]) * (points[2][0] - points[0][0]);
        area += signed.abs() * 0.5;
    }
    Ok(area)
}

/// Exact footprint test for a (possibly quarter-turned) entity and one 1 m cell,
/// by separating axes: the rectangle's own axes and the world axes.
fn footprint_overlaps_cell(entity: &EntityInstance, x: i32, z: i32) -> bool {
    // Local half extents: the entity's own width/depth, before any rotation.
    let half_w = f64::from(entity.width_m) * 0.5;
    let half_d = f64::from(entity.depth_m) * 0.5;
    if ![half_w, half_d, f64::from(entity.rotation_y_deg)]
        .iter()
        .all(|value| value.is_finite())
        || half_w <= 0.
        || half_d <= 0.
    {
        return true;
    }
    let (cx, cz) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
    let dx = cx - f64::from(entity.world_x);
    let dz = cz - f64::from(entity.world_z);
    let angle = f64::from(entity.rotation_y_deg).to_radians();
    let (c, s) = (angle.cos(), angle.sin());
    // World axes.
    let projected_x = half_w * c.abs() + half_d * s.abs();
    let projected_z = half_w * s.abs() + half_d * c.abs();
    if dx.abs() >= projected_x + 0.5 || dz.abs() >= projected_z + 0.5 {
        return false;
    }
    // The rectangle's own axes.
    let local_x = dx * c - dz * s;
    let local_z = dx * s + dz * c;
    local_x.abs() < half_w + 0.5 * (c.abs() + s.abs())
        && local_z.abs() < half_d + 0.5 * (c.abs() + s.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;
    use crate::settlement::landcover::{Material, SurfacePatch, SurfaceRealization};
    use crate::settlement::landuse_ledger::{LandUseLedger, PhysicalCover, PurposeAccount};
    use crate::worldgen::{GroundingSpec, LandUseZone, SceneSpec, WaterKind};
    use std::collections::BTreeMap;

    const HALF: i32 = 128;

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "band".into(),
            width_m: (2 * HALF) as u32,
            depth_m: (2 * HALF) as u32,
            origin_x: -HALF,
            origin_z: -HALF,
            seed_offset: 0,
        }
    }

    /// A 24 m channel down the middle of the scene, with the opt-in switch on.
    fn water(opt_in: bool) -> WaterGeometry {
        let mut water = super::super::water_geometry(
            WaterKind::River,
            &[],
            &scene(),
            &serde_json::Value::Null,
        );
        water.center_x = 0.;
        water.center_z = 0.;
        water.half_width = 12.;
        water.half_depth = f64::from(HALF);
        water.road_water_crossings = opt_in;
        water
    }

    fn terrain(wet: bool) -> BakedTerrain {
        let scene = scene();
        let values: Vec<f64> = (0..2 * HALF)
            .flat_map(|z| {
                (0..2 * HALF).map(move |x| {
                    let inside = ((x - HALF) as f64).abs() < 12. && wet;
                    if inside { -1.5 } else { 1.0 }
                })
            })
            .collect();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                -HALF,
                -HALF,
                2 * HALF as u32,
                2 * HALF as u32,
                &encode_millimetres(2 * HALF as u32, 2 * HALF as u32, values).unwrap(),
            )
            .unwrap();
        terrain
    }

    fn zones() -> Vec<LandUseZone> {
        vec![
            LandUseZone {
                id: "west".into(),
                kind: "urban".into(),
                world_x: -HALF,
                world_z: -HALF,
                width_m: HALF as u32,
                depth_m: (2 * HALF) as u32,
                source: "test".into(),
            },
            LandUseZone {
                id: "east".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: -HALF,
                width_m: HALF as u32,
                depth_m: (2 * HALF) as u32,
                source: "test".into(),
            },
        ]
    }

    fn site(id: &str, x: i32, z: i32) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "roadId": "road",
            "worldX": x,
            "worldZ": z,
            "widthM": 24,
            "depthM": 24,
            "supportLevelM": 5,
            "supportLevelMm": 5000,
            "frontageRoadPoint": [x, z - 24],
            "rotationYDeg": 0.,
            "naturalMinM": 1.,
            "naturalMaxM": 1.,
        })
    }

    fn plan(sites: Vec<serde_json::Value>) -> RoadNetworkPlan {
        serde_json::from_value(serde_json::json!({
            "model": "band-test",
            "naturalHeightfieldHash": "test",
            "sceneId": "band",
            "status": "planned_connected_network",
            "gridSpacingM": 64,
            "maxRoadbedFillM": 6.,
            "levelM": 5,
            "candidates": 1,
            "rejectedWater": 0,
            "rejectedRelief": 0,
            "unselectedCandidates": 0,
            "nodes": [],
            "edges": [],
            "roadsideSites": sites,
        }))
        .unwrap()
    }

    fn building(id: &str, x: i32, z: i32) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: "prop.building".into(),
            kind: "urban_building".into(),
            world_x: x,
            world_z: z,
            world_y: 1,
            width_m: 10.,
            depth_m: 10.,
            height_m: 6.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        }
    }

    fn band_cell_patch(x: i32, z: i32) -> SurfacePatch {
        SurfacePatch {
            id: format!("band.{x}.{z}"),
            source_cell_id: "cell".into(),
            parcel_id: "band".into(),
            zone_kind: if x < 0 { "urban".into() } else { "rural".into() },
            material: Material::PublicOpenSpace,
            area_m2: 1.,
            vertices: vec![
                [x as f64, 1., z as f64],
                [x as f64 + 1., 1., z as f64],
                [x as f64 + 1., 1., z as f64 + 1.],
                [x as f64, 1., z as f64 + 1.],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    /// The band surface realized exactly as the production pass does it: one
    /// public-open-space square per band cell.
    fn surfaces(water: WaterGeometry) -> SurfaceRealization {
        let mut patches = Vec::new();
        for z in -HALF..HALF {
            for x in -HALF..HALF {
                if water.shore_band_cell(x, z) {
                    patches.push(band_cell_patch(x, z));
                }
            }
        }
        SurfaceRealization {
            version: 2,
            scene_id: "band".into(),
            final_heightfield_hash: "test".into(),
            patches,
            rejections: vec![],
            agriculture: Default::default(),
            ecology: Default::default(),
        }
    }

    fn ledger(public_open_space_m2: f64) -> LandUseLedger {
        let mut by_purpose = BTreeMap::new();
        for kind in ["urban", "rural"] {
            by_purpose.insert(
                kind.to_string(),
                PurposeAccount {
                    target_ratio: 0.5,
                    target_m2: 0.,
                    planned_m2: (2 * HALF * HALF) as f64,
                    planned_scene_ratio: 0.5,
                    realized_m2: public_open_space_m2 / 2.,
                    realized_scene_ratio: 0.5,
                    unresolved_m2: 0.,
                    target_deficit_m2: 0.,
                    target_excess_m2: 0.,
                    physical: PhysicalCover {
                        public_open_space_m2: public_open_space_m2 / 2.,
                        ..Default::default()
                    },
                },
            );
        }
        LandUseLedger {
            version: 1,
            accounting: "disjoint-purpose-cover-v1".into(),
            scene_id: "band".into(),
            scene_area_m2: (4 * HALF * HALF) as f64,
            by_purpose,
            realized_m2: public_open_space_m2,
            unresolved_m2: 0.,
            transport_subset_m2: 0.,
            conversions: vec![],
            residual: None,
            allocation_status: "pass".into(),
            realization_status: "incomplete".into(),
            status: "incomplete".into(),
        }
    }

    fn audit_with(
        surfaces: Option<&SurfaceRealization>,
        plan: Option<&RoadNetworkPlan>,
        entities: &[EntityInstance],
        ledger: Option<&LandUseLedger>,
        opt_in: bool,
    ) -> ShoreBandAudit {
        let scene = scene();
        let water = water(opt_in);
        // The natural raster has the bed under the water and dry ground beside it;
        // the final raster is the same, because a waterfront bake fills nothing.
        let natural = terrain(true);
        let final_terrain = terrain(true);
        audit(
            &scene,
            water,
            &natural,
            &final_terrain,
            plan,
            &zones(),
            entities,
            surfaces,
            ledger,
        )
        .unwrap()
    }

    #[test]
    fn a_dry_lot_and_a_continuous_public_band_pass_the_independent_audit() {
        let water = water(true);
        let surfaces = surfaces(water);
        let plan = plan(vec![site("pad.dry", 40, 0)]);
        let ledger = ledger(surfaces.patches.len() as f64);
        let audit = audit_with(Some(&surfaces), Some(&plan), &[], Some(&ledger), true);
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.clearance_m, WATER_SHORE_CLEARANCE_M);
        // 24 m channel: 4 m of dry band on each bank over the full depth.
        assert_eq!(audit.band_cells, 2 * 4 * 2 * HALF as u64);
        assert_eq!(audit.uncovered_cells, 0);
        assert_eq!(audit.public_open_space_m2, audit.band_cells as f64);
        assert_eq!(audit.passage_components["west"], 1);
        assert_eq!(audit.passage_components["east"], 1);
        assert!(audit.wet_pad_cells == 0 && audit.band_pad_cells == 0);
    }

    #[test]
    fn a_lot_standing_on_water_or_in_the_band_fails_by_name() {
        let water = water(true);
        let surfaces = surfaces(water);
        let ledger = ledger(surfaces.patches.len() as f64);
        // Centred in the channel.
        let wet = plan(vec![site("pad.wet", 0, 0)]);
        let audit = audit_with(Some(&surfaces), Some(&wet), &[], Some(&ledger), true);
        assert_eq!(audit.status, "fail");
        assert!(audit.wet_pad_cells > 0);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("pad.wet stands on water")),
            "{:?}",
            audit.failures
        );
        // Standing on the west bank inside the reserved strip.
        let band = plan(vec![site("pad.band", -14, 0)]);
        let audit = audit_with(Some(&surfaces), Some(&band), &[], Some(&ledger), true);
        assert_eq!(audit.status, "fail");
        assert!(audit.band_pad_cells > 0);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("pad.band enters the reserved shore public band")),
            "{:?}",
            audit.failures
        );
    }

    #[test]
    fn a_building_inside_the_band_fails_by_name() {
        let water = water(true);
        let surfaces = surfaces(water);
        let ledger = ledger(surfaces.patches.len() as f64);
        let plan = plan(vec![site("pad.dry", 40, 0)]);
        let audit = audit_with(
            Some(&surfaces),
            Some(&plan),
            &[building("generated.house", -14, 0)],
            Some(&ledger),
            true,
        );
        assert_eq!(audit.status, "fail");
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("generated.house") && f.contains("shore public band")),
            "{:?}",
            audit.failures
        );
    }

    #[test]
    fn a_square_metre_credited_twice_and_an_inflated_ledger_fail_by_name() {
        let water = water(true);
        let mut surfaces = surfaces(water);
        // A second public surface on a band cell that already has one.
        let duplicate = surfaces.patches[0].clone();
        surfaces.patches.push(SurfacePatch {
            id: "band.duplicate".into(),
            ..duplicate
        });
        let plan = plan(vec![site("pad.dry", 40, 0)]);
        let honest = ledger(surfaces.patches.len() as f64 - 1.);
        let audit = audit_with(Some(&surfaces), Some(&plan), &[], Some(&honest), true);
        assert_eq!(audit.status, "fail");
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("credited 2 times")),
            "{:?}",
            audit.failures
        );
        // A ledger that books more public open space than the surfaces realize.
        let mut honest = honest.clone();
        for row in honest.by_purpose.values_mut() {
            row.physical.public_open_space_m2 += 10.;
        }
        let audit = audit_with(Some(&surfaces), Some(&plan), &[], Some(&honest), true);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("the ledger books")),
            "{:?}",
            audit.failures
        );
    }

    #[test]
    fn a_hole_in_the_band_breaks_its_continuity_by_name() {
        let water = water(true);
        let mut surfaces = surfaces(water);
        // Cut the whole 4 m cross-section of the east bank at one station, which is
        // what an interruption of the promenade actually looks like: a single
        // missing square metre is still walkable around.
        let cut: Vec<(i32, i32)> = (12..16).map(|x| (x, 0)).collect();
        for cell in &cut {
            assert!(water.shore_band_cell(cell.0, cell.1));
        }
        surfaces
            .patches
            .retain(|patch| !cut.contains(&patch_cell(patch).unwrap()));
        let plan = plan(vec![site("pad.dry", 40, 0)]);
        let ledger = ledger(surfaces.patches.len() as f64);
        let audit = audit_with(Some(&surfaces), Some(&plan), &[], Some(&ledger), true);
        assert_eq!(audit.status, "fail");
        assert_eq!(audit.uncovered_cells, cut.len() as u64);
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("no public passage surface")),
            "{:?}",
            audit.failures
        );
        assert!(
            audit
                .failures
                .iter()
                .any(|f| f.contains("east shore is not continuous")),
            "{:?}",
            audit.failures
        );
    }

    #[test]
    fn without_the_switch_there_is_no_band_to_audit() {
        let audit = audit_with(None, None, &[], None, false);
        assert_eq!(audit.status, "not_applicable");
        assert_eq!(audit.band_cells, 0);
        assert!(audit.failures.is_empty());
    }
}

