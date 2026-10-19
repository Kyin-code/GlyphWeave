//! Round 51/53: the 1 km probe's opt-in state is a repository artefact, not a commit
//! body.
//!
//! `examples/b01-1km-probe.manifest.json` declares none of the switches the probe's
//! numbers depend on - it is the closed state - so the numbers could only be cited
//! from a commit message. `examples/b01-1km-probe-optin.manifest.json` is that input:
//! the probe, byte for byte, plus exactly four declared keys.
//!
//! Two layers, because the two failure modes are different:
//!
//!  * the DEFAULT tests parse both manifests and pin (a) every switch with the value
//!    the generators themselves read - not just the text - and (b) the "nothing else
//!    moved" contract, so deleting a switch or editing a seed turns into a red test
//!    instead of a silently different world;
//!  * the `#[ignore]`d test bakes the opt-in manifest end to end (1 km; ~17 s with a
//!    release binary, a few minutes with this test's own debug binary) and pins the
//!    numbers this file quotes. Set `GLYPHWEAVE_AUDIT_WORLD` to an already baked
//!    output directory to audit that world instead of baking.
//!
//! Round 53 measured the last unpaid entrance on this world and paid it:
//! `generated.residential-recovery.scene-0.3` passed `assigned_road_gate` when the
//! joint layout accepted it, and the frontage programme - which runs afterwards - then
//! raised the ground around its door above the lot's own declared top, so the bake's
//! access audit found no walkable route to the street 23.15 m away. The plan now asks
//! the same gate again once the pad list is whole
//! (`worldgen::road_network::refuse_entrances_lost_after_pad_list`) and refuses that
//! lot BY NAME, with the measured detour, before it becomes an entity. The numbers
//! below are the world that publishes after that refusal: 43 pads, 15 homes, 24
//! shops, no unverified entrance, no disconnected entity, 7 gate failures - and the
//! housing minimum the recovery pads exist to close is still 12 short, which is the
//! honest state of this probe rather than a hidden access failure.
use glyphweave_core::worldgen::{bounded_cut, frontage, urban_growth};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The closed probe. Compiled in, so the test cannot silently read another file.
const BASE: &str = include_str!("../../../../examples/b01-1km-probe.manifest.json");
/// The probe plus the four switches the quoted numbers were measured with.
const OPT_IN: &str = include_str!("../../../../examples/b01-1km-probe-optin.manifest.json");

/// The acceptance failure that names an entrance the bake cannot verify. The
/// development audit appends it to the pad's entity id.
const UNVERIFIED_ACCESS: &str = "no_verified_assigned_road_access";

/// The pad round 53 removed, and the category the plan publishes the refusal under.
/// It is the joint-frontage recovery home that stood on road
/// `generated.road-network.scene-0.125-126` with a declared top of -1.000 m: the
/// frontage programme's fill rings then put 0.72 m on the ground two metres north of
/// its door, a 106.7 % step where the 25 % walkable contract allows no step at all.
const REFUSED_PAD: &str = "generated.residential-recovery.scene-0.3";
const REFUSAL_CATEGORY: &str = "assigned_road_unreachable_after_pad_list_complete";
/// The measured distance from that lot's door to the nearest street: the pavement it
/// was assigned, whose rectangle ends 23.15 m away.
const REFUSAL_DETOUR: &str =
    "its door is 23.15 m from the nearest street (generated.road-network.scene-0.125-126)";

// ---------------------------------------------------------------------------
// What the world publishes. MEASURED at round 53 from
// `examples/b01-1km-probe-optin.manifest.json` with the released binary; the ledger's
// historical "0 unverified / 7 gate lines" and round 51's "44 pads / 16 homes / 1
// unverified / 8 gate lines" are both superseded by the refusal documented above.
// ---------------------------------------------------------------------------

/// `developmentAudit.plannedBuildings`: 15 homes + 24 shops + 4 services.
const PADS: u64 = 43;
/// `entityKinds.residential_home`: 3 retained roadside homes + 3 recovery homes + 9
/// frontage homes. The refused lot is not one of them.
const HOMES: u64 = 15;
/// `entityKinds.storefront`: the shop request is fully met.
const SHOPS: u64 = 24;
/// Pads whose entrance the post-bake audit could not verify.
const UNVERIFIED_ENTRANCES: usize = 0;
/// Buildings the access graph cannot walk to.
const DISCONNECTED_ENTITIES: u64 = 0;
/// `world-quality-gate.json.failures`: the six content exceptions plus the joint
/// housing minimum the recovery pads exist (and still fail) to close. Round 51's
/// eighth line, `has unverified building access routes`, is gone with the lot.
const GATE_FAILURES: usize = 7;
/// The cut ledger the bake verified against its own published raster: 27 pads and 24
/// graded roads, unchanged from before the refusal because the withdrawn lot carried
/// no excavation at all (it was fill-only).
const CUT_CELLS: u64 = 27_178;
const CUT_DEPTH_SUM_MM: u64 = 6_598_299;
const CUT_MAX_DEPTH_MM: u64 = 1_519;
/// The joint layout's own numbers after the refusal: 18 asked, 6 realized.
const JOINT_PLANNED: u64 = 6;
const JOINT_MISSING: u64 = 12;

/// The four declared keys, as `(JSON pointer, value)`.
fn switches() -> Vec<(&'static str, Value)> {
    vec![
        ("/style/frontageProgram/ruralConversion", json!("explicit")),
        ("/style/frontageProgram/terrainPolicy", json!("bounded-cut")),
        ("/style/frontageProgram/maxCutMm", json!(2000)),
        ("/style/landUseProfile/urbanGrowth", json!("axis-and-node")),
    ]
}

fn parsed(source: &str) -> Value {
    serde_json::from_str(source).expect("the committed example parses as JSON")
}

fn style(manifest: &Value) -> &Value {
    &manifest["style"]
}

/// Drop one declared key from the parsed manifest, so what is left can be
/// compared field for field with the base probe.
fn drop_switch(manifest: &mut Value, pointer: &str) {
    let (parent, key) = pointer
        .rsplit_once('/')
        .expect("a pointer has a parent and a key");
    let removed = manifest
        .pointer_mut(parent)
        .expect("the declared parent exists")
        .as_object_mut()
        .expect("the declared parent is an object")
        .remove(key);
    assert!(removed.is_some(), "{pointer} must be declared");
}

/// The base probe's own bytes with the four declaration lines inserted: the
/// opt-in file is a copy of the probe, so this must equal it character for
/// character. Line endings are normalised first, because a clone may check the
/// examples out with CRLF.
fn with_switches(base: &str) -> String {
    let axis = "      \"urbanCoreRatio\": 0.02\n";
    let program = "    \"frontageProgram\": {\n      \"version\": 1,\n";
    assert_eq!(
        base.matches(axis).count(),
        1,
        "the base probe still ends landUseProfile with urbanCoreRatio 0.02; \
         if that line moved, re-derive the opt-in example instead of editing it"
    );
    assert_eq!(
        base.matches(program).count(),
        1,
        "the base probe still opens frontageProgram with version 1; \
         if that block moved, re-derive the opt-in example instead of editing it"
    );
    base.replace(
        axis,
        "      \"urbanCoreRatio\": 0.02,\n      \"urbanGrowth\": \"axis-and-node\"\n",
    )
    .replace(
        program,
        "    \"frontageProgram\": {\n      \"version\": 1,\n      \"ruralConversion\": \"explicit\",\n      \"terrainPolicy\": \"bounded-cut\",\n      \"maxCutMm\": 2000,\n",
    )
}

fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The numbers one baked world publishes, read from its own bytes: the pad plan, the
/// two building kinds it resolved into, the pads whose entrance did not verify, the
/// access graph, the gate's failure list, the joint layout's audit, the plan's own
/// pad records and the cut ledger.
struct Published {
    pads: Value,
    homes: Value,
    shops: Value,
    unverified: Vec<String>,
    disconnected: Value,
    gate_failures: Vec<String>,
    joint: Value,
    development: Value,
    refusals: Vec<Value>,
    planned_sites: Vec<String>,
    entity_ids: Vec<String>,
    replacement_ids: Vec<String>,
    conversion_ids: Vec<String>,
    cut_ledger: Value,
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(path)
            .unwrap_or_else(|error| panic!("{} is not published: {error}", path.display())),
    )
    .unwrap()
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

impl Published {
    fn read(world: &Path) -> Published {
        let scene = world.join("scenes/scene-0");
        let quality = read_json(&world.join("quality-report.json"));
        let gate = read_json(&world.join("world-quality-gate.json"));
        let roads = read_json(&scene.join("road-network-plan.json"));
        let joint_audit = read_json(&scene.join("joint-layout-audit.json"));
        let development = read_json(&scene.join("development-audit.json"));
        let engineered = read_json(&scene.join("engineered-terrain.json"));
        // The published entity list, from the preview index rather than the 71 MB
        // scene payload: the ids and kinds are the same bytes the bake writes.
        let preview = read_json(&scene.join("preview-scene.json"));
        let report = &quality["scenes"][0];
        let failures = report["developmentAudit"]["failures"]
            .as_array()
            .expect("the development audit publishes its failures")
            .iter()
            .map(|failure| failure.as_str().expect("a failure is text").to_owned())
            .filter(|failure| failure.ends_with(UNVERIFIED_ACCESS))
            .collect();
        Published {
            pads: report["developmentAudit"]["plannedBuildings"].clone(),
            homes: report["entityKinds"]["residential_home"].clone(),
            shops: report["entityKinds"]["storefront"].clone(),
            unverified: failures,
            disconnected: report["accessGraph"]["disconnectedEntities"].clone(),
            gate_failures: strings(&gate, "failures"),
            joint: joint_audit,
            development,
            refusals: roads["roadsideSiteRejections"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            planned_sites: roads["roadsideSites"]
                .as_array()
                .expect("the plan publishes its pads")
                .iter()
                .map(|site| site["id"].as_str().unwrap_or_default().to_owned())
                .collect(),
            replacement_ids: strings(&roads["jointLayout"], "replacementResidentialIds"),
            conversion_ids: roads["padConversions"]
                .as_array()
                .map(|records| {
                    records
                        .iter()
                        .map(|record| record["parcelId"].as_str().unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            entity_ids: preview["entities"]
                .as_array()
                .map(|entities| {
                    entities
                        .iter()
                        .map(|entity| entity["entityId"].as_str().unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            cut_ledger: engineered["boundedCut"].clone(),
        }
    }

    fn refusal_of(&self, pad: &str) -> &Value {
        self.refusals
            .iter()
            .find(|refusal| refusal["edgeId"] == pad)
            .unwrap_or_else(|| panic!("{pad} is refused nowhere: {:#?}", self.refusals))
    }
}

// ---------------------------------------------------------------------------
// Layer 1 - default, no bake: the manifest really declares the switches.
// ---------------------------------------------------------------------------

/// Every switch exists, holds the exact value, AND the generator's own parser
/// reads it as ON. Text that looks right but is spelled `"bounded cut"` would
/// pass a string compare and still bake the historical world; these parsers are
/// the ones `bake_world` uses, so they cannot disagree.
#[test]
fn the_optin_manifest_declares_every_switch_the_probe_numbers_depend_on() {
    let opt_in = parsed(OPT_IN);
    for (pointer, expected) in switches() {
        assert_eq!(
            opt_in.pointer(pointer),
            Some(&expected),
            "{pointer} is what makes the quoted numbers reproducible"
        );
    }

    let opt_in_style = style(&opt_in);
    let (policy, refused) = bounded_cut::CutPolicy::from_style(opt_in_style);
    assert_eq!(
        policy,
        bounded_cut::CutPolicy::Bounded { max_cut_mm: 2000 },
        "the bounded-cut policy must parse as declared (refusal: {refused:?})"
    );
    assert!(policy.enabled());
    assert_eq!(policy.cap_mm(), 2000);
    assert!(
        urban_growth::enabled(opt_in_style),
        "the axis-and-node urban growth must be ON"
    );

    let program = frontage::from_style(opt_in_style)
        .expect("the frontage program parses")
        .expect("the probe declares a frontage program");
    assert_eq!(program.rural_conversion, Some(json!("explicit")));
    assert_eq!(program.terrain_policy, Some(json!("bounded-cut")));
    assert_eq!(program.max_cut_mm, Some(json!(2000)));
    assert!(
        program
            .requests
            .iter()
            .all(|r| r.rural_conversion.is_explicit()),
        "the program-level switch applies to every request"
    );
}

// ---------------------------------------------------------------------------
// Layer 2 - default, no bake: nothing but those switches moved.
// ---------------------------------------------------------------------------

/// The opt-in file is the probe plus four keys: the parsed values are identical
/// field for field once the keys are removed, and the bytes are identical once
/// the four declaration lines are inserted. A changed seed, origin, size,
/// zoning or demand therefore fails here rather than producing a world that is
/// merely "close" to the quoted one.
#[test]
fn the_optin_manifest_moves_nothing_but_the_declared_switches() {
    let base = parsed(BASE);
    let mut opt_in = parsed(OPT_IN);

    // The base probe cannot reproduce the numbers: it declares none of the
    // switches, and the generators read all four as OFF (the bounded-cut
    // `"unrecognised"` and the absent urban growth both mean the historical
    // fill-only, radial plan).
    for (pointer, _) in switches() {
        assert!(
            base.pointer(pointer).is_none(),
            "the closed probe must not declare {pointer}"
        );
    }
    let (policy, _) = bounded_cut::CutPolicy::from_style(style(&base));
    assert!(!policy.enabled(), "the closed probe is fill-only");
    assert!(
        !urban_growth::enabled(style(&base)),
        "the closed probe is radial"
    );
    assert!(
        frontage::from_style(style(&base))
            .expect("the closed probe parses")
            .expect("the closed probe declares a frontage program")
            .requests
            .iter()
            .all(|r| !r.rural_conversion.is_explicit()),
        "the closed probe cannot convert rural land"
    );

    for (pointer, _) in switches() {
        drop_switch(&mut opt_in, pointer);
    }
    assert_eq!(
        opt_in, base,
        "seed, origin, size, zoning and demand must be the probe's own values"
    );

    assert_eq!(
        lf(OPT_IN),
        with_switches(&lf(BASE)),
        "the opt-in example must be the probe's bytes plus the four declaration lines"
    );
}

// ---------------------------------------------------------------------------
// Layer 3 - opt-in, one 1 km bake: the numbers this file quotes.
// ---------------------------------------------------------------------------

struct TestDir(PathBuf);
impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("glyphweave-{label}-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn glyphweave() -> &'static str {
    env!("CARGO_BIN_EXE_glyphweave")
}

/// Bake the opt-in example through the shipped entry point, or audit the world
/// `GLYPHWEAVE_AUDIT_WORLD` already names.
#[test]
#[ignore = "1 km bake; set GLYPHWEAVE_AUDIT_WORLD to an already baked world to skip it"]
fn the_optin_manifest_replays_the_probes_published_numbers() {
    let named = std::env::var_os("GLYPHWEAVE_AUDIT_WORLD").map(PathBuf::from);
    let owned = if named.is_none() {
        Some(TestDir::new("b01-1km-probe-optin"))
    } else {
        None
    };
    let world = match (&named, &owned) {
        (Some(path), _) => path.clone(),
        (None, Some(dir)) => dir.0.clone(),
        (None, None) => unreachable!("one source of a world"),
    };
    if let Some(dir) = &owned {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../examples/b01-1km-probe-optin.manifest.json");
        let bake = Command::new(glyphweave())
            .arg("generate-world")
            .arg(&manifest)
            .arg(&dir.0)
            .output()
            .expect("generate-world runs");
        assert!(
            bake.status.success(),
            "the opt-in manifest must bake: {}",
            String::from_utf8_lossy(&bake.stderr)
        );
    }

    // The gate recomputes the development audit from the published scene and
    // writes the reports this test reads. It exits non-zero on an incomplete
    // world, which is what this probe still is (the housing minimum is unmet);
    // the failure LIST is the fact, and it must no longer name access at all.
    let gate = Command::new(glyphweave())
        .arg("world-quality-gate")
        .arg(&world)
        .output()
        .expect("world-quality-gate runs");
    assert!(
        !gate.status.success(),
        "this probe is still an incomplete world; a passing gate means the numbers \
         below were fixed and both this test and the ledger row must be re-measured"
    );
    let published = Published::read(&world);

    // --- 1. the pad plan, the two building kinds, the shop programme ----------
    assert_eq!(
        published.pads,
        json!(PADS),
        "planned pads on the 1 km probe"
    );
    assert_eq!(
        published.homes,
        json!(HOMES),
        "published residential_home entities"
    );
    assert_eq!(
        published.shops,
        json!(SHOPS),
        "published storefront entities"
    );
    assert_eq!(
        published.development["realizedBuildings"],
        json!(PADS),
        "every planned pad is realized exactly once"
    );
    assert_eq!(published.development["status"], json!("incomplete"));
    assert_eq!(
        published.development["failures"],
        json!([]),
        "no pad fails the development audit any more"
    );

    // --- 2. no unverified entrance, no disconnected entity --------------------
    assert_eq!(
        published.unverified.len(),
        UNVERIFIED_ENTRANCES,
        "pads without a verified entrance: {:?}",
        published.unverified
    );
    assert_eq!(
        published.disconnected,
        json!(DISCONNECTED_ENTITIES),
        "the access graph agrees with the development audit"
    );
    assert_eq!(
        published.joint["realizedResidentialWithAccess"],
        json!(JOINT_PLANNED),
        "every published home is realized with access to its assigned street"
    );
    assert_eq!(
        published.joint["failures"],
        json!([]),
        "the joint audit names no housing failure"
    );
    assert_eq!(published.joint["status"], json!("incomplete"));
    assert_eq!(published.joint["residentialMinimum"], json!(18));
    assert_eq!(published.joint["missingResidential"], json!(JOINT_MISSING));
    assert_eq!(published.joint["unfilledServices"], json!(0));

    // --- 3. the refused lot: named, measured, and gone from every record ------
    let refusal = published.refusal_of(REFUSED_PAD);
    assert_eq!(
        refusal["category"],
        json!(REFUSAL_CATEGORY),
        "the refusal is published under its own category"
    );
    let reason = refusal["reason"]
        .as_str()
        .expect("a refusal carries its reason");
    assert!(
        reason.contains(&format!(
            "pad {REFUSED_PAD} (24x24, declared top -1.000 m on road \
             generated.road-network.scene-0.125-126) is refused once the pad list is whole"
        )),
        "{reason}"
    );
    assert!(
        reason.contains(
            "no checked pedestrian route to its own street \
             (no_walkable_route_within_budget)"
        ),
        "{reason}"
    );
    assert!(reason.contains(REFUSAL_DETOUR), "{reason}");
    assert!(reason.contains("within the 120 m budget"), "{reason}");
    // The five records that used to carry it, all at once.
    assert!(
        !published.planned_sites.iter().any(|id| id == REFUSED_PAD),
        "the refused lot is not a planned pad"
    );
    assert!(
        !published
            .replacement_ids
            .iter()
            .any(|id| id == REFUSED_PAD),
        "the joint receipt keeps no slot for it"
    );
    assert!(
        !published.conversion_ids.iter().any(|id| id == REFUSED_PAD),
        "it converts no land"
    );
    assert!(
        !published.entity_ids.iter().any(|id| id == REFUSED_PAD),
        "no building entity carries it"
    );
    assert!(
        !published
            .entity_ids
            .iter()
            .any(|id| *id == format!("{REFUSED_PAD}.building")),
        "nor a parcel product"
    );
    assert_eq!(
        published.planned_sites.len() as u64,
        PADS,
        "the pad list is exactly the published pads"
    );

    // --- 4. the gate's failure list ------------------------------------------
    assert_eq!(
        published.gate_failures.len(),
        GATE_FAILURES,
        "gate failure lines: {:#?}",
        published.gate_failures
    );
    assert!(
        !published
            .gate_failures
            .iter()
            .any(|failure| failure.contains("unverified building access")),
        "the access line is gone: {:#?}",
        published.gate_failures
    );
    assert!(
        published
            .gate_failures
            .iter()
            .any(|failure| failure.contains("joint frontage housing minimum remains unmet")),
        "the housing shortfall is still reported, not hidden: {:#?}",
        published.gate_failures
    );

    // --- 5. the cut ledger equals the ground truth ---------------------------
    // The bake verifies its own ledger against the raster it publishes
    // (`bounded_cut::verify`) and refuses to publish a world that disagrees, so a
    // successful bake IS the equality. What is pinned here is that the ledger's
    // totals are the sums of its own entries - one arithmetic, no second table -
    // and that withdrawing a fill-only lot moved none of them.
    let ledger = &published.cut_ledger;
    assert_eq!(ledger["enabled"], json!(true));
    assert_eq!(ledger["maxCutMm"], json!(2000));
    let entries: Vec<&Value> = ledger["pads"]
        .as_array()
        .expect("the ledger reports its pads")
        .iter()
        .chain(
            ledger["roads"]
                .as_array()
                .expect("the ledger reports its roads")
                .iter(),
        )
        .collect();
    let cut_cells: u64 = entries
        .iter()
        .map(|entry| entry["cutCells"].as_u64().unwrap_or(0))
        .sum();
    let depth_sum: u64 = entries
        .iter()
        .map(|entry| entry["cutDepthSumMm"].as_u64().unwrap_or(0))
        .sum();
    let max_depth: u64 = entries
        .iter()
        .map(|entry| entry["maxCutDepthMm"].as_u64().unwrap_or(0))
        .max()
        .unwrap_or(0);
    assert_eq!(ledger["cutCells"], json!(cut_cells));
    assert_eq!(ledger["cutDepthSumMm"], json!(depth_sum));
    assert_eq!(ledger["maxCutDepthMm"], json!(max_depth));
    assert_eq!(ledger["cutCells"], json!(CUT_CELLS));
    assert_eq!(ledger["cutDepthSumMm"], json!(CUT_DEPTH_SUM_MM));
    assert_eq!(ledger["maxCutDepthMm"], json!(CUT_MAX_DEPTH_MM));
    assert!(
        !entries.iter().any(|entry| entry["entityId"] == REFUSED_PAD),
        "the refused lot carried no excavation, so the ledger never named it"
    );
}
