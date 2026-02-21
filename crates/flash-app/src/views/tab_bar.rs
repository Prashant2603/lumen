// Zed-style tab bar:
//  • Tab bar background = bg_secondary (darker chrome)
//  • Active tab background = bg_primary (same as the editor below → they merge visually)
//  • Inactive tab background = bg_surface
//  • 1-px right border between every tab
//  • 1-px bottom border on the whole tab bar (acts as the separator line)
//    — the active tab's bg_primary makes it appear "open", inactive tabs
//      sit behind the bottom line giving the "closed" look.
//  • No bottom accent stripe (that's the VSCode style, not Zed)

use iced::widget::{button, container, row, text};
use iced::{Color, Element, Length};

use crate::app::{Message, Tab};
use crate::theme::Palette;

pub fn view<'a>(tabs: &[Tab], active: usize, p: Palette) -> Element<'a, Message> {
    let bg_bar = p.bg_secondary;  // tab-bar chrome
    let bg_act = p.bg_primary;    // active tab = editor bg → visual merge
    let bg_in  = p.bg_surface;    // inactive tab
    let bg_h   = p.bg_hover;
    let fg_act = p.fg_primary;
    let fg_in  = p.fg_muted;
    let bdr    = p.border;

    let mut bar = row![].spacing(0);

    for (idx, tab) in tabs.iter().enumerate() {
        let is_active = idx == active;
        let name      = tab.file_name();
        let bg_col    = if is_active { bg_act } else { bg_in };
        let fg_col    = if is_active { fg_act } else { fg_in };

        // Close ×  — always shown, muted
        let close_hover_bg = if is_active { bg_h } else {
            Color { a: 0.6, ..bg_h }
        };
        let close_btn = button(text("x").size(11).color(fg_in))
            .on_press(Message::CloseTab(idx))
            .padding([1, 5])
            .style(move |_: &iced::Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => Some(close_hover_bg.into()),
                    _ => None,
                },
                text_color: fg_in,
                border: iced::Border { radius: 3.0.into(), ..Default::default() },
                shadow: iced::Shadow::default(),
            });

        // The tab interior: label + close
        let interior = container(
            row![
                text(shorten(&name, 24)).size(12).color(fg_col),
                close_btn,
            ]
            .spacing(2)
            .align_y(iced::alignment::Vertical::Center),
        )
        .padding(iced::Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 14.0 })
        .height(Length::Fill)
        .center_y(Length::Fill);

        // 1-px right border separating tabs
        let tab_with_border = row![
            interior,
            container(text("").size(1))
                .width(Length::Fixed(1.0))
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(bdr.into()),
                    ..Default::default()
                }),
        ]
        .height(Length::Fill);

        let tab_btn = button(tab_with_border)
            .on_press(Message::SwitchTab(idx))
            .padding(0)
            .height(Length::Fill)
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered if !is_active => bg_h.into(),
                    _ => bg_col.into(),
                }),
                text_color: fg_col,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
            });

        bar = bar.push(tab_btn);
    }

    // Spacer filling the rest of the tab bar
    bar = bar.push(
        container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(bg_bar.into()),
                ..Default::default()
            }),
    );

    // The whole bar sits on top of the editor. Its 1-px bottom border acts as
    // the separator line. The active tab's bg_primary colour makes it appear to
    // open up into the content below (Zed's visual trick).
    container(bar)
        .width(Length::Fill)
        .height(Length::Fixed(34.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bg_bar.into()),
            border: iced::Border {
                color:  bdr,
                width:  1.0,
                radius: 0.0.into(),
            },
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
