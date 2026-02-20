use iced::widget::{button, column, container, row, text};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::theme::{AppTheme, Palette};

/// Full-screen overlay containing a centred settings card.
pub fn view<'a>(app_theme: AppTheme, font_size: f32, line_wrap: bool, p: Palette) -> Element<'a, Message> {
    let bg_p  = p.bg_primary;
    let bg_s  = p.bg_surface;
    let bg_h  = p.bg_hover;
    let fg    = p.fg_primary;
    let fgm   = p.fg_muted;
    let bdr   = p.border;
    let acc   = p.accent;

    // ── Theme section ────────────────────────────────────────────────────────
    let theme_section = {
        let label = text("Theme").size(13).color(acc);
        let mut col = column![label].spacing(6);
        for &t in AppTheme::all() {
            let is_selected = t == app_theme;
            let dot_color = if is_selected { acc } else { fgm };
            let row_style = move |_: &iced::Theme| container::Style {
                background: if is_selected {
                    Some(Color { a: 0.15, ..acc }.into())
                } else {
                    None
                },
                border: iced::Border {
                    color: if is_selected {
                        Color { a: 0.4, ..acc }
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            };
            let theme_row = container(
                row![
                    // Simulated radio dot
                    container(text(if is_selected { "●" } else { "○" }).size(12).color(dot_color))
                        .width(Length::Fixed(20.0)),
                    text(t.name()).size(13).color(fg),
                ]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding([5, 10])
            .style(row_style);

            // Wrap in a button for click
            let theme_btn = button(theme_row)
                .on_press(Message::SetTheme(t))
                .padding(0)
                .width(Length::Fill)
                .style(move |_: &iced::Theme, _status| button::Style {
                    background: None,
                    text_color: fg,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                });
            col = col.push(theme_btn);
        }
        col
    };

    // ── Font size section ────────────────────────────────────────────────────
    let minus_btn = button(text("−").size(16).color(fg))
        .on_press(Message::ZoomOut)
        .padding([2, 10])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered | button::Status::Pressed => bg_h.into(),
                _ => bg_s.into(),
            }),
            text_color: fg,
            border: iced::Border { color: bdr, width: 1.0, radius: 3.0.into() },
            shadow: iced::Shadow::default(),
        });
    let plus_btn = button(text("+").size(14).color(fg))
        .on_press(Message::ZoomIn)
        .padding([2, 10])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered | button::Status::Pressed => bg_h.into(),
                _ => bg_s.into(),
            }),
            text_color: fg,
            border: iced::Border { color: bdr, width: 1.0, radius: 3.0.into() },
            shadow: iced::Shadow::default(),
        });
    let reset_btn = button(text("Reset").size(11).color(fgm))
        .on_press(Message::ZoomReset)
        .padding([2, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bg_h.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: fgm,
            border: iced::Border { color: bdr, width: 1.0, radius: 3.0.into() },
            shadow: iced::Shadow::default(),
        });
    let font_section = column![
        text("Font Size").size(13).color(acc),
        row![
            minus_btn,
            container(text(format!("{:.0} px", font_size)).size(13).color(fg))
                .width(Length::Fixed(52.0))
                .padding([2, 0])
                .center_x(Length::Fixed(52.0)),
            plus_btn,
            reset_btn,
        ]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center),
    ]
    .spacing(8);

    // ── Line wrap section ────────────────────────────────────────────────────
    let wrap_label   = if line_wrap { "On" } else { "Off" };
    let wrap_btn = button(
        row![
            container(text(if line_wrap { "☑" } else { "☐" }).size(13).color(if line_wrap { acc } else { fgm }))
                .width(Length::Fixed(20.0)),
            text(format!("Line Wrap  [{}]", wrap_label)).size(13).color(fg),
        ]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::WrapToggle)
    .padding([5, 10])
    .width(Length::Fill)
    .style(move |_: &iced::Theme, status| button::Style {
        background: Some(match status {
            button::Status::Hovered => bg_h.into(),
            _ => Color::TRANSPARENT.into(),
        }),
        text_color: fg,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
    });

    let wrap_section = column![
        text("Display").size(13).color(acc),
        wrap_btn,
    ]
    .spacing(6);

    // ── About ────────────────────────────────────────────────────────────────
    let about = column![
        text("Flash").size(13).color(acc),
        text("v0.1.0 · High-performance log viewer").size(11).color(fgm),
    ]
    .spacing(2);

    // ── Header ───────────────────────────────────────────────────────────────
    let close_btn = button(text("✕").size(14).color(fgm))
        .on_press(Message::ToggleSettings)
        .padding([2, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bg_h.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: fgm,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        });
    let header = row![
        text("Settings").size(15).color(fg),
        iced::widget::horizontal_space(),
        close_btn,
    ]
    .align_y(iced::alignment::Vertical::Center);

    // ── Card ─────────────────────────────────────────────────────────────────
    let mk_divider = move || -> Element<'a, Message> {
        container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(bdr.into()),
                ..Default::default()
            })
            .into()
    };

    let card = container(
        column![
            header,
            mk_divider(),
            theme_section,
            mk_divider(),
            font_section,
            mk_divider(),
            wrap_section,
            mk_divider(),
            about,
        ]
        .spacing(12)
        .padding(20)
        .width(Length::Fixed(300.0)),
    )
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg_p.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 8.0.into() },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    });

    // ── Backdrop + centred card ───────────────────────────────────────────────
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_: &iced::Theme| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
            ..Default::default()
        })
        .into()
}
