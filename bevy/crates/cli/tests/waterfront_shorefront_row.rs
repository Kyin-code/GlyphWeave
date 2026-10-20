//! Round 24: the shoreline commercial row, and the freeze that keeps it out of every world that
//! did not ask for it.
//!
//! Round 21/23 measured the defect: the registered river-delta sample's twelve
//! `generated.storefront.stable-v1.*` buildings were the legacy bayfront band at
//! `centre ± (half_width + 150)`, i.e. 150 m from the water, entirely inside the rural zoning and
//! three blocks behind the quay, while the two streets that run ALONG the shore carried no shopfront
//! at all. This round retires that band for any world whose road plan can plan the commerce on the
//! first street landward of the shore (`worldgen::shorefront`), and keeps it byte for byte for every
//! world whose plan cannot.
//!
//! What these tests pin, in order:
//!
//! 1. a waterfront world with NO road plan and a waterless world publish the same bytes they always
//!    did (blake3 over `scene.json` plus every height payload);
//! 2. the row really is a quay row: every shop stands within 20 m of the published shoreline
//!    public-passage band, fronts the first street landward of the shore, and is walkable from THAT
//!    street. The row's commerce inside the URBAN zoning is pinned too - it is zero on this fixture,
//!    and the named refusals that keep it zero are asserted, so the gap is a measurement instead of
//!    a claim;
//! 3. no commercial footprint - the row's or anyone else's - enters the water or the shore band, on
//!    the level of the 1 m cell;
//! 4. the authored along-road conversion switch decides whether the row's rural stations convert:
//!    off, the ledger books nothing; on, every converted square metre is re-derivable and the plan's
//!    records equal the ledger's;
//! 5. two bakes of the same input publish identical bytes.
use glyphweave_core::baked_terrain::BakedTerrain;
use glyphweave_core::rules::ObjectRegistry;
use glyphweave_core::worldgen::{
    SceneIndex, SceneSpec, WATER_SHORE_CLEARANCE_M, WorldManifest, bake_world,
    verify_published_placement, water_geometry, water_kind,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::{fs, path::PathBuf};

/// The published river-delta sample, compiled in: the test cannot drift from the example.
const SAMPLE: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");
const SCENE_SIZE_M: u32 = 512;

/// The frzoen digests of the two worlds the row may never reach. Measured on `6eeaf85` (the commit
/// before this round) and again here: identical, because the whole row is gated on a road plan and a
/// stated street morphology.
///
/// Round 40 re-measured the waterfront digest: the plan-less world no longer publishes what sat
/// inside its own 4 m shore clearance (generation now reads the published constant). The row's
/// gate is unchanged - the same scene, plan and rasters are otherwise byte-identical.
const FROZEN_WATERFRONT_DIGEST: &str =
    "a9630499b3e5d6c5ecc32c6037de69d04cc0e89daad7a79307ac64f625c205b4";
const FROZEN_WATERLESS_DIGEST: &str =
    "49faeab9f532144fd5c75c115622f86506038a5c05c65657619a9511e241f91d";

struct TestDir(PathBuf);
impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("glyphweave-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
}

fn glyphweave() -> &'static str {
    env!("CARGO_BIN_EXE_glyphweave")
}

/// The published example, with `ruralConversion` stated and its repo-relative path made absolute.
fn sample(explicit_conversion: bool) -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    if explicit_conversion {
        manifest.style["frontageProgram"]["ruralConversion"] = json!("explicit");
    }
    manifest
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap()
}

fn raster(dir: &TestDir, scene: &SceneIndex, baked: bool) -> BakedTerrain {
    let spec = SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    let mut terrain = BakedTerrain::new(&spec).unwrap();
    let folder = dir.0.join("scenes").join(&scene.scene_id);
    for chunk in &scene.chunks {
        let file = if baked {
            chunk.height_file.clone()
        } else {
            chunk
                .natural_height_file
                .clone()
                .expect("every baked chunk publishes its natural payload")
        };
        let bytes = fs::read(folder.join(file)).unwrap();
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
    terrain
}

fn published_digest(dir: &TestDir, scene: &Value) -> String {
    let folder = dir.0.join("scenes/scene-0");
    let mut hasher = blake3::Hasher::new();
    hasher.update(&fs::read(folder.join("scene.json")).unwrap());
    for chunk in scene["chunks"].as_array().unwrap() {
        hasher.update(&fs::read(folder.join(chunk["heightFile"].as_str().unwrap())).unwrap());
    }
    hasher.finalize().to_hex().to_string()
}

/// The commercial kinds this round measures: the shells a shopper walks into.
fn is_commercial(kind: &str) -> bool {
    matches!(
        kind,
        "storefront"
            | "shopping_mall"
            | "office_tower"
            | "convenience_store"
            | "commercial_center"
            | "night_market"
            | "entertainment_center"
            | "market"
    )
}

fn building_half_extents(width_m: f32, depth_m: f32, rotation_y_deg: f32) -> (f64, f64) {
    let quarter = (rotation_y_deg.round() as i32).rem_euclid(180) == 90;
    if quarter {
        (f64::from(depth_m) / 2., f64::from(width_m) / 2.)
    } else {
        (f64::from(width_m) / 2., f64::from(depth_m) / 2.)
    }
}

/// A 512 m waterfront town with a structured river through its middle and NO road-first opt-in: the
/// historical waterfront world, which has no plan and therefore no shoreline row.
fn waterfront_without_opt_in() -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = 42;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.scenes[0].origin_x = 0;
    m.scenes[0].origin_z = 0;
    m.style["generationProfile"] = json!("strict");
    m.style["rulesMode"] = json!("rules");
    m.style["rulesDir"] = json!(rules());
    m.style["terrainProfile"] = json!("plains");
    m.style["water"] = json!({
        "waterType": "river",
        "levelPolicy": "horizontal-datum",
        "levelM": 0
    });
    m.landmarks = vec![serde_json::from_value(json!({
        "entityId": "landmark.waterfront-river",
        "name": "waterfront river",
        "type": "river",
        "purpose": "structured water the scene must live with",
        "description": "north-south river through the middle of the town",
        "sceneId": "scene-0",
        "worldX": 256,
        "worldZ": 256,
        "worldY": 0,
        "widthM": 16,
        "depthM": 512,
        "heightM": 1,
        "assetId": "water.waterfront-river"
    }))
    .unwrap()];
    m
}

/// A 512 m town that declares NO water at all.
fn waterless_town() -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = 42;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.scenes[0].origin_x = 0;
    m.scenes[0].origin_z = 0;
    m.style["generationProfile"] = json!("strict");
    m.style["rulesMode"] = json!("rules");
    m.style["rulesDir"] = json!(rules());
    m.style["terrainProfile"] = json!("plains");
    m.style["water"] = json!({
        "waterType": "none",
        "levelPolicy": "horizontal-datum",
        "levelM": 0
    });
    m.landmarks.clear();
    m
}

// ---------------------------------------------------------------------------------------------
// 1. The freeze: no plan, no row, no changed byte.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_world_with_no_plan_and_a_waterless_world_are_byte_identical() {
    let dir = TestDir::new("shorefront-no-plan");
    let manifest = waterfront_without_opt_in();
    bake_world(&manifest, &dir.0).unwrap();
    let scene: Value =
        serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap();
    assert!(
        scene["roadNetworkPlan"].is_null(),
        "a water scene without the opt-in plans no road network, so the row cannot exist either"
    );
    assert_eq!(
        published_digest(&dir, &scene),
        FROZEN_WATERFRONT_DIGEST,
        "the plan-less waterfront world changed bytes: the shoreline row is gated on a road plan \
         and a stated street morphology and may never reach a world that has neither"
    );

    let dir = TestDir::new("shorefront-waterless");
    let manifest = waterless_town();
    bake_world(&manifest, &dir.0).unwrap();
    let scene: Value =
        serde_json::from_slice(&fs::read(dir.0.join("scenes/scene-0/scene.json")).unwrap()).unwrap();
    assert!(
        scene["roadNetworkPlan"].is_object(),
        "a waterless strict world still plans its road network"
    );
    assert_eq!(
        published_digest(&dir, &scene),
        FROZEN_WATERLESS_DIGEST,
        "the waterless world changed bytes: a shoreline row needs a structured river, and a world \
         with no water has none"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. The row stands on the quay street, within 20 m of the shore.
// ---------------------------------------------------------------------------------------------

/// The published shoreline public-passage band of this sample: the dry strip within the frozen
/// clearance of the water body.
///
/// The band is a waterline offset, so beside a channel that runs the whole scene it is exactly two
/// strips, one per bank, and the distance from a column to the band is measured face to face. The
/// channel's own published geometry is the `river` entity's centre and width, which is what a reader
/// of the baked scene has.
struct ShoreBand {
    west: (f64, f64),
    east: (f64, f64),
}
impl ShoreBand {
    fn of(scene: &SceneIndex) -> Self {
        let river = scene
            .entities
            .iter()
            .find(|entity| entity.kind == "river")
            .expect("the sample publishes its structured river");
        let centre = f64::from(river.world_x);
        let half = f64::from(river.width_m) / 2.;
        Self {
            west: (centre - half - WATER_SHORE_CLEARANCE_M, centre - half),
            east: (centre + half, centre + half + WATER_SHORE_CLEARANCE_M),
        }
    }
    fn centre_x(scene: &SceneIndex) -> f64 {
        f64::from(
            scene
                .entities
                .iter()
                .find(|entity| entity.kind == "river")
                .expect("the sample publishes its structured river")
                .world_x,
        )
    }
    /// Metres from this column to the nearest band strip, face to face.
    fn distance_m(&self, x: f64) -> f64 {
        let d_west = (self.west.0 - x).max(x - self.west.1).max(0.);
        let d_east = (self.east.0 - x).max(x - self.east.1).max(0.);
        d_west.min(d_east)
    }
}

#[test]
fn the_row_stands_on_the_first_quay_street_within_twenty_metres_of_the_shore() {
    let dir = TestDir::new("shorefront-row");
    let manifest = sample(false);
    bake_world(&manifest, &dir.0).expect("the sample bakes");
    let scene = scene_index(&dir);
    let water = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &manifest.scenes[0],
        &manifest.style,
    );
    let plan = scene.road_network_plan.as_ref().expect("road plan");
    let band = ShoreBand::of(&scene);
    let channel_x = ShoreBand::centre_x(&scene);

    // The row is published by its own programme, and its refusals are published by name.
    let outcome = plan
        .service_outcomes
        .iter()
        .find(|outcome| outcome.request.id == "shorefront-row")
        .expect("the sample's road plan carries the shoreline row");
    assert_eq!(
        outcome.planned_site_ids.len(),
        SAMPLE_SHOREFRONT_SHOPS,
        "the row realizes what the fixture's quay frontage can hold: {outcome:?}"
    );
    assert_eq!(outcome.request.count, SAMPLE_SHOREFRONT_DEMAND);

    // Every shop stands within 20 m of the band, in the published entity list, and is walkable
    // from its OWN street. This is the whole claim: commerce on the shore, not 150 m off it.
    let rows: Vec<_> = scene
        .entities
        .iter()
        .filter(|entity| entity.entity_id.starts_with("generated.service.scene-0.shorefront-row."))
        .collect();
    assert_eq!(rows.len(), SAMPLE_SHOREFRONT_SHOPS);
    let mut zones: BTreeMap<&str, usize> = BTreeMap::new();
    for shop in &rows {
        assert_eq!(shop.kind, "storefront", "the row is commercial");
        let (half_w, _) = building_half_extents(shop.width_m, shop.depth_m, shop.rotation_y_deg);
        // The face of the shop that is nearest the water.
        let near_face = if f64::from(shop.world_x) < channel_x {
            f64::from(shop.world_x) + half_w
        } else {
            f64::from(shop.world_x) - half_w
        };
        let distance = band.distance_m(near_face);
        assert!(
            distance <= 20.0,
            "{} stands {distance} m from the shore band, further than the 20 m first row",
            shop.entity_id
        );
        let site = plan
            .roadside_sites
            .iter()
            .find(|site| site.id == shop.entity_id)
            .expect("every shop is a planned site");
        assert_eq!(site.kind(), "storefront");
        assert!(
            plan.entities()
                .iter()
                .any(|road| road.entity_id == site.road_id
                    && road.rotation_y_deg == 90.0
                    && water.clears_shore(
                        f64::from(road.world_x),
                        f64::from(road.world_z),
                        4.0,
                        32.0
                    )),
            "{} must front a street that runs ALONG the shore, not across it",
            shop.entity_id
        );
        let zone = scene
            .land_use_zones
            .iter()
            .find(|zone| {
                zone.world_x <= shop.world_x
                    && shop.world_x < zone.world_x + zone.width_m as i32
                    && zone.world_z <= shop.world_z
                    && shop.world_z < zone.world_z + zone.depth_m as i32
            })
            .map(|zone| zone.kind.as_str())
            .unwrap_or("unassigned");
        *zones.entry(zone).or_default() += 1;
    }
    assert_eq!(
        zones.get("rural").copied().unwrap_or(0),
        SAMPLE_SHOREFRONT_SHOPS,
        "the quay stations this fixture's road grid leaves legal are all in its rural north band"
    );
    assert_eq!(
        zones.get("urban").copied().unwrap_or(0),
        SAMPLE_SHOREFRONT_URBAN_SHOPS,
        "GAP, pinned: the row's commerce inside the URBAN zoning. The registered river-delta bank \
         rises away from the water; the frozen contracts are the 0.5 m walkable entrance apron and \
         the fill-only platform, so a quay station's window fill exceeds what its own road may \
         climb almost everywhere in the urban stretch. The named refusals ({:?}) say so station by \
         station instead of the row silently missing",
        outcome.rejected_candidates
    );
}

// ---------------------------------------------------------------------------------------------
// 3. No commercial footprint enters the water or the band, on the 1 m cell.
// ---------------------------------------------------------------------------------------------

#[test]
fn no_commercial_footprint_enters_the_water_or_the_shore_band() {
    let dir = TestDir::new("shorefront-water");
    let manifest = sample(false);
    bake_world(&manifest, &dir.0).expect("the sample bakes");
    let scene = scene_index(&dir);
    let water = water_geometry(
        water_kind(&manifest.style),
        &manifest.landmarks,
        &manifest.scenes[0],
        &manifest.style,
    );
    let natural = raster(&dir, &scene, false);
    let mut checked = 0_u64;
    for entity in &scene.entities {
        if !is_commercial(&entity.kind) {
            continue;
        }
        let (half_w, half_d) =
            building_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
        for z in entity.world_z - half_d as i32..entity.world_z + half_d as i32 {
            for x in entity.world_x - half_w as i32..entity.world_x + half_w as i32 {
                checked += 1;
                assert!(
                    !water.contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5),
                    "commercial {} stands on water at ({x},{z})",
                    entity.entity_id
                );
                assert!(
                    !water.shore_band_cell(x, z),
                    "commercial {} enters the reserved shore band at ({x},{z})",
                    entity.entity_id
                );
                assert!(
                    natural.height(x, z).is_some_and(|h| h >= 0.),
                    "commercial {} stands on a below-datum cell at ({x},{z})",
                    entity.entity_id
                );
            }
        }
        assert!(
            water.clears_shore(
                f64::from(entity.world_x),
                f64::from(entity.world_z),
                half_w,
                half_d
            ),
            "commercial {} does not keep the published {} m shore clearance",
            entity.entity_id,
            WATER_SHORE_CLEARANCE_M
        );
    }
    assert!(checked > 0, "the sample carries commercial buildings to check");
}

// ---------------------------------------------------------------------------------------------
// 4. The conversion switch decides whether the row's rural stations convert.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_rural_conversion_switch_decides_whether_the_row_converts() {
    // OFF: nothing declares a purpose, nothing converts, no plan gains the key.
    let off_dir = TestDir::new("shorefront-conversion-off");
    bake_world(&sample(false), &off_dir.0).expect("the sample bakes without the switch");
    let off: Value = read_json(off_dir.0.join("scenes/scene-0/scene.json"));
    let off_plan = &off["roadNetworkPlan"];
    assert_eq!(
        off_plan["padConversions"].as_array().map_or(0, Vec::len),
        0,
        "without the explicit switch no pad declares a purpose"
    );
    assert!(
        off_plan["frontage"].get("conversions").is_none(),
        "without the explicit switch no frontage parcel converts"
    );

    // ON: every converted square metre is re-derivable, and the plan and the ledger agree.
    let on_dir = TestDir::new("shorefront-conversion-on");
    bake_world(&sample(true), &on_dir.0).expect("the sample bakes with the switch");
    let on: Value = read_json(on_dir.0.join("scenes/scene-0/scene.json"));
    let on_plan = &on["roadNetworkPlan"];
    let pad_records = on_plan["padConversions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let shoreline_records: Vec<&Value> = pad_records
        .iter()
        .filter(|record| {
            record["parcelId"]
                .as_str()
                .is_some_and(|id| id.starts_with("generated.service.scene-0.shorefront-row."))
        })
        .collect();
    assert_eq!(
        shoreline_records.len(),
        SAMPLE_SHOREFRONT_CONVERTED_SHOPS,
        "every quay shop that stands inside the urban edge converts, and the one that stands 83 m \
         from it is refused by name instead of being booked as urban land: {pad_records:?}"
    );
    for record in &shoreline_records {
        assert_eq!(record["from"], json!("rural"));
        assert_eq!(record["to"], json!("urban"));
        assert_eq!(
            record["areaM2"],
            json!(SAMPLE_SHOREFRONT_CONVERTED_M2),
            "a shop converts exactly its own buildable core"
        );
    }
    // The shop the band refused is gone from the plan, and the plan says WHY, by name.
    let row_outcome = on_plan["serviceOutcomes"]
        .as_array()
        .expect("the plan publishes its service outcomes")
        .iter()
        .find(|outcome| outcome["request"]["id"] == json!("shorefront-row"))
        .expect("the row's outcome is published");
    assert_eq!(
        row_outcome["plannedSiteIds"].as_array().map(Vec::len),
        Some(SAMPLE_SHOREFRONT_CONVERTED_SHOPS),
        "the explicit policy plans the two quay shops the urban edge reaches: {row_outcome:?}"
    );
    assert!(
        row_outcome["rejectedCandidates"]
            .get(glyphweave_core::worldgen::conversion_fringe::CONVERSION_OUTSIDE_THE_URBAN_FRINGE)
            .is_some(),
        "the row's own refusals must name the urban edge: {row_outcome:?}"
    );

    // The gate's own audit: the plan's records equal the ledger's, by name.
    let output = std::process::Command::new(glyphweave())
        .arg("world-quality-gate")
        .arg(&on_dir.0)
        .output()
        .unwrap();
    let gate: Value = read_json(on_dir.0.join("world-quality-gate.json"));
    let quality: Value = read_json(on_dir.0.join("quality-report.json"));
    let scene_quality = &quality["scenes"][0];
    let conversion = &scene_quality["purposeConversionAudit"];
    assert_eq!(conversion["status"], json!("pass"), "{conversion}");
    assert_eq!(
        conversion["planned"].as_u64().unwrap() as usize,
        pad_records.len() + on_plan["frontage"]["conversions"].as_array().map_or(0, Vec::len),
        "every record the plan claims is re-derived"
    );
    assert_eq!(
        conversion["applied"],
        conversion["planned"],
        "the ledger applies exactly the plan's records"
    );
    assert_eq!(
        conversion["refusedPads"], json!(0),
        "no refused pad is booked"
    );
    let ledger = &scene_quality["landUseLedger"];
    let ledger_records = ledger["conversions"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        ledger_records.len(),
        conversion["planned"].as_u64().unwrap() as usize
    );
    let converted: i64 = ledger_records
        .iter()
        .map(|record| record["areaM2"].as_i64().unwrap())
        .sum();
    assert_eq!(converted, conversion["convertedM2"].as_i64().unwrap());
    assert!(
        !output.status.success() || gate["status"] == json!("fail"),
        "the sample's release verdict is asserted by the Round 21 test; this one only needs the \
         conversion audit to recompute"
    );
    // The engineering reconciliation still recomputes from the published ids.
    let placement: Value = read_json(on_dir.0.join("engineering-placement.json"));
    let published: std::collections::BTreeSet<String> = on["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entity| entity["entityId"].as_str().unwrap().to_owned())
        .collect();
    let authored: std::collections::BTreeSet<String> = on["landmarks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|landmark| landmark["entityId"].as_str().unwrap().to_owned())
        .collect();
    let registry = ObjectRegistry::load_dir(&rules()).unwrap();
    verify_published_placement(&placement["scenes"][0], &published, &authored, Some(&registry))
        .expect("the published skips recompute with the row in the plan");
}

// ---------------------------------------------------------------------------------------------
// 5. Determinism.
// ---------------------------------------------------------------------------------------------

#[test]
fn two_bakes_of_the_sample_publish_identical_bytes() {
    let first = TestDir::new("shorefront-determinism-a");
    let second = TestDir::new("shorefront-determinism-b");
    let manifest = sample(false);
    bake_world(&manifest, &first.0).unwrap();
    bake_world(&manifest, &second.0).unwrap();
    for file in ["engineering-placement.json", "scenes/scene-0/scene.json"] {
        assert_eq!(
            fs::read(first.0.join(file)).unwrap(),
            fs::read(second.0.join(file)).unwrap(),
            "{file} must be byte-identical across two bakes of the same input"
        );
    }
}

/// What the registered sample's quay frontage realizes, and the zoning it lands in.
///
/// `SAMPLE_SHOREFRONT_DEMAND` is what the row asks for (two bank streets, one shop per 18 m); the
/// shops are what the fixture's frozen contracts allow (see the pinned gap above).
const SAMPLE_SHOREFRONT_DEMAND: u32 = 42;
const SAMPLE_SHOREFRONT_SHOPS: usize = 3;
const SAMPLE_SHOREFRONT_URBAN_SHOPS: usize = 0;
/// The buildable core one converted quay shop takes, in whole square metres.
const SAMPLE_SHOREFRONT_CONVERTED_M2: i64 = 100;
/// How many of the three quay shops convert with `ruralConversion: "explicit"` on.
///
/// Round 31: the row is planned at the quay, and the sample's urban zoning stops 83 m south of the
/// northernmost station, so that shop's buildable core (z 35..45) is refused BY NAME as
/// `conversion_outside_the_urban_fringe` - it may not be booked as urban land 83 m from the city.
/// The other two stand 35 m and 19 m from the zoning and convert exactly as before. The row keeps
/// three shops with the switch OFF, which is what `SAMPLE_SHOREFRONT_SHOPS` measures: the band
/// narrows the *purpose*, never the frontage the sample authors.
const SAMPLE_SHOREFRONT_CONVERTED_SHOPS: usize = 2;
