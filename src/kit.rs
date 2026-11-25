//! Small custom-painted widgets shared by every screen.

use crate::theme::{self, Palette, mix};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Id, Margin, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, epaint::Shadow, pos2, vec2,
};

pub fn id(salt: impl egui::AsIdSalt) -> Id {
    Id::NULL.with(salt)
}

pub fn pal(ui: &Ui) -> Palette {
    Palette::of(ui.ctx())
}

pub fn text(s: impl Into<String>, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(s).font(theme::body(size)).color(color)
}
pub fn strong(s: impl Into<String>, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(s).font(theme::semibold(size)).color(color)
}
pub fn serif(s: impl Into<String>, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(s).font(theme::serif(size)).color(color)
}

/// A small all-caps section heading.
pub fn caps(ui: &mut Ui, label: &str) {
    let p = pal(ui);
    ui.label(strong(label.to_uppercase(), 10.5, p.muted));
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    card_with(ui, 14, add)
}

pub fn card_with<R>(
    ui: &mut Ui,
    padding: i8,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let p = pal(ui);
    Frame::NONE
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(padding))
        .shadow(Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(if p.dark { 60 } else { 10 }),
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

/// Paint an icon-font glyph centred at `center`.
pub fn paint_icon(ui: &Ui, center: egui::Pos2, size: f32, glyph: &str, color: Color32) {
    ui.painter()
        .text(center, Align2::CENTER_CENTER, glyph, theme::icons(size), color);
}

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Primary,
    Ghost,
    Danger,
}

pub fn button(ui: &mut Ui, kind: Btn, label: &str) -> Response {
    button_impl(ui, kind, None, label, false)
}

pub fn small_button(ui: &mut Ui, kind: Btn, label: &str) -> Response {
    button_impl(ui, kind, None, label, true)
}

pub fn icon_text_button(ui: &mut Ui, kind: Btn, icon: &str, label: &str) -> Response {
    button_impl(ui, kind, Some(icon), label, false)
}

pub fn small_icon_button(ui: &mut Ui, kind: Btn, icon: &str, label: &str) -> Response {
    button_impl(ui, kind, Some(icon), label, true)
}

fn button_impl(ui: &mut Ui, kind: Btn, icon: Option<&str>, label: &str, small: bool) -> Response {
    let p = pal(ui);
    let enabled = ui.is_enabled();
    let (fg, fill, hover_fill, stroke) = match kind {
        Btn::Primary => (p.on_accent, p.accent, p.accent_hover, Stroke::NONE),
        Btn::Ghost => (p.text, Color32::TRANSPARENT, p.surface_alt, Stroke::new(1.0, p.border)),
        Btn::Danger => (
            p.danger,
            Color32::TRANSPARENT,
            mix(p.surface, p.danger, 0.1),
            Stroke::new(1.0, mix(p.border, p.danger, 0.5)),
        ),
    };
    let fg = if enabled { fg } else { fg.gamma_multiply(0.4) };
    let (font, height, pad, isize) = if small {
        (theme::semibold(12.5), 28.0, 11.0, 15.0)
    } else {
        (theme::semibold(13.5), 34.0, 15.0, 17.0)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let icon_w = if icon.is_some() { isize + 6.0 } else { 0.0 };
    let size = vec2(galley.size().x + pad * 2.0 + icon_w, height);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, 0.12);
    let bg = if fill == Color32::TRANSPARENT {
        hover_fill.gamma_multiply(t)
    } else {
        mix(fill, hover_fill, t)
    };
    let bg = if enabled { bg } else { bg.gamma_multiply(0.5) };
    ui.painter().rect(rect, CornerRadius::same(10), bg, stroke, StrokeKind::Inside);
    let mut x = rect.left() + pad;
    if let Some(ic) = icon {
        paint_icon(ui, pos2(x + isize / 2.0, rect.center().y), isize, ic, fg);
        x += icon_w;
    }
    ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A square icon button. `active` shows it as pressed/selected.
pub fn icon_button(ui: &mut Ui, glyph: &str, tip: &str, active: bool) -> Response {
    sized_icon_button(ui, glyph, tip, active, 32.0, 18.0)
}

pub fn sized_icon_button(
    ui: &mut Ui,
    glyph: &str,
    tip: &str,
    active: bool,
    box_size: f32,
    icon_size: f32,
) -> Response {
    let p = pal(ui);
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(vec2(box_size, box_size), if enabled { Sense::click() } else { Sense::hover() });
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, 0.1);
    let bg = if active {
        p.accent_soft
    } else {
        p.surface_alt.gamma_multiply(t)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), bg);
    let color = if active { p.accent } else { mix(p.muted, p.text, t) };
    let color = if enabled { color } else { color.gamma_multiply(0.4) };
    paint_icon(ui, rect.center(), icon_size, glyph, color);
    let resp = resp.on_hover_text(tip);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A pill used for toggles and choices.
pub fn chip(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let p = pal(ui);
    let font = if selected { theme::semibold(12.5) } else { theme::medium(12.5) };
    let fg = if selected { p.accent } else { p.muted };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, 28.0), Sense::click());
    let hovered = resp.hovered();
    let (fill, stroke) = if selected {
        (p.accent_soft, Stroke::new(1.0, mix(p.accent, p.surface, 0.55)))
    } else {
        (if hovered { p.surface_alt } else { p.surface }, Stroke::new(1.0, p.border))
    };
    ui.painter().rect(rect, CornerRadius::same(14), fill, stroke, StrokeKind::Inside);
    let fg = if hovered && !selected { p.text } else { fg };
    ui.painter().galley(pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A two-or-more segment control. Returns true if the value changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, items: &[(T, &str)]) -> bool {
    let p = pal(ui);
    let widths: Vec<f32> = items
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), theme::semibold(12.5), p.text).size().x + 24.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 32.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), p.surface_alt);
    let mut x = rect.left() + 3.0;
    let mut changed = false;
    for ((value, label), w) in items.iter().zip(widths) {
        let seg = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(w, 26.0));
        let resp = ui.interact(seg, id(("seg", label, rect.min.x as i32)), Sense::click());
        let selected = *current == *value;
        if selected {
            ui.painter().rect(seg, CornerRadius::same(8), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let color = if selected { p.text } else if resp.hovered() { p.text } else { p.muted };
        ui.painter().text(seg.center(), Align2::CENTER_CENTER, *label, if selected { theme::semibold(12.5) } else { theme::medium(12.5) }, color);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !selected {
            *current = *value;
            changed = true;
        }
        x += w;
    }
    changed
}

pub fn progress_bar(ui: &mut Ui, frac: f32, color: Color32, height: f32) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().max(1.0), height), Sense::hover());
    let r = height / 2.0;
    ui.painter().rect_filled(rect, r, mix(p.surface_alt, p.border, 0.6));
    let frac = frac.clamp(0.0, 1.0);
    if frac > 0.0 {
        let w = (rect.width() * frac).max(height);
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(w, height)), r, color);
    }
}

/// A one-line text field showing a length in the project's units. Edits are
/// parsed on commit (Enter or focus loss); invalid input snaps back.
/// Returns the new value in inches when the user committed a valid change.
pub fn length_field(
    ui: &mut Ui,
    salt: impl egui::AsIdSalt,
    value: f32,
    units: crate::units::Units,
    width: f32,
) -> Option<f32> {
    let id = ui.make_persistent_id(salt);
    let editing_id = id.with("buf");
    let mut buf: String = ui
        .data_mut(|d| d.get_temp::<String>(editing_id))
        .unwrap_or_else(|| units.len(value));
    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .id(id)
            .desired_width(width)
            .margin(Margin::symmetric(8, 6)),
    );
    let mut out = None;
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(editing_id, buf.clone()));
    } else {
        ui.data_mut(|d| d.remove_temp::<String>(editing_id));
    }
    // Commit on Enter or when focus leaves; Escape abandons the edit.
    if resp.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(v) = units.parse_len(&buf) {
            if (v - value).abs() > 0.01 && v >= 1.0 {
                out = Some(v);
            }
        }
    }
    out
}
