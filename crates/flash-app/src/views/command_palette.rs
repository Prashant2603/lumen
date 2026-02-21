use iced::widget::{button, column, container, mouse_area, row, text, text_input, Column};
use iced::{Color, Element, Length};

use crate::app::{Message, PaletteCmd};
use crate::theme::Palette;

pub fn view<'a>(
    query:    &str,
    cmds:     Vec<PaletteCmd>,
    selected: usize,
    p:        Palette,
) -> Element<'a, Message> {
    let bg_p  = p.bg_primary;
    let bg_s  = p.bg_surface;
    let bg_h  = p.bg_hover;
    let fg    = p.fg_primary;
    let fgm   = p.fg_muted;
    let bdr   = p.border;
    let acc   = p.accent;

    // Search input
    let input = text_input("Type a command…", query)
        .id(text_input::Id::new("palette_input"))
        .on_input(Message::PaletteQueryChanged)
        .on_submit(Message::PaletteSelect)
        .padding([10, 14])
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
                radius: 0.0.into(),
            },
            icon:        fgm,
            placeholder: fgm,
            value:       fg,
            selection:   acc,
        });

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    let total = cmds.len();

    for (idx, cmd) in cmds.into_iter().enumerate() {
        let is_sel = idx == selected;
        let row_bg = if is_sel { Some(bg_h) } else { None };
        let label_color = if is_sel { fg } else { fg };
        let hint_color  = fgm;

        let row_widget = container(
            row![
                text(cmd.label).size(13).color(label_color).width(Length::Fill),
                text(cmd.shortcut).size(11).color(hint_color),
            ]
            .spacing(16)
            .align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .padding([6, 14])
        .style(move |_: &iced::Theme| container::Style {
            background: row_bg.map(|c| c.into()),
            ..Default::default()
        });

        let btn = button(row_widget)
            .on_press(Message::PaletteSelect)
            .padding(0)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(bg_h.into()),
                    _ => None,
                },
                text_color: fg,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
            });

        rows.push(btn.into());

        if idx + 1 < total {
            rows.push(
                container(text("").size(1))
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .style(move |_: &iced::Theme| container::Style {
                        background: Some(Color { a: 0.3, ..bdr }.into()),
                        ..Default::default()
                    })
                    .into(),
            );
        }
    }

    let list_height = (total as f32 * 31.0).min(400.0);

    let card = container(
        column![
            input,
            container(text("").size(1))
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(bdr.into()),
                    ..Default::default()
                }),
            container(Column::with_children(rows).width(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fixed(list_height)),
        ]
        .width(Length::Fixed(480.0)),
    )
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg_s.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 8.0.into() },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..Default::default()
    });

    // Backdrop: click anywhere outside the card to close
    mouse_area(
        container(
            container(card)
                .center_x(Length::Fill)
                .padding(iced::Padding { top: 80.0, right: 0.0, bottom: 0.0, left: 0.0 }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &iced::Theme| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
            ..Default::default()
        }),
    )
    .on_press(Message::PaletteClose)
    .into()
}
