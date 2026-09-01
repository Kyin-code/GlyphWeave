//! Load `.object.toml` descriptors and validate them against schema rules.

use std::collections::HashMap;
use std::path::Path;

use super::errors::RuleLoadError;
use super::schema::{Fallback, ItemKind, ObjectDescriptor, PlacementPhase};

/// Validate a parsed descriptor for schema-level sanity. Returns a list of
/// human-readable problems (empty = valid).
pub fn validate_descriptor(d: &ObjectDescriptor) -> Vec<String> {
    let mut problems = Vec::new();
    if d.id.is_empty() {
        problems.push("id must not be empty".into());
    }
    if d.geometry.footprint[0] <= 0.0 || d.geometry.footprint[1] <= 0.0 {
        problems.push("geometry.footprint must be > 0".into());
    }
    if d.geometry.height <= 0.0 {
        problems.push("geometry.height must be > 0".into());
    }
    if d.geometry.clearance < 0.0 {
        problems.push("geometry.clearance must be >= 0".into());
    }
    // Finite-value checks.
    let finite = |v: f32| v.is_finite();
    if !finite(d.geometry.footprint[0]) || !finite(d.geometry.footprint[1]) {
        problems.push("geometry.footprint must be finite".into());
    }
    if !finite(d.geometry.height) {
        problems.push("geometry.height must be finite".into());
    }
    if !finite(d.geometry.clearance) {
        problems.push("geometry.clearance must be finite".into());
    }
    if !finite(d.environment.max_slope) {
        problems.push("environment.max_slope must be finite".into());
    }
    // max_slope = 0 means "no slope limit" (any slope allowed), matching the
    // compiler which only emits MaxSlope when max_slope > 0.
    if d.environment.max_slope < 0.0 {
        problems.push("environment.max_slope must be >= 0".into());
    }
    // Relation distance / weight sanity.
    for r in &d.relations.avoid {
        if r.distance < 0.0 {
            problems.push(format!("relations.avoid {}: distance must be >= 0", r.kind as u8));
        }
        if !finite(r.distance) {
            problems.push(format!("relations.avoid {}: distance must be finite", r.kind as u8));
        }
    }
    for r in &d.relations.require {
        if r.distance < 0.0 {
            problems.push(format!("relations.require {}: distance must be >= 0", r.kind as u8));
        }
        if !finite(r.distance) {
            problems.push(format!("relations.require {}: distance must be finite", r.kind as u8));
        }
    }
    for r in &d.relations.prefer {
        if r.distance < 0.0 {
            problems.push(format!("relations.prefer {}: distance must be >= 0", r.kind as u8));
        }
        if !finite(r.distance) {
            problems.push(format!("relations.prefer {}: distance must be finite", r.kind as u8));
        }
        if !finite(r.weight) || !(0.0..=10.0).contains(&r.weight) {
            problems.push(format!("relations.prefer {}: weight must be finite and in [0,10]", r.kind as u8));
        }
    }
    // Anchor sanity: non-empty id, valid side, no duplicate ids, finite radius.
    let mut seen_anchors = std::collections::HashSet::new();
    for a in &d.anchors {
        if a.id.is_empty() {
            problems.push("anchor id must not be empty".into());
        }
        if !seen_anchors.insert(&a.id) {
            problems.push(format!("duplicate anchor id '{}'", a.id));
        }
        match a.side.as_str() {
            "front" | "back" | "left" | "right" | "north" | "south" | "east" | "west" => {}
            _ => problems.push(format!("anchor '{}': invalid side '{}'", a.id, a.side)),
        }
        if a.clear_radius < 0.0 {
            problems.push(format!("anchor {}: clear_radius must be >= 0", a.id));
        }
    }
    // Placement sanity.
    if d.placement.attempts == 0 {
        problems.push("placement.attempts must be >= 1".into());
    }
    // MVP only implements fallback = skip; move/shrink are accepted by the
    // schema but silently do nothing today, so reject them loudly.
    if d.placement.fallback != Fallback::Skip {
        problems.push(
            "placement.fallback: only 'skip' is implemented in the MVP; move/shrink are rejected to avoid silent no-ops"
                .into(),
        );
    }
    if d.placement.fallback == Fallback::Move && !d.placement.allow_rotate {
        problems.push("placement.fallback=move usually wants allow_rotate=true".into());
    }
    problems
}

/// Parse a descriptor from a TOML string and run schema validation.
pub fn parse_descriptor(toml_str: &str, source: &str) -> Result<ObjectDescriptor, RuleLoadError> {
    let d: ObjectDescriptor =
        toml::from_str(toml_str).map_err(|e| RuleLoadError::Toml {
            path: source.to_string(),
            source: e,
        })?;
    let problems = validate_descriptor(&d);
    if !problems.is_empty() {
        return Err(RuleLoadError::Schema {
            id: d.id.clone(),
            message: problems.join("; "),
        });
    }
    Ok(d)
}

/// Read + parse + validate one `.object.toml` file.
pub fn load_descriptor(path: &Path) -> Result<ObjectDescriptor, RuleLoadError> {
    let src = std::fs::read_to_string(path).map_err(|e| RuleLoadError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    parse_descriptor(&src, &path.display().to_string())
}

/// Load every `*.object.toml` in a directory. A missing directory or a
/// directory with no descriptors is an error: the rules engine must never
/// silently run with "no rules" (which would fake an all-clean report).
pub fn load_dir(dir: &Path) -> Result<HashMap<String, ObjectDescriptor>, RuleLoadError> {
    let mut out = HashMap::new();
    if !dir.exists() {
        return Err(RuleLoadError::Io {
            path: dir.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "rules directory does not exist",
            ),
        });
    }
    for entry in std::fs::read_dir(dir).map_err(|e| RuleLoadError::Io {
        path: dir.display().to_string(),
        source: e,
    })? {
        let entry = entry.map_err(|e| RuleLoadError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let p = entry.path();
        if p.extension().map(|e| e == "toml").unwrap_or(false) {
            let d = load_descriptor(&p)?;
            if out.contains_key(&d.id) {
                return Err(RuleLoadError::Schema {
                    id: d.id,
                    message: format!("duplicate id in {}", p.display()),
                });
            }
            out.insert(d.id.clone(), d);
        }
    }
    if out.is_empty() {
        return Err(RuleLoadError::Schema {
            id: "<dir>".into(),
            message: format!(
                "no *.object.toml descriptors found in {} (empty rules directory)",
                dir.display()
            ),
        });
    }
    Ok(out)
}

/// Registry: id → descriptor, with lookup helpers.
#[derive(Debug, Default)]
pub struct ObjectRegistry {
    pub descriptors: HashMap<String, ObjectDescriptor>,
}

impl ObjectRegistry {
    pub fn load_dir(dir: &Path) -> Result<Self, RuleLoadError> {
        Ok(Self {
            descriptors: load_dir(dir)?,
        })
    }

    pub fn get(&self, id: &str) -> Option<&ObjectDescriptor> {
        self.descriptors.get(id)
    }

    pub fn kinds(&self) -> Vec<ItemKind> {
        self.descriptors.values().map(|d| d.kind).collect()
    }

    pub fn by_phase(&self, phase: PlacementPhase) -> Vec<&ObjectDescriptor> {
        let mut v: Vec<&ObjectDescriptor> = self
            .descriptors
            .values()
            .filter(|d| d.placement.phase == phase)
            .collect();
        v.sort_by_key(|d| d.placement.priority);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_descriptor() {
        let toml = r#"
id = "rock_round"
kind = "rock"

[geometry]
footprint = [2.0, 2.0]
height = 1.2

[environment]
on_ground = true
max_slope = 40

[placement]
phase = "vegetation"
"#;
        let d = parse_descriptor(toml, "test").expect("valid");
        assert_eq!(d.id, "rock_round");
        assert!(validate_descriptor(&d).is_empty());
    }

    #[test]
    fn parse_invalid_footprint() {
        let toml = r#"
id = "bad"
kind = "tree"

[geometry]
footprint = [0.0, 2.0]
height = 1.0

[placement]
phase = "vegetation"
"#;
        match parse_descriptor(toml, "test") {
            Err(e) => {
                assert!(
                    matches!(&e, RuleLoadError::Schema { .. }),
                    "expected Schema, got {e:?}"
                );
            }
            Ok(_) => panic!("expected an error for zero footprint"),
        }
    }

    #[test]
    fn registry_load_missing_dir_fails() {
        let err = ObjectRegistry::load_dir(Path::new("/nonexistent")).unwrap_err();
        assert!(matches!(err, RuleLoadError::Io { .. }));
    }
}
