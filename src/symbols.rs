//! Plan-view drawings of furniture and fixtures, in a unit box:
//! x and y each run -0.5..0.5 across the item's width and depth (the back of
//! the item is at -y). They are scaled to whatever size the user gives them.

use crate::geom::{V2, v2};
use crate::model::Sym;
use std::f32::consts::TAU;

pub struct Prim {
    pub pts: Vec<V2>,
    pub closed: bool,
    /// Filled with the furniture color (outlines and details are stroke-only).
    pub fill: bool,
    /// Drawn in the lighter detail stroke.
    pub soft: bool,
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, fill: bool, soft: bool) -> Prim {
    Prim {
        pts: vec![v2(x0, y0), v2(x1, y0), v2(x1, y1), v2(x0, y1)],
        closed: true,
        fill,
        soft,
    }
}

fn line(a: (f32, f32), b: (f32, f32)) -> Prim {
    Prim { pts: vec![v2(a.0, a.1), v2(b.0, b.1)], closed: false, fill: false, soft: true }
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, fill: bool, soft: bool) -> Prim {
    let n = 28;
    Prim {
        pts: (0..n)
            .map(|i| {
                let a = TAU * i as f32 / n as f32;
                v2(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect(),
        closed: true,
        fill,
        soft,
    }
}

pub fn prims(sym: Sym) -> Vec<Prim> {
    match sym {
        Sym::Bed => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            rect(-0.46, -0.47, -0.04, -0.29, true, true),
            rect(0.04, -0.47, 0.46, -0.29, true, true),
            line((-0.5, -0.12), (0.5, -0.12)),
            line((-0.5, -0.06), (0.5, -0.06)),
        ],
        Sym::Sofa => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            rect(-0.5, -0.5, 0.5, -0.24, true, true),
            rect(-0.5, -0.5, -0.38, 0.5, true, true),
            rect(0.38, -0.5, 0.5, 0.5, true, true),
            line((-0.13, -0.24), (-0.13, 0.5)),
            line((0.13, -0.24), (0.13, 0.5)),
        ],
        Sym::Armchair => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            rect(-0.5, -0.5, 0.5, -0.24, true, true),
            rect(-0.5, -0.5, -0.34, 0.5, true, true),
            rect(0.34, -0.5, 0.5, 0.5, true, true),
        ],
        Sym::DiningTable => {
            let mut v = vec![rect(-0.43, -0.33, 0.43, 0.33, true, false)];
            for cx in [-0.27, 0.0, 0.27] {
                v.push(rect(cx - 0.11, -0.5, cx + 0.11, -0.36, true, true));
                v.push(rect(cx - 0.11, 0.36, cx + 0.11, 0.5, true, true));
            }
            v
        }
        Sym::RoundTable => {
            let mut v = vec![ellipse(0.0, 0.0, 0.34, 0.34, true, false)];
            for i in 0..6 {
                let a = TAU * i as f32 / 6.0;
                v.push(ellipse(0.425 * a.cos(), 0.425 * a.sin(), 0.075, 0.075, true, true));
            }
            v
        }
        Sym::Desk => vec![
            rect(-0.5, -0.5, 0.5, 0.12, true, false),
            rect(-0.2, -0.44, 0.2, -0.38, false, true),
            ellipse(0.0, 0.36, 0.16, 0.14, true, true),
        ],
        Sym::Wardrobe => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            line((0.0, -0.5), (0.0, 0.5)),
            line((-0.5, -0.5), (0.0, 0.5)),
            line((0.5, -0.5), (0.0, 0.5)),
        ],
        Sym::Toilet => vec![
            rect(-0.36, -0.5, 0.36, -0.2, true, false),
            ellipse(0.0, 0.14, 0.4, 0.36, true, false),
            ellipse(0.0, 0.14, 0.26, 0.24, false, true),
        ],
        Sym::Vanity => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            ellipse(0.0, 0.05, 0.3, 0.3, false, true),
            ellipse(0.0, -0.32, 0.04, 0.05, false, true),
        ],
        Sym::Bathtub => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            rect(-0.44, -0.4, 0.44, 0.4, false, true),
            ellipse(-0.36, 0.0, 0.035, 0.07, false, true),
        ],
        Sym::Shower => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            line((-0.5, -0.5), (0.5, 0.5)),
            line((0.5, -0.5), (-0.5, 0.5)),
            ellipse(0.0, 0.0, 0.07, 0.07, true, true),
        ],
        Sym::Counter => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            line((-0.25, -0.5), (-0.25, 0.5)),
            line((0.0, -0.5), (0.0, 0.5)),
            line((0.25, -0.5), (0.25, 0.5)),
        ],
        Sym::Stove => {
            let mut v = vec![rect(-0.5, -0.5, 0.5, 0.5, true, false)];
            for (x, y) in [(-0.25, -0.22), (0.25, -0.22), (-0.25, 0.2), (0.25, 0.2)] {
                v.push(ellipse(x, y, 0.15, 0.15, false, true));
                v.push(ellipse(x, y, 0.06, 0.06, false, true));
            }
            v
        }
        Sym::Fridge => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            line((-0.5, -0.12), (0.5, -0.12)),
            rect(0.36, -0.06, 0.42, 0.32, false, true),
            rect(0.36, -0.4, 0.42, -0.2, false, true),
        ],
        Sym::WasherDryer => vec![
            rect(-0.5, -0.5, 0.0, 0.5, true, false),
            rect(0.0, -0.5, 0.5, 0.5, true, false),
            ellipse(-0.25, 0.08, 0.18, 0.3, false, true),
            ellipse(0.25, 0.08, 0.18, 0.3, false, true),
        ],
        Sym::Fireplace => vec![
            rect(-0.5, -0.5, 0.5, 0.5, true, false),
            rect(-0.34, -0.2, 0.34, 0.5, false, true),
            line((-0.34, -0.2), (-0.2, 0.5)),
            line((0.34, -0.2), (0.2, 0.5)),
        ],
        Sym::Stairs => {
            let mut v = vec![rect(-0.5, -0.5, 0.5, 0.5, true, false)];
            for i in 1..11 {
                let y = -0.5 + i as f32 / 11.0;
                v.push(line((-0.5, y), (0.5, y)));
            }
            // Arrow up the middle, pointing toward the top (-y).
            v.push(line((0.0, 0.42), (0.0, -0.38)));
            v.push(Prim {
                pts: vec![v2(-0.14, -0.2), v2(0.0, -0.42), v2(0.14, -0.2)],
                closed: false,
                fill: false,
                soft: false,
            });
            v
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_draws_inside_its_box() {
        for s in Sym::ALL {
            let p = prims(s);
            assert!(!p.is_empty(), "{s:?} has no drawing");
            for prim in &p {
                assert!(prim.pts.len() >= 2);
                for pt in &prim.pts {
                    assert!(
                        pt.x.abs() <= 0.5001 && pt.y.abs() <= 0.5001,
                        "{s:?} point {pt:?} outside the unit box"
                    );
                }
            }
        }
    }

    #[test]
    fn every_symbol_has_a_filled_body() {
        for s in Sym::ALL {
            assert!(prims(s).iter().any(|p| p.fill), "{s:?} has no body");
        }
    }
}
