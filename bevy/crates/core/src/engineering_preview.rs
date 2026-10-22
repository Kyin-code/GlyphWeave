//! B01-G23: a read-only 3D preview consumer for one verified engineering package.
//!
//! Single consumer, one data flow, nothing re-synthesised:
//!
//! ```text
//!   saved EngineeringInput + committed EngineeringBundle
//!     -> production load_input + verify_bundle   (unchanged, this module calls them)
//!     -> a versioned read-only export: source bytes, source hashes, and a render model
//!        derived one-to-one from the saved geometry
//!     -> adapters/html/engineering viewer over the exported files
//!     -> this module's independent audit: re-verify, recompute the render model, compare
//! ```
//!
//! Nothing here re-runs terrain synthesis, carving, routing, the support verifier or any
//! stair planning. The engineered, natural and baseline fields are copied as *display*
//! samples (millimetres to metres) and are never written back. `world` and `display`
//! coordinates are the same metres on the documented frame (X east, Y up, Z north), so no
//! consumer may scale the route and the solids differently.
//!
//! The four conclusions the audit keeps apart:
//!
//! * `engineeringVerification` — the production verifier's own verdict on the saved bytes;
//! * `sourceBinding` — that the exported source copies are the bytes that were verified;
//! * `renderParity` — that the saved render model equals the model recomputed from the
//!   saved bundle, so a rewritten vertex, level, part or route mapping is caught even when
//!   the outer file hash was recomputed;
//! * `previewCompleteness` — that every declared file is present, whole, and that nothing
//!   undeclared was added.
//!
//! A valid engineering package does not imply a correct render projection, and a
//! self-consistent hash does not imply a valid engineering package.

use crate::worldgen::engineering_connections as ec;
use ec::{EngineeringBundle, EngineeringInput};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// The preview description's schema. A consumer refuses anything else.
pub const PREVIEW_SCHEMA: &str = "glyphweave.engineering-preview/1";
pub const RENDER_MODEL_SCHEMA: &str = "glyphweave.engineering-render-model/1";
pub const AUDIT_SCHEMA: &str = "glyphweave.engineering-preview-audit/1";
pub const PREVIEW_VERSION: u32 = 1;
pub const MODULE_VERSION: &str = "g23.1";

/// The declared display tolerance: the model is authored in `f64` world metres and uploaded
/// to the GPU as `f32`, which is exact to well under this bound at this packet's
/// coordinate range (|coordinate| < 64 m). It is a *display* tolerance and never an
/// engineering pass tolerance.
pub const DISPLAY_F32_TOLERANCE_M: f64 = 1e-5;

pub const DESCRIPTION_FILE: &str = "engineering-preview.json";
pub const SOURCE_INPUT_FILE: &str = "source/input.json";
pub const SOURCE_BUNDLE_FILE: &str = "source/bundle.json";
pub const RENDER_MODEL_FILE: &str = "render-model.json";
pub const VIEWER_ENTRY_FILE: &str = "preview/index.html";

/// The box topology used for every `MeshPart`. The production mesh walks the same eight
/// corners as bottom quad 0..3 then top quad 4..7, with 12 triangles; this is declared here
/// because the production table is private to its own module, and it is checked below for
/// face coverage, corner use, degeneracy and outward orientation.
pub const BOX_TRIANGLES: [[u8; 3]; 12] = [
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
];

/// The static modules the viewer needs, embedded from the repository adapter tree exactly
/// the way `worldgen::refresh_html_adapter` embeds the default page. No CDN, no network:
/// these bytes are what makes the export loadable offline.
pub const PREVIEW_ASSETS: &[(&str, &[u8])] = &[
    (
        "preview/index.html",
        include_bytes!("../../../../adapters/html/engineering/index.html"),
    ),
    (
        "preview/engineering-preview.js",
        include_bytes!("../../../../adapters/html/engineering/engineering-preview.js"),
    ),
    (
        "preview/preview-model.js",
        include_bytes!("../../../../adapters/html/engineering/preview-model.js"),
    ),
    (
        "preview/vendor/three/build/three.module.js",
        include_bytes!("../../../../adapters/html/vendor/three/build/three.module.js"),
    ),
    (
        "preview/vendor/three/build/three.core.js",
        include_bytes!("../../../../adapters/html/vendor/three/build/three.core.js"),
    ),
    (
        "preview/vendor/three/examples/jsm/controls/OrbitControls.js",
        include_bytes!(
            "../../../../adapters/html/vendor/three/examples/jsm/controls/OrbitControls.js"
        ),
    ),
    (
        "preview/vendor/three/LICENSE",
        include_bytes!("../../../../adapters/html/vendor/three/LICENSE"),
    ),
];

/// The roles the current preview format defines. A declared role outside this set is a format
/// violation, not an extension.
pub const ROLE_SOURCE_INPUT: &str = "source-input";
pub const ROLE_SOURCE_BUNDLE: &str = "source-bundle";
pub const ROLE_RENDER_MODEL: &str = "render-model";
pub const ROLE_VIEWER: &str = "viewer";
pub const ROLE_VIEWER_MODULE: &str = "viewer-module";
pub const ROLE_VENDOR: &str = "vendor";

/// Every role the supported format defines, for validating declared entries.
pub const FORMAT_ROLES: &[&str] = &[
    ROLE_SOURCE_INPUT,
    ROLE_SOURCE_BUNDLE,
    ROLE_RENDER_MODEL,
    ROLE_VIEWER,
    ROLE_VIEWER_MODULE,
    ROLE_VENDOR,
];

/// One file the *supported preview format* requires, with the role it must declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredAsset {
    pub path: &'static str,
    pub role: &'static str,
}

/// The payload files the current preview format requires, excluding the description itself
/// (`engineering-preview.json`), which is the completion marker and is deliberately kept out
/// of its own hash cycle.
///
/// This is the format's rule, and the exporter emits exactly this set. An audit that only
/// re-checked the package's own `files[]` would pass a package whose entry was deleted
/// together with its declaration: the package stays self-consistent, the audit's four
/// sections all pass, and the entry answers 404. The required set therefore has to come from
/// the format, never from the remainder of the audited declaration list.
pub const REQUIRED_ASSETS: &[RequiredAsset] = &[
    RequiredAsset {
        path: SOURCE_INPUT_FILE,
        role: ROLE_SOURCE_INPUT,
    },
    RequiredAsset {
        path: SOURCE_BUNDLE_FILE,
        role: ROLE_SOURCE_BUNDLE,
    },
    RequiredAsset {
        path: RENDER_MODEL_FILE,
        role: ROLE_RENDER_MODEL,
    },
    RequiredAsset {
        path: VIEWER_ENTRY_FILE,
        role: ROLE_VIEWER,
    },
    RequiredAsset {
        path: "preview/engineering-preview.js",
        role: ROLE_VIEWER_MODULE,
    },
    RequiredAsset {
        path: "preview/preview-model.js",
        role: ROLE_VIEWER_MODULE,
    },
    RequiredAsset {
        path: "preview/vendor/three/build/three.module.js",
        role: ROLE_VENDOR,
    },
    RequiredAsset {
        path: "preview/vendor/three/build/three.core.js",
        role: ROLE_VENDOR,
    },
    RequiredAsset {
        path: "preview/vendor/three/examples/jsm/controls/OrbitControls.js",
        role: ROLE_VENDOR,
    },
    RequiredAsset {
        path: "preview/vendor/three/LICENSE",
        role: ROLE_VENDOR,
    },
];

/// The required asset for a canonical relative path, when the format defines one.
pub fn required_asset(path: &str) -> Option<&'static RequiredAsset> {
    REQUIRED_ASSETS.iter().find(|asset| asset.path == path)
}

/// Whether a declared path is a canonical relative preview path: no absolute form, no drive or
/// scheme prefix, no parent traversal, no backslash and no empty segment.
pub fn is_canonical_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.ends_with('/') || path.contains('\\') {
        return false;
    }
    if path.contains(':') {
        return false;
    }
    path.split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

// ---------------------------------------------------------------------------
// SHA-256, implemented here so the export needs no new dependency
// ---------------------------------------------------------------------------

mod sha256 {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut h = H0;
        let bit_len = (data.len() as u64).wrapping_mul(8);
        let mut message = data.to_vec();
        message.push(0x80);
        while message.len() % 64 != 56 {
            message.push(0);
        }
        message.extend_from_slice(&bit_len.to_be_bytes());
        for chunk in message.chunks(64) {
            let mut w = [0u32; 64];
            for (index, word) in w.iter_mut().take(16).enumerate() {
                let at = index * 4;
                *word =
                    u32::from_be_bytes([chunk[at], chunk[at + 1], chunk[at + 2], chunk[at + 3]]);
            }
            for index in 16..64 {
                let s0 = w[index - 15].rotate_right(7)
                    ^ w[index - 15].rotate_right(18)
                    ^ (w[index - 15] >> 3);
                let s1 = w[index - 2].rotate_right(17)
                    ^ w[index - 2].rotate_right(19)
                    ^ (w[index - 2] >> 10);
                w[index] = w[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[index - 7])
                    .wrapping_add(s1);
            }
            let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
            let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
            for index in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[index])
                    .wrapping_add(w[index]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }
        let mut out = [0u8; 32];
        for (index, word) in h.iter().enumerate() {
            out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    pub fn hex(data: &[u8]) -> String {
        let mut out = String::with_capacity(64);
        for byte in digest(data) {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }
}

/// SHA-256 of these bytes, lowercase hex. The browser re-computes the same value with
/// SubtleCrypto before it renders anything.
pub fn sha256_hex(data: &[u8]) -> String {
    sha256::hex(data)
}

// ---------------------------------------------------------------------------
// Exported facts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFileFact {
    pub path: String,
    /// `source-input`, `source-bundle`, `render-model`, `viewer`, `viewer-module`,
    /// `vendor`.
    pub role: String,
    pub bytes: u64,
    pub sha256: String,
    pub objects: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds_m: Option<[f64; 6]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFacts {
    /// Bytes and SHA-256 of the file on disk, as the exporter read it.
    pub input_bytes: u64,
    pub input_sha256: String,
    pub bundle_bytes: u64,
    pub bundle_sha256: String,
    /// The bundle's *own* internal hashes. Different units, kept apart from the file
    /// hashes above and never written into the same field.
    pub bundle_input_hash: String,
    pub bundle_payload_hash: String,
    pub contract_version: u32,
    pub publication_state: String,
    pub scene_id: String,
    pub units: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationFacts {
    pub engine: String,
    pub engine_module_version: String,
    pub engine_contract_version: u32,
    pub verify_status: String,
    pub verify_finding_count: usize,
    pub input_hash_matches: bool,
    pub payload_hash_matches: bool,
    pub route_status: String,
    pub route_capability: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayConvention {
    pub world_to_display: String,
    pub applies_to: String,
    pub f32_tolerance_m: f64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDescription {
    pub schema: String,
    pub version: u32,
    /// `complete` only after every payload file is in place; the description is written
    /// last and is the completion marker.
    pub export_status: String,
    pub domain: String,
    pub coordinate_frame: String,
    pub display_convention: DisplayConvention,
    pub scene_id: String,
    pub units: String,
    pub source: SourceFacts,
    pub verification: VerificationFacts,
    pub render_model: PreviewFileFact,
    pub viewer_entry: String,
    pub files: Vec<PreviewFileFact>,
    pub trust_boundary: String,
    pub consumer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPart {
    pub part_id: String,
    pub structure_id: String,
    pub role: String,
    pub level_id: String,
    pub run_index: u32,
    pub index: u32,
    pub top_mm: i32,
    pub bottom_mm: i32,
    /// The eight saved corners, verbatim: bottom quad 0..3 then top quad 4..7.
    pub corners_m: [[f64; 3]; 8],
    pub triangles: [[u8; 3]; 12],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNode {
    pub node_id: String,
    pub kind: String,
    pub position_m: [f64; 3],
    pub surface_id: String,
    pub level_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road_id: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderEdge {
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub kind: String,
    pub bidirectional: bool,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_id: Option<String>,
    pub polyline_m: Vec<[f64; 3]>,
    pub horizontal_length_m: f64,
    pub path_length_3d_m: f64,
    pub rise_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRoute {
    pub status: String,
    pub capability: String,
    pub from_port_id: String,
    pub to_road_id: String,
    pub path_node_ids: Vec<String>,
    pub path_edge_ids: Vec<String>,
    pub polyline_m: Vec<[f64; 3]>,
    pub horizontal_length_m: f64,
    pub path_length_3d_m: f64,
    pub cumulative_rise_m: f64,
    pub cumulative_descent_m: f64,
    pub limits: RenderRouteLimits,
    /// The saved query this route is the result of. The viewer never re-routes.
    pub query: RenderQuery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRouteLimits {
    pub max_length_3d_m: f64,
    pub max_rise_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderQuery {
    pub from_port_id: String,
    pub to_road_id: String,
    pub capability: String,
    pub max_length_3d_m: f64,
    pub max_rise_m: f64,
    pub road_id_binding: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySurface {
    pub width: u32,
    pub depth: u32,
    pub origin_x: i32,
    pub origin_z: i32,
    /// Display samples in metres, copied from the saved millimetres. A display mesh is
    /// built from these; no synthesis, no carving, no write-back.
    pub heights_m: Vec<f64>,
    /// The hash the bundle (engineered) or the input (natural, baseline) records.
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTerrain {
    pub engineered: DisplaySurface,
    pub natural: DisplaySurface,
    pub baseline: DisplaySurface,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCheckBox {
    pub id: String,
    pub x0: f64,
    pub z0: f64,
    pub x1: f64,
    pub z1: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCounts {
    pub parts: usize,
    pub structures: usize,
    pub nodes: usize,
    pub edges: usize,
    pub route_points: usize,
    pub terrain_cells: usize,
    pub obstacles: usize,
    pub water: usize,
    pub triangles: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds_m: Option<[f64; 6]>,
    pub structure_ids: Vec<String>,
    pub level_ids: Vec<String>,
    pub surface_ids: Vec<String>,
    pub part_roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderModel {
    pub schema: String,
    pub version: u32,
    pub scene_id: String,
    pub units: String,
    pub coordinate_frame: String,
    pub display_convention: DisplayConvention,
    pub parts: Vec<RenderPart>,
    pub nodes: Vec<RenderNode>,
    pub edges: Vec<RenderEdge>,
    pub route: RenderRoute,
    pub terrain: RenderTerrain,
    /// Read-only check layer, verbatim from the saved input.
    pub obstacles: Vec<RenderCheckBox>,
    pub water: Vec<RenderCheckBox>,
    /// Explicit empty declarations stay distinct from absent context: these two fields
    /// are always present and always say which state the input was in.
    pub declared_obstacles: bool,
    pub declared_water: bool,
    pub counts: RenderCounts,
    /// The production mesh totals recorded in the bundle, for the reader to compare.
    pub production_mesh_points: usize,
    pub production_mesh_triangles: usize,
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PreviewExport {
    pub description: PreviewDescription,
    pub description_json: Vec<u8>,
    pub source_input: Vec<u8>,
    pub source_bundle: Vec<u8>,
    pub render_model_json: Vec<u8>,
    /// `preview/...` modules, already embedded from the adapter tree.
    pub assets: Vec<(String, Vec<u8>)>,
}

fn err(category: &str, message: impl Into<String>) -> ec::EngineeringError {
    ec::EngineeringError::new(category, message)
}

fn bounds_of(points: impl Iterator<Item = [f64; 3]>) -> Option<[f64; 6]> {
    let mut seen = false;
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for point in points {
        if !point.iter().all(|value| value.is_finite()) {
            continue;
        }
        seen = true;
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if !seen {
        return None;
    }
    Some([min[0], min[1], min[2], max[0], max[1], max[2]])
}

fn capability_names(values: &[ec::Capability]) -> Vec<String> {
    values
        .iter()
        .map(|capability| ec::capability_name(*capability).to_string())
        .collect()
}

fn display_convention() -> DisplayConvention {
    DisplayConvention {
        world_to_display: "identity".into(),
        applies_to: "both the structure solids and the route polyline".into(),
        f32_tolerance_m: DISPLAY_F32_TOLERANCE_M,
        note: format!(
            "display metres equal world metres on the contract frame (X east, Y up, Z north); nothing is flattened, dropped to the ground or raised. The only declared difference is the f32 upload of an f64 world coordinate, bounded by {DISPLAY_F32_TOLERANCE_M} m at this packet's range, and it is a display tolerance, never an engineering one."
        ),
    }
}

fn display_surface(
    width: u32,
    depth: u32,
    origin_x: i32,
    origin_z: i32,
    mm: &[i32],
    content_hash: &str,
) -> DisplaySurface {
    DisplaySurface {
        width,
        depth,
        origin_x,
        origin_z,
        heights_m: mm.iter().map(|value| f64::from(*value) / 1000.0).collect(),
        content_hash: content_hash.to_string(),
    }
}

/// Build the render model from the saved bytes. Pure: it copies and re-keys, it never
/// re-derives geometry from a label.
pub fn build_render_model(
    input: &EngineeringInput,
    bundle: &EngineeringBundle,
) -> Result<RenderModel, ec::EngineeringError> {
    if input.grid.width != bundle.surface.width
        || input.grid.depth != bundle.surface.depth
        || input.grid.origin_x != bundle.surface.origin_x
        || input.grid.origin_z != bundle.surface.origin_z
    {
        return Err(err(
            "preview_grid_mismatch",
            format!(
                "the input grid {}x{} at ({}, {}) and the bundle surface {}x{} at ({}, {}) are not the same domain",
                input.grid.width,
                input.grid.depth,
                input.grid.origin_x,
                input.grid.origin_z,
                bundle.surface.width,
                bundle.surface.depth,
                bundle.surface.origin_x,
                bundle.surface.origin_z
            ),
        ));
    }
    let expected = bundle.surface.width as usize * bundle.surface.depth as usize;
    if bundle.surface.mm.len() != expected {
        return Err(err(
            "preview_surface_mismatch",
            format!(
                "the bundle surface declares {expected} cells but carries {}",
                bundle.surface.mm.len()
            ),
        ));
    }
    if input.grid.natural_mm.len() != expected || input.grid.baseline_mm.len() != expected {
        return Err(err(
            "preview_surface_mismatch",
            format!(
                "the input grid declares {expected} cells per field but carries natural {} and baseline {}",
                input.grid.natural_mm.len(),
                input.grid.baseline_mm.len()
            ),
        ));
    }

    let mut parts: Vec<RenderPart> = vec![];
    let mut structure_ids: Vec<String> = vec![];
    let mut level_ids: Vec<String> = vec![];
    let mut part_roles: Vec<String> = vec![];
    for structure in &bundle.structures {
        if !structure_ids.contains(&structure.structure_id) {
            structure_ids.push(structure.structure_id.clone());
        }
        for part in &structure.parts {
            if !level_ids.contains(&part.level_id) {
                level_ids.push(part.level_id.clone());
            }
            if !part_roles.contains(&part.role) {
                part_roles.push(part.role.clone());
            }
            parts.push(RenderPart {
                part_id: part.part_id.clone(),
                structure_id: part.structure_id.clone(),
                role: part.role.clone(),
                level_id: part.level_id.clone(),
                run_index: part.run_index,
                index: part.index,
                top_mm: part.top_mm,
                bottom_mm: part.bottom_mm,
                corners_m: part.corners_m,
                triangles: BOX_TRIANGLES,
            });
        }
    }
    if parts.is_empty() && !bundle.structures.is_empty() {
        return Err(err(
            "preview_structure_without_parts",
            "a structure exists but carries no walking member to display",
        ));
    }

    let nodes: Vec<RenderNode> = bundle
        .nodes
        .iter()
        .map(|node| RenderNode {
            node_id: node.node_id.clone(),
            kind: node.kind.clone(),
            position_m: node.position_m,
            surface_id: node.surface_id.clone(),
            level_id: node.level_id.clone(),
            road_id: node.road_id.clone(),
            capabilities: capability_names(&node.capabilities),
        })
        .collect();
    let edges: Vec<RenderEdge> = bundle
        .graph
        .edges
        .iter()
        .map(|edge| RenderEdge {
            edge_id: edge.edge_id.clone(),
            from_node: edge.from_node.clone(),
            to_node: edge.to_node.clone(),
            kind: edge.kind.clone(),
            bidirectional: edge.bidirectional,
            capabilities: capability_names(&edge.capabilities),
            structure_id: edge.structure_id.clone(),
            polyline_m: edge.polyline_m.clone(),
            horizontal_length_m: edge.horizontal_length_m,
            path_length_3d_m: edge.path_length_3d_m,
            rise_m: edge.rise_m,
        })
        .collect();

    let surface_ids = {
        let mut ids: Vec<String> = vec![];
        for node in &bundle.nodes {
            if !ids.contains(&node.surface_id) {
                ids.push(node.surface_id.clone());
            }
        }
        ids
    };

    let route = RenderRoute {
        status: bundle.route.status.clone(),
        capability: ec::capability_name(bundle.route.capability).to_string(),
        from_port_id: bundle.route.from_port_id.clone(),
        to_road_id: bundle.route.to_road_id.clone(),
        path_node_ids: bundle.route.path_node_ids.clone(),
        path_edge_ids: bundle.route.path_edge_ids.clone(),
        polyline_m: bundle.route.path_polyline_m.clone(),
        horizontal_length_m: bundle.route.horizontal_length_m,
        path_length_3d_m: bundle.route.path_length_3d_m,
        cumulative_rise_m: bundle.route.cumulative_rise_m,
        cumulative_descent_m: bundle.route.cumulative_descent_m,
        limits: RenderRouteLimits {
            max_length_3d_m: bundle.route.limits.max_length_3d_m,
            max_rise_m: bundle.route.limits.max_rise_m,
        },
        query: RenderQuery {
            from_port_id: input.query.from_port_id.clone(),
            to_road_id: input.query.to_road_id.clone(),
            capability: ec::capability_name(input.query.capability).to_string(),
            max_length_3d_m: input.query.max_length_3d_m,
            max_rise_m: input.query.max_rise_m,
            road_id_binding:
                "roadId is a declaration of this human-authored input; no ST18 or real road binding is claimed"
                    .into(),
        },
    };

    let terrain = RenderTerrain {
        engineered: display_surface(
            bundle.surface.width,
            bundle.surface.depth,
            bundle.surface.origin_x,
            bundle.surface.origin_z,
            &bundle.surface.mm,
            &bundle.surface.engineered_hash,
        ),
        natural: display_surface(
            input.grid.width,
            input.grid.depth,
            input.grid.origin_x,
            input.grid.origin_z,
            &input.grid.natural_mm,
            &bundle.surface.natural_hash,
        ),
        baseline: display_surface(
            input.grid.width,
            input.grid.depth,
            input.grid.origin_x,
            input.grid.origin_z,
            &input.grid.baseline_mm,
            &bundle.surface.baseline_hash,
        ),
        note: "display meshes built from the saved samples only; no terrain synthesis, no carve re-run, no write-back"
            .into(),
    };

    let obstacles: Vec<RenderCheckBox> = input
        .obstacles
        .iter()
        .map(|obstacle| RenderCheckBox {
            id: obstacle.id.clone(),
            x0: obstacle.rect.x0,
            z0: obstacle.rect.z0,
            x1: obstacle.rect.x1,
            z1: obstacle.rect.z1,
            bottom_m: Some(obstacle.bottom_m),
            top_m: Some(obstacle.top_m),
        })
        .collect();
    let water: Vec<RenderCheckBox> = input
        .water
        .iter()
        .map(|water| RenderCheckBox {
            id: water.id.clone(),
            x0: water.rect.x0,
            z0: water.rect.z0,
            x1: water.rect.x1,
            z1: water.rect.z1,
            bottom_m: None,
            top_m: None,
        })
        .collect();

    let bounds = bounds_of(
        parts
            .iter()
            .flat_map(|part| part.corners_m.iter().copied())
            .chain(route.polyline_m.iter().copied())
            .chain(nodes.iter().map(|node| node.position_m)),
    );

    let counts = RenderCounts {
        parts: parts.len(),
        structures: structure_ids.len(),
        nodes: nodes.len(),
        edges: edges.len(),
        route_points: route.polyline_m.len(),
        terrain_cells: expected,
        obstacles: obstacles.len(),
        water: water.len(),
        triangles: parts.len() * BOX_TRIANGLES.len(),
        bounds_m: bounds,
        structure_ids,
        level_ids,
        surface_ids,
        part_roles,
    };

    Ok(RenderModel {
        schema: RENDER_MODEL_SCHEMA.into(),
        version: PREVIEW_VERSION,
        scene_id: bundle.scene_id.clone(),
        units: bundle.units.clone(),
        coordinate_frame:
            "world metres; X east, Y up, Z north; levels in the saved facts stay signed millimetres"
                .into(),
        display_convention: display_convention(),
        parts,
        nodes,
        edges,
        route,
        terrain,
        obstacles,
        water,
        declared_obstacles: true,
        declared_water: true,
        counts,
        production_mesh_points: bundle.mesh_points,
        production_mesh_triangles: bundle.mesh_triangles,
    })
}

/// Verify the saved bytes with the production verifier and collect the facts the export
/// records. Read-only: nothing in the input or the bundle is modified to fit the preview.
pub fn verify_sources(
    input_json: &str,
    bundle_json: &str,
) -> Result<(EngineeringInput, EngineeringBundle, ec::VerifyReport), ec::EngineeringError> {
    let input = ec::load_input(input_json)?;
    let bundle: EngineeringBundle = serde_json::from_str(bundle_json).map_err(|error| {
        err(
            "preview_bundle_unreadable",
            format!("the saved bundle is not a readable bundle: {error}"),
        )
    })?;
    if bundle.publication.state != "committed" {
        return Err(err(
            "preview_bundle_not_committed",
            format!(
                "only a committed publication may be consumed; this bundle is {}",
                bundle.publication.state
            ),
        ));
    }
    if bundle.publication.contract_version != ec::ENGINEERING_CONNECTIONS_VERSION {
        return Err(err(
            "preview_contract_version",
            format!(
                "the bundle was committed at contract version {} but this module implements {}",
                bundle.publication.contract_version,
                ec::ENGINEERING_CONNECTIONS_VERSION
            ),
        ));
    }
    // Two files that merely share a sceneId are not one source: the domain, the unit
    // contract and the saved input hash all have to agree as well.
    if input.scene_id != bundle.scene_id {
        return Err(err(
            "preview_scene_mismatch",
            format!(
                "the input scene {} and the bundle scene {} are different sources",
                input.scene_id, bundle.scene_id
            ),
        ));
    }
    if input.units != bundle.units {
        return Err(err(
            "preview_units_mismatch",
            format!(
                "the input unit contract {:?} and the bundle unit contract {:?} disagree",
                input.units, bundle.units
            ),
        ));
    }
    let report = ec::verify_bundle(&input, &bundle);
    if report.status != "verified" {
        return Err(err(
            "preview_source_not_verified",
            format!(
                "the production verifier refused this source: status {} with {} finding(s), first {:?}",
                report.status,
                report.finding_count,
                report.findings.first().map(|finding| finding.code.clone())
            ),
        )
        .with_detail(serde_json::json!({
            "verifyStatus": report.status,
            "findingCount": report.finding_count,
            "findings": report.findings.iter().take(10).map(|finding| serde_json::json!({
                "code": finding.code, "at": finding.at, "expected": finding.expected, "found": finding.found,
            })).collect::<Vec<_>>(),
        })));
    }
    if !report.payload_hash_matches {
        return Err(err(
            "preview_payload_hash_mismatch",
            "the bundle's payload hash does not match its own content",
        ));
    }
    // Grid and route identity are cross-checked before anything is exported.
    if input.grid.width != bundle.surface.width
        || input.grid.depth != bundle.surface.depth
        || input.grid.origin_x != bundle.surface.origin_x
        || input.grid.origin_z != bundle.surface.origin_z
    {
        return Err(err(
            "preview_grid_mismatch",
            "the input grid and the bundle surface describe different domains",
        ));
    }
    Ok((input, bundle, report))
}

fn file_fact(
    path: &str,
    role: &str,
    bytes: &[u8],
    objects: usize,
    bounds_m: Option<[f64; 6]>,
) -> PreviewFileFact {
    PreviewFileFact {
        path: path.to_string(),
        role: role.to_string(),
        bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        objects,
        bounds_m,
    }
}

/// Build the whole export in memory: the description, the source copies, the render model
/// and the viewer modules. The caller owns publication.
pub fn build_export(
    input_json: &str,
    bundle_json: &str,
) -> Result<PreviewExport, ec::EngineeringError> {
    let (input, bundle, report) = verify_sources(input_json, bundle_json)?;
    let render_model = build_render_model(&input, &bundle)?;
    let render_model_json = serde_json::to_vec_pretty(&render_model).map_err(|error| {
        err(
            "preview_serialise",
            format!("the render model could not be serialised: {error}"),
        )
    })?;

    // The exporter publishes exactly the format's required set, taken from the same rule the
    // audit enforces, so the emitted list and the required list cannot drift apart.
    let payload: [(&str, &[u8]); 3] = [
        (SOURCE_INPUT_FILE, input_json.as_bytes()),
        (SOURCE_BUNDLE_FILE, bundle_json.as_bytes()),
        (RENDER_MODEL_FILE, &render_model_json),
    ];
    let mut files: Vec<PreviewFileFact> = vec![];
    for asset in REQUIRED_ASSETS {
        let bytes = if let Some((_, bytes)) = payload.iter().find(|(path, _)| *path == asset.path) {
            *bytes
        } else {
            let (_, bytes) = PREVIEW_ASSETS
                .iter()
                .find(|(path, _)| *path == asset.path)
                .ok_or_else(|| {
                    err(
                        "preview_internal",
                        format!(
                            "the format requires {} but no asset bytes are embedded for it",
                            asset.path
                        ),
                    )
                })?;
            *bytes
        };
        let (objects, bounds_m) = if asset.path == RENDER_MODEL_FILE {
            (
                render_model.parts.len() + render_model.nodes.len() + render_model.edges.len(),
                render_model.counts.bounds_m,
            )
        } else if asset.role == ROLE_SOURCE_INPUT || asset.role == ROLE_SOURCE_BUNDLE {
            (1, None)
        } else {
            (0, None)
        };
        files.push(file_fact(asset.path, asset.role, bytes, objects, bounds_m));
    }
    if files.len() != REQUIRED_ASSETS.len() {
        return Err(err(
            "preview_internal",
            format!(
                "the export produced {} payload file(s) but the format requires {}",
                files.len(),
                REQUIRED_ASSETS.len()
            ),
        ));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let render_model_fact = files
        .iter()
        .find(|fact| fact.path == RENDER_MODEL_FILE)
        .cloned()
        .ok_or_else(|| err("preview_internal", "the render model fact is missing"))?;

    let description = PreviewDescription {
        schema: PREVIEW_SCHEMA.into(),
        version: PREVIEW_VERSION,
        export_status: "complete".into(),
        domain:
            "one human-authored engineering package: structures, ports, edges and one saved route query"
                .into(),
        coordinate_frame:
            "world metres; X east, Y up, Z north; levels in the saved facts stay signed millimetres"
                .into(),
        display_convention: display_convention(),
        scene_id: bundle.scene_id.clone(),
        units: bundle.units.clone(),
        source: SourceFacts {
            input_bytes: input_json.len() as u64,
            input_sha256: sha256_hex(input_json.as_bytes()),
            bundle_bytes: bundle_json.len() as u64,
            bundle_sha256: sha256_hex(bundle_json.as_bytes()),
            bundle_input_hash: bundle.input_hash.clone(),
            bundle_payload_hash: bundle.payload_hash.clone(),
            contract_version: bundle.publication.contract_version,
            publication_state: bundle.publication.state.clone(),
            scene_id: input.scene_id.clone(),
            units: input.units.clone(),
        },
        verification: VerificationFacts {
            engine: "glyphweave_core::worldgen::engineering_connections".into(),
            engine_module_version: MODULE_VERSION.into(),
            engine_contract_version: ec::ENGINEERING_CONNECTIONS_VERSION,
            verify_status: report.status.clone(),
            verify_finding_count: report.finding_count,
            input_hash_matches: report.input_hash_matches,
            payload_hash_matches: report.payload_hash_matches,
            route_status: bundle.route.status.clone(),
            route_capability: ec::capability_name(bundle.route.capability).to_string(),
        },
        render_model: render_model_fact,
        viewer_entry: VIEWER_ENTRY_FILE.into(),
        files,
        trust_boundary:
            "the browser verifies these file hashes and says only 'verified at export time, file integrity still matches'; it does not claim to run the Rust engineering audit. `audit-engineering-preview` re-runs the production verifier and recomputes the render model."
                .into(),
        consumer: "adapters/html/engineering".into(),
    };

    let description_json = serde_json::to_vec_pretty(&description).map_err(|error| {
        err(
            "preview_serialise",
            format!("the description could not be serialised: {error}"),
        )
    })?;

    Ok(PreviewExport {
        description,
        description_json,
        source_input: input_json.as_bytes().to_vec(),
        source_bundle: bundle_json.as_bytes().to_vec(),
        render_model_json,
        assets: PREVIEW_ASSETS
            .iter()
            .map(|(path, bytes)| ((*path).to_string(), bytes.to_vec()))
            .collect(),
    })
}

/// Read the observed files of a preview directory into `path -> (bytes, sha256)`. Paths are
/// relative to the preview root, `/`-separated, and every symlink/reparse point is refused
/// rather than followed, so a package cannot point the audit outside itself.
pub fn observe_directory(
    root: &Path,
) -> Result<BTreeMap<String, (u64, String)>, ec::EngineeringError> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| {
            err(
                "preview_unreadable",
                format!("cannot read {}: {error}", dir.display()),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                err(
                    "preview_unreadable",
                    format!("cannot read an entry: {error}"),
                )
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                err(
                    "preview_unreadable",
                    format!("cannot stat {}: {error}", path.display()),
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(err(
                    "preview_symlink_refused",
                    format!(
                        "{} is a link; a preview package may not contain one",
                        path.display()
                    ),
                ));
            }
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| err("preview_path_escape", "a file is outside the preview root"))?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path).map_err(|error| {
                err(
                    "preview_unreadable",
                    format!("cannot read {}: {error}", path.display()),
                )
            })?;
            let hash = sha256_hex(&bytes);
            out.insert(relative, (bytes.len() as u64, hash));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Independent audit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSection {
    /// `pass` or `fail`.
    pub status: String,
    pub detail: String,
    pub findings: Vec<String>,
}

impl AuditSection {
    fn new() -> Self {
        Self {
            status: "pass".into(),
            detail: String::new(),
            findings: vec![],
        }
    }
    fn fail(&mut self, finding: impl Into<String>) {
        self.status = "fail".into();
        self.findings.push(finding.into());
    }
    fn note(&mut self, detail: impl Into<String>) {
        self.detail = detail.into();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub schema: String,
    pub version: u32,
    pub preview_root: String,
    /// `pass` only when all four sections pass.
    pub status: String,
    pub engineering_verification: AuditSection,
    pub source_binding: AuditSection,
    pub render_parity: AuditSection,
    pub preview_completeness: AuditSection,
    pub findings: Vec<String>,
    pub finding_count: usize,
    pub observed: serde_json::Value,
}

const DIFF_LIMIT: usize = 40;

fn diff_json(
    expected: &serde_json::Value,
    found: &serde_json::Value,
    path: &str,
    out: &mut Vec<String>,
) {
    if out.len() >= DIFF_LIMIT {
        return;
    }
    match (expected, found) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (key, value) in a {
                let child = format!("{path}.{key}");
                match b.get(key) {
                    Some(other) => diff_json(value, other, &child, out),
                    None => out.push(format!("{child}: missing in the saved projection")),
                }
                if out.len() >= DIFF_LIMIT {
                    return;
                }
            }
            for key in b.keys() {
                if !a.contains_key(key) {
                    out.push(format!(
                        "{path}.{key}: present only in the saved projection"
                    ));
                }
                if out.len() >= DIFF_LIMIT {
                    return;
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!(
                    "{path}: saved {} element(s), recomputed {}",
                    b.len(),
                    a.len()
                ));
                return;
            }
            for (index, value) in a.iter().enumerate() {
                diff_json(value, &b[index], &format!("{path}[{index}]"), out);
                if out.len() >= DIFF_LIMIT {
                    return;
                }
            }
        }
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            let same = match (a.as_f64(), b.as_f64()) {
                // Exact for the saved geometry: the projection is a copy, not a fit.
                (Some(left), Some(right)) => left == right,
                _ => a == b,
            };
            if !same {
                out.push(format!("{path}: saved {b} recomputed {a}"));
            }
        }
        _ => {
            if expected != found {
                let shorten = |value: &serde_json::Value| {
                    let text = value.to_string();
                    if text.len() > 120 {
                        format!("{}…", &text[..120])
                    } else {
                        text
                    }
                };
                out.push(format!(
                    "{path}: saved {} recomputed {}",
                    shorten(found),
                    shorten(expected)
                ));
            }
        }
    }
}

/// Audit one exported preview. It never trusts the saved `verified` text: it re-runs the
/// production verifier, recomputes the render model from the saved bundle, and compares.
/// Structural problems are reported as findings rather than as an `Err`, so the caller
/// always has a written report to hand on.
#[allow(clippy::too_many_arguments)]
pub fn audit_export(
    preview_root: &str,
    description_json: Option<&str>,
    input_json: Option<&str>,
    bundle_json: Option<&str>,
    render_model_json: Option<&str>,
    observed: &BTreeMap<String, (u64, String)>,
) -> AuditReport {
    let mut engineering = AuditSection::new();
    let mut binding = AuditSection::new();
    let mut parity = AuditSection::new();
    let mut completeness = AuditSection::new();

    let description: Option<PreviewDescription> = match description_json {
        Some(text) => match serde_json::from_str::<PreviewDescription>(text) {
            Ok(value) => Some(value),
            Err(error) => {
                completeness.fail(format!("the preview description is not readable: {error}"));
                None
            }
        },
        None => {
            completeness.fail(format!(
                "{DESCRIPTION_FILE} is missing: the export never completed"
            ));
            None
        }
    };

    // --- previewCompleteness -------------------------------------------------
    //
    // The required set is the *format's* rule, never the remainder of the package's own
    // `files[]`. Deleting a required file and its declaration together leaves the package
    // perfectly self-consistent, so a completeness check that only re-read the declarations
    // would report pass while the entry answers 404. Every required path must therefore be
    // declared exactly once, with the format's role, present on disk, and matching its own
    // declared bytes and hash.
    if let Some(description) = &description {
        if description.schema != PREVIEW_SCHEMA {
            completeness.fail(format!(
                "unsupported preview schema {:?}; this audit reads {PREVIEW_SCHEMA}",
                description.schema
            ));
        }
        if description.version != PREVIEW_VERSION {
            completeness.fail(format!(
                "unsupported preview version {}; this audit reads {PREVIEW_VERSION}",
                description.version
            ));
        }
        if description.schema == PREVIEW_SCHEMA && description.version == PREVIEW_VERSION {
            if description.export_status != "complete" {
                completeness.fail(format!(
                    "export status is {:?}, not complete",
                    description.export_status
                ));
            }
            if description.viewer_entry != VIEWER_ENTRY_FILE {
                completeness.fail(format!(
                    "the declared viewer entry {:?} is not the supported entry {VIEWER_ENTRY_FILE}",
                    description.viewer_entry
                ));
            }
            if required_asset(&description.viewer_entry).map(|asset| asset.role)
                != Some(ROLE_VIEWER)
            {
                completeness.fail(format!(
                    "the declared viewer entry {:?} is not a required viewer asset of this format",
                    description.viewer_entry
                ));
            }
            // Every declared entry must be a canonical relative path carrying a defined role.
            for fact in &description.files {
                if !is_canonical_relative_path(&fact.path) {
                    completeness.fail(format!(
                        "declared path {:?} is not a canonical relative preview path",
                        fact.path
                    ));
                }
                if !FORMAT_ROLES.contains(&fact.role.as_str()) {
                    completeness.fail(format!(
                        "{}: declared role {:?} is not defined by the supported format",
                        fact.path, fact.role
                    ));
                }
            }
            // A duplicate declaration is refused even when the two entries agree byte for byte.
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for fact in &description.files {
                *counts.entry(fact.path.as_str()).or_insert(0) += 1;
            }
            for (path, count) in &counts {
                if *count > 1 {
                    completeness.fail(format!(
                        "{path}: declared {count} times; the format allows exactly one entry per required path"
                    ));
                }
            }
            // The required set, checked independently of what the package declared.
            let mut required_ok = 0_usize;
            for asset in REQUIRED_ASSETS {
                let entries: Vec<&PreviewFileFact> = description
                    .files
                    .iter()
                    .filter(|fact| fact.path == asset.path)
                    .collect();
                let Some(fact) = entries.first() else {
                    completeness.fail(format!(
                        "{}: the supported format requires this {} asset and no files[] entry declares it{}",
                        asset.path,
                        asset.role,
                        if observed.contains_key(asset.path) {
                            " (the file is present on disk but undeclared)"
                        } else {
                            " (the file is absent from disk as well)"
                        }
                    ));
                    continue;
                };
                if fact.role != asset.role {
                    completeness.fail(format!(
                        "{}: declared role {:?}, the format requires {:?}",
                        asset.path, fact.role, asset.role
                    ));
                }
                match observed.get(asset.path) {
                    None => completeness.fail(format!(
                        "{}: declared as {} but the file is absent from disk",
                        asset.path, fact.role
                    )),
                    Some((bytes, hash)) => {
                        if *bytes != fact.bytes {
                            completeness.fail(format!(
                                "{}: declared {} bytes, on disk {bytes}",
                                asset.path, fact.bytes
                            ));
                        }
                        if *hash != fact.sha256 {
                            completeness.fail(format!(
                                "{}: content hash does not match the declared one",
                                asset.path
                            ));
                        }
                        required_ok += 1;
                    }
                }
            }
            // An entry the format does not define is a silent attempt to extend the contract.
            for fact in &description.files {
                if required_asset(&fact.path).is_none() {
                    completeness.fail(format!(
                        "{}: declared with role {:?}, but the supported format defines no such asset",
                        fact.path, fact.role
                    ));
                }
            }
            // The fixed references must agree with the unique entries and the real files.
            let render_entries: Vec<&PreviewFileFact> = description
                .files
                .iter()
                .filter(|fact| fact.path == RENDER_MODEL_FILE)
                .collect();
            match render_entries.first() {
                None => completeness.fail(format!(
                    "{RENDER_MODEL_FILE}: the format requires it and no files[] entry declares it"
                )),
                Some(fact) => {
                    if description.render_model.path != fact.path
                        || description.render_model.role != fact.role
                        || description.render_model.bytes != fact.bytes
                        || description.render_model.sha256 != fact.sha256
                    {
                        completeness.fail(format!(
                            "the renderModel reference (path {:?}, role {:?}, {} bytes, hash {}) does not agree with the unique {RENDER_MODEL_FILE} entry",
                            description.render_model.path,
                            description.render_model.role,
                            description.render_model.bytes,
                            description.render_model.sha256
                        ));
                    }
                }
            }
            // Observed files the format does not define and the description does not account for.
            for path in observed.keys() {
                if required_asset(path).is_none() && path != DESCRIPTION_FILE {
                    completeness.fail(format!("undeclared file inside the preview: {path}"));
                }
            }
            if !observed.contains_key(DESCRIPTION_FILE) {
                completeness.fail(format!(
                    "{DESCRIPTION_FILE} is absent from disk; the export never completed"
                ));
            } else if let Some((bytes, hash)) = observed.get(DESCRIPTION_FILE) {
                let text = description_json.unwrap_or("");
                if *bytes != text.len() as u64 || *hash != sha256_hex(text.as_bytes()) {
                    completeness
                        .fail("the description on disk is not the description that was read");
                }
            }
            completeness.note(format!(
                "{required_ok} of {} required payload file(s) present, declared once with the right role and matching their own bytes and hash; {} declared entr(ies); {} file(s) observed",
                REQUIRED_ASSETS.len(),
                description.files.len(),
                observed.len()
            ));
        } else {
            // An unrecognised format is refused outright rather than validated against this
            // format's asset set, which would be meaningless for a rule set we do not know.
            completeness.note(format!(
                "the preview declares schema {:?} version {}; this audit enforces the {PREVIEW_SCHEMA} version {PREVIEW_VERSION} asset set and cannot judge another format",
                description.schema, description.version
            ));
        }
    }

    // --- sourceBinding -------------------------------------------------------
    let mut verified: Option<(EngineeringInput, EngineeringBundle, ec::VerifyReport)> = None;
    match (description.as_ref(), input_json, bundle_json) {
        (Some(description), Some(input_text), Some(bundle_text)) => {
            let input_hash = sha256_hex(input_text.as_bytes());
            let bundle_hash = sha256_hex(bundle_text.as_bytes());
            if input_hash != description.source.input_sha256
                || input_text.len() as u64 != description.source.input_bytes
            {
                binding
                    .fail("the exported input copy is not the input the description binds by hash");
            }
            if bundle_hash != description.source.bundle_sha256
                || bundle_text.len() as u64 != description.source.bundle_bytes
            {
                binding.fail(
                    "the exported bundle copy is not the bundle the description binds by hash",
                );
            }
            match ec::load_input(input_text) {
                Ok(input) => {
                    if input.scene_id != description.source.scene_id {
                        binding.fail(format!(
                            "the description scene {} and the input scene {} disagree",
                            description.source.scene_id, input.scene_id
                        ));
                    }
                    if input.units != description.source.units {
                        binding.fail("the description unit contract and the input disagree");
                    }
                    match serde_json::from_str::<EngineeringBundle>(bundle_text) {
                        Ok(bundle) => {
                            if bundle.scene_id != input.scene_id {
                                binding.fail(format!(
                                    "the input scene {} and the bundle scene {} are different sources",
                                    input.scene_id, bundle.scene_id
                                ));
                            }
                            if bundle.units != input.units {
                                binding.fail(
                                    "the bundle and the input declare different unit contracts",
                                );
                            }
                            if bundle.scene_id != description.scene_id {
                                binding.fail("the description scene and the bundle scene disagree");
                            }
                            if bundle.input_hash != description.source.bundle_input_hash {
                                binding.fail("the bundle's own input hash changed after export");
                            }
                            if bundle.payload_hash != description.source.bundle_payload_hash {
                                binding.fail("the bundle's own payload hash changed after export");
                            }
                            if bundle.publication.state != "committed" {
                                binding.fail(format!(
                                    "the bundle publication state is {}",
                                    bundle.publication.state
                                ));
                            }
                            if bundle.publication.contract_version
                                != ec::ENGINEERING_CONNECTIONS_VERSION
                            {
                                binding.fail(format!(
                                    "the bundle was committed at contract version {}",
                                    bundle.publication.contract_version
                                ));
                            }
                            let report = ec::verify_bundle(&input, &bundle);
                            if report.status != "verified" {
                                engineering.fail(format!(
                                    "the production verifier refused the exported source: {} with {} finding(s): {:?}",
                                    report.status,
                                    report.finding_count,
                                    report
                                        .findings
                                        .iter()
                                        .take(5)
                                        .map(|finding| format!("{} at {}", finding.code, finding.at))
                                        .collect::<Vec<_>>()
                                ));
                            }
                            if !report.payload_hash_matches {
                                engineering
                                    .fail("the bundle payload hash does not match its content");
                            }
                            if description.source.publication_state != bundle.publication.state {
                                binding.fail("the declared publication state is not the bundle's");
                            }
                            if report.status == "verified" {
                                engineering.note(format!(
                                    "production verify_bundle: verified, {} finding(s), payloadHashMatches {}, inputHashMatches {}",
                                    report.finding_count,
                                    report.payload_hash_matches,
                                    report.input_hash_matches
                                ));
                            }
                            verified = Some((input, bundle, report));
                        }
                        Err(error) => binding.fail(format!(
                            "the exported bundle copy is not a readable bundle: {error}"
                        )),
                    }
                }
                Err(error) => binding.fail(format!(
                    "the exported input copy is not a readable description: {}",
                    error.message
                )),
            }
            binding.note(format!(
                "input sha256 {input_hash}, bundle sha256 {bundle_hash} re-read from the export"
            ));
        }
        (None, _, _) => binding.fail("no description to bind the sources to"),
        _ => binding.fail("the exported source copies are missing"),
    }

    // --- renderParity --------------------------------------------------------
    match (&verified, render_model_json) {
        (Some((input, bundle, _)), Some(model_text)) => {
            match serde_json::from_str::<serde_json::Value>(model_text) {
                Err(error) => {
                    parity.fail(format!("the saved render model is not readable: {error}"))
                }
                Ok(saved_value) => match build_render_model(input, bundle) {
                    Err(error) => parity.fail(format!(
                        "the render model could not be recomputed from the saved bundle: {}",
                        error.message
                    )),
                    Ok(recomputed) => {
                        let recomputed_value =
                            serde_json::to_value(&recomputed).unwrap_or(serde_json::Value::Null);
                        let mut differences: Vec<String> = vec![];
                        diff_json(&recomputed_value, &saved_value, "$", &mut differences);
                        if !differences.is_empty() {
                            for difference in &differences {
                                parity.fail(format!("render projection differs: {difference}"));
                            }
                        }
                        // The topology and the display convention are re-checked on their own.
                        if let Err(problem) = check_box_topology() {
                            parity.fail(problem);
                        }
                        if recomputed.display_convention != display_convention() {
                            parity.fail("the saved display convention is not the declared identity convention");
                        }
                        if recomputed.counts.triangles
                            != recomputed.parts.len() * BOX_TRIANGLES.len()
                        {
                            parity.fail("the declared triangle count is not 12 per part");
                        }
                        if differences.is_empty() {
                            parity.note(format!(
                                "the saved projection equals the projection recomputed from the saved bundle: {} part(s), {} node(s), {} edge(s), {} route point(s), {} terrain cell(s), bounds {:?}",
                                recomputed.counts.parts,
                                recomputed.counts.nodes,
                                recomputed.counts.edges,
                                recomputed.counts.route_points,
                                recomputed.counts.terrain_cells,
                                recomputed.counts.bounds_m
                            ));
                        }
                    }
                },
            }
        }
        (None, _) => parity.fail("the render model cannot be checked without a verified source"),
        (_, None) => parity.fail(format!("{RENDER_MODEL_FILE} is missing")),
    }

    // --- description against its own verification facts ----------------------
    if let (Some(description), Some((_, bundle, report))) = (&description, &verified) {
        if description.verification.verify_status != report.status
            || description.verification.verify_finding_count != report.finding_count
            || description.verification.payload_hash_matches != report.payload_hash_matches
            || description.verification.route_status != bundle.route.status
        {
            binding.fail(
                "the description's recorded verification facts are not what the production verifier reports now",
            );
        }
        if description.render_model.sha256
            != render_model_json
                .map(|text| sha256_hex(text.as_bytes()))
                .unwrap_or_default()
        {
            binding.fail("the description's render model hash is not the saved render model's");
        }
    }

    let mut findings: Vec<String> = vec![];
    for section in [&engineering, &binding, &parity, &completeness] {
        for finding in &section.findings {
            findings.push(finding.clone());
        }
    }
    findings.retain(|finding| !finding.is_empty());
    let status = if engineering.status == "pass"
        && binding.status == "pass"
        && parity.status == "pass"
        && completeness.status == "pass"
    {
        "pass"
    } else {
        "fail"
    };

    AuditReport {
        schema: AUDIT_SCHEMA.into(),
        version: PREVIEW_VERSION,
        preview_root: preview_root.to_string(),
        status: status.into(),
        engineering_verification: engineering,
        source_binding: binding,
        render_parity: parity,
        preview_completeness: completeness,
        finding_count: findings.len(),
        findings,
        observed: serde_json::json!({
            "observedFiles": observed.len(),
            "sceneId": description.as_ref().map(|value| value.scene_id.clone()),
            "exportStatus": description.as_ref().map(|value| value.export_status.clone()),
        }),
    }
}

/// A written refusal for a preview that could not even be walked: every section fails with
/// the same reason, so a reader is never shown a green section for a package the audit
/// never managed to read.
pub fn refusal_report(preview_root: &str, message: &str) -> AuditReport {
    let section = || AuditSection {
        status: "fail".into(),
        detail: message.to_string(),
        findings: vec![message.to_string()],
    };
    AuditReport {
        schema: AUDIT_SCHEMA.into(),
        version: PREVIEW_VERSION,
        preview_root: preview_root.to_string(),
        status: "fail".into(),
        engineering_verification: section(),
        source_binding: section(),
        render_parity: section(),
        preview_completeness: section(),
        findings: vec![message.to_string()],
        finding_count: 1,
        observed: serde_json::json!({ "observedFiles": 0, "walkFailed": true }),
    }
}

/// Signed volume of the declared topology over an axis-aligned unit box, in the corner
/// order the production `MeshPart` uses (bottom quad 0..3, top quad 4..7).
fn box_topology_volume() -> f64 {
    let corners: [[f64; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let mut volume = 0.0_f64;
    for triangle in BOX_TRIANGLES {
        let a = corners[triangle[0] as usize];
        let b = corners[triangle[1] as usize];
        let c = corners[triangle[2] as usize];
        volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    volume
}

/// Check the declared box topology independently: no degenerate triangle, all eight corners
/// used, every undirected edge twice (a closed surface) and one consistent orientation, so
/// no triangle is wound against the rest.
///
/// The measured sign is inward for this corner order. That is the production table, not a
/// choice made here: the accepted packet's own `view-mesh.obj` winds the same way (its top
/// face normal points down). This module deliberately reuses the production topology rather
/// than inventing a second one, the viewer renders with `DoubleSide` so the sign cannot
/// shade incorrectly, and the projection therefore cannot disagree with the production mesh.
fn check_box_topology() -> Result<(), String> {
    let mut edges: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut used = [false; 8];
    for triangle in BOX_TRIANGLES {
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[0] == triangle[2] {
            return Err(format!("triangle {triangle:?} is degenerate"));
        }
        for corner in triangle {
            used[corner as usize] = true;
        }
        for pair in 0..3 {
            let (a, b) = (triangle[pair], triangle[(pair + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            *edges.entry(key).or_insert(0) += 1;
        }
    }
    if used.iter().any(|value| !value) {
        return Err("the box topology does not use all eight corners".into());
    }
    if let Some((edge, count)) = edges.iter().find(|(_, count)| **count != 2) {
        return Err(format!(
            "the box topology is not closed: edge {:?} is used {count} time(s)",
            edge
        ));
    }
    if edges.len() != 18 {
        return Err(format!(
            "the box topology has {} edges, expected 18",
            edges.len()
        ));
    }
    let volume = box_topology_volume();
    if (volume.abs() - 1.0).abs() > 1e-9 {
        return Err(format!(
            "the box topology is not one consistently oriented closed box (signed volume {volume:.6})"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn case(name: &str) -> String {
        std::fs::read_to_string(
            preview_root()
                .join("tests/resources/g19r2")
                .join(format!("{name}.json")),
        )
        .unwrap_or_else(|error| panic!("case {name}: {error}"))
    }

    #[test]
    fn g23_sha256_matches_the_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // A length that crosses both the 55-byte padding edge and the 64-byte block edge.
        assert_eq!(
            sha256_hex(&vec![b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_hex(&vec![b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
        let long = vec![b'a'; 1000];
        assert_eq!(
            sha256_hex(&long),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    #[test]
    fn g23_box_topology_is_a_closed_consistently_wound_box() {
        assert!(check_box_topology().is_ok(), "{:?}", check_box_topology());
        assert_eq!(BOX_TRIANGLES.len(), 12);
        // The measured sign is inward for the production corner order, and it is the same
        // sign the accepted packet's own OBJ mesh has. Recorded so a later change that
        // silently flips a face is visible rather than merely "still 12 triangles".
        let volume = box_topology_volume();
        assert!(
            (volume + 1.0).abs() < 1e-9,
            "the production box topology winds inward (signed volume {volume})"
        );
    }

    #[test]
    fn g23_export_binds_the_source_and_recomputes_identically() {
        let input_json = case("minimal-straight-stair");
        let (bundle, _) = ec::build_bundle(&ec::load_input(&input_json).unwrap()).unwrap();
        let mut bundle = bundle;
        ec::commit_bundle(&mut bundle, "bundle.json", "report.json").unwrap();
        let bundle_json = serde_json::to_string_pretty(&bundle).unwrap();

        let export = build_export(&input_json, &bundle_json).expect("the export builds");
        assert_eq!(export.description.schema, PREVIEW_SCHEMA);
        assert_eq!(export.description.export_status, "complete");
        assert_eq!(export.description.verification.verify_status, "verified");
        assert!(export.description.verification.payload_hash_matches);
        assert_eq!(
            export.description.source.bundle_sha256,
            sha256_hex(bundle_json.as_bytes())
        );
        assert_eq!(
            export.description.source.input_sha256,
            sha256_hex(input_json.as_bytes())
        );
        assert_ne!(
            export.description.source.bundle_payload_hash, export.description.source.bundle_sha256,
            "the bundle's own hash and the file hash are different units and must not be mixed"
        );
        // Every declared file is present in memory with the declared bytes and hash.
        for fact in &export.description.files {
            let bytes = match fact.path.as_str() {
                SOURCE_INPUT_FILE => &export.source_input,
                SOURCE_BUNDLE_FILE => &export.source_bundle,
                RENDER_MODEL_FILE => &export.render_model_json,
                other => export
                    .assets
                    .iter()
                    .find(|(path, _)| path == other)
                    .map(|(_, bytes)| bytes)
                    .unwrap_or_else(|| panic!("asset {other} is not embedded")),
            };
            assert_eq!(fact.bytes, bytes.len() as u64, "{}", fact.path);
            assert_eq!(fact.sha256, sha256_hex(bytes), "{}", fact.path);
        }
        assert!(
            export
                .description
                .files
                .iter()
                .any(|fact| fact.path == VIEWER_ENTRY_FILE && fact.role == "viewer")
        );

        // The render model carries the saved geometry one-to-one.
        let model: RenderModel = serde_json::from_slice(&export.render_model_json).unwrap();
        assert_eq!(model.scene_id, bundle.scene_id);
        assert_eq!(
            model.parts.len(),
            bundle
                .structures
                .iter()
                .map(|s| s.parts.len())
                .sum::<usize>()
        );
        for (part, saved) in model.parts.iter().zip(
            bundle
                .structures
                .iter()
                .flat_map(|structure| structure.parts.iter()),
        ) {
            assert_eq!(part.part_id, saved.part_id);
            assert_eq!(part.corners_m, saved.corners_m);
            assert_eq!(part.triangles, BOX_TRIANGLES);
        }
        assert_eq!(model.route.polyline_m, bundle.route.path_polyline_m);
        assert_eq!(
            model.terrain.engineered.heights_m.len(),
            bundle.surface.mm.len()
        );
        assert_eq!(
            model.terrain.natural.heights_m.len(),
            ec::load_input(&input_json).unwrap().grid.natural_mm.len()
        );
        assert_eq!(model.counts.triangles, model.parts.len() * 12);
        assert_eq!(model.display_convention.world_to_display, "identity");
    }

    #[test]
    fn g23_audit_passes_a_clean_export_and_refuses_every_rewrite() {
        let input_json = case("minimal-switchback");
        let (bundle, _) = ec::build_bundle(&ec::load_input(&input_json).unwrap()).unwrap();
        let mut bundle = bundle;
        ec::commit_bundle(&mut bundle, "bundle.json", "report.json").unwrap();
        let bundle_json = serde_json::to_string_pretty(&bundle).unwrap();
        let export = build_export(&input_json, &bundle_json).expect("the export builds");

        let mut observed: BTreeMap<String, (u64, String)> = BTreeMap::new();
        for fact in &export.description.files {
            let bytes = match fact.path.as_str() {
                SOURCE_INPUT_FILE => export.source_input.clone(),
                SOURCE_BUNDLE_FILE => export.source_bundle.clone(),
                RENDER_MODEL_FILE => export.render_model_json.clone(),
                other => export
                    .assets
                    .iter()
                    .find(|(path, _)| path == other)
                    .map(|(_, bytes)| bytes.clone())
                    .unwrap(),
            };
            observed.insert(fact.path.clone(), (bytes.len() as u64, sha256_hex(&bytes)));
        }
        observed.insert(
            DESCRIPTION_FILE.into(),
            (
                export.description_json.len() as u64,
                sha256_hex(&export.description_json),
            ),
        );
        let description_text = String::from_utf8(export.description_json.clone()).unwrap();

        let clean = audit_export(
            "clean",
            Some(&description_text),
            Some(&input_json),
            Some(&bundle_json),
            Some(std::str::from_utf8(&export.render_model_json).unwrap()),
            &observed,
        );
        assert_eq!(clean.status, "pass", "{:?}", clean.findings);
        assert!(clean.engineering_verification.status == "pass");
        assert!(clean.source_binding.status == "pass");
        assert!(clean.render_parity.status == "pass");
        assert!(clean.preview_completeness.status == "pass");

        // A rewritten render projection with every outer hash recomputed must still be
        // refused by renderParity alone.
        let mut model: serde_json::Value =
            serde_json::from_slice(&export.render_model_json).unwrap();
        model["parts"][0]["cornersM"][0][0] = serde_json::json!(999.0);
        model["parts"][0]["levelId"] = serde_json::json!("forged::level");
        let tampered_model = serde_json::to_string_pretty(&model).unwrap();
        let mut tampered_observed = observed.clone();
        tampered_observed.insert(
            RENDER_MODEL_FILE.into(),
            (
                tampered_model.len() as u64,
                sha256_hex(tampered_model.as_bytes()),
            ),
        );
        let mut tampered_description: serde_json::Value =
            serde_json::from_str(&description_text).unwrap();
        for fact in tampered_description["files"].as_array_mut().unwrap() {
            if fact["path"] == RENDER_MODEL_FILE {
                fact["bytes"] = serde_json::json!(tampered_model.len());
                fact["sha256"] = serde_json::json!(sha256_hex(tampered_model.as_bytes()));
            }
        }
        let tampered_description_text =
            serde_json::to_string_pretty(&tampered_description).unwrap();
        tampered_observed.insert(
            DESCRIPTION_FILE.into(),
            (
                tampered_description_text.len() as u64,
                sha256_hex(tampered_description_text.as_bytes()),
            ),
        );
        let caught = audit_export(
            "projection-tamper",
            Some(&tampered_description_text),
            Some(&input_json),
            Some(&bundle_json),
            Some(&tampered_model),
            &tampered_observed,
        );
        assert_eq!(caught.status, "fail");
        assert_eq!(caught.render_parity.status, "fail");
        assert!(
            caught
                .render_parity
                .findings
                .iter()
                .any(|finding| finding.contains("cornersM")),
            "{:?}",
            caught.render_parity.findings
        );
        assert!(
            caught
                .render_parity
                .findings
                .iter()
                .any(|finding| finding.contains("levelId")),
            "{:?}",
            caught.render_parity.findings
        );

        // A missing declared file is a completeness failure.
        let mut missing = observed.clone();
        missing.remove(VIEWER_ENTRY_FILE);
        let absent = audit_export(
            "missing-viewer",
            Some(&description_text),
            Some(&input_json),
            Some(&bundle_json),
            Some(std::str::from_utf8(&export.render_model_json).unwrap()),
            &missing,
        );
        assert_eq!(absent.status, "fail");
        assert_eq!(absent.preview_completeness.status, "fail");

        // A swapped source copy breaks the binding even when every outer hash still
        // matches the description.
        let other_input = case("minimal-straight-stair");
        let swapped = audit_export(
            "swapped-source",
            Some(&description_text),
            Some(&other_input),
            Some(&bundle_json),
            Some(std::str::from_utf8(&export.render_model_json).unwrap()),
            &observed,
        );
        assert_eq!(swapped.status, "fail");
        assert_eq!(swapped.source_binding.status, "fail");
        assert_eq!(
            swapped.engineering_verification.status, "fail",
            "a mismatched input must also fail the engineering verification, not pass quietly"
        );
    }

    #[test]
    fn g23_export_refuses_staged_and_broken_sources() {
        let input_json = case("minimal-switchback");
        let (bundle, _) = ec::build_bundle(&ec::load_input(&input_json).unwrap()).unwrap();
        let staged_json = serde_json::to_string_pretty(&bundle).unwrap();
        assert_eq!(
            build_export(&input_json, &staged_json)
                .expect_err("a staged bundle is not a publication")
                .category,
            "preview_bundle_not_committed"
        );

        let mut committed = bundle.clone();
        ec::commit_bundle(&mut committed, "bundle.json", "report.json").unwrap();
        let committed_json = serde_json::to_string_pretty(&committed).unwrap();
        assert!(build_export(&input_json, &committed_json).is_ok());

        // A reused scene id from a different scene is not the same source.
        let other = case("positive-terrace");
        assert!(build_export(&other, &committed_json).is_err());

        // A destroyed payload hash is refused before anything is written.
        let mut broken = committed.clone();
        broken.payload_hash = "0".repeat(64);
        let broken_json = serde_json::to_string_pretty(&broken).unwrap();
        let error =
            build_export(&input_json, &broken_json).expect_err("a broken payload hash is refused");
        assert!(
            error.category == "preview_source_not_verified"
                || error.category == "preview_payload_hash_mismatch",
            "{}",
            error.category
        );
    }

    /// The derived negative packages this packet's CLI matrix consumes. They are built here
    /// because a self-consistent bundle needs the production payload hash, which only the
    /// module itself computes; a normal test run writes nothing, and the export of the same
    /// fixtures is gated on an explicit environment variable with `create_new`.
    #[test]
    fn g23_derived_negative_packages_are_refused() {
        let input_json = case("minimal-switchback");
        let (bundle, _) = ec::build_bundle(&ec::load_input(&input_json).unwrap()).unwrap();
        let mut committed = bundle.clone();
        ec::commit_bundle(&mut committed, "bundle.json", "report.json").unwrap();
        let bundle_json = serde_json::to_string_pretty(&committed).unwrap();
        assert!(build_export(&input_json, &bundle_json).is_ok());

        // 1. a staged bundle is not a publication
        let mut staged = committed.clone();
        staged.publication.state = "staged".into();
        staged.payload_hash = String::new();
        let staged_json = serde_json::to_string_pretty(&staged).unwrap();
        let staged_error = build_export(&input_json, &staged_json).expect_err("staged is refused");
        assert_eq!(staged_error.category, "preview_bundle_not_committed");

        // 2. a damaged payload hash is refused on integrity, before any semantics
        let mut damaged = committed.clone();
        damaged.payload_hash = "0".repeat(64);
        let damaged_json = serde_json::to_string_pretty(&damaged).unwrap();
        let damaged_error =
            build_export(&input_json, &damaged_json).expect_err("a broken payload hash is refused");
        assert!(
            damaged_error.category == "preview_source_not_verified"
                || damaged_error.category == "preview_payload_hash_mismatch",
            "{}",
            damaged_error.category
        );

        // 3. the support-facts schema version removed and the payload hash recomputed: the
        //    production verifier refuses, so it is not the outer hash that protects this.
        let mut value: serde_json::Value = serde_json::from_str(&bundle_json).unwrap();
        let mut stripped = 0;
        if let Some(edges) = value["graph"]["edges"].as_array_mut() {
            for edge in edges {
                if let Some(facts) = edge["verification"]["detail"]["supportFacts"].as_object_mut()
                {
                    for fact in facts.values_mut() {
                        if let Some(object) = fact.as_object_mut() {
                            if object.remove("schemaVersion").is_some() {
                                stripped += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(stripped > 0, "the bundle saves support facts to strip");
        let mut missing_schema: EngineeringBundle = serde_json::from_value(value).unwrap();
        missing_schema.payload_hash = ec::payload_hash_of(&missing_schema).unwrap();
        let missing_json = serde_json::to_string_pretty(&missing_schema).unwrap();
        let missing_error = build_export(&input_json, &missing_json)
            .expect_err("a missing support schema is refused");
        assert_eq!(missing_error.category, "preview_source_not_verified");
        assert!(
            missing_error.message.contains("verified") || missing_error.message.contains("refused")
        );

        // 4. saved geometry rewritten and the payload hash recomputed: still refused by the
        //    designated support re-check, never by the hash the tamper recomputed.
        let mut geometry = committed.clone();
        let mut rewritten = 0;
        for structure in geometry.structures.iter_mut() {
            if let Some(part) = structure.parts.first_mut() {
                for corner in part.corners_m.iter_mut() {
                    corner[0] = corner[0] * 0.5 + 4.0;
                }
                rewritten += 1;
            }
            if rewritten > 0 {
                break;
            }
        }
        assert!(rewritten > 0);
        geometry.payload_hash = ec::payload_hash_of(&geometry).unwrap();
        let geometry_json = serde_json::to_string_pretty(&geometry).unwrap();
        let geometry_error =
            build_export(&input_json, &geometry_json).expect_err("rewritten geometry is refused");
        assert_eq!(geometry_error.category, "preview_source_not_verified");
        assert!(
            geometry_error
                .detail
                .to_string()
                .contains("stored_capabilities_resampled")
                || geometry_error.message.contains("refused"),
            "{}",
            geometry_error.message
        );

        // 5. a different scene is not the same source, whatever its file name
        let other = case("positive-terrace");
        assert!(build_export(&other, &bundle_json).is_err());

        if let Ok(dir) = std::env::var("DSH_G23_FIXTURE_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let fixtures: [(&str, &str); 5] = [
                ("fixtures/staged-bundle.json", &staged_json),
                ("fixtures/payload-damaged-bundle.json", &damaged_json),
                ("fixtures/missing-support-schema-bundle.json", &missing_json),
                ("fixtures/geometry-damaged-bundle.json", &geometry_json),
                ("fixtures/switchback-input.json", &input_json),
            ];
            for (name, text) in fixtures {
                let path = dir.join(name);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .unwrap_or_else(|error| panic!("create_new {}: {error}", path.display()));
                std::io::Write::write_all(&mut file, text.as_bytes()).unwrap();
            }
        }
    }

    // ---- G23R1: the format's required asset set ------------------------------------

    /// A clean in-memory export together with the observed-file map an audit would read from
    /// the disk, so a test can remove a file and its declaration the way the inspection did.
    struct CleanExport {
        input_json: String,
        bundle_json: String,
        export: PreviewExport,
        observed: BTreeMap<String, (u64, String)>,
        description_text: String,
    }

    fn asset_bytes(export: &PreviewExport, path: &str) -> Vec<u8> {
        match path {
            SOURCE_INPUT_FILE => export.source_input.clone(),
            SOURCE_BUNDLE_FILE => export.source_bundle.clone(),
            RENDER_MODEL_FILE => export.render_model_json.clone(),
            other => export
                .assets
                .iter()
                .find(|(candidate, _)| candidate == other)
                .map(|(_, bytes)| bytes.clone())
                .unwrap_or_else(|| panic!("asset {other} is not embedded")),
        }
    }

    fn clean_export(name: &str) -> CleanExport {
        let input_json = case(name);
        let (bundle, _) = ec::build_bundle(&ec::load_input(&input_json).unwrap()).unwrap();
        let mut bundle = bundle;
        ec::commit_bundle(&mut bundle, "bundle.json", "report.json").unwrap();
        let bundle_json = serde_json::to_string_pretty(&bundle).unwrap();
        let export = build_export(&input_json, &bundle_json).expect("the export builds");
        let mut observed: BTreeMap<String, (u64, String)> = BTreeMap::new();
        for fact in &export.description.files {
            let bytes = asset_bytes(&export, &fact.path);
            observed.insert(fact.path.clone(), (bytes.len() as u64, sha256_hex(&bytes)));
        }
        observed.insert(
            DESCRIPTION_FILE.into(),
            (
                export.description_json.len() as u64,
                sha256_hex(&export.description_json),
            ),
        );
        let description_text = String::from_utf8(export.description_json.clone()).unwrap();
        CleanExport {
            input_json,
            bundle_json,
            export,
            observed,
            description_text,
        }
    }

    /// Rewrite the description with one `files[]` entry deleted, and keep the observed map
    /// consistent with the rewritten description and the deleted file.
    fn remove_asset(
        fixture: &CleanExport,
        path: &str,
    ) -> (String, BTreeMap<String, (u64, String)>) {
        let mut value: serde_json::Value = serde_json::from_str(&fixture.description_text).unwrap();
        value["files"]
            .as_array_mut()
            .unwrap()
            .retain(|fact| fact["path"] != serde_json::json!(path));
        let text = serde_json::to_string_pretty(&value).unwrap();
        let mut observed = fixture.observed.clone();
        observed.remove(path);
        observed.insert(
            DESCRIPTION_FILE.into(),
            (text.len() as u64, sha256_hex(text.as_bytes())),
        );
        (text, observed)
    }

    fn audit_variant(
        label: &str,
        fixture: &CleanExport,
        description_text: &str,
        observed: &BTreeMap<String, (u64, String)>,
    ) -> AuditReport {
        audit_export(
            label,
            Some(description_text),
            Some(&fixture.input_json),
            Some(&fixture.bundle_json),
            Some(std::str::from_utf8(&fixture.export.render_model_json).unwrap()),
            observed,
        )
    }

    /// Replace one string field of the description and keep the observed map consistent.
    fn edit_description(
        fixture: &CleanExport,
        edit: impl FnOnce(&mut serde_json::Value),
    ) -> (String, BTreeMap<String, (u64, String)>) {
        let mut value: serde_json::Value = serde_json::from_str(&fixture.description_text).unwrap();
        edit(&mut value);
        let text = serde_json::to_string_pretty(&value).unwrap();
        let mut observed = fixture.observed.clone();
        observed.insert(
            DESCRIPTION_FILE.into(),
            (text.len() as u64, sha256_hex(text.as_bytes())),
        );
        (text, observed)
    }

    /// The format rule and the embedded asset table are one list: a drift between them would
    /// let the exported set and the required set disagree, which is exactly the class of gap
    /// this round closes.
    #[test]
    fn g23r1_required_asset_set_matches_the_embedded_assets() {
        let mut embedded: Vec<&str> = PREVIEW_ASSETS.iter().map(|(path, _)| *path).collect();
        embedded.sort_unstable();
        let mut required_viewer: Vec<&str> = REQUIRED_ASSETS
            .iter()
            .filter(|asset| {
                asset.role == ROLE_VIEWER
                    || asset.role == ROLE_VIEWER_MODULE
                    || asset.role == ROLE_VENDOR
            })
            .map(|asset| asset.path)
            .collect();
        required_viewer.sort_unstable();
        assert_eq!(
            embedded, required_viewer,
            "the required asset set and the embedded assets must be the same list"
        );
        assert_eq!(REQUIRED_ASSETS.len(), 10);
        assert_eq!(
            REQUIRED_ASSETS
                .iter()
                .filter(|asset| asset.role == ROLE_VENDOR)
                .count(),
            4
        );
        assert_eq!(
            required_asset(VIEWER_ENTRY_FILE).map(|asset| asset.role),
            Some(ROLE_VIEWER)
        );
        assert_eq!(
            required_asset("preview/vendor/three/build/three.core.js").map(|asset| asset.role),
            Some(ROLE_VENDOR)
        );
        assert!(required_asset("preview/extra.js").is_none());
        assert!(!is_canonical_relative_path("../escape.js"));
        assert!(!is_canonical_relative_path("/abs.js"));
        assert!(!is_canonical_relative_path("a\\b.js"));
        assert!(!is_canonical_relative_path("preview//x.js"));
        assert!(is_canonical_relative_path("preview/vendor/three/LICENSE"));

        // The exporter emits exactly that set for a clean package.
        let fixture = clean_export("minimal-switchback");
        assert_eq!(
            fixture.export.description.files.len(),
            REQUIRED_ASSETS.len()
        );
        for asset in REQUIRED_ASSETS {
            let matches: Vec<&PreviewFileFact> = fixture
                .export
                .description
                .files
                .iter()
                .filter(|fact| fact.path == asset.path)
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "{} must be declared exactly once",
                asset.path
            );
            assert_eq!(matches[0].role, asset.role, "{} role", asset.path);
        }
        let clean = audit_variant(
            "clean",
            &fixture,
            &fixture.description_text,
            &fixture.observed,
        );
        assert_eq!(
            clean.preview_completeness.status, "pass",
            "{:?}",
            clean.preview_completeness.findings
        );
    }

    /// The inspection's fixed counterexample: delete `preview/index.html` and the `files[]`
    /// entry that declared it, leaving `viewerEntry` pointing at it. The package stays
    /// self-consistent, so the engineering and projection sections must still pass and only
    /// completeness may refuse it.
    #[test]
    fn g23r1_removing_the_entry_and_its_declaration_fails_completeness() {
        let fixture = clean_export("minimal-switchback");
        let (text, observed) = remove_asset(&fixture, VIEWER_ENTRY_FILE);
        let report = audit_variant("missing-entry", &fixture, &text, &observed);
        assert_eq!(report.status, "fail");
        assert_eq!(report.engineering_verification.status, "pass");
        assert_eq!(report.source_binding.status, "pass");
        assert_eq!(report.render_parity.status, "pass");
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains(VIEWER_ENTRY_FILE)),
            "{:?}",
            report.preview_completeness.findings
        );
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("absent from disk")),
            "{:?}",
            report.preview_completeness.findings
        );
        assert!(report.finding_count > 0);
    }

    /// The same class for a vendored module: the entry is present, `three.core.js` and its
    /// declaration are gone, and completeness must name it.
    #[test]
    fn g23r1_removing_a_vendor_module_and_its_declaration_fails_completeness() {
        let fixture = clean_export("minimal-switchback");
        let path = "preview/vendor/three/build/three.core.js";
        let (text, observed) = remove_asset(&fixture, path);
        let report = audit_variant("missing-vendor", &fixture, &text, &observed);
        assert_eq!(report.status, "fail");
        assert_eq!(report.engineering_verification.status, "pass");
        assert_eq!(report.source_binding.status, "pass");
        assert_eq!(report.render_parity.status, "pass");
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains(path)),
            "{:?}",
            report.preview_completeness.findings
        );
    }

    /// Parameterised over the whole required set: removing any one file together with its
    /// declaration is refused by completeness, naming that path. Removing a source file
    /// legitimately also breaks the sections that consume it, so only the completeness
    /// outcome is pinned here.
    #[test]
    fn g23r1_every_required_asset_is_required_independently() {
        let fixture = clean_export("minimal-switchback");
        for asset in REQUIRED_ASSETS {
            let (text, observed) = remove_asset(&fixture, asset.path);
            let report = audit_variant(asset.path, &fixture, &text, &observed);
            assert_eq!(report.status, "fail", "{} must not pass", asset.path);
            assert_eq!(
                report.preview_completeness.status, "fail",
                "{} must fail completeness",
                asset.path
            );
            assert!(
                report
                    .preview_completeness
                    .findings
                    .iter()
                    .any(|finding| finding.contains(asset.path)),
                "{}: the finding must name the path: {:?}",
                asset.path,
                report.preview_completeness.findings
            );
        }
    }

    /// Duplicate declarations, a wrong role, a wrong entry reference and a wrong render-model
    /// reference are each refused on their own.
    #[test]
    fn g23r1_duplicate_wrong_role_and_wrong_reference_are_refused() {
        let fixture = clean_export("minimal-switchback");

        let (text, observed) = edit_description(&fixture, |value| {
            let first = value["files"].as_array().unwrap()[0].clone();
            value["files"].as_array_mut().unwrap().push(first);
        });
        let report = audit_variant("duplicate", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("declared 2 times")),
            "{:?}",
            report.preview_completeness.findings
        );

        let (text, observed) = edit_description(&fixture, |value| {
            for fact in value["files"].as_array_mut().unwrap() {
                if fact["path"] == serde_json::json!(VIEWER_ENTRY_FILE) {
                    fact["role"] = serde_json::json!(ROLE_VENDOR);
                }
            }
        });
        let report = audit_variant("wrong-role", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("declared role")),
            "{:?}",
            report.preview_completeness.findings
        );

        let (text, observed) = edit_description(&fixture, |value| {
            value["viewerEntry"] = serde_json::json!("preview/other.html");
        });
        let report = audit_variant("wrong-entry", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("viewer entry")),
            "{:?}",
            report.preview_completeness.findings
        );

        let (text, observed) = edit_description(&fixture, |value| {
            value["renderModel"]["sha256"] = serde_json::json!("0".repeat(64));
        });
        let report = audit_variant("wrong-reference", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("renderModel reference")),
            "{:?}",
            report.preview_completeness.findings
        );
    }

    /// An unrecognised format is refused rather than validated against a rule set that does not
    /// describe it, and an entry the format does not define cannot be invented.
    #[test]
    fn g23r1_unknown_schema_version_and_extra_entries_are_refused() {
        let fixture = clean_export("minimal-switchback");

        let (text, observed) = edit_description(&fixture, |value| {
            value["schema"] = serde_json::json!("glyphweave.engineering-preview/9");
        });
        let report = audit_variant("unknown-schema", &fixture, &text, &observed);
        assert_eq!(report.status, "fail");
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("unsupported preview schema")),
            "{:?}",
            report.preview_completeness.findings
        );

        let (text, observed) = edit_description(&fixture, |value| {
            value["version"] = serde_json::json!(99);
        });
        let report = audit_variant("unknown-version", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("unsupported preview version")),
            "{:?}",
            report.preview_completeness.findings
        );

        let extra = sha256_hex(b"abc");
        let (text, mut observed) = edit_description(&fixture, |value| {
            value["files"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "path": "preview/extra.js",
                    "role": ROLE_VIEWER_MODULE,
                    "bytes": 3,
                    "sha256": sha256_hex(b"abc"),
                    "objects": 0,
                }));
        });
        observed.insert("preview/extra.js".into(), (3, extra));
        let report = audit_variant("extra-entry", &fixture, &text, &observed);
        assert_eq!(report.preview_completeness.status, "fail");
        assert!(
            report
                .preview_completeness
                .findings
                .iter()
                .any(|finding| finding.contains("defines no such asset")),
            "{:?}",
            report.preview_completeness.findings
        );
    }
}
