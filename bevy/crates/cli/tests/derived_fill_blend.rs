//! Round 54: `style.terrainPolicy.earthworkBlend = "derived-from-fill"` derives each
//! pad's and each road's fill-transition ring from the FILL it actually places,
//! instead of from the platform's elevation (`3 x (ground + 32)`) or from the
//! structure's size (`"derived-from-footprint"`, round 34).
//!
//! The defect: the elevation ring is a function of the plate's height and never of
//! the earthwork, so a 6 m platform publishes `3 x (6 + 32) = 114 m` - and the 1 km
//! probe's 10 m plates publish 126 m - which lets a neighbour 58..106 m away raise
//! the ground around another lot's door. Round 53 measured the consequence
//! (`assigned_road_unreachable_after_pad_list_complete`, one lot withdrawn at 23.15 m
//! from its own street) and named the ring as the root cause.
//!
//! The value: `ring = clamp(fill_depth / SIDE_SLOPE_GRADE + BLEND_APRON_M, FLOOR,
//! CEILING)` with `fill_depth = max(0, design - natural_min)` - the same difference
//! the project's 6 m roadbed fill contract already reads - and `SIDE_SLOPE_GRADE =
//! 0.5`, the 1:2 (H:V) fill batter that the legacy ring's own `3` encodes as
//! `1.5 / 0.5`. A 6 m fill therefore publishes `6 / 0.5 + 8 = 20 m`, not 114 m.
//!
//! What this file asserts, all of it read back from the bytes a bake published:
//!
//! 1. every planned pad's ring IS the formula, evaluated on that pad's own published
//!    record (`supportLevelMm` over `naturalMinM`), and the entity published beside
//!    the record carries the same number;
//! 2. the CLOSED arm still publishes the elevation ring on every building and road
//!    carve, and no `earthworkBlendM` key anywhere - the freeze that makes the value
//!    opt-in rather than a change of the default world;
//! 3. a ROAD keeps its corridor ring as a floor and loses its elevation ring: the
//!    widest road carve falls from 121.071 m to 19.18 m, and every roadbed carve's
//!    published ring is the one its plan record carries (the independent road audit
//!    compares the two, so a split would be refused by name);
//! 4. the OLD value (`"derived-from-footprint"`, round 34) keeps its own kind set -
//!    pads only, roads untouched - and its own formula, unchanged;
//! 5. two bakes of the fill arm publish identical bytes;
//! 6. what the value costs: the hillside town keeps all 22 pads, 15 homes, 6 shops and
//!    the same 24 named refusals, while the registered waterfront sample loses exactly
//!    ONE pad - the bank church, to the round-53 access refusal - because its frontage
//!    relied on the 111 m neighbours' fill to form a plateau. That is reported here as
//!    a fact, not smoothed over: the fill ring is a 1:2 batter and a batter is not a
//!    pedestrian route.
//!
//! The 1 km replay is `#[ignore]`d (one bake of a 1 km scene); set
//! `GLYPHWEAVE_AUDIT_WORLD` to an already baked `derived-from-fill` probe to audit
//! that world instead of baking a new one.
use glyphweave_core::worldgen::earthwork_blend::{
    BLEND_APRON_M, BLEND_CEILING_M, BLEND_FLOOR_M, Policy, SIDE_SLOPE_GRADE, derived_fill_blend_m,
    policy,
};
use glyphweave_core::worldgen::{SceneIndex, WorldManifest, bake_world};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The published samples, compiled in: the test cannot drift from the examples.
const HILLSIDE: &str = include_str!("../../../../examples/hillside-town-512.manifest.json");
const WATERFRONT: &str = include_str!("../../../../examples/waterfront-city-512.manifest.json");

const FILL: &str = "derived-from-fill";
const FOOTPRINT: &str = "derived-from-footprint";

/// `3 x (6 + 32)`: the elevation ring of the plate this round is about.
const LEGACY_SIX_METRE_RING: f64 = 114.0;
/// `6 / 0.5 + 8`: the fill ring of the same plate, and the run a 1:2 batter spends.
const FILL_SIX_METRE_RING: f64 = 20.0;
const FILL_SIX_METRE_RUN: f64 = 12.0;

/// A directory a test owns, or - for a world the caller NAMED - one it may only
/// read. The flag is what keeps `GLYPHWEAVE_AUDIT_WORLD` from deleting the world it
/// was asked to audit.
struct TestDir(PathBuf, bool);
impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glyphweave-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path, true)
    }
    fn borrowed(path: PathBuf) -> Self {
        Self(path, false)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        if self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
}

/// One sample with exactly one declared `earthworkBlend`, or none.
fn sample(source: &str, blend: Option<&str>) -> WorldManifest {
    let mut manifest: WorldManifest = serde_json::from_str(source).expect("the sample parses");
    manifest.style["rulesDir"] = json!(rules());
    match blend {
        Some(value) => manifest.style["terrainPolicy"] = json!({ "earthworkBlend": value }),
        None => {
            manifest
                .style
                .as_object_mut()
                .map(|style| style.remove("terrainPolicy"));
        }
    }
    manifest
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &fs::read(path)
            .unwrap_or_else(|error| panic!("{} is not published: {error}", path.display())),
    )
    .unwrap()
}

fn scene_dir(dir: &TestDir) -> PathBuf {
    dir.0.join("scenes/scene-0")
}

fn scene_index(dir: &TestDir) -> SceneIndex {
    serde_json::from_slice(&fs::read(scene_dir(dir).join("scene.json")).unwrap()).unwrap()
}

fn plan(dir: &TestDir) -> Value {
    read_json(&scene_dir(dir).join("road-network-plan.json"))
}

fn ledger(dir: &TestDir) -> Vec<Value> {
    read_json(&scene_dir(dir).join("engineered-terrain.json"))["operations"]
        .as_array()
        .expect("the ledger publishes its operations")
        .clone()
}

fn published_files(root: &Path) -> BTreeMap<String, String> {
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

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0e-3
}

/// The ring the fill rule publishes for one pad RECORD, derived from the record
/// alone: its own declared top over its own published natural minimum.
fn record_fill_ring(site: &Value) -> f64 {
    let top = match site["supportLevelMm"].as_i64() {
        Some(mm) => mm as f64 / 1000.,
        None => site["supportLevelM"].as_f64().expect("a declared top"),
    };
    let natural_min = site["naturalMinM"].as_f64().expect("a natural minimum");
    let depth = (top - natural_min).max(0.0);
    (depth / SIDE_SLOPE_GRADE + BLEND_APRON_M).clamp(BLEND_FLOOR_M, BLEND_CEILING_M)
}

/// Every building carve `(entity id, ring)` and every roadbed carve, off the
/// published earthwork ledger. The 2 m entrance aprons are their own carve, not a
/// pad's ring.
fn carves(dir: &TestDir) -> (Vec<(String, f64)>, Vec<(String, f64)>) {
    let mut pads = Vec::new();
    let mut roads = Vec::new();
    for operation in ledger(dir) {
        let id = operation["entityId"].as_str().unwrap_or_default().to_owned();
        let ring = operation["blendM"].as_f64().expect("a published ring");
        if operation["kind"] == json!("roadbed") {
            roads.push((id, ring));
        } else if ring > 2.5 {
            pads.push((id, ring));
        }
    }
    (pads, roads)
}

fn widest(rings: &[(String, f64)]) -> f64 {
    rings.iter().map(|(_, ring)| *ring).fold(0.0_f64, f64::max)
}

/// 1. Every planned pad's ring IS the formula, on that pad's own record, and the
///    entity the world publishes beside the record carries the same number.
#[test]
fn every_planned_pad_publishes_the_ring_its_own_fill_depth_derives() {
    let dir = TestDir::new("fill-pads");
    bake_world(&sample(HILLSIDE, Some(FILL)), &dir.0).expect("the fill arm bakes");
    let plan = plan(&dir);
    let sites = plan["roadsideSites"]
        .as_array()
        .expect("the plan publishes pads");
    assert_eq!(sites.len(), 22, "the hillside town still plans 22 pads");
    let scene = scene_index(&dir);
    let mut rings = Vec::new();
    for site in sites {
        let recorded = site["earthworkBlendM"]
            .as_f64()
            .unwrap_or_else(|| panic!("{} carries no ring", site["id"]));
        let want = record_fill_ring(site);
        assert!(
            close(recorded, want),
            "{} publishes {recorded} where its own record derives {want}",
            site["id"]
        );
        // The same number on the published entity: one ring, both sides.
        let entity = scene
            .entities
            .iter()
            .find(|entity| entity.entity_id == site["id"].as_str().unwrap_or_default())
            .unwrap_or_else(|| panic!("{} has no entity", site["id"]));
        let on_entity = entity
            .grounding
            .earthwork_blend_m
            .unwrap_or_else(|| panic!("{}'s entity carries no ring", site["id"]));
        assert!(
            close(f64::from(on_entity), recorded),
            "{}: record {recorded} vs entity {on_entity}",
            site["id"]
        );
        rings.push(recorded);
    }
    rings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        close(rings[0], 8.178),
        "the narrowest pad ring (0.356 m of fill): {}",
        rings[0]
    );
    assert_eq!(
        rings[rings.len() - 1],
        17.0,
        "the widest pad ring (4.5 m of fill)"
    );
    assert!(
        rings.iter().all(|ring| *ring >= BLEND_FLOOR_M && *ring <= 18.0),
        "every pad ring stays inside the floor and well under the ceiling: {rings:?}"
    );
    // And the world's building carves are the same numbers.
    let (pads, _) = carves(&dir);
    assert_eq!(pads.len(), 24, "22 pads + 2 rule-engine buildings");
    let widest_pad = widest(&pads);
    assert!(close(widest_pad, 18.202), "the widest building carve: {widest_pad}");
}

/// 2. The CLOSED arm is the elevation ring, unchanged, and carries no ring key at
///    all. This is the freeze the value is opt-in for.
#[test]
fn the_closed_arm_still_publishes_the_elevation_ring_and_no_ring_key() {
    let dir = TestDir::new("fill-closed");
    bake_world(&sample(HILLSIDE, None), &dir.0).expect("the closed arm bakes");
    let plan = plan(&dir);
    for site in plan["roadsideSites"].as_array().unwrap() {
        assert!(
            site.get("earthworkBlendM").is_none(),
            "{} publishes a ring the world never asked for",
            site["id"]
        );
    }
    for edge in plan["edges"].as_array().unwrap() {
        assert!(
            edge.get("earthworkBlendM").is_none(),
            "{} publishes a ring the world never asked for",
            edge["id"]
        );
    }
    assert!(
        scene_index(&dir)
            .entities
            .iter()
            .all(|entity| entity.grounding.earthwork_blend_m.is_none()),
        "no entity of a world that never asked carries a ring"
    );
    // Every carve is `3 x (ground + 32)`, straight off the published ledger.
    for operation in ledger(&dir) {
        let ring = operation["blendM"].as_f64().unwrap();
        if ring <= 2.5 {
            continue;
        }
        let target = operation["targetHeightM"]
            .as_f64()
            .expect("a published target");
        let want = (3.0 * (target + 32.0)).max(6.0);
        assert!(
            close(ring, want),
            "{} publishes {ring} where the elevation formula gives {want}",
            operation["entityId"]
        );
    }
    let (pads, roads) = carves(&dir);
    assert!(
        close(widest(&pads), 144.0),
        "the 16 m plate's 144 m ring: {}",
        widest(&pads)
    );
    assert_eq!(roads.len(), 28);
    assert!(
        close(widest(&roads), 121.071),
        "the widest road carve is the elevation ring: {}",
        widest(&roads)
    );
}

/// 3. A road keeps its corridor ring as a FLOOR and loses its elevation ring, and
///    the ring its carve publishes is the one its own plan record carries - the
///    agreement the independent road audit compares byte for byte.
#[test]
fn a_road_keeps_its_corridor_floor_and_publishes_its_recorded_ring() {
    let dir = TestDir::new("fill-roads");
    bake_world(&sample(HILLSIDE, Some(FILL)), &dir.0).expect("the fill arm bakes");
    let plan = plan(&dir);
    let (_, roads) = carves(&dir);
    assert_eq!(roads.len(), 28, "28 road carves");
    for edge in plan["edges"].as_array().unwrap() {
        let id = edge["id"].as_str().unwrap();
        let Some(recorded) = edge["earthworkBlendM"].as_f64() else {
            // A registered STRUCTURE DECK places no embankment: no ring, no carve.
            continue;
        };
        assert!(
            recorded >= 6.0,
            "{id} publishes {recorded} below the corridor's own ring"
        );
        if let Some((_, carved)) = roads.iter().find(|(entity, _)| entity == id) {
            assert!(
                close(*carved, recorded),
                "{id}: the carve publishes {carved} and the record {recorded}"
            );
        }
    }
    assert!(
        close(widest(&roads), 19.18),
        "the widest road carve: {}",
        widest(&roads)
    );
    assert!(
        widest(&roads) < 121.071,
        "the corridor ring is narrower than the elevation ring it replaces"
    );
}

/// 4. The OLD value keeps its own kind set and its own formula: pads only, roads
///    untouched, `max(width, height) + 8`.
#[test]
fn the_old_value_keeps_its_own_rings_and_never_touches_a_road() {
    let dir = TestDir::new("fill-footprint");
    bake_world(&sample(HILLSIDE, Some(FOOTPRINT)), &dir.0)
        .expect("the footprint arm bakes");
    let plan = plan(&dir);
    let mut narrowest = f64::INFINITY;
    for site in plan["roadsideSites"].as_array().unwrap() {
        let recorded = site["earthworkBlendM"]
            .as_f64()
            .expect("the record's ring");
        let width = site["widthM"].as_f64().unwrap();
        let height = site["program"]["heightM"].as_f64().unwrap_or(8.0);
        let want = (width.max(height) + BLEND_APRON_M).clamp(BLEND_FLOOR_M, BLEND_CEILING_M);
        assert!(
            close(recorded, want),
            "{}: publishes {recorded}, the footprint formula gives {want}",
            site["id"]
        );
        narrowest = narrowest.min(recorded);
    }
    assert_eq!(
        narrowest, 16.0,
        "an 8x8 m pad's footprint ring, and still wider than the widest fill ring"
    );
    for edge in plan["edges"].as_array().unwrap() {
        assert!(
            edge.get("earthworkBlendM").is_none(),
            "{}: the footprint value is about structures, not roads",
            edge["id"]
        );
    }
    let (_, roads) = carves(&dir);
    assert!(
        close(widest(&roads), 121.071),
        "every road keeps the elevation ring under the old value"
    );
}

/// 5. Two bakes of the fill arm publish identical bytes.
#[test]
fn two_fill_bakes_are_byte_identical() {
    let first = TestDir::new("fill-twice-a");
    let second = TestDir::new("fill-twice-b");
    let manifest = sample(HILLSIDE, Some(FILL));
    bake_world(&manifest, &first.0).expect("the fill arm bakes");
    bake_world(&manifest, &second.0).expect("the fill arm bakes twice");
    let a = published_files(&first.0);
    let b = published_files(&second.0);
    let moved: Vec<&String> = a
        .iter()
        .filter(|(path, digest)| b.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .collect();
    assert!(moved.is_empty(), "fill bytes moved: {moved:?}");
}

/// 6. The fill ring table: 6 m of fill is 12 m of 1:2 run plus the apron, and it is
///    a fifth of the elevation ring on the same plate.
#[test]
fn a_six_metre_fill_publishes_twenty_metres_not_a_hundred_and_fourteen() {
    assert_eq!(FILL_SIX_METRE_RUN, 6.0 / SIDE_SLOPE_GRADE);
    assert_eq!(derived_fill_blend_m(6.0), FILL_SIX_METRE_RING);
    assert_eq!(
        derived_fill_blend_m(6.0),
        FILL_SIX_METRE_RUN + BLEND_APRON_M
    );
    assert_eq!(LEGACY_SIX_METRE_RING, 3.0 * (6.0 + 32.0));
    assert!(derived_fill_blend_m(6.0) < LEGACY_SIX_METRE_RING / 5.0);
    // The value the generator's own parser reads, not just the text.
    assert_eq!(
        policy(&json!({"terrainPolicy": {"earthworkBlend": FILL}})),
        Policy::Fill
    );
    assert_eq!(policy(&json!({})), Policy::Off);
}

/// 7. What the value does to the two registered 512 m worlds, measured pad by pad.
///    The hillside town is unchanged; the waterfront loses ONE pad, and the refusal
///    is the round-53 category, by name.
#[test]
fn the_fill_value_keeps_the_hillside_and_costs_the_waterfront_one_pad() {
    let closed = TestDir::new("fill-hillside-closed");
    let fill = TestDir::new("fill-hillside-fill");
    bake_world(&sample(HILLSIDE, None), &closed.0).expect("the closed hillside bakes");
    bake_world(&sample(HILLSIDE, Some(FILL)), &fill.0).expect("the fill hillside bakes");
    let (closed_plan, fill_plan) = (plan(&closed), plan(&fill));
    let names = |plan: &Value| -> Vec<String> {
        plan["roadsideSites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|site| site["id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names(&closed_plan), names(&fill_plan), "the same pads, in order");
    assert_eq!(
        closed_plan["roadsideSiteRejections"], fill_plan["roadsideSiteRejections"],
        "the same named refusals"
    );
    let kinds = |dir: &TestDir| -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for entity in &scene_index(dir).entities {
            *counts.entry(entity.kind.clone()).or_insert(0) += 1;
        }
        counts
    };
    let (closed_kinds, fill_kinds) = (kinds(&closed), kinds(&fill));
    assert_eq!(
        closed_kinds.get("residential_home"),
        fill_kinds.get("residential_home")
    );
    assert_eq!(
        closed_kinds.get("storefront"),
        fill_kinds.get("storefront")
    );
    assert_eq!(fill_kinds.get("residential_home"), Some(&15));
    assert_eq!(fill_kinds.get("storefront"), Some(&6));

    let wf_closed = TestDir::new("fill-waterfront-closed");
    let wf_fill = TestDir::new("fill-waterfront-fill");
    bake_world(&sample(WATERFRONT, None), &wf_closed.0).expect("the closed waterfront bakes");
    bake_world(&sample(WATERFRONT, Some(FILL)), &wf_fill.0)
        .expect("the fill waterfront bakes");
    let (wf_closed_plan, wf_fill_plan) = (plan(&wf_closed), plan(&wf_fill));
    assert_eq!(
        wf_closed_plan["roadsideSites"].as_array().unwrap().len(),
        29,
        "the registered sample plans 29 pads when closed"
    );
    assert_eq!(
        wf_fill_plan["roadsideSites"].as_array().unwrap().len(),
        28,
        "the fill ring costs this world exactly one pad"
    );
    let refusals = wf_fill_plan["roadsideSiteRejections"].as_array().unwrap();
    let loss: Vec<&Value> = refusals
        .iter()
        .filter(|refusal| {
            refusal["category"] == json!("assigned_road_unreachable_after_pad_list_complete")
        })
        .collect();
    assert_eq!(loss.len(), 1, "one named access refusal: {refusals:#?}");
    assert_eq!(
        loss[0]["edgeId"].as_str(),
        Some("generated.service.scene-0.bank-church.0")
    );
    assert!(
        loss[0]["reason"]
            .as_str()
            .unwrap()
            .contains("no checked pedestrian route"),
        "{}",
        loss[0]["reason"]
    );
}

// ---------------------------------------------------------------------------
// The 1 km replay: measured once, at round 54, from the opt-in probe plus the one
// extra key `style.terrainPolicy.earthworkBlend = "derived-from-fill"`.
// ---------------------------------------------------------------------------

const PADS_FILL: u64 = 41;
/// `entityKinds.residential_home`: the closed probe publishes 15, this value 13 -
/// the round-53 buried lot RETURNS (`generated.residential-recovery.scene-0.3`, +1)
/// and three frontage homes are refused instead
/// (`pad_fill_conflicts_with_neighbour`, -3). The net is -2, and it is the reason
/// this value does not close the housing gap on this world: it removes the burying,
/// it does not reconcile a 2.3 m level difference between two lots 32 m apart.
const HOMES_FILL: u64 = 13;
const SHOPS_FILL: u64 = 24;
/// The joint layout's own count: 7 realized of 18 asked, one MORE than the closed
/// probe's 6 - the recovered lot is a joint-layout slot, the three lost homes are
/// frontage homes the joint layout never counted.
const JOINT_REALIZED_FILL: u64 = 7;
const JOINT_MISSING_FILL: u64 = 11;
/// The widest pad ring the fill value publishes: `5.802 / 0.5 + 8`.
const WIDEST_PAD_RING_FILL: f64 = 19.604;
/// The closed probe's widest pad ring, for the same world.
const WIDEST_PAD_RING_CLOSED: f64 = 126.105;
const PAD_ENVELOPE_FILL: f64 = 79_645.0;
const PAD_ENVELOPE_CLOSED: f64 = 2_325_735.0;
const CUT_CELLS_FILL: u64 = 28_208;
const CUT_DEPTH_SUM_MM_FILL: u64 = 7_069_173;
const CUT_MAX_DEPTH_MM_FILL: u64 = 1_519;

#[ignore = "1 km bake; set GLYPHWEAVE_AUDIT_WORLD to an already baked world to skip it"]
#[test]
fn the_fill_value_replays_the_one_km_probe() {
    let named = std::env::var_os("GLYPHWEAVE_AUDIT_WORLD").map(PathBuf::from);
    let owned = named.is_none().then(|| TestDir::new("fill-1km"));
    if let Some(dir) = &owned {
        let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../examples/b01-1km-probe-optin.manifest.json");
        let mut manifest: WorldManifest =
            serde_json::from_slice(&fs::read(&example).unwrap()).unwrap();
        manifest.style["terrainPolicy"] = json!({ "earthworkBlend": FILL });
        bake_world(&manifest, &dir.0).expect("the 1 km fill arm bakes");
    }
    // A world the caller named is AUDITED, never deleted: only the temporary one
    // this test baked is removed when the test ends.
    let dir = match &named {
        Some(path) => TestDir::borrowed(path.clone()),
        None => TestDir::borrowed(owned.as_ref().expect("one source").0.clone()),
    };
    // The gate writes the census and the failure list the numbers below are read
    // from; it exits non-zero on this incomplete world, which is expected.
    let gate = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("world-quality-gate")
        .arg(&dir.0)
        .output()
        .expect("world-quality-gate runs");
    assert!(!gate.status.success(), "this probe is still an incomplete world");

    let quality = read_json(&dir.0.join("quality-report.json"))["scenes"][0].clone();
    let kinds = &quality["entityKinds"];
    assert_eq!(
        quality["developmentAudit"]["plannedBuildings"],
        json!(PADS_FILL)
    );
    assert_eq!(kinds["residential_home"], json!(HOMES_FILL));
    assert_eq!(kinds["storefront"], json!(SHOPS_FILL));
    let joint = read_json(&scene_dir(&dir).join("joint-layout-audit.json"));
    assert_eq!(
        joint["realizedResidentialWithAccess"],
        json!(JOINT_REALIZED_FILL)
    );
    assert_eq!(joint["residentialMinimum"], json!(18));
    assert_eq!(joint["missingResidential"], json!(JOINT_MISSING_FILL));

    let plan = plan(&dir);
    let categories: BTreeMap<String, usize> =
        plan["roadsideSiteRejections"]
            .as_array()
            .unwrap()
            .iter()
            .fold(BTreeMap::new(), |mut acc, refusal| {
                *acc.entry(refusal["category"].as_str().unwrap().to_owned())
                    .or_insert(0) += 1;
                acc
            });
    assert_eq!(
        categories
            .get("assigned_road_unreachable_after_pad_list_complete")
            .copied()
            .unwrap_or(0),
        0,
        "the round-53 buried door is reachable again: {categories:?}"
    );
    assert_eq!(
        categories
            .get("pad_fill_conflicts_with_neighbour")
            .copied()
            .unwrap_or(0),
        3
    );

    let (pads, roads) = carves(&dir);
    assert!(close(widest(&pads), WIDEST_PAD_RING_FILL), "{}", widest(&pads));
    assert!(
        widest(&pads) < WIDEST_PAD_RING_CLOSED / 6.0,
        "the widest pad ring is more than six times narrower"
    );
    assert!(!roads.is_empty());
    let envelope: f64 = ledger(&dir)
        .iter()
        .filter(|o| o["kind"] != json!("roadbed") && o["blendM"].as_f64().unwrap() > 2.5)
        .map(|o| {
            4.0 * (o["halfWidthM"].as_f64().unwrap() + o["blendM"].as_f64().unwrap())
                * (o["halfDepthM"].as_f64().unwrap() + o["blendM"].as_f64().unwrap())
        })
        .sum();
    assert!((envelope - PAD_ENVELOPE_FILL).abs() < 2.0, "{envelope}");
    assert!(envelope * 25.0 < PAD_ENVELOPE_CLOSED);
    let cut = read_json(&scene_dir(&dir).join("engineered-terrain.json"))["boundedCut"].clone();
    assert_eq!(cut["cutCells"], json!(CUT_CELLS_FILL));
    assert_eq!(cut["cutDepthSumMm"], json!(CUT_DEPTH_SUM_MM_FILL));
    assert_eq!(cut["maxCutDepthMm"], json!(CUT_MAX_DEPTH_MM_FILL));
    let gate = read_json(&dir.0.join("world-quality-gate.json"));
    let failures = gate["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 7, "{failures:#?}");
    assert!(
        !failures.iter().any(|failure| failure
            .as_str()
            .unwrap_or("")
            .contains("unverified building access")),
        "{failures:#?}"
    );
    assert!(
        failures.iter().any(|failure| failure
            .as_str()
            .unwrap_or("")
            .contains("joint frontage housing minimum remains unmet")),
        "{failures:#?}"
    );
}
