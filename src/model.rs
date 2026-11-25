//! The plan itself: projects, floors and the shapes drawn on them.
//! All geometry is in inches (see `units.rs` for display and parsing).

use crate::geom::*;
use crate::units::{Units, SQIN_PER_SQFT};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const ROOM_COLORS: usize = 8;
pub const DEFAULT_WALL: f32 = 6.0;
pub const DOOR_WIDTH: f32 = 36.0;
pub const WINDOW_WIDTH: f32 = 48.0;

/// Furniture and fixture symbols, drawn in plan view by `symbols.rs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Sym {
    Bed,
    Sofa,
    Armchair,
    DiningTable,
    RoundTable,
    Desk,
    Wardrobe,
    Toilet,
    Vanity,
    Bathtub,
    Shower,
    Counter,
    Stove,
    Fridge,
    WasherDryer,
    Fireplace,
    Stairs,
}

impl Sym {
    pub const ALL: [Sym; 17] = [
        Sym::Bed,
        Sym::Sofa,
        Sym::Armchair,
        Sym::DiningTable,
        Sym::RoundTable,
        Sym::Desk,
        Sym::Wardrobe,
        Sym::Toilet,
        Sym::Vanity,
        Sym::Bathtub,
        Sym::Shower,
        Sym::Counter,
        Sym::Stove,
        Sym::Fridge,
        Sym::WasherDryer,
        Sym::Fireplace,
        Sym::Stairs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Sym::Bed => "Bed",
            Sym::Sofa => "Sofa",
            Sym::Armchair => "Armchair",
            Sym::DiningTable => "Dining table",
            Sym::RoundTable => "Round table",
            Sym::Desk => "Desk",
            Sym::Wardrobe => "Wardrobe",
            Sym::Toilet => "Toilet",
            Sym::Vanity => "Vanity sink",
            Sym::Bathtub => "Bathtub",
            Sym::Shower => "Shower",
            Sym::Counter => "Counter",
            Sym::Stove => "Stove",
            Sym::Fridge => "Fridge",
            Sym::WasherDryer => "Washer / dryer",
            Sym::Fireplace => "Fireplace",
            Sym::Stairs => "Stairs",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Sym::Bed | Sym::Sofa | Sym::Armchair | Sym::DiningTable | Sym::RoundTable
            | Sym::Desk | Sym::Wardrobe | Sym::Fireplace => "Living",
            Sym::Toilet | Sym::Vanity | Sym::Bathtub | Sym::Shower | Sym::WasherDryer => "Bath & laundry",
            Sym::Counter | Sym::Stove | Sym::Fridge => "Kitchen",
            Sym::Stairs => "Structure",
        }
    }

    /// Typical size in inches (width, depth).
    pub fn default_size(self) -> V2 {
        match self {
            Sym::Bed => v2(60.0, 80.0),
            Sym::Sofa => v2(84.0, 36.0),
            Sym::Armchair => v2(34.0, 34.0),
            Sym::DiningTable => v2(84.0, 54.0),
            Sym::RoundTable => v2(60.0, 60.0),
            Sym::Desk => v2(60.0, 48.0),
            Sym::Wardrobe => v2(72.0, 24.0),
            Sym::Toilet => v2(20.0, 28.0),
            Sym::Vanity => v2(30.0, 21.0),
            Sym::Bathtub => v2(60.0, 32.0),
            Sym::Shower => v2(36.0, 36.0),
            Sym::Counter => v2(96.0, 25.0),
            Sym::Stove => v2(30.0, 26.0),
            Sym::Fridge => v2(36.0, 30.0),
            Sym::WasherDryer => v2(54.0, 30.0),
            Sym::Fireplace => v2(60.0, 16.0),
            Sym::Stairs => v2(36.0, 120.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Rect,
    Ellipse,
    Triangle,
    Polygon,
    Wall,
    Door,
    SlidingDoor,
    Window,
    Furniture(Sym),
}

impl Kind {
    /// Shapes whose area counts toward the square-footage total.
    pub fn is_room(self) -> bool {
        matches!(self, Kind::Rect | Kind::Ellipse | Kind::Triangle | Kind::Polygon)
    }
    pub fn is_opening(self) -> bool {
        matches!(self, Kind::Door | Kind::SlidingDoor | Kind::Window)
    }
    /// Shapes described by center / size / rotation (the rest keep a point list).
    pub fn is_boxed(self) -> bool {
        !matches!(self, Kind::Polygon | Kind::Wall)
    }
    /// Back-to-front drawing layer.
    pub fn layer(self) -> u8 {
        match self {
            k if k.is_room() => 0,
            Kind::Wall => 1,
            k if k.is_opening() => 2,
            _ => 3,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Rect => "Rectangle",
            Kind::Ellipse => "Ellipse",
            Kind::Triangle => "Triangle",
            Kind::Polygon => "Custom shape",
            Kind::Wall => "Wall",
            Kind::Door => "Door",
            Kind::SlidingDoor => "Sliding door",
            Kind::Window => "Window",
            Kind::Furniture(s) => s.name(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub id: u64,
    pub kind: Kind,
    #[serde(default)]
    pub label: String,
    /// Index into the room color palette.
    #[serde(default)]
    pub color: u8,
    #[serde(default)]
    pub locked: bool,
    // Boxed shapes:
    #[serde(default)]
    pub center: V2,
    #[serde(default)]
    pub size: V2,
    #[serde(default)]
    pub rot: f32,
    /// Mirror an opening or symbol across its own x / y axis.
    #[serde(default)]
    pub flip_x: bool,
    #[serde(default)]
    pub flip_y: bool,
    // Point shapes (custom shapes, walls):
    #[serde(default)]
    pub points: Vec<V2>,
    #[serde(default)]
    pub thickness: f32,
}

impl Shape {
    fn blank(kind: Kind) -> Shape {
        Shape {
            id: 0,
            kind,
            label: String::new(),
            color: 0,
            locked: false,
            center: V2::ZERO,
            size: V2::ZERO,
            rot: 0.0,
            flip_x: false,
            flip_y: false,
            points: Vec::new(),
            thickness: 0.0,
        }
    }

    pub fn boxed(kind: Kind, center: V2, size: V2) -> Shape {
        debug_assert!(kind.is_boxed());
        Shape { center, size, ..Shape::blank(kind) }
    }

    pub fn polygon(points: Vec<V2>) -> Shape {
        Shape { points, ..Shape::blank(Kind::Polygon) }
    }

    pub fn wall(a: V2, b: V2) -> Shape {
        Shape { points: vec![a, b], thickness: DEFAULT_WALL, ..Shape::blank(Kind::Wall) }
    }

    pub fn opening(kind: Kind, center: V2, rot: f32, width: f32, depth: f32) -> Shape {
        debug_assert!(kind.is_opening());
        Shape { rot, ..Shape::boxed(kind, center, v2(width, depth)) }
    }

    /// The shape's outline in world coordinates. Walls get square end caps so
    /// corners join cleanly.
    pub fn outline(&self) -> Vec<V2> {
        match self.kind {
            Kind::Ellipse => ellipse_points(self.center, self.size, self.rot, 72),
            Kind::Triangle => triangle_points(self.center, self.size, self.rot).to_vec(),
            Kind::Polygon => self.points.clone(),
            Kind::Wall => {
                let (a, b) = self.wall_ends();
                let dir = (b - a).normalized();
                let half = self.thickness.max(1.0) / 2.0;
                let n = dir.perp() * half;
                let (a, b) = (a - dir * half, b + dir * half);
                vec![a + n, b + n, b - n, a - n]
            }
            _ => box_corners(self.center, self.size, self.rot).to_vec(),
        }
    }

    pub fn wall_ends(&self) -> (V2, V2) {
        match self.points.as_slice() {
            [a, b, ..] => (*a, *b),
            _ => (V2::ZERO, V2::ZERO),
        }
    }

    pub fn area(&self) -> f32 {
        match self.kind {
            Kind::Rect => self.size.x * self.size.y,
            Kind::Ellipse => std::f32::consts::PI * self.size.x * self.size.y / 4.0,
            Kind::Triangle => self.size.x * self.size.y / 2.0,
            Kind::Polygon => polygon_area(&self.points),
            _ => 0.0,
        }
    }

    pub fn centroid(&self) -> V2 {
        match self.kind {
            Kind::Polygon => polygon_centroid(&self.points),
            Kind::Wall => {
                let (a, b) = self.wall_ends();
                a.lerp(b, 0.5)
            }
            Kind::Triangle => {
                // Centroid sits a third of the way up from the base.
                self.center + v2(0.0, self.size.y / 6.0).rotated(self.rot)
            }
            _ => self.center,
        }
    }

    pub fn bounds(&self) -> Option<(V2, V2)> {
        bounds(&self.outline())
    }

    pub fn hit(&self, p: V2, tol: f32) -> bool {
        match self.kind {
            Kind::Wall => {
                let (a, b) = self.wall_ends();
                dist_to_segment(p, a, b) <= (self.thickness / 2.0).max(tol)
            }
            k if k.is_room() => {
                let o = self.outline();
                point_in_polygon(p, &o)
                    || (0..o.len()).any(|i| dist_to_segment(p, o[i], o[(i + 1) % o.len()]) <= tol)
            }
            _ => {
                // Test in the box's own frame so rotation is handled exactly.
                let local = (p - self.center).rotated(-self.rot);
                local.x.abs() <= self.size.x / 2.0 + tol && local.y.abs() <= self.size.y / 2.0 + tol
            }
        }
    }

    pub fn translate(&mut self, d: V2) {
        self.center += d;
        for p in &mut self.points {
            *p += d;
        }
    }

    /// Rotate by `delta` radians about `pivot`.
    pub fn rotate_about(&mut self, pivot: V2, delta: f32) {
        self.center = pivot + (self.center - pivot).rotated(delta);
        self.rot = wrap_angle(self.rot + delta);
        for p in &mut self.points {
            *p = pivot + (*p - pivot).rotated(delta);
        }
    }

    pub fn display_name(&self) -> String {
        if self.label.trim().is_empty() {
            self.kind.name().to_string()
        } else {
            self.label.clone()
        }
    }

    /// Longest dimension, for sorting and sanity checks.
    pub fn is_degenerate(&self) -> bool {
        match self.kind {
            Kind::Polygon => self.points.len() < 3 || self.area() < 1.0,
            Kind::Wall => {
                let (a, b) = self.wall_ends();
                a.dist(b) < 1.0
            }
            _ => self.size.x < 1.0 || self.size.y < 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    pub shapes: Vec<Shape>,
}

impl Floor {
    /// Indices into `shapes`, back to front.
    pub fn draw_order(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.shapes.len()).collect();
        idx.sort_by_key(|&i| self.shapes[i].kind.layer());
        idx
    }

    /// The top-most shape under `p`, if any.
    pub fn pick(&self, p: V2, tol: f32) -> Option<u64> {
        self.draw_order()
            .into_iter()
            .rev()
            .map(|i| &self.shapes[i])
            .find(|s| s.hit(p, tol))
            .map(|s| s.id)
    }

    pub fn find(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }

    pub fn area(&self) -> f32 {
        self.shapes.iter().map(Shape::area).sum()
    }

    /// The nearest wall within `reach` of `p`: its end points and thickness.
    pub fn wall_near(&self, p: V2, reach: f32) -> Option<(V2, V2, f32)> {
        self.shapes
            .iter()
            .filter(|s| s.kind == Kind::Wall && !s.is_degenerate())
            .filter_map(|w| {
                let (a, b) = w.wall_ends();
                let d = dist_to_segment(p, a, b);
                (d <= reach).then_some((d, a, b, w.thickness))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, a, b, t)| (a, b, t))
    }

    /// Where a door or window would sit if dropped near `p`: the closest point
    /// on a wall within `reach`, with the wall's angle and thickness.
    pub fn opening_slot(&self, p: V2, reach: f32) -> Option<(V2, f32, f32)> {
        let (a, b, t) = self.wall_near(p, reach)?;
        Some((closest_on_segment(p, a, b).0, (b - a).angle(), t))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Proportion {
    Square,
    Classic,
    Wide,
    Long,
}

impl Proportion {
    pub const ALL: [Proportion; 4] =
        [Proportion::Square, Proportion::Classic, Proportion::Wide, Proportion::Long];
    pub fn ratio(self) -> f32 {
        match self {
            Proportion::Square => 1.0,
            Proportion::Classic => 4.0 / 3.0,
            Proportion::Wide => 1.5,
            Proportion::Long => 2.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Proportion::Square => "Square",
            Proportion::Classic => "4 : 3",
            Proportion::Wide => "3 : 2",
            Proportion::Long => "2 : 1",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Footprint {
    pub w: f32,
    pub h: f32,
}

impl Footprint {
    /// A footprint of about `area` square inches at width:depth `ratio`,
    /// rounded to the snap grid so its edges land on grid lines.
    pub fn from_area(area: f32, ratio: f32, snap_step: f32) -> Footprint {
        let w = (area * ratio).max(1.0).sqrt();
        let step = snap_step.max(1.0);
        let w = (w / step).round().max(1.0) * step;
        let h = ((area / w) / step).round().max(1.0) * step;
        Footprint { w, h }
    }

    pub fn area(self) -> f32 {
        self.w * self.h
    }

    pub fn rect(self) -> [V2; 4] {
        [v2(0.0, 0.0), v2(self.w, 0.0), v2(self.w, self.h), v2(0.0, self.h)]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub units: Units,
    /// Total living area wanted across all floors, in square inches.
    pub target_area: f32,
    pub footprint: Footprint,
    pub floors: Vec<Floor>,
    pub next_id: u64,
    /// Drawing snap increment in inches.
    pub snap: f32,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

pub struct NewPlan {
    pub name: String,
    pub units: Units,
    /// Total living area in the chosen unit (sq ft or m²).
    pub area: f32,
    pub floors: usize,
    pub proportion: Proportion,
}

impl Project {
    pub fn create(n: &NewPlan) -> Project {
        let floors = n.floors.clamp(1, 6);
        let target = n.units.area_to_sq_inches(n.area.max(1.0));
        let snap = n.units.default_snap();
        let now = Utc::now();
        Project {
            version: FORMAT_VERSION,
            id: fresh_id(),
            name: if n.name.trim().is_empty() { "Untitled plan".into() } else { n.name.trim().into() },
            units: n.units,
            target_area: target,
            footprint: Footprint::from_area(target / floors as f32, n.proportion.ratio(), snap),
            floors: (0..floors)
                .map(|i| Floor {
                    name: floor_name(i, floors),
                    shapes: Vec::new(),
                })
                .collect(),
            next_id: 1,
            snap,
            created: now,
            modified: now,
        }
    }

    pub fn add_shape(&mut self, floor: usize, mut shape: Shape) -> u64 {
        shape.id = self.next_id;
        self.next_id += 1;
        let id = shape.id;
        self.floors[floor].shapes.push(shape);
        id
    }

    pub fn total_area(&self) -> f32 {
        self.floors.iter().map(Floor::area).sum()
    }

    /// Everything that should be visible when fitting the plan to a view or page.
    pub fn extents(&self, floor: usize) -> (V2, V2) {
        let mut pts: Vec<V2> = self.footprint.rect().to_vec();
        for s in &self.floors[floor].shapes {
            pts.extend(s.outline());
        }
        bounds(&pts).unwrap_or((V2::ZERO, v2(self.footprint.w, self.footprint.h)))
    }

    pub fn add_floor(&mut self) -> usize {
        let n = self.floors.len();
        self.floors.push(Floor { name: format!("Floor {}", n + 1), shapes: Vec::new() });
        n
    }
}

fn floor_name(i: usize, total: usize) -> String {
    match (i, total) {
        (0, 1) => "Main floor".into(),
        (0, _) => "Ground floor".into(),
        (1, _) => "Second floor".into(),
        (2, _) => "Third floor".into(),
        (n, _) => format!("Floor {}", n + 1),
    }
}

pub fn fresh_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{:x}{:03x}", nanos, N.fetch_add(1, Ordering::Relaxed) & 0xfff)
}

pub fn sqft(sq_inches: f32) -> f32 {
    sq_inches / SQIN_PER_SQFT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(area: f32, floors: usize, p: Proportion) -> Project {
        Project::create(&NewPlan {
            name: "Test".into(),
            units: Units::Imperial,
            area,
            floors,
            proportion: p,
        })
    }

    #[test]
    fn footprint_matches_target_area_per_floor() {
        for &(area, floors) in &[(1200.0, 1), (2400.0, 2), (3000.0, 3), (850.0, 1)] {
            for p in Proportion::ALL {
                let pr = plan(area, floors, p);
                let want = area / floors as f32;
                let got = sqft(pr.footprint.area());
                assert!((got - want).abs() / want < 0.06, "{area} {floors} {p:?}: {got} vs {want}");
                assert_eq!(pr.floors.len(), floors);
            }
        }
    }

    #[test]
    fn footprint_follows_proportion() {
        let p = plan(2000.0, 1, Proportion::Long);
        assert!(p.footprint.w > p.footprint.h * 1.8);
        let sq = plan(2000.0, 1, Proportion::Square);
        assert!((sq.footprint.w - sq.footprint.h).abs() <= 12.0);
    }

    #[test]
    fn footprint_edges_are_on_the_snap_grid() {
        let p = plan(1234.0, 1, Proportion::Wide);
        assert_eq!(p.footprint.w % p.snap, 0.0);
        assert_eq!(p.footprint.h % p.snap, 0.0);
    }

    #[test]
    fn shape_areas() {
        let r = Shape::boxed(Kind::Rect, v2(0.0, 0.0), v2(120.0, 144.0));
        assert_eq!(r.area(), 120.0 * 144.0);
        let e = Shape::boxed(Kind::Ellipse, v2(0.0, 0.0), v2(100.0, 100.0));
        assert!((e.area() - std::f32::consts::PI * 2500.0).abs() < 1.0);
        let t = Shape::boxed(Kind::Triangle, v2(0.0, 0.0), v2(100.0, 60.0));
        assert_eq!(t.area(), 3000.0);
        let poly = Shape::polygon(vec![v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 50.0), v2(0.0, 50.0)]);
        assert_eq!(poly.area(), 5000.0);
        let w = Shape::wall(v2(0.0, 0.0), v2(100.0, 0.0));
        assert_eq!(w.area(), 0.0);
    }

    #[test]
    fn total_area_only_counts_rooms() {
        let mut p = plan(1000.0, 1, Proportion::Square);
        p.add_shape(0, Shape::boxed(Kind::Rect, v2(100.0, 100.0), v2(120.0, 120.0)));
        p.add_shape(0, Shape::wall(v2(0.0, 0.0), v2(200.0, 0.0)));
        p.add_shape(0, Shape::boxed(Kind::Furniture(Sym::Bed), v2(100.0, 100.0), Sym::Bed.default_size()));
        assert_eq!(p.total_area(), 120.0 * 120.0);
    }

    #[test]
    fn ids_are_unique_and_increment() {
        let mut p = plan(1000.0, 1, Proportion::Square);
        let a = p.add_shape(0, Shape::wall(v2(0.0, 0.0), v2(10.0, 0.0)));
        let b = p.add_shape(0, Shape::wall(v2(0.0, 0.0), v2(10.0, 0.0)));
        assert_ne!(a, b);
        assert!(p.floors[0].find(a).is_some());
    }

    #[test]
    fn hit_testing_respects_rotation_and_layers() {
        let mut p = plan(1000.0, 1, Proportion::Square);
        let room = p.add_shape(0, Shape::boxed(Kind::Rect, v2(100.0, 100.0), v2(200.0, 200.0)));
        let mut bed = Shape::boxed(Kind::Furniture(Sym::Bed), v2(100.0, 100.0), v2(60.0, 80.0));
        bed.rot = std::f32::consts::FRAC_PI_2; // now 80 wide, 60 tall
        let bed = p.add_shape(0, bed);
        let f = &p.floors[0];
        // Furniture sits above the room.
        assert_eq!(f.pick(v2(100.0, 100.0), 1.0), Some(bed));
        // Inside rotated width (80/2 = 40) but outside unrotated width (30).
        assert_eq!(f.pick(v2(135.0, 100.0), 1.0), Some(bed));
        // Outside the bed but inside the room.
        assert_eq!(f.pick(v2(100.0, 180.0), 1.0), Some(room));
        assert_eq!(f.pick(v2(500.0, 500.0), 1.0), None);
    }

    #[test]
    fn walls_are_hit_along_their_thickness() {
        let w = Shape::wall(v2(0.0, 0.0), v2(100.0, 0.0));
        assert!(w.hit(v2(50.0, 2.0), 0.0));
        assert!(!w.hit(v2(50.0, 10.0), 0.0));
        assert!(w.hit(v2(50.0, 10.0), 11.0));
    }

    #[test]
    fn walls_have_capped_ends() {
        let w = Shape::wall(v2(0.0, 0.0), v2(100.0, 0.0));
        let (lo, hi) = w.bounds().unwrap();
        assert_eq!(lo.x, -3.0);
        assert_eq!(hi.x, 103.0);
    }

    #[test]
    fn openings_snap_to_nearest_wall() {
        let mut p = plan(1000.0, 1, Proportion::Square);
        p.add_shape(0, Shape::wall(v2(0.0, 0.0), v2(240.0, 0.0)));
        p.add_shape(0, Shape::wall(v2(0.0, 100.0), v2(240.0, 100.0)));
        let (c, ang, t) = p.floors[0].opening_slot(v2(60.0, 90.0), 24.0).unwrap();
        assert_eq!(c, v2(60.0, 100.0));
        assert_eq!(ang, 0.0);
        assert_eq!(t, DEFAULT_WALL);
        assert!(p.floors[0].opening_slot(v2(60.0, 50.0), 24.0).is_none());
    }

    #[test]
    fn rotating_about_a_pivot_moves_and_turns() {
        let mut s = Shape::boxed(Kind::Rect, v2(10.0, 0.0), v2(4.0, 2.0));
        s.rotate_about(V2::ZERO, std::f32::consts::FRAC_PI_2);
        assert!((s.center.x).abs() < 1e-4 && (s.center.y - 10.0).abs() < 1e-4);
        assert!((s.rot - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        let mut w = Shape::wall(v2(10.0, 0.0), v2(20.0, 0.0));
        w.rotate_about(V2::ZERO, std::f32::consts::PI);
        assert!((w.points[0].x + 10.0).abs() < 1e-3);
    }

    #[test]
    fn serde_round_trip_keeps_everything() {
        let mut p = plan(1800.0, 2, Proportion::Wide);
        let mut s = Shape::boxed(Kind::Rect, v2(50.0, 60.0), v2(120.0, 100.0));
        s.label = "Kitchen".into();
        s.color = 3;
        p.add_shape(0, s);
        p.add_shape(1, Shape::polygon(vec![v2(0.0, 0.0), v2(50.0, 0.0), v2(0.0, 50.0)]));
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.floors[0].shapes[0].label, "Kitchen");
        assert_eq!(back.floors[1].shapes[0].points.len(), 3);
        assert_eq!(back.footprint, p.footprint);
    }

    #[test]
    fn older_files_missing_new_fields_still_load() {
        let json = r#"{"id":1,"kind":"Rect","center":{"x":0,"y":0},"size":{"x":10,"y":10}}"#;
        let s: Shape = serde_json::from_str(json).unwrap();
        assert_eq!(s.label, "");
        assert!(!s.locked);
    }

    #[test]
    fn degenerate_shapes_are_detected() {
        assert!(Shape::boxed(Kind::Rect, V2::ZERO, v2(0.2, 50.0)).is_degenerate());
        assert!(Shape::wall(V2::ZERO, v2(0.5, 0.0)).is_degenerate());
        assert!(Shape::polygon(vec![V2::ZERO, v2(1.0, 0.0)]).is_degenerate());
        assert!(!Shape::boxed(Kind::Rect, V2::ZERO, v2(10.0, 10.0)).is_degenerate());
    }

    #[test]
    fn fresh_ids_differ() {
        assert_ne!(fresh_id(), fresh_id());
    }
}
