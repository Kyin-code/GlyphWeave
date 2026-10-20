//! Explicit human-environment relationship graph.
//!
//! This first version is deliberately derived from final Rust-owned entities:
//! it records stable settlement, district, parcel, public-space and building
//! relations without letting the renderer invent urban structure.

pub mod agriculture;
pub mod development;
pub mod ecology;
pub mod ecology_habitat;
mod farm_access;
mod pedestrian_index;
pub mod farm_program;
pub mod landcover;
pub mod landuse_ledger;
pub mod morphology;
pub mod morphology_compile;
pub mod proposals;
pub mod residual_land;
pub mod service_relations;

use crate::worldgen::{EntityInstance, SceneSpec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParcelPlan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub source: String,
    #[serde(default)]
    pub zones: Vec<crate::worldgen::LandUseZone>,
    pub parcels: Vec<PlannedParcel>,
    pub public_spaces: Vec<PlannedPublicSpace>,
    pub development_envelopes: Vec<DevelopmentEnvelope>,
    pub infrastructure: InfrastructurePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InfrastructurePlan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub corridors: Vec<PlannedInfrastructureCorridor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedInfrastructureCorridor {
    pub id: String,
    pub zone_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub reserved_area_m2: u32,
    #[serde(default)]
    pub realization_status: String,
    #[serde(default)]
    pub accepted_entity_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentEnvelope {
    pub id: String,
    pub zone_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub frontage_reserved_m: u32,
    pub drainage_reserved_m: u32,
    pub public_space_reserved_m2: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedParcel {
    pub id: String,
    pub zone_id: String,
    pub land_use: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub frontage_reserved: bool,
    pub drainage_reserved: bool,
    #[serde(default)]
    pub realization_status: String,
    #[serde(default)]
    pub accepted_entity_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedPublicSpace {
    pub id: String,
    pub zone_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
}

/// Coordinates in planning records are minimum corners, not entity centers.
/// Reserves are geometric space, never proof of a constructed road or drain.
pub fn plan_parcels(zones: &[crate::worldgen::LandUseZone]) -> ParcelPlan {
    let mut plan = ParcelPlan {
        format: "glyphweave.parcel-plan".into(),
        version: 2,
        status: "planned-before-entity-generation-v2".into(),
        source: "zone-partition-v2".into(),
        zones: zones.to_vec(),
        infrastructure: InfrastructurePlan {
            format: "glyphweave.infrastructure-plan".into(),
            version: 2,
            status: "reserved-before-infrastructure-entities-v2".into(),
            corridors: Vec::new(),
        },
        ..Default::default()
    };
    for zone in zones {
        let urban = zone.kind == "urban";
        let reserve = zone.depth_m.min(if urban {
            12
        } else if zone.kind == "unassigned" {
            0
        } else {
            8
        });
        if reserve > 0 && zone.width_m > 0 {
            plan.infrastructure
                .corridors
                .push(PlannedInfrastructureCorridor {
                    id: format!(
                        "{}.{}",
                        zone.id,
                        if urban { "frontage" } else { "drainage" }
                    ),
                    zone_id: zone.id.clone(),
                    kind: if urban {
                        "road-frontage-reserve"
                    } else {
                        "drainage-reserve"
                    }
                    .into(),
                    world_x: zone.world_x,
                    world_z: zone.world_z,
                    width_m: zone.width_m,
                    depth_m: reserve,
                    reserved_area_m2: zone.width_m.saturating_mul(reserve),
                    realization_status: "planned".into(),
                    accepted_entity_ids: Vec::new(),
                    rejection_reason: None,
                });
        }
        // Coordinates are checked again by the validator before bake output.
        let ez = i64::from(zone.world_z) + i64::from(reserve);
        let Ok(ez) = i32::try_from(ez) else {
            continue;
        };
        let ed = zone.depth_m - reserve;
        let split = zone.width_m >= 200 && zone.depth_m >= 200;
        let public_depth = if urban && split { 20.min(ed) } else { 0 };
        plan.development_envelopes.push(DevelopmentEnvelope {
            id: format!("{}.development-envelope", zone.id),
            zone_id: zone.id.clone(),
            world_x: zone.world_x,
            world_z: ez,
            width_m: zone.width_m,
            depth_m: ed,
            frontage_reserved_m: if urban { reserve } else { 0 },
            drainage_reserved_m: if urban { 0 } else { reserve },
            public_space_reserved_m2: zone.width_m.saturating_mul(public_depth),
        });
        if public_depth > 0 {
            plan.public_spaces.push(PlannedPublicSpace {
                id: format!("{}.public-space", zone.id),
                zone_id: zone.id.clone(),
                kind: "neighborhood-green".into(),
                world_x: zone.world_x,
                world_z: ez,
                width_m: zone.width_m,
                depth_m: public_depth,
            });
        }
        if zone.kind == "unassigned" {
            continue;
        }
        let available_d = ed - public_depth;
        let count = if split { 2 } else { 1 };
        // Even metre dimensions have exactly representable integer centers.
        let numerator = if split { 42_u64 } else { 72_u64 };
        let w = ((u64::from(zone.width_m) * numerator / 100) as u32) & !1;
        let d = ((u64::from(available_d) * numerator / 100) as u32) & !1;
        if w < 40 || d < 40 || w * count > zone.width_m || d * count > available_d {
            continue;
        }
        let margin_x = (zone.width_m - count * w) / (count + 1);
        let margin_z = (available_d - count * d) / (count + 1);
        for row in 0..count {
            for col in 0..count {
                let x_offset = if col == 0 {
                    margin_x
                } else {
                    zone.width_m - margin_x - w
                };
                let z_offset = if row == 0 {
                    margin_z
                } else {
                    available_d - margin_z - d
                };
                let x = i64::from(zone.world_x) + i64::from(x_offset);
                let z = i64::from(ez) + i64::from(public_depth + z_offset);
                let (Ok(x), Ok(z)) = (i32::try_from(x), i32::try_from(z)) else {
                    continue;
                };
                plan.parcels.push(PlannedParcel {
                    id: format!("{}.parcel.{}", zone.id, row * count + col),
                    zone_id: zone.id.clone(),
                    land_use: zone.kind.clone(),
                    world_x: x,
                    world_z: z,
                    width_m: w,
                    depth_m: d,
                    frontage_reserved: urban,
                    drainage_reserved: !urban,
                    realization_status: "planned".into(),
                    accepted_entity_ids: Vec::new(),
                    rejection_reason: None,
                });
            }
        }
    }
    plan
}

#[derive(Clone, Copy, Debug)]
struct PlanRect {
    x: i64,
    z: i64,
    right: i64,
    bottom: i64,
}
impl PlanRect {
    fn new(x: i32, z: i32, w: u32, d: u32) -> Result<Self, String> {
        let r = Self {
            x: i64::from(x),
            z: i64::from(z),
            right: i64::from(x) + i64::from(w),
            bottom: i64::from(z) + i64::from(d),
        };
        if w == 0 || d == 0 || r.right > i64::from(i32::MAX) || r.bottom > i64::from(i32::MAX) {
            return Err("empty or unrepresentable planning rectangle".into());
        }
        Ok(r)
    }
    fn contains(self, b: Self) -> bool {
        self.x <= b.x && self.z <= b.z && self.right >= b.right && self.bottom >= b.bottom
    }
    fn overlaps(self, b: Self) -> bool {
        self.x < b.right && self.right > b.x && self.z < b.bottom && self.bottom > b.z
    }
    fn area(self) -> u64 {
        ((self.right - self.x) * (self.bottom - self.z)) as u64
    }
}

fn non_overlapping(mut rectangles: Vec<(&str, PlanRect)>) -> Result<(), String> {
    rectangles.sort_by_key(|(_, r)| r.x);
    let mut active: Vec<(&str, PlanRect)> = Vec::new();
    for (id, rect) in rectangles {
        active.retain(|(_, r)| r.right > rect.x);
        for (other, r) in &active {
            if rect.overlaps(*r) {
                return Err(format!("planning rectangles {id} and {other} overlap"));
            }
        }
        active.push((id, rect));
    }
    Ok(())
}

/// Checks source parentage, geometry and disjoint land consumption. This is
/// intentionally independent of placement success and rendering.
pub fn validate_parcel_plan(plan: &ParcelPlan) -> Result<(), String> {
    use std::collections::{BTreeMap, BTreeSet};
    if plan.version != 2 {
        return Err("unsupported parcel plan version; rebake required".into());
    }
    let mut ids = BTreeSet::new();
    let mut unique = |id: &str| -> Result<(), String> {
        if id.is_empty() || !ids.insert(id.to_owned()) {
            Err(format!("duplicate/empty planning id {id}"))
        } else {
            Ok(())
        }
    };
    let mut zones = BTreeMap::new();
    for z in &plan.zones {
        unique(&z.id)?;
        zones.insert(
            z.id.as_str(),
            PlanRect::new(z.world_x, z.world_z, z.width_m, z.depth_m)?,
        );
    }
    non_overlapping(zones.iter().map(|(id, r)| (*id, *r)).collect())?;
    let mut envelopes = BTreeMap::new();
    for e in &plan.development_envelopes {
        unique(&e.id)?;
        let z = zones
            .get(e.zone_id.as_str())
            .ok_or_else(|| format!("envelope {} has no zone", e.id))?;
        // A fully reserved tiny zone has an empty developable remainder.
        let rect = if e.depth_m == 0 {
            None
        } else {
            Some(PlanRect::new(e.world_x, e.world_z, e.width_m, e.depth_m)?)
        };
        let reserve = u64::from(e.frontage_reserved_m) + u64::from(e.drainage_reserved_m);
        if reserve > (z.bottom - z.z) as u64
            || i64::from(e.world_x) != z.x
            || i64::from(e.world_z) != z.z + reserve as i64
            || i64::from(e.width_m) != z.right - z.x
            || i64::from(e.depth_m) + reserve as i64 != z.bottom - z.z
        {
            return Err(format!(
                "envelope {} does not partition its source zone",
                e.id
            ));
        }
        if envelopes.insert(e.zone_id.as_str(), (rect, e)).is_some() {
            return Err("multiple envelopes for zone".into());
        }
    }
    if zones.len() != envelopes.len() {
        return Err("missing zone envelope".into());
    }
    let mut occupied = Vec::new();
    let mut reserve_areas = BTreeMap::<&str, u64>::new();
    let mut public_areas = BTreeMap::<&str, u64>::new();
    for c in &plan.infrastructure.corridors {
        unique(&c.id)?;
        let z = zones
            .get(c.zone_id.as_str())
            .ok_or("corridor has no source zone")?;
        let (e, _) = envelopes
            .get(c.zone_id.as_str())
            .ok_or("corridor has no envelope")?;
        let r = PlanRect::new(c.world_x, c.world_z, c.width_m, c.depth_m)?;
        if !z.contains(r)
            || e.is_some_and(|e| e.overlaps(r))
            || r.area() != u64::from(c.reserved_area_m2)
        {
            return Err(format!("invalid reserve {}", c.id));
        }
        *reserve_areas.entry(&c.zone_id).or_default() += r.area();
        occupied.push((c.id.as_str(), r));
    }
    for p in &plan.parcels {
        unique(&p.id)?;
        let (e, _) = envelopes
            .get(p.zone_id.as_str())
            .ok_or("parcel has no envelope")?;
        let r = PlanRect::new(p.world_x, p.world_z, p.width_m, p.depth_m)?;
        if !e.is_some_and(|e| e.contains(r)) {
            return Err(format!("parcel {} escapes envelope", p.id));
        }
        occupied.push((p.id.as_str(), r));
    }
    for p in &plan.public_spaces {
        unique(&p.id)?;
        let (e, _) = envelopes
            .get(p.zone_id.as_str())
            .ok_or("public space has no envelope")?;
        let r = PlanRect::new(p.world_x, p.world_z, p.width_m, p.depth_m)?;
        if !e.is_some_and(|e| e.contains(r)) {
            return Err(format!("public space {} escapes envelope", p.id));
        }
        *public_areas.entry(&p.zone_id).or_default() += r.area();
        occupied.push((p.id.as_str(), r));
    }
    non_overlapping(occupied)?;
    for (zone, (rect, e)) in &envelopes {
        if rect.map_or(0, |r| r.area()) + reserve_areas.get(zone).copied().unwrap_or(0)
            != zones[zone].area()
        {
            return Err("zone reserve area mismatch".into());
        }
        if public_areas.get(zone).copied().unwrap_or(0) != u64::from(e.public_space_reserved_m2) {
            return Err("public space budget mismatch".into());
        }
    }
    Ok(())
}

pub fn realize_infrastructure_plan(plan: &mut InfrastructurePlan, entities: &[EntityInstance]) {
    for corridor in &mut plan.corridors {
        let min_x = corridor.world_x as f32;
        let min_z = corridor.world_z as f32;
        let max_x = min_x + corridor.width_m as f32;
        let max_z = min_z + corridor.depth_m as f32;
        corridor.accepted_entity_ids = entities
            .iter()
            .filter(|entity| {
                let compatible = if corridor.kind == "drainage-reserve" {
                    entity.kind == "canal" || entity.kind == "water"
                } else {
                    entity.kind == "road" || entity.kind == "sidewalk"
                };
                if !compatible {
                    return false;
                }
                let bounds = entity.computed_bounds();
                bounds.min_x >= min_x
                    && bounds.max_x <= max_x
                    && bounds.min_z >= min_z
                    && bounds.max_z <= max_z
            })
            .map(|entity| entity.entity_id.clone())
            .collect();
        if corridor.accepted_entity_ids.is_empty() {
            corridor.realization_status = "reserved-not-realized".into();
            corridor.rejection_reason = Some("no-compatible-infrastructure-entity".into());
        } else {
            corridor.realization_status = "realized".into();
            corridor.rejection_reason = None;
        }
    }
    plan.status = "reserved-and-realized-v1".into();
}

pub fn planned_entity_id(plan_id: &str) -> String {
    format!("generated.plan.{plan_id}")
}

pub fn realize_parcel_plan(plan: &mut ParcelPlan, entities: &[EntityInstance]) {
    use std::collections::BTreeMap;
    let mut by_id = BTreeMap::<&str, Vec<&EntityInstance>>::new();
    for e in entities {
        by_id.entry(&e.entity_id).or_default().push(e);
    }
    for p in &mut plan.parcels {
        p.accepted_entity_ids.clear();
        let expected = planned_entity_id(&p.id);
        let reason = match by_id.get(expected.as_str()) {
            None => Some("missing_final_entity"),
            Some(matches) if matches.len() != 1 => Some("ambiguous_entity_id"),
            Some(matches) => {
                let e = matches[0];
                let compatible = match p.land_use.as_str() {
                    "urban" => matches!(e.kind.as_str(), "residential_block" | "residential_home"),
                    "rural" => matches!(e.kind.as_str(), "farmland" | "pasture"),
                    "nature" => matches!(e.kind.as_str(), "green_space" | "nature_reserve"),
                    _ => false,
                };
                if !compatible {
                    Some("incompatible_entity_kind")
                } else if !e.scale.is_finite()
                    || e.scale <= 0.
                    || !e.width_m.is_finite()
                    || e.width_m <= 0.
                    || !e.depth_m.is_finite()
                    || e.depth_m <= 0.
                    || !e.rotation_y_deg.is_finite()
                {
                    Some("invalid_entity_geometry")
                } else {
                    // Support arbitrary authored rotations, not only quarter turns.
                    let angle = f64::from(e.rotation_y_deg).rem_euclid(360.);
                    let (sin, cos) = match angle {
                        0. | 180. => (0., 1.),
                        90. | 270. => (1., 0.),
                        _ => {
                            let (s, c) = angle.to_radians().sin_cos();
                            (s.abs(), c.abs())
                        }
                    };
                    let w = f64::from(e.width_m) * f64::from(e.scale) * 0.5;
                    let d = f64::from(e.depth_m) * f64::from(e.scale) * 0.5;
                    let (hw, hd) = (w * cos + d * sin, w * sin + d * cos);
                    let (x, z) = (f64::from(e.world_x), f64::from(e.world_z));
                    if x - hw < f64::from(p.world_x)
                        || z - hd < f64::from(p.world_z)
                        || x + hw > f64::from(p.world_x) + f64::from(p.width_m)
                        || z + hd > f64::from(p.world_z) + f64::from(p.depth_m)
                    {
                        Some("entity_outside_parcel")
                    } else {
                        None
                    }
                }
            }
        };
        p.rejection_reason = reason.map(str::to_owned);
        p.realization_status = if reason.is_none() {
            "accepted"
        } else {
            "unrealized"
        }
        .into();
        if reason.is_none() {
            p.accepted_entity_ids.push(expected);
        }
    }
    plan.status = if plan.parcels.is_empty() {
        "no_developable_parcels"
    } else if plan
        .parcels
        .iter()
        .all(|p| p.realization_status == "accepted")
    {
        "all_parcels_realized"
    } else {
        "partially_realized"
    }
    .into();
}

/// Rebuild planning and realization from inputs, never trust persisted status.
pub fn verify_parcel_plan(
    plan: &ParcelPlan,
    zones: &[crate::worldgen::LandUseZone],
    entities: &[EntityInstance],
) -> Result<(), String> {
    validate_parcel_plan(plan)?;
    let mut expected = plan_parcels(zones);
    realize_parcel_plan(&mut expected, entities);
    realize_infrastructure_plan(&mut expected.infrastructure, entities);
    if serde_json::to_value(plan).map_err(|e| e.to_string())?
        != serde_json::to_value(expected).map_err(|e| e.to_string())?
    {
        return Err(
            "persisted parcel plan differs from independently rebuilt geometry or realization"
                .into(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettlementGraph {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub settlements: Vec<SettlementNode>,
    pub districts: Vec<DistrictNode>,
    pub parcels: Vec<ParcelNode>,
    pub public_spaces: Vec<PublicSpaceNode>,
    pub building_relations: Vec<BuildingRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementNode {
    pub id: String,
    pub kind: String,
    pub center_x: f32,
    pub center_z: f32,
    pub district_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistrictNode {
    pub id: String,
    pub settlement_id: String,
    pub kind: String,
    pub center_x: f32,
    pub center_z: f32,
    pub parcel_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelNode {
    pub id: String,
    pub district_id: String,
    pub land_use: String,
    pub center_x: f32,
    pub center_z: f32,
    pub width_m: f32,
    pub depth_m: f32,
    pub entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSpaceNode {
    pub id: String,
    pub district_id: String,
    pub kind: String,
    pub entity_id: String,
    pub area_m2: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingRelation {
    pub entity_id: String,
    pub settlement_id: String,
    pub district_id: String,
    pub parcel_id: String,
    pub nearest_road_id: Option<String>,
    pub distance_to_road_m: Option<f32>,
    pub relation_reason: String,
}

pub(crate) fn is_building(kind: &str) -> bool {
    matches!(
        kind,
        "building"
            | "building_tower"
            | "building_cluster"
            | "urban_building"
            | "residential_block"
            | "residential_tower"
            | "residential_home"
            | "commercial_center"
            | "entertainment_center"
            | "school"
            | "town_hall"
            | "market"
            | "industrial"
            | "temple"
            | "church"
            | "storefront"
            | "shopping_mall"
            | "office_tower"
            | "convenience_store"
            | "night_market"
    )
}

fn district_kind(kind: &str) -> &'static str {
    match kind {
        "industrial" => "industrial",
        "commercial_center"
        | "entertainment_center"
        | "market"
        | "storefront"
        | "shopping_mall"
        | "office_tower"
        | "convenience_store"
        | "night_market" => "mixed_commercial",
        "school" | "town_hall" | "temple" | "church" => "civic_institutional",
        _ => "residential_mixed",
    }
}

pub fn derive_settlement_graph(scene: &SceneSpec, entities: &[EntityInstance]) -> SettlementGraph {
    let settlement_id = format!("{}.settlement.0", scene.scene_id);
    let center_x = scene.origin_x as f32 + scene.width_m as f32 * 0.5;
    let center_z = scene.origin_z as f32 + scene.depth_m as f32 * 0.5;
    let mut districts = Vec::<DistrictNode>::new();
    let mut parcels = Vec::<ParcelNode>::new();
    let mut relations = Vec::<BuildingRelation>::new();
    let mut public_spaces = Vec::<PublicSpaceNode>::new();
    let roads: Vec<&EntityInstance> = entities
        .iter()
        .filter(|entity| entity.kind == "road")
        .collect();

    for (index, entity) in entities
        .iter()
        .filter(|entity| is_building(&entity.kind))
        .enumerate()
    {
        let kind = district_kind(&entity.kind);
        let district_id = format!("{}.district.{}", scene.scene_id, kind);
        if !districts.iter().any(|district| district.id == district_id) {
            districts.push(DistrictNode {
                id: district_id.clone(),
                settlement_id: settlement_id.clone(),
                kind: kind.into(),
                center_x: entity.world_x as f32,
                center_z: entity.world_z as f32,
                parcel_ids: Vec::new(),
            });
        }
        let parcel_id = format!("{}.parcel.{index}", scene.scene_id);
        let land_use = if kind == "industrial" {
            "industrial"
        } else if kind == "mixed_commercial" {
            "commercial"
        } else if kind == "civic_institutional" {
            "civic"
        } else {
            "residential"
        };
        parcels.push(ParcelNode {
            id: parcel_id.clone(),
            district_id: district_id.clone(),
            land_use: land_use.into(),
            center_x: entity.world_x as f32,
            center_z: entity.world_z as f32,
            width_m: entity.width_m,
            depth_m: entity.depth_m,
            entity_ids: vec![entity.entity_id.clone()],
        });
        if let Some(district) = districts
            .iter_mut()
            .find(|district| district.id == district_id)
        {
            district.parcel_ids.push(parcel_id.clone());
        }
        let nearest = roads.iter().min_by(|a, b| {
            let da =
                ((a.world_x - entity.world_x).pow(2) + (a.world_z - entity.world_z).pow(2)) as f32;
            let db =
                ((b.world_x - entity.world_x).pow(2) + (b.world_z - entity.world_z).pow(2)) as f32;
            da.total_cmp(&db)
        });
        let (nearest_road_id, distance) = nearest
            .map(|road| {
                (
                    Some(road.entity_id.clone()),
                    Some(
                        (((road.world_x - entity.world_x).pow(2)
                            + (road.world_z - entity.world_z).pow(2))
                            as f32)
                            .sqrt(),
                    ),
                )
            })
            .unwrap_or((None, None));
        relations.push(BuildingRelation {
            entity_id: entity.entity_id.clone(),
            settlement_id: settlement_id.clone(),
            district_id,
            parcel_id,
            nearest_road_id,
            distance_to_road_m: distance,
            relation_reason: "derived-from-landuse-kind-and-nearest-road".into(),
        });
    }
    for entity in entities.iter().filter(|entity| {
        matches!(
            entity.kind.as_str(),
            "green_space" | "nature_reserve" | "community_plaza" | "park"
        )
    }) {
        let district_id = format!("{}.district.public", scene.scene_id);
        public_spaces.push(PublicSpaceNode {
            id: format!("{}.public-space.{}", scene.scene_id, public_spaces.len()),
            district_id,
            kind: entity.kind.clone(),
            entity_id: entity.entity_id.clone(),
            area_m2: entity.width_m * entity.depth_m,
        });
    }
    let district_ids = districts
        .iter()
        .map(|district| district.id.clone())
        .collect();
    SettlementGraph {
        format: "glyphweave.settlement-graph".into(),
        version: 1,
        status: "derived-relationship-contract-v1".into(),
        settlements: vec![SettlementNode {
            id: settlement_id,
            kind: "generated-mixed-settlement".into(),
            center_x,
            center_z,
            district_ids,
        }],
        districts,
        parcels,
        public_spaces,
        building_relations: relations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::SceneSpec;

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "scene".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn entity(kind: &str, id: &str, x: i32, z: i32) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: format!("prop.{kind}"),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            scale: 1.0,
            width_m: 20.0,
            depth_m: 20.0,
            height_m: 10.0,
            rotation_y_deg: 0.0,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        }
    }

    #[test]
    fn parcel_plan_preserves_zone_parentage_and_contract_flags() {
        let zones = vec![crate::worldgen::LandUseZone {
            id: "scene.zone.urban.0".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m: 160,
            source: "test".into(),
        }];
        let plan = plan_parcels(&zones);
        assert_eq!(plan.status, "planned-before-entity-generation-v2");
        assert_eq!(plan.parcels.len(), 1);
        assert!(
            plan.parcels
                .iter()
                .all(|p| p.zone_id == zones[0].id && p.frontage_reserved)
        );
        assert_eq!(plan.public_spaces.len(), 0);
        let mut realized = plan.clone();
        let entity = crate::worldgen::EntityInstance {
            entity_id: planned_entity_id(&plan.parcels[0].id),
            asset_id: "prop.residential_block".into(),
            kind: "residential_block".into(),
            world_x: 80,
            world_z: 80,
            world_y: 0,
            scale: 1.0,
            width_m: 40.0,
            depth_m: 40.0,
            height_m: 8.0,
            rotation_y_deg: 0.0,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        realize_parcel_plan(&mut realized, &[entity]);
        assert_eq!(realized.status, "all_parcels_realized");
        assert!(
            realized
                .parcels
                .iter()
                .any(|p| p.realization_status == "accepted")
        );
    }

    #[test]
    fn infrastructure_realization_records_compatible_entities() {
        let zones = vec![crate::worldgen::LandUseZone {
            id: "scene.zone.urban.0".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let mut plan = plan_parcels(&zones);
        let entity = EntityInstance {
            entity_id: "generated.road-ew.1".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 256,
            world_z: 6,
            world_y: 0,
            scale: 1.0,
            width_m: 512.0,
            depth_m: 10.0,
            height_m: 0.2,
            rotation_y_deg: 0.0,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        };
        realize_infrastructure_plan(&mut plan.infrastructure, &[entity]);
        assert!(
            plan.infrastructure
                .corridors
                .iter()
                .any(|c| c.realization_status == "realized")
        );
    }

    #[test]
    fn parcel_validation_rejects_overlapping_same_zone_parcels() {
        let mut plan = plan_parcels(&[crate::worldgen::LandUseZone {
            id: "scene.zone.urban.0".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 160,
            depth_m: 160,
            source: "test".into(),
        }]);
        plan.parcels.push(plan.parcels[0].clone());
        assert!(validate_parcel_plan(&plan).is_err());
    }

    #[test]
    fn infrastructure_plan_records_reserved_corridors() {
        let zones = vec![crate::worldgen::LandUseZone {
            id: "scene.zone.urban.0".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let plan = plan_parcels(&zones);
        assert_eq!(
            plan.infrastructure.status,
            "reserved-before-infrastructure-entities-v2"
        );
        assert!(
            plan.infrastructure
                .corridors
                .iter()
                .any(|c| c.kind == "road-frontage-reserve")
        );
    }

    #[test]
    fn development_envelope_reserves_are_inside_parent_zone() {
        let zones = vec![crate::worldgen::LandUseZone {
            id: "scene.zone.rural.0".into(),
            kind: "rural".into(),
            world_x: 100,
            world_z: 200,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let plan = plan_parcels(&zones);
        let envelope = &plan.development_envelopes[0];
        assert!(envelope.world_x >= zones[0].world_x);
        assert!(envelope.world_z >= zones[0].world_z);
        assert!(
            envelope.world_x + envelope.width_m as i32
                <= zones[0].world_x + zones[0].width_m as i32
        );
        assert!(
            envelope.world_z + envelope.depth_m as i32
                <= zones[0].world_z + zones[0].depth_m as i32
        );
        assert_eq!(envelope.drainage_reserved_m, 8);
    }

    #[test]
    fn parcel_plan_keeps_parcels_inside_development_envelopes() {
        let zones = vec![crate::worldgen::LandUseZone {
            id: "scene.zone.urban.0".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let plan = plan_parcels(&zones);
        assert_eq!(plan.parcels.len(), 4);
        assert_eq!(plan.development_envelopes.len(), 1);
        let envelope = &plan.development_envelopes[0];
        for parcel in &plan.parcels {
            assert!(parcel.world_x >= envelope.world_x);
            assert!(parcel.world_z >= envelope.world_z);
            assert!(
                parcel.world_x + parcel.width_m as i32
                    <= envelope.world_x + envelope.width_m as i32
            );
            assert!(
                parcel.world_z + parcel.depth_m as i32
                    <= envelope.world_z + envelope.depth_m as i32
            );
        }
    }

    #[test]
    fn derives_parcel_and_nearest_road_relationships() {
        let graph = derive_settlement_graph(
            &scene(),
            &[
                entity("road", "road.0", 100, 100),
                entity("residential_home", "home.0", 120, 100),
                entity("green_space", "park.0", 140, 120),
            ],
        );
        assert_eq!(graph.settlements.len(), 1);
        assert_eq!(graph.parcels.len(), 1);
        assert_eq!(graph.building_relations.len(), 1);
        assert_eq!(
            graph.building_relations[0].nearest_road_id.as_deref(),
            Some("road.0")
        );
        assert_eq!(graph.public_spaces.len(), 1);
    }
    #[test]
    fn macro_plan_itself_must_pass_geometry_gate() {
        let zones = [crate::worldgen::LandUseZone {
            id: "macro".into(),
            kind: "urban".into(),
            world_x: -512,
            world_z: -256,
            width_m: 240,
            depth_m: 240,
            source: "test".into(),
        }];
        let plan = plan_parcels(&zones);
        assert_eq!(plan.parcels.len(), 4);
        validate_parcel_plan(&plan).unwrap();
    }
    fn test_zone(w: u32, d: u32) -> crate::worldgen::LandUseZone {
        crate::worldgen::LandUseZone {
            id: "z".into(),
            kind: "urban".into(),
            world_x: -700,
            world_z: -400,
            width_m: w,
            depth_m: d,
            source: "test".into(),
        }
    }

    #[test]
    fn partitions_validate_for_tiny_odd_partial_and_macro_zones() {
        for w in [1, 7, 12, 39, 64, 159, 160, 239, 240, 241, 511, 512] {
            for d in [1, 7, 12, 39, 64, 159, 160, 239, 240, 241, 511, 512] {
                for kind in ["urban", "rural", "nature"] {
                    let mut zone = test_zone(w, d);
                    zone.kind = kind.into();
                    let plan = plan_parcels(&[zone]);
                    validate_parcel_plan(&plan).unwrap_or_else(|e| panic!("{w}x{d} {kind}: {e}"));
                }
            }
        }
    }

    #[test]
    fn invalid_parent_public_overlap_and_reserve_geometry_fail_closed() {
        let plan = plan_parcels(&[test_zone(240, 240)]);
        validate_parcel_plan(&plan).unwrap();
        let mut bad = plan.clone();
        bad.parcels[0].zone_id = "missing".into();
        assert!(validate_parcel_plan(&bad).is_err());
        let mut bad = plan.clone();
        bad.infrastructure.corridors[0].world_z += 15;
        assert!(validate_parcel_plan(&bad).is_err());
        let mut bad = plan.clone();
        bad.public_spaces[0].world_x = bad.parcels[0].world_x;
        bad.public_spaces[0].world_z = bad.parcels[0].world_z;
        bad.public_spaces[0].width_m = 20;
        assert!(validate_parcel_plan(&bad).is_err());
        let mut bad = plan.clone();
        bad.zones.clear();
        assert!(validate_parcel_plan(&bad).is_err());
        let mut bad = plan.clone();
        bad.parcels[0].width_m = u32::MAX;
        assert!(validate_parcel_plan(&bad).is_err());
    }

    #[test]
    fn realization_requires_exact_origin_id_and_valid_final_geometry() {
        let mut plan = plan_parcels(&[test_zone(160, 160)]);
        let p = plan.parcels[0].clone();
        let mut e = entity(
            "residential_block",
            "unrelated",
            p.world_x + 30,
            p.world_z + 30,
        );
        realize_parcel_plan(&mut plan, &[e.clone()]);
        assert!(plan.parcels[0].accepted_entity_ids.is_empty());
        e.entity_id = planned_entity_id(&p.id);
        realize_parcel_plan(&mut plan, &[e.clone()]);
        assert_eq!(
            plan.parcels[0].accepted_entity_ids,
            vec![e.entity_id.clone()]
        );
        realize_parcel_plan(&mut plan, &[e.clone(), e.clone()]);
        assert_eq!(
            plan.parcels[0].rejection_reason.as_deref(),
            Some("ambiguous_entity_id")
        );
        e.scale = 20.;
        realize_parcel_plan(&mut plan, &[e]);
        assert_eq!(
            plan.parcels[0].rejection_reason.as_deref(),
            Some("entity_outside_parcel")
        );
        let mut empty = plan_parcels(&[test_zone(7, 7)]);
        realize_parcel_plan(&mut empty, &[]);
        assert_eq!(empty.status, "no_developable_parcels");
    }

    #[test]
    fn maximum_scene_has_nonempty_disjoint_macro_plan() {
        let scene = SceneSpec {
            width_m: 6000,
            depth_m: 10000,
            ..scene()
        };
        let zones = crate::worldgen::plan_landuse_zones(
            &scene,
            &crate::worldgen::LandUseProfile::default_demo(),
        );
        let plan = plan_parcels(&zones);
        assert!(plan.parcels.len() > 3000);
        validate_parcel_plan(&plan).unwrap();
    }
    #[test]
    fn diagonal_rotation_cannot_hide_an_outside_corner() {
        let mut plan = plan_parcels(&[test_zone(160, 160)]);
        let p = plan.parcels[0].clone();
        let mut e = entity(
            "residential_block",
            &planned_entity_id(&p.id),
            p.world_x + 11,
            p.world_z + 11,
        );
        realize_parcel_plan(&mut plan, &[e.clone()]);
        assert_eq!(plan.parcels[0].realization_status, "accepted");
        e.rotation_y_deg = 45.;
        realize_parcel_plan(&mut plan, &[e]);
        assert_eq!(
            plan.parcels[0].rejection_reason.as_deref(),
            Some("entity_outside_parcel")
        );
    }
}
