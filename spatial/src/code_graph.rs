//! Code structure mapped to the Eisenstein hexagonal lattice.
//!
//! Functions are nodes placed on hex coordinates via a space-filling curve.
//! Calls become directed edges. Modules become Voronoï cells that partition
//! the lattice. Import coupling is measured as lattice distance.
//!
//! # Layout Strategy
//!
//! We use a **peano-like hexagonal spiral** to assign lattice positions:
//! functions in the same module cluster together (forming a Voronoï cell),
//! and cross-module calls become edges whose lattice distance reveals
//! coupling strength.
//!
//! Distance 1 → same module (adjacent hex, shared edge)
//! Distance 2 → sibling module (reachable via one hop)
//! Distance 3+ → increasingly distant coupling

use std::collections::{HashMap, HashSet};
use eisenstein::E12;

use crate::{FileId, FunctionId, LatticePos};

/// A function node on the Eisenstein lattice.
#[derive(Debug, Clone)]
pub struct FunctionNode {
    /// Unique identifier.
    pub id: FunctionId,
    /// Position on the Eisenstein lattice.
    pub pos: LatticePos,
    /// Functions this function calls (by FunctionId).
    pub calls: Vec<FunctionId>,
    /// Functions that call this function.
    pub called_by: Vec<FunctionId>,
    /// Estimated cognitive complexity (cyclomatic + nesting).
    pub complexity: u32,
    /// Lines of code.
    pub lines_of_code: u32,
}

/// A module (directory) containing one or more functions.
#[derive(Debug, Clone)]
pub struct ModuleCell {
    /// File path (e.g., "crates/spatial/src/lib.rs").
    pub file: FileId,
    /// Functions contained in this module.
    pub functions: Vec<FunctionId>,
    /// Voronoï cell center on the lattice.
    pub center: LatticePos,
    /// Modules that this module imports from.
    pub imports: Vec<FileId>,
    /// Modules that import from this module.
    pub imported_by: Vec<FileId>,
}

/// The full code graph mapped onto the Eisenstein lattice.
#[derive(Debug, Clone)]
pub struct CodeGraph {
    /// All function nodes, indexed by FunctionId.
    pub functions: HashMap<FunctionId, FunctionNode>,
    /// All module cells, indexed by FileId.
    pub modules: HashMap<FileId, ModuleCell>,
    /// Flat counter for sequential hex spiral allocation.
    alloc_counter: u64,
    /// Positions already assigned.
    assigned: HashSet<LatticePos>,
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGraph {
    /// Create a new empty code graph.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            modules: HashMap::new(),
            alloc_counter: 0,
            assigned: HashSet::new(),
        }
    }

    /// Register a file/module and allocate its Voronoï cell center.
    pub fn register_module(&mut self, file: FileId) {
        if self.modules.contains_key(&file) {
            return;
        }
        let center = self.alloc_next_pos();
        let cell = ModuleCell {
            file: file.clone(),
            functions: Vec::new(),
            center,
            imports: Vec::new(),
            imported_by: Vec::new(),
        };
        self.modules.insert(file, cell);
    }

    /// Register a function node at the given module's Voronoï cell.
    /// If no position is provided, allocates a nearby lattice point.
    pub fn register_function(
        &mut self,
        file: FileId,
        name: String,
        complexity: u32,
        lines_of_code: u32,
        pos: Option<LatticePos>,
    ) {
        // Ensure module exists
        if !self.modules.contains_key(&file) {
            self.register_module(file.clone());
        }

        let fn_id = FunctionId {
            file: file.clone(),
            name: name.clone(),
        };

        if self.functions.contains_key(&fn_id) {
            return; // already registered
        }

        // Place near the module center
        let pos = match pos {
            Some(p) if !self.assigned.contains(&p) => p,
            _ => {
                let center = self.modules[&file].center;
                // Find the nearest unoccupied hex within a disk around center
                self.nearest_free_near(center, 5)
            }
        };

        self.assigned.insert(pos);

        let node = FunctionNode {
            id: fn_id.clone(),
            pos,
            calls: Vec::new(),
            called_by: Vec::new(),
            complexity,
            lines_of_code,
        };

        self.functions.insert(fn_id.clone(), node);
        if let Some(cell) = self.modules.get_mut(&file) {
            cell.functions.push(fn_id);
        }
    }

    /// Add a call edge from `caller` to `callee`.
    pub fn add_call(&mut self, caller: &FunctionId, callee: &FunctionId) {
        if let Some(node) = self.functions.get_mut(caller) {
            if !node.calls.contains(callee) {
                node.calls.push(callee.clone());
            }
        }
        if let Some(node) = self.functions.get_mut(callee) {
            if !node.called_by.contains(caller) {
                node.called_by.push(caller.clone());
            }
        }
    }

    /// Register an import from `importer` to `imported`.
    pub fn add_import(&mut self, importer: &FileId, imported: &FileId) {
        // Both modules must exist
        if !self.modules.contains_key(importer) {
            self.register_module(importer.clone());
        }
        if !self.modules.contains_key(imported) {
            self.register_module(imported.clone());
        }

        if let Some(cell) = self.modules.get_mut(importer) {
            if !cell.imports.contains(imported) {
                cell.imports.push(imported.clone());
            }
        }
        if let Some(cell) = self.modules.get_mut(imported) {
            if !cell.imported_by.contains(importer) {
                cell.imported_by.push(importer.clone());
            }
        }
    }

    /// Lattice distance (hex distance) between two functions.
    pub fn function_distance(&self, a: &FunctionId, b: &FunctionId) -> Option<u32> {
        let node_a = self.functions.get(a)?;
        let node_b = self.functions.get(b)?;
        Some((node_a.pos - node_b.pos).hex_distance())
    }

    /// Lattice distance between two modules (distance between their centers).
    pub fn module_distance(&self, a: &FileId, b: &FileId) -> Option<u32> {
        let cell_a = self.modules.get(a)?;
        let cell_b = self.modules.get(b)?;
        Some((cell_a.center - cell_b.center).hex_distance())
    }

    /// Find the shortest call chain between two functions (BFS on call graph).
    pub fn call_distance(&self, start: &FunctionId, target: &FunctionId) -> Option<usize> {
        if start == target {
            return Some(0);
        }
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        visited.insert(start.clone());
        queue.push_back((start.clone(), 0usize));

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(node) = self.functions.get(&current) {
                for callee in &node.calls {
                    if callee == target {
                        return Some(depth + 1);
                    }
                    if visited.insert(callee.clone()) {
                        queue.push_back((callee.clone(), depth + 1));
                    }
                }
            }
        }
        None // unreachable
    }

    /// List all functions within a given lattice distance of a function.
    pub fn functions_within_distance(
        &self,
        fn_id: &FunctionId,
        max_distance: u32,
    ) -> Vec<&FunctionId> {
        let node = match self.functions.get(fn_id) {
            Some(n) => n,
            None => return Vec::new(),
        };
        let center = node.pos;
        self.functions
            .iter()
            .filter(|(id, n)| {
                **id != *fn_id && (n.pos - center).hex_distance() <= max_distance
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Get the module cell for a function.
    pub fn module_for_function(&self, fn_id: &FunctionId) -> Option<&ModuleCell> {
        self.modules.get(&fn_id.file)
    }

    /// Recompute module Voronoï cell centers based on contained functions.
    /// Each module center becomes the centroid (hex-rounded) of its function positions.
    pub fn recompute_centroids(&mut self) {
        // Group functions by module
        let mut by_module: HashMap<FileId, Vec<LatticePos>> = HashMap::new();
        for (id, node) in &self.functions {
            by_module.entry(id.file.clone()).or_default().push(node.pos);
        }

        for (file, positions) in by_module {
            if positions.is_empty() {
                continue;
            }
            if let Some(cell) = self.modules.get_mut(&file) {
                cell.center = centroid(positions.as_slice());
            }
        }
    }

    // ── Internal helpers ──

    fn alloc_next_pos(&mut self) -> LatticePos {
        let n = self.alloc_counter;
        self.alloc_counter += 1;
        index_to_hex_spiral(n)
    }

    fn nearest_free_near(&mut self, center: LatticePos, search_radius: u32) -> LatticePos {
        let disk = eisenstein::HexDisk::radius(search_radius);
        let mut best: Option<LatticePos> = None;
        let mut best_dist = u32::MAX;

        for p in disk.iter() {
            let candidate = center + p;
            if !self.assigned.contains(&candidate) {
                let d = candidate.hex_distance();
                if d < best_dist {
                    best_dist = d;
                    best = Some(candidate);
                }
            }
        }

        best.unwrap_or_else(|| {
            // Fallback: allocate fresh
            let fresh = self.alloc_next_pos();
            self.assigned.insert(fresh);
            fresh
        })
    }
}

/// Convert a flat index to a hex ring-spiral position.
/// Index 0 → (0,0), indices 1-6 → ring 1, 7-18 → ring 2, etc.

/// Convert a flat index to a hex ring-spiral position.
/// Index 0 → (0,0), indices 1-6 → ring 1, 7-18 → ring 2, etc.
fn index_to_hex_spiral(n: u64) -> LatticePos {
    if n == 0 {
        return E12::new(0, 0);
    }
    
    // Find ring k from n.
    // Ring 0: 1 point (indices: just 0)
    // Ring 1: 6 points (indices 1..=6)
    // Ring 2: 12 points (indices 7..=18)
    // Ring k: 6k points, cumulative before ring k = 3k(k-1)
    //
    // Solve: 3k(k-1) < n <= 3k(k+1)
    // k ≈ sqrt(n/3)
    // Work in f64 to avoid overflow, then clamp
    
    let nf = n as f64;
    let mut k_f64 = (nf / 3.0).sqrt();
    if k_f64 < 1.0 {
        k_f64 = 1.0;
    }
    
    // Check nearby integer rings (f64 is precise enough for u64 ranges we use)
    let mut k: u64 = k_f64.ceil() as u64;
    
    // Adjust k if needed (walk up to find the correct ring)
    // Use saturating math to avoid overflow for giant n
    for _ in 0..5 {
        let before = k.saturating_mul(3).saturating_mul(k.saturating_sub(1));
        let after = k.saturating_mul(3).saturating_mul(k.saturating_add(1));
        if n > before && n <= after {
            break;
        }
        k += 1;
    }
    
    // Final fallback: just compute using the derived k
    let before = k.saturating_mul(3).saturating_mul(k.saturating_sub(1));
    let offset = if n <= before { 0 } else { n - before - 1 };
    let k_sz = k as i32;
    
    let side = offset / k;
    let pos_on_side = offset % k;
    
    // The ring starts at (k, 0) and walks around
    let start = E12::new(k_sz, 0);
    
    // The 6 sides of the ring (axial hex coordinates)
    let side_dirs = [
        E12::new(-1, 1),  // side 0: (k,0) → (0,k)
        E12::new(-1, 0),  // side 1: (0,k) → (-k,k)
        E12::new(0, -1),  // side 2: (-k,k) → (-k,0)
        E12::new(1, -1),  // side 3: (-k,0) → (0,-k)
        E12::new(1, 0),   // side 4: (0,-k) → (k,-k)
        E12::new(0, 1),   // side 5: (k,-k) → (k,0)
    ];
    
    let mut pos = start;
    for s in 0..(side.min(5) as usize) {
        pos = pos + side_dirs[s].scale(k_sz);
    }
    let s = (side % 6) as usize;
    pos = pos + side_dirs[s].scale(pos_on_side as i32);
    
    pos
}

/// Compute the hex-rounded centroid of a set of lattice positions.
fn centroid(positions: &[LatticePos]) -> LatticePos {
    if positions.is_empty() {
        return E12::new(0, 0);
    }
    let sum_a: f64 = positions.iter().map(|p| p.a() as f64).sum();
    let sum_b: f64 = positions.iter().map(|p| p.b() as f64).sum();
    let n = positions.len() as f64;
    let avg_a = sum_a / n;
    let avg_b = sum_b / n;

    // Snap to nearest Eisenstein integer using snapkit's Voronoï method
    let snapped = snapkit::eisenstein_round_voronoi(avg_a, avg_b);
    E12::new(snapped.a as i32, snapped.b as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_function_and_call() {
        let mut graph = CodeGraph::new();

        let file_a = FileId("src/main.rs".into());
        let file_b = FileId("src/helper.rs".into());

        graph.register_function(file_a.clone(), "main".into(), 5, 20, None);
        graph.register_function(file_b.clone(), "helper".into(), 3, 15, None);

        let main_id = FunctionId {
            file: file_a.clone(),
            name: "main".into(),
        };
        let helper_id = FunctionId {
            file: file_b.clone(),
            name: "helper".into(),
        };

        graph.add_call(&main_id, &helper_id);

        assert!(graph.functions.contains_key(&main_id));
        assert!(graph.functions.contains_key(&helper_id));
        assert_eq!(graph.functions[&main_id].calls.len(), 1);
        assert_eq!(graph.functions[&helper_id].called_by.len(), 1);
    }

    #[test]
    fn test_function_distance() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/lib.rs".into());

        graph.register_function(
            file.clone(),
            "foo".into(),
            1,
            5,
            Some(E12::new(0, 0)),
        );
        graph.register_function(
            file.clone(),
            "bar".into(),
            1,
            5,
            Some(E12::new(1, 0)),
        );

        let foo = FunctionId {
            file: file.clone(),
            name: "foo".into(),
        };
        let bar = FunctionId {
            file,
            name: "bar".into(),
        };

        let dist = graph.function_distance(&foo, &bar);
        assert_eq!(dist, Some(1)); // adjacent hexes
    }

    #[test]
    fn test_module_distance() {
        let mut graph = CodeGraph::new();
        let a = FileId("crates/a/src/lib.rs".into());
        let b = FileId("crates/b/src/lib.rs".into());

        graph.register_module(a.clone());
        graph.register_module(b.clone());

        let dist = graph.module_distance(&a, &b);
        assert!(dist.is_some());
    }

    #[test]
    fn test_call_distance_bfs() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/lib.rs".into());

        let ids: Vec<_> = (0..5)
            .map(|i| {
                let name = format!("fn_{}", i);
                graph.register_function(file.clone(), name.clone(), 1, 5, None);
                FunctionId {
                    file: file.clone(),
                    name,
                }
            })
            .collect();

        // Chain: fn_0 → fn_1 → fn_2 → fn_3
        for i in 0..3 {
            graph.add_call(&ids[i], &ids[i + 1]);
        }

        assert_eq!(graph.call_distance(&ids[0], &ids[3]), Some(3));
        assert_eq!(graph.call_distance(&ids[0], &ids[4]), None); // unreachable
    }

    #[test]
    fn test_lattice_distance_clustering() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/mod.rs".into());

        // Place functions at known positions
        graph.register_function(
            file.clone(),
            "near_a".into(),
            1,
            10,
            Some(E12::new(0, 0)),
        );
        graph.register_function(
            file.clone(),
            "near_b".into(),
            1,
            10,
            Some(E12::new(1, 0)),
        );
        graph.register_function(
            file.clone(),
            "far".into(),
            1,
            10,
            Some(E12::new(10, 0)),
        );

        let near_a = FunctionId {
            file: file.clone(),
            name: "near_a".into(),
        };
        let near_b = FunctionId {
            file: file.clone(),
            name: "near_b".into(),
        };
        let far = FunctionId {
            file,
            name: "far".into(),
        };

        let close = graph.functions_within_distance(&near_a, 1);
        assert!(close.contains(&&near_b));
        assert!(!close.contains(&&far));
    }

    #[test]
    fn test_hexagonal_spiral_no_collisions() {
        let mut graph = CodeGraph::new();
        let file = FileId("src/lib.rs".into());
        let n = 100;
        for i in 0..n {
            graph.register_function(file.clone(), format!("fn_{}", i), 1, 5, None);
        }
        let ids: Vec<_> = graph.functions.keys().cloned().collect();
        // Check all functions have unique positions
        let mut positions: HashSet<LatticePos> = HashSet::new();
        for id in &ids {
            let pos = graph.functions[id].pos;
            assert!(
                positions.insert(pos),
                "Duplicate position for {:?}: {:?}",
                id, pos
            );
        }
        assert_eq!(positions.len(), ids.len());
    }
}
