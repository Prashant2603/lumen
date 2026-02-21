use std::path::PathBuf;

use iced::widget::{button, column, container, mouse_area, row, scrollable, text};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::theme::Palette;

pub fn view<'a>(recent_files: &'a [PathBuf], p: Palette) -> Element<'a, Message> {
    let bg  = p.bg_primary;
    let bg_s = p.bg_surface;
    let bdr = p.border;
    let fg  = p.fg_primary;
    let fgm = p.fg_muted;
    let acc = p.accent;
    let bgh = p.bg_hover;

    let close_btn = button(text("x").size(13).color(fgm))
        .on_press(Message::ToggleRecentFiles)
        .padding([3, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bgh.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: fgm,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        });

    let header = row![
        text("Recent Files").size(14).color(fg),
        iced::widget::horizontal_space(),
        close_btn,
    ]
    .align_y(iced::alignment::Vertical::Center);

    let mut list = column![].spacing(2);

    if recent_files.is_empty() {
        list = list.push(
            container(text("No recent files yet.").size(13).color(fgm))
                .padding([8, 0]),
        );
    } else {
        for path in recent_files {
            let file_name = path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dir_str = path.parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let path_clone = path.clone();
            let row_btn = button(
                column![
                    text(file_name).size(13).color(fg),
                    text(shorten_path(&dir_str, 40)).size(11).color(fgm),
                ]
                .spacing(1),
            )
            .on_press(Message::FileDrop(path_clone))
            .padding([7, 12])
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

            list = list.push(row_btn);
        }
    }

    let divider = container(text("").size(1))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Color { a: 0.3, ..bdr }.into()),
            ..Default::default()
        });

    let card = container(
        column![
            header,
            divider,
            scrollable(list.padding([4, 0])).height(Length::Shrink),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fixed(380.0)),
    )
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 8.0.into() },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    });

    let _ = (bg_s, acc); // suppress unused warnings

    mouse_area(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &iced::Theme| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.50).into()),
                ..Default::default()
            }),
    )
    .on_press(Message::ToggleRecentFiles)
    .into()
}

fn shorten_path(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let tail: String = s.chars().rev().take(max - 3).collect::<String>().chars().rev().collect();
        format!("...{}", tail)
    }
}
