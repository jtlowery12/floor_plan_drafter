//! The drawing surface: pointer and keyboard handling for every tool, plus the
//! editing overlays (selection, handles, previews, scale bar).

use crate::editor::*;
use crate::geom::*;
use crate::model::*;
use crate::paint;
use crate::render::{self, PaperTheme};
use crate::theme::Palette;
use eframe::egui::{
    self, Align2, Color32, Context, Key, Modifiers, Painter, PointerButton, Pos2, Rect, Sense,
    Shape as EShape, Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::f32::consts::{FRAC_PI_2, PI};

const DEFAULT_DROP: f32 = 120.0; // a click drops a 10 ft square room
const CLICK_SLOP: f32 = 4.0;

struct Inp {
    world: V2,
    shift: bool,
    alt: bool,
    pressed: bool,
    released: bool,
    down: bool,
    double: bool,
    moved: f32,
}

fn to_v(p: Pos2, origin: Pos2) -> V2 {
    v2(p.x - origin.x, p.y - origin.y)
}

fn to_p(v: V2, origin: Pos2) -> Pos2 {
    pos2(origin.x + v.x, origin.y + v.y)
}

pub fn paper_theme(ctx: &Context) -> PaperTheme {
    if Palette::of(ctx).dark { PaperTheme::dark() } else { PaperTheme::light() }
}

pub fn show(ed: &mut Editor, ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let th = paper_theme(&ctx);
    let pal = Palette::of(&ctx);
    if !ed.view_ready || ed.view.scale <= 0.0 {
        ed.fit(v2(rect.width(), rect.height()));
    }

    if !ctx.egui_wants_keyboard_input() {
        keys(ed, &ctx);
    }
    pointer(ed, ui, &resp, rect);

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, th.outside);
    // Footprint "paper".
    let fp: Vec<Pos2> = ed
        .project
        .footprint
        .rect()
        .iter()
        .map(|&p| to_p(ed.view.to_screen(p), rect.min))
        .collect();
    painter.add(EShape::convex_polygon(fp, th.paper, Stroke::NONE));
    if ed.opts.grid {
        let g = render::grid(&ed.view, v2(rect.width(), rect.height()), &th, ed.project.units);
        paint::paint(&painter, rect.min, &g);
    }
    let cmds = render::plan(&ed.project, ed.floor, &ed.view, &th, &ed.opts);
    paint::paint(&painter, rect.min, &cmds);
    overlays(ed, &painter, rect, &th, &pal);
    scale_bar(ed, &painter, rect, &th);
    ed.hint = hint(ed);
}

// ---- input ------------------------------------------------------------------

fn pointer(ed: &mut Editor, ui: &mut Ui, resp: &egui::Response, rect: Rect) {
    let ctx = ui.ctx().clone();
    let (latest, hover, mods, pressed, released, down, delta, zoom_d, scroll, mid, sec, space) =
        ui.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.hover_pos(),
                i.modifiers,
                i.pointer.button_pressed(PointerButton::Primary),
                i.pointer.button_released(PointerButton::Primary),
                i.pointer.button_down(PointerButton::Primary),
                i.pointer.delta(),
                i.zoom_delta(),
                i.smooth_scroll_delta.y,
                i.pointer.button_down(PointerButton::Middle),
                i.pointer.button_down(PointerButton::Secondary),
                i.key_down(Key::Space),
            )
        });
    let Some(latest) = latest else { return };
    let screen = to_v(latest, rect.min);
    let world = ed.view.to_world(screen);
    let inside = resp.hovered() && hover.is_some();

    // Zoom about the cursor.
    if inside {
        let factor = zoom_d * (scroll * 0.0018).exp();
        if (factor - 1.0).abs() > 1e-4 {
            ed.view.zoom_at(screen, factor, 0.01, 16.0);
        }
    }

    // Panning: middle or right button, space+drag, or the hand tool.
    let panning = (inside || matches!(ed.drag, Drag::Pan))
        && (mid || sec || (down && (space || ed.tool == Tool::Pan)));
    if panning {
        ed.view.origin = ed.view.origin - v2(delta.x, delta.y) * (1.0 / ed.view.scale);
        ed.drag = Drag::None;
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    if inside && !down {
        ed.cursor = Some(ed.snap_point(world, &[]));
    } else if !inside && !down {
        ed.cursor = None;
    }

    // Right-click finishes a drafting chain.
    if resp.secondary_clicked() {
        match ed.tool {
            Tool::Wall => ed.wall_start = None,
            Tool::Polygon => ed.poly.clear(),
            _ => {}
        }
    }

    if pressed && inside && !space {
        ed.press = Some(screen);
    }
    let moved = ed.press.map_or(0.0, |p| p.dist(screen));
    let inp = Inp {

        world,
        shift: mods.shift,
        alt: mods.alt,
        pressed: pressed && inside && !space,
        released,
        down,
        double: resp.double_clicked(),
        moved,
    };

    set_cursor_icon(ed, &ctx, inside, world);

    if inp.pressed {
        on_press(ed, &inp);
    }
    if inp.down && !matches!(ed.drag, Drag::None) {
        on_drag(ed, &inp);
    }
    if inp.released {
        on_release(ed, &inp, inside);
        ed.press = None;
    }
    if inp.double && ed.tool == Tool::Polygon {
        ed.finish_polygon();
    }
    if inp.double && ed.tool == Tool::Wall {
        ed.wall_start = None;
        ed.wall_fresh = false;
    }
    if inp.double && ed.tool == Tool::Select {
        if ed.pick(inp.world).is_some() {
            ed.focus_name = true;
        }
    }
}

fn set_cursor_icon(ed: &Editor, ctx: &Context, inside: bool, world: V2) {
    if !inside && matches!(ed.drag, Drag::None) {
        return;
    }
    use egui::CursorIcon as C;
    let icon = match ed.tool {
        Tool::Pan => C::Grab,
        Tool::Select => match &ed.drag {
            Drag::Move { .. } => C::Grabbing,
            Drag::None => match ed.single().and_then(|s| ed.handle_at(s, world)) {
                Some(Handle::Rotate) => C::Alias,
                Some(Handle::Vertex(_)) | Some(Handle::Mid(_)) => C::Crosshair,
                Some(Handle::Edge(hx, hy)) => match (hx, hy) {
                    (_, 0) => C::ResizeHorizontal,
                    (0, _) => C::ResizeVertical,
                    _ => C::ResizeNeSw,
                },
                None => {
                    if ed.pick(world).is_some() { C::PointingHand } else { C::Default }
                }
            },
            _ => C::Default,
        },
        _ => C::Crosshair,
    };
    ctx.set_cursor_icon(icon);
}

fn wall_target(ed: &Editor, start: V2, world: V2, shift: bool, exclude: &[u64]) -> V2 {
    let d = world - start;
    let lock = 8.0 / ed.view.scale;
    let (lx, ly) = (!shift && d.x.abs() < lock, !shift && d.y.abs() < lock);
    let mut t = start + v2(if lx { 0.0 } else { d.x }, if ly { 0.0 } else { d.y });
    t = ed.snap_point(t, exclude);
    if lx {
        t.x = start.x;
    }
    if ly {
        t.y = start.y;
    }
    t
}

fn on_press(ed: &mut Editor, inp: &Inp) {
    let snapped = ed.snap_point(inp.world, &[]);
    match ed.tool {
        Tool::Select => select_press(ed, inp),
        Tool::Pan => ed.drag = Drag::Pan,
        t if t.is_box_tool() => ed.drag = Drag::Create { start: snapped, cur: snapped },
        Tool::Wall => {
            if ed.wall_start.is_none() {
                ed.wall_start = Some(snapped);
                ed.wall_fresh = true;
            }
        }
        _ => {}
    }
}

fn select_press(ed: &mut Editor, inp: &Inp) {
    if !inp.shift {
        if let Some(s) = ed.single().cloned() {
            if let Some(h) = ed.handle_at(&s, inp.world) {
                match h {
                    Handle::Edge(hx, hy) => ed.drag = Drag::Resize { orig: s, hx, hy, changed: false },
                    Handle::Rotate => {
                        let pivot = s.center;
                        let start = (inp.world - pivot).angle();
                        ed.drag = Drag::Rotate { orig: s, pivot, start, changed: false };
                    }
                    Handle::Vertex(i) => {
                        let linked = ed.linked_wall_ends(&s, i);
                        ed.drag = Drag::Vertex { orig: s, idx: i, linked, changed: false };
                    }
                    Handle::Mid(i) => {
                        let mid = s.points[i].lerp(s.points[(i + 1) % s.points.len()], 0.5);
                        ed.checkpoint();
                        if let Some(idx) = ed.insert_vertex(s.id, i, mid) {
                            let orig = ed.floor().find(s.id).cloned().unwrap_or(s);
                            ed.drag = Drag::Vertex { orig, idx, linked: Vec::new(), changed: true };
                        }
                    }
                }
                return;
            }
        }
    }
    match ed.pick(inp.world) {
        Some(id) => {
            if inp.shift {
                if let Some(pos) = ed.selection.iter().position(|&x| x == id) {
                    ed.selection.remove(pos);
                } else {
                    ed.selection.push(id);
                }
                return;
            }
            if !ed.is_selected(id) {
                ed.select_only(id);
            }
            let origins: Vec<Shape> = ed.selected().into_iter().filter(|s| !s.locked).cloned().collect();
            if origins.is_empty() {
                return;
            }
            let pts: Vec<V2> = origins.iter().flat_map(|s| s.outline()).collect();
            let anchor = bounds(&pts).map_or(inp.world, |(lo, _)| lo);
            ed.drag = Drag::Move { origins, grab: inp.world, anchor, changed: false };
        }
        None => {
            if !inp.shift {
                ed.selection.clear();
            }
            ed.drag = Drag::Marquee { start: inp.world, cur: inp.world };
        }
    }
}

fn on_drag(ed: &mut Editor, inp: &Inp) {
    let drag = ed.drag.clone();
    match drag {
        Drag::Marquee { start, .. } => ed.drag = Drag::Marquee { start, cur: inp.world },
        Drag::Create { start, .. } => {
            let mut cur = ed.snap_point(inp.world, &[]);
            if inp.shift {
                let d = cur - start;
                let m = d.x.abs().max(d.y.abs());
                cur = start + v2(m * if d.x < 0.0 { -1.0 } else { 1.0 }, m * if d.y < 0.0 { -1.0 } else { 1.0 });
            }
            ed.drag = Drag::Create { start, cur };
        }
        Drag::Move { origins, grab, anchor, changed } => {
            let raw = inp.world - grab;
            let mut delta = if raw.len() * ed.view.scale < 1.0 && !changed {
                V2::ZERO
            } else {
                let exclude: Vec<u64> = origins.iter().map(|s| s.id).collect();
                ed.snap_point(anchor + raw, &exclude) - anchor
            };
            // A lone door or window slides along walls instead of floating free.
            let mut opening_pose: Option<(V2, f32, f32, bool)> = None;
            if let [o] = origins.as_slice() {
                if o.kind.is_opening() {
                    opening_pose = ed.opening_placement(inp.world, o.size.x);
                    if opening_pose.is_none() {
                        delta = V2::ZERO;
                    }
                }
            }
            let moved = delta.len() > 0.01 || opening_pose.is_some_and(|(c, ..)| c.dist(origins[0].center) > 0.01);
            if moved && !changed {
                ed.checkpoint();
            }
            for o in &origins {
                if let Some(s) = ed.floor_mut().find_mut(o.id) {
                    *s = o.clone();
                    if let Some((c, ang, depth, flip)) = opening_pose {
                        s.center = c;
                        s.rot = ang;
                        s.size.y = depth;
                        s.flip_y = flip;
                    } else {
                        s.translate(delta);
                    }
                }
            }
            ed.drag = Drag::Move { origins, grab, anchor, changed: changed || moved };
        }
        Drag::Resize { orig, hx, hy, changed } => {
            let current = ed.floor().find(orig.id).map(|s| (s.center, s.size));
            let would = ed_clone_apply(ed, &orig, hx, hy, inp.world);
            let differs = would != current;
            if differs && !changed {
                ed.checkpoint();
            }
            if differs {
                ed.apply_resize(&orig, hx, hy, inp.world);
            }
            ed.drag = Drag::Resize { orig, hx, hy, changed: changed || differs };
        }
        Drag::Vertex { orig, idx, linked, changed } => {
            let mut ex: Vec<u64> = linked.iter().map(|l| l.0).collect();
            ex.push(orig.id);
            let target = if orig.kind == Kind::Wall && orig.points.len() == 2 {
                wall_target(ed, orig.points[1 - idx], inp.world, inp.shift, &ex)
            } else {
                ed.snap_point(inp.world, &ex)
            };
            let will_change = target != orig.points[idx];
            if will_change && !changed {
                ed.checkpoint();
            }
            ed.apply_vertex(&orig, idx, &linked, target);
            ed.drag = Drag::Vertex { orig, idx, linked, changed: changed || will_change };
        }
        Drag::Rotate { orig, pivot, start, changed } => {
            let raw = (inp.world - pivot).angle() - start;
            let delta = if inp.alt { raw } else { snap(raw, PI / 12.0) };
            if delta.abs() > 1e-4 && !changed {
                ed.checkpoint();
            }
            ed.apply_rotation(&orig, pivot, delta);
            ed.drag = Drag::Rotate { orig, pivot, start, changed: changed || delta.abs() > 1e-4 };
        }
        Drag::None | Drag::Pan => {}
    }
}

/// What a resize would produce, without recording history. Used to decide
/// whether the first drag frame actually changes anything.
fn ed_clone_apply(ed: &mut Editor, orig: &Shape, hx: i8, hy: i8, world: V2) -> Option<(V2, V2)> {
    let saved = ed.floor().find(orig.id).cloned()?;
    ed.apply_resize(orig, hx, hy, world);
    let after = ed.floor().find(orig.id).map(|s| (s.center, s.size));
    if let Some(s) = ed.floor_mut().find_mut(orig.id) {
        *s = saved;
    }
    after
}

fn on_release(ed: &mut Editor, inp: &Inp, inside: bool) {
    let click = inp.moved < CLICK_SLOP;
    let drag = std::mem::replace(&mut ed.drag, Drag::None);
    match drag {
        Drag::Marquee { start, cur } => {
            let (lo, hi) = (v2(start.x.min(cur.x), start.y.min(cur.y)), v2(start.x.max(cur.x), start.y.max(cur.y)));
            if hi.x - lo.x > 2.0 / ed.view.scale || hi.y - lo.y > 2.0 / ed.view.scale {
                let hits: Vec<u64> = ed
                    .floor()
                    .shapes
                    .iter()
                    .filter(|s| {
                        s.bounds()
                            .is_some_and(|(a, b)| a.x <= hi.x && b.x >= lo.x && a.y <= hi.y && b.y >= lo.y)
                    })
                    .map(|s| s.id)
                    .collect();
                for id in hits {
                    if !ed.selection.contains(&id) {
                        ed.selection.push(id);
                    }
                }
            }
        }
        Drag::Create { start, cur } => {
            let kind = ed.tool.kind().unwrap_or(Kind::Rect);
            let shape = if click {
                ed.box_from_corners(kind, start, start + v2(DEFAULT_DROP, DEFAULT_DROP))
            } else {
                ed.box_from_corners(kind, start, cur)
            };
            if let Some(s) = shape {
                let id = ed.add(s);
                ed.set_tool(Tool::Select);
                ed.select_only(id);
            }
        }
        Drag::Pan => {}
        _ => {}
    }
    if !inside && !inp.down {
        // Released outside the canvas: only drags (handled above) complete.
    }

    match ed.tool {
        Tool::Wall => wall_release(ed, inp, click),
        Tool::Polygon if click && inside => polygon_click(ed, inp),
        t if t.is_opening_tool() && click && inside => place_opening(ed, inp, t),
        Tool::Furniture if click && inside => place_furniture(ed, inp),
        _ => {}
    }
}

fn wall_release(ed: &mut Editor, inp: &Inp, click: bool) {
    let Some(start) = ed.wall_start else { return };
    if ed.wall_fresh {
        ed.wall_fresh = false;
        if click {
            return; // first click of a click-click chain
        }
    }
    let end = wall_target(ed, start, inp.world, inp.shift, &[]);
    if end.dist(start) < ed.project.snap.max(2.0) {
        return;
    }
    ed.add(Shape::wall(start, end));
    ed.wall_start = Some(end);
}

fn polygon_click(ed: &mut Editor, inp: &Inp) {
    let p = ed.snap_point(inp.world, &[]);
    if ed.poly.len() >= 3 && ed.view.to_screen(ed.poly[0]).dist(ed.view.to_screen(p)) < 12.0 {
        ed.finish_polygon();
        return;
    }
    if ed.poly.last().is_none_or(|l| l.dist(p) > 0.5) {
        ed.poly.push(p);
    }
}

fn place_opening(ed: &mut Editor, inp: &Inp, tool: Tool) {
    let width = Editor::opening_width(tool);
    let Some((center, ang, depth, flip_y)) = ed.opening_placement(inp.world, width) else {
        return;
    };
    let kind = tool.kind().unwrap_or(Kind::Door);
    let mut s = Shape::opening(kind, center, ang, width, depth);
    s.flip_y = flip_y;
    s.flip_x = ed.ghost_flip;
    ed.add(s);
}

fn place_furniture(ed: &mut Editor, inp: &Inp) {
    let c = ed.snap_point(inp.world, &[]);
    let mut s = Shape::boxed(Kind::Furniture(ed.sym), c, ed.sym.default_size());
    s.rot = ed.ghost_rot;
    s.flip_x = ed.ghost_flip;
    let id = ed.add(s);
    ed.set_tool(Tool::Select);
    ed.select_only(id);
}

// ---- keyboard -----------------------------------------------------------------

fn keys(ed: &mut Editor, ctx: &Context) {
    let take = |ctx: &Context, mods: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(mods, key));
    let cmd = Modifiers::COMMAND;
    let shift_cmd = Modifiers::COMMAND | Modifiers::SHIFT;

    if take(ctx, cmd, Key::Z) {
        ed.undo();
    }
    if take(ctx, shift_cmd, Key::Z) || take(ctx, cmd, Key::Y) {
        ed.redo();
    }
    if take(ctx, cmd, Key::D) {
        ed.duplicate_selection();
    }
    if take(ctx, cmd, Key::A) {
        ed.set_tool(Tool::Select);
        ed.select_all();
    }
    if take(ctx, cmd, Key::R) {
        if ed.tool == Tool::Furniture {
            ed.ghost_rot = wrap_angle(ed.ghost_rot + FRAC_PI_2);
        } else {
            ed.rotate_selection(FRAC_PI_2);
        }
    }
    if take(ctx, cmd, Key::Num0) {
        ed.view_ready = false;
    }
    if take(ctx, Modifiers::NONE, Key::Escape) {
        if !ed.poly.is_empty() || ed.wall_start.is_some() {
            ed.cancel_drafts();
        } else if ed.tool != Tool::Select {
            ed.set_tool(Tool::Select);
        } else {
            ed.selection.clear();
        }
    }
    if take(ctx, Modifiers::NONE, Key::Enter) {
        match ed.tool {
            Tool::Polygon => {
                ed.finish_polygon();
            }
            Tool::Wall => {
                ed.wall_start = None;
            }
            _ => {}
        }
    }
    if take(ctx, Modifiers::NONE, Key::Backspace) || take(ctx, Modifiers::NONE, Key::Delete) {
        if ed.tool == Tool::Polygon && !ed.poly.is_empty() {
            ed.poly.pop();
        } else {
            ed.delete_selection();
        }
    }
    let step = if ed.snap_on { ed.project.snap } else { 1.0 };
    for (key, d) in [
        (Key::ArrowLeft, v2(-step, 0.0)),
        (Key::ArrowRight, v2(step, 0.0)),
        (Key::ArrowUp, v2(0.0, -step)),
        (Key::ArrowDown, v2(0.0, step)),
    ] {
        if take(ctx, Modifiers::NONE, key) {
            ed.nudge(d);
        }
        if take(ctx, Modifiers::SHIFT, key) {
            ed.nudge(d * (12.0 / step.max(1.0)));
        }
    }
    for (key, tool) in [
        (Key::V, Tool::Select),
        (Key::H, Tool::Pan),
        (Key::R, Tool::Rect),
        (Key::E, Tool::Ellipse),
        (Key::T, Tool::Triangle),
        (Key::P, Tool::Polygon),
        (Key::W, Tool::Wall),
        (Key::D, Tool::Door),
        (Key::S, Tool::SlidingDoor),
        (Key::N, Tool::Window),
        (Key::F, Tool::Furniture),
    ] {
        if take(ctx, Modifiers::NONE, key) {
            ed.set_tool(tool);
        }
    }
    if take(ctx, Modifiers::NONE, Key::X) {
        ed.ghost_flip = !ed.ghost_flip;
    }
    if take(ctx, Modifiers::NONE, Key::G) {
        ed.opts.grid = !ed.opts.grid;
    }
}

fn hint(ed: &Editor) -> String {
    let u = ed.project.units;
    match ed.tool {
        Tool::Select => match ed.selection.len() {
            0 => "Click a shape to select it, or drag to select several. Scroll to zoom, right-drag to pan.".into(),
            1 => "Drag to move. Drag the handles to resize or rotate. Press Delete to remove.".into(),
            n => format!("{n} selected. Drag to move together, Ctrl+D to duplicate, Delete to remove."),
        },
        Tool::Pan => "Drag to pan the view.".into(),
        Tool::Rect | Tool::Ellipse | Tool::Triangle => format!(
            "Drag to draw. A single click drops a {0} × {0} room. Hold Shift for equal sides.",
            u.len(DEFAULT_DROP)
        ),
        Tool::Polygon => "Click each corner. Click the first point, double-click or press Enter to finish. Backspace undoes a point.".into(),
        Tool::Wall => "Click to start a wall, click again to end it and keep going. Esc, Enter or right-click stops. Hold Shift for any angle.".into(),
        Tool::Door | Tool::SlidingDoor | Tool::Window => "Move onto a wall and click to place. X flips the hinge side. Esc when done.".into(),
        Tool::Furniture => format!("Click to place the {}. Ctrl+R rotates, X mirrors, Esc cancels.", ed.sym.name().to_lowercase()),
    }
}

// ---- overlays -------------------------------------------------------------------

fn overlays(ed: &Editor, painter: &Painter, rect: Rect, th: &PaperTheme, pal: &Palette) {
    let o = rect.min;
    let accent = th.accent;
    let sp = |w: V2| to_p(ed.view.to_screen(w), o);
    let outline_stroke = Stroke::new(2.0, accent);

    // Hover highlight.
    if ed.tool == Tool::Select && matches!(ed.drag, Drag::None) {
        if let Some(c) = ed.cursor {
            if let Some(id) = ed.pick(c) {
                if !ed.is_selected(id) {
                    if let Some(s) = ed.floor().find(id) {
                        stroke_outline(painter, &s.outline().iter().map(|&p| sp(p)).collect::<Vec<_>>(), Stroke::new(1.5, accent.gamma_multiply(0.55)));
                    }
                }
            }
        }
    }

    // Selection.
    for s in ed.selected() {
        let pts: Vec<Pos2> = s.outline().iter().map(|&p| sp(p)).collect();
        stroke_outline(painter, &pts, outline_stroke);
    }
    if let Some(s) = ed.single() {
        let handle_color = pal.surface;
        for (h, w) in ed.handles(s) {
            let p = sp(w);
            match h {
                Handle::Rotate => {
                    let top = sp(s.center + v2(0.0, -s.size.y / 2.0).rotated(s.rot));
                    painter.line_segment([top, p], Stroke::new(1.2, accent));
                    painter.circle(p, 6.0, handle_color, Stroke::new(2.0, accent));
                    painter.text(p, Align2::CENTER_CENTER, crate::icons::ROTATE, crate::theme::icons(11.0), accent);
                }
                Handle::Mid(_) => {
                    painter.circle(p, 3.5, handle_color, Stroke::new(1.2, accent.gamma_multiply(0.7)));
                }
                _ => {
                    let r = Rect::from_center_size(p, vec2(9.0, 9.0));
                    painter.rect(r, 2.0, handle_color, Stroke::new(1.8, accent), StrokeKind::Inside);
                }
            }
        }
        if s.locked {
            let c = sp(s.centroid());
            painter.text(c, Align2::CENTER_CENTER, crate::icons::LOCK, crate::theme::icons(16.0), accent);
        }
    }

    // Tool previews.
    match &ed.drag {
        Drag::Marquee { start, cur } => {
            let r = Rect::from_two_pos(sp(*start), sp(*cur));
            painter.rect(r, 0.0, accent.gamma_multiply(0.10), Stroke::new(1.2, accent), StrokeKind::Inside);
        }
        Drag::Create { start, cur } => {
            if let Some(kind) = ed.tool.kind() {
                if let Some(mut s) = ed.box_from_corners(kind, *start, *cur) {
                    s.color = ed.next_color();
                    ghost_shape(ed, painter, o, th, &s);
                    let label = format!("{} × {}", ed.project.units.len(s.size.x), ed.project.units.len(s.size.y));
                    badge(painter, sp(s.center), &label, accent, pal);
                }
            }
        }
        _ => {}
    }

    if let Some(c) = ed.cursor {
        match ed.tool {
            Tool::Rect | Tool::Ellipse | Tool::Triangle | Tool::Polygon | Tool::Wall
                if matches!(ed.drag, Drag::None) =>
            {
                snap_marker(painter, sp(c), accent);
            }
            _ => {}
        }
    }

    // Polygon in progress.
    if ed.tool == Tool::Polygon && !ed.poly.is_empty() {
        let mut pts: Vec<Pos2> = ed.poly.iter().map(|&p| sp(p)).collect();
        if let Some(c) = ed.cursor {
            pts.push(sp(c));
        }
        painter.add(EShape::line(pts.clone(), Stroke::new(2.0, accent)));
        for (i, p) in ed.poly.iter().enumerate() {
            let r = if i == 0 { 6.0 } else { 3.5 };
            painter.circle(sp(*p), r, pal.surface, Stroke::new(2.0, accent));
        }
        if let (Some(last), Some(c)) = (ed.poly.last(), ed.cursor) {
            badge(painter, sp((*last).lerp(c, 0.5)), &ed.project.units.len(last.dist(c)), accent, pal);
        }
    }

    // Wall in progress.
    if ed.tool == Tool::Wall {
        if let (Some(start), Some(cur)) = (ed.wall_start, ed.cursor) {
            let end = wall_target(ed, start, cur, false, &[]);
            let mut w = Shape::wall(start, end);
            w.thickness = DEFAULT_WALL;
            let pts: Vec<Pos2> = w.outline().iter().map(|&p| sp(p)).collect();
            painter.add(EShape::convex_polygon(pts, accent.gamma_multiply(0.45), Stroke::new(1.5, accent)));
            painter.circle(sp(start), 4.5, pal.surface, Stroke::new(2.0, accent));
            if start.dist(end) > 1.0 {
                badge(painter, sp(start.lerp(end, 0.5)), &ed.project.units.len(start.dist(end)), accent, pal);
            }
        }
    }

    // Ghost for placement tools.
    if let Some(c) = ed.cursor {
        if ed.tool.is_opening_tool() {
            let width = Editor::opening_width(ed.tool);
            if let Some((center, ang, depth, flip_y)) = ed.opening_placement(c, width) {
                let mut s = Shape::opening(ed.tool.kind().unwrap_or(Kind::Door), center, ang, width, depth);
                s.flip_y = flip_y;
                s.flip_x = ed.ghost_flip;
                ghost_shape(ed, painter, o, th, &s);
            }
        } else if ed.tool == Tool::Furniture {
            let mut s = Shape::boxed(Kind::Furniture(ed.sym), c, ed.sym.default_size());
            s.rot = ed.ghost_rot;
            s.flip_x = ed.ghost_flip;
            ghost_shape(ed, painter, o, th, &s);
        }
    }
}

fn ghost_shape(ed: &Editor, painter: &Painter, origin: Pos2, th: &PaperTheme, s: &Shape) {
    let cmds = render::shape_cmds(s, &ed.view, th, &render::Opts { dims: false, labels: false, ..ed.opts }, ed.project.units);
    paint::paint(painter, origin, &cmds);
    let pts: Vec<Pos2> = s.outline().iter().map(|&p| to_p(ed.view.to_screen(p), origin)).collect();
    let mut path = pts.clone();
    if let Some(f) = pts.first() {
        path.push(*f);
    }
    painter.extend(EShape::dashed_line(&path, Stroke::new(1.5, th.accent), 6.0, 4.0));
}

fn stroke_outline(painter: &Painter, pts: &[Pos2], stroke: Stroke) {
    if pts.len() < 2 {
        return;
    }
    let mut path = pts.to_vec();
    path.push(pts[0]);
    painter.add(EShape::line(path, stroke));
}

fn snap_marker(painter: &Painter, p: Pos2, color: Color32) {
    painter.circle_stroke(p, 6.0, Stroke::new(1.6, color));
    painter.circle_filled(p, 1.8, color);
}

fn badge(painter: &Painter, at: Pos2, text: &str, accent: Color32, pal: &Palette) {
    let galley = painter.layout_no_wrap(text.to_owned(), crate::theme::semibold(12.0), pal.on_accent);
    let r = Rect::from_center_size(at, galley.size() + vec2(14.0, 8.0));
    painter.rect_filled(r, 8.0, accent);
    painter.galley(r.center() - galley.size() / 2.0, galley, pal.on_accent);
}

/// A scale bar in the lower-left corner, sized to a round number of feet or metres.
fn scale_bar(ed: &Editor, painter: &Painter, rect: Rect, th: &PaperTheme) {
    let units = ed.project.units;
    let candidates: &[f32] = match units {
        crate::units::Units::Imperial => &[12.0, 24.0, 60.0, 120.0, 240.0, 600.0, 1200.0, 2400.0],
        crate::units::Units::Metric => {
            &[39.37, 98.43, 196.85, 393.7, 984.3, 1968.5, 3937.0, 9843.0]
        }
    };
    let target = 110.0 / ed.view.scale;
    let len = candidates
        .iter()
        .copied()
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
        .unwrap_or(120.0);
    let px = len * ed.view.scale;
    let base = pos2(rect.left() + 22.0, rect.bottom() - 26.0);
    let col = th.ink_soft;
    let st = Stroke::new(2.0, col);
    painter.rect_filled(
        Rect::from_min_max(base - vec2(10.0, 24.0), base + vec2(px + 14.0, 14.0)),
        8.0,
        th.outside.gamma_multiply(0.85),
    );
    painter.line_segment([base, base + vec2(px, 0.0)], st);
    for x in [0.0, px] {
        painter.line_segment([base + vec2(x, -5.0), base + vec2(x, 5.0)], st);
    }
    painter.text(
        base + vec2(px / 2.0, -9.0),
        Align2::CENTER_CENTER,
        units.len(len),
        crate::theme::medium(11.5),
        col,
    );
}
