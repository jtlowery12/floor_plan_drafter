//! Export a plan as a to-scale sheet: SVG, PNG or PDF, with a scale bar and
//! title block. The sheet is built from the same drawing commands the canvas
//! uses, so the export matches the screen.

use crate::editor::Editor;
use crate::geom::*;
use crate::kit::{self, Btn};
use crate::model::*;
use crate::paint;
use crate::render::{self, Cmd, Opts, PaperTheme, View, Weight};
use crate::theme::{self, Palette};
use crate::units::Units;
use eframe::egui::{self, Color32, Context, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use std::fmt::Write as _;
use svg2pdf::usvg;
use std::path::{Path, PathBuf};

const PX_PER_IN: f32 = 96.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Pdf,
    Png,
    Svg,
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Png => "png",
            Format::Svg => "svg",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Paper {
    Letter,
    A4,
    Tabloid,
    A3,
}

impl Paper {
    const ALL: [Paper; 4] = [Paper::Letter, Paper::A4, Paper::Tabloid, Paper::A3];
    fn label(self) -> &'static str {
        match self {
            Paper::Letter => "Letter  8.5 × 11 in",
            Paper::A4 => "A4  210 × 297 mm",
            Paper::Tabloid => "Tabloid  11 × 17 in",
            Paper::A3 => "A3  297 × 420 mm",
        }
    }
    /// Portrait size in inches.
    fn inches(self) -> (f32, f32) {
        match self {
            Paper::Letter => (8.5, 11.0),
            Paper::A4 => (8.268, 11.693),
            Paper::Tabloid => (11.0, 17.0),
            Paper::A3 => (11.693, 16.535),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub format: Format,
    pub paper: Paper,
    pub landscape: bool,
    pub all_floors: bool,
    pub dims: bool,
    pub labels: bool,
    pub grid: bool,
    pub footprint: bool,
    pub title_block: bool,
    pub status: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            format: Format::Pdf,
            paper: Paper::Letter,
            landscape: true,
            all_floors: false,
            dims: true,
            labels: true,
            grid: false,
            footprint: true,
            title_block: true,
            status: None,
        }
    }
}

/// One finished page: everything in page pixels (96 per inch).
pub struct Sheet {
    pub w: f32,
    pub h: f32,
    pub cmds: Vec<Cmd>,
    pub scale_label: String,
}

fn poly(pts: Vec<V2>, closed: bool, fill: Option<Color32>, stroke: Option<(Color32, f32)>) -> Cmd {
    Cmd::Poly { pts, closed, fill, stroke, dash: None }
}

fn text(at: V2, s: &str, size: f32, color: Color32, weight: Weight, left: bool) -> Cmd {
    Cmd::Text { at, text: s.to_string(), size, color, weight, angle: 0.0, halo: None, left }
}

fn rect_pts(x: f32, y: f32, w: f32, h: f32) -> Vec<V2> {
    vec![v2(x, y), v2(x + w, y), v2(x + w, y + h), v2(x, y + h)]
}

struct ScalePick {
    px_per_inch: f32,
    label: String,
}

/// Largest standard drawing scale that fits, or the exact fit marked "not to scale".
fn pick_scale(units: Units, fit: f32) -> ScalePick {
    let candidates: Vec<(f32, String)> = match units {
        Units::Imperial => [
            (0.5, "1/2\" = 1'-0\""),
            (0.375, "3/8\" = 1'-0\""),
            (0.25, "1/4\" = 1'-0\""),
            (0.1875, "3/16\" = 1'-0\""),
            (0.125, "1/8\" = 1'-0\""),
            (0.09375, "3/32\" = 1'-0\""),
            (0.0625, "1/16\" = 1'-0\""),
            (0.03125, "1/32\" = 1'-0\""),
        ]
        .iter()
        .map(|(paper_in_per_ft, l)| (PX_PER_IN * paper_in_per_ft / 12.0, l.to_string()))
        .collect(),
        Units::Metric => [20.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 250.0, 500.0, 1000.0]
            .iter()
            .map(|r: &f32| (PX_PER_IN / r, format!("1:{}", *r as u32)))
            .collect(),
    };
    for (px, label) in &candidates {
        if *px <= fit {
            return ScalePick { px_per_inch: *px, label: label.clone() };
        }
    }
    ScalePick { px_per_inch: fit, label: "Not to scale".into() }
}

pub fn sheet(project: &Project, floor: usize, s: &Settings) -> Sheet {
    let th = PaperTheme::light();
    let units = project.units;
    let (pw, ph) = s.paper.inches();
    let (w, h) = if s.landscape { (ph, pw) } else { (pw, ph) };
    let (w, h) = (w * PX_PER_IN, h * PX_PER_IN);
    let margin = 0.45 * PX_PER_IN;
    let block = if s.title_block { 1.05 * PX_PER_IN } else { 0.0 };
    let area = Rect::from_min_max(pos2(margin, margin), pos2(w - margin, h - margin - block));
    let pad = 0.6 * PX_PER_IN;

    let (lo, hi) = project.extents(floor);
    let (ww, wh) = ((hi.x - lo.x).max(12.0), (hi.y - lo.y).max(12.0));
    let fit = ((area.width() - 2.0 * pad) / ww).min((area.height() - 2.0 * pad - 40.0) / wh).max(1e-4);
    let pick = pick_scale(units, fit);
    let scale = pick.px_per_inch;
    let center = (lo + hi) * 0.5;
    let area_c = v2(area.center().x, area.center().y - 14.0);
    let view = View { origin: center - area_c * (1.0 / scale), scale };

    let mut cmds: Vec<Cmd> = Vec::new();
    // Sheet border.
    cmds.push(poly(rect_pts(margin, margin, area.width(), h - 2.0 * margin), true, None, Some((th.ink, 1.6))));
    // Optional grid, clipped to the drawing area by construction.
    if s.grid {
        let gv = View { origin: view.to_world(v2(area.min.x, area.min.y)), scale };
        for c in render::grid(&gv, v2(area.width(), area.height()), &th, units) {
            if let Cmd::Poly { pts, closed, fill, stroke, dash } = c {
                let pts = pts.into_iter().map(|p| p + v2(area.min.x, area.min.y)).collect();
                cmds.push(Cmd::Poly { pts, closed, fill, stroke, dash });
            }
        }
    }
    let opts = Opts { grid: false, dims: s.dims, footprint: s.footprint, labels: s.labels };
    cmds.extend(render::plan(project, floor, &view, &th, &opts));
    scale_bar(&mut cmds, units, scale, v2(area.min.x + 22.0, area.max.y - 26.0), &th);
    if s.title_block {
        title_block(&mut cmds, project, floor, &pick.label, w, h, margin, block, &th);
    }
    Sheet { w, h, cmds, scale_label: pick.label }
}

fn scale_bar(cmds: &mut Vec<Cmd>, units: Units, scale: f32, at: V2, th: &PaperTheme) {
    let steps: &[f32] = match units {
        Units::Imperial => &[12.0, 24.0, 60.0, 120.0, 240.0, 600.0, 1200.0, 2400.0],
        Units::Metric => &[39.37, 98.43, 196.85, 393.7, 984.3, 1968.5, 3937.0, 9843.0],
    };
    let target = 1.4 * PX_PER_IN / scale;
    let len = steps.iter().copied().min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs())).unwrap_or(120.0);
    let px = len * scale;
    let half = px / 2.0;
    let bh = 6.0;
    cmds.push(poly(rect_pts(at.x, at.y, half, bh), true, Some(th.ink), Some((th.ink, 1.0))));
    cmds.push(poly(rect_pts(at.x + half, at.y, half, bh), true, Some(Color32::WHITE), Some((th.ink, 1.0))));
    cmds.push(text(v2(at.x, at.y - 9.0), "0", 10.5, th.ink, Weight::Medium, false));
    cmds.push(text(v2(at.x + px, at.y - 9.0), &units.len(len), 10.5, th.ink, Weight::Medium, false));
    cmds.push(text(v2(at.x + px / 2.0, at.y + bh + 11.0), "Scale bar", 9.5, th.ink_soft, Weight::Regular, false));
}

#[allow(clippy::too_many_arguments)]
fn title_block(cmds: &mut Vec<Cmd>, p: &Project, floor: usize, scale: &str, w: f32, h: f32, margin: f32, block: f32, th: &PaperTheme) {
    let x0 = margin;
    let y0 = h - margin - block;
    let bw = w - 2.0 * margin;
    cmds.push(poly(rect_pts(x0, y0, bw, block), true, None, Some((th.ink, 1.6))));
    let cols = [0.0, bw * 0.46, bw * 0.74];
    for c in &cols[1..] {
        cmds.push(poly(vec![v2(x0 + c, y0), v2(x0 + c, y0 + block)], false, None, Some((th.ink, 1.0))));
    }
    let pad = 16.0;
    let units = p.units;
    let f = &p.floors[floor];
    let date = chrono::Local::now().format("%b %-d, %Y").to_string();
    let name = render::fit_text(&p.name, bw * 0.46 - 2.0 * pad, 22.0);
    cmds.push(text(v2(x0 + pad, y0 + 26.0), &name, 22.0, th.ink, Weight::Semibold, true));
    cmds.push(text(v2(x0 + pad, y0 + 52.0), &format!("{}  ·  {}", f.name, date), 12.5, th.ink_soft, Weight::Regular, true));
    cmds.push(text(v2(x0 + pad, y0 + 74.0), "Concept sketch for discussion with an architect. Not a construction document.", 10.0, th.muted, Weight::Regular, true));
    let c1 = x0 + cols[1] + pad;
    cmds.push(text(v2(c1, y0 + 20.0), "LIVING AREA", 9.5, th.muted, Weight::Semibold, true));
    cmds.push(text(v2(c1, y0 + 41.0), &format!("This floor  {}", units.area(f.area())), 13.0, th.ink, Weight::Medium, true));
    cmds.push(text(v2(c1, y0 + 61.0), &format!("All floors  {}  of  {}", units.area(p.total_area()), units.area(p.target_area)), 13.0, th.ink, Weight::Regular, true));
    let c2 = x0 + cols[2] + pad;
    cmds.push(text(v2(c2, y0 + 20.0), "SCALE", 9.5, th.muted, Weight::Semibold, true));
    cmds.push(text(v2(c2, y0 + 44.0), scale, 16.0, th.ink, Weight::Semibold, true));
    cmds.push(text(v2(c2, y0 + 66.0), "Print at 100% for true scale", 10.0, th.muted, Weight::Regular, true));
}

// ---- SVG -------------------------------------------------------------------------

fn col(c: Color32) -> (String, f32) {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    (format!("rgb({r},{g},{b})"), a as f32 / 255.0)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn svg(sheet: &Sheet) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n",
        w = sheet.w,
        h = sheet.h
    );
    for c in &sheet.cmds {
        match c {
            Cmd::Poly { pts, closed, fill, stroke, dash } => {
                if pts.len() < 2 {
                    continue;
                }
                let list: String = pts.iter().map(|p| format!("{:.2},{:.2}", p.x, p.y)).collect::<Vec<_>>().join(" ");
                let tag = if *closed { "polygon" } else { "polyline" };
                let mut a = String::new();
                match fill {
                    Some(f) if *closed => {
                        let (c, op) = col(*f);
                        let _ = write!(a, " fill=\"{c}\" fill-opacity=\"{op:.3}\"");
                    }
                    _ => a.push_str(" fill=\"none\""),
                }
                if let Some((c, wd)) = stroke {
                    let (c, op) = col(*c);
                    let _ = write!(a, " stroke=\"{c}\" stroke-opacity=\"{op:.3}\" stroke-width=\"{wd:.2}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"");
                    if let Some((d, g)) = dash {
                        let _ = write!(a, " stroke-dasharray=\"{d:.1} {g:.1}\"");
                    }
                }
                let _ = writeln!(o, "<{tag} points=\"{list}\"{a}/>");
            }
            Cmd::Text { at, text, size, color, weight, angle, halo, left } => {
                let wt = match weight {
                    Weight::Regular => 400,
                    Weight::Medium => 500,
                    Weight::Semibold => 600,
                };
                let (c, op) = col(*color);
                let halo_attr = halo.map(|h| {
                    let (hc, _) = col(h);
                    format!(" stroke=\"{hc}\" stroke-width=\"3.4\" stroke-linejoin=\"round\" paint-order=\"stroke\"")
                });
                let _ = writeln!(
                    o,
                    "<g transform=\"translate({:.2} {:.2}) rotate({:.3})\"><text x=\"0\" y=\"{:.2}\" text-anchor=\"{}\" font-family=\"Inter\" font-size=\"{size}\" font-weight=\"{wt}\" fill=\"{c}\" fill-opacity=\"{op:.3}\"{}>{}</text></g>",
                    at.x,
                    at.y,
                    angle.to_degrees(),
                    size * 0.36,
                    if *left { "start" } else { "middle" },
                    halo_attr.unwrap_or_default(),
                    esc(text)
                );
            }
        }
    }
    o.push_str("</svg>\n");
    o
}

// ---- rasterise / PDF --------------------------------------------------------------------

fn options() -> usvg::Options<'static> {
    let mut db = usvg::fontdb::Database::new();
    for f in [theme::INTER_REGULAR, theme::INTER_MEDIUM, theme::INTER_SEMIBOLD] {
        db.load_font_data(f.to_vec());
    }
    db.set_sans_serif_family("Inter");
    let mut o = usvg::Options::default();
    o.font_family = "Inter".into();
    *o.fontdb_mut() = db;
    o
}

fn tree(sheet: &Sheet) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(&svg(sheet), &options()).map_err(|e| e.to_string())
}

pub fn png(sheet: &Sheet, dpi: f32) -> Result<Vec<u8>, String> {
    let t = tree(sheet)?;
    let k = dpi / PX_PER_IN;
    let (w, h) = ((sheet.w * k).ceil() as u32, (sheet.h * k).ceil() as u32);
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).ok_or("page too large")?;
    resvg::render(&t, resvg::tiny_skia::Transform::from_scale(k, k), &mut pix.as_mut());
    pix.encode_png().map_err(|e| e.to_string())
}

/// One PDF, one page per sheet.
pub fn pdf(sheets: &[Sheet]) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect as PRect, Ref};
    use std::collections::HashMap;
    let mut alloc = Ref::new(1);
    let catalog = alloc.bump();
    let page_tree = alloc.bump();
    let mut pages = Vec::new();
    let mut pdf = Pdf::new();
    let k = 72.0 / PX_PER_IN;
    for sh in sheets {
        let t = tree(sh)?;
        let (mut chunk, xref) = svg2pdf::to_chunk(&t, svg2pdf::ConversionOptions::default()).map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let xref = *map.get(&xref).ok_or("pdf build failed")?;
        let (page, content_id) = (alloc.bump(), alloc.bump());
        let (pw, ph) = (sh.w * k, sh.h * k);
        let mut p = pdf.page(page);
        p.media_box(PRect::new(0.0, 0.0, pw, ph));
        p.parent(page_tree);
        p.contents(content_id);
        p.resources().x_objects().pair(Name(b"S1"), xref);
        p.finish();
        let mut c = Content::new();
        c.transform([pw, 0.0, 0.0, ph, 0.0, 0.0]).x_object(Name(b"S1"));
        pdf.stream(content_id, &c.finish());
        pdf.extend(&chunk);
        pages.push(page);
    }
    pdf.catalog(catalog).pages(page_tree);
    pdf.pages(page_tree).kids(pages.iter().copied()).count(pages.len() as i32);
    Ok(pdf.finish())
}

fn safe_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let n = n.trim().to_string();
    if n.is_empty() { "plan".into() } else { n }
}

/// Run the export. Returns a human-readable result.
pub fn run(project: &Project, floor: usize, s: &Settings, dest: &Path) -> Result<String, String> {
    let floors: Vec<usize> = if s.all_floors { (0..project.floors.len()).collect() } else { vec![floor] };
    let sheets: Vec<Sheet> = floors.iter().map(|&f| sheet(project, f, s)).collect();
    match s.format {
        Format::Pdf => {
            std::fs::write(dest, pdf(&sheets)?).map_err(|e| e.to_string())?;
            Ok(format!("Saved {}", dest.display()))
        }
        fmt => {
            let stem = dest.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "plan".into());
            let dir = dest.parent().unwrap_or(Path::new("."));
            let mut last = PathBuf::new();
            for (i, sh) in sheets.iter().enumerate() {
                let path = if sheets.len() == 1 {
                    dest.to_path_buf()
                } else {
                    dir.join(format!("{stem} - {}.{}", safe_name(&project.floors[floors[i]].name), fmt.ext()))
                };
                let bytes = match fmt {
                    Format::Png => png(sh, 220.0)?,
                    _ => svg(sh).into_bytes(),
                };
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                last = path;
            }
            Ok(if sheets.len() == 1 { format!("Saved {}", last.display()) } else { format!("Saved {} files in {}", sheets.len(), dir.display()) })
        }
    }
}

// ---- modal ----------------------------------------------------------------------------------

fn scaled(cmds: &[Cmd], k: f32) -> Vec<Cmd> {
    cmds.iter()
        .map(|c| match c {
            Cmd::Poly { pts, closed, fill, stroke, dash } => Cmd::Poly {
                pts: pts.iter().map(|&p| p * k).collect(),
                closed: *closed,
                fill: *fill,
                stroke: stroke.map(|(c, w)| (c, (w * k).max(0.6))),
                dash: dash.map(|(d, g)| (d * k, g * k)),
            },
            Cmd::Text { at, text, size, color, weight, angle, halo, left } => Cmd::Text {
                left: *left,
                at: *at * k,
                text: text.clone(),
                size: size * k,
                color: *color,
                weight: *weight,
                angle: *angle,
                halo: *halo,
            },
        })
        .collect()
}

fn preview(ui: &mut Ui, sh: &Sheet, max: egui::Vec2) {
    let k = (max.x / sh.w).min(max.y / sh.h);
    let size = vec2(sh.w * k, sh.h * k);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect.translate(vec2(0.0, 3.0)), 3.0, Color32::from_black_alpha(40));
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    paint::paint(&painter, rect.min, &scaled(&sh.cmds, k));
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(190)), StrokeKind::Inside);
}

pub fn modal(ctx: &Context, ed: &mut Editor) {
    let p = Palette::of(ctx);
    let id = egui::Id::new("export_settings");
    let mut s: Settings = ctx.data_mut(|d| d.get_temp::<Settings>(id)).unwrap_or_default();
    let mut close = false;
    let mut go = false;
    let resp = egui::Modal::new(egui::Id::new("export_modal")).show(ctx, |ui| {
        ui.set_width(780.0);
        ui.label(kit::serif("Export for your architect", 24.0, p.text));
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.label(kit::text("Format", 13.0, p.muted));
                kit::segmented(ui, &mut s.format, &[(Format::Pdf, "PDF"), (Format::Png, "PNG image"), (Format::Svg, "SVG")]);
                ui.label(kit::text("Paper", 13.0, p.muted));
                egui::ComboBox::from_id_salt("paper").selected_text(s.paper.label()).width(260.0).show_ui(ui, |ui| {
                    for pp in Paper::ALL {
                        ui.selectable_value(&mut s.paper, pp, pp.label());
                    }
                });
                kit::segmented(ui, &mut s.landscape, &[(true, "Landscape"), (false, "Portrait")]);
                if ed.project.floors.len() > 1 {
                    ui.label(kit::text("Floors", 13.0, p.muted));
                    kit::segmented(ui, &mut s.all_floors, &[(false, "This floor"), (true, "All floors")]);
                }
                ui.label(kit::text("Include", 13.0, p.muted));
                ui.checkbox(&mut s.dims, "Dimensions");
                ui.checkbox(&mut s.labels, "Room names and areas");
                ui.checkbox(&mut s.footprint, "Footprint outline");
                ui.checkbox(&mut s.grid, "Grid");
                ui.checkbox(&mut s.title_block, "Title block and scale");
            });
            ui.add_space(12.0);
            let sh = sheet(&ed.project, ed.floor, &s);
            ui.vertical(|ui| {
                preview(ui, &sh, vec2(400.0, 360.0));
                ui.add_space(4.0);
                ui.label(kit::text(
                    format!("Drawn at {}. Print at 100% to keep the scale true.", sh.scale_label.to_lowercase()),
                    12.0,
                    p.muted,
                ));
            });
        });
        if let Some(msg) = &s.status {
            ui.add_space(4.0);
            ui.label(kit::text(msg, 12.5, p.success));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if kit::button(ui, Btn::Ghost, "Close").clicked() {
                close = true;
            }
            if kit::icon_text_button(ui, Btn::Primary, crate::icons::DOWNLOAD, "Save as…").clicked() {
                go = true;
            }
        });
    });
    if go {
        let fname = format!("{}.{}", safe_name(&ed.project.name), s.format.ext());
        let dlg = rfd::FileDialog::new().set_file_name(&fname).add_filter(s.format.ext(), &[s.format.ext()]);
        if let Some(dest) = dlg.save_file() {
            s.status = Some(match run(&ed.project, ed.floor, &s, &dest) {
                Ok(m) => m,
                Err(e) => format!("Export failed: {e}"),
            });
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, s));
    if close || resp.should_close() {
        ed.show_export = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sample() -> Project {
        let mut p = Project::create(&NewPlan {
            name: "Lake house & studio".into(),
            units: Units::Imperial,
            area: 1800.0,
            floors: 2,
            proportion: Proportion::Classic,
        });
        let mut r = Shape::boxed(Kind::Rect, v2(180.0, 150.0), v2(300.0, 240.0));
        r.label = "Living room".into();
        p.add_shape(0, r);
        let mut k = Shape::boxed(Kind::Rect, v2(480.0, 150.0), v2(240.0, 240.0));
        k.label = "Kitchen".into();
        k.color = 1;
        p.add_shape(0, k);
        p.add_shape(0, Shape::wall(v2(30.0, 30.0), v2(630.0, 30.0)));
        p.add_shape(0, Shape::boxed(Kind::Furniture(Sym::Sofa), v2(150.0, 220.0), Sym::Sofa.default_size()));
        p
    }

    #[test]
    fn exports_png_pdf_and_svg() {
        let p = sample();
        let s = Settings::default();
        let dir = std::env::temp_dir().join(format!("dt-export-{}", fresh_id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (fmt, name) in [(Format::Pdf, "a.pdf"), (Format::Png, "a.png"), (Format::Svg, "a.svg")] {
            let st = Settings { format: fmt, all_floors: fmt == Format::Pdf, ..s.clone() };
            let msg = run(&p, 0, &st, &dir.join(name)).unwrap();
            assert!(msg.starts_with("Saved"), "{msg}");
        }
        let pdf = std::fs::read(dir.join("a.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let png = std::fs::read(dir.join("a.png")).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        if let Ok(out) = std::env::var("DT_EXPORT_KEEP") {
            std::fs::create_dir_all(&out).unwrap();
            for n in ["a.pdf", "a.png", "a.svg"] {
                std::fs::copy(dir.join(n), format!("{out}/{n}")).unwrap();
            }
        }
    }

    #[test]
    fn picks_a_standard_scale_when_it_fits() {
        let pick = pick_scale(Units::Imperial, PX_PER_IN * 0.25 / 12.0 + 0.01);
        assert_eq!(pick.label, "1/4\" = 1'-0\"");
        let tiny = pick_scale(Units::Imperial, 0.01);
        assert_eq!(tiny.label, "Not to scale");
    }
}
