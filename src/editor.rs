//! The editing session for one plan: selection, tools, undo history and all
//! the operations that change the drawing. No UI code lives here, which keeps
//! these rules easy to test.

use crate::geom::*;
use crate::model::*;
use crate::render::{Opts, View};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Pan,
    Rect,
    Ellipse,
    Triangle,
    Polygon,
    Wall,
    Door,
    SlidingDoor,
    Window,
    Furniture,
}

impl Tool {
    pub fn kind(self) -> Option<Kind> {
        match self {
            Tool::Rect => Some(Kind::Rect),
            Tool::Ellipse => Some(Kind::Ellipse),
            Tool::Triangle => Some(Kind::Triangle),
            Tool::Door => Some(Kind::Door),
            Tool::SlidingDoor => Some(Kind::SlidingDoor),
            Tool::Window => Some(Kind::Window),
            _ => None,
        }
    }
    pub fn is_box_tool(self) -> bool {
        matches!(self, Tool::Rect | Tool::Ellipse | Tool::Triangle)
    }
    pub fn is_opening_tool(self) -> bool {
        matches!(self, Tool::Door | Tool::SlidingDoor | Tool::Window)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Handle {
    /// Box resize: which edges move (-1, 0 or 1 on each axis).
    Edge(i8, i8),
    Rotate,
    Vertex(usize),
    /// Midpoint of a polygon edge; dragging it inserts a new corner.
    Mid(usize),
}

#[derive(Clone, Debug)]
pub enum Drag {
    None,
    Pan,
    Marquee { start: V2, cur: V2 },
    Move { origins: Vec<Shape>, grab: V2, anchor: V2, changed: bool },
    Resize { orig: Shape, hx: i8, hy: i8, changed: bool },
    Vertex { orig: Shape, idx: usize, linked: Vec<(u64, usize, Shape)>, changed: bool },
    Rotate { orig: Shape, pivot: V2, start: f32, changed: bool },
    Create { start: V2, cur: V2 },
}

const UNDO_LIMIT: usize = 100;

pub struct Editor {
    pub project: Project,
    pub floor: usize,
    pub tool: Tool,
    pub sym: Sym,
    pub ghost_rot: f32,
    pub ghost_flip: bool,
    pub selection: Vec<u64>,
    pub view: View,
    pub view_ready: bool,
    pub opts: Opts,
    pub snap_on: bool,
    pub undo: Vec<Project>,
    pub redo: Vec<Project>,
    coalesce: Option<(String, Instant)>,
    pub dirty: bool,
    pub last_edit: Instant,
    pub drag: Drag,
    pub poly: Vec<V2>,
    pub wall_start: Option<V2>,
    pub wall_fresh: bool,
    pub press: Option<V2>,
    pub cursor: Option<V2>,
    pub hint: String,
    pub show_settings: bool,
    pub show_export: bool,
    pub focus_name: bool,
}

impl Editor {
    pub fn new(project: Project) -> Editor {
        Editor {
            project,
            floor: 0,
            tool: Tool::Select,
            sym: Sym::Bed,
            ghost_rot: 0.0,
            ghost_flip: false,
            selection: Vec::new(),
            view: View { origin: V2::ZERO, scale: 1.0 },
            view_ready: false,
            opts: Opts::default(),
            snap_on: true,
            undo: Vec::new(),
            redo: Vec::new(),
            coalesce: None,
            dirty: false,
            last_edit: Instant::now(),
            drag: Drag::None,
            poly: Vec::new(),
            wall_start: None,
            wall_fresh: false,
            press: None,
            cursor: None,
            hint: String::new(),
            show_settings: false,
            show_export: false,
            focus_name: false,
        }
    }

    pub fn floor(&self) -> &Floor {
        &self.project.floors[self.floor]
    }

    pub fn floor_mut(&mut self) -> &mut Floor {
        &mut self.project.floors[self.floor]
    }

    pub fn snap_step(&self) -> f32 {
        if self.snap_on { self.project.snap } else { 0.0 }
    }

    // ---- history ----------------------------------------------------------

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_edit = Instant::now();
    }

    /// Record the current state so the next change can be undone.
    pub fn checkpoint(&mut self) {
        self.coalesce = None;
        self.undo.push(self.project.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.mark_dirty();
    }

    /// Like `checkpoint`, but repeated edits with the same key within a couple
    /// of seconds (typing in a field) share one undo step.
    pub fn checkpoint_coalesced(&mut self, key: &str) {
        let fresh = match &self.coalesce {
            Some((k, t)) => k != key || t.elapsed() > Duration::from_secs(2),
            None => true,
        };
        if fresh {
            self.checkpoint();
        } else {
            self.mark_dirty();
        }
        self.coalesce = Some((key.to_string(), Instant::now()));
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, prev));
            self.after_history_jump();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, next));
            self.after_history_jump();
        }
    }

    fn after_history_jump(&mut self) {
        self.coalesce = None;
        self.drag = Drag::None;
        self.floor = self.floor.min(self.project.floors.len() - 1);
        let ids: Vec<u64> = self.floor().shapes.iter().map(|s| s.id).collect();
        self.selection.retain(|id| ids.contains(id));
        self.mark_dirty();
    }

    // ---- tools ------------------------------------------------------------

    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.cancel_drafts();
        }
        self.tool = tool;
        if tool != Tool::Select {
            // Drawing tools start from a clean slate.
            self.selection.clear();
        }
        self.ghost_rot = 0.0;
        self.ghost_flip = false;
    }

    pub fn cancel_drafts(&mut self) {
        self.poly.clear();
        self.wall_start = None;
        self.wall_fresh = false;
        self.drag = Drag::None;
    }

    // ---- snapping and picking ---------------------------------------------

    /// Snap a world point: first to a nearby corner of the drawing, otherwise
    /// to the grid.
    pub fn snap_point(&self, raw: V2, exclude: &[u64]) -> V2 {
        if !self.snap_on {
            return raw;
        }
        let tol = 10.0 / self.view.scale;
        let mut best: Option<(f32, V2)> = None;
        let mut consider = |p: V2| {
            let d = p.dist(raw);
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
        };
        for p in self.project.footprint.rect() {
            consider(p);
        }
        for s in &self.floor().shapes {
            if exclude.contains(&s.id) {
                continue;
            }
            match s.kind {
                Kind::Wall | Kind::Polygon => s.points.iter().for_each(|&p| consider(p)),
                Kind::Rect | Kind::Triangle => s.outline().into_iter().for_each(&mut consider),
                _ => {}
            }
        }
        best.map_or_else(|| snap_v(raw, self.project.snap), |(_, p)| p)
    }

    pub fn pick_tolerance(&self) -> f32 {
        6.0 / self.view.scale
    }

    pub fn pick(&self, world: V2) -> Option<u64> {
        self.floor().pick(world, self.pick_tolerance())
    }

    // ---- selection ---------------------------------------------------------

    pub fn is_selected(&self, id: u64) -> bool {
        self.selection.contains(&id)
    }

    pub fn select_only(&mut self, id: u64) {
        self.selection = vec![id];
    }

    pub fn select_all(&mut self) {
        self.selection = self.floor().shapes.iter().map(|s| s.id).collect();
    }

    pub fn selected(&self) -> Vec<&Shape> {
        self.selection.iter().filter_map(|&id| self.floor().find(id)).collect()
    }

    pub fn single(&self) -> Option<&Shape> {
        match self.selection.as_slice() {
            [id] => self.floor().find(*id),
            _ => None,
        }
    }

    // ---- operations ---------------------------------------------------------

    pub fn delete_selection(&mut self) {
        let ids: Vec<u64> = self
            .selection
            .iter()
            .copied()
            .filter(|&id| self.floor().find(id).is_some_and(|s| !s.locked))
            .collect();
        if ids.is_empty() {
            return;
        }
        self.checkpoint();
        self.floor_mut().shapes.retain(|s| !ids.contains(&s.id));
        self.selection.clear();
    }

    pub fn duplicate_selection(&mut self) {
        let originals: Vec<Shape> = self.selected().into_iter().cloned().collect();
        if originals.is_empty() {
            return;
        }
        self.checkpoint();
        let off = v2(24.0, 24.0);
        let floor = self.floor;
        let mut ids = Vec::new();
        for mut s in originals {
            s.translate(off);
            s.locked = false;
            ids.push(self.project.add_shape(floor, s));
        }
        self.selection = ids;
    }

    pub fn rotate_selection(&mut self, delta: f32) {
        let ids = self.selection.clone();
        let shapes: Vec<&Shape> = self.selected();
        if shapes.is_empty() {
            return;
        }
        let pts: Vec<V2> = shapes.iter().flat_map(|s| s.outline()).collect();
        let pivot = bounds(&pts).map_or(V2::ZERO, |(lo, hi)| (lo + hi) * 0.5);
        self.checkpoint();
        for id in ids {
            if let Some(s) = self.floor_mut().find_mut(id) {
                if !s.locked {
                    s.rotate_about(pivot, delta);
                }
            }
        }
    }

    pub fn nudge(&mut self, d: V2) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        self.checkpoint_coalesced("nudge");
        for id in ids {
            if let Some(s) = self.floor_mut().find_mut(id) {
                if !s.locked {
                    s.translate(d);
                }
            }
        }
    }

    pub fn flip_selection(&mut self, x: bool) {
        let ids = self.selection.clone();
        self.checkpoint();
        for id in ids {
            if let Some(s) = self.floor_mut().find_mut(id) {
                if x { s.flip_x = !s.flip_x } else { s.flip_y = !s.flip_y }
            }
        }
    }

    pub fn set_color(&mut self, color: u8) {
        let ids = self.selection.clone();
        self.checkpoint_coalesced("color");
        for id in ids {
            if let Some(s) = self.floor_mut().find_mut(id) {
                if s.kind.is_room() {
                    s.color = color;
                }
            }
        }
    }

    pub fn set_label(&mut self, id: u64, label: String) {
        self.checkpoint_coalesced(&format!("label{id}"));
        if let Some(s) = self.floor_mut().find_mut(id) {
            s.label = label;
        }
    }

    pub fn set_locked(&mut self, locked: bool) {
        let ids = self.selection.clone();
        self.checkpoint();
        for id in ids {
            if let Some(s) = self.floor_mut().find_mut(id) {
                s.locked = locked;
            }
        }
    }

    /// Set the size of a box shape, keeping its center.
    pub fn set_size(&mut self, id: u64, size: V2) {
        self.checkpoint_coalesced(&format!("size{id}"));
        if let Some(s) = self.floor_mut().find_mut(id) {
            if s.kind.is_boxed() {
                s.size = size;
            }
        }
    }

    pub fn set_rotation(&mut self, id: u64, degrees: f32) {
        self.checkpoint_coalesced(&format!("rot{id}"));
        if let Some(s) = self.floor_mut().find_mut(id) {
            s.rot = wrap_angle(degrees.to_radians());
        }
    }

    pub fn set_wall_length(&mut self, id: u64, len: f32) {
        self.checkpoint_coalesced(&format!("len{id}"));
        if let Some(s) = self.floor_mut().find_mut(id) {
            if s.kind == Kind::Wall && s.points.len() >= 2 {
                let dir = (s.points[1] - s.points[0]).normalized();
                s.points[1] = s.points[0] + dir * len;
            }
        }
    }

    pub fn set_thickness(&mut self, id: u64, t: f32) {
        self.checkpoint_coalesced(&format!("thick{id}"));
        if let Some(s) = self.floor_mut().find_mut(id) {
            if s.kind == Kind::Wall {
                s.thickness = t.clamp(1.0, 48.0);
            }
        }
    }

    pub fn add(&mut self, shape: Shape) -> u64 {
        self.checkpoint();
        let floor = self.floor;
        self.project.add_shape(floor, shape)
    }

    /// Draw walls along the edges of the selected room.
    pub fn walls_around_selection(&mut self) {
        let Some(room) = self.single().filter(|s| s.kind.is_room()).cloned() else { return };
        let outline = if room.kind == Kind::Ellipse { Vec::new() } else { room.outline() };
        if outline.len() < 3 {
            return;
        }
        self.checkpoint();
        let floor = self.floor;
        let mut ids = Vec::new();
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            if a.dist(b) >= 1.0 {
                ids.push(self.project.add_shape(floor, Shape::wall(a, b)));
            }
        }
        self.selection = ids;
    }

    pub fn add_floor(&mut self) {
        if self.project.floors.len() >= 8 {
            return;
        }
        self.checkpoint();
        self.floor = self.project.add_floor();
        self.selection.clear();
    }

    pub fn delete_floor(&mut self, idx: usize) {
        if self.project.floors.len() <= 1 || idx >= self.project.floors.len() {
            return;
        }
        self.checkpoint();
        self.project.floors.remove(idx);
        self.floor = self.floor.min(self.project.floors.len() - 1);
        self.selection.clear();
    }

    pub fn rename_floor(&mut self, idx: usize, name: String) {
        self.checkpoint_coalesced(&format!("floor{idx}"));
        if let Some(f) = self.project.floors.get_mut(idx) {
            f.name = name;
        }
    }

    pub fn switch_floor(&mut self, idx: usize) {
        if idx < self.project.floors.len() && idx != self.floor {
            self.floor = idx;
            self.selection.clear();
            self.cancel_drafts();
        }
    }

    pub fn set_footprint(&mut self, w: f32, h: f32) {
        self.checkpoint();
        self.project.footprint = Footprint { w: w.max(12.0), h: h.max(12.0) };
    }

    pub fn set_target_area(&mut self, sq_inches: f32) {
        self.checkpoint_coalesced("target");
        self.project.target_area = sq_inches.max(144.0);
    }

    pub fn set_units(&mut self, units: crate::units::Units) {
        if self.project.units != units {
            self.checkpoint();
            self.project.units = units;
            self.project.snap = units.default_snap();
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.checkpoint_coalesced("name");
        self.project.name = name;
    }

    pub fn fit(&mut self, size: V2) {
        let ext = self.project.extents(self.floor);
        self.view = View::fit(ext, size, 80.0);
        self.view_ready = true;
    }

    // ---- handles ------------------------------------------------------------

    /// Handles to show for a single selected shape, with world positions.
    pub fn handles(&self, s: &Shape) -> Vec<(Handle, V2)> {
        if s.locked {
            return Vec::new();
        }
        let mut out = Vec::new();
        match s.kind {
            Kind::Wall => {
                for (i, p) in s.points.iter().enumerate() {
                    out.push((Handle::Vertex(i), *p));
                }
            }
            Kind::Polygon => {
                for (i, p) in s.points.iter().enumerate() {
                    out.push((Handle::Vertex(i), *p));
                }
                for i in 0..s.points.len() {
                    out.push((Handle::Mid(i), s.points[i].lerp(s.points[(i + 1) % s.points.len()], 0.5)));
                }
            }
            k if k.is_opening() => {
                for hx in [-1i8, 1] {
                    out.push((Handle::Edge(hx, 0), s.center + v2(hx as f32 * s.size.x / 2.0, 0.0).rotated(s.rot)));
                }
            }
            _ => {
                for hy in -1i8..=1 {
                    for hx in -1i8..=1 {
                        if hx != 0 || hy != 0 {
                            let local = v2(hx as f32 * s.size.x / 2.0, hy as f32 * s.size.y / 2.0);
                            out.push((Handle::Edge(hx, hy), s.center + local.rotated(s.rot)));
                        }
                    }
                }
                let off = 26.0 / self.view.scale;
                out.push((Handle::Rotate, s.center + v2(0.0, -s.size.y / 2.0 - off).rotated(s.rot)));
            }
        }
        out
    }

    pub fn handle_at(&self, s: &Shape, world: V2) -> Option<Handle> {
        let tol = 8.0 / self.view.scale;
        self.handles(s)
            .into_iter()
            .filter(|(_, p)| p.dist(world) <= tol)
            .min_by(|a, b| a.1.dist(world).total_cmp(&b.1.dist(world)))
            .map(|(h, _)| h)
    }

    // ---- drag application -----------------------------------------------------

    /// Apply a resize of a box from its original, given the cursor in world space.
    pub fn apply_resize(&mut self, orig: &Shape, hx: i8, hy: i8, cursor: V2) {
        let step = if self.snap_on { self.project.snap } else { 0.0 };
        let min = if step > 0.0 { step } else { 1.0 };
        let local = (cursor - orig.center).rotated(-orig.rot);
        let mut size = orig.size;
        let mut shift = V2::ZERO;
        if hx != 0 {
            let anchor = -(hx as f32) * orig.size.x / 2.0;
            let w = (((local.x - anchor) * hx as f32).max(0.0)).max(min);
            let w = if step > 0.0 { snap(w, step).max(min) } else { w };
            size.x = w;
            shift.x = anchor + hx as f32 * w / 2.0;
        }
        if hy != 0 {
            let anchor = -(hy as f32) * orig.size.y / 2.0;
            let h = (((local.y - anchor) * hy as f32).max(0.0)).max(min);
            let h = if step > 0.0 { snap(h, step).max(min) } else { h };
            size.y = h;
            shift.y = anchor + hy as f32 * h / 2.0;
        }
        let id = orig.id;
        if let Some(s) = self.floor_mut().find_mut(id) {
            s.size = size;
            s.center = orig.center + shift.rotated(orig.rot);
        }
    }

    pub fn apply_vertex(&mut self, orig: &Shape, idx: usize, linked: &[(u64, usize, Shape)], target: V2) {
        let id = orig.id;
        if let Some(s) = self.floor_mut().find_mut(id) {
            if idx < s.points.len() {
                s.points[idx] = target;
            }
        }
        // Walls that shared this corner follow it.
        for (lid, lidx, lorig) in linked {
            if let Some(s) = self.floor_mut().find_mut(*lid) {
                let mut moved = lorig.clone();
                moved.points[*lidx] = target;
                *s = moved;
            }
        }
    }

    /// Other walls with an end exactly where `orig`'s end `idx` is.
    pub fn linked_wall_ends(&self, orig: &Shape, idx: usize) -> Vec<(u64, usize, Shape)> {
        if orig.kind != Kind::Wall {
            return Vec::new();
        }
        let p = orig.points[idx];
        self.floor()
            .shapes
            .iter()
            .filter(|s| s.kind == Kind::Wall && s.id != orig.id)
            .flat_map(|s| {
                (0..s.points.len())
                    .filter(|&i| s.points[i].dist(p) < 0.5)
                    .map(|i| (s.id, i, s.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn apply_rotation(&mut self, orig: &Shape, pivot: V2, delta: f32) {
        let id = orig.id;
        if let Some(s) = self.floor_mut().find_mut(id) {
            let mut r = orig.clone();
            r.rotate_about(pivot, delta);
            *s = r;
        }
    }

    pub fn insert_vertex(&mut self, id: u64, after: usize, at: V2) -> Option<usize> {
        let s = self.floor_mut().find_mut(id)?;
        if s.kind != Kind::Polygon {
            return None;
        }
        s.points.insert(after + 1, at);
        Some(after + 1)
    }

    /// Place a door/window ghost or real opening along a wall near `cursor`.
    /// Returns (center, angle, depth, flip_y) if a wall is within reach.
    pub fn opening_placement(&self, cursor: V2, width: f32) -> Option<(V2, f32, f32, bool)> {
        let reach = 28.0 / self.view.scale;
        let (a, b, t) = self.floor().wall_near(cursor, reach)?;
        let len = a.dist(b);
        if len < width {
            return None;
        }
        let dir = (b - a).normalized();
        let along = (cursor - a).dot(dir);
        let step = self.snap_step();
        let mut s = if step > 0.0 { snap(along, step) } else { along };
        s = s.clamp(width / 2.0, len - width / 2.0);
        let center = a + dir * s;
        let ang = dir.angle();
        // Swing toward the side the cursor is on.
        let side = (cursor - center).dot(dir.perp());
        Some((center, ang, t, side > 0.0))
    }

    pub fn opening_width(tool: Tool) -> f32 {
        match tool {
            Tool::Window => WINDOW_WIDTH,
            Tool::SlidingDoor => 72.0,
            _ => DOOR_WIDTH,
        }
    }

    /// Create a rectangle-style shape from two opposite corners.
    pub fn box_from_corners(&self, kind: Kind, a: V2, b: V2) -> Option<Shape> {
        let (lo, hi) = (v2(a.x.min(b.x), a.y.min(b.y)), v2(a.x.max(b.x), a.y.max(b.y)));
        let size = hi - lo;
        let min = self.project.snap.max(1.0);
        if size.x < min || size.y < min {
            return None;
        }
        let mut s = Shape::boxed(kind, (lo + hi) * 0.5, size);
        s.color = self.next_color();
        Some(s)
    }

    /// Colors cycle so neighbouring new rooms look different.
    pub fn next_color(&self) -> u8 {
        let rooms = self.floor().shapes.iter().filter(|s| s.kind.is_room()).count();
        (rooms % ROOM_COLORS) as u8
    }

    pub fn finish_polygon(&mut self) -> bool {
        if self.poly.len() < 3 {
            return false;
        }
        let mut s = Shape::polygon(std::mem::take(&mut self.poly));
        s.color = self.next_color();
        if s.is_degenerate() {
            return false;
        }
        let id = self.add(s);
        self.set_tool(Tool::Select);
        self.select_only(id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Units;

    fn editor() -> Editor {
        let p = Project::create(&NewPlan {
            name: "T".into(),
            units: Units::Imperial,
            area: 1200.0,
            floors: 1,
            proportion: Proportion::Classic,
        });
        let mut e = Editor::new(p);
        e.view = View { origin: V2::ZERO, scale: 1.0 };
        e
    }

    fn room(e: &mut Editor, x: f32, y: f32, w: f32, h: f32) -> u64 {
        e.add(Shape::boxed(Kind::Rect, v2(x, y), v2(w, h)))
    }

    #[test]
    fn undo_and_redo_restore_the_drawing() {
        let mut e = editor();
        room(&mut e, 60.0, 60.0, 120.0, 120.0);
        assert_eq!(e.floor().shapes.len(), 1);
        e.undo();
        assert_eq!(e.floor().shapes.len(), 0);
        e.redo();
        assert_eq!(e.floor().shapes.len(), 1);
        assert!(!e.can_redo());
    }

    #[test]
    fn a_new_edit_clears_the_redo_stack() {
        let mut e = editor();
        room(&mut e, 60.0, 60.0, 120.0, 120.0);
        e.undo();
        assert!(e.can_redo());
        room(&mut e, 0.0, 0.0, 12.0, 12.0);
        assert!(!e.can_redo());
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut e = editor();
        for _ in 0..(UNDO_LIMIT + 20) {
            room(&mut e, 0.0, 0.0, 12.0, 12.0);
        }
        assert_eq!(e.undo.len(), UNDO_LIMIT);
    }

    #[test]
    fn typing_in_a_field_is_one_undo_step() {
        let mut e = editor();
        let id = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        let before = e.undo.len();
        for t in ["K", "Ki", "Kit", "Kitc"] {
            e.set_label(id, t.into());
        }
        assert_eq!(e.undo.len(), before + 1);
        e.undo();
        assert_eq!(e.floor().find(id).unwrap().label, "");
    }

    #[test]
    fn undo_drops_selection_of_removed_shapes() {
        let mut e = editor();
        let id = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        e.select_only(id);
        e.undo();
        assert!(e.selection.is_empty());
    }

    #[test]
    fn delete_skips_locked_shapes() {
        let mut e = editor();
        let a = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        let b = room(&mut e, 300.0, 60.0, 120.0, 120.0);
        e.selection = vec![a, b];
        e.set_locked(true);
        e.selection = vec![a];
        e.set_locked(false);
        e.selection = vec![a, b];
        e.delete_selection();
        assert!(e.floor().find(a).is_none());
        assert!(e.floor().find(b).is_some(), "locked shape must survive");
    }

    #[test]
    fn duplicate_offsets_and_selects_the_copy() {
        let mut e = editor();
        let a = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        e.select_only(a);
        e.duplicate_selection();
        assert_eq!(e.floor().shapes.len(), 2);
        assert_ne!(e.selection[0], a);
        let copy = e.single().unwrap();
        assert_eq!(copy.center, v2(84.0, 84.0));
    }

    #[test]
    fn resize_from_the_right_edge_keeps_the_left_edge_fixed() {
        let mut e = editor();
        let id = room(&mut e, 60.0, 60.0, 120.0, 120.0); // spans x 0..120
        let orig = e.floor().find(id).unwrap().clone();
        e.apply_resize(&orig, 1, 0, v2(198.0, 60.0));
        let s = e.floor().find(id).unwrap();
        assert_eq!(s.size.x, 198.0);
        assert_eq!(s.size.y, 120.0);
        assert_eq!(s.center.x - s.size.x / 2.0, 0.0);
    }

    #[test]
    fn resize_snaps_to_the_grid_and_never_collapses() {
        let mut e = editor();
        let id = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        let orig = e.floor().find(id).unwrap().clone();
        e.apply_resize(&orig, 1, 1, v2(133.0, 147.0));
        let s = e.floor().find(id).unwrap();
        assert_eq!(s.size.x % 6.0, 0.0);
        assert_eq!(s.size.y % 6.0, 0.0);
        // Drag past the opposite edge: clamps to the minimum size.
        e.apply_resize(&orig, 1, 0, v2(-500.0, 60.0));
        assert_eq!(e.floor().find(id).unwrap().size.x, 6.0);
    }

    #[test]
    fn resize_works_on_rotated_boxes() {
        let mut e = editor();
        let id = room(&mut e, 100.0, 100.0, 120.0, 60.0);
        e.floor_mut().find_mut(id).unwrap().rot = std::f32::consts::FRAC_PI_2;
        let orig = e.floor().find(id).unwrap().clone();
        // The box's local +x axis now points down the screen (+y world).
        // Pull the local right edge from 60 to 120 along it.
        e.apply_resize(&orig, 1, 0, v2(100.0, 220.0));
        let s = e.floor().find(id).unwrap();
        assert_eq!(s.size.x, 180.0);
        // Local left edge (world y = 40) stays put.
        assert!((s.center.y - 130.0).abs() < 0.01 && (s.center.x - 100.0).abs() < 0.01, "{:?}", s.center);
    }

    #[test]
    fn dragging_a_wall_corner_moves_connected_walls() {
        let mut e = editor();
        let a = e.add(Shape::wall(v2(0.0, 0.0), v2(100.0, 0.0)));
        let b = e.add(Shape::wall(v2(100.0, 0.0), v2(100.0, 100.0)));
        let c = e.add(Shape::wall(v2(300.0, 300.0), v2(400.0, 300.0)));
        let orig = e.floor().find(a).unwrap().clone();
        let linked = e.linked_wall_ends(&orig, 1);
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].0, b);
        e.apply_vertex(&orig, 1, &linked, v2(120.0, 20.0));
        assert_eq!(e.floor().find(a).unwrap().points[1], v2(120.0, 20.0));
        assert_eq!(e.floor().find(b).unwrap().points[0], v2(120.0, 20.0));
        assert_eq!(e.floor().find(b).unwrap().points[1], v2(100.0, 100.0));
        assert_eq!(e.floor().find(c).unwrap().points[0], v2(300.0, 300.0));
    }

    #[test]
    fn snapping_prefers_nearby_corners_over_the_grid() {
        let mut e = editor();
        e.add(Shape::wall(v2(7.0, 7.0), v2(200.0, 7.0)));
        // 3 inches from the wall's end, which is itself off-grid.
        let p = e.snap_point(v2(9.0, 9.0), &[]);
        assert_eq!(p, v2(7.0, 7.0));
        // Far from any corner: grid.
        let q = e.snap_point(v2(95.0, 61.0), &[]);
        assert_eq!(q, v2(96.0, 60.0));
        e.snap_on = false;
        assert_eq!(e.snap_point(v2(95.0, 61.0), &[]), v2(95.0, 61.0));
    }

    #[test]
    fn corner_snapping_tolerance_follows_zoom() {
        let mut e = editor();
        e.add(Shape::wall(v2(7.0, 7.0), v2(200.0, 7.0)));
        e.view.scale = 0.2; // zoomed far out: 10px = 50 inches
        assert_eq!(e.snap_point(v2(30.0, 20.0), &[]), v2(7.0, 7.0));
    }

    #[test]
    fn doors_snap_onto_walls_and_swing_toward_the_cursor() {
        let mut e = editor();
        e.add(Shape::wall(v2(0.0, 0.0), v2(240.0, 0.0)));
        let (c, ang, depth, flip) = e.opening_placement(v2(100.0, 10.0), 36.0).unwrap();
        assert_eq!(c.y, 0.0);
        assert_eq!(c.x % 6.0, 0.0);
        assert_eq!(ang, 0.0);
        assert_eq!(depth, DEFAULT_WALL);
        assert!(flip, "cursor below the wall (+y) should swing the door downward");
        let (_, _, _, flip_up) = e.opening_placement(v2(100.0, -10.0), 36.0).unwrap();
        assert!(!flip_up);
        assert!(e.opening_placement(v2(100.0, 200.0), 36.0).is_none());
    }

    #[test]
    fn openings_stay_fully_on_their_wall() {
        let mut e = editor();
        e.add(Shape::wall(v2(0.0, 0.0), v2(240.0, 0.0)));
        let (c, ..) = e.opening_placement(v2(2.0, 3.0), 36.0).unwrap();
        assert_eq!(c.x, 18.0);
        let (c, ..) = e.opening_placement(v2(239.0, 3.0), 36.0).unwrap();
        assert_eq!(c.x, 222.0);
        // A wall shorter than the opening can't hold it.
        let mut e2 = editor();
        e2.add(Shape::wall(v2(0.0, 0.0), v2(20.0, 0.0)));
        assert!(e2.opening_placement(v2(10.0, 2.0), 36.0).is_none());
    }

    #[test]
    fn walls_around_a_room_follow_its_outline() {
        let mut e = editor();
        let id = room(&mut e, 60.0, 60.0, 120.0, 120.0);
        e.select_only(id);
        e.walls_around_selection();
        let walls = e.floor().shapes.iter().filter(|s| s.kind == Kind::Wall).count();
        assert_eq!(walls, 4);
        assert_eq!(e.selection.len(), 4);
        // Ellipses have no straight edges to wall.
        let mut e2 = editor();
        let el = e2.add(Shape::boxed(Kind::Ellipse, v2(60.0, 60.0), v2(100.0, 100.0)));
        e2.select_only(el);
        e2.walls_around_selection();
        assert_eq!(e2.floor().shapes.len(), 1);
    }

    #[test]
    fn polygon_needs_three_points() {
        let mut e = editor();
        e.set_tool(Tool::Polygon);
        e.poly = vec![v2(0.0, 0.0), v2(60.0, 0.0)];
        assert!(!e.finish_polygon());
        e.poly.push(v2(60.0, 60.0));
        assert!(e.finish_polygon());
        assert_eq!(e.tool, Tool::Select);
        assert_eq!(e.selection.len(), 1);
    }

    #[test]
    fn tiny_drags_do_not_create_rooms() {
        let e = editor();
        assert!(e.box_from_corners(Kind::Rect, v2(0.0, 0.0), v2(3.0, 100.0)).is_none());
        assert!(e.box_from_corners(Kind::Rect, v2(100.0, 100.0), v2(0.0, 0.0)).is_some());
    }

    #[test]
    fn new_rooms_cycle_through_colors() {
        let mut e = editor();
        let a = e.box_from_corners(Kind::Rect, v2(0.0, 0.0), v2(60.0, 60.0)).unwrap();
        e.add(a);
        let b = e.box_from_corners(Kind::Rect, v2(0.0, 0.0), v2(60.0, 60.0)).unwrap();
        assert_ne!(e.floor().shapes[0].color, b.color);
    }

    #[test]
    fn floors_can_be_added_renamed_and_removed_but_never_all_deleted() {
        let mut e = editor();
        e.add_floor();
        assert_eq!(e.project.floors.len(), 2);
        assert_eq!(e.floor, 1);
        e.rename_floor(1, "Loft".into());
        assert_eq!(e.project.floors[1].name, "Loft");
        e.delete_floor(1);
        e.delete_floor(0);
        assert_eq!(e.project.floors.len(), 1);
    }

    #[test]
    fn floor_count_is_capped() {
        let mut e = editor();
        for _ in 0..20 {
            e.add_floor();
        }
        assert_eq!(e.project.floors.len(), 8);
    }

    #[test]
    fn rotating_a_selection_turns_it_about_its_center() {
        let mut e = editor();
        let a = room(&mut e, 100.0, 100.0, 100.0, 40.0);
        e.select_only(a);
        e.rotate_selection(std::f32::consts::FRAC_PI_2);
        let s = e.floor().find(a).unwrap();
        assert!((s.center.x - 100.0).abs() < 0.01 && (s.center.y - 100.0).abs() < 0.01);
        assert!((s.rot - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn handles_exist_for_every_shape_kind() {
        let mut e = editor();
        let r = room(&mut e, 100.0, 100.0, 100.0, 40.0);
        assert_eq!(e.handles(e.floor().find(r).unwrap()).len(), 9); // 8 edges + rotate
        let w = e.add(Shape::wall(v2(0.0, 0.0), v2(50.0, 0.0)));
        assert_eq!(e.handles(e.floor().find(w).unwrap()).len(), 2);
        let p = e.add(Shape::polygon(vec![v2(0.0, 0.0), v2(50.0, 0.0), v2(0.0, 50.0)]));
        assert_eq!(e.handles(e.floor().find(p).unwrap()).len(), 6); // 3 corners + 3 midpoints
        let d = e.add(Shape::opening(Kind::Door, v2(10.0, 0.0), 0.0, 36.0, 6.0));
        assert_eq!(e.handles(e.floor().find(d).unwrap()).len(), 2);
        e.select_only(r);
        e.set_locked(true);
        assert!(e.handles(e.floor().find(r).unwrap()).is_empty());
    }

    #[test]
    fn handle_hit_testing_scales_with_zoom() {
        let mut e = editor();
        let r = room(&mut e, 100.0, 100.0, 100.0, 40.0);
        let s = e.floor().find(r).unwrap().clone();
        // Right edge midpoint is at (150, 100).
        assert_eq!(e.handle_at(&s, v2(153.0, 101.0)), Some(Handle::Edge(1, 0)));
        assert_eq!(e.handle_at(&s, v2(170.0, 100.0)), None);
        e.view.scale = 0.25; // 8px is now 32 inches
        assert_eq!(e.handle_at(&s, v2(170.0, 100.0)), Some(Handle::Edge(1, 0)));
    }

    #[test]
    fn changing_units_resets_the_snap_step() {
        let mut e = editor();
        e.set_units(Units::Metric);
        assert_eq!(e.project.snap, Units::Metric.default_snap());
        e.undo();
        assert_eq!(e.project.units, Units::Imperial);
    }

    #[test]
    fn inserting_a_polygon_vertex() {
        let mut e = editor();
        let p = e.add(Shape::polygon(vec![v2(0.0, 0.0), v2(60.0, 0.0), v2(0.0, 60.0)]));
        let idx = e.insert_vertex(p, 0, v2(30.0, -10.0)).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(e.floor().find(p).unwrap().points.len(), 4);
        assert_eq!(e.floor().find(p).unwrap().points[1], v2(30.0, -10.0));
    }
}
