use iced::widget::{button, column, container, row, text, text_input};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::theme::Palette;

pub fn view<'a>(input: &str, total_lines: usize, p: Palette) -> Element<'a, Message> {
    let bg_p = p.bg_primary;
    let bg_s = p.bg_surface;
    let bg_h = p.bg_hover;
    let fg   = p.fg_primary;
    let fgm  = p.fg_muted;
    let bdr  = p.border;
    let acc  = p.accent;

    let hint = if total_lines > 0 {
        format!("Enter line number (1–{})", total_lines)
    } else {
        "Enter line number".to_string()
    };

    let text_in = text_input(&hint, input)
        .on_input(Message::JumpInputChanged)
        .on_submit(Message::JumpSubmit)
        .padding([8, 12])
        .size(14)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| iced::widget::text_input::Style {
            background: bg_p.into(),
            border: iced::Border {
                color: match status {
                    iced::widget::text_input::Status::Focused => acc,
                    _ => bdr,
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            icon:        fgm,
            placeholder: fgm,
            value:       fg,
            selection:   acc,
        });

    let btn_style = move |_: &iced::Theme, status: button::Status| button::Style {
        background: Some(match status {
            button::Status::Hovered | button::Status::Pressed => bg_h.into(),
            _ => bg_s.into(),
        }),
        text_color: fg,
        border: iced::Border { color: bdr, width: 1.0, radius: 4.0.into() },
        shadow: iced::Shadow::default(),
    };

    let go_btn = button(text("Go").size(13).color(fg))
        .on_press(Message::JumpSubmit)
        .padding([6, 16])
        .style(btn_style);

    let cancel_btn = button(text("Cancel").size(13).color(fgm))
        .on_press(Message::JumpClose)
        .padding([6, 16])
        .style(btn_style);

    let card = container(
        column![
            text("Jump to Line").size(15).color(fg),
            text_in,
            row![iced::widget::horizontal_space(), cancel_btn, go_btn]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
        ]
        .spacing(12)
        .padding(20)
        .width(Length::Fixed(340.0)),
    )
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg_s.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 8.0.into() },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    });

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
