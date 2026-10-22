//! `saved-context-preview`: an explicit, optional project entry for an audited
//! G25 context-preview package.
//!
//! The user must name the package explicitly; the entry never scans the
//! workspace for a package. It validates the package read-only, then serves an
//! entry page that states the recorded source facts and embeds the package's own
//! G25 2D viewer. Nothing is written into the package or the source root, and a
//! package that fails validation never starts a server, so no stale model can be
//! shown.
//!
//! This is the path A contract: a read-only 2D context projection. The entry
//! provides no WebGL/3D rendering, no engineering feasibility and no route
//! safety proof.

use super::engineering_context::check_report_path;
use glyphweave_core::engineering_context::resolve_within_root;
use glyphweave_core::engineering_preview::sha256_hex;
use serde_json::{Value, json};
use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

type CliResult<T> = Result<T, Box<dyn Error>>;

const ENTRY_SCHEMA: &str = "saved-context-preview-entry-v1";
const PREVIEW_SCHEMA: &str = "engineering-context-preview-v1";
const MODEL_FILE: &str = "render-model.json";
const MARKER_FILE: &str = "COMPLETE.json";
const MANIFEST_FILE: &str = "manifest.json";
const VIEWER_FILE: &str = "viewer.js";
const HTML_FILE: &str = "preview.html";
const README_FILE: &str = "README.md";
const REQUIRED_FILES: [&str; 6] = [
    MODEL_FILE,
    MARKER_FILE,
    MANIFEST_FILE,
    VIEWER_FILE,
    HTML_FILE,
    README_FILE,
];
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_PORT: u16 = 8791;

pub(super) fn run(args: &[String]) -> CliResult<()> {
    let mut preview: Option<PathBuf> = None;
    let mut port = DEFAULT_PORT;
    let mut report: Option<PathBuf> = None;
    let mut expect_scene: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--preview" => {
                preview = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--port" => {
                let raw = args
                    .get(index + 1)
                    .ok_or("--port needs a value")?
                    .parse::<u16>()
                    .map_err(|_| "--port must be a port number")?;
                port = raw;
                index += 2;
            }
            "--report" => {
                report = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--expect-scene" => {
                expect_scene = args.get(index + 1).cloned();
                index += 2;
            }
            other => return Err(format!("unrecognised argument {other}").into()),
        }
    }
    // Explicit selection only: a missing path is a usage error, never a scan.
    let preview = preview.ok_or(
        "saved-context-preview requires an explicit --preview DIR (this entry never scans for packages)",
    )?;
    // The optional report/log path must not overwrite an input.
    if let Some(report) = &report {
        check_report_path(report, &preview, &preview)?;
    }
    let facts = inspect_package(&preview).map_err(|detail| -> Box<dyn Error> {
        format!(
            "saved-context-preview refused {}: {detail}",
            preview.display()
        )
        .into()
    })?;
    if let Some(expected) = &expect_scene {
        if facts.scene_id.as_deref() != Some(expected.as_str()) {
            return Err(format!(
                "refused: the package scene is {:?}, not the expected {expected}",
                facts.scene_id
            )
            .into());
        }
    }
    if let Some(report) = &report {
        if let Some(parent) = report.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(report, serde_json::to_vec_pretty(&facts.as_json())?)?;
    }
    println!("entry contract: path A read-only 2D context projection");
    println!("package: {}", preview.display());
    match facts.window {
        Some((min_x, min_z, width_m, depth_m)) => println!(
            "scene: {:?} window: {min_x},{min_z},{width_m},{depth_m} completeness: complete",
            facts.scene_id
        ),
        None => println!(
            "scene: {:?} window: none completeness: complete",
            facts.scene_id
        ),
    }
    println!(
        "counts: entities={} roads={} access={} observationOnly={}",
        facts.entity_count, facts.road_count, facts.access_count, facts.observation_only_count
    );
    serve(preview, port, facts)
}

#[derive(Debug, Clone)]
struct PackageFacts {
    package: String,
    scene_id: Option<String>,
    window: Option<(i64, i64, i64, i64)>,
    world_path: Option<String>,
    scene_path: Option<String>,
    world_sha256: Option<String>,
    scene_sha256: Option<String>,
    context_sha256: Option<String>,
    manifest_sha256: String,
    marker_sha256: String,
    model_sha256: String,
    schema_version: Option<String>,
    natural_equals_final: bool,
    differing_samples: Option<i64>,
    entity_count: i64,
    road_count: i64,
    access_count: i64,
    access_in_window: i64,
    reference_only_access: i64,
    observation_only_count: i64,
    unmodelled_kind_count: usize,
    schema_drift: Option<String>,
    limitations: Vec<String>,
    files: Vec<String>,
}

impl PackageFacts {
    fn as_json(&self) -> Value {
        json!({
            "schemaVersion": ENTRY_SCHEMA,
            "package": self.package,
            "completeness": "complete",
            "sceneId": self.scene_id,
            "window": self.window.map(|w| json!([w.0, w.1, w.2, w.3])),
            "sources": {
                "world": {"path": self.world_path, "sha256": self.world_sha256},
                "scene": {"path": self.scene_path, "sha256": self.scene_sha256},
                "context": {"sha256": self.context_sha256},
                "manifest": {"sha256": self.manifest_sha256},
                "marker": {"sha256": self.marker_sha256},
                "model": {"sha256": self.model_sha256},
            },
            "modelSchemaVersion": self.schema_version,
            "naturalEqualsFinal": self.natural_equals_final,
            "differingSamples": self.differing_samples,
            "counts": {
                "entities": self.entity_count,
                "roads": self.road_count,
                "access": self.access_count,
                "accessInWindow": self.access_in_window,
                "referenceOnlyAccess": self.reference_only_access,
                "observationOnly": self.observation_only_count,
            },
            "unmodelledKindCount": self.unmodelled_kind_count,
            "schemaDrift": self.schema_drift,
            "limitations": self.limitations,
            "files": self.files,
            "contract": "path A read-only 2D context projection; no WebGL/3D, no engineering feasibility, no route safety proof",
        })
    }
}

fn read_package_file(root: &Path, name: &str, budget: &mut u64) -> Result<Vec<u8>, String> {
    let resolved = resolve_within_root(root, name)
        .map_err(|error| format!("{name} is not a confined path: {error}"))?;
    let metadata =
        fs::metadata(&resolved).map_err(|error| format!("{name} is unreadable: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{name} is not a regular file"));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("{name} exceeds the per-file byte cap"));
    }
    *budget = budget.saturating_add(metadata.len());
    fs::read(&resolved).map_err(|error| format!("{name} is unreadable: {error}"))
}

fn object(bytes: &[u8], name: &str) -> Result<Value, String> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(_) => Err(format!("{name} is not a JSON object")),
        Err(error) => Err(format!("{name} is not readable JSON: {error}")),
    }
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::to_string)
}

/// Read-only completeness check of an explicitly selected package: closed file
/// set, confined paths, regular files, schema version, and the marker -> model
/// and marker -> manifest hash chain.
fn inspect_package(package: &Path) -> Result<PackageFacts, String> {
    let root =
        fs::canonicalize(package).map_err(|error| format!("the package is unusable: {error}"))?;
    if !root.is_dir() {
        return Err("the package is not a directory".to_string());
    }
    let mut files: Vec<String> = Vec::new();
    match fs::read_dir(&root) {
        Ok(entries) => {
            for entry in entries {
                let entry =
                    entry.map_err(|error| format!("cannot read a directory entry: {error}"))?;
                let name = entry.file_name().to_string_lossy().to_string();
                if !REQUIRED_FILES.contains(&name.as_str()) {
                    return Err(format!("undeclared file {name} is present in the package"));
                }
                let file_type = entry
                    .file_type()
                    .map_err(|error| format!("cannot inspect {name}: {error}"))?;
                if !file_type.is_file() {
                    return Err(format!("{name} is not a regular file"));
                }
                files.push(name);
            }
        }
        Err(error) => return Err(format!("the package directory is unreadable: {error}")),
    }
    files.sort();
    for name in REQUIRED_FILES {
        if !files.iter().any(|present| present == name) {
            return Err(format!("required file {name} is missing"));
        }
    }
    let mut budget = 0u64;
    let model_bytes = read_package_file(&root, MODEL_FILE, &mut budget)?;
    let manifest_bytes = read_package_file(&root, MANIFEST_FILE, &mut budget)?;
    let marker_bytes = read_package_file(&root, MARKER_FILE, &mut budget)?;
    // The viewer assets are read only to prove they exist and are non-empty.
    for name in [VIEWER_FILE, HTML_FILE, README_FILE] {
        let bytes = read_package_file(&root, name, &mut budget)?;
        if bytes.is_empty() {
            return Err(format!("{name} is empty"));
        }
    }
    let model = object(&model_bytes, MODEL_FILE)?;
    let manifest = object(&manifest_bytes, MANIFEST_FILE)?;
    let marker = object(&marker_bytes, MARKER_FILE)?;
    let model_sha256 = sha256_hex(&model_bytes);
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let marker_sha256 = sha256_hex(&marker_bytes);
    if marker.get("status").and_then(Value::as_str) != Some("complete") {
        return Err("the completion marker is not complete".to_string());
    }
    if string_at(&marker, &["modelSha256"]).as_deref() != Some(model_sha256.as_str()) {
        return Err("the marker does not hash the render model".to_string());
    }
    if string_at(&marker, &["manifestSha256"]).as_deref() != Some(manifest_sha256.as_str()) {
        return Err("the marker does not hash the manifest".to_string());
    }
    if string_at(&manifest, &["modelSha256"]).as_deref() != Some(model_sha256.as_str()) {
        return Err("the manifest does not hash the render model".to_string());
    }
    match string_at(&model, &["schemaVersion"]).as_deref() {
        Some(PREVIEW_SCHEMA) => {}
        Some(other) => {
            return Err(format!(
                "unsupported render model schemaVersion {other}; expected {PREVIEW_SCHEMA}"
            ));
        }
        None => return Err("the render model declares no schemaVersion".to_string()),
    }
    let status = model.get("status").cloned().unwrap_or(Value::Null);
    let number = |value: &Value, key: &str| value.get(key).and_then(Value::as_i64).unwrap_or(0);
    let window = model.get("window").and_then(|window| {
        Some((
            window.get("minX")?.as_i64()?,
            window.get("minZ")?.as_i64()?,
            window.get("widthM")?.as_i64()?,
            window.get("depthM")?.as_i64()?,
        ))
    });
    let access_records = model
        .get("access")
        .and_then(|access| access.get("records"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let observation_only_count = access_records
        .iter()
        .filter(|record| {
            record.get("sourceObservationOnly").and_then(Value::as_bool) == Some(true)
                && record.get("notReverified").and_then(Value::as_bool) == Some(true)
        })
        .count() as i64;
    let unmodelled_kind_count = model
        .get("unmodelledKinds")
        .and_then(Value::as_object)
        .map(|map| map.len())
        .unwrap_or(0);
    Ok(PackageFacts {
        package: root.display().to_string(),
        scene_id: string_at(&model, &["sources", "sceneId"]),
        window,
        world_path: string_at(&model, &["sources", "world", "path"]),
        scene_path: string_at(&model, &["sources", "scene", "path"]),
        world_sha256: string_at(&model, &["sources", "world", "sha256"]),
        scene_sha256: string_at(&model, &["sources", "scene", "sha256"]),
        context_sha256: string_at(&model, &["sources", "context", "sha256"]),
        manifest_sha256,
        marker_sha256,
        model_sha256,
        schema_version: string_at(&model, &["schemaVersion"]),
        natural_equals_final: status
            .get("naturalEqualsFinal")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        differing_samples: model
            .get("surfaces")
            .and_then(|surfaces| surfaces.get("differingSamples"))
            .and_then(Value::as_i64),
        entity_count: number(&status, "entityCount"),
        road_count: number(&status, "roadCount"),
        access_count: number(&status, "accessRecordCount"),
        access_in_window: number(&status, "accessRecordsInWindow"),
        reference_only_access: number(&status, "referenceOnlyAccessRecords"),
        observation_only_count,
        unmodelled_kind_count,
        schema_drift: model
            .get("schemaDrift")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str)
            .map(str::to_string),
        limitations: model
            .get("limitations")
            .and_then(Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        files,
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The entry page: states the recorded source facts and embeds the package's own
/// viewer through a same-origin iframe, so the viewer files are served exactly
/// as delivered and never rewritten.
fn entry_page(facts: &PackageFacts) -> String {
    let short = |value: &Option<String>| {
        value
            .as_deref()
            .map(|text| text.chars().take(16).collect::<String>())
            .unwrap_or_else(|| "none".to_string())
    };
    let window = facts
        .window
        .map(|w| format!("{},{},{},{}", w.0, w.1, w.2, w.3))
        .unwrap_or_else(|| "none".to_string());
    let drift = facts
        .schema_drift
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let limitations = facts
        .limitations
        .iter()
        .map(|line| format!("<li>{}</li>", escape(line)))
        .collect::<String>();
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Saved-scene context preview entry</title>
<style>
 body {{ margin:0; font:13px/1.45 ui-monospace, Consolas, monospace; background:#14171c; color:#d8dee9; }}
 header {{ padding:10px 14px; border-bottom:1px solid #2b3038; }}
 main {{ display:flex; height:calc(100vh - 210px); }}
 #facts {{ width:420px; padding:10px 14px; border-right:1px solid #2b3038; overflow:auto; }}
 #view {{ flex:1; }}
 iframe {{ width:100%; height:100%; border:0; background:#0f1216; }}
 h1 {{ font-size:14px; margin:0 0 6px; }}
 h2 {{ font-size:12px; margin:10px 0 4px; color:#8fa1b3; text-transform:uppercase; letter-spacing:.04em; }}
 table {{ border-collapse:collapse; width:100%; }}
 td {{ padding:1px 4px; vertical-align:top; }}
 td:first-child {{ color:#8fa1b3; white-space:nowrap; }}
 .badge {{ display:inline-block; padding:1px 6px; border-radius:8px; background:#2b3038; margin-right:4px; }}
 .warn {{ color:#f0b849; }} .ok {{ color:#7fd18b; }} ul {{ margin:4px 0; padding-left:18px; }}
</style></head>
<body>
<header>
 <h1>Saved-scene context preview entry</h1>
 <div>contract: <span class="badge">path A</span> read-only 2D context projection - no WebGL/3D, no engineering feasibility, no route safety proof</div>
 <div>load status: <span class="ok" id="load-status">complete</span> <span id="load-error"></span></div>
</header>
<main>
 <div id="facts">
  <h2>Source</h2>
  <table>
   <tr><td>package</td><td id="f-package">{package}</td></tr>
   <tr><td>sceneId</td><td id="f-scene">{scene}</td></tr>
   <tr><td>window</td><td id="f-window">{window}</td></tr>
   <tr><td>world</td><td id="f-world">{world_path}</td></tr>
   <tr><td>scene path</td><td id="f-scenepath">{scene_path}</td></tr>
   <tr><td>world sha256</td><td id="f-worldsha">{world_sha}</td></tr>
   <tr><td>scene sha256</td><td id="f-scenesha">{scene_sha}</td></tr>
   <tr><td>context sha256</td><td id="f-contextsha">{context_sha}</td></tr>
   <tr><td>manifest sha256</td><td id="f-manifestsha">{manifest_sha}</td></tr>
   <tr><td>marker sha256</td><td id="f-markersha">{marker_sha}</td></tr>
   <tr><td>model sha256</td><td id="f-modelsha">{model_sha}</td></tr>
   <tr><td>model schema</td><td id="f-schema">{schema}</td></tr>
  </table>
  <h2>State</h2>
  <table>
   <tr><td>natural == final</td><td id="f-natural">{natural}</td></tr>
   <tr><td>differing samples</td><td id="f-differing">{differing}</td></tr>
   <tr><td>entities</td><td id="f-entities">{entities}</td></tr>
   <tr><td>roads</td><td id="f-roads">{roads}</td></tr>
   <tr><td>access records</td><td id="f-access">{access}</td></tr>
   <tr><td>access in window</td><td id="f-accesswindow">{access_window}</td></tr>
   <tr><td>referenceOnly access</td><td id="f-reference">{reference}</td></tr>
   <tr><td>observation-only labels</td><td id="f-observation">{observation}</td></tr>
   <tr><td>source fact / not converted kinds</td><td id="f-unmodelled">{unmodelled}</td></tr>
   <tr><td>schema drift</td><td id="f-drift" class="{drift_class}">{drift}</td></tr>
  </table>
  <div><span class="badge">sourceObservationOnly</span><span class="badge">notReverified</span> every saved access record</div>
  <h2>Limitations</h2>
  <ul id="f-limitations">{limitations}</ul>
 </div>
 <div id="view"><iframe id="viewer" src="/package/preview.html" title="read-only 2D context projection"></iframe></div>
</main>
<script>
 window.addEventListener('error', function (event) {{
   document.getElementById('load-status').textContent = 'failed';
   document.getElementById('load-status').className = 'warn';
   document.getElementById('load-error').textContent = 'the viewer failed to load; no model is displayed';
 }});
 window.__ENTRY_STATE__ = {{
   entrySchema: '{entry_schema}',
   completeness: 'complete',
   sceneId: document.getElementById('f-scene').textContent,
   window: document.getElementById('f-window').textContent,
   contract: 'path A read-only 2D context projection',
   viewport: 'iframe'
 }};
</script>
</body></html>
"##,
        package = escape(&facts.package),
        scene = escape(facts.scene_id.as_deref().unwrap_or("none")),
        window = escape(&window),
        world_path = escape(facts.world_path.as_deref().unwrap_or("none")),
        scene_path = escape(facts.scene_path.as_deref().unwrap_or("none")),
        world_sha = short(&facts.world_sha256),
        scene_sha = short(&facts.scene_sha256),
        context_sha = short(&facts.context_sha256),
        manifest_sha = short(&Some(facts.manifest_sha256.clone())),
        marker_sha = short(&Some(facts.marker_sha256.clone())),
        model_sha = short(&Some(facts.model_sha256.clone())),
        schema = escape(facts.schema_version.as_deref().unwrap_or("none")),
        natural = facts.natural_equals_final,
        differing = facts.differing_samples.unwrap_or(0),
        entities = facts.entity_count,
        roads = facts.road_count,
        access = facts.access_count,
        access_window = facts.access_in_window,
        reference = facts.reference_only_access,
        observation = facts.observation_only_count,
        unmodelled = facts.unmodelled_kind_count,
        drift = escape(&drift),
        drift_class = if facts.schema_drift.is_some() {
            "warn"
        } else {
            "ok"
        },
        limitations = limitations,
        entry_schema = ENTRY_SCHEMA,
    )
}

fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// Serves the entry page, the recorded facts and the package files themselves.
/// Only GET and HEAD are accepted, and package paths stay inside the package
/// root, so the entry can never write to or escape the selected package.
fn serve(package: PathBuf, port: u16, facts: PackageFacts) -> CliResult<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("cannot bind 127.0.0.1:{port}: {error}"))?;
    let page = entry_page(&facts);
    let status_json = serde_json::to_vec_pretty(&facts.as_json())?;
    println!("serving: http://127.0.0.1:{port}/");
    println!("viewer: http://127.0.0.1:{port}/package/{HTML_FILE}");
    println!("facts:  http://127.0.0.1:{port}/status.json");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(&stream);
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let target = parts.next().unwrap_or("/").to_string();
        let path = target.split('?').next().unwrap_or("/").to_string();
        if method != "GET" && method != "HEAD" {
            respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"read-only entry",
            );
            continue;
        }
        let body: Option<(String, Vec<u8>)> = if path == "/" || path == "/index.html" {
            Some((
                "text/html; charset=utf-8".to_string(),
                page.clone().into_bytes(),
            ))
        } else if path == "/status.json" {
            Some((
                "application/json; charset=utf-8".to_string(),
                status_json.clone(),
            ))
        } else if let Some(relative) = path.strip_prefix("/package/") {
            match resolve_within_root(&package, relative) {
                Ok(resolved) if resolved.is_file() => match fs::read(&resolved) {
                    Ok(bytes) => Some((content_type(relative).to_string(), bytes)),
                    Err(_) => None,
                },
                _ => None,
            }
        } else {
            None
        };
        match body {
            Some((content_type, bytes)) => {
                respond(&mut stream, "200 OK", &content_type, &bytes);
            }
            None => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
        }
    }
    Ok(())
}

/// Verdict for the port a handoff wants to use.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PortVerdict {
    /// Nothing is listening: the handoff may serve.
    Free,
    /// The same package is already served: no second service is started.
    SamePackage,
    /// A different package is served: refuse rather than switch silently.
    OtherPackage(String),
}

/// Pure decision so the port ownership rule is testable without the network.
fn port_verdict(probe: Option<String>, selected_model_sha256: &str) -> PortVerdict {
    match probe {
        None => PortVerdict::Free,
        Some(served) if served == selected_model_sha256 => PortVerdict::SamePackage,
        Some(served) => PortVerdict::OtherPackage(served),
    }
}

/// Reads the render-model hash a running entry serves on `port`, if any.
fn probe_entry(port: u16) -> Option<String> {
    use std::io::Read;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let request = "GET /status.json HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    let text = String::from_utf8_lossy(&response);
    let body = text.split("\r\n\r\n").nth(1)?;
    let value: Value = serde_json::from_str(body).ok()?;
    string_at(&value, &["sources", "model", "sha256"])
}

/// The explicit operator handoff. The package path must come from the operator;
/// the handoff never scans the workspace, validates before it opens anything,
/// refuses to silently switch a busy port to another package, and starts no
/// second service for a package that is already being served.
pub(super) fn run_handoff(args: &[String]) -> CliResult<()> {
    let mut preview: Option<PathBuf> = None;
    let mut port = DEFAULT_PORT;
    let mut report: Option<PathBuf> = None;
    let mut expect_scene: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--preview" => {
                preview = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--port" => {
                port = args
                    .get(index + 1)
                    .ok_or("--port needs a value")?
                    .parse::<u16>()
                    .map_err(|_| "--port must be a port number")?;
                index += 2;
            }
            "--report" => {
                report = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--expect-scene" => {
                expect_scene = args.get(index + 1).cloned();
                index += 2;
            }
            other => return Err(format!("unrecognised argument {other}").into()),
        }
    }
    let preview = preview.ok_or(
        "saved-context-preview-handoff requires an explicit --preview DIR chosen by the operator (this handoff never scans the workspace)",
    )?;
    if let Some(report) = &report {
        check_report_path(report, &preview, &preview)?;
    }
    let facts = inspect_package(&preview).map_err(|detail| -> Box<dyn Error> {
        format!(
            "handoff refused {detail}: nothing is opened and the previously displayed package is not kept"
        )
        .into()
    })?;
    if let Some(expected) = &expect_scene {
        if facts.scene_id.as_deref() != Some(expected.as_str()) {
            return Err(format!(
                "refused: the package scene is {:?}, not the expected {expected}",
                facts.scene_id
            )
            .into());
        }
    }
    if let Some(report) = &report {
        if let Some(parent) = report.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(report, serde_json::to_vec_pretty(&facts.as_json())?)?;
    }
    println!(
        "handoff contract: path A read-only 2D context projection; no WebGL/3D, no engineering feasibility, no route safety proof"
    );
    match port_verdict(probe_entry(port), &facts.model_sha256) {
        PortVerdict::SamePackage => {
            println!(
                "handoff: port {port} already serves this same package; no second service was started"
            );
            println!("entry: http://127.0.0.1:{port}/");
            Ok(())
        }
        PortVerdict::OtherPackage(served) => Err(format!(
            "refused: port {port} already serves a different package (model {}); stop it or choose another port. The existing display is unchanged and no silent switch was performed",
            &served[..served.len().min(12)]
        )
        .into()),
        PortVerdict::Free => {
            println!("selected package: {}", preview.display());
            println!("scene: {:?} window: {:?}", facts.scene_id, facts.window);
            println!(
                "counts: entities={} roads={} access={} observationOnly={}",
                facts.entity_count, facts.road_count, facts.access_count, facts.observation_only_count
            );
            println!(
                "naturalEqualsFinal: {} differingSamples: {:?}",
                facts.natural_equals_final, facts.differing_samples
            );
            println!(
                "schema drift: {}",
                facts.schema_drift.clone().unwrap_or_else(|| "none".to_string())
            );
            let short = |value: &Option<String>| {
                value
                    .as_deref()
                    .map(|text| text.chars().take(12).collect::<String>())
                    .unwrap_or_else(|| "none".to_string())
            };
            println!(
                "sources: world={} scene={} context={} model={}",
                short(&facts.world_sha256),
                short(&facts.scene_sha256),
                short(&facts.context_sha256),
                short(&Some(facts.model_sha256.clone()))
            );
            println!(
                "handoff: validation complete for {} required file(s); opening the read-only entry",
                facts.files.len()
            );
            serve(preview, port, facts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "gwc-entry-{}-{}-{}",
            std::process::id(),
            tag,
            nanos
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn g25_package() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../bevy/work/collaboration/ds-b01-g25-context-preview-consumer/runs/p16-final-package/package",
        )
    }

    /// Copies the delivered G25 package so tests only ever write their own copy.
    fn copied_package(tag: &str) -> PathBuf {
        let source = g25_package();
        let target = temp_dir(tag);
        for name in REQUIRED_FILES {
            fs::copy(source.join(name), target.join(name)).expect("copy");
        }
        target
    }

    fn load_facts(tag: &str) -> PackageFacts {
        inspect_package(&copied_package(tag)).expect("a delivered package must load")
    }

    #[test]
    fn a_delivered_package_loads_with_its_source_facts() {
        let facts = load_facts("ok");
        assert_eq!(facts.scene_id.as_deref(), Some("scene-0"));
        assert_eq!(facts.window, Some((256, 32, 64, 64)));
        assert_eq!(facts.schema_version.as_deref(), Some(PREVIEW_SCHEMA));
        assert!(facts.world_sha256.is_some() && facts.scene_sha256.is_some());
        assert_eq!(facts.entity_count, 4);
        assert_eq!(facts.road_count, 3);
        assert_eq!(facts.access_count, 18);
        assert_eq!(facts.observation_only_count, 18);
        assert!(
            facts.schema_drift.is_some(),
            "drift must be shown, not hidden"
        );
        assert!(!facts.limitations.is_empty());
        assert_eq!(facts.files.len(), REQUIRED_FILES.len());
    }

    #[test]
    fn the_entry_page_states_the_required_facts_and_embeds_the_delivered_viewer() {
        let facts = load_facts("page");
        let page = entry_page(&facts);
        for needle in [
            "path A",
            "read-only 2D context projection",
            "scene-0",
            "256,32,64,64",
            "sourceObservationOnly",
            "notReverified",
            "schema drift",
            "natural == final",
            "entities",
            "roads",
            "access records",
            "/package/preview.html",
        ] {
            assert!(page.contains(needle), "the entry page must state {needle}");
        }
        assert!(page.contains(&facts.model_sha256.chars().take(16).collect::<String>()));
        assert!(!page.contains("WebGL renderer"));
    }

    #[test]
    fn missing_files_wrong_hashes_and_unknown_versions_are_refused() {
        // Missing marker.
        let dir = copied_package("no-marker");
        fs::remove_file(dir.join(MARKER_FILE)).expect("remove marker");
        assert!(inspect_package(&dir).is_err());

        // Manifest hash mismatch.
        let dir = copied_package("bad-manifest");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read"))
                .expect("json");
        manifest["modelSha256"] = json!("0".repeat(64));
        fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).expect("json"),
        )
        .expect("write");
        assert!(inspect_package(&dir).is_err());

        // Render model hash mismatch.
        let dir = copied_package("bad-model");
        let mut model: Value =
            serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("read")).expect("json");
        model["status"]["entityCount"] = json!(99);
        fs::write(
            dir.join(MODEL_FILE),
            serde_json::to_vec_pretty(&model).expect("json"),
        )
        .expect("write");
        assert!(inspect_package(&dir).is_err());

        // Unknown schema version.
        let dir = copied_package("bad-version");
        let mut model: Value =
            serde_json::from_slice(&fs::read(dir.join(MODEL_FILE)).expect("read")).expect("json");
        model["schemaVersion"] = json!("engineering-context-preview-v999");
        let bytes = serde_json::to_vec_pretty(&model).expect("json");
        fs::write(dir.join(MODEL_FILE), &bytes).expect("write");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(dir.join(MANIFEST_FILE)).expect("read"))
                .expect("json");
        manifest["modelSha256"] = json!(sha256_hex(&bytes));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("json");
        fs::write(dir.join(MANIFEST_FILE), &manifest_bytes).expect("write");
        let mut marker: Value =
            serde_json::from_slice(&fs::read(dir.join(MARKER_FILE)).expect("read")).expect("json");
        marker["modelSha256"] = json!(sha256_hex(&bytes));
        marker["manifestSha256"] = json!(sha256_hex(&manifest_bytes));
        fs::write(
            dir.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker).expect("json"),
        )
        .expect("write");
        assert!(
            inspect_package(&dir).is_err(),
            "an unknown version must be refused"
        );

        // Extra file.
        let dir = copied_package("extra");
        fs::write(dir.join("extra.json"), b"{}").expect("extra");
        assert!(inspect_package(&dir).is_err());
    }

    #[test]
    fn a_refused_package_leaves_no_state_for_a_later_good_load() {
        let bad = copied_package("bad-then-good");
        fs::remove_file(bad.join(MARKER_FILE)).expect("remove marker");
        assert!(inspect_package(&bad).is_err());
        let good = load_facts("bad-then-good-reload");
        assert_eq!(good.entity_count, 4);
        assert_eq!(good.access_count, 18);
        assert!(good.schema_drift.is_some());
    }

    #[test]
    fn inspection_never_writes_into_the_package() {
        let dir = copied_package("read-only");
        let before: Vec<(String, String)> = REQUIRED_FILES
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    sha256_hex(&fs::read(dir.join(name)).expect("read")),
                )
            })
            .collect();
        let facts = inspect_package(&dir).expect("loads");
        let page = entry_page(&facts);
        assert!(page.contains("scene-0"));
        let after: Vec<(String, String)> = REQUIRED_FILES
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    sha256_hex(&fs::read(dir.join(name)).expect("read")),
                )
            })
            .collect();
        assert_eq!(
            before, after,
            "inspection and rendering must not modify the package"
        );
        assert_eq!(
            fs::read_dir(&dir).expect("dir").count(),
            REQUIRED_FILES.len()
        );
    }

    #[test]
    fn handoff_requires_an_explicit_package_path() {
        let error = super::run_handoff(&[]).expect_err("a missing --preview must be refused");
        assert!(error.to_string().contains("explicit --preview"));
        assert!(super::run_handoff(&["--scan".to_string()]).is_err());
    }

    #[test]
    fn handoff_rejects_a_directory_that_is_not_a_package() {
        let dir = temp_dir("handoff-not-a-package");
        let args: Vec<String> = vec!["--preview".into(), dir.display().to_string()];
        assert!(super::run_handoff(&args).is_err());
    }

    #[test]
    fn handoff_refuses_an_in_package_report_path_before_serving() {
        let dir = copied_package("handoff-report");
        let target = dir.join("handoff.json");
        let args: Vec<String> = vec![
            "--preview".into(),
            dir.display().to_string(),
            "--report".into(),
            target.display().to_string(),
            "--port".into(),
            "0".into(),
        ];
        assert!(super::run_handoff(&args).is_err());
        assert!(
            !target.exists(),
            "no report may be written into the package"
        );
    }

    #[test]
    fn port_verdict_distinguishes_free_same_and_other() {
        let sha = "a".repeat(64);
        assert_eq!(port_verdict(None, &sha), PortVerdict::Free);
        assert_eq!(
            port_verdict(Some(sha.clone()), &sha),
            PortVerdict::SamePackage
        );
        match port_verdict(Some("b".repeat(64)), &sha) {
            PortVerdict::OtherPackage(other) => assert_eq!(other, "b".repeat(64)),
            other => panic!("unexpected verdict {other:?}"),
        }
    }

    #[test]
    fn explicit_flag_contract_is_enforced() {
        // No --preview: the entry must refuse rather than scan for a package.
        let error = super::run(&[]).expect_err("a missing --preview must be refused");
        assert!(error.to_string().contains("explicit --preview"));
        // An unrecognised argument is refused too.
        assert!(super::run(&["--scan".to_string()]).is_err());
    }
}
