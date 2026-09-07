//! Explicit natural-stage road engineering study. Never applies it to a world.
use super::*;
use glyphweave_core::worldgen::{
    SceneIndex, WorldIndex, WorldManifest,
    road_alignment::{self, CorridorDesign, CorridorRequest},
};
fn inside(root: &Path, path: &Path) -> CliResult<PathBuf> {
    let path = path.canonicalize()?;
    if !path.starts_with(root) {
        return Err("corridor source escapes world directory".into());
    }
    Ok(path)
}
use std::path::PathBuf;
pub(super) fn run(args: &[String], verify: bool) -> CliResult<()> {
    if args.len() != 3 {
        return Err("requires WORLD_DIR REQUEST.json DESIGN.json (design output must be new and outside world)".into());
    }
    let root = fs::canonicalize(&args[0])?;
    let request: CorridorRequest = serde_json::from_reader(fs::File::open(&args[1])?)?;
    let world: WorldIndex = serde_json::from_reader(fs::File::open(root.join("world.json"))?)?;
    let manifest: WorldManifest =
        serde_json::from_reader(fs::File::open(root.join("glyphweave.manifest.json"))?)?;
    manifest.validate()?;
    let spec = manifest
        .scenes
        .iter()
        .find(|s| s.scene_id == request.scene_id)
        .ok_or("corridor scene missing from manifest")?;
    let mut found = None;
    for name in &world.scenes {
        let path = inside(&root, &root.join(name))?;
        let scene: SceneIndex =
            serde_json::from_reader(std::io::BufReader::new(fs::File::open(&path)?))?;
        if scene.scene_id == request.scene_id {
            if found.is_some() {
                return Err("duplicate corridor scene identity".into());
            }
            found = Some((scene, path));
        }
    }
    let (scene, path) = found.ok_or("corridor scene missing from world index")?;
    if (scene.width_m, scene.depth_m, scene.origin_x, scene.origin_z)
        != (spec.width_m, spec.depth_m, spec.origin_x, spec.origin_z)
    {
        return Err("corridor scene domain differs from manifest".into());
    }
    let mut natural = BakedTerrain::new(spec)?;
    for chunk in &scene.chunks {
        let name = chunk
            .natural_height_file
            .as_ref()
            .ok_or("corridor requires natural raster, not final terrain")?;
        let source = inside(
            &root,
            &path.parent().ok_or("scene parent missing")?.join(name),
        )?;
        natural.insert_chunk(
            chunk.world_x,
            chunk.world_z,
            chunk.valid_width_m,
            chunk.valid_depth_m,
            &fs::read(source)?,
        )?;
    }
    natural.require_complete()?;
    if natural.content_hash()? != scene.natural_terrain.heightfield_hash {
        return Err("corridor natural raster hash differs from source snapshot".into());
    }
    let water = glyphweave_core::worldgen::water_geometry(
        glyphweave_core::worldgen::water_kind(&manifest.style),
        &manifest.landmarks,
        spec,
        &manifest.style,
    );
    if verify {
        let design: CorridorDesign = serde_json::from_reader(fs::File::open(&args[2])?)?;
        road_alignment::verify(
            &design,
            &request,
            spec,
            &natural,
            water,
            &scene.land_use_zones,
        )?;
        println!(
            "corridor design verified against natural raster; not a built-road or world-quality certificate"
        );
    } else {
        // Never overwrite an authority file, a prior study, or a report.
        let output = Path::new(&args[2]);
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()?;
        if parent.starts_with(&root) {
            return Err("corridor design output must be outside authoritative world".into());
        }
        let output = parent.join(output.file_name().ok_or("invalid corridor output name")?);
        if output.exists() {
            return Err("corridor design output already exists".into());
        }
        let design =
            road_alignment::design(&request, spec, &natural, water, &scene.land_use_zones)?;
        road_alignment::verify(
            &design,
            &request,
            spec,
            &natural,
            water,
            &scene.land_use_zones,
        )?;
        let bytes = serde_json::to_vec_pretty(&design)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        println!(
            "corridor design_only_not_applied: {}m, max rise {}mm/m, proposed fill {:.3}m3; authoritative world unchanged",
            design.profile_mm.len() - 1,
            design.maximum_rise_mm_per_m,
            design.proposed_fill_volume_m3
        );
    }
    Ok(())
}
