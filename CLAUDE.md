# Flash — Log Viewer

High-performance read-only log viewer in Rust. Handles files up to 2 GB via memory-mapped I/O.

## Tech Stack
- **UI:** `iced 0.13` — native Rust GUI, Catppuccin Mocha theme (+ Nord, Tokyo Night, Gruvbox, Latte)
- **Core:** `memmap2` file mapping, custom byte-offset line indexer, background regex search via `crossbeam-channel`
- **File dialogs:** `rfd 0.15` · **Async:** `tokio` · **File watching:** `notify`

## Workspace
```
lumen/
├── Cargo.toml
├── crates/
│   ├── flash-core/src/
│   │   ├── lib.rs            # Re-exports public API
│   │   ├── file_map.rs       # memmap2 wrapper
│   │   ├── line_index.rs     # Byte-offset line indexer (has unit tests)
│   │   ├── line_reader.rs    # Line text access via LineIndex
│   │   ├── log_level.rs      # TRACE/DEBUG/INFO/WARN/ERROR detection (has unit tests)
│   │   └── search.rs         # Background regex worker thread
│   └── flash-app/src/
│       ├── main.rs           # Entry point, window config, #[windows_subsystem]
│       ├── app.rs            # App state, Tab struct, Message enum, update/view/subscription
│       ├── theme.rs          # Palette struct + 5 theme palettes
│       ├── views/
│       │   ├── search_bar.rs     # Top toolbar: Open, ≡ menu, file info, regex search, Tail
│       │   ├── filter_bar.rs     # Level chips (TRACE…ERROR), Wrap toggle, quick filter input
│       │   ├── log_view.rs       # Virtual-scroll log area, gutter, minimap, status bar
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
- Log-level syntax highlighting + per-level filter chips
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
- `App` holds a `Vec<Tab>`; each `Tab` owns its `FileMap`, `LineIndex`, search state, filters, bookmarks, and highlights.
- File bytes are loaded into `Arc<Vec<u8>>` (for the search worker) **and** memory-mapped (for line reading) — two copies for large files; acceptable trade-off.
- Search worker is a dedicated OS thread; results arrive in batches of 1000 via `crossbeam-channel`; UI polls every 50 ms.
- Horizontal `scrollable` was removed from `log_view` — it consumed all y-delta wheel events, breaking vertical scroll. Long lines are now clipped; use Wrap mode to see them in full.
- `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` in `main.rs` suppresses the CMD console window on Windows.
