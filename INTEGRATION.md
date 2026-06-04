# Integration Guide: Ternary Modules in Hermit Zed

> Where ternary intelligence would plug into Zed's editor architecture.

## Overview

Hermit Zed (fork of Zed editor) currently integrates a **spectral analysis** crate for codebase topology visualization. Ternary modules would extend this by adding ternary decision logic to Zed's agent system, completion pipeline, and extension runtime.

## Current Integration: Spectral Analysis

| Module | Location | Role |
|--------|----------|------|
| `CathedralProbe` | `crates/spectral_analysis/src/spectral_analysis.rs` | Spectral graph computations (Fiedler vector, Cheeger constant, community detection) |
| `spectral_analysis_lsp` | `crates/spectral_analysis/src/bin/spectral_analysis_lsp.rs` | LSP binary exposing spectral analysis to the editor |
| Zed extension | `extensions/spectral_analysis/` | Extension integration layer |

## Where Ternary Modules Would Plug In

### 1. Ternary Completion Filter → `crates/agent/`

Zed's agent subsystem handles AI-assisted editing. Ternary filtering would sit between the agent's response and the editor:

```rust
// Hypothetical: in crates/agent/src/ternary_filter.rs
// Filters AI completion suggestions using ternary signals

pub struct TernaryCompletionFilter {
    /// Track which suggestion patterns the user accepts (+1),
    /// ignores (0), or explicitly rejects (-1)
    history: VecDeque<(CompletionContext, Trit)>,
}

impl TernaryCompletionFilter {
    /// Score a completion suggestion based on historical ternary outcomes
    pub fn score(&self, suggestion: &CompletionSuggestion) -> f64 {
        // Match against similar contexts in history
        // Return weighted score: Choose (+1) boosts, Avoid (-1) suppresses
    }
}
```

**Where it connects:** The agent panel (`crates/agent/`) — between the agent's response generator and the editor's inline completion renderer.

### 2. Ternary Workflow Detector → `crates/project/`

The project model already tracks file relationships. Ternary workflow detection would classify editing sessions:

```rust
// Hypothetical: in crates/project/src/ternary_workflow.rs
// Detects what phase of work the user is in using ternary signals

pub enum WorkflowPhase {
    Exploring,    // mostly Unknown (0) — reading, navigating
    Constructing, // mostly Choose (+1) — writing new code
    Refactoring,  // balanced Choose/Avoid — restructuring
    Debugging,    // mostly Avoid (-1) — deleting/reverting
}
```

**Where it connects:** `crates/project/` — the project model already tracks buffers, diagnostics, and git status. Workflow detection would layer ternary signals on top.

### 3. Ternary Extension API → `extensions/`

The spectral analysis extension demonstrates the pattern. Ternary extensions would follow the same model:

```rust
// In extensions/ternary_intelligence/ — following spectral_analysis pattern
// Zed extension that exposes ternary metrics to the editor UI

// Extension manifest (extension.toml):
// [id] name = "ternary-intelligence"
// [dependencies] "cathedral_probe" = "*"

// The extension would:
// 1. Subscribe to buffer change events
// 2. Record ternary signals (added/deleted/unchanged)
// 3. Display workflow phase in the status bar
// 4. Expose conservation metrics via custom commands
```

**Where it connects:** Zed's extension system (`extensions/`) — the same architecture as `spectral_analysis`.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Zed Editor UI                                           │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────┐  │
│  │ Agent Panel  │  │ Status Bar    │  │ Editor View  │  │
│  └──────┬───────┘  └───────┬───────┘  └──────┬───────┘  │
│         │                  │                  │           │
│  ┌──────▼──────────────────▼──────────────────▼───────┐  │
│  │ crates/agent/                                       │  │
│  │  ┌─────────────────────────┐  ┌──────────────────┐ │  │
│  │  │ TernaryCompletionFilter │  │ TernaryWorkflow   │ │  │
│  │  │ (score suggestions)     │  │ Detector          │ │  │
│  │  └─────────────────────────┘  └──────────────────┘ │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ crates/spectral_analysis/ (existing)               │  │
│  │  CathedralProbe → Fiedler, Cheeger, Communities   │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ extensions/                                        │  │
│  │  spectral_analysis/ (existing)                     │  │
│  │  ternary_intelligence/ (future)                    │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Committed Files

- `a749cc18` — `crates/spectral_analysis/` — spectral analysis crate + LSP binary
- `a749cc18` — `extensions/spectral_analysis/` — Zed extension integration
- `25793d2d` — README rewrite for spectral analysis section

## Implementation Path

1. **Phase 1:** Add `crates/ternary_intelligence/` as a new workspace crate (like `spectral_analysis`)
2. **Phase 2:** Create `extensions/ternary_intelligence/` following the spectral analysis extension pattern
3. **Phase 3:** Hook into `crates/agent/` for completion filtering
4. **Phase 4:** Hook into `crates/project/` for workflow detection
5. **Phase 5:** Add `Cargo.toml` workspace member (single-line change, as with spectral_analysis)
