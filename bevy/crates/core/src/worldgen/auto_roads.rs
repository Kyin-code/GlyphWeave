//! Bounded deterministic road growth from immutable terrain and planned demand.
//! Reaching a zone centre is accessibility evidence, NEVER realized land area.
use super::{
    EntityInstance, LandUseZone, SceneSpec, WaterGeometry, WorldManifest,
    road_alignment::{AlignmentContext, CorridorRequest},
    road_extensions,
    road_network::RoadNetworkPlan,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneProgram {
    pub scene_id: String,
    pub max_new_segments: usize,
    pub max_candidate_attempts: usize,
    pub segment_lengths_m: Vec<u32>,
    pub priority_purposes: Vec<String>,
    pub reach_distance_m: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub scenes: Vec<SceneProgram>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub zone_bounds: [i64; 4],
    pub before_has_road: bool,
    pub after_has_road: bool,
    pub zone_id: String,
    pub purpose: String,
    pub centre: [i32; 2],
    pub before_distance_m: Option<u64>,
    pub after_distance_m: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RejectedExample {
    pub start: [i32; 2],
    pub end: [i32; 2],
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub version: u32,
    pub model: String,
    pub scope: String,
    pub program: SceneProgram,
    pub stop_reason: String,
    pub attempted_candidates: usize,
    pub accepted_segments: usize,
    pub generated_road_ids: Vec<String>,
    pub targets: Vec<Target>,
    pub unserved_targets: usize,
    pub rejected_by_reason: BTreeMap<String, usize>,
    pub rejection_examples: Vec<RejectedExample>,
}
fn err(s: impl Into<String>) -> TerrainError {
    TerrainError(s.into())
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<Program>, TerrainError> {
    let Some(value) = style.get("automaticRoadExtensions") else {
        return Ok(None);
    };
    let p: Program = serde_json::from_value(value.clone()).map_err(|e| err(e.to_string()))?;
    if p.version != 1 || p.scenes.is_empty() {
        return Err(err("invalid automatic road program"));
    }
    let mut ids = BTreeSet::new();
    for s in &p.scenes {
        if !ids.insert(&s.scene_id)
            || s.scene_id.is_empty()
            || !(1..=256).contains(&s.max_new_segments)
            || !(1..=65536).contains(&s.max_candidate_attempts)
            || s.segment_lengths_m.is_empty()
            || s.segment_lengths_m.len() > 8
            || s.segment_lengths_m
                .iter()
                .any(|v| *v < 16 || *v > 1024 || v % 2 != 0)
            || s.segment_lengths_m.iter().collect::<BTreeSet<_>>().len()
                != s.segment_lengths_m.len()
            || s.priority_purposes.is_empty()
            || s.priority_purposes
                .iter()
                .any(|s| !matches!(s.as_str(), "urban" | "rural"))
            || s.priority_purposes.iter().collect::<BTreeSet<_>>().len()
                != s.priority_purposes.len()
            || !(8..=160).contains(&s.reach_distance_m)
        {
            return Err(err("invalid automatic road scene limits/purposes"));
        }
    }
    Ok(Some(p))
}
pub fn validate_manifest(m: &WorldManifest) -> Result<(), TerrainError> {
    if let Some(p) = from_style(&m.style)? {
        if super::height_encoding_from_style(&m.style).map_err(|e| err(e.to_string()))?
            != crate::height_payload::HeightEncoding::Millimetre
        {
            return Err(err("automatic roads require millimetre heights"));
        }
        for program in p.scenes {
            let scene = m
                .scenes
                .iter()
                .find(|s| s.scene_id == program.scene_id)
                .ok_or_else(|| err("automatic roads refer to unknown scene"))?;
            let w =
                super::water_geometry(super::water_kind(&m.style), &m.landmarks, scene, &m.style);
            if !super::road_network::enabled(&m.style, w) {
                return Err(err("automatic roads require active road-first planning"));
            }
        }
    }
    Ok(())
}
fn road_bounds(e: &EntityInstance, margin: i64) -> [i64; 4] {
    let (w, d) = if e.rotation_y_deg == 0. {
        (e.width_m as i64 / 2 + margin, 4 + margin)
    } else {
        (4 + margin, e.width_m as i64 / 2 + margin)
    };
    [
        i64::from(e.world_x) - w,
        i64::from(e.world_z) - d,
        i64::from(e.world_x) + w,
        i64::from(e.world_z) + d,
    ]
}
fn distance(p: [i32; 2], r: [i64; 4]) -> u64 {
    let x = i64::from(p[0]);
    let z = i64::from(p[1]);
    (r[0] - x).max(x - r[2]).max(0) as u64 + (r[1] - z).max(z - r[3]).max(0) as u64
}
fn intersects(a: [i64; 4], b: [i64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
fn has_road(bounds: [i64; 4], roads: &[EntityInstance]) -> bool {
    roads.iter().any(|e| intersects(bounds, road_bounds(e, 0)))
}
fn nearest(p: [i32; 2], roads: &[EntityInstance]) -> Option<u64> {
    roads.iter().map(|e| distance(p, road_bounds(e, 0))).min()
}
fn levels(
    plan: &RoadNetworkPlan,
    roads: &[EntityInstance],
) -> Result<BTreeMap<[i32; 2], i32>, TerrainError> {
    let by_id: BTreeMap<_, _> = roads.iter().map(|e| (e.entity_id.as_str(), e)).collect();
    let mut result = BTreeMap::new();
    for n in &plan.nodes {
        let mut height = None;
        for e in plan.edges.iter().filter(|e| e.from == n.id || e.to == n.id) {
            let road = by_id
                .get(e.id.as_str())
                .ok_or_else(|| err("automatic growth parent road missing"))?;
            let h = road
                .grounding
                .road_profile
                .as_ref()
                .and_then(|p| p.height_at(f64::from(n.world_x), f64::from(n.world_z)))
                .unwrap_or(f64::from(road.ground_height_m()));
            let mm = (h * 1000.).round() as i32;
            if height.is_some_and(|old| old != mm) {
                return Err(err("automatic growth parent node has incompatible heights"));
            }
            height = Some(mm);
        }
        if let Some(h) = height {
            result.insert([n.world_x, n.world_z], h);
        }
    }
    Ok(result)
}
#[derive(Clone)]
struct Candidate {
    start: [i32; 2],
    end: [i32; 2],
    node: usize,
    priority: usize,
    served: usize,
    gain: u64,
    length: u32,
}
impl Candidate {
    fn key(&self) -> [i32; 4] {
        [self.start[0], self.start[1], self.end[0], self.end[1]]
    }
    fn bounds(&self, margin: i64) -> [i64; 4] {
        let (mx, mz) = if self.start[1] == self.end[1] {
            (margin, 4 + margin)
        } else {
            (4 + margin, margin)
        };
        [
            i64::from(self.start[0].min(self.end[0])) - mx,
            i64::from(self.start[1].min(self.end[1])) - mz,
            i64::from(self.start[0].max(self.end[0])) + mx,
            i64::from(self.start[1].max(self.end[1])) + mz,
        ]
    }
}
fn clear(
    c: &Candidate,
    plan: &RoadNetworkPlan,
    roads: &[EntityInstance],
    ctx: &AlignmentContext<'_>,
) -> Result<(), TerrainError> {
    let x = ((i64::from(c.start[0]) + i64::from(c.end[0])) / 2) as i32;
    let z = ((i64::from(c.start[1]) + i64::from(c.end[1])) / 2) as i32;
    let (hw, hd) = if c.start[1] == c.end[1] {
        (c.length as i32 / 2 + 4, 7)
    } else {
        (7, c.length as i32 / 2 + 4)
    };
    if ctx
        .mask
        .check("auto-candidate".into(), "road", x, z, hw, hd)
        .is_some()
    {
        return Err(err("reserved_or_outside_zoning"));
    }
    let b = [
        i64::from(x - hw),
        i64::from(z - hd),
        i64::from(x + hw),
        i64::from(z + hd),
    ];
    for road in roads {
        // Same padded road support used by r46; longitudinal 4m, lateral 7m.
        let (w, d) = if road.rotation_y_deg == 0. {
            (road.width_m as i64 / 2 + 4, 7)
        } else {
            (7, road.width_m as i64 / 2 + 4)
        };
        let r = [
            i64::from(road.world_x) - w,
            i64::from(road.world_z) - d,
            i64::from(road.world_x) + w,
            i64::from(road.world_z) + d,
        ];
        let q = [
            b[0].max(r[0]),
            b[1].max(r[1]),
            b[2].min(r[2]),
            b[3].min(r[3]),
        ];
        if q[0] <= q[2]
            && q[1] <= q[3]
            && !road_extensions::shared_junctions(plan, &road.entity_id, c.start, c.end)
                .iter()
                .any(|p| {
                    q[0] >= i64::from(p[0]) - 8
                        && q[2] <= i64::from(p[0]) + 8
                        && q[1] >= i64::from(p[1]) - 8
                        && q[3] <= i64::from(p[1]) + 8
                })
        {
            return Err(err("existing_road_crossing"));
        }
    }
    for site in &plan.roadside_sites {
        let (w, d) = site.world_half_extents();
        if b[0] < i64::from(site.world_x) + i64::from(w) + 4
            && b[2] > i64::from(site.world_x) - i64::from(w) - 4
            && b[1] < i64::from(site.world_z) + i64::from(d) + 4
            && b[3] > i64::from(site.world_z) - i64::from(d) - 4
        {
            return Err(err("existing_obligated_site"));
        }
    }
    Ok(())
}
fn category(reason: &str) -> String {
    if reason.contains("infeasible") || reason.contains("no feasible constant") {
        "profile_infeasible".into()
    } else if reason.contains("protected") || reason.contains("zoning") {
        "reserved_or_outside_zoning".into()
    } else if reason.contains("padded footprint") {
        "padded_support_fill".into()
    } else {
        reason.into()
    }
}
pub(super) fn append(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    t: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
    style: &serde_json::Value,
) -> Result<(), TerrainError> {
    let Some(all) = from_style(style)? else {
        return Ok(());
    };
    let Some(program) = all
        .scenes
        .into_iter()
        .find(|s| s.scene_id == scene.scene_id)
    else {
        return Ok(());
    };
    let original = plan.entities();
    let mut targets: Vec<_> = zones
        .iter()
        .filter(|z| program.priority_purposes.contains(&z.kind))
        .map(|z| {
            let centre = [
                z.world_x + (z.width_m / 2) as i32,
                z.world_z + (z.depth_m / 2) as i32,
            ];
            let before = nearest(centre, &original);
            let bounds = [
                i64::from(z.world_x),
                i64::from(z.world_z),
                i64::from(z.world_x) + i64::from(z.width_m),
                i64::from(z.world_z) + i64::from(z.depth_m),
            ];
            let entered = has_road(bounds, &original);
            Target {
                zone_bounds: bounds,
                before_has_road: entered,
                after_has_road: entered,
                zone_id: z.id.clone(),
                purpose: z.kind.clone(),
                centre,
                before_distance_m: before,
                after_distance_m: before,
            }
        })
        .collect();
    targets.sort_by(|a, b| a.zone_id.cmp(&b.zone_id));
    let ctx = AlignmentContext::new(scene, t, water, zones)?;
    let mut report = Report {
        version: 1,
        model: "bounded-purpose-centre-and-zone-growth-v2".into(),
        scope: "zone-road-intersection-and-centre-proximity-not-realized-land-or-global-optimality"
            .into(),
        program: program.clone(),
        stop_reason: String::new(),
        attempted_candidates: 0,
        accepted_segments: 0,
        generated_road_ids: vec![],
        targets,
        unserved_targets: 0,
        rejected_by_reason: BTreeMap::new(),
        rejection_examples: vec![],
    };
    let mut attempted = BTreeSet::new();
    'growth: loop {
        let roads = plan.entities();
        let node_levels = levels(plan, &roads)?;
        for target in &mut report.targets {
            target.after_distance_m = nearest(target.centre, &roads);
            target.after_has_road = has_road(target.zone_bounds, &roads);
        }
        report.unserved_targets = report
            .targets
            .iter()
            .filter(|t| {
                !t.after_has_road
                    || t.after_distance_m
                        .is_none_or(|d| d > u64::from(program.reach_distance_m))
            })
            .count();
        let stop = if report.targets.is_empty() {
            Some("no_target_zones")
        } else if report.unserved_targets == 0 {
            Some("all_target_zones_entered_and_centres_near_road")
        } else if report.accepted_segments >= program.max_new_segments {
            Some("segment_budget_exhausted")
        } else if report.attempted_candidates >= program.max_candidate_attempts {
            Some("candidate_budget_exhausted")
        } else if node_levels.is_empty() {
            Some("no_existing_connected_frontier")
        } else {
            None
        };
        if let Some(stop) = stop {
            report.stop_reason = stop.into();
            break;
        }
        let mut candidates = vec![];
        for n in &plan.nodes {
            for [dx, dz] in [[-1, 0], [0, -1], [1, 0], [0, 1]] {
                for &length in &program.segment_lengths_m {
                    let x = i64::from(n.world_x) + i64::from(dx) * i64::from(length);
                    let z = i64::from(n.world_z) + i64::from(dz) * i64::from(length);
                    if x < i64::from(scene.origin_x) + 8
                        || z < i64::from(scene.origin_z) + 8
                        || x + 8 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
                        || z + 8 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
                    {
                        continue;
                    }
                    let mut c = Candidate {
                        start: [n.world_x, n.world_z],
                        end: [x as i32, z as i32],
                        node: n.id,
                        priority: usize::MAX,
                        served: 0,
                        gain: 0,
                        length,
                    };
                    if attempted.contains(&c.key()) {
                        continue;
                    }
                    for (priority, purpose) in program.priority_purposes.iter().enumerate() {
                        for target in report.targets.iter().filter(|t| {
                            &t.purpose == purpose
                                && (!t.after_has_road
                                    || t.after_distance_m
                                        .is_some_and(|d| d > u64::from(program.reach_distance_m)))
                        }) {
                            let old = target.after_distance_m.unwrap();
                            let next = old.min(distance(target.centre, c.bounds(0)));
                            c.gain += old - next;
                            if next <= u64::from(program.reach_distance_m)
                                && (target.after_has_road
                                    || intersects(target.zone_bounds, c.bounds(0)))
                            {
                                c.served += 1;
                            }
                        }
                        if c.gain > 0 || c.served > 0 {
                            c.priority = priority;
                            break;
                        }
                    }
                    if c.priority != usize::MAX {
                        candidates.push(c);
                    }
                }
            }
        }
        candidates.sort_by_key(|c| {
            (
                c.priority,
                Reverse(c.served),
                Reverse(c.gain),
                c.length,
                c.node,
                c.end,
            )
        });
        if candidates.is_empty() {
            report.stop_reason = "no_progress_candidate".into();
            break;
        }
        for c in candidates {
            if report.attempted_candidates >= program.max_candidate_attempts {
                report.stop_reason = "candidate_budget_exhausted".into();
                break 'growth;
            }
            attempted.insert(c.key());
            report.attempted_candidates += 1;
            let id = format!(
                "generated.road-auto.{}.{}",
                scene.scene_id, report.accepted_segments
            );
            let result = (|| {
                clear(&c, plan, &roads, &ctx)?;
                let r = CorridorRequest {
                    scene_id: scene.scene_id.clone(),
                    start: c.start,
                    end: c.end,
                    road_width_m: 8,
                    shoulder_width_m: 3,
                    max_fill_mm: 6000,
                    max_rise_mm_per_m: 80,
                    start_level_mm: Some(
                        *node_levels
                            .get(&c.start)
                            .ok_or_else(|| err("missing start node height"))?,
                    ),
                    end_level_mm: node_levels.get(&c.end).copied(),
                    junction_length_m: 8,
                };
                let request = ctx.infer_end(&r)?;
                road_extensions::append_one(plan, &ctx, &request, id.clone())
            })();
            match result {
                Ok(()) => {
                    report.accepted_segments += 1;
                    report.generated_road_ids.push(id);
                    continue 'growth;
                }
                Err(e) => {
                    *report.rejected_by_reason.entry(category(&e.0)).or_default() += 1;
                    if report.rejection_examples.len() < 32 {
                        report.rejection_examples.push(RejectedExample {
                            start: c.start,
                            end: c.end,
                            reason: e.0,
                        });
                    }
                }
            }
        }
        report.stop_reason = "no_feasible_progress_candidate".into();
        break;
    }
    plan.automatic_extensions = Some(report);
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
        let s = SceneSpec {
            scene_id: "auto".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut t = BakedTerrain::new(&s).unwrap();
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
                            f64::from(x - 100) * 0.02
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
            &s,
            &serde_json::Value::Null,
        );
        let zones = vec![
            LandUseZone {
                id: "rural".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: 0,
                width_m: 320,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "urban".into(),
                kind: "urban".into(),
                world_x: 320,
                world_z: 0,
                width_m: 192,
                depth_m: 512,
                source: "test".into(),
            },
        ];
        let p:RoadNetworkPlan=serde_json::from_value(serde_json::json!({"model":"fixture","naturalHeightfieldHash":t.content_hash().unwrap(),"sceneId":"auto","status":"planned_connected_network","gridSpacingM":64,"maxRoadbedFillM":6,"levelM":1,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,"nodes":[{"id":0,"worldX":32,"worldZ":128},{"id":1,"worldX":96,"worldZ":128}],"edges":[{"id":"base","from":0,"to":1,"worldX":64,"worldZ":128,"lengthM":64,"rotationYDeg":0,"naturalMinM":0,"naturalMaxM":0}],"roadsideSites":[]})).unwrap();
        let style = serde_json::json!({"automaticRoadExtensions":{"version":1,"scenes":[{"sceneId":"auto","maxNewSegments":3,"maxCandidateAttempts":100,"segmentLengthsM":[256,128,64],"priorityPurposes":["urban"],"reachDistanceM":128}]}});
        (s, t, water, zones, p, style)
    }
    #[test]
    fn automatically_reaches_target_without_coordinates_or_level_inputs() {
        let (s, t, w, z, mut p, style) = fixture();
        let before = p.entities();
        append(&mut p, &s, &t, w, &z, &style).unwrap();
        let a = p.automatic_extensions.as_ref().unwrap();
        assert_eq!(
            a.stop_reason,
            "all_target_zones_entered_and_centres_near_road"
        );
        assert_eq!(a.unserved_targets, 0);
        assert!(a.accepted_segments >= 2);
        assert_eq!(a.generated_road_ids.len(), a.accepted_segments);
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(&p.entities()[..1]).unwrap()
        );
        assert!(
            p.extensions
                .iter()
                .any(|e| e.design.request.start_level_mm != e.design.request.end_level_mm)
        );
        let mut repeated = fixture().4;
        append(&mut repeated, &s, &t, w, &z, &style).unwrap();
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::to_value(repeated).unwrap()
        );
    }
    #[test]
    fn segment_and_candidate_budgets_keep_unsatisfied_targets() {
        let (s, t, w, z, p, style) = fixture();
        for (field, expected) in [
            ("maxNewSegments", "segment_budget_exhausted"),
            ("maxCandidateAttempts", "candidate_budget_exhausted"),
        ] {
            let mut limited = style.clone();
            limited["automaticRoadExtensions"]["scenes"][0][field] = serde_json::json!(1);
            let mut result = p.clone();
            append(&mut result, &s, &t, w, &z, &limited).unwrap();
            let a = result.automatic_extensions.unwrap();
            assert_eq!(a.stop_reason, expected);
            assert!(a.unserved_targets > 0);
            assert!(a.accepted_segments <= 1);
            assert_eq!(
                a.attempted_candidates,
                a.accepted_segments + a.rejected_by_reason.values().sum::<usize>()
            );
        }
    }
    #[test]
    fn protection_barrier_and_missing_frontier_are_not_bypassed() {
        let (s, t, w, _, p, style) = fixture();
        let zones = vec![
            LandUseZone {
                id: "rural".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: 0,
                width_m: 160,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "protected".into(),
                kind: "nature".into(),
                world_x: 160,
                world_z: 0,
                width_m: 64,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "urban".into(),
                kind: "urban".into(),
                world_x: 224,
                world_z: 0,
                width_m: 288,
                depth_m: 512,
                source: "test".into(),
            },
        ];
        let mut result = p.clone();
        append(&mut result, &s, &t, w, &zones, &style).unwrap();
        let a = result.automatic_extensions.as_ref().unwrap();
        assert!(a.unserved_targets > 0);
        assert!(
            a.rejected_by_reason
                .contains_key("reserved_or_outside_zoning")
        );
        for e in &result.extensions {
            assert!(e.design.request.end[0] + 7 < 160);
        }
        let mut empty = p;
        empty.edges.clear();
        empty.nodes.clear();
        append(&mut empty, &s, &t, w, &zones, &style).unwrap();
        assert_eq!(
            empty.automatic_extensions.unwrap().stop_reason,
            "no_existing_connected_frontier"
        );
    }
    #[test]
    fn malformed_or_coordinate_supplied_auto_program_is_rejected() {
        let (_, _, _, _, _, style) = fixture();
        for field in ["maxNewSegments", "maxCandidateAttempts"] {
            let mut bad = style.clone();
            bad["automaticRoadExtensions"]["scenes"][0][field] = serde_json::json!(0);
            assert!(from_style(&bad).is_err());
        }
        let mut bad = style.clone();
        bad["automaticRoadExtensions"]["scenes"][0]["start"] = serde_json::json!([0, 0]);
        assert!(from_style(&bad).is_err());
        bad = style;
        bad["automaticRoadExtensions"]["scenes"][0]["priorityPurposes"] =
            serde_json::json!(["nature"]);
        assert!(from_style(&bad).is_err());
    }
    #[test]
    fn nearby_road_outside_zone_does_not_satisfy_actual_zone_entry() {
        let (s, t, w, _, mut p, mut style) = fixture();
        let zone = |id: &str, kind: &str, x, z, width, depth| LandUseZone {
            id: id.into(),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            width_m: width,
            depth_m: depth,
            source: "test".into(),
        };
        let zones = vec![
            zone("west", "rural", 0, 0, 128, 512),
            zone("north", "rural", 128, 0, 384, 128),
            zone("urban", "urban", 128, 128, 128, 128),
            zone("east", "rural", 256, 128, 256, 128),
            zone("south", "rural", 128, 256, 384, 256),
        ];
        style["automaticRoadExtensions"]["scenes"][0]["reachDistanceM"] = serde_json::json!(160);
        append(&mut p, &s, &t, w, &zones, &style).unwrap();
        let report = p.automatic_extensions.unwrap();
        assert!(report.targets[0].before_distance_m.unwrap() <= 160);
        assert!(!report.targets[0].before_has_road);
        assert!(report.accepted_segments > 0);
        assert!(report.targets[0].after_has_road);
        assert_eq!(report.unserved_targets, 0);
    }
    #[test]
    fn automatic_growth_is_translation_covariant_for_nonzero_origins() {
        let (s, t, w, z, p, style) = fixture();
        let mut first = p.clone();
        append(&mut first, &s, &t, w, &z, &style).unwrap();
        let (ox, oz) = (-1_000_000, 2_000_000);
        let mut shifted = s.clone();
        shifted.origin_x = ox;
        shifted.origin_z = oz;
        let bytes = encode_millimetres(
            512,
            512,
            (0..512)
                .flat_map(|zz| (0..512).map(move |xx| (xx, zz)))
                .map(|(x, z)| f64::from(t.height_millimetres(x, z).unwrap()) / 1000.),
        )
        .unwrap();
        let mut raster = BakedTerrain::new(&shifted).unwrap();
        raster.insert_chunk(ox, oz, 512, 512, &bytes).unwrap();
        let mut zones = z.clone();
        for q in &mut zones {
            q.world_x += ox;
            q.world_z += oz;
        }
        let mut plan = p;
        for n in &mut plan.nodes {
            n.world_x += ox;
            n.world_z += oz;
        }
        for e in &mut plan.edges {
            e.world_x += ox;
            e.world_z += oz;
        }
        plan.natural_heightfield_hash = raster.content_hash().unwrap();
        append(&mut plan, &shifted, &raster, w, &zones, &style).unwrap();
        assert_eq!(first.extensions.len(), plan.extensions.len());
        for (a, b) in first.extensions.iter().zip(&plan.extensions) {
            assert_eq!(a.design.profile_mm, b.design.profile_mm);
            assert_eq!(
                a.design.request.start,
                [
                    b.design.request.start[0] - ox,
                    b.design.request.start[1] - oz
                ]
            );
            assert_eq!(
                a.design.request.end,
                [b.design.request.end[0] - ox, b.design.request.end[1] - oz]
            );
        }
        let a = first.automatic_extensions.unwrap();
        let b = plan.automatic_extensions.unwrap();
        assert_eq!(a.stop_reason, b.stop_reason);
        assert_eq!(a.attempted_candidates, b.attempted_candidates);
        assert_eq!(
            a.targets
                .iter()
                .map(|t| t.after_distance_m)
                .collect::<Vec<_>>(),
            b.targets
                .iter()
                .map(|t| t.after_distance_m)
                .collect::<Vec<_>>()
        );
    }
}
