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
    if d.environment.max_slope < 0.0 {
        problems.push("environment.max_slope must be >= 0".into());
    }
    for r in &d.relations.avoid {
        if r.distance < 0.0 {
            problems.push(format!("relations.avoid {}: distance must be >= 0", r.kind as u8));
        }
    }
    for a in &d.anchors {
        if a.id.is_empty() {
            problems.push("anchor id must not be empty".into());
        }
        if a.clear_radius < 0.0 {
            problems.push(format!("anchor {}: clear_radius must be >= 0", a.id));
        }
    }
    // Placement sanity.
    if d.placement.attempts == 0 {
        problems.push("placement.attempts must be >= 1".into());
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

/// Load every `*.object.toml` in a directory.
pub fn load_dir(dir: &Path) -> Result<HashMap<String, ObjectDescriptor>, RuleLoadError> {
    let mut out = HashMap::new();
    if !dir.exists() {
        return Ok(out);
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
    fn registry_load_dir_empty() {
        let reg = ObjectRegistry::load_dir(Path::new("/nonexistent")).expect("ok");
        assert!(reg.descriptors.is_empty());
    }
}
