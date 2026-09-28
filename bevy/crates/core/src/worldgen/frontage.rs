//! Deterministic demand-led parcels on the FINAL planned road network.
//! Local road station heights are engineering inputs, not city-area credit.
use super::{
    AccessGraph, EntityInstance, LandUseZone, SceneSpec, WaterGeometry, WorldManifest,
    road_network::{RoadNetworkPlan, RoadsideSite},
    services::SiteProgram,
    zoning::ZoningMask,
    parcel_geometry::{self, ParcelInsets},
};
use crate::{
    baked_terrain::{BakedTerrain, TerrainError},
    height_payload::{HeightEncoding, encode_millimetres},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub requests: Vec<Request>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub scene_id: String,
    pub purpose: String,
    pub kind: String,
    pub count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
    pub station_spacing_m: u32,
    pub setback_m: u32,
    pub max_candidate_attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parcel_insets: Option<ParcelInsets>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outcome {
    pub request: Request,
    pub site_ids: Vec<String>,
    pub attempted: u32,
    pub unfilled: u32,
    pub rejected: BTreeMap<String, u32>,
    pub stop_reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub version: u32,
    pub model: String,
    pub outcomes: Vec<Outcome>,
}
fn err(s: impl Into<String>) -> TerrainError {
    TerrainError(s.into())
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<Program>, TerrainError> {
    let Some(v) = style.get("frontageProgram") else {
        return Ok(None);
    };
    let p: Program = serde_json::from_value(v.clone()).map_err(|e| err(e.to_string()))?;
    if p.version != 1 || p.requests.is_empty() || p.requests.len() > 64 {
        return Err(err("invalid frontage program"));
    }
    let mut ids = BTreeSet::new();
    let mut count = 0_u64;
    for r in &p.requests {
        if r.id.is_empty()
            || r.id.len() > 64
            || !r
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !ids.insert((&r.scene_id, &r.id))
            || r.scene_id.is_empty()
            || !matches!(r.purpose.as_str(), "urban" | "rural")
            || !(1..=1024).contains(&r.count)
            || !(1..=65536).contains(&r.max_candidate_attempts)
            || !(8..=128).contains(&r.station_spacing_m)
            || r.station_spacing_m % 2 != 0
            || !(8..=32).contains(&r.setback_m)
            || r.setback_m % 2 != 0
        {
            return Err(err("invalid frontage id/purpose/count/search limits"));
        }
        // Bounded search geometry; the manifest asset contract is checked separately.
        let bounds = match r.kind.as_str() {
            "residential_home" => (12..=48, 10..=36, 3..=14),
            "residential_block" => (24..=100, 16..=60, 12..=60),
            "storefront" => (8..=36, 8..=24, 3..=12),
            _ => return Err(err("unsupported frontage building kind")),
        };
        if !bounds.0.contains(&r.width_m)
            || !bounds.1.contains(&r.depth_m)
            || !bounds.2.contains(&r.height_m)
            || r.width_m % 2 != 0
            || r.depth_m % 2 != 0
        {
            return Err(err("invalid exact frontage dimensions; no shrinking"));
        }
        r.parcel_insets
            .unwrap_or_default()
            .validate_for_request(r.width_m, r.depth_m, r.setback_m)
            .map_err(err)?;
        count += u64::from(r.count);
    }
    if count > 2048 {
        return Err(err("frontage program exceeds 2048 buildings"));
    }
    Ok(Some(p))
}
pub fn validate_manifest(m: &WorldManifest) -> Result<(), TerrainError> {
    if let Some(p) = from_style(&m.style)? {
        if super::height_encoding_from_style(&m.style).map_err(|e| err(e.to_string()))?
            != HeightEncoding::Millimetre
        {
            return Err(err("frontage requires millimetre heights"));
        }
        for r in p.requests {
            let contract = m
                .style
                .get("assetContracts")
                .and_then(|v| v.get(&r.kind))
                .ok_or_else(|| err("frontage requires an explicit asset contract"))?;
            for (axis, size) in [
                ("Width", r.width_m),
                ("Depth", r.depth_m),
                ("Height", r.height_m),
            ] {
                let min = contract
                    .get(format!("min{axis}M"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| err("frontage asset minimum missing"))?;
                let max = contract
                    .get(format!("max{axis}M"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| err("frontage asset maximum missing"))?;
                if !min.is_finite()
                    || !max.is_finite()
                    || min > max
                    || f64::from(size) < min
                    || f64::from(size) > max
                {
                    return Err(err(format!(
                        "frontage {} {axis} violates asset contract; no resizing",
                        r.id
                    )));
                }
            }
            let scene = m
                .scenes
                .iter()
                .find(|s| s.scene_id == r.scene_id)
                .ok_or_else(|| err("unknown frontage scene"))?;
            let water =
                super::water_geometry(super::water_kind(&m.style), &m.landmarks, scene, &m.style);
            if !super::road_network::enabled(&m.style, water) {
                return Err(err("frontage requires road-first planning"));
            }
        }
    }
    Ok(())
}
/// The engineered entrance is a bounded cardinal connection, never an arbitrary earthwork.
pub fn validate_entity(e: &EntityInstance) -> Result<(), TerrainError> {
    let Some([x, z]) = e.grounding.frontage_road_point else {
        return Ok(());
    };
    if !matches!(
        e.kind.as_str(),
        "residential_home" | "residential_block" | "storefront"
    ) || e.anchors.len() != 1
        || e.grounding.road_profile.is_some()
    {
        return Err(err("invalid frontage support owner"));
    }
    let a = &e.anchors[0];
    let dx = i64::from(x) - i64::from(a.world_x);
    let dz = i64::from(z) - i64::from(a.world_z);
    let length = dx.abs() + dz.abs();
    let direction = match a.direction.as_str() {
        "north" => dx == 0 && dz < 0,
        "south" => dx == 0 && dz > 0,
        "east" => dz == 0 && dx > 0,
        "west" => dz == 0 && dx < 0,
        _ => false,
    };
    if !direction
        || !(1..=40).contains(&length)
        || !e.ground_height_m().is_finite()
        || !(-32.0..=256.0).contains(&e.ground_height_m())
    {
        return Err(err("invalid frontage support alignment/length/height"));
    }
    Ok(())
}
fn bounds(e: &EntityInstance, margin: i64) -> [i64; 4] {
    let (w, d) = super::spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
    [
        i64::from(e.world_x) - w as i64 - margin,
        i64::from(e.world_z) - d as i64 - margin,
        i64::from(e.world_x) + w as i64 + margin,
        i64::from(e.world_z) + d as i64 + margin,
    ]
}
fn overlap(a: [i64; 4], b: [i64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
pub(super) fn parcel_boundary(site: &RoadsideSite) -> Result<[i64; 4], TerrainError> {
    parcel_geometry::boundary_for_entity(
        &site.entity(),
        site.parcel_insets.unwrap_or_default(),
    )
    .map_err(err)
}
/// Production road iteration order for the local station search. Wider
/// hierarchy roads are kept as topology/collectors; local roads get first
/// chance when their width is narrower. All-8m legacy inputs retain the
/// original plan iteration order.
pub(super) fn profile_roads(plan: &RoadNetworkPlan) -> Vec<EntityInstance> {
    let mut roads = plan.entities();
    if roads.iter().any(|road| road.depth_m > 8.) {
        roads.sort_by_key(|road| (road.depth_m as u32, road.entity_id.clone()));
    }
    roads
}
/// The exact station list `append` visits on one road for one request, or
/// `None` when the road carries no local profile contract. Shared with the
/// bounded supply probe so a measurement can never drift from placement.
pub(super) fn stations(
    road: &EntityInstance,
    r: &Request,
) -> Result<Option<Vec<usize>>, TerrainError> {
    // At present only profile-bearing roads support this local-height path.
    let Some(profile) = &road.grounding.road_profile else {
        return Ok(None);
    };
    profile.validate_entity(road)?;
    let length = profile.elevations_mm.len() - 1;
    let margin = r.width_m / 2 + 8;
    if length <= 2 * margin as usize {
        return Ok(None);
    }
    // Wider roads need their extra half-width as junction clearance so the
    // first parcel does not overlap the connecting collector.
    let station_start = margin as usize + (road.depth_m as usize).saturating_sub(8);
    if station_start > length - margin as usize {
        return Ok(None);
    }
    Ok(Some(
        (station_start..=length - margin as usize)
            .step_by(r.station_spacing_m as usize)
            .collect(),
    ))
}
/// Build the exact `RoadsideSite` production would try at this station/side.
pub(super) fn candidate_site(
    road: &EntityInstance,
    r: &Request,
    scene_id: &str,
    station: usize,
    side: i32,
) -> Result<RoadsideSite, TerrainError> {
    let profile = road
        .grounding
        .road_profile
        .as_ref()
        .ok_or_else(|| err("frontage candidate requires a road profile"))?;
    let dx = (profile.end[0] - profile.start[0]).signum();
    let dz = (profile.end[1] - profile.start[1]).signum();
    let rx = profile.start[0] + dx * station as i32;
    let rz = profile.start[1] + dz * station as i32;
    let offset = (road.depth_m / 2.) as i32 + r.setback_m as i32 + (r.depth_m / 2) as i32;
    let (x, z, yaw) = if dx != 0 {
        (rx, rz + side * offset, if side > 0 { 0. } else { 180. })
    } else {
        (rx + side * offset, rz, if side > 0 { 90. } else { 270. })
    };
    let level = i32::try_from(profile.elevations_mm[station])
        .map_err(|_| err("frontage height overflow"))?;
    let id = format!(
        "generated.frontage.{}.{}.{}.{}.{}",
        scene_id,
        r.id,
        road.entity_id,
        station,
        if side > 0 { "positive" } else { "negative" }
    );
    Ok(RoadsideSite {
        program: Some(SiteProgram {
            request_id: r.id.clone(),
            kind: r.kind.clone(),
            height_m: r.height_m,
        }),
        id,
        road_id: road.entity_id.clone(),
        world_x: x,
        world_z: z,
        width_m: r.width_m,
        depth_m: r.depth_m,
        support_level_m: level.div_euclid(1000),
        parcel_insets: r.parcel_insets,
        support_level_mm: Some(level),
        frontage_road_point: Some([rx, rz]),
        rotation_y_deg: yaw,
        natural_min_m: f32::INFINITY,
        natural_max_m: f32::NEG_INFINITY,
    })
}
/// Scene envelope and purpose gate, applied before any engineering preflight.
pub(super) fn envelope_reason(
    parcel: [i64; 4],
    scene: &SceneSpec,
    zones: &[LandUseZone],
    purpose: &str,
) -> Option<&'static str> {
    if parcel[0] < i64::from(scene.origin_x) + 8
        || parcel[1] < i64::from(scene.origin_z) + 8
        || parcel[2] + 8 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
        || parcel[3] + 8 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
    {
        Some("outside_scene")
    } else if !in_purpose(parcel, zones, purpose) {
        Some("wrong_purpose_or_protected")
    } else {
        None
    }
}
fn in_purpose(b: [i64; 4], zones: &[LandUseZone], purpose: &str) -> bool {
    let covered: i64 = zones
        .iter()
        .filter(|z| z.kind == purpose)
        .map(|z| {
            (b[2].min(i64::from(z.world_x) + i64::from(z.width_m)) - b[0].max(i64::from(z.world_x)))
                .max(0)
                * (b[3].min(i64::from(z.world_z) + i64::from(z.depth_m))
                    - b[1].max(i64::from(z.world_z)))
                .max(0)
        })
        .sum();
    covered == (b[2] - b[0]) * (b[3] - b[1])
}
fn raw_mm(n: &BakedTerrain, x: i32, z: i32) -> Result<i32, TerrainError> {
    n.height_millimetres(x, z)
        .ok_or_else(|| err("missing frontage source sample"))
}
fn sample(
    n: &BakedTerrain,
    mask: &ZoningMask,
    carves: &[super::TerrainCarve],
    x: i32,
    z: i32,
) -> Result<i32, TerrainError> {
    let base = f64::from(raw_mm(n, x, z)?) / 1000.;
    let h = if mask.protected_at(x, z) {
        base
    } else {
        super::apply_terrain_carves(base, x, z, carves)
            .max(base)
            .min(base + mask.fill_capacity_at(x, z))
    };
    Ok((h * 1000.).round() as i32)
}
/// Structured evidence for one real rejection. The decision itself stays the
/// `&'static str` reason key; this only records what was actually found.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RejectDetail {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicting_entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicting_site: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
impl RejectDetail {
    fn of(reason: &str) -> Self {
        Self {
            reason: reason.into(),
            conflicting_entity: None,
            conflicting_site: None,
            at: None,
            natural_mm: None,
            design_mm: None,
            before_mm: None,
            after_mm: None,
            note: None,
        }
    }
    /// Evidence for the pre-engineering envelope/purpose gate.
    pub(super) fn envelope(reason: &str, parcel: [i64; 4]) -> Self {
        Self {
            at: Some([
                ((parcel[0] + parcel[2]) / 2) as i32,
                ((parcel[1] + parcel[3]) / 2) as i32,
            ]),
            note: Some(format!(
                "parcel=[{},{},{},{}]",
                parcel[0], parcel[1], parcel[2], parcel[3]
            )),
            ..Self::of(reason)
        }
    }
}
fn support_check(
    site: &RoadsideSite,
    n: &BakedTerrain,
    mask: &ZoningMask,
    max_fill_mm: i32,
    detail: &mut Option<RejectDetail>,
) -> Result<Option<&'static str>, TerrainError> {
    let e = site.entity();
    let b = bounds(&e, 2);
    let level = site.support_level_mm.expect("new sites are precise");
    for z in b[1]..=b[3] {
        for x in b[0]..=b[2] {
            let h = raw_mm(n, x as i32, z as i32)?;
            let note = if h > level {
                Some("natural_above_design_level")
            } else if i64::from(level) - i64::from(h) > i64::from(max_fill_mm) {
                Some("fill_exceeds_roadbed_limit")
            } else if f64::from(level - h) > mask.fill_capacity_at(x as i32, z as i32) * 1000. {
                Some("fill_exceeds_zoning_capacity")
            } else {
                None
            };
            if let Some(note) = note {
                *detail = Some(RejectDetail {
                    at: Some([x as i32, z as i32]),
                    natural_mm: Some(h),
                    design_mm: Some(level),
                    note: Some(note.into()),
                    ..RejectDetail::of("natural_support_or_fill_limit")
                });
                return Ok(Some("natural_support_or_fill_limit"));
            }
        }
    }
    Ok(None)
}
fn connection_is_reserved(mask: &ZoningMask, e: &EntityInstance, x: i32, z: i32) -> bool {
    let a = &e.anchors[0];
    for zz in a.world_z.min(z) - 2..=a.world_z.max(z) + 2 {
        for xx in a.world_x.min(x) - 2..=a.world_x.max(x) + 2 {
            if mask
                .check("frontage-access".into(), "access", xx, zz, 0, 0)
                .is_some()
            {
                return true;
            }
        }
    }
    false
}
fn preflight(
    site: &RoadsideSite,
    plan: &RoadNetworkPlan,
    n: &BakedTerrain,
    mask: &ZoningMask,
    water: WaterGeometry,
    existing: &[EntityInstance],
    base_carves: &[super::TerrainCarve],
    detail: &mut Option<RejectDetail>,
) -> Result<Option<&'static str>, TerrainError> {
    let e = site.entity();
    validate_entity(&e)?;
    let b = bounds(&e, 2);
    let (hw, hd) = site.world_half_extents();
    let parcel = parcel_boundary(site)?;
    let parcel_half_w = (parcel[2] - parcel[0]) as f64 * 0.5;
    let parcel_half_d = (parcel[3] - parcel[1]) as f64 * 0.5;
    let parcel_center_x = (parcel[0] + parcel[2]) as f64 * 0.5;
    let parcel_center_z = (parcel[1] + parcel[3]) as f64 * 0.5;
    if water.intersects_axis_aligned(
        parcel_center_x,
        parcel_center_z,
        parcel_half_w,
        parcel_half_d,
    ) {
        *detail = Some(RejectDetail {
            at: Some([parcel_center_x as i32, parcel_center_z as i32]),
            ..RejectDetail::of("water")
        });
        return Ok(Some("water"));
    }
    for old in existing {
        let old_bounds = plan
            .roadside_sites
            .iter()
            .find(|s| s.id == old.entity_id)
            .map(parcel_boundary)
            .transpose()?
            .unwrap_or_else(|| bounds(old, if old.kind == "road" { 4 } else { 2 }));
        if overlap(parcel, old_bounds) {
            *detail = Some(RejectDetail {
                conflicting_entity: Some(old.entity_id.clone()),
                at: Some([
                    (parcel[0].max(old_bounds[0])) as i32,
                    (parcel[1].max(old_bounds[1])) as i32,
                ]),
                ..RejectDetail::of("occupied_or_road_support")
            });
            return Ok(Some("occupied_or_road_support"));
        }
    }
    if let Some(other) = plan
        .roadside_sites
        .iter()
        .find(|s| super::joint_frontage::incompatible(site, s, plan))
    {
        *detail = Some(RejectDetail {
            conflicting_site: Some(other.id.clone()),
            ..RejectDetail::of("reserved_entrance")
        });
        return Ok(Some("reserved_entrance"));
    }
    if let Some(reason) = support_check(
        site,
        n,
        mask,
        (plan.max_roadbed_fill_m * 1000.).round() as i32,
        detail,
    )? {
        return Ok(Some(reason));
    }
    let mut carves = base_carves.to_vec();
    let proposed = super::plan_terrain_carves(std::slice::from_ref(&e));
    carves.extend(proposed.iter().cloned());
    // Adding a building must not alter any previously fixed hard support.
    for old in existing {
        let area = bounds(
            old,
            if old.kind == "road" {
                3
            } else if old.grounding.frontage_road_point.is_some() {
                2
            } else {
                1
            },
        );
        for carve in &proposed {
            let x0 = (carve.cx - carve.half_w - carve.blend_m).floor() as i64;
            let z0 = (carve.cz - carve.half_d - carve.blend_m).floor() as i64;
            let x1 = (carve.cx + carve.half_w + carve.blend_m).ceil() as i64;
            let z1 = (carve.cz + carve.half_d + carve.blend_m).ceil() as i64;
            if !overlap(area, [x0, z0, x1, z1]) {
                continue;
            }
            for z in area[1].max(z0)..=area[3].min(z1) {
                for x in area[0].max(x0)..=area[2].min(x1) {
                    if sample(n, mask, &carves, x as i32, z as i32)?
                        != sample(n, mask, base_carves, x as i32, z as i32)?
                    {
                        *detail = Some(RejectDetail {
                            conflicting_entity: Some(old.entity_id.clone()),
                            at: Some([x as i32, z as i32]),
                            before_mm: Some(sample(
                                n,
                                mask,
                                base_carves,
                                x as i32,
                                z as i32,
                            )?),
                            after_mm: Some(sample(n, mask, &carves, x as i32, z as i32)?),
                            ..RejectDetail::of("existing_support_changed")
                        });
                        return Ok(Some("existing_support_changed"));
                    }
                }
            }
        }
    }
    // The connection is an engineering footprint, not an unchecked graph edge.
    let [cx, cz] = site.frontage_road_point.unwrap();
    let anchor = e.anchors.first().unwrap();
    let ax = i64::from(anchor.world_x);
    let az = i64::from(anchor.world_z);
    let corridor = [
        ax.min(i64::from(cx)) - 2,
        az.min(i64::from(cz)) - 2,
        ax.max(i64::from(cx)) + 3,
        az.max(i64::from(cz)) + 3,
    ];
    let connection_protected = connection_is_reserved(mask, &e, cx, cz);
    if connection_protected
        || water.intersects_corridor(ax as f64, az as f64, cx as f64, cz as f64, 2.)
    {
        *detail = Some(RejectDetail {
            at: Some([ax as i32, az as i32]),
            note: Some(if connection_protected {
                "access_corridor_reserved_zoning".into()
            } else {
                "access_corridor_intersects_water".into()
            }),
            ..RejectDetail::of("entrance_reserved_or_water")
        });
        return Ok(Some("entrance_reserved_or_water"));
    }
    for z in corridor[1]..corridor[3] {
        for x in corridor[0]..corridor[2] {
            let before = raw_mm(n, x as i32, z as i32)?;
            let after = sample(n, mask, &carves, x as i32, z as i32)?;
            if after < before
                || i64::from(after) - i64::from(before)
                    > (plan.max_roadbed_fill_m * 1000.).round() as i64
            {
                *detail = Some(RejectDetail {
                    at: Some([x as i32, z as i32]),
                    before_mm: Some(before),
                    after_mm: Some(after),
                    ..RejectDetail::of("entrance_fill_limit")
                });
                return Ok(Some("entrance_fill_limit"));
            }
        }
    }
    // Narrow complete local raster for the same pedestrian solver used after bake.
    let road = existing
        .iter()
        .find(|r| r.entity_id == site.road_id)
        .ok_or_else(|| err("frontage road missing"))?;
    let (rx, rz) = if road.rotation_y_deg == 0. {
        (e.world_x, road.world_z)
    } else {
        (road.world_x, e.world_z)
    };
    let x0 = (b[0] as i32).min(rx - 4) - 2;
    let z0 = (b[1] as i32).min(rz - 4) - 2;
    let x1 = (b[2] as i32).max(rx + 4) + 2;
    let z1 = (b[3] as i32).max(rz + 4) + 2;
    let local = SceneSpec {
        scene_id: "frontage-preflight".into(),
        width_m: (x1 - x0 + 1) as u32,
        depth_m: (z1 - z0 + 1) as u32,
        origin_x: x0,
        origin_z: z0,
        seed_offset: 0,
    };
    let mut values = Vec::new();
    for z in z0..=z1 {
        for x in x0..=x1 {
            values.push(f64::from(sample(n, mask, &carves, x, z)?) / 1000.);
        }
    }
    let mut raster = BakedTerrain::new(&local)?;
    raster.insert_chunk(
        x0,
        z0,
        local.width_m,
        local.depth_m,
        &encode_millimetres(local.width_m, local.depth_m, values)
            .map_err(|e| err(e.to_string()))?,
    )?;
    // Same final platform must be present, not simply a low centre point.
    let level = site.support_level_mm.unwrap();
    for z in e.world_z - hd as i32 - 2..=e.world_z + hd as i32 + 2 {
        for x in e.world_x - hw as i32 - 2..=e.world_x + hw as i32 + 2 {
            if raw_mm(&raster, x, z)? != level {
                *detail = Some(RejectDetail {
                    at: Some([x, z]),
                    before_mm: Some(level),
                    after_mm: Some(raw_mm(&raster, x, z)?),
                    note: Some("engineered_platform_sample_differs".into()),
                    ..RejectDetail::of("engineered_platform_not_flat")
                });
                return Ok(Some("engineered_platform_not_flat"));
            }
        }
    }
    let mut entities = existing.to_vec();
    entities.push(e.clone());
    let links = super::access::access_for_entity(&e, &entities, &raster, water);
    if !links
        .iter()
        .any(|l| l.connected && l.road_id.as_deref() == Some(&site.road_id))
    {
        let reason = if links.iter().any(|l| l.connected) {
            "access_to_different_road"
        } else {
            match links.first().map(|l| l.status.as_str()) {
                Some("entrance_step_requires_engineering") => "entrance_step_requires_engineering",
                Some("entrance_clearance_blocked") => "entrance_clearance_blocked",
                _ => "no_local_walkable_frontage",
            }
        };
        *detail = Some(RejectDetail {
            note: Some(links.first().map(|l| l.status.clone()).unwrap_or_default()),
            ..RejectDetail::of(reason)
        });
        return Ok(Some(reason));
    }
    Ok(None)
}
/// Immutable-to-the-caller candidate state: the raw source, the zoning mask and
/// the accumulated engineering footprint of everything already accepted. The
/// placement loop and the bounded supply probe share this exact state so a
/// measurement can never disagree with the placement it predicts.
pub(super) struct PlacementState<'a> {
    pub n: &'a BakedTerrain,
    pub mask: &'a ZoningMask,
    pub water: WaterGeometry,
    pub existing: Vec<EntityInstance>,
    pub carves: Vec<super::TerrainCarve>,
}
impl<'a> PlacementState<'a> {
    pub(super) fn new(
        n: &'a BakedTerrain,
        mask: &'a ZoningMask,
        water: WaterGeometry,
        existing: Vec<EntityInstance>,
    ) -> Self {
        let carves = super::plan_terrain_carves(&existing);
        Self {
            n,
            mask,
            water,
            existing,
            carves,
        }
    }
    /// Full production engineering preflight for one candidate.
    pub(super) fn assess(
        &self,
        site: &RoadsideSite,
        plan: &RoadNetworkPlan,
    ) -> Result<Option<&'static str>, TerrainError> {
        let mut detail = None;
        preflight(
            site,
            plan,
            self.n,
            self.mask,
            self.water,
            &self.existing,
            &self.carves,
            &mut detail,
        )
    }
    /// Same preflight decision with the structured evidence of the rejection.
    pub(super) fn assess_detailed(
        &self,
        site: &RoadsideSite,
        plan: &RoadNetworkPlan,
    ) -> Result<(Option<&'static str>, Option<RejectDetail>), TerrainError> {
        let mut detail = None;
        let reason = preflight(
            site,
            plan,
            self.n,
            self.mask,
            self.water,
            &self.existing,
            &self.carves,
            &mut detail,
        )?;
        Ok((reason, detail))
    }
    pub(super) fn sample_natural(
        &self,
        site: &mut RoadsideSite,
        parcel: [i64; 4],
    ) -> Result<(), TerrainError> {
        for zz in parcel[1]..parcel[3] {
            for xx in parcel[0]..parcel[2] {
                let h = raw_mm(self.n, xx as i32, zz as i32)? as f32 / 1000.;
                site.natural_min_m = site.natural_min_m.min(h);
                site.natural_max_m = site.natural_max_m.max(h);
            }
        }
        Ok(())
    }
    /// Accept a site the way the production loop does: its platform becomes
    /// fixed support for every later candidate.
    pub(super) fn commit(&mut self, site: &RoadsideSite) {
        let entity = site.entity();
        self.carves
            .extend(super::plan_terrain_carves(std::slice::from_ref(&entity)));
        self.existing.push(entity);
    }
}
/// Cumulative accounting for one authored request. Production placement and the
/// read-only diagnosis both drive this same struct, so remaining demand and the
/// request's own station budget can never diverge between them: demand is
/// measured across every road, never restarted per road.
#[derive(Debug, Clone)]
pub(super) struct RequestBudget {
    pub request: Request,
    pub site_ids: Vec<String>,
    pub attempted: u32,
    pub rejected: BTreeMap<String, u32>,
}
impl RequestBudget {
    pub(super) fn new(request: &Request) -> Self {
        Self {
            request: request.clone(),
            site_ids: vec![],
            attempted: 0,
            rejected: BTreeMap::new(),
        }
    }
    pub(super) fn satisfied(&self) -> bool {
        self.site_ids.len() >= self.request.count as usize
    }
    pub(super) fn remaining(&self) -> u32 {
        self.request.count.saturating_sub(self.site_ids.len() as u32)
    }
    pub(super) fn budget_left(&self) -> bool {
        self.attempted < self.request.max_candidate_attempts
    }
    /// Claim one station attempt. False means this request is finished for the
    /// whole placement, either by demand or by its own cumulative budget.
    pub(super) fn claim_attempt(&mut self) -> bool {
        if self.satisfied() || !self.budget_left() {
            return false;
        }
        self.attempted += 1;
        true
    }
    pub(super) fn reject(&mut self, reason: &str) {
        *self.rejected.entry(reason.into()).or_default() += 1;
    }
    pub(super) fn accept(&mut self, id: &str) {
        self.site_ids.push(id.into());
    }
    pub(super) fn stop_reason(&self) -> String {
        if self.satisfied() {
            "demand_satisfied".into()
        } else if !self.budget_left() {
            "candidate_budget_exhausted".into()
        } else {
            "no_feasible_frontage".into()
        }
    }
}
pub(super) fn append(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    n: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    style: &serde_json::Value,
) -> Result<(), TerrainError> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    let requests: Vec<_> = program
        .requests
        .into_iter()
        .filter(|r| r.scene_id == scene.scene_id)
        .collect();
    if requests.is_empty() {
        return Ok(());
    }
    if n.encoding() != Some(HeightEncoding::Millimetre) {
        return Err(err("frontage requires millimetre source"));
    }
    let zones = zones.ok_or_else(|| err("frontage requires zoning"))?;
    let mask = ZoningMask::new(scene, zones)?;
    // The append is atomic on error, while unsatisfied valid demand is reported.
    let mut staged = plan.clone();
    let roads = profile_roads(&staged);
    let mut state = PlacementState::new(
        n,
        &mask,
        water,
        {
            let mut existing = roads.clone();
            existing.extend(staged.site_entities());
            existing
        },
    );
    let mut report = Report {
        version: 1,
        model: "local-road-station-frontage-v1".into(),
        outcomes: vec![],
    };
    for r in requests {
        let mut budget = RequestBudget::new(&r);
        'search: for road in &roads {
            let Some(stations) = stations(road, &r)? else {
                continue;
            };
            for station in stations {
                for side in [-1_i32, 1] {
                    if !budget.claim_attempt() {
                        break 'search;
                    }
                    let mut site =
                        candidate_site(road, &r, &scene.scene_id, station, side)?;
                    let parcel = parcel_boundary(&site)?;
                    let mut reason = envelope_reason(parcel, scene, zones, &r.purpose);
                    if reason.is_none() {
                        reason = state.assess(&site, &staged)?;
                    }
                    if let Some(reason) = reason {
                        budget.reject(reason);
                        continue;
                    }
                    state.sample_natural(&mut site, parcel)?;
                    state.commit(&site);
                    budget.accept(&site.id);
                    staged.roadside_sites.push(site);
                }
            }
        }
        report.outcomes.push(Outcome {
            request: budget.request.clone(),
            site_ids: budget.site_ids.clone(),
            attempted: budget.attempted,
            unfilled: budget.remaining(),
            rejected: budget.rejected.clone(),
            stop_reason: budget.stop_reason(),
        });
    }
    staged.frontage = Some(report);
    *plan = staged;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub status: String,
    pub requested: u32,
    pub planned: usize,
    pub realized_with_access: usize,
    pub unfilled: u32,
    pub failures: Vec<String>,
    /// Machine-readable realized-with-access identities. Additive: the counts
    /// and thresholds above are unchanged, and older documents without it
    /// still deserialize.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub realized_ids: Vec<String>,
}
/// Call after road-network source replay; metadata alone never proves realization.
pub fn audit(plan: &RoadNetworkPlan, entities: &[EntityInstance], access: &AccessGraph) -> Audit {
    let mut a = Audit {
        status: "pass".into(),
        requested: 0,
        planned: 0,
        realized_with_access: 0,
        unfilled: 0,
        failures: vec![],
        realized_ids: vec![],
    };
    if let Some(report) = &plan.frontage {
        for o in &report.outcomes {
            a.requested += o.request.count;
            a.planned += o.site_ids.len();
            a.unfilled += o.unfilled;
            for id in &o.site_ids {
                let site = plan.roadside_sites.iter().find(|s| &s.id == id);
                let found: Vec<_> = entities.iter().filter(|e| &e.entity_id == id).collect();
                let exact = site.is_some_and(|s| {
                    let mut expected = s.entity();
                    expected.normalize_spatial_semantics();
                    expected.bounds = None;
                    found.len() == 1 && {
                        let mut actual = found[0].clone();
                        actual.bounds = None;
                        serde_json::to_value(actual).ok() == serde_json::to_value(expected).ok()
                    }
                });
                if exact
                    && site.is_some_and(|s| {
                        access.links.iter().any(|l| {
                            l.entity_id == *id
                                && l.connected
                                && l.road_id.as_deref() == Some(&s.road_id)
                        })
                    })
                {
                    a.realized_with_access += 1;
                    a.realized_ids.push(id.clone());
                } else {
                    a.failures.push(format!(
                        "frontage building missing/changed/disconnected {id}"
                    ));
                }
            }
        }
    }
    if a.unfilled > 0 {
        a.failures.push("frontage demand remains unfilled".into());
    }
    if !a.failures.is_empty() {
        a.status = "fail".into();
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(
        origin: i32,
    ) -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
        serde_json::Value,
    ) {
        let s = SceneSpec {
            scene_id: "frontage".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: origin,
            origin_z: origin,
            seed_offset: 0,
        };
        let mut n = BakedTerrain::new(&s).unwrap();
        n.insert_chunk(
            origin,
            origin,
            512,
            512,
            &encode_millimetres(512, 512, vec![-2.123; 512 * 512]).unwrap(),
        )
        .unwrap();
        let w = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &s,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "city".into(),
            kind: "urban".into(),
            world_x: origin,
            world_z: origin,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let mut p:RoadNetworkPlan=serde_json::from_value(serde_json::json!({"model":"fixture","naturalHeightfieldHash":n.content_hash().unwrap(),"sceneId":"frontage","status":"planned_connected_network","gridSpacingM":64,"maxRoadbedFillM":6,"levelM":0,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,"nodes":[{"id":0,"worldX":origin+32,"worldZ":origin+128},{"id":1,"worldX":origin+96,"worldZ":origin+128}],"edges":[{"id":"base","from":0,"to":1,"worldX":origin+64,"worldZ":origin+128,"lengthM":64,"rotationYDeg":0,"naturalMinM":-2,"naturalMaxM":-2}],"roadsideSites":[]})).unwrap();
        let ext = serde_json::json!({"roadExtensions":{"version":1,"corridors":[{"sceneId":"frontage","start":[origin+96,origin+128],"end":[origin+352,origin+128],"roadWidthM":8,"shoulderWidthM":3,"maxFillMm":6000,"maxRiseMmPerM":80,"startLevelMm":0,"endLevelMm":2374,"junctionLengthM":8}]}});
        super::super::road_extensions::append(&mut p, &s, &n, w, &zones, &ext).unwrap();
        let style = serde_json::json!({"frontageProgram":{"version":1,"requests":[{"id":"homes","sceneId":"frontage","purpose":"urban","kind":"residential_home","count":4,"widthM":12,"depthM":10,"heightM":8,"stationSpacingM":16,"setbackM":8,"maxCandidateAttempts":128}]}});
        (s, n, w, zones, p, style)
    }
    #[test]
    fn local_millimetre_buildings_have_access_and_preserve_roads() {
        let (s, n, w, z, mut p, style) = fixture(0);
        let old = serde_json::to_value(p.entities()).unwrap();
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(o.unfilled, 0, "{:?}", o);
        assert_eq!(serde_json::to_value(p.entities()).unwrap(), old);
        assert_eq!(p.roadside_sites.len(), 4);
        assert!(
            p.roadside_sites
                .iter()
                .any(|s| s.support_level_mm.unwrap() % 1000 != 0)
        );
        for site in &p.roadside_sites {
            let mm = site.support_level_mm.unwrap();
            let e = site.entity();
            assert_eq!(e.world_y, mm.div_euclid(1000));
            assert_eq!((e.ground_height_m() * 1000.).round() as i32, mm);
        }
        let mut again = fixture(0).4;
        append(&mut again, &s, &n, w, Some(&z), &style).unwrap();
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::to_value(again).unwrap()
        );
    }
    #[test]
    fn explicit_parcel_insets_are_validated_preserved_and_used_for_conflicts() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        style["frontageProgram"]["requests"][0]["parcelInsets"] = insets.clone();
        style["frontageProgram"]["requests"][0]["count"] = serde_json::json!(3);
        style["frontageProgram"]["requests"][0]["stationSpacingM"] = serde_json::json!(16);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        assert!(p
            .roadside_sites
            .iter()
            .all(|site| site.parcel_insets.is_some()));
        for site in &p.roadside_sites {
            let boundary = parcel_boundary(site).unwrap();
            assert_eq!(boundary[2] - boundary[0], 20);
            assert_eq!(boundary[3] - boundary[1], 26);
        }
        assert!(p.frontage.as_ref().unwrap().outcomes[0]
            .rejected
            .contains_key("occupied_or_road_support"));

        let mut too_deep_front = fixture(0).5;
        too_deep_front["frontageProgram"]["requests"][0]["parcelInsets"] =
            serde_json::json!({"frontM":10,"sideM":4,"rearM":12});
        assert!(from_style(&too_deep_front).is_err());
    }
    #[test]
    fn feasible_long_profiled_control_realizes_twenty_four_explicit_parcels() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        let context = super::super::road_alignment::AlignmentContext::new(&s, &n, w, &z).unwrap();
        let request = super::super::road_alignment::CorridorRequest {
            scene_id: "frontage".into(),
            start: [352, 128],
            end: [480, 128],
            road_width_m: 8,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(2374),
            end_level_mm: Some(3000),
            junction_length_m: 8,
        };
        super::super::road_extensions::append_one(
            &mut p,
            &context,
            &request,
            "generated.road-extension.frontage.1".into(),
        )
        .unwrap();
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        let mut homes = style["frontageProgram"]["requests"][0].clone();
        homes["id"] = serde_json::json!("homes-b01");
        homes["count"] = serde_json::json!(16);
        homes["stationSpacingM"] = serde_json::json!(24);
        homes["parcelInsets"] = insets.clone();
        let mut shops = homes.clone();
        shops["id"] = serde_json::json!("shops-b01");
        shops["kind"] = serde_json::json!("storefront");
        shops["heightM"] = serde_json::json!(6);
        shops["count"] = serde_json::json!(8);
        style["frontageProgram"]["requests"] = serde_json::json!([homes, shops]);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let outcomes = &p.frontage.as_ref().unwrap().outcomes;
        assert_eq!(outcomes.iter().map(|o| o.unfilled).sum::<u32>(), 0, "{outcomes:?}");
        assert_eq!(p.roadside_sites.len(), 24);
        for site in &p.roadside_sites {
            let boundary = parcel_boundary(site).unwrap();
            assert_eq!((boundary[2] - boundary[0]) * (boundary[3] - boundary[1]), 520);
            assert!(site.parcel_insets.is_some());
        }
    }
    #[test]
    fn demand_and_budget_shortfalls_are_not_resized_or_silently_passed() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        style["frontageProgram"]["requests"][0]["count"] = serde_json::json!(100);
        style["frontageProgram"]["requests"][0]["maxCandidateAttempts"] = serde_json::json!(1);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(o.attempted, 1);
        assert!(o.unfilled >= 99);
        assert_eq!(o.stop_reason, "candidate_budget_exhausted");
        assert_eq!(audit(&p, &[], &AccessGraph::default()).status, "fail");
    }
    #[test]
    fn wrong_purpose_and_protection_do_not_earn_buildings() {
        let (s, n, w, mut z, mut p, style) = fixture(0);
        z[0].kind = "nature".into();
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        assert!(p.roadside_sites.is_empty());
        assert_eq!(p.frontage.as_ref().unwrap().outcomes[0].unfilled, 4);
        assert!(
            p.frontage.as_ref().unwrap().outcomes[0]
                .rejected
                .contains_key("wrong_purpose_or_protected")
        );
    }
    #[test]
    fn nonzero_origin_covariance_and_unknown_inputs() {
        let mut results = vec![];
        for origin in [0, -512] {
            let (s, n, w, z, mut p, style) = fixture(origin);
            append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
            results.push(
                p.roadside_sites
                    .iter()
                    .map(|a| (a.world_x - origin, a.world_z - origin, a.support_level_mm))
                    .collect::<Vec<_>>(),
            );
        }
        assert!(!results[0].is_empty());
        assert_eq!(results[0], results[1]);
        let mut style = fixture(0).5;
        style["frontageProgram"]["requests"][0]["worldX"] = serde_json::json!(100);
        assert!(from_style(&style).is_err());
    }
    #[test]
    fn invalid_source_does_not_partially_modify_network() {
        let (s, _n, w, z, mut p, style) = fixture(0);
        let n = BakedTerrain::new(&s).unwrap();
        let old = serde_json::to_value(&p).unwrap();
        assert!(append(&mut p, &s, &n, w, Some(&z), &style).is_err());
        assert_eq!(serde_json::to_value(p).unwrap(), old);
    }
    #[test]
    fn explicit_connection_suppresses_old_blended_ridge_but_never_cuts_nature() {
        let (s, n, w, z, mut p, style) = fixture(0);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let e = p.roadside_sites.last().unwrap().entity();
        let [rx, rz] = e.grounding.frontage_road_point.unwrap();
        let a = &e.anchors[0];
        let x = rx + (a.world_x - rx).signum() * 8;
        let zz = rz + (a.world_z - rz).signum() * 8;
        let ridge = super::super::TerrainCarve {
            profile: None,
            cx: f64::from(x) + 8.,
            cz: f64::from(zz),
            half_w: 1.,
            half_d: 1.,
            target_h_m: 10.,
            blend_m: 50.,
            priority: 1,
            disturbance: 160,
        };
        let mut before = e.clone();
        before.grounding.frontage_road_point = None;
        let mut old = super::super::plan_terrain_carves(&[before]);
        old.push(ridge.clone());
        let mut new = super::super::plan_terrain_carves(&[e.clone()]);
        new.push(ridge);
        // The real road activates hard-support ownership and occupies the other 7m.
        for c in super::super::plan_terrain_carves(&p.entities()) {
            old.push(c.clone());
            new.push(c);
        }
        let base = f64::from(n.height(x, zz).unwrap());
        let target = f64::from(e.ground_height_m());
        assert!(super::super::apply_terrain_carves(base, x, zz, &old) > target + 1.);
        assert!((super::super::apply_terrain_carves(base, x, zz, &new) - target).abs() < 0.0001);
        assert!(super::super::apply_terrain_carves(target + 0.1, x, zz, &new) >= target + 0.1);
    }
    #[test]
    fn manifest_asset_contract_is_checked_before_placement() {
        let mut m: WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/local-height-frontage.manifest.json"
        ))
        .unwrap();
        assert!(validate_manifest(&m).is_ok());
        m.style["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(18);
        assert!(
            validate_manifest(&m)
                .unwrap_err()
                .to_string()
                .contains("asset contract")
        );
        m.style["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(24);
        m.style["frontageProgram"]["requests"][0]["sceneId"] = serde_json::json!("missing");
        assert!(validate_manifest(&m).is_err());
    }
    #[test]
    fn negative_millimetre_frontage_is_not_raised_by_legacy_quarter_metre_rebasing() {
        // Actual first failed r48 maximum-world site; no full work/ fixture dependency.
        let m: WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/maximum-local-height-frontage.manifest.json"
        ))
        .unwrap();
        let scene = &m.scenes[0];
        let water = super::super::water_geometry(
            super::super::water_kind(&m.style),
            &m.landmarks,
            scene,
            &m.style,
        );
        let site:RoadsideSite=serde_json::from_value(serde_json::json!({"program":{"requestId":"urban-homes","kind":"residential_home","heightM":8},"id":"generated.frontage.regression","roadId":"road","worldX":3788,"worldZ":4920,"widthM":24,"depthM":24,"supportLevelM":-2,"supportLevelMm":-1354,"frontageRoadPoint":[3788,4896],"rotationYDeg":0,"naturalMinM":-6.666,"naturalMaxM":-3.232})).unwrap();
        let e = site.entity();
        let expected = serde_json::to_value(&e).unwrap();
        let mut entities = vec![e];
        let carves = super::super::plan_terrain_carves(&entities);
        super::super::rebase_grounded_entities_to_carved_surface(
            m.world.seed,
            water,
            &mut entities,
            &carves,
        );
        assert_eq!(
            serde_json::to_value(&entities[0]).unwrap(),
            expected,
            "a precise planned platform must not pass through quarter-metre rounding/ceil"
        );
        let local = SceneSpec {
            scene_id: "regression".into(),
            width_m: 32,
            depth_m: 32,
            origin_x: 3772,
            origin_z: 4904,
            seed_offset: 0,
        };
        for mm in [-1354, -1353] {
            let mut t = BakedTerrain::new(&local).unwrap();
            t.insert_chunk(
                local.origin_x,
                local.origin_z,
                32,
                32,
                &encode_millimetres(32, 32, vec![f64::from(mm) / 1000.; 32 * 32]).unwrap(),
            )
            .unwrap();
            super::super::rebase_entities_to_baked_surface(&mut entities, &t);
            assert_eq!(
                serde_json::to_value(&entities[0]).unwrap(),
                expected,
                "even wrong terrain must be audited, not silently heal the planned entity"
            );
        }
    }
}
