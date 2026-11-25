//! Application shell: start screen <-> editor, autosave, plan settings.

use crate::editor::Editor;
use crate::kit::{self, Btn};
use crate::model::*;
use crate::panels::{self, Actions};
use crate::start::{self, Choice, Wizard};
use crate::store::{Store, Summary};
use crate::theme::{self, Palette};
use crate::units::Units;
use crate::{canvas, export};
use eframe::egui::{self, Context, Frame, Margin, ThemePreference, Ui, vec2};
use std::time::{Duration, Instant};

const AUTOSAVE: Duration = Duration::from_millis(600);

pub struct App {
    store: Store,
    editor: Option<Editor>,
    wizard: Wizard,
    plans: Vec<Summary>,
    toast: Option<(String, Instant)>,
    confirm_delete: Option<String>,
    settings: SettingsDraft,
}

#[derive(Default)]
struct SettingsDraft {
    target: String,
    open_for: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store) -> Self {
        theme::apply(&cc.egui_ctx);
        let plans = store.list();
        App {
            store,
            editor: None,
            wizard: Wizard::default(),
            plans,
            toast: None,
            confirm_delete: None,
            settings: SettingsDraft::default(),
        }
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn save_now(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        ed.project.modified = chrono::Utc::now();
        match self.store.save(&ed.project) {
            Ok(()) => ed.dirty = false,
            Err(e) => {
                let msg = format!("Couldn't save: {e}");
                self.say(msg);
            }
        }
    }

    fn close_editor(&mut self) {
        self.save_now();
        self.editor = None;
        self.plans = self.store.list();
    }

    fn open(&mut self, id: &str) {
        match self.store.load(id) {
            Ok(p) => self.editor = Some(Editor::new(p)),
            Err(e) => self.say(format!("Couldn't open that plan: {e}")),
        }
    }

    fn start_screen(&mut self, ui: &mut Ui) {
        let Some(choice) = start::show(ui, &mut self.wizard, &self.plans, &self.store) else { return };
        match choice {
            Choice::Create(n) => {
                let p = Project::create(&n);
                if let Err(e) = self.store.save(&p) {
                    self.say(format!("Couldn't save: {e}"));
                }
                self.editor = Some(Editor::new(p));
                self.wizard = Wizard::default();
            }
            Choice::Open(id) => self.open(&id),
            Choice::Delete(id) => self.confirm_delete = Some(id),
            Choice::Import => {
                if let Some(path) = rfd::FileDialog::new().add_filter("Plan", &["json"]).pick_file() {
                    match self.store.import(&path) {
                        Ok(p) => {
                            self.plans = self.store.list();
                            self.say(format!("Imported “{}”", p.name));
                        }
                        Err(e) => self.say(format!("That file isn't a plan: {e}")),
                    }
                }
            }
        }
    }

    fn editor_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = Palette::of(&ctx);
        let mut actions = Actions::default();
        let Some(ed) = self.editor.as_mut() else { return };
        egui::Panel::top("top")
            .frame(Frame::NONE.fill(p.surface).stroke(egui::Stroke::new(1.0, p.border)))
            .exact_size(52.0)
            .show(ui, |ui| panels::top_bar(ed, ui, &mut actions));
        egui::Panel::bottom("bottom")
            .frame(Frame::NONE.fill(p.surface).stroke(egui::Stroke::new(1.0, p.border)))
            .exact_size(40.0)
            .show(ui, |ui| panels::bottom_bar(ed, ui));
        egui::Panel::left("tools")
            .frame(Frame::NONE.fill(p.surface).stroke(egui::Stroke::new(1.0, p.border)))
            .exact_size(64.0)
            .resizable(false)
            .show(ui, |ui| panels::tool_column(ed, ui));
        egui::Panel::right("side")
            .frame(Frame::NONE.fill(p.bg).stroke(egui::Stroke::new(1.0, p.border)).inner_margin(Margin::symmetric(14, 0)))
            .exact_size(304.0)
            .resizable(false)
            .show(ui, |ui| panels::side_panel(ed, ui));
        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| canvas::show(ed, ui));

        if ed.show_settings {
            self.settings_modal(&ctx);
        }
        if let Some(ed) = self.editor.as_mut() {
            if ed.show_export {
                export::modal(&ctx, ed);
            }
        }

        if let Some(pref) = actions.theme {
            ctx.set_theme(pref);
        }
        if actions.home {
            self.close_editor();
        }
    }

    fn settings_modal(&mut self, ctx: &Context) {
        let Some(ed) = self.editor.as_mut() else { return };
        let p = Palette::of(ctx);
        if self.settings.open_for.as_deref() != Some(ed.project.id.as_str()) {
            self.settings.open_for = Some(ed.project.id.clone());
            self.settings.target = format!("{}", ed.project.units.area_value(ed.project.target_area).round());
        }
        let mut close = false;
        let resp = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(kit::serif("Plan settings", 24.0, p.text));
            ui.add_space(4.0);
            ui.label(kit::text("Units", 13.0, p.muted));
            let mut units = ed.project.units;
            if kit::segmented(ui, &mut units, &[(Units::Imperial, "Feet & inches"), (Units::Metric, "Metric")]) {
                ed.set_units(units);
                self.settings.target = format!("{}", units.area_value(ed.project.target_area).round());
            }
            ui.add_space(4.0);
            ui.label(kit::text(format!("Target living area ({})", ed.project.units.area_unit()), 13.0, p.muted));
            ui.add(egui::TextEdit::singleline(&mut self.settings.target).margin(Margin::symmetric(10, 8)).desired_width(160.0));
            let parsed = self.settings.target.trim().replace(',', "").parse::<f32>().ok().filter(|a| *a >= 50.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(parsed.is_some(), |ui| {
                    if kit::small_button(ui, Btn::Ghost, "Apply target").clicked() {
                        if let Some(a) = parsed {
                            ed.set_target_area(ed.project.units.area_to_sq_inches(a));
                        }
                    }
                    if kit::small_button(ui, Btn::Ghost, "Resize footprint to target").clicked() {
                        if let Some(a) = parsed {
                            ed.set_target_area(ed.project.units.area_to_sq_inches(a));
                            let per_floor = ed.project.target_area / ed.project.floors.len() as f32;
                            let ratio = (ed.project.footprint.w / ed.project.footprint.h).max(0.1);
                            let fp = Footprint::from_area(per_floor, ratio, ed.project.snap);
                            ed.set_footprint(fp.w, fp.h);
                            ed.view_ready = false;
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.label(kit::text("Footprint (per floor)", 13.0, p.muted));
            let fp = ed.project.footprint;
            ui.horizontal(|ui| {
                if let Some(w) = kit::length_field(ui, "fp_w", fp.w, ed.project.units, 110.0) {
                    ed.set_footprint(w, fp.h);
                }
                ui.label(kit::text("×", 14.0, p.muted));
                if let Some(h) = kit::length_field(ui, "fp_h", fp.h, ed.project.units, 110.0) {
                    ed.set_footprint(fp.w, h);
                }
            });
            ui.add_space(6.0);
            ui.label(kit::text("Floors", 13.0, p.muted));
            let mut delete = None;
            let n = ed.project.floors.len();
            for i in 0..n {
                ui.horizontal(|ui| {
                    let mut name = ed.project.floors[i].name.clone();
                    if ui.add(egui::TextEdit::singleline(&mut name).margin(Margin::symmetric(10, 6)).desired_width(220.0)).changed() {
                        ed.rename_floor(i, name);
                    }
                    if n > 1 && kit::small_button(ui, Btn::Danger, "Delete").clicked() {
                        delete = Some(i);
                    }
                });
            }
            if let Some(i) = delete {
                ed.delete_floor(i);
            }
            if n < 8 && kit::small_button(ui, Btn::Ghost, "Add floor").clicked() {
                ed.add_floor();
            }
            ui.add_space(8.0);
            ui.label(kit::text(
                "Note: areas are added up room by room, so overlapping rooms count twice. Keep rooms side by side for an accurate total.",
                11.5,
                p.faint,
            ));
            ui.add_space(8.0);
            if kit::button(ui, Btn::Primary, "Done").clicked() {
                close = true;
            }
        });
        if resp.should_close() || close {
            ed.show_settings = false;
            self.settings.open_for = None;
        }
    }

    fn delete_modal(&mut self, ctx: &Context) {
        let Some(id) = self.confirm_delete.clone() else { return };
        let p = Palette::of(ctx);
        let name = self.plans.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        let (mut cancel, mut yes) = (false, false);
        let r = egui::Modal::new(egui::Id::new("confirm_delete")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(kit::serif("Delete this plan?", 22.0, p.text));
            ui.label(kit::text(format!("“{name}” will be removed from this computer. This can't be undone."), 13.0, p.muted));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if kit::button(ui, Btn::Ghost, "Cancel").clicked() {
                    cancel = true;
                }
                if kit::button(ui, Btn::Danger, "Delete plan").clicked() {
                    yes = true;
                }
            });
        });
        if yes {
            if let Err(e) = self.store.delete(&id) {
                self.say(format!("Couldn't delete: {e}"));
            }
            self.plans = self.store.list();
            self.confirm_delete = None;
        } else if cancel || r.should_close() {
            self.confirm_delete = None;
        }
    }

    fn toast_ui(&mut self, ctx: &Context) {
        let Some((msg, at)) = &self.toast else { return };
        if at.elapsed() > Duration::from_secs(5) {
            self.toast = None;
            return;
        }
        let p = Palette::of(ctx);
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -56.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::NONE.fill(p.text).corner_radius(10).inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.label(kit::text(msg.clone(), 13.5, p.bg));
                });
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = Palette::of(&ctx);
        if self.editor.is_some() {
            self.editor_screen(ui);
        } else {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(p.bg))
                .show(ui, |ui| self.start_screen(ui));
        }
        self.delete_modal(&ctx);
        self.toast_ui(&ctx);

        // Autosave shortly after the last edit.
        if let Some(ed) = &self.editor {
            if ed.dirty {
                if ed.last_edit.elapsed() >= AUTOSAVE && matches!(ed.drag, crate::editor::Drag::None) {
                    self.save_now();
                } else {
                    ctx.request_repaint_after(AUTOSAVE);
                }
            }
        }
        let _ = ThemePreference::System;
    }

    fn on_exit(&mut self) {
        self.save_now();
    }
}
