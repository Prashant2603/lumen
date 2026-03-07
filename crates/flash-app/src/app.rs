use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use flash_core::{
    spawn_pipeline_worker, spawn_search_worker, FileMap, LineIndex, LineReader,
    PipelineHandle, PipelineResponse, SearchHandle, SearchResponse,
};
use iced::keyboard::{self, Key};
use iced::widget::{column, container, row, stack, text, text_input};
use iced::{event, mouse, window};
use iced::{Color, Element, Length, Subscription, Task, Theme};

use notify::Watcher as _;
use sysinfo::{ProcessesToUpdate, System as SysInfo};

use crate::pipeline::TransformPipeline;
use crate::theme::{self, AppTheme, Palette};
use crate::views;
use crate::widgets::virtual_list;

// ── Highlight colors palette (4 slots) ───────────────────────────────────────

pub const HIGHLIGHT_COLORS: [Color; 4] = [
    Color { r: 0.40, g: 0.80, b: 0.40, a: 1.0 }, // green
    Color { r: 0.70, g: 0.45, b: 0.90, a: 1.0 }, // purple
    Color { r: 0.30, g: 0.85, b: 0.90, a: 1.0 }, // cyan
    Color { r: 0.93, g: 0.63, b: 0.35, a: 1.0 }, // amber
];

// ── TextSel ───────────────────────────────────────────────────────────────────

/// Character-level text selection (anchor = where drag started, focus = current cursor).
#[derive(Clone, Copy, Debug)]
pub struct TextSel {
    pub anchor_line: usize,
    pub anchor_col:  usize,
    pub focus_line:  usize,
    pub focus_col:   usize,
}

impl TextSel {
    /// Return (lo, hi) in (line, col) order so lo ≤ hi.
    pub fn normalised(&self) -> ((usize, usize), (usize, usize)) {
        let a = (self.anchor_line, self.anchor_col);
        let f = (self.focus_line,  self.focus_col);
        if a <= f { (a, f) } else { (f, a) }
    }
    /// True when anchor == focus (no text actually selected).
    pub fn is_empty(&self) -> bool {
        self.anchor_line == self.focus_line && self.anchor_col == self.focus_col
    }
}

// ── ViewRow ───────────────────────────────────────────────────────────────────

/// A row in the virtual-scroll view.
#[derive(Debug, Clone)]
pub enum ViewRow {
    /// A regular line from the filtered set (by original index).
    Line(usize),
    /// A context line shown around an expanded anchor line.
    ContextLine { src: usize, anchor: usize },
}

impl ViewRow {
    pub fn src(&self) -> usize {
        match self {
            ViewRow::Line(n)                    => *n,
            ViewRow::ContextLine { src, .. } => *src,
        }
    }
}

// ── Structs ───────────────────────────────────────────────────────────────────

pub struct OpenFile {
    pub file_map:   FileMap,
    pub line_index: LineIndex,
    pub path:       PathBuf,
}

#[derive(Debug, Clone)]
pub struct ExtraHighlight {
    pub pattern: String,
    pub regex:   Option<regex::Regex>,
    pub color:   Color,
}

/// All state belonging to a single open file/tab.
pub struct Tab {
    pub open_file:             OpenFile,
    pub scroll_offset:         usize,
    pub search_query:          String,
    pub search_results:        Vec<(usize, String)>,
    pub search_result_set:     HashSet<usize>,
    pub selected_result:       Option<usize>,
    pub search_handle:         SearchHandle,
    pub search_in_progress:    bool,
    pub compiled_search_regex: Option<regex::Regex>,
    pub file_data_arc:         Arc<dyn AsRef<[u8]> + Send + Sync>,
    pub line_offsets_arc:      Arc<Vec<u64>>,
    pub hidden_levels:         HashSet<flash_core::LogLevel>,
    pub line_filter_query:     String,
    pub selected_line:         Option<usize>,
    // Bookmarks
    pub bookmarks:             HashSet<usize>,
    pub extra_highlights:      Vec<ExtraHighlight>,
    // Pipeline
    pub pipeline:              TransformPipeline,
    pub pipeline_handle:       PipelineHandle,
    pub pipeline_stale:        bool,
    pub pipeline_open:         bool,
    pub view_rows:             Vec<ViewRow>,
    pub last_pipeline_output:  Vec<usize>,
    pub context_expanded:      HashSet<usize>,
    pub jump_source_line:      Option<usize>,
    pub pipeline_preview_to:   Option<u64>,   // layer id up to which we preview
    pub filter_regex:          Option<regex::Regex>, // compiled filter regex (when regex mode on)
    pub h_scroll_offset:       usize,         // horizontal char offset (non-wrap mode)
}

impl Tab {
    pub fn file_name(&self) -> String {
        self.open_file.path.file_name().unwrap_or_default().to_string_lossy().to_string()
    }
    pub fn total_lines(&self) -> usize { self.open_file.line_index.line_count() }
    pub fn total_visible_lines(&self) -> usize {
        if self.jump_source_line.is_some() {
            self.total_lines()
        } else {
            self.view_rows.len()
        }
    }

    pub fn clear_search(&mut self) {
        self.search_handle.cancel();
        self.search_query.clear();
        self.search_results.clear();
        self.search_result_set.clear();
        self.selected_result     = None;
        self.search_in_progress  = false;
        self.compiled_search_regex = None;
    }

    /// Apply quick text filter (hidden_levels + line_filter_query) to the
    /// pipeline output, expand context, and store in view_rows.
    pub fn rebuild_view_rows_from_filter(&mut self, filtered: Vec<usize>) {
        // Borrow individual fields to avoid capturing `self` in closure
        let data: &[u8]       = self.file_data_arc.as_ref().as_ref();
        let line_index        = &self.open_file.line_index;
        let reader            = LineReader::new(data, line_index);
        let q_lower           = self.line_filter_query.to_lowercase();
        let hidden_levels     = &self.hidden_levels;
        let filter_regex      = &self.filter_regex;

        let post_quick: Vec<usize> = filtered.into_iter().filter(|&i| {
            if !hidden_levels.is_empty() {
                if let Some(line) = reader.get_line(i) {
                    if let Some(lvl) = flash_core::LogLevel::detect(line) {
                        if hidden_levels.contains(&lvl) { return false; }
                    }
                }
            }
            if !q_lower.is_empty() {
                if let Some(line) = reader.get_line(i) {
                    if let Some(re) = filter_regex {
                        if !re.is_match(line) { return false; }
                    } else {
                        if !line.to_lowercase().contains(&q_lower) { return false; }
                    }
                }
            }
            true
        }).collect();

        let total = self.open_file.line_index.line_count();
        // Clone context_expanded to avoid borrow conflict when writing view_rows
        let context_expanded = self.context_expanded.clone();
        self.view_rows = Self::expand_with_context(&post_quick, &context_expanded, total);
        // Only snap to top if the current position is now out of bounds
        if self.scroll_offset >= self.view_rows.len().max(1) {
            self.scroll_offset = 0;
        }
    }

    fn expand_with_context(
        filtered: &[usize],
        expanded: &HashSet<usize>,
        total:    usize,
    ) -> Vec<ViewRow> {
        if expanded.is_empty() {
            return filtered.iter().map(|&src| ViewRow::Line(src)).collect();
        }

        const CTX: usize = 5;
        let filtered_set: HashSet<usize> = filtered.iter().copied().collect();

        // Collect (sort_key, ViewRow) pairs, deduplicated
        let mut order: Vec<(usize, ViewRow)> = Vec::new();
        let mut emitted: HashSet<usize>      = HashSet::new();

        for &anchor in filtered {
            if expanded.contains(&anchor) {
                let start = anchor.saturating_sub(CTX);
                let end   = (anchor + CTX + 1).min(total);
                for src in start..end {
                    if emitted.insert(src) {
                        if src == anchor || filtered_set.contains(&src) {
                            order.push((src, ViewRow::Line(src)));
                        } else {
                            order.push((src, ViewRow::ContextLine { src, anchor }));
                        }
                    }
                }
            } else if emitted.insert(anchor) {
                order.push((anchor, ViewRow::Line(anchor)));
            }
        }

        order.sort_by_key(|(k, _)| *k);
        order.into_iter().map(|(_, row)| row).collect()
    }
}

// ── App struct ────────────────────────────────────────────────────────────────

pub struct App {
    // Tabs
    pub tabs:       Vec<Tab>,
    pub active_tab: usize,
    // Viewport
    viewport_lines: usize,
    window_height:  f32,
    // Scrollbar drag
    scrollbar_hover_y:  f32,
    scrollbar_dragging: bool,
    // Font / zoom
    font_size: f32,
    // Modifier key tracking
    modifiers: iced::keyboard::Modifiers,
    // Theme / settings
    app_theme:       AppTheme,
    pub bg_opacity:  f32,  // 0.0 = fully transparent, 1.0 = fully opaque
    settings_open:   bool,
    info_panel_open: bool,
    // Line wrap
    pub line_wrap: bool,
    // Filter regex mode
    pub filter_is_regex:    bool,
    // Color lines by log level
    pub color_log_levels:   bool,
    // Jump to line modal
    jump_open:  bool,
    jump_input: String,
    // Command palette
    palette_open:     bool,
    palette_query:    String,
    palette_selected: usize,
    // Search history
    search_history:     Vec<String>,
    history_cursor:     Option<usize>,
    history_temp_query: String,
    // Recent files
    recent_files: Vec<PathBuf>,
    recent_files_open: bool,
    // Process stats (CPU / memory shown in info panel)
    sysinfo_sys:  SysInfo,
    proc_mem_mb:  f64,
    proc_cpu_pct: f64,
    // Cursor position (x,y) for context menu placement
    cursor_x: f32,
    // Character-level text selection
    pub text_sel:        Option<TextSel>,
    mouse_selecting:     bool,
    hovered_line:        Option<usize>,  // last line the cursor was over (from mouse_area on_enter/on_move)
    log_area_top_cal:    Option<f32>,    // calibrated log area top (from LineHovered + cursor_y)
    last_click_time:     Option<Instant>, // for double/triple-click detection
    click_count:         u32,             // 1=single, 2=double, 3=triple
    pub block_cursor:    bool,            // false=line caret (default), true=block (Insert toggles)
    // Right-click context menu (position frozen at the moment of right-click)
    context_menu_line: Option<usize>,
    context_menu_x:    f32,
    context_menu_y:    f32,
    // Tail mode / file watching
    tail_mode: bool,
    _watcher:  Option<notify::RecommendedWatcher>,
    watch_rx:  Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
}

// ── Command palette commands ──────────────────────────────────────────────────

pub struct PaletteCmd {
    pub label:    String,
    pub shortcut: &'static str,
    pub action:   Message,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    FileOpen,
    FileDrop(PathBuf),
    FileLoaded(Result<(PathBuf, Option<Vec<u8>>), String>),
    ReloadActiveFile,
    FileReloaded(Result<(PathBuf, Option<Vec<u8>>), String>),
    ScrollTo(usize),
    ScrollBy(i64),
    GoToBottom,
    MouseScrolled(f32),
    SearchQueryChanged(String),
    SearchSubmit,
    PollSearchResults,
    ResultClicked(usize),
    Clear,
    Export,
    ExportSaved(Result<(), String>),
    WindowResized(iced::Size),
    KeyPressed(Key, keyboard::Modifiers),
    // Scrollbar
    ScrollbarClicked,
    ScrollbarReleased,
    CursorMoved(f32, f32),   // x, y
    // Filters
    ToggleLevelFilter(flash_core::LogLevel),
    LineFilterChanged(String),
    ToggleFilterRegex,
    ToggleColorLogLevels,
    ClearFilters,
    // Zoom
    ZoomIn,
    ZoomOut,
    ZoomReset,
    // Modifier
    ModifiersChanged(iced::keyboard::Modifiers),
    // Theme / settings
    SetTheme(AppTheme),
    SetOpacity(f32),
    ToggleSettings,
    // Line selection / clipboard
    LineClicked(usize),
    CopyToClipboard,
    // Character-level text selection
    LeftMouseDown,        // global left-button press — starts selection (rich_text eats on_press)
    TextSelUpdate(f32),   // cursor_x; col-only update (line tracked by LineHovered)
    TextSelClear,
    LineHovered(usize),   // cursor moved over log row `src` (for drag-select row tracking)
    // Jump to line
    JumpOpen,
    JumpInputChanged(String),
    JumpSubmit,
    JumpClose,
    // Line wrap
    WrapToggle,
    // Command palette
    PaletteOpen,
    PaletteClose,
    PaletteQueryChanged(String),
    PaletteMove(i32),
    PaletteSelect,
    PaletteRunIdx(usize), // mouse-click on a specific palette row
    // Tabs
    CloseTab(usize),
    SwitchTab(usize),
    // Bookmarks
    ToggleBookmark(usize),
    NextBookmark,
    PrevBookmark,
    // Search history
    HistoryPrev,
    HistoryNext,
    // Tail mode / file watching
    TailToggle,
    PollFileChange,
    // Extra highlights
    AddHighlight,
    RemoveHighlight(usize),
    // Right info panel
    ToggleInfoPanel,
    // Process stats update
    UpdateProcStats,
    // Recent files panel
    ToggleRecentFiles,
    // Horizontal scroll (non-wrap mode, Shift+wheel)
    HScrollBy(i64),
    HScrollTo(f32),  // slider-based scrollbar: absolute position (char offset as f32)
    // Right-click context menu
    RightClickLine(usize),
    CloseContextMenu,
    CopyLine(usize),   // carries line index directly — immune to race with CloseContextMenu
    CopySelection,     // copy the current text_sel to clipboard (context menu)
    // Pipeline
    TogglePipeline,
    PipelinePreviewLayer(Option<u64>),  // None = clear preview
    PipelineAddFilter,
    PipelineAddRewrite,
    PipelineAddMask,
    PipelineRemoveLayer(u64),
    PipelineToggleLayer(u64),
    PipelineToggleLayerExclude(u64),
    PipelineMoveLayer(u64, i32),       // -1 = up, +1 = down
    PipelineEditPattern(u64, String),
    PipelineEditExtra(u64, String),
    PipelineCommitLayer(u64),          // Enter/Apply — validate regex, trigger pipeline
    PollPipeline,
    ToggleContext(usize),              // toggle context expansion for anchor line
    JumpToSource(usize),              // bypass pipeline, show raw line
    JumpToSourceClear,                // return to pipeline view
    Noop,
}

// ── App implementation ────────────────────────────────────────────────────────

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            tabs:               Vec::new(),
            active_tab:         0,
            viewport_lines:     50,
            window_height:      900.0,
            scrollbar_hover_y:  0.0,
            scrollbar_dragging: false,
            font_size:          16.0,
            modifiers:          iced::keyboard::Modifiers::default(),
            app_theme:          AppTheme::CatppuccinMocha,
            bg_opacity:         1.0,
            settings_open:      false,
            info_panel_open:    true,
            line_wrap:          false,
            filter_is_regex:    false,
            color_log_levels:   false,
            jump_open:          false,
            jump_input:         String::new(),
            palette_open:       false,
            palette_query:      String::new(),
            palette_selected:   0,
            search_history:     Vec::new(),
            history_cursor:     None,
            history_temp_query: String::new(),
            recent_files:       load_recent_files(),
            recent_files_open:  false,
            sysinfo_sys:        SysInfo::new(),
            proc_mem_mb:        0.0,
            proc_cpu_pct:       0.0,
            cursor_x:           0.0,
            text_sel:           None,
            mouse_selecting:    false,
            hovered_line:       None,
            log_area_top_cal:   None,
            last_click_time:    None,
            click_count:        0,
            block_cursor:       false,
            context_menu_line:  None,
            context_menu_x:     0.0,
            context_menu_y:     0.0,
            tail_mode:          false,
            _watcher:           None,
            watch_rx:           None,
        };
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        match self.tab() {
            Some(t) => format!("Flash — {}", t.file_name()),
            None    => "Flash — Log Viewer".to_string(),
        }
    }

    fn palette(&self) -> Palette {
        let p = theme::palette(self.app_theme);
        if self.bg_opacity < 1.0 {
            theme::apply_opacity(p, self.bg_opacity)
        } else {
            p
        }
    }

    fn tab(&self)     -> Option<&Tab>     { self.tabs.get(self.active_tab) }
    fn tab_mut(&mut self) -> Option<&mut Tab> { self.tabs.get_mut(self.active_tab) }

    fn clamp_scroll(&self, offset: usize) -> usize {
        let total = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
        virtual_list::clamp_offset(offset, total, self.viewport_lines)
    }

    /// Pixel offset from the window left edge to the start of log text (char column 0).
    fn gutter_offset(&self) -> f32 {
        const ACTIVITY_BAR: f32 = 56.0;
        const PIPELINE_W:   f32 = 260.0;
        const GUTTER_STRIP: f32 = 6.0;  // 3px bookmark + 3px search-result strips
        let pipeline_w     = self.tab().map(|t| if t.pipeline_open { PIPELINE_W } else { 0.0 }).unwrap_or(0.0);
        let total_lines    = self.tab().map(|t| t.total_lines()).unwrap_or(1);
        let line_num_chars = format!("{}", total_lines.max(1)).len() as f32;
        let line_num_col   = (line_num_chars * 8.5 + 20.0_f32).max(44.0_f32);
        ACTIVITY_BAR + pipeline_w + GUTTER_STRIP + line_num_col
    }

    /// Top of the log content area (y pixels from window top).
    fn log_area_top(&self) -> f32 {
        let sep_h = if self.tabs.len() > 1 { 34.0_f32 } else { 1.0 };
        sep_h + 38.0 // tab-bar/separator + filter_bar
    }

    /// Convert a screen (cursor_x, cursor_y) into (line_index, char_col).
    /// Uses calibrated log_area_top if available (set by LineHovered).
    fn cursor_to_line_col(&self, cursor_x: f32, cursor_y: f32) -> Option<(usize, usize)> {
        let tab = self.tab()?;
        let gutter_off = self.gutter_offset();
        if cursor_x < gutter_off { return None; }

        let line_h = virtual_list::line_height_for_font(self.font_size);
        let top    = self.log_area_top_cal.unwrap_or_else(|| self.log_area_top());
        if cursor_y < top { return None; }
        let row_idx = ((cursor_y - top) / line_h) as usize;
        if row_idx >= self.viewport_lines { return None; }
        let view_idx = tab.scroll_offset + row_idx;

        let src = if tab.jump_source_line.is_some() {
            if view_idx >= tab.total_lines() { return None; }
            view_idx
        } else {
            tab.view_rows.get(view_idx).map(|vr| vr.src())?
        };

        let char_w     = self.font_size * 0.6;
        let col_screen = ((cursor_x - gutter_off).max(0.0) / char_w) as usize;
        let col        = col_screen + tab.h_scroll_offset;
        Some((src, col))
    }

    /// Character count for a given source line (after pipeline transforms).
    fn get_line_char_count(&self, line: usize) -> usize {
        let Some(tab) = self.tab() else { return 0; };
        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
        let Some(text) = reader.get_line(line) else { return 0; };
        let display = tab.pipeline.apply_text_transforms(text);
        display.chars().count()
    }

    /// Find the start of the previous word from (line, col).
    fn word_left(&self, line: usize, col: usize) -> (usize, usize) {
        let Some(tab) = self.tab() else { return (line, col); };
        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
        let Some(text) = reader.get_line(line) else { return (line, col); };
        let display = tab.pipeline.apply_text_transforms(text);
        let chars: Vec<char> = display.chars().collect();
        if col == 0 {
            // Wrap to end of previous line
            if let Some(vi) = self.view_row_idx_for_src(line) {
                if vi > 0 {
                    if let Some(prev_src) = self.src_for_view_row(vi - 1) {
                        return (prev_src, self.get_line_char_count(prev_src));
                    }
                }
            }
            return (line, 0);
        }
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut p = col.min(chars.len());
        // Skip whitespace backwards
        while p > 0 && chars[p - 1].is_whitespace() { p -= 1; }
        if p == 0 { return (line, 0); }
        // Skip word chars or non-word chars backwards
        let target_is_word = is_word(chars[p - 1]);
        while p > 0 && is_word(chars[p - 1]) == target_is_word { p -= 1; }
        (line, p)
    }

    /// Find the start of the next word from (line, col).
    fn word_right(&self, line: usize, col: usize) -> (usize, usize) {
        let Some(tab) = self.tab() else { return (line, col); };
        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
        let Some(text) = reader.get_line(line) else { return (line, col); };
        let display = tab.pipeline.apply_text_transforms(text);
        let chars: Vec<char> = display.chars().collect();
        let len = chars.len();
        if col >= len {
            // Wrap to start of next line
            let total_view = tab.total_visible_lines();
            if let Some(vi) = self.view_row_idx_for_src(line) {
                if vi + 1 < total_view {
                    if let Some(next_src) = self.src_for_view_row(vi + 1) {
                        return (next_src, 0);
                    }
                }
            }
            return (line, len);
        }
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut p = col;
        // Skip current word/non-word chars forward
        let target_is_word = is_word(chars[p]);
        while p < len && is_word(chars[p]) == target_is_word { p += 1; }
        // Skip whitespace forward
        while p < len && chars[p].is_whitespace() { p += 1; }
        (line, p)
    }

    /// Find word boundaries around (line, col). Returns (start_col, end_col).
    fn word_at(&self, line: usize, col: usize) -> (usize, usize) {
        let Some(tab) = self.tab() else { return (col, col + 1); };
        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
        let Some(text) = reader.get_line(line) else { return (col, col + 1); };
        let display = tab.pipeline.apply_text_transforms(text);
        let chars: Vec<char> = display.chars().collect();
        if chars.is_empty() { return (0, 0); }
        if col >= chars.len() { return (chars.len(), chars.len()); }
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let target_is_word = is_word(chars[col]);
        let mut start = col;
        while start > 0 && is_word(chars[start - 1]) == target_is_word { start -= 1; }
        let mut end = col;
        while end < chars.len() && is_word(chars[end]) == target_is_word { end += 1; }
        (start, end)
    }

    /// Get view_row index for a source line number.
    fn view_row_idx_for_src(&self, src: usize) -> Option<usize> {
        let tab = self.tab()?;
        if tab.jump_source_line.is_some() {
            if src < tab.total_lines() { Some(src) } else { None }
        } else {
            tab.view_rows.iter().position(|vr| vr.src() == src)
        }
    }

    /// Get source line for a view_row index.
    fn src_for_view_row(&self, view_idx: usize) -> Option<usize> {
        let tab = self.tab()?;
        if tab.jump_source_line.is_some() {
            if view_idx < tab.total_lines() { Some(view_idx) } else { None }
        } else {
            tab.view_rows.get(view_idx).map(|vr| vr.src())
        }
    }

    /// Scroll so that the cursor (focus line) is visible.
    fn ensure_cursor_visible(&mut self) {
        let Some(sel) = self.text_sel else { return; };
        let Some(view_idx) = self.view_row_idx_for_src(sel.focus_line) else { return; };
        let vp = self.viewport_lines;
        let Some(tab) = self.tab_mut() else { return; };
        if view_idx < tab.scroll_offset {
            tab.scroll_offset = view_idx;
        } else if view_idx >= tab.scroll_offset + vp {
            tab.scroll_offset = view_idx.saturating_sub(vp.saturating_sub(1));
        }
    }

    /// Move cursor to (line, col). If `extend` is true, keep anchor and move focus only.
    fn move_cursor_to(&mut self, line: usize, col: usize, extend: bool) {
        if extend {
            if let Some(sel) = &mut self.text_sel {
                sel.focus_line = line;
                sel.focus_col  = col;
            } else {
                self.text_sel = Some(TextSel {
                    anchor_line: line, anchor_col: col,
                    focus_line: line, focus_col: col,
                });
            }
            // Clear row-level highlight when extending selection
            if let Some(t) = self.tab_mut() { t.selected_line = None; }
        } else {
            self.text_sel = Some(TextSel {
                anchor_line: line, anchor_col: col,
                focus_line: line, focus_col: col,
            });
        }
        self.ensure_cursor_visible();
    }

    fn recalc_viewport(&mut self) {
        let sep_h    = if self.tabs.len() > 1 { 34.0_f32 } else { 1.0 };
        let pipeline_w = self.tab().map(|t| if t.pipeline_open { 260.0_f32 } else { 0.0 }).unwrap_or(0.0);
        let _ = pipeline_w; // used implicitly through window_height
        // search_bar removed; filter_bar (~38px) + status_bar (24px) + separator
        let overhead = sep_h + 38.0 + 24.0;
        let available = (self.window_height - overhead).max(100.0);
        self.viewport_lines = virtual_list::visible_lines_for_font(available, self.font_size);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.scroll_offset = virtual_list::clamp_offset(
                tab.scroll_offset, tab.total_visible_lines(), self.viewport_lines,
            );
        }
    }

    fn scroll_to_cursor_y(&mut self) {
        let toolbar_h: f32 = 38.0;  // filter_bar height (search_bar removed)
        let status_h:  f32 = 24.0;
        let results_h: f32 = self.tab()
            .map(|t| if t.search_results.is_empty() && !t.search_in_progress { 0.0 } else { 226.0 })
            .unwrap_or(0.0);
        let sb_height   = (self.window_height - toolbar_h - status_h - results_h).max(1.0);
        let fraction    = ((self.scrollbar_hover_y - toolbar_h) / sb_height).clamp(0.0, 1.0);
        let viewport_vp = self.viewport_lines;
        if let Some(tab) = self.tab_mut() {
            let max_scroll = tab.total_visible_lines().saturating_sub(viewport_vp);
            let target  = (fraction * max_scroll as f32) as usize;
            let clamped = virtual_list::clamp_offset(target, tab.total_visible_lines(), viewport_vp);
            tab.scroll_offset = clamped;
        }
    }

    fn setup_watcher(&mut self) {
        drop(self.watch_rx.take());
        drop(self._watcher.take());
        let Some(tab) = self.tab() else { return; };
        let path = tab.open_file.path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        match notify::RecommendedWatcher::new(
            move |evt: notify::Result<notify::Event>| { let _ = tx.send(evt); },
            notify::Config::default(),
        ) {
            Ok(mut w) => {
                if notify::Watcher::watch(&mut w, &path, notify::RecursiveMode::NonRecursive).is_ok() {
                    self._watcher = Some(w);
                    self.watch_rx = Some(rx);
                }
            }
            Err(_) => {}
        }
    }

    fn push_search_history(&mut self, query: String) {
        if query.is_empty() { return; }
        self.search_history.retain(|q| q != &query);
        self.search_history.insert(0, query);
        self.search_history.truncate(50);
    }

    /// Trigger a pipeline run (or bypass if no filter layers). Clears any stage preview.
    fn trigger_pipeline(&mut self) {
        {
            let Some(tab) = self.tab_mut() else { return; };
            tab.pipeline_preview_to = None;   // editing a layer exits preview mode
            if !tab.pipeline.has_active_filter_layers() {
                // No filter layers — all lines pass
                let all: Vec<usize> = (0..tab.total_lines()).collect();
                tab.last_pipeline_output = all.clone();
                tab.rebuild_view_rows_from_filter(all);
                tab.pipeline_stale = false;
            } else {
                let config = tab.pipeline.to_config();
                tab.pipeline_handle.run(config);
                tab.pipeline_stale = true;
            }
        }
        self.recalc_viewport();
    }

    // ── Command palette ───────────────────────────────────────────────────────

    fn all_palette_cmds(&self) -> Vec<PaletteCmd> {
        let has_file  = !self.tabs.is_empty();
        let tail_lbl  = if self.tail_mode { "Disable Tail Mode (Live)" } else { "Enable Tail Mode (Live)" };
        let wrap_lbl  = if self.line_wrap  { "Disable Line Wrap" } else { "Enable Line Wrap" };

        let mut cmds = vec![
            PaletteCmd { label: "Open File".into(),          shortcut: "Ctrl+O", action: Message::FileOpen },
            PaletteCmd { label: "Reload File".into(),        shortcut: "F5",     action: Message::ReloadActiveFile },
            PaletteCmd { label: "Jump to Line".into(),       shortcut: "Ctrl+G", action: Message::JumpOpen },
            PaletteCmd { label: "Toggle Settings".into(),    shortcut: "",       action: Message::ToggleSettings },
            PaletteCmd { label: "Toggle Info Panel".into(),  shortcut: "",       action: Message::ToggleInfoPanel },
            PaletteCmd { label: "Zoom In".into(),            shortcut: "Ctrl++", action: Message::ZoomIn },
            PaletteCmd { label: "Zoom Out".into(),           shortcut: "Ctrl+-", action: Message::ZoomOut },
            PaletteCmd { label: "Reset Zoom".into(),         shortcut: "Ctrl+0", action: Message::ZoomReset },
            PaletteCmd { label: wrap_lbl.into(),             shortcut: "",       action: Message::WrapToggle },
            PaletteCmd { label: tail_lbl.into(),             shortcut: "",       action: Message::TailToggle },
            PaletteCmd { label: "Go to Top".into(),          shortcut: "Ctrl+Home", action: Message::ScrollTo(0) },
            PaletteCmd { label: "Go to Bottom".into(),       shortcut: "Ctrl+End",  action: Message::GoToBottom },
            PaletteCmd { label: "Next Bookmark".into(),      shortcut: "F2",     action: Message::NextBookmark },
            PaletteCmd { label: "Prev Bookmark".into(),      shortcut: "⇧F2",    action: Message::PrevBookmark },
            PaletteCmd { label: "Toggle Pipeline".into(),    shortcut: "",       action: Message::TogglePipeline },
        ];
        if has_file {
            cmds.push(PaletteCmd { label: "Clear Search & Filters".into(), shortcut: "", action: Message::Clear });
            cmds.push(PaletteCmd { label: "Export Results".into(),          shortcut: "", action: Message::Export });
            cmds.push(PaletteCmd { label: "Add Highlight (current search)".into(), shortcut: "", action: Message::AddHighlight });
        }
        for &t in AppTheme::all() {
            cmds.push(PaletteCmd { label: format!("Theme: {}", t.name()), shortcut: "", action: Message::SetTheme(t) });
        }
        for path in self.recent_files.iter().take(10) {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            cmds.push(PaletteCmd {
                label:    format!("Open Recent: {}", name),
                shortcut: "",
                action:   Message::FileDrop(path.clone()),
            });
        }
        for q in self.search_history.iter().take(10) {
            cmds.push(PaletteCmd {
                label:    format!("History: {}", q),
                shortcut: "",
                action:   Message::SearchQueryChanged(q.clone()),
            });
        }
        cmds
    }

    pub fn filtered_palette_cmds(&self) -> Vec<PaletteCmd> {
        let all = self.all_palette_cmds();
        if self.palette_query.is_empty() { return all; }
        let q = self.palette_query.to_lowercase();
        all.into_iter().filter(|c| c.label.to_lowercase().contains(&q)).collect()
    }

    // ── Update ────────────────────────────────────────────────────────────────

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {

            // ── File open / drop ──────────────────────────────────────────────
            Message::FileOpen => {
                return Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Open Log File")
                            .add_filter("Log files", &["log", "txt", "json", "csv", "gz"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        match handle {
                            Some(h) => load_file(h.path().to_path_buf()).await,
                            None    => Err("Cancelled".to_string()),
                        }
                    },
                    Message::FileLoaded,
                );
            }

            Message::FileDrop(path) => {
                self.recent_files_open = false;
                return Task::perform(load_file(path), Message::FileLoaded);
            }

            Message::FileLoaded(result) => {
                if let Ok((path, gz_data)) = result {
                    if let Some(tab) = build_tab(path.clone(), gz_data) {
                        self.tabs.push(tab);
                        self.active_tab = self.tabs.len() - 1;
                        self.recent_files.retain(|p| p != &path);
                        self.recent_files.insert(0, path);
                        self.recent_files.truncate(10);
                        save_recent_files(&self.recent_files);
                        self.setup_watcher();
                        self.recalc_viewport();
                    }
                }
            }

            // ── File reload ───────────────────────────────────────────────────
            Message::ReloadActiveFile => {
                let Some(tab) = self.tab() else { return Task::none(); };
                let path = tab.open_file.path.clone();
                return Task::perform(load_file(path), Message::FileReloaded);
            }

            Message::FileReloaded(result) => {
                let Ok((path, gz_data)) = result else { return Task::none(); };
                let ai = self.active_tab;
                let Some(tab) = self.tabs.get_mut(ai) else { return Task::none(); };
                let Ok(file_map) = FileMap::open(&path) else { return Task::none(); };
                let data_arc: Arc<dyn AsRef<[u8]> + Send + Sync> = match gz_data {
                    None    => {
                        let mmap_arc = file_map.clone_mmap_arc();
                        let bytes: &[u8] = mmap_arc.as_ref().as_ref();
                        if std::str::from_utf8(bytes).is_ok() {
                            mmap_arc
                        } else {
                            Arc::new(ensure_utf8(bytes.to_vec()))
                        }
                    }
                    Some(v) => Arc::new(ensure_utf8(v)),
                };
                let line_index = LineIndex::build(data_arc.as_ref().as_ref());
                let line_count = line_index.line_count();
                let mut offsets = Vec::with_capacity(line_count + 1);
                for i in 0..=line_count { if let Some(o) = line_index.offset(i) { offsets.push(o); } }
                let file_len = data_arc.as_ref().as_ref().len() as u64;
                if offsets.last().copied() != Some(file_len) { offsets.push(file_len); }
                let offsets_arc      = Arc::new(offsets);
                let search_handle    = spawn_search_worker(data_arc.clone(), offsets_arc.clone());
                let pipeline_handle  = spawn_pipeline_worker(data_arc.clone(), offsets_arc.clone());
                tab.search_handle.cancel();
                tab.open_file         = OpenFile { file_map, line_index, path };
                tab.file_data_arc     = data_arc;
                tab.line_offsets_arc  = offsets_arc;
                tab.search_handle     = search_handle;
                tab.pipeline_handle   = pipeline_handle;
                tab.search_results.clear();
                tab.search_result_set.clear();
                tab.selected_result   = None;
                tab.search_in_progress = false;
                tab.compiled_search_regex = None;
                // Reset pipeline view state
                tab.context_expanded.clear();
                tab.jump_source_line = None;
                let all: Vec<usize> = (0..line_count).collect();
                tab.last_pipeline_output = all.clone();
                tab.view_rows = all.iter().map(|&i| ViewRow::Line(i)).collect();
                tab.pipeline_stale = false;
                // Re-run search if there was one
                if !tab.search_query.is_empty() {
                    tab.compiled_search_regex = regex::Regex::new(&tab.search_query).ok();
                    let q = tab.search_query.clone();
                    tab.search_handle.search(q);
                    tab.search_in_progress = true;
                }
                let total = self.tabs[ai].total_visible_lines();
                if self.tail_mode {
                    self.tabs[ai].scroll_offset = total.saturating_sub(self.viewport_lines);
                }
                // Re-trigger pipeline if there are filter layers
                self.trigger_pipeline();
                self.recalc_viewport();
            }

            // ── Tail mode / file watching ─────────────────────────────────────
            Message::TailToggle => {
                self.tail_mode = !self.tail_mode;
                if self.tail_mode { return self.update(Message::GoToBottom); }
            }

            Message::PollFileChange => {
                if let Some(rx) = &self.watch_rx {
                    let mut changed = false;
                    while rx.try_recv().is_ok() { changed = true; }
                    if changed && self.tail_mode {
                        return self.update(Message::ReloadActiveFile);
                    }
                }
            }

            // ── Scroll ───────────────────────────────────────────────────────
            Message::ScrollTo(line) => {
                let c = self.clamp_scroll(line);
                if let Some(t) = self.tab_mut() { t.scroll_offset = c; }
            }
            Message::ScrollBy(delta) => {
                let cur = self.tab().map(|t| t.scroll_offset).unwrap_or(0);
                let new = (cur as i64 + delta).max(0) as usize;
                let c = self.clamp_scroll(new);
                if let Some(t) = self.tab_mut() { t.scroll_offset = c; }
            }
            Message::GoToBottom => {
                let total = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                return self.update(Message::ScrollTo(total));
            }
            Message::MouseScrolled(delta) => {
                // Any scroll dismisses the right-click context menu
                self.context_menu_line = None;
                if self.modifiers.control() {
                    if delta > 0.0 { return self.update(Message::ZoomIn); }
                    else if delta < 0.0 { return self.update(Message::ZoomOut); }
                } else if self.modifiers.shift() && !self.line_wrap {
                    // Shift+wheel = horizontal scroll (6 chars per notch)
                    let chars = (delta * 6.0).round() as i64;
                    if chars != 0 { return self.update(Message::HScrollBy(-chars)); }
                } else {
                    let lines = (delta * 3.0).round() as i64;
                    if lines != 0 { return self.update(Message::ScrollBy(-lines)); }
                }
            }

            // ── Search ───────────────────────────────────────────────────────
            Message::SearchQueryChanged(query) => {
                self.history_cursor = None;
                self.history_temp_query.clear();
                if let Some(t) = self.tab_mut() { t.search_query = query; }
            }
            Message::SearchSubmit => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if tab.search_query.is_empty() { tab.clear_search(); return Task::none(); }
                tab.compiled_search_regex = regex::Regex::new(&tab.search_query).ok();
                tab.search_handle.cancel();
                tab.search_results.clear();
                tab.search_result_set.clear();
                tab.selected_result    = None;
                tab.search_in_progress = true;
                let q = tab.search_query.clone();
                tab.search_handle.search(q.clone());
                self.push_search_history(q);
                self.history_cursor = None;
            }
            Message::PollSearchResults => {
                for tab in &mut self.tabs {
                    if !tab.search_in_progress { continue; }
                    for resp in tab.search_handle.try_recv_all() {
                        match resp {
                            SearchResponse::Batch(batch) => {
                                for r in batch {
                                    tab.search_result_set.insert(r.line_number);
                                    tab.search_results.push((r.line_number, r.line_text));
                                }
                            }
                            SearchResponse::Complete(_)
                            | SearchResponse::Cancelled
                            | SearchResponse::Error(_) => { tab.search_in_progress = false; }
                        }
                    }
                }
            }
            Message::ResultClicked(idx) => {
                let vp = self.viewport_lines;
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                tab.selected_result = Some(idx);
                if let Some((line_num, _)) = tab.search_results.get(idx) {
                    let target  = line_num.saturating_sub(vp / 2);
                    let clamped = virtual_list::clamp_offset(target, tab.total_visible_lines(), vp);
                    tab.scroll_offset = clamped;
                }
            }

            // ── Clear ────────────────────────────────────────────────────────
            Message::Clear => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.clear_search();
                    tab.scroll_offset     = 0;
                    tab.hidden_levels.clear();
                    tab.line_filter_query.clear();
                    tab.selected_line     = None;
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.history_cursor = None;
                self.history_temp_query.clear();
            }

            // ── Export ───────────────────────────────────────────────────────
            Message::Export => {
                let Some(tab) = self.tab() else { return Task::none(); };
                let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
                let text = if !tab.search_results.is_empty() {
                    tab.search_results.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n")
                } else if !tab.view_rows.is_empty() {
                    tab.view_rows.iter()
                        .filter_map(|row| {
                            if let ViewRow::Line(i) = row {
                                reader.get_line(*i)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>().join("\n")
                } else {
                    (0..reader.line_count()).filter_map(|i| reader.get_line(i)).collect::<Vec<_>>().join("\n")
                };
                return Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Export Lines")
                            .add_filter("Log files", &["log", "txt"])
                            .add_filter("All files", &["*"])
                            .save_file().await;
                        match handle {
                            Some(h) => tokio::fs::write(h.path(), text.as_bytes()).await.map_err(|e| e.to_string()),
                            None    => Err("Cancelled".to_string()),
                        }
                    },
                    Message::ExportSaved,
                );
            }
            Message::ExportSaved(_) => {}

            // ── Window ───────────────────────────────────────────────────────
            Message::WindowResized(size) => {
                self.window_height = size.height;
                self.recalc_viewport();
            }

            // ── Keyboard ─────────────────────────────────────────────────────
            Message::KeyPressed(key, modifiers) => {
                let shift    = modifiers.shift();
                let ctrl     = modifiers.control();
                let has_file = !self.tabs.is_empty();
                let has_cursor = self.text_sel.is_some() && has_file;

                match key {
                Key::Named(keyboard::key::Named::ArrowLeft) => {
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        if ctrl {
                            // Ctrl+Left / Ctrl+Shift+Left: word-level movement
                            let (nl, nc) = self.word_left(sel.focus_line, sel.focus_col);
                            self.move_cursor_to(nl, nc, shift);
                        } else if !shift && !sel.is_empty() {
                            // Collapse selection to its start
                            let ((lo_l, lo_c), _) = sel.normalised();
                            self.move_cursor_to(lo_l, lo_c, false);
                        } else {
                            let (nl, nc) = if sel.focus_col > 0 {
                                (sel.focus_line, sel.focus_col - 1)
                            } else {
                                let prev_src = self.view_row_idx_for_src(sel.focus_line)
                                    .and_then(|vi| if vi > 0 { self.src_for_view_row(vi - 1) } else { None })
                                    .unwrap_or(sel.focus_line);
                                if prev_src != sel.focus_line {
                                    (prev_src, self.get_line_char_count(prev_src))
                                } else {
                                    (sel.focus_line, 0)
                                }
                            };
                            self.move_cursor_to(nl, nc, shift);
                        }
                    }
                }
                Key::Named(keyboard::key::Named::ArrowRight) => {
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        if ctrl {
                            // Ctrl+Right / Ctrl+Shift+Right: word-level movement
                            let (nl, nc) = self.word_right(sel.focus_line, sel.focus_col);
                            self.move_cursor_to(nl, nc, shift);
                        } else if !shift && !sel.is_empty() {
                            // Collapse selection to its end
                            let (_, (hi_l, hi_c)) = sel.normalised();
                            self.move_cursor_to(hi_l, hi_c, false);
                        } else {
                            let line_len = self.get_line_char_count(sel.focus_line);
                            let (nl, nc) = if sel.focus_col < line_len {
                                (sel.focus_line, sel.focus_col + 1)
                            } else {
                                let total_view = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                                let next_src = self.view_row_idx_for_src(sel.focus_line)
                                    .and_then(|vi| if vi + 1 < total_view { self.src_for_view_row(vi + 1) } else { None });
                                match next_src {
                                    Some(ns) => (ns, 0),
                                    None     => (sel.focus_line, sel.focus_col),
                                }
                            };
                            self.move_cursor_to(nl, nc, shift);
                        }
                    }
                }
                Key::Named(keyboard::key::Named::ArrowDown) => {
                    if ctrl { return self.update(Message::HistoryNext); }
                    if self.palette_open { return self.update(Message::PaletteMove(1)); }
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        let total_view = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                        if let Some(vi) = self.view_row_idx_for_src(sel.focus_line) {
                            let new_vi = (vi + 1).min(total_view.saturating_sub(1));
                            if let Some(new_src) = self.src_for_view_row(new_vi) {
                                self.move_cursor_to(new_src, sel.focus_col, shift);
                            }
                        }
                    } else {
                        return self.update(Message::ScrollBy(1));
                    }
                }
                Key::Named(keyboard::key::Named::ArrowUp) => {
                    if ctrl { return self.update(Message::HistoryPrev); }
                    if self.palette_open { return self.update(Message::PaletteMove(-1)); }
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        if let Some(vi) = self.view_row_idx_for_src(sel.focus_line) {
                            let new_vi = vi.saturating_sub(1);
                            if let Some(new_src) = self.src_for_view_row(new_vi) {
                                self.move_cursor_to(new_src, sel.focus_col, shift);
                            }
                        }
                    } else {
                        return self.update(Message::ScrollBy(-1));
                    }
                }
                Key::Named(keyboard::key::Named::Home) => {
                    if ctrl {
                        // Ctrl+Home / Ctrl+Shift+Home: move to file start
                        if has_file {
                            if let Some(first_src) = self.src_for_view_row(0) {
                                self.move_cursor_to(first_src, 0, shift);
                            } else {
                                return self.update(Message::ScrollTo(0));
                            }
                        } else {
                            return self.update(Message::ScrollTo(0));
                        }
                    } else if has_cursor {
                        // Home / Shift+Home: move to start of line
                        let src = self.text_sel.unwrap().focus_line;
                        self.move_cursor_to(src, 0, shift);
                    } else {
                        return self.update(Message::ScrollTo(0));
                    }
                }
                Key::Named(keyboard::key::Named::End) => {
                    if ctrl {
                        // Ctrl+End / Ctrl+Shift+End: move to file end
                        if has_file {
                            let total_view = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                            if let Some(last_src) = self.src_for_view_row(total_view.saturating_sub(1)) {
                                let line_len = self.get_line_char_count(last_src);
                                self.move_cursor_to(last_src, line_len, shift);
                            } else {
                                return self.update(Message::GoToBottom);
                            }
                        } else {
                            return self.update(Message::GoToBottom);
                        }
                    } else if has_cursor {
                        // End / Shift+End: move to end of line
                        let src = self.text_sel.unwrap().focus_line;
                        let line_len = self.get_line_char_count(src);
                        self.move_cursor_to(src, line_len, shift);
                    } else {
                        return self.update(Message::GoToBottom);
                    }
                }
                Key::Named(keyboard::key::Named::PageDown) => {
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        let jump = self.viewport_lines.saturating_sub(2);
                        let total_view = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                        if let Some(vi) = self.view_row_idx_for_src(sel.focus_line) {
                            let new_vi = (vi + jump).min(total_view.saturating_sub(1));
                            if let Some(new_src) = self.src_for_view_row(new_vi) {
                                self.move_cursor_to(new_src, sel.focus_col, shift);
                            }
                        }
                    } else {
                        let d = self.viewport_lines.saturating_sub(2) as i64;
                        return self.update(Message::ScrollBy(d));
                    }
                }
                Key::Named(keyboard::key::Named::PageUp) => {
                    if has_cursor {
                        let sel = self.text_sel.unwrap();
                        let jump = self.viewport_lines.saturating_sub(2);
                        if let Some(vi) = self.view_row_idx_for_src(sel.focus_line) {
                            let new_vi = vi.saturating_sub(jump);
                            if let Some(new_src) = self.src_for_view_row(new_vi) {
                                self.move_cursor_to(new_src, sel.focus_col, shift);
                            }
                        }
                    } else {
                        let d = -(self.viewport_lines.saturating_sub(2) as i64);
                        return self.update(Message::ScrollBy(d));
                    }
                }
                Key::Named(keyboard::key::Named::Insert) => {
                    self.block_cursor = !self.block_cursor;
                }
                Key::Named(keyboard::key::Named::F2) => {
                    if shift { return self.update(Message::PrevBookmark); }
                    return self.update(Message::NextBookmark);
                }
                Key::Named(keyboard::key::Named::F5) => {
                    if has_file { return self.update(Message::ReloadActiveFile); }
                }
                Key::Named(keyboard::key::Named::Escape) => {
                    if self.palette_open {
                        self.palette_open = false; self.palette_query.clear(); self.palette_selected = 0;
                    } else if self.jump_open {
                        self.jump_open = false; self.jump_input.clear();
                    } else if self.settings_open {
                        self.settings_open = false;
                    } else if self.text_sel.is_some() {
                        self.text_sel = None;
                        self.mouse_selecting = false;
                    } else if let Some(t) = self.tab_mut() {
                        if t.jump_source_line.is_some() {
                            t.jump_source_line = None;
                            let _ = self.recalc_viewport();
                        } else {
                            t.selected_line = None;
                        }
                    }
                }
                Key::Character(c) if ctrl => match c.as_str() {
                    "=" | "+" => return self.update(Message::ZoomIn),
                    "-"       => return self.update(Message::ZoomOut),
                    "0"       => return self.update(Message::ZoomReset),
                    "a"       => {
                        // Ctrl+A: select all visible lines
                        if has_file {
                            let total_view = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
                            if total_view > 0 {
                                let first_src = self.src_for_view_row(0).unwrap_or(0);
                                let last_src  = self.src_for_view_row(total_view - 1).unwrap_or(0);
                                let last_len  = self.get_line_char_count(last_src);
                                self.text_sel = Some(TextSel {
                                    anchor_line: first_src, anchor_col: 0,
                                    focus_line: last_src, focus_col: last_len,
                                });
                                if let Some(t) = self.tab_mut() { t.selected_line = None; }
                            }
                        }
                    }
                    "b"       => {
                        if let Some(n) = self.tab().and_then(|t| t.selected_line) {
                            return self.update(Message::ToggleBookmark(n));
                        }
                    }
                    "c"       => {
                        // If there's a non-empty character-level selection, copy it
                        if let Some(sel) = self.text_sel {
                            if !sel.is_empty() {
                                let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
                                if let Some(tab) = self.tab() {
                                    let reader = LineReader::new(
                                        tab.file_data_arc.as_ref().as_ref(),
                                        &tab.open_file.line_index,
                                    );
                                    let mut parts: Vec<String> = Vec::new();
                                    for line_n in lo_line..=hi_line {
                                        if let Some(raw) = reader.get_line(line_n) {
                                            let display = tab.pipeline.apply_text_transforms(raw);
                                            let chars: Vec<char> = display.chars().collect();
                                            let start = if line_n == lo_line { lo_col.min(chars.len()) } else { 0 };
                                            let end   = if line_n == hi_line { hi_col.min(chars.len()) } else { chars.len() };
                                            parts.push(chars[start..end].iter().collect());
                                        }
                                    }
                                    let text = parts.join("\n");
                                    if !text.is_empty() {
                                        return iced::clipboard::write(text);
                                    }
                                }
                            }
                        }
                        // Fallback: copy whole selected line
                        return self.update(Message::CopyToClipboard);
                    }
                    "p"       => return self.update(Message::PaletteOpen),
                    "g"       => return self.update(Message::JumpOpen),
                    "o"       => return self.update(Message::FileOpen),
                    "r"       => {
                        if !self.recent_files.is_empty() {
                            return self.update(Message::ToggleRecentFiles);
                        }
                    }
                    _         => {}
                },
                _ => {}
            }},

            // ── Scrollbar ────────────────────────────────────────────────────
            Message::ScrollbarClicked => { self.scrollbar_dragging = true; self.scroll_to_cursor_y(); }
            Message::ScrollbarReleased => {
                self.scrollbar_dragging = false;
                self.mouse_selecting    = false;
            }
            Message::CursorMoved(x, y) => {
                self.cursor_x = x;
                self.scrollbar_hover_y = y;
                if self.scrollbar_dragging { self.scroll_to_cursor_y(); }
                if self.mouse_selecting {
                    let gutter_off = self.gutter_offset();
                    let top = self.log_area_top_cal.unwrap_or_else(|| self.log_area_top());
                    let line_h = virtual_list::line_height_for_font(self.font_size);
                    let bottom = top + (self.viewport_lines as f32) * line_h;

                    if let Some((src, col)) = self.cursor_to_line_col(x, y) {
                        // Normal drag within viewport
                        if let Some(sel) = &mut self.text_sel {
                            if src != sel.anchor_line || col != sel.anchor_col {
                                if let Some(t) = self.tabs.get_mut(self.active_tab) {
                                    t.selected_line = None;
                                }
                            }
                            sel.focus_line = src;
                            sel.focus_col  = col;
                        }
                    } else if x >= gutter_off && (y < top || y > bottom) {
                        // Drag auto-scroll: cursor is above or below viewport
                        // (NOT triggered when cursor is in gutter — that's just lateral movement)
                        let char_w = self.font_size * 0.6;
                        let col_screen = ((x - gutter_off).max(0.0) / char_w) as usize;
                        let col = col_screen + self.tab().map(|t| t.h_scroll_offset).unwrap_or(0);
                        if y < top {
                            if let Some(tab) = self.tab_mut() {
                                if tab.scroll_offset > 0 { tab.scroll_offset -= 1; }
                            }
                            if let Some(new_src) = self.src_for_view_row(
                                self.tab().map(|t| t.scroll_offset).unwrap_or(0)
                            ) {
                                if let Some(sel) = &mut self.text_sel {
                                    sel.focus_line = new_src;
                                    sel.focus_col  = col;
                                    if let Some(t) = self.tabs.get_mut(self.active_tab) {
                                        t.selected_line = None;
                                    }
                                }
                            }
                        } else {
                            let vp = self.viewport_lines;
                            if let Some(tab) = self.tab_mut() {
                                let max_off = tab.total_visible_lines().saturating_sub(vp);
                                if tab.scroll_offset < max_off { tab.scroll_offset += 1; }
                            }
                            let last_vi = self.tab().map(|t| {
                                (t.scroll_offset + vp).min(t.total_visible_lines()).saturating_sub(1)
                            }).unwrap_or(0);
                            if let Some(new_src) = self.src_for_view_row(last_vi) {
                                if let Some(sel) = &mut self.text_sel {
                                    sel.focus_line = new_src;
                                    sel.focus_col  = col;
                                    if let Some(t) = self.tabs.get_mut(self.active_tab) {
                                        t.selected_line = None;
                                    }
                                }
                            }
                        }
                    }
                    // else: cursor is in gutter area — do nothing (don't extend selection)
                }
            }

            // ── Text selection ────────────────────────────────────────────────
            // Global left-button press: rich_text captures on_press so we
            // handle click-to-place-cursor via the subscription instead.
            // Uses hovered_line (set by mouse_area on_enter/on_move) for the
            // exact line — no pixel offset guessing required.
            Message::LeftMouseDown => {
                // Close context menu on any left click
                self.context_menu_line = None;
                if let Some(src) = self.hovered_line {
                    let gutter_off = self.gutter_offset();
                    let char_w     = self.font_size * 0.6;
                    let col_screen = ((self.cursor_x - gutter_off).max(0.0) / char_w) as usize;
                    let col        = col_screen + self.tab().map(|t| t.h_scroll_offset).unwrap_or(0);

                    // Detect double/triple clicks (within 500ms)
                    let now = Instant::now();
                    let is_repeat = self.last_click_time
                        .map_or(false, |t| now.duration_since(t).as_millis() < 500);
                    if is_repeat {
                        self.click_count += 1;
                    } else {
                        self.click_count = 1;
                    }
                    self.last_click_time = Some(now);

                    if self.click_count >= 3 {
                        // Triple-click: select entire line
                        let line_len = self.get_line_char_count(src);
                        self.text_sel = Some(TextSel {
                            anchor_line: src, anchor_col: 0,
                            focus_line: src, focus_col: line_len,
                        });
                        self.mouse_selecting = true;
                        if let Some(t) = self.tab_mut() { t.selected_line = None; }
                    } else if self.click_count == 2 {
                        // Double-click: select word
                        let (word_start, word_end) = self.word_at(src, col);
                        self.text_sel = Some(TextSel {
                            anchor_line: src, anchor_col: word_start,
                            focus_line: src, focus_col: word_end,
                        });
                        self.mouse_selecting = true;
                        if let Some(t) = self.tab_mut() { t.selected_line = None; }
                    } else if self.modifiers.shift() {
                        // Shift+click: extend selection from anchor to click point
                        if let Some(sel) = &mut self.text_sel {
                            sel.focus_line = src;
                            sel.focus_col  = col;
                        } else {
                            self.text_sel = Some(TextSel {
                                anchor_line: src, anchor_col: col,
                                focus_line: src, focus_col: col,
                            });
                        }
                        self.mouse_selecting = true;
                        if let Some(t) = self.tab_mut() { t.selected_line = None; }
                    } else {
                        // Single click: place cursor
                        self.text_sel = Some(TextSel {
                            anchor_line: src, anchor_col: col,
                            focus_line:  src, focus_col:  col,
                        });
                        self.mouse_selecting = true;
                        if let Some(t) = self.tab_mut() { t.selected_line = Some(src); }
                    }
                }
            }
            Message::TextSelUpdate(_cursor_x) => {
                // Now handled entirely in CursorMoved above; kept for API compat
            }
            Message::TextSelClear => {
                self.text_sel        = None;
                self.mouse_selecting = false;
            }
            // LineHovered: track which line the cursor is over (for LeftMouseDown to use).
            // Also calibrates the log_area_top offset for cursor_to_line_col.
            Message::LineHovered(src) => {
                self.hovered_line = Some(src);
                // Calibrate: compute actual log area top from known line + cursor_y
                if let Some(tab) = self.tab() {
                    let line_h = virtual_list::line_height_for_font(self.font_size);
                    // Find view_row index for this source line
                    let view_idx = if tab.jump_source_line.is_some() {
                        src
                    } else {
                        tab.view_rows.iter().position(|vr| vr.src() == src)
                            .unwrap_or(src)
                    };
                    let row_in_viewport = view_idx.saturating_sub(tab.scroll_offset);
                    let cal_top = self.scrollbar_hover_y - (row_in_viewport as f32 + 0.5) * line_h;
                    self.log_area_top_cal = Some(cal_top);
                }
                if self.mouse_selecting {
                    let gutter_off = self.gutter_offset();
                    let char_w     = self.font_size * 0.6;
                    let col_screen = ((self.cursor_x - gutter_off).max(0.0) / char_w) as usize;
                    let col        = col_screen + self.tab().map(|t| t.h_scroll_offset).unwrap_or(0);
                    if let Some(sel) = &mut self.text_sel {
                        if src != sel.anchor_line || col != sel.anchor_col {
                            if let Some(t) = self.tabs.get_mut(self.active_tab) {
                                t.selected_line = None;
                            }
                        }
                        sel.focus_line = src;
                        sel.focus_col  = col;
                    }
                }
            }

            // ── Filters ──────────────────────────────────────────────────────
            Message::ToggleLevelFilter(level) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    if tab.hidden_levels.contains(&level) { tab.hidden_levels.remove(&level); }
                    else { tab.hidden_levels.insert(level); }
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.recalc_viewport();
            }
            Message::LineFilterChanged(query) => {
                {
                    let is_regex = self.filter_is_regex;
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.line_filter_query = query.clone();
                    // Keep search_query in sync so pressing Enter triggers a search on this text
                    tab.search_query = query.clone();
                    tab.filter_regex = if is_regex && !query.is_empty() {
                        regex::Regex::new(&query).ok()
                    } else { None };
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.recalc_viewport();
            }
            Message::ToggleFilterRegex => {
                self.filter_is_regex = !self.filter_is_regex;
                let is_regex = self.filter_is_regex;
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.filter_regex = if is_regex && !tab.line_filter_query.is_empty() {
                        regex::Regex::new(&tab.line_filter_query).ok()
                    } else { None };
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.recalc_viewport();
            }
            Message::ClearFilters => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.hidden_levels.clear();
                    tab.line_filter_query.clear();
                    tab.filter_regex = None;
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.recalc_viewport();
            }

            // ── Zoom ─────────────────────────────────────────────────────────
            Message::ZoomIn    => { self.font_size = (self.font_size + 1.0).min(28.0); self.recalc_viewport(); }
            Message::ZoomOut   => { self.font_size = (self.font_size - 1.0).max(8.0);  self.recalc_viewport(); }
            Message::ZoomReset => { self.font_size = 16.0; self.recalc_viewport(); }
            Message::ModifiersChanged(mods) => { self.modifiers = mods; }

            // ── Theme / settings ─────────────────────────────────────────────
            Message::SetTheme(t)     => { self.app_theme = t; }
            Message::SetOpacity(v)   => { self.bg_opacity = v.clamp(0.0, 1.0); }
            Message::ToggleSettings  => { self.settings_open = !self.settings_open; }
            Message::ToggleInfoPanel => { self.info_panel_open = !self.info_panel_open; }
            Message::WrapToggle => {
                self.line_wrap = !self.line_wrap;
                // Reset horizontal scroll when toggling wrap
                if let Some(t) = self.tab_mut() { t.h_scroll_offset = 0; }
                self.recalc_viewport();
            }
            Message::HScrollBy(delta) => {
                if !self.line_wrap {
                    if let Some(t) = self.tab_mut() {
                        t.h_scroll_offset = (t.h_scroll_offset as i64 + delta).max(0) as usize;
                    }
                }
            }
            Message::HScrollTo(pos) => {
                if !self.line_wrap {
                    if let Some(t) = self.tab_mut() {
                        t.h_scroll_offset = pos.round().max(0.0) as usize;
                    }
                }
            }
            Message::ToggleColorLogLevels => { self.color_log_levels = !self.color_log_levels; }

            // ── Line selection ────────────────────────────────────────────────
            Message::LineClicked(_n) => {
                // Fires on mouse_area on_release. Do NOT reset text_sel here —
                // LeftMouseDown already placed the cursor/anchor on press, and
                // overwriting it on release would destroy any drag selection.
            }
            Message::CopyToClipboard => {
                if let Some(tab) = self.tab() {
                    if let Some(n) = tab.selected_line {
                        let reader = LineReader::new(tab.file_data_arc.as_ref().as_ref(), &tab.open_file.line_index);
                        if let Some(text) = reader.get_line(n) {
                            return iced::clipboard::write(text.to_string());
                        }
                    }
                }
            }

            // ── Jump to line ─────────────────────────────────────────────────
            Message::JumpOpen => {
                if !self.tabs.is_empty() {
                    self.jump_open = true;
                    self.jump_input.clear();
                    return text_input::focus(text_input::Id::new("jump_input"));
                }
            }
            Message::JumpInputChanged(s) => { self.jump_input = s; }
            Message::JumpSubmit => {
                if let Ok(n) = self.jump_input.trim().parse::<usize>() {
                    let line    = n.saturating_sub(1);
                    let clamped = self.clamp_scroll(line);
                    if let Some(t) = self.tab_mut() { t.scroll_offset = clamped; }
                }
                self.jump_open = false; self.jump_input.clear();
            }
            Message::JumpClose => { self.jump_open = false; self.jump_input.clear(); }

            // ── Command palette ──────────────────────────────────────────────
            Message::PaletteOpen => {
                self.palette_open = true; self.palette_query.clear(); self.palette_selected = 0;
                return text_input::focus(text_input::Id::new("palette_input"));
            }
            Message::PaletteClose => {
                self.palette_open = false; self.palette_query.clear(); self.palette_selected = 0;
            }
            Message::PaletteQueryChanged(q) => { self.palette_query = q; self.palette_selected = 0; }
            Message::PaletteMove(dir) => {
                let count = self.filtered_palette_cmds().len();
                if count == 0 { return Task::none(); }
                let sel = self.palette_selected as i64 + dir as i64;
                self.palette_selected = sel.rem_euclid(count as i64) as usize;
            }
            Message::PaletteSelect => {
                let cmds = self.filtered_palette_cmds();
                let sel  = self.palette_selected.min(cmds.len().saturating_sub(1));
                if let Some(cmd) = cmds.into_iter().nth(sel) {
                    let action = cmd.action;
                    self.palette_open = false; self.palette_query.clear(); self.palette_selected = 0;
                    return self.update(action);
                }
            }
            Message::PaletteRunIdx(idx) => {
                // Mouse-click on a specific row — run that row's action regardless of keyboard selection
                let cmds = self.filtered_palette_cmds();
                if let Some(cmd) = cmds.into_iter().nth(idx) {
                    let action = cmd.action;
                    self.palette_open = false; self.palette_query.clear(); self.palette_selected = 0;
                    return self.update(action);
                }
            }

            // ── Tabs ─────────────────────────────────────────────────────────
            Message::SwitchTab(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                    self.recalc_viewport();
                    self.setup_watcher();
                }
            }
            Message::CloseTab(idx) => {
                if idx < self.tabs.len() {
                    self.tabs[idx].search_handle.cancel();
                    self.tabs[idx].pipeline_handle.shutdown();
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                    if self.tabs.is_empty() { self.active_tab = 0; }
                    self.recalc_viewport();
                    self.setup_watcher();
                }
            }

            // ── Bookmarks ─────────────────────────────────────────────────────
            Message::ToggleBookmark(line_num) => {
                if let Some(tab) = self.tab_mut() {
                    if tab.bookmarks.contains(&line_num) {
                        tab.bookmarks.remove(&line_num);
                    } else {
                        tab.bookmarks.insert(line_num);
                    }
                }
            }
            Message::NextBookmark => {
                let Some(tab) = self.tab() else { return Task::none(); };
                if tab.bookmarks.is_empty() { return Task::none(); }
                let cur = tab.scroll_offset;
                let mut sorted: Vec<usize> = tab.bookmarks.iter().copied().collect();
                sorted.sort_unstable();
                let next = sorted.iter().find(|&&b| b > cur).copied().unwrap_or(sorted[0]);
                let c = self.clamp_scroll(next);
                if let Some(t) = self.tab_mut() { t.scroll_offset = c; }
            }
            Message::PrevBookmark => {
                let Some(tab) = self.tab() else { return Task::none(); };
                if tab.bookmarks.is_empty() { return Task::none(); }
                let cur = tab.scroll_offset;
                let mut sorted: Vec<usize> = tab.bookmarks.iter().copied().collect();
                sorted.sort_unstable();
                let prev = sorted.iter().rev().find(|&&b| b < cur).copied()
                    .unwrap_or(*sorted.last().unwrap());
                let c = self.clamp_scroll(prev);
                if let Some(t) = self.tab_mut() { t.scroll_offset = c; }
            }

            // ── Search history ────────────────────────────────────────────────
            Message::HistoryPrev => {
                if self.search_history.is_empty() { return Task::none(); }
                if self.history_cursor.is_none() {
                    self.history_temp_query = self.tab().map(|t| t.search_query.clone()).unwrap_or_default();
                }
                let next = match self.history_cursor {
                    None    => 0,
                    Some(i) => (i + 1).min(self.search_history.len() - 1),
                };
                self.history_cursor = Some(next);
                let q = self.search_history[next].clone();
                if let Some(t) = self.tab_mut() { t.search_query = q; }
            }
            Message::HistoryNext => {
                match self.history_cursor {
                    None    => {}
                    Some(0) => {
                        self.history_cursor = None;
                        let q = self.history_temp_query.clone();
                        if let Some(t) = self.tab_mut() { t.search_query = q; }
                    }
                    Some(i) => {
                        self.history_cursor = Some(i - 1);
                        let q = self.search_history[i - 1].clone();
                        if let Some(t) = self.tab_mut() { t.search_query = q; }
                    }
                }
            }

            // ── Extra highlights ──────────────────────────────────────────────
            Message::AddHighlight => {
                let is_regex = self.filter_is_regex;
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if tab.extra_highlights.len() >= HIGHLIGHT_COLORS.len() { return Task::none(); }
                if tab.line_filter_query.is_empty() { return Task::none(); }
                let idx     = tab.extra_highlights.len();
                let pattern = tab.line_filter_query.clone();
                // Build a regex: use literal escape when not in regex mode
                let re = if is_regex {
                    regex::Regex::new(&pattern).ok()
                } else {
                    regex::Regex::new(&regex::escape(&pattern)).ok()
                };
                let color = HIGHLIGHT_COLORS[idx];
                tab.extra_highlights.push(ExtraHighlight { pattern, regex: re, color });
            }
            Message::RemoveHighlight(idx) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if idx < tab.extra_highlights.len() { tab.extra_highlights.remove(idx); }
            }

            // ── Pipeline ─────────────────────────────────────────────────────
            Message::TogglePipeline => {
                if let Some(tab) = self.tab_mut() {
                    tab.pipeline_open = !tab.pipeline_open;
                }
            }

            Message::PipelinePreviewLayer(maybe_id) => {
                match maybe_id {
                    None => {
                        // Clear preview → re-run full pipeline
                        self.trigger_pipeline();
                    }
                    Some(id) => {
                        // Toggle: clicking the same layer again clears the preview
                        let already = self.tab().and_then(|t| t.pipeline_preview_to) == Some(id);
                        if already {
                            self.trigger_pipeline();
                        } else {
                            let Some(tab) = self.tab_mut() else { return Task::none(); };
                            tab.pipeline_preview_to = Some(id);
                            if tab.pipeline.has_filter_layers_up_to(id) {
                                let config = tab.pipeline.to_config_up_to(id);
                                tab.pipeline_handle.run(config);
                                tab.pipeline_stale = true;
                            } else {
                                // No filter layers up to this point — all lines pass
                                let all: Vec<usize> = (0..tab.total_lines()).collect();
                                tab.last_pipeline_output = all.clone();
                                tab.rebuild_view_rows_from_filter(all);
                                tab.pipeline_stale = false;
                            }
                            self.recalc_viewport();
                        }
                    }
                }
            }

            Message::PipelineAddFilter => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                let id = tab.pipeline.alloc_id();
                tab.pipeline.layers.push(crate::pipeline::UiLayer::new_filter(id, "", false));
            }

            Message::PipelineAddRewrite => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                let id = tab.pipeline.alloc_id();
                tab.pipeline.layers.push(crate::pipeline::UiLayer::new_rewrite(id, "", ""));
            }

            Message::PipelineAddMask => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                let id = tab.pipeline.alloc_id();
                tab.pipeline.layers.push(crate::pipeline::UiLayer::new_mask(id, "", "***"));
            }

            Message::PipelineRemoveLayer(id) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.pipeline.layers.retain(|ul| ul.layer.id != id);
                }
                self.trigger_pipeline();
            }

            Message::PipelineToggleLayer(id) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    if let Some(ul) = tab.pipeline.layers.iter_mut().find(|ul| ul.layer.id == id) {
                        ul.layer.enabled = !ul.layer.enabled;
                    }
                }
                self.trigger_pipeline();
            }

            Message::PipelineToggleLayerExclude(id) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    if let Some(ul) = tab.pipeline.layers.iter_mut().find(|ul| ul.layer.id == id) {
                        if let flash_core::LayerKind::Filter { exclude, .. } = &mut ul.layer.kind {
                            *exclude = !*exclude;
                        }
                    }
                }
                self.trigger_pipeline();
            }

            Message::PipelineMoveLayer(id, dir) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    if let Some(pos) = tab.pipeline.layers.iter().position(|ul| ul.layer.id == id) {
                        let new_pos = (pos as i64 + dir as i64)
                            .max(0)
                            .min(tab.pipeline.layers.len() as i64 - 1) as usize;
                        tab.pipeline.layers.swap(pos, new_pos);
                    }
                }
                self.trigger_pipeline();
            }

            Message::PipelineEditPattern(id, s) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if let Some(ul) = tab.pipeline.layers.iter_mut().find(|ul| ul.layer.id == id) {
                    ul.draft_pattern = s;
                }
            }

            Message::PipelineEditExtra(id, s) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if let Some(ul) = tab.pipeline.layers.iter_mut().find(|ul| ul.layer.id == id) {
                    ul.draft_extra = s;
                }
            }

            Message::PipelineCommitLayer(id) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if let Some(ul) = tab.pipeline.layers.iter_mut().find(|ul| ul.layer.id == id) {
                    let pattern = ul.draft_pattern.clone();
                    let extra   = ul.draft_extra.clone();
                    // Validate and apply
                    match &ul.layer.kind {
                        flash_core::LayerKind::Filter { exclude, .. } => {
                            let excl = *exclude;
                            match regex::Regex::new(&pattern) {
                                Ok(_) => {
                                    ul.layer.kind = flash_core::LayerKind::Filter {
                                        pattern, exclude: excl,
                                    };
                                    ul.parse_error  = None;
                                    ul.compiled_re  = None;
                                }
                                Err(e) => {
                                    ul.parse_error = Some(e.to_string());
                                    return Task::none();
                                }
                            }
                        }
                        flash_core::LayerKind::Rewrite { .. } => {
                            match regex::Regex::new(&pattern) {
                                Ok(re) => {
                                    ul.layer.kind  = flash_core::LayerKind::Rewrite {
                                        find: pattern, replacement: extra,
                                    };
                                    ul.compiled_re = Some(re);
                                    ul.parse_error = None;
                                }
                                Err(e) => {
                                    ul.parse_error = Some(e.to_string());
                                    return Task::none();
                                }
                            }
                        }
                        flash_core::LayerKind::Mask { .. } => {
                            match regex::Regex::new(&pattern) {
                                Ok(re) => {
                                    ul.layer.kind  = flash_core::LayerKind::Mask {
                                        pattern, mask_with: extra,
                                    };
                                    ul.compiled_re = Some(re);
                                    ul.parse_error = None;
                                }
                                Err(e) => {
                                    ul.parse_error = Some(e.to_string());
                                    return Task::none();
                                }
                            }
                        }
                    }
                }
                self.trigger_pipeline();
            }

            Message::PollPipeline => {
                let mut needs_recalc = false;
                for tab in &mut self.tabs {
                    if !tab.pipeline_stale { continue; }
                    for resp in tab.pipeline_handle.try_recv_all() {
                        match resp {
                            PipelineResponse::Complete(indices) => {
                                tab.last_pipeline_output = indices.clone();
                                tab.rebuild_view_rows_from_filter(indices);
                                tab.pipeline_stale = false;
                                needs_recalc = true;
                            }
                            PipelineResponse::Cancelled => {}
                            PipelineResponse::Error(_e) => {
                                tab.pipeline_stale = false;
                            }
                        }
                    }
                }
                if needs_recalc { self.recalc_viewport(); }
            }

            Message::ToggleContext(anchor) => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    if tab.context_expanded.contains(&anchor) {
                        tab.context_expanded.remove(&anchor);
                    } else {
                        tab.context_expanded.insert(anchor);
                    }
                    let output = tab.last_pipeline_output.clone();
                    tab.rebuild_view_rows_from_filter(output);
                }
                self.recalc_viewport();
            }

            Message::JumpToSource(n) => {
                let vp = self.viewport_lines;
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                tab.jump_source_line = Some(n);
                tab.selected_line    = Some(n);
                let total  = tab.total_lines();
                let target = n.saturating_sub(vp / 2);
                tab.scroll_offset = virtual_list::clamp_offset(target, total, vp);
            }

            Message::JumpToSourceClear => {
                {
                    let Some(tab) = self.tab_mut() else { return Task::none(); };
                    tab.jump_source_line = None;
                }
                self.recalc_viewport();
            }

            // ── Process stats ─────────────────────────────────────────────────
            Message::UpdateProcStats => {
                if let Ok(pid) = sysinfo::get_current_pid() {
                    self.sysinfo_sys.refresh_processes(
                        ProcessesToUpdate::Some(&[pid]),
                        false,
                    );
                    if let Some(proc) = self.sysinfo_sys.process(pid) {
                        self.proc_mem_mb  = proc.memory() as f64 / 1_048_576.0;
                        self.proc_cpu_pct = proc.cpu_usage() as f64;
                    }
                }
            }

            // ── Recent files panel ────────────────────────────────────────────
            Message::ToggleRecentFiles => {
                self.recent_files_open = !self.recent_files_open;
                self.context_menu_line = None;
            }

            // ── Right-click context menu ──────────────────────────────────────
            Message::RightClickLine(n) => {
                // Only set selected_line if there's no active text selection
                let has_text_sel = self.text_sel.map_or(false, |s| !s.is_empty());
                if !has_text_sel {
                    if let Some(t) = self.tab_mut() { t.selected_line = Some(n); }
                }
                self.context_menu_line = if self.context_menu_line == Some(n) { None } else { Some(n) };
                // Freeze cursor position now so the menu doesn't follow the mouse.
                if self.context_menu_line.is_some() {
                    self.context_menu_x = self.cursor_x;
                    self.context_menu_y = self.scrollbar_hover_y;
                }
            }
            Message::CloseContextMenu => { self.context_menu_line = None; }
            Message::CopyLine(n) => {
                // Line index carried in the message — no dependency on context_menu_line
                self.context_menu_line = None;
                if let Some(tab) = self.tab() {
                    let reader = LineReader::new(
                        tab.file_data_arc.as_ref().as_ref(),
                        &tab.open_file.line_index,
                    );
                    if let Some(raw) = reader.get_line(n) {
                        // Apply pipeline transforms so what's copied matches what's displayed
                        let display = tab.pipeline.apply_text_transforms(raw);
                        return iced::clipboard::write(display.into_owned());
                    }
                }
            }

            Message::CopySelection => {
                self.context_menu_line = None;
                if let Some(sel) = self.text_sel {
                    if !sel.is_empty() {
                        let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
                        if let Some(tab) = self.tab() {
                            let reader = LineReader::new(
                                tab.file_data_arc.as_ref().as_ref(),
                                &tab.open_file.line_index,
                            );
                            let mut parts: Vec<String> = Vec::new();
                            for line_n in lo_line..=hi_line {
                                if let Some(raw) = reader.get_line(line_n) {
                                    let display = tab.pipeline.apply_text_transforms(raw);
                                    let chars: Vec<char> = display.chars().collect();
                                    let start = if line_n == lo_line { lo_col.min(chars.len()) } else { 0 };
                                    let end   = if line_n == hi_line { hi_col.min(chars.len()) } else { chars.len() };
                                    parts.push(chars[start..end].iter().collect());
                                }
                            }
                            let text = parts.join("\n");
                            if !text.is_empty() {
                                return iced::clipboard::write(text);
                            }
                        }
                    }
                }
            }

            Message::Noop => {}
        }
        Task::none()
    }

    // ── View ──────────────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let p   = self.palette();
        let bg  = p.bg_primary;
        let bdr = p.border;

        let active = self.tabs.get(self.active_tab);

        let file_info = active.map(|t| {
            let size = t.file_data_arc.as_ref().as_ref().len() as u64;
            (t.file_name(), size, t.total_lines())
        });
        let _search_query      = active.map(|t| t.search_query.as_str()).unwrap_or("");
        let _result_count      = active.map(|t| t.search_results.len()).unwrap_or(0);
        let search_in_progress = active.map(|t| t.search_in_progress).unwrap_or(false);
        let has_file           = !self.tabs.is_empty();

        // ── Selected line text/level for info panel ───────────────────────────
        let selected_line_text: Option<String> = active.and_then(|t| {
            t.selected_line.and_then(|n| {
                let data = t.file_data_arc.as_ref().as_ref();
                let r    = LineReader::new(data, &t.open_file.line_index);
                r.get_line(n).map(|s| s.to_string())
            })
        });
        let selected_line_level = selected_line_text.as_deref()
            .and_then(flash_core::LogLevel::detect);

        // ── Activity bar (left, always visible) ───────────────────────────────
        let pipeline_open = active.map(|t| t.pipeline_open).unwrap_or(false);
        let activity_bar  = views::left_sidebar::view(
            pipeline_open, self.info_panel_open, has_file, p,
        );

        // (search_bar removed — filter bar is now the primary search/filter input)

        // ── Filter bar ────────────────────────────────────────────────────────
        let empty_hl: HashSet<flash_core::LogLevel> = HashSet::new();
        let hidden_levels    = active.map(|t| &t.hidden_levels).unwrap_or(&empty_hl);
        let line_filter_q    = active.map(|t| t.line_filter_query.as_str()).unwrap_or("");
        let extra_highlights = active.map(|t| t.extra_highlights.as_slice()).unwrap_or(&[]);

        let filter_count: Option<usize> = active.and_then(|t| {
            if t.pipeline.has_active_filter_layers()
               || !t.hidden_levels.is_empty()
               || !t.line_filter_query.is_empty()
            {
                let cnt = t.view_rows.iter().filter(|r| matches!(r, ViewRow::Line(_))).count();
                Some(cnt)
            } else {
                None
            }
        });

        let filter_bar = views::filter_bar::view(
            hidden_levels, line_filter_q, has_file, filter_count,
            extra_highlights, self.line_wrap, self.filter_is_regex, self.tail_mode,
            self.recent_files_open, !self.recent_files.is_empty(), p,
        );

        // ── Log view ──────────────────────────────────────────────────────────
        let reader            = active.map(|t| LineReader::new(t.file_data_arc.as_ref().as_ref(), &t.open_file.line_index));
        let total_visible     = active.map(|t| t.total_visible_lines()).unwrap_or(0);
        let scroll_offset     = active.map(|t| t.scroll_offset).unwrap_or(0);
        let compiled_regex    = active.and_then(|t| t.compiled_search_regex.as_ref());
        let empty_set: HashSet<usize> = HashSet::new();
        let search_result_set = active.map(|t| &t.search_result_set).unwrap_or(&empty_set);
        let selected_line     = active.and_then(|t| t.selected_line);
        let empty_bm: HashSet<usize> = HashSet::new();
        let bookmarks         = active.map(|t| &t.bookmarks).unwrap_or(&empty_bm);
        let search_results    = active.map(|t| t.search_results.as_slice()).unwrap_or(&[]);
        let view_rows         = active.map(|t| t.view_rows.as_slice());
        let jump_source       = active.and_then(|t| t.jump_source_line);
        let pipeline_stale      = active.map(|t| t.pipeline_stale).unwrap_or(false);
        let pipeline_preview_to = active.and_then(|t| t.pipeline_preview_to);
        let empty_ctx: HashSet<usize> = HashSet::new();
        let context_expanded    = active.map(|t| &t.context_expanded).unwrap_or(&empty_ctx);

        let h_scroll_offset = active.map(|t| t.h_scroll_offset).unwrap_or(0);
        let log_view = views::log_view::view(
            reader, scroll_offset, self.viewport_lines, total_visible,
            compiled_regex, search_result_set,
            view_rows, jump_source,
            self.font_size, selected_line, self.line_wrap,
            bookmarks, extra_highlights, search_results,
            context_expanded,
            active.map(|t| &t.pipeline),
            pipeline_stale,
            pipeline_preview_to,
            self.color_log_levels,
            h_scroll_offset,
            self.text_sel,
            self.block_cursor,
            p,
        );

        let selected_result = active.and_then(|t| t.selected_result);
        let results_panel = views::results_panel::view(
            search_results, selected_result, search_in_progress, compiled_regex, p,
        );

        // ── Right info panel ──────────────────────────────────────────────────
        let info_panel = views::info_panel::view(
            file_info.as_ref().map(|(n, _, _)| n.clone()),
            file_info.as_ref().map(|(_, s, _)| *s),
            file_info.as_ref().map(|(_, _, l)| *l),
            selected_line,
            selected_line_text,
            selected_line_level,
            scroll_offset,
            self.viewport_lines,
            total_visible,
            self.proc_mem_mb,
            self.proc_cpu_pct,
            p,
        );

        // ── Centre column ─────────────────────────────────────────────────────
        let hr = container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(bdr.into()), ..Default::default()
            });

        let mut center_col = column![];
        if self.tabs.len() > 1 {
            center_col = center_col.push(views::tab_bar::view(&self.tabs, self.active_tab, p));
        } else {
            center_col = center_col.push(hr);
        }
        center_col = center_col.push(filter_bar).push(log_view).push(results_panel);
        let center: Element<'_, Message> = center_col.width(Length::Fill).into();

        // ── 3-panel body: [activity | (pipeline) | center | (info)] ───────────
        let mut body_row = row![activity_bar];
        if let Some(tab) = active {
            if tab.pipeline_open {
                let pp = views::pipeline_panel::view(&tab.pipeline, tab.pipeline_stale, pipeline_preview_to, p);
                body_row = body_row.push(pp);
            }
        }
        body_row = body_row.push(center);
        if self.info_panel_open {
            body_row = body_row.push(info_panel);
        }
        let body: Element<'_, Message> = body_row
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let main_layout = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(bg.into()), ..Default::default()
            });

        if self.palette_open {
            let cmds    = self.filtered_palette_cmds();
            let overlay = views::command_palette::view(&self.palette_query, cmds, self.palette_selected, p);
            stack![main_layout, overlay].into()
        } else if self.jump_open {
            let total   = active.map(|t| t.total_lines()).unwrap_or(0);
            let overlay = views::jump_to_line::view(&self.jump_input, total, p);
            stack![main_layout, overlay].into()
        } else if self.settings_open {
            let overlay = views::settings_panel::view(self.app_theme, self.font_size, self.line_wrap, self.color_log_levels, self.bg_opacity, p);
            stack![main_layout, overlay].into()
        } else if self.recent_files_open {
            let overlay = views::recent_panel::view(&self.recent_files, p);
            stack![main_layout, overlay].into()
        } else if let Some(ctx_line) = self.context_menu_line {
            let has_sel = self.text_sel.map_or(false, |s| !s.is_empty());
            let overlay = views::context_menu::view(ctx_line, self.context_menu_x, self.context_menu_y, has_sel, p);
            stack![main_layout, overlay].into()
        } else {
            main_layout.into()
        }
    }

    pub fn theme(&self) -> Theme {
        let base = self.app_theme.iced_theme();
        if self.bg_opacity >= 1.0 {
            return base;
        }
        // Create a custom iced theme with transparent background (Windows Terminal style).
        // The iced Palette.background controls the window clear color — making it
        // transparent lets the desktop show through when the window compositor supports it.
        let mut iced_pal = base.palette();
        iced_pal.background = iced::Color {
            a: iced_pal.background.a * self.bg_opacity,
            ..iced_pal.background
        };
        iced::Theme::custom_with_fn(
            format!("{} ({}%)", self.app_theme.name(), (self.bg_opacity * 100.0).round()),
            iced_pal,
            |pal| iced::theme::palette::Extended::generate(pal),
        )
    }

    // ── Subscriptions ─────────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![];

        subs.push(keyboard::on_key_press(|key, modifiers| Some(Message::KeyPressed(key, modifiers))));
        subs.push(window::resize_events().map(|(_id, size)| Message::WindowResized(size)));

        subs.push(event::listen_with(|evt, status, _id| {
            if let event::Status::Ignored = status {
                if let iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) = evt {
                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. }  => y,
                        mouse::ScrollDelta::Pixels { y, .. } => y / 18.0,
                    };
                    return Some(Message::MouseScrolled(y));
                }
            }
            None
        }));

        subs.push(event::listen_with(|evt, _status, _id| match evt {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::CursorMoved(position.x, position.y)),
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Some(Message::LeftMouseDown),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::ScrollbarReleased),
            _ => None,
        }));

        subs.push(event::listen_with(|evt, _status, _id| {
            if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = evt {
                Some(Message::ModifiersChanged(mods))
            } else { None }
        }));

        subs.push(event::listen_with(|evt, _status, _id| {
            if let iced::Event::Window(window::Event::FileDropped(path)) = evt {
                Some(Message::FileDrop(path))
            } else { None }
        }));

        if self.tabs.iter().any(|t| t.search_in_progress) {
            subs.push(iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::PollSearchResults));
        }

        if self.tabs.iter().any(|t| t.pipeline_stale) {
            subs.push(iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::PollPipeline));
        }

        if self.watch_rx.is_some() {
            subs.push(iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::PollFileChange));
        }

        // Process stats — refresh every 2 seconds, only when the info panel is visible
        if self.info_panel_open && !self.tabs.is_empty() {
            subs.push(iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::UpdateProcStats));
        }

        Subscription::batch(subs)
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

async fn load_file(path: PathBuf) -> Result<(PathBuf, Option<Vec<u8>>), String> {
    if path.extension().and_then(|e| e.to_str()) == Some("gz") {
        let raw = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
        use flate2::read::GzDecoder;
        use std::io::Read;
        let mut dec = GzDecoder::new(&raw[..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).map_err(|e| e.to_string())?;
        Ok((path, Some(out)))
    } else {
        Ok((path, None))
    }
}

fn ensure_utf8(raw: Vec<u8>) -> Vec<u8> {
    if std::str::from_utf8(&raw).is_err() {
        String::from_utf8_lossy(&raw).into_owned().into_bytes()
    } else {
        raw
    }
}

fn build_tab(path: PathBuf, gz_data: Option<Vec<u8>>) -> Option<Tab> {
    let file_map = FileMap::open(&path).ok()?;
    let data_arc: Arc<dyn AsRef<[u8]> + Send + Sync> = match gz_data {
        None    => {
            // Check if the mmap data is valid UTF-8; if not, convert lossy
            // (handles BOM, Windows-1252, and other non-UTF-8 encodings)
            let mmap_arc = file_map.clone_mmap_arc();
            let bytes: &[u8] = mmap_arc.as_ref().as_ref();
            if std::str::from_utf8(bytes).is_ok() {
                mmap_arc
            } else {
                Arc::new(ensure_utf8(bytes.to_vec()))
            }
        }
        Some(v) => Arc::new(ensure_utf8(v)),
    };
    let line_index  = LineIndex::build(data_arc.as_ref().as_ref());
    let line_count  = line_index.line_count();
    let mut offsets = Vec::with_capacity(line_count + 1);
    for i in 0..=line_count { if let Some(o) = line_index.offset(i) { offsets.push(o); } }
    let file_len = data_arc.as_ref().as_ref().len() as u64;
    if offsets.last().copied() != Some(file_len) { offsets.push(file_len); }
    let offsets_arc     = Arc::new(offsets);
    let search_handle   = spawn_search_worker(data_arc.clone(), offsets_arc.clone());
    let pipeline_handle = spawn_pipeline_worker(data_arc.clone(), offsets_arc.clone());

    // Initially all lines pass (no filter layers)
    let last_pipeline_output: Vec<usize> = (0..line_count).collect();
    let view_rows: Vec<ViewRow> = last_pipeline_output.iter().map(|&i| ViewRow::Line(i)).collect();

    Some(Tab {
        open_file:             OpenFile { file_map, line_index, path },
        scroll_offset:         0,
        search_query:          String::new(),
        search_results:        Vec::new(),
        search_result_set:     HashSet::new(),
        selected_result:       None,
        search_handle,
        search_in_progress:    false,
        compiled_search_regex: None,
        file_data_arc:         data_arc,
        line_offsets_arc:      offsets_arc,
        hidden_levels:         HashSet::new(),
        line_filter_query:     String::new(),
        selected_line:         None,
        bookmarks:             HashSet::new(),
        extra_highlights:      Vec::new(),
        pipeline:              TransformPipeline::new(),
        pipeline_handle,
        pipeline_stale:        false,
        pipeline_open:         false,
        view_rows,
        last_pipeline_output,
        context_expanded:      HashSet::new(),
        jump_source_line:      None,
        pipeline_preview_to:   None,
        filter_regex:          None,
        h_scroll_offset:       0,
    })
}

// ── Recent-files persistence ──────────────────────────────────────────────────

fn recent_files_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".flash_recent")
}

fn load_recent_files() -> Vec<PathBuf> {
    std::fs::read_to_string(recent_files_path())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .take(10)
        .collect()
}

fn save_recent_files(files: &[PathBuf]) {
    let content = files.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(recent_files_path(), content);
}

pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate the cursor_to_line_col math without needing a full App.
    /// This validates the mapping from screen coordinates to (line, col).
    fn sim_cursor_to_line_col(
        cursor_x: f32,
        cursor_y: f32,
        font_size: f32,
        gutter_off: f32,
        log_area_top: f32,
        viewport_lines: usize,
        scroll_offset: usize,
        total_lines: usize,
        h_scroll_offset: usize,
    ) -> Option<(usize, usize)> {
        if cursor_x < gutter_off { return None; }
        let line_h = virtual_list::line_height_for_font(font_size);
        if cursor_y < log_area_top { return None; }
        let row_idx = ((cursor_y - log_area_top) / line_h) as usize;
        if row_idx >= viewport_lines { return None; }
        let view_idx = scroll_offset + row_idx;
        if view_idx >= total_lines { return None; }
        let src = view_idx; // simplified: no view_rows mapping

        let char_w = font_size * 0.6;
        let col_screen = ((cursor_x - gutter_off).max(0.0) / char_w) as usize;
        let col = col_screen + h_scroll_offset;
        Some((src, col))
    }

    #[test]
    fn test_click_maps_to_correct_line_and_col() {
        let font_size = 16.0;
        let gutter_off = 106.0; // 56 activity + 6 gutter + 44 line num
        let log_area_top = 39.0; // 1px separator + 38px filter bar
        let viewport_lines = 50;
        let total_lines = 100;
        let line_h = virtual_list::line_height_for_font(font_size); // ~22.16

        // Click on the first visible line, character column 5
        let char_w = font_size * 0.6; // 9.6
        let x = gutter_off + 5.0 * char_w + 1.0; // middle of char 5
        let y = log_area_top + 2.0; // top of first row
        let result = sim_cursor_to_line_col(x, y, font_size, gutter_off, log_area_top, viewport_lines, 0, total_lines, 0);
        assert_eq!(result, Some((0, 5)));

        // Click on line 3 (0-indexed), with scroll offset 10
        let y = log_area_top + 3.0 * line_h + 2.0;
        let result = sim_cursor_to_line_col(x, y, font_size, gutter_off, log_area_top, viewport_lines, 10, total_lines, 0);
        assert_eq!(result, Some((13, 5))); // scroll_offset=10 + row=3

        // Click on col 2 with h_scroll_offset = 10
        let x = gutter_off + 2.0 * char_w + 1.0;
        let y = log_area_top + 2.0;
        let result = sim_cursor_to_line_col(x, y, font_size, gutter_off, log_area_top, viewport_lines, 0, total_lines, 10);
        assert_eq!(result, Some((0, 12))); // col_screen=2 + h_scroll=10
    }

    #[test]
    fn test_click_outside_log_area_returns_none() {
        let font_size = 16.0;
        let gutter_off = 106.0;
        let log_area_top = 39.0;

        // Click above log area (in toolbar)
        let result = sim_cursor_to_line_col(200.0, 10.0, font_size, gutter_off, log_area_top, 50, 0, 100, 0);
        assert!(result.is_none(), "click above log area should be None");

        // Click in gutter area (left of text)
        let result = sim_cursor_to_line_col(50.0, 60.0, font_size, gutter_off, log_area_top, 50, 0, 100, 0);
        assert!(result.is_none(), "click in gutter should be None");

        // Click below viewport
        let line_h = virtual_list::line_height_for_font(font_size);
        let y = log_area_top + 50.0 * line_h + 5.0; // past viewport_lines=50
        let result = sim_cursor_to_line_col(200.0, y, font_size, gutter_off, log_area_top, 50, 0, 100, 0);
        assert!(result.is_none(), "click below viewport should be None");
    }

    #[test]
    fn test_text_sel_state_transitions() {
        // Simulate: click sets anchor=focus, drag moves focus, release stops
        let mut sel = TextSel {
            anchor_line: 5, anchor_col: 10,
            focus_line: 5, focus_col: 10,
        };
        assert!(sel.is_empty(), "initial click should be empty selection");

        // Simulate drag: focus moves
        sel.focus_line = 5;
        sel.focus_col = 20;
        assert!(!sel.is_empty(), "after drag, selection should not be empty");
        let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
        assert_eq!((lo_line, lo_col), (5, 10));
        assert_eq!((hi_line, hi_col), (5, 20));

        // Simulate multi-line drag
        sel.focus_line = 8;
        sel.focus_col = 3;
        let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
        assert_eq!((lo_line, lo_col), (5, 10));
        assert_eq!((hi_line, hi_col), (8, 3));

        // Backward selection (drag up)
        sel.anchor_line = 10;
        sel.anchor_col = 5;
        sel.focus_line = 7;
        sel.focus_col = 15;
        let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
        assert_eq!((lo_line, lo_col), (7, 15));
        assert_eq!((hi_line, hi_col), (10, 5));
    }

    #[test]
    fn test_selected_line_cleared_on_drag() {
        // Simulate: click sets selected_line, drag clears it
        let mut selected_line: Option<usize> = None;
        let mut sel = TextSel {
            anchor_line: 5, anchor_col: 10,
            focus_line: 5, focus_col: 10,
        };

        // On click: set selected_line
        selected_line = Some(5);
        assert_eq!(selected_line, Some(5));

        // On first CursorMoved while mouse_selecting:
        // position same as anchor → keep selected_line
        let new_line = 5;
        let new_col = 10;
        if new_line != sel.anchor_line || new_col != sel.anchor_col {
            selected_line = None;
        }
        assert_eq!(selected_line, Some(5), "no drag yet, keep selected_line");

        // Position diverges → clear selected_line
        let new_line = 5;
        let new_col = 15;
        if new_line != sel.anchor_line || new_col != sel.anchor_col {
            selected_line = None;
        }
        sel.focus_line = new_line;
        sel.focus_col = new_col;
        assert_eq!(selected_line, None, "drag started, selected_line should be cleared");
        assert!(!sel.is_empty(), "selection should now be non-empty");
    }
}
