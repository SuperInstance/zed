//! Pythagorean snap cursor jumps and Eisenstein-distance related-code finding.
//!
//! # Pythagorean Snap
//!
//! "Pythagorean jump" means moving the cursor to positions whose Cartesian
//! distance from the current position equals a Pythagorean triple value.
//! On an Eisenstein lattice, this becomes a jump to hexes at a specific
//! Eisenstein triple distance — positions where a² - ab + b² = c².
//!
//! # Related-Code Finding
//!
//! Using Eisenstein lattice distance, we find "nearby" code in the spatial
//! code graph: functions within distance 1 are in the same module;
//! distance 2+ are increasingly distant relationships.
//!
//! # Lattice-Aware Multi-Cursor
//!
//! Multi-cursor placements snap to hex lattice positions, creating a
//! spatially distributed editing surface. Cursors at adjacent hexes
//! suggest "edit these simultaneously" intent.

use std::collections::HashSet;
use eisenstein::{E12, EisensteinTriple};
use snapkit::voronoi::eisenstein_round_voronoi;
use crate::code_graph::CodeGraph;
use crate::FunctionId;

/// A cursor position in Cartesian coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPos {
    pub x: f64,
    pub y: f64,
}

impl CursorPos {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Convert to an Eisenstein lattice position.
    pub fn to_lattice(self) -> E12 {
        let snapped = eisenstein_round_voronoi(self.x, self.y);
        E12::new(snapped.a as i32, snapped.b as i32)
    }

    /// Convert from an Eisenstein lattice position.
    pub fn from_lattice(pos: E12) -> Self {
        // Convert E12 → Cartesian: x = a - b/2, y = b * √3/2
        let x = pos.a() as f64 - pos.b() as f64 * 0.5;
        let y = pos.b() as f64 * 0.8660254037844386; // √3/2
        Self { x, y }
    }

    /// Euclidean distance between two cursor positions.
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// A Pythagorean snap jump — moving cursor to positions at specific
/// Eisenstein-triple distance from an origin.
#[derive(Debug, Clone)]
pub struct PythagoreanJump {
    /// Origin position.
    pub origin: CursorPos,
    /// Target positions for the jump.
    pub targets: Vec<CursorPos>,
    /// The Eisenstein triple distance used.
    pub triple: (i32, i32, u32),
}

/// Find all Eisenstein lattice positions at exactly `c` hex-distance
/// (Eisenstein norm distance = c²) from the origin hex.
pub fn pythagorean_jumps_from(origin: E12, c: u32) -> Vec<E12> {
    // Find all Eisenstein triples with this c value
    let all_triples = EisensteinTriple::all_with_max_norm(c);
    let mut targets = Vec::new();

    for triple in all_triples {
        if triple.c() != c {
            continue;
        }
        let delta = E12::new(triple.a(), triple.b());
        // Add all D6 rotations and reflections
        for rot in delta.d6_rotations() {
            let target = origin + rot;
            targets.push(target);
        }
        // Also the negative (flip)
        let neg = E12::new(-triple.a(), -triple.b());
        for rot in neg.d6_rotations() {
            let target = origin + rot;
            targets.push(target);
        }
    }

    // Deduplicate
    let mut seen = HashSet::new();
    targets.retain(|t| seen.insert(*t));
    targets.sort_by_key(|t| (t.a(), t.b()));
    targets
}

/// Find related functions using Eisenstein lattice distance.
///
/// Returns function IDs whose lattice distance from `fn_id` is within
/// `max_distance`, ordered by increasing distance.
pub fn related_functions<'a>(
    graph: &'a CodeGraph,
    fn_id: &'a FunctionId,
    max_distance: u32,
    max_results: usize,
) -> Vec<(&'a FunctionId, u32, bool)> {
    let node = match graph.functions.get(fn_id) {
        Some(n) => n,
        None => return Vec::new(),
    };
    let origin = node.pos;

    let mut results: Vec<(&FunctionId, u32, bool)> = graph
        .functions
        .iter()
        .filter(|(id, n)| {
            **id != *fn_id && (n.pos - origin).hex_distance() <= max_distance
        })
        .map(|(id, n)| {
            let dist = (n.pos - origin).hex_distance();
            let same_module = id.file == fn_id.file;
            (id, dist, same_module)
        })
        .collect();

    // Sort by distance, then same-module first, then name
    results.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| b.2.cmp(&a.2)) // same-module first
            .then_with(|| a.0.name.cmp(&b.0.name))
    });

    results.truncate(max_results);
    results
}

/// Generate multi-cursor positions on the hex lattice.
///
/// Given an origin cursor, generates positions at adjacent hexes (distance 1)
/// and at triple-distance positions (distance = triple_dist).
/// Returns a list of candidate cursor positions.
pub fn lattice_aware_multi_cursor(
    origin: CursorPos,
    triple_dist: u32,
    include_adjacent: bool,
) -> Vec<CursorPos> {
    let origin_lattice = origin.to_lattice();
    let mut positions = Vec::new();

    if include_adjacent {
        // Adjacent hexes (the 6 unit directions)
        for dir in E12::directions() {
            let neighbor = origin_lattice + dir;
            positions.push(CursorPos::from_lattice(neighbor));
        }
    }

    // Pythagorean jump targets
    let jumps = pythagorean_jumps_from(origin_lattice, triple_dist);
    for target in jumps {
        positions.push(CursorPos::from_lattice(target));
    }

    // Deduplicate by rounding to 3 decimal places
    let mut seen = HashSet::new();
    // We use a tolerance-based dedup by formatting
    positions.retain(|p| {
        let key = format!("{:.3},{:.3}", p.x, p.y);
        seen.insert(key)
    });

    positions
}

/// Find all functions in the same Voronoï cell (same module) as the cursor.
pub fn functions_at_lattice_position(
    graph: &CodeGraph,
    pos: CursorPos,
) -> Vec<&FunctionId> {
    let lattice = pos.to_lattice();
    graph
        .functions
        .iter()
        .filter(|(_, node)| node.pos == lattice)
        .map(|(id, _)| id)
        .collect()
}

/// Find the nearest function to a cursor position in the code graph.
pub fn nearest_function<'a>(
    graph: &'a CodeGraph,
    pos: CursorPos,
) -> Option<(&'a FunctionId, u32)> {
    let lattice = pos.to_lattice();
    graph
        .functions
        .iter()
        .map(|(id, node)| {
            let dist = (node.pos - lattice).hex_distance();
            (id, dist)
        })
        .min_by_key(|(_, dist)| *dist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_graph::CodeGraph;
    use crate::FileId;

    #[test]
    fn test_cursor_to_lattice_roundtrip() {
        let pos = CursorPos::new(1.0, 0.0);
        let lattice = pos.to_lattice();
        let back = CursorPos::from_lattice(lattice);
        assert!(
            pos.distance(back) < 0.1,
            "Roundtrip distance too large: {}",
            pos.distance(back)
        );
    }

    #[test]
    fn test_pythagorean_jumps_basic() {
        let origin = E12::new(0, 0);
        let jumps = pythagorean_jumps_from(origin, 1);
        assert!(!jumps.is_empty(), "Should find jumps for c=1");
        // Some jumps should be at hex distance 1 (the 6 units)
        assert!(jumps.iter().any(|p| p.hex_distance() == 1));
    }

    #[test]
    fn test_related_functions() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/lib.rs".into());

        let fn_ids: Vec<_> = (0..5)
            .map(|i| {
                let name = format!("fn_{}", i);
                graph.register_function(
                    file.clone(),
                    name.clone(),
                    1,
                    5,
                    Some(E12::new(i as i32, 0)),
                );
                FunctionId {
                    file: file.clone(),
                    name,
                }
            })
            .collect();

        let related = related_functions(&graph, &fn_ids[0], 3, 10);
        assert!(related.len() >= 2);
        // fn_1 should be at distance 1
        assert!(related.iter().any(|(id, d, _)| id.name == "fn_1" && *d == 1));
    }

    #[test]
    fn test_lattice_multi_cursor() {
        let origin = CursorPos::new(0.0, 0.0);
        let cursors = lattice_aware_multi_cursor(origin, 7, true);
        assert!(cursors.len() >= 6, "Should have at least 6 adjacent cursors");
    }

    #[test]
    fn test_nearest_function() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/main.rs".into());
        graph.register_function(file.clone(), "main".into(), 5, 20, Some(E12::new(0, 0)));

        let cursor = CursorPos::new(0.5, 0.0);
        let nearest = nearest_function(&graph, cursor);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().0.name, "main");
    }
}
