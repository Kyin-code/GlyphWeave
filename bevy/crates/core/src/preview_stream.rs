//! Derived, hash-bound render projection; full scene remains audit authority.
use crate::{
    settlement::{ecology::EcologyPlant, landcover::SurfacePatch},
    worldgen::SceneIndex,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload<'a> {
    format: &'static str,
    version: u32,
    scene_id: &'a str,
    world_revision: &'a str,
    chunk_x: u32,
    chunk_z: u32,
    patches: Vec<&'a SurfacePatch>,
    plants: Vec<&'a EcologyPlant>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    chunk_x: u32,
    chunk_z: u32,
    file: String,
    sha256: String,
    bytes: usize,
    patch_count: usize,
    plant_count: usize,
}
fn chunk_range(scene: &SceneIndex, rect: [f64; 4]) -> Result<(u32, u32, u32, u32)> {
    let [x, z, r, b] = rect;
    if !rect.iter().all(|v| v.is_finite())
        || r <= x
        || b <= z
        || x < f64::from(scene.origin_x)
        || z < f64::from(scene.origin_z)
        || r > f64::from(scene.origin_x) + f64::from(scene.width_m)
        || b > f64::from(scene.origin_z) + f64::from(scene.depth_m)
    {
        return Err("invalid render patch bounds".into());
    }
    let size = f64::from(scene.chunk_size_m);
    let ox = f64::from(scene.origin_x);
    let oz = f64::from(scene.origin_z);
    let lo = |v: f64, max: u32| (v.floor().max(0.) as u32).min(max - 1);
    // Closed mesh bounds are conservatively duplicated at touching seams;
    // merged browser payloads de-duplicate by ID. No triangles are clipped.
    Ok((
        lo((x - ox) / size, scene.chunk_count_x),
        lo((r - ox) / size, scene.chunk_count_x),
        lo((z - oz) / size, scene.chunk_count_z),
        lo((b - oz) / size, scene.chunk_count_z),
    ))
}
pub fn write(scene: &SceneIndex, revision: &str, directory: &Path) -> Result<()> {
    if scene.chunk_size_m == 0 || scene.chunk_count_x == 0 || scene.chunk_count_z == 0 {
        return Err("invalid render scene domain".into());
    }
    if scene.width_m == 0
        || scene.depth_m == 0
        || scene.chunk_count_x != scene.width_m.div_ceil(scene.chunk_size_m)
        || scene.chunk_count_z != scene.depth_m.div_ceil(scene.chunk_size_m)
        || scene.chunks.len() as u64
            != u64::from(scene.chunk_count_x) * u64::from(scene.chunk_count_z)
    {
        return Err("incomplete render chunk coverage".into());
    }
    let mut chunks = BTreeMap::new();
    let mut domains = BTreeMap::new();
    for c in &scene.chunks {
        if c.chunk_x >= scene.chunk_count_x || c.chunk_z >= scene.chunk_count_z {
            return Err("render chunk outside grid".into());
        }
        let dx = c.chunk_x * scene.chunk_size_m;
        let dz = c.chunk_z * scene.chunk_size_m;
        if i64::from(c.world_x) != i64::from(scene.origin_x) + i64::from(dx)
            || i64::from(c.world_z) != i64::from(scene.origin_z) + i64::from(dz)
            || c.valid_width_m != (scene.width_m - dx).min(scene.chunk_size_m)
            || c.valid_depth_m != (scene.depth_m - dz).min(scene.chunk_size_m)
        {
            return Err("invalid render chunk domain".into());
        }
        domains.insert((c.chunk_x, c.chunk_z), c);
        if chunks
            .insert(
                (c.chunk_x, c.chunk_z),
                Payload {
                    format: "glyphweave.render-chunk",
                    version: 1,
                    scene_id: &scene.scene_id,
                    world_revision: revision,
                    chunk_x: c.chunk_x,
                    chunk_z: c.chunk_z,
                    patches: vec![],
                    plants: vec![],
                },
            )
            .is_some()
        {
            return Err("duplicate render chunk".into());
        }
    }
    let mut patch_ids = BTreeSet::new();
    let mut plant_ids = BTreeSet::new();
    if let Some(surfaces) = &scene.landcover_surfaces {
        for patch in &surfaces.patches {
            if !patch_ids.insert(&patch.id) {
                return Err("duplicate render patch id".into());
            }
            if patch.vertices.iter().flatten().any(|v| !v.is_finite())
                || patch.triangles.is_empty()
                || patch
                    .triangles
                    .iter()
                    .flatten()
                    .any(|i| *i as usize >= patch.vertices.len())
            {
                return Err("invalid render patch geometry".into());
            }
            let bounds = patch.vertices.iter().fold(
                [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ],
                |a, p| {
                    [
                        a[0].min(p[0]),
                        a[1].min(p[2]),
                        a[2].max(p[0]),
                        a[3].max(p[2]),
                    ]
                },
            );
            let (x0, x1, z0, z1) = chunk_range(scene, bounds)?;
            for z in z0..=z1 {
                for x in x0..=x1 {
                    chunks
                        .get_mut(&(x, z))
                        .ok_or("render patch missing chunk")?
                        .patches
                        .push(patch);
                }
            }
        }
        for plant in &surfaces.ecology.plants {
            if !plant_ids.insert(&plant.id) || !patch_ids.contains(&plant.source_patch_id) {
                return Err("invalid render plant provenance".into());
            }
            let x = ((plant.world_x - f64::from(scene.origin_x)) / f64::from(scene.chunk_size_m))
                .floor();
            let z = ((plant.world_z - f64::from(scene.origin_z)) / f64::from(scene.chunk_size_m))
                .floor();
            if !x.is_finite()
                || !z.is_finite()
                || x < 0.
                || z < 0.
                || x >= f64::from(scene.chunk_count_x)
                || z >= f64::from(scene.chunk_count_z)
            {
                return Err("render plant outside domain".into());
            }
            let chunk = domains
                .get(&(x as u32, z as u32))
                .ok_or("render plant missing chunk")?;
            if plant.world_x >= f64::from(chunk.world_x) + f64::from(chunk.valid_width_m)
                || plant.world_z >= f64::from(chunk.world_z) + f64::from(chunk.valid_depth_m)
            {
                return Err("render plant escapes partial chunk".into());
            }
            let target = chunks.get_mut(&(x as u32, z as u32)).unwrap();
            if !target.patches.iter().any(|p| p.id == plant.source_patch_id) {
                return Err("render plant source outside chunk".into());
            }
            target.plants.push(plant);
        }
    }
    fs::create_dir_all(directory.join("render-chunks"))?;
    let mut descriptors = vec![];
    for ((x, z), payload) in &chunks {
        let bytes = serde_json::to_vec(payload)?;
        let sha256 = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        // Content-addressed filenames make index replacement safe for readers
        // holding an older directory; never delete old files during export.
        let file = format!("render-chunks/{x}-{z}-{sha256}.json");
        let path = directory.join(&file);
        if path.exists() {
            if fs::read(&path)? != bytes {
                return Err("conflicting content-addressed payload".into());
            }
        } else {
            publish(&path, &bytes)?;
        }
        descriptors.push(Descriptor {
            chunk_x: *x,
            chunk_z: *z,
            file,
            sha256,
            bytes: bytes.len(),
            patch_count: payload.patches.len(),
            plant_count: payload.plants.len(),
        });
    }
    let mut header = serde_json::json!({
        "sceneId":scene.scene_id,"widthM":scene.width_m,"depthM":scene.depth_m,"originX":scene.origin_x,"originZ":scene.origin_z,
        "chunkSizeM":scene.chunk_size_m,"chunkCountX":scene.chunk_count_x,"chunkCountZ":scene.chunk_count_z,"chunks":scene.chunks,
        "landmarks":scene.landmarks,"entities":scene.entities,"accessSurfaces":scene.access_surfaces,
        "developmentPlan":scene.development_plan,"landcoverPlan":scene.landcover_plan,"landUseZones":scene.land_use_zones,"landUseBudget":scene.land_use_budget,
        "roadNetworkPlan":scene.road_network_plan.as_ref().map(|p|serde_json::json!({"roadsideSites":p.roadside_sites,"jointLayout":p.joint_layout})),
        "landcoverSurfaces":null,
        "renderStream":{"format":"glyphweave.render-directory","version":1,"worldRevision":revision,"chunks":descriptors,"totalPatches":patch_ids.len(),"totalPlants":plant_ids.len()}
    });
    // serde_json::to_value widens f32 into f64 before serializing (0.24 ->
    // 0.2399999946...), unlike the authoritative typed scene serializer.
    // Round-trip ONLY compact metadata through that same typed serializer.
    header["entities"] = exact_json(&scene.entities)?;
    header["landmarks"] = exact_json(&scene.landmarks)?;
    header["accessSurfaces"] = exact_json(&scene.access_surfaces)?;
    header["developmentPlan"] = exact_json(&scene.development_plan)?;
    header["landcoverPlan"] = exact_json(&scene.landcover_plan)?;
    header["landUseZones"] = exact_json(&scene.land_use_zones)?;
    header["landUseBudget"] = exact_json(&scene.land_use_budget)?;
    if let Some(p) = &scene.road_network_plan {
        header["roadNetworkPlan"] = serde_json::json!({"roadsideSites":exact_json(&p.roadside_sites)?,"jointLayout":exact_json(&p.joint_layout)?});
    }
    if let Some(s) = &scene.landcover_surfaces {
        header["landcoverSurfaces"] = serde_json::json!({"version":s.version,"sceneId":s.scene_id,"finalHeightfieldHash":s.final_heightfield_hash,
            "patches":[],"rejections":[],"agriculture":{"model":s.agriculture.model,"fields":s.agriculture.fields,"rejectedFields":[],
                "previewFocusHeights":s.patches.iter().filter(|p|p.material==crate::settlement::landcover::Material::CultivatedSoil).filter_map(|p|p.vertices.first().map(|v|(&p.parcel_id,v[1]))).collect::<BTreeMap<_,_>>()},
            "ecology":{"model":s.ecology.model,"classes":s.ecology.classes,"totalAreaM2":s.ecology.total_area_m2,"status":s.ecology.status,"plants":[],"communities":[],"rejections":[]}});
    }
    publish(
        &directory.join("preview-scene.json"),
        &serde_json::to_vec(&header)?,
    )?;
    Ok(())
}
fn exact_json<T: Serialize>(value: &T) -> Result<serde_json::Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(value)?)?)
}
// Publish from the same filesystem only after the complete payload is flushed.
// A concurrent reader sees the old complete directory or the new one, not half JSON.
fn publish(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temp = path.with_extension(format!("{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seam_bounds_cover_both_chunks_and_partial_edge() {
        let scene:SceneIndex=serde_json::from_value(serde_json::json!({"sceneId":"test","widthM":600,"depthM":600,"originX":-1000,"originZ":50,"chunkSizeM":512,"chunkCountX":2,"chunkCountZ":2,"chunks":[],"entities":[],"landmarks":[]})).unwrap();
        assert_eq!(
            chunk_range(&scene, [-504., 60., -488., 76.]).unwrap(),
            (0, 1, 0, 0)
        );
        assert_eq!(
            chunk_range(&scene, [-488., 562., -400., 650.]).unwrap(),
            (1, 1, 1, 1)
        );
        assert!(chunk_range(&scene, [0., 0., 0., 0.]).is_err());
    }
    fn fixture() -> SceneIndex {
        let mut scene: SceneIndex = serde_json::from_value(serde_json::json!({
            "sceneId":"test","widthM":600,"depthM":600,"originX":-1000,"originZ":50,
            "chunkSizeM":512,"chunkCountX":2,"chunkCountZ":2,"chunks":[],"entities":[],"landmarks":[],
            "landcoverSurfaces":{"version":2,"sceneId":"test","finalHeightfieldHash":"h","rejections":[],
            "agriculture":{"model":"test","fields":[],"rejectedFields":[]},"patches":[{
                "id":"seam","sourceCellId":"cell","parcelId":"parcel","zoneKind":"conservation",
                "material":"conservation_ground","areaM2":3200.0,
                "vertices":[[-520.0,12.25,530.0],[-440.0,12.25,530.0],[-440.0,12.25,570.0],[-520.0,12.25,570.0]],
                "triangles":[[0,1,2],[0,2,3]]}],
            "ecology":{"model":"test","classes":[],"totalAreaM2":3200.0,"status":"pass","rejections":[],"plants":[
                {"id":"p0","sourcePatchId":"seam","kind":"shrub","worldX":-510.0,"worldZ":540.0,"worldY":12.25,"heightM":1.0,"widthM":1.0},
                {"id":"p1","sourcePatchId":"seam","kind":"shrub","worldX":-488.0,"worldZ":562.0,"worldY":12.25,"heightM":1.0,"widthM":1.0}]}
            }
        })).unwrap();
        for z in 0..2 {
            for x in 0..2 {
                scene.chunks.push(
                    serde_json::from_value(serde_json::json!({
                        "chunkX":x,"chunkZ":z,"worldX":-1000+x*512,"worldZ":50+z*512,
                        "validWidthM":if x==0 {512} else {88},"validDepthM":if z==0 {512} else {88},
                        "heightFile":"height","surfaceFile":"surface","lod2File":"lod","hash":"h"
                    }))
                    .unwrap(),
                );
            }
        }
        scene
    }
    #[test]
    fn projection_preserves_exact_geometry_provenance_and_seams() {
        let scene = fixture();
        let dir = std::env::temp_dir().join(format!(
            "glyphweave-stream-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write(&scene, "revision", &dir).unwrap();
        let header: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("preview-scene.json")).unwrap()).unwrap();
        let original = serde_json::to_value(scene.landcover_surfaces.as_ref().unwrap()).unwrap();
        let mut plants = BTreeMap::new();
        for d in header["renderStream"]["chunks"].as_array().unwrap() {
            let bytes = fs::read(dir.join(d["file"].as_str().unwrap())).unwrap();
            assert_eq!(bytes.len() as u64, d["bytes"].as_u64().unwrap());
            assert_eq!(
                Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                d["sha256"].as_str().unwrap()
            );
            let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                data["patches"], original["patches"],
                "seam mesh must not be clipped/rounded"
            );
            for plant in data["plants"].as_array().unwrap() {
                assert!(
                    plants
                        .insert(plant["id"].as_str().unwrap().to_owned(), plant.clone())
                        .is_none()
                );
            }
        }
        assert_eq!(plants.len(), 2);
        for p in original["ecology"]["plants"].as_array().unwrap() {
            assert_eq!(&plants[p["id"].as_str().unwrap()], p);
        }
        let before = fs::read(dir.join("preview-scene.json")).unwrap();
        write(&scene, "revision", &dir).unwrap();
        assert_eq!(before, fs::read(dir.join("preview-scene.json")).unwrap());
        // Cleanup is confined to this uniquely created test directory.
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_projection_inputs_fail_before_publication() {
        // Validation fails before creating the output directory.
        let dir =
            std::env::temp_dir().join(format!("glyphweave-invalid-stream-{}", std::process::id()));
        for mode in 0..11 {
            let mut scene = fixture();
            match mode {
                0 => {
                    scene.chunks.pop();
                }
                1 => scene.chunks[1] = scene.chunks[0].clone(),
                2 => scene.chunks[3].valid_width_m = 512,
                3 => scene.landcover_surfaces.as_mut().unwrap().patches[0].vertices[0][0] = -1001.,
                4 => {
                    scene.landcover_surfaces.as_mut().unwrap().patches[0].vertices[0][1] = f64::NAN
                }
                5 => scene.landcover_surfaces.as_mut().unwrap().patches[0].triangles[0][0] = 999,
                6 => scene.landcover_surfaces.as_mut().unwrap().ecology.plants[0].world_x = -400.,
                7 => {
                    scene.landcover_surfaces.as_mut().unwrap().ecology.plants[0].source_patch_id =
                        "missing".into()
                }
                8 => {
                    let s = scene.landcover_surfaces.as_mut().unwrap();
                    s.patches.push(s.patches[0].clone());
                }
                9 => {
                    let s = scene.landcover_surfaces.as_mut().unwrap();
                    s.ecology.plants.push(s.ecology.plants[0].clone());
                }
                _ => scene.chunks[0].world_x += 1,
            }
            assert!(write(&scene, "revision", &dir).is_err(), "mode {mode}");
            assert!(!dir.exists());
        }
    }
    #[test]
    fn authored_f64_survives_json_decode_and_projection_exactly() {
        // Found by full 6x10km parity check: default fast parsing lost one ULP.
        for value in [
            3.8610000610351562_f64,
            2.9820001125335693,
            9864.000000000002,
        ] {
            let json = serde_json::to_vec(&value).unwrap();
            let decoded: f64 = serde_json::from_slice(&json).unwrap();
            assert_eq!(value.to_bits(), decoded.to_bits());
        }
    }
    #[test]
    fn preview_metadata_uses_authoritative_f32_decimal_serialization() {
        for value in [0.2_f32, 0.24, 0.08, 0.4] {
            let original: serde_json::Value =
                serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(exact_json(&value).unwrap(), original);
            assert_ne!(
                serde_json::to_value(value).unwrap(),
                original,
                "fixture must expose f32 widening"
            );
        }
    }
}
