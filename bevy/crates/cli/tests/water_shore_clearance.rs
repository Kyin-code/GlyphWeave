//! Round 40: the two gate accounts that Round 21 recorded and no round had cleared.
//!
//! 1. **`tree is inside water geometry`** - the generation side asked for 1.0 m of
//!    shore clearance while the published `scale-audit` refuses at 4.0 m, so a
//!    legal bake of the hillside town published three trees on its banks that the
//!    audit then refused. Generation now reads the audit's own constant
//!    (`worldgen::WATER_SHORE_CLEARANCE_M`), so the samples the audit would refuse
//!    are never produced. The audit was NOT loosened: its margin is still 4.0 m.
//! 2. **`strict profile has unruled entity kinds: river`** - the authored channel
//!    was published without a rules descriptor, so a strict profile reported it as
//!    a coverage gap. `assets/objects/river.object.toml` now describes the body,
//!    and the rules engine CHECKS it (`checked_items` grows by exactly one, the
//!    river) instead of disclosing it.
//!
//! What this file asserts, all of it read back from the bytes the bake published
//! and from the release-facing commands:
//!
//! * the hillside town's `scale-audit` passes with **0** tree-in-water failures and
//!   76 trees, and an independent recomputation of the audit's own predicate over
//!   the published entities finds **0** objects inside the 4 m band;
//! * the same world's `rules-audit` reports **no** unruled kind, and the gate no
//!   longer fails on either old account;
//! * the waterfront sample's river is audited: with `river.object.toml` the report
//!   checks 425 items and discloses none, and the SAME world audited against the
//!   same registry minus that one file checks 424 and discloses `river` - so the
//!   one extra checked item is the river, not a coincidence;
//! * a world whose rules are CLOSED (legacy profile, no `rulesMode`) publishes the
//!   same bytes whether or not `river.object.toml` exists in its registry;
//! * two bakes of the waterfront sample publish identical bytes.
use glyphweave_core::worldgen::{
    SceneIndex, WATER_SHORE_CLEARANCE_M, WorldManifest, bake_world, footprint_intersects_water,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::process::Command;
use std::{fs, path::PathBuf};

/// The two published samples this round measures, compiled in so the test cannot
/// drift from the example.
const HILLSIDE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
const WATERFRONT: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");

/// The three trees the HEAD binary published inside the hillside town's 4 m shore
/// band, by entity id. They are the three `scale-audit` failures this round closes.
const HILLSIDE_TREES_IN_WATER: [&str; 3] = [
    "generated.template-tree.stable-v1.13f56ad3a38b52bfb4ad4fa93d33a26a5166951f5d418aa0efc3e17a198896c1",
    "generated.template-tree.stable-v1.a995d52690515c01c8716e945cc54dc645170ec4fedd6ce5dde42f36b36ea806",
    "generated.template-tree.stable-v1.fc317e71e7855bc6e84614a7639ec6171b3f835ddb90eca6211882109f02b0a2",
];
/// The trees the hillside bake keeps: 79 before this round, 76 now.
const HILLSIDE_TREES: usize = 76;

/// The gate's verdict on the hillside town AFTER both accounts are closed. These
/// five failures were already true of the HEAD world (the same `quality-report`
/// audits, unchanged), but the gate is fail-fast: it aborted at `scale-audit`, so
/// they were never printed.
///
/// Round 41 removed the last road failure from this list (`final road network
/// failed independent design audit`, the off arm's 6.705 m / 7.150 m roadbed
/// overfill). A graded road now owns its transport band's fill, so the two auto
/// roads stay inside the 6 m contract and the road audit passes; the four land-use
/// failures that remain are outside this round.
const HILLSIDE_OFF_GATE_FAILURES: [&str; 4] = [
    "\"scene-0\" local-height frontage demand or realization remains incomplete",
    "\"scene-0\" automatic road targets remain out of reach after \"no_feasible_progress_candidate\"",
    "\"scene-0\" purpose-area allocation or realization remains incomplete",
    "\"scene-0\" landcover remains unrealized",
];
/// The waterfront gate's verdict once the river is ruled: the three land-use
/// realization failures that Round 37/39 recorded remain - the fourth
/// (`strict profile has unruled entity kinds: river`) is gone.
const WATERFRONT_GATE_FAILURES: [&str; 3] = [
    "\"scene-0\" local-height frontage demand or realization remains incomplete",
    "\"scene-0\" purpose-area allocation or realization remains incomplete",
    "\"scene-0\" landcover remains unrealized",
];
/// `rules-audit` counts with the river ruled, and without it (the positive control).
const WATERFRONT_CHECKED_WITH_RIVER: u64 = 398;
const WATERFRONT_CHECKED_WITHOUT_RIVER: u64 = 397;

/// The kinds the published `scale-audit` refuses when their footprint enters the
/// shore band (`crates/cli/src/main.rs`), replicated so the test can recompute the
/// audit instead of trusting its own report.
const AUDITED_KINDS: [&str; 12] = [
    "tree",
    "bush",
    "rock",
    "building",
    "urban_building",
    "building_tower",
    "storefront",
    "bench",
    "lamp",
    "grass_clump",
    "fallen_log",
    "building_cluster",
];

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

/// A copy of the shipped registry without `river.object.toml`: the positive
/// control that the one extra checked item of the ruled world IS the river.
fn rules_without_the_river_descriptor(dir: &TestDir) -> PathBuf {
    let pruned = dir.0.join("rules-without-river");
    fs::create_dir(&pruned).unwrap();
    for entry in fs::read_dir(rules()).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().and_then(|n| n.to_str()) == Some("river.object.toml") {
            continue;
        }
        fs::copy(&path, pruned.join(path.file_name().unwrap())).unwrap();
    }
    pruned
}

fn glyphweave() -> &'static str {
    env!("CARGO_BIN_EXE_glyphweave")
}

fn run(verb: &str, world: &std::path::Path) -> std::process::Output {
    Command::new(glyphweave())
        .arg(verb)
        .arg(world)
        .output()
        .expect("the CLI runs")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn scene_index(world: &std::path::Path) -> SceneIndex {
    serde_json::from_slice(&fs::read(world.join("scenes/scene-0/scene.json")).unwrap()).unwrap()
}

/// The hillside sample as committed, with its one repo-relative path made absolute.
fn hillside() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(HILLSIDE).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest
}

/// The registered waterfront sample with the explicit conversion switch.
fn waterfront() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(WATERFRONT).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["frontageProgram"]["ruralConversion"] = json!("explicit");
    manifest
}

/// The same river, with the rules engine CLOSED (`legacy` profile, no `rulesMode`),
/// so no descriptor can govern it and no `rules-audit.json` is published at all.
fn closed_river() -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(WATERFRONT).expect("sample parses");
    manifest.style["rulesDir"] = json!(rules());
    manifest.style["generationProfile"] = json!("legacy");
    manifest.style.as_object_mut().unwrap().remove("rulesMode");
    for key in [
        "automaticRoadExtensions",
        "frontageProgram",
        "settlementProgram",
        "agricultureProgram",
    ] {
        manifest.style.as_object_mut().unwrap().remove(key);
    }
    manifest
}

/// Every published file, keyed by its relative path, hashed with blake3.
fn published_files(root: &std::path::Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
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

/// 1. The hillside town publishes no tree inside its 4 m shore band: the bake's own
///    `scale-audit` says so, and so does an independent recomputation of the same
///    predicate over the published entities.
#[test]
fn the_hillside_banks_publish_no_tree_inside_the_shore_band() {
    let dir = TestDir::new("shore-hillside");
    let manifest = hillside();
    bake_world(&manifest, &dir.0).expect("the committed hillside town bakes");

    let audit = run("scale-audit", &dir.0);
    assert!(
        audit.status.success(),
        "scale-audit must pass once generation keeps the published clearance: {}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let report = read_json(dir.0.join("scale-audit.json"));
    assert_eq!(report["status"], json!("pass"));
    assert_eq!(report["failures"], json!([]), "no object is inside water geometry");
    assert_eq!(report["treeCount"], json!(HILLSIDE_TREES));

    let scene = scene_index(&dir.0);
    for id in HILLSIDE_TREES_IN_WATER {
        assert!(
            !scene.entities.iter().any(|entity| entity.entity_id == id),
            "{id} was the audit's own failure and may no longer be generated"
        );
    }
    // The audit's predicate, recomputed here from the published entities: the
    // generation side may not disagree with it by even one sample.
    let water = {
        let landmarks: Vec<glyphweave_core::worldgen::LandmarkSpec> = manifest
            .landmarks
            .iter()
            .filter(|landmark| landmark.scene_id == "scene-0")
            .cloned()
            .collect();
        let scene_spec = glyphweave_core::worldgen::SceneSpec {
            scene_id: "scene-0".into(),
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            origin_x: scene.origin_x,
            origin_z: scene.origin_z,
            seed_offset: 0,
        };
        glyphweave_core::worldgen::water_geometry(
            glyphweave_core::worldgen::water_kind(&manifest.style),
            &landmarks,
            &scene_spec,
            &manifest.style,
        )
    };
    let mut inside = Vec::new();
    for entity in &scene.entities {
        if !AUDITED_KINDS.contains(&entity.kind.as_str()) {
            continue;
        }
        if footprint_intersects_water(entity, water, WATER_SHORE_CLEARANCE_M) {
            inside.push((entity.entity_id.clone(), entity.kind.clone()));
        }
    }
    assert!(
        inside.is_empty(),
        "the published world still holds objects inside the {} m band: {inside:?}",
        WATER_SHORE_CLEARANCE_M
    );

    // The other old account: the hillside channel is ruled too. `unruled_items` is
    // omitted from the published report when it is empty (`skip_serializing_if`).
    let rules_report = read_json(dir.0.join("rules-audit.json"));
    assert!(
        rules_report.get("unruled_items").is_none(),
        "no entity kind may be disclosed as unruled: {}",
        rules_report["unruled_items"]
    );
    assert_eq!(rules_report["rejected_items"], json!(0));

    // The gate no longer fails on either old account; it now reports the failures
    // that were always true of this world but hidden behind the fail-fast stage.
    let gate = run("world-quality-gate", &dir.0);
    assert!(!gate.status.success(), "the land-use backlog is not this round's");
    let gate_report = read_json(dir.0.join("world-quality-gate.json"));
    let failures: Vec<&str> = gate_report["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|failure| failure.as_str().unwrap())
        .collect();
    assert_eq!(failures, HILLSIDE_OFF_GATE_FAILURES);
    assert!(
        failures.iter().all(|failure| !failure.contains("water geometry")),
        "the tree-in-water account is closed"
    );
    assert!(
        failures.iter().all(|failure| !failure.contains("unruled")),
        "the unruled-river account is closed"
    );
}

/// 2. The waterfront river is CHECKED by the rules engine, not disclosed as an
///    unruled coverage gap. The positive control - the same world against the same
///    registry minus `river.object.toml` - shows the one extra checked item is the
///    river itself.
#[test]
fn the_river_is_checked_by_the_rules_engine_instead_of_disclosed_as_unruled() {
    let dir = TestDir::new("shore-waterfront");
    let manifest = waterfront();
    bake_world(&manifest, &dir.0).expect("the waterfront sample bakes");

    let report = read_json(dir.0.join("rules-audit.json"));
    assert!(
        report.get("unruled_items").is_none(),
        "the river may not be disclosed as unruled any more: {}",
        report["unruled_items"]
    );
    assert_eq!(report["checked_items"], json!(WATERFRONT_CHECKED_WITH_RIVER));
    assert_eq!(report["passed_items"], json!(WATERFRONT_CHECKED_WITH_RIVER));
    assert_eq!(report["rejected_items"], json!(0));
    assert_eq!(report["rejects"], json!([]));
    let scene = scene_index(&dir.0);
    assert_eq!(
        scene.entities.iter().filter(|entity| entity.kind == "river").count(),
        1,
        "the body the report now checks is the authored river"
    );

    // The positive control: the same published world, the same registry minus the
    // river descriptor, discloses exactly one unruled kind and checks one fewer item.
    let pruned = rules_without_the_river_descriptor(&dir);
    let control_report_path = dir.0.join("rules-audit-control.json");
    let output = Command::new(glyphweave())
        .args(["rules", "audit"])
        .arg(&dir.0)
        .arg("--rules")
        .arg(&pruned)
        .arg("--report")
        .arg(&control_report_path)
        .output()
        .expect("the CLI runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("checked={WATERFRONT_CHECKED_WITHOUT_RIVER}"))
            && stdout.contains("unruled=1"),
        "the control audit must disclose the river: {stdout}"
    );
    let control = read_json(control_report_path);
    assert_eq!(control["unruled_items"], json!(["river"]));
    assert_eq!(control["checked_items"], json!(WATERFRONT_CHECKED_WITHOUT_RIVER));
    assert_eq!(
        control["passed_items"],
        json!(WATERFRONT_CHECKED_WITHOUT_RIVER)
    );

    // The ruled audit says the same thing when the CLI re-runs it on the world.
    let ruled_path = dir.0.join("rules-audit-cli.json");
    let output = Command::new(glyphweave())
        .args(["rules", "audit"])
        .arg(&dir.0)
        .arg("--rules")
        .arg(rules())
        .arg("--report")
        .arg(&ruled_path)
        .output()
        .expect("the CLI runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("checked={WATERFRONT_CHECKED_WITH_RIVER}"))
            && stdout.contains("rejected=0")
            && stdout.contains("unruled=0"),
        "the river must be checked and pass: {stdout}"
    );

    // The gate: the three land-use failures remain, the unruled account is gone.
    let gate = run("world-quality-gate", &dir.0);
    assert!(!gate.status.success());
    let gate_report = read_json(dir.0.join("world-quality-gate.json"));
    let failures: Vec<&str> = gate_report["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|failure| failure.as_str().unwrap())
        .collect();
    assert_eq!(failures, WATERFRONT_GATE_FAILURES);
}

/// 3. The river rule reaches no world whose rules are closed: the same manifest and
///    the same registry PATH publish the same bytes whether or not
///    `river.object.toml` is present. The registry is mutated between bakes, so the
///    manifest (and the world revision derived from it) cannot drift.
#[test]
fn the_river_rule_reaches_no_world_whose_rules_are_closed() {
    let dir = TestDir::new("shore-closed");
    let registry = dir.0.join("registry");
    fs::create_dir(&registry).unwrap();
    for entry in fs::read_dir(rules()).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().and_then(|n| n.to_str()) == Some("river.object.toml") {
            continue;
        }
        fs::copy(&path, registry.join(path.file_name().unwrap())).unwrap();
    }
    let mut manifest = closed_river();
    manifest.style["rulesDir"] = json!(registry);
    assert!(manifest.style.get("rulesMode").is_none());

    let without = dir.0.join("without");
    bake_world(&manifest, &without).expect("a rules-closed river world bakes");
    fs::copy(
        rules().join("river.object.toml"),
        registry.join("river.object.toml"),
    )
    .unwrap();
    let with = dir.0.join("with");
    bake_world(&manifest, &with).expect("the same world bakes with the river rule present");

    // A closed world publishes no rules report at all, so no descriptor can govern
    // (or refuse) its river; the channel itself is still published.
    for world in [&without, &with] {
        assert!(!world.join("rules-audit.json").exists());
        assert!(!world.join("rules-placement.json").exists());
        assert_eq!(scene_index(world).entities.iter().filter(|e| e.kind == "river").count(), 1);
    }
    let mut a = published_files(&without);
    let mut b = published_files(&with);
    // `generation-report.json` records the output directory and therefore always
    // differs between two folders; it is not world content.
    a.remove("generation-report.json");
    b.remove("generation-report.json");
    assert_eq!(
        a, b,
        "a rules-closed world must not see the river descriptor at all"
    );
}

/// 4. Two bakes of the waterfront sample publish identical bytes, the rules report
///    (whose counts moved this round) included.
#[test]
fn two_waterfront_bakes_are_byte_identical() {
    let first = TestDir::new("shore-two-bakes-a");
    let second = TestDir::new("shore-two-bakes-b");
    let manifest = waterfront();
    bake_world(&manifest, &first.0).expect("first bake");
    bake_world(&manifest, &second.0).expect("second bake");
    let mut a = published_files(&first.0);
    let mut b = published_files(&second.0);
    a.remove("generation-report.json");
    b.remove("generation-report.json");
    assert_eq!(a, b, "two bakes of one manifest must publish the same bytes");
}
