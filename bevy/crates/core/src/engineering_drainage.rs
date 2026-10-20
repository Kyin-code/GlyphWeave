//! C-E2: compare the natural and post-engineering drainage graphs.
//!
//! This is a routing-integrity comparison, not a flood or discharge model. It
//! answers a narrower, verifiable question: did engineered terrain create,
//! remove or reroute a sampled flow path? The result is published next to the
//! engineered terrain so a consumer can explain the impact instead of treating
//! the final heightfield as if it were still natural.
use crate::natural::{DrainageGraph, audit_drainage_graph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DrainageNodeChange {
    pub node_id: String,
    pub natural_kind: String,
    pub engineered_kind: String,
    pub natural_to: Option<String>,
    pub engineered_to: Option<String>,
    pub natural_elevation_mm: Option<i64>,
    pub engineered_elevation_mm: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineeringDrainageImpact {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub scope: String,
    pub natural_model: String,
    pub engineered_model: String,
    pub natural_nodes: usize,
    pub engineered_nodes: usize,
    pub natural_edges: usize,
    pub engineered_edges: usize,
    pub new_sinks: Vec<String>,
    pub lost_sinks: Vec<String>,
    pub changed_flows: Vec<DrainageNodeChange>,
    pub raised_terminals: Vec<DrainageNodeChange>,
    pub failures: Vec<String>,
}

fn terminal(node_kind: &str) -> bool {
    matches!(
        node_kind,
        "boundary_outlet" | "unresolved_flat" | "closed_sink"
    )
}

fn mm(elevation: Option<f32>) -> Option<i64> {
    elevation.map(|value| (f64::from(value) * 1_000.).round() as i64)
}

/// Deterministic comparison keyed by the shared sample-node ids. Nothing is
/// inferred from node order, so a graph that is merely reordered is a no-op.
pub fn compare(natural: &DrainageGraph, engineered: &DrainageGraph) -> EngineeringDrainageImpact {
    let natural_audit = audit_drainage_graph(natural);
    let engineered_audit = audit_drainage_graph(engineered);
    let natural_nodes: BTreeMap<&str, _> =
        natural.nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let engineered_nodes: BTreeMap<&str, _> = engineered
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let natural_to: BTreeMap<&str, &str> = natural
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    let engineered_to: BTreeMap<&str, &str> = engineered
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    let natural_sinks: BTreeSet<&str> = natural
        .nodes
        .iter()
        .filter(|node| terminal(&node.kind))
        .map(|node| node.id.as_str())
        .collect();
    let engineered_sinks: BTreeSet<&str> = engineered
        .nodes
        .iter()
        .filter(|node| terminal(&node.kind))
        .map(|node| node.id.as_str())
        .collect();
    let new_sinks = engineered_sinks
        .difference(&natural_sinks)
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let lost_sinks = natural_sinks
        .difference(&engineered_sinks)
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let mut changed_flows = Vec::new();
    let mut raised_terminals = Vec::new();
    for (id, engineered_node) in &engineered_nodes {
        let Some(natural_node) = natural_nodes.get(id) else {
            continue;
        };
        let n_to = natural_to.get(id).copied();
        let e_to = engineered_to.get(id).copied();
        if n_to != e_to {
            changed_flows.push(DrainageNodeChange {
                node_id: (*id).to_string(),
                natural_kind: natural_node.kind.clone(),
                engineered_kind: engineered_node.kind.clone(),
                natural_to: n_to.map(str::to_string),
                engineered_to: e_to.map(str::to_string),
                natural_elevation_mm: mm(natural_node.elevation_m),
                engineered_elevation_mm: mm(engineered_node.elevation_m),
            });
        }
        if terminal(&engineered_node.kind) {
            let natural_e = mm(natural_node.elevation_m).unwrap_or(0);
            let engineered_e = mm(engineered_node.elevation_m).unwrap_or(0);
            if engineered_e - natural_e > 50 {
                raised_terminals.push(DrainageNodeChange {
                    node_id: (*id).to_string(),
                    natural_kind: natural_node.kind.clone(),
                    engineered_kind: engineered_node.kind.clone(),
                    natural_to: n_to.map(str::to_string),
                    engineered_to: e_to.map(str::to_string),
                    natural_elevation_mm: Some(natural_e),
                    engineered_elevation_mm: Some(engineered_e),
                });
            }
        }
    }
    changed_flows.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    raised_terminals.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let mut failures = Vec::new();
    if natural_audit.status != "pass" {
        failures.push("natural drainage graph does not pass its own audit".into());
    }
    if engineered_audit.status != "pass" {
        failures.push("engineered drainage graph does not pass its own audit".into());
    }
    let status = if failures.is_empty() {
        "pass"
    } else {
        "fail"
    }
    .to_string();
    EngineeringDrainageImpact {
        format: "glyphweave.engineering-drainage-impact".into(),
        version: 1,
        status,
        scope: "sampled-routing-integrity-not-hydraulic-safety".into(),
        natural_model: natural.status.clone(),
        engineered_model: engineered.status.clone(),
        natural_nodes: natural.nodes.len(),
        engineered_nodes: engineered.nodes.len(),
        natural_edges: natural.edges.len(),
        engineered_edges: engineered.edges.len(),
        new_sinks,
        lost_sinks,
        changed_flows,
        raised_terminals,
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::natural::{DrainageBasin, DrainageEdge, DrainageNode};

    fn node(id: &str, kind: &str, elevation: f32) -> DrainageNode {
        DrainageNode {
            id: id.into(),
            kind: kind.into(),
            world_x: 0.,
            world_z: 0.,
            elevation_m: Some(elevation),
            flow_accumulation: 1,
            cell_area_m2: 1024,
            contributing_area_m2: 1024,
            basin_id: String::new(),
        }
    }
    fn edge(from: &str, to: &str) -> DrainageEdge {
        DrainageEdge {
            id: format!("{from}->{to}"),
            from: from.into(),
            to: to.into(),
            kind: "surface_flow".into(),
            discharge_class: "headwater_area".into(),
        }
    }
    fn graph(
        nodes: Vec<DrainageNode>,
        edges: Vec<DrainageEdge>,
        basins: Vec<DrainageBasin>,
    ) -> DrainageGraph {
        DrainageGraph {
            format: "glyphweave.drainage-graph".into(),
            version: 2,
            status: crate::natural::DRAINAGE_MODEL.into(),
            sample_spacing_m: 32,
            scene_area_m2: 1024 * nodes.len() as u64,
            nodes,
            edges,
            basins,
        }
    }

    fn valid_pair(engineered_to: &str, engineered_sink_elevation: f32) -> (DrainageGraph, DrainageGraph) {
        let mut natural_a = node("a", "sampled_cell", 10.);
        natural_a.basin_id = "basin.b".into();
        let mut natural_b = node("b", "boundary_outlet", 8.);
        natural_b.flow_accumulation = 2;
        natural_b.contributing_area_m2 = 2048;
        natural_b.basin_id = "basin.b".into();
        let mut natural_c = node("c", "closed_sink", 7.);
        natural_c.basin_id = "basin.c".into();
        let natural = graph(
            vec![natural_a, natural_b, natural_c],
            vec![edge("a", "b")],
            vec![
                DrainageBasin {
                    id: "basin.b".into(),
                    status: "boundary_outlet".into(),
                    area_m2: Some(2048.),
                    terminal_node_id: "b".into(),
                },
                DrainageBasin {
                    id: "basin.c".into(),
                    status: "closed_sink".into(),
                    area_m2: Some(1024.),
                    terminal_node_id: "c".into(),
                },
            ],
        );
        let mut a = node("a", "sampled_cell", 10.);
        a.flow_accumulation = 1;
        a.contributing_area_m2 = 1024;
        a.basin_id = if engineered_to == "c" {
            "basin.c".into()
        } else {
            "basin.b".into()
        };
        let mut b = node("b", "boundary_outlet", 8.);
        if engineered_to == "c" {
            b.flow_accumulation = 1;
            b.contributing_area_m2 = 1024;
        } else {
            b.flow_accumulation = 2;
            b.contributing_area_m2 = 2048;
        }
        b.basin_id = "basin.b".into();
        let mut c = node("c", "closed_sink", engineered_sink_elevation);
        c.flow_accumulation = 2;
        c.contributing_area_m2 = 2048;
        c.basin_id = "basin.c".into();
        let engineered = graph(
            vec![a, b, c],
            vec![edge("a", engineered_to)],
            vec![
                DrainageBasin {
                    id: "basin.b".into(),
                    status: "boundary_outlet".into(),
                    area_m2: Some(if engineered_to == "c" { 1024. } else { 2048. }),
                    terminal_node_id: "b".into(),
                },
                DrainageBasin {
                    id: "basin.c".into(),
                    status: "closed_sink".into(),
                    area_m2: Some(2048.),
                    terminal_node_id: "c".into(),
                },
            ],
        );
        (natural, engineered)
    }

    #[test]
    fn rerouted_flow_is_named_and_deterministic() {
        let (natural, engineered) = valid_pair("c", 7.);
        let impact = compare(&natural, &engineered);
        assert_eq!(impact.status, "pass", "{impact:#?}");
        assert_eq!(impact.changed_flows.len(), 1);
        assert_eq!(impact.changed_flows[0].node_id, "a");
        assert_eq!(impact.changed_flows[0].engineered_to.as_deref(), Some("c"));
    }

    #[test]
    fn a_raised_terminal_is_reported() {
        let (_, engineered) = valid_pair("c", 8.2);
        let natural = valid_pair("c", 8.).0;
        let impact = compare(&natural, &engineered);
        assert_eq!(impact.raised_terminals.len(), 1, "{impact:#?}");
        assert_eq!(impact.raised_terminals[0].node_id, "c");
    }

    #[test]
    fn node_order_does_not_change_the_result() {
        let (a, _) = valid_pair("c", 7.);
        let mut b = a.clone();
        b.nodes.reverse();
        b.basins.reverse();
        assert_eq!(compare(&a, &b), compare(&a, &a));
    }
}
