use iced::widget::{button, column, container, mouse_area, text};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::theme::Palette;

/// Small floating context menu that appears on right-click of a log line.
pub fn view<'a>(
    line_num:  usize,
    cursor_x:  f32,
    cursor_y:  f32,
    p:         Palette,
) -> Element<'a, Message> {
    let bg  = p.bg_surface;
    let bdr = p.border;
    let fg  = p.fg_primary;
    let fgm = p.fg_muted;
    let bgh = p.bg_hover;

    let copy_btn = button(
        column![
            text(format!("Copy  (line {})", line_num + 1)).size(13).color(fg),
        ]
    )
    .on_press(Message::CopyLine(line_num))
    .padding([8, 16])
    .width(Length::Fill)
    .style(move |_: &iced::Theme, status| button::Style {
        background: Some(match status {
            button::Status::Hovered | button::Status::Pressed => bgh.into(),
            _ => Color::TRANSPARENT.into(),
        }),
        text_color: fg,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
    });

    let dismiss_btn = button(text("Dismiss").size(12).color(fgm))
        .on_press(Message::CloseContextMenu)
        .padding([6, 16])
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bgh.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: fgm,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        });

    let menu = container(
        column![copy_btn, dismiss_btn].spacing(0).width(Length::Fixed(200.0)),
    )
    .padding(4)
    .width(Length::Fixed(200.0))
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 6.0.into() },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    });

    // Position the menu near the cursor; clamp so it doesn't go off-screen
    let menu_x = (cursor_x + 4.0).max(0.0);
    let menu_y = (cursor_y + 4.0).max(0.0);

    let positioned = container(menu)
        .padding(iced::Padding {
            top:    menu_y,
            left:   menu_x,
            right:  0.0,
            bottom: 0.0,
        })
        .width(Length::Fill)
        .height(Length::Fill);

    // Clicking the backdrop closes the menu
    mouse_area(positioned)
        .on_press(Message::CloseContextMenu)
        .on_right_press(Message::CloseContextMenu)
        .into()
}
