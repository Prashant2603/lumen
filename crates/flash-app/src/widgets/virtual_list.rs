/// Virtual scrolling constants and helpers.

/// Line height for a given font size (same 13→18px ratio as before, scaled).
pub fn line_height_for_font(font_size: f32) -> f32 {
    (font_size * 1.385).max(10.0)
}

/// Number of visible lines based on window height and current font size.
pub fn visible_lines_for_font(viewport_height: f32, font_size: f32) -> usize {
    (viewport_height / line_height_for_font(font_size)).ceil() as usize
}

/// Clamp scroll offset to valid range.
pub fn clamp_offset(offset: usize, total_lines: usize, viewport_lines: usize) -> usize {
    if total_lines <= viewport_lines {
        return 0;
    }
    offset.min(total_lines.saturating_sub(viewport_lines))
}
