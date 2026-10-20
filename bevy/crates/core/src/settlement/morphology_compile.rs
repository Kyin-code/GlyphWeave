//! Deterministic compilation of a morphology profile into the *effective*
//! programs that the Rust settlement pipeline actually consumes.
//!
//! A profile is an engineering parameter preset. It is never a second
//! placement algorithm, never a demand source and never a renderer-side
//! country rule. Compilation therefore is:
//!
//! * **deterministic** - no hash-map iteration, clock or randomness; the same
//!   style/scene input always yields byte-identical effective programs;
//! * **additive** - the original manifest keeps every explicit value and the
//!   profile only supplies a default where the input stated nothing;
//! * **fail-closed** - a default that the downstream consumer contract
//!   rejects is a compile error naming the profile field, never a silent
//!   clamp, resize or shrink;
//! * **explicit about coverage** - every profile field is reported as
//!   `original-input`, `profile-default`, `derived` or `unsupported`.
//!
//! Range-to-scalar rule (the only place a profile range becomes a number):
//! `midpoint(range)`, floored to the previous even integer for metre
//! quantities, rounded to three decimals for ratios. Derived numbers are
//! engineering parameters, not claims about a national building code.
//!
//! Quantity semantics, kept explicit because they are different objects:
//!
//! * `parcelWidthM`/`parcelDepthM` describe the **complete parcel** including
//!   its yards. They are never written into a building footprint. The default
//!   footprint is `parcel - insets` (checked arithmetic, positive even metres)
//!   and must still pass the building consumer and asset contracts.
//! * `frontInsetM`/`sideInsetM`/`rearInsetM` are the parcel-to-footprint
//!   insets that separate the two.
//! * `frontInsetM` also bounds the **street setback**. A profile parcel's front
//!   yard runs from the building to the parcel's front boundary, so when a
//!   request states no `setbackM` the compiler derives
//!   `frontInset + frontage::ROAD_PROTECTION_M`. That is the only setback at
//!   which the parcel's front boundary sits exactly on the road's protected
//!   line: the frontage preflight rejects any parcel that enters the band, so a
//!   smaller derivation would be a default that can never place a building. A
//!   stated setback is never overridden, and a derivation outside the frontage
//!   request band is refused instead of clamped.
//! * `publicSpaceRatio` is a public-space share. It is not a pasture share:
//!   it fills `urbanOpenSpaceRatio` for the scenes that own an urban remainder,
//!   and it never fills `ruralPastureRatio`. A program that states only the
//!   public-space share is complete because the residual-land consumer's named
//!   default for an unstated ratio is zero; the compile report records that
//!   default per scene instead of the compiler inventing a pasture number.
use super::morphology::{self, MorphologyProfile};
use crate::worldgen::{LandUseProfile, SceneSpec, plan_landuse_zones};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Version of the published effective-program document.
pub const EFFECTIVE_PROGRAMS_VERSION: u32 = 1;
/// Revision label recorded next to the effective programs. Bumped because the
/// compiled program set changed: a profile with `publicSpaceRatio` now also
/// compiles `residualLandProgram.scenes[*].urbanOpenSpaceRatio`, so the same
/// manifest produces a different effective document (and world identity) than
/// it did under v4. Old documents are rejected, never reinterpreted.
pub const EFFECTIVE_PROGRAMS_REVISION: &str = "b04-c1-r2c-effective-programs-v5";
/// The value came from the manifest the user wrote.
pub const SOURCE_ORIGINAL: &str = "original-input";
/// The value came from a profile field the user did not override.
pub const SOURCE_PROFILE: &str = "profile-default";
/// The value came from a deterministic table in this module.
pub const SOURCE_DERIVED: &str = "derived";
/// The profile field has no consumer in the current pipeline.
pub const SOURCE_UNSUPPORTED: &str = "unsupported";

/// Road tier widths used when a profile supplies a hierarchy. The profile
/// only names roles, so the width is a derived engineering value, not a
/// measured fact about the source cases.
fn role_width_m(role: &str) -> u32 {
    match role {
        "collector" => 16,
        "local" => 12,
        "farm_access" => 8,
        _ => 8,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldSource {
    /// JSON path inside the effective style, e.g.
    /// `frontageProgram.requests[0].parcelInsets.frontM`.
    pub path: String,
    /// One of `original-input`, `profile-default`, `derived`, `unsupported`.
    pub source: String,
    /// Profile field that produced the value, when there is one.
    pub profile_field: String,
    /// Human-readable, deterministic explanation.
    pub detail: String,
}

impl FieldSource {
    fn new(path: impl Into<String>, source: &str, profile_field: &str, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
            profile_field: profile_field.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectiveReport {
    pub format: String,
    pub version: u32,
    pub revision: String,
    pub status: String,
    pub profile_id: String,
    pub profile_version: u32,
    pub source_case_ids: Vec<String>,
    pub evidence_level: String,
    pub scenes: Vec<String>,
    pub manifest_style_hash: String,
    pub effective_style_hash: String,
    pub field_sources: Vec<FieldSource>,
    pub unsupported: Vec<FieldSource>,
    pub notes: Vec<String>,
    /// The full effective style. Publishing it lets an independent audit
    /// recompile from the original manifest and compare byte for byte.
    pub effective_style: Value,
}

#[derive(Debug, Clone)]
pub struct CompiledStyle {
    pub style: Value,
    pub report: EffectiveReport,
}

fn err(message: impl Into<String>) -> String {
    message.into()
}

fn hash(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| err(error.to_string()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// Midpoint of a profile metre range, floored to the previous even integer.
///
/// Shared with the shorefront row (`worldgen::frontage::shorefront_row_contract`), which has to
/// derive the SAME street setback and parcel insets for a world that states only a morphology
/// profile: a row placed on a second derivation could disagree with the authored parcels about the
/// street it fronts.
pub(crate) fn even_midpoint(range: [u32; 2], field: &str) -> Result<u32, String> {
    if range[0] > range[1] {
        return Err(err(format!("profile field {field} has an inverted range")));
    }
    let mid = (range[0] + range[1]) / 2;
    let even = mid - (mid % 2);
    if even < range[0] {
        return Err(err(format!(
            "profile field {field} range {range:?} has no even metre value at or below its midpoint"
        )));
    }
    Ok(even)
}

/// Midpoint of a profile ratio range, rounded to three decimals.
fn ratio_midpoint(range: [f64; 2], field: &str) -> Result<f64, String> {
    if !range.iter().all(|value| value.is_finite()) || range[0] > range[1] {
        return Err(err(format!("profile field {field} has an invalid ratio range")));
    }
    Ok(((range[0] + range[1]) * 0.5 * 1000.).round() / 1000.)
}

fn as_object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| err(format!("{path} must be a JSON object")))
}

fn as_array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| err(format!("{path} must be a JSON array")))
}

fn require_integer(object: &Map<String, Value>, field: &str, path: &str) -> Result<u64, String> {
    let value = object
        .get(field)
        .ok_or_else(|| err(format!("{path}.{field} must be stated explicitly")))?;
    value
        .as_u64()
        .ok_or_else(|| err(format!("{path}.{field} must be a non-negative integer")))
}

/// The profile's complete parcel box: parcel dimensions plus the insets that
/// separate the building footprint from the parcel edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParcelPolicy {
    width_m: u32,
    depth_m: u32,
    front_m: u32,
    side_m: u32,
    rear_m: u32,
}

impl ParcelPolicy {
    fn from_profile(profile: &MorphologyProfile) -> Result<Self, String> {
        Ok(Self {
            width_m: even_midpoint(profile.parcel_width_m, "parcelWidthM")?,
            depth_m: even_midpoint(profile.parcel_depth_m, "parcelDepthM")?,
            front_m: even_midpoint(profile.front_inset_m, "frontInsetM")?,
            side_m: even_midpoint(profile.side_inset_m, "sideInsetM")?,
            rear_m: even_midpoint(profile.rear_inset_m, "rearInsetM")?,
        })
    }

    fn insets(&self) -> Value {
        json!({"frontM": self.front_m, "sideM": self.side_m, "rearM": self.rear_m})
    }

    /// Footprint width implied by the complete parcel: the parcel minus both
    /// side insets. A non-positive or odd result is a refusal, never a clamp.
    /// Each axis is derived separately so an explicit axis is never blocked by
    /// a default it does not use.
    fn footprint_width(&self) -> Result<u32, String> {
        let width = self
            .width_m
            .checked_sub(
                self.side_m
                    .checked_mul(2)
                    .ok_or_else(|| err("profile parcel side insets overflow"))?,
            )
            .ok_or_else(|| {
                err(format!(
                    "profile parcel box {}m wide cannot hold 2x{}m side insets",
                    self.width_m, self.side_m
                ))
            })?;
        if width == 0 || width % 2 != 0 {
            return Err(err(format!(
                "profile parcel box implies a degenerate building width {width}m"
            )));
        }
        Ok(width)
    }

    /// Footprint depth implied by the complete parcel: the parcel minus the
    /// front and rear insets.
    fn footprint_depth(&self) -> Result<u32, String> {
        let depth = self
            .depth_m
            .checked_sub(self.front_m)
            .and_then(|value| value.checked_sub(self.rear_m))
            .ok_or_else(|| {
                err(format!(
                    "profile parcel box {}m deep cannot hold {}m front + {}m rear insets",
                    self.depth_m, self.front_m, self.rear_m
                ))
            })?;
        if depth == 0 || depth % 2 != 0 {
            return Err(err(format!(
                "profile parcel box implies a degenerate building depth {depth}m"
            )));
        }
        Ok(depth)
    }

    /// The full parcel implied by an explicit building footprint plus this
    /// policy's insets, or `None` on overflow.
    fn parcel_of(&self, width_m: u32, depth_m: u32) -> Option<(u32, u32)> {
        let width = width_m.checked_add(self.side_m.checked_mul(2)?)?;
        let depth = depth_m
            .checked_add(self.front_m)?
            .checked_add(self.rear_m)?;
        Some((width, depth))
    }
}

/// Fill the frontage program. Demand identity and demand volume are never
/// inferred; only unstated morphology parameters are defaulted.
fn compile_frontage(
    style: &Value,
    effective: &mut Value,
    profile: &MorphologyProfile,
    sources: &mut Vec<FieldSource>,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let Some(program) = style.get("frontageProgram") else {
        return Ok(());
    };
    let program_path = "frontageProgram";
    let object = as_object(program, program_path)?.clone();
    let version = require_integer(&object, "version", program_path)?;
    if version != 1 {
        return Err(err(format!("{program_path}.version must be 1")));
    }
    let requests = as_array(
        object
            .get("requests")
            .ok_or_else(|| err(format!("{program_path}.requests must be stated explicitly")))?,
        &format!("{program_path}.requests"),
    )?
    .clone();

    let policy = ParcelPolicy::from_profile(profile)?;
    let height_default = even_midpoint(profile.height_m, "heightM")?;
    // The single number the frontage preflight uses to protect a road corridor.
    // Reading it from the consumer (instead of repeating 4 here) is what keeps a
    // derived setback and the rejection rule from ever disagreeing.
    let road_protection_m = u32::try_from(crate::worldgen::frontage::ROAD_PROTECTION_M)
        .map_err(|_| err("frontage road-protection band is not a metre quantity"))?;

    let mut compiled = Vec::with_capacity(requests.len());
    for (index, request) in requests.iter().enumerate() {
        let path = format!("{program_path}.requests[{index}]");
        let request = as_object(request, &path)?;
        // Keep every authored key, including unknown ones: the strict reader
        // below rejects typos instead of letting a default hide them.
        let mut item = request.clone();
        for field in ["id", "sceneId", "purpose", "kind"] {
            if !item.contains_key(field) {
                return Err(err(format!(
                    "{path}.{field} must be stated explicitly; a morphology profile never invents demand"
                )));
            }
        }
        match item.get("count") {
            Some(value) if value.as_u64().is_some_and(|count| count > 0) => {}
            Some(_) => {
                return Err(err(format!(
                    "{path}.count must be a positive integer demand"
                )));
            }
            None => {
                return Err(err(format!(
                    "{path}.count is required; a morphology profile never allocates buildings"
                )));
            }
        }
        // Resolve this request's *effective* parcel insets first: an explicit
        // object is parsed strictly, otherwise the profile supplies it. The
        // default footprint must be derived from these values, not from the
        // profile defaults, or an explicit inset would silently keep a
        // footprint computed for a different parcel.
        let authored_insets = item.contains_key("parcelInsets");
        let (inset_front, inset_side, inset_rear) = if authored_insets {
            let insets = item
                .get("parcelInsets")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    err(format!(
                        "{path}.parcelInsets must be an object with frontM/sideM/rearM"
                    ))
                })?;
            let axis = |key: &str| -> Result<u32, String> {
                insets
                    .get(key)
                    .and_then(Value::as_u64)
                    .filter(|value| *value <= u64::from(u32::MAX))
                    .map(|value| value as u32)
                    .ok_or_else(|| {
                        err(format!(
                            "{path}.parcelInsets.{key} must be a non-negative integer"
                        ))
                    })
            };
            (axis("frontM")?, axis("sideM")?, axis("rearM")?)
        } else {
            (policy.front_m, policy.side_m, policy.rear_m)
        };
        let inset_origin = if authored_insets {
            "authored"
        } else {
            "profile"
        };
        let effective_policy = ParcelPolicy {
            width_m: policy.width_m,
            depth_m: policy.depth_m,
            front_m: inset_front,
            side_m: inset_side,
            rear_m: inset_rear,
        };
        // Building footprint: this request's effective insets supply the
        // default only where that axis said nothing, and only that axis is
        // derived. A stated dimension is never blocked by a default it does not
        // use: an explicitly authored footprint that leaves the profile's
        // heuristic band is kept and reported, not refused.
        for (field, profile_field) in [
            ("widthM", "parcelWidthM"),
            ("depthM", "parcelDepthM"),
            ("heightM", "heightM"),
        ] {
            let field_path = format!("{path}.{field}");
            if item.contains_key(field) {
                sources.push(FieldSource::new(
                    field_path,
                    SOURCE_ORIGINAL,
                    profile_field,
                    "explicit request value; the profile never overrides or resizes it",
                ));
                continue;
            }
            let (value, derivation) = match field {
                "widthM" => {
                    let width = effective_policy.footprint_width()?;
                    (
                        width,
                        format!(
                            "complete parcel {}m - 2x{inset_side}m {inset_origin} side inset = {width}m footprint",
                            policy.width_m
                        ),
                    )
                }
                "depthM" => {
                    let depth = effective_policy.footprint_depth()?;
                    (
                        depth,
                        format!(
                            "complete parcel {}m - {inset_front}m {inset_origin} front - {inset_rear}m {inset_origin} rear = {depth}m footprint",
                            policy.depth_m
                        ),
                    )
                }
                _ => (
                    height_default,
                    format!("profile height midpoint {height_default}m"),
                ),
            };
            item.insert(field.into(), json!(value));
            sources.push(FieldSource::new(
                field_path,
                SOURCE_PROFILE,
                profile_field,
                format!("unstated; {derivation}"),
            ));
        }
        // Search budgets are engineering inputs of the request, not morphology
        // of the source cases, and the profile has no mapping for them. The one
        // exception is the street setback: when it is unstated it is derived
        // below. An AUTHORED setback keeps this exact entry, in this exact
        // position, so a request that already states its setback compiles to the
        // same effective bytes, field sources and world revision as before.
        for field in ["stationSpacingM", "setbackM", "maxCandidateAttempts"] {
            if item.contains_key(field) {
                sources.push(FieldSource::new(
                    format!("{path}.{field}"),
                    SOURCE_ORIGINAL,
                    "",
                    "request search/engineering parameter outside the profile's morphology scope",
                ));
                continue;
            }
            if field != "setbackM" {
                return Err(err(format!(
                    "{path}.{field} must be stated explicitly; the morphology profile has no consumer mapping for it"
                )));
            }
            // The street setback is morphology, and it is the only field derived
            // from more than one quantity: the front yard runs from the building
            // to the parcel's front boundary, and that boundary may not enter the
            // road protection band. `frontInset + band` is therefore the one
            // value that puts the parcel's front boundary exactly on the
            // protected road line. The request's own effective front inset is
            // used, so an authored inset policy is honoured, and an impossible
            // derivation is refused instead of clamped.
            let setback = inset_front
                .checked_add(road_protection_m)
                .ok_or_else(|| err(format!("{path}.setbackM: front inset overflow")))?;
            if !(8..=32).contains(&setback) || setback % 2 != 0 {
                return Err(err(format!(
                    "{path}: the {inset_origin} front inset of {inset_front}m plus the {road_protection_m}m frontage road-protection band derives a {setback}m street setback from frontInsetM, outside the frontage request band 8..=32 even metres; state setbackM explicitly"
                )));
            }
            item.insert("setbackM".into(), json!(setback));
            sources.push(FieldSource::new(
                format!("{path}.setbackM"),
                SOURCE_DERIVED,
                "frontInsetM",
                format!(
                    "unstated; {inset_origin} front inset {inset_front}m + the frontage road-protection band {road_protection_m}m = {setback}m street setback, placing the parcel front boundary exactly on the protected road line"
                ),
            ));
        }
        if authored_insets {
            sources.push(FieldSource::new(
                format!("{path}.parcelInsets"),
                SOURCE_ORIGINAL,
                "frontInsetM/sideInsetM/rearInsetM",
                "explicit parcel insets; the profile never overrides them and the default footprint uses them",
            ));
        } else {
            item.insert("parcelInsets".into(), policy.insets());
            for (key, value, profile_field, declared) in [
                ("frontM", policy.front_m, "frontInsetM", profile.front_inset_m),
                ("sideM", policy.side_m, "sideInsetM", profile.side_inset_m),
                ("rearM", policy.rear_m, "rearInsetM", profile.rear_inset_m),
            ] {
                sources.push(FieldSource::new(
                    format!("{path}.parcelInsets.{key}"),
                    SOURCE_PROFILE,
                    profile_field,
                    format!(
                        "unstated; profile {} range {declared:?} midpoint {value}m",
                        profile_field
                    ),
                ));
            }
        }
        // Report an explicit footprint that leaves the profile's parcel band.
        // The explicit value is kept; only the deviation is recorded, and the
        // insets it was combined with are named honestly.
        if request.contains_key("widthM") || request.contains_key("depthM") {
            let built = (
                item.get("widthM").and_then(Value::as_u64).map(|v| v as u32),
                item.get("depthM").and_then(Value::as_u64).map(|v| v as u32),
            );
            if let (Some(width), Some(depth)) = built {
                if let Some((parcel_width, parcel_depth)) =
                    effective_policy.parcel_of(width, depth)
                {
                    for (field, parcel, declared) in [
                        ("parcelWidthM", parcel_width, profile.parcel_width_m),
                        ("parcelDepthM", parcel_depth, profile.parcel_depth_m),
                    ] {
                        if parcel < declared[0] || parcel > declared[1] {
                            notes.push(format!(
                                "{path}: explicit {width}x{depth}m building plus the {inset_origin} insets gives a {parcel_width}x{parcel_depth}m parcel, outside the profile {field} band {declared:?}; the explicit value is kept unchanged"
                            ));
                        }
                    }
                }
            }
        }
        compiled.push(Value::Object(item));
    }
    // Keep the authored container verbatim except for `requests`, so unknown
    // or misnamed keys still reach the strict reader and are rejected instead
    // of being silently dropped by the compiler.
    let mut replacement = object;
    replacement.insert("requests".into(), Value::Array(compiled));
    let style_object = effective
        .as_object_mut()
        .ok_or_else(|| err("style must be a JSON object"))?;
    style_object.insert(program_path.into(), Value::Object(replacement));
    crate::worldgen::frontage::from_style(effective)
        .map_err(|error| {
            let derived: Vec<&str> = sources
                .iter()
                .filter(|source| {
                    source.source == SOURCE_PROFILE && source.path.starts_with(program_path)
                })
                .map(|source| source.path.as_str())
                .collect();
            err(format!(
                "effective frontageProgram is rejected by its contract: {error} (profile-derived fields: {})",
                if derived.is_empty() {
                    "none".to_owned()
                } else {
                    derived.join(", ")
                }
            ))
        })?;
    Ok(())
}

/// Build the profile's road hierarchy. Roles come from the profile, widths
/// come from the documented role table in this module.
fn hierarchy_from_profile(profile: &MorphologyProfile) -> Value {
    let tiers: Vec<Value> = profile
        .road_hierarchy
        .iter()
        .map(|role| {
            json!({
                "id": format!("profile-{role}"),
                "role": role,
                "widthM": role_width_m(role),
                "purposes": ["urban", "rural"],
            })
        })
        .collect();
    json!({"tiers": tiers})
}

/// Attach the profile road hierarchy to scenes that did not state one.
fn compile_auto_roads(
    style: &Value,
    effective: &mut Value,
    profile: &MorphologyProfile,
    sources: &mut Vec<FieldSource>,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let Some(program) = style.get("automaticRoadExtensions") else {
        notes.push(
            "profile roadHierarchy needs an automaticRoadExtensions program; none was declared, so no road hierarchy was injected"
                .into(),
        );
        return Ok(());
    };
    let path = "automaticRoadExtensions";
    let object = as_object(program, path)?.clone();
    let version = require_integer(&object, "version", path)?;
    if !matches!(version, 1 | 2 | 3) {
        return Err(err(format!("{path}.version must be 1, 2 or 3")));
    }
    let scenes = object
        .get("scenes")
        .ok_or_else(|| err(format!("{path}.scenes must be stated explicitly")))?;
    let mut scenes = as_array(scenes, &format!("{path}.scenes"))?.clone();
    for (index, scene) in scenes.iter_mut().enumerate() {
        let scene_path = format!("{path}.scenes[{index}]");
        let scene_object = scene
            .as_object_mut()
            .ok_or_else(|| err(format!("{scene_path} must be a JSON object")))?;
        if scene_object.contains_key("hierarchy") {
            sources.push(FieldSource::new(
                format!("{scene_path}.hierarchy"),
                SOURCE_ORIGINAL,
                "roadHierarchy",
                "explicit hierarchy; the profile never overrides it",
            ));
            continue;
        }
        scene_object.insert("hierarchy".into(), hierarchy_from_profile(profile));
        sources.push(FieldSource::new(
            format!("{scene_path}.hierarchy"),
            SOURCE_PROFILE,
            "roadHierarchy",
            format!(
                "unstated; profile roles {} became tiers",
                profile.road_hierarchy.join(",")
            ),
        ));
        for role in &profile.road_hierarchy {
            sources.push(FieldSource::new(
                format!("{scene_path}.hierarchy.tiers[role={role}].widthM"),
                SOURCE_DERIVED,
                "roadHierarchy",
                format!("role width table: {role} -> {}m", role_width_m(role)),
            ));
        }
    }
    let mut replacement = object;
    // The profile only ever *adds* the hierarchy, so the effective version is
    // the authored one or 2 when a hierarchy was injected. An authored
    // demand-led version 3 is preserved, never silently downgraded.
    replacement.insert("version".into(), json!(version.max(2)));
    replacement.insert("scenes".into(), Value::Array(scenes));
    effective
        .as_object_mut()
        .ok_or_else(|| err("style must be a JSON object"))?
        .insert(path.into(), Value::Object(replacement));
    crate::worldgen::auto_roads::from_style(effective)
        .map_err(|error| err(format!("effective automaticRoadExtensions is rejected: {error}")))?;
    Ok(())
}

/// Attach the profile public-space share to the residual-land consumer.
///
/// `publicSpaceRatio` is a public-space share and nothing else. It fills
/// `urbanOpenSpaceRatio` for every scene that can own urban remainder; it never
/// fills `ruralPastureRatio`, which is a different quantity with no profile
/// source. The two are independent statements, so a program that states only
/// the public-space share is *complete*: an unstated ratio resolves to
/// `residual_land::ABSENT_RATIO_DEFAULT` (zero) in the consumer, and the compile
/// report names that default per scene instead of inventing a pasture number.
///
/// Scenes with no urban zoning are never given an entry, because the compiler
/// does not state a policy a scene cannot use. Legacy behaviour (no morphology
/// profile at all) is untouched: `compile` returns `None` before this runs.
fn compile_residual(
    style: &Value,
    effective: &mut Value,
    profile: &MorphologyProfile,
    scenes: &[SceneSpec],
    sources: &mut Vec<FieldSource>,
    unsupported: &mut Vec<FieldSource>,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let path = "residualLandProgram";
    let share = ratio_midpoint(profile.public_space_ratio, "publicSpaceRatio")?;
    if !(0. ..=0.25).contains(&share) {
        return Err(err(format!(
            "profile publicSpaceRatio midpoint {share} exceeds the residual-land urban bound 0.25; declare urbanOpenSpaceRatio explicitly"
        )));
    }
    let Some(land_use) = LandUseProfile::from_style(style) else {
        unsupported.push(FieldSource::new(
            format!("{path}.scenes[*].urbanOpenSpaceRatio"),
            SOURCE_UNSUPPORTED,
            "publicSpaceRatio",
            "an open-space share needs an explicit landUseProfile to produce the urban remainder it subdivides",
        ));
        return Ok(());
    };
    let mut object = match style.get(path) {
        Some(program) => as_object(program, path)?.clone(),
        None => {
            let mut object = Map::new();
            object.insert("version".into(), json!(1));
            object.insert("scenes".into(), json!({}));
            object
        }
    };
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(err(format!("{path}.version must be 1")));
    }
    let mut scene_map = match object.remove("scenes") {
        Some(value) => as_object(&value, &format!("{path}.scenes"))?.clone(),
        None => Map::new(),
    };
    for scene in scenes {
        let zones = plan_landuse_zones(scene, &land_use);
        let owns_urban_remainder = zones.iter().any(|zone| zone.kind == "urban");
        // An authored entry always stays (and is checked for its own sake); a
        // scene without urban remainder never gets a synthesised one.
        if !owns_urban_remainder && !scene_map.contains_key(&scene.scene_id) {
            continue;
        }
        let scene_path = format!("{path}.scenes.{}", scene.scene_id);
        let entry = scene_map
            .entry(scene.scene_id.clone())
            .or_insert_with(|| json!({}));
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| err(format!("{scene_path} must be a JSON object")))?;
        if entry.contains_key("urbanOpenSpaceRatio") {
            sources.push(FieldSource::new(
                format!("{scene_path}.urbanOpenSpaceRatio"),
                SOURCE_ORIGINAL,
                "publicSpaceRatio",
                "explicit open-space share; the profile never overrides it",
            ));
        } else if owns_urban_remainder {
            entry.insert("urbanOpenSpaceRatio".into(), json!(share));
            sources.push(FieldSource::new(
                format!("{scene_path}.urbanOpenSpaceRatio"),
                SOURCE_PROFILE,
                "publicSpaceRatio",
                format!("unstated; profile midpoint public-space share {share}"),
            ));
        }
        if entry.contains_key("ruralPastureRatio") {
            sources.push(FieldSource::new(
                format!("{scene_path}.ruralPastureRatio"),
                SOURCE_ORIGINAL,
                "",
                "explicit pasture share; the profile has no independent pasture parameter and never overrides it",
            ));
        } else {
            notes.push(format!(
                "{scene_path}.ruralPastureRatio is unstated; a public-space share is not a pasture share, so the compiler states nothing and the residual-land consumer applies its named default {default}",
                default = super::residual_land::ABSENT_RATIO_DEFAULT
            ));
        }
    }
    let declared_by_scene = !scene_map.is_empty();
    object.insert("scenes".into(), Value::Object(scene_map));
    if style.get(path).is_none() && !declared_by_scene {
        // No scene owns an urban remainder and the manifest declared no
        // program: there is nothing to state, so nothing is added. A residual
        // program with zero scenes would be a rejected document, not a default.
        unsupported.push(FieldSource::new(
            format!("{path}.scenes[*].urbanOpenSpaceRatio"),
            SOURCE_UNSUPPORTED,
            "publicSpaceRatio",
            "no scene owns an urban remainder, so there is no open-space share to state",
        ));
        return Ok(());
    }
    effective
        .as_object_mut()
        .ok_or_else(|| err("style must be a JSON object"))?
        .insert(path.into(), Value::Object(object));
    super::residual_land::from_style(effective)
        .map_err(|error| err(format!("effective residualLandProgram is rejected: {error}")))?;
    Ok(())
}

/// Attach the profile village field template to scenes that own a rural zone.
fn compile_agriculture(
    style: &Value,
    effective: &mut Value,
    profile: &MorphologyProfile,
    scenes: &[SceneSpec],
    sources: &mut Vec<FieldSource>,
    unsupported: &mut Vec<FieldSource>,
) -> Result<(), String> {
    let Some(template) = profile.village_field_template else {
        return Ok(());
    };
    let path = "agricultureProgram";
    if style
        .get("naturalOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        unsupported.push(FieldSource::new(
            format!("{path}.scenes[*].defaultField"),
            SOURCE_UNSUPPORTED,
            "villageFieldTemplate",
            "naturalOnly manifest forbids an agriculture program",
        ));
        return Ok(());
    }
    let Some(land_use) = LandUseProfile::from_style(style) else {
        unsupported.push(FieldSource::new(
            format!("{path}.scenes[*].defaultField"),
            SOURCE_UNSUPPORTED,
            "villageFieldTemplate",
            "agricultureProgram needs an explicit landUseProfile",
        ));
        return Ok(());
    };
    let mut object = match style.get(path) {
        Some(program) => as_object(program, path)?.clone(),
        None => {
            let mut object = Map::new();
            object.insert("version".into(), json!(1));
            object.insert("scenes".into(), json!({}));
            object
        }
    };
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(err(format!("{path}.version must be 1")));
    }
    let mut scene_map = match object.remove("scenes") {
        Some(value) => as_object(&value, &format!("{path}.scenes"))?.clone(),
        None => Map::new(),
    };
    for scene in scenes {
        let zones = plan_landuse_zones(scene, &land_use);
        if !zones.iter().any(|zone| zone.kind == "rural") {
            continue;
        }
        let scene_path = format!("{path}.scenes.{}", scene.scene_id);
        let entry = scene_map
            .entry(scene.scene_id.clone())
            .or_insert_with(|| json!({}));
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| err(format!("{scene_path} must be a JSON object")))?;
        if entry.contains_key("defaultField") {
            sources.push(FieldSource::new(
                format!("{scene_path}.defaultField"),
                SOURCE_ORIGINAL,
                "villageFieldTemplate",
                "explicit field template; the profile never overrides it",
            ));
            continue;
        }
        entry.insert(
            "defaultField".into(),
            json!({"widthM": template[0], "depthM": template[1]}),
        );
        sources.push(FieldSource::new(
            format!("{scene_path}.defaultField"),
            SOURCE_PROFILE,
            "villageFieldTemplate",
            format!("unstated; profile template {}x{}m", template[0], template[1]),
        ));
    }
    object.insert("scenes".into(), Value::Object(scene_map));
    effective
        .as_object_mut()
        .ok_or_else(|| err("style must be a JSON object"))?
        .insert(path.into(), Value::Object(object));
    super::farm_program::from_style(effective)
        .map_err(|error| err(format!("effective agricultureProgram is rejected: {error}")))?;
    Ok(())
}

/// Compile the effective style for a manifest, or `None` for the legacy path.
pub fn compile(style: &Value, scenes: &[SceneSpec]) -> Result<Option<CompiledStyle>, String> {
    let Some(profile) = morphology::resolve(style)? else {
        return Ok(None);
    };
    let mut effective = style.clone();
    let mut field_sources = Vec::new();
    let mut unsupported = Vec::new();
    let mut notes = Vec::new();
    compile_frontage(
        style,
        &mut effective,
        &profile,
        &mut field_sources,
        &mut notes,
    )?;
    compile_auto_roads(
        style,
        &mut effective,
        &profile,
        &mut field_sources,
        &mut notes,
    )?;
    compile_residual(
        style,
        &mut effective,
        &profile,
        scenes,
        &mut field_sources,
        &mut unsupported,
        &mut notes,
    )?;
    compile_agriculture(
        style,
        &mut effective,
        &profile,
        scenes,
        &mut field_sources,
        &mut unsupported,
    )?;
    field_sources.push(FieldSource::new(
        "service-coverage-audit.json.radiusM",
        SOURCE_DERIVED,
        "serviceRadiusM",
        "worldgen audits every realized dwelling against every realized service at this radius; it never places facilities",
    ));
    field_sources.push(FieldSource::new(
        "morphology-coverage-audit.json.targetCoverageRatio",
        SOURCE_DERIVED,
        "coverageRatio",
        "opt-in worldgen audit measures each realized building footprint against its parcel; it never resizes authored geometry",
    ));
    for (field, reason) in [(
        "terrain",
        "descriptive profile metadata; the manifest terrain profile stays authoritative",
    )] {
        unsupported.push(FieldSource::new(
            format!("profile.{field}"),
            SOURCE_UNSUPPORTED,
            field,
            reason,
        ));
    }
    // Fail closed: every touched program must still be accepted by its own
    // strict reader, so a profile default can never widen a contract.
    crate::worldgen::road_extensions::from_style(&effective)
        .map_err(|error| err(format!("effective roadExtensions is rejected: {error}")))?;
    crate::worldgen::services::from_style(&effective)
        .map_err(|error| err(format!("effective settlementProgram is rejected: {error}")))?;
    let effective_style_hash = hash(&effective)?;
    let manifest_style_hash = hash(style)?;
    let report = EffectiveReport {
        format: "glyphweave.effective-programs".into(),
        version: EFFECTIVE_PROGRAMS_VERSION,
        revision: EFFECTIVE_PROGRAMS_REVISION.into(),
        status: "compiled".into(),
        profile_id: profile.profile_id,
        profile_version: profile.version,
        source_case_ids: profile.source_case_ids,
        evidence_level: profile.evidence_level,
        scenes: scenes.iter().map(|scene| scene.scene_id.clone()).collect(),
        manifest_style_hash,
        effective_style_hash,
        field_sources,
        unsupported,
        notes,
        effective_style: effective.clone(),
    };
    Ok(Some(CompiledStyle {
        style: effective,
        report,
    }))
}

/// Independent audit: recompile from the original manifest and compare with
/// the published document. The published value is never trusted as input.
pub fn audit_published(
    published: &Value,
    style: &Value,
    scenes: &[SceneSpec],
) -> Result<Option<CompiledStyle>, String> {
    let recompiled = compile(style, scenes)?;
    match (&recompiled, published) {
        (None, _) => Ok(None),
        (Some(compiled), published) => {
            let expected =
                serde_json::to_value(&compiled.report).map_err(|error| err(error.to_string()))?;
            if expected != *published {
                return Err(err(
                    "effective-programs.json differs from the recompiled manifest+profile",
                ));
            }
            Ok(Some(compiled.clone()))
        }
    }
}

/// The effective style used by generation and independent audit. Returns the
/// original style unchanged on the legacy path.
pub fn effective_style_or_original(
    style: &Value,
    scenes: &[SceneSpec],
) -> Result<Value, String> {
    Ok(compile(style, scenes)?
        .map(|compiled| compiled.style)
        .unwrap_or_else(|| style.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "scene-0".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn land_use(urban: f64) -> Value {
        json!({
            "theme": "temperate-plain",
            "urbanCoreRatio": urban,
            "suburbanRatio": urban,
            "greenRatio": 0.06,
            "farmRatio": 0.32,
            "forestRatio": 0.14,
            "pastureRatio": 0.26,
            "reserveRatio": 0.04
        })
    }

    fn home_request() -> Value {
        json!({
            "id": "homes",
            "sceneId": "scene-0",
            "purpose": "urban",
            "kind": "residential_home",
            "count": 16,
            "widthM": 12,
            "depthM": 10,
            "heightM": 8,
            "stationSpacingM": 24,
            "setbackM": 8,
            "maxCandidateAttempts": 512
        })
    }

    fn shop_request() -> Value {
        json!({
            "id": "shops",
            "sceneId": "scene-0",
            "purpose": "urban",
            "kind": "storefront",
            "count": 8,
            "widthM": 12,
            "depthM": 10,
            "heightM": 6,
            "stationSpacingM": 24,
            "setbackM": 8,
            "maxCandidateAttempts": 512
        })
    }

    /// Explicit building geometry plus an unstated parcel-inset policy. This is
    /// the shape the CLI control uses: demand stays authored, morphology is
    /// filled by the profile.
    fn style(profile_id: &str) -> Value {
        json!({
            "morphologyProfile": profile_id,
            "landUseProfile": land_use(0.10),
            "frontageProgram": {"version": 1, "requests": [home_request(), shop_request()]},
            "automaticRoadExtensions": {
                "version": 1,
                "scenes": [{
                    "sceneId": "scene-0",
                    "maxNewSegments": 16,
                    "maxCandidateAttempts": 512,
                    "segmentLengthsM": [128, 64],
                    "priorityPurposes": ["urban", "rural"],
                    "reachDistanceM": 64
                }]
            }
        })
    }

    fn compiled(profile_id: &str) -> CompiledStyle {
        compile(&style(profile_id), &[scene()])
            .unwrap()
            .expect("profile compiles")
    }

    fn insets(style: &Value, request: usize) -> (u64, u64, u64) {
        let insets = &style["frontageProgram"]["requests"][request]["parcelInsets"];
        (
            insets["frontM"].as_u64().unwrap(),
            insets["sideM"].as_u64().unwrap(),
            insets["rearM"].as_u64().unwrap(),
        )
    }

    #[test]
    fn six_profiles_expand_with_sources_and_declared_unsupported_fields() {
        let profiles = crate::settlement::morphology::builtins();
        let ids: Vec<String> = profiles
            .iter()
            .map(|profile| profile.profile_id.clone())
            .collect();
        assert_eq!(ids.len(), 6);
        // The same range-to-scalar rule the compiler uses, applied by the test
        // so the expectation is a number rather than a restatement of code.
        let share = |id: &str| {
            let profile = profiles
                .iter()
                .find(|profile| profile.profile_id == id)
                .unwrap();
            ((profile.public_space_ratio[0] + profile.public_space_ratio[1]) * 0.5 * 1000.).round()
                / 1000.
        };
        let mut inset_table = Vec::new();
        for id in &ids {
            let compiled = compiled(id);
            assert_eq!(compiled.report.profile_id, *id);
            assert_eq!(compiled.report.status, "compiled");
            assert_eq!(
                compiled.report.effective_style,
                compiled.style,
                "the report publishes the exact effective style generation consumes"
            );
            assert_ne!(
                compiled.report.effective_style_hash, compiled.report.manifest_style_hash,
                "a profile actually changed the effective programs for {id}"
            );
            // Authored demand and geometry survive untouched.
            assert_eq!(
                compiled.style["frontageProgram"]["requests"][0]["count"],
                json!(16)
            );
            assert_eq!(
                compiled.style["frontageProgram"]["requests"][0]["widthM"],
                json!(12)
            );
            // Profile-supplied parcel insets reached every request.
            assert_eq!(insets(&compiled.style, 0), insets(&compiled.style, 1));
            inset_table.push((id.clone(), insets(&compiled.style, 0)));
            // Road hierarchy is derived from the profile roles.
            let hierarchy = &compiled.style["automaticRoadExtensions"]["scenes"][0]["hierarchy"];
            assert_eq!(compiled.style["automaticRoadExtensions"]["version"], json!(2));
            assert!(hierarchy["tiers"].as_array().unwrap().len() >= 2);
            // The profile's public-space share reaches the residual-land
            // consumer as the urban open-space ratio and nothing else: no
            // pasture share, no forest share, no other ratio is invented.
            assert_eq!(
                compiled.style["residualLandProgram"],
                json!({"version": 1, "scenes": {"scene-0": {"urbanOpenSpaceRatio": share(id)}}}),
                "{id} must compile exactly the profile public-space share"
            );
            assert!(
                compiled
                    .report
                    .field_sources
                    .iter()
                    .any(|entry| entry.path.ends_with("urbanOpenSpaceRatio")
                        && entry.source == SOURCE_PROFILE
                        && entry.profile_field == "publicSpaceRatio"),
                "{id} must report the public-space source"
            );
            assert!(
                compiled
                    .report
                    .unsupported
                    .iter()
                    .all(|entry| entry.profile_field != "publicSpaceRatio"),
                "{id} must not declare a consumed field unsupported"
            );
            // Unsupported fields are declared, never silently claimed.
            for field in ["terrain"] {
                assert!(
                    compiled
                        .report
                        .unsupported
                        .iter()
                        .any(|entry| entry.profile_field == field
                            && entry.source == SOURCE_UNSUPPORTED),
                    "{id} must declare {field} unsupported"
                );
            }
            assert!(
                compiled.report.field_sources.iter().any(|entry| {
                    entry.profile_field == "coverageRatio"
                        && entry.source == SOURCE_DERIVED
                        && entry
                            .path
                            .ends_with("morphology-coverage-audit.json.targetCoverageRatio")
                }),
                "{id} must report the opt-in coverage audit consumer"
            );
            assert!(
                compiled
                    .report
                    .field_sources
                    .iter()
                    .any(|entry| entry.source == SOURCE_PROFILE
                        && entry.profile_field == "frontInsetM")
            );
        }
        let compact = inset_table
            .iter()
            .find(|(id, _)| id == "compact-mixed-block")
            .unwrap()
            .1;
        let low_density = inset_table
            .iter()
            .find(|(id, _)| id == "low-density-residential")
            .unwrap()
            .1;
        assert_ne!(
            compact, low_density,
            "profiles must differ in real parcel geometry, not only in a string id"
        );
    }

    #[test]
    fn legacy_manifest_without_profile_is_byte_identical() {
        let plain = json!({"landUseProfile": land_use(0.10)});
        assert!(compile(&plain, &[scene()]).unwrap().is_none());
        assert_eq!(
            effective_style_or_original(&plain, &[scene()]).unwrap(),
            plain
        );
    }

    /// A profile manifest that already states its own street setback compiles to
    /// the same effective document it produced before the derivation existed,
    /// apart from the single block that B-04 deliberately added: the residual
    /// land program carrying the profile's public-space share.
    ///
    /// The digests were captured from the parent commit. Removing
    /// `residualLandProgram` from the compiled document must still reproduce the
    /// pre-wiring style byte for byte (checked below), so a drift anywhere else
    /// (an authored setback included) is a hard failure instead of a silent
    /// world-revision change. The report digest moved once when
    /// `serviceRadiusM` and `coverageRatio` gained audit consumers; the
    /// effective style itself did not move.
    #[test]
    fn authored_setback_documents_keep_their_pre_wiring_digests() {
        for (profile_id, pre_wiring_style_digest, style_digest, report_digest) in [
            (
                "compact-mixed-block",
                "7b2b48790a5338e64dad1549386aaf838191fa7f9b6c9f568c7cd5df4a96eaaa",
                "98a6499ef8de3f2b54d34f0033e9a9b26ce0551b23441762f1b3ddf2f3eea96c",
                "b50f7972dd808e1ef27b880e57a87acd91b22d88a1a710a4661152dc79787abf",
            ),
            (
                "village-field",
                "75620dee9131ac7df0d72319c060df79e0ace7ccb4d5ba72bf8ccbe4f1b6a0b0",
                "f921ae85a58e9a83dc0730cd525915d16c1e41e9390c7534814a5ca010ffc298",
                "bad07d324860abaa30379334e5992ebdc9dfc74ca8f93dc9f6062164f7523f30",
            ),
        ] {
            let compiled = compile(&style(profile_id), &[scene()]).unwrap().unwrap();
            let mut pre_wiring = compiled.style.clone();
            let program = pre_wiring
                .as_object_mut()
                .unwrap()
                .remove("residualLandProgram")
                .expect("the public-space share is compiled for every urban scene");
            assert_eq!(
                blake3::hash(&serde_json::to_vec(&pre_wiring).unwrap())
                    .to_hex()
                    .as_str(),
                pre_wiring_style_digest,
                "{profile_id}: the pre-wiring document must be untouched apart from {program:?}"
            );
            assert_eq!(
                blake3::hash(&serde_json::to_vec(&compiled.style).unwrap())
                    .to_hex()
                    .as_str(),
                style_digest,
                "{profile_id}: the effective style for an authored setback must not drift"
            );
            assert_eq!(
                blake3::hash(&serde_json::to_vec(&compiled.report).unwrap())
                    .to_hex()
                    .as_str(),
                report_digest,
                "{profile_id}: the published effective-programs document must not drift"
            );
        }
    }

    /// The legacy path must stay byte-identical, not merely equal. A manifest
    /// without a morphology profile is passed through untouched, so its
    /// serialized bytes keep this frozen blake3 digest. The derived street
    /// setback lives inside the profile compiler, and the frontage consumer
    /// still requires the field for a profile-free request, so the default can
    /// never leak into an old input.
    #[test]
    fn legacy_style_bytes_and_blake3_are_frozen() {
        let legacy = json!({
            "landUseProfile": land_use(0.10),
            "frontageProgram": {"version": 1, "requests": [home_request()]}
        });
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let digest = blake3::hash(&bytes).to_hex().to_string();
        assert_eq!(
            digest, "262c93c57cde77ab9968bc486f560078dbb2b0f00b66f5365e8161ddda7d1df4",
            "the profile-free style bytes changed; no profile path may touch a legacy input"
        );
        assert!(compile(&legacy, &[scene()]).unwrap().is_none());
        let effective = effective_style_or_original(&legacy, &[scene()]).unwrap();
        assert_eq!(serde_json::to_vec(&effective).unwrap(), bytes);
        // A profile-free request still has to state its own setback: the
        // compiler's derivation is not a frontage default.
        let mut missing = legacy.clone();
        missing["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("setbackM");
        assert!(
            crate::worldgen::frontage::from_style(&missing).is_err(),
            "the derived setback must never widen the legacy frontage contract"
        );
    }

    #[test]
    fn profile_type_and_unknown_id_are_rejected() {
        for value in [json!(42), json!(["village-field"]), Value::Null] {
            let mut broken = style("village-field");
            broken["morphologyProfile"] = value;
            assert!(compile(&broken, &[scene()]).is_err());
        }
        let mut unknown = style("village-field");
        unknown["morphologyProfile"] = json!("no-such-profile");
        assert!(compile(&unknown, &[scene()])
            .unwrap_err()
            .contains("unknown morphologyProfile"));
    }

    /// A complete parcel is not a building. The default footprint is the
    /// parcel minus its yards, and an explicit footprint always wins.
    #[test]
    fn default_footprint_is_the_parcel_minus_its_yards() {
        // low-density-residential: parcel 26x50, insets 8/8/26, height 10.
        // That implies a 10x16 storefront footprint, which the storefront
        // consumer contract accepts (8..36 x 8..24 x 3..12).
        let mut value = json!({
            "morphologyProfile": "low-density-residential",
            "landUseProfile": land_use(0.10),
            "frontageProgram": {"version": 1, "requests": [{
                "id": "shops", "sceneId": "scene-0", "purpose": "urban",
                "kind": "storefront", "count": 4,
                "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 512
            }]}
        });
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        let request = &compiled.style["frontageProgram"]["requests"][0];
        assert_eq!(request["widthM"], json!(10));
        assert_eq!(request["depthM"], json!(16));
        assert_eq!(request["heightM"], json!(10));
        assert_eq!(insets(&compiled.style, 0), (8, 8, 26));
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].widthM")
                && entry.source == SOURCE_PROFILE
                && entry.profile_field == "parcelWidthM"
                && entry.detail.contains("complete parcel 26m - 2x8m profile side inset")
        }));
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].depthM")
                && entry.source == SOURCE_PROFILE
                && entry.profile_field == "parcelDepthM"
                && entry
                    .detail
                    .contains("complete parcel 50m - 8m profile front - 26m profile rear")
        }));
        // The derived footprint plus the profile insets really reconstructs
        // the profile's complete parcel through the production geometry.
        let bounds = crate::worldgen::parcel_geometry::boundary(
            0,
            0,
            10,
            16,
            0.,
            crate::worldgen::parcel_geometry::ParcelInsets {
                front_m: 8,
                side_m: 8,
                rear_m: 26,
            },
        )
        .unwrap();
        assert_eq!((bounds[2] - bounds[0], bounds[3] - bounds[1]), (26, 50));

        // Explicit geometry always wins, and the profile only supplies the
        // unstated part.
        value["frontageProgram"]["requests"][0]["widthM"] = json!(12);
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        let request = &compiled.style["frontageProgram"]["requests"][0];
        assert_eq!(request["widthM"], json!(12));
        assert_eq!(request["depthM"], json!(16));
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].widthM") && entry.source == SOURCE_ORIGINAL
        }));
        // An explicit 12m building plus the 8/8/26 insets is a 28x50m parcel,
        // still inside the profile parcel band, so no deviation note is raised.
        assert!(
            compiled
                .report
                .notes
                .iter()
                .all(|note| !note.contains("outside the profile parcel")),
            "{:?}",
            compiled.report.notes
        );
    }

    /// An explicit footprint outside the profile's parcel band is kept exactly
    /// as authored, with the deviation reported rather than adjusted.
    #[test]
    fn explicit_geometry_outside_the_profile_band_is_reported_not_adjusted() {
        let mut value = style("village-field");
        value["frontageProgram"]["requests"][0]["widthM"] = json!(48);
        value["frontageProgram"]["requests"][0]["depthM"] = json!(36);
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        let request = &compiled.style["frontageProgram"]["requests"][0];
        assert_eq!(request["widthM"], json!(48));
        assert_eq!(request["depthM"], json!(36));
        assert!(
            compiled
                .report
                .notes
                .iter()
                .any(|note| note.contains("outside the profile parcelWidthM band")),
            "{:?}",
            compiled.report.notes
        );
        assert!(
            compiled
                .report
                .notes
                .iter()
                .any(|note| note.contains("outside the profile parcelDepthM band")),
            "{:?}",
            compiled.report.notes
        );
    }

    /// The parcel-minus-yards arithmetic is checked and refuses impossible or
    /// B04-C1-R2: an explicit parcel policy must drive the default footprint.
    /// R2 closeout: a stated axis is never blocked by the default of another
    /// axis. The inspector's axis cases are the acceptance set.
    #[test]
    fn explicit_axes_are_never_blocked_by_an_unused_default() {
        let build = |patch: Value| -> Result<CompiledStyle, String> {
            let mut request = json!({
                "id": "shop", "sceneId": "scene-0", "purpose": "urban",
                "kind": "storefront", "count": 1,
                "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192,
                "parcelInsets": {"frontM": 8, "sideM": 16, "rearM": 26}
            });
            for (key, value) in patch.as_object().unwrap() {
                request[key] = value.clone();
            }
            let style = json!({
                "morphologyProfile": "low-density-residential",
                "frontageProgram": {"version": 1, "requests": [request]}
            });
            compile(&style, &[scene()]).map(|compiled| compiled.unwrap())
        };

        // Both axes authored: the profile's own parcel-minus-side arithmetic is
        // invalid here (26 - 2x16), but it is never computed, so the legal
        // explicit footprint passes and the 44x50 parcel deviation is reported.
        let both = build(json!({"widthM": 12, "depthM": 16, "heightM": 10})).unwrap();
        let request = both.style["frontageProgram"]["requests"][0].clone();
        assert_eq!(request["widthM"], json!(12));
        assert_eq!(request["depthM"], json!(16));
        assert_eq!(
            request["parcelInsets"],
            json!({"frontM": 8, "sideM": 16, "rearM": 26})
        );
        let insets: crate::worldgen::parcel_geometry::ParcelInsets =
            serde_json::from_value(request["parcelInsets"].clone()).unwrap();
        let bounds = crate::worldgen::parcel_geometry::boundary(0, 0, 12, 16, 0., insets).unwrap();
        assert_eq!((bounds[2] - bounds[0], bounds[3] - bounds[1]), (44, 50));
        assert!(
            both.report
                .notes
                .iter()
                .any(|note| note.contains("outside the profile parcelWidthM band")),
            "{:?}",
            both.report.notes
        );
        assert!(both.report.field_sources.iter().all(|entry| {
            !entry.path.ends_with("requests[0].widthM")
                && !entry.path.ends_with("requests[0].depthM")
                || entry.source == SOURCE_ORIGINAL
        }));

        // Only depth missing: width stays authored, depth is derived from the
        // authored insets (50 - 8 - 26 = 16).
        let depth_only = build(json!({"widthM": 12, "heightM": 10})).unwrap();
        let request = depth_only.style["frontageProgram"]["requests"][0].clone();
        assert_eq!(request["widthM"], json!(12));
        assert_eq!(request["depthM"], json!(16));
        assert!(depth_only.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].depthM") && entry.source == SOURCE_PROFILE
        }));
        assert!(depth_only.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].widthM") && entry.source == SOURCE_ORIGINAL
        }));

        // Only width missing: the real default width is 26 - 2x16 < 0, so this
        // must be refused (the authored depth cannot rescue it).
        let error = build(json!({"depthM": 16, "heightM": 10})).unwrap_err();
        assert!(
            error.contains("cannot hold 2x16m side insets"),
            "{error}"
        );

        // Symmetric depth case: front 8 + rear 50 exceeds the 50m parcel, so a
        // missing depth is refused while an authored depth is kept.
        let deep = json!({"frontM": 8, "sideM": 2, "rearM": 50});
        let mut value = json!({
            "morphologyProfile": "low-density-residential",
            "frontageProgram": {"version": 1, "requests": [{
                "id": "shop", "sceneId": "scene-0", "purpose": "urban",
                "kind": "storefront", "count": 1, "widthM": 12, "depthM": 16,
                "heightM": 10, "stationSpacingM": 24, "setbackM": 12,
                "maxCandidateAttempts": 8192, "parcelInsets": deep
            }]}
        });
        assert!(compile(&value, &[scene()]).unwrap().is_some());
        value["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("depthM");
        let error = compile(&value, &[scene()]).unwrap_err();
        assert!(error.contains("cannot hold 8m front + 50m rear insets"), "{error}");
        // The same authored depth with the width missing is still fine because
        // the width default (26 - 2x2 = 22) is valid on its own axis: the two
        // axes are derived and validated independently.
        value["frontageProgram"]["requests"][0]["depthM"] = json!(16);
        value["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("widthM");
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        assert_eq!(
            compiled.style["frontageProgram"]["requests"][0]["widthM"],
            json!(22)
        );
    }

    /// B04-C1-R2: an explicit parcel policy must drive the default footprint.
    /// The inspector's three probe inputs are the acceptance cases.
    #[test]
    fn request_insets_drive_the_default_footprint() {
        let build = |insets: Option<Value>| -> Result<CompiledStyle, String> {
            let mut value = json!({
                "morphologyProfile": "low-density-residential",
                "frontageProgram": {"version": 1, "requests": [{
                    "id": "shop", "sceneId": "scene-0", "purpose": "urban",
                    "kind": "storefront", "count": 1,
                    "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192
                }]}
            });
            if let Some(insets) = insets {
                value["frontageProgram"]["requests"][0]["parcelInsets"] = insets;
            }
            compile(&value, &[scene()]).map(|compiled| compiled.unwrap())
        };
        let parcel = |compiled: &CompiledStyle| {
            let request = &compiled.style["frontageProgram"]["requests"][0];
            let insets: crate::worldgen::parcel_geometry::ParcelInsets =
                serde_json::from_value(request["parcelInsets"].clone()).unwrap();
            let bounds = crate::worldgen::parcel_geometry::boundary(
                0,
                0,
                request["widthM"].as_u64().unwrap() as u32,
                request["depthM"].as_u64().unwrap() as u32,
                0.,
                insets,
            )
            .unwrap();
            (
                request["widthM"].as_u64().unwrap(),
                request["depthM"].as_u64().unwrap(),
                bounds[2] - bounds[0],
                bounds[3] - bounds[1],
            )
        };

        // 1. Unstated insets: the profile baseline stays 10x16 in a 26x50 parcel.
        let baseline = build(None).unwrap();
        assert_eq!(parcel(&baseline), (10, 16, 26, 50));
        assert!(baseline.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].widthM")
                && entry.detail.contains("2x8m profile side inset")
        }));

        // 2. Authored 8/2/26: the footprint must be derived from *those* insets,
        //    giving 22x16 inside the same 26x50 parcel (not the stale 10x16).
        let authored = build(Some(json!({"frontM": 8, "sideM": 2, "rearM": 26}))).unwrap();
        assert_eq!(parcel(&authored), (22, 16, 26, 50));
        assert!(authored.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].widthM")
                && entry.source == SOURCE_PROFILE
                && entry.detail.contains("2x2m authored side inset")
        }));
        assert!(authored.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].depthM")
                && entry
                    .detail
                    .contains("8m authored front - 26m authored rear")
        }));
        assert!(authored.report.field_sources.iter().any(|entry| {
            entry.path == "frontageProgram.requests[0].parcelInsets"
                && entry.source == SOURCE_ORIGINAL
                && entry.detail.contains("default footprint uses them")
        }));

        // 3. Authored 2/2/2 derives 22x46, which the storefront depth contract
        //    must refuse. The stale 10x16 footprint is never silently reused.
        let error = build(Some(json!({"frontM": 2, "sideM": 2, "rearM": 2}))).unwrap_err();
        assert!(error.contains("no shrinking"), "{error}");
        assert!(
            error.contains("requests[0].depthM") && error.contains("requests[0].widthM"),
            "the refusal must name the derived axes: {error}"
        );

        // A malformed explicit policy is refused before any derivation.
        for broken in [
            json!({"sideM": 2, "rearM": 26}),
            json!({"frontM": "8", "sideM": 2, "rearM": 26}),
            json!([]),
            json!(null),
        ] {
            let error = build(Some(broken)).unwrap_err();
            assert!(error.contains("parcelInsets"), "{error}");
        }
    }

    /// B04-C1-R2: axis-by-axis priority with the effective insets.
    #[test]
    fn explicit_axes_win_and_only_missing_axes_are_derived() {
        let build = |request_patch: Value| -> Result<CompiledStyle, String> {
            let mut value = json!({
                "morphologyProfile": "low-density-residential",
                "frontageProgram": {"version": 1, "requests": [{
                    "id": "shop", "sceneId": "scene-0", "purpose": "urban",
                    "kind": "storefront", "count": 1,
                    "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192,
                    "parcelInsets": {"frontM": 8, "sideM": 2, "rearM": 26}
                }]}
            });
            for (key, item) in request_patch.as_object().unwrap() {
                value["frontageProgram"]["requests"][0][key] = item.clone();
            }
            compile(&value, &[scene()]).map(|compiled| compiled.unwrap())
        };
        let request = |compiled: &CompiledStyle| {
            compiled.style["frontageProgram"]["requests"][0].clone()
        };
        let source_of = |compiled: &CompiledStyle, suffix: &str| -> String {
            compiled
                .report
                .field_sources
                .iter()
                .find(|entry| entry.path.ends_with(suffix))
                .map(|entry| entry.source.clone())
                .unwrap_or_default()
        };

        // Only width explicit: depth still comes from the authored insets.
        let only_width = build(json!({"widthM": 12})).unwrap();
        assert_eq!(request(&only_width)["widthM"], json!(12));
        assert_eq!(request(&only_width)["depthM"], json!(16));
        assert_eq!(source_of(&only_width, "widthM"), SOURCE_ORIGINAL);
        assert_eq!(source_of(&only_width, "depthM"), SOURCE_PROFILE);

        // Only depth explicit: width still comes from the authored insets.
        let only_depth = build(json!({"depthM": 20})).unwrap();
        assert_eq!(request(&only_depth)["widthM"], json!(22));
        assert_eq!(request(&only_depth)["depthM"], json!(20));
        assert_eq!(source_of(&only_depth, "widthM"), SOURCE_PROFILE);
        assert_eq!(source_of(&only_depth, "depthM"), SOURCE_ORIGINAL);

        // Both explicit: no derivation, and the authored insets are untouched.
        let both = build(json!({"widthM": 34, "depthM": 22})).unwrap();
        assert_eq!(request(&both)["widthM"], json!(34));
        assert_eq!(request(&both)["depthM"], json!(22));
        assert_eq!(
            request(&both)["parcelInsets"],
            json!({"frontM": 8, "sideM": 2, "rearM": 26})
        );
        assert!(both
            .report
            .field_sources
            .iter()
            .all(|entry| !entry.path.ends_with("widthM")
                || entry.source == SOURCE_ORIGINAL
                || entry.source == SOURCE_PROFILE));
        // 34 + 2x2 = 38m is above the profile parcelWidthM band [16, 36], so the
        // deviation is reported rather than adjusted.
        assert!(
            both.report
                .notes
                .iter()
                .any(|note| note.contains("outside the profile parcelWidthM band")),
            "{:?}",
            both.report.notes
        );
        assert!(both
            .report
            .notes
            .iter()
            .all(|note| !note.contains("profile insets")));
    }

    /// B04-C1-R2: the derived parcel keeps its extents for all four cardinal
    /// fronts, using authored insets rather than the profile defaults.
    #[test]
    fn derived_parcel_with_authored_insets_holds_for_all_four_fronts() {
        let compiled = compile(
            &json!({
                "morphologyProfile": "low-density-residential",
                "frontageProgram": {"version": 1, "requests": [{
                    "id": "shop", "sceneId": "scene-0", "purpose": "urban",
                    "kind": "storefront", "count": 1,
                    "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 8192,
                    "parcelInsets": {"frontM": 8, "sideM": 2, "rearM": 26}
                }]}
            }),
            &[scene()],
        )
        .unwrap()
        .unwrap();
        let request = &compiled.style["frontageProgram"]["requests"][0];
        let (width, depth) = (
            request["widthM"].as_u64().unwrap() as u32,
            request["depthM"].as_u64().unwrap() as u32,
        );
        let insets = crate::worldgen::parcel_geometry::ParcelInsets {
            front_m: 8,
            side_m: 2,
            rear_m: 26,
        };
        for rotation in [0., 90., 180., 270.] {
            let bounds = crate::worldgen::parcel_geometry::boundary(
                0, 0, width, depth, rotation, insets,
            )
            .unwrap();
            let extents = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
            let expected = if matches!(rotation.round() as i32, 90 | 270) {
                (50, 26)
            } else {
                (26, 50)
            };
            assert_eq!(extents, expected, "rotation {rotation}");
        }
    }

    /// The parcel-minus-yards arithmetic is checked and refuses impossible or
    /// odd combinations instead of clamping them.
    #[test]
    fn footprint_deduction_rejects_degenerate_and_odd_parcels() {
        let base = ParcelPolicy {
            width_m: 20,
            depth_m: 30,
            front_m: 4,
            side_m: 4,
            rear_m: 8,
        };
        assert_eq!(base.footprint_width().unwrap(), 12);
        assert_eq!(base.footprint_depth().unwrap(), 18);
        for (policy, width_fails, depth_fails) in [
            // Side insets consume the whole parcel width.
            (
                ParcelPolicy {
                    width_m: 8,
                    ..base
                },
                true,
                false,
            ),
            // Front and rear insets consume the whole parcel depth.
            (
                ParcelPolicy {
                    depth_m: 12,
                    ..base
                },
                false,
                true,
            ),
            // Odd footprint after deduction (a profile range cannot produce an
            // odd parcel, but the arithmetic must still refuse one).
            (
                ParcelPolicy {
                    width_m: 21,
                    ..base
                },
                true,
                false,
            ),
        ] {
            // Each axis is validated on its own: a degenerate default width
            // must never be reported as a degenerate depth, and vice versa.
            assert_eq!(
                policy.footprint_width().is_err(),
                width_fails,
                "width of {policy:?}"
            );
            assert_eq!(
                policy.footprint_depth().is_err(),
                depth_fails,
                "depth of {policy:?}"
            );
        }
    }

    /// The complete parcel box is orientation independent: all four cardinal
    /// fronts give the same parcel extents through the production geometry.
    #[test]
    fn derived_parcel_box_holds_for_all_four_cardinal_fronts() {
        for profile_id in [
            "compact-mixed-block",
            "perimeter-block",
            "low-density-residential",
            "clustered-residential",
            "terrain-constrained-compact",
            "village-field",
        ] {
            let profile = crate::settlement::morphology::builtins()
                .into_iter()
                .find(|profile| profile.profile_id == profile_id)
                .unwrap();
            let policy = ParcelPolicy::from_profile(&profile).unwrap();
            let width_m = policy.footprint_width().unwrap();
            let depth_m = policy.footprint_depth().unwrap();
            let insets = crate::worldgen::parcel_geometry::ParcelInsets {
                front_m: policy.front_m,
                side_m: policy.side_m,
                rear_m: policy.rear_m,
            };
            for rotation in [0., 90., 180., 270.] {
                let bounds = crate::worldgen::parcel_geometry::boundary(
                    0, 0, width_m, depth_m, rotation, insets,
                )
                .unwrap();
                let extents = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
                let (parcel_x, parcel_z) = (policy.width_m as i64, policy.depth_m as i64);
                let expected = if matches!(rotation.round() as i32, 90 | 270) {
                    (parcel_z, parcel_x)
                } else {
                    (parcel_x, parcel_z)
                };
                assert_eq!(
                    extents,
                    expected,
                    "{profile_id} at {rotation}deg"
                );
                // The complete parcel stays inside the profile's declared band.
                assert!(policy.width_m >= profile.parcel_width_m[0]
                    && policy.width_m <= profile.parcel_width_m[1]);
                assert!(policy.depth_m >= profile.parcel_depth_m[0]
                    && policy.depth_m <= profile.parcel_depth_m[1]);
            }
        }
    }

    #[test]
    fn a_missing_count_is_never_allocated_by_the_profile() {
        let mut value = style("village-field");
        value["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("count");
        let error = compile(&value, &[scene()]).unwrap_err();
        assert!(error.contains("never allocates buildings"), "{error}");
        let mut zero = style("village-field");
        zero["frontageProgram"]["requests"][0]["count"] = json!(0);
        assert!(compile(&zero, &[scene()]).is_err());
    }

    #[test]
    fn incompatible_default_is_rejected_instead_of_clamped() {
        // A profile-derived field that the building consumer refuses must fail
        // the compile, and the message must name the derived field.
        let cases = [
            // compact-mixed-block height midpoint 16m > 14m home bound.
            ("compact-mixed-block", "heightM", "16m > 14m"),
            // low-density-residential footprint width 10m < 12m home bound.
            ("low-density-residential", "widthM", "10m < 12m"),
        ];
        for (profile_id, field, _reason) in cases {
            let mut value = style(profile_id);
            value["frontageProgram"]["requests"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            let error = compile(&value, &[scene()]).unwrap_err();
            assert!(error.contains("frontageProgram"), "{profile_id}: {error}");
            assert!(error.contains("no shrinking"), "{profile_id}: {error}");
            assert!(
                error.contains(field),
                "{profile_id}: the refusal must name the derived {field}: {error}"
            );
            assert!(
                error.contains("profile-derived fields"),
                "{profile_id}: {error}"
            );
        }
    }

    #[test]
    fn a_missing_search_budget_is_not_invented() {
        let mut value = style("village-field");
        value["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("stationSpacingM");
        let error = compile(&value, &[scene()]).unwrap_err();
        assert!(error.contains("stationSpacingM"), "{error}");
        assert!(error.contains("no consumer mapping"), "{error}");
    }

    /// The profile's front yard bounds the street setback. A request that states
    /// no `setbackM` gets the front inset plus the frontage road-protection band,
    /// which is the one setback whose parcel front boundary sits exactly on the
    /// protected road line (one metre less and the parcel preflight rejects every
    /// candidate). A stated setback is never replaced, and a derivation outside
    /// the frontage band is refused instead of clamped.
    #[test]
    fn unstated_street_setback_is_the_front_yard_plus_the_road_band() {
        let band = u32::try_from(crate::worldgen::frontage::ROAD_PROTECTION_M).unwrap();
        assert_eq!(band, 4, "the road-protection band is a frozen 4m contract");
        // profile -> (front inset, derived setback)
        for (profile_id, front, setback) in [
            ("compact-mixed-block", 4, 8),
            ("perimeter-block", 4, 8),
            ("low-density-residential", 8, 12),
            ("clustered-residential", 6, 10),
            ("terrain-constrained-compact", 4, 8),
            ("village-field", 6, 10),
        ] {
            let mut value = style(profile_id);
            for request in value["frontageProgram"]["requests"].as_array_mut().unwrap() {
                request.as_object_mut().unwrap().remove("setbackM");
            }
            let compiled = compile(&value, &[scene()]).unwrap().unwrap();
            for index in 0..2 {
                let request = &compiled.style["frontageProgram"]["requests"][index];
                assert_eq!(
                    request["parcelInsets"]["frontM"],
                    json!(front),
                    "{profile_id}[{index}]"
                );
                assert_eq!(request["setbackM"], json!(setback), "{profile_id}[{index}]");
                // The gap between the road corridor and the parcel's front
                // boundary is exactly the protection band, for every profile.
                assert_eq!(setback - front, band, "{profile_id}[{index}]");
            }
            assert!(compiled.report.field_sources.iter().any(|entry| {
                entry.path.ends_with("requests[0].setbackM")
                    && entry.source == SOURCE_DERIVED
                    && entry.profile_field == "frontInsetM"
                    && entry.detail.contains("road-protection band 4m")
            }));
            // The frontage consumer accepts the derived value unchanged.
            let program = crate::worldgen::frontage::from_style(&compiled.style)
                .unwrap()
                .unwrap();
            assert!(program.requests.iter().all(|request| request.setback_m == setback));
            assert!(program
                .requests
                .iter()
                .all(|request| request.parcel_insets.unwrap().front_m < setback));
        }

        // An authored setback wins and is reported as authored truth.
        let mut authored = style("compact-mixed-block");
        for request in authored["frontageProgram"]["requests"].as_array_mut().unwrap() {
            request["setbackM"] = json!(12);
        }
        let compiled = compile(&authored, &[scene()]).unwrap().unwrap();
        for index in 0..2 {
            assert_eq!(
                compiled.style["frontageProgram"]["requests"][index]["setbackM"],
                json!(12)
            );
        }
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].setbackM")
                && entry.source == SOURCE_ORIGINAL
                && entry.profile_field.is_empty()
                && entry.detail
                    == "request search/engineering parameter outside the profile's morphology scope"
        }));

        // The request's own authored front inset drives the derivation, so an
        // authored parcel policy is honoured rather than the profile's range.
        let mut own_insets = style("compact-mixed-block");
        for request in own_insets["frontageProgram"]["requests"]
            .as_array_mut()
            .unwrap()
        {
            request.as_object_mut().unwrap().remove("setbackM");
            request["parcelInsets"] = json!({"frontM": 10, "sideM": 2, "rearM": 4});
        }
        let compiled = compile(&own_insets, &[scene()]).unwrap().unwrap();
        assert_eq!(
            compiled.style["frontageProgram"]["requests"][0]["setbackM"],
            json!(14),
            "10m authored front inset + 4m band"
        );
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.path.ends_with("requests[0].setbackM")
                && entry.detail.contains("authored front inset 10m")
        }));

        // Outside the frontage request band: refuse, never clamp.
        let mut too_far = style("compact-mixed-block");
        for request in too_far["frontageProgram"]["requests"].as_array_mut().unwrap() {
            request.as_object_mut().unwrap().remove("setbackM");
            request["parcelInsets"] = json!({"frontM": 30, "sideM": 2, "rearM": 4});
        }
        let error = compile(&too_far, &[scene()]).unwrap_err();
        assert!(error.contains("frontInsetM"), "{error}");
        assert!(error.contains("34m street setback"), "{error}");
    }

    /// The published effective document is a claim about the derivation, so a
    /// forged derived setback must be rejected by the independent audit exactly
    /// like a forged footprint or inset.
    #[test]
    fn a_forged_derived_setback_is_rejected_by_the_audit() {
        let mut value = style("compact-mixed-block");
        for request in value["frontageProgram"]["requests"].as_array_mut().unwrap() {
            request.as_object_mut().unwrap().remove("setbackM");
        }
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        assert_eq!(
            compiled.style["frontageProgram"]["requests"][0]["setbackM"],
            json!(8)
        );
        let published = serde_json::to_value(&compiled.report).unwrap();
        assert!(audit_published(&published, &value, &[scene()])
            .unwrap()
            .is_some());
        for (label, forged) in [
            ("wider", json!(12)),
            ("narrower", json!(6)),
        ] {
            let mut document = published.clone();
            document["effectiveStyle"]["frontageProgram"]["requests"][0]["setbackM"] = forged;
            assert!(
                audit_published(&document, &value, &[scene()]).is_err(),
                "a {label} forged street setback must not be accepted"
            );
        }
    }

    #[test]
    fn compilation_is_deterministic_and_request_order_is_stable() {
        let first = compiled("compact-mixed-block");
        let second = compiled("compact-mixed-block");
        assert_eq!(first.style, second.style);
        assert_eq!(
            serde_json::to_value(&first.report).unwrap(),
            serde_json::to_value(&second.report).unwrap()
        );
        let mut extended = style("compact-mixed-block");
        extended["frontageProgram"]["requests"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "later-store",
                "sceneId": "scene-0",
                "purpose": "urban",
                "kind": "storefront",
                "count": 1,
                "widthM": 12,
                "depthM": 10,
                "heightM": 6,
                "stationSpacingM": 24,
                "setbackM": 8,
                "maxCandidateAttempts": 64
            }));
        let extended = compile(&extended, &[scene()]).unwrap().unwrap();
        let ids: Vec<&str> = extended.style["frontageProgram"]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["homes", "shops", "later-store"]);
    }

    #[test]
    fn village_field_profile_wires_hierarchy_residual_land_and_field_template() {
        let compiled = compiled("village-field");
        let tiers = compiled.style["automaticRoadExtensions"]["scenes"][0]["hierarchy"]["tiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tier| {
                (
                    tier["role"].as_str().unwrap().to_owned(),
                    tier["widthM"].as_u64().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            tiers,
            vec![
                ("collector".to_owned(), 16),
                ("local".to_owned(), 12),
                ("farm_access".to_owned(), 8),
            ]
        );
        assert!(compiled
            .report
            .field_sources
            .iter()
            .any(|entry| entry.source == SOURCE_DERIVED && entry.profile_field == "roadHierarchy"));
        assert_eq!(
            compiled.style["agricultureProgram"]["scenes"]["scene-0"]["defaultField"],
            json!({"widthM": 24, "depthM": 24})
        );
        assert!(compiled.report.field_sources.iter().any(|entry| {
            entry.source == SOURCE_PROFILE && entry.profile_field == "villageFieldTemplate"
        }));
        // The public-space share now has a consumer: it compiles the urban
        // open-space ratio for the urban scene and states no pasture share, so
        // the consumer's named default (0) applies to the part it cannot know.
        assert_eq!(
            compiled.style["residualLandProgram"],
            json!({"version": 1, "scenes": {"scene-0": {"urbanOpenSpaceRatio": 0.14}}})
        );
        assert!(compiled
            .report
            .field_sources
            .iter()
            .any(|entry| entry.source == SOURCE_PROFILE
                && entry.profile_field == "publicSpaceRatio"
                && entry.path.ends_with("urbanOpenSpaceRatio")));
        assert!(
            compiled
                .report
                .unsupported
                .iter()
                .all(|entry| entry.profile_field != "publicSpaceRatio"),
            "a field with a real consumer is not unsupported"
        );
        assert!(compiled.report.notes.iter().any(|note| note.contains(
            "residualLandProgram.scenes.scene-0.ruralPastureRatio is unstated"
        )));
    }

    #[test]
    fn a_profile_without_a_field_template_declares_it_absent_not_defaulted() {
        let compiled = compiled("compact-mixed-block");
        assert!(compiled.style.get("agricultureProgram").is_none());
        assert!(compiled
            .report
            .field_sources
            .iter()
            .all(|entry| entry.profile_field != "villageFieldTemplate"));
    }

    #[test]
    fn published_report_is_recompiled_from_source_not_trusted() {
        let value = style("village-field");
        let compiled = compile(&value, &[scene()]).unwrap().unwrap();
        let published = serde_json::to_value(&compiled.report).unwrap();
        let audited = audit_published(&published, &value, &[scene()])
            .unwrap()
            .unwrap();
        assert_eq!(audited.style, compiled.style);
        // A forged effective document is rejected.
        let mut forged = published.clone();
        forged["effectiveStyle"]["frontageProgram"]["requests"][0]["widthM"] = json!(30);
        assert!(audit_published(&forged, &value, &[scene()]).is_err());
        // The residual public-space share is part of the published document:
        // changing it, or deleting the field that states it, is rejected too.
        let mut forged_ratio = published.clone();
        forged_ratio["effectiveStyle"]["residualLandProgram"]["scenes"]["scene-0"]
            ["urbanOpenSpaceRatio"] = json!(0.05);
        assert!(audit_published(&forged_ratio, &value, &[scene()]).is_err());
        let mut removed_ratio = published.clone();
        removed_ratio["effectiveStyle"]["residualLandProgram"]["scenes"]["scene-0"]
            .as_object_mut()
            .unwrap()
            .remove("urbanOpenSpaceRatio");
        assert!(audit_published(&removed_ratio, &value, &[scene()]).is_err());
        // So is a recompiled document for a different profile version.
        let mut wrong_profile = value.clone();
        wrong_profile["morphologyProfile"] = json!("compact-mixed-block");
        assert!(audit_published(&published, &wrong_profile, &[scene()]).is_err());
        // The legacy path publishes nothing.
        assert!(audit_published(&published, &json!({}), &[scene()])
            .unwrap()
            .is_none());
    }

    #[test]
    fn b04_village_field_example_keeps_every_explicit_program() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../../../../examples/morphology/b04-village-field.manifest.json"
        ))
        .unwrap();
        let style = &manifest["style"];
        let scenes: Vec<SceneSpec> = serde_json::from_value(manifest["scenes"].clone()).unwrap();
        let compiled = compile(style, &scenes).unwrap().unwrap();
        assert_eq!(
            compiled.style["frontageProgram"], style["frontageProgram"],
            "explicit demand is authored truth"
        );
        assert_eq!(
            compiled.style["automaticRoadExtensions"], style["automaticRoadExtensions"]
        );
        assert_eq!(
            compiled.style["agricultureProgram"], style["agricultureProgram"]
        );
        assert_eq!(
            compiled.style["residualLandProgram"], style["residualLandProgram"]
        );
    }

    /// The honest coverage matrix for the *derived footprint* defaults, per
    /// building kind. Parcel insets always work; a footprint default is only
    /// used where the building consumer contract accepts it, and otherwise the
    /// compile refuses rather than shrinking the building. Asset-contract
    /// bounds are additionally enforced by `WorldManifest::validate`.
    #[test]
    fn profile_footprint_matrix_is_accepted_or_refused_never_shrunk() {
        // profile -> (parcel insets, derived footprint)
        let expected = [
            ("compact-mixed-block", (4, 2, 4), (14, 16)),
            ("perimeter-block", (4, 4, 22), (34, 30)),
            ("low-density-residential", (8, 8, 26), (10, 16)),
            ("clustered-residential", (6, 4, 16), (26, 22)),
            ("terrain-constrained-compact", (4, 2, 8), (16, 16)),
            ("village-field", (6, 4, 20), (14, 8)),
        ];
        let kinds = ["residential_home", "residential_block", "storefront"];
        let mut matrix = Vec::new();
        for (profile_id, expected_insets, footprint) in expected {
            let compiled = compiled(profile_id);
            assert_eq!(insets(&compiled.style, 0), expected_insets, "{profile_id}");
            for kind in kinds {
                let mut value = json!({
                    "morphologyProfile": profile_id,
                    "landUseProfile": land_use(0.10),
                    "frontageProgram": {"version": 1, "requests": [{
                        "id": "target", "sceneId": "scene-0", "purpose": "urban",
                        "kind": kind, "count": 4,
                        "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 512
                    }]}
                });
                let outcome = compile(&value, &[scene()]);
                match outcome {
                    Ok(Some(compiled)) => {
                        let request = &compiled.style["frontageProgram"]["requests"][0];
                        assert_eq!(
                            (
                                request["widthM"].as_u64().unwrap() as u32,
                                request["depthM"].as_u64().unwrap() as u32
                            ),
                            footprint,
                            "{profile_id}/{kind} must use the parcel-minus-yards footprint"
                        );
                        matrix.push((profile_id, kind, true));
                    }
                    Err(error) => {
                        assert!(
                            error.contains("no shrinking"),
                            "{profile_id}/{kind}: {error}"
                        );
                        matrix.push((profile_id, kind, false));
                    }
                    Ok(None) => panic!("{profile_id} must resolve"),
                }
                // A default that the contract refuses must never appear as a
                // silently resized building.
                value["frontageProgram"]["requests"][0]["kind"] = json!(kind);
                if let Ok(Some(compiled)) = compile(&value, &[scene()]) {
                    let request = &compiled.style["frontageProgram"]["requests"][0];
                    assert_eq!(
                        (
                            request["widthM"].as_u64().unwrap() as u32,
                            request["depthM"].as_u64().unwrap() as u32
                        ),
                        footprint,
                        "{profile_id}/{kind} must not be resized into range"
                    );
                }
            }
        }
        // Documented matrix: only these combinations clear the consumer
        // contract without shrinking.
        let accepted: Vec<(&str, &str)> = matrix
            .iter()
            .filter(|(_, _, ok)| *ok)
            .map(|(profile, kind, _)| (*profile, *kind))
            .collect();
        assert_eq!(
            accepted,
            vec![
                ("perimeter-block", "residential_block"),
                ("low-density-residential", "storefront"),
                ("clustered-residential", "residential_block"),
                ("village-field", "storefront"),
            ],
            "matrix changed; re-derive the parcel/footprint coverage before updating"
        );
    }

    /// The two CLI controls are the same authored demand under two profiles.
    /// They must validate and compile without inventing or dropping demand.
    #[test]
    fn cli_control_manifests_compile_from_the_same_authored_demand() {
        let mut compiled = Vec::new();
        for (raw, expected) in [
            (
                include_str!(
                    "../../../../../examples/morphology/b04-c1-compact-mixed-block.manifest.json"
                ),
                "compact-mixed-block",
            ),
            (
                include_str!("../../../../../examples/morphology/b04-c1-village-field.manifest.json"),
                "village-field",
            ),
        ] {
            let manifest: Value = serde_json::from_str(raw).unwrap();
            let scenes: Vec<SceneSpec> =
                serde_json::from_value(manifest["scenes"].clone()).unwrap();
            let world: crate::worldgen::WorldManifest =
                serde_json::from_value(manifest.clone()).unwrap();
            world.validate().expect("control manifest validates");
            let compiled_style = compile(&manifest["style"], &scenes).unwrap().unwrap();
            assert_eq!(compiled_style.report.profile_id, expected);
            let requests = compiled_style.style["frontageProgram"]["requests"]
                .as_array()
                .unwrap();
            assert_eq!(requests.len(), 2);
            for (request, (count, kind)) in requests
                .iter()
                .zip([(16_u64, "residential_home"), (8, "storefront")])
            {
                assert_eq!(request["count"], json!(count));
                assert_eq!(request["kind"], json!(kind));
                assert_eq!(request["widthM"], json!(12));
                assert_eq!(request["depthM"], json!(10));
                assert!(
                    request["parcelInsets"].is_object(),
                    "the profile supplies the unstated parcel policy"
                );
            }
            // Hierarchy is injected because the control declares a v1 program
            // without tiers.
            assert_eq!(compiled_style.style["automaticRoadExtensions"]["version"], json!(2));
            let roles: Vec<&str> = compiled_style.style["automaticRoadExtensions"]["scenes"][0]
                ["hierarchy"]["tiers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tier| tier["role"].as_str().unwrap())
                .collect();
            if expected == "compact-mixed-block" {
                assert_eq!(roles, vec!["collector", "local"]);
            } else {
                assert_eq!(roles, vec!["collector", "local", "farm_access"]);
            }
            compiled.push((expected, compiled_style));
        }
        let (_, compact) = &compiled[0];
        let (_, village) = &compiled[1];
        assert_ne!(compact.style, village.style);
        assert_ne!(
            compact.style["frontageProgram"]["requests"][0]["parcelInsets"],
            village.style["frontageProgram"]["requests"][0]["parcelInsets"]
        );
        assert!(village.style["agricultureProgram"].is_object());
        assert!(compact.style.get("agricultureProgram").is_none());
        // Neither control declares a residual program, so both receive exactly
        // the profile's public-space share and no other ratio. The share still
        // follows the profile: the two controls disagree on it.
        for compiled in [compact, village] {
            let program = &compiled.style["residualLandProgram"];
            let scene = &program["scenes"]["scene-0"];
            assert_eq!(program["version"], json!(1));
            assert!(scene["urbanOpenSpaceRatio"].is_number());
            assert!(
                scene.get("ruralPastureRatio").is_none(),
                "a public-space share can never become a pasture share"
            );
            assert!(
                compiled
                    .report
                    .unsupported
                    .iter()
                    .all(|entry| entry.profile_field != "publicSpaceRatio")
            );
        }
        assert_ne!(
            compact.style["residualLandProgram"]["scenes"]["scene-0"]["urbanOpenSpaceRatio"],
            village.style["residualLandProgram"]["scenes"]["scene-0"]["urbanOpenSpaceRatio"]
        );
    }

    /// The legacy path must stay byte-identical, not merely equal, even when the
    /// manifest already declares the residual program B-04 now also compiles for
    /// profile manifests. A profile-free input is passed through untouched, so
    /// its serialized bytes keep this frozen blake3 digest. The public-space
    /// share a legacy manifest states works on its own, but no compiler default
    /// may ever leak into an old input.
    #[test]
    fn legacy_residual_program_bytes_are_frozen_without_a_profile() {
        let legacy = json!({
            "landUseProfile": land_use(0.10),
            "residualLandProgram": {
                "version": 1,
                "scenes": {"scene-0": {"urbanOpenSpaceRatio": 0.1, "ruralPastureRatio": 0.2}}
            }
        });
        let bytes = serde_json::to_vec(&legacy).unwrap();
        assert_eq!(
            blake3::hash(&bytes).to_hex().to_string(),
            "d591f8430591aafa2145032080479bdc097f6a1e70130fd8aa701a4fa042acc6",
            "the profile-free residual bytes changed; no profile path may touch a legacy input"
        );
        assert!(compile(&legacy, &[scene()]).unwrap().is_none());
        let effective = effective_style_or_original(&legacy, &[scene()]).unwrap();
        assert_eq!(serde_json::to_vec(&effective).unwrap(), bytes);
        // The legacy document is still consumed exactly as written.
        let program =
            crate::settlement::residual_land::for_scene(&effective, &scene()).unwrap().unwrap();
        assert_eq!(program.urban_open_space_ratio(), 0.1);
        assert_eq!(program.rural_pasture_ratio(), 0.2);
        assert!(program.unstated_ratios().is_empty());
    }

    /// T1: the compiler must not swallow keys the strict consumer rejects.
    /// Regressions here let a misspelled parameter fall back to a default.
    #[test]
    fn unknown_keys_reach_the_strict_consumer_and_are_rejected() {
        let cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
            (
                "frontageProgram container",
                Box::new(|value: &mut Value| {
                    value["frontageProgram"]["unexpectedField"] = json!(true)
                }),
            ),
            (
                "frontageProgram request",
                Box::new(|value: &mut Value| {
                    value["frontageProgram"]["requests"][0]["widhtM"] = json!(12)
                }),
            ),
            (
                "automaticRoadExtensions container",
                Box::new(|value: &mut Value| {
                    value["automaticRoadExtensions"]["unexpectedField"] = json!(true)
                }),
            ),
            (
                "automaticRoadExtensions scene",
                Box::new(|value: &mut Value| {
                    value["automaticRoadExtensions"]["scenes"][0]["unexpectedField"] = json!(true)
                }),
            ),
        ];
        for (label, mutate) in cases {
            let mut value = style("village-field");
            mutate(&mut value);
            assert!(
                compile(&value, &[scene()]).is_err(),
                "{label}: an unknown key must be rejected, not dropped"
            );
        }
        // Programs that are otherwise complete keep their unknown keys too.
        let mut residual = style("village-field");
        residual["residualLandProgram"] = json!({
            "version": 1,
            "scenes": {"scene-0": {"urbanOpenSpaceRatio": 0.1, "ruralPastureRatio": 0.2}}
        });
        assert!(compile(&residual, &[scene()]).is_ok());
        let mut bad = residual.clone();
        bad["residualLandProgram"]["unexpectedField"] = json!(true);
        assert!(compile(&bad, &[scene()]).is_err());
        let mut bad = residual.clone();
        bad["residualLandProgram"]["scenes"]["scene-0"]["unexpectedField"] = json!(true);
        assert!(compile(&bad, &[scene()]).is_err());

        let mut agriculture = style("village-field");
        agriculture["agricultureProgram"] = json!({
            "version": 1,
            "scenes": {"scene-0": {"defaultField": {"widthM": 24, "depthM": 24}}}
        });
        assert!(compile(&agriculture, &[scene()]).is_ok());
        let mut bad = agriculture.clone();
        bad["agricultureProgram"]["unexpectedField"] = json!(true);
        assert!(compile(&bad, &[scene()]).is_err());
        let mut bad = agriculture.clone();
        bad["agricultureProgram"]["scenes"]["scene-0"]["unexpectedField"] = json!(true);
        assert!(compile(&bad, &[scene()]).is_err());
    }

    /// T1: malformed containers, wrong types and wrong versions fail closed,
    /// while a legal omitted default still compiles.
    #[test]
    fn malformed_program_shapes_are_rejected_and_omissions_still_compile() {
        for (label, value) in [
            ("null program", json!(null)),
            ("array program", json!([])),
            ("string version", json!({"version": "1", "requests": []})),
            ("null version", json!({"version": null, "requests": []})),
            ("unsupported version", json!({"version": 2, "requests": []})),
            ("missing requests", json!({"version": 1})),
            ("request object", json!({"version": 1, "requests": [{}]})),
            (
                "string count",
                json!({"version": 1, "requests": [{
                    "id": "a", "sceneId": "scene-0", "purpose": "urban",
                    "kind": "storefront", "count": "4",
                    "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 64
                }]}),
            ),
        ] {
            let mut candidate = style("village-field");
            candidate["frontageProgram"] = value;
            assert!(
                compile(&candidate, &[scene()]).is_err(),
                "{label} must be rejected"
            );
        }
        // The legal omission path is exactly what the compiler exists for.
        let mut legal = json!({
            "morphologyProfile": "village-field",
            "landUseProfile": land_use(0.10),
            "frontageProgram": {"version": 1, "requests": [{
                "id": "shops", "sceneId": "scene-0", "purpose": "urban",
                "kind": "storefront", "count": 4,
                "stationSpacingM": 24, "setbackM": 12, "maxCandidateAttempts": 64
            }]}
        });
        assert!(compile(&legal, &[scene()]).unwrap().is_some());
        // ... and a legal omission is not mistaken for a missing mandatory key.
        legal["frontageProgram"]["requests"][0]
            .as_object_mut()
            .unwrap()
            .remove("count");
        assert!(compile(&legal, &[scene()])
            .unwrap_err()
            .contains("never allocates buildings"));
    }

    /// T3: a public-space share is not a pasture share. The profile fills the
    /// open-space share and nothing else; the pasture share stays unstated and
    /// the residual-land consumer applies its named default of zero.
    #[test]
    fn public_space_share_never_becomes_a_pasture_share() {
        // An explicit pasture share is preserved verbatim for every profile.
        let mut authored = Vec::new();
        for profile_id in ["compact-mixed-block", "village-field"] {
            let mut value = style(profile_id);
            value["residualLandProgram"] = json!({
                "version": 1,
                "scenes": {"scene-0": {"ruralPastureRatio": 0.4}}
            });
            let compiled = compile(&value, &[scene()]).unwrap().unwrap();
            let scene = &compiled.style["residualLandProgram"]["scenes"]["scene-0"];
            assert_eq!(scene["ruralPastureRatio"], json!(0.4), "{profile_id}");
            assert!(compiled.report.field_sources.iter().any(|entry| {
                entry.path.ends_with("ruralPastureRatio")
                    && entry.source == SOURCE_ORIGINAL
                    && entry.profile_field.is_empty()
            }));
            // The unstated urban share is the only field the profile fills.
            assert!(compiled.report.field_sources.iter().any(|entry| {
                entry.path.ends_with("urbanOpenSpaceRatio")
                    && entry.source == SOURCE_PROFILE
                    && entry.profile_field == "publicSpaceRatio"
            }));
            // The pasture share never appears as a profile-sourced value: no
            // field source may claim publicSpaceRatio wrote it.
            assert!(compiled
                .report
                .field_sources
                .iter()
                .filter(|entry| entry.path.ends_with("ruralPastureRatio"))
                .all(|entry| entry.source == SOURCE_ORIGINAL));
            authored.push((
                profile_id,
                scene["ruralPastureRatio"].clone(),
                scene["urbanOpenSpaceRatio"].clone(),
            ));
        }
        // Same authored pasture share, different profile: the pasture share is
        // unchanged while the open-space share follows the profile.
        assert_eq!(authored[0].1, authored[1].1);
        assert_ne!(authored[0].2, authored[1].2);

        // A declared program that states only the open-space share is complete.
        // The compiler states no pasture number; the consumer's named default
        // (0) is recorded in the report instead.
        let mut urban_only = style("village-field");
        urban_only["residualLandProgram"] = json!({
            "version": 1,
            "scenes": {"scene-0": {"urbanOpenSpaceRatio": 0.1}}
        });
        let urban_compiled = compile(&urban_only, &[scene()]).unwrap().unwrap();
        let entry = &urban_compiled.style["residualLandProgram"]["scenes"]["scene-0"];
        assert_eq!(entry["urbanOpenSpaceRatio"], json!(0.1), "authored share wins");
        assert!(
            entry.get("ruralPastureRatio").is_none(),
            "the compiler must not invent a pasture share"
        );
        assert!(urban_compiled.report.notes.iter().any(|note| note
            .contains("residualLandProgram.scenes.scene-0.ruralPastureRatio is unstated")));
        assert!(urban_compiled
            .report
            .unsupported
            .iter()
            .all(|entry| entry.profile_field != "publicSpaceRatio"));

        // No program at all: the profile supplies its public-space share and
        // still no pasture share, so the same named default applies.
        let compiled = compiled("village-field");
        let program = &compiled.style["residualLandProgram"];
        assert_eq!(program["scenes"]["scene-0"], json!({"urbanOpenSpaceRatio": 0.14}));
        assert!(program["scenes"]["scene-0"].get("ruralPastureRatio").is_none());
        // Every scene that owns an urban remainder receives the same share, in
        // manifest-independent key order and with no other ratio.
        let second_scene = SceneSpec {
            scene_id: "scene-1".into(),
            width_m: 512,
            depth_m: 512,
            origin_x: 1024,
            origin_z: 0,
            seed_offset: 0,
        };
        let two_scenes = compile(&style("village-field"), &[scene(), second_scene])
            .unwrap()
            .unwrap();
        assert_eq!(
            two_scenes.style["residualLandProgram"],
            json!({
                "version": 1,
                "scenes": {
                    "scene-0": {"urbanOpenSpaceRatio": 0.14},
                    "scene-1": {"urbanOpenSpaceRatio": 0.14}
                }
            }),
            "every urban scene receives exactly the open-space share"
        );
        // A profile whose land-use target leaves no urban zone gets no program:
        // the compiler never states a policy a scene cannot use, and it never
        // emits a document with zero scenes.
        let mut no_urban = style("village-field");
        no_urban["landUseProfile"]["urbanCoreRatio"] = json!(0.0);
        no_urban["landUseProfile"]["suburbanRatio"] = json!(0.0);
        let compiled = compile(&no_urban, &[scene()]).unwrap().unwrap();
        assert!(compiled.style.get("residualLandProgram").is_none());
        assert!(compiled.report.unsupported.iter().any(|entry| {
            entry.profile_field == "publicSpaceRatio"
                && entry.source == SOURCE_UNSUPPORTED
                && entry.detail.contains("no scene owns an urban remainder")
        }));
    }
}
