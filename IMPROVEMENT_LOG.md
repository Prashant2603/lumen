# Flash Log Viewer — Improvement Log

> Comprehensive code-review + simulated end-user walkthrough using `test_fixtures/sample.log`.
> Date: 2026-02-21.

---

## Legend

| Severity | Meaning |
|----------|---------|
| 🔴 CRITICAL | Wrong data shown / data loss |
| 🟠 HIGH | Core feature broken or severely degraded |
| 🟡 MEDIUM | Noticeable UX friction; workarounds exist |
| 🟢 LOW | Polish / nice-to-have improvements |

---

## 🔴 CRITICAL Issues

### C-1 — CopyLine copies raw bytes, not pipeline-transformed text (what the user sees)

**Location:** `crates/flash-app/src/app.rs:1374-1386`

When a **Rewrite** or **Mask** pipeline layer is active, every displayed line is transformed at render time in `log_view.rs` via `pl.apply_text_transforms(raw)`. However, `Message::CopyLine(n)` reads the line directly from the file (`reader.get_line(n)`), returning the untransformed raw bytes.

**Repro:** Add a Rewrite layer that replaces timestamps. Right-click any line → "Copy". Clipboard contains the original timestamp, not the transformed display text the user sees.

**Fix:** Apply `tab.pipeline.apply_text_transforms(raw)` before writing to clipboard:

```rust
Message::CopyLine(n) => {
    self.context_menu_line = None;
    if let Some(tab) = self.tab() {
        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
        if let Some(raw) = reader.get_line(n) {
            let display = tab.pipeline.apply_text_transforms(raw);
            return iced::clipboard::write(display.into_owned());
        }
    }
}
```

---

## 🟠 HIGH Issues

### H-1 — Horizontal scrollbar is purely decorative (not clickable or draggable)

**Location:** `crates/flash-app/src/views/log_view.rs:481-522`

The `h_scrollbar` element renders a thumb at the correct proportional position but has **no interaction handlers** — no `mouse_area`, no click/press callbacks. The only way to scroll horizontally is `Shift+Wheel`, which is completely undiscoverable and unavailable on trackpads. Users will see the scrollbar and click it expecting it to work, but nothing happens.

**Fix:** Wrap the bar in a `mouse_area` that emits `HScrollBy` based on the click X position, or implement draggable-thumb behavior mirroring the existing `scrollbar_dragging` / `ScrollbarClicked` / `CursorMoved` pattern.

---

### H-2 — Scroll position resets to top on every filter keystroke

**Location:** `crates/flash-app/src/app.rs:156-158` inside `Tab::rebuild_view_rows_from_filter`

```rust
self.view_rows    = Self::expand_with_context(&post_quick, &context_expanded, total);
self.scroll_offset = 0;   // always unconditional reset
```

Every character typed into the quick-filter box triggers `LineFilterChanged` → `rebuild_view_rows_from_filter` → `scroll_offset = 0`. If a user is deep in a large file and refines the filter, they are snapped back to the top on every keystroke.

**Fix:** Only reset `scroll_offset` if the current offset would exceed the new row count — the clamp in `recalc_viewport` is sufficient to handle overshooting.

---

### H-3 — No settings persistence: theme, font size, wrap, bookmarks, pipeline lost on restart

**Location:** `crates/flash-app/src/app.rs:App::new()`

Only recent files (`~/.flash_recent`) survive a restart. All other user preferences are reset:

- Theme selection → always `CatppuccinMocha`
- Font size / zoom → always 16 px
- Wrap mode → always off
- Color log levels → always off
- Bookmarks → lost on tab close / restart
- Pipeline layers → lost on tab close / restart

**Fix (priority order):**
1. Save `{ theme, font_size, line_wrap, color_log_levels }` to `~/.flash_config.toml` on change.
2. Save/load bookmarks keyed by file path.
3. Optionally serialize pipeline config per file.

---

### H-4 — Jump-to-line (Ctrl+G) jumps to view-row index, not file line number

**Location:** `crates/flash-app/src/app.rs:982-990`

```rust
Message::JumpSubmit => {
    if let Ok(n) = self.jump_input.trim().parse::<usize>() {
        let line    = n.saturating_sub(1);
        let clamped = self.clamp_scroll(line);  // clamps to total_visible_lines()
        ...
    }
}
```

`clamp_scroll` calls `total_visible_lines()` which returns `view_rows.len()` when the pipeline is filtering. So if the pipeline filters 5,000 lines down to 200 visible rows and the user types "1500", they land at view row 200 (clamped to last), not file line 1500. Users universally expect Ctrl+G to navigate to a **file line number**.

**Fix:** When the pipeline is active, scan `view_rows` for the first entry whose `src` ≥ the requested file line, then scroll to that view-row index. Fall back to the last row if not found.

---

## 🟡 MEDIUM Issues

### M-1 — Scroll position stored in field misleadingly named `scrollbar_hover_y`

**Location:** `crates/flash-app/src/app.rs:244`, `:876-880`, `:1564-1566`

The field `scrollbar_hover_y: f32` is used for two unrelated purposes:

1. Scrollbar dragging — the cursor Y relative to the scrollbar track.
2. Context menu positioning — `views::context_menu::view(..., self.scrollbar_hover_y, ...)`.

The context menu positioning happens to work because both uses reference the same window-relative cursor Y. However the naming is a maintenance hazard: a future developer might "fix" the scrollbar Y calculation without realising it also moves the context menu.

**Fix:** Add a separate `cursor_y: f32` field mirroring `cursor_x: f32`, set both in `CursorMoved`. Use `cursor_y` for the context menu and `scrollbar_hover_y` only for the scrollbar drag.

---

### M-2 — Alternating row stripe pattern is discontinuous after pipeline filtering

**Location:** `crates/flash-app/src/views/log_view.rs:303`

```rust
let alt_row_bg: Option<Color> = if src % 2 == 1 { Some(p.bg_alt_row) } else { None };
```

`src` is the **original file line number**, not the view-row index. When filtering, consecutive visible lines from file lines 1, 3, 5 (all odd) all receive the same stripe colour — no visible alternation. Lines from even file indices all appear un-striped.

**Fix:** Use the loop counter `idx` (the view-row index) instead of `src % 2`.

---

### M-3 — Pipeline preview status bar hint says `[>]`, button actually shows `▶`

**Location:** `crates/flash-app/src/views/log_view.rs:427`

```
"PREVIEW  ...  click [>] on a layer to change preview, or [>] on again to exit"
```

The pipeline panel renders the preview button as the Unicode triangle character `▶` (U+25B6). The status bar hint uses ASCII `[>]`, which won't match what the user sees.

**Fix:** Replace `[>]` with `▶` in the format string.

---

### M-4 — ClearFilters does not stop the background search worker

**Location:** `crates/flash-app/src/app.rs:919-929`

`Message::ClearFilters` clears `line_filter_query` and `hidden_levels` but does **not** cancel the regex search (`search_query`, `search_results`, `compiled_search_regex`, `search_handle`). The "✕" button in the filter bar only clears the quick filter. If the user has a search running, clearing filters has no effect on it.

**Fix:** Decide whether "clear" should clear everything. If so, call `tab.clear_search()` from `ClearFilters`. Alternatively, add a separate visible "Clear All" button.

---

### M-5 — `pipeline_w` computed but unused; pipeline panel overlaps log content

**Location:** `crates/flash-app/src/app.rs:426-427`

```rust
let pipeline_w = self.tab().map(|t| if t.pipeline_open { 260.0_f32 } else { 0.0 }).unwrap_or(0.0);
let _ = pipeline_w; // used implicitly through window_height
```

The comment claims it is used implicitly, but `viewport_lines` is calculated purely from `window_height` without subtracting the pipeline panel width. The pipeline panel (260 px) reduces the horizontal space for lines, but `recalc_viewport` doesn't account for this when calculating `viewport_lines`. This means the viewport line count is correct vertically, but horizontal clipping occurs when the pipeline is open without the h-scroll offset being aware of the reduced viewport width.

**Fix:** Remove the dead `pipeline_w` computation and the `let _ =` suppression. Document that viewport line count is deliberately calculated only from vertical height.

---

### M-6 — No keyboard shortcut to open Recent Files panel

**Location:** `crates/flash-app/src/app.rs:813-871` (`KeyPressed` handler)

The "Recent" button only accessible via mouse. There is no `Ctrl+R` or similar binding.

**Fix:** Add `"r"` under `modifiers.control()` to emit `Message::ToggleRecentFiles`.

---

### M-7 — Context menu does not dismiss on scroll

**Location:** `crates/flash-app/src/app.rs`

If a context menu is open (`context_menu_line = Some(n)`) and the user scrolls the log view, the menu remains visible but now points to a different visual line than the one originally right-clicked. The menu should close on any scroll event.

**Fix:** In `Message::MouseScrolled`, `Message::ScrollBy`, `Message::ScrollTo`, and `Message::GoToBottom` handlers, add `self.context_menu_line = None;`.

---

## 🟢 LOW Issues

### L-1 — Pressing Enter in filter input is a no-op (Noop message)

**Location:** `crates/flash-app/src/views/filter_bar.rs:57`

```rust
.on_submit(Message::Noop)
```

Since filtering is live (updates on each keystroke), Enter doing nothing is logically acceptable. However users accustomed to pressing Enter after typing a search/filter get no confirmation. A possible interpretation: Enter could lock in the filter as a pinned highlight, or submit the term as a regex search.

**Suggested fix:** Map Enter to `Message::SearchSubmit` so typing in the filter box and pressing Enter triggers a highlighted search, matching VS Code's Ctrl+F behaviour.

---

### L-2 — "Recent" button has invisible border in closed state

**Location:** `crates/flash-app/src/views/filter_bar.rs:188`

```rust
let rec_bdr = if recent_open { Color { a: 0.50, ..acc } } else { Color::TRANSPARENT };
```

The "Wrap" and `.*` buttons always show a subtle border (`Color { a: 0.35, ..bdr }`). The "Recent" button is borderless when closed, making it visually inconsistent.

**Fix:** Use `Color { a: 0.35, ..bdr }` for the closed-state border.

---

### L-3 — Proc-stats subscription fires even when no file is loaded

**Location:** `crates/flash-app/src/app.rs:1626`

```rust
subs.push(iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::UpdateProcStats));
```

This subscription is unconditional — it fires on the splash screen and when no tabs are open. The `sysinfo` call is cheap but unnecessary overhead.

**Fix:** Condition on `self.info_panel_open && !self.tabs.is_empty()`.

---

### L-4 — `tail_mode` is global; should be per-tab

**Location:** `crates/flash-app/src/app.rs` — `App` struct

When multiple tabs are open and the user toggles Tail mode, the `tail_mode: bool` flag applies globally. File watching (`_watcher` / `watch_rx`) is per active tab, but the flag is shared. Opening a second tab while tail mode is on means the second tab is also considered "live" if focus returns to it.

**Fix:** Move `tail_mode` into `Tab`. The watcher is already per-tab (set up via `setup_watcher()`).

---

### L-5 — No UI affordance to manually reload the current file

**Location:** `crates/flash-app/src/app.rs:603-607`

`Message::ReloadActiveFile` exists and works correctly, but is only triggered indirectly via `PollFileChange` in tail mode. There is no "Reload" button, menu entry, or keyboard shortcut (e.g., F5).

**Fix:** Add `ReloadActiveFile` to the command palette and handle `Key::Named(keyboard::key::Named::F5)` in `KeyPressed`.

---

### L-6 — Minimap click is off by tab-bar height when multiple tabs are open

**Location:** `crates/flash-app/src/app.rs:440`

```rust
let toolbar_h: f32 = 38.0;  // filter_bar height (search_bar removed)
```

When multiple tabs are open, a 34 px tab bar is shown above the filter bar. The minimap click handler (`scroll_to_cursor_y`) subtracts only the filter bar height (38 px), not 38 + 34 = 72 px. Clicking near the top of the minimap when tabs are visible will overshoot the intended scroll target.

**Fix:**

```rust
let toolbar_h: f32 = if self.tabs.len() > 1 { 38.0 + 34.0 } else { 38.0 };
```

---

### L-7 — Toggling an empty pipeline Filter layer triggers an unnecessary pipeline run

**Location:** `crates/flash-app/src/app.rs:1187-1195`

`PipelineToggleLayer` unconditionally calls `self.trigger_pipeline()`. If the layer has an empty or invalid pattern (no compiled regex), the pipeline worker receives an empty config and runs over the entire file needlessly.

**Fix:** Skip `trigger_pipeline()` if the only active filter layers have empty patterns.

---

### L-8 — `AddHighlight` uses `line_filter_query` pattern, not `search_query`

**Location:** `crates/flash-app/src/app.rs:1102-1117`

The "Add Highlight" command in the command palette and `+ Highlight` button in the filter bar pin `tab.line_filter_query` as a highlight colour. If the user's active search is in `search_query` (submitted via Enter in the old search bar), the highlight will be empty or unexpected.

Since the search bar was removed and filter bar is the sole input, this is now consistent — but should be documented.

---

## UX Suggestions (No Code Bugs)

### U-1 — Add tooltips to pipeline panel buttons
The "Filter", "Rewrite", "Mask", "▲", "▼", "✕", "▶" pipeline buttons have no tooltips. New users won't know the difference between Rewrite and Mask without reading source code.

### U-2 — Show "Shift+Scroll to scroll horizontally" hint when wrap is off
Add a subtle hint in the status bar text when `!line_wrap`: `Shift+Scroll: pan • ` prefix. This makes the horizontal scroll interaction discoverable.

### U-3 — Context menu: add "Select Line" option
Right-clicking a line currently sets `context_menu_line` but does not set `selected_line`. The context menu "Copy (line N)" button is the only action. Adding "Select" would allow the user to copy via Ctrl+C after dismissal, keeping the workflow familiar.

### U-4 — Undo-close-tab (Ctrl+Shift+T)
Closing a tab is irreversible. Storing the last-closed tab's path in `recent_files` and handling `Ctrl+Shift+T` → `FileDrop(recent_files[0])` would match browser conventions.

### U-5 — Pipeline panel: show layer type badge colours
Filter layers could show a blue badge, Rewrite yellow, Mask red — making the layer type identifiable at a glance without reading the label.

---

---

## Additional Issues Found (Second-Pass QA Review)

### C-2 — Runtime panic: `results_panel` slices `line_text` by byte index

**Location:** `crates/flash-app/src/views/results_panel.rs:57-58` ✅ FIXED

```rust
// Before (panics on multi-byte UTF-8 — CJK, emoji, accented chars):
let truncated: String = if line_text.len() > 200 {
    format!("{}…", &line_text[..200])  // byte-index slice
};

// After:
let mut chars = line_text.chars();
let s: String = chars.by_ref().take(200).collect();
if chars.next().is_some() { format!("{}…", s) } else { s }
```

Any log containing non-ASCII characters longer than 200 bytes (e.g., CJK log messages, emoji-tagged logs, accented service names) would cause a `thread 'main' panicked at 'byte index X is not a char boundary'` crash.

---

### C-3 — Search functionality completely inaccessible from UI ✅ FIXED

**Location:** `crates/flash-app/src/views/filter_bar.rs:57`, `app.rs:893-904`

`search_bar.rs` was removed and the filter bar became the primary input, but the background regex search worker was never wired to it:

- `filter_bar.rs` had `.on_submit(Message::Noop)` — pressing Enter did nothing
- `tab.search_query` was never set from the filter input; `SearchSubmit` used `search_query` which remained empty
- The results panel, search highlighting (gold gutter strip, row background), minimap hit markers, and search history were unreachable

**Fix applied:**
- `LineFilterChanged` now also sets `tab.search_query = query.clone()` to keep both fields in sync
- Filter bar `on_submit` changed from `Noop` to `SearchSubmit` — pressing Enter now triggers the background regex search worker and populates the results panel

---

### H-5 — Command palette mouse clicks always execute the keyboard-highlighted item ✅ FIXED

**Location:** `crates/flash-app/src/views/command_palette.rs:69-81`

Every palette row button sent `Message::PaletteSelect`. The handler executed `cmds.nth(self.palette_selected)` — the keyboard cursor position. Clicking any row in the palette would run the keyboard-highlighted command instead of the clicked one.

**Fix applied:** Added `Message::PaletteRunIdx(usize)`. Each palette row button now sends `PaletteRunIdx(idx)` which executes that specific row's action directly, independent of keyboard selection.

---

### M-8 — ResultClicked scrolls to wrong position when pipeline filter is active

**Location:** `crates/flash-app/src/app.rs:744-754`

```rust
Message::ResultClicked(idx) => {
    ...
    let target = line_num.saturating_sub(vp / 2);
    let clamped = virtual_list::clamp_offset(target, tab.total_visible_lines(), vp);
    tab.scroll_offset = clamped;
```

`line_num` is the **original file line number** from the search result. `scroll_offset` is an index into `view_rows` (the filtered set). When the pipeline has filtered 5,000 lines to 200 visible rows, clicking a search result from file line 4,500 clamps to view row 200 (last), landing nowhere near the result.

**Fix needed:** Convert `line_num` to a view-row index (scan `view_rows` for the closest `src` to `line_num`) before setting `scroll_offset`.

---

### M-9 — Bookmark navigation (F2) broken when pipeline filter is active

**Location:** `crates/flash-app/src/app.rs:1049-1068`

```rust
Message::NextBookmark => {
    let next = sorted.iter().find(|&&b| b > cur).copied().unwrap_or(sorted[0]);
    let c = self.clamp_scroll(next);
    if let Some(t) = self.tab_mut() { t.scroll_offset = c; }
}
```

`b` is an original file line number from `tab.bookmarks`. `clamp_scroll` clamps to `total_visible_lines()` (view_rows count). Pressing F2 when a pipeline filter is active will clamp to the last visible row, not the bookmarked line's position in the filtered view.

**Fix needed:** Same pattern as M-8 — convert file line → view_row index before scrolling.

---

## Summary Table

| ID | Severity | Status | Description | File |
|----|----------|--------|-------------|------|
| C-1 | 🔴 CRITICAL | ✅ Fixed | CopyLine copies raw bytes, not transformed text | `app.rs` |
| C-2 | 🔴 CRITICAL | ✅ Fixed | Byte-index slice panic on multi-byte UTF-8 in results panel | `results_panel.rs` |
| C-3 | 🔴 CRITICAL | ✅ Fixed | Search worker completely inaccessible from UI | `filter_bar.rs`, `app.rs` |
| H-1 | 🟠 HIGH | Open | H-scrollbar decorative only, not clickable | `log_view.rs` |
| H-2 | 🟠 HIGH | ✅ Fixed | Scroll resets to top on every filter keystroke | `app.rs` |
| H-3 | 🟠 HIGH | Open | Settings/bookmarks/pipeline not persisted across restarts | app-wide |
| H-4 | 🟠 HIGH | Open | Ctrl+G jumps to view-row index, not file line number | `app.rs` |
| H-5 | 🟠 HIGH | ✅ Fixed | Palette mouse clicks run keyboard-highlighted item | `command_palette.rs` |
| M-1 | 🟡 MEDIUM | Open | `scrollbar_hover_y` dual-use; needs `cursor_y` alias | `app.rs` |
| M-2 | 🟡 MEDIUM | ✅ Fixed | Alternating row stripes discontinuous after filtering | `log_view.rs` |
| M-3 | 🟡 MEDIUM | ✅ Fixed | Preview status bar says `[>]` but button shows `▶` | `log_view.rs` |
| M-4 | 🟡 MEDIUM | Open | ClearFilters doesn't stop search worker | `app.rs` |
| M-5 | 🟡 MEDIUM | Open | `pipeline_w` computed but suppressed with `let _` | `app.rs` |
| M-6 | 🟡 MEDIUM | ✅ Fixed | No keyboard shortcut (Ctrl+R) for Recent Files | `app.rs` |
| M-7 | 🟡 MEDIUM | ✅ Fixed | Context menu stays open on scroll | `app.rs` |
| M-8 | 🟡 MEDIUM | Open | ResultClicked scrolls to wrong position with pipeline filter | `app.rs` |
| M-9 | 🟡 MEDIUM | Open | F2 bookmark navigation broken with pipeline filter | `app.rs` |
| L-1 | 🟢 LOW | ✅ Fixed | Enter in filter input now triggers search (was Noop) | `filter_bar.rs` |
| L-2 | 🟢 LOW | ✅ Fixed | Recent button invisible border when closed | `filter_bar.rs` |
| L-3 | 🟢 LOW | ✅ Fixed | Proc-stats poll fires even with no file open | `app.rs` |
| L-4 | 🟢 LOW | Open | `tail_mode` global, should be per-tab | `app.rs` |
| L-5 | 🟢 LOW | ✅ Fixed | No UI/keyboard shortcut to manually reload file (F5) | `app.rs` |
| L-6 | 🟢 LOW | Open | Minimap click off-by-34px when tab bar shown | `app.rs` |
| L-7 | 🟢 LOW | Open | Empty pipeline layer still triggers full pipeline run | `app.rs` |
| L-8 | 🟢 LOW | Open | AddHighlight uses filter query, not search query | `app.rs` |
