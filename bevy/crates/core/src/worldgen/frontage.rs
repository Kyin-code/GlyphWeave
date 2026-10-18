//! Deterministic demand-led parcels on the FINAL planned road network.
//! Local road station heights are engineering inputs, not city-area credit.
use super::{
    AccessGraph, EntityInstance, LandUseZone, SceneSpec, WaterGeometry, WorldManifest,
    road_network::{RoadNetworkPlan, RoadsideSite},
    services::SiteProgram,
    zoning::ZoningMask,
    parcel_geometry::{self, ParcelInsets},
};
use crate::{
    baked_terrain::{BakedTerrain, TerrainError},
    height_payload::{HeightEncoding, encode_millimetres},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Minimum percentage of a parcel rectangle that must lie in zones of the request's own purpose for
/// the parcel to belong to that purpose. 100 would restore the strict "entirely inside" rule that the
/// real 512 m input could not satisfy; 90 keeps a parcel that merely clips a zone edge while still
/// rejecting one that mostly stands in another kind of zone.
const MIN_PURPOSE_COVERAGE_PERCENT: i64 = 75;

/// The frozen road-protection band of the parcel preflight: a parcel boundary
/// must stay at least this many metres outside a road footprint, and the parcel
/// is rejected as `occupied_or_road_support` when it does not. Named because the
/// morphology compiler derives a default street setback from it
/// (`settlement::morphology_compile`), so a profile default and this check can
/// never disagree about the same number. This is an engineering contract: a
/// profile may look at it, never widen it.
pub const ROAD_PROTECTION_M: i64 = 4;

/// The explicit along-road purpose-conversion policy of one request.
///
/// The authored switch is `style.frontageProgram.ruralConversion`. Only the exact string
/// [`RuralConversion::EXPLICIT`] turns it on: a missing key, `null` or any other value leaves the
/// historical purpose gate in place, so no previously authored manifest can change. `Disabled` is
/// the default, which is why an old manifest still produces a byte-identical plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuralConversion {
    /// The historical gate: only zones of the request's own purpose may carry a parcel.
    #[default]
    Disabled,
    /// An urban request may also take a parcel whose buildable core lies WHOLLY in rural land. The
    /// parcel is then recorded as a rural-to-urban conversion, which the land-use ledger applies.
    RuralToUrban,
}

impl RuralConversion {
    /// The one authored value that enables the switch.
    pub const EXPLICIT: &'static str = "explicit";

    /// Read the switch from the authored style value. Anything but the exact string is off, so a
    /// typo can never silently enable a land-use conversion.
    pub fn from_declared(value: Option<&serde_json::Value>) -> Self {
        match value.and_then(serde_json::Value::as_str) {
            Some(Self::EXPLICIT) => Self::RuralToUrban,
            _ => Self::Disabled,
        }
    }

    /// True only for the authored, explicit policy.
    pub fn is_explicit(self) -> bool {
        self == Self::RuralToUrban
    }
}

/// One parcel whose purpose this program converted from rural to urban.
///
/// The record is the only thing a ledger may use to move purpose area: it names the parcel, the
/// direction of the conversion and the converted land (the parcel's buildable core, in whole square
/// metres), so the transfer can be re-derived and audited instead of trusted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conversion {
    pub parcel_id: String,
    pub from: String,
    pub to: String,
    pub area_m2: i64,
}

impl Conversion {
    /// The purpose a converted parcel came from. Only rural land may be converted, and the record
    /// states it so a forged direction fails by name instead of being applied.
    pub const RURAL: &'static str = "rural";
    /// The purpose a converted parcel becomes.
    pub const URBAN: &'static str = "urban";

    pub fn rural_to_urban(parcel_id: &str, area_m2: i64) -> Self {
        Self {
            parcel_id: parcel_id.into(),
            from: Self::RURAL.into(),
            to: Self::URBAN.into(),
            area_m2,
        }
    }
}

/// The purpose a roadside PAD's building kind declares, or `None` when the settlement programme does
/// not know the kind.
///
/// A pad is not a frontage request: it carries no authored `purpose`, and that absence is exactly why
/// the historical pad programme could stand on any buildable land and let the "city" grow on the
/// farmland edge while the urban core stayed empty. The building kind is the only authored fact a pad
/// has, so its purpose is DERIVED from that kind here and declared on the land the pad takes. A kind
/// this function does not know cannot declare a purpose at all and is refused by name
/// (`pad_purpose_undeclared`) instead of being handed a default.
pub fn pad_purpose(kind: &str) -> Option<&'static str> {
    match kind {
        "residential_home" | "residential_block" | "storefront" | "school" | "church"
        | "convenience_store" | "town_hall" | "market" => Some(Conversion::URBAN),
        _ => None,
    }
}

/// What the explicit along-road policy decides about ONE pad parcel BEFORE the pad lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PadPurpose {
    /// The purpose the pad declares on the land it takes. `None` whenever the policy is off, which is
    /// the historical pad programme: no declaration, no gate, no conversion.
    pub declared: Option<&'static str>,
    /// Why the pad may not land, if it may not.
    pub reason: Option<&'static str>,
    /// Buildable-core area the pad converts from rural to urban purpose, in whole square metres.
    pub converted_m2: Option<i64>,
}

impl PadPurpose {
    /// The policy-off verdict: no declaration, no refusal, no conversion.
    pub(crate) const OFF: PadPurpose = PadPurpose {
        declared: None,
        reason: None,
        converted_m2: None,
    };
}

/// The purpose gate of ONE roadside pad, measured on the pad's own parcel.
///
/// With `style.frontageProgram.ruralConversion` off this is the identity `PadPurpose::OFF`, so the
/// historical pad programme cannot move by a single byte. Switched on, the pad must first declare a
/// purpose and then pass the SAME envelope verdict an along-road frontage parcel passes: urban land
/// carries it directly, land whose buildable core is WHOLLY rural is converted and recorded, and
/// everything else - nature, unassigned, protection, a parcel outside the scene - is refused under
/// the existing `wrong_purpose_or_protected` key. Water, the reserved shore band and the engineering
/// gates are refused BEFORE this by the pad's own checks and keep their own named reasons.
pub(crate) fn pad_purpose_of(
    parcel: [i64; 4],
    insets: ParcelInsets,
    rotation_y_deg: f32,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    kind: &str,
    conversion: RuralConversion,
) -> PadPurpose {
    if !conversion.is_explicit() {
        return PadPurpose::OFF;
    }
    let Some(purpose) = pad_purpose(kind) else {
        return PadPurpose {
            declared: None,
            reason: Some("pad_purpose_undeclared"),
            converted_m2: None,
        };
    };
    let (reason, converted_m2) =
        envelope_verdict(parcel, insets, rotation_y_deg, scene, zones, purpose, conversion);
    PadPurpose {
        declared: Some(purpose),
        reason,
        converted_m2,
    }
}

/// The buildable-core area one planned pad converts from rural to urban purpose, if it earns one.
///
/// Scene-free on purpose: this is the form the AUDIT needs, because a record's area must be provable
/// from the plan, the pad's own parcel and the zoning alone. The scene envelope was already enforced
/// when the pad landed, and a parcel that claims a single protected square metre earns nothing here
/// (`verify_conversion` refuses such a record by name on its own).
pub fn pad_conversion_area(site: &RoadsideSite, zones: &[LandUseZone]) -> Option<i64> {
    let parcel = parcel_boundary(site).ok()?;
    if parcel_claims_protected(parcel, zones) {
        return None;
    }
    let core = buildable_core(
        parcel,
        site.parcel_insets.unwrap_or_default(),
        site.rotation_y_deg,
    )?;
    if core_entirely_in(core, zones, Conversion::RURAL) {
        Some(area(core))
    } else {
        None
    }
}

/// The same verdict for one planned pad, from that pad's own parcel geometry and its own kind.
pub(crate) fn pad_purpose_for_site(
    site: &RoadsideSite,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    conversion: RuralConversion,
) -> Result<PadPurpose, TerrainError> {
    if !conversion.is_explicit() {
        return Ok(PadPurpose::OFF);
    }
    Ok(pad_purpose_of(
        parcel_boundary(site)?,
        site.parcel_insets.unwrap_or_default(),
        site.rotation_y_deg,
        scene,
        zones,
        site.kind(),
        conversion,
    ))
}

/// Every rural-to-urban conversion this plan's roadside PADS earned, in plan order.
///
/// The records are derived from the FINAL pad list instead of being accumulated while placing, so a
/// pad that a later pass displaces or renames cannot leave a record behind: a conversion exists
/// exactly as long as its parcel does. Only a pad that DECLARED a purpose can appear here; a frontage
/// parcel keeps its own record in the frontage report, so no square metre is converted twice.
pub(crate) fn pad_conversions(
    plan: &RoadNetworkPlan,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    conversion: RuralConversion,
) -> Result<Vec<Conversion>, TerrainError> {
    let mut records = Vec::new();
    if !conversion.is_explicit() {
        return Ok(records);
    }
    for site in &plan.roadside_sites {
        if site.purpose.is_none() {
            continue;
        }
        if let Some(area_m2) = pad_purpose_for_site(site, scene, zones, conversion)?.converted_m2 {
            records.push(Conversion::rural_to_urban(&site.id, area_m2));
        }
    }
    Ok(records)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    /// `style.frontageProgram.ruralConversion`, kept verbatim: only the exact string "explicit" is a
    /// policy, every other value (and a missing one) is off. `from_style` derives the typed policy
    /// onto each request, because a request is what the gate and the placement loop decide on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rural_conversion: Option<serde_json::Value>,
    pub requests: Vec<Request>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub scene_id: String,
    pub purpose: String,
    pub kind: String,
    pub count: u32,
    pub width_m: u32,
    pub depth_m: u32,
    pub height_m: u32,
    pub station_spacing_m: u32,
    pub setback_m: u32,
    pub max_candidate_attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parcel_insets: Option<ParcelInsets>,
    /// The along-road purpose-conversion policy this request is placed under. It is DERIVED by
    /// `from_style` from the program-level switch and skipped by serde on purpose: a request may not
    /// widen its own purpose gate, and every already published request must serialize byte-identically.
    #[serde(skip)]
    pub rural_conversion: RuralConversion,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outcome {
    pub request: Request,
    pub site_ids: Vec<String>,
    pub attempted: u32,
    pub unfilled: u32,
    pub rejected: BTreeMap<String, u32>,
    pub stop_reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub version: u32,
    pub model: String,
    pub outcomes: Vec<Outcome>,
    /// Every parcel this report accepted by converting rural land to urban land, in placement order.
    /// Empty (and therefore absent) whenever the switch is off, so an old plan round-trips unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversions: Vec<Conversion>,
}
fn err(s: impl Into<String>) -> TerrainError {
    TerrainError(s.into())
}
pub fn from_style(style: &serde_json::Value) -> Result<Option<Program>, TerrainError> {
    let Some(v) = style.get("frontageProgram") else {
        return Ok(None);
    };
    let mut p: Program = serde_json::from_value(v.clone()).map_err(|e| err(e.to_string()))?;
    if p.version != 1 || p.requests.is_empty() || p.requests.len() > 64 {
        return Err(err("invalid frontage program"));
    }
    let mut ids = BTreeSet::new();
    let mut count = 0_u64;
    for r in &p.requests {
        if r.id.is_empty()
            || r.id.len() > 64
            || !r
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !ids.insert((&r.scene_id, &r.id))
            || r.scene_id.is_empty()
            || !matches!(r.purpose.as_str(), "urban" | "rural")
            || !(1..=1024).contains(&r.count)
            || !(1..=65536).contains(&r.max_candidate_attempts)
            || !(8..=128).contains(&r.station_spacing_m)
            || r.station_spacing_m % 2 != 0
            || !(8..=32).contains(&r.setback_m)
            || r.setback_m % 2 != 0
        {
            return Err(err("invalid frontage id/purpose/count/search limits"));
        }
        // Bounded search geometry; the manifest asset contract is checked separately.
        let bounds = match r.kind.as_str() {
            "residential_home" => (12..=48, 10..=36, 3..=14),
            "residential_block" => (24..=100, 16..=60, 12..=60),
            "storefront" => (8..=36, 8..=24, 3..=12),
            _ => return Err(err("unsupported frontage building kind")),
        };
        if !bounds.0.contains(&r.width_m)
            || !bounds.1.contains(&r.depth_m)
            || !bounds.2.contains(&r.height_m)
            || r.width_m % 2 != 0
            || r.depth_m % 2 != 0
        {
            return Err(err("invalid exact frontage dimensions; no shrinking"));
        }
        r.parcel_insets
            .unwrap_or_default()
            .validate_for_request(r.width_m, r.depth_m, r.setback_m)
            .map_err(err)?;
        count += u64::from(r.count);
    }
    if count > 2048 {
        return Err(err("frontage program exceeds 2048 buildings"));
    }
    // The switch is authored once per program and applied to every request of it, so placement and
    // the read-only supply measurement (which both work from a `Request`) cannot disagree about it.
    let conversion = RuralConversion::from_declared(p.rural_conversion.as_ref());
    for r in &mut p.requests {
        r.rural_conversion = conversion;
    }
    Ok(Some(p))
}
pub fn validate_manifest(m: &WorldManifest) -> Result<(), TerrainError> {
    if let Some(p) = from_style(&m.style)? {
        if super::height_encoding_from_style(&m.style).map_err(|e| err(e.to_string()))?
            != HeightEncoding::Millimetre
        {
            return Err(err("frontage requires millimetre heights"));
        }
        for r in p.requests {
            let contract = m
                .style
                .get("assetContracts")
                .and_then(|v| v.get(&r.kind))
                .ok_or_else(|| err("frontage requires an explicit asset contract"))?;
            for (axis, size) in [
                ("Width", r.width_m),
                ("Depth", r.depth_m),
                ("Height", r.height_m),
            ] {
                let min = contract
                    .get(format!("min{axis}M"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| err("frontage asset minimum missing"))?;
                let max = contract
                    .get(format!("max{axis}M"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| err("frontage asset maximum missing"))?;
                if !min.is_finite()
                    || !max.is_finite()
                    || min > max
                    || f64::from(size) < min
                    || f64::from(size) > max
                {
                    return Err(err(format!(
                        "frontage {} {axis} violates asset contract; no resizing",
                        r.id
                    )));
                }
            }
            let scene = m
                .scenes
                .iter()
                .find(|s| s.scene_id == r.scene_id)
                .ok_or_else(|| err("unknown frontage scene"))?;
            let water =
                super::water_geometry(super::water_kind(&m.style), &m.landmarks, scene, &m.style);
            if !super::road_network::enabled(&m.style, water) {
                return Err(err("frontage requires road-first planning"));
            }
        }
    }
    Ok(())
}
/// The engineered entrance is a bounded cardinal connection, never an arbitrary earthwork.
pub fn validate_entity(e: &EntityInstance) -> Result<(), TerrainError> {
    let Some([x, z]) = e.grounding.frontage_road_point else {
        return Ok(());
    };
    if !matches!(
        e.kind.as_str(),
        "residential_home" | "residential_block" | "storefront"
    ) || e.anchors.len() != 1
        || e.grounding.road_profile.is_some()
    {
        return Err(err("invalid frontage support owner"));
    }
    let a = &e.anchors[0];
    let dx = i64::from(x) - i64::from(a.world_x);
    let dz = i64::from(z) - i64::from(a.world_z);
    let length = dx.abs() + dz.abs();
    let direction = match a.direction.as_str() {
        "north" => dx == 0 && dz < 0,
        "south" => dx == 0 && dz > 0,
        "east" => dz == 0 && dx > 0,
        "west" => dz == 0 && dx < 0,
        _ => false,
    };
    if !direction
        || !(1..=40).contains(&length)
        || !e.ground_height_m().is_finite()
        || !(-32.0..=256.0).contains(&e.ground_height_m())
    {
        return Err(err("invalid frontage support alignment/length/height"));
    }
    Ok(())
}
fn bounds(e: &EntityInstance, margin: i64) -> [i64; 4] {
    let (w, d) = super::spatial_half_extents(e.width_m, e.depth_m, e.rotation_y_deg);
    [
        i64::from(e.world_x) - w as i64 - margin,
        i64::from(e.world_z) - d as i64 - margin,
        i64::from(e.world_x) + w as i64 + margin,
        i64::from(e.world_z) + d as i64 + margin,
    ]
}
fn overlap(a: [i64; 4], b: [i64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
pub(super) fn parcel_boundary(site: &RoadsideSite) -> Result<[i64; 4], TerrainError> {
    parcel_geometry::boundary_for_entity(
        &site.entity(),
        site.parcel_insets.unwrap_or_default(),
    )
    .map_err(err)
}
/// Production road iteration order for the local station search. Wider
/// hierarchy roads are kept as topology/collectors; local roads get first
/// chance when their width is narrower. All-8m legacy inputs retain the
/// original plan iteration order.
pub(super) fn profile_roads(plan: &RoadNetworkPlan) -> Vec<EntityInstance> {
    let mut roads = plan.entities();
    if roads.iter().any(|road| road.depth_m > 8.) {
        roads.sort_by_key(|road| (road.depth_m as u32, road.entity_id.clone()));
    }
    roads
}
/// The exact station list `append` visits on one road for one request, or
/// `None` when the road carries no local profile contract. Shared with the
/// bounded supply probe so a measurement can never drift from placement.
pub(super) fn stations(
    road: &EntityInstance,
    r: &Request,
) -> Result<Option<Vec<usize>>, TerrainError> {
    // At present only profile-bearing roads support this local-height path.
    let Some(profile) = &road.grounding.road_profile else {
        return Ok(None);
    };
    profile.validate_entity(road)?;
    let length = profile.elevations_mm.len() - 1;
    let margin = r.width_m / 2 + 8;
    if length <= 2 * margin as usize {
        return Ok(None);
    }
    // Wider roads need their extra half-width as junction clearance so the
    // first parcel does not overlap the connecting collector.
    let station_start = margin as usize + (road.depth_m as usize).saturating_sub(8);
    if station_start > length - margin as usize {
        return Ok(None);
    }
    Ok(Some(
        (station_start..=length - margin as usize)
            .step_by(r.station_spacing_m as usize)
            .collect(),
    ))
}
/// One candidate handed out by [`StationSchedule`]: the request that gets its
/// turn, and the station and side the candidate is tried at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Turn {
    pub request: usize,
    pub station: usize,
    pub side: i32,
}
/// The candidate order of ONE road, shared by placement and measurement.
///
/// `append` places from this schedule and `frontage_supply::diagnose` measures
/// from it, so a read-only diagnosis cannot describe a search the bake never
/// performs. The order is: per station, per side, with the requests ROTATED by
/// that diagonal. At a shared station usually only ONE parcel fits, so a fixed
/// request order would let the first request take every contested station while
/// the later requests place nothing even though they are considered.
pub(super) struct StationSchedule<'a> {
    requests: &'a [Request],
    /// Station ordinates per request; `None` when that request has no station on
    /// this road. `stations` is `?`-propagated once per road, exactly as
    /// production always evaluated it before placing anything on that road.
    stations: Vec<Option<Vec<usize>>>,
    /// Longest station list: the number of diagonals this road offers.
    diagonals: usize,
}
impl<'a> StationSchedule<'a> {
    pub(super) fn new(
        road: &EntityInstance,
        requests: &'a [Request],
    ) -> Result<Self, TerrainError> {
        let mut lists = Vec::with_capacity(requests.len());
        for request in requests {
            lists.push(stations(road, request)?);
        }
        let diagonals = lists
            .iter()
            .filter_map(|list| list.as_ref().map(Vec::len))
            .max()
            .unwrap_or(0);
        Ok(Self {
            requests,
            stations: lists,
            diagonals,
        })
    }
    /// Station list of one request on this road, if the road carries it.
    pub(super) fn stations_of(&self, request: usize) -> Option<&[usize]> {
        self.stations.get(request)?.as_deref()
    }
    /// Every candidate this road offers, in production order. A request that
    /// has no station at a diagonal simply takes no turn there, which is what
    /// keeps the rotation from inventing a candidate.
    pub(super) fn turns(&self) -> impl Iterator<Item = Turn> + '_ {
        let count = self.requests.len();
        (0..self.diagonals).flat_map(move |position| {
            [-1_i32, 1].into_iter().flat_map(move |side| {
                let offset = (position * 2 + usize::from(side > 0)) % count.max(1);
                (0..count).filter_map(move |step| {
                    let request = (offset + step) % count.max(1);
                    let station = *self.stations_of(request)?.get(position)?;
                    Some(Turn {
                        request,
                        station,
                        side,
                    })
                })
            })
        })
    }
}
/// Build the exact `RoadsideSite` production would try at this station/side.
pub(super) fn candidate_site(
    road: &EntityInstance,
    r: &Request,
    scene_id: &str,
    station: usize,
    side: i32,
) -> Result<RoadsideSite, TerrainError> {
    let profile = road
        .grounding
        .road_profile
        .as_ref()
        .ok_or_else(|| err("frontage candidate requires a road profile"))?;
    let dx = (profile.end[0] - profile.start[0]).signum();
    let dz = (profile.end[1] - profile.start[1]).signum();
    let rx = profile.start[0] + dx * station as i32;
    let rz = profile.start[1] + dz * station as i32;
    let offset = (road.depth_m / 2.) as i32 + r.setback_m as i32 + (r.depth_m / 2) as i32;
    let (x, z, yaw) = if dx != 0 {
        (rx, rz + side * offset, if side > 0 { 0. } else { 180. })
    } else {
        (rx + side * offset, rz, if side > 0 { 90. } else { 270. })
    };
    let level = i32::try_from(profile.elevations_mm[station])
        .map_err(|_| err("frontage height overflow"))?;
    let id = format!(
        "generated.frontage.{}.{}.{}.{}.{}",
        scene_id,
        r.id,
        road.entity_id,
        station,
        if side > 0 { "positive" } else { "negative" }
    );
    Ok(RoadsideSite {
        program: Some(SiteProgram {
            request_id: r.id.clone(),
            kind: r.kind.clone(),
            height_m: r.height_m,
        }),
        // A frontage parcel's purpose is its authored request, which `verify_conversion` reads from
        // the outcome; it never declares one on the parcel itself.
        purpose: None,
        id,
        road_id: road.entity_id.clone(),
        world_x: x,
        world_z: z,
        width_m: r.width_m,
        depth_m: r.depth_m,
        support_level_m: level.div_euclid(1000),
        parcel_insets: r.parcel_insets,
        support_level_mm: Some(level),
        frontage_road_point: Some([rx, rz]),
        rotation_y_deg: yaw,
        natural_min_m: f32::INFINITY,
        natural_max_m: f32::NEG_INFINITY,
    })
}
/// Scene envelope and purpose gate, applied before any engineering preflight.
///
/// `insets` is the parcel's declared inset set, used to locate the BUILDABLE CORE that the purpose
/// coverage is measured over. Setbacks are not built land, so a parcel whose building site is in
/// purpose land is accepted even when its rear setback reaches across a zone edge.
pub(super) fn envelope_reason(
    parcel: [i64; 4],
    insets: ParcelInsets,
    rotation_y_deg: f32,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    purpose: &str,
) -> Option<&'static str> {
    if outside_scene(parcel, scene) {
        Some("outside_scene")
    } else if parcel_claims_protected(parcel, zones) {
        // The setback exemption below is what lets a parcel's rear band cross a
        // zone edge, and it is correct for urban/rural edges: setback land is not
        // built land. It is NOT correct for protected land, because the parcel
        // rectangle is the ownership unit the development plan partitions into
        // yard cells, so protected land inside it is reserved as private yard.
        Some("wrong_purpose_or_protected")
    } else if !in_purpose_core(parcel, insets, rotation_y_deg, zones, purpose) {
        Some("wrong_purpose_or_protected")
    } else {
        None
    }
}

/// True when the parcel rectangle shares at least one whole square metre with a
/// zone that is neither `urban` nor `rural`.
///
/// This is the parcel-level form of the development audit's own rule
/// (`settlement::development::verify`: a `Building`/`Yard` cell in a zone whose
/// kind is not urban/rural is counted in `zone_conflict_m2` and fails with
/// "residential parcel conflicts with nature/unassigned zoning"). B05 measured
/// the consequence: on the seed-42 village-farmland fixture two converted shops
/// had rear setbacks reaching 12 m into the nature zone, which turned 480 m2 of
/// protected cell into yard cells; the conservation surface lost exactly that
/// 480 m2 (49152 -> 48672) and the nature ledger booked it as unresolved. A
/// parcel may therefore not straddle a protected boundary at all, no matter
/// which purpose or conversion earned it.
fn parcel_claims_protected(parcel: [i64; 4], zones: &[LandUseZone]) -> bool {
    zones
        .iter()
        .filter(|zone| !matches!(zone.kind.as_str(), "urban" | "rural"))
        .any(|zone| intersection_area(parcel, zone) > 0)
}

/// The scene envelope test, shared by the historical gate and the conversion gate so the two can
/// never disagree about the edge margin.
fn outside_scene(parcel: [i64; 4], scene: &SceneSpec) -> bool {
    parcel[0] < i64::from(scene.origin_x) + 8
        || parcel[1] < i64::from(scene.origin_z) + 8
        || parcel[2] + 8 >= i64::from(scene.origin_x) + i64::from(scene.width_m)
        || parcel[3] + 8 >= i64::from(scene.origin_z) + i64::from(scene.depth_m)
}

/// The gate as production applies it to ONE request, with the purpose conversion this parcel earns.
///
/// The disabled branch calls `envelope_reason` ITSELF rather than a copy of its logic, so switching
/// the conversion off cannot change a single decision. The enabled branch adds exactly one
/// acceptance: an urban request whose buildable core lies WHOLLY in rural land. Nature, unassigned
/// and protection zones reject the parcel in both branches, and `preflight` still refuses water, so
/// a conversion can never be pointed at land that is not rural. A parcel rectangle that includes any
/// protected square metre is refused in both branches too (`parcel_claims_protected`): the
/// conversion only decides what the BUILDABLE CORE stands on, never what the parcel may own.
///
/// Returns the rejection reason (the same `&'static str` keys the audit counts) and, when the parcel
/// is accepted by conversion, the converted buildable-core area in whole square metres.
pub(super) fn envelope_verdict(
    parcel: [i64; 4],
    insets: ParcelInsets,
    rotation_y_deg: f32,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    purpose: &str,
    conversion: RuralConversion,
) -> (Option<&'static str>, Option<i64>) {
    if !conversion.is_explicit() {
        return (
            envelope_reason(parcel, insets, rotation_y_deg, scene, zones, purpose),
            None,
        );
    }
    if outside_scene(parcel, scene) {
        return (Some("outside_scene"), None);
    }
    if parcel_claims_protected(parcel, zones) {
        return (Some("wrong_purpose_or_protected"), None);
    }
    let Some(core) = buildable_core(parcel, insets, rotation_y_deg) else {
        // a parcel too small for a buildable core, or a non-cardinal rotation, cannot be converted
        return (Some("wrong_purpose_or_protected"), None);
    };
    if in_purpose(core, zones, purpose) {
        return (None, None);
    }
    if purpose == Conversion::URBAN && core_entirely_in(core, zones, Conversion::RURAL) {
        return (None, Some(area(core)));
    }
    (Some("wrong_purpose_or_protected"), None)
}

/// Re-derive what a conversion claim MUST be, from the plan's own parcel geometry and the zoning.
///
/// Every published record is compared with this result instead of being trusted, so a hand-edited
/// ledger entry fails here by name. `water` is optional because the ledger layer has no water
/// authority; the quality gate always passes it, so a claim that touches water is refused where it
/// can be.
pub fn verify_conversion(
    plan: &RoadNetworkPlan,
    zones: &[LandUseZone],
    water: Option<WaterGeometry>,
    conversion: &Conversion,
) -> Result<i64, String> {
    if conversion.from != Conversion::RURAL || conversion.to != Conversion::URBAN {
        return Err("conversion is not rural to urban".into());
    }
    if conversion.area_m2 <= 0 {
        return Err("conversion area is not a positive whole area".into());
    }
    let site = plan
        .roadside_sites
        .iter()
        .find(|site| site.id == conversion.parcel_id)
        .ok_or_else(|| {
            format!(
                "conversion parcel {} is not a planned frontage parcel",
                conversion.parcel_id
            )
        })?;
    let purpose = plan
        .frontage
        .as_ref()
        .and_then(|report| {
            report.outcomes.iter().find(|outcome| {
                outcome
                    .site_ids
                    .iter()
                    .any(|id| id == &conversion.parcel_id)
            })
        })
        .map(|outcome| outcome.request.purpose.clone());
    // A roadside PAD is not an authored frontage request: it declares its purpose on its own parcel
    // instead. The declaration is re-derived from the pad's kind here rather than trusted, so a pad
    // whose real purpose is not the urban one it claims fails by name.
    let purpose = match purpose {
        Some(purpose) => purpose,
        None => match site.purpose.as_deref() {
            Some(declared) => {
                match pad_purpose(site.kind()) {
                    Some(derived) if derived == declared => {}
                    Some(derived) => {
                        return Err(format!(
                            "conversion parcel {} declares purpose {declared} but its kind {} is {derived}",
                            conversion.parcel_id,
                            site.kind()
                        ));
                    }
                    None => {
                        return Err(format!(
                            "conversion parcel {} declares purpose {declared} but its kind {} is no settlement purpose",
                            conversion.parcel_id,
                            site.kind()
                        ));
                    }
                }
                declared.to_string()
            }
            None => {
                return Err(format!(
                    "conversion parcel {} belongs to no authored request and declares no purpose",
                    conversion.parcel_id
                ));
            }
        },
    };
    if purpose != Conversion::URBAN {
        return Err(format!(
            "conversion parcel {} belongs to a {purpose} request, not an urban one",
            conversion.parcel_id
        ));
    }
    let parcel = parcel_boundary(site).map_err(|e| e.0)?;
    if parcel_claims_protected(parcel, zones) {
        return Err(format!(
            "conversion parcel {} claims protected land",
            conversion.parcel_id
        ));
    }
    let core = buildable_core(
        parcel,
        site.parcel_insets.unwrap_or_default(),
        site.rotation_y_deg,
    )
    .ok_or_else(|| {
        format!(
            "conversion parcel {} has no buildable core",
            conversion.parcel_id
        )
    })?;
    if !core_entirely_in(core, zones, Conversion::RURAL) {
        return Err(format!(
            "conversion parcel {} core is not wholly rural land",
            conversion.parcel_id
        ));
    }
    if let Some(water) = water {
        let center_x = (parcel[0] + parcel[2]) as f64 * 0.5;
        let center_z = (parcel[1] + parcel[3]) as f64 * 0.5;
        let half_w = (parcel[2] - parcel[0]) as f64 * 0.5;
        let half_d = (parcel[3] - parcel[1]) as f64 * 0.5;
        if water.intersects_axis_aligned(center_x, center_z, half_w, half_d) {
            return Err(format!(
                "conversion parcel {} touches water",
                conversion.parcel_id
            ));
        }
    }
    Ok(area(core))
}

/// Whole square metres of one parcel rectangle.
fn area(b: [i64; 4]) -> i64 {
    (b[2] - b[0]) * (b[3] - b[1])
}

/// Area one rectangle shares with one zone, in whole square metres.
fn intersection_area(b: [i64; 4], zone: &LandUseZone) -> i64 {
    let width = (b[2].min(i64::from(zone.world_x) + i64::from(zone.width_m))
        - b[0].max(i64::from(zone.world_x)))
    .max(0);
    let depth = (b[3].min(i64::from(zone.world_z) + i64::from(zone.depth_m))
        - b[1].max(i64::from(zone.world_z)))
    .max(0);
    width * depth
}

/// True when EVERY square metre of `b` lies inside a zone of exactly `kind`, and no zone of any
/// other kind touches `b` at all.
///
/// Both halves matter for a purpose conversion: an uncovered sliver is not rural land, and a core
/// that overlaps a nature, unassigned or protection zone would convert protected land. The
/// sum-of-intersections test is exact for the production zoning, whose cells tile the scene without
/// overlap (`ZoningMask::new` refuses anything else); when zones do overlap the sum can exceed the
/// core area, so the claim is re-checked square metre by square metre instead of being trusted.
fn core_entirely_in(b: [i64; 4], zones: &[LandUseZone], kind: &str) -> bool {
    let total = area(b);
    if total <= 0 {
        return false;
    }
    let mut covered = 0;
    for zone in zones {
        let shared = intersection_area(b, zone);
        if shared == 0 {
            continue;
        }
        if zone.kind != kind {
            return false;
        }
        covered += shared;
    }
    if covered < total {
        return false;
    }
    if covered == total {
        return true;
    }
    for z in b[1]..b[3] {
        for x in b[0]..b[2] {
            let inside = zones.iter().any(|zone| {
                zone.kind == kind
                    && x >= i64::from(zone.world_x)
                    && z >= i64::from(zone.world_z)
                    && x < i64::from(zone.world_x) + i64::from(zone.width_m)
                    && z < i64::from(zone.world_z) + i64::from(zone.depth_m)
            });
            if !inside {
                return false;
            }
        }
    }
    true
}

/// The BUILDABLE CORE of a parcel rectangle: the building envelope minus a one metre skin, expressed
/// as [x0, z0, x1, z1]. The purpose-coverage rule is measured over this core rather than over the whole
/// parcel, because the parcel rectangle also contains the front, side and rear SETBACKS, and setback
/// land is not built land.
///
/// B01 measured evidence: with the whole rectangle measured, a 24x24 m residential_home (the minimum
/// the asset contract allows) is centred road/2 + setback + depth/2 from the road, so it occupies
/// roughly 12 to 36 m outward. Inside a 143 m zone cut by a road corridor the outer part of that band
/// frequently falls outside the zone, so the parcel was rejected as wrong_purpose_or_protected even
/// though its building site was in urban land. That is why 0 of 120 homes realized at both
/// urbanCoreRatio 0.02 and 0.15 while the zoning held room for 72 and then 252 parcels.
///
/// Measuring the core keeps the rule honest: a parcel whose BUILDING SITE lies in another kind of zone
/// is still rejected, so "wrong purpose earns no buildings" is preserved.
///
/// B01-R1 T1: the core MUST rotate with the building. `ParcelInsets.front_m` is the inset along the
/// building's LOCAL forward direction (north for yaw 0, west for 90, south for 180, east for 270, as
/// documented on parcel_geometry::boundary). An earlier version subtracted `front` from world Z and
/// `rear` from world Z unconditionally, which is correct ONLY at yaw 0: at 90/180/270 it applied the
/// front and rear insets on the wrong axis and swapped them relative to the entrance. That made the
/// purpose-coverage test disagree with the parcel boundary it is derived from.
///
/// The mapping below mirrors `parcel_geometry::boundary` exactly: the local forward axis is world Z for
/// yaw 0/180 and world X for yaw 90/270, and the side insets run along the other axis.
///
/// Returns `None` when the rotation is not one of the four cardinal directions or the core is degenerate,
/// so a caller cannot silently treat an unusable core as covered.
pub(crate) fn buildable_core(
    b: [i64; 4],
    insets: ParcelInsets,
    rotation_y_deg: f32,
) -> Option<[i64; 4]> {
    const SKIN: i64 = 1;
    let front = i64::from(insets.front_m) + SKIN;
    let side = i64::from(insets.side_m) + SKIN;
    let rear = i64::from(insets.rear_m) + SKIN;
    let core = match rotation_y_deg.round() as i32 {
        // front toward -Z
        0 => [b[0] + side, b[1] + front, b[2] - side, b[3] - rear],
        // front toward -X
        90 => [b[0] + front, b[1] + side, b[2] - rear, b[3] - side],
        // front toward +Z
        180 => [b[0] + side, b[1] + rear, b[2] - side, b[3] - front],
        // front toward +X
        270 => [b[0] + rear, b[1] + side, b[2] - front, b[3] - side],
        _ => return None,
    };
    if core[2] <= core[0] || core[3] <= core[1] {
        return None;
    }
    Some(core)
}

fn in_purpose_core(
    b: [i64; 4],
    insets: ParcelInsets,
    rotation_y_deg: f32,
    zones: &[LandUseZone],
    purpose: &str,
) -> bool {
    let Some(core) = buildable_core(b, insets, rotation_y_deg) else {
        // a parcel too small to have a buildable core, or a non-cardinal rotation, cannot be judged by
        // this rule and is therefore not in purpose
        return false;
    };
    in_purpose(core, zones, purpose)
}

fn in_purpose(b: [i64; 4], zones: &[LandUseZone], purpose: &str) -> bool {
    let covered: i64 = zones
        .iter()
        .filter(|z| z.kind == purpose)
        .map(|z| {
            (b[2].min(i64::from(z.world_x) + i64::from(z.width_m)) - b[0].max(i64::from(z.world_x)))
                .max(0)
                * (b[3].min(i64::from(z.world_z) + i64::from(z.depth_m))
                    - b[1].max(i64::from(z.world_z)))
                .max(0)
        })
        .sum();
    let total = (b[2] - b[0]) * (b[3] - b[1]);
    // G34-R5-B01: the previous rule required the parcel rectangle to be covered COMPLETELY by zones of
    // the request purpose (`covered == total`). On real zone geometry that rejects almost every parcel,
    // because the road network cuts the single urban zone and a 20x26 m parcel straddles the cut: the
    // real 512 m input realized 5 of 16 urban homes and 0 of 8 urban shops, with 23 of only 30 attempts
    // rejected as wrong_purpose_or_protected. The authored demand is satisfiable -- the unit fixture in
    // this file realizes all 24 sites when the zone cooperates -- so the defect is this predicate, not
    // the demand or the search.
    //
    // The rule is now a bounded majority: a parcel belongs to the purpose when purpose zones cover at
    // least MIN_PURPOSE_COVERAGE_PERCENT of it. A parcel that mostly lies in another kind of zone is
    // still rejected, so the "wrong purpose does not earn buildings" behaviour is preserved; only the
    // strict equality is relaxed. Protection is handled separately by envelope_reason and is not
    // weakened here.
    covered * 100 >= total * MIN_PURPOSE_COVERAGE_PERCENT
}
fn raw_mm(n: &BakedTerrain, x: i32, z: i32) -> Result<i32, TerrainError> {
    n.height_millimetres(x, z)
        .ok_or_else(|| err("missing frontage source sample"))
}
fn sample(
    n: &BakedTerrain,
    mask: &ZoningMask,
    carves: &[super::TerrainCarve],
    x: i32,
    z: i32,
) -> Result<i32, TerrainError> {
    let base = f64::from(raw_mm(n, x, z)?) / 1000.;
    let h = if mask.protected_at(x, z) {
        base
    } else {
        super::apply_terrain_carves(base, x, z, carves)
            .max(base)
            .min(base + mask.fill_capacity_at(x, z))
    };
    Ok((h * 1000.).round() as i32)
}
/// Structured evidence for one real rejection. The decision itself stays the
/// `&'static str` reason key; this only records what was actually found.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RejectDetail {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicting_entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicting_site: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_mm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
impl RejectDetail {
    fn of(reason: &str) -> Self {
        Self {
            reason: reason.into(),
            conflicting_entity: None,
            conflicting_site: None,
            at: None,
            natural_mm: None,
            design_mm: None,
            before_mm: None,
            after_mm: None,
            note: None,
        }
    }
    /// Evidence for the pre-engineering envelope/purpose gate.
    pub(super) fn envelope(reason: &str, parcel: [i64; 4]) -> Self {
        Self {
            at: Some([
                ((parcel[0] + parcel[2]) / 2) as i32,
                ((parcel[1] + parcel[3]) / 2) as i32,
            ]),
            note: Some(format!(
                "parcel=[{},{},{},{}]",
                parcel[0], parcel[1], parcel[2], parcel[3]
            )),
            ..Self::of(reason)
        }
    }
}
fn support_check(
    site: &RoadsideSite,
    n: &BakedTerrain,
    mask: &ZoningMask,
    max_fill_mm: i32,
    detail: &mut Option<RejectDetail>,
) -> Result<Option<&'static str>, TerrainError> {
    let e = site.entity();
    let b = bounds(&e, 2);
    let level = site.support_level_mm.expect("new sites are precise");
    for z in b[1]..=b[3] {
        for x in b[0]..=b[2] {
            let h = raw_mm(n, x as i32, z as i32)?;
            let note = if h > level {
                Some("natural_above_design_level")
            } else if i64::from(level) - i64::from(h) > i64::from(max_fill_mm) {
                Some("fill_exceeds_roadbed_limit")
            } else if f64::from(level - h) > mask.fill_capacity_at(x as i32, z as i32) * 1000. {
                Some("fill_exceeds_zoning_capacity")
            } else {
                None
            };
            if let Some(note) = note {
                *detail = Some(RejectDetail {
                    at: Some([x as i32, z as i32]),
                    natural_mm: Some(h),
                    design_mm: Some(level),
                    note: Some(note.into()),
                    ..RejectDetail::of("natural_support_or_fill_limit")
                });
                return Ok(Some("natural_support_or_fill_limit"));
            }
        }
    }
    Ok(None)
}
fn connection_is_reserved(mask: &ZoningMask, e: &EntityInstance, x: i32, z: i32) -> bool {
    let a = &e.anchors[0];
    for zz in a.world_z.min(z) - 2..=a.world_z.max(z) + 2 {
        for xx in a.world_x.min(x) - 2..=a.world_x.max(x) + 2 {
            if mask
                .check("frontage-access".into(), "access", xx, zz, 0, 0)
                .is_some()
            {
                return true;
            }
        }
    }
    false
}
fn preflight(
    site: &RoadsideSite,
    plan: &RoadNetworkPlan,
    n: &BakedTerrain,
    mask: &ZoningMask,
    water: WaterGeometry,
    existing: &[EntityInstance],
    base_carves: &[super::TerrainCarve],
    detail: &mut Option<RejectDetail>,
) -> Result<Option<&'static str>, TerrainError> {
    let e = site.entity();
    validate_entity(&e)?;
    let b = bounds(&e, 2);
    let (hw, hd) = site.world_half_extents();
    let parcel = parcel_boundary(site)?;
    let parcel_half_w = (parcel[2] - parcel[0]) as f64 * 0.5;
    let parcel_half_d = (parcel[3] - parcel[1]) as f64 * 0.5;
    let parcel_center_x = (parcel[0] + parcel[2]) as f64 * 0.5;
    let parcel_center_z = (parcel[1] + parcel[3]) as f64 * 0.5;
    if water.intersects_axis_aligned(
        parcel_center_x,
        parcel_center_z,
        parcel_half_w,
        parcel_half_d,
    ) {
        *detail = Some(RejectDetail {
            at: Some([parcel_center_x as i32, parcel_center_z as i32]),
            ..RejectDetail::of("water")
        });
        return Ok(Some("water"));
    }
    for old in existing {
        let old_bounds = plan
            .roadside_sites
            .iter()
            .find(|s| s.id == old.entity_id)
            .map(parcel_boundary)
            .transpose()?
            .unwrap_or_else(|| {
                bounds(
                    old,
                    if old.kind == "road" {
                        ROAD_PROTECTION_M
                    } else {
                        2
                    },
                )
            });
        if overlap(parcel, old_bounds) {
            *detail = Some(RejectDetail {
                conflicting_entity: Some(old.entity_id.clone()),
                at: Some([
                    (parcel[0].max(old_bounds[0])) as i32,
                    (parcel[1].max(old_bounds[1])) as i32,
                ]),
                ..RejectDetail::of("occupied_or_road_support")
            });
            return Ok(Some("occupied_or_road_support"));
        }
    }
    if let Some(other) = plan
        .roadside_sites
        .iter()
        .find(|s| super::joint_frontage::incompatible(site, s, plan))
    {
        *detail = Some(RejectDetail {
            conflicting_site: Some(other.id.clone()),
            ..RejectDetail::of("reserved_entrance")
        });
        return Ok(Some("reserved_entrance"));
    }
    if let Some(reason) = support_check(
        site,
        n,
        mask,
        (plan.max_roadbed_fill_m * 1000.).round() as i32,
        detail,
    )? {
        return Ok(Some(reason));
    }
    let mut carves = base_carves.to_vec();
    let proposed = super::plan_terrain_carves(std::slice::from_ref(&e));
    carves.extend(proposed.iter().cloned());
    // Adding a building must not alter any previously fixed hard support.
    for old in existing {
        let area = bounds(
            old,
            if old.kind == "road" {
                3
            } else if old.grounding.frontage_road_point.is_some() {
                2
            } else {
                1
            },
        );
        for carve in &proposed {
            let x0 = (carve.cx - carve.half_w - carve.blend_m).floor() as i64;
            let z0 = (carve.cz - carve.half_d - carve.blend_m).floor() as i64;
            let x1 = (carve.cx + carve.half_w + carve.blend_m).ceil() as i64;
            let z1 = (carve.cz + carve.half_d + carve.blend_m).ceil() as i64;
            if !overlap(area, [x0, z0, x1, z1]) {
                continue;
            }
            for z in area[1].max(z0)..=area[3].min(z1) {
                for x in area[0].max(x0)..=area[2].min(x1) {
                    if sample(n, mask, &carves, x as i32, z as i32)?
                        != sample(n, mask, base_carves, x as i32, z as i32)?
                    {
                        *detail = Some(RejectDetail {
                            conflicting_entity: Some(old.entity_id.clone()),
                            at: Some([x as i32, z as i32]),
                            before_mm: Some(sample(
                                n,
                                mask,
                                base_carves,
                                x as i32,
                                z as i32,
                            )?),
                            after_mm: Some(sample(n, mask, &carves, x as i32, z as i32)?),
                            ..RejectDetail::of("existing_support_changed")
                        });
                        return Ok(Some("existing_support_changed"));
                    }
                }
            }
        }
    }
    // The connection is an engineering footprint, not an unchecked graph edge.
    let [cx, cz] = site.frontage_road_point.unwrap();
    let anchor = e.anchors.first().unwrap();
    let ax = i64::from(anchor.world_x);
    let az = i64::from(anchor.world_z);
    let corridor = [
        ax.min(i64::from(cx)) - 2,
        az.min(i64::from(cz)) - 2,
        ax.max(i64::from(cx)) + 3,
        az.max(i64::from(cz)) + 3,
    ];
    let connection_protected = connection_is_reserved(mask, &e, cx, cz);
    if connection_protected
        || water.intersects_corridor(ax as f64, az as f64, cx as f64, cz as f64, 2.)
    {
        *detail = Some(RejectDetail {
            at: Some([ax as i32, az as i32]),
            note: Some(if connection_protected {
                "access_corridor_reserved_zoning".into()
            } else {
                "access_corridor_intersects_water".into()
            }),
            ..RejectDetail::of("entrance_reserved_or_water")
        });
        return Ok(Some("entrance_reserved_or_water"));
    }
    for z in corridor[1]..corridor[3] {
        for x in corridor[0]..corridor[2] {
            let before = raw_mm(n, x as i32, z as i32)?;
            let after = sample(n, mask, &carves, x as i32, z as i32)?;
            if after < before
                || i64::from(after) - i64::from(before)
                    > (plan.max_roadbed_fill_m * 1000.).round() as i64
            {
                *detail = Some(RejectDetail {
                    at: Some([x as i32, z as i32]),
                    before_mm: Some(before),
                    after_mm: Some(after),
                    ..RejectDetail::of("entrance_fill_limit")
                });
                return Ok(Some("entrance_fill_limit"));
            }
        }
    }
    // Narrow complete local raster for the same pedestrian solver used after bake.
    let road = existing
        .iter()
        .find(|r| r.entity_id == site.road_id)
        .ok_or_else(|| err("frontage road missing"))?;
    let (rx, rz) = if road.rotation_y_deg == 0. {
        (e.world_x, road.world_z)
    } else {
        (road.world_x, e.world_z)
    };
    let x0 = (b[0] as i32).min(rx - 4) - 2;
    let z0 = (b[1] as i32).min(rz - 4) - 2;
    let x1 = (b[2] as i32).max(rx + 4) + 2;
    let z1 = (b[3] as i32).max(rz + 4) + 2;
    let local = SceneSpec {
        scene_id: "frontage-preflight".into(),
        width_m: (x1 - x0 + 1) as u32,
        depth_m: (z1 - z0 + 1) as u32,
        origin_x: x0,
        origin_z: z0,
        seed_offset: 0,
    };
    let mut values = Vec::new();
    for z in z0..=z1 {
        for x in x0..=x1 {
            values.push(f64::from(sample(n, mask, &carves, x, z)?) / 1000.);
        }
    }
    let mut raster = BakedTerrain::new(&local)?;
    raster.insert_chunk(
        x0,
        z0,
        local.width_m,
        local.depth_m,
        &encode_millimetres(local.width_m, local.depth_m, values)
            .map_err(|e| err(e.to_string()))?,
    )?;
    // Same final platform must be present, not simply a low centre point.
    let level = site.support_level_mm.unwrap();
    for z in e.world_z - hd as i32 - 2..=e.world_z + hd as i32 + 2 {
        for x in e.world_x - hw as i32 - 2..=e.world_x + hw as i32 + 2 {
            if raw_mm(&raster, x, z)? != level {
                *detail = Some(RejectDetail {
                    at: Some([x, z]),
                    before_mm: Some(level),
                    after_mm: Some(raw_mm(&raster, x, z)?),
                    note: Some("engineered_platform_sample_differs".into()),
                    ..RejectDetail::of("engineered_platform_not_flat")
                });
                return Ok(Some("engineered_platform_not_flat"));
            }
        }
    }
    let mut entities = existing.to_vec();
    entities.push(e.clone());
    let links = super::access::access_for_entity(&e, &entities, &raster, water);
    if !links
        .iter()
        .any(|l| l.connected && l.road_id.as_deref() == Some(&site.road_id))
    {
        let reason = if links.iter().any(|l| l.connected) {
            "access_to_different_road"
        } else {
            match links.first().map(|l| l.status.as_str()) {
                Some("entrance_step_requires_engineering") => "entrance_step_requires_engineering",
                Some("entrance_clearance_blocked") => "entrance_clearance_blocked",
                _ => "no_local_walkable_frontage",
            }
        };
        *detail = Some(RejectDetail {
            note: Some(links.first().map(|l| l.status.clone()).unwrap_or_default()),
            ..RejectDetail::of(reason)
        });
        return Ok(Some(reason));
    }
    Ok(None)
}
/// Immutable-to-the-caller candidate state: the raw source, the zoning mask and
/// the accumulated engineering footprint of everything already accepted. The
/// placement loop and the bounded supply probe share this exact state so a
/// measurement can never disagree with the placement it predicts.
pub(super) struct PlacementState<'a> {
    pub n: &'a BakedTerrain,
    pub mask: &'a ZoningMask,
    pub water: WaterGeometry,
    pub existing: Vec<EntityInstance>,
    pub carves: Vec<super::TerrainCarve>,
}
impl<'a> PlacementState<'a> {
    pub(super) fn new(
        n: &'a BakedTerrain,
        mask: &'a ZoningMask,
        water: WaterGeometry,
        existing: Vec<EntityInstance>,
    ) -> Self {
        let carves = super::plan_terrain_carves(&existing);
        Self {
            n,
            mask,
            water,
            existing,
            carves,
        }
    }
    /// Full production engineering preflight for one candidate.
    pub(super) fn assess(
        &self,
        site: &RoadsideSite,
        plan: &RoadNetworkPlan,
    ) -> Result<Option<&'static str>, TerrainError> {
        let mut detail = None;
        preflight(
            site,
            plan,
            self.n,
            self.mask,
            self.water,
            &self.existing,
            &self.carves,
            &mut detail,
        )
    }
    /// Same preflight decision with the structured evidence of the rejection.
    pub(super) fn assess_detailed(
        &self,
        site: &RoadsideSite,
        plan: &RoadNetworkPlan,
    ) -> Result<(Option<&'static str>, Option<RejectDetail>), TerrainError> {
        let mut detail = None;
        let reason = preflight(
            site,
            plan,
            self.n,
            self.mask,
            self.water,
            &self.existing,
            &self.carves,
            &mut detail,
        )?;
        Ok((reason, detail))
    }
    pub(super) fn sample_natural(
        &self,
        site: &mut RoadsideSite,
        parcel: [i64; 4],
    ) -> Result<(), TerrainError> {
        for zz in parcel[1]..parcel[3] {
            for xx in parcel[0]..parcel[2] {
                let h = raw_mm(self.n, xx as i32, zz as i32)? as f32 / 1000.;
                site.natural_min_m = site.natural_min_m.min(h);
                site.natural_max_m = site.natural_max_m.max(h);
            }
        }
        Ok(())
    }
    /// Accept a site the way the production loop does: its platform becomes
    /// fixed support for every later candidate.
    pub(super) fn commit(&mut self, site: &RoadsideSite) {
        let entity = site.entity();
        self.carves
            .extend(super::plan_terrain_carves(std::slice::from_ref(&entity)));
        self.existing.push(entity);
    }
}
/// Cumulative accounting for one authored request. Production placement and the
/// read-only diagnosis both drive this same struct, so remaining demand and the
/// request's own station budget can never diverge between them: demand is
/// measured across every road, never restarted per road.
#[derive(Debug, Clone)]
pub(super) struct RequestBudget {
    pub request: Request,
    pub site_ids: Vec<String>,
    pub attempted: u32,
    pub rejected: BTreeMap<String, u32>,
}
impl RequestBudget {
    pub(super) fn new(request: &Request) -> Self {
        Self {
            request: request.clone(),
            site_ids: vec![],
            attempted: 0,
            rejected: BTreeMap::new(),
        }
    }
    pub(super) fn satisfied(&self) -> bool {
        self.site_ids.len() >= self.request.count as usize
    }
    pub(super) fn remaining(&self) -> u32 {
        self.request.count.saturating_sub(self.site_ids.len() as u32)
    }
    pub(super) fn budget_left(&self) -> bool {
        self.attempted < self.request.max_candidate_attempts
    }
    /// Claim one station attempt. False means this request is finished for the
    /// whole placement, either by demand or by its own cumulative budget.
    pub(super) fn claim_attempt(&mut self) -> bool {
        if self.satisfied() || !self.budget_left() {
            return false;
        }
        self.attempted += 1;
        true
    }
    pub(super) fn reject(&mut self, reason: &str) {
        *self.rejected.entry(reason.into()).or_default() += 1;
    }
    pub(super) fn accept(&mut self, id: &str) {
        self.site_ids.push(id.into());
    }
    pub(super) fn stop_reason(&self) -> String {
        if self.satisfied() {
            "demand_satisfied".into()
        } else if !self.budget_left() {
            "candidate_budget_exhausted".into()
        } else {
            "no_feasible_frontage".into()
        }
    }
}
/// Try ONE candidate site for ONE request at ONE station and side, recording the outcome on the
/// request's budget. Extracted so the station-sharing loop in `append` stays readable.
///
/// Returns the purpose conversion the accepted parcel earned, if any, so `append` writes the record
/// that the land-use ledger later applies. A rejected peer contributes nothing.
#[allow(clippy::too_many_arguments)]
fn claim_and_place(
    road: &EntityInstance,
    r: &Request,
    scene: &SceneSpec,
    zones: &[LandUseZone],
    state: &mut PlacementState<'_>,
    staged: &mut RoadNetworkPlan,
    budget: &mut RequestBudget,
    station: usize,
    side: i32,
) -> Result<Option<Conversion>, TerrainError> {
    if !budget.claim_attempt() {
        return Ok(None);
    }
    let mut site = candidate_site(road, r, &scene.scene_id, station, side)?;
    let parcel = parcel_boundary(&site)?;
    let (mut reason, converted_m2) = envelope_verdict(
        parcel,
        r.parcel_insets.unwrap_or_default(),
        site.rotation_y_deg,
        scene,
        zones,
        &r.purpose,
        r.rural_conversion,
    );
    if reason.is_none() {
        reason = state.assess(&site, staged)?;
    }
    if let Some(reason) = reason {
        budget.reject(reason);
        return Ok(None);
    }
    state.sample_natural(&mut site, parcel)?;
    state.commit(&site);
    budget.accept(&site.id);
    let conversion = converted_m2.map(|area_m2| Conversion::rural_to_urban(&site.id, area_m2));
    staged.roadside_sites.push(site);
    Ok(conversion)
}

pub(super) fn append(
    plan: &mut RoadNetworkPlan,
    scene: &SceneSpec,
    n: &BakedTerrain,
    water: WaterGeometry,
    zones: Option<&[LandUseZone]>,
    style: &serde_json::Value,
) -> Result<(), TerrainError> {
    let Some(program) = from_style(style)? else {
        return Ok(());
    };
    let requests: Vec<_> = program
        .requests
        .into_iter()
        .filter(|r| r.scene_id == scene.scene_id)
        .collect();
    if requests.is_empty() {
        return Ok(());
    }
    if n.encoding() != Some(HeightEncoding::Millimetre) {
        return Err(err("frontage requires millimetre source"));
    }
    let zones = zones.ok_or_else(|| err("frontage requires zoning"))?;
    let mask = ZoningMask::new(scene, zones)?;
    // The append is atomic on error, while unsatisfied valid demand is reported.
    let mut staged = plan.clone();
    let roads = profile_roads(&staged);
    let mut state = PlacementState::new(
        n,
        &mask,
        water,
        {
            let mut existing = roads.clone();
            existing.extend(staged.site_entities());
            existing
        },
    );
    let mut report = Report {
        version: 1,
        model: "local-road-station-frontage-v1".into(),
        outcomes: vec![],
        conversions: vec![],
    };
    // B01 measured defect: the requests used to be served strictly one after another, each scanning
    // every road and station to exhaustion before the next request was considered. On the real 512 m
    // input the homes request therefore consumed the urban frontage and the shops request realized 0 of
    // 8, with occupied_or_road_support 12. Authored demand must SHARE the limited frontage instead.
    //
    // The allocation is now round-robin over stations: for every road, in station order, each request
    // gets its turn at that station before any request moves on. A request that has met its own demand
    // or exhausted its attempt budget simply skips its turn, so nothing can be starved by an
    // insatiable request ahead of it. The schedule itself lives in `StationSchedule` so the read-only
    // diagnosis walks the very same order instead of re-deriving it. Realization evidence is written
    // in program order afterwards, so the published outcome order is unchanged.
    let mut budgets: Vec<RequestBudget> = requests.iter().map(RequestBudget::new).collect();
    for road in &roads {
        let schedule = StationSchedule::new(road, &requests)?;
        for turn in schedule.turns() {
            let budget = &mut budgets[turn.request];
            if budget.satisfied() || !budget.budget_left() {
                continue;
            }
            if let Some(conversion) = claim_and_place(
                road,
                &requests[turn.request],
                scene,
                zones,
                &mut state,
                &mut staged,
                budget,
                turn.station,
                turn.side,
            )? {
                // Recorded ONLY for an accepted parcel, so the ledger can never apply a
                // conversion for a candidate that engineering refused afterwards.
                report.conversions.push(conversion);
            }
        }
    }
    for (index, r) in requests.iter().enumerate() {
        let budget = &budgets[index];
        report.outcomes.push(Outcome {
            request: r.clone(),
            site_ids: budget.site_ids.clone(),
            attempted: budget.attempted,
            unfilled: budget.remaining(),
            rejected: budget.rejected.clone(),
            stop_reason: budget.stop_reason(),
        });
    }
    staged.frontage = Some(report);
    *plan = staged;
    Ok(())
}
/// The authored street morphology a shoreline commercial row is planned from: its street setback,
/// taken from the world's own first frontage request, or derived from `style.morphologyProfile`
/// exactly as `settlement::morphology_compile` derives it for an unstated request
/// (`frontInset + ROAD_PROTECTION_M`).
///
/// `None` means the world states no street morphology at all, and then there is no shoreline row
/// (`worldgen::shorefront`): a row placed on a setback nobody authored would be this module inventing
/// a street contract, which is the defect, not the fix. Such a world keeps the legacy bayfront band.
pub fn shorefront_row_contract(style: &serde_json::Value) -> Option<u32> {
    if let Ok(Some(program)) = from_style(style) {
        if let Some(request) = program.requests.first() {
            return Some(request.setback_m);
        }
    }
    let profile = crate::settlement::morphology::resolve(style).ok().flatten()?;
    let front =
        crate::settlement::morphology_compile::even_midpoint(profile.front_inset_m, "frontInsetM")
            .ok()?;
    front.checked_add(ROAD_PROTECTION_M as u32)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub status: String,
    pub requested: u32,
    pub planned: usize,
    pub realized_with_access: usize,
    pub unfilled: u32,
    pub failures: Vec<String>,
    /// Machine-readable realized-with-access identities. Additive: the counts
    /// and thresholds above are unchanged, and older documents without it
    /// still deserialize.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub realized_ids: Vec<String>,
}
/// Call after road-network source replay; metadata alone never proves realization.
pub fn audit(plan: &RoadNetworkPlan, entities: &[EntityInstance], access: &AccessGraph) -> Audit {
    let mut a = Audit {
        status: "pass".into(),
        requested: 0,
        planned: 0,
        realized_with_access: 0,
        unfilled: 0,
        failures: vec![],
        realized_ids: vec![],
    };
    if let Some(report) = &plan.frontage {
        for o in &report.outcomes {
            a.requested += o.request.count;
            a.planned += o.site_ids.len();
            a.unfilled += o.unfilled;
            for id in &o.site_ids {
                let site = plan.roadside_sites.iter().find(|s| &s.id == id);
                let found: Vec<_> = entities.iter().filter(|e| &e.entity_id == id).collect();
                let exact = site.is_some_and(|s| {
                    let mut expected = s.entity();
                    expected.normalize_spatial_semantics();
                    expected.bounds = None;
                    found.len() == 1 && {
                        let mut actual = found[0].clone();
                        actual.bounds = None;
                        serde_json::to_value(actual).ok() == serde_json::to_value(expected).ok()
                    }
                });
                if exact
                    && site.is_some_and(|s| {
                        access.links.iter().any(|l| {
                            l.entity_id == *id
                                && l.connected
                                && l.road_id.as_deref() == Some(&s.road_id)
                        })
                    })
                {
                    a.realized_with_access += 1;
                    a.realized_ids.push(id.clone());
                } else {
                    a.failures.push(format!(
                        "frontage building missing/changed/disconnected {id}"
                    ));
                }
            }
        }
    }
    if a.unfilled > 0 {
        a.failures.push("frontage demand remains unfilled".into());
    }
    if !a.failures.is_empty() {
        a.status = "fail".into();
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(
        origin: i32,
    ) -> (
        SceneSpec,
        BakedTerrain,
        WaterGeometry,
        Vec<LandUseZone>,
        RoadNetworkPlan,
        serde_json::Value,
    ) {
        let s = SceneSpec {
            scene_id: "frontage".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: origin,
            origin_z: origin,
            seed_offset: 0,
        };
        let mut n = BakedTerrain::new(&s).unwrap();
        n.insert_chunk(
            origin,
            origin,
            512,
            512,
            &encode_millimetres(512, 512, vec![-2.123; 512 * 512]).unwrap(),
        )
        .unwrap();
        let w = super::super::water_geometry(
            super::super::WaterKind::None,
            &[],
            &s,
            &serde_json::Value::Null,
        );
        let zones = vec![LandUseZone {
            id: "city".into(),
            kind: "urban".into(),
            world_x: origin,
            world_z: origin,
            width_m: 512,
            depth_m: 512,
            source: "test".into(),
        }];
        let mut p:RoadNetworkPlan=serde_json::from_value(serde_json::json!({"model":"fixture","naturalHeightfieldHash":n.content_hash().unwrap(),"sceneId":"frontage","status":"planned_connected_network","gridSpacingM":64,"maxRoadbedFillM":6,"levelM":0,"candidates":1,"rejectedWater":0,"rejectedRelief":0,"unselectedCandidates":0,"nodes":[{"id":0,"worldX":origin+32,"worldZ":origin+128},{"id":1,"worldX":origin+96,"worldZ":origin+128}],"edges":[{"id":"base","from":0,"to":1,"worldX":origin+64,"worldZ":origin+128,"lengthM":64,"rotationYDeg":0,"naturalMinM":-2,"naturalMaxM":-2}],"roadsideSites":[]})).unwrap();
        let ext = serde_json::json!({"roadExtensions":{"version":1,"corridors":[{"sceneId":"frontage","start":[origin+96,origin+128],"end":[origin+352,origin+128],"roadWidthM":8,"shoulderWidthM":3,"maxFillMm":6000,"maxRiseMmPerM":80,"startLevelMm":0,"endLevelMm":2374,"junctionLengthM":8}]}});
        super::super::road_extensions::append(&mut p, &s, &n, w, &zones, &ext).unwrap();
        let style = serde_json::json!({"frontageProgram":{"version":1,"requests":[{"id":"homes","sceneId":"frontage","purpose":"urban","kind":"residential_home","count":4,"widthM":12,"depthM":10,"heightM":8,"stationSpacingM":16,"setbackM":8,"maxCandidateAttempts":128}]}});
        (s, n, w, zones, p, style)
    }
    #[test]
    fn local_millimetre_buildings_have_access_and_preserve_roads() {
        let (s, n, w, z, mut p, style) = fixture(0);
        let old = serde_json::to_value(p.entities()).unwrap();
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(o.unfilled, 0, "{:?}", o);
        assert_eq!(serde_json::to_value(p.entities()).unwrap(), old);
        assert_eq!(p.roadside_sites.len(), 4);
        assert!(
            p.roadside_sites
                .iter()
                .any(|s| s.support_level_mm.unwrap() % 1000 != 0)
        );
        for site in &p.roadside_sites {
            let mm = site.support_level_mm.unwrap();
            let e = site.entity();
            assert_eq!(e.world_y, mm.div_euclid(1000));
            assert_eq!((e.ground_height_m() * 1000.).round() as i32, mm);
        }
        let mut again = fixture(0).4;
        append(&mut again, &s, &n, w, Some(&z), &style).unwrap();
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::to_value(again).unwrap()
        );
    }
    #[test]
    fn explicit_parcel_insets_are_validated_preserved_and_used_for_conflicts() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        style["frontageProgram"]["requests"][0]["parcelInsets"] = insets.clone();
        style["frontageProgram"]["requests"][0]["count"] = serde_json::json!(3);
        style["frontageProgram"]["requests"][0]["stationSpacingM"] = serde_json::json!(16);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        assert!(p
            .roadside_sites
            .iter()
            .all(|site| site.parcel_insets.is_some()));
        for site in &p.roadside_sites {
            let boundary = parcel_boundary(site).unwrap();
            assert_eq!(boundary[2] - boundary[0], 20);
            assert_eq!(boundary[3] - boundary[1], 26);
        }
        assert!(p.frontage.as_ref().unwrap().outcomes[0]
            .rejected
            .contains_key("occupied_or_road_support"));

        let mut too_deep_front = fixture(0).5;
        too_deep_front["frontageProgram"]["requests"][0]["parcelInsets"] =
            serde_json::json!({"frontM":10,"sideM":4,"rearM":12});
        assert!(from_style(&too_deep_front).is_err());
    }
    #[test]
    fn the_declared_depth_is_the_parcel_depth_not_the_building_depth() {
        // B01: the 1 km demand realized 0 of 120 homes while the urban zoning had room for 252 of them.
        // The candidate position depends on r.depth_m through
        //   offset = road/2 + setback + r.depth_m/2
        // so r.depth_m is used as the PARCEL depth. This test states the relationship explicitly so a
        // future placement change cannot silently reinterpret the field.
        let (s, n, w, z, mut p, style) = fixture(0);
        let mut p2 = p.clone();
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        // same request but declaring a deeper parcel must move the parcel centre further from the road
        let mut style2 = style.clone();
        style2["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(20);
        append(&mut p2, &s, &n, w, Some(&z), &style2).unwrap();
        let a = &p.roadside_sites[0];
        let b = &p2.roadside_sites[0];
        let da = (a.world_z - a.frontage_road_point.unwrap()[1]).abs();
        let db = (b.world_z - b.frontage_road_point.unwrap()[1]).abs();
        assert!(
            db > da,
            "declaring a deeper parcel must place it further from the road: {da} vs {db}"
        );
        assert_eq!(db - da, 5, "the shift must be exactly half the depth difference");
        p = p2;
        assert!(!p.roadside_sites.is_empty());
    }
    #[test]
    fn a_large_parcel_can_still_be_placed_inside_a_full_urban_zone() {
        // B01 question: at 1 km the authored demand is 120 homes at 24x24 m and it realized 0 of 120
        // while multiplying the urban zoning 3.5x changed nothing. Before blaming the parcel size,
        // check the cheapest case: inside a zone that is entirely urban, can a 24x24 m parcel with the
        // same setback be placed at all? If yes, parcel size is not the blocker by itself.
        let (s, n, w, z, mut p, mut style) = fixture(0);
        style["frontageProgram"]["requests"][0]["widthM"] = serde_json::json!(24);
        style["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(24);
        style["frontageProgram"]["requests"][0]["count"] = serde_json::json!(2);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(
            o.unfilled, 0,
            "a 24x24 m parcel must be placeable inside a fully urban zone; got {:?}",
            o
        );
        assert_eq!(p.roadside_sites.len(), 2);
    }
    #[test]
    fn an_oversubscribed_request_does_not_starve_a_later_one() {
        // B01 measured defect: on the real 512 m input the homes request was served first and consumed
        // the urban frontage, so the shops request realized 0 of 8 with occupied_or_road_support 12.
        // This fixture asks for far more homes than the road can carry and then asks for shops: the
        // homes request must not be able to take every station before the shops request is considered.
        let (s, n, w, z, mut p, mut style) = fixture(0);
        let mut homes = style["frontageProgram"]["requests"][0].clone();
        homes["id"] = serde_json::json!("homes");
        homes["count"] = serde_json::json!(300);
        let mut shops = homes.clone();
        shops["id"] = serde_json::json!("shops");
        shops["kind"] = serde_json::json!("storefront");
        shops["count"] = serde_json::json!(4);
        style["frontageProgram"]["requests"] = serde_json::json!([homes, shops]);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let outcomes = &p.frontage.as_ref().unwrap().outcomes;
        let home_outcome = outcomes.iter().find(|o| o.request.id == "homes").unwrap();
        let shop_outcome = outcomes.iter().find(|o| o.request.id == "shops").unwrap();
        // the shops request must get its share even though the homes request is insatiable
        assert!(
            !shop_outcome.site_ids.is_empty(),
            "the later request was starved: shops got {} of 4 while homes took {} ({:?})",
            shop_outcome.site_ids.len(),
            home_outcome.site_ids.len(),
            shop_outcome
        );
        // and the homes request must not have consumed every station for itself
        assert!(
            home_outcome.site_ids.len() < 300,
            "the fixture must remain oversubscribed so the sharing rule is actually exercised"
        );
    }
    #[test]
    fn feasible_long_profiled_control_realizes_twenty_four_explicit_parcels() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        let context = super::super::road_alignment::AlignmentContext::new(&s, &n, w, &z).unwrap();
        let request = super::super::road_alignment::CorridorRequest {
            scene_id: "frontage".into(),
            start: [352, 128],
            end: [480, 128],
            road_width_m: 8,
            shoulder_width_m: 3,
            max_fill_mm: 6000,
            max_rise_mm_per_m: 80,
            start_level_mm: Some(2374),
            end_level_mm: Some(3000),
            junction_length_m: 8,
        };
        super::super::road_extensions::append_one(
            &mut p,
            &context,
            &request,
            "generated.road-extension.frontage.1".into(),
        )
        .unwrap();
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        let mut homes = style["frontageProgram"]["requests"][0].clone();
        homes["id"] = serde_json::json!("homes-b01");
        homes["count"] = serde_json::json!(16);
        homes["stationSpacingM"] = serde_json::json!(24);
        homes["parcelInsets"] = insets.clone();
        let mut shops = homes.clone();
        shops["id"] = serde_json::json!("shops-b01");
        shops["kind"] = serde_json::json!("storefront");
        shops["heightM"] = serde_json::json!(6);
        shops["count"] = serde_json::json!(8);
        style["frontageProgram"]["requests"] = serde_json::json!([homes, shops]);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let outcomes = &p.frontage.as_ref().unwrap().outcomes;
        assert_eq!(outcomes.iter().map(|o| o.unfilled).sum::<u32>(), 0, "{outcomes:?}");
        assert_eq!(p.roadside_sites.len(), 24);
        for site in &p.roadside_sites {
            let boundary = parcel_boundary(site).unwrap();
            assert_eq!((boundary[2] - boundary[0]) * (boundary[3] - boundary[1]), 520);
            assert!(site.parcel_insets.is_some());
        }
    }
    #[test]
    fn demand_and_budget_shortfalls_are_not_resized_or_silently_passed() {
        let (s, n, w, z, mut p, mut style) = fixture(0);
        style["frontageProgram"]["requests"][0]["count"] = serde_json::json!(100);
        style["frontageProgram"]["requests"][0]["maxCandidateAttempts"] = serde_json::json!(1);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(o.attempted, 1);
        assert!(o.unfilled >= 99);
        assert_eq!(o.stop_reason, "candidate_budget_exhausted");
        assert_eq!(audit(&p, &[], &AccessGraph::default()).status, "fail");
    }
    #[test]
    fn a_zone_edge_across_the_rear_setback_does_not_reject_the_parcel() {
        // B01 measured defect: the asset contract forces residential_home to be at least 24x24 m, the
        // placement puts the parcel roughly 12 to 36 m from the road, and the 128 m zoning grid cut by
        // roads leaves a narrower band than that. The whole-parcel coverage rule therefore rejected
        // parcels whose BUILDING SITE was in urban land, because the rear SETBACK reached across the
        // zone edge. Setback land is not built land, so it must not be what decides the purpose.
        //
        // The test calibrates itself from the product's own geometry: it first appends with pure urban
        // zoning to learn where the parcel actually lands, then places a small RURAL zone over that
        // parcel's rear band (leaving the full urban zone underneath, so the scene stays fully zoned)
        // and requires the parcel to still be realized.
        let (s, n, w, z, mut p, style) = fixture(0);
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        let mut home = style["frontageProgram"]["requests"][0].clone();
        home["id"] = serde_json::json!("homes-rear-edge");
        home["widthM"] = serde_json::json!(24);
        home["depthM"] = serde_json::json!(24);
        home["count"] = serde_json::json!(2);
        home["parcelInsets"] = insets.clone();
        let mut style2 = style.clone();
        style2["frontageProgram"]["requests"] = serde_json::json!([home]);
        let mut probe = p.clone();
        if let Err(e) = append(&mut probe, &s, &n, w, Some(&z), &style2) {
            panic!("append failed for the rear-edge fixture probe: {}", e.0);
        }
        let site = &probe.roadside_sites[0];
        let parcel = parcel_boundary(site).unwrap();
        // the rear band as a SEPARATE non-overlapping zone. It is made deeper than the rear SETBACK so
        // the whole-parcel coverage really does fall below the threshold, which is the case this test
        // exists to pin: the parcel fails whole-parcel coverage while its buildable core stays urban.
        // The original urban zone is shortened so the two tile the scene without overlapping (the
        // validator rejects overlaps).
        //
        // B01-R1 T1: the split axis depends on the site's ROTATION, because the rear setback runs along
        // the building's local forward axis (world Z at yaw 0/180, world X at yaw 90/270). The previous
        // version assumed world Z, which was only valid before the core became rotation-aware.
        let yaw = site.rotation_y_deg.round() as i32;
        let rel_z = matches!(yaw, 0 | 180);
        let front_inset = i64::from(insets["frontM"].as_i64().unwrap_or(0));
        let core_span = if rel_z {
            (parcel[3] - parcel[1]) - front_inset - 2
        } else {
            (parcel[2] - parcel[0]) - front_inset - 2
        };
        let cut = if rel_z {
            (parcel[1] + front_inset + 1 + core_span) as i32
        } else {
            (parcel[0] + front_inset + 1 + core_span) as i32
        };
        let mut z2 = z.clone();
        for zone in z2.iter_mut() {
            if zone.kind != "urban" {
                continue;
            }
            let start = if rel_z { zone.world_z } else { zone.world_x };
            let len = if rel_z { zone.depth_m } else { zone.width_m };
            let end = start + i32::try_from(len).unwrap();
            let clamped = cut.clamp(start + 1, (end - 1).max(start + 1));
            let new_len = u32::try_from(clamped - start).unwrap_or(1).max(1);
            if rel_z {
                zone.depth_m = new_len;
            } else {
                zone.width_m = new_len;
            }
        }
        z2.push(if rel_z {
            LandUseZone {
                id: "rear-band".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: cut,
                width_m: 512,
                depth_m: (512 - cut).max(1) as u32,
                source: "test-rear-band".into(),
            }
        } else {
            LandUseZone {
                id: "rear-band".into(),
                kind: "rural".into(),
                world_x: cut,
                world_z: 0,
                width_m: (512 - cut).max(1) as u32,
                depth_m: 512,
                source: "test-rear-band".into(),
            }
        });
        if let Err(e) = append(&mut p, &s, &n, w, Some(&z2), &style2) {
            panic!("append failed for the rear-edge fixture: {}", e.0);
        }
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(
            o.unfilled, 0,
            "a parcel whose buildable core is inside the urban zone must not be rejected because its \
             rear setback crosses the zone edge; got {:?}",
            o
        );
        assert_eq!(p.roadside_sites.len(), 2);
        // NOTE: the purpose gate may still fire for OTHER stations in this fixture; what this test
        // pins is that it does not prevent THESE two parcels from being realized.
    }
    #[test]
    fn a_parcel_rectangle_that_includes_protected_land_is_refused() {
        // B05 measured defect on the seed-42 village-farmland fixture: two urban shops
        // earned by `ruralConversion: "explicit"` had their BUILDABLE CORE wholly in
        // rural land (so the conversion gate accepted them) while their parcel
        // rectangles, which is what the development plan partitions into yard cells,
        // reached 12 m into a nature zone through the 20 m rear inset. 480 m2 of a
        // protected cell became yard cells, the conservation surface fell from 49152
        // to 48672 m2, and the nature ledger booked the difference as unresolved.
        //
        // The fixture below is that geometry in miniature: a 40 x 80 m parcel whose
        // core [13,13,43,75] is entirely rural, and a protected band that starts where
        // the core ends, so ONLY the parcel rectangle test can refuse it.
        let scene = SceneSpec {
            scene_id: "protected-parcel".into(),
            width_m: 128,
            depth_m: 128,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        };
        let zone = |id: &str, kind: &str, z: i32, depth: u32| LandUseZone {
            id: id.into(),
            kind: kind.into(),
            world_x: 0,
            world_z: z,
            width_m: 128,
            depth_m: depth,
            source: "test".into(),
        };
        let insets = ParcelInsets {
            front_m: 4,
            side_m: 4,
            rear_m: 12,
        };
        let parcel = [8_i64, 8, 48, 88];
        assert_eq!(
            buildable_core(parcel, insets, 0.),
            Some([13, 13, 43, 75]),
            "the core must be the part the conversion gate measures"
        );
        let rural_only = vec![zone("rural", "rural", 0, 128)];
        let protected_tail = vec![zone("rural", "rural", 0, 76), zone("preserve", "nature", 76, 52)];
        // The protected band shares exactly 40 m x 12 m with the parcel: the same 480 m2
        // the seed-42 fixture lost.
        assert_eq!(
            intersection_area(parcel, &protected_tail[1]),
            480,
            "the protected band must overlap the parcel by the measured 480 m2"
        );
        // Without the protected band the conversion is legal: the core is wholly rural.
        assert_eq!(
            envelope_verdict(
                parcel,
                insets,
                0.,
                &scene,
                &rural_only,
                "urban",
                RuralConversion::RuralToUrban
            ),
            (None, Some(area([13, 13, 43, 75])))
        );
        // The historical (switch-off) gate refuses the urban request either way, but the
        // reason must be the purpose/parcel rule, not an unrelated failure.
        assert_eq!(
            envelope_verdict(
                parcel,
                insets,
                0.,
                &scene,
                &protected_tail,
                "urban",
                RuralConversion::RuralToUrban
            ),
            (Some("wrong_purpose_or_protected"), None)
        );
        assert_eq!(
            envelope_reason(parcel, insets, 0., &scene, &protected_tail, "urban"),
            Some("wrong_purpose_or_protected")
        );
        // A rural request on the same parcel is refused for the same reason: protected
        // land is not a matter of purpose.
        assert_eq!(
            envelope_reason(parcel, insets, 0., &scene, &protected_tail, "rural"),
            Some("wrong_purpose_or_protected")
        );
        // A parcel that stops at the protected boundary keeps its permission, so the rule
        // refuses the overlap and not the neighbourhood.
        let clear = [8_i64, 8, 48, 76];
        assert_eq!(
            envelope_verdict(
                clear,
                insets,
                0.,
                &scene,
                &protected_tail,
                "urban",
                RuralConversion::RuralToUrban
            ),
            (None, Some(area([13, 13, 43, 63])))
        );
    }
    #[test]
    fn a_protected_band_across_the_rear_setback_rejects_the_parcel_end_to_end() {
        // Same calibration as `a_zone_edge_across_the_rear_setback_does_not_reject_the_parcel`,
        // but the rear band is a NATURE zone instead of a rural one. The buildable core
        // stays in urban land and the parcel is only refused by the parcel-rectangle rule,
        // which is exactly the seed-42 B05 shape reached through the real `append` path.
        let (s, n, w, z, mut p, style) = fixture(0);
        let insets = serde_json::json!({"frontM":4,"sideM":4,"rearM":12});
        let mut home = style["frontageProgram"]["requests"][0].clone();
        home["id"] = serde_json::json!("homes-rear-protected");
        home["widthM"] = serde_json::json!(24);
        home["depthM"] = serde_json::json!(24);
        home["count"] = serde_json::json!(2);
        home["parcelInsets"] = insets.clone();
        let mut style2 = style.clone();
        style2["frontageProgram"]["requests"] = serde_json::json!([home]);
        let mut probe = p.clone();
        append(&mut probe, &s, &n, w, Some(&z), &style2).unwrap();
        assert_eq!(probe.roadside_sites.len(), 2, "the calibration must place both");
        let site = &probe.roadside_sites[0];
        let parcel = parcel_boundary(site).unwrap();
        let yaw = site.rotation_y_deg.round() as i32;
        let rel_z = matches!(yaw, 0 | 180);
        let front_inset = i64::from(insets["frontM"].as_i64().unwrap_or(0));
        let core_span = if rel_z {
            (parcel[3] - parcel[1]) - front_inset - 2
        } else {
            (parcel[2] - parcel[0]) - front_inset - 2
        };
        let cut = if rel_z {
            (parcel[1] + front_inset + 1 + core_span) as i32
        } else {
            (parcel[0] + front_inset + 1 + core_span) as i32
        };
        let mut z2 = z.clone();
        for zone in z2.iter_mut() {
            if zone.kind != "urban" {
                continue;
            }
            let start = if rel_z { zone.world_z } else { zone.world_x };
            let len = if rel_z { zone.depth_m } else { zone.width_m };
            let end = start + i32::try_from(len).unwrap();
            let clamped = cut.clamp(start + 1, (end - 1).max(start + 1));
            let new_len = u32::try_from(clamped - start).unwrap_or(1).max(1);
            if rel_z {
                zone.depth_m = new_len;
            } else {
                zone.width_m = new_len;
            }
        }
        z2.push(if rel_z {
            LandUseZone {
                id: "rear-preserve".into(),
                kind: "nature".into(),
                world_x: 0,
                world_z: cut,
                width_m: 512,
                depth_m: (512 - cut).max(1) as u32,
                source: "test-rear-preserve".into(),
            }
        } else {
            LandUseZone {
                id: "rear-preserve".into(),
                kind: "nature".into(),
                world_x: cut,
                world_z: 0,
                width_m: (512 - cut).max(1) as u32,
                depth_m: 512,
                source: "test-rear-preserve".into(),
            }
        });
        append(&mut p, &s, &n, w, Some(&z2), &style2).unwrap();
        // The calibrated parcel is the one that WOULD have been accepted: its buildable
        // core is still entirely urban, so the historical purpose gate alone still lets
        // it through, and only the parcel rectangle reaches into the preserve.
        let preserve = z2.iter().find(|zone| zone.kind == "nature").unwrap();
        assert!(
            intersection_area(parcel, preserve) > 0,
            "the preserve must reach into the calibrated parcel"
        );
        assert!(
            in_purpose_core(
                parcel,
                site.parcel_insets.unwrap_or_default(),
                site.rotation_y_deg,
                &z2,
                "urban"
            ),
            "the buildable core must stay inside the urban zone, so the purpose gate is not the reason"
        );
        assert_eq!(
            envelope_verdict(
                parcel,
                site.parcel_insets.unwrap_or_default(),
                site.rotation_y_deg,
                &s,
                &z2,
                "urban",
                RuralConversion::Disabled,
            ),
            (Some("wrong_purpose_or_protected"), None),
            "the parcel rectangle rule must be the refusal"
        );
        let o = &p.frontage.as_ref().unwrap().outcomes[0];
        assert_eq!(
            o.unfilled, 2,
            "a parcel whose rectangle claims protected land must not be realized: {o:?}"
        );
        assert_eq!(p.roadside_sites.len(), 0);
        // Every attempt in this fixture is refused under the same key: the preserve cuts
        // the corridor's frontage, and no station may buy it back.
        assert_eq!(
            o.rejected.get("wrong_purpose_or_protected").copied(),
            Some(o.attempted),
            "the refusal must be reported under the purpose/protection key: {o:?}"
        );
    }
    #[test]
    fn explicit_rural_to_urban_conversion_is_off_by_default_and_never_takes_protected_land() {
        // B01 measured defect: on the real 1 km input the urban frontage requests are rejected with
        // `wrong_purpose_or_protected` 96-98% of the time, because the zoning cell is cut by the road
        // corridor and the parcel's buildable core stands in rural land next to an urban zone. The
        // authored switch `frontageProgram.ruralConversion: "explicit"` may convert exactly that land
        // and nothing else. This test pins the whole boundary of that permission:
        //   * switch OFF -> zero conversions and a plan that cannot even gain the new key,
        //   * switch ON  -> 4 of 4 urban homes on rural land, 80 m2 of core each, 320 m2 in total,
        //   * nature / unassigned land and water are still refused, with nothing recorded.
        let (s, n, w, urban_zones, base, style) = fixture(0);
        // The fixture corridor runs east-west at z = 128 and every candidate parcel stands between
        // z = 104 and z = 152, so one horizontal cut at z = 100 decides the purpose of EVERY candidate.
        let banded = |kind: &str| -> Vec<LandUseZone> {
            vec![
                LandUseZone {
                    id: "north".into(),
                    kind: "urban".into(),
                    world_x: 0,
                    world_z: 0,
                    width_m: 512,
                    depth_m: 100,
                    source: "test-north".into(),
                },
                LandUseZone {
                    id: "band".into(),
                    kind: kind.into(),
                    world_x: 0,
                    world_z: 100,
                    width_m: 512,
                    depth_m: 412,
                    source: "test-band".into(),
                },
            ]
        };
        let rural = banded("rural");
        // OFF is the historical behaviour: an urban request earns nothing on rural land.
        let mut off = base.clone();
        append(&mut off, &s, &n, w, Some(&rural), &style).unwrap();
        let off_report = off.frontage.as_ref().unwrap();
        let off_outcome = &off_report.outcomes[0];
        assert!(off.roadside_sites.is_empty(), "{off_outcome:?}");
        assert_eq!(off_outcome.unfilled, 4);
        assert!(off_report.conversions.is_empty());
        assert_eq!(
            off_outcome
                .rejected
                .get("wrong_purpose_or_protected")
                .copied()
                .unwrap_or(0),
            off_outcome.attempted,
            "with the switch off the historical purpose gate must refuse every attempt"
        );
        let off_json = serde_json::to_value(&off).unwrap();
        assert!(
            off_json["frontage"].get("conversions").is_none(),
            "a plan without conversions must not gain the conversions key"
        );
        assert!(
            off_json["frontage"]["outcomes"][0]["request"]
                .get("ruralConversion")
                .is_none(),
            "the derived policy must never leak into a published request"
        );
        // Only the exact string enables the switch; every other spelling is the historical gate.
        for other in ["", "off", "true", "Explicit"] {
            let mut disabled = base.clone();
            let mut other_style = style.clone();
            other_style["frontageProgram"]["ruralConversion"] = serde_json::json!(other);
            append(&mut disabled, &s, &n, w, Some(&rural), &other_style).unwrap();
            assert_eq!(
                serde_json::to_value(&disabled).unwrap(),
                off_json,
                "the switch must accept the exact string \"explicit\" only, not {other:?}"
            );
        }
        // ON: the same four urban homes land on rural land. Each converts the 10x8 m buildable core of
        // its 16x14 m parcel (12x10 m building, legacy 2 m insets and the 1 m skin are not built land).
        let mut explicit_style = style.clone();
        explicit_style["frontageProgram"]["ruralConversion"] = serde_json::json!("explicit");
        let mut on = base.clone();
        append(&mut on, &s, &n, w, Some(&rural), &explicit_style).unwrap();
        let report = on.frontage.as_ref().unwrap();
        assert_eq!(report.outcomes[0].unfilled, 0, "{:?}", report.outcomes[0]);
        assert_eq!(report.outcomes[0].site_ids.len(), 4);
        assert_eq!(report.conversions.len(), 4);
        assert_eq!(report.conversions.iter().map(|c| c.area_m2).sum::<i64>(), 320);
        for conversion in &report.conversions {
            assert_eq!(conversion.from, "rural");
            assert_eq!(conversion.to, "urban");
            assert_eq!(conversion.area_m2, 80);
            assert!(report.outcomes[0].site_ids.contains(&conversion.parcel_id));
            assert_eq!(verify_conversion(&on, &rural, None, conversion).unwrap(), 80);
        }
        // PROTECTED: nature and unassigned land are not convertible, so the switch earns nothing there.
        for kind in ["nature", "unassigned"] {
            let zones = banded(kind);
            let mut protected = base.clone();
            append(&mut protected, &s, &n, w, Some(&zones), &explicit_style).unwrap();
            let report = protected.frontage.as_ref().unwrap();
            assert!(
                protected.roadside_sites.is_empty(),
                "the switch must not convert {kind} land"
            );
            assert_eq!(report.outcomes[0].unfilled, 4);
            assert!(report.conversions.is_empty());
            assert_eq!(
                report.outcomes[0]
                    .rejected
                    .get("wrong_purpose_or_protected")
                    .copied()
                    .unwrap_or(0),
                report.outcomes[0].attempted,
                "{kind} land must still be refused by the purpose gate"
            );
        }
        // WATER: the engineering preflight still refuses a wet parcel, so it is never converted.
        let lake = WaterGeometry {
            kind: super::super::WaterKind::Lake,
            center_x: 256.,
            center_z: 128.,
            half_width: 400.,
            half_depth: 200.,
            scene_width_m: s.width_m,
            scene_depth_m: s.depth_m,
            terrain_origin_x: s.origin_x,
            terrain_origin_z: s.origin_z,
            smooth_rolling: false,
            road_water_crossings: false,
        };
        let mut wet = base.clone();
        append(&mut wet, &s, &n, lake, Some(&rural), &explicit_style).unwrap();
        let report = wet.frontage.as_ref().unwrap();
        assert!(
            wet.roadside_sites.is_empty(),
            "a wet parcel must never be converted"
        );
        assert_eq!(report.outcomes[0].unfilled, 4);
        assert!(report.conversions.is_empty());
        assert!(
            report.outcomes[0].rejected.contains_key("water"),
            "{:?}",
            report.outcomes[0]
        );
        // and the audit primitive refuses the same record once water is part of the evidence
        for conversion in &on.frontage.as_ref().unwrap().conversions {
            assert_eq!(
                verify_conversion(&on, &rural, Some(lake), conversion).unwrap_err(),
                format!("conversion parcel {} touches water", conversion.parcel_id)
            );
        }
        // A parcel whose buildable core is urban land cannot be claimed as a conversion at all.
        let mut urban_plan = base.clone();
        append(&mut urban_plan, &s, &n, w, Some(&urban_zones), &style).unwrap();
        let parcel_id = urban_plan.frontage.as_ref().unwrap().outcomes[0].site_ids[0].clone();
        let forged = Conversion::rural_to_urban(&parcel_id, 80);
        assert_eq!(
            verify_conversion(&urban_plan, &urban_zones, None, &forged).unwrap_err(),
            format!("conversion parcel {parcel_id} core is not wholly rural land")
        );
    }

    #[test]
    fn wrong_purpose_and_protection_do_not_earn_buildings() {
        let (s, n, w, mut z, mut p, style) = fixture(0);
        z[0].kind = "nature".into();
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        assert!(p.roadside_sites.is_empty());
        assert_eq!(p.frontage.as_ref().unwrap().outcomes[0].unfilled, 4);
        assert!(
            p.frontage.as_ref().unwrap().outcomes[0]
                .rejected
                .contains_key("wrong_purpose_or_protected")
        );
    }
    #[test]
    fn all_four_orientations_are_reachable_and_a_neighbour_conflict_is_refused() {
        // G34-R5-B01 task book line 86 requires negative cases for the four orientations and for a
        // neighbour conflict. The fixture's corridor runs EAST-WEST, so a request against it produces only
        // yaw 0 (north side) and yaw 180 (south side); the east and west orientations are exercised by no
        // existing test. An attempt to add a north-south corridor to the fixture failed with
        // "duplicate road extension id", so this drives the production candidate function directly.
        let (s, n, w, z, p, style) = fixture(0);
        let profiled: Vec<_> = p
            .entities()
            .into_iter()
            .filter(|r| r.grounding.road_profile.is_some())
            .collect();
        assert!(
            !profiled.is_empty(),
            "the fixture must provide at least one profile-bearing road"
        );
        let r = from_style(&style).unwrap().unwrap().requests[0].clone();
        let mut seen = std::collections::BTreeSet::new();
        // the east-west road gives yaw 0 and 180
        for road in &profiled {
            for side in [1, -1] {
                let Some(stations) = stations(road, &r).unwrap() else {
                    continue;
                };
                let Some(station) = stations.first().copied() else {
                    continue;
                };
                let site = candidate_site(road, &r, &s.scene_id, station, side).unwrap();
                assert!(
                    matches!(site.rotation_y_deg, 0. | 90. | 180. | 270.),
                    "a candidate must face one of the four legal directions, got {}",
                    site.rotation_y_deg
                );
                seen.insert(site.rotation_y_deg as i32);
                // NEGATIVE CASE: a parcel pushed outside the scene must be refused by the envelope gate
                let parcel = parcel_boundary(&site).unwrap();
                let outside = [parcel[0] - 5000, parcel[1], parcel[2] - 5000, parcel[3]];
                assert_eq!(
                    envelope_reason(
                        outside,
                        site.parcel_insets.unwrap_or_default(),
                        site.rotation_y_deg,
                        &s,
                        &z,
                        "urban"
                    ),
                    Some("outside_scene"),
                    "a parcel outside the scene must be refused by the envelope gate"
                );
            }
        }
        // the north-south direction gives yaw 90 and 270. The fixture only builds an EAST-WEST
        // corridor, so this constructs a road that satisfies every clause of RoadProfile::validate_entity
        // rather than cloning and rotating one (which fails that validation).
        let ns_profile = super::super::road_profile::RoadProfile {
            version: 1,
            road_width_m: 12,
            start: [200, 128],
            end: [200, 256],
            elevations_mm: (0..=128).map(|i| i64::from(i) * 5).collect(),
            max_fill_mm: 0,
        };
        let ns = EntityInstance {
            entity_id: "fixture.north-south".into(),
            asset_id: "prop.road".into(),
            kind: "road".into(),
            world_x: 200,
            world_z: 192,
            // validate_entity requires ground_height_m() == profile.height_at(world_x, world_z); the
            // profile rises 5 mm per metre from z=128, so at its centre (z=192) the height is 0.320 m.
            world_y: 0,
            width_m: 128.,
            depth_m: 12.,
            height_m: 0.24,
            rotation_y_deg: 90.,
            scale: 1.,
            grounding: super::super::GroundingSpec {
                road_profile: Some(ns_profile),
                bottom_offset_m: 0.320,
                ..Default::default()
            },
            anchors: Vec::new(),
            bounds: None,
        };
        ns.grounding
            .road_profile
            .as_ref()
            .unwrap()
            .validate_entity(&ns)
            .expect("the constructed north-south road must satisfy the profile contract");
        for side in [1, -1] {
            let Some(stations) = stations(&ns, &r).unwrap() else {
                panic!("the north-south road must support stations");
            };
            let Some(station) = stations.first().copied() else {
                panic!("the north-south road must have at least one station");
            };
            let site = candidate_site(&ns, &r, &s.scene_id, station, side).unwrap();
            seen.insert(site.rotation_y_deg as i32);
        }
        assert_eq!(
            seen,
            [0, 90, 180, 270].into_iter().collect(),
            "all four legal orientations must be reachable through the production candidate function"
        );

        // NEGATIVE CASE, neighbour conflict: a demand larger than the frontage can hold must report
        // explicit rejections rather than silently shrinking
        let mut placed = p.clone();
        let mut style_full = style.clone();
        style_full["frontageProgram"]["requests"][0]["count"] = serde_json::json!(16);
        append(&mut placed, &s, &n, w, Some(&z), &style_full).unwrap();
        let outcomes = &placed.frontage.as_ref().unwrap().outcomes;
        assert!(
            outcomes
                .iter()
                .any(|o| !o.rejected.is_empty() || o.unfilled > 0),
            "an over-large demand must produce explicit rejections or unfilled requests, got {:?}",
            outcomes
        );
    }
    #[test]
    fn asymmetric_insets_rotate_the_buildable_core_with_the_building() {
        // B01-R1 T1. ParcelInsets.front_m is the inset along the building's LOCAL forward direction
        // (north at yaw 0, west 90, south 180, east 270), while side_m runs along the local right/left
        // axis. The core must therefore rotate with the building.
        //
        // The earlier version subtracted front from world Z and rear from world Z unconditionally, which
        // happens to be right at yaw 0 and WRONG at 90/180/270: it applied the insets on the wrong axis
        // and swapped front against rear. This test states the expected core for the asymmetric case
        // front=4, side=2, rear=12 by hand, for every cardinal rotation, and calls the production
        // function rather than a copy of its logic.
        let insets = ParcelInsets { front_m: 4, side_m: 2, rear_m: 12 };
        const SKIN: i64 = 1;
        // a parcel rectangle far from any zone, used only for its bounds
        let b: [i64; 4] = [0, 0, 40, 60]; // x0,z0,x1,z1
        let f = i64::from(insets.front_m) + SKIN; // 5
        let s = i64::from(insets.side_m) + SKIN; // 3
        let r = i64::from(insets.rear_m) + SKIN; // 13
        let expected: [(f32, [i64; 4]); 4] = [
            // yaw 0: forward is -Z, so front shrinks z0 and rear shrinks z1; sides act on x
            (0., [b[0] + s, b[1] + f, b[2] - s, b[3] - r]),
            // yaw 90: forward is -X, so front shrinks x0 and rear shrinks x1; sides act on z
            (90., [b[0] + f, b[1] + s, b[2] - r, b[3] - s]),
            // yaw 180: forward is +Z, so rear shrinks z0 and front shrinks z1
            (180., [b[0] + s, b[1] + r, b[2] - s, b[3] - f]),
            // yaw 270: forward is +X, so rear shrinks x0 and front shrinks x1
            (270., [b[0] + r, b[1] + s, b[2] - f, b[3] - s]),
        ];
        for (yaw, want) in expected {
            let got = buildable_core(b, insets, yaw).unwrap_or_else(|| {
                panic!("an asymmetric inset set must yield a core at yaw {yaw}")
            });
            assert_eq!(
                got, want,
                "the buildable core at yaw {yaw} must rotate with the building, not stay on world Z"
            );
            // the core must be strictly inside the parcel on every side
            assert!(got[0] >= b[0] && got[1] >= b[1] && got[2] <= b[2] && got[3] <= b[3]);
        }
        // the front and rear insets must be distinguishable: asymmetric insets must NOT give a
        // symmetric core, which is precisely what the old world-Z-only version produced at 90/270
        let core90 = buildable_core(b, insets, 90.).unwrap();
        let mirrored = [b[0] + r, b[1] + s, b[2] - f, b[3] - s];
        assert_ne!(
            core90, mirrored,
            "front and rear must not be interchangeable at yaw 90"
        );
        // a non-cardinal rotation has no defined local forward and must be refused, not guessed
        for yaw in [45., 135., 1., -90.] {
            assert!(
                buildable_core(b, insets, yaw).is_none(),
                "a non-cardinal rotation ({yaw}) must not be treated as having a forward axis"
            );
        }
        // a parcel too small to hold the insets plus a skin must be refused rather than inverted
        assert!(buildable_core([0, 0, 10, 10], insets, 0.).is_none());
        // LEGACY REGRESSION: the symmetric [2,2,2] inset set must behave identically in all four
        // directions, because with equal insets the rotation cannot matter.
        let sym = ParcelInsets { front_m: 2, side_m: 2, rear_m: 2 };
        let first = buildable_core(b, sym, 0.).unwrap();
        assert_eq!(first, [b[0] + 3, b[1] + 3, b[2] - 3, b[3] - 3]);
        for yaw in [0., 90., 180., 270.] {
            assert_eq!(
                buildable_core(b, sym, yaw).unwrap(),
                first,
                "a symmetric inset set must give the same core at every rotation, got a difference at {yaw}"
            );
        }
    }
    #[test]
    fn nonzero_origin_covariance_and_unknown_inputs() {
        let mut results = vec![];
        for origin in [0, -512] {
            let (s, n, w, z, mut p, style) = fixture(origin);
            append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
            results.push(
                p.roadside_sites
                    .iter()
                    .map(|a| (a.world_x - origin, a.world_z - origin, a.support_level_mm))
                    .collect::<Vec<_>>(),
            );
        }
        assert!(!results[0].is_empty());
        assert_eq!(results[0], results[1]);
        let mut style = fixture(0).5;
        style["frontageProgram"]["requests"][0]["worldX"] = serde_json::json!(100);
        assert!(from_style(&style).is_err());
    }
    #[test]
    fn invalid_source_does_not_partially_modify_network() {
        let (s, _n, w, z, mut p, style) = fixture(0);
        let n = BakedTerrain::new(&s).unwrap();
        let old = serde_json::to_value(&p).unwrap();
        assert!(append(&mut p, &s, &n, w, Some(&z), &style).is_err());
        assert_eq!(serde_json::to_value(p).unwrap(), old);
    }
    #[test]
    fn explicit_connection_suppresses_old_blended_ridge_but_never_cuts_nature() {
        let (s, n, w, z, mut p, style) = fixture(0);
        append(&mut p, &s, &n, w, Some(&z), &style).unwrap();
        let e = p.roadside_sites.last().unwrap().entity();
        let [rx, rz] = e.grounding.frontage_road_point.unwrap();
        let a = &e.anchors[0];
        let x = rx + (a.world_x - rx).signum() * 8;
        let zz = rz + (a.world_z - rz).signum() * 8;
        let ridge = super::super::TerrainCarve {
            profile: None,
            cx: f64::from(x) + 8.,
            cz: f64::from(zz),
            half_w: 1.,
            half_d: 1.,
            target_h_m: 10.,
            blend_m: 50.,
            priority: 1,
            disturbance: 160,
        };
        let mut before = e.clone();
        before.grounding.frontage_road_point = None;
        let mut old = super::super::plan_terrain_carves(&[before]);
        old.push(ridge.clone());
        let mut new = super::super::plan_terrain_carves(&[e.clone()]);
        new.push(ridge);
        // The real road activates hard-support ownership and occupies the other 7m.
        for c in super::super::plan_terrain_carves(&p.entities()) {
            old.push(c.clone());
            new.push(c);
        }
        let base = f64::from(n.height(x, zz).unwrap());
        let target = f64::from(e.ground_height_m());
        assert!(super::super::apply_terrain_carves(base, x, zz, &old) > target + 1.);
        assert!((super::super::apply_terrain_carves(base, x, zz, &new) - target).abs() < 0.0001);
        assert!(super::super::apply_terrain_carves(target + 0.1, x, zz, &new) >= target + 0.1);
    }
    #[test]
    fn manifest_asset_contract_is_checked_before_placement() {
        let mut m: WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/local-height-frontage.manifest.json"
        ))
        .unwrap();
        assert!(validate_manifest(&m).is_ok());
        m.style["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(18);
        assert!(
            validate_manifest(&m)
                .unwrap_err()
                .to_string()
                .contains("asset contract")
        );
        m.style["frontageProgram"]["requests"][0]["depthM"] = serde_json::json!(24);
        m.style["frontageProgram"]["requests"][0]["sceneId"] = serde_json::json!("missing");
        assert!(validate_manifest(&m).is_err());
    }
    #[test]
    fn negative_millimetre_frontage_is_not_raised_by_legacy_quarter_metre_rebasing() {
        // Actual first failed r48 maximum-world site; no full work/ fixture dependency.
        let m: WorldManifest = serde_json::from_str(include_str!(
            "../../../../../examples/maximum-local-height-frontage.manifest.json"
        ))
        .unwrap();
        let scene = &m.scenes[0];
        let water = super::super::water_geometry(
            super::super::water_kind(&m.style),
            &m.landmarks,
            scene,
            &m.style,
        );
        let site:RoadsideSite=serde_json::from_value(serde_json::json!({"program":{"requestId":"urban-homes","kind":"residential_home","heightM":8},"id":"generated.frontage.regression","roadId":"road","worldX":3788,"worldZ":4920,"widthM":24,"depthM":24,"supportLevelM":-2,"supportLevelMm":-1354,"frontageRoadPoint":[3788,4896],"rotationYDeg":0,"naturalMinM":-6.666,"naturalMaxM":-3.232})).unwrap();
        let e = site.entity();
        let expected = serde_json::to_value(&e).unwrap();
        let mut entities = vec![e];
        let carves = super::super::plan_terrain_carves(&entities);
        super::super::rebase_grounded_entities_to_carved_surface(
            m.world.seed,
            water,
            &mut entities,
            &carves,
        );
        assert_eq!(
            serde_json::to_value(&entities[0]).unwrap(),
            expected,
            "a precise planned platform must not pass through quarter-metre rounding/ceil"
        );
        let local = SceneSpec {
            scene_id: "regression".into(),
            width_m: 32,
            depth_m: 32,
            origin_x: 3772,
            origin_z: 4904,
            seed_offset: 0,
        };
        for mm in [-1354, -1353] {
            let mut t = BakedTerrain::new(&local).unwrap();
            t.insert_chunk(
                local.origin_x,
                local.origin_z,
                32,
                32,
                &encode_millimetres(32, 32, vec![f64::from(mm) / 1000.; 32 * 32]).unwrap(),
            )
            .unwrap();
            super::super::rebase_entities_to_baked_surface(&mut entities, &t);
            assert_eq!(
                serde_json::to_value(&entities[0]).unwrap(),
                expected,
                "even wrong terrain must be audited, not silently heal the planned entity"
            );
        }
    }

    /// A 512 m scene cut into one urban block and one rural block by a single
    /// horizontal boundary at z = 256. Every pad parcel below is placed around it, so one
    /// number decides whether its core is urban, wholly rural or neither.
    fn banded_zones(scene: &SceneSpec, north: &str) -> Vec<LandUseZone> {
        vec![
            LandUseZone {
                id: "north".into(),
                kind: north.into(),
                world_x: scene.origin_x,
                world_z: scene.origin_z,
                width_m: scene.width_m,
                depth_m: 256,
                source: "test-north".into(),
            },
            LandUseZone {
                id: "south".into(),
                kind: "rural".into(),
                world_x: scene.origin_x,
                world_z: scene.origin_z + 256,
                width_m: scene.width_m,
                depth_m: 256,
                source: "test-south".into(),
            },
        ]
    }

    fn pad_scene() -> SceneSpec {
        SceneSpec {
            scene_id: "pad-purpose".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    /// The pad programme's whole purpose contract, measured on the parcel itself: an explicit
    /// switch is the ONLY thing that makes a pad declare a purpose, the declaration reuses the
    /// frontage envelope verdict, and every land that is not urban or wholly rural is refused.
    #[test]
    fn a_pad_declares_its_purpose_only_under_the_explicit_switch_and_converts_only_rural_core() {
        let s = pad_scene();
        // A 24x24 pad on a zero-inset parcel: a 22x22 = 484 m2 buildable core.
        let insets = ParcelInsets { front_m: 0, side_m: 0, rear_m: 0 };
        let parcel_at = |z: i64| {
            let z = i64::from(0) + z;
            // x 200..224, z z0..z0+24 in world metres
            [200, z, 224, z + 24]
        };
        // OFF is the identity: the pad declares nothing, is never refused and never converts, even
        // when it stands on protected land. That is what makes the switch byte-identical.
        for kind in ["residential_home", "school", "convenience_store", "unknown_kind"] {
            for north in ["urban", "rural", "nature", "unassigned"] {
                let zones = banded_zones(&s, north);
                assert_eq!(
                    pad_purpose_of(
                        parcel_at(300),
                        insets,
                        0.,
                        &s,
                        &zones,
                        kind,
                        RuralConversion::Disabled,
                    ),
                    PadPurpose::OFF,
                    "kind {kind} on {north} land must be untouched with the switch off"
                );
            }
        }
        let rural = banded_zones(&s, "urban");
        // ON, wholly rural core (z 280..304 stands in the rural half): the pad declares urban and
        // converts exactly its buildable core.
        let verdict = pad_purpose_of(
            parcel_at(280),
            insets,
            0.,
            &s,
            &rural,
            "residential_home",
            RuralConversion::RuralToUrban,
        );
        assert_eq!(verdict.declared, Some("urban"));
        assert_eq!(verdict.reason, None);
        assert_eq!(verdict.converted_m2, Some(484));
        // ON, urban core: declared, placed directly, nothing converted.
        let verdict = pad_purpose_of(
            parcel_at(100),
            insets,
            0.,
            &s,
            &rural,
            "residential_home",
            RuralConversion::RuralToUrban,
        );
        assert_eq!((verdict.declared, verdict.reason, verdict.converted_m2), (Some("urban"), None, None));
        // ON, a core that is MOSTLY urban lands directly: the pad reuses the frontage programme's
        // bounded-majority rule, so a core that only clips the rural edge is still urban land. No
        // conversion is claimed for it, which is what keeps the two programmes on one rule.
        let verdict = pad_purpose_of(
            parcel_at(236),
            insets,
            0.,
            &s,
            &rural,
            "residential_home",
            RuralConversion::RuralToUrban,
        );
        assert_eq!(
            (verdict.declared, verdict.reason, verdict.converted_m2),
            (Some("urban"), None, None),
            "a core that is 17 of its 22 rows in urban land is urban land"
        );
        // ON, a core that is neither mostly urban nor wholly rural is refused under the existing
        // purpose/protection key rather than converted, because a conversion may only take land whose
        // buildable core is WHOLLY rural.
        let verdict = pad_purpose_of(
            parcel_at(244),
            insets,
            0.,
            &s,
            &rural,
            "residential_home",
            RuralConversion::RuralToUrban,
        );
        assert_eq!(verdict.reason, Some("wrong_purpose_or_protected"));
        assert_eq!(verdict.converted_m2, None);
        // ON, protected land: a pad is never given a purpose there (the pad's own zoning and water
        // gates refuse it first; this is the second, independent refusal).
        for north in ["nature", "unassigned"] {
            let zones = banded_zones(&s, north);
            // the pad stands INSIDE the protected half here, so its core is that land
            let verdict = pad_purpose_of(
                parcel_at(100),
                insets,
                0.,
                &s,
                &zones,
                "residential_home",
                RuralConversion::RuralToUrban,
            );
            assert_eq!(
                verdict.reason,
                Some("wrong_purpose_or_protected"),
                "{north} land must not be converted"
            );
            assert_eq!(verdict.converted_m2, None);
        }
        // A parcel whose RECTANGLE reaches one protected square metre earns nothing, even when its
        // buildable core is wholly rural: the conversion decides the core, never what the parcel may
        // own. The 1 m nature column runs through the whole scene so the west/east bands keep tiling.
        let protected_column = vec![
            LandUseZone {
                id: "north-west".into(),
                kind: "urban".into(),
                world_x: 0,
                world_z: 0,
                width_m: 223,
                depth_m: 256,
                source: "test".into(),
            },
            LandUseZone {
                id: "column".into(),
                kind: "nature".into(),
                world_x: 223,
                world_z: 0,
                width_m: 1,
                depth_m: 512,
                source: "test".into(),
            },
            LandUseZone {
                id: "north-east".into(),
                kind: "urban".into(),
                world_x: 224,
                world_z: 0,
                width_m: 288,
                depth_m: 256,
                source: "test".into(),
            },
            LandUseZone {
                id: "south-west".into(),
                kind: "rural".into(),
                world_x: 0,
                world_z: 256,
                width_m: 223,
                depth_m: 256,
                source: "test".into(),
            },
            LandUseZone {
                id: "south-east".into(),
                kind: "rural".into(),
                world_x: 224,
                world_z: 256,
                width_m: 288,
                depth_m: 256,
                source: "test".into(),
            },
        ];
        assert_eq!(
            protected_column
                .iter()
                .map(|zone| i64::from(zone.width_m) * i64::from(zone.depth_m))
                .sum::<i64>(),
            512 * 512,
            "the protected fixture must tile the scene"
        );
        let claimed = parcel_at(280);
        let core = buildable_core(claimed, insets, 0.).unwrap();
        assert!(
            core_entirely_in(core, &protected_column, "rural"),
            "the buildable core must be wholly rural, so only the parcel rule can refuse it"
        );
        let verdict = pad_purpose_of(
            claimed,
            insets,
            0.,
            &s,
            &protected_column,
            "residential_home",
            RuralConversion::RuralToUrban,
        );
        assert_eq!(verdict.reason, Some("wrong_purpose_or_protected"));
        assert_eq!(verdict.converted_m2, None);
        // A kind the settlement programme does not know cannot declare a purpose at all, and that is
        // the ONE new named refusal this round adds.
        let verdict = pad_purpose_of(
            parcel_at(280),
            insets,
            0.,
            &s,
            &rural,
            "turnip_cart",
            RuralConversion::RuralToUrban,
        );
        assert_eq!(verdict, PadPurpose { declared: None, reason: Some("pad_purpose_undeclared"), converted_m2: None });
        // The scene-free form the AUDIT uses answers the same area for the same parcel.
        let site = RoadsideSite {
            program: None,
            purpose: Some("urban".into()),
            id: "generated.site".into(),
            road_id: "road".into(),
            world_x: 212,
            world_z: 292,
            width_m: 24,
            depth_m: 24,
            support_level_m: 0,
            parcel_insets: Some(insets),
            support_level_mm: None,
            frontage_road_point: None,
            rotation_y_deg: 0.,
            natural_min_m: 0.,
            natural_max_m: 0.,
        };
        assert_eq!(pad_conversion_area(&site, &rural), Some(484));
        let urban_site = RoadsideSite { world_z: 112, ..site.clone() };
        assert_eq!(pad_conversion_area(&urban_site, &rural), None);
        assert_eq!(
            pad_conversion_area(&site, &protected_column),
            None,
            "a parcel with any protected square metre converts nothing"
        );
    }
}
