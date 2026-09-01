//! Error types for the rules engine: load errors, placement errors and the
//! machine-readable rejection reasons.

use serde::Serialize;
use thiserror::Error;

use super::schema::ItemKind;

/// Errors while loading / validating an object descriptor file.
#[derive(Debug, Error)]
pub enum RuleLoadError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path} as TOML: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("schema validation failed for {id}: {message}")]
    Schema { id: String, message: String },
}

/// Why a candidate position was rejected. Serialises to JSON for reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    OutOfBounds,
    InWater,
    ForbiddenHazard,
    /// slope in metres/100m, max in metres/100m.
    SlopeTooHigh { slope: f32, max: f32 },
    ReservationConflict,
    GeometryCollision { conflict_kind: ItemKind },
    MissingRequiredRelation { kind: ItemKind },
    BlockedEntrance { anchor: String },
    NotGrounded,
    DisconnectedAccess,
}

impl RejectReason {
    /// Short, human-readable string (used in the JSON report so `reason` is a
    /// plain string rather than an object).
    pub fn as_str(&self) -> String {
        format!("{self}")
    }
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectReason::OutOfBounds => write!(f, "out of bounds"),
            RejectReason::InWater => write!(f, "in water"),
            RejectReason::ForbiddenHazard => write!(f, "forbidden hazard"),
            RejectReason::SlopeTooHigh { slope, max } => {
                write!(f, "slope {slope}% > max {max}%")
            }
            RejectReason::ReservationConflict => write!(f, "reservation conflict"),
            RejectReason::GeometryCollision { conflict_kind } => {
                write!(f, "geometry collision with {conflict_kind:?}")
            }
            RejectReason::MissingRequiredRelation { kind } => {
                write!(f, "missing required relation near {kind:?}")
            }
            RejectReason::BlockedEntrance { anchor } => {
                write!(f, "entrance '{anchor}' blocked")
            }
            RejectReason::NotGrounded => write!(f, "not grounded"),
            RejectReason::DisconnectedAccess => write!(f, "disconnected access"),
        }
    }
}

/// One rejected candidate, serialisable to the validation report.
#[derive(Debug, Clone, Serialize)]
pub struct RejectRecord {
    pub item_id: String,
    pub candidate_x: i32,
    pub candidate_z: i32,
    /// Human-readable reason string (e.g. "in water", "slope 40% > max 30%").
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_with: Option<String>,
    #[serde(default)]
    pub rule: String,
}

/// Result of one placement call.
#[derive(Debug, Clone, Default)]
pub struct PlacementOutcome {
    /// Entities successfully placed.
    pub placed: Vec<crate::worldgen::EntityInstance>,
    /// Rejected candidates (for diagnostics / reports).
    pub rejected: Vec<RejectRecord>,
}

impl PlacementOutcome {
    pub fn is_clean(&self) -> bool {
        self.rejected
            .iter()
            .all(|r| !is_severe_reason(&r.reason))
    }
}

/// True if a reason string represents a hard violation (floating / submerged /
/// collision / blocked entrance). Diagnostic-only reasons (slope, bounds,
/// missing relation) are not "severe" for a clean check.
pub fn is_severe_reason(reason: &str) -> bool {
    reason.starts_with("in water")
        || reason.starts_with("not grounded")
        || reason.starts_with("geometry collision")
        || reason.starts_with("entrance ")
}

/// Aggregate validation report for a whole map (serialisable).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    pub seed: u64,
    pub buildings: usize,
    pub roads: usize,
    pub floating_items: usize,
    pub submerged_items: usize,
    pub blocked_entrances: usize,
    pub geometry_collisions: usize,
    pub disconnected_roads: usize,
    /// Full reject list (may be large — serialise to file, not stdout).
    pub rejects: Vec<RejectRecord>,
    /// Entities that were checked against a rule.
    #[serde(default)]
    pub checked_items: usize,
    /// Entities that passed all hard constraints.
    #[serde(default)]
    pub passed_items: usize,
    /// Entities that were rejected by at least one hard constraint.
    #[serde(default)]
    pub rejected_items: usize,
    /// Entity kind strings with no matching descriptor (coverage gaps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unruled_items: Vec<String>,
}

impl ValidationReport {
    /// Number of distinct unruled kinds (coverage gap indicator).
    pub fn unruled_count(&self) -> usize {
        let mut set: Vec<&str> = self.unruled_items.iter().map(String::as_str).collect();
        set.sort_unstable();
        set.dedup();
        set.len()
    }

    /// Increment the matching counter for a reject reason.
    pub fn count_reason(&mut self, reason: &RejectReason) {
        match reason {
            RejectReason::InWater | RejectReason::ForbiddenHazard => self.submerged_items += 1,
            RejectReason::NotGrounded => self.floating_items += 1,
            RejectReason::BlockedEntrance { .. } => self.blocked_entrances += 1,
            RejectReason::GeometryCollision { .. } | RejectReason::ReservationConflict => {
                self.geometry_collisions += 1;
            }
            RejectReason::DisconnectedAccess => self.disconnected_roads += 1,
            RejectReason::OutOfBounds
            | RejectReason::SlopeTooHigh { .. }
            | RejectReason::MissingRequiredRelation { .. } => {}
        }
    }
}
