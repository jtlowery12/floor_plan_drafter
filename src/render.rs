//! Turns a plan into drawing commands in screen/page pixels. The same command
//! list is painted by egui on the canvas and written out as SVG for export, so
//! what you see is exactly what you hand to the architect.

use crate::geom::*;
use crate::model::*;
use crate::symbols;
use crate::units::Units;
use eframe::egui::Color32;

/// Maps world inches to pixels: `screen = (world - origin) * scale`.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: V2,
    pub scale: f32,
}

impl View {
    pub fn to_screen(&self, w: V2) -> V2 {
        (w - self.origin) * self.scale
    }
    pub fn to_world(&self, s: V2) -> V2 {
        s * (1.0 / self.scale) + self.origin
    }
    /// A view that fits `world` (min, max) into `size` pixels with a margin.
    pub fn fit(world: (V2, V2), size: V2, margin: f32) -> View {
        let (lo, hi) = world;
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let scale = ((size.x - 2.0 * margin) / w).min((size.y - 2.0 * margin) / h).max(1e-4);
        let center = (lo + hi) * 0.5;
        View { origin: center - (size * (0.5 / scale)), scale }
    }
    /// Keep the world point under `screen` fixed while changing the scale.
    pub fn zoom_at(&mut self, screen: V2, factor: f32, min: f32, max: f32) {
        let anchor = self.to_world(screen);
        self.scale = (self.scale * factor).clamp(min, max);
        self.origin = anchor - screen * (1.0 / self.scale);
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
}

#[derive(Clone, Debug)]
pub enum Cmd {
    Poly {
        pts: Vec<V2>,
        closed: bool,
        fill: Option<Color32>,
        stroke: Option<(Color32, f32)>,
        dash: Option<(f32, f32)>,
    },
    Text {
        at: V2,
        text: String,
        size: f32,
        color: Color32,
        weight: Weight,
        angle: f32,
        halo: Option<Color32>,
        left: bool,
    },
}

pub struct PaperTheme {
    pub dark: bool,
    pub outside: Color32,
    pub paper: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub ink: Color32,
    pub ink_soft: Color32,
    pub muted: Color32,
    pub footprint: Color32,
    pub accent: Color32,
    pub furniture: Color32,
    pub glass: Color32,
    pub rooms: [Color32; ROOM_COLORS],
}

pub const ROOM_COLOR_NAMES: [&str; ROOM_COLORS] =
    ["Linen", "Sage", "Sky", "Blush", "Sand", "Lilac", "Mint", "Peach"];

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl PaperTheme {
    pub fn light() -> Self {
        PaperTheme {
            dark: false,
            outside: rgb(0xEC, 0xEF, 0xF4),
            paper: rgb(0xFE, 0xFD, 0xFA),
            grid_minor: rgb(0xEA, 0xEE, 0xF4),
            grid_major: rgb(0xD5, 0xDC, 0xE8),
            ink: rgb(0x2A, 0x31, 0x40),
            ink_soft: rgb(0x6B, 0x76, 0x8A),
            muted: rgb(0x7D, 0x87, 0x9A),
            footprint: rgb(0x5B, 0x7F, 0xB8),
            accent: rgb(0x2F, 0x6F, 0xEB),
            furniture: rgb(0xFF, 0xFF, 0xFF),
            glass: rgb(0xDD, 0xEC, 0xFA),
            rooms: [
                rgb(0xF1, 0xEB, 0xDF),
                rgb(0xD9, 0xE8, 0xDA),
                rgb(0xD6, 0xE6, 0xF5),
                rgb(0xF6, 0xDC, 0xDF),
                rgb(0xF3, 0xE3, 0xC3),
                rgb(0xE3, 0xDD, 0xF3),
                rgb(0xD2, 0xEE, 0xE9),
                rgb(0xF9, 0xDD, 0xCB),
            ],
        }
    }

    /// A blueprint: light linework on deep blue.
    pub fn dark() -> Self {
        PaperTheme {
            dark: true,
            outside: rgb(0x0A, 0x14, 0x24),
            paper: rgb(0x0F, 0x1F, 0x38),
            grid_minor: rgb(0x15, 0x29, 0x47),
            grid_major: rgb(0x20, 0x3A, 0x62),
            ink: rgb(0xE6, 0xEE, 0xFA),
            ink_soft: rgb(0x93, 0xA8, 0xCC),
            muted: rgb(0x7F, 0x95, 0xB8),
            footprint: rgb(0x6E, 0xA0, 0xFF),
            accent: rgb(0x6E, 0xA0, 0xFF),
            furniture: rgb(0x16, 0x2B, 0x4A),
            glass: rgb(0x1E, 0x3D, 0x66),
            rooms: [
                rgb(0x2A, 0x38, 0x52),
                rgb(0x22, 0x4A, 0x45),
                rgb(0x1F, 0x46, 0x72),
                rgb(0x55, 0x33, 0x46),
                rgb(0x55, 0x48, 0x2E),
                rgb(0x43, 0x3C, 0x6C),
                rgb(0x1D, 0x4E, 0x50),
                rgb(0x61, 0x3F, 0x31),
            ],
        }
    }

    pub fn room(&self, i: u8) -> Color32 {
        self.rooms[i as usize % ROOM_COLORS]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub grid: bool,
    pub dims: bool,
    pub footprint: bool,
    pub labels: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { grid: true, dims: true, footprint: true, labels: true }
    }
}

fn poly(pts: Vec<V2>, closed: bool, fill: Option<Color32>, stroke: Option<(Color32, f32)>) -> Cmd {
    Cmd::Poly { pts, closed, fill, stroke, dash: None }
}

/// Faint grid lines across a `size`-pixel viewport.
pub fn grid(view: &View, size: V2, th: &PaperTheme, units: Units) -> Vec<Cmd> {
    let (fine, heavy) = units.grid_steps(view.scale);
    let lo = view.to_world(V2::ZERO);
    let hi = view.to_world(size);
    let mut out = Vec::new();
    let mut lines = |step: f32, color: Color32, width: f32, skip: Option<f32>| {
        let mut x = (lo.x / step).floor() * step;
        while x <= hi.x {
            if skip.is_none_or(|s| ((x / s).round() * s - x).abs() > 1e-3) {
                let sx = view.to_screen(v2(x, 0.0)).x.round() + 0.5;
                out.push(poly(vec![v2(sx, 0.0), v2(sx, size.y)], false, None, Some((color, width))));
            }
            x += step;
        }
        let mut y = (lo.y / step).floor() * step;
        while y <= hi.y {
            if skip.is_none_or(|s| ((y / s).round() * s - y).abs() > 1e-3) {
                let sy = view.to_screen(v2(0.0, y)).y.round() + 0.5;
                out.push(poly(vec![v2(0.0, sy), v2(size.x, sy)], false, None, Some((color, width))));
            }
            y += step;
        }
    };
    lines(fine, th.grid_minor, 1.0, Some(heavy));
    lines(heavy, th.grid_major, 1.0, None);
    out
}

/// Everything on one floor: footprint, rooms, walls, openings, furniture.
pub fn plan(project: &Project, floor: usize, view: &View, th: &PaperTheme, o: &Opts) -> Vec<Cmd> {
    let units = project.units;
    let mut out = Vec::new();
    let f = &project.floors[floor];

    if o.footprint {
        footprint(project, view, th, &mut out);
    }
    for i in f.draw_order() {
        shape(&f.shapes[i], view, th, o, units, &mut out);
    }
    out
}

fn footprint(project: &Project, view: &View, th: &PaperTheme, out: &mut Vec<Cmd>) {
    let fp = project.footprint;
    let pts: Vec<V2> = fp.rect().iter().map(|&p| view.to_screen(p)).collect();
    out.push(Cmd::Poly {
        pts: pts.clone(),
        closed: true,
        fill: None,
        stroke: Some((th.footprint, 1.6)),
        dash: Some((7.0, 5.0)),
    });
    let units = project.units;
    let mid_top = (pts[0] + pts[1]) * 0.5;
    let mid_left = (pts[0] + pts[3]) * 0.5;
    out.push(Cmd::Text {
        at: mid_top + v2(0.0, -14.0),
        text: units.len(fp.w),
        size: 12.0,
        color: th.footprint,
        weight: Weight::Semibold,
        angle: 0.0,
        halo: Some(th.paper),
        left: false,
    });
    out.push(Cmd::Text {
        at: mid_left + v2(-14.0, 0.0),
        text: units.len(fp.h),
        size: 12.0,
        color: th.footprint,
        weight: Weight::Semibold,
        angle: -std::f32::consts::FRAC_PI_2,
        halo: Some(th.paper),
        left: false,
    });
}

/// Drawing commands for a single shape (used for placement ghosts).
pub fn shape_cmds(s: &Shape, view: &View, th: &PaperTheme, o: &Opts, units: Units) -> Vec<Cmd> {
    let mut out = Vec::new();
    shape(s, view, th, o, units, &mut out);
    out
}

fn shape(s: &Shape, view: &View, th: &PaperTheme, o: &Opts, units: Units, out: &mut Vec<Cmd>) {
    match s.kind {
        k if k.is_room() => room(s, view, th, o, units, out),
        Kind::Wall => wall(s, view, th, o, units, out),
        Kind::Door => door(s, view, th, out),
        Kind::SlidingDoor => sliding_door(s, view, th, out),
        Kind::Window => window(s, view, th, out),
        Kind::Furniture(sym) => furniture(s, sym, view, th, out),
        _ => {}
    }
}

fn room(s: &Shape, view: &View, th: &PaperTheme, o: &Opts, units: Units, out: &mut Vec<Cmd>) {
    let outline = s.outline();
    if outline.len() < 3 {
        return;
    }
    let pts: Vec<V2> = outline.iter().map(|&p| view.to_screen(p)).collect();
    out.push(poly(pts.clone(), true, Some(th.room(s.color)), Some((th.ink_soft, 1.4))));

    let (lo, hi) = bounds(&pts).unwrap_or((V2::ZERO, V2::ZERO));
    let (bw, bh) = (hi.x - lo.x, hi.y - lo.y);

    if o.dims && s.kind != Kind::Ellipse {
        let ccw = signed_area(&pts) > 0.0;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let len_px = a.dist(b);
            if len_px < 64.0 {
                continue;
            }
            let wa = outline[i];
            let wb = outline[(i + 1) % outline.len()];
            let dir = (b - a).normalized();
            // Outward normal depends on winding.
            let mut n = dir.perp();
            if ccw {
                n = -n;
            }
            let at = (a + b) * 0.5 + n * 11.0;
            out.push(dim_text(at, dir.angle(), units.len(wa.dist(wb)), th));
        }
    }

    if !o.labels || bw < 46.0 || bh < 30.0 {
        return;
    }
    let c = view.to_screen(s.centroid());
    let name = fit_text(&s.display_name(), bw - 12.0, 13.0);
    let show_area = bh >= 52.0;
    let name_y = if show_area { c.y - 8.0 } else { c.y };
    out.push(Cmd::Text {
        at: v2(c.x, name_y),
        text: name,
        size: 13.0,
        color: th.ink,
        weight: Weight::Semibold,
        angle: 0.0,
        halo: None,
        left: false,
    });
    if show_area {
        let mut line = units.area(s.area());
        if o.dims && s.kind == Kind::Rect && bw > 150.0 {
            line = format!("{} × {}  ·  {}", units.len(s.size.x), units.len(s.size.y), line);
        }
        out.push(Cmd::Text {
            at: v2(c.x, c.y + 9.0),
            text: fit_text(&line, bw - 12.0, 11.5),
            size: 11.5,
            color: th.ink_soft,
            weight: Weight::Regular,
            angle: 0.0,
            halo: None,
            left: false,
        });
    }
}

fn dim_text(at: V2, mut angle: f32, text: String, th: &PaperTheme) -> Cmd {
    // Keep text readable: never upside-down.
    if angle > std::f32::consts::FRAC_PI_2 {
        angle -= PI_F;
    } else if angle < -std::f32::consts::FRAC_PI_2 {
        angle += PI_F;
    }
    Cmd::Text {
        at,
        text,
        size: 11.0,
        color: th.muted,
        weight: Weight::Medium,
        angle,
        halo: Some(th.paper),
        left: false,
    }
}

const PI_F: f32 = std::f32::consts::PI;

/// Shorten text with an ellipsis so it roughly fits `max_px` at `size`.
pub fn fit_text(text: &str, max_px: f32, size: f32) -> String {
    let per_char = size * 0.58;
    let max_chars = (max_px / per_char).floor().max(1.0) as usize;
    if text.chars().count() <= max_chars {
        text.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        let mut t: String = text.chars().take(max_chars - 1).collect();
        t.push('…');
        t
    }
}

fn wall(s: &Shape, view: &View, th: &PaperTheme, o: &Opts, units: Units, out: &mut Vec<Cmd>) {
    let pts: Vec<V2> = s.outline().iter().map(|&p| view.to_screen(p)).collect();
    out.push(poly(pts, true, Some(th.ink), None));
    if o.dims {
        let (a, b) = s.wall_ends();
        let (sa, sb) = (view.to_screen(a), view.to_screen(b));
        if sa.dist(sb) >= 64.0 {
            let dir = (sb - sa).normalized();
            let at = (sa + sb) * 0.5 - dir.perp() * (s.thickness * view.scale / 2.0 + 10.0);
            out.push(dim_text(at, dir.angle(), units.len(a.dist(b)), th));
        }
    }
}

/// Local (along-wall, across-wall) inches to screen pixels for an opening or symbol.
fn frame(s: &Shape, view: &View) -> impl Fn(f32, f32) -> V2 + use<> {
    let (c, r, fx, fy, v) = (
        s.center,
        s.rot,
        if s.flip_x { -1.0 } else { 1.0 },
        if s.flip_y { -1.0 } else { 1.0 },
        *view,
    );
    move |x: f32, y: f32| v.to_screen(c + v2(x * fx, y * fy).rotated(r))
}

fn rect_pts(f: &impl Fn(f32, f32) -> V2, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<V2> {
    vec![f(x0, y0), f(x1, y0), f(x1, y1), f(x0, y1)]
}

fn door(s: &Shape, view: &View, th: &PaperTheme, out: &mut Vec<Cmd>) {
    let f = frame(s, view);
    let (w, t) = (s.size.x, s.size.y.max(2.0));
    let cut = t / 2.0 + 0.6;
    out.push(poly(rect_pts(&f, -w / 2.0, -cut, w / 2.0, cut), true, Some(th.paper), None));
    for x in [-w / 2.0, w / 2.0] {
        out.push(poly(vec![f(x, -t / 2.0), f(x, t / 2.0)], false, None, Some((th.ink, 1.5))));
    }
    // Leaf, open to 90 degrees, and its swing arc.
    let hinge = (-w / 2.0, 0.0);
    out.push(poly(vec![f(hinge.0, hinge.1), f(hinge.0, -w)], false, None, Some((th.ink, 1.8))));
    let arc: Vec<V2> = (0..=16)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 * (1.0 - i as f32 / 16.0);
            f(hinge.0 + w * a.cos(), w * a.sin())
        })
        .collect();
    out.push(poly(arc, false, None, Some((th.ink_soft, 1.0))));
}

fn sliding_door(s: &Shape, view: &View, th: &PaperTheme, out: &mut Vec<Cmd>) {
    let f = frame(s, view);
    let (w, t) = (s.size.x, s.size.y.max(2.0));
    let cut = t / 2.0 + 0.6;
    out.push(poly(rect_pts(&f, -w / 2.0, -cut, w / 2.0, cut), true, Some(th.paper), None));
    let stroke = Some((th.ink, 1.4));
    out.push(poly(
        rect_pts(&f, -w / 2.0, -t / 2.0, w * 0.08, 0.0),
        true,
        Some(th.glass),
        stroke,
    ));
    out.push(poly(
        rect_pts(&f, -w * 0.08, 0.0, w / 2.0, t / 2.0),
        true,
        Some(th.glass),
        stroke,
    ));
}

fn window(s: &Shape, view: &View, th: &PaperTheme, out: &mut Vec<Cmd>) {
    let f = frame(s, view);
    let (w, t) = (s.size.x, s.size.y.max(2.0));
    let cut = t / 2.0 + 0.6;
    out.push(poly(rect_pts(&f, -w / 2.0, -cut, w / 2.0, cut), true, Some(th.paper), None));
    out.push(poly(
        rect_pts(&f, -w / 2.0, -t / 2.0, w / 2.0, t / 2.0),
        true,
        Some(th.glass),
        Some((th.ink, 1.5)),
    ));
    for y in [-t / 6.0, t / 6.0] {
        out.push(poly(vec![f(-w / 2.0, y), f(w / 2.0, y)], false, None, Some((th.accent, 1.0))));
    }
}

fn furniture(s: &Shape, sym: Sym, view: &View, th: &PaperTheme, out: &mut Vec<Cmd>) {
    let f = frame(s, view);
    for p in symbols::prims(sym) {
        let pts: Vec<V2> = p.pts.iter().map(|q| f(q.x * s.size.x, q.y * s.size.y)).collect();
        let (color, width) = if p.soft { (th.ink_soft, 1.0) } else { (th.ink, 1.4) };
        out.push(Cmd::Poly {
            pts,
            closed: p.closed,
            fill: p.fill.then_some(th.furniture),
            stroke: Some((color, width)),
            dash: None,
        });
    }
    if !s.label.trim().is_empty() {
        let c = view.to_screen(s.center);
        out.push(Cmd::Text {
            at: c,
            text: s.label.clone(),
            size: 11.0,
            color: th.ink,
            weight: Weight::Medium,
            angle: 0.0,
            halo: Some(th.furniture),
            left: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Units;

    fn project() -> Project {
        let mut p = Project::create(&NewPlan {
            name: "T".into(),
            units: Units::Imperial,
            area: 1200.0,
            floors: 1,
            proportion: Proportion::Classic,
        });
        let mut r = Shape::boxed(Kind::Rect, v2(120.0, 96.0), v2(240.0, 192.0));
        r.label = "Living room".into();
        p.add_shape(0, r);
        p.add_shape(0, Shape::wall(v2(0.0, 0.0), v2(240.0, 0.0)));
        p.add_shape(0, Shape::opening(Kind::Door, v2(60.0, 0.0), 0.0, 36.0, 6.0));
        p.add_shape(0, Shape::opening(Kind::Window, v2(180.0, 0.0), 0.0, 48.0, 6.0));
        p.add_shape(0, Shape::opening(Kind::SlidingDoor, v2(120.0, 0.0), 0.0, 72.0, 6.0));
        p.add_shape(0, Shape::boxed(Kind::Furniture(Sym::Bed), v2(100.0, 100.0), Sym::Bed.default_size()));
        p
    }

    fn texts(cmds: &[Cmd]) -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                Cmd::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn view_round_trips_and_zooms_about_a_point() {
        let mut v = View { origin: v2(10.0, 20.0), scale: 2.0 };
        let w = v2(55.0, 66.0);
        let back = v.to_world(v.to_screen(w));
        assert!((back.x - w.x).abs() < 1e-3 && (back.y - w.y).abs() < 1e-3);
        let before = v.to_world(v2(100.0, 100.0));
        v.zoom_at(v2(100.0, 100.0), 1.5, 0.01, 50.0);
        let after = v.to_world(v2(100.0, 100.0));
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
        assert_eq!(v.scale, 3.0);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut v = View { origin: V2::ZERO, scale: 1.0 };
        v.zoom_at(V2::ZERO, 1000.0, 0.1, 8.0);
        assert_eq!(v.scale, 8.0);
        v.zoom_at(V2::ZERO, 0.0001, 0.1, 8.0);
        assert_eq!(v.scale, 0.1);
    }

    #[test]
    fn fit_puts_the_world_inside_the_viewport() {
        let v = View::fit((v2(0.0, 0.0), v2(400.0, 200.0)), v2(800.0, 600.0), 20.0);
        let a = v.to_screen(v2(0.0, 0.0));
        let b = v.to_screen(v2(400.0, 200.0));
        assert!(a.x >= 19.0 && a.y >= 19.0 && b.x <= 781.0 && b.y <= 581.0);
    }

    #[test]
    fn plan_emits_label_area_and_dimensions() {
        let p = project();
        let view = View { origin: v2(-20.0, -20.0), scale: 2.0 };
        let cmds = plan(&p, 0, &view, &PaperTheme::light(), &Opts::default());
        let t = texts(&cmds);
        assert!(t.iter().any(|s| s == "Living room"), "{t:?}");
        assert!(t.iter().any(|s| s.contains("320 sq ft")), "{t:?}");
        assert!(t.iter().any(|s| s == "20'"), "footprint width, {t:?}");
    }

    #[test]
    fn dimensions_can_be_turned_off() {
        let p = project();
        let view = View { origin: v2(-20.0, -20.0), scale: 2.0 };
        let on = texts(&plan(&p, 0, &view, &PaperTheme::light(), &Opts::default()));
        let off = texts(&plan(&p, 0, &view, &PaperTheme::light(), &Opts { dims: false, ..Opts::default() }));
        assert!(off.len() < on.len());
        assert!(!off.iter().any(|s| s.contains('×')));
    }

    #[test]
    fn tiny_rooms_skip_their_labels() {
        let p = project();
        let view = View { origin: V2::ZERO, scale: 0.05 };
        let t = texts(&plan(&p, 0, &view, &PaperTheme::light(), &Opts::default()));
        assert!(!t.iter().any(|s| s == "Living room"));
    }

    #[test]
    fn every_shape_kind_renders_without_panicking() {
        let mut p = project();
        p.add_shape(0, Shape::boxed(Kind::Ellipse, v2(300.0, 300.0), v2(80.0, 60.0)));
        p.add_shape(0, Shape::boxed(Kind::Triangle, v2(400.0, 300.0), v2(80.0, 60.0)));
        p.add_shape(0, Shape::polygon(vec![v2(0.0, 300.0), v2(60.0, 300.0), v2(60.0, 340.0), v2(30.0, 340.0), v2(30.0, 380.0), v2(0.0, 380.0)]));
        for sym in Sym::ALL {
            p.add_shape(0, Shape::boxed(Kind::Furniture(sym), v2(500.0, 500.0), sym.default_size()));
        }
        for scale in [0.02, 0.3, 1.0, 6.0] {
            let view = View { origin: V2::ZERO, scale };
            let cmds = plan(&p, 0, &view, &PaperTheme::dark(), &Opts::default());
            for c in &cmds {
                if let Cmd::Poly { pts, .. } = c {
                    assert!(pts.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
                }
            }
            assert!(cmds.len() > 20);
        }
    }

    #[test]
    fn grid_lines_stay_in_the_viewport_and_thin_out_when_zoomed_out() {
        let th = PaperTheme::light();
        let size = v2(800.0, 600.0);
        let near = grid(&View { origin: V2::ZERO, scale: 4.0 }, size, &th, Units::Imperial);
        let far = grid(&View { origin: V2::ZERO, scale: 0.1 }, size, &th, Units::Imperial);
        assert!(!near.is_empty());
        assert!(far.len() < 400, "too many grid lines when zoomed out: {}", far.len());
        let metric = grid(&View { origin: V2::ZERO, scale: 1.0 }, size, &th, Units::Metric);
        assert!(!metric.is_empty());
    }

    #[test]
    fn long_names_are_shortened() {
        assert_eq!(fit_text("Hi", 100.0, 13.0), "Hi");
        let t = fit_text("A very long room name indeed", 60.0, 13.0);
        assert!(t.ends_with('…') && t.chars().count() < 12, "{t}");
        assert_eq!(fit_text("abc", 1.0, 13.0), "…");
    }

    #[test]
    fn metric_plans_label_in_metric() {
        let mut p = project();
        p.units = Units::Metric;
        let view = View { origin: v2(-20.0, -20.0), scale: 2.0 };
        let t = texts(&plan(&p, 0, &view, &PaperTheme::light(), &Opts::default()));
        assert!(t.iter().any(|s| s.contains("m²")), "{t:?}");
    }
}
