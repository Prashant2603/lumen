use flash_core::LayerKind;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::pipeline::{TransformPipeline, UiLayer};
use crate::theme::Palette;

pub fn view<'a>(
    pipeline:   &'a TransformPipeline,
    stale:      bool,
    preview_to: Option<u64>,
    p:          Palette,
) -> Element<'a, Message> {
    let bg  = p.bg_secondary;
    let bdr = p.border;
    let acc = p.accent;
    let fgm = p.fg_muted;
    let fg  = p.fg_primary;
    let bgh = p.bg_hover;

    // ── Header row ────────────────────────────────────────────────────────────
    let add_filter_btn = chip_btn("+ Filter", p.log_info, bgh)
        .on_press(Message::PipelineAddFilter);
    let add_rewrite_btn = chip_btn("+ Rewrite", p.log_warn, bgh)
        .on_press(Message::PipelineAddRewrite);
    let add_mask_btn = chip_btn("+ Mask", p.log_debug, bgh)
        .on_press(Message::PipelineAddMask);

    let close_btn = button(text("x").size(14).color(fgm))
        .on_press(Message::TogglePipeline)
        .padding([4, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => bgh.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: fgm,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        });

    let header = container(
        row![
            text("Pipeline").size(14).color(acc),
            add_filter_btn,
            add_rewrite_btn,
            add_mask_btn,
            iced::widget::Space::with_width(Length::Fill),
            close_btn,
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([6, 8])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(p.bg_surface.into()),
        border: iced::Border {
            color: Color { a: 0.3, ..bdr },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // ── Layer cards ───────────────────────────────────────────────────────────
    let mut layer_cards: Vec<Element<'a, Message>> = Vec::new();

    for (pos, ul) in pipeline.layers.iter().enumerate() {
        let id         = ul.layer.id;
        let enabled    = ul.layer.enabled;
        let is_last    = pos == pipeline.layers.len() - 1;
        let badge_color = layer_badge_color(ul, p);
        let dimmed_fg  = if enabled { fg } else { fgm };

        // Type badge
        let badge = container(
            text(ul.kind_label()).size(12).color(badge_color),
        )
        .padding([3, 7])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Color { a: 0.15, ..badge_color }.into()),
            border: iced::Border {
                color: Color { a: 0.4, ..badge_color },
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        });

        // Enabled checkbox (toggle button)
        let check_label = if enabled { "[on]" } else { "[off]" };
        let toggle_enabled = button(text(check_label).size(13).color(if enabled { acc } else { fgm }))
            .on_press(Message::PipelineToggleLayer(id))
            .padding([3, 8])
            .style(ghost_btn_style(bgh));

        // Up / Down / Delete buttons
        let up_btn = icon_btn("↑", id, bgh, fgm, pos == 0, Message::PipelineMoveLayer(id, -1));
        let dn_btn = icon_btn("↓", id, bgh, fgm, is_last, Message::PipelineMoveLayer(id, 1));
        let del_btn = button(text("del").size(12).color(p.log_error))
            .on_press(Message::PipelineRemoveLayer(id))
            .padding([4, 8])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => Color { a: 0.15, ..p.log_error }.into(),
                    _ => Color::TRANSPARENT.into(),
                }),
                text_color: p.log_error,
                border: iced::Border {
                    color: Color { a: 0.3, ..p.log_error },
                    width: 1.0,
                    radius: 3.0.into(),
                },
                shadow: iced::Shadow::default(),
            });

        let header_row = row![
            toggle_enabled,
            badge,
            iced::widget::Space::with_width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center);

        // Pattern input
        let pattern_placeholder = match &ul.layer.kind {
            LayerKind::Filter  { .. } => "regex pattern…",
            LayerKind::Rewrite { .. } => "find regex…",
            LayerKind::Mask    { .. } => "pattern to mask…",
        };
        let pattern_val = ul.draft_pattern.clone();
        let pattern_input = text_input(pattern_placeholder, &pattern_val)
            .on_input(move |s| Message::PipelineEditPattern(id, s))
            .on_submit(Message::PipelineCommitLayer(id))
            .padding([6, 8])
            .size(13)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, status| text_input::Style {
                background: p.bg_primary.into(),
                border: iced::Border {
                    color: match status {
                        text_input::Status::Focused => acc,
                        _ => Color { a: 0.4, ..bdr },
                    },
                    width: 1.0,
                    radius: 3.0.into(),
                },
                icon:        fgm,
                placeholder: Color { a: 0.35, ..fgm },
                value:       dimmed_fg,
                selection:   acc,
            });

        let mut card_col = column![header_row, pattern_input].spacing(4);

        // Include/Exclude toggle for Filter layers
        if let LayerKind::Filter { exclude, .. } = &ul.layer.kind {
            let excl       = *exclude;
            let incl_label = if excl { "Incl" } else { "Incl [v]" };
            let excl_label = if excl { "Excl [v]" } else { "Excl" };
            let incl_color = if excl { fgm } else { p.log_info };
            let excl_color = if excl { p.log_error } else { fgm };
            let excl_bg_i  = if excl { Color::TRANSPARENT } else { Color { a: 0.12, ..p.log_info } };
            let excl_bg_e  = if excl { Color { a: 0.12, ..p.log_error } } else { Color::TRANSPARENT };

            let incl_btn = button(text(incl_label).size(12).color(incl_color))
                .on_press(if excl { Message::PipelineToggleLayerExclude(id) } else { Message::Noop })
                .padding([4, 10])
                .style(move |_: &iced::Theme, _| button::Style {
                    background: Some(excl_bg_i.into()),
                    text_color: incl_color,
                    border: iced::Border { color: Color { a: 0.3, ..incl_color }, width: 1.0, radius: 5.0.into() },
                    shadow: iced::Shadow::default(),
                });
            let excl_btn = button(text(excl_label).size(12).color(excl_color))
                .on_press(if excl { Message::Noop } else { Message::PipelineToggleLayerExclude(id) })
                .padding([4, 10])
                .style(move |_: &iced::Theme, _| button::Style {
                    background: Some(excl_bg_e.into()),
                    text_color: excl_color,
                    border: iced::Border { color: Color { a: 0.3, ..excl_color }, width: 1.0, radius: 3.0.into() },
                    shadow: iced::Shadow::default(),
                });
            card_col = card_col.push(row![incl_btn, excl_btn].spacing(4));
        }

        // Extra input for Rewrite/Mask
        match &ul.layer.kind {
            LayerKind::Rewrite { .. } => {
                let extra_val = ul.draft_extra.clone();
                let extra_input = text_input("replacement…", &extra_val)
                    .on_input(move |s| Message::PipelineEditExtra(id, s))
                    .on_submit(Message::PipelineCommitLayer(id))
                    .padding([6, 8])
                    .size(13)
                    .width(Length::Fill)
                    .style(move |_: &iced::Theme, status| text_input::Style {
                        background: p.bg_primary.into(),
                        border: iced::Border {
                            color: match status {
                                text_input::Status::Focused => acc,
                                _ => Color { a: 0.4, ..bdr },
                            },
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                        icon:        fgm,
                        placeholder: Color { a: 0.35, ..fgm },
                        value:       dimmed_fg,
                        selection:   acc,
                    });
                card_col = card_col.push(extra_input);
            }
            LayerKind::Mask { .. } => {
                let extra_val = ul.draft_extra.clone();
                let extra_input = text_input("mask string…", &extra_val)
                    .on_input(move |s| Message::PipelineEditExtra(id, s))
                    .on_submit(Message::PipelineCommitLayer(id))
                    .padding([6, 8])
                    .size(13)
                    .width(Length::Fill)
                    .style(move |_: &iced::Theme, status| text_input::Style {
                        background: p.bg_primary.into(),
                        border: iced::Border {
                            color: match status {
                                text_input::Status::Focused => acc,
                                _ => Color { a: 0.4, ..bdr },
                            },
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                        icon:        fgm,
                        placeholder: Color { a: 0.35, ..fgm },
                        value:       dimmed_fg,
                        selection:   acc,
                    });
                card_col = card_col.push(extra_input);
            }
            _ => {}
        }

        // Parse error
        if let Some(err) = &ul.parse_error {
            let short_err: String = err.chars().take(60).collect();
            card_col = card_col.push(
                text(short_err).size(12).color(p.log_error),
            );
        }

        // Preview toggle button
        let preview_active = preview_to == Some(id);
        let prev_color     = if preview_active { acc } else { fgm };
        let prev_bg_rest   = if preview_active { Color { a: 0.15, ..acc } } else { Color::TRANSPARENT };
        let prev_bdr_alpha = if preview_active { 0.5 } else { 0.0 };
        let preview_btn = button(text(if preview_active { "[>] on" } else { "[>]" }).size(12).color(prev_color))
            .on_press(Message::PipelinePreviewLayer(if preview_active { None } else { Some(id) }))
            .padding([2, 5])
            .style(move |_: &iced::Theme, status| button::Style {
                background: Some(match status {
                    button::Status::Hovered => Color { a: 0.25, ..acc }.into(),
                    _ => prev_bg_rest.into(),
                }),
                text_color: prev_color,
                border: iced::Border {
                    color: Color { a: prev_bdr_alpha, ..acc },
                    width: 1.0,
                    radius: 3.0.into(),
                },
                shadow: iced::Shadow::default(),
            });

        // Navigation row
        let nav_row = row![
            up_btn, dn_btn,
            iced::widget::Space::with_width(Length::Fill),
            preview_btn,
            del_btn,
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center);
        card_col = card_col.push(nav_row);

        let card_bg     = p.bg_primary;
        let card_bdr    = if preview_active { Color { a: 0.6, ..acc } } else { Color { a: 0.25, ..bdr } };
        let card_bdr_w  = if preview_active { 1.5 } else { 1.0 };
        let card = container(card_col.width(Length::Fill))
            .width(Length::Fill)
            .padding([8, 10])
            .style(move |_: &iced::Theme| container::Style {
                background: Some(card_bg.into()),
                border: iced::Border {
                    color: card_bdr,
                    width: card_bdr_w,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });

        layer_cards.push(card.into());
    }

    // Empty state hint
    if pipeline.layers.is_empty() {
        layer_cards.push(
            container(
                text("Add layers with the buttons above.\nFilter keeps/excludes lines.\nRewrite replaces text.\nMask hides sensitive data.")
                    .size(13)
                    .color(fgm),
            )
            .padding([8, 8])
            .into(),
        );
    }

    // Computing indicator
    if stale {
        layer_cards.push(
            container(text("Computing…").size(13).color(acc))
                .padding([4, 8])
                .into(),
        );
    }

    let layers_col = column(layer_cards).spacing(6).width(Length::Fill);
    let layers_scroll = scrollable(layers_col.padding([6, 8]))
        .height(Length::Fill);

    let full = column![header, layers_scroll].width(Length::Fixed(290.0));

    container(full)
        .width(Length::Fixed(290.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(bg.into()),
            border: iced::Border {
                color: Color { a: 0.3, ..bdr },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

// ── Style helpers ─────────────────────────────────────────────────────────────

fn layer_badge_color(ul: &UiLayer, p: Palette) -> Color {
    match &ul.layer.kind {
        LayerKind::Filter  { .. } => p.log_info,
        LayerKind::Rewrite { .. } => p.log_warn,
        LayerKind::Mask    { .. } => p.log_debug,
    }
}

fn chip_btn<'a>(label: &'a str, color: Color, _hover_bg: Color) -> button::Button<'a, Message> {
    button(text(label).size(12).color(color))
        .padding([4, 10])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => Color { a: 0.25, ..color }.into(),
                _ => Color { a: 0.12, ..color }.into(),
            }),
            text_color: color,
            border: iced::Border {
                color: Color { a: 0.4, ..color },
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        })
}

fn ghost_btn_style(hover_bg: Color) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_: &iced::Theme, status| button::Style {
        background: Some(match status {
            button::Status::Hovered => hover_bg.into(),
            _ => Color::TRANSPARENT.into(),
        }),
        text_color: Color::WHITE,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
    }
}

fn icon_btn<'a>(
    label:    &'a str,
    _id:      u64,
    hover_bg: Color,
    fg:       Color,
    disabled: bool,
    msg:      Message,
) -> Element<'a, Message> {
    let alpha = if disabled { 0.3 } else { 0.7 };
    let color = Color { a: alpha, ..fg };
    let mut btn = button(text(label).size(13).color(color))
        .padding([4, 8])
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(match status {
                button::Status::Hovered if !disabled => hover_bg.into(),
                _ => Color::TRANSPARENT.into(),
            }),
            text_color: color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        });
    if !disabled {
        btn = btn.on_press(msg);
    }
    btn.into()
}
