//! Immutable natural-stage baseline and final earthwork diff.
//!
//! The natural raster is frozen before routes, buildings or engineering exist.
//! The final bake is then compared cell-by-cell against that same raster so
//! every road bed, pad, terrace, reclamation or excavation remains accountable.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::baked_terrain::{BakedTerrain, HeightComparison, TerrainError};
use crate::worldgen::SceneSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub status: String,
    pub stage_order: Vec<String>,
    pub human_trace_count: usize,
    pub road_plan_present: bool,
    pub development_plan_present: bool,
    pub cells: usize,
    pub natural_height_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural_controls_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub status: String,
    pub baseline_unchanged: bool,
    pub natural_height_hash: String,
    pub final_height_hash: String,
    pub engineering_operation_count: usize,
    pub diff: HeightComparison,
}

pub fn freeze(
    scene: &SceneSpec,
    natural: &BakedTerrain,
    natural_controls: Option<&Value>,
) -> Result<Baseline, TerrainError> {
    if !natural.matches_scene(scene) {
        return Err(TerrainError(
            "natural baseline scene/terrain mismatch".into(),
        ));
    }
    natural.require_complete()?;
    let (width, depth) = natural.scene_dimensions();
    let natural_controls_hash = natural_controls
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|error| TerrainError(format!("natural controls encode failed: {error}")))?
        .map(|bytes| blake3::hash(&bytes).to_hex().to_string());
    Ok(Baseline {
        format: "glyphweave.natural-baseline".into(),
        version: 1,
        scene_id: scene.scene_id.clone(),
        status: "pass".into(),
        stage_order: vec![
            "natural-heightfield".into(),
            "natural-drainage".into(),
            "human-planning".into(),
        ],
        human_trace_count: 0,
        road_plan_present: false,
        development_plan_present: false,
        cells: width as usize * depth as usize,
        natural_height_hash: natural.content_hash()?,
        natural_controls_hash,
    })
}

pub fn final_diff(
    scene: &SceneSpec,
    baseline: &Baseline,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    engineering_operation_count: usize,
) -> Result<Diff, TerrainError> {
    if baseline.scene_id != scene.scene_id || baseline.status != "pass" {
        return Err(TerrainError(
            "natural baseline is not usable for this scene".into(),
        ));
    }
    let natural_hash = natural.content_hash()?;
    if natural_hash != baseline.natural_height_hash {
        return Err(TerrainError(
            "natural raster changed after baseline freeze".into(),
        ));
    }
    let final_hash = final_terrain.content_hash()?;
    let diff = natural.compare_heightfields(final_terrain)?;
    Ok(Diff {
        format: "glyphweave.natural-final-diff".into(),
        version: 1,
        scene_id: scene.scene_id.clone(),
        status: "pass".into(),
        baseline_unchanged: final_hash == natural_hash,
        natural_height_hash: natural_hash,
        final_height_hash: final_hash,
        engineering_operation_count,
        diff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height_payload::encode_millimetres;

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "baseline".into(),
            width_m: 2,
            depth_m: 2,
            origin_x: -3,
            origin_z: 7,
            seed_offset: 0,
        }
    }

    fn terrain(samples: [f64; 4]) -> BakedTerrain {
        let spec = scene();
        let mut terrain = BakedTerrain::new(&spec).unwrap();
        terrain
            .insert_chunk(
                spec.origin_x,
                spec.origin_z,
                2,
                2,
                &encode_millimetres(2, 2, samples).unwrap(),
            )
            .unwrap();
        terrain
    }

    #[test]
    fn baseline_is_immutable_and_final_diff_is_exact() {
        let scene = scene();
        let natural = terrain([0., 0., 0., 0.]);
        let baseline = freeze(
            &scene,
            &natural,
            Some(&serde_json::json!({"setting": "coastal-mountain"})),
        )
        .unwrap();
        assert_eq!(baseline.human_trace_count, 0);
        assert!(baseline.natural_controls_hash.is_some());

        let final_terrain = terrain([0., 0.5, -0.25, 0.]);
        let report = final_diff(&scene, &baseline, &natural, &final_terrain, 2).unwrap();
        assert_eq!(report.engineering_operation_count, 2);
        assert_eq!(report.diff.changed_cells, 2);
        assert_eq!(report.diff.raised_cells, 1);
        assert_eq!(report.diff.lowered_cells, 1);
        assert_eq!(report.diff.max_rise_mm, 500);
        assert_eq!(report.diff.max_lower_mm, 250);
        assert!(!report.baseline_unchanged);
    }

    #[test]
    fn changed_natural_raster_after_freeze_is_refused() {
        let scene = scene();
        let baseline = freeze(&scene, &terrain([0., 0., 0., 0.]), None).unwrap();
        let changed_natural = terrain([1., 0., 0., 0.]);
        let final_terrain = terrain([1., 0., 0., 0.]);
        assert!(final_diff(&scene, &baseline, &changed_natural, &final_terrain, 0).is_err());
    }
}
