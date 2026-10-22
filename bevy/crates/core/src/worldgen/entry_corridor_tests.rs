//! The graded entry corridor's contract, measured on one fixture.
//!
//! ## The fixture
//!
//! A 256 m scene at the origin. The natural ground is FLAT at `-1.000 m`, so the
//! lot stands on its own datum with no ring of its own to speak of: everything that
//! stands between its door and its street is the STREET's fill. One flat 8 m street
//! (64 m long) is centred at `(128, 50)`, publishes its surface at `STREET_MM`, and
//! carries the OPT-IN 6 m fill-transition ring (`earthworkBlendM = 6`) instead of
//! the historical 100 m conservative envelope. One 24x24 lot stands at `(128, 78)`
//! with its door facing north: the door cell is `(128, 65)`, the lot's `+-2 m` audit
//! window ends at `z = 64`, and the street's graded band (an unprofiled 8 m road
//! carves `depth/2 + 1.5 = 5.5 m`) ends at `z = 55`. The corridor therefore runs
//! `z = 63 -> 56`: **7 m of run**, two cells wide.
//!
//! That is the R54 shape exactly: `1.500 m` of roadbed fill across a 6 m ring is a
//! cubic smoothstep whose steepest 1 m step is `36.9 %` (`1.5 x 1.5 / 6` at the
//! ring's midpoint), and the street's own band publishes its surface over the whole
//! pavement. The checked route refuses the door, and the straight approach's
//! minimum feasible grade - the number the refusal quotes - is the linear rise over
//! the run the lot can offer. With `STREET_MM = +500` that is `1.500 / 7 = 21.4 %`
//! (the corridor fits); with `STREET_MM = +1500` it is `2.500 / 7 = 35.7 %` (it does
//! not, and the refusal names that number).

use super::*;
use crate::baked_terrain::BakedTerrain;
use crate::worldgen::road_network::RoadsideSite;
use crate::worldgen::{EntityInstance, GroundingSpec, SceneSpec, WaterKind, water_geometry};
use serde_json::json;

const LOT: &str = "lot";
const STREET: &str = "street";
/// The lot's declared top: exactly the natural ground, so its platform is flat and
/// its own ring is inert - the street's fill is the whole obstruction.
const PAD_MM: i32 = -1_000;
/// The street's own fill-transition ring. The historical default is the
/// conservative `3 x (ground + 32)` envelope (100 m here), which spreads 1.5 m of
/// fill over a 4 % slope and would hide the defect this fixture is about; 6 m is the
/// derived ring's own floor and the number this fixture measures.
const STREET_RING_M: f32 = 6.0;

fn scene() -> SceneSpec {
    SceneSpec {
        scene_id: "entry-corridor".into(),
        width_m: 256,
        depth_m: 256,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    }
}

/// Natural ground: flat at `-1.000 m` everywhere.
fn natural() -> BakedTerrain {
    let scene = scene();
    let mut terrain = BakedTerrain::new(&scene).unwrap();
    let values = vec![-1.0_f64; 256 * 256];
    terrain
        .insert_chunk(
            0,
            0,
            256,
            256,
            &crate::height_payload::encode_millimetres(256, 256, values).unwrap(),
        )
        .unwrap();
    terrain
}

fn water() -> WaterGeometry {
    water_geometry(WaterKind::None, &[], &scene(), &serde_json::Value::Null)
}

/// The flat street: 64 m long, 8 m of pavement, its surface at `street_mm`, its
/// fill-transition ring narrowed to the OPT-IN 6 m.
fn street_at(street_mm: i32) -> EntityInstance {
    let metres = f64::from(street_mm) / 1000.;
    EntityInstance {
        entity_id: STREET.into(),
        asset_id: "prop.road".into(),
        kind: "road".into(),
        world_x: 128,
        world_z: 50,
        world_y: metres.floor() as i32,
        width_m: 64.,
        depth_m: 8.,
        height_m: 0.24,
        scale: 1.,
        rotation_y_deg: 0.,
        grounding: GroundingSpec {
            bottom_offset_m: (metres - metres.floor()) as f32,
            earthwork_blend_m: Some(STREET_RING_M),
            ..Default::default()
        },
        anchors: vec![],
        bounds: None,
    }
}

/// The lot: one 24x24 pad whose door faces north, its flat top on the natural
/// ground, no cap and no derived ring of its own.
fn lot() -> RoadsideSite {
    RoadsideSite {
        program: None,
        purpose: None,
        id: LOT.into(),
        road_id: STREET.into(),
        world_x: 128,
        world_z: 78,
        width_m: 24,
        depth_m: 24,
        support_level_m: PAD_MM.div_euclid(1000),
        parcel_insets: None,
        support_level_mm: Some(PAD_MM),
        frontage_road_point: None,
        bounded_cut_mm: None,
        earthwork_blend_m: None,
        entry_corridor: None,
        rotation_y_deg: 0.,
        natural_min_m: -1.,
        natural_max_m: -1.,
    }
}

/// The plan's own carves for the fixture, exactly as the bake would plan them.
fn carves(site: &RoadsideSite, street: &EntityInstance) -> Vec<super::super::TerrainCarve> {
    super::super::plan_terrain_carves(&[street.clone(), site.entity()])
}

/// The composed surface at one cell: the bake's own composition, without zoning.
fn composed(carves: &[super::super::TerrainCarve], terrain: &BakedTerrain, x: i32, z: i32) -> f64 {
    let base = f64::from(terrain.height(x, z).unwrap());
    super::super::apply_terrain_carves(base, x, z, carves)
}

/// The steepest 1 m step along the pedestrian line `x = 128` from the lot's door
/// (`z = 64`, the last cell of its window) down to the street's pavement edge
/// (`z = 54`), on the composed surface. This is the shape the checked route's 0.5 m
/// sampling refuses.
fn steepest_step_on_the_line(
    carves: &[super::super::TerrainCarve],
    terrain: &BakedTerrain,
) -> (f64, i32) {
    let mut previous: Option<f64> = None;
    let mut worst = 0.0_f64;
    let mut at = 0_i32;
    for z in (54..=64).rev() {
        let h = composed(carves, terrain, 128, z);
        if let Some(old) = previous {
            let grade = (h - old).abs();
            if grade > worst {
                worst = grade;
                at = z;
            }
        }
        previous = Some(h);
    }
    (worst, at)
}

/// The gate alone: no corridor is ever offered.
fn gate(
    site: &RoadsideSite,
    street: &EntityInstance,
    terrain: &BakedTerrain,
) -> Option<(String, String)> {
    super::super::road_network::assigned_road_gate(
        site,
        std::slice::from_ref(street),
        &[],
        terrain,
        None,
        0,
        water(),
    )
    .unwrap()
    .map(|(category, reason)| (category.to_owned(), reason))
}

/// The gate with the corridor attempt this module adds.
fn gate_with(
    site: &mut RoadsideSite,
    street: &EntityInstance,
    terrain: &BakedTerrain,
    policy: Policy,
) -> Option<(String, String)> {
    gate_or_corridor(
        site,
        std::slice::from_ref(street),
        &[],
        terrain,
        None,
        0,
        water(),
        policy,
        6.0,
    )
    .unwrap()
}

/// Build the same published raster the bake would write, from a plan's carves.
fn published_raster(carves: &[super::super::TerrainCarve], terrain: &BakedTerrain) -> BakedTerrain {
    let scene = scene();
    let mut values = Vec::with_capacity(256 * 256);
    for z in 0..256 {
        for x in 0..256 {
            values.push(composed(carves, terrain, x, z));
        }
    }
    let mut raster = BakedTerrain::new(&scene).unwrap();
    raster
        .insert_chunk(
            0,
            0,
            256,
            256,
            &crate::height_payload::encode_millimetres(256, 256, values).unwrap(),
        )
        .unwrap();
    raster
}

/// Zones that cover the whole 256x256 scene exactly once, with one 4x4 nature
/// patch the corridor has to respect.
fn zones_with_nature() -> Vec<crate::worldgen::LandUseZone> {
    let zone =
        |id: &str, kind: &str, x: i32, z: i32, w: u32, d: u32| crate::worldgen::LandUseZone {
            id: id.into(),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            width_m: w,
            depth_m: d,
            source: "fixture".into(),
        };
    vec![
        zone("rural.north", "rural", 0, 0, 256, 58),
        zone("rural.west", "rural", 0, 58, 126, 4),
        zone("nature", "nature", 126, 58, 4, 4),
        zone("rural.east", "rural", 130, 58, 126, 4),
        zone("rural.south", "rural", 0, 62, 256, 194),
    ]
}

// ---------------------------------------------------------------------------
// 1. The switch itself.
// ---------------------------------------------------------------------------

/// The policy is OFF unless the exact authored value is declared: absent and every
/// other spelling is off, and `Policy::is_off` (the serde skip) is what keeps the
/// plan record byte-identical for a world that never asked.
#[test]
fn the_corridor_is_off_unless_the_exact_value_is_declared() {
    for style in [
        json!({}),
        json!({"terrainPolicy": {}}),
        json!({"terrainPolicy": {"entryCorridor": "on"}}),
        json!({"terrainPolicy": {"entryCorridor": "Graded"}}),
        json!({"terrainPolicy": {"entryCorridor": true}}),
        json!({"frontageProgram": {"terrainPolicy": "graded"}}),
    ] {
        let (policy, reason) = Policy::from_style(&style);
        assert_eq!(policy, Policy::Off, "{style}");
        assert!(!policy.enabled(), "{style}");
        assert!(reason.is_some(), "{style} must name why it is off");
        assert!(policy.is_off(), "{style}");
    }
    let (policy, reason) =
        Policy::from_style(&json!({"terrainPolicy": {"entryCorridor": "graded"}}));
    assert_eq!(policy, Policy::Graded);
    assert_eq!(reason, None);
    assert!(!policy.is_off());
    // The policy serializes to the authored word, and an OFF one to its own.
    assert_eq!(serde_json::to_value(policy).unwrap(), json!("graded"));
    let (off, _) = Policy::from_style(&json!({}));
    assert_eq!(serde_json::to_value(off).unwrap(), json!("off"));
}

// ---------------------------------------------------------------------------
// 2. The fixture really is the R54 shape.
// ---------------------------------------------------------------------------

/// Before any corridor: the gate refuses the lot, and the obstruction is the
/// STREET's own fill slope - not the natural ground, which is flat. With the street
/// `1.500 m` above the natural ground, the measured steepest 1 m step of the
/// composed surface is the 37.0 % the 6 m ring produces for that fill
/// (`1.5 x 1.5 / 6` at the ring's midpoint), and the straight approach's minimum
/// feasible grade is `1.500 / 7 = 21.4 %` - which the corridor can hold.
#[test]
fn the_fixture_approach_is_a_steep_fill_slope_and_the_gate_refuses_it() {
    let terrain = natural();
    let street = street_at(500);
    let mut site = lot();
    let planned = carves(&site, &street);
    let (step, at) = steepest_step_on_the_line(&planned, &terrain);
    assert!(
        (step - 0.370).abs() < 0.005,
        "the street's own 6 m ring: 1.5 x 1.5 / 6 at its midpoint, measured {step:.4} at z={at}"
    );
    assert!(
        step > 0.25,
        "the fixture's approach must break the 25% contract: {step}"
    );

    let refusal = gate(&site, &street, &terrain).expect("the lot is not enterable today");
    assert_eq!(refusal.0, "assigned_road_not_reachable_at_acceptance");
    // The straight approach: 1.5 m of rise over the 7 m the corridor would get.
    let measured = 1.5_f64 / 7.0;
    assert!((measured - 0.214).abs() < 1e-3, "21.4% = 1.5 m over 7 m");
    // And with the policy OFF the corridor attempt changes nothing at all.
    assert_eq!(
        gate_with(&mut site, &street, &terrain, Policy::Off),
        Some(refusal)
    );
    assert!(
        site.entry_corridor.is_none(),
        "the off state plans no corridor"
    );
    // A street that stands 2.5 m above the lot is the same shape, steeper: the
    // fill is 2.5 m, the ring's steepest 1 m step is 61.9 %, and the straight
    // approach's minimum grade is 35.7 % - over the contract, so that one is
    // refused by name (test ③ below).
    let high = street_at(1_500);
    let (step, at) = steepest_step_on_the_line(&carves(&site, &high), &terrain);
    assert!(
        (step - 0.619).abs() < 0.02,
        "1.5 x 2.5 / 6 at the ring's midpoint, measured {step:.4} at z={at}"
    );
    assert!((2.5_f64 / 7.0 - 0.357).abs() < 1e-3);
}

// ---------------------------------------------------------------------------
// 3. ① A graded corridor is planned, and it is walkable cell by cell.
// ---------------------------------------------------------------------------

/// With the street `1.500 m` above the lot's top the corridor has `1.500 / 7 =
/// 21.4 %` to spend: the gate passes, the pad keeps a corridor, and every
/// station-to-station step of the PUBLISHED surface under the corridor is at or
/// below the 25 % contract (the design grade is 21.4 %, and the raster is
/// quantized to whole millimetres).
#[test]
fn the_graded_corridor_opens_the_entrance_and_is_walkable_cell_by_cell() {
    let terrain = natural();
    let street = street_at(500);
    let mut site = lot();
    assert!(
        gate(&site, &street, &terrain).is_some(),
        "the lot needs the corridor"
    );
    let refusal = gate_with(&mut site, &street, &terrain, Policy::Graded);
    assert_eq!(refusal, None, "the corridor must open the entrance");

    let corridor = site
        .entry_corridor
        .clone()
        .expect("the pad keeps its corridor");
    assert_eq!(corridor.version, VERSION);
    assert_eq!(corridor.pad_id, LOT);
    assert_eq!(corridor.road_id, STREET);
    assert_eq!(corridor.axis, Axis::Z);
    assert_eq!(corridor.width_m, 2);
    // The corridor runs from the first cell outside the lot's +-2 m window
    // (z = 63) to the first cell outside the street's graded band (z = 56).
    assert_eq!((corridor.from[0], corridor.from[1]), (128, 63));
    assert_eq!((corridor.to[0], corridor.to[1]), (129, 56));
    assert_eq!(corridor.length_m, 7.0);
    assert_eq!(corridor.stations(), 7);
    assert_eq!(corridor.start_mm, PAD_MM);
    assert_eq!(corridor.end_mm, 500, "the street's own published surface");
    assert_eq!(corridor.cell_count, 16, "8 stations x 2 m of pavement");
    assert_eq!(corridor.cells.len(), 16);
    assert_eq!(
        corridor.max_fill_mm, 1_500,
        "the deepest fill at the street end"
    );
    assert_eq!(corridor.fill_sum_mm, 12_000);

    // Cell by cell, on the surface the bake publishes: the corridor's own profile
    // is at or below the contract, and it climbs toward the street at 21.4%.
    let planned = carves(&site, &street);
    let mut previous: Option<f64> = None;
    let mut steepest = 0.0_f64;
    for station in 0..=corridor.stations() {
        let height = composed(&planned, &terrain, 128, 63 - station);
        if let Some(old) = previous {
            let step = (height - old).abs();
            assert!(
                step <= GRADE_CONTRACT + 1e-9,
                "station {station} steps {:.3} m, above the {:.0}% contract",
                step,
                GRADE_CONTRACT * 100.
            );
            steepest = steepest.max(step);
        }
        previous = Some(height);
    }
    assert!(
        (steepest - 1.5 / 7.0).abs() < 2.0 / 1000.,
        "the ramp is laid at {:.1}%, not {:.1}%",
        steepest * 100.,
        1.5 / 7.0 * 100.
    );
    // The route itself agrees: the lot's door now reaches its own street.
    let links = super::super::access::plan_time_links(
        &site.entity(),
        STREET,
        std::slice::from_ref(&street),
        &[],
        &terrain,
        None,
        None,
        water(),
    )
    .unwrap();
    assert!(
        links
            .iter()
            .any(|link| link.connected && link.road_id.as_deref() == Some(STREET)),
        "{links:#?}"
    );
}

// ---------------------------------------------------------------------------
// 4. ② The ledger, and the audit that recomputes it.
// ---------------------------------------------------------------------------

/// The record's length, endpoints, totals and per-cell elevations all agree with an
/// INDEPENDENT recomputation from the record's own geometry and the natural raster,
/// and with the raster the bake publishes: every corridor cell publishes exactly
/// `max(natural, ramp)`. Mutating one recorded cell (or the published raster) is
/// caught by name.
#[test]
fn the_corridor_ledger_recomputes_from_the_plan_and_the_natural_raster() {
    let terrain = natural();
    let street = street_at(500);
    let mut site = lot();
    assert_eq!(
        gate_with(&mut site, &street, &terrain, Policy::Graded),
        None
    );
    let planned = carves(&site, &street);
    let published = published_raster(&planned, &terrain);
    let sites = vec![site.clone()];

    let audit = verify(&sites, &terrain, &published, &scene(), None, water(), 6.0);
    assert_eq!(audit.status, "pass", "{:#?}", audit.failures);
    assert_eq!(audit.corridor_count, 1);
    assert_eq!(audit.cells, 16);
    assert_eq!(audit.longest_m, 7.0);
    assert_eq!(
        audit.max_grade_permille, 215,
        "1.500 m over 7 m, rounded per station"
    );
    assert_eq!(audit.grade_contract, 0.25);
    assert_eq!(audit.length_budget_m, 120.0);
    assert_eq!(audit.fill_sum_mm, 12_000);
    assert_eq!(audit.max_fill_mm, 1_500);
    let row = &audit.corridors[0];
    assert_eq!(row.pad_id, LOT);
    assert_eq!(row.status, "pass");
    assert_eq!(row.cells, 16);
    let corridor = sites[0].entry_corridor.as_ref().unwrap();
    let recomputed_sum: i64 = corridor
        .cells
        .iter()
        .map(|cell| i64::from(cell.target_mm - cell.natural_mm))
        .sum();
    assert_eq!(corridor.fill_sum_mm, recomputed_sum);
    assert_eq!(row.fill_sum_mm, recomputed_sum);
    // Every cell's natural is the natural raster, its target is the published one.
    for cell in &corridor.cells {
        assert_eq!(
            i64::from(cell.natural_mm),
            i64::from(terrain.height_millimetres(cell.x, cell.z).unwrap())
        );
        assert_eq!(
            i64::from(cell.target_mm),
            i64::from(published.height_millimetres(cell.x, cell.z).unwrap())
        );
        assert_eq!(cell.fill_mm, cell.target_mm - cell.natural_mm);
        assert!(cell.fill_mm >= 0, "the corridor never excavates");
    }

    // A forged cell is refused by name.
    let mut forged = sites.clone();
    let broken = forged[0].entry_corridor.as_mut().unwrap();
    broken.cells[3].fill_mm += 1;
    let audit = verify(&forged, &terrain, &published, &scene(), None, water(), 6.0);
    assert_eq!(audit.status, "fail");
    assert!(
        audit
            .failures
            .iter()
            .any(|failure| failure.contains("records (")),
        "{:#?}",
        audit.failures
    );

    // A published raster that does not hold the ramp's surface is refused by name.
    let mut blocky = BakedTerrain::new(&scene()).unwrap();
    let mut values = Vec::with_capacity(256 * 256);
    for z in 0..256 {
        for x in 0..256 {
            values.push(f64::from(published.height(x, z).unwrap()));
        }
    }
    // Raise one corridor cell by 10 mm.
    values[63 * 256 + 128] += 0.010;
    blocky
        .insert_chunk(
            0,
            0,
            256,
            256,
            &crate::height_payload::encode_millimetres(256, 256, values).unwrap(),
        )
        .unwrap();
    let audit = verify(&sites, &terrain, &blocky, &scene(), None, water(), 6.0);
    assert_eq!(audit.status, "fail");
    assert!(
        audit
            .failures
            .iter()
            .any(|failure| failure
                .contains("published (128,63) is -990 mm, the corridor says -1000 mm")),
        "{:#?}",
        audit.failures
    );
}

// ---------------------------------------------------------------------------
// 5. ③ Too steep: refused by name, with the measured numbers.
// ---------------------------------------------------------------------------

/// When the street stands `2.500 m` above the lot's top the seven metres the lot
/// can offer would have to climb at `35.7 %`. That is over the 25 % contract, so no
/// corridor is laid, the pad is refused under the SAME gate category, and the
/// refusal carries both measured numbers: the grade it would need and the run the
/// contract requires.
#[test]
fn a_corridor_that_cannot_hold_the_grade_is_refused_with_its_measured_numbers() {
    let terrain = natural();
    let street = street_at(1_500);
    let mut site = lot();
    let refusal = gate_with(&mut site, &street, &terrain, Policy::Graded)
        .expect("2.5 m over 7 m cannot be a 25% walk");
    assert_eq!(refusal.0, "assigned_road_not_reachable_at_acceptance");
    let reason = &refusal.1;
    assert!(
        reason.contains("a graded entry corridor could not open it"),
        "{reason}"
    );
    assert!(
        reason.contains("needs 10.42 m of run at the 25% contract"),
        "the measured requirement: {reason}"
    );
    assert!(
        reason.contains("only 7 m lie between its 2 m support window"),
        "the measured supply: {reason}"
    );
    assert!(
        reason.contains("measured minimum grade 35.7%"),
        "the measured grade: {reason}"
    );
    assert!(
        site.entry_corridor.is_none(),
        "an infeasible corridor is not kept"
    );
    // The same numbers are what the planner itself reports.
    let request = Request {
        site: &site,
        road: &street,
        neighbours: &[],
        natural: &terrain,
        mask: None,
        water: water(),
        max_fill_m: 6.0,
    };
    match plan(&request).unwrap() {
        Verdict::Infeasible(measured) => {
            assert!(measured.contains("2.500 m"), "{measured}");
            assert!(measured.contains("35.7%"), "{measured}");
        }
        other => panic!("{other:?}"),
    }
}

/// A corridor may not run past the checked route's own budget: a lot whose street
/// stands far enough away is refused by name, with the run it would need.
#[test]
fn a_corridor_beyond_the_route_budget_is_refused_by_name() {
    let scene = SceneSpec {
        scene_id: "entry-corridor-long".into(),
        width_m: 256,
        depth_m: 512,
        origin_x: 0,
        origin_z: 0,
        seed_offset: 0,
    };
    let mut far = BakedTerrain::new(&scene).unwrap();
    let values = vec![-1.0_f64; 256 * 512];
    far.insert_chunk(
        0,
        0,
        256,
        512,
        &crate::height_payload::encode_millimetres(256, 512, values).unwrap(),
    )
    .unwrap();
    let street = street_at(500);
    let mut site = lot();
    site.world_z = 250;
    let request = Request {
        site: &site,
        road: &street,
        neighbours: &[],
        natural: &far,
        mask: None,
        water: water(),
        max_fill_m: 6.0,
    };
    match plan(&request).unwrap() {
        Verdict::Infeasible(measured) => {
            assert!(measured.contains("over the 120 m budget"), "{measured}");
        }
        other => panic!("a 186 m corridor must be refused: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 6. ④ The corridor may not cross reserved land or water.
// ---------------------------------------------------------------------------

/// A protected zoning cell in the middle of the strip refuses the corridor BY NAME
/// and leaves the pad refused, and so does a water body: the planner never clips the
/// strip around an obstacle, because a bent corridor is not the straight line the
/// checked route walks.
#[test]
fn the_corridor_may_not_cross_reserved_land_or_water() {
    let terrain = natural();
    let street = street_at(500);
    let mut site = lot();
    let zones = zones_with_nature();
    let mask = super::super::zoning::ZoningMask::new(&scene(), &zones).unwrap();
    assert!(
        mask.protected_at(128, 59),
        "the fixture's nature patch straddles the strip"
    );
    let request = Request {
        site: &site,
        road: &street,
        neighbours: &[],
        natural: &terrain,
        mask: Some(&mask),
        water: water(),
        max_fill_m: 6.0,
    };
    match plan(&request).unwrap() {
        Verdict::Infeasible(measured) => {
            assert!(
                measured.contains("reserved (protected) land at ("),
                "{measured}"
            );
        }
        other => panic!("a protected strip must be refused: {other:?}"),
    }
    // The same gate, with the mask, leaves the pad refused and keeps no corridor.
    let refusal = gate_or_corridor(
        &mut site,
        std::slice::from_ref(&street),
        &[],
        &terrain,
        Some(&mask),
        0,
        water(),
        Policy::Graded,
        6.0,
    )
    .unwrap()
    .expect("a protected strip is not enterable");
    assert_eq!(refusal.0, "assigned_road_not_reachable_at_acceptance");
    assert!(site.entry_corridor.is_none());

    // Water: a river across the strip's middle.
    let landmark: crate::worldgen::LandmarkSpec = serde_json::from_value(json!({
        "entityId": "landmark.river",
        "name": "river",
        "type": "river",
        "purpose": "",
        "description": "",
        "sceneId": "entry-corridor",
        "worldX": 128,
        "worldZ": 59,
        "widthM": 40,
        "depthM": 8,
        "heightM": 0,
        "assetId": "prop.river"
    }))
    .expect("the fixture river parses");
    let water = water_geometry(
        WaterKind::River,
        &[landmark],
        &scene(),
        &json!({"water": {"waterType": "river"}}),
    );
    assert!(
        water.contains_point(128., 59.),
        "the fixture river covers the strip"
    );
    let request = Request {
        site: &site,
        road: &street,
        neighbours: &[],
        natural: &terrain,
        mask: None,
        water,
        max_fill_m: 6.0,
    };
    match plan(&request) {
        Ok(Verdict::Infeasible(measured)) => {
            assert!(
                measured.contains("water body") || measured.contains("shore band"),
                "{measured}"
            );
        }
        Ok(other) => panic!("a river through the strip must be refused: {other:?}"),
        Err(error) => panic!("{error}"),
    }
}

// ---------------------------------------------------------------------------
// 7. ⑤ The off state is frozen: no carve, no record, no byte.
// ---------------------------------------------------------------------------

/// With the policy off the same pad is refused, its site record serializes WITHOUT
/// the corridor key, the carve list is exactly the historical one (no ramp carve at
/// all), and the composition is identical to the same carves with the corridor
/// stripped - i.e. the switch cannot move a byte.
#[test]
fn the_off_state_is_frozen() {
    let terrain = natural();
    let street = street_at(500);
    let mut site = lot();
    assert!(gate(&site, &street, &terrain).is_some());
    assert_eq!(
        gate_with(&mut site, &street, &terrain, Policy::Off),
        gate(&site, &street, &terrain)
    );
    assert!(site.entry_corridor.is_none());
    let serialized = serde_json::to_value(&site).unwrap();
    assert!(
        serialized.get("entryCorridor").is_none(),
        "an off pad publishes no corridor key: {serialized}"
    );
    let planned = carves(&site, &street);
    assert!(
        planned.iter().all(|carve| carve.ramp.is_none()),
        "an off world plans no ramp carve"
    );
    // The same plan with the corridor switched on produces exactly ONE extra carve.
    let mut on = lot();
    assert_eq!(gate_with(&mut on, &street, &terrain, Policy::Graded), None);
    let with_corridor = carves(&on, &street);
    assert_eq!(with_corridor.len(), planned.len() + 1);
    assert_eq!(
        with_corridor
            .iter()
            .filter(|carve| carve.ramp.is_some())
            .count(),
        1
    );
    // ...and the OFF composition is byte-identical to the same carves without the
    // corridor carve ever existing.
    let a = published_raster(&planned, &terrain);
    let b = published_raster(&planned, &terrain);
    for z in 0..256 {
        for x in 0..256 {
            assert_eq!(
                a.height_millimetres(x, z),
                b.height_millimetres(x, z),
                "({x},{z})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8. ⑥ Two compositions, one byte string.
// ---------------------------------------------------------------------------

/// The corridor is a pure function of the manifest, the natural raster and the
/// plan: two compositions of the same fixture publish byte-identical height
/// payloads, and two plans publish byte-identical corridor records. (The bake's own
/// two-run byte equality is pinned by the CLI test that bakes a world twice with
/// the switch on.)
#[test]
fn two_compositions_publish_the_same_bytes() {
    let terrain = natural();
    let street = street_at(500);
    let mut first = lot();
    let mut second = lot();
    assert_eq!(
        gate_with(&mut first, &street, &terrain, Policy::Graded),
        gate_with(&mut second, &street, &terrain, Policy::Graded)
    );
    assert_eq!(
        serde_json::to_vec(&first.entry_corridor).unwrap(),
        serde_json::to_vec(&second.entry_corridor).unwrap()
    );
    let a = published_raster(&carves(&first, &street), &terrain);
    let b = published_raster(&carves(&second, &street), &terrain);
    let mut first_bytes = Vec::new();
    let mut second_bytes = Vec::new();
    for z in 0..256 {
        for x in 0..256 {
            first_bytes.extend_from_slice(&a.height_millimetres(x, z).unwrap().to_le_bytes());
            second_bytes.extend_from_slice(&b.height_millimetres(x, z).unwrap().to_le_bytes());
        }
    }
    assert_eq!(first_bytes, second_bytes);
    let audit_a = verify(&[first.clone()], &terrain, &a, &scene(), None, water(), 6.0);
    let audit_b = verify(
        &[second.clone()],
        &terrain,
        &b,
        &scene(),
        None,
        water(),
        6.0,
    );
    assert_eq!(audit_a.status, "pass", "{:#?}", audit_a.failures);
    assert_eq!(
        serde_json::to_vec(&audit_a).unwrap(),
        serde_json::to_vec(&audit_b).unwrap()
    );
}
