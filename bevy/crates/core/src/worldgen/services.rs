//! Explicit service footprints on verified lowland frontage. No implicit
//! population, staffing, jurisdiction or legacy-template demand is inferred.
use super::{
    AccessGraph, EntityInstance, SceneSpec, WaterGeometry,
    road_network::{RoadNetworkPlan, RoadsideSite},
    zoning::ZoningMask,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceProgram {
    pub version: u32,
    pub requests: Vec<ServiceRequest>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub housing_minimum: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRequest {
    pub id: String,
    pub scene_id: String,
    pub kind: String,
    pub count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SiteProgram {
    pub request_id: String,
    pub kind: String,
    pub height_m: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_cohort: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceOutcome {
    pub request: ServiceRequest,
    pub planned_site_ids: Vec<String>,
    pub unfilled_count: u32,
    pub rejected_candidates: BTreeMap<String, u32>,
    pub status: String,
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<ServiceProgram>, String> {
    let Some(value) = style.get("settlementProgram") else {
        return Ok(None);
    };
    let program: ServiceProgram = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid settlementProgram: {e}"))?;
    if !matches!(program.version, 1 | 2) || program.requests.len() > 64 {
        return Err("unsupported service program version or more than 64 requests".into());
    }
    if (program.version == 1 && !program.housing_minimum.is_empty())
        || program.housing_minimum.values().any(|n| *n > 65536)
    {
        return Err("housingMinimum requires v2 and counts at most 65536".into());
    }
    let mut ids = BTreeSet::new();
    let mut count = 0_u64;
    for r in &program.requests {
        if r.id.is_empty()
            || r.id.len() > 64
            || !r
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&r.id)
            || r.scene_id.is_empty()
            || r.count == 0
            || r.count > 32
        {
            return Err("invalid or duplicate service request id/scene/count".into());
        }
        // Bounds match the existing asset contract, not a real building code.
        let (minw, maxw, mind, maxd, minh, maxh) = match r.kind.as_str() {
            "convenience_store" => (6, 40, 6, 30, 3, 12),
            "school" => (30, 128, 26, 120, 4, 30),
            "church" => (12, 80, 12, 80, 3, 40),
            "town_hall" => (30, 128, 24, 120, 8, 60),
            "market" => (36, 128, 30, 128, 3, 20),
            _ => return Err(format!("unsupported requested service kind {}", r.kind)),
        };
        if !(minw..=maxw).contains(&r.width_m)
            || !(mind..=maxd).contains(&r.depth_m)
            || !(minh..=maxh).contains(&r.height_m)
            || r.width_m % 2 != 0
            || r.depth_m % 2 != 0
        {
            return Err(format!(
                "service {} has invalid exact dimensions; no automatic shrinking",
                r.id
            ));
        }
        count += u64::from(r.count);
    }
    if count > 64 {
        return Err("service program exceeds 64 requested facilities".into());
    }
    Ok(Some(program))
}
// Inclusive support extents; occupancy rectangles themselves are half-open.
fn overlap(x: i64, z: i64, hw: i64, hd: i64, xx: i64, zz: i64, ww: i64, dd: i64) -> bool {
    (x - xx).abs() < hw + ww && (z - zz).abs() < hd + dd
}
pub(super) fn entrance_corridor(
    site: &RoadsideSite,
    plan: &RoadNetworkPlan,
) -> Option<(i64, i64, i64, i64)> {
    let road = plan.edges.iter().find(|r| r.id == site.road_id)?;
    let front = i64::from(site.depth_m / 2 + 1);
    let (dx, dz) = match site.rotation_y_deg as i32 {
        90 => (-front, 0),
        180 => (0, front),
        270 => (front, 0),
        _ => (0, -front),
    };
    let (x, z) = (i64::from(site.world_x) + dx, i64::from(site.world_z) + dz);
    Some(if road.rotation_y_deg == 0. {
        (
            x,
            (z + i64::from(road.world_z)) / 2,
            1,
            (z - i64::from(road.world_z)).abs() / 2 + 1,
        )
    } else {
        (
            (x + i64::from(road.world_x)) / 2,
            z,
            (x - i64::from(road.world_x)).abs() / 2 + 1,
            1,
        )
    })
}

pub(super) fn append(
    result: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[super::LandUseZone],
    program: &ServiceProgram,
    pad_conversion: super::frontage::RuralConversion,
    road_cut_mm: i32,
) -> Result<(), TerrainError> {
    append_with(
        result,
        scene,
        natural,
        water,
        zones,
        program,
        pad_conversion,
        None,
        false,
        &[12_i64, 28, 44],
        road_cut_mm,
    )
}

/// The shoreline commercial row's own entry point: the SAME pad programme, on the streets
/// `worldgen::shorefront` assigned to the row, carrying the published shore clearance.
///
/// Two differences from the service programme, both of which would be wrong anywhere else: the
/// candidate search walks only the row's own streets (a service may go anywhere in the city, a quay
/// shop may not), and a candidate whose pad does not keep the 4 m shore clearance is refused by name
/// (`shore_band`) instead of being placed in the public passage band. The service pads do not carry
/// that gate because their candidate test already dilates the water by 2 m and they are not planned
/// against a shoreline on purpose; the row is.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_shorefront(
    result: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[super::LandUseZone],
    program: &ServiceProgram,
    pad_conversion: super::frontage::RuralConversion,
    only_roads: &std::collections::BTreeSet<String>,
    setbacks: &[i64],
    road_cut_mm: i32,
) -> Result<(), TerrainError> {
    append_with(
        result,
        scene,
        natural,
        water,
        zones,
        program,
        pad_conversion,
        Some(only_roads),
        true,
        setbacks,
        road_cut_mm,
    )
}

/// The one pad placement loop. `only_roads` restricts the candidate search to a stated street set,
/// and `enforce_shore_band` adds the shoreline public passage gate; both are inert for every caller
/// that passes `None`/`false`, which is why the service programme's own placements are unchanged.
#[allow(clippy::too_many_arguments)]
fn append_with(
    result: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[super::LandUseZone],
    program: &ServiceProgram,
    pad_conversion: super::frontage::RuralConversion,
    only_roads: Option<&std::collections::BTreeSet<String>>,
    enforce_shore_band: bool,
    setbacks: &[i64],
    // The plan's graded-road cut allowance in millimetres (`0` = none). The
    // acceptance gate below composes the surface the bake publishes, so it must
    // know the street's own excavation.
    road_cut_mm: i32,
) -> Result<(), TerrainError> {
    let mask = ZoningMask::new(scene, zones)?;
    for r in program
        .requests
        .iter()
        .filter(|r| r.scene_id == scene.scene_id)
    {
        let mut outcome = ServiceOutcome {
            request: r.clone(),
            planned_site_ids: vec![],
            unfilled_count: r.count,
            rejected_candidates: BTreeMap::new(),
            status: "unfilled".into(),
        };
        let Some(level) = result.level_m else {
            outcome
                .rejected_candidates
                .insert("no_connected_road_network".into(), 1);
            result.service_outcomes.push(outcome);
            continue;
        };
        // B01-T2 R12: the graded surface of every road this program may use, taken
        // through the same priority `RoadNetworkPlan::entities()` publishes (edge
        // profile first, then the extension design). A pad whose assigned road is
        // graded must be declared on that road's own surface, not on the network's
        // flat `level_m`: the two datums differ wherever the profile dips, and the
        // development audit requires the checked route to reach the assigned road.
        let road_entities = result.entities();
        let road_profiles: BTreeMap<String, super::road_profile::RoadProfile> = road_entities
            .iter()
            .filter_map(|e| {
                e.grounding
                    .road_profile
                    .clone()
                    .map(|p| (e.entity_id.clone(), p))
            })
            .collect();
        // The same composition the roadside station scan uses (rule A'): what the
        // ROADS already require inside a candidate pad's window. A pad is a
        // flat, fill-only earthwork, so its declared top has to be at least this
        // and at most it plus the walkable entrance rise from the road it is
        // assigned - otherwise the checked route detours to another road and the
        // development audit reports `no_verified_assigned_road_access`.
        let road_carves = super::plan_terrain_carves(&road_entities);
        let mut examined = 0_u32;
        'instances: for serial in 0..r.count {
            let mut accepted = None;
            'candidates: for road in &result.edges {
                if only_roads.is_some_and(|only| !only.contains(&road.id)) {
                    continue;
                }
                for setback in setbacks.iter().copied() {
                    for side in [-1_i64, 1] {
                        for along in [0_i64, -24, 24, -16, 16, -8, 8] {
                            examined += 1;
                            if examined > 65536 {
                                *outcome
                                    .rejected_candidates
                                    .entry("candidate_search_budget_exhausted".into())
                                    .or_default() += 1;
                                break 'instances;
                            }
                            let yaw = if road.rotation_y_deg == 0. {
                                if side < 0 { 180. } else { 0. }
                            } else if side < 0 {
                                270.
                            } else {
                                90.
                            };
                            let (hw, hd) = if road.rotation_y_deg == 0. {
                                (i64::from(r.width_m / 2), i64::from(r.depth_m / 2))
                            } else {
                                (i64::from(r.depth_m / 2), i64::from(r.width_m / 2))
                            };
                            let normal = i64::from(r.depth_m / 2) + setback;
                            let (x, z) = if road.rotation_y_deg == 0. {
                                (
                                    i64::from(road.world_x) + along,
                                    i64::from(road.world_z) + normal * side,
                                )
                            } else {
                                (
                                    i64::from(road.world_x) + normal * side,
                                    i64::from(road.world_z) + along,
                                )
                            };
                            // The pad's frontage point on its assigned road's axis.
                            let frontage = if road.rotation_y_deg == 0. {
                                (x, i64::from(road.world_z))
                            } else {
                                (i64::from(road.world_x), z)
                            };
                            let own_mm = road_profiles.get(&road.id).and_then(|p| {
                                p.height_at(frontage.0 as f64, frontage.1 as f64)
                                    .map(|h| (h * 1000.).round() as i64)
                            });
                            // Rule A': a graded road's own surface at the entrance is
                            // the pad's datum, taken as the nearest whole metre. A pad
                            // is a flat fill-only earthwork whose declared top is an
                            // integer level, so the metre snap is what keeps the frozen
                            // plan, the final entity and the raster equal; the snap
                            // costs at most 0.5 m and is accepted only while it stays
                            // inside the one-cell walkable entrance step the apron gate
                            // allows.
                            let (declared_mm, entrance_rise_m) = match own_mm {
                                Some(own) => {
                                    let snapped =
                                        ((own as f64 + 500.) / 1000.).floor() as i64 * 1000;
                                    (snapped, (snapped - own).abs() as f64 / 1000.)
                                }
                                None => (i64::from(level) * 1000, 0.),
                            };
                            let declared_f = declared_mm as f64 / 1000.;
                            let mut reason = None;
                            if x - hw - 2 < i64::from(scene.origin_x)
                                || z - hd - 2 < i64::from(scene.origin_z)
                                || x + hw + 2
                                    >= i64::from(scene.origin_x) + i64::from(scene.width_m)
                                || z + hd + 2
                                    >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
                            {
                                reason = Some("outside_scene");
                            } else if mask
                                .check(
                                    r.id.clone(),
                                    &r.kind,
                                    x as i32,
                                    z as i32,
                                    (hw + 2) as i32,
                                    (hd + 2) as i32,
                                )
                                .is_some()
                            {
                                reason = Some("reserved_land_intersection");
                            } else if water.intersects_axis_aligned(
                                x as f64,
                                z as f64,
                                (hw + 2) as f64,
                                (hd + 2) as f64,
                            ) {
                                reason = Some("water_intersection");
                            } else if enforce_shore_band
                                && !water.clears_shore(x as f64, z as f64, hw as f64, hd as f64)
                            {
                                // The shoreline row may not stand in the public passage band. The
                                // clearance is the frozen constant that DEFINES the band, so a pad
                                // that keeps it owns no band cell (`water_query::shore_band_cell`).
                                reason = Some("shore_band");
                            } else if result.edges.iter().any(|e| {
                                let (w, d) = if e.rotation_y_deg == 0. {
                                    (35, 7)
                                } else {
                                    (7, 35)
                                };
                                overlap(
                                    x,
                                    z,
                                    hw + 2,
                                    hd + 2,
                                    i64::from(e.world_x),
                                    i64::from(e.world_z),
                                    w,
                                    d,
                                )
                            }) {
                                reason = Some("road_clearance");
                            } else if result.roadside_sites.iter().any(|s| {
                                let (w, d) = s.world_half_extents();
                                overlap(
                                    x,
                                    z,
                                    hw + 2,
                                    hd + 2,
                                    i64::from(s.world_x),
                                    i64::from(s.world_z),
                                    i64::from(w + 2),
                                    i64::from(d + 2),
                                )
                            }) {
                                reason = Some("existing_parcel");
                            }
                            // Reserve the direct, 2m-wide entrance corridor before
                            // accepting an outer-row site. Actual access is audited
                            // again against the final raster and all final entities.
                            let (cx, cz, cw, cd) = if road.rotation_y_deg == 0. {
                                (
                                    x,
                                    (z - side * (hd + 1) + i64::from(road.world_z)) / 2,
                                    1,
                                    (setback - 1) / 2 + 1,
                                )
                            } else {
                                (
                                    (x - side * (hw + 1) + i64::from(road.world_x)) / 2,
                                    z,
                                    (setback - 1) / 2 + 1,
                                    1,
                                )
                            };
                            if reason.is_none()
                                && (mask
                                    .check(
                                        r.id.clone(),
                                        "service_access",
                                        cx as i32,
                                        cz as i32,
                                        cw as i32,
                                        cd as i32,
                                    )
                                    .is_some()
                                    || water.intersects_axis_aligned(
                                        cx as f64, cz as f64, cw as f64, cd as f64,
                                    ))
                            {
                                reason = Some("entrance_corridor_reserved_or_water");
                            }
                            if reason.is_none()
                                && result.roadside_sites.iter().any(|s| {
                                    let (w, d) = s.world_half_extents();
                                    overlap(
                                        cx,
                                        cz,
                                        cw,
                                        cd,
                                        i64::from(s.world_x),
                                        i64::from(s.world_z),
                                        i64::from(w + 2),
                                        i64::from(d + 2),
                                    )
                                })
                            {
                                reason = Some("entrance_corridor_blocked");
                            }
                            if reason.is_none()
                                && result.edges.iter().filter(|e| e.id != road.id).any(|e| {
                                    let (w, d) = if e.rotation_y_deg == 0. {
                                        (32, 4)
                                    } else {
                                        (4, 32)
                                    };
                                    overlap(
                                        cx,
                                        cz,
                                        cw,
                                        cd,
                                        i64::from(e.world_x),
                                        i64::from(e.world_z),
                                        w,
                                        d,
                                    )
                                })
                            {
                                reason = Some("entrance_reaches_different_road");
                            }
                            // The checked route always ends on the road nearest the
                            // entrance, so the pad has to be nearer its OWN road than
                            // any other: beside a crossing road the walk reaches the
                            // crossing one first and the development audit reports
                            // `no_verified_assigned_road_access` for a site that is
                            // perfectly reachable - just not from the road it was
                            // assigned to. Same rule the roadside station scan uses.
                            if reason.is_none() {
                                let own = road_entities
                                    .iter()
                                    .find(|entity| entity.entity_id == road.id)
                                    .map_or(0., |entity| {
                                        super::road_network::road_rect_distance(
                                            entity, x as i32, z as i32,
                                        )
                                    });
                                let nearest_other = road_entities
                                    .iter()
                                    .filter(|entity| entity.entity_id != road.id)
                                    .map(|entity| {
                                        super::road_network::road_rect_distance(
                                            entity, x as i32, z as i32,
                                        )
                                    })
                                    .fold(f64::INFINITY, f64::min);
                                if own > nearest_other {
                                    reason = Some("entrance_reaches_different_road");
                                }
                            }
                            if reason.is_none()
                                && result.roadside_sites.iter().any(|site| {
                                    entrance_corridor(site, result).is_some_and(
                                        |(cx, cz, cw, cd)| {
                                            overlap(x, z, hw + 2, hd + 2, cx, cz, cw, cd)
                                        },
                                    )
                                })
                            {
                                reason = Some("existing_entrance_corridor");
                            }
                            let mut min = f32::INFINITY;
                            let mut max = f32::NEG_INFINITY;
                            let mut carved_max = f32::NEG_INFINITY;
                            if reason.is_none() && entrance_rise_m > super::access::WALKABLE_GRADE {
                                reason = Some("entrance_rise_exceeds_contract");
                            }
                            if reason.is_none() {
                                'samples: for zz in z - hd - 2..=z + hd + 2 {
                                    for xx in x - hw - 2..=x + hw + 2 {
                                        let h = natural.height(xx as i32, zz as i32).ok_or_else(
                                            || {
                                                TerrainError(
                                                    "missing service natural sample".into(),
                                                )
                                            },
                                        )?;
                                        min = min.min(h);
                                        max = max.max(h);
                                        // What the roads already require inside the
                                        // pad's own window: a neighbouring road's
                                        // earthwork must not push the pad's flat top
                                        // out of the audited tolerance.
                                        carved_max = carved_max.max(super::apply_terrain_carves(
                                            f64::from(h),
                                            xx as i32,
                                            zz as i32,
                                            &road_carves,
                                        )
                                            as f32);
                                        // A pad is realized by a fill-only carve, so its
                                        // declared flat top has to be the surface its own
                                        // window can actually hold: never below the natural
                                        // ground it keeps, never above it plus the fill cap.
                                        if f64::from(h) > declared_f
                                            || declared_f - f64::from(h)
                                                > f64::from(result.max_roadbed_fill_m)
                                            || declared_f
                                                > f64::from(h)
                                                    + mask.fill_capacity_at(xx as i32, zz as i32)
                                        {
                                            reason = Some("support_height_or_fill_envelope");
                                            break 'samples;
                                        }
                                    }
                                }
                                if reason.is_none()
                                    && f64::from(carved_max)
                                        > declared_f + super::access::WALKABLE_GRADE
                                {
                                    reason = Some("pad_carve_exceeds_declared_level");
                                }
                            }
                            if let Some(reason) = reason {
                                *outcome
                                    .rejected_candidates
                                    .entry(reason.into())
                                    .or_default() += 1;
                                continue;
                            }
                            let mut site = RoadsideSite {
                                id: format!(
                                    "generated.service.{}.{}.{}",
                                    scene.scene_id, r.id, serial
                                ),
                                road_id: road.id.clone(),
                                world_x: x as i32,
                                world_z: z as i32,
                                width_m: r.width_m,
                                depth_m: r.depth_m,
                                support_level_m: i32::try_from(declared_mm)
                                    .map(|mm| mm.div_euclid(1000))
                                    .unwrap_or(level),
                                parcel_insets: None,
                                // A service pad keeps the integer-level contract the service
                                // audit recognises (`support_level_mm` marks a graded pad it
                                // deliberately does not credit), and a service building is not
                                // a frontage-program kind, so it carries no frontage point.
                                support_level_mm: None,
                                frontage_road_point: None,
                                bounded_cut_mm: None,
                                earthwork_blend_m: None,
                                rotation_y_deg: yaw,
                                natural_min_m: min,
                                natural_max_m: max,
                                program: Some(SiteProgram {
                                    request_id: r.id.clone(),
                                    kind: r.kind.clone(),
                                    height_m: r.height_m,
                                    district_id: None,
                                    construction_cohort: None,
                                }),
                                purpose: None,

                                entry_corridor: None,
                            };
                            // The checked entrance is judged BEFORE the purpose verdict, so a pad whose
                            // own street cannot be walked to never converts land, never books a service
                            // outcome and never publishes an entity. The verdict comes from the same
                            // search the post-bake access audit runs, on the surface the bake composes.
                            //
                            // OPT-IN `style.terrainPolicy.entryCorridor`: before the candidate is
                            // withdrawn, ONE graded walking corridor is offered to it - accepted only
                            // when this SAME gate then passes on the surface the corridor publishes.
                            if let Some((category, _)) = super::entry_corridor::gate_or_corridor(
                                &mut site,
                                &road_entities,
                                &result.site_entities(),
                                natural,
                                Some(&mask),
                                road_cut_mm,
                                water,
                                result.entry_corridor_policy,
                                result.max_roadbed_fill_m,
                            )? {
                                // The service outcome publishes refusal COUNTS, one per named
                                // category: the pad is withdrawn here, before any conversion, so
                                // plan, service record, joint layout and entity list all agree.
                                *outcome
                                    .rejected_candidates
                                    .entry(category.into())
                                    .or_default() += 1;
                                continue;
                            }
                            // The pad declares a purpose BEFORE it lands (explicit policy only). It
                            // is applied last, after every engineering gate above, so a refused
                            // water, shore-band, reserved-land or fill candidate keeps its own
                            // named reason and this gate never contests one.
                            let verdict = super::frontage::pad_purpose_for_site(
                                &site,
                                scene,
                                zones,
                                pad_conversion,
                            )?;
                            if let Some(reason) = verdict.reason {
                                *outcome
                                    .rejected_candidates
                                    .entry(reason.into())
                                    .or_default() += 1;
                                continue;
                            }
                            site.purpose = verdict.declared.map(str::to_owned);
                            accepted = Some(site);
                            break 'candidates;
                        }
                    }
                }
            }
            if let Some(site) = accepted {
                outcome.planned_site_ids.push(site.id.clone());
                result.roadside_sites.push(site);
            } else {
                break;
            }
        }
        outcome.unfilled_count = r.count - outcome.planned_site_ids.len() as u32;
        outcome.status = if outcome.unfilled_count == 0 {
            "planned"
        } else {
            "unfilled"
        }
        .into();
        result.service_outcomes.push(outcome);
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAudit {
    pub status: String,
    pub requested: u32,
    pub planned: usize,
    pub realized_with_access: usize,
    pub unfilled: u32,
    pub failures: Vec<String>,
}
/// Called after the source road/program plan is rebuilt independently. A
/// planned footprint alone never proves a facility has been realized.
pub fn audit(
    plan: &RoadNetworkPlan,
    entities: &[EntityInstance],
    access: &AccessGraph,
) -> ServiceAudit {
    // The shoreline commercial row publishes its outcome through this same list (it IS a pad
    // programme, `worldgen::shorefront`), but it is not an authored service request: counting its
    // demand here would report the settlement programme as owing 42 shopfronts it never asked for,
    // and its unfilled candidates as a service failure. Its realization is held by the road-network
    // audit instead, exactly like every other planned pad.
    let outcomes: Vec<&ServiceOutcome> = plan
        .service_outcomes
        .iter()
        .filter(|outcome| outcome.request.id != super::shorefront::SHOREFRONT_ROW)
        .collect();
    let requested = outcomes.iter().map(|o| o.request.count).sum();
    let mut audit = ServiceAudit {
        status: "pass".into(),
        requested,
        planned: 0,
        realized_with_access: 0,
        unfilled: outcomes.iter().map(|o| o.unfilled_count).sum(),
        failures: vec![],
    };
    for (s, mut expected) in plan.roadside_sites.iter().zip(plan.site_entities()) {
        if s.program.is_none()
            || s.support_level_mm.is_some()
            || s.program
                .as_ref()
                .is_some_and(|program| program.request_id == super::shorefront::SHOREFRONT_ROW)
        {
            continue;
        }
        audit.planned += 1;
        expected.normalize_spatial_semantics();
        expected.bounds = None;
        let found: Vec<_> = entities.iter().filter(|e| e.entity_id == s.id).collect();
        let exact = found.len() == 1 && {
            let mut e = found[0].clone();
            e.bounds = None;
            serde_json::to_value(e).ok() == serde_json::to_value(&expected).ok()
        };
        let connected = access.links.iter().any(|l| {
            l.entity_id == s.id
                && l.connected
                && l.required
                && l.road_id.as_deref() == Some(s.road_id.as_str())
        });
        if exact && connected {
            audit.realized_with_access += 1;
        } else {
            audit.failures.push(format!(
                "{} missing, altered or without assigned-road access",
                s.id
            ));
        }
    }
    if audit.unfilled > 0 || audit.realized_with_access != audit.requested as usize {
        audit.status = "incomplete".into();
    }
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    audit
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<super::super::LandUseZone>,
        RoadNetworkPlan,
    ) {
        let scene = SceneSpec {
            scene_id: "services".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: -256,
            origin_z: -256,
            seed_offset: 0,
        };
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(-256, -256, 512, 512, &vec![0; 512 * 512 * 2])
            .unwrap();
        let zones = vec![super::super::LandUseZone {
            id: "buildable".into(),
            kind: "rural".into(),
            world_x: -256,
            world_z: -256,
            width_m: 512,
            depth_m: 512,
            source: "fixture".into(),
        }];
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let roads=serde_json::from_value(serde_json::json!({"sceneId":"services","model":"test","naturalHeightfieldHash":"test","status":"planned_connected_network",
            "gridSpacingM":64,"maxRoadbedFillM":6.,"levelM":0,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,
            "nodes":[],"edges":[{"id":"road","from":0,"to":1,"worldX":0,"worldZ":0,"lengthM":64,"rotationYDeg":0.,"naturalMinM":0.,"naturalMaxM":0.}],
            "roadsideSites":[{"id":"home","roadId":"road","worldX":0,"worldZ":24,"widthM":24,"depthM":24,"supportLevelM":0,"rotationYDeg":0.,"naturalMinM":0.,"naturalMaxM":0.}]})).unwrap();
        (scene, natural, water, zones, roads)
    }
    fn request(kind: &str, w: u32, d: u32) -> ServiceProgram {
        ServiceProgram {
            version: 1,
            housing_minimum: BTreeMap::new(),
            requests: vec![ServiceRequest {
                id: "requested".into(),
                scene_id: "services".into(),
                kind: kind.into(),
                count: 1,
                width_m: w,
                depth_m: d,
                height_m: 8,
            }],
        }
    }
    #[test]
    fn explicit_variable_school_is_added_without_replacing_homes() {
        let (s, n, w, z, mut p) = fixture();
        let home = p.roadside_sites[0].clone();
        append(
            &mut p,
            &s,
            &n,
            w,
            &z,
            &request("school", 40, 30),
            super::super::frontage::RuralConversion::Disabled,
            0,
        )
        .unwrap();
        assert_eq!(p.roadside_sites.len(), 2);
        assert_eq!(
            serde_json::to_value(&p.roadside_sites[0]).unwrap(),
            serde_json::to_value(home).unwrap()
        );
        let site = &p.roadside_sites[1];
        assert_eq!((site.width_m, site.depth_m), (40, 30));
        let e = &p.site_entities()[1];
        assert_eq!(e.kind, "school");
        assert_eq!(e.height_m, 8.);
        assert_eq!((e.anchors[0].world_z - e.world_z).abs(), 16);
        let d = crate::settlement::development::plan(&s, &z, &p).unwrap();
        let b = d
            .buildings
            .iter()
            .find(|b| b.entity.kind == "school")
            .unwrap();
        assert_eq!(b.footprint.area(), 1200);
        assert_eq!(
            d.parcels
                .iter()
                .find(|v| v.building_id == b.id)
                .unwrap()
                .boundary
                .area(),
            44 * 34
        );
    }
    #[test]
    fn forbidden_or_infeasible_support_retains_exact_unfilled_request() {
        let (s, _, w, mut z, mut p) = fixture();
        z[0].kind = "nature".into();
        let mut n = BakedTerrain::new(&s).unwrap();
        n.insert_chunk(-256, -256, 512, 512, &vec![0; 512 * 512 * 2])
            .unwrap();
        append(
            &mut p,
            &s,
            &n,
            w,
            &z,
            &request("school", 80, 60),
            super::super::frontage::RuralConversion::Disabled,
            0,
        )
        .unwrap();
        assert_eq!(p.roadside_sites.len(), 1);
        assert_eq!(p.service_outcomes[0].unfilled_count, 1);
        assert_eq!(p.service_outcomes[0].request.width_m, 80);
        assert!(
            p.service_outcomes[0]
                .rejected_candidates
                .contains_key("reserved_land_intersection")
        );
        assert_eq!(
            audit(&p, &p.site_entities(), &AccessGraph::default()).status,
            "incomplete"
        );
    }
    #[test]
    fn exact_service_requires_actual_entity_and_assigned_access() {
        let (s, n, w, z, mut p) = fixture();
        append(
            &mut p,
            &s,
            &n,
            w,
            &z,
            &request("church", 20, 28),
            super::super::frontage::RuralConversion::Disabled,
            0,
        )
        .unwrap();
        let mut e = p.site_entities();
        for e in &mut e {
            e.normalize_spatial_semantics();
        }
        let mut access = AccessGraph::default();
        let id = p.service_outcomes[0].planned_site_ids[0].clone();
        access.links.push(serde_json::from_value(serde_json::json!({"entityId":id,"anchorId":"front","roadId":"road","distanceM":7.,"connected":true,"required":true,"status":"walkable_route","path":[]})).unwrap());
        assert_eq!(audit(&p, &e, &access).status, "pass");
        e.last_mut().unwrap().width_m -= 2.;
        assert_eq!(audit(&p, &e, &access).status, "fail");
        assert_eq!(audit(&p, &[], &access).status, "fail");
    }
    #[test]
    fn malformed_demands_are_rejected_instead_of_clamped_or_silently_ignored() {
        let original = serde_json::to_value(request("school", 40, 30)).unwrap();
        for case in 0..5 {
            let mut v = original.clone();
            match case {
                0 => v["requests"][0]["widthM"] = serde_json::json!(39),
                1 => v["requests"][0]["count"] = serde_json::json!(0),
                2 => v["requests"][0]["kind"] = serde_json::json!("imaginary"),
                3 => v["requests"][0]["ignoreMe"] = serde_json::json!(true),
                _ => v["requests"]
                    .as_array_mut()
                    .unwrap()
                    .push(original["requests"][0].clone()),
            }
            assert!(from_style(&serde_json::json!({"settlementProgram":v})).is_err());
        }
        assert!(
            from_style(&serde_json::json!({"settlementProgram":original}))
                .unwrap()
                .is_some()
        );
        assert!(from_style(&serde_json::Value::Null).unwrap().is_none());
    }
}
