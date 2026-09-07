//! GlyphWeave-original, area-driven vegetation budgets and deterministic placement.
//! Densities/stratum weights are design presets, not ecological measurements.
use super::landcover::{self, Material, SurfacePatch};
use crate::{
    baked_terrain::BakedTerrain,
    worldgen::{EntityInstance, SceneSpec, WaterGeometry},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    let mut result = EcologyRealization {
        model: "area-budgeted-stratified-vegetation-v3".into(),
        communities: communities(patches)?,
        ..Default::default()
    };
    let patches: BTreeMap<_, _> = patches.iter().map(|p| (p.id.as_str(), p)).collect();
    let obstacles: Vec<_> = entities
        .iter()
        .filter(|e| landcover::ecology_obstacle(e))
        .collect();
    let mut classes = BTreeMap::<String, (usize, f64, BTreeSet<String>)>::new();
    for c in &mut result.communities {
        let patch = patches[c.source_patch_id.as_str()];
        let p = profile(patch.material).unwrap();
        let entry = classes.entry(p.class.into()).or_default();
        entry.0 += 1;
        entry.1 += landcover::mesh_area(patch)?;
        entry.2.insert(patch.source_cell_id.clone());
        let patch_bounds = bounds(patch);
        let mut local = vec![];
        for (budget, layer) in c.layer_budgets.iter_mut().zip(p.layers) {
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
                    &obstacles,
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
                        budget.realized_count += 1;
                        c.realized_plant_count += 1;
                    }
                    Err(reason) => {
                        result.rejections.push(EcologyRejection {
                            id,
                            source_patch_id: patch.id.clone(),
                            layer: layer.name.into(),
                            reason,
                        });
                        budget.unfilled_count += 1;
                        c.unfilled_plant_count += 1;
                    }
                }
            }
        }
        result.plants.extend(local);
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
pub(super) fn verify_plant_clearance(
    plant: &EcologyPlant,
    patch: &SurfacePatch,
    water: WaterGeometry,
    entities: &[EntityInstance],
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
    if entities
        .iter()
        .filter(|e| landcover::ecology_obstacle(e))
        .any(|e| landcover::footprint_intersects_entity(e, plant.world_x, plant.world_z, r, r))
    {
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
        verify_plant_clearance(plant, patch_map[source], water, entities)?;
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
            || rejection.reason != "no clear full-size ecology anchor in source patch"
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
}
