# Migration Analysis: hermit-zed

**Date:** 2026-06-04
**Fork of:** zed-industries/zed (Zed Editor)
**Scope:** SuperInstance integration — spectral analysis crate + extension

---

## Current State

| Metric | Value |
|--------|-------|
| Behind upstream | 95 commits |
| Our commits | 2 commits |
| Files changed (ours) | 10 files, +1,462 lines |
| Shared file conflicts | **2 files** (Cargo.toml, README.md) |

## Our Changes

Just 2 commits adding a self-contained spectral analysis system:

1. `08a2ce4` — feat: add spectral analysis crate + extension
2. `1f50609` — Rewrite spectral-analysis README section: show, don't sell

All 8 spectral_analysis files are **new additions** (don't exist upstream):
- `crates/spectral_analysis/` — core analysis library + LSP binary
- `extensions/spectral_analysis/` — Zed extension integration

Only 2 files modify existing upstream code:
- `Cargo.toml` — added workspace member
- `README.md` — added spectral analysis section

## Upstream Changes

95 commits of Zed editor development, touching hundreds of files across:
- Agent/ACP subsystem (agent settings, thread, tools, permissions)
- UI (agent panel, conversation view, entry view)
- Editor core, keymaps, settings
- CI/CD workflows

## Conflict Risk: 🟢 LOW

**Only 2 shared files** with potential conflicts:

1. **Cargo.toml** — we added a workspace member; upstream likely added/updated many crates. Low-risk conflict (additive).
2. **README.md** — we added a section; upstream may have updated theirs. Easy manual merge.

All our substantive code is in new directories that don't exist upstream — zero conflict potential.

## Recommended Strategy: Fresh Rebase

This is the ideal rebase scenario — few commits, minimal overlap.

### Steps

1. `git fetch upstream`
2. `git rebase upstream/main` — expect ~2 minor conflicts (Cargo.toml, README.md)
3. Resolve: add our workspace member to new Cargo.toml, merge README sections
4. Force-push rebased branch

### Alternative: Fresh Clone + Manual Copy
If Cargo.toml has changed dramatically (workspace structure refactored):
1. Fresh clone of upstream
2. Copy `crates/spectral_analysis/` and `extensions/spectral_analysis/` into place
3. Add workspace member to new Cargo.toml
4. Merge README section

## Estimated Effort

| Phase | Time |
|-------|------|
| Rebase + conflict resolution | 15–30 min |
| Testing (cargo build + extension) | 30 min |
| **Total** | **45 min – 1 hour** |

## Files to Watch

- `Cargo.toml` — workspace member list may have restructuring
- `README.md` — section ordering
- `Cargo.lock` — auto-generated, commit after build
