//! GlyphWeave-original, area-driven vegetation budgets and deterministic placement.
//! Densities/stratum weights are design presets, not ecological measurements.
use super::landcover::{self, Material, SurfacePatch};
use crate::{
    baked_terrain::BakedTerrain,
    worldgen::{EntityInstance, SceneSpec, WaterGeometry},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[path = "ecology_spatial.rs"]
mod ecology_spatial;
use ecology_spatial::ObstacleQuery;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyClassSummary {
    pub class_name: String,
    pub patch_count: usize,
    pub area_m2: f64,
    pub source_cell_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyPlant {
    pub id: String,
    pub source_patch_id: String,
    pub kind: String,
    #[serde(default)]
    pub layer: String,
    pub world_x: f64,
    pub world_z: f64,
    pub world_y: f64,
    pub height_m: f64,
    pub width_m: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyLayerBudget {
    pub layer: String,
    pub kind: String,
    pub planned_count: usize,
    pub realized_count: usize,
    pub unfilled_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyCommunity {
    pub source_patch_id: String,
    pub community_kind: String,
    // Legacy planning priors, NOT measured crown-cover percentages.
    pub canopy_cover: f64,
    pub understory_cover: f64,
    pub ground_cover: f64,
    pub edge_factor: f64,
    pub hydrology_class: String,
    pub density_per_ha: f64,
    pub planned_plant_count: usize,
    pub realized_plant_count: usize,
    pub unfilled_plant_count: usize,
    pub layer_budgets: Vec<EcologyLayerBudget>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyRejection {
    pub id: String,
    pub source_patch_id: String,
    pub layer: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcologyRealization {
    pub model: String,
    pub classes: Vec<EcologyClassSummary>,
    pub total_area_m2: f64,
    pub status: String,
    #[serde(default)]
    pub plants: Vec<EcologyPlant>,
    #[serde(default)]
    pub communities: Vec<EcologyCommunity>,
    pub rejections: Vec<EcologyRejection>,
}
struct Layer {
    name: &'static str,
    kind: &'static str,
    weight: usize,
    height: f64,
    width: f64,
}
struct Profile {
    name: &'static str,
    class: &'static str,
    hydrology: &'static str,
    density: f64,
    cover: [f64; 4],
    layers: &'static [Layer],
}
fn profile(material: Material) -> Option<Profile> {
    Some(match material {
        Material::ConservationForestFloor => Profile {
            name: "forest",
            class: "forest_floor",
            hydrology: "upland",
            density: 420.,
            cover: [0.72, 0.38, 0.86, 0.45],
            layers: &[
                Layer {
                    name: "canopy",
                    kind: "tree",
                    weight: 40,
                    height: 16.,
                    width: 4.,
                },
                Layer {
                    name: "understory",
                    kind: "tree",
                    weight: 30,
                    height: 7.,
                    width: 2.5,
                },
                Layer {
                    name: "ground",
                    kind: "bush",
                    weight: 30,
                    height: 1.2,
                    width: 1.5,
                },
            ],
        },
        Material::ConservationWetland => Profile {
            name: "wetland",
            class: "wetland",
            hydrology: "wet",
            density: 280.,
            cover: [0.08, 0.62, 0.92, 0.78],
            layers: &[
                Layer {
                    name: "emergent",
                    kind: "reed",
                    weight: 70,
                    height: 2.5,
                    width: 0.8,
                },
                Layer {
                    name: "understory",
                    kind: "bush",
                    weight: 30,
                    height: 1.,
                    width: 1.,
                },
            ],
        },
        Material::ConservationGround => Profile {
            name: "shrubland",
            class: "ground",
            hydrology: "mesic",
            density: 190.,
            cover: [0.18, 0.54, 0.68, 0.58],
            layers: &[
                Layer {
                    name: "shrub",
                    kind: "bush",
                    weight: 60,
                    height: 1.2,
                    width: 1.5,
                },
                Layer {
                    name: "ground",
                    kind: "bush",
                    weight: 40,
                    height: 0.7,
                    width: 0.9,
                },
            ],
        },
        _ => return None,
    })
}
// Hamilton allocation conserves the integer patch budget; zero-count layers
// remain explicit, rather than forcing a minimum on tiny partial patches.
fn apportion(total: usize, layers: &[Layer]) -> Vec<usize> {
    let weight: usize = layers.iter().map(|l| l.weight).sum();
    let mut counts: Vec<_> = layers.iter().map(|l| total * l.weight / weight).collect();
    let remaining = total - counts.iter().sum::<usize>();
    let mut order: Vec<_> = (0..layers.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(total * layers[i].weight % weight), i));
    for &i in order.iter().take(remaining) {
        counts[i] += 1;
    }
    counts
}
fn bounds(p: &SurfacePatch) -> [f64; 4] {
    [
        p.vertices
            .iter()
            .map(|v| v[0])
            .fold(f64::INFINITY, f64::min),
        p.vertices
            .iter()
            .map(|v| v[0])
            .fold(f64::NEG_INFINITY, f64::max),
        p.vertices
            .iter()
            .map(|v| v[2])
            .fold(f64::INFINITY, f64::min),
        p.vertices
            .iter()
            .map(|v| v[2])
            .fold(f64::NEG_INFINITY, f64::max),
    ]
}
fn communities(patches: &[SurfacePatch]) -> Result<Vec<EcologyCommunity>, String> {
    let mut patches: Vec<_> = patches
        .iter()
        .filter(|p| profile(p.material).is_some())
        .collect();
    patches.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen = BTreeSet::new();
    let mut cumulative = BTreeMap::<&str, (f64, usize)>::new();
    let mut result = vec![];
    for patch in patches {
        if !seen.insert(&patch.id) {
            return Err("duplicate ecology patch".into());
        }
        let p = profile(patch.material).unwrap();
        let area = landcover::mesh_area(patch)?;
        if !area.is_finite() || area <= 0. || area > 256. + 1e-7 {
            return Err("invalid ecology patch area".into());
        }
        let entry = cumulative.entry(p.class).or_default();
        entry.0 += area;
        let cumulative_target = (entry.0 * p.density / 10000.).round() as usize;
        let count = cumulative_target - entry.1;
        entry.1 = cumulative_target;
        let counts = apportion(count, p.layers);
        result.push(EcologyCommunity {
            source_patch_id: patch.id.clone(),
            community_kind: p.name.into(),
            canopy_cover: p.cover[0],
            understory_cover: p.cover[1],
            ground_cover: p.cover[2],
            edge_factor: p.cover[3],
            hydrology_class: p.hydrology.into(),
            density_per_ha: p.density,
            planned_plant_count: count,
            realized_plant_count: 0,
            unfilled_plant_count: 0,
            layer_budgets: p
                .layers
                .iter()
                .zip(counts)
                .map(|(l, n)| EcologyLayerBudget {
                    layer: l.name.into(),
                    kind: l.kind.into(),
                    planned_count: n,
                    realized_count: 0,
                    unfilled_count: 0,
                })
                .collect(),
        });
    }
    Ok(result)
}
pub(super) fn realize(
    scene: &SceneSpec,
    patches: &[SurfacePatch],
    terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<EcologyRealization, String> {
    let obstacles = ObstacleQuery::indexed(entities);
    realize_with_obstacles(scene, patches, terrain, water, &obstacles)
}

fn realize_with_obstacles(
    scene: &SceneSpec,
    patches: &[SurfacePatch],
    terrain: &BakedTerrain,
    water: WaterGeometry,
    obstacles: &ObstacleQuery<'_>,
) -> Result<EcologyRealization, String> {
    let mut result = EcologyRealization {
        model: "area-budgeted-stratified-vegetation-v3".into(),
        communities: communities(patches)?,
        ..Default::default()
    };
    let patches: BTreeMap<_, _> = patches.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut classes = BTreeMap::<String, (usize, f64, BTreeSet<String>)>::new();
    for c in &mut result.communities {
        let patch = patches[c.source_patch_id.as_str()];
        let p = profile(patch.material).unwrap();
        let entry = classes.entry(p.class.into()).or_default();
        entry.0 += 1;
        entry.1 += landcover::mesh_area(patch)?;
        entry.2.insert(patch.source_cell_id.clone());
        let patch_bounds = bounds(patch);
        // Every accepted full-size candidate lies in this patch (including EPS).
        // Preserve the full original geometry checks and all search attempts.
        let patch_obstacles = obstacles.query(patch_bounds);
        let mut local = vec![];
        let mut rejected = vec![];
        for (budget, layer) in c.layer_budgets.iter().zip(p.layers) {
            for ordinal in 0..budget.planned_count {
                let id = format!("{}.{}.{ordinal}", patch.id, layer.name);
                let seed = blake3::hash(format!("{}:{}", scene.seed_offset, id).as_bytes());
                let bytes = seed.as_bytes();
                let u =
                    f64::from(u32::from_le_bytes(bytes[0..4].try_into().unwrap())) / 4294967296.;
                let v =
                    f64::from(u32::from_le_bytes(bytes[4..8].try_into().unwrap())) / 4294967296.;
                let [x0, x1, z0, z1] = patch_bounds;
                let preferred = [x0 + (x1 - x0) * u, z0 + (z1 - z0) * v];
                match landcover::choose_ecology_position(
                    preferred,
                    patch_bounds,
                    layer.width,
                    water,
                    &patch_obstacles,
                    &local,
                ) {
                    Ok([x, z]) => {
                        let y = landcover::terrain_height(terrain, [x, z])?;
                        local.push(EcologyPlant {
                            id,
                            source_patch_id: patch.id.clone(),
                            kind: layer.kind.into(),
                            layer: layer.name.into(),
                            world_x: x,
                            world_z: z,
                            world_y: y,
                            height_m: layer.height,
                            width_m: layer.width,
                        });
                    }
                    Err(reason) => {
                        rejected.push(EcologyRejection {
                            id,
                            source_patch_id: patch.id.clone(),
                            layer: layer.name.into(),
                            reason,
                        });
                    }
                }
            }
        }
        // A random anchor accepted early can close the patch for the instances
        // that follow it, even when the planned quota is still satisfiable on
        // the same land (A milestone exception #4: 342379/342380). Re-layout only
        // the patch that actually fell short; a patch that met its quota is
        // never touched, so every complete world keeps its exact bytes.
        if !rejected.is_empty() {
            let counts: Vec<usize> = c.layer_budgets.iter().map(|b| b.planned_count).collect();
            let relaid = relayout_patch(
                patch,
                patch_bounds,
                p.layers,
                &counts,
                terrain,
                water,
                &patch_obstacles,
            )?;
            if let Some((plants, relaid_rejected)) = relaid {
                if plants.len() > local.len() {
                    local = plants;
                    rejected = relaid_rejected;
                }
            }
        }
        for (budget, layer) in c.layer_budgets.iter_mut().zip(p.layers) {
            budget.realized_count = local
                .iter()
                .filter(|plant| plant.layer == layer.name)
                .count();
            budget.unfilled_count = rejected
                .iter()
                .filter(|rejection| rejection.layer == layer.name)
                .count();
        }
        c.realized_plant_count = local.len();
        c.unfilled_plant_count = rejected.len();
        result.plants.extend(local);
        result.rejections.extend(rejected);
    }
    result.classes = classes
        .into_iter()
        .map(
            |(class_name, (patch_count, area_m2, ids))| EcologyClassSummary {
                class_name,
                patch_count,
                area_m2,
                source_cell_ids: ids.into_iter().collect(),
            },
        )
        .collect();
    result.total_area_m2 = result.classes.iter().map(|c| c.area_m2).sum();
    result.status = if !result.rejections.is_empty() {
        "incomplete"
    } else if result.communities.is_empty() {
        "unrealized"
    } else {
        "instances-realized"
    }
    .into();
    result.plants.sort_by(|a, b| a.id.cmp(&b.id));
    result.rejections.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}
pub(super) fn overlaps(a: &EcologyPlant, b: &EcologyPlant) -> bool {
    let r = (a.width_m + b.width_m) * 0.5;
    (a.world_x - b.world_x).abs() < r - 1e-9 && (a.world_z - b.world_z).abs() < r - 1e-9
}

// A footprint-sized lattice for one layer: consecutive centres are exactly one
// footprint apart, so two instances of the same layer can never overlap, and
// every centre keeps its full footprint inside the patch (including EPS). An
// empty lattice means the patch is narrower than one footprint, which stays a
// named capacity refusal rather than an error.
fn width_lattice(bounds: [f64; 4], width: f64) -> Option<Vec<[f64; 2]>> {
    if !(width > 0.) || !bounds.iter().all(|v| v.is_finite()) {
        return None;
    }
    let [min_x, max_x, min_z, max_z] = bounds;
    let r = width * 0.5;
    let axis = |min: f64, max: f64| {
        let mut out = vec![];
        let mut position = min + r;
        while position + r <= max + 1e-9 {
            out.push(position);
            position += width;
        }
        out
    };
    let xs = axis(min_x, max_x);
    let zs = axis(min_z, max_z);
    let mut lattice = Vec::with_capacity(xs.len() * zs.len());
    for &z in &zs {
        for &x in &xs {
            lattice.push([x, z]);
        }
    }
    Some(lattice)
}

// Whole-patch re-layout used only after the anchor scan left the patch short.
// Largest footprints are placed first (a small instance can never consume the
// last cell a large one needs), each on its own footprint lattice, so the
// planned quota is realized whenever the land can physically hold it. The
// admission predicate is the same one the anchor scan and verify use; whatever
// still cannot be placed stays an explicit, named rejection.
fn relayout_patch(
    patch: &SurfacePatch,
    patch_bounds: [f64; 4],
    layers: &[Layer],
    counts: &[usize],
    terrain: &BakedTerrain,
    water: WaterGeometry,
    obstacles: &[&EntityInstance],
) -> Result<Option<(Vec<EcologyPlant>, Vec<EcologyRejection>)>, String> {
    // Out-of-domain patches keep exactly the anchor-scan outcome they had.
    if patch_bounds[1] - patch_bounds[0] > 16. + 1e-9
        || patch_bounds[3] - patch_bounds[2] > 16. + 1e-9
    {
        return Ok(None);
    }
    let mut planned: Vec<(usize, usize)> = vec![];
    for (layer_index, &count) in counts.iter().enumerate() {
        planned.extend((0..count).map(|ordinal| (layer_index, ordinal)));
    }
    planned.sort_by(|&(layer_a, ordinal_a), &(layer_b, ordinal_b)| {
        layers[layer_b]
            .width
            .total_cmp(&layers[layer_a].width)
            .then(layer_a.cmp(&layer_b))
            .then(ordinal_a.cmp(&ordinal_b))
    });
    let mut plants = vec![];
    let mut rejections = vec![];
    for (layer_index, ordinal) in planned {
        let layer = &layers[layer_index];
        let id = format!("{}.{}.{ordinal}", patch.id, layer.name);
        let Some(lattice) = width_lattice(patch_bounds, layer.width) else {
            return Ok(None);
        };
        // The id only rotates the scan start, so equal layers still fill
        // distinct cells while different instances do not stack in a row.
        let start = if lattice.is_empty() {
            0
        } else {
            let seed = blake3::hash(id.as_bytes());
            (u32::from_le_bytes(seed.as_bytes()[8..12].try_into().unwrap()) as usize)
                % lattice.len()
        };
        let mut anchor = None;
        for step in 0..lattice.len() {
            let [x, z] = lattice[(start + step) % lattice.len()];
            if !landcover::anchor_is_clear(
                patch_bounds,
                layer.width,
                x,
                z,
                water,
                obstacles,
                &plants,
            ) {
                continue;
            }
            // A sampled height that the greedy path would also refuse is not a
            // new world-wide failure: skip the cell and keep searching.
            if let Ok(y) = landcover::terrain_height(terrain, [x, z]) {
                anchor = Some([x, z, y]);
                break;
            }
        }
        match anchor {
            Some([x, z, y]) => plants.push(EcologyPlant {
                id,
                source_patch_id: patch.id.clone(),
                kind: layer.kind.into(),
                layer: layer.name.into(),
                world_x: x,
                world_z: z,
                world_y: y,
                height_m: layer.height,
                width_m: layer.width,
            }),
            None => rejections.push(EcologyRejection {
                id,
                source_patch_id: patch.id.clone(),
                layer: layer.name.into(),
                reason: landcover::NO_ECOLOGY_ANCHOR.into(),
            }),
        }
    }
    Ok(Some((plants, rejections)))
}
#[cfg(test)]
pub(super) fn verify_plant_clearance(
    plant: &EcologyPlant,
    patch: &SurfacePatch,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<(), String> {
    verify_plant_clearance_with(plant, patch, water, |x, z, r| {
        entities
            .iter()
            .filter(|e| landcover::ecology_obstacle(e))
            .any(|e| landcover::footprint_intersects_entity(e, x, z, r, r))
    })
}

fn verify_plant_clearance_with(
    plant: &EcologyPlant,
    patch: &SurfacePatch,
    water: WaterGeometry,
    blocked: impl FnOnce(f64, f64, f64) -> bool,
) -> Result<(), String> {
    if ![
        plant.world_x,
        plant.world_y,
        plant.world_z,
        plant.height_m,
        plant.width_m,
    ]
    .iter()
    .all(|v| v.is_finite())
        || plant.width_m <= 0.
        || plant.height_m <= 0.
    {
        return Err("invalid ecology plant contract".into());
    }
    let [x0, x1, z0, z1] = bounds(patch);
    let r = plant.width_m * 0.5;
    if plant.world_x - r < x0 - 1e-9
        || plant.world_x + r > x1 + 1e-9
        || plant.world_z - r < z0 - 1e-9
        || plant.world_z + r > z1 + 1e-9
    {
        return Err("ecology plant footprint is outside source patch".into());
    }
    if water.intersects_axis_aligned(plant.world_x, plant.world_z, r, r) {
        return Err("ecology plant intersects water".into());
    }
    if blocked(plant.world_x, plant.world_z, r) {
        return Err("ecology plant intersects hard entity".into());
    }
    Ok(())
}
pub(super) fn verify_contract(
    actual: &EcologyRealization,
    patches: &[SurfacePatch],
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<(), String> {
    // Rebuild independently from final entities; no generation index/cache is trusted.
    let obstacles = ObstacleQuery::indexed(entities);
    verify_contract_with_obstacles(actual, patches, water, &obstacles)
}

fn verify_contract_with_obstacles(
    actual: &EcologyRealization,
    patches: &[SurfacePatch],
    water: WaterGeometry,
    obstacles: &ObstacleQuery<'_>,
) -> Result<(), String> {
    let expected = communities(patches)?;
    if expected.len() != actual.communities.len() {
        return Err("ecology community count mismatch".into());
    }
    let patch_map: BTreeMap<_, _> = patches.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut pending = BTreeMap::new();
    for (c, e) in actual.communities.iter().zip(&expected) {
        let mut header = c.clone();
        header.realized_plant_count = 0;
        header.unfilled_plant_count = 0;
        for l in &mut header.layer_budgets {
            l.realized_count = 0;
            l.unfilled_count = 0;
        }
        if &header != e {
            return Err("ecology area density budget mismatch".into());
        }
        let realized = c
            .layer_budgets
            .iter()
            .try_fold(0usize, |sum, l| sum.checked_add(l.realized_count))
            .ok_or("ecology count overflow")?;
        let unfilled = c
            .layer_budgets
            .iter()
            .try_fold(0usize, |sum, l| sum.checked_add(l.unfilled_count))
            .ok_or("ecology count overflow")?;
        if c.realized_plant_count != realized
            || c.unfilled_plant_count != unfilled
            || realized.checked_add(unfilled) != Some(c.planned_plant_count)
        {
            return Err("ecology community count is not conserved".into());
        }
        for l in &c.layer_budgets {
            if l.realized_count.checked_add(l.unfilled_count) != Some(l.planned_count) {
                return Err("ecology layer count is not conserved".into());
            }
            for ordinal in 0..l.planned_count {
                pending.insert(
                    format!("{}.{}.{ordinal}", c.source_patch_id, l.layer),
                    (
                        c.source_patch_id.as_str(),
                        l.layer.as_str(),
                        l.kind.as_str(),
                    ),
                );
            }
        }
    }
    let mut counts = BTreeMap::<(&str, &str), (usize, usize)>::new();
    let mut by_patch = BTreeMap::<&str, Vec<&EcologyPlant>>::new();
    for plant in &actual.plants {
        let (source, layer, kind) = pending
            .remove(&plant.id)
            .ok_or("duplicate or unplanned ecology plant")?;
        if plant.source_patch_id != source || plant.layer != layer || plant.kind != kind {
            return Err("ecology plant provenance mismatch".into());
        }
        verify_plant_clearance_with(plant, patch_map[source], water, |x, z, r| {
            obstacles
                .query([x - r, x + r, z - r, z + r])
                .iter()
                .any(|e| landcover::footprint_intersects_entity(e, x, z, r, r))
        })?;
        let neighbors = by_patch.entry(source).or_default();
        if neighbors.iter().any(|p| overlaps(plant, p)) {
            return Err("ecology plants overlap".into());
        }
        neighbors.push(plant);
        counts.entry((source, layer)).or_default().0 += 1;
    }
    for rejection in &actual.rejections {
        let (source, layer, _) = pending
            .remove(&rejection.id)
            .ok_or("duplicate or unplanned ecology rejection")?;
        if rejection.source_patch_id != source
            || rejection.layer != layer
            || rejection.reason != landcover::NO_ECOLOGY_ANCHOR
        {
            return Err("invalid ecology rejection".into());
        }
        counts.entry((source, layer)).or_default().1 += 1;
    }
    if !pending.is_empty() {
        return Err("missing ecology outcome".into());
    }
    for c in &actual.communities {
        for l in &c.layer_budgets {
            let observed = counts
                .get(&(c.source_patch_id.as_str(), l.layer.as_str()))
                .copied()
                .unwrap_or_default();
            if observed != (l.realized_count, l.unfilled_count) {
                return Err("ecology summary differs from outcomes".into());
            }
        }
    }
    Ok(())
}
// Numeric tolerance is only for JSON roundtrip geometry; counts/IDs remain exact.
pub(super) fn matches(a: &EcologyRealization, b: &EcologyRealization) -> bool {
    a.model == b.model
        && a.status == b.status
        && (a.total_area_m2 - b.total_area_m2).abs() <= 1e-7
        && a.classes.len() == b.classes.len()
        && a.classes.iter().zip(&b.classes).all(|(x, y)| {
            x.class_name == y.class_name
                && x.patch_count == y.patch_count
                && (x.area_m2 - y.area_m2).abs() <= 1e-7
                && x.source_cell_ids == y.source_cell_ids
        })
        && a.plants.len() == b.plants.len()
        && a.plants.iter().zip(&b.plants).all(|(x, y)| {
            x.id == y.id
                && x.source_patch_id == y.source_patch_id
                && x.kind == y.kind
                && x.layer == y.layer
                && (x.world_x - y.world_x).abs() <= 1e-7
                && (x.world_z - y.world_z).abs() <= 1e-7
                && (x.world_y - y.world_y).abs() <= 1e-6
                && (x.height_m - y.height_m).abs() <= 1e-7
                && (x.width_m - y.width_m).abs() <= 1e-7
        })
        && a.communities == b.communities
        && a.rejections == b.rejections
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch(id: &str, x: f64, z: f64, w: f64, d: f64, material: Material) -> SurfacePatch {
        SurfacePatch {
            id: id.into(),
            source_cell_id: "source".into(),
            parcel_id: "source".into(),
            zone_kind: "nature".into(),
            material,
            area_m2: w * d,
            vertices: vec![
                [x, 4., z],
                [x, 4., z + d],
                [x + w, 4., z + d],
                [x + w, 4., z],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }
    fn fixture() -> (SceneSpec, Vec<SurfacePatch>, BakedTerrain, WaterGeometry) {
        let s = SceneSpec {
            scene_id: "ecology".into(),
            width_m: 32,
            depth_m: 32,
            origin_x: -16,
            origin_z: 32,
            seed_offset: 7,
        };
        let patches = (0..4)
            .map(|i| {
                patch(
                    &format!("patch-{i}"),
                    -16. + (i % 2) as f64 * 16.,
                    32. + (i / 2) as f64 * 16.,
                    16.,
                    16.,
                    Material::ConservationForestFloor,
                )
            })
            .collect();
        let mut terrain = BakedTerrain::new(&s).unwrap();
        terrain
            .insert_chunk(-16, 32, 32, 32, &16i16.to_le_bytes().repeat(32 * 32))
            .unwrap();
        let style = serde_json::Value::Null;
        let water =
            crate::worldgen::water_geometry(crate::worldgen::water_kind(&style), &[], &s, &style);
        (s, patches, terrain, water)
    }
    #[test]
    fn cumulative_density_preserves_class_budget_and_input_order_independence() {
        let mut patches: Vec<_> = (0..100)
            .map(|i| {
                patch(
                    &format!("p-{i:03}"),
                    i as f64 * 16.,
                    0.,
                    16.,
                    16.,
                    Material::ConservationForestFloor,
                )
            })
            .collect();
        let a = communities(&patches).unwrap();
        assert_eq!(a.iter().map(|c| c.planned_plant_count).sum::<usize>(), 1075);
        assert!(a.iter().any(|c| c.planned_plant_count == 10));
        assert!(a.iter().any(|c| c.planned_plant_count == 11));
        for c in &a {
            assert_eq!(
                c.planned_plant_count,
                c.layer_budgets
                    .iter()
                    .map(|l| l.planned_count)
                    .sum::<usize>()
            );
        }
        patches.reverse();
        assert_eq!(a, communities(&patches).unwrap());
        let wet = vec![patch(
            "wet",
            0.,
            0.,
            16.,
            16.,
            Material::ConservationWetland,
        )];
        assert_eq!(communities(&wet).unwrap()[0].planned_plant_count, 7);
        let small = vec![patch(
            "small",
            0.,
            0.,
            1.,
            1.,
            Material::ConservationForestFloor,
        )];
        let c = communities(&small).unwrap();
        assert_eq!(c[0].planned_plant_count, 0);
        assert_eq!(c[0].layer_budgets.len(), 3); // explicit zero budgets, no fake minimum
    }
    /// A-milestone exception #4: one instance of a 342380 quota stayed unfilled
    /// in the maximum world even though the patch could physically hold it. The
    /// four canopy anchors below are the published anchors of
    /// `scene-0.landcover.4563.conservation.208.32` (16x16 forest patch) from the
    /// r49 maximum world, translated into this fixture's patch-1 (0..16, 32..48).
    /// They cover every in-patch 1m anchor, so the anchor scan must refuse, and
    /// the whole-patch re-layout must still realize the 5+3+3 quota.
    #[test]
    fn published_one_plant_shortfall_geometry_relayouts_to_the_full_quota() {
        let (_, _, terrain, water) = fixture();
        let bounds = [0., 16., 32., 48.];
        let published = [
            [3.090812887996, 3.915967516601],
            [11.605730935931, 13.638830453157],
            [4.857992429286, 10.034225314856],
            [10.011684183031, 5.68160983175],
        ];
        let occupied: Vec<EcologyPlant> = published
            .iter()
            .map(|&[dx, dz]| EcologyPlant {
                world_x: bounds[0] + dx,
                world_z: bounds[2] + dz,
                width_m: 4.,
                height_m: 16.,
                ..Default::default()
            })
            .collect();
        // The observed proximate cause: every 1m anchor is already excluded by
        // the four earlier canopy instances, so the fifth cannot be placed even
        // though a 5-canopy layout exists on the same 16x16 land.
        let mut clear_anchors = 0;
        for iz in 0..=16 {
            for ix in 0..=16 {
                let x = bounds[0] + 2. + f64::from(ix);
                let z = bounds[2] + 2. + f64::from(iz);
                if landcover::anchor_is_clear(bounds, 4., x, z, water, &[], &occupied) {
                    clear_anchors += 1;
                }
            }
        }
        assert_eq!(
            clear_anchors, 0,
            "published anchors must exhaust the 1m grid"
        );
        assert_eq!(
            landcover::choose_ecology_position([8., 40.], bounds, 4., water, &[], &occupied)
                .unwrap_err(),
            landcover::NO_ECOLOGY_ANCHOR
        );
        let layers = profile(Material::ConservationForestFloor).unwrap().layers;
        let (plants, rejections) = relayout_patch(
            &patch(
                "published",
                0.,
                32.,
                16.,
                16.,
                Material::ConservationForestFloor,
            ),
            bounds,
            layers,
            &[5, 3, 3],
            &terrain,
            water,
            &[],
        )
        .unwrap()
        .expect("in-domain patch must be relaid out");
        assert_eq!(rejections.len(), 0);
        assert_eq!(plants.len(), 11);
        for layer in ["canopy", "understory", "ground"] {
            assert_eq!(
                plants.iter().filter(|plant| plant.layer == layer).count(),
                if layer == "canopy" { 5 } else { 3 },
                "{layer}"
            );
        }
        for (index, plant) in plants.iter().enumerate() {
            assert!(plant.world_x - plant.width_m * 0.5 >= bounds[0] - 1e-9);
            assert!(plant.world_x + plant.width_m * 0.5 <= bounds[1] + 1e-9);
            assert!(plant.world_z - plant.width_m * 0.5 >= bounds[2] - 1e-9);
            assert!(plant.world_z + plant.width_m * 0.5 <= bounds[3] + 1e-9);
            assert!(
                !plants[index + 1..]
                    .iter()
                    .any(|other| overlaps(plant, other))
            );
        }
    }

    /// A layer footprint wider than the patch itself is a real capacity refusal:
    /// the re-layout must name it, not abort the world.
    #[test]
    fn footprint_wider_than_the_patch_stays_a_named_refusal() {
        let (_, _, terrain, water) = fixture();
        let layers = profile(Material::ConservationForestFloor).unwrap().layers;
        let (plants, rejections) = relayout_patch(
            &patch("tiny", 0., 32., 2., 2., Material::ConservationForestFloor),
            [0., 2., 32., 34.],
            layers,
            &[1, 0, 0],
            &terrain,
            water,
            &[],
        )
        .unwrap()
        .expect("in-domain patch must be relaid out");
        assert!(plants.is_empty());
        assert_eq!(rejections.len(), 1);
        assert_eq!(rejections[0].layer, "canopy");
        assert_eq!(rejections[0].reason, landcover::NO_ECOLOGY_ANCHOR);
    }

    /// Seeds that realized only 42 of 43 planned instances before the repair
    /// existed (`patch-*.canopy.4` refused in a 16x16 patch with room to spare).
    /// Each must now return the full quota with no named refusal left.
    #[test]
    fn seed_offsets_that_lost_the_fifth_canopy_now_realize_the_whole_quota() {
        let (mut scene, patches, terrain, water) = fixture();
        let mut seen = 0;
        for seed in [13224u64, 19741, 29992, 30500, 34656, 38398] {
            scene.seed_offset = seed;
            let actual = realize(&scene, &patches, &terrain, water, &[]).unwrap();
            assert_eq!(
                actual
                    .communities
                    .iter()
                    .map(|community| community.planned_plant_count)
                    .sum::<usize>(),
                43,
                "seed {seed}"
            );
            assert_eq!(actual.plants.len(), 43, "seed {seed}");
            assert!(actual.rejections.is_empty(), "seed {seed}");
            assert_eq!(actual.status, "instances-realized", "seed {seed}");
            verify_contract(&actual, &patches, water, &[]).unwrap();
            seen += 1;
        }
        assert_eq!(seen, 6);
    }

    #[test]
    fn density_generation_repeats_roundtrips_and_has_no_pair_overlap() {
        let (s, patches, t, w) = fixture();
        let a = realize(&s, &patches, &t, w, &[]).unwrap();
        assert_eq!(a.plants.len(), 43);
        assert!(a.rejections.is_empty());
        verify_contract(&a, &patches, w, &[]).unwrap();
        for (i, p) in a.plants.iter().enumerate() {
            for q in &a.plants[i + 1..] {
                assert!(!overlaps(p, q));
            }
        }
        assert_eq!(a, realize(&s, &patches, &t, w, &[]).unwrap());
        let mut reversed = patches.clone();
        reversed.reverse();
        assert_eq!(a, realize(&s, &reversed, &t, w, &[]).unwrap());
        let decoded: EcologyRealization =
            serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        verify_contract(&decoded, &patches, w, &[]).unwrap();
        assert!(matches(&a, &decoded));
        let mut different = s.clone();
        different.seed_offset += 1;
        let b = realize(&different, &patches, &t, w, &[]).unwrap();
        assert_eq!(a.plants.len(), b.plants.len());
        assert_ne!(a.plants, b.plants);
    }
    #[test]
    fn blocked_ground_preserves_requested_density_and_accounts_every_missing_instance() {
        let (s, patches, t, w) = fixture();
        let obstacle:EntityInstance=serde_json::from_value(serde_json::json!({"entityId":"obstacle","assetId":"test","kind":"rock","worldX":0,"worldZ":48,"worldY":4,"scale":1.,"widthM":64.,"depthM":64.,"heightM":4.})).unwrap();
        let a = realize(&s, &patches, &t, w, &[obstacle.clone()]).unwrap();
        assert!(a.plants.is_empty());
        assert_eq!(a.rejections.len(), 43);
        assert_eq!(a.status, "incomplete");
        assert_eq!(
            a.communities
                .iter()
                .map(|c| c.planned_plant_count)
                .sum::<usize>(),
            43
        );
        verify_contract(&a, &patches, w, &[obstacle.clone()]).unwrap();
        for mode in 0..5 {
            let mut bad = a.clone();
            match mode {
                0 => {
                    bad.rejections.remove(0);
                }
                1 => bad.rejections.push(bad.rejections[0].clone()),
                2 => bad.communities[0].unfilled_plant_count = 0,
                3 => bad.communities[0].layer_budgets[0].unfilled_count = usize::MAX,
                _ => bad.rejections[0].source_patch_id = "absent".into(),
            }
            assert!(
                verify_contract(&bad, &patches, w, &[obstacle.clone()]).is_err(),
                "mode {mode}"
            );
        }
    }
    fn spatial_test_entity(id: &str, x: i32, z: i32, w: f32, d: f32, yaw: f32) -> EntityInstance {
        let mut e: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId":id,"assetId":"test","kind":"rock","worldX":x,"worldZ":z,
            "worldY":4,"widthM":w,"depthM":d,"heightM":2.,"scale":1.
        }))
        .unwrap();
        e.rotation_y_deg = yaw;
        e
    }

    fn footprint_checks() -> usize {
        landcover::FOOTPRINT_CHECKS.with(|count| count.get())
    }

    fn assert_spatial_verify_equal(
        actual: &EcologyRealization,
        patches: &[SurfacePatch],
        water: WaterGeometry,
        entities: &[EntityInstance],
    ) -> Result<(), String> {
        let reference = verify_contract_with_obstacles(
            actual,
            patches,
            water,
            &ObstacleQuery::linear(entities),
        );
        // Each verifier constructs its own view from the supplied final entities.
        let indexed = verify_contract(actual, patches, water, entities);
        assert_eq!(
            indexed, reference,
            "verify must preserve the first rejection reason"
        );
        indexed
    }

    #[test]
    fn indexed_and_linear_ecology_are_byte_exact_and_reduce_generation_and_verify_checks() {
        let (scene, patches, terrain, water) = fixture();
        let terrain_before = terrain.content_hash().unwrap();
        for mode in 0..8 {
            let mut entities: Vec<_> = (0..1024)
                .map(|i| {
                    spatial_test_entity(&format!("far-{i}"), 2000 + i * 64, -2000, 2., 4., 45.)
                })
                .collect();
            let mut patches = patches.clone();
            let mut w = water;
            match mode {
                0 => {}
                1 => entities.push(spatial_test_entity("rotated", -8, 40, 30., 0.1, 45.)),
                2 => entities.push(spatial_test_entity("blocked", 0, 48, 64., 64., 0.)),
                3 => {
                    let mut road = spatial_test_entity("long-road", 0, 48, 100_000., 1., 17.);
                    road.kind = "road".into();
                    entities.push(road);
                }
                4 => entities.push(spatial_test_entity(
                    "huge", 500_000, 500_000, 2_000_000., 2_000_000., 31.,
                )),
                5 => {
                    patches[1].material = Material::ConservationWetland;
                    patches[2].material = Material::ConservationGround;
                    w = crate::worldgen::water_geometry(
                        crate::worldgen::WaterKind::River,
                        &[],
                        &scene,
                        &serde_json::json!({}),
                    );
                }
                6 => {
                    let mut invalid =
                        spatial_test_entity("invalid", 1_000_000, 1_000_000, 1., 1., 0.);
                    invalid.scale = -1.;
                    entities.push(invalid);
                }
                _ => entities.push(spatial_test_entity("partial", -8, 48, 16., 32., 0.)),
            }
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let linear = ObstacleQuery::linear(&entities);
            let expected = realize_with_obstacles(&scene, &patches, &terrain, w, &linear).unwrap();
            let linear_checks = footprint_checks() - before;
            let linear_us = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let indexed = ObstacleQuery::indexed(&entities);
            let actual = realize_with_obstacles(&scene, &patches, &terrain, w, &indexed).unwrap();
            let indexed_checks = footprint_checks() - before;
            let indexed_us = started.elapsed().as_micros();
            assert_eq!(
                actual, expected,
                "mode {mode}: all fields, IDs, budgets and rejection order"
            );
            let actual_bytes = serde_json::to_vec(&actual).unwrap();
            let expected_bytes = serde_json::to_vec(&expected).unwrap();
            assert_eq!(
                actual_bytes, expected_bytes,
                "mode {mode}: no tolerant output comparison"
            );
            assert_eq!(
                realize(&scene, &patches, &terrain, w, &entities).unwrap(),
                actual
            );
            let mut reversed = patches.clone();
            reversed.reverse();
            assert_eq!(
                realize(&scene, &reversed, &terrain, w, &entities).unwrap(),
                actual
            );

            let started = std::time::Instant::now();
            let before = footprint_checks();
            let verify_linear = verify_contract_with_obstacles(
                &actual,
                &patches,
                w,
                &ObstacleQuery::linear(&entities),
            );
            let verify_linear_checks = footprint_checks() - before;
            let verify_linear_us = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let verify_indexed = verify_contract(&actual, &patches, w, &entities);
            let verify_indexed_checks = footprint_checks() - before;
            let verify_indexed_us = started.elapsed().as_micros();
            assert_eq!(
                verify_indexed, verify_linear,
                "mode {mode}: independent verify parity"
            );
            assert!(verify_indexed.is_ok(), "mode {mode}");
            if mode == 0 {
                assert_eq!(actual.plants.len(), 43);
                assert!(actual.rejections.is_empty());
                assert!(indexed_checks * 10 < linear_checks);
                assert!(verify_indexed_checks * 10 < verify_linear_checks);
            }
            if mode == 1 {
                assert!(indexed_checks > 0 && indexed_checks * 10 < linear_checks);
                assert!(
                    verify_indexed_checks > 0 && verify_indexed_checks * 10 < verify_linear_checks
                );
            }
            if mode == 7 {
                assert!(!actual.plants.is_empty() && !actual.rejections.is_empty());
                assert_eq!(actual.plants.len() + actual.rejections.len(), 43);
                assert!(actual.rejections.windows(2).all(|p| p[0].id < p[1].id));
            }
            if mode == 2 || mode == 4 || mode == 6 {
                assert!(actual.plants.is_empty());
                assert_eq!(actual.rejections.len(), 43);
                assert_eq!(actual.status, "incomplete");
            }
            eprintln!(
                "{}",
                serde_json::json!({
                    "format":"glyphweave.ecology-spatial-test", "mode":mode,
                    "outputBlake3":blake3::hash(&actual_bytes).to_hex().to_string(),
                    "plants":actual.plants.len(),"rejections":actual.rejections.len(),
                    "linearCandidateVisits":linear.candidate_visits.get(),
                    "indexedCandidateVisits":indexed.candidate_visits.get(),
                    "generationLinearChecks":linear_checks,"generationIndexedChecks":indexed_checks,
                    "verifyLinearChecks":verify_linear_checks,"verifyIndexedChecks":verify_indexed_checks,
                    "generationLinearUs":linear_us,"generationIndexedUs":indexed_us,
                    "verifyLinearUs":verify_linear_us,"verifyIndexedUs":verify_indexed_us
                })
            );
        }
        assert_eq!(terrain.content_hash().unwrap(), terrain_before);
    }

    #[test]
    fn indexed_verify_retains_negative_contract_errors_and_uses_new_final_obstacles() {
        let (scene, patches, terrain, water) = fixture();
        let original = realize(&scene, &patches, &terrain, water, &[]).unwrap();
        assert!(assert_spatial_verify_equal(&original, &patches, water, &[]).is_ok());
        for mode in 0..9 {
            let mut bad = original.clone();
            let expected_reason = match mode {
                0 => {
                    bad.plants.push(bad.plants[0].clone());
                    "duplicate or unplanned ecology plant"
                }
                1 => {
                    bad.plants[0].source_patch_id = "absent".into();
                    "ecology plant provenance mismatch"
                }
                2 => {
                    bad.plants[0].width_m = 1000.;
                    "ecology plant footprint is outside source patch"
                }
                3 => {
                    bad.plants[0].world_x = f64::NAN;
                    "invalid ecology plant contract"
                }
                4 => {
                    bad.plants[0].height_m = 0.;
                    "invalid ecology plant contract"
                }
                5 => {
                    bad.plants.remove(0);
                    "missing ecology outcome"
                }
                6 => {
                    bad.communities[0].density_per_ha += 1.;
                    "ecology area density budget mismatch"
                }
                7 => {
                    bad.communities[0].realized_plant_count += 1;
                    "ecology community count is not conserved"
                }
                _ => {
                    let a = &bad.plants[0];
                    let i = bad
                        .plants
                        .iter()
                        .enumerate()
                        .skip(1)
                        .find(|(_, p)| {
                            p.source_patch_id == a.source_patch_id && p.width_m <= a.width_m
                        })
                        .map(|(i, _)| i)
                        .unwrap();
                    bad.plants[i].world_x = a.world_x;
                    bad.plants[i].world_z = bad.plants[0].world_z;
                    "ecology plants overlap"
                }
            };
            assert_eq!(
                assert_spatial_verify_equal(&bad, &patches, water, &[]).unwrap_err(),
                expected_reason,
                "mode {mode}"
            );
        }
        // A valid generation cannot serve as a clearance cache for changed final entities.
        let p = &original.plants[0];
        let obstacle = spatial_test_entity(
            "new-final-blocker",
            p.world_x.round() as i32,
            p.world_z.round() as i32,
            4.,
            4.,
            45.,
        );
        assert_eq!(
            assert_spatial_verify_equal(&original, &patches, water, &[obstacle]).unwrap_err(),
            "ecology plant intersects hard entity"
        );
        let blocked = [spatial_test_entity("blocked", 0, 48, 64., 64., 0.)];
        let river = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::River,
            &[],
            &scene,
            &serde_json::json!({}),
        );
        assert!(river.intersects_axis_aligned(0., 48., 0.4, 0.4));
        let water_patch = patch(
            "water-test",
            -8.,
            40.,
            16.,
            16.,
            Material::ConservationWetland,
        );
        let water_plant = EcologyPlant {
            world_x: 0.,
            world_z: 48.,
            world_y: 4.,
            width_m: 0.8,
            height_m: 2.,
            ..Default::default()
        };
        assert_eq!(
            verify_plant_clearance(&water_plant, &water_patch, river, &[]).unwrap_err(),
            "ecology plant intersects water"
        );
        assert_eq!(
            verify_plant_clearance_with(&water_plant, &water_patch, river, |_, _, _| panic!(
                "water must reject before obstacle query"
            ))
            .unwrap_err(),
            "ecology plant intersects water"
        );
        let rejected = realize(&scene, &patches, &terrain, water, &blocked).unwrap();
        for mode in 0..4 {
            let mut bad = rejected.clone();
            let expected_reason = match mode {
                0 => {
                    bad.rejections[0].reason = "wrong-reason".into();
                    "invalid ecology rejection"
                }
                1 => {
                    bad.rejections.push(bad.rejections[0].clone());
                    "duplicate or unplanned ecology rejection"
                }
                2 => {
                    bad.rejections.remove(0);
                    "missing ecology outcome"
                }
                _ => {
                    bad.rejections[0].source_patch_id = "absent".into();
                    "invalid ecology rejection"
                }
            };
            assert_eq!(
                assert_spatial_verify_equal(&bad, &patches, water, &blocked).unwrap_err(),
                expected_reason
            );
        }
    }

    #[test]
    fn indexed_candidate_search_preserves_width_water_occupied_and_exhaustion_order() {
        let (scene, _, _, water) = fixture();
        let domains = [
            [-32., -16., -16., 0.],
            [-16., 0., 32., 48.],
            [16., 32., -32., -16.],
        ];
        let entities = [
            spatial_test_entity("rotated", -24, -8, 20., 0.1, 45.),
            spatial_test_entity("boundary", 32, -24, 1., 40., 0.),
            spatial_test_entity("far", 10000, 10000, 2., 2., 0.),
        ];
        let linear = ObstacleQuery::linear(&entities);
        let indexed = ObstacleQuery::indexed(&entities);
        let river = crate::worldgen::water_geometry(
            crate::worldgen::WaterKind::River,
            &[],
            &scene,
            &serde_json::json!({}),
        );
        for bounds in domains {
            for width in [0.8, 1., 1.5, 2.5, 4., 8., 16., 17., f64::NAN] {
                for w in [water, river] {
                    for occupied_width in [0.8, 4., 16.] {
                        let center = [(bounds[0] + bounds[1]) * 0.5, (bounds[2] + bounds[3]) * 0.5];
                        let occupied = [EcologyPlant {
                            world_x: center[0],
                            world_z: center[1],
                            width_m: occupied_width,
                            ..Default::default()
                        }];
                        for preferred in [
                            center,
                            [bounds[0], bounds[2]],
                            [bounds[1] + 1e-9, bounds[3]],
                        ] {
                            assert_eq!(
                                landcover::choose_ecology_position(
                                    preferred,
                                    bounds,
                                    width,
                                    w,
                                    &indexed.query(bounds),
                                    &occupied
                                ),
                                landcover::choose_ecology_position(
                                    preferred,
                                    bounds,
                                    width,
                                    w,
                                    &linear.query(bounds),
                                    &occupied
                                )
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    #[ignore = "real WORLD_DIR linear/indexed replay; coordinating agent RUN slot only"]
    fn real_world_dir_linear_indexed_ecology_equivalence() {
        use crate::worldgen::{SceneIndex, WorldIndex, WorldManifest};
        use std::{
            fs::File,
            io::BufReader,
            path::{Path, PathBuf},
        };
        fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
            serde_json::from_reader(BufReader::new(
                File::open(path).expect("required world JSON missing"),
            ))
            .expect("invalid world JSON")
        }
        fn contained(root: &Path, path: PathBuf) -> PathBuf {
            let real = path.canonicalize().expect("required world input missing");
            assert!(
                real.starts_with(root),
                "world input escapes root: {}",
                path.display()
            );
            real
        }
        let root = PathBuf::from(
            std::env::var_os("WORLD_DIR")
                .expect("WORLD_DIR is required; never default to an old work/r49 world"),
        )
        .canonicalize()
        .expect("WORLD_DIR must exist");
        let manifest: WorldManifest =
            read_json(&contained(&root, root.join("glyphweave.manifest.json")));
        let world: WorldIndex = read_json(&contained(&root, root.join("world.json")));
        assert_eq!(manifest.format, "glyphweave-world");
        assert_eq!(manifest.version, 1);
        assert_eq!(world.format, manifest.format);
        assert_eq!(world.version, manifest.version);
        assert_eq!(world.seed, manifest.world.seed);
        assert_eq!(world.render_mode, manifest.world.render_mode);
        assert!(
            !world.scenes.is_empty(),
            "real equivalence needs at least one scene"
        );
        assert_eq!(world.scenes.len(), manifest.scenes.len());
        let mut seen = std::collections::BTreeSet::new();
        for scene_file in &world.scenes {
            let scene_path = contained(&root, root.join(scene_file));
            let scene: SceneIndex = read_json(&scene_path);
            assert!(seen.insert(scene.scene_id.clone()), "duplicate sceneId");
            let spec = manifest
                .scenes
                .iter()
                .find(|s| s.scene_id == scene.scene_id)
                .expect("scene absent from manifest");
            assert_eq!(
                (scene.width_m, scene.depth_m, scene.origin_x, scene.origin_z),
                (spec.width_m, spec.depth_m, spec.origin_x, spec.origin_z)
            );
            let mut terrain = BakedTerrain::new(spec).unwrap();
            for chunk in &scene.chunks {
                let path = contained(&root, scene_path.parent().unwrap().join(&chunk.height_file));
                // Only one bounded chunk payload is buffered at a time.
                assert!(
                    (1..=512).contains(&chunk.valid_width_m)
                        && (1..=512).contains(&chunk.valid_depth_m)
                );
                let limit =
                    u64::from(chunk.valid_width_m) * u64::from(chunk.valid_depth_m) * 4 + 1024;
                assert!(
                    std::fs::metadata(&path).unwrap().len() <= limit,
                    "oversize height chunk"
                );
                let bytes = std::fs::read(path).expect("height chunk unreadable");
                terrain
                    .insert_chunk(
                        chunk.world_x,
                        chunk.world_z,
                        chunk.valid_width_m,
                        chunk.valid_depth_m,
                        &bytes,
                    )
                    .unwrap();
            }
            terrain.require_complete().unwrap();
            let surfaces = scene
                .landcover_surfaces
                .as_ref()
                .expect("published landcover/ecology required");
            assert_eq!(surfaces.scene_id, spec.scene_id);
            assert_eq!(
                surfaces.final_heightfield_hash,
                terrain.content_hash().unwrap()
            );
            let water = crate::worldgen::water_geometry(
                crate::worldgen::water_kind(&manifest.style),
                &scene.landmarks,
                spec,
                &manifest.style,
            );
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let linear = ObstacleQuery::linear(&scene.entities);
            let expected =
                realize_with_obstacles(spec, &surfaces.patches, &terrain, water, &linear).unwrap();
            let linear_us = started.elapsed().as_micros();
            let linear_checks = footprint_checks() - before;
            let expected_bytes = serde_json::to_vec(&expected).unwrap();
            let output_hash = blake3::hash(&expected_bytes).to_hex().to_string();
            drop(expected);
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let indexed =
                realize(spec, &surfaces.patches, &terrain, water, &scene.entities).unwrap();
            let indexed_us = started.elapsed().as_micros();
            let indexed_checks = footprint_checks() - before;
            let indexed_bytes = serde_json::to_vec(&indexed).unwrap();
            assert_eq!(
                indexed_bytes, expected_bytes,
                "linear/indexed full serialization differs"
            );
            assert_eq!(
                indexed_bytes,
                serde_json::to_vec(&surfaces.ecology).unwrap(),
                "published ecology is not the same-input result; do not treat a mismatched world as equivalent"
            );
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let old_verify = verify_contract_with_obstacles(
                &indexed,
                &surfaces.patches,
                water,
                &ObstacleQuery::linear(&scene.entities),
            );
            let verify_linear_us = started.elapsed().as_micros();
            let verify_linear_checks = footprint_checks() - before;
            let started = std::time::Instant::now();
            let before = footprint_checks();
            let new_verify = verify_contract(&indexed, &surfaces.patches, water, &scene.entities);
            let verify_indexed_us = started.elapsed().as_micros();
            let verify_indexed_checks = footprint_checks() - before;
            assert_eq!(old_verify, new_verify);
            new_verify.unwrap();
            eprintln!(
                "{}",
                serde_json::json!({"format":"glyphweave.real-ecology-equivalence","world":root,
                "worldRevision":world.revision,"sceneId":scene.scene_id,"outputBlake3":output_hash,
                "entities":scene.entities.len(),"patches":surfaces.patches.len(),"plants":indexed.plants.len(),
                "rejections":indexed.rejections.len(),"linearChecks":linear_checks,"indexedChecks":indexed_checks,
                "linearUs":linear_us,"indexedUs":indexed_us,"verifyLinearChecks":verify_linear_checks,
                "verifyIndexedChecks":verify_indexed_checks,"verifyLinearUs":verify_linear_us,"verifyIndexedUs":verify_indexed_us})
            );
        }
    }
}
