import os, re
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "launcher", "NativeMonsLauncher.cs"), encoding='utf-8').read()
STR = r'"((?:[^"\\]|\\.)*)"'
def unesc(s): return s.encode('utf-8').decode('unicode_escape').encode('latin-1').decode('utf-8')
i = src.index("Page CreditsPage()"); j = src.index("// Keys each engine answers", i)
blk = src[i:j]
k = blk.index("+ Gen1Credit")
g1 = re.search(r'Gen1Credit = ' + STR, src).group(1)
before = re.findall(STR, blk[:k]); after = re.findall(STR, blk[k:])
before = [b for b in before if b != "Credits"]
body = unesc("".join(before) + g1 + "".join(after))
open("credits_body.txt", "w", encoding='utf-8').write(body)
L = ["measure body @NotoSans-Regular.ttf 16.43 448 1 " + body.replace("\n", "\\n")]
items = [("NotoSans-Regular.ttf", 20.29, "Fulldex"), ("NotoSans-Regular.ttf", 20.29, "From your games"), ("NotoSans-SemiBold.ttf", 28.7, "Settings"),
         ("NotoSans-SemiBold.ttf", 13.27, "9:41"), ("NotoSans-Light.ttf", 23, "MONS"), ("NotoSans-SemiBold.ttf", 41.23, "Native"), ("NotoSans-Light.ttf", 41.23, "Mons"),
         ("NotoSans-Regular.ttf", 13.27, "DS and GBA games: widescreen, frame rate and resolution."), ("NotoSans-Regular.ttf", 13.27, "Looking for your games"),
         ("NotoSans-SemiBold.ttf", 13.27, "Collection"), ("NotoSans-Black.ttf", 23, "NATIVE"), ("NotoSans-Regular.ttf", 20.29, "Download Pokedex")]
for n, (f, px, t) in enumerate(items):
    L.append(f"measure s{n} @{f} {px} 0 0 {t}")
L.append("measure note @NotoSans-Regular.ttf 13.27 455 1 DS and GBA games: widescreen, frame rate and resolution.")
open("t8.txt", "w", encoding='utf-8').write("\n".join(L) + "\n")
print(repr(body[:120]), body.count("\n"))
