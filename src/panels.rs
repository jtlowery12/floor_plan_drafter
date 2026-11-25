//! Editor chrome: top bar, tool column, floor bar and the right-hand panel
//! (area meter, properties, furniture library).

use crate::editor::*;
use crate::geom::*;
use crate::icons;
use crate::kit::{self, Btn, card, pal};
use crate::model::*;
use crate::paint;
use crate::render::{self, PaperTheme, ROOM_COLOR_NAMES, View};
use crate::theme::{self, Palette, mix};
use crate::units::Units;
use eframe::egui::{
    self, Align, CornerRadius, Layout, Margin, Rect, Sense, Stroke, StrokeKind, TextEdit,
    ThemePreference, Ui, pos2, vec2,
};

/// Things the editor chrome asks the surrounding app to do.
#[derive(Default)]
pub struct Actions {
    pub home: bool,
    pub theme: Option<ThemePreference>,
}

fn sep(ui: &mut Ui) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 24.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.border);
}

pub fn top_bar(ed: &mut Editor, ui: &mut Ui, actions: &mut Actions) {
    let p = pal(ui);
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        if kit::icon_button(ui, icons::HOME, "All plans", false).clicked() {
            actions.home = true;
        }
        ui.add_space(2.0);
        let mut name = ed.project.name.clone();
        let r = ui.add(
            TextEdit::singleline(&mut name)
                .font(theme::serif(19.0))
                .frame(egui::Frame::NONE)
                .desired_width(230.0)
                .text_color(p.text),
        );
        if r.changed() {
            ed.set_name(name);
        }
        ui.add_space(6.0);
        sep(ui);
        ui.add_space(2.0);
        let (u, r) = (ed.can_undo(), ed.can_redo());
        ui.add_enabled_ui(u, |ui| {
            if kit::icon_button(ui, icons::UNDO, "Undo (Ctrl+Z)", false).clicked() {
                ed.undo();
            }
        });
        ui.add_enabled_ui(r, |ui| {
            if kit::icon_button(ui, icons::REDO, "Redo (Ctrl+Shift+Z)", false).clicked() {
                ed.redo();
            }
        });
        ui.add_space(2.0);
        sep(ui);
        ui.add_space(2.0);
        if kit::icon_button(ui, icons::GRID, "Show grid (G)", ed.opts.grid).clicked() {
            ed.opts.grid = !ed.opts.grid;
        }
        if kit::icon_button(ui, icons::RULER, "Show dimensions", ed.opts.dims).clicked() {
            ed.opts.dims = !ed.opts.dims;
        }
        if kit::icon_button(ui, icons::TEXT, "Show room names and areas", ed.opts.labels).clicked() {
            ed.opts.labels = !ed.opts.labels;
        }
        ui.add_space(4.0);
        snap_picker(ed, ui);
        ui.add_space(2.0);
        sep(ui);
        ui.add_space(2.0);
        if kit::icon_button(ui, icons::ZOOM_OUT, "Zoom out", false).clicked() {
            let c = v2(ed.view.origin.x, ed.view.origin.y);
            let _ = c;
            zoom_center(ed, 1.0 / 1.25);
        }
        if kit::icon_button(ui, icons::ZOOM_IN, "Zoom in", false).clicked() {
            zoom_center(ed, 1.25);
        }
        if kit::icon_button(ui, icons::FIT, "Fit plan to window (Ctrl+0)", false).clicked() {
            ed.view_ready = false;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            if kit::icon_text_button(ui, Btn::Primary, icons::EXPORT, "Export").clicked() {
                ed.show_export = true;
            }
            if kit::icon_button(ui, icons::SETTINGS, "Plan settings", false).clicked() {
                ed.show_settings = true;
            }
            let dark = Palette::of(ui.ctx()).dark;
            let (glyph, tip, next) = if dark {
                (icons::LIGHT, "Switch to light", ThemePreference::Light)
            } else {
                (icons::DARK, "Switch to dark", ThemePreference::Dark)
            };
            if kit::icon_button(ui, glyph, tip, false).clicked() {
                actions.theme = Some(next);
            }
        });
    });
}

/// Zoom about the middle of the canvas (the top bar doesn't know its exact size,
/// so we use the current view's center, which the next frame re-derives).
fn zoom_center(ed: &mut Editor, factor: f32) {
    let c = ed.view.to_screen(ed.cursor.unwrap_or(v2(ed.project.footprint.w / 2.0, ed.project.footprint.h / 2.0)));
    ed.view.zoom_at(c, factor, 0.01, 16.0);
}

fn snap_picker(ed: &mut Editor, ui: &mut Ui) {
    let units = ed.project.units;
    let current = units
        .snap_options()
        .iter()
        .find(|(v, _)| (*v - ed.project.snap).abs() < 0.01)
        .map_or("Custom", |(_, l)| *l);
    let text = if ed.snap_on { format!("Snap {current}") } else { "Snap off".to_string() };
    egui::ComboBox::from_id_salt("snap_picker")
        .selected_text(text)
        .width(104.0)
        .show_ui(ui, |ui| {
            if ui.selectable_label(!ed.snap_on, "Off").clicked() {
                ed.snap_on = false;
            }
            for (v, l) in units.snap_options() {
                if ui.selectable_label(ed.snap_on && (ed.project.snap - v).abs() < 0.01, *l).clicked() {
                    ed.snap_on = true;
                    ed.project.snap = *v;
                    ed.dirty = true;
                }
            }
        });
}

pub fn tool_column(ed: &mut Editor, ui: &mut Ui) {
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let groups: &[&[(Tool, &str, &str)]] = &[
            &[
                (Tool::Select, icons::SELECT, "Select and move (V)"),
                (Tool::Pan, icons::PAN, "Pan (H) — or right-drag any time"),
            ],
            &[
                (Tool::Rect, icons::RECT, "Rectangle (R)"),
                (Tool::Ellipse, icons::ELLIPSE, "Ellipse (E)"),
                (Tool::Triangle, icons::TRIANGLE, "Triangle (T)"),
                (Tool::Polygon, icons::POLYGON, "Custom shape (P)"),
            ],
            &[
                (Tool::Wall, icons::WALL, "Wall (W)"),
                (Tool::Door, icons::DOOR, "Door (D)"),
                (Tool::SlidingDoor, icons::SLIDING, "Sliding door (S)"),
                (Tool::Window, icons::WINDOW, "Window (N)"),
            ],
            &[(Tool::Furniture, icons::FURNITURE, "Furniture and fixtures (F)")],
        ];
        for (gi, group) in groups.iter().enumerate() {
            if gi > 0 {
                ui.add_space(4.0);
                let p = pal(ui);
                let (r, _) = ui.allocate_exact_size(vec2(28.0, 1.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, p.border);
                ui.add_space(4.0);
            }
            for (tool, glyph, tip) in group.iter() {
                if kit::sized_icon_button(ui, glyph, tip, ed.tool == *tool, 42.0, 22.0).clicked() {
                    ed.set_tool(*tool);
                }
            }
        }
    });
}

pub fn bottom_bar(ed: &mut Editor, ui: &mut Ui) {
    let p = pal(ui);
    ui.horizontal_centered(|ui| {
        ui.add_space(14.0);
        ui.label(kit::strong("FLOORS", 10.5, p.muted));
        ui.add_space(2.0);
        let names: Vec<String> = ed.project.floors.iter().map(|f| f.name.clone()).collect();
        let mut switch = None;
        for (i, n) in names.iter().enumerate() {
            if kit::chip(ui, i == ed.floor, n).clicked() {
                switch = Some(i);
            }
        }
        if let Some(i) = switch {
            ed.switch_floor(i);
        }
        if ed.project.floors.len() < 8 && kit::icon_button(ui, icons::ADD, "Add a floor", false).clicked() {
            ed.add_floor();
        }
        ui.add_space(8.0);
        ui.add(egui::Label::new(kit::text(&ed.hint, 12.0, p.muted)).truncate());
    });
}

// ---- right panel ------------------------------------------------------------------

pub fn side_panel(ed: &mut Editor, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.y = 12.0;
        area_meter(ed, ui);
        if ed.tool == Tool::Furniture {
            library(ed, ui);
        } else if !ed.selection.is_empty() {
            properties(ed, ui);
        } else {
            quick_help(ed, ui);
        }
        ui.add_space(14.0);
    });
}

fn area_meter(ed: &mut Editor, ui: &mut Ui) {
    let p = pal(ui);
    let units = ed.project.units;
    let drawn = ed.project.total_area();
    let target = ed.project.target_area;
    let frac = if target > 0.0 { drawn / target } else { 0.0 };
    let diff = target - drawn;
    let (status, color) = if frac > 1.05 {
        (format!("{} over", units.area(-diff)), p.warning)
    } else if frac >= 0.95 {
        ("On target".to_string(), p.success)
    } else {
        (format!("{} to go", units.area(diff)), p.accent)
    };
    card(ui, |ui| {
        ui.horizontal(|ui| {
            kit::caps(ui, "Living area");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::sized_icon_button(ui, icons::EDIT, "Change the target or footprint", false, 24.0, 14.0).clicked() {
                    ed.show_settings = true;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(kit::strong(format!("{:.0}", units.area_value(drawn).max(0.0).round().abs()), 30.0, p.text));
            ui.label(kit::text(format!("of {}", units.area(target)), 13.0, p.muted));
        });
        kit::progress_bar(ui, frac.min(1.0), color, 8.0);
        ui.label(kit::strong(status, 12.5, color));
        if ed.project.floors.len() > 1 {
            ui.add_space(2.0);
            for f in &ed.project.floors {
                ui.horizontal(|ui| {
                    ui.label(kit::text(&f.name, 12.5, p.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(kit::text(units.area(f.area()), 12.5, p.text));
                    });
                });
            }
        }
        ui.add_space(2.0);
        let fp = ed.project.footprint;
        ui.label(kit::text(
            format!("Footprint {} × {} per floor", units.len(fp.w), units.len(fp.h)),
            11.5,
            p.faint,
        ));
    });
}

fn quick_help(ed: &Editor, ui: &mut Ui) {
    let p = pal(ui);
    card(ui, |ui| {
        kit::caps(ui, "Getting started");
        let steps: [(&str, &str); 4] = [
            ("1", "Draw rooms with the shape tools. A click drops a ready-made room."),
            ("2", "Add walls (W), then doors and windows onto them."),
            ("3", "Drop in furniture (F) to test the layout."),
            ("4", "Export a PDF to hand to your architect."),
        ];
        for (n, t) in steps {
            ui.horizontal_top(|ui| {
                ui.label(kit::strong(n, 12.5, p.accent));
                ui.add(egui::Label::new(kit::text(t, 12.5, p.muted)).wrap());
            });
        }
        ui.add_space(2.0);
        ui.label(kit::text(
            format!(
                "Tip: names and sizes are editable in the panel when you select something. Everything saves automatically in {}.",
                "your data folder"
            ),
            11.5,
            p.faint,
        ));
        let _ = ed;
    });
}

fn paper_for(ui: &Ui) -> PaperTheme {
    if Palette::of(ui.ctx()).dark { PaperTheme::dark() } else { PaperTheme::light() }
}

fn properties(ed: &mut Editor, ui: &mut Ui) {
    let p = pal(ui);
    let th = paper_for(ui);
    let units = ed.project.units;
    let count = ed.selection.len();
    let single = ed.single().cloned();

    card(ui, |ui| {
        ui.horizontal(|ui| {
            let title = match &single {
                Some(s) => s.kind.name().to_string(),
                None => format!("{count} items"),
            };
            kit::caps(ui, &title);
        });

        if let Some(s) = &single {
            if matches!(s.kind, k if k.is_room() || matches!(k, Kind::Furniture(_))) {
                let mut label = s.label.clone();
                let id = kit::id(("name_field", s.id));
                let mut te = TextEdit::singleline(&mut label)
                    .id(id)
                    .hint_text(if s.kind.is_room() { "Room name, e.g. Kitchen" } else { "Optional label" })
                    .margin(Margin::symmetric(10, 8))
                    .desired_width(f32::INFINITY);
                if s.kind.is_room() {
                    te = te.font(theme::semibold(14.0));
                }
                let r = ui.add(te);
                if ed.focus_name {
                    r.request_focus();
                    ed.focus_name = false;
                }
                if r.changed() {
                    ed.set_label(s.id, label);
                }
            }
        }

        // Color (rooms).
        let any_room = ed.selected().iter().any(|s| s.kind.is_room());
        if any_room {
            ui.add_space(2.0);
            let current = single.as_ref().map(|s| s.color);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for i in 0..ROOM_COLORS {
                    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                    let col = th.room(i as u8);
                    ui.painter().rect(rect, CornerRadius::same(8), col, Stroke::new(1.0, mix(col, p.text, 0.25)), StrokeKind::Inside);
                    if current == Some(i as u8) {
                        ui.painter().rect_stroke(rect.expand(2.5), CornerRadius::same(10), Stroke::new(2.0, p.accent), StrokeKind::Outside);
                    }
                    if resp.on_hover_text(ROOM_COLOR_NAMES[i]).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        ed.set_color(i as u8);
                    }
                }
            });
        }

        if let Some(s) = &single {
            measurements(ed, ui, s, units);
        }

        ui.add_space(4.0);
        actions_row(ed, ui, single.as_ref());
    });
}

fn field_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.add_sized([62.0, 26.0], egui::Label::new(kit::text(label, 12.5, p.muted)));
        add(ui);
    });
}

fn measurements(ed: &mut Editor, ui: &mut Ui, s: &Shape, units: Units) {
    let p = pal(ui);
    ui.spacing_mut().item_spacing.y = 6.0;
    match s.kind {
        Kind::Wall => {
            let (a, b) = s.wall_ends();
            field_row(ui, "Length", |ui| {
                if let Some(v) = kit::length_field(ui, ("wlen", s.id), a.dist(b), units, 110.0) {
                    ed.set_wall_length(s.id, v);
                }
            });
            field_row(ui, "Thickness", |ui| {
                if let Some(v) = kit::length_field(ui, ("wthick", s.id), s.thickness, units, 110.0) {
                    ed.set_thickness(s.id, v);
                }
            });
        }
        Kind::Polygon => {
            ui.label(kit::text(format!("Area {}", units.area(s.area())), 12.5, p.muted));
            ui.label(kit::text(format!("{} corners — drag the dots to reshape, or the small circles to add corners.", s.points.len()), 11.5, p.faint));
        }
        k if k.is_opening() => {
            field_row(ui, "Width", |ui| {
                if let Some(v) = kit::length_field(ui, ("owidth", s.id), s.size.x, units, 110.0) {
                    ed.set_size(s.id, v2(v, s.size.y));
                }
            });
        }
        _ => {
            field_row(ui, "Width", |ui| {
                if let Some(v) = kit::length_field(ui, ("w", s.id), s.size.x, units, 110.0) {
                    ed.set_size(s.id, v2(v, s.size.y));
                }
            });
            field_row(ui, "Depth", |ui| {
                if let Some(v) = kit::length_field(ui, ("h", s.id), s.size.y, units, 110.0) {
                    ed.set_size(s.id, v2(s.size.x, v));
                }
            });
            field_row(ui, "Rotation", |ui| {
                let mut deg = s.rot.to_degrees();
                let r = ui.add(egui::DragValue::new(&mut deg).speed(1.0).range(-180.0..=180.0).suffix("°").max_decimals(0));
                if r.changed() {
                    ed.set_rotation(s.id, deg);
                }
            });
            if s.kind.is_room() {
                ui.label(kit::strong(format!("Area  {}", units.area(s.area())), 12.5, p.text));
            }
        }
    }
}

fn actions_row(ed: &mut Editor, ui: &mut Ui, single: Option<&Shape>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        if kit::icon_button(ui, icons::COPY, "Duplicate (Ctrl+D)", false).clicked() {
            ed.duplicate_selection();
        }
        if kit::icon_button(ui, icons::ROTATE, "Rotate 90° (Ctrl+R)", false).clicked() {
            ed.rotate_selection(std::f32::consts::FRAC_PI_2);
        }
        let flippable = single.is_some_and(|s| s.kind.is_opening() || matches!(s.kind, Kind::Furniture(_)));
        if flippable {
            if kit::icon_button(ui, icons::FLIP, "Flip swing / mirror", false).clicked() {
                ed.flip_selection(false);
            }
            if single.is_some_and(|s| s.kind.is_opening()) && kit::sized_icon_button(ui, icons::ARCHITECTURE, "Swap hinge side", false, 32.0, 18.0).clicked() {
                ed.flip_selection(true);
            }
        }
        let locked = single.is_some_and(|s| s.locked);
        if kit::icon_button(ui, if locked { icons::LOCK } else { icons::UNLOCK }, if locked { "Unlock" } else { "Lock in place" }, locked).clicked() {
            ed.set_locked(!locked);
        }
        if single.is_some_and(|s| s.kind.is_room() && s.kind != Kind::Ellipse)
            && kit::icon_button(ui, icons::WALLS, "Draw walls around this room", false).clicked()
        {
            ed.walls_around_selection();
        }
        if kit::icon_button(ui, icons::DELETE, "Delete (Del)", false).clicked() {
            ed.delete_selection();
        }
    });
}

fn library(ed: &mut Editor, ui: &mut Ui) {
    let p = pal(ui);
    let th = paper_for(ui);
    card(ui, |ui| {
        kit::caps(ui, "Furniture & fixtures");
        let mut last = "";
        let tile = 74.0;
        let per_row = ((ui.available_width() + 6.0) / (tile + 6.0)).floor().max(1.0) as usize;
        let mut group: Vec<Sym> = Vec::new();
        let mut flush = |ui: &mut Ui, ed: &mut Editor, group: &mut Vec<Sym>, name: &str| {
            if group.is_empty() {
                return;
            }
            ui.add_space(4.0);
            ui.label(kit::strong(name, 11.5, p.muted));
            for row in group.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for &sym in row {
                        sym_tile(ed, ui, &th, sym, tile);
                    }
                });
            }
            group.clear();
        };
        for sym in Sym::ALL {
            if sym.group() != last {
                flush(ui, ed, &mut group, last);
                last = sym.group();
            }
            group.push(sym);
        }
        flush(ui, ed, &mut group, last);
    });
}

fn sym_tile(ed: &mut Editor, ui: &mut Ui, th: &PaperTheme, sym: Sym, size: f32) {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size + 16.0), Sense::click());
    let box_r = Rect::from_min_size(rect.min, vec2(size, size));
    let selected = ed.sym == sym;
    let hovered = resp.hovered();
    ui.painter().rect(
        box_r,
        CornerRadius::same(10),
        if selected { p.accent_soft } else if hovered { p.surface_alt } else { th.paper },
        Stroke::new(if selected { 1.5 } else { 1.0 }, if selected { p.accent } else { p.border }),
        StrokeKind::Inside,
    );
    // Draw the real plan symbol, scaled to fit the tile.
    let d = sym.default_size();
    let shape = Shape::boxed(Kind::Furniture(sym), v2(0.0, 0.0), d);
    let view = View::fit((v2(-d.x / 2.0, -d.y / 2.0), v2(d.x / 2.0, d.y / 2.0)), v2(size, size), 11.0);
    let cmds = render::shape_cmds(&shape, &view, th, &render::Opts::default(), ed.project.units);
    paint::paint(&ui.painter_at(box_r), box_r.min, &cmds);
    ui.painter().text(
        pos2(rect.center().x, rect.bottom() - 7.0),
        egui::Align2::CENTER_CENTER,
        crate::render::fit_text(sym.name(), size, 11.0),
        theme::medium(11.0),
        if selected { p.accent } else { p.muted },
    );
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        ed.sym = sym;
        ed.set_tool(Tool::Furniture);
    }
}

