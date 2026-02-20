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
    let bg_s  = p.bg_surface;
    let bg_h  = p.bg_hover;
    let fg    = p.fg_primary;
    let fgm   = p.fg_muted;
    let bdr   = p.border;
    let acc   = p.accent;
    let bg2   = p.bg_secondary;

    let btn_style = move |_: &iced::Theme, status: button::Status| button::Style {
        background: Some(match status {
            button::Status::Hovered | button::Status::Pressed => bg_h.into(),
            _ => bg_s.into(),
        }),
        text_color: fg,
        border: iced::Border { color: bdr, width: 1.0, radius: 4.0.into() },
        shadow: iced::Shadow::default(),
    };

    let open_btn = button(text("Open File").size(13))
        .on_press(Message::FileOpen)
        .padding([5, 12])
        .style(btn_style);

    let settings_btn = button(text("⚙ Settings").size(13))
        .on_press(Message::ToggleSettings)
        .padding([5, 12])
        .style(btn_style);

    let export_btn = if has_file {
        button(text("Export").size(13))
            .on_press(Message::Export)
            .padding([5, 12])
            .style(btn_style)
    } else {
        button(text("Export").size(13).color(fgm))
            .padding([5, 12])
            .style(btn_style)
    };

    let live_green = Color::from_rgb(0.40, 0.92, 0.40);
    let tail_btn = button(
        text(if tail_mode { "⏸ Tail" } else { "▶ Tail" }).size(13),
    )
    .on_press(Message::TailToggle)
    .padding([5, 12])
    .style(move |_: &iced::Theme, status: button::Status| {
        if tail_mode {
            button::Style {
                background: Some(Color::from_rgba(0.3, 0.7, 0.3, 0.22).into()),
                text_color: live_green,
                border: iced::Border {
                    color: Color::from_rgba(0.3, 0.7, 0.3, 0.55),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow::default(),
            }
        } else {
            button::Style {
                background: Some(match status {
                    button::Status::Hovered | button::Status::Pressed => bg_h.into(),
                    _ => bg_s.into(),
                }),
                text_color: fg,
                border: iced::Border { color: bdr, width: 1.0, radius: 4.0.into() },
                shadow: iced::Shadow::default(),
            }
        }
    });

    let input = text_input("Search (regex)...", search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmit)
        .padding(7)
        .size(13)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| text_input::Style {
            background: p.bg_primary.into(),
            border: iced::Border {
                color: match status {
                    text_input::Status::Focused => acc,
                    _ => bdr,
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: fgm,
            placeholder: fgm,
            value: fg,
            selection: acc,
        });

    let status_txt = if search_in_progress {
        text(format!("Searching… ({} found)", result_count)).size(12).color(fgm)
    } else if result_count > 0 {
        text(format!("{} matches", result_count)).size(12).color(acc)
    } else if !search_query.is_empty() && has_file {
        text("No matches").size(12).color(fgm)
    } else {
        text("").size(12)
    };

    let mut bar = row![].spacing(6).padding([6, 12]);
    bar = bar.push(open_btn);
    bar = bar.push(settings_btn);
    bar = bar.push(export_btn);
    bar = bar.push(tail_btn);
    bar = bar.push(divider(p));

    if let Some((name, size, lines)) = file_info {
        bar = bar.push(
            text(format!("{}  {}  {} lines", name, app::format_file_size(size), lines))
                .size(11)
                .color(fgm),
        );
        if tail_mode {
            bar = bar.push(
                text("● LIVE").size(11).color(live_green),
            );
        }
        bar = bar.push(divider(p));
    }

    bar = bar.push(input);
    bar = bar.push(status_txt);

    if has_file {
        let clear_btn = button(text("Clear").size(13))
            .on_press(Message::Clear)
            .padding([5, 12])
            .style(btn_style);
        bar = bar.push(clear_btn);
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
        .height(Length::Fixed(22.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(p.border.into()),
            ..Default::default()
        })
        .into()
}
