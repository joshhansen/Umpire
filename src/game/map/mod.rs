//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

pub mod dijkstra;
pub mod gen;
mod grid;
pub mod newmap;
mod terrain;
mod tile;

pub use self::terrain::Terrain;
pub use self::tile::Tile;
pub use self::grid::LocationGrid;
