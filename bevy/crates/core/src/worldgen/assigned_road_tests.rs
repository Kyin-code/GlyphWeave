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
