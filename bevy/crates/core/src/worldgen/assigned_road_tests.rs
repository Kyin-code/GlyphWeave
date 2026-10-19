//! The entrance contract at ACCEPTANCE: a lot may only land on a street its own
//! door can actually walk to, and that decision has to be read off the surface
//! the bake publishes - not off a declaration and not after the fact.
//!
//! Two facts are pinned here against the committed 512 m hillside sample:
//!
//!  * every accepted lot's route is recomputed from the PUBLISHED geometry (the
//!    scene's entities and the baked height payloads) and must end on the street
//!    the lot was assigned; one concrete path is pinned by length and endpoint;
//!  * the exact defect this round fixes - a lot whose plateau stands one step
//!    above its own street, with a walkable route around the block - is refused
//!    by name, and NO record can change that verdict: the gate never reads one.
use super::*;
use crate::baked_terrain::{BakedTerrain, TerrainError};
use road_network::RoadsideSite;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

const SAMPLE: &str = include_str!("../../../../../examples/hillside-town-512.manifest.json");

struct TempWorld(PathBuf);
impl TempWorld {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glyphweave-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn scene(&self) -> PathBuf {
        self.0.join("scenes/scene-0")
    }
}
impl Drop for TempWorld {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hillside() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects"));
    manifest
}

/// The published final raster, re-read from the baked height payloads exactly as
/// a reader of the world would.
fn published_terrain(scene: &SceneIndex, dir: &std::path::Path) -> BakedTerrain {
    let spec = SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    let mut terrain = BakedTerrain::new(&spec).unwrap();
    for chunk in &scene.chunks {
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("chunk-{}-{}.json", chunk.chunk_x, chunk.chunk_z))).unwrap()).unwrap();
        let bytes = std::fs::read(dir.join(meta["heightFile"].as_str().unwrap())).unwrap();
        terrain
            .insert_chunk(
                chunk.chunk_x as i32,
                chunk.chunk_z as i32,
                chunk.valid_width_m,
                chunk.valid_depth_m,
                &bytes,
            )
            .unwrap();
    }
    terrain.require_complete().unwrap();
    terrain
}

fn half_extents(entity: &EntityInstance) -> (f64, f64) {
    spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg)
}

/// Is `point` on the street's own rectangle (one cell of slack, which is the
/// route search's own goal tolerance)?
fn on_street(road: &EntityInstance, point: &AccessPoint) -> bool {
    let (hw, hd) = half_extents(road);
    (point.x - f64::from(road.world_x)).abs() <= hw + 1.0
        && (point.z - f64::from(road.world_z)).abs() <= hd + 1.0
}

/// 1. Every accepted lot's entrance is verified again from the published world.
///
/// This is the audit's own question ("does a checked route end on the street the
/// lot is assigned?") asked of a graph REBUILT from the published entities and
/// the baked raster, so no record in the scene is trusted. The hillside sample
/// publishes 22 lots; every one reaches its OWN street, and the shop this round
/// refused (`...gorge-shops...0.2.64.positive`) is not in the world at all.
#[test]
fn a_recomputed_graph_reaches_every_assigned_street_and_the_refused_lot_is_gone() {
    let world = TempWorld::new("assigned-road-recompute");
    bake_world(&hillside(), &world.0).expect("the committed hillside bakes");
    let scene: SceneIndex =
        serde_json::from_slice(&std::fs::read(world.scene().join("scene.json")).unwrap()).unwrap();
    let plan = scene.road_network_plan.as_ref().expect("the sample plans streets");
    assert_eq!(plan.roadside_sites.len(), 22);
    let terrain = published_terrain(&scene, &world.scene());
    let spec = SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    let water = water_geometry(WaterKind::River, &scene.landmarks, &spec, &hillside().style);
    let graph = build_access_graph(&scene.entities, &terrain, water);
    assert_eq!(graph.disconnected_entities, 0, "no accepted lot is stranded");

    // Every lot, one checked path each, ending on the street it was assigned.
    let mut lengths: BTreeMap<&str, f64> = BTreeMap::new();
    for site in &plan.roadside_sites {
        let link = graph
            .links
            .iter()
            .find(|link| link.entity_id == site.id && link.connected)
            .unwrap_or_else(|| panic!("{} has no checked route in the recomputed graph", site.id));
        assert_eq!(
            link.road_id.as_deref(),
            Some(site.road_id.as_str()),
            "{} reaches another street",
            site.id
        );
        assert!(!link.path.is_empty(), "{} carries no checked polyline", site.id);
        let road = scene
            .entities
            .iter()
            .find(|entity| entity.entity_id == site.road_id)
            .expect("the assigned street is published");
        let end = link.path.last().unwrap();
        assert!(
            on_street(road, end),
            "{} ends at ({}, {}), off street {}",
            site.id,
            end.x,
            end.z,
            site.road_id
        );
        let length = link.distance_m.expect("a connected link carries its length");
        assert!(length > 0. && length <= access::MAX_LENGTH, "{}: {length}", site.id);
        lengths.insert(site.id.as_str(), length);
    }
    assert_eq!(lengths.len(), 22);

    // ONE concrete entrance, pinned: the gorge shop on road-auto 0.2's west side
    // walks 8.000 m from its door to the street edge at (346, 160).
    let shop = "generated.frontage.scene-0.gorge-shops.generated.road-auto.scene-0.2.64.negative";
    let link = graph
        .links
        .iter()
        .find(|link| link.entity_id == shop)
        .expect("the west-side shop is published");
    assert_eq!(link.road_id.as_deref(), Some("generated.road-auto.scene-0.2"));
    assert_eq!(link.distance_m, Some(8.0));
    let end = link.path.last().unwrap();
    assert_eq!((end.x, end.z), (346.0, 160.0));

    // The lot this round refused is nowhere: no entity, no link, no planned site.
    let refused = "generated.frontage.scene-0.gorge-shops.generated.road-auto.scene-0.2.64.positive";
    assert!(!scene.entities.iter().any(|entity| entity.entity_id == refused));
    assert!(!graph.links.iter().any(|link| link.entity_id == refused));
    assert!(!plan.roadside_sites.iter().any(|site| site.id == refused));
    // And the refusal is NAMED in the published frontage outcomes.
    let shops = plan
        .frontage
        .as_ref()
        .expect("the frontage programme reports")
        .outcomes
        .iter()
        .find(|outcome| outcome.request.id == "gorge-shops")
        .expect("the gorge-shops request is reported");
    // The refusal is named by the preflight's own vocabulary: the checked route
    // ends on the NEXT street, not on the one this shop is assigned.
    let refused_for_access = shops
        .rejected
        .get("access_to_different_road")
        .copied()
        .unwrap_or(0)
        + shops
            .rejected
            .get("no_local_walkable_frontage")
            .copied()
            .unwrap_or(0);
    assert_eq!(
        refused_for_access, 1,
        "the walkable-entrance refusal is published: {:?}",
        shops.rejected
    );
}

/// 2. The acceptance gate refuses a lot whose own street stands one step below
///    its plateau, and it refuses it BY NAME - while a lot on the same ground
///    whose declared top really is the street's level is accepted.
///
/// The raster is a 1 m step: the lot's plateau and its approach sit at 0.000 m,
/// the street band at -1.000 m. A 1 m drop in one cell is a 100 % grade, four
/// times the 25 % walkable contract, so no checked route can enter the lot from
/// its street; the route instead walks around the block. Nothing in the record
/// can change that: the gate reads the composed surface, never `supportLevelMm`
/// and never a published access link.
#[test]
fn a_lot_above_its_own_street_is_refused_at_acceptance_by_name() {
    let scene = SceneSpec {
        scene_id: "acceptance".into(),
        width_m: 64,
        depth_m: 64,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    };
    // Two rasters on the same lot and street: a 1 m STEP into the street band
    // (four times the walkable grade), and the flat ground on which a lot level
    // with its street stands. The step is the probe's defect; the flat ground is
    // the control that shows the gate is not an always-refuse rule.
    let raster = |step: bool| {
        let mut values = Vec::with_capacity(64 * 64);
        for z in 0..64 {
            let h = if step && z < 36 { 0.0 } else { -1.0 };
            for _ in 0..64 {
                values.push(h);
            }
        }
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(
                0,
                0,
                64,
                64,
                &crate::height_payload::encode_millimetres(64, 64, values).unwrap(),
            )
            .unwrap();
        natural
    };
    let water = water_geometry(WaterKind::None, &[], &scene, &json!({}));
    // One street, 64 m along X, 8 m wide, on the lower band.
    let road = EntityInstance {
        entity_id: "street".into(),
        asset_id: "prop.road".into(),
        kind: "road".into(),
        world_x: 32,
        world_z: 40,
        world_y: -1,
        width_m: 64.,
        depth_m: 8.,
        height_m: 0.24,
        rotation_y_deg: 0.,
        scale: 1.,
        grounding: GroundingSpec::default(),
        anchors: vec![],
        bounds: None,
    };
    let site = |level_mm: i32| RoadsideSite {
        program: None,
        purpose: None,
        id: "lot".into(),
        road_id: "street".into(),
        world_x: 32,
        world_z: 16,
        width_m: 24,
        depth_m: 24,
        support_level_m: level_mm.div_euclid(1000),
        parcel_insets: None,
        support_level_mm: Some(level_mm),
        frontage_road_point: None,
        bounded_cut_mm: None,
        earthwork_blend_m: None,
        rotation_y_deg: 0.,
        natural_min_m: -1.0,
        natural_max_m: 0.0,
    };
    let step = raster(true);
    // The honest record: a plateau at 0.000 m, one metre above its street.
    let high = site(0);
    let verdict = road_network::assigned_road_gate(
        &high,
        std::slice::from_ref(&road),
        &[],
        &step,
        None,
        0,
        water,
    )
    .expect("the gate composes a raster");
    let (category, reason) = verdict.expect("a 1 m step into the street cannot be walked");
    assert_eq!(category, "assigned_road_not_reachable_at_acceptance");
    assert!(reason.contains("street"), "{reason}");

    // A FORGED record cannot buy the lot: declaring the frontage point on the
    // street's own axis changes nothing, because the gate never reads it.
    let mut forged = high.clone();
    forged.frontage_road_point = Some([32, 40]);
    let forged_verdict = road_network::assigned_road_gate(
        &forged,
        std::slice::from_ref(&road),
        &[],
        &step,
        None,
        0,
        water,
    )
    .expect("the gate composes a raster");
    assert!(forged_verdict.is_some(), "a declared frontage point is not access");

    // The control: the same lot on flat ground level with its street is
    // enterable, so the refusal above is the 1 m step, not the rule.
    let flat = raster(false);
    let level = site(-1000);
    let accepted = road_network::assigned_road_gate(
        &level,
        std::slice::from_ref(&road),
        &[],
        &flat,
        None,
        0,
        water,
    )
    .expect("the gate composes a raster");
    assert!(accepted.is_none(), "a lot level with its street is enterable");
}

/// The raster a test builds must be complete; a missing sample is an error, not a
/// silently invented height.
#[test]
fn a_missing_natural_sample_is_an_error_not_a_guess() {
    let scene = SceneSpec {
        scene_id: "blank".into(),
        width_m: 8,
        depth_m: 8,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    };
    let natural = BakedTerrain::new(&scene).unwrap();
    let site = RoadsideSite {
        program: None,
        purpose: None,
        id: "lot".into(),
        road_id: "street".into(),
        world_x: 4,
        world_z: 4,
        width_m: 4,
        depth_m: 4,
        support_level_m: 0,
        parcel_insets: None,
        support_level_mm: Some(0),
        frontage_road_point: None,
        bounded_cut_mm: None,
        earthwork_blend_m: None,
        rotation_y_deg: 0.,
        natural_min_m: 0.,
        natural_max_m: 0.,
    };
    let road = EntityInstance {
        entity_id: "street".into(),
        asset_id: "prop.road".into(),
        kind: "road".into(),
        world_x: 4,
        world_z: 4,
        world_y: 0,
        width_m: 8.,
        depth_m: 4.,
        height_m: 0.24,
        rotation_y_deg: 0.,
        scale: 1.,
        grounding: GroundingSpec::default(),
        anchors: vec![],
        bounds: None,
    };
    let water = water_geometry(WaterKind::None, &[], &scene, &json!({}));
    let error: TerrainError =
        road_network::assigned_road_gate(&site, &[road], &[], &natural, None, 0, water).unwrap_err();
    assert!(error.0.contains("missing natural sample"), "{}", error.0);
}

// ---------------------------------------------------------------------------
// Round 46: the acceptance surface is composed per WINDOW, off a carve index, and
// memoised per window. These three tests pin the three claims that make that
// legitimate: the pruned composition is the full composition cell for cell, a memo
// hit is the raster a miss produces, and a direct entrance never pays for the full
// search window at all.
// ---------------------------------------------------------------------------

/// One scene with every kind of carve the published composition treats differently:
/// a graded road (with a profile AND a cut cap: the road-cut/transport branches),
/// a flat road, cap-carrying pads of two sizes (platform, cut floor, apron), a
/// fill-only pad and a rotated pad. The natural ground is a slope with a hill and a
/// hollow so fills, rings and cuts all fire somewhere.
fn carve_composition_fixture() -> (SceneSpec, BakedTerrain, Vec<TerrainCarve>) {
    let scene = SceneSpec {
        scene_id: "carve-index".into(),
        width_m: 256,
        depth_m: 256,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    };
    let mut values = Vec::with_capacity(256 * 256);
    for z in 0..256 {
        for x in 0..256 {
            let gx = f64::from(x) - 128.;
            let gz = f64::from(z) - 128.;
            let hill = 6.0 * (-(gx * gx + gz * gz) / 6_000.0).exp();
            let hollow = -3.0 * (-((gx - 0.) * (gx - 0.) + (gz - 80.) * (gz - 80.)) / 3_000.0).exp();
            values.push(1.5 + 0.006 * gx - 0.004 * gz + hill + hollow);
        }
    }
    let mut natural = BakedTerrain::new(&scene).expect("scene raster");
    natural
        .insert_chunk(
            0,
            0,
            256,
            256,
            &crate::height_payload::encode_millimetres(256, 256, values).expect("payload"),
        )
        .expect("chunk");

    let road = |id: &str, x: i32, z: i32, w: f32, d: f32, profile: bool, cut: Option<i32>| {
        let mut grounding = GroundingSpec::default();
        grounding.bounded_cut_mm = cut;
        if profile {
            grounding.road_profile = Some(crate::worldgen::road_profile::RoadProfile {
                version: 1,
                road_width_m: 12,
                start: [x - (w as i32) / 2, z],
                end: [x + (w as i32) / 2, z],
                elevations_mm: vec![2_000, 1_400, 900, 1_300, 2_100],
                max_fill_mm: 6_000,
            });
        }
        EntityInstance {
            entity_id: id.into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            width_m: w,
            depth_m: d,
            height_m: 0.24,
            rotation_y_deg: 0.,
            scale: 1.,
            grounding,
            anchors: vec![],
            bounds: None,
        }
    };
    let pad = |id: &str, kind: &str, x: i32, z: i32, w: f32, d: f32, rot: f32, cut: Option<i32>, blend: Option<f32>| {
        let mut grounding = GroundingSpec::default();
        grounding.bounded_cut_mm = cut;
        grounding.earthwork_blend_m = blend;
        grounding.frontage_road_point = Some([x, z + 20]);
        EntityInstance {
            entity_id: id.into(),
            asset_id: "prop.building".into(),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            width_m: w,
            depth_m: d,
            height_m: 8.,
            rotation_y_deg: rot,
            scale: 1.,
            grounding,
            anchors: vec![crate::worldgen::SpatialAnchor {
                id: "front".into(),
                world_x: x,
                world_z: z + (d as i32) / 2,
                direction: "south".into(),
                target: None,
            }],
            bounds: None,
        }
    };
    let entities = vec![
        road("graded", 128, 64, 224., 12., true, Some(2_000)),
        road("flat", 128, 200, 200., 8., false, None),
        pad("home-a", "residential_home", 64, 96, 24., 24., 0., Some(2_000), Some(32.0)),
        pad("home-b", "residential_home", 176, 96, 24., 24., 15., Some(2_000), None),
        pad("shop-a", "storefront", 96, 160, 12., 10., 0., None, Some(20.0)),
        pad("shop-b", "storefront", 200, 160, 12., 10., 45., Some(1_000), Some(20.0)),
    ];
    let carves = plan_terrain_carves(&entities);
    (scene, natural, carves)
}

fn pseudo_random(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed
}

/// 1. The indexed (windowed) composition equals the full-list composition CELL FOR
///    CELL, bit for bit, on 200 pseudo-random windows - including windows that hold
///    only natural ground, windows cut by the scene edge, and single-cell windows.
///    Both the unzoned and the zoned published composition are checked, because the
///    zoning path adds the protected-land and fill-taper branches.
#[test]
fn the_indexed_window_composes_every_cell_the_full_list_composes() {
    let (scene, natural, carves) = carve_composition_fixture();
    assert!(carves.len() > 12, "the fixture carries real carves: {}", carves.len());
    let index = CarveIndex::new(&carves);
    let water = water_geometry(WaterKind::River, &[], &scene, &json!({}));
    let zones = plan_landuse_zones(
        &scene,
        &LandUseProfile {
            theme: CityForm::TemperatePlain,
            urban_core_ratio: 0.10,
            suburban_ratio: 0.05,
            green_ratio: 0.05,
            farm_ratio: 0.60,
            forest_ratio: 0.10,
            pasture_ratio: 0.05,
            reserve_ratio: 0.05,
        },
    );
    let mask = zoning::ZoningMask::new(&scene, &zones).expect("zoning mask");

    let mut seed = 0x5EED_2545_F491_4F6D_u64;
    let mut compared = 0_u64;
    for window in 0..200 {
        let raw = pseudo_random(&mut seed);
        let w = 1 + (raw % 40) as i32;
        let h = 1 + ((raw >> 16) % 32) as i32;
        let x0 = ((raw >> 24) % 256) as i32;
        let z0 = ((raw >> 32) % 256) as i32;
        let x1 = (x0 + w).min(255);
        let z1 = (z0 + h).min(255);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let plain = published_surface_from_raster(&natural, None, water, &carves, x, z)
                    .expect("natural sample");
                let pruned = published_surface_from_raster_indexed(
                    &natural, None, water, &carves, &index, x, z,
                )
                .expect("natural sample");
                assert_eq!(
                    plain.to_bits(),
                    pruned.to_bits(),
                    "window {window} cell ({x},{z}): the pruned list must compose the plain answer"
                );
                let zoned =
                    published_surface_from_raster(&natural, Some(&mask), water, &carves, x, z)
                        .expect("natural sample");
                let zoned_pruned = published_surface_from_raster_indexed(
                    &natural,
                    Some(&mask),
                    water,
                    &carves,
                    &index,
                    x,
                    z,
                )
                .expect("natural sample");
                assert_eq!(
                    zoned.to_bits(),
                    zoned_pruned.to_bits(),
                    "window {window} cell ({x},{z}): the zoned pruned list must compose the plain answer"
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 20_000, "the windows cover real area: {compared} cells");
}

/// 2. A window memo HIT is the raster a MISS produces, byte for byte: the window is
///    composed cold (every cell a miss), then composed again from the memo (every
///    cell a hit), and the two encoded millimetre payloads are compared as bytes.
#[test]
fn a_window_memo_hit_is_the_same_raster_as_a_miss() {
    let (_scene, natural, carves) = carve_composition_fixture();
    let index = CarveIndex::new(&carves);
    let water = water_geometry(WaterKind::None, &[], &_scene, &json!({}));
    let (x0, z0, cols, rows) = (40_i32, 48_i32, 64_i32, 48_i32);
    let mut memo = WindowMemo::new();
    let mut compose = |memo: &mut WindowMemo| -> Vec<u8> {
        if memo.is_empty() {
            memo.reset(x0, z0, cols, rows);
        }
        let mut values = Vec::with_capacity((cols * rows) as usize);
        for z in z0..z0 + rows {
            for x in x0..x0 + cols {
                let value = match memo.get(x, z) {
                    Some(hit) => hit,
                    None => {
                        let composed = published_surface_from_raster_indexed(
                            &natural, None, water, &carves, &index, x, z,
                        )
                        .expect("natural sample");
                        memo.set(x, z, composed);
                        composed
                    }
                };
                values.push(value);
            }
        }
        crate::height_payload::encode_millimetres(cols as u32, rows as u32, values)
            .expect("payload")
    };
    let cold = compose(&mut memo);
    // The whole window is now memoised: every read below is a HIT.
    for z in z0..z0 + rows {
        for x in x0..x0 + cols {
            assert!(memo.get(x, z).is_some(), "cell ({x},{z}) was not memoised");
        }
    }
    let warm = compose(&mut memo);
    assert_eq!(
        cold, warm,
        "a memo hit must produce the very bytes a miss produced"
    );
    // And the memoised raster is the unmemoised one for the same window.
    let mut direct = Vec::with_capacity((cols * rows) as usize);
    for z in z0..z0 + rows {
        for x in x0..x0 + cols {
            direct.push(
                published_surface_from_raster_indexed(&natural, None, water, &carves, &index, x, z)
                    .expect("natural sample"),
            );
        }
    }
    assert_eq!(
        crate::height_payload::encode_millimetres(cols as u32, rows as u32, direct)
            .expect("payload"),
        warm,
        "the memo must hold the direct composition"
    );
}

/// 3. A candidate whose DIRECT entrance works returns from the PHASE 1 window and
///    never composes the full search window. The assertion is on the composed
///    windows themselves: exactly one window, and it is the 24 m one (its extent is
///    the lot plus two 24 m margins), not the 124 m search budget.
#[test]
fn a_direct_entrance_never_composes_the_full_window() {
    let scene = SceneSpec {
        scene_id: "early-exit".into(),
        width_m: 128,
        depth_m: 128,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    };
    let mut natural = BakedTerrain::new(&scene).expect("scene raster");
    natural
        .insert_chunk(
            0,
            0,
            128,
            128,
            &crate::height_payload::encode_millimetres(128, 128, vec![0.0; 128 * 128])
                .expect("payload"),
        )
        .expect("chunk");
    let road = EntityInstance {
        entity_id: "street".into(),
        asset_id: "prop.road".into(),
        kind: "road".into(),
        world_x: 64,
        // North of the lot: the generated door faces that way, so the straight
        // entrance the acceptance check tries first is the real one.
        world_z: 32,
        world_y: 0,
        width_m: 128.,
        depth_m: 8.,
        height_m: 0.24,
        rotation_y_deg: 0.,
        scale: 1.,
        grounding: GroundingSpec::default(),
        anchors: vec![],
        bounds: None,
    };
    let site = RoadsideSite {
        program: None,
        purpose: None,
        id: "lot".into(),
        road_id: "street".into(),
        world_x: 64,
        world_z: 64,
        width_m: 24,
        depth_m: 24,
        support_level_m: 0,
        parcel_insets: None,
        support_level_mm: Some(0),
        frontage_road_point: None,
        bounded_cut_mm: None,
        earthwork_blend_m: None,
        rotation_y_deg: 0.,
        natural_min_m: 0.0,
        natural_max_m: 0.0,
    };
    let water = water_geometry(WaterKind::None, &[], &scene, &json!({}));
    super::access::observation::begin_plan_windows();
    let verdict = road_network::assigned_road_gate(&site, &[road], &[], &natural, None, 0, water)
        .expect("the gate composes a raster");
    assert!(verdict.is_none(), "a lot level with its street is enterable");
    let windows = super::access::observation::plan_windows();
    assert_eq!(
        windows.len(),
        1,
        "the direct entrance must be decided by the phase-1 window alone: {windows:?}"
    );
    let (x0, z0, x1, z1) = windows[0];
    // Lot 24x24 at (64,64) plus the 24 m phase-1 margin, plus the assigned street's
    // nearest point - and never the 124 m search budget of phase 2.
    assert!(
        x1 - x0 + 1 <= 24 + 2 * 24 + 2 && z1 - z0 + 1 <= 24 + 2 * 24 + 2,
        "phase 1 must stay a small window, got {}x{}",
        x1 - x0 + 1,
        z1 - z0 + 1
    );
    assert!(
        (x1 - x0 + 1) * (z1 - z0 + 1) < 76 * 76,
        "the full window would be far larger"
    );
}

