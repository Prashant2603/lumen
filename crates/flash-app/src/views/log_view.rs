use std::collections::HashSet;

use flash_core::{LineReader, LogLevel};
use iced::widget::{column, container, mouse_area, rich_text, row, scrollable, span, text, Column};
use iced::{Color, Element, Length};

use crate::app::{ExtraHighlight, Message};
use crate::theme::Palette;

pub fn view<'a>(
    reader:            Option<LineReader<'a>>,
    scroll_offset:     usize,
    viewport_lines:    usize,
    total_lines:       usize,
    compiled_regex:    Option<&regex::Regex>,
    search_result_set: &HashSet<usize>,
    active_filter:     Option<&'a [usize]>,
    font_size:         f32,
    selected_line:     Option<usize>,
    line_wrap:         bool,
    bookmarks:         &HashSet<usize>,
    extra_highlights:  &[ExtraHighlight],
    _search_results:   &[(usize, String)],
    p:                 Palette,
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

    let lines: Vec<(usize, &str)> = if let Some(filter) = active_filter {
        let end = (scroll_offset + viewport_lines + 1).min(filter.len());
        filter[scroll_offset..end]
            .iter()
            .filter_map(|&actual| reader.get_line(actual).map(|t| (actual, t)))
            .collect()
    } else {
        reader.get_lines(scroll_offset, viewport_lines + 1)
    };

    let total_for_digits = if let Some(filter) = active_filter {
        filter.last().copied().unwrap_or(0) + 1
    } else {
        total_lines
    };
    let line_num_chars = format!("{}", total_for_digits).len();
    let line_num_col_w = Length::Fixed((line_num_chars as f32 * 8.5 + 20.0).max(44.0));

    // In wrap mode each row is Fill width; in scroll mode each row is Shrink width.
    let row_width = if line_wrap { Length::Fill } else { Length::Shrink };

    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(lines.len());

    for (line_num, line_text) in &lines {
        let line_num        = *line_num;
        let is_search_match = search_result_set.contains(&line_num);
        let is_selected     = selected_line == Some(line_num);
        let is_bookmark     = bookmarks.contains(&line_num);
        let level           = LogLevel::detect(line_text);
        let line_color      = match level {
            Some(l) => p.log_level_color(l),
            None    => p.fg_primary,
        };

        // ── Gutter: left 3px = bookmark, right 3px = search match ────────────
        // The gutter is clickable → ToggleBookmark
        let bm_color_opt = if is_bookmark { Some(p.accent) } else { None };
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

        let gutter = mouse_area(
            row![bm_strip, sr_strip].height(Length::Fill),
        )
        .on_press(Message::ToggleBookmark(line_num))
        .interaction(iced::mouse::Interaction::Pointer);

        // ── Line number ───────────────────────────────────────────────────────
        let ln_color = p.line_number;
        let ln_widget = container(
            text(format!("{:>width$}", line_num + 1, width = line_num_chars))
                .size(font_size)
                .color(ln_color),
        )
        .width(line_num_col_w)
        .padding(iced::Padding { top: 1.0, right: 6.0, bottom: 1.0, left: 4.0 });

        // ── Text spans (search + extra highlight coloring) ────────────────────
        let smb = p.search_match_bg;
        let smf = p.search_match_fg;

        let text_spans: Vec<_> = if is_search_match {
            // Search match: highlight the matched text in orange
            if let Some(re) = compiled_regex {
                let mut spans = Vec::new();
                let mut last_end = 0;
                for m in re.find_iter(line_text) {
                    if m.start() > last_end {
                        spans.push(span(&line_text[last_end..m.start()]).color(line_color));
                    }
                    spans.push(
                        span(&line_text[m.start()..m.end()])
                            .color(smf)
                            .background(smb),
                    );
                    last_end = m.end();
                }
                if last_end < line_text.len() {
                    spans.push(span(&line_text[last_end..]).color(line_color));
                }
                spans
            } else {
                vec![span(*line_text).color(line_color)]
            }
        } else if let Some(eh) = extra_highlights.iter().find(|eh| {
            eh.regex.as_ref().map_or(false, |re| re.is_match(line_text))
        }) {
            // Extra highlight: show matched portions in the highlight color
            if let Some(re) = &eh.regex {
                let hl_color = eh.color;
                let hl_bg    = Color { a: 0.35, ..hl_color };
                let hl_fg    = Color::from_rgb(0.0, 0.0, 0.0);
                let mut spans = Vec::new();
                let mut last_end = 0;
                for m in re.find_iter(line_text) {
                    if m.start() > last_end {
                        spans.push(span(&line_text[last_end..m.start()]).color(line_color));
                    }
                    spans.push(
                        span(&line_text[m.start()..m.end()])
                            .color(hl_fg)
                            .background(hl_bg),
                    );
                    last_end = m.end();
                }
                if last_end < line_text.len() {
                    spans.push(span(&line_text[last_end..]).color(line_color));
                }
                spans
            } else {
                vec![span(*line_text).color(line_color)]
            }
        } else {
            vec![span(*line_text).color(line_color)]
        };

        // ── Row background ────────────────────────────────────────────────────
        let extra_hl_bg: Option<Color> = if !is_search_match && !is_selected {
            extra_highlights
                .iter()
                .find(|eh| eh.regex.as_ref().map_or(false, |re| re.is_match(line_text)))
                .map(|eh| Color { a: 0.10, ..eh.color })
        } else {
            None
        };

        let row_bg = if is_selected {
            Some(p.selected_line)
        } else if is_search_match {
            Some(p.search_row_bg)
        } else {
            extra_hl_bg
        };

        // In wrap mode the text container fills the row width.
        let text_w = if line_wrap { Length::Fill } else { Length::Shrink };
        let text_widget = container(rich_text(text_spans).size(font_size))
            .width(text_w)
            .padding(iced::Padding { top: 1.0, right: 8.0, bottom: 1.0, left: 0.0 })
            .style(move |_: &iced::Theme| container::Style {
                background: row_bg.map(|c| c.into()),
                ..Default::default()
            });

        // Content (line number + text) is clickable for line selection
        let content_row = row![ln_widget, text_widget]
            .width(row_width)
            .height(Length::Shrink);
        let clickable_content = mouse_area(content_row)
            .on_press(Message::LineClicked(line_num))
            .interaction(iced::mouse::Interaction::Text);

        // Full row: [gutter (clickable for bookmark)] [content (clickable for selection)]
        let full_row = row![gutter, clickable_content]
            .width(row_width)
            .height(Length::Shrink);

        rows.push(full_row.into());
    }

    // ── Scroll position indicator ─────────────────────────────────────────────
    let scroll_pct = if total_lines > 0 {
        (scroll_offset as f64 / total_lines as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let sbg     = p.bg_secondary;
    let sborder = p.border;
    let muted   = p.fg_muted;
    let status_bar = container(
        row![
            text(format!(
                "Lines {}-{} of {}",
                scroll_offset + 1,
                (scroll_offset + viewport_lines).min(total_lines),
                total_lines,
            ))
            .size(11)
            .color(muted),
            text(format!("  {:.0}%", scroll_pct)).size(11).color(muted),
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .padding([4, 12])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(sbg.into()),
        border: iced::Border { color: sborder, width: 1.0, radius: 0.0.into() },
        ..Default::default()
    });

    let pbg = p.bg_primary;
    let log_content = Column::with_children(rows);

    // ── Content area: horizontal scroll when not wrapping ────────────────────
    let content_area: Element<'a, Message> = if line_wrap {
        container(log_content.width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(pbg.into()),
                ..Default::default()
            })
            .into()
    } else {
        let h_scroll = scrollable(log_content.width(Length::Shrink))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(6).scroller_width(6),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme, _status| scrollable::Style {
                container: container::Style {
                    background: Some(pbg.into()),
                    ..Default::default()
                },
                vertical_rail:   scrollable::Rail {
                    background: None,
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        color: Color::TRANSPARENT,
                        border: iced::Border::default(),
                    },
                },
                horizontal_rail: scrollable::Rail {
                    background: Some(p.bg_secondary.into()),
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        color: Color::from_rgba(0.6, 0.6, 0.6, 0.45),
                        border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    },
                },
                gap: None,
            });
        h_scroll.into()
    };

    let minimap   = build_minimap(scroll_offset, total_lines, viewport_lines, search_result_set, bookmarks, p);
    let main_area = row![content_area, minimap].height(Length::Fill);

    column![main_area, status_bar].into()
}

// ── Minimap (density ruler + viewport indicator) ──────────────────────────────

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

    for &line in search_result_set {
        has_search[to_bucket(line)] = true;
    }
    for &line in bookmarks {
        has_bm[to_bucket(line)] = true;
    }

    // Viewport bucket range
    let vp_start = to_bucket(scroll_offset);
    let vp_end   = to_bucket((scroll_offset + viewport_lines).min(total_lines.saturating_sub(1)));

    let sm_color = p.search_gutter;
    let bm_color = p.accent;
    let vp_color = Color::from_rgba(0.65, 0.65, 0.70, 0.28);

    let get_color = |i: usize| -> Color {
        let in_vp = i >= vp_start && i <= vp_end;
        if has_bm[i] {
            if in_vp {
                // Dim bookmark color slightly inside viewport
                Color { r: bm_color.r * 0.75, g: bm_color.g * 0.75, b: bm_color.b * 0.75, a: 1.0 }
            } else {
                bm_color
            }
        } else if has_search[i] {
            if in_vp {
                Color { r: sm_color.r * 0.75, g: sm_color.g * 0.75, b: sm_color.b * 0.75, a: sm_color.a }
            } else {
                sm_color
            }
        } else if in_vp {
            vp_color
        } else {
            sbg
        }
    };

    // Build merged segments (consecutive same-color buckets → one FillPortion each)
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
