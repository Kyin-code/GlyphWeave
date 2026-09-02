//! Rules engine (MVP): declarative per-object placement rules.
//!
//! Design doc: docs/rules-engine-mvp.zh-CN.md
//! Principle: objects are described in TOML (`.object.toml`); a unified
//! validator checks candidates against environment + relation constraints;
//! placement commits only valid positions (never force-places).
//!
//! Module map:
//! - `schema`     — strongly-typed descriptor types
//! - `loader`     — TOML parsing + schema validation + registry
//! - `constraint` — descriptor → constraint list compiler
//! - `validator`  — hard constraint checks + soft scoring
//! - `placement`  — unified placement pipeline
//! - `errors`     — load errors + RejectReason + reports

pub mod audit;
pub mod constraint;
pub mod errors;
pub mod loader;
pub mod placement;
pub mod schema;
pub mod validator;

pub use audit::{audit_entities, entity_to_placed, kind_from_str};
pub use errors::{PlacementOutcome, RejectReason, RejectRecord, RuleLoadError, ValidationReport};
pub use loader::{ObjectRegistry, load_descriptor, load_dir};
pub use placement::{PlacementIndex, PlacementRequest, PlacementSource, place_all, place_one};
pub use schema::{Biome, HazardKind, ItemKind, ObjectDescriptor};
pub use validator::{Footprint, PlacedKind, PlacementContext, check_hard, score_soft};
