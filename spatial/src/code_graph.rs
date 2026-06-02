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
        // Grow the search radius until we find a free position
        for r in search_radius..=search_radius + 20 {
            let disk = eisenstein::HexDisk::radius(r);
            let mut best: Option<LatticePos> = None;
            let mut best_dist = u32::MAX;

            for p in disk.iter() {
                let candidate = center + p;
                if !self.assigned.contains(&candidate) {
                    let d = self.assigned.len() as u32; // tie-break by insertion order
                    if d < best_dist {
                        best_dist = d;
                        best = Some(candidate);
                    }
                }
            }

            if let Some(found) = best {
                return found;
            }
        }

        // Absolute fallback: follow the clockwise spiral from center
        let mut candidate = center;
        for _ in 0..1000 {
            if !self.assigned.contains(&candidate) {
                return candidate;
            }
            // Move one step outward in hex spiral
            let n = self.alloc_counter;
            self.alloc_counter += 1;
            candidate = center + index_to_hex_spiral(n);
        }
        unreachable!("Exhausted search for free hex position");
    }
}

/// Convert a flat index to a hex ring-spiral position.
/// Convert a flat index to a hex ring-spiral position.
/// Index 0 → (0,0), indices 1-6 → ring 1, 7-18 → ring 2, etc.
fn index_to_hex_spiral(n: u64) -> LatticePos {
    if n == 0 {
        return E12::new(0, 0);
    }
    
    // Find ring k: 3k(k-1) < n <= 3k(k+1)
    // k = ceil( (sqrt(1 + 4n/3) - 1) / 2 )
    let sqrt_term = (1.0 + (4.0 * n as f64) / 3.0).sqrt();
    let k = ((sqrt_term - 1.0) / 2.0).ceil() as u64;

    let before = 3 * k * (k - 1);
    let offset = n - before - 1;
    let side = offset / k;
    let pos_on_side = offset % k;
    let k_sz = k as i32;
    let p = pos_on_side as i32;
    
    // Ring starts at (k, 0) and walks 6 sides, each with k points.
    match side {
        0 => E12::new(k_sz - p, p),            // side 0: (k,0) -> (0,k)
        1 => E12::new(-p, k_sz),                // side 1: (0,k) -> (-k,k)
        2 => E12::new(-k_sz, k_sz - p),        // side 2: (-k,k) -> (-k,0)
        3 => E12::new(-k_sz + p, -p),           // side 3: (-k,0) -> (0,-k)
        4 => E12::new(p, -k_sz),                // side 4: (0,-k) -> (k,-k)
        5 => E12::new(k_sz, -k_sz + p),        // side 5: (k,-k) -> (k,0)
        _ => unreachable!(),
    }
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
