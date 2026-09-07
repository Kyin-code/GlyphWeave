//! Land-use feasibility at the planning boundary. No relabelling of nature or
//! unassigned cells to rescue a candidate. Rectangle area coverage is exact.
use super::{LandUseZone, SceneSpec};
#[path = "zoning_distance.rs"]
mod distance;
use crate::baked_terrain::TerrainError;
use distance::{Bounds, DistanceIndex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZoningRejection {
    pub candidate_id: String,
    pub kind: String,
    pub min_x: i64,
    pub min_z: i64,
    pub width_m: u32,
    pub depth_m: u32,
    pub conflicting_zone_ids: Vec<String>,
    pub reason: String,
}
struct Zone {
    id: String,
    kind: String,
    x: i64,
    z: i64,
    right: i64,
    bottom: i64,
}
pub(super) struct ZoningMask {
    zones: Vec<Zone>,
    protected_distance: DistanceIndex,
    pub hash: String,
    rows: BTreeMap<i64, Vec<(i64, i64, bool)>>,
}
impl ZoningMask {
    pub fn new(scene: &SceneSpec, zones: &[LandUseZone]) -> Result<Self, TerrainError> {
        let mut ids = BTreeSet::new();
        let mut area = 0_u64;
        let mut rects = vec![];
        for z in zones {
            let x = i64::from(z.world_x);
            let zz = i64::from(z.world_z);
            let r = x + i64::from(z.width_m);
            let b = zz + i64::from(z.depth_m);
            if !ids.insert(&z.id)
                || z.id.is_empty()
                || z.width_m == 0
                || z.depth_m == 0
                || x < i64::from(scene.origin_x)
                || zz < i64::from(scene.origin_z)
                || r > i64::from(scene.origin_x) + i64::from(scene.width_m)
                || b > i64::from(scene.origin_z) + i64::from(scene.depth_m)
            {
                return Err(TerrainError(
                    "invalid zoning input for road planning".into(),
                ));
            }
            area += u64::from(z.width_m) * u64::from(z.depth_m);
            rects.push(Zone {
                id: z.id.clone(),
                kind: z.kind.clone(),
                x,
                z: zz,
                right: r,
                bottom: b,
            });
        }
        rects.sort_by_key(|z| (z.x, z.z));
        for (i, z) in rects.iter().enumerate() {
            for other in rects.iter().skip(i + 1).take_while(|o| o.x < z.right) {
                if z.z < other.bottom && z.bottom > other.z {
                    return Err(TerrainError("overlapping road planning zones".into()));
                }
            }
        }
        if area != u64::from(scene.width_m) * u64::from(scene.depth_m) {
            return Err(TerrainError(
                "zoning must cover the complete road planning scene".into(),
            ));
        }
        let hash =
            blake3::hash(&serde_json::to_vec(zones).map_err(|e| TerrainError(e.to_string()))?)
                .to_hex()
                .to_string();
        let mut rows = BTreeMap::<i64, Vec<_>>::new();
        for zone in &rects {
            for z in zone.z..zone.bottom {
                rows.entry(z).or_default().push((
                    zone.x,
                    zone.right,
                    !matches!(zone.kind.as_str(), "urban" | "rural"),
                ));
            }
        }
        for row in rows.values_mut() {
            row.sort_by_key(|r| r.0);
        }
        let protected_distance = DistanceIndex::new(
            rects
                .iter()
                .filter(|q| !matches!(q.kind.as_str(), "urban" | "rural"))
                .map(|q| Bounds {
                    x: q.x,
                    z: q.z,
                    right: q.right - 1,
                    bottom: q.bottom - 1,
                })
                .collect(),
        );
        Ok(Self {
            protected_distance,
            zones: rects,
            hash,
            rows,
        })
    }
    /// Added fill tapers on the developable side, at most 0.5m per metre
    /// away from a protected raster cell. This avoids a hard cut-off wall.
    /// It bounds the added fill envelope, not natural terrain's own slope.
    pub fn fill_capacity_at(&self, x: i32, z: i32) -> f64 {
        self.protected_distance
            .distance(i64::from(x), i64::from(z))
            .map_or(f64::INFINITY, |d| d as f64 * 0.5)
    }
    pub fn protected_at(&self, x: i32, z: i32) -> bool {
        let Some(row) = self.rows.get(&i64::from(z)) else {
            return false;
        };
        let x = i64::from(x);
        let i = row.partition_point(|r| r.0 <= x);
        i > 0 && x < row[i - 1].1 && row[i - 1].2
    }
    pub fn check(
        &self,
        id: String,
        kind: &str,
        cx: i32,
        cz: i32,
        half_w: i32,
        half_d: i32,
    ) -> Option<ZoningRejection> {
        // Natural sample endpoints included: a half-open geometry footprint
        // has right=cx+half_w; engineering support probes at the edge require
        // an extra metre here to avoid accepting a protected edge sample.
        let x = i64::from(cx) - i64::from(half_w);
        let z = i64::from(cz) - i64::from(half_d);
        let right = i64::from(cx) + i64::from(half_w) + 1;
        let bottom = i64::from(cz) + i64::from(half_d) + 1;
        let mut covered = 0;
        let mut conflicts = vec![];
        for zone in self.zones.iter().take_while(|q| q.x < right) {
            let w = (right.min(zone.right) - x.max(zone.x)).max(0);
            let d = (bottom.min(zone.bottom) - z.max(zone.z)).max(0);
            if w * d == 0 {
                continue;
            }
            covered += w * d;
            if !matches!(zone.kind.as_str(), "urban" | "rural") {
                conflicts.push(zone.id.clone());
            }
        }
        conflicts.sort();
        if conflicts.is_empty() && covered == (right - x) * (bottom - z) {
            return None;
        }
        Some(ZoningRejection {
            candidate_id: id,
            kind: kind.into(),
            min_x: x,
            min_z: z,
            width_m: (right - x) as u32,
            depth_m: (bottom - z) as u32,
            conflicting_zone_ids: conflicts,
            reason: if covered != (right - x) * (bottom - z) {
                "support_footprint_outside_zoning"
            } else {
                "reserved_land_use_intersection"
            }
            .into(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (SceneSpec, Vec<LandUseZone>) {
        let s = SceneSpec {
            scene_id: "z".into(),
            origin_x: -100,
            origin_z: 50,
            width_m: 100,
            depth_m: 100,
            seed_offset: 0,
        };
        let z = vec![
            LandUseZone {
                id: "buildable".into(),
                kind: "rural".into(),
                world_x: -100,
                world_z: 50,
                width_m: 50,
                depth_m: 100,
                source: "test".into(),
            },
            LandUseZone {
                id: "preserve".into(),
                kind: "nature".into(),
                world_x: -50,
                world_z: 50,
                width_m: 50,
                depth_m: 100,
                source: "test".into(),
            },
        ];
        (s, z)
    }
    #[test]
    fn indexed_fill_capacity_equals_original_full_scan_for_all_samples_and_halo() {
        for origin in [-1_000_000_000, 0, 1_000_000_000] {
            let scene = SceneSpec {
                scene_id: "distance-parity".into(),
                origin_x: origin,
                origin_z: origin,
                width_m: 128,
                depth_m: 96,
                seed_offset: 0,
            };
            for kinds in [
                ["urban", "rural", "nature", "unassigned"],
                ["rural"; 4],
                ["nature"; 4],
                ["unknown"; 4],
            ] {
                let zones: Vec<_> = (0..8)
                    .flat_map(|row| (0..8).map(move |col| (row, col)))
                    .map(|(row, col)| LandUseZone {
                        id: format!("z{row}-{col}"),
                        kind: kinds[(row + col) % 4].into(),
                        world_x: origin + col as i32 * 16,
                        world_z: origin + row as i32 * 12,
                        width_m: 16,
                        depth_m: 12,
                        source: "test".into(),
                    })
                    .collect();
                let mask = ZoningMask::new(&scene, &zones).unwrap();
                for dz in -2..98 {
                    for dx in -2..130 {
                        let x = origin + dx;
                        let z = origin + dz;
                        let reference = zones
                            .iter()
                            .filter(|q| !matches!(q.kind.as_str(), "urban" | "rural"))
                            .map(|q| {
                                let xx = i64::from(x);
                                let zz = i64::from(z);
                                let xdist = (i64::from(q.world_x) - xx)
                                    .max(xx - (i64::from(q.world_x) + i64::from(q.width_m) - 1))
                                    .max(0);
                                let zdist = (i64::from(q.world_z) - zz)
                                    .max(zz - (i64::from(q.world_z) + i64::from(q.depth_m) - 1))
                                    .max(0);
                                xdist.max(zdist) as f64 * 0.5
                            })
                            .fold(f64::INFINITY, f64::min);
                        assert_eq!(
                            mask.fill_capacity_at(x, z),
                            reference,
                            "origin={origin} sample={dx},{dz}"
                        );
                        if dx >= 0 && dx < 128 && dz >= 0 && dz < 96 {
                            assert_eq!(mask.protected_at(x, z), reference == 0.);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn full_support_not_just_centre_respects_reserved_zone() {
        let (s, z) = fixture();
        let m = ZoningMask::new(&s, &z).unwrap();
        assert!(m.check("ok".into(), "home", -75, 80, 14, 14).is_none());
        let r = m.check("bad".into(), "home", -60, 80, 14, 14).unwrap();
        assert_eq!(r.conflicting_zone_ids, vec!["preserve"]);
        assert_eq!(r.reason, "reserved_land_use_intersection");
        assert!(m.check("edge".into(), "home", -64, 80, 14, 14).is_some());
    }
    #[test]
    fn malformed_missing_and_overlapping_zones_fail() {
        let (s, z) = fixture();
        let mut bad = z.clone();
        bad.pop();
        assert!(ZoningMask::new(&s, &bad).is_err());
        let mut bad = z.clone();
        bad[1].world_x -= 1;
        bad[1].width_m += 1;
        assert!(ZoningMask::new(&s, &bad).is_err());
        let mut bad = z.clone();
        bad[0].id = bad[1].id.clone();
        assert!(ZoningMask::new(&s, &bad).is_err());
    }
    #[test]
    fn fill_capacity_tapers_before_protected_raster_and_hash_checks_full_inputs() {
        let (s, z) = fixture();
        let m = ZoningMask::new(&s, &z).unwrap();
        for x in -70..-50 {
            assert_eq!(m.fill_capacity_at(x, 80), (-50 - x) as f64 * 0.5);
            assert!(!m.protected_at(x, 80));
        }
        assert_eq!(m.fill_capacity_at(-50, 80), 0.);
        assert!(m.protected_at(-50, 80));
    }
    #[test]
    fn full_protection_audit_detects_single_modified_pixel_and_structure_overlap() {
        use crate::baked_terrain::BakedTerrain;
        let (s, z) = fixture();
        let mut n = BakedTerrain::new(&s).unwrap();
        let bytes = vec![0; 100 * 100 * 2];
        n.insert_chunk(-100, 50, 100, 100, &bytes).unwrap();
        assert_eq!(
            audit_protection(&s, &z, &n, &n, &[]).unwrap().status,
            "pass"
        );
        let mut bytes = bytes;
        let at = (7 * 100 + 51) * 2;
        bytes[at] = 1;
        let mut f = BakedTerrain::new(&s).unwrap();
        f.insert_chunk(-100, 50, 100, 100, &bytes).unwrap();
        let a = audit_protection(&s, &z, &n, &f, &[]).unwrap();
        assert_eq!(a.changed_height_samples, 1);
        assert_eq!(a.max_height_change_m, 0.25);
        assert_eq!(a.protected_area_m2, 5000);
        let e:super::super::EntityInstance=serde_json::from_value(serde_json::json!({"entityId":"authored.house","assetId":"prop.building","kind":"building","worldX":-52,"worldZ":80,"worldY":0,"widthM":8,"depthM":8,"heightM":5,"scale":1})).unwrap();
        let a = audit_protection(&s, &z, &n, &n, &[e]).unwrap();
        assert_eq!(a.conflicting_entity_ids, vec!["authored.house"]);
        assert_eq!(a.status, "fail");
    }

    #[test]
    fn changed_zoning_changes_source_identity() {
        let (s, z) = fixture();
        let a = ZoningMask::new(&s, &z).unwrap();
        let mut z = z;
        z[1].kind = "urban".into();
        let b = ZoningMask::new(&s, &z).unwrap();
        assert_ne!(a.hash, b.hash);
        assert!(b.check("allowed".into(), "road", -30, 80, 10, 10).is_none());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionAudit {
    pub status: String,
    pub protected_area_m2: u64,
    pub changed_height_samples: u64,
    pub max_height_change_m: f64,
    pub conflicting_entity_ids: Vec<String>,
}
/// Independent full-resolution verification, not a stored pass flag. Reserved
/// land forbids earthwork changes and unplanned hard infrastructure footprints.
pub fn audit_protection(
    scene: &SceneSpec,
    zones: &[LandUseZone],
    natural: &crate::baked_terrain::BakedTerrain,
    final_terrain: &crate::baked_terrain::BakedTerrain,
    entities: &[super::EntityInstance],
) -> Result<ProtectionAudit, TerrainError> {
    let mask = ZoningMask::new(scene, zones)?;
    if !natural.matches_scene(scene) || !final_terrain.matches_scene(scene) {
        return Err(TerrainError("protection raster scene mismatch".into()));
    }
    natural.require_complete()?;
    final_terrain.require_complete()?;
    let mut audit = ProtectionAudit {
        status: "pass".into(),
        protected_area_m2: 0,
        changed_height_samples: 0,
        max_height_change_m: 0.,
        conflicting_entity_ids: vec![],
    };
    for zone in mask
        .zones
        .iter()
        .filter(|z| !matches!(z.kind.as_str(), "urban" | "rural"))
    {
        audit.protected_area_m2 += ((zone.right - zone.x) * (zone.bottom - zone.z)) as u64;
        for z in zone.z..zone.bottom {
            for x in zone.x..zone.right {
                let a = natural
                    .height(x as i32, z as i32)
                    .ok_or_else(|| TerrainError("missing protected natural sample".into()))?;
                let b = final_terrain
                    .height(x as i32, z as i32)
                    .ok_or_else(|| TerrainError("missing protected final sample".into()))?;
                if a != b {
                    audit.changed_height_samples += 1;
                    audit.max_height_change_m =
                        audit.max_height_change_m.max(f64::from((a - b).abs()));
                }
            }
        }
        for e in entities.iter().filter(|e| {
            crate::settlement::is_building(&e.kind)
                || matches!(
                    e.kind.as_str(),
                    "road" | "canal" | "parking_lot" | "railway"
                )
        }) {
            let (hw, hd) = super::spatial_half_extents(
                e.width_m * e.scale,
                e.depth_m * e.scale,
                e.rotation_y_deg,
            );
            let x = f64::from(e.world_x);
            let z = f64::from(e.world_z);
            if x + hw > zone.x as f64
                && x - hw < zone.right as f64
                && z + hd > zone.z as f64
                && z - hd < zone.bottom as f64
            {
                audit.conflicting_entity_ids.push(e.entity_id.clone());
            }
        }
    }
    audit.conflicting_entity_ids.sort();
    audit.conflicting_entity_ids.dedup();
    if audit.changed_height_samples > 0 || !audit.conflicting_entity_ids.is_empty() {
        audit.status = "fail".into();
    }
    Ok(audit)
}
