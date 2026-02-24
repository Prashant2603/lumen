use iced::Color;

// ── Theme enum ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    CatppuccinMocha,
    CatppuccinLatte,
    Nord,
    TokyoNight,
    GruvboxDark,
    /// Clean light theme inspired by Tailwind/React/Linear UIs
    LightWeb,
    /// Atom / VS Code One Dark Pro
    OneDark,
}

impl AppTheme {
    pub fn name(self) -> &'static str {
        match self {
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::Nord            => "Nord",
            Self::TokyoNight      => "Tokyo Night",
            Self::GruvboxDark     => "Gruvbox Dark",
            Self::LightWeb        => "Light (Web)",
            Self::OneDark         => "One Dark",
        }
    }

    #[allow(dead_code)]
    pub fn is_dark(self) -> bool {
        !matches!(self, Self::CatppuccinLatte | Self::LightWeb)
    }

    pub fn iced_theme(self) -> iced::Theme {
        match self {
            Self::CatppuccinMocha => iced::Theme::CatppuccinMocha,
            Self::CatppuccinLatte => iced::Theme::CatppuccinLatte,
            Self::Nord            => iced::Theme::Nord,
            Self::TokyoNight      => iced::Theme::TokyoNight,
            Self::GruvboxDark     => iced::Theme::GruvboxDark,
            Self::LightWeb        => iced::Theme::Light,
            Self::OneDark         => iced::Theme::Dark,
        }
    }

    pub fn all() -> &'static [AppTheme] {
        &[
            AppTheme::CatppuccinMocha,
            AppTheme::CatppuccinLatte,
            AppTheme::Nord,
            AppTheme::TokyoNight,
            AppTheme::GruvboxDark,
            AppTheme::LightWeb,
            AppTheme::OneDark,
        ]
    }
}

// ── Palette ───────────────────────────────────────────────────────────────────

/// All runtime colors for the current theme. Copy-type so it can be passed
/// cheaply to view functions and captured by style closures.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg_primary:      Color,
    pub bg_secondary:    Color,
    pub bg_surface:      Color,
    pub bg_hover:        Color,
    pub bg_alt_row:      Color,  // subtle alternating row stripe
    pub fg_primary:      Color,
    pub fg_muted:        Color,
    pub accent:          Color,
    pub border:          Color,
    pub log_trace:       Color,
    pub log_debug:       Color,
    pub log_info:        Color,
    pub log_warn:        Color,
    pub log_error:       Color,
    pub search_match_bg: Color,
    pub search_match_fg: Color,
    pub search_row_bg:   Color,
    pub search_gutter:   Color,
    pub line_number:     Color,
    pub selected_line:   Color,
    pub context_row_bg:  Color,
    pub selection_bg:    Color,   // character-level text selection highlight
}

impl Palette {
    pub fn log_level_color(self, level: flash_core::LogLevel) -> Color {
        match level {
            flash_core::LogLevel::Trace => self.log_trace,
            flash_core::LogLevel::Debug => self.log_debug,
            flash_core::LogLevel::Info  => self.log_info,
            flash_core::LogLevel::Warn  => self.log_warn,
            flash_core::LogLevel::Error => self.log_error,
        }
    }
}

pub fn palette(theme: AppTheme) -> Palette {
    match theme {
        AppTheme::CatppuccinMocha => mocha(),
        AppTheme::CatppuccinLatte => latte(),
        AppTheme::Nord            => nord(),
        AppTheme::TokyoNight      => tokyo_night(),
        AppTheme::GruvboxDark     => gruvbox_dark(),
        AppTheme::LightWeb        => light_web(),
        AppTheme::OneDark         => one_dark(),
    }
}

// ── Theme palettes ────────────────────────────────────────────────────────────

fn mocha() -> Palette {
    Palette {
        bg_primary:      c(0x1e, 0x1e, 0x2e),
        bg_secondary:    c(0x18, 0x18, 0x25),
        bg_surface:      c(0x31, 0x31, 0x44),
        bg_hover:        c(0x45, 0x45, 0x59),
        bg_alt_row:      c(0x21, 0x21, 0x31),
        fg_primary:      c(0xcd, 0xd6, 0xf4),
        fg_muted:        c(0x6c, 0x70, 0x86),
        accent:          c(0x89, 0xb4, 0xfa),
        border:          c(0x58, 0x5b, 0x70),
        log_trace:       c(0x6c, 0x70, 0x86),
        log_debug:       c(0x89, 0xb4, 0xfa),
        log_info:        c(0xa6, 0xe3, 0xa1),
        log_warn:        c(0xfa, 0xb3, 0x87),
        log_error:       c(0xf3, 0x8b, 0xa8),
        search_match_bg: c(0xff, 0xb8, 0x6c),
        search_match_fg: Color::BLACK,
        search_row_bg:   Color::from_rgba(0xf9 as f32/255., 0xe2 as f32/255., 0xaf as f32/255., 0.12),
        search_gutter:   c(0xff, 0xb8, 0x6c),
        line_number:     c(0x6c, 0x70, 0x86),
        selected_line:   Color::from_rgba(0x89 as f32/255., 0xb4 as f32/255., 0xfa as f32/255., 0.15),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x89 as f32/255., 0xb4 as f32/255., 0xfa as f32/255., 0.40),
    }
}

fn latte() -> Palette {
    Palette {
        bg_primary:      c(0xef, 0xf1, 0xf5),
        bg_secondary:    c(0xe6, 0xe9, 0xef),
        bg_surface:      c(0xcc, 0xd0, 0xda),
        bg_hover:        c(0xbc, 0xc0, 0xcc),
        bg_alt_row:      c(0xe9, 0xec, 0xf0),
        fg_primary:      c(0x4c, 0x4f, 0x69),
        fg_muted:        c(0x8c, 0x8f, 0xa1),
        accent:          c(0x1e, 0x66, 0xf5),
        border:          c(0xbc, 0xc0, 0xcc),
        log_trace:       c(0x8c, 0x8f, 0xa1),
        log_debug:       c(0x1e, 0x66, 0xf5),
        log_info:        c(0x40, 0xa0, 0x2b),
        log_warn:        c(0xdf, 0x8e, 0x1d),
        log_error:       c(0xd2, 0x0f, 0x39),
        search_match_bg: c(0xfe, 0x64, 0x0b),
        search_match_fg: Color::WHITE,
        search_row_bg:   Color::from_rgba(0xfe as f32/255., 0x64 as f32/255., 0x0b as f32/255., 0.10),
        search_gutter:   c(0xfe, 0x64, 0x0b),
        line_number:     c(0x8c, 0x8f, 0xa1),
        selected_line:   Color::from_rgba(0x1e as f32/255., 0x66 as f32/255., 0xf5 as f32/255., 0.12),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x1e as f32/255., 0x66 as f32/255., 0xf5 as f32/255., 0.40),
    }
}

fn nord() -> Palette {
    Palette {
        bg_primary:      c(0x2e, 0x34, 0x40),
        bg_secondary:    c(0x24, 0x28, 0x33),
        bg_surface:      c(0x43, 0x4c, 0x5e),
        bg_hover:        c(0x4c, 0x56, 0x6a),
        bg_alt_row:      c(0x2c, 0x32, 0x3e),
        fg_primary:      c(0xec, 0xef, 0xf4),
        fg_muted:        c(0x90, 0x99, 0xa6),
        accent:          c(0x88, 0xc0, 0xd0),
        border:          c(0x4c, 0x56, 0x6a),
        log_trace:       c(0x90, 0x99, 0xa6),
        log_debug:       c(0x88, 0xc0, 0xd0),
        log_info:        c(0xa3, 0xbe, 0x8c),
        log_warn:        c(0xeb, 0xcb, 0x8b),
        log_error:       c(0xbf, 0x61, 0x6a),
        search_match_bg: c(0xeb, 0xcb, 0x8b),
        search_match_fg: c(0x2e, 0x34, 0x40),
        search_row_bg:   Color::from_rgba(0xeb as f32/255., 0xcb as f32/255., 0x8b as f32/255., 0.12),
        search_gutter:   c(0xeb, 0xcb, 0x8b),
        line_number:     c(0x4c, 0x56, 0x6a),
        selected_line:   Color::from_rgba(0x88 as f32/255., 0xc0 as f32/255., 0xd0 as f32/255., 0.15),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x88 as f32/255., 0xc0 as f32/255., 0xd0 as f32/255., 0.40),
    }
}

fn tokyo_night() -> Palette {
    Palette {
        bg_primary:      c(0x1a, 0x1b, 0x26),
        bg_secondary:    c(0x16, 0x16, 0x1e),
        bg_surface:      c(0x24, 0x28, 0x3b),
        bg_hover:        c(0x2e, 0x31, 0x49),
        bg_alt_row:      c(0x1c, 0x1d, 0x28),
        fg_primary:      c(0xc0, 0xca, 0xf5),
        fg_muted:        c(0x56, 0x5f, 0x89),
        accent:          c(0x7a, 0xa2, 0xf7),
        border:          c(0x3b, 0x3d, 0x57),
        log_trace:       c(0x56, 0x5f, 0x89),
        log_debug:       c(0x7a, 0xa2, 0xf7),
        log_info:        c(0x9e, 0xce, 0x6a),
        log_warn:        c(0xe0, 0xaf, 0x68),
        log_error:       c(0xf7, 0x76, 0x8e),
        search_match_bg: c(0xe0, 0xaf, 0x68),
        search_match_fg: c(0x1a, 0x1b, 0x26),
        search_row_bg:   Color::from_rgba(0xe0 as f32/255., 0xaf as f32/255., 0x68 as f32/255., 0.12),
        search_gutter:   c(0xe0, 0xaf, 0x68),
        line_number:     c(0x3b, 0x3d, 0x57),
        selected_line:   Color::from_rgba(0x7a as f32/255., 0xa2 as f32/255., 0xf7 as f32/255., 0.15),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x7a as f32/255., 0xa2 as f32/255., 0xf7 as f32/255., 0.40),
    }
}

fn gruvbox_dark() -> Palette {
    Palette {
        bg_primary:      c(0x28, 0x28, 0x28),
        bg_secondary:    c(0x1d, 0x20, 0x21),
        bg_surface:      c(0x3c, 0x38, 0x36),
        bg_hover:        c(0x50, 0x49, 0x45),
        bg_alt_row:      c(0x2a, 0x2a, 0x2a),
        fg_primary:      c(0xeb, 0xdb, 0xb2),
        fg_muted:        c(0x92, 0x83, 0x74),
        accent:          c(0x83, 0xa5, 0x98),
        border:          c(0x50, 0x49, 0x45),
        log_trace:       c(0x92, 0x83, 0x74),
        log_debug:       c(0x83, 0xa5, 0x98),
        log_info:        c(0xb8, 0xbb, 0x26),
        log_warn:        c(0xfa, 0xbd, 0x2f),
        log_error:       c(0xfb, 0x49, 0x34),
        search_match_bg: c(0xfa, 0xbd, 0x2f),
        search_match_fg: c(0x28, 0x28, 0x28),
        search_row_bg:   Color::from_rgba(0xfa as f32/255., 0xbd as f32/255., 0x2f as f32/255., 0.12),
        search_gutter:   c(0xfa, 0xbd, 0x2f),
        line_number:     c(0x50, 0x49, 0x45),
        selected_line:   Color::from_rgba(0x83 as f32/255., 0xa5 as f32/255., 0x98 as f32/255., 0.18),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x83 as f32/255., 0xa5 as f32/255., 0x98 as f32/255., 0.45),
    }
}

/// Clean light theme — Tailwind/React/Linear-inspired
/// bg: white/slate-50, accent: blue-600, borders: slate-200, text: slate-900
fn light_web() -> Palette {
    Palette {
        bg_primary:      c(0xFF, 0xFF, 0xFF),  // white
        bg_secondary:    c(0xF8, 0xFA, 0xFC),  // slate-50
        bg_surface:      c(0xF1, 0xF5, 0xF9),  // slate-100
        bg_hover:        c(0xE2, 0xE8, 0xF0),  // slate-200
        bg_alt_row:      c(0xFA, 0xFA, 0xFB),  // very subtle stripe
        fg_primary:      c(0x0F, 0x17, 0x2A),  // slate-950
        fg_muted:        c(0x64, 0x74, 0x8B),  // slate-500
        accent:          c(0x25, 0x63, 0xEB),  // blue-600
        border:          c(0xCB, 0xD5, 0xE1),  // slate-300
        log_trace:       c(0x94, 0xA3, 0xB8),  // slate-400
        log_debug:       c(0x25, 0x63, 0xEB),  // blue-600
        log_info:        c(0x05, 0x96, 0x69),  // emerald-600
        log_warn:        c(0xD9, 0x77, 0x06),  // amber-600
        log_error:       c(0xDC, 0x26, 0x26),  // red-600
        search_match_bg: c(0xFD, 0xE6, 0x8A),  // amber-200
        search_match_fg: c(0x0F, 0x17, 0x2A),
        search_row_bg:   Color::from_rgba(0xFD as f32/255., 0xE6 as f32/255., 0x8A as f32/255., 0.40),
        search_gutter:   c(0xD9, 0x77, 0x06),  // amber-600
        line_number:     c(0x94, 0xA3, 0xB8),  // slate-400
        selected_line:   Color::from_rgba(0x25 as f32/255., 0x63 as f32/255., 0xEB as f32/255., 0.10),
        context_row_bg:  Color::from_rgba(0.0, 0.0, 0.0, 0.03),
        selection_bg:    Color::from_rgba(0x25 as f32/255., 0x63 as f32/255., 0xEB as f32/255., 0.35),
    }
}

/// Atom / VS Code One Dark Pro
fn one_dark() -> Palette {
    Palette {
        bg_primary:      c(0x28, 0x2C, 0x34),
        bg_secondary:    c(0x21, 0x25, 0x2B),
        bg_surface:      c(0x2C, 0x31, 0x3A),
        bg_hover:        c(0x3E, 0x44, 0x52),
        bg_alt_row:      c(0x2A, 0x2E, 0x36),
        fg_primary:      c(0xAB, 0xB2, 0xBF),
        fg_muted:        c(0x5C, 0x63, 0x70),
        accent:          c(0x61, 0xAF, 0xEF),
        border:          c(0x3E, 0x44, 0x52),
        log_trace:       c(0x5C, 0x63, 0x70),
        log_debug:       c(0x61, 0xAF, 0xEF),
        log_info:        c(0x98, 0xC3, 0x79),
        log_warn:        c(0xE5, 0xC0, 0x7B),
        log_error:       c(0xE0, 0x6C, 0x75),
        search_match_bg: c(0xE5, 0xC0, 0x7B),
        search_match_fg: c(0x28, 0x2C, 0x34),
        search_row_bg:   Color::from_rgba(0xE5 as f32/255., 0xC0 as f32/255., 0x7B as f32/255., 0.12),
        search_gutter:   c(0xE5, 0xC0, 0x7B),
        line_number:     c(0x4B, 0x52, 0x63),
        selected_line:   Color::from_rgba(0x61 as f32/255., 0xAF as f32/255., 0xEF as f32/255., 0.15),
        context_row_bg:  Color::from_rgba(0.5, 0.5, 0.5, 0.08),
        selection_bg:    Color::from_rgba(0x61 as f32/255., 0xAF as f32/255., 0xEF as f32/255., 0.40),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline(always)]
const fn c(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}
