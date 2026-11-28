# fit-icon.py <E790|E72D>: fits our icon's control points (em units) to the GDI+ render at 400 px by coordinate
# descent on the stroked-shape XOR. Test-only design aid; prints the fitted points for icons.rs.
import os, struct, sys
import numpy as np

OUT = os.environ.get('PROBE_OUT', os.path.join(os.environ.get('NATIVEMONS_WORK', os.environ.get('TEMP', '.')), 'gdiplus-probe'))
S, X0, Y0, W = 400.0, 67.0, 0.0, 1 / 16


def load(p):
    d = open(p, 'rb').read(); w, h = struct.unpack('<ii', d[:8])
    return (255 - np.frombuffer(d[8:], np.uint8).reshape(h, w, 4)[:, :, 1].astype(int)) > 127


def bez(p0, p1, p2, p3, n=24):
    t = np.linspace(0, 1, n)[:, None]
    return (1 - t) ** 3 * p0 + 3 * (1 - t) ** 2 * t * p1 + 3 * (1 - t) * t * t * p2 + t ** 3 * p3


H, Wd = 440, 560
ys, xs = np.mgrid[0:H, 0:Wd]
px = np.stack([(xs - X0) / S, (ys - Y0) / S], -1).reshape(-1, 2)
# Only the region a shape's free points can reach is compared (fast); the rest stays as fitted.
ROI = {'E790': (0.0, 0.42, 0.62, 1.02), 'E72D': (0.0, 0.0, 1.0, 1.0)}


def stroke_mask(polys):
    """Pixels within half the stroke of any polyline segment."""
    d = np.full(px.shape[0], 9.0)
    for poly in polys:
        a, b = poly[:-1], poly[1:]
        for i in range(len(a)):
            ab = b[i] - a[i]; l2 = (ab * ab).sum() or 1e-12
            t = np.clip(((px - a[i]) @ ab) / l2, 0, 1)
            q = a[i] + t[:, None] * ab
            d = np.minimum(d, np.sqrt(((px - q) ** 2).sum(1)))
    return (d <= W / 2).reshape(H, Wd)


def palette(v):
    r = 0.469
    ang = np.radians(np.linspace(175, 175 + 282, 120))
    ring = np.stack([0.5 + r * np.cos(ang), 0.5 + r * np.sin(ang)], 1)
    P = v.reshape(-1, 2)
    pts = [ring[-1]] + list(P) + [ring[0]]
    segs = [ring]
    for k in range(4):
        segs.append(bez(*[np.array(q) for q in pts[3 * k:3 * k + 4]], n=16))
    return segs


def share(v):
    P = v.reshape(-1, 2)
    tip, top_back, low_back, ctl = P[0], P[1], P[2], P[3:]
    box = np.array([[0.03125, 0.25], [0.03125, 0.84575], [0.71875, 0.84575], [0.71875, 0.69]])
    head = np.array([low_back, [0.656, 0.6], tip, [0.656, 0.075], top_back])
    up = bez(top_back, ctl[0], ctl[1], ctl[2])
    lo = bez(ctl[2], ctl[3], ctl[4], low_back)
    return [box, head, up, lo]


SHAPES = {
    'E790': (palette, [0.385, 0.93, 0.385, 0.84, 0.425, 0.76, 0.475, 0.67, 0.485, 0.59, 0.41, 0.55, 0.31, 0.49, 0.22, 0.525, 0.14, 0.565, 0.07, 0.60, 0.03, 0.575]),
    'E72D': (share, [0.92, 0.34, 0.656, 0.205, 0.656, 0.485, 0.43, 0.205, 0.24, 0.39, 0.205, 0.66, 0.31, 0.535, 0.45, 0.485]),
}
code = sys.argv[1]
fn, v = SHAPES[code]
v = np.array(v, float)
g = load(f'{OUT}/g/t16/i{code}_400.bin')
if code == 'E790':
    # the dots are fitted already: leave them out of the target
    for (x, y) in [(0.438, 0.252), (0.688, 0.314), (0.25, 0.377), (0.75, 0.564), (0.625, 0.752)]:
        g[(((xs - X0) / S - x) ** 2 + ((ys - Y0) / S - y) ** 2) < 0.075 ** 2] = False
x0r, y0r, x1r, y1r = ROI[code]
roi = ((px[:, 0] >= x0r) & (px[:, 0] <= x1r) & (px[:, 1] >= y0r) & (px[:, 1] <= y1r))
px_all = px
px = px[roi]
H2 = None


def stroke_roi(polys):
    d = np.full(px.shape[0], 9.0)
    for poly in polys:
        a, b = poly[:-1], poly[1:]
        ab = b - a; l2 = np.maximum((ab * ab).sum(1), 1e-12)
        for i in range(len(a)):
            t = np.clip(((px - a[i]) @ ab[i]) / l2[i], 0, 1)
            q = a[i] + t[:, None] * ab[i]
            d = np.minimum(d, ((px - q) ** 2).sum(1))
    return d <= (W / 2) ** 2


g_roi = g.reshape(-1)[roi]
cost = lambda vv: (stroke_roi(fn(vv)) ^ g_roi).sum()
best = cost(v)
print('start', best)
for step in [0.02, 0.01, 0.005, 0.0025]:
    improved = True
    while improved:
        improved = False
        for i in range(len(v)):
            for dlt in (step, -step):
                t = v.copy(); t[i] += dlt
                c = cost(t)
                if c < best:
                    best, v, improved = c, t, True
    print('step', step, best)
print(' '.join('%.4f' % x for x in v))
