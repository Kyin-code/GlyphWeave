//! Natural raster provenance and bounded drainage routing.
//!
//! This is a sampled steepest-descent model, not hydraulic erosion or a flood
//! prediction. Closed/flat terminals are preserved and exposed, never invented
//! as ocean outlets. All areas are horizontal square metres, not discharge.

use crate::baked_terrain::{BakedTerrain, TerrainError};
use crate::worldgen::SceneSpec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const DRAINAGE_MODEL: &str = "sampled-steepest-descent-preserve-sinks-v2";
const SPACING: u32 = 32;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NaturalTerrainSnapshot {
    pub format: String,
    pub version: u32,
    pub model: String,
    pub heightfield: String,
    #[serde(default)]
    pub heightfield_hash: String,
    pub water_model: String,
    pub drainage_model: String,
    pub erosion_model: String,
    pub source_classification: String,
    pub status: String,
    pub seed: u64,
    pub scene_id: String,
    pub width_m: u32,
    pub depth_m: u32,
    #[serde(default)]
    pub origin_x: i32,
    #[serde(default)]
    pub origin_z: i32,
    pub drainage: DrainageGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrainageGraph {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub sample_spacing_m: u32,
    #[serde(default)]
    pub scene_area_m2: u64,
    pub nodes: Vec<DrainageNode>,
    pub edges: Vec<DrainageEdge>,
    pub basins: Vec<DrainageBasin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrainageNode {
    pub id: String,
    pub kind: String,
    pub world_x: f64,
    pub world_z: f64,
    pub elevation_m: Option<f32>,
    pub flow_accumulation: u32,
    #[serde(default)]
    pub cell_area_m2: u64,
    #[serde(default)]
    pub contributing_area_m2: u64,
    #[serde(default)]
    pub basin_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrainageEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    /// Historical field name: describes contributing AREA class, not m3/s.
    pub discharge_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrainageBasin {
    pub id: String,
    pub status: String,
    pub area_m2: Option<f32>,
    #[serde(default)]
    pub terminal_node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NaturalTerrainAudit {
    pub scope: String,
    pub status: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub basin_count: usize,
    pub invalid_edges: usize,
    pub invalid_samples: usize,
    /// Legacy field name: count of edges that climb or remain level.
    pub descending_edges: usize,
    pub cycles: usize,
    pub integrity_errors: Vec<String>,
    pub boundary_outlet_count: usize,
    pub closed_sink_count: usize,
    pub unresolved_flat_count: usize,
    pub source_area_m2: u64,
    pub terminal_area_m2: u64,
}

fn area_class(area: u64) -> &'static str {
    match area {
        0..=4096 => "headwater_area",
        4097..=32768 => "tributary_area",
        _ => "main_channel_area",
    }
}

impl NaturalTerrainSnapshot {
    /// Uncomputed compatibility constructor; it cannot pass the new audit.
    /// Water footprints remain a separate world contract, never fake graph edges.
    pub fn authored_water(
        seed: u64,
        scene_id: &str,
        width_m: u32,
        depth_m: u32,
        _water_kind: &str,
        _water_x: f32,
        _water_z: f32,
    ) -> Self {
        Self {
            format: "glyphweave.natural-terrain".into(),
            version: 2,
            model: "procedural-macro-relief-legacy".into(),
            heightfield: "chunk-descriptor.naturalHeightFile".into(),
            water_model: "separate-world-footprints-not-routed-hydrology".into(),
            drainage_model: DRAINAGE_MODEL.into(),
            erosion_model: "deterministic-valley-carving-approximation".into(),
            source_classification: "local-implementation-provenance-review-pending".into(),
            status: "uncomputed".into(),
            seed,
            scene_id: scene_id.into(),
            width_m,
            depth_m,
            ..Self::default()
        }
    }

    pub fn with_derived_drainage(
        mut self,
        terrain: &BakedTerrain,
        scene: &SceneSpec,
    ) -> Result<Self, TerrainError> {
        self.drainage = derive_drainage_graph(terrain, scene)?;
        self.heightfield_hash = terrain.content_hash()?;
        self.origin_x = scene.origin_x;
        self.origin_z = scene.origin_z;
        self.status = "sampled-routing-with-explicit-terminals".into();
        Ok(self)
    }
}

/// No resampling from a nearest chunk, zero fallback, or NaN sentinel.
/// Strict downhill routing is acyclic; flat/depression treatment is explicit.
pub fn derive_drainage_graph(
    terrain: &BakedTerrain,
    scene: &SceneSpec,
) -> Result<DrainageGraph, TerrainError> {
    if !terrain.matches_scene(scene) {
        return Err(TerrainError("drainage scene/raster mismatch".into()));
    }
    terrain.require_complete()?;
    let cols = scene.width_m.div_ceil(SPACING) as usize;
    let rows = scene.depth_m.div_ceil(SPACING) as usize;
    let mut nodes = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            let width = (scene.width_m - col as u32 * SPACING).min(SPACING);
            let depth = (scene.depth_m - row as u32 * SPACING).min(SPACING);
            // Sampling near the centre of each partition cell includes partial
            // edge cells. Represented area, not sample count, drives totals.
            let x = scene.origin_x + (col as u32 * SPACING + width / 2) as i32;
            let z = scene.origin_z + (row as u32 * SPACING + depth / 2) as i32;
            let height = terrain
                .height(x, z)
                .ok_or_else(|| TerrainError("missing drainage sample".into()))?;
            let area = u64::from(width) * u64::from(depth);
            nodes.push(DrainageNode {
                id: format!("sample.{}", nodes.len()),
                kind: "sampled_cell".into(),
                world_x: f64::from(x),
                world_z: f64::from(z),
                elevation_m: Some(height),
                flow_accumulation: 1,
                cell_area_m2: area,
                contributing_area_m2: area,
                basin_id: String::new(),
            });
        }
    }
    let mut next = vec![None; nodes.len()];
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            let mut best_grade = 0.0_f64;
            let mut flat_neighbor = false;
            for dz in -1_isize..=1 {
                for dx in -1_isize..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let (c, r) = (col as isize + dx, row as isize + dz);
                    if c < 0 || r < 0 || c >= cols as isize || r >= rows as isize {
                        continue;
                    }
                    let j = r as usize * cols + c as usize;
                    let drop =
                        f64::from(nodes[i].elevation_m.unwrap() - nodes[j].elevation_m.unwrap());
                    flat_neighbor |= drop == 0.0;
                    let distance = (f64::from(nodes[i].world_x) - f64::from(nodes[j].world_x))
                        .hypot(f64::from(nodes[i].world_z) - f64::from(nodes[j].world_z));
                    let grade = drop / distance;
                    if grade > best_grade {
                        best_grade = grade;
                        next[i] = Some(j);
                    }
                }
            }
            if next[i].is_none() {
                nodes[i].kind = if row == 0 || col == 0 || row + 1 == rows || col + 1 == cols {
                    "boundary_outlet"
                } else if flat_neighbor {
                    "unresolved_flat"
                } else {
                    "closed_sink"
                }
                .into();
            }
        }
    }
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        nodes[b]
            .elevation_m
            .unwrap()
            .total_cmp(&nodes[a].elevation_m.unwrap())
            .then(a.cmp(&b))
    });
    for &i in &order {
        if let Some(j) = next[i] {
            nodes[j].flow_accumulation += nodes[i].flow_accumulation;
            nodes[j].contributing_area_m2 += nodes[i].contributing_area_m2;
        }
    }
    let mut basins = Vec::new();
    for &i in order.iter().rev() {
        nodes[i].basin_id = if let Some(j) = next[i] {
            nodes[j].basin_id.clone()
        } else {
            let id = format!("basin.{i}");
            basins.push(DrainageBasin {
                id: id.clone(),
                status: nodes[i].kind.clone(),
                area_m2: Some(nodes[i].contributing_area_m2 as f32),
                terminal_node_id: nodes[i].id.clone(),
            });
            id
        };
    }
    basins.sort_by(|a, b| a.id.cmp(&b.id));
    let edges = next
        .iter()
        .enumerate()
        .filter_map(|(i, &j)| {
            j.map(|j| DrainageEdge {
                id: format!("edge.{i}"),
                from: nodes[i].id.clone(),
                to: nodes[j].id.clone(),
                kind: "surface_flow".into(),
                discharge_class: area_class(nodes[i].contributing_area_m2).into(),
            })
        })
        .collect();
    Ok(DrainageGraph {
        format: "glyphweave.drainage-graph".into(),
        version: 2,
        status: DRAINAGE_MODEL.into(),
        sample_spacing_m: SPACING,
        scene_area_m2: u64::from(scene.width_m) * u64::from(scene.depth_m),
        nodes,
        edges,
        basins,
    })
}

/// O((V+E) log V), without repeatedly scanning every edge for each upstream cell.
/// Verifies data integrity only; explicit unresolved flats are NOT flood safety.
pub fn audit_drainage_graph(graph: &DrainageGraph) -> NaturalTerrainAudit {
    let mut audit = NaturalTerrainAudit {
        scope: "sampled-routing-integrity-not-hydraulic-safety".into(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        basin_count: graph.basins.len(),
        ..NaturalTerrainAudit::default()
    };
    let mut errors = BTreeSet::new();
    if graph.nodes.is_empty()
        || graph.basins.is_empty()
        || graph.scene_area_m2 == 0
        || graph.format != "glyphweave.drainage-graph"
        || graph.version != 2
        || graph.sample_spacing_m != SPACING
        || graph.status != DRAINAGE_MODEL
    {
        errors.insert("invalid_or_empty_graph_contract");
    }
    let mut ids = BTreeMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        if node.id.is_empty() || ids.insert(node.id.as_str(), i).is_some() {
            errors.insert("duplicate_or_empty_node_id");
        }
        if !node.world_x.is_finite()
            || !node.world_z.is_finite()
            || node.elevation_m.is_none_or(|h| !h.is_finite())
            || node.cell_area_m2 == 0
            || node.cell_area_m2 > u64::from(SPACING * SPACING)
        {
            audit.invalid_samples += 1;
        }
    }
    let mut next = vec![None; graph.nodes.len()];
    let mut indegree = vec![0usize; graph.nodes.len()];
    let mut edge_ids = BTreeSet::new();
    for edge in &graph.edges {
        if edge.id.is_empty() || !edge_ids.insert(&edge.id) {
            errors.insert("duplicate_or_empty_edge_id");
        }
        let (Some(&i), Some(&j)) = (ids.get(edge.from.as_str()), ids.get(edge.to.as_str())) else {
            audit.invalid_edges += 1;
            continue;
        };
        if next[i].is_some() || i == j || edge.kind != "surface_flow" {
            audit.invalid_edges += 1;
        }
        if let (Some(a), Some(b)) = (graph.nodes[i].elevation_m, graph.nodes[j].elevation_m) {
            if b >= a {
                audit.descending_edges += 1;
            }
        }
        if next[i].is_none() {
            next[i] = Some(j);
            indegree[j] += 1;
        }
        if edge.discharge_class != area_class(graph.nodes[i].contributing_area_m2) {
            errors.insert("area_class_mismatch");
        }
    }
    let mut queue: VecDeque<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| (v == 0).then_some(i))
        .collect();
    let mut counts = vec![1u64; graph.nodes.len()];
    let mut areas: Vec<u64> = graph.nodes.iter().map(|n| n.cell_area_m2).collect();
    let mut order = Vec::new();
    while let Some(i) = queue.pop_front() {
        order.push(i);
        if let Some(j) = next[i] {
            counts[j] = counts[j].saturating_add(counts[i]);
            areas[j] = areas[j].saturating_add(areas[i]);
            indegree[j] -= 1;
            if indegree[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    audit.cycles = graph.nodes.len() - order.len();
    let mut terminals = vec![None; graph.nodes.len()];
    for &i in order.iter().rev() {
        terminals[i] = next[i].map_or(Some(i), |j| terminals[j]);
    }
    let mut basin_map = BTreeMap::new();
    let mut basin_terminals = BTreeSet::new();
    for basin in &graph.basins {
        if basin.id.is_empty() || basin_map.insert(basin.id.as_str(), basin).is_some() {
            errors.insert("duplicate_or_empty_basin_id");
        }
        if !basin_terminals.insert(&basin.terminal_node_id) {
            errors.insert("duplicate_basin_terminal");
        }
        let Some(&terminal) = ids.get(basin.terminal_node_id.as_str()) else {
            errors.insert("missing_basin_terminal");
            continue;
        };
        if next[terminal].is_some()
            || basin.status != graph.nodes[terminal].kind
            || basin.area_m2.is_none_or(|a| {
                !a.is_finite() || (f64::from(a) - areas[terminal] as f64).abs() > 4.0
            })
        {
            errors.insert("invalid_basin_terminal_or_area");
        }
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        audit.source_area_m2 = audit.source_area_m2.saturating_add(node.cell_area_m2);
        if counts[i] != u64::from(node.flow_accumulation) || areas[i] != node.contributing_area_m2 {
            errors.insert("accumulation_mismatch");
        }
        if next[i].is_none() {
            audit.terminal_area_m2 = audit.terminal_area_m2.saturating_add(areas[i]);
            match node.kind.as_str() {
                "boundary_outlet" => audit.boundary_outlet_count += 1,
                "closed_sink" => audit.closed_sink_count += 1,
                "unresolved_flat" => audit.unresolved_flat_count += 1,
                _ => {
                    errors.insert("undeclared_terminal");
                }
            }
        } else if node.kind != "sampled_cell" {
            errors.insert("terminal_has_downstream");
        }
        match (basin_map.get(node.basin_id.as_str()), terminals[i]) {
            (Some(basin), Some(t)) if basin.terminal_node_id == graph.nodes[t].id => {}
            _ => {
                errors.insert("basin_membership_mismatch");
            }
        }
    }
    if audit.source_area_m2 != graph.scene_area_m2 || audit.terminal_area_m2 != graph.scene_area_m2
    {
        errors.insert("contributing_area_not_conserved");
    }
    audit.integrity_errors = errors.into_iter().map(str::to_owned).collect();
    audit.status = if audit.integrity_errors.is_empty()
        && audit.invalid_edges == 0
        && audit.invalid_samples == 0
        && audit.descending_edges == 0
        && audit.cycles == 0
    {
        "pass"
    } else {
        "fail"
    }
    .into();
    audit
}

/// Compare the full serialized graph and full-resolution source identity with
/// a fresh derivation. Never trusts the old audit's success field.
pub fn verify_snapshot(
    snapshot: &NaturalTerrainSnapshot,
    terrain: &BakedTerrain,
    scene: &SceneSpec,
) -> Result<NaturalTerrainAudit, TerrainError> {
    if snapshot.version != 2
        || snapshot.format != "glyphweave.natural-terrain"
        || snapshot.scene_id != scene.scene_id
        || snapshot.width_m != scene.width_m
        || snapshot.depth_m != scene.depth_m
        || snapshot.origin_x != scene.origin_x
        || snapshot.origin_z != scene.origin_z
        || snapshot.drainage_model != DRAINAGE_MODEL
    {
        return Err(TerrainError("natural snapshot metadata mismatch".into()));
    }
    if snapshot.heightfield_hash != terrain.content_hash()? {
        return Err(TerrainError("natural heightfield hash mismatch".into()));
    }
    let expected = derive_drainage_graph(terrain, scene)?;
    if snapshot.drainage != expected {
        return Err(TerrainError(
            "natural drainage graph differs from raster derivation".into(),
        ));
    }
    let audit = audit_drainage_graph(&expected);
    if audit.status != "pass" {
        return Err(TerrainError(format!(
            "natural graph integrity failed: {:?}",
            audit.integrity_errors
        )));
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::STREAM_CHUNK_METERS;

    fn fixture(
        width: u32,
        depth: u32,
        x: i32,
        z: i32,
        height: impl Fn(u32, u32) -> i16,
    ) -> (SceneSpec, BakedTerrain) {
        let scene = SceneSpec {
            scene_id: "natural".into(),
            width_m: width,
            depth_m: depth,
            origin_x: x,
            origin_z: z,
            seed_offset: 0,
        };
        let mut raster = BakedTerrain::new(&scene).unwrap();
        for cz in (0..depth).step_by(STREAM_CHUNK_METERS as usize) {
            for cx in (0..width).step_by(STREAM_CHUNK_METERS as usize) {
                let w = (width - cx).min(STREAM_CHUNK_METERS);
                let d = (depth - cz).min(STREAM_CHUNK_METERS);
                let mut bytes = Vec::with_capacity((w * d * 2) as usize);
                for dz in 0..d {
                    for dx in 0..w {
                        bytes.extend_from_slice(&height(cx + dx, cz + dz).to_le_bytes());
                    }
                }
                raster
                    .insert_chunk(x + cx as i32, z + cz as i32, w, d, &bytes)
                    .unwrap();
            }
        }
        (scene, raster)
    }

    #[test]
    fn missing_mismatched_or_empty_terrain_fails_before_routing() {
        let (scene, raster) = fixture(96, 96, 0, 0, |_, _| 0);
        assert!(derive_drainage_graph(&BakedTerrain::new(&scene).unwrap(), &scene).is_err());
        let mut other = scene.clone();
        other.origin_x = 20;
        assert!(derive_drainage_graph(&raster, &other).is_err());
        assert_eq!(
            audit_drainage_graph(&DrainageGraph::default()).status,
            "fail"
        );
    }

    #[test]
    fn partial_edge_cells_conserve_area_and_have_real_terminals() {
        let (scene, raster) = fixture(65, 35, -999, 777, |x, _| (100 - x) as i16);
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        let a = audit_drainage_graph(&g);
        assert_eq!(a.status, "pass", "{a:?}");
        assert_eq!(a.source_area_m2, 65 * 35);
        assert_eq!(a.terminal_area_m2, 65 * 35);
        assert_eq!(g.nodes.len(), 6);
        assert_eq!(g.nodes.iter().map(|n| n.cell_area_m2).sum::<u64>(), 65 * 35);
        assert!(!g.nodes.iter().any(|n| n.id == "outlet.boundary"));
    }

    #[test]
    fn flats_and_internal_depressions_are_explicit_not_fake_outlets() {
        let (scene, raster) = fixture(96, 96, 0, 0, |x, z| {
            if (32..64).contains(&x) && (32..64).contains(&z) {
                0
            } else {
                40
            }
        });
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        let a = audit_drainage_graph(&g);
        assert_eq!(a.status, "pass");
        assert_eq!(a.closed_sink_count, 1);
        assert_eq!(a.boundary_outlet_count, 0);
        assert_eq!(g.basins.len(), 1);
        assert_eq!(g.nodes[4].contributing_area_m2, 96 * 96);
        let (scene, raster) = fixture(96, 96, 0, 0, |_, _| 0);
        let a = audit_drainage_graph(&derive_drainage_graph(&raster, &scene).unwrap());
        assert_eq!(a.unresolved_flat_count, 1);
        assert_eq!(a.boundary_outlet_count, 8);
        assert_eq!(a.terminal_area_m2, 96 * 96);
    }

    #[test]
    fn chooses_steepest_grade_not_lowest_diagonal() {
        let (scene, raster) = fixture(96, 96, 0, 0, |x, z| match (x / 32, z / 32) {
            (1, 1) => 40,
            (2, 1) => 24,
            (2, 2) => 20,
            _ => 80,
        });
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        assert_eq!(
            g.edges.iter().find(|e| e.from == "sample.4").unwrap().to,
            "sample.5"
        );
    }

    #[test]
    fn corruption_of_graph_ids_flow_basins_and_elevations_is_rejected() {
        let (scene, raster) = fixture(96, 96, 0, 0, |x, z| (1000 - x - z) as i16);
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        let assert_bad = |bad: &DrainageGraph| assert_eq!(audit_drainage_graph(bad).status, "fail");
        let mut b = g.clone();
        b.nodes[0].flow_accumulation += 1;
        assert_bad(&b);
        let mut b = g.clone();
        b.nodes[0].elevation_m = Some(f32::NAN);
        assert_bad(&b);
        let mut b = g.clone();
        b.nodes[0].id = b.nodes[1].id.clone();
        assert_bad(&b);
        let mut b = g.clone();
        b.edges[0].to = "missing".into();
        assert_bad(&b);
        let mut b = g.clone();
        b.edges.push(b.edges[0].clone());
        assert_bad(&b);
        let mut b = g.clone();
        b.edges[0].to = b.edges[0].from.clone();
        assert_bad(&b);
        let mut b = g.clone();
        b.basins[0].area_m2 = Some(1.0);
        assert_bad(&b);
        let mut b = g.clone();
        b.nodes[0].basin_id = "missing".into();
        assert_bad(&b);
    }

    #[test]
    fn snapshot_binds_full_resolution_not_just_sampled_cells() {
        let (scene, raster) = fixture(96, 96, -100, 200, |x, z| (1000 - x - z) as i16);
        let snapshot =
            NaturalTerrainSnapshot::authored_water(42, &scene.scene_id, 96, 96, "none", 0., 0.)
                .with_derived_drainage(&raster, &scene)
                .unwrap();
        verify_snapshot(&snapshot, &raster, &scene).unwrap();
        let (_, tampered) = fixture(96, 96, -100, 200, |x, z| {
            if x == 0 && z == 0 {
                9999
            } else {
                (1000 - x - z) as i16
            }
        });
        assert_eq!(
            derive_drainage_graph(&raster, &scene).unwrap(),
            derive_drainage_graph(&tampered, &scene).unwrap()
        );
        assert!(verify_snapshot(&snapshot, &tampered, &scene).is_err());
        let mut altered = snapshot.clone();
        altered.drainage.nodes[0].flow_accumulation += 1;
        assert!(verify_snapshot(&altered, &raster, &scene).is_err());
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let decoded: NaturalTerrainSnapshot = serde_json::from_slice(&encoded).unwrap();
        verify_snapshot(&decoded, &raster, &scene).unwrap();
    }

    #[test]
    fn large_origins_do_not_collapse_adjacent_sample_positions() {
        let (scene, raster) = fixture(96, 96, i32::MAX - 100, i32::MIN + 100, |x, z| {
            (1000 - x - z) as i16
        });
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        assert_eq!(g.nodes[1].world_x - g.nodes[0].world_x, 32.0);
        assert_eq!(audit_drainage_graph(&g).status, "pass");
    }

    #[test]
    fn maximum_scene_routing_has_bounded_nodes_and_conserves_area() {
        let (scene, raster) = fixture(6000, 10000, -3000, -5000, |x, z| ((x + z) / 2) as i16);
        let g = derive_drainage_graph(&raster, &scene).unwrap();
        let a = audit_drainage_graph(&g);
        assert_eq!(g.nodes.len(), 188 * 313);
        assert_eq!(a.status, "pass", "{a:?}");
        assert_eq!(a.terminal_area_m2, 60_000_000);
        assert_eq!(a.source_area_m2, 60_000_000);
    }
}
