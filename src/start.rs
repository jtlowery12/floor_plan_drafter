//! The start screen: create a plan from a square-footage number, or reopen one.

use crate::icons;
use crate::kit::{self, Btn, card, pal};
use crate::model::*;
use crate::paint;
use crate::render::{self, PaperTheme, View};
use crate::store::{Store, Summary};
use crate::theme;
use crate::units::Units;
use eframe::egui::{self, Align, Layout, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, vec2};

pub struct Wizard {
    pub name: String,
    pub units: Units,
    pub area_text: String,
    pub floors: usize,
    pub proportion: Proportion,
}

impl Default for Wizard {
    fn default() -> Self {
        Wizard {
            name: String::new(),
            units: Units::Imperial,
            area_text: "2000".into(),
            floors: 1,
            proportion: Proportion::Classic,
        }
    }
}

impl Wizard {
    pub fn area(&self) -> Option<f32> {
        self.area_text.trim().replace(',', "").parse::<f32>().ok().filter(|a| *a >= 50.0 && *a <= 100_000.0)
    }
    fn plan(&self) -> Option<NewPlan> {
        Some(NewPlan {
            name: self.name.clone(),
            units: self.units,
            area: self.area()?,
            floors: self.floors,
            proportion: self.proportion,
        })
    }
}

pub enum Choice {
    Create(NewPlan),
    Open(String),
    Delete(String),
    Import,
}

pub fn show(ui: &mut Ui, wiz: &mut Wizard, plans: &[Summary], store: &Store) -> Option<Choice> {
    let p = pal(ui);
    let mut choice = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(980.0);
            ui.add_space(46.0);
            ui.label(kit::serif("Drafting Table", 40.0, p.text));
            ui.label(kit::text(
                "Start with a square footage. Draw the home you want. Hand it to your architect.",
                15.0,
                p.muted,
            ));
            ui.add_space(22.0);
            ui.columns(2, |cols| {
                new_plan_card(&mut cols[0], wiz, &mut choice);
                saved_plans(&mut cols[1], plans, store, &mut choice);
            });
            ui.add_space(30.0);
        });
    });
    choice
}

fn new_plan_card(ui: &mut Ui, wiz: &mut Wizard, choice: &mut Option<Choice>) {
    let p = pal(ui);
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            kit::caps(ui, "New plan");
            ui.add(
                TextEdit::singleline(&mut wiz.name)
                    .hint_text("Name, e.g. Lake house")
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                ui.label(kit::text("Units", 13.0, p.muted));
                let before = wiz.units;
                kit::segmented(ui, &mut wiz.units, &[(Units::Imperial, "Feet & inches"), (Units::Metric, "Metric")]);
                if before != wiz.units {
                    let a = wiz.area();
                    if let Some(a) = a {
                        let sqin = before.area_to_sq_inches(a);
                        wiz.area_text = format!("{}", wiz.units.area_value(sqin).round());
                    }
                }
            });
            ui.label(kit::text(format!("Total living area ({})", wiz.units.area_unit()), 13.0, p.muted));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut wiz.area_text)
                        .font(theme::serif(26.0))
                        .margin(egui::Margin::symmetric(10, 6))
                        .desired_width(150.0),
                );
                for preset in presets(wiz.units) {
                    if kit::chip(ui, wiz.area_text == preset.to_string(), &preset.to_string()).clicked() {
                        wiz.area_text = preset.to_string();
                    }
                }
            });
            if wiz.area().is_none() {
                ui.label(kit::text("Enter a number between 50 and 100,000.", 12.0, p.danger));
            }
            ui.horizontal(|ui| {
                ui.label(kit::text("Floors", 13.0, p.muted));
                for n in 1..=3usize {
                    if kit::chip(ui, wiz.floors == n, &n.to_string()).clicked() {
                        wiz.floors = n;
                    }
                }
            });
            ui.label(kit::text("Footprint shape", 13.0, p.muted));
            let th = if p.dark { PaperTheme::dark() } else { PaperTheme::light() };
            ui.horizontal(|ui| {
                for prop in Proportion::ALL {
                    proportion_tile(ui, wiz, prop, &th);
                }
            });
            ui.add_space(4.0);
            let ok = wiz.area().is_some();
            ui.add_enabled_ui(ok, |ui| {
                if kit::icon_text_button(ui, Btn::Primary, icons::ARCHITECTURE, "Create skeleton plan").clicked() {
                    if let Some(n) = wiz.plan() {
                        *choice = Some(Choice::Create(n));
                    }
                }
            });
            ui.label(kit::text(
                "You get a to-scale outline for each floor and a live meter showing how much of your target you've drawn.",
                12.0,
                p.faint,
            ));
        });
    });
}

fn presets(units: Units) -> [u32; 4] {
    match units {
        Units::Imperial => [1200, 1800, 2500, 3500],
        Units::Metric => [80, 120, 180, 250],
    }
}

fn proportion_tile(ui: &mut Ui, wiz: &mut Wizard, prop: Proportion, th: &PaperTheme) {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(72.0, 70.0), Sense::click());
    let sel = wiz.proportion == prop;
    ui.painter().rect(
        rect,
        10,
        if sel { p.accent_soft } else { p.surface },
        Stroke::new(if sel { 1.5 } else { 1.0 }, if sel { p.accent } else { p.border }),
        StrokeKind::Inside,
    );
    let r = prop.ratio();
    let (w, h) = if r >= 1.0 { (44.0, 44.0 / r) } else { (44.0 * r, 44.0) };
    let b = Rect::from_center_size(rect.center() - vec2(0.0, 7.0), vec2(w, h));
    ui.painter().rect(b, 2, th.paper, Stroke::new(1.6, if sel { p.accent } else { p.muted }), StrokeKind::Inside);
    ui.painter().text(
        rect.center_bottom() - vec2(0.0, 9.0),
        egui::Align2::CENTER_CENTER,
        prop.label(),
        theme::medium(11.5),
        if sel { p.accent } else { p.muted },
    );
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        wiz.proportion = prop;
    }
}

fn saved_plans(ui: &mut Ui, plans: &[Summary], store: &Store, choice: &mut Option<Choice>) {
    let p = pal(ui);
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.horizontal(|ui| {
            ui.label(kit::strong("YOUR PLANS", 11.0, p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::small_icon_button(ui, Btn::Ghost, icons::UPLOAD, "Import").clicked() {
                    *choice = Some(Choice::Import);
                }
            });
        });
        if plans.is_empty() {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(kit::serif("Nothing yet", 20.0, p.text));
                    ui.label(kit::text("Your plans are saved on this computer, and nowhere else.", 13.0, p.muted));
                });
                ui.add_space(24.0);
            });
            return;
        }
        let th = if p.dark { PaperTheme::dark() } else { PaperTheme::light() };
        for s in plans {
            plan_row(ui, s, store, &th, choice);
        }
    });
}

fn plan_row(ui: &mut Ui, s: &Summary, store: &Store, th: &PaperTheme, choice: &mut Option<Choice>) {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 92.0), Sense::click());
    let hov = resp.hovered();
    ui.painter().rect(
        rect,
        14,
        if hov { p.surface_alt } else { p.surface },
        Stroke::new(1.0, if hov { p.accent } else { p.border }),
        StrokeKind::Inside,
    );
    // Thumbnail: the real drawing, rendered small.
    let thumb = Rect::from_min_size(rect.min + vec2(10.0, 10.0), vec2(96.0, 72.0));
    ui.painter().rect_filled(thumb, 8, th.outside);
    if let Ok(project) = store.load(&s.id) {
        let view = View::fit(project.extents(0), crate::geom::v2(96.0, 72.0), 6.0);
        let o = render::Opts { grid: false, dims: false, labels: false, ..Default::default() };
        let cmds = render::plan(&project, 0, &view, th, &o);
        let painter = ui.painter_at(thumb);
        let fp: Vec<egui::Pos2> = project.footprint.rect().iter().map(|&q| {
            let v = view.to_screen(q);
            thumb.min + vec2(v.x, v.y)
        }).collect();
        painter.add(egui::Shape::convex_polygon(fp, th.paper, Stroke::NONE));
        paint::paint(&painter, thumb.min, &cmds);
    }
    let x = thumb.right() + 14.0;
    let name = render::fit_text(&s.name, rect.right() - x - 50.0, 16.0);
    ui.painter().text(egui::pos2(x, rect.top() + 24.0), egui::Align2::LEFT_CENTER, name, theme::semibold(16.0), p.text);
    ui.painter().text(
        egui::pos2(x, rect.top() + 46.0),
        egui::Align2::LEFT_CENTER,
        format!(
            "{} target · {} floor{}",
            s.units.area(s.target_area),
            s.floors,
            if s.floors == 1 { "" } else { "s" }
        ),
        theme::body(12.5),
        p.muted,
    );
    let local = s.modified.with_timezone(&chrono::Local);
    ui.painter().text(
        egui::pos2(x, rect.top() + 66.0),
        egui::Align2::LEFT_CENTER,
        format!("Edited {}", local.format("%b %-d, %Y")),
        theme::body(11.5),
        p.faint,
    );
    // Delete button, top-right.
    let del = Rect::from_center_size(egui::pos2(rect.right() - 24.0, rect.top() + 24.0), vec2(30.0, 30.0));
    let dr = ui.interact(del, kit::id(("del", &s.id)), Sense::click());
    if hov || dr.hovered() {
        kit::paint_icon(ui, del.center(), 18.0, icons::DELETE, if dr.hovered() { p.danger } else { p.faint });
    }
    if dr.on_hover_text("Delete plan").clicked() {
        *choice = Some(Choice::Delete(s.id.clone()));
    } else if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *choice = Some(Choice::Open(s.id.clone()));
    }
}
