use std::collections::HashSet;

use flash_core::{LineReader, LogLevel};
use iced::widget::{button, column, container, mouse_area, rich_text, row, span, text, Column};
use iced::{Color, Element, Length};

use crate::app::{ExtraHighlight, Message, ViewRow};
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

    let total_for_digits = total_lines;
    let line_num_chars   = format!("{}", total_for_digits).len();
    let line_num_col_w   = Length::Fixed((line_num_chars as f32 * 8.5 + 20.0).max(44.0));
    let row_width        = if line_wrap { Length::Fill } else { Length::Shrink };

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
            let ctx_bg         = p.context_row_bg;
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

            let text_w = if line_wrap { Length::Fill } else { Length::Shrink };
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
                    .width(text_w),
            )
            .width(text_w)
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

        // Text spans — span content is always an owned String to avoid 'a borrow issues
        let smb = ghost(p.search_match_bg, pipeline_stale);
        let smf = ghost(p.search_match_fg, pipeline_stale);

        // Helper: build a span from a &str slice using an owned copy
        // This ensures the span doesn't borrow from any local data
        macro_rules! owned_span {
            ($s:expr) => { span($s.to_string()) }
        }

        let text_spans: Vec<_> = if is_search_match {
            if let Some(re) = compiled_regex {
                let mut spans = Vec::new();
                let mut last_end = 0;
                for m in re.find_iter(display_text) {
                    if m.start() > last_end {
                        spans.push(owned_span!(&display_text[last_end..m.start()]).color(line_color));
                    }
                    spans.push(
                        owned_span!(&display_text[m.start()..m.end()])
                            .color(smf)
                            .background(smb),
                    );
                    last_end = m.end();
                }
                if last_end < display_text.len() {
                    spans.push(owned_span!(&display_text[last_end..]).color(line_color));
                }
                spans
            } else {
                vec![owned_span!(display_text).color(line_color)]
            }
        } else if let Some(eh) = extra_highlights.iter().find(|eh| {
            eh.regex.as_ref().map_or(false, |re| re.is_match(display_text))
        }) {
            if let Some(re) = &eh.regex {
                let hl_color = ghost(eh.color, pipeline_stale);
                let hl_bg    = Color { a: 0.35, ..hl_color };
                let hl_fg    = ghost(Color::from_rgb(0.0, 0.0, 0.0), pipeline_stale);
                let mut spans = Vec::new();
                let mut last_end = 0;
                for m in re.find_iter(display_text) {
                    if m.start() > last_end {
                        spans.push(owned_span!(&display_text[last_end..m.start()]).color(line_color));
                    }
                    spans.push(
                        owned_span!(&display_text[m.start()..m.end()])
                            .color(hl_fg)
                            .background(hl_bg),
                    );
                    last_end = m.end();
                }
                if last_end < display_text.len() {
                    spans.push(owned_span!(&display_text[last_end..]).color(line_color));
                }
                spans
            } else {
                vec![owned_span!(display_text).color(line_color)]
            }
        } else {
            vec![owned_span!(display_text).color(line_color)]
        };

        // Row background — priority: selected > search match > extra hl > alternating stripe
        let extra_hl_bg: Option<Color> = if !is_search_match && !is_selected {
            extra_highlights
                .iter()
                .find(|eh| eh.regex.as_ref().map_or(false, |re| re.is_match(display_text)))
                .map(|eh| Color { a: 0.10, ..ghost(eh.color, pipeline_stale) })
        } else {
            None
        };

        let alt_row_bg: Option<Color> = if src % 2 == 1 { Some(p.bg_alt_row) } else { None };

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

        let clickable_content = mouse_area(content_row)
            .on_press(Message::LineClicked(src))
            .on_right_press(Message::RightClickLine(src))
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
            "PREVIEW  Lines {}-{} of {}  ({:.0}%)  —  click [>] on a layer to change preview, or [>] on again to exit",
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

    let content_w = if line_wrap { Length::Fill } else { Length::Shrink };
    let content_area: Element<'a, Message> = container(log_content.width(content_w))
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

    column![main_area, status_bar].into()
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
