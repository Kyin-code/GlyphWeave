//! Stable optional-candidate identity and read-only inventory change audit.
//! This is not a geometry/rules audit or an implicit legacy-ID migration.
use super::EntityInstance;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet}};

pub const ENTITY_ID_SCHEME: &str = "stable-v1";

/// Original planner slot, before collision rejection or engineered movement.
/// Neither output ordinal nor manifest digest belongs in this key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityCandidateKey {
    pub scene_id: String,
    pub planner: String,
    pub kind: String,
    pub slot: Vec<i64>,
}
impl EntityCandidateKey {
    pub fn stable_id(&self) -> Result<String, String> {
        if self.scene_id.is_empty() || self.planner.is_empty() || self.kind.is_empty() || self.slot.is_empty()
            || !self.planner.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            return Err("invalid stable-v1 candidate key".into());
        }
        // Length-delimited serialization avoids ambiguous joined-string keys.
        let bytes = serde_json::to_vec(&(ENTITY_ID_SCHEME, self)).map_err(|e| e.to_string())?;
        Ok(format!("generated.{}.stable-v1.{}", self.planner, blake3::hash(&bytes).to_hex()))
    }
}

pub(super) struct CandidateIds {
    scene: String,
    keys: RefCell<BTreeSet<EntityCandidateKey>>,
    errors: RefCell<Vec<String>>,
}
impl CandidateIds {
    pub(super) fn new(scene: &str) -> Self {
        Self {scene:scene.into(),keys:RefCell::new(BTreeSet::new()),errors:RefCell::new(vec![])}
    }
    pub(super) fn claim(&self, planner: &str, kind: &str, slot: &[i64]) -> String {
        let key = EntityCandidateKey {scene_id:self.scene.clone(),planner:planner.into(),kind:kind.into(),slot:slot.to_vec()};
        if !self.keys.borrow_mut().insert(key.clone()) {
            self.errors.borrow_mut().push(format!("duplicate candidate key: {key:?}"));
        }
        match key.stable_id() {
            Ok(id) => id,
            Err(error) => {self.errors.borrow_mut().push(error); String::new()},
        }
    }
    pub(super) fn finish(&self, entities: &[EntityInstance]) -> Result<(), String> {
        // Even keys whose proposals were rejected participate in duplicate detection.
        if !self.errors.borrow().is_empty() {return Err(self.errors.borrow().join("; "));}
        unique_entities(entities).map(|_| ())
    }
}

fn unique_entities(entities: &[EntityInstance]) -> Result<BTreeMap<&str, &EntityInstance>, String> {
    let mut map = BTreeMap::new();
    for entity in entities {
        if ![entity.width_m, entity.depth_m, entity.height_m, entity.scale, entity.rotation_y_deg,
            entity.grounding.bottom_offset_m, entity.grounding.roadbed_offset_m, entity.grounding.tolerance_m]
            .iter().all(|v| v.is_finite()) {
            return Err(format!("nonfinite entity fields cannot be compared losslessly: {}", entity.entity_id));
        }
        if entity.entity_id.is_empty() || map.insert(entity.entity_id.as_str(), entity).is_some() {
            return Err(format!("duplicate or empty entity ID: {}",entity.entity_id));
        }
    }
    Ok(map)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EntityChangeKind { Preserved, Added, Moved, Updated, Removed }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityChangeApproval { pub entity_id: String, pub change: EntityChangeKind }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityInventoryChange {
    pub entity_id: String,
    pub changes: Vec<EntityChangeKind>,
    pub approved: bool,
    pub reasons: Vec<String>,
    /// Explicit changed top-level fields, not just counts/hashes.
    pub changed_fields: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityInventoryAudit {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub entries: Vec<EntityInventoryChange>,
    pub failures: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum EntityInventoryMode {
    Fresh,
    /// Scheme metadata must come from the caller's world receipts, not an ID prefix guess.
    Preserving { old_scheme: String, new_scheme: String },
}

/// Additions are reported, not silently credited as preserved. Moves include
/// world XYZ/yaw/bottom offset; simultaneous other changes also require Updated.
/// Anchors, footprint/scale, provenance, and other grounding fields are updates.
/// Unknown schemes are unsupported; legacy -> stable-v1 requires separate migration.
pub fn audit_entity_inventory(
    old: &[EntityInstance],
    new: &[EntityInstance],
    approvals: &[EntityChangeApproval],
    mode: EntityInventoryMode,
) -> EntityInventoryAudit {
    use EntityChangeKind::*;
    let mut audit = EntityInventoryAudit {format:"glyphweave.entity-inventory-audit".into(),version:1,
        status:"pass".into(),entries:vec![],failures:vec![]};
    match mode {
        EntityInventoryMode::Fresh if !old.is_empty() => audit.failures.push("fresh generation cannot claim a nonempty old inventory".into()),
        EntityInventoryMode::Preserving {old_scheme,new_scheme} => {
            if old_scheme != new_scheme {
                audit.failures.push("migration-required: inventory schemes differ; no automatic rename map".into());
            } else if !["legacy",ENTITY_ID_SCHEME].contains(&old_scheme.as_str()) {
                audit.failures.push("unsupported inventory identity scheme".into());
            }
        },
        _ => {},
    }
    let old_map = unique_entities(old);
    let new_map = unique_entities(new);
    if let Err(e) = &old_map {audit.failures.push(format!("old: {e}"));}
    if let Err(e) = &new_map {audit.failures.push(format!("new: {e}"));}
    let mut allowed = BTreeSet::new();
    for approval in approvals {
        if approval.entity_id.is_empty() || approval.entity_id.contains('*')
            || !matches!(approval.change,Removed|Moved|Updated)
            || !allowed.insert((approval.entity_id.as_str(),approval.change)) {
            audit.failures.push(format!("invalid/duplicate exact change approval: {approval:?}"));
        }
    }
    if let (Ok(old_map),Ok(new_map)) = (old_map,new_map) {
        let ids: BTreeSet<_> = old_map.keys().chain(new_map.keys()).copied().collect();
        let mut used = BTreeSet::new();
        for id in ids {
            let mut entry = EntityInventoryChange {entity_id:id.into(),changes:vec![],approved:true,reasons:vec![],changed_fields:vec![]};
            match (old_map.get(id),new_map.get(id)) {
                (None,Some(_)) => {entry.changes.push(Added);entry.reasons.push("new identity; not a preserved legacy object".into());},
                (Some(_),None) => {entry.changes.push(Removed);entry.reasons.push("old identity absent from new final entities".into());},
                (Some(a),Some(b)) => {
                    let mut a = serde_json::to_value(a).expect("EntityInstance serialization");
                    let mut b = serde_json::to_value(b).expect("EntityInstance serialization");
                    for field in a.as_object().unwrap().keys().chain(b.as_object().unwrap().keys()) {
                        if a.get(field) != b.get(field) && !entry.changed_fields.contains(field) {entry.changed_fields.push(field.clone());}
                    }
                    entry.changed_fields.sort();
                    if a == b {entry.changes.push(Preserved);}
                    else {
                        let moved = ["worldX","worldY","worldZ","rotationYDeg"].iter().any(|f| a[*f] != b[*f])
                            || a["grounding"]["bottomOffsetM"] != b["grounding"]["bottomOffsetM"];
                        if moved {
                            entry.changes.push(Moved);
                            for field in ["worldX","worldY","worldZ","rotationYDeg"] {a[field]=serde_json::Value::Null;b[field]=serde_json::Value::Null;}
                            a["grounding"]["bottomOffsetM"]=serde_json::Value::Null;
                            b["grounding"]["bottomOffsetM"]=serde_json::Value::Null;
                        }
                        if a != b {entry.changes.push(Updated);}
                        entry.reasons.push("final entity fields differ; geometry/rules validation is separate".into());
                    }
                },
                _ => unreachable!(),
            }
            for change in &entry.changes {
                if matches!(change,Removed|Moved|Updated) {
                    if allowed.contains(&(id,*change)) {used.insert((id,*change));}
                    else {
                        entry.approved=false;
                        let reason=format!("unapproved {change:?}: {id}");
                        entry.reasons.push(reason.clone());audit.failures.push(reason);
                    }
                }
            }
            audit.entries.push(entry);
        }
        for approval in allowed.difference(&used) {audit.failures.push(format!("unused/nonmatching exact approval: {approval:?}"));}
    }
    if !audit.failures.is_empty() {audit.status="fail".into();}
    audit
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entity(id: &str) -> EntityInstance {
        serde_json::from_value(serde_json::json!({"entityId":id,"assetId":"prop.rock","kind":"rock",
            "worldX":-30,"worldZ":70,"worldY":-2,"widthM":2.,"depthM":2.,"heightM":1.,"scale":1.,
            "grounding":{"bottomOffsetM":0.646}})).unwrap()
    }
    fn preserving() -> EntityInventoryMode {
        EntityInventoryMode::Preserving {old_scheme:ENTITY_ID_SCHEME.into(),new_scheme:ENTITY_ID_SCHEME.into()}
    }
    #[test]
    fn stable_keys_ignore_success_order_unrelated_candidates_and_scene_changes_are_namespaced() {
        let a = CandidateIds::new("scene-a");
        let first=a.claim("natural","tree",&[-32,16]);
        let second=a.claim("natural","tree",&[-16,16]);
        a.finish(&[entity(&first),entity(&second)]).unwrap();
        let b=CandidateIds::new("scene-a");
        b.claim("unrelated-optional","rock",&[1000,1000]);
        // First proposal is rejected: no final entity, but the candidate key is reserved.
        b.claim("natural","tree",&[-32,16]);
        assert_eq!(b.claim("natural","tree",&[-16,16]),second);
        b.finish(&[entity(&second),entity("generated.authored-explicit")]).unwrap();
        assert_ne!(CandidateIds::new("scene-b").claim("natural","tree",&[-16,16]),second);
        assert_ne!(CandidateIds::new("scene-a").claim("natural","bush",&[-16,16]),second);
        let key=EntityCandidateKey {scene_id:"scene-a".into(),planner:"natural".into(),kind:"tree".into(),slot:vec![-16,16]};
        let decoded:EntityCandidateKey=serde_json::from_slice(&serde_json::to_vec(&key).unwrap()).unwrap();
        assert_eq!(decoded.stable_id().unwrap(),second);
    }
    #[test]
    fn duplicate_candidate_keys_including_rejected_and_duplicate_final_ids_fail_closed() {
        let keys=CandidateIds::new("scene");
        let id=keys.claim("natural","tree",&[4,9]);
        assert_eq!(id,keys.claim("natural","tree",&[4,9]));
        assert!(keys.finish(&[]).unwrap_err().contains("duplicate candidate key"));
        let keys=CandidateIds::new("scene");
        assert!(keys.finish(&[entity("explicit"),entity("explicit")]).is_err());
        let key=EntityCandidateKey {scene_id:"scene".into(),planner:"ambiguous/planner".into(),kind:"tree".into(),slot:vec![1]};
        assert!(key.stable_id().is_err());
    }
    #[test]
    fn inventory_requires_exact_approvals_and_keeps_each_field_diff() {
        use EntityChangeKind::*;
        let old=vec![entity("unchanged"),entity("move"),entity("update"),entity("remove")];
        let mut new=old[..3].to_vec();
        new[1].world_x+=1;
        new[2].width_m+=1.;
        new.push(entity("added"));
        let before=serde_json::to_vec(&old).unwrap();
        let denied=audit_entity_inventory(&old,&new,&[],preserving());
        assert_eq!(denied.status,"fail");
        assert_eq!(denied.failures.len(),3);
        for (id,kind) in [("unchanged",Preserved),("move",Moved),("update",Updated),("remove",Removed),("added",Added)] {
            let e=denied.entries.iter().find(|e|e.entity_id==id).unwrap();
            assert_eq!(e.changes,vec![kind]);
        }
        let approvals=vec![EntityChangeApproval{entity_id:"move".into(),change:Moved},
            EntityChangeApproval{entity_id:"update".into(),change:Updated},
            EntityChangeApproval{entity_id:"remove".into(),change:Removed}];
        let allowed=audit_entity_inventory(&old,&new,&approvals,preserving());
        assert_eq!(allowed.status,"pass");
        assert_eq!(allowed.entries.iter().find(|e|e.entity_id=="move").unwrap().changed_fields,vec!["worldX"]);
        let decoded:EntityInventoryAudit=serde_json::from_slice(&serde_json::to_vec(&allowed).unwrap()).unwrap();
        assert_eq!(decoded,allowed);
        assert_eq!(before,serde_json::to_vec(&old).unwrap());
        let mut mixed=old.clone();mixed[1].world_y+=1;mixed[1].asset_id="prop.other".into();
        let partial=audit_entity_inventory(&old,&mixed,&approvals[..1],preserving());
        assert_eq!(partial.status,"fail");
        assert!(partial.failures.iter().any(|r|r.contains("Updated")));
    }
    #[test]
    fn inventory_rejects_duplicate_wildcard_unknown_approval_and_silent_migration() {
        let a=vec![entity("explicit")];
        for (old,new) in [(vec![entity("explicit"),entity("explicit")],a.clone()),(a.clone(),vec![entity("explicit"),entity("explicit")])] {
            assert_eq!(audit_entity_inventory(&old,&new,&[],preserving()).status,"fail");
        }
        for id in ["*","absent"] {
            assert_eq!(audit_entity_inventory(&a,&[],&[EntityChangeApproval{entity_id:id.into(),change:EntityChangeKind::Removed}],preserving()).status,"fail");
        }
        let migration=audit_entity_inventory(&a,&a,&[],EntityInventoryMode::Preserving{old_scheme:"legacy".into(),new_scheme:ENTITY_ID_SCHEME.into()});
        assert_eq!(migration.status,"fail");
        assert!(migration.failures[0].contains("migration-required"));
        assert_eq!(audit_entity_inventory(&a,&a,&[],EntityInventoryMode::Fresh).status,"fail");
        assert_eq!(audit_entity_inventory(&[],&a,&[],EntityInventoryMode::Fresh).status,"pass");
        let mut nan=a.clone();nan[0].rotation_y_deg=f32::NAN;
        assert_eq!(audit_entity_inventory(&nan,&nan,&[],preserving()).status,"fail");
        let mut mm=a.clone();mm[0].grounding.bottom_offset_m+=0.001;
        let audit=audit_entity_inventory(&a,&mm,&[],preserving());
        assert_eq!(audit.status,"fail");assert_eq!(audit.entries[0].changes,vec![EntityChangeKind::Moved]);
    }
}
