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
    pub reason: RejectReason,
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
            .all(|r| !matches!(r.reason, RejectReason::InWater | RejectReason::NotGrounded | RejectReason::GeometryCollision { .. } | RejectReason::BlockedEntrance { .. }))
    }
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
    #[serde(skip)]
    pub rejects: Vec<RejectRecord>,
}
