use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use flash_core::{
    spawn_search_worker, FileMap, LineIndex, LineReader, SearchHandle, SearchResponse,
};
use iced::keyboard::{self, Key};
use iced::widget::{column, container, stack, text};
use iced::{event, mouse, window};
use iced::{Color, Element, Length, Subscription, Task, Theme};

use notify::Watcher as _;

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
    pub file_data_arc:         Arc<Vec<u8>>,
    pub line_offsets_arc:      Arc<Vec<u64>>,
    pub hidden_levels:         HashSet<flash_core::LogLevel>,
    pub line_filter_query:     String,
    pub active_line_filter:    Option<Vec<usize>>,
    pub selected_line:         Option<usize>,
    // New
    pub bookmarks:             HashSet<usize>,
    pub extra_highlights:      Vec<ExtraHighlight>,
}

impl Tab {
    pub fn file_name(&self) -> String {
        self.open_file.path.file_name().unwrap_or_default().to_string_lossy().to_string()
    }
    pub fn total_lines(&self) -> usize { self.open_file.line_index.line_count() }
    pub fn total_visible_lines(&self) -> usize {
        self.active_line_filter.as_ref().map(|f| f.len()).unwrap_or_else(|| self.total_lines())
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
    pub fn rebuild_line_filter(&mut self) {
        let active = !self.hidden_levels.is_empty() || !self.line_filter_query.is_empty();
        if !active { self.active_line_filter = None; self.scroll_offset = 0; return; }
        let reader      = LineReader::new(&self.open_file.file_map, &self.open_file.line_index);
        let total       = reader.line_count();
        let q_lower     = self.line_filter_query.to_lowercase();
        let mut indices = Vec::new();
        for i in 0..total {
            if let Some(line) = reader.get_line(i) {
                if !self.hidden_levels.is_empty() {
                    if let Some(lvl) = flash_core::LogLevel::detect(line) {
                        if self.hidden_levels.contains(&lvl) { continue; }
                    }
                }
                if !q_lower.is_empty() && !line.to_lowercase().contains(&q_lower) { continue; }
                indices.push(i);
            }
        }
        self.scroll_offset      = 0;
        self.active_line_filter = Some(indices);
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
    app_theme:     AppTheme,
    settings_open: bool,
    // Line wrap
    pub line_wrap: bool,
    // Jump to line modal
    jump_open:  bool,
    jump_input: String,
    // Command palette
    palette_open:     bool,
    palette_query:    String,
    palette_selected: usize,
    // Search history (#6)
    search_history:     Vec<String>,
    history_cursor:     Option<usize>,
    history_temp_query: String,
    // Recent files (#13)
    recent_files: Vec<PathBuf>,
    // Tail mode / file watching (#7 / #8)
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
    FileLoaded(Result<(PathBuf, Vec<u8>), String>),
    ReloadActiveFile,
    FileReloaded(Result<(PathBuf, Vec<u8>), String>),
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
    CursorMoved(f32),
    // Filters
    ToggleLevelFilter(flash_core::LogLevel),
    LineFilterChanged(String),
    ClearFilters,
    // Zoom
    ZoomIn,
    ZoomOut,
    ZoomReset,
    // Modifier
    ModifiersChanged(iced::keyboard::Modifiers),
    // Theme / settings
    SetTheme(AppTheme),
    ToggleSettings,
    // Line selection / clipboard
    LineClicked(usize),
    CopyToClipboard,
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
    // Tabs
    CloseTab(usize),
    SwitchTab(usize),
    // Bookmarks (#9)
    ToggleBookmark(usize),
    NextBookmark,
    PrevBookmark,
    // Search history (#6)
    HistoryPrev,
    HistoryNext,
    // Tail mode / file watching (#7 / #8)
    TailToggle,
    PollFileChange,
    // Extra highlights (#12)
    AddHighlight,
    RemoveHighlight(usize),
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
            font_size:          13.0,
            modifiers:          iced::keyboard::Modifiers::default(),
            app_theme:          AppTheme::CatppuccinMocha,
            settings_open:      false,
            line_wrap:          false,
            jump_open:          false,
            jump_input:         String::new(),
            palette_open:       false,
            palette_query:      String::new(),
            palette_selected:   0,
            search_history:     Vec::new(),
            history_cursor:     None,
            history_temp_query: String::new(),
            recent_files:       Vec::new(),
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

    fn palette(&self) -> Palette { theme::palette(self.app_theme) }

    fn tab(&self)     -> Option<&Tab>     { self.tabs.get(self.active_tab) }
    fn tab_mut(&mut self) -> Option<&mut Tab> { self.tabs.get_mut(self.active_tab) }

    fn clamp_scroll(&self, offset: usize) -> usize {
        let total = self.tab().map(|t| t.total_visible_lines()).unwrap_or(0);
        virtual_list::clamp_offset(offset, total, self.viewport_lines)
    }

    fn recalc_viewport(&mut self) {
        let sep_h    = if self.tabs.len() > 1 { 34.0_f32 } else { 1.0 };
        let overhead = 47.0 + sep_h + 28.0 + 24.0;
        let available = (self.window_height - overhead).max(100.0);
        self.viewport_lines = virtual_list::visible_lines_for_font(available, self.font_size);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.scroll_offset = virtual_list::clamp_offset(
                tab.scroll_offset, tab.total_visible_lines(), self.viewport_lines,
            );
        }
    }

    fn scroll_to_cursor_y(&mut self) {
        let toolbar_h: f32 = 47.0;
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

    /// Set up a file watcher for the active tab's file.
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

    // ── Command palette ───────────────────────────────────────────────────────

    fn all_palette_cmds(&self) -> Vec<PaletteCmd> {
        let has_file  = !self.tabs.is_empty();
        let tail_lbl  = if self.tail_mode { "Disable Tail Mode (Live)" } else { "Enable Tail Mode (Live)" };
        let wrap_lbl  = if self.line_wrap  { "Disable Line Wrap" } else { "Enable Line Wrap" };

        let mut cmds = vec![
            PaletteCmd { label: "Open File".into(),          shortcut: "Ctrl+O", action: Message::FileOpen },
            PaletteCmd { label: "Jump to Line".into(),       shortcut: "Ctrl+G", action: Message::JumpOpen },
            PaletteCmd { label: "Toggle Settings".into(),    shortcut: "",       action: Message::ToggleSettings },
            PaletteCmd { label: "Zoom In".into(),            shortcut: "Ctrl++", action: Message::ZoomIn },
            PaletteCmd { label: "Zoom Out".into(),           shortcut: "Ctrl+-", action: Message::ZoomOut },
            PaletteCmd { label: "Reset Zoom".into(),         shortcut: "Ctrl+0", action: Message::ZoomReset },
            PaletteCmd { label: wrap_lbl.into(),             shortcut: "",       action: Message::WrapToggle },
            PaletteCmd { label: tail_lbl.into(),             shortcut: "",       action: Message::TailToggle },
            PaletteCmd { label: "Go to Top".into(),          shortcut: "Home",   action: Message::ScrollTo(0) },
            PaletteCmd { label: "Go to Bottom".into(),       shortcut: "End",    action: Message::GoToBottom },
            PaletteCmd { label: "Next Bookmark".into(),      shortcut: "F2",     action: Message::NextBookmark },
            PaletteCmd { label: "Prev Bookmark".into(),      shortcut: "⇧F2",    action: Message::PrevBookmark },
        ];
        if has_file {
            cmds.push(PaletteCmd { label: "Clear Search & Filters".into(), shortcut: "", action: Message::Clear });
            cmds.push(PaletteCmd { label: "Export Results".into(),          shortcut: "", action: Message::Export });
            cmds.push(PaletteCmd { label: "Add Highlight (current search)".into(), shortcut: "", action: Message::AddHighlight });
        }
        for &t in AppTheme::all() {
            cmds.push(PaletteCmd { label: format!("Theme: {}", t.name()), shortcut: "", action: Message::SetTheme(t) });
        }
        // Recent files (#13)
        for path in self.recent_files.iter().take(10) {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            cmds.push(PaletteCmd {
                label:    format!("Open Recent: {}", name),
                shortcut: "",
                action:   Message::FileDrop(path.clone()),
            });
        }
        // Search history (#6)
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

            // ── File open / drop (#14) ────────────────────────────────────────
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
                return Task::perform(load_file(path), Message::FileLoaded);
            }

            Message::FileLoaded(result) => {
                if let Ok((path, raw)) = result {
                    let data = ensure_utf8(raw);
                    if let Some(tab) = build_tab(path.clone(), data) {
                        self.tabs.push(tab);
                        self.active_tab = self.tabs.len() - 1;
                        // Recent files (#13)
                        self.recent_files.retain(|p| p != &path);
                        self.recent_files.insert(0, path);
                        self.recent_files.truncate(10);
                        self.setup_watcher();
                        self.recalc_viewport();
                    }
                }
            }

            // ── File reload (tail mode / #7 #8) ──────────────────────────────
            Message::ReloadActiveFile => {
                let Some(tab) = self.tab() else { return Task::none(); };
                let path = tab.open_file.path.clone();
                return Task::perform(load_file(path), Message::FileReloaded);
            }

            Message::FileReloaded(result) => {
                let Ok((path, raw)) = result else { return Task::none(); };
                let data = ensure_utf8(raw);
                let ai = self.active_tab;
                let Some(tab) = self.tabs.get_mut(ai) else { return Task::none(); };
                let line_index = LineIndex::build(&data);
                let line_count = line_index.line_count();
                let mut offsets = Vec::with_capacity(line_count + 1);
                for i in 0..=line_count { if let Some(o) = line_index.offset(i) { offsets.push(o); } }
                let file_len = data.len() as u64;
                if offsets.last().copied() != Some(file_len) { offsets.push(file_len); }
                let data_arc    = Arc::new(data);
                let offsets_arc = Arc::new(offsets);
                if let Ok(file_map) = FileMap::open(&path) {
                    let search_handle = spawn_search_worker(data_arc.clone(), offsets_arc.clone());
                    tab.search_handle.cancel();
                    tab.open_file         = OpenFile { file_map, line_index, path };
                    tab.file_data_arc     = data_arc;
                    tab.line_offsets_arc  = offsets_arc;
                    tab.search_handle     = search_handle;
                    tab.search_results.clear();
                    tab.search_result_set.clear();
                    tab.selected_result   = None;
                    tab.search_in_progress = false;
                    tab.compiled_search_regex = None;
                    tab.active_line_filter = None;
                    // Re-run the search if there was one
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
                    self.recalc_viewport();
                }
            }

            // ── Tail mode / file watching (#7 / #8) ──────────────────────────
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
                if self.modifiers.control() {
                    if delta > 0.0 { return self.update(Message::ZoomIn); }
                    else if delta < 0.0 { return self.update(Message::ZoomOut); }
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
                // Poll ALL searching tabs so background searches continue when tab is not active
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
                if let Some(tab) = self.tab_mut() {
                    tab.clear_search();
                    tab.scroll_offset      = 0;
                    tab.hidden_levels.clear();
                    tab.line_filter_query.clear();
                    tab.active_line_filter = None;
                    tab.selected_line      = None;
                }
                self.history_cursor = None;
                self.history_temp_query.clear();
            }

            // ── Export ───────────────────────────────────────────────────────
            Message::Export => {
                let Some(tab) = self.tab() else { return Task::none(); };
                let reader = LineReader::new(&tab.open_file.file_map, &tab.open_file.line_index);
                let text = if !tab.search_results.is_empty() {
                    tab.search_results.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n")
                } else if let Some(filter) = &tab.active_line_filter {
                    filter.iter().filter_map(|&i| reader.get_line(i)).collect::<Vec<_>>().join("\n")
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
            Message::KeyPressed(key, modifiers) => match key {
                Key::Named(keyboard::key::Named::PageDown) => {
                    let d = self.viewport_lines.saturating_sub(2) as i64;
                    return self.update(Message::ScrollBy(d));
                }
                Key::Named(keyboard::key::Named::PageUp) => {
                    let d = -(self.viewport_lines.saturating_sub(2) as i64);
                    return self.update(Message::ScrollBy(d));
                }
                Key::Named(keyboard::key::Named::Home) => return self.update(Message::ScrollTo(0)),
                Key::Named(keyboard::key::Named::End)  => return self.update(Message::GoToBottom),
                Key::Named(keyboard::key::Named::ArrowDown) => {
                    if modifiers.control() { return self.update(Message::HistoryNext); }
                    if self.palette_open { return self.update(Message::PaletteMove(1)); }
                    return self.update(Message::ScrollBy(1));
                }
                Key::Named(keyboard::key::Named::ArrowUp) => {
                    if modifiers.control() { return self.update(Message::HistoryPrev); }
                    if self.palette_open { return self.update(Message::PaletteMove(-1)); }
                    return self.update(Message::ScrollBy(-1));
                }
                Key::Named(keyboard::key::Named::F2) => {
                    if modifiers.shift() { return self.update(Message::PrevBookmark); }
                    return self.update(Message::NextBookmark);
                }
                Key::Named(keyboard::key::Named::Escape) => {
                    if self.palette_open {
                        self.palette_open = false; self.palette_query.clear(); self.palette_selected = 0;
                    } else if self.jump_open {
                        self.jump_open = false; self.jump_input.clear();
                    } else if self.settings_open {
                        self.settings_open = false;
                    } else if let Some(t) = self.tab_mut() {
                        t.selected_line = None;
                    }
                }
                Key::Character(c) if modifiers.control() => match c.as_str() {
                    "=" | "+" => return self.update(Message::ZoomIn),
                    "-"       => return self.update(Message::ZoomOut),
                    "0"       => return self.update(Message::ZoomReset),
                    "b"       => {
                        // Ctrl+B: toggle bookmark on selected line
                        if let Some(n) = self.tab().and_then(|t| t.selected_line) {
                            return self.update(Message::ToggleBookmark(n));
                        }
                    }
                    "c"       => return self.update(Message::CopyToClipboard),
                    "p"       => return self.update(Message::PaletteOpen),
                    "g"       => return self.update(Message::JumpOpen),
                    "o"       => return self.update(Message::FileOpen),
                    _         => {}
                },
                _ => {}
            },

            // ── Scrollbar ────────────────────────────────────────────────────
            Message::ScrollbarClicked => { self.scrollbar_dragging = true; self.scroll_to_cursor_y(); }
            Message::ScrollbarReleased => { self.scrollbar_dragging = false; }
            Message::CursorMoved(y) => {
                self.scrollbar_hover_y = y;
                if self.scrollbar_dragging { self.scroll_to_cursor_y(); }
            }

            // ── Filters ──────────────────────────────────────────────────────
            Message::ToggleLevelFilter(level) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if tab.hidden_levels.contains(&level) { tab.hidden_levels.remove(&level); }
                else { tab.hidden_levels.insert(level); }
                tab.rebuild_line_filter();
                self.recalc_viewport();
            }
            Message::LineFilterChanged(query) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                tab.line_filter_query = query;
                tab.rebuild_line_filter();
                self.recalc_viewport();
            }
            Message::ClearFilters => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                tab.hidden_levels.clear();
                tab.line_filter_query.clear();
                tab.active_line_filter = None;
                tab.scroll_offset      = 0;
                self.recalc_viewport();
            }

            // ── Zoom ─────────────────────────────────────────────────────────
            Message::ZoomIn    => { self.font_size = (self.font_size + 1.0).min(28.0); self.recalc_viewport(); }
            Message::ZoomOut   => { self.font_size = (self.font_size - 1.0).max(8.0);  self.recalc_viewport(); }
            Message::ZoomReset => { self.font_size = 13.0; self.recalc_viewport(); }
            Message::ModifiersChanged(mods) => { self.modifiers = mods; }

            // ── Theme / settings ─────────────────────────────────────────────
            Message::SetTheme(t)    => { self.app_theme = t; }
            Message::ToggleSettings => { self.settings_open = !self.settings_open; }
            Message::WrapToggle     => { self.line_wrap = !self.line_wrap; }

            // ── Line selection ────────────────────────────────────────────────
            Message::LineClicked(n) => {
                if let Some(t) = self.tab_mut() {
                    t.selected_line = if t.selected_line == Some(n) { None } else { Some(n) };
                }
            }
            Message::CopyToClipboard => {
                if let Some(tab) = self.tab() {
                    if let Some(n) = tab.selected_line {
                        let reader = LineReader::new(&tab.open_file.file_map, &tab.open_file.line_index);
                        if let Some(text) = reader.get_line(n) {
                            return iced::clipboard::write(text.to_string());
                        }
                    }
                }
            }

            // ── Jump to line ─────────────────────────────────────────────────
            Message::JumpOpen => {
                if !self.tabs.is_empty() { self.jump_open = true; self.jump_input.clear(); }
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
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                    if self.tabs.is_empty() { self.active_tab = 0; }
                    self.recalc_viewport();
                    self.setup_watcher();
                }
            }

            // ── Bookmarks (#9) ────────────────────────────────────────────────
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

            // ── Search history (#6) ───────────────────────────────────────────
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

            // ── Extra highlights (#12) ────────────────────────────────────────
            Message::AddHighlight => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if tab.extra_highlights.len() >= HIGHLIGHT_COLORS.len() { return Task::none(); }
                if tab.search_query.is_empty() { return Task::none(); }
                let idx     = tab.extra_highlights.len();
                let pattern = tab.search_query.clone();
                let re      = regex::Regex::new(&pattern).ok();
                let color   = HIGHLIGHT_COLORS[idx];
                tab.extra_highlights.push(ExtraHighlight { pattern, regex: re, color });
            }
            Message::RemoveHighlight(idx) => {
                let Some(tab) = self.tab_mut() else { return Task::none(); };
                if idx < tab.extra_highlights.len() { tab.extra_highlights.remove(idx); }
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
            (t.file_name(), t.file_data_arc.len() as u64, t.total_lines())
        });
        let search_query       = active.map(|t| t.search_query.as_str()).unwrap_or("");
        let result_count       = active.map(|t| t.search_results.len()).unwrap_or(0);
        let search_in_progress = active.map(|t| t.search_in_progress).unwrap_or(false);
        let has_file           = !self.tabs.is_empty();

        let search_bar = views::search_bar::view(
            search_query, result_count, search_in_progress, has_file,
            file_info.as_ref().map(|(n, s, l)| (n.as_str(), *s, *l)),
            self.tail_mode,
            p,
        );

        let empty_hl: HashSet<flash_core::LogLevel> = HashSet::new();
        let hidden_levels    = active.map(|t| &t.hidden_levels).unwrap_or(&empty_hl);
        let line_filter_q    = active.map(|t| t.line_filter_query.as_str()).unwrap_or("");
        let filter_count     = active.and_then(|t| t.active_line_filter.as_ref().map(|f| f.len()));
        let extra_highlights = active.map(|t| t.extra_highlights.as_slice()).unwrap_or(&[]);

        let filter_bar = views::filter_bar::view(
            hidden_levels, line_filter_q, has_file, filter_count,
            extra_highlights, search_query, p,
        );

        let reader            = active.map(|t| LineReader::new(&t.open_file.file_map, &t.open_file.line_index));
        let total_visible     = active.map(|t| t.total_visible_lines()).unwrap_or(0);
        let active_filter     = active.and_then(|t| t.active_line_filter.as_deref());
        let scroll_offset     = active.map(|t| t.scroll_offset).unwrap_or(0);
        let compiled_regex    = active.and_then(|t| t.compiled_search_regex.as_ref());
        let empty_set: HashSet<usize> = HashSet::new();
        let search_result_set = active.map(|t| &t.search_result_set).unwrap_or(&empty_set);
        let selected_line     = active.and_then(|t| t.selected_line);
        let empty_bm: HashSet<usize> = HashSet::new();
        let bookmarks         = active.map(|t| &t.bookmarks).unwrap_or(&empty_bm);
        let search_results    = active.map(|t| t.search_results.as_slice()).unwrap_or(&[]);

        let log_view = views::log_view::view(
            reader, scroll_offset, self.viewport_lines, total_visible,
            compiled_regex, search_result_set, active_filter,
            self.font_size, selected_line, self.line_wrap,
            bookmarks, extra_highlights, search_results,
            p,
        );

        let selected_result = active.and_then(|t| t.selected_result);
        let results_panel = views::results_panel::view(
            search_results, selected_result, search_in_progress, compiled_regex, p,
        );

        let hr = container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(bdr.into()), ..Default::default()
            });

        let mut col = column![search_bar];
        if self.tabs.len() > 1 {
            col = col.push(views::tab_bar::view(&self.tabs, self.active_tab, p));
        } else {
            col = col.push(hr);
        }
        col = col.push(filter_bar).push(log_view).push(results_panel);

        let main_layout = container(col)
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
            let overlay = views::settings_panel::view(self.app_theme, self.font_size, self.line_wrap, p);
            stack![main_layout, overlay].into()
        } else {
            main_layout.into()
        }
    }

    pub fn theme(&self) -> Theme { self.app_theme.iced_theme() }

    // ── Subscriptions ─────────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![];

        subs.push(keyboard::on_key_press(|key, modifiers| Some(Message::KeyPressed(key, modifiers))));
        subs.push(window::resize_events().map(|(_id, size)| Message::WindowResized(size)));

        // Mouse wheel (ignored-only to avoid fighting the results scrollable)
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

        // Global cursor + button release (scrollbar drag)
        subs.push(event::listen_with(|evt, _status, _id| match evt {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::CursorMoved(position.y)),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::ScrollbarReleased),
            _ => None,
        }));

        // Modifier key tracking (Ctrl+Wheel zoom, Ctrl+Arrow history)
        subs.push(event::listen_with(|evt, _status, _id| {
            if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = evt {
                Some(Message::ModifiersChanged(mods))
            } else { None }
        }));

        // File drop (#14)
        subs.push(event::listen_with(|evt, _status, _id| {
            if let iced::Event::Window(window::Event::FileDropped(path)) = evt {
                Some(Message::FileDrop(path))
            } else { None }
        }));

        if self.tabs.iter().any(|t| t.search_in_progress) {
            subs.push(iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::PollSearchResults));
        }

        // File change polling (#7 / #8)
        if self.watch_rx.is_some() {
            subs.push(iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::PollFileChange));
        }

        Subscription::batch(subs)
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Async helper: read + decompress a file and return (path, bytes).
async fn load_file(path: PathBuf) -> Result<(PathBuf, Vec<u8>), String> {
    let raw = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    let data = if path.extension().and_then(|e| e.to_str()) == Some("gz") {
        use flate2::read::GzDecoder;
        use std::io::Read;
        let mut dec = GzDecoder::new(&raw[..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).map_err(|e| e.to_string())?;
        out
    } else {
        raw
    };
    Ok((path, data))
}

/// If bytes aren't valid UTF-8, do a lossy replacement.
fn ensure_utf8(raw: Vec<u8>) -> Vec<u8> {
    if std::str::from_utf8(&raw).is_err() {
        String::from_utf8_lossy(&raw).into_owned().into_bytes()
    } else {
        raw
    }
}

/// Build a Tab from a path + raw UTF-8 bytes. Returns None if mmap fails.
fn build_tab(path: PathBuf, data: Vec<u8>) -> Option<Tab> {
    let line_index  = LineIndex::build(&data);
    let line_count  = line_index.line_count();
    let mut offsets = Vec::with_capacity(line_count + 1);
    for i in 0..=line_count { if let Some(o) = line_index.offset(i) { offsets.push(o); } }
    let file_len = data.len() as u64;
    if offsets.last().copied() != Some(file_len) { offsets.push(file_len); }
    let data_arc    = Arc::new(data);
    let offsets_arc = Arc::new(offsets);
    let file_map    = FileMap::open(&path).ok()?;
    let search_handle = spawn_search_worker(data_arc.clone(), offsets_arc.clone());
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
        active_line_filter:    None,
        selected_line:         None,
        bookmarks:             HashSet::new(),
        extra_highlights:      Vec::new(),
    })
}

pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}
