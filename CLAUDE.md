# Flash — Log Viewer Project

## Project Overview
Flash is a high-performance, read-only log viewer built in Rust, optimized for large files (up to 2GB) with a modern UI inspired by the Zed editor.

## Tech Stack
- **Backend:** Rust with `memmap2` for memory-mapped file access
- **UI:** `iced 0.13` (native Rust GUI framework, Catppuccin Mocha theme)
- **Search:** `regex` crate with background thread worker via `crossbeam-channel`
- **File dialogs:** `rfd 0.15`
- **Async runtime:** `tokio`

## Workspace Structure
```
lumen/
├── Cargo.toml                          # Workspace root
├── crates/
│   ├── flash-core/                     # Core engine library
│   │   ├── src/lib.rs                  # Re-exports public API
│   │   ├── src/file_map.rs             # Memory-mapped file wrapper (memmap2)
│   │   ├── src/line_index.rs           # Byte-offset line indexer (has tests)
│   │   ├── src/line_reader.rs          # Reads lines from mmap via LineIndex
│   │   ├── src/log_level.rs            # Log level detection: TRACE/DEBUG/INFO/WARN/ERROR (has tests)
│   │   └── src/search.rs              # Background regex search worker thread
│   └── flash-app/                      # Iced GUI application
│       ├── src/main.rs                 # Entry point, window config
│       ├── src/app.rs                  # App state, Message enum, update/view/subscription
│       ├── src/theme.rs                # Color constants (Catppuccin Mocha palette)
│       ├── src/views/search_bar.rs     # Top toolbar: Open File, search input, Clear
│       ├── src/views/log_view.rs       # Main log area with virtual scrolling, search highlighting
│       ├── src/views/results_panel.rs  # Bottom panel: clickable search results
│       └── src/widgets/virtual_list.rs # Virtual scroll helpers (line height, clamping)
└── test_fixtures/                      # (empty — needs sample.log)
```

## Assignment (3 Steps)
1. **Core Engine:** CLI-first foundation, open 2GB file instantly, line indexer with byte offsets, output first 100 + last 100 lines to console to prove performance.
2. **Modern UI (Zed-Inspired):** Dark themed layout, main log area, bottom filtered results panel, top search bar. Placeholders for: File Open, Clear, Settings, Export.
3. **Advanced Features:** Real-time regex filtering in background thread, search results panel with click-to-jump, log level syntax highlighting.

## Current Status (as of 2026-02-14)

### Completed
- ✅ Memory-mapped file access (`FileMap`)
- ✅ Line indexer with byte offsets (`LineIndex`) + unit tests
- ✅ Line reader (`LineReader`)
- ✅ Log level detection (`LogLevel`) + unit tests
- ✅ Background regex search worker (`SearchHandle`, `spawn_search_worker`)
- ✅ Iced GUI app with dark Catppuccin Mocha theme
- ✅ Virtual scrolling log view (renders only visible lines)
- ✅ Search bar with regex input and submit
- ✅ Search results panel with clickable results that jump to matching line
- ✅ Log level syntax highlighting (color-coded TRACE/DEBUG/INFO/WARN/ERROR)
- ✅ Search match highlighting (yellow background on matching text)
- ✅ Keyboard navigation (PageUp/Down, Home/End, Arrow keys)
- ✅ File Open button + async file dialog (rfd)
- ✅ Clear button
- ✅ Settings and Export button placeholders added
- ✅ `regex = "1"` already in `flash-app/Cargo.toml`

### Remaining Work (Original)
- ❌ **CLI performance proof (Step 1):** No CLI binary exists
- ❌ **Test fixture:** `test_fixtures/` is empty
- ❌ **Rust toolchain:** Not installed on this machine — need `rustup`

## UI Overhaul — COMPLETED (as of 2026-02-14)

### Step 1: theme.rs — Add New Search Highlight Colors (DONE)
- Replace `SEARCH_HIGHLIGHT` → `SEARCH_MATCH_BG` = `Color::from_rgb(0xff/255, 0xb8/255, 0x6c/255)` (bright orange #ffb86c)
- Replace `SEARCH_HIGHLIGHT_BG` → `SEARCH_ROW_BG` = `Color::from_rgba(0xf9/255, 0xe2/255, 0xaf/255, 0.12)` (12% alpha)
- Add `SEARCH_MATCH_FG` = `Color::from_rgb(0.0, 0.0, 0.0)` (pure black)
- Add `SEARCH_GUTTER` = same value as `SEARCH_MATCH_BG`

### Step 2: app.rs — Mouse Wheel Scrolling + Cached Regex + File Info (DONE)
- Add imports: `iced::event`, `iced::mouse`
- Add `MouseScrolled(f32)` to `Message` enum
- Add `compiled_search_regex: Option<regex::Regex>` to `App` struct, init as `None`
- Handle `MouseScrolled`: multiply delta by 3, delegate to `ScrollBy` (negate direction)
- In `SearchSubmit`: compile regex with `regex::Regex::new(&self.search_query).ok()` and cache
- In `Clear`: set `compiled_search_regex = None`
- Add `iced::event::listen_with` subscription for `mouse::Event::WheelScrolled`
  - Check `event::Status::Ignored` to avoid conflicts with results panel scrollable
  - `ScrollDelta::Lines { y, .. }` → use y directly
  - `ScrollDelta::Pixels { y, .. }` → divide by 18.0
- Compute `file_info: Option<(String, u64, usize)>` from `OpenFile` path + `file_data_arc.len()` + `line_index.line_count()`
- Pass `file_info.as_ref().map(|(n, s, l)| (n.as_str(), *s, *l))` to `search_bar::view()`
- Pass `self.compiled_search_regex.as_ref()` to `log_view::view()` and `results_panel::view()`
- Add `fn horizontal_rule() -> Element<Message>` — 1px container with `BORDER` background
- Add `fn format_file_size(bytes: u64) -> String` helper (B/KB/MB/GB)
- Insert `horizontal_rule()` between search_bar and log_view in layout

### Step 3: log_view.rs — Scrollbar + Better Highlighting + Empty State (DONE)
- Change signature: replace `search_query: &str` with `compiled_regex: Option<&regex::Regex>`
- **Empty state:** Centered branded welcome with "Flash" title (32px, ACCENT_BLUE), subtitle, instructions, capacity note
- **Highlighting:** Use `compiled_regex` instead of compiling per-frame. Use `SEARCH_MATCH_BG`/`SEARCH_MATCH_FG` for match spans, `SEARCH_ROW_BG` for row backgrounds
- **Gutter:** 3px left-edge marker on matching lines using `SEARCH_GUTTER` color (use row with conditional gutter container)
- **Scrollbar:** `build_scrollbar()` function returning 12px-wide track with proportional thumb
  - Track: `BG_SECONDARY` background
  - Thumb: semi-transparent gray (`Color::from_rgba(0.6, 0.6, 0.6, 0.4)`), 4px border radius
  - Use `FillPortion` for top_spacer/thumb/bottom_spacer proportions (scale to 1000)
  - Minimum thumb height: 3% of track
  - Visual-only, no drag interaction
- Layout: `row![main_area, scrollbar].height(Fill)` above status bar

### Step 4: search_bar.rs — Professional Toolbar (DONE)
- Add `file_info: Option<(&str, u64, usize)>` parameter
- Add `toolbar_button_style(_theme, status) -> button::Style`:
  - Active: `BG_SURFACE` bg, `BORDER` border, 4px radius
  - Hovered: `BG_HOVER` bg
  - Pressed/Disabled: `BG_SURFACE` bg (disabled gets `FG_MUTED` text)
- Apply `toolbar_button_style` to all buttons
- Style `text_input`: `BG_PRIMARY` bg, `BORDER` border, `ACCENT_BLUE` border when focused, 4px radius
- Add `vertical_divider()` helper: 1px wide, 24px tall, `BORDER` color
- Add file info display (name, formatted size, line count) between buttons and search input
- Build toolbar with: `[Open][Settings][Export] | file_info | [input] [status] [Clear]`
- Add `format_file_size()` local helper

### Step 5: results_panel.rs — Hover + Match Highlighting (DONE)
- Add `compiled_regex: Option<&regex::Regex>` parameter
- Update header: "Search Results" + "(N matches)" in muted color
- Button hover effect: use `status` parameter in style closure, show `BG_HOVER` on hover or selected
- Highlight matching text in results using compiled_regex with `SEARCH_MATCH_BG`/`SEARCH_MATCH_FG`
- Remove inner container bg (button style handles bg now)

### Verification
- `cargo check --workspace` — zero errors, 1 pre-existing warning (unused `Noop` variant)
- `cargo test --workspace` — all 5 tests pass
- Rust toolchain installed: rustc 1.93.1 (stable)

## Build Commands
```bash
cargo check --workspace          # Type check
cargo test --workspace           # Run tests (line_index, log_level)
cargo build -p flash-app         # Build GUI
cargo run -p flash-app           # Run GUI
cargo run -p flash-core --bin flash-cli -- <file>  # Run CLI (after creating it)
```

## Architecture Notes
- File loading reads entire file into `Vec<u8>` via `tokio::fs::read` (app.rs:99), then also mmaps it via `FileMap::open`. The `Vec<u8>` is wrapped in `Arc` for the search worker. This duplicates memory for large files — a future optimization would be to use mmap data directly for search.
- Search worker runs on a dedicated OS thread (not tokio), communicates via `crossbeam-channel`. Results arrive in batches of 1000. A 50ms polling subscription in the UI drains results.
- Virtual scrolling: `viewport_lines` is calculated from window height / 18px line height. Only visible lines are rendered each frame.
