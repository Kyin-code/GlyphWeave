//! Deterministic new/old-district demand expansion.
//!
//! This is a bounded vertical slice of the urban-history requirement, not a
//! population or economic simulation. An author states a city centre and a
//! short ordered list of development iterations. Each iteration owns one radial
//! band and a set of frontage requests. Expansion writes those requests into the
//! existing `frontageProgram` contract, where every road, parcel, terrain and
//! access gate continues to run unchanged.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::worldgen::{SceneSpec, road_network::RoadNetworkPlan};

const MAX_ITERATIONS: usize = 8;
const MAX_REQUESTS: usize = 64;
const MAX_CENTER_DISTANCE_M: u32 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub scene_id: String,
    pub center_x: i32,
    pub center_z: i32,
    /// Shared `frontageProgram` fields such as rural conversion and terrain
    /// policy. `requests` is intentionally not accepted here.
    #[serde(default)]
    pub frontage: Map<String, Value>,
    pub iterations: Vec<Iteration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Iteration {
    pub id: String,
    pub construction_cohort: String,
    pub center_distance_m: [u32; 2],
    pub requests: Vec<IterationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IterationRequest {
    pub id: String,
    pub kind: String,
    pub count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
    pub station_spacing_m: u32,
    pub setback_m: u32,
    pub max_candidate_attempts: u32,
    #[serde(default)]
    pub purpose: Option<String>,
    /// Optional tighter radius for one request inside its iteration.
    #[serde(default)]
    pub center_distance_m: Option<[u32; 2]>,
}

pub fn expand(style: &Value, scenes: &[SceneSpec]) -> Result<Value, String> {
    let Some(raw) = style.get("urbanDistrictProgram") else {
        return Ok(style.clone());
    };
    let program: Program = serde_json::from_value(raw.clone())
        .map_err(|error| format!("invalid urbanDistrictProgram: {error}"))?;
    validate(&program, scenes)?;

    let mut effective = style.clone();
    let root = effective
        .as_object_mut()
        .ok_or_else(|| "style must be a JSON object".to_string())?;
    let frontage = if let Some(value) = root.get_mut("frontageProgram") {
        value
            .as_object_mut()
            .ok_or_else(|| "frontageProgram must be an object".to_string())?
    } else {
        let mut value = program.frontage.clone();
        value.insert("version".into(), json!(1));
        value.insert("requests".into(), json!([]));
        root.insert("frontageProgram".into(), Value::Object(value));
        root.get_mut("frontageProgram")
            .and_then(Value::as_object_mut)
            .expect("frontageProgram was just inserted")
    };
    if frontage.get("version") != Some(&json!(1)) {
        return Err("frontageProgram.version must be 1".into());
    }
    for (key, value) in &program.frontage {
        if key == "requests" || key == "version" {
            return Err(format!(
                "urbanDistrictProgram.frontage.{key} is reserved or unsupported"
            ));
        }
        if !frontage.contains_key(key) {
            frontage.insert(key.clone(), value.clone());
        }
    }
    let existing = frontage
        .entry("requests")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| "frontageProgram.requests must be an array".to_string())?;
    let mut ids: BTreeSet<String> = existing
        .iter()
        .filter_map(|request| request.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    for iteration in &program.iterations {
        for request in &iteration.requests {
            let id = format!("{}-{}", iteration.id, request.id);
            if !ids.insert(id.clone()) {
                return Err(format!("duplicate expanded district request id {id}"));
            }
            let ring = request
                .center_distance_m
                .unwrap_or(iteration.center_distance_m);
            existing.push(json!({
                "id": id,
                "sceneId": program.scene_id,
                "purpose": request.purpose.as_deref().unwrap_or("urban"),
                "kind": request.kind,
                "count": request.count,
                "widthM": request.width_m,
                "depthM": request.depth_m,
                "heightM": request.height_m,
                "stationSpacingM": request.station_spacing_m,
                "setbackM": request.setback_m,
                "maxCandidateAttempts": request.max_candidate_attempts,
                "centerDistanceM": ring,
                "centerWorld": [program.center_x, program.center_z],
                "districtId": iteration.id,
                "constructionCohort": iteration.construction_cohort,
            }));
        }
    }
    Ok(effective)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub center_world: [i32; 2],
    pub status: String,
    pub iterations: Vec<IterationAudit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IterationAudit {
    pub id: String,
    pub construction_cohort: String,
    pub center_distance_m: [u32; 2],
    pub requested: u32,
    pub planned: u32,
    pub unfilled: u32,
    pub by_kind: BTreeMap<String, u32>,
    pub average_center_distance_m: Option<f64>,
    pub rejected: BTreeMap<String, u32>,
}

pub fn audit(
    style: &Value,
    scene: &SceneSpec,
    plan: &RoadNetworkPlan,
) -> Result<Option<Audit>, String> {
    let Some(raw) = style.get("urbanDistrictProgram") else {
        return Ok(None);
    };
    let program: Program = serde_json::from_value(raw.clone())
        .map_err(|error| format!("invalid urbanDistrictProgram: {error}"))?;
    if program.scene_id != scene.scene_id {
        return Ok(None);
    }
    let Some(report) = &plan.frontage else {
        return Ok(None);
    };
    let mut iterations = Vec::new();
    for iteration in &program.iterations {
        let outcomes: Vec<_> = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.request.district_id.as_deref() == Some(iteration.id.as_str()))
            .collect();
        let requested = outcomes.iter().map(|outcome| outcome.request.count).sum();
        let planned = outcomes
            .iter()
            .map(|outcome| outcome.site_ids.len() as u32)
            .sum();
        let unfilled = outcomes.iter().map(|outcome| outcome.unfilled).sum();
        let mut by_kind = BTreeMap::new();
        let mut rejected = BTreeMap::new();
        for outcome in &outcomes {
            *by_kind.entry(outcome.request.kind.clone()).or_default() +=
                outcome.site_ids.len() as u32;
            for (reason, count) in &outcome.rejected {
                *rejected.entry(reason.clone()).or_default() += count;
            }
        }
        let ids: BTreeSet<&str> = outcomes
            .iter()
            .flat_map(|outcome| outcome.site_ids.iter().map(String::as_str))
            .collect();
        let distances: Vec<f64> = plan
            .roadside_sites
            .iter()
            .filter(|site| ids.contains(site.id.as_str()))
            .map(|site| {
                let dx = f64::from(site.world_x - program.center_x);
                let dz = f64::from(site.world_z - program.center_z);
                dx.hypot(dz)
            })
            .collect();
        iterations.push(IterationAudit {
            id: iteration.id.clone(),
            construction_cohort: iteration.construction_cohort.clone(),
            center_distance_m: iteration.center_distance_m,
            requested,
            planned,
            unfilled,
            by_kind,
            average_center_distance_m: (!distances.is_empty()).then(|| {
                let sum: f64 = distances.iter().sum();
                (sum / distances.len() as f64 * 100.).round() / 100.
            }),
            rejected,
        });
    }
    let status = if iterations.iter().all(|iteration| iteration.unfilled == 0) {
        "pass"
    } else {
        "partial"
    };
    Ok(Some(Audit {
        format: "glyphweave.urban-district-audit".into(),
        version: 1,
        scene_id: scene.scene_id.clone(),
        center_world: [program.center_x, program.center_z],
        status: status.into(),
        iterations,
    }))
}

fn validate(program: &Program, scenes: &[SceneSpec]) -> Result<(), String> {
    if program.version != 1
        || program.iterations.is_empty()
        || program.iterations.len() > MAX_ITERATIONS
    {
        return Err("urbanDistrictProgram requires version 1 and 1..=8 iterations".into());
    }
    let scene = scenes
        .iter()
        .find(|scene| scene.scene_id == program.scene_id)
        .ok_or_else(|| "urbanDistrictProgram.sceneId names no scene".to_string())?;
    let max_x = scene.origin_x.saturating_add(scene.width_m as i32);
    let max_z = scene.origin_z.saturating_add(scene.depth_m as i32);
    if program.center_x < scene.origin_x
        || program.center_z < scene.origin_z
        || program.center_x >= max_x
        || program.center_z >= max_z
    {
        return Err("urbanDistrictProgram centre lies outside its scene".into());
    }
    let mut iterations = BTreeSet::new();
    let mut requests = 0usize;
    let mut ids = BTreeSet::new();
    for iteration in &program.iterations {
        if !valid_id(&iteration.id) || !iterations.insert(iteration.id.as_str()) {
            return Err("invalid or duplicate urban district iteration id".into());
        }
        if !matches!(
            iteration.construction_cohort.as_str(),
            "established" | "renewal" | "new"
        ) {
            return Err("constructionCohort must be established, renewal or new".into());
        }
        valid_ring(iteration.center_distance_m)?;
        if iteration.requests.is_empty() {
            return Err(format!(
                "district iteration {} has no requests",
                iteration.id
            ));
        }
        for request in &iteration.requests {
            requests += 1;
            if requests > MAX_REQUESTS {
                return Err("urbanDistrictProgram expands beyond 64 requests".into());
            }
            if !valid_id(&request.id) {
                return Err(format!("invalid district request id {:?}", request.id));
            }
            if !ids.insert((iteration.id.as_str(), request.id.as_str())) {
                return Err(format!(
                    "duplicate district request id {}/{}",
                    iteration.id, request.id
                ));
            }
            if request.count == 0 {
                return Err("district request count must be positive".into());
            }
            if let Some(ring) = request.center_distance_m {
                valid_ring(ring)?;
            }
            if request
                .purpose
                .as_deref()
                .is_some_and(|purpose| !matches!(purpose, "urban" | "rural"))
            {
                return Err("district request purpose must be urban or rural".into());
            }
        }
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 24
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_ring(ring: [u32; 2]) -> Result<(), String> {
    if ring[0] > ring[1] || ring[1] > MAX_CENTER_DISTANCE_M {
        return Err("centerDistanceM must satisfy inner <= outer <= 10000".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::SceneSpec;

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn style() -> Value {
        json!({
            "morphologyProfile": "compact-mixed-block",
            "urbanDistrictProgram": {
                "version": 1,
                "sceneId": "scene-0",
                "centerX": 320,
                "centerZ": 320,
                "frontage": {
                    "ruralConversion": "explicit",
                    "terrainPolicy": "bounded-cut",
                    "maxCutMm": 2000
                },
                "iterations": [
                    {
                        "id": "old-core",
                        "constructionCohort": "established",
                        "centerDistanceM": [0, 160],
                        "requests": [{
                            "id": "shops",
                            "kind": "storefront",
                            "count": 4,
                            "widthM": 12,
                            "depthM": 10,
                            "heightM": 6,
                            "stationSpacingM": 16,
                            "setbackM": 8,
                            "maxCandidateAttempts": 256
                        }]
                    },
                    {
                        "id": "new-ring",
                        "constructionCohort": "new",
                        "centerDistanceM": [180, 360],
                        "requests": [{
                            "id": "homes",
                            "kind": "residential_home",
                            "count": 8,
                            "widthM": 16,
                            "depthM": 12,
                            "heightM": 8,
                            "stationSpacingM": 16,
                            "setbackM": 8,
                            "maxCandidateAttempts": 256
                        }]
                    }
                ]
            }
        })
    }

    #[test]
    fn iterations_expand_to_radial_frontage_requests() {
        let expanded = expand(&style(), &[scene()]).unwrap();
        let requests = expanded["frontageProgram"]["requests"].as_array().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["id"], "old-core-shops");
        assert_eq!(requests[0]["centerWorld"], json!([320, 320]));
        assert_eq!(requests[0]["centerDistanceM"], json!([0, 160]));
        assert_eq!(requests[0]["constructionCohort"], "established");
        assert_eq!(requests[1]["constructionCohort"], "new");
        assert_eq!(expanded["frontageProgram"]["terrainPolicy"], "bounded-cut");
        let parsed = crate::worldgen::frontage::from_style(&expanded)
            .unwrap()
            .unwrap();
        assert!(parsed.requests[0].allows_center_distance(320, 320));
        assert!(!parsed.requests[0].allows_center_distance(500, 320));
        assert!(parsed.requests[1].allows_center_distance(500, 320));
    }

    #[test]
    fn invalid_rings_and_centres_fail_closed() {
        let mut bad_ring = style();
        bad_ring["urbanDistrictProgram"]["iterations"][0]["centerDistanceM"] = json!([200, 100]);
        assert!(expand(&bad_ring, &[scene()]).is_err());

        let mut outside = style();
        outside["urbanDistrictProgram"]["centerX"] = json!(900);
        assert!(expand(&outside, &[scene()]).is_err());
    }
}
