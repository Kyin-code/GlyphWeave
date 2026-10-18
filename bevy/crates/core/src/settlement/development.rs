//! GlyphWeave-original planar ownership contract. Road-first parcels replace
//! zone-sized building proxies. Half-open integer rectangles conserve area;
//! zoning targets, building footprint and unfinished yard are distinct facts.
use crate::worldgen::{
    AccessGraph, EntityInstance, LandUseZone, SceneSpec,
    parcel_geometry::{self, ParcelInsets},
    road_network::RoadNetworkPlan,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rect {
    pub x: i64,
    pub z: i64,
    pub width_m: u32,
    pub depth_m: u32,
}
impl Rect {
    fn valid(self) -> bool {
        self.x >= i64::from(i32::MIN)
            && self.x <= i64::from(i32::MAX)
            && self.z >= i64::from(i32::MIN)
            && self.z <= i64::from(i32::MAX)
            && self.width_m > 0
            && self.width_m <= 6000
            && self.depth_m > 0
            && self.depth_m <= 10000
            && self.right() <= i64::from(i32::MAX)
            && self.bottom() <= i64::from(i32::MAX)
    }
    fn right(self) -> i64 {
        self.x + i64::from(self.width_m)
    }
    fn bottom(self) -> i64 {
        self.z + i64::from(self.depth_m)
    }
    pub fn area(self) -> u64 {
        u64::from(self.width_m) * u64::from(self.depth_m)
    }
    fn intersect(self, other: Self) -> Option<Self> {
        let (x, z) = (self.x.max(other.x), self.z.max(other.z));
        let (r, b) = (
            self.right().min(other.right()),
            self.bottom().min(other.bottom()),
        );
        (r > x && b > z).then(|| Self {
            x,
            z,
            width_m: (r - x) as u32,
            depth_m: (b - z) as u32,
        })
    }
    /// Area two rectangles share, in whole square metres. The purpose allocation
    /// (`worldgen::conversion_budget`) measures "how much urban land this rect
    /// commits" with exactly this arithmetic, so the two cannot disagree about a
    /// single square metre.
    pub(crate) fn intersection_area(self, other: Self) -> u64 {
        self.intersect(other).map_or(0, |shared| shared.area())
    }
    fn contains(self, other: Self) -> bool {
        self.intersect(other) == Some(other)
    }
    fn subtract(self, other: Self) -> Vec<Self> {
        let Some(i) = self.intersect(other) else {
            return vec![self];
        };
        let mut result = Vec::new();
        for (x, z, r, b) in [
            (self.x, self.z, self.right(), i.z),
            (self.x, i.bottom(), self.right(), self.bottom()),
            (self.x, i.z, i.x, i.bottom()),
            (i.right(), i.z, self.right(), i.bottom()),
        ] {
            if r > x && b > z {
                result.push(Self {
                    x,
                    z,
                    width_m: (r - x) as u32,
                    depth_m: (b - z) as u32,
                });
            }
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandcoverPlan {
    pub version: u32,
    pub source: String,
    pub scene_id: String,
    pub scene_area_m2: u64,
    pub cells: Vec<LandcoverCell>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub farm_pattern: Option<super::farm_program::FarmPattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_program: Option<super::residual_land::SceneResidualProgram>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandcoverCell {
    pub id: String,
    pub boundary: Rect,
    pub source_cell_id: String,
    pub zone_id: String,
    pub zone_kind: String,
    pub role: LandcoverRole,
    pub realization_status: String,
    pub realization_reason: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandcoverRole {
    ManagedYard,
    AgricultureCandidate,
    ConservationCandidate,
    FutureDevelopment,
    Unmanaged,
}
fn landcover_role(cell: &LandCell) -> LandcoverRole {
    match cell.role {
        LandRole::Yard => LandcoverRole::ManagedYard,
        LandRole::Unplanned => match cell.zone_kind.as_str() {
            "rural" => LandcoverRole::AgricultureCandidate,
            "nature" => LandcoverRole::ConservationCandidate,
            "urban" => LandcoverRole::FutureDevelopment,
            _ => LandcoverRole::Unmanaged,
        },
        _ => LandcoverRole::Unmanaged,
    }
}
pub fn plan_landcover(
    scene: &SceneSpec,
    development: &DevelopmentPlan,
) -> Result<LandcoverPlan, String> {
    if development.scene_id != scene.scene_id || development.scene_area_m2 != bounds(scene).area() {
        return Err("landcover source development mismatch".into());
    }
    let cells = development
        .cells
        .iter()
        .filter(|c| matches!(c.role, LandRole::Yard | LandRole::Unplanned))
        .enumerate()
        .map(|(i, c)| LandcoverCell {
            id: format!("{}.landcover.{}", scene.scene_id, i),
            boundary: c.boundary,
            source_cell_id: c.id.clone(),
            zone_id: c.zone_id.clone(),
            zone_kind: c.zone_kind.clone(),
            role: landcover_role(c),
            realization_status: "planned".into(),
            realization_reason: Some("surface-realization-contract-not-yet-implemented".into()),
        })
        .collect();
    let plan = LandcoverPlan {
        farm_pattern: None,
        residual_program: None,
        version: 1,
        source: "development-cells-v1".into(),
        scene_id: scene.scene_id.clone(),
        scene_area_m2: bounds(scene).area(),
        cells,
    };
    validate_landcover(&plan, scene, development)?;
    Ok(plan)
}
pub fn plan_landcover_with_program(
    scene: &SceneSpec,
    development: &DevelopmentPlan,
    style: &serde_json::Value,
) -> Result<LandcoverPlan, String> {
    let mut plan = plan_landcover(scene, development)?;
    plan.farm_pattern = super::farm_program::for_scene(style, scene)?;
    plan.residual_program = super::residual_land::for_scene(style, scene)?;
    validate_landcover(&plan, scene, development)?;
    Ok(plan)
}

pub fn validate_landcover(
    plan: &LandcoverPlan,
    scene: &SceneSpec,
    development: &DevelopmentPlan,
) -> Result<(), String> {
    if plan.version != 1
        || plan.scene_id != scene.scene_id
        || plan.scene_area_m2 != bounds(scene).area()
    {
        return Err("landcover header mismatch".into());
    }
    if let Some(pattern) = &plan.farm_pattern {
        pattern.validate()?;
        for id in pattern.zone_fields.keys() {
            if !development
                .cells
                .iter()
                .any(|c| &c.zone_id == id && c.zone_kind == "rural")
            {
                return Err("farm pattern references non-rural or unknown source zone".into());
            }
        }
    }
    if let Some(program) = &plan.residual_program {
        program.validate()?;
    }
    let source: BTreeMap<_, _> = development.cells.iter().map(|c| (&c.id, c)).collect();
    let mut ids = BTreeSet::new();
    let mut area = 0;
    let mut source_ids = BTreeSet::new();
    for c in &plan.cells {
        let src = source
            .get(&c.source_cell_id)
            .ok_or("landcover cell missing development source")?;
        if c.id.is_empty()
            || !ids.insert(&c.id)
            || !source_ids.insert(&c.source_cell_id)
            || !c.boundary.valid()
            || c.boundary != src.boundary
            || c.zone_id != src.zone_id
            || c.zone_kind != src.zone_kind
            || !matches!(src.role, LandRole::Yard | LandRole::Unplanned)
            || c.realization_status != "planned"
            || c.realization_reason.as_deref()
                != Some("surface-realization-contract-not-yet-implemented")
        {
            return Err("invalid landcover source or realization contract".into());
        }
        if c.role != landcover_role(src) {
            return Err("landcover role differs from source zone".into());
        }
        area += c.boundary.area();
    }
    let expected = development
        .cells
        .iter()
        .filter(|c| matches!(c.role, LandRole::Yard | LandRole::Unplanned))
        .map(|c| c.boundary.area())
        .sum::<u64>();
    if area != expected {
        return Err("landcover area is not conserved over managed remainder".into());
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandcoverAudit {
    pub status: String,
    pub planned_area_m2: u64,
    pub realized_area_m2: u64,
    pub unresolved_area_m2: u64,
    pub cells: usize,
    pub failures: Vec<String>,
}
/// Generation-time plan validation. Callers performing an independent audit
/// must use `audit_landcover` with an explicitly sourced expected policy.
pub fn validate_landcover_plan(
    record: &LandcoverPlan,
    scene: &SceneSpec,
    development: &DevelopmentPlan,
) -> Result<LandcoverAudit, String> {
    let mut expected = plan_landcover(scene, development)?;
    expected.farm_pattern = record.farm_pattern.clone();
    expected.residual_program = record.residual_program.clone();
    if serde_json::to_value(record).map_err(|e| e.to_string())?
        != serde_json::to_value(&expected).map_err(|e| e.to_string())?
    {
        return Err("landcover plan differs from independently rebuilt remainder".into());
    }
    validate_landcover(record, scene, development)?;
    let planned = record.cells.iter().map(|c| c.boundary.area()).sum::<u64>();
    Ok(LandcoverAudit {
        status: if planned == 0 { "pass" } else { "incomplete" }.into(),
        planned_area_m2: planned,
        realized_area_m2: 0,
        unresolved_area_m2: planned,
        cells: record.cells.len(),
        failures: Vec::new(),
    })
}

/// Independent landcover audit. `expected_residual` must come from the frozen
/// manifest/scene source, never from `record` itself.
pub fn audit_landcover(
    record: &LandcoverPlan,
    scene: &SceneSpec,
    development: &DevelopmentPlan,
    expected_residual: Option<&super::residual_land::SceneResidualProgram>,
) -> Result<LandcoverAudit, String> {
    let mut expected = plan_landcover(scene, development)?;
    expected.farm_pattern = record.farm_pattern.clone();
    expected.residual_program = expected_residual.cloned();
    if serde_json::to_value(record).map_err(|e| e.to_string())?
        != serde_json::to_value(&expected).map_err(|e| e.to_string())?
    {
        return Err("landcover plan differs from authoritative remainder/policy".into());
    }
    validate_landcover(record, scene, development)?;
    let planned = record.cells.iter().map(|c| c.boundary.area()).sum::<u64>();
    Ok(LandcoverAudit {
        status: if planned == 0 { "pass" } else { "incomplete" }.into(),
        planned_area_m2: planned,
        realized_area_m2: 0,
        unresolved_area_m2: planned,
        cells: record.cells.len(),
        failures: Vec::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevelopmentPlan {
    pub version: u32,
    pub source: String,
    pub scene_id: String,
    pub natural_heightfield_hash: String,
    pub scene_area_m2: u64,
    pub road_ids: Vec<String>,
    pub parcels: Vec<FrontageParcel>,
    pub buildings: Vec<BuildingPlan>,
    pub cells: Vec<LandCell>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontageParcel {
    pub id: String,
    pub road_id: String,
    pub building_id: String,
    pub boundary: Rect,
    pub footprint_m2: u64,
    pub yard_m2: u64,
    #[serde(default, skip_serializing_if = "ParcelInsets::is_legacy")]
    pub parcel_insets: ParcelInsets,
    /// A real lot may straddle the coarse zoning grid. Preserve exact source
    /// contributions instead of choosing a nearest zone and losing area.
    pub source_zones: BTreeMap<String, u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildingPlan {
    pub id: String,
    pub parcel_id: String,
    pub road_id: String,
    pub footprint: Rect,
    pub entity: EntityInstance,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandCell {
    pub id: String,
    pub zone_id: String,
    pub zone_kind: String,
    pub role: LandRole,
    pub owner_id: Option<String>,
    pub boundary: Rect,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandRole {
    Road,
    Building,
    Yard,
    Unplanned,
}
impl DevelopmentPlan {
    pub fn building_entities(&self) -> Vec<EntityInstance> {
        self.buildings.iter().map(|p| p.entity.clone()).collect()
    }
}
fn bounds(scene: &SceneSpec) -> Rect {
    Rect {
        x: i64::from(scene.origin_x),
        z: i64::from(scene.origin_z),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
    }
}
fn zone_rect(z: &LandUseZone) -> Rect {
    Rect {
        x: i64::from(z.world_x),
        z: i64::from(z.world_z),
        width_m: z.width_m,
        depth_m: z.depth_m,
    }
}
fn disjoint(rects: &[(String, Rect)]) -> Result<(), String> {
    let mut sorted: Vec<_> = rects.iter().collect();
    sorted.sort_by_key(|(_, r)| r.x);
    let mut active: Vec<&(String, Rect)> = Vec::new();
    for item in sorted {
        active.retain(|(_, r)| r.right() > item.1.x);
        if let Some((id, _)) = active.iter().find(|(_, r)| r.intersect(item.1).is_some()) {
            return Err(format!(
                "development ownership overlap: {} and {id}",
                item.0
            ));
        }
        active.push(item);
    }
    Ok(())
}

/// The roadbed rectangle `plan` partitions a zone with for one road entity.
///
/// Factored out so the purpose allocation (`worldgen::conversion_budget`) can
/// measure the same land the partition commits without re-deriving the geometry:
/// a road that is 8 m wide and 64 m long along z owns the same square metres in
/// both places, including the width/depth swap of a rotated road.
pub(crate) fn road_rect(entity: &EntityInstance) -> Rect {
    let (w, d) = if entity.rotation_y_deg == 0. {
        (entity.width_m as u32, entity.depth_m as u32)
    } else {
        (entity.depth_m as u32, entity.width_m as u32)
    };
    Rect {
        x: i64::from(entity.world_x) - i64::from(w / 2),
        z: i64::from(entity.world_z) - i64::from(d / 2),
        width_m: w,
        depth_m: d,
    }
}

/// The frontage-parcel rectangle `plan` partitions a zone with for one roadside
/// site, derived from the site's own exact building geometry and authored insets.
pub(crate) fn parcel_rect(
    site: &crate::worldgen::road_network::RoadsideSite,
) -> Result<Rect, String> {
    let p = parcel_geometry::boundary_for_entity(
        &site.entity(),
        site.parcel_insets.unwrap_or_default(),
    )?;
    Ok(Rect {
        x: p[0],
        z: p[1],
        width_m: (p[2] - p[0]) as u32,
        depth_m: (p[3] - p[1]) as u32,
    })
}

/// Deterministic pre-generation plan. The road plan must first be verified
/// against its natural raster at the caller's trust boundary.
pub fn plan(
    scene: &SceneSpec,
    zones: &[LandUseZone],
    roads: &RoadNetworkPlan,
) -> Result<DevelopmentPlan, String> {
    let scene_rect = bounds(scene);
    if !scene_rect.valid() || roads.scene_id != scene.scene_id {
        return Err("development scene mismatch/empty scene".into());
    }
    let mut zone_ids = BTreeSet::new();
    let mut zone_area = 0;
    for z in zones {
        if z.id.is_empty()
            || !zone_ids.insert(&z.id)
            || zone_rect(z).area() == 0
            || !scene_rect.contains(zone_rect(z))
        {
            return Err("invalid development source zone".into());
        }
        zone_area += zone_rect(z).area();
    }
    disjoint(
        &zones
            .iter()
            .map(|z| (z.id.clone(), zone_rect(z)))
            .collect::<Vec<_>>(),
    )?;
    if zone_area != scene_rect.area() {
        return Err("development source zones do not cover scene".into());
    }
    let mut result = DevelopmentPlan {
        version: 1,
        source: "road-frontage-ownership-v1".into(),
        scene_id: scene.scene_id.clone(),
        natural_heightfield_hash: roads.natural_heightfield_hash.clone(),
        scene_area_m2: scene_rect.area(),
        road_ids: roads.edges.iter().map(|e| e.id.clone()).collect(),
        parcels: vec![],
        buildings: vec![],
        cells: vec![],
    };
    let mut owners = Vec::<(LandRole, String, Rect)>::new();
    let mut road_ids = BTreeSet::new();
    let mut road_rects = Vec::new();
    for e in roads.entities() {
        if !road_ids.insert(e.entity_id.clone())
            || (e.width_m != 64. && e.grounding.road_profile.is_none())
            || !matches!(e.depth_m, 8. | 12. | 16.)
            || !matches!(e.rotation_y_deg, 0. | 90.)
        {
            return Err("invalid development source road".into());
        }
        if let Some(profile)=&e.grounding.road_profile {
            profile.validate_entity(&e).map_err(|error|error.to_string())?;
        }
        let r = road_rect(&e);
        if !scene_rect.contains(r) {
            return Err("road outside development scene".into());
        }
        road_rects.push(r);
        owners.push((LandRole::Road, e.entity_id, r));
    }
    let mut site_ids = BTreeSet::new();
    for (site, mut entity) in roads.roadside_sites.iter().zip(roads.site_entities()) {
        if !site_ids.insert(&site.id)
            || !road_ids.contains(&site.road_id)
            || site.width_m == 0
            || site.depth_m == 0
            || site.width_m > 128
            || site.depth_m > 128
            || site.width_m % 2 != 0
            || site.depth_m % 2 != 0
            || !matches!(site.rotation_y_deg, 0. | 90. | 180. | 270.)
        {
            return Err("invalid development source site".into());
        }
        let (hw, hd) = site.world_half_extents();
        let insets = site.parcel_insets.unwrap_or_default();
        let boundary = parcel_rect(site)?;
        if !scene_rect.contains(boundary)
            || road_rects.iter().any(|r| r.intersect(boundary).is_some())
        {
            return Err("frontage parcel intersects road or escapes scene".into());
        }
        let footprint = Rect {
            x: i64::from(site.world_x) - i64::from(hw),
            z: i64::from(site.world_z) - i64::from(hd),
            width_m: hw * 2,
            depth_m: hd * 2,
        };
        let parcel_id = format!("{}.parcel", site.id);
        let building_id = format!("{}.building", site.id);
        let source_zones = zones
            .iter()
            .filter_map(|z| {
                zone_rect(z)
                    .intersect(boundary)
                    .map(|r| (z.id.clone(), r.area()))
            })
            .collect();
        result.parcels.push(FrontageParcel {
            id: parcel_id.clone(),
            road_id: site.road_id.clone(),
            building_id: building_id.clone(),
            boundary,
            footprint_m2: footprint.area(),
            yard_m2: boundary.area() - footprint.area(),
            parcel_insets: insets,
            source_zones,
        });
        entity.normalize_spatial_semantics();
        result.buildings.push(BuildingPlan {
            id: building_id.clone(),
            parcel_id: parcel_id.clone(),
            road_id: site.road_id.clone(),
            footprint,
            entity,
        });
        owners.push((LandRole::Building, building_id, footprint));
        for yard in boundary.subtract(footprint) {
            owners.push((LandRole::Yard, parcel_id.clone(), yard));
        }
    }
    disjoint(
        &result
            .parcels
            .iter()
            .map(|p| (p.id.clone(), p.boundary))
            .collect::<Vec<_>>(),
    )?;
    // Resolve road intersections by stable edge order before assigning cells.
    // Every zone is partitioned completely, including unfinished remainder.
    for zone in zones {
        let mut remaining = vec![zone_rect(zone)];
        for (role, owner, rect) in &owners {
            if rect.intersect(zone_rect(zone)).is_none() {
                continue;
            }
            let mut next = Vec::new();
            for r in remaining {
                if let Some(i) = r.intersect(*rect) {
                    result.cells.push(LandCell {
                        id: format!("{}.land.{}", zone.id, result.cells.len()),
                        zone_id: zone.id.clone(),
                        zone_kind: zone.kind.clone(),
                        role: *role,
                        owner_id: Some(owner.clone()),
                        boundary: i,
                    });
                }
                next.extend(r.subtract(*rect));
            }
            remaining = next;
        }
        for r in remaining {
            result.cells.push(LandCell {
                id: format!("{}.land.{}", zone.id, result.cells.len()),
                zone_id: zone.id.clone(),
                zone_kind: zone.kind.clone(),
                role: LandRole::Unplanned,
                owner_id: None,
                boundary: r,
            });
        }
    }
    validate(&result, scene, zones)?;
    Ok(result)
}

pub fn validate(
    p: &DevelopmentPlan,
    scene: &SceneSpec,
    zones: &[LandUseZone],
) -> Result<(), String> {
    let scene_rect = bounds(scene);
    if !scene_rect.valid()
        || p.version != 1
        || p.scene_id != scene.scene_id
        || p.scene_area_m2 != scene_rect.area()
    {
        return Err("development header mismatch".into());
    }
    let mut zone_map = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for z in zones {
        if z.id.is_empty()
            || !ids.insert(&z.id)
            || !zone_rect(z).valid()
            || !scene_rect.contains(zone_rect(z))
        {
            return Err("invalid development zone".into());
        }
        zone_map.insert(&z.id, z);
    }
    disjoint(
        &zones
            .iter()
            .map(|z| (z.id.clone(), zone_rect(z)))
            .collect::<Vec<_>>(),
    )?;
    if zones.iter().map(|z| zone_rect(z).area()).sum::<u64>() != scene_rect.area() {
        return Err("development zone coverage mismatch".into());
    }
    let road_ids: BTreeSet<_> = p.road_ids.iter().collect();
    for id in &p.road_ids {
        if id.is_empty() || !ids.insert(id) {
            return Err("invalid development road reference".into());
        }
    }
    let parcels: BTreeMap<_, _> = p.parcels.iter().map(|p| (&p.id, p)).collect();
    let buildings: BTreeMap<_, _> = p.buildings.iter().map(|b| (&b.id, b)).collect();
    for parcel in &p.parcels {
        if parcel.id.is_empty()
            || !ids.insert(&parcel.id)
            || !parcel.boundary.valid()
            || !scene_rect.contains(parcel.boundary)
            || !road_ids.contains(&parcel.road_id)
            || parcel.footprint_m2.checked_add(parcel.yard_m2) != Some(parcel.boundary.area())
        {
            return Err("invalid frontage parcel".into());
        }
        let sources: BTreeMap<_, _> = zones
            .iter()
            .filter_map(|z| {
                zone_rect(z)
                    .intersect(parcel.boundary)
                    .map(|r| (z.id.clone(), r.area()))
            })
            .collect();
        if sources != parcel.source_zones {
            return Err("parcel zone contributions mismatch".into());
        }
        let b = buildings
            .get(&parcel.building_id)
            .ok_or("parcel missing building plan")?;
        let expected_boundary = parcel_geometry::boundary_for_entity(&b.entity, parcel.parcel_insets)?;
        if !b.footprint.valid()
            || b.parcel_id != parcel.id
            || b.road_id != parcel.road_id
            || !parcel.boundary.contains(b.footprint)
            || b.footprint.area() != parcel.footprint_m2
            || [parcel.boundary.x, parcel.boundary.z,
                parcel.boundary.x + i64::from(parcel.boundary.width_m),
                parcel.boundary.z + i64::from(parcel.boundary.depth_m)] != expected_boundary
        {
            return Err("building/parcel relationship mismatch".into());
        }
    }
    disjoint(
        &p.parcels
            .iter()
            .map(|p| (p.id.clone(), p.boundary))
            .collect::<Vec<_>>(),
    )?;
    let mut entity_ids = BTreeSet::new();
    for b in &p.buildings {
        if b.id.is_empty()
            || !ids.insert(&b.id)
            || !entity_ids.insert(&b.entity.entity_id)
            || road_ids.contains(&b.entity.entity_id)
            || !parcels
                .get(&b.parcel_id)
                .is_some_and(|p| p.building_id == b.id)
        {
            return Err("invalid building plan parent/id".into());
        }
    }
    let mut areas = BTreeMap::<&String, u64>::new();
    let mut owner_areas = BTreeMap::<&String, u64>::new();
    for c in &p.cells {
        let z = zone_map
            .get(&c.zone_id)
            .ok_or("development cell missing source zone")?;
        if c.id.is_empty()
            || !ids.insert(&c.id)
            || !c.boundary.valid()
            || !zone_rect(z).contains(c.boundary)
            || z.kind != c.zone_kind
        {
            return Err("invalid development land cell".into());
        }
        let valid_owner = match (&c.role, &c.owner_id) {
            (LandRole::Unplanned, None) => true,
            (LandRole::Road, Some(id)) => road_ids.contains(id),
            (LandRole::Building, Some(id)) => buildings
                .get(id)
                .is_some_and(|b| b.footprint.contains(c.boundary)),
            (LandRole::Yard, Some(id)) => parcels.get(id).is_some_and(|p| {
                p.boundary.contains(c.boundary)
                    && buildings
                        .get(&p.building_id)
                        .is_some_and(|b| b.footprint.intersect(c.boundary).is_none())
            }),
            _ => false,
        };
        if !valid_owner {
            return Err("invalid development cell owner".into());
        }
        *areas.entry(&c.zone_id).or_default() += c.boundary.area();
        if let Some(id) = &c.owner_id {
            *owner_areas.entry(id).or_default() += c.boundary.area();
        }
    }
    disjoint(
        &p.cells
            .iter()
            .map(|c| (c.id.clone(), c.boundary))
            .collect::<Vec<_>>(),
    )?;
    for (id, z) in zone_map {
        if areas.get(id).copied().unwrap_or(0) != zone_rect(z).area() {
            return Err("development zone area not conserved".into());
        }
    }
    for p in &p.parcels {
        if owner_areas.get(&p.id).copied().unwrap_or(0) != p.yard_m2
            || owner_areas.get(&p.building_id).copied().unwrap_or(0) != p.footprint_m2
        {
            return Err("development parcel ownership area mismatch".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentAudit {
    pub status: String,
    pub scene_area_m2: u64,
    pub partition_area_m2: u64,
    pub road_union_m2: u64,
    pub planned_building_footprint_m2: u64,
    pub reserved_yard_m2: u64,
    pub unplanned_m2: u64,
    pub planned_buildings: usize,
    pub realized_buildings: usize,
    pub zone_conflict_m2: u64,
    pub unplanned_building_ids: Vec<String>,
    pub failures: Vec<String>,
    pub incomplete_reasons: Vec<String>,
}
/// Independent reconstruction: persisted plan/status/area never supplies an
/// acceptance fact. Roads, foundations and pavement require their own audits.
pub fn verify(
    record: &DevelopmentPlan,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    roads: &RoadNetworkPlan,
    entities: &[EntityInstance],
    access: &AccessGraph,
) -> Result<DevelopmentAudit, String> {
    let expected = plan(scene, zones, roads)?;
    if serde_json::to_value(record).map_err(|e| e.to_string())?
        != serde_json::to_value(&expected).map_err(|e| e.to_string())?
    {
        return Err("development plan differs from independently rebuilt ownership".into());
    }
    let mut a = DevelopmentAudit {
        status: "incomplete".into(),
        scene_area_m2: expected.scene_area_m2,
        partition_area_m2: 0,
        road_union_m2: 0,
        planned_building_footprint_m2: 0,
        reserved_yard_m2: 0,
        unplanned_m2: 0,
        planned_buildings: expected.buildings.len(),
        realized_buildings: 0,
        zone_conflict_m2: 0,
        unplanned_building_ids: vec![],
        failures: vec![],
        incomplete_reasons: vec![],
    };
    for c in &expected.cells {
        let area = c.boundary.area();
        a.partition_area_m2 += area;
        match c.role {
            LandRole::Road => a.road_union_m2 += area,
            LandRole::Building => a.planned_building_footprint_m2 += area,
            LandRole::Yard => a.reserved_yard_m2 += area,
            LandRole::Unplanned => a.unplanned_m2 += area,
        }
        if matches!(c.role, LandRole::Building | LandRole::Yard)
            && !matches!(c.zone_kind.as_str(), "urban" | "rural")
        {
            a.zone_conflict_m2 += area;
        }
    }
    let mut actual = BTreeMap::<&str, Vec<&EntityInstance>>::new();
    for e in entities {
        actual.entry(&e.entity_id).or_default().push(e);
    }
    for b in &expected.buildings {
        let Some(found) = actual.get(b.entity.entity_id.as_str()) else {
            a.failures.push(format!("{}: missing_final_entity", b.id));
            continue;
        };
        if found.len() != 1 {
            a.failures.push(format!("{}: ambiguous_entity_id", b.id));
            continue;
        }
        let mut e = found[0].clone();
        e.bounds = None; // Derived bounds are not source geometry.
        let mut source = b.entity.clone();
        source.bounds = None;
        // Do NOT normalize or recreate missing entrances in an audit.
        if serde_json::to_value(&e).map_err(|e| e.to_string())?
            != serde_json::to_value(&source).map_err(|e| e.to_string())?
        {
            a.failures
                .push(format!("{}: entity_differs_from_building_plan", b.id));
            continue;
        }
        if !access.links.iter().any(|l| {
            l.entity_id == e.entity_id
                && l.connected
                && l.required
                && l.road_id.as_deref() == Some(b.road_id.as_str())
        }) {
            a.failures
                .push(format!("{}: no_verified_assigned_road_access", b.id));
            continue;
        }
        a.realized_buildings += 1;
    }
    let planned_ids: BTreeSet<_> = expected
        .buildings
        .iter()
        .map(|b| b.entity.entity_id.as_str())
        .collect();
    a.unplanned_building_ids = entities
        .iter()
        .filter(|e| super::is_building(&e.kind) && !planned_ids.contains(e.entity_id.as_str()))
        .map(|e| e.entity_id.clone())
        .collect();
    if !a.unplanned_building_ids.is_empty() {
        a.failures.push("buildings_without_development_plan".into());
    }
    if a.zone_conflict_m2 > 0 {
        a.failures
            .push("residential parcel conflicts with nature/unassigned zoning".into());
    }
    if a.unplanned_m2 > 0 {
        a.incomplete_reasons
            .push("remaining_land_has_no_development_or_conservation_plan".into());
    }
    if a.reserved_yard_m2 > 0 {
        a.incomplete_reasons
            .push("yard_reserved_but_landcover_not_realized".into());
    }
    if a.planned_buildings == 0 {
        a.incomplete_reasons.push("no_planned_buildings".into());
    }
    a.status = if !a.failures.is_empty() {
        "fail"
    } else if !a.incomplete_reasons.is_empty() {
        "incomplete"
    } else {
        "pass"
    }
    .into();
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{
        AccessLink,
        road_network::{RoadEdge, RoadNode, RoadsideSite},
    };
    fn fixture() -> (SceneSpec, Vec<LandUseZone>, RoadNetworkPlan) {
        let scene = SceneSpec {
            scene_id: "test".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zones = (0..2)
            .map(|i| LandUseZone {
                id: format!("zone{i}"),
                kind: "urban".into(),
                world_x: i * 64,
                world_z: 0,
                width_m: 64,
                depth_m: 128,
                source: "test".into(),
            })
            .collect();
        let roads = RoadNetworkPlan {
            automatic_extensions: None,
            frontage: None,
            base_profile_rejections: Vec::new(),
            roadside_site_rejections: Vec::new(),
            extensions: vec![],
            model: "test".into(),
            natural_heightfield_hash: "source".into(),
            scene_id: "test".into(),
            status: "planned".into(),
            grid_spacing_m: 64,
            max_roadbed_fill_m: 6.,
            level_m: Some(0),
            candidates: 2,
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
            nodes: vec![RoadNode {
                id: 0,
                world_x: 32,
                world_z: 64,
            }],
            edges: vec![
                RoadEdge {
                    id: "road0".into(),
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
                },
                RoadEdge {
                    id: "road1".into(),
                    from: 1,
                    to: 2,
                    world_x: 96,
                    world_z: 64,
                    length_m: 64,
                    rotation_y_deg: 90.,
                    natural_min_m: 0.,
                    natural_max_m: 0.,
                    // B01-T2 mechanical sync: unprofiled legacy edge in this fixture
                    profile: None,
                },
            ],
            roadside_sites: vec![RoadsideSite {
                program: None,
                purpose: None,
                id: "generated.site".into(),
                road_id: "road0".into(),
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
        (scene, zones, roads)
    }
    fn access(p: &DevelopmentPlan) -> AccessGraph {
        AccessGraph {
            version: 2,
            connected_entities: p.buildings.len(),
            disconnected_entities: 0,
            links: p
                .buildings
                .iter()
                .map(|b| AccessLink {
                    entity_id: b.entity.entity_id.clone(),
                    anchor_id: "front".into(),
                    road_id: Some(b.road_id.clone()),
                    distance_m: Some(7.3),
                    connected: true,
                    required: true,
                    status: "walkable_route".into(),
                    path: vec![],
                })
                .collect(),
        }
    }
    #[test]
    fn full_ownership_conserves_area_without_double_counting_intersections() {
        let (s, z, r) = fixture();
        let p = plan(&s, &z, &r).unwrap();
        let a = verify(&p, &s, &z, &r, &p.building_entities(), &access(&p)).unwrap();
        assert_eq!(a.status, "incomplete");
        assert!(a.failures.is_empty());
        assert_eq!(a.partition_area_m2, 128 * 128);
        assert_eq!(a.road_union_m2, 992); // 2*512 - 4*8 crossing
        assert_eq!(a.planned_building_footprint_m2, 576);
        assert_eq!(a.reserved_yard_m2, 208);
        assert_eq!(a.unplanned_m2, 128 * 128 - 992 - 784);
        assert_eq!(a.realized_buildings, 1);
        assert_eq!(
            p.parcels[0]
                .source_zones
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![392, 392]
        );
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::to_value(plan(&s, &z, &r).unwrap()).unwrap()
        );
    }
    #[test]
    fn explicit_asymmetric_parcel_insets_are_authoritative_and_tamper_evident() {
        let (s, z, mut r) = fixture();
        r.roadside_sites[0].width_m = 12;
        r.roadside_sites[0].depth_m = 10;
        r.roadside_sites[0].parcel_insets = Some(ParcelInsets {
            front_m: 4,
            side_m: 4,
            rear_m: 12,
        });
        let p = plan(&s, &z, &r).unwrap();
        let parcel = &p.parcels[0];
        assert_eq!(
            [
                parcel.boundary.x,
                parcel.boundary.z,
                parcel.boundary.x + i64::from(parcel.boundary.width_m),
                parcel.boundary.z + i64::from(parcel.boundary.depth_m),
            ],
            [54, 79, 74, 105]
        );
        assert_eq!(parcel.boundary.area(), 520);
        assert_eq!(parcel.footprint_m2, 120);
        assert_eq!(parcel.yard_m2, 400);
        assert_eq!(parcel.source_zones.values().copied().collect::<Vec<_>>(), [260, 260]);
        let mut forged = p.clone();
        forged.parcels[0].boundary.x += 1;
        assert!(verify(
            &forged,
            &s,
            &z,
            &r,
            &forged.building_entities(),
            &access(&forged)
        )
        .is_err());
    }
    #[test]
    fn subtraction_partitions_all_overlap_cases() {
        let base = Rect {
            x: 0,
            z: 0,
            width_m: 10,
            depth_m: 10,
        };
        for x in -2..12 {
            for z in -2..12 {
                for w in 1..5 {
                    for d in 1..5 {
                        let cut = Rect {
                            x,
                            z,
                            width_m: w,
                            depth_m: d,
                        };
                        let pieces = base.subtract(cut);
                        let removed = base.intersect(cut).map_or(0, Rect::area);
                        assert_eq!(pieces.iter().map(|r| r.area()).sum::<u64>() + removed, 100);
                        assert!(
                            pieces
                                .iter()
                                .all(|r| base.contains(*r) && r.intersect(cut).is_none())
                        );
                        disjoint(
                            &pieces
                                .iter()
                                .enumerate()
                                .map(|(i, r)| (i.to_string(), *r))
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    #[test]
    fn invalid_source_zones_and_parcel_road_collisions_fail() {
        let (s, z, r) = fixture();
        let mut bad = z.clone();
        bad[1].world_x -= 1;
        assert!(plan(&s, &bad, &r).is_err());
        let mut bad = z.clone();
        bad[1].width_m -= 1;
        assert!(plan(&s, &bad, &r).is_err());
        let mut bad = r.clone();
        bad.roadside_sites[0].world_z = 64;
        assert!(plan(&s, &z, &bad).is_err());
        let mut bad = r.clone();
        bad.roadside_sites[0].road_id = "missing".into();
        assert!(plan(&s, &z, &bad).is_err());
        let mut bad = r.clone();
        bad.roadside_sites.push(bad.roadside_sites[0].clone());
        assert!(plan(&s, &z, &bad).is_err());
    }
    #[test]
    fn audit_rejects_geometry_parentage_missing_entrances_and_wrong_road() {
        let (s, z, r) = fixture();
        let p = plan(&s, &z, &r).unwrap();
        let entities = p.building_entities();
        let a = access(&p);
        let mut bad = p.clone();
        bad.buildings[0].parcel_id = "forged".into();
        assert!(verify(&bad, &s, &z, &r, &entities, &a).is_err());
        let mut bad = p.clone();
        bad.cells.retain(|c| c.role != LandRole::Unplanned);
        assert!(validate(&bad, &s, &z).is_err());
        assert!(verify(&bad, &s, &z, &r, &entities, &a).is_err());
        let mut bad = entities.clone();
        bad[0].anchors.clear();
        assert_eq!(verify(&p, &s, &z, &r, &bad, &a).unwrap().status, "fail");
        let mut bad = entities.clone();
        bad[0].world_x += 1;
        assert_eq!(verify(&p, &s, &z, &r, &bad, &a).unwrap().status, "fail");
        assert_eq!(
            verify(&p, &s, &z, &r, &[], &a).unwrap().realized_buildings,
            0
        );
        let mut bad = entities.clone();
        bad.push(entities[0].clone());
        assert_eq!(verify(&p, &s, &z, &r, &bad, &a).unwrap().status, "fail");
        let mut bad = a.clone();
        bad.links[0].road_id = Some("road1".into());
        assert_eq!(
            verify(&p, &s, &z, &r, &entities, &bad).unwrap().status,
            "fail"
        );
        let mut bad = entities.clone();
        bad[0].entity_id = "unplanned-house".into();
        let audit = verify(&p, &s, &z, &r, &bad, &a).unwrap();
        assert_eq!(audit.unplanned_building_ids, vec!["unplanned-house"]);
    }
    #[test]
    fn nature_zoning_conflict_is_not_relabelled_as_residential_success() {
        let (s, mut z, r) = fixture();
        z[0].kind = "nature".into();
        let p = plan(&s, &z, &r).unwrap();
        let a = verify(&p, &s, &z, &r, &p.building_entities(), &access(&p)).unwrap();
        assert_eq!(a.zone_conflict_m2, 392);
        assert_eq!(a.status, "fail");
    }
    #[test]
    fn ownership_validation_rejects_forged_cells_and_extreme_coordinates() {
        let (s, z, r) = fixture();
        let p = plan(&s, &z, &r).unwrap();
        let mut bad = p.clone();
        bad.cells[0].boundary.x = i64::MAX;
        assert!(validate(&bad, &s, &z).is_err());
        let mut bad = p.clone();
        bad.parcels[0].footprint_m2 = u64::MAX;
        assert!(validate(&bad, &s, &z).is_err());
        let mut bad = p.clone();
        bad.cells[0].owner_id = Some("forged".into());
        assert!(validate(&bad, &s, &z).is_err());
        let mut bad = p.clone();
        bad.parcels[0].source_zones.clear();
        assert!(validate(&bad, &s, &z).is_err());
        let mut bad = p.clone();
        bad.buildings[0].entity.entity_id = "road0".into();
        assert!(validate(&bad, &s, &z).is_err());
    }
    #[test]
    fn area_accounting_ignores_yards_unplanned_proxies_and_wrong_entities() {
        let (s, z, r) = fixture();
        let p = plan(&s, &z, &r).unwrap();
        let mut entities = r.entities();
        entities.extend(p.building_entities());
        assert_eq!(realized_footprint_area(&p, &r, &entities), 992 + 576);
        let graph = settlement_graph(&p, &entities);
        assert_eq!(graph.parcels[0].width_m, 28.);
        assert_eq!(graph.building_relations[0].parcel_id, p.parcels[0].id);
        assert_eq!(
            graph.building_relations[0].nearest_road_id.as_deref(),
            Some("road0")
        );
        let mut proxy = p.building_entities()[0].clone();
        proxy.kind = "farmland".into();
        proxy.entity_id = "unplanned-farmland".into();
        proxy.width_m = 128.;
        proxy.depth_m = 128.;
        entities.push(proxy);
        assert_eq!(realized_footprint_area(&p, &r, &entities), 992 + 576);
        entities[2].width_m += 1.;
        assert_eq!(realized_footprint_area(&p, &r, &entities), 992);
    }
    #[test]
    fn maximum_scene_partition_preserves_all_zones_and_undeveloped_land() {
        let (mut s, _, mut r) = fixture();
        s.width_m = 6000;
        s.depth_m = 10000;
        r.scene_id = s.scene_id.clone();
        let profile = crate::worldgen::LandUseProfile::from_style(
            &serde_json::json!({"landUseProfile":{"theme":"temperate-plain"}}),
        )
        .unwrap();
        let zones = crate::worldgen::plan_landuse_zones(&s, &profile);
        let p = plan(&s, &zones, &r).unwrap();
        assert_eq!(
            p.cells.iter().map(|c| c.boundary.area()).sum::<u64>(),
            60_000_000
        );
        assert!(p.cells.len() >= zones.len());
        assert_eq!(
            p.cells
                .iter()
                .filter(|c| c.role == LandRole::Unplanned)
                .map(|c| c.boundary.area())
                .sum::<u64>(),
            60_000_000 - 992 - 784
        );
        validate(&p, &s, &zones).unwrap();
    }

    #[test]
    fn landcover_candidates_preserve_remainder_without_claiming_realization() {
        let (s, z, r) = fixture();
        let p = plan(&s, &z, &r).unwrap();
        let lc = plan_landcover(&s, &p).unwrap();
        let audit = audit_landcover(&lc, &s, &p, None).unwrap();
        assert_eq!(audit.status, "incomplete");
        assert_eq!(audit.realized_area_m2, 0);
        assert_eq!(audit.unresolved_area_m2, 128 * 128 - 992 - 576);
        assert_eq!(
            lc.cells
                .iter()
                .filter(|c| c.role == LandcoverRole::ManagedYard)
                .map(|c| c.boundary.area())
                .sum::<u64>(),
            208
        );
        let mut bad = lc.clone();
        bad.cells[0].realization_status = "realized".into();
        assert!(validate_landcover(&bad, &s, &p).is_err());
        assert!(audit_landcover(&bad, &s, &p, None).is_err());
        let mut bad = lc.clone();
        bad.cells.pop();
        assert!(validate_landcover(&bad, &s, &p).is_err());
        let mut bad = lc.clone();
        bad.cells[0].source_cell_id = "fake".into();
        assert!(validate_landcover(&bad, &s, &p).is_err());
        let mut bad = lc.clone();
        let mut duplicate = lc.cells[0].clone();
        duplicate.id = "unique-but-duplicate-source".into();
        bad.cells.push(duplicate);
        assert!(validate_landcover(&bad, &s, &p).is_err());
        let mut bad = lc.clone();
        bad.cells[0].role = LandcoverRole::Unmanaged;
        assert!(audit_landcover(&bad, &s, &p, None).is_err());
    }
    #[test]
    fn landcover_does_not_invent_agriculture_or_conservation_implementation() {
        let (s, mut z, mut r) = fixture();
        r.edges.clear();
        r.roadside_sites.clear();
        z[0].kind = "rural".into();
        z[1].kind = "nature".into();
        let p = plan(&s, &z, &r).unwrap();
        let lc = plan_landcover(&s, &p).unwrap();
        assert_eq!(lc.cells[0].role, LandcoverRole::AgricultureCandidate);
        assert_eq!(lc.cells[1].role, LandcoverRole::ConservationCandidate);
        assert!(lc.cells.iter().all(|c| c.realization_status == "planned"));
        assert_eq!(
            audit_landcover(&lc, &s, &p, None).unwrap().unresolved_area_m2,
            128 * 128
        );
    }

    #[test]
    fn empty_network_keeps_the_whole_scene_unfinished() {
        let (s, z, mut r) = fixture();
        r.edges.clear();
        r.roadside_sites.clear();
        let p = plan(&s, &z, &r).unwrap();
        let a = verify(&p, &s, &z, &r, &[], &AccessGraph::default()).unwrap();
        assert_eq!(a.unplanned_m2, 128 * 128);
        assert_eq!(a.status, "incomplete");
    }
}

/// Land accounting counts only exact planned, present footprint ownership.
/// Reserved yard and unrelated legacy landcover proxies earn no realized area.
/// This is area evidence, not an access/foundation acceptance substitute.
#[derive(Debug, Default)]
pub struct OwnedAreas {
    pub road_m2: u64,
    pub urban_m2: u64,
    pub rural_m2: u64,
}
impl OwnedAreas {
    pub fn total(&self) -> u64 {
        self.road_m2 + self.urban_m2 + self.rural_m2
    }
}
#[cfg(test)]
fn realized_footprint_area(
    plan: &DevelopmentPlan,
    roads: &RoadNetworkPlan,
    entities: &[EntityInstance],
) -> u64 {
    realized_footprint_areas(plan, roads, entities).total()
}
/// Exact present owners shared by the compatibility budget and purpose ledger.
pub(super) fn realized_cells<'a>(
    plan: &'a DevelopmentPlan,
    roads: &RoadNetworkPlan,
    entities: &[EntityInstance],
) -> Vec<&'a LandCell> {
    let mut actual = BTreeMap::<&str, Vec<&EntityInstance>>::new();
    for e in entities {
        actual.entry(&e.entity_id).or_default().push(e);
    }
    let mut owners = BTreeSet::new();
    for (id, entity) in roads
        .entities()
        .into_iter()
        .map(|e| (e.entity_id.clone(), e))
        .chain(
            plan.buildings
                .iter()
                .map(|b| (b.id.clone(), b.entity.clone())),
        )
    {
        if let Some(found) = actual.get(entity.entity_id.as_str()) {
            if found.len() == 1 && same_entity(found[0], &entity) {
                owners.insert(id);
            }
        }
    }
    plan.cells
        .iter()
        .filter(|c| c.owner_id.as_ref().is_some_and(|id| owners.contains(id)))
        .collect()
}
pub fn realized_footprint_areas(
    plan: &DevelopmentPlan,
    roads: &RoadNetworkPlan,
    entities: &[EntityInstance],
) -> OwnedAreas {
    let mut areas = OwnedAreas::default();
    for c in realized_cells(plan, roads, entities) {
        match c.role {
            LandRole::Road => areas.road_m2 += c.boundary.area(),
            LandRole::Building if c.zone_kind == "urban" => areas.urban_m2 += c.boundary.area(),
            LandRole::Building if c.zone_kind == "rural" => areas.rural_m2 += c.boundary.area(),
            _ => {} // A building in nature/unassigned cannot earn land-use credit.
        }
    }
    areas
}
fn same_entity(a: &EntityInstance, b: &EntityInstance) -> bool {
    let (mut a, mut b) = (a.clone(), b.clone());
    a.bounds = None;
    b.bounds = None;
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Relationships use pre-generation parcel and assigned-road IDs, not nearest
/// entities. The graph is a plan view with observations, not a safety verdict.
pub fn settlement_graph(
    plan: &DevelopmentPlan,
    entities: &[EntityInstance],
) -> super::SettlementGraph {
    let settlement_id = format!("{}.settlement.road-network", plan.scene_id);
    let district_id = format!("{}.district.frontage", plan.scene_id);
    let mut graph = super::SettlementGraph {
        format: "glyphweave.settlement-graph".into(),
        version: 2,
        status: "planned-frontage-with-final-observations-v2".into(),
        ..Default::default()
    };
    let mut center_x = 0.;
    let mut center_z = 0.;
    for p in &plan.parcels {
        let b = plan
            .buildings
            .iter()
            .find(|b| b.id == p.building_id)
            .expect("validated plan building");
        let present = entities
            .iter()
            .filter(|e| e.entity_id == b.entity.entity_id)
            .count()
            == 1;
        let x = p.boundary.x as f64 + f64::from(p.boundary.width_m) * 0.5;
        let z = p.boundary.z as f64 + f64::from(p.boundary.depth_m) * 0.5;
        center_x += x;
        center_z += z;
        graph.parcels.push(super::ParcelNode {
            id: p.id.clone(),
            district_id: district_id.clone(),
            land_use: if b.entity.kind == "residential_home" {
                "residential".into()
            } else {
                b.entity.kind.clone()
            },
            center_x: x as f32,
            center_z: z as f32,
            width_m: p.boundary.width_m as f32,
            depth_m: p.boundary.depth_m as f32,
            entity_ids: if present {
                vec![b.entity.entity_id.clone()]
            } else {
                vec![]
            },
        });
        graph.building_relations.push(super::BuildingRelation {
            entity_id: b.entity.entity_id.clone(),
            settlement_id: settlement_id.clone(),
            district_id: district_id.clone(),
            parcel_id: p.id.clone(),
            nearest_road_id: Some(b.road_id.clone()),
            distance_to_road_m: None,
            relation_reason: "planned_frontage_assigned_road_not_nearest_search".into(),
        });
    }
    if !plan.parcels.is_empty() {
        let n = plan.parcels.len() as f64;
        graph.districts.push(super::DistrictNode {
            id: district_id.clone(),
            settlement_id: settlement_id.clone(),
            kind: "road_frontage_residential".into(),
            center_x: (center_x / n) as f32,
            center_z: (center_z / n) as f32,
            parcel_ids: plan.parcels.iter().map(|p| p.id.clone()).collect(),
        });
        graph.settlements.push(super::SettlementNode {
            id: settlement_id,
            kind: "planned_lowland_settlement".into(),
            center_x: (center_x / n) as f32,
            center_z: (center_z / n) as f32,
            district_ids: vec![district_id],
        });
    }
    graph
}
