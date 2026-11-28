import sys, os
G = "../g/t8"; O = "../o/t8"
def load(p):
    L = open(p, encoding='utf-8').read().splitlines()
    head = L[:2]; boxes = [tuple(float(v) for v in l.split()[1:]) for l in L[2:] if l.strip()]
    return head, boxes
for n in sorted(os.listdir(G)):
    g = load(os.path.join(G, n)); o = load(os.path.join(O, n))
    bad = [i for i, (a, b) in enumerate(zip(g[1], o[1])) if a[:3] != b[:3]]
    print(n, 'gdi', g[0][1], '| ours', o[0][1], '| boxes differ', len(bad), 'of', len(g[1]))
    if g[0][0] != o[0][0]: print('   heights', g[0][0], '|', o[0][0])
    for i in bad[:int(sys.argv[1]) if len(sys.argv) > 1 else 3]:
        print('   ', i, 'gdi', g[1][i], 'ours', o[1][i])
