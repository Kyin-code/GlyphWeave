//! End-to-end contract: actual bake files must audit identically via the CLI,
//! and corrupted height/entity state must fail without being auto-repaired.
use glyphweave_core::worldgen::{WorldManifest, bake_world};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

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
fn run_audit(dir: &TestDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .args(["rules", "audit"])
        .arg(&dir.0)
        .arg("--rules")
        .arg(rules())
        .arg("--report")
        .arg(dir.0.join("cli-audit.json"))
        .output()
        .unwrap()
}
fn read(dir: &TestDir, name: &str) -> Value {
    serde_json::from_slice(&fs::read(dir.0.join(name)).unwrap()).unwrap()
}
fn manifest(seed: u64) -> WorldManifest {
    let mut m = WorldManifest::default_demo();
    m.world.seed = seed;
    m.scenes[0].width_m = 512;
    m.scenes[0].depth_m = 512;
    m.landmarks.clear();
    m.style["rulesDir"] = serde_json::json!(rules());
    m.style["rulesMode"] = serde_json::json!("rules");
    m.style["generationProfile"] = serde_json::json!("strict");
    m
}
#[test]
fn fresh_bakes_match_cli_audit_across_seeds_and_detect_corruption() {
    let mut pavement_fixture_count = 0;
    for seed in [7, 42, 20260905] {
        let dir = TestDir::new("audit-parity");
        let m = manifest(seed);
        bake_world(&m, &dir.0).unwrap();
        let expected = read(&dir, "rules-audit.json");
        let output = run_audit(&dir);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(expected, read(&dir, "cli-audit.json"));
        let scene_path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
        let scene = read(&dir, &scene_path);
        assert!(
            !scene["entities"].as_array().unwrap().is_empty(),
            "empty output cannot prove parity"
        );
        let surfaces = scene["accessSurfaces"].as_array().unwrap();
        if !surfaces.is_empty() {
            pavement_fixture_count += 1;
            assert!(
                surfaces
                    .iter()
                    .all(|s| !s["triangles"].as_array().unwrap().is_empty())
            );
            let mut bad_surface = scene.clone();
            let y = bad_surface["accessSurfaces"][0]["vertices"][0][1]
                .as_f64()
                .unwrap();
            bad_surface["accessSurfaces"][0]["vertices"][0][1] = serde_json::json!(y + 0.25);
            fs::write(
                dir.0.join(&scene_path),
                serde_json::to_vec(&bad_surface).unwrap(),
            )
            .unwrap();
            let bad = run_audit(&dir);
            assert!(!bad.status.success(), "edited pavement must fail CLI audit");
            assert!(String::from_utf8_lossy(&bad.stderr).contains("persisted access surfaces"));
            bad_surface["accessSurfaces"] = serde_json::json!([]);
            fs::write(
                dir.0.join(&scene_path),
                serde_json::to_vec(&bad_surface).unwrap(),
            )
            .unwrap();
            assert!(
                !run_audit(&dir).status.success(),
                "missing pavement must fail CLI audit"
            );
            fs::write(dir.0.join(&scene_path), serde_json::to_vec(&scene).unwrap()).unwrap();
        }
        let chunk = scene["chunks"][0]["heightFile"].as_str().unwrap();
        let height_path = dir.0.join("scenes").join(&m.scenes[0].scene_id).join(chunk);
        let bytes = fs::read(&height_path).unwrap();
        fs::write(&height_path, &bytes[..bytes.len() - 1]).unwrap();
        let output = run_audit(&dir);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("payload mismatch"));
        fs::write(&height_path, &bytes).unwrap();
        let mut duplicate = scene.clone();
        let chunk = duplicate["chunks"][0].clone();
        duplicate["chunks"].as_array_mut().unwrap().push(chunk);
        fs::write(
            dir.0.join(&scene_path),
            serde_json::to_vec(&duplicate).unwrap(),
        )
        .unwrap();
        assert!(!run_audit(&dir).status.success());
        let mut missing = scene.clone();
        missing["chunks"] = serde_json::json!([]);
        fs::write(
            dir.0.join(&scene_path),
            serde_json::to_vec(&missing).unwrap(),
        )
        .unwrap();
        assert!(!run_audit(&dir).status.success());
        let mut mismatch = scene.clone();
        mismatch["widthM"] = serde_json::json!(513);
        fs::write(
            dir.0.join(&scene_path),
            serde_json::to_vec(&mismatch).unwrap(),
        )
        .unwrap();
        assert!(!run_audit(&dir).status.success());
        let mut duplicate = scene.clone();
        let entity = duplicate["entities"][0].clone();
        duplicate["entities"].as_array_mut().unwrap().push(entity);
        fs::write(
            dir.0.join(&scene_path),
            serde_json::to_vec(&duplicate).unwrap(),
        )
        .unwrap();
        assert!(!run_audit(&dir).status.success());
        let mut sunken = scene.clone();
        // An explicitly persisted building must be audited, not silently lifted.
        let entity = &mut sunken["entities"][0];
        entity["kind"] = serde_json::json!("building");
        entity["widthM"] = serde_json::json!(10.0);
        entity["depthM"] = serde_json::json!(10.0);
        entity["worldX"] = serde_json::json!(256);
        entity["worldZ"] = serde_json::json!(256);
        entity["worldY"] = serde_json::json!(-10000);
        fs::write(
            dir.0.join(&scene_path),
            serde_json::to_vec(&sunken).unwrap(),
        )
        .unwrap();
        let sunken_output = run_audit(&dir);
        assert!(
            !sunken_output.status.success(),
            "audit must not rebase a corrupted persisted entity"
        );
        // Changing a road/building can invalidate pavement before the rules
        // stage. Rebuild ONLY derived pavement to isolate grounding; never
        // repair the persisted entity or terrain, or read a stale report.
        let mut typed: glyphweave_core::worldgen::SceneIndex =
            serde_json::from_value(sunken).unwrap();
        let mut raster = glyphweave_core::baked_terrain::BakedTerrain::new(&m.scenes[0]).unwrap();
        let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
        for chunk in &typed.chunks {
            raster
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &fs::read(folder.join(&chunk.height_file)).unwrap(),
                )
                .unwrap();
        }
        let water = glyphweave_core::worldgen::water_geometry(
            glyphweave_core::worldgen::water_kind(&m.style),
            &m.landmarks,
            &m.scenes[0],
            &m.style,
        );
        let access = glyphweave_core::worldgen::build_access_graph(&typed.entities, &raster, water);
        typed.access_surfaces =
            glyphweave_core::worldgen::bake_access_surfaces(&access, &raster).unwrap();
        fs::write(dir.0.join(&scene_path), serde_json::to_vec(&typed).unwrap()).unwrap();
        fs::remove_file(dir.0.join("cli-audit.json")).unwrap();
        let output = run_audit(&dir);
        assert!(!output.status.success());
        assert!(
            dir.0.join("cli-audit.json").exists(),
            "rules stage did not run: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            read(&dir, "cli-audit.json")["floating_items"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    assert!(
        pavement_fixture_count > 0,
        "seed set must include real baked pavement geometry"
    );
}

#[test]
fn quality_report_rebuilds_access_instead_of_trusting_serialized_success() {
    let dir = TestDir::new("access-tampering");
    let m = manifest(7);
    bake_world(&m, &dir.0).unwrap();
    let scene_path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let mut scene = read(&dir, &scene_path);
    scene["entities"] = serde_json::json!([{
        "entityId":"landmark.required-house","kind":"building","assetId":"prop.building",
        "worldX":256,"worldZ":256,"worldY":0,"widthM":10,"depthM":10,"heightM":8,"scale":1,
        "anchors":[{"id":"door","worldX":256,"worldZ":250,"direction":"north","target":"road"}]
    }]);

    // Keep unrelated planning facts consistent with the deliberately changed
    // entities; this test isolates forged access rather than stale parcels.
    let mut typed: glyphweave_core::worldgen::SceneIndex =
        serde_json::from_value(scene.clone()).unwrap();
    if let Some(mut plan) = typed.parcel_plan.take() {
        glyphweave_core::settlement::realize_parcel_plan(&mut plan, &typed.entities);
        glyphweave_core::settlement::realize_infrastructure_plan(
            &mut plan.infrastructure,
            &typed.entities,
        );
        let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
        fs::write(
            folder.join("parcel-plan.json"),
            serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        fs::write(
            folder.join("infrastructure-plan.json"),
            serde_json::to_vec(&plan.infrastructure).unwrap(),
        )
        .unwrap();
        typed.parcel_plan = Some(plan);
    }
    if let (Some(development), Some(plan)) = (&typed.development_plan, &typed.landcover_plan) {
        let spec = &m.scenes[0];
        let mut terrain = glyphweave_core::baked_terrain::BakedTerrain::new(spec).unwrap();
        let folder = dir.0.join("scenes").join(&spec.scene_id);
        for c in &typed.chunks {
            terrain
                .insert_chunk(
                    c.world_x,
                    c.world_z,
                    c.valid_width_m,
                    c.valid_depth_m,
                    &fs::read(folder.join(&c.height_file)).unwrap(),
                )
                .unwrap();
        }
        let water = glyphweave_core::worldgen::water_geometry(
            glyphweave_core::worldgen::water_kind(&m.style),
            &m.landmarks,
            spec,
            &m.style,
        );
        let surfaces = glyphweave_core::settlement::landcover::realize(
            spec,
            development,
            plan,
            &terrain,
            water,
            &typed.entities,
        )
        .unwrap();
        fs::write(
            folder.join("landcover-surfaces.json"),
            serde_json::to_vec(&surfaces).unwrap(),
        )
        .unwrap();
        typed.landcover_surfaces = Some(surfaces);
    }
    typed.land_use_budget = Some(glyphweave_core::worldgen::plan_landuse_budget(
        &typed,
        &glyphweave_core::worldgen::LandUseProfile::from_style(&m.style).unwrap(),
    ));
    if let Some(plan) = &typed.development_plan {
        let graph =
            glyphweave_core::settlement::development::settlement_graph(plan, &typed.entities);
        fs::write(
            dir.0
                .join("scenes")
                .join(&m.scenes[0].scene_id)
                .join("settlement-graph.json"),
            serde_json::to_vec(&graph).unwrap(),
        )
        .unwrap();
        typed.settlement_graph = Some(graph);
    }
    scene = serde_json::to_value(typed).unwrap();
    scene["accessGraph"] = serde_json::json!({"version":2,"links":[],"connectedEntities":999,"disconnectedEntities":0});
    fs::write(dir.0.join(&scene_path), serde_json::to_vec(&scene).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read(&dir, "quality-report.json");
    assert_eq!(
        report["scenes"][0]["accessGraph"]["disconnectedEntities"],
        1
    );
    assert_eq!(report["scenes"][0]["accessGraph"]["connectedEntities"], 0);
    assert_eq!(
        report["scenes"][0]["accessGraph"]["links"][0]["required"],
        true
    );
}

#[test]
fn natural_audit_rebuilds_from_full_raster_and_rejects_forged_success() {
    let dir = TestDir::new("natural-provenance");
    let mut m = manifest(42);
    // This fixture exercises raster provenance, not city placement. No rules
    // threshold is changed; the production city fixtures remain above.
    m.style["naturalOnly"] = serde_json::json!(true);
    m.style["generationProfile"] = serde_json::json!("prototype");
    m.style["rulesMode"] = serde_json::json!("legacy");
    bake_world(&m, &dir.0).unwrap();
    let scene_path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let original_scene = read(&dir, &scene_path);
    let scene_dir = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    // A previous pass/fail report is not evidence. Recomputing a valid source
    // succeeds even when the historical report has been replaced with garbage.
    fs::write(
        scene_dir.join("natural-terrain-audit.json"),
        b"not an audit",
    )
    .unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        read(&dir, "quality-report.json")["scenes"][0]["naturalTerrainAudit"]["status"],
        "pass"
    );

    // Change a non-sampled pixel without truncating the payload. The full
    // raster hash must catch this even when the derived graph is identical.
    let natural_path = scene_dir.join(
        original_scene["chunks"][0]["naturalHeightFile"]
            .as_str()
            .unwrap(),
    );
    let bytes = fs::read(&natural_path).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    fs::write(&natural_path, &corrupt).unwrap();
    fs::write(
        scene_dir.join("natural-terrain-audit.json"),
        b"{\"status\":\"pass\"}",
    )
    .unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("hash mismatch"));
    fs::write(&natural_path, &bytes).unwrap();

    // Forge the graph while leaving both copied snapshots consistent.
    let mut scene = original_scene.clone();
    scene["naturalTerrain"]["drainage"]["nodes"][0]["flowAccumulation"] = serde_json::json!(999);
    fs::write(dir.0.join(&scene_path), serde_json::to_vec(&scene).unwrap()).unwrap();
    fs::write(
        scene_dir.join("natural-terrain.json"),
        serde_json::to_vec(&scene["naturalTerrain"]).unwrap(),
    )
    .unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("differs from raster"));
    fs::write(
        dir.0.join(&scene_path),
        serde_json::to_vec(&original_scene).unwrap(),
    )
    .unwrap();
    fs::write(
        scene_dir.join("natural-terrain.json"),
        serde_json::to_vec(&original_scene["naturalTerrain"]).unwrap(),
    )
    .unwrap();

    fs::write(&natural_path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(!run().status.success());
    fs::write(&natural_path, &bytes).unwrap();
    fs::remove_file(&natural_path).unwrap();
    assert!(!run().status.success());
}

#[test]
fn natural_raster_is_independent_of_generated_human_entities() {
    let natural_dir = TestDir::new("natural-before-humans");
    let developed_dir = TestDir::new("natural-with-humans");
    let mut m = manifest(42);
    m.style["generationProfile"] = serde_json::json!("prototype");
    m.style["rulesMode"] = serde_json::json!("legacy");
    m.style["naturalOnly"] = serde_json::json!(true);
    bake_world(&m, &natural_dir.0).unwrap();
    m.style["naturalOnly"] = serde_json::json!(false);
    bake_world(&m, &developed_dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let natural = read(&natural_dir, &path);
    let developed = read(&developed_dir, &path);
    assert_ne!(natural["entities"], developed["entities"]);
    assert_eq!(
        natural["naturalTerrain"]["heightfieldHash"],
        developed["naturalTerrain"]["heightfieldHash"]
    );
    assert_eq!(
        natural["naturalTerrain"]["drainage"],
        developed["naturalTerrain"]["drainage"]
    );
    let prefix = format!("scenes/{}/", m.scenes[0].scene_id);
    for chunk in natural["chunks"].as_array().unwrap() {
        let file = chunk["naturalHeightFile"].as_str().unwrap();
        assert_eq!(
            fs::read(natural_dir.0.join(format!("{prefix}{file}"))).unwrap(),
            fs::read(developed_dir.0.join(format!("{prefix}{file}"))).unwrap()
        );
    }
}

#[test]
fn quality_report_rebuilds_development_and_rejects_forged_ownership() {
    let dir = TestDir::new("development-provenance");
    let mut m = manifest(20260906);
    m.style["landUseProfile"]["theme"] = serde_json::json!("temperate-plain");
    m.style["landUseProfile"]["urbanCoreRatio"] = serde_json::json!(0.12);
    m.style["landUseProfile"]["suburbanRatio"] = serde_json::json!(0.144);
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let scene = read(&dir, &path);
    assert!(
        scene["parcelPlan"].is_null(),
        "two conflicting authorities are forbidden"
    );
    let plan = &scene["developmentPlan"];
    assert!(!plan["buildings"].as_array().unwrap().is_empty());
    let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    // A persisted success report is never evidence of land realization.
    fs::write(
        folder.join("development-audit.json"),
        b"{\"status\":\"pass\"}",
    )
    .unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let audit = read(&dir, "quality-report.json")["scenes"][0]["developmentAudit"].clone();
    assert_eq!(audit["status"], "incomplete");
    assert_eq!(audit["partitionAreaM2"], 512 * 512);
    assert!(audit["unplannedM2"].as_u64().unwrap() > 0);
    assert_eq!(audit["realizedBuildings"], audit["plannedBuildings"]);
    assert_eq!(
        scene["landUseBudget"]["realizationAccounting"],
        "disjoint-owned-surfaces-v3"
    );
    let realized_yard =
        read(&dir, "quality-report.json")["scenes"][0]["landcoverAudit"]["realizedAreaM2"]
            .as_f64()
            .unwrap();
    assert!(
        realized_yard > 0. && realized_yard <= audit["reservedYardM2"].as_f64().unwrap() + 1e-7
    );
    let expected_area = (audit["roadUnionM2"].as_u64().unwrap()
        + audit["plannedBuildingFootprintM2"].as_u64().unwrap()) as f64
        + realized_yard;
    assert!((scene["landUseBudget"]["assignedM2"].as_f64().unwrap() - expected_area).abs() < 1e-7);
    for b in plan["buildings"].as_array().unwrap() {
        assert!(
            scene["settlementGraph"]["buildingRelations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["entityId"] == b["entity"]["entityId"]
                    && r["parcelId"] == b["parcelId"]
                    && r["nearestRoadId"] == b["roadId"])
        );
    }
    let gate = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("world-quality-gate")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(!gate.status.success());
    assert!(
        read(&dir, "world-quality-gate.json")["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().contains("road-first land-use"))
    );
    // Saved landcover reports are never acceptance evidence; rebuild them.
    fs::write(folder.join("landcover-audit.json"), b"not a report").unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        read(&dir, "quality-report.json")["scenes"][0]["landcoverAudit"]["realizedAreaM2"]
            .as_f64()
            .unwrap(),
        realized_yard
    );
    for mutation in 0..3 {
        let mut forged = scene.clone();
        match mutation {
            0 => {
                forged["landcoverPlan"]["cells"][0]["realizationStatus"] =
                    serde_json::json!("realized")
            }
            1 => forged["landcoverPlan"]["cells"][0]["role"] = serde_json::json!("unmanaged"),
            _ => {
                forged["landcoverPlan"]["cells"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
        }
        fs::write(dir.0.join(&path), serde_json::to_vec(&forged).unwrap()).unwrap();
        fs::write(
            folder.join("landcover-plan.json"),
            serde_json::to_vec(&forged["landcoverPlan"]).unwrap(),
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        // The independent audit re-sources the residual policy from the frozen
        // manifest (R1 source binding), so a forged plan is rejected by the
        // authoritative-remainder message rather than the older wording.
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("landcover plan differs from authoritative remainder/policy")
        );
    }
    fs::write(dir.0.join(&path), serde_json::to_vec(&scene).unwrap()).unwrap();
    fs::write(
        folder.join("landcover-plan.json"),
        serde_json::to_vec(&scene["landcoverPlan"]).unwrap(),
    )
    .unwrap();
    // Forge BOTH copies. Independent reconstruction must catch wrong parentage,
    // erased remainder, inflated yard, and fabricated ownership alike.
    for mutation in 0..4 {
        let mut forged = scene.clone();
        match mutation {
            0 => {
                forged["developmentPlan"]["buildings"][0]["parcelId"] =
                    serde_json::json!("fake-parent")
            }
            1 => {
                forged["developmentPlan"]["cells"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c["role"] != "unplanned");
            }
            2 => forged["developmentPlan"]["parcels"][0]["yardM2"] = serde_json::json!(999999),
            _ => forged["developmentPlan"]["cells"][0]["ownerId"] = serde_json::json!("fake-owner"),
        }
        fs::write(dir.0.join(&path), serde_json::to_vec(&forged).unwrap()).unwrap();
        fs::write(
            folder.join("development-plan.json"),
            serde_json::to_vec(&forged["developmentPlan"]).unwrap(),
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("independently rebuilt ownership")
        );
    }
    fs::write(dir.0.join(&path), serde_json::to_vec(&scene).unwrap()).unwrap();
    fs::write(folder.join("development-plan.json"), b"{}").unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("development-plan.json differs"));
    let mut missing = scene.clone();
    missing["developmentPlan"] = serde_json::Value::Null;
    fs::write(dir.0.join(&path), serde_json::to_vec(&missing).unwrap()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing required road-first development")
    );
}

#[test]
fn lowland_road_network_is_real_connected_and_independently_audited() {
    let dir = TestDir::new("road-network-acceptance");
    let mut m = manifest(20260906);
    m.style["landUseProfile"]["theme"] = serde_json::json!("temperate-plain");
    m.style["landUseProfile"]["urbanCoreRatio"] = serde_json::json!(0.12);
    m.style["landUseProfile"]["suburbanRatio"] = serde_json::json!(0.144);
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let scene = read(&dir, &path);
    let roads = scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "road")
        .count();
    assert!(roads >= 4, "must not pass with a token road or empty graph");
    assert_eq!(
        roads,
        scene["roadNetworkPlan"]["edges"].as_array().unwrap().len()
    );
    assert!(
        scene["accessGraph"]["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["required"] == true && l["connected"] == true)
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read(&dir, "quality-report.json");
    assert_eq!(report["scenes"][0]["roadNetworkAudit"]["status"], "pass");
    let road_audit = &report["scenes"][0]["roadNetworkAudit"];
    assert!(road_audit["plannedSites"].as_u64().unwrap() > 0);
    assert_eq!(road_audit["realizedSites"], road_audit["plannedSites"]);
    assert_eq!(road_audit["maxSiteDesignHeightErrorM"], 0.0);
    assert_eq!(
        report["scenes"][0]["roadNetworkAudit"]["maxDesignHeightErrorM"],
        0.0
    );
    let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    // A forged saved audit cannot excuse missing final road geometry.
    let mut forged = scene.clone();
    forged["entities"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["kind"] != "road");
    fs::write(dir.0.join(&path), serde_json::to_vec(&forged).unwrap()).unwrap();
    fs::write(
        folder.join("road-network-audit.json"),
        b"{\"status\":\"pass\"}",
    )
    .unwrap();
    assert!(
        !run().status.success(),
        "budget/plan revalidation must reject removed roads"
    );
    let mut forged = scene.clone();
    forged["roadNetworkPlan"] = serde_json::Value::Null;
    fs::write(dir.0.join(&path), serde_json::to_vec(&forged).unwrap()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing required natural road"));
}

#[test]
fn legacy_non_road_first_parcel_provenance_remains_independently_checked() {
    let dir = TestDir::new("legacy-parcel-contract");
    let mut m = manifest(42);
    m.style["generationProfile"] = serde_json::json!("prototype");
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let mut scene = read(&dir, &path);
    assert!(scene["developmentPlan"].is_null());
    assert!(
        !scene["parcelPlan"]["parcels"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    scene["parcelPlan"]["parcels"][0]["realizationStatus"] = serde_json::json!("accepted");
    scene["parcelPlan"]["parcels"][0]["acceptedEntityIds"] = serde_json::json!(["fake"]);
    fs::write(dir.0.join(&path), serde_json::to_vec(&scene).unwrap()).unwrap();
    fs::write(
        dir.0
            .join("scenes")
            .join(&m.scenes[0].scene_id)
            .join("parcel-plan.json"),
        serde_json::to_vec(&scene["parcelPlan"]).unwrap(),
    )
    .unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("independently rebuilt"));
}

#[test]
fn kilometer_development_preserves_cross_zone_parcels_and_graph_roundtrip() {
    let dir = TestDir::new("kilometer-ownership");
    let mut m = manifest(20260906);
    m.scenes[0].width_m = 1000;
    m.scenes[0].depth_m = 1000;
    m.style["landUseProfile"]["theme"] = serde_json::json!("temperate-plain");
    m.style["landUseProfile"]["urbanCoreRatio"] = serde_json::json!(0.12);
    m.style["landUseProfile"]["suburbanRatio"] = serde_json::json!(0.144);
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let scene = read(&dir, &path);
    assert!(
        scene["developmentPlan"]["parcels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["sourceZones"].as_object().unwrap().len() > 1)
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let quality = read(&dir, "quality-report.json");
    let audit = &quality["scenes"][0]["developmentAudit"];
    assert_eq!(audit["partitionAreaM2"], 1_000_000);
    assert_eq!(audit["realizedBuildings"], 9);
    assert_eq!(audit["status"], "incomplete");
    let budget = &quality["scenes"][0]["landUseBudget"];
    assert_eq!(
        budget["actualTransportM2"].as_f64().unwrap(),
        audit["roadUnionM2"].as_f64().unwrap()
    );
    assert_eq!(
        budget["assignedM2"].as_f64().unwrap(),
        budget["actualTransportM2"].as_f64().unwrap()
            + budget["actualUrbanM2"].as_f64().unwrap()
            + budget["actualRuralM2"].as_f64().unwrap()
    );
    let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    let mut forged = scene.clone();
    forged["settlementGraph"]["buildingRelations"][0]["parcelId"] =
        serde_json::json!("nearby-but-not-assigned");
    fs::write(dir.0.join(&path), serde_json::to_vec(&forged).unwrap()).unwrap();
    fs::write(
        folder.join("settlement-graph.json"),
        serde_json::to_vec(&forged["settlementGraph"]).unwrap(),
    )
    .unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("settlement graph differs from road-first parentage")
    );
}

#[test]
fn yard_surfaces_are_real_geometry_and_both_copies_are_independently_audited() {
    use glyphweave_core::worldgen::{LandUseProfile, SceneIndex, plan_landuse_budget};
    let dir = TestDir::new("yard-surfaces");
    let mut m = manifest(20260906);
    m.style["landUseProfile"]["theme"] = serde_json::json!("temperate-plain");
    m.style["landUseProfile"]["urbanCoreRatio"] = serde_json::json!(0.12);
    m.style["landUseProfile"]["suburbanRatio"] = serde_json::json!(0.144);
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    let scene = read(&dir, &path);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read(&dir, "quality-report.json");
    let audit = &report["scenes"][0]["landcoverAudit"];
    assert!((audit["realizedAreaM2"].as_f64().unwrap() - 208.).abs() < 1e-7);
    assert!(audit["pavementAreaM2"].as_f64().unwrap() > 0.);
    assert_eq!(audit["unresolvedYardM2"], 0.0);
    assert_eq!(audit["status"], "incomplete");
    assert!(
        !report["scenes"][0]["developmentAudit"]["incompleteReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "yard_reserved_but_landcover_not_realized")
    );
    let surfaces = &scene["landcoverSurfaces"];
    assert!(
        surfaces["patches"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| !p["triangles"].as_array().unwrap().is_empty())
    );
    // Keep unrelated budget facts consistent, so these tests reach surface
    // reconstruction rather than merely fail an earlier stale-area check.
    for mutation in 0..6 {
        let mut bad = scene.clone();
        let patches = &mut bad["landcoverSurfaces"]["patches"];
        match mutation {
            0 => {
                let y = patches[0]["vertices"][0][1].as_f64().unwrap();
                patches[0]["vertices"][0][1] = serde_json::json!(y + 0.25);
            }
            1 => {
                let x = patches[0]["vertices"][0][0].as_f64().unwrap();
                patches[0]["vertices"][0][0] = serde_json::json!(x + 0.25);
            }
            2 => patches[0]["sourceCellId"] = serde_json::json!("unrelated-yard"),
            3 => {
                patches.as_array_mut().unwrap().pop();
            }
            4 => patches[0]["areaM2"] = serde_json::json!(9999.),
            _ => patches[0]["triangles"][0][0] = serde_json::json!(u32::MAX),
        }
        let mut typed: SceneIndex = serde_json::from_value(bad).unwrap();
        typed.land_use_budget = Some(plan_landuse_budget(
            &typed,
            &LandUseProfile::from_style(&m.style).unwrap(),
        ));
        let bad = serde_json::to_value(typed).unwrap();
        fs::write(dir.0.join(&path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            folder.join("landcover-surfaces.json"),
            serde_json::to_vec(&bad["landcoverSurfaces"]).unwrap(),
        )
        .unwrap();
        fs::write(
            folder.join("landcover-audit.json"),
            b"{\"status\":\"pass\"}",
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("independent realization"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(dir.0.join(&path), serde_json::to_vec(&scene).unwrap()).unwrap();
    let mut missing = surfaces.clone();
    missing["patches"] = serde_json::json!([]);
    fs::write(
        folder.join("landcover-surfaces.json"),
        serde_json::to_vec(&missing).unwrap(),
    )
    .unwrap();
    assert!(!run().status.success());
    fs::write(
        folder.join("landcover-surfaces.json"),
        serde_json::to_vec(surfaces).unwrap(),
    )
    .unwrap();
    let mut missing = scene.clone();
    missing["landcoverSurfaces"] = serde_json::Value::Null;
    let mut typed: SceneIndex = serde_json::from_value(missing).unwrap();
    typed.land_use_budget = Some(plan_landuse_budget(
        &typed,
        &LandUseProfile::from_style(&m.style).unwrap(),
    ));
    fs::write(dir.0.join(&path), serde_json::to_vec(&typed).unwrap()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing required landcover surfaces")
    );
}

#[test]
fn plains_agriculture_has_real_field_access_surfaces_and_resists_forgery() {
    use glyphweave_core::worldgen::{LandUseProfile, SceneIndex, plan_landuse_budget};
    let dir = TestDir::new("agriculture-surfaces");
    let mut m = manifest(7);
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["landUseProfile"] = serde_json::json!({"theme":"temperate-plain","urbanCoreRatio":0.02,"suburbanRatio":0.02,"farmRatio":0.7,"pastureRatio":0.1,"forestRatio":0.05,"reserveRatio":0.05,"greenRatio":0.05});
    bake_world(&m, &dir.0).unwrap();
    let path = format!("scenes/{}/scene.json", m.scenes[0].scene_id);
    let scene = read(&dir, &path);
    let folder = dir.0.join("scenes").join(&m.scenes[0].scene_id);
    assert!(
        scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["kind"] != "farmland" && e["kind"] != "pasture")
    );
    let fields = &scene["landcoverSurfaces"]["agriculture"]["fields"];
    assert!(
        fields.as_array().unwrap().len() >= 2,
        "single token field cannot validate agricultural generation"
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let q = read(&dir, "quality-report.json");
    let audit = &q["scenes"][0]["landcoverAudit"];
    assert_eq!(
        audit["agricultureFields"].as_u64().unwrap(),
        fields.as_array().unwrap().len() as u64
    );
    assert_eq!(
        audit["cultivatedAreaM2"].as_f64().unwrap(),
        fields.as_array().unwrap().len() as f64 * 256.
    );
    assert!(audit["farmTrackAreaM2"].as_f64().unwrap() > 0.);
    assert_eq!(audit["status"], "incomplete");
    for f in fields.as_array().unwrap() {
        assert!(
            scene["entities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["entityId"] == f["roadId"] && e["kind"] == "road")
        );
        assert!(
            scene["landcoverPlan"]["cells"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == f["sourceCellId"] && c["role"] == "agriculture_candidate")
        );
    }
    let road_area = q["scenes"][0]["developmentAudit"]["roadUnionM2"]
        .as_f64()
        .unwrap();
    assert_eq!(
        scene["landUseBudget"]["actualTransportM2"]
            .as_f64()
            .unwrap(),
        road_area + audit["farmTrackAreaM2"].as_f64().unwrap()
    );
    // Source IDs, actual route, metadata and actual vertices are all required.
    for mutation in 0..6 {
        let mut bad = scene.clone();
        let ag = &mut bad["landcoverSurfaces"]["agriculture"];
        match mutation {
            0 => ag["fields"][0]["roadId"] = serde_json::json!("not-a-road"),
            1 => ag["fields"][0]["accessPath"][0][0] = serde_json::json!(-100),
            2 => ag["fields"][0]["sourceCellId"] = serde_json::json!("nature"),
            3 => {
                ag["fields"].as_array_mut().unwrap().pop();
            }
            4 => {
                let patch = bad["landcoverSurfaces"]["patches"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|p| p["material"] == "cultivated_soil")
                    .unwrap();
                let y = patch["vertices"][0][1].as_f64().unwrap();
                patch["vertices"][0][1] = serde_json::json!(y + 0.25);
            }
            _ => bad["landcoverSurfaces"]["patches"]
                .as_array_mut()
                .unwrap()
                .retain(|p| p["material"] != "farm_track"),
        }
        let mut typed: SceneIndex = serde_json::from_value(bad).unwrap();
        typed.land_use_budget = Some(plan_landuse_budget(
            &typed,
            &LandUseProfile::from_style(&m.style).unwrap(),
        ));
        let bad = serde_json::to_value(typed).unwrap();
        fs::write(dir.0.join(&path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            folder.join("landcover-surfaces.json"),
            serde_json::to_vec(&bad["landcoverSurfaces"]).unwrap(),
        )
        .unwrap();
        fs::write(
            folder.join("landcover-audit.json"),
            b"{\"status\":\"pass\"}",
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("independent realization"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn millimetre_bakes_use_one_explicit_contract_for_core_cli_and_agriculture() {
    use glyphweave_core::height_payload::{HeightEncoding, HeightPayload};
    let dir = TestDir::new("precise-plains");
    let mut m = manifest(7);
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["landUseProfile"] = serde_json::json!({"theme":"temperate-plain","urbanCoreRatio":0.02,"suburbanRatio":0.02,"farmRatio":0.7,"pastureRatio":0.1,"forestRatio":0.05,"reserveRatio":0.05,"greenRatio":0.05});
    bake_world(&m, &dir.0).unwrap();
    let scene = read(&dir, "scenes/scene-0/scene.json");
    let folder = dir.0.join("scenes/scene-0");
    let final_path = folder.join(scene["chunks"][0]["heightFile"].as_str().unwrap());
    let natural_path = folder.join(scene["chunks"][0]["naturalHeightFile"].as_str().unwrap());
    let natural = fs::read(&natural_path).unwrap();
    let final_bytes = fs::read(&final_path).unwrap();
    let payload = HeightPayload::parse(&natural, 512, 512).unwrap();
    assert_eq!(payload.encoding, HeightEncoding::Millimetre);
    assert!(
        (0..payload.count).any(|i| payload.raw(i).rem_euclid(250) != 0),
        "not upscaled legacy steps"
    );
    let sidecar = read(&dir, "sidecar.json");
    assert_eq!(
        sidecar["contract"]["heightEncoding"],
        "gwhm-v1-i32le-millimeter"
    );
    assert!((sidecar["contract"]["heightPrecisionM"].as_f64().unwrap() - 0.001).abs() < 1e-9);
    let run = |command: &str| {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg(command)
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    for cmd in ["validate-world", "scale-audit", "quality-report"] {
        let output = run(cmd);
        assert!(
            output.status.success(),
            "{cmd}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let q = read(&dir, "quality-report.json");
    assert_eq!(q["scenes"][0]["naturalTerrainAudit"]["status"], "pass");
    assert_eq!(q["scenes"][0]["roadNetworkAudit"]["status"], "pass");
    assert!(
        q["scenes"][0]["landcoverAudit"]["agricultureFields"]
            .as_u64()
            .unwrap()
            >= 8
    );
    assert!(run_audit(&dir).status.success());
    assert_eq!(read(&dir, "cli-audit.json"), read(&dir, "rules-audit.json"));
    let gate = run("world-quality-gate");
    assert!(!gate.status.success());
    assert_eq!(read(&dir, "world-quality-gate.json")["status"], "fail");
    // Header damage and sample tampering cannot be covered by saved pass reports.
    let mut bad = natural.clone();
    bad[4] = 2;
    fs::write(&natural_path, &bad).unwrap();
    assert!(!run("quality-report").status.success());
    let mut bad = natural.clone();
    bad[16] ^= 1;
    fs::write(&natural_path, &bad).unwrap();
    let output = run("quality-report");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("natural heightfield hash mismatch"));
    fs::write(&natural_path, &natural).unwrap();
    fs::write(&final_path, &final_bytes[..final_bytes.len() - 1]).unwrap();
    assert!(!run("scale-audit").status.success());
    fs::write(&final_path, &final_bytes).unwrap();
    // A valid legacy payload is still the WRONG format for this manifest.
    let parsed = HeightPayload::parse(&final_bytes, 512, 512).unwrap();
    let legacy: Vec<_> = (0..parsed.count)
        .flat_map(|i| ((parsed.metres(i) * 4.) as i16).to_le_bytes())
        .collect();
    fs::write(&final_path, legacy).unwrap();
    let output = run("quality-report");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("height encoding differs from manifest")
    );
    fs::write(&final_path, &final_bytes).unwrap();
    let mut bad = sidecar.clone();
    bad["contract"]["heightPrecisionM"] = serde_json::json!(0.25);
    fs::write(
        dir.0.join("sidecar.json"),
        serde_json::to_vec(&bad).unwrap(),
    )
    .unwrap();
    let output = run("validate-world");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("sidecar height contract differs"));
}

#[test]
fn precise_natural_height_is_identical_across_chunk_and_scene_boundaries() {
    use glyphweave_core::baked_terrain::BakedTerrain;
    use glyphweave_core::worldgen::SceneIndex;
    let mut m = manifest(42);
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    m.style["generationProfile"] = serde_json::json!("prototype");
    m.style["rulesMode"] = serde_json::json!("legacy");
    m.style["naturalOnly"] = serde_json::json!(true);
    m.scenes[0].origin_x = -400;
    m.scenes[0].origin_z = -90;
    m.scenes[0].width_m = 513;
    m.scenes[0].depth_m = 512;
    let mut second = m.scenes[0].clone();
    second.scene_id = "overlap".into();
    second.origin_x = 100;
    second.width_m = 512;
    m.scenes.push(second);
    m.style["terrainDomain"] =
        serde_json::json!({"originX":-400,"originZ":-90,"widthM":1012,"depthM":512});
    let dir = TestDir::new("precise-seams");
    bake_world(&m, &dir.0).unwrap();
    let mut rasters = vec![];
    for spec in &m.scenes {
        let scene: SceneIndex =
            serde_json::from_value(read(&dir, &format!("scenes/{}/scene.json", spec.scene_id)))
                .unwrap();
        let mut raster = BakedTerrain::new(spec).unwrap();
        for c in scene.chunks {
            raster
                .insert_chunk(
                    c.world_x,
                    c.world_z,
                    c.valid_width_m,
                    c.valid_depth_m,
                    &fs::read(
                        dir.0
                            .join("scenes")
                            .join(&spec.scene_id)
                            .join(c.natural_height_file.unwrap()),
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        raster.require_complete().unwrap();
        rasters.push(raster);
    }
    for z in -90..-26 {
        for x in 100..113 {
            assert_eq!(
                rasters[0].height(x, z),
                rasters[1].height(x, z),
                "seam {x},{z}"
            );
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("quality-report")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn zoned_bake_preserves_reserved_samples_and_rejects_forged_evidence() {
    use glyphweave_core::height_payload::HeightPayload;
    let dir = TestDir::new("zoning-protection");
    let mut m = manifest(7);
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["landUseProfile"] = serde_json::json!({"theme":"temperate-plain","urbanCoreRatio":0.02,"suburbanRatio":0.02,"farmRatio":0.7,"pastureRatio":0.1,"forestRatio":0.05,"reserveRatio":0.05,"greenRatio":0.05});
    bake_world(&m, &dir.0).unwrap();
    let scene_path = "scenes/scene-0/scene.json";
    let scene = read(&dir, scene_path);
    let folder = dir.0.join("scenes/scene-0");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    let report = read(&dir, "quality-report.json");
    assert_eq!(report["version"], 2);
    let q = &report["scenes"][0];
    let ledger = &q["landUseLedger"];
    assert_eq!(ledger, &read(&dir, "scenes/scene-0/land-use-ledger.json"));
    assert_eq!(ledger["status"], "incomplete");
    assert!(ledger["byPurpose"]["rural"]["realizedM2"].as_f64().unwrap() > 0.);
    assert_eq!(
        q["landUseArea"]["ruralM2"],
        ledger["byPurpose"]["rural"]["realizedM2"]
    );
    assert_eq!(
        q["landUseArea"]["entityFootprintDiagnostic"]["ruralM2"],
        0.0
    );
    let realized = ledger["realizedM2"].as_f64().unwrap();
    let unresolved = ledger["unresolvedM2"].as_f64().unwrap();
    assert!((realized + unresolved - 512. * 512.).abs() < 1e-6);
    let developed = ledger["byPurpose"]["urban"]["realizedM2"].as_f64().unwrap()
        + ledger["byPurpose"]["rural"]["realizedM2"].as_f64().unwrap();
    assert!((developed - q["landUseBudget"]["assignedM2"].as_f64().unwrap()).abs() < 1e-6);
    assert!(
        ledger["byPurpose"]["nature"]["realizedM2"]
            .as_f64()
            .unwrap()
            > 0.
    );

    // A published success/area claim is never read as acceptance authority.
    fs::write(
        folder.join("land-use-ledger.json"),
        b"{\"status\":\"pass\",\"realizedM2\":262144}",
    )
    .unwrap();
    assert!(run().status.success());
    assert_eq!(
        &read(&dir, "quality-report.json")["scenes"][0]["landUseLedger"],
        ledger
    );
    fs::write(
        folder.join("land-use-ledger.json"),
        serde_json::to_vec(ledger).unwrap(),
    )
    .unwrap();
    let gate = Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg("world-quality-gate")
        .arg(&dir.0)
        .output()
        .unwrap();
    assert!(!gate.status.success());
    assert!(
        read(&dir, "world-quality-gate.json")["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().is_some_and(
                |s| s.contains("purpose-area allocation or realization remains incomplete")
            ))
    );
    assert_eq!(q["landProtectionAudit"]["status"], "pass");
    assert_eq!(q["developmentAudit"]["zoneConflictM2"], 0.0);
    assert_eq!(
        q["developmentAudit"]["unplannedBuildingIds"],
        serde_json::json!([])
    );
    assert!(q["roadNetworkAudit"]["plannedEdges"].as_u64().unwrap() > 0);
    // Round 45: this world's graded streets were cut below the network level the
    // roadside pads declare, so every centre placement is a lot standing a step
    // above its own street. The walkable-entrance gate refuses those at
    // acceptance (the same rule R13 already applies to a demanded settlement), so
    // this scene publishes no roadside lot at all. The audit must still hold its
    // own arithmetic on an empty set.
    assert_eq!(q["roadNetworkAudit"]["plannedSites"].as_u64().unwrap(), 0);
    assert_eq!(
        q["roadNetworkAudit"]["plannedSites"],
        q["roadNetworkAudit"]["realizedSites"]
    );
    assert!(scene["developmentProposals"].as_array().unwrap().len() > 0);
    assert!(
        scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| !matches!(
                e["kind"].as_str(),
                Some("canal" | "railway" | "parking_lot")
            ))
    );
    // Compare every reserved sample from the actual payloads, not audit flags.
    let c = &scene["chunks"][0];
    assert_eq!(scene["chunks"].as_array().unwrap().len(), 1);
    let final_path = folder.join(c["heightFile"].as_str().unwrap());
    let final_bytes = fs::read(&final_path).unwrap();
    let natural_bytes = fs::read(folder.join(c["naturalHeightFile"].as_str().unwrap())).unwrap();
    let natural = HeightPayload::parse(&natural_bytes, 512, 512).unwrap();
    let terrain = HeightPayload::parse(&final_bytes, 512, 512).unwrap();
    let mut checked = 0_u64;
    let mut tamper_index = None;
    for zone in scene["landUseZones"].as_array().unwrap() {
        if matches!(zone["kind"].as_str(), Some("urban" | "rural")) {
            continue;
        }
        let x = zone["worldX"].as_i64().unwrap() as usize;
        let z = zone["worldZ"].as_i64().unwrap() as usize;
        let w = zone["widthM"].as_u64().unwrap() as usize;
        let d = zone["depthM"].as_u64().unwrap() as usize;
        for zz in z..z + d {
            for xx in x..x + w {
                let i = zz * 512 + xx;
                assert_eq!(natural.raw(i), terrain.raw(i), "protected sample {xx},{zz}");
                checked += 1;
            }
        }
        tamper_index = Some((z + d / 2) * 512 + x + w / 2);
    }
    assert!(checked > 0);
    assert_eq!(q["landProtectionAudit"]["protectedAreaM2"], checked);
    // A stored failure cannot override good data, and a stored pass cannot
    // hide even a one-millimetre change in a protected pixel.
    fs::write(
        folder.join("land-protection-audit.json"),
        b"{\"status\":\"fail\"}",
    )
    .unwrap();
    assert!(run().status.success());
    assert_eq!(
        read(&dir, "quality-report.json")["scenes"][0]["landProtectionAudit"],
        q["landProtectionAudit"]
    );
    fs::write(
        folder.join("land-protection-audit.json"),
        b"{\"status\":\"pass\"}",
    )
    .unwrap();
    let mut bad = final_bytes.clone();
    let offset = 16 + tamper_index.unwrap() * 4;
    let raw = i32::from_le_bytes(bad[offset..offset + 4].try_into().unwrap());
    bad[offset..offset + 4].copy_from_slice(&(raw + 1).to_le_bytes());
    fs::write(&final_path, &bad).unwrap();
    // Surface provenance also binds the complete final raster. CLI rejects
    // that mismatch before writing a report; isolate the protection result
    // using the public audit on the same actual, mutated payloads.
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("independent realization"));
    let mut natural_raster =
        glyphweave_core::baked_terrain::BakedTerrain::new(&m.scenes[0]).unwrap();
    natural_raster
        .insert_chunk(0, 0, 512, 512, &natural_bytes)
        .unwrap();
    let mut final_raster = glyphweave_core::baked_terrain::BakedTerrain::new(&m.scenes[0]).unwrap();
    final_raster.insert_chunk(0, 0, 512, 512, &bad).unwrap();
    let typed: glyphweave_core::worldgen::SceneIndex =
        serde_json::from_value(scene.clone()).unwrap();
    let protection = glyphweave_core::worldgen::zoning::audit_protection(
        &m.scenes[0],
        &typed.land_use_zones,
        &natural_raster,
        &final_raster,
        &typed.entities,
    )
    .unwrap();
    assert_eq!(protection.status, "fail");
    assert_eq!(protection.changed_height_samples, 1);
    fs::write(&final_path, final_bytes).unwrap();
    // Change both published copies: byte equality alone must not be authority.
    for field in ["zoningHash", "zoningRejections"] {
        let mut forged = scene.clone();
        forged["roadNetworkPlan"][field] = if field == "zoningHash" {
            serde_json::json!("forged")
        } else {
            serde_json::json!([])
        };
        assert_ne!(forged["roadNetworkPlan"], scene["roadNetworkPlan"]);
        fs::write(dir.0.join(scene_path), serde_json::to_vec(&forged).unwrap()).unwrap();
        fs::write(
            folder.join("road-network-plan.json"),
            serde_json::to_vec(&forged["roadNetworkPlan"]).unwrap(),
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("road plan differs from natural raster reconstruction"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn explicit_services_are_real_exact_buildings_and_unfilled_demand_cannot_be_forged() {
    let dir = TestDir::new("service-program");
    let mut m = manifest(7);
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["landUseProfile"] = serde_json::json!({"theme":"temperate-plain","urbanCoreRatio":0.02,"suburbanRatio":0.02,"farmRatio":0.7,"pastureRatio":0.1,"forestRatio":0.05,"reserveRatio":0.05,"greenRatio":0.05});
    m.style["settlementProgram"] = serde_json::json!({"version":1,"requests":[
        {"id":"village-shop","sceneId":"scene-0","kind":"convenience_store","count":2,"widthM":12,"depthM":10,"heightM":4},
        {"id":"community-church","sceneId":"scene-0","kind":"church","count":1,"widthM":20,"depthM":28,"heightM":10},
        {"id":"primary-school","sceneId":"scene-0","kind":"school","count":1,"widthM":40,"depthM":30,"heightM":10}
    ]});
    bake_world(&m, &dir.0).unwrap();
    let path = "scenes/scene-0/scene.json";
    let scene = read(&dir, path);
    let folder = dir.0.join("scenes/scene-0");
    let run = |command: &str| {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg(command)
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    assert!(run("quality-report").status.success());
    assert!(run_audit(&dir).status.success());
    let quality = read(&dir, "quality-report.json");
    let audit = &quality["scenes"][0]["serviceProgramAudit"];
    assert_eq!(audit["requested"], 4);
    assert!(audit["realizedWithAccess"].as_u64().unwrap() > 0);
    assert_eq!(audit["planned"], audit["realizedWithAccess"]);
    assert_eq!(audit["status"], "incomplete");
    assert_eq!(quality["scenes"][0]["roadNetworkAudit"]["status"], "pass");
    assert_eq!(
        quality["scenes"][0]["landProtectionAudit"]["status"],
        "pass"
    );
    assert_eq!(
        quality["scenes"][0]["developmentAudit"]["unplannedBuildingIds"],
        serde_json::json!([])
    );
    let entities = scene["entities"].as_array().unwrap();
    assert_eq!(
        entities
            .iter()
            .filter(|e| e["kind"] == "residential_home")
            .count(),
        18,
        "services must not displace baseline homes"
    );
    for site in scene["roadNetworkPlan"]["roadsideSites"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["program"].is_object())
    {
        let entity = entities
            .iter()
            .find(|e| e["entityId"] == site["id"])
            .unwrap();
        assert_eq!(entity["kind"], site["program"]["kind"]);
        assert_eq!(
            entity["widthM"].as_f64().unwrap(),
            site["widthM"].as_f64().unwrap()
        );
        assert_eq!(
            entity["depthM"].as_f64().unwrap(),
            site["depthM"].as_f64().unwrap()
        );
        assert_eq!(
            entity["heightM"].as_f64().unwrap(),
            site["program"]["heightM"].as_f64().unwrap()
        );
    }
    fs::write(
        folder.join("service-program-audit.json"),
        b"{\"status\":\"pass\",\"unfilled\":0}",
    )
    .unwrap();
    fs::write(folder.join("service-program-outcomes.json"), b"[]").unwrap();
    assert!(run("quality-report").status.success());
    assert_eq!(
        &read(&dir, "quality-report.json")["scenes"][0]["serviceProgramAudit"],
        audit
    );
    assert!(!run("world-quality-gate").status.success());
    assert!(
        read(&dir, "world-quality-gate.json")["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v
                .as_str()
                .is_some_and(|s| s.contains("explicit service requests remain unrealized")))
    );
    // Alter BOTH source copies: independent manifest/natural reconstruction
    // must still reject a shrink or a forged zero shortfall.
    for mutation in 0..2 {
        let mut bad = scene.clone();
        if mutation == 0 {
            let site = bad["roadNetworkPlan"]["roadsideSites"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|s| s["program"].is_object())
                .unwrap();
            site["widthM"] = serde_json::json!(6);
        } else {
            bad["roadNetworkPlan"]["serviceOutcomes"][2]["unfilledCount"] = serde_json::json!(0);
        }
        fs::write(dir.0.join(path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            folder.join("road-network-plan.json"),
            serde_json::to_vec(&bad["roadNetworkPlan"]).unwrap(),
        )
        .unwrap();
        let output = run("quality-report");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("road plan differs from natural raster reconstruction"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn preview_survives_client_disconnect_and_invalid_feedback() {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let dir = TestDir::new("preview-resilience");
    fs::create_dir(dir.0.join("preview")).unwrap();
    fs::write(
        dir.0.join("preview/index.html"),
        "verified preview still alive",
    )
    .unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("preview")
            .arg(&dir.0)
            .arg(port.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            drop(stream);
            break;
        }
        assert!(server.0.try_wait().unwrap().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for request in [
        "GET /preview/index.html",
        "POST /api/feedback HTTP/1.1\r\nContent-Length: 3\r\n\r\n???",
        "POST /api/feedback HTTP/1.1\r\nContent-Length: 4194305\r\n\r\n",
    ] {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "one malformed client killed preview"
        );
        let mut good = TcpStream::connect(("127.0.0.1", port)).unwrap();
        good.set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        good.write_all(b"GET /preview/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        good.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("verified preview still alive"));
    }
}
#[test]
fn joint_program_recovers_housing_minimum_and_preserves_source_obligations() {
    let dir = TestDir::new("joint-frontage");
    let mut m = manifest(7);
    let fixture: Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../examples/service-program-joint-plains.manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for field in [
        "heightPrecision",
        "terrainProfile",
        "landUseProfile",
        "settlementProgram",
    ] {
        m.style[field] = fixture["style"][field].clone();
    }
    bake_world(&m, &dir.0).unwrap();
    let path = "scenes/scene-0/scene.json";
    let scene = read(&dir, path);
    let folder = dir.0.join("scenes/scene-0");
    let run = |command: &str| {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg(command)
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    assert!(run("quality-report").status.success());
    assert!(run_audit(&dir).status.success());
    let quality = read(&dir, "quality-report.json");
    let q = &quality["scenes"][0];
    assert_eq!(q["serviceProgramAudit"]["realizedWithAccess"], 4);
    assert_eq!(q["serviceProgramAudit"]["status"], "pass");
    assert_eq!(q["jointLayoutAudit"]["residentialMinimum"], 18);
    assert_eq!(q["jointLayoutAudit"]["realizedResidentialWithAccess"], 18);
    assert_eq!(q["jointLayoutAudit"]["status"], "pass");
    assert_eq!(q["roadNetworkAudit"]["realizedEdges"], 29);
    assert_eq!(q["landProtectionAudit"]["changedHeightSamples"], 0);
    assert_eq!(q["developmentAudit"]["zoneConflictM2"], 0.0);
    let joint = &scene["roadNetworkPlan"]["jointLayout"];
    assert_eq!(joint["baselineResidentialCount"], 18);
    assert_eq!(
        joint["displacedResidentialIds"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        joint["retainedResidentialIds"].as_array().unwrap().len()
            + joint["replacementResidentialIds"].as_array().unwrap().len(),
        18
    );
    let school = scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "school")
        .unwrap();
    assert_eq!(school["widthM"], 40.0);
    assert_eq!(school["depthM"], 30.0);
    fs::write(
        folder.join("joint-layout-audit.json"),
        b"{\"status\":\"fail\"}",
    )
    .unwrap();
    assert!(run("quality-report").status.success());
    assert_eq!(
        read(&dir, "quality-report.json")["scenes"][0]["jointLayoutAudit"],
        q["jointLayoutAudit"]
    );
    assert!(
        !run("world-quality-gate").status.success(),
        "full ecology/landcover remains incomplete"
    );
    for mode in 0..3 {
        let mut bad = scene.clone();
        match mode {
            0 => {
                bad["roadNetworkPlan"]["jointLayout"]["residentialMinimum"] = serde_json::json!(16)
            }
            1 => {
                bad["roadNetworkPlan"]["jointLayout"]["replacementResidentialIds"] =
                    serde_json::json!([])
            }
            _ => {
                let slot = bad["roadNetworkPlan"]["roadsideSites"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|s| s["id"].as_str().unwrap().contains("residential-recovery"))
                    .unwrap();
                slot["worldX"] = serde_json::json!(slot["worldX"].as_i64().unwrap() + 1);
            }
        }
        fs::write(dir.0.join(path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            folder.join("road-network-plan.json"),
            serde_json::to_vec(&bad["roadNetworkPlan"]).unwrap(),
        )
        .unwrap();
        let out = run("quality-report");
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .contains("road plan differs from natural raster reconstruction"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn unavailable_joint_housing_fails_gate_without_lowering_the_request() {
    let dir = TestDir::new("joint-shortfall");
    let mut m = manifest(7);
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["landUseProfile"] = serde_json::json!({"urbanCoreRatio":0.,"suburbanRatio":0.,"farmRatio":0.,"pastureRatio":0.,"forestRatio":1.,"reserveRatio":0.,"greenRatio":0.});
    m.style["settlementProgram"] =
        serde_json::json!({"version":2,"housingMinimum":{"scene-0":1},"requests":[]});
    bake_world(&m, &dir.0).unwrap();
    let run = |cmd: &str| {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg(cmd)
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    fs::write(
        dir.0.join("scenes/scene-0/joint-layout-audit.json"),
        b"{\"status\":\"pass\",\"missingResidential\":0}",
    )
    .unwrap();
    assert!(run("quality-report").status.success());
    let q = read(&dir, "quality-report.json");
    assert_eq!(q["scenes"][0]["jointLayoutAudit"]["residentialMinimum"], 1);
    assert_eq!(q["scenes"][0]["jointLayoutAudit"]["missingResidential"], 1);
    assert_eq!(q["scenes"][0]["jointLayoutAudit"]["status"], "incomplete");
    assert!(!run("world-quality-gate").status.success());
    assert!(
        read(&dir, "world-quality-gate.json")["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v
                .as_str()
                .is_some_and(|s| s.contains("joint frontage housing minimum remains unmet")))
    );
}

#[test]
fn ecology_audit_accepts_valid_layers_and_rejects_both_copies_forged() {
    let dir = TestDir::new("ecology-independent");
    let mut m: WorldManifest = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../examples/service-program-joint-plains.manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    m.style["rulesDir"] = serde_json::json!(rules());
    bake_world(&m, &dir.0).unwrap();
    let path = "scenes/scene-0/scene.json";
    let scene = read(&dir, path);
    let published_path = dir.0.join("scenes/scene-0/landcover-surfaces.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let good = run();
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    let plants = scene["landcoverSurfaces"]["ecology"]["plants"]
        .as_array()
        .unwrap();
    let communities = scene["landcoverSurfaces"]["ecology"]["communities"]
        .as_array()
        .unwrap();
    assert!(plants.len() > communities.len() && !communities.is_empty());
    for layer in ["canopy", "understory", "ground", "emergent", "shrub"] {
        assert!(
            plants.iter().any(|p| p["layer"] == layer),
            "missing {layer}"
        );
    }
    let target: u64 = communities
        .iter()
        .map(|c| c["plannedPlantCount"].as_u64().unwrap())
        .sum();
    assert_eq!(
        target, 1915,
        "round(40960*.042)+round(4352*.028)+round(3840*.019)"
    );
    let report = read(&dir, "quality-report.json");
    assert_eq!(
        report["scenes"][0]["landcoverAudit"]["ecologyPlannedPlants"],
        target
    );
    assert_eq!(
        report["scenes"][0]["landcoverAudit"]["ecologyRealizedPlants"],
        plants.len()
    );
    assert_eq!(
        plants.len()
            + scene["landcoverSurfaces"]["ecology"]["rejections"]
                .as_array()
                .unwrap()
                .len(),
        target as usize
    );
    for mode in 0..14 {
        let mut bad = scene.clone();
        let ecology = &mut bad["landcoverSurfaces"]["ecology"];
        match mode {
            0 => {
                ecology["plants"].as_array_mut().unwrap().remove(0);
            }
            1 => {
                let plant = ecology["plants"][0].clone();
                ecology["plants"].as_array_mut().unwrap().push(plant);
            }
            2 => ecology["plants"][0]["sourcePatchId"] = serde_json::json!("absent-patch"),
            3 => ecology["plants"][0]["layer"] = serde_json::json!("forged-layer"),
            4 => {
                let y = ecology["plants"][0]["worldY"].as_f64().unwrap();
                ecology["plants"][0]["worldY"] = serde_json::json!(y + 1.);
            }
            5 => ecology["communities"][0]["plannedPlantCount"] = serde_json::json!(999),
            6 => ecology["communities"][0]["densityPerHa"] = serde_json::json!(999.),
            7 => ecology["totalAreaM2"] = serde_json::json!(999.),
            8 => {
                ecology["communities"].as_array_mut().unwrap().remove(0);
            }
            9 => {
                let item = ecology["communities"][0].clone();
                ecology["communities"].as_array_mut().unwrap().push(item);
            }
            10 => ecology["plants"][0]["widthM"] = serde_json::json!(1000.),
            11 => {
                let p = ecology["plants"][0].clone();
                ecology["rejections"].as_array_mut().unwrap().push(serde_json::json!({"id":p["id"],"sourcePatchId":p["sourcePatchId"],"layer":p["layer"],"reason":"no clear full-size ecology anchor in source patch"}));
            }
            12 => {
                ecology["communities"][0]["layerBudgets"][0]["plannedCount"] =
                    serde_json::json!(999)
            }
            _ => {
                ecology["plants"][1]["worldX"] = ecology["plants"][0]["worldX"].clone();
                ecology["plants"][1]["worldZ"] = ecology["plants"][0]["worldZ"].clone();
            }
        }
        // Both serialized copies agree, and a forged success summary is present.
        // Only independent reconstruction can reject this consistently.
        fs::write(dir.0.join(path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            &published_path,
            serde_json::to_vec(&bad["landcoverSurfaces"]).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.0.join("scenes/scene-0/landcover-audit.json"),
            b"{\"status\":\"pass\"}",
        )
        .unwrap();
        let result = run();
        assert!(!result.status.success(), "forgery mode {mode} passed");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("independent realization"),
            "mode {mode}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    // Also ensure the independently published copy is not ignored.
    fs::write(dir.0.join(path), serde_json::to_vec(&scene).unwrap()).unwrap();
    let result = run();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("published landcover independent realization")
    );
    fs::write(
        published_path,
        serde_json::to_vec(&scene["landcoverSurfaces"]).unwrap(),
    )
    .unwrap();
    let restored = run();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
}

#[test]
fn detour_fields_have_real_routes_and_reject_forged_route_evidence() {
    let dir = TestDir::new("farm-detours");
    let mut m: WorldManifest = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../examples/service-program-joint-plains.manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    m.world.seed = 42;
    m.style["rulesDir"] = serde_json::json!(rules());
    bake_world(&m, &dir.0).unwrap();
    let scene = read(&dir, "scenes/scene-0/scene.json");
    let fields = scene["landcoverSurfaces"]["agriculture"]["fields"]
        .as_array()
        .unwrap();
    let detours: Vec<_> = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f["accessMethod"] == "bounded_grid_detour")
        .collect();
    assert!(
        !detours.is_empty(),
        "must exercise actual generated detours"
    );
    for (_, f) in &detours {
        assert!(f["accessSearchExpanded"].as_u64().unwrap() > 0);
        assert!(f["accessSearchExpanded"].as_u64().unwrap() <= 4096);
        assert!(f["accessLengthM"].as_u64().unwrap() <= 160);
        let path = f["accessPath"].as_array().unwrap();
        let length: u64 = path
            .windows(2)
            .map(|p| {
                let a = [p[0][0].as_i64().unwrap(), p[0][1].as_i64().unwrap()];
                let b = [p[1][0].as_i64().unwrap(), p[1][1].as_i64().unwrap()];
                assert!(a[0] == b[0] || a[1] == b[1]);
                a[0].abs_diff(b[0]) + a[1].abs_diff(b[1])
            })
            .sum();
        assert_eq!(length, f["accessLengthM"].as_u64().unwrap());
        assert!(
            scene["entities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "road" && e["entityId"] == f["roadId"])
        );
    }
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let original = run();
    assert!(
        original.status.success(),
        "{}",
        String::from_utf8_lossy(&original.stderr)
    );
    let index = detours[0].0;
    for mode in 0..4 {
        let mut bad = scene.clone();
        let field = &mut bad["landcoverSurfaces"]["agriculture"]["fields"][index];
        match mode {
            0 => field["accessMethod"] = serde_json::json!("direct_axis_route"),
            1 => field["accessLengthM"] = serde_json::json!(1),
            2 => field["accessSearchExpanded"] = serde_json::json!(0),
            _ => field["accessPath"][1][0] = serde_json::json!(-999),
        }
        fs::write(
            dir.0.join("scenes/scene-0/scene.json"),
            serde_json::to_vec(&bad).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.0.join("scenes/scene-0/landcover-surfaces.json"),
            serde_json::to_vec(&bad["landcoverSurfaces"]).unwrap(),
        )
        .unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("independent realization"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(
        dir.0.join("scenes/scene-0/scene.json"),
        serde_json::to_vec(&scene).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.0.join("scenes/scene-0/landcover-surfaces.json"),
        serde_json::to_vec(&scene["landcoverSurfaces"]).unwrap(),
    )
    .unwrap();
    assert!(run().status.success());
}

#[test]
fn explicit_farm_dimensions_are_manifest_bound_and_mesh_area_is_not_fixed() {
    let dir = TestDir::new("farm-dimensions");
    let mut manifest: WorldManifest = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../examples/multiscale-farms.manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    bake_world(&manifest, &dir.0).unwrap();
    let scene_path = "scenes/scene-0/scene.json";
    let original = read(&dir, scene_path);
    let fields = original["landcoverSurfaces"]["agriculture"]["fields"]
        .as_array()
        .unwrap();
    assert!(
        fields
            .iter()
            .any(|f| f["boundary"]["widthM"] == 48 && f["boundary"]["depthM"] == 32)
    );
    assert!(
        fields
            .iter()
            .any(|f| f["boundary"]["widthM"] == 32 && f["boundary"]["depthM"] == 24)
    );
    let pattern = &original["landcoverPlan"]["farmPattern"];
    let mut soil = 0u64;
    let mut headland = 0u64;
    for f in fields {
        let source = original["landcoverPlan"]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == f["sourceCellId"])
            .unwrap();
        let zone = source["zoneId"].as_str().unwrap();
        let expected = pattern["zoneFields"]
            .get(zone)
            .unwrap_or(&pattern["defaultField"]);
        let w = f["boundary"]["widthM"].as_u64().unwrap();
        let d = f["boundary"]["depthM"].as_u64().unwrap();
        assert_eq!(f["boundary"]["widthM"], expected["widthM"]);
        assert_eq!(f["boundary"]["depthM"], expected["depthM"]);
        assert_eq!(f["cultivated"]["widthM"].as_u64().unwrap(), w - 8);
        assert_eq!(f["cultivated"]["depthM"].as_u64().unwrap(), d - 8);
        soil += (w - 8) * (d - 8);
        headland += w * d - (w - 8) * (d - 8);
    }
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_glyphweave"))
            .arg("quality-report")
            .arg(&dir.0)
            .output()
            .unwrap()
    };
    let good = run();
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    let q = read(&dir, "quality-report.json");
    assert_eq!(
        q["scenes"][0]["landcoverAudit"]["cultivatedAreaM2"]
            .as_f64()
            .unwrap(),
        soil as f64
    );
    assert_eq!(
        q["scenes"][0]["landcoverAudit"]["headlandAreaM2"]
            .as_f64()
            .unwrap(),
        headland as f64
    );
    assert_ne!(soil, fields.len() as u64 * 256);
    // Matching scene and sidecar plans cannot replace the manifest input.
    for mode in 0..3 {
        let mut bad = original.clone();
        match mode {
            0 => {
                bad["landcoverPlan"]["farmPattern"]["defaultField"]["widthM"] =
                    serde_json::json!(24)
            }
            1 => {
                bad["landcoverPlan"]
                    .as_object_mut()
                    .unwrap()
                    .remove("farmPattern");
            }
            _ => {
                bad["landcoverSurfaces"]["agriculture"]["fields"][0]["boundary"]["widthM"] =
                    serde_json::json!(24)
            }
        }
        fs::write(dir.0.join(scene_path), serde_json::to_vec(&bad).unwrap()).unwrap();
        fs::write(
            dir.0.join("scenes/scene-0/landcover-plan.json"),
            serde_json::to_vec(&bad["landcoverPlan"]).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.0.join("scenes/scene-0/landcover-surfaces.json"),
            serde_json::to_vec(&bad["landcoverSurfaces"]).unwrap(),
        )
        .unwrap();
        let result = run();
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains(if mode < 2 {
                "authoritative manifest"
            } else {
                "independent realization"
            }),
            "{error}"
        );
    }
    fs::write(
        dir.0.join(scene_path),
        serde_json::to_vec(&original).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.0.join("scenes/scene-0/landcover-plan.json"),
        serde_json::to_vec(&original["landcoverPlan"]).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.0.join("scenes/scene-0/landcover-surfaces.json"),
        serde_json::to_vec(&original["landcoverSurfaces"]).unwrap(),
    )
    .unwrap();
    assert!(run().status.success());
}

#[test]
fn opt_in_bake_trace_preserves_authoritative_outputs_and_reports_stages() {
    let dir = TestDir::new("bake-trace");
    let mut m = manifest(7);
    m.style["terrainProfile"] = serde_json::json!("plains");
    m.style["heightPrecision"] = serde_json::json!("millimeter");
    let input = dir.0.join("input.json");
    fs::write(&input, serde_json::to_vec(&m).unwrap()).unwrap();
    let quiet = dir.0.join("quiet");
    let traced = dir.0.join("traced");
    let run = |destination: &std::path::Path, enabled: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_glyphweave"));
        cmd.arg("generate-world").arg(&input).arg(destination);
        if enabled {
            cmd.env("GLYPHWEAVE_BAKE_TRACE", "1");
        } else {
            cmd.env_remove("GLYPHWEAVE_BAKE_TRACE");
        }
        cmd.output().unwrap()
    };
    let a = run(&quiet, false);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert!(!String::from_utf8_lossy(&a.stderr).contains("glyphweave.bake-trace"));
    let b = run(&traced, true);
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
    let events: Vec<Value> = String::from_utf8_lossy(&b.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["format"] == "glyphweave.bake-trace")
        .collect();
    for stage in [
        "manifest-validation",
        "natural-heightfield",
        "natural-drainage",
        "road-and-service-planning",
        "development-partition",
        "entity-generation-and-preflight",
        "final-heightfield-and-semantics",
        "final-entity-access-and-audit",
        "landcover-realization",
        "landcover-independent-reconstruction",
        "landcover-serialization",
        "landuse-accounting",
        "scene-serialization",
        "world-publication",
    ] {
        assert!(
            events.iter().any(|e| e["stage"] == stage),
            "missing {stage}"
        );
    }
    assert!(
        events
            .windows(2)
            .all(|p| p[0]["elapsedMs"].as_u64().unwrap() <= p[1]["elapsedMs"].as_u64().unwrap())
    );
    for file in [
        "world.json",
        "scenes/scene-0/scene.json",
        "scenes/scene-0/landcover-surfaces.json",
        "scenes/scene-0/chunk-0-0.height.bin",
        "scenes/scene-0/chunk-0-0.natural.height.bin",
    ] {
        assert_eq!(
            fs::read(quiet.join(file)).unwrap(),
            fs::read(traced.join(file)).unwrap(),
            "trace changed {file}"
        );
    }
}

#[test]
fn preview_export_is_derived_only_and_preserves_all_authority_bytes() {
    fn snapshot(root: &std::path::Path, path: &std::path::Path, out: &mut std::collections::BTreeMap<PathBuf, blake3::Hash>) {
        for entry in fs::read_dir(path).unwrap() {
            let path=entry.unwrap().path();
            let name=path.file_name().unwrap().to_string_lossy();
            if name=="preview" || name=="render-chunks" || name=="preview-scene.json" { continue; }
            if path.is_dir() { snapshot(root,&path,out); }
            else { out.insert(path.strip_prefix(root).unwrap().to_owned(),blake3::hash(&fs::read(path).unwrap())); }
        }
    }
    let dir=TestDir::new("preview-export");
    let m=manifest(7);
    bake_world(&m,&dir.0).unwrap();
    let mut before=std::collections::BTreeMap::new();
    snapshot(&dir.0,&dir.0,&mut before);
    assert!(before.len()>10);
    for _ in 0..2 {
        let result=Command::new(env!("CARGO_BIN_EXE_glyphweave")).arg("export-preview-stream").arg(&dir.0).output().unwrap();
        assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
        let mut after=std::collections::BTreeMap::new();snapshot(&dir.0,&dir.0,&mut after);
        assert_eq!(before,after);
    }
    let original=read(&dir,"scenes/scene-0/scene.json");
    let projected=read(&dir,"scenes/scene-0/preview-scene.json");
    assert_eq!(projected["entities"],original["entities"]);
    assert_eq!(projected["accessSurfaces"],original["accessSurfaces"]);
    let mut patches=std::collections::BTreeMap::new();
    let mut plants=std::collections::BTreeMap::new();
    for d in projected["renderStream"]["chunks"].as_array().unwrap() {
        let data=read(&dir,&format!("scenes/scene-0/{}",d["file"].as_str().unwrap()));
        for patch in data["patches"].as_array().unwrap() { patches.insert(patch["id"].as_str().unwrap().to_owned(),patch.clone()); }
        for plant in data["plants"].as_array().unwrap() { assert!(plants.insert(plant["id"].as_str().unwrap().to_owned(),plant.clone()).is_none()); }
    }
    let surfaces=&original["landcoverSurfaces"];
    assert!(!patches.is_empty());
    assert_eq!(patches.len(),surfaces["patches"].as_array().unwrap().len());
    assert_eq!(plants.len(),surfaces["ecology"]["plants"].as_array().unwrap().len());
    for patch in surfaces["patches"].as_array().unwrap() { assert_eq!(&patches[patch["id"].as_str().unwrap()],patch); }
    for plant in surfaces["ecology"]["plants"].as_array().unwrap() { assert_eq!(&plants[plant["id"].as_str().unwrap()],plant); }
}

#[test]
fn road_corridor_cli_keeps_design_separate_and_rejects_forgery() {
    use glyphweave_core::{baked_terrain::BakedTerrain,height_payload::encode_millimetres,natural::NaturalTerrainSnapshot};
    let dir=TestDir::new("corridor-cli");let world=dir.0.join("world");
    fs::create_dir_all(world.join("scenes/scene-0")).unwrap();
    let m=manifest(7);let s=&m.scenes[0];
    let bytes=encode_millimetres(512,512,(0..512).flat_map(|_|(0..512).map(|x|x as f64*0.04))).unwrap();
    let mut terrain=BakedTerrain::new(s).unwrap();terrain.insert_chunk(0,0,512,512,&bytes).unwrap();
    let mut natural=NaturalTerrainSnapshot::default();natural.heightfield_hash=terrain.content_hash().unwrap();
    let scene=serde_json::json!({"sceneId":"scene-0","widthM":512,"depthM":512,"originX":0,"originZ":0,"chunkSizeM":512,"chunkCountX":1,"chunkCountZ":1,"entities":[],"landmarks":[],"naturalTerrain":natural,
        "landUseZones":[{"id":"rural","kind":"rural","worldX":0,"worldZ":0,"widthM":512,"depthM":512,"source":"test"}],
        "chunks":[{"chunkX":0,"chunkZ":0,"worldX":0,"worldZ":0,"validWidthM":512,"validDepthM":512,"heightFile":"final.bin","naturalHeightFile":"natural.bin","surfaceFile":"surface","lod2File":"lod","hash":"test"}]});
    fs::write(world.join("scenes/scene-0/natural.bin"),&bytes).unwrap();
    fs::write(world.join("scenes/scene-0/scene.json"),serde_json::to_vec(&scene).unwrap()).unwrap();
    fs::write(world.join("glyphweave.manifest.json"),serde_json::to_vec(&m).unwrap()).unwrap();
    fs::write(world.join("world.json"),serde_json::to_vec(&serde_json::json!({"format":"glyphweave-world","version":1,"name":"study","seed":7,"renderMode":"2.5d","revision":"test","scenes":["scenes/scene-0/scene.json"]})).unwrap()).unwrap();
    let request=dir.0.join("request.json");let output=dir.0.join("design.json");
    fs::write(&request,br#"{"sceneId":"scene-0","start":[32,128],"end":[288,128],"roadWidthM":8,"shoulderWidthM":3,"maxFillMm":6000,"maxRiseMmPerM":80,"startLevelMm":null,"endLevelMm":null,"junctionLengthM":8}"#).unwrap();
    let run=|verb:&str,out:&std::path::Path|Command::new(env!("CARGO_BIN_EXE_glyphweave")).arg(verb).arg(&world).arg(&request).arg(out).output().unwrap();
    let result=run("design-road-corridor",&output);assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    assert!(run("verify-road-corridor",&output).status.success());
    assert_eq!(fs::read(world.join("scenes/scene-0/natural.bin")).unwrap(),bytes);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(world.join("scenes/scene-0/scene.json")).unwrap()).unwrap(),scene);
    assert!(!run("design-road-corridor",&output).status.success(),"must not overwrite existing study");
    assert!(!run("design-road-corridor",&world.join("unsafe.json")).status.success());assert!(!world.join("unsafe.json").exists());
    let original:Value=serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(original["status"],"design_only_not_applied");
    for mode in 0..3 {
        let mut forged=original.clone();match mode {0=>forged["profileMm"][10]=serde_json::json!(99999),1=>forged["roadMesh"]["vertices"][5][1]=serde_json::json!(99),_=>forged["proposedFillVolumeM3"]=serde_json::json!(999)};
        fs::write(&output,serde_json::to_vec(&forged).unwrap()).unwrap();assert!(!run("verify-road-corridor",&output).status.success());
    }
    fs::write(&output,serde_json::to_vec(&original).unwrap()).unwrap();
    let mut corrupt=bytes.clone();let last=corrupt.len()-1;corrupt[last]^=1;fs::write(world.join("scenes/scene-0/natural.bin"),corrupt).unwrap();
    assert!(!run("verify-road-corridor",&output).status.success(),"full source hash must bind samples outside corridor too");
}

#[test]
fn graded_extensions_are_applied_and_independently_checked_not_just_designs() {
    use glyphweave_core::{baked_terrain::BakedTerrain,worldgen::{SceneIndex,road_network,water_geometry,water_kind,services}};
    let dir=TestDir::new("graded-applied");
    let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../examples/graded-road-extensions.manifest.json");
    let mut m:WorldManifest=serde_json::from_slice(&fs::read(input).unwrap()).unwrap();m.style["rulesDir"]=serde_json::json!(rules());
    bake_world(&m,&dir.0).unwrap();let s=&m.scenes[0];
    let original=read(&dir,"scenes/scene-0/scene.json");
    let scene:SceneIndex=serde_json::from_value(original.clone()).unwrap();let plan=scene.road_network_plan.as_ref().unwrap();
    assert_eq!(plan.extensions.len(),1);assert_eq!(plan.edges.len(),32);
    assert_eq!(read(&dir,"scenes/scene-0/road-network-audit.json")["status"],"pass");
    let road=scene.entities.iter().find(|e|e.entity_id==plan.extensions[0].entity_id).unwrap();
    let profile=road.grounding.road_profile.as_ref().unwrap();assert_ne!(profile.elevations_mm[0],*profile.elevations_mm.last().unwrap());
    let engineering=read(&dir,"scenes/scene-0/engineered-terrain.json");assert_eq!(engineering["version"],2);
    assert!(engineering["operations"].as_array().unwrap().iter().any(|o|o["entityId"]==road.entity_id&&o["roadProfile"].is_object()));
    let ledger=read(&dir,"scenes/scene-0/land-use-ledger.json");assert_eq!(ledger["byPurpose"]["urban"]["physical"]["roadsM2"],256.0);
    let c=&scene.chunks[0];let folder=dir.0.join("scenes/scene-0");let nb=fs::read(folder.join(c.natural_height_file.as_ref().unwrap())).unwrap();let fb=fs::read(folder.join(&c.height_file)).unwrap();
    let mut natural=BakedTerrain::new(s).unwrap();natural.insert_chunk(c.world_x,c.world_z,c.valid_width_m,c.valid_depth_m,&nb).unwrap();
    let mut terrain=BakedTerrain::new(s).unwrap();terrain.insert_chunk(c.world_x,c.world_z,c.valid_width_m,c.valid_depth_m,&fb).unwrap();
    let water=water_geometry(water_kind(&m.style),&m.landmarks,s,&m.style);let program=services::from_style(&m.style).unwrap();
    let verify=|t:&BakedTerrain|road_network::verify_with_style(plan,s,&natural,t,water,&scene.entities,Some(&scene.land_use_zones),program.as_ref(),&m.style).unwrap();
    assert_eq!(verify(&terrain).status,"pass");
    for x in 217..=231 {for z in 92..=164 {assert!((f64::from(terrain.height(x,z).unwrap())-profile.height_at(f64::from(x),f64::from(z)).unwrap()).abs()<0.00005);}}
    // Corrupt only the outer shoulder by one millimetre. Generic 0.26m legacy
    // road tolerance must not hide corruption in the new precise contract.
    let mut bad=fb.clone();let offset=16+4*(128*512+231);let value=i32::from_le_bytes(bad[offset..offset+4].try_into().unwrap())+1;bad[offset..offset+4].copy_from_slice(&value.to_le_bytes());
    let mut altered=BakedTerrain::new(s).unwrap();altered.insert_chunk(0,0,512,512,&bad).unwrap();let failed=verify(&altered);assert_eq!(failed.status,"fail");assert!(failed.max_design_height_error_m>0.0009);
    let run=||Command::new(env!("CARGO_BIN_EXE_glyphweave")).arg("quality-report").arg(&dir.0).output().unwrap();
    assert!(run().status.success());assert_eq!(read(&dir,"quality-report.json")["scenes"][0]["roadNetworkAudit"]["status"],"pass");
    let mut forged=original.clone();forged["roadNetworkPlan"]["extensions"][0]["design"]["profileMm"][20]=serde_json::json!(9999);
    fs::write(folder.join("scene.json"),serde_json::to_vec(&forged).unwrap()).unwrap();fs::write(folder.join("road-network-plan.json"),serde_json::to_vec(&forged["roadNetworkPlan"]).unwrap()).unwrap();
    assert!(!run().status.success(),"forging both copies must not bypass manifest-bound reconstruction");
}

#[test]
fn automatic_road_bake_rebuilds_choices_and_keeps_budget_shortfalls() {
    let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../examples/automatic-road-growth.manifest.json");
    let mut m:WorldManifest=serde_json::from_slice(&fs::read(input).unwrap()).unwrap();m.style["rulesDir"]=serde_json::json!(rules());
    assert!(m.style.get("roadExtensions").is_none());
    let dir=TestDir::new("automatic-road-bake");bake_world(&m,&dir.0).unwrap();
    let scene=read(&dir,"scenes/scene-0/scene.json");let a=&scene["roadNetworkPlan"]["automaticExtensions"];
    assert!(a["acceptedSegments"].as_u64().unwrap()>0);assert_eq!(a["unservedTargets"],0);
    assert!(a["targets"].as_array().unwrap().iter().any(|t|t["purpose"]=="urban"&&t["beforeHasRoad"]==false&&t["afterHasRoad"]==true));
    assert_eq!(read(&dir,"scenes/scene-0/road-network-audit.json")["status"],"pass");
    assert_eq!(read(&dir,"scenes/scene-0/service-program-audit.json")["realizedWithAccess"],4);
    assert!(scene["roadNetworkPlan"]["extensions"].as_array().unwrap().iter().any(|e|e["design"]["request"]["endLevelMm"].as_i64().unwrap()%1000!=0));
    let run=|command:&str,d:&TestDir|Command::new(env!("CARGO_BIN_EXE_glyphweave")).arg(command).arg(&d.0).output().unwrap();
    assert!(run("scale-audit",&dir).status.success());assert!(run("quality-report",&dir).status.success());
    assert_eq!(read(&dir,"quality-report.json")["scenes"][0]["automaticRoads"],*a);
    // Copy-consistent forged outcomes must still be rejected against the input.
    let mut forged=scene.clone();forged["roadNetworkPlan"]["automaticExtensions"]["acceptedSegments"]=serde_json::json!(0);
    fs::write(dir.0.join("scenes/scene-0/scene.json"),serde_json::to_vec(&forged).unwrap()).unwrap();
    fs::write(dir.0.join("scenes/scene-0/road-network-plan.json"),serde_json::to_vec(&forged["roadNetworkPlan"]).unwrap()).unwrap();
    assert!(!run("quality-report",&dir).status.success());
    // A smaller explicit budget is a negative test, not a way to pass a gate.
    m.style["automaticRoadExtensions"]["scenes"][0]["maxNewSegments"]=serde_json::json!(1);
    let limited=TestDir::new("automatic-road-budget");bake_world(&m,&limited.0).unwrap();
    let a=read(&limited,"scenes/scene-0/road-network-plan.json")["automaticExtensions"].clone();
    assert_eq!(a["acceptedSegments"],1);assert_eq!(a["stopReason"],"segment_budget_exhausted");assert!(a["unservedTargets"].as_u64().unwrap()>0);
    assert!(!run("world-quality-gate",&limited).status.success());
    assert!(read(&limited,"world-quality-gate.json")["failures"].as_array().unwrap().iter().any(|s|s.as_str().unwrap().contains("automatic road targets")));
}

#[test]
fn local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/local-height-frontage.manifest.json");
    let mut manifest: WorldManifest = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    manifest.style["rulesDir"] = serde_json::json!(rules());
    let dir = TestDir::new("local-height-frontage");
    bake_world(&manifest, &dir.0).unwrap();
    let scene_path = dir.0.join("scenes/scene-0/scene.json");
    let scene_bytes = fs::read(&scene_path).unwrap();
    let scene: Value = serde_json::from_slice(&scene_bytes).unwrap();
    let generated: Vec<_> = scene["entities"].as_array().unwrap().iter()
        .filter(|e| e["entityId"].as_str().unwrap().starts_with("generated.frontage."))
        .collect();
    assert_eq!(generated.len(), 5, "8 requested buildings must retain the 3-home shortfall");
    assert!(generated.iter().any(|e| e["grounding"]["bottomOffsetM"].as_f64().unwrap() != 0.));
    for e in &generated {
        assert!(e["grounding"]["frontageRoadPoint"].is_array());
    }
    let outcomes = scene["roadNetworkPlan"]["frontage"]["outcomes"].as_array().unwrap();
    let homes = outcomes.iter().find(|o| o["request"]["id"] == "urban-homes").unwrap();
    let shops = outcomes.iter().find(|o| o["request"]["id"] == "urban-shops").unwrap();
    assert_eq!(homes["request"]["count"], 6);
    assert_eq!(homes["unfilled"], 3, "the original residential deficit must not disappear");
    assert_eq!(shops["request"]["count"], 2);
    assert_eq!(shops["unfilled"], 0);

    let run = |command: &str| Command::new(env!("CARGO_BIN_EXE_glyphweave"))
        .arg(command).arg(&dir.0).output().unwrap();
    // Remove any prior report before every quality run: a failed command may
    // leave no report, and an old report must never prove restored-scene success.
    let fresh_quality = || -> std::process::Output {
        let path = dir.0.join("quality-report.json");
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
        run("quality-report")
    };
    let assert_contract = |q: &Value| {
        assert!(q["accessGraph"].is_object(), "independent graph must be present");
        assert_eq!(scene["accessGraph"], q["accessGraph"],
            "persisted graph must equal the independent final-entity reconstruction");
        assert_eq!(q["roadNetworkAudit"]["status"], "pass", "{}", q["roadNetworkAudit"]);
        assert_eq!(q["landProtectionAudit"]["changedHeightSamples"], 0);
        assert_eq!(q["serviceProgramAudit"]["requested"], 4);
        assert_eq!(q["serviceProgramAudit"]["realizedWithAccess"], 4);
        assert_eq!(q["frontageAudit"]["requested"], 8);
        assert_eq!(q["frontageAudit"]["planned"], 5);
        assert_eq!(q["frontageAudit"]["unfilled"], 3);
        assert_eq!(q["frontageAudit"]["realizedWithAccess"].as_u64().unwrap(), generated.len() as u64);
        assert!(q["landUseLedger"]["byPurpose"]["urban"]["physical"]["buildingsM2"]
            .as_f64().unwrap() > 0.);
    };
    let quality = fresh_quality();
    assert!(quality.status.success(), "{}", String::from_utf8_lossy(&quality.stderr));
    let baseline_report = read(&dir, "quality-report.json");
    let baseline = &baseline_report["scenes"][0];
    assert_contract(baseline);
    let baseline_gate_run = run("world-quality-gate");
    assert_eq!(baseline_gate_run.status.code(), Some(1), "{}",
        String::from_utf8_lossy(&baseline_gate_run.stderr));
    let baseline_gate = read(&dir, "world-quality-gate.json");
    assert_eq!(baseline_gate["status"], "fail");
    assert!(baseline_gate["failures"].as_array().unwrap().iter().any(|f|
        f.as_str().unwrap().contains("local-height frontage demand or realization remains incomplete")));

    let entity_id = generated[0]["entityId"].as_str().unwrap();
    let source = &scene["developmentPlan"]["buildings"].as_array().unwrap().iter()
        .find(|b| b["entity"]["entityId"] == entity_id).unwrap()["entity"];
    // The CLI checks the independently rebuilt ownership budget before it
    // emits frontageAudit. development::same_entity includes anchors and
    // grounding (only derived bounds are ignored). These are its actual
    // diagnostic and source fields, not invented anchor-specific CLI errors.
    for field in ["anchor", "roadPoint"] {
        let mut changed = scene.clone();
        let entity = changed["entities"].as_array_mut().unwrap().iter_mut()
            .find(|e| e["entityId"] == entity_id).unwrap();
        assert_eq!(entity["anchors"], source["anchors"]);
        assert_eq!(entity["grounding"], source["grounding"]);
        if field == "anchor" {
            let x = entity["anchors"][0]["worldX"].as_i64().unwrap();
            entity["anchors"][0]["worldX"] = serde_json::json!(x + 1);
            assert_ne!(entity["anchors"], source["anchors"]);
            assert_eq!(entity["grounding"], source["grounding"]);
        } else {
            let x = entity["grounding"]["frontageRoadPoint"][0].as_i64().unwrap();
            entity["grounding"]["frontageRoadPoint"][0] = serde_json::json!(x + 1);
            assert_ne!(entity["grounding"]["frontageRoadPoint"], source["grounding"]["frontageRoadPoint"]);
            assert_eq!(entity["anchors"], source["anchors"]);
        }
        let mut reverted = changed.clone();
        let original_coordinate = if field == "anchor" {
            "/anchors/0/worldX"
        } else {
            "/grounding/frontageRoadPoint/0"
        };
        let entity = reverted["entities"].as_array_mut().unwrap().iter_mut()
            .find(|e| e["entityId"] == entity_id).unwrap();
        *entity.pointer_mut(original_coordinate).unwrap() =
            generated[0].pointer(original_coordinate).unwrap().clone();
        assert_eq!(reverted, scene, "{field} must be the only changed source field");
        fs::write(&scene_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let rejected = fresh_quality();
        let diagnostic = String::from_utf8_lossy(&rejected.stderr);
        assert_eq!(rejected.status.code(), Some(1),
            "{entity_id} {field} mutation must be a CLI rejection, not a crash: {diagnostic}");
        assert!(diagnostic.contains(
            "glyphweave: persisted land-use budget differs from rebuilt entity and zone areas"),
            "{entity_id} {field} must reject its changed source ownership: {diagnostic}");
        assert!(!diagnostic.contains("panicked"), "{diagnostic}");
        assert!(!dir.0.join("quality-report.json").exists(),
            "source rejection must not reuse a previously successful report");
        assert_eq!(read(&dir, "scenes/scene-0/scene.json"), changed,
            "quality-report must not repair the {field} mutation");

        // Restore the exact baseline bytes, run a new independent audit, then
        // compare the new graph and all demand/realization/protection ledgers.
        fs::write(&scene_path, &scene_bytes).unwrap();
        let restored_quality = fresh_quality();
        assert!(restored_quality.status.success(), "after restoring {field}: {}",
            String::from_utf8_lossy(&restored_quality.stderr));
        let restored_report = read(&dir, "quality-report.json");
        let restored = &restored_report["scenes"][0];
        assert_contract(restored);
        for key in ["accessGraph", "frontageAudit", "serviceProgramAudit",
            "landProtectionAudit", "roadNetworkAudit", "landUseLedger"] {
            assert_eq!(restored[key], baseline[key], "{key} changed after restoring {field}");
        }
        assert_eq!(fs::read(&scene_path).unwrap(), scene_bytes,
            "restored {field} audit must not mutate the original scene");
    }
    let restored_gate_run = run("world-quality-gate");
    assert_eq!(restored_gate_run.status.code(), Some(1), "{}",
        String::from_utf8_lossy(&restored_gate_run.stderr));
    let restored_gate = read(&dir, "world-quality-gate.json");
    assert_eq!(restored_gate["status"], "fail");
    assert_eq!(restored_gate["failures"], baseline_gate["failures"],
        "restoration must not erase or change the original gate failures");

    // Copy-consistent metadata edits are not source evidence.
    let mut forged = scene.clone();
    forged["roadNetworkPlan"]["frontage"]["outcomes"][0]["unfilled"] = serde_json::json!(0);
    fs::write(&scene_path, serde_json::to_vec(&forged).unwrap()).unwrap();
    fs::write(dir.0.join("scenes/scene-0/road-network-plan.json"),
        serde_json::to_vec(&forged["roadNetworkPlan"]).unwrap()).unwrap();
    let forged_quality = fresh_quality();
    assert_eq!(forged_quality.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&forged_quality.stderr)
        .contains("road plan differs from natural raster reconstruction"),
        "{}", String::from_utf8_lossy(&forged_quality.stderr));
    fs::write(&scene_path, &scene_bytes).unwrap();
    fs::write(dir.0.join("scenes/scene-0/road-network-plan.json"),
        serde_json::to_vec(&scene["roadNetworkPlan"]).unwrap()).unwrap();
    // A single-millimetre support corruption must never be accepted.
    let e = generated[0];
    let x = e["worldX"].as_i64().unwrap() as usize;
    let z = e["worldZ"].as_i64().unwrap() as usize;
    let path = dir.0.join("scenes/scene-0/chunk-0-0.height.bin");
    let mut payload = fs::read(&path).unwrap();
    let offset = 16 + (z * 512 + x) * 4;
    let old = i32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap());
    payload[offset..offset + 4].copy_from_slice(&(old + 1).to_le_bytes());
    fs::write(path, payload).unwrap();
    assert!(!fresh_quality().status.success());
}
