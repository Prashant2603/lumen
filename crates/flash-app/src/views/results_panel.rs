use flash_core::LogLevel;
use iced::widget::{button, column, container, rich_text, row, scrollable, span, text, Column};
use iced::{Element, Length};

use crate::app::Message;
use crate::theme::Palette;

pub fn view<'a>(
    results: &[(usize, String)],
    selected_result: Option<usize>,
    search_in_progress: bool,
    compiled_regex: Option<&regex::Regex>,
    force_show: bool,
    p: Palette,
) -> Element<'a, Message> {
    if results.is_empty() && !search_in_progress && !force_show {
        return container(text("").size(1))
            .height(0)
            .width(Length::Fill)
            .into();
    }

    let bg_s  = p.bg_surface;
    let bdr   = p.border;
    let fgp   = p.fg_primary;
    let fgm   = p.fg_muted;
    let acc   = p.accent;
    let bgh   = p.bg_hover;
    let smb   = p.search_match_bg;
    let smf   = p.search_match_fg;
    let ln_c  = p.line_number;

    let header = container(
        row![
            text("Search Results").size(12).color(fgp),
            text(format!("  {} matches", results.len())).size(12).color(fgm),
        ],
    )
    .width(Length::Fill)
    .padding([5, 12])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(bg_s.into()),
        border: iced::Border { color: bdr, width: 1.0, radius: 0.0.into() },
        ..Default::default()
    });

    let max_display = 500.min(results.len());
    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(max_display);

    for (idx, (line_num, line_text)) in results.iter().take(max_display).enumerate() {
        let is_selected = selected_result == Some(idx);
        let level = LogLevel::detect(line_text);
        let line_color = match level {
            Some(l) => p.log_level_color(l),
            None    => fgp,
        };
        let line_num_str = format!("{:>6}  ", line_num + 1);
        // Use char-based truncation to avoid byte-boundary panics on multi-byte UTF-8
        let truncated: String = {
            let mut chars = line_text.chars();
            let s: String = chars.by_ref().take(200).collect();
            if chars.next().is_some() { format!("{}…", s) } else { s }
        };

        let spans = if let Some(re) = compiled_regex {
            let mut s = vec![span(line_num_str).color(ln_c)];
            let mut last = 0;
            for m in re.find_iter(&truncated) {
                if m.start() > last {
                    s.push(span(truncated[last..m.start()].to_string()).color(line_color));
                }
                s.push(
                    span(truncated[m.start()..m.end()].to_string())
                        .color(smf)
                        .background(smb),
                );
                last = m.end();
            }
            if last < truncated.len() {
                s.push(span(truncated[last..].to_string()).color(line_color));
            }
            s
        } else {
            vec![
                span(line_num_str).color(ln_c),
                span(truncated).color(line_color),
            ]
        };

        let result_row = button(
            container(rich_text(spans).size(13).font(iced::Font::MONOSPACE))
                .width(Length::Fill)
                .padding([2, 8]),
        )
        .on_press(Message::ResultClicked(idx))
        .padding(0)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, status| {
            let bg = if is_selected {
                Some(bgh)
            } else {
                match status {
                    button::Status::Hovered => Some(bgh),
                    _ => None,
                }
            };
            button::Style {
                background: bg.map(|c| c.into()),
                text_color: fgp,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
            }
        });

        rows.push(result_row.into());
    }

    if results.len() > max_display {
        rows.push(
            container(
                text(format!("… and {} more results", results.len() - max_display))
                    .size(11)
                    .color(fgm),
            )
            .padding([4, 12])
            .into(),
        );
    }

    // Search-in-progress row
    if search_in_progress {
        rows.push(
            container(text("Searching…").size(11).color(acc))
                .padding([4, 12])
                .into(),
        );
    }

    let results_list = scrollable(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(200.0));

    column![header, results_list].into()
}
