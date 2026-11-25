//! Small 2D vector and polygon helpers. World units are inches, +y points down.

use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;
use std::ops::{Add, AddAssign, Mul, Neg, Sub};

#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);

    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn rotated(self, a: f32) -> V2 {
        let (s, c) = a.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        self + (o - self) * t
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn normalized(self) -> V2 {
        let l = self.len();
        if l < 1e-6 { V2::ZERO } else { self * (1.0 / l) }
    }
    pub fn perp(self) -> V2 {
        v2(-self.y, self.x)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        *self = *self + o;
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}

pub fn snap(v: f32, step: f32) -> f32 {
    if step <= 0.0 { v } else { (v / step).round() * step }
}

pub fn snap_v(p: V2, step: f32) -> V2 {
    v2(snap(p.x, step), snap(p.y, step))
}

/// Corners of a rotated box, clockwise from top-left (in the box's own frame).
pub fn box_corners(center: V2, size: V2, rot: f32) -> [V2; 4] {
    let (hw, hh) = (size.x / 2.0, size.y / 2.0);
    [v2(-hw, -hh), v2(hw, -hh), v2(hw, hh), v2(-hw, hh)].map(|c| center + c.rotated(rot))
}

pub fn ellipse_points(center: V2, size: V2, rot: f32, n: usize) -> Vec<V2> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f32 / n as f32;
            center + v2(a.cos() * size.x / 2.0, a.sin() * size.y / 2.0).rotated(rot)
        })
        .collect()
}

/// Isosceles triangle with its apex at the top-centre of the box.
pub fn triangle_points(center: V2, size: V2, rot: f32) -> [V2; 3] {
    let (hw, hh) = (size.x / 2.0, size.y / 2.0);
    [v2(0.0, -hh), v2(hw, hh), v2(-hw, hh)].map(|c| center + c.rotated(rot))
}

pub fn polygon_area(pts: &[V2]) -> f32 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s.abs() / 2.0
}

pub fn polygon_centroid(pts: &[V2]) -> V2 {
    if pts.is_empty() {
        return V2::ZERO;
    }
    let mut a2 = 0.0;
    let mut c = V2::ZERO;
    for i in 0..pts.len() {
        let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
        let cr = p.x * q.y - q.x * p.y;
        a2 += cr;
        c += v2((p.x + q.x) * cr, (p.y + q.y) * cr);
    }
    if a2.abs() < 1e-3 {
        // Degenerate: use the average of the points.
        let n = pts.len() as f32;
        return v2(
            pts.iter().map(|p| p.x).sum::<f32>() / n,
            pts.iter().map(|p| p.y).sum::<f32>() / n,
        );
    }
    c * (1.0 / (3.0 * a2))
}

pub fn point_in_polygon(p: V2, pts: &[V2]) -> bool {
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Closest point to `p` on segment `a`-`b`, and the parameter `t` in 0..=1.
pub fn closest_on_segment(p: V2, a: V2, b: V2) -> (V2, f32) {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-6 {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (a + ab * t, t)
}

pub fn dist_to_segment(p: V2, a: V2, b: V2) -> f32 {
    p.dist(closest_on_segment(p, a, b).0)
}

pub fn bounds(pts: &[V2]) -> Option<(V2, V2)> {
    let first = *pts.first()?;
    Some(pts.iter().fold((first, first), |(lo, hi), p| {
        (v2(lo.x.min(p.x), lo.y.min(p.y)), v2(hi.x.max(p.x), hi.y.max(p.y)))
    }))
}

/// Normalise an angle to (-π, π].
pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a % TAU;
    if a > std::f32::consts::PI {
        a -= TAU;
    } else if a <= -std::f32::consts::PI {
        a += TAU;
    }
    a
}


/// Signed area (positive when the points run clockwise on screen, y down).
pub fn signed_area(pts: &[V2]) -> f32 {
    let mut s = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

pub fn is_convex(pts: &[V2]) -> bool {
    if pts.len() < 4 {
        return true;
    }
    let mut sign = 0.0f32;
    for i in 0..pts.len() {
        let (a, b, c) = (pts[i], pts[(i + 1) % pts.len()], pts[(i + 2) % pts.len()]);
        let cr = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        if cr.abs() < 1e-4 {
            continue;
        }
        if sign == 0.0 {
            sign = cr.signum();
        } else if cr.signum() != sign {
            return false;
        }
    }
    true
}

/// Ear-clipping triangulation of a simple polygon. Returns index triples.
/// Degenerate input yields fewer (possibly zero) triangles rather than panicking.
pub fn triangulate(pts: &[V2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let ccw = signed_area(pts) > 0.0;
    let mut idx: Vec<usize> = if ccw { (0..n).collect() } else { (0..n).rev().collect() };
    let cross = |a: V2, b: V2, c: V2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let in_tri = |p: V2, a: V2, b: V2, c: V2| {
        cross(a, b, p) >= -1e-6 && cross(b, c, p) >= -1e-6 && cross(c, a, p) >= -1e-6
    };
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 1e-6 {
                continue; // reflex or flat
            }
            let blocked = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && in_tri(pts[j], a, b, c));
            if blocked {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // Self-intersecting or fully degenerate: drop a vertex and carry on.
            idx.remove(0);
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn area_of_square_and_triangle() {
        let sq = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)];
        assert!((polygon_area(&sq) - 100.0).abs() < 1e-3);
        let tri = triangle_points(v2(0.0, 0.0), v2(10.0, 8.0), 0.0);
        assert!((polygon_area(&tri) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn centroid_of_rectangle() {
        let r = [v2(0.0, 0.0), v2(20.0, 0.0), v2(20.0, 10.0), v2(0.0, 10.0)];
        let c = polygon_centroid(&r);
        assert!((c.x - 10.0).abs() < 1e-3 && (c.y - 5.0).abs() < 1e-3);
    }

    #[test]
    fn point_in_concave_polygon() {
        // An L shape.
        let l = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 4.0), v2(4.0, 4.0), v2(4.0, 10.0), v2(0.0, 10.0)];
        assert!(point_in_polygon(v2(2.0, 8.0), &l));
        assert!(point_in_polygon(v2(8.0, 2.0), &l));
        assert!(!point_in_polygon(v2(8.0, 8.0), &l));
    }

    #[test]
    fn rotation_and_segments() {
        let r = v2(1.0, 0.0).rotated(FRAC_PI_2);
        assert!((r.x).abs() < 1e-5 && (r.y - 1.0).abs() < 1e-5);
        let (p, t) = closest_on_segment(v2(5.0, 3.0), v2(0.0, 0.0), v2(10.0, 0.0));
        assert!((p.x - 5.0).abs() < 1e-5 && (t - 0.5).abs() < 1e-5);
        assert!((dist_to_segment(v2(-3.0, 4.0), v2(0.0, 0.0), v2(10.0, 0.0)) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn ellipse_area_close_to_pi_ab() {
        let pts = ellipse_points(v2(0.0, 0.0), v2(20.0, 10.0), 0.3, 96);
        let want = std::f32::consts::PI * 10.0 * 5.0;
        assert!((polygon_area(&pts) - want).abs() / want < 0.01);
    }

    #[test]
    fn angle_wrapping() {
        assert!((wrap_angle(3.0 * std::f32::consts::PI) - std::f32::consts::PI).abs() < 1e-4);
        assert!(wrap_angle(0.5).abs() - 0.5 < 1e-6);
    }

    #[test]
    fn snapping() {
        assert_eq!(snap(7.0, 6.0), 6.0);
        assert_eq!(snap(9.1, 6.0), 12.0);
        assert_eq!(snap(9.1, 0.0), 9.1);
    }
    #[test]
    fn triangulation_covers_the_polygon_area() {
        let l = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 4.0), v2(4.0, 4.0), v2(4.0, 10.0), v2(0.0, 10.0)];
        for pts in [l.to_vec(), l.iter().rev().copied().collect::<Vec<_>>()] {
            let tris = triangulate(&pts);
            assert_eq!(tris.len(), 4);
            let total: f32 = tris.iter().map(|t| polygon_area(&[pts[t[0]], pts[t[1]], pts[t[2]]])).sum();
            assert!((total - polygon_area(&pts)).abs() < 1e-3);
        }
    }

    #[test]
    fn triangulation_survives_degenerate_input() {
        assert!(triangulate(&[]).is_empty());
        assert!(triangulate(&[v2(0.0, 0.0), v2(1.0, 1.0)]).is_empty());
        let line = [v2(0.0, 0.0), v2(5.0, 0.0), v2(10.0, 0.0)];
        let _ = triangulate(&line); // must not panic or hang
        let bow = [v2(0.0, 0.0), v2(10.0, 10.0), v2(10.0, 0.0), v2(0.0, 10.0)];
        let _ = triangulate(&bow);
    }

    #[test]
    fn convexity() {
        let sq = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 10.0)];
        assert!(is_convex(&sq));
        let l = [v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 4.0), v2(4.0, 4.0), v2(4.0, 10.0), v2(0.0, 10.0)];
        assert!(!is_convex(&l));
    }
}
