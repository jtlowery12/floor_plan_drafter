//! Look and feel of the application chrome (panels, buttons, fonts). The
//! drawing itself is themed separately in `render::PaperTheme`.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    Theme, Vec2, Visuals, epaint::Shadow,
};
use std::sync::Arc;

pub const APP_NAME: &str = "Drafting Table";

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl Palette {
    pub fn light() -> Self {
        Palette {
            dark: false,
            bg: c(0xF3, 0xF5, 0xF9),
            surface: c(0xFF, 0xFF, 0xFF),
            surface_alt: c(0xEE, 0xF1, 0xF6),
            border: c(0xDF, 0xE4, 0xED),
            text: c(0x1F, 0x26, 0x33),
            muted: c(0x6B, 0x75, 0x88),
            faint: c(0xA3, 0xAC, 0xBC),
            accent: c(0x2F, 0x6F, 0xEB),
            accent_hover: c(0x24, 0x59, 0xC4),
            accent_soft: c(0xE4, 0xED, 0xFD),
            on_accent: Color32::WHITE,
            success: c(0x2E, 0x9D, 0x6A),
            warning: c(0xC9, 0x8A, 0x1B),
            danger: c(0xD1, 0x43, 0x43),
        }
    }

    pub fn dark() -> Self {
        Palette {
            dark: true,
            bg: c(0x0B, 0x12, 0x20),
            surface: c(0x11, 0x1A, 0x2C),
            surface_alt: c(0x19, 0x25, 0x3C),
            border: c(0x24, 0x32, 0x4C),
            text: c(0xE8, 0xEE, 0xF9),
            muted: c(0x93, 0xA3, 0xBF),
            faint: c(0x5F, 0x6F, 0x8C),
            accent: c(0x6E, 0xA0, 0xFF),
            accent_hover: c(0x8A, 0xB4, 0xFF),
            accent_soft: c(0x1B, 0x2B, 0x4A),
            on_accent: c(0x0B, 0x12, 0x20),
            success: c(0x5B, 0xC4, 0x93),
            warning: c(0xE2, 0xAC, 0x4A),
            danger: c(0xF0, 0x7A, 0x7A),
        }
    }

    pub fn of(ctx: &egui::Context) -> Self {
        match ctx.theme() {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn serif(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif".into()))
}
pub fn icons(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

/// Fonts as raw bytes, shared with the exporter so PDFs and PNGs match the screen.
pub const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter_400Regular.ttf");
pub const INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter_500Medium.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter_600SemiBold.ttf");
const PLAYFAIR: &[u8] = include_bytes!("../assets/fonts/PlayfairDisplay_600SemiBold.ttf");
const ICONS: &[u8] = include_bytes!("../assets/fonts/MaterialIcons-Outlined-subset.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 5] = [
        ("Inter", INTER_REGULAR),
        ("InterMedium", INTER_MEDIUM),
        ("InterSemiBold", INTER_SEMIBOLD),
        ("Playfair", PLAYFAIR),
        ("Icons", ICONS),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".to_owned());
    for (family, face) in [
        ("medium", "InterMedium"),
        ("semibold", "InterSemiBold"),
        ("serif", "Playfair"),
    ] {
        fonts.families.insert(
            FontFamily::Name(family.into()),
            vec![face.to_owned(), "Inter".to_owned()],
        );
    }
    fonts
        .families
        .insert(FontFamily::Name("icons".into()), vec!["Icons".to_owned()]);
    ctx.set_fonts(fonts);
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let radius = CornerRadius::same(8);
    let hairline = Stroke::new(1.0, p.border);

    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.window_stroke = hairline;
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow {
        offset: [0, 10],
        blur: 36,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 140 } else { 44 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }),
    };
    v.faint_bg_color = p.surface_alt;
    v.extreme_bg_color = p.surface_alt;
    v.text_edit_bg_color = Some(p.surface_alt);
    v.code_bg_color = p.surface_alt;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.weak_text_color = Some(p.muted);
    v.striped = false;
    v.selection.bg_fill = mix(p.accent_soft, p.accent, 0.25);
    v.selection.stroke = Stroke::new(1.0, p.accent);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = hairline;
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;
    for (s, fill) in [
        (&mut w.inactive, p.surface_alt),
        (&mut w.hovered, p.surface_alt),
        (&mut w.open, p.surface_alt),
    ] {
        s.bg_fill = fill;
        s.weak_bg_fill = fill;
        s.fg_stroke = Stroke::new(1.0, p.text);
        s.corner_radius = radius;
    }
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.hovered.bg_stroke = Stroke::new(1.0, mix(p.border, p.muted, 0.4));
    w.hovered.expansion = 0.0;
    w.open.bg_stroke = Stroke::new(1.0, p.border);
    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;
    v
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Light, visuals(&Palette::light()));
    ctx.set_visuals_of(Theme::Dark, visuals(&Palette::dark()));
    ctx.all_styles_mut(|s| {
        use egui::TextStyle::*;
        s.text_styles.insert(Small, body(12.0));
        s.text_styles.insert(Body, body(13.5));
        s.text_styles.insert(Button, medium(13.5));
        s.text_styles.insert(Monospace, FontId::monospace(13.0));
        s.text_styles.insert(Heading, serif(26.0));
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size = Vec2::new(32.0, 30.0);
        s.spacing.window_margin = Margin::same(18);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.indent = 16.0;
    });
}
