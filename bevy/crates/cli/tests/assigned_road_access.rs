//! The entrance contract at ACCEPTANCE, seen from the published world.
//!
//! A lot is planned only when the street it is assigned to is walkable from its
//! own door on the surface the bake composes. When that is false the candidate is
//! refused BY NAME during planning and withdrawn everywhere - plan, service
//! outcome, joint layout, conversion record and entity list - instead of being
//! published and then refused by the development audit.
//!
//! These tests pin the committed hillside sample: the one shop whose street stood
//! a step below its plateau (`...gorge-shops...0.2.64.positive`) is gone from the
//! world, every accepted lot still has its own street, a forged plan cannot bring
//! the lot back, two bakes are byte-identical, and the service world whose pads
//! were already consistent is frozen byte for byte.
use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::process::Command;
use std::{fs, path::PathBuf};

const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
const PLAINS: &str = include_str!("../../../../examples/service-program-plains.manifest.json");

/// The lot this round refused, and the request that wanted it.
const REFUSED: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-auto.scene-0.2.64.positive";
const REFUSED_PARCEL: &str =
    "generated.frontage.scene-0.gorge-shops.generated.road-auto.scene-0.2.64.positive.parcel";
/// The committed hillside town after the refusal: 22 lots, 15 homes, 6 shops.
const HILLSIDE_ENTITIES: usize = 422;
const HILLSIDE_PADS: usize = 22;
const HILLSIDE_HOMES: usize = 15;
const HILLSIDE_SHOPS: usize = 6;
/// The service world was already consistent: not one byte may move. The tree
/// digest also covers copied preview adapters, the mandatory final entity audit,
/// and third-party notices, so contract additions move this constant without
/// moving the authoritative terrain or scene data.
const FROZEN_PLAINS_DIGEST: &str =
    "8f5275a3c46882db96948b23cc1bd9113bdcfe2e844bb38dc2280f59b19aa81f";

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

fn sample() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(SAMPLE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest
}

fn plains() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(PLAINS).expect("plains parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest
}

fn scene_dir(dir: &TestDir) -> PathBuf {
    dir.0.join("scenes/scene-0")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn quality_report(dir: &TestDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap()
}

fn published_files(root: &PathBuf) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.clone()];
    while let Some(folder) = stack.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(
                    relative,
                    blake3::hash(&fs::read(&path).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    files
}

/// One digest over every published file of a world; `generation-report.json`
/// records the output directory and nothing else in a world embeds a path.
fn world_digest(files: &BTreeMap<String, String>) -> String {
    let mut hasher = blake3::Hasher::new();
    for (path, digest) in files
        .iter()
        .filter(|(path, _)| !path.contains("generation-report"))
    {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(digest.as_bytes());
        hasher.update(b"\0");
    }
    hasher.finalize().to_hex().to_string()
}

/// 1. The refused lot is withdrawn from ALL FIVE published places at once, and
///    the world it belonged to now passes the development audit it used to fail.
#[test]
fn the_refused_lot_is_absent_from_the_plan_service_joint_conversion_and_entities() {
    let dir = TestDir::new("assigned-road-withdrawn");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");
    let scene = read_json(scene_dir(&dir).join("scene.json"));
    let roads = read_json(scene_dir(&dir).join("road-network-plan.json"));
    let plan = read_json(scene_dir(&dir).join("development-plan.json"));
    let entities = scene["entities"].as_array().unwrap();

    // 1. no planned lot
    assert!(
        !roads["roadsideSites"]
            .as_array()
            .unwrap()
            .iter()
            .any(|site| site["id"] == REFUSED),
        "the refused lot is not a planned site"
    );
    // 2. no service/joint outcome names it
    let joint = roads["jointLayout"].clone();
    for key in [
        "retainedResidentialIds",
        "displacedResidentialIds",
        "replacementResidentialIds",
    ] {
        assert!(
            !joint[key]
                .as_array()
                .map(|ids| ids.iter().any(|id| id == REFUSED))
                .unwrap_or(false),
            "{key} names the refused lot"
        );
    }
    // 3. no conversion parcel
    assert!(
        !roads["frontage"]["conversions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|conversion| conversion["parcelId"] == REFUSED_PARCEL),
        "the refused lot converts no land"
    );
    // 4. no development-plan building or parcel
    assert!(
        !plan["buildings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|building| building["id"] == format!("{REFUSED}.building")),
        "the refused lot owns no parcel product"
    );
    assert!(
        !plan["parcels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parcel| parcel["id"] == REFUSED_PARCEL),
        "the refused lot owns no parcel"
    );
    // 5. no published entity, and no access link
    assert!(!entities.iter().any(|entity| entity["entityId"] == REFUSED));
    assert!(
        !scene["accessGraph"]["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["entityId"] == REFUSED),
        "the refused lot has no access link"
    );
    // The refusal is published by NAME, on the request that wanted the lot.
    let shops = roads["frontage"]["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outcome| outcome["request"]["id"] == "gorge-shops")
        .unwrap();
    // The refusal is named by the preflight's own vocabulary: the checked route
    // ends on the NEXT street, not on the one this shop is assigned to.
    let refused_for_access = shops["rejected"]["access_to_different_road"]
        .as_u64()
        .unwrap_or(0)
        + shops["rejected"]["no_local_walkable_frontage"]
            .as_u64()
            .unwrap_or(0);
    assert_eq!(
        refused_for_access, 1,
        "the walkable-entrance refusal is published: {shops}"
    );

    // The world's own numbers, and the audit it used to fail.
    let audit = read_json(scene_dir(&dir).join("development-audit.json"));
    assert_eq!(entities.len(), HILLSIDE_ENTITIES);
    assert_eq!(audit["plannedBuildings"], json!(HILLSIDE_PADS));
    assert_eq!(audit["realizedBuildings"], json!(HILLSIDE_PADS));
    assert_eq!(audit["failures"], json!([]));
    assert_eq!(audit["status"], "incomplete");
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity["kind"] == "residential_home")
            .count(),
        HILLSIDE_HOMES
    );
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity["kind"] == "storefront")
            .count(),
        HILLSIDE_SHOPS
    );
}

/// 2. A forged plan cannot buy the lot back: `quality-report` rebuilds the
///    ownership from the manifest and the natural raster, so a building that no
///    rebuilt plan owns fails by name however well its record is written.
#[test]
fn a_forged_plan_building_cannot_buy_the_refused_lot() {
    let dir = TestDir::new("assigned-road-forged");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");
    let path = scene_dir(&dir).join("development-plan.json");
    let mut plan = read_json(path.clone());
    // Copy an honest parcel product - geometry, entity and all - and give it the
    // refused lot's identity. Every field is a plausible declaration; the only
    // thing wrong with it is that no surface carries this lot.
    let mut forged = plan["buildings"][0].clone();
    forged["id"] = json!(format!("{REFUSED}.building"));
    forged["parcelId"] = json!(REFUSED_PARCEL);
    forged["entity"]["entityId"] = json!(REFUSED);
    plan["buildings"].as_array_mut().unwrap().push(forged);
    let mut parcel = plan["parcels"][0].clone();
    parcel["id"] = json!(REFUSED_PARCEL);
    plan["parcels"].as_array_mut().unwrap().push(parcel);
    fs::write(&path, serde_json::to_vec(&plan).unwrap()).unwrap();

    let output = quality_report(&dir);
    assert!(
        !output.status.success(),
        "a forged ownership record must not be accepted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("differs from scene plan") || stderr.contains("independently rebuilt"),
        "the rebuild refuses it by name: {stderr}"
    );
}

/// 3. Two bakes of the access world publish the same bytes, file for file.
#[test]
fn two_bakes_of_the_access_world_are_byte_identical() {
    let first = TestDir::new("assigned-road-bake-1");
    let second = TestDir::new("assigned-road-bake-2");
    bake_world(&sample(), &first.0).expect("the first bake");
    bake_world(&sample(), &second.0).expect("the second bake");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    assert_eq!(
        a.len(),
        b.len(),
        "the two bakes publish a different file set"
    );
    let mut differing = vec![];
    for (path, digest) in &a {
        if path.contains("generation-report") {
            continue;
        }
        if b.get(path) != Some(digest) {
            differing.push(path.clone());
        }
    }
    assert!(differing.is_empty(), "differing files: {differing:?}");
}

/// 4. The closed state does not move: the service world's pads were already
///    consistent, so the new acceptance gate publishes exactly the bytes the
///    previous binary did. One constant over the whole tree.
#[test]
fn the_service_world_publishes_the_frozen_bytes() {
    let dir = TestDir::new("assigned-road-frozen-plains");
    bake_world(&plains(), &dir.0).expect("the plains sample bakes");
    let files = published_files(&dir.0);
    let digest = world_digest(&files);
    assert_eq!(
        digest,
        FROZEN_PLAINS_DIGEST,
        "the closed-state world moved: {digest} ({} files)",
        files.len()
    );
}
