use flash_core::LogLevel;
use iced::widget::{button, container, row, text, text_input};
use iced::{Color, Element, Length};

use crate::app::{ExtraHighlight, Message, HIGHLIGHT_COLORS};
use crate::theme::Palette;

pub fn view<'a>(
    hidden_levels:       &std::collections::HashSet<LogLevel>,
    line_filter:         &'a str,
    has_file:            bool,
    active_filter_count: Option<usize>,
    extra_highlights:    &[ExtraHighlight],
    line_wrap:           bool,
    filter_is_regex:     bool,
    tail_mode:           bool,
    recent_open:         bool,
    has_recent:          bool,
    search_on_enter:     bool,
    p:                   Palette,
) -> Element<'a, Message> {
    let _ = hidden_levels; // level filter handler code kept, UI hidden

    let bdr = p.border;
    let bgh = p.bg_hover;
    let bgp = p.bg_primary;
    let fgm = p.fg_muted;
    let acc = p.accent;
    let fg  = p.fg_primary;

    let mut bar = row![].spacing(6).padding([6, 12]);

    // ── Wrap toggle ───────────────────────────────────────────────────────────
    let wrap_fg  = if line_wrap { acc } else { fgm };
    let wrap_bg  = if line_wrap { Color { a: 0.15, ..acc } } else { Color::TRANSPARENT };
    let wrap_bdr = if line_wrap { Color { a: 0.50, ..acc } } else { Color { a: 0.35, ..bdr } };
    bar = bar.push(
        button(text("Wrap").size(13).color(wrap_fg))
            .on_press(Message::WrapToggle)
            .padding([5, 12])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => bgh.into(),
                    _ => wrap_bg.into(),
                }),
                text_color: wrap_fg,
                border: iced::Border { color: wrap_bdr, width: 1.0, radius: 5.0.into() },
                shadow: iced::Shadow::default(),
            }),
    );

    bar = bar.push(divider(p));

    // ── Search / filter input ─────────────────────────────────────────────────
    let placeholder = if filter_is_regex {
        if search_on_enter { "regex filter… (press Enter)" } else { "regex filter…" }
    } else {
        if search_on_enter { "filter… (press Enter)" } else { "filter…  (click .* for regex)" }
    };
    let filter_input = text_input(placeholder, line_filter)
        .on_input(Message::LineFilterChanged)
        .on_submit(Message::SearchSubmit)  // Enter runs background regex search + populates results panel
        .padding([6, 10])
        .size(13)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| text_input::Style {
            background: p.bg_secondary.into(),
            border: iced::Border {
                color: match status {
                    text_input::Status::Focused => acc,
                    _ => if filter_is_regex && !line_filter.is_empty() {
                        Color { a: 0.55, ..acc }
                    } else {
                        Color { a: 0.35, ..bdr }
                    },
                },
                width: 1.0,
                radius: 5.0.into(),
            },
            icon:        fgm,
            placeholder: Color { a: 0.35, ..fgm },
            value:       fg,
            selection:   acc,
        });
    bar = bar.push(filter_input);

    // ── Regex toggle .*  ──────────────────────────────────────────────────────
    let re_color = if filter_is_regex { acc } else { fgm };
    let re_bg    = if filter_is_regex { Color { a: 0.15, ..acc } } else { Color::TRANSPARENT };
    let re_bdr   = if filter_is_regex { Color { a: 0.50, ..acc } } else { Color { a: 0.35, ..bdr } };
    bar = bar.push(
        button(text(".*").size(12).color(re_color))
            .on_press(Message::ToggleFilterRegex)
            .padding([5, 10])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => bgh.into(),
                    _ => re_bg.into(),
                }),
                text_color: re_color,
                border: iced::Border { color: re_bdr, width: 1.0, radius: 5.0.into() },
                shadow: iced::Shadow::default(),
            }),
    );

    // ── Clear button ──────────────────────────────────────────────────────────
    if has_file && !line_filter.is_empty() {
        bar = bar.push(
            button(text("x").size(13).color(fgm))
                .on_press(Message::ClearFilters)
                .padding([5, 10])
                .style(move |_: &iced::Theme, status| button::Style {
                    background: Some(match status {
                        button::Status::Hovered => bgh.into(),
                        _ => bgp.into(),
                    }),
                    text_color: fgm,
                    border: iced::Border { color: Color { a: 0.28, ..bdr }, width: 1.0, radius: 5.0.into() },
                    shadow: iced::Shadow::default(),
                }),
        );
    }

    // ── Match count ───────────────────────────────────────────────────────────
    if let Some(count) = active_filter_count {
        bar = bar.push(
            container(text(format!("{} lines", count)).size(12).color(acc))
                .padding([0, 4]),
        );
    }

    bar = bar.push(divider(p));

    // ── Extra highlights ──────────────────────────────────────────────────────
    let can_add = has_file
        && !line_filter.is_empty()
        && extra_highlights.len() < HIGHLIGHT_COLORS.len();

    if can_add {
        let next_color = HIGHLIGHT_COLORS[extra_highlights.len()];
        bar = bar.push(
            button(text("+ Highlight").size(12).color(next_color))
                .on_press(Message::AddHighlight)
                .padding([5, 10])
                .style(move |_: &iced::Theme, status| button::Style {
                    background: Some(match status {
                        button::Status::Hovered => Color { a: 0.28, ..next_color }.into(),
                        _ => Color { a: 0.12, ..next_color }.into(),
                    }),
                    text_color: next_color,
                    border: iced::Border {
                        color: Color { a: 0.40, ..next_color },
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                }),
        );
    }

    for (idx, eh) in extra_highlights.iter().enumerate() {
        let hl_color = eh.color;
        let label    = shorten(&eh.pattern, 14);
        bar = bar.push(
            button(
                row![text(label).size(12), text(" x").size(12)].spacing(0),
            )
            .on_press(Message::RemoveHighlight(idx))
            .padding([5, 10])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => Color { a: 0.55, ..hl_color }.into(),
                    _ => Color { a: 0.35, ..hl_color }.into(),
                }),
                text_color: Color::from_rgb(0.0, 0.0, 0.0),
                border: iced::Border {
                    color: Color { a: 0.65, ..hl_color },
                    width: 1.0,
                    radius: 5.0.into(),
                },
                shadow: iced::Shadow::default(),
            }),
        );
    }

    // ── Flex spacer + Recent + Tail toggle ───────────────────────────────────
    bar = bar.push(iced::widget::Space::with_width(Length::Fill));

    // Recent files button (only when there are recent files)
    if has_recent {
        let rec_fg  = if recent_open { acc } else { fgm };
        let rec_bg  = if recent_open { Color { a: 0.15, ..acc } } else { Color::TRANSPARENT };
        let rec_bdr = if recent_open { Color { a: 0.50, ..acc } } else { Color { a: 0.35, ..bdr } };
        bar = bar.push(
            button(text("Recent").size(12).color(rec_fg))
                .on_press(Message::ToggleRecentFiles)
                .padding([5, 12])
                .style(move |_: &iced::Theme, status| button::Style {
                    background: Some(match status {
                        button::Status::Hovered => bgh.into(),
                        _ => rec_bg.into(),
                    }),
                    text_color: rec_fg,
                    border: iced::Border { color: rec_bdr, width: 1.0, radius: 5.0.into() },
                    shadow: iced::Shadow::default(),
                }),
        );
    }

    let live_green = Color::from_rgb(0.35, 0.85, 0.35);
    bar = bar.push(
        button(
            text(if tail_mode { "● LIVE" } else { "Tail" }).size(12),
        )
        .on_press(Message::TailToggle)
        .padding([5, 12])
        .style(move |_: &iced::Theme, status: button::Status| {
            if tail_mode {
                button::Style {
                    background: Some(Color::from_rgba(0.20, 0.60, 0.20, 0.18).into()),
                    text_color: live_green,
                    border: iced::Border {
                        color: Color::from_rgba(0.20, 0.60, 0.20, 0.40),
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                }
            } else {
                button::Style {
                    background: Some(match status {
                        button::Status::Hovered => bgh.into(),
                        _ => Color::TRANSPARENT.into(),
                    }),
                    text_color: fgm,
                    border: iced::Border { color: Color::TRANSPARENT, width: 0.0, radius: 5.0.into() },
                    shadow: iced::Shadow::default(),
                }
            }
        }),
    );

    container(bar)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bgp.into()),
            border: iced::Border {
                color: Color { a: 0.20, ..bdr },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn divider<'a>(p: Palette) -> Element<'a, Message> {
    container(text("").size(1))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(20.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Color { a: 0.30, ..p.border }.into()),
            ..Default::default()
        })
        .into()
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max - 1).collect::<String>())
    }
}
