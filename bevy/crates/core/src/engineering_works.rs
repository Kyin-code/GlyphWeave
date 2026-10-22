//! C-E1 first engineering structures: retaining walls and culvert records.
//!
//! These are data entities with stable ids, geometry, volume and source
//! operation. Rendering is intentionally left to consumers; the Rust core
//! publishes the same record the HTML/Godot adapters can later mesh. No entity
//! is invented for a fill the terrain engine did not actually apply.
use crate::baked_terrain::BakedTerrain;
use crate::engineering::EngineeredTerrainOperation;
use crate::worldgen::road_network::WaterCrossing;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineeringStructure {
    pub id: String,
    pub kind: String,
    pub entity_id: String,
    pub source_operation_id: String,
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub height_m: f32,
    pub length_m: f32,
    pub thickness_m: f32,
    pub volume_m3: f32,
    pub material: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CulvertRecord {
    pub id: String,
    pub kind: String,
    pub road_id: String,
    pub water_id: String,
    pub span: [i32; 4],
    pub length_m: u32,
    pub wet_cells: u32,
    pub invert_mm: i32,
    pub deck_mm: i32,
    pub diameter_m: f32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainWorksPlan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub policy: String,
    pub retaining_walls: Vec<EngineeringStructure>,
    pub culverts: Vec<CulvertRecord>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct WorksPolicy {
    pub retaining_walls: bool,
    pub drainage_structures: bool,
    pub min_fill_mm: i32,
    pub max_wall_height_mm: i32,
    pub max_structures: usize,
}

impl Default for WorksPolicy {
    fn default() -> Self {
        Self {
            retaining_walls: false,
            drainage_structures: false,
            min_fill_mm: 300,
            max_wall_height_mm: 6_000,
            max_structures: 4_096,
        }
    }
}

pub fn policy_from_style(style: &serde_json::Value) -> Result<Option<WorksPolicy>, String> {
    let Some(value) = style.get("terrainWorks") else {
        return Ok(None);
    };
    let object = value
        .as_object()
        .ok_or("terrainWorks must be an object")?;
    let mut policy = WorksPolicy::default();
    for (key, target) in [
        ("retainingWalls", &mut policy.retaining_walls),
        ("drainageStructures", &mut policy.drainage_structures),
    ] {
        if let Some(value) = object.get(key) {
            *target = match value.as_str() {
                Some("auto") => true,
                Some(other) => return Err(format!("terrainWorks.{key} has unsupported value {other}")),
                None => return Err(format!("terrainWorks.{key} must be a string")),
            };
        }
    }
    for (key, target, min, max) in [
        ("minFillMm", &mut policy.min_fill_mm, 0, 6_000),
        ("maxWallHeightMm", &mut policy.max_wall_height_mm, 300, 6_000),
    ] {
        if let Some(value) = object.get(key) {
            let value = value
                .as_i64()
                .ok_or_else(|| format!("terrainWorks.{key} must be an integer"))?;
            if value < min || value > max {
                return Err(format!("terrainWorks.{key} must be within {min}..={max}"));
            }
            *target = value as i32;
        }
    }
    if let Some(value) = object.get("maxStructures") {
        let value = value
            .as_u64()
            .ok_or("terrainWorks.maxStructures must be a non-negative integer")?;
        if value == 0 || value > 65_536 {
            return Err("terrainWorks.maxStructures must be within 1..=65536".into());
        }
        policy.max_structures = value as usize;
    }
    Ok(Some(policy))
}

fn height_at(terrain: &BakedTerrain, x: f32, z: f32) -> Option<f64> {
    terrain
        .height(x.round() as i32, z.round() as i32)
        .map(f64::from)
}

/// Plan structures from the operations actually applied to the final raster.
/// A fill thinner than `min_fill_mm` is left alone; a fill taller than the
/// declared wall cap is a named failure instead of a silent short wall.
pub fn plan(
    operations: &[EngineeredTerrainOperation],
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    crossings: &[WaterCrossing],
    policy: Option<WorksPolicy>,
) -> Option<TerrainWorksPlan> {
    let policy = policy?;
    if !policy.retaining_walls && !policy.drainage_structures {
        return None;
    }
    let mut walls = Vec::new();
    let mut failures = Vec::new();
    if policy.retaining_walls {
        for operation in operations {
            let Some(natural_h) = height_at(natural, operation.center_x, operation.center_z) else {
                failures.push(format!("{}: natural height missing", operation.id));
                continue;
            };
            let Some(final_h) = height_at(final_terrain, operation.center_x, operation.center_z)
            else {
                failures.push(format!("{}: final height missing", operation.id));
                continue;
            };
            let fill_mm = ((final_h - natural_h) * 1_000.).round() as i32;
            if fill_mm < policy.min_fill_mm {
                continue;
            }
            if fill_mm > policy.max_wall_height_mm {
                failures.push(format!(
                    "{}: retaining height {fill_mm}mm exceeds maximum {}mm",
                    operation.id, policy.max_wall_height_mm
                ));
                continue;
            }
            let cx = operation.center_x as f64;
            let cz = operation.center_z as f64;
            let hw = f64::from(operation.half_width_m);
            let hd = f64::from(operation.half_depth_m);
            let height = fill_mm as f32 / 1_000.;
            let thickness = 0.4_f32;
            let sides: [[[f32; 2]; 2]; 4] = [
                [[(cx - hw) as f32, cz as f32], [(cx + hw) as f32, cz as f32]],
                [[(cx + hw) as f32, cz as f32], [(cx + hw) as f32, (cz + hd) as f32]],
                [[(cx + hw) as f32, (cz + hd) as f32], [(cx - hw) as f32, (cz + hd) as f32]],
                [[(cx - hw) as f32, (cz + hd) as f32], [(cx - hw) as f32, cz as f32]],
            ];
            let side_indices: &[usize] = if operation.kind == "roadbed" {
                if hw >= hd { &[0, 2] } else { &[1, 3] }
            } else {
                &[0, 1, 2, 3]
            };
            for side in side_indices.iter().map(|index| &sides[*index]) {
                let dx = f64::from(side[1][0] - side[0][0]);
                let dz = f64::from(side[1][1] - side[0][1]);
                let length = dx.hypot(dz).max(0.);
                walls.push(EngineeringStructure {
                    id: format!("{}.retaining-wall.{}", operation.id, walls.len()),
                    kind: "retaining_wall".into(),
                    entity_id: operation.entity_id.clone(),
                    source_operation_id: operation.id.clone(),
                    start: side[0],
                    end: side[1],
                    height_m: height,
                    length_m: length as f32,
                    thickness_m: thickness,
                    volume_m3: (length * f64::from(height) * f64::from(thickness)) as f32,
                    material: "stone".into(),
                    status: "generated".into(),
                });
            }
        }
    }
    let mut culverts = Vec::new();
    if policy.drainage_structures {
        for crossing in crossings {
            let diameter = (crossing.width_m as f32 / 8.).clamp(0.6, 3.0);
            culverts.push(CulvertRecord {
                id: format!("{}.culvert", crossing.road_id),
                kind: "culvert".into(),
                road_id: crossing.road_id.clone(),
                water_id: crossing.water_id.clone(),
                span: crossing.span,
                length_m: crossing.length_m,
                wet_cells: crossing.wet_cells,
                invert_mm: crossing.bed_min_mm,
                deck_mm: crossing.deck_level_mm,
                diameter_m: diameter,
                status: "generated".into(),
            });
        }
    }
    if walls.len() + culverts.len() > policy.max_structures {
        failures.push(format!(
            "terrain works budget exceeded: {} structures > {}",
            walls.len() + culverts.len(),
            policy.max_structures
        ));
    }
    Some(TerrainWorksPlan {
        format: "glyphweave.terrain-works".into(),
        version: 1,
        status: if failures.is_empty() { "pass" } else { "fail" }.into(),
        policy: "retaining-walls-and-culverts-v1".into(),
        retaining_walls: walls,
        culverts,
        failures,
    })
}

/// Independent reconstruction of the published works from the same operations,
/// natural/final rasters and crossing records. A hand-edited wall or culvert is
/// refused rather than trusted.
#[allow(clippy::too_many_arguments)]
pub fn verify(
    published: &TerrainWorksPlan,
    operations: &[EngineeredTerrainOperation],
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    crossings: &[WaterCrossing],
    policy: Option<WorksPolicy>,
) -> Result<(), String> {
    let expected = plan(operations, natural, final_terrain, crossings, policy)
        .ok_or("terrain works were published without an enabled policy")?;
    if serde_json::to_value(published).map_err(|e| e.to_string())?
        != serde_json::to_value(&expected).map_err(|e| e.to_string())?
    {
        return Err("terrain works differ from independent reconstruction".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{height_payload, worldgen::SceneSpec};

    fn terrain(value: i16) -> BakedTerrain {
        let scene = SceneSpec {
            scene_id: "works".into(),
            width_m: 64,
            depth_m: 64,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                0,
                0,
                64,
                64,
                &height_payload::encode_millimetres(64, 64, vec![f64::from(value) / 1000.; 64 * 64])
                    .unwrap(),
            )
            .unwrap();
        terrain
    }
    fn operation(kind: &str) -> EngineeredTerrainOperation {
        EngineeredTerrainOperation {
            road_profile: None,
            route_grade: None,
            id: "op".into(),
            kind: kind.into(),
            entity_id: "entity".into(),
            center_x: 32.,
            center_z: 32.,
            half_width_m: 8.,
            half_depth_m: 6.,
            rotation_y_deg: 0.,
            target_height_m: 2.,
            blend_m: 3.,
            priority: 1,
            status: "applied-to-final-heightfield".into(),
        }
    }
    fn policy() -> WorksPolicy {
        WorksPolicy {
            retaining_walls: true,
            ..Default::default()
        }
    }

    #[test]
    fn a_raised_pad_gets_four_walls_with_volume() {
        let natural = terrain(0);
        let final_terrain = terrain(2000);
        let plan = plan(&[operation("building_or_parcel_pad")], &natural, &final_terrain, &[], Some(policy()))
            .unwrap();
        assert_eq!(plan.status, "pass");
        assert_eq!(plan.retaining_walls.len(), 4);
        assert!(plan.retaining_walls.iter().all(|wall| wall.height_m == 2.));
        assert!(plan.retaining_walls.iter().all(|wall| wall.volume_m3 > 0.));
    }

    #[test]
    fn a_fill_above_the_declared_wall_cap_is_a_named_failure() {
        let natural = terrain(0);
        let final_terrain = terrain(7000);
        let plan = plan(&[operation("building_or_parcel_pad")], &natural, &final_terrain, &[], Some(policy()))
            .unwrap();
        assert_eq!(plan.status, "fail");
        assert!(plan.failures.iter().any(|failure| failure.contains("exceeds maximum")));
        assert!(plan.retaining_walls.is_empty());
    }

    #[test]
    fn absent_policy_is_a_no_op() {
        assert!(plan(&[], &terrain(0), &terrain(1000), &[], None).is_none());
        assert!(policy_from_style(&serde_json::json!({})).unwrap().is_none());
        assert!(policy_from_style(&serde_json::json!({
            "terrainWorks": {"retainingWalls": "auto", "maxWallHeightMm": 9000}
        }))
        .is_err());
    }

    #[test]
    fn a_tampered_wall_or_culvert_is_refused() {
        let natural = terrain(0);
        let final_terrain = terrain(2000);
        let ops = [operation("building_or_parcel_pad")];
        let policy = policy();
        let mut published =
            plan(&ops, &natural, &final_terrain, &[], Some(policy)).unwrap();
        verify(
            &published,
            &ops,
            &natural,
            &final_terrain,
            &[],
            Some(policy),
        )
        .unwrap();
        published.retaining_walls[0].height_m += 0.1;
        assert!(
            verify(
                &published,
                &ops,
                &natural,
                &final_terrain,
                &[],
                Some(policy)
            )
            .is_err()
        );
    }
}
