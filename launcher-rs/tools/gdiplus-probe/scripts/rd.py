import struct,sys
def load(p):
    d=open(p,'rb').read(); w,h=struct.unpack('<ii',d[:8]); return w,h,d[8:]
def px(img,x,y):
    w,h,d=img; i=(y*w+x)*4; return d[i+2],d[i+1],d[i],d[i+3]
