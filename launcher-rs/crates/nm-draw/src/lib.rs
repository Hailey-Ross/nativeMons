//! A System.Drawing-shaped 2D API that reproduces GDI+'s pixels: tiny-skia (pixmaps, PNG, transforms, fallback
//! raster) and cosmic-text (fonts, glyphs) underneath, our GDI+ clone for every fill, stroke, clip and blend.

pub mod bicubic;
pub mod bitmap;
pub mod brush;
pub mod color;
pub mod flatten;
pub mod geom;
pub mod graphics;
pub mod icons;
pub mod matrix;
pub mod net_random;
pub mod path;
pub mod prof;
pub mod raster;
pub mod stroke;
pub mod text;

pub use bitmap::Bitmap;
pub use brush::{Brush, EllipseGradient, LinearGradient, WrapMode};
pub use color::Color;
pub use geom::{PointF, RectangleF, SizeF};
pub use graphics::{DashStyle, Graphics, GraphicsState, InterpolationMode, Pen, PixelOffsetMode, SmoothingMode};
pub use matrix::Matrix;
pub use net_random::NetRandom;
pub use path::GraphicsPath;
pub use raster::FillMode;
pub use stroke::{LineCap, LineJoin};
pub use text::{Font, FontFace, StringAlignment, StringFormat};
