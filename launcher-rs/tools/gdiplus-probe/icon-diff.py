# icon-diff.py: per-icon numbers from a run of scripts/t16.txt (GDI+ glyph vs our code-drawn icon).
import struct, sys, os
import numpy as np
def load(p):
    d=open(p,'rb').read(); w,h=struct.unpack('<ii',d[:8]); a=np.frombuffer(d[8:],np.uint8).reshape(h,w,4)
    return 255-a[:,:,1].astype(int)
out=os.environ.get('PROBE_OUT', os.path.join(os.environ.get('NATIVEMONS_WORK', os.environ.get('TEMP', '.')), 'gdiplus-probe'))
# icon-diff.py [script] [codes...]: defaults to t16 and the first five icons.
script = sys.argv[1] if len(sys.argv) > 1 else "t16"
codes = sys.argv[2:] or ["E711","E921","E790","E8BD","E72D"]
for c in codes:
    row=[c]
    for sz in ["400","17","10"]:
        g=load(f'{out}/g/{script}/i{c}_{sz}.bin'); o=load(f'{out}/o/{script}/i{c}_{sz}.bin')
        gi=g>127; oi=o>127; iou=(gi&oi).sum()/max(1,(gi|oi).sum())
        wiou=np.minimum(g,o).sum()/max(1,np.maximum(g,o).sum())
        row.append(f'{sz}px: ink IoU {wiou*100:5.1f}% (binary {iou*100:5.1f}%) max {np.abs(g-o).max():3d}')
    print(' | '.join(row))
