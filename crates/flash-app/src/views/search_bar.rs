use iced::widget::{button, container, row, text, text_input};
use iced::{Color, Element, Length};

use crate::app::{self, Message};
use crate::theme::Palette;

pub fn view<'a>(
    search_query: &str,
    result_count: usize,
    search_in_progress: bool,
    has_file: bool,
    file_info: Option<(&str, u64, usize)>,
    tail_mode: bool,
    p: Palette,
) -> Element<'a, Message> {
    let bg_h = p.bg_hover;
    let fg   = p.fg_primary;
    let fgm  = p.fg_muted;
    let bdr  = p.border;
    let acc  = p.accent;
    let bg2  = p.bg_secondary;

    // Subtle ghost button: transparent bg, muted label — highlights on hover.
    let ghost = move |_: &iced::Theme, status: button::Status| button::Style {
        background: Some(match status {
            button::Status::Hovered | button::Status::Pressed => bg_h.into(),
            _ => Color::TRANSPARENT.into(),
        }),
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => fg,
            _ => fgm,
        },
        border: iced::Border {
            color: match status {
                button::Status::Hovered => Color { a: 0.35, ..bdr },
                _ => Color::TRANSPARENT,
            },
            width: 1.0,
            radius: 4.0.into(),
        },
        shadow: iced::Shadow::default(),
    };

    // Open button — always present, slightly more prominent
    let open_btn = button(text("Open").size(12))
        .on_press(Message::FileOpen)
        .padding([5, 11])
        .style(ghost);

    // Command-palette / menu button — ≡ opens all other actions
    let menu_btn = button(text("≡").size(15))
        .on_press(Message::PaletteOpen)
        .padding([4, 10])
        .style(ghost);

    // Live tail toggle — glows green when active
    let live_green = Color::from_rgb(0.35, 0.85, 0.35);
    let tail_btn = button(
        text(if tail_mode { "● LIVE" } else { "Tail" }).size(11),
    )
    .on_press(Message::TailToggle)
    .padding([5, 11])
    .style(move |_: &iced::Theme, status: button::Status| {
        if tail_mode {
            button::Style {
                background: Some(Color::from_rgba(0.20, 0.60, 0.20, 0.18).into()),
                text_color: live_green,
                border: iced::Border {
                    color: Color::from_rgba(0.20, 0.60, 0.20, 0.40),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow::default(),
            }
        } else {
            button::Style {
                background: Some(match status {
                    button::Status::Hovered => bg_h.into(),
                    _ => Color::TRANSPARENT.into(),
                }),
                text_color: fgm,
                border: iced::Border { color: Color::TRANSPARENT, width: 0.0, radius: 4.0.into() },
                shadow: iced::Shadow::default(),
            }
        }
    });

    // Regex search input — fills available space
    let input = text_input("regex search…", search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmit)
        .padding([6, 10])
        .size(13)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| text_input::Style {
            background: p.bg_primary.into(),
            border: iced::Border {
                color: match status {
                    text_input::Status::Focused => acc,
                    _ => Color { a: 0.45, ..bdr },
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            icon:        fgm,
            placeholder: Color { a: 0.4, ..fgm },
            value:       fg,
            selection:   Color { a: 0.35, ..acc },
        });

    // Match count / in-progress status
    let status_txt = if search_in_progress {
        text(format!("⟳  {}", result_count)).size(11).color(Color { a: 0.7, ..acc })
    } else if result_count > 0 {
        text(format!("{}  hits", result_count)).size(11).color(acc)
    } else if !search_query.is_empty() && has_file {
        text("no match").size(11).color(Color { a: 0.5, ..fgm })
    } else {
        text("").size(11)
    };

    // ── Assemble toolbar ──────────────────────────────────────────────────────
    let mut bar = row![].spacing(2).padding([5, 10]);

    // Left: action buttons
    bar = bar.push(open_btn);
    bar = bar.push(menu_btn);
    bar = bar.push(sep(p));

    // Centre-left: file metadata (only when a file is open)
    if let Some((name, size, lines)) = file_info {
        let info = text(format!(
            "{}  ·  {}  ·  {} lines",
            name,
            app::format_file_size(size),
            lines,
        ))
        .size(11)
        .color(Color { a: 0.50, ..fgm });

        bar = bar.push(container(info).padding([0, 10]));
        bar = bar.push(sep(p));
    }

    // Centre: search input + hit count (input fills all remaining space)
    bar = bar.push(container(input).padding([0, 6]).width(Length::Fill));
    bar = bar.push(container(status_txt).padding([0, 4]));

    // Clear button — only shown while a query is active
    if !search_query.is_empty() {
        let clear_btn = button(text("×").size(15).color(fgm))
            .on_press(Message::Clear)
            .padding([3, 8])
            .style(ghost);
        bar = bar.push(clear_btn);
    }

    // Right: tail indicator
    bar = bar.push(sep(p));
    bar = bar.push(tail_btn);

    container(bar)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bg2.into()),
            border: iced::Border {
                color: Color { a: 0.25, ..bdr },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

// 1 px vertical separator, slightly transparent
fn sep<'a>(p: Palette) -> Element<'a, Message> {
    container(text("").size(1))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(20.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Color { a: 0.30, ..p.border }.into()),
            ..Default::default()
        })
        .into()
}
