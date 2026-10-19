//! Round 51: the 1 km probe's opt-in state is a repository artefact, not a
//! commit body.
//!
//! The baseline ledger quotes this world as **44 pads / 16 homes / 24 shops /
//! 0 unverified entrances / 7 gate lines** (`f641015`, `764ebde`), but
//! `examples/b01-1km-probe.manifest.json` declares none of the switches those
//! numbers depend on - it is the closed state - so the numbers could only be
//! cited from a commit message. `examples/b01-1km-probe-optin.manifest.json` is
//! that input: the probe, byte for byte, plus exactly four declared keys.
//!
//! Two layers, because the two failure modes are different:
//!
//!  * the DEFAULT tests parse both manifests and pin (a) every switch with the
//!    value the generators themselves read - not just the text - and (b) the
//!    "nothing else moved" contract, so deleting a switch or editing a seed
//!    turns into a red test instead of a silently different world;
//!  * the `#[ignore]`d test bakes the opt-in manifest end to end (1 km; ~17 s
//!    with a release binary, a few minutes with this test's own debug binary)
//!    and pins the numbers the ledger quotes. Set `GLYPHWEAVE_AUDIT_WORLD` to an
//!    already baked output directory to audit that world instead of baking.
use glyphweave_core::worldgen::{bounded_cut, frontage, urban_growth};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

/// The closed probe. Compiled in, so the test cannot silently read another file.
const BASE: &str = include_str!("../../../../examples/b01-1km-probe.manifest.json");
/// The probe plus the four switches the quoted numbers were measured with.
const OPT_IN: &str = include_str!("../../../../examples/b01-1km-probe-optin.manifest.json");

/// The acceptance failure that names an entrance the bake cannot verify. The
/// development audit appends it to the pad's entity id.
const UNVERIFIED_ACCESS: &str = "no_verified_assigned_road_access";

/// The one pad that still fronts a street it cannot enter, measured at
/// `416db02` with the opt-in manifest. It is the joint-frontage recovery home
/// that `f641015` did **not** remove: the two roadside pads it refused are gone,
/// this one was planned and published, and only the post-bake access audit
/// disagrees with the plan-time gate.
const UNVERIFIED_PAD: &str =
    "generated.residential-recovery.scene-0.3.building: no_verified_assigned_road_access";

// ---------------------------------------------------------------------------
// What the world publishes. MEASURED at `416db02` from
// `examples/b01-1km-probe-optin.manifest.json`; the ledger's historical
// "0 unverified / 7 gate lines" is the two numbers this file corrects, and the
// reason is stated in `UNVERIFIED_PAD`'s comment and in the ledger itself.
// ---------------------------------------------------------------------------

/// `developmentAudit.plannedBuildings`: 16 homes + 24 shops + 4 services.
const PADS: u64 = 44;
/// `entityKinds.residential_home`: 9 frontage + 3 roadside + 4 recovery.
const HOMES: u64 = 16;
/// `entityKinds.storefront`: the shop request is fully met.
const SHOPS: u64 = 24;
/// Pads whose entrance the post-bake audit could not verify.
const UNVERIFIED_ENTRANCES: usize = 1;
/// `world-quality-gate.json.failures`: the six content exceptions, plus the
/// unverified access line, plus the joint housing minimum the recovery pads
/// exist to close.
const GATE_FAILURES: usize = 8;

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

/// The five numbers the ledger quotes, read from one baked world: the pad plan,
/// the two building kinds it resolved into, the pads whose entrance did not
/// verify, and the gate's own failure list.
struct Published {
    pads: Value,
    homes: Value,
    shops: Value,
    unverified: Vec<String>,
    disconnected: Value,
    gate_failures: Vec<String>,
}

impl Published {
    fn from_reports(quality: &Value, gate: &Value) -> Published {
        let scene = &quality["scenes"][0];
        let failures = scene["developmentAudit"]["failures"]
            .as_array()
            .expect("the development audit publishes its failures")
            .iter()
            .map(|failure| failure.as_str().expect("a failure is text").to_owned())
            .filter(|failure| failure.ends_with(UNVERIFIED_ACCESS))
            .collect();
        Published {
            pads: scene["developmentAudit"]["plannedBuildings"].clone(),
            homes: scene["entityKinds"]["residential_home"].clone(),
            shops: scene["entityKinds"]["storefront"].clone(),
            unverified: failures,
            disconnected: scene["accessGraph"]["disconnectedEntities"].clone(),
            gate_failures: gate["failures"]
                .as_array()
                .expect("the gate publishes its failures")
                .iter()
                .map(|failure| failure.as_str().expect("a gate failure is text").to_owned())
                .collect(),
        }
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
// Layer 3 - opt-in, one 1 km bake: the numbers the ledger quotes.
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

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(
        &std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} is not published: {error}", path.display())),
    )
    .unwrap()
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
    // writes both reports this test reads. It exits non-zero on a failing world,
    // which is the state this probe is in; the failure LIST below is the fact.
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
    let published = Published::from_reports(
        &read_json(world.join("quality-report.json")),
        &read_json(world.join("world-quality-gate.json")),
    );

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
        published.unverified.len(),
        UNVERIFIED_ENTRANCES,
        "pads without a verified entrance: {:?}",
        published.unverified
    );
    assert_eq!(
        published.unverified,
        vec![UNVERIFIED_PAD.to_owned()],
        "the one pad the plan accepts and the access audit refuses"
    );
    assert_eq!(
        published.disconnected,
        json!(1),
        "the access graph agrees with the development audit"
    );
    assert_eq!(
        published.gate_failures.len(),
        GATE_FAILURES,
        "gate failure lines: {:#?}",
        published.gate_failures
    );
}
