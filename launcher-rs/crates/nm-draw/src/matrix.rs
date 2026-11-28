use crate::geom::PointF;

/// System.Drawing.Drawing2D.Matrix on a tiny-skia Transform (sx=m11, ky=m12, kx=m21, sy=m22, tx=dx, ty=dy), floats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix(pub tiny_skia::Transform);

impl Default for Matrix {
    fn default() -> Self { Matrix::IDENTITY }
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix(tiny_skia::Transform { sx: 1.0, kx: 0.0, ky: 0.0, sy: 1.0, tx: 0.0, ty: 0.0 });
    pub fn is_identity(&self) -> bool { self.0.is_identity() }
    pub fn m11(&self) -> f32 { self.0.sx }
    pub fn m12(&self) -> f32 { self.0.ky }
    pub fn m21(&self) -> f32 { self.0.kx }
    pub fn m22(&self) -> f32 { self.0.sy }
    /// MatrixOrder.Prepend: `m` applies first.
    pub fn prepend(&mut self, m: &Matrix) { self.0 = self.0.pre_concat(m.0); }
    pub fn translate(&mut self, x: f32, y: f32) { self.0 = self.0.pre_translate(x, y); }
    pub fn scale(&mut self, sx: f32, sy: f32) { self.0 = self.0.pre_scale(sx, sy); }
    /// Degrees, clockwise on screen, as GDI+ RotateTransform (sin and cos in double, then float).
    pub fn rotate(&mut self, deg: f32) {
        let r = (deg as f64).to_radians();
        let (s, c) = (r.sin() as f32, r.cos() as f32);
        self.0 = self.0.pre_concat(tiny_skia::Transform::from_row(c, s, -s, c, 0.0, 0.0));
    }
    pub fn apply(&self, p: PointF) -> PointF {
        let mut q = [tiny_skia::Point::from_xy(p.x, p.y)];
        self.0.map_points(&mut q);
        PointF::new(q[0].x, q[0].y)
    }
    pub fn invert(&self) -> Option<Matrix> { self.0.invert().map(Matrix) }
}
