//! GlyphWeave core: pure logic, no Bevy dependency.
pub mod baked_terrain;
pub mod chunk;
pub mod coords;
pub mod edit;
pub mod engineering;
pub mod engineering_context;
pub mod engineering_drainage;
pub mod engineering_preview;
pub mod engineering_works;
pub mod error;
pub mod gameplay;
#[cfg(test)]
mod gemap;
pub mod height_payload;
pub mod intent;
pub mod layer;
pub mod migration;
pub mod natural;
pub mod preview_stream;
pub mod preview_stream_audit;
pub mod rules;
pub mod settlement;
pub mod storage;
pub mod tile;
pub mod voxel;
pub mod world;
pub mod worldgen;
