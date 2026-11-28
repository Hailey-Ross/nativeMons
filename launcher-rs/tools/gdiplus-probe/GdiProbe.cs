// Test only, never ships: runs a drawing script through GDI+ and writes each canvas as raw ARGB, so nm-draw can
// run the same script and compare. Script format: launcher-rs/tools/gdiplus-probe/README in the step 1 note.
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Linq;

static class GdiProbe {
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;
    static Bitmap bmp, img; static Graphics g; static Brush brush = Brushes.Black; static Pen pen = new Pen(Color.Black, 1);
    static readonly Stack<GraphicsState> saved = new Stack<GraphicsState>();

    static int Main(string[] argv) {
        if (argv.Length < 2) { Console.Error.WriteLine("GdiProbe <script> <out folder>"); return 1; }
        Directory.CreateDirectory(argv[1]);
        int line = 0;
        foreach (var raw in File.ReadAllLines(argv[0])) {
            line++;
            var l = raw.Trim();
            if (l == "" || l.StartsWith("#")) continue;
            var a = l.Split(new[] { ' ', '\t' }, StringSplitOptions.RemoveEmptyEntries);
            try { Run(a, argv[1]); } catch (Exception e) { Console.Error.WriteLine("line " + line + ": " + e.Message); return 2; }
        }
        return 0;
    }
    static float F(string s) { return float.Parse(s, Inv); }
    static Color C(string[] a, int i) { return Color.FromArgb(int.Parse(a[i]), int.Parse(a[i + 1]), int.Parse(a[i + 2]), int.Parse(a[i + 3])); }
    static PointF[] Pts(string[] a, int i) { int n = int.Parse(a[i]); var p = new PointF[n]; for (int k = 0; k < n; k++) p[k] = new PointF(F(a[i + 1 + 2 * k]), F(a[i + 2 + 2 * k])); return p; }
    static RectangleF R(string[] a, int i) { return new RectangleF(F(a[i]), F(a[i + 1]), F(a[i + 2]), F(a[i + 3])); }
    // The launcher's Round().
    static GraphicsPath Round(RectangleF r, float rad) {
        var path = new GraphicsPath();
        float d = Math.Min(rad * 2, Math.Min(r.Width, r.Height));
        if (d <= 0) { path.AddRectangle(r); return path; }
        path.AddArc(r.X, r.Y, d, d, 180, 90); path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90); path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }
    static readonly Dictionary<string, FontFamily> families = new Dictionary<string, FontFamily>();
    static readonly List<System.Drawing.Text.PrivateFontCollection> keep = new List<System.Drawing.Text.PrivateFontCollection>();
    static FontFamily Family(string file) {
        FontFamily f;
        if (families.TryGetValue(file, out f)) return f;
        // @<file>: a font file in the folder NM_FONTS names (paths with spaces can't go in a script line).
        string key = file;
        if (file.StartsWith("@")) file = Path.Combine(Environment.GetEnvironmentVariable("NM_FONTS") ?? "", file.Substring(1));
        if (!file.EndsWith(".ttf")) return families[key] = new FontFamily(file.Replace('_', ' '));
        var c = new System.Drawing.Text.PrivateFontCollection(); c.AddFontFile(file); keep.Add(c);
        return families[key] = c.Families[0];
    }
    static LineCap Cap(string s) { return s == "round" ? LineCap.Round : s == "square" ? LineCap.Square : LineCap.Flat; }
    static LineJoin Join(string s) { return s == "round" ? LineJoin.Round : s == "bevel" ? LineJoin.Bevel : LineJoin.Miter; }

    static void Run(string[] a, string outDir) {
        switch (a[0]) {
            case "canvas":
                if (g != null) g.Dispose(); if (bmp != null) bmp.Dispose();
                bmp = new Bitmap(int.Parse(a[1]), int.Parse(a[2]), a.Length > 3 && a[3] == "parg" ? PixelFormat.Format32bppPArgb : PixelFormat.Format32bppArgb);
                g = Graphics.FromImage(bmp); saved.Clear(); break;
            case "clear": g.Clear(C(a, 1)); break;
            case "smooth": g.SmoothingMode = a[1] == "1" ? SmoothingMode.AntiAlias : a[1] == "8x8" ? (SmoothingMode)6 : SmoothingMode.None; break;
            case "pixoff": g.PixelOffsetMode = a[1] == "half" ? PixelOffsetMode.Half : PixelOffsetMode.None; break;
            case "compq": g.CompositingQuality = a[1] == "gamma" ? CompositingQuality.GammaCorrected : CompositingQuality.Default; break;
            case "solid": brush = new SolidBrush(C(a, 1)); break;
            // linear x y w h angle c1 c2 [blend n f.. p..] [colorblend n c.. p..]
            case "linear": {
                var lb = new LinearGradientBrush(R(a, 1), C(a, 6), C(a, 10), F(a[5]));
                int i = 14;
                if (a.Length > i && a[i] == "wrap") { lb.WrapMode = (WrapMode)Enum.Parse(typeof(WrapMode), a[i + 1]); i += 2; }
                if (a.Length > i && a[i] == "blend") { int n = int.Parse(a[i + 1]); var b = new Blend(n); for (int k = 0; k < n; k++) { b.Factors[k] = F(a[i + 2 + k]); b.Positions[k] = F(a[i + 2 + n + k]); } lb.Blend = b; }
                if (a.Length > i && a[i] == "colors") { int n = int.Parse(a[i + 1]); var b = new ColorBlend(n); for (int k = 0; k < n; k++) { b.Colors[k] = C(a, i + 2 + 4 * k); b.Positions[k] = F(a[i + 2 + 4 * n + k]); } lb.InterpolationColors = b; }
                if (a.Length > i && a[i] == "gamma") lb.GammaCorrection = true;
                brush = lb; break;
            }
            // pathellipse x y w h center surround
            case "pathellipse": {
                var p = new GraphicsPath(); p.AddEllipse(R(a, 1));
                brush = new PathGradientBrush(p) { CenterColor = C(a, 5), SurroundColors = new[] { C(a, 9) } }; break;
            }
            // pen a r g b width [startcap endcap join] [dash solid|dash|dot]
            case "pen": {
                pen = new Pen(C(a, 1), F(a[5]));
                if (a.Length > 8) { pen.StartCap = Cap(a[6]); pen.EndCap = Cap(a[7]); pen.LineJoin = Join(a[8]); }
                if (a.Length > 10 && a[9] == "dash") pen.DashStyle = a[10] == "dash" ? DashStyle.Dash : a[10] == "dot" ? DashStyle.Dot : DashStyle.Solid;
                break;
            }
            case "fillrect": g.FillRectangle(brush, R(a, 1)); break;
            case "fillellipse": g.FillEllipse(brush, R(a, 1)); break;
            case "fillpie": g.FillPie(brush, F(a[1]), F(a[2]), F(a[3]), F(a[4]), F(a[5]), F(a[6])); break;
            case "fillround": using (var p = Round(R(a, 1), F(a[5]))) g.FillPath(brush, p); break;
            case "drawround": using (var p = Round(R(a, 1), F(a[5]))) g.DrawPath(pen, p); break;
            case "fillpoly": g.FillPolygon(brush, Pts(a, 1)); break;
            case "drawrect": g.DrawRectangle(pen, F(a[1]), F(a[2]), F(a[3]), F(a[4])); break;
            case "drawellipse": g.DrawEllipse(pen, R(a, 1)); break;
            case "drawline": g.DrawLine(pen, F(a[1]), F(a[2]), F(a[3]), F(a[4])); break;
            case "drawlines": g.DrawLines(pen, Pts(a, 1)); break;
            case "drawcurve": g.DrawCurve(pen, Pts(a, 1)); break;
            case "drawpoly": g.DrawPolygon(pen, Pts(a, 1)); break;
            case "drawarc": g.DrawArc(pen, R(a, 1), F(a[5]), F(a[6])); break;
            case "drawbezier": g.DrawBezier(pen, F(a[1]), F(a[2]), F(a[3]), F(a[4]), F(a[5]), F(a[6]), F(a[7]), F(a[8])); break;
            case "cliprect": g.SetClip(R(a, 1), CombineMode.Intersect); break;
            case "clipround": using (var p = Round(R(a, 1), F(a[5]))) g.SetClip(p, CombineMode.Intersect); break;
            case "clipellipse": using (var p = new GraphicsPath()) { p.AddEllipse(R(a, 1)); g.SetClip(p, CombineMode.Intersect); } break;
            case "resetclip": g.ResetClip(); break;
            case "translate": g.TranslateTransform(F(a[1]), F(a[2])); break;
            case "rotate": g.RotateTransform(F(a[1])); break;
            case "scale": g.ScaleTransform(F(a[1]), F(a[2])); break;
            case "save": saved.Push(g.Save()); break;
            case "restore": g.Restore(saved.Pop()); break;
            // Region scans of a rounded rect, as text: what an aliased path clip covers.
            case "scans": {
                using (var p = Round(R(a, 2), F(a[6]))) using (var rg = new Region(p)) using (var m = new Matrix())
                    File.WriteAllLines(Path.Combine(outDir, a[1] + ".txt"), rg.GetRegionScans(m).Select(s => string.Format(Inv, "{0} {1} {2} {3}", s.X, s.Y, s.Width, s.Height)).ToArray());
                break;
            }
            // Flattened points of an arc, ellipse or bezier: the curve tolerance.
            case "flatten": {
                using (var p = new GraphicsPath()) {
                    if (a[2] == "ellipse") p.AddEllipse(R(a, 3)); else if (a[2] == "round") p.AddPath(Round(R(a, 3), F(a[7])), false);
                    else if (a[2] == "arc") p.AddArc(R(a, 3), F(a[7]), F(a[8]));
                    var raw = p.PathPoints.Select(q => string.Format(Inv, "{0:R} {1:R}", q.X, q.Y)).ToList();
                    p.Flatten(null, a.Length > 9 && a[2] == "arc" ? F(a[9]) : 0.25f);
                    File.WriteAllLines(Path.Combine(outDir, a[1] + ".txt"), raw.Concat(new[] { "--" }).Concat(p.PathPoints.Select(q => string.Format(Inv, "{0:R} {1:R}", q.X, q.Y))).ToArray());
                }
                break;
            }
            // widen <name> ellipse|rect|round|arc x y w h [rad | start sweep]: the current pen's Widen outline (dash placement), as text.
            case "widen": {
                using (var p = new GraphicsPath()) {
                    if (a[2] == "ellipse") p.AddEllipse(R(a, 3)); else if (a[2] == "rect") p.AddRectangle(R(a, 3)); else if (a[2] == "round") p.AddPath(Round(R(a, 3), F(a[7])), false); else p.AddArc(R(a, 3), F(a[7]), F(a[8]));
                    p.Widen(pen, g != null ? g.Transform : null, 0.25f);
                    File.WriteAllLines(Path.Combine(outDir, a[1] + ".txt"), p.PathPoints.Zip(p.PathTypes, (q, t) => string.Format(Inv, "{0:R} {1:R} {2}", q.X, q.Y, t)).ToArray());
                }
                break;
            }
            // grab: the canvas so far becomes the image drawunscaled/drawimage draw (on a later canvas).
            case "grab": if (img != null) img.Dispose(); img = (Bitmap)bmp.Clone(); break;
            case "drawunscaled": g.DrawImageUnscaled(img, int.Parse(a[1]), int.Parse(a[2])); break;
            case "drawimage": g.DrawImage(img, R(a, 1)); break;
            // drawimagehq x y w h: HighQualityBilinear (the wallpaper preview).
            case "drawimagehq": { var im = g.InterpolationMode; g.InterpolationMode = InterpolationMode.HighQualityBilinear; g.DrawImage(img, R(a, 1)); g.InterpolationMode = im; break; }
            // drawimagebc x y w h: HighQualityBicubic (a PC game's title frames).
            case "drawimagebc": { var im = g.InterpolationMode; g.InterpolationMode = InterpolationMode.HighQualityBicubic; g.DrawImage(img, R(a, 1)); g.InterpolationMode = im; break; }
            // drawimageattr x y w h alpha: into Rectangle(x, y, w, h) with a ColorMatrix alpha (as the setup leave).
            case "drawimageattr": using (var ia = new ImageAttributes()) { ia.SetColorMatrix(new ColorMatrix { Matrix33 = F(a[5]) }); g.DrawImage(img, new Rectangle(int.Parse(a[1]), int.Parse(a[2]), int.Parse(a[3]), int.Parse(a[4])), 0, 0, img.Width, img.Height, GraphicsUnit.Pixel, ia); } break;
            // drawimagenn x y w h: as the launcher's DrawSprite (NearestNeighbor, PixelOffsetMode.Half).
            case "drawimagenn": { var im = g.InterpolationMode; var po = g.PixelOffsetMode; g.InterpolationMode = InterpolationMode.NearestNeighbor; g.PixelOffsetMode = PixelOffsetMode.Half; g.DrawImage(img, R(a, 1)); g.InterpolationMode = im; g.PixelOffsetMode = po; break; }
            // noise seed w h opaque|alpha: an image of System.Random pixels (fits a filter's weights).
            case "noise": {
                if (img != null) img.Dispose(); img = new Bitmap(int.Parse(a[2]), int.Parse(a[3]), PixelFormat.Format32bppArgb);
                var rnd = new Random(int.Parse(a[1]));
                for (int y = 0; y < img.Height; y++) for (int x = 0; x < img.Width; x++)
                    img.SetPixel(x, y, Color.FromArgb(a[4] == "alpha" ? rnd.Next(256) : 255, rnd.Next(256), rnd.Next(256), rnd.Next(256)));
                break;
            }
            // loadbin <file>: an image from a .bin as "out" writes it (relative to the working folder).
            case "loadbin": {
                var b = File.ReadAllBytes(a[1]); int w = BitConverter.ToInt32(b, 0), h = BitConverter.ToInt32(b, 4);
                if (img != null) img.Dispose(); img = new Bitmap(w, h, PixelFormat.Format32bppArgb);
                var d = img.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
                for (int y = 0; y < h; y++) System.Runtime.InteropServices.Marshal.Copy(b, 8 + y * w * 4, d.Scan0 + y * d.Stride, w * 4);
                img.UnlockBits(d); break;
            }
            // drawimagebcrot x y w h tx ty deg alpha: CapturePc's swirl (HighQualityBicubic, rotated, ColorMatrix alpha).
            case "drawimagebcrot": {
                var im = g.InterpolationMode; var st = g.Save(); g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.TranslateTransform(F(a[5]), F(a[6])); g.RotateTransform(F(a[7]));
                using (var ia = new ImageAttributes()) { ia.SetColorMatrix(new ColorMatrix { Matrix33 = F(a[8]) }); g.DrawImage(img, new Rectangle(int.Parse(a[1]), int.Parse(a[2]), int.Parse(a[3]), int.Parse(a[4])), 0, 0, img.Width, img.Height, GraphicsUnit.Pixel, ia); }
                g.Restore(st); g.InterpolationMode = im; break;
            }
            case "grabsub": if (img != null) img.Dispose(); img = bmp.Clone(new Rectangle(int.Parse(a[1]), int.Parse(a[2]), int.Parse(a[3]), int.Parse(a[4])), PixelFormat.Format32bppArgb); break;
            case "out": {
                var d = bmp.LockBits(new Rectangle(0, 0, bmp.Width, bmp.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
                var bytes = new byte[bmp.Width * bmp.Height * 4];
                for (int y = 0; y < bmp.Height; y++) System.Runtime.InteropServices.Marshal.Copy(d.Scan0 + y * d.Stride, bytes, y * bmp.Width * 4, bmp.Width * 4);
                bmp.UnlockBits(d);
                using (var f = File.Create(Path.Combine(outDir, a[1] + ".bin"))) {
                    var head = BitConverter.GetBytes(bmp.Width).Concat(BitConverter.GetBytes(bmp.Height)).ToArray();
                    f.Write(head, 0, 8); f.Write(bytes, 0, bytes.Length);
                }
                if (a.Length > 2 && a[2] == "png") bmp.Save(Path.Combine(outDir, a[1] + ".png"), ImageFormat.Png);
                break;
            }
            // fontinfo <name> <ttf>: the family's design metrics.
            case "fontinfo": {
                var ff = Family(a[2]);
                File.WriteAllText(Path.Combine(outDir, a[1] + ".txt"), string.Format(Inv, "{0} em {1} ascent {2} descent {3} spacing {4}", ff.Name, ff.GetEmHeight(FontStyle.Regular), ff.GetCellAscent(FontStyle.Regular), ff.GetCellDescent(FontStyle.Regular), ff.GetLineSpacing(FontStyle.Regular)));
                break;
            }
            // measure <name> <ttf> <px> <width> <wrap 0|1> <text...>: MeasureString as Write's StringFormat sets it,
            // then the box of every character (MeasureCharacterRanges, in a layout rect of that width).
            case "measure": {
                var ff = Family(a[2]);
                using (var f = new Font(ff, F(a[3]), ff.IsStyleAvailable(FontStyle.Regular) ? FontStyle.Regular : FontStyle.Bold, GraphicsUnit.Pixel))
                using (var sf = new StringFormat()) {
                    if (g == null) { bmp = new Bitmap(8, 8); g = Graphics.FromImage(bmp); }
                    g.SmoothingMode = SmoothingMode.AntiAlias; g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
                    float width = F(a[4]); bool wrap = a[5] == "1";
                    string text = string.Join(" ", a.Skip(6)).Replace("\\n", "\n");
                    sf.Trimming = StringTrimming.EllipsisCharacter; sf.FormatFlags = wrap ? StringFormatFlags.LineLimit : StringFormatFlags.NoWrap;
                    var lines = new List<string>();
                    lines.Add(string.Format(Inv, "height {0:R} fontheight {1} size {2:R}", f.GetHeight(g), f.Height, f.Size));
                    var plain = width > 0 ? g.MeasureString(text, f, (int)width) : g.MeasureString(text, f);
                    lines.Add(string.Format(Inv, "measure {0:R} {1:R}", plain.Width, plain.Height));
                    var big = new RectangleF(0, 0, width > 0 ? width : 100000, 100000);
                    for (int i = 0; i < text.Length; i += 32) {
                        int n = Math.Min(32, text.Length - i);
                        sf.SetMeasurableCharacterRanges(Enumerable.Range(i, n).Select(k => new CharacterRange(k, 1)).ToArray());
                        var rg = g.MeasureCharacterRanges(text, f, big, sf);
                        for (int k = 0; k < n; k++) { var b = rg[k].GetBounds(g); lines.Add(string.Format(Inv, "{0} {1:R} {2:R} {3:R} {4:R}", i + k, b.X, b.Y, b.Width, b.Height)); rg[k].Dispose(); }
                    }
                    File.WriteAllLines(Path.Combine(outDir, a[1] + ".txt"), lines.ToArray());
                }
                break;
            }
            // text <ttf> <px> <a r g b> <x y w h> <wrap> <halign> <valign> <text...>: DrawString as Write does.
            case "text": {
                var ff = Family(a[1]);
                using (var f = new Font(ff, F(a[2]), ff.IsStyleAvailable(FontStyle.Regular) ? FontStyle.Regular : FontStyle.Bold, GraphicsUnit.Pixel))
                using (var sf = new StringFormat()) using (var b = new SolidBrush(C(a, 3))) {
                    g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
                    sf.Alignment = (StringAlignment)int.Parse(a[12]); sf.LineAlignment = (StringAlignment)int.Parse(a[13]);
                    sf.Trimming = StringTrimming.EllipsisCharacter; sf.FormatFlags = a[11] == "1" ? StringFormatFlags.LineLimit : StringFormatFlags.NoWrap;
                    g.DrawString(string.Join(" ", a.Skip(14)), f, b, R(a, 7), sf);
                }
                break;
            }
            // random <name> <seed> <n>: System.Random's first NextDouble values, and Next(100).
            case "random": {
                var r = new Random(int.Parse(a[2])); var r2 = new Random(int.Parse(a[2]));
                File.WriteAllLines(Path.Combine(outDir, a[1] + ".txt"), Enumerable.Range(0, int.Parse(a[3])).Select(k => r.NextDouble().ToString("R", Inv) + " " + r2.Next(100)).ToArray());
                break;
            }
            default: throw new Exception("unknown op " + a[0]);
        }
    }
}
