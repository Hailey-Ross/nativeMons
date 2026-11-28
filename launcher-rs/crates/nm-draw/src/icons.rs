//! Code-drawn stand-ins for the Segoe MDL2 Assets glyphs the launcher uses, drawn as our own paths in the glyph's
//! 1 em box (measured against GDI+ renders, never taken from the font), so no Microsoft font is needed anywhere.

use crate::geom::{PointF, RectangleF};
use crate::{Brush, Color, Graphics, GraphicsPath, LineJoin, Pen};

/// The icons drawn here.
pub const CODES: [char; 30] = ['\u{E724}', '\u{E8C8}', '\u{E90F}', '\u{E8AC}', '\u{E73E}', '\u{E708}', '\u{E71B}', '\u{E71C}', '\u{E721}', '\u{E74C}', '\u{E77B}', '\u{E7BA}', '\u{E8CB}', '\u{E8FB}', '\u{E8FD}', '\u{E711}', '\u{E921}', '\u{E790}', '\u{E8BD}', '\u{E72D}', '\u{E740}', '\u{E767}', '\u{E799}', '\u{E7F5}', '\u{E7F6}', '\u{E8A9}', '\u{E8B2}', '\u{E8D6}', '\u{E91B}', '\u{E895}'];

pub fn has(c: char) -> bool { CODES.contains(&c) }

/// Draws icon `c` in its em box (as the font would place the glyph). False when there is no such icon.
pub fn draw(g: &mut Graphics, c: char, em: RectangleF, color: Color) -> bool { draw_ex(g, c, em, color, em.width < 32.0) }

/// As `draw`; `hint` false draws the outline unhinted, as a glyph path (AddString) is.
pub fn draw_ex(g: &mut Graphics, c: char, em: RectangleF, color: Color, hint: bool) -> bool {
    let _p = crate::prof::span("icons");
    let s = em.width;
    // Small sizes sit on the hinted glyph grid; only the cross is grid-fitted (symmetric about a pixel center).
    let snap = hint && c == '\u{E711}';
    // A hinted em's pixel centers run em.x .. em.x + s - 1, so its geometry sits half a pixel up and left.
    let o = if hint { 0.5 } else { 0.0 };
    let p = |x: f32, y: f32| {
        let q = PointF::new(em.x - o + x * s, em.y - o + y * s);
        if snap { PointF::new(q.x.round(), q.y.round()) } else { q }
    };
    // MDL2's stroke: 1/16 em, flat ends, mitered corners, at its true width even near 1 px (the glyph's gray fringes).
    let mut pen = Pen::new(color, s / 16.0);
    pen.true_width = true;
    let fill = Brush::Solid(color);
    let circ = |cx: f32, cy: f32, r: f32| RectangleF::new(em.x - o + (cx - r) * s, em.y - o + (cy - r) * s, 2.0 * r * s, 2.0 * r * s);
    match c {
        // Cancel: two diagonals.
        '\u{E711}' => {
            g.draw_line(&pen, p(0.1415, 0.1415), p(0.8585, 0.8585));
            g.draw_line(&pen, p(0.8585, 0.1415), p(0.1415, 0.8585));
        }
        // ChromeMinimize: one bar.
        '\u{E921}' => {
            g.fill_rectangle(&fill, RectangleF::new(em.x - o, em.y - o + 0.4 * s, s, 0.1 * s));
        }
        // Color: a painter's palette with a thumb hole and five paint dots.
        '\u{E790}' => {
            let mut path = GraphicsPath::new();
            let r = 0.469;
            path.add_arc(RectangleF::new(em.x - o + (0.5 - r) * s, em.y - o + (0.5 - r) * s, 2.0 * r * s, 2.0 * r * s), 175.0, 282.0);
            path.add_bezier(p(0.443, 0.966), p(0.38, 0.865), p(0.4025, 0.84), p(0.445, 0.77));
            path.add_bezier(p(0.445, 0.77), p(0.49, 0.68), p(0.465, 0.605), p(0.39, 0.55));
            path.add_bezier(p(0.39, 0.55), p(0.28, 0.51), p(0.25, 0.545), p(0.18, 0.585));
            path.add_bezier(p(0.18, 0.585), p(0.105, 0.605), p(0.085, 0.595), p(0.032, 0.541));
            path.close_figure();
            g.draw_path(&pen.clone().round_join(), &path);
            for (x, y) in [(0.438, 0.252), (0.688, 0.314), (0.25, 0.377), (0.75, 0.564), (0.625, 0.752)] {
                g.fill_ellipse(&fill, RectangleF::new(em.x - o + (x - 0.0625) * s, em.y - o + (y - 0.0625) * s, 0.125 * s, 0.125 * s));
            }
        }
        // Message: a speech bubble with its tail at the lower left.
        '\u{E8BD}' => {
            let b = [p(0.03125, 0.09375), p(0.96875, 0.09375), p(0.96875, 0.71875), p(0.342, 0.71875), p(0.156, 0.905), p(0.156, 0.71875), p(0.03125, 0.71875)];
            g.draw_polygon(&pen, &b);
        }
        // Share: an open box with an arrow curving up out of it.
        '\u{E72D}' => {
            g.draw_lines(&pen, &[p(0.03125, 0.25), p(0.03125, 0.84575), p(0.71875, 0.84575), p(0.71875, 0.69)]);
            let mut a = GraphicsPath::new();
            a.add_lines(&[p(0.656, 0.4625), p(0.656, 0.6), p(0.93, 0.35), p(0.656, 0.075), p(0.656, 0.22)]);
            a.add_bezier(p(0.656, 0.22), p(0.41, 0.205), p(0.22, 0.39), p(0.215, 0.6325));
            a.add_bezier(p(0.215, 0.6325), p(0.315, 0.515), p(0.4675, 0.4775), p(0.656, 0.4625));
            a.close_figure();
            let mut pen = pen;
            pen.line_join = LineJoin::Miter;
            g.draw_path(&pen, &a);
        }
        // FullScreen: a diagonal with a corner bracket at each end.
        '\u{E740}' => {
            g.draw_lines(&pen, &[p(0.625, 0.09375), p(0.90625, 0.09375), p(0.90625, 0.375)]);
            g.draw_lines(&pen, &[p(0.09375, 0.625), p(0.09375, 0.90625), p(0.375, 0.90625)]);
            g.draw_line(&pen, p(0.11, 0.89), p(0.89, 0.11));
        }
        // Volume: a speaker and three sound waves.
        '\u{E767}' => {
            g.draw_polygon(&pen, &[p(0.03125, 0.34375), p(0.15625, 0.34375), p(0.34375, 0.15625), p(0.34375, 0.84375), p(0.15625, 0.65625), p(0.03125, 0.65625)]);
            for r in [0.1875f32, 0.375, 0.5625] {
                let mut a = GraphicsPath::new();
                a.add_arc(RectangleF::new(em.x - o + (0.40625 - r) * s, em.y - o + (0.5 - r) * s, 2.0 * r * s, 2.0 * r * s), -48.0, 96.0);
                g.draw_path(&pen, &a);
            }
        }
        // A portrait frame between two dotted film edges.
        '\u{E799}' => {
            g.draw_polygon(&pen, &[p(0.28125, 0.15625), p(0.71875, 0.15625), p(0.71875, 0.78125), p(0.28125, 0.78125)]);
            for y in [0.125f32, 0.25, 0.375, 0.5, 0.625, 0.75] {
                for x in [0.0f32, 0.9375] { g.fill_rectangle(&fill, RectangleF::new(em.x - o + x * s, em.y - o + y * s, 0.0625 * s, 0.0625 * s)); }
            }
            for y in [0.125f32, 0.75] {
                for x in [0.125f32, 0.8125] { g.fill_rectangle(&fill, RectangleF::new(em.x - o + x * s, em.y - o + y * s, 0.0625 * s, 0.0625 * s)); }
            }
        }
        // Speakers: a cabinet with a woofer (ring and dot) and a tweeter ring.
        '\u{E7F5}' => {
            g.draw_polygon(&pen, &[p(0.21875, 0.09375), p(0.78125, 0.09375), p(0.78125, 0.96875), p(0.21875, 0.96875)]);
            for (cy, r) in [(0.3775f32, 0.15625f32), (0.7525, 0.09375)] { g.draw_ellipse(&pen, RectangleF::new(em.x - o + (0.5 - r) * s, em.y - o + (cy - r) * s, 2.0 * r * s, 2.0 * r * s)); }
            g.fill_ellipse(&fill, RectangleF::new(em.x - o + (0.5 - 0.046875) * s, em.y - o + (0.376 - 0.046875) * s, 0.09375 * s, 0.09375 * s));
        }
        // Headphones: a band over the top and two ear cups.
        '\u{E7F6}' => {
            let mut band = GraphicsPath::new();
            band.add_line(p(0.15625, 0.78125), p(0.15625, 0.4375));
            band.add_arc(RectangleF::new(em.x - o + 0.15625 * s, em.y - o + 0.09375 * s, 0.6875 * s, 0.6875 * s), 180.0, 180.0);
            band.add_line(p(0.84375, 0.4375), p(0.84375, 0.78125));
            g.draw_path(&pen, &band);
            for right in [false, true] {
                let x = |v: f32| if right { 1.0 - v } else { v };
                let mut cup = GraphicsPath::new();
                cup.add_lines(&[p(x(0.15625), 0.53125), p(x(0.28125), 0.53125), p(x(0.28125), 0.84375), p(x(0.21875), 0.84375)]);
                let cx = if right { 0.71875 } else { 0.15625 };
                cup.add_arc(RectangleF::new(em.x - o + cx * s, em.y - o + 0.71875 * s, 0.125 * s, 0.125 * s), 90.0, if right { -90.0 } else { 90.0 });
                g.draw_path(&pen, &cup);
            }
        }
        // ViewAll: a square in four.
        '\u{E8A9}' => {
            g.draw_polygon(&pen, &[p(0.09375, 0.09375), p(0.84375, 0.09375), p(0.84375, 0.84375), p(0.09375, 0.84375)]);
            g.draw_line(&pen, p(0.46875, 0.09375), p(0.46875, 0.84375));
            g.draw_line(&pen, p(0.09375, 0.46875), p(0.84375, 0.46875));
        }
        // Movies: a film strip with sprocket holes down both sides.
        '\u{E8B2}' => {
            g.draw_polygon(&pen, &[p(0.03125, 0.15625), p(0.96875, 0.15625), p(0.96875, 0.78125), p(0.03125, 0.78125)]);
            for (y, h) in [(0.125f32, 0.1275f32), (0.3125, 0.065), (0.4375, 0.065), (0.5625, 0.065), (0.6875, 0.1275)] {
                for x in [0.125f32, 0.8125] { g.fill_rectangle(&fill, RectangleF::new(em.x - o + x * s, em.y - o + y * s, 0.0625 * s, h * s)); }
            }
        }
        // Audio: an eighth note and a beamed pair.
        '\u{E8D6}' => {
            g.draw_line(&pen, p(0.28125, 0.0625), p(0.28125, 0.62));
            let mut flag = GraphicsPath::new();
            flag.add_bezier(p(0.28125, 0.0625), p(0.3, 0.17), p(0.40625, 0.19), p(0.40625, 0.31));
            g.draw_path(&pen, &flag);
            g.draw_ellipse(&pen, RectangleF::new(em.x - o + 0.08 * s, em.y - o + 0.51 * s, 0.205 * s, 0.135 * s));
            g.draw_line(&pen, p(0.53125, 0.4), p(0.53125, 0.84));
            g.draw_line(&pen, p(0.90625, 0.31), p(0.90625, 0.78));
            g.draw_line(&pen, p(0.53125, 0.4), p(0.90625, 0.31));
            g.draw_ellipse(&pen, RectangleF::new(em.x - o + 0.35 * s, em.y - o + 0.75 * s, 0.18 * s, 0.155 * s));
            g.draw_ellipse(&pen, RectangleF::new(em.x - o + 0.72 * s, em.y - o + 0.69 * s, 0.19 * s, 0.145 * s));
        }
        // Photo: a picture frame with an inner mat.
        '\u{E91B}' => {
            g.draw_polygon(&pen, &[p(0.03125, 0.15625), p(0.96875, 0.15625), p(0.96875, 0.84375), p(0.03125, 0.84375)]);
            g.draw_polygon(&pen, &[p(0.15625, 0.28125), p(0.84375, 0.28125), p(0.84375, 0.71875), p(0.15625, 0.71875)]);
        }
        // Sync: two arcs of a circle, each ending in an arrowhead.
        '\u{E895}' => {
            let r = 0.46875f32;
            let circle = RectangleF::new(em.x - o + (0.5 - r) * s, em.y - o + (0.5 - r) * s, 2.0 * r * s, 2.0 * r * s);
            for (start, sweep) in [(195.0f32, 145.0f32), (15.0, 145.0)] {
                let mut a = GraphicsPath::new();
                a.add_arc(circle, start, sweep);
                g.draw_path(&pen, &a);
            }
            g.draw_lines(&pen, &[p(0.75, 0.34375), p(0.96875, 0.34375), p(0.96875, 0.125)]);
            g.draw_lines(&pen, &[p(0.25, 0.65625), p(0.03125, 0.65625), p(0.03125, 0.875)]);
        }
        // QuietHours: a crescent moon, the outer circle cut by a second one.
        '\u{E708}' => {
            let mut m = GraphicsPath::new();
            m.add_arc(circ(0.4375, 0.5, 0.46875), -75.10, 217.79);
            m.add_arc(circ(0.054, 0.2433, 0.5409), 88.87, -110.15);
            m.close_figure();
            g.draw_path(&pen, &m);
        }
        // Link: two open stadiums, each broken where the other passes.
        '\u{E71B}' => {
            for flip in [false, true] {
                let q = |x: f32, y: f32| if flip { p(1.0 - x, 1.0 - y) } else { p(x, y) };
                let a0 = if flip { 180.0 } else { 0.0 };
                let mut l = GraphicsPath::new();
                l.add_line(q(0.4375, 0.59375), q(0.46875, 0.59375));
                let (r1, r2) = if flip { (circ(0.53125, 0.59375, 0.1875), circ(0.78125, 0.59375, 0.1875)) } else { (circ(0.46875, 0.40625, 0.1875), circ(0.21875, 0.40625, 0.1875)) };
                l.add_arc(r1, 90.0 + a0, -180.0);
                l.add_arc(r2, 270.0 + a0, -180.0);
                l.add_line(q(0.21875, 0.59375), q(0.25, 0.59375));
                g.draw_path(&pen, &l);
            }
        }
        // Filter: a funnel.
        '\u{E71C}' => {
            g.draw_polygon(&pen, &[p(0.03125, 0.09375), p(0.96875, 0.09375), p(0.96875, 0.15625), p(0.59375, 0.53125), p(0.59375, 0.90625), p(0.40625, 0.90625), p(0.40625, 0.53125), p(0.03125, 0.15625)]);
        }
        // Search: a lens and its handle.
        '\u{E721}' => {
            g.draw_ellipse(&pen, circ(0.65625, 0.34375, 0.3125));
            let mut h = pen.clone();
            h.end_cap = crate::LineCap::Round;
            g.draw_line(&h, p(0.4353, 0.5647), p(0.03125, 0.96875));
        }
        // OEM: three squares of a grid and a fourth turned on its corner.
        '\u{E74C}' => {
            g.draw_polygon(&pen, &[p(0.09375, 0.15625), p(0.46875, 0.15625), p(0.46875, 0.53125), p(0.84375, 0.53125), p(0.84375, 0.90625), p(0.09375, 0.90625)]);
            g.draw_line(&pen, p(0.09375, 0.53125), p(0.46875, 0.53125));
            g.draw_line(&pen, p(0.46875, 0.53125), p(0.46875, 0.90625));
            let (cx, cy, d) = (0.71875, 0.28125, 0.24375);
            g.draw_polygon(&pen, &[p(cx, cy - d), p(cx + d, cy), p(cx, cy + d), p(cx - d, cy)]);
        }
        // Contact: a head and shoulders.
        '\u{E77B}' => {
            g.draw_ellipse(&pen, circ(0.5, 0.3125, 0.28125));
            g.draw_arc(&pen, circ(0.5, 1.0, 0.40625), 180.0, 180.0);
        }
        // Warning: a triangle with an exclamation mark.
        '\u{E7BA}' => {
            g.draw_polygon(&pen, &[p(0.46875, 0.0703), p(0.8867, 0.90625), p(0.0508, 0.90625)]);
            g.fill_rectangle(&fill, RectangleF::new(em.x - o + 0.4375 * s, em.y - o + 0.375 * s, 0.0625 * s, 0.3125 * s));
            g.fill_rectangle(&fill, RectangleF::new(em.x - o + 0.4375 * s, em.y - o + 0.75 * s, 0.0625 * s, 0.0625 * s));
        }
        // Sort: an arrow up beside an arrow down.
        '\u{E8CB}' => {
            g.draw_line(&pen, p(0.28125, 0.06), p(0.28125, 1.0));
            g.draw_lines(&pen, &[p(0.0625, 0.265625), p(0.28125, 0.046875), p(0.5, 0.265625)]);
            g.draw_line(&pen, p(0.71875, 0.0), p(0.71875, 0.94));
            g.draw_lines(&pen, &[p(0.9375, 0.734375), p(0.71875, 0.953125), p(0.5, 0.734375)]);
        }
        // CheckMark: a small check.
        '\u{E73E}' => {
            g.draw_lines(&pen, &[p(0.125, 0.5), p(0.375, 0.75), p(0.875, 0.25)]);
        }
        // Accept: a check mark.
        '\u{E8FB}' => {
            g.draw_lines(&pen, &[p(0.03125, 0.53125), p(0.3125, 0.8125), p(0.96875, 0.15625)]);
        }
        // BulletedList: four dots and four lines.
        '\u{E8FD}' => {
            for y in [0.1875f32, 0.375, 0.5625, 0.75] {
                g.fill_rectangle(&fill, RectangleF::new(em.x - o, em.y - o + y * s, 0.0625 * s, 0.0625 * s));
                g.fill_rectangle(&fill, RectangleF::new(em.x - o + 0.1875 * s, em.y - o + y * s, 0.8125 * s, 0.0625 * s));
            }
        }
        // Send, measured against GDI+ at 400 px (t36): a paper plane pointing right, its fold along the middle.
        '\u{E724}' => {
            g.draw_polygon(&pen, &[p(0.0547, 0.0938), p(0.8926, 0.46875), p(0.0547, 0.84375), p(0.1484, 0.46875)]);
            g.draw_line(&pen, p(0.1484, 0.46875), p(0.85, 0.46875));
        }
        // Copy (measured, t36): a dog-eared page in front of another whose corner is cut.
        '\u{E8C8}' => {
            g.draw_lines(&pen, &[p(0.3125, 0.78125), p(0.09375, 0.78125), p(0.09375, 0.03125), p(0.46875, 0.03125), p(0.65625, 0.21875)]);
            g.draw_polygon(&pen, &[p(0.34375, 0.21875), p(0.71875, 0.21875), p(0.90625, 0.40625), p(0.90625, 0.96875), p(0.34375, 0.96875)]);
            g.draw_lines(&pen, &[p(0.71875, 0.21875), p(0.71875, 0.40625), p(0.90625, 0.40625)]);
        }
        // Repair (measured, t36): a wrench, its round-ended handle into a ring with a square jaw.
        '\u{E90F}' => {
            let (cx, cy, r) = (0.71875, 0.28125, 0.25);
            let (ex, ey, er) = (0.1638, 0.8362, 0.13258);
            let head = RectangleF::new(em.x - o + (cx - r) * s, em.y - o + (cy - r) * s, 2.0 * r * s, 2.0 * r * s);
            // The head with its jaw, then the handle: two figures, so the handle meets the ring in a notch, as the glyph does.
            let mut path = GraphicsPath::new();
            path.add_arc(head, 167.0, 132.6);
            path.add_lines(&[p(0.8423, 0.0640), p(0.65625, 0.25), p(0.75, 0.34375), p(0.9360, 0.1577)]);
            path.add_arc(head, 330.4, 132.5);
            path.start_figure();
            path.add_line(p(0.6627, 0.5248), p(ex + 0.09375, ey + 0.09375));
            path.add_arc(circ(ex, ey, er), 45.0, 180.0);
            path.add_line(p(ex - 0.09375, ey - 0.09375), p(0.4752, 0.3373));
            g.draw_path(&pen, &path);
        }
        // Rename (measured, t36): a text box open at its right, a bar of text in it, and a text cursor.
        '\u{E8AC}' => {
            g.draw_lines(&pen, &[p(0.625, 0.34375), p(0.03125, 0.34375), p(0.03125, 0.65625), p(0.625, 0.65625)]);
            g.draw_lines(&pen, &[p(0.8125, 0.34375), p(0.96875, 0.34375), p(0.96875, 0.65625), p(0.8125, 0.65625)]);
            g.fill_rectangle(&fill, RectangleF::new(em.x - o + 0.125 * s, em.y - o + 0.4375 * s, 0.375 * s, 0.125 * s));
            let mut path = GraphicsPath::new();
            for (x0, y0, k) in [(0.5625f32, 0.15625f32, 1.0f32), (0.875, 0.15625, -1.0)] {
                // A serif curving from its tip into the stem, top and bottom alike.
                path.start_figure();
                path.add_bezier(p(x0, y0), p(0.71875 - k * 0.06, y0), p(0.71875, 0.2), p(0.71875, 0.28));
                path.add_line(p(0.71875, 0.28), p(0.71875, 0.72));
                path.add_bezier(p(0.71875, 0.72), p(0.71875, 0.8), p(0.71875 - k * 0.06, 1.0 - y0), p(x0, 1.0 - y0));
            }
            g.draw_path(&pen, &path);
        }
        _ => return false,
    }
    true
}
