//! # spatial — Eisenstein spatial intelligence for the zed editor
//!
//! Code structure → hexagonal lattice. Functions become nodes, calls become
//! edges, modules become Voronoï cells. Import coupling is lattice distance.
//!
//! ## Why Eisenstein?
//!
//! Code relationships aren't linear. They're hexagonal — every function has
//! up to 6 "close" neighbors (same module), then layers of increasing distance
//! (cross-module, cross-crate, transitive). The Eisenstein lattice maps this
//! naturally: adjacent hexes share an edge (strong coupling); diagonals share
//! only a vertex (weak coupling).
//!
//! ## Modules
//!
//! - [`code_graph`] — Functions as nodes on the Eisenstein lattice, calls as
//!   edges, modules as Voronoï cells. Import coupling = lattice distance.
//! - [`snap_navigation`] — Pythagorean snap cursor jumps, Eisenstein distance
//!   for related-code finding, lattice-aware multi-cursor.
//! - [`spatial_analysis`] — File dependency → Fiedler vector, bridge file
//!   detection, reorganization suggestions.
//! - [`report`] — Show-don't-sell diagnostic output.

pub mod code_graph;
pub mod snap_navigation;
pub mod spatial_analysis;
pub mod report;

/// Re-export of the core Eisenstein integer type for convenience.
pub use eisenstein::E12;

/// Re-export of the Voronoï snap for Cartesian → lattice conversion.
pub use snapkit::eisenstein_round_voronoi;

/// A stable identifier for a source file or module in the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(pub String);

/// A stable identifier for a function within a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionId {
    pub file: FileId,
    pub name: String,
}

/// A position on the Eisenstein hexagonal lattice.
pub type LatticePos = eisenstein::E12;

/// Convert from lattice position to a human-readable hex coordinate string.
pub fn fmt_pos(pos: LatticePos) -> String {
    format!("({}, {})", pos.a(), pos.b())
}
