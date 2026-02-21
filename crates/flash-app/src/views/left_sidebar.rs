use iced::widget::{button, column, container, text};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::theme::Palette;

pub fn view<'a>(
    pipeline_open:   bool,
    info_panel_open: bool,
    has_file:        bool,
    p:               Palette,
) -> Element<'a, Message> {
    let bg  = p.bg_secondary;
    let bdr = p.border;

    // ── Helper: single activity-bar button ───────────────────────────────────
    fn sbtn<'a>(
        icon:    &'static str,
        label:   &'static str,
        msg:     Message,
        active:  bool,
        p:       Palette,
    ) -> Element<'a, Message> {
        let acc    = p.accent;
        let fgm    = p.fg_muted;
        let bgh    = p.bg_hover;
        let fg     = if active { acc } else { fgm };
        let btn_bg = if active { Color { a: 0.13, ..acc } } else { Color::TRANSPARENT };

        button(
            column![
                text(icon).size(16).color(fg),
                text(label).size(10).color(Color { a: if active { 1.0 } else { 0.70 }, ..fg }),
            ]
            .spacing(3)
            .align_x(iced::alignment::Horizontal::Center)
            .width(Length::Fill),
        )
        .on_press(msg)
        .padding([12, 4])
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bgh.into(),
                _                       => btn_bg.into(),
            }),
            text_color: fg,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        })
        .into()
    }

    // ── Helper: 1px horizontal divider ───────────────────────────────────────
    let div = |bdr: Color| -> Element<'a, Message> {
        container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Color { a: 0.35, ..bdr }.into()),
                ..Default::default()
            })
            .into()
    };

    // ── Assemble column ───────────────────────────────────────────────────────
    let mut col = column![].spacing(0).width(Length::Fill);

    col = col.push(sbtn("=",  "menu",     Message::PaletteOpen,    false,           p));
    col = col.push(sbtn("+",  "open",     Message::FileOpen,       false,           p));
    col = col.push(div(bdr));

    if has_file {
        col = col.push(sbtn("|>", "pipeline", Message::TogglePipeline,  pipeline_open,   p));
        col = col.push(sbtn("#",  "jump",     Message::JumpOpen,         false,           p));
        col = col.push(sbtn("i",  "info",     Message::ToggleInfoPanel,  info_panel_open, p));
        col = col.push(div(bdr));
    }

    // Flex spacer
    col = col.push(
        container(text("").size(1))
            .height(Length::Fill)
            .width(Length::Fill),
    );

    col = col.push(div(bdr));
    col = col.push(sbtn("*",  "prefs",    Message::ToggleSettings, false,           p));

    // ── Panel container ───────────────────────────────────────────────────────
    container(col)
        .width(Length::Fixed(56.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bg.into()),
            border: iced::Border {
                color: Color { a: 0.5, ..bdr },
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}
