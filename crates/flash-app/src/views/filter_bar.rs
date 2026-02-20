use flash_core::LogLevel;
use iced::widget::{button, container, row, text, text_input};
use iced::{Color, Element, Length};

use crate::app::{ExtraHighlight, Message, HIGHLIGHT_COLORS};
use crate::theme::Palette;

pub fn view<'a>(
    hidden_levels: &std::collections::HashSet<LogLevel>,
    line_filter: &str,
    has_file: bool,
    active_filter_count: Option<usize>,
    extra_highlights: &[ExtraHighlight],
    search_query: &str,
    p: Palette,
) -> Element<'a, Message> {
    let levels: [(LogLevel, &str, Color); 5] = [
        (LogLevel::Trace, "TRACE", p.log_trace),
        (LogLevel::Debug, "DEBUG", p.log_debug),
        (LogLevel::Info,  "INFO",  p.log_info),
        (LogLevel::Warn,  "WARN",  p.log_warn),
        (LogLevel::Error, "ERROR", p.log_error),
    ];

    let bg2 = p.bg_secondary;
    let bdr = p.border;
    let bgh = p.bg_hover;
    let bgp = p.bg_primary;
    let fgm = p.fg_muted;
    let acc = p.accent;

    let mut bar = row![].spacing(5).padding([4, 12]);

    for (level, label, color) in levels {
        let hidden = hidden_levels.contains(&level);
        let fg_color = if hidden { fgm } else { color };
        let bg_color = if hidden {
            bgp
        } else {
            Color::from_rgba(color.r, color.g, color.b, 0.14)
        };
        let border_color = if hidden {
            bdr
        } else {
            Color { a: 0.5, ..color }
        };

        let btn = button(text(label).size(11).color(fg_color))
            .on_press(Message::ToggleLevelFilter(level))
            .padding([3, 8])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => bgh.into(),
                    _ => bg_color.into(),
                }),
                text_color: fg_color,
                border: iced::Border {
                    color: border_color,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                shadow: iced::Shadow::default(),
            });
        bar = bar.push(btn);
    }

    bar = bar.push(divider(p));

    // Quick filter input
    let filter_input = text_input("Filter lines…", line_filter)
        .on_input(Message::LineFilterChanged)
        .on_submit(Message::Noop)
        .padding([3, 8])
        .size(12)
        .width(Length::Fixed(200.0))
        .style(move |_: &iced::Theme, status| text_input::Style {
            background: bgp.into(),
            border: iced::Border {
                color: match status {
                    text_input::Status::Focused => acc,
                    _ => bdr,
                },
                width: 1.0,
                radius: 3.0.into(),
            },
            icon: fgm,
            placeholder: fgm,
            value: p.fg_primary,
            selection: acc,
        });
    bar = bar.push(filter_input);

    // Clear filters button (visible only when something is active)
    if has_file && (!line_filter.is_empty() || !hidden_levels.is_empty()) {
        let clear_btn = button(text("✕ Filters").size(11))
            .on_press(Message::ClearFilters)
            .padding([3, 8])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => bgh.into(),
                    _ => bgp.into(),
                }),
                text_color: fgm,
                border: iced::Border { color: bdr, width: 1.0, radius: 3.0.into() },
                shadow: iced::Shadow::default(),
            });
        bar = bar.push(clear_btn);
    }

    // Filter result count
    if let Some(count) = active_filter_count {
        bar = bar.push(
            text(format!("{} lines", count)).size(11).color(acc),
        );
    }

    // ── Extra highlights section ──────────────────────────────────────────────
    let can_add = has_file
        && !search_query.is_empty()
        && extra_highlights.len() < HIGHLIGHT_COLORS.len();

    if can_add || !extra_highlights.is_empty() {
        bar = bar.push(divider(p));
    }

    // "+" button — adds the current search query as a new highlight
    if can_add {
        let next_color = HIGHLIGHT_COLORS[extra_highlights.len()];
        let add_btn = button(
            text("+ Highlight").size(11).color(next_color),
        )
        .on_press(Message::AddHighlight)
        .padding([3, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => Color { a: 0.25, ..next_color }.into(),
                _ => Color { a: 0.12, ..next_color }.into(),
            }),
            text_color: next_color,
            border: iced::Border {
                color: Color { a: 0.45, ..next_color },
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        });
        bar = bar.push(add_btn);
    }

    // Existing highlight chips (click to remove)
    for (idx, eh) in extra_highlights.iter().enumerate() {
        let hl_color = eh.color;
        let label = shorten(&eh.pattern, 14);
        let chip = button(
            row![
                text(label).size(11),
                text(" ×").size(11),
            ]
            .spacing(0),
        )
        .on_press(Message::RemoveHighlight(idx))
        .padding([3, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => Color { a: 0.55, ..hl_color }.into(),
                _ => Color { a: 0.35, ..hl_color }.into(),
            }),
            text_color: Color::from_rgb(0.0, 0.0, 0.0),
            border: iced::Border {
                color: Color { a: 0.7, ..hl_color },
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        });
        bar = bar.push(chip);
    }

    container(bar)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bg2.into()),
            border: iced::Border { color: bdr, width: 1.0, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
}

fn divider<'a>(p: Palette) -> Element<'a, Message> {
    container(text("").size(1))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(18.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(p.border.into()),
            ..Default::default()
        })
        .into()
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
