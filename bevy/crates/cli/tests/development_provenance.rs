//! The development audit must be able to tell a building a PARCEL owns from a
//! building the RULE ENGINE placed, and must keep refusing one that is neither.
//!
//! The audit used to subtract the plan's building ids from the scene's building
//! ids, so every procedurally placed structure (`urban_building`, `resort_lodge`,
//! a template building) was reported as `buildings_without_development_plan`.
//! That is not a defect in the world: generation proposes rule structures and the
//! placement rules accept them, and no lot exists for the plan to name them. The
//! fix publishes the generator's own candidate keys as the scene's provenance and
//! counts them apart (`ruleGeneratedBuildings`), while a building with neither a
//! parcel nor a valid origin is still refused.
//!
//! These tests pin the numbers on the committed hillside sample, prove the
//! refusal survives (deleted provenance, forged provenance), and freeze the
//! closed state: a world with no development ledger publishes neither the
//! provenance nor the count, and its bytes are one constant.
use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::process::Command;
use std::{fs, path::PathBuf};

const SAMPLE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
const PLAINS: &str = include_str!("../../../../examples/service-program-plains.manifest.json");

/// The committed hillside town's own numbers, measured from the published world.
const HILLSIDE_ENTITIES: usize = 436;
const HILLSIDE_URBAN_BUILDINGS: usize = 2;
const HILLSIDE_PLANNED: usize = 22;
const HILLSIDE_REALIZED: usize = 22;
const HILLSIDE_RULE_GENERATED: usize = 2;

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

/// The committed plains sample WITHOUT its road-first ledger: the historical
/// generic-land settlement path places seven ordinary buildings and publishes no
/// development plan, so this round's feature is closed on it.
fn no_ledger() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(PLAINS).expect("plains parses");
    manifest.style["generationProfile"] = json!("prototype");
    manifest.style["generationIntent"] = json!({"settlement": "town"});
    for key in [
        "landUseProfile",
        "settlementProgram",
        "frontageProgram",
        "automaticRoadExtensions",
        "agricultureProgram",
    ] {
        manifest.style.as_object_mut().unwrap().remove(key);
    }
    manifest.style["rulesDir"] = json!(rules());
    manifest
}

fn scene_dir(dir: &TestDir) -> PathBuf {
    dir.0.join("scenes/scene-0")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn quality_report(dir: &TestDir) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "quality-report must rebuild the audit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_json(dir.0.join("quality-report.json"))
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
    for (path, digest) in files.iter().filter(|(path, _)| !path.contains("generation-report")) {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(digest.as_bytes());
        hasher.update(b"\0");
    }
    hasher.finalize().to_hex().to_string()
}

fn rewrite_scene(dir: &TestDir, edit: impl FnOnce(&mut Value)) {
    let path = scene_dir(dir).join("scene.json");
    let mut scene: Value = read_json(path.clone());
    edit(&mut scene);
    fs::write(path, serde_json::to_vec(&scene).unwrap()).unwrap();
}

/// 1. The rule-generated buildings are counted apart and leave the failure list.
#[test]
fn the_rule_generated_buildings_are_counted_apart_and_leave_the_failure_list() {
    let dir = TestDir::new("dev-origin-hillside");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");

    let scene = read_json(scene_dir(&dir).join("scene.json"));
    let entities = scene["entities"].as_array().unwrap();
    assert_eq!(entities.len(), HILLSIDE_ENTITIES);
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity["kind"] == "urban_building")
            .count(),
        HILLSIDE_URBAN_BUILDINGS
    );

    // The scene publishes the generator's own candidate keys, one per rule
    // building, and each recomputes the id of a building the plan does not own.
    let origins = scene["ruleBuildingCandidates"].as_array().unwrap();
    assert_eq!(origins.len(), HILLSIDE_RULE_GENERATED);
    let plan = read_json(scene_dir(&dir).join("development-plan.json"));
    let planned: Vec<&str> = plan["buildings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|building| building["entity"]["entityId"].as_str().unwrap())
        .collect();
    for origin in origins {
        assert_eq!(origin["planner"], "urban-building");
        assert_eq!(origin["kind"], "urban_building");
        let key: glyphweave_core::worldgen::EntityCandidateKey =
            serde_json::from_value(origin.clone()).expect("a published candidate key");
        let id = key.stable_id().expect("the key names a stable id");
        assert!(
            entities.iter().any(|entity| entity["entityId"] == id.as_str()),
            "{id} must be a published entity"
        );
        assert!(!planned.contains(&id.as_str()), "{id} is not a parcel's product");
    }

    let audit = read_json(scene_dir(&dir).join("development-audit.json"));
    assert_eq!(audit["plannedBuildings"], json!(HILLSIDE_PLANNED));
    assert_eq!(audit["realizedBuildings"], json!(HILLSIDE_REALIZED));
    assert_eq!(audit["ruleGeneratedBuildings"], json!(HILLSIDE_RULE_GENERATED));
    assert_eq!(audit["unplannedBuildingIds"], json!([]));
    assert!(
        !audit["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "buildings_without_development_plan"),
        "the rule structures no longer masquerade as stray buildings: {audit}"
    );
    // Round 45: the one real gap this test used to pin - a shop pad with no
    // verified assigned road access - is now refused at acceptance, so the audit
    // has no failure left and the world is merely incomplete (the remaining land
    // and the yard surface are ordinary open work, not a refusal).
    assert_eq!(audit["failures"], json!([]));
    assert_eq!(audit["status"], "incomplete");

    // The CLI rebuilds the same audit from the published scene, so the count is
    // not a bake-only artefact.
    let rebuilt = quality_report(&dir);
    let rebuilt_audit = &rebuilt["scenes"][0]["developmentAudit"];
    assert_eq!(
        rebuilt_audit["ruleGeneratedBuildings"],
        json!(HILLSIDE_RULE_GENERATED)
    );
    assert_eq!(rebuilt_audit["unplannedBuildingIds"], json!([]));
    assert_eq!(rebuilt_audit, &audit);
}

/// 2. Removing the provenance is not a way to pass: the building has neither a
///    parcel nor an origin, so the audit refuses it by name.
#[test]
fn deleting_the_published_provenance_brings_the_refusal_back() {
    let dir = TestDir::new("dev-origin-deleted");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");
    rewrite_scene(&dir, |scene| {
        assert_eq!(
            scene["ruleBuildingCandidates"].as_array().unwrap().len(),
            HILLSIDE_RULE_GENERATED
        );
        scene["ruleBuildingCandidates"] = json!([]);
    });
    let rebuilt = quality_report(&dir);
    let audit = &rebuilt["scenes"][0]["developmentAudit"];
    assert_eq!(audit["status"], "fail");
    assert_eq!(audit["ruleGeneratedBuildings"], Value::Null);
    assert_eq!(
        audit["unplannedBuildingIds"].as_array().unwrap().len(),
        HILLSIDE_RULE_GENERATED
    );
    assert!(
        audit["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "buildings_without_development_plan"),
        "{audit}"
    );
}

/// 3. A forged origin is refused by name: a claim that does not recompute the id
///    of anything published cannot launder a building, and the honest claim it
///    sits beside is still counted.
#[test]
fn a_forged_origin_is_refused_by_name() {
    let dir = TestDir::new("dev-origin-forged");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");
    let scene = read_json(scene_dir(&dir).join("scene.json"));
    let honest = scene["ruleBuildingCandidates"][0].clone();

    // A fabricated slot cannot name an entity that is already in the scene: the
    // stable id is a hash of the candidate key, so the claim does not resolve.
    rewrite_scene(&dir, |scene| {
        let mut forged = honest.clone();
        forged["slot"] = json!([1, 2, 3]);
        scene["ruleBuildingCandidates"] = json!([honest, forged]);
    });
    let rebuilt = quality_report(&dir);
    let audit = &rebuilt["scenes"][0]["developmentAudit"];
    assert_eq!(audit["status"], "fail");
    assert_eq!(audit["ruleGeneratedBuildings"], json!(1));
    assert!(
        audit["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure
                .as_str()
                .unwrap()
                .ends_with("rule_building_origin_names_no_published_entity")),
        "{audit}"
    );
}

/// 3b. The same refusal, one layer down: a published entity whose kind disagrees
///     with the key that produced its id is refused, not credited. (The CLI
///     refuses such an edit even earlier - a land-use budget rebuilt from a scene
///     whose building changed kind no longer matches - so this is asserted on the
///     audit body, the layer that owns the provenance rule.)
#[test]
fn an_origin_that_disagrees_with_the_published_kind_is_refused() {
    use glyphweave_core::worldgen::{AccessGraph, EntityCandidateKey};
    let dir = TestDir::new("dev-origin-kind");
    bake_world(&sample(), &dir.0).expect("the committed hillside bakes");
    let mut scene: glyphweave_core::worldgen::SceneIndex =
        serde_json::from_slice(&fs::read(scene_dir(&dir).join("scene.json")).unwrap()).unwrap();
    let road_plan = scene.road_network_plan.clone().expect("the sample plans roads");
    let zones = scene.land_use_zones.clone();
    let plan = scene.development_plan.clone().expect("the sample owns parcels");
    let honest: EntityCandidateKey = scene.rule_building_candidates[0].clone();
    let rule_id = honest.stable_id().unwrap();
    for entity in &mut scene.entities {
        if entity.entity_id == rule_id {
            entity.kind = "storefront".into();
        }
    }
    let spec = glyphweave_core::worldgen::SceneSpec {
        scene_id: scene.scene_id.clone(),
        width_m: scene.width_m,
        depth_m: scene.depth_m,
        origin_x: scene.origin_x,
        origin_z: scene.origin_z,
        seed_offset: 0,
    };
    // This arm is about the provenance rule alone, so the access graph is left
    // empty: every planned building then reports its own missing access, which
    // does not touch the rule-building classification asserted below.
    let access = AccessGraph::default();
    let audit = glyphweave_core::settlement::development::verify(
        &plan,
        &spec,
        &zones,
        &road_plan,
        &scene.entities,
        &access,
        &scene.rule_building_candidates,
    )
    .unwrap();
    assert_eq!(audit.rule_generated_buildings, 1);
    assert!(
        audit.failures.iter().any(|failure| failure.ends_with(
            "rule_building_origin_kind_differs_from_published_entity (origin says \
             urban_building, scene says storefront)"
        )),
        "{audit:?}"
    );
}

/// 4. Closed state, frozen: a world with NO development ledger publishes no
///    provenance and no count at all, and its published bytes are one constant.
///    The constant was measured from the HEAD binary's own output for this
///    fixture, file for file (see the round report); the audit never sees it.
#[test]
fn a_world_without_a_ledger_publishes_no_provenance_and_stays_frozen() {
    let dir = TestDir::new("dev-origin-noplan");
    bake_world(&no_ledger(), &dir.0).expect("the ledger-less world bakes");
    let scene = read_json(scene_dir(&dir).join("scene.json"));
    assert!(
        scene.get("ruleBuildingCandidates").is_none(),
        "no ledger, no provenance: the field must be absent, not empty"
    );
    assert!(
        !scene_dir(&dir).join("development-plan.json").exists(),
        "the fixture must have no development ledger"
    );
    // The world is not empty of rule structures: seven ordinary buildings stand
    // in it, and they are the reason the field would have been published.
    let buildings = scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entity| entity["kind"] == "building")
        .count();
    assert_eq!(buildings, 7);
    assert_eq!(
        world_digest(&published_files(&dir.0)),
        FROZEN_NO_LEDGER_DIGEST,
        "the closed state moved a byte"
    );
}

const FROZEN_NO_LEDGER_DIGEST: &str =
    "aa4383c4ddcf3e6fcf2eafdf28d21a410b2c972577cdacc700ab651dcf337ab6";

/// 5. Two bakes of the provenance-bearing world publish identical bytes.
#[test]
fn two_bakes_of_the_provenance_world_are_byte_identical() {
    let first = TestDir::new("dev-origin-first");
    let second = TestDir::new("dev-origin-second");
    bake_world(&sample(), &first.0).expect("first bake");
    bake_world(&sample(), &second.0).expect("second bake");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert!(moved.is_empty(), "two bakes moved bytes: {moved:?}");
    assert_eq!(world_digest(&a), world_digest(&b));
}
