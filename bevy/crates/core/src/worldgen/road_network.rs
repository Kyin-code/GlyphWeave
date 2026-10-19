//! Bounded fill-only, level lowland road network. This deliberately does not
//! claim to model mountain passes, bridges or an optimized highway alignment.
use super::LandUseZone;
use super::zoning::{ZoningMask, ZoningRejection};
use super::{
    EntityInstance, GroundingSpec, SceneSpec, SpatialAnchor, WaterGeometry, WorldManifest,
};
use crate::baked_terrain::{BakedTerrain, TerrainError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNetworkPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage: Option<super::frontage::Report>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_extensions: Option<super::auto_roads::Report>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<super::road_extensions::RoadExtension>,
    pub model: String,
    pub natural_heightfield_hash: String,
    pub scene_id: String,
    pub status: String,
    pub grid_spacing_m: u32,
    pub max_roadbed_fill_m: f32,
    pub level_m: Option<i32>,
    pub candidates: usize,
    pub rejected_water: usize,
    pub rejected_relief: usize,
    pub unselected_candidates: usize,
    pub nodes: Vec<RoadNode>,
    pub edges: Vec<RoadEdge>,
    pub roadside_sites: Vec<RoadsideSite>,
    /// OPT-IN bounded cut: the cap every GRADED road's support band may excavate
    /// under, in whole millimetres, stamped onto each road entity's own
    /// `grounding.boundedCutMm` by [`Self::entities`].
    ///
    /// Absent (and therefore not serialized) for every plan whose style did not
    /// enable `style.frontageProgram.terrainPolicy = "bounded-cut"` with a legal
    /// `maxCutMm`, so no road entity gains a cap and no road carve can lower the
    /// ground. A road without a longitudinal profile never receives it either: a
    /// flat road has no authored surface to cut to and keeps the historical
    /// fill-only contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_cut_mm: Option<i32>,
    #[serde(default)]
    pub zoning_hash: Option<String>,
    #[serde(default)]
    pub zoning_rejections: Vec<ZoningRejection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_outcomes: Vec<super::services::ServiceOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint_layout: Option<super::joint_frontage::JointLayout>,
    /// B01-T2 C2: named, PERSISTED refusals from base-edge profile generation.
    ///
    /// An earlier version collected these into a local Vec that the caller discarded, so "refusals are
    /// retained" was false at the product level: nothing survived into the plan an audit could read. A
    /// refused base edge still keeps `profile: None` and therefore has NO frontage eligibility, but the
    /// reason now travels with the plan.
    ///
    /// This is a diagnostic record, not a profile registry and not a second source of profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_profile_rejections: Vec<BaseProfileRejection>,
    /// B01-T2 R7-1 (rule A'): roadside sites that would have been placed but
    /// cannot be, with a named category (`pad_overlaps_road_band`,
    /// `pad_fill_exceeds_contract`, `entrance_grade_exceeds_contract`). A refused
    /// site is NOT placed and is never silently dropped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roadside_site_rejections: Vec<BaseProfileRejection>,
    /// Opt-in only: the parcels this plan's roadside PADS converted from rural to urban purpose, in
    /// plan order. Empty (and therefore absent) unless
    /// `style.frontageProgram.ruralConversion` is the exact string "explicit", so no existing plan
    /// gains the key. The land-use ledger applies these beside the frontage programme's own
    /// conversions, and `audit_conversions` re-derives every record from the pad's own geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pad_conversions: Vec<super::frontage::Conversion>,
    /// Opt-in only, and only when the cap actually refused something: the urban purpose allocation
    /// the plan's explicit rural-to-urban conversions were spent against, with every refusal and the
    /// area it needed. A plan that never hit the cap publishes nothing here, so every already
    /// generated world keeps its exact bytes.
    ///
    /// See `worldgen::conversion_budget` for the definition: the conversions may not exceed the urban
    /// land the zoning plans minus the urban land the plan has already committed to roads and
    /// parcels. That is what stops a conversion from pushing the urban row of the land-use ledger
    /// past its plan (`ledger purpose overcredited`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversion_allocation: Option<super::conversion_budget::ConversionAllocation>,
    /// Opt-in only, and only when the urban-edge policy actually refused something: the band the
    /// plan's explicit conversions were confined to, with how far the nearest refused candidate
    /// stood from the nearest urban zone. A plan whose conversions all sit on the urban edge
    /// publishes nothing here, so its bytes are unchanged.
    ///
    /// See `worldgen::conversion_fringe`: a conversion may only take rural land whose buildable core
    /// lies within `FRINGE_M` of an existing urban zone, so the urban purpose grows at the city's
    /// edge instead of appearing in whatever field a road crosses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversion_fringe: Option<super::conversion_fringe::ConversionFringe>,
    /// Road-first over structured water: one engineering structure per road
    /// segment that crosses the scene's water body.
    ///
    /// Empty for every input that did not opt in
    /// (`automaticRoadExtensions.allowStructuredWater`), so the field is absent
    /// from every existing plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub water_crossings: Vec<WaterCrossing>,
    /// Opt-in only: candidates that intersect the water body but were refused,
    /// with the reason. Never a silent drop - the historical count-only
    /// `rejected_water` cannot say why a waterfront scene has no crossing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub water_crossing_rejections: Vec<BaseProfileRejection>,
}

/// One named refusal from base-edge profile generation (B01-T2 C2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaseProfileRejection {
    /// the edge that could not be given a legal profile
    pub edge_id: String,
    /// refusal category, stable enough to assert on
    pub category: String,
    /// the specific reason, including any downstream solver message
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNode {
    pub id: usize,
    pub world_x: i32,
    pub world_z: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadEdge {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub world_x: i32,
    pub world_z: i32,
    pub length_m: u32,
    pub rotation_y_deg: f32,
    pub natural_min_m: f32,
    pub natural_max_m: f32,
    /// B01-R1 T2 (OPT-B): the authoritative local profile for a BASE road edge.
    ///
    /// Before this field existed, `RoadNetworkPlan::entities()` could only find a profile by matching an
    /// entry in `plan.extensions` (road_network.rs entities(), which still runs as the second priority).
    /// `plan.extensions` is the registry of roads ADDED by the extension mechanism, so a base
    /// `generated.road-network.*` edge never had a profile, and `frontage::stations()` returns None for a
    /// road without one. That left the majority of legal base roads permanently unable to carry frontage.
    ///
    /// This is NOT a plan-level profile registry: the profile lives on the edge whose geometry it
    /// describes, so the endpoint/length/width/midpoint agreement that `RoadProfile::validate_entity`
    /// demands is structural rather than looked up.
    ///
    /// Serde default keeps every existing plan JSON readable, and an absent profile preserves the legacy
    /// behaviour for inputs that never entered the profile design chain. It is never a stand-in for a
    /// failed design: a base edge either carries a profile the solver produced and validation accepted, or
    /// it carries none and its rejection is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<super::road_profile::RoadProfile>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadsideSite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<super::services::SiteProgram>,
    /// Opt-in only: the purpose this pad DECLARES on the land it takes, derived from its own kind
    /// before it landed (`frontage::pad_purpose`). Absent for every pad of a world that did not
    /// enable the explicit along-road conversion, so no published pad gains the key - and a pad with
    /// no declaration cannot convert anything, which is what the audit re-checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    pub id: String,
    pub road_id: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: u32,
    pub depth_m: u32,
    pub support_level_m: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parcel_insets: Option<super::parcel_geometry::ParcelInsets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_level_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontage_road_point: Option<[i32; 2]>,
    /// OPT-IN bounded cut: the deepest excavation this pad's own support surface
    /// may take from the natural surface, in whole millimetres.
    ///
    /// Only `frontage::append` sets it, only under
    /// `style.frontageProgram.terrainPolicy = "bounded-cut"` with a legal
    /// `maxCutMm`, and only when the pad actually needs a cut. Every other pad of
    /// every input publishes no such key, which is what keeps the default output
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_cut_mm: Option<i32>,
    /// OPT-IN `style.terrainPolicy.earthworkBlend = "derived-from-footprint"`: the
    /// width of the fill-transition ring this pad's OWN footprint derives, in
    /// metres - the same number its realized entity carries in
    /// `grounding.earthworkBlendM`.
    ///
    /// The PLAN RECORD has to carry it, not just the entity. A planned pad is
    /// rebuilt from this record by `settlement::development::plan` (three
    /// independent readers compare that rebuild with the published entity, byte
    /// for byte), and a rebuild that derives entities from the road plan alone
    /// knows no earthwork policy. A ring only one side carried was a plan/entity
    /// split rather than an earthwork decision, so a planned pad used to keep the
    /// historical `3 x (ground + 32)` ring - the 114 m envelope on the hillside
    /// town's 6 m plate. Writing the derived ring into the record is what lets
    /// the plan and the entity share ONE ring.
    ///
    /// Only [`super::earthwork_blend::stamp_plan`] ever sets it, only for a pad
    /// whose kind `plan_terrain_carves` actually carves as a building, and only
    /// under that policy. Every other pad of every input publishes no such key,
    /// which is what keeps the default output byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earthwork_blend_m: Option<f32>,
    pub rotation_y_deg: f32,
    pub natural_min_m: f32,
    pub natural_max_m: f32,
}
impl RoadsideSite {
    /// The building kind this pad realizes: its declared programme kind, or the settlement
    /// programme's own residential default when it carries no programme record.
    pub fn kind(&self) -> &str {
        self.program
            .as_ref()
            .map_or("residential_home", |p| p.kind.as_str())
    }
    /// The vertical size the pad's realized entity carries: its declared
    /// programme height, or the settlement default when it carries no programme.
    ///
    /// One accessor, two readers - [`Self::entity`] and
    /// [`super::earthwork_blend::site_ring_m`] - so the derived ring is computed
    /// from exactly the height the entity publishes.
    pub fn height_m(&self) -> f32 {
        self.program.as_ref().map_or(8., |p| p.height_m as f32)
    }
    pub(crate) fn entity(&self) -> EntityInstance {
        let s = self;
        EntityInstance {
            entity_id: s.id.clone(),
            asset_id: format!("prop.{}", s.kind()),
            kind: s.kind().into(),
            world_x: s.world_x,
            world_z: s.world_z,
            world_y: s
                .support_level_mm
                .map_or(s.support_level_m, |mm| mm.div_euclid(1000)),
            width_m: s.width_m as f32,
            depth_m: s.depth_m as f32,
            height_m: s.height_m(),
            scale: 1.,
            rotation_y_deg: s.rotation_y_deg,
            grounding: GroundingSpec {
                frontage_road_point: s.frontage_road_point,
                bottom_offset_m: s
                    .support_level_mm
                    .map_or(0., |mm| mm.rem_euclid(1000) as f32 / 1000.),
                // The pad record the earthwork ledger is keyed by: a published
                // entity carries the cap its own pad was accepted under, so the
                // audit can recompute the excavation from the world itself.
                bounded_cut_mm: s.bounded_cut_mm,
                // The same one-for-one hand-off for the OPT-IN fill-transition
                // ring: the record carries it, the entity publishes it, and the
                // plan rebuild (`settlement::development::plan`) reads it back out
                // of the record, so "the plan's pad" and "the world's entity"
                // cannot carry two different rings.
                earthwork_blend_m: s.earthwork_blend_m,
                ..Default::default()
            },
            anchors: vec![s.front_anchor()],
            bounds: None,
        }
    }
    pub fn world_half_extents(&self) -> (u32, u32) {
        if matches!(self.rotation_y_deg as i32, 90 | 270) {
            (self.depth_m / 2, self.width_m / 2)
        } else {
            (self.width_m / 2, self.depth_m / 2)
        }
    }
    /// The cut footprint this pad's own published record describes, or `None` when
    /// the pad carries no cap (a fill-only pad publishes no footprint at all).
    ///
    /// This is the unit the placement exclusion, the ledger and the audit all
    /// compare, so an auditor holding only the published plan can re-derive it from
    /// the same fields the generator used.
    pub fn cut_footprint(&self) -> Option<super::bounded_cut::CutFootprint> {
        self.bounded_cut_mm
            .map(|cap| super::bounded_cut::CutFootprint::of(&self.entity(), cap))
    }
    pub(super) fn front_anchor(&self) -> SpatialAnchor {
        // Cardinal frontage is calculated in integer metres. Do not round a
        // large world coordinate through f32 to place an entrance.
        let front = self.depth_m as i32 / 2 + 1;
        let (dx, dz, direction) = match self.rotation_y_deg as i32 {
            90 => (-front, 0, "west"),
            180 => (0, front, "south"),
            270 => (front, 0, "east"),
            _ => (0, -front, "north"),
        };
        SpatialAnchor {
            id: "front".into(),
            world_x: self.world_x + dx,
            world_z: self.world_z + dz,
            direction: direction.into(),
            target: Some("road".into()),
        }
    }
}
impl RoadNetworkPlan {
    pub fn site_entities(&self) -> Vec<EntityInstance> {
        self.roadside_sites
            .iter()
            .map(RoadsideSite::entity)
            .collect()
    }
    pub fn entities(&self) -> Vec<EntityInstance> {
        let Some(level) = self.level_m else {
            return Vec::new();
        };
        self.edges
            .iter()
            .map(|e| {
                // B01-R1 T2 (OPT-B) profile priority, locked by test:
                //   1. the edge's OWN authoritative profile (base roads after T2);
                //   2. the legacy extensions lookup (roads ADDED by the extension mechanism);
                //   3. none, which keeps the legacy flat behaviour for inputs that never entered a
                //      profile design chain. Priority 3 must not become a silent pseudo-profile for a
                //      zoned base road: a base road whose design failed carries no profile and the
                //      rejection is retained by the planning caller.
                let profile = e
                    .profile
                    .clone()
                    .or_else(|| {
                        self.extensions
                            .iter()
                            .find(|p| p.entity_id == e.id)
                            .map(|p| super::road_extensions::profile(&p.design))
                    });
                let road_width_m = profile
                    .as_ref()
                    .map_or(8., |p| p.road_width_m as f32);
                let height = profile
                    .as_ref()
                    .and_then(|p| p.height_at(f64::from(e.world_x), f64::from(e.world_z)))
                    .unwrap_or(f64::from(level));
                // OPT-IN bounded cut, only for a road that carries the graded
                // longitudinal profile its cut must honour. The plan carries ONE
                // cap; a flat road (no profile) has no authored surface to cut to
                // and stays fill-only, so the entity publishes no cap and
                // `plan_terrain_carves` grants it no carve cap either.
                let bounded_cut_mm = profile.as_ref().and(self.road_cut_mm);
                EntityInstance {
                    entity_id: e.id.clone(),
                    asset_id: "prop.road".into(),
                    kind: "road".into(),
                    world_x: e.world_x,
                    world_z: e.world_z,
                    world_y: height.floor() as i32,
                    width_m: e.length_m as f32,
                    depth_m: road_width_m,
                    height_m: 0.24,
                    rotation_y_deg: e.rotation_y_deg,
                    scale: 1.,
                    grounding: GroundingSpec {
                        road_profile: profile,
                        bottom_offset_m: (height - height.floor()) as f32,
                        // Opt-in: a registered water crossing is a STRUCTURE deck.
                        // `plan_terrain_carves` refuses to carve it, so the water
                        // bed under the span and the ground under the span are
                        // exactly the natural raster. False everywhere else, and
                        // not serialized when false, so existing entities are
                        // byte-identical.
                        structure_deck: self
                            .water_crossings
                            .iter()
                            .any(|crossing| crossing.road_id == e.id),
                        // OPT-IN bounded cut, only for a road that carries the
                        // graded longitudinal profile its cut must honour. The
                        // plan carries ONE cap; a flat road (no profile) has no
                        // authored surface to cut to and stays fill-only, so the
                        // entity publishes no cap and `plan_terrain_carves` grants
                        // it no carve cap either.
                        bounded_cut_mm,
                        ..Default::default()
                    },
                    anchors: Vec::new(),
                    bounds: None,
                }
            })
            .collect()
    }
}

/// The only accepted value of `style.automaticRoadExtensions.allowStructuredWater`.
///
/// The value is a decision, not a flag: it says every road metre over the
/// scene's structured water body is carried by a registered engineering
/// structure (`bridge`/`causeway`) and that no earthwork is placed in the
/// water. Absent or any other value keeps road-first disabled for a water
/// scene, which is the historical behaviour (`enabled`).
pub const STRUCTURED_WATER_OPT_IN: &str = "bridge-or-causeway";

/// Does this style authorize road-first planning across structured water?
///
/// Pure string test against [`STRUCTURED_WATER_OPT_IN`]; it never inspects the
/// water geometry, so the caller can ask before a scene is even baked. The
/// consuming bake additionally stamps the same predicate onto `WaterGeometry`
/// (`road_water_crossings`), which is what keys the no-earthwork-in-water rule
/// for the raster.
pub fn allows_structured_water(style: &serde_json::Value) -> bool {
    style
        .get("automaticRoadExtensions")
        .and_then(|program| program.get("allowStructuredWater"))
        .and_then(serde_json::Value::as_str)
        == Some(STRUCTURED_WATER_OPT_IN)
}

pub fn enabled(style: &serde_json::Value, water: WaterGeometry) -> bool {
    matches!(
        style
            .get("generationProfile")
            .and_then(serde_json::Value::as_str),
        Some("strict" | "release")
    ) && (water.kind == super::WaterKind::None || allows_structured_water(style))
        && style.get("landUseProfile").is_some()
        && style
            .get("naturalOnly")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        && !matches!(
            super::generation_settlement(style),
            Some("wilderness" | "pastoral")
        )
}

#[cfg(test)]
pub(super) fn plan(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
) -> Result<RoadNetworkPlan, TerrainError> {
    plan_with_zones(scene, terrain, water, None)
}
/// B01-T2 R11 G3: where does a roadside pad stand on its edge?
///
/// Returns `(pad x, pad z, own level at the pad's frontage point (millimetres;
/// `None` on the legacy unprofiled path), frontage point)`.
///
/// A profiled edge is scanned at every legal station of the frontage path's own
/// station rule, and the pad takes the station with the SMALLEST rise between
/// the road's required surface over the pad's window (rule A', the production
/// carve composition) and the road's own surface at that station. The rise is
/// what the entrance apron has to carry between the pad's flat top and the
/// graded band edge, and the apron is one raster cell wide, so a pad raised by
/// more than the walkable grade over one cell cannot be entered from its own
/// road: its checked route detours to another road and the development audit
/// reports `no_verified_assigned_road_access`. Fixing the pad at the edge's
/// centre was the defect - the centre is usually the deepest point of the
/// profile's local dip, which is exactly where that rise is largest.
///
/// Stations that fail the walkability test are not used; a pad that can be
/// placed at the centre by the legacy rule (no profile) keeps the old
/// placement byte-identically, and an edge whose stations all fail keeps the
/// centre too (the existing gates then refuse it by name, as before).
/// Distance from a world point to a (possibly quarter-turned) entity's
/// footprint rectangle, in metres. Used by the station scan to keep a roadside
/// pad on the road it was assigned.
pub(super) fn road_rect_distance(entity: &EntityInstance, x: i32, z: i32) -> f64 {
    let (hw, hd) = super::spatial_half_extents(
        entity.width_m,
        entity.depth_m,
        entity.rotation_y_deg,
    );
    let dx = (f64::from(x) - f64::from(entity.world_x)).abs() - hw;
    let dz = (f64::from(z) - f64::from(entity.world_z)).abs() - hd;
    dx.max(0.).hypot(dz.max(0.))
}

/// `(pad x, pad z, own level at the pad's frontage point (millimetres; `None` on
/// the legacy unprofiled path or the centre fallback), frontage point, whether
/// the caller must keep the legacy 2/2/2 parcel ring)`.
type RoadsidePlacement = (i32, i32, Option<i64>, [i32; 2], bool);

/// The half extents of a roadside pad's published AUDIT window, in world axes:
/// the pad plus the 2 m apron its carve, its flat-top audit and its descriptor
/// clearance all work on. Derived from the same function the planner and the
/// audit use, so the three cannot disagree about what ground a pad owns.
pub(super) fn pad_audit_half_extents(
    width_m: f32,
    depth_m: f32,
    rotation_y_deg: f32,
) -> (f64, f64) {
    super::support_pad_half_extents(width_m, depth_m, rotation_y_deg, true)
}

/// The elevation of a road's OWN graded surface at a world point, millimetres.
///
/// A profiled road publishes a `RoadProfile` and the surface is read from it at
/// the footprint's own along-axis station - the same `height_at` the frontage
/// path, the carve and the audit use. An unprofiled road is flat on its own
/// published `world_y` / `bottom_offset_m`, which is that road's surface and
/// nothing else.
///
/// This exists because a waterfront pad may no longer be declared on the
/// network's single `level_m`: on a shore the bank, the quay and the crossing
/// deck sit at different heights, so the pad has to stand on the surface of the
/// road it fronts, at the station it fronts it.
fn road_surface_mm_at(road: &EntityInstance, x: f64, z: f64) -> Option<i64> {
    match &road.grounding.road_profile {
        Some(profile) => profile.height_at(x, z).map(|h| (h * 1000.).round() as i64),
        None => Some((f64::from(road.ground_height_m()) * 1000.).round() as i64),
    }
}

#[allow(clippy::too_many_arguments)]
fn roadside_placement_candidates(
    e: &RoadEdge,
    side: i32,
    road_entities: &[EntityInstance],
    request: &super::frontage::Request,
    terrain: &BakedTerrain,
    road_carves: &[super::TerrainCarve],
    pad_win: i32,
    scene: &SceneSpec,
    waterfront: bool,
) -> Vec<RoadsidePlacement> {
    let centre = if e.rotation_y_deg == 0. {
        (
            i64::from(e.world_x),
            i64::from(e.world_z) + 24 * i64::from(side),
        )
    } else {
        (
            i64::from(e.world_x) + 24 * i64::from(side),
            i64::from(e.world_z),
        )
    };
    let legacy = (
        centre.0 as i32,
        centre.1 as i32,
        None,
        [e.world_x, e.world_z],
        true,
    );
    // Opt-in waterfront: a pad may only be declared on a level derived from its
    // OWN road, so the centre placement carries that road's surface elevation (and
    // its frontage point) instead of the legacy flat network level. The legacy
    // 2 m parcel ring is kept: the pad clears the shore band by the clearance
    // constant, so the ring (2 m wider) cannot reach it either.
    let centre_surface_mm = |fallback: Option<i64>| -> Option<i64> {
        if !waterfront {
            return fallback;
        }
        road_entities
            .iter()
            .find(|entity| entity.entity_id == e.id)
            .and_then(|road| {
                road_surface_mm_at(road, f64::from(e.world_x), f64::from(e.world_z))
            })
            .or(fallback)
    };
    let waterfront_centre = centre_surface_mm(None).map(|mm| {
        (
            centre.0 as i32,
            centre.1 as i32,
            Some(mm),
            [e.world_x, e.world_z],
            true,
        )
    });
    let Some(profile) = e.profile.as_ref() else {
        return vec![waterfront_centre.unwrap_or(legacy)];
    };
    // A profiled edge the station scan cannot serve (short edge, or a station list
    // the frontage rule refuses) keeps the pre-R11 centre placement and its flat
    // level, but NOT the legacy parcel ring: the zero-inset parcel is a property
    // of the road being profiled (D5's scope is the unprofiled neighbour), and
    // handing it back here re-opened the G1 yard shortfall in the frozen W1
    // fixture (51.0 -> 4419.0 m2) when measured.
    let profiled_centre = (
        centre.0 as i32,
        centre.1 as i32,
        centre_surface_mm(None),
        [e.world_x, e.world_z],
        false,
    );
    let Some(road_entity) = road_entities.iter().find(|entity| entity.entity_id == e.id) else {
        return vec![profiled_centre];
    };
    let Ok(Some(stations)) = super::frontage::stations(road_entity, request) else {
        return vec![profiled_centre];
    };
    let dx = (profile.end[0] - profile.start[0]).signum();
    let dz = (profile.end[1] - profile.start[1]).signum();
    // `(rise, is_centre, placement)`. The station scan only relocates a pad when
    // the legacy centre placement is itself unwalkable: a road whose graded
    // surface already sits at the pad's own level (rise 0, e.g. a base edge pinned
    // flat at the network level) keeps the legacy layout, count and identity - and
    // a road that dips across the pad's window hands the pad to the station with
    // the smallest rise, because there the centre is the worst candidate.
    let mut ranked: Vec<(f64, bool, i32, i32, Option<i64>, [i32; 2], bool)> = Vec::new();
    {
        let (cx, cz) = (centre.0 as i32, centre.1 as i32);
        let mut required_max = profile
            .height_at(f64::from(e.world_x), f64::from(e.world_z))
            .map(|h| (h * 1000.).round() as i64)
            .unwrap_or(i64::from(0));
        for zz in cz - pad_win..=cz + pad_win {
            for xx in cx - pad_win..=cx + pad_win {
                if let Some(h) = terrain.height(xx, zz) {
                    let required = super::apply_terrain_carves(
                        f64::from(h),
                        xx,
                        zz,
                        road_carves,
                    );
                    required_max = required_max.max((required * 1000.).round() as i64);
                }
            }
        }
        let own_centre = profile
            .height_at(f64::from(e.world_x), f64::from(e.world_z))
            .map(|h| (h * 1000.).round() as i64)
            .unwrap_or(required_max);
        let rise_centre = (required_max - own_centre) as f64 / 1000.;
        if rise_centre <= super::access::WALKABLE_GRADE {
            // The legacy centre placement is walkable as it stands: keep it, and
            // with it the legacy layout, count and identity of this edge's parcel.
            return vec![profiled_centre];
        }
        if rise_centre <= super::access::WALKABLE_GRADE * super::ENTRANCE_APRON_RUN_M {
            // Walkable only across the engineered entrance apron, which is exactly
            // what a frontage point builds: the pad keeps the centre position (so
            // the legacy layout, count and identity survive) but takes the graded
            // datum at that centre, and the corridor carve carries the remaining
            // rise over its own run.
            return vec![(
                centre.0 as i32,
                centre.1 as i32,
                Some(own_centre),
                [e.world_x, e.world_z],
                false,
            )];
        }
        ranked.push((
            rise_centre,
            true,
            cx,
            cz,
            centre_surface_mm(None),
            [e.world_x, e.world_z],
            false,
        ));
    }
    for &station in &stations {
        let rx = profile.start[0] + dx * station as i32;
        let rz = profile.start[1] + dz * station as i32;
        let (x, z) = if dx != 0 {
            (rx, rz + 24 * side)
        } else {
            (rx + 24 * side, rz)
        };
        if i64::from(x) - 14 < i64::from(scene.origin_x)
            || i64::from(z) - 14 < i64::from(scene.origin_z)
            || i64::from(x) + 14 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
            || i64::from(z) + 14 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
        {
            continue;
        }
        let own_mm = profile.elevations_mm[station];
        let mut required_max = own_mm;
        for zz in z - pad_win..=z + pad_win {
            for xx in x - pad_win..=x + pad_win {
                if let Some(h) = terrain.height(xx, zz) {
                    let required =
                        super::apply_terrain_carves(f64::from(h), xx, zz, road_carves);
                    required_max = required_max.max((required * 1000.).round() as i64);
                }
            }
        }
        let rise_m = (required_max - own_mm) as f64 / 1000.;
        // The apron is `ENTRANCE_APRON_RUN_M` long, so a rise is walkable while it
        // stays inside the audited grade over that run - not over a single cell.
        if rise_m > super::access::WALKABLE_GRADE {
            continue;
        }
        // The development audit requires the pad's checked route to reach the
        // road it was ASSIGNED, so a station is only usable while that road is
        // also the one the pad is nearest to: beside a crossing road the walk
        // reaches the crossing one first, and the building fails
        // `no_verified_assigned_road_access` while being perfectly walkable
        // (measured in the first station-scan world: 33 of 33 links walkable,
        // 14 still failing on the assignment clause).
        let own_distance = road_rect_distance(road_entity, x, z);
        let nearest_other = road_entities
            .iter()
            .filter(|other| other.entity_id != road_entity.entity_id)
            .map(|other| road_rect_distance(other, x, z))
            .fold(f64::INFINITY, f64::min);
        if own_distance > nearest_other {
            continue;
        }
        ranked.push((rise_m, false, x, z, Some(own_mm), [rx, rz], false));
    }
    if ranked.iter().all(|(_, is_centre, ..)| *is_centre) {
        return vec![profiled_centre];
    }
    // Ascending rise, and the centre placement wins every tie.
    ranked.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
    });
    let mut out: Vec<RoadsidePlacement> = ranked
        .into_iter()
        .map(|(_, _, x, z, own_mm, frontage, legacy_parcel)| {
            (x, z, own_mm, frontage, legacy_parcel)
        })
        .collect();
    out
}

/// One road segment that is carried over a structured water body by an
/// engineering structure instead of by earthwork.
///
/// This record is the *only* authorization for a road to sit on water, and it
/// is audited against the water body and the natural/final rasters rather than
/// trusted: [`measure_water_crossing`] recomputes every number here from the
/// road's own published footprint, and [`audit_water_crossings`] refuses any
/// disagreement by name.
///
/// A crossing road performs NO earthwork: its deck is a structure, so the water
/// bed, the banks and the water surface are exactly what the natural raster
/// already holds. `causeway` is accepted as a type because the authorization is
/// about the structure class, not about how it is built; both classes are held
/// to the same invariant - nothing is filled, and the deck clears the water.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaterCrossing {
    /// the road entity that carries the span
    pub road_id: String,
    /// `"bridge"` or `"causeway"`; nothing else is a legal structure here
    pub kind: String,
    /// the road's pavement width over the span, in metres
    pub width_m: u32,
    /// measured length of the span over water: the number of 1 m stations along
    /// the road axis whose pavement band holds at least one water cell
    pub length_m: u32,
    /// footprint cells of the pavement band that lie inside the water body
    pub wet_cells: u32,
    /// bounding box of those cells, `[min_x, min_z, max_x, max_z]`
    pub span: [i32; 4],
    /// canonical identity of the crossed water body (`water.<scene>.<kind>`)
    pub water_id: String,
    /// `"lake"` or `"river"` - the crossed body's class, restated so a record
    /// cannot outlive the water it names
    pub water_kind: String,
    /// the water datum the deck has to clear, millimetres above the scene datum
    pub water_level_mm: i32,
    /// the deck surface, millimetres (the road's own design level)
    pub deck_level_mm: i32,
    /// natural bed range under the span, millimetres
    pub bed_min_mm: i32,
    pub bed_max_mm: i32,
    /// highest natural ground on the DRY part of the roadbed. The deck is
    /// required to sit at or above it (no excavation) and to reach it from
    /// above by no more than the roadbed fill contract (`maxRoadbedFillM`).
    pub take_off_max_mm: i32,
}

/// Independently recomputable measurement of one road's contact with water.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaterCrossingMeasurement {
    pub wet_cells: u32,
    pub stations_m: u32,
    pub span: [i32; 4],
    pub bed_min_mm: i32,
    pub bed_max_mm: i32,
}

/// Measure the pavement band of `road` against `water`, from the published road
/// geometry alone. `None` when the band never touches the water body.
///
/// The band is the road's graded TRANSPORT footprint
/// (`graded_transport_half_extents`): the same rectangle the earthwork contract
/// is audited on and the same one the planner scans, so the planned record, the
/// audit's re-measurement and the fill/no-fill clauses cannot disagree about
/// which cells belong to the road.
///
/// One function produces both the planned record and the audit's re-measurement,
/// so the two cannot drift: a record is a *claim* about this function's output,
/// and a forged claim is a disagreement with it.
pub fn measure_water_crossing(
    road: &EntityInstance,
    water: WaterGeometry,
    natural: &BakedTerrain,
) -> Result<Option<WaterCrossingMeasurement>, TerrainError> {
    if water.kind == super::WaterKind::None {
        return Ok(None);
    }
    let (hw, hd) = super::graded_transport_half_extents(road);
    let along_x = road.rotation_y_deg != 90.;
    let mut wet_cells = 0_u32;
    let mut stations = 0_u32;
    let mut span = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    let mut bed_min_mm = i32::MAX;
    let mut bed_max_mm = i32::MIN;
    let (lo_along, hi_along) = if along_x {
        (road.world_x - hw, road.world_x + hw)
    } else {
        (road.world_z - hd, road.world_z + hd)
    };
    let transverse = if along_x { hd } else { hw };
    for station in lo_along..=hi_along {
        let mut station_wet = false;
        for offset in -transverse..=transverse {
            let (x, z) = if along_x {
                (station, road.world_z + offset)
            } else {
                (road.world_x + offset, station)
            };
            if !water.contains_point(f64::from(x), f64::from(z)) {
                continue;
            }
            station_wet = true;
            wet_cells += 1;
            span[0] = span[0].min(x);
            span[1] = span[1].min(z);
            span[2] = span[2].max(x);
            span[3] = span[3].max(z);
            // The bed is the NATURAL surface under the deck: the record states
            // it so a span cannot be claimed over ground that was never wet.
            let h = natural
                .height(x, z)
                .ok_or_else(|| TerrainError("water crossing missing natural bed sample".into()))?;
            let mm = (h * 1000.).round() as i32;
            bed_min_mm = bed_min_mm.min(mm);
            bed_max_mm = bed_max_mm.max(mm);
        }
        if station_wet {
            stations += 1;
        }
    }
    if wet_cells == 0 {
        return Ok(None);
    }
    Ok(Some(WaterCrossingMeasurement {
        wet_cells,
        stations_m: stations,
        span,
        bed_min_mm,
        bed_max_mm,
    }))
}

/// The water surface datum, in millimetres above the scene datum.
///
/// `terrain_height_metres` is the authority: every lake/river bed cell is
/// clamped into `-7.0 ..= -1.0` m and every bank cell is lifted to at least
/// `1.5` m, so the surface a deck has to clear is the scene datum itself.
/// `style.water.levelM` is a *reporting* field - the bake never reads it - so it
/// cannot move this number without moving the baked bed first.
pub const WATER_DATUM_MM: i32 = 0;

/// The structure classes a water crossing may use. Anything else is refused.
pub const WATER_CROSSING_KINDS: [&str; 2] = ["bridge", "causeway"];

pub fn water_kind_name(kind: super::WaterKind) -> &'static str {
    match kind {
        super::WaterKind::None => "none",
        super::WaterKind::Lake => "lake",
        super::WaterKind::River => "river",
    }
}

/// Canonical identity of a scene's structured water body, restated in every
/// crossing record so a record cannot outlive the water it names.
pub fn water_body_id(scene_id: &str, water: WaterGeometry) -> String {
    format!("water.{scene_id}.{}", water_kind_name(water.kind))
}

/// One candidate segment that touches the water body, carried from the
/// candidate scan into the level search and then into the published record.
#[derive(Clone, Copy)]
pub(super) struct CrossingDraft {
    /// highest natural ground on the DRY part of the band
    dry_max_m: f32,
}

pub(super) fn plan_with_zones(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
) -> Result<RoadNetworkPlan, TerrainError> {
    plan_with_zones_demand(
        scene,
        terrain,
        water,
        zones,
        false,
        super::frontage::RuralConversion::Disabled,
        None,
        // A bare zoned plan never opts into the bounded cut: no style reaches it.
        0,
    )
}

/// B01-T2 R13: `settlement_demand` says whether this scene actually asks for
/// settlement content (a `settlementProgram`, hence real pads that must be
/// enterable). A demand-free plan keeps every geometrically legal grading: its
/// roadside scan is speculative and its refusals would only remove evidence.
/// A demanding scene refuses grading that cannot host its own pads - see the
/// R13 pre-pass below for the measurements and the exact rule.
///
/// `pad_conversion` is the authored explicit along-road conversion policy the whole scene is under
/// (`style.frontageProgram.ruralConversion`). It only ever narrows or re-books a pad: with the
/// policy off every pad decision below is byte-identical to the historical programme.
pub(super) fn plan_with_zones_demand(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    settlement_demand: bool,
    pad_conversion: super::frontage::RuralConversion,
    // The street setback the shoreline commercial row is planned at, or None when this world
    // states no street morphology to plan one from (rontage::shorefront_row_contract).
    shorefront_setback_m: Option<u32>,
    // The bounded-cut allowance of the longitudinal road profile, in whole millimetres: `0` for a
    // world that never opted in, which keeps the fill-only solver byte for byte.
    profile_cut_mm: i32,
) -> Result<RoadNetworkPlan, TerrainError> {
    let zoning = zones.map(|z| ZoningMask::new(scene, z)).transpose()?;
    terrain.require_complete()?;
    if !terrain.matches_scene(scene) {
        return Err(TerrainError("road plan terrain mismatch".into()));
    }
    let step = 64;
    let max_fill = 6.;
    let mut result = RoadNetworkPlan {
        road_cut_mm: None,
        automatic_extensions: None,
        frontage: None,
        // B01-T2 C2: filled once base-edge profiles are attempted and their refusals are known
        base_profile_rejections: Vec::new(),
        // B01-T2 R7-1: filled once roadside sites are attempted and their refusals are known
        roadside_site_rejections: Vec::new(),
        extensions: vec![],
        model: if zoning.is_some() {
            "zoned-fill-envelope-lowland-frontage-v4"
        } else {
            "connected-level-lowland-fill-frontage-v2"
        }
        .into(),
        natural_heightfield_hash: terrain.content_hash()?,
        scene_id: scene.scene_id.clone(),
        status: "no_feasible_network".into(),
        grid_spacing_m: step,
        max_roadbed_fill_m: max_fill,
        level_m: None,
        candidates: 0,
        rejected_water: 0,
        rejected_relief: 0,
        unselected_candidates: 0,
        nodes: Vec::new(),
        edges: Vec::new(),
        roadside_sites: Vec::new(),
        pad_conversions: Vec::new(),
        conversion_allocation: None,
        conversion_fringe: None,
        zoning_hash: zoning.as_ref().map(|z| z.hash.clone()),
        zoning_rejections: Vec::new(),
        service_outcomes: Vec::new(),
        joint_layout: None,
        water_crossings: Vec::new(),
        water_crossing_rejections: Vec::new(),
    };
    let xs: Vec<i32> = (32..scene.width_m.saturating_sub(32))
        .step_by(step as usize)
        .map(|x| scene.origin_x + x as i32)
        .collect();
    let zs: Vec<i32> = (32..scene.depth_m.saturating_sub(32))
        .step_by(step as usize)
        .map(|z| scene.origin_z + z as i32)
        .collect();
    let mut all_nodes = Vec::new();
    for &z in &zs {
        for &x in &xs {
            all_nodes.push(RoadNode {
                id: all_nodes.len(),
                world_x: x,
                world_z: z,
            });
        }
    }
    let mut edges = Vec::new();
    let mut level_caps = Vec::new();
    let mut crossing_drafts: BTreeMap<String, CrossingDraft> = BTreeMap::new();
    for row in 0..zs.len() {
        for col in 0..xs.len() {
            let from = row * xs.len() + col;
            for (exists, to, rot) in [
                (col + 1 < xs.len(), from + 1, 0.),
                (row + 1 < zs.len(), from + xs.len(), 90.),
            ] {
                if !exists {
                    continue;
                }
                result.candidates += 1;
                let a = &all_nodes[from];
                let b = &all_nodes[to];
                let x = ((i64::from(a.world_x) + i64::from(b.world_x)) / 2) as i32;
                let z = ((i64::from(a.world_z) + i64::from(b.world_z)) / 2) as i32;
                let (hw, hd) = if rot == 0. {
                    (step as i32 / 2 + 4, 7)
                } else {
                    (7, step as i32 / 2 + 4)
                };
                if let Some(reject) = zoning.as_ref().and_then(|mask| {
                    mask.check(
                        format!("generated.road-network.{}.{}-{}", scene.scene_id, from, to),
                        "road",
                        x,
                        z,
                        hw,
                        hd,
                    )
                }) {
                    result.zoning_rejections.push(reject);
                    continue;
                }
                let water_contact = water.intersects_axis_aligned(
                    f64::from(x),
                    f64::from(z),
                    f64::from(hw),
                    f64::from(hd),
                );
                if water_contact && !water.road_water_crossings() {
                    result.rejected_water += 1;
                    continue;
                }
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                let mut protected_cap = f64::INFINITY;
                let mut wet_cells = 0_u32;
                // Full authoritative raster, not nine probes that miss ridges.
                for zz in z - hd..=z + hd {
                    for xx in x - hw..=x + hw {
                        let h = terrain.height(xx, zz).ok_or_else(|| {
                            TerrainError("road footprint missing natural sample".into())
                        })?;
                        if water_contact
                            && water.contains_point(f64::from(xx), f64::from(zz))
                        {
                            // Opt-in structure deck: the span over water carries
                            // no earthwork, so it is excluded from the roadbed
                            // fill range and can never raise the bed.
                            wet_cells += 1;
                            continue;
                        }
                        min = min.min(h);
                        max = max.max(h);
                        if let Some(mask) = &zoning {
                            protected_cap =
                                protected_cap.min(f64::from(h) + mask.fill_capacity_at(xx, zz));
                        }
                    }
                }
                let crossing = wet_cells > 0;
                if crossing && !min.is_finite() {
                    // A candidate whose whole band is water has no dry support at
                    // either end: gripping it would need piers standing in the
                    // water, which the water-surface invariant forbids. Refused
                    // BY NAME so a waterfront scene can say why it has no bridge.
                    result.water_crossing_rejections.push(BaseProfileRejection {
                        edge_id: format!(
                            "generated.road-network.{}.{}-{}",
                            scene.scene_id, from, to
                        ),
                        category: "crossing_spans_only_water".into(),
                        reason: format!(
                            "candidate at ({x},{z}) is {}x{} m and its entire graded transport band is water; a structure with no dry take-off at either end would need piers in the water body",
                            hw * 2 + 1,
                            hd * 2 + 1
                        ),
                    });
                    result.rejected_water += 1;
                    continue;
                }
                if !crossing && (max.ceil() - min > max_fill || f64::from(max.ceil()) > protected_cap)
                {
                    result.rejected_relief += 1;
                    continue;
                }
                if crossing {
                    // A deck is not an embankment: the roadbed FILL contract does
                    // not apply to it, and the 6 m number is not relaxed - it is
                    // re-applied to the only earthwork contact the span has, the
                    // dry take-off (`level - dry_max <= max_roadbed_fill_m`, see
                    // the level search below). Filling the water instead is
                    // forbidden outright.
                    crossing_drafts.insert(
                        format!("generated.road-network.{}.{}-{}", scene.scene_id, from, to),
                        CrossingDraft { dry_max_m: max },
                    );
                }
                level_caps.push(protected_cap);
                edges.push(RoadEdge {
                    id: format!("generated.road-network.{}.{}-{}", scene.scene_id, from, to),
                    from,
                    to,
                    world_x: x,
                    world_z: z,
                    length_m: step,
                    rotation_y_deg: rot,
                    natural_min_m: min,
                    natural_max_m: max,
                    // B01-R1 T2 (OPT-B): attached below by attach_base_edge_profiles once the network
                    // level is chosen. Left None here so a candidate that fails design never carries a
                    // stand-in profile.
                    profile: None,
                });
            }
        }
    }
    let levels: BTreeSet<i32> = edges
        .iter()
        .map(|e| e.natural_max_m.ceil() as i32)
        .collect();
    let mut best: Vec<usize> = Vec::new();
    let mut best_distance = f64::INFINITY;
    for level in levels {
        let mut adjacency = vec![Vec::new(); all_nodes.len()];
        for (i, e) in edges.iter().enumerate() {
            // A structure deck has no embankment to build, so the roadbed-fill
            // bound is applied to its only earthwork contact instead: the deck
            // may sit at or above the highest dry cell (never excavating) and no
            // more than `max_fill` above it. Its own span carries no fill at all.
            let fill_ok = if let Some(draft) = crossing_drafts.get(&e.id) {
                level as f32 >= e.natural_max_m
                    && level as f32 - draft.dry_max_m <= max_fill
                    // The deck has to clear the water surface, or the "road" is a
                    // submerged slab and the water is displaced by the roadbed.
                    && level as f32 > WATER_DATUM_MM as f32 / 1000.
            } else {
                level as f32 >= e.natural_max_m && level as f32 - e.natural_min_m <= max_fill
            };
            if fill_ok && f64::from(level) <= level_caps[i] {
                adjacency[e.from].push((e.to, i));
                adjacency[e.to].push((e.from, i));
            }
        }
        let mut seen = vec![false; all_nodes.len()];
        for start in 0..all_nodes.len() {
            if seen[start] || adjacency[start].is_empty() {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            seen[start] = true;
            let mut component = BTreeSet::new();
            let mut distance = f64::INFINITY;
            while let Some(n) = queue.pop_front() {
                let node = &all_nodes[n];
                distance = distance.min(
                    (f64::from(node.world_x)
                        - f64::from(scene.origin_x)
                        - f64::from(scene.width_m) * 0.5)
                        .powi(2)
                        + (f64::from(node.world_z)
                            - f64::from(scene.origin_z)
                            - f64::from(scene.depth_m) * 0.5)
                            .powi(2),
                );
                for &(neighbor, edge) in &adjacency[n] {
                    component.insert(edge);
                    if !seen[neighbor] {
                        seen[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
            if component.len() > best.len()
                || (component.len() == best.len() && distance < best_distance)
            {
                best = component.into_iter().collect();
                best_distance = distance;
                result.level_m = Some(level);
            }
        }
    }
    result.unselected_candidates = edges.len() - best.len();
    let mut used = BTreeMap::new();
    for i in best {
        let e = edges[i].clone();
        used.insert(e.from, all_nodes[e.from].clone());
        used.insert(e.to, all_nodes[e.to].clone());
        result.edges.push(e);
    }
    result.nodes = used.into_values().collect();
    // Opt-in water crossings: one engineering structure per SELECTED segment that
    // touches the water body. The record is measured from the very pavement
    // geometry the product publishes (`RoadNetworkPlan::entities`), so the audit
    // can recompute it rather than trust it.
    if let Some(level) = result.level_m {
        let mut records = Vec::new();
        for entity in result.entities() {
            let Some(draft) = crossing_drafts.get(&entity.entity_id) else {
                continue;
            };
            let Some(measured) = measure_water_crossing(&entity, water, terrain)? else {
                continue;
            };
            records.push(WaterCrossing {
                road_id: entity.entity_id.clone(),
                // A span that leaves the water bed untouched is a bridge; a
                // causeway record is accepted by the audit only under the same
                // no-displacement invariant, which this planner never has to
                // project because it only builds free spans.
                kind: "bridge".into(),
                width_m: entity.depth_m.round() as u32,
                length_m: measured.stations_m,
                wet_cells: measured.wet_cells,
                span: measured.span,
                water_id: water_body_id(&scene.scene_id, water),
                water_kind: water_kind_name(water.kind).into(),
                water_level_mm: WATER_DATUM_MM,
                deck_level_mm: level * 1000,
                bed_min_mm: measured.bed_min_mm,
                bed_max_mm: measured.bed_max_mm,
                take_off_max_mm: (draft.dry_max_m * 1000.).round() as i32,
            });
        }
        result.water_crossings = records;
    }
    // B01-R1 T2 (OPT-B): now that the base edges and the chosen level are known, give each base edge its
    // own authoritative profile through the REAL alignment solver. Edges whose design or validation does
    // not hold keep `profile: None` and their named rejection is returned to the caller, so a base road
    // can never acquire a pseudo-profile it did not earn.
    if let Some(level) = result.level_m {
        // take the nodes out of the plan for the duration of the borrow, because the helper needs the
        // node table while mutating the edges. The nodes are put back unconditionally afterwards, and a
        // helper error propagates only AFTER the plan is whole again, so a failure cannot leave a
        // half-updated public plan with its nodes missing.
        let nodes = std::mem::take(&mut result.nodes);
        let attached = attach_base_edge_profiles(
            &mut result,
            scene,
            terrain,
            water,
            &nodes,
            level,
            zones,
            profile_cut_mm,
        );
        result.nodes = nodes;
        // B01-T2 C2: PERSIST the refusals instead of discarding them. A refused edge keeps
        // profile: None and therefore has no frontage eligibility; the reason now travels with the plan.
        result.base_profile_rejections = attached?;
    }
    if !result.edges.is_empty() {
        result.status = "planned_connected_network".into();
    }
    // The shoreline commercial row, planned BEFORE the roadside pad programme below scans for
    // its own stations. The order IS the urban-design claim: the quay street's frontage belongs
    // to the commerce. Measured on the registered river-delta sample (Round 24), planning this
    // row AFTER the pads left 2 of 28 urban stations free, because the dwellings had already
    // taken the whole block behind the quay street in 24 m lots and `joint_frontage` keeps a
    // parcel 2 m clear of its neighbour. Planned here, the pad scan reserves around it instead.
    //
    // `shorefront_setback_m` is `Some` only when the manifest states the street morphology the
    // row is planned from (`frontage::shorefront_row_contract`), so a scene that states none
    // keeps the plan - and the legacy bayfront band - it always published.
    if shorefront_setback_m.is_some() {
        super::shorefront::plan(&mut result, scene, terrain, water, zones, pad_conversion)?;
    }

    if let Some(level) = result.level_m {
        // B01-T2 R11 G3: the station scan uses the frontage path's own request
        // shape for a 24x24 roadside pad (setback 8 puts its centre the same
        // 24 m off the axis the roadside rule has always used).
        let roadside_request = super::frontage::Request {
            id: "roadside".into(),
            scene_id: scene.scene_id.clone(),
            purpose: "residential".into(),
            kind: "residential_home".into(),
            count: 1,
            width_m: 24,
            depth_m: 24,
            height_m: 12,
            station_spacing_m: 8,
            setback_m: 8,
            max_candidate_attempts: 8,
            parcel_insets: Some(super::parcel_geometry::ParcelInsets {
                front_m: 0,
                side_m: 0,
                rear_m: 0,
            }),
            rural_conversion: super::frontage::RuralConversion::Disabled,
        };
        // B01-T2 R13 (T-0013): a grading is only usable if THIS edge's own roadside
        // pad can be entered from its graded surface. The pad is flat and the carve
        // is fill-only, so its declared top is the maximum the road network requires
        // inside the pad's window (rule A'). When that maximum sits further above the
        // graded surface than the entrance apron can walk, NO station of this edge can
        // carry an enterable pad, and keeping the grading only produces sites that are
        // planned, built and then found unreachable.
        //
        // Measured on the frozen joint fixture BEFORE this gate (`rk-x/joint-a01`,
        // 2026-10-18): 0 of 22 sites carried a frontage point or a graded datum, all
        // 17 realised homes were `connected` to a DIFFERENT road than the one they
        // were assigned, and `realizedResidentialWithAccess` was 1 of 18. Refusing
        // every base grading restored 18 of 18, so the mechanism is the grading, not
        // the pad rule.
        //
        // The refusal is BY NAME and the edge keeps the flat legacy level the pad
        // contract was validated against; a graded edge is never silently flattened
        // and a flat road is never silently substituted for a graded one.
        let apron_capacity_m = super::access::WALKABLE_GRADE * super::ENTRANCE_APRON_RUN_M;
        let (probe_audit_half, _) = super::support_pad_half_extents(24., 24., 0., true);
        let probe_audit_win: i32 = probe_audit_half.ceil() as i32;
        if settlement_demand {
            // The fill-only context: `with_cut(0)` is the historical solver, i.e. the
            // depth every frontage contract in this project was validated at. Read
            // once, because a grading the cut strands is retried at that depth below -
            // and only when the world opted into the cut at all, so a world without the
            // policy takes exactly the same path (and the same work) it always took.
            let fill_only = if profile_cut_mm > 0 {
                match zones {
                    Some(zones) => Some(
                        super::road_alignment::AlignmentContext::new(scene, terrain, water, zones)?
                            .with_cut(0),
                    ),
                    None => None,
                }
            } else {
                None
            };
            let probe_entities = result.entities();
            let probe_carves = super::plan_terrain_carves(&probe_entities);
            let mut downgraded: Vec<BaseProfileRejection> = Vec::new();
            for edge in result.edges.iter_mut() {
                let Some(profile) = edge.profile.as_ref() else {
                    continue;
                };
                let Some(probe_entity) = probe_entities
                    .iter()
                    .find(|entity| entity.entity_id == edge.id)
                else {
                    continue;
                };
                // A road that offers no legal station at all carries no pad, so its
                // grading creates no unreachable site and is left alone.
                let Ok(Some(stations)) =
                    super::frontage::stations(probe_entity, &roadside_request)
                else {
                    continue;
                };
                let dx = (profile.end[0] - profile.start[0]).signum();
                let dz = (profile.end[1] - profile.start[1]).signum();
                let mut usable = false;
                let mut best_rise = f64::INFINITY;
                'stations: for &station in &stations {
                    let Some(own_mm) = profile.elevations_mm.get(station).copied() else {
                        continue;
                    };
                    let rx = profile.start[0] + dx * station as i32;
                    let rz = profile.start[1] + dz * station as i32;
                    for side in [-1, 1] {
                        let (px, pz) = if dx != 0 {
                            (rx, rz + 24 * side)
                        } else {
                            (rx + 24 * side, rz)
                        };
                        let mut required_max_mm = own_mm;
                        let mut complete = true;
                        'window: for zz in pz - probe_audit_win..=pz + probe_audit_win {
                            for xx in px - probe_audit_win..=px + probe_audit_win {
                                let Some(h) = terrain.height(xx, zz) else {
                                    complete = false;
                                    break 'window;
                                };
                                let required = super::apply_terrain_carves(
                                    f64::from(h),
                                    xx,
                                    zz,
                                    &probe_carves,
                                );
                                required_max_mm =
                                    required_max_mm.max((required * 1000.).round() as i64);
                            }
                        }
                        if !complete {
                            continue;
                        }
                        let rise_m = (required_max_mm - own_mm) as f64 / 1000.;
                        best_rise = best_rise.min(rise_m);
                        if rise_m <= apron_capacity_m {
                            usable = true;
                            break 'stations;
                        }
                    }
                }
                if !usable {
                    let edge_id = edge.id.clone();
                    edge.profile = None;
                    downgraded.push(BaseProfileRejection {
                        edge_id,
                        category: "grading_leaves_no_enterable_pad".into(),
                        reason: format!(
                            "edge {}: best station still leaves its 24x24 roadside pad {:.3} m above the graded surface (apron capacity {:.3} m), so the grading earns no frontage",
                            edge.id, best_rise, apron_capacity_m
                        ),
                    });
                }
            }
            // ROUND 41 REPAIR: a cut may not strand the road's OWN frontage.
            //
            // The gate above measures the pad this edge's grading would carry - a flat,
            // fill-only 24x24 platform whose declared top is the highest the composed
            // fills require there - against the surface the world publishes. Under
            // `bounded-cut` the longitudinal solver may place that surface BELOW the
            // natural ground, so a grading that is sound at the fill-only depth can end
            // up further beneath its own pad than the entrance apron can walk. Measured
            // on the 1 km probe: two base edges that carried a graded profile lost it to
            // a 627 mm / 631 mm step against the 500 mm apron, which mattered far beyond
            // the two roads - `urban_growth` reads a run's pavement width from its
            // profile, so a dropped grading also leaves the growth axes, and the world's
            // urban zones were rewritten (pads 46 -> 36, shops 24 -> 15).
            //
            // So a grading is refused only when it cannot serve its own pad at ANY depth
            // it is allowed to take. When the fill-only design carries the pad, the edge
            // keeps its profile AT THAT SHALLOWER DEPTH: the excavation is bounded by the
            // apron contract instead of the grading being thrown away. No contract moves
            // - the same apron, the same `base_edge_profile` validation, the same cap -
            // and a world without the policy (`profile_cut_mm == 0`) never enters here, so
            // its decisions are exactly the ones it always took.
            let mut refused: Vec<BaseProfileRejection> = Vec::new();
            match (profile_cut_mm > 0, fill_only.as_ref()) {
                (true, Some(fill_only)) => {
                    let nodes = std::mem::take(&mut result.nodes);
                    for rejection in downgraded {
                        let Some(index) = result
                            .edges
                            .iter()
                            .position(|edge| edge.id == rejection.edge_id)
                        else {
                            refused.push(rejection);
                            continue;
                        };
                        let edge = result.edges[index].clone();
                        let Ok(profile) = base_edge_profile(&edge, &nodes, fill_only, level) else {
                            refused.push(rejection);
                            continue;
                        };
                        result.edges[index].profile = Some(profile);
                        let entities = result.entities();
                        let carves = super::plan_terrain_carves(&entities);
                        let rise = result.edges[index]
                            .profile
                            .as_ref()
                            .and_then(|profile| {
                                entities
                                    .iter()
                                    .find(|entity| entity.entity_id == rejection.edge_id)
                                    .and_then(|entity| {
                                        best_roadside_pad_rise(
                                            profile,
                                            entity,
                                            &carves,
                                            terrain,
                                            &roadside_request,
                                            probe_audit_win,
                                        )
                                    })
                            });
                        match rise {
                            Some(rise) if rise <= apron_capacity_m => {}
                            _ => {
                                result.edges[index].profile = None;
                                refused.push(rejection);
                            }
                        }
                    }
                    result.nodes = nodes;
                }
                _ => refused = downgraded,
            }
            result.base_profile_rejections.extend(refused);
        }
        // B01-T2 R7-1 (rule A'): the surface the ROADS require, computed with the
        // production carve implementation over the road entities only, plus each
        // road's protected band (pavement + shoulder + graded band).
        let road_entities = result.entities();
        let road_carves = super::plan_terrain_carves(&road_entities);
        // Opt-in waterfront: set once here so every gate below reads the SAME
        // authorization the pads, the shore band and the audit all hang from.
        let waterfront = water.road_water_crossings();
        let road_bands: Vec<(i64, i64, i64, i64)> = road_entities
            .iter()
            .map(|entity| {
                let (hw, hd) = super::graded_flat_top_half_extents(entity);
                (
                    i64::from(entity.world_x),
                    i64::from(entity.world_z),
                    i64::from(hw),
                    i64::from(hd),
                )
            })
            .collect();
        // B01-T2 R9-1: ONE pad window, derived from the same function the carve
        // and the audit use, for every planning scan and the band-overlap check.
        // (The audit checks `world +- (pad half + 2)`; A' used to scan +-12.)
        let (pad_half_f, _) = super::support_pad_half_extents(24., 24., 0., true);
        // R9-1 measured outcome: widening A' to the AUDIT window (+-14) makes the
        // declaration itself higher, which un-refuses the site and returns the yard
        // total to 4419 m2 - i.e. it is worse, not better. The measurements
        // therefore keep TWO windows, both DERIVED (never hand-written):
        //   * pad_win   = the pad's own half extents (spatial_half_extents of 24x24 = 12):
        //                 what A' scans and what the band-overlap check uses;
        //   * audit_win = the pad plus the 2 m apron the carve and the audit use
        //                 (support_pad_half_extents(.., precise = true) = 14): what the
        //                 natural-terrain gates scan, exactly as before.
        let (pad_own_hw_f, _) = super::spatial_half_extents(24., 24., 0.);
        let pad_win: i32 = pad_own_hw_f.ceil() as i32;
        let pad_win_i64: i64 = i64::from(pad_win);
        let audit_win: i32 = pad_half_f.ceil() as i32;
        // Opt-in waterfront: the pad's fill gate scans the window the AUDIT
        // scans, not only the pad's own. A waterfront bank drops several metres
        // inside the 2 m apron between the two windows, so a pad whose own window
        // is legal can still be audited against a cell the gate never looked at.
        // The two windows are byte-identical for every input that did not opt in.
        let fill_gate_win: i32 = if water.road_water_crossings() {
            audit_win
        } else {
            pad_win
        };
        let mut site_rejections: Vec<BaseProfileRejection> = Vec::new();
        // `roadside_request` is defined once at the top of this block: the R13
        // grading-feasibility pre-pass and this station scan must ask the frontage
        // path the SAME question, or the two would disagree about which stations exist.
        for e in &result.edges {
            for side in [-1, 1] {
                // Every legal station of this edge and side, in ascending
                // entrance rise, then the centre placement as the last resort.
                // A single "best" station was the defect: whenever the caller's
                // own gates refused that one (a neighbouring pad inside the 28 m
                // spacing rule, zoning, water, a road it would overlap) the whole
                // edge fell through to the centre, which is usually the deepest
                // point of the profile's local dip and therefore the least
                // reachable placement the edge offers.
                let candidates = roadside_placement_candidates(
                    e,
                    side,
                    &road_entities,
                    &roadside_request,
                    terrain,
                    &road_carves,
                    // The window rule A' must cover is the one the CARVE and the
                    // audit use (pad + 2 m apron = `audit_win`): the pad's door
                    // sits at depth/2 + 1 m from its centre, i.e. inside that
                    // window and outside `pad_win`, and a door whose cell the
                    // declaration never saw is exactly the
                    // `entrance_step_requires_engineering` refusal.
                    audit_win,
                    scene,
                    waterfront,
                );
                for (x, z, station_own_mm, frontage_point, legacy_parcel) in candidates {
                let (x, z) = (i64::from(x), i64::from(z));
                if x - 14 < i64::from(scene.origin_x)
                    || z - 14 < i64::from(scene.origin_z)
                    || x + 14 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
                    || z + 14 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
                {
                    continue;
                }
                let (x, z) = (x as i32, z as i32); // Bounds checked in i64 above.
                if let Some(reject) = zoning.as_ref().and_then(|mask| {
                    mask.check(
                        format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        "residential_home",
                        x,
                        z,
                        14,
                        14,
                    )
                }) {
                    result.zoning_rejections.push(reject);
                    continue;
                }
                if water.road_water_crossings() {
                    // Opt-in waterfront: a pad stands on DRY land and keeps the
                    // shore clearance the published scale audit already measures
                    // (`water_query::WATER_SHORE_CLEARANCE_M`, one constant for
                    // both). The footprint is dilated by that clearance, which
                    // proves the distance rather than sampling it, and also proves
                    // the lot owns no cell of the reserved shore band. The refusal
                    // is NAMED: a silent skip is how a city ends up with no lots
                    // and no explanation.
                    if !water.clears_shore(
                        f64::from(x),
                        f64::from(z),
                        f64::from(pad_win),
                        f64::from(pad_win),
                    ) {
                        site_rejections.push(BaseProfileRejection {
                            edge_id: format!(
                                "generated.roadside.{}.{}.{side}",
                                scene.scene_id, e.id
                            ),
                            category: "pad_touches_water_or_shore_band".into(),
                            reason: format!(
                                "site at ({x},{z}) on road {}: a 24x24 pad there would put water or the reserved shore clearance strip of {} m inside its own footprint",
                                e.id,
                                super::water_query::WATER_SHORE_CLEARANCE_M
                            ),
                        });
                        continue;
                    }
                } else if water.intersects_axis_aligned(f64::from(x), f64::from(z), 14., 14.) {
                    continue;
                }
                if result.edges.iter().any(|r| {
                    let (hw, hd) = if r.rotation_y_deg == 0. {
                        (r.length_m as i32 / 2 + 3, 7)
                    } else {
                        (7, r.length_m as i32 / 2 + 3)
                    };
                    (r.world_x - x).abs() < hw + 14 && (r.world_z - z).abs() < hd + 14
                }) {
                    continue;
                }
                if result
                    .roadside_sites
                    .iter()
                    .any(|s| (s.world_x - x).abs() < 28 && (s.world_z - z).abs() < 28)
                {
                    continue;
                }
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                let mut protected_cap = f64::INFINITY;
                for zz in z - audit_win..=z + audit_win {
                    for xx in x - audit_win..=x + audit_win {
                        let h = terrain.height(xx, zz).ok_or_else(|| {
                            TerrainError("missing roadside natural sample".into())
                        })?;
                        min = min.min(h);
                        max = max.max(h);
                        if let Some(mask) = &zoning {
                            protected_cap =
                                protected_cap.min(f64::from(h) + mask.fill_capacity_at(xx, zz));
                        }
                    }
                }
                // B01-T2 R8-1: these gates must be evaluated against the height the
                // pad will ACTUALLY be declared at (rule A'), not against the flat
                // `level_m`. For an unprofiled (legacy) adjacent road that declared
                // height is still the flat level, so legacy behaviour is bit-identical;
                // for a profiled road it is the road's own surface, which is why sites
                // whose natural ground sits ABOVE the road could previously be placed
                // and then never realise (carving is fill-only and cannot cut them
                // down). Such a site is now refused BY NAME instead of being placed
                // and left unresolved.
                // The pad's own level is the road's surface AT ITS STATION (the
                // scanned frontage point), not at the edge's centre.
                //
                // Opt-in waterfront: the base of rule A' is the road's OWN graded
                // surface (its profile at that station, or that road's published
                // flat level when it carries no profile) instead of the network's
                // single `level_m`. A shore is a multi-level surface - bank, quay
                // and crossing deck stand at different heights - so a pad declared
                // on the scene datum would either bury a lower neighbour or be
                // buried by a higher one.
                let gate_own_mm = station_own_mm;
                let gate_base_mm = match gate_own_mm {
                    Some(mm) => mm,
                    // A placement without a station is the edge's centre one, whose
                    // frontage point is the edge midpoint; its surface is the own
                    // road's surface there.
                    None if waterfront => road_entities
                        .iter()
                        .find(|entity| entity.entity_id == e.id)
                        .and_then(|road| {
                            road_surface_mm_at(road, f64::from(e.world_x), f64::from(e.world_z))
                        })
                        .unwrap_or(i64::from(level) * 1000),
                    None => i64::from(level) * 1000,
                };
                let mut gate_declared_mm = gate_base_mm;
                for zz in z - pad_win..=z + pad_win {
                    for xx in x - pad_win..=x + pad_win {
                        if let Some(h) = terrain.height(xx, zz) {
                            let required = super::apply_terrain_carves(
                                f64::from(h),
                                xx,
                                zz,
                                &road_carves,
                            );
                            gate_declared_mm =
                                gate_declared_mm.max((required * 1000.).round() as i64);
                        }
                    }
                }
                let gate_level = gate_declared_mm as f64 / 1000.;
                if f64::from(max) > gate_level
                    || gate_level - f64::from(min) > f64::from(max_fill)
                    || gate_level > protected_cap
                {
                    site_rejections.push(BaseProfileRejection {
                        edge_id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        category: "pad_below_natural_ground_or_over_contract".into(),
                        reason: format!(
                            "site at ({x},{z}) on road {}: declared pad {gate_level:.3} m against natural {min:.3}..{max:.3} m (fill cap {max_fill} m, protected cap {protected_cap:.3} m)",
                            e.id
                        ),
                    });
                    continue;
                }
                // B01-T2 R7-1 (rule A'): the pad stays FLAT, but its height is the
                // MAXIMUM the roads already require anywhere inside the pad, so that
                // the fill-only carve (`terrain.max(target)`) can agree with the
                // declaration at every cell. A neighbouring road's blend ramp is
                // allowed to raise the pad - what is forbidden is sharing cells with
                // a road's protected band. Sites that cannot satisfy the existing
                // contracts are refused BY NAME, never placed and never dropped.
                let pad_half: i64 = pad_win_i64; // the shared 24x24 pad window (R9-1)
                let overlaps_band = road_bands.iter().any(|(cx, cz, hw, hd)| {
                    (i64::from(x) - cx).abs() < hw + pad_half
                        && (i64::from(z) - cz).abs() < hd + pad_half
                });
                if overlaps_band {
                    site_rejections.push(BaseProfileRejection {
                        edge_id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        category: "pad_overlaps_road_band".into(),
                        reason: format!(
                            "site at ({x},{z}) on road {} would overlap a road's pavement+shoulder+graded band",
                            e.id
                        ),
                    });
                    continue;
                }
                // Opt-in waterfront: the pad's flat top (rule A') is declared over
                // the AUDIT window, because the published site audit requires every
                // cell of the pad's 2 m apron to equal the declared level exactly.
                // Its base is the pad's OWN road surface - the profile elevation at
                // that station, or that road's published flat level when it carries
                // no profile - raised to whatever a crossing road's fill-only ramp
                // already demands inside that apron. A shore is a multi-level
                // surface, so this base is per site and never the scene datum; the
                // pad still needs no cut (the natural ground may never stand above
                // it) and no more than the roadbed fill contract.
                let waterfront_declared_mm: Option<i64> = if water.road_water_crossings() {
                    let mut declared = gate_base_mm;
                    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
                    for zz in z - audit_win..=z + audit_win {
                        for xx in x - audit_win..=x + audit_win {
                            let natural_m = f64::from(terrain.height(xx, zz).ok_or_else(|| {
                                TerrainError("missing roadside natural sample".into())
                            })?);
                            low = low.min(natural_m);
                            high = high.max(natural_m);
                            let required = super::apply_terrain_carves(
                                natural_m,
                                xx,
                                zz,
                                &road_carves,
                            );
                            declared = declared.max((required * 1000.).round() as i64);
                        }
                    }
                    let declared_f = declared as f64 / 1000.;
                    let own_road_f = gate_base_mm as f64 / 1000.;
                    let entrance_limit = 24.0 * 0.080; // the road's own 80 mm/m grade over the 24 m offset
                    // The apron is what an entrance can climb in one walkable cell.
                    // A pad raised further above its OWN road than that cannot be
                    // entered from the road it fronts: the checked route detours to a
                    // different road and the development audit refuses the lot as
                    // "no_verified_assigned_road_access". The same capacity already
                    // decides `grading_leaves_no_enterable_pad`, so the two gates
                    // cannot disagree about what "enterable" means.
                    let apron_capacity = super::access::WALKABLE_GRADE
                        * super::ENTRANCE_APRON_RUN_M
                        * 1000.;
                    let (category, reason) = if high > declared_f + 1e-9
                        || declared_f - low > f64::from(max_fill) + 1e-9
                        || high - low > f64::from(max_fill)
                    {
                        (
                            "pad_ground_exceeds_roadbed_contract",
                            format!(
                                "site at ({x},{z}) on road {}: its flat pad on {declared_f:.3} m would need {:.3} m of fill over natural {low:.3}..{high:.3} m (cap {max_fill} m, no excavation)",
                                e.id,
                                declared_f - low
                            ),
                        )
                    } else if (declared - gate_base_mm) as f64 > apron_capacity + 1e-6 {
                        (
                            "pad_not_enterable_from_own_road",
                            format!(
                                "site at ({x},{z}) on road {}: its own graded surface stands at {own_road_f:.3} m while the pad has to be {declared_f:.3} m to stay flat, a {:.3} m entrance step the {} m apron cannot carry",
                                e.id,
                                declared_f - own_road_f,
                                super::ENTRANCE_APRON_RUN_M
                            ),
                        )
                    } else if declared_f - own_road_f > entrance_limit + 1e-9 {
                        (
                            "entrance_grade_exceeds_contract",
                            format!(
                                "site at ({x},{z}) on road {}: its own road stands at {own_road_f:.3} m while the pad has to be {declared_f:.3} m to stay flat, a {:.3} m entrance step (limit {entrance_limit:.3} m)",
                                e.id,
                                declared_f - own_road_f
                            ),
                        )
                    } else {
                        ("", String::new())
                    };
                    if !category.is_empty() {
                        site_rejections.push(BaseProfileRejection {
                            edge_id: format!(
                                "generated.roadside.{}.{}.{side}",
                                scene.scene_id, e.id
                            ),
                            category: category.into(),
                            reason,
                        });
                        continue;
                    }
                    Some(declared)
                } else {
                    None
                };
                let own_mm = station_own_mm;
                let Some(own_mm) = own_mm else {
                    // legacy / unprofiled adjacent road: keep the old flat-level path
                    // B01-T2 R12: the pad is realized by a fill-only carve, so its
                    // declared flat top has to be the height the gates above just
                    // certified as realizable. `gate_declared_mm` is exactly that:
                    // the network level raised by everything the roads require
                    // inside the pad's own window. Declaring the bare `level`
                    // instead left the declaration below the raster the carve
                    // actually produced, the post-bake rebase then moved the
                    // entity by a whole metre, and the frozen plan no longer
                    // matched the final entity. When `level` is already realizable
                    // the declaration is byte-identical to the legacy shape.
                    // Only a genuinely unprofiled edge (no graded surface at all)
                    // takes the raised declaration: its road is flat on `level`, so
                    // raising the pad to the height its own window really holds keeps
                    // the entrance step inside one walkable cell. A profiled edge
                    // whose station scan found no usable station keeps the legacy
                    // centre placement byte-identically, exactly as the scan's own
                    // contract states.
                    //
                    // Opt-in waterfront: the pad takes the declaration derived above
                    // (its own road's surface over the audited apron) and publishes it
                    // as a precise `support_level_mm` with its frontage point, so it
                    // is a real frontage lot with a checked entrance rather than a
                    // flat pad nobody can walk to.
                    let support_mm: Option<i32> = match waterfront_declared_mm {
                        Some(declared) => i32::try_from(declared).ok(),
                        None => i32::try_from(gate_declared_mm)
                            .ok()
                            .filter(|_| e.profile.is_none())
                            .filter(|mm| i64::from(*mm) != i64::from(level) * 1000),
                    };
                    let mut site = RoadsideSite {
                        program: None,
                        purpose: None,
                        id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        road_id: e.id.clone(),
                        world_x: x,
                        world_z: z,
                        width_m: 24,
                        depth_m: 24,
                        support_level_m: support_mm.map_or(level, |mm| mm.div_euclid(1000)),
                        // R11 G1 applies to the PROFILED path only. This branch
                        // is the legacy unprofiled neighbour: its pad is flat on
                        // `level` and its 2 m ring is ordinary realizable yard,
                        // so its parcel geometry must stay byte-identical to b8
                        // (`None` = ParcelInsets::legacy() = 2/2/2). See the
                        // inspector's R11 note 1, item 3.
                        // Only a truly unprofiled neighbour keeps b8's 2/2/2
                        // ring; a profiled edge that the station scan could not
                        // serve keeps the zero-inset parcel (see the scan above).
                        parcel_insets: if legacy_parcel {
                            None
                        } else {
                            Some(super::parcel_geometry::ParcelInsets {
                                front_m: 0,
                                side_m: 0,
                                rear_m: 0,
                            })
                        },
                        support_level_mm: support_mm,
                        frontage_road_point: support_mm.map(|_| [e.world_x, e.world_z]),
                        bounded_cut_mm: None,
                        // A roadside pad of the settlement programme is NOT a
                        // frontageProgram parcel and never excavates: the opt-in
                        // bounded cut is scoped to `frontage::append`'s pads.
                        // The OPT-IN derived earthwork ring is written by
                        // `earthwork_blend::stamp_plan`, once the pad list is whole
                        // and every generator has run.
                        earthwork_blend_m: None,
                        rotation_y_deg: if e.rotation_y_deg == 0. {
                            if side < 0 { 180. } else { 0. }
                        } else if side < 0 {
                            270.
                        } else {
                            90.
                        },
                        natural_min_m: min,
                        natural_max_m: max,
                    };
                    let verdict = pad_purpose_gate(&site, scene, zones, pad_conversion)?;
                    if let Some(reason) = verdict.reason {
                        // The urban-edge refusal carries the measured distance in the prose (the
                        // category alone cannot say HOW far out the pad stood); every other reason
                        // appends nothing, so its published text is unchanged.
                        let detail = zones.map_or(String::new(), |zones| {
                            super::frontage::pad_refusal_detail(&site, zones, reason)
                        });
                        site_rejections.push(BaseProfileRejection {
                            edge_id: site.id.clone(),
                            category: reason.into(),
                            reason: format!(
                                "pad at ({x},{z}) on road {}: {reason}{detail} for a {} pad",
                                e.id,
                                site.kind()
                            ),
                        });
                        continue;
                    }
                    site.purpose = verdict.declared.map(str::to_owned);
                    result.roadside_sites.push(site);
                    continue;
                };
                let mut declared_mm = waterfront_declared_mm.unwrap_or(own_mm);
                let mut worst_fill = f64::NEG_INFINITY;
                // Same window as the station scan and the carve/audit: the pad
                // plus the 2 m apron, so the declaration covers the door cell.
                for zz in z - audit_win..=z + audit_win {
                    for xx in x - audit_win..=x + audit_win {
                        let natural_mm = f64::from(
                            terrain
                                .height(xx, zz)
                                .ok_or_else(|| TerrainError("missing roadside natural sample".into()))?,
                        );
                        let required = super::apply_terrain_carves(
                            natural_mm,
                            xx,
                            zz,
                            &road_carves,
                        );
                        declared_mm = declared_mm.max((required * 1000.).round() as i64);
                    }
                }
                let declared_f = declared_mm as f64 / 1000.;
                for zz in z - fill_gate_win..=z + fill_gate_win {
                    for xx in x - fill_gate_win..=x + fill_gate_win {
                        let natural_m = f64::from(
                            terrain
                                .height(xx, zz)
                                .ok_or_else(|| TerrainError("missing roadside natural sample".into()))?,
                        );
                        let cap = zoning.as_ref().map_or(f64::from(max_fill), |mask| {
                            mask.fill_capacity_at(xx, zz).min(f64::from(max_fill))
                        });
                        let fill = declared_f - natural_m;
                        worst_fill = worst_fill.max(fill);
                        if fill < -1e-9 || fill > cap + 1e-9 {
                            worst_fill = f64::INFINITY;
                        }
                    }
                }
                let entrance_drop = (declared_f - own_mm as f64 / 1000.).abs();
                let entrance_limit = 24.0 * 0.080; // the road's own 80 mm/m grade over the 24 m offset
                let (category, reason) = if !worst_fill.is_finite() || worst_fill > f64::from(max_fill) + 1e-9 {
                    (
                        "pad_fill_exceeds_contract",
                        format!(
                            "site at ({x},{z}) on road {} needs {:.3} m of fill (cap {max_fill} m)",
                            e.id, worst_fill
                        ),
                    )
                } else if entrance_drop > entrance_limit + 1e-9 {
                    (
                        "entrance_grade_exceeds_contract",
                        format!(
                            "site at ({x},{z}) on road {} has a {:.3} m entrance drop (limit {:.3} m)",
                            e.id, entrance_drop, entrance_limit
                        ),
                    )
                } else {
                    ("", String::new())
                };
                if !category.is_empty() {
                    site_rejections.push(BaseProfileRejection {
                        edge_id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                        category: category.into(),
                        reason,
                    });
                    continue;
                }
                let support_mm: Option<i32> = i32::try_from(declared_mm).ok();
                let mut site = RoadsideSite {
                    program: None,
                    purpose: None,
                    id: format!("generated.roadside.{}.{}.{side}", scene.scene_id, e.id),
                    road_id: e.id.clone(),
                    world_x: x,
                    world_z: z,
                    width_m: 24,
                    depth_m: 24,
                    support_level_m: support_mm.map_or(level, |mm| mm.div_euclid(1000)),
                    parcel_insets: Some(super::parcel_geometry::ParcelInsets { front_m: 0, side_m: 0, rear_m: 0 }),
                    support_level_mm: support_mm,
                    frontage_road_point: support_mm.map(|_| frontage_point),
                    bounded_cut_mm: None,
                    // Settlement-programme pad: never excavates (see above). The
                    // OPT-IN derived earthwork ring is written by
                    // `earthwork_blend::stamp_plan` once the pad list is whole.
                    earthwork_blend_m: None,
                    rotation_y_deg: if e.rotation_y_deg == 0. {
                        if side < 0 { 180. } else { 0. }
                    } else if side < 0 {
                        270.
                    } else {
                        90.
                    },
                    natural_min_m: min,
                    natural_max_m: max,
                };
                let verdict = pad_purpose_gate(&site, scene, zones, pad_conversion)?;
                if let Some(reason) = verdict.reason {
                    let detail = zones.map_or(String::new(), |zones| {
                        super::frontage::pad_refusal_detail(&site, zones, reason)
                    });
                    site_rejections.push(BaseProfileRejection {
                        edge_id: site.id.clone(),
                        category: reason.into(),
                        reason: format!(
                            "pad at ({x},{z}) on road {}: {reason}{detail} for a {} pad",
                            e.id,
                            site.kind()
                        ),
                    });
                    continue;
                }
                site.purpose = verdict.declared.map(str::to_owned);
                result.roadside_sites.push(site);
                }
            }
        }
        result.roadside_site_rejections = site_rejections;
    }
    Ok(result)
}

/// The purpose gate of one roadside pad, as the pad would land.
///
/// Without land-use zoning there is no purpose to declare, so the explicit policy is inert and the
/// pad programme keeps its historical shape. With zoning the pad is judged by
/// `frontage::pad_purpose_for_site`: the pad must declare a purpose first, and only then may its
/// buildable core be urban land (lands as-is), wholly rural land (converted and recorded) or nothing
/// (refused by name).
fn pad_purpose_gate(
    site: &RoadsideSite,
    scene: &SceneSpec,
    zones: Option<&[LandUseZone]>,
    conversion: super::frontage::RuralConversion,
) -> Result<super::frontage::PadPurpose, TerrainError> {
    let Some(zones) = zones else {
        return Ok(super::frontage::PadPurpose::OFF);
    };
    super::frontage::pad_purpose_for_site(site, scene, zones, conversion)
}

pub(super) fn plan_with_program(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    pad_conversion: super::frontage::RuralConversion,
    shorefront_setback_m: Option<u32>,
    profile_cut_mm: i32,
) -> Result<RoadNetworkPlan, TerrainError> {
    let mut result = plan_with_zones_demand(
        scene,
        terrain,
        water,
        zones,
        program.is_some(),
        pad_conversion,
        shorefront_setback_m,
        profile_cut_mm,
    )?;
    let Some(program) = program else {
        return Ok(result);
    };
    let zones = zones.ok_or_else(|| TerrainError("service program requires zoning".into()))?;
    if program.version == 2 {
        return super::joint_frontage::plan(
            result,
            scene,
            terrain,
            water,
            zones,
            program,
            pad_conversion,
        );
    }
    super::services::append(
        &mut result,
        scene,
        terrain,
        water,
        zones,
        program,
        pad_conversion,
    )?;
    Ok(result)
}

pub(super) fn plan_with_style(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkPlan, TerrainError> {
    plan_with_style_declared(
        scene,
        terrain,
        water,
        zones,
        program,
        style,
        super::NaturalSource::Undeclared,
    )
}

/// The best (lowest) rise a 24x24 roadside pad of `request` can achieve against
/// `profile`, measured on the surface `carves` compose.
///
/// This is the enterability probe the grading gate runs, lifted into one place so
/// the gate's refusal and the fill-only retry that may overturn it measure the
/// SAME quantity on the SAME composed raster: the pad's declared top is the
/// highest fill this cell window requires, and the rise is that top above the
/// road's own authored surface at the station. `None` when no station of the edge
/// can carry the pad at all (which is not a refusal: such an edge creates no site).
fn best_roadside_pad_rise(
    profile: &super::road_profile::RoadProfile,
    entity: &super::EntityInstance,
    carves: &[super::TerrainCarve],
    terrain: &BakedTerrain,
    request: &super::frontage::Request,
    probe_audit_win: i32,
) -> Option<f64> {
    let stations = super::frontage::stations(entity, request).ok()??;
    let dx = (profile.end[0] - profile.start[0]).signum();
    let dz = (profile.end[1] - profile.start[1]).signum();
    let mut best_rise = f64::INFINITY;
    for &station in &stations {
        let Some(own_mm) = profile.elevations_mm.get(station).copied() else {
            continue;
        };
        let rx = profile.start[0] + dx * station as i32;
        let rz = profile.start[1] + dz * station as i32;
        for side in [-1, 1] {
            let (px, pz) = if dx != 0 {
                (rx, rz + 24 * side)
            } else {
                (rx + 24 * side, rz)
            };
            let mut required_max_mm = own_mm;
            let mut complete = true;
            'window: for zz in pz - probe_audit_win..=pz + probe_audit_win {
                for xx in px - probe_audit_win..=px + probe_audit_win {
                    let Some(h) = terrain.height(xx, zz) else {
                        complete = false;
                        break 'window;
                    };
                    let required = super::apply_terrain_carves(f64::from(h), xx, zz, carves);
                    required_max_mm = required_max_mm.max((required * 1000.).round() as i64);
                }
            }
            if !complete {
                continue;
            }
            best_rise = best_rise.min((required_max_mm - own_mm) as f64 / 1000.);
        }
    }
    best_rise.is_finite().then_some(best_rise)
}

/// B01-R1 T2 (OPT-B): one base edge -> CorridorRequest -> real solver -> validated RoadProfile.
///
/// Why this exists: `frontage::stations()` requires the road entity to carry a `grounding.road_profile`,
/// and before this the only way an edge obtained one was by matching an entry in `plan.extensions`, which
/// is the registry of roads ADDED by the extension mechanism. Base `generated.road-network.*` edges are
/// not extensions, so they never had a profile and never carried frontage.
///
/// This helper is deliberately small and calls the SAME solver the extension path uses
/// (`AlignmentContext::design`), so slope, transverse raster, fill capacity, protected zoning and water
/// constraints are all still enforced. It invents no flat profile.
///
/// Rejection discipline: every failure returns a structured `TerrainError` naming the edge and the
/// category. The caller keeps the edge and leaves `profile: None`; it must not substitute a stand-in.
fn base_edge_profile(
    edge: &RoadEdge,
    nodes: &[RoadNode],
    context: &super::road_alignment::AlignmentContext<'_>,
    level: i32,
) -> Result<super::road_profile::RoadProfile, (String, String)> {
    // (category, reason) so the refusal is structured and persists with a stable label
    let reject = |category: &str, reason: String| (category.to_string(), reason);
    // The graded chain is only realizable on the world's own raster. A legal
    // `RoadProfile` steps at most 80 mm between adjacent 1 m stations
    // (`RoadProfile::validate_shape`, max_rise_mm_per_m = 80 below), while a
    // quarter-metre raster can only express multiples of 250 mm. The only legal
    // profile a quarter-metre world can hold is therefore a constant one, so a
    // graded base profile can never be realized there and the independent audit
    // (which holds the graded band to 0.05 mm) would refuse it by name anyway.
    // Refuse here, by name, and keep `profile: None`: exactly the discipline
    // every other refusal in this function follows. This is the same rule the
    // extension contract already states in `road_extensions::validate_manifest`
    // ("graded road extensions require millimetre heights").
    if context.terrain.encoding() != Some(crate::height_payload::HeightEncoding::Millimetre) {
        return Err(reject(
            "height_encoding_cannot_represent_graded_profile",
            format!(
                "edge {}: world height encoding {:?} cannot represent a road profile that steps at most 80 mm per metre",
                edge.id,
                context.terrain.encoding()
            ),
        ));
    }
    // category: node/scene identity
    let from = nodes
        .iter()
        .find(|n| n.id == edge.from)
        .ok_or_else(|| reject("endpoint_not_a_node", format!("edge {}: from node {} is not a node of the plan", edge.id, edge.from)))?;
    let to = nodes
        .iter()
        .find(|n| n.id == edge.to)
        .ok_or_else(|| reject("endpoint_not_a_node", format!("edge {}: to node {} is not a node of the plan", edge.id, edge.to)))?;
    // category: axis and length domain
    let dx = i64::from(to.world_x) - i64::from(from.world_x);
    let dz = i64::from(to.world_z) - i64::from(from.world_z);
    if (dx == 0) == (dz == 0) {
        return Err(reject("not_axis_aligned", format!("edge {}: not a single horizontal or vertical axis", edge.id)));
    }
    let length = dx.abs() + dz.abs();
    if length % 2 != 0 {
        return Err(reject("odd_length", format!("edge {}: length {} is odd", edge.id, length)));
    }
    if !(16..=10000).contains(&length) {
        return Err(reject("length_out_of_domain", format!("edge {}: length {} outside 16..=10000", edge.id, length)));
    }
    if u32::try_from(length).ok() != Some(edge.length_m) {
        return Err(reject("length_disagrees_with_endpoints", format!("edge {}: declared length_m {} disagrees with its endpoints ({}m)", edge.id, edge.length_m, length)));
    }
    // category: elevation domain, with CHECKED arithmetic. `saturating_mul(1000).abs()` was wrong twice
    // over: saturation silently clamps into a legal-looking value, and `abs()` on i32::MIN overflows.
    // The RoadProfile domain is -32000..=256000 millimetres, applied per sample by validate_shape.
    let level_mm = i64::from(level)
        .checked_mul(1000)
        .ok_or_else(|| reject("elevation_overflow", format!("edge {}: level {level} mm conversion overflowed", edge.id)))?;
    if !(-32000..=256000).contains(&level_mm) {
        return Err(reject("elevation_out_of_domain", format!("edge {}: level {} m gives {level_mm} mm, outside -32000..=256000", edge.id, level)));
    }
    // category: engineering parameter contract, inherited unchanged
    const ROAD_WIDTH_M: u32 = 8;
    const SHOULDER_M: u32 = 3;
    const MAX_FILL_MM: u32 = 6000;
    const MAX_RISE_MM_PER_M: u32 = 80;
    let start_level_mm = i32::try_from(level_mm)
        .map_err(|_| reject("elevation_overflow", format!("edge {}: {level_mm} mm does not fit i32", edge.id)))?;
    // B01-T2 R3 V2: the platform is DERIVED from the extension contract (see
    // `base_tie_in_platform_m`), not hard-coded.
    let platform_m = base_tie_in_platform_m(edge.length_m);
    let request = super::road_alignment::CorridorRequest {
        scene_id: context.scene.scene_id.clone(),
        start: [from.world_x, from.world_z],
        end: [to.world_x, to.world_z],
        road_width_m: ROAD_WIDTH_M,
        shoulder_width_m: SHOULDER_M,
        max_fill_mm: MAX_FILL_MM,
        max_rise_mm_per_m: MAX_RISE_MM_PER_M,
        start_level_mm: Some(start_level_mm),
        end_level_mm: Some(start_level_mm),
        junction_length_m: platform_m,
    };
    // category: missing samples / infeasible engineering. The solver's own message names
    // slope, transverse raster, fill capacity, protected zoning or water.
    let design = context
        .design(&request)
        .map_err(|e| reject("design_infeasible", format!("edge {}: solver refused: {}", edge.id, e.0)))?;
    let profile = super::road_extensions::profile(&design);
    profile
        .validate_shape()
        .map_err(|e| reject("profile_shape_invalid", format!("edge {}: {}", edge.id, e.0)))?;
    // category: shape/elevation domain of the produced samples, checked explicitly because validate_shape
    // owns the domain and the caller must not assume it succeeded silently
    if profile
        .elevations_mm
        .iter()
        .any(|h| !(-32000..=256000).contains(h))
    {
        return Err(reject("elevation_out_of_domain", format!("edge {}: a produced sample is outside -32000..=256000 mm", edge.id)));
    }
    // category: the profile must describe THIS edge, because entities() builds the entity from the edge
    let expect_rotation = if dx == 0 { 90. } else { 0. };
    if !edge.rotation_y_deg.is_finite() || (edge.rotation_y_deg - expect_rotation).abs() > f32::EPSILON {
        return Err(reject("edge_geometry_not_finite_or_misaligned", format!("edge {}: rotation {} is not the finite cardinal {expect_rotation}", edge.id, edge.rotation_y_deg)));
    }
    if profile.road_width_m != ROAD_WIDTH_M
        || u32::try_from(profile.elevations_mm.len().saturating_sub(1)).ok() != Some(edge.length_m)
        || i64::from(edge.world_x) * 2 != i64::from(profile.start[0]) + i64::from(profile.end[0])
        || i64::from(edge.world_z) * 2 != i64::from(profile.start[1]) + i64::from(profile.end[1])
    {
        return Err(reject("profile_entity_mismatch", format!("edge {}: profile geometry disagrees with the edge it describes", edge.id)));
    }
    Ok(profile)
}

/// B01-T2 R3 V2: the base road's tie-in platform, in metres.
///
/// It is DERIVED from the extension contract's widest legal junction occupancy
/// (`road_extensions::max_junction_occupancy_m`, currently 11 m for the 8/12/16 m
/// contract) with a 12 m design floor, then clamped to half the edge so a short
/// edge cannot make its own design infeasible. Before this the value was the bare
/// literal 8 (and briefly 12), which could silently stop covering the contract if
/// a wider road were ever admitted.
pub(super) fn base_tie_in_platform_m(length_m: u32) -> u32 {
    base_required_tie_in_platform_m().min(length_m / 2).max(1)
}

/// The full platform a base edge is expected to offer. `base_platform_is_short`
/// is true when the edge is too short to provide it.
pub(super) fn base_required_tie_in_platform_m() -> u32 {
    BASE_TIE_IN_PLATFORM_FLOOR_M.max(super::road_extensions::max_junction_occupancy_m() as u32)
}

/// Whether this edge is too short to offer the full tie-in platform, i.e. its
/// endpoints cannot safely host the widest extension the contract admits.
pub(super) fn base_platform_is_short(length_m: u32) -> bool {
    base_tie_in_platform_m(length_m) < base_required_tie_in_platform_m()
}

/// Design floor for the base tie-in platform. The contract-derived value is a
/// lower bound; this floor keeps the 64 m base network's geometry as validated.
const BASE_TIE_IN_PLATFORM_FLOOR_M: u32 = 12;

/// Attach profiles to as many base edges as can LEGALLY carry one.
///
/// An edge whose design or validation fails is left with `profile: None` and a NAMED rejection is
/// returned for persistence. A refusal is NOT an error of the plan: the base road network remains valid
/// for the purposes it already served, and the task contract requires the refusal to be retained rather
/// than forced into a flat profile.
///
/// The refusals are returned rather than logged so the caller can persist them into
/// `RoadNetworkPlan::base_profile_rejections`; discarding them was the F2 defect.
pub(super) fn attach_base_edge_profiles(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    nodes: &[RoadNode],
    level: i32,
    zones: Option<&[LandUseZone]>,
    cut_mm: i32,
) -> Result<Vec<BaseProfileRejection>, TerrainError> {
    let Some(zones) = zones else {
        // with no land-use zones there is no legal corridor to check against. The task forbids inventing
        // one, so base edges keep profile: None and the reason is recorded rather than guessed.
        return Ok(vec![BaseProfileRejection {
            edge_id: "*".into(),
            category: "legacy_no_zones".into(),
            reason: "no land-use zones supplied; base edge profiles not attempted, so no base edge has \
                     frontage eligibility from this path"
                .into(),
        }]);
    };
    // The bounded-cut policy, read once by the caller from the world's own style:
    // `0` is a world that never opted in and keeps the fill-only solver.
    let context =
        super::road_alignment::AlignmentContext::new(scene, terrain, water, zones)?.with_cut(cut_mm);
    // Structural guard (I5): a plan whose edges do not have unique ids, or whose
    // edges do not map onto the node table, is damaged rather than merely
    // unprofiled. That must fail closed so no half-updated public plan escapes.
    {
        let mut ids = BTreeSet::new();
        for edge in plan.edges.iter() {
            if !ids.insert(edge.id.as_str()) {
                return Err(TerrainError(format!(
                    "structural damage: duplicate base edge id {}",
                    edge.id
                )));
            }
        }
        let node_ids: BTreeSet<usize> = nodes.iter().map(|n| n.id).collect();
        for edge in plan.edges.iter() {
            if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
                return Err(TerrainError(format!(
                    "structural damage: base edge {} references a node outside the plan",
                    edge.id
                )));
            }
        }
    }
    let mut refusals = Vec::new();
    let decks: BTreeSet<String> = plan
        .water_crossings
        .iter()
        .map(|crossing| crossing.road_id.clone())
        .collect();
    for edge in plan.edges.iter_mut() {
        if edge.profile.is_some() {
            continue;
        }
        if decks.contains(&edge.id) {
            // A registered water crossing is a structure deck: the corridor
            // designer models a graded EARTHWORK support band, which this span
            // deliberately does not have. The refusal is named instead of being
            // reported as a solver failure, and the deck keeps the flat junction
            // level, so its longitudinal grade is 0 mm/m - inside the contract's
            // 80 mm/m rather than exempt from it.
            refusals.push(BaseProfileRejection {
                edge_id: edge.id.clone(),
                category: "water_crossing_structure_deck".into(),
                reason: format!(
                    "edge {}: registered bridge over structured water carries no graded earthwork corridor, so no longitudinal profile is designed; the deck is flat at the junction level (0 mm/m rise against the 80 mm/m contract)",
                    edge.id
                ),
            });
            continue;
        }
        match base_edge_profile(edge, nodes, &context, level) {
            Ok(profile) => edge.profile = Some(profile),
            Err((category, reason)) => refusals.push(BaseProfileRejection {
                edge_id: edge.id.clone(),
                category,
                reason,
            }),
        }
    }
    // F4: the shape/geometry checks inside `base_edge_profile` build the profile
    // from the edge, but only the REAL `entities()` output can close the
    // contract. Validate each profiled base edge against the entity the product
    // chain will actually consume; a mismatch is a single-edge refusal (the road
    // stays, it just earns no frontage), never a stand-in profile.
    let entities = plan.entities();
    for edge in plan.edges.iter_mut() {
        if edge.profile.is_none() {
            continue;
        }
        let entity = entities
            .iter()
            .find(|entity| entity.entity_id == edge.id)
            .ok_or_else(|| {
                TerrainError(format!(
                    "structural damage: base edge {} has no produced entity",
                    edge.id
                ))
            })?;
        let outcome = edge
            .profile
            .as_ref()
            .expect("checked above")
            .validate_entity(entity);
        if let Err(error) = outcome {
            edge.profile = None;
            refusals.push(BaseProfileRejection {
                edge_id: edge.id.clone(),
                category: "profile_entity_mismatch".into(),
                reason: format!("edge {}: {}", edge.id, error.0),
            });
        }
    }
    Ok(refusals)
}

/// The same planning entry with an explicit declaration of where the scene's
/// authoritative natural surface comes from. Version-3 supply needs it: the
/// demand-led phase must never guess the source, so a caller that does not
/// declare one is refused rather than defaulted to a zero seed or a raster
/// fallback.
pub(super) fn plan_with_style_declared(
    scene: &SceneSpec,
    terrain: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
    source: super::NaturalSource,
) -> Result<RoadNetworkPlan, TerrainError> {
    // One authored switch, read once: the roadside PAD programme and the along-road frontage
    // programme share `style.frontageProgram.ruralConversion`, so they can never disagree about
    // which land may change purpose.
    let pad_conversion = super::frontage::RuralConversion::from_declared(
        style
            .get("frontageProgram")
            .and_then(|program| program.get("ruralConversion")),
    );
    // The bounded-cut policy, read once: the ONE cap the longitudinal road
    // profile may excavate under. `0` (the default, and every world that never
    // opts in) keeps the historical fill-only solver byte for byte.
    let (profile_cut_policy, _) = super::bounded_cut::CutPolicy::from_style(style);
    let profile_cut_mm = profile_cut_policy.cap_mm();
    let mut result = plan_with_program(
        scene,
        terrain,
        water,
        zones,
        program,
        pad_conversion,
        super::frontage::shorefront_row_contract(style),
        profile_cut_mm,
    )?;
    if super::road_extensions::from_style(style)?.is_some() {
        super::road_extensions::append(
            &mut result,
            scene,
            terrain,
            water,
            zones.ok_or_else(|| TerrainError("extensions require zones".into()))?,
            style,
        )?;
    }
    if super::auto_roads::from_style(style)?.is_some() {
        super::auto_roads::append_declared(
            &mut result,
            scene,
            terrain,
            water,
            zones.ok_or_else(|| TerrainError("automatic roads require zones".into()))?,
            style,
            source,
        )?;
    }
    // The graded road's own fill contract, now that every ROAD of the plan exists
    // and before any frontage pad is placed on one. A road whose bounded cut still
    // cannot hold the 6 m roadbed contract is refused BY NAME here and withdrawn
    // (plan + entity list), so no pad is ever attached to a road the world will
    // not publish. The probe carries the cap internally; the plan publishes
    // `road_cut_mm` only after the pad list is whole, because the frontage
    // programme's acceptance gates keep reading the historical fill-only
    // envelopes they were calibrated on.
    let (road_cut_policy, _) = super::bounded_cut::CutPolicy::from_style(style);
    if road_cut_policy.enabled() {
        super::road_support::refuse_overfilled_roads(
            &mut result, scene, terrain, water, zones, style,
        )?;
    }
    super::frontage::append(&mut result, scene, terrain, water, zones, style)?;
    if road_cut_policy.enabled() {
        result.road_cut_mm = Some(road_cut_policy.cap_mm());
    }
    // The pad list is whole: every generator that plans pads has run (the roadside
    // programme of `plan_with_zones_demand`, the service programme or the joint
    // layout of `plan_with_program`, the road extensions, the automatic roads and
    // the frontage programme), and nothing below adds a pad. ONE pass now writes
    // the OPT-IN derived fill-transition ring into the record of every pad that
    // `plan_terrain_carves` carves as a building, so the three pad paths cannot
    // publish three different answers - and so the ring travels with the record
    // into `settlement::development::plan`'s rebuild, which is what makes the
    // plan's pad and the world's pad the same bytes.
    //
    // It runs BEFORE `refuse_unrealizable_pads` on purpose: the plan's own
    // earthwork check is what decides whether a pad's declared flat top is held,
    // so it has to see the ring the bake will actually compose. It runs AFTER
    // `frontage::append` for the same reason in reverse: the acceptance gates of
    // the frontage programme keep reading the historical envelopes they were
    // calibrated on, so the pad list itself cannot move under this switch.
    super::earthwork_blend::stamp_plan(&mut result, style);
    // The pad list is whole. A pad whose declared flat top the plan's own earthwork
    // cannot hold is refused BY NAME here and taken out of the plan, where it can
    // still be seen: publishing it and deleting it from the generated world later
    // is what used to leave the plan, the ledger and the world disagreeing about a
    // pad that no longer existed. Refusing before the next line is what makes the
    // ordering safe - the conversion records are derived from the FINAL pad list,
    // so a refused pad can never earn one.
    refuse_unrealizable_pads(&mut result, scene, terrain, water, zones, source)?;
    // Every pad that declared an URBAN purpose and stands on wholly-rural land converts exactly its
    // own buildable core, once. The records are derived from the FINAL pad list (see
    // `frontage::pad_conversions`), so a pad displaced or renamed by the joint layout cannot leave a
    // stale record behind. With the switch off the list stays empty and no plan gains the key.
    if let Some(zones) = zones {
        // Conversions are spent against the plan's urban purpose allocation
        // (`worldgen::conversion_budget`): the rural land an explicit conversion turns into urban
        // purpose may not exceed the urban land the zoning plans minus the urban land the plan has
        // already committed to roads and parcels. A pad that declares an urban purpose on
        // wholly-rural land and no longer fits that allocation is refused BY NAME here and taken out
        // of the plan, exactly like a pad whose flat top cannot be held: it was planned, so it may
        // not silently become a parcel for land the world never allocated as urban, and it may not
        // push the land-use ledger's urban row past its plan (`ledger purpose overcredited`).
        let outcome = super::frontage::pad_conversions(
            &result,
            scene,
            zones,
            pad_conversion,
            // Only an opted-in world spends against the allocation, so only an opted-in world reads
            // the residual-land policy here: a plan with the switch off keeps its exact bytes and its
            // exact failure order.
            if pad_conversion.is_explicit() {
                super::conversion_budget::residual_open_space_ratio(style, scene)
                    .map_err(TerrainError)?
            } else {
                0.0
            },
        )?;
        let mut refusals = Vec::new();
        for refusal in &outcome.refused {
            refusals.push(BaseProfileRejection {
                edge_id: refusal.parcel_id.clone(),
                category: super::conversion_budget::CONVERSION_EXCEEDS_URBAN_ALLOCATION.into(),
                reason: format!(
                    "pad {} would convert {} m2 of rural land to urban purpose, but the urban \
                     allocation left only {} m2, so the pad is refused here by name instead of \
                     leaving the land-use ledger overcredited",
                    refusal.parcel_id, refusal.required_m2, refusal.remaining_m2,
                ),
            });
            forget_refused_pad(&mut result, &refusal.parcel_id);
        }
        result.roadside_site_rejections.extend(refusals);
        result.pad_conversions = outcome.records;
        result.conversion_allocation = outcome.allocation;
    }
    // Opt-in waterfront: the roadside pad programme is NOT refused as a whole any
    // more, and the reason it used to be is worth recording because this used to
    // be the difference between a planned city and an empty quay.
    //
    // The old rule read "a waterfront shore is a multi-level surface, so a pad
    // declared flat on the scene level cannot hold". The premise is right and the
    // conclusion was wrong: what a shore forbids is a pad declared on the SCENE
    // level, not a flat pad. Each pad therefore takes the elevation of its own
    // road's graded surface at its own station (`support_level_mm`, the mechanism
    // profiled roads already publish), keeps the shore clearance that reserves the
    // public passage band, and is refused BY NAME per site when its own ground
    // cannot hold it (`pad_touches_water_or_shore_band`,
    // `pad_ground_exceeds_roadbed_contract`, `entrance_grade_exceeds_contract`).
    // The plan no longer swallows the pad programme, so the refusals that remain
    // are about individual lots and an audit can read them.
    Ok(result)
}
#[cfg(test)]
mod waterfront_tests {
    //! Opt-in road-first across structured water
    //! (`style.automaticRoadExtensions.allowStructuredWater = "bridge-or-causeway"`).
    //!
    //! The contract, in the order it is asserted:
    //!
    //!  * a water scene WITHOUT the switch still plans no road network at all
    //!    (`enabled` is the only gate, and it has not moved);
    //!  * a water scene WITH the switch plans a connected network and registers
    //!    one bridge per road segment that touches the water;
    //!  * every registered number - span length, wet-cell count, position box,
    //!    bed range, deck level, take-off - equals the value RECOMPUTED from the
    //!    road's own published geometry, the water body and the natural raster;
    //!  * the water surface is exactly the same before and after: nothing is
    //!    filled in the water, so the wetted area cannot move;
    //!  * the existing numeric contracts still hold on the span: the deck sits at
    //!    or above every dry cell, the rise from the take-off stays inside the
    //!    6 m roadbed fill contract, and the flat deck is 0 mm/m against the
    //!    8%/m grade contract;
    //!  * forged records fail BY NAME: an overstated span, a deleted record, a dry
    //!    road marked as a bridge, a wrong water body and a buried deck.
    //!
    //! The rasters are produced by the PRODUCTION encoder
    //! (`generate_chunk_encoded`) from the plan's own entities and carves, so the
    //! audit sees exactly the raster a bake would publish for this plan.
    use super::*;
    use crate::worldgen::{
        LandUseProfile, NaturalSource, SceneSpec, WaterKind, plan_landuse_zones,
        plan_terrain_carves, water_geometry,
    };

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "waterfront".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn style(opt_in: bool) -> serde_json::Value {
        let mut program = serde_json::json!({
            "version": 1,
            "scenes": [{
                "sceneId": "waterfront",
                "maxNewSegments": 1,
                "maxCandidateAttempts": 1,
                "segmentLengthsM": [64],
                "priorityPurposes": ["urban"],
                "reachDistanceM": 16
            }]
        });
        if opt_in {
            program["allowStructuredWater"] = serde_json::json!(STRUCTURED_WATER_OPT_IN);
        }
        serde_json::json!({
            "generationProfile": "strict",
            "heightPrecision": "millimeter",
            "terrainProfile": "plains",
            "landUseProfile": {
                "theme": "temperate-plain",
                "urbanCoreRatio": 0.10,
                "suburbanRatio": 0.10,
                "greenRatio": 0.06,
                "farmRatio": 0.32,
                "forestRatio": 0.14,
                "pastureRatio": 0.26,
                "reserveRatio": 0.04
            },
            "water": { "waterType": "river", "levelPolicy": "horizontal-datum", "levelM": 0 },
            "automaticRoadExtensions": program
        })
    }

    fn landmark() -> super::super::LandmarkSpec {
        serde_json::from_value(serde_json::json!({
            "entityId": "landmark.waterfront-river",
            "name": "waterfront river",
            "type": "river",
            "purpose": "structured water the plan has to span",
            "description": "north-south river through the middle of the town",
            "sceneId": "waterfront",
            "worldX": 256,
            "worldZ": 256,
            "worldY": 0,
            "widthM": 16,
            "depthM": 512,
            "heightM": 1,
            "assetId": "water.waterfront-river"
        }))
        .unwrap()
    }

    fn water(style: &serde_json::Value, scene: &SceneSpec) -> WaterGeometry {
        water_geometry(
            super::super::water_kind(style),
            &[landmark()],
            scene,
            style,
        )
    }

    /// The production raster for this scene and carve set, as the bake would
    /// publish it (`generate_chunk_encoded` writes the same bytes). `zoning` is
    /// the same mask the bake hands the encoder, so protected land keeps its
    /// natural height exactly as it does in a published world.
    fn raster(
        scene: &SceneSpec,
        seed: u64,
        water: WaterGeometry,
        carves: &[super::super::TerrainCarve],
        zoning: Option<&crate::worldgen::zoning::ZoningMask>,
    ) -> BakedTerrain {
        let (height, _surface, _lod2, _terrain) = super::super::generate_chunk_encoded(
            seed,
            scene.origin_x,
            scene.origin_z,
            scene.width_m,
            scene.depth_m,
            water,
            carves,
            false,
            zoning,
        );
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &height,
            )
            .unwrap();
        terrain
    }

    /// Wet cells: inside the water body and strictly below the baked datum.
    fn wet_area(terrain: &BakedTerrain, water: WaterGeometry) -> u64 {
        let (origin_x, origin_z) = terrain.scene_origin();
        let (width, depth) = terrain.scene_dimensions();
        let mut wet = 0_u64;
        for z in origin_z..origin_z + depth as i32 {
            for x in origin_x..origin_x + width as i32 {
                if water.contains_point(f64::from(x), f64::from(z))
                    && terrain.height(x, z).is_some_and(|h| h < 0.)
                {
                    wet += 1;
                }
            }
        }
        wet
    }

    #[test]
    fn without_the_switch_a_water_scene_still_plans_no_network() {
        let style = style(false);
        let scene = scene();
        // `enabled` is the one predicate the whole product chain asks; the switch
        // is the only thing that changed, and without it a structured water body
        // still disables road-first exactly as before.
        assert!(!allows_structured_water(&style));
        let water = water(&style, &scene);
        assert!(!enabled(&style, water));
    }

    #[test]
    fn opt_in_plans_spans_that_match_the_water_and_leave_it_untouched() {
        let seed = 42_u64;
        let style = style(true);
        let scene = scene();
        let water = water(&style, &scene);
        assert!(allows_structured_water(&style));
        assert!(enabled(&style, water));
        assert!(water.road_water_crossings());
        let zones = plan_landuse_zones(
            &scene,
            &LandUseProfile::from_style(&style).unwrap(),
        );
        let mask = crate::worldgen::zoning::ZoningMask::new(&scene, &zones).unwrap();
        let natural = raster(&scene, seed, water, &[], None);
        let source = NaturalSource::AuthoredSeed { seed, precise: false };
        let plan = plan_with_style_declared(
            &scene,
            &natural,
            water,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        assert!(
            !plan.edges.is_empty(),
            "the opt-in has to produce a connected network: {plan:?}"
        );
        assert_eq!(plan.status, "planned_connected_network");
        assert!(
            !plan.water_crossings.is_empty(),
            "a road network over a 16 m river must register spans"
        );

        // The published entities and the production raster for exactly those
        // entities: no rebase, no settlement programme, just the road plan.
        let entities: Vec<EntityInstance> = plan
            .entities()
            .into_iter()
            .chain(plan.site_entities())
            .collect();
        let carves = plan_terrain_carves(&entities);
        let final_terrain = raster(&scene, seed, water, &carves, Some(&mask));

        // (2) every record is a claim about the road's own published geometry.
        let mut measured_total = 0_u32;
        let mut recorded_total = 0_u32;
        for crossing in &plan.water_crossings {
            assert_eq!(crossing.kind, "bridge");
            assert_eq!(crossing.water_id, water_body_id("waterfront", water));
            assert_eq!(crossing.water_kind, "river");
            assert_eq!(crossing.water_level_mm, WATER_DATUM_MM);
            assert!(crossing.length_m > 0 && crossing.wet_cells > 0);
            assert_eq!(
                i64::from(crossing.length_m),
                i64::from(crossing.span[2]) - i64::from(crossing.span[0]) + 1
            );
            let road = plan
                .entities()
                .into_iter()
                .find(|entity| entity.entity_id == crossing.road_id)
                .expect("crossing names a planned road");
            let measured = measure_water_crossing(&road, water, &natural)
                .unwrap()
                .expect("the named road really does touch the water");
            assert_eq!(crossing.length_m, measured.stations_m, "{}", crossing.road_id);
            assert_eq!(crossing.wet_cells, measured.wet_cells);
            assert_eq!(crossing.span, measured.span);
            assert_eq!(crossing.bed_min_mm, measured.bed_min_mm);
            assert_eq!(crossing.bed_max_mm, measured.bed_max_mm);
            assert_eq!(crossing.deck_level_mm, road.world_y * 1000);
            assert!(crossing.deck_level_mm > crossing.water_level_mm);
            recorded_total += crossing.length_m;
            measured_total += measured.stations_m;
        }
        assert_eq!(recorded_total, measured_total);
        // (3) the water surface is EXACTLY the same before and after.
        let wet_before = wet_area(&natural, water);
        let wet_after = wet_area(&final_terrain, water);
        assert!(wet_before > 0);
        assert_eq!(
            wet_before, wet_after,
            "the span must place no earthwork in the water"
        );
        // (4) the independent audit of the same record passes end to end,
        //     including the ledger-side clauses (`verify` re-plans and compares,
        //     checks every road/site transform and the earthwork envelope).
        assert_eq!(
            audit_water_crossings(&plan, water, &natural, &final_terrain).unwrap(),
            Vec::<String>::new()
        );
        let audit = verify_with_style_declared(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.realized_edges, plan.edges.len());
        // (4b) the reserved-land audit is unchanged, and nothing the plan builds
        //      occupies the water: no roadside pad may claim it.
        let protection =
            super::super::zoning::audit_protection(&scene, &zones, &natural, &final_terrain, &entities)
                .unwrap();
        assert_eq!(protection.status, "pass", "{protection:?}");
        for entity in plan.site_entities() {
            assert!(
                !super::super::footprint_intersects_water(&entity, water, 0.),
                "{} occupies the water",
                entity.entity_id
            );
        }
        let deck_roads = plan
            .entities()
            .iter()
            .filter(|entity| entity.grounding.structure_deck)
            .count();
        assert_eq!(deck_roads, plan.water_crossings.len());
        // No earthwork at all under a deck: every watered cell of the span keeps
        // the natural height exactly, which is what makes the wetted area
        // integral to the millimetre.
        for crossing in &plan.water_crossings {
            for z in crossing.span[1]..=crossing.span[3] {
                for x in crossing.span[0]..=crossing.span[2] {
                    if water.contains_point(f64::from(x), f64::from(z)) {
                        assert_eq!(
                            final_terrain.height(x, z),
                            natural.height(x, z),
                            "({x},{z}) under {} was changed",
                            crossing.road_id
                        );
                    }
                }
            }
        }
        // (5) same input, twice: byte-identical plan.
        let again = plan_with_style_declared(
            &scene,
            &natural,
            water,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&plan).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        println!(
            "waterfront opt-in: {} roads, {} spans, {} m of span over water, wet area {} -> {} m2, level {:?} m, deck {:?} mm",
            plan.edges.len(),
            plan.water_crossings.len(),
            recorded_total,
            wet_before,
            wet_after,
            plan.level_m,
            plan.water_crossings.first().map(|c| c.deck_level_mm)
        );
    }

    /// The waterfront pad rule, asserted from the plan's own numbers.
    ///
    /// Three claims, each recomputed rather than trusted:
    ///
    ///  * every pad carries a PRECISE elevation and a frontage point: no lot is
    ///    declared on the scene's single `level_m` any more, which is what the
    ///    retired `waterfront_pad_needs_a_second_level` refusal was about;
    ///  * that elevation IS the surface of the road the pad fronts, at the point
    ///    it fronts it, no more than the entrance apron above it - so the pad can
    ///    be entered from its own road and the shore's other levels cannot bury it;
    ///  * no pad cell stands in the water or in the reserved shore band, which is
    ///    the same statement as "the pad keeps the published shore clearance".
    #[test]
    fn waterfront_pads_stand_on_their_own_road_surface_and_keep_the_shore_band() {
        let seed = 42_u64;
        let style = style(true);
        let scene = scene();
        let water = water(&style, &scene);
        let zones = plan_landuse_zones(
            &scene,
            &LandUseProfile::from_style(&style).unwrap(),
        );
        let natural = raster(&scene, seed, water, &[], None);
        let source = NaturalSource::AuthoredSeed { seed, precise: false };
        let plan = plan_with_style_declared(
            &scene,
            &natural,
            water,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        assert!(
            !plan.roadside_sites.is_empty(),
            "the waterfront fixture has to plan pads to assert the pad rule"
        );
        let roads = plan.entities();
        let apron_mm = 1000. * super::super::access::WALKABLE_GRADE
            * super::super::ENTRANCE_APRON_RUN_M;
        for site in &plan.roadside_sites {
            let level = site.support_level_mm.unwrap_or_else(|| {
                panic!("site {} has no precise elevation: {site:?}", site.id)
            });
            let [fx, fz] = site.frontage_road_point.unwrap_or_else(|| {
                panic!("site {} has no frontage point: {site:?}", site.id)
            });
            let road = roads
                .iter()
                .find(|road| road.entity_id == site.road_id)
                .unwrap_or_else(|| panic!("site {} names no road", site.id));
            let surface_mm = match &road.grounding.road_profile {
                Some(profile) => (profile
                    .height_at(f64::from(fx), f64::from(fz))
                    .expect("frontage point inside the road's profile")
                    * 1000.)
                    .round(),
                None => (f64::from(road.ground_height_m()) * 1000.).round(),
            };
            let rise_mm = f64::from(level) - surface_mm;
            assert!(
                rise_mm >= -1e-9,
                "site {} is below its own road surface: {level} < {surface_mm}",
                site.id
            );
            assert!(
                rise_mm <= apron_mm,
                "site {} stands {:.3} m above its own road, more than the {} m apron can climb",
                site.id,
                rise_mm / 1000.,
                super::super::ENTRANCE_APRON_RUN_M
            );
            // Dry land, and outside the reserved strip, cell by cell.
            let (half_w, half_d) = site.world_half_extents();
            for z in site.world_z - half_d as i32..site.world_z + half_d as i32 {
                for x in site.world_x - half_w as i32..site.world_x + half_w as i32 {
                    assert!(
                        !water.contains_point(f64::from(x) + 0.5, f64::from(z) + 0.5),
                        "site {} stands on water at ({x},{z})",
                        site.id
                    );
                    assert!(
                        !water.shore_band_cell(x, z),
                        "site {} enters the reserved shore band at ({x},{z})",
                        site.id
                    );
                }
            }
            assert!(
                water.clears_shore(
                    f64::from(site.world_x),
                    f64::from(site.world_z),
                    f64::from(half_w as i32),
                    f64::from(half_d as i32)
                ),
                "site {} does not keep the published shore clearance",
                site.id
            );
        }
        // The refusal category that used to swallow the whole pad programme is
        // gone by name, and every refusal that remains is a waterfront one.
        assert!(
            !plan
                .roadside_site_rejections
                .iter()
                .any(|rejection| rejection.category == "waterfront_pad_needs_a_second_level"),
            "the blanket pad refusal must not come back: {:?}",
            plan.roadside_site_rejections
        );
        for rejection in &plan.roadside_site_rejections {
            assert!(
                matches!(
                    rejection.category.as_str(),
                    "pad_touches_water_or_shore_band"
                        | "pad_not_enterable_from_own_road"
                        | "pad_ground_exceeds_roadbed_contract"
                        | "pad_fill_exceeds_contract"
                        | "entrance_grade_exceeds_contract"
                        | "pad_overlaps_road_band"
                        | "pad_below_natural_ground_or_over_contract"
                ),
                "unexpected waterfront pad refusal {}: {}",
                rejection.category,
                rejection.reason
            );
        }
    }

    #[test]
    fn forged_span_records_fail_by_name() {
        let seed = 42_u64;
        let style = style(true);
        let scene = scene();
        let water = water(&style, &scene);
        let zones = plan_landuse_zones(&scene, &LandUseProfile::from_style(&style).unwrap());
        let mask = crate::worldgen::zoning::ZoningMask::new(&scene, &zones).unwrap();
        let natural = raster(&scene, seed, water, &[], None);
        let source = NaturalSource::AuthoredSeed { seed, precise: false };
        let plan = plan_with_style_declared(
            &scene,
            &natural,
            water,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        let entities: Vec<EntityInstance> = plan
            .entities()
            .into_iter()
            .chain(plan.site_entities())
            .collect();
        let final_terrain = raster(&scene, seed, water, &plan_terrain_carves(&entities), Some(&mask));
        let audit = |forged: &RoadNetworkPlan| {
            audit_water_crossings(forged, water, &natural, &final_terrain).unwrap()
        };
        assert!(audit(&plan).is_empty());

        // 1. an inflated span length
        let mut inflated = plan.clone();
        inflated.water_crossings[0].length_m += 3;
        let failures = audit(&inflated);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("disagrees with the water it names"));

        // 2. a deleted record
        let mut deleted = plan.clone();
        let road_id = deleted.water_crossings[0].road_id.clone();
        deleted.water_crossings.remove(0);
        let failures = audit(&deleted);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("unregistered water crossing"));
        assert!(failures[0].contains(&road_id));

        // 3. a dry road marked as a bridge
        let dry = plan
            .entities()
            .into_iter()
            .find(|entity| {
                !plan
                    .water_crossings
                    .iter()
                    .any(|crossing| crossing.road_id == entity.entity_id)
                    && measure_water_crossing(entity, water, &natural)
                        .unwrap()
                        .is_none()
            })
            .expect("a dry road exists");
        let mut dry_marked = plan.clone();
        let mut forged = plan.water_crossings[0].clone();
        forged.road_id = dry.entity_id.clone();
        dry_marked.water_crossings.push(forged);
        let failures = audit(&dry_marked);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("never touches that water body")),
            "{failures:?}"
        );

        // 4. a record that names a different water body
        let mut wrong_water = plan.clone();
        wrong_water.water_crossings[0].water_id = "water.somewhere-else.lake".into();
        let failures = audit(&wrong_water);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("but this scene holds"));

        // 5. a deck that does not clear the water datum: the deck must be the
        //    road's own published surface AND strictly above the datum, so a
        //    lowered deck is refused for whichever of the two it breaks first.
        let mut buried = plan.clone();
        buried.water_crossings[0].deck_level_mm = WATER_DATUM_MM;
        let failures = audit(&buried);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("deck"), "{failures:?}");

        // 6. an unknown structure class
        let mut tunnel = plan.clone();
        tunnel.water_crossings[0].kind = "tunnel".into();
        let failures = audit(&tunnel);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("only [\"bridge\", \"causeway\"] may span water"));

        // A forged record is also refused BY NAME by the production audit entry,
        // instead of being flattened into "plan differs from reconstruction".
        let errored = verify_with_style_declared(
            &inflated,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
            source,
        )
        .unwrap();
        assert_eq!(errored.status, "fail");
        assert!(
            errored
                .failures
                .iter()
                .any(|failure| failure.contains("disagrees with the water it names")),
            "{:?}",
            errored.failures
        );
    }
}

#[cfg(test)]
mod pad_purpose_tests {
    //! The roadside pad programme under `style.frontageProgram.ruralConversion`.
    //!
    //! A pad carries no authored purpose, which is exactly why the historical programme could stand
    //! on any buildable land: the "city" grew on the farmland edge while the urban core stayed empty,
    //! and the purpose-gated frontage requests lost the roadside to it. This module pins the fix to
    //! the SAME conversion mechanism the along-road programme already uses:
    //!
    //!  * with the switch off, the plan is BYTE-IDENTICAL (a blake3 lock on two fixtures: one dry,
    //!    one waterfront) and no pad declares a purpose or a conversion;
    //!  * with it on, every pad declares an urban purpose before it lands, a pad whose buildable core
    //!    is wholly rural is converted and recorded, and the ledger moves exactly that area;
    //!  * `audit_conversions` re-derives every pad record from the pad's own parcel, so a forged area,
    //!    a duplicate parcel, a protected parcel and a deleted record all fail by name.
    use super::*;
    use crate::settlement::landuse_ledger;
    use crate::worldgen::{
        LandUseZone, SceneSpec, frontage, plan_landuse_zones, water_geometry, water_kind,
    };

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "pad-purpose".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    /// A flat natural raster: a pad's earthwork is then the only thing that can move a height, so a
    /// pad either lands on its own declared datum or is refused for a reason that is named.
    fn flat_terrain(scene: &SceneSpec) -> BakedTerrain {
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &crate::height_payload::encode_millimetres(
                    scene.width_m,
                    scene.depth_m,
                    vec![0.; (scene.width_m * scene.depth_m) as usize],
                )
                .unwrap(),
            )
            .unwrap();
        terrain
    }

    /// The zoning this module reasons about: the western half is urban, the eastern half rural. A pad
    /// whose 22 m buildable core stays inside one half is therefore unambiguous, and a pad on the
    /// quarter-kilometre boundary is the case the gate must refuse.
    fn banded_zones() -> Vec<LandUseZone> {
        vec![
            LandUseZone {
                id: "west-urban".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "east-rural".into(),
                kind: "rural".into(),
                world_x: 256,
                world_z: 0,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
        ]
    }

    fn style(switch: Option<&str>) -> serde_json::Value {
        let mut style = serde_json::json!({
            "generationProfile": "strict",
            "heightPrecision": "millimeter",
            "terrainProfile": "plains",
            "landUseProfile": {
                "theme": "temperate-plain",
                "urbanCoreRatio": 0.25,
                "suburbanRatio": 0.05,
                "greenRatio": 0.05,
                "farmRatio": 0.40,
                "forestRatio": 0.10,
                "pastureRatio": 0.10,
                "reserveRatio": 0.05
            },
            "frontageProgram": {
                "version": 1,
                "requests": [{
                    "id": "urban-homes",
                    "sceneId": "pad-purpose",
                    "purpose": "urban",
                    "kind": "residential_home",
                    "count": 4,
                    "widthM": 24,
                    "depthM": 24,
                    "heightM": 8,
                    "stationSpacingM": 32,
                    "setbackM": 8,
                    "maxCandidateAttempts": 256
                }]
            },
            "settlementProgram": {
                "version": 2,
                "housingMinimum": { "pad-purpose": 8 },
                "requests": [{
                    "id": "pad-school",
                    "sceneId": "pad-purpose",
                    "kind": "school",
                    "count": 1,
                    "widthM": 40,
                    "depthM": 30,
                    "heightM": 10
                }]
            }
        });
        if let Some(value) = switch {
            style["frontageProgram"]["ruralConversion"] = serde_json::json!(value);
        }
        style
    }

    fn plan(
        scene: &SceneSpec,
        terrain: &BakedTerrain,
        style: &serde_json::Value,
    ) -> Result<RoadNetworkPlan, crate::baked_terrain::TerrainError> {
        let zones = banded_zones();
        let water = water_geometry(water_kind(style), &[], scene, style);
        plan_with_style_declared(
            scene,
            terrain,
            water,
            Some(&zones),
            None,
            style,
            crate::worldgen::NaturalSource::SuppliedRaster,
        )
    }

    /// The reusable lock the round asks for: one water-free fixture and one waterfront fixture, both
    /// with the switch OFF, pinned by the blake3 of the plan's own published bytes.
    fn blake3_of(plan: &RoadNetworkPlan) -> String {
        blake3::hash(&serde_json::to_vec(plan).unwrap())
            .to_hex()
            .to_string()
    }

    #[test]
    fn the_switch_off_plan_is_byte_identical_and_carries_no_purpose_on_a_dry_fixture() {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        // Absent, null and every other spelling are the historical programme: nothing to declare.
        let off = plan(&scene, &terrain, &style(None)).unwrap();
        let off_bytes = serde_json::to_vec(&off).unwrap();
        assert_eq!(
            blake3_of(&off),
            DRY_PLAN_OFF_BLAKE3,
            "the dry pad programme's published bytes changed with the switch off"
        );
        for other in [None, Some(""), Some("off"), Some("true"), Some("Explicit")] {
            let disabled = plan(&scene, &terrain, &style(other)).unwrap();
            assert_eq!(
                serde_json::to_vec(&disabled).unwrap(),
                off_bytes,
                "only the exact string \"explicit\" may enable the pad policy, not {other:?}"
            );
        }
        // The pads really are there - a lock on an empty plan would prove nothing - and none of
        // them declares a purpose, while the plan carries no conversion list at all.
        assert!(
            !off.roadside_sites.is_empty(),
            "the fixture must plan pads, or the off-lock proves nothing"
        );
        assert!(
            off.roadside_sites.iter().all(|site| site.purpose.is_none()),
            "a pad of a switched-off world may not gain a purpose"
        );
        assert!(off.pad_conversions.is_empty());
        let json = serde_json::to_value(&off).unwrap();
        assert!(
            json.get("padConversions").is_none(),
            "a plan without pad conversions must not gain the key"
        );
        assert!(
            json["roadsideSites"]
                .as_array()
                .unwrap()
                .iter()
                .all(|site| site.get("purpose").is_none()),
            "a pad without a purpose must not gain the key"
        );
    }

    #[test]
    fn the_switch_off_plan_is_byte_identical_on_a_waterfront_fixture() {
        // The structured-water opt-in is what puts the pad programme on a graded bank, where every
        // pad already carries a precise datum: the purpose switch must still change nothing.
        let scene = SceneSpec {
            scene_id: "pad-purpose".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut style = style(None);
        style["automaticRoadExtensions"] = serde_json::json!({
            "version": 1,
            "allowStructuredWater": STRUCTURED_WATER_OPT_IN,
            "scenes": [{
                "sceneId": "pad-purpose",
                "maxNewSegments": 2,
                "maxCandidateAttempts": 64,
                "segmentLengthsM": [64],
                "priorityPurposes": ["urban", "rural"],
                "reachDistanceM": 32
            }]
        });
        style["water"] = serde_json::json!({
            "waterType": "river",
            "levelPolicy": "horizontal-datum",
            "levelM": 0
        });
        let landmark: crate::worldgen::LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId": "landmark.pad-river",
            "name": "pad river",
            "type": "river",
            "purpose": "structured water the pads have to keep clear of",
            "description": "north-south river through the middle of the pad bank",
            "sceneId": "pad-purpose",
            "worldX": 256,
            "worldZ": 256,
            "worldY": 0,
            "widthM": 16,
            "depthM": 512,
            "heightM": 1,
            "assetId": "water.pad-river"
        }))
        .unwrap();
        let water = water_geometry(water_kind(&style), &[landmark], &scene, &style);
        let mask = crate::worldgen::zoning::ZoningMask::new(&scene, &banded_zones()).unwrap();
        let (height, _surface, _lod2, _terrain) = crate::worldgen::generate_chunk_encoded(
            42,
            scene.origin_x,
            scene.origin_z,
            scene.width_m,
            scene.depth_m,
            water,
            &[],
            // a millimetre source, because this fixture really authors a frontage programme
            true,
            Some(&mask),
        );
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &height,
            )
            .unwrap();
        let built = plan_with_style_declared(
            &scene,
            &terrain,
            water,
            Some(&banded_zones()),
            None,
            &style,
            crate::worldgen::NaturalSource::AuthoredSeed {
                seed: 42,
                precise: true,
            },
        )
        .unwrap();
        assert!(
            !built.roadside_sites.is_empty(),
            "the waterfront fixture must plan pads, or the off-lock proves nothing"
        );
        assert!(
            built.roadside_sites.iter().all(|site| site.purpose.is_none()),
            "a waterfront pad of a switched-off world may not gain a purpose"
        );
        assert_eq!(
            blake3_of(&built),
            WATERFRONT_PLAN_OFF_BLAKE3,
            "the waterfront pad programme's published bytes changed with the switch off"
        );
    }

    #[test]
    fn the_switch_on_declares_urban_and_converts_exactly_the_rural_core_it_takes() {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        let zones = banded_zones();
        let off = plan(&scene, &terrain, &style(None)).unwrap();
        let on = plan(&scene, &terrain, &style(Some("explicit"))).unwrap();
        // Every pad declares the purpose of its own kind before it lands.
        assert!(!on.roadside_sites.is_empty());
        for site in &on.roadside_sites {
            if site.program.as_ref().is_some_and(|program| program.request_id == "urban-homes") {
                // a frontage parcel carries its purpose on its authored request instead
                continue;
            }
            assert_eq!(
                site.purpose.as_deref(),
                Some("urban"),
                "pad {} did not declare its purpose",
                site.id
            );
            assert_eq!(
                frontage::pad_purpose(site.kind()),
                Some("urban"),
                "the declared purpose must be the one the pad's own kind maps to"
            );
        }
        // The conversion list is exactly the pads whose buildable core is wholly rural - and every
        // one of them is a pad, not a frontage parcel.
        let mut expected: Vec<frontage::Conversion> = Vec::new();
        for site in &on.roadside_sites {
            if site.purpose.is_none() {
                continue;
            }
            if let Some(area) = frontage::pad_conversion_area(site, &zones) {
                expected.push(frontage::Conversion::rural_to_urban(&site.id, area));
            }
        }
        assert_eq!(on.pad_conversions, expected);
        assert!(
            !on.pad_conversions.is_empty(),
            "the fixture must convert at least one pad, or the on-case proves nothing"
        );
        for record in &on.pad_conversions {
            let site = on
                .roadside_sites
                .iter()
                .find(|site| site.id == record.parcel_id)
                .expect("a record must name a planned pad");
            let core = frontage::buildable_core(
                frontage::parcel_boundary(site).unwrap(),
                site.parcel_insets.unwrap_or_default(),
                site.rotation_y_deg,
            )
            .expect("a planned pad has a buildable core");
            assert_eq!(
                record.area_m2,
                (core[2] - core[0]) * (core[3] - core[1]),
                "a pad converts its buildable core, not its parcel"
            );
            assert_eq!((record.from.as_str(), record.to.as_str()), ("rural", "urban"));
        }
        // Round 31 narrows this contract on purpose. Until the urban-edge policy, the switch only
        // re-booked land: a pad that declared an urban purpose could stand anywhere its own envelope
        // allowed and the ledger merely moved its core from rural to urban. That is what put urban
        // purpose in the middle of farmland, so a pad whose core is rural land outside the 64 m band
        // (`conversion_fringe`) is now REFUSED BY NAME and never planned - the programme may therefore
        // be SMALLER with the switch on than with it off. What must still hold is the triangle: every
        // pad that is gone is named, and the number of pads that disappeared is the number of
        // out-of-band refusals.
        let out_of_band = on
            .roadside_site_rejections
            .iter()
            .filter(|rejection| {
                rejection.category
                    == crate::worldgen::conversion_fringe::CONVERSION_OUTSIDE_THE_URBAN_FRINGE
            })
            .count();
        assert!(
            out_of_band > 0,
            "the fixture's east half carries pads further than 64 m from the urban edge, or the \
             narrowing this test now pins would be untested"
        );
        assert!(
            on.roadside_sites.len() + out_of_band >= off.roadside_sites.len(),
            "no pad may disappear without a named urban-edge refusal: {} planned + {} refused < {} \
             historical",
            on.roadside_sites.len(),
            out_of_band,
            off.roadside_sites.len()
        );
        // Every pad that is still planned either stands on urban land or converts inside the band.
        for site in &on.roadside_sites {
            let Some(core) = frontage::site_buildable_core(site, &zones) else {
                continue;
            };
            assert!(
                !frontage::core_is_rural(core, &zones)
                    || crate::worldgen::conversion_fringe::within_band(
                        crate::worldgen::conversion_fringe::distance_to_urban(core, &zones)
                    ),
                "pad {} stands on rural land outside the urban fringe",
                site.id
            );
        }
        // And every refusal that names the urban edge carries the distance it measured.
        for rejection in &on.roadside_site_rejections {
            if rejection.category
                != crate::worldgen::conversion_fringe::CONVERSION_OUTSIDE_THE_URBAN_FRINGE
            {
                continue;
            }
            assert!(
                rejection.reason.contains("m from the nearest urban zone"),
                "an urban-edge refusal must publish its distance: {}",
                rejection.reason
            );
        }
        // The ledger applies exactly the plan's records, once, and the audit re-derives them.
        let converted: i64 = on.pad_conversions.iter().map(|record| record.area_m2).sum();
        let audit = landuse_ledger::audit_conversions(&on, &zones, water_geometry(
            water_kind(&style(None)),
            &[],
            &scene,
            &style(None),
        ), None);
        assert_eq!(
            (audit.planned, audit.converted_m2),
            (on.pad_conversions.len(), converted),
            "{audit:?}"
        );
        // ... and the identity the ledger enforces is the one the records claim: the sum of the pad
        // records is what moves between the two rows.
        assert!(converted > 0);
    }

    /// The blake3 locks above. They are fixtures, not published samples: if one of them changes the
    /// switch-off pad programme moved, which is exactly what this round may not do.
    ///
    /// `WATERFRONT_PLAN_OFF_BLAKE3` moved ONCE, in Round 24, and on purpose: this fixture states a
    /// `frontageProgram`, which is the street morphology the shoreline commercial row is planned
    /// from (`frontage::shorefront_row_contract`), so the waterfront plan now carries the row's own
    /// outcome - including the stations it refused, BY NAME, which is the record that keeps a missing
    /// quay frontage a measurement instead of a silence. Nothing about the pads moved with it: the
    /// dry lock below is unchanged, and the switch-off pad programme still declares no purpose.
    const DRY_PLAN_OFF_BLAKE3: &str = "c007835a3b0fe905882b06a601de1219c9c182c2ad75afac61489d7a8b285b4c";
    const WATERFRONT_PLAN_OFF_BLAKE3: &str =
        "33718fc32c41a810e24abf60de587c04ae44b4ec496b0a88eafd141efe106a6e";
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Test-only view of the base domain (same rule as the append phase's
    /// trusted contract derivation: B01-T2 C3 identity by planning provenance,
    /// not by profile presence).
    #[allow(dead_code)]
    pub(super) fn base_only_plan_for_test(plan: &RoadNetworkPlan) -> RoadNetworkPlan {
        let extension_ids: std::collections::BTreeSet<&str> = plan
            .extensions
            .iter()
            .map(|extension| extension.entity_id.as_str())
            .collect();
        let automatic_ids: std::collections::BTreeSet<&str> = plan
            .automatic_extensions
            .as_ref()
            .map(|report| {
                report
                    .generated_road_ids
                    .iter()
                    .map(String::as_str)
                    .collect()
            })
            .unwrap_or_default();
        let mut base = plan.clone();
        base.edges.retain(|edge| {
            !extension_ids.contains(edge.id.as_str()) && !automatic_ids.contains(edge.id.as_str())
        });
        let keep: std::collections::BTreeSet<usize> =
            base.edges.iter().flat_map(|e| [e.from, e.to]).collect();
        base.nodes.retain(|node| keep.contains(&node.id));
        base.extensions.clear();
        base
    }
    fn fixture() -> (SceneSpec, BakedTerrain, WaterGeometry) {
        let scene = SceneSpec {
            scene_id: "roads".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: -256,
            origin_z: -256,
            seed_offset: 0,
        };
        let mut raster = BakedTerrain::new(&scene).unwrap();
        raster
            .insert_chunk(-256, -256, 512, 512, &0_i16.to_le_bytes().repeat(512 * 512))
            .unwrap();
        let water =
            super::super::legacy_water_geometry(super::super::WaterKind::None, 512, 512, 0.);
        (scene, raster, water)
    }
    #[test]
    fn flat_network_is_connected_nonempty_and_deterministic() {
        let (s, t, w) = fixture();
        let a = plan(&s, &t, w).unwrap();
        let b = plan(&s, &t, w).unwrap();
        assert!(!a.edges.is_empty());
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
        let mut visited = BTreeSet::from([a.nodes[0].id]);
        loop {
            let n = visited.len();
            for e in &a.edges {
                if visited.contains(&e.from) || visited.contains(&e.to) {
                    visited.insert(e.from);
                    visited.insert(e.to);
                }
            }
            if n == visited.len() {
                break;
            }
        }
        assert_eq!(visited.len(), a.nodes.len());
        assert_eq!(a.level_m, Some(0));
    }

    #[test]
    fn the_legacy_undeclared_entry_and_unknown_sources_are_refused() {
        use crate::height_payload::encode_millimetres;
        let (scene, _flat, water) = fixture();
        let zones = vec![LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: scene.origin_x,
            world_z: scene.origin_z,
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            source: "test".into(),
        }];
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let values: Vec<f64> = (0..scene.depth_m as i32)
            .flat_map(|z| (0..scene.width_m as i32).map(move |x| (x, z)))
            .map(|(x, z)| {
                super::super::terrain_height_metres(
                    0,
                    scene.origin_x + x,
                    scene.origin_z + z,
                    water,
                    &[],
                )
            })
            .collect();
        natural
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        let plan = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        // The old undeclared audit wrapper must still refuse a version-3 input
        // rather than silently falling back to a zero seed or a sampled source.
        let v3_style = serde_json::json!({
            "frontageProgram": {"version": 1, "requests": [{
                "id": "homes", "sceneId": "roads", "purpose": "urban",
                "kind": "residential_home", "count": 1, "widthM": 12, "depthM": 10,
                "heightM": 8, "stationSpacingM": 24, "setbackM": 8,
                "maxCandidateAttempts": 8,
                "parcelInsets": {"frontM": 4, "sideM": 2, "rearM": 4}
            }]},
            "automaticRoadExtensions": {"version": 3, "scenes": [{
                "sceneId": "roads", "maxNewSegments": 1, "maxCandidateAttempts": 8,
                "segmentLengthsM": [64], "priorityPurposes": ["urban"],
                "reachDistanceM": 32,
                "hierarchy": {"tiers": [
                    {"id": "collector-16", "role": "collector", "widthM": 16, "purposes": ["urban"]},
                    {"id": "local-12", "role": "local", "widthM": 12, "purposes": ["urban"]}
                ]},
                "supply": {"version": 1, "maxCandidateStates": 1, "preflightBudgetMs": 180000, "mode": "append"}
            }]},
        });
        let error = verify_with_style(
            &plan,
            &scene,
            &natural,
            &natural,
            water,
            &entities,
            Some(&zones),
            None,
            &v3_style,
        )
        .unwrap_err();
        assert!(
            error.0.contains("declared natural source"),
            "{}",
            error.0
        );
        // The manifest-bound derivation is the shared rule, and an authored
        // source this build cannot reproduce is refused instead of guessed onto
        // `SuppliedRaster`.
        let mut manifest = super::super::WorldManifest::default_demo();
        manifest.scenes = vec![scene.clone()];
        manifest.style["heightPrecision"] = serde_json::json!("millimeter");
        let declared = super::super::declared_natural_source(&manifest, &scene).unwrap();
        assert_eq!(
            declared,
            super::super::NaturalSource::AuthoredSeed {
                seed: manifest.world.seed ^ scene.seed_offset,
                precise: true,
            }
        );
        manifest.style["terrain"] = serde_json::json!("imported-raster");
        let error = super::super::declared_natural_source(&manifest, &scene).unwrap_err();
        assert!(
            error.to_string().contains("unsupported authored terrain source"),
            "{error}"
        );
    }

    #[test]
    fn shared_final_support_leaf_is_the_production_audit_body() {
        use crate::height_payload::encode_millimetres;
        let (scene, _flat, water) = fixture();
        let zones = vec![LandUseZone {
            id: "urban".into(),
            kind: "urban".into(),
            world_x: scene.origin_x,
            world_z: scene.origin_z,
            width_m: scene.width_m,
            depth_m: scene.depth_m,
            source: "test".into(),
        }];
        // The authoritative natural source is the authored seed field, so the
        // production encoder and the carve path agree cell for cell.
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let values: Vec<f64> = (0..scene.depth_m as i32)
            .flat_map(|z| (0..scene.width_m as i32).map(move |x| (x, z)))
            .map(|(x, z)| {
                super::super::terrain_height_metres(
                    0,
                    scene.origin_x + x,
                    scene.origin_z + z,
                    water,
                    &[],
                )
            })
            .collect();
        natural
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        let plan = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let carves = super::super::plan_terrain_carves(&entities);
        let mask = super::super::zoning::ZoningMask::new(&scene, &zones).unwrap();
        let (height, _s, _l, _t) = super::super::generate_chunk_encoded(
            0,
            scene.origin_x,
            scene.origin_z,
            scene.width_m,
            scene.depth_m,
            water,
            &carves,
            true,
            Some(&mask),
        );
        let mut final_terrain = BakedTerrain::new(&scene).unwrap();
        final_terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &height,
            )
            .unwrap();
        let empty_style = serde_json::json!({});
        let production = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &empty_style,
        )
        .unwrap();
        let leaf = audit_final_support_for_record(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &empty_style,
        )
        .unwrap();
        assert_eq!(production.realized_edges, leaf.realized_edges);
        assert_eq!(production.planned_edges, leaf.planned_edges);
        assert_eq!(production.realized_sites, leaf.realized_sites);
        assert_eq!(production.failures, leaf.failures);
        assert!(plan.edges.len() > 0);
    }

    #[test]
    fn final_audit_detects_deleted_road_and_forged_plan() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        let mut entities = planned.site_entities();
        entities.extend(planned.entities());
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "pass"
        );
        entities.pop();
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "fail"
        );
        let mut forged = planned.clone();
        forged.edges.pop();
        assert!(verify(&forged, &scene, &natural, &natural, water, &entities).is_err());
    }
    #[test]
    fn roadside_audit_rejects_missing_duplicate_moved_and_lifted_sites() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        assert!(!planned.roadside_sites.is_empty());
        let mut entities = planned.entities();
        entities.extend(planned.site_entities());
        let valid = verify(&planned, &scene, &natural, &natural, water, &entities).unwrap();
        assert_eq!(valid.status, "pass");
        assert_eq!(valid.realized_sites, planned.roadside_sites.len());
        let site_index = planned.edges.len();
        let mut missing = entities.clone();
        missing.remove(site_index);
        assert!(
            verify(&planned, &scene, &natural, &natural, water, &missing)
                .unwrap()
                .failures
                .iter()
                .any(|f| f.contains("missing planned roadside site"))
        );
        let mut duplicate = entities.clone();
        duplicate.push(entities[site_index].clone());
        assert!(
            verify(&planned, &scene, &natural, &natural, water, &duplicate)
                .unwrap()
                .failures
                .iter()
                .any(|f| f.contains("duplicate roadside site"))
        );
        let mut moved = entities.clone();
        moved[site_index].world_x += 1;
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &moved)
                .unwrap()
                .status,
            "fail"
        );
        let site = &planned.roadside_sites[0];
        let mut bytes = 0_i16.to_le_bytes().repeat(512 * 512);
        let offset =
            (((site.world_z - scene.origin_z) * 512 + site.world_x - scene.origin_x) * 2) as usize;
        bytes[offset..offset + 2].copy_from_slice(&4_i16.to_le_bytes());
        let mut lifted = BakedTerrain::new(&scene).unwrap();
        lifted
            .insert_chunk(scene.origin_x, scene.origin_z, 512, 512, &bytes)
            .unwrap();
        let audit = verify(&planned, &scene, &natural, &lifted, water, &entities).unwrap();
        assert_eq!(audit.status, "fail");
        assert_eq!(audit.realized_edges, planned.edges.len());
        assert_eq!(audit.max_site_design_height_error_m, 1.);
    }

    #[test]
    fn roadside_front_doors_face_the_assigned_road() {
        let (scene, natural, water) = fixture();
        let planned = plan(&scene, &natural, water).unwrap();
        for (site, entity) in planned.roadside_sites.iter().zip(planned.site_entities()) {
            let road = planned.edges.iter().find(|e| e.id == site.road_id).unwrap();
            let a = &entity.anchors[0];
            let original = (i64::from(entity.world_x) - i64::from(road.world_x)).abs()
                + (i64::from(entity.world_z) - i64::from(road.world_z)).abs();
            let door = (i64::from(a.world_x) - i64::from(road.world_x)).abs()
                + (i64::from(a.world_z) - i64::from(road.world_z)).abs();
            assert_eq!(original, 24);
            assert_eq!(door, 11);
            assert_eq!(a.target.as_deref(), Some("road"));
        }
        let mut entities = planned.entities();
        entities.extend(planned.site_entities());
        entities[planned.edges.len()].anchors[0].world_x += 1;
        assert_eq!(
            verify(&planned, &scene, &natural, &natural, water, &entities)
                .unwrap()
                .status,
            "fail"
        );
    }

    #[test]
    fn roadside_bounds_are_checked_without_i32_overflow() {
        let (mut scene, _, water) = fixture();
        for origin in [i32::MIN, i32::MAX - 512] {
            scene.origin_x = origin;
            scene.origin_z = origin;
            let mut raster = BakedTerrain::new(&scene).unwrap();
            raster
                .insert_chunk(
                    origin,
                    origin,
                    512,
                    512,
                    &0_i16.to_le_bytes().repeat(512 * 512),
                )
                .unwrap();
            let planned = plan(&scene, &raster, water).unwrap();
            assert!(!planned.edges.is_empty());
            assert!(!planned.roadside_sites.is_empty());
            for site in &planned.roadside_sites {
                assert!(i64::from(site.world_x) - 14 >= i64::from(origin));
                assert!(i64::from(site.world_z) + 14 < i64::from(origin) + 512);
            }
        }
    }

    #[test]
    fn full_raster_peak_is_rejected_and_water_blocks_network() {
        let (scene, _, water) = fixture();
        let mut natural = BakedTerrain::new(&scene).unwrap();
        let mut bytes = 0_i16.to_le_bytes().repeat(512 * 512);
        let idx = (32 * 512 + 48) * 2;
        bytes[idx..idx + 2].copy_from_slice(&160_i16.to_le_bytes());
        natural.insert_chunk(-256, -256, 512, 512, &bytes).unwrap();
        let planned = plan(&scene, &natural, water).unwrap();
        assert!(planned.rejected_relief > 0);
        assert!(!planned.edges.iter().any(|e| e.from == 0 && e.to == 1));
        let wet = WaterGeometry {
            kind: super::super::WaterKind::Lake,
            center_x: 0.,
            center_z: 0.,
            half_width: 2000.,
            half_depth: 2000.,
            scene_width_m: 512,
            scene_depth_m: 512,
            terrain_origin_x: 0,
            terrain_origin_z: 0,
            smooth_rolling: false,
            road_water_crossings: false,
        };
        let planned = plan(&scene, &natural, wet).unwrap();
        assert!(planned.edges.is_empty());
        assert_eq!(planned.rejected_water, planned.candidates);
    }
    #[test]
    fn zoned_planning_rejects_reserved_candidates_and_rebuilds_their_evidence() {
        let (scene, natural, water) = fixture();
        let zones = vec![
            LandUseZone {
                id: "buildable".into(),
                kind: "rural".into(),
                world_x: -256,
                world_z: -256,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "protected".into(),
                kind: "nature".into(),
                world_x: 0,
                world_z: -256,
                width_m: 256,
                depth_m: 512,
                source: "test".into(),
            },
        ];
        let p = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        assert!(!p.edges.is_empty());
        assert!(!p.roadside_sites.is_empty());
        assert!(p.zoning_hash.is_some());
        assert!(!p.zoning_rejections.is_empty());
        assert!(p.roadside_sites.iter().all(|s| s.world_x + 14 < 0));
        let mut entities = p.entities();
        entities.extend(p.site_entities());
        assert_eq!(
            verify_with_zones(
                &p,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&zones)
            )
            .unwrap()
            .status,
            "pass"
        );
        assert!(verify(&p, &scene, &natural, &natural, water, &entities).is_err());
        let mut forged = p.clone();
        forged.zoning_rejections.clear();
        assert!(
            verify_with_zones(
                &forged,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&zones)
            )
            .is_err()
        );
        let mut changed = zones.clone();
        changed[1].kind = "urban".into();
        assert!(
            verify_with_zones(
                &p,
                &scene,
                &natural,
                &natural,
                water,
                &entities,
                Some(&changed)
            )
            .is_err()
        );
    }

    #[test]
    fn missing_raster_cannot_be_silently_treated_as_flat() {
        let (s, _, w) = fixture();
        assert!(plan(&s, &BakedTerrain::new(&s).unwrap(), w).is_err());
    }

    type GradedFixture = (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
        i32,
    );

    /// A gentle cone around (160,160): non-flat in both axes, so a legal graded
    /// corridor exists in every cardinal direction while the shoulder is
    /// genuinely blended. Production planning/carve/audit are used unchanged.
    fn graded_fixture() -> GradedFixture {
        use crate::height_payload::encode_millimetres;
        let scene = SceneSpec {
            scene_id: "graded-audit".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut values = Vec::with_capacity(512 * 512);
        for z in 0..512 {
            for x in 0..512 {
                let fall = (f64::from(x) - 160.).abs() + (f64::from(z) - 160.).abs();
                values.push((8. - 0.05 * fall).max(0.));
            }
        }
        let mut natural = BakedTerrain::new(&scene).unwrap();
        natural
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, values).unwrap(),
            )
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "graded.urban".into(),
            kind: "urban".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let base = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let level_mm = base.level_m.expect("resolved base level") * 1000;
        (scene, natural, water, zones, base, level_mm)
    }

    /// A style with one graded corridor of `width_m`, along the axis implied by
    /// `rotation` (0 = x, 90 = z), starting from a real base node. The search
    /// only accepts candidates the production planner itself accepts.
    fn single_corridor_style(fixture: &GradedFixture, width_m: u32, rotation: i32) -> Option<serde_json::Value> {
        let (scene, natural, water, zones, base, level_mm) = fixture;
        let directions: [(i32, i32); 2] = if rotation == 90 {
            [(0, 1), (0, -1)]
        } else {
            [(1, 0), (-1, 0)]
        };
        for node in &base.nodes {
            for (dx, dz) in directions {
                for length in [64_i32, 96, 48] {
                    let start = [node.world_x, node.world_z];
                    let end = [start[0] + dx * length, start[1] + dz * length];
                    if end[0] < 12 || end[1] < 12
                        || end[0] + 12 > scene.width_m as i32
                        || end[1] + 12 > scene.depth_m as i32
                    {
                        continue;
                    }
                    let candidate = serde_json::json!({
                        "roadExtensions": {"version": 1, "corridors": [{
                            "sceneId": "graded-audit",
                            "start": start,
                            "end": end,
                            "roadWidthM": width_m,
                            "shoulderWidthM": 3,
                            "maxFillMm": 6000,
                            "maxRiseMmPerM": 80,
                            "startLevelMm": level_mm,
                            "endLevelMm": level_mm,
                            "junctionLengthM": 8
                        }]}
                    });
                    let mut probe = base.clone();
                    if super::super::road_extensions::append(
                        &mut probe,
                        scene,
                        natural,
                        *water,
                        zones,
                        &candidate,
                    )
                    .is_ok()
                    {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// R2 W1-2 diagnostic (ignored): dump the graded base plan's nodes, edges and
    /// attached profile samples, so the overlap rejection can be attributed.
    #[test]
    #[ignore = "diagnostic: dumps graded base node/edge/profile geometry"]
    fn w1_diagnose_base_profile_geometry() {
        let fixture = graded_fixture();
        let (scene, _natural, _water, _zones, base, level_mm) = &fixture;
        println!("scene {}x{} origin {} {}", scene.width_m, scene.depth_m, scene.origin_x, scene.origin_z);
        println!("level_m={:?} level_mm={level_mm}", base.level_m);
        println!("status={} edges={} nodes={} rejections={}", base.status, base.edges.len(), base.nodes.len(), base.base_profile_rejections.len());
        for r in &base.base_profile_rejections {
            println!("REJ edge={} cat={} reason={}", r.edge_id, r.category, r.reason);
        }
        for n in &base.nodes {
            println!("NODE id={} x={} z={}", n.id, n.world_x, n.world_z);
        }
        for e in &base.edges {
            let h = |x: i32, z: i32| {
                e.profile
                    .as_ref()
                    .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
            };
            let (mx, mz) = ((e.world_x), (e.world_z));
            let from = base.nodes.iter().find(|n| n.id == e.from).map(|n| (n.world_x, n.world_z));
            let to = base.nodes.iter().find(|n| n.id == e.to).map(|n| (n.world_x, n.world_z));
            println!(
                "EDGE {} from={:?} to={:?} c=({},{}) len={} rot={} profiled={} h_start={:?} h_mid={:?} h_end={:?}",
                e.id, from, to, mx, mz, e.length_m, e.rotation_y_deg,
                e.profile.is_some(),
                from.and_then(|(x, z)| h(x, z)),
                h(mx, mz),
                to.and_then(|(x, z)| h(x, z)),
            );
        }
        // what entities() reports for the base edges
        for ent in base.entities() {
            let p = ent.grounding.road_profile.as_ref();
            println!(
                "ENT {} w={} d={} y={} ground={} profiled={} h_at_center={:?}",
                ent.entity_id, ent.width_m, ent.depth_m, ent.world_y, ent.ground_height_m(),
                p.is_some(),
                p.and_then(|p| p.height_at(f64::from(ent.world_x), f64::from(ent.world_z))),
            );
        }
    }

    /// R2 W1-2 diagnostic (ignored): replay the graded corridor search and print
    /// the exact rejection each candidate hits, so the regression is attributed
    /// by observation instead of a guessed probe.
    #[test]
    #[ignore = "diagnostic: prints the rejection reason per graded corridor candidate"]
    fn w1_diagnose_graded_corridor_rejections() {
        let fixture = graded_fixture();
        let (scene, natural, water, zones, base, level_mm) = &fixture;
        let mut printed = 0;
        for node in &base.nodes {
            for (dx, dz) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                for length in [64_i32, 96, 48] {
                    let start = [node.world_x, node.world_z];
                    let end = [start[0] + dx * length, start[1] + dz * length];
                    if end[0] < 12
                        || end[1] < 12
                        || end[0] + 12 > scene.width_m as i32
                        || end[1] + 12 > scene.depth_m as i32
                    {
                        continue;
                    }
                    let candidate = serde_json::json!({
                        "roadExtensions": {"version": 1, "corridors": [{
                            "sceneId": "graded-audit", "start": start, "end": end,
                            "roadWidthM": 8, "shoulderWidthM": 3, "maxFillMm": 6000,
                            "maxRiseMmPerM": 80, "startLevelMm": *level_mm,
                            "endLevelMm": *level_mm, "junctionLengthM": 8
                        }]}
                    });
                    let mut probe = base.clone();
                    match super::super::road_extensions::append(
                        &mut probe,
                        scene,
                        natural,
                        *water,
                        zones,
                        &candidate,
                    ) {
                        Ok(()) => println!("ACCEPT start={start:?} end={end:?}"),
                        Err(e) => println!("REJECT start={start:?} end={end:?}: {}", e.0),
                    }
                    printed += 1;
                    if printed >= 40 {
                        return;
                    }
                }
            }
        }
    }

    /// A 16m collector feeding a 12m local road: the shared node is pinned to
    /// the base level, so the two corridors must agree there.
    fn collector_with_local_style(fixture: &GradedFixture) -> Option<serde_json::Value> {
        let (scene, natural, water, zones, base, level_mm) = fixture;
        let directions: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        // Deterministic bounded search: straight continuations first, then
        // L-shaped junctions, shortest first.
        let mut attempts = 0_u32;
        for node in &base.nodes {
            for (dx, dz) in directions {
                for (ex, ez) in directions {
                    if (dx + ex, dz + ez) == (0, 0) {
                        continue;
                    }
                    for (first, second) in [
                        (48_i32, 48_i32),
                        (64, 48),
                        (48, 64),
                        (64, 64),
                        (32, 48),
                        (48, 32),
                    ] {
                    if attempts >= 600 {
                        return None;
                    }
                    attempts += 1;
                    let start = [node.world_x, node.world_z];
                    let mid = [start[0] + dx * first, start[1] + dz * first];
                    let end = [mid[0] + ex * second, mid[1] + ez * second];
                    if start[0] < 12 || start[1] < 12
                        || mid[0] < 12 || mid[1] < 12
                        || end[0] < 12 || end[1] < 12
                        || end[0] + 12 > scene.width_m as i32
                        || end[1] + 12 > scene.depth_m as i32
                    {
                        continue;
                    }
                    let candidate = serde_json::json!({
                        "roadExtensions": {"version": 1, "corridors": [
                            {"sceneId": "graded-audit", "start": start, "end": mid,
                             "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                             "maxRiseMmPerM": 80, "startLevelMm": level_mm,
                             "endLevelMm": level_mm, "junctionLengthM": 8},
                            {"sceneId": "graded-audit", "start": mid, "end": end,
                             "roadWidthM": 12, "shoulderWidthM": 3, "maxFillMm": 6000,
                             "maxRiseMmPerM": 80, "startLevelMm": level_mm,
                             "endLevelMm": level_mm, "junctionLengthM": 8}
                        ]}
                    });
                    let mut probe = base.clone();
                    if super::super::road_extensions::append(
                        &mut probe,
                        scene,
                        natural,
                        *water,
                        zones,
                        &candidate,
                    )
                    .is_ok()
                    {
                        return Some(candidate);
                    }
                    }
                }
            }
        }
        None
    }

    /// Build the production final terrain (carve over the planned entities).
    fn carved_final(
        scene: &SceneSpec,
        natural: &BakedTerrain,
        entities: &[EntityInstance],
    ) -> BakedTerrain {
        use crate::height_payload::encode_millimetres;
        let carves = super::super::plan_terrain_carves(entities);
        let mut values = Vec::with_capacity((scene.width_m * scene.depth_m) as usize);
        for z in scene.origin_z..scene.origin_z + scene.depth_m as i32 {
            for x in scene.origin_x..scene.origin_x + scene.width_m as i32 {
                values.push(super::super::apply_terrain_carves(
                    f64::from(natural.height(x, z).unwrap()),
                    x,
                    z,
                    &carves,
                ));
            }
        }
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(
                scene.origin_x,
                scene.origin_z,
                scene.width_m,
                scene.depth_m,
                &encode_millimetres(scene.width_m, scene.depth_m, values).unwrap(),
            )
            .unwrap();
        terrain
    }

    /// The graded band of one profiled road as world-axis halves.
    fn band_of(entity: &EntityInstance) -> (i32, i32) {
        super::super::graded_flat_top_half_extents(entity)
    }

    /// Worst |final - design| inside a road's graded band.
    fn worst_band_error(
        entity: &EntityInstance,
        final_terrain: &BakedTerrain,
    ) -> f32 {
        let profile = entity.grounding.road_profile.as_ref().unwrap();
        let (band_x, band_z) = band_of(entity);
        let mut worst = 0_f32;
        for dz in -band_z..=band_z {
            for dx in -band_x..=band_x {
                let (x, z) = (entity.world_x + dx, entity.world_z + dz);
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                worst = worst.max((actual - target).abs());
            }
        }
        worst
    }

    #[test]
    fn graded_road_audit_accepts_legal_engineering_and_rejects_real_damage() {
        let fixture = graded_fixture();
        let style = single_corridor_style(&fixture, 12, 0)
            .expect("the control terrain admits a legal graded corridor");
        let (scene, natural, water, zones, _base, _level_mm) = fixture;
        let plan = plan_with_style(&scene, &natural, water, Some(&zones), None, &style).unwrap();
        assert_eq!(plan.extensions.len(), 1, "the control corridor must be built");
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        // Exactly the production final terrain: the carve over the plan.
        let final_terrain = carved_final(&scene, &natural, &entities);
        use crate::height_payload::encode_millimetres;
        let audit = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        assert_eq!(
            audit.status, "pass",
            "legal graded engineering must pass: {:?}",
            audit.failures
        );
        assert_eq!(audit.realized_edges, plan.edges.len());

        // W1 band rule: max(7, pavement/2 + 1) keeps the 8m and 12m values
        // bit-identical at 7m and gives a 16m collector 9m, so every contract
        // width covers its pavement and the structural guard stays silent.
        for (width, band) in [(8.0_f32, 7), (12.0, 7), (16.0, 9)] {
            let probe: EntityInstance = serde_json::from_value(serde_json::json!({
                "entityId": "probe", "assetId": "prop.road", "kind": "road",
                "worldX": 0, "worldZ": 0, "worldY": 0, "scale": 1.0,
                "widthM": 128.0, "depthM": width, "heightM": 0.2, "rotationYDeg": 0.0
            }))
            .unwrap();
            let (along, transverse) = super::super::graded_flat_top_half_extents(&probe);
            let (transport_along, transport_transverse) =
                super::super::graded_transport_half_extents(&probe);
            let (_, pavement_transverse) = super::super::graded_pavement_half_extents(&probe);
            assert_eq!(along, 68);
            assert_eq!(transport_along, 68);
            assert_eq!(transport_transverse, width as i32 / 2 + 3);
            assert_eq!(pavement_transverse, width as i32 / 2);
            assert_eq!(transverse, band, "graded band for a {width}m road");
            assert!(
                transverse >= pavement_transverse,
                "the graded band must cover the {width}m pavement"
            );
            assert_eq!(
                super::super::graded_band_shortfall_m(pavement_transverse, transverse),
                0
            );
        }
        // Narrow-band regression: the pre-W1 constant band (7m) under a 16m
        // pavement (8m half) is exactly one structural shortfall, so a road in
        // that state is refused instead of being silently counted as built.
        assert_eq!(super::super::graded_band_shortfall_m(8, 7), 1);
        assert_eq!(super::super::graded_band_shortfall_m(6, 7), 0);
        assert_eq!(super::super::graded_band_shortfall_m(4, 7), 0);
        assert_eq!(super::super::graded_band_shortfall_m(12, 7), 5);

        // The whole graded footprint, shoulder included, holds the design
        // level: that is what the independent audit requires.
        //
        // B01-T2 W1-2: this test's subject is the graded extension corridor it
        // just built. It used to find "the profiled road", which implicitly
        // worked while base roads carried no profile; now that every base edge
        // carries its own solver profile, the same predicate can land on the 8 m
        // base road instead (whose graded band equals its transport band, so the
        // shoulder-deviation assertion below cannot hold for it). Selecting the
        // built extension keeps the original intent and every assertion.
        let graded = entities
            .iter()
            .find(|entity| {
                entity.entity_id.starts_with("generated.road-extension.")
                    && entity.grounding.road_profile.is_some()
            })
            .expect("a profiled road");
        let profile = graded.grounding.road_profile.as_ref().unwrap();
        // The design level is required across the earthwork's graded band.
        let (along_half, transverse_half) = super::super::graded_flat_top_half_extents(graded);
        // A road at rotation 0 runs along x, so its transverse axis is z.
        let transverse_is_z = graded.rotation_y_deg != 90.;
        let mut worst = 0_f32;
        for dz in -transverse_half..=transverse_half {
            for dx in -along_half..=along_half {
                let (x, z) = if transverse_is_z {
                    (graded.world_x + dx, graded.world_z + dz)
                } else {
                    (graded.world_x + dz, graded.world_z + dx)
                };
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                worst = worst.max((actual - target).abs());
            }
        }
        assert!(
            worst <= 0.00005,
            "the graded footprint must hold the design level, worst {worst}"
        );

        // The transition band is judged by its own contract: it is blended and
        // never required to equal the design level, so the audit still passes
        // while that ring genuinely deviates from the design height.
        let transport_half = if transverse_is_z {
            super::super::graded_transport_half_extents(graded).1
        } else {
            super::super::graded_transport_half_extents(graded).0
        };
        assert!(transport_half > transverse_half);
        let mut worst_transition = 0_f32;
        for dz in -transport_half..=transport_half {
            if dz.abs() <= transverse_half {
                continue;
            }
            for dx in -along_half..=along_half {
                let (x, z) = if transverse_is_z {
                    (graded.world_x + dx, graded.world_z + dz)
                } else {
                    (graded.world_x + dz, graded.world_z + dx)
                };
                let target = profile.height_at(f64::from(x), f64::from(z)).unwrap() as f32;
                let actual = final_terrain.height(x, z).unwrap();
                let natural_h = natural.height(x, z).unwrap();
                assert!(
                    actual >= natural_h - 0.0005,
                    "the shoulder must never excavate: actual {actual}, natural {natural_h}"
                );
                assert!(
                    actual - natural_h <= 6.0 + 0.0005,
                    "the shoulder fill must stay inside the road's cap: actual {actual}, natural {natural_h}"
                );
                worst_transition = worst_transition.max((actual - target).abs());
            }
        }
        assert!(
            worst_transition > 0.001,
            "the shoulder must really deviate, got {worst_transition}"
        );

        let pavement_half = if transverse_is_z {
            super::super::graded_pavement_half_extents(graded).1
        } else {
            super::super::graded_pavement_half_extents(graded).0
        };
        let at = |transverse: i32, along: i32| {
            if transverse_is_z {
                (graded.world_x + along, graded.world_z + transverse)
            } else {
                (graded.world_x + transverse, graded.world_z + along)
            }
        };
        let perturb = |cell: (i32, i32), delta: f64| -> BakedTerrain {
            let mut terrain = BakedTerrain::new(&scene).unwrap();
            let mut values = Vec::with_capacity(512 * 512);
            for zz in 0..512 {
                for xx in 0..512 {
                    let base = f64::from(final_terrain.height(xx, zz).unwrap());
                    values.push(if (xx, zz) == cell { base + delta } else { base });
                }
            }
            terrain
                .insert_chunk(
                    0,
                    0,
                    512,
                    512,
                    &encode_millimetres(512, 512, values).unwrap(),
                )
                .unwrap();
            terrain
        };
        let audit_of = |terrain: &BakedTerrain| {
            verify_with_style(
                &plan,
                &scene,
                &natural,
                terrain,
                water,
                &entities,
                Some(&zones),
                None,
                &style,
            )
            .unwrap()
        };

        // Pavement edge +1mm: one design-error failure, and the failing road is
        // not counted as realized.
        let pavement_edge = audit_of(&perturb(at(pavement_half, 0), 0.001));
        assert_eq!(pavement_edge.status, "fail");
        assert_eq!(
            pavement_edge.failures.len(),
            1,
            "{:?}",
            pavement_edge.failures
        );
        assert!(pavement_edge.failures[0].contains("changed after planning"));
        assert_eq!(pavement_edge.realized_edges, plan.edges.len() - 1);

        // Graded-band edge +1mm: still inside the band the earthwork flattens,
        // so it is a design error even though it is outside the pavement.
        let band_edge = audit_of(&perturb(at(transverse_half, 0), 0.001));
        assert_eq!(band_edge.status, "fail");
        assert_eq!(band_edge.failures.len(), 1, "{:?}", band_edge.failures);
        assert!(band_edge.max_design_height_error_m > 0.0009);

        // Unauthorized excavation: lowering the pavement 0.2m below the natural
        // ground fails on the cut criterion (not merely as a design error).
        let excavated = {
            let cell = at(pavement_half, 0);
            let natural_m = f64::from(natural.height(cell.0, cell.1).unwrap());
            let final_m = f64::from(final_terrain.height(cell.0, cell.1).unwrap());
            audit_of(&perturb(cell, natural_m - final_m - 0.2))
        };
        assert_eq!(excavated.status, "fail");
        assert!(
            excavated.failures[0].contains("cut 0.2"),
            "{:?}",
            excavated.failures
        );

        // Over-fill: raising a transport-footprint cell above the road's own
        // fill cap fails on the fill criterion.
        let overfilled = {
            let cell = at(pavement_half, 0);
            let natural_mm = natural.height_millimetres(cell.0, cell.1).unwrap() as f64 / 1000.;
            audit_of(&perturb(cell, natural_mm + 6.5 - f64::from(
                final_terrain.height(cell.0, cell.1).unwrap(),
            )))
        };
        assert_eq!(overfilled.status, "fail");
        assert!(
            overfilled.max_final_roadbed_fill_m > 6.0,
            "fill {} must exceed the cap",
            overfilled.max_final_roadbed_fill_m
        );
    }

    /// B02-W1: 8m, 12m and 16m graded roads, each in two orthogonal directions,
    /// on non-flat natural ground, through production plan -> carve -> verify.
    #[test]
    fn graded_width_and_direction_matrix_is_supported() {
        let fixture = graded_fixture();
        let (scene, natural, water, zones, _base, _level_mm) = &fixture;
        let mut matrix = Vec::new();
        for width in [8_u32, 12, 16] {
            for rotation in [0_i32, 90] {
                let style = single_corridor_style(&fixture, width, rotation).unwrap_or_else(|| {
                    panic!("no legal {width}m corridor was found at rotation {rotation}")
                });
                let plan = plan_with_style(scene, natural, *water, Some(zones), None, &style)
                    .unwrap_or_else(|error| panic!("{width}m/{rotation}: {error}"));
                assert_eq!(plan.extensions.len(), 1, "{width}m/{rotation}");
                let mut entities = plan.entities();
                entities.extend(plan.site_entities());
                let final_terrain = carved_final(scene, natural, &entities);
                let audit = verify_with_style(
                    &plan,
                    scene,
                    natural,
                    &final_terrain,
                    *water,
                    &entities,
                    Some(zones),
                    None,
                    &style,
                )
                .unwrap_or_else(|error| panic!("{width}m/{rotation}: {error}"));
                assert_eq!(
                    audit.status, "pass",
                    "{width}m/{rotation} must pass: {:?}",
                    audit.failures
                );
                assert_eq!(audit.realized_edges, plan.edges.len());
                // Every profiled road's graded band covers its pavement, holds
                // the design level exactly, and keeps the shoulder as a fill.
                //
                // B01-T2 W1-2: the matrix is about the ONE extension corridor
                // built per (width, rotation) combination, so it must select that
                // extension rather than every profiled entity - base edges now
                // carry their own 8 m profiles and would both inflate the matrix
                // and violate the width-specific band assertions.
                for entity in entities
                    .iter()
                    .filter(|entity| {
                        entity.entity_id.starts_with("generated.road-extension.")
                            && entity.grounding.road_profile.is_some()
                    })
                {
                    let (band_x, band_z) = band_of(entity);
                    let (pavement_x, pavement_z) =
                        super::super::graded_pavement_half_extents(entity);
                    let transverse_band = if entity.rotation_y_deg == 90. {
                        band_x
                    } else {
                        band_z
                    };
                    let transverse_pavement = if entity.rotation_y_deg == 90. {
                        pavement_x
                    } else {
                        pavement_z
                    };
                    assert!(
                        transverse_band >= transverse_pavement,
                        "{width}m/{rotation}: band {transverse_band} < pavement {transverse_pavement}"
                    );
                    assert_eq!(transverse_band, (entity.depth_m as i32 / 2 + 1).max(7));
                    let worst = worst_band_error(entity, &final_terrain);
                    assert!(
                        worst <= 0.00005,
                        "{width}m/{rotation}: graded band deviation {worst}"
                    );
                    matrix.push((width, rotation, entity.depth_m as u32, transverse_band));
                }
            }
        }
        assert_eq!(matrix.len(), 6, "one profiled road per combination");
        assert!(matrix.iter().any(|(_, _, width, band)| *width == 16 && *band == 9));
        assert!(matrix.iter().all(|(_, _, width, band)| *band == (*width as i32 / 2 + 1).max(7)));
    }

    /// B02-W1: a real 16m collector feeding a 12m local road, both pinned to
    /// the base level, with the shared junction held to one elevation.
    #[test]
    fn wide_collector_with_local_connection_is_supported() {
        let fixture = graded_fixture();
        let style = collector_with_local_style(&fixture)
            .expect("the control terrain admits a 16m collector feeding a 12m local road");
        let (scene, natural, water, zones, _base, level_mm) = fixture;
        let plan = plan_with_style(&scene, &natural, water, Some(&zones), None, &style)
            .unwrap_or_else(|error| panic!("collector+local planning failed: {error}"));
        assert_eq!(plan.extensions.len(), 2, "collector + local must both be built");
        let widths: Vec<u32> = plan
            .extensions
            .iter()
            .map(|extension| extension.design.request.road_width_m)
            .collect();
        assert_eq!(widths, vec![16, 12]);
        // The junction is shared and pinned to the base level in both designs.
        let shared = plan.extensions[0].design.request.end;
        assert_eq!(plan.extensions[1].design.request.start, shared);
        for extension in &plan.extensions {
            let start_level = extension.design.request.start_level_mm.unwrap();
            let end_level = extension.design.request.end_level_mm.unwrap();
            assert_eq!(start_level, level_mm);
            assert_eq!(end_level, level_mm);
        }
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        let final_terrain = carved_final(&scene, &natural, &entities);
        let audit = verify_with_style(
            &plan,
            &scene,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        assert_eq!(audit.status, "pass", "{:?}", audit.failures);
        assert_eq!(audit.realized_edges, plan.edges.len());
        // The 16m pavement is fully supported inside its 9m graded band.
        let collector = entities
            .iter()
            .find(|entity| {
                entity
                    .grounding
                    .road_profile
                    .as_ref()
                    .is_some_and(|profile| profile.road_width_m == 16)
            })
            .expect("the collector entity");
        let collector_band = band_of(collector);
        let collector_transverse = if collector.rotation_y_deg == 90. {
            collector_band.0
        } else {
            collector_band.1
        };
        assert_eq!(collector_transverse, 9, "16m pavement needs a 9m graded band");
        assert!(worst_band_error(collector, &final_terrain) <= 0.00005);
        assert_eq!(
            collector
                .grounding
                .road_profile
                .as_ref()
                .unwrap()
                .height_at(
                    f64::from(shared[0]),
                    f64::from(shared[1])
                )
                .map(|value| (value * 1000.).round() as i32),
            Some(level_mm)
        );
    }

    /// Manual diagnostic (ignored): searches the published C1 compact terrain
    /// for a 16m collector feeding a 12m local road that production planning
    /// accepts, and audits the best candidate. Used once to author the W1 CLI
    /// control; it needs a historical published world, so it is not part of the
    /// normal suite. Run with `--ignored --nocapture`.
    /// G34-R5-B01 round 20. Determinism probe for the `quality-report` blocker.
    ///
    /// `quality-report` fails on EVERY world this round produced, including the 512 m baseline, with
    /// "road plan differs from natural raster reconstruction". That check (verify_impl, this file) rebuilds
    /// the plan from the published manifest plus the natural raster and requires EXACT serde equality with
    /// the published record. Everything else passes: validate-world binds the documents to the manifest,
    /// the zone check recomputes plan_landuse_zones and matches, and the generator's own in-pipeline
    /// audits pass.
    ///
    /// If road planning is not deterministic for identical inputs, that exact comparison can fail while
    /// both plans are individually valid, which would explain the whole observation. This probe plans the
    /// same scene TWICE from the same reconstructed inputs and compares the serialised plans, reporting
    /// the first differing JSON path when they differ.
    ///
    /// Ignored by default because it needs a produced world on disk. Run with:
    ///   cargo test -p glyphweave-core --lib b01_road_plan_is_deterministic -- --ignored --nocapture
    #[test]
    #[ignore = "needs bevy/work/collaboration/ds-b-mainline-resume-20260929/runs/b0-baseline-a02/world"]
    fn b01_road_plan_is_deterministic() {
        use crate::worldgen::WaterKind;
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = repo.join(
            "bevy/work/collaboration/ds-b-mainline-resume-20260929/runs/b0-baseline-a02/world",
        );
        if !root.is_dir() {
            eprintln!("SKIP missing {}", root.display());
            return;
        }
        let manifest: crate::worldgen::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).unwrap(),
        )
        .unwrap();
        let spec = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&spec.scene_id);
        let scene: crate::worldgen::SceneIndex =
            serde_json::from_slice(&std::fs::read(dir.join("scene.json")).unwrap()).unwrap();
        let mut natural = BakedTerrain::new(&spec).unwrap();
        for chunk in &scene.chunks {
            natural
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &std::fs::read(dir.join(chunk.natural_height_file.as_ref().unwrap())).unwrap(),
                )
                .unwrap();
        }
        natural.require_complete().unwrap();
        let water = super::super::water_geometry(WaterKind::None, &[], &spec, &manifest.style);
        let profile = crate::worldgen::LandUseProfile::from_style(&manifest.style).unwrap();
        let zones = crate::worldgen::plan_landuse_zones(&spec, &profile);
        let source = super::super::declared_natural_source(&manifest, &spec).unwrap();
        let first = plan_with_style_declared(
            &spec,
            &natural,
            water,
            Some(&zones),
            None,
            &manifest.style,
            source,
        )
        .unwrap();
        // quality-report builds the service programme from the effective style and passes it, and
        // plan_with_style_declared itself reads the programme from the style when none is supplied, so the
        // probe compares both arms explicitly.
        let program = crate::worldgen::services::from_style(&manifest.style).unwrap();
        eprintln!("programme built from the manifest style: {}", program.is_some());
        let with_program = plan_with_style_declared(
            &spec,
            &natural,
            water,
            Some(&zones),
            program.as_ref(),
            &manifest.style,
            source,
        )
        .unwrap();
        let d = serde_json::to_value(&with_program).unwrap();
        let second = plan_with_style_declared(
            &spec,
            &natural,
            water,
            Some(&zones),
            None,
            &manifest.style,
            source,
        )
        .unwrap();
        let a = serde_json::to_value(&first).unwrap();
        let b = serde_json::to_value(&second).unwrap();
        if a == b {
            eprintln!("DETERMINISTIC: two plans from identical inputs are byte-equal");
        } else {
            eprintln!("NON-DETERMINISTIC: two plans from identical inputs DIFFER");
        }
        // also compare against what was published, which is what quality-report does
        let published: crate::worldgen::road_network::RoadNetworkPlan = serde_json::from_slice(
            &std::fs::read(dir.join("road-network-plan.json")).unwrap(),
        )
        .unwrap();
        let c = serde_json::to_value(&published).unwrap();
        eprintln!(
            "first == second : {}\nfirst == published : {}",
            a == b,
            a == c
        );
        if a != c {
            report_first_json_difference(&a, &c, "$");
            // G34-R5-B01 round 29: turn the version-drift claim into an executable check.
            //
            // I had been asserting from file timestamps that every produced world predates the current
            // frontage source. Timestamps are fragile -- a later run into the same directory rewrites them
            // -- so this proves the drift from the artefacts themselves: the published plan's model name
            // differs from what the current code produces, and the difference is confined to the frontage
            // and roadside-site fields that the changed source owns.
            let published_model = c
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<none>")
                .to_string();
            let rebuilt_model = a
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<none>")
                .to_string();
            eprintln!("published model: {published_model}\nrebuilt   model: {rebuilt_model}");
            assert_ne!(
                published_model, rebuilt_model,
                "this world reproduces under the current code, so it does NOT predate the frontage change \
                 and the round 28 version-drift finding no longer applies to it"
            );
            // the drift must be attributable to frontage, not to something unrelated
            let mut frontage_paths = 0_u32;
            let mut other_paths = 0_u32;
            count_differing_paths(&a, &c, "$", &mut frontage_paths, &mut other_paths);
            eprintln!(
                "differing paths: frontage/roadsideSites {frontage_paths}, other {other_paths}"
            );
            assert!(
                frontage_paths > 0,
                "the rebuild must differ from the published plan somewhere in the frontage output"
            );
        } else {
            eprintln!(
                "NOTE: this world REPRODUCES under the current code, so the version-drift finding does not apply"
            );
        }
        assert_eq!(a, b, "road planning must be deterministic for identical inputs");
    }
    /// Count differing JSON paths, split into the frontage-owned part of the plan and everything else, so
    /// a rebuild difference can be attributed rather than merely reported.
    fn count_differing_paths(
        a: &serde_json::Value,
        b: &serde_json::Value,
        path: &str,
        frontage: &mut u32,
        other: &mut u32,
    ) {
        use serde_json::Value;
        let is_frontage = |p: &str| p.starts_with("$.frontage") || p.starts_with("$.roadsideSites");
        match (a, b) {
            (Value::Object(ma), Value::Object(mb)) => {
                for (k, va) in ma {
                    let child = format!("{path}.{k}");
                    match mb.get(k) {
                        Some(vb) => count_differing_paths(va, vb, &child, frontage, other),
                        None => {
                            if is_frontage(&child) {
                                *frontage += 1;
                            } else {
                                *other += 1;
                            }
                        }
                    }
                }
                for k in mb.keys() {
                    if !ma.contains_key(k) {
                        let child = format!("{path}.{k}");
                        if is_frontage(&child) {
                            *frontage += 1;
                        } else {
                            *other += 1;
                        }
                    }
                }
            }
            (Value::Array(va), Value::Array(vb)) => {
                if va.len() != vb.len() {
                    if is_frontage(path) {
                        *frontage += 1;
                    } else {
                        *other += 1;
                    }
                } else {
                    for (i, (x, y)) in va.iter().zip(vb.iter()).enumerate() {
                        count_differing_paths(x, y, &format!("{path}[{i}]"), frontage, other);
                    }
                }
            }
            _ => {
                if a != b {
                    if is_frontage(path) {
                        *frontage += 1;
                    } else {
                        *other += 1;
                    }
                }
            }
        }
    }
    /// Print the first differing JSON path between two values, so an opaque equality failure names the
    /// field that actually differs instead of reporting only that something does.
    fn report_first_json_difference(a: &serde_json::Value, b: &serde_json::Value, path: &str) {
        use serde_json::Value;
        match (a, b) {
            (Value::Object(ma), Value::Object(mb)) => {
                for (k, va) in ma {
                    match mb.get(k) {
                        Some(vb) => report_first_json_difference(va, vb, &format!("{path}.{k}")),
                        None => eprintln!("first difference: {path}.{k} present in A, absent in B"),
                    }
                }
                for k in mb.keys() {
                    if !ma.contains_key(k) {
                        eprintln!("first difference: {path}.{k} absent in A, present in B");
                    }
                }
            }
            (Value::Array(va), Value::Array(vb)) => {
                if va.len() != vb.len() {
                    eprintln!(
                        "first difference: {path} length {} vs {}",
                        va.len(),
                        vb.len()
                    );
                } else {
                    for (i, (x, y)) in va.iter().zip(vb.iter()).enumerate() {
                        report_first_json_difference(x, y, &format!("{path}[{i}]"));
                    }
                }
            }
            _ => {
                if a != b {
                    eprintln!("first difference: {path} = {a} vs {b}");
                }
            }
        }
    }
    #[test]
    #[ignore = "needs bevy/work/collaboration/ds-b04-c1-r1/gen-compact-mixed-block; prints W1 corridor candidates"]
    fn w1_cli_control_corridor_search() {
        use crate::height_payload::encode_millimetres;
        use crate::worldgen::WaterKind;
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = repo.join("bevy/work/collaboration/ds-b04-c1-r1/gen-compact-mixed-block");
        if !root.is_dir() {
            eprintln!("SKIP missing {}", root.display());
            return;
        }
        let manifest: crate::worldgen::WorldManifest = serde_json::from_slice(
            &std::fs::read(root.join("glyphweave.manifest.json")).unwrap(),
        )
        .unwrap();
        let spec = manifest.scenes[0].clone();
        let dir = root.join("scenes").join(&spec.scene_id);
        let scene: crate::worldgen::SceneIndex =
            serde_json::from_slice(&std::fs::read(dir.join("scene.json")).unwrap()).unwrap();
        let mut natural = BakedTerrain::new(&spec).unwrap();
        for chunk in &scene.chunks {
            natural
                .insert_chunk(
                    chunk.world_x,
                    chunk.world_z,
                    chunk.valid_width_m,
                    chunk.valid_depth_m,
                    &std::fs::read(dir.join(chunk.natural_height_file.as_ref().unwrap()))
                        .unwrap(),
                )
                .unwrap();
        }
        let water =
            super::super::water_geometry(WaterKind::None, &[], &spec, &manifest.style);
        let profile = crate::worldgen::LandUseProfile::from_style(&manifest.style).unwrap();
        let zones = crate::worldgen::plan_landuse_zones(&spec, &profile);
        let base = plan_with_zones(&spec, &natural, water, Some(&zones)).unwrap();
        let level_mm = base.level_m.unwrap() * 1000;
        let alignment =
            super::super::road_alignment::AlignmentContext::new(&spec, &natural, water, &zones)
                .unwrap();
        let solve_end = |start: [i32; 2], end: [i32; 2], width: u32, start_level: i32| {
            let request = super::super::road_alignment::CorridorRequest {
                scene_id: spec.scene_id.clone(),
                start,
                end,
                road_width_m: width,
                shoulder_width_m: 3,
                max_fill_mm: 6000,
                max_rise_mm_per_m: 80,
                start_level_mm: Some(start_level),
                end_level_mm: None,
                junction_length_m: 8,
            };
            alignment.design(&request).ok().map(|design| {
                let solved = *design.profile_mm.last().unwrap() as i32;
                let mut levels = vec![solved];
                for delta in [-3000, -2000, -1000, -500, 500, 1000, 2000, 3000] {
                    levels.push(solved + delta);
                }
                levels
            })
        };
        let try_append = |corridors: Vec<serde_json::Value>| -> bool {
            let style = serde_json::json!({
                "roadExtensions": {"version": 1, "corridors": corridors}
            });
            let mut probe = base.clone();
            super::super::road_extensions::append(
                &mut probe,
                &spec,
                &natural,
                water,
                &zones,
                &style,
            )
            .is_ok()
        };
        let mut found = Vec::new();
        let mut attempts = 0_u32;
        'search: for node in &base.nodes {
            for (dx, dz) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                for (ex, ez) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                    if (dx + ex, dz + ez) == (0, 0) {
                        continue;
                    }
                    for (first, second) in [
                        (48_i32, 48_i32),
                        (64, 48),
                        (48, 64),
                        (32, 48),
                        (64, 96),
                        (96, 64),
                    ] {
                        if attempts >= 4000 {
                            break 'search;
                        }
                        attempts += 1;
                        let start = [node.world_x, node.world_z];
                        let mid = [start[0] + dx * first, start[1] + dz * first];
                        let end = [mid[0] + ex * second, mid[1] + ez * second];
                        if [start, mid, end].iter().any(|point| {
                            point[0] < 12
                                || point[1] < 12
                                || point[0] + 12 > spec.width_m as i32
                                || point[1] + 12 > spec.depth_m as i32
                        }) {
                            continue;
                        }
                        let Some(first_levels) = solve_end(start, mid, 16, level_mm) else {
                            continue;
                        };
                        for first_level in first_levels {
                            let Some(second_levels) = solve_end(mid, end, 12, first_level) else {
                                continue;
                            };
                            for second_level in second_levels {
                                let corridors = vec![
                                    serde_json::json!({
                                        "sceneId": spec.scene_id, "start": start, "end": mid,
                                        "roadWidthM": 16, "shoulderWidthM": 3,
                                        "maxFillMm": 6000, "maxRiseMmPerM": 80,
                                        "startLevelMm": level_mm, "endLevelMm": first_level,
                                        "junctionLengthM": 8
                                    }),
                                    serde_json::json!({
                                        "sceneId": spec.scene_id, "start": mid, "end": end,
                                        "roadWidthM": 12, "shoulderWidthM": 3,
                                        "maxFillMm": 6000, "maxRiseMmPerM": 80,
                                        "startLevelMm": first_level, "endLevelMm": second_level,
                                        "junctionLengthM": 8
                                    }),
                                ];
                                if try_append(corridors.clone()) {
                                    found.push((
                                        start,
                                        mid,
                                        end,
                                        first,
                                        second,
                                        level_mm,
                                        first_level,
                                        second_level,
                                    ));
                                    break;
                                }
                            }
                            if found.len() >= 6 {
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        eprintln!("W1 attempts={attempts} candidates={found:?}");
        let Some((start, mid, end, first, second, start_level, first_level, second_level)) =
            found.first().copied()
        else {
            eprintln!("W1 no candidate found");
            return;
        };
        let style = serde_json::json!({
            "roadExtensions": {"version": 1, "corridors": [
                {"sceneId": spec.scene_id, "start": start, "end": mid,
                 "roadWidthM": 16, "shoulderWidthM": 3, "maxFillMm": 6000,
                 "maxRiseMmPerM": 80, "startLevelMm": start_level,
                 "endLevelMm": first_level, "junctionLengthM": 8},
                {"sceneId": spec.scene_id, "start": mid, "end": end,
                 "roadWidthM": 12, "shoulderWidthM": 3, "maxFillMm": 6000,
                 "maxRiseMmPerM": 80, "startLevelMm": first_level,
                 "endLevelMm": second_level, "junctionLengthM": 8}
            ]}
        });
        let plan = plan_with_style(&spec, &natural, water, Some(&zones), None, &style).unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        let carves = super::super::plan_terrain_carves(&entities);
        let mut values = Vec::with_capacity((spec.width_m * spec.depth_m) as usize);
        for z in spec.origin_z..spec.origin_z + spec.depth_m as i32 {
            for x in spec.origin_x..spec.origin_x + spec.width_m as i32 {
                values.push(super::super::apply_terrain_carves(
                    f64::from(natural.height(x, z).unwrap()),
                    x,
                    z,
                    &carves,
                ));
            }
        }
        let mut final_terrain = BakedTerrain::new(&spec).unwrap();
        final_terrain
            .insert_chunk(
                spec.origin_x,
                spec.origin_z,
                spec.width_m,
                spec.depth_m,
                &encode_millimetres(spec.width_m, spec.depth_m, values).unwrap(),
            )
            .unwrap();
        let audit = verify_with_style(
            &plan,
            &spec,
            &natural,
            &final_terrain,
            water,
            &entities,
            Some(&zones),
            None,
            &style,
        )
        .unwrap();
        eprintln!(
            "W1 best start={start:?} mid={mid:?} end={end:?} lengths=({first},{second}) levels=({start_level},{first_level},{second_level}) audit={:?} realized={}/{}",
            audit.status, audit.realized_edges, audit.planned_edges
        );
        for failure in &audit.failures {
            eprintln!("W1 FAILURE {failure}");
        }
        eprintln!(
            "W1 manifest corridors: {}",
            serde_json::to_string(&style["roadExtensions"]["corridors"]).unwrap()
        );
    }

    /// A corridor whose fixed start level does not match its parent node is
    /// refused by production planning, not silently re-levelled.
    #[test]
    fn fixed_node_level_mismatch_is_refused_by_planning() {
        let fixture = graded_fixture();
        let (scene, natural, water, zones, base, level_mm) = &fixture;
        let node = base.nodes.first().expect("a base node");
        let mut style = serde_json::json!({
            "roadExtensions": {"version": 1, "corridors": [{
                "sceneId": "graded-audit",
                "start": [node.world_x, node.world_z],
                "end": [node.world_x + 64, node.world_z],
                "roadWidthM": 16,
                "shoulderWidthM": 3,
                "maxFillMm": 6000,
                "maxRiseMmPerM": 80,
                "startLevelMm": level_mm + 250,
                "endLevelMm": level_mm + 250,
                "junctionLengthM": 8
            }]}
        });
        let error = plan_with_style(scene, natural, *water, Some(zones), None, &style)
            .expect_err("a mismatched fixed junction must be refused");
        assert!(
            error.to_string().contains("start junction height mismatch")
                || error.to_string().contains("corridor infeasible"),
            "{error}"
        );
        // The same corridor at the node's own level is acceptable, proving the
        // refusal is about the mismatch and not about the geometry.
        style["roadExtensions"]["corridors"][0]["startLevelMm"] = serde_json::json!(level_mm);
        style["roadExtensions"]["corridors"][0]["endLevelMm"] = serde_json::json!(level_mm);
        let _ = plan_with_style(scene, natural, *water, Some(zones), None, &style);
    }

    // ============ B01-T2 R2 W1-3 / W1-4 / W1-5 / W4 offline fixtures ============

    fn flat_512_scene() -> (SceneSpec, BakedTerrain, WaterGeometry, Vec<LandUseZone>) {
        use crate::height_payload::encode_millimetres;
        let scene = SceneSpec {
            scene_id: "v1v2".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &encode_millimetres(512, 512, std::iter::repeat_n(0., 512 * 512)).unwrap(),
            )
            .unwrap();
        let water = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &scene,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "v1v2.rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        (scene, terrain, water, zones)
    }

    /// A stub base plan with exactly one 8 m base road whose profile holds the
    /// 1000 mm tie-in level for `plateau_samples` stations from the `from` node
    /// and then rises 80 mm/m away from it. This models an existing road whose
    /// pinned plateau is shorter than its nominal platform.
    fn stub_plan(start: [i32; 2], end: [i32; 2], plateau_samples: usize) -> RoadNetworkPlan {
        let length = (i64::from(end[0]) - i64::from(start[0])).abs()
            + (i64::from(end[1]) - i64::from(start[1])).abs();
        let mut elevations_mm = Vec::new();
        for i in 0..=length {
            let d = i - plateau_samples as i64;
            elevations_mm.push(if d <= 0 { 1000 } else { 1000 + 80 * d });
        }
        let profile = super::super::road_profile::RoadProfile {
            version: 1,
            road_width_m: 8,
            start,
            end,
            elevations_mm,
            max_fill_mm: 6000,
        };
        let midpoint = [
            ((i64::from(start[0]) + i64::from(end[0])) / 2) as i32,
            ((i64::from(start[1]) + i64::from(end[1])) / 2) as i32,
        ];
        let rotation = if start[1] == end[1] { 0. } else { 90. };
        let mut plan: RoadNetworkPlan = serde_json::from_value(serde_json::json!({
            "model": "v1v2-stub",
            "naturalHeightfieldHash": "stub",
            "sceneId": "v1v2",
            "status": "planned_connected_network",
            "gridSpacingM": 64,
            "maxRoadbedFillM": 6,
            "levelM": 1,
            "candidates": 1,
            "rejectedWater": 0,
            "rejectedRelief": 0,
            "unselectedCandidates": 0,
            "nodes": [
                {"id": 0, "worldX": start[0], "worldZ": start[1]},
                {"id": 1, "worldX": end[0], "worldZ": end[1]}
            ],
            "edges": [{
                "id": "generated.road-network.v1v2.0-1",
                "from": 0, "to": 1,
                "worldX": midpoint[0], "worldZ": midpoint[1],
                "lengthM": length, "rotationYDeg": rotation,
                "naturalMinM": 0.0, "naturalMaxM": 0.0
            }],
            "roadsideSites": []
        }))
        .unwrap();
        plan.edges[0].profile = Some(profile);
        plan
    }

    fn stub_request(
        start: [i32; 2],
        end: [i32; 2],
        width: u32,
    ) -> super::super::road_alignment::CorridorRequest {
        super::super::road_alignment::CorridorRequest {
            scene_id: "v1v2".into(),
            start,
            end,
            road_width_m: width,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(1000),
            end_level_mm: Some(1000),
            junction_length_m: 8,
        }
    }

    /// V2-1/V2-2: the base tie-in platform is DERIVED from the extension
    /// contract, and covers every width that contract admits.
    #[test]
    fn v2_base_platform_is_derived_from_the_extension_contract() {
        assert_eq!(
            super::super::road_extensions::max_junction_occupancy_m(),
            11
        );
        assert_eq!(super::base_required_tie_in_platform_m(), 12);
        assert_eq!(super::base_tie_in_platform_m(64), 12);
        assert!(!super::base_platform_is_short(64));
        assert_eq!(super::base_tie_in_platform_m(20), 10);
        assert!(super::base_platform_is_short(20));
        for width in super::super::road_extensions::EXTENSION_ROAD_WIDTHS_M {
            assert!(
                super::base_required_tie_in_platform_m() as i32
                    >= super::super::road_extensions::junction_occupancy_m(width),
                "the derived platform must cover the occupancy of a {width} m road"
            );
        }
    }

    /// V1-2 negative + positive: inside a junction the two surfaces must agree at
    /// every overlapping cell, so an extension whose footprint reaches past the
    /// existing road's pinned plateau is refused; the same corridor against a
    /// 12 m plateau is accepted.
    #[test]
    fn v1_junction_overlap_requires_cell_continuity() {
        let (scene, terrain, water, zones) = flat_512_scene();
        let context =
            super::super::road_alignment::AlignmentContext::new(&scene, &terrain, water, &zones)
                .unwrap();
        // Negative: the base plateau is only 4 stations, so a 16 m extension
        // (occupancy 11 <= the nominal 12 m platform, so the named short-platform
        // rule does NOT fire) is refused by the per-cell continuity rule.
        let mut short_plateau = stub_plan([96, 128], [160, 128], 4);
        let error = super::super::road_extensions::append_one(
            &mut short_plateau,
            &context,
            &stub_request([96, 128], [96, 64], 16),
            "stub.short-plateau".into(),
        )
        .unwrap_err();
        assert!(
            error.0.contains("overlaps road outside a matching endpoint junction"),
            "{}",
            error.0
        );
        assert!(
            !error.0.contains("base_platform_short"),
            "the per-cell rule, not the short-platform rule, must decide this case: {}",
            error.0
        );
        // Positive: the same geometry against a 12 m plateau is accepted.
        let mut full_plateau = stub_plan([96, 128], [160, 128], 12);
        super::super::road_extensions::append_one(
            &mut full_plateau,
            &context,
            &stub_request([96, 128], [96, 64], 16),
            "stub.full-plateau".into(),
        )
        .unwrap();
    }

    /// V2-3: a base edge too short for the full platform refuses a wide extension
    /// BY NAME, and still accepts one that fits.
    #[test]
    fn v2_short_base_platform_refuses_a_wide_extension_by_name() {
        let (scene, terrain, water, zones) = flat_512_scene();
        let context =
            super::super::road_alignment::AlignmentContext::new(&scene, &terrain, water, &zones)
                .unwrap();
        // A 20 m base edge offers only a 10 m tie-in platform.
        let mut plan = stub_plan([96, 128], [116, 128], 10);
        let error = super::super::road_extensions::append_one(
            &mut plan.clone(),
            &context,
            &stub_request([96, 128], [96, 64], 16),
            "stub.too-wide".into(),
        )
        .unwrap_err();
        assert!(
            error.0.contains("base_platform_short_for_extension_width"),
            "{}",
            error.0
        );
        // An 8 m extension (occupancy 7 <= 10) is still welcome there.
        super::super::road_extensions::append_one(
            &mut plan,
            &context,
            &stub_request([96, 128], [96, 64], 8),
            "stub.narrow".into(),
        )
        .unwrap();
    }

    /// V1-5 / C4 point 2: a plan that arrives carrying a FORGED profile never runs
    /// through `base_edge_profile` (the profile is already `Some`), so the F4
    /// block is the only thing that can clear it.
    #[test]
    fn f4_pre_existing_forged_profile_is_cleared_with_a_named_refusal() {
        let (scene, terrain, water, zones) = flat_512_scene();
        let mut plan = stub_plan([96, 128], [160, 128], 12);
        // Forge it: the profile claims a different segment than the edge.
        {
            let profile = plan.edges[0].profile.as_mut().unwrap();
            profile.start = [32, 32];
            profile.end = [96, 32];
        }
        let nodes = plan.nodes.clone();
        let refusals = super::attach_base_edge_profiles(
            &mut plan,
            &scene,
            &terrain,
            water,
            &nodes,
            1,
            Some(&zones),
            // No bounded-cut policy in this fixture: the profile stays fill-only.
            0,
        )
        .unwrap();
        assert!(
            refusals
                .iter()
                .any(|r| r.category == "profile_entity_mismatch"),
            "{refusals:?}"
        );
        assert!(
            plan.edges[0].profile.is_none(),
            "a forged pre-existing profile must not survive F4"
        );
    }

    fn graded_full_plan() -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
    ) {
        let (scene, natural, water, zones, base, _level) = graded_fixture();
        (scene, natural, water, zones, base)
    }

    /// B01-T2 R3 V5: the 1 km frozen-input STAGE REPLAY.
    ///
    /// It reads the frozen 1 km manifest and runs the production chain in
    /// process: natural terrain (the same pure function the bake uses, but in
    /// memory) -> land-use zoning -> road plan with base-edge profiles and their
    /// refusals -> extensions/automatic roads -> frontage stations and candidate
    /// sites -> the development plan's homes/shops. It writes NO world product,
    /// never calls the CLI and never produces a gemap/sidecar: its only output is
    /// `r3/1km-stage-replay.json` (plus an optional digest for the two-run
    /// byte-identity check). Evidence level: offline-stage-replay.
    #[test]
    #[ignore = "R3 V5 offline stage replay of the frozen 1 km input; writes only r3/1km-stage-replay.json"]
    fn v5_1km_stage_replay() {
        use crate::height_payload::encode_millimetres;
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let manifest_path = std::env::var_os("GLYPHWEAVE_1KM_MANIFEST")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                repo.join(
                    "bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r2/1km-candidate/frozen-input/b01-1km-probe.manifest.json",
                )
            });
        let manifest_bytes = std::fs::read(&manifest_path).unwrap();
        let manifest: super::super::WorldManifest =
            serde_json::from_slice(&manifest_bytes).unwrap();
        let scene = &manifest.scenes[0];
        let style = &manifest.style;
        let seed = manifest.world.seed ^ scene.seed_offset;
        let water = super::super::water_geometry(
            super::super::water_kind(style),
            &manifest.landmarks,
            scene,
            style,
        );
        // The natural raster is a pure function of (seed, scene, water); build it
        // in memory with the SAME height function the bake uses.
        let mut terrain = BakedTerrain::new(scene).unwrap();
        let chunk = 512_u32;
        for chunk_z in 0..scene.depth_m.div_ceil(chunk) {
            for chunk_x in 0..scene.width_m.div_ceil(chunk) {
                let dx = chunk_x * chunk;
                let dz = chunk_z * chunk;
                let width = (scene.width_m - dx).min(chunk);
                let depth = (scene.depth_m - dz).min(chunk);
                let x = scene.origin_x + dx as i32;
                let z = scene.origin_z + dz as i32;
                let bytes = encode_millimetres(
                    width,
                    depth,
                    (0..depth).flat_map(|row| {
                        (0..width).map(move |col| {
                            super::super::terrain_height_metres(
                                seed,
                                x + col as i32,
                                z + row as i32,
                                water,
                                &[],
                            )
                        })
                    }),
                )
                .unwrap();
                terrain.insert_chunk(x, z, width, depth, &bytes).unwrap();
            }
        }
        terrain.require_complete().unwrap();
        let zones = super::super::LandUseProfile::from_style(style)
            .map(|profile| super::super::plan_landuse_zones(scene, &profile));
        let program = super::super::services::from_style(style).unwrap();
        let source = super::super::declared_natural_source(&manifest, scene).unwrap();
        let plan = super::plan_with_style_declared(
            scene,
            &terrain,
            water,
            zones.as_deref(),
            program.as_ref(),
            style,
            source,
        )
        .unwrap();

        let request = {
            let mut r = frontage_request(&scene.scene_id);
            r.width_m = 24;
            r.depth_m = 24;
            r.setback_m = 8;
            r
        };
        let entities = plan.entities();
        let mut stations_total = 0_usize;
        let mut candidate_sites = 0_usize;
        let mut per_road: Vec<serde_json::Value> = Vec::new();
        for entity in &entities {
            let Some(profile) = entity.grounding.road_profile.as_ref() else {
                continue;
            };
            let stations = super::super::frontage::stations(entity, &request)
                .unwrap()
                .unwrap_or_default();
            stations_total += stations.len();
            let mut accepted = 0_usize;
            let mut first_rejection: Option<String> = None;
            for &station in &stations {
                for side in [-1_i32, 1] {
                    match super::super::frontage::candidate_site(
                        entity,
                        &request,
                        &scene.scene_id,
                        station,
                        side,
                    ) {
                        Ok(_) => accepted += 1,
                        Err(error) => {
                            if first_rejection.is_none() {
                                first_rejection = Some(error.0);
                            }
                        }
                    }
                }
            }
            candidate_sites += accepted;
            per_road.push(serde_json::json!({
                "roadId": entity.entity_id,
                "roadWidthM": profile.road_width_m,
                "stations": stations.len(),
                "candidateSites": accepted,
                "firstRejection": first_rejection,
            }));
        }
        let outcomes = plan
            .frontage
            .as_ref()
            .map(|report| {
                report
                    .outcomes
                    .iter()
                    .map(|outcome| {
                        serde_json::json!({
                            "requestId": outcome.request.id,
                            "demand": outcome.request.count,
                            "realized": outcome.site_ids.len(),
                            "attempted": outcome.attempted,
                            "unfilled": outcome.unfilled,
                            "stopReason": outcome.stop_reason,
                            "rejectedByReason": outcome.rejected,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let development = zones.as_ref().and_then(|zones| {
            crate::settlement::development::plan(scene, zones, &plan)
                .ok()
                .map(|plan| serde_json::to_value(&plan).unwrap())
        });
        // B01-T2 R4 R-3: carry the replay past frontage into the development
        // audit, the yard/surface realization and (where the production input is
        // reachable in process) the land-use ledger. Everything here is in
        // memory; no world product is written.
        let mut stage_audit: serde_json::Value = serde_json::json!({ "status": "notObserved" });
        if let Some(zones) = zones.as_ref() {
            let carves = super::super::plan_terrain_carves(&entities);
            let mut final_terrain = BakedTerrain::new(scene).unwrap();
            let carve_chunk = 512_u32;
            for chunk_z in 0..scene.depth_m.div_ceil(carve_chunk) {
                for chunk_x in 0..scene.width_m.div_ceil(carve_chunk) {
                    let dx = chunk_x * carve_chunk;
                    let dz = chunk_z * carve_chunk;
                    let width = (scene.width_m - dx).min(carve_chunk);
                    let depth = (scene.depth_m - dz).min(carve_chunk);
                    let x = scene.origin_x + dx as i32;
                    let z = scene.origin_z + dz as i32;
                    let mut values = Vec::with_capacity((width * depth) as usize);
                    for row in 0..depth {
                        for col in 0..width {
                            let xx = x + col as i32;
                            let zz = z + row as i32;
                            values.push(super::super::apply_terrain_carves(
                                f64::from(terrain.height(xx, zz).unwrap()),
                                xx,
                                zz,
                                &carves,
                            ));
                        }
                    }
                    final_terrain
                        .insert_chunk(
                            x,
                            z,
                            width,
                            depth,
                            &encode_millimetres(width, depth, values).unwrap(),
                        )
                        .unwrap();
                }
            }
            let access = super::super::build_access_graph(&entities, &final_terrain, water);
            match crate::settlement::development::plan(scene, zones, &plan) {
                Ok(development_plan) => {
                    let audit = crate::settlement::development::verify(
                        &development_plan,
                        scene,
                        zones,
                        &plan,
                        &entities,
                        &access,
                        &[],
                    );
                    let landcover_plan =
                        crate::settlement::development::plan_landcover(scene, &development_plan);
                    let mut yard_unresolved_m2 = serde_json::json!("notObserved");
                    let mut surface_status = serde_json::json!("notObserved");
                    let mut surface_failures: Vec<String> = Vec::new();
                    let mut surface_unresolved_yard_m2: serde_json::Value =
                        serde_json::json!("notObserved");
                    match (audit, landcover_plan) {
                        (Ok(audit), Ok(landcover_plan)) => {
                            if let Ok(surfaces) = crate::settlement::landcover::realize(
                                scene,
                                &development_plan,
                                &landcover_plan,
                                &final_terrain,
                                water,
                                &entities,
                            ) {
                                if let Ok(surface_audit) = crate::settlement::landcover::verify(
                                    &surfaces,
                                    scene,
                                    &development_plan,
                                    &landcover_plan,
                                    &final_terrain,
                                    water,
                                    &entities,
                                ) {
                                    surface_status =
                                        serde_json::json!(surface_audit.status.clone());
                                    surface_failures = surface_audit.failures.clone();
                                    surface_unresolved_yard_m2 =
                                        serde_json::json!(surface_audit.unresolved_yard_m2);
                                }
                            }
                            yard_unresolved_m2 = surface_unresolved_yard_m2.clone();
                            stage_audit = serde_json::json!({
                                "status": audit.status,
                                "realizedBuildings": audit.realized_buildings,
                                "plannedBuildings": audit.planned_buildings,
                                "failures": audit.failures,
                                "incompleteReasons": audit.incomplete_reasons,
                                "yardUnresolvedM2": yard_unresolved_m2,
                                "surfaceStatus": surface_status,
                                "surfaceFailures": surface_failures,
                            });
                        }
                        (audit, landcover_plan) => {
                            stage_audit = serde_json::json!({
                                "status": "notObserved",
                                "coverageGap": "development audit or landcover plan could not be built in process",
                                "auditError": audit.err(),
                                "landcoverPlanError": landcover_plan.err(),
                            });
                        }
                    }
                }
                Err(error) => {
                    stage_audit = serde_json::json!({
                        "status": "notObserved",
                        "coverageGap": "the development plan could not be built in process",
                        "error": error.to_string(),
                    });
                }
            }
        }
        // The land-use ledger is produced by `landuse_ledger::analyze(&SceneIndex,
        // ..)`, whose input is the post-bake scene index that only the CLI
        // generation path assembles. It is therefore NOT replayable in this
        // evidence level and is reported as notObserved with the reason (R-3-2).
        let ledger = serde_json::json!({
            "status": "notObserved",
            "coverageGap": "landuse_ledger::analyze requires the post-bake SceneIndex, which only the CLI generation path builds; not reachable from an offline in-process replay",
        });
        let buildings_by_kind = development
            .as_ref()
            .and_then(|value| value["buildings"].as_array())
            .map(|buildings| {
                let mut counts = std::collections::BTreeMap::<String, usize>::new();
                for building in buildings {
                    let kind = building["kind"]
                        .as_str()
                        .or_else(|| building["assetId"].as_str())
                        .or_else(|| building["asset_id"].as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    *counts.entry(kind).or_default() += 1;
                }
                counts
            })
            .unwrap_or_default();
        // Which roads actually carry the realized frontage sites, and how many of
        // them are BASE roads (the R3/V6 criterion needs this).
        // "Base" means planning provenance (not an extension member and not a
        // road this plan already reported as automatic), the same rule C3 uses -
        // NOT merely "has an edge in the plan".
        let non_base_ids: std::collections::BTreeSet<&str> = plan
            .extensions
            .iter()
            .map(|extension| extension.entity_id.as_str())
            .chain(
                plan.automatic_extensions
                    .as_ref()
                    .map(|report| {
                        report
                            .generated_road_ids
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )
            .collect();
        let mut frontage_road_ids: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for outcome in plan
            .frontage
            .as_ref()
            .map(|report| report.outcomes.as_slice())
            .unwrap_or_default()
        {
            for site_id in &outcome.site_ids {
                if let Some(site) = plan
                    .roadside_sites
                    .iter()
                    .find(|site| site.id == *site_id)
                {
                    frontage_road_ids.insert(site.road_id.clone());
                }
            }
        }
        let frontage_roads_that_are_base: Vec<String> = frontage_road_ids
            .iter()
            .filter(|id| {
                plan.edges.iter().any(|edge| edge.id == **id)
                    && !non_base_ids.contains(id.as_str())
            })
            .cloned()
            .collect();
        let rejections: Vec<serde_json::Value> = plan
            .base_profile_rejections
            .iter()
            .map(|rejection| {
                serde_json::json!({
                    "edgeId": rejection.edge_id,
                    "category": rejection.category,
                    "reason": rejection.reason,
                })
            })
            .collect();
        let mut by_category = std::collections::BTreeMap::<String, usize>::new();
        for rejection in &plan.base_profile_rejections {
            *by_category.entry(rejection.category.clone()).or_default() += 1;
        }
        let report = serde_json::json!({
            "schema": "b01t2c-r3-1km-stage-replay-v1",
            "evidenceLevel": "offline-stage-replay",
            "notAWorldMeasurement": "this is NOT a production-world result and must not be used to claim city-capacity growth",
            "inputManifest": manifest_path.display().to_string(),
            "inputSha256": blake3::hash(&manifest_bytes).to_hex().to_string(),
            "sceneId": scene.scene_id,
            "sceneSizeM": [scene.width_m, scene.depth_m],
            "seed": seed,
            "planModel": plan.model,
            "planStatus": plan.status,
            "levelM": plan.level_m,
            "roadEdgesTotal": plan.edges.len(),
            "baseProfileRoads": plan.edges.iter().filter(|e| e.profile.is_some()).count(),
            "extensionRoads": plan.extensions.len(),
            "automaticRoads": plan
                .automatic_extensions
                .as_ref()
                .map_or(0, |report| report.generated_road_ids.len()),
            "baseProfileRejections": rejections,
            "baseProfileRejectionsByCategory": by_category,
            "roadsideSites": plan.roadside_sites.len(),
            "frontageOutcomes": outcomes,
            "frontageRoadIds": frontage_road_ids.iter().cloned().collect::<Vec<_>>(),
            "frontageRoadsThatAreBase": frontage_roads_that_are_base,
            "stationsTotal": stations_total,
            "candidateSitesTotal": candidate_sites,
            "perRoad": per_road,
            "developmentPlanRecords": development
                .as_ref()
                .and_then(|value| value["buildings"].as_array())
                .map_or(0, |buildings| buildings.len()),
            "developmentBuildingsByKind": buildings_by_kind,
            "developmentAudit": stage_audit,
            "landUseLedger": ledger,
        });
        let text = serde_json::to_string_pretty(&report).unwrap();
        if let Some(path) = std::env::var_os("GLYPHWEAVE_1KM_REPLAY_OUT") {
            std::fs::write(path, format!("{text}\n")).unwrap();
        }
        if let Some(path) = std::env::var_os("GLYPHWEAVE_1KM_REPLAY_DIGEST") {
            std::fs::write(path, format!("{}\n", blake3::hash(text.as_bytes()).to_hex())).unwrap();
        }
        if std::env::var_os("GLYPHWEAVE_1KM_REPLAY_PRINT").is_some() {
            println!("{text}");
        }
    }

    /// R11 G1: the roadside parcel rule is per-provenance. A pad that sits on a
    /// profile requires exactly the flat pad (zero insets), because its 2 m ring
    /// lies in the road's blend ramp and can never be realised as flat yard. An
    /// unprofiled neighbour keeps the legacy 2/2/2 ring that b8 realised.
    #[test]
    fn roadside_parcel_insets_follow_the_profile_rule() {
        let (scene, natural, water) = fixture();
        let flat = plan(&scene, &natural, water).unwrap();
        assert!(!flat.roadside_sites.is_empty());
        assert!(
            flat.roadside_sites.iter().all(|site| site.parcel_insets.is_none()),
            "an unprofiled neighbour must keep b8's legacy parcel ring"
        );

        let (scene, natural, water, _zones, graded) = graded_full_plan();
        assert!(
            !graded.roadside_sites.is_empty(),
            "the graded plan carries roadside pads"
        );
        for site in &graded.roadside_sites {
            let insets = site
                .parcel_insets
                .expect("a profiled pad declares its parcel explicitly");
            assert_eq!(
                (insets.front_m, insets.side_m, insets.rear_m),
                (0, 0, 0),
                "{}: a flat pad's required yard IS the pad",
                site.id
            );
            let boundary = super::super::frontage::parcel_boundary(site).unwrap();
            assert_eq!(
                (boundary[2] - boundary[0]) * (boundary[3] - boundary[1]),
                i64::from(site.width_m) * i64::from(site.depth_m),
                "{}: zero insets make the parcel area equal the pad area",
                site.id
            );
        }
        let _ = (scene, natural, water);
    }

    /// R11 G2: the post-bake anchor reconciliation must leave a PROFILED road on
    /// its own longitudinal surface. The value it used to recompute -
    /// `ceil(baked_centre - bottom_offset)` - lands on an exact integer when the
    /// baked centre equals the profile, and f32 rounds that integer just above
    /// its floor, which moved the anchor a whole metre and broke the scale
    /// audit's contract clause for `generated.road-network.scene-0.129-130`.
    #[test]
    fn post_bake_reconciliation_keeps_a_profiled_road_on_its_profile() {
        use crate::height_payload::encode_millimetres;
        let scene = SceneSpec {
            scene_id: "contract-boundary".into(),
            width_m: 16,
            depth_m: 16,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let values: Vec<f64> = vec![-1.374; 16 * 16];
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        terrain
            .insert_chunk(0, 0, 16, 16, &encode_millimetres(16, 16, values).unwrap())
            .unwrap();
        let height = -1.374_f64;
        let profile = super::super::road_profile::RoadProfile {
            version: 1,
            road_width_m: 8,
            start: [0, 8],
            end: [16, 8],
            elevations_mm: (0..=16).map(|_| -1374_i64).collect(),
            max_fill_mm: 6000,
        };
        let mut entity = EntityInstance {
            entity_id: "generated.road-network.scene-0.contract-boundary".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 8,
            world_z: 8,
            world_y: height.floor() as i32,
            width_m: 16.,
            depth_m: 8.,
            height_m: 0.24,
            rotation_y_deg: 0.,
            scale: 1.,
            grounding: GroundingSpec {
                road_profile: Some(profile),
                bottom_offset_m: (height - height.floor()) as f32,
                ..Default::default()
            },
            anchors: Vec::new(),
            bounds: None,
        };
        entity
            .grounding
            .road_profile
            .as_ref()
            .unwrap()
            .validate_entity(&entity)
            .expect("a road that sits on its own profile satisfies the contract");
        // Record WHY this is fragile: the raw expression sits above -2.0 in f32
        // even though the exact value is -2, so a naive ceil() picks -1.
        let baked = terrain.height(entity.world_x, entity.world_z).unwrap();
        let offset = entity.ground_height_m() - entity.world_y as f32;
        assert!(
            baked - offset > -2.0,
            "expected the rounding boundary the 1 km audit hit, got {}",
            baked - offset
        );
        super::super::rebase_generated_transport_to_baked_surface(
            std::slice::from_mut(&mut entity),
            &[],
            &terrain,
            None,
        );
        assert_eq!(entity.world_y, -2, "the anchor must not be re-derived");
        assert!(
            (f64::from(entity.ground_height_m()) + 1.374).abs() < 1e-6,
            "the anchor still describes the profile: {}",
            entity.ground_height_m()
        );
        entity
            .grounding
            .road_profile
            .as_ref()
            .unwrap()
            .validate_entity(&entity)
            .expect("the reconciliation must leave the profile contract intact");
    }

    /// B01-T2 R11 G3: why do the roadside pads fail the access audit offline?
    ///
    /// It runs the production chain and the production access graph
    /// (`place_generated_entrances` + `build_access_graph`) on the frozen 1 km
    /// input over the production encoder's raster, and reports every link that
    /// is not `walkable_route`, with the attributed road and the required flag.
    /// Writes NO world product; output only when `GLYPHWEAVE_1KM_ACCESS_OUT` is
    /// set.
    #[test]
    #[ignore = "R11 G3 offline access replay of the frozen 1 km input; writes only when GLYPHWEAVE_1KM_ACCESS_OUT is set"]
    fn v5_1km_access_replay() {
        use crate::height_payload::encode_millimetres;
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let manifest_path = std::env::var_os("GLYPHWEAVE_1KM_MANIFEST")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                repo.join(
                    "bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r2/1km-candidate/frozen-input/b01-1km-probe.manifest.json",
                )
            });
        let manifest_bytes = std::fs::read(&manifest_path).unwrap();
        let manifest: super::super::WorldManifest =
            serde_json::from_slice(&manifest_bytes).unwrap();
        let scene = &manifest.scenes[0];
        let style = &manifest.style;
        let seed = manifest.world.seed ^ scene.seed_offset;
        let water = super::super::water_geometry(
            super::super::water_kind(style),
            &manifest.landmarks,
            scene,
            style,
        );
        let mut terrain = BakedTerrain::new(scene).unwrap();
        let chunk_size = 512_u32;
        for chunk_z in 0..scene.depth_m.div_ceil(chunk_size) {
            for chunk_x in 0..scene.width_m.div_ceil(chunk_size) {
                let dx = chunk_x * chunk_size;
                let dz = chunk_z * chunk_size;
                let width = (scene.width_m - dx).min(chunk_size);
                let depth = (scene.depth_m - dz).min(chunk_size);
                let x = scene.origin_x + dx as i32;
                let z = scene.origin_z + dz as i32;
                let bytes = encode_millimetres(
                    width,
                    depth,
                    (0..depth).flat_map(|row| {
                        (0..width).map(move |col| {
                            super::super::terrain_height_metres(
                                seed,
                                x + col as i32,
                                z + row as i32,
                                water,
                                &[],
                            )
                        })
                    }),
                )
                .unwrap();
                terrain.insert_chunk(x, z, width, depth, &bytes).unwrap();
            }
        }
        terrain.require_complete().unwrap();
        let zones = super::super::LandUseProfile::from_style(style)
            .map(|profile| super::super::plan_landuse_zones(scene, &profile));
        let program = super::super::services::from_style(style).unwrap();
        let source = super::super::declared_natural_source(&manifest, scene).unwrap();
        let plan = super::plan_with_style_declared(
            scene,
            &terrain,
            water,
            zones.as_deref(),
            program.as_ref(),
            style,
            source,
        )
        .unwrap();
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let carves = super::super::plan_terrain_carves(&entities);
        let mask = zones
            .as_deref()
            .map(|zones| super::super::zoning::ZoningMask::new(scene, zones).unwrap());
        let mut final_terrain = BakedTerrain::new(scene).unwrap();
        for chunk_z in 0..scene.depth_m.div_ceil(chunk_size) {
            for chunk_x in 0..scene.width_m.div_ceil(chunk_size) {
                let dx = chunk_x * chunk_size;
                let dz = chunk_z * chunk_size;
                let width = (scene.width_m - dx).min(chunk_size);
                let depth = (scene.depth_m - dz).min(chunk_size);
                let x = scene.origin_x + dx as i32;
                let z = scene.origin_z + dz as i32;
                let (height, _surface, _lod2, _semantics) =
                    super::super::generate_chunk_encoded(
                        seed,
                        x,
                        z,
                        width,
                        depth,
                        water,
                        &carves,
                        true,
                        mask.as_ref(),
                    );
                final_terrain
                    .insert_chunk(x, z, width, depth, &height)
                    .unwrap();
            }
        }
        super::super::rebase_entities_to_baked_surface(&mut entities, &final_terrain);
        super::super::rebase_generated_transport_to_baked_surface(
            &mut entities,
            &[],
            &final_terrain,
            None,
        );
        // The bake's own order: choose generated entrances, then build the graph
        // the development audit reads.
        let graph = super::super::access::place_generated_entrances(
            &mut entities,
            &[],
            &final_terrain,
            water,
        );
        // The development audit's clause, offline: a checked link must reach the
        // road the pad was ASSIGNED.
        let mut assigned_failures: Vec<serde_json::Value> = Vec::new();
        for site in &plan.roadside_sites {
            let link = graph.links.iter().find(|link| link.entity_id == site.id);
            let matched = link.is_some_and(|link| {
                link.connected
                    && link.required
                    && link.road_id.as_deref() == Some(site.road_id.as_str())
            });
            if !matched {
                assigned_failures.push(serde_json::json!({
                    "siteId": site.id,
                    "assignedRoadId": site.road_id,
                    "linkRoadId": link.and_then(|link| link.road_id.clone()),
                    "linkStatus": link.map(|link| link.status.clone()).unwrap_or_else(|| "missing".into()),
                }));
            }
        }
        let mut by_status: std::collections::BTreeMap<String, usize> = Default::default();
        let mut failing: Vec<serde_json::Value> = Vec::new();
        for link in &graph.links {
            *by_status.entry(link.status.clone()).or_default() += 1;
            if link.status == "walkable_route" {
                continue;
            }
            failing.push(serde_json::json!({
                "entityId": link.entity_id,
                "anchorId": link.anchor_id,
                "roadId": link.road_id,
                "connected": link.connected,
                "required": link.required,
                "status": link.status,
                "distanceM": link.distance_m,
            }));
        }
        // The plan's own assignment, so the dump can be compared with the link
        // the graph actually attributed.
        let assignments: Vec<serde_json::Value> = plan
            .roadside_sites
            .iter()
            .map(|site| {
                serde_json::json!({
                    "siteId": site.id,
                    "assignedRoadId": site.road_id,
                    "worldX": site.world_x,
                    "worldZ": site.world_z,
                    "supportLevelMm": site.support_level_mm,
                    "supportLevelM": site.support_level_m,
                    "frontageRoadPoint": site.frontage_road_point,
                    "parcelInsets": site.parcel_insets,
                })
            })
            .collect();
        let report = serde_json::json!({
            "schema": "b01t2c-r11-access-replay-v1",
            "evidenceLevel": "offline-stage-replay",
            "notAWorldMeasurement": "this is NOT a production-world result",
            "inputSha256": blake3::hash(&manifest_bytes).to_hex().to_string(),
            "sceneId": scene.scene_id,
            "linkedEntities": graph.links.iter().map(|l| l.entity_id.clone()).collect::<std::collections::BTreeSet<_>>().len(),
            "connectedEntities": graph.connected_entities,
            "disconnectedEntities": graph.disconnected_entities,
            "byStatus": by_status,
            "failingLinks": failing,
            "assignedRoadFailureCount": assigned_failures.len(),
            "assignedRoadFailures": assigned_failures,
            "roadsideAssignments": assignments,
        });
        let text = serde_json::to_string_pretty(&report).unwrap();
        if let Some(path) = std::env::var_os("GLYPHWEAVE_1KM_ACCESS_OUT") {
            std::fs::write(path, format!("{text}\n")).unwrap();
        }
        if std::env::var_os("GLYPHWEAVE_1KM_ACCESS_PRINT").is_some() {
            println!("{text}");
        }
    }

    fn frontage_request(scene_id: &str) -> super::super::frontage::Request {
        super::super::frontage::Request {
            id: "t2c-r2".into(),
            scene_id: scene_id.into(),
            purpose: "residential".into(),
            kind: "residential_home".into(),
            count: 1,
            width_m: 10,
            depth_m: 10,
            height_m: 6,
            station_spacing_m: 16,
            setback_m: 4,
            max_candidate_attempts: 8,
            parcel_insets: Some(super::super::parcel_geometry::ParcelInsets { front_m: 0, side_m: 0, rear_m: 0 }),
            rural_conversion: super::super::frontage::RuralConversion::Disabled,
        }
    }

    /// B01-T2 R11 G2: the road longitudinal contract, replayed offline on the
    /// frozen 1 km input.
    ///
    /// It runs the production planning chain, then repeats the bake's own
    /// post-bake reconciliation (`rebase_entities_to_baked_surface` +
    /// `rebase_generated_transport_to_baked_surface`) against the production
    /// ENCODER's raster (`generate_chunk_encoded`, millimetres, zoning-aware),
    /// and finally applies the very clause the scale audit applies to every
    /// profiled road: `RoadProfile::validate_entity`. Writes NO world product
    /// and never calls the CLI; its only output is
    /// `r11/1km-road-contract.json` when `GLYPHWEAVE_1KM_CONTRACT_OUT` is set.
    #[test]
    #[ignore = "R11 G2 offline road-contract replay of the frozen 1 km input; writes only when GLYPHWEAVE_1KM_CONTRACT_OUT is set"]
    fn v5_1km_road_contract_replay() {
        use crate::height_payload::encode_millimetres;
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let manifest_path = std::env::var_os("GLYPHWEAVE_1KM_MANIFEST")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                repo.join(
                    "bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r2/1km-candidate/frozen-input/b01-1km-probe.manifest.json",
                )
            });
        let manifest_bytes = std::fs::read(&manifest_path).unwrap();
        let manifest: super::super::WorldManifest =
            serde_json::from_slice(&manifest_bytes).unwrap();
        let scene = &manifest.scenes[0];
        let style = &manifest.style;
        let seed = manifest.world.seed ^ scene.seed_offset;
        let water = super::super::water_geometry(
            super::super::water_kind(style),
            &manifest.landmarks,
            scene,
            style,
        );
        let mut terrain = BakedTerrain::new(scene).unwrap();
        let chunk_size = 512_u32;
        for chunk_z in 0..scene.depth_m.div_ceil(chunk_size) {
            for chunk_x in 0..scene.width_m.div_ceil(chunk_size) {
                let dx = chunk_x * chunk_size;
                let dz = chunk_z * chunk_size;
                let width = (scene.width_m - dx).min(chunk_size);
                let depth = (scene.depth_m - dz).min(chunk_size);
                let x = scene.origin_x + dx as i32;
                let z = scene.origin_z + dz as i32;
                let bytes = encode_millimetres(
                    width,
                    depth,
                    (0..depth).flat_map(|row| {
                        (0..width).map(move |col| {
                            super::super::terrain_height_metres(
                                seed,
                                x + col as i32,
                                z + row as i32,
                                water,
                                &[],
                            )
                        })
                    }),
                )
                .unwrap();
                terrain.insert_chunk(x, z, width, depth, &bytes).unwrap();
            }
        }
        terrain.require_complete().unwrap();
        let zones = super::super::LandUseProfile::from_style(style)
            .map(|profile| super::super::plan_landuse_zones(scene, &profile));
        let program = super::super::services::from_style(style).unwrap();
        let source = super::super::declared_natural_source(&manifest, scene).unwrap();
        let plan = super::plan_with_style_declared(
            scene,
            &terrain,
            water,
            zones.as_deref(),
            program.as_ref(),
            style,
            source,
        )
        .unwrap();
        // The bake's carve set and its post-bake reconciliation both run over
        // the plan's road entities plus the sites they carry, normalized the
        // same way (see the bake's `road_carves`).
        let mut entities = plan.entities();
        entities.extend(plan.site_entities());
        for entity in &mut entities {
            entity.normalize_spatial_semantics();
        }
        let carves = super::super::plan_terrain_carves(&entities);
        let mask = zones
            .as_deref()
            .map(|zones| super::super::zoning::ZoningMask::new(scene, zones).unwrap());
        let mut final_terrain = BakedTerrain::new(scene).unwrap();
        for chunk_z in 0..scene.depth_m.div_ceil(chunk_size) {
            for chunk_x in 0..scene.width_m.div_ceil(chunk_size) {
                let dx = chunk_x * chunk_size;
                let dz = chunk_z * chunk_size;
                let width = (scene.width_m - dx).min(chunk_size);
                let depth = (scene.depth_m - dz).min(chunk_size);
                let x = scene.origin_x + dx as i32;
                let z = scene.origin_z + dz as i32;
                let (height, _surface, _lod2, _semantics) =
                    super::super::generate_chunk_encoded(
                        seed,
                        x,
                        z,
                        width,
                        depth,
                        water,
                        &carves,
                        true,
                        mask.as_ref(),
                    );
                final_terrain
                    .insert_chunk(x, z, width, depth, &height)
                    .unwrap();
            }
        }
        let landmarks: Vec<super::super::LandmarkSpec> = Vec::new();
        super::super::rebase_entities_to_baked_surface(&mut entities, &final_terrain);
        super::super::rebase_generated_transport_to_baked_surface(
            &mut entities,
            &landmarks,
            &final_terrain,
            None,
        );
        let mut checked = 0_usize;
        let mut violations: Vec<serde_json::Value> = Vec::new();
        for entity in &entities {
            let Some(profile) = entity.grounding.road_profile.as_ref() else {
                continue;
            };
            checked += 1;
            let centre_profile_m = profile
                .height_at(f64::from(entity.world_x), f64::from(entity.world_z))
                .unwrap_or(f64::NAN);
            let baked_centre_m = final_terrain
                .height(entity.world_x, entity.world_z)
                .unwrap_or(f64::NAN as f32);
            if let Err(error) = profile.validate_entity(entity) {
                violations.push(serde_json::json!({
                    "entityId": entity.entity_id,
                    "error": error.to_string(),
                    "worldY": entity.world_y,
                    "bottomOffsetM": entity.grounding.bottom_offset_m,
                    "groundM": entity.ground_height_m(),
                    "profileAtCentreM": centre_profile_m,
                    "bakedTerrainAtCentreM": baked_centre_m,
                }));
            }
        }
        let report = serde_json::json!({
            "schema": "b01t2c-r11-road-contract-v1",
            "evidenceLevel": "offline-stage-replay",
            "notAWorldMeasurement": "this is NOT a production-world result and must not be used to claim city-capacity growth",
            "inputManifest": manifest_path.display().to_string(),
            "inputSha256": blake3::hash(&manifest_bytes).to_hex().to_string(),
            "sceneId": scene.scene_id,
            "profiledRoadsChecked": checked,
            "violations": violations,
            "violationCount": violations.len(),
        });
        let text = serde_json::to_string_pretty(&report).unwrap();
        if let Some(path) = std::env::var_os("GLYPHWEAVE_1KM_CONTRACT_OUT") {
            std::fs::write(path, format!("{text}\n")).unwrap();
        }
        if std::env::var_os("GLYPHWEAVE_1KM_CONTRACT_PRINT").is_some() {
            println!("{text}");
        }
    }

    /// C4 point 1: profile -> entities() -> validate_entity -> stations() non-empty
    /// -> at least one real candidate site, on the PRODUCTION chain.
    #[test]
    fn c4_profiled_base_reaches_entities_validation_stations_and_a_site() {
        let (scene, _natural, _water, _zones, base) = graded_full_plan();
        assert!(!base.edges.is_empty());
        assert!(
            base.edges.iter().all(|edge| edge.profile.is_some()),
            "every selected base edge earns a profile"
        );
        let entities = base.entities();
        let profiled: Vec<_> = entities
            .iter()
            .filter(|entity| entity.grounding.road_profile.is_some())
            .collect();
        assert_eq!(profiled.len(), base.edges.len());
        for entity in &profiled {
            entity
                .grounding
                .road_profile
                .as_ref()
                .unwrap()
                .validate_entity(entity)
                .unwrap();
        }
        let request = frontage_request(&scene.scene_id);
        let mut sites = 0;
        for entity in &profiled {
            if let Some(stations) =
                super::super::frontage::stations(entity, &request).unwrap()
            {
                if !stations.is_empty() {
                    super::super::frontage::candidate_site(
                        entity,
                        &request,
                        &scene.scene_id,
                        stations[0],
                        1,
                    )
                    .unwrap();
                    sites += 1;
                }
            }
        }
        assert!(sites > 0, "a profiled base road must yield a real candidate site");
    }

    /// C3: base identity is planning provenance, not profile presence, and the
    /// trusted contract is derived from the real validated base plan and then
    /// refuses any silent change to a base edge's authoritative profile.
    #[test]
    fn c3_base_identity_is_provenance_and_the_contract_locks_profiles() {
        let (_scene, natural, _water, _zones, base) = graded_full_plan();
        let natural_hash = natural.content_hash().unwrap();
        // The pre-T2 identity predicate ("base == unprofiled") now sees an EMPTY
        // base domain. This is the counterexample that the old rule produced.
        let pre_t2_base = base
            .entities()
            .into_iter()
            .filter(|entity| entity.kind == "road" && entity.grounding.road_profile.is_none())
            .count();
        assert_eq!(
            pre_t2_base, 0,
            "profile presence can no longer identify the base domain"
        );
        // Provenance identity keeps the whole base network ...
        let trusted_base = super::super::auto_roads::base_only_plan(&base).unwrap();
        assert_eq!(trusted_base.edges.len(), base.edges.len());
        // ... and the trusted contract is built from it instead of refusing it.
        let contract =
            super::super::road_parcel_feasibility::trusted_base_contract(&trusted_base, &natural_hash)
                .unwrap();
        assert_eq!(contract.base_edges.len(), base.edges.len());
        assert!(contract.base_edges.iter().all(|edge| edge.profile.is_some()));
        // Mutation sensitivity: a silently re-graded base profile is refused ...
        let mut changed = base.clone();
        let victim = changed
            .edges
            .iter_mut()
            .find(|edge| edge.profile.is_some())
            .unwrap();
        victim.profile.as_mut().unwrap().elevations_mm[1] += 40;
        let changed_base = super::super::auto_roads::base_only_plan(&changed).unwrap();
        assert!(
            super::super::road_parcel_feasibility::audit_node_heights(&changed_base, &contract)
                .is_err(),
            "a changed base profile must be refused"
        );
        // ... and so is flattening it back to a constant legacy level.
        let mut flattened = base.clone();
        let victim = flattened
            .edges
            .iter_mut()
            .find(|edge| edge.profile.is_some())
            .unwrap();
        let profile = victim.profile.as_mut().unwrap();
        let first = profile.elevations_mm[0];
        for sample in profile.elevations_mm.iter_mut() {
            *sample = first;
        }
        let flat_base = super::super::auto_roads::base_only_plan(&flattened).unwrap();
        assert!(
            super::super::road_parcel_feasibility::audit_node_heights(&flat_base, &contract)
                .is_err(),
            "flattening a base profile must be refused"
        );
    }

    /// F4: a single base edge that cannot earn a legal profile is RETAINED with
    /// `profile: None`, receives a named rejection, and earns no station or site.
    #[test]
    fn f4_single_edge_refusal_is_named_retained_and_gives_no_station_or_site() {
        let (scene, natural, water, zones, base, _level_mm) = graded_fixture();
        let level = base.level_m.expect("resolved base level");
        let mut plan = base.clone();
        for edge in plan.edges.iter_mut() {
            edge.profile = None;
        }
        plan.base_profile_rejections.clear();
        // Damage exactly one edge so no legal profile can describe it: its
        // declared centre is no longer the midpoint of its endpoints.
        let victim_id = plan.edges[1].id.clone();
        plan.edges[1].world_x += 1;
        let nodes = plan.nodes.clone();
        let refusals = super::attach_base_edge_profiles(
            &mut plan,
            &scene,
            &natural,
            water,
            &nodes,
            level,
            Some(&zones),
            0,
        )
        .unwrap();
        assert!(
            refusals
                .iter()
                .any(|r| r.edge_id == victim_id && r.category == "profile_entity_mismatch"),
            "{refusals:?}"
        );
        let victim = plan.edges.iter().find(|edge| edge.id == victim_id).unwrap();
        assert!(victim.profile.is_none(), "a refused base edge keeps no profile");
        let entities = plan.entities();
        let victim_entity = entities
            .iter()
            .find(|entity| entity.entity_id == victim_id)
            .unwrap();
        assert!(victim_entity.grounding.road_profile.is_none());
        let request = frontage_request(&scene.scene_id);
        assert_eq!(
            super::super::frontage::stations(victim_entity, &request).unwrap(),
            None
        );
        assert!(
            super::super::frontage::candidate_site(
                victim_entity,
                &request,
                &scene.scene_id,
                13,
                1
            )
            .is_err()
        );
        // Every other base edge still earned a profile and keeps its road.
        assert!(
            plan.edges
                .iter()
                .filter(|edge| edge.id != victim_id)
                .all(|edge| edge.profile.is_some())
        );
    }

    /// F4/I5: structural damage fails closed instead of being downgraded to a
    /// per-edge refusal, so no half-updated plan can escape.
    #[test]
    fn f4_structural_damage_fails_closed() {
        let (scene, natural, water, zones, base, _level_mm) = graded_fixture();
        let level = base.level_m.expect("resolved base level");
        let mut duplicated = base.clone();
        let clone = duplicated.edges[0].clone();
        duplicated.edges.push(clone);
        let nodes = duplicated.nodes.clone();
        let error = super::attach_base_edge_profiles(
            &mut duplicated,
            &scene,
            &natural,
            water,
            &nodes,
            level,
            Some(&zones),
            0,
        )
        .unwrap_err();
        assert!(error.0.contains("structural damage"), "{error:?}");

        let mut orphan = base.clone();
        let missing = orphan.nodes.iter().map(|node| node.id).max().unwrap() + 7;
        orphan.edges[0].to = missing;
        let nodes = orphan.nodes.clone();
        let error = super::attach_base_edge_profiles(
            &mut orphan,
            &scene,
            &natural,
            water,
            &nodes,
            level,
            Some(&zones),
            0,
        )
        .unwrap_err();
        assert!(error.0.contains("structural damage"), "{error:?}");
    }

    /// F5: the elevation gate is exact at -32000/256000 mm, extreme i32 levels
    /// and non-finite rotations are refused, and nothing panics or saturates its
    /// way to a legal-looking value.
    #[test]
    fn f5_numeric_boundary_gate_is_exact_and_panic_free() {
        let (scene, natural, water, zones, base, _level_mm) = graded_fixture();
        let level = base.level_m.expect("resolved base level");
        let context =
            super::super::road_alignment::AlignmentContext::new(&scene, &natural, water, &zones)
                .unwrap();
        let edge = base
            .edges
            .iter()
            .find(|edge| edge.profile.is_some())
            .unwrap()
            .clone();
        let nodes = &base.nodes;
        // Past the millimetre domain and the i32 extremes: refused by the domain
        // gate itself, never by a saturated pass and never by a panic.
        for probe in [-33, 257, i32::MIN, i32::MAX] {
            let (category, reason) =
                super::base_edge_profile(&edge, nodes, &context, probe).unwrap_err();
            assert!(
                category == "elevation_out_of_domain" || category == "elevation_overflow",
                "{probe}: {category} {reason}"
            );
        }
        // The legal domain edges are NOT refused by the elevation domain gate.
        for probe in [-32, 256, level] {
            if let Err((category, _)) =
                super::base_edge_profile(&edge, nodes, &context, probe)
            {
                assert_ne!(category, "elevation_out_of_domain", "level {probe}");
                assert_ne!(category, "elevation_overflow", "level {probe}");
            }
        }
        // Non-finite / non-cardinal rotations are refused before any tolerance test.
        for rotation in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 45.] {
            let mut bad = edge.clone();
            bad.rotation_y_deg = rotation;
            let (category, _) =
                super::base_edge_profile(&bad, nodes, &context, level).unwrap_err();
            assert_eq!(category, "edge_geometry_not_finite_or_misaligned", "{rotation}");
        }
        // The legal cardinals: the edge's own cardinal still produces a profile,
        // the other cardinal is refused as a geometry mismatch (not silently kept).
        for rotation in [0., 90.] {
            let mut probe = edge.clone();
            probe.rotation_y_deg = rotation;
            if rotation == edge.rotation_y_deg {
                assert!(super::base_edge_profile(&probe, nodes, &context, level).is_ok());
            } else {
                let (category, _) =
                    super::base_edge_profile(&probe, nodes, &context, level).unwrap_err();
                assert_eq!(category, "edge_geometry_not_finite_or_misaligned");
            }
        }
    }

    /// W4: the same input built twice serializes to identical bytes, including
    /// the attached profiles and the persisted refusal order.
    #[test]
    fn w4_graded_plan_serialization_is_byte_identical_across_builds() {
        let (scene, natural, water, zones, _base) = graded_full_plan();
        let first = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let second = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
    }

    /// W4 points 2 + 3: a cross-process digest and a golden summary snapshot for
    /// the T2 chain. The snapshot is only rewritten when the caller explicitly
    /// sets `GLYPHWEAVE_UPDATE_GOLDEN`; a drift is a hard failure otherwise.
    #[test]
    fn w4_chain_digest_and_golden_summary_are_stable() {
        let (scene, natural, water, zones, _base) = graded_full_plan();
        let plan = plan_with_zones(&scene, &natural, water, Some(&zones)).unwrap();
        let entities = plan.entities();
        let request = frontage_request(&scene.scene_id);
        let mut stations = 0_usize;
        let mut sites = 0_usize;
        for entity in &entities {
            if let Some(found) = super::super::frontage::stations(entity, &request).unwrap() {
                stations += found.len();
                if !found.is_empty() {
                    super::super::frontage::candidate_site(
                        entity,
                        &request,
                        &scene.scene_id,
                        found[0],
                        1,
                    )
                    .unwrap();
                    sites += 1;
                }
            }
        }
        let profiles: Vec<_> = plan.edges.iter().map(|edge| edge.profile.clone()).collect();
        let summary = serde_json::json!({
            "model": plan.model,
            "status": plan.status,
            "levelM": plan.level_m,
            "edges": plan.edges.len(),
            "profiledEdges": profiles.iter().filter(|profile| profile.is_some()).count(),
            "rejections": plan.base_profile_rejections.len(),
            "rejectionCategories": plan
                .base_profile_rejections
                .iter()
                .map(|rejection| rejection.category.clone())
                .collect::<Vec<_>>(),
            "entities": entities.len(),
            "stations": stations,
            "candidateSites": sites,
            "profileSampleHash": blake3::hash(&serde_json::to_vec(&profiles).unwrap())
                .to_hex()
                .to_string(),
            "entityIdsHash": blake3::hash(
                &serde_json::to_vec(
                    &entities
                        .iter()
                        .map(|entity| entity.entity_id.clone())
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            )
            .to_hex()
            .to_string(),
        });
        let golden_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/resources/t2c-r2-golden");
        let golden = golden_dir.join("chain-summary.json");
        if std::env::var("GLYPHWEAVE_UPDATE_GOLDEN").is_ok() {
            std::fs::create_dir_all(&golden_dir).unwrap();
            std::fs::write(&golden, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
        }
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&golden).unwrap_or_else(|error| {
                panic!("missing golden {}: {error}", golden.display())
            }))
            .unwrap();
        assert_eq!(
            summary, expected,
            "T2 chain golden drifted; update only with GLYPHWEAVE_UPDATE_GOLDEN=1"
        );
        if let Ok(path) = std::env::var("GLYPHWEAVE_W4_DIGEST_OUT") {
            let digest = blake3::hash(&serde_json::to_vec(&summary).unwrap())
                .to_hex()
                .to_string();
            std::fs::write(path, format!("{digest}\n")).unwrap();
        }
    }

    /// C4 point 3 + legacy compatibility: save -> load keeps every base profile
    /// and the persisted refusals, while legacy JSON without the T2 fields still
    /// loads and preserves the unprofiled legacy behaviour.
    #[test]
    fn c4_serde_round_trip_keeps_base_profiles_and_rejections() {
        let (_scene, _natural, _water, _zones, base) = graded_full_plan();
        let mut plan = base.clone();
        plan.base_profile_rejections = vec![BaseProfileRejection {
            edge_id: "generated.road-network.graded-audit.0-1".into(),
            category: "profile_entity_mismatch".into(),
            reason: "fixture".into(),
        }];
        let bytes = serde_json::to_vec(&plan).unwrap();
        let back: RoadNetworkPlan = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.base_profile_rejections, plan.base_profile_rejections);
        assert_eq!(back.edges.len(), plan.edges.len());
        for (left, right) in back.edges.iter().zip(plan.edges.iter()) {
            assert_eq!(left.id, right.id);
            assert_eq!(left.profile, right.profile);
        }
        // Legacy JSON: no `baseProfileRejections`, no `profile` on any edge.
        let mut legacy = serde_json::to_value(&plan).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("baseProfileRejections");
        for edge in legacy["edges"].as_array_mut().unwrap() {
            edge.as_object_mut().unwrap().remove("profile");
        }
        let legacy_plan: RoadNetworkPlan = serde_json::from_value(legacy).unwrap();
        assert!(legacy_plan.base_profile_rejections.is_empty());
        assert!(legacy_plan.edges.iter().all(|edge| edge.profile.is_none()));
        assert!(
            legacy_plan
                .entities()
                .iter()
                .all(|entity| entity.grounding.road_profile.is_none())
        );
    }
}

/// Independent audit of the scene's water crossings, from the plan's own
/// published road geometry, the water body and BOTH rasters.
///
/// Every number in a [`WaterCrossing`] is a *claim*, and each one is recomputed
/// here by [`measure_water_crossing`] (the same function the planner used, so the
/// two cannot drift) or read off the natural/final rasters. There is no trusted
/// shortcut: a forged record fails by name.
///
/// The clauses, in the order they are checked:
///
///  1. the record names a road that the plan actually contains;
///  2. its structure class is `bridge` or `causeway`;
///  3. it names THIS scene's water body and its class;
///  4. the road it names really does reach the water (a dry road marked as a
///     bridge is refused);
///  5. the recorded span length, wet-cell count, position box and bed range
///     equal the measured ones (an overstated or understated length is refused);
///  6. the recorded deck level equals the road's own published surface at the
///     span, and clears the water datum;
///  7. the recorded take-off equals the highest DRY natural cell, the deck sits
///     at or above it, and the rise from it stays inside
///     `maxRoadbedFillM` - the roadbed fill contract applied to the only
///     earthwork contact a structure deck has;
///  8. every road that reaches the water carries a record (a deleted record is
///     refused);
///  9. no earthwork was placed in the water: the wetted area of the scene is the
///     same before and after, cell for cell, so the surface cannot be shrunk by
///     filling the bed.
#[allow(clippy::too_many_lines)]
pub fn audit_water_crossings(
    record: &RoadNetworkPlan,
    water: WaterGeometry,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
) -> Result<Vec<String>, TerrainError> {
    let mut failures = Vec::new();
    if water.kind == super::WaterKind::None {
        if !record.water_crossings.is_empty() {
            failures.push(format!(
                "{} water crossings are recorded but the scene declares no structured water",
                record.water_crossings.len()
            ));
        }
        return Ok(failures);
    }
    let expected_water_id = water_body_id(&record.scene_id, water);
    let expected_water_kind = water_kind_name(water.kind);
    let roads: Vec<EntityInstance> = record.entities();
    let by_id: BTreeMap<&str, &EntityInstance> =
        roads.iter().map(|e| (e.entity_id.as_str(), e)).collect();
    let mut recorded: BTreeSet<&str> = BTreeSet::new();
    for crossing in &record.water_crossings {
        let road = crossing.road_id.as_str();
        if !recorded.insert(road) {
            failures.push(format!(
                "water crossing {road} is recorded twice; one span cannot carry two structures"
            ));
            continue;
        }
        let Some(entity) = by_id.get(road) else {
            failures.push(format!(
                "water crossing {road} names no road in the plan"
            ));
            continue;
        };
        if !WATER_CROSSING_KINDS.contains(&crossing.kind.as_str()) {
            failures.push(format!(
                "water crossing {road} has structure type {:?}; only {:?} may span water",
                crossing.kind, WATER_CROSSING_KINDS
            ));
            continue;
        }
        if crossing.water_id != expected_water_id || crossing.water_kind != expected_water_kind {
            failures.push(format!(
                "water crossing {road} names water body {}/{} but this scene holds {expected_water_id}/{expected_water_kind}",
                crossing.water_id, crossing.water_kind
            ));
            continue;
        }
        let (pavement_hw, pavement_hd) = super::graded_pavement_half_extents(entity);
        let along_x = entity.rotation_y_deg != 90.;
        let measured_width = if along_x { pavement_hd } else { pavement_hw } * 2;
        if i64::from(crossing.width_m) != i64::from(measured_width) {
            failures.push(format!(
                "water crossing {road} records a {}-m deck but the road's own pavement is {measured_width} m wide",
                crossing.width_m
            ));
            continue;
        }
        let Some(measured) = measure_water_crossing(entity, water, natural)? else {
            failures.push(format!(
                "water crossing {road} records a span over {expected_water_id} but the road never touches that water body"
            ));
            continue;
        };
        if crossing.length_m != measured.stations_m
            || crossing.wet_cells != measured.wet_cells
            || crossing.span != measured.span
            || crossing.bed_min_mm != measured.bed_min_mm
            || crossing.bed_max_mm != measured.bed_max_mm
        {
            failures.push(format!(
                "water crossing {road} disagrees with the water it names: recorded {} m / {} cells at {:?} bed {}..{} mm, measured {} m / {} cells at {:?} bed {}..{} mm",
                crossing.length_m,
                crossing.wet_cells,
                crossing.span,
                crossing.bed_min_mm,
                crossing.bed_max_mm,
                measured.stations_m,
                measured.wet_cells,
                measured.span,
                measured.bed_min_mm,
                measured.bed_max_mm
            ));
            continue;
        }
        let span_centre = [
            (measured.span[0] + measured.span[2]) / 2,
            (measured.span[1] + measured.span[3]) / 2,
        ];
        let deck_mm = entity
            .grounding
            .road_profile
            .as_ref()
            .and_then(|p| p.height_at(f64::from(span_centre[0]), f64::from(span_centre[1])))
            .map_or_else(
                || entity.world_y.saturating_mul(1000),
                |m| (m * 1000.).round() as i32,
            );
        if crossing.deck_level_mm != deck_mm {
            failures.push(format!(
                "water crossing {road} records deck {} mm but the road's own surface at the span is {deck_mm} mm",
                crossing.deck_level_mm
            ));
            continue;
        }
        if crossing.water_level_mm != WATER_DATUM_MM {
            failures.push(format!(
                "water crossing {road} records water level {} mm but the baked datum is {WATER_DATUM_MM} mm",
                crossing.water_level_mm
            ));
            continue;
        }
        if crossing.deck_level_mm <= crossing.water_level_mm {
            failures.push(format!(
                "water crossing {road} deck at {} mm does not clear the water datum at {} mm",
                crossing.deck_level_mm, crossing.water_level_mm
            ));
            continue;
        }
        let measured_take_off = measured_take_off_mm(entity, water, natural)?;
        if crossing.take_off_max_mm != measured_take_off {
            failures.push(format!(
                "water crossing {road} records take-off {} mm but the highest dry cell under the road is {measured_take_off} mm",
                crossing.take_off_max_mm
            ));
            continue;
        }
        if crossing.deck_level_mm < crossing.take_off_max_mm {
            failures.push(format!(
                "water crossing {road} deck at {} mm is below its {} mm take-off, which would excavate the shore",
                crossing.deck_level_mm, crossing.take_off_max_mm
            ));
            continue;
        }
        let rise_mm = i64::from(crossing.deck_level_mm) - i64::from(crossing.take_off_max_mm);
        let cap_mm = f64::from(record.max_roadbed_fill_m) * 1000. + 1.;
        if rise_mm as f64 > cap_mm {
            failures.push(format!(
                "water crossing {road} raises its deck {rise_mm} mm above the take-off, beyond the {} m roadbed fill contract",
                record.max_roadbed_fill_m
            ));
            continue;
        }
        for (x, z) in [
            (measured.span[0], measured.span[1]),
            (measured.span[2], measured.span[3]),
        ] {
            let (Some(n), Some(f)) = (natural.height(x, z), final_terrain.height(x, z)) else {
                failures.push(format!(
                    "water crossing {road} has no raster sample at ({x},{z})"
                ));
                continue;
            };
            if f != n {
                failures.push(format!(
                    "water crossing {road} raised the water bed at ({x},{z}) from {n} m to {f} m; a span must place no earthwork in the water"
                ));
            }
        }
    }
    // A road that reaches the water may not do so anonymously.
    for entity in &roads {
        if recorded.contains(entity.entity_id.as_str()) {
            continue;
        }
        if let Some(measured) = measure_water_crossing(entity, water, natural)? {
            failures.push(format!(
                "unregistered water crossing on road {}: {} m of its pavement band lies inside {expected_water_id} with no bridge or causeway record",
                entity.entity_id, measured.stations_m
            ));
        }
    }
    // No building parcel may claim the water, whatever the road plan says.
    for site in record.site_entities() {
        if super::footprint_intersects_water(&site, water, 0.) {
            failures.push(format!(
                "roadside site {} occupies {expected_water_id}",
                site.entity_id
            ));
        }
    }
    // The wetted area of the scene, recomputed from BOTH rasters. A fill inside
    // the water lifts the bed above the datum, the cell leaves the wet set and
    // the surface shrinks - exactly the "fill the lake to build the road" defect
    // this contract exists to refuse.
    if water.road_water_crossings() || !record.water_crossings.is_empty() {
        let (origin_x, origin_z) = final_terrain.scene_origin();
        let (width, depth) = final_terrain.scene_dimensions();
        let (mut footprint, mut wet_natural, mut wet_final) = (0_u64, 0_u64, 0_u64);
        for z in origin_z..origin_z + depth as i32 {
            for x in origin_x..origin_x + width as i32 {
                if !water.contains_point(f64::from(x), f64::from(z)) {
                    continue;
                }
                footprint += 1;
                if natural.height_millimetres(x, z).is_some_and(|h| h < WATER_DATUM_MM) {
                    wet_natural += 1;
                }
                if final_terrain
                    .height_millimetres(x, z)
                    .is_some_and(|h| h < WATER_DATUM_MM)
                {
                    wet_final += 1;
                }
            }
        }
        if wet_natural != footprint {
            failures.push(format!(
                "natural water body of {expected_water_id} is not entirely below the {WATER_DATUM_MM} mm datum ({wet_natural} of {footprint} cells)"
            ));
        }
        if wet_natural != wet_final {
            failures.push(format!(
                "water surface area changed: {wet_natural} m2 of {expected_water_id} before, {wet_final} m2 after"
            ));
        }
    }
    Ok(failures)
}

/// Highest DRY natural cell of the road's pavement band, in millimetres.
///
/// The mirror of the planner's `dry_max`: a structure deck is required to sit at
/// or above it (no excavation) and within the roadbed fill contract above it.
fn measured_take_off_mm(
    road: &EntityInstance,
    water: WaterGeometry,
    natural: &BakedTerrain,
) -> Result<i32, TerrainError> {
    let (hw, hd) = super::graded_transport_half_extents(road);
    let mut take_off = i32::MIN;
    let mut dry = 0_u32;
    for z in road.world_z - hd..=road.world_z + hd {
        for x in road.world_x - hw..=road.world_x + hw {
            if water.contains_point(f64::from(x), f64::from(z)) {
                continue;
            }
            dry += 1;
            let h = natural.height(x, z).ok_or_else(|| {
                TerrainError("water crossing missing natural take-off sample".into())
            })?;
            take_off = take_off.max((h * 1000.).round() as i32);
        }
    }
    if dry == 0 {
        return Ok(i32::MIN);
    }
    Ok(take_off)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadNetworkAudit {
    pub status: String,
    pub planned_edges: usize,
    pub realized_edges: usize,
    pub planned_sites: usize,
    pub realized_sites: usize,
    pub max_final_site_fill_m: f32,
    pub max_site_design_height_error_m: f32,
    /// The deepest excavation any pad of this plan took, in metres. Absent for
    /// every world whose pads are fill-only (the whole project before the opt-in
    /// bounded cut), so the published bytes of those worlds do not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_site_cut_m: Option<f32>,
    pub max_final_roadbed_fill_m: f32,
    pub max_design_height_error_m: f32,
    pub failures: Vec<String>,
}

/// Rebuild from natural raster and inspect every final roadbed sample.
/// Does not repair missing roads or accept a saved success flag as evidence.
pub fn verify(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_with_zones(record, scene, natural, final_terrain, water, entities, None)
}
pub fn verify_with_zones(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_with_program(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        None,
    )
}
pub fn verify_with_program(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        None,
        false,
        super::NaturalSource::Undeclared,
    )
}
pub fn verify_with_style(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_with_style_declared(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        style,
        super::NaturalSource::Undeclared,
    )
}
/// The production audit with an explicit natural-source declaration, so the
/// source rebuild that the audit performs for comparison cannot enter the
/// version-3 planner with a different seed than the candidate was built with.
pub(crate) fn verify_with_style_declared(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
    source: super::NaturalSource,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        Some(style),
        false,
        source,
    )
}

/// The independent audit entry for callers outside the generator (the CLI).
///
/// It derives the scene's declared natural source from the *trusted original
/// manifest* with the same rule generation uses, then runs the full production
/// audit: the source rebuild re-plans the scene, compares the rebuilt plan with
/// the record and audits the final support. It never reads the source back out
/// of the published product and never takes the trusted-record shortcut, so a
/// record that its own manifest cannot reproduce is refused here. A manifest
/// whose declared terrain source this build cannot author is refused too,
/// instead of being guessed onto `SuppliedRaster`.
#[allow(clippy::too_many_arguments)]
pub fn verify_with_manifest(
    record: &RoadNetworkPlan,
    manifest: &WorldManifest,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkAudit, TerrainError> {
    let source = super::declared_natural_source(manifest, scene)
        .map_err(|error| TerrainError(error.to_string()))?;
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        Some(style),
        false,
        source,
    )
}

/// The same production support/entity audit as [`verify_with_style`], but for a
/// plan that is already trusted by its caller: it is not re-planned and not
/// compared for equality, so a demand-led append candidate can be audited with
/// the production rules, thresholds and coverage instead of a second, weaker
/// check. The caller is responsible for the provenance of `record` (the
/// accepted append path) and for [`audit_extension_provenance`].
pub(crate) fn audit_final_support_for_record(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: &serde_json::Value,
) -> Result<RoadNetworkAudit, TerrainError> {
    verify_impl(
        record,
        scene,
        natural,
        final_terrain,
        water,
        entities,
        zones,
        program,
        Some(style),
        true,
        // A trusted record is never re-planned here, so no source is needed to
        // rebuild it; the caller has already asserted its provenance.
        super::NaturalSource::Undeclared,
    )
}

/// Independent source rebuild of every graded extension in a plan. The
/// production audit gets this for free by re-planning and comparing; a trusted
/// candidate must assert it explicitly. Read-only, no re-planning.
pub(crate) fn audit_extension_provenance(
    plan: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: &[LandUseZone],
) -> Result<Vec<String>, TerrainError> {
    let mut failures = vec![];
    for extension in &plan.extensions {
        if let Err(error) = super::road_alignment::verify(
            &extension.design,
            &extension.design.request,
            scene,
            natural,
            water,
            zones,
        ) {
            failures.push(format!(
                "extension {} source rebuild failed: {error}",
                extension.entity_id
            ));
        }
    }
    Ok(failures)
}

fn verify_impl(
    record: &RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    final_terrain: &BakedTerrain,
    water: WaterGeometry,
    entities: &[EntityInstance],
    zones: Option<&[LandUseZone]>,
    program: Option<&super::services::ServiceProgram>,
    style: Option<&serde_json::Value>,
    trusted_record: bool,
    source: super::NaturalSource,
) -> Result<RoadNetworkAudit, TerrainError> {
    // Water crossings are audited on the RECORD, before the plan-equality gate:
    // a forged crossing record must fail BY NAME ("overstated length", "dry road
    // marked as a bridge", "deleted record"), not be flattened into the generic
    // "plan differs from natural raster reconstruction".
    let crossing_failures = audit_water_crossings(record, water, natural, final_terrain)?;
    if !crossing_failures.is_empty() {
        return Ok(RoadNetworkAudit {
            status: "fail".into(),
            planned_edges: record.edges.len(),
            realized_edges: 0,
            planned_sites: record.roadside_sites.len(),
            realized_sites: 0,
            max_final_site_fill_m: 0.,
            max_site_design_height_error_m: 0.,
            max_site_cut_m: None,
            max_final_roadbed_fill_m: 0.,
            max_design_height_error_m: 0.,
            failures: crossing_failures,
        });
    }
    let expected = if trusted_record {
        record.clone()
    } else if let Some(style) = style {
        plan_with_style_declared(scene, natural, water, zones, program, style, source)?
    } else {
        plan_with_program(
            scene,
            natural,
            water,
            zones,
            program,
            super::frontage::RuralConversion::Disabled,
            // No style, so no stated street morphology: the reconstruction this branch
            // compares against is the morphology-free plan, which is exactly what a world
            // that states none publishes.
            None,
            // ...and no style means no bounded-cut policy either.
            0,
        )?
    };
    let mut expected = expected;
    if !trusted_record {
        // The entity-stage refusals the record carries are the one decision the
        // natural-source rebuild cannot make (see `apply_entity_pad_refusals`).
        // They are applied to the rebuild - never skipped - and then JUSTIFIED
        // from the published raster: a refused pad must be a pad whose declared
        // top the world does not hold, which is what keeps a written refusal from
        // hiding a pad the world realises.
        let refused = entity_refused_pad_ids(record);
        if !refused.is_empty() {
            let mut held_by_world = Vec::new();
            for pad_id in &refused {
                let Some(site) = expected
                    .roadside_sites
                    .iter()
                    .find(|site| site.id == *pad_id)
                else {
                    continue;
                };
                let Some(verdict) =
                    super::bounded_cut::pad_top_holds(site, |x, z| {
                        final_terrain.height_millimetres(x, z)
                    })
                else {
                    continue;
                };
                if verdict.holds() {
                    held_by_world.push((*pad_id).to_string());
                }
            }
            apply_entity_pad_refusals(&mut expected, record)?;
            if let Some(pad_id) = held_by_world.first() {
                return Err(TerrainError(format!(
                    "pad {pad_id} is refused under {PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM} but \
                     the published raster holds its declared flat top, so the refusal withdraws a \
                     pad the world realises"
                )));
            }
        }
    }

    if !trusted_record
        && serde_json::to_value(&expected).map_err(|e| TerrainError(e.to_string()))?
            != serde_json::to_value(record).map_err(|e| TerrainError(e.to_string()))?
    {
        return Err(TerrainError(
            "road plan differs from natural raster reconstruction".into(),
        ));
    }
    if !final_terrain.matches_scene(scene) {
        return Err(TerrainError("road audit final terrain mismatch".into()));
    }
    final_terrain.require_complete()?;
    let mut audit = RoadNetworkAudit {
        status: "pass".into(),
        planned_edges: expected.edges.len(),
        realized_edges: 0,
        planned_sites: expected.roadside_sites.len(),
        realized_sites: 0,
        max_final_site_fill_m: 0.,
        max_site_design_height_error_m: 0.,
        max_site_cut_m: None,
        max_final_roadbed_fill_m: 0.,
        max_design_height_error_m: 0.,
        failures: Vec::new(),
    };
    if expected.edges.is_empty() {
        audit
            .failures
            .push("no feasible connected road network".into());
    }
    let mut ids = BTreeMap::<&str, Vec<&EntityInstance>>::new();
    for e in entities {
        ids.entry(&e.entity_id).or_default().push(e);
    }
    for planned in expected.entities() {
        let Some(found) = ids.get(planned.entity_id.as_str()) else {
            audit
                .failures
                .push(format!("missing planned road {}", planned.entity_id));
            continue;
        };
        if found.len() != 1 {
            audit
                .failures
                .push(format!("duplicate road id {}", planned.entity_id));
            continue;
        }
        let e = found[0];
        if e.kind != "road"
            || e.asset_id != planned.asset_id
            || e.height_m != planned.height_m
            || e.world_x != planned.world_x
            || e.world_z != planned.world_z
            || e.width_m != planned.width_m
            || e.depth_m != planned.depth_m
            || e.rotation_y_deg != planned.rotation_y_deg
            || e.scale != 1.
            || serde_json::to_value(&e.grounding).ok()
                != serde_json::to_value(&planned.grounding).ok()
        {
            audit
                .failures
                .push(format!("road transform differs from plan {}", e.entity_id));
            continue;
        }
        // Four ranges, each with its own contract:
        //  * pavement     - the declared road width; held exactly;
        //  * graded band  - what `plan_terrain_carves` flattens; held exactly,
        //                   and it must cover the pavement;
        //  * transport    - pavement + the corridor's 3m shoulder + the end
        //    footprint      platforms; bounded by fill and no-excavation;
        //  * transition   - the carve's blend beyond it; only the fill envelope
        //                   applies, never the design level.
        let (hw, hd) = super::graded_transport_half_extents(e);
        // A structural shortfall is one failure per road, recorded before any
        // sampling, and such a road is never counted as realized.
        if planned.grounding.road_profile.is_some() {
            let (flat_hw, flat_hd) = super::graded_flat_top_half_extents(&planned);
            let flat_transverse = if planned.rotation_y_deg == 90. {
                flat_hw
            } else {
                flat_hd
            };
            let (pavement_hw, pavement_hd) = super::graded_pavement_half_extents(&planned);
            let pavement_transverse = if planned.rotation_y_deg == 90. {
                pavement_hw
            } else {
                pavement_hd
            };
            let shortfall =
                super::graded_band_shortfall_m(pavement_transverse, flat_transverse);
            if shortfall > 0 {
                audit.failures.push(format!(
                    "road {} graded band is {}m narrower than its declared pavement half",
                    planned.entity_id, shortfall
                ));
                continue;
            }
        }
        let mut error = 0_f32;
        let mut error_at = (e.world_x, e.world_z);
        let mut fill = 0_f32;
        let mut cut = 0_f32;
        for z in e.world_z - hd..=e.world_z + hd {
            for x in e.world_x - hw..=e.world_x + hw {
                let n = natural
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing natural roadbed sample".into()))?;
                let f = final_terrain
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing final roadbed sample".into()))?;
                let (paved_hw, paved_hd) = if planned.grounding.road_profile.is_some() {
                    // The design level is required exactly where the earthwork
                    // flattens the road (`graded_flat_top_half_extents`); the
                    // rest of the transport footprint is audited as earthwork
                    // through the fill cap and the no-excavation rule below.
                    // Requiring the design level across the whole padded
                    // footprint graded the blend/apron as pavement, which no
                    // legal graded road on sloping ground can satisfy.
                    let (flat_hw, flat_hd) = super::graded_flat_top_half_extents(&planned);
                    (flat_hw, flat_hd)
                } else if e.rotation_y_deg == 0. {
                    (e.width_m as i32 / 2, 5)
                } else {
                    (5, e.width_m as i32 / 2)
                };
                if (x - e.world_x).abs() <= paved_hw
                    && (z - e.world_z).abs() <= paved_hd
                    // A registered structure deck is not terrain: the pavement is
                    // a span carried by a bridge/causeway, so there is no design
                    // level to hold in the raster. What must hold instead is
                    // audited by `audit_water_crossings` (no earthwork in the
                    // water, deck clears the datum, take-off stays inside the
                    // fill contract) and by the earthwork envelope below, which is
                    // unchanged.
                    && !planned.grounding.structure_deck
                {
                    let target = planned
                        .grounding
                        .road_profile
                        .as_ref()
                        .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
                        .unwrap_or(f64::from(planned.world_y));
                    let sample_error = (f - target as f32).abs();
                    if sample_error > error {
                        error = sample_error;
                        error_at = (x, z);
                    }
                }
                fill = fill.max(f - n);
                cut = cut.max(n - f);
            }
        }
        // A structure deck places NO earthwork of its own: `plan_terrain_carves`
        // refuses to carve it, so any fill or cut this loop reads inside its band
        // belongs to NEIGHBOURING roads' transition rings (a fill-only carve's
        // blend reaches far past its own graded band), not to the span. The span's
        // own contract is both stricter and cell-exact - no earthwork anywhere in
        // the water (`audit_water_crossings`), the deck at or above every dry cell,
        // and no more than the roadbed fill contract above its take-off - so the
        // earthwork maxima are not attributed to it.
        if planned.grounding.structure_deck {
            fill = 0.;
            cut = 0.;
        }
        audit.max_final_roadbed_fill_m = audit.max_final_roadbed_fill_m.max(fill);
        audit.max_design_height_error_m = audit.max_design_height_error_m.max(error);
        let tolerance = if planned.grounding.road_profile.is_some() {
            0.00005
        } else {
            0.26
        };
        let max_fill = planned
            .grounding
            .road_profile
            .as_ref()
            .map_or(expected.max_roadbed_fill_m, |p| {
                p.max_fill_mm as f32 / 1000.
            });
        // OPT-IN bounded cut: a graded road that publishes a cap may excavate its
        // own transport band, at most that cap per cell. The cap is the ENTITY's
        // own (stamped from the plan's `road_cut_mm`), so a road without one keeps
        // the historical no-excavation contract; the per-cell depth proof is
        // `bounded_cut::verify`, which owns the ledger and the exact formula. This
        // audit keeps the fill contract above it unchanged.
        let cut_allowance_m = planned
            .grounding
            .bounded_cut_mm
            .map_or(0_f32, |cap| cap as f32 / 1000.);
        if error > tolerance
            || fill > max_fill + tolerance
            || cut > cut_allowance_m + tolerance
            || e.world_y != planned.world_y
        {
            audit.failures.push(format!(
                "road {} changed after planning: design error {error}m at ({},{}), fill {fill}m, cut {cut}m",
                e.entity_id, error_at.0, error_at.1
            ));
        } else {
            audit.realized_edges += 1;
        }
    }
    // Planned sites are obligations, not decorative hints. Audit their final
    // geometry and the full support pad independently of serialized reports.
    for planned in expected.site_entities() {
        let Some(found) = ids.get(planned.entity_id.as_str()) else {
            audit.failures.push(format!(
                "missing planned roadside site {}",
                planned.entity_id
            ));
            continue;
        };
        if found.len() != 1 {
            audit
                .failures
                .push(format!("duplicate roadside site id {}", planned.entity_id));
            continue;
        }
        let e = found[0];
        let anchors_match = e.anchors.len() == planned.anchors.len()
            && e.anchors.iter().zip(&planned.anchors).all(|(a, b)| {
                a.id == b.id
                    && a.world_x == b.world_x
                    && a.world_z == b.world_z
                    && a.direction == b.direction
                    && a.target == b.target
            });
        if !anchors_match
            || e.kind != planned.kind
            || e.asset_id != planned.asset_id
            || e.world_x != planned.world_x
            || e.world_z != planned.world_z
            || e.world_y != planned.world_y
            || e.grounding.bottom_offset_m != planned.grounding.bottom_offset_m
            || e.grounding.roadbed_offset_m != planned.grounding.roadbed_offset_m
            || e.width_m != planned.width_m
            || e.depth_m != planned.depth_m
            || e.height_m != planned.height_m
            || e.rotation_y_deg != planned.rotation_y_deg
            || e.scale != 1.
        {
            audit.failures.push(format!(
                "roadside site transform differs from plan {}",
                e.entity_id
            ));
            continue;
        }
        let mut error = 0_f32;
        let mut fill = 0_f32;
        let mut cut = 0_f32;
        let (hw, hd) = super::spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
        let (hw, hd) = (hw as i32, hd as i32);
        // The pad's own carve is fill-only and its target is the declared top, so
        // its contribution to any cell is bounded by `declared - natural`. In a
        // waterfront block a legally higher lot can sit inside this pad's 2 m
        // apron and be raised to ITS level; attributing that earthwork to this pad
        // would charge it with fill it never placed. Opt-in only, so every existing
        // audit keeps its numbers.
        let own_cap_m = planned.ground_height_m() + 0.0005;
        for z in e.world_z - hd - 2..=e.world_z + hd + 2 {
            for x in e.world_x - hw - 2..=e.world_x + hw + 2 {
                let n = natural
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing natural site sample".into()))?;
                let f = final_terrain
                    .height(x, z)
                    .ok_or_else(|| TerrainError("missing final site sample".into()))?;
                if (x - e.world_x).abs() <= hw && (z - e.world_z).abs() <= hd {
                    error = error.max((f - planned.ground_height_m()).abs());
                }
                // `water` is the opt-in authorization the whole waterfront path
                // hangs from; every other world keeps the unbounded reading.
                if water.road_water_crossings() {
                    fill = fill.max((f - n).min(own_cap_m - n));
                } else {
                    fill = fill.max(f - n);
                }
                cut = cut.max(n - f);
            }
        }
        audit.max_final_site_fill_m = audit.max_final_site_fill_m.max(fill);
        audit.max_site_design_height_error_m = audit.max_site_design_height_error_m.max(error);
        let precise = expected
            .roadside_sites
            .iter()
            .find(|s| s.id == planned.entity_id)
            .and_then(|s| s.support_level_mm);
        // OPT-IN bounded cut: a pad whose own published record carries a cap is
        // allowed to stand BELOW the natural surface inside that cap - that is
        // the excavation its plan declared, and `bounded_cut::verify` recomputes
        // every cell of it. A pad without a cap keeps the historical fill-only
        // contract, so no world that did not opt in changes here.
        let cut_limit_m: f32 = expected
            .roadside_sites
            .iter()
            .find(|s| s.id == planned.entity_id)
            .and_then(|s| s.bounded_cut_mm)
            .map_or(0., |cap| super::bounded_cut::clamp_cap(cap) as f32 / 1000.);
        // The declared top is held over the FOOTPRINT. A cut pad's own graded ring
        // stands outside it by design (that is how a platform is carved out of a
        // slope instead of leaving a wall one metre thick), so its +2 m audit
        // margin is not a flat-top claim any more; a fill-only pad keeps the full
        // window.
        let mismatch_mm = precise.and_then(|level| {
            let margin = if cut_limit_m > 0. { 0 } else { 2 };
            (e.world_z - hd - margin..=e.world_z + hd + margin).find_map(|z| {
                (e.world_x - hw - margin..=e.world_x + hw + margin).find_map(|x| {
                    let actual = final_terrain.height_millimetres(x, z);
                    (actual != Some(level)).then_some((x, z, level, actual))
                })
            })
        });
        let precision = if precise.is_some() { 0.0001 } else { 0.26 };
        if cut_limit_m > 0. {
            audit.max_site_cut_m = Some(audit.max_site_cut_m.map_or(cut, |known| known.max(cut)));
        }
        if mismatch_mm.is_some()
            || error > precision
            || fill > expected.max_roadbed_fill_m + precision
            || cut > cut_limit_m + precision
        {
            audit.failures.push(format!("roadside site {} changed after planning: design error {error}m, fill {fill}m, cut {cut}m, support sample {mismatch_mm:?}", e.entity_id));
        } else {
            audit.realized_sites += 1;
        }
    }
    if !audit.failures.is_empty() {
        audit.status = "fail".into();
    }
    Ok(audit)
}

/// Prevent a proposed high building platform from flooding an already designed
/// roadbed with its fill transition. Checks natural+candidate engineering,
/// not a looser collision descriptor. Authored assets still fail final audit.
pub(super) fn building_fill_conflicts(
    terrain_context: &super::TerrainContext<'_>,
    roads: &[EntityInstance],
    candidate: &EntityInstance,
) -> bool {
    let carves = super::plan_terrain_carves(std::slice::from_ref(candidate));
    if carves.is_empty() {
        return false;
    }
    for road in roads {
        let (hw, hd) = super::spatial_half_extents(road.width_m, road.depth_m, road.rotation_y_deg);
        let (hw, hd) = (hw.ceil() as i32 + 4, hd.ceil() as i32 + 4);
        if !carves.iter().any(|c| {
            (c.cx - f64::from(road.world_x)).abs() <= c.half_w + c.blend_m + f64::from(hw)
                && (c.cz - f64::from(road.world_z)).abs() <= c.half_d + c.blend_m + f64::from(hd)
        }) {
            continue;
        }
        for z in road.world_z - hd..=road.world_z + hd {
            for x in road.world_x - hw..=road.world_x + hw {
                let h = terrain_context.height(x, z, &carves);
                let target = road
                    .grounding
                    .road_profile
                    .as_ref()
                    .and_then(|p| p.height_at(f64::from(x), f64::from(z)))
                    .unwrap_or(f64::from(road.world_y));
                if f64::from(h) > target + 0.01 {
                    return true;
                }
            }
        }
    }
    false
}

/// One neighbouring pad a candidate's own earthwork would bury, with the numbers
/// that make a refusal checkable instead of plausible.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PadFillConflict {
    /// The already declared pad whose flat top the candidate's own fill raises.
    pub neighbour_id: String,
    /// That pad's declared surface (`ground_height_m`), in metres.
    pub neighbour_top_m: f64,
    /// The highest the candidate's OWN envelope reaches inside that pad's audit
    /// window, in metres.
    pub flooded_m: f64,
    /// The earthwork the candidate places above the natural ground at the worst
    /// cell, in metres.
    pub fill_m: f64,
    /// The cell where the worst of it lands.
    pub at: (i32, i32),
}

/// The worst cell of ONE candidate's fill envelope inside ONE pad's audit window,
/// or `None` when the candidate does not bury that pad.
///
/// The two facts a pad's flat top needs, restated for a FLAT precision surface:
///
///  * The target is the pad's PUBLISHED SURFACE (`ground_height_m`, which keeps a
///    precise pad's millimetre offset), never the metre-floored `world_y`.
///  * Only cells the candidate's own carve raised count. Ground that naturally
///    stands higher than a pad beside it is a bank the lot stands next to, not
///    fill the candidate would place.
#[allow(clippy::too_many_arguments)]
fn pad_fill_worst_cell(
    pad: &EntityInstance,
    carves: &[super::TerrainCarve],
    natural_at: &dyn Fn(i32, i32) -> Option<f32>,
    carved_at: &dyn Fn(i32, i32, &[super::TerrainCarve]) -> Option<f32>,
) -> Option<PadFillConflict> {
    let (hw, hd) = super::spatial_half_extents(pad.width_m, pad.depth_m, pad.rotation_y_deg);
    let (hw, hd) = (hw.ceil() as i32 + 4, hd.ceil() as i32 + 4);
    let target = f64::from(pad.ground_height_m());
    let mut worst: Option<PadFillConflict> = None;
    for z in pad.world_z - hd..=pad.world_z + hd {
        for x in pad.world_x - hw..=pad.world_x + hw {
            let flooded = carved_at(x, z, carves)?;
            if f64::from(flooded) <= target + 0.01 {
                continue;
            }
            let natural = natural_at(x, z)?;
            if f64::from(flooded) <= f64::from(natural) + 0.01 {
                continue;
            }
            let better = worst
                .as_ref()
                .is_none_or(|known| f64::from(flooded) > known.flooded_m);
            if better {
                worst = Some(PadFillConflict {
                    neighbour_id: pad.entity_id.clone(),
                    neighbour_top_m: target,
                    flooded_m: f64::from(flooded),
                    fill_m: f64::from(flooded) - f64::from(natural),
                    at: (x, z),
                });
            }
        }
    }
    worst
}

/// Does this candidate's fill envelope flood an already declared roadside pad?
///
/// The same question [`building_fill_conflicts`] asks of a roadbed, with the two
/// differences a FLAT precision surface needs (see [`pad_fill_worst_cell`]).
/// Opt-in waterfront only: the general function above keeps every other world's
/// placements byte-identically.
pub(super) fn pad_fill_conflicts(
    terrain_context: &super::TerrainContext<'_>,
    pads: &[EntityInstance],
    candidate: &EntityInstance,
) -> bool {
    pad_fill_conflict_of(
        pads,
        candidate,
        &|x, z| Some(terrain_context.height(x, z, &[])),
        &|x, z, carves| Some(terrain_context.height(x, z, carves)),
    )
    .is_some()
}

/// The first pad in `pads` whose declared top this candidate's own fill buries.
pub(super) fn pad_fill_conflict_of(
    pads: &[EntityInstance],
    candidate: &EntityInstance,
    natural_at: &dyn Fn(i32, i32) -> Option<f32>,
    carved_at: &dyn Fn(i32, i32, &[super::TerrainCarve]) -> Option<f32>,
) -> Option<PadFillConflict> {
    let carves = super::plan_terrain_carves(std::slice::from_ref(candidate));
    if carves.is_empty() {
        return None;
    }
    for pad in pads {
        if pad.entity_id == candidate.entity_id {
            continue;
        }
        let (hw, hd) = super::spatial_half_extents(pad.width_m, pad.depth_m, pad.rotation_y_deg);
        let (hw, hd) = (hw.ceil() as i32 + 4, hd.ceil() as i32 + 4);
        let reaches = carves.iter().any(|c| {
            (c.cx - f64::from(pad.world_x)).abs() <= c.half_w + c.blend_m + f64::from(hw)
                && (c.cz - f64::from(pad.world_z)).abs() <= c.half_d + c.blend_m + f64::from(hd)
        });
        if !reaches {
            continue;
        }
        if let Some(conflict) = pad_fill_worst_cell(pad, &carves, natural_at, carved_at) {
            return Some(conflict);
        }
    }
    None
}

/// The stable category a pad is refused under when a NEIGHBOUR's fill is what
/// makes its declared flat top unbuildable.
///
/// This, `PAD_UNREALIZABLE_CATEGORY` and `CONVERSION_EXCEEDS_URBAN_ALLOCATION`
/// are the only roadside categories whose `edgeId` names a pad that WAS in the plan
/// and was taken back out of it again: every other roadside category names a
/// prospective site that was never planned, and two candidates on the same road and
/// side share one prospective id. Audits may therefore assert on these three alone
/// that the pad is absent from the plan, from the conversions, from the ledger and
/// from the published scene - which is how "planned, booked, but absent from the
/// world" is refused by name instead of reappearing as a silent deletion.
pub const PAD_FILL_CONFLICT_CATEGORY: &str = "pad_fill_conflicts_with_neighbour";

/// The same refusal when no neighbour's fill accounts for it: the plan's own
/// earthwork cannot hold the pad's declared top either.
pub const PAD_UNREALIZABLE_CATEGORY: &str = "pad_flat_top_unrealizable";

/// A pad the ENTITY stage took back out of the plan because a rule-engine
/// building's platform stands over its declared top.
///
/// The fourth way a pad that WAS planned may leave the world, and the only one
/// that cannot be decided at planning time: the plan's own earthwork is fine and
/// the entity that buries the pad does not exist yet when the pad list is
/// accepted (`worldgen::pad_completion`). The category is stable and machine
/// readable, and the reason carries the entity id, the cell count and the lift, so
/// an audit can tell this refusal from a planning one.
pub const PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM: &str = "pad_top_conflicts_with_entity_platform";

/// A pad whose declared flat top stands BELOW the surface the world's SETTLED road
/// network already puts on its own footprint.
///
/// The fifth way a pad that WAS planned may leave the world, and the one the
/// acceptance gate could not see: a pad is declared on the surface of the road it
/// fronts (`frontage::append`, and `gate_declared_mm` for the roadside programme),
/// which is the right rule for the road the pad belongs to - but the pad's
/// footprint may also be covered by the fill of a LATER road, whose graded top is
/// a priority-10 platform the pad's own bench may not take back. Grading is
/// fill-only, so a declaration below that surface is a flat top the plan can never
/// hold: the raster publishes the road, `bounded_cut::verify` step 5 fails, and the
/// entity stage could only withdraw the pad after it had earned a slot and a
/// conversion. This category says so at planning time instead, with the road, its
/// surface, the cell count and the lift.
pub const PAD_BELOW_SETTLED_ROAD_PLATFORM: &str = "pad_below_settled_road_platform";

/// Every category a pad that was IN the plan can be taken back out under.
///
/// The third entry is the urban purpose allocation
/// (`worldgen::conversion_budget::CONVERSION_EXCEEDS_URBAN_ALLOCATION`): the pad
/// declared an urban purpose on wholly-rural land and the converted land does not
/// fit what the world planned as urban, so the pad may not stay planned for land
/// the plan never allocated. The fourth is the entity-stage platform refusal and
/// the fifth the settled-road-platform refusal above.
pub const PAD_REFUSED_CATEGORIES: [&str; 5] = [
    PAD_FILL_CONFLICT_CATEGORY,
    PAD_UNREALIZABLE_CATEGORY,
    super::conversion_budget::CONVERSION_EXCEEDS_URBAN_ALLOCATION,
    PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM,
    PAD_BELOW_SETTLED_ROAD_PLATFORM,
];

/// The pad ids the plan refused by name out of an already planned pad list.
///
/// A refused pad is not a parcel of this world any more: it may not be converted,
/// planned, published or booked. Everything downstream reads the set from here so
/// the four views cannot disagree about which pads exist.
pub(crate) fn refused_pad_ids(plan: &RoadNetworkPlan) -> BTreeSet<&str> {
    plan.roadside_site_rejections
        .iter()
        .filter(|rejection| PAD_REFUSED_CATEGORIES.contains(&rejection.category.as_str()))
        .map(|rejection| rejection.edge_id.as_str())
        .collect()
}

/// The surface the BAKE will publish, sampled before it exists.
///
/// A planning gate may not model the earthwork differently from the encoder, and
/// that is exactly how a pad used to be deleted: the old rule compared a
/// candidate's OWN blend ring against a neighbour's pad, which ignores the rule
/// `apply_terrain_carves` enforces - a neighbouring blended fill cannot overwrite a
/// validated hard support surface whose road band reaches the cell. Two pads that
/// rule called mutually exclusive were in fact both realizable, and one of them was
/// removed from the world anyway. The gate therefore asks the question the BAKE and
/// the AUDIT ask - "does the surface all of the plan's earthwork produces hold this
/// pad's declared top?" - and only then looks for a neighbour to name.
enum PlannedSurface<'a> {
    /// The authored procedural field re-derived from the declared seed and
    /// precision: what the production encoder writes.
    Field(super::TerrainContext<'a>),
    /// The supplied natural raster, with the same carve composition on top. Used
    /// when the request did not declare an authored source.
    Raster(super::RasterSurface<'a>),
}
impl PlannedSurface<'_> {
    fn natural(&self, x: i32, z: i32) -> Option<f32> {
        match self {
            Self::Field(context) => Some(context.height(x, z, &[])),
            Self::Raster(raster) => raster.natural_at(x, z),
        }
    }
    fn height(&self, x: i32, z: i32, carves: &[super::TerrainCarve]) -> Option<f32> {
        match self {
            Self::Field(context) => Some(context.height(x, z, carves)),
            Self::Raster(raster) => raster.height(x, z, carves),
        }
    }
}

/// What the plan's own earthwork does to ONE pad's audited flat top.
///
/// The numbers are the audit's, measured the audit's way (its window, its
/// precision, its fill contract), so a pad refused here is a pad the independent
/// audit would have refused afterwards - the difference is that a refusal is named
/// in the plan instead of the pad appearing in it and being deleted from the world.
struct PadWindowBreach {
    declared_m: f64,
    /// The surface at the cell that explains the refusal.
    worst_m: f64,
    at: (i32, i32),
    natural_m: f64,
    /// The deepest excavation anywhere in the window, metres. A fill-only pad
    /// declares no excavation at all, so this is a contract failure even when its
    /// own cells are level; a cap-carrying pad declares one, and only the part of
    /// it that its own cap does not permit can be a failure.
    cut_m: f64,
    /// For a cap-carrying pad whose refusal is an excavation BELOW the deepest
    /// surface its own cap permits: that surface and the cap bounding it.
    /// `None` for the historical fill-only pad, whose record declares no cut.
    floor: Option<(f64, i32)>,
}

/// The audit's own verdict on one pad, computed before the payload exists.
///
/// Two earthwork contracts meet here and the pad's own record decides which one it
/// is judged under:
///
///  * a pad that carries no `boundedCutMm` is FILL-ONLY, exactly as before: every
///    cell of its +-2 m window must publish its declared millimetre level, the fill
///    is bounded by the plan's roadbed contract, and any excavation at all is a
///    contract failure;
///  * a pad that carries a cap DECLARES an excavation. Its `CutFootprint` is the
///    footprint the generator's per-cell guard, the published ledger and
///    `bounded_cut::verify` all read, and `cut_surface_m` is the one surface
///    function they all evaluate - so this gate asks the same question they ask
///    instead of charging the bench as if it were a fill that never happened. The
///    PLATFORM (`frontage::bounds(entity, 0)`, the building itself) still has to
///    hold the declared level exactly, which is the obligation the road audit and
///    `bounded_cut::verify` step 5 re-derive from the published bytes; the graded
///    shoulder outside it may be benched (a bank the one cap cannot reach the level
///    is a designed face, not a flatness defect) and may be cut as deep as the pad's
///    own cap permits - a frontage pad's shoulder reaches the road pavement, where
///    the road's own graded surface legitimately stands below the pad's bench.
fn pad_window_breach(
    site: &RoadsideSite,
    entity: &EntityInstance,
    surface: &PlannedSurface<'_>,
    carves: &[super::TerrainCarve],
    max_roadbed_fill_m: f32,
    waterfront: bool,
) -> Option<PadWindowBreach> {
    // The two windows, both rounded by the one function the placement preflight
    // uses for a pad footprint: `platform` is the building itself (the cells the
    // declared top is an audited obligation over) and `window` the historical
    // +-2 m audit window the fill contract is measured over.
    let platform = super::frontage::bounds(entity, 0);
    let window = super::frontage::bounds(entity, 2);
    let declared = f64::from(entity.ground_height_m());
    // The bench this pad's own record declares, or `None` for a fill-only pad.
    let declared_cut = site
        .bounded_cut_mm
        .map(|cap| super::bounded_cut::CutFootprint::of(entity, cap));
    // The same cap the audit applies to a waterfront pad's own contribution: its
    // carve is fill-only and its target is the declared top, so it can never be
    // charged with earthwork it did not place.
    let own_cap = declared + 0.0005;
    let precision = if site.support_level_mm.is_some() {
        0.0001
    } else {
        0.26
    };
    // A published sample is a whole number of millimetres, so the bench is compared
    // at the audit's own `CUT_MM_TOLERANCE` and not at the metre precision.
    let bench_tolerance = super::bounded_cut::CUT_MM_TOLERANCE as f64 / 1000.;
    let mut error = 0_f64;
    let mut fill = f64::NEG_INFINITY;
    let mut cut = f64::NEG_INFINITY;
    // The deepest cell standing BELOW the deepest surface its own record permits:
    // an excavation the record does not declare, and the only excavation rule 3
    // still refuses for a pad that carries a cap.
    let mut below_floor = 0_f64;
    // The cell that explains each way the window can fail, kept apart so a refusal
    // quotes one coherent cell instead of mixing a deviation from one place with a
    // fill height from another.
    let mut footprint_worst: Option<((i32, i32), f64, f64)> = None;
    let mut fill_worst: Option<((i32, i32), f64, f64)> = None;
    let mut cut_worst: Option<((i32, i32), f64, f64)> = None;
    // cell, published surface there, natural surface there, deepest surface the cap permits there
    let mut floor_worst: Option<((i32, i32), f64, f64, f64)> = None;
    let mut sample_mm = None;
    for z in window[1]..=window[3] {
        for x in window[0]..=window[2] {
            // `bounds` rounds the footprint once, in i64; the samples are i32 cells.
            let (x, z) = (x as i32, z as i32);
            let natural = surface.natural(x, z)?;
            let final_m = surface.height(x, z, carves)?;
            let (natural, final_m) = (f64::from(natural), f64::from(final_m));
            let rise = final_m - natural;
            let own_rise = if waterfront {
                rise.min(own_cap - natural)
            } else {
                rise
            };
            let on_platform = i64::from(x) >= platform[0]
                && i64::from(x) <= platform[2]
                && i64::from(z) >= platform[1]
                && i64::from(z) <= platform[3];
            if on_platform {
                let deviation = (final_m - declared).abs();
                error = error.max(deviation);
                if footprint_worst
                    .as_ref()
                    .is_none_or(|(_, known, _)| deviation > *known)
                {
                    footprint_worst = Some(((x, z), final_m, natural));
                }
            } else if let Some(footprint) = &declared_cut {
                // The graded shoulder. `cut_surface_m` is the ONE formula the
                // encoder, the ledger and the audit read, so the shoulder is
                // compared against the deepest surface this pad's own record can
                // produce there: the same function evaluated at the lowest target
                // its cap permits. Every target in `[natural - cap, declared]` is a
                // surface the record allows, and the cap - not the declared top -
                // is the number the project contracts on, so a shoulder may stand
                // below the bench (a frontage pad's shoulder reaches the road
                // pavement, where the road's own graded surface legitimately does)
                // and still be the pad's own earthwork. A cell ABOVE the floor is a
                // fill or a legal bench, and rule 2 bounds the fill; a cell BELOW it
                // is an excavation no target of this pad could have produced.
                let cap_m = f64::from(footprint.cap_mm) / 1000.;
                let floor = super::bounded_cut::cut_surface_m(
                    natural,
                    natural - cap_m,
                    footprint.outside_m(x, z),
                    footprint.cap_mm,
                );
                let shortfall = floor - final_m;
                if shortfall > below_floor {
                    below_floor = shortfall;
                    floor_worst = Some(((x, z), final_m, natural, floor));
                }
            }
            // Which cells the declared millimetre level is an obligation over: the
            // whole +-2 m window for a fill-only pad (its ring is an apron, not a
            // bench), the building footprint for a pad whose graded shoulder is a
            // designed face.
            if declared_cut.is_none() || on_platform {
                if sample_mm.is_none() {
                    if let Some(level) = site.support_level_mm {
                        let actual = (final_m * 1000.).round() as i32;
                        if actual != level {
                            sample_mm = Some((x, z, level, Some(actual)));
                        }
                    }
                }
            }
            fill = fill.max(own_rise);
            if fill_worst
                .as_ref()
                .is_none_or(|(_, known, _)| own_rise > *known)
            {
                fill_worst = Some(((x, z), final_m, natural));
            }
            let excavation = natural - final_m;
            cut = cut.max(excavation);
            if cut_worst
                .as_ref()
                .is_none_or(|(_, known, _)| excavation > *known)
            {
                cut_worst = Some(((x, z), final_m, natural));
            }
        }
    }
    // The legal cut footprint this pad's record declares, held by the plan's
    // earthwork: `None` when the pad declares no cut at all, or when some cell of
    // its audit window stands BELOW the deepest surface its own cap permits, which
    // is an excavation the record does not declare.
    let cut_footprint = declared_cut.filter(|_| below_floor <= bench_tolerance);
    // The audit's verdict, cell for cell: a precise pad must publish its exact
    // millimetre level, a design error above the precision fails, and the fill and
    // cut contracts are the plan's own published numbers.
    let failing_rule = if sample_mm.is_some() {
        0
    } else if error > precision {
        1
    } else if fill > f64::from(max_roadbed_fill_m) + precision {
        2
    } else if cut_footprint.is_none() && cut > precision {
        3
    } else {
        return None;
    };
    let (at, worst_m, natural_m, floor) = match failing_rule {
        0 => {
            let (x, z, _, actual) = sample_mm?;
            let final_m = f64::from(actual?) / 1000.;
            let natural = surface.natural(x, z)?;
            ((x, z), final_m, f64::from(natural), None)
        }
        1 => {
            let ((x, z), final_m, natural) = footprint_worst?;
            ((x, z), final_m, natural, None)
        }
        2 => {
            let ((x, z), final_m, natural) = fill_worst?;
            ((x, z), final_m, natural, None)
        }
        _ => match floor_worst {
            // The refusal is an excavation below what the pad's own cap permits:
            // quote that cell and the floor, not the deepest legal cut elsewhere.
            Some(((x, z), final_m, natural, floor)) => (
                (x, z),
                final_m,
                natural,
                Some((floor, declared_cut.map_or(0, |f| f.cap_mm))),
            ),
            None => {
                let ((x, z), final_m, natural) = cut_worst?;
                ((x, z), final_m, natural, None)
            }
        },
    };
    Some(PadWindowBreach {
        declared_m: declared,
        worst_m,
        at,
        natural_m,
        cut_m: cut,
        floor,
    })
}

/// Which planned entity's own fill is standing over a refused pad's declared top.
///
/// Attribution only: the DECISION is always made on the surface all of the plan's
/// earthwork produces. This answers "whose fill is standing there" so the refusal
/// can name the neighbour and the height it puts on the ground, at ONE cell.
///
/// The tuple also says whether the attributed entity OWNS that cell - its own
/// carve's graded flat top covers it. That is the one case the ENCODER cannot heal
/// with the pad's own platform (`apply_terrain_carves`: a road-owned cell resolves
/// to the road, `PadPlatform::Active` or not), so it is the case the refusal has to
/// name as a settled ROAD PLATFORM rather than as a neighbour's fill.
fn fill_attribution(
    world: &[EntityInstance],
    pad: &EntityInstance,
    declared_m: f64,
    surface: &PlannedSurface<'_>,
) -> Option<FillAttribution> {
    let (hw, hd) = super::spatial_half_extents(pad.width_m, pad.depth_m, pad.rotation_y_deg);
    // The pad's OWN cells: the flat top is what cannot be held there. Air beyond
    // the footprint is the pad's apron, which the fill contract governs instead.
    let (hw, hd) = (hw as i32, hd as i32);
    let mut best: Option<FillAttribution> = None;
    for entity in world {
        if entity.entity_id == pad.entity_id {
            continue;
        }
        let carves = super::plan_terrain_carves(std::slice::from_ref(entity));
        if carves.is_empty() {
            continue;
        }
        let reaches = carves.iter().any(|carve| {
            (carve.cx - f64::from(pad.world_x)).abs()
                <= carve.half_w + carve.blend_m + f64::from(hw)
                && (carve.cz - f64::from(pad.world_z)).abs()
                    <= carve.half_d + carve.blend_m + f64::from(hd)
        });
        if !reaches {
            continue;
        }
        for z in pad.world_z - hd..=pad.world_z + hd {
            for x in pad.world_x - hw..=pad.world_x + hw {
                let Some(height) = surface.height(x, z, &carves) else {
                    continue;
                };
                let Some(natural) = surface.natural(x, z) else {
                    continue;
                };
                let (height, natural) = (f64::from(height), f64::from(natural));
                if height <= declared_m + 0.005 {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|found| height > found.height_m)
                {
                    // Does this entity's OWN graded flat top cover the cell? That is
                    // the one surface the encoder will not let a pad's platform
                    // displace, so the refusal has to name it as a settled road
                    // platform rather than as a neighbour's fill.
                    let owns = entity.kind == "road"
                        && carves.iter().any(|carve| {
                            (f64::from(x) - carve.cx).abs() <= carve.half_w
                                && (f64::from(z) - carve.cz).abs() <= carve.half_d
                        });
                    best = Some(FillAttribution {
                        entity_id: entity.entity_id.clone(),
                        kind: entity.kind.clone(),
                        height_m: height,
                        natural_m: natural,
                        at: (x, z),
                        owns_cell: owns,
                    });
                }
            }
        }
    }
    best
}

/// Which neighbour's fill stands over a refused pad's declared top, and whether it
/// owns that cell (see [`fill_attribution`]).
struct FillAttribution {
    entity_id: String,
    kind: String,
    height_m: f64,
    natural_m: f64,
    at: (i32, i32),
    /// The neighbour's own graded flat top covers `at`: the encoder cannot publish
    /// the pad's platform there, so the declared top is genuinely unreachable.
    owns_cell: bool,
}

/// Refuse, BY NAME, every planned pad whose declared flat top the plan's own
/// earthwork cannot hold, and take it out of the plan before anything is derived
/// from the pad list.
///
/// A pad's flat top is a published, audited obligation: the road audit measures
/// every pad cell against its declaration and refuses the scene when it does not
/// hold. A pad that cannot hold it is therefore not a pad - and the plan says so
/// here, where the pad list is whole and nothing has been derived from it yet:
///
///  * the refusal is retained in `roadside_site_rejections` under
///    `PAD_FILL_CONFLICT_CATEGORY` (a neighbour's fill stands over the declared
///    top) or `PAD_UNREALIZABLE_CATEGORY` (the plan's own earthwork does), with
///    the fill height, the neighbour it names and the cell it lands on;
///  * the pad leaves `roadside_sites`, and `forget_refused_pad` takes it out of
///    the service outcomes, the joint receipt AND the frontage report - both the
///    outcome that accepted it and the rural-to-urban conversion it earned - so no
///    record reports a facility or a converted parcel that cannot exist;
///  * because this runs BEFORE `frontage::pad_conversions` (which derives its
///    records from the final pad list) and before the development plan, a refused
///    pad never becomes a conversion, a parcel, a building or an entity. Nothing
///    is deleted from a generated world afterwards, because nothing is generated.
///
/// Refusing is monotone - removing an envelope can only lower the surface, so no
/// new breach can appear - and the loop is bounded by the pad count. The numeric
/// contracts are not touched: a pad is refused precisely because its declaration
/// cannot be met under them.
pub(super) fn refuse_unrealizable_pads(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    natural: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    source: super::NaturalSource,
) -> Result<Vec<BaseProfileRejection>, TerrainError> {
    if plan.roadside_sites.is_empty() {
        return Ok(Vec::new());
    }
    let zoning = zones
        .map(|zones| ZoningMask::new(scene, zones))
        .transpose()?;
    let surface = match source {
        super::NaturalSource::AuthoredSeed { seed, precise } => {
            PlannedSurface::Field(super::TerrainContext {
                seed,
                water,
                precise,
                zoning: zoning.as_ref(),
            })
        }
        _ => PlannedSurface::Raster(super::RasterSurface {
            natural,
            water,
            zoning: zoning.as_ref(),
        }),
    };
    let mut rejections: Vec<BaseProfileRejection> = Vec::new();
    loop {
        // The settled ROAD network on its own: the plan's edges, normalised exactly
        // as the pads are, and never a pad's carve. The road-platform gate below
        // measures the world's own transport decision, not the earthwork this pass
        // is still deciding.
        let mut roads: Vec<EntityInstance> = plan.entities();
        for road in &mut roads {
            road.normalize_spatial_semantics();
        }
        let pads: Vec<EntityInstance> = plan.site_entities();
        let mut world = roads.clone();
        world.extend(pads.iter().cloned());
        for entity in &mut world {
            entity.normalize_spatial_semantics();
        }
        let carves = super::plan_terrain_carves(&world);
        let waterfront = water.road_water_crossings();
        let mut breach: Option<(usize, PadWindowBreach)> = None;
        for (index, site) in plan.roadside_sites.iter().enumerate() {
            if let Some(found) = pad_window_breach(
                site,
                &pads[index],
                &surface,
                &carves,
                plan.max_roadbed_fill_m,
                waterfront,
            ) {
                breach = Some((index, found));
                break;
            }
        }
        let Some((index, found)) = breach else { break };
        let site = &plan.roadside_sites[index];
        let pad = &pads[index];
        let units = format!("{}x{}", site.width_m, site.depth_m);
        let (category, reason) = window_refusal(
            site,
            pad,
            &found,
            &world,
            &surface,
            &carves,
            &units,
            plan.max_roadbed_fill_m,
        );
        let pad_id = site.id.clone();
        rejections.push(BaseProfileRejection {
            edge_id: pad_id.clone(),
            category: category.into(),
            reason,
        });
        forget_refused_pad(plan, &pad_id);
    }
    plan.roadside_site_rejections.extend(rejections.clone());
    Ok(rejections)
}

/// The published category and reason of one plan-time pad refusal.
///
/// Two categories, because two different decisions can be the reason:
///
///  * [`PAD_BELOW_SETTLED_ROAD_PLATFORM`] when the fill standing over the declared
///    top belongs to a road that OWNS the cell (its graded flat top covers it).
///    That is the one case the encoder cannot heal with the pad's own platform -
///    `apply_terrain_carves` resolves a road-owned cell to the road, whatever
///    `PadPlatform` says - so the pad's declaration is unreachable and the refusal
///    carries the road, its surface, the cell count and the lift;
///  * [`PAD_FILL_CONFLICT_CATEGORY`] for every other neighbour's fill, and
///    [`PAD_UNREALIZABLE_CATEGORY`] when no neighbour accounts for it. Their
///    wording is untouched, so no world that is refused today changes a byte.
fn window_refusal(
    site: &RoadsideSite,
    pad: &EntityInstance,
    found: &PadWindowBreach,
    world: &[EntityInstance],
    surface: &PlannedSurface<'_>,
    carves: &[super::TerrainCarve],
    units: &str,
    max_fill_m: f32,
) -> (&'static str, String) {
    let attributed = fill_attribution(world, pad, found.declared_m, surface);
    // A cap-carrying pad's refusal can be an excavation BELOW the deepest
    // surface its own cap permits; that cell and the cap are quoted by name. The
    // note is empty for a fill-only pad, so its refusal keeps the exact wording -
    // and therefore the exact bytes - it always had.
    let floor_note = found.floor.map_or_else(String::new, |(floor_m, cap)| {
        format!(
            ", of which {:.3} m stands below the deepest surface the pad's own {cap} mm cap \
             permits at ({}, {}) - {floor_m:.3} m",
            floor_m - found.worst_m,
            found.at.0,
            found.at.1
        )
    });
    match &attributed {
        Some(found_fill) if found_fill.owns_cell && found_fill.kind == "road" => {
            let (cells, total_cells) = cells_above(surface, pad, found.declared_m, carves);
            (
                PAD_BELOW_SETTLED_ROAD_PLATFORM,
                format!(
                    "pad {} ({units}, declared top {:.3} m on road {}) cannot hold its flat top \
                     against the SETTLED road network: {} owns the cell at ({}, {}) with its graded \
                     platform and publishes {:.3} m there, {:.3} m of fill above the natural {:.3} m \
                     and {:.3} m over that declared top across {cells} of {total_cells} footprint \
                     cells, and a road-owned cell is the one surface a pad's own platform may not \
                     displace - so the pad is refused here by name instead of being deleted from the \
                     world after planning",
                    site.id,
                    found.declared_m,
                    site.road_id,
                    found_fill.entity_id,
                    found_fill.at.0,
                    found_fill.at.1,
                    found_fill.height_m,
                    found_fill.height_m - found_fill.natural_m,
                    found_fill.natural_m,
                    found_fill.height_m - found.declared_m,
                ),
            )
        }
        Some(found_fill) => (
            PAD_FILL_CONFLICT_CATEGORY,
            format!(
                "pad {} ({units}, declared top {:.3} m on road {}) cannot hold its flat top: \
                 {} stands {:.3} m at ({}, {}), {:.3} m of fill above the natural {:.3} m there and \
                 {:.3} m over that declared top, so the pad is refused here by name instead of \
                 being deleted from the world after planning",
                site.id,
                found.declared_m,
                site.road_id,
                found_fill.entity_id,
                found_fill.height_m,
                found_fill.at.0,
                found_fill.at.1,
                found_fill.height_m - found_fill.natural_m,
                found_fill.natural_m,
                found_fill.height_m - found.declared_m,
            ),
        ),
        None => (
            PAD_UNREALIZABLE_CATEGORY,
            format!(
                "pad {} ({units}, declared top {:.3} m on road {}) cannot hold its flat top: \
                 the plan's own earthwork leaves {:.3} m at ({}, {}) in its audit window \
                 ({:.3} m of fill above the natural {:.3} m there, {:.3} m off the declaration, \
                 cut {:.3} m, against the {max_fill_m} m fill contract{floor_note}) and no \
                 neighbour's fill accounts for it",
                site.id,
                found.declared_m,
                site.road_id,
                found.worst_m,
                found.at.0,
                found.at.1,
                found.worst_m - found.natural_m,
                found.natural_m,
                found.worst_m - found.declared_m,
                found.cut_m,
            ),
        ),
    }
}

/// How many cells of one pad's own platform stand above its declared top, and how
/// many the platform has. Read by the settled-road refusal so it can quote the
/// count the way every other refusal quotes its cell.
fn cells_above(
    surface: &PlannedSurface<'_>,
    pad: &EntityInstance,
    declared_m: f64,
    carves: &[super::TerrainCarve],
) -> (u32, u32) {
    let platform = super::frontage::bounds(pad, 0);
    let (mut cells, mut total) = (0_u32, 0_u32);
    for z in platform[1]..=platform[3] {
        for x in platform[0]..=platform[2] {
            let (x, z) = (x as i32, z as i32);
            total += 1;
            if surface
                .height(x, z, carves)
                .is_some_and(|h| f64::from(h) > declared_m + 0.005)
            {
                cells += 1;
            }
        }
    }
    (cells, total)
}

/// Refuse, BY NAME, one planned pad whose declared flat top the ENTITY earthwork
/// does not hold, and take it out of every record derived from the pad list.
///
/// This is the entity-stage half of [`refuse_unrealizable_pads`]: the pad list is
/// already whole and the plan's own earthwork already verified, but a rule-engine
/// building platform stands over the pad (`worldgen::pad_completion` decided it
/// cannot be deepened within the one cap). The refusal is published under
/// [`PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM`] - a category of
/// [`PAD_REFUSED_CATEGORIES`], so every existing reader of "a pad that was planned
/// and is not planned any more" picks it up without a new rule - and the pad
/// leaves `roadside_sites`, the frontage report, the service outcomes, the joint
/// receipt, the conversion records and (through the caller) the entity list.
///
/// Returns whether the pad was still planned; refusing a pad twice is a no-op, so
/// the bounded completion loop cannot count one pad as two refusals.
pub(crate) fn refuse_pad_top_conflict(
    plan: &mut RoadNetworkPlan,
    pad_id: &str,
    reason: String,
) -> bool {
    if !plan.roadside_sites.iter().any(|site| site.id == pad_id) {
        return false;
    }
    plan.roadside_site_rejections.push(BaseProfileRejection {
        edge_id: pad_id.to_string(),
        category: PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM.into(),
        reason,
    });
    forget_refused_pad(plan, pad_id);
    true
}

/// Apply the ENTITY-stage pad refusals a PUBLISHED plan carries to a plan that was
/// rebuilt from the manifest alone, and name every way that can be wrong.
///
/// The road audit rebuilds the plan from the natural source and compares it with
/// the record byte for byte, which is what makes a hand-edited plan impossible.
/// A pad refused at the entity stage is the one decision that rebuild cannot
/// reproduce - the rebuild has no entities, so no platform stands over the pad -
/// so the same refusal is applied to the rebuild before the comparison, using the
/// record's own published category and reason. Two things keep that from hiding a
/// pad:
///
///  * the refused pad must be a pad the REBUILD plans (a refusal that names a pad
///    no plan produces is refused by name here), so a refusal cannot invent a
///    pad; and
///  * the caller re-derives the refusal's own justification from the published
///    bytes (the pad's declared top must NOT be held by the published raster -
///    see `verify_impl`), so a refusal cannot be written over a pad the world
///    actually realises.
fn apply_entity_pad_refusals(
    expected: &mut RoadNetworkPlan,
    record: &RoadNetworkPlan,
) -> Result<(), TerrainError> {
    let refusals: Vec<BaseProfileRejection> = record
        .roadside_site_rejections
        .iter()
        .filter(|rejection| rejection.category == PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM)
        .cloned()
        .collect();
    for rejection in refusals {
        let planned = expected
            .roadside_sites
            .iter()
            .any(|site| site.id == rejection.edge_id);
        if !planned {
            return Err(TerrainError(format!(
                "pad {} is refused under {} but the natural-source rebuild does not plan it, so \
                 the refusal names a pad no plan produces",
                rejection.edge_id, rejection.category
            )));
        }
        if !refuse_pad_top_conflict(expected, &rejection.edge_id, rejection.reason) {
            return Err(TerrainError(format!(
                "pad {} is refused under {} but could not be withdrawn from the rebuild",
                rejection.edge_id, rejection.category
            )));
        }
    }
    Ok(())
}

/// The pads one plan refused under [`PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM`].
pub(crate) fn entity_refused_pad_ids(plan: &RoadNetworkPlan) -> Vec<&str> {
    plan.roadside_site_rejections
        .iter()
        .filter(|rejection| rejection.category == PAD_TOP_CONFLICTS_WITH_ENTITY_PLATFORM)
        .map(|rejection| rejection.edge_id.as_str())
        .collect()
}

/// Take one refused pad out of every record the plan derives from its pad list.
///
/// A refusal is only honest if the plan stops claiming the pad EVERYWHERE: a
/// service outcome that still lists it would report the facility as planned but
/// unrealized (a phantom), the joint receipt would keep a residential slot for a
/// lot that no longer exists, and the along-road frontage report would keep both
/// its accepted-slot record and the rural-to-urban conversion that record earned -
/// which is precisely how a refused pad used to reach the ledger as a parcel the
/// plan does not plan. All of those counts are recomputed from the sites that
/// remain, never decremented by hand, and the frontage report's own tables are
/// withdrawn through the report's single `withdraw_site` operation.
///
/// The roadside pad programme's own conversion records (`plan.pad_conversions`,
/// derived by `frontage::pad_conversions` from the FINAL pad list) move with it:
/// a pad refused by `refuse_unrealizable_pads` was removed BEFORE they were
/// derived and never earned one, but a pad refused at the ENTITY stage
/// (`worldgen::pad_completion`) was already granted its record, and keeping it
/// would book rural-to-urban area for a lot with no entity. The urban allocation
/// is left as the plan spent it - the budget is a ceiling, not an identity, and
/// re-granting a withdrawn pad's area would silently reorder every other pad's
/// decision.
fn forget_refused_pad(plan: &mut RoadNetworkPlan, pad_id: &str) {
    plan.roadside_sites.retain(|site| site.id != pad_id);
    plan.pad_conversions
        .retain(|record| record.parcel_id != pad_id);
    if let Some(report) = plan.frontage.as_mut() {
        report.withdraw_site(pad_id);
    }
    for outcome in &mut plan.service_outcomes {
        outcome.planned_site_ids.retain(|id| id != pad_id);
        outcome.unfilled_count = outcome
            .request
            .count
            .saturating_sub(outcome.planned_site_ids.len() as u32);
        outcome.status = if outcome.unfilled_count == 0 {
            "planned"
        } else {
            "unfilled"
        }
        .into();
    }
    let residential = plan
        .roadside_sites
        .iter()
        .filter(|site| site.program.is_none())
        .count();
    if let Some(joint) = &mut plan.joint_layout {
        joint.retained_residential_ids.retain(|id| id != pad_id);
        joint.displaced_residential_ids.retain(|id| id != pad_id);
        joint.replacement_residential_ids.retain(|id| id != pad_id);
        joint.unfilled_residential = joint.residential_minimum.saturating_sub(residential);
    }
}

#[cfg(test)]
mod pad_refusal_tests {
    //! A pad whose declared flat top the plan's own earthwork cannot hold is
    //! REFUSED BY NAME and taken out of the plan before anything is derived from
    //! it - never planned, never converted, never published and then deleted.
    //!
    //! Round 23 measured the defect these tests lock: a waterfront pad was planned,
    //! declared urban, converted 468 m2 - and then its entity was deleted from the
    //! world by a rule that compared the candidate's own blend ring against a
    //! neighbour, ignoring the hard-support rule the encoder applies. The plan and
    //! the ledger kept a pad the world did not carry. The fixture below is a plain
    //! declaration on flat ground, so the arithmetic is exact and can be asserted
    //! to the millimetre.
    use super::*;
    use crate::worldgen::{NaturalSource, WaterKind, water_geometry};

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "pads".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    /// Flat ground at the datum: every sample is 0 m, so the only height on the
    /// map is the earthwork the plan itself places.
    fn flat_terrain(scene: &SceneSpec) -> BakedTerrain {
        let mut terrain = BakedTerrain::new(scene).unwrap();
        terrain
            .insert_chunk(0, 0, 512, 512, &vec![0_u8; 512 * 512 * 2])
            .unwrap();
        terrain
    }

    fn dry_water(scene: &SceneSpec) -> WaterGeometry {
        water_geometry(WaterKind::None, &[], scene, &serde_json::Value::Null)
    }

    fn rural_zones() -> Vec<LandUseZone> {
        vec![LandUseZone {
            id: "rural".into(),
            kind: "rural".into(),
            world_x: 0,
            world_z: 0,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }]
    }

    /// The same rural scene with the CITY the conversion has to continue: an urban band on the north
    /// edge, close enough that the fixture's pads (whose buildable cores start at z = 89) stand inside
    /// the 64 m urban fringe (`conversion_fringe`). Without a city edge there is nothing for an
    /// explicit conversion to extend, and the policy refuses it by name - which is a different test.
    fn rural_with_city() -> Vec<LandUseZone> {
        vec![
            LandUseZone {
                id: "rural".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: 40,
                width_m: 512,
                depth_m: 472,
                source: "test".into(),
            },
            LandUseZone {
                id: "north-city".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 512,
                depth_m: 40,
                source: "test".into(),
            },
        ]
    }

    /// A 24x24 pad site on the legacy 2/2/2 parcel: its 22x22 = 484 m2 buildable
    /// core is what a conversion may take.
    fn site(id: &str, x: i32, z: i32, support_level_m: i32, program: bool) -> serde_json::Value {
        let program = if program {
            serde_json::json!({"requestId": "church", "kind": "church", "heightM": 10})
        } else {
            serde_json::Value::Null
        };
        serde_json::json!({
            "id": id,
            "roadId": "road",
            "worldX": x,
            "worldZ": z,
            "widthM": 24,
            "depthM": 24,
            "supportLevelM": support_level_m,
            "rotationYDeg": 0.,
            "naturalMinM": 0.,
            "naturalMaxM": 0.,
            "purpose": "urban",
            "program": program,
        })
    }

    fn plan(sites: Vec<serde_json::Value>, service_sites: u32) -> RoadNetworkPlan {
        let mut plan: RoadNetworkPlan = serde_json::from_value(serde_json::json!({
            "sceneId": "pads",
            "model": "test",
            "naturalHeightfieldHash": "test",
            "status": "planned_connected_network",
            "gridSpacingM": 64,
            "maxRoadbedFillM": 6.,
            "levelM": 0,
            "candidates": 1,
            "rejectedWater": 0,
            "rejectedRelief": 0,
            "unselectedCandidates": 0,
            "nodes": [],
            "edges": [],
            "roadsideSites": sites,
        }))
        .unwrap();
        if service_sites > 0 {
            let planned: Vec<String> = plan
                .roadside_sites
                .iter()
                .filter(|site| site.program.is_some())
                .map(|site| site.id.clone())
                .collect();
            plan.service_outcomes = vec![
                serde_json::from_value(serde_json::json!({
                    "request": {
                        "id": "church-request", "sceneId": "pads", "kind": "church",
                        "count": service_sites, "widthM": 20, "depthM": 24, "heightM": 10,
                    },
                    "plannedSiteIds": planned,
                    "unfilledCount": 0,
                    "rejectedCandidates": {},
                    "status": "planned",
                }))
                .unwrap(),
            ];
        }
        plan
    }

    /// The two pads of the exact conflict: a 5.000 m platform 30 m from a 4.000 m
    /// one, on flat ground. Both are legal alone; together the higher one's fill
    /// ring reaches 4.970 m inside the lower one's own footprint, 0.970 m over its
    /// declaration, which no pad audit can accept.
    fn conflicting_plan() -> RoadNetworkPlan {
        plan(
            vec![
                site("pad.high", 400, 100, 5, false),
                site("pad.low", 430, 100, 4, false),
            ],
            0,
        )
    }

    fn refuse(plan: &mut RoadNetworkPlan) -> Vec<BaseProfileRejection> {
        let scene = scene();
        let terrain = flat_terrain(&scene);
        // The fixture's raster IS the declared natural surface, so the sweep must
        // read it instead of re-deriving the procedural field: the arithmetic in
        // these tests is the plan's own earthwork on flat ground.
        refuse_unrealizable_pads(
            plan,
            &scene,
            &terrain,
            dry_water(&scene),
            None,
            NaturalSource::SuppliedRaster,
        )
        .unwrap()
    }

    #[test]
    fn a_pad_the_plan_cannot_hold_is_refused_by_name_with_the_neighbour_and_its_numbers() {
        let mut plan = conflicting_plan();
        let rejections = refuse(&mut plan);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        let rejection = &rejections[0];
        // The LOWER pad is the one that cannot hold its declaration: generated
        // earthwork is fill-only, so the higher platform's ring stands inside its
        // footprint. It is named, and it is gone from the plan - not published and
        // then deleted.
        assert_eq!(rejection.edge_id, "pad.low");
        assert_eq!(rejection.category, PAD_FILL_CONFLICT_CATEGORY);
        let reason = &rejection.reason;
        for expected in [
            "pad.low",
            "(418, 88)",
            "declared top 4.000 m",
            "pad.high stands 4.970 m",
            "0.970 m over that declared top",
            "4.970 m of fill above the natural 0.000 m",
        ] {
            assert!(
                reason.contains(expected),
                "the refusal must carry {expected:?}: {reason}"
            );
        }
        // The plan no longer claims the pad anywhere.
        assert_eq!(
            plan.roadside_sites
                .iter()
                .map(|site| site.id.as_str())
                .collect::<Vec<_>>(),
            vec!["pad.high"]
        );
        assert_eq!(plan.site_entities().len(), 1);
        assert_eq!(plan.roadside_site_rejections, rejections);
        // The triangle, at the plan level: the pads the planner decided on are
        // either kept or named. Two pads went in, one is planned, one is refused.
        assert_eq!(
            plan.roadside_sites.len() + rejections.len(),
            2,
            "pad count = planned pads + named refusals"
        );
        // And no conversion is derived for it: the records the ledger applies come
        // from the FINAL pad list, so a refused pad cannot be booked.
        let scene = scene();
        // The conversion spends the plan's URBAN purpose allocation, so the fixture needs urban land
        // to spend, AND that land has to be the city the conversion continues: the pads' cores start
        // at z = 89, so a 40 m urban band on the north edge is 49 m away - inside the 64 m urban
        // fringe (`conversion_fringe`). (A world that allocates no urban land at all can convert
        // nothing - its ledger would be overcredited by any conversion - and a world whose urban land
        // is further than the band away can convert nothing either.)
        let zones = rural_with_city();
        let conversions = super::super::frontage::pad_conversions(
            &plan,
            &scene,
            &zones,
            super::super::frontage::RuralConversion::RuralToUrban,
            0.0,
        )
        .unwrap()
        .records;
        assert_eq!(
            conversions,
            vec![super::super::frontage::Conversion::rural_to_urban(
                "pad.high", 484
            )],
            "only the pad the plan keeps earns a conversion"
        );
        assert!(
            conversions.iter().all(|record| record.parcel_id != "pad.low"),
            "a refused pad may not be booked"
        );
    }

    #[test]
    fn a_refused_frontage_pad_takes_its_conversion_and_its_outcome_with_it() {
        // Round 28: the refused pad left its rural-to-urban conversion in the
        // frontage report, so the ledger's union named a parcel the plan no longer
        // carried and the whole bake was refused. The pad is withdrawn from the
        // report's OWN tables by identity, in one operation.
        let mut plan = conflicting_plan();
        let report: super::super::frontage::Report = serde_json::from_value(serde_json::json!({
            "version": 1,
            "model": "local-road-station-frontage-v1",
            "outcomes": [{
                "request": {
                    "id": "homes", "sceneId": "pads", "purpose": "urban",
                    "kind": "residential_home", "count": 2, "widthM": 24, "depthM": 24,
                    "heightM": 8, "stationSpacingM": 16, "setbackM": 8,
                    "maxCandidateAttempts": 16
                },
                "siteIds": ["pad.high", "pad.low"],
                "attempted": 2, "unfilled": 0, "rejected": {},
                "stopReason": "demand_satisfied"
            }],
            "conversions": [
                {"parcelId": "pad.high", "from": "rural", "to": "urban", "areaM2": 484},
                {"parcelId": "pad.low", "from": "rural", "to": "urban", "areaM2": 484}
            ]
        }))
        .unwrap();
        plan.frontage = Some(report.clone());
        let rejections = refuse(&mut plan);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        assert_eq!(rejections[0].edge_id, "pad.low");
        let report = plan.frontage.as_ref().expect("the report survives the refusal");
        assert_eq!(report.outcomes[0].site_ids, vec!["pad.high"]);
        assert_eq!(
            report.outcomes[0].unfilled, 1,
            "demand is recomputed from the sites that remain"
        );
        assert_eq!(
            report.outcomes[0].stop_reason,
            "no_feasible_frontage",
            "the request can no longer claim its demand was satisfied"
        );
        assert_eq!(
            report.conversions,
            vec![super::super::frontage::Conversion::rural_to_urban(
                "pad.high", 484
            )],
            "the refused pad's conversion is withdrawn with it - and only that one"
        );
        // The audit primitive resolves the kept record and names the withdrawn one.
        let scene = scene();
        // The city the kept pad's conversion continues, as above: its core is 49 m from the urban
        // band, so `verify_conversion` resolves the kept record and its area.
        let zones = rural_with_city();
        assert_eq!(
            super::super::frontage::verify_conversion(
                &plan,
                &zones,
                None,
                &super::super::frontage::Conversion::rural_to_urban("pad.low", 484)
            )
            .unwrap_err(),
            "conversion parcel pad.low is not a planned frontage parcel"
        );
        assert_eq!(
            super::super::frontage::verify_conversion(
                &plan,
                &zones,
                None,
                &report.conversions[0]
            )
            .unwrap(),
            484
        );
        // Two runs of the gate and the withdrawal produce identical bytes.
        let mut again = conflicting_plan();
        again.frontage = Some(report.clone());
        let second_rejections = refuse(&mut again);
        assert_eq!(second_rejections, rejections);
        assert_eq!(
            serde_json::to_vec(&again).unwrap(),
            serde_json::to_vec(&plan).unwrap()
        );
    }

    #[test]
    fn a_refused_service_pad_is_reported_unfilled_instead_of_becoming_a_phantom() {
        let mut plan = plan(
            vec![
                site("pad.high", 400, 100, 5, false),
                site("pad.service", 430, 100, 4, true),
            ],
            1,
        );
        assert_eq!(plan.service_outcomes[0].planned_site_ids.len(), 1);
        let rejections = refuse(&mut plan);
        assert_eq!(rejections.len(), 1);
        assert_eq!(rejections[0].edge_id, "pad.service");
        // The request is not "planned" any more: the outcome says unfilled, so the
        // service audit reports 0 of 1 instead of a facility that cannot exist.
        let outcome = &plan.service_outcomes[0];
        assert!(outcome.planned_site_ids.is_empty(), "{outcome:?}");
        assert_eq!(outcome.unfilled_count, 1);
        assert_eq!(outcome.status, "unfilled");
    }

    #[test]
    fn pads_that_can_coexist_keep_the_plan_byte_identical() {
        // Same level, far apart, and a pair whose rings overlap but whose
        // declarations both hold: nothing may move.
        let mut plan = plan(
            vec![
                site("pad.a", 100, 100, 5, false),
                site("pad.b", 400, 400, 3, false),
                site("pad.c", 160, 100, 5, false),
            ],
            0,
        );
        let before = serde_json::to_value(&plan).unwrap();
        let rejections = refuse(&mut plan);
        assert!(rejections.is_empty(), "{rejections:?}");
        assert_eq!(serde_json::to_value(&plan).unwrap(), before);
        assert_eq!(plan.site_entities().len(), 3);
    }

    #[test]
    fn a_pad_platform_the_contract_forbids_is_refused_without_inventing_a_neighbour() {
        // A pad declared 7 m above the flat ground is over the 6 m fill contract
        // even with no neighbour at all: it must still be refused, by the other
        // name, with its own fill number.
        let mut plan = plan(vec![site("pad.tall", 200, 200, 4, false)], 0);
        plan.roadside_sites[0].support_level_m = 7;
        let rejections = refuse(&mut plan);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        assert_eq!(rejections[0].category, PAD_UNREALIZABLE_CATEGORY);
        assert_eq!(rejections[0].edge_id, "pad.tall");
        assert!(
            rejections[0]
                .reason
                .contains("7.000 m of fill above the natural 0.000 m")
                && rejections[0].reason.contains("against the 6 m fill contract"),
            "the refusal must carry the fill it would need and the contract it breaks: {}",
            rejections[0].reason
        );
        assert!(plan.roadside_sites.is_empty());
    }

    // ---------------------------------------------------------------------------
    // The gate made road-aware: the SETTLED road network's own graded platform is
    // carried into the pad acceptance verdict, so the refusal names the road, its
    // surface, the cells it owns and the lift - and a road's blended fill, which the
    // pad's own platform legitimately displaces, still is not a refusal.
    // ---------------------------------------------------------------------------

    /// A 64x8 flat road on the plan's `level_m`, exactly as `plan.entities()`
    /// publishes one: `plan_terrain_carves` reads it as a 36 x 5.5 m graded flat top
    /// with a 6 m taper outside it.
    fn road_edge(id: &str, x: i32, z: i32) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "from": 0,
            "to": 1,
            "worldX": x,
            "worldZ": z,
            "lengthM": 64,
            "rotationYDeg": 0.,
            "naturalMinM": 0.,
            "naturalMaxM": 0.,
        })
    }

    fn plan_with_road(
        level_m: i32,
        sites: Vec<serde_json::Value>,
        road: serde_json::Value,
    ) -> RoadNetworkPlan {
        let mut plan = plan(sites, 0);
        plan.edges = serde_json::from_value(serde_json::json!([road])).unwrap();
        plan.level_m = Some(level_m);
        plan
    }

    /// Requirement 1: a pad whose declared flat top stands below the graded PLATFORM
    /// of a road the plan already settled is refused AT PLANNING TIME, by the road's
    /// own name, with the platform height, the cell count and the lift - never
    /// planned, published and then withdrawn from the world.
    #[test]
    fn a_settled_road_platform_over_a_pad_is_refused_at_plan_time_by_name() {
        let terrain = level_terrain(0);
        let water = dry_water(&scene());
        // A 24x24 pad declared at 5.000 m under a 64x8 road graded to 6.000 m: the
        // road's flat top covers 11 rows of the pad's 25x25 platform, and on those
        // cells the encoder resolves the cell to the road, so 5.000 m is unreachable.
        let mut plan = plan_with_road(
            6,
            vec![frontage_pad("pad.under-road", 200, 100, 5000, Some(2000))],
            road_edge("road.main", 200, 100),
        );
        let rejections = refuse_on(&mut plan, &terrain, water);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        let rejection = &rejections[0];
        assert_eq!(rejection.edge_id, "pad.under-road");
        assert_eq!(
            rejection.category, PAD_BELOW_SETTLED_ROAD_PLATFORM,
            "the settled road platform is the reason, not a neighbour's fill: {}",
            rejection.reason
        );
        for expected in [
            "pad.under-road",
            "declared top 5.000 m on road road",
            "road.main owns the cell at (188, 95)",
            "publishes 6.000 m there",
            "6.000 m of fill above the natural 0.000 m",
            "1.000 m over that declared top",
            "across 275 of 625 footprint cells",
            "a road-owned cell is the one surface a pad's own platform may not displace",
        ] {
            assert!(
                rejection.reason.contains(expected),
                "the refusal must carry {expected:?}: {}",
                rejection.reason
            );
        }
        // The plan no longer claims the pad anywhere.
        assert!(plan.roadside_sites.is_empty());
        assert!(plan.site_entities().is_empty());
        assert_eq!(plan.roadside_site_rejections, rejections);
        assert!(refused_pad_ids(&plan).contains("pad.under-road"));
    }

    /// Requirement 2: a road's TAPERED fill is not a platform. The same road's blend
    /// reaches the pad but owns none of its cells, the pad's own platform displaces
    /// it, and the gate must leave the plan exactly as it found it - byte for byte.
    #[test]
    fn a_road_taper_over_a_pad_window_is_not_a_refusal_and_moves_no_byte() {
        let terrain = level_terrain(0);
        let water = dry_water(&scene());
        // 18 m north of the road: the pad's platform (z 106..130) is outside the
        // road's flat top (z <= 105) and inside its 6 m taper, so the taper stands
        // ~5.96 m over a 5.000 m declaration - and the platform still wins.
        let mut plan = plan_with_road(
            6,
            vec![frontage_pad("pad.beside-road", 200, 118, 5000, Some(2000))],
            road_edge("road.main", 200, 100),
        );
        // The taper really is above the declaration before the gate runs: this is the
        // surface a gate that compared against the taper would refuse on.
        let entity = plan.roadside_sites[0].entity();
        let mut road_only = plan.entities();
        for road in &mut road_only {
            road.normalize_spatial_semantics();
        }
        let road_carves = super::super::plan_terrain_carves(&road_only);
        let taper_mm =
            (super::super::apply_terrain_carves(0., 200, 106, &road_carves) * 1000.).round() as i32;
        assert!(
            taper_mm > 5000,
            "the fixture must stand under the road's taper: {taper_mm} mm"
        );
        // No road cell owns any part of the pad.
        let owns = road_carves.iter().any(|carve| {
            (200. - carve.cx).abs() <= carve.half_w && (106. - carve.cz).abs() <= carve.half_d
        });
        assert!(!owns, "the fixture must be outside the road's graded flat top");
        let _ = entity;
        let before = serde_json::to_vec(&plan).unwrap();
        let rejections = refuse_on(&mut plan, &terrain, dry_water(&scene()));
        assert!(
            rejections.is_empty(),
            "a road taper is not a settled platform: {rejections:?}"
        );
        assert_eq!(
            serde_json::to_vec(&plan).unwrap(),
            before,
            "the plan is byte-identical: a taper never refuses a pad"
        );
        // And the pad keeps its own declared top: the platform is what publishes.
        assert_eq!(
            planned_mm(&plan, 0., 200, 118),
            5000,
            "the pad's platform displaces the taper, cell for cell"
        );
    }

    /// Requirement 3: the refusal is ONE operation over every record derived from the
    /// pad list - plan, service outcomes, joint receipt, frontage report and the
    /// rural-to-urban conversion the pad earned.
    #[test]
    fn a_pad_refused_for_a_road_platform_leaves_every_record_with_it() {
        let terrain = level_terrain(0);
        let water = dry_water(&scene());
        let mut plan = plan_with_road(
            6,
            vec![frontage_pad("pad.under-road", 200, 100, 5000, Some(2000))],
            road_edge("road.main", 200, 100),
        );
        plan.pad_conversions = vec![super::super::frontage::Conversion::rural_to_urban(
            "pad.under-road",
            484,
        )];
        plan.joint_layout = Some(super::super::joint_frontage::JointLayout {
            model: "test".into(),
            residential_minimum: 2,
            baseline_residential_count: 1,
            retained_residential_ids: vec!["pad.under-road".into()],
            displaced_residential_ids: vec![],
            replacement_residential_ids: vec![],
            unfilled_residential: 1,
            recovery_rejections: Default::default(),
        });
        plan.service_outcomes = vec![serde_json::from_value(serde_json::json!({
            "request": {
                "id": "school-request", "sceneId": "pads", "kind": "school",
                "count": 1, "widthM": 40, "depthM": 30, "heightM": 10,
            },
            "plannedSiteIds": ["pad.under-road"],
            "unfilledCount": 0,
            "rejectedCandidates": {},
            "status": "planned",
        }))
        .unwrap()];
        let rejections = refuse_on(&mut plan, &terrain, water);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        assert_eq!(rejections[0].category, PAD_BELOW_SETTLED_ROAD_PLATFORM);
        // 1. The plan.
        assert!(plan.roadside_sites.is_empty());
        // 2. The conversion it earned.
        assert!(plan.pad_conversions.is_empty());
        // 3. The service outcome reports the facility UNFILLED, never a phantom.
        assert_eq!(plan.service_outcomes.len(), 1);
        assert!(plan.service_outcomes[0].planned_site_ids.is_empty());
        assert_eq!(plan.service_outcomes[0].unfilled_count, 1);
        assert_eq!(plan.service_outcomes[0].status, "unfilled");
        // 4. The joint receipt is charged, never lowered.
        let joint = plan.joint_layout.as_ref().expect("the receipt survives");
        assert!(joint.retained_residential_ids.is_empty());
        assert_eq!(joint.residential_minimum, 2);
        assert_eq!(joint.unfilled_residential, 2);
        // 5. The refusal is published for the readers of "a pad that was planned and
        //    is not planned any more".
        assert!(refused_pad_ids(&plan).contains("pad.under-road"));
    }

    /// The same gate on a raster the caller authors.
    fn refuse_on(
        plan: &mut RoadNetworkPlan,
        terrain: &BakedTerrain,
        water: WaterGeometry,
    ) -> Vec<BaseProfileRejection> {
        let scene = scene();
        refuse_unrealizable_pads(
            plan,
            &scene,
            terrain,
            water,
            None,
            NaturalSource::SuppliedRaster,
        )
        .unwrap()
    }

    /// A raster whose every sample is `height_mm`: the ground on which a bounded
    /// cut has exact arithmetic.
    fn level_terrain(height_mm: i32) -> BakedTerrain {
        let scene = scene();
        let mut terrain = BakedTerrain::new(&scene).unwrap();
        let values = vec![f64::from(height_mm) / 1000.; 512 * 512];
        terrain
            .insert_chunk(
                0,
                0,
                512,
                512,
                &crate::height_payload::encode_millimetres(512, 512, values).unwrap(),
            )
            .unwrap();
        terrain
    }

    /// A FRONTAGE pad exactly as `frontage::append` publishes one: a precise
    /// `supportLevelMm`, the entrance point every frontage pad carries (which is
    /// what makes the support carve's +-2 m margin the margin the gate window uses)
    /// and, when the pad legally excavates, the `boundedCutMm` the placement
    /// decision recorded.
    fn frontage_pad(
        id: &str,
        x: i32,
        z: i32,
        level_mm: i32,
        cap: Option<i32>,
    ) -> serde_json::Value {
        let mut value = site(id, x, z, level_mm.div_euclid(1000), false);
        value["supportLevelMm"] = serde_json::json!(level_mm);
        value["frontageRoadPoint"] = serde_json::json!([x, z - 20]);
        if let Some(cap) = cap {
            value["boundedCutMm"] = serde_json::json!(cap);
        }
        value
    }

    /// The surface the bake will publish at one cell, composed from the plan's own
    /// earthwork exactly as the encoder composes it.
    fn planned_mm(plan: &RoadNetworkPlan, natural_m: f64, x: i32, z: i32) -> i32 {
        let mut world = plan.entities();
        world.extend(plan.site_entities());
        for entity in &mut world {
            entity.normalize_spatial_semantics();
        }
        let carves = super::super::plan_terrain_carves(&world);
        let surface = super::super::apply_terrain_carves(natural_m, x, z, &carves);
        (surface * 1000.).round() as i32
    }

    /// Requirement 2: a pad that declares a cut survives the very gate that used to
    /// strip it, and the PLATFORM it stands on is the declared level cell for cell -
    /// the obligation the road audit and `bounded_cut::verify` re-derive from the
    /// published bytes, not a claim about the +-2 m ring.
    #[test]
    fn a_declared_cut_holds_its_platform_and_is_not_refused() {
        // One metre of bank, a two metre cap: the flat top comes down to the declared
        // level everywhere, so the pad is realizable and the gate has to say so.
        let terrain = level_terrain(1000);
        let water = dry_water(&scene());
        let mut plan = plan(vec![frontage_pad("pad.bench", 200, 200, 0, Some(2000))], 0);
        let rejections = refuse_on(&mut plan, &terrain, water);
        assert!(
            rejections.is_empty(),
            "a legal bench is not a breach: {rejections:?}"
        );
        assert_eq!(plan.roadside_sites.len(), 1, "the pad is still planned");
        assert_eq!(plan.roadside_sites[0].bounded_cut_mm, Some(2000));
        // The platform - the building footprint - is the declared level, cell by cell.
        let entity = plan.roadside_sites[0].entity();
        let platform = super::super::frontage::bounds(&entity, 0);
        let mut cells = 0;
        for z in platform[1]..=platform[3] {
            for x in platform[0]..=platform[2] {
                assert_eq!(
                    planned_mm(&plan, 1.0, x as i32, z as i32),
                    0,
                    "platform cell ({x},{z}) is not the declared 0 mm top"
                );
                cells += 1;
            }
        }
        assert_eq!(cells, 625, "a 24x24 pad stands on a 25x25 cell platform");
        // The bench is REAL: one metre down over the 29x29 flat top (the footprint
        // plus the support carve's own 2 m margin), then a 25 % ring that meets the
        // natural surface four metres out - the same numbers the ledger measures.
        assert_eq!(planned_mm(&plan, 1.0, 200, 200), 0);
        assert_eq!(planned_mm(&plan, 1.0, 214, 200), 0, "the flat top holds out to +-14");
        assert_eq!(planned_mm(&plan, 1.0, 215, 200), 250);
        assert_eq!(planned_mm(&plan, 1.0, 216, 200), 500);
        assert_eq!(planned_mm(&plan, 1.0, 217, 200), 750);
        assert_eq!(
            planned_mm(&plan, 1.0, 218, 200),
            1000,
            "the ring meets the natural surface at the walkable grade"
        );
        // The excavation is inside the one cap: 1.000 m, never the 2.000 m the record
        // allows and never a millimetre more than the bank it removes.
        assert_eq!(
            1000 - planned_mm(&plan, 1.0, 200, 200),
            1000,
            "the deepest cell removes exactly the bank, not the cap"
        );
    }

    /// Requirement 3: the door is cut-AWARE, not cut-blind. A bench whose one cap
    /// cannot reach the declared level is refused - the platform is the obligation,
    /// and 2 m of cap cannot take a 5 m bank down to 0.
    #[test]
    fn a_cut_that_cannot_reach_its_declared_top_is_still_refused() {
        let terrain = level_terrain(5000);
        let water = dry_water(&scene());
        let mut unreachable = plan(vec![frontage_pad("pad.deep", 200, 200, 0, Some(2000))], 0);
        let rejections = refuse_on(&mut unreachable, &terrain, water);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        assert_eq!(rejections[0].edge_id, "pad.deep");
        assert_eq!(rejections[0].category, PAD_UNREALIZABLE_CATEGORY);
        assert!(
            rejections[0].reason.contains("leaves 3.000 m"),
            "the refusal names the surface the one cap actually reaches: {}",
            rejections[0].reason
        );
        assert!(unreachable.roadside_sites.is_empty());
        // The very same ground with a declared top the cap CAN reach is accepted: the
        // refusal is about the number, not about cutting as such.
        let mut reachable = plan(vec![frontage_pad("pad.ledge", 200, 200, 3000, Some(2000))], 0);
        let rejections = refuse_on(&mut reachable, &terrain, water);
        assert!(rejections.is_empty(), "{rejections:?}");
        assert_eq!(planned_mm(&reachable, 5.0, 200, 200), 3000);
    }

    /// Requirement 4: the OFF branch is the historical one. The SAME ground and the
    /// SAME declaration are refused when the pad carries no cap - a fill-only pad may
    /// not excavate - and a pad that needs no earthwork keeps its plan byte for byte.
    #[test]
    fn a_pad_without_a_cap_keeps_the_historical_fill_only_contract() {
        let water = dry_water(&scene());
        // No cap: the plan cannot lower the ground, so the platform stays 1.000 m
        // above its declared top and the pad is refused, with the same wording and
        // the same one number every fill-only world has always reported.
        let mut filling = plan(vec![frontage_pad("pad.fills", 200, 200, 0, None)], 0);
        let rejections = refuse_on(&mut filling, &level_terrain(1000), water);
        assert_eq!(rejections.len(), 1, "{rejections:?}");
        assert_eq!(rejections[0].category, PAD_UNREALIZABLE_CATEGORY);
        assert!(
            rejections[0].reason.contains("leaves 1.000 m"),
            "{}",
            rejections[0].reason
        );
        assert!(
            !rejections[0].reason.contains("cap"),
            "the fill-only refusal gains no clause: {}",
            rejections[0].reason
        );
        assert!(filling.roadside_sites.is_empty());
        // The positive arm: a pad that already stands on its own ground needs no
        // earthwork at all, and the gate leaves the plan exactly as it found it.
        let mut level = plan(vec![frontage_pad("pad.level", 200, 200, 0, None)], 0);
        let before = serde_json::to_vec(&level).unwrap();
        let rejections = refuse_on(&mut level, &level_terrain(0), water);
        assert!(rejections.is_empty(), "{rejections:?}");
        assert_eq!(serde_json::to_vec(&level).unwrap(), before);
        assert!(level.roadside_sites[0].bounded_cut_mm.is_none());
    }

    /// Requirement 1 (frontage arm): with no pad carrying a cap the gate is
    /// BYTE-FROZEN. The fixture is frontage-shaped - precise levels, entrance points,
    /// one pad refused for the fill contract - so the digest covers both arms; any
    /// movement of an off-state decision breaks it.
    #[test]
    fn the_refusal_gate_is_byte_frozen_for_a_frontage_world_with_no_cap() {
        let mut tall = site("frontage.tall", 300, 300, 7, false);
        tall["supportLevelMm"] = serde_json::json!(7000);
        tall["frontageRoadPoint"] = serde_json::json!([300, 280]);
        // 240 m apart: a fill's blend runs at 3 m per metre of height plus 32, so
        // nothing here reaches a neighbour's window and the fixture is about the gate
        // alone.
        let build = || {
            plan(
                vec![
                    frontage_pad("frontage.a", 60, 60, 0, None),
                    frontage_pad("frontage.b", 300, 60, 2000, None),
                    frontage_pad("frontage.c", 60, 300, 0, None),
                    tall.clone(),
                ],
                0,
            )
        };
        let water = dry_water(&scene());
        let terrain = level_terrain(0);
        let mut first = build();
        let rejections = refuse_on(&mut first, &terrain, water);
        let mut second = build();
        let again = refuse_on(&mut second, &terrain, water);
        assert_eq!(rejections, again, "the gate is a function of its input");
        assert_eq!(
            first.roadside_sites.len(),
            3,
            "only the 7 m pad is refused: {rejections:?}"
        );
        assert!(
            first
                .roadside_sites
                .iter()
                .all(|site| site.bounded_cut_mm.is_none())
        );
        let digest = blake3::hash(&serde_json::to_vec(&first).unwrap())
            .to_hex()
            .to_string();
        assert_eq!(
            digest, "5551ae519ac2617ca258c85086805cf1d04cdcc250e38c6356f20ba2ac8668ef",
            "the frozen blake3 of the frontage refusal gate with the bounded cut off"
        );
    }

    /// Requirement 1 (waterfront arm): the same freeze for a world that crosses
    /// structured water, where the gate measures a pad's own contribution against
    /// the declared top instead of the fill it caused.
    #[test]
    fn the_refusal_gate_is_byte_frozen_for_a_waterfront_world_with_no_cap() {
        let scene = scene();
        let style = serde_json::json!({
            "automaticRoadExtensions": {
                "allowStructuredWater": super::STRUCTURED_WATER_OPT_IN
            }
        });
        let water = water_geometry(WaterKind::River, &[], &scene, &style);
        assert!(water.road_water_crossings(), "the fixture opts into the crossing");
        let mut plan = plan(
            vec![
                frontage_pad("frontage.quay.a", 120, 200, 0, None),
                frontage_pad("frontage.quay.b", 400, 300, 1000, None),
            ],
            0,
        );
        let rejections = refuse_on(&mut plan, &level_terrain(0), water);
        assert!(rejections.is_empty(), "{rejections:?}");
        assert!(
            plan.roadside_sites
                .iter()
                .all(|site| site.bounded_cut_mm.is_none())
        );
        let digest = blake3::hash(&serde_json::to_vec(&plan).unwrap())
            .to_hex()
            .to_string();
        assert_eq!(
            digest, "e89b70a63b9adadb281953bcf4b4665c364653061eeb7482b53eb6b61b49e797",
            "the frozen blake3 of the waterfront refusal gate with the bounded cut off"
        );
    }
}

