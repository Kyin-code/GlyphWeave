//! Persisted, terrain-conforming pedestrian pavement, distinct from integer-centred boxes.
use super::access::{HALF_WIDTH, MAX_LENGTH, height, point};
use super::{AccessGraph, BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};

/// All vertex coordinates are world-space METRES (as is entity worldY).
/// The thin overlay never changes the terrain or becomes a routing goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessSurface {
    pub version: u32,
    pub entity_id: String,
    pub anchor_id: String,
    pub road_id: String,
    pub width_m: f64,
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

pub fn bake_access_surfaces(
    graph: &AccessGraph,
    terrain: &BakedTerrain,
) -> Result<Vec<AccessSurface>, TerrainError> {
    terrain.require_complete()?;
    let mut result = Vec::new();
    for link in graph.links.iter().filter(|link| link.connected) {
        let road = link
            .road_id
            .as_ref()
            .ok_or_else(|| TerrainError("connected route without road".into()))?;
        if link.path.len() < 2 || link.path.len() > 6002 {
            return Err(TerrainError("invalid access route point count".into()));
        }
        let mut surface = AccessSurface {
            version: 1,
            entity_id: link.entity_id.clone(),
            anchor_id: link.anchor_id.clone(),
            road_id: road.clone(),
            width_m: HALF_WIDTH * 2.,
            vertices: vec![],
            triangles: vec![],
        };
        let mut total = 0.;
        for pair in link.path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dz) = (b.x - a.x, b.z - a.z);
            let length = dx.hypot(dz);
            total += length;
            if !length.is_finite() || total > MAX_LENGTH + 1e-6 {
                return Err(TerrainError("invalid access route length".into()));
            }
            if length < 1e-8 {
                continue;
            }
            let rows = (length / 0.5).ceil() as u32;
            let base = surface.vertices.len() as u32;
            for row in 0..=rows {
                let t = f64::from(row) / f64::from(rows);
                for lateral in [-HALF_WIDTH, 0., HALF_WIDTH] {
                    let p = point(
                        a.x + dx * t - dz / length * lateral,
                        a.z + dz * t + dx / length * lateral,
                    );
                    let y = height(terrain, p).ok_or_else(|| {
                        TerrainError("access pavement outside loaded terrain".into())
                    })?;
                    surface.vertices.push([p.x, y + 0.04, p.z]);
                }
            }
            for row in 0..rows {
                for col in 0..2 {
                    let i = base + row * 3 + col;
                    // Upward winding for X-right / Y-up / Z-forward coordinates.
                    surface.triangles.push([i, i + 1, i + 3]);
                    surface.triangles.push([i + 1, i + 4, i + 3]);
                }
            }
        }
        if surface.triangles.is_empty() {
            return Err(TerrainError("empty access pavement".into()));
        }
        result.push(surface);
    }
    Ok(result)
}

/// Compare persisted geometry with a newly computed graph, NEVER scene.accessGraph.
/// Missing or edited pavements are failures, even if the path report says connected.
pub fn validate_access_surfaces(
    graph: &AccessGraph,
    terrain: &BakedTerrain,
    actual: &[AccessSurface],
) -> Result<(), TerrainError> {
    let expected = bake_access_surfaces(graph, terrain)?;
    let matches = expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(e, a)| {
            e.version == a.version
                && e.entity_id == a.entity_id
                && e.anchor_id == a.anchor_id
                && e.road_id == a.road_id
                && a.width_m.is_finite()
                && (e.width_m - a.width_m).abs() < 1e-9
                && e.triangles == a.triangles
                && e.vertices.len() == a.vertices.len()
                && e.vertices.iter().zip(&a.vertices).all(|(ev, av)| {
                    ev.iter()
                        .zip(av)
                        .all(|(x, y)| y.is_finite() && (x - y).abs() < 1e-9)
                })
        });
    if matches {
        Ok(())
    } else {
        Err(TerrainError(
            "persisted access surfaces differ from independently verified routes/terrain".into(),
        ))
    }
}
