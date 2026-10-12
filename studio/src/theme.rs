//! Studio's look: three themes (midnight blue, dark and light), each with
//! the colours of the window, its panels and the syntax highlighting.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

use crate::highlight;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeName {
    Midnight,
    Dark,
    Light,
}

impl ThemeName {
    pub const ALL: [ThemeName; 3] = [ThemeName::Midnight, ThemeName::Dark, ThemeName::Light];

    pub fn id(self) -> &'static str {
        match self {
            ThemeName::Midnight => "midnight",
            ThemeName::Dark => "dark",
            ThemeName::Light => "light",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|name| name.id() == id)
    }
}

pub struct Palette {
    pub dark: bool,
    /// Behind and between the panels.
    pub window: Color32,
    /// The panels ("cards").
    pub panel: Color32,
    pub panel_border: Color32,
    /// The editor, terminal and text fields.
    pub editor: Color32,
    pub text: Color32,
    pub weak_text: Color32,
    pub accent: Color32,
    /// Selected items: a soft version of the accent.
    pub selection: Color32,
    pub button: Color32,
    pub button_hover: Color32,
    pub success: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub gutter: Color32,
    pub syntax: highlight::Theme,
}

pub const MIDNIGHT: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(0x06, 0x0b, 0x18),
    panel: Color32::from_rgb(0x0c, 0x14, 0x27),
    panel_border: Color32::from_rgb(0x1c, 0x2a, 0x4a),
    editor: Color32::from_rgb(0x09, 0x10, 0x20),
    text: Color32::from_rgb(0xdc, 0xe4, 0xf5),
    weak_text: Color32::from_rgb(0x7d, 0x8c, 0xae),
    accent: Color32::from_rgb(0x3b, 0x82, 0xf6),
    selection: Color32::from_rgb(0x17, 0x34, 0x6e),
    button: Color32::from_rgb(0x13, 0x1f, 0x3b),
    button_hover: Color32::from_rgb(0x1d, 0x2e, 0x55),
    success: Color32::from_rgb(0x4a, 0xde, 0x80),
    danger: Color32::from_rgb(0xf8, 0x71, 0x71),
    warning: Color32::from_rgb(0xfa, 0xcc, 0x15),
    gutter: Color32::from_rgb(0x4b, 0x5a, 0x7c),
    syntax: highlight::Theme {
        text: Color32::from_rgb(0xdc, 0xe4, 0xf5),
        keyword: Color32::from_rgb(0xf4, 0x72, 0xb6),
        constant: Color32::from_rgb(0x60, 0xa5, 0xfa),
        builtin: Color32::from_rgb(0xfd, 0xe0, 0x47),
        string: Color32::from_rgb(0xfb, 0x92, 0x3c),
        number: Color32::from_rgb(0x86, 0xef, 0xac),
        comment: Color32::from_rgb(0x64, 0x74, 0x8b),
        error_background: Color32::from_rgb(0x4c, 0x15, 0x1f),
        paused_background: Color32::from_rgb(0x3b, 0x35, 0x0c),
        squiggle: Color32::from_rgb(0xf8, 0x71, 0x71),
    },
};

pub const DARK: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(0x11, 0x11, 0x11),
    panel: Color32::from_rgb(0x1e, 0x1e, 0x1e),
    panel_border: Color32::from_rgb(0x30, 0x30, 0x30),
    editor: Color32::from_rgb(0x18, 0x18, 0x18),
    text: Color32::from_rgb(0xd4, 0xd4, 0xd4),
    weak_text: Color32::from_rgb(0x8a, 0x8a, 0x8a),
    accent: Color32::from_rgb(0x0e, 0x8a, 0xd4),
    selection: Color32::from_rgb(0x04, 0x39, 0x5e),
    button: Color32::from_rgb(0x2a, 0x2a, 0x2a),
    button_hover: Color32::from_rgb(0x37, 0x37, 0x37),
    success: Color32::from_rgb(0x89, 0xd1, 0x85),
    danger: Color32::from_rgb(0xf4, 0x87, 0x71),
    warning: Color32::from_rgb(0xdc, 0xdc, 0xaa),
    gutter: Color32::from_rgb(0x6e, 0x6e, 0x6e),
    syntax: highlight::DARK,
};

pub const LIGHT: Palette = Palette {
    dark: false,
    window: Color32::from_rgb(0xe4, 0xe9, 0xf2),
    panel: Color32::from_rgb(0xfb, 0xfc, 0xfe),
    panel_border: Color32::from_rgb(0xcf, 0xd8, 0xe6),
    editor: Color32::WHITE,
    text: Color32::from_rgb(0x1f, 0x29, 0x37),
    weak_text: Color32::from_rgb(0x6b, 0x72, 0x80),
    accent: Color32::from_rgb(0x25, 0x63, 0xeb),
    selection: Color32::from_rgb(0xd6, 0xe4, 0xff),
    button: Color32::from_rgb(0xec, 0xf0, 0xf6),
    button_hover: Color32::from_rgb(0xdd, 0xe5, 0xf2),
    success: Color32::from_rgb(0x15, 0x80, 0x3d),
    danger: Color32::from_rgb(0xdc, 0x26, 0x26),
    warning: Color32::from_rgb(0xb4, 0x53, 0x09),
    gutter: Color32::from_rgb(0x9c, 0xa3, 0xaf),
    syntax: highlight::Theme {
        text: Color32::from_rgb(0x1f, 0x1f, 0x1f),
        keyword: Color32::from_rgb(0xaf, 0x00, 0xdb),
        constant: Color32::from_rgb(0x00, 0x00, 0xff),
        builtin: Color32::from_rgb(0x79, 0x5e, 0x26),
        string: Color32::from_rgb(0xa3, 0x15, 0x15),
        number: Color32::from_rgb(0x09, 0x86, 0x58),
        comment: Color32::from_rgb(0x00, 0x80, 0x00),
        error_background: Color32::from_rgb(0xff, 0xdc, 0xdc),
        paused_background: Color32::from_rgb(0xff, 0xf3, 0xb0),
        squiggle: Color32::from_rgb(0xe5, 0x14, 0x00),
    },
};

pub fn palette(name: ThemeName) -> &'static Palette {
    match name {
        ThemeName::Midnight => &MIDNIGHT,
        ThemeName::Dark => &DARK,
        ThemeName::Light => &LIGHT,
    }
}

pub const RADIUS: u8 = 10;

/// The frame of a panel: a rounded card with a little space around it.
pub fn card(palette: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.panel)
        .stroke(Stroke::new(1.0, palette.panel_border))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::same(10))
        .outer_margin(egui::Margin::same(4))
}

/// Makes every widget use the palette.
pub fn apply(context: &egui::Context, palette: &Palette) {
    let theme = if palette.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    context.set_theme(theme);
    context.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        for (text_style, font) in style.text_styles.iter_mut() {
            font.size = match text_style {
                egui::TextStyle::Small => 12.0,
                egui::TextStyle::Heading => 20.0,
                _ => 14.5,
            };
        }
        let visuals = &mut style.visuals;
        visuals.dark_mode = palette.dark;
        visuals.override_text_color = Some(palette.text);
        visuals.panel_fill = palette.window;
        visuals.window_fill = palette.panel;
        visuals.window_stroke = Stroke::new(1.0, palette.panel_border);
        visuals.window_corner_radius = CornerRadius::same(RADIUS + 2);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.extreme_bg_color = palette.editor;
        visuals.faint_bg_color = palette.button;
        visuals.code_bg_color = palette.editor;
        visuals.hyperlink_color = palette.accent;
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = Stroke::new(1.0, palette.accent);
        visuals.warn_fg_color = palette.warning;
        visuals.error_fg_color = palette.danger;
        let radius = CornerRadius::same(6);
        let widgets = &mut visuals.widgets;
        for (state, fill, stroke) in [
            (
                &mut widgets.noninteractive,
                palette.panel,
                palette.panel_border,
            ),
            (&mut widgets.inactive, palette.button, palette.panel_border),
            (&mut widgets.hovered, palette.button_hover, palette.accent),
            (&mut widgets.active, palette.selection, palette.accent),
            (
                &mut widgets.open,
                palette.button_hover,
                palette.panel_border,
            ),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.corner_radius = radius;
            state.fg_stroke.color = palette.text;
        }
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.panel_border);
        widgets.noninteractive.fg_stroke.color = palette.text;
    });
}
