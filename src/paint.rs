//! The egui backend for `render::Cmd` lists.

use crate::geom::{V2, is_convex, triangulate};
use crate::render::{Cmd, Weight};
use crate::theme;
use eframe::egui::{
    self, Color32, Mesh, Painter, Pos2, Shape, Stroke, epaint::{PathShape, PathStroke, TextShape, Vertex}, pos2, vec2,
};

fn p2(origin: Pos2, v: V2) -> Pos2 {
    pos2(origin.x + v.x, origin.y + v.y)
}

/// Paint commands whose coordinates are relative to `origin`.
pub fn paint(painter: &Painter, origin: Pos2, cmds: &[Cmd]) {
    for cmd in cmds {
        match cmd {
            Cmd::Poly { pts, closed, fill, stroke, dash } => {
                if pts.len() < 2 {
                    continue;
                }
                let screen: Vec<Pos2> = pts.iter().map(|&v| p2(origin, v)).collect();
                if let (Some(f), true) = (fill, *closed && pts.len() >= 3) {
                    fill_polygon(painter, &screen, pts, *f);
                }
                if let Some((color, width)) = stroke {
                    let st = Stroke::new(*width, *color);
                    let mut path = screen.clone();
                    if *closed {
                        path.push(screen[0]);
                    }
                    match dash {
                        Some((d, g)) => {
                            painter.extend(Shape::dashed_line(&path, st, *d, *g));
                        }
                        None => {
                            painter.add(Shape::Path(PathShape {
                                points: path,
                                closed: false,
                                fill: Color32::TRANSPARENT,
                                stroke: PathStroke::from(st),
                            }));
                        }
                    }
                }
            }
            Cmd::Text { at, text, size, color, weight, angle, halo, left } => {
                let font = match weight {
                    Weight::Regular => theme::body(*size),
                    Weight::Medium => theme::medium(*size),
                    Weight::Semibold => theme::semibold(*size),
                };
                let galley = painter.layout_no_wrap(text.clone(), font, *color);
                let half = if *left { vec2(0.0, galley.size().y / 2.0) } else { galley.size() / 2.0 };
                let (s, c) = angle.sin_cos();
                let off = vec2(half.x * c - half.y * s, half.x * s + half.y * c);
                let pos = p2(origin, *at) - off;
                if let Some(h) = halo {
                    let hg = painter.layout_no_wrap(text.clone(), match weight {
                        Weight::Regular => theme::body(*size),
                        Weight::Medium => theme::medium(*size),
                        Weight::Semibold => theme::semibold(*size),
                    }, *h);
                    for k in 0..8 {
                        let a = k as f32 * std::f32::consts::FRAC_PI_4;
                        let d = vec2(a.cos(), a.sin()) * 1.8;
                        painter.add(TextShape::new(pos + d, hg.clone(), *h).with_angle(*angle));
                    }
                }
                painter.add(TextShape::new(pos, galley, *color).with_angle(*angle));
            }
        }
    }
}

fn fill_polygon(painter: &Painter, screen: &[Pos2], world: &[V2], color: Color32) {
    if is_convex(world) {
        painter.add(Shape::convex_polygon(screen.to_vec(), color, Stroke::NONE));
        return;
    }
    // Concave: triangulate ourselves so L-shaped rooms fill correctly.
    let tris = triangulate(world);
    if tris.is_empty() {
        return;
    }
    let mut mesh = Mesh::default();
    for p in screen {
        mesh.vertices.push(Vertex { pos: *p, uv: egui::epaint::WHITE_UV, color });
    }
    for t in tris {
        mesh.indices.extend(t.iter().map(|&i| i as u32));
    }
    painter.add(Shape::mesh(mesh));
}
