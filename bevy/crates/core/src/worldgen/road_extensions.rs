//! Explicit connected extensions of an existing verified road program.
//! No source road/site is removed. Rejected requests are errors, not omissions.
use super::{
    EntityInstance, LandUseZone, SceneSpec, WaterGeometry, WorldManifest,
    road_alignment::{self, CorridorDesign, CorridorRequest},
    road_network::{RoadEdge, RoadNetworkPlan, RoadNode, RoadTierRef},
    road_profile::RoadProfile,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionProgram {
    pub version: u32,
    pub corridors: Vec<CorridorRequest>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoadExtension {
    pub entity_id: String,
    pub parent_road_ids: Vec<String>,
    pub design: CorridorDesign,
    /// Authored hierarchy provenance for an automatically selected road.
    /// Absent for the explicit `roadExtensions` contract and for every legacy
    /// road that did not come from a hierarchy-aware programme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<RoadTierRef>,
}
fn err(s: impl Into<String>) -> TerrainError {
    TerrainError(s.into())
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<ExtensionProgram>, TerrainError> {
    let Some(value) = style.get("roadExtensions") else {
        return Ok(None);
    };
    let program: ExtensionProgram =
        serde_json::from_value(value.clone()).map_err(|e| err(e.to_string()))?;
    if program.version != 1 || program.corridors.is_empty() {
        return Err(err("invalid road extension program"));
    }
    Ok(Some(program))
}
pub fn validate_manifest(m: &WorldManifest) -> Result<(), TerrainError> {
    if let Some(p) = from_style(&m.style)? {
        if super::height_encoding_from_style(&m.style).map_err(|e| err(e.to_string()))?
            != crate::height_payload::HeightEncoding::Millimetre
        {
            return Err(err("graded road extensions require millimetre heights"));
        }
        for r in p.corridors {
            let scene = m
                .scenes
                .iter()
                .find(|s| s.scene_id == r.scene_id)
                .ok_or_else(|| err("road extension refers to unknown scene"))?;
            let w =
                super::water_geometry(super::water_kind(&m.style), &m.landmarks, scene, &m.style);
            if !super::road_network::enabled(&m.style, w) {
                return Err(err("road extensions require active road-first planning"));
            }
        }
    }
    Ok(())
}
pub fn profile(d: &CorridorDesign) -> RoadProfile {
    RoadProfile {
        version: 1,
        road_width_m: d.request.road_width_m,
        start: d.request.start,
        end: d.request.end,
        elevations_mm: d.profile_mm.clone(),
        max_fill_mm: d.request.max_fill_mm,
    }
}
// ==================== B01-T2 R3 V2: the junction contract ====================
//
// The base road's tie-in platform must cover every junction a LEGAL extension
// can occupy. Exporting the contract here means
// `road_network::base_tie_in_platform_m` DERIVES its value instead of
// hard-coding one, so admitting a wider road to the extension contract later
// cannot silently invalidate the base platform.

/// The road widths the extension contract admits (`append_one` validation).
pub const EXTENSION_ROAD_WIDTHS_M: [u32; 3] = [8, 12, 16];
/// The corridor contract's fixed shoulder width.
pub const EXTENSION_SHOULDER_M: u32 = 3;

/// The graded flat-top half-width held exactly, mirroring
/// `graded_flat_top_half_extents`: `max(7, width/2 + 1)`.
pub const fn extension_graded_half_m(width: u32) -> i32 {
    let band = (width / 2 + 1) as i32;
    if band > 7 { band } else { 7 }
}

/// The graded transport half-width, mirroring `graded_transport_half_extents`:
/// pavement + the fixed 3 m shoulder (`width/2 + 3`).
pub const fn extension_transport_half_m(width: u32) -> i32 {
    (width / 2) as i32 + EXTENSION_SHOULDER_M as i32
}

/// The junction occupancy radius a corridor of this width needs: the larger of
/// its graded band and its transport footprint.
pub const fn junction_occupancy_m(width: u32) -> i32 {
    let graded = extension_graded_half_m(width);
    let transport = extension_transport_half_m(width);
    if graded > transport { graded } else { transport }
}

/// The largest junction occupancy any width the extension contract admits can
/// require. 8/12/16 give occupancies 7/9/11, so this is 11.
pub const fn max_junction_occupancy_m() -> i32 {
    let mut worst = 0;
    let mut i = 0;
    while i < EXTENSION_ROAD_WIDTHS_M.len() {
        let occupancy = junction_occupancy_m(EXTENSION_ROAD_WIDTHS_M[i]);
        if occupancy > worst { worst = occupancy; }
        i += 1;
    }
    worst
}

fn at(e: &EntityInstance, x: i32, z: i32) -> f64 {
    e.grounding
        .road_profile
        .as_ref()
        .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
        .unwrap_or(f64::from(e.ground_height_m()))
}
fn rect(e: &EntityInstance) -> [i64; 4] {
    let (hw, hd) = if e.rotation_y_deg == 0. {
        (e.width_m as i64 / 2 + 4, e.depth_m as i64 / 2 + 3)
    } else {
        (e.depth_m as i64 / 2 + 3, e.width_m as i64 / 2 + 4)
    };
    [
        i64::from(e.world_x) - hw,
        i64::from(e.world_z) - hd,
        i64::from(e.world_x) + hw,
        i64::from(e.world_z) + hd,
    ]
}
/// A candidate that leaves one endpoint in the opposite direction from an
/// existing road leaving that same endpoint is a continuation, not a crossing.
/// The padded support rectangles overlap end-to-end even though the pavements
/// meet only at the shared node.
pub(super) fn collinear_continuation(
    candidate_start: [i32; 2],
    candidate_end: [i32; 2],
    old: &EntityInstance,
    shared: [i32; 2],
) -> bool {
    let Some(profile) = &old.grounding.road_profile else {
        return false;
    };
    let candidate = if candidate_start == shared {
        [
            candidate_end[0] - candidate_start[0],
            candidate_end[1] - candidate_start[1],
        ]
    } else if candidate_end == shared {
        [
            candidate_start[0] - candidate_end[0],
            candidate_start[1] - candidate_end[1],
        ]
    } else {
        return false;
    };
    let existing = if profile.start == shared {
        [
            profile.end[0] - profile.start[0],
            profile.end[1] - profile.start[1],
        ]
    } else if profile.end == shared {
        [
            profile.start[0] - profile.end[0],
            profile.start[1] - profile.end[1],
        ]
    } else {
        return false;
    };
    candidate[0].signum() * existing[1].signum() - candidate[1].signum() * existing[0].signum() == 0
        && candidate[0].signum() * existing[0].signum()
            + candidate[1].signum() * existing[1].signum()
            < 0
}
pub fn append(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    t: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    style: &serde_json::Value,
) -> Result<(), TerrainError> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    if !program
        .corridors
        .iter()
        .any(|r| r.scene_id == scene.scene_id)
    {
        return Ok(());
    }
    let context = road_alignment::AlignmentContext::new(scene, t, water, zones)?;
    for (ordinal, r) in program
        .corridors
        .iter()
        .enumerate()
        .filter(|(_, r)| r.scene_id == scene.scene_id)
    {
        append_one(
            plan,
            &context,
            r,
            format!("generated.road-extension.{}.{}", scene.scene_id, ordinal),
        )?;
    }
    Ok(())
}
/// A geometric contact is not a topology edge. Only endpoints that are nodes
/// of BOTH roads can host a v1 junction; interior T-junctions require splitting.
pub(super) fn shared_junctions(
    plan: &RoadNetworkPlan,
    old_id: &str,
    start: [i32; 2],
    end: [i32; 2],
) -> Vec<[i32; 2]> {
    let Some(edge) = plan.edges.iter().find(|e| e.id == old_id) else {
        return vec![];
    };
    [start, end]
        .into_iter()
        .filter(|p| {
            plan.nodes
                .iter()
                .any(|n| [n.world_x, n.world_z] == *p && (edge.from == n.id || edge.to == n.id))
        })
        .collect()
}
pub(super) fn append_one(
    plan: &mut RoadNetworkPlan,
    context: &road_alignment::AlignmentContext<'_>,
    r: &CorridorRequest,
    id: String,
) -> Result<(), TerrainError> {
    append_one_with_tier(plan, context, r, id, None)
}

pub(super) fn append_one_with_tier(
    plan: &mut RoadNetworkPlan,
    context: &road_alignment::AlignmentContext<'_>,
    r: &CorridorRequest,
    id: String,
    tier: Option<RoadTierRef>,
) -> Result<(), TerrainError> {
    let t = context.terrain;
    let mask = &context.mask;
    if plan.edges.iter().any(|e| e.id == id) {
        return Err(err("duplicate road extension id"));
    }
    let start_level = r
        .start_level_mm
        .ok_or_else(|| err("extension needs fixed start junction"))?;
    let end_level = r
        .end_level_mm
        .ok_or_else(|| err("extension needs fixed end junction"))?;
    let length = (i64::from(r.end[0]) - i64::from(r.start[0])).abs()
        + (i64::from(r.end[1]) - i64::from(r.start[1])).abs();
    if !matches!(r.road_width_m, 8 | 12 | 16)
        || r.shoulder_width_m != 3
        || r.junction_length_m < 8
        || length < 16
        || length % 2 != 0
    {
        return Err(err(
            "extension requires explicit 8/12/16m road, 3m shoulders, even length and >=8m junction platforms",
        ));
    }
    let from = plan
        .nodes
        .iter()
        .find(|n| [n.world_x, n.world_z] == r.start)
        .ok_or_else(|| err("extension start is not an existing road node"))?
        .id;
    let previous = plan.entities();
    let parents: Vec<_> = plan
        .edges
        .iter()
        .filter(|e| e.from == from || e.to == from)
        .map(|e| e.id.clone())
        .collect();
    if parents.is_empty() {
        return Err(err("extension has no existing road attachment"));
    }
    for id in &parents {
        let e = previous
            .iter()
            .find(|e| &e.entity_id == id)
            .ok_or_else(|| err("missing extension parent entity"))?;
        if (at(e, r.start[0], r.start[1]) * 1000. - f64::from(start_level)).abs() > 0.01 {
            return Err(err("extension start junction height mismatch"));
        }
    }
    let design = context.design(r)?;
    context.verify(&design, r)?;
    let profile = profile(&design);
    let x = ((i64::from(r.start[0]) + i64::from(r.end[0])) / 2) as i32;
    let z = ((i64::from(r.start[1]) + i64::from(r.end[1])) / 2) as i32;
    let (hw, hd) = if r.start[1] == r.end[1] {
        (length as i32 / 2 + 4, r.road_width_m as i32 / 2 + 3)
    } else {
        (r.road_width_m as i32 / 2 + 3, length as i32 / 2 + 4)
    };
    if mask
        .check("extension".into(), "road", x, z, hw, hd)
        .is_some()
    {
        return Err(err("extension end support crosses protected zoning"));
    }
    // The fill envelope this corridor must hold, per cell: at most the roadbed
    // fill contract above the natural ground, and at most the design's OWN
    // bounded-cut allowance below it.   - every world that never opted into
    // the policy - keeps the historical `fill >= 0` rule exactly.
    let cut_allowance_m = f64::from(context.cut_mm()) / 1000.;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for zz in z - hd..=z + hd {
        for xx in x - hw..=x + hw {
            let h = t
                .height_millimetres(xx, zz)
                .ok_or_else(|| err("extension end support outside natural raster"))?;
            let target = profile
                .height_at(f64::from(xx), f64::from(zz))
                .ok_or_else(|| err("missing extension height"))?;
            let fill = target - f64::from(h) / 1000.;
            if fill < -cut_allowance_m - 1e-9
                || fill > f64::from(r.max_fill_mm) / 1000. + 1e-9
                || fill > mask.fill_capacity_at(xx, zz) + 1e-9
            {
                return Err(err(
                    "extension padded footprint violates natural fill envelope",
                ));
            }
            min = min.min(h as f32 / 1000.);
            max = max.max(h as f32 / 1000.);
        }
    }
    let bounds = [
        i64::from(x - hw),
        i64::from(z - hd),
        i64::from(x + hw),
        i64::from(z + hd),
    ];
    let junction_radius = i64::from(r.junction_length_m.max(r.road_width_m / 2 + 3));
    // No new mid-edge or grade-separated intersections are invented. Every
    // overlapping support point must belong to a declared endpoint junction.
    for old in &previous {
        let b = rect(old);
        let q = [
            bounds[0].max(b[0]),
            bounds[1].max(b[1]),
            bounds[2].min(b[2]),
            bounds[3].min(b[3]),
        ];
        if q[0] > q[2] || q[1] > q[3] {
            continue;
        }
        let junctions = shared_junctions(plan, &old.entity_id, r.start, r.end);
        // No new mid-edge or grade-separated intersections are invented: any
        // overlap with an existing road must be confined to a declared endpoint
        // junction, so a road the new corridor does not actually connect to
        // cannot be overlapped at all.
        if junctions.is_empty() {
            return Err(err(format!(
                "extension overlaps road outside a matching endpoint junction:{}",
                old.entity_id
            )));
        }
        let continuation = junctions
            .iter()
            .any(|point| collinear_continuation(r.start, r.end, old, *point));
        // B01-T2 R3 V2-3: a BASE road whose own tie-in platform is shorter than
        // the junction occupancy this extension needs cannot host it safely.
        // Refuse it by name instead of silently accepting a stepped junction.
        // Extension-to-extension junctions are governed by the per-cell rule
        // below (their plateau is the caller's own `junction_length_m`).
        if !plan
            .extensions
            .iter()
            .any(|existing| existing.entity_id == old.entity_id)
        {
            if let Some(old_profile) = &old.grounding.road_profile {
                // NOTE (R4 R-5-2): for a road EntityInstance, `width_m` is the
                // road's ALONG-LENGTH (`entities()` sets `width_m = e.length_m`)
                // and `depth_m` is the pavement width. So this read is "how long
                // is the existing road", which is what the platform rule needs -
                // not the road's width.
                let available = super::road_network::base_tie_in_platform_m(old.width_m as u32);
                let needed = junction_occupancy_m(r.road_width_m) as u32;
                if needed > available {
                    return Err(err(format!(
                        "base_platform_short_for_extension_width: {} offers a {}m tie-in \
                         platform but a {}m extension needs {}m (junction at {},{}); profile \
                         start/end = {},{}",
                        old.entity_id,
                        available,
                        r.road_width_m,
                        needed,
                        junctions[0][0],
                        junctions[0][1],
                        old_profile.start[0],
                        old_profile.start[1],
                    )));
                }
            }
        }
        for zz in q[1]..=q[3] {
            for xx in q[0]..=q[2] {
                if continuation {
                    continue;
                }
                if !junctions
                    .iter()
                    .any(|p| {
                        (xx - i64::from(p[0])).abs() <= junction_radius
                            && (zz - i64::from(p[1])).abs() <= junction_radius
                    })
                    // B01-T2 R3 V1: per-cell continuity. Inside the junction the
                    // two surfaces must agree at every overlapping cell;
                    // accepting a step inside a junction would weaken the
                    // pavement-continuity contract. The contract-derived base
                    // tie-in platform (V2) is what makes this rule satisfiable
                    // across the whole legal extension width range, rather than a
                    // relaxation of it.
                    || (profile.height_at(xx as f64, zz as f64).unwrap()
                        - at(old, xx as i32, zz as i32))
                    .abs()
                        > 0.00001
                {
                    return Err(err(
                        format!(
                            "extension overlaps road outside a matching endpoint junction:{}",
                            old.entity_id
                        ),
                    ));
                }
            }
        }
        // The junction point(s) themselves must also agree exactly; this is
        // equivalent to (or stricter than) the per-cell rule above and keeps the
        // physical meaning of a junction explicit.
        for p in &junctions {
            let (jx, jz) = (i64::from(p[0]), i64::from(p[1]));
            let junction_height = profile
                .height_at(jx as f64, jz as f64)
                .ok_or_else(|| err("extension junction is outside its own profile"))?;
            if (junction_height - at(old, p[0], p[1])).abs() > 0.00001 {
                return Err(err(
                    "extension junction height disagrees with the existing road",
                ));
            }
        }
    }
    for site in &plan.roadside_sites {
        let (sw, sd) = site.world_half_extents();
        if bounds[0] < i64::from(site.world_x) + i64::from(sw) + 4
            && bounds[2] > i64::from(site.world_x) - i64::from(sw) - 4
            && bounds[1] < i64::from(site.world_z) + i64::from(sd) + 4
            && bounds[3] > i64::from(site.world_z) - i64::from(sd) - 4
        {
            return Err(err("extension intersects an existing obligated site/yard"));
        }
    }
    let to = if let Some(node) = plan.nodes.iter().find(|n| [n.world_x, n.world_z] == r.end) {
        for edge in plan
            .edges
            .iter()
            .filter(|e| e.from == node.id || e.to == node.id)
        {
            let old = previous.iter().find(|e| e.entity_id == edge.id).unwrap();
            if (at(old, r.end[0], r.end[1]) * 1000. - f64::from(end_level)).abs() > 0.01 {
                return Err(err("extension end junction height mismatch"));
            }
        }
        node.id
    } else {
        let id = plan.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        plan.nodes.push(RoadNode {
            id,
            world_x: r.end[0],
            world_z: r.end[1],
        });
        id
    };

    plan.edges.push(RoadEdge {
        id: id.clone(),
        from,
        to,
        world_x: x,
        world_z: z,
        length_m: length as u32,
        rotation_y_deg: if r.start[1] == r.end[1] { 0. } else { 90. },
        natural_min_m: min,
        natural_max_m: max,
        // B01-R1 T2 (OPT-B): an extension road keeps `RoadExtension.design` as the SINGLE source of its
        // profile, so the edge's own field is deliberately None. `entities()` still finds the profile
        // through the legacy extensions lookup, which is priority 2. Writing a copy here would create two
        // sources of truth for the same road.
        profile: None,
        tier: tier.clone(),
        // OPT-IN earthworkBlend: the fill-derived ring is written by
        // earthwork_blend::stamp_plan once the whole pad list is final, so an
        // extension road starts without one - exactly like every base edge.
        earthwork_blend_m: None,
    });
    plan.extensions.push(RoadExtension {
        entity_id: id,
        parent_road_ids: parents,
        design,
        tier,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;
    fn fixture() -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
        serde_json::Value,
    ) {
        let scene = SceneSpec {
            scene_id: "ext".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut t = BakedTerrain::new(&scene).unwrap();
        t.insert_chunk(
            0,
            0,
            512,
            512,
            &encode_millimetres(
                512,
                512,
                (0..512).flat_map(|_| {
                    (0..512).map(|x| {
                        if x <= 100 {
                            0.
                        } else {
                            (x - 100) as f64 * 0.02
                        }
                    })
                }),
            )
            .unwrap(),
        )
        .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let p:RoadNetworkPlan=serde_json::from_value(serde_json::json!({"model":"fixture","naturalHeightfieldHash":t.content_hash().unwrap(),"sceneId":"ext","status":"planned_connected_network","gridSpacingM":64,"maxRoadbedFillM":6,"levelM":1,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,"nodes":[{"id":0,"worldX":32,"worldZ":128},{"id":1,"worldX":96,"worldZ":128}],"edges":[{"id":"base","from":0,"to":1,"worldX":64,"worldZ":128,"lengthM":64,"rotationYDeg":0,"naturalMinM":0,"naturalMaxM":0}],"roadsideSites":[]})).unwrap();
        let style = serde_json::json!({"roadExtensions":{"version":1,"corridors":[{"sceneId":"ext","start":[96,128],"end":[224,128],"roadWidthM":8,"shoulderWidthM":3,"maxFillMm":6000,"maxRiseMmPerM":80,"startLevelMm":1000,"endLevelMm":3000,"junctionLengthM":8}]}});
        (scene, t, water, zones, p, style)
    }
    #[test]
    fn extensions_keep_source_edges_and_bind_shared_junctions() {
        let (s, t, w, z, mut p, style) = fixture();
        let old = serde_json::to_value(&p.edges).unwrap();
        append(&mut p, &s, &t, w, &z, &style).unwrap();
        assert_eq!(p.extensions.len(), 1);
        assert_eq!(p.edges.len(), 2);
        assert_eq!(serde_json::to_value(&p.edges[..1]).unwrap(), old);
        assert_eq!(p.edges[1].from, 1);
        let e = p.entities().pop().unwrap();
        let profile = e.grounding.road_profile.as_ref().unwrap();
        profile.validate_entity(&e).unwrap();
        assert_eq!(profile.height_at(96., 128.), Some(1.));
        assert_eq!(profile.height_at(224., 128.), Some(3.));
        let mut again = fixture().4;
        append(&mut again, &s, &t, w, &z, &style).unwrap();
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::to_value(again).unwrap()
        );
    }
    #[test]
    fn explicit_8_12_16_metre_widths_reach_entity_and_profile() {
        for width in [8, 12, 16] {
            let (s, t, w, z, mut p, mut style) = fixture();
            style["roadExtensions"]["corridors"][0]["roadWidthM"] = serde_json::json!(width);
            append(&mut p, &s, &t, w, &z, &style).unwrap();
            let entity = p.entities().pop().unwrap();
            assert_eq!(entity.depth_m, width as f32);
            let profile = entity.grounding.road_profile.as_ref().unwrap();
            assert_eq!(profile.road_width_m, width);
            profile.validate_entity(&entity).unwrap();
        }
        let (s, t, w, z, mut p, mut style) = fixture();
        style["roadExtensions"]["corridors"][0]["roadWidthM"] = serde_json::json!(10);
        assert!(append(&mut p, &s, &t, w, &z, &style).is_err());
    }
    #[test]
    fn detached_height_mismatched_and_conflicting_extensions_are_not_omitted() {
        let (s, t, w, z, p, style) = fixture();
        for mode in 0..4 {
            let mut bad = style.clone();
            match mode {
                0 => bad["roadExtensions"]["corridors"][0]["start"] = serde_json::json!([100, 128]),
                1 => {
                    bad["roadExtensions"]["corridors"][0]["startLevelMm"] = serde_json::json!(2000)
                }
                2 => bad["roadExtensions"]["corridors"][0]["endLevelMm"] = serde_json::Value::Null,
                _ => bad["roadExtensions"]["corridors"][0]["end"] = serde_json::json!([32, 128]),
            };
            assert!(append(&mut p.clone(), &s, &t, w, &z, &bad).is_err());
        }
    }
    #[test]
    fn applied_profile_matches_support_and_wins_over_unrelated_blends() {
        let (s, t, w, z, mut p, style) = fixture();
        append(&mut p, &s, &t, w, &z, &style).unwrap();
        let entities = p.entities();
        let carves = super::super::plan_terrain_carves(&entities);
        let e = &entities[1];
        let profile = e.grounding.road_profile.as_ref().unwrap();
        for x in 92..=228 {
            for zz in 121..=135 {
                let natural = f64::from(t.height_millimetres(x, zz).unwrap()) / 1000.;
                let actual = super::super::apply_terrain_carves(natural, x, zz, &carves);
                assert!((actual - profile.height_at(x as f64, zz as f64).unwrap()).abs() < 1e-9);
            }
        }
        let mut mixed = carves.clone();
        mixed.push(super::super::TerrainCarve {
            profile: None,
            cx: 160.,
            cz: 180.,
            half_w: 12.,
            half_d: 12.,
            rotation_y_deg: 0.,
            target_h_m: 10.,
            blend_m: 100.,
            priority: 1,
            disturbance: 160,
            cut_cap_mm: None,

            ramp: None,
        });
        let actual = super::super::apply_terrain_carves(1.2, 160, 128, &mixed);
        assert_eq!(actual, profile.height_at(160., 128.).unwrap());
        mixed.reverse();
        assert_eq!(
            actual,
            super::super::apply_terrain_carves(1.2, 160, 128, &mixed)
        );
        assert!(super::super::apply_terrain_carves(1.2, 160, 148, &carves) <= 7.2);
    }
    #[test]
    fn graded_road_ownership_uses_real_length_not_legacy_grid_spacing() {
        let (s, t, w, z, mut p, style) = fixture();
        append(&mut p, &s, &t, w, &z, &style).unwrap();
        let development = crate::settlement::development::plan(&s, &z, &p).unwrap();
        let road_area: u64 = development
            .cells
            .iter()
            .filter(|c| c.role == crate::settlement::development::LandRole::Road)
            .map(|c| u64::from(c.boundary.width_m) * u64::from(c.boundary.depth_m))
            .sum();
        assert_eq!(road_area, 64 * 8 + 128 * 8);
        let mut bad = p.clone();
        bad.edges[1].length_m = 126;
        assert!(crate::settlement::development::plan(&s, &z, &bad).is_err());
    }
    #[test]
    fn endpoint_contact_with_long_road_interior_requires_an_explicit_graph_junction() {
        let (s, _, w, z, mut plan, _) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(
            0,
            0,
            512,
            512,
            &encode_millimetres(512, 512, std::iter::repeat_n(0., 512 * 512)).unwrap(),
        )
        .unwrap();
        plan.natural_heightfield_hash = t.content_hash().unwrap();
        let ctx = road_alignment::AlignmentContext::new(&s, &t, w, &z).unwrap();
        let request = |start, end, a, b| CorridorRequest {
            scene_id: s.scene_id.clone(),
            start,
            end,
            road_width_m: 8,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(a),
            end_level_mm: Some(b),
            junction_length_m: 8,
        };
        for (i, r) in [
            request([96, 128], [352, 128], 1000, 1000),
            request([96, 128], [96, 256], 1000, 1000),
            request([96, 256], [224, 256], 1000, 0),
        ]
        .iter()
        .enumerate()
        {
            append_one(&mut plan, &ctx, r, format!("test-{i}")).unwrap();
        }
        let before = serde_json::to_value(&plan).unwrap();
        // This would geometrically touch the flat middle of test-0 at the
        // correct height, but test-0 has no graph node at (224,128).
        let result = append_one(
            &mut plan,
            &ctx,
            &request([224, 256], [224, 128], 0, 0),
            "unmodeled-t-junction".into(),
        );
        assert!(
            result.is_err(),
            "an unsplit old graph edge must not become an implicit junction"
        );
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            before,
            "rejected candidates must not partially mutate graph state"
        );
    }
    #[test]
    fn collinear_end_to_end_continuation_is_not_a_crossing() {
        let (s, _, w, z, mut plan, _) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(
            0,
            0,
            512,
            512,
            &encode_millimetres(512, 512, std::iter::repeat_n(0., 512 * 512)).unwrap(),
        )
        .unwrap();
        plan.natural_heightfield_hash = t.content_hash().unwrap();
        let ctx = road_alignment::AlignmentContext::new(&s, &t, w, &z).unwrap();
        let request = CorridorRequest {
            scene_id: s.scene_id.clone(),
            start: [96, 128],
            end: [160, 128],
            road_width_m: 16,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(1000),
            end_level_mm: Some(2000),
            junction_length_m: 8,
        };
        append_one(&mut plan, &ctx, &request, "continuation".into()).unwrap();
        assert_eq!(plan.edges.len(), 2);
    }
    #[test]
    fn perpendicular_endpoint_overlap_still_requires_an_explicit_split() {
        let (s, _, w, z, mut plan, _) = fixture();
        let mut t = BakedTerrain::new(&s).unwrap();
        t.insert_chunk(
            0,
            0,
            512,
            512,
            &encode_millimetres(512, 512, std::iter::repeat_n(0., 512 * 512)).unwrap(),
        )
        .unwrap();
        plan.natural_heightfield_hash = t.content_hash().unwrap();
        let ctx = road_alignment::AlignmentContext::new(&s, &t, w, &z).unwrap();
        let request = CorridorRequest {
            scene_id: s.scene_id.clone(),
            start: [96, 128],
            end: [96, 192],
            road_width_m: 16,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(0),
            end_level_mm: Some(1000),
            junction_length_m: 8,
        };
        assert!(append_one(&mut plan, &ctx, &request, "corner".into()).is_err());
    }
}
