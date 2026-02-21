use iced::widget::{column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use crate::app::{format_file_size, Message};
use crate::theme::Palette;
use flash_core::LogLevel;

pub fn view<'a>(
    file_name:      Option<String>,
    file_size:      Option<u64>,
    total_lines:    Option<usize>,
    selected_line:  Option<usize>,
    selected_text:  Option<String>,
    selected_level: Option<LogLevel>,
    scroll_offset:  usize,
    viewport_lines: usize,
    visible_lines:  usize,
    proc_mem_mb:    f64,
    proc_cpu_pct:   f64,
    p:              Palette,
) -> Element<'a, Message> {
    let bg  = p.bg_secondary;
    let bdr = p.border;
    let fgm = p.fg_muted;

    // ── Builders ─────────────────────────────────────────────────────────────

    fn section_header<'a>(label: &'static str, fgm: Color) -> Element<'a, Message> {
        container(
            text(label)
                .size(10)
                .color(Color { a: 0.55, ..fgm }),
        )
        .padding(iced::Padding { top: 10.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .width(Length::Fill)
        .into()
    }

    fn kv_row<'a>(key: &'static str, val: String, fgm: Color, fg: Color) -> Element<'a, Message> {
        row![
            text(key).size(11).color(fgm).width(Length::Fixed(64.0)),
            text(val).size(11).color(fg),
        ]
        .spacing(4)
        .padding([2, 12])
        .into()
    }

    fn sep<'a>(bdr: Color) -> Element<'a, Message> {
        container(text("").size(1))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Color { a: 0.28, ..bdr }.into()),
                ..Default::default()
            })
            .into()
    }

    let mut col = column![].spacing(0);

    // ── FILE section ─────────────────────────────────────────────────────────
    col = col.push(section_header("FILE", fgm));
    match &file_name {
        Some(name) => {
            col = col.push(kv_row("Name", name.clone(), fgm, p.fg_primary));
            if let Some(sz) = file_size {
                col = col.push(kv_row("Size", format_file_size(sz), fgm, p.fg_primary));
            }
            if let Some(lns) = total_lines {
                col = col.push(kv_row("Lines", format!("{}", lns), fgm, p.fg_primary));
            }
        }
        None => {
            col = col.push(
                container(text("No file open").size(11).color(fgm))
                    .padding([2, 12])
                    .width(Length::Fill),
            );
        }
    }

    col = col.push(sep(bdr));

    // ── VIEW section ─────────────────────────────────────────────────────────
    col = col.push(section_header("VIEW", fgm));
    if visible_lines > 0 {
        let pct = (scroll_offset as f64 / visible_lines.max(1) as f64 * 100.0).min(100.0);
        col = col.push(kv_row("Scroll", format!("{:.0}%", pct),         fgm, p.fg_primary));
        col = col.push(kv_row("Window", format!("{} lines", viewport_lines), fgm, p.fg_primary));
        col = col.push(kv_row("Total",  format!("{} lines", visible_lines),  fgm, p.fg_primary));
    } else {
        col = col.push(
            container(text("—").size(11).color(fgm))
                .padding([2, 12])
                .width(Length::Fill),
        );
    }

    col = col.push(sep(bdr));

    // ── SELECTION section ─────────────────────────────────────────────────────
    col = col.push(section_header("SELECTION", fgm));
    if let Some(n) = selected_line {
        col = col.push(kv_row("Line #", format!("{}", n + 1), fgm, p.fg_primary));

        if let Some(lvl) = selected_level {
            let lvl_str = match lvl {
                LogLevel::Trace => "TRACE",
                LogLevel::Debug => "DEBUG",
                LogLevel::Info  => "INFO",
                LogLevel::Warn  => "WARN",
                LogLevel::Error => "ERROR",
            };
            let lvl_color = p.log_level_color(lvl);
            col = col.push(
                row![
                    text("Level").size(11).color(fgm).width(Length::Fixed(64.0)),
                    text(lvl_str).size(11).color(lvl_color),
                ]
                .spacing(4)
                .padding([2, 12]),
            );
        }

        if let Some(txt) = selected_text {
            let preview: String = txt.chars().take(180).collect();
            let preview = if txt.len() > 180 {
                format!("{}…", preview)
            } else {
                preview
            };
            let acc = p.accent;
            col = col.push(
                container(text(preview).size(10).color(p.fg_primary))
                    .padding([6, 10])
                    .width(Length::Fill)
                    .style(move |_: &iced::Theme| container::Style {
                        background: Some(Color { a: 0.07, ..acc }.into()),
                        border: iced::Border {
                            color: Color { a: 0.18, ..acc },
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }
    } else {
        col = col.push(
            container(text("Click a line to inspect").size(11).color(fgm))
                .padding([2, 12])
                .width(Length::Fill),
        );
    }

    col = col.push(sep(bdr));

    // ── PROCESS section ───────────────────────────────────────────────────────
    col = col.push(section_header("PROCESS", fgm));
    let mem_str = if proc_mem_mb < 1024.0 {
        format!("{:.1} MB", proc_mem_mb)
    } else {
        format!("{:.2} GB", proc_mem_mb / 1024.0)
    };
    col = col.push(kv_row("Memory", mem_str, fgm, p.fg_primary));

    // CPU bar
    let cpu_color = if proc_cpu_pct > 50.0 { p.log_error }
                    else if proc_cpu_pct > 20.0 { p.log_warn }
                    else { p.log_info };
    let cpu_str = format!("{:.1}%", proc_cpu_pct);
    col = col.push(
        row![
            text("CPU").size(11).color(fgm).width(Length::Fixed(64.0)),
            container(text("").size(1))
                .width(Length::Fixed(((proc_cpu_pct / 100.0 * 80.0) as f32).clamp(1.0, 80.0)))
                .height(Length::Fixed(8.0))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(cpu_color.into()),
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            text(cpu_str).size(11).color(cpu_color),
        ]
        .spacing(6)
        .padding([2, 12])
        .align_y(iced::alignment::Vertical::Center),
    );

    // Trailing spacer so the panel feels padded at the bottom
    col = col.push(
        container(text("").size(1))
            .height(Length::Fixed(12.0))
            .width(Length::Fill),
    );

    // ── Panel container ───────────────────────────────────────────────────────
    container(
        scrollable(col.width(Length::Fill)).height(Length::Fill),
    )
    .width(Length::Fixed(220.0))
    .height(Length::Fill)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
            color: Color { a: 0.45, ..bdr },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}
