# Flash — Log Viewer

High-performance read-only log viewer in Rust. Handles files up to 2 GB via memory-mapped I/O.

## Tech Stack
- **UI:** `iced 0.13` — native Rust GUI, Catppuccin Mocha theme (+ Nord, Tokyo Night, Gruvbox, Latte)
- **Core:** `memmap2` file mapping, custom byte-offset line indexer, background regex search via `crossbeam-channel`, transformation pipeline worker
- **File dialogs:** `rfd 0.15` · **Async:** `tokio` · **File watching:** `notify`

## Workspace
```
lumen/
├── Cargo.toml
├── crates/
│   ├── flash-core/src/
│   │   ├── lib.rs                # Re-exports public API (including pipeline types)
│   │   ├── file_map.rs           # memmap2 wrapper
│   │   ├── line_index.rs         # Byte-offset line indexer (has unit tests)
│   │   ├── line_reader.rs        # Line text access via LineIndex
│   │   ├── log_level.rs          # TRACE/DEBUG/INFO/WARN/ERROR detection (has unit tests)
│   │   ├── search.rs             # Background regex worker thread
│   │   ├── pipeline.rs           # LayerKind, PipelineLayer, PipelineConfig, PipelineResponse types
│   │   └── pipeline_worker.rs    # PipelineHandle, spawn_pipeline_worker
│   └── flash-app/src/
│       ├── main.rs               # Entry point, window config, #[windows_subsystem], mod pipeline
│       ├── app.rs                # App state, Tab struct, Message enum, update/view/subscription;
│       │                         #   ViewRow enum, new Tab fields, rebuild_view_rows_from_filter,
│       │                         #   expand_with_context, trigger_pipeline, 15 new Messages
│       ├── pipeline.rs           # UiLayer, TransformPipeline (UI-side state, apply_text_transforms)
│       ├── theme.rs              # Palette struct + 5 theme palettes; context_row_bg: Color added
│       ├── views/
│       │   ├── search_bar.rs     # Top toolbar: Open, ≡ menu, file info, regex search, Tail
│       │   ├── filter_bar.rs     # Wrap toggle, quick filter input, Pipeline toggle button
│       │   │                     #   (level chips hidden from UI; handler code retained)
│       │   ├── log_view.rs       # Virtual-scroll log area, gutter, minimap, status bar;
│       │   │                     #   ViewRow rendering, context rows, stale ghost alpha,
│       │   │                     #   context/source buttons, apply_text_transforms per line
│       │   ├── pipeline_panel.rs # Left sidebar (260px): pipeline layer list and controls
│       │   ├── results_panel.rs  # Clickable regex-match results at bottom
│       │   ├── tab_bar.rs        # Multi-tab strip (shown when >1 tab open)
│       │   ├── command_palette.rs# Ctrl+P overlay with fuzzy search over all commands
│       │   ├── jump_to_line.rs   # Ctrl+G modal
│       │   └── settings_panel.rs # Theme / font-size / wrap settings overlay
│       └── widgets/
│           └── virtual_list.rs   # Viewport line-count helpers
└── test_fixtures/                # (empty)
```

## Features (all implemented)
- Virtual scrolling — only visible lines rendered (fast on 2 GB files)
- Multi-tab file opening, drag-and-drop, recent files
- Regex search with background worker; results panel with click-to-jump
- Log-level syntax highlighting (per-level filter chips hidden from UI; code kept for handler compatibility)
- Line-wrap toggle (button in filter bar + command palette)
- Quick text filter (plain substring, not regex)
- Extra highlight slots (up to 4 simultaneous patterns in different colours)
- Bookmarks (click gutter strip, F2/Shift+F2 to navigate)
- Minimap (right-side density ruler showing search hits, bookmarks, viewport)
- Tail mode / live file watching
- Command palette (Ctrl+P) — all commands searchable
- Jump-to-line modal (Ctrl+G)
- Zoom (Ctrl+/Ctrl–/Ctrl+0), line selection + Ctrl+C copy
- 5 themes, settings panel
- Mouse wheel vertical scroll (wheel events bypass horizontal scrollable to avoid conflict)
- **Transformation Pipeline** — non-destructive, layer-based transform engine
  - Layer types: Filter (include/exclude), Rewrite (regex replace), Mask (regex replace with mask)
  - Pipeline sidebar (260px, toggled via "Pipeline" button in filter bar)
  - Background worker thread for Filter layers (async, mirrors search worker pattern)
  - Rewrite/Mask layers applied at render time per visible line
  - Context expansion (±5 lines around filtered hits, click triangle to expand/collapse)
  - Jump-to-source mode (bypasses pipeline to show raw file view)
  - Stale ghosting (alpha 0.45 while pipeline computes)

## Build Commands
```bash
cargo check --workspace                   # Type-check
cargo test --workspace                    # Run unit tests (5 pass: line_index, log_level)
cargo run -p flash-app                    # Run on Linux

# Windows exe (cross-compile from Linux — no Docker, no Windows needed)
# Toolchain: cargo-xwin + clang/lld (all already installed on this machine)
cargo xwin build --release -p flash-app --target x86_64-pc-windows-msvc
# → target/x86_64-pc-windows-msvc/release/flash-app.exe  (~13 MB, self-contained GUI exe)
```

## Architecture Notes
- `App` holds a `Vec<Tab>`; each `Tab` owns its `FileMap`, `LineIndex`, search state, filters, bookmarks, highlights, and pipeline state.
- File bytes are loaded into `Arc<Vec<u8>>` (for the search worker) **and** memory-mapped (for line reading) — two copies for large files; acceptable trade-off.
- Search worker is a dedicated OS thread; results arrive in batches of 1000 via `crossbeam-channel`; UI polls every 50 ms.
- Horizontal `scrollable` was removed from `log_view` — it consumed all y-delta wheel events, breaking vertical scroll. Long lines are now clipped; use Wrap mode to see them in full.
- `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` in `main.rs` suppresses the CMD console window on Windows.

### Transformation Pipeline Architecture
- `active_line_filter: Option<Vec<usize>>` was **replaced** by `view_rows: Vec<ViewRow>` + `last_pipeline_output: Vec<usize>`.
- `ViewRow` has two variants: `Line(usize)` and `ContextLine { src, anchor }`.
- Pipeline output flows: `PipelineWorker` → `last_pipeline_output` → `rebuild_view_rows_from_filter` (applies hidden_levels + line_filter_query synchronously) → `view_rows` → virtual scroll.
- `log_view` pre-collects display strings as `Vec<String>` before building iced elements (to avoid `'a` lifetime issues with iced's `span()`).
- `Tab.total_visible_lines()` returns `total_lines()` when `jump_source_line` is `Some`, else `view_rows.len()`.
- New `Tab` fields introduced by the pipeline: `pipeline`, `pipeline_handle`, `pipeline_stale`, `pipeline_open`, `view_rows`, `last_pipeline_output`, `context_expanded`, `jump_source_line`.
- `Palette` gained a `context_row_bg: Color` field; all 5 themes define this colour.
