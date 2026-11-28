/// System.Drawing.Color: straight (not premultiplied) ARGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Color.Empty: compares unequal to every real color.
    pub empty: bool,
}

impl Color {
    pub const fn from_argb(a: i32, r: i32, g: i32, b: i32) -> Color {
        Color { a: a as u8, r: r as u8, g: g as u8, b: b as u8, empty: false }
    }
    pub const fn rgb(r: i32, g: i32, b: i32) -> Color { Color::from_argb(255, r, g, b) }
    /// Color.FromArgb(alpha, baseColor).
    pub const fn with_alpha(a: i32, c: Color) -> Color { Color::from_argb(a, c.r as i32, c.g as i32, c.b as i32) }
    pub const EMPTY: Color = Color { a: 0, r: 0, g: 0, b: 0, empty: true };
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub fn is_empty(&self) -> bool { self.empty }
    pub fn to_argb(&self) -> i32 { ((self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32) as i32 }

    /// ColorTranslator.FromHtml for "#RRGGBB".
    pub fn from_html(s: &str) -> Color {
        let h = s.trim_start_matches('#');
        let v = u32::from_str_radix(h, 16).unwrap_or(0);
        Color::rgb((v >> 16 & 255) as i32, (v >> 8 & 255) as i32, (v & 255) as i32)
    }

    /// Color.GetHue, GetSaturation, GetBrightness, as .NET computes them (floats).
    pub fn get_hue(&self) -> f32 {
        if self.r == self.g && self.g == self.b { return 0.0; }
        let (r, g, b) = (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let mut hue = if r == max { (g - b) / delta } else if g == max { 2.0 + (b - r) / delta } else { 4.0 + (r - g) / delta };
        hue *= 60.0;
        if hue < 0.0 { hue += 360.0; }
        hue
    }
    pub fn get_saturation(&self) -> f32 {
        let (r, g, b) = (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max == min { return 0.0; }
        let l = (max + min) / 2.0;
        if l <= 0.5 { (max - min) / (max + min) } else { (max - min) / (2.0 - max - min) }
    }
}
