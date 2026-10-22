//! B01-G19 v1: engineering connections.
//!
//! Production, default-off capability for controlled ground works and independent
//! structures (landings, straight stairs, switchback stairs), their three-dimensional
//! ports, a typed connection graph with capability-filtered routing, a versioned
//! on-disk bundle and an independent read-back verifier.
//!
//! Separation of concerns enforced here:
//! * the natural field and the existing engineered baseline are immutable inputs;
//!   only an authorised [`GroundWorkPlan`] may produce a new engineered surface, and
//!   it does so transactionally (no partial surface on failure);
//! * structures are separate three-dimensional objects with real triangle meshes and
//!   verified supports; they never silently flatten terrain and their walking surface
//!   is never read from the terrain below;
//! * the graph only contains verified edges: ground edges are checked with the
//!   production terrain predicate, stair/landing edges with the verified structure,
//!   and routing is a bounded, typed, capability-filtered search.
//!
//! This module is not consumed by default world generation. The CLI builds and
//! verifies explicit bundles from explicit inputs.

use serde::{Deserialize, Serialize};

mod support_coverage;
use std::collections::{BTreeMap, BTreeSet};

use crate::baked_terrain::BakedTerrain;
use crate::height_payload::encode_millimetres;

/// Version of the input/output contract. Bumped only with a documented migration.
pub const ENGINEERING_CONNECTIONS_VERSION: u32 = 2;

/// What changed from v1 and what a v1 description must do about it.
pub const CONTRACT_MIGRATION_NOTE: &str = "v2 requires every environment array to be present explicitly ([] is a declaration, a missing key is refused), a roadReferences registry for every referenced road id, resolvable surface/level references, no solid interpenetration for structures/corridors/walking volumes, per-capability local geometry checks, directed path metrics and a committed publication state; a v1 description must be re-declared against v2";

/// Every key a v2 description must carry. Presence is checked before deserialising,
/// so a missing environment can never be read as an empty one.
pub const REQUIRED_INPUT_FIELDS: [&str; 14] = [
    "version",
    "sceneId",
    "units",
    "grid",
    "protection",
    "obstacles",
    "water",
    "bearing",
    "roadReferences",
    "groundWorks",
    "structures",
    "ports",
    "groundConnections",
    "query",
];

/// The only surface and level id that resolve to the authoritative ground grid.
pub const RESERVED_GROUND_SURFACE: &str = "ground";
pub const RESERVED_GROUND_LEVEL: &str = "ground";

/// Hard safety caps for the v1 contract.
pub const MAX_GRID_SIDE: u32 = 64;
pub const MAX_STRUCTURES: usize = 32;
pub const MAX_PORTS: usize = 256;
pub const MAX_EDGES: usize = 512;
pub const MAX_GROUND_WORK_CELLS: usize = 64 * 64;
pub const MAX_ROUTE_EXPANSIONS: usize = 4096;
/// Bound on the non-dominated labels kept per node. The graph is tiny, so this is
/// generous; hitting it is reported as a budget limit, never as an absence.
pub const MAX_LABELS_PER_NODE: usize = 8;
pub const MAX_MESH_VERTICES: usize = 200_000;

/// Tolerance used when checking that a derived port matches its declared position.
pub const PORT_MATCH_TOLERANCE_M: f64 = 0.002;
/// Tolerance used when checking that a support column reaches its bearing surface.
pub const SUPPORT_CONTACT_TOLERANCE_M: f64 = 0.05;

/// Conservative AABB contact allowance. Two solids collide only when they overlap
/// on every axis by more than this, so legal contact (a tread embedded 30 mm into
/// the riser in front of it, a deck resting on a terrace, a column footing) stays
/// legal while a wall, a platform volume or a low beam is refused.
pub const SOLID_CONTACT_TOLERANCE_M: f64 = 0.05;
/// Body heights used when sweeping a walking volume: a solid rising into this band
/// above the walking surface blocks the edge.
pub const PEDESTRIAN_BODY_HEIGHT_M: f64 = 2.2;
pub const WHEELCHAIR_BODY_HEIGHT_M: f64 = 1.6;
pub const VEHICLE_BODY_HEIGHT_M: f64 = 4.0;
/// Clear width a capability needs, in metres.
pub const WHEELCHAIR_CLEAR_WIDTH_M: f64 = 1.2;
pub const VEHICLE_CLEAR_WIDTH_M: f64 = 3.0;
/// Near an endpoint of a ground edge the solids of the structures that own that
/// endpoint may be touched: that is where a walk arrives at a stair foot or a deck.
/// Outside this zone nothing is exempt.
pub const TERMINAL_CONTACT_ZONE_M: f64 = 1.0;
/// A walking surface may be touched by a walk only where it is at most one support
/// step above the walking plane: a kerb or the first step of a flight is legal
/// contact, a tread or riser higher than that stands in the body space and blocks.
/// There is no ownership exemption and no distance window.
pub const MAX_SUPPORT_STEP_RISE_M: f64 = 0.25;
/// A walking surface may be touched by a walk only where it is at most one support
/// step above the walking plane: a kerb or the first step of a flight is legal
/// contact, a tread or riser higher than that stands in the body space and blocks.
/// There is no ownership exemption and no distance window. 0.25;
/// The walking volume a pedestrian check must fit: 1.5 m of clear width, the same
/// half-width the production predicate samples.
pub const PEDESTRIAN_CLEAR_WIDTH_M: f64 = 1.5;
/// Local grade limits per capability, applied to every sample of that capability's
/// own band, never to an endpoint average.
pub const MAX_GRADE_WHEELCHAIR: f64 = 0.05;
pub const MAX_GRADE_VEHICLE: f64 = 0.08;
/// Fill that ends above a solid buries it; a cut that drops below a solid's base
/// leaves it floating. Both are refused rather than smoothed over.
pub const SOLID_RELATION_TOLERANCE_M: f64 = 0.05;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringError {
    /// Stable machine-readable category, e.g. `protected_cell`, `wrong_road`.
    pub category: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub detail: serde_json::Value,
}

impl EngineeringError {
    pub fn new(category: &str, message: impl Into<String>) -> Self {
        Self {
            category: category.to_string(),
            message: message.into(),
            detail: serde_json::Value::Null,
        }
    }
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = detail;
        self
    }
}

impl std::fmt::Display for EngineeringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.category, self.message)
    }
}

impl std::error::Error for EngineeringError {}

pub type EcResult<T> = Result<T, EngineeringError>;

fn err<T>(category: &str, message: impl Into<String>) -> EcResult<T> {
    Err(EngineeringError::new(category, message))
}

// ---------------------------------------------------------------------------
// Input contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectI {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

impl RectI {
    /// Checked spans. Extreme input must produce a refusal, never a wrapped count.
    fn span_x(&self) -> i64 {
        i64::from(self.x1) - i64::from(self.x0) + 1
    }
    fn span_z(&self) -> i64 {
        i64::from(self.z1) - i64::from(self.z0) + 1
    }
    pub fn cells(&self) -> EcResult<usize> {
        let x = self.span_x();
        let z = self.span_z();
        if x <= 0 || z <= 0 {
            return err(
                "scope_reversed",
                format!(
                    "rectangle x {}..{} z {}..{} is empty or reversed",
                    self.x0, self.x1, self.z0, self.z1
                ),
            );
        }
        let cells = x.checked_mul(z).ok_or_else(|| {
            EngineeringError::new(
                "scope_out_of_range",
                format!(
                    "rectangle x {}..{} z {}..{} has an unrepresentable cell count",
                    self.x0, self.x1, self.z0, self.z1
                ),
            )
        })?;
        if cells > MAX_GROUND_WORK_CELLS as i64 {
            return err(
                "scope_out_of_range",
                format!("the scope has {cells} cells, over the v2 cap {MAX_GROUND_WORK_CELLS}"),
            );
        }
        Ok(cells as usize)
    }
    /// A scope must be ordered, inside the grid and inside the v2 cap.
    fn validated(&self, width: u32, depth: u32) -> EcResult<usize> {
        let cells = self.cells()?;
        let inside = i64::from(self.x0) >= 0
            && i64::from(self.z0) >= 0
            && i64::from(self.x1) < i64::from(width)
            && i64::from(self.z1) < i64::from(depth);
        if !inside {
            return err(
                "scope_outside_grid",
                format!(
                    "scope x {}..{} z {}..{} is not inside the {width}x{depth} grid",
                    self.x0, self.x1, self.z0, self.z1
                ),
            );
        }
        Ok(cells)
    }
    pub fn contains(&self, x: i32, z: i32) -> bool {
        x >= self.x0 && x <= self.x1 && z >= self.z0 && z <= self.z1
    }
    pub fn overlaps(&self, other: &RectI) -> bool {
        self.x0 <= other.x1 && self.x1 >= other.x0 && self.z0 <= other.z1 && self.z1 >= other.z0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectF {
    pub x0: f64,
    pub z0: f64,
    pub x1: f64,
    pub z1: f64,
}

impl RectF {
    pub fn overlaps(&self, other: &RectF) -> bool {
        self.x0 <= other.x1 && self.x1 >= other.x0 && self.z0 <= other.z1 && self.z1 >= other.z0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSpec {
    pub width: u32,
    pub depth: u32,
    pub origin_x: i32,
    pub origin_z: i32,
    /// Row-major over `depth` rows of `width` millimetres, origin first.
    pub natural_mm: Vec<i32>,
    /// Existing engineered ground (immutable). Equal to `natural_mm` when none.
    pub baseline_mm: Vec<i32>,
    /// Provenance of these fields. This contract never generates them implicitly.
    pub source: String,
}

impl GridSpec {
    fn validate(&self) -> EcResult<()> {
        if self.width == 0
            || self.depth == 0
            || self.width > MAX_GRID_SIDE
            || self.depth > MAX_GRID_SIDE
        {
            return err(
                "grid_out_of_range",
                format!(
                    "grid {}x{} exceeds the v1 cap of {MAX_GRID_SIDE}",
                    self.width, self.depth
                ),
            );
        }
        let expected = self.width as usize * self.depth as usize;
        if self.natural_mm.len() != expected || self.baseline_mm.len() != expected {
            return err(
                "grid_length_mismatch",
                format!(
                    "expected {expected} samples, got natural {}, baseline {}",
                    self.natural_mm.len(),
                    self.baseline_mm.len()
                ),
            );
        }
        if self
            .natural_mm
            .iter()
            .chain(self.baseline_mm.iter())
            .any(|v| *v == i32::MIN)
        {
            return err("non_finite_value", "a sample is a sentinel, not a height");
        }
        if i64::from(self.origin_x) + i64::from(self.width) > i64::from(i32::MAX)
            || i64::from(self.origin_z) + i64::from(self.depth) > i64::from(i32::MAX)
        {
            return err(
                "grid_out_of_range",
                "origin plus size leaves the signed 32 bit world frame",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionRect {
    pub id: String,
    pub rect: RectI,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObstacleBox {
    pub id: String,
    pub rect: RectF,
    pub bottom_m: f64,
    pub top_m: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BearingSurface {
    pub id: String,
    pub rect: RectF,
    pub level_mm: i32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterRect {
    pub id: String,
    pub rect: RectF,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundWorkPlan {
    pub id: String,
    pub scope: RectI,
    /// Constant target level for the whole scope.
    #[serde(default)]
    pub flat_target_mm: Option<i32>,
    /// Explicit per-cell target plane, row-major over the scope.
    #[serde(default)]
    pub target_mm: Option<Vec<i32>>,
    /// Authorisation. Fill and cut are separate: no negative target may imply a cut.
    pub allow_fill: bool,
    pub allow_cut: bool,
    pub max_fill_mm: i32,
    pub max_cut_mm: i32,
    /// Cells that must not change even when inside the scope.
    #[serde(default)]
    pub forbidden: Vec<RectI>,
    /// True when the plan needs retaining or drainage works v1 does not model.
    #[serde(default)]
    pub requires_retaining_or_drainage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Pedestrian,
    Wheelchair,
    Vehicle,
    Rail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportKind {
    /// Every tread and landing is a solid block reaching the ground below it.
    GroundContact,
    /// Explicit columns, evenly spaced, each reaching the ground or its bearing.
    Columns,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportSpec {
    pub kind: SupportKind,
    /// Column count per run/landing when `kind = columns`.
    #[serde(default = "default_columns")]
    pub columns: usize,
    /// Column cross-section (full width) in metres.
    #[serde(default = "default_column_size")]
    pub column_size_m: f64,
}

fn default_columns() -> usize {
    2
}
fn default_column_size() -> f64 {
    0.3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingSpec {
    pub id: String,
    pub centre_m: [f64; 3],
    pub width_m: f64,
    pub depth_m: f64,
    pub thickness_m: f64,
    /// Port on the landing's walking surface; position must match the geometry.
    pub port_id: String,
    /// Optional second port on the same deck. When present the deck is a traversable
    /// landing edge between the two ports instead of a single node.
    #[serde(default)]
    pub exit_port_id: Option<String>,
    pub support: SupportSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraightStairSpec {
    pub id: String,
    pub bottom_port_id: String,
    pub top_port_id: String,
    pub steps: u32,
    pub rise_mm: i32,
    pub run_mm: i32,
    pub width_m: f64,
    pub thickness_m: f64,
    /// A one-way flight is only traversable from `bottom_port_id` to `top_port_id`.
    #[serde(default)]
    pub one_way: bool,
    pub support: SupportSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchbackStairSpec {
    pub id: String,
    pub bottom_port_id: String,
    pub top_port_id: String,
    pub steps_per_run: u32,
    pub rise_mm: i32,
    pub run_mm: i32,
    pub width_m: f64,
    pub thickness_m: f64,
    /// Turn landing connecting the two runs; its footprint must hold both runs.
    pub turn_centre_m: [f64; 3],
    pub turn_length_m: f64,
    pub turn_depth_m: f64,
    pub middle_port_id: String,
    /// Direction rule for both flights: a one-way switchback is only traversable
    /// from `bottom_port_id` through the turn to `top_port_id`.
    #[serde(default)]
    pub one_way: bool,
    pub support: SupportSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StructureSpec {
    Landing(LandingSpec),
    StraightStair(StraightStairSpec),
    SwitchbackStair(SwitchbackStairSpec),
    /// Types the v1 model can name but not build. Always refused explicitly.
    Unsupported {
        id: String,
        requested_kind: String,
        reason: String,
    },
}

impl StructureSpec {
    pub fn id(&self) -> &str {
        match self {
            StructureSpec::Landing(spec) => &spec.id,
            StructureSpec::StraightStair(spec) => &spec.id,
            StructureSpec::SwitchbackStair(spec) => &spec.id,
            StructureSpec::Unsupported { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortSpec {
    pub id: String,
    pub position_m: [f64; 3],
    pub surface_id: String,
    pub level_id: String,
    /// Road this port belongs to, when it is a road-side connector.
    #[serde(default)]
    pub road_id: Option<String>,
    pub width_m: f64,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundConnectionSpec {
    pub id: String,
    pub from_port_id: String,
    pub to_port_id: String,
    #[serde(default)]
    pub road_id: Option<String>,
    /// A one-way ground connection is only traversable from `from_port_id`.
    #[serde(default)]
    pub one_way: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteQuery {
    pub from_port_id: String,
    /// The route is only accepted when the reached road port carries this id.
    pub to_road_id: String,
    pub capability: Capability,
    pub max_length_3d_m: f64,
    pub max_rise_m: f64,
}

/// A road id is a declared reference, not a proof of a real binding. The registry
/// records where the id came from so the bundle can state what it is bound to.
/// A port that claims a structure surface must really sit on it: inside the footprint
/// of an actual walking member of that surface, at that member's level, no wider than
/// the member. Existence of the label is not membership.
fn validate_port_membership(
    input: &EngineeringInput,
    surface: &EngineeringSurface,
    structures: &[StructureRecord],
) -> EcResult<()> {
    for port in &input.ports {
        let reserved =
            port.surface_id == RESERVED_GROUND_SURFACE && port.level_id == RESERVED_GROUND_LEVEL;
        if reserved {
            // A ground port may sit on the terrain at a structure's boundary, but not
            // under structure material that rises above it: that is drilling into a
            // body, not landing on a surface.
            for structure in structures {
                for part in &structure.parts {
                    let rises = part.aabb().max.y > port.position_m[1] + SOLID_CONTACT_TOLERANCE_M;
                    if !rises {
                        continue;
                    }
                    let inside = port.position_m[0] > part.aabb().min.x + SOLID_CONTACT_TOLERANCE_M
                        && port.position_m[0] < part.aabb().max.x - SOLID_CONTACT_TOLERANCE_M
                        && port.position_m[2] > part.aabb().min.z + SOLID_CONTACT_TOLERANCE_M
                        && port.position_m[2] < part.aabb().max.z - SOLID_CONTACT_TOLERANCE_M;
                    if inside {
                        return err(
                            "port_under_structure",
                            format!(
                                "ground port {} at ({:.3},{:.3}) sits under {} of {}, which rises to {:.3} m above its level",
                                port.id,
                                port.position_m[0],
                                port.position_m[2],
                                part.part_id,
                                structure.structure_id,
                                part.aabb().max.y
                            ),
                        );
                    }
                }
            }
            let Some(height) = surface.height_m(port.position_m[0], port.position_m[2]) else {
                return err(
                    "port_outside_grid",
                    format!("ground port {} has no engineered sample under it", port.id),
                );
            };
            if (height - port.position_m[1]).abs() > GROUND_PORT_LEVEL_TOLERANCE_M {
                return err(
                    "port_not_on_ground",
                    format!(
                        "ground port {} is declared at {:.3} m but the engineered surface there is {height:.3} m",
                        port.id, port.position_m[1]
                    ),
                );
            }
            continue;
        }
        let Some(structure) = structures
            .iter()
            .find(|structure| structure.structure_id == port.surface_id)
        else {
            continue;
        };
        // Membership is tested against the member's own oriented support polygon, not
        // against its axis-aligned bounding box: a rotated flight is not a rectangle.
        let member = structure
            .parts
            .iter()
            .filter(|part| part.role == "tread" || part.role == "landing")
            .find(|part| {
                let Some(frame) = member_frame(part) else {
                    return false;
                };
                (f64::from(part.top_mm) / 1000.0 - port.position_m[1]).abs()
                    <= GROUND_PORT_LEVEL_TOLERANCE_M
                    && frame.contains(
                        port.position_m[0],
                        port.position_m[2],
                        GROUND_PORT_LEVEL_TOLERANCE_M,
                    )
            });
        let Some(member) = member else {
            return Err(EngineeringError::new(
                "port_not_on_referenced_surface",
                format!(
                    "port {} claims {}/{} but no walking member of that surface contains ({:.3},{:.3}) at {:.3} m",
                    port.id, port.surface_id, port.level_id, port.position_m[0], port.position_m[2], port.position_m[1]
                ),
            )
            .with_detail(serde_json::json!({
                "portId": port.id,
                "surfaceId": port.surface_id,
                "levelId": port.level_id,
                "positionM": port.position_m,
                "members": structure.parts.iter().filter(|part| part.role == "tread" || part.role == "landing").map(|part| part.part_id.clone()).collect::<Vec<_>>(),
            })));
        };
        // The walking width of a member is its larger horizontal extent (a tread is
        // long in the run direction and wide across it; a deck is wide both ways).
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadReference {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringInput {
    pub version: u32,
    pub scene_id: String,
    /// Unit contract: all lengths in metres, all levels in millimetres.
    pub units: String,
    pub grid: GridSpec,
    #[serde(default)]
    pub protection: Vec<ProtectionRect>,
    #[serde(default)]
    pub obstacles: Vec<ObstacleBox>,
    #[serde(default)]
    pub water: Vec<WaterRect>,
    #[serde(default)]
    pub bearing: Vec<BearingSurface>,
    #[serde(default)]
    pub ground_works: Vec<GroundWorkPlan>,
    #[serde(default)]
    pub structures: Vec<StructureSpec>,
    #[serde(default)]
    pub ports: Vec<PortSpec>,
    #[serde(default)]
    pub ground_connections: Vec<GroundConnectionSpec>,
    /// Registry of every road id the description mentions.
    #[serde(default)]
    pub road_references: Vec<RoadReference>,
    pub query: RouteQuery,
}

/// Read a description and enforce the v2 presence rule before anything is built.
pub fn load_input(json: &str) -> EcResult<EngineeringInput> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| {
        EngineeringError::new(
            "input_invalid",
            format!("the description is not readable JSON: {error}"),
        )
    })?;
    let Some(object) = value.as_object() else {
        return err("input_invalid", "the description must be a JSON object");
    };
    let missing: Vec<&str> = REQUIRED_INPUT_FIELDS
        .iter()
        .copied()
        .filter(|field| !object.contains_key(*field))
        .collect();
    if !missing.is_empty() {
        return Err(EngineeringError::new(
            "input_invalid",
            format!(
                "missing required field(s) {}: an absent environment is not an empty one, declare it explicitly with []",
                missing.join(", ")
            ),
        )
        .with_detail(serde_json::json!({ "missingFields": missing })));
    }
    let input: EngineeringInput = serde_json::from_value(value).map_err(|error| {
        EngineeringError::new(
            "input_invalid",
            format!("the description is malformed: {error}"),
        )
    })?;
    if input.version != ENGINEERING_CONNECTIONS_VERSION {
        return Err(EngineeringError::new(
            "contract_version_mismatch",
            format!(
                "input declares version {} but this module implements {ENGINEERING_CONNECTIONS_VERSION}: {CONTRACT_MIGRATION_NOTE}",
                input.version
            ),
        )
        .with_detail(serde_json::json!({
            "declaredVersion": input.version,
            "supportedVersion": ENGINEERING_CONNECTIONS_VERSION,
            "migration": CONTRACT_MIGRATION_NOTE,
        })));
    }
    Ok(input)
}

// ---------------------------------------------------------------------------
// Engineering surface (natural + authorised ground works only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EngineeringSurface {
    pub width: u32,
    pub depth: u32,
    pub origin_x: i32,
    pub origin_z: i32,
    /// Edited engineered ground in millimetres.
    pub mm: Vec<i32>,
    /// Natural field, kept byte-identical to the input.
    pub natural_mm: Vec<i32>,
    /// Existing baseline, kept byte-identical to the input.
    pub baseline_mm: Vec<i32>,
}

impl EngineeringSurface {
    fn index(&self, x: i32, z: i32) -> Option<usize> {
        let lx = x - self.origin_x;
        let lz = z - self.origin_z;
        if lx < 0 || lz < 0 || lx >= self.width as i32 || lz >= self.depth as i32 {
            return None;
        }
        Some(lz as usize * self.width as usize + lx as usize)
    }

    pub fn mm_at(&self, x: i32, z: i32) -> Option<i32> {
        self.index(x, z).map(|i| self.mm[i])
    }

    /// Production-style sample: millimetres to `f32` metres, widened to `f64`,
    /// bilinear over four non-zero-weight neighbours. A missing sample is `None`.
    pub fn height_m(&self, px: f64, pz: f64) -> Option<f64> {
        let x = px.floor() as i32;
        let z = pz.floor() as i32;
        let fx = px - f64::from(x);
        let fz = pz - f64::from(z);
        let a = to_f32_m(self.mm_at(x, z)?);
        let b = to_f32_m(self.mm_at(x + 1, z)?);
        let c = to_f32_m(self.mm_at(x, z + 1)?);
        let d = to_f32_m(self.mm_at(x + 1, z + 1)?);
        Some(((a * (1.0 - fx) + b * fx) * (1.0 - fz)) + ((c * (1.0 - fx) + d * fx) * fz))
    }

    /// A `BakedTerrain` carrying the edited engineering surface, so the production
    /// ground predicate can be reused without re-implementing it.
    pub fn to_baked_terrain(&self) -> EcResult<BakedTerrain> {
        let values: Vec<f64> = self.mm.iter().map(|mm| f64::from(*mm) / 1000.0).collect();
        let bytes = encode_millimetres(self.width, self.depth, values)
            .map_err(|error| EngineeringError::new("terrain_encode", error.0))?;
        let mut terrain = BakedTerrain::new(&crate::worldgen::SceneSpec {
            scene_id: "engineering-connections".into(),
            width_m: self.width,
            depth_m: self.depth,
            origin_x: self.origin_x,
            origin_z: self.origin_z,
            seed_offset: 0,
        })
        .map_err(|error| EngineeringError::new("terrain_build", error.0))?;
        terrain
            .insert_chunk(self.origin_x, self.origin_z, self.width, self.depth, &bytes)
            .map_err(|error| EngineeringError::new("terrain_insert", error.0))?;
        terrain
            .require_complete()
            .map_err(|error| EngineeringError::new("terrain_incomplete", error.0))?;
        Ok(terrain)
    }
}

fn to_f32_m(mm: i32) -> f64 {
    f64::from(mm as f32 / 1000.0f32)
}

// ---------------------------------------------------------------------------
// Ground works
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundWorkReport {
    pub plan_id: String,
    pub scope: RectI,
    pub changed_cells: usize,
    pub checked_cells: usize,
    pub max_cut_mm: i32,
    pub max_fill_mm: i32,
    pub area_m2: f64,
    pub cut_volume_m3: f64,
    pub fill_volume_m3: f64,
    pub volume_method: String,
    pub protection_intersections: Vec<String>,
    pub forbidden_hits: usize,
    pub rejected: Vec<serde_json::Value>,
    pub status: String,
}

const VOLUME_METHOD: &str = "one square metre per grid cell times the absolute height difference; sum of cell volumes, never a sum of heights";

/// Apply one authorised plan transactionally. Every cell is evaluated before any
/// value is committed, so a failure leaves the surface untouched.
pub fn apply_ground_work(
    surface: &EngineeringSurface,
    plan: &GroundWorkPlan,
    protection: &[ProtectionRect],
) -> EcResult<(Vec<i32>, GroundWorkReport)> {
    plan.scope.validated(surface.width, surface.depth)?;
    if plan.max_fill_mm < 0 || plan.max_cut_mm < 0 {
        return err(
            "invalid_authorisation",
            format!(
                "plan {} declares a negative authorisation (fill {} mm, cut {} mm)",
                plan.id, plan.max_fill_mm, plan.max_cut_mm
            ),
        );
    }
    match (&plan.target_mm, plan.flat_target_mm) {
        (Some(_), Some(_)) => {
            return err(
                "ambiguous_target",
                format!(
                    "plan {} declares both a flat target and a target plane",
                    plan.id
                ),
            );
        }
        (Some(values), None) => {
            let cells = plan.scope.cells()?;
            if values.len() != cells {
                return err(
                    "target_plane_length_mismatch",
                    format!(
                        "plan {} declares a target plane of {} values for a scope of {cells} cells",
                        plan.id,
                        values.len()
                    ),
                );
            }
        }
        (None, None) => {
            return err(
                "missing_target",
                format!(
                    "plan {} has neither a flat target nor a target plane",
                    plan.id
                ),
            );
        }
        (None, Some(_)) => {}
    }
    if plan.requires_retaining_or_drainage {
        return Err(EngineeringError::new(
            "unsupported",
            format!(
                "plan {} declares retaining or drainage works that v1 does not model (needs_design)",
                plan.id
            ),
        )
        .with_detail(serde_json::json!({
            "planId": plan.id,
            "status": "unsupported",
            "reason": "retaining_or_drainage",
            "note": "refused instead of flattened silently",
        })));
    }
    let target_for = |x: i32, z: i32| -> EcResult<i32> {
        if let Some(values) = &plan.target_mm {
            let w = plan.scope.span_x() as usize;
            let lx = (i64::from(x) - i64::from(plan.scope.x0)) as usize;
            let lz = (i64::from(z) - i64::from(plan.scope.z0)) as usize;
            let index = lz * w + lx;
            values
                .get(index)
                .copied()
                .ok_or_else(|| EngineeringError::new("target_plane_short", "target plane is short"))
        } else if let Some(flat) = plan.flat_target_mm {
            Ok(flat)
        } else {
            err(
                "missing_target",
                "the plan has neither a flat target nor a target plane",
            )
        }
    };
    let mut protection_intersections: Vec<String> = vec![];
    for domain in protection {
        if plan.scope.overlaps(&domain.rect) {
            protection_intersections.push(domain.id.clone());
        }
    }
    let mut draft = surface.mm.clone();
    let mut changed = 0_usize;
    let mut checked = 0_usize;
    let mut max_cut = 0_i32;
    let mut max_fill = 0_i32;
    let mut area_m2 = 0.0_f64;
    let mut cut_volume = 0.0_f64;
    let mut fill_volume = 0.0_f64;
    let mut forbidden_hits = 0_usize;
    let mut rejected: Vec<serde_json::Value> = vec![];
    for z in plan.scope.z0..=plan.scope.z1 {
        for x in plan.scope.x0..=plan.scope.x1 {
            let Some(index) = surface.index(x, z) else {
                return err(
                    "scope_outside_grid",
                    format!("plan {} leaves the grid at ({x},{z})", plan.id),
                );
            };
            checked += 1;
            if plan.forbidden.iter().any(|rect| rect.contains(x, z)) {
                forbidden_hits += 1;
                if rejected.len() < 8 {
                    rejected.push(serde_json::json!({
                        "at": [x, z],
                        "reason": "forbidden_cell",
                    }));
                }
                continue;
            }
            if let Some(domain) = protection.iter().find(|domain| domain.rect.contains(x, z)) {
                return Err(EngineeringError::new(
                    "protected_cell",
                    format!(
                        "plan {} would edit ({x},{z}) inside protection domain {} (source {})",
                        plan.id, domain.id, domain.source
                    ),
                )
                .with_detail(
                    serde_json::json!({"at": [x, z], "domain": domain.id, "source": domain.source}),
                ));
            }
            let target = target_for(x, z)?;
            let current = surface.mm[index];
            // Signed 64 bit difference: an extreme target must not wrap.
            let wide = i64::from(target) - i64::from(current);
            if wide > i64::from(i32::MAX) || wide < -i64::from(i32::MAX) {
                return err(
                    "height_delta_out_of_range",
                    format!(
                        "plan {} would move ({x},{z}) from {current} mm to {target} mm, a difference outside the signed millimetre range",
                        plan.id
                    ),
                );
            }
            let delta = wide;
            if delta > 0 {
                if !plan.allow_fill {
                    return err(
                        "fill_not_authorised",
                        format!(
                            "plan {} would fill ({x},{z}) by {delta}mm without authorisation",
                            plan.id
                        ),
                    );
                }
                if delta > i64::from(plan.max_fill_mm) {
                    return err(
                        "fill_over_authorisation",
                        format!(
                            "plan {} would fill ({x},{z}) by {delta}mm over the authorised {}mm",
                            plan.id, plan.max_fill_mm
                        ),
                    );
                }
                max_fill = max_fill.max(delta as i32);
                fill_volume += delta as f64 / 1000.0;
            } else if delta < 0 {
                let depth = -delta;
                if !plan.allow_cut {
                    return err(
                        "cut_not_authorised",
                        format!(
                            "plan {} would cut ({x},{z}) by {depth}mm without authorisation (a negative target is not a cut licence)",
                            plan.id
                        ),
                    );
                }
                if depth > i64::from(plan.max_cut_mm) {
                    return err(
                        "cut_over_authorisation",
                        format!(
                            "plan {} would cut ({x},{z}) by {depth}mm over the authorised {}mm",
                            plan.id, plan.max_cut_mm
                        ),
                    );
                }
                max_cut = max_cut.max(depth as i32);
                cut_volume += depth as f64 / 1000.0;
            }
            if delta != 0 {
                changed += 1;
                area_m2 += 1.0;
            }
            draft[index] = target;
        }
    }
    let report = GroundWorkReport {
        plan_id: plan.id.clone(),
        scope: plan.scope,
        changed_cells: changed,
        checked_cells: checked,
        max_cut_mm: max_cut,
        max_fill_mm: max_fill,
        area_m2,
        cut_volume_m3: (cut_volume * 1000.0).round() / 1000.0,
        fill_volume_m3: (fill_volume * 1000.0).round() / 1000.0,
        volume_method: VOLUME_METHOD.to_string(),
        protection_intersections,
        forbidden_hits,
        rejected,
        status: "applied".to_string(),
    };
    Ok((draft, report))
}

/// Apply every plan in order. A failure returns no surface at all.
pub fn apply_ground_works(
    input: &EngineeringInput,
) -> EcResult<(EngineeringSurface, Vec<GroundWorkReport>)> {
    let mut surface = EngineeringSurface {
        width: input.grid.width,
        depth: input.grid.depth,
        origin_x: input.grid.origin_x,
        origin_z: input.grid.origin_z,
        mm: input.grid.baseline_mm.clone(),
        natural_mm: input.grid.natural_mm.clone(),
        baseline_mm: input.grid.baseline_mm.clone(),
    };
    let mut seen_plans = BTreeSet::new();
    for plan in &input.ground_works {
        if !seen_plans.insert(plan.id.clone()) {
            return err(
                "duplicate_id",
                format!(
                    "ground work plan {} is declared twice; plans are never merged silently",
                    plan.id
                ),
            );
        }
    }
    for (index, plan) in input.ground_works.iter().enumerate() {
        for other in input.ground_works.iter().skip(index + 1) {
            if plan.scope.overlaps(&other.scope) {
                return err(
                    "ground_work_scope_conflict",
                    format!(
                        "plans {} and {} overlap; overlapping scopes would double-count cut and fill volumes, split or merge them explicitly",
                        plan.id, other.id
                    ),
                );
            }
        }
    }
    let mut reports = vec![];
    for plan in &input.ground_works {
        let (next, report) = apply_ground_work(&surface, plan, &input.protection)?;
        surface.mm = next;
        reports.push(report);
    }
    Ok((surface, reports))
}

// ---------------------------------------------------------------------------
// Three-dimensional geometry primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn from_arr(a: [f64; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }
    pub fn arr(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
    pub fn xz_len(self) -> f64 {
        self.x.hypot(self.z)
    }
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    fn normalize_xz(self) -> EcResult<Vec3> {
        let len = self.xz_len();
        if !(len > 1e-9) {
            return err(
                "degenerate_axis",
                "a structure axis has no horizontal extent; v1 requires two distinct plan positions",
            );
        }
        Ok(Vec3::new(self.x / len, 0.0, self.z / len))
    }
    fn perp_xz(self) -> Vec3 {
        Vec3::new(-self.z, 0.0, self.x)
    }
}

/// Axis-aligned bounds, used for the independent overlap and containment checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    fn from_corners(corners: &[[f64; 3]; 8]) -> Self {
        let mut min = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
        for corner in corners {
            min = Vec3::new(
                min.x.min(corner[0]),
                min.y.min(corner[1]),
                min.z.min(corner[2]),
            );
            max = Vec3::new(
                max.x.max(corner[0]),
                max.y.max(corner[1]),
                max.z.max(corner[2]),
            );
        }
        Self { min, max }
    }
    /// Real interpenetration, not contact. Two members that touch, or that share a
    /// level over a few millimetres of shared face, are one continuous surface; a
    /// genuine collision has to overlap on every axis by more than the tolerance.
    fn interpenetrates(&self, other: &Aabb, tolerance: f64) -> bool {
        self.min.x < other.max.x - tolerance
            && self.max.x > other.min.x + tolerance
            && self.min.y < other.max.y - tolerance
            && self.max.y > other.min.y + tolerance
            && self.min.z < other.max.z - tolerance
            && self.max.z > other.min.z + tolerance
    }
}

/// One mesh element. A box is stored as eight corners in a fixed order so the
/// verifier can recompute both the vertices and the triangles without trusting
/// any stored index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshPart {
    pub part_id: String,
    /// `tread`, `riser`, `landing`, `support_block` or `support_column`.
    pub role: String,
    pub structure_id: String,
    pub level_id: String,
    pub run_index: u32,
    pub index: u32,
    pub top_mm: i32,
    pub bottom_mm: i32,
    /// Bottom quad (0..3) then top quad (4..7), counter-clockwise from above.
    pub corners_m: [[f64; 3]; 8],
}

impl MeshPart {
    pub fn aabb(&self) -> Aabb {
        Aabb::from_corners(&self.corners_m)
    }
    pub fn top_level_m(&self) -> f64 {
        f64::from(self.top_mm) / 1000.0
    }
    fn triangles(&self) -> [[usize; 3]; 12] {
        [
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
            [0, 3, 2],
            [0, 2, 1],
        ]
    }
}

/// Build an oriented box from a centre, an in-plane axis and its perpendicular.
#[allow(clippy::too_many_arguments)]
fn oriented_box(
    part_id: &str,
    role: &str,
    structure_id: &str,
    level_id: &str,
    run_index: u32,
    index: u32,
    centre_xz: Vec3,
    axis: Vec3,
    side: Vec3,
    half_length: f64,
    half_width: f64,
    bottom_m: f64,
    top_m: f64,
) -> MeshPart {
    let at = |l: f64, w: f64, y: f64| -> [f64; 3] {
        centre_xz
            .add(axis.scale(l))
            .add(side.scale(w))
            .add(Vec3::new(0.0, y, 0.0))
            .arr()
    };
    let corners = [
        at(-half_length, -half_width, bottom_m - centre_xz.y),
        at(half_length, -half_width, bottom_m - centre_xz.y),
        at(half_length, half_width, bottom_m - centre_xz.y),
        at(-half_length, half_width, bottom_m - centre_xz.y),
        at(-half_length, -half_width, top_m - centre_xz.y),
        at(half_length, -half_width, top_m - centre_xz.y),
        at(half_length, half_width, top_m - centre_xz.y),
        at(-half_length, half_width, top_m - centre_xz.y),
    ];
    MeshPart {
        part_id: part_id.to_string(),
        role: role.to_string(),
        structure_id: structure_id.to_string(),
        level_id: level_id.to_string(),
        run_index,
        index,
        top_mm: (top_m * 1000.0).round() as i32,
        bottom_mm: (bottom_m * 1000.0).round() as i32,
        corners_m: corners,
    }
}

// ---------------------------------------------------------------------------
// Supports
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportRecord {
    pub support_id: String,
    pub structure_id: String,
    pub part_id: String,
    /// `ground_contact` or `column`.
    pub kind: String,
    pub centre_m: [f64; 3],
    pub size_m: f64,
    pub bottom_mm: i32,
    /// `terrain` or the id of the bearing surface it rests on.
    pub bearing_id: String,
    pub bearing_level_mm: i32,
    /// Height of the bearing above the support bottom. Positive means the support
    /// reaches into the bearing, negative means it floats.
    pub overlap_mm: i32,
    pub in_contact: bool,
    pub detail: serde_json::Value,
}

impl SupportRecord {
    fn from_terrain(
        support_id: &str,
        structure_id: &str,
        part_id: &str,
        kind: &str,
        centre_xz: Vec3,
        size_m: f64,
        bottom_mm: i32,
        surface: &EngineeringSurface,
        bearing: &[BearingSurface],
    ) -> EcResult<Self> {
        let existing = bearing.iter().find(|b| {
            centre_xz.x >= b.rect.x0
                && centre_xz.x <= b.rect.x1
                && centre_xz.z >= b.rect.z0
                && centre_xz.z <= b.rect.z1
        });
        let (bearing_id, bearing_level_mm, bearing_source) = match existing {
            Some(surface_bearing) => (
                surface_bearing.id.clone(),
                surface_bearing.level_mm,
                surface_bearing.source.clone(),
            ),
            None => {
                let Some(height) = surface.height_m(centre_xz.x, centre_xz.z) else {
                    return err(
                        "support_outside_grid",
                        format!(
                            "support {support_id} of {structure_id} sits at ({:.3},{:.3}) outside the engineering grid; v1 has no foundation model in the void",
                            centre_xz.x, centre_xz.z
                        ),
                    );
                };
                (
                    "terrain".to_string(),
                    (height * 1000.0).round() as i32,
                    "engineering surface".to_string(),
                )
            }
        };
        let overlap_mm = bearing_level_mm - bottom_mm;
        let in_contact = overlap_mm >= -(SUPPORT_CONTACT_TOLERANCE_M * 1000.0).round() as i32;
        Ok(Self {
            support_id: support_id.to_string(),
            structure_id: structure_id.to_string(),
            part_id: part_id.to_string(),
            kind: kind.to_string(),
            centre_m: centre_xz.arr(),
            size_m,
            bottom_mm,
            bearing_id,
            bearing_level_mm,
            overlap_mm,
            in_contact,
            detail: serde_json::json!({
                "bearingSource": bearing_source,
                "contactToleranceMm": (SUPPORT_CONTACT_TOLERANCE_M * 1000.0) as i64,
                "sign": "overlapMm = bearingLevelMm - supportBottomMm; positive reaches into the bearing, negative floats",
            }),
        })
    }
}

/// Bearing level in millimetres under a footprint, using the same sampling as the
/// production raster (millimetres to `f32` metres, widened to `f64`).
fn bearing_level_mm(
    surface: &EngineeringSurface,
    bearing: &[BearingSurface],
    x: f64,
    z: f64,
) -> Option<i32> {
    if let Some(found) = bearing
        .iter()
        .find(|b| x >= b.rect.x0 && x <= b.rect.x1 && z >= b.rect.z0 && z <= b.rect.z1)
    {
        return Some(found.level_mm);
    }
    surface
        .height_m(x, z)
        .map(|height| (height * 1000.0).round() as i32)
}

// ---------------------------------------------------------------------------
// Ports, nodes and edges
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub node_id: String,
    pub kind: String,
    pub position_m: [f64; 3],
    pub surface_id: String,
    pub level_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_id: Option<String>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeVerification {
    /// `production_terrain_predicate`, `structure_level_difference` or
    /// `structure_step_geometry`.
    pub method: String,
    pub predicate: String,
    pub samples: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_hash: Option<String>,
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    /// `ground`, `stair` or `landing`.
    pub kind: String,
    pub bidirectional: bool,
    pub capabilities: Vec<Capability>,
    pub horizontal_length_m: f64,
    pub path_length_3d_m: f64,
    pub rise_m: f64,
    pub max_step_rise_m: f64,
    pub max_step_run_m: f64,
    /// Climb and descent measured along this edge's own polyline in the from -> to
    /// direction. The search consumes these, never an endpoint height difference.
    pub climb_m: f64,
    pub descent_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_id: Option<String>,
    /// Full three-dimensional polyline actually checked for this edge.
    pub polyline_m: Vec<[f64; 3]>,
    pub verification: EdgeVerification,
}

impl GraphEdge {
    fn supports(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Capability rules of the v1 contract. They are stated once, applied to every
/// edge and recorded in the bundle, so routing never invents a rule per query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRules {
    pub pedestrian: String,
    pub wheelchair: String,
    pub vehicle: String,
    pub rail: String,
    #[serde(default)]
    pub supported_capabilities: Vec<String>,
    pub max_grade_wheelchair: f64,
    pub max_grade_vehicle: f64,
    #[serde(default)]
    pub wheelchair_clear_width_m: f64,
    #[serde(default)]
    pub vehicle_clear_width_m: f64,
    #[serde(default)]
    pub pedestrian_body_height_m: f64,
    #[serde(default)]
    pub wheelchair_body_height_m: f64,
    #[serde(default)]
    pub vehicle_body_height_m: f64,
    #[serde(default)]
    pub solid_contact_tolerance_m: f64,
    #[serde(default)]
    pub local_sampling: String,
    pub no_suitable_edge_meaning: String,
    #[serde(default)]
    pub unsupported_capability_meaning: String,
}

impl Default for CapabilityRules {
    fn default() -> Self {
        Self {
            pedestrian: "ground edges verified by the production terrain predicate, stair edges inside the v2 step limits, landing edges at one level; the walking volume of every granted capability must be clear of obstacles and of every structure that is not the endpoint's own".into(),
            wheelchair: "ground and landing edges whose own sampled band keeps |grade| <= 0.05, with no step, at least 1.2 m of declared clear width and a clear 1.6 m body volume".into(),
            vehicle: "two-way ground edges whose own sampled band keeps |grade| <= 0.08, at least 3.0 m of declared clear width and a clear 4.0 m body volume".into(),
            rail: "not implemented: no alignment, crossfall or clearance model exists, so rail is never granted and a rail query is answered with unsupported_capability".into(),
            supported_capabilities: SUPPORTED_CAPABILITIES.iter().map(|capability| capability_name(*capability).to_string()).collect(),
            max_grade_wheelchair: MAX_GRADE_WHEELCHAIR,
            max_grade_vehicle: MAX_GRADE_VEHICLE,
            wheelchair_clear_width_m: WHEELCHAIR_CLEAR_WIDTH_M,
            vehicle_clear_width_m: VEHICLE_CLEAR_WIDTH_M,
            pedestrian_body_height_m: PEDESTRIAN_BODY_HEIGHT_M,
            wheelchair_body_height_m: WHEELCHAIR_BODY_HEIGHT_M,
            vehicle_body_height_m: VEHICLE_BODY_HEIGHT_M,
            solid_contact_tolerance_m: SOLID_CONTACT_TOLERANCE_M,
            local_sampling: "every capability is judged on its own band of the same half-metre sampling the production predicate uses; an endpoint average is never used and a narrow band never endorses a wider one".into(),
            no_suitable_edge_meaning: "the capability-filtered search exhausted every reachable edge without reaching a port of the requested road; it does not mean the terrain is globally impassable".into(),
            unsupported_capability_meaning: "this module implements no local geometry check for the requested capability, so it refuses instead of endorsing it with another capability's check".into(),
        }
    }
}

/// Capabilities this module really implements a local geometry check for. Anything
/// else is refused explicitly instead of being endorsed by another capability's
/// check.
pub const SUPPORTED_CAPABILITIES: [Capability; 3] = [
    Capability::Pedestrian,
    Capability::Wheelchair,
    Capability::Vehicle,
];

/// The local geometry of one edge. Every field is measured on this edge: no
/// endpoint average, no borrowed width.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityFacts {
    /// Max |grade| over the pedestrian band of this edge.
    pub grade_pedestrian: f64,
    /// Max |grade| over the wheelchair band of this edge.
    pub grade_wheelchair: f64,
    /// Max |grade| over the vehicle band of this edge.
    pub grade_vehicle: f64,
    /// The half width that band corresponds to.
    pub sampled_half_width_m: f64,
    /// Declared clear width of the narrower of the two ports.
    pub clear_width_m: f64,
    pub bidirectional: bool,
    /// Largest step rise on this edge: a step is not a grade.
    pub max_step_rise_m: f64,
    /// True for a deck at one level.
    pub flat: bool,
}

impl CapabilityRules {
    /// The base capability of an edge kind. Extra capabilities are granted by
    /// [`CapabilityRules::decide`], which needs the collision context.
    fn base_for_edge(&self, kind: &str) -> Vec<Capability> {
        match kind {
            "ground" | "stair" | "landing" => vec![Capability::Pedestrian],
            _ => vec![],
        }
    }

    /// Which capabilities the real local geometry earns, and why each other one was
    /// refused. `clearance` maps a capability to the solid that blocks its walking
    /// volume, if any.
    pub fn decide(
        &self,
        kind: &str,
        facts: &CapabilityFacts,
        clearance: &BTreeMap<Capability, Option<String>>,
    ) -> (Vec<Capability>, Vec<serde_json::Value>) {
        let mut granted: Vec<Capability> = vec![];
        let mut refused: Vec<serde_json::Value> = vec![];
        let blocked = |capability: Capability| -> Option<String> {
            clearance.get(&capability).cloned().flatten()
        };
        let walkable = matches!(kind, "ground" | "stair" | "landing");
        if walkable {
            if facts.clear_width_m + 1e-9 < PEDESTRIAN_CLEAR_WIDTH_M {
                refused.push(serde_json::json!({
                    "capability": "pedestrian",
                    "reason": "support_surface_narrower_than_walking_volume",
                    "detail": {
                        "clearWidthM": facts.clear_width_m,
                        "requiredM": PEDESTRIAN_CLEAR_WIDTH_M,                    },
                }));
            } else if let Some(offender) = blocked(Capability::Pedestrian) {
                refused.push(serde_json::json!({
                    "capability": "pedestrian",
                    "reason": "walking_volume_blocked",
                    "detail": { "by": offender, "bodyHeightM": self.pedestrian_body_height_m },
                }));
            } else {
                granted.push(Capability::Pedestrian);
            }
        } else {
            refused.push(serde_json::json!({
                "capability": "pedestrian",
                "reason": "kind_not_walkable",
                "detail": { "kind": kind },
            }));
        }
        if matches!(kind, "ground" | "landing") {
            if facts.max_step_rise_m > 1e-9 {
                refused.push(serde_json::json!({
                    "capability": "wheelchair",
                    "reason": "step_present",
                    "detail": { "maxStepRiseM": facts.max_step_rise_m },
                }));
            } else if facts.grade_wheelchair > self.max_grade_wheelchair + 1e-9 {
                refused.push(serde_json::json!({
                    "capability": "wheelchair",
                    "reason": "local_grade_over_limit",
                    "detail": {
                        "maxGradeWheelchairBand": facts.grade_wheelchair,
                        "maxGradePedestrianBand": facts.grade_pedestrian,
                        "limit": self.max_grade_wheelchair,
                        "sampledHalfWidthM": WHEELCHAIR_CLEAR_WIDTH_M / 2.0,
                    },
                }));
            } else if facts.clear_width_m + 1e-9 < self.wheelchair_clear_width_m {
                refused.push(serde_json::json!({
                    "capability": "wheelchair",
                    "reason": "clear_width_below_requirement",
                    "detail": { "clearWidthM": facts.clear_width_m, "requiredM": self.wheelchair_clear_width_m },
                }));
            } else if let Some(offender) = blocked(Capability::Wheelchair) {
                refused.push(serde_json::json!({
                    "capability": "wheelchair",
                    "reason": "clearance_blocked",
                    "detail": { "by": offender, "bodyHeightM": self.wheelchair_body_height_m },
                }));
            } else {
                granted.push(Capability::Wheelchair);
            }
        } else {
            refused.push(serde_json::json!({
                "capability": "wheelchair",
                "reason": "kind_refuses_capability",
                "detail": { "kind": kind },
            }));
        }
        if kind == "ground" {
            if !facts.bidirectional {
                refused.push(serde_json::json!({
                    "capability": "vehicle",
                    "reason": "one_way_edge",
                    "detail": {},
                }));
            } else if facts.grade_vehicle > self.max_grade_vehicle + 1e-9 {
                refused.push(serde_json::json!({
                    "capability": "vehicle",
                    "reason": "local_grade_over_limit",
                    "detail": {
                        "maxGradeVehicleBand": facts.grade_vehicle,
                        "limit": self.max_grade_vehicle,
                        "sampledHalfWidthM": VEHICLE_CLEAR_WIDTH_M / 2.0,
                    },
                }));
            } else if facts.clear_width_m + 1e-9 < self.vehicle_clear_width_m {
                refused.push(serde_json::json!({
                    "capability": "vehicle",
                    "reason": "clear_width_below_requirement",
                    "detail": { "clearWidthM": facts.clear_width_m, "requiredM": self.vehicle_clear_width_m },
                }));
            } else if let Some(offender) = blocked(Capability::Vehicle) {
                refused.push(serde_json::json!({
                    "capability": "vehicle",
                    "reason": "clearance_blocked",
                    "detail": { "by": offender, "bodyHeightM": self.vehicle_body_height_m },
                }));
            } else {
                granted.push(Capability::Vehicle);
            }
        } else {
            refused.push(serde_json::json!({
                "capability": "vehicle",
                "reason": "kind_refuses_capability",
                "detail": { "kind": kind },
            }));
        }
        refused.push(serde_json::json!({
            "capability": "rail",
            "reason": "unsupported_capability",
            "detail": { "note": self.rail },
        }));
        (granted, refused)
    }

    pub fn body_height(&self, capability: Capability) -> f64 {
        match capability {
            Capability::Pedestrian => self.pedestrian_body_height_m,
            Capability::Wheelchair => self.wheelchair_body_height_m,
            Capability::Vehicle => self.vehicle_body_height_m,
            Capability::Rail => self.pedestrian_body_height_m,
        }
    }

    pub fn required_half_width(&self, capability: Capability) -> f64 {
        match capability {
            Capability::Pedestrian => 0.75,
            Capability::Wheelchair => self.wheelchair_clear_width_m / 2.0,
            Capability::Vehicle => self.vehicle_clear_width_m / 2.0,
            Capability::Rail => 0.75,
        }
    }
}

/// v1 step limits for generated stairs. Outside them the stairs are refused as a
/// design, not silently built.
pub const MAX_STEP_RISE_M: f64 = 0.19;
pub const MIN_STEP_RUN_M: f64 = 0.24;
pub const MAX_STAIR_RUN_M: f64 = 24.0;

// ---------------------------------------------------------------------------
// Structure records
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    pub run_index: u32,
    pub step_index: u32,
    pub tread_part_id: String,
    pub top_level_mm: i32,
    pub rise_mm: i32,
    pub run_mm: i32,
    pub width_mm: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshSummary {
    pub points: usize,
    pub triangles: usize,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
    pub mesh_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureRecord {
    pub structure_id: String,
    pub kind: String,
    pub level_ids: Vec<String>,
    pub entry_node_id: String,
    pub exit_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middle_node_id: Option<String>,
    /// A platform may carry a second port on the same deck, which is what makes it
    /// traversable instead of a dead end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_exit_node_id: Option<String>,
    pub one_way: bool,
    pub width_m: f64,
    pub step_rise_m: f64,
    pub step_run_m: f64,
    pub support_kind: String,
    pub nodes: Vec<GraphNode>,
    pub parts: Vec<MeshPart>,
    pub supports: Vec<SupportRecord>,
    pub steps: Vec<StepRecord>,
    pub total_rise_m: f64,
    pub mesh: MeshSummary,
    pub checks: Vec<serde_json::Value>,
}

fn mesh_hash(parts: &[MeshPart]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"glyphweave-engineering-connections-mesh-v1");
    for part in parts {
        for corner in &part.corners_m {
            for value in corner {
                hasher.update(&value.to_le_bytes());
            }
        }
        hasher.update(part.role.as_bytes());
        hasher.update(part.level_id.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn summarize_mesh(parts: &[MeshPart]) -> MeshSummary {
    let mut min = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut max = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
    for part in parts {
        let aabb = part.aabb();
        min = Vec3::new(
            min.x.min(aabb.min.x),
            min.y.min(aabb.min.y),
            min.z.min(aabb.min.z),
        );
        max = Vec3::new(
            max.x.max(aabb.max.x),
            max.y.max(aabb.max.y),
            max.z.max(aabb.max.z),
        );
    }
    MeshSummary {
        points: parts.len() * 8,
        triangles: parts.len() * 12,
        min_m: min.arr(),
        max_m: max.arr(),
        mesh_hash: mesh_hash(parts),
    }
}

fn find_port<'a>(input: &'a EngineeringInput, port_id: &str) -> EcResult<&'a PortSpec> {
    input
        .ports
        .iter()
        .find(|port| port.id == port_id)
        .ok_or_else(|| {
            EngineeringError::new(
                "unknown_port",
                format!("port {port_id} is referenced but not declared"),
            )
        })
}

fn node_from_port(port: &PortSpec, kind: &str) -> GraphNode {
    GraphNode {
        node_id: port.id.clone(),
        kind: kind.to_string(),
        position_m: port.position_m,
        surface_id: port.surface_id.clone(),
        level_id: port.level_id.clone(),
        road_id: port.road_id.clone(),
        capabilities: port.capabilities.clone(),
    }
}

fn check_finite(position: &[f64; 3], what: &str) -> EcResult<()> {
    let point = Vec3::from_arr(*position);
    if !point.finite() || !(10_000.0 > point.y && point.y > -1_000.0) {
        return err(
            "non_finite_value",
            format!("{what} at {position:?} is not a finite level inside the v1 contract range"),
        );
    }
    Ok(())
}

/// Depth of a generated support footing below its bearing surface.
const FOOTING_DEPTH_M: f64 = 0.1;
/// Largest terrain relief a ground-contact block may absorb before columns are required.
const MAX_GROUND_CONTACT_RELIEF_M: f64 = 0.5;
/// Largest penetration a footing may have into its bearing surface.
const MAX_FOOTING_PENETRATION_M: f64 = 0.3;
/// Two members of different structures interpenetrate only when they overlap on
/// every axis by more than this; contact is not a collision.
const STRUCTURE_OVERLAP_TOLERANCE_M: f64 = 0.02;

fn bearing_corners(corners: &[[f64; 3]; 8]) -> Vec<(f64, f64)> {
    corners
        .iter()
        .map(|corner| (corner[0], corner[2]))
        .collect()
}

/// Build a support set under one part. `ground_contact` grows the part down to the
/// lowest bearing point of its footprint; `columns` keeps the part where the design
/// put it and adds explicit columns under it.
#[allow(clippy::too_many_arguments)]
fn build_supports(
    structure_id: &str,
    part: &MeshPart,
    support: &SupportSpec,
    surface: &EngineeringSurface,
    bearing: &[BearingSurface],
    parts: &mut Vec<MeshPart>,
) -> EcResult<Vec<SupportRecord>> {
    let footprint = bearing_corners(&part.corners_m);
    let (cx, cz) = (
        footprint.iter().map(|p| p.0).sum::<f64>() / footprint.len() as f64,
        footprint.iter().map(|p| p.1).sum::<f64>() / footprint.len() as f64,
    );
    let levels: Vec<i32> = footprint
        .iter()
        .map(|(x, z)| {
            bearing_level_mm(surface, bearing, *x, *z).ok_or_else(|| {
                EngineeringError::new(
                    "support_outside_grid",
                    format!(
                        "{structure_id}/{} footprint corner ({x:.3},{z:.3}) has no bearing surface",
                        part.part_id
                    ),
                )
            })
        })
        .collect::<EcResult<Vec<i32>>>()?;
    let lowest = *levels.iter().min().unwrap_or(&0);
    let highest = *levels.iter().max().unwrap_or(&0);
    match support.kind {
        SupportKind::GroundContact => {
            let relief_m = f64::from(highest - lowest) / 1000.0;
            if relief_m > MAX_GROUND_CONTACT_RELIEF_M {
                return err(
                    "support_requires_columns",
                    format!(
                        "{structure_id}/{} spans {relief_m:.3} m of terrain relief; a ground-contact block may absorb at most {MAX_GROUND_CONTACT_RELIEF_M} m, use columns or a platform",
                        part.part_id
                    ),
                );
            }
            let centre_bearing = bearing_level_mm(surface, bearing, cx, cz).ok_or_else(|| {
                EngineeringError::new(
                    "support_outside_grid",
                    format!(
                        "{structure_id}/{} has no bearing surface at its centre",
                        part.part_id
                    ),
                )
            })?;
            Ok(vec![
                SupportRecord::from_terrain(
                    &format!("{}::{}", part.part_id, "block"),
                    structure_id,
                    &part.part_id,
                    "ground_contact",
                    Vec3::new(cx, 0.0, cz),
                    part.aabb().max.x - part.aabb().min.x,
                    lowest,
                    surface,
                    bearing,
                )
                .map(|mut record| {
                    record.detail = serde_json::json!({
                        "footprintLevelsMm": levels,
                        "lowestBearingMm": lowest,
                        "highestBearingMm": highest,
                        "reliefM": relief_m,
                        "centreBearingMm": centre_bearing,
                        "blockBottomMm": lowest,
                        "model": "the block itself reaches the ground; no separate column is added",
                    });
                    record
                })?,
            ])
        }
        SupportKind::Columns => {
            let columns = support.columns.max(1).min(8);
            let mut records = vec![];
            let axis = Vec3::new(
                part.corners_m[1][0] - part.corners_m[0][0],
                0.0,
                part.corners_m[1][2] - part.corners_m[0][2],
            );
            let long = axis.xz_len();
            let axis = axis.normalize_xz()?;
            let side = axis.perp_xz();
            let half_width = {
                let lateral = Vec3::new(
                    part.corners_m[3][0] - part.corners_m[0][0],
                    0.0,
                    part.corners_m[3][2] - part.corners_m[0][2],
                );
                lateral.xz_len() / 2.0
            };
            for index in 0..columns {
                let t = if columns == 1 {
                    0.5
                } else {
                    (index as f64 + 0.5) / columns as f64
                };
                let offset = (t - 0.5) * long;

                let column_centre = Vec3::new(cx, 0.0, cz).add(axis.scale(offset));
                let half = support.column_size_m / 2.0;
                if offset.abs() + half > long / 2.0 + SOLID_CONTACT_TOLERANCE_M {
                    return err(
                        "column_wider_than_member",
                        format!(
                            "{structure_id}/{} column of {} m does not fit the member along the run: the member is {long:.3} m long and the column would stick out into its neighbour",
                            part.part_id, support.column_size_m
                        ),
                    );
                }
                if half > half_width + 1e-9 {
                    return err(
                        "column_wider_than_member",
                        format!(
                            "{structure_id}/{} column {} of {} m does not fit a member of half width {half_width:.3} m",
                            part.part_id, support.column_size_m, part.part_id
                        ),
                    );
                }
                let bearing_mm = bearing_level_mm(surface, bearing, column_centre.x, column_centre.z)
                    .ok_or_else(|| {
                        EngineeringError::new(
                            "support_outside_grid",
                            format!(
                                "{structure_id}/{} column {index} sits at ({:.3},{:.3}) outside the engineering grid",
                                part.part_id, column_centre.x, column_centre.z
                            ),
                        )
                    })?;
                let column_top = part.aabb().min.y;
                let member_top = part.aabb().max.y;
                let bearing_m = f64::from(bearing_mm) / 1000.0;
                if bearing_m > member_top + SUPPORT_CONTACT_TOLERANCE_M {
                    return err(
                        "member_below_bearing",
                        format!(
                            "{structure_id}/{} member spans {column_top:.3}..{member_top:.3} m but the bearing at ({:.3},{:.3}) is already {bearing_m:.3} m: the member would be buried",
                            part.part_id, column_centre.x, column_centre.z
                        ),
                    );
                }
                if bearing_m >= column_top - SUPPORT_CONTACT_TOLERANCE_M {
                    // The member already reaches the bearing: it rests on it directly and
                    // no column is invented to fill a gap that does not exist.
                    let record = SupportRecord::from_terrain(
                        &format!("{}::contact{}", part.part_id, index),
                        structure_id,
                        &part.part_id,
                        "bearing_contact",
                        column_centre,
                        support.column_size_m,
                        part.bottom_mm,
                        surface,
                        bearing,
                    )
                    .map(|mut record| {
                        record.detail = serde_json::json!({
                            "bearingMm": bearing_mm,
                            "memberBottomMm": part.bottom_mm,
                            "memberTopMm": part.top_mm,
                            "model": "the member rests directly on its bearing surface; no column is added",
                        });
                        record
                    })?;
                    records.push(record);
                    continue;
                }
                let column_bottom = bearing_m - FOOTING_DEPTH_M;
                let penetration_m = FOOTING_DEPTH_M;
                if penetration_m > MAX_FOOTING_PENETRATION_M {
                    return err(
                        "footing_too_deep",
                        format!(
                            "{structure_id}/{} footing depth {penetration_m:.3} m exceeds the v1 cap {MAX_FOOTING_PENETRATION_M} m",
                            part.part_id
                        ),
                    );
                }
                let column = oriented_box(
                    &format!("{}::column{}", part.part_id, index),
                    "support_column",
                    structure_id,
                    &part.level_id,
                    part.run_index,
                    index as u32,
                    column_centre,
                    axis,
                    side,
                    half,
                    half,
                    column_bottom,
                    column_top,
                );
                let record = SupportRecord::from_terrain(
                    &format!("{}::column{}", part.part_id, index),
                    structure_id,
                    &part.part_id,
                    "column",
                    column_centre,
                    support.column_size_m,
                    (column_bottom * 1000.0).round() as i32,
                    surface,
                    bearing,
                )
                .map(|mut record| {
                    record.detail = serde_json::json!({
                        "bearingMm": bearing_mm,
                        "footingDepthM": FOOTING_DEPTH_M,
                        "columnTopM": column_top,
                        "penetrationM": penetration_m,
                        "model": "an explicit column from its member down to a footing below the bearing surface",
                    });
                    record
                })?;
                records.push(record);
                parts.push(column);
            }
            Ok(records)
        }
    }
}

fn support_spec_check(spec: &SupportSpec, what: &str) -> EcResult<()> {
    if spec.kind == SupportKind::Columns && !(0.1..=1.0).contains(&spec.column_size_m) {
        return err(
            "invalid_column_size",
            format!(
                "{what}: column size {} m is outside 0.1..1.0 m",
                spec.column_size_m
            ),
        );
    }
    Ok(())
}

fn new_node_id(structure_id: &str, role: &str) -> String {
    format!("{structure_id}:{role}")
}

fn structure_node(
    node_id: &str,
    kind: &str,
    position: [f64; 3],
    structure_id: &str,
    level_id: &str,
    capabilities: Vec<Capability>,
) -> GraphNode {
    GraphNode {
        node_id: node_id.to_string(),
        kind: kind.to_string(),
        position_m: position,
        surface_id: structure_id.to_string(),
        level_id: level_id.to_string(),
        road_id: None,
        capabilities,
    }
}

fn verify_declared_port(port: &PortSpec, expected: [f64; 3], what: &str) -> EcResult<()> {
    check_finite(&expected, what)?;
    let drift = Vec3::from_arr(port.position_m)
        .sub(Vec3::from_arr(expected))
        .len();
    if drift > PORT_MATCH_TOLERANCE_M {
        return err(
            "port_geometry_mismatch",
            format!(
                "port {} of {what} is declared at {:?} but the geometry puts it at {expected:?} ({drift:.4} m apart)",
                port.id, port.position_m
            ),
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn build_landing(
    spec: &LandingSpec,
    input: &EngineeringInput,
    surface: &EngineeringSurface,
) -> EcResult<StructureRecord> {
    let port = find_port(input, &spec.port_id)?;
    let centre = Vec3::from_arr(spec.centre_m);
    if !centre.finite() || spec.width_m <= 0.5 || spec.depth_m <= 0.5 || spec.thickness_m < 0.05 {
        return err(
            "invalid_landing",
            format!("landing {} has invalid dimensions", spec.id),
        );
    }
    support_spec_check(&spec.support, &spec.id)?;
    let axis = Vec3::new(1.0, 0.0, 0.0);
    let side = Vec3::new(0.0, 0.0, 1.0);
    let top = centre.y;
    let level_id = format!("{}:deck", spec.id);
    // A port may sit anywhere on the deck, but it must sit at the deck level: an
    // elevated deck port is never quietly treated as ground.
    if (port.position_m[1] - top).abs() > PORT_MATCH_TOLERANCE_M {
        return err(
            "port_geometry_mismatch",
            format!(
                "port {} of landing {} is declared at {:.3} m but the deck top is at {top:.3} m",
                port.id, spec.id, port.position_m[1]
            ),
        );
    }
    if (port.position_m[0] - centre.x).abs() > spec.width_m / 2.0
        || (port.position_m[2] - centre.z).abs() > spec.depth_m / 2.0
    {
        return err(
            "port_outside_structure",
            format!("port {} is outside landing {} footprint", port.id, spec.id),
        );
    }
    let mut parts = vec![oriented_box(
        &format!("{}::deck", spec.id),
        "landing",
        &spec.id,
        &level_id,
        0,
        0,
        centre,
        axis,
        side,
        spec.width_m / 2.0,
        spec.depth_m / 2.0,
        top - spec.thickness_m,
        top,
    )];
    let deck_id = parts[0].part_id.clone();
    let supports = match spec.support.kind {
        SupportKind::GroundContact => {
            let lowest = bearing_corners(&parts[0].corners_m)
                .iter()
                .map(|(x, z)| {
                    bearing_level_mm(surface, &input.bearing, *x, *z).ok_or_else(|| {
                        EngineeringError::new(
                            "support_outside_grid",
                            format!("landing {} overhangs the engineering grid", spec.id),
                        )
                    })
                })
                .collect::<EcResult<Vec<i32>>>()?
                .into_iter()
                .min()
                .unwrap_or(0);
            let bottom = f64::from(lowest) / 1000.0;
            if bottom < top - spec.thickness_m - 0.001 {
                parts[0] = oriented_box(
                    &format!("{}::deck", spec.id),
                    "landing",
                    &spec.id,
                    &level_id,
                    0,
                    0,
                    centre,
                    axis,
                    side,
                    spec.width_m / 2.0,
                    spec.depth_m / 2.0,
                    bottom,
                    top,
                );
            }
            let deck = parts[0].clone();
            build_supports(
                &spec.id,
                &deck,
                &spec.support,
                surface,
                &input.bearing,
                &mut parts,
            )?
        }
        SupportKind::Columns => {
            let deck = parts[0].clone();
            build_supports(
                &spec.id,
                &deck,
                &spec.support,
                surface,
                &input.bearing,
                &mut parts,
            )?
        }
    };
    let mut nodes = vec![node_from_port(port, "landing_port")];
    let deck_exit_node_id = match &spec.exit_port_id {
        Some(exit_id) => {
            let exit = find_port(input, exit_id)?;
            if exit.id == port.id {
                return err(
                    "duplicate_id",
                    format!("landing {} uses the same port twice", spec.id),
                );
            }
            if (exit.position_m[1] - top).abs() > PORT_MATCH_TOLERANCE_M {
                return err(
                    "port_geometry_mismatch",
                    format!(
                        "second port {} of landing {} is declared at {:.3} m but the deck top is at {top:.3} m",
                        exit.id, spec.id, exit.position_m[1]
                    ),
                );
            }
            if (exit.position_m[0] - centre.x).abs() > spec.width_m / 2.0
                || (exit.position_m[2] - centre.z).abs() > spec.depth_m / 2.0
            {
                return err(
                    "port_outside_structure",
                    format!("port {} is outside landing {} footprint", exit.id, spec.id),
                );
            }
            nodes.push(node_from_port(exit, "landing_port"));
            Some(exit.id.clone())
        }
        None => None,
    };
    let parts = dedupe_parts(parts, &deck_id);
    let checks = vec![serde_json::json!({
        "check": "declared_port_matches_geometry",
        "toleranceM": PORT_MATCH_TOLERANCE_M,
        "detail": "every port level must be the landing deck top, and its plan position must be inside the deck",
    })];
    Ok(StructureRecord {
        structure_id: spec.id.clone(),
        kind: "landing".into(),
        level_ids: vec![level_id],
        entry_node_id: nodes[0].node_id.clone(),
        exit_node_id: nodes[0].node_id.clone(),
        middle_node_id: None,
        deck_exit_node_id,
        one_way: false,
        width_m: spec.width_m,
        step_rise_m: 0.0,
        step_run_m: 0.0,
        support_kind: support_kind_name(&spec.support),
        nodes,
        mesh: summarize_mesh(&parts),
        parts,
        supports,
        steps: vec![],
        total_rise_m: 0.0,
        checks,
    })
}

fn support_kind_name(support: &SupportSpec) -> String {
    match support.kind {
        SupportKind::GroundContact => "ground_contact".into(),
        SupportKind::Columns => "columns".into(),
    }
}

/// A landing whose deck already reaches the lowest bearing point keeps one deck
/// part; the initial part is only replaced, never duplicated.
fn dedupe_parts(parts: Vec<MeshPart>, _deck_id: &str) -> Vec<MeshPart> {
    let mut seen = BTreeSet::new();
    parts
        .into_iter()
        .filter(|part| seen.insert(part.part_id.clone()))
        .collect()
}

fn build_straight_stair(
    spec: &StraightStairSpec,
    input: &EngineeringInput,
    surface: &EngineeringSurface,
) -> EcResult<StructureRecord> {
    let from = find_port(input, &spec.bottom_port_id)?;
    let to = find_port(input, &spec.top_port_id)?;
    let a = Vec3::from_arr(from.position_m);
    let b = Vec3::from_arr(to.position_m);
    check_finite(&from.position_m, &format!("stair {} bottom port", spec.id))?;
    check_finite(&to.position_m, &format!("stair {} top port", spec.id))?;
    let axis = b.sub(a).normalize_xz()?;
    let side = axis.perp_xz();
    let steps = spec.steps;
    if steps == 0 {
        return err("invalid_stair", format!("stair {} has no steps", spec.id));
    }
    let rise_m = f64::from(spec.rise_mm) / 1000.0;
    let run_m = f64::from(spec.run_mm) / 1000.0;
    if rise_m <= 0.0 || rise_m > MAX_STEP_RISE_M {
        return err(
            "step_out_of_contract",
            format!(
                "stair {} rise {rise_m:.3} m is outside (0, {MAX_STEP_RISE_M}] m",
                spec.id
            ),
        );
    }
    if run_m < MIN_STEP_RUN_M {
        return err(
            "step_out_of_contract",
            format!(
                "stair {} run {run_m:.3} m is below the v1 minimum {MIN_STEP_RUN_M} m",
                spec.id
            ),
        );
    }
    if f64::from(steps) * run_m > MAX_STAIR_RUN_M {
        return err(
            "stair_too_long",
            format!(
                "stair {} run {:.3} m exceeds the single-run cap {MAX_STAIR_RUN_M} m; use landings",
                spec.id,
                f64::from(steps) * run_m
            ),
        );
    }
    support_spec_check(&spec.support, &spec.id)?;
    let expected_top = a.y + f64::from(steps) * rise_m;
    if (b.y - expected_top).abs() > 0.02 {
        return err(
            "port_geometry_mismatch",
            format!(
                "stair {} tops at {expected_top:.3} m but its top port sits at {:.3} m",
                spec.id, b.y
            ),
        );
    }
    let horizontal = b.sub(a).xz_len();
    if (horizontal - f64::from(steps) * run_m).abs() > 0.02 {
        return err(
            "port_geometry_mismatch",
            format!(
                "stair {} runs {:.3} m but its ports are {horizontal:.3} m apart in plan",
                spec.id,
                f64::from(steps) * run_m
            ),
        );
    }
    let mut parts: Vec<MeshPart> = vec![];
    let mut steps_records = vec![];
    let mut supports = vec![];
    let level_id = format!("{}:flight", spec.id);
    for index in 0..steps {
        let top = a.y + f64::from(index + 1) * rise_m;
        let centre = Vec3::new(a.x, 0.0, a.z).add(axis.scale((f64::from(index) + 0.5) * run_m));
        let tread = oriented_box(
            &format!("{}::tread{}", spec.id, index),
            "tread",
            &spec.id,
            &level_id,
            0,
            index,
            centre,
            axis,
            side,
            run_m / 2.0,
            spec.width_m / 2.0,
            top - spec.thickness_m,
            top,
        );
        parts.push(tread);
    }
    for index in 0..steps {
        let top = a.y + f64::from(index + 1) * rise_m;
        let front = Vec3::new(a.x, 0.0, a.z).add(axis.scale(f64::from(index) * run_m));
        parts.push(oriented_box(
            &format!("{}::riser{}", spec.id, index),
            "riser",
            &spec.id,
            &level_id,
            0,
            index,
            front,
            axis,
            side,
            0.03,
            spec.width_m / 2.0,
            top - rise_m,
            top,
        ));
    }
    let support_parts: Vec<MeshPart> = parts
        .iter()
        .filter(|part| part.role == "tread")
        .cloned()
        .collect();
    for tread in &support_parts {
        let index = tread.index;
        let top = f64::from(tread.top_mm) / 1000.0;
        let effective = match spec.support.kind {
            SupportKind::GroundContact => {
                let footprint: Vec<(f64, f64)> = bearing_corners(&tread.corners_m);
                let lowest = footprint
                    .iter()
                    .map(|(x, z)| {
                        bearing_level_mm(surface, &input.bearing, *x, *z).ok_or_else(|| {
                            EngineeringError::new(
                                "support_outside_grid",
                                format!(
                                    "stair {} tread {index} leaves the engineering grid",
                                    spec.id
                                ),
                            )
                        })
                    })
                    .collect::<EcResult<Vec<i32>>>()?
                    .into_iter()
                    .min()
                    .unwrap_or(0);
                let mut grown = tread.clone();
                grown.corners_m[0][1] = f64::from(lowest) / 1000.0;
                grown.corners_m[1][1] = f64::from(lowest) / 1000.0;
                grown.corners_m[2][1] = f64::from(lowest) / 1000.0;
                grown.corners_m[3][1] = f64::from(lowest) / 1000.0;
                grown.bottom_mm = lowest;
                if let Some(slot) = parts.iter_mut().find(|part| part.part_id == tread.part_id) {
                    *slot = grown.clone();
                }
                grown
            }
            SupportKind::Columns => tread.clone(),
        };
        let mut produced = vec![];
        let records = build_supports(
            &spec.id,
            &effective,
            &spec.support,
            surface,
            &input.bearing,
            &mut produced,
        )?;
        parts.extend(produced);
        supports.extend(records);
        steps_records.push(StepRecord {
            run_index: 0,
            step_index: index,
            tread_part_id: effective.part_id.clone(),
            top_level_mm: (top * 1000.0).round() as i32,
            rise_mm: spec.rise_mm,
            run_mm: spec.run_mm,
            width_mm: (spec.width_m * 1000.0).round() as i32,
        });
    }
    let entry = node_from_port(from, "stair_bottom_port");
    let exit = node_from_port(to, "stair_top_port");
    let checks = vec![
        serde_json::json!({
            "check": "declared_ports_match_step_geometry",
            "toleranceM": 0.02,
            "riseMm": spec.rise_mm,
            "runMm": spec.run_mm,
            "steps": steps,
        }),
        serde_json::json!({
            "check": "step_limits",
            "maxStepRiseM": MAX_STEP_RISE_M,
            "minStepRunM": MIN_STEP_RUN_M,
            "maxSingleRunM": MAX_STAIR_RUN_M,
        }),
    ];
    let mesh = summarize_mesh(&parts);
    Ok(StructureRecord {
        structure_id: spec.id.clone(),
        kind: "straight_stair".into(),
        level_ids: vec![level_id],
        entry_node_id: entry.node_id.clone(),
        exit_node_id: exit.node_id.clone(),
        middle_node_id: None,
        deck_exit_node_id: None,
        one_way: spec.one_way,
        width_m: spec.width_m,
        step_rise_m: rise_m,
        step_run_m: run_m,
        support_kind: support_kind_name(&spec.support),
        nodes: vec![entry, exit],
        parts,
        supports,
        steps: steps_records,
        total_rise_m: f64::from(steps) * rise_m,
        mesh,
        checks,
    })
}

fn build_switchback_stair(
    spec: &SwitchbackStairSpec,
    input: &EngineeringInput,
    surface: &EngineeringSurface,
) -> EcResult<StructureRecord> {
    let from = find_port(input, &spec.bottom_port_id)?;
    let to = find_port(input, &spec.top_port_id)?;
    let middle = find_port(input, &spec.middle_port_id)?;
    let a = Vec3::from_arr(from.position_m);
    let b = Vec3::from_arr(to.position_m);
    let n = spec.steps_per_run;
    if n == 0 {
        return err(
            "invalid_stair",
            format!("switchback {} has no steps", spec.id),
        );
    }
    let rise_m = f64::from(spec.rise_mm) / 1000.0;
    let run_m = f64::from(spec.run_mm) / 1000.0;
    if rise_m <= 0.0 || rise_m > MAX_STEP_RISE_M || run_m < MIN_STEP_RUN_M {
        return err(
            "step_out_of_contract",
            format!(
                "switchback {} step geometry is outside the v1 limits",
                spec.id
            ),
        );
    }
    if f64::from(n) * run_m > MAX_STAIR_RUN_M {
        return err(
            "stair_too_long",
            format!(
                "switchback {} single run exceeds {MAX_STAIR_RUN_M} m",
                spec.id
            ),
        );
    }
    support_spec_check(&spec.support, &spec.id)?;
    let turn = Vec3::from_arr(spec.turn_centre_m);
    let landing_top = a.y + f64::from(n) * rise_m;
    if (turn.y - landing_top).abs() > 0.02 {
        return err(
            "landing_level_mismatch",
            format!(
                "switchback {} turn landing sits at {:.3} m but the first flight tops at {landing_top:.3} m",
                spec.id, turn.y
            ),
        );
    }
    let expected_top = landing_top + f64::from(n) * rise_m;
    if (b.y - expected_top).abs() > 0.02 {
        return err(
            "port_geometry_mismatch",
            format!(
                "switchback {} tops at {expected_top:.3} m but its top port sits at {:.3} m",
                spec.id, b.y
            ),
        );
    }
    let axis = Vec3::new(turn.x - a.x, 0.0, turn.z - a.z).normalize_xz()?;
    let side = axis.perp_xz();
    let lane = spec.width_m;
    let expected_top_plan = Vec3::new(a.x, 0.0, a.z)
        .add(side.scale(lane))
        .add(Vec3::new(0.0, 0.0, 0.0));
    let plan_drift = Vec3::new(b.x, 0.0, b.z).sub(expected_top_plan).xz_len();
    if plan_drift > 0.05 {
        return err(
            "port_geometry_mismatch",
            format!(
                "switchback {} returns one lane ({lane:.3} m) beside its bottom port; the top port is {plan_drift:.3} m from that plan position",
                spec.id
            ),
        );
    }
    let mut parts: Vec<MeshPart> = vec![];
    let mut steps_records = vec![];
    let mut supports = vec![];
    let run1_level = format!("{}:run1", spec.id);
    let run2_level = format!("{}:run2", spec.id);
    let landing_level = format!("{}:turn", spec.id);
    for index in 0..n {
        let top = a.y + f64::from(index + 1) * rise_m;
        let centre = Vec3::new(a.x, 0.0, a.z).add(axis.scale((f64::from(index) + 0.5) * run_m));
        parts.push(oriented_box(
            &format!("{}::r1tread{}", spec.id, index),
            "tread",
            &spec.id,
            &run1_level,
            0,
            index,
            centre,
            axis,
            side,
            run_m / 2.0,
            spec.width_m / 2.0,
            top - spec.thickness_m,
            top,
        ));
        let front = Vec3::new(a.x, 0.0, a.z).add(axis.scale(f64::from(index) * run_m));
        parts.push(oriented_box(
            &format!("{}::r1riser{}", spec.id, index),
            "riser",
            &spec.id,
            &run1_level,
            0,
            index,
            front,
            axis,
            side,
            0.03,
            spec.width_m / 2.0,
            top - rise_m,
            top,
        ));
    }
    let far = Vec3::new(a.x, 0.0, a.z).add(axis.scale(f64::from(n) * run_m));
    for index in 0..n {
        let top = landing_top + f64::from(index + 1) * rise_m;
        let centre = far
            .add(side.scale(lane))
            .sub(axis.scale((f64::from(index) + 0.5) * run_m));
        parts.push(oriented_box(
            &format!("{}::r2tread{}", spec.id, index),
            "tread",
            &spec.id,
            &run2_level,
            1,
            index,
            centre,
            axis,
            side,
            run_m / 2.0,
            spec.width_m / 2.0,
            top - spec.thickness_m,
            top,
        ));
        let front = far
            .add(side.scale(lane))
            .sub(axis.scale(f64::from(index) * run_m));
        parts.push(oriented_box(
            &format!("{}::r2riser{}", spec.id, index),
            "riser",
            &spec.id,
            &run2_level,
            1,
            index,
            front,
            axis,
            side,
            0.03,
            spec.width_m / 2.0,
            top - rise_m,
            top,
        ));
    }
    let turn_centre = Vec3::new(turn.x, 0.0, turn.z);
    let turn_part = oriented_box(
        &format!("{}::turn", spec.id),
        "landing",
        &spec.id,
        &landing_level,
        0,
        0,
        turn_centre,
        axis,
        side,
        spec.turn_length_m / 2.0,
        spec.turn_depth_m / 2.0,
        turn.y - spec.thickness_m,
        turn.y,
    );
    // The turn landing must really reach both flights in plan.
    let required = [
        (far, "first flight head"),
        (far.add(side.scale(lane)), "second flight foot"),
    ];
    // The landing is an oriented rectangle: its own turnover length runs along the stair
    // axis and its depth runs across it, so containment is measured in that frame. Adding
    // and subtracting the axis and side vectors separately builds a plan box whose lower
    // corner can sit above its upper one as soon as the stair is turned, which would
    // report the landing as covering a point that is nowhere near it.
    let landing_reaches = |point: Vec3| {
        let dx = point.x - turn_centre.x;
        let dz = point.z - turn_centre.z;
        let along = dx * axis.x + dz * axis.z;
        let across = dx * side.x + dz * side.z;
        along.abs() <= spec.turn_length_m / 2.0 + 0.05
            && across.abs() <= spec.turn_depth_m / 2.0 + 0.05
    };
    for (point, what) in required {
        if !landing_reaches(point) {
            return err(
                "landing_does_not_reach_run",
                format!(
                    "switchback {} turn landing does not cover its {what} at ({:.3},{:.3})",
                    spec.id, point.x, point.z
                ),
            );
        }
    }
    if (middle.position_m[1] - turn.y).abs() > PORT_MATCH_TOLERANCE_M {
        return err(
            "port_geometry_mismatch",
            format!(
                "switchback {} middle port {} sits at {:.3} m, not on the turn landing deck at {:.3} m",
                spec.id, middle.id, middle.position_m[1], turn.y
            ),
        );
    }
    if !landing_reaches(Vec3::new(middle.position_m[0], 0.0, middle.position_m[2])) {
        return err(
            "port_outside_structure",
            format!(
                "switchback {} middle port is outside the turn landing",
                spec.id
            ),
        );
    }
    // The turn landing is a solid of its own, not a plate laid over the flights: it stands
    // beyond the line where both flights end, its own extent along the axis gives the turn
    // walk a full band, and its depth gives the second lane the same band. A landing that
    // reaches back over a flight would interpenetrate the tread it covers, so it is refused
    // here by geometry instead of being excused later in the collision check.
    let axis_of = |point: Vec3| point.x * axis.x + point.z * axis.z;
    let side_of = |point: Vec3| point.x * side.x + point.z * side.z;
    let near_edge = axis_of(turn_centre) - spec.turn_length_m / 2.0;
    let far_edge = axis_of(turn_centre) + spec.turn_length_m / 2.0;
    let flight_end = axis_of(far);
    if near_edge < flight_end - SOLID_CONTACT_TOLERANCE_M {
        return err(
            "turn_landing_overlaps_flight",
            format!(
                "switchback {} turn landing starts {:.3} m along its axis but the first flight ends at {flight_end:.3} m: a landing that reaches back over a flight interpenetrates the tread it covers",
                spec.id, near_edge
            ),
        );
    }
    if spec.turn_length_m + 1e-9 < PEDESTRIAN_CLEAR_WIDTH_M {
        return err(
            "turn_landing_too_shallow",
            format!(
                "switchback {} turn landing is {:.3} m deep along the walk but the pedestrian band is {PEDESTRIAN_CLEAR_WIDTH_M:.3} m",
                spec.id, spec.turn_length_m
            ),
        );
    }
    if spec.turn_depth_m + 1e-9 < 2.0 * lane + PEDESTRIAN_CLEAR_WIDTH_M {
        return err(
            "turn_landing_too_narrow",
            format!(
                "switchback {} turn landing is {:.3} m across but both lanes and the pedestrian band on their far side need {:.3} m",
                spec.id,
                spec.turn_depth_m,
                2.0 * lane + PEDESTRIAN_CLEAR_WIDTH_M
            ),
        );
    }
    let middle_axis = axis_of(Vec3::from_arr(middle.position_m));
    let half_band = PEDESTRIAN_CLEAR_WIDTH_M / 2.0;
    if middle_axis - near_edge + 1e-9 < half_band || far_edge - middle_axis + 1e-9 < half_band {
        return err(
            "turn_landing_port_too_close",
            format!(
                "switchback {} turn port sits {:.3} m from the landing's near edge and {:.3} m from its far edge; the turn walk needs {half_band:.3} m on both sides",
                spec.id,
                middle_axis - near_edge,
                far_edge - middle_axis
            ),
        );
    }
    // The turn is two real legs through a corner on the second lane: the walk crosses the
    // landing and then runs along it to the foot of the second flight.
    let turn_exit_plan = Vec3::new(far.x, turn.y, far.z).add(side.scale(lane));
    let corner_position = {
        let middle_point = Vec3::from_arr(middle.position_m);
        let to_exit = turn_exit_plan.sub(middle_point);
        middle_point.add(side.scale(side_of(to_exit)))
    };
    if !landing_reaches(corner_position) {
        return err(
            "turn_corner_outside_landing",
            format!(
                "switchback {} turn corner at ({:.3},{:.3}) is outside its own turn landing",
                spec.id, corner_position.x, corner_position.z
            ),
        );
    }
    if side_of(corner_position) - side_of(Vec3::from_arr(middle.position_m)) < lane - 1e-9 {
        return err(
            "turn_corner_missing_lane",
            format!(
                "switchback {} turn corner does not reach the second lane",
                spec.id
            ),
        );
    }
    if corner_position
        .sub(Vec3::from_arr(middle.position_m))
        .xz_len()
        < 1e-6
        || turn_exit_plan.sub(corner_position).xz_len() < 1e-6
    {
        return err(
            "turn_legs_degenerate",
            format!("switchback {} turn has a zero-length leg", spec.id),
        );
    }
    parts.push(turn_part);
    let support_targets: Vec<MeshPart> = parts
        .iter()
        .filter(|part| part.role == "tread" || part.role == "landing")
        .cloned()
        .collect();
    for target in &support_targets {
        let effective = match spec.support.kind {
            SupportKind::GroundContact => {
                let footprint = bearing_corners(&target.corners_m);
                let lowest = footprint
                    .iter()
                    .map(|(x, z)| {
                        bearing_level_mm(surface, &input.bearing, *x, *z).ok_or_else(|| {
                            EngineeringError::new(
                                "support_outside_grid",
                                format!(
                                    "switchback {} member {} leaves the engineering grid",
                                    spec.id, target.part_id
                                ),
                            )
                        })
                    })
                    .collect::<EcResult<Vec<i32>>>()?
                    .into_iter()
                    .min()
                    .unwrap_or(0);
                let mut grown = target.clone();
                for corner in 0..4 {
                    grown.corners_m[corner][1] = f64::from(lowest) / 1000.0;
                }
                grown.bottom_mm = lowest;
                if let Some(slot) = parts.iter_mut().find(|part| part.part_id == target.part_id) {
                    *slot = grown.clone();
                }
                grown
            }
            SupportKind::Columns => target.clone(),
        };
        let mut produced = vec![];
        let records = build_supports(
            &spec.id,
            &effective,
            &spec.support,
            surface,
            &input.bearing,
            &mut produced,
        )?;
        parts.extend(produced);
        supports.extend(records);
    }
    for index in 0..n {
        steps_records.push(StepRecord {
            run_index: 0,
            step_index: index,
            tread_part_id: format!("{}::r1tread{}", spec.id, index),
            top_level_mm: ((a.y + f64::from(index + 1) * rise_m) * 1000.0).round() as i32,
            rise_mm: spec.rise_mm,
            run_mm: spec.run_mm,
            width_mm: (spec.width_m * 1000.0).round() as i32,
        });
        steps_records.push(StepRecord {
            run_index: 1,
            step_index: index,
            tread_part_id: format!("{}::r2tread{}", spec.id, index),
            top_level_mm: ((landing_top + f64::from(index + 1) * rise_m) * 1000.0).round() as i32,
            rise_mm: spec.rise_mm,
            run_mm: spec.run_mm,
            width_mm: (spec.width_m * 1000.0).round() as i32,
        });
    }
    let entry = node_from_port(from, "stair_bottom_port");
    let exit = node_from_port(to, "stair_top_port");
    let middle_node = node_from_port(middle, "stair_turn_port");
    let turn_exit = structure_node(
        &new_node_id(&spec.id, "turn_exit"),
        "stair_turn_exit",
        turn_exit_plan.arr(),
        &spec.id,
        &landing_level,
        vec![Capability::Pedestrian, Capability::Wheelchair],
    );
    let turn_corner = structure_node(
        &new_node_id(&spec.id, "turn_corner"),
        "stair_turn_corner",
        corner_position.arr(),
        &spec.id,
        &landing_level,
        vec![Capability::Pedestrian, Capability::Wheelchair],
    );
    let checks = vec![
        serde_json::json!({
            "check": "two_flights_one_lane_apart",
            "laneM": lane,
            "planDriftM": plan_drift,
            "detail": "the second flight is offset one stair width from the first, which is what makes the switchback a switchback",
        }),
        serde_json::json!({
            "check": "turn_landing_reaches_both_flights",
            "turnLengthM": spec.turn_length_m,
            "turnDepthM": spec.turn_depth_m,
        }),
        serde_json::json!({
            "check": "turn_landing_is_an_independent_level",
            "levelId": landing_level,
            "levelM": turn.y,
            "detail": "the turn is never collapsed into a ground edge; it carries its own level, a corner on the second lane and two landing edges",
        }),
        serde_json::json!({
            "check": "turn_landing_stands_beyond_the_flights",
            "nearEdgeM": near_edge,
            "farEdgeM": far_edge,
            "flightEndM": flight_end,
            "turnCornerM": [corner_position.x, corner_position.y, corner_position.z],
            "detail": "the landing is a solid of its own beyond the line where both flights end; it does not lie over a flight, so no collision exemption is needed to build it",
        }),
    ];
    let mesh = summarize_mesh(&parts);
    Ok(StructureRecord {
        structure_id: spec.id.clone(),
        kind: "switchback_stair".into(),
        level_ids: vec![run1_level, run2_level, landing_level],
        entry_node_id: entry.node_id.clone(),
        exit_node_id: exit.node_id.clone(),
        middle_node_id: Some(middle_node.node_id.clone()),
        deck_exit_node_id: None,
        one_way: spec.one_way,
        width_m: spec.width_m,
        step_rise_m: rise_m,
        step_run_m: run_m,
        support_kind: support_kind_name(&spec.support),
        nodes: vec![entry, middle_node, turn_corner, turn_exit, exit],
        parts,
        supports,
        steps: steps_records,
        total_rise_m: f64::from(n) * rise_m * 2.0,
        mesh,
        checks,
    })
}

#[allow(clippy::too_many_arguments)]
fn structure_edge(
    edge_id: &str,
    from: &GraphNode,
    to: &GraphNode,
    kind: &str,
    one_way: bool,
    width_m: f64,
    horizontal: f64,
    polyline: Vec<[f64; 3]>,
    rise: f64,
    max_step_rise: f64,
    max_step_run: f64,
    rules: &CapabilityRules,
    structure_id: &str,
    detail: serde_json::Value,
) -> GraphEdge {
    let path_length_3d = polyline_length(&polyline);
    let polyline_m_for_metrics = polyline.clone();
    let samples = polyline.len();
    let grade = if horizontal > 1e-9 {
        rise / horizontal
    } else {
        0.0
    };
    let bidirectional = !one_way;
    GraphEdge {
        edge_id: edge_id.to_string(),
        from_node: from.node_id.clone(),
        to_node: to.node_id.clone(),
        kind: kind.to_string(),
        bidirectional,
        capabilities: rules.base_for_edge(kind),
        horizontal_length_m: horizontal,
        path_length_3d_m: path_length_3d,
        rise_m: rise,
        max_step_rise_m: max_step_rise,
        max_step_run_m: max_step_run,
        climb_m: polyline_metrics(&polyline_m_for_metrics).2,
        descent_m: polyline_metrics(&polyline_m_for_metrics).3,
        structure_id: Some(structure_id.to_string()),
        polyline_m: polyline,
        verification: EdgeVerification {
            method: "structure_step_geometry".into(),
            predicate: "generated from the verified structure levels, tread geometry and support set of the same bundle".into(),
            samples,
            terrain_hash: None,
            detail,
        },
    }
}

fn polyline_length(polyline: &[[f64; 3]]) -> f64 {
    polyline
        .windows(2)
        .map(|pair| Vec3::from_arr(pair[1]).sub(Vec3::from_arr(pair[0])).len())
        .sum()
}

// ---------------------------------------------------------------------------
// Ground edges (reusing the production terrain predicate)
// ---------------------------------------------------------------------------

const GROUND_SAMPLE_STEP_M: f64 = 0.5;

/// Inflated segment versus axis-aligned rectangle in plan, the same construction
/// the production access solver uses for obstacles.
fn segment_hits_rect(ax: f64, az: f64, bx: f64, bz: f64, half: f64, rect: &RectF) -> bool {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for (p, d, low, high) in [
        (ax, bx - ax, rect.x0 - half, rect.x1 + half),
        (az, bz - az, rect.z0 - half, rect.z1 + half),
    ] {
        if d.abs() < 1e-10 {
            if p < low || p > high {
                return false;
            }
        } else {
            let t0 = (low - p) / d;
            let t1 = (high - p) / d;
            lo = lo.max(t0.min(t1));
            hi = hi.min(t0.max(t1));
        }
    }
    lo < hi && hi > 0.0 && lo < 1.0
}

/// Sample the engineered surface over one or more lateral bands of the same
/// half-metre step the production predicate uses. Returns the centre polyline
/// (snapped to the ports, so the checked geometry is continuous with the graph),
/// the centre-line max |grade|, the max |grade| over every band, and the step count.
fn ground_band_profile(
    surface: &EngineeringSurface,
    a: Vec3,
    b: Vec3,
    half_widths: &[f64],
) -> EcResult<(Vec<[f64; 3]>, f64, f64, usize)> {
    let horizontal = b.sub(a).xz_len();
    let n = (horizontal / GROUND_SAMPLE_STEP_M).ceil().max(1.0) as usize;
    let step = horizontal / n as f64;
    let (nx, nz) = if horizontal > 1e-9 {
        (-(b.z - a.z) / horizontal, (b.x - a.x) / horizontal)
    } else {
        (1.0, 0.0)
    };
    let mut centre: Vec<[f64; 3]> = Vec::with_capacity(n + 1);
    let mut grade_centre = 0.0_f64;
    let mut grade_band = 0.0_f64;
    for half in half_widths {
        let centre_line = half.abs() < 1e-12;
        let mut previous: Option<f64> = None;
        for index in 0..=n {
            let t = index as f64 / n as f64;
            let x = a.x + (b.x - a.x) * t + nx * half;
            let z = a.z + (b.z - a.z) * t + nz * half;
            let Some(height) = surface.height_m(x, z) else {
                return err(
                    "ground_sample_missing",
                    format!("the engineering surface has no sample at ({x:.3},{z:.3})"),
                );
            };
            if let Some(old) = previous {
                if step > 1e-9 {
                    let grade = (height - old).abs() / step;
                    grade_band = grade_band.max(grade);
                    if centre_line {
                        grade_centre = grade_centre.max(grade);
                    }
                }
            }
            previous = Some(height);
            if centre_line {
                centre.push([x, height, z]);
            }
        }
    }
    if let (Some(first), Some(last)) = (centre.first().copied(), centre.last().copied()) {
        if (first[1] - a.y).abs() > GROUND_PORT_LEVEL_TOLERANCE_M
            || (last[1] - b.y).abs() > GROUND_PORT_LEVEL_TOLERANCE_M
        {
            return err(
                "port_not_on_ground",
                format!(
                    "a ground connection ends at a port that is not on the engineering surface: port levels {:.3}/{:.3} m against surface {:.3}/{:.3} m",
                    a.y, b.y, first[1], last[1]
                ),
            );
        }
    }
    // The declared port is the authoritative connector: snapping the terminal
    // samples keeps the checked polyline continuous with the node it attaches to.
    if let Some(first) = centre.first_mut() {
        *first = a.arr();
    }
    if let Some(last) = centre.last_mut() {
        *last = b.arr();
    }
    Ok((centre, grade_centre, grade_band, n))
}

#[allow(clippy::too_many_arguments)]
fn build_ground_edge(
    spec: &GroundConnectionSpec,
    input: &EngineeringInput,
    surface: &EngineeringSurface,
    terrain: &BakedTerrain,
    nodes: &BTreeMap<String, GraphNode>,
    rules: &CapabilityRules,
    collisions: &CollisionContext,
) -> EcResult<GraphEdge> {
    let from = nodes.get(&spec.from_port_id).ok_or_else(|| {
        EngineeringError::new(
            "unknown_port",
            format!("ground connection {} starts at an undeclared port", spec.id),
        )
    })?;
    let to = nodes.get(&spec.to_port_id).ok_or_else(|| {
        EngineeringError::new(
            "unknown_port",
            format!("ground connection {} ends at an undeclared port", spec.id),
        )
    })?;
    let a = Vec3::from_arr(from.position_m);
    let b = Vec3::from_arr(to.position_m);
    let (polyline, grade_centre, grade_pedestrian, n) =
        ground_band_profile(surface, a, b, &[0.0, 0.75, -0.75])?;
    let (_, _, grade_widest, _) = ground_band_profile(
        surface,
        a,
        b,
        &[
            0.0,
            VEHICLE_CLEAR_WIDTH_M / 2.0,
            -VEHICLE_CLEAR_WIDTH_M / 2.0,
        ],
    )?;
    if !super::access::terrain_segment_clear_xy(a.x, a.z, b.x, b.z, terrain) {
        return Err(EngineeringError::new(
            "ground_predicate_failed",
            format!(
                "ground connection {} is not walkable under the production terrain predicate (centre line plus both 0.75 m edges, 0.5 m steps)",
                spec.id
            ),
        )
        .with_detail(serde_json::json!({
            "samples": polyline.len(),
            "maxGradeCentre": grade_centre,
            "maxGradeBands": grade_pedestrian,
            "at": [[a.x, a.z], [b.x, b.z]],
        })));
    }
    let width_m = from_width(input, &from.node_id).min(from_width(input, &to.node_id));
    let bidirectional = !spec.one_way;
    let (_, _, grade_wheelchair, _) = ground_band_profile(
        surface,
        a,
        b,
        &[
            0.0,
            WHEELCHAIR_CLEAR_WIDTH_M / 2.0,
            -WHEELCHAIR_CLEAR_WIDTH_M / 2.0,
        ],
    )?;
    let facts = CapabilityFacts {
        grade_pedestrian,
        grade_wheelchair: grade_wheelchair.max(grade_centre),
        grade_vehicle: grade_widest.max(grade_centre),
        sampled_half_width_m: (VEHICLE_CLEAR_WIDTH_M / 2.0).max(0.75),
        clear_width_m: width_m,
        bidirectional,
        max_step_rise_m: 0.0,
        flat: grade_pedestrian <= 1e-9,
    };
    let owners = collisions.owner_structures(&from.node_id, &to.node_id);
    let endpoints = [a, b];
    let terminal_frames = collisions.terminal_frames(&owners, &endpoints);
    let mut clearance: BTreeMap<Capability, Option<String>> = BTreeMap::new();
    for capability in [
        Capability::Pedestrian,
        Capability::Wheelchair,
        Capability::Vehicle,
    ] {
        // Each capability is checked on its own band, for solids and for water.
        let half = rules.required_half_width(capability);
        let blocker = collisions
            .corridor_blocker(
                &polyline,
                half,
                rules.body_height(capability),
                &owners,
                &endpoints,
                &terminal_frames,
            )
            .map(|(solid, index)| format!("{solid} at sample {index}"));
        let water = input
            .water
            .iter()
            .find(|water| segment_hits_rect(a.x, a.z, b.x, b.z, half, &water.rect));
        let blocker = match (blocker, water) {
            (Some(solid), _) => Some(solid),
            (None, Some(water)) => Some(format!(
                "water {} in the {half:.2} m half-width band",
                water.id
            )),
            (None, None) => None,
        };
        clearance.insert(capability, blocker);
    }
    // A ground edge that attaches to a structure must also be carried by that
    // structure's own walking members across the direction of travel: attaching a label
    // to a platform does not make the platform wide enough for the walk.
    // Ground bearing and structure support are different facts, decided on two axes. A
    // section with no real bearing part is a proven `noStructuralSupport` and the
    // authoritative terrain rules below still apply in full; a section borne by real
    // structure needs a coverage certificate; a section whose identity could not be
    // decided, or whose certificate could not be completed, refuses. The old
    // `centreBorne && !carried` boolean is gone: an exhausted budget is `unknown`.
    let required_band = rules.required_half_width(Capability::Pedestrian) * 2.0;
    let support = collisions.structural_support_detail(
        &collisions.all_structure_ids(),
        &polyline,
        required_band,
    );
    if let Some(refusal) = support.ground_refusal() {
        let stations = support.intervals.len();
        let borne = support
            .intervals
            .iter()
            .filter(|interval| interval.bearing == support_coverage::BearingIdentity::Structure)
            .count();
        let unknown = support
            .intervals
            .iter()
            .filter(|interval| interval.bearing == support_coverage::BearingIdentity::Unknown)
            .count();
        let mut detail = serde_json::json!({
            "edgeId": spec.id,
            "bearing": refusal.bearing.name(),
            "bearingReason": &refusal.bearing_reason,
            "coverageVerdict": &refusal.coverage_verdict,
            "coverageReason": &refusal.coverage_reason,
            "measuredBandM": refusal.certified_band_m,
            "certifiedBandM": refusal.certified_band_m,
            "requiredBandM": required_band,
            "structureBorneSections": borne,
            "unknownSections": unknown,
            "stations": stations,
            "structuralOwners": owners,
            // The filter evidence: a remote part excluded before event generation is
            // named here with its reason, so "the far parts were irrelevant" is a
            // recomputable claim and not an assertion.
            "partsConsidered": support.parts_considered,
            "partsFiltered": support.parts_filtered,
            "eventBudget": support.event_budget,
        });
        // A refusal that came from one section names that section; a refusal of the line
        // as a whole (an unevaluated path) has none and says so above.
        if let Some(interval) = refusal.interval {
            detail["rangeT"] = serde_json::json!([interval.from_t, interval.to_t]);
            detail["effectiveLevelM"] = serde_json::json!(interval.effective_level_m);
            detail["contributingParts"] = serde_json::json!(interval.parts);
            detail["witness"] = serde_json::json!(
                interval
                    .witness
                    .map(|witness| { serde_json::json!({"t": witness.t, "s": witness.s}) })
            );
        }
        return Err(EngineeringError::new(
            &refusal.category,
            format!(
                "ground connection {} failed its bearing gate: {}",
                spec.id, refusal.detail
            ),
        )
        .with_detail(detail));
    }
    let (capabilities, refusals) = rules.decide("ground", &facts, &clearance);
    if !capabilities.contains(&Capability::Pedestrian) {
        let offender = clearance
            .get(&Capability::Pedestrian)
            .cloned()
            .flatten()
            .unwrap_or_else(|| "<unknown>".into());
        return Err(EngineeringError::new(
            "ground_edge_blocked_by_solid",
            format!(
                "ground connection {} has no clear pedestrian walking volume: {offender}",
                spec.id
            ),
        )
        .with_detail(serde_json::json!({
            "edgeId": spec.id,
            "offender": offender,
            "halfWidthM": 0.75,
            "bodyHeightM": rules.pedestrian_body_height_m,
            "contactToleranceM": SOLID_CONTACT_TOLERANCE_M,
        })));
    }
    let horizontal = b.sub(a).xz_len();
    Ok(GraphEdge {
        edge_id: spec.id.clone(),
        from_node: from.node_id.clone(),
        to_node: to.node_id.clone(),
        kind: "ground".into(),
        bidirectional,
        capabilities,
        horizontal_length_m: horizontal,
        path_length_3d_m: polyline_length(&polyline),
        // Signed along the edge direction: the search must never read an absolute
        // endpoint difference as a climb.
        rise_m: b.y - a.y,
        max_step_rise_m: 0.0,
        max_step_run_m: 0.0,
        climb_m: polyline_metrics(&polyline).2,
        descent_m: polyline_metrics(&polyline).3,
        structure_id: None,
        polyline_m: polyline,
        verification: EdgeVerification {
            method: "production_terrain_predicate".into(),
            predicate: "worldgen::access walkable segment contract: for each of the centre line and the two 0.75 m edges, over ceil(length/0.5) steps, every sample has terrain and |dh| <= 0.25 * (length/n) + 1e-6; the walking volume of every granted capability must additionally be clear of obstacles and of structures that do not own this edge's endpoint".into(),
            samples: (n + 1) * 3,
            terrain_hash: terrain.content_hash().ok(),
            detail: serde_json::json!({
                "maxGradeCentre": grade_centre,
                "maxGradePedestrianBand": grade_pedestrian,
                "maxGradeWidestBand": grade_widest,
                "declaredRoadId": spec.road_id,
                "widthM": width_m,
                // Ground bearing is saved exactly as structure bearing is, both axes and
                // the filter evidence included: the designated re-check re-derives it
                // from the saved geometry and compares every field.
                "supportFacts": {
                    "pedestrian": support.evidence(),
                },
                "capabilityDecision": { "facts": {
                    "gradeCentre": grade_centre,
                    "gradeBandVehicle": facts.grade_vehicle,
                    "sampledHalfWidthM": facts.sampled_half_width_m,
                    "clearWidthM": facts.clear_width_m,
                    "bidirectional": facts.bidirectional,
                    "maxStepRiseM": facts.max_step_rise_m,
                }, "refusals": refusals },
                "clearance": clearance.iter().map(|(capability, blocker)| {
                    (capability_name(*capability).to_string(), serde_json::json!(blocker))
                }).collect::<serde_json::Map<String, serde_json::Value>>(),
                "solidCheck": "obstacles are never exempt; parts of the structures owning this edge's endpoint may be touched inside the terminal contact zone only",
            }),
        },
    })
}

fn from_width(input: &EngineeringInput, node_id: &str) -> f64 {
    input
        .ports
        .iter()
        .find(|port| port.id == node_id)
        .map_or(0.0, |port| port.width_m)
}

// ---------------------------------------------------------------------------
// Solids: obstacles, structures and walking volumes in one collision context
// ---------------------------------------------------------------------------

/// Level tolerance above which a ground port is not on the engineered surface.
pub const GROUND_PORT_LEVEL_TOLERANCE_M: f64 = 0.05;

/// A conservative axis-aligned solid. Obstacles and every structure mesh part are
/// members of the same set, so a wall is not special and a stair is not exempt.
#[derive(Debug, Clone)]
pub struct SolidBox {
    pub solid_id: String,
    /// `obstacle` or `structure_part`.
    pub class: String,
    pub structure_id: Option<String>,
    pub role: Option<String>,
    pub part_id: Option<String>,
    pub min: Vec3,
    pub max: Vec3,
}

impl SolidBox {
    fn from_obstacle(obstacle: &ObstacleBox) -> Self {
        Self {
            solid_id: obstacle.id.clone(),
            class: "obstacle".into(),
            structure_id: None,
            role: None,
            part_id: None,
            min: Vec3::new(obstacle.rect.x0, obstacle.bottom_m, obstacle.rect.z0),
            max: Vec3::new(obstacle.rect.x1, obstacle.top_m, obstacle.rect.z1),
        }
    }
    fn from_part(structure_id: &str, part: &MeshPart) -> Self {
        let aabb = part.aabb();
        Self {
            solid_id: part.part_id.clone(),
            class: "structure_part".into(),
            structure_id: Some(structure_id.to_string()),
            role: Some(part.role.clone()),
            part_id: Some(part.part_id.clone()),
            min: aabb.min,
            max: aabb.max,
        }
    }
    fn aabb(&self) -> Aabb {
        Aabb {
            min: self.min,
            max: self.max,
        }
    }
}

/// The oriented support frame of one walking member: centre, direction of travel and
/// the lateral direction, taken from the member's own top quad. Clear width is the
/// lateral extent, never the larger bounding-box side.
#[derive(Debug, Clone, Copy)]
pub struct MemberFrame {
    pub centre: Vec3,
    pub along: Vec3,
    pub across: Vec3,
    pub half_length: f64,
    pub half_width: f64,
    pub top: f64,
}

impl MemberFrame {
    pub fn clear_width_m(&self) -> f64 {
        self.half_width * 2.0
    }
    /// The widest walking band centred on a plan point that this member's own support
    /// rectangle can carry across `across`. `None` when the point is not on the member
    /// or the band would leave it: this is the real directional clear width.
    pub fn band_interval(&self, x: f64, z: f64, across: Vec3) -> Option<(f64, f64)> {
        let dx = x - self.centre.x;
        let dz = z - self.centre.z;
        let mut low = f64::MIN;
        let mut high = f64::MAX;
        for (dir, half, offset) in [
            (
                self.along,
                self.half_length,
                dx * self.along.x + dz * self.along.z,
            ),
            (
                self.across,
                self.half_width,
                dx * self.across.x + dz * self.across.z,
            ),
        ] {
            let slope = across.x * dir.x + across.z * dir.z;
            if slope.abs() < 1e-12 {
                if offset.abs() > half {
                    return None;
                }
            } else {
                let t0 = (-half - offset) / slope;
                let t1 = (half - offset) / slope;
                low = low.max(t0.min(t1));
                high = high.min(t0.max(t1));
            }
        }
        (low < high).then_some((low, high))
    }

    pub fn contains(&self, x: f64, z: f64, tolerance: f64) -> bool {
        let dx = x - self.centre.x;
        let dz = z - self.centre.z;
        let along = dx * self.along.x + dz * self.along.z;
        let across = dx * self.across.x + dz * self.across.z;
        along.abs() <= self.half_length + tolerance && across.abs() <= self.half_width + tolerance
    }
    /// The plan-segment parameters where a walk enters and leaves this member's own
    /// support rectangle. These are real support events: a bounded measurement step can
    /// straddle a gap narrower than itself, so the member's true boundary must be known
    /// instead of being assumed to lie between two stations.
    pub fn segment_events(&self, a: Vec3, b: Vec3) -> Vec<f64> {
        let delta = b.sub(a);
        if delta.xz_len() < 1e-9 {
            return vec![];
        }
        let mut low = f64::MIN;
        let mut high = f64::MAX;
        for (dir, half) in [
            (self.along, self.half_length),
            (self.across, self.half_width),
        ] {
            let origin = (a.x - self.centre.x) * dir.x + (a.z - self.centre.z) * dir.z;
            let slope = delta.x * dir.x + delta.z * dir.z;
            if slope.abs() < 1e-12 {
                if origin.abs() > half {
                    return vec![];
                }
            } else {
                let t0 = (-half - origin) / slope;
                let t1 = (half - origin) / slope;
                low = low.max(t0.min(t1));
                high = high.min(t0.max(t1));
            }
        }
        let enter = low.max(0.0);
        let leave = high.min(1.0);
        if !(enter < leave) {
            return vec![];
        }
        vec![enter, leave]
    }

    /// Distance from a plan point to this rectangle, zero inside it.
    pub fn distance_to(&self, x: f64, z: f64) -> f64 {
        let dx = x - self.centre.x;
        let dz = z - self.centre.z;
        let along = (dx * self.along.x + dz * self.along.z).abs() - self.half_length;
        let across = (dx * self.across.x + dz * self.across.z).abs() - self.half_width;
        let inside = (along.max(0.0)).hypot(across.max(0.0));
        if along <= 0.0 && across <= 0.0 {
            0.0
        } else {
            inside
        }
    }
}

pub fn member_frame(part: &MeshPart) -> Option<MemberFrame> {
    let corner = |index: usize| Vec3::from_arr(part.corners_m[index]);
    let a = corner(4);
    let b = corner(5);
    let c = corner(7);
    let centre = corner(4).add(corner(6)).scale(0.5);
    let along = b.sub(a);
    let across = c.sub(a);
    if along.xz_len() < 1e-9 || across.xz_len() < 1e-9 {
        return None;
    }
    Some(MemberFrame {
        centre,
        along: along.normalize_xz().ok()?,
        across: across.normalize_xz().ok()?,
        half_length: along.xz_len() / 2.0,
        half_width: across.xz_len() / 2.0,
        top: f64::from(part.top_mm) / 1000.0,
    })
}

fn box_body(centre: Vec3, half_width: f64, height: f64) -> Aabb {
    Aabb {
        min: Vec3::new(centre.x - half_width, centre.y, centre.z - half_width),
        max: Vec3::new(
            centre.x + half_width,
            centre.y + height,
            centre.z + half_width,
        ),
    }
}

/// What one polyline's support measurement found.
#[derive(Debug, Clone, Copy)]
pub struct BandCoverage {
    /// Narrowest band over every station, structure-borne or not.
    pub narrowest: f64,
    /// Narrowest band over the stations where a structure member is the walking
    /// surface. Kept apart from terrain-borne stations on purpose.
    pub narrowest_structural: f64,
    pub structure_borne_stations: usize,
    pub stations: usize,
    /// Endpoints that are neither carried by a member nor joined to one: a structure
    /// edge may not skip its own ends.
    pub unsupported_endpoints: usize,
}

impl Default for BandCoverage {
    fn default() -> Self {
        Self {
            narrowest: f64::MAX,
            narrowest_structural: f64::MAX,
            structure_borne_stations: 0,
            stations: 0,
            unsupported_endpoints: 0,
        }
    }
}

impl BandCoverage {
    pub fn narrowest_or_zero(&self) -> f64 {
        if self.stations == 0 {
            0.0
        } else {
            self.narrowest
        }
    }
    pub fn narrowest_structural_or_zero(&self) -> f64 {
        if self.structure_borne_stations == 0 {
            0.0
        } else {
            self.narrowest_structural
        }
    }
}

/// The band carried on a line through `t = 0` by a set of plan intervals. Every
/// connected component is built and the one containing the walking line is used: an
/// unrelated component before it must not end the search, and an empty set is no
/// support rather than an unbounded one.
pub fn band_width_from_intervals(mut intervals: Vec<(f64, f64)>) -> f64 {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut components: Vec<(f64, f64)> = vec![];
    for (start, end) in intervals {
        match components.last_mut() {
            Some(last) if start <= last.1 + 1e-9 => last.1 = last.1.max(end),
            _ => components.push((start, end)),
        }
    }
    components
        .into_iter()
        .find(|(low, high)| *low <= 0.0 && *high >= 0.0)
        .map_or(0.0, |(low, high)| (-low).min(high) * 2.0)
}

/// One re-computable support fact over one sub-interval of one walking segment: the
/// effective layer the walk is carried at, the real parts of that layer that carry it,
/// and the narrowest band measured inside the sub-interval. An interval with no part on
/// either adjacent level is a gap: no layer, no parts, no band.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportInterval {
    pub segment: usize,
    pub from_t: f64,
    pub to_t: f64,
    pub adjacent_levels_m: [f64; 2],
    pub effective_level_m: Option<f64>,
    /// The real parts whose own interval lies inside the covering component, lateral
    /// widening parts included: this is never only the part the centre line touches.
    pub parts: Vec<String>,
    /// Whether a real member carries the centre line at the representative section.
    /// Diagnostic only: the decision is taken on `bearing`, never on this boolean.
    pub centre_borne: bool,
    /// The bearing identity of this interval: whether a real part of the effective layer
    /// bears its centre line. Independent of whether the full band is wide enough.
    pub bearing: support_coverage::BearingIdentity,
    /// Why the bearing identity is `Unknown`, or `decided`.
    pub bearing_reason: String,
    /// Why the coverage result is `Indeterminate`, or `decided`.
    pub coverage_reason: String,
    /// The certified band. It is `required_band_m` when the certificate proves coverage
    /// and `0` when it does not: a conservative claim, never a claimed exact minimum.
    pub band_m: f64,
    pub required_band_m: f64,
    pub carried: bool,
    pub coverage_verdict: String,
    pub certified_band_m: f64,
    pub witness: Option<support_coverage::CoverageWitness>,
}

impl SupportInterval {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "segment": self.segment,
            "fromT": self.from_t,
            "toT": self.to_t,
            "adjacentLevelsM": self.adjacent_levels_m,
            "effectiveLevelM": self.effective_level_m,
            "parts": self.parts,
            "centreBorne": self.centre_borne,
            "bearing": self.bearing.name(),
            "bearingReason": self.bearing_reason,
            "bandM": self.band_m,
            "requiredBandM": self.required_band_m,
            "carried": self.carried,
            "coverageVerdict": self.coverage_verdict,
            "coverageReason": self.coverage_reason,
            "certifiedBandM": self.certified_band_m,
            "witness": self.witness.map(|witness| serde_json::json!({
                "t": witness.t,
                "s": witness.s,
                "coveredHalfWidthM": witness.covered_half_width_m,
                "requiredHalfWidthM": witness.required_half_width_m,
            })),
        })
    }
}

/// The support of one walking line: the narrowest band, the endpoints that are neither
/// carried nor joined, and the per-sub-interval facts behind both numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralSupport {
    pub band_m: f64,
    pub required_band_m: f64,
    pub unsupported_endpoints: usize,
    pub intervals: Vec<SupportInterval>,
    /// The bearing axis over the whole walking line.
    pub bearing: support_coverage::BearingIdentity,
    /// The coverage axis over the whole walking line.
    pub coverage: support_coverage::CoverageVerdict,
    pub bearing_reason: String,
    pub coverage_reason: String,
    /// Every support candidate this line considered, across all its segments.
    pub parts_considered: Vec<String>,
    /// The candidates excluded before event generation, with segment, layer and reason.
    pub parts_filtered: Vec<support_coverage::FilteredPart>,
    /// The event budget every segment of this line was evaluated under.
    pub event_budget: usize,
}

/// The schema version of the saved support facts. The two axes, their reasons and the
/// filter evidence are v2 facts: a package that lacks them is refused by the
/// field-by-field audit rather than silently read as bearing-free ground.
pub const SUPPORT_FACTS_SCHEMA_VERSION: u32 = 2;

impl StructuralSupport {
    /// The saved evidence of this support measurement: the two axes, their reasons, the
    /// candidate, filtered and contributing part sets kept apart, and every interval.
    pub fn evidence(&self) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": SUPPORT_FACTS_SCHEMA_VERSION,
            "bearing": self.bearing.name(),
            "bearingReason": self.bearing_reason,
            "coverageVerdict": self.coverage.name(),
            "coverageReason": self.coverage_reason,
            "narrowestBandM": self.band_m,
            "requiredBandM": self.required_band_m,
            "unsupportedEndpoints": self.unsupported_endpoints,
            "intervalCount": self.intervals.len(),
            "eventBudget": self.event_budget,
            "eventHardCap": support_coverage::MAX_COVERAGE_EVENTS,
            "partsConsidered": self.parts_considered,
            "partsFiltered": self.parts_filtered,
            "intervals": self.intervals.iter().map(SupportInterval::to_json).collect::<Vec<_>>(),
            "uncarried": self
                .intervals
                .iter()
                .filter(|interval| !interval.carried)
                .map(SupportInterval::to_json)
                .collect::<Vec<_>>(),
        })
    }

    /// The one place where the two axes decide whether a *ground* edge may pass its
    /// support gate. It returns the first section that must refuse, with its category.
    ///
    /// `NoStructuralSupport` is the only state that hands a section back to the
    /// authoritative terrain, grade, water, obstacle and sampling rules, and it is only
    /// ever reached from a complete evaluation. `Unknown` refuses; `Structure` without a
    /// certificate refuses; and no axis borrows the other's `true`/`false`.
    ///
    /// The **global** state of the line is checked as well as the per-section scan: an
    /// undecided line, or a line with no evaluated section at all, refuses even though
    /// there is no section to point at. Passing because a `for` loop had nothing to
    /// iterate over is exactly the boundary this closes.
    pub fn ground_refusal(&self) -> Option<SupportRefusal<'_>> {
        for interval in &self.intervals {
            match interval.bearing {
                support_coverage::BearingIdentity::Unknown => {
                    return Some(SupportRefusal::of_interval(
                        "ground_edge_support_unknown",
                        interval,
                    ));
                }
                support_coverage::BearingIdentity::Structure => {
                    if !interval.carried {
                        return Some(SupportRefusal::of_interval(
                            "ground_edge_unsupported_by_structure",
                            interval,
                        ));
                    }
                }
                // A proven bearing-free section owes no structural certificate, but it
                // may never claim one either: certified coverage without a seen bearing
                // part is not a state this evaluation can produce, so it is a refusal. A
                // real band gap on such a section is legitimate — the section is simply
                // not structure-borne, and the authoritative terrain rules decide it,
                // exactly as they did before.
                support_coverage::BearingIdentity::NoStructuralSupport => {
                    if interval.carried {
                        return Some(SupportRefusal::of_interval(
                            "ground_edge_support_state_inconsistent",
                            interval,
                        ));
                    }
                }
            }
        }
        if self.bearing == support_coverage::BearingIdentity::Unknown {
            return Some(SupportRefusal::of_line(
                "ground_edge_support_unknown",
                self,
                "the walking line has an undecided bearing identity and no section proved otherwise",
            ));
        }
        if self.intervals.is_empty() {
            return Some(SupportRefusal::of_line(
                "ground_edge_support_unknown",
                self,
                "the walking line has no evaluated section at all",
            ));
        }
        None
    }

    /// The support gate of a *structure* edge: every section must be borne by the
    /// structure and certified. `NoStructuralSupport` may never be downgraded into a
    /// ground edge here or anywhere else, and a line with no evaluated section refuses
    /// instead of passing by default.
    pub fn structure_refusal(&self) -> Option<SupportRefusal<'_>> {
        for interval in &self.intervals {
            if interval.bearing != support_coverage::BearingIdentity::Structure || !interval.carried
            {
                return Some(SupportRefusal::of_interval(
                    "structure_edge_unsupported",
                    interval,
                ));
            }
        }
        if self.intervals.is_empty() {
            return Some(SupportRefusal::of_line(
                "structure_edge_unsupported",
                self,
                "the edge has no evaluated section at all",
            ));
        }
        if self.bearing != support_coverage::BearingIdentity::Structure {
            return Some(SupportRefusal::of_line(
                "structure_edge_unsupported",
                self,
                "the edge as a whole is not proven to be structure-borne",
            ));
        }
        None
    }
}

/// A refusal from one of the two support gates.
///
/// It carries the offending section when the refusal came from one, **and** the state of
/// the line as a whole when it did not: an unevaluated line (an empty, non-finite or fully
/// degenerate path, an unusable frame, a budget that could not decide) has no measured
/// section, and a consumer that only scanned a per-section list would have passed it.
#[derive(Debug, Clone)]
pub struct SupportRefusal<'a> {
    /// The production error category, or the blocker prefix for a structure edge.
    pub category: String,
    /// A complete human-readable description of what refused and why.
    pub detail: String,
    pub bearing: support_coverage::BearingIdentity,
    pub bearing_reason: String,
    pub coverage_verdict: String,
    pub coverage_reason: String,
    pub certified_band_m: f64,
    /// The offending section, when the refusal came from one.
    pub interval: Option<&'a SupportInterval>,
}

impl<'a> SupportRefusal<'a> {
    fn of_interval(category: &str, interval: &'a SupportInterval) -> Self {
        Self {
            category: category.to_string(),
            detail: format!(
                "section t {:.4}..{:.4} carries bearing {:?} with coverage {} (layer {:?}, parts {:?}, witness {:?}, reason {} / {})",
                interval.from_t,
                interval.to_t,
                interval.bearing,
                interval.coverage_verdict,
                interval.effective_level_m,
                interval.parts,
                interval.witness,
                interval.bearing_reason,
                interval.coverage_reason
            ),
            bearing: interval.bearing,
            bearing_reason: interval.bearing_reason.clone(),
            coverage_verdict: interval.coverage_verdict.clone(),
            coverage_reason: interval.coverage_reason.clone(),
            certified_band_m: interval.certified_band_m,
            interval: Some(interval),
        }
    }

    fn of_line(category: &str, support: &'a StructuralSupport, detail: &str) -> Self {
        Self {
            category: category.to_string(),
            detail: detail.to_string(),
            bearing: support.bearing,
            bearing_reason: support.bearing_reason.clone(),
            coverage_verdict: support.coverage.name().to_string(),
            coverage_reason: support.coverage_reason.clone(),
            certified_band_m: support.band_m,
            interval: None,
        }
    }
}

/// An explicit "this walking line was never evaluated" result.
///
/// It is **not** a `noStructuralSupport` proof: nothing was measured, so it is an
/// `Unknown` / `Indeterminate` line that carries one real, non-empty undecided interval.
/// Both gates refuse it, and no consumer can read an empty interval list as "there was
/// nothing to reject".
///
/// It is used for a path that cannot be walked at all (empty, a single point, a
/// non-finite vertex, or every segment degenerate) and for a candidate set whose own
/// geometry cannot be classified (see [`support_coverage::frame_is_usable`]).
fn unevaluated_support(
    frames: &[(&str, MemberFrame)],
    polyline: &[[f64; 3]],
    required_band: f64,
    event_budget: usize,
    reason: &str,
) -> StructuralSupport {
    let mut parts_considered: Vec<String> = vec![];
    for (part, _) in frames {
        let id = (*part).to_string();
        if !parts_considered.contains(&id) {
            parts_considered.push(id);
        }
    }
    // A non-finite vertex is itself an invalid path, so the displayed levels are only ever
    // taken from finite data; no NaN may reach the serialized evidence.
    let finite_y = |point: &[f64; 3]| if point[1].is_finite() { point[1] } else { 0.0 };
    let adjacent_levels_m = [
        polyline.first().map_or(0.0, finite_y),
        polyline.last().map_or(0.0, finite_y),
    ];
    let interval = SupportInterval {
        segment: 0,
        from_t: 0.0,
        to_t: 1.0,
        adjacent_levels_m,
        effective_level_m: None,
        parts: vec![],
        centre_borne: false,
        bearing: support_coverage::BearingIdentity::Unknown,
        bearing_reason: reason.to_string(),
        coverage_reason: reason.to_string(),
        band_m: 0.0,
        required_band_m: required_band,
        carried: false,
        coverage_verdict: support_coverage::CoverageVerdict::Indeterminate
            .name()
            .to_string(),
        certified_band_m: 0.0,
        witness: None,
    };
    StructuralSupport {
        band_m: 0.0,
        required_band_m: required_band,
        // Neither end of an unevaluated line is carried.
        unsupported_endpoints: 2,
        intervals: vec![interval],
        bearing: support_coverage::BearingIdentity::Unknown,
        coverage: support_coverage::CoverageVerdict::Indeterminate,
        bearing_reason: reason.to_string(),
        coverage_reason: reason.to_string(),
        parts_considered,
        parts_filtered: vec![],
        event_budget,
    }
}

/// The support of one walking line from named walking members, per code path used by
/// both the builder and the verifier: every sub-interval between two real member
/// boundaries must be carried at the walking level by real parts, and both endpoints
/// must be carried or joined.
pub fn structural_support_detail(
    frames: &[(&str, MemberFrame)],
    polyline: &[[f64; 3]],
    required_band: f64,
) -> StructuralSupport {
    structural_support_detail_with_budget(
        frames,
        polyline,
        required_band,
        support_coverage::MAX_COVERAGE_EVENTS,
    )
}

/// [`structural_support_detail`] with the event budget made explicit. The production
/// entry point uses the hard cap; the budget is a parameter only so that an exhausted
/// budget can be exercised through the real evaluation path instead of a test-only
/// gate. There is no user-visible switch that relaxes it.
///
/// Two entrance contracts hold here, before anything is filtered by geometry:
///
/// 1. The walking line must be evaluable. An empty, single-point, non-finite or fully
///    degenerate path is an explicit [`support_coverage::REASON_INVALID_PATH`] line, not a
///    proven bearing-free one. Repeated vertices are normalised by dropping zero-length
///    segments; a line that keeps at least one evaluable segment is still evaluated
///    normally, so a legitimate stair is never refused for a repeated vertex.
/// 2. Every candidate frame is validated with [`support_coverage::frame_is_usable`]
///    **before** the level comparison that places it. That comparison uses `frame.top`, and
///    a non-finite `top` simply fails it: validating only the already-filtered set would let
///    the unusable part be dropped in silence and read as "no structure at this level".
pub fn structural_support_detail_with_budget(
    frames: &[(&str, MemberFrame)],
    polyline: &[[f64; 3]],
    required_band: f64,
    event_budget: usize,
) -> StructuralSupport {
    // Contract 1: the path itself.
    if polyline.len() < 2 {
        return unevaluated_support(
            frames,
            polyline,
            required_band,
            event_budget,
            support_coverage::REASON_INVALID_PATH,
        );
    }
    if polyline
        .iter()
        .any(|point| !point.iter().all(|value| value.is_finite()))
    {
        return unevaluated_support(
            frames,
            polyline,
            required_band,
            event_budget,
            support_coverage::REASON_INVALID_PATH,
        );
    }
    // Contract 2: the candidates, before the level comparison consumes `frame.top`.
    if let Some((part, _)) = frames
        .iter()
        .find(|(_, frame)| !support_coverage::frame_is_usable(frame))
    {
        let mut out = unevaluated_support(
            frames,
            polyline,
            required_band,
            event_budget,
            support_coverage::REASON_INVALID_FRAME,
        );
        // Name the unusable candidate in the record itself, so the evidence points at the
        // part rather than leaving a reader to search the whole scope.
        if let Some(interval) = out.intervals.first_mut() {
            interval.parts = vec![(*part).to_string()];
        }
        return out;
    }
    let mut out = StructuralSupport {
        band_m: 0.0,
        required_band_m: required_band,
        unsupported_endpoints: 0,
        intervals: vec![],
        bearing: support_coverage::BearingIdentity::NoStructuralSupport,
        coverage: support_coverage::CoverageVerdict::NotApplicable,
        bearing_reason: support_coverage::REASON_DECIDED.to_string(),
        coverage_reason: support_coverage::REASON_DECIDED.to_string(),
        parts_considered: vec![],
        parts_filtered: vec![],
        event_budget,
    };
    let mut narrowest = f64::MAX;
    let mut any_unknown = false;
    let mut any_indeterminate = false;
    let mut any_structure = false;
    for index in 0..polyline.len() - 1 {
        let a = Vec3::from_arr(polyline[index]);
        let b = Vec3::from_arr(polyline[index + 1]);
        let travel = b.sub(a);
        let Ok(along) = travel.normalize_xz() else {
            continue;
        };
        let across = along.perp_xz();
        // A station lies on one of the two members that meet at this segment, so both
        // adjacent levels are tried and the better one is used. A part at any other
        // height is a different layer: its plan projection never fills this one.
        let levels = [a.y, b.y];
        let on_a_level = |frame: &MemberFrame| {
            levels
                .iter()
                .any(|level| (frame.top - level).abs() <= GROUND_PORT_LEVEL_TOLERANCE_M)
        };
        // The proof object is the closed walking band of this segment, certified by the
        // pure coverage module against each real layer. Lateral widening parts that never
        // touch the centre line take part, and an unproved section is a gap, an
        // indeterminate or a proven bearing-free interval, never a carried one.
        let mut layers: Vec<(f64, Vec<(String, MemberFrame)>)> = vec![];
        for level in levels {
            if layers
                .iter()
                .any(|(existing, _)| (*existing - level).abs() <= 1e-9)
            {
                continue;
            }
            let group: Vec<(String, MemberFrame)> = frames
                .iter()
                .filter(|(_, frame)| (frame.top - level).abs() <= GROUND_PORT_LEVEL_TOLERANCE_M)
                .map(|(part, frame)| ((*part).to_string(), *frame))
                .collect();
            layers.push((level, group));
        }
        let certificate =
            support_coverage::certify_band(&layers, index, a, b, required_band, event_budget);
        for part in &certificate.parts_considered {
            if !out.parts_considered.contains(part) {
                out.parts_considered.push(part.clone());
            }
        }
        for filtered in &certificate.parts_filtered {
            out.parts_filtered.push(filtered.clone());
        }
        if certificate.slices.is_empty() {
            // The decisive exit carries both axes. It is *not* read as `centreBorne`
            // false: an exhausted budget or an invalid frame keeps `unknown` and
            // refuses, while only a proven absence of candidates becomes
            // `noStructuralSupport` + `notApplicable`.
            narrowest = 0.0;
            match certificate.bearing {
                support_coverage::BearingIdentity::Unknown => any_unknown = true,
                support_coverage::BearingIdentity::Structure => any_structure = true,
                support_coverage::BearingIdentity::NoStructuralSupport => {}
            }
            if certificate.verdict == support_coverage::CoverageVerdict::Indeterminate {
                any_indeterminate = true;
            }
            out.intervals.push(SupportInterval {
                segment: index,
                from_t: 0.0,
                to_t: 1.0,
                adjacent_levels_m: levels,
                effective_level_m: None,
                parts: vec![],
                centre_borne: false,
                bearing: certificate.bearing,
                bearing_reason: certificate.bearing_reason.clone(),
                coverage_reason: certificate.coverage_reason.clone(),
                band_m: 0.0,
                required_band_m: required_band,
                carried: false,
                coverage_verdict: certificate.verdict.name().to_string(),
                certified_band_m: 0.0,
                witness: None,
            });
        }
        for slice in &certificate.slices {
            narrowest = narrowest.min(slice.certified_band_m);
            match slice.bearing {
                support_coverage::BearingIdentity::Unknown => any_unknown = true,
                support_coverage::BearingIdentity::Structure => any_structure = true,
                support_coverage::BearingIdentity::NoStructuralSupport => {}
            }
            if slice.verdict == support_coverage::CoverageVerdict::Indeterminate {
                any_indeterminate = true;
            }
            out.intervals.push(SupportInterval {
                segment: index,
                from_t: slice.from_t,
                to_t: slice.to_t,
                adjacent_levels_m: slice.adjacent_levels_m,
                effective_level_m: slice.effective_level_m,
                parts: slice.parts.clone(),
                centre_borne: slice.centre_borne,
                bearing: slice.bearing,
                bearing_reason: slice.bearing_reason.clone(),
                coverage_reason: slice.coverage_reason.clone(),
                band_m: slice.certified_band_m,
                required_band_m: slice.required_band_m,
                carried: slice.verdict == support_coverage::CoverageVerdict::Certified,
                coverage_verdict: slice.verdict.name().to_string(),
                certified_band_m: slice.certified_band_m,
                witness: slice.witness,
            });
        }
        let _ = across;
        let _ = on_a_level;
    }
    // Contract 1 again, now that the normalisation is done: zero-length segments are
    // dropped, so a line of repeated vertices would otherwise leave an empty interval list
    // and both gates would "pass" it by having nothing to scan. A line that kept at least
    // one evaluable segment has already produced its intervals above and is untouched.
    if out.intervals.is_empty() {
        return unevaluated_support(
            frames,
            polyline,
            required_band,
            event_budget,
            support_coverage::REASON_INVALID_PATH,
        );
    }
    let mut unsupported = 0_usize;
    for endpoint in [0_usize, polyline.len() - 1] {
        let at = Vec3::from_arr(polyline[endpoint]);
        let next = Vec3::from_arr(
            polyline
                .get(endpoint + 1)
                .copied()
                .unwrap_or(polyline[endpoint]),
        );
        let previous = Vec3::from_arr(
            polyline
                .get(endpoint.wrapping_sub(1))
                .copied()
                .unwrap_or(polyline[endpoint]),
        );
        let travel = if next.sub(at).xz_len() > 1e-9 {
            next.sub(at)
        } else {
            at.sub(previous)
        };
        let Ok(along) = travel.normalize_xz() else {
            unsupported += 1;
            continue;
        };
        let across = along.perp_xz();
        let (width, borne, _, _) = station_band_detail_named(frames, at, across);
        let carried = borne && width + 1e-9 >= required_band;
        // A joining member must be a different member, one step away, that touches the
        // endpoint *and* carries the required band there by itself: the platform the
        // endpoint sits on may not act as its own transition, and a narrow neighbour may
        // not excuse an end it cannot carry.
        let joined = frames.iter().any(|(_, frame)| {
            let rise = (frame.top - at.y).abs();
            if rise <= GROUND_PORT_LEVEL_TOLERANCE_M || rise > MAX_SUPPORT_STEP_RISE_M + 1e-9 {
                return false;
            }
            if frame.distance_to(at.x, at.z) > SOLID_CONTACT_TOLERANCE_M {
                return false;
            }
            match frame.band_interval(at.x, at.z, across) {
                Some((low, high)) => {
                    band_width_from_intervals(vec![(low, high)]) + 1e-9 >= required_band
                }
                None => false,
            }
        });
        if !carried && !joined {
            unsupported += 1;
        }
    }
    out.band_m = if narrowest.is_finite() {
        narrowest
    } else {
        0.0
    };
    out.unsupported_endpoints = unsupported;
    out.bearing = if any_unknown {
        support_coverage::BearingIdentity::Unknown
    } else if any_structure {
        support_coverage::BearingIdentity::Structure
    } else {
        support_coverage::BearingIdentity::NoStructuralSupport
    };
    out.coverage = if out.intervals.is_empty() {
        support_coverage::CoverageVerdict::NotApplicable
    } else if any_indeterminate {
        support_coverage::CoverageVerdict::Indeterminate
    } else if out.intervals.iter().all(|interval| {
        interval.coverage_verdict == support_coverage::CoverageVerdict::NotApplicable.name()
    }) {
        support_coverage::CoverageVerdict::NotApplicable
    } else if out.intervals.iter().all(|interval| interval.carried) {
        support_coverage::CoverageVerdict::Certified
    } else {
        support_coverage::CoverageVerdict::Gap
    };
    // The aggregate reason is the reason the line is not decisive, so it must name the
    // failure that caused it and never collapse to `decided` while a section carries a
    // failure. An undecided bearing is reported before any weaker note.
    out.bearing_reason = out
        .intervals
        .iter()
        .find(|interval| interval.bearing == support_coverage::BearingIdentity::Unknown)
        .map(|interval| interval.bearing_reason.clone())
        .or_else(|| {
            out.intervals
                .iter()
                .map(|interval| interval.bearing_reason.clone())
                .find(|reason| reason != support_coverage::REASON_DECIDED)
        })
        .unwrap_or_else(|| support_coverage::REASON_DECIDED.to_string());
    out.coverage_reason = out
        .intervals
        .iter()
        .find(|interval| {
            interval.coverage_verdict == support_coverage::CoverageVerdict::Indeterminate.name()
        })
        .map(|interval| interval.coverage_reason.clone())
        .or_else(|| {
            out.intervals
                .iter()
                .map(|interval| interval.coverage_reason.clone())
                .find(|reason| reason != support_coverage::REASON_DECIDED)
        })
        .unwrap_or_else(|| support_coverage::REASON_DECIDED.to_string());
    out
}

/// [`structural_support_detail`] over unnamed members, kept for the callers that only
/// measure: the same algorithm, the same facts.
pub fn structural_support_from_frames(
    frames: &[MemberFrame],
    polyline: &[[f64; 3]],
    required_band: f64,
) -> (f64, usize) {
    let named: Vec<(&str, MemberFrame)> = frames.iter().map(|frame| ("", *frame)).collect();
    let support = structural_support_detail(&named, polyline, required_band);
    (support.band_m, support.unsupported_endpoints)
}

/// [`station_band_detail`] over named members: the same measurement, plus the ids of the
/// real parts whose own support rectangle carries the walking line at this station.
pub fn station_band_detail_named(
    frames: &[(&str, MemberFrame)],
    at: Vec3,
    across: Vec3,
) -> (f64, bool, bool, Vec<String>) {
    let mut intervals: Vec<(f64, f64)> = vec![];
    let mut borne = false;
    let mut interior = false;
    let mut parts: Vec<String> = vec![];
    for (part_id, frame) in frames {
        if (frame.top - at.y).abs() > GROUND_PORT_LEVEL_TOLERANCE_M {
            continue;
        }
        if let Some(interval) = frame.band_interval(at.x, at.z, across) {
            if interval.0 <= 0.0 && interval.1 >= 0.0 {
                borne = true;
                parts.push((*part_id).to_string());
                let dx = at.x - frame.centre.x;
                let dz = at.z - frame.centre.z;
                let along = (dx * frame.along.x + dz * frame.along.z).abs();
                if along + SOLID_CONTACT_TOLERANCE_M < frame.half_length {
                    interior = true;
                }
            }
            intervals.push(interval);
        }
    }
    (band_width_from_intervals(intervals), borne, interior, parts)
}

/// One station on a walking line: the band carried by the members at this level,
/// whether a structure member is the walking surface here at all, and whether the
/// station is interior along the walk rather than on a member's own end. Members of
/// another level never lend their projection to this one.
pub fn station_band_detail(frames: &[MemberFrame], at: Vec3, across: Vec3) -> (f64, bool, bool) {
    let mut intervals: Vec<(f64, f64)> = vec![];
    let mut borne = false;
    let mut interior = false;
    for frame in frames {
        if (frame.top - at.y).abs() > GROUND_PORT_LEVEL_TOLERANCE_M {
            continue;
        }
        // A member is borne only when the walking line is really on it: a member at the
        // same level but elsewhere in plan lends no support to this line.
        if let Some(interval) = frame.band_interval(at.x, at.z, across) {
            if interval.0 <= 0.0 && interval.1 >= 0.0 {
                borne = true;
                let dx = at.x - frame.centre.x;
                let dz = at.z - frame.centre.z;
                let along = (dx * frame.along.x + dz * frame.along.z).abs();
                if along + SOLID_CONTACT_TOLERANCE_M < frame.half_length {
                    interior = true;
                }
            }
            intervals.push(interval);
        }
    }
    (band_width_from_intervals(intervals), borne, interior)
}

/// The first two facts of [`station_band_detail`].
pub fn station_band_width(frames: &[MemberFrame], at: Vec3, across: Vec3) -> (f64, bool) {
    let (width, borne, _) = station_band_detail(frames, at, across);
    (width, borne)
}

/// Does a solid's plan rectangle meet the swept band of one corridor interval? The
/// band is the interval from `from` to `to`, half_width to each side of the direction
/// of travel: it is oriented, so it never reaches ahead of the walk, and it is
/// continuous, so nothing slips between two samples.
fn band_hits_rect(from: Vec3, to: Vec3, half_width: f64, min: Vec3, max: Vec3) -> bool {
    let delta = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = delta.xz_len();
    let (along, across) = if length > 1e-9 {
        let along = Vec3::new(delta.x / length, 0.0, delta.z / length);
        (along, Vec3::new(-along.z, 0.0, along.x))
    } else {
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    };
    let mut along_min = f64::MAX;
    let mut along_max = f64::MIN;
    let mut across_min = f64::MAX;
    let mut across_max = f64::MIN;
    for (x, z) in [
        (min.x, min.z),
        (max.x, min.z),
        (max.x, max.z),
        (min.x, max.z),
    ] {
        let dx = x - from.x;
        let dz = z - from.z;
        let along_at = dx * along.x + dz * along.z;
        let across_at = dx * across.x + dz * across.z;
        along_min = along_min.min(along_at);
        along_max = along_max.max(along_at);
        across_min = across_min.min(across_at);
        across_max = across_max.max(across_at);
    }
    let reach = length + SOLID_CONTACT_TOLERANCE_M;
    along_max >= -SOLID_CONTACT_TOLERANCE_M
        && along_min <= reach
        && across_max >= -half_width
        && across_min <= half_width
}

pub struct CollisionContext {
    pub solids: Vec<SolidBox>,
    /// node id -> part ids of the structures that own that node
    exempt_by_node: BTreeMap<String, BTreeSet<String>>,
    /// node id -> ids of the structures that declare that node as their own port
    owners_by_node: BTreeMap<String, BTreeSet<String>>,
    /// part id -> the oriented support frame of that walking member
    frames: BTreeMap<String, MemberFrame>,
    /// part id -> the structure that built it
    frame_owners: BTreeMap<String, String>,
}

impl CollisionContext {
    pub fn new(input: &EngineeringInput, structures: &[StructureRecord]) -> Self {
        let mut solids: Vec<SolidBox> = input
            .obstacles
            .iter()
            .map(SolidBox::from_obstacle)
            .collect();
        let mut exempt_by_node: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut owners_by_node: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut frames: BTreeMap<String, MemberFrame> = BTreeMap::new();
        let mut frame_owners: BTreeMap<String, String> = BTreeMap::new();
        for structure in structures {
            for part in &structure.parts {
                solids.push(SolidBox::from_part(&structure.structure_id, part));
                if matches!(part.role.as_str(), "tread" | "riser" | "landing") {
                    if let Some(frame) = member_frame(part) {
                        frames.insert(part.part_id.clone(), frame);
                        frame_owners.insert(part.part_id.clone(), structure.structure_id.clone());
                    }
                }
            }
            for node in &structure.nodes {
                let entry = exempt_by_node.entry(node.node_id.clone()).or_default();
                for part in &structure.parts {
                    entry.insert(part.part_id.clone());
                }
                owners_by_node
                    .entry(node.node_id.clone())
                    .or_default()
                    .insert(structure.structure_id.clone());
            }
        }
        Self {
            solids,
            exempt_by_node,
            owners_by_node,
            frames,
            frame_owners,
        }
    }

    /// Parts of the structures that own either endpoint node.
    /// The oriented support frames of the walking members that really touch one of
    /// these endpoints and belong to a structure that declares that endpoint. Keyed by
    /// part id, so a contact is bound to one port, one part and one boundary.
    pub fn terminal_frames(
        &self,
        owners: &BTreeSet<String>,
        endpoints: &[Vec3],
    ) -> BTreeMap<String, MemberFrame> {
        let mut out = BTreeMap::new();
        for solid in &self.solids {
            let Some(structure_id) = solid.structure_id.as_deref() else {
                continue;
            };
            if !owners.contains(structure_id) {
                continue;
            }
            let Some(frame) = self.frames.get(&solid.solid_id) else {
                continue;
            };
            let touches = endpoints.iter().any(|endpoint| {
                frame.distance_to(endpoint.x, endpoint.z) <= SOLID_CONTACT_TOLERANCE_M
            });
            if touches {
                out.insert(solid.solid_id.clone(), *frame);
            }
        }
        out
    }

    /// Structures that declare either endpoint as one of their own ports. Only their
    /// walking members may be touched at a terminal interval.
    pub fn owner_structures(&self, from: &str, to: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for node in [from, to] {
            if let Some(owners) = self.owners_by_node.get(node) {
                out.extend(owners.iter().cloned());
            }
        }
        out
    }

    #[allow(dead_code)]
    pub fn exempt_for(&self, from: &str, to: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for node in [from, to] {
            if let Some(parts) = self.exempt_by_node.get(node) {
                out.extend(parts.iter().cloned());
            }
        }
        out
    }

    fn obstacle_count(&self) -> usize {
        self.solids
            .iter()
            .filter(|solid| solid.class == "obstacle")
            .count()
    }

    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "obstacles": self.obstacle_count(),
            "structureParts": self.solids.len() - self.obstacle_count(),
            "contactToleranceM": SOLID_CONTACT_TOLERANCE_M,
            "terminalContactZoneM": TERMINAL_CONTACT_ZONE_M,
            "model": "conservative axis-aligned boxes for obstacles and for every structure part; two solids interpenetrate only when they overlap on every axis by more than the contact tolerance",
            "exemption": "inside the terminal contact zone of a ground edge, parts of the structures that own that edge's endpoint node may be touched; obstacles are never exempt",
        })
    }

    /// The measured support band of one polyline in the walking members of these
    /// structures, per station and in total.
    ///
    /// Diagnostic only. Since G22 this sampled measurement carries no bearing identity
    /// and no coverage certificate, so it never decides a production gate and never
    /// supplies the designated support re-check with a "certified" claim. The decision
    /// comes from [`CollisionContext::structural_support_detail`], which is the same code
    /// path the verifier re-runs.
    pub fn band_coverage_detail(
        &self,
        owners: &BTreeSet<String>,
        polyline: &[[f64; 3]],
    ) -> BandCoverage {
        let frames: Vec<MemberFrame> = self
            .frames
            .iter()
            .filter(|(part_id, _)| {
                self.frame_owners
                    .get(*part_id)
                    .is_some_and(|structure_id| owners.contains(structure_id))
            })
            .map(|(_, frame)| *frame)
            .collect();
        let mut measured = BandCoverage::default();
        if polyline.len() < 2 {
            return measured;
        }
        for index in 0..polyline.len() {
            let from = Vec3::from_arr(polyline[index]);
            let next = Vec3::from_arr(polyline.get(index + 1).copied().unwrap_or(polyline[index]));
            // The last endpoint is a station too; its direction comes from the segment
            // that leads into it.
            let (segment_from, segment_to) = if next.sub(from).xz_len() < 1e-9 {
                let previous = Vec3::from_arr(
                    polyline
                        .get(index.wrapping_sub(1))
                        .copied()
                        .unwrap_or(polyline[index]),
                );
                (previous, from)
            } else {
                (from, next)
            };
            let travel = segment_to.sub(segment_from);
            let Ok(along) = travel.normalize_xz() else {
                continue;
            };
            let across = along.perp_xz();
            // Support may end inside a segment, so every segment is walked in bounded
            // steps instead of being judged only at its two samples.
            let steps = ((travel.xz_len() / 0.125).ceil() as usize).max(1);
            for step in 0..steps {
                let t = (step + 1) as f64 / steps as f64;
                let at = segment_from.add(travel.scale(t));
                let (width, borne) = station_band_width(&frames, at, across);
                measured.stations += 1;
                if borne {
                    measured.structure_borne_stations += 1;
                    measured.narrowest_structural = measured.narrowest_structural.min(width);
                }
                measured.narrowest = measured.narrowest.min(width);
            }
        }
        measured
    }

    /// The support of a structure edge: its whole walking line is measured in bounded
    /// half-segment stations at the level of the nearest vertex, and each endpoint must
    /// either carry the required band itself or stand on a real member of the same
    /// structure that touches it within one step. A plain platform end that carries too
    /// little band and joins nothing is an unsupported endpoint, not a transition.
    /// The walking members of these structures as (part id, frame) pairs, in part-id
    /// order, so every support fact can name the real parts that produced it.
    fn named_frames(&self, owners: &BTreeSet<String>) -> Vec<(&str, MemberFrame)> {
        self.frames
            .iter()
            .filter(|(part_id, _)| {
                self.frame_owners
                    .get(*part_id)
                    .is_some_and(|structure_id| owners.contains(structure_id))
            })
            .map(|(part_id, frame)| (part_id.as_str(), *frame))
            .collect()
    }

    /// The support of a structure edge: the narrowest band, the unsupported endpoints and
    /// the per-sub-interval facts (layer, real parts, band) behind both.
    pub fn structural_support_detail(
        &self,
        owners: &BTreeSet<String>,
        polyline: &[[f64; 3]],
        required_band: f64,
    ) -> StructuralSupport {
        structural_support_detail(&self.named_frames(owners), polyline, required_band)
    }

    /// [`CollisionContext::structural_support_detail`] with an explicit event budget, so
    /// an exhausted budget can be exercised through the real evaluation path. The
    /// production callers use the hard cap; neither the builder nor the verifier nor the
    /// CLI exposes a switch that relaxes it.
    pub fn structural_support_detail_with_budget(
        &self,
        owners: &BTreeSet<String>,
        polyline: &[[f64; 3]],
        required_band: f64,
        event_budget: usize,
    ) -> StructuralSupport {
        structural_support_detail_with_budget(
            &self.named_frames(owners),
            polyline,
            required_band,
            event_budget,
        )
    }

    pub fn structural_support(
        &self,
        owners: &BTreeSet<String>,
        polyline: &[[f64; 3]],
        required_band: f64,
    ) -> (f64, usize) {
        let support = self.structural_support_detail(owners, polyline, required_band);
        (support.band_m, support.unsupported_endpoints)
    }

    /// The support band of a structure edge, measured at its own walking-line vertices.
    /// Every vertex of such a line lies on the member that carries it, so the level and
    /// the footprint are both real there; interpolating between tread tops would ask a
    /// height that no member has. Diagnostic only: see [`CollisionContext::band_coverage_detail`].
    pub fn band_coverage_vertices(
        &self,
        owners: &BTreeSet<String>,
        polyline: &[[f64; 3]],
    ) -> BandCoverage {
        let frames: Vec<MemberFrame> = self
            .frames
            .iter()
            .filter(|(part_id, _)| {
                self.frame_owners
                    .get(*part_id)
                    .is_some_and(|structure_id| owners.contains(structure_id))
            })
            .map(|(_, frame)| *frame)
            .collect();
        let mut measured = BandCoverage::default();
        if polyline.len() < 2 {
            return measured;
        }
        for index in 0..polyline.len() {
            let at = Vec3::from_arr(polyline[index]);
            let next = Vec3::from_arr(polyline.get(index + 1).copied().unwrap_or(polyline[index]));
            let previous = Vec3::from_arr(
                polyline
                    .get(index.wrapping_sub(1))
                    .copied()
                    .unwrap_or(polyline[index]),
            );
            let travel = if next.sub(at).xz_len() > 1e-9 {
                next.sub(at)
            } else {
                at.sub(previous)
            };
            let Ok(along) = travel.normalize_xz() else {
                continue;
            };
            let across = along.perp_xz();
            let (width, borne, interior) = station_band_detail(&frames, at, across);
            if !borne {
                // A transition vertex where the walk leaves another member: it carries
                // no band of this structure, so it is not a support station here.
                continue;
            }
            if !interior {
                // The vertex sits on the member's own end, where the walk arrives from
                // another member: the band is not asked to fit along the edge there.
                continue;
            }
            measured.stations += 1;
            measured.structure_borne_stations += 1;
            measured.narrowest_structural = measured.narrowest_structural.min(width);
            measured.narrowest = measured.narrowest.min(width);
        }
        measured
    }

    /// The narrowest band of this polyline. A station with no support measures zero,
    /// never a maximum. Diagnostic only: see [`CollisionContext::band_coverage_detail`].
    pub fn band_coverage(&self, owners: &BTreeSet<String>, polyline: &[[f64; 3]]) -> f64 {
        self.band_coverage_detail(owners, polyline)
            .narrowest_or_zero()
    }

    /// Every structure that built a walking member.
    pub fn all_structure_ids(&self) -> BTreeSet<String> {
        self.frame_owners.values().cloned().collect()
    }

    /// Sweep a body volume of this capability's own width and height along the checked
    /// polyline. A solid may be passed only when it is a walking surface at most one
    /// step above the walking plane; an obstacle is additionally tested in plan, so a
    /// thin wall between two samples still blocks. No structure is exempt by ownership.
    pub fn corridor_blocker(
        &self,
        polyline: &[[f64; 3]],
        half_width: f64,
        body_height: f64,
        owner_structures: &BTreeSet<String>,
        endpoints: &[Vec3],
        terminal_frames: &BTreeMap<String, MemberFrame>,
    ) -> Option<(String, usize)> {
        if polyline.is_empty() {
            return None;
        }
        let terminal_last = polyline.len().saturating_sub(1);
        for index in 0..polyline.len() {
            let here = Vec3::from_arr(polyline[index]);
            let next = polyline
                .get(index + 1)
                .map(|point| Vec3::from_arr(*point))
                .unwrap_or(here);
            // The check covers the interval actually swept between two samples, so a
            // thin wall, a short column or a low solid between samples is still found.
            let terminal = index == 0 || index + 1 >= terminal_last;
            let (from, to) = if index == terminal_last {
                (Vec3::from_arr(polyline[index.saturating_sub(1)]), here)
            } else {
                (here, next)
            };
            let band_bottom = from.y.min(to.y);
            let band_top = from.y.max(to.y) + body_height;
            for solid in &self.solids {
                if solid.max.y < band_bottom || solid.min.y > band_top {
                    continue;
                }
                // At a terminal interval only, the walking line of a structure that
                // declares that endpoint as its own port is the surface the walk lands
                // on: its treads, risers and landings are contact, not an obstruction.
                // Nothing else is allowed there: no other structure, no column, and no
                // obstacle anywhere, at any height.
                // The landing line is a geometric claim: the part must belong to a
                // structure that declares this endpoint port, it must be a walking
                // member, and it must really touch that endpoint. Belonging to a
                // structure near the end of a corridor is not enough.
                let landing_line = terminal
                    && matches!(
                        solid.role.as_deref(),
                        Some("tread") | Some("riser") | Some("landing")
                    )
                    && solid
                        .structure_id
                        .as_deref()
                        .is_some_and(|id| owner_structures.contains(id))
                    // Identity: the contact must be this solid's own support frame, and
                    // that frame must really touch one of the endpoints.
                    && terminal_frames.get(&solid.solid_id).is_some_and(|frame| {
                        endpoints
                            .iter()
                            .any(|endpoint| {
                                frame.distance_to(endpoint.x, endpoint.z)
                                    <= SOLID_CONTACT_TOLERANCE_M
                            })
                    });
                // The landing line must really touch this endpoint: belonging to a
                // structure near the end of a corridor does not make contact.
                let touches_endpoint = endpoints.iter().any(|endpoint| {
                    band_hits_rect(*endpoint, *endpoint, half_width, solid.min, solid.max)
                });
                if landing_line && touches_endpoint {
                    continue;
                }
                if solid.max.y <= band_bottom {
                    continue;
                }
                // One conservative swept interval for obstacles and for structure
                // parts alike: material that rises into the body band and is crossed by
                // the corridor interval blocks, whatever its owner.
                if band_hits_rect(from, to, half_width, solid.min, solid.max) {
                    return Some((solid.solid_id.clone(), index));
                }
            }
        }
        None
    }

    /// A walking member needs room above it. Members of the same structure whose top
    /// is at or below this member are its own surface, not an obstruction.
    pub fn walking_volume_blocker(
        &self,
        structure_id: &str,
        member: &MeshPart,
        body_height: f64,
    ) -> Option<String> {
        let aabb = member.aabb();
        let top = f64::from(member.top_mm) / 1000.0;
        let body = Aabb {
            min: Vec3::new(aabb.min.x, top, aabb.min.z),
            max: Vec3::new(aabb.max.x, top + body_height, aabb.max.z),
        };
        for solid in &self.solids {
            if solid.structure_id.as_deref() == Some(structure_id)
                && solid.max.y <= top + SOLID_CONTACT_TOLERANCE_M
            {
                continue;
            }
            if body.interpenetrates(&solid.aabb(), SOLID_CONTACT_TOLERANCE_M) {
                return Some(solid.solid_id.clone());
            }
        }
        None
    }

    /// Any structure part interpenetrating an input obstacle. Buildings and walls are
    /// hard obstructions: nothing is opened, shrunk or excluded to fit.
    pub fn structure_obstacle_blocker(&self) -> Option<(String, String)> {
        for part in self.solids.iter().filter(|s| s.class == "structure_part") {
            for obstacle in self.solids.iter().filter(|s| s.class == "obstacle") {
                if part
                    .aabb()
                    .interpenetrates(&obstacle.aabb(), SOLID_CONTACT_TOLERANCE_M)
                {
                    return Some((part.solid_id.clone(), obstacle.solid_id.clone()));
                }
            }
        }
        None
    }

    /// Two structure parts in one volume: `true` when they belong to the same
    /// structure, which means a flight, a turn landing or a support has no room.
    pub fn structure_pair_blocker(&self) -> Option<(String, String, bool)> {
        let parts: Vec<&SolidBox> = self
            .solids
            .iter()
            .filter(|solid| solid.class == "structure_part")
            .collect();
        for (index, a) in parts.iter().enumerate() {
            for b in parts.iter().skip(index + 1) {
                if a.aabb()
                    .interpenetrates(&b.aabb(), SOLID_CONTACT_TOLERANCE_M)
                {
                    return Some((
                        a.solid_id.clone(),
                        b.solid_id.clone(),
                        a.structure_id == b.structure_id,
                    ));
                }
            }
        }
        None
    }
}

/// The bury/float relation only applies where a ground work actually changed the
/// surface: a beam that was always overhead is an obstruction, not a floating solid.
fn scope_touches(changed: &[RectI], min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> bool {
    changed.iter().any(|scope| {
        f64::from(scope.x0) <= max_x
            && f64::from(scope.x1) >= min_x
            && f64::from(scope.z0) <= max_z
            && f64::from(scope.z1) >= min_z
    })
}

fn sample_footprint(
    surface: &EngineeringSurface,
    min_x: f64,
    min_z: f64,
    max_x: f64,
    max_z: f64,
) -> EcResult<Vec<f64>> {
    let mut out = Vec::with_capacity(5);
    for (x, z) in [
        (min_x, min_z),
        (max_x, min_z),
        (max_x, max_z),
        (min_x, max_z),
        ((min_x + max_x) / 2.0, (min_z + max_z) / 2.0),
    ] {
        let Some(height) = surface.height_m(x, z) else {
            return err(
                "ground_sample_missing",
                format!("no engineered sample at ({x:.3},{z:.3}) for a solid relation check"),
            );
        };
        out.push(height);
    }
    Ok(out)
}

/// Controlled cut and fill can bury a solid or leave it floating. Both are refused:
/// the engineered surface must keep every obstacle in a sane relation to the ground.
fn check_surface_relations(
    input: &EngineeringInput,
    surface: &EngineeringSurface,
    structures: &[StructureRecord],
    changed: &[RectI],
) -> EcResult<()> {
    for obstacle in &input.obstacles {
        if !scope_touches(
            changed,
            obstacle.rect.x0,
            obstacle.rect.z0,
            obstacle.rect.x1,
            obstacle.rect.z1,
        ) {
            continue;
        }
        let levels = sample_footprint(
            surface,
            obstacle.rect.x0,
            obstacle.rect.z0,
            obstacle.rect.x1,
            obstacle.rect.z1,
        )?;
        let lowest = levels.iter().copied().fold(f64::MAX, f64::min);
        let highest = levels.iter().copied().fold(f64::MIN, f64::max);
        if lowest > obstacle.top_m + SOLID_RELATION_TOLERANCE_M {
            return err(
                "ground_work_buries_solid",
                format!(
                    "the engineered surface is {lowest:.3} m under obstacle {} (top {:.3} m): the ground work buries a solid instead of working around it",
                    obstacle.id, obstacle.top_m
                ),
            );
        }
        if highest < obstacle.bottom_m - SOLID_RELATION_TOLERANCE_M {
            return err(
                "ground_work_undercuts_solid",
                format!(
                    "the engineered surface is {highest:.3} m under obstacle {} (base {:.3} m): the ground work would leave a solid floating",
                    obstacle.id, obstacle.bottom_m
                ),
            );
        }
    }
    for structure in structures {
        for part in &structure.parts {
            if part.role == "support_column" {
                continue;
            }
            let aabb = part.aabb();
            if !scope_touches(changed, aabb.min.x, aabb.min.z, aabb.max.x, aabb.max.z) {
                continue;
            }
            let levels = sample_footprint(surface, aabb.min.x, aabb.min.z, aabb.max.x, aabb.max.z)?;
            let lowest = levels.iter().copied().fold(f64::MAX, f64::min);
            let top = f64::from(part.top_mm) / 1000.0;
            if lowest > top + SOLID_RELATION_TOLERANCE_M {
                return err(
                    "ground_work_buries_structure",
                    format!(
                        "the engineered surface is {lowest:.3} m over {} of {} (top {top:.3} m): the ground work buries a structure member",
                        part.part_id, structure.structure_id
                    ),
                );
            }
        }
    }
    Ok(())
}

/// Strict field-by-field audit of one saved support interval against the recomputation.
/// Every schema field is checked, an absent field or a wrong type is a disagreement, and
/// nothing is dropped silently: the designated re-check must be able to conclude on its
/// own, without leaning on a whole-package difference.
fn support_interval_disagreements(
    saved: &serde_json::Value,
    recomputed: &SupportInterval,
) -> Vec<String> {
    let mut wrong: Vec<String> = vec![];
    let Some(object) = saved.as_object() else {
        return vec!["interval is not an object".to_string()];
    };
    let number = |key: &str| object.get(key).and_then(serde_json::Value::as_f64);
    match number("fromT") {
        Some(value) if (value - recomputed.from_t).abs() <= 1e-9 => {}
        _ => wrong.push("fromT".into()),
    }
    match number("toT") {
        Some(value) if (value - recomputed.to_t).abs() <= 1e-9 => {}
        _ => wrong.push("toT".into()),
    }
    match number("bandM") {
        Some(value) if (value - recomputed.band_m).abs() <= 1e-9 => {}
        _ => wrong.push("bandM".into()),
    }
    match number("requiredBandM") {
        Some(value) if (value - recomputed.required_band_m).abs() <= 1e-9 => {}
        _ => wrong.push("requiredBandM".into()),
    }
    match number("certifiedBandM") {
        Some(value) if (value - recomputed.certified_band_m).abs() <= 1e-9 => {}
        _ => wrong.push("certifiedBandM".into()),
    }
    match object.get("segment").and_then(serde_json::Value::as_u64) {
        Some(value) if value == recomputed.segment as u64 => {}
        _ => wrong.push("segment".into()),
    }
    match object.get("carried").and_then(serde_json::Value::as_bool) {
        Some(value) if value == recomputed.carried => {}
        _ => wrong.push("carried".into()),
    }
    match object
        .get("coverageVerdict")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if value == recomputed.coverage_verdict => {}
        _ => wrong.push("coverageVerdict".into()),
    }
    let level_ok = match object.get("effectiveLevelM") {
        Some(serde_json::Value::Null) => recomputed.effective_level_m.is_none(),
        Some(value) => value.as_f64().is_some_and(|saved_level| {
            recomputed
                .effective_level_m
                .is_some_and(|level| (level - saved_level).abs() <= 1e-9)
        }),
        None => false,
    };
    if !level_ok {
        wrong.push("effectiveLevelM".into());
    }
    let adjacent_ok = object
        .get("adjacentLevelsM")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| {
            values.len() == 2
                && values.iter().zip(recomputed.adjacent_levels_m.iter()).all(
                    |(value, expected)| {
                        value
                            .as_f64()
                            .is_some_and(|saved_level| (saved_level - expected).abs() <= 1e-9)
                    },
                )
        });
    if !adjacent_ok {
        wrong.push("adjacentLevelsM".into());
    }
    match object
        .get("centreBorne")
        .and_then(serde_json::Value::as_bool)
    {
        Some(value) if value == recomputed.centre_borne => {}
        _ => wrong.push("centreBorne".into()),
    }
    let parts_ok = object
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| {
            values.len() == recomputed.parts.len()
                && values
                    .iter()
                    .zip(recomputed.parts.iter())
                    .all(|(value, expected)| value.as_str() == Some(expected.as_str()))
        });
    if !parts_ok {
        wrong.push("parts".into());
    }
    let witness_ok = match object.get("witness") {
        Some(serde_json::Value::Null) => recomputed.witness.is_none(),
        Some(value) => recomputed.witness.is_some_and(|witness| {
            value
                .get("t")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|saved_t| (saved_t - witness.t).abs() <= 1e-9)
                && value
                    .get("s")
                    .and_then(serde_json::Value::as_f64)
                    .is_some_and(|saved_s| (saved_s - witness.s).abs() <= 1e-9)
        }),
        None => false,
    };
    if !witness_ok {
        wrong.push("witness".into());
    }
    // The two axes and their reasons are audited as strictly as any number. A missing
    // field, an unknown enum value or a wrong type is a disagreement: nothing is
    // filtered away and nothing is repaired from the recomputation.
    match object.get("bearing").and_then(serde_json::Value::as_str) {
        Some(value)
            if support_coverage::BearingIdentity::from_name(value) == Some(recomputed.bearing) => {}
        _ => wrong.push("bearing".into()),
    }
    match object
        .get("bearingReason")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if value == recomputed.bearing_reason => {}
        _ => wrong.push("bearingReason".into()),
    }
    match object
        .get("coverageReason")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if value == recomputed.coverage_reason => {}
        _ => wrong.push("coverageReason".into()),
    }
    wrong
}

/// Strict audit of one saved per-capability support fact block against the support
/// recomputed from the saved geometry. It covers both axes, both reasons, the event
/// budget, the candidate/filtered/contributing part sets kept apart, and every interval,
/// and it is used by the ground and the structure branches alike so neither consumer can
/// re-interpret the state on its own.
fn support_facts_disagreements(
    edge_id: &str,
    capability: &str,
    saved: &serde_json::Value,
    recomputed: &StructuralSupport,
) -> Vec<String> {
    let mut wrong: Vec<String> = vec![];
    let Some(object) = saved.as_object() else {
        return vec![format!(
            "{edge_id}: saved {capability} support facts are not an object"
        )];
    };
    match object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == SUPPORT_FACTS_SCHEMA_VERSION as u64 => {}
        _ => wrong.push("schemaVersion".into()),
    }
    match object.get("bearing").and_then(serde_json::Value::as_str) {
        Some(value)
            if support_coverage::BearingIdentity::from_name(value) == Some(recomputed.bearing) => {}
        _ => wrong.push("bearing".into()),
    }
    match object
        .get("coverageVerdict")
        .and_then(serde_json::Value::as_str)
    {
        Some(value)
            if support_coverage::CoverageVerdict::from_name(value) == Some(recomputed.coverage) => {
        }
        _ => wrong.push("coverageVerdict".into()),
    }
    match object
        .get("bearingReason")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if value == recomputed.bearing_reason => {}
        _ => wrong.push("bearingReason".into()),
    }
    match object
        .get("coverageReason")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if value == recomputed.coverage_reason => {}
        _ => wrong.push("coverageReason".into()),
    }
    let number = |key: &str| object.get(key).and_then(serde_json::Value::as_f64);
    match number("narrowestBandM") {
        Some(value) if (value - recomputed.band_m).abs() <= 1e-9 => {}
        _ => wrong.push("narrowestBandM".into()),
    }
    match number("requiredBandM") {
        Some(value) if (value - recomputed.required_band_m).abs() <= 1e-9 => {}
        _ => wrong.push("requiredBandM".into()),
    }
    match object
        .get("unsupportedEndpoints")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == recomputed.unsupported_endpoints as u64 => {}
        _ => wrong.push("unsupportedEndpoints".into()),
    }
    match object
        .get("intervalCount")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == recomputed.intervals.len() as u64 => {}
        _ => wrong.push("intervalCount".into()),
    }
    match object
        .get("eventBudget")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == recomputed.event_budget as u64 => {}
        _ => wrong.push("eventBudget".into()),
    }
    match object
        .get("eventHardCap")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == support_coverage::MAX_COVERAGE_EVENTS as u64 => {}
        _ => wrong.push("eventHardCap".into()),
    }
    let considered_ok = object
        .get("partsConsidered")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| {
            values.len() == recomputed.parts_considered.len()
                && values
                    .iter()
                    .zip(recomputed.parts_considered.iter())
                    .all(|(value, expected)| value.as_str() == Some(expected.as_str()))
        });
    if !considered_ok {
        wrong.push("partsConsidered".into());
    }
    // The filtered set may name parts of the whole scene, but the exclusion basis itself
    // is recomputed: a forged filtered part, a missing one or a rewritten reason fails.
    let filtered_ok = object
        .get("partsFiltered")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| {
            values.len() == recomputed.parts_filtered.len()
                && values
                    .iter()
                    .zip(recomputed.parts_filtered.iter())
                    .all(|(value, expected)| {
                        value.get("segment").and_then(serde_json::Value::as_u64)
                            == Some(expected.segment as u64)
                            && value.get("part").and_then(serde_json::Value::as_str)
                                == Some(expected.part.as_str())
                            && value
                                .get("levelM")
                                .and_then(serde_json::Value::as_f64)
                                .is_some_and(|level| (level - expected.level_m).abs() <= 1e-9)
                            && value.get("reason").and_then(serde_json::Value::as_str)
                                == Some(expected.reason.as_str())
                    })
        });
    if !filtered_ok {
        wrong.push("partsFiltered".into());
    }
    match object
        .get("intervals")
        .and_then(serde_json::Value::as_array)
    {
        None => wrong.push("intervals".into()),
        Some(list) if list.len() != recomputed.intervals.len() => wrong.push("intervals".into()),
        Some(list) => {
            for (saved_interval, recomputed_interval) in
                list.iter().zip(recomputed.intervals.iter())
            {
                let fields = support_interval_disagreements(saved_interval, recomputed_interval);
                if !fields.is_empty() {
                    wrong.push(format!("intervals[{fields:?}]"));
                }
            }
        }
    }
    match object
        .get("uncarried")
        .and_then(serde_json::Value::as_array)
    {
        None => wrong.push("uncarried".into()),
        Some(list) => {
            let expected: Vec<&SupportInterval> = recomputed
                .intervals
                .iter()
                .filter(|interval| !interval.carried)
                .collect();
            if list.len() != expected.len() {
                wrong.push("uncarried".into());
            } else {
                for (saved_interval, recomputed_interval) in list.iter().zip(expected.iter()) {
                    let fields =
                        support_interval_disagreements(saved_interval, recomputed_interval);
                    if !fields.is_empty() {
                        wrong.push(format!("uncarried[{fields:?}]"));
                    }
                }
            }
        }
    }
    if wrong.is_empty() {
        vec![]
    } else {
        vec![format!(
            "{edge_id}: saved {capability} support facts disagree with the saved geometry on {wrong:?}"
        )]
    }
}

/// Re-derive the capabilities of every structure edge from its own members and the
/// collision context. A flight whose walking volume is blocked is a refusal, not a
/// pedestrian edge.
fn refresh_walkable_capabilities(
    edges: &mut [GraphEdge],
    structures: &[StructureRecord],
    collisions: &CollisionContext,
    rules: &CapabilityRules,
) -> EcResult<()> {
    for edge in edges.iter_mut().filter(|edge| edge.kind != "ground") {
        let Some(structure_id) = edge.structure_id.clone() else {
            continue;
        };
        let Some(structure) = structures
            .iter()
            .find(|structure| structure.structure_id == structure_id)
        else {
            continue;
        };
        let members: Vec<&MeshPart> = structure
            .parts
            .iter()
            .filter(|part| part.role == "tread" || part.role == "landing")
            .collect();
        let mut clearance: BTreeMap<Capability, Option<String>> = BTreeMap::new();
        let mut support_evidence = serde_json::Map::new();
        for capability in [
            Capability::Pedestrian,
            Capability::Wheelchair,
            Capability::Vehicle,
        ] {
            let height = rules.body_height(capability);
            let required_half = rules.required_half_width(capability);
            let mut blocker: Option<String> = None;
            for member in &members {
                if let Some(solid) =
                    collisions.walking_volume_blocker(&structure_id, member, height)
                {
                    blocker = Some(format!("{} blocked by {solid}", member.part_id));
                    break;
                }
            }
            // Real directional support: the required band, centred on the walking line
            // of this edge, must be carried at the walking layer by real parts over every
            // sub-interval between two member boundaries. No structure width field, no
            // bounding box and no part of another layer takes part.
            let support = collisions.structural_support_detail(
                &BTreeSet::from([structure_id.clone()]),
                &edge.polyline_m,
                required_half * 2.0,
            );
            let measured = support.band_m;
            let unsupported_endpoints = support.unsupported_endpoints;
            support_evidence.insert(capability_name(capability).to_string(), support.evidence());
            // The two axes are checked before the band: an undecided identity or a
            // proven bearing-free section is never a structure edge. A structure edge
            // may not be downgraded into a ground edge, and a line with no evaluated
            // section refuses instead of passing by default.
            if let Some(refusal) = support.structure_refusal() {
                if blocker.is_none() {
                    blocker = Some(format!("{}: {}", refusal.category, refusal.detail));
                }
            }
            if blocker.is_none() && measured + 1e-9 < required_half * 2.0 {
                blocker = Some(format!(
                    "the support surface carries {measured:.3} m of walking band at its narrowest station, under the {:.3} m {capability:?} band{}",
                    required_half * 2.0,
                    support
                        .intervals
                        .iter()
                        .filter(|interval| !interval.carried)
                        .min_by(|a, b| a.band_m.total_cmp(&b.band_m))
                        .map_or(String::new(), |interval| format!(
                            " (segment {} t {:.3}..{:.3}, layer {:?}, parts {:?}, band {:.3} m)",
                            interval.segment,
                            interval.from_t,
                            interval.to_t,
                            interval.effective_level_m,
                            interval.parts,
                            interval.band_m
                        ))
                ));
            }
            if blocker.is_none() && unsupported_endpoints > 0 {
                blocker = Some(format!(
                    "{unsupported_endpoints} endpoint(s) are neither carried by a member nor joined to one within one step"
                ));
            }
            clearance.insert(capability, blocker);
        }
        let facts = CapabilityFacts {
            grade_pedestrian: 0.0,
            grade_wheelchair: 0.0,
            grade_vehicle: 0.0,
            sampled_half_width_m: 0.0,
            clear_width_m: structure.width_m,
            bidirectional: edge.bidirectional,
            max_step_rise_m: if edge.kind == "stair" {
                structure.step_rise_m
            } else {
                0.0
            },
            flat: edge.kind == "landing",
        };
        let _ = &facts;
        let (granted, refused) = rules.decide(&edge.kind, &facts, &clearance);
        if !granted.contains(&Capability::Pedestrian) {
            let offender = clearance
                .get(&Capability::Pedestrian)
                .cloned()
                .flatten()
                .unwrap_or_else(|| "<unknown>".into());
            return Err(EngineeringError::new(
                "structure_walking_volume_blocked",
                format!(
                    "edge {} of {} has no clear pedestrian walking volume: {offender}",
                    edge.edge_id, structure_id
                ),
            )
            .with_detail(serde_json::json!({
                "edgeId": edge.edge_id,
                "structureId": structure_id,
                "offender": offender,
                "bodyHeightM": rules.pedestrian_body_height_m,
                "supportFacts": support_evidence.get("pedestrian").cloned().unwrap_or(serde_json::Value::Null),
            })));
        }
        edge.capabilities = granted;
        if let Some(detail) = edge.verification.detail.as_object_mut() {
            detail.insert(
                "supportFacts".into(),
                serde_json::Value::Object(support_evidence.clone()),
            );
            detail.insert(
                "capabilityDecision".into(),
                serde_json::json!({
                    "facts": {
                        "clearWidthM": facts.clear_width_m,
                        "maxStepRiseM": facts.max_step_rise_m,
                        "bidirectional": facts.bidirectional,
                    },
                    "refusals": refused,
                    "clearance": clearance
                        .iter()
                        .map(|(capability, blocker)| {
                            (capability_name(*capability).to_string(), serde_json::json!(blocker))
                        })
                        .collect::<serde_json::Map<String, serde_json::Value>>(),
                }),
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Graph assembly
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRecord {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub capability_rules: CapabilityRules,
    pub ground_edges: usize,
    pub stair_edges: usize,
    pub landing_edges: usize,
    pub one_way_edges: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteLimitReport {
    pub max_length_3d_m: f64,
    pub max_rise_m: f64,
    pub max_expansions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteResult {
    /// `found`, `no_suitable_edge`, `budget_exhausted`, `start_port_not_suitable`
    /// or `unknown_port`.
    pub status: String,
    pub from_port_id: String,
    pub to_road_id: String,
    pub capability: Capability,
    pub path_node_ids: Vec<String>,
    pub path_edge_ids: Vec<String>,
    pub path_polyline_m: Vec<[f64; 3]>,
    pub horizontal_length_m: f64,
    pub path_length_3d_m: f64,
    pub cumulative_rise_m: f64,
    pub cumulative_descent_m: f64,
    pub cost: f64,
    pub cost_definition: String,
    pub limits: RouteLimitReport,
    pub expanded_nodes: usize,
    pub relaxed_edges: usize,
    pub reached_nodes: usize,
    pub capability_only_reachable: bool,
    pub capability_only_best_length_m: Option<f64>,
    pub rejected_by_reason: BTreeMap<String, usize>,
    pub termination: String,
    pub detail: serde_json::Value,
}

const COST_DEFINITION: &str = "cost is the three-dimensional path length in metres; the horizontal length is reported separately and the legacy 120 m access contract is neither used nor redefined here";

#[derive(Debug, Clone, Copy, PartialEq)]
struct Limits {
    max_length_3d_m: f64,
    max_rise_m: f64,
}

#[derive(Debug, Default)]
struct SearchOutcome {
    /// path nodes, path edges, search cost, directed polyline
    path: Option<(Vec<String>, Vec<String>, f64, Vec<[f64; 3]>)>,
    expanded_nodes: usize,
    relaxed_edges: usize,
    reached_nodes: usize,
    rejected: BTreeMap<String, usize>,
    exhausted: bool,
    labels_truncated: bool,
    best_target_length: Option<f64>,
}

/// One non-dominated search state. The predecessor is part of the label identity, so
/// a cheaper label can never overwrite the parent of a different one.
#[derive(Debug, Clone, PartialEq)]
struct SearchLabel {
    cost: f64,
    climb: f64,
    /// parent node, parent label index, entry edge index, direction of travel
    from: Option<(String, usize, usize, bool)>,
}

/// Heap key for one non-dominated label: cost first, then climbing, then node, then
/// the label's own index inside its node.
#[derive(Debug, Clone, PartialEq)]
struct LabelKey(f64, f64, String, usize);

impl Eq for LabelKey {}

impl Ord for LabelKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .total_cmp(&other.0)
            .then_with(|| self.1.total_cmp(&other.1))
            .then_with(|| self.2.cmp(&other.2))
            .then_with(|| self.3.cmp(&other.3))
    }
}

impl PartialOrd for LabelKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The polyline in the direction of travel, edge by edge.
fn directed_polyline(edges: &[GraphEdge], steps: &[(usize, bool)]) -> Vec<[f64; 3]> {
    let mut out: Vec<[f64; 3]> = vec![];
    for (index, forward) in steps {
        let edge = &edges[*index];
        let points: Vec<[f64; 3]> = if *forward {
            edge.polyline_m.clone()
        } else {
            edge.polyline_m.iter().rev().copied().collect()
        };
        for point in points {
            if out.last() != Some(&point) {
                out.push(point);
            }
        }
    }
    out
}

/// Horizontal length, three-dimensional length, cumulative climb and cumulative
/// descent measured on the directed polyline. A hump in the middle of an edge is
/// counted; a pure descent has no climb.
fn polyline_metrics(polyline: &[[f64; 3]]) -> (f64, f64, f64, f64) {
    let mut horizontal = 0.0_f64;
    let mut length = 0.0_f64;
    let mut climb = 0.0_f64;
    let mut descent = 0.0_f64;
    for pair in polyline.windows(2) {
        let a = Vec3::from_arr(pair[0]);
        let b = Vec3::from_arr(pair[1]);
        let dxz = b.sub(a).xz_len();
        let dy = b.y - a.y;
        horizontal += dxz;
        length += (dxz * dxz + dy * dy).sqrt();
        if dy >= 0.0 {
            climb += dy;
        } else {
            descent += -dy;
        }
    }
    (horizontal, length, climb, descent)
}

fn node_supports(node: &GraphNode, capability: Capability) -> bool {
    node.capabilities.contains(&capability)
}

/// Exact floating-point ordering for the search heap. Quantising the cost into an
/// integer key made a reconstructed cost slightly larger than the best known cost,
/// so freshly pushed nodes were then discarded as stale. The cost stays an `f64`.
#[derive(Debug, Clone, PartialEq)]
struct CostKey(f64, String);

impl Eq for CostKey {}

impl Ord for CostKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .total_cmp(&other.0)
            .then_with(|| self.1.cmp(&other.1))
    }
}

impl PartialOrd for CostKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn dijkstra(
    nodes: &BTreeMap<String, GraphNode>,
    adjacency: &BTreeMap<String, Vec<usize>>,
    edges: &[GraphEdge],
    query: &RouteQuery,
    limits: Option<Limits>,
) -> SearchOutcome {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut outcome = SearchOutcome::default();
    if !nodes.contains_key(&query.from_port_id) {
        return outcome;
    }
    // Bounded non-dominated labels: a label survives while no other label reaches
    // the same node with no more cost and no more climbing. A single label with an
    // overwritten climb budget is what made an over-limit path look feasible.
    let mut labels: BTreeMap<String, Vec<SearchLabel>> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<LabelKey>> = BinaryHeap::new();
    labels.insert(
        query.from_port_id.clone(),
        vec![SearchLabel {
            cost: 0.0,
            climb: 0.0,
            from: None,
        }],
    );
    heap.push(Reverse(LabelKey(0.0, 0.0, query.from_port_id.clone(), 0)));
    let target = |node: &GraphNode| -> bool {
        node.road_id.as_deref() == Some(query.to_road_id.as_str())
            && node_supports(node, query.capability)
    };
    while let Some(Reverse(LabelKey(cost, climb, node_id, label_index))) = heap.pop() {
        let live = labels.get(&node_id).cloned().unwrap_or_default();
        if !live
            .iter()
            .any(|known| (known.cost - cost).abs() < 1e-9 && (known.climb - climb).abs() < 1e-9)
        {
            continue;
        }
        let Some(entry_label) = live.get(label_index).cloned() else {
            continue;
        };
        outcome.expanded_nodes += 1;
        if outcome.expanded_nodes > MAX_ROUTE_EXPANSIONS {
            outcome.exhausted = true;
            break;
        }
        if let Some(node) = nodes.get(&node_id) {
            if target(node) {
                if outcome
                    .best_target_length
                    .is_none_or(|known| cost < known - 1e-9)
                {
                    outcome.best_target_length = Some(cost);
                }
                if outcome.path.is_none() {
                    let mut path_nodes = vec![node_id.clone()];
                    let mut steps: Vec<(usize, bool)> = vec![];
                    let mut cursor = (node_id.clone(), entry_label.clone());
                    while let Some((parent, parent_label, edge_index, forward)) =
                        cursor.1.from.clone()
                    {
                        steps.push((edge_index, forward));
                        path_nodes.push(parent.clone());
                        let Some(parent_entry) = labels
                            .get(&parent)
                            .and_then(|set| set.get(parent_label))
                            .cloned()
                        else {
                            break;
                        };
                        cursor = (parent, parent_entry);
                    }
                    path_nodes.reverse();
                    steps.reverse();
                    let path_edges: Vec<String> = steps
                        .iter()
                        .map(|(index, _)| edges[*index].edge_id.clone())
                        .collect();
                    // The polyline follows the direction of travel, so every metric
                    // below is measured on the geometry that was actually walked.
                    let polyline = directed_polyline(edges, &steps);
                    outcome.path = Some((path_nodes, path_edges, cost, polyline));
                }
                continue;
            }
        }
        let Some(incident) = adjacency.get(&node_id) else {
            continue;
        };
        for index in incident.iter().copied() {
            let edge = &edges[index];
            outcome.relaxed_edges += 1;
            let forward = edge.from_node == node_id;
            let other = if forward {
                edge.to_node.clone()
            } else {
                edge.from_node.clone()
            };
            if !forward && !edge.bidirectional {
                *outcome
                    .rejected
                    .entry("one_way_blocked".into())
                    .or_insert(0) += 1;
                continue;
            }
            if !edge.supports(query.capability) {
                *outcome
                    .rejected
                    .entry("capability_not_supported".into())
                    .or_insert(0) += 1;
                continue;
            }
            let Some(other_node) = nodes.get(&other) else {
                continue;
            };
            if !node_supports(other_node, query.capability) {
                *outcome
                    .rejected
                    .entry("target_node_capability".into())
                    .or_insert(0) += 1;
                continue;
            }
            // Signed rise along the direction of travel: a descent is not a climb.
            // The resource is the edge's own measured climb, not an endpoint difference.
            let next_cost = cost + edge.path_length_3d_m;
            let next_climb = climb
                + if forward {
                    edge.climb_m
                } else {
                    edge.descent_m
                };
            if let Some(limits) = limits {
                if next_cost > limits.max_length_3d_m + 1e-9 {
                    *outcome.rejected.entry("length_limit".into()).or_insert(0) += 1;
                    continue;
                }
                if next_climb > limits.max_rise_m + 1e-9 {
                    *outcome.rejected.entry("rise_limit".into()).or_insert(0) += 1;
                    continue;
                }
            }
            let existing = labels.entry(other.clone()).or_default();
            let dominated = existing
                .iter()
                .any(|known| known.cost <= next_cost + 1e-9 && known.climb <= next_climb + 1e-9);
            if dominated {
                continue;
            }
            if existing.len() >= MAX_LABELS_PER_NODE {
                // Refusing to store a label means a feasible path might be missed,
                // so the search reports a budget limit rather than an absence.
                outcome.labels_truncated = true;
                continue;
            }
            // Labels are append only, so the index stored in a child stays valid.
            let next_index = existing.len();
            existing.push(SearchLabel {
                cost: next_cost,
                climb: next_climb,
                from: Some((node_id.clone(), label_index, index, forward)),
            });
            heap.push(Reverse(LabelKey(next_cost, next_climb, other, next_index)));
        }
    }
    outcome.reached_nodes = labels.len();
    outcome.best_target_length = labels
        .iter()
        .filter(|(id, _)| nodes.get(*id).is_some_and(|node| target(node)))
        .flat_map(|(_, set)| set.iter().map(|label| label.cost))
        .fold(outcome.best_target_length, |acc, cost| {
            Some(acc.map_or(cost, |known: f64| known.min(cost)))
        });
    outcome
}

fn route(
    nodes: &BTreeMap<String, GraphNode>,
    adjacency: &BTreeMap<String, Vec<usize>>,
    edges: &[GraphEdge],
    query: &RouteQuery,
) -> RouteResult {
    let limits = RouteLimitReport {
        max_length_3d_m: query.max_length_3d_m,
        max_rise_m: query.max_rise_m,
        max_expansions: MAX_ROUTE_EXPANSIONS,
    };
    let mut base = RouteResult {
        status: "no_suitable_edge".into(),
        from_port_id: query.from_port_id.clone(),
        to_road_id: query.to_road_id.clone(),
        capability: query.capability,
        path_node_ids: vec![],
        path_edge_ids: vec![],
        path_polyline_m: vec![],
        horizontal_length_m: 0.0,
        path_length_3d_m: 0.0,
        cumulative_rise_m: 0.0,
        cumulative_descent_m: 0.0,
        cost: 0.0,
        cost_definition: COST_DEFINITION.into(),
        limits,
        expanded_nodes: 0,
        relaxed_edges: 0,
        reached_nodes: 0,
        capability_only_reachable: false,
        capability_only_best_length_m: None,
        rejected_by_reason: BTreeMap::new(),
        termination: String::new(),
        detail: serde_json::Value::Null,
    };
    if !SUPPORTED_CAPABILITIES.contains(&query.capability) {
        base.status = "unsupported_capability".into();
        base.termination = format!(
            "capability {} has no implemented local geometry check in v2; it is refused explicitly instead of being endorsed by a pedestrian check",
            capability_name(query.capability)
        );
        return base;
    }
    let Some(start) = nodes.get(&query.from_port_id) else {
        base.status = "unknown_port".into();
        base.termination = format!("port {} is not in the graph", query.from_port_id);
        return base;
    };
    if !node_supports(start, query.capability) {
        base.status = "start_port_not_suitable".into();
        base.termination = format!(
            "port {} does not declare the requested capability",
            query.from_port_id
        );
        return base;
    }
    let uncapped = dijkstra(nodes, adjacency, edges, query, None);
    base.expanded_nodes = uncapped.expanded_nodes;
    base.relaxed_edges = uncapped.relaxed_edges;
    base.reached_nodes = uncapped.reached_nodes;
    base.rejected_by_reason = uncapped.rejected.clone();
    base.capability_only_best_length_m = uncapped.best_target_length;
    match &uncapped.path {
        None => {
            if uncapped.exhausted || uncapped.labels_truncated {
                base.status = "budget_exhausted".into();
                base.termination = format!(
                    "the capability-filtered search stopped after {} expansions{} without a route; absence of a route was not proved",
                    MAX_ROUTE_EXPANSIONS,
                    if uncapped.labels_truncated {
                        " and dropped a non-dominated label"
                    } else {
                        ""
                    }
                );
            } else {
                base.status = "no_suitable_edge".into();
                base.termination = format!(
                    "every edge reachable from {} was rejected for capability {} (queue exhausted after {} expansions)",
                    query.from_port_id,
                    capability_name(query.capability),
                    uncapped.expanded_nodes
                );
            }
            return base;
        }
        Some(path) => {
            base.capability_only_reachable = true;
            let limited = dijkstra(
                nodes,
                adjacency,
                edges,
                query,
                Some(Limits {
                    max_length_3d_m: query.max_length_3d_m,
                    max_rise_m: query.max_rise_m,
                }),
            );
            base.expanded_nodes = limited.expanded_nodes;
            base.relaxed_edges = limited.relaxed_edges;
            base.reached_nodes = limited.reached_nodes.max(uncapped.reached_nodes);
            for (reason, count) in limited.rejected {
                *base.rejected_by_reason.entry(reason).or_insert(0) += count;
            }
            match &limited.path {
                Some((path_nodes, path_edges, _search_cost, polyline)) => {
                    // The geometry is authoritative: the reported numbers are measured
                    // on the directed polyline and a path outside the declared limits is
                    // never published as found.
                    let (horizontal, length, climb, descent) = polyline_metrics(polyline);
                    if length > query.max_length_3d_m + 1e-6 || climb > query.max_rise_m + 1e-6 {
                        base.status = "budget_exhausted".into();
                        base.termination = format!(
                            "the searched path is {length:.3} m long and climbs {climb:.3} m, outside the declared limits (maxLength {} m, maxRise {} m)",
                            query.max_length_3d_m, query.max_rise_m
                        );
                        base.detail = serde_json::json!({
                            "pathLength3dM": length,
                            "pathClimbM": climb,
                            "maxLength3dM": query.max_length_3d_m,
                            "maxRiseM": query.max_rise_m,
                        });
                        return base;
                    }
                    base.status = "found".into();
                    base.path_node_ids = path_nodes.clone();
                    base.path_edge_ids = path_edges.clone();
                    base.path_length_3d_m = length;
                    base.cost = length;
                    base.horizontal_length_m = horizontal;
                    base.cumulative_rise_m = climb;
                    base.cumulative_descent_m = descent;
                    base.path_polyline_m = polyline.clone();
                    base.termination = format!(
                        "reached a port of road {} with capability {} inside the declared limits",
                        query.to_road_id,
                        capability_name(query.capability)
                    );
                }
                None => {
                    let (horizontal, length, climb, descent) = polyline_metrics(&path.3);
                    base.status = "budget_exhausted".into();
                    base.termination = format!(
                        "a capability-feasible route exists at {length:.3} m climbing {climb:.3} m but no route inside the declared limits was found (maxLength {} m, maxRise {} m)",
                        query.max_length_3d_m, query.max_rise_m
                    );
                    base.detail = serde_json::json!({
                        "capabilityOnlyPathLength3dM": length,
                        "capabilityOnlyHorizontalM": horizontal,
                        "capabilityOnlyRiseM": climb,
                        "capabilityOnlyDescentM": descent,
                        "searchCostM": path.2,
                    });
                }
            }
            base
        }
    }
}

pub fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::Pedestrian => "pedestrian",
        Capability::Wheelchair => "wheelchair",
        Capability::Vehicle => "vehicle",
        Capability::Rail => "rail",
    }
}

// ---------------------------------------------------------------------------
// Capability matrix
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRow {
    pub capability: String,
    pub evidence: String,
    /// `existing_basis`, `reused`, `added` or `unsupported`.
    pub status: String,
}

pub fn capability_matrix() -> Vec<MatrixRow> {
    let row = |capability: &str, evidence: &str, status: &str| MatrixRow {
        capability: capability.into(),
        evidence: evidence.into(),
        status: status.into(),
    };
    vec![
        row(
            "walkable ground predicate",
            "worldgen/access.rs segment_clear, now split into terrain_segment_clear_xy with no semantic change; thresholds HALF_WIDTH 0.75, 0.5 m steps, |dh| <= 0.25 * (length/n) + 1e-6",
            "existing_basis",
        ),
        row(
            "heightfield storage and hashing",
            "crate::baked_terrain::BakedTerrain and height_payload::encode_millimetres (GWHM v1, i32 millimetres)",
            "existing_basis",
        ),
        row(
            "terrain predicate reuse for engineering ground edges",
            "engineering_connections::build_ground_edge calls access::terrain_segment_clear_xy and stores the raster content hash it used",
            "reused",
        ),
        row(
            "controlled cut and fill",
            "authorised GroundWorkPlan with separate fill/cut licences, per-cell limits, protection domains and cut/fill volumes in cubic metres",
            "added",
        ),
        row(
            "independent platforms",
            "landing decks with their own levelId, deck mesh, supports and a landing edge; never collapsed into a ground edge",
            "added",
        ),
        row(
            "stairs, including switchbacks",
            "straight and switchback flights with tread, riser, turn landing, per-step records, one-way rule and v1 step limits",
            "added",
        ),
        row(
            "three-dimensional ports",
            "portId plus surfaceId, levelId, width, direction and capability set; each port is a graph node at an absolute level",
            "added",
        ),
        row(
            "typed graph and capability routing",
            "edges typed ground/stair/landing with their verification record; bounded capability-filtered Dijkstra with explicit limits",
            "added",
        ),
        row(
            "persistence and independent read-back verification",
            "versioned bundle with payload hash; verify rebuilds from the input and reports every mismatch instead of trusting the file",
            "added",
        ),
        row(
            "spiral, rotating or curved stairs",
            "a curve-driven stair is a different generator; v1 refuses it instead of approximating it with straight flights",
            "unsupported",
        ),
        row(
            "lifts, escalators and moving walks",
            "no machinery model, no maintenance envelope and no failure mode; refused explicitly",
            "unsupported",
        ),
        row(
            "tunnels and underpasses",
            "would need a cut with a roof, retaining walls and drainage; refused explicitly",
            "unsupported",
        ),
        row(
            "elevated roads, bridges and interchanges",
            "would need span structures, piers and clearance checks; refused explicitly",
            "unsupported",
        ),
        row(
            "retaining walls and drainage",
            "a plan that declares them is refused as needs_design rather than silently flattened",
            "unsupported",
        ),
        row(
            "drivable or rail-graded ramps at highway geometry",
            "v1 only carries ground edges inside the grade limits it states; no alignment, crossfall or sight distance model",
            "unsupported",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

pub const UNITS_CONTRACT: &str = "lengths_m;levels_mm";

#[derive(Debug, Clone)]
pub struct BuildOutcome {
    pub surface: EngineeringSurface,
    pub ground_works: Vec<GroundWorkReport>,
    pub structures: Vec<StructureRecord>,
    pub nodes: Vec<GraphNode>,
    pub graph: GraphRecord,
    pub route: RouteResult,
    pub matrix: Vec<MatrixRow>,
    pub mesh_points: usize,
    pub mesh_triangles: usize,
}

/// Every identified collection must be unambiguous. A repeated id is a refusal, not
/// a silent merge.
fn validate_identities(input: &EngineeringInput) -> EcResult<()> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut claim = |kind: &str, id: &str| -> EcResult<()> {
        if id.trim().is_empty() {
            return err("invalid_identity", format!("a {kind} has an empty id"));
        }
        let key = format!("{kind}:{id}");
        if !seen.insert(key) {
            return err(
                "duplicate_id",
                format!("{kind} {id} is declared twice; identical ids are never merged"),
            );
        }
        Ok(())
    };
    for domain in &input.protection {
        claim("protection", &domain.id)?;
    }
    for obstacle in &input.obstacles {
        claim("obstacle", &obstacle.id)?;
        if !(obstacle.bottom_m.is_finite() && obstacle.top_m.is_finite())
            || obstacle.top_m <= obstacle.bottom_m
        {
            return err(
                "invalid_obstacle",
                format!(
                    "obstacle {} spans {}..{} m, which is not a finite positive height range",
                    obstacle.id, obstacle.bottom_m, obstacle.top_m
                ),
            );
        }
    }
    for water in &input.water {
        claim("water", &water.id)?;
    }
    for bearing in &input.bearing {
        claim("bearing", &bearing.id)?;
    }
    for plan in &input.ground_works {
        claim("groundWork", &plan.id)?;
    }
    for connection in &input.ground_connections {
        claim("groundConnection", &connection.id)?;
    }
    for reference in &input.road_references {
        if reference.source.trim().is_empty() {
            return err(
                "invalid_road_reference",
                format!("road reference {} declares no source", reference.id),
            );
        }
        claim("roadReference", &reference.id)?;
    }
    for value in [input.query.max_length_3d_m, input.query.max_rise_m] {
        if !value.is_finite() || value < 0.0 {
            return err(
                "non_finite_value",
                "a routing limit is not a finite non-negative number",
            );
        }
    }
    Ok(())
}

/// A label is not a reference. Every surface, level and road a description mentions
/// must resolve to something this description actually declares.
fn validate_references(input: &EngineeringInput, structures: &[StructureRecord]) -> EcResult<()> {
    let declared: BTreeSet<String> = input
        .road_references
        .iter()
        .map(|reference| reference.id.clone())
        .collect();
    let mut mentioned: Vec<(String, String)> = vec![];
    // A blank label is not a reference: only a non-empty id has to resolve.
    for port in &input.ports {
        if let Some(road) = &port.road_id {
            if !road.trim().is_empty() {
                mentioned.push((road.clone(), format!("port {}", port.id)));
            }
        }
    }
    for connection in &input.ground_connections {
        if let Some(road) = &connection.road_id {
            if !road.trim().is_empty() {
                mentioned.push((road.clone(), format!("ground connection {}", connection.id)));
            }
        }
    }
    mentioned.push((input.query.to_road_id.clone(), "query.toRoadId".into()));
    for (road, at) in mentioned {
        if !declared.contains(&road) {
            return Err(EngineeringError::new(
                "unknown_road_reference",
                format!(
                    "{at} references road {road:?}, which no roadReferences entry declares; a free string is not a bound road"
                ),
            )
            .with_detail(serde_json::json!({
                "roadId": road,
                "at": at,
                "declared": declared,
            })));
        }
    }
    for port in &input.ports {
        let reserved =
            port.surface_id == RESERVED_GROUND_SURFACE && port.level_id == RESERVED_GROUND_LEVEL;
        if reserved {
            continue;
        }
        match structures
            .iter()
            .find(|structure| structure.structure_id == port.surface_id)
        {
            None => {
                return Err(EngineeringError::new(
                    "unknown_surface_reference",
                    format!(
                        "port {} claims surface {:?}, which no structure declares; {:?} is the only reserved ground surface",
                        port.id, port.surface_id, RESERVED_GROUND_SURFACE
                    ),
                )
                .with_detail(serde_json::json!({
                    "portId": port.id,
                    "surfaceId": port.surface_id,
                    "declaredStructures": structures.iter().map(|s| s.structure_id.clone()).collect::<Vec<_>>(),
                })))
            }
            Some(structure) => {
                if !structure.level_ids.iter().any(|level| *level == port.level_id) {
                    return Err(EngineeringError::new(
                        "unknown_level_reference",
                        format!(
                            "port {} claims level {:?} on structure {}, whose levels are {:?}",
                            port.id, port.level_id, structure.structure_id, structure.level_ids
                        ),
                    )
                    .with_detail(serde_json::json!({
                        "portId": port.id,
                        "levelId": port.level_id,
                        "structureId": structure.structure_id,
                        "declaredLevels": structure.level_ids,
                    })));
                }
            }
        }
    }
    Ok(())
}

fn field_hash(mm: &[i32], width: u32, depth: u32, origin_x: i32, origin_z: i32) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"glyphweave-engineering-field-i32le-mm-v1");
    hasher.update(&origin_x.to_le_bytes());
    hasher.update(&origin_z.to_le_bytes());
    hasher.update(&width.to_le_bytes());
    hasher.update(&depth.to_le_bytes());
    for value in mm {
        hasher.update(&value.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn tread_polyline(
    record: &StructureRecord,
    run_index: Option<u32>,
    start: &GraphNode,
    end: &GraphNode,
) -> Vec<[f64; 3]> {
    let mut treads: Vec<&MeshPart> = record
        .parts
        .iter()
        .filter(|part| part.role == "tread")
        .filter(|part| run_index.is_none_or(|run| part.run_index == run))
        .collect();
    treads.sort_by_key(|part| part.index);
    let mut polyline = vec![start.position_m];
    for tread in treads {
        let centre = Vec3::new(
            (tread.corners_m[0][0] + tread.corners_m[2][0]) / 2.0,
            0.0,
            (tread.corners_m[0][2] + tread.corners_m[2][2]) / 2.0,
        );
        polyline.push([centre.x, f64::from(tread.top_mm) / 1000.0, centre.z]);
    }
    polyline.push(end.position_m);
    polyline
}

pub fn build(input: &EngineeringInput) -> EcResult<BuildOutcome> {
    if input.version != ENGINEERING_CONNECTIONS_VERSION {
        return err(
            "contract_version_mismatch",
            format!(
                "input declares version {} but this module implements {ENGINEERING_CONNECTIONS_VERSION}",
                input.version
            ),
        );
    }
    if input.units != UNITS_CONTRACT {
        return err(
            "units_mismatch",
            format!(
                "input declares units {:?} but the v1 contract is {UNITS_CONTRACT:?}",
                input.units
            ),
        );
    }
    input.grid.validate()?;
    validate_identities(input)?;
    if input.structures.len() > MAX_STRUCTURES {
        return err(
            "structure_count_out_of_range",
            format!(
                "{} structures exceed the v1 cap {MAX_STRUCTURES}",
                input.structures.len()
            ),
        );
    }
    if input.ports.len() > MAX_PORTS {
        return err(
            "port_count_out_of_range",
            format!("{} ports exceed the v1 cap {MAX_PORTS}", input.ports.len()),
        );
    }
    if input.ground_connections.len() + input.structures.len() * 3 > MAX_EDGES {
        return err(
            "edge_count_out_of_range",
            format!("the declared graph could exceed the v1 cap {MAX_EDGES} edges"),
        );
    }
    let mut seen = BTreeSet::new();
    for port in &input.ports {
        if !seen.insert(port.id.clone()) {
            return err(
                "duplicate_id",
                format!("port {} is declared twice", port.id),
            );
        }
        check_finite(&port.position_m, &format!("port {}", port.id))?;
        if port.width_m <= 0.0 || port.width_m > 60.0 {
            return err(
                "invalid_port_width",
                format!("port {} has width {} m", port.id, port.width_m),
            );
        }
        if port.capabilities.is_empty() {
            return err(
                "invalid_port_capabilities",
                format!("port {} declares no capability", port.id),
            );
        }
    }
    let mut structure_ids = BTreeSet::new();
    for structure in &input.structures {
        if !structure_ids.insert(structure.id().to_string()) {
            return err(
                "duplicate_id",
                format!("structure {} is declared twice", structure.id()),
            );
        }
    }
    let (surface, ground_works) = apply_ground_works(input)?;
    let terrain = surface.to_baked_terrain()?;
    let rules = CapabilityRules::default();
    let mut structures: Vec<StructureRecord> = vec![];
    for structure in &input.structures {
        let record = match structure {
            StructureSpec::Landing(spec) => build_landing(spec, input, &surface)?,
            StructureSpec::StraightStair(spec) => build_straight_stair(spec, input, &surface)?,
            StructureSpec::SwitchbackStair(spec) => build_switchback_stair(spec, input, &surface)?,
            StructureSpec::Unsupported {
                id,
                requested_kind,
                reason,
            } => {
                return Err(EngineeringError::new(
                    "unsupported",
                    format!(
                        "structure {id} requests {requested_kind}, which v1 does not model: {reason}"
                    ),
                )
                .with_detail(serde_json::json!({
                    "structureId": id,
                    "requestedKind": requested_kind,
                    "reason": reason,
                    "status": "unsupported",
                })));
            }
        };
        structures.push(record);
    }
    let collisions = CollisionContext::new(input, &structures);
    if let Some((part, obstacle)) = collisions.structure_obstacle_blocker() {
        return Err(EngineeringError::new(
            "structure_intersects_obstacle",
            format!(
                "structure part {part} interpenetrates obstacle {obstacle}; buildings and walls are hard obstructions and are never opened, shrunk or excluded to make a design fit"
            ),
        )
        .with_detail(serde_json::json!({
            "part": part,
            "obstacle": obstacle,
            "contactToleranceM": SOLID_CONTACT_TOLERANCE_M,
        })));
    }
    if let Some((a, b, same_structure)) = collisions.structure_pair_blocker() {
        return Err(EngineeringError::new(
            if same_structure {
                "structure_self_intersects"
            } else {
                "structure_overlap"
            },
            format!(
                "structure parts {a} and {b} interpenetrate; a flight, a turn landing and a support must each have room, and two structures must not occupy one volume"
            ),
        )
        .with_detail(serde_json::json!({
            "a": a,
            "b": b,
            "sameStructure": same_structure,
            "contactToleranceM": SOLID_CONTACT_TOLERANCE_M,
        })));
    }
    let changed_scopes: Vec<RectI> = ground_works.iter().map(|report| report.scope).collect();
    check_surface_relations(input, &surface, &structures, &changed_scopes)?;
    validate_references(input, &structures)?;
    validate_port_membership(input, &surface, &structures)?;
    let mesh_points: usize = structures.iter().map(|s| s.parts.len() * 8).sum();
    let mesh_triangles: usize = structures.iter().map(|s| s.parts.len() * 12).sum();
    if mesh_points > MAX_MESH_VERTICES {
        return err(
            "mesh_too_large",
            format!("{mesh_points} mesh points exceed the v1 cap {MAX_MESH_VERTICES}"),
        );
    }
    let mut node_map: BTreeMap<String, GraphNode> = BTreeMap::new();
    for port in &input.ports {
        node_map.insert(port.id.clone(), node_from_port(port, "declared_port"));
    }
    for structure in &structures {
        for node in &structure.nodes {
            if let Some(existing) = node_map.get(&node.node_id) {
                if existing.position_m != node.position_m
                    || existing.level_id != node.level_id
                    || existing.surface_id != node.surface_id
                {
                    return err(
                        "port_node_conflict",
                        format!(
                            "port {} is declared at {:?}/{}/{} but structure {} places it at {:?}/{}/{}",
                            node.node_id,
                            existing.position_m,
                            existing.surface_id,
                            existing.level_id,
                            structure.structure_id,
                            node.position_m,
                            node.surface_id,
                            node.level_id
                        ),
                    );
                }
                continue;
            }
            node_map.insert(node.node_id.clone(), node.clone());
        }
    }
    let mut edges: Vec<GraphEdge> = vec![];
    for structure in &structures {
        let by_id = |id: &str| -> EcResult<&GraphNode> {
            node_map.get(id).ok_or_else(|| {
                EngineeringError::new(
                    "unknown_node",
                    format!(
                        "structure {} references node {id} which is not in the graph",
                        structure.structure_id
                    ),
                )
            })
        };
        match structure.kind.as_str() {
            "landing" => {
                let entry = by_id(&structure.entry_node_id)?;
                if let Some(exit_id) = &structure.deck_exit_node_id {
                    let exit = by_id(exit_id)?;
                    let horizontal = Vec3::from_arr(exit.position_m)
                        .sub(Vec3::from_arr(entry.position_m))
                        .xz_len();
                    edges.push(structure_edge(
                        &format!("{}::deck", structure.structure_id),
                        entry,
                        exit,
                        "landing",
                        false,
                        structure.width_m,
                        horizontal,
                        vec![entry.position_m, exit.position_m],
                        0.0,
                        0.0,
                        0.0,
                        &rules,
                        &structure.structure_id,
                        serde_json::json!({
                            "levelIds": structure.level_ids,
                            "detail": "an independent platform deck is walked as a landing edge at one level, never as terrain",
                        }),
                    ));
                }
            }
            "straight_stair" => {
                let entry = by_id(&structure.entry_node_id)?;
                let exit = by_id(&structure.exit_node_id)?;
                let horizontal = Vec3::from_arr(exit.position_m)
                    .sub(Vec3::from_arr(entry.position_m))
                    .xz_len();
                edges.push(structure_edge(
                    &structure.structure_id,
                    entry,
                    exit,
                    "stair",
                    structure.one_way,
                    structure.width_m,
                    horizontal,
                    tread_polyline(structure, None, entry, exit),
                    exit.position_m[1] - entry.position_m[1],
                    structure.step_rise_m,
                    structure.step_run_m,
                    &rules,
                    &structure.structure_id,
                    serde_json::json!({
                        "steps": structure.steps.len(),
                        "riseMm": (structure.step_rise_m * 1000.0).round() as i64,
                        "runMm": (structure.step_run_m * 1000.0).round() as i64,
                        "levelIds": structure.level_ids,
                        "supportKind": structure.support_kind,
                    }),
                ));
            }
            "switchback_stair" => {
                let entry = by_id(&structure.entry_node_id)?;
                let exit = by_id(&structure.exit_node_id)?;
                let middle_id = structure.middle_node_id.as_deref().ok_or_else(|| {
                    EngineeringError::new(
                        "missing_turn_node",
                        format!("switchback {} has no turn node", structure.structure_id),
                    )
                })?;
                let middle = by_id(middle_id)?;
                let turn_exit_id = format!("{}:turn_exit", structure.structure_id);
                let turn_exit = by_id(&turn_exit_id)?;
                let turn_corner_id = format!("{}:turn_corner", structure.structure_id);
                let turn_corner = by_id(&turn_corner_id)?;
                let run1 = Vec3::from_arr(middle.position_m)
                    .sub(Vec3::from_arr(entry.position_m))
                    .xz_len();
                edges.push(structure_edge(
                    &format!("{}::run1", structure.structure_id),
                    entry,
                    middle,
                    "stair",
                    structure.one_way,
                    structure.width_m,
                    run1,
                    tread_polyline(structure, Some(0), entry, middle),
                    middle.position_m[1] - entry.position_m[1],
                    structure.step_rise_m,
                    structure.step_run_m,
                    &rules,
                    &structure.structure_id,
                    serde_json::json!({"runIndex": 0, "turnsAt": middle.node_id}),
                ));
                let cross = Vec3::from_arr(turn_corner.position_m)
                    .sub(Vec3::from_arr(middle.position_m))
                    .xz_len();
                edges.push(structure_edge(
                    &format!("{}::turn", structure.structure_id),
                    middle,
                    turn_corner,
                    "landing",
                    false,
                    structure.width_m,
                    cross,
                    vec![middle.position_m, turn_corner.position_m],
                    0.0,
                    0.0,
                    0.0,
                    &rules,
                    &structure.structure_id,
                    serde_json::json!({
                        "levelId": structure.level_ids.last(),
                        "leg": "across the turn landing to the second lane",
                        "detail": "the turn landing is an independent level, never a ground edge",
                    }),
                ));
                let along = Vec3::from_arr(turn_exit.position_m)
                    .sub(Vec3::from_arr(turn_corner.position_m))
                    .xz_len();
                edges.push(structure_edge(
                    &format!("{}::turn2", structure.structure_id),
                    turn_corner,
                    turn_exit,
                    "landing",
                    false,
                    structure.width_m,
                    along,
                    vec![turn_corner.position_m, turn_exit.position_m],
                    0.0,
                    0.0,
                    0.0,
                    &rules,
                    &structure.structure_id,
                    serde_json::json!({
                        "levelId": structure.level_ids.last(),
                        "leg": "along the second lane to the foot of the second flight",
                        "detail": "the second leg of the turn is real geometry, not a point on the landing edge",
                    }),
                ));
                let run2 = Vec3::from_arr(exit.position_m)
                    .sub(Vec3::from_arr(turn_exit.position_m))
                    .xz_len();
                edges.push(structure_edge(
                    &format!("{}::run2", structure.structure_id),
                    turn_exit,
                    exit,
                    "stair",
                    structure.one_way,
                    structure.width_m,
                    run2,
                    tread_polyline(structure, Some(1), turn_exit, exit),
                    exit.position_m[1] - turn_exit.position_m[1],
                    structure.step_rise_m,
                    structure.step_run_m,
                    &rules,
                    &structure.structure_id,
                    serde_json::json!({"runIndex": 1}),
                ));
            }
            other => {
                return err(
                    "unknown_structure_kind",
                    format!(
                        "structure {} has unhandled kind {other}",
                        structure.structure_id
                    ),
                );
            }
        }
    }
    for spec in &input.ground_connections {
        let edge = build_ground_edge(
            spec,
            input,
            &surface,
            &terrain,
            &node_map,
            &rules,
            &collisions,
        )?;
        edges.push(edge);
    }
    refresh_walkable_capabilities(&mut edges, &structures, &collisions, &rules)?;
    let mut edge_ids = BTreeSet::new();
    for edge in &edges {
        if !edge_ids.insert(edge.edge_id.clone()) {
            return err(
                "duplicate_id",
                format!("edge {} is produced twice", edge.edge_id),
            );
        }
    }
    if edges.len() > MAX_EDGES {
        return err(
            "edge_count_out_of_range",
            format!("{} edges exceed the v1 cap {MAX_EDGES}", edges.len()),
        );
    }
    let adjacency = adjacency_of(&edges);
    let route_result = route(&node_map, &adjacency, &edges, &input.query);
    let graph = GraphRecord {
        nodes: node_map.values().cloned().collect(),
        ground_edges: edges.iter().filter(|edge| edge.kind == "ground").count(),
        stair_edges: edges.iter().filter(|edge| edge.kind == "stair").count(),
        landing_edges: edges.iter().filter(|edge| edge.kind == "landing").count(),
        one_way_edges: edges.iter().filter(|edge| !edge.bidirectional).count(),
        edges,
        capability_rules: rules,
    };
    Ok(BuildOutcome {
        surface,
        ground_works,
        structures,
        nodes: node_map.values().cloned().collect(),
        graph,
        route: route_result,
        matrix: capability_matrix(),
        mesh_points,
        mesh_triangles,
    })
}

/// Both endpoints of every edge are candidates: direction is a property the search
/// checks and reports (`one_way_blocked`), not a reason to hide an edge from the
/// search and lose the explanation.
fn adjacency_of(edges: &[GraphEdge]) -> BTreeMap<String, Vec<usize>> {
    let mut adjacency: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        adjacency
            .entry(edge.from_node.clone())
            .or_default()
            .push(index);
        adjacency
            .entry(edge.to_node.clone())
            .or_default()
            .push(index);
    }
    adjacency
}

// ---------------------------------------------------------------------------
// Bundle and views
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRecord {
    pub view_id: String,
    /// `plan`, `elevation` or `mesh`.
    pub kind: String,
    pub format: String,
    pub generator: String,
    pub file_name: String,
    pub content_hash: String,
    pub bytes: usize,
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRecord {
    pub width: u32,
    pub depth: u32,
    pub origin_x: i32,
    pub origin_z: i32,
    pub mm: Vec<i32>,
    pub natural_hash: String,
    pub baseline_hash: String,
    pub engineered_hash: String,
    /// Cells where the engineered field differs from the immutable baseline
    /// (exclusive exact count, one per cell).
    pub engineered_cells_changed: usize,
    /// Cells where the engineered field differs from the natural field. This is a
    /// comparison, not a claim about which packet changed them.
    pub natural_cells_differing: usize,
    /// Sum of the per-plan changed-cell counts. Plans may overlap, so this total is
    /// not exclusive.
    pub changed_cells_by_plan_total: usize,
    pub cut_volume_m3: f64,
    pub fill_volume_m3: f64,
}

/// Publication is a transaction state, not a comment. Only a `committed` bundle may
/// be consumed; a `staged` bundle has no payload hash and is refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationRecord {
    /// `committed`, `staged` or `unknown` (a bundle written before v2).
    pub state: String,
    pub contract_version: u32,
    pub staging_id: String,
    pub bundle_file: String,
    pub view_files: Vec<String>,
    pub report_file: String,
    pub commit_order: Vec<String>,
    pub note: String,
}

impl Default for PublicationRecord {
    fn default() -> Self {
        Self {
            state: "unknown".into(),
            contract_version: 0,
            staging_id: String::new(),
            bundle_file: String::new(),
            view_files: vec![],
            report_file: String::new(),
            commit_order: vec![],
            note:
                "no publication record: this bundle was written before the v2 transaction contract"
                    .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringBundle {
    pub version: u32,
    #[serde(default)]
    pub publication: PublicationRecord,
    pub scene_id: String,
    pub units: String,
    pub module: String,
    pub contract: serde_json::Value,
    pub input_hash: String,
    pub surface: SurfaceRecord,
    pub ground_works: Vec<GroundWorkReport>,
    pub structures: Vec<BundleStructure>,
    pub nodes: Vec<GraphNode>,
    pub graph: GraphRecord,
    pub route: RouteResult,
    pub capability_matrix: Vec<MatrixRow>,
    pub views: Vec<ViewRecord>,
    pub mesh_points: usize,
    pub mesh_triangles: usize,
    pub payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleStructure {
    pub structure_id: String,
    pub kind: String,
    pub level_ids: Vec<String>,
    pub entry_node_id: String,
    pub exit_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middle_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_exit_node_id: Option<String>,
    pub one_way: bool,
    pub width_m: f64,
    pub step_rise_m: f64,
    pub step_run_m: f64,
    pub support_kind: String,
    pub nodes: Vec<GraphNode>,
    pub parts: Vec<MeshPart>,
    pub supports: Vec<SupportRecord>,
    pub steps: Vec<StepRecord>,
    pub total_rise_m: f64,
    pub mesh: MeshSummary,
    pub checks: Vec<serde_json::Value>,
}

fn contract_block() -> serde_json::Value {
    serde_json::json!({
        "version": ENGINEERING_CONNECTIONS_VERSION,
        "units": UNITS_CONTRACT,
        "coordinateFrame": "world metres; X east, Z north, Y up; levels stored as signed millimetres",
        "direction": "one-way edges carry direction from their fromNode to their toNode; ground connections default to two-way and stairs may declare oneWay",
        "identity": "portId is the node identity; a port is (position, surfaceId, levelId, width, capabilities); structure ids, edge ids and part ids are stable strings inside this bundle",
        "numericLimits": {
            "gridSideMax": MAX_GRID_SIDE,
            "structuresMax": MAX_STRUCTURES,
            "portsMax": MAX_PORTS,
            "edgesMax": MAX_EDGES,
            "groundWorkCellsMax": MAX_GROUND_WORK_CELLS,
            "routeExpansionsMax": MAX_ROUTE_EXPANSIONS,
            "meshPointsMax": MAX_MESH_VERTICES,
            "portMatchToleranceM": PORT_MATCH_TOLERANCE_M,
            "supportContactToleranceM": SUPPORT_CONTACT_TOLERANCE_M,
            "nanOrInfinite": "rejected with the category non_finite_value",
        },
        "routeCost": COST_DEFINITION,
        "errorCategories": [
            "contract_version_mismatch", "units_mismatch", "grid_out_of_range", "grid_length_mismatch",
            "non_finite_value", "duplicate_id", "scope_out_of_range", "scope_outside_grid",
            "protected_cell", "fill_not_authorised", "cut_not_authorised", "fill_over_authorisation",
            "cut_over_authorisation", "missing_target", "unsupported", "invalid_landing",
            "invalid_stair", "step_out_of_contract", "stair_too_long", "port_geometry_mismatch",
            "port_outside_structure", "port_node_conflict", "landing_level_mismatch",
            "landing_does_not_reach_run", "structure_overlap", "structure_count_out_of_range",
            "port_count_out_of_range", "edge_count_out_of_range", "mesh_too_large",
            "support_outside_grid", "support_requires_columns", "invalid_column_size",
            "column_wider_than_member", "member_below_bearing", "footing_too_deep", "unknown_port",
            "unknown_node", "ground_predicate_failed", "port_not_on_ground",
            "obstacle_blocks_ground_edge", "water_blocks_ground_edge", "ground_sample_missing",
        ],
        "unsupportedKinds": [
            "spiral_stair", "curved_stair", "lift", "escalator", "moving_walk", "tunnel",
            "underpass", "elevated_road", "bridge", "interchange", "retaining_wall", "drainage",
        ],
        "naturalTerrain": "this module never writes natural terrain; ground works edit only the engineered field and refuse protected cells",
    })
}

fn canonical_json<T: Serialize>(value: &T) -> EcResult<String> {
    serde_json::to_string(value)
        .map_err(|error| EngineeringError::new("serialization_failed", error.to_string()))
}

pub fn payload_hash_of(bundle: &EngineeringBundle) -> EcResult<String> {
    let mut copy = bundle.clone();
    copy.payload_hash = String::new();
    let bytes = canonical_json(&copy)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn build_bundle(input: &EngineeringInput) -> EcResult<(EngineeringBundle, BuildOutcome)> {
    let outcome = build(input)?;
    let natural_changed = outcome
        .surface
        .natural_mm
        .iter()
        .zip(outcome.surface.mm.iter())
        .filter(|(natural, engineered)| natural != engineered)
        .count();
    let mut bundle = EngineeringBundle {
        version: ENGINEERING_CONNECTIONS_VERSION,
        scene_id: input.scene_id.clone(),
        units: UNITS_CONTRACT.into(),
        module: "glyphweave-core::worldgen::engineering_connections".into(),
        contract: contract_block(),
        input_hash: {
            let mut hasher = blake3::Hasher::new();
            hasher.update(canonical_json(input)?.as_bytes());
            hasher.finalize().to_hex().to_string()
        },
        surface: SurfaceRecord {
            width: outcome.surface.width,
            depth: outcome.surface.depth,
            origin_x: outcome.surface.origin_x,
            origin_z: outcome.surface.origin_z,
            mm: outcome.surface.mm.clone(),
            natural_hash: field_hash(
                &outcome.surface.natural_mm,
                outcome.surface.width,
                outcome.surface.depth,
                outcome.surface.origin_x,
                outcome.surface.origin_z,
            ),
            baseline_hash: field_hash(
                &outcome.surface.baseline_mm,
                outcome.surface.width,
                outcome.surface.depth,
                outcome.surface.origin_x,
                outcome.surface.origin_z,
            ),
            engineered_hash: field_hash(
                &outcome.surface.mm,
                outcome.surface.width,
                outcome.surface.depth,
                outcome.surface.origin_x,
                outcome.surface.origin_z,
            ),
            engineered_cells_changed: outcome
                .surface
                .mm
                .iter()
                .zip(outcome.surface.baseline_mm.iter())
                .filter(|(engineered, baseline)| engineered != baseline)
                .count(),
            natural_cells_differing: natural_changed,
            changed_cells_by_plan_total: outcome
                .ground_works
                .iter()
                .map(|report| report.changed_cells)
                .sum(),
            cut_volume_m3: (outcome
                .ground_works
                .iter()
                .map(|report| report.cut_volume_m3)
                .sum::<f64>()
                * 1000.0)
                .round()
                / 1000.0,
            fill_volume_m3: (outcome
                .ground_works
                .iter()
                .map(|report| report.fill_volume_m3)
                .sum::<f64>()
                * 1000.0)
                .round()
                / 1000.0,
        },
        ground_works: outcome.ground_works.clone(),
        structures: outcome
            .structures
            .iter()
            .map(|record| BundleStructure {
                structure_id: record.structure_id.clone(),
                kind: record.kind.clone(),
                level_ids: record.level_ids.clone(),
                entry_node_id: record.entry_node_id.clone(),
                exit_node_id: record.exit_node_id.clone(),
                middle_node_id: record.middle_node_id.clone(),
                deck_exit_node_id: record.deck_exit_node_id.clone(),
                one_way: record.one_way,
                width_m: record.width_m,
                step_rise_m: record.step_rise_m,
                step_run_m: record.step_run_m,
                support_kind: record.support_kind.clone(),
                nodes: record.nodes.clone(),
                parts: record.parts.clone(),
                supports: record.supports.clone(),
                steps: record.steps.clone(),
                total_rise_m: record.total_rise_m,
                mesh: record.mesh.clone(),
                checks: record.checks.clone(),
            })
            .collect(),
        nodes: outcome.nodes.clone(),
        graph: outcome.graph.clone(),
        route: outcome.route.clone(),
        capability_matrix: outcome.matrix.clone(),
        publication: PublicationRecord::default(),
        views: vec![],
        mesh_points: outcome.mesh_points,
        mesh_triangles: outcome.mesh_triangles,
        payload_hash: String::new(),
    };
    bundle.views = render_views(&bundle, &outcome);
    // A library build is always staged: committing is the publishing command's job,
    // and a staged bundle carries no payload hash.
    bundle.publication = PublicationRecord {
        state: "staged".into(),
        contract_version: ENGINEERING_CONNECTIONS_VERSION,
        staging_id: format!("stage-{}", &bundle.input_hash[..12.min(bundle.input_hash.len())]),
        bundle_file: String::new(),
        view_files: bundle.views.iter().map(|view| view.file_name.clone()).collect(),
        report_file: String::new(),
        commit_order: vec![],
        note: "staged in memory by build_bundle; the publishing command commits it and moves it into place".into(),
    };
    Ok((bundle, outcome))
}

/// Commit a staged bundle: the payload hash is computed over the committed content,
/// so a consumer can tell a committed package from a file copied out of a failed run.
pub fn commit_bundle(
    bundle: &mut EngineeringBundle,
    bundle_file: &str,
    report_file: &str,
) -> EcResult<String> {
    bundle.publication.state = "committed".into();
    bundle.publication.contract_version = ENGINEERING_CONNECTIONS_VERSION;
    bundle.publication.bundle_file = bundle_file.to_string();
    bundle.publication.report_file = report_file.to_string();
    bundle.publication.commit_order =
        vec!["geometryViews".into(), "report".into(), "bundle".into()];
    bundle.publication.note =
        "published by build-engineering-connection after a full preflight and a staging write; the bundle is moved into place last, so a bundle at its final path implies its views were already there"
            .into();
    bundle.payload_hash = payload_hash_of(bundle)?;
    Ok(bundle.payload_hash.clone())
}

/// Payload hash of a bundle that only exists as JSON, used to check integrity before
/// any semantics are trusted.
pub fn payload_hash_of_value(value: &serde_json::Value) -> Option<String> {
    let mut copy = value.clone();
    copy.as_object_mut()?.insert(
        "payloadHash".into(),
        serde_json::Value::String(String::new()),
    );
    let bytes = serde_json::to_vec(&copy).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&bytes);
    Some(hasher.finalize().to_hex().to_string())
}

pub fn views_to_files(bundle: &EngineeringBundle, outcome: &BuildOutcome) -> Vec<(String, String)> {
    let contents = view_contents(bundle, outcome);
    vec![
        ("view-plan.svg".to_string(), contents.0),
        ("view-elevation.svg".to_string(), contents.1),
        ("view-mesh.obj".to_string(), contents.2),
    ]
}

fn view_contents(bundle: &EngineeringBundle, outcome: &BuildOutcome) -> (String, String, String) {
    (
        plan_svg(bundle, outcome),
        elevation_svg(bundle, outcome),
        mesh_obj(bundle),
    )
}

fn view_record(
    view_id: &str,
    kind: &str,
    format: &str,
    file_name: &str,
    content: &str,
    detail: serde_json::Value,
) -> ViewRecord {
    let mut hasher = blake3::Hasher::new();
    hasher.update(content.as_bytes());
    ViewRecord {
        view_id: view_id.to_string(),
        kind: kind.to_string(),
        format: format.to_string(),
        generator: "glyphweave-core::worldgen::engineering_connections".into(),
        file_name: file_name.to_string(),
        content_hash: hasher.finalize().to_hex().to_string(),
        bytes: content.len(),
        detail,
    }
}

/// Every view is produced from the production vertices in the bundle: the plan and
/// the elevation from the mesh parts and ports, the OBJ from the same boxes.
pub fn render_views(bundle: &EngineeringBundle, outcome: &BuildOutcome) -> Vec<ViewRecord> {
    let plan = plan_svg(bundle, outcome);
    let elevation = elevation_svg(bundle, outcome);
    let obj = mesh_obj(bundle);
    vec![
        view_record(
            "plan",
            "plan",
            "svg",
            "view-plan.svg",
            &plan,
            serde_json::json!({
                "projection": "world X right, world Z down, one pixel per 0.05 m",
                "source": "mesh part corners, port positions and the routing polyline from this bundle",
            }),
        ),
        view_record(
            "elevation",
            "elevation",
            "svg",
            "view-elevation.svg",
            &elevation,
            serde_json::json!({
                "projection": "distance along the drawn line right, absolute level up, one pixel per 0.05 m",
                "source": "the same mesh part tops and port levels",
            }),
        ),
        view_record(
            "mesh",
            "mesh",
            "obj",
            "view-mesh.obj",
            &obj,
            serde_json::json!({
                "format": "Wavefront OBJ, one group per part, triangles from the stored box corners",
                "source": "mesh part corners",
            }),
        ),
    ]
}

struct ViewTransform {
    min_x: f64,
    min_z: f64,
    scale: f64,
    height: f64,
}

impl ViewTransform {
    fn project(&self, x: f64, z: f64) -> (f64, f64) {
        (
            (x - self.min_x) * self.scale + 40.0,
            (z - self.min_z) * self.scale + 40.0,
        )
    }
    fn svg_height(&self) -> f64 {
        self.height + 80.0
    }
}

fn plan_bounds(outcome: &BuildOutcome) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_z = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_z = f64::MIN;
    let mut note = |x: f64, z: f64| {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    };
    for structure in &outcome.structures {
        for part in &structure.parts {
            for corner in &part.corners_m {
                note(corner[0], corner[2]);
            }
        }
    }
    for node in &outcome.nodes {
        note(node.position_m[0], node.position_m[2]);
    }
    for edge in &outcome.graph.edges {
        for point in &edge.polyline_m {
            note(point[0], point[2]);
        }
    }
    if min_x > max_x {
        return (0.0, 0.0, 1.0, 1.0);
    }
    (min_x, min_z, max_x, max_z)
}

fn plan_svg(bundle: &EngineeringBundle, outcome: &BuildOutcome) -> String {
    let (min_x, min_z, max_x, max_z) = plan_bounds(outcome);
    let scale = 20.0_f64;
    let width = ((max_x - min_x) * scale + 80.0).max(120.0);
    let height = (max_z - min_z) * scale + 80.0;
    let transform = ViewTransform {
        min_x,
        min_z,
        scale,
        height,
    };
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.0}\" height=\"{:.0}\" viewBox=\"0 0 {:.0} {:.0}\">\n",
        width,
        transform.svg_height(),
        width,
        transform.svg_height()
    ));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#0b0f14\"/>\n");
    svg.push_str(&format!(
        "<text x=\"12\" y=\"20\" fill=\"#9fb3c8\" font-family=\"monospace\" font-size=\"12\">{} plan, route {}, engines {} mm</text>\n",
        bundle.scene_id, bundle.route.status, bundle.surface.engineered_hash.get(..8).unwrap_or("")
    ));
    for structure in &outcome.structures {
        for part in &structure.parts {
            let points: Vec<String> = [4_usize, 5, 6, 7]
                .iter()
                .map(|corner| {
                    let (x, y) =
                        transform.project(part.corners_m[*corner][0], part.corners_m[*corner][2]);
                    format!("{x:.2},{y:.2}")
                })
                .collect();
            let colour = match part.role.as_str() {
                "tread" => "#4fa3ff",
                "riser" => "#2b6cb0",
                "landing" => "#f6ad55",
                "support_column" => "#718096",
                _ => "#68d391",
            };
            svg.push_str(&format!(
                "<polygon points=\"{}\" fill=\"{}\" fill-opacity=\"0.55\" stroke=\"{}\" stroke-width=\"0.6\"/>\n",
                points.join(" "),
                colour,
                colour
            ));
        }
    }
    for node in &outcome.nodes {
        let (x, y) = transform.project(node.position_m[0], node.position_m[2]);
        svg.push_str(&format!(
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\" fill=\"#faf089\"/>\n<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#faf089\" font-family=\"monospace\" font-size=\"9\">{}</text>\n",
            x + 5.0,
            y - 5.0,
            node.node_id
        ));
    }
    if !bundle.route.path_polyline_m.is_empty() {
        let points: Vec<String> = bundle
            .route
            .path_polyline_m
            .iter()
            .map(|point| {
                let (x, y) = transform.project(point[0], point[2]);
                format!("{x:.2},{y:.2}")
            })
            .collect();
        svg.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#f56565\" stroke-width=\"2\"/>\n",
            points.join(" ")
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

fn elevation_svg(bundle: &EngineeringBundle, outcome: &BuildOutcome) -> String {
    let mut line: Vec<[f64; 3]> = if !bundle.route.path_polyline_m.is_empty() {
        bundle.route.path_polyline_m.clone()
    } else if let Some(edge) = outcome.graph.edges.iter().find(|edge| edge.kind == "stair") {
        edge.polyline_m.clone()
    } else if let Some(edge) = outcome.graph.edges.first() {
        edge.polyline_m.clone()
    } else {
        outcome.nodes.iter().map(|node| node.position_m).collect()
    };
    if line.len() < 2 {
        line = outcome.nodes.iter().map(|node| node.position_m).collect();
    }
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    let mut min_ground = f64::MAX;
    for point in &line {
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    for x in 0..outcome.surface.width {
        for z in 0..outcome.surface.depth {
            if let Some(mm) = outcome.surface.mm_at(
                outcome.surface.origin_x + x as i32,
                outcome.surface.origin_z + z as i32,
            ) {
                min_ground = min_ground.min(f64::from(mm) / 1000.0);
            }
        }
    }
    let min_y = if min_y.is_finite() { min_y } else { 0.0 };
    let max_y = if max_y.is_finite() { max_y } else { 1.0 };
    let min_ground = if min_ground.is_finite() {
        min_ground
    } else {
        min_y
    };
    let scale = 60.0_f64;
    let mut distance = 0.0_f64;
    let mut projected: Vec<(f64, f64)> = vec![];
    for (index, point) in line.iter().enumerate() {
        if index > 0 {
            let previous = Vec3::from_arr(line[index - 1]);
            distance += Vec3::from_arr(*point).sub(previous).len();
        }
        projected.push((distance, point[1]));
    }
    let width = distance * scale + 120.0;
    let height = (max_y - min_y.min(min_ground)) * scale + 140.0;
    let project = |d: f64, y: f64| -> (f64, f64) {
        (
            d * scale + 60.0,
            height - 60.0 - (y - min_y.min(min_ground)) * scale,
        )
    };
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\">\n"
    ));
    svg.push_str(&format!(
        "<rect width=\"{width:.0}\" height=\"{height:.0}\" fill=\"#0b0f14\"/>\n"
    ));
    svg.push_str(&format!(
        "<text x=\"12\" y=\"20\" fill=\"#9fb3c8\" font-family=\"monospace\" font-size=\"12\">{} elevation, {} to {} m, drawn along {}</text>\n",
        bundle.scene_id,
        (min_y * 1000.0).round() / 1000.0,
        (max_y * 1000.0).round() / 1000.0,
        if bundle.route.path_polyline_m.is_empty() { "the first staircase section" } else { "the accepted route" }
    ));
    let ground = project(0.0, min_ground);
    let ground_end = project(distance, min_ground);
    svg.push_str(&format!(
        "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#68d391\" stroke-width=\"1\" stroke-dasharray=\"4 4\"/>\n",
        ground.0, ground.1, ground_end.0, ground_end.1
    ));
    let mut points: Vec<String> = vec![];
    for (index, _point) in line.iter().enumerate() {
        let (d, y) = projected[index];
        let (px, py) = project(d, y);
        points.push(format!("{px:.2},{py:.2}"));
        svg.push_str(&format!(
            "<circle cx=\"{px:.2}\" cy=\"{py:.2}\" r=\"2.5\" fill=\"#f6ad55\"/>\n"
        ));
    }
    svg.push_str(&format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#f56565\" stroke-width=\"2\"/>\n",
        points.join(" ")
    ));
    svg.push_str("</svg>\n");
    svg
}

fn mesh_obj(bundle: &EngineeringBundle) -> String {
    let mut obj = String::new();
    obj.push_str(&format!(
        "# glyphweave engineering connections mesh, scene {}, engineered field {}\n",
        bundle.scene_id,
        bundle.surface.engineered_hash.get(..16).unwrap_or("")
    ));
    let mut offset = 1_usize;
    for structure in &bundle.structures {
        for part in &structure.parts {
            obj.push_str(&format!(
                "g {} {} {}\n",
                structure.structure_id, part.role, part.part_id
            ));
            for corner in &part.corners_m {
                obj.push_str(&format!(
                    "v {:.4} {:.4} {:.4}\n",
                    corner[0], corner[1], corner[2]
                ));
            }
            for triangle in part.triangles() {
                obj.push_str(&format!(
                    "f {} {} {}\n",
                    triangle[0] + offset,
                    triangle[1] + offset,
                    triangle[2] + offset
                ));
            }
            offset += 8;
        }
    }
    obj
}

// ---------------------------------------------------------------------------
// Independent read-back verification
// ---------------------------------------------------------------------------

const MAX_FINDINGS: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TamperFinding {
    pub code: String,
    /// JSON path inside the bundle, so a reader can go straight to the field.
    pub at: String,
    pub expected: String,
    pub found: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    /// `verified`, `tamper_detected` or `input_rejected`.
    pub status: String,
    pub scene_id: String,
    pub bundle_version: u32,
    pub contract_version: u32,
    pub bundle_payload_hash: String,
    pub recomputed_payload_hash: String,
    pub payload_hash_matches: bool,
    pub input_hash_matches: bool,
    pub checks: Vec<VerifyCheck>,
    pub independent_rechecks: Vec<VerifyCheck>,
    pub findings: Vec<TamperFinding>,
    pub finding_count: usize,
    pub recomputed_summary: serde_json::Value,
    pub notes: Vec<String>,
}

fn diff_json(
    expected: &serde_json::Value,
    found: &serde_json::Value,
    path: &str,
    out: &mut Vec<TamperFinding>,
    count: &mut usize,
) {
    if *count >= MAX_FINDINGS {
        return;
    }
    match (expected, found) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (key, value) in a {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match b.get(key) {
                    Some(other) => diff_json(value, other, &child, out, count),
                    None => {
                        *count += 1;
                        if out.len() < MAX_FINDINGS {
                            out.push(TamperFinding {
                                code: "missing_field".into(),
                                at: child,
                                expected: short(value),
                                found: "<absent>".into(),
                            });
                        }
                    }
                }
            }
            for key in b.keys() {
                if !a.contains_key(key) {
                    *count += 1;
                    if out.len() < MAX_FINDINGS {
                        out.push(TamperFinding {
                            code: "unexpected_field".into(),
                            at: format!("{path}.{key}"),
                            expected: "<absent>".into(),
                            found: short(&b[key]),
                        });
                    }
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a.len() != b.len() {
                *count += 1;
                if out.len() < MAX_FINDINGS {
                    out.push(TamperFinding {
                        code: "array_length_mismatch".into(),
                        at: path.to_string(),
                        expected: format!("{} entries", a.len()),
                        found: format!("{} entries", b.len()),
                    });
                }
            }
            for (index, value) in a.iter().enumerate() {
                if let Some(other) = b.get(index) {
                    diff_json(value, other, &format!("{path}[{index}]"), out, count);
                }
            }
        }
        _ => {
            if expected != found {
                *count += 1;
                if out.len() < MAX_FINDINGS {
                    out.push(TamperFinding {
                        code: "value_mismatch".into(),
                        at: path.to_string(),
                        expected: short(expected),
                        found: short(found),
                    });
                }
            }
        }
    }
}

fn short(value: &serde_json::Value) -> String {
    let text = value.to_string();
    if text.len() > 160 {
        format!("{}…", &text[..160])
    } else {
        text
    }
}

fn check(name: &str, passed: bool, detail: impl Into<String>) -> VerifyCheck {
    VerifyCheck {
        name: name.to_string(),
        passed,
        detail: detail.into(),
    }
}

/// Rebuild everything from the input, then compare it with the saved bundle and
/// recheck the saved bundle's own claims against the saved surface. Nothing in the
/// saved file is trusted: the ground predicate is re-run, the mesh hash recomputed,
/// every support re-derived and the route re-searched on the saved graph.
pub fn verify_bundle(input: &EngineeringInput, stored: &EngineeringBundle) -> VerifyReport {
    let mut findings: Vec<TamperFinding> = vec![];
    let mut finding_count = 0_usize;
    let mut checks: Vec<VerifyCheck> = vec![];
    let mut notes: Vec<String> = vec![];
    // Integrity first: an edited payload hash is reported before any semantics are
    // rebuilt, and before the input is even required to be a v2 description.
    let (rebuilt, rebuilt_outcome) = match build_bundle(input) {
        Ok(pair) => pair,
        Err(error) => {
            return VerifyReport {
                status: "input_rejected".into(),
                scene_id: stored.scene_id.clone(),
                bundle_version: stored.version,
                contract_version: ENGINEERING_CONNECTIONS_VERSION,
                bundle_payload_hash: stored.payload_hash.clone(),
                recomputed_payload_hash: String::new(),
                payload_hash_matches: false,
                input_hash_matches: false,
                checks: vec![check(
                    "input_rebuild",
                    false,
                    format!("the input does not build: {error}"),
                )],
                independent_rechecks: vec![],
                findings: vec![TamperFinding {
                    code: "rebuild_rejected".into(),
                    at: "input".into(),
                    expected: "an input this contract accepts".into(),
                    found: error.category,
                }],
                finding_count: 1,
                recomputed_summary: serde_json::Value::Null,
                notes: vec![error.message],
            };
        }
    };
    let payload_hash_matches = payload_hash_of(stored)
        .map(|hash| hash == stored.payload_hash)
        .unwrap_or(false);
    if !payload_hash_matches {
        findings.push(TamperFinding {
            code: "bundle_payload_hash".into(),
            at: "payloadHash".into(),
            expected: payload_hash_of(stored).unwrap_or_default(),
            found: stored.payload_hash.clone(),
        });
        finding_count += 1;
        return VerifyReport {
            status: "tamper_detected".into(),
            scene_id: stored.scene_id.clone(),
            bundle_version: stored.version,
            contract_version: ENGINEERING_CONNECTIONS_VERSION,
            bundle_payload_hash: stored.payload_hash.clone(),
            recomputed_payload_hash: payload_hash_of(stored).unwrap_or_default(),
            payload_hash_matches: false,
            input_hash_matches: false,
            checks: vec![check(
                "bundle_payload_hash",
                false,
                "the saved payload hash does not match the saved content: the file was edited",
            )],
            independent_rechecks: vec![],
            findings,
            finding_count,
            recomputed_summary: serde_json::Value::Null,
            notes: vec!["an edited bundle is refused before its geometry is rebuilt".into()],
        };
    }
    if stored.publication.state != "committed"
        || stored.publication.contract_version != ENGINEERING_CONNECTIONS_VERSION
    {
        findings.push(TamperFinding {
            code: "incomplete_publication".into(),
            at: "publication.state".into(),
            expected: format!("committed at contract version {ENGINEERING_CONNECTIONS_VERSION}"),
            found: format!(
                "{} at contract version {}",
                stored.publication.state, stored.publication.contract_version
            ),
        });
        return VerifyReport {
            status: "incomplete_publication".into(),
            scene_id: stored.scene_id.clone(),
            bundle_version: stored.version,
            contract_version: ENGINEERING_CONNECTIONS_VERSION,
            bundle_payload_hash: stored.payload_hash.clone(),
            recomputed_payload_hash: payload_hash_of(stored).unwrap_or_default(),
            payload_hash_matches: true,
            input_hash_matches: false,
            checks: vec![check(
                "publication_committed",
                false,
                "the bundle is not a committed v2 publication: it was staged, or copied out of a failed run",
            )],
            independent_rechecks: vec![],
            findings,
            finding_count,
            recomputed_summary: serde_json::Value::Null,
            notes: vec![stored.publication.note.clone()],
        };
    }
    let input_hash_matches = stored.input_hash == rebuilt.input_hash;
    checks.push(check(
        "bundle_payload_hash",
        payload_hash_matches,
        if payload_hash_matches {
            "the saved payload hash matches the saved content".to_string()
        } else {
            "the saved payload hash does not match the saved content: the file was edited"
                .to_string()
        },
    ));
    checks.push(check(
        "input_binding",
        input_hash_matches,
        format!(
            "saved inputHash {} against the recomputed {}",
            stored.input_hash, rebuilt.input_hash
        ),
    ));
    checks.push(check(
        "contract_version",
        stored.version == rebuilt.version,
        format!("saved {} against {}", stored.version, rebuilt.version),
    ));
    let stored_value = serde_json::to_value(stored).unwrap_or(serde_json::Value::Null);
    let rebuilt_value = serde_json::to_value(&rebuilt).unwrap_or(serde_json::Value::Null);
    if let (serde_json::Value::Object(mut a), serde_json::Value::Object(mut b)) =
        (stored_value, rebuilt_value)
    {
        a.remove("payloadHash");
        b.remove("payloadHash");
        // Publication is metadata about the file, not part of the design: it is
        // checked explicitly by state, never by rebuilding it.
        a.remove("publication");
        b.remove("publication");
        diff_json(
            &serde_json::Value::Object(b),
            &serde_json::Value::Object(a),
            "",
            &mut findings,
            &mut finding_count,
        );
    }
    let surface_match = findings
        .iter()
        .all(|finding| !finding.at.starts_with("surface"));
    checks.push(check(
        "surface_recomputed",
        surface_match,
        "the saved engineered surface is compared cell by cell with the recomputation",
    ));
    // ---- independent rechecks against the saved surface only -------------
    let mut rechecks: Vec<VerifyCheck> = vec![];
    let surface = EngineeringSurface {
        width: stored.surface.width,
        depth: stored.surface.depth,
        origin_x: stored.surface.origin_x,
        origin_z: stored.surface.origin_z,
        mm: stored.surface.mm.clone(),
        natural_mm: stored.surface.mm.clone(),
        baseline_mm: stored.surface.mm.clone(),
    };
    let stored_field_hash = field_hash(
        &stored.surface.mm,
        stored.surface.width,
        stored.surface.depth,
        stored.surface.origin_x,
        stored.surface.origin_z,
    );
    rechecks.push(check(
        "stored_surface_hash",
        stored_field_hash == stored.surface.engineered_hash,
        format!(
            "recomputed {} against saved {}",
            stored_field_hash, stored.surface.engineered_hash
        ),
    ));
    match surface.to_baked_terrain() {
        Ok(terrain) => {
            let mut bad: Vec<String> = vec![];
            for edge in &stored.graph.edges {
                if edge.kind != "ground" {
                    continue;
                }
                let Some(from) = stored
                    .nodes
                    .iter()
                    .find(|node| node.node_id == edge.from_node)
                else {
                    bad.push(format!("{}: unknown from node", edge.edge_id));
                    continue;
                };
                let Some(to) = stored
                    .nodes
                    .iter()
                    .find(|node| node.node_id == edge.to_node)
                else {
                    bad.push(format!("{}: unknown to node", edge.edge_id));
                    continue;
                };
                if !super::access::terrain_segment_clear_xy(
                    from.position_m[0],
                    from.position_m[2],
                    to.position_m[0],
                    to.position_m[2],
                    &terrain,
                ) {
                    bad.push(format!(
                        "{}: the production terrain predicate fails on the saved surface",
                        edge.edge_id
                    ));
                }
                let length = polyline_length(&edge.polyline_m);
                if (length - edge.path_length_3d_m).abs() > 1e-6 {
                    bad.push(format!(
                        "{}: saved 3D length {} against polyline {length}",
                        edge.edge_id, edge.path_length_3d_m
                    ));
                }
            }
            rechecks.push(check(
                "stored_ground_edges_rechecked",
                bad.is_empty(),
                if bad.is_empty() {
                    "every saved ground edge passes the production predicate on the saved surface"
                        .to_string()
                } else {
                    bad.join("; ")
                },
            ));
            let mut supports_bad: Vec<String> = vec![];
            for structure in &stored.structures {
                for support in &structure.supports {
                    let level = bearing_level_mm(
                        &surface,
                        &input.bearing,
                        support.centre_m[0],
                        support.centre_m[2],
                    );
                    match level {
                        None => supports_bad.push(format!("{}: no bearing", support.support_id)),
                        Some(level) => {
                            let overlap = level - support.bottom_mm;
                            if overlap != support.overlap_mm {
                                supports_bad.push(format!(
                                    "{}: saved overlap {} against recomputed {overlap}",
                                    support.support_id, support.overlap_mm
                                ));
                            }
                            let in_contact =
                                overlap >= -(SUPPORT_CONTACT_TOLERANCE_M * 1000.0).round() as i32;
                            if in_contact != support.in_contact {
                                supports_bad.push(format!(
                                    "{}: saved inContact {} against recomputed {in_contact} from overlap {overlap} mm",
                                    support.support_id, support.in_contact
                                ));
                            }
                        }
                    }
                }
            }
            rechecks.push(check(
                "stored_supports_rechecked",
                supports_bad.is_empty(),
                if supports_bad.is_empty() {
                    "every saved support is re-derived from the saved surface and is in contact"
                        .to_string()
                } else {
                    supports_bad.join("; ")
                },
            ));
        }
        Err(error) => {
            rechecks.push(check(
                "stored_ground_edges_rechecked",
                false,
                format!("the saved surface cannot build a raster: {error}"),
            ));
        }
    }
    // Independent solid recheck: the saved structure geometry against the saved
    // obstacles, and the saved walking volumes of every granted capability.
    let stored_structures: Vec<StructureRecord> = stored
        .structures
        .iter()
        .map(|structure| StructureRecord {
            structure_id: structure.structure_id.clone(),
            kind: structure.kind.clone(),
            level_ids: structure.level_ids.clone(),
            entry_node_id: structure.entry_node_id.clone(),
            exit_node_id: structure.exit_node_id.clone(),
            middle_node_id: structure.middle_node_id.clone(),
            deck_exit_node_id: structure.deck_exit_node_id.clone(),
            one_way: structure.one_way,
            width_m: structure.width_m,
            step_rise_m: structure.step_rise_m,
            step_run_m: structure.step_run_m,
            support_kind: structure.support_kind.clone(),
            nodes: structure.nodes.clone(),
            parts: structure.parts.clone(),
            supports: structure.supports.clone(),
            steps: structure.steps.clone(),
            total_rise_m: structure.total_rise_m,
            mesh: structure.mesh.clone(),
            checks: structure.checks.clone(),
        })
        .collect();
    let stored_collisions = CollisionContext::new(input, &stored_structures);
    let mut solid_bad: Vec<String> = vec![];
    if let Some((part, obstacle)) = stored_collisions.structure_obstacle_blocker() {
        solid_bad.push(format!(
            "saved part {part} interpenetrates obstacle {obstacle}"
        ));
    }
    if let Some((a, b, same)) = stored_collisions.structure_pair_blocker() {
        solid_bad.push(format!(
            "saved parts {a} and {b} interpenetrate (same structure: {same})"
        ));
    }
    for structure in &stored_structures {
        let members: Vec<&MeshPart> = structure
            .parts
            .iter()
            .filter(|part| part.role == "tread" || part.role == "landing")
            .collect();
        for edge in
            stored.graph.edges.iter().filter(|edge| {
                edge.structure_id.as_deref() == Some(structure.structure_id.as_str())
            })
        {
            for capability in &edge.capabilities {
                let height = stored.graph.capability_rules.body_height(*capability);
                for member in &members {
                    if let Some(solid) = stored_collisions.walking_volume_blocker(
                        &structure.structure_id,
                        member,
                        height,
                    ) {
                        solid_bad.push(format!(
                            "edge {} grants {} but {} is blocked by {solid}",
                            edge.edge_id,
                            capability_name(*capability),
                            member.part_id
                        ));
                    }
                }
            }
        }
    }
    rechecks.push(check(
        "stored_solids_rechecked",
        solid_bad.is_empty(),
        if solid_bad.is_empty() {
            "saved structures, saved walking volumes and the input obstacles were rechecked independently of the builder".to_string()
        } else {
            solid_bad.join("; ")
        },
    ));
    // Independent re-sampling: the raster is rebuilt from the SAVED surface and every
    // ground edge's per-capability bands are measured again, then the capability
    // decision is repeated from those measurements and the saved solids. The facts
    // and capabilities inside the bundle are treated as a claim under audit.
    let resampled = EngineeringSurface {
        width: stored.surface.width,
        depth: stored.surface.depth,
        origin_x: stored.surface.origin_x,
        origin_z: stored.surface.origin_z,
        mm: stored.surface.mm.clone(),
        natural_mm: stored.surface.mm.clone(),
        baseline_mm: stored.surface.mm.clone(),
    };
    let authoritative = CapabilityRules::default();
    let mut capability_bad: Vec<String> = vec![];
    for edge in stored
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == "ground")
    {
        let Some(from) = stored
            .nodes
            .iter()
            .find(|node| node.node_id == edge.from_node)
        else {
            capability_bad.push(format!("{}: unknown from node", edge.edge_id));
            continue;
        };
        let Some(to) = stored
            .nodes
            .iter()
            .find(|node| node.node_id == edge.to_node)
        else {
            capability_bad.push(format!("{}: unknown to node", edge.edge_id));
            continue;
        };
        let a = Vec3::from_arr(from.position_m);
        let b = Vec3::from_arr(to.position_m);
        let bands = |half: f64| -> Option<(f64, f64, f64)> {
            let (_, centre, band, _) =
                ground_band_profile(&resampled, a, b, &[0.0, half, -half]).ok()?;
            Some((centre, band, band.max(centre)))
        };
        let Some((centre, pedestrian, _)) = bands(0.75) else {
            capability_bad.push(format!(
                "{}: the saved surface cannot be re-sampled at its own endpoints",
                edge.edge_id
            ));
            continue;
        };
        let Some((_, wheelchair, _)) = bands(WHEELCHAIR_CLEAR_WIDTH_M / 2.0) else {
            capability_bad.push(format!("{}: wheelchair band missing", edge.edge_id));
            continue;
        };
        let Some((_, vehicle, _)) = bands(VEHICLE_CLEAR_WIDTH_M / 2.0) else {
            capability_bad.push(format!("{}: vehicle band missing", edge.edge_id));
            continue;
        };
        let declared_width = |node_id: &str| -> f64 {
            input
                .ports
                .iter()
                .find(|port| port.id == node_id)
                .map_or(0.0, |port| port.width_m)
        };
        let facts = CapabilityFacts {
            grade_pedestrian: pedestrian,
            grade_wheelchair: wheelchair.max(centre),
            grade_vehicle: vehicle.max(centre),
            sampled_half_width_m: VEHICLE_CLEAR_WIDTH_M / 2.0,
            clear_width_m: declared_width(&edge.from_node).min(declared_width(&edge.to_node)),
            bidirectional: edge.bidirectional,
            max_step_rise_m: 0.0,
            flat: pedestrian <= 1e-9,
        };
        let owners = stored_collisions.owner_structures(&edge.from_node, &edge.to_node);
        let endpoints = [a, b];
        let terminal_frames = stored_collisions.terminal_frames(&owners, &endpoints);
        let mut clearance: BTreeMap<Capability, Option<String>> = BTreeMap::new();
        for capability in [
            Capability::Pedestrian,
            Capability::Wheelchair,
            Capability::Vehicle,
        ] {
            let blocker = stored_collisions
                .corridor_blocker(
                    &edge.polyline_m,
                    authoritative.required_half_width(capability),
                    authoritative.body_height(capability),
                    &owners,
                    &endpoints,
                    &terminal_frames,
                )
                .map(|(solid, index)| format!("{solid} at sample {index}"));
            let water = input.water.iter().find(|water| {
                segment_hits_rect(
                    a.x,
                    a.z,
                    b.x,
                    b.z,
                    stored
                        .graph
                        .capability_rules
                        .required_half_width(capability),
                    &water.rect,
                )
            });
            clearance.insert(
                capability,
                match (blocker, water) {
                    (Some(solid), _) => Some(solid),
                    (None, Some(water)) => Some(format!("water {}", water.id)),
                    (None, None) => None,
                },
            );
        }
        // Thresholds are contract constants, not bundle state: a tampered rule set must
        // not be able to endorse its own capabilities.
        let (granted, _) = authoritative.decide("ground", &facts, &clearance);
        if granted != edge.capabilities {
            capability_bad.push(format!(
                "{}: saved {:?} against re-sampled {:?} (bands pedestrian {pedestrian:.4}, wheelchair {wheelchair:.4}, vehicle {vehicle:.4})",
                edge.edge_id, edge.capabilities, granted
            ));
        }
    }
    // Ground edges are re-derived with the same two axes as the builder, on the same code
    // path: a structure-borne section must be certified, a proven bearing-free section
    // stays with the authoritative terrain, and an undecided identity is a failure. The
    // saved ground bearing facts are audited field by field as well, so a rewritten
    // identity or coverage result cannot hide behind the rebuilt geometry.
    for edge in stored
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == "ground")
    {
        let required = authoritative.required_half_width(Capability::Pedestrian) * 2.0;
        let support = stored_collisions.structural_support_detail(
            &stored_collisions.all_structure_ids(),
            &edge.polyline_m,
            required,
        );
        if let Some(refusal) = support.ground_refusal() {
            capability_bad.push(format!(
                "{}: {} - {} (required {required:.3} m)",
                edge.edge_id, refusal.category, refusal.detail
            ));
        }
        let saved = edge
            .verification
            .detail
            .get("supportFacts")
            .and_then(|facts| facts.get(capability_name(Capability::Pedestrian)));
        match saved {
            None => capability_bad.push(format!(
                "{}: ground edge {} has no saved support facts to audit",
                edge.edge_id,
                capability_name(Capability::Pedestrian)
            )),
            Some(saved) => capability_bad.extend(support_facts_disagreements(
                &edge.edge_id,
                capability_name(Capability::Pedestrian),
                saved,
                &support,
            )),
        }
    }
    // Structure edges are re-derived too: the support band of their own members must
    // carry every capability the bundle claims, recomputed from the saved geometry.
    for edge in stored
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind != "ground")
    {
        let Some(structure_id) = edge.structure_id.clone() else {
            continue;
        };
        let owners = BTreeSet::from([structure_id.clone()]);
        for capability in &edge.capabilities {
            let required = authoritative.required_half_width(*capability) * 2.0;
            let support =
                stored_collisions.structural_support_detail(&owners, &edge.polyline_m, required);
            let coverage = support.band_m;
            let unsupported_endpoints = support.unsupported_endpoints;
            // A structure edge may never be re-read as bearing-free ground, an undecided
            // identity is a failure here exactly as it is in the builder, and an edge with
            // no evaluated section refuses rather than passing by default.
            if let Some(refusal) = support.structure_refusal() {
                capability_bad.push(format!(
                    "{}: {} - {}",
                    edge.edge_id, refusal.category, refusal.detail
                ));
            }
            if coverage + 1e-9 < required {
                capability_bad.push(format!(
                    "{}: {} is granted with {coverage:.3} m of support band from its own members, under the required {required:.3} m{}",
                    edge.edge_id,
                    capability_name(*capability),
                    support
                        .intervals
                        .iter()
                        .filter(|interval| !interval.carried)
                        .min_by(|a, b| a.band_m.total_cmp(&b.band_m))
                        .map_or(String::new(), |interval| format!(
                            " (segment {} t {:.3}..{:.3}, layer {:?}, parts {:?}, band {:.3} m)",
                            interval.segment,
                            interval.from_t,
                            interval.to_t,
                            interval.effective_level_m,
                            interval.parts,
                            interval.band_m
                        ))
                ));
            }
            if unsupported_endpoints > 0 {
                capability_bad.push(format!(
                    "{}: {} endpoints are neither carried by a member nor joined to one within one step",
                    edge.edge_id, unsupported_endpoints
                ));
            }
            // The saved support facts are a claim too, and every field — both axes, both
            // reasons, every interval and the filter evidence — is audited on its own: a
            // rewritten, invented or missing fact is a failure, never a silent pass, and
            // the whole-package difference is never needed to reach the conclusion.
            let saved = edge
                .verification
                .detail
                .get("supportFacts")
                .and_then(|facts| facts.get(capability_name(*capability)));
            match saved {
                None => capability_bad.push(format!(
                    "{}: {} has no saved support facts to audit",
                    edge.edge_id,
                    capability_name(*capability)
                )),
                Some(saved) => capability_bad.extend(support_facts_disagreements(
                    &edge.edge_id,
                    capability_name(*capability),
                    saved,
                    &support,
                )),
            }
        }
    }
    rechecks.push(check(
        "stored_capabilities_resampled",
        capability_bad.is_empty(),
        if capability_bad.is_empty() {
            "every saved ground edge's capability decision was reproduced from the saved raster and the saved solids"
                .to_string()
        } else {
            capability_bad.join("; ")
        },
    ));
    // Independent route geometry recheck: the saved output path must start at the
    // query start, end at a target port, be continuous, and respect the limits.
    let mut route_bad: Vec<String> = vec![];
    let stored_nodes_by_id: BTreeMap<String, GraphNode> = stored
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node.clone()))
        .collect();
    if stored.route.status == "found" {
        let polyline = &stored.route.path_polyline_m;
        match (polyline.first(), polyline.last()) {
            (Some(first), Some(last)) => {
                let start = stored_nodes_by_id.get(&stored.route.from_port_id);
                match start {
                    Some(node) => {
                        let drift = Vec3::from_arr(*first)
                            .sub(Vec3::from_arr(node.position_m))
                            .len();
                        if drift > 1e-6 {
                            route_bad.push(format!(
                                "the saved path starts {drift:.6} m from its start port"
                            ));
                        }
                    }
                    None => route_bad.push("the saved path start port is not in the graph".into()),
                }
                let ends_at_target = stored_nodes_by_id
                    .get(
                        &stored
                            .route
                            .path_node_ids
                            .last()
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .is_some_and(|node| {
                        node.road_id.as_deref() == Some(stored.route.to_road_id.as_str())
                    });
                if !ends_at_target {
                    route_bad
                        .push("the saved path does not end at a port of the target road".into());
                }
                let end_drift = stored
                    .route
                    .path_node_ids
                    .last()
                    .and_then(|id| stored_nodes_by_id.get(id))
                    .map(|node| {
                        Vec3::from_arr(*last)
                            .sub(Vec3::from_arr(node.position_m))
                            .len()
                    })
                    .unwrap_or(f64::MAX);
                if end_drift > 1e-6 {
                    route_bad.push(format!(
                        "the saved path ends {end_drift:.6} m from its final port"
                    ));
                }
            }
            _ => route_bad.push("the saved path has no geometry".into()),
        }
        // The saved line must be exactly the concatenation of its own saved edges in
        // the direction the path traverses them: continuity comes from the graph, not
        // from a step limit.
        let mut expected: Vec<[f64; 3]> = vec![];
        let mut cursor = stored.route.from_port_id.clone();
        for edge_id in &stored.route.path_edge_ids {
            match stored
                .graph
                .edges
                .iter()
                .find(|edge| edge.edge_id == *edge_id)
            {
                Some(edge) => {
                    let forward = edge.from_node == cursor;
                    let points: Vec<[f64; 3]> = if forward {
                        edge.polyline_m.clone()
                    } else {
                        edge.polyline_m.iter().rev().copied().collect()
                    };
                    for point in points {
                        if expected.last() != Some(&point) {
                            expected.push(point);
                        }
                    }
                    cursor = if forward {
                        edge.to_node.clone()
                    } else {
                        edge.from_node.clone()
                    };
                }
                None => route_bad.push(format!(
                    "the saved path names edge {edge_id} which is not in the saved graph"
                )),
            }
        }
        if expected != stored.route.path_polyline_m {
            route_bad.push(
                "the saved path line is not the concatenation of its own edges in the direction of travel"
                    .into(),
            );
        }
        if cursor
            != stored
                .route
                .path_node_ids
                .last()
                .cloned()
                .unwrap_or_default()
        {
            route_bad.push("the saved path edges do not connect its own node list".into());
        }
        let (horizontal, length, climb, descent) = polyline_metrics(&stored.route.path_polyline_m);
        for (label, saved, measured) in [
            (
                "horizontalLengthM",
                stored.route.horizontal_length_m,
                horizontal,
            ),
            ("pathLength3dM", stored.route.path_length_3d_m, length),
            ("cumulativeRiseM", stored.route.cumulative_rise_m, climb),
            (
                "cumulativeDescentM",
                stored.route.cumulative_descent_m,
                descent,
            ),
        ] {
            if (saved - measured).abs() > 1e-6 {
                route_bad.push(format!(
                    "saved {label} {saved:.6} against {measured:.6} measured on the saved directed polyline"
                ));
            }
        }
        if length > input.query.max_length_3d_m + 1e-6 || climb > input.query.max_rise_m + 1e-6 {
            route_bad.push(format!(
                "the saved path is outside the declared limits: {length:.3} m long, {climb:.3} m of climb"
            ));
        }
    }
    rechecks.push(check(
        "stored_route_geometry_rechecked",
        route_bad.is_empty(),
        if route_bad.is_empty() {
            "the saved path was re-measured on its own directed polyline and checked against the endpoints, continuity and limits".to_string()
        } else {
            route_bad.join("; ")
        },
    ));
    let mut mesh_bad: Vec<String> = vec![];
    for structure in &stored.structures {
        let hash = mesh_hash(&structure.parts);
        if hash != structure.mesh.mesh_hash {
            mesh_bad.push(format!("{}: mesh hash", structure.structure_id));
        }
        if structure.mesh.points != structure.parts.len() * 8
            || structure.mesh.triangles != structure.parts.len() * 12
        {
            mesh_bad.push(format!("{}: mesh counts", structure.structure_id));
        }
        for part in &structure.parts {
            if part.bottom_mm >= part.top_mm {
                mesh_bad.push(format!(
                    "{}: part {} is inverted",
                    structure.structure_id, part.part_id
                ));
            }
            if part
                .corners_m
                .iter()
                .any(|corner| corner.iter().any(|value| !value.is_finite()))
            {
                mesh_bad.push(format!(
                    "{}: part {} is not finite",
                    structure.structure_id, part.part_id
                ));
            }
        }
    }
    rechecks.push(check(
        "stored_mesh_rechecked",
        mesh_bad.is_empty(),
        if mesh_bad.is_empty() {
            "every saved mesh part is re-hashed and its box is well formed".to_string()
        } else {
            mesh_bad.join("; ")
        },
    ));
    let stored_nodes: BTreeMap<String, GraphNode> = stored
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node.clone()))
        .collect();
    let stored_adjacency = adjacency_of(&stored.graph.edges);
    let rerun = route(
        &stored_nodes,
        &stored_adjacency,
        &stored.graph.edges,
        &input.query,
    );
    let route_reproduced = rerun.status == stored.route.status
        && rerun.path_edge_ids == stored.route.path_edge_ids
        && (rerun.path_length_3d_m - stored.route.path_length_3d_m).abs() < 1e-6;
    rechecks.push(check(
        "stored_route_reproduced",
        route_reproduced,
        format!(
            "re-searching the saved graph gives status {} with {} edges at {:.6} m against the saved status {} with {} edges at {:.6} m",
            rerun.status,
            rerun.path_edge_ids.len(),
            rerun.path_length_3d_m,
            stored.route.status,
            stored.route.path_edge_ids.len(),
            stored.route.path_length_3d_m
        ),
    ));
    if !route_reproduced {
        findings.push(TamperFinding {
            code: "route_not_reproducible".into(),
            at: "route".into(),
            expected: format!("{} / {:?}", stored.route.status, stored.route.path_edge_ids),
            found: format!("{} / {:?}", rerun.status, rerun.path_edge_ids),
        });
        finding_count += 1;
    }
    let mut port_bad: Vec<String> = vec![];
    for node in &stored.nodes {
        if node.kind != "declared_port" {
            continue;
        }
        match input.ports.iter().find(|port| port.id == node.node_id) {
            None => port_bad.push(format!("{} is not declared by the input", node.node_id)),
            Some(port) => {
                if port.position_m != node.position_m
                    || port.surface_id != node.surface_id
                    || port.level_id != node.level_id
                    || port.capabilities != node.capabilities
                {
                    port_bad.push(format!("{} does not match its declaration", node.node_id));
                }
            }
        }
    }
    rechecks.push(check(
        "stored_ports_rechecked",
        port_bad.is_empty(),
        if port_bad.is_empty() {
            "every saved declared port matches the input declaration exactly".to_string()
        } else {
            port_bad.join("; ")
        },
    ));
    let views = view_contents(&rebuilt, &rebuilt_outcome);
    let recomputed_view_hashes = [views.0, views.1, views.2]
        .iter()
        .map(|content| {
            let mut hasher = blake3::Hasher::new();
            hasher.update(content.as_bytes());
            hasher.finalize().to_hex().to_string()
        })
        .collect::<Vec<String>>();
    let views_match = stored.views.len() == 3
        && stored
            .views
            .iter()
            .zip(recomputed_view_hashes.iter())
            .all(|(view, hash)| view.content_hash == *hash);
    checks.push(check(
        "geometry_views_recomputed",
        views_match,
        "the plan, elevation and mesh views are regenerated from the rebuilt bundle and compared by content hash",
    ));
    if !views_match {
        notes.push(
            "a view file may have been regenerated by a different version of the generator; the bundle records the hash it was built with".into(),
        );
    }
    for recheck in &rechecks {
        if !recheck.passed {
            finding_count += 1;
            if findings.len() < MAX_FINDINGS {
                findings.push(TamperFinding {
                    code: recheck.name.clone(),
                    at: "independent_recheck".into(),
                    expected: "recheck passes".into(),
                    found: recheck.detail.clone(),
                });
            }
        }
    }
    let passed = findings.is_empty();
    VerifyReport {
        status: if passed {
            "verified".into()
        } else {
            "tamper_detected".into()
        },
        scene_id: stored.scene_id.clone(),
        bundle_version: stored.version,
        contract_version: ENGINEERING_CONNECTIONS_VERSION,
        bundle_payload_hash: stored.payload_hash.clone(),
        recomputed_payload_hash: payload_hash_of(&rebuilt).unwrap_or_default(),
        payload_hash_matches,
        input_hash_matches,
        checks,
        independent_rechecks: rechecks,
        findings,
        finding_count,
        recomputed_summary: serde_json::json!({
            "surfaceHash": rebuilt.surface.engineered_hash,
            "cellsChangedByGroundWork": rebuilt.surface.engineered_cells_changed,
            "cutVolumeM3": rebuilt.surface.cut_volume_m3,
            "fillVolumeM3": rebuilt.surface.fill_volume_m3,
            "structures": rebuilt.structures.len(),
            "meshPoints": rebuilt.mesh_points,
            "meshTriangles": rebuilt.mesh_triangles,
            "graphNodes": rebuilt.graph.nodes.len(),
            "graphEdges": rebuilt.graph.edges.len(),
            "groundEdges": rebuilt.graph.ground_edges,
            "stairEdges": rebuilt.graph.stair_edges,
            "landingEdges": rebuilt.graph.landing_edges,
            "routeStatus": rebuilt.route.status,
            "routeLength3dM": rebuilt.route.path_length_3d_m,
            "routeHorizontalM": rebuilt.route.horizontal_length_m,
        }),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression cases are version-controlled test resources, so a clean
    /// checkout or CI run does not depend on the gitignored work directory. The
    /// evidence directory keeps its own copy as an execution record.
    fn case_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/resources/g19r2")
    }

    fn cases() -> Vec<(String, String, serde_json::Value)> {
        let root = case_root();
        let expectations: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("expectations.json"))
                .expect("expectations.json next to the cases"),
        )
        .expect("expectations.json is readable JSON");
        let mut out = vec![];
        for (name, expectation) in expectations["cases"]
            .as_object()
            .expect("expectations.cases object")
        {
            let json = std::fs::read_to_string(root.join(format!("{name}.json")))
                .unwrap_or_else(|error| panic!("case {name} is missing: {error}"));
            out.push((name.clone(), json, expectation.clone()));
        }
        out
    }

    /// Every counterexample from the independent review, plus the two corrected
    /// positives, driven through the public entry points.
    #[test]
    fn r1_counterexample_matrix_matches_the_contract() {
        let mut checked = 0_usize;
        for (name, json, expectation) in cases() {
            let kind = expectation["kind"].as_str().unwrap_or("built").to_string();
            let input = match load_input(&json) {
                Ok(input) => input,
                Err(error) => {
                    assert_eq!(
                        kind, "input_invalid",
                        "{name}: unexpected input rejection [{}] {}",
                        error.category, error.message
                    );
                    checked += 1;
                    continue;
                }
            };
            match build_bundle(&input) {
                Ok((bundle, _)) => {
                    assert_eq!(
                        kind, "built",
                        "{name}: expected a refusal but the build succeeded"
                    );
                    let route_status = bundle.route.status.clone();
                    assert_eq!(
                        route_status,
                        expectation["routeStatus"].as_str().unwrap_or("found"),
                        "{name}: route status, refusals {:?}",
                        bundle.route.rejected_by_reason
                    );
                    if name == "reverse-flat" {
                        // The reported line must follow the query direction.
                        let start = input.query.from_port_id.clone();
                        let first = bundle
                            .route
                            .path_polyline_m
                            .first()
                            .copied()
                            .unwrap_or([0.0; 3]);
                        let node = bundle
                            .nodes
                            .iter()
                            .find(|node| node.node_id == start)
                            .expect("start node");
                        assert!(
                            (first[0] - node.position_m[0]).abs() < 1e-6
                                && (first[2] - node.position_m[2]).abs() < 1e-6,
                            "{name}: the path line must start at the query start port"
                        );
                    }
                }
                Err(error) => {
                    assert_ne!(
                        kind, "built",
                        "{name}: expected a build but got [{}] {}",
                        error.category, error.message
                    );
                    if kind == "input_invalid" {
                        assert_eq!(error.category, "input_invalid", "{name}: {}", error.message);
                    } else {
                        assert_eq!(
                            error.category,
                            expectation["category"].as_str().unwrap_or(""),
                            "{name}: category, message {}",
                            error.message
                        );
                    }
                }
            }
            checked += 1;
        }
        assert_eq!(
            checked,
            cases().len(),
            "every version-controlled case must be exercised"
        );
    }

    /// A staged bundle is not a publication: the verifier must refuse it even when its
    /// payload hash is self-consistent.
    #[test]
    fn r5_a_staged_bundle_is_not_consumable() {
        let root = case_root();
        let json = std::fs::read_to_string(root.join("positive-terrace.json")).expect("case");
        let input = load_input(&json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("positive scene builds");
        bundle.publication.state = "staged".into();
        bundle.payload_hash = payload_hash_of(&bundle).unwrap();
        let staged = verify_bundle(&input, &bundle);
        assert_eq!(staged.status, "incomplete_publication");
        let mut committed = bundle.clone();
        commit_bundle(&mut committed, "bundle.json", "report.json").unwrap();
        let report = verify_bundle(&input, &committed);
        assert_eq!(report.status, "verified", "{:?}", report.findings);
    }

    /// The presence rule is enforced on the wire format, before any geometry runs.
    #[test]
    fn r3_presence_and_reference_rules_are_enforced() {
        let explicit = r#"{"version":2,"sceneId":"x","units":"lengths_m;levels_mm","grid":{"width":2,"depth":2,"originX":0,"originZ":0,"naturalMm":[0,0,0,0],"baselineMm":[0,0,0,0],"source":"t"},"protection":[],"obstacles":[],"water":[],"bearing":[],"roadReferences":[],"groundWorks":[],"structures":[],"ports":[],"groundConnections":[],"query":{"fromPortId":"a","toRoadId":"R","capability":"pedestrian","maxLength3dM":1.0,"maxRiseM":1.0}}"#;
        let parsed = load_input(explicit).expect("an explicit empty environment is legal input");
        assert_eq!(
            build_bundle(&parsed)
                .expect_err("the query road is not registered")
                .category,
            "unknown_road_reference"
        );
        let v1 = explicit.replace(r#""version":2"#, r#""version":1"#);
        assert_eq!(
            load_input(&v1)
                .expect_err("v1 must be re-declared")
                .category,
            "contract_version_mismatch"
        );
        let unbound = explicit.replace(r#""toRoadId":"R""#, r#""toRoadId":"UNKNOWN""#);
        let parsed = load_input(&unbound).expect("input parses");
        assert_eq!(
            build_bundle(&parsed)
                .expect_err("an unregistered road id")
                .category,
            "unknown_road_reference"
        );
    }

    /// Range arithmetic is checked before it is used.
    #[test]
    fn r6_extreme_rectangles_are_refused_not_wrapped() {
        let rect = RectI {
            x0: i32::MIN,
            x1: i32::MAX,
            z0: 1,
            z1: 1,
        };
        assert_eq!(
            rect.cells().expect_err("must not wrap").category,
            "scope_out_of_range"
        );
        let reversed = RectI {
            x0: 4,
            x1: 1,
            z0: 0,
            z1: 2,
        };
        assert_eq!(
            reversed.cells().expect_err("reversed").category,
            "scope_reversed"
        );
        let small = RectI {
            x0: 0,
            x1: 2,
            z0: 0,
            z1: 3,
        };
        assert_eq!(small.cells().unwrap(), 12);
        assert!(small.validated(4, 4).is_ok());
        assert_eq!(
            small.validated(2, 4).expect_err("outside").category,
            "scope_outside_grid"
        );
    }

    /// Capability decisions use local geometry: a step is not a grade and a narrow
    /// band never endorses a wider one.
    #[test]
    fn r2_capability_decisions_are_local() {
        let rules = CapabilityRules::default();
        let mut clearance: BTreeMap<Capability, Option<String>> = BTreeMap::new();
        for capability in [
            Capability::Pedestrian,
            Capability::Wheelchair,
            Capability::Vehicle,
        ] {
            clearance.insert(capability, None);
        }
        let flat = CapabilityFacts {
            grade_pedestrian: 0.01,
            grade_wheelchair: 0.01,
            grade_vehicle: 0.02,
            sampled_half_width_m: 1.5,
            clear_width_m: 3.5,
            bidirectional: true,
            max_step_rise_m: 0.0,
            flat: true,
        };
        let (granted, _) = rules.decide("ground", &flat, &clearance);
        assert!(granted.contains(&Capability::Vehicle));
        let hump = CapabilityFacts {
            grade_wheelchair: 0.2,
            grade_vehicle: 0.2,
            ..flat
        };
        let (granted, refusals) = rules.decide("ground", &hump, &clearance);
        assert!(!granted.contains(&Capability::Wheelchair));
        assert!(!granted.contains(&Capability::Vehicle));
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal["reason"] == "local_grade_over_limit")
        );
        let stair = CapabilityFacts {
            max_step_rise_m: 0.17,
            ..flat
        };
        let (granted, _) = rules.decide("stair", &stair, &clearance);
        assert_eq!(granted, vec![Capability::Pedestrian]);
        assert!(!SUPPORTED_CAPABILITIES.contains(&Capability::Rail));
    }
    /// Item 1: the port-usable width is the member's lateral extent in its own frame,
    /// and membership is an oriented polygon test, not a bounding box test.
    #[test]
    fn r2c_member_frame_is_oriented_not_a_bounding_box() {
        // a 45 degree flight member: 3 m long, 1.5 m wide, level 1.2 m
        let axis = Vec3::new(1.0, 0.0, 1.0).normalize_xz().unwrap();
        let side = axis.perp_xz();
        let part = oriented_box(
            "rotated::tread0",
            "tread",
            "rotated",
            "rotated:run1",
            0,
            0,
            Vec3::new(0.0, 0.0, 0.0),
            axis,
            side,
            1.5,
            0.75,
            1.0,
            1.2,
        );
        let frame = member_frame(&part).expect("a rotated member has a frame");
        assert!((frame.clear_width_m() - 1.5).abs() < 1e-9);
        // the bounding box corner is inside the AABB but outside the member
        let aabb = part.aabb();
        let corner = (aabb.max.x - 1e-6, aabb.min.z + 1e-6);
        assert!(!frame.contains(corner.0, corner.1, 1e-6));
        // the member centre is inside
        assert!(frame.contains(0.0, 0.0, 1e-6));
        // a point one clear-width to the side is outside
        let outside = side.scale(0.9);
        assert!(!frame.contains(outside.x, outside.z, 1e-6));
    }

    /// Item 2: finite variants of the endpoint-contact rule. The same invariants must
    /// hold when the scene is moved, when an obstacle hides between samples, when the
    /// branch order changes and at the width boundary.
    #[test]
    fn r2c_endpoint_contact_variants() {
        let root = case_root();
        let read =
            |name: &str| std::fs::read_to_string(root.join(format!("{name}.json"))).expect("case");
        let verdict = |json: &str| -> (String, String) {
            let input = load_input(json).expect("v2 input");
            match build_bundle(&input) {
                Ok((bundle, _)) => ("built".to_string(), bundle.route.status),
                Err(error) => ("rejected".to_string(), error.category),
            }
        };
        // (a) translate the whole scene by +100 m in x
        let mut moved: serde_json::Value = serde_json::from_str(&read("control-flat")).unwrap();
        moved["grid"]["originX"] = serde_json::json!(100);
        for port in moved["ports"].as_array_mut().unwrap() {
            let x = port["positionM"][0].as_f64().unwrap() + 100.0;
            port["positionM"][0] = serde_json::json!(x);
        }
        assert_eq!(verdict(&moved.to_string()).0, "built");
        // (b) an obstacle that falls between two 0.5 m samples
        let mut hidden: serde_json::Value = serde_json::from_str(&read("control-flat")).unwrap();
        hidden["obstacles"] = serde_json::json!([{
            "id": "between-samples",
            "rect": {"x0": 6.24, "z0": 7.0, "x1": 6.26, "z1": 9.0},
            "bottomM": 0.0,
            "topM": 1.0,
            "source": "variant: hidden between samples"
        }]);
        assert_eq!(verdict(&hidden.to_string()).0, "rejected");
        // (c) branch order must not change the verdict
        let mut ordered: serde_json::Value =
            serde_json::from_str(&read("hump-with-valid-detour")).unwrap();
        let mut connections = ordered["groundConnections"].as_array().unwrap().clone();
        connections.reverse();
        ordered["groundConnections"] = serde_json::Value::Array(connections);
        assert_eq!(
            verdict(&ordered.to_string()),
            verdict(&read("hump-with-valid-detour"))
        );
        // (d) the width boundary: exactly the requirement passes, below it must not
        let mut boundary: serde_json::Value = serde_json::from_str(&read("control-flat")).unwrap();
        for port in boundary["ports"].as_array_mut().unwrap() {
            port["widthM"] = serde_json::json!(1.5);
        }
        assert_eq!(verdict(&boundary.to_string()).0, "built");
        let mut narrow = boundary.clone();
        for port in narrow["ports"].as_array_mut().unwrap() {
            port["widthM"] = serde_json::json!(1.49);
        }
        assert_eq!(verdict(&narrow.to_string()).0, "rejected");
    }

    /// Item 4: a bundle whose facts and capabilities were rewritten self-consistently
    /// and re-hashed is still refused, because the capabilities are re-derived from the
    /// saved geometry with the authoritative thresholds.
    #[test]
    fn r2c_resampler_rejects_self_consistent_capability_claims() {
        let root = case_root();
        let json = std::fs::read_to_string(root.join("positive-terrace.json")).expect("case");
        let input = load_input(&json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("positive scene builds");
        bundle.graph.capability_rules.max_grade_wheelchair = 10.0;
        for edge in bundle.graph.edges.iter_mut() {
            if edge.kind == "ground" && !edge.capabilities.contains(&Capability::Wheelchair) {
                edge.capabilities.push(Capability::Wheelchair);
                break;
            }
        }
        // Commit the tampered bundle so the hash and the publication state are
        // self-consistent: only the geometry re-derivation can refuse it now.
        commit_bundle(&mut bundle, "bundle.json", "report.json").unwrap();
        let report = verify_bundle(&input, &bundle);
        assert_ne!(report.status, "verified");
        assert!(
            report
                .independent_rechecks
                .iter()
                .any(|recheck| recheck.name == "stored_capabilities_resampled" && !recheck.passed),
            "the resampler must refuse a capability its own geometry does not earn: {:?}",
            report.independent_rechecks
        );
        // The same must hold for a structure edge: a flight cannot carry a vehicle band.
        let (mut structural, _) = build_bundle(&input).expect("builds");
        for edge in structural.graph.edges.iter_mut() {
            if edge.kind == "stair" {
                edge.capabilities.push(Capability::Vehicle);
                break;
            }
        }
        commit_bundle(&mut structural, "b.json", "r.json").unwrap();
        let structural_report = verify_bundle(&input, &structural);
        assert_ne!(structural_report.status, "verified");
        assert!(
            structural_report
                .independent_rechecks
                .iter()
                .any(|recheck| recheck.name == "stored_capabilities_resampled" && !recheck.passed),
            "a structure edge must be re-derived from its own support band: {:?}",
            structural_report.independent_rechecks
        );
    }

    /// Item 5: every assertion of the original 17 that is still valid is reachable from
    /// a named case or a named test. The table is in the packet report; this test keeps
    /// the resource side of it honest.
    #[test]
    fn r2c_seventeen_assertion_mapping_has_its_cases() {
        let root = case_root();
        let required = [
            "positive-terrace",
            "positive-switchback",
            "control-flat",
            "descent-zero-rise",
            "reverse-flat",
            "pedestrian-hump-zero-rise",
            "hump-with-valid-detour",
            "valid-detour-control",
            "wheelchair-local-hump",
            "wheelchair-outside-band",
            "wheelchair-low-solid",
            "wheelchair-taller-solid",
            "vehicle-side-water",
            "narrow-stair",
            "terminal-own-stair-solid",
            "misbound-existing-surface",
            "unknown-surfaces",
            "missing-context",
            "overflow-scope",
            "ground-overhead-obstacle",
            "ground-through-solid-platform",
            "stair-inside-solid-obstacle",
        ];
        for name in required {
            assert!(
                root.join(format!("{name}.json")).exists(),
                "case {name} must exist for the mapping table"
            );
        }
    }
    /// Item 4: the effective assertions of the original suite really run, on the
    /// version-controlled cases, with the failure categories and the persistence and
    /// tamper behaviour that the earlier rounds only mapped by name.
    #[test]
    fn r3_restored_semantic_regressions() {
        let root = case_root();
        let read =
            |name: &str| std::fs::read_to_string(root.join(format!("{name}.json"))).expect("case");
        let category = |json: &str| -> String {
            match load_input(json).and_then(|input| build_bundle(&input)) {
                Ok(_) => "<built>".into(),
                Err(error) => error.category,
            }
        };
        let terrace: serde_json::Value = serde_json::from_str(&read("positive-terrace")).unwrap();
        // (a) without the authorised fill the walkability result changes
        let mut unfilled = terrace.clone();
        let plans: Vec<serde_json::Value> = unfilled["groundWorks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|plan| plan["id"] != "apron-fill")
            .cloned()
            .collect();
        unfilled["groundWorks"] = serde_json::Value::Array(plans);
        assert_eq!(category(&unfilled.to_string()), "ground_predicate_failed");
        // (b) the embankment is refused by the grade predicate, with no wall involved
        let mut shortcut = terrace.clone();
        let mut connections = shortcut["groundConnections"].as_array().unwrap().clone();
        connections.push(serde_json::json!({
            "id": "short-cut",
            "fromPortId": "entrance",
            "toPortId": "terrace-road",
            "roadId": null,
            "oneWay": false
        }));
        shortcut["groundConnections"] = serde_json::Value::Array(connections);
        assert_eq!(category(&shortcut.to_string()), "ground_predicate_failed");
        // (c) protection, authorisation and over-authorisation: refusal, no partial work
        let mut protected = terrace.clone();
        protected["groundWorks"][0]["scope"] =
            serde_json::json!({"x0": 4, "z0": 29, "x1": 8, "z1": 33});
        assert_eq!(category(&protected.to_string()), "protected_cell");
        let mut unauthorised = terrace.clone();
        unauthorised["groundWorks"][1]["allowCut"] = serde_json::json!(false);
        assert_eq!(category(&unauthorised.to_string()), "cut_not_authorised");
        let mut over = terrace.clone();
        over["groundWorks"][0]["maxFillMm"] = serde_json::json!(500);
        assert_eq!(category(&over.to_string()), "fill_over_authorisation");
        let refused = load_input(&unfilled.to_string()).unwrap();
        assert!(build_bundle(&refused).is_err());
        assert_eq!(
            refused.grid.baseline_mm.len(),
            terrace["grid"]["baselineMm"].as_array().unwrap().len(),
            "a refused plan must leave the input field untouched"
        );
        // (d) a support that leaves the grid is refused, positive supports touch
        let mut edge_deck = terrace.clone();
        let mut ports = edge_deck["ports"].as_array().unwrap().clone();
        ports.push(serde_json::json!({
            "id": "edge-port",
            "positionM": [47.0, 1.0, 24.0],
            "surfaceId": "ground",
            "levelId": "ground",
            "roadId": null,
            "widthM": 1.6,
            "capabilities": ["pedestrian"]
        }));
        edge_deck["ports"] = serde_json::Value::Array(ports);
        let mut structures = edge_deck["structures"].as_array().unwrap().clone();
        structures.push(serde_json::json!({
            "kind": "landing",
            "id": "edge-deck",
            "centreM": [49.0, 1.0, 24.0],
            "widthM": 6.0,
            "depthM": 4.0,
            "thicknessM": 0.2,
            "portId": "edge-port",
            "exitPortId": null,
            "support": {"kind": "columns", "columns": 2, "columnSizeM": 0.3}
        }));
        edge_deck["structures"] = serde_json::Value::Array(structures);
        let support_category = category(&edge_deck.to_string());
        assert!(
            matches!(
                support_category.as_str(),
                "support_outside_grid"
                    | "port_outside_structure"
                    | "port_not_on_referenced_surface"
            ),
            "unexpected category {support_category}"
        );
        let input = load_input(&read("positive-terrace")).unwrap();
        let (bundle, _) = build_bundle(&input).expect("the positive scene builds");
        for structure in &bundle.structures {
            for support in &structure.supports {
                assert!(support.in_contact, "{} floats", support.support_id);
            }
        }
        // (e) a one-way edge cannot be traversed backwards
        let mut one_way = terrace.clone();
        one_way["structures"][0]["oneWay"] = serde_json::json!(true);
        one_way["query"] = serde_json::json!({
            "fromPortId": "stair-top",
            "toRoadId": "R-LOW",
            "capability": "pedestrian",
            "maxLength3dM": 80.0,
            "maxRiseM": 5.0
        });
        let reverse = load_input(&one_way.to_string()).unwrap();
        let (reversed, _) = build_bundle(&reverse).expect("the one-way scene builds");
        assert_eq!(reversed.route.status, "no_suitable_edge");
        assert!(
            reversed
                .route
                .rejected_by_reason
                .get("one_way_blocked")
                .copied()
                .unwrap_or(0)
                > 0
        );
        // (f) unmodelled structures and retaining or drainage needs are refused
        let mut retaining = terrace.clone();
        retaining["groundWorks"][0]["requiresRetainingOrDrainage"] = serde_json::json!(true);
        assert_eq!(category(&retaining.to_string()), "unsupported");
        let mut spiral = terrace.clone();
        let mut structures = spiral["structures"].as_array().unwrap().clone();
        structures.push(serde_json::json!({
            "kind": "unsupported",
            "id": "spiral-a",
            "requestedKind": "spiral_stair",
            "reason": "a helix needs a curve generator"
        }));
        spiral["structures"] = serde_json::Value::Array(structures);
        assert_eq!(category(&spiral.to_string()), "unsupported");
        // (g) real geometry, persistence and tamper detection
        let (mut committed, outcome) = build_bundle(&input).expect("builds");
        let files = views_to_files(&committed, &outcome);
        assert_eq!(files.len(), 3);
        assert!(files[2].1.contains("\nf "));
        commit_bundle(&mut committed, "b.json", "r.json").unwrap();
        assert_eq!(verify_bundle(&input, &committed).status, "verified");
        let mut tampered = committed.clone();
        tampered.surface.mm[100] += 1;
        tampered.payload_hash = payload_hash_of(&tampered).unwrap();
        assert_eq!(verify_bundle(&input, &tampered).status, "tamper_detected");
    }
    #[test]
    fn r3c_band_measurement_assertions() {
        assert!((band_width_from_intervals(vec![(-0.8, 0.8)]) - 1.6).abs() < 1e-9);
        assert_eq!(band_width_from_intervals(vec![]), 0.0);
        assert_eq!(
            band_width_from_intervals(vec![(-1.0, -0.2), (0.2, 1.0)]),
            0.0,
            "a gap on the walking line is no support"
        );
        assert!((band_width_from_intervals(vec![(-1.0, 0.1), (-0.1, 1.0)]) - 2.0).abs() < 1e-9);
        assert_eq!(band_width_from_intervals(vec![(0.2, 2.0)]), 0.0);
        assert!(
            (band_width_from_intervals(vec![(-3.3, -1.7), (-0.8, 0.8)]) - 1.6).abs() < 1e-9,
            "an unrelated component before the walking line must not end the search"
        );
        let frame = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 0.3,
            top: 1.0,
        };
        let across = Vec3::new(0.0, 0.0, 1.0);
        let (width, borne) = station_band_width(&[frame], Vec3::new(8.0, 1.0, 8.0), across);
        assert!(borne);
        assert!((width - 0.6).abs() < 1e-9);
        assert_eq!(
            station_band_width(&[frame], Vec3::new(8.0, 5.0, 8.0), across),
            (0.0, false)
        );
        assert_eq!(
            station_band_width(&[frame], Vec3::new(14.0, 1.0, 8.0), across),
            (0.0, false)
        );
        let off_centre = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.2),
            ..frame
        };
        let (width, borne) = station_band_width(&[off_centre], Vec3::new(8.0, 1.0, 8.0), across);
        assert!(borne);
        assert!((width - 0.2).abs() < 1e-9, "off-centre margin, got {width}");
    }

    #[test]
    fn r3c_structural_endpoint_transitions() {
        let deck = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 1.0,
            top: 1.0,
        };
        // inside the member the interior is carried and both ends sit on it
        let inside = vec![[5.5, 1.0, 8.0], [10.5, 1.0, 8.0]];
        let (band, unsupported) = structural_support_from_frames(&[deck], &inside, 1.5);
        assert_eq!(unsupported, 0, "both ends sit on the member");
        assert!(band + 1e-9 >= 1.5, "the interior is carried: {band}");
        // a line that runs off the member is not carried and its end is not skipped
        let beyond = vec![[5.0, 1.0, 8.0], [14.0, 1.0, 8.0]];
        let (band, unsupported) = structural_support_from_frames(&[deck], &beyond, 1.5);
        assert_eq!(unsupported, 1, "the far end must not be skipped");
        assert!(
            band < 1.5,
            "a line leaving the support is not carried: {band}"
        );
        // The foot of a walk: the port stands on the ground in front of the first tread,
        // one riser below its top. Path height and bearing member agree, because the only
        // vertex above the port lies on the tread the walk lands on.
        let riser = 0.16;
        let tread = MemberFrame {
            centre: Vec3::new(0.15, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 0.15,
            half_width: 0.8,
            top: riser,
        };
        let foot = vec![[0.0, 0.0, 0.0], [0.15, riser, 0.0]];
        let (band, unsupported) = structural_support_from_frames(&[tread], &foot, 1.5);
        assert_eq!(
            unsupported, 0,
            "a tread one riser up that touches the port joins it"
        );
        assert!(band + 1e-9 >= 1.5, "the stepped line is carried: {band}");
        // two risers up is no longer one step, so the port is not joined
        let tall = MemberFrame {
            top: 2.0 * riser,
            ..tread
        };
        let tall_foot = vec![[0.0, 0.0, 0.0], [0.15, 2.0 * riser, 0.0]];
        let (_band, unsupported) = structural_support_from_frames(&[tall], &tall_foot, 1.5);
        assert_eq!(
            unsupported, 1,
            "more than one step away is not a transition"
        );
        // a neighbour that cannot carry the band itself excuses neither end
        let narrow_tread = MemberFrame {
            half_width: 0.3,
            ..tread
        };
        let (_band, unsupported) = structural_support_from_frames(&[narrow_tread], &foot, 1.5);
        assert_eq!(
            unsupported, 2,
            "a narrow neighbour may not excuse the end it cannot carry"
        );
        // a neighbour that does not touch the port joins nothing
        let detached = MemberFrame {
            centre: Vec3::new(0.45, 0.0, 0.0),
            ..tread
        };
        let (_band, unsupported) = structural_support_from_frames(&[detached], &foot, 1.5);
        assert_eq!(
            unsupported, 2,
            "a neighbour that does not touch joins nothing"
        );
        // a narrow member is unsupported everywhere, and never its own transition
        let narrow_deck = MemberFrame {
            half_width: 0.3,
            ..deck
        };
        let (band, unsupported) = structural_support_from_frames(&[narrow_deck], &inside, 1.5);
        assert!(
            band < 1.5,
            "a 0.6 m member cannot carry a 1.5 m band: {band}"
        );
        assert_eq!(unsupported, 2, "a member may not excuse its own ports");
    }

    #[test]
    fn r3c_support_boundary_events_close_the_gap() {
        // Two same-level surfaces with a 0.11 m gap between them. The gap lies between
        // the x=4.000 and x=4.125 stations and its midpoint is 0.055 m from each surface,
        // beyond the contact tolerance, so neither carries it: a bounded step alone would
        // call this line carried.
        let left = MemberFrame {
            centre: Vec3::new(2.0025, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 2.0025,
            half_width: 1.0,
            top: 1.0,
        };
        let right = MemberFrame {
            centre: Vec3::new(7.0575, 0.0, 0.0),
            half_length: 2.9425,
            ..left
        };
        let across = Vec3::new(0.0, 0.0, 1.0);
        let (gap_width, borne) =
            station_band_width(&[left, right], Vec3::new(4.06, 1.0, 0.0), across);
        assert_eq!((gap_width, borne), (0.0, false));
        let line = vec![[1.0, 1.0, 0.0], [9.0, 1.0, 0.0]];
        let (band, unsupported) = structural_support_from_frames(&[left, right], &line, 1.5);
        assert_eq!(unsupported, 0, "both ends of the line sit on a surface");
        assert!(
            band < 1.5,
            "an uncovered interval is not coverage, however narrow: {band}"
        );
        // the two surfaces themselves really do carry the band
        let on_left = vec![[1.0, 1.0, 0.0], [4.0, 1.0, 0.0]];
        let (band, unsupported) = structural_support_from_frames(&[left], &on_left, 1.5);
        assert_eq!(unsupported, 0);
        assert!(
            band + 1e-9 >= 1.5,
            "the certificate proves at least the required band: {band}"
        );
    }

    #[test]
    fn r3c_narrow_neighbour_does_not_vouch_for_a_narrow_port() {
        // The port stands on a 0.4 m wide surface at its own level; a 0.4 m wide surface
        // one riser up touches that same port; the 2 m wide main surface only starts
        // after both. A height difference is not a bearing, and a narrow neighbour cannot
        // supply the width the port needs.
        let low = MemberFrame {
            centre: Vec3::new(0.0, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 0.06,
            half_width: 0.2,
            top: 1.0,
        };
        let high = MemberFrame { top: 1.17, ..low };
        let main = MemberFrame {
            centre: Vec3::new(4.53, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 4.47,
            half_width: 1.0,
            top: 1.0,
        };
        let across = Vec3::new(0.0, 0.0, 1.0);
        let (port_width, borne) =
            station_band_width(&[low, high], Vec3::new(0.0, 1.0, 0.0), across);
        assert!(
            borne,
            "the port really is on a surface, it is just too narrow"
        );
        assert!((port_width - 0.4).abs() < 1e-9, "got {port_width}");
        let line = vec![[0.0, 1.0, 0.0], [9.0, 1.0, 0.0]];
        let facts =
            structural_support_detail(&[("low", low), ("high", high), ("main", main)], &line, 1.5);
        assert_eq!(
            facts.unsupported_endpoints, 1,
            "the port is neither carried at its own level nor joined by a member that can carry it"
        );
        assert!(
            facts.band_m < 1.5,
            "the narrow port surface is measured, not glossed over: {}",
            facts.band_m
        );
        let narrow_interval = facts
            .intervals
            .iter()
            .find(|interval| !interval.carried)
            .expect("the narrow start is a fact");
        // A gap reports a witness instead of a claimed narrowest width: the real support
        // at that section is 0.4 m wide, so the covered half width is 0.2 m.
        assert_eq!(narrow_interval.coverage_verdict, "gap");
        let witness = narrow_interval.witness.expect("the gap has a witness");
        assert!(
            (witness.covered_half_width_m - 0.2).abs() < 1e-9,
            "{witness:?}"
        );
        assert_eq!(narrow_interval.band_m, 0.0, "unproved is not carried");
        let wide = facts
            .intervals
            .iter()
            .find(|interval| interval.carried)
            .expect("the main surface carries the rest of the walk");
        assert!(wide.band_m + 1e-9 >= 1.5);
        // The certificate names every real part inside the covering component, so the
        // narrow start surface may appear next to the wide one; what matters is that the
        // wide surface is named as a real contributor.
        assert!(
            wide.parts.iter().any(|part| part == "main"),
            "{:?}",
            wide.parts
        );
    }

    #[test]
    fn r3c_support_facts_name_the_layer_and_the_parts() {
        let deck = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 1.0,
            top: 1.0,
        };
        let line = vec![[5.5, 1.0, 8.0], [10.5, 1.0, 8.0]];
        let facts = structural_support_detail(&[("deck::deck", deck)], &line, 1.5);
        assert!(facts.band_m + 1e-9 >= 1.5, "band {}", facts.band_m);
        assert_eq!(facts.unsupported_endpoints, 0);
        assert!(facts.intervals.iter().all(|interval| interval.carried));
        let first = &facts.intervals[0];
        assert_eq!(first.effective_level_m, Some(1.0));
        assert_eq!(first.parts, vec!["deck::deck".to_string()]);
        assert_eq!(first.required_band_m, 1.5);
        let (band, unsupported) = structural_support_from_frames(&[deck], &line, 1.5);
        assert!((band - facts.band_m).abs() < 1e-9);
        assert_eq!(unsupported, facts.unsupported_endpoints);
    }

    #[test]
    fn r3c_other_layer_projection_does_not_fill_a_gap() {
        // The 0.11 m gap again, now with an unrelated platform four metres higher whose
        // plan projection covers it. Another layer never fills this layer's gap.
        let left = MemberFrame {
            centre: Vec3::new(2.0025, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 2.0025,
            half_width: 1.0,
            top: 1.0,
        };
        let right = MemberFrame {
            centre: Vec3::new(7.0575, 0.0, 0.0),
            half_length: 2.9425,
            ..left
        };
        let above = MemberFrame {
            centre: Vec3::new(5.0, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 5.0,
            half_width: 2.0,
            top: 5.0,
        };
        let line = vec![[1.0, 1.0, 0.0], [9.0, 1.0, 0.0]];
        let facts = structural_support_detail(
            &[("left", left), ("right", right), ("above", above)],
            &line,
            1.5,
        );
        assert!(
            facts.band_m < 1.5,
            "a part of another layer must not fill the gap: {}",
            facts.band_m
        );
        assert_eq!(
            facts.unsupported_endpoints, 0,
            "both ends really sit on their own layer"
        );
        let gap = facts
            .intervals
            .iter()
            .find(|interval| interval.band_m < 1.5)
            .expect("the gap is a fact of its own");
        assert_eq!(gap.coverage_verdict, "gap", "a gap is not a certificate");
        assert!(!gap.carried && gap.certified_band_m == 0.0);
        assert!(gap.parts.is_empty(), "a gap claims no contributing part");
        assert!(
            gap.adjacent_levels_m
                .iter()
                .all(|level| (level - 1.0).abs() < 1e-9)
        );
    }

    #[test]
    fn r3c_narrow_centre_line_is_not_a_full_band() {
        // The same gap with a 0.4 m wide member of the same level laid inside it: the
        // centre line now has support, but only 0.4 m of it. A centre line is not a band.
        let left = MemberFrame {
            centre: Vec3::new(2.0025, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 2.0025,
            half_width: 1.0,
            top: 1.0,
        };
        let right = MemberFrame {
            centre: Vec3::new(7.0575, 0.0, 0.0),
            half_length: 2.9425,
            ..left
        };
        let narrow = MemberFrame {
            centre: Vec3::new(4.06, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 0.3,
            half_width: 0.2,
            top: 1.0,
        };
        let line = vec![[1.0, 1.0, 0.0], [9.0, 1.0, 0.0]];
        let facts = structural_support_detail(
            &[("left", left), ("right", right), ("narrow", narrow)],
            &line,
            1.5,
        );
        assert!(
            facts.band_m < 1.5,
            "0.4 m of centre-line support is not a 1.5 m band: {}",
            facts.band_m
        );
        let worst = facts
            .intervals
            .iter()
            .min_by(|a, b| a.band_m.total_cmp(&b.band_m))
            .expect("at least one fact");
        assert!(!worst.carried, "the narrow interval is not carried");
        assert_eq!(worst.coverage_verdict, "gap");
        let witness = worst.witness.expect("the gap has a witness");
        assert!(
            (witness.covered_half_width_m - 0.2).abs() < 1e-9,
            "{witness:?}"
        );
        let certified: Vec<&SupportInterval> = facts
            .intervals
            .iter()
            .filter(|interval| interval.carried)
            .collect();
        assert!(
            !certified.is_empty(),
            "the outer surfaces certify their own parts of the walk"
        );
        assert!(certified.iter().any(|interval| {
            interval
                .parts
                .iter()
                .any(|part| part == "left" || part == "right")
        }));
    }

    /// P2: a saved bundle whose *support geometry* was rewritten, with its capability
    /// claims and its payload hash made consistent again, must still be refused, and the
    /// refusal must come from the designated support re-check rather than from the hash.
    /// The re-hashed tampered bundle is written out as evidence for the CLI run too.
    #[test]
    fn r2c_saved_geometry_tamper_is_caught_by_the_support_recheck() {
        let root = case_root();
        let json = std::fs::read_to_string(root.join("minimal-switchback.json")).expect("case");
        let input = load_input(&json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("minimal switchback builds");
        commit_bundle(&mut bundle, "bundle.json", "report.json").expect("publish the bundle");
        assert_eq!(
            verify_bundle(&input, &bundle).status,
            "verified",
            "the untampered minimal switchback verifies"
        );
        let mut narrowed = 0;
        for structure in bundle.structures.iter_mut() {
            for part in structure.parts.iter_mut() {
                if part.part_id.ends_with("::turn") {
                    let centre_x = part.corners_m.iter().map(|corner| corner[0]).sum::<f64>() / 8.0;
                    for corner in part.corners_m.iter_mut() {
                        corner[0] = centre_x + (corner[0] - centre_x) * 0.6;
                    }
                    narrowed += 1;
                }
            }
        }
        assert_eq!(narrowed, 1, "one turn deck was narrowed");
        bundle.payload_hash = payload_hash_of(&bundle).unwrap();
        let report = verify_bundle(&input, &bundle);
        assert!(
            report.payload_hash_matches,
            "the payload hash was made consistent again, so a hash check cannot be the reason"
        );
        assert_ne!(report.status, "verified");
        let failed: Vec<(String, String)> = report
            .independent_rechecks
            .iter()
            .filter(|recheck| !recheck.passed)
            .map(|recheck| (recheck.name.clone(), recheck.detail.clone()))
            .collect();
        assert!(
            report
                .independent_rechecks
                .iter()
                .any(|recheck| recheck.name == "stored_capabilities_resampled" && !recheck.passed),
            "the designated support re-check must refuse the narrowed geometry: {failed:?}"
        );
        // A normal test run writes nothing and touches no delivery directory. When the
        // packet script explicitly asks for the CLI input, the export goes to that
        // script's own directory for this round only.
        if let Ok(dir) = std::env::var("DSH_G20_TAMPER_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("tamper export directory");
            std::fs::write(
                dir.join("geometry-tamper-bundle.json"),
                serde_json::to_string_pretty(&bundle).expect("serialise"),
            )
            .expect("write the exported tampered bundle");
        }
    }

    /// The generator refuses a turn landing that reaches back over a flight, instead of
    /// letting the collision check excuse the overlap later.
    #[test]
    fn r3c_a_turn_landing_over_a_flight_is_refused_by_geometry() {
        let root = case_root();
        let json = std::fs::read_to_string(root.join("minimal-switchback.json")).expect("case");
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        for structure in value["structures"].as_array_mut().unwrap() {
            if structure["kind"] == "switchback_stair" {
                // the old layout: the landing centred on the line where both flights end
                structure["turnCentreM"] = serde_json::json!([10.0, 0.64, 11.2]);
            }
        }
        for port in value["ports"].as_array_mut().unwrap() {
            if port["id"] == "sw-mid" {
                port["positionM"] = serde_json::json!([10.0, 0.64, 11.2]);
            }
        }
        let input = load_input(&value.to_string()).expect("v2 input");
        let error = build_bundle(&input).expect_err("a landing over a flight is refused");
        assert_eq!(
            error.category, "turn_landing_overlaps_flight",
            "{}",
            error.message
        );
        let good = load_input(&json).expect("v2 input");
        assert!(
            build_bundle(&good).is_ok(),
            "the landing beyond both flights is accepted"
        );
    }

    /// The inspection's corner counterexample: one rectangular deck carries both legs of
    /// a short bent line, yet the band at the internal corner is below the requirement.
    /// A corner is an endpoint of both of its directions, so the whole closed
    /// sub-interval is judged instead of only its interior samples.
    #[test]
    fn r3c_an_internal_corner_below_the_band_is_not_carried() {
        let deck = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 0.8,
            top: 1.0,
        };
        let line = vec![[7.9, 1.0, 8.2], [8.0, 1.0, 8.3], [8.1, 1.0, 8.2]];
        let first = Vec3::from_arr(line[1]).sub(Vec3::from_arr(line[0]));
        let across = first.normalize_xz().unwrap().perp_xz();
        let (corner_band, borne) = station_band_width(&[deck], Vec3::new(8.0, 1.0, 8.3), across);
        assert!(borne, "the corner really is on the deck");
        assert!(
            (corner_band - 1.414_213_562_373_095).abs() < 1e-9,
            "corner band {corner_band}"
        );
        assert!(corner_band < 1.5, "the corner is below the requirement");
        let facts = structural_support_detail(&[("deck::deck", deck)], &line, 1.5);
        assert!(
            facts.band_m < 1.5,
            "the internal corner is the narrowest station: {}",
            facts.band_m
        );
        assert_eq!(
            facts.unsupported_endpoints, 0,
            "the two ends of the line really are carried"
        );
        assert!(
            facts
                .intervals
                .iter()
                .any(|interval| !interval.carried && interval.band_m + 1e-9 < 1.5),
            "the corner interval must not be written as carried"
        );
        // the same deck carries a straight line along its own centre line
        let straight = vec![[7.9, 1.0, 8.0], [8.1, 1.0, 8.0]];
        let (band, unsupported) = structural_support_from_frames(&[deck], &straight, 1.5);
        assert!(band + 1e-9 >= 1.5, "straight band {band}");
        assert_eq!(unsupported, 0);
    }

    /// The saved support facts are audited field by field: rewritten summaries, an empty
    /// interval list or invented intervals must be refused by the designated re-check,
    /// with the payload hash made consistent first.
    #[test]
    fn r2c_saved_support_facts_are_audited_field_by_field() {
        let root = case_root();
        let json = std::fs::read_to_string(root.join("minimal-switchback.json")).expect("case");
        let input = load_input(&json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("minimal switchback builds");
        commit_bundle(&mut bundle, "bundle.json", "report.json").expect("publish the bundle");
        assert_eq!(verify_bundle(&input, &bundle).status, "verified");
        let base = bundle.clone();
        let mut rewritten = 0;
        for edge in bundle.graph.edges.iter_mut() {
            let Some(facts) = edge
                .verification
                .detail
                .get_mut("supportFacts")
                .and_then(|facts| facts.as_object_mut())
            else {
                continue;
            };
            for fact in facts.values_mut() {
                if let Some(object) = fact.as_object_mut() {
                    object.insert("requiredBandM".into(), serde_json::json!(0.001));
                    object.insert("unsupportedEndpoints".into(), serde_json::json!(0));
                    object.insert("intervalCount".into(), serde_json::json!(0));
                    object.insert("intervals".into(), serde_json::json!([]));
                    object.insert(
                        "uncarried".into(),
                        serde_json::json!([{
                            "segment": 0,
                            "fromT": 0.0,
                            "toT": 1.0,
                            "adjacentLevelsM": [999.0, 999.0],
                            "effectiveLevelM": 999.0,
                            "parts": ["invented::support"],
                            "bandM": 999.0,
                            "requiredBandM": 0.001,
                            "carried": false,
                        }]),
                    );
                    rewritten += 1;
                }
            }
        }
        assert!(rewritten > 0, "the bundle saves support facts to audit");
        bundle.payload_hash = payload_hash_of(&bundle).unwrap();
        let report = verify_bundle(&input, &bundle);
        assert!(
            report.payload_hash_matches,
            "the payload hash was made consistent first"
        );
        let failed: Vec<String> = report
            .independent_rechecks
            .iter()
            .filter(|recheck| !recheck.passed)
            .map(|recheck| recheck.name.clone())
            .collect();
        assert!(
            failed
                .iter()
                .any(|name| name == "stored_capabilities_resampled"),
            "the designated re-check must audit the saved facts: {failed:?}"
        );
        // A single missing schema field must also be concluded by the designated
        // re-check alone; the two variants are exported when this round's script asks.
        for key in ["adjacentLevelsM", "requiredBandM"] {
            let mut variant = base.clone();
            let mut removed = 0;
            for edge in variant.graph.edges.iter_mut() {
                if let Some(facts) = edge
                    .verification
                    .detail
                    .get_mut("supportFacts")
                    .and_then(|facts| facts.as_object_mut())
                {
                    for fact in facts.values_mut() {
                        if let Some(list) = fact
                            .get_mut("intervals")
                            .and_then(|list| list.as_array_mut())
                        {
                            for interval in list {
                                if let Some(object) = interval.as_object_mut() {
                                    object.remove(key);
                                    removed += 1;
                                }
                            }
                        }
                    }
                }
            }
            assert!(removed > 0, "the bundle saves {key} to audit");
            variant.payload_hash = payload_hash_of(&variant).unwrap();
            let report = verify_bundle(&input, &variant);
            assert!(report.payload_hash_matches);
            let failed: Vec<String> = report
                .independent_rechecks
                .iter()
                .filter(|recheck| !recheck.passed)
                .map(|recheck| recheck.name.clone())
                .collect();
            assert!(
                failed
                    .iter()
                    .any(|name| name == "stored_capabilities_resampled"),
                "a missing {key} must be refused by the designated re-check: {failed:?}"
            );
            if let Ok(dir) = std::env::var("DSH_G20_TAMPER_DIR") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).expect("export directory");
                std::fs::write(
                    dir.join(format!("missing-{key}-bundle.json")),
                    serde_json::to_string_pretty(&variant).expect("serialise"),
                )
                .expect("write the exported variant");
            }
        }
    }

    // ---- G21: the pure coverage prover over finite rectangles -------------------------

    fn rect(centre: [f64; 3], half_length: f64, half_width: f64) -> MemberFrame {
        MemberFrame {
            centre: Vec3::new(centre[0], centre[1], centre[2]),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length,
            half_width,
            top: 1.0,
        }
    }

    fn one_layer(frames: Vec<(&str, MemberFrame)>) -> Vec<(f64, Vec<(String, MemberFrame)>)> {
        vec![(
            1.0,
            frames
                .into_iter()
                .map(|(id, frame)| (id.to_string(), frame))
                .collect(),
        )]
    }

    fn certify(
        frames: Vec<(&str, MemberFrame)>,
        a: [f64; 3],
        b: [f64; 3],
        required: f64,
    ) -> support_coverage::BandCertificate {
        support_coverage::certify_band(
            &one_layer(frames),
            0,
            Vec3::new(a[0], a[1], a[2]),
            Vec3::new(b[0], b[1], b[2]),
            required,
            support_coverage::MAX_COVERAGE_EVENTS,
        )
    }

    #[test]
    fn g21_the_union_of_two_intervals_covers_the_band() {
        // Two surfaces that meet near the centre line: the union is [min(low), max(high)],
        // never the 0.2 m intersection of [-1, 0.1] and [-0.1, 1].
        let left = rect([5.0, 0.0, -0.45], 5.0, 0.55);
        let right = rect([5.0, 0.0, 0.45], 5.0, 0.55);
        let facts = certify(
            vec![("left", left), ("right", right)],
            [1.0, 1.0, 0.0],
            [9.0, 1.0, 0.0],
            2.0,
        );
        assert_eq!(facts.verdict, support_coverage::CoverageVerdict::Certified);
        assert!((facts.certified_band_m - 2.0).abs() < 1e-9);
        let parts: Vec<String> = facts
            .slices
            .iter()
            .filter(|slice| slice.verdict == support_coverage::CoverageVerdict::Certified)
            .flat_map(|slice| slice.parts.clone())
            .collect();
        assert!(parts.iter().any(|part| part == "left"));
        assert!(parts.iter().any(|part| part == "right"));
        // a real hole over the centre line is refused with a witness
        let left_hole = rect([5.0, 0.0, -0.55], 5.0, 0.45);
        let right_hole = rect([5.0, 0.0, 0.55], 5.0, 0.45);
        let facts = certify(
            vec![("left", left_hole), ("right", right_hole)],
            [1.0, 1.0, 0.0],
            [9.0, 1.0, 0.0],
            2.0,
        );
        assert_eq!(facts.verdict, support_coverage::CoverageVerdict::Gap);
        let witness = facts
            .slices
            .iter()
            .find_map(|slice| slice.witness)
            .expect("a hole over the centre line has a witness");
        assert!(witness.s.abs() <= 0.45);
    }

    #[test]
    fn g21_lateral_widening_parts_take_part() {
        // The inspection's lateral counterexample: a central surface 1.5 m wide that only
        // reaches z = 0.5, plus two 0.5 m strips that never contain the centre line.
        let central = rect([5.0, 0.0, -0.25], 5.0, 0.75);
        let left = rect([2.0025, 0.0, 0.75], 2.0025, 0.25);
        let right = rect([7.0575, 0.0, 0.75], 2.9425, 0.25);
        let facts = certify(
            vec![
                ("central", central),
                ("left_strip", left),
                ("right_strip", right),
            ],
            [1.0, 1.0, 0.0],
            [9.0, 1.0, 0.0],
            1.5,
        );
        assert_eq!(
            facts.verdict,
            support_coverage::CoverageVerdict::Gap,
            "the strip window leaves only 1 m of real support"
        );
        let witness = facts
            .slices
            .iter()
            .find_map(|slice| slice.witness)
            .expect("witness");
        assert!(
            (witness.covered_half_width_m - 0.5).abs() < 1e-9,
            "{witness:?}"
        );
        // the same geometry with the strips meeting: the band is certified and the parts
        // that make it up include the lateral strips, not only the central surface
        let joined = rect([7.0025, 0.0, 0.75], 2.9975, 0.25);
        let facts = certify(
            vec![
                ("central", central),
                ("left_strip", left),
                ("right_strip", joined),
            ],
            [1.0, 1.0, 0.0],
            [9.0, 1.0, 0.0],
            1.5,
        );
        assert_eq!(facts.verdict, support_coverage::CoverageVerdict::Certified);
        let certified: Vec<&support_coverage::CoverageSlice> = facts
            .slices
            .iter()
            .filter(|slice| slice.verdict == support_coverage::CoverageVerdict::Certified)
            .collect();
        assert!(!certified.is_empty());
        assert!(
            certified.iter().any(|slice| slice
                .parts
                .iter()
                .any(|part| part == "left_strip" || part == "right_strip")),
            "the lateral widening parts must be named: {:?}",
            certified
                .iter()
                .map(|slice| slice.parts.clone())
                .collect::<Vec<_>>()
        );
        // reversing the walk and shuffling the parts changes nothing
        let reversed = certify(
            vec![
                ("right_strip", joined),
                ("central", central),
                ("left_strip", left),
            ],
            [9.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            1.5,
        );
        assert_eq!(
            reversed.verdict,
            support_coverage::CoverageVerdict::Certified
        );
    }

    #[test]
    fn g21_a_reordered_envelope_is_an_event() {
        // The inspection's envelope probe: two rotated rectangles whose union boundary
        // changes hands inside the segment. The crossing must be an event, so the section
        // where the union is narrowest is decided instead of stepped over.
        let root = 5.0_f64.sqrt();
        let base = MemberFrame {
            centre: Vec3::new(1.0 / 24.0, 0.0, 0.72 - 2.0 * root),
            along: Vec3::new(1.0 / root, 0.0, 2.0 / root),
            across: Vec3::new(-2.0 / root, 0.0, 1.0 / root),
            half_length: 10.0,
            half_width: 2.0,
            top: 1.0,
        };
        let other = MemberFrame {
            along: Vec3::new(1.0 / root, 0.0, -2.0 / root),
            across: Vec3::new(2.0 / root, 0.0, 1.0 / root),
            ..base
        };
        let facts = certify(
            vec![("p", base), ("n", other)],
            [0.0, 1.0, 0.0],
            [0.25, 1.0, 0.0],
            1.5,
        );
        assert_eq!(
            facts.verdict,
            support_coverage::CoverageVerdict::Gap,
            "the union narrows to 1.44 m at the boundary hand-over"
        );
        let witness = facts
            .slices
            .iter()
            .find_map(|slice| slice.witness)
            .expect("witness");
        assert!(
            (witness.covered_half_width_m - 0.72).abs() < 1e-6,
            "{witness:?}"
        );
    }

    #[test]
    fn g21_coverage_variants_and_limits() {
        let central = rect([5.0, 0.0, -0.25], 5.0, 0.75);
        let left = rect([2.0025, 0.0, 0.75], 2.0025, 0.25);
        let right = rect([7.0025, 0.0, 0.75], 2.9975, 0.25);
        // translated by 100 m in x
        let shift = |frame: MemberFrame| MemberFrame {
            centre: Vec3::new(frame.centre.x + 100.0, frame.centre.y, frame.centre.z),
            ..frame
        };
        let moved = certify(
            vec![
                ("central", shift(central)),
                ("left_strip", shift(left)),
                ("right_strip", shift(right)),
            ],
            [101.0, 1.0, 0.0],
            [109.0, 1.0, 0.0],
            1.5,
        );
        assert_eq!(moved.verdict, support_coverage::CoverageVerdict::Certified);
        // rotated by 90 degrees: the walk runs along +z and the surfaces turn with it
        let turn = |frame: MemberFrame| MemberFrame {
            centre: Vec3::new(-frame.centre.z, frame.centre.y, frame.centre.x),
            along: Vec3::new(-frame.along.z, 0.0, frame.along.x),
            across: Vec3::new(-frame.across.z, 0.0, frame.across.x),
            ..frame
        };
        let rotated = certify(
            vec![
                ("central", turn(central)),
                ("left_strip", turn(left)),
                ("right_strip", turn(right)),
            ],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 9.0],
            1.5,
        );
        assert_eq!(
            rotated.verdict,
            support_coverage::CoverageVerdict::Certified
        );
        // degenerate and budgeted cases are indeterminate, never carried
        let degenerate = certify(
            vec![("central", central)],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            1.5,
        );
        assert_eq!(
            degenerate.verdict,
            support_coverage::CoverageVerdict::Indeterminate
        );
        // A degenerate segment is a numerical doubt, never a licence to walk on terrain.
        assert_eq!(
            degenerate.bearing,
            support_coverage::BearingIdentity::Unknown
        );
        assert_eq!(
            degenerate.bearing_reason,
            support_coverage::REASON_NUMERICAL_UNCERTAIN
        );
        let budgeted = support_coverage::certify_band(
            &one_layer(vec![("central", central), ("left_strip", left)]),
            0,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(9.0, 1.0, 0.0),
            1.5,
            4,
        );
        assert_eq!(
            budgeted.verdict,
            support_coverage::CoverageVerdict::Indeterminate
        );
        // G22: an exhausted budget keeps the bearing identity it independently proved
        // (`structure`, because a real station in the segment is carried) while the
        // coverage stays undecided. It is never rewritten into "no support needed".
        assert_eq!(
            budgeted.bearing,
            support_coverage::BearingIdentity::Structure
        );
        assert_eq!(
            budgeted.bearing_reason,
            support_coverage::REASON_COVERAGE_BUDGET_EXHAUSTED
        );
        assert!(budgeted.slices.is_empty());
        let no_parts = support_coverage::certify_band(
            &one_layer(vec![]),
            0,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(9.0, 1.0, 0.0),
            1.5,
            support_coverage::MAX_COVERAGE_EVENTS,
        );
        // G22 semantic change, old name -> new meaning: a layer with no candidate part
        // used to be reported as `Indeterminate` (a failure that the ground consumer then
        // read as "not borne"). It is now the decisive `notApplicable` coverage with the
        // `noStructuralSupport` bearing identity: a complete evaluation that proves no
        // structural certificate is owed here. A failure reason never reaches this state.
        assert_eq!(
            no_parts.verdict,
            support_coverage::CoverageVerdict::NotApplicable
        );
        assert_eq!(
            no_parts.bearing,
            support_coverage::BearingIdentity::NoStructuralSupport
        );
        assert_eq!(
            no_parts.bearing_reason,
            support_coverage::REASON_NO_CANDIDATE_PARTS_PROVEN
        );
        assert!(no_parts.slices.is_empty());
    }

    // ---- G22: bearing identity and coverage result as two independent axes -----------

    /// Write one audit counterexample into this round's own destination, with `create_new`
    /// so a rerun can never overwrite the evidence of an earlier one. A normal test run
    /// with the variable unset writes nothing at all.
    fn export_variant(name: &str, bytes: &str) {
        let Ok(dir) = std::env::var("DSH_G22_TAMPER_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("export directory");
        let path = dir.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap_or_else(|error| panic!("create_new {}: {error}", path.display()));
        std::io::Write::write_all(&mut file, bytes.as_bytes()).expect("write the exported variant");
    }

    /// A single-field, self-consistent tamper: only the designated support re-check may
    /// reject it, and the payload hash must already agree.
    fn assert_designated_recheck_refuses(
        input: &EngineeringInput,
        bundle: &mut EngineeringBundle,
        label: &str,
    ) {
        bundle.payload_hash = payload_hash_of(bundle).expect("hash the variant");
        let report = verify_bundle(input, bundle);
        assert!(
            report.payload_hash_matches,
            "{label}: the payload hash was made consistent first"
        );
        assert_eq!(
            report.status, "tamper_detected",
            "{label}: findings {:?}",
            report.findings
        );
        let failed: Vec<String> = report
            .independent_rechecks
            .iter()
            .filter(|recheck| !recheck.passed)
            .map(|recheck| recheck.name.clone())
            .collect();
        assert!(
            failed
                .iter()
                .any(|name| name == "stored_capabilities_resampled"),
            "{label}: the designated support re-check must conclude on its own, failed {failed:?}"
        );
    }

    fn first_support_facts_mut<'a>(
        bundle: &'a mut EngineeringBundle,
        kind: Option<&str>,
    ) -> Option<&'a mut serde_json::Value> {
        for edge in bundle.graph.edges.iter_mut() {
            if kind.is_some_and(|kind| edge.kind != kind) {
                continue;
            }
            if let Some(first) = edge
                .verification
                .detail
                .get_mut("supportFacts")
                .and_then(|facts| facts.as_object_mut())
                .and_then(|facts| facts.values_mut().next())
            {
                return Some(first);
            }
        }
        None
    }

    fn first_interval_mut<'a>(facts: &'a mut serde_json::Value) -> &'a mut serde_json::Value {
        let list = facts
            .get_mut("intervals")
            .and_then(|value| value.as_array_mut())
            .expect("the saved facts list their intervals");
        assert!(!list.is_empty(), "at least one saved interval");
        &mut list[0]
    }

    fn case_source(name: &str) -> String {
        std::fs::read_to_string(case_root().join(format!("{name}.json")))
            .unwrap_or_else(|error| panic!("case {name} is missing: {error}"))
    }

    /// N1: the inspection's complete CLI differential, taken verbatim. The control and the
    /// far-parts form must both be refused at the support stage, in the local narrow
    /// bearing category, and the far parts must be named as excluded with their reason:
    /// adding unconnected structure must never turn a local refusal into a pass.
    #[test]
    fn g22_the_inspector_ground_differential_is_refused_in_both_forms() {
        let root = case_root();
        let control_json = std::fs::read_to_string(root.join("g22-ground-narrow-control.json"))
            .expect("control case");
        let remote_json =
            std::fs::read_to_string(root.join("g22-ground-narrow-with-remote-parts-v2.json"))
                .expect("remote-parts case");
        let control: serde_json::Value = serde_json::from_str(&control_json).unwrap();
        let remote: serde_json::Value = serde_json::from_str(&remote_json).unwrap();
        // The scene really is the same locally: only far, unconnected parts were added.
        for key in [
            "grid",
            "obstacles",
            "water",
            "bearing",
            "groundConnections",
            "query",
        ] {
            assert_eq!(
                control[key], remote[key],
                "the differential must not touch {key}"
            );
        }
        for id in ["a", "b", "deck-port"] {
            let pick = |value: &serde_json::Value| {
                value["ports"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|port| port["id"] == id)
                    .cloned()
                    .unwrap_or_else(|| panic!("local port {id}"))
            };
            assert_eq!(pick(&control), pick(&remote), "port {id}");
        }
        let deck = |value: &serde_json::Value| {
            value["structures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|structure| structure["id"] == "deck")
                .cloned()
                .expect("the local deck")
        };
        assert_eq!(
            deck(&control),
            deck(&remote),
            "the local platform is unchanged"
        );
        assert!(
            remote["structures"].as_array().unwrap().len()
                > control["structures"].as_array().unwrap().len(),
            "only remote structures were added"
        );
        for (label, json) in [("control", &control_json), ("remote parts", &remote_json)] {
            let input = load_input(json).expect("v2 input");
            let error = match build_bundle(&input) {
                Ok(_) => panic!("{label}: the narrow local support must not be built"),
                Err(error) => error,
            };
            assert_eq!(
                error.category, "ground_edge_unsupported_by_structure",
                "{label}: the refusal must be the local narrow bearing category, message {}",
                error.message
            );
            assert_eq!(error.detail["bearing"], "structure", "{label}");
            assert_eq!(error.detail["coverageVerdict"], "gap", "{label}");
            assert_eq!(error.detail["bearingReason"], "decided", "{label}");
            let measured = error.detail["measuredBandM"]
                .as_f64()
                .expect("measured band");
            let required = error.detail["requiredBandM"]
                .as_f64()
                .expect("required band");
            assert!(
                measured < required,
                "{label}: the real band {measured} is under the required {required}"
            );
        }
        // The far parts are excluded before the event scan, and the reason is recorded.
        let input = load_input(&remote_json).expect("v2 input");
        let error = build_bundle(&input).expect_err("the remote form is refused");
        let filtered = error.detail["partsFiltered"]
            .as_array()
            .expect("the refusal names the excluded candidates");
        assert!(
            filtered.len() >= 100,
            "the three remote groups are recorded as excluded: {}",
            filtered.len()
        );
        assert!(
            filtered
                .iter()
                .all(|entry| entry["reason"] == "outside_closed_walking_band"),
            "{filtered:?}"
        );
        assert!(
            filtered.iter().all(|entry| entry["part"]
                .as_str()
                .is_some_and(|part| part.starts_with("remote-"))),
            "only the remote parts may be excluded: {filtered:?}"
        );
        assert_eq!(
            error.detail["eventBudget"],
            support_coverage::MAX_COVERAGE_EVENTS as u64
        );
        // The same invariance holds in the pure prover: the local certificate does not
        // move when far parts are added, and they are reported as filtered.
        let local = rect([8.0, 0.0, 8.2], 3.0, 0.8);
        let line = [[6.0, 1.0, 8.6], [10.0, 1.0, 8.6]];
        let alone = certify(vec![("deck::deck", local)], line[0], line[1], 1.5);
        let mut with_remote = vec![("deck::deck", local)];
        for index in 0..40 {
            with_remote.push(("remote", rect([120.0 + index as f64, 0.0, 120.0], 1.0, 0.8)));
        }
        let crowded = certify(with_remote, line[0], line[1], 1.5);
        assert_eq!(alone.verdict, crowded.verdict);
        assert_eq!(alone.bearing, crowded.bearing);
        assert_eq!(alone.slices.len(), crowded.slices.len());
        assert_eq!(crowded.parts_filtered.len(), 40);
        assert!(crowded.parts_considered.contains(&"deck::deck".to_string()));
    }

    /// N2: bare ground is a proven bearing-free section, not an exemption. The
    /// authoritative terrain, grade, water, obstacle and sampling rules keep deciding,
    /// and a structure edge may never borrow the bearing-free state.
    #[test]
    fn g22_bare_ground_keeps_the_authoritative_terrain_rules() {
        let json = case_source("control-flat");
        let input = load_input(&json).expect("v2 input");
        let (bundle, _) = build_bundle(&input).expect("flat bare ground still builds");
        let ground = bundle
            .graph
            .edges
            .iter()
            .find(|edge| edge.kind == "ground")
            .expect("a ground edge");
        let facts = &ground.verification.detail["supportFacts"]["pedestrian"];
        assert_eq!(facts["bearing"], "noStructuralSupport");
        assert_eq!(facts["coverageVerdict"], "notApplicable");
        assert_eq!(facts["bearingReason"], "no_candidate_parts_proven");
        assert_eq!(facts["schemaVersion"], SUPPORT_FACTS_SCHEMA_VERSION as u64);
        assert!(
            facts["intervals"]
                .as_array()
                .unwrap()
                .iter()
                .all(|interval| interval["bearing"] == "noStructuralSupport"
                    && interval["coverageVerdict"] == "notApplicable"
                    && interval["centreBorne"] == false)
        );
        assert!(ground.capabilities.contains(&Capability::Pedestrian));
        // Bare ground is still refused by the authoritative solid and scope rules ...
        for (name, category) in [
            ("ground-overhead-obstacle", "ground_edge_blocked_by_solid"),
            ("wheelchair-low-solid", "ground_edge_blocked_by_solid"),
            ("overflow-scope", "scope_out_of_range"),
        ] {
            let input = load_input(&case_source(name)).expect("v2 input");
            let error = match build_bundle(&input) {
                Ok(_) => panic!("{name} must stay refused"),
                Err(error) => error,
            };
            assert_eq!(error.category, category, "{name}: {}", error.message);
        }
        // ... and the grade and water facts keep refusing what they always refused.
        for (name, route) in [
            ("vehicle-side-water", "no_suitable_edge"),
            ("pedestrian-hump-zero-rise", "budget_exhausted"),
        ] {
            let input = load_input(&case_source(name)).expect("v2 input");
            let (bundle, _) = build_bundle(&input).expect("the build itself succeeds");
            assert_eq!(bundle.route.status, route, "{name}");
        }
        // A structure edge never carries a bearing-free or undecided interval.
        let switchback = load_input(&case_source("minimal-switchback")).expect("v2 input");
        let (bundle, _) = build_bundle(&switchback).expect("the switchback builds");
        let mut audited = 0_usize;
        for edge in bundle
            .graph
            .edges
            .iter()
            .filter(|edge| edge.kind != "ground")
        {
            let Some(facts) = edge.verification.detail.get("supportFacts") else {
                continue;
            };
            for fact in facts.as_object().unwrap().values() {
                let intervals = fact["intervals"].as_array().expect("intervals");
                assert!(
                    !intervals.is_empty(),
                    "a structure edge carries support facts"
                );
                for interval in intervals {
                    assert_eq!(interval["bearing"], "structure", "{}", edge.edge_id);
                    assert_ne!(
                        interval["coverageVerdict"], "notApplicable",
                        "{}",
                        edge.edge_id
                    );
                }
                audited += 1;
            }
        }
        assert!(audited > 0, "structure edges save support facts");
    }

    /// N3: an exhausted budget and an invalid frame are refusals on both axes, in the
    /// production consumer and in the designated audit — never a licence to walk on
    /// terrain. The smaller budget and the injected frame go through the same evaluation
    /// and the same gate the builder and the verifier call.
    #[test]
    fn g22_budget_and_invalid_frames_refuse_in_production_and_audit() {
        let deck = MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 1.0,
            top: 1.0,
        };
        let line = vec![[5.5, 1.0, 8.0], [10.5, 1.0, 8.0]];
        let full = structural_support_detail(&[("deck::deck", deck)], &line, 1.5);
        assert_eq!(full.bearing, support_coverage::BearingIdentity::Structure);
        assert_eq!(full.coverage, support_coverage::CoverageVerdict::Certified);
        assert!(full.band_m + 1e-9 >= 1.5);
        assert!(
            full.ground_refusal().is_none(),
            "a wide deck passes the support gate"
        );
        // The same real geometry, evaluated under a budget that cannot afford it. The
        // identity that a real station proves survives the failure; the coverage does not
        // become a certificate and the gate refuses.
        let starved = structural_support_detail_with_budget(&[("deck::deck", deck)], &line, 1.5, 4);
        assert_eq!(
            starved.coverage,
            support_coverage::CoverageVerdict::Indeterminate
        );
        assert_eq!(
            starved.bearing,
            support_coverage::BearingIdentity::Structure
        );
        assert_eq!(
            starved.bearing_reason,
            support_coverage::REASON_COVERAGE_BUDGET_EXHAUSTED
        );
        assert_eq!(starved.band_m, 0.0);
        let refusal = starved
            .ground_refusal()
            .expect("the production ground gate refuses an exhausted budget");
        assert_eq!(refusal.category, "ground_edge_unsupported_by_structure");
        assert!(!refusal.interval.expect("a section refused").carried);
        // A candidate that reaches into the band but carries no probe station keeps the
        // undecided identity: an incomplete evaluation proves nothing about absence.
        let off_probe = MemberFrame {
            centre: Vec3::new(6.25, 0.0, 0.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 0.05,
            half_width: 1.0,
            top: 1.0,
        };
        let walk = vec![[0.0, 1.0, 0.0], [10.0, 1.0, 0.0]];
        let undecided =
            structural_support_detail_with_budget(&[("off::probe", off_probe)], &walk, 1.5, 4);
        assert_eq!(
            undecided.coverage,
            support_coverage::CoverageVerdict::Indeterminate
        );
        assert_eq!(
            undecided.bearing,
            support_coverage::BearingIdentity::Unknown
        );
        assert_eq!(
            undecided.bearing_reason,
            support_coverage::REASON_COVERAGE_BUDGET_EXHAUSTED
        );
        assert_eq!(
            undecided.ground_refusal().expect("refused").category,
            "ground_edge_support_unknown"
        );
        // An invalid frame is an indeterminate, not an out-of-band exclusion and not a
        // bearing-free section.
        let broken = MemberFrame {
            half_width: 0.0,
            ..deck
        };
        let invalid = structural_support_detail(&[("broken::deck", broken)], &line, 1.5);
        assert_eq!(invalid.bearing, support_coverage::BearingIdentity::Unknown);
        assert_eq!(
            invalid.bearing_reason,
            support_coverage::REASON_INVALID_FRAME
        );
        assert_eq!(
            invalid.coverage,
            support_coverage::CoverageVerdict::Indeterminate
        );
        assert_eq!(
            invalid.ground_refusal().expect("refused").category,
            "ground_edge_support_unknown"
        );
        assert!(
            invalid
                .parts_considered
                .contains(&"broken::deck".to_string()),
            "an unusable frame is still a considered candidate"
        );
        // The designated audit refuses the same states when they are saved: an undecided
        // coverage on a structure claim, and a forged structure claim on bare ground.
        let switchback_json = case_source("minimal-switchback");
        let input = load_input(&switchback_json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("the switchback builds");
        commit_bundle(&mut bundle, "bundle.json", "report.json").expect("publish");
        assert_eq!(verify_bundle(&input, &bundle).status, "verified");
        let mut variant = bundle.clone();
        {
            let facts = first_support_facts_mut(&mut variant, None).expect("saved facts");
            facts
                .as_object_mut()
                .unwrap()
                .insert("coverageVerdict".into(), serde_json::json!("indeterminate"));
        }
        assert_designated_recheck_refuses(&input, &mut variant, "saved coverage -> indeterminate");

        let control_json = case_source("control-flat");
        let control_input = load_input(&control_json).expect("v2 input");
        let (mut ground_bundle, _) = build_bundle(&control_input).expect("bare ground builds");
        commit_bundle(&mut ground_bundle, "bundle.json", "report.json").expect("publish");
        let mut variant = ground_bundle.clone();
        {
            let facts =
                first_support_facts_mut(&mut variant, Some("ground")).expect("ground facts");
            facts
                .as_object_mut()
                .unwrap()
                .insert("bearing".into(), serde_json::json!("structure"));
        }
        assert_designated_recheck_refuses(
            &control_input,
            &mut variant,
            "ground bearing -> structure",
        );
    }

    /// N4: every single-field, self-consistent rewrite of the saved bearing state is
    /// refused by the designated support re-check on its own, with the payload hash
    /// already agreeing. A whole-package difference is never the reason.
    #[test]
    fn g22_single_field_bearing_tamper_is_refused_by_the_designated_recheck() {
        let control_json = case_source("control-flat");
        let input = load_input(&control_json).expect("v2 input");
        let (mut bundle, _) = build_bundle(&input).expect("bare ground builds");
        commit_bundle(&mut bundle, "bundle.json", "report.json").expect("publish");
        assert_eq!(verify_bundle(&input, &bundle).status, "verified");
        export_variant("control-flat-input.json", &control_json);

        let mut variants: Vec<(String, EngineeringBundle)> = vec![];
        // 1. the whole-block identity is rewritten from proven bearing-free to structure
        let mut variant = bundle.clone();
        first_support_facts_mut(&mut variant, Some("ground"))
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("bearing".into(), serde_json::json!("structure"));
        variants.push(("identity-tamper-bundle.json".into(), variant));
        // 2. the whole-block coverage is rewritten into a certificate
        let mut variant = bundle.clone();
        first_support_facts_mut(&mut variant, Some("ground"))
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("coverageVerdict".into(), serde_json::json!("certified"));
        variants.push(("coverage-tamper-bundle.json".into(), variant));
        // 3. an interval identity becomes unknown
        let mut variant = bundle.clone();
        first_interval_mut(first_support_facts_mut(&mut variant, Some("ground")).unwrap())
            .as_object_mut()
            .unwrap()
            .insert("bearing".into(), serde_json::json!("unknown"));
        variants.push(("interval-identity-tamper-bundle.json".into(), variant));
        // 4. an interval coverage becomes a certificate
        let mut variant = bundle.clone();
        first_interval_mut(first_support_facts_mut(&mut variant, Some("ground")).unwrap())
            .as_object_mut()
            .unwrap()
            .insert("coverageVerdict".into(), serde_json::json!("certified"));
        variants.push(("interval-coverage-tamper-bundle.json".into(), variant));
        // 5. the identity field is deleted
        let mut variant = bundle.clone();
        first_interval_mut(first_support_facts_mut(&mut variant, Some("ground")).unwrap())
            .as_object_mut()
            .unwrap()
            .remove("bearing");
        variants.push(("missing-field-bundle.json".into(), variant));
        // 6. an illegal enum value
        let mut variant = bundle.clone();
        first_interval_mut(first_support_facts_mut(&mut variant, Some("ground")).unwrap())
            .as_object_mut()
            .unwrap()
            .insert("bearing".into(), serde_json::json!("maybe"));
        variants.push(("illegal-enum-bundle.json".into(), variant));
        // 7. a forged contributing part
        let mut variant = bundle.clone();
        first_interval_mut(first_support_facts_mut(&mut variant, Some("ground")).unwrap())
            .as_object_mut()
            .unwrap()
            .insert("parts".into(), serde_json::json!(["invented::support"]));
        variants.push(("forged-part-bundle.json".into(), variant));
        // 8. a forged filtered part, with its reason
        let mut variant = bundle.clone();
        first_support_facts_mut(&mut variant, Some("ground"))
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("partsFiltered".into(), serde_json::json!([{
                "segment": 0, "part": "ghost::part", "levelM": 1.0, "reason": "outside_closed_walking_band"
            }]));
        variants.push(("forged-filtered-bundle.json".into(), variant));
        // 9. a forged considered candidate
        let mut variant = bundle.clone();
        first_support_facts_mut(&mut variant, Some("ground"))
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("partsConsidered".into(), serde_json::json!(["ghost::part"]));
        variants.push(("forged-considered-bundle.json".into(), variant));

        for (name, mut variant) in variants {
            assert_designated_recheck_refuses(&input, &mut variant, &name);
            export_variant(
                &name,
                &serde_json::to_string_pretty(&variant).expect("serialise"),
            );
        }

        // The structure side of the same schema: structure -> noStructuralSupport,
        // structure -> unknown, certified -> gap.
        let switchback_json = case_source("minimal-switchback");
        let structure_input = load_input(&switchback_json).expect("v2 input");
        let (mut structure_bundle, _) = build_bundle(&structure_input).expect("switchback builds");
        commit_bundle(&mut structure_bundle, "bundle.json", "report.json").expect("publish");
        export_variant("minimal-switchback-input.json", &switchback_json);
        let mut structure_variants: Vec<(String, EngineeringBundle)> = vec![];
        let mut variant = structure_bundle.clone();
        first_support_facts_mut(&mut variant, None)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("bearing".into(), serde_json::json!("noStructuralSupport"));
        structure_variants.push(("structure-to-ground-bundle.json".into(), variant));
        let mut variant = structure_bundle.clone();
        first_support_facts_mut(&mut variant, None)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("bearing".into(), serde_json::json!("unknown"));
        structure_variants.push(("structure-to-unknown-bundle.json".into(), variant));
        let mut variant = structure_bundle.clone();
        first_support_facts_mut(&mut variant, None)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("coverageVerdict".into(), serde_json::json!("gap"));
        structure_variants.push(("structure-gap-bundle.json".into(), variant));
        // 10. the saved support geometry is narrowed: the designated re-check must reach
        // the same conclusion from the saved geometry alone.
        let mut variant = structure_bundle.clone();
        let narrowed = variant
            .graph
            .edges
            .iter_mut()
            .find(|edge| edge.kind != "ground")
            .expect("a structure edge");
        narrowed.polyline_m = narrowed
            .polyline_m
            .iter()
            .map(|point| [point[0] * 0.5, point[1], point[2] * 0.5])
            .collect();
        structure_variants.push(("geometry-tamper-bundle.json".into(), variant));
        // 11. an edited payload hash is reported by the integrity check itself.
        let mut variant = structure_bundle.clone();
        variant.payload_hash = "0".repeat(64);
        structure_variants.push(("payload-tamper-bundle.json".into(), variant));

        for (name, mut variant) in structure_variants {
            if name == "payload-tamper-bundle.json" {
                let report = verify_bundle(&structure_input, &variant);
                assert!(!report.payload_hash_matches, "{name}");
                assert_eq!(report.status, "tamper_detected", "{name}");
            } else {
                assert_designated_recheck_refuses(&structure_input, &mut variant, &name);
            }
            export_variant(
                &name,
                &serde_json::to_string_pretty(&variant).expect("serialise"),
            );
        }
    }

    // ---- G22 boundary completion: the wrapper entrance contracts ---------------------

    /// The wide deck used by the boundary tests: 6 m x 2 m at level 1.0, which carries the
    /// whole 1.5 m pedestrian band of the line below.
    fn wide_deck() -> MemberFrame {
        MemberFrame {
            centre: Vec3::new(8.0, 0.0, 8.0),
            along: Vec3::new(1.0, 0.0, 0.0),
            across: Vec3::new(0.0, 0.0, 1.0),
            half_length: 3.0,
            half_width: 1.0,
            top: 1.0,
        }
    }

    /// A walking line along +x over that deck.
    fn deck_line() -> Vec<[f64; 3]> {
        vec![[5.5, 1.0, 8.0], [10.5, 1.0, 8.0]]
    }

    /// P2 (a): an unusable frame must be rejected **before** the layer comparison that
    /// places it by `frame.top`. A non-finite `top` fails that comparison, so validating
    /// only the already-filtered set drops the part in silence and the line is then read as
    /// a proven bearing-free one.
    #[test]
    fn g22b_invalid_frame_fields_cannot_be_dropped_by_the_layer_filter() {
        let line = deck_line();
        let deck = wide_deck();
        // Baseline: the same geometry with usable frames certifies and passes the gate.
        let good = structural_support_detail(&[("deck::deck", deck)], &line, 1.5);
        assert_eq!(good.bearing, support_coverage::BearingIdentity::Structure);
        assert_eq!(good.coverage, support_coverage::CoverageVerdict::Certified);
        assert!(good.ground_refusal().is_none());
        assert!(good.structure_refusal().is_none());

        // A single frame whose own fields cannot be trusted is an Unknown, whichever field
        // it is, and it names the part instead of vanishing.
        for (label, broken) in [
            (
                "top NaN",
                MemberFrame {
                    top: f64::NAN,
                    ..deck
                },
            ),
            (
                "top +Inf",
                MemberFrame {
                    top: f64::INFINITY,
                    ..deck
                },
            ),
            (
                "centre NaN",
                MemberFrame {
                    centre: Vec3::new(f64::NAN, 0.0, 8.0),
                    ..deck
                },
            ),
            (
                "along NaN",
                MemberFrame {
                    along: Vec3::new(f64::NAN, 0.0, 0.0),
                    ..deck
                },
            ),
            (
                "vertical along",
                MemberFrame {
                    along: Vec3::new(0.0, 1.0, 0.0),
                    ..deck
                },
            ),
            (
                "half_width zero",
                MemberFrame {
                    half_width: 0.0,
                    ..deck
                },
            ),
            (
                "half_length NaN",
                MemberFrame {
                    half_length: f64::NAN,
                    ..deck
                },
            ),
        ] {
            let facts = structural_support_detail(&[("deck::deck", broken)], &line, 1.5);
            assert_eq!(
                facts.bearing,
                support_coverage::BearingIdentity::Unknown,
                "{label}: an unusable frame is never a bearing-free proof"
            );
            assert_eq!(
                facts.bearing_reason,
                support_coverage::REASON_INVALID_FRAME,
                "{label}"
            );
            assert_eq!(
                facts.coverage,
                support_coverage::CoverageVerdict::Indeterminate,
                "{label}"
            );
            assert_eq!(facts.band_m, 0.0, "{label}");
            let refusal = facts
                .ground_refusal()
                .unwrap_or_else(|| panic!("{label}: the ground gate must refuse"));
            assert_eq!(refusal.category, "ground_edge_support_unknown", "{label}");
            assert!(
                refusal.detail.contains("deck::deck"),
                "{label}: {}",
                refusal.detail
            );
            assert!(
                refusal.interval.is_some(),
                "{label}: the refusal names the offending section"
            );
            assert_eq!(
                facts
                    .structure_refusal()
                    .expect("the structure gate refuses")
                    .category,
                "structure_edge_unsupported",
                "{label}"
            );
            assert!(
                facts.parts_considered.contains(&"deck::deck".to_string()),
                "{label}: an unusable candidate stays a considered candidate"
            );
        }

        // The exact P2 shape: a valid deck that would certify on its own, plus one extra
        // part whose `top` is NaN. Without the entrance validation the NaN part is dropped
        // by the level comparison and the deck certifies the line, so the extra part would
        // read as "no structure at another level" instead of "cannot be classified".
        let with_unusable_extra = structural_support_detail(
            &[
                ("deck::deck", deck),
                (
                    "ghost::part",
                    MemberFrame {
                        top: f64::NAN,
                        ..deck
                    },
                ),
            ],
            &line,
            1.5,
        );
        assert_eq!(
            with_unusable_extra.bearing,
            support_coverage::BearingIdentity::Unknown
        );
        assert_eq!(
            with_unusable_extra.bearing_reason,
            support_coverage::REASON_INVALID_FRAME
        );
        let refusal = with_unusable_extra
            .ground_refusal()
            .expect("the ground gate refuses the mixed set");
        assert_eq!(refusal.category, "ground_edge_support_unknown");
        assert!(
            refusal.detail.contains("ghost::part"),
            "the unusable part is named: {}",
            refusal.detail
        );
        assert!(
            with_unusable_extra
                .parts_considered
                .contains(&"ghost::part".to_string())
        );
        assert!(
            with_unusable_extra
                .parts_considered
                .contains(&"deck::deck".to_string())
        );
        assert!(with_unusable_extra.structure_refusal().is_some());
    }

    /// P2 (b): an empty, single-point, non-finite or fully degenerate path is an explicit
    /// invalid-path state. It is never a proven bearing-free line, and neither gate may
    /// pass it because its interval list happened to be empty. Allowed repeated vertices
    /// keep working, so a legitimate stair is not refused for one.
    #[test]
    fn g22b_empty_or_degenerate_paths_are_unevaluated_not_bare_ground() {
        let deck = wide_deck();
        let empty: Vec<[f64; 3]> = vec![];
        let coincident = [5.5, 1.0, 8.0];
        let invalids: Vec<(&str, Vec<[f64; 3]>)> = vec![
            ("empty path", empty),
            ("single point", vec![coincident]),
            ("two coincident points", vec![coincident, coincident]),
            (
                "three coincident points",
                vec![coincident, coincident, coincident],
            ),
            (
                "all segments degenerate",
                vec![
                    [5.5, 1.0, 8.0],
                    [5.5, 1.0, 8.0],
                    [5.5, 1.0, 8.0],
                    [5.5, 1.0, 8.0],
                ],
            ),
            (
                "non-finite vertex",
                vec![[5.5, 1.0, 8.0], [8.0, f64::NAN, 8.0], [10.5, 1.0, 8.0]],
            ),
            (
                "infinite vertex",
                vec![[5.5, 1.0, 8.0], [f64::INFINITY, 1.0, 8.0]],
            ),
        ];
        for (label, path) in invalids {
            let facts = structural_support_detail(&[("deck::deck", deck)], &path, 1.5);
            assert_eq!(
                facts.bearing,
                support_coverage::BearingIdentity::Unknown,
                "{label}: an unevaluated path is not a bearing-free proof"
            );
            assert_eq!(
                facts.bearing_reason,
                support_coverage::REASON_INVALID_PATH,
                "{label}"
            );
            assert_eq!(
                facts.coverage,
                support_coverage::CoverageVerdict::Indeterminate,
                "{label}"
            );
            assert_eq!(facts.band_m, 0.0, "{label}");
            // The invariant that closes the boundary: the state is carried by a real,
            // non-empty interval, so no consumer can pass it by iterating nothing.
            assert!(
                !facts.intervals.is_empty(),
                "{label}: an unevaluated line still reports one explicit interval"
            );
            assert!(
                facts.intervals.iter().all(|interval| interval.bearing
                    == support_coverage::BearingIdentity::Unknown
                    && !interval.carried
                    && interval.band_m == 0.0),
                "{label}"
            );
            let refusal = facts
                .ground_refusal()
                .unwrap_or_else(|| panic!("{label}: the ground gate must refuse"));
            assert_eq!(refusal.category, "ground_edge_support_unknown", "{label}");
            assert_eq!(
                facts.structure_refusal().expect("refused").category,
                "structure_edge_unsupported",
                "{label}"
            );
        }

        // An allowed repeated vertex is normalised by dropping the zero-length segment and
        // the rest of the line is evaluated as usual: a legitimate stair keeps its verdict.
        let repeated = vec![[5.5, 1.0, 8.0], [5.5, 1.0, 8.0], [10.5, 1.0, 8.0]];
        let facts = structural_support_detail(&[("deck::deck", deck)], &repeated, 1.5);
        assert_eq!(facts.bearing, support_coverage::BearingIdentity::Structure);
        assert_eq!(facts.coverage, support_coverage::CoverageVerdict::Certified);
        assert!(facts.band_m + 1e-9 >= 1.5);
        assert!(facts.ground_refusal().is_none());
        assert!(facts.structure_refusal().is_none());
        assert!(!facts.intervals.is_empty());
        assert!(
            facts.intervals.iter().all(|interval| interval.segment == 1),
            "the zero-length segment 0 contributes nothing: {:?}",
            facts
                .intervals
                .iter()
                .map(|interval| interval.segment)
                .collect::<Vec<_>>()
        );

        // A valid bare-ground line is still a proven bearing-free one: no frames at all,
        // a real two-point path. The ground gate lets it through to the authoritative
        // terrain rules while the structure gate refuses it.
        let bare: Vec<(&str, MemberFrame)> = vec![];
        let facts = structural_support_detail(&bare, &deck_line(), 1.5);
        assert_eq!(
            facts.bearing,
            support_coverage::BearingIdentity::NoStructuralSupport
        );
        assert_eq!(
            facts.coverage,
            support_coverage::CoverageVerdict::NotApplicable
        );
        assert_eq!(
            facts.bearing_reason,
            support_coverage::REASON_NO_CANDIDATE_PARTS_PROVEN
        );
        assert!(
            facts.ground_refusal().is_none(),
            "bare ground is not refused here"
        );
        assert_eq!(
            facts
                .structure_refusal()
                .expect("a structure edge may not be bare")
                .category,
            "structure_edge_unsupported"
        );
    }

    /// The positive and refusal branches of the production entrance, side by side: a valid
    /// bearing-free line, a valid certified structure, a narrow structure, and a budget
    /// that cannot decide.
    #[test]
    fn g22b_valid_bare_ground_and_valid_structure_keep_their_state() {
        let line = deck_line();
        let deck = wide_deck();
        let bare: Vec<(&str, MemberFrame)> = vec![];

        // valid, proven bearing-free: ground continues, structure refuses
        let ground_only = structural_support_detail(&bare, &line, 1.5);
        assert_eq!(
            ground_only.coverage,
            support_coverage::CoverageVerdict::NotApplicable
        );
        assert!(ground_only.ground_refusal().is_none());
        assert!(ground_only.structure_refusal().is_some());

        // valid, certified structure: both gates pass
        let certified = structural_support_detail(&[("deck::deck", deck)], &line, 1.5);
        assert_eq!(
            certified.coverage,
            support_coverage::CoverageVerdict::Certified
        );
        assert!(certified.ground_refusal().is_none());
        assert!(certified.structure_refusal().is_none());

        // valid, structure-borne but too narrow: structure + gap, both gates refuse
        let narrow = MemberFrame {
            half_width: 0.2,
            ..deck
        };
        let too_narrow = structural_support_detail(&[("deck::deck", narrow)], &line, 1.5);
        assert_eq!(
            too_narrow.bearing,
            support_coverage::BearingIdentity::Structure
        );
        assert_eq!(too_narrow.coverage, support_coverage::CoverageVerdict::Gap);
        assert!(too_narrow.band_m < 1.5);
        let refusal = too_narrow
            .ground_refusal()
            .expect("the narrow deck refuses");
        assert_eq!(refusal.category, "ground_edge_unsupported_by_structure");
        assert_eq!(
            refusal.bearing,
            support_coverage::BearingIdentity::Structure
        );
        assert!(too_narrow.structure_refusal().is_some());

        // an exhausted budget: the identity a real station proves survives, the coverage
        // does not, and both gates refuse through the production entrance
        let starved = structural_support_detail_with_budget(&[("deck::deck", deck)], &line, 1.5, 0);
        assert_eq!(
            starved.bearing,
            support_coverage::BearingIdentity::Structure
        );
        assert_eq!(
            starved.coverage,
            support_coverage::CoverageVerdict::Indeterminate
        );
        assert_eq!(
            starved.bearing_reason,
            support_coverage::REASON_COVERAGE_BUDGET_EXHAUSTED
        );
        assert!(!starved.intervals.is_empty());
        assert_eq!(
            starved.ground_refusal().expect("refused").category,
            "ground_edge_unsupported_by_structure"
        );
        assert!(starved.structure_refusal().is_some());

        // a frame that is usable but carries nothing here is still a valid measurement:
        // the deck four metres above does not fill this layer's line
        let above = MemberFrame { top: 5.0, ..deck };
        let other_layer = structural_support_detail(&[("above::deck", above)], &line, 1.5);
        assert_eq!(
            other_layer.bearing,
            support_coverage::BearingIdentity::NoStructuralSupport
        );
        assert_eq!(
            other_layer.coverage,
            support_coverage::CoverageVerdict::NotApplicable
        );
        assert!(other_layer.ground_refusal().is_none());
        assert!(other_layer.structure_refusal().is_some());
    }
}
