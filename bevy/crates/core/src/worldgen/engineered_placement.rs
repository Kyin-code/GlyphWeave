//! Transactional reconciliation after infrastructure pads, before raster bake.
//! Never relax a descriptor, resize/delete an entity, or move an authored
//! landmark to make a gate green. Generated features try bounded nearby sites;
//! the whole local snapshot is returned only after another global audit passes.
//!
//! There is exactly ONE way a world may lose an entity here, and it is written
//! in that entity's own descriptor: `fallback = "skip"` says "if you cannot
//! satisfy me, do not place me". It applies to generated objects only - never to
//! authored entities, landmarks, infrastructure layout (a road, bridge, railway,
//! canal, causeway or sidewalk is a plan, not a candidate) - and every removal is
//! published by name in the report's `skips[]` so it can be recomputed. Two exits
//! honour it: a fixed-class feature that cannot be moved (`exit = "fixed"`) and a
//! movable feature whose bounded nearby search ran out (`exit =
//! "search-exhausted"`). Every other rejection stays a hard error.

use super::*;
use crate::rules::audit::entity_to_placed_with_descriptor;
use crate::rules::errors::RejectRecord;
use crate::rules::schema::Fallback;
use crate::rules::{Footprint, ObjectRegistry, check_hard};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROUNDS: usize = 4;
const SEARCH_RADII_M: [i32; 5] = [2, 4, 8, 16, 32];

/// Kinds whose placement IS infrastructure layout rather than a candidate site.
/// Moving or dropping one could silently break a road graph, a registered bridge
/// or a canal's hydrology, so reconciliation is never allowed to remove them.
const INFRASTRUCTURE_KINDS: [&str; 6] = [
    "road",
    "bridge",
    "railway",
    "canal",
    "causeway",
    "sidewalk",
];

/// May reconciliation REMOVE this entity instead of moving it or failing?
///
/// T-0013 (ii): a GENERATED, non-landmark, non-infrastructure object whose own
/// object descriptor declares `fallback = "skip"` may be skipped rather than
/// failing the whole bake. Its contract already says "if you cannot satisfy me,
/// do not place me"; before this the reconciliation treated every
/// frontage-bearing entity as fixed and returned `fixed entity ... fails
/// engineered terrain`, so one shop whose authored setback (12 m) cannot satisfy
/// its own 8 m road-distance contract made a whole world unbuildable (measured on
/// `morphology_compile`).
///
/// Round 19 measured the other half of the same contract: a *movable* class
/// entity whose bounded nearby search ran out still returned `stalled`, so two
/// storefronts on an 8.57%/11.18% bank made the waterfront opt-in world
/// unbakeable even though both descriptors declare `fallback = "skip"`. The
/// predicate is shared by both exits so they cannot drift.
///
/// Authored entities, landmarks, infrastructure layout and descriptors that ask
/// to move/shrink stay hard errors.
fn descriptor_permits_skip(
    registry: &ObjectRegistry,
    landmarks: &[LandmarkSpec],
    entity: &EntityInstance,
) -> bool {
    entity.entity_id.starts_with("generated.")
        && !landmarks.iter().any(|l| l.entity_id == entity.entity_id)
        && !INFRASTRUCTURE_KINDS.contains(&entity.kind.as_str())
        && crate::rules::audit::descriptor_for(registry, entity)
            .is_some_and(|d| d.placement.fallback == Fallback::Skip)
}

/// One published removal. `reason` is the rejecting rule's own message (the same
/// string the audit reported) and `rule` names the descriptor file that refused
/// the entity, so an auditor can re-run that descriptor and reproduce the skip.
/// `exit` names which of reconciliation's two skip-capable exits produced it.
fn skip_record(
    entity: &EntityInstance,
    round: usize,
    reject: &RejectRecord,
    exit: &'static str,
) -> serde_json::Value {
    serde_json::json!({
        "entityId": &entity.entity_id,
        "kind": &entity.kind,
        "round": round + 1,
        "reason": &reject.reason,
        "rule": &reject.rule,
        "descriptorFallback": "skip",
        "exit": exit,
    })
}

/// The stall exit's one way forward: remove exactly the rejected entities whose
/// own descriptor permits it, in audit order (so `skips[]` is deterministic),
/// and report whether anything was removed.
///
/// This never widens a numeric contract. An entity is removed only because its
/// descriptor authorizes it; every other rejected entity still fails the bake,
/// which is why the caller turns "nothing removable" into the same `stalled`
/// error as before.
fn remove_skippable_rejects(
    registry: &ObjectRegistry,
    landmarks: &[LandmarkSpec],
    entities: &mut Vec<EntityInstance>,
    rejects: &[RejectRecord],
    round: usize,
    skips: &mut Vec<serde_json::Value>,
) -> bool {
    let mut ordered: Vec<(String, serde_json::Value)> = Vec::new();
    for reject in rejects {
        let Some(entity) = entities.iter().find(|e| e.entity_id == reject.item_id) else {
            continue;
        };
        if !descriptor_permits_skip(registry, landmarks, entity) {
            continue;
        }
        if ordered.iter().any(|(id, _)| *id == entity.entity_id) {
            continue;
        }
        ordered.push((
            entity.entity_id.clone(),
            skip_record(entity, round, reject, "search-exhausted"),
        ));
    }
    if ordered.is_empty() {
        return false;
    }
    let removed: BTreeSet<String> = ordered.iter().map(|(id, _)| id.clone()).collect();
    entities.retain(|entity| !removed.contains(&entity.entity_id));
    skips.extend(ordered.into_iter().map(|(_, record)| record));
    true
}

pub(super) fn reconcile(
    seed: u64,
    scene: &SceneSpec,
    landmarks: &[LandmarkSpec],
    base: &[EntityInstance],
    water: WaterGeometry,
    rules_dir: &Path,
    terrain_context: &TerrainContext<'_>,
) -> WorldgenResult<(Vec<EntityInstance>, Vec<TerrainCarve>, serde_json::Value)> {
    let registry = ObjectRegistry::load_dir(rules_dir)
        .map_err(|e| WorldgenError::InvalidRules(e.to_string()))?;
    let mut entities = base.to_vec();
    let mut moves = Vec::new();
    let mut skips: Vec<serde_json::Value> = Vec::new();
    let mut original_positions = BTreeMap::new();
    for entity in base {
        original_positions.insert(entity.entity_id.clone(), (entity.world_x, entity.world_z));
    }
    // Rounds `0..MAX_ROUNDS` may move a feature to a nearby legal site. The
    // `MAX_ROUNDS` round may not move anything (that is what "exhausted" always
    // meant), but it may still honour a descriptor's own fallback; the extra pass
    // exists so a removal made in that last round is confirmed by an independent
    // audit instead of falling out of this loop.
    for round in 0..=(MAX_ROUNDS + 1) {
        let carves = plan_terrain_carves(&entities);
        let height = |x, z| terrain_context.height(x, z, &carves);
        let audit = audit_scene(
            seed,
            scene,
            landmarks,
            &entities,
            water,
            rules_dir,
            AuditOptions {
                height_at: Some(&height),
                slope_half: None,
            },
        )
        .map_err(|e| WorldgenError::InvalidRules(e.to_string()))?;
        if audit.rejected_items == 0 {
            let output_items = entities.len();
            let report = serde_json::json!({
                "sceneId":scene.scene_id,"rounds":round,"moves":moves,
                "inputItems":base.len(),"outputItems":output_items,"removedItems":skips.len(),
                // The plan and the output are published as ids so an auditor
                // can recompute every removal without regenerating a world:
                // a skip must name a planned item, that item must be absent
                // from the output and from the published scene, and plan =
                // output + skips must conserve.
                "inputEntityIds": base.iter().map(|e| e.entity_id.as_str()).collect::<Vec<_>>(),
                "outputEntityIds": entities.iter().map(|e| e.entity_id.as_str()).collect::<Vec<_>>(),
                "skips":skips,
                "checkedItems":audit.checked_items,"rejectedItems":0,
            });
            return Ok((entities, carves, report));
        }
        let unresolved = serde_json::to_string(&audit.rejects)?;
        if round > MAX_ROUNDS {
            return Err(WorldgenError::InvalidRules(format!(
                "engineering reconciliation exhausted in {}: {}",
                scene.scene_id, unresolved
            )));
        }
        let rejects = audit.rejects;
        let mut progress = false;
        if round < MAX_ROUNDS {
            for reject in &rejects {
                let Some(index) = entities
                    .iter()
                    .position(|e| e.entity_id == reject.item_id)
                else {
                    continue;
                };
                let source = entities[index].clone();
                // Infrastructure layout and user-authored entities are fixed.
                // Moving an entire road could silently break its graph / bridge
                // connections.
                if source.grounding.frontage_road_point.is_some()
                    || !source.entity_id.starts_with("generated.")
                    || landmarks.iter().any(|l| l.entity_id == source.entity_id)
                    || INFRASTRUCTURE_KINDS.contains(&source.kind.as_str())
                {
                    // T-0013 (ii): a GENERATED frontage object whose own object
                    // descriptor declares `fallback = skip` may be SKIPPED rather
                    // than failing the whole bake. See `descriptor_permits_skip`
                    // for why this exit and the stall exit share one predicate.
                    if descriptor_permits_skip(&registry, landmarks, &source) {
                        entities.remove(index);
                        skips.push(skip_record(&source, round, reject, "fixed"));
                        progress = true;
                        continue;
                    }
                    return Err(WorldgenError::InvalidRules(format!(
                        "fixed entity {} fails engineered terrain: {}",
                        source.entity_id, reject.reason
                    )));
                }
                let Some(desc) = crate::rules::audit::descriptor_for(&registry, &source) else {
                    continue;
                };
                let (hard, _) = crate::rules::constraint::compile(desc);
                let others: Vec<_> = entities
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, e)| {
                        let descriptor = crate::rules::audit::descriptor_for(&registry, e);
                        entity_to_placed_with_descriptor(e, descriptor)
                    })
                    .collect();
                let other_entities: Vec<_> = entities
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, e)| e.clone())
                    .collect();
                let other_carves = plan_terrain_carves(&other_entities);
                let (origin_x, origin_z) = original_positions[&source.entity_id];
                let mut offsets = vec![(0, 0)];
                if matches!(desc.placement.fallback, crate::rules::schema::Fallback::Move) {
                    for radius in SEARCH_RADII_M {
                        for (dx, dz) in [
                            (0, -1),
                            (1, 0),
                            (0, 1),
                            (-1, 0),
                            (1, -1),
                            (1, 1),
                            (-1, 1),
                            (-1, -1),
                        ] {
                            offsets.push((dx * radius, dz * radius));
                        }
                    }
                }
                for (dx, dz) in offsets {
                    let mut candidate = source.clone();
                    candidate.world_x = origin_x.saturating_add(dx);
                    candidate.world_z = origin_z.saturating_add(dz);
                    let (hw, hd) = spatial_half_extents(
                        candidate.width_m,
                        candidate.depth_m,
                        candidate.rotation_y_deg,
                    );
                    let fp = Footprint {
                        rotation_y_deg: candidate.rotation_y_deg,
                        cx: candidate.world_x as f32,
                        cz: candidate.world_z as f32,
                        half_w: hw as f32 + desc.geometry.clearance,
                        half_d: hd as f32 + desc.geometry.clearance,
                    };
                    if fp.cx - fp.half_w < scene.origin_x as f32
                        || fp.cz - fp.half_d < scene.origin_z as f32
                        || fp.cx + fp.half_w > scene.origin_x as f32 + scene.width_m as f32
                        || fp.cz + fp.half_d > scene.origin_z as f32 + scene.depth_m as f32
                    {
                        continue;
                    }
                    // Do not carry the old site's elevation or support pad to the new
                    // site. Sample its full footprint against other committed works.
                    let mut ceiling = f32::NEG_INFINITY;
                    for z in -(hd.ceil() as i32)..=hd.ceil() as i32 {
                        for x in -(hw.ceil() as i32)..=hw.ceil() as i32 {
                            ceiling = ceiling.max(terrain_context.height(
                                candidate.world_x + x,
                                candidate.world_z + z,
                                &other_carves,
                            ));
                        }
                    }
                    let offset = candidate.ground_height_m() - candidate.world_y as f32;
                    candidate.world_y = (ceiling - offset).ceil() as i32;
                    let mut trial_carves = other_carves.clone();
                    trial_carves.extend(plan_terrain_carves(std::slice::from_ref(&candidate)));
                    trial_carves.sort_by_key(|c| c.priority);
                    let height = |x, z| terrain_context.height(x, z, &trial_carves);
                    let valid =
                        with_height_placement_context(seed, scene, water, &height, |ctx| {
                            check_hard(desc, &fp, ctx, &hard, &others).is_ok()
                        });
                    if !valid {
                        continue;
                    }
                    let delta_x = candidate.world_x - source.world_x;
                    let delta_z = candidate.world_z - source.world_z;
                    for anchor in &mut candidate.anchors {
                        anchor.world_x = anchor.world_x.saturating_add(delta_x);
                        anchor.world_z = anchor.world_z.saturating_add(delta_z);
                    }
                    candidate.bounds = Some(candidate.computed_bounds());
                    if (source.world_x, source.world_z, source.world_y)
                        == (candidate.world_x, candidate.world_z, candidate.world_y)
                    {
                        continue;
                    }
                    moves.push(serde_json::json!({"entityId":source.entity_id,"round":round+1,"reason":&reject.reason,
                        "from":{"x":source.world_x,"z":source.world_z,"y":source.world_y},
                        "to":{"x":candidate.world_x,"z":candidate.world_z,"y":candidate.world_y}}));
                    entities[index] = candidate;
                    progress = true;
                    break;
                }
            }
        }
        if !progress {
            // Nothing could be moved. A descriptor that asks to be skipped is not
            // a reason to fail a whole world, so remove those (and only those) by
            // name; everything else still fails below.
            progress =
                remove_skippable_rejects(&registry, landmarks, &mut entities, &rejects, round, &mut skips);
        }
        if !progress {
            // Only a round with NO skippable reject is fatal. The two exits keep
            // their existing messages: a stalled round versus the bounded round
            // budget running out.
            return Err(WorldgenError::InvalidRules(if round == MAX_ROUNDS {
                format!(
                    "engineering reconciliation exhausted in {}: {}",
                    scene.scene_id, unresolved
                )
            } else {
                format!(
                    "engineering placement stalled in {} at round {}; unresolved={}",
                    scene.scene_id, round, unresolved
                )
            }));
        }
    }
    unreachable!("bounded engineering loop returns success or failure")
}

/// Independently recomputed summary of one published reconciliation report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacementReportAudit {
    pub plan_items: usize,
    pub output_items: usize,
    pub skipped_items: usize,
    pub skipped_by_rule: BTreeMap<String, usize>,
}

fn string_ids(report: &serde_json::Value, field: &str) -> Option<Vec<String>> {
    report
        .get(field)?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

/// Recompute ONE published `engineering-placement.json` scene entry from the
/// artifacts a bake published.
///
/// Removal is the only way reconciliation can shrink a scene, so the report
/// publishes the reconcile plan's ids, its output ids and the `skips[]` records.
/// This audit refuses, by name:
///
///  * a `removedItems` count that disagrees with the published records;
///  * a removal published without the plan ids needed to recompute it;
///  * a plan that does not conserve (`plan = output + skips`, no duplicates);
///  * a skip that names an item that was never planned, a skip that is still in
///    the reconcile output or still published in the scene, an authored entity,
///    an authored landmark, or infrastructure (road/bridge/railway/canal/
///    causeway/sidewalk);
///  * a skip whose `reason` names nothing, whose `rule` is not a loaded
///    descriptor, whose descriptor does not govern the skipped kind, or whose
///    descriptor does not declare `fallback = "skip"`;
///  * `plannedPadIds`: a pad the ROAD PLAN planned that is neither in the
///    reconcile output nor named as a skip, or that is in the output but not
///    published. This is the triangle Round 23 fell through - plan, ledger and
///    world must agree about which pads exist, and a pad deleted from the world
///    before reconciliation left `inputItems == outputItems` and every other
///    audit agreeing with itself;
///  * `refusedEntityIds`: a pad the planner REFUSED by name that is still in the
///    reconcile plan, still in its output, published in the scene or an authored
///    landmark. A published refusal cannot be quietly ignored downstream.
///
/// A report baked before this shape existed carries no ids; when it also removed
/// nothing there is nothing to recompute and it stays readable.
pub fn verify_published_placement(
    report: &serde_json::Value,
    published_entity_ids: &BTreeSet<String>,
    published_landmark_ids: &BTreeSet<String>,
    registry: Option<&ObjectRegistry>,
) -> Result<PlacementReportAudit, String> {
    let scene_id = report
        .get("sceneId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<unnamed>");
    let skips: &[serde_json::Value] = report
        .get("skips")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let removed_items = match report
        .get("removedItems")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) => value as usize,
        None => {
            // Before Round 2 the report had no removal bookkeeping at all, so a
            // report with neither records nor plan ids is a legacy artifact, not
            // an unreadable one. Anything else must be recomputable.
            if skips.is_empty() && report.get("inputEntityIds").is_none() {
                return Ok(PlacementReportAudit::default());
            }
            return Err(format!("{scene_id}: report has no removedItems"));
        }
    };
    if removed_items != skips.len() {
        return Err(format!(
            "{scene_id}: removedItems={removed_items} but {} skip record(s) are published",
            skips.len()
        ));
    }
    let plan_ids = string_ids(report, "inputEntityIds");
    let output_ids = string_ids(report, "outputEntityIds");
    let (Some(plan_ids), Some(output_ids)) = (plan_ids, output_ids) else {
        if removed_items > 0 {
            return Err(format!(
                "{scene_id}: {removed_items} removal(s) are published without inputEntityIds/outputEntityIds, so no skip can be recomputed"
            ));
        }
        return Ok(PlacementReportAudit::default());
    };
    let read_count = |field: &str| -> Result<usize, String> {
        report
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as usize)
            .ok_or_else(|| format!("{scene_id}: report has no {field}"))
    };
    let input_items = read_count("inputItems")?;
    let output_items = read_count("outputItems")?;
    if input_items != plan_ids.len() {
        return Err(format!(
            "{scene_id}: inputItems={input_items} but {} plan ids are published",
            plan_ids.len()
        ));
    }
    if output_items != output_ids.len() {
        return Err(format!(
            "{scene_id}: outputItems={output_items} but {} output ids are published",
            output_ids.len()
        ));
    }
    if input_items != output_items + removed_items {
        return Err(format!(
            "{scene_id}: plan does not conserve: inputItems={input_items} != outputItems={output_items} + removedItems={removed_items}"
        ));
    }
    let plan: BTreeSet<&str> = plan_ids.iter().map(String::as_str).collect();
    if plan.len() != plan_ids.len() {
        return Err(format!("{scene_id}: the plan publishes a duplicate item id"));
    }
    let output: BTreeSet<&str> = output_ids.iter().map(String::as_str).collect();
    if output.len() != output_ids.len() {
        return Err(format!("{scene_id}: the output publishes a duplicate item id"));
    }
    // A refusal the PLANNER published (`refusedEntityIds`, written only when a pad was refused by
    // name - see `road_network::PAD_FILL_CONFLICT_CATEGORY`) must be honoured end to end. A refused
    // item that is still in the reconcile plan, still in its output, or still standing in the
    // published scene is the exact defect this field exists to catch: planned, booked and absent (or
    // worse, planned, booked and present while its own contract fails). Refusing it here means a
    // reported refusal cannot be quietly ignored downstream.
    for id in string_ids(report, "refusedEntityIds").unwrap_or_default() {
        if plan.contains(id.as_str()) {
            return Err(format!(
                "{scene_id}: item {id} is refused by the planner and is still in the reconcile plan"
            ));
        }
        if output.contains(id.as_str()) {
            return Err(format!(
                "{scene_id}: item {id} is refused by the planner and is still in the reconcile output"
            ));
        }
        if published_entity_ids.contains(&id) {
            return Err(format!(
                "{scene_id}: item {id} is refused by the planner and is published in the scene"
            ));
        }
        if published_landmark_ids.contains(&id) {
            return Err(format!(
                "{scene_id}: item {id} is refused by the planner and is an authored landmark"
            ));
        }
    }
    // The other half of the same triangle: every pad the road plan PLANNED must be accounted for -
    // either reconciled into the output (and then standing in the published scene) or named as a
    // skip. A planned pad that is neither is exactly the Round-23 defect: the plan and the ledger
    // kept it, the world did not, and every audit agreed with itself because none of them compared
    // the plan's pad list with the published ids. Refusing it here compares all three.
    let skip_ids: BTreeSet<&str> = skips
        .iter()
        .filter_map(|skip| skip.get("entityId").and_then(serde_json::Value::as_str))
        .collect();
    for id in string_ids(report, "plannedPadIds").unwrap_or_default() {
        if !output.contains(id.as_str()) {
            if skip_ids.contains(id.as_str()) {
                continue;
            }
            return Err(format!(
                "{scene_id}: planned pad {id} is neither in the reconcile output nor named as a skip; \
                 the plan and the ledger claim a pad the world does not carry"
            ));
        }
        if !published_entity_ids.contains(&id) {
            return Err(format!(
                "{scene_id}: planned pad {id} is in the reconcile output but is not published; \
                 the plan and the ledger claim a pad the world does not carry"
            ));
        }
    }
    for id in &output {
        if !plan.contains(id) {
            return Err(format!(
                "{scene_id}: output item {id} was never in the reconcile plan"
            ));
        }
    }
    for id in published_entity_ids {
        if !output.contains(id.as_str()) {
            return Err(format!(
                "{scene_id}: published entity {id} is not in the reconcile output"
            ));
        }
    }
    let mut audit = PlacementReportAudit {
        plan_items: input_items,
        output_items,
        skipped_items: removed_items,
        skipped_by_rule: BTreeMap::new(),
    };
    let mut skipped: BTreeSet<String> = BTreeSet::new();
    for skip in skips {
        let field = |name: &str| skip.get(name).and_then(serde_json::Value::as_str);
        let entity_id = field("entityId")
            .ok_or_else(|| format!("{scene_id}: a skip record has no entityId"))?;
        let kind = field("kind").unwrap_or_default();
        let reason = field("reason").unwrap_or_default();
        let rule = field("rule").unwrap_or_default();
        if !skipped.insert(entity_id.to_owned()) {
            return Err(format!(
                "{scene_id}: entity {entity_id} is skipped more than once"
            ));
        }
        if !plan.contains(entity_id) {
            return Err(format!(
                "{scene_id}: skip {entity_id} is not a planned item, so the rejection cannot be recomputed"
            ));
        }
        if output.contains(entity_id) {
            return Err(format!(
                "{scene_id}: skip {entity_id} is still present in the reconcile output"
            ));
        }
        if published_entity_ids.contains(entity_id) {
            return Err(format!(
                "{scene_id}: skip {entity_id} is still present in the published scene"
            ));
        }
        if published_landmark_ids.contains(entity_id) {
            return Err(format!(
                "{scene_id}: skip {entity_id} names an authored landmark, which is never skippable"
            ));
        }
        if !entity_id.starts_with("generated.") {
            return Err(format!(
                "{scene_id}: skip {entity_id} names an authored entity, which is never skippable"
            ));
        }
        if kind.is_empty() {
            return Err(format!("{scene_id}: skip {entity_id} has no kind"));
        }
        if INFRASTRUCTURE_KINDS.contains(&kind) {
            return Err(format!(
                "{scene_id}: skip {entity_id} names infrastructure kind {kind:?}, which is never skippable"
            ));
        }
        if field("descriptorFallback") != Some("skip") {
            return Err(format!(
                "{scene_id}: skip {entity_id} does not declare descriptorFallback=skip"
            ));
        }
        if reason.is_empty() {
            return Err(format!(
                "{scene_id}: skip {entity_id} has no rejecting reason, so it points at no rule"
            ));
        }
        if rule.is_empty() {
            return Err(format!(
                "{scene_id}: skip {entity_id} names no descriptor file"
            ));
        }
        if let Some(registry) = registry {
            let descriptor = registry
                .descriptors
                .values()
                .find(|d| format!("{}.object.toml", d.id) == rule)
                .ok_or_else(|| {
                    format!(
                        "{scene_id}: skip {entity_id} names rule {rule}, which is not a loaded descriptor"
                    )
                })?;
            if descriptor.placement.fallback != Fallback::Skip {
                return Err(format!(
                    "{scene_id}: skip {entity_id} names rule {rule}, which does not declare fallback=skip"
                ));
            }
            if !descriptor.matches_kind(kind) {
                return Err(format!(
                    "{scene_id}: skip {entity_id} names rule {rule}, which does not govern kind {kind:?}"
                ));
            }
        }
        *audit.skipped_by_rule.entry(rule.to_owned()).or_default() += 1;
    }
    for id in &plan {
        if !output.contains(id) && !skipped.contains(*id) {
            return Err(format!(
                "{scene_id}: plan item {id} is neither in the output nor named as a removal"
            ));
        }
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{GroundingSpec, WaterKind, water_geometry};
    use std::path::PathBuf;

    fn rules_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/objects")
    }

    fn scene() -> SceneSpec {
        SceneSpec {
            scene_id: "engineering".into(),
            width_m: 256,
            depth_m: 256,
            origin_x: 0,
            origin_z: 0,
            seed_offset: 0,
        }
    }

    fn water(scene: &SceneSpec) -> WaterGeometry {
        water_geometry(WaterKind::None, &[], scene, &serde_json::Value::Null)
    }

    fn context(water: WaterGeometry) -> TerrainContext<'static> {
        TerrainContext {
            seed: 42,
            water,
            precise: true,
            zoning: None,
        }
    }

    fn entity(id: &str, kind: &str, x: i32, z: i32, size: f32) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: format!("prop.{kind}"),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            scale: 1.0,
            width_m: size,
            depth_m: size,
            height_m: 6.0,
            rotation_y_deg: 0.0,
            grounding: GroundingSpec::default(),
            anchors: Vec::new(),
            bounds: None,
        }
    }

    /// Sit the entity on the highest sample of its own footprint so the audit's
    /// grounding clause is not the reason it is refused.
    fn ground(entity: &mut EntityInstance, context: &TerrainContext<'_>) {
        let (hw, hd) = spatial_half_extents(entity.width_m, entity.depth_m, entity.rotation_y_deg);
        let mut top = f32::NEG_INFINITY;
        for z in -(hd.ceil() as i32)..=hd.ceil() as i32 {
            for x in -(hw.ceil() as i32)..=hw.ceil() as i32 {
                top = top.max(context.height(entity.world_x + x, entity.world_z + z, &[]));
            }
        }
        entity.world_y = top.ceil() as i32;
    }

    /// A storefront cannot be placed inside a 160 m green space: it overlaps the
    /// green area at its own site and at every site the bounded search can try
    /// (the search radii are at most 32 m), so `reconcile` has to reach its stall
    /// exit. The green space is movable but blocked the same way, so this fixture
    /// reproduces Round 19's `stalled` refusal without a real waterfront.
    fn collision_fixture() -> (SceneSpec, WaterGeometry, Vec<EntityInstance>) {
        let scene = scene();
        let water = water(&scene);
        let ctx = context(water);
        let mut green = entity("generated.green-space.0", "green_space", 128, 128, 160.0);
        ground(&mut green, &ctx);
        let mut shop = entity("generated.storefront.0", "storefront", 128, 128, 8.0);
        shop.depth_m = 6.0;
        shop.height_m = 4.0;
        ground(&mut shop, &ctx);
        (scene, water, vec![green, shop])
    }

    fn reconcile_fixture(
        scene: &SceneSpec,
        water: WaterGeometry,
        entities: &[EntityInstance],
        landmarks: &[LandmarkSpec],
    ) -> WorldgenResult<(Vec<EntityInstance>, Vec<TerrainCarve>, serde_json::Value)> {
        // `TerrainContext` borrows nothing here; keep it alive for the call.
        let ctx = context(water);
        reconcile(42, scene, landmarks, entities, water, &rules_dir(), &ctx)
    }

    fn skips_of(report: &serde_json::Value) -> Vec<serde_json::Value> {
        report["skips"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn stalled_search_honours_descriptor_fallback_and_names_the_skip() {
        let (scene, water, entities) = collision_fixture();
        let (repaired, _carves, report) =
            reconcile_fixture(&scene, water, &entities, &[]).expect("a skippable feature must not fail the world");
        // The storefront is gone; the green space (a movable descriptor) stays.
        assert_eq!(repaired.len(), 1);
        assert_eq!(repaired[0].entity_id, "generated.green-space.0");
        assert_eq!(report["inputItems"], serde_json::json!(2));
        assert_eq!(report["outputItems"], serde_json::json!(1));
        assert_eq!(report["removedItems"], serde_json::json!(1));
        let skips = skips_of(&report);
        assert_eq!(skips.len(), 1);
        assert_eq!(
            skips[0]["entityId"], serde_json::json!("generated.storefront.0"),
            "the skipped feature must be named"
        );
        assert_eq!(skips[0]["kind"], serde_json::json!("storefront"));
        assert_eq!(skips[0]["descriptorFallback"], serde_json::json!("skip"));
        assert_eq!(skips[0]["exit"], serde_json::json!("search-exhausted"));
        assert_eq!(skips[0]["rule"], serde_json::json!("storefront.object.toml"));
        assert_eq!(skips[0]["round"], serde_json::json!(1));
        let reason = skips[0]["reason"].as_str().unwrap();
        assert!(
            reason.starts_with("geometry collision with Building ("),
            "the skip must record the audit's own rejection reason, got {reason:?}"
        );
        // Published ids make the removal recomputable and conserve exactly.
        let plan: Vec<&str> = report["inputEntityIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let output: Vec<&str> = report["outputEntityIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(plan, vec!["generated.green-space.0", "generated.storefront.0"]);
        assert_eq!(output, vec!["generated.green-space.0"]);
        // The true audit must agree with the published report.
        let published: BTreeSet<String> = output.iter().map(|id| (*id).to_owned()).collect();
        let audit = verify_published_placement(&report, &published, &BTreeSet::new(), None)
            .expect("a legitimate skip must recompute");
        assert_eq!(audit.plan_items, 2);
        assert_eq!(audit.output_items, 1);
        assert_eq!(audit.skipped_items, 1);
        assert_eq!(audit.skipped_by_rule["storefront.object.toml"], 1);
    }

    #[test]
    fn stalled_search_keeps_a_move_descriptor_a_hard_error() {
        let scene = scene();
        let water = water(&scene);
        let ctx = context(water);
        let mut green = entity("generated.green-space.0", "green_space", 128, 128, 160.0);
        ground(&mut green, &ctx);
        // `school` is governed by a descriptor that declares `fallback = "move"`:
        // its contract does NOT authorize removal, so a searched-out stall stays
        // fatal even though it is generated.
        let mut school = entity("generated.school.0", "school", 128, 128, 24.0);
        school.height_m = 8.0;
        ground(&mut school, &ctx);
        let error = reconcile_fixture(&scene, water, &[green, school], &[])
            .expect_err("a move-only descriptor must not be skipped");
        let message = error.to_string();
        assert!(
            message.contains("stalled") && message.contains("generated.school.0"),
            "expected a stalled refusal naming the school, got {message}"
        );
        assert!(
            message.contains("geometry collision"),
            "the recorded reason must be the rule that refused it, got {message}"
        );
    }

    #[test]
    fn authored_landmark_and_infrastructure_stay_hard_errors() {
        let scene = scene();
        let water = water(&scene);
        let ctx = context(water);
        let mut green = entity("generated.green-space.0", "green_space", 128, 128, 160.0);
        ground(&mut green, &ctx);

        // 1. An authored entity, even with a skip-descriptor kind.
        let mut authored = entity("authored.house", "building", 128, 128, 12.0);
        ground(&mut authored, &ctx);
        let error = reconcile_fixture(&scene, water, &[green.clone(), authored], &[])
            .expect_err("authored entities are never skippable");
        assert!(
            error.to_string().contains("fixed entity authored.house"),
            "{}",
            error
        );

        // 2. An authored landmark, even with a skip-descriptor kind.
        let landmark: LandmarkSpec = serde_json::from_value(serde_json::json!({
            "entityId":"landmark.shop","name":"authored shop","type":"building",
            "purpose":"authored","description":"authored shop","sceneId":"engineering",
            "worldX":128,"worldZ":128,"worldY":0,
            "widthM":12,"depthM":12,"heightM":8,"assetId":"prop.building"
        }))
        .unwrap();
        let mut landmark_entity = entity("landmark.shop", "building", 128, 128, 12.0);
        ground(&mut landmark_entity, &ctx);
        let error = reconcile_fixture(
            &scene,
            water,
            &[green.clone(), landmark_entity],
            std::slice::from_ref(&landmark),
        )
        .expect_err("authored landmarks are never skippable");
        assert!(
            error.to_string().contains("fixed entity landmark.shop"),
            "{}",
            error
        );

        // 3. Infrastructure layout: a generated road at the scene edge is out of
        //    bounds. Infrastructure takes the fixed-class exit (it is never even
        //    offered a nearby site), so its own descriptor's `fallback = "skip"`
        //    cannot remove a road.
        let mut road = entity("generated.road.0", "road", 0, 100, 8.0);
        road.depth_m = 8.0;
        road.height_m = 0.5;
        let error = reconcile_fixture(&scene, water, &[road], &[])
            .expect_err("infrastructure is never skippable");
        let message = error.to_string();
        assert!(
            message.contains("fixed entity generated.road.0"),
            "expected a fixed-entity refusal naming the road, got {message}"
        );
        assert!(message.contains("out of bounds"), "{message}");
    }

    #[test]
    fn skip_records_are_identical_on_two_bakes() {
        let (scene, water, entities) = collision_fixture();
        let (first_entities, _, first) = reconcile_fixture(&scene, water, &entities, &[])
            .expect("fixture reconciles");
        let (second_entities, _, second) = reconcile_fixture(&scene, water, &entities, &[])
            .expect("fixture reconciles");
        assert_eq!(first, second, "the whole report must be deterministic");
        assert_eq!(
            first_entities
                .iter()
                .map(|e| e.entity_id.clone())
                .collect::<Vec<_>>(),
            second_entities
                .iter()
                .map(|e| e.entity_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(skips_of(&first), skips_of(&second));
    }

    #[test]
    fn published_skips_are_recomputable_and_forgery_is_refused() {
        let (scene, water, entities) = collision_fixture();
        let (repaired, _, report) = reconcile_fixture(&scene, water, &entities, &[])
            .expect("fixture reconciles");
        let published: BTreeSet<String> =
            repaired.iter().map(|e| e.entity_id.clone()).collect();
        let registry = ObjectRegistry::load_dir(&rules_dir()).unwrap();
        verify_published_placement(&report, &published, &BTreeSet::new(), Some(&registry))
            .expect("a published legitimate skip must recompute against the real descriptors");

        // Deleting the record while the entity is still gone breaks plan
        // conservation (and the removedItems count).
        let mut deleted = report.clone();
        deleted["skips"] = serde_json::json!([]);
        deleted["removedItems"] = serde_json::json!(0);
        assert!(
            verify_published_placement(&deleted, &published, &BTreeSet::new(), None).is_err(),
            "a deleted skip record must be refused"
        );

        // Publishing the entity without its removal is refused too.
        let mut kept = published.clone();
        kept.insert("generated.storefront.0".into());
        assert!(
            verify_published_placement(&report, &kept, &BTreeSet::new(), None).is_err(),
            "a skip for an entity that is still published must be refused"
        );

        // Forging a record for an entity that was never planned is refused.
        let mut forged = report.clone();
        forged["skips"][0]["entityId"] = serde_json::json!("generated.storefront.999");
        assert!(
            verify_published_placement(&forged, &published, &BTreeSet::new(), None).is_err(),
            "a forged skip must be refused"
        );

        // Naming a descriptor that does not ask to be skipped is refused.
        let mut wrong_rule = report.clone();
        wrong_rule["skips"][0]["rule"] = serde_json::json!("landscape.object.toml");
        assert!(
            verify_published_placement(
                &wrong_rule,
                &published,
                &BTreeSet::new(),
                Some(&registry)
            )
            .is_err(),
            "a rule that does not declare fallback=skip must be refused"
        );

        // An authored landmark is never skippable, even when the plan and the
        // published scene agree that it is gone.
        let mut landmark_skip = report.clone();
        landmark_skip["inputEntityIds"] = serde_json::json!([
            "generated.green-space.0",
            "landmark.waterfront-river"
        ]);
        landmark_skip["outputEntityIds"] = serde_json::json!(["generated.green-space.0"]);
        landmark_skip["inputItems"] = serde_json::json!(2);
        landmark_skip["outputItems"] = serde_json::json!(1);
        landmark_skip["skips"][0]["entityId"] =
            serde_json::json!("landmark.waterfront-river");
        let mut landmark_published = BTreeSet::new();
        landmark_published.insert("generated.green-space.0".to_string());
        let mut landmarks = BTreeSet::new();
        landmarks.insert("landmark.waterfront-river".to_string());
        let message = verify_published_placement(&landmark_skip, &landmark_published, &landmarks, None)
            .expect_err("a skip naming an authored landmark must be refused");
        assert!(message.contains("authored landmark"), "{message}");

        // Infrastructure layout is never skippable either, even when its own
        // descriptor declares fallback=skip (road.object.toml does).
        let mut road_skip = landmark_skip.clone();
        road_skip["inputEntityIds"] = serde_json::json!([
            "generated.green-space.0",
            "generated.road.0"
        ]);
        road_skip["skips"][0]["entityId"] = serde_json::json!("generated.road.0");
        road_skip["skips"][0]["kind"] = serde_json::json!("road");
        road_skip["skips"][0]["rule"] = serde_json::json!("road.object.toml");
        let message = verify_published_placement(&road_skip, &landmark_published, &BTreeSet::new(), Some(&registry))
            .expect_err("a skip naming infrastructure must be refused");
        assert!(message.contains("infrastructure"), "{message}");

        // A report baked before removal bookkeeping existed stays readable: it
        // removed nothing, so there is nothing to recompute.
        let legacy = serde_json::json!({"sceneId": "legacy", "rounds": 1});
        let audit = verify_published_placement(&legacy, &published, &BTreeSet::new(), None)
            .expect("a legacy report without removals stays readable");
        assert_eq!(audit, PlacementReportAudit::default());
    }

    /// The pad triangle: the plan's pad list, the reconcile output and the published
    /// scene must agree. Round 23's defect fell exactly through the gap this closes -
    /// the pad stayed in the plan and in the ledger, and its entity was deleted from
    /// the world before reconciliation, so `inputItems == outputItems` and every
    /// other audit agreed with itself. The refusal list is honoured from the same
    /// report, in the other direction.
    #[test]
    fn a_planned_pad_must_be_reconciled_and_published_and_a_refusal_must_be_honoured() {
        let report = serde_json::json!({
            "sceneId": "pads",
            "rounds": 0,
            "moves": [],
            "inputItems": 2,
            "outputItems": 2,
            "removedItems": 0,
            "inputEntityIds": ["generated.pad.kept", "generated.tree.0"],
            "outputEntityIds": ["generated.pad.kept", "generated.tree.0"],
            "skips": [],
            "checkedItems": 2,
            "rejectedItems": 0,
            "plannedPadIds": ["generated.pad.kept"],
        });
        let published = |ids: &[&str]| -> BTreeSet<String> {
            ids.iter().map(|id| (*id).to_owned()).collect()
        };
        let both = published(&["generated.pad.kept", "generated.tree.0"]);
        let audit = verify_published_placement(&report, &both, &BTreeSet::new(), None)
            .expect("a consistent pad triangle must recompute");
        assert_eq!((audit.plan_items, audit.output_items), (2, 2));
        // 1. The defect itself: the plan claims a pad that never reached the
        //    reconcile output (it was deleted from the world earlier) and was never
        //    named as a skip. Nothing else in the report moves.
        let mut lost = report.clone();
        lost["plannedPadIds"] =
            serde_json::json!(["generated.pad.kept", "generated.pad.church"]);
        let error = verify_published_placement(&lost, &both, &BTreeSet::new(), None)
            .expect_err("a planned pad the world does not carry must be refused by name");
        assert_eq!(
            error,
            "pads: planned pad generated.pad.church is neither in the reconcile output nor named \
             as a skip; the plan and the ledger claim a pad the world does not carry"
        );
        // 2. Reconciled but not published: same defect one stage later.
        let mut unpublished = report.clone();
        unpublished["plannedPadIds"] = serde_json::json!(["generated.pad.kept"]);
        let error = verify_published_placement(
            &unpublished,
            &published(&["generated.tree.0"]),
            &BTreeSet::new(),
            None,
        )
        .expect_err("a planned pad that is not published must be refused by name");
        assert_eq!(
            error,
            "pads: planned pad generated.pad.kept is in the reconcile output but is not \
             published; the plan and the ledger claim a pad the world does not carry"
        );
        // 3. A named skip is the legitimate way for a planned pad to leave: the
        //    triangle then closes.
        let mut skipped = report.clone();
        skipped["outputItems"] = serde_json::json!(1);
        skipped["removedItems"] = serde_json::json!(1);
        skipped["outputEntityIds"] = serde_json::json!(["generated.tree.0"]);
        skipped["skips"] = serde_json::json!([{
            "entityId": "generated.pad.kept",
            "kind": "residential_home",
            "descriptorFallback": "skip",
            "reason": "geometry collision with Building (pads)",
            "rule": "residential_home.object.toml",
            "round": 1,
            "exit": "search-exhausted",
        }]);
        let audit = verify_published_placement(&skipped, &published(&["generated.tree.0"]), &BTreeSet::new(), None)
            .expect("a planned pad removed by name is accounted for");
        assert_eq!(audit.skipped_items, 1);
        // 4. A refusal the planner published must be honoured: the same report that
        //    is fine above fails the moment the refused id reappears in the plan.
        let mut refused = report.clone();
        refused["refusedEntityIds"] = serde_json::json!(["generated.pad.church"]);
        assert!(
            verify_published_placement(&refused, &both, &BTreeSet::new(), None).is_ok(),
            "a refusal that is absent everywhere is fine"
        );
        refused["refusedEntityIds"] = serde_json::json!(["generated.pad.kept"]);
        let error = verify_published_placement(&refused, &both, &BTreeSet::new(), None)
            .expect_err("a refused item still in the reconcile plan must be refused by name");
        assert_eq!(
            error,
            "pads: item generated.pad.kept is refused by the planner and is still in the \
             reconcile plan"
        );
    }
}
