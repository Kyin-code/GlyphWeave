//! Independent parity audit between the authoritative `SceneIndex` and the
//! derived render projection produced by `preview_stream::write`.
//!
//! This is deliberately **not** the producer: it never calls
//! `preview_stream::chunk_range`. Chunk membership is recomputed here with an
//! independent half-open rectangle test, and the authoritative scene is parsed
//! with `serde_json::from_reader` over a hashing reader (never `read_to_string`).
//!
//! Output contract: `docs/collaboration/a-d2-parity-contract.zh-CN.md`.

use crate::{
    height_payload::{HeightEncoding, HeightPayload},
    settlement::landcover::Material,
    worldgen::{ChunkDescriptor, EntityInstance, SceneIndex, WorldIndex},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

/// Same typed round-trip the producer uses, so f32 fields keep their shortest
/// decimal form instead of being widened to f64 by `to_value`.
fn exact_json<T: serde::Serialize>(value: &T) -> Result<Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(value)?)?)
}

pub const PARITY_FORMAT: &str = "glyphweave.render-parity";
pub const PARITY_VERSION: u32 = 1;

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Hashes every byte actually consumed by the parser, so the recorded source
/// hash always corresponds to the bytes this audit read (never a second read).
struct HashingReader<R: Read> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}
impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}

fn read_hashed_scene(path: &Path) -> Result<(SceneIndex, String, u64)> {
    let mut reader = HashingReader {
        inner: File::open(path)?,
        hasher: Sha256::new(),
        bytes: 0,
    };
    let scene: SceneIndex = {
        let mut buffered = BufReader::with_capacity(1 << 20, &mut reader);
        let scene = serde_json::from_reader(&mut buffered)?;
        // Drain any trailing bytes so the hash covers the whole file.
        std::io::copy(&mut buffered, &mut std::io::sink())?;
        scene
    };
    let sha = reader
        .hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((scene, sha, reader.bytes))
}

/// Contract digest: sorted UTF-8 IDs, compact JSON array, SHA256. Duplicate IDs
/// are rejected rather than silently de-duplicated.
fn ids_digest(ids: &[String]) -> Result<String> {
    let mut set = BTreeSet::new();
    for id in ids {
        if id.is_empty() {
            return Err("empty render ID cannot be hashed".into());
        }
        if !set.insert(id.clone()) {
            return Err(format!("duplicate render ID: {id}").into());
        }
    }
    let sorted: Vec<&String> = set.iter().collect();
    Ok(sha256_hex(serde_json::to_vec(&sorted)?.as_slice()))
}

fn aabb(vertices: &[[f64; 3]]) -> Result<[f64; 4]> {
    if vertices.is_empty() {
        return Err("empty geometry".into());
    }
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for v in vertices {
        if !v.iter().all(|c| c.is_finite()) {
            return Err("non-finite geometry".into());
        }
        b[0] = b[0].min(v[0]);
        b[1] = b[1].min(v[2]);
        b[2] = b[2].max(v[0]);
        b[3] = b[3].max(v[2]);
    }
    Ok(b)
}

/// Independent half-open rectangle test. A patch belongs to chunk (cx,cz) iff
/// its AABB intersects `[chunkMin, chunkMax)` in both axes. This reproduces the
/// producer's closed-bounds/seam-duplication semantics without calling its
/// `chunk_range` helper.
fn rect_in_chunk(b: [f64; 4], cx: u32, cz: u32, scene: &SceneIndex) -> Result<bool> {
    let size = f64::from(scene.chunk_size_m);
    let ox = f64::from(scene.origin_x);
    let oz = f64::from(scene.origin_z);
    if b[2] <= b[0] || b[3] <= b[1] {
        return Err("degenerate render bounds".into());
    }
    if b[0] < ox
        || b[1] < oz
        || b[2] > ox + f64::from(scene.width_m)
        || b[3] > oz + f64::from(scene.depth_m)
    {
        return Err("render geometry outside scene domain".into());
    }
    let cmin_x = ox + f64::from(cx) * size;
    let cmin_z = oz + f64::from(cz) * size;
    let cmax_x = cmin_x + size;
    let cmax_z = cmin_z + size;
    Ok(b[2] >= cmin_x && b[0] < cmax_x && b[3] >= cmin_z && b[1] < cmax_z)
}

fn chunk_of_plant(
    plant: &crate::settlement::ecology::EcologyPlant,
    scene: &SceneIndex,
) -> Result<(u32, u32)> {
    let size = f64::from(scene.chunk_size_m);
    let x = ((plant.world_x - f64::from(scene.origin_x)) / size).floor();
    let z = ((plant.world_z - f64::from(scene.origin_z)) / size).floor();
    if !x.is_finite()
        || !z.is_finite()
        || x < 0.
        || z < 0.
        || x >= f64::from(scene.chunk_count_x)
        || z >= f64::from(scene.chunk_count_z)
    {
        return Err("render plant outside scene domain".into());
    }
    Ok((x as u32, z as u32))
}

fn is_non_ecology_entity(entity: &EntityInstance) -> bool {
    !matches!(
        entity.kind.as_str(),
        "tree" | "bush" | "rock" | "grass_clump" | "reed" | "fallen_log"
    )
}

fn is_natural_contract_kind(kind: &str) -> bool {
    matches!(kind, "tree" | "rock")
}

fn entity_in_chunk(entity: &EntityInstance, chunk: &ChunkDescriptor) -> bool {
    f64::from(entity.world_x) >= f64::from(chunk.world_x)
        && f64::from(entity.world_x) < f64::from(chunk.world_x) + f64::from(chunk.valid_width_m)
        && f64::from(entity.world_z) >= f64::from(chunk.world_z)
        && f64::from(entity.world_z) < f64::from(chunk.world_z) + f64::from(chunk.valid_depth_m)
}

fn contained_chunk_file(scene_dir: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("{label} path escapes scene directory: {relative}").into());
    }
    let joined = scene_dir.join(path);
    if !joined.starts_with(scene_dir) {
        return Err(format!("{label} path escapes scene directory: {relative}").into());
    }
    Ok(joined)
}

fn height_encoding_name(encoding: HeightEncoding) -> &'static str {
    match encoding {
        HeightEncoding::QuarterMetre => "quarter-metre",
        HeightEncoding::Millimetre => "millimetre",
    }
}

fn read_height_file(
    scene_dir: &Path,
    relative: &str,
    width: u32,
    depth: u32,
    label: &str,
) -> Result<(Vec<u8>, &'static str)> {
    let path = contained_chunk_file(scene_dir, relative, label)?;
    let bytes = fs::read(&path).map_err(|e| format!("missing {label} {relative}: {e}"))?;
    let payload = HeightPayload::parse(&bytes, width, depth)?;
    let encoding = height_encoding_name(payload.encoding);
    Ok((bytes, encoding))
}

fn expected_landcover_summary(scene: &SceneIndex) -> Result<Value> {
    let Some(surfaces) = &scene.landcover_surfaces else {
        return Ok(Value::Null);
    };
    let preview_focus_heights: BTreeMap<&str, f64> = surfaces
        .patches
        .iter()
        .filter(|patch| patch.material == Material::CultivatedSoil)
        .filter_map(|patch| {
            patch
                .vertices
                .first()
                .map(|vertex| (patch.parcel_id.as_str(), vertex[1]))
        })
        .collect();
    Ok(json!({
        "version": surfaces.version,
        "sceneId": surfaces.scene_id,
        "finalHeightfieldHash": surfaces.final_heightfield_hash,
        "patches": [],
        "rejections": [],
        "agriculture": {
            "model": surfaces.agriculture.model,
            "fields": to_value(&surfaces.agriculture.fields)?,
            "rejectedFields": [],
            "previewFocusHeights": preview_focus_heights,
        },
        "ecology": {
            "model": surfaces.ecology.model,
            "classes": to_value(&surfaces.ecology.classes)?,
            "totalAreaM2": surfaces.ecology.total_area_m2,
            "status": surfaces.ecology.status,
            "plants": [],
            "communities": [],
            "rejections": [],
        }
    }))
}

fn expected_road_network_header(scene: &SceneIndex) -> Result<Value> {
    match &scene.road_network_plan {
        Some(plan) => Ok(json!({
            "roadsideSites": exact_json(&plan.roadside_sites)?,
            "jointLayout": exact_json(&plan.joint_layout)?,
        })),
        None => Ok(Value::Null),
    }
}

fn reject_unknown_keys(
    value: &Value,
    allowed: &[&str],
    label: &str,
    scene_id: &str,
    failures: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        failures.push(format!("{scene_id}: {label} is not an object"));
        return;
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            failures.push(format!("{scene_id}: {label} has unknown field {key}"));
        }
    }
}

fn access_surface_id(scene_id: &str, entity_id: &str, anchor_id: &str) -> Result<String> {
    Ok(format!(
        "access:{}",
        serde_json::to_string(&vec![scene_id, entity_id, anchor_id])?
    ))
}

/// Reject `out_dir` that already exists or lives inside the world tree.
fn prepare_out(world_root: &Path, out_dir: &Path) -> Result<PathBuf> {
    let world = fs::canonicalize(world_root)?;
    let out = if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(out_dir)
    };
    if out.exists() {
        return Err(format!("output directory already exists: {}", out.display()).into());
    }
    // Validate the parent so the (not yet existing) leaf cannot be resolved
    // into the world tree via a symlinked parent.
    let parent = out.parent().ok_or("output directory has no parent")?;
    let parent = fs::canonicalize(parent).map_err(|e| format!("output parent must exist: {e}"))?;
    if parent.starts_with(&world) {
        return Err("output directory must be outside the world tree".into());
    }
    let leaf = out.file_name().ok_or("output directory has no name")?;
    Ok(parent.join(leaf))
}

/// Build the parity document for a whole world. Returns the JSON value that was
/// written to `OUT_DIR/parity.json` and per-scene compact audit sources.
pub fn audit_world(world_root: &Path, out_dir: &Path) -> Result<Value> {
    let out = prepare_out(world_root, out_dir)?;
    let world_bytes = fs::read(world_root.join("world.json"))?;
    let world: WorldIndex = serde_json::from_slice(&world_bytes)?;
    let world_sha = sha256_hex(&world_bytes);
    let world_canon = fs::canonicalize(world_root)?;

    fs::create_dir_all(&out)?;

    let mut failures: Vec<String> = Vec::new();
    let mut scenes_out: Vec<Value> = Vec::new();

    for scene_rel in &world.scenes {
        let scene_path = world_canon.join(scene_rel);
        let scene_canon = fs::canonicalize(&scene_path)
            .map_err(|e| format!("scene {scene_rel} not found: {e}"))?;
        if !scene_canon.starts_with(&world_canon) {
            return Err(format!("scene escapes world tree: {scene_rel}").into());
        }
        let (scene, source_sha, source_bytes) = read_hashed_scene(&scene_canon)?;
        let scene_dir = scene_canon.parent().ok_or("scene has no directory")?;
        let header_path = scene_dir.join("preview-scene.json");
        let header_bytes = fs::read(&header_path)
            .map_err(|e| format!("missing preview-scene.json for {}: {e}", scene.scene_id))?;
        let header: Value = serde_json::from_slice(&header_bytes)?;
        let header_sha = sha256_hex(&header_bytes);

        let mut scene_failures: Vec<String> = Vec::new();
        macro_rules! expect {
            ($cond:expr, $($msg:tt)*) => {
                if !$cond { scene_failures.push(format!($($msg)*)); }
            };
        }
        expect!(
            header.get("sceneId").and_then(Value::as_str) == Some(scene.scene_id.as_str()),
            "{}: header sceneId mismatch",
            scene.scene_id
        );
        expect!(
            header.get("widthM").and_then(Value::as_u64) == Some(u64::from(scene.width_m))
                && header.get("depthM").and_then(Value::as_u64) == Some(u64::from(scene.depth_m))
                && header.get("originX").and_then(Value::as_i64) == Some(i64::from(scene.origin_x))
                && header.get("originZ").and_then(Value::as_i64) == Some(i64::from(scene.origin_z))
                && header.get("chunkSizeM").and_then(Value::as_u64)
                    == Some(u64::from(scene.chunk_size_m)),
            "{}: header scene domain mismatch",
            scene.scene_id
        );
        expect!(
            header.get("chunkCountX").and_then(Value::as_u64)
                == Some(u64::from(scene.chunk_count_x))
                && header.get("chunkCountZ").and_then(Value::as_u64)
                    == Some(u64::from(scene.chunk_count_z)),
            "{}: header chunk counts differ from source",
            scene.scene_id
        );
        reject_unknown_keys(
            &header,
            &[
                "sceneId",
                "widthM",
                "depthM",
                "originX",
                "originZ",
                "chunkSizeM",
                "chunkCountX",
                "chunkCountZ",
                "chunks",
                "landmarks",
                "entities",
                "accessSurfaces",
                "developmentPlan",
                "landcoverPlan",
                "landUseZones",
                "landUseBudget",
                "roadNetworkPlan",
                "landcoverSurfaces",
                "renderStream",
            ],
            "header",
            &scene.scene_id,
            &mut scene_failures,
        );
        for (field, expected) in [
            ("chunks", exact_json(&scene.chunks)?),
            ("landmarks", exact_json(&scene.landmarks)?),
            ("entities", exact_json(&scene.entities)?),
            ("accessSurfaces", exact_json(&scene.access_surfaces)?),
            ("developmentPlan", exact_json(&scene.development_plan)?),
            ("landcoverPlan", exact_json(&scene.landcover_plan)?),
            ("landUseZones", exact_json(&scene.land_use_zones)?),
            ("landUseBudget", exact_json(&scene.land_use_budget)?),
            ("roadNetworkPlan", expected_road_network_header(&scene)?),
            ("landcoverSurfaces", expected_landcover_summary(&scene)?),
        ] {
            if header.get(field) != Some(&expected) {
                scene_failures.push(format!(
                    "{}: header {field} differs from authoritative source",
                    scene.scene_id
                ));
            }
        }
        let render_stream = header.get("renderStream").cloned().unwrap_or(Value::Null);
        reject_unknown_keys(
            &render_stream,
            &[
                "format",
                "version",
                "worldRevision",
                "chunks",
                "totalPatches",
                "totalPlants",
                "totalEntities",
            ],
            "renderStream",
            &scene.scene_id,
            &mut scene_failures,
        );
        expect!(
            render_stream.get("format").and_then(Value::as_str)
                == Some("glyphweave.render-directory")
                && render_stream.get("version").and_then(Value::as_u64) == Some(1)
                && render_stream.get("worldRevision").and_then(Value::as_str)
                    == Some(world.revision.as_str()),
            "{}: render stream header identity/revision mismatch",
            scene.scene_id
        );
        expect!(
            render_stream.get("totalPatches").and_then(Value::as_u64)
                == Some(
                    scene
                        .landcover_surfaces
                        .as_ref()
                        .map_or(0, |s| s.patches.len()) as u64
                )
                && render_stream.get("totalPlants").and_then(Value::as_u64)
                    == Some(
                        scene
                            .landcover_surfaces
                            .as_ref()
                            .map_or(0, |s| s.ecology.plants.len()) as u64,
                    ),
            "{}: render stream totals differ from source",
            scene.scene_id
        );
        if let Some(total_entities) = render_stream.get("totalEntities") {
            expect!(
                total_entities.as_u64() == Some(scene.entities.len() as u64),
                "{}: render stream totalEntities differs from source",
                scene.scene_id
            );
        }

        // Descriptor table from the projection header.
        let mut descriptors: BTreeMap<(u32, u32), (String, String, u64, u64, u64)> =
            BTreeMap::new();
        if let Some(chunks) = render_stream.get("chunks").and_then(Value::as_array) {
            for c in chunks {
                let key = (
                    c.get("chunkX").and_then(Value::as_u64).unwrap_or(u64::MAX) as u32,
                    c.get("chunkZ").and_then(Value::as_u64).unwrap_or(u64::MAX) as u32,
                );
                let entry = (
                    c.get("file")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    c.get("sha256")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    c.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                    c.get("patchCount").and_then(Value::as_u64).unwrap_or(0),
                    c.get("plantCount").and_then(Value::as_u64).unwrap_or(0),
                );
                if descriptors.insert(key, entry).is_some() {
                    scene_failures.push(format!(
                        "{}: duplicate descriptor {:?}",
                        scene.scene_id, key
                    ));
                }
            }
        }

        // Independently recompute expected per-chunk membership from the source.
        let mut source_chunks: BTreeMap<(u32, u32), &ChunkDescriptor> = BTreeMap::new();
        for chunk in &scene.chunks {
            if source_chunks
                .insert((chunk.chunk_x, chunk.chunk_z), chunk)
                .is_some()
            {
                scene_failures.push(format!("{}: duplicate source chunk", scene.scene_id));
            }
        }
        let mut expected_patches: BTreeMap<
            (u32, u32),
            Vec<&crate::settlement::landcover::SurfacePatch>,
        > = BTreeMap::new();
        let mut expected_non_mesh: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        let mut expected_plants: BTreeMap<
            (u32, u32),
            Vec<&crate::settlement::ecology::EcologyPlant>,
        > = BTreeMap::new();
        let mut expected_access: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        let mut expected_entities: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        let mut expected_natural_entities: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        let mut patch_ids: BTreeSet<String> = BTreeSet::new();
        let mut plant_ids: BTreeSet<String> = BTreeSet::new();
        let mut all_non_ecology_entity_ids: BTreeSet<String> = BTreeSet::new();
        let mut all_natural_entity_ids: BTreeSet<String> = BTreeSet::new();
        let mut all_entity_ids: BTreeSet<String> = BTreeSet::new();

        for entity in &scene.entities {
            if !all_entity_ids.insert(entity.entity_id.clone()) {
                return Err(format!("duplicate source entity id: {}", entity.entity_id).into());
            }
            if !is_non_ecology_entity(entity) {
                if is_natural_contract_kind(&entity.kind) {
                    all_natural_entity_ids.insert(entity.entity_id.clone());
                    for ((cx, cz), chunk) in &source_chunks {
                        if entity_in_chunk(entity, chunk) {
                            let entry = expected_natural_entities.entry((*cx, *cz)).or_default();
                            if !entry.contains(&entity.entity_id) {
                                entry.push(entity.entity_id.clone());
                            }
                        }
                    }
                }
                continue;
            }
            all_non_ecology_entity_ids.insert(entity.entity_id.clone());
            for ((cx, cz), chunk) in &source_chunks {
                if entity_in_chunk(entity, chunk) {
                    let entry = expected_entities.entry((*cx, *cz)).or_default();
                    if !entry.contains(&entity.entity_id) {
                        entry.push(entity.entity_id.clone());
                    }
                }
            }
        }
        for landmark in &scene.landmarks {
            if landmark.entity_id.is_empty() {
                continue;
            }
            all_non_ecology_entity_ids.insert(landmark.entity_id.clone());
            for ((cx, cz), chunk) in &source_chunks {
                if f64::from(landmark.world_x) >= f64::from(chunk.world_x)
                    && f64::from(landmark.world_x)
                        < f64::from(chunk.world_x) + f64::from(chunk.valid_width_m)
                    && f64::from(landmark.world_z) >= f64::from(chunk.world_z)
                    && f64::from(landmark.world_z)
                        < f64::from(chunk.world_z) + f64::from(chunk.valid_depth_m)
                {
                    let entry = expected_entities.entry((*cx, *cz)).or_default();
                    if !entry.contains(&landmark.entity_id) {
                        entry.push(landmark.entity_id.clone());
                    }
                }
            }
        }
        let all_non_ecology_entity_ids: Vec<String> =
            all_non_ecology_entity_ids.into_iter().collect();
        let all_natural_entity_ids: Vec<String> = all_natural_entity_ids.into_iter().collect();

        if let Some(surfaces) = &scene.landcover_surfaces {
            for patch in &surfaces.patches {
                if !patch_ids.insert(patch.id.clone()) {
                    return Err(format!("duplicate source patch id: {}", patch.id).into());
                }
                let b = aabb(&patch.vertices)?;
                for cz in 0..scene.chunk_count_z {
                    for cx in 0..scene.chunk_count_x {
                        if rect_in_chunk(b, cx, cz, &scene)? {
                            expected_patches.entry((cx, cz)).or_default().push(patch);
                            if patch.material == Material::AccessPavement {
                                expected_non_mesh
                                    .entry((cx, cz))
                                    .or_default()
                                    .push(patch.id.clone());
                            }
                        }
                    }
                }
            }
            for plant in &surfaces.ecology.plants {
                if !plant_ids.insert(plant.id.clone()) {
                    return Err(format!("duplicate source plant id: {}", plant.id).into());
                }
                if !patch_ids.contains(&plant.source_patch_id) {
                    return Err(format!("plant {} has unknown source patch", plant.id).into());
                }
                let key = chunk_of_plant(plant, &scene)?;
                expected_plants.entry(key).or_default().push(plant);
            }
        }
        for surface in &scene.access_surfaces {
            let id = access_surface_id(&scene.scene_id, &surface.entity_id, &surface.anchor_id)?;
            let b = aabb(&surface.vertices)?;
            for cz in 0..scene.chunk_count_z {
                for cx in 0..scene.chunk_count_x {
                    if rect_in_chunk(b, cx, cz, &scene)? {
                        expected_access
                            .entry((cx, cz))
                            .or_default()
                            .push(id.clone());
                    }
                }
            }
        }

        // Compare every projected chunk against the independent expectation.
        let mut chunks_out: Vec<Value> = Vec::new();
        let mut expected_descriptors: Vec<Value> = Vec::new();
        for cx in 0..scene.chunk_count_x {
            for cz in 0..scene.chunk_count_z {
                let key = (cx, cz);
                let (file, want_sha, want_bytes, want_patch, want_plant) =
                    match descriptors.get(&key) {
                        Some(d) => d.clone(),
                        None => {
                            scene_failures.push(format!(
                                "{}: missing descriptor for {cx}:{cz}",
                                scene.scene_id
                            ));
                            continue;
                        }
                    };
                let chunk_path = scene_dir.join(&file);
                if !chunk_path.starts_with(scene_dir) || file.contains("..") {
                    scene_failures.push(format!("{}: unsafe chunk path {file}", scene.scene_id));
                    continue;
                }
                let chunk_bytes = match fs::read(&chunk_path) {
                    Ok(b) => b,
                    Err(e) => {
                        scene_failures
                            .push(format!("{}: missing chunk {file}: {e}", scene.scene_id));
                        continue;
                    }
                };
                let got_sha = sha256_hex(&chunk_bytes);
                if got_sha != want_sha || chunk_bytes.len() as u64 != want_bytes {
                    scene_failures.push(format!(
                        "{}: chunk {file} hash/bytes mismatch",
                        scene.scene_id
                    ));
                }
                let payload: Value = serde_json::from_slice(&chunk_bytes)
                    .map_err(|e| format!("{}: chunk {file} invalid JSON: {e}", scene.scene_id))?;
                let exp_patches: Vec<Value> = expected_patches
                    .get(&key)
                    .map(|v| v.iter().map(|p| to_value(p)).collect::<Result<Vec<_>>>())
                    .transpose()?
                    .unwrap_or_default();
                let exp_plants: Vec<Value> = expected_plants
                    .get(&key)
                    .map(|v| v.iter().map(|p| to_value(p)).collect::<Result<Vec<_>>>())
                    .transpose()?
                    .unwrap_or_default();
                if payload.get("patches") != Some(&Value::Array(exp_patches)) {
                    scene_failures.push(format!(
                        "{}: chunk {cx}:{cz} patches differ from source",
                        scene.scene_id
                    ));
                }
                if payload.get("plants") != Some(&Value::Array(exp_plants)) {
                    scene_failures.push(format!(
                        "{}: chunk {cx}:{cz} plants differ from source",
                        scene.scene_id
                    ));
                }
                if payload.get("chunkX").and_then(Value::as_u64) != Some(u64::from(cx))
                    || payload.get("chunkZ").and_then(Value::as_u64) != Some(u64::from(cz))
                    || payload.get("worldRevision").and_then(Value::as_str)
                        != Some(world.revision.as_str())
                {
                    scene_failures.push(format!(
                        "{}: chunk {cx}:{cz} payload identity mismatch",
                        scene.scene_id
                    ));
                }

                let patch_id_list: Vec<String> = expected_patches
                    .get(&key)
                    .map(|v| v.iter().map(|p| p.id.clone()).collect())
                    .unwrap_or_default();
                let plant_id_list: Vec<String> = expected_plants
                    .get(&key)
                    .map(|v| v.iter().map(|p| p.id.clone()).collect())
                    .unwrap_or_default();
                let non_mesh = expected_non_mesh.get(&key).cloned().unwrap_or_default();
                let access = expected_access.get(&key).cloned().unwrap_or_default();
                let entity_id_list = expected_entities.get(&key).cloned().unwrap_or_default();
                let natural_entity_id_list = expected_natural_entities
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();

                if patch_id_list.len() as u64 != want_patch
                    || plant_id_list.len() as u64 != want_plant
                {
                    scene_failures.push(format!(
                        "{}: chunk {cx}:{cz} descriptor counts mismatch",
                        scene.scene_id
                    ));
                }

                let source_chunk = source_chunks.get(&key).ok_or_else(|| {
                    format!("{}: source chunk {cx}:{cz} is missing", scene.scene_id)
                })?;
                let (height_bytes, height_encoding) = read_height_file(
                    scene_dir,
                    &source_chunk.height_file,
                    source_chunk.valid_width_m,
                    source_chunk.valid_depth_m,
                    "height file",
                )?;
                let natural = match &source_chunk.natural_height_file {
                    Some(relative) => {
                        let (bytes, encoding) = read_height_file(
                            scene_dir,
                            relative,
                            source_chunk.valid_width_m,
                            source_chunk.valid_depth_m,
                            "natural height file",
                        )?;
                        Some((relative.clone(), bytes, encoding))
                    }
                    None => None,
                };
                let entity_ids_sha = ids_digest(&entity_id_list)?;
                let natural_entity_ids_sha = ids_digest(&natural_entity_id_list)?;

                chunks_out.push(json!({
                    "key": format!("{cx}:{cz}"),
                    "chunkX": cx, "chunkZ": cz,
                    "worldX": scene.origin_x as i64 + i64::from(cx) * i64::from(scene.chunk_size_m),
                    "worldZ": scene.origin_z as i64 + i64::from(cz) * i64::from(scene.chunk_size_m),
                    "validWidthM": (scene.width_m - cx * scene.chunk_size_m).min(scene.chunk_size_m),
                    "validDepthM": (scene.depth_m - cz * scene.chunk_size_m).min(scene.chunk_size_m),
                    "file": file,
                    "sha256": got_sha,
                    "bytes": chunk_bytes.len(),
                    "patches": patch_id_list.len(),
                    "plants": plant_id_list.len(),
                    "plantIdsSha256": ids_digest(&plant_id_list)?,
                    "patchIdsSha256": ids_digest(&patch_id_list)?,
                    "nonMeshPatches": non_mesh.len(),
                    "nonMeshPatchIdsSha256": ids_digest(&non_mesh)?,
                    "accessSurfaces": access.len(),
                    "accessSurfaceIdsSha256": ids_digest(&access)?,
                    "entities": entity_id_list.len(),
                    "entityIds": entity_id_list,
                    "entityIdsSha256": entity_ids_sha,
                    "naturalEntities": natural_entity_id_list.len(),
                    "naturalEntityIds": natural_entity_id_list,
                    "naturalEntityIdsSha256": natural_entity_ids_sha,
                    "heightFile": source_chunk.height_file,
                    "heightBytes": height_bytes.len(),
                    "heightSha256": sha256_hex(&height_bytes),
                    "heightEncoding": height_encoding,
                    "naturalHeightFile": natural.as_ref().map(|(relative, _, _)| relative.clone()),
                    "naturalHeightBytes": natural.as_ref().map(|(_, bytes, _)| bytes.len()),
                    "naturalHeightSha256": natural.as_ref().map(|(_, bytes, _)| sha256_hex(bytes)),
                    "naturalHeightEncoding": natural.as_ref().map(|(_, _, encoding)| *encoding),
                }));
                expected_descriptors.push(json!({
                    "chunkX": cx,
                    "chunkZ": cz,
                    "file": file,
                    "sha256": got_sha,
                    "bytes": chunk_bytes.len(),
                    "patchCount": patch_id_list.len(),
                    "plantCount": plant_id_list.len(),
                }));
            }
        }
        if render_stream.get("chunks") != Some(&Value::Array(expected_descriptors)) {
            scene_failures.push(format!(
                "{}: renderStream chunks differ from authoritative source/chunk payloads",
                scene.scene_id
            ));
        }

        if !scene_failures.is_empty() {
            failures.extend(scene_failures.iter().cloned());
        }
        let unique_patches: Vec<String> = patch_ids.iter().cloned().collect();
        let unique_plants: Vec<String> = plant_ids.iter().cloned().collect();
        let mut entity_kinds: BTreeMap<&str, u64> = BTreeMap::new();
        for entity in &scene.entities {
            *entity_kinds.entry(entity.kind.as_str()).or_insert(0) += 1;
        }
        let sample_count: u64 = scene
            .chunks
            .iter()
            .map(|c| u64::from(c.valid_width_m) * u64::from(c.valid_depth_m))
            .sum();
        let (
            surface_patches,
            ecology_plants,
            ecology_communities,
            ecology_rejections,
            agriculture_fields,
        ) = match &scene.landcover_surfaces {
            Some(s) => (
                s.patches.len(),
                s.ecology.plants.len(),
                s.ecology.communities.len(),
                s.ecology.rejections.len(),
                s.agriculture.fields.len(),
            ),
            None => (0, 0, 0, 0, 0),
        };
        let audit_source = json!({
            "format": "glyphweave.render-audit-source",
            "version": 1,
            "sceneId": scene.scene_id,
            "widthM": scene.width_m, "depthM": scene.depth_m,
            "originX": scene.origin_x, "originZ": scene.origin_z,
            "chunkSizeM": scene.chunk_size_m,
            "chunks": scene.chunks,
            "entities": scene.entities,
            "accessSurfaces": scene.access_surfaces,
            "roadNetworkPlan": exact_json(&scene.road_network_plan)?,
            "accessGraph": exact_json(&scene.access_graph)?,
            "landUseZones": scene.land_use_zones,
            "patchCount": patch_ids.len(),
            "plantCount": plant_ids.len(),
            "surfacePatchCount": surface_patches,
            "ecologyPlantCount": ecology_plants,
            "ecologyCommunityCount": ecology_communities,
            "ecologyRejectionCount": ecology_rejections,
            "agriculturalFieldCount": agriculture_fields,
            "sampleCount": sample_count,
            "entityKinds": entity_kinds,
            "source": {"path": scene_canon.display().to_string(), "sha256": source_sha, "bytes": source_bytes},
            "header": {"path": fs::canonicalize(&header_path)?.display().to_string(), "sha256": header_sha, "bytes": header_bytes.len()},
            "worldRevision": world.revision,
        });
        let audit_name = format!("audit-source-{}.json", sanitize(&scene.scene_id));
        let audit_bytes = serde_json::to_vec(&audit_source)?;
        let audit_sha = sha256_hex(&audit_bytes);
        write_new(&out.join(&audit_name), &audit_bytes)?;
        scenes_out.push(json!({
            "sceneId": scene.scene_id,
            "widthM": scene.width_m, "depthM": scene.depth_m,
            "originX": scene.origin_x, "originZ": scene.origin_z,
            "chunkSizeM": scene.chunk_size_m,
            "source": {"path": scene_canon.display().to_string(), "sha256": source_sha, "bytes": source_bytes},
            "header": {"path": fs::canonicalize(&header_path)?.display().to_string(), "sha256": header_sha, "bytes": header_bytes.len()},
            "auditInput": {"path": audit_name, "bytes": audit_bytes.len(), "sha256": audit_sha},
            "uniquePlants": unique_plants.len(),
            "uniquePatches": unique_patches.len(),
            "nonEcologyEntityIds": all_non_ecology_entity_ids,
            "naturalEntityIds": all_natural_entity_ids,
            "chunks": chunks_out,
        }));
    }

    let status = if failures.is_empty() { "pass" } else { "fail" };
    let parity = json!({
        "format": PARITY_FORMAT,
        "version": PARITY_VERSION,
        "status": status,
        "worldRevision": world.revision,
        "worldFile": {"path": fs::canonicalize(world_root.join("world.json"))?.display().to_string(), "sha256": world_sha},
        "scenes": scenes_out,
        "failures": failures,
    });
    write_new(
        &out.join("parity.json"),
        &serde_json::to_vec_pretty(&parity)?,
    )?;
    Ok(parity)
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Exclusive create: refuse to overwrite an existing file.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("refusing to overwrite {}: {e}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_digest_is_order_independent_and_rejects_duplicates() {
        let a = ids_digest(&["b".into(), "a".into()]).unwrap();
        let b = ids_digest(&["a".into(), "b".into()]).unwrap();
        assert_eq!(a, b);
        assert!(ids_digest(&["a".into(), "a".into()]).is_err());
        assert!(ids_digest(&["".into()]).is_err());
    }

    #[test]
    fn access_surface_id_matches_contract_shape() {
        assert_eq!(
            access_surface_id("scene-0", "ent", "anchor").unwrap(),
            "access:[\"scene-0\",\"ent\",\"anchor\"]"
        );
    }

    #[test]
    fn hashing_reader_counts_and_hashes_exact_bytes() {
        let data = b"glyphweave".to_vec();
        let mut reader = HashingReader {
            inner: std::io::Cursor::new(data.clone()),
            hasher: Sha256::new(),
            bytes: 0,
        };
        let mut out = Vec::new();
        std::io::copy(&mut reader, &mut out).unwrap();
        assert_eq!(out, data);
        assert_eq!(reader.bytes, data.len() as u64);
        let sha = reader
            .hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(sha, sha256_hex(&data));
    }
}
