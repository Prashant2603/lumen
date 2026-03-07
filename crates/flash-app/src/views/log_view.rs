use std::collections::HashSet;

use flash_core::{LineReader, LogLevel};
use iced::widget::{button, column, container, mouse_area, rich_text, row, slider, span, text, Column};
use iced::{Color, Element, Length};

use crate::app::{ExtraHighlight, Message, TextSel, ViewRow};
use crate::pipeline::TransformPipeline;
use crate::theme::Palette;

pub fn view<'a>(
    reader:            Option<LineReader<'a>>,
    scroll_offset:     usize,
    viewport_lines:    usize,
    total_lines:       usize,
    compiled_regex:    Option<&regex::Regex>,
    search_result_set: &HashSet<usize>,
    view_rows:         Option<&'a [ViewRow]>,
    jump_source:       Option<usize>,
    font_size:         f32,
    selected_line:     Option<usize>,
    line_wrap:         bool,
    bookmarks:         &HashSet<usize>,
    extra_highlights:  &[ExtraHighlight],
    _search_results:   &[(usize, String)],
    context_expanded:    &HashSet<usize>,
    pipeline:            Option<&'a TransformPipeline>,
    pipeline_stale:      bool,
    pipeline_preview_to: Option<u64>,
    color_log_levels:    bool,
    h_scroll_offset:     usize,
    text_sel:            Option<TextSel>,
    block_cursor:        bool,
    p:                   Palette,
) -> Element<'a, Message> {
    if reader.is_none() || total_lines == 0 {
        let dimmer = Color { a: 0.5, ..p.fg_muted };
        return container(
            column![
                text("Flash").size(32).color(p.accent),
                text("High-performance log viewer").size(14).color(p.fg_muted),
                text("Drop a file or click Open File to begin").size(14).color(p.fg_muted),
                text("Supports files up to 2 GB · .gz decompression · multi-tab").size(12).color(dimmer),
            ]
            .spacing(8)
            .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(p.bg_primary.into()),
            ..Default::default()
        })
        .into();
    }

    let reader = reader.unwrap();

    // ── Determine which rows to render ───────────────────────────────────────
    let is_raw_mode = jump_source.is_some();

    struct RowInfo {
        src:        usize,
        is_context: bool,
        _anchor:    Option<usize>,
    }

    let render_rows: Vec<RowInfo> = if is_raw_mode {
        (scroll_offset..(scroll_offset + viewport_lines + 1).min(total_lines))
            .map(|i| RowInfo { src: i, is_context: false, _anchor: None })
            .collect()
    } else if let Some(rows) = view_rows {
        let end = (scroll_offset + viewport_lines + 1).min(rows.len());
        rows[scroll_offset..end]
            .iter()
            .map(|row| match row {
                ViewRow::Line(n) => RowInfo { src: *n, is_context: false, _anchor: None },
                ViewRow::ContextLine { src, anchor } => RowInfo {
                    src: *src, is_context: true, _anchor: Some(*anchor),
                },
            })
            .collect()
    } else {
        reader.get_lines(scroll_offset, viewport_lines + 1)
            .into_iter()
            .map(|(i, _)| RowInfo { src: i, is_context: false, _anchor: None })
            .collect()
    };

    // True only when the pipeline is actively filtering lines
    let pipeline_has_filters = pipeline
        .map(|pl| pl.has_active_filter_layers())
        .unwrap_or(false);

    // Pre-compute display texts as owned Strings so span content can own its data
    // (iced's span() requires 'a lifetime; using span(String) avoids borrow issues)
    let display_strings: Vec<String> = render_rows.iter().map(|ri| {
        let raw = reader.get_line(ri.src).unwrap_or("");
        if let Some(pl) = pipeline {
            if let Some(preview_id) = pipeline_preview_to {
                pl.apply_text_transforms_up_to(raw, preview_id).into_owned()
            } else {
                pl.apply_text_transforms(raw).into_owned()
            }
        } else {
            raw.to_string()
        }
    }).collect();

    // Apply horizontal character offset (non-wrap mode only)
    let display_strings: Vec<String> = if !line_wrap && h_scroll_offset > 0 {
        display_strings.into_iter().map(|s| {
            // Skip h_scroll_offset characters (unicode-safe)
            s.chars().skip(h_scroll_offset).collect()
        }).collect()
    } else {
        display_strings
    };

    // Compute max visible line length for the scrollbar thumb
    let max_visible_len: usize = if !line_wrap {
        display_strings.iter().map(|s| s.chars().count()).max().unwrap_or(0) + h_scroll_offset
    } else { 0 };

    // Use the full file line count for digit width so source line numbers always fit,
    // even when filtering reduces visible lines to a small subset.
    let total_for_digits = reader.line_count().max(total_lines);
    let line_num_chars   = format!("{}", total_for_digits).len();
    let line_num_col_w   = Length::Fixed((line_num_chars as f32 * 8.5 + 20.0).max(44.0));
    // Always Fill so clicking anywhere on a row (including empty right-side area) registers.
    let row_width        = Length::Fill;

    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(render_rows.len());

    for (idx, row_info) in render_rows.iter().enumerate() {
        let src          = row_info.src;
        let is_context   = row_info.is_context;
        let display_text = display_strings[idx].as_str();

        let is_search_match = search_result_set.contains(&src);
        let is_selected     = selected_line == Some(src);
        let is_bookmark     = bookmarks.contains(&src);
        let level           = LogLevel::detect(display_text);
        let base_line_color = if color_log_levels {
            match level {
                Some(l) => p.log_level_color(l),
                None    => p.fg_primary,
            }
        } else {
            p.fg_primary
        };

        let line_color = ghost(base_line_color, pipeline_stale);
        let ln_color   = ghost(p.line_number, pipeline_stale);

        // ── Context rows: simplified rendering ───────────────────────────────
        if is_context {
            // Apply selection highlight to context rows (full-row bg overlay)
            let ctx_is_selected = text_sel.map_or(false, |sel| {
                if sel.is_empty() { return false; }
                let ((lo_line, _), (hi_line, _)) = sel.normalised();
                src >= lo_line && src <= hi_line
            });
            let ctx_bg         = if ctx_is_selected { ghost(p.selection_bg, pipeline_stale) } else { p.context_row_bg };
            let ctx_text_color = ghost(Color { a: 0.6, ..base_line_color }, pipeline_stale);
            let display_owned  = display_text.to_string();

            let ln_widget = container(
                text(format!("{:>width$}", src + 1, width = line_num_chars))
                    .size(font_size)
                    .font(iced::Font::MONOSPACE)
                    .color(ctx_text_color),
            )
            .width(line_num_col_w)
            .padding(iced::Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 });

            let ctx_text_w = if line_wrap { Length::Fill } else { Length::Shrink };
            let wrap_mode = if line_wrap {
                iced::widget::text::Wrapping::WordOrGlyph
            } else {
                iced::widget::text::Wrapping::None
            };
            let text_widget = container(
                rich_text([span(display_owned).color(ctx_text_color)])
                    .size(font_size)
                    .font(iced::Font::MONOSPACE)
                    .wrapping(wrap_mode)
                    .width(ctx_text_w),
            )
            .width(ctx_text_w)
            .padding(iced::Padding { top: 2.0, right: 10.0, bottom: 2.0, left: 0.0 })
            .style(move |_: &iced::Theme| container::Style {
                background: Some(ctx_bg.into()),
                ..Default::default()
            });

            let full_row = row![ln_widget, text_widget]
                .width(row_width)
                .height(Length::Shrink);
            rows.push(full_row.into());
            continue;
        }

        // ── Regular row ───────────────────────────────────────────────────────

        // Gutter strips
        let bm_color_opt = if is_bookmark     { Some(p.accent)        } else { None };
        let sr_color_opt = if is_search_match { Some(p.search_gutter) } else { None };

        let bm_strip = container(text("").size(1))
            .width(Length::Fixed(3.0))
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: bm_color_opt.map(|c| c.into()),
                ..Default::default()
            });
        let sr_strip = container(text("").size(1))
            .width(Length::Fixed(3.0))
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: sr_color_opt.map(|c| c.into()),
                ..Default::default()
            });

        let gutter = mouse_area(row![bm_strip, sr_strip].height(Length::Fill))
            .on_press(Message::ToggleBookmark(src))
            .interaction(iced::mouse::Interaction::Pointer);

        // Line number
        let ln_widget = container(
            text(format!("{:>width$}", src + 1, width = line_num_chars))
                .size(font_size)
                .font(iced::Font::MONOSPACE)
                .color(ln_color),
        )
        .width(line_num_col_w)
        .padding(iced::Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 });

        // ── Compute character-level selection range for this line ──────────────
        // sel_range = (start_char, end_char) relative to display_text (after h_scroll_offset applied)
        let sel_range: Option<(usize, usize)> = text_sel.and_then(|sel| {
            if sel.is_empty() { return None; }
            let ((lo_line, lo_col), (hi_line, hi_col)) = sel.normalised();
            if src < lo_line || src > hi_line { return None; }
            let start_col_abs = if src == lo_line { lo_col } else { 0 };
            let end_col_abs   = if src == hi_line { hi_col } else { usize::MAX };
            // Adjust for horizontal scroll offset
            let start = start_col_abs.saturating_sub(h_scroll_offset);
            let end   = if end_col_abs == usize::MAX {
                usize::MAX
            } else {
                end_col_abs.saturating_sub(h_scroll_offset)
            };
            if end != usize::MAX && start >= end { return None; }
            Some((start, end))
        });

        // ── Build text spans using char-level annotation ───────────────────────
        // This approach handles search/extra-highlight and selection overlays uniformly.
        let smb = ghost(p.search_match_bg, pipeline_stale);
        let smf = ghost(p.search_match_fg, pipeline_stale);

        let chars: Vec<char> = display_text.chars().collect();
        let char_count = chars.len();

        // Per-char foreground and optional background annotations
        let mut fg_ann: Vec<Color>        = vec![line_color; char_count];
        let mut bg_ann: Vec<Option<Color>> = vec![None; char_count];

        // Build a byte-index → char-index lookup for regex matches (byte positions)
        // Only built when we actually need it (search match or extra highlight active)
        let build_byte_to_char = || -> Vec<usize> {
            let mut map = vec![0usize; display_text.len() + 1];
            let mut ci  = 0usize;
            for (bi, _ch) in display_text.char_indices() {
                map[bi] = ci;
                ci += 1;
            }
            // Fill any trailing bytes for multi-byte chars
            map[display_text.len()] = ci;
            map
        };

        if is_search_match {
            if let Some(re) = compiled_regex {
                let b2c = build_byte_to_char();
                for m in re.find_iter(display_text) {
                    let sc = b2c[m.start()];
                    let ec = b2c[m.end()].min(char_count);
                    for i in sc..ec { fg_ann[i] = smf; bg_ann[i] = Some(smb); }
                }
            }
        } else if let Some(eh) = extra_highlights.iter().find(|eh| {
            eh.regex.as_ref().map_or(false, |re| re.is_match(display_text))
        }) {
            if let Some(re) = &eh.regex {
                let hl_color = ghost(eh.color, pipeline_stale);
                let hl_bg    = Color { a: 0.35, ..hl_color };
                let hl_fg    = ghost(Color::from_rgb(0.0, 0.0, 0.0), pipeline_stale);
                let b2c      = build_byte_to_char();
                for m in re.find_iter(display_text) {
                    let sc = b2c[m.start()];
                    let ec = b2c[m.end()].min(char_count);
                    for i in sc..ec { fg_ann[i] = hl_fg; bg_ann[i] = Some(hl_bg); }
                }
            }
        }

        // Apply selection overlay (overrides bg, preserves fg from search/hl)
        if let Some((sel_start, sel_end)) = sel_range {
            let start = sel_start.min(char_count);
            let end   = if sel_end == usize::MAX { char_count } else { sel_end.min(char_count) };
            let base  = ghost(p.selection_bg, pipeline_stale);
            let sel_bg = Color { a: (base.a * 1.4).min(0.65), ..base };
            for i in start..end { bg_ann[i] = Some(sel_bg); }
        }

        // ── Cursor (caret) ──────────────────────────────────────────────────
        // Block cursor (Insert toggles): inverted colors on the character.
        // Line cursor (default): accent-colored background on the character,
        //   keeping original fg — NO extra characters inserted (that would
        //   shift text and break coordinate mapping).
        let cursor_col_on_this_line: Option<usize> = text_sel.and_then(|sel| {
            if src == sel.focus_line {
                let col = sel.focus_col.saturating_sub(h_scroll_offset);
                Some(col)
            } else {
                None
            }
        });

        let caret_color = ghost(p.accent, pipeline_stale);
        let cursor_at_eol = if let Some(cc) = cursor_col_on_this_line {
            if cc < char_count {
                if block_cursor {
                    // Block: invert colors
                    fg_ann[cc] = p.bg_primary;
                    bg_ann[cc] = Some(caret_color);
                } else {
                    // Line: accent background, keep original fg
                    bg_ann[cc] = Some(Color { a: 0.45, ..caret_color });
                }
                false
            } else {
                true // cursor past end of line
            }
        } else {
            false
        };

        // Merge consecutive same-styled chars into spans
        let mut text_spans: Vec<_> = if char_count == 0 {
            vec![span("".to_string()).color(line_color)]
        } else {
            let mut spans = Vec::new();
            let mut i = 0;
            while i < char_count {
                let fg = fg_ann[i];
                let bg = bg_ann[i];
                let mut j = i + 1;
                while j < char_count
                    && color_approx_eq(fg_ann[j], fg)
                    && color_opt_approx_eq(bg_ann[j], bg)
                {
                    j += 1;
                }
                let text: String = chars[i..j].iter().collect();
                let mut s = span(text).color(fg);
                if let Some(bg_color) = bg { s = s.background(bg_color); }
                spans.push(s);
                i = j;
            }
            spans
        };

        // Append cursor indicator at end of line (only character we add — at EOL so no shift).
        // Only in block cursor mode (inverted space). Line cursor mode uses background-only
        // styling on existing characters, so nothing to append at EOL (avoids extra width).
        if (cursor_at_eol || (cursor_col_on_this_line.is_some() && char_count == 0)) && block_cursor {
            text_spans.push(span(" ".to_string()).color(p.bg_primary).background(caret_color));
        }

        // Row background — priority: selected > search match > extra hl > alternating stripe
        // Note: text selection is handled per-character via span backgrounds (sel_range above),
        // NOT via full-row background — otherwise the whole row looks the same and you can't
        // tell which characters are actually selected.
        let extra_hl_bg: Option<Color> = if !is_search_match && !is_selected {
            extra_highlights
                .iter()
                .find(|eh| eh.regex.as_ref().map_or(false, |re| re.is_match(display_text)))
                .map(|eh| Color { a: 0.10, ..ghost(eh.color, pipeline_stale) })
        } else {
            None
        };

        // Use view-row index (not src) so stripes always alternate visually after filtering
        let alt_row_bg: Option<Color> = if (scroll_offset + idx) % 2 == 1 { Some(p.bg_alt_row) } else { None };

        let row_bg = if is_selected {
            Some(p.selected_line)
        } else if is_search_match {
            Some(p.search_row_bg)
        } else {
            extra_hl_bg.or(alt_row_bg)
        };

        let text_w = if line_wrap { Length::Fill } else { Length::Shrink };
        let wrap_mode = if line_wrap {
            iced::widget::text::Wrapping::WordOrGlyph
        } else {
            iced::widget::text::Wrapping::None
        };
        let text_widget = container(
            rich_text(text_spans).size(font_size).font(iced::Font::MONOSPACE).wrapping(wrap_mode).width(text_w),
        )
        .width(text_w)
            .padding(iced::Padding { top: 2.0, right: 10.0, bottom: 2.0, left: 0.0 })
            .style(move |_: &iced::Theme| container::Style {
                background: row_bg.map(|c| c.into()),
                ..Default::default()
            });

        // Context expand/collapse button — only when pipeline is actively filtering
        let ctx_btn: Option<Element<'a, Message>> = if !is_raw_mode && pipeline_has_filters {
            let is_expanded = context_expanded.contains(&src);
            let ctx_label   = if is_expanded { "▼" } else { "▶" };
            let ctx_color   = ghost(p.fg_muted, pipeline_stale);
            let bgh2        = p.bg_hover;
            let btn = button(text(ctx_label).size(10).color(ctx_color))
                .on_press(Message::ToggleContext(src))
                .padding([1, 4])
                .style(move |_: &iced::Theme, status| button::Style {
                    background: Some(match status {
                        button::Status::Hovered => bgh2.into(),
                        _ => Color::TRANSPARENT.into(),
                    }),
                    text_color: ctx_color,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                });
            Some(btn.into())
        } else {
            None
        };

        // Jump-to-source / back button — ↑src only shown when pipeline is filtering
        let src_btn: Option<Element<'a, Message>> = if is_raw_mode {
            // Show "← back" on the first visible line only to avoid clutter
            if src == render_rows.first().map(|r| r.src).unwrap_or(src) {
                let back_color = p.accent;
                let bgh4       = p.bg_hover;
                let btn = button(text("← back").size(9).color(back_color))
                    .on_press(Message::JumpToSourceClear)
                    .padding([1, 4])
                    .style(move |_: &iced::Theme, status| button::Style {
                        background: Some(match status {
                            button::Status::Hovered => bgh4.into(),
                            _ => Color::TRANSPARENT.into(),
                        }),
                        text_color: back_color,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                    });
                Some(btn.into())
            } else {
                None
            }
        } else if pipeline_has_filters {
            // ↑src only makes sense when the pipeline is filtering lines
            let jts_color = ghost(p.fg_muted, pipeline_stale);
            let bgh3      = p.bg_hover;
            let btn = button(text("↑src").size(9).color(jts_color))
                .on_press(Message::JumpToSource(src))
                .padding([1, 4])
                .style(move |_: &iced::Theme, status| button::Style {
                    background: Some(match status {
                        button::Status::Hovered => bgh3.into(),
                        _ => Color::TRANSPARENT.into(),
                    }),
                    text_color: jts_color,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                });
            Some(btn.into())
        } else {
            None
        };

        // Assemble content row
        let mut content_row = row![ln_widget, text_widget].spacing(0);
        if let Some(b) = ctx_btn { content_row = content_row.push(b); }
        if let Some(b) = src_btn { content_row = content_row.push(b); }
        let content_row = content_row.width(row_width).height(Length::Shrink);

        // Wrap in a Fill container so clicking anywhere on the row (including
        // empty right-side space) registers, not just over the text itself.
        let row_container = container(content_row)
            .width(Length::Fill)
            .height(Length::Shrink);
        let clickable_content = mouse_area(row_container)
            .on_release(Message::LineClicked(src))
            .on_right_press(Message::RightClickLine(src))
            .on_enter(Message::LineHovered(src))
            .on_move(move |_| Message::LineHovered(src))
            .interaction(iced::mouse::Interaction::Text);

        let full_row = row![gutter, clickable_content]
            .width(row_width)
            .height(Length::Shrink);

        rows.push(full_row.into());
    }

    // ── Status bar ────────────────────────────────────────────────────────────
    let scroll_pct = if total_lines > 0 {
        (scroll_offset as f64 / total_lines as f64 * 100.0).min(100.0)
    } else { 0.0 };

    let sbg     = p.bg_secondary;
    let sborder = p.border;
    let muted   = ghost(p.fg_muted, pipeline_stale);

    let status_text = if let Some(jl) = jump_source {
        format!("RAW VIEW  line {} of {}  —  Esc or <- back to return to pipeline view", jl + 1, total_lines)
    } else if pipeline_preview_to.is_some() {
        let visible = view_rows.map(|r| r.len()).unwrap_or(total_lines);
        format!(
            "PREVIEW  Lines {}-{} of {}  ({:.0}%)  —  click ▶ on a layer to change preview, or ▶ again to exit",
            scroll_offset + 1,
            (scroll_offset + viewport_lines).min(visible),
            visible,
            scroll_pct,
        )
    } else {
        let visible  = view_rows.map(|r| r.len()).unwrap_or(total_lines);
        let sel_part = match selected_line {
            Some(n) => format!("  |  Line {} selected", n + 1),
            None    => String::new(),
        };
        format!(
            "Lines {}-{} of {}  ({:.0}%){}",
            scroll_offset + 1,
            (scroll_offset + viewport_lines).min(visible),
            visible,
            scroll_pct,
            sel_part,
        )
    };

    let status_text_color = if pipeline_preview_to.is_some() { p.accent } else { muted };
    let status_bar = container(text(status_text).size(11).color(status_text_color))
        .width(Length::Fill)
        .padding([4, 14])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(sbg.into()),
            border: iced::Border { color: sborder, width: 1.0, radius: 0.0.into() },
            ..Default::default()
        });

    let pbg = p.bg_primary;
    let log_content = Column::with_children(rows);

    // Always Fill — the outer container has clip(true) so long lines get clipped.
    // Using Shrink here caused rows (all Fill-width) to collapse to 0 width on some platforms.
    let content_area: Element<'a, Message> = container(log_content.width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(pbg.into()),
            ..Default::default()
        })
        .into();

    let total_for_minimap = view_rows.map(|r| r.len()).unwrap_or(total_lines);
    let minimap = build_minimap(
        scroll_offset, total_for_minimap, viewport_lines,
        search_result_set, bookmarks, p,
    );
    let main_area = row![content_area, minimap].height(Length::Fill);

    // ── Horizontal scrollbar (non-wrap mode) ─────────────────────────────────
    // Uses an iced slider widget for native click + drag support.
    let h_scrollbar: Option<Element<'a, Message>> = if !line_wrap {
        let sbg2    = p.bg_secondary;
        let sbdr2   = p.border;
        let thumb_c = p.fg_muted;

        // Max scroll range = longest visible line length (at least 1 beyond current offset)
        let max_scroll: f32 = (max_visible_len.max(h_scroll_offset + 1) as f32).max(1.0);

        let rail_bg   = Color { a: 0.0, ..sbg2 };          // transparent rail
        let thumb_bg  = Color { a: 0.75, ..thumb_c };
        let thumb_hov = Color { a: 0.95, ..thumb_c };

        let hbar = slider(0.0..=max_scroll, h_scroll_offset as f32, Message::HScrollTo)
            .width(Length::Fill)
            .height(10.0)
            .style(move |_: &iced::Theme, status| {
                let handle_bg = match status {
                    slider::Status::Hovered | slider::Status::Dragged => thumb_hov,
                    _ => thumb_bg,
                };
                slider::Style {
                    rail: slider::Rail {
                        backgrounds: (rail_bg.into(), rail_bg.into()),
                        width: 4.0,
                        border: iced::Border::default(),
                    },
                    handle: slider::Handle {
                        shape: slider::HandleShape::Rectangle {
                            width: 40,
                            border_radius: 2.0.into(),
                        },
                        background: handle_bg.into(),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                    },
                }
            });

        let bar = container(hbar)
            .width(Length::Fill)
            .height(Length::Fixed(14.0))
            .padding(iced::Padding { top: 2.0, bottom: 2.0, left: 0.0, right: 0.0 })
            .style(move |_: &iced::Theme| container::Style {
                background: Some(sbg2.into()),
                border: iced::Border { color: sbdr2, width: 1.0, radius: 0.0.into() },
                ..Default::default()
            });

        Some(bar.into())
    } else {
        None
    };

    let mut col = column![main_area];
    if let Some(hbar) = h_scrollbar { col = col.push(hbar); }
    col.push(status_bar).into()
}

// ── Stale ghosting helper ─────────────────────────────────────────────────────

#[inline]
fn ghost(c: Color, stale: bool) -> Color {
    if stale { Color { a: c.a * 0.45, ..c } } else { c }
}

// ── Minimap ───────────────────────────────────────────────────────────────────

fn build_minimap<'a>(
    scroll_offset:     usize,
    total_lines:       usize,
    viewport_lines:    usize,
    search_result_set: &HashSet<usize>,
    bookmarks:         &HashSet<usize>,
    p:                 Palette,
) -> Element<'a, Message> {
    let sbg = p.bg_secondary;

    if total_lines == 0 {
        return container(text("").size(1))
            .width(Length::Fixed(12.0))
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(sbg.into()),
                ..Default::default()
            })
            .into();
    }

    const N: usize = 100;

    let to_bucket = |line: usize| -> usize {
        if total_lines <= 1 { 0 } else { (line * N / total_lines).min(N - 1) }
    };

    let mut has_search = vec![false; N];
    let mut has_bm     = vec![false; N];

    for &line in search_result_set { has_search[to_bucket(line)] = true; }
    for &line in bookmarks         { has_bm[to_bucket(line)]     = true; }

    let vp_start = to_bucket(scroll_offset);
    let vp_end   = to_bucket((scroll_offset + viewport_lines).min(total_lines.saturating_sub(1)));

    let sm_color = p.search_gutter;
    let bm_color = p.accent;
    let vp_color = Color::from_rgba(0.65, 0.65, 0.70, 0.28);

    let get_color = |i: usize| -> Color {
        let in_vp = i >= vp_start && i <= vp_end;
        if has_bm[i] {
            if in_vp {
                Color { r: bm_color.r * 0.75, g: bm_color.g * 0.75, b: bm_color.b * 0.75, a: 1.0 }
            } else { bm_color }
        } else if has_search[i] {
            if in_vp {
                Color { r: sm_color.r * 0.75, g: sm_color.g * 0.75, b: sm_color.b * 0.75, a: sm_color.a }
            } else { sm_color }
        } else if in_vp { vp_color } else { sbg }
    };

    let mut segments: Vec<(u16, Color)> = Vec::new();
    let mut i = 0;
    while i < N {
        let color = get_color(i);
        let mut count = 1u16;
        while i + (count as usize) < N {
            if !color_approx_eq(get_color(i + (count as usize)), color) { break; }
            count += 1;
        }
        segments.push((count, color));
        i += count as usize;
    }

    let children: Vec<Element<'a, Message>> = segments
        .into_iter()
        .map(|(count, color)| {
            container(text("").size(1))
                .height(Length::FillPortion(count))
                .width(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(color.into()),
                    ..Default::default()
                })
                .into()
        })
        .collect();

    let track = container(Column::with_children(children))
        .width(Length::Fixed(12.0))
        .height(Length::Fill)
        .padding([2, 1])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(sbg.into()),
            ..Default::default()
        });

    mouse_area(track)
        .on_press(Message::ScrollbarClicked)
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

#[inline]
fn color_approx_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.002
        && (a.g - b.g).abs() < 0.002
        && (a.b - b.b).abs() < 0.002
        && (a.a - b.a).abs() < 0.002
}

#[inline]
fn color_opt_approx_eq(a: Option<Color>, b: Option<Color>) -> bool {
    match (a, b) {
        (None, None)         => true,
        (Some(ca), Some(cb)) => color_approx_eq(ca, cb),
        _                    => false,
    }
}
