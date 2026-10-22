//! Read-only engineering context for one explicitly selected world window.
//!
//! The caller names a world root and a window; everything else is derived from
//! the saved bytes by one bounded read. The parsed scene, its compatibility
//! projection and the source bindings are produced by that single read and are
//! re-checked on every use, so a caller cannot pair the bytes of one scene with
//! the identity or geometry of another and still obtain a context that claims
//! to describe the saved source.
//!
//! This module never builds an `EngineeringInput`, never routes, never grades,
//! and never promotes the saved `connected` flag into a re-verified pass. Facts
//! copied from the source are marked `sourceObservationOnly` / `notReverified`
//! or `referenceOnly`; checks that did not run stay `notEvaluated`.
//!
//! Height rasters are decoded by the authoritative `HeightPayload::parse`; this
//! module only maps its raw units to integer millimetres losslessly.

use crate::engineering_preview::{is_canonical_relative_path, sha256_hex};
use crate::height_payload::{HeightEncoding, HeightPayload};
use crate::worldgen::{ChunkDescriptor, EntityInstance, SceneIndex};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: &str = "engineering-context-v3";
pub const ROLE_FINAL_HEIGHT: &str = "final-height";
pub const ROLE_NATURAL_HEIGHT: &str = "natural-height";
pub const ROLE_WORLD_INDEX: &str = "world-index";
pub const ROLE_SCENE_INDEX: &str = "scene-index";
/// Limitation facts the producer must always disclose. The audit requires each
/// of these prefixes in the freshly rebuilt context, so a package cannot drop,
/// shrink or replace them.
pub const MANDATED_LIMITATION_PREFIXES: [&str; 3] = [
    "saved access flags are source observations, not re-verified routes",
    "no EngineeringInput is built and no route is re-computed",
    "no water, protection or collider role is converted; see unmodelledKinds",
];
/// Required only when the saved scene predates the current schema.
pub const MANDATED_DRIFT_LIMITATION_PREFIX: &str =
    "the saved scene predates the current SceneIndex schema";
/// Fixed check set of a produced context: name, required state and the exact
/// detail the audit demands. Anything else is a contract violation.
pub const CONTEXT_CHECK_CONTRACT: [(&str, &str, &str); 9] = [
    ("sceneIdentityBound", "evaluated", "scene "),
    (
        "sourceBytesRecorded",
        "diagnostic",
        "path/role/bytes/sha256 recorded for",
    ),
    ("terrainComparison", "diagnostic", "natural and final"),
    (
        "sourceBinding",
        "notEvaluated",
        "formal audit stage; requires an independent rebuild from the caller-given root",
    ),
    (
        "terrainParity",
        "notEvaluated",
        "formal audit stage; not yet executed",
    ),
    (
        "entityAndRoadParity",
        "notEvaluated",
        "formal audit stage; not yet executed",
    ),
    (
        "contextCompleteness",
        "notEvaluated",
        "formal audit stage; not yet executed",
    ),
    (
        "accessRouteReverification",
        "notEvaluated",
        "the saved connected flag is copied as an observation only",
    ),
    (
        "engineeringFeasibility",
        "notEvaluated",
        "no EngineeringInput is built, no stair, no re-route",
    ),
];
/// Phrases that assert a verification this pipeline never performs. They are
/// refused anywhere in the check details or the limitations.
pub const FORBIDDEN_CLAIM_PHRASES: [&str; 6] = [
    "engineering feasible",
    "route verified",
    "verified route ",
    "buildable",
    "safe to build",
    "safetyverified",
];
/// The selected window is bounded to 128 m per side by the packet contract.
pub const MAX_WINDOW_SIDE_M: i32 = 128;
pub const MAX_WORLD_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_SCENE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_HEIGHT_BYTES: u64 = 16 * 1024 * 1024;
/// Cumulative cap over every height payload actually consumed in one read.
pub const MAX_TOTAL_HEIGHT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SELECTED_CHUNKS: usize = 64;
pub const MAX_ENTITY_RECORDS: usize = 8192;
pub const MAX_ACCESS_RECORDS: usize = 8192;

/// Scene subtrees this contract does not consume. They are declared so a
/// compatibility projection can drop them *explicitly*; the list is never
/// widened to accommodate a new failure.
pub const UNCONSUMED_SCENE_KEYS: [&str; 8] = [
    "landUseBudget",
    "landUseZones",
    "settlementGraph",
    "parcelPlan",
    "developmentPlan",
    "landcoverPlan",
    "landcoverSurfaces",
    "developmentProposals",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextError {
    WindowMalformed(String),
    WindowEmpty,
    WindowTooLarge {
        side_m: i32,
        limit_m: i32,
    },
    WindowOutsideScene {
        detail: String,
    },
    MissingChunk {
        world_x: i32,
        world_z: i32,
    },
    OverlappingCoverage {
        world_x: i32,
        world_z: i32,
        chunks: Vec<(i32, i32)>,
    },
    CoverageContradiction {
        detail: String,
    },
    MissingNaturalHeight {
        path: String,
    },
    PathNotCanonicalRelative {
        path: String,
    },
    PathEscapesRoot {
        path: String,
        detail: String,
    },
    SourceRead {
        path: String,
        detail: String,
    },
    SourceTooLarge {
        path: String,
        limit: u64,
        actual: u64,
    },
    TotalPayloadTooLarge {
        limit: u64,
        actual: u64,
    },
    /// The authoritative height parser refused these bytes.
    HeightRejected {
        path: String,
        detail: String,
    },
    IndexOutOfRange {
        path: String,
        index: u64,
        count: u64,
    },
    ChunkGridMismatch {
        detail: String,
    },
    PlanUnreadable {
        detail: String,
    },
    SceneUnreadable {
        detail: String,
    },
    SceneIdentityMismatch {
        detail: String,
    },
    ConsumedGeometryMutated {
        detail: String,
    },
    InconsistentSource {
        detail: String,
    },
    UnsupportedSource {
        detail: String,
    },
    DuplicateRoadIdentity {
        id: String,
        count: usize,
    },
    DuplicateNodeIdentity {
        id: u64,
        count: usize,
    },
    DuplicateEntityIdentity {
        id: String,
        count: usize,
    },
    UnknownRoadNode {
        road_id: String,
        node: u64,
    },
    TooManyRecords {
        what: &'static str,
        limit: usize,
        actual: usize,
    },
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowMalformed(s) => write!(f, "window is malformed: {s}"),
            Self::WindowEmpty => write!(f, "window has a non-positive side"),
            Self::WindowTooLarge { side_m, limit_m } => {
                write!(f, "window side {side_m} m exceeds the {limit_m} m limit")
            }
            Self::WindowOutsideScene { detail } => write!(f, "window leaves the scene: {detail}"),
            Self::MissingChunk { world_x, world_z } => {
                write!(f, "no chunk covers world sample ({world_x}, {world_z})")
            }
            Self::OverlappingCoverage {
                world_x,
                world_z,
                chunks,
            } => write!(
                f,
                "world sample ({world_x}, {world_z}) is covered by {} chunks {chunks:?}",
                chunks.len()
            ),
            Self::CoverageContradiction { detail } => write!(f, "contradictory coverage: {detail}"),
            Self::MissingNaturalHeight { path } => {
                write!(f, "natural height payload is missing: {path}")
            }
            Self::PathNotCanonicalRelative { path } => {
                write!(f, "path is not a canonical relative path: {path}")
            }
            Self::PathEscapesRoot { path, detail } => {
                write!(f, "path {path} escapes the source root: {detail}")
            }
            Self::SourceRead { path, detail } => write!(f, "cannot read {path}: {detail}"),
            Self::SourceTooLarge {
                path,
                limit,
                actual,
            } => {
                write!(f, "{path} is {actual} bytes, over the {limit} byte cap")
            }
            Self::TotalPayloadTooLarge { limit, actual } => {
                write!(
                    f,
                    "consumed height payloads total {actual} bytes, over {limit}"
                )
            }
            Self::HeightRejected { path, detail } => {
                write!(f, "authoritative height parser refused {path}: {detail}")
            }
            Self::IndexOutOfRange { path, index, count } => {
                write!(f, "index {index} is outside {path} ({count} samples)")
            }
            Self::ChunkGridMismatch { detail } => write!(f, "chunk grid mismatch: {detail}"),
            Self::PlanUnreadable { detail } => write!(f, "road plan is unreadable: {detail}"),
            Self::SceneUnreadable { detail } => write!(f, "scene is unreadable: {detail}"),
            Self::SceneIdentityMismatch { detail } => {
                write!(f, "scene identity does not match its source: {detail}")
            }
            Self::ConsumedGeometryMutated { detail } => {
                write!(
                    f,
                    "consumed scene geometry changed after it was read: {detail}"
                )
            }
            Self::InconsistentSource { detail } => write!(f, "inconsistent source: {detail}"),
            Self::UnsupportedSource { detail } => write!(f, "unsupported source: {detail}"),
            Self::DuplicateRoadIdentity { id, count } => {
                write!(f, "road edge identity {id} appears {count} times")
            }
            Self::DuplicateNodeIdentity { id, count } => {
                write!(f, "road node identity {id} appears {count} times")
            }
            Self::DuplicateEntityIdentity { id, count } => {
                write!(f, "entity identity {id} appears {count} times")
            }
            Self::UnknownRoadNode { road_id, node } => {
                write!(f, "road {road_id} references undeclared node {node}")
            }
            Self::TooManyRecords {
                what,
                limit,
                actual,
            } => {
                write!(f, "too many {what} records: {actual} over {limit}")
            }
        }
    }
}

impl std::error::Error for ContextError {}

pub fn encoding_label(encoding: HeightEncoding) -> &'static str {
    match encoding {
        HeightEncoding::Millimetre => "gwhm-v1-i32le-millimetre",
        HeightEncoding::QuarterMetre => "legacy-i16le-quarter-metre",
    }
}

/// One decoded raster, in integer millimetres. Decoding is delegated to the
/// authoritative parser; the multiply for legacy quarter-metres is the only
/// conversion applied here and is lossless.
#[derive(Debug, Clone)]
pub struct DecodedHeight {
    pub path: String,
    pub role: &'static str,
    pub sha256: String,
    pub bytes: u64,
    pub width: u32,
    pub depth: u32,
    pub encoding: HeightEncoding,
    pub millimetres: Vec<i32>,
}

impl DecodedHeight {
    pub fn decode(
        path: &str,
        role: &'static str,
        bytes: &[u8],
        width: u32,
        depth: u32,
    ) -> Result<Self, ContextError> {
        let payload = HeightPayload::parse(bytes, width, depth).map_err(|error| {
            ContextError::HeightRejected {
                path: path.to_string(),
                detail: error.to_string(),
            }
        })?;
        let mut millimetres = Vec::with_capacity(payload.count);
        for index in 0..payload.count {
            let millimetres_value = match payload.encoding {
                HeightEncoding::Millimetre => payload.raw(index),
                HeightEncoding::QuarterMetre => {
                    payload.raw(index).checked_mul(250).ok_or_else(|| {
                        ContextError::HeightRejected {
                            path: path.to_string(),
                            detail: format!("sample {index} overflows millimetres"),
                        }
                    })?
                }
            };
            millimetres.push(millimetres_value);
        }
        Ok(Self {
            path: path.to_string(),
            role,
            sha256: sha256_hex(bytes),
            bytes: bytes.len() as u64,
            width,
            depth,
            encoding: payload.encoding,
            millimetres,
        })
    }

    pub fn millimetres_at(&self, local_x: u32, local_z: u32) -> Result<i32, ContextError> {
        let count = u64::from(self.width) * u64::from(self.depth);
        let index = u64::from(local_z) * u64::from(self.width) + u64::from(local_x);
        if local_x >= self.width || local_z >= self.depth {
            return Err(ContextError::IndexOutOfRange {
                path: self.path.clone(),
                index,
                count,
            });
        }
        self.millimetres
            .get(index as usize)
            .copied()
            .ok_or(ContextError::IndexOutOfRange {
                path: self.path.clone(),
                index,
                count,
            })
    }

    pub fn evidence(&self) -> HeightFileEvidence {
        HeightFileEvidence {
            path: self.path.clone(),
            role: self.role,
            bytes: self.bytes,
            sha256: self.sha256.clone(),
            width: self.width,
            depth: self.depth,
            encoding: encoding_label(self.encoding),
        }
    }
}

/// Serializable description of one consumed height file. Deliberately carries
/// no samples: the context exposes the local crop, not whole-scene rasters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightFileEvidence {
    pub path: String,
    pub role: &'static str,
    pub bytes: u64,
    pub sha256: String,
    pub width: u32,
    pub depth: u32,
    pub encoding: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBinding {
    pub path: String,
    pub role: &'static str,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct ChunkSource {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub world_x: i32,
    pub world_z: i32,
    pub valid_width_m: u32,
    pub valid_depth_m: u32,
    pub final_height: DecodedHeight,
    pub natural_height: DecodedHeight,
}

impl ChunkSource {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        world_x: i32,
        world_z: i32,
        valid_width_m: u32,
        valid_depth_m: u32,
        final_height: DecodedHeight,
        natural_height: DecodedHeight,
    ) -> Self {
        Self {
            chunk_x,
            chunk_z,
            world_x,
            world_z,
            valid_width_m,
            valid_depth_m,
            final_height,
            natural_height,
        }
    }

    pub fn key(&self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }

    /// Half-open on x and z, so touching chunks never both claim a sample.
    pub fn covers(&self, world_x: i32, world_z: i32) -> bool {
        let in_x = world_x >= self.world_x
            && i64::from(world_x) < i64::from(self.world_x) + i64::from(self.valid_width_m);
        let in_z = world_z >= self.world_z
            && i64::from(world_z) < i64::from(self.world_z) + i64::from(self.valid_depth_m);
        in_x && in_z
    }

    /// Relevance for reading: a chunk supplies samples to the window only when
    /// its half-open extent meets the window's half-open extent. A chunk that
    /// merely touches the far edge contributes no sample and is never opened,
    /// so its payloads may legitimately be absent. (Entity selection uses the
    /// closed rule instead, because contact is ambiguous for footprints.)
    pub fn extent_meets(&self, window: WindowRect) -> bool {
        let self_max_x = i64::from(self.world_x) + i64::from(self.valid_width_m);
        let self_max_z = i64::from(self.world_z) + i64::from(self.valid_depth_m);
        let window_max_x = i64::from(window.min_x) + i64::from(window.width_m);
        let window_max_z = i64::from(window.min_z) + i64::from(window.depth_m);
        i64::from(self.world_x) < window_max_x
            && self_max_x > i64::from(window.min_x)
            && i64::from(self.world_z) < window_max_z
            && self_max_z > i64::from(window.min_z)
    }

    fn check_extent(&self) -> Result<(), ContextError> {
        for decoded in [&self.final_height, &self.natural_height] {
            if decoded.width != self.valid_width_m || decoded.depth != self.valid_depth_m {
                return Err(ContextError::CoverageContradiction {
                    detail: format!(
                        "chunk({}, {}) declares {} x {} m but a payload is {} x {}",
                        self.chunk_x,
                        self.chunk_z,
                        self.valid_width_m,
                        self.valid_depth_m,
                        decoded.width,
                        decoded.depth
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn evidences(&self) -> Vec<HeightFileEvidence> {
        vec![self.final_height.evidence(), self.natural_height.evidence()]
    }
}

/// Scene projection produced by the compatibility read.
#[derive(Debug, Clone)]
pub struct SceneLoad {
    pub scene: SceneIndex,
    pub strict: bool,
    pub strict_error: Option<String>,
    pub dropped_keys: Vec<String>,
}

/// Reads a saved scene index. The strict read is attempted first and its error
/// is always kept; only when the strict read fails *and* the failure survives
/// dropping the declared unconsumed subtrees is the read refused. A dropped
/// subtree is recorded, never reported as verified, and the skip list is fixed.
pub fn load_scene_index(bytes: &[u8]) -> Result<SceneLoad, ContextError> {
    let strict_error = match serde_json::from_slice::<SceneIndex>(bytes) {
        Ok(scene) => {
            return Ok(SceneLoad {
                scene,
                strict: true,
                strict_error: None,
                dropped_keys: Vec::new(),
            });
        }
        Err(error) => error.to_string(),
    };
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| ContextError::SceneUnreadable {
            detail: error.to_string(),
        })?;
    let Some(object) = value.as_object_mut() else {
        return Err(ContextError::SceneUnreadable {
            detail: "scene root is not an object".to_string(),
        });
    };
    let mut dropped_keys = Vec::new();
    for key in UNCONSUMED_SCENE_KEYS {
        if object.remove(key).is_some() {
            dropped_keys.push(key.to_string());
        }
    }
    if dropped_keys.is_empty() {
        return Err(ContextError::SceneUnreadable {
            detail: strict_error,
        });
    }
    let scene: SceneIndex =
        serde_json::from_value(value).map_err(|error| ContextError::SceneUnreadable {
            detail: format!(
                "strict read failed: {strict_error}; after dropping {dropped_keys:?}: {error}"
            ),
        })?;
    Ok(SceneLoad {
        scene,
        strict: false,
        strict_error: Some(strict_error),
        dropped_keys,
    })
}

fn json_of<T: Serialize>(value: &T) -> Result<serde_json::Value, ContextError> {
    serde_json::to_value(value).map_err(|error| ContextError::PlanUnreadable {
        detail: error.to_string(),
    })
}

/// Digest over exactly the scene data this contract consumes. It is computed
/// when the scene is read and re-computed on every use, so in-process edits to
/// the identity or the consumed geometry are detected instead of being silently
/// described as the saved source.
fn consumed_digest(scene: &SceneIndex) -> Result<String, ContextError> {
    let mut map = serde_json::Map::new();
    map.insert("sceneId".into(), serde_json::json!(scene.scene_id));
    map.insert("widthM".into(), serde_json::json!(scene.width_m));
    map.insert("depthM".into(), serde_json::json!(scene.depth_m));
    map.insert("originX".into(), serde_json::json!(scene.origin_x));
    map.insert("originZ".into(), serde_json::json!(scene.origin_z));
    map.insert("chunkSizeM".into(), serde_json::json!(scene.chunk_size_m));
    map.insert("chunkCountX".into(), serde_json::json!(scene.chunk_count_x));
    map.insert("chunkCountZ".into(), serde_json::json!(scene.chunk_count_z));
    map.insert("chunks".into(), json_of(&scene.chunks)?);
    map.insert("entities".into(), json_of(&scene.entities)?);
    map.insert("roadNetworkPlan".into(), json_of(&scene.road_network_plan)?);
    map.insert("accessGraph".into(), json_of(&scene.access_graph)?);
    map.insert("naturalTerrain".into(), json_of(&scene.natural_terrain)?);
    map.insert(
        "engineeredTerrain".into(),
        json_of(&scene.engineered_terrain)?,
    );
    let bytes = serde_json::to_vec(&serde_json::Value::Object(map)).map_err(|error| {
        ContextError::PlanUnreadable {
            detail: error.to_string(),
        }
    })?;
    Ok(sha256_hex(&bytes))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// Resolves a source-relative path under `root`, refusing traversal, absolute
/// paths, symbolic links and Windows reparse points, and verifying after
/// resolution that the target really is inside the root. Public so the audit
/// package reader reuses exactly this rule instead of inventing a second one.
pub fn resolve_within_root(root: &Path, relative: &str) -> Result<PathBuf, ContextError> {
    if !is_canonical_relative_path(relative) {
        return Err(ContextError::PathNotCanonicalRelative {
            path: relative.to_string(),
        });
    }
    let root_canonical = fs::canonicalize(root).map_err(|error| ContextError::SourceRead {
        path: root.display().to_string(),
        detail: error.to_string(),
    })?;
    let mut current = root_canonical.clone();
    for part in relative.split('/') {
        current.push(part);
        let metadata =
            fs::symlink_metadata(&current).map_err(|error| ContextError::SourceRead {
                path: relative.to_string(),
                detail: error.to_string(),
            })?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(ContextError::PathEscapesRoot {
                path: relative.to_string(),
                detail: format!("{part} is a symbolic link or reparse point"),
            });
        }
    }
    let target = fs::canonicalize(&current).map_err(|error| ContextError::SourceRead {
        path: relative.to_string(),
        detail: error.to_string(),
    })?;
    if !target.starts_with(&root_canonical) {
        return Err(ContextError::PathEscapesRoot {
            path: relative.to_string(),
            detail: "resolved target is outside the source root".to_string(),
        });
    }
    Ok(target)
}

fn read_capped(root: &Path, relative: &str, cap: u64) -> Result<Vec<u8>, ContextError> {
    let path = resolve_within_root(root, relative)?;
    let metadata = fs::metadata(&path).map_err(|error| ContextError::SourceRead {
        path: relative.to_string(),
        detail: error.to_string(),
    })?;
    if metadata.len() > cap {
        return Err(ContextError::SourceTooLarge {
            path: relative.to_string(),
            limit: cap,
            actual: metadata.len(),
        });
    }
    fs::read(&path).map_err(|error| ContextError::SourceRead {
        path: relative.to_string(),
        detail: error.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub min_x: i32,
    pub min_z: i32,
    pub width_m: i32,
    pub depth_m: i32,
}

impl WindowRect {
    /// Parses the packet's `x,z,width,depth` window syntax.
    pub fn parse(text: &str) -> Result<Self, ContextError> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(ContextError::WindowMalformed(text.to_string()));
        }
        let mut numbers = [0i32; 4];
        for (slot, part) in numbers.iter_mut().zip(parts.iter()) {
            *slot = part
                .parse::<i32>()
                .map_err(|_| ContextError::WindowMalformed(text.to_string()))?;
        }
        Ok(Self {
            min_x: numbers[0],
            min_z: numbers[1],
            width_m: numbers[2],
            depth_m: numbers[3],
        })
    }

    /// Size and arithmetic checks that every crop entry performs itself.
    pub fn check_shape(&self) -> Result<(), ContextError> {
        if self.width_m <= 0 || self.depth_m <= 0 {
            return Err(ContextError::WindowEmpty);
        }
        for side in [self.width_m, self.depth_m] {
            if side > MAX_WINDOW_SIDE_M {
                return Err(ContextError::WindowTooLarge {
                    side_m: side,
                    limit_m: MAX_WINDOW_SIDE_M,
                });
            }
        }
        // Forces the extent arithmetic to be checked before any indexing.
        let _ = self.sample_count();
        Ok(())
    }

    pub fn validate_against(
        &self,
        origin_x: i32,
        origin_z: i32,
        width_m: u32,
        depth_m: u32,
    ) -> Result<(), ContextError> {
        self.check_shape()?;
        let max_x = i64::from(origin_x) + i64::from(width_m);
        let max_z = i64::from(origin_z) + i64::from(depth_m);
        let window_max_x = i64::from(self.min_x) + i64::from(self.width_m);
        let window_max_z = i64::from(self.min_z) + i64::from(self.depth_m);
        if i64::from(self.min_x) < i64::from(origin_x)
            || i64::from(self.min_z) < i64::from(origin_z)
            || window_max_x > max_x
            || window_max_z > max_z
        {
            return Err(ContextError::WindowOutsideScene {
                detail: format!(
                    "window {},{},{},{} is not inside origin ({origin_x}, {origin_z}) extent {width_m} x {depth_m}",
                    self.min_x, self.min_z, self.width_m, self.depth_m
                ),
            });
        }
        Ok(())
    }

    pub fn validate(&self, scene: &SceneIndex) -> Result<(), ContextError> {
        self.validate_against(scene.origin_x, scene.origin_z, scene.width_m, scene.depth_m)
    }

    pub fn sample_count(&self) -> u64 {
        (self.width_m as u64) * (self.depth_m as u64)
    }

    pub fn min_x_f64(&self) -> f64 {
        f64::from(self.min_x)
    }

    pub fn min_z_f64(&self) -> f64 {
        f64::from(self.min_z)
    }

    pub fn max_x_f64(&self) -> f64 {
        f64::from(self.min_x) + f64::from(self.width_m)
    }

    pub fn max_z_f64(&self) -> f64 {
        f64::from(self.min_z) + f64::from(self.depth_m)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleRef {
    pub world_x: i32,
    pub world_z: i32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub local_x: u32,
    pub local_z: u32,
    pub sample_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crop {
    pub window: WindowRect,
    pub width_m: i32,
    pub depth_m: i32,
    pub final_mm: Vec<i32>,
    pub natural_mm: Vec<i32>,
    pub covering_chunks: Vec<(i32, i32)>,
    pub natural_equals_final: bool,
    pub differing_samples: u64,
    #[serde(skip_serializing)]
    pub samples: Vec<SampleRef>,
}

/// Crops a window from per-chunk payloads. Every sample resolves its own
/// covering chunk; a sample covered by no chunk, or by more than one, is
/// refused instead of falling back to any particular chunk. The entry performs
/// its own shape and arithmetic checks.
pub fn crop_chunks(chunks: &[ChunkSource], window: WindowRect) -> Result<Crop, ContextError> {
    window.check_shape()?;
    for chunk in chunks {
        chunk.check_extent()?;
    }
    let total = window.sample_count();
    let mut final_mm = Vec::with_capacity(total as usize);
    let mut natural_mm = Vec::with_capacity(total as usize);
    let mut samples = Vec::with_capacity(total as usize);
    let mut covering: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut differing = 0u64;

    for local_z in 0..window.depth_m {
        for local_x in 0..window.width_m {
            let world_x = window.min_x + local_x;
            let world_z = window.min_z + local_z;
            let mut matches = chunks.iter().filter(|c| c.covers(world_x, world_z));
            let chunk = matches
                .next()
                .ok_or(ContextError::MissingChunk { world_x, world_z })?;
            let extra: Vec<(i32, i32)> = matches.map(ChunkSource::key).collect();
            if !extra.is_empty() {
                let mut keys = vec![chunk.key()];
                keys.extend(extra);
                return Err(ContextError::OverlappingCoverage {
                    world_x,
                    world_z,
                    chunks: keys,
                });
            }
            let in_chunk_x = (i64::from(world_x) - i64::from(chunk.world_x)) as u32;
            let in_chunk_z = (i64::from(world_z) - i64::from(chunk.world_z)) as u32;
            let final_value = chunk.final_height.millimetres_at(in_chunk_x, in_chunk_z)?;
            let natural_value = chunk
                .natural_height
                .millimetres_at(in_chunk_x, in_chunk_z)?;
            if final_value != natural_value {
                differing += 1;
            }
            let sample_index =
                u64::from(in_chunk_z) * u64::from(chunk.valid_width_m) + u64::from(in_chunk_x);
            covering.insert(chunk.key());
            samples.push(SampleRef {
                world_x,
                world_z,
                chunk_x: chunk.chunk_x,
                chunk_z: chunk.chunk_z,
                local_x: in_chunk_x,
                local_z: in_chunk_z,
                sample_index,
            });
            final_mm.push(final_value);
            natural_mm.push(natural_value);
        }
    }

    Ok(Crop {
        window,
        width_m: window.width_m,
        depth_m: window.depth_m,
        natural_equals_final: differing == 0,
        differing_samples: differing,
        covering_chunks: covering.into_iter().collect(),
        final_mm,
        natural_mm,
        samples,
    })
}

/// Facts about one road node preserved from the plan.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeFact {
    pub id: u64,
    pub world_x: Option<i32>,
    pub world_z: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEdge {
    pub id: String,
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub world_x: Option<i32>,
    pub world_z: Option<i32>,
    pub length_m: Option<f64>,
    pub rotation_y_deg: Option<f64>,
    pub natural_min_m: Option<f64>,
    pub natural_max_m: Option<f64>,
}

/// Road-plan facts read from the serialised plan, so the plan schema is
/// preserved verbatim rather than re-modelled here. Structural integrity is
/// checked over the whole plan: duplicate edge or node identities and endpoint
/// references to undeclared nodes are refused, and an edge-bearing plan is
/// never allowed to skip the endpoint check.
pub fn plan_facts(
    scene: &SceneIndex,
) -> Result<(BTreeMap<String, PlanEdge>, BTreeMap<u64, NodeFact>), ContextError> {
    let mut edges: BTreeMap<String, PlanEdge> = BTreeMap::new();
    let mut nodes: BTreeMap<u64, NodeFact> = BTreeMap::new();
    let mut node_counts: BTreeMap<u64, usize> = BTreeMap::new();
    let Some(plan) = scene.road_network_plan.as_ref() else {
        return Ok((edges, nodes));
    };
    let value = json_of(plan)?;
    if let Some(array) = value.get("nodes").and_then(serde_json::Value::as_array) {
        for node in array {
            let Some(id) = node.get("id").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            *node_counts.entry(id).or_insert(0) += 1;
            let read_i32 = |key: &str| {
                node.get(key)
                    .and_then(serde_json::Value::as_i64)
                    .map(|value| value as i32)
            };
            nodes.insert(
                id,
                NodeFact {
                    id,
                    world_x: read_i32("worldX"),
                    world_z: read_i32("worldZ"),
                },
            );
        }
    }
    for (id, count) in &node_counts {
        if *count > 1 {
            return Err(ContextError::DuplicateNodeIdentity {
                id: *id,
                count: *count,
            });
        }
    }
    let mut edge_counts: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(array) = value.get("edges").and_then(serde_json::Value::as_array) {
        for edge in array {
            let id = edge
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            if id.is_empty() {
                continue;
            }
            *edge_counts.entry(id.clone()).or_insert(0) += 1;
            let seen = edge_counts[&id];
            if seen > 1 {
                return Err(ContextError::DuplicateRoadIdentity { id, count: seen });
            }
            let read_u64 = |key: &str| edge.get(key).and_then(serde_json::Value::as_u64);
            let read_i32 = |key: &str| {
                edge.get(key)
                    .and_then(serde_json::Value::as_i64)
                    .map(|value| value as i32)
            };
            let read_f64 = |key: &str| edge.get(key).and_then(serde_json::Value::as_f64);
            edges.insert(
                id,
                PlanEdge {
                    id: String::new(),
                    from: read_u64("from"),
                    to: read_u64("to"),
                    world_x: read_i32("worldX"),
                    world_z: read_i32("worldZ"),
                    length_m: read_f64("lengthM"),
                    rotation_y_deg: read_f64("rotationYDeg"),
                    natural_min_m: read_f64("naturalMinM"),
                    natural_max_m: read_f64("naturalMaxM"),
                },
            );
        }
    }
    for (id, edge) in edges.iter_mut() {
        edge.id = id.clone();
    }
    // Endpoint references are checked whenever any edge exists, including when
    // the node table is empty: an edge-bearing plan with no nodes is refused.
    for edge in edges.values() {
        for endpoint in [edge.from, edge.to].into_iter().flatten() {
            if !nodes.contains_key(&endpoint) {
                return Err(ContextError::UnknownRoadNode {
                    road_id: edge.id.clone(),
                    node: endpoint,
                });
            }
        }
    }
    Ok((edges, nodes))
}

/// Entity identities must be unique: a duplicate makes every later lookup
/// ambiguous, so it is refused rather than resolved by first match.
pub fn validate_scene_identities(scene: &SceneIndex) -> Result<(), ContextError> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entity in &scene.entities {
        *counts.entry(entity.entity_id.as_str()).or_insert(0) += 1;
    }
    for (id, count) in counts {
        if count > 1 {
            return Err(ContextError::DuplicateEntityIdentity {
                id: id.to_string(),
                count,
            });
        }
    }
    if scene.scene_id.trim().is_empty() {
        return Err(ContextError::SceneIdentityMismatch {
            detail: "the saved scene has an empty scene id".to_string(),
        });
    }
    Ok(())
}

/// Road relations for the roads the window selects. A named road without its
/// plan edge, or a road entity whose saved position or rotation contradicts the
/// plan edge, is reported as an inconsistent source instead of being recorded
/// as a merely absent edge. Values are compared in source units and the
/// rotation convention of the scene, with no added tolerance.
pub fn validate_road_geometry(scene: &SceneIndex, window: WindowRect) -> Result<(), ContextError> {
    let (edges, _nodes) = plan_facts(scene)?;
    for entity in &scene.entities {
        if entity.kind != "road" {
            continue;
        }
        let footprint = footprint_of(entity, None);
        if !footprint_intersects(&footprint, window) {
            continue;
        }
        let Some(edge) = edges.get(&entity.entity_id) else {
            return Err(ContextError::InconsistentSource {
                detail: format!(
                    "road entity {} in the window has no plan edge",
                    entity.entity_id
                ),
            });
        };
        if let Some(plan_x) = edge.world_x {
            if plan_x != entity.world_x {
                return Err(ContextError::InconsistentSource {
                    detail: format!(
                        "road {} x disagrees: entity {} vs plan {}",
                        entity.entity_id, entity.world_x, plan_x
                    ),
                });
            }
        }
        if let Some(plan_z) = edge.world_z {
            if plan_z != entity.world_z {
                return Err(ContextError::InconsistentSource {
                    detail: format!(
                        "road {} z disagrees: entity {} vs plan {}",
                        entity.entity_id, entity.world_z, plan_z
                    ),
                });
            }
        }
        if let Some(plan_rotation) = edge.rotation_y_deg {
            if plan_rotation != f64::from(entity.rotation_y_deg) {
                return Err(ContextError::InconsistentSource {
                    detail: format!(
                        "road {} rotation disagrees: entity {} vs plan {}",
                        entity.entity_id, entity.rotation_y_deg, plan_rotation
                    ),
                });
            }
        }
    }
    Ok(())
}

/// A source read is bound to the exact bytes it came from: the parsed scene,
/// the compatibility projection, the consumed-file bindings and the payloads
/// are all produced here and re-verified on every use.
#[derive(Debug, Clone)]
pub struct BoundScene {
    world_dir: PathBuf,
    world_path: String,
    scene_declaration: String,
    scene_rel_path: String,
    scene_dir: String,
    declared_scene_id: String,
    load: SceneLoad,
    consumed_digest: String,
    window: WindowRect,
    chunks: Vec<ChunkSource>,
    skipped_chunks: Vec<(i32, i32)>,
    total_height_bytes: u64,
    bindings: Vec<SourceBinding>,
}

impl BoundScene {
    /// Reads the world descriptor, follows its own scene declaration, reads the
    /// declared scene file, and consumes the height payloads of the chunks the
    /// window touches. Payloads of non-intersecting chunks are never opened, so
    /// a missing far-away raster cannot fail a legal local window.
    pub fn read(world_dir: &Path, window: WindowRect) -> Result<Self, ContextError> {
        window.check_shape()?;
        let world_path = "world.json".to_string();
        let world_bytes = read_capped(world_dir, &world_path, MAX_WORLD_BYTES)?;
        let world: serde_json::Value = serde_json::from_slice(&world_bytes).map_err(|error| {
            ContextError::SceneUnreadable {
                detail: format!("world.json: {error}"),
            }
        })?;
        let scene_declaration = scene_declaration_of(&world)?;
        let scene_rel_path = scene_declaration.clone();
        let scene_dir = scene_rel_path
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_string())
            .ok_or_else(|| ContextError::UnsupportedSource {
                detail: format!("scene declaration {scene_rel_path} has no directory component"),
            })?;
        if !scene_rel_path.ends_with("/scene.json") {
            return Err(ContextError::UnsupportedSource {
                detail: format!("scene declaration {scene_rel_path} is not a scene.json path"),
            });
        }
        let declared_scene_id = scene_dir
            .rsplit_once('/')
            .map(|(_, id)| id.to_string())
            .unwrap_or_else(|| scene_dir.clone());

        let scene_bytes = read_capped(world_dir, &scene_rel_path, MAX_SCENE_BYTES)?;
        let load = load_scene_index(&scene_bytes)?;
        if load.scene.scene_id != declared_scene_id {
            return Err(ContextError::SceneIdentityMismatch {
                detail: format!(
                    "declared {} but the scene file says {}",
                    declared_scene_id, load.scene.scene_id
                ),
            });
        }
        window.validate(&load.scene)?;
        validate_scene_identities(&load.scene)?;

        let mut bindings = vec![
            SourceBinding {
                path: world_path.clone(),
                role: ROLE_WORLD_INDEX,
                bytes: world_bytes.len() as u64,
                sha256: sha256_hex(&world_bytes),
            },
            SourceBinding {
                path: scene_rel_path.clone(),
                role: ROLE_SCENE_INDEX,
                bytes: scene_bytes.len() as u64,
                sha256: sha256_hex(&scene_bytes),
            },
        ];
        let digest = consumed_digest(&load.scene)?;

        let declared_chunks =
            u64::from(load.scene.chunk_count_x) * u64::from(load.scene.chunk_count_z);
        if declared_chunks != load.scene.chunks.len() as u64 {
            return Err(ContextError::ChunkGridMismatch {
                detail: format!(
                    "grid declares {declared_chunks} chunks but the scene lists {}",
                    load.scene.chunks.len()
                ),
            });
        }

        let mut candidates: Vec<&ChunkDescriptor> = Vec::new();
        let mut skipped_chunks: Vec<(i32, i32)> = Vec::new();
        for descriptor in &load.scene.chunks {
            let probe = ChunkSource::new(
                descriptor.chunk_x as i32,
                descriptor.chunk_z as i32,
                descriptor.world_x,
                descriptor.world_z,
                descriptor.valid_width_m,
                descriptor.valid_depth_m,
                empty_decoded(ROLE_FINAL_HEIGHT),
                empty_decoded(ROLE_NATURAL_HEIGHT),
            );
            if probe.extent_meets(window) {
                candidates.push(descriptor);
            } else {
                skipped_chunks.push((descriptor.chunk_x as i32, descriptor.chunk_z as i32));
            }
        }
        if candidates.is_empty() {
            return Err(ContextError::MissingChunk {
                world_x: window.min_x,
                world_z: window.min_z,
            });
        }
        if candidates.len() > MAX_SELECTED_CHUNKS {
            return Err(ContextError::TooManyRecords {
                what: "selected chunk",
                limit: MAX_SELECTED_CHUNKS,
                actual: candidates.len(),
            });
        }

        let mut chunks: Vec<ChunkSource> = Vec::new();
        let mut total_height_bytes = 0u64;
        for descriptor in candidates {
            let chunk = read_chunk(
                world_dir,
                &scene_dir,
                descriptor,
                &mut bindings,
                &mut total_height_bytes,
            )?;
            chunk.check_extent()?;
            chunks.push(chunk);
        }

        Ok(Self {
            world_dir: world_dir.to_path_buf(),
            world_path,
            scene_declaration,
            scene_rel_path,
            scene_dir,
            declared_scene_id,
            load,
            consumed_digest: digest,
            window,
            chunks,
            skipped_chunks,
            total_height_bytes,
            bindings,
        })
    }

    pub fn scene(&self) -> &SceneIndex {
        &self.load.scene
    }

    pub fn window(&self) -> WindowRect {
        self.window
    }

    pub fn bindings(&self) -> &[SourceBinding] {
        &self.bindings
    }

    pub fn world_dir(&self) -> &Path {
        &self.world_dir
    }

    pub fn scene_rel_path(&self) -> &str {
        &self.scene_rel_path
    }

    pub fn scene_declaration(&self) -> &str {
        &self.scene_declaration
    }

    pub fn declared_scene_id(&self) -> &str {
        &self.declared_scene_id
    }

    pub fn skipped_chunks(&self) -> &[(i32, i32)] {
        &self.skipped_chunks
    }

    pub fn total_height_bytes(&self) -> u64 {
        self.total_height_bytes
    }

    pub fn chunk_evidences(&self) -> Vec<HeightFileEvidence> {
        let mut evidences = Vec::new();
        for chunk in &self.chunks {
            evidences.extend(chunk.evidences());
        }
        evidences
    }

    /// Required files derived from the actual scene path and the actual
    /// descriptor names; nothing is hard-coded and no natural height file name
    /// is guessed.
    pub fn required_file_set(&self) -> Result<Vec<String>, ContextError> {
        let mut set = vec![self.world_path.clone(), self.scene_rel_path.clone()];
        for chunk in &self.chunks {
            // Payload paths already carry the scene directory prefix.
            set.push(chunk.final_height.path.clone());
            set.push(chunk.natural_height.path.clone());
        }
        set.sort();
        set.dedup();
        Ok(set)
    }

    /// Re-verifies that the in-memory scene still is the scene these bytes
    /// produced. Any identity or consumed-geometry edit is refused here.
    pub fn verify_identity(&self) -> Result<(), ContextError> {
        if self.load.scene.scene_id != self.declared_scene_id {
            return Err(ContextError::SceneIdentityMismatch {
                detail: format!(
                    "source declares {} but the scene now says {}",
                    self.declared_scene_id, self.load.scene.scene_id
                ),
            });
        }
        let digest = consumed_digest(&self.load.scene)?;
        if digest != self.consumed_digest {
            return Err(ContextError::ConsumedGeometryMutated {
                detail: format!("consumed digest changed from {}", self.consumed_digest),
            });
        }
        let required = self.required_file_set()?;
        for path in &required {
            if !self.bindings.iter().any(|binding| &binding.path == path) {
                return Err(ContextError::InconsistentSource {
                    detail: format!("required file {path} is not among the recorded bindings"),
                });
            }
        }
        Ok(())
    }

    pub fn crop(&self) -> Result<Crop, ContextError> {
        let crop = crop_chunks(&self.chunks, self.window)?;
        Ok(crop)
    }
}

fn empty_decoded(role: &'static str) -> DecodedHeight {
    DecodedHeight {
        path: String::new(),
        role,
        sha256: String::new(),
        bytes: 0,
        width: 1,
        depth: 1,
        encoding: HeightEncoding::Millimetre,
        millimetres: Vec::new(),
    }
}

/// The scene path comes from the world descriptor's own scene list; a world
/// that declares more than one scene is reported as unsupported rather than
/// silently reduced to the first entry.
fn scene_declaration_of(world: &serde_json::Value) -> Result<String, ContextError> {
    let Some(scenes) = world.get("scenes").and_then(serde_json::Value::as_array) else {
        return Err(ContextError::UnsupportedSource {
            detail: "world.json declares no scene list".to_string(),
        });
    };
    let mut declarations: Vec<String> = Vec::new();
    for entry in scenes {
        if let Some(path) = entry.as_str() {
            declarations.push(path.to_string());
        } else if let Some(path) = entry.get("path").and_then(serde_json::Value::as_str) {
            declarations.push(path.to_string());
        }
    }
    match declarations.len() {
        0 => Err(ContextError::UnsupportedSource {
            detail: "world.json declares no usable scene path".to_string(),
        }),
        1 => Ok(declarations.remove(0)),
        count => Err(ContextError::UnsupportedSource {
            detail: format!("world.json declares {count} scenes; this contract reads exactly one"),
        }),
    }
}

fn read_height(
    world_dir: &Path,
    scene_dir: &str,
    relative: &str,
    role: &'static str,
    width: u32,
    depth: u32,
    bindings: &mut Vec<SourceBinding>,
    total: &mut u64,
) -> Result<DecodedHeight, ContextError> {
    let label = format!("{scene_dir}/{relative}");
    let bytes = read_capped(world_dir, &label, MAX_HEIGHT_BYTES)?;
    *total = total.saturating_add(bytes.len() as u64);
    if *total > MAX_TOTAL_HEIGHT_BYTES {
        return Err(ContextError::TotalPayloadTooLarge {
            limit: MAX_TOTAL_HEIGHT_BYTES,
            actual: *total,
        });
    }
    bindings.push(SourceBinding {
        path: label.clone(),
        role,
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    });
    DecodedHeight::decode(&label, role, &bytes, width, depth)
}

fn read_chunk(
    world_dir: &Path,
    scene_dir: &str,
    descriptor: &ChunkDescriptor,
    bindings: &mut Vec<SourceBinding>,
    total: &mut u64,
) -> Result<ChunkSource, ContextError> {
    let final_height = read_height(
        world_dir,
        scene_dir,
        &descriptor.height_file,
        ROLE_FINAL_HEIGHT,
        descriptor.valid_width_m,
        descriptor.valid_depth_m,
        bindings,
        total,
    )?;
    let natural_rel = descriptor.natural_height_file.clone().ok_or_else(|| {
        ContextError::MissingNaturalHeight {
            path: format!(
                "chunk({}, {}) names no natural height payload",
                descriptor.chunk_x, descriptor.chunk_z
            ),
        }
    })?;
    let natural_height = read_height(
        world_dir,
        scene_dir,
        &natural_rel,
        ROLE_NATURAL_HEIGHT,
        descriptor.valid_width_m,
        descriptor.valid_depth_m,
        bindings,
        total,
    )?;
    Ok(ChunkSource::new(
        descriptor.chunk_x as i32,
        descriptor.chunk_z as i32,
        descriptor.world_x,
        descriptor.world_z,
        descriptor.valid_width_m,
        descriptor.valid_depth_m,
        final_height,
        natural_height,
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Footprint {
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
    pub source: &'static str,
}

/// Footprint of an entity. `world` is the saved entity object; when it carries
/// `bounds` the saved rectangle is compared against the outline computed from
/// the realised dimensions and rotation, and the **union** is used, so a saved
/// bound that is smaller than the true outline can never shrink an obstacle out
/// of the window. The realised `widthM`/`depthM` already include the asset
/// scale, so `scale` is preserved verbatim and never applied a second time.
pub fn footprint_of(entity: &EntityInstance, world: Option<&serde_json::Value>) -> Footprint {
    let half_width = f64::from(entity.width_m) / 2.0;
    let half_depth = f64::from(entity.depth_m) / 2.0;
    let (sin, cos) = f64::from(entity.rotation_y_deg).to_radians().sin_cos();
    let centre_x = f64::from(entity.world_x);
    let centre_z = f64::from(entity.world_z);
    let corners = [
        (-half_width, -half_depth),
        (half_width, -half_depth),
        (half_width, half_depth),
        (-half_width, half_depth),
    ];
    let mut min_x = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;
    for (local_x, local_z) in corners {
        let x = centre_x + local_x * cos - local_z * sin;
        let z = centre_z + local_x * sin + local_z * cos;
        min_x = min_x.min(x);
        min_z = min_z.min(z);
        max_x = max_x.max(x);
        max_z = max_z.max(z);
    }
    if let Some(bounds) = world.and_then(|value| value.get("bounds")) {
        let read = |key: &str| bounds.get(key).and_then(serde_json::Value::as_f64);
        if let (Some(saved_min_x), Some(saved_min_z), Some(saved_max_x), Some(saved_max_z)) =
            (read("minX"), read("minZ"), read("maxX"), read("maxZ"))
        {
            return Footprint {
                min_x: min_x.min(saved_min_x),
                min_z: min_z.min(saved_min_z),
                max_x: max_x.max(saved_max_x),
                max_z: max_z.max(saved_max_z),
                source: "unionOfSavedBoundsAndOutline",
            };
        }
    }
    Footprint {
        min_x,
        min_z,
        max_x,
        max_z,
        source: "computedOutline",
    }
}

/// Closed-boundary intersection: an entity whose outline merely touches the
/// window edge is kept, because contact is ambiguous at source precision.
pub fn footprint_intersects(footprint: &Footprint, window: WindowRect) -> bool {
    footprint.min_x <= window.max_x_f64()
        && footprint.max_x >= window.min_x_f64()
        && footprint.min_z <= window.max_z_f64()
        && footprint.max_z >= window.min_z_f64()
}

/// Bound rectangle straight from the saved entity, kept verbatim so the source
/// fact survives even when the conservative union is used for selection.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedBounds {
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRecord {
    pub entity_id: String,
    pub asset_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub world_y: i32,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    pub rotation_y_deg: f32,
    /// Saved verbatim; the realised dimensions already contain it.
    pub scale: Option<f64>,
    pub saved_bounds: Option<SavedBounds>,
    pub grounding: serde_json::Value,
    pub anchors: serde_json::Value,
    pub outline: Footprint,
    pub bounds_within_outline: Option<bool>,
    pub inclusion: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadRecord {
    pub road_id: String,
    pub source_entity_present: bool,
    pub source_entity_kind: Option<String>,
    pub plan_edge_present: bool,
    pub from_node: Option<u64>,
    pub to_node: Option<u64>,
    pub from_node_fact: Option<NodeFact>,
    pub to_node_fact: Option<NodeFact>,
    pub world_x: Option<i32>,
    pub world_z: Option<i32>,
    pub length_m: Option<f64>,
    pub rotation_y_deg: Option<f64>,
    pub natural_min_m: Option<f64>,
    pub natural_max_m: Option<f64>,
    pub geometry: Footprint,
    pub geometry_agrees_with_plan: bool,
    pub node_facts_reference_only: bool,
}

/// Plan edges that have no matching scene entity are preserved as reference
/// facts, flagged instead of being dropped or silently treated as roads.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEdgeReference {
    pub id: String,
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub world_x: Option<i32>,
    pub world_z: Option<i32>,
    pub reference_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPointFact {
    pub x: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessRecord {
    pub entity_id: String,
    pub anchor_id: String,
    pub road_id: Option<String>,
    /// Raw saved distance, source units, never rounded away.
    pub distance_m: Option<f64>,
    /// Derived convenience value; not a substitute for the raw fact.
    pub distance_mm_derived: Option<i64>,
    pub connected_in_source: bool,
    pub required_in_source: bool,
    pub status: String,
    pub path: Vec<AccessPointFact>,
    pub in_window: bool,
    /// True when the record refers to something outside the selected window.
    pub reference_only: bool,
    /// The saved flag is copied, never treated as this round's proof.
    pub source_observation_only: bool,
    pub not_reverified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckState {
    pub name: &'static str,
    pub state: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedCounts {
    pub window_samples: u64,
    pub differing_samples: u64,
    pub selected_entities: usize,
    pub roads_in_window: usize,
    pub access_records_total: usize,
    pub access_records_in_window: usize,
    pub reference_only_access_records: usize,
    pub consumed_height_bytes: u64,
    pub consumed_height_files: usize,
    pub skipped_chunks: Vec<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringContext {
    pub schema_version: String,
    pub read_only: bool,
    pub scene_binding: SceneBinding,
    pub window: WindowRect,
    pub source_bindings: Vec<SourceBinding>,
    pub required_file_set: Vec<String>,
    pub height_files: Vec<HeightFileEvidence>,
    pub terrain_comparison: TerrainComparison,
    pub crop: Crop,
    pub entities: Vec<EntityRecord>,
    pub roads: Vec<RoadRecord>,
    pub plan_edge_references: Vec<PlanEdgeReference>,
    pub access_records: Vec<AccessRecord>,
    pub access_scope: &'static str,
    pub entity_kind_counts: BTreeMap<String, usize>,
    pub unmodelled_kinds: BTreeMap<String, usize>,
    pub checks: Vec<CheckState>,
    pub limitations: Vec<String>,
    pub derived: DerivedCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneBinding {
    pub world_path: String,
    pub world_sha256: String,
    pub scene_declaration: String,
    pub scene_path: String,
    pub scene_sha256: String,
    pub declared_scene_id: String,
    pub scene_id: String,
    pub consumed_digest: String,
    pub identity_verified: bool,
    pub strict_scene_parse: bool,
    pub scene_schema_drift: Option<String>,
    pub dropped_unconsumed_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainComparison {
    pub samples: u64,
    pub differing_samples: u64,
    pub natural_equals_final: bool,
}

/// Builds the versioned read-only context from a bound source. Nothing is
/// written to disk, the source identity is re-verified first, and no field
/// claims feasibility, safety or buildability.
pub fn build_context(bound: &BoundScene) -> Result<EngineeringContext, ContextError> {
    bound.verify_identity()?;
    let scene = bound.scene();
    let window = bound.window();
    let crop = bound.crop()?;
    let (edges, nodes) = plan_facts(scene)?;
    validate_road_geometry(scene, window)?;

    let mut selected: Vec<(&EntityInstance, serde_json::Value, Footprint)> = Vec::new();
    for entity in &scene.entities {
        let value = json_of(entity)?;
        let footprint = footprint_of(entity, Some(&value));
        if footprint_intersects(&footprint, window) {
            selected.push((entity, value, footprint));
        }
    }
    if selected.len() > MAX_ENTITY_RECORDS {
        return Err(ContextError::TooManyRecords {
            what: "entity",
            limit: MAX_ENTITY_RECORDS,
            actual: selected.len(),
        });
    }
    if scene.access_graph.links.len() > MAX_ACCESS_RECORDS {
        return Err(ContextError::TooManyRecords {
            what: "access",
            limit: MAX_ACCESS_RECORDS,
            actual: scene.access_graph.links.len(),
        });
    }

    let mut entity_kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    for entity in &scene.entities {
        *entity_kind_counts.entry(entity.kind.clone()).or_insert(0) += 1;
    }
    let mut unmodelled_kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (kind, count) in &entity_kind_counts {
        if kind != "road" {
            unmodelled_kinds.insert(kind.clone(), *count);
        }
    }

    let mut selected_ids: BTreeSet<String> = BTreeSet::new();
    let entities: Vec<EntityRecord> = selected
        .iter()
        .map(|(entity, value, footprint)| {
            selected_ids.insert(entity.entity_id.clone());
            let centre_in_x =
                entity.world_x >= window.min_x && entity.world_x < window.max_x_f64() as i32;
            let centre_in_z =
                entity.world_z >= window.min_z && entity.world_z < window.max_z_f64() as i32;
            let saved_bounds = value.get("bounds").and_then(|bounds| {
                let read = |key: &str| bounds.get(key).and_then(serde_json::Value::as_f64);
                match (read("minX"), read("minZ"), read("maxX"), read("maxZ")) {
                    (Some(min_x), Some(min_z), Some(max_x), Some(max_z)) => Some(SavedBounds {
                        min_x,
                        min_z,
                        max_x,
                        max_z,
                    }),
                    _ => None,
                }
            });
            let bounds_within_outline = saved_bounds.as_ref().map(|saved| {
                footprint.min_x <= saved.min_x
                    && footprint.min_z <= saved.min_z
                    && footprint.max_x >= saved.max_x
                    && footprint.max_z >= saved.max_z
            });
            EntityRecord {
                entity_id: entity.entity_id.clone(),
                asset_id: entity.asset_id.clone(),
                kind: entity.kind.clone(),
                world_x: entity.world_x,
                world_z: entity.world_z,
                world_y: entity.world_y,
                width_m: entity.width_m,
                depth_m: entity.depth_m,
                height_m: entity.height_m,
                rotation_y_deg: entity.rotation_y_deg,
                scale: value.get("scale").and_then(serde_json::Value::as_f64),
                saved_bounds,
                grounding: value
                    .get("grounding")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                anchors: value
                    .get("anchors")
                    .cloned()
                    .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
                outline: footprint.clone(),
                bounds_within_outline,
                inclusion: if centre_in_x && centre_in_z {
                    "centreInsideWindow"
                } else {
                    "outlineEntersWindowWithCentreOutside"
                },
            }
        })
        .collect();

    let mut roads: Vec<RoadRecord> = Vec::new();
    let mut plan_edge_references: Vec<PlanEdgeReference> = Vec::new();
    for (entity, _value, footprint) in selected
        .iter()
        .filter(|(entity, _, _)| entity.kind == "road")
    {
        let edge = edges.get(&entity.entity_id);
        let node_fact = |id: Option<u64>| id.and_then(|id| nodes.get(&id).cloned());
        let geometry_agrees = edge.map_or(false, |edge| {
            edge.world_x == Some(entity.world_x)
                && edge.world_z == Some(entity.world_z)
                && edge.rotation_y_deg == Some(f64::from(entity.rotation_y_deg))
        });
        roads.push(RoadRecord {
            road_id: entity.entity_id.clone(),
            source_entity_present: true,
            source_entity_kind: Some(entity.kind.clone()),
            plan_edge_present: edge.is_some(),
            from_node: edge.and_then(|edge| edge.from),
            to_node: edge.and_then(|edge| edge.to),
            from_node_fact: node_fact(edge.and_then(|edge| edge.from)),
            to_node_fact: node_fact(edge.and_then(|edge| edge.to)),
            world_x: edge.and_then(|edge| edge.world_x),
            world_z: edge.and_then(|edge| edge.world_z),
            length_m: edge.and_then(|edge| edge.length_m),
            rotation_y_deg: edge.and_then(|edge| edge.rotation_y_deg),
            natural_min_m: edge.and_then(|edge| edge.natural_min_m),
            natural_max_m: edge.and_then(|edge| edge.natural_max_m),
            geometry: footprint.clone(),
            geometry_agrees_with_plan: geometry_agrees,
            node_facts_reference_only: true,
        });
    }
    for edge in edges.values() {
        let has_entity = scene
            .entities
            .iter()
            .any(|entity| entity.entity_id == edge.id);
        if !has_entity {
            plan_edge_references.push(PlanEdgeReference {
                id: edge.id.clone(),
                from: edge.from,
                to: edge.to,
                world_x: edge.world_x,
                world_z: edge.world_z,
                reference_only: true,
            });
        }
    }

    let access_records: Vec<AccessRecord> = scene
        .access_graph
        .links
        .iter()
        .map(|link| {
            let in_window = selected_ids.contains(&link.entity_id);
            AccessRecord {
                entity_id: link.entity_id.clone(),
                anchor_id: link.anchor_id.clone(),
                road_id: link.road_id.clone(),
                distance_m: link.distance_m,
                distance_mm_derived: link.distance_m.map(|value| (value * 1000.0).round() as i64),
                connected_in_source: link.connected,
                required_in_source: link.required,
                status: link.status.clone(),
                path: link
                    .path
                    .iter()
                    .map(|point| AccessPointFact {
                        x: point.x,
                        z: point.z,
                    })
                    .collect(),
                in_window,
                reference_only: !in_window,
                source_observation_only: true,
                not_reverified: true,
            }
        })
        .collect();

    let checks = vec![
        CheckState {
            name: "sceneIdentityBound",
            state: "evaluated",
            detail: format!(
                "scene {} read from {} and re-verified against the consumed digest",
                bound.scene().scene_id,
                bound.scene_rel_path()
            ),
        },
        CheckState {
            name: "sourceBytesRecorded",
            state: "diagnostic",
            detail: format!(
                "path/role/bytes/sha256 recorded for {} consumed file(s); recording hashes is not an independent re-verification",
                bound.bindings().len()
            ),
        },
        CheckState {
            name: "terrainComparison",
            state: "diagnostic",
            detail: format!(
                "natural and final {} over the window",
                if crop.natural_equals_final {
                    "agree"
                } else {
                    "differ"
                }
            ),
        },
        CheckState {
            name: "sourceBinding",
            state: "notEvaluated",
            detail:
                "formal audit stage; requires an independent rebuild from the caller-given root"
                    .to_string(),
        },
        CheckState {
            name: "terrainParity",
            state: "notEvaluated",
            detail: "formal audit stage; not yet executed".to_string(),
        },
        CheckState {
            name: "entityAndRoadParity",
            state: "notEvaluated",
            detail: "formal audit stage; not yet executed".to_string(),
        },
        CheckState {
            name: "contextCompleteness",
            state: "notEvaluated",
            detail: "formal audit stage; not yet executed".to_string(),
        },
        CheckState {
            name: "accessRouteReverification",
            state: "notEvaluated",
            detail: "the saved connected flag is copied as an observation only".to_string(),
        },
        CheckState {
            name: "engineeringFeasibility",
            state: "notEvaluated",
            detail: "no EngineeringInput is built, no stair, no re-route".to_string(),
        },
    ];

    let mut limitations = vec![
        "saved access flags are source observations, not re-verified routes".to_string(),
        "no EngineeringInput is built and no route is re-computed".to_string(),
        "the saved scene index is the authority for entity and road identity".to_string(),
        "no water, protection or collider role is converted; see unmodelledKinds".to_string(),
        format!(
            "height payloads for {} non-intersecting chunk(s) were never opened",
            bound.skipped_chunks().len()
        ),
    ];
    if let Some(error) = &bound.load.strict_error {
        limitations.push(format!(
            "the saved scene predates the current SceneIndex schema ({error}); unconsumed subtrees {:?} were dropped and are not verified",
            bound.load.dropped_keys
        ));
    }

    let derived = DerivedCounts {
        window_samples: crop.final_mm.len() as u64,
        differing_samples: crop.differing_samples,
        selected_entities: entities.len(),
        roads_in_window: roads.len(),
        access_records_total: access_records.len(),
        access_records_in_window: access_records
            .iter()
            .filter(|record| record.in_window)
            .count(),
        reference_only_access_records: access_records
            .iter()
            .filter(|record| record.reference_only)
            .count(),
        consumed_height_bytes: bound.total_height_bytes(),
        consumed_height_files: bound.chunk_evidences().len(),
        skipped_chunks: bound.skipped_chunks().to_vec(),
    };

    let scene_binding = SceneBinding {
        world_path: bound.world_path.clone(),
        world_sha256: bound
            .bindings()
            .iter()
            .find(|binding| binding.role == ROLE_WORLD_INDEX)
            .map(|binding| binding.sha256.clone())
            .unwrap_or_default(),
        scene_declaration: bound.scene_declaration().to_string(),
        scene_path: bound.scene_rel_path().to_string(),
        scene_sha256: bound
            .bindings()
            .iter()
            .find(|binding| binding.role == ROLE_SCENE_INDEX)
            .map(|binding| binding.sha256.clone())
            .unwrap_or_default(),
        declared_scene_id: bound.declared_scene_id().to_string(),
        scene_id: scene.scene_id.clone(),
        consumed_digest: consumed_digest(scene)?,
        identity_verified: true,
        strict_scene_parse: bound.load.strict,
        scene_schema_drift: bound.load.strict_error.clone(),
        dropped_unconsumed_keys: bound.load.dropped_keys.clone(),
    };

    Ok(EngineeringContext {
        schema_version: SCHEMA_VERSION.to_string(),
        read_only: true,
        scene_binding,
        window,
        source_bindings: bound.bindings().to_vec(),
        required_file_set: bound.required_file_set()?,
        height_files: bound.chunk_evidences(),
        terrain_comparison: TerrainComparison {
            samples: crop.final_mm.len() as u64,
            differing_samples: crop.differing_samples,
            natural_equals_final: crop.natural_equals_final,
        },
        crop,
        entities,
        roads,
        plan_edge_references,
        access_records,
        access_scope: "sceneWideWithReferenceFlags",
        entity_kind_counts,
        unmodelled_kinds,
        checks,
        limitations,
        derived,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED_WINDOW: &str = "256,32,64,64";
    const NAMED_ROAD: &str = "generated.road-network.scene-0.4-11";

    fn millimetre_payload(width: u32, depth: u32, fill: impl Fn(u32, u32) -> i32) -> Vec<u8> {
        let mut metres = Vec::new();
        for z in 0..depth {
            for x in 0..width {
                metres.push(f64::from(fill(x, z)) / 1000.0);
            }
        }
        crate::height_payload::encode_millimetres(width, depth, metres).expect("payload encodes")
    }

    fn quarter_payload(width: u32, depth: u32, fill: impl Fn(u32, u32) -> i16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity((width * depth) as usize * 2);
        for z in 0..depth {
            for x in 0..width {
                bytes.extend_from_slice(&fill(x, z).to_le_bytes());
            }
        }
        bytes
    }

    fn natural_for(width: u32, depth: u32, shift: i32) -> DecodedHeight {
        DecodedHeight::decode(
            "chunk.natural.height.bin",
            ROLE_NATURAL_HEIGHT,
            &millimetre_payload(width, depth, |x, z| (x as i32) * 1000 + z as i32 + shift),
            width,
            depth,
        )
        .expect("natural payload decodes")
    }

    fn chunk(
        chunk_x: i32,
        chunk_z: i32,
        world_x: i32,
        world_z: i32,
        width: u32,
        depth: u32,
        natural: Option<DecodedHeight>,
    ) -> ChunkSource {
        let final_height = DecodedHeight::decode(
            &format!("chunk-{chunk_x}-{chunk_z}.height.bin"),
            ROLE_FINAL_HEIGHT,
            &millimetre_payload(width, depth, |x, z| (x as i32) * 1000 + z as i32),
            width,
            depth,
        )
        .expect("final payload decodes");
        ChunkSource::new(
            chunk_x,
            chunk_z,
            world_x,
            world_z,
            width,
            depth,
            final_height,
            natural.unwrap_or_else(|| natural_for(width, depth, 0)),
        )
    }

    fn fixed_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../bevy/work/joint-r27-verified-baseline")
    }

    fn fixed_bound(window: &str) -> BoundScene {
        BoundScene::read(
            &fixed_dir(),
            WindowRect::parse(window).expect("window parses"),
        )
        .expect("fixed source binds")
    }

    fn fixed_scene() -> SceneIndex {
        fixed_bound(FIXED_WINDOW).scene().clone()
    }

    fn scene_with(mutate: impl FnOnce(&mut serde_json::Value)) -> SceneIndex {
        let scene = fixed_scene();
        let mut value = serde_json::to_value(&scene).expect("scene to value");
        mutate(&mut value);
        serde_json::from_value(value).expect("mutated scene parses")
    }

    /// A temporary directory outside the workspace and outside every frozen
    /// directory, used only for counterexample fixtures.
    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("gwc-{}-{}-{}", std::process::id(), tag, nanos));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    // ---- identity binding -------------------------------------------------

    fn strict_read(bytes: &[u8]) -> String {
        serde_json::from_slice::<SceneIndex>(bytes)
            .expect_err("the current schema must refuse the older saved scene")
            .to_string()
    }

    #[test]
    fn strict_read_of_the_fixed_scene_reports_the_access_method_drift() {
        let bytes = fs::read(fixed_dir().join("scenes/scene-0/scene.json")).expect("readable");
        let text = strict_read(&bytes);
        assert!(text.contains("accessMethod"), "got: {text}");
        assert!(!text.contains("expected usize"), "got: {text}");
    }

    #[test]
    fn compatibility_read_retains_the_consumed_subtrees() {
        let bytes = fs::read(fixed_dir().join("scenes/scene-0/scene.json")).expect("readable");
        let load = load_scene_index(&bytes).expect("compatibility read");
        assert!(!load.strict);
        assert!(
            load.strict_error
                .as_deref()
                .unwrap_or_default()
                .contains("accessMethod")
        );
        assert!(load.dropped_keys.contains(&"landcoverPlan".to_string()));
        assert_eq!(load.scene.scene_id, "scene-0");
        assert_eq!(load.scene.entities.len(), 51);
        assert_eq!(load.scene.access_graph.links.len(), 18);
        assert_eq!(load.scene.chunks.len(), 1);
    }

    /// A dropped subtree must never be read as a proven-empty safety set.
    #[test]
    fn omitted_planning_subtrees_are_not_reported_as_verified() {
        let context = build_context(&fixed_bound(FIXED_WINDOW)).expect("context builds");
        assert!(!context.scene_binding.strict_scene_parse);
        assert!(context.scene_binding.scene_schema_drift.is_some());
        assert!(
            context
                .scene_binding
                .dropped_unconsumed_keys
                .contains(&"landcoverPlan".to_string())
        );
        assert!(
            context
                .limitations
                .iter()
                .any(|line| line.contains("are not verified"))
        );
        assert!(
            !context
                .limitations
                .iter()
                .any(|line| line.contains("proven empty"))
        );
    }

    /// If a consumed subtree is illegal, the compatibility projection must
    /// refuse it instead of widening the skip list.
    #[test]
    fn illegal_consumed_subtree_is_refused() {
        let scene = fixed_scene();
        let mut value = serde_json::to_value(&scene).expect("to value");
        let object = value.as_object_mut().expect("object");
        assert!(object.remove("entities").is_some(), "entities is consumed");
        value["naturalTerrain"] = serde_json::json!({});
        let bytes = serde_json::to_vec(&value).expect("encode");
        let error = load_scene_index(&bytes).expect_err("must be refused");
        assert!(
            matches!(error, ContextError::SceneUnreadable { .. }),
            "{error}"
        );
    }

    /// The historical integer error is reproduced, and the message proves it
    /// carries no field path: both `usize` counters yield the same text.
    #[test]
    fn map_where_usize_expected_carries_no_field_path() {
        let base = serde_json::to_value(fixed_scene()).expect("to value");
        let mut with_connected = base.clone();
        with_connected["accessGraph"]["connectedEntities"] = serde_json::json!({});
        let first = serde_json::from_value::<SceneIndex>(with_connected)
            .expect_err("must be refused")
            .to_string();
        let mut with_disconnected = base;
        with_disconnected["accessGraph"]["disconnectedEntities"] = serde_json::json!({});
        let second = serde_json::from_value::<SceneIndex>(with_disconnected)
            .expect_err("must be refused")
            .to_string();
        assert_eq!(first, "invalid type: map, expected usize");
        assert_eq!(first, second, "the message cannot localise the field");
    }

    #[test]
    fn empty_natural_terrain_object_yields_missing_field_not_usize() {
        let mut value = serde_json::to_value(fixed_scene()).expect("to value");
        value["naturalTerrain"] = serde_json::json!({});
        let text = serde_json::from_value::<SceneIndex>(value)
            .expect_err("must be refused")
            .to_string();
        assert!(text.contains("missing field"), "got: {text}");
        assert!(!text.contains("expected usize"), "got: {text}");
    }

    /// Counterexample P1-1a: keeping the original bytes and hashes while
    /// editing the in-memory scene identity must not yield a same-source claim.
    #[test]
    fn in_memory_scene_identity_change_is_refused() {
        let mut bound = fixed_bound(FIXED_WINDOW);
        let before = bound.bindings().to_vec();
        bound.load.scene.scene_id = "unbound-scene".to_string();
        let error = build_context(&bound).expect_err("identity edit must be refused");
        assert!(
            matches!(error, ContextError::SceneIdentityMismatch { .. }),
            "{error}"
        );
        assert_eq!(before, bound.bindings(), "bindings are untouched bytes");
    }

    /// Counterexample P1-1b: editing consumed geometry in memory is caught by
    /// the consumed digest even though the bytes and hashes are unchanged.
    #[test]
    fn in_memory_consumed_geometry_change_is_refused() {
        let mut bound = fixed_bound(FIXED_WINDOW);
        bound.load.scene.entities[0].world_x += 1;
        let error = build_context(&bound).expect_err("geometry edit must be refused");
        assert!(
            matches!(error, ContextError::ConsumedGeometryMutated { .. }),
            "{error}"
        );
    }

    /// The scene path is taken from world.json's own scene list, and the scene
    /// file's identity must agree with the declared directory.
    #[test]
    fn scene_path_comes_from_the_world_declaration_and_identity_is_checked() {
        let bound = fixed_bound(FIXED_WINDOW);
        assert_eq!(bound.scene_declaration(), "scenes/scene-0/scene.json");
        assert_eq!(bound.scene_rel_path(), "scenes/scene-0/scene.json");
        assert_eq!(bound.declared_scene_id(), "scene-0");
        assert_eq!(bound.scene().scene_id, "scene-0");

        let dir = temp_dir("identity");
        fs::create_dir_all(dir.join("scenes/scene-9")).expect("scene dir");
        fs::write(
            dir.join("world.json"),
            br#"{"format":"glyphweave-world","version":1,"scenes":["scenes/scene-9/scene.json"]}"#,
        )
        .expect("world write");
        let scene = fixed_scene();
        fs::write(
            dir.join("scenes/scene-9/scene.json"),
            serde_json::to_vec(&scene).expect("encode"),
        )
        .expect("scene write");
        let error = BoundScene::read(&dir, WindowRect::parse(FIXED_WINDOW).expect("window"))
            .expect_err("mismatched identity must be refused");
        assert!(
            matches!(error, ContextError::SceneIdentityMismatch { .. }),
            "{error}"
        );
    }

    #[test]
    fn world_declaring_several_scenes_is_unsupported() {
        let dir = temp_dir("multi");
        fs::write(
            dir.join("world.json"),
            br#"{"scenes":["scenes/scene-0/scene.json","scenes/scene-1/scene.json"]}"#,
        )
        .expect("world write");
        let error = BoundScene::read(&dir, WindowRect::parse("0,0,4,4").expect("window"))
            .expect_err("must be refused");
        assert!(
            matches!(error, ContextError::UnsupportedSource { .. }),
            "{error}"
        );
    }

    // ---- window-first bounded reading ------------------------------------

    /// A window that touches only one chunk must not require the far chunk's
    /// payloads to exist.
    #[test]
    fn far_chunk_payloads_are_not_read_and_may_be_absent() {
        let dir = temp_dir("far");
        fs::create_dir_all(dir.join("scenes/scene-0")).expect("scene dir");
        fs::write(
            dir.join("world.json"),
            br#"{"scenes":["scenes/scene-0/scene.json"]}"#,
        )
        .expect("world write");
        let mut scene = fixed_scene();
        scene.width_m = 4;
        scene.depth_m = 2;
        scene.chunk_size_m = 2;
        scene.chunk_count_x = 2;
        scene.chunk_count_z = 1;
        scene.origin_x = 0;
        scene.origin_z = 0;
        let mut near = scene.chunks[0].clone();
        near.chunk_x = 0;
        near.chunk_z = 0;
        near.world_x = 0;
        near.world_z = 0;
        near.valid_width_m = 2;
        near.valid_depth_m = 2;
        near.height_file = "chunk-0-0.height.bin".to_string();
        near.natural_height_file = Some("chunk-0-0.natural.height.bin".to_string());
        let mut far = near.clone();
        far.chunk_x = 1;
        far.world_x = 2;
        far.height_file = "chunk-1-0.height.bin".to_string();
        far.natural_height_file = Some("chunk-1-0.natural.height.bin".to_string());
        scene.chunks = vec![near, far];
        fs::write(
            dir.join("scenes/scene-0/scene.json"),
            serde_json::to_vec(&scene).expect("encode"),
        )
        .expect("scene write");
        fs::write(
            dir.join("scenes/scene-0/chunk-0-0.height.bin"),
            millimetre_payload(2, 2, |x, z| (x as i32) * 1000 + z as i32),
        )
        .expect("final write");
        fs::write(
            dir.join("scenes/scene-0/chunk-0-0.natural.height.bin"),
            millimetre_payload(2, 2, |x, z| (x as i32) * 1000 + z as i32),
        )
        .expect("natural write");

        let bound = BoundScene::read(&dir, WindowRect::parse("0,0,2,2").expect("window"))
            .expect("the local window must succeed without the far payloads");
        assert_eq!(bound.skipped_chunks(), &[(1, 0)]);
        assert_eq!(bound.chunk_evidences().len(), 2);
        assert_eq!(
            bound.bindings().len(),
            4,
            "world, scene and two local payloads"
        );
        let context = build_context(&bound).expect("context builds");
        assert_eq!(context.derived.window_samples, 4);
        assert_eq!(context.derived.skipped_chunks, vec![(1, 0)]);

        // A window over the far chunk must still refuse the missing payload.
        let error = BoundScene::read(&dir, WindowRect::parse("2,0,2,2").expect("window"))
            .expect_err("the far payload is genuinely required here");
        assert!(matches!(error, ContextError::SourceRead { .. }), "{error}");

        // A descriptor that names no natural payload must be refused, never
        // filled in from the final raster or a guessed file name.
        let mut without_natural = scene.clone();
        without_natural.chunks[0].natural_height_file = None;
        fs::write(
            dir.join("scenes/scene-0/scene.json"),
            serde_json::to_vec(&without_natural).expect("encode"),
        )
        .expect("scene rewrite");
        let error = BoundScene::read(&dir, WindowRect::parse("0,0,2,2").expect("window"))
            .expect_err("a missing natural declaration must be refused");
        assert!(
            matches!(error, ContextError::MissingNaturalHeight { .. }),
            "{error}"
        );
    }

    #[test]
    fn required_file_set_comes_from_the_actual_scene_path_and_names() {
        let bound = fixed_bound(FIXED_WINDOW);
        let required = bound.required_file_set().expect("required set");
        assert!(required.contains(&"world.json".to_string()));
        assert!(required.contains(&"scenes/scene-0/scene.json".to_string()));
        assert!(required.contains(&"scenes/scene-0/chunk-0-0.height.bin".to_string()));
        assert!(required.contains(&"scenes/scene-0/chunk-0-0.natural.height.bin".to_string()));
        assert_eq!(required.len(), 4);
        bound
            .verify_identity()
            .expect("bindings cover the required set");
    }

    #[test]
    fn path_confinement_refuses_traversal_absolute_and_missing_targets() {
        let dir = temp_dir("confine");
        assert!(resolve_within_root(&dir, "../outside.bin").is_err());
        assert!(resolve_within_root(&dir, "C:/Windows/win.ini").is_err());
        assert!(resolve_within_root(&dir, "scenes/scene-0/scene.json").is_err());
    }

    // ---- authoritative decoder -------------------------------------------

    #[test]
    fn decoder_delegates_to_the_authoritative_parser() {
        let bytes = quarter_payload(3, 2, |x, z| (x as i16) * 4 + z as i16 - 2);
        let mine = DecodedHeight::decode("legacy.height.bin", ROLE_FINAL_HEIGHT, &bytes, 3, 2)
            .expect("decodes");
        let authoritative = HeightPayload::parse(&bytes, 3, 2).expect("frozen parser accepts");
        assert_eq!(mine.encoding, HeightEncoding::QuarterMetre);
        for index in 0..authoritative.count {
            assert_eq!(
                mine.millimetres[index],
                authoritative.raw(index) * 250,
                "legacy conversion must be exactly lossless"
            );
        }
        let millimetre = millimetre_payload(4, 3, |x, z| (z as i32) * 100 - x as i32);
        let decoded =
            DecodedHeight::decode("probe.height.bin", ROLE_FINAL_HEIGHT, &millimetre, 4, 3)
                .expect("decodes");
        let authoritative = HeightPayload::parse(&millimetre, 4, 3).expect("frozen parser accepts");
        assert_eq!(decoded.encoding, HeightEncoding::Millimetre);
        for index in 0..authoritative.count {
            assert_eq!(decoded.millimetres[index], authoritative.raw(index));
        }
    }

    /// The inspector's 513 x 1 legacy payload is legal-length but exceeds the
    /// authoritative 512 cap, so it must be refused here as well.
    #[test]
    fn invalid_dimensions_are_refused_like_the_authoritative_parser() {
        let bytes = quarter_payload(513, 1, |_, _| 1);
        assert!(HeightPayload::parse(&bytes, 513, 1).is_err());
        let error = DecodedHeight::decode("513.height.bin", ROLE_FINAL_HEIGHT, &bytes, 513, 1)
            .expect_err("must be refused");
        assert!(
            matches!(error, ContextError::HeightRejected { .. }),
            "{error}"
        );
    }

    #[test]
    fn bad_header_and_bad_length_are_refused() {
        let mut bytes = millimetre_payload(2, 2, |_, _| 0);
        bytes[4..8].copy_from_slice(&7u32.to_le_bytes());
        assert!(matches!(
            DecodedHeight::decode("v7.height.bin", ROLE_FINAL_HEIGHT, &bytes, 2, 2)
                .expect_err("bad version"),
            ContextError::HeightRejected { .. }
        ));
        let mut bytes = millimetre_payload(4, 4, |_, _| 0);
        bytes.truncate(bytes.len() - 8);
        assert!(matches!(
            DecodedHeight::decode("cut.height.bin", ROLE_FINAL_HEIGHT, &bytes, 4, 4)
                .expect_err("bad length"),
            ContextError::HeightRejected { .. }
        ));
    }

    #[test]
    fn index_out_of_range_is_refused_rather_than_aliased() {
        let decoded = DecodedHeight::decode(
            "tiny.height.bin",
            ROLE_FINAL_HEIGHT,
            &millimetre_payload(2, 2, |_, _| 5),
            2,
            2,
        )
        .expect("decodes");
        assert!(matches!(
            decoded.millimetres_at(2, 0).expect_err("out of range"),
            ContextError::IndexOutOfRange {
                index: 2,
                count: 4,
                ..
            }
        ));
    }

    // ---- crop ------------------------------------------------------------

    #[test]
    fn crop_keeps_final_and_natural_separate() {
        let chunks = vec![chunk(0, 0, 0, 0, 2, 2, Some(natural_for(2, 2, 500)))];
        let crop = crop_chunks(&chunks, WindowRect::parse("0,0,2,2").expect("window"))
            .expect("crop succeeds");
        assert_eq!(crop.final_mm.len(), 4);
        assert!(!crop.natural_equals_final);
        assert_eq!(crop.final_mm[3], 1001);
        assert_eq!(crop.natural_mm[3], 1501);
    }

    #[test]
    fn crop_cross_chunk_window_uses_every_covering_chunk() {
        let chunks = vec![
            chunk(0, 0, 0, 0, 2, 2, Some(natural_for(2, 2, 0))),
            chunk(1, 0, 2, 0, 2, 2, Some(natural_for(2, 2, 7000))),
        ];
        let crop = crop_chunks(&chunks, WindowRect::parse("1,0,2,1").expect("window"))
            .expect("crop spans the boundary");
        assert_eq!(crop.covering_chunks, vec![(0, 0), (1, 0)]);
        assert_eq!(crop.samples[0].local_x, 1);
        assert_eq!(crop.samples[1].chunk_x, 1);
        assert_eq!(crop.final_mm[1], 0);
        assert_eq!(crop.natural_mm[1], 7000);
    }

    #[test]
    fn crop_handles_non_zero_and_negative_origins() {
        let chunks = vec![chunk(0, 0, -64, -32, 4, 4, Some(natural_for(4, 4, 0)))];
        let crop = crop_chunks(&chunks, WindowRect::parse("-63,-31,2,2").expect("window"))
            .expect("crop succeeds");
        assert_eq!(crop.samples[0].local_x, 1);
        assert_eq!(crop.final_mm[0], 1001);
        assert_eq!(crop.samples[3].local_z, 2);
        assert_eq!(crop.final_mm[3], 2002);
    }

    #[test]
    fn crop_refuses_missing_and_overlapping_coverage() {
        let single = vec![chunk(0, 0, 0, 0, 2, 2, None)];
        assert!(matches!(
            crop_chunks(&single, WindowRect::parse("1,1,2,2").expect("window"))
                .expect_err("no covering chunk"),
            ContextError::MissingChunk { .. }
        ));
        let double = vec![chunk(0, 0, 0, 0, 2, 2, None), chunk(1, 1, 0, 0, 2, 2, None)];
        assert!(matches!(
            crop_chunks(&double, WindowRect::parse("0,0,1,1").expect("window"))
                .expect_err("overlap"),
            ContextError::OverlappingCoverage { .. }
        ));
    }

    #[test]
    fn crop_checks_shape_and_arithmetic_itself() {
        let chunks = vec![chunk(0, 0, 0, 0, 2, 2, None)];
        assert!(matches!(
            crop_chunks(&chunks, WindowRect::parse("0,0,0,4").expect("window"))
                .expect_err("empty window"),
            ContextError::WindowEmpty
        ));
        assert!(matches!(
            crop_chunks(&chunks, WindowRect::parse("0,0,999999,1").expect("window"))
                .expect_err("oversized window"),
            ContextError::WindowTooLarge { .. }
        ));
    }

    #[test]
    fn chunk_extent_contradiction_is_refused() {
        let mut chunks = vec![chunk(0, 0, 0, 0, 2, 2, None)];
        chunks[0].valid_width_m = 3;
        assert!(matches!(
            crop_chunks(&chunks, WindowRect::parse("0,0,2,2").expect("window"))
                .expect_err("must be refused"),
            ContextError::CoverageContradiction { .. }
        ));
    }

    // ---- entity selection and preserved facts ----------------------------

    #[test]
    fn window_limits_and_bounds_are_enforced() {
        let scene = fixed_scene();
        assert!(matches!(
            WindowRect::parse("0,0,129,4")
                .expect("window")
                .validate(&scene)
                .expect_err("too wide"),
            ContextError::WindowTooLarge { .. }
        ));
        assert!(matches!(
            WindowRect::parse("500,0,64,4")
                .expect("window")
                .validate(&scene)
                .expect_err("outside"),
            ContextError::WindowOutsideScene { .. }
        ));
        assert!(matches!(
            WindowRect::parse("0,0,4").expect_err("malformed"),
            ContextError::WindowMalformed(_)
        ));
    }

    /// P1-2 counterexample 1: the saved bounds are smaller than the real
    /// outline, so the entity must still be selected (union, not blind trust).
    #[test]
    fn saved_bounds_smaller_than_the_outline_do_not_hide_an_entity() {
        let entity = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            entities.clear();
            entities.push(serde_json::json!({
                "entityId": "probe.wall",
                "assetId": "prop.wall",
                "kind": "residential_home",
                "worldX": 254,
                "worldZ": 64,
                "worldY": 0,
                "scale": 1.0,
                "widthM": 8.0,
                "depthM": 8.0,
                "heightM": 3.0,
                "rotationYDeg": 0.0,
                "grounding": {"pivot": "bottom", "bottomOffsetM": 0.0, "roadbedOffsetM": 0.0, "toleranceM": 1.0},
                "anchors": [],
                "bounds": {"minX": 253.0, "minZ": 63.0, "maxX": 255.0, "maxZ": 65.0}
            }));
        });
        let window = WindowRect::parse(FIXED_WINDOW).expect("window");
        let entity = &entity.entities[0];
        let footprint = footprint_of(entity, Some(&json_of(entity).expect("to value")));
        assert!(
            footprint_intersects(&footprint, window),
            "the true outline reaches x=258 inside the window"
        );
        assert_eq!(footprint.source, "unionOfSavedBoundsAndOutline");
        assert_eq!(footprint.max_x, 258.0, "union keeps the true outline");
        assert_eq!(footprint.min_x, 250.0, "the true outline also widens min x");
    }

    /// P1-2 counterexample 2: an outline that only touches the window edge.
    #[test]
    fn outline_touching_the_window_boundary_is_kept() {
        let entity = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            entities.clear();
            entities.push(serde_json::json!({
                "entityId": "probe.touch",
                "assetId": "prop.wall",
                "kind": "residential_home",
                "worldX": 254,
                "worldZ": 64,
                "worldY": 0,
                "scale": 1.0,
                "widthM": 4.0,
                "depthM": 4.0,
                "heightM": 3.0,
                "rotationYDeg": 0.0,
                "grounding": {"pivot": "bottom", "bottomOffsetM": 0.0, "roadbedOffsetM": 0.0, "toleranceM": 1.0},
                "anchors": []
            }));
        });
        let entity = &entity.entities[0];
        let footprint = footprint_of(entity, None);
        assert_eq!(footprint.max_x, 256.0, "the outline ends exactly on x=256");
        assert_eq!(footprint.source, "computedOutline");
        assert!(
            footprint_intersects(&footprint, WindowRect::parse(FIXED_WINDOW).expect("window")),
            "closed-boundary contact must be kept"
        );
    }

    /// P1-2 counterexample 3: rotated outline in the window, centre outside.
    #[test]
    fn rotated_outline_with_centre_outside_the_window_is_kept() {
        let entity = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            entities.clear();
            entities.push(serde_json::json!({
                "entityId": "probe.rotated",
                "assetId": "prop.wall",
                "kind": "residential_home",
                "worldX": 250,
                "worldZ": 64,
                "worldY": 0,
                "scale": 1.0,
                "widthM": 4.0,
                "depthM": 40.0,
                "heightM": 3.0,
                "rotationYDeg": 90.0,
                "grounding": {"pivot": "bottom", "bottomOffsetM": 0.0, "roadbedOffsetM": 0.0, "toleranceM": 1.0},
                "anchors": []
            }));
        });
        let entity = &entity.entities[0];
        let footprint = footprint_of(entity, None);
        assert!(entity.world_x < 256, "the centre is outside the window");
        assert!(
            footprint.max_x > 256.0,
            "but the rotated outline reaches in"
        );
        assert!(footprint_intersects(
            &footprint,
            WindowRect::parse(FIXED_WINDOW).expect("window")
        ));
    }

    #[test]
    fn fixed_window_context_reports_the_named_road() {
        let context = build_context(&fixed_bound(FIXED_WINDOW)).expect("context builds");
        assert!(context.read_only);
        assert_eq!(context.window.min_x, 256);
        assert_eq!(context.derived.window_samples, 4096);
        let named = context
            .roads
            .iter()
            .find(|road| road.road_id == NAMED_ROAD)
            .expect("the named road is inside the fixed window");
        assert!(named.plan_edge_present);
        assert!(named.geometry_agrees_with_plan);
        assert_eq!(named.from_node, Some(4));
        assert_eq!(named.to_node, Some(11));
        let from = named.from_node_fact.as_ref().expect("node fact kept");
        assert_eq!(from.id, 4);
        assert_eq!(from.world_x, Some(288));
        assert_eq!(from.world_z, Some(32));
        assert!(
            context
                .checks
                .iter()
                .any(|check| check.name == "contextCompleteness" && check.state == "notEvaluated")
        );
        assert!(
            context
                .checks
                .iter()
                .any(|check| check.name == "terrainComparison" && check.state == "diagnostic")
        );
        assert!(
            context
                .checks
                .iter()
                .any(|check| check.name == "sourceBytesRecorded" && check.state == "diagnostic")
        );
    }

    /// P1-2 counterexample 4: full entity and access facts survive, and
    /// out-of-window access records are flagged rather than silently included.
    #[test]
    fn entity_and_access_facts_are_preserved_field_by_field() {
        let context = build_context(&fixed_bound(FIXED_WINDOW)).expect("context builds");
        let named = context
            .entities
            .iter()
            .find(|entity| entity.entity_id == NAMED_ROAD)
            .expect("the named road entity is selected");
        assert_eq!(named.kind, "road");
        assert_eq!(named.scale, Some(1.0));
        assert_eq!(named.width_m, 64.0);
        assert_eq!(named.depth_m, 8.0);
        assert_eq!(named.rotation_y_deg, 90.0);
        let saved = named.saved_bounds.as_ref().expect("saved bounds kept");
        assert_eq!((saved.min_x, saved.min_z), (284.0, 32.0));
        assert_eq!((saved.max_x, saved.max_z), (292.0, 96.0));
        assert_eq!(named.bounds_within_outline, Some(true));
        assert_eq!(named.grounding["pivot"], serde_json::json!("bottom"));
        assert_eq!(named.grounding["toleranceM"], serde_json::json!(1.0));
        assert!(named.anchors.is_array());

        assert_eq!(context.access_scope, "sceneWideWithReferenceFlags");
        assert_eq!(context.access_records.len(), 18);
        let roadside = context
            .access_records
            .iter()
            .find(|record| record.road_id.as_deref() == Some(NAMED_ROAD))
            .expect("the roadside access record is present");
        assert_eq!(roadside.anchor_id, "front");
        assert_eq!(roadside.distance_m, Some(8.0));
        assert_eq!(roadside.distance_mm_derived, Some(8000));
        assert_eq!(roadside.status, "walkable_route");
        assert_eq!(roadside.path.len(), 3, "the saved polyline is kept");
        assert_eq!(roadside.path[0].x, 276.0);
        assert_eq!(roadside.path[2].x, 284.0);
        assert!(roadside.source_observation_only && roadside.not_reverified);
        assert!(
            context
                .access_records
                .iter()
                .any(|record| record.reference_only),
            "records outside the window are flagged referenceOnly"
        );
        assert_eq!(
            context.derived.reference_only_access_records,
            context
                .access_records
                .iter()
                .filter(|record| record.reference_only)
                .count()
        );
    }

    /// The realised dimensions already carry the asset scale, so scale must be
    /// preserved instead of applied a second time.
    #[test]
    fn scale_is_preserved_verbatim_and_not_applied_twice() {
        let entity: EntityInstance = serde_json::from_value(serde_json::json!({
            "entityId": "probe.scaled",
            "assetId": "prop.wall",
            "kind": "residential_home",
            "worldX": 0,
            "worldZ": 0,
            "worldY": 0,
            "scale": 2.0,
            "widthM": 4.0,
            "depthM": 4.0,
            "heightM": 3.0,
            "rotationYDeg": 0.0,
            "grounding": {"pivot": "bottom", "bottomOffsetM": 0.0, "roadbedOffsetM": 0.0, "toleranceM": 1.0},
            "anchors": []
        }))
        .expect("entity parses");
        let footprint = footprint_of(&entity, None);
        assert_eq!(footprint.max_x, 2.0, "realised half extent is widthM/2");
        assert_eq!(footprint.min_x, -2.0);
        let value = json_of(&entity).expect("to value");
        assert_eq!(value["scale"], serde_json::json!(2.0));
        assert_eq!(
            footprint.max_x - footprint.min_x,
            4.0,
            "applying scale again would give 8 m and is wrong"
        );
    }

    // ---- road relations --------------------------------------------------

    #[test]
    fn road_edges_without_any_node_table_are_refused() {
        let scene = scene_with(|value| {
            value["roadNetworkPlan"]["nodes"] = serde_json::json!([]);
        });
        let error = plan_facts(&scene).expect_err("endpoint check must still run");
        assert!(
            matches!(error, ContextError::UnknownRoadNode { .. }),
            "{error}"
        );
    }

    #[test]
    fn duplicate_nodes_edges_and_entities_are_refused() {
        let duplicate_node = scene_with(|value| {
            let nodes = value["roadNetworkPlan"]["nodes"]
                .as_array_mut()
                .expect("nodes");
            let copy = nodes[0].clone();
            nodes.push(copy);
        });
        assert!(matches!(
            plan_facts(&duplicate_node).expect_err("duplicate node"),
            ContextError::DuplicateNodeIdentity { .. }
        ));
        let duplicate_edge = scene_with(|value| {
            let edges = value["roadNetworkPlan"]["edges"]
                .as_array_mut()
                .expect("edges");
            let copy = edges[0].clone();
            edges.push(copy);
        });
        assert!(matches!(
            plan_facts(&duplicate_edge).expect_err("duplicate edge"),
            ContextError::DuplicateRoadIdentity { .. }
        ));
        let duplicate_entity = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            let copy = entities[0].clone();
            entities.push(copy);
        });
        assert!(matches!(
            validate_scene_identities(&duplicate_entity).expect_err("duplicate entity"),
            ContextError::DuplicateEntityIdentity { .. }
        ));
    }

    #[test]
    fn road_entity_without_a_plan_edge_is_an_inconsistent_source() {
        let scene = scene_with(|value| {
            let edges = value["roadNetworkPlan"]["edges"]
                .as_array_mut()
                .expect("edges");
            edges.retain(|edge| edge["id"] != serde_json::json!(NAMED_ROAD));
        });
        let error =
            validate_road_geometry(&scene, WindowRect::parse(FIXED_WINDOW).expect("window"))
                .expect_err("a named road without its plan edge must not pass");
        assert!(
            matches!(error, ContextError::InconsistentSource { .. }),
            "{error}"
        );
    }

    #[test]
    fn road_geometry_contradicting_the_plan_is_refused() {
        let scene = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            for entity in entities.iter_mut() {
                if entity["entityId"] == serde_json::json!(NAMED_ROAD) {
                    entity["worldX"] = serde_json::json!(310);
                    entity["rotationYDeg"] = serde_json::json!(0.0);
                }
            }
        });
        let error =
            validate_road_geometry(&scene, WindowRect::parse(FIXED_WINDOW).expect("window"))
                .expect_err("contradicting geometry must be refused");
        assert!(
            matches!(error, ContextError::InconsistentSource { .. }),
            "{error}"
        );
    }

    #[test]
    fn plan_edges_without_an_entity_are_kept_as_reference_only() {
        let scene = scene_with(|value| {
            let entities = value["entities"].as_array_mut().expect("entities");
            entities.retain(|entity| entity["entityId"] != serde_json::json!(NAMED_ROAD));
        });
        let (edges, _nodes) = plan_facts(&scene).expect("plan still reads");
        assert!(edges.contains_key(NAMED_ROAD));
    }

    #[test]
    fn full_scene_entity_and_road_totals_are_unchanged() {
        let context = build_context(&fixed_bound(FIXED_WINDOW)).expect("context builds");
        assert_eq!(context.entity_kind_counts["road"], 29);
        assert_eq!(context.entity_kind_counts["residential_home"], 18);
        assert_eq!(context.entities.len(), context.derived.selected_entities);
        assert!(context.plan_edge_references.is_empty());
        assert!(context.unmodelled_kinds.contains_key("residential_home"));
    }
}
