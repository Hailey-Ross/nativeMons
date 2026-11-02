// nativeMons launcher: one menu for native Pokemon ports on PC, for the controller, keyboard or mouse.
//   D-pad / left stick = move, A = select, B = back. Arrows / Enter / Esc work too.
//
// The window, splash, tiles and detail panel come from the steam launcher of the Skate 3 Rust engine's
// hails-additions branch (github.com/Hailey-Ross/rusty-trucks, tools/steam-launcher, GPL-3.0). The games run as
// separate programs, so nothing from either engine is built into this one.
//
// Licensed under the GNU GPL v3 or later (see LICENSE).
// Build (build.bat): the C# compiler that ships with Windows, C# 5, so no string interpolation or ?.
//   NativeMonsLauncher.exe --play <game id> starts one game and closes afterwards (shortcuts, Steam).
//   NativeMonsLauncher.exe --snapshot <out.png> [main[:<game id>]|games|game|mods|folders|credits|job|splash|sprites] [<w>x<h>]
//   draws one screen and exits.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows.Forms;

static class Native {
    [StructLayout(LayoutKind.Sequential)]
    public struct XState { public uint Packet; public ushort Buttons; public byte LT, RT; public short LX, LY, RX, RY; }
    [DllImport("xinput1_4.dll")] public static extern int XInputGetState(int i, out XState s);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(int pid);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern uint GetShortPathName(string path, StringBuilder shortPath, uint size);
    [DllImport("user32.dll")] public static extern bool ReleaseCapture();
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, int msg, IntPtr w, IntPtr l);

    // Windows only lets the foreground owner hand the foreground on; joining its input for the call lifts that.
    public static void Front(IntPtr h) {
        if (h == IntPtr.Zero) return;
        if (IsIconic(h)) ShowWindow(h, 9);
        uint fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero), me = GetCurrentThreadId();
        bool joined = fg != 0 && fg != me && AttachThreadInput(me, fg, true);
        BringWindowToTop(h); SetForegroundWindow(h);
        if (joined) AttachThreadInput(me, fg, false);
    }
}

// Where everything is. Each path can be changed in state\nativemons.cfg (key=value).
static class Config {
    public static readonly string Here = AppDomain.CurrentDomain.BaseDirectory;
    public static readonly string State = Environment.GetEnvironmentVariable("NATIVEMONS_STATE") ?? Path.Combine(Here, "state");
    static readonly Dictionary<string, string> values = new Dictionary<string, string>();
    public static void Load() {
        Directory.CreateDirectory(State);
        string f = Path.Combine(State, "nativemons.cfg");
        if (!File.Exists(f)) return;
        foreach (var l in File.ReadAllLines(f)) {
            int eq = l.IndexOf('=');
            if (eq > 0 && !l.StartsWith("#")) values[l.Substring(0, eq).Trim()] = l.Substring(eq + 1).Trim();
        }
    }
    static string Get(string key, string fallback) {
        string v;
        return Path.GetFullPath(Path.Combine(Here, values.TryGetValue(key, out v) ? v : fallback));
    }
    public static string Love { get { return Get("love", @"love\lovec.exe"); } }
    public static string Gen1 { get { return Get("gen1recomp", @"engines\gen1recomp"); } }
    public static string Fulldex { get { return Get("fulldex", @"engines\fulldex\Pokemon Fulldex.exe"); } }
    public static string Node { get { return Get("node", @"tools\openhome\node.exe"); } }
    public static string OpenHomeTool { get { return Get("openhome", @"tools\openhome\convert.cjs"); } }
    public static string PlatinumPort { get { return Get("pokeplatinum", @"engines\pokeplatinum\main.exe"); } }
    public static string Nativeplat { get { return Get("nativeplat", @"engines\nativeplat\nativeplat.exe"); } }
    public static string Patcher { get { return Get("patcher", @"tools\nds-patcher\hails_patcher.py"); } }
    public static string MonBridge { get { return Get("monbridge", @"tools\mon-bridge\mon-bridge.exe"); } }
    public static string GamesFile { get { return Get("games", "games.json"); } }
    public static string Python { get { string v; return values.TryGetValue("python", out v) ? v : "python"; } }
    public static string FulldexDir { get { return Path.GetDirectoryName(Fulldex); } }
    public static List<string> RomFolders() {
        string f = Path.Combine(State, "rom-folders.txt");
        var l = File.Exists(f) ? File.ReadAllLines(f).Where(x => x.Trim() != "").ToList() : new List<string>();
        return l;
    }
    public static void SaveRomFolders(List<string> folders) { File.WriteAllLines(Path.Combine(State, "rom-folders.txt"), folders.ToArray()); }
    public static string Read(string name, string fallback) {
        try { string f = Path.Combine(State, name); return File.Exists(f) ? File.ReadAllText(f).Trim() : fallback; } catch { return fallback; }
    }
    public static void Write(string name, string text) { try { File.WriteAllText(Path.Combine(State, name), text); } catch { } }
}

// A version's colours, from games.json: main, deep, light, and a finish (plain or a scene: crystal, gold, silver, flame, tide, bolt).
class Theme {
    public Color Main, Deep, Light, Ink; public string Finish = "plain";
    public string GameId;   // whose title screen to show, once it has been captured (see GameArt)
    // Gold, silver and crystal surfaces have no single colour to use for text on cream.
    public bool Plain { get { return Finish != "gold" && Finish != "silver" && Finish != "crystal"; } }
    // Text in the version's colour on a cream box, unless that colour is too light to read there.
    public Color OnCream { get { double lum = 0.299 * Deep.R + 0.587 * Deep.G + 0.114 * Deep.B; return Plain && lum < 110 ? Deep : Color.FromArgb(38, 40, 50); } }
    public static Theme Make(string main, string deep, string light, string finish) {
        var t = new Theme { Main = ColorTranslator.FromHtml(main), Deep = ColorTranslator.FromHtml(deep), Light = ColorTranslator.FromHtml(light), Finish = finish ?? "plain" };
        // Dark ink on a light surface (Yellow, Gold, Silver, White, Crystal), light ink on a dark one.
        double lum = 0.299 * t.Main.R + 0.587 * t.Main.G + 0.114 * t.Main.B;
        t.Ink = lum > 150 ? Color.FromArgb(28, 28, 34) : Color.FromArgb(252, 250, 244);
        return t;
    }
    public static readonly Theme Default = Make("#D6302B", "#8E1712", "#F2665C", "plain");
    public static Theme[] Parade = new[] { Default };
}
class GameEntry {
    public Theme Theme = Theme.Default;
    public int Dex;   // how many National Dex entries this game has
    public string Id, Title, Console, Gen1, FulldexCode, Header, Save, Port, Nativeplat;
    public int ArtFrame;   // when this game's title screen is up, in frames from power-on
    public string Gen1Rom, FulldexRom, NativeplatRom, FoundUnsupported, FoundSha1;
    public bool Imported;
    public bool OnGen1 { get { return Gen1 != null && (Imported || Gen1Rom != null); } }
    public bool OnFulldex { get { return FulldexCode != null && FulldexRom != null && File.Exists(Config.Fulldex); } }
    public bool OnPort { get { return Port != null && FulldexRom != null && File.Exists(Config.PlatinumPort); } }
    public bool OnNativeplat { get { return Nativeplat != null && NativeplatRom != null && File.Exists(Config.Nativeplat); } }
    public bool Playable { get { return OnGen1 || OnFulldex || OnPort || OnNativeplat; } }
    public bool IsDs { get { return Console == "Nintendo DS"; } }
    public bool HasBoth { get { return OnGen1 && OnFulldex; } }
    // The player's choice where both engines can run it; Fulldex by default (widescreen).
    public List<string> Engines() {
        var l = new List<string>();
        if (OnFulldex) l.Add("fulldex");
        if (OnGen1) l.Add("gen1");
        if (OnPort) l.Add("port");
        if (OnNativeplat) l.Add("nativeplat");
        return l;
    }
    public string Engine() {
        var l = Engines();
        if (l.Count == 0) return "gen1";
        string chosen = Config.Read("engine-" + Id + ".txt", l[0]);
        return l.Contains(chosen) ? chosen : l[0];
    }
    public static string EngineName(string engine) { return engine == "fulldex" ? "Fulldex (widescreen, 60 fps)" : engine == "port" ? "the Platinum PC port (native)" : engine == "nativeplat" ? "nativeplat (native)" : "Gen1Recomp (native, with mods)"; }
    public static string EngineShort(string engine) { return engine == "fulldex" ? "Fulldex" : engine == "port" ? "Platinum PC port" : engine == "nativeplat" ? "nativeplat" : "Gen1Recomp"; }
}

// Unpacks a DS ROM the way the Platinum PC port does on first launch, so its file picker never shows.
// Files already there stay. What it writes comes from the player's own ROM, and stays on their PC.
static class NdsExtract {
    static void Put(string file, byte[] rom, uint offset, uint size) {
        if (File.Exists(file)) return;
        if (offset + (ulong)size > (ulong)rom.Length) throw new InvalidDataException("That ROM looks cut short.");
        Directory.CreateDirectory(Path.GetDirectoryName(file));
        using (var f = new FileStream(file, FileMode.CreateNew, FileAccess.Write)) f.Write(rom, (int)offset, (int)size);
    }
    static uint U32(byte[] b, uint at) { return BitConverter.ToUInt32(b, (int)at); }
    static ushort U16(byte[] b, uint at) { return BitConverter.ToUInt16(b, (int)at); }
    public static void Run(string romFile, string dir) {
        var rom = File.ReadAllBytes(romFile);
        uint arm9 = U32(rom, 0x20), arm9Size = U32(rom, 0x2C), arm7 = U32(rom, 0x30), arm7Size = U32(rom, 0x3C);
        uint fnt = U32(rom, 0x40), fntSize = U32(rom, 0x44), fat = U32(rom, 0x48), fatSize = U32(rom, 0x4C);
        uint ovt9 = U32(rom, 0x50), ovt9Size = U32(rom, 0x54), ovt7 = U32(rom, 0x58), ovt7Size = U32(rom, 0x5C);
        uint icon = U32(rom, 0x68), rsa = U32(rom, 0x80);
        Put(Path.Combine(dir, "header.bin"), rom, 0, 0x4000);
        Put(Path.Combine(dir, "arm9.bin"), rom, arm9, arm9Size);
        Put(Path.Combine(dir, "arm7.bin"), rom, arm7, arm7Size);
        Put(Path.Combine(dir, "arm9ovt.bin"), rom, ovt9, ovt9Size);
        Put(Path.Combine(dir, "arm7ovt.bin"), rom, ovt7, ovt7Size);
        if (icon != 0) {
            ushort kind = U16(rom, icon);
            Put(Path.Combine(dir, "banner.bin"), rom, icon, kind == 2 ? 0x940u : kind == 3 ? 0xA40u : kind == 0x103 ? 0x23C0u : 0x840u);
        }
        Put(Path.Combine(dir, "fnt.bin"), rom, fnt, fntSize);
        Put(Path.Combine(dir, "fat.bin"), rom, fat, fatSize);
        Put(Path.Combine(dir, "rsasig.bin"), rom, rsa, 136);
        for (uint i = 0; i < ovt9Size / 32; i++) Overlay(rom, Path.Combine(dir, "overlay9", "overlay9_" + U32(rom, ovt9 + i * 32) + ".bin"), U16(rom, ovt9 + i * 32 + 24), fat);
        for (uint i = 0; i < ovt7Size / 32; i++) Overlay(rom, Path.Combine(dir, "overlay7", "overlay7_" + U32(rom, ovt7 + i * 32) + ".bin"), U16(rom, ovt7 + i * 32 + 24), fat);
        Tree(rom, fnt, fat, 0xF000, dir);
    }
    static void Overlay(byte[] rom, string file, ushort id, uint fat) {
        uint start = U32(rom, fat + id * 8u);
        Put(file, rom, start, U32(rom, fat + id * 8u + 4) - start);
    }
    static void Tree(byte[] rom, uint fnt, uint fat, int dirId, string path) {
        Directory.CreateDirectory(path);
        uint entry = fnt + (uint)(dirId & 0xFFF) * 8;
        uint at = fnt + U32(rom, entry);
        ushort fileId = U16(rom, entry + 4);
        while (true) {
            int head = rom[at++];
            if (head == 0) break;
            int length = head & 0x7F;
            string name = Encoding.ASCII.GetString(rom, (int)at, length);
            if (name.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 || name == "..") throw new InvalidDataException("That ROM has a bad file name in it.");
            at += (uint)length;
            if ((head & 0x80) != 0) {
                int child = U16(rom, at); at += 2;
                Tree(rom, fnt, fat, child, Path.Combine(path, name));
            } else {
                uint start = U32(rom, fat + fileId * 8u);
                Put(Path.Combine(path, name), rom, start, U32(rom, fat + fileId * 8u + 4) - start);
                fileId++;
            }
        }
    }
}

// The Platinum PC port's save.bin holds the same data as a DS save, but its 64-bit build pads some structures to 8-byte
// boundaries: the s64 times in GameTime and SystemData, the BoxPokemon (u64 ribbons) in each Daycare slot, and PCBoxes.
// Nothing else differs, so a save converts both ways by adding or removing that padding and redoing each block's footer.
static class PlatinumSave {
    const int MainDs = 0xCF2C, BoxesDs = 0x121E4, FooterSize = 20, HalfSize = 0x40000, Size = 0x80000;
    const uint Signature = 0x20060623;
    static readonly int[] MainPads = { 0x34, 0x48, 0x64, 0x1740, 0x182C };   // where, in DS offsets, 4 bytes are added
    static readonly int[] BoxesPads = { 0x4, BoxesDs - FooterSize };
    static int MainPort { get { return MainDs + MainPads.Length * 4; } }
    static int BoxesPort { get { return BoxesDs + BoxesPads.Length * 4; } }
    static ushort Crc(byte[] d, int at, int n) {
        ushort c = 0xFFFF;
        for (int i = 0; i < n; i++) {
            c ^= (ushort)(d[at + i] << 8);
            for (int b = 0; b < 8; b++) c = (ushort)((c & 0x8000) != 0 ? (c << 1) ^ 0x1021 : c << 1);
        }
        return c;
    }
    static bool Valid(byte[] d, int at, int size, int id) {
        int f = at + size - FooterSize;
        return f + FooterSize <= d.Length && BitConverter.ToUInt32(d, f + 8) == size && BitConverter.ToUInt32(d, f + 12) == Signature
            && d[f + 16] == id && BitConverter.ToUInt16(d, f + 18) == Crc(d, at, size - FooterSize);
    }
    // One block's body, with padding added or removed. Removed padding has to be zero, or it wasn't padding.
    static void Body(byte[] src, int from, byte[] dst, int to, int dsBody, int[] pads, bool toPort) {
        int ds = 0, port = 0;
        foreach (int pad in pads) {
            int run = pad - ds;
            if (toPort) { Array.Copy(src, from + ds, dst, to + port, run); Array.Clear(dst, to + port + run, 4); }
            else {
                Array.Copy(src, from + port, dst, to + ds, run);
                for (int k = 0; k < 4; k++) if (src[from + port + run + k] != 0) throw new InvalidDataException("The port's save has data where padding was expected, so it wasn't converted.");
            }
            ds += run; port += run + 4;
        }
        int rest = dsBody - ds;
        if (toPort) Array.Copy(src, from + ds, dst, to + port, rest); else Array.Copy(src, from + port, dst, to + ds, rest);
    }
    static void Footer(byte[] src, int srcFooter, byte[] dst, int at, int size) {
        int f = at + size - FooterSize;
        Array.Copy(src, srcFooter, dst, f, FooterSize);
        BitConverter.GetBytes((uint)size).CopyTo(dst, f + 8);
        BitConverter.GetBytes(Crc(dst, at, size - FooterSize)).CopyTo(dst, f + 18);
    }
    static byte[] Convert(byte[] input, bool toPort) {
        var src = new byte[Size];
        for (int i = 0; i < Size; i++) src[i] = i < input.Length ? input[i] : (byte)0xFF;
        var dst = (byte[])src.Clone();
        int mainFrom = toPort ? MainDs : MainPort, boxesFrom = toPort ? BoxesDs : BoxesPort, mainTo = toPort ? MainPort : MainDs, boxesTo = toPort ? BoxesPort : BoxesDs;
        int converted = 0;
        foreach (int half in new[] { 0, HalfSize }) {
            // Both blocks are always rewritten together, and blank them first so nothing old shows past a shorter block.
            bool main = Valid(src, half, mainFrom, 0), boxes = Valid(src, half + mainFrom, boxesFrom, 1);
            int area = Math.Max(MainDs + BoxesDs, MainPort + BoxesPort);
            for (int i = 0; i < area; i++) dst[half + i] = 0xFF;
            if (main) { Body(src, half, dst, half, MainDs - FooterSize, MainPads, toPort); Footer(src, half + mainFrom - FooterSize, dst, half, mainTo); converted++; }
            if (boxes) { Body(src, half + mainFrom, dst, half + mainTo, BoxesDs - FooterSize, BoxesPads, toPort); Footer(src, half + mainFrom + boxesFrom - FooterSize, dst, half + mainTo, boxesTo); converted++; }
        }
        if (converted == 0) throw new InvalidDataException("No valid save data was found to convert.");
        // The port fills space it hasn't used with zeros. On a cartridge unused space reads as erased.
        if (!toPort) for (int sector = 0; sector < Size; sector += 0x1000) {
            bool empty = true;
            for (int i = 0; i < 0x1000 && empty; i++) empty = dst[sector + i] == 0;
            if (empty) for (int i = 0; i < 0x1000; i++) dst[sector + i] = 0xFF;
        }
        return dst;
    }
    public static byte[] ToPort(byte[] ds) { return Convert(ds, true); }
    public static byte[] ToDs(byte[] port) { return Convert(port, false); }
}

// Each game's own title screen, captured from the player's ROM on their PC by running it without a window. None of it
// ships: it lives in state\game-art, and a game without any keeps its drawn scene.
static class GameArt {
    const int Count = 48, Step = 6;   // ten pictures a second, for just under five seconds
    static readonly object gate = new object();
    static string loadedId; static Bitmap[] loaded; static Bitmap loadedLogo;
    static string Folder(string id) { return Path.Combine(Config.State, "game-art", id); }
    static string FrameFile(string id, int i) { return Path.Combine(Folder(id), "frame-" + i.ToString("00") + ".jpg"); }
    static string LogoFile(string id) { return Path.Combine(Folder(id), "logo.png"); }
    public static bool Has(string id) { return id != null && File.Exists(FrameFile(id, Count - 1)); }
    // The home screen's wallpaper: one of the game's towns.
    static string TownFile(string id) { return Path.Combine(Folder(id), "town.jpg"); }
    public static bool HasTown(string id) { return id != null && File.Exists(TownFile(id)); }
    static string loadedTownId; static Bitmap loadedTown;
    public static Bitmap Town(string id) {
        if (!HasTown(id)) return null;
        lock (gate) {
            if (loadedTownId != id) {
                try {
                    Bitmap next; using (var f = new Bitmap(TownFile(id))) next = new Bitmap(f);
                    if (loadedTown != null) loadedTown.Dispose();
                    loadedTown = next; loadedTownId = id;
                } catch { return null; }
            }
            return loadedTown;
        }
    }
    public static bool CanCaptureTown(GameEntry g) { return g.OnNativeplat || (g.Gen1 != null && g.Gen1Rom != null); }
    // nativeplat's test mode, which uses saves of its own and never the player's. At its furthest camera the whole of Twinleaf
    // fits and the people, the player included, are too far off to be drawn. Diamond and Pearl play a scripted new game (from
    // gen4recomp's tests/dp) until they can walk, then jump to the town. Platinum's save lab makes a save standing there.
    const string DpNewGame = "1250:start;1350:a:4:40:35;2800:down;2820:down;2840:a;2900:a:4:40:14;3560:tap:128:100:8:40:3;3700:a:4:40:49;5850:tap:82:118:6;6600:b:4:40:6;6950:down;6980:a;7040:a:4:40:40;9300:a:4:40:120";
    const string Twinleaf = "411", TwinleafX = "110", TwinleafZ = "888";
    static int RunNativeplatTest(string spec, string lab, string warp) {
        var psi = new ProcessStartInfo(Config.Nativeplat) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(Config.Nativeplat), RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        psi.EnvironmentVariables["SDL_VIDEO_DRIVER"] = "dummy";
        psi.EnvironmentVariables["SDL_AUDIO_DRIVER"] = "dummy";
        psi.EnvironmentVariables["NP_AUTOTEST"] = spec;
        if (lab != null) { psi.EnvironmentVariables["PC_LAB"] = lab; psi.EnvironmentVariables["PC_LAB_AT"] = "300"; }
        if (warp != null) psi.EnvironmentVariables["PC_WARP"] = warp;
        using (var p = Process.Start(psi)) {
            try { p.PriorityClass = ProcessPriorityClass.Idle; } catch { }
            p.BeginOutputReadLine(); p.BeginErrorReadLine();
            if (!p.WaitForExit(600000)) { try { p.Kill(); } catch { } return -1; }
            return p.ExitCode;
        }
    }
    static string CaptureTownNativeplat(GameEntry g) {
        string shot = Path.Combine(Config.State, "game-art", g.Id + "-town.png"), root = Path.Combine(Path.GetDirectoryName(Config.Nativeplat), "userdata");
        string slotName = "nativemons-wallpaper", slot = Path.Combine(root, "saves", g.Nativeplat, slotName + ".sav"), script = shot + ".press";
        string view = "size=1024x768,layout=top,zoom=1024,game=" + g.Nativeplat + ",png=" + shot + ",rom=" + LauncherForm.NativeplatRomCopy(g);
        try {
            Directory.CreateDirectory(Path.GetDirectoryName(shot));
            if (File.Exists(shot)) File.Delete(shot);
            if (g.Nativeplat == "platinum") {
                Directory.CreateDirectory(Path.GetDirectoryName(slot));
                File.WriteAllBytes(slot, new byte[0]);
                string stored = ",storage=1,slot=" + slotName;
                // This run ends itself once the save is made, with an exit code of its own.
                RunNativeplatTest("frames=1500," + view + stored, "inline:map " + Twinleaf + " " + TwinleafX + " " + TwinleafZ + " 1;", null);
                if (new FileInfo(slot).Length == 0) return "The wallpaper's save wasn't made.";
                if (File.Exists(shot)) File.Delete(shot);
                RunNativeplatTest("frames=2800," + view + stored + ",press=1450:start:4:40:4;1800:a:4:60:6", null, null);
            } else {
                File.WriteAllText(script, DpNewGame.Replace(";", "\n"));
                RunNativeplatTest("frames=15500," + view + ",press=@" + script, null, Twinleaf + ":" + TwinleafX + ":" + TwinleafZ + ":1");
            }
            if (!File.Exists(shot)) return "No picture came out.";
            // Around the town itself, which sits in the upper middle of this view, clear of the dark past the loaded map.
            return SaveTown(g, shot, new RectangleF(0.1f, 0.06f, 0.66f, 0.9f));
        } catch (Exception x) { return x.Message; }
        finally {
            foreach (var f in new[] { shot, script, slot, slot + ".bak", slot + ".tmp", Path.Combine(root, "options-autotest.ini") }) try { if (File.Exists(f)) File.Delete(f); } catch { }
        }
    }
    // Whatever surrounds the picture is cut away: the window's edge, or the unloaded dark beyond a map.
    static string SaveTown(GameEntry g, string shot, RectangleF? part = null) {
        Directory.CreateDirectory(Folder(g.Id));
        using (var pic = new Bitmap(shot)) {
            int left = pic.Width, top = pic.Height, right = 0, bottom = 0;
            for (int y = 0; y < pic.Height; y += 2) for (int x = 0; x < pic.Width; x += 2) {
                var c = pic.GetPixel(x, y);
                if (c.R + c.G + c.B > 24) { left = Math.Min(left, x); top = Math.Min(top, y); right = Math.Max(right, x); bottom = Math.Max(bottom, y); }
            }
            if (left >= right || top >= bottom) return "The town came out blank.";
            var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
            var quality = new System.Drawing.Imaging.EncoderParameters(1);
            quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 90L);
            var area = Rectangle.FromLTRB(left, top, Math.Min(pic.Width, right + 2), Math.Min(pic.Height, bottom + 2));
            if (part != null) area = new Rectangle((int)(pic.Width * part.Value.X), (int)(pic.Height * part.Value.Y), (int)(pic.Width * part.Value.Width), (int)(pic.Height * part.Value.Height));
            using (var cut = pic.Clone(area, pic.PixelFormat)) cut.Save(TownFile(g.Id), jpeg, quality);
        }
        return null;
    }
    // Always under the throwaway identity: the game has to start a new game to reach a town, and that must never meet a real save.
    public static string CaptureTown(GameEntry g) {
        if (g.IsDs) return CaptureTownNativeplat(g);
        string shot = Path.Combine(Config.State, "game-art", g.Id + "-town.png");
        string identityFolder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LOVE", ArtIdentity);
        try {
            Directory.CreateDirectory(Path.GetDirectoryName(shot));
            if (File.Exists(shot)) File.Delete(shot);
            var import = LauncherForm.Gen1Start("");
            import.EnvironmentVariables["POKEPORT_IDENTITY"] = ArtIdentity;
            import.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
            import.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
            int code;
            using (var rom = new LauncherForm.PlainPath(g.Gen1Rom)) {
                import.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = rom.Path_;
                LauncherForm.RunCapture(import, 240, out code);
            }
            if (code != 0) return "Could not prepare the game for its picture.";
            var psi = LauncherForm.Gen1Start("");
            psi.EnvironmentVariables["POKEPORT_IDENTITY"] = ArtIdentity;
            psi.EnvironmentVariables["POKEPORT_VERSION"] = g.Gen1;
            psi.EnvironmentVariables["POKEPORT_BACKGROUND"] = "1";
            psi.EnvironmentVariables["POKEPORT_DRIVER"] = "src/nativemons/TownArt.lua";
            psi.EnvironmentVariables["NATIVEMONS_TOWN_ART"] = shot.Replace('\\', '/');
            string said = LauncherForm.RunCapture(psi, 240, out code);
            if (!File.Exists(shot)) return "No picture came out. " + said;
            return SaveTown(g, shot);
        } catch (Exception x) { return x.Message; }
        finally {
            try { if (File.Exists(shot)) File.Delete(shot); } catch { }
            try { if (Directory.Exists(identityFolder)) Directory.Delete(identityFolder, true); } catch { }
        }
    }
    static void Load(string id) {
        if (loadedId == id) return;
        var old = loaded; var oldLogo = loadedLogo;
        var next = new Bitmap[Count];
        for (int i = 0; i < Count; i++) using (var f = new Bitmap(FrameFile(id, i))) next[i] = new Bitmap(f);
        Bitmap logo = null;
        if (File.Exists(LogoFile(id))) using (var f = new Bitmap(LogoFile(id))) logo = new Bitmap(f);
        loaded = next; loadedLogo = logo; loadedId = id;
        if (old != null) foreach (var b in old) b.Dispose();
        if (oldLogo != null) oldLogo.Dispose();
    }
    // There and back, so the end meets the beginning without a jump.
    public static Bitmap Frame(string id, float t) {
        if (!Has(id)) return null;
        lock (gate) {
            try { Load(id); } catch { return null; }
            int n = (int)(t * 10) % (Count * 2 - 2);
            return loaded[n < Count ? n : Count * 2 - 2 - n];
        }
    }
    public static Bitmap Logo(string id) {
        if (!Has(id)) return null;
        lock (gate) { try { Load(id); return loadedLogo; } catch { return null; } }
    }
    public static void Draw(Graphics g, Bitmap pic, RectangleF r) {
        float scale = Math.Max(r.Width / pic.Width, r.Height / pic.Height), w = pic.Width * scale, h = pic.Height * scale;
        var st = g.Save();
        g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
        g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;
        g.DrawImage(pic, r.X + (r.Width - w) / 2, r.Y + (r.Height - h) / 2, w, h);
        g.Restore(st);
    }
    static long Distance(Bitmap a, Bitmap b) {
        long sum = 0;
        for (int y = 0; y < a.Height; y++) for (int x = 0; x < a.Width; x++) {
            Color p = a.GetPixel(x, y), q = b.GetPixel(x, y);
            sum += Math.Abs(p.R - q.R) + Math.Abs(p.G - q.G) + Math.Abs(p.B - q.B);
        }
        return sum;
    }
    // The widescreen view shows the game's two screens side by side, and some titles swap them. The logo is the half that looks
    // the same in every picture, which leaves the other as the moving art.
    static void Sort(string[] shots, string id) {
        const int thumbW = 32, thumbH = 24;
        var thumbs = new List<Bitmap>(); var sides = new List<bool>(); var sources = new List<int>();
        int bandTop = int.MaxValue, bandBottom = 0, width = 0, height = 0;
        foreach (var file in shots) using (var pic = new Bitmap(file)) {
            width = pic.Width; height = pic.Height;
            for (int y = 0; y < pic.Height; y += 4) {
                bool lit = false;
                for (int x = 0; x < pic.Width && !lit; x += 8) { var c = pic.GetPixel(x, y); lit = c.R + c.G + c.B > 24; }
                if (lit) { if (y < bandTop) bandTop = y; if (y > bandBottom) bandBottom = y; }
            }
        }
        if (bandTop >= bandBottom) throw new InvalidDataException("The title screen came out blank.");
        var band = new Rectangle(0, bandTop, width, Math.Min(height - bandTop, bandBottom - bandTop + 4));
        for (int i = 0; i < shots.Length; i++) using (var pic = new Bitmap(shots[i])) foreach (bool right in new[] { false, true }) {
            var half = new Rectangle(right ? width / 2 : 0, band.Y, width / 2, band.Height);
            var thumb = new Bitmap(thumbW, thumbH);
            using (var g = Graphics.FromImage(thumb)) g.DrawImage(pic, new Rectangle(0, 0, thumbW, thumbH), half, GraphicsUnit.Pixel);
            thumbs.Add(thumb); sides.Add(right); sources.Add(i);
        }
        int logo = 0; long best = long.MaxValue;
        for (int i = 0; i < thumbs.Count; i++) {
            long total = 0;
            var near = new List<long>();
            for (int j = 0; j < thumbs.Count; j += 3) near.Add(Distance(thumbs[i], thumbs[j]));
            near.Sort();
            for (int k = 0; k < near.Count / 2; k++) total += near[k];
            if (total < best) { best = total; logo = i; }
        }
        Directory.CreateDirectory(Folder(id));
        var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
        var quality = new System.Drawing.Imaging.EncoderParameters(1);
        quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 85L);
        for (int i = 0; i < shots.Length; i++) {
            bool leftIsLogo = Distance(thumbs[i * 2], thumbs[logo]) < Distance(thumbs[i * 2 + 1], thumbs[logo]);
            using (var pic = new Bitmap(shots[i])) {
                // Without the small print along the bottom, which would be huge once it fills the window.
                var art = new Rectangle(leftIsLogo ? width / 2 : 0, band.Y, width / 2, band.Height * 88 / 100);
                using (var cut = pic.Clone(art, pic.PixelFormat)) cut.Save(FrameFile(id, i), jpeg, quality);
            }
        }
        using (var pic = new Bitmap(shots[sources[logo]])) {
            var area = new Rectangle(sides[logo] ? width / 2 : 0, band.Y, width / 2, band.Height);
            using (var cut = pic.Clone(area, pic.PixelFormat)) cut.Save(LogoFile(id), System.Drawing.Imaging.ImageFormat.Png);
        }
        foreach (var t in thumbs) t.Dispose();
    }
    public static bool CanCapture(GameEntry g) {
        if (g.IsDs && g.FulldexCode == null) return g.OnNativeplat;
        if (g.IsDs) return g.ArtFrame > 0 && g.FulldexRom != null && File.Exists(Config.Fulldex) && (!Library.NeedsPatch(g.FulldexRom) || File.Exists(Config.Patcher));
        return g.Gen1 != null && (g.Imported || g.Gen1Rom != null);
    }
    // Game Boy and GBA games: Gen1Recomp shows its own title screen in a window kept out of sight, with the sound off. A single
    // screen has no logo half to sort, and whatever surrounds the game in the window is cut away.
    // A game that isn't prepared yet gets prepared under a throwaway identity, removed afterwards: preparing it for real would
    // turn on save sharing for a game the player may only use with Fulldex.
    const string ArtIdentity = "nativemons-art";
    static string CaptureGen1(GameEntry g) {
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots");
        bool throwaway = !g.Imported;
        string identityFolder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LOVE", ArtIdentity);
        try {
            if (throwaway) {
                var import = LauncherForm.Gen1Start("");
                import.EnvironmentVariables["POKEPORT_IDENTITY"] = ArtIdentity;
                import.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
                import.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
                using (var rom = new LauncherForm.PlainPath(g.Gen1Rom)) {
                    import.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = rom.Path_;
                    int code; LauncherForm.RunCapture(import, 240, out code);
                    if (code != 0) return "Could not prepare the game for its picture.";
                }
            }
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            var psi = LauncherForm.Gen1Start("");
            if (throwaway) psi.EnvironmentVariables["POKEPORT_IDENTITY"] = ArtIdentity;
            psi.EnvironmentVariables["POKEPORT_VERSION"] = g.Gen1;
            psi.EnvironmentVariables["POKEPORT_BACKGROUND"] = "1";
            psi.EnvironmentVariables["POKEPORT_DRIVER"] = "src/nativemons/TitleArt.lua";
            psi.EnvironmentVariables["NATIVEMONS_TITLE_ART"] = temp.Replace('\\', '/');
            using (var p = Process.Start(psi)) {
                try { p.PriorityClass = ProcessPriorityClass.Idle; } catch { }
                p.BeginOutputReadLine(); p.BeginErrorReadLine();
                if (!p.WaitForExit(240000)) { try { p.Kill(); } catch { } return "It took too long."; }
            }
            var shots = Directory.GetFiles(temp, "*.png").OrderBy(x => x, StringComparer.Ordinal).ToArray();
            if (shots.Length != Count) return "Only " + shots.Length + " pictures came out.";
            int left = int.MaxValue, top = int.MaxValue, right = 0, bottom = 0, width = 0, height = 0;
            foreach (var file in shots) using (var pic = new Bitmap(file)) {
                width = pic.Width; height = pic.Height;
                for (int y = 0; y < pic.Height; y += 2) for (int x = 0; x < pic.Width; x += 2) {
                    var c = pic.GetPixel(x, y);
                    if (c.R + c.G + c.B > 24) { left = Math.Min(left, x); top = Math.Min(top, y); right = Math.Max(right, x); bottom = Math.Max(bottom, y); }
                }
            }
            if (left >= right || top >= bottom) return "The title screen came out blank.";
            var area = Rectangle.FromLTRB(left, top, Math.Min(width, right + 2), Math.Min(height, bottom + 2));
            Directory.CreateDirectory(Folder(g.Id));
            var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
            var quality = new System.Drawing.Imaging.EncoderParameters(1);
            quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 85L);
            for (int i = 0; i < Count; i++) using (var pic = new Bitmap(shots[i])) using (var cut = pic.Clone(area, pic.PixelFormat)) cut.Save(FrameFile(g.Id, i), jpeg, quality);
            return null;
        } catch (Exception x) { return x.Message; }
        finally {
            try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { }
            try { if (throwaway && Directory.Exists(identityFolder)) Directory.Delete(identityFolder, true); } catch { }
        }
    }
    // A game only nativeplat runs: its test mode shows the title screen with no window or sound and keeps no save.
    static string CaptureNativeplat(GameEntry g) {
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots");
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            var psi = new ProcessStartInfo(Config.Nativeplat) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(Config.Nativeplat), RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            psi.EnvironmentVariables["SDL_VIDEO_DRIVER"] = "dummy";
            psi.EnvironmentVariables["SDL_AUDIO_DRIVER"] = "dummy";
            psi.EnvironmentVariables["NP_AUTOTEST"] = "frames=" + (1500 + Count * Step) + ",shots=" + Step + ",size=512x384,layout=top,game=" + g.Nativeplat + ",png=" + Path.Combine(temp, "s.png") + ",rom=" + LauncherForm.NativeplatRomCopy(g) + ",press=1200:start:10";
            using (var p = Process.Start(psi)) {
                try { p.PriorityClass = ProcessPriorityClass.Idle; } catch { }
                p.BeginOutputReadLine(); p.BeginErrorReadLine();
                if (!p.WaitForExit(180000)) { try { p.Kill(); } catch { } return "It took too long."; }
            }
            Func<string, int> number = delegate(string f) { string n = Path.GetFileNameWithoutExtension(f); int at = n.LastIndexOf('-'), v; return at >= 0 && int.TryParse(n.Substring(at + 1), out v) ? v : -1; };
            var shots = Directory.GetFiles(temp, "s-*.png").OrderBy(number).ToArray();
            if (shots.Length < Count) return "Only " + shots.Length + " pictures came out.";
            shots = shots.Skip(shots.Length - Count).ToArray();
            Directory.CreateDirectory(Folder(g.Id));
            var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
            var quality = new System.Drawing.Imaging.EncoderParameters(1);
            quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 85L);
            for (int i = 0; i < Count; i++) using (var pic = new Bitmap(shots[i])) pic.Save(FrameFile(g.Id, i), jpeg, quality);
            return null;
        } catch (Exception x) { return x.Message; }
        finally { try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { } }
    }
    // Null when done, or why not. It uses no save, at low priority, and the ROM is only read.
    public static string Capture(GameEntry g) {
        if (!g.IsDs) return CaptureGen1(g);
        if (g.FulldexCode == null) return CaptureNativeplat(g);
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots"), patched = temp + "-rom";
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            string dir = Config.FulldexDir, romFile = g.FulldexRom;
            // An encrypted ROM won't start. Its picture comes from a patched copy, and the player's file isn't touched.
            if (Library.NeedsPatch(romFile)) {
                if (Directory.Exists(patched)) Directory.Delete(patched, true);
                Directory.CreateDirectory(patched);
                romFile = Path.Combine(patched, "rom.nds");
                File.Copy(g.FulldexRom, romFile);
                File.SetAttributes(romFile, FileAttributes.Normal);
                var fix = new ProcessStartInfo(Config.Python, "\"" + Config.Patcher + "\" \"" + romFile + "\"") { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = Path.GetDirectoryName(Config.Patcher), RedirectStandardOutput = true, RedirectStandardError = true };
                int code; LauncherForm.RunCapture(fix, 120, out code);
                // The patcher backs up what it changes, and this copy's backup would stop the next copy being patched.
                try { File.Delete(Path.Combine(Path.GetDirectoryName(Config.Patcher), "backups", "rom.secure-area.bin")); } catch { }
                if (code != 0 || Library.NeedsPatch(romFile)) return "Could not patch a copy of the ROM for its picture.";
            }
            var at = Enumerable.Range(0, Count).Select(i => (g.ArtFrame + i * Step).ToString()).ToArray();
            string args = "\"" + romFile + "\" --no-save --config \"" + Path.Combine(dir, "settings.cfg") + "\" --jit --headless --compose 960x720 --shot-dir \"" + temp + "\" --shot-at " + string.Join(",", at) + " --frames " + (g.ArtFrame + Count * Step + 10);
            if (g.FulldexCode != "IPGE" && g.FulldexCode != "IPKE") args += " --funcs \"" + Path.Combine(dir, "data", "funcs", g.FulldexCode + ".txt") + "\"";
            var psi = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            using (var p = Process.Start(psi)) {
                try { p.PriorityClass = ProcessPriorityClass.Idle; } catch { }
                p.BeginOutputReadLine(); p.BeginErrorReadLine();
                if (!p.WaitForExit(180000)) { try { p.Kill(); } catch { } return "It took too long."; }
            }
            var shots = Directory.GetFiles(temp, "*.bmp").OrderBy(x => x, StringComparer.Ordinal).ToArray();
            if (shots.Length != Count) return "Only " + shots.Length + " pictures came out.";
            Sort(shots, g.Id);
            return null;
        } catch (Exception x) { return x.Message; }
        finally {
            try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { }
            try { if (Directory.Exists(patched)) Directory.Delete(patched, true); } catch { }
        }
    }
    // One game at a time, behind everything else. Anything the player starts simply waits or goes first.
    public static void CaptureMissing(Func<bool> busy) {
        foreach (var g in Library.Games.Where(x => CanCapture(x) && !Has(x.Id)).ToList()) {
            if (busy()) return;
            Capture(g);
        }
        foreach (var g in Library.Games.Where(x => CanCaptureTown(x) && !HasTown(x.Id)).ToList()) {
            if (busy()) return;
            CaptureTown(g);
        }
    }
}

// Settings holds a default for all games, and each game can follow it or choose its own. "own" leaves things as they are, and
// nothing is written for a game unless one of these says so. Most are kept inside each game's save. Engine ones aren't, so they
// work for every generation, and go in the engine's own settings when a game starts.
class GameOption {
    public string Key, Name, Note; public char Icon = '\uE713'; public string[] Values, Labels; public bool GameOnly, Engine; public int FirstGen = 1;
    public string Label(string value) { int i = Array.IndexOf(Values, value); return i >= 0 ? Labels[i] : value; }
}
static class GameOptions {
    public const string Own = "own", Follow = "follow";
    public static readonly GameOption[] All = {
        new GameOption { Key = "text", Name = "Text speed", Icon = '\uE8D2', Values = new[] { "slow", "mid", "fast" }, Labels = new[] { "Slow", "Medium", "Fast" } },
        new GameOption { Key = "scene", Name = "Battle scene", Icon = '\uE8B2', Values = new[] { "on", "off" }, Labels = new[] { "On", "Off" } },
        new GameOption { Key = "style", Name = "Battle style", Icon = '\uE8AB', Values = new[] { "shift", "set" }, Labels = new[] { "Shift", "Set" } },
        new GameOption { Key = "sound", Name = "Sound", Icon = '\uE7F5', Values = new[] { "mono", "stereo" }, Labels = new[] { "Mono", "Stereo" }, FirstGen = 2 },
        new GameOption { Key = "button", Name = "Button mode", Icon = '\uE7FC', Values = new[] { "normal", "alt", "lequalsa" }, Labels = new[] { "Normal", "Alternate (LR or Start = X)", "L = A" }, GameOnly = true, FirstGen = 3 },
        new GameOption { Key = "frame", Name = "Text frame", Icon = '\uE8A9', Values = Enumerable.Range(1, 20).Select(i => i.ToString()).ToArray(), Labels = Enumerable.Range(1, 20).Select(i => "Type " + i).ToArray(), GameOnly = true, FirstGen = 3 },
        new GameOption { Key = "vol", Name = "Volume", Icon = '\uE767', Engine = true, Values = new[] { "0", "20", "40", "60", "80", "100" }, Labels = new[] { "Off", "20%", "40%", "60%", "80%", "100%" }, Note = "Overall volume, on every engine." },
        new GameOption { Key = "music", Name = "Music volume", Icon = '\uE8D6', Engine = true, Values = new[] { "0", "20", "40", "60", "80", "100" }, Labels = new[] { "Off", "20%", "40%", "60%", "80%", "100%" }, Note = "On Gen1Recomp and nativeplat. Fulldex and the Platinum PC port only have the overall volume." },
        new GameOption { Key = "sfx", Name = "Sound effects", Icon = '\uE7F6', Engine = true, Values = new[] { "0", "20", "40", "60", "80", "100" }, Labels = new[] { "Off", "20%", "40%", "60%", "80%", "100%" }, Note = "On Gen1Recomp and nativeplat. Fulldex and the Platinum PC port only have the overall volume." },
        new GameOption { Key = "full", Name = "Fullscreen", Icon = '\uE740', Engine = true, Values = new[] { "on", "off" }, Labels = new[] { "On", "Off" }, Note = "On every engine." },
        new GameOption { Key = "wide", Name = "Widescreen", Icon = '\uE799', Engine = true, Values = new[] { "on", "off" }, Labels = new[] { "On", "Off" }, Note = "The world on Fulldex and nativeplat, and battles on Gen1Recomp. The Platinum PC port doesn't have it." },
        new GameOption { Key = "smooth", Name = "Smooth scaling", Icon = '\uE91B', Engine = true, Values = new[] { "on", "off" }, Labels = new[] { "On", "Off" }, Note = "Softens the picture on Fulldex and nativeplat, instead of sharp pixels." },
    };
    public static int Generation(GameEntry g) {
        if (g.Console == "Game Boy") return 1;
        if (g.Console == "Game Boy Color") return 2;
        if (g.Console == "Game Boy Advance") return 3;
        return g.Dex > 493 ? 5 : 4;
    }
    // Where a save's options are known. Gen 5's aren't yet.
    public static bool Available(GameEntry g) { return Generation(g) <= 4; }
    public static bool Has(GameEntry g, GameOption o) { return o.Engine || (Available(g) && Generation(g) >= o.FirstGen); }
    static string File_(GameEntry g) { return Path.Combine(Config.State, g == null ? "game-options.txt" : "game-options-" + g.Id + ".txt"); }
    static Dictionary<string, string> Read(GameEntry g) {
        var d = new Dictionary<string, string>();
        try { if (File.Exists(File_(g))) foreach (var l in File.ReadAllLines(File_(g))) { int e = l.IndexOf('='); if (e > 0) d[l.Substring(0, e)] = l.Substring(e + 1); } } catch { }
        // The first version of this setting had a file of its own.
        if (g == null && !d.ContainsKey("text")) { string old = Config.Read("text-speed.txt", Own); if (old != Own) d["text"] = old; }
        return d;
    }
    // What's stored for this place: for a game "follow" unless changed, for Settings "own" unless changed.
    public static string Stored(GameEntry g, GameOption o) {
        string v;
        return Read(g).TryGetValue(o.Key, out v) ? v : g == null || o.GameOnly ? Own : Follow;
    }
    public static string Next(GameEntry g, GameOption o) {
        var order = new List<string>();
        if (g != null && !o.GameOnly) order.Add(Follow);
        order.Add(Own); order.AddRange(o.Values);
        return order[(order.IndexOf(Stored(g, o)) + 1) % order.Count];
    }
    public static void Store(GameEntry g, GameOption o, string value) {
        var d = Read(g); d[o.Key] = value;
        Directory.CreateDirectory(Config.State);
        File.WriteAllLines(File_(g), d.Select(x => x.Key + "=" + x.Value).ToArray());
    }
    public static string Describe(GameEntry g, GameOption o, string value) {
        if (value == Follow) return "Global";
        return value == Own ? (g == null ? "Each game's own" : "This game's own") : o.Label(value);
    }
    // The settings this game should start with, as key=value.
    public static Dictionary<string, string> ForGame(GameEntry g) {
        var r = new Dictionary<string, string>();
        foreach (var o in All) {
            if (!Has(g, o)) continue;
            string v = Stored(g, o);
            if (v == Follow) v = Stored(null, o);
            if (v != Own) r[o.Key] = v;
        }
        return r;
    }
    public static bool IsEngine(string key) { return All.First(o => o.Key == key).Engine; }
    // A value in an INI file, keeping every other line. Engines fill in anything the file doesn't have.
    public static void SetIni(string file, string section, string key, string value) {
        var lines = File.Exists(file) ? File.ReadAllLines(file).ToList() : new List<string>();
        int start = lines.FindIndex(l => l.Trim().Equals("[" + section + "]", StringComparison.OrdinalIgnoreCase));
        if (start < 0) { lines.Add("[" + section + "]"); lines.Add(key + " = " + value); File.WriteAllLines(file, lines.ToArray()); return; }
        int end = start + 1;
        while (end < lines.Count && !lines[end].TrimStart().StartsWith("[")) end++;
        for (int i = start + 1; i < end; i++) {
            int eq = lines[i].IndexOf('=');
            if (eq > 0 && lines[i].Substring(0, eq).Trim().Equals(key, StringComparison.OrdinalIgnoreCase)) {
                lines[i] = lines[i].Substring(0, eq + 1) + (lines[i][eq - 1] == ' ' ? " " : "") + value;
                File.WriteAllLines(file, lines.ToArray());
                return;
            }
        }
        lines.Insert(end, key + " = " + value);
        File.WriteAllLines(file, lines.ToArray());
    }
}

// What tells one save from another. From PKHeX for a cartridge save, or read as text from a Gen1Recomp slot file.
class SaveInfo {
    public string Trainer = "", Party = "";
    public int Hours, Minutes, Badges = -1, Caught = -1;
    public string Played { get { return Hours + "h " + Minutes + "m"; } }
    public string TileLine { get { return (Trainer != "" ? Trainer + ", " : "") + Played; } }
    public string Details(DateTime saved) {
        var first = new List<string>();
        if (Trainer != "") first.Add("Trainer " + Trainer);
        first.Add(Played + " played");
        if (Badges >= 0) first.Add(Badges + (Badges == 1 ? " badge" : " badges"));
        if (Caught >= 0) first.Add(Caught + " caught");
        return string.Join(", ", first.ToArray()) + "\nParty: " + (Party != "" ? Party : "empty") + "\nLast saved " + saved.ToString("d MMM yyyy, HH:mm");
    }
    static string Title(string name) {
        var parts = name.Replace('_', ' ').ToLowerInvariant().Split(' ');
        for (int i = 0; i < parts.Length; i++) if (parts[i] != "") parts[i] = char.ToUpperInvariant(parts[i][0]) + parts[i].Substring(1);
        return string.Join(" ", parts);
    }
    // Slot files are plain nested tables, written with two spaces per level, so a line's indent says where it belongs.
    public static SaveInfo FromSlot(string file) {
        var info = new SaveInfo();
        string block = null, inner = null;
        int kanto = 0, johto = 0; bool sawBadges = false;
        string level = null; var party = new List<string>();
        foreach (var raw in File.ReadLines(file)) {
            string line = raw.TrimEnd();
            int indent = line.Length - line.TrimStart().Length;
            string text = line.Trim();
            if (indent == 2) {
                inner = null;
                block = text.EndsWith("= {") ? text.Substring(0, text.Length - 3).Trim() : null;
                int cut = text.IndexOf(" = ");
                if (cut > 0 && text.Substring(0, cut) == "playTime" && block == null) {
                    double seconds;
                    if (double.TryParse(text.Substring(cut + 3).TrimEnd(','), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out seconds)) { info.Hours = (int)(seconds / 3600); info.Minutes = (int)(seconds / 60) % 60; }
                }
                continue;
            }
            if (block == null) continue;
            if (indent == 4) {
                inner = text.EndsWith("= {") ? text.Substring(0, text.Length - 3).Trim() : null;
                if (block == "player" && text.StartsWith("name = ")) info.Trainer = Unquote(text.Substring(7));
                if (block == "player" && (text.StartsWith("badges = {") || text.StartsWith("kantoBadges = {"))) sawBadges = true;
                if (block == "playTime" && text.StartsWith("hours = ")) int.TryParse(text.Substring(8).TrimEnd(','), out info.Hours);
                if (block == "playTime" && text.StartsWith("minutes = ")) int.TryParse(text.Substring(10).TrimEnd(','), out info.Minutes);
                continue;
            }
            if (indent != 6) continue;
            if (block == "player" && inner == "badges" && text.Contains("= true")) johto++;
            if (block == "player" && inner == "kantoBadges" && text.Contains("= true")) kanto++;
            if (block == "pokedex" && (inner == "caught" || inner == "owned") && text.Contains("= true")) info.Caught = Math.Max(info.Caught, 0) + 1;
            if (block == "party") {
                if (text.StartsWith("level = ")) level = text.Substring(8).TrimEnd(',');
                if (text.StartsWith("species = ")) party.Add(Title(Unquote(text.Substring(10))) + (level != null ? " " + level : ""));
            }
        }
        if (sawBadges) info.Badges = johto + kanto;
        if (info.Caught < 0) info.Caught = 0;
        info.Party = string.Join(", ", party.ToArray());
        return info;
    }
    static string Unquote(string s) { return s.TrimEnd(',').Trim().Trim('"'); }
}

// A game's saves. One is in use, at the paths the engines expect; the rest are parked in state\save-sets\<game>\<number>.
static class SaveSets {
    public static string Root(GameEntry g) { return Path.Combine(Config.State, "save-sets", g.Id); }
    public static string Dir(GameEntry g, string set) { return Path.Combine(Root(g), set); }
    public static string Journal(GameEntry g) { return Path.Combine(Root(g), "switching.txt"); }
    public static string Active(GameEntry g) {
        try { string f = Path.Combine(Root(g), "active.txt"); if (File.Exists(f)) { string v = File.ReadAllText(f).Trim(); if (v != "") return v; } } catch { }
        return "1";
    }
    public static void SetActive(GameEntry g, string set) {
        Directory.CreateDirectory(Root(g));
        string f = Path.Combine(Root(g), "active.txt");
        File.WriteAllText(f + ".part", set);
        File.Copy(f + ".part", f, true); File.Delete(f + ".part");
    }
    public static List<string> All(GameEntry g) {
        var l = Directory.Exists(Root(g)) ? Directory.GetDirectories(Root(g)).Select(Path.GetFileName).ToList() : new List<string>();
        if (!l.Contains(Active(g))) l.Add(Active(g));
        int a, b;
        l.Sort((x, y) => int.TryParse(x, out a) && int.TryParse(y, out b) ? a.CompareTo(b) : string.CompareOrdinal(x, y));
        return l;
    }
    public static string Name(GameEntry g, string set) {
        try { string f = Path.Combine(Dir(g, set), "name.txt"); if (File.Exists(f)) { string v = File.ReadAllText(f).Trim(); if (v != "") return v; } } catch { }
        return "Save " + set;
    }
    public static void SetName(GameEntry g, string set, string name) {
        Directory.CreateDirectory(Dir(g, set));
        File.WriteAllText(Path.Combine(Dir(g, set), "name.txt"), name);
    }
    // Numbers are never reused, so a new save can't inherit a deleted one's backups.
    public static string Next(GameEntry g) {
        Directory.CreateDirectory(Root(g));
        string f = Path.Combine(Root(g), "next.txt");
        int n = 1, v;
        foreach (var s in All(g)) if (int.TryParse(s, out v) && v > n) n = v;
        try { if (File.Exists(f) && int.TryParse(File.ReadAllText(f).Trim(), out v) && v > n) n = v; } catch { }
        n++;
        File.WriteAllText(f, n.ToString());
        Directory.CreateDirectory(Dir(g, n.ToString()));
        return n.ToString();
    }
}

// The ROM folders and game list. Reads the game code from each ROM, and hashes only what Gen1Recomp needs.
static class Library {
    public static List<GameEntry> Games = new List<GameEntry>();
    public static string Note = "";
    public static bool Gen1Found;
    public static void Scan(Func<string, string> gen1Cli) {
        var list = new List<GameEntry>();
        var json = new JavaScriptSerializer().Deserialize<List<Dictionary<string, object>>>(File.ReadAllText(Config.GamesFile));
        Func<Dictionary<string, object>, string, string> str = delegate(Dictionary<string, object> d, string k) { object v; return d.TryGetValue(k, out v) ? v as string : null; };
        foreach (var d in json) {
            var e = new GameEntry { Id = str(d, "id"), Title = str(d, "title"), Console = str(d, "console"), Gen1 = str(d, "gen1"), FulldexCode = str(d, "fulldex"), Header = str(d, "header"), Save = str(d, "save"), Port = str(d, "port"), Nativeplat = str(d, "nativeplat") };
            object c, dx;
            if (d.TryGetValue("dex", out dx)) e.Dex = Convert.ToInt32(dx);
            object af; if (d.TryGetValue("artFrame", out af)) e.ArtFrame = Convert.ToInt32(af);
            if (d.TryGetValue("colors", out c)) {
                var hex = ((System.Collections.IEnumerable)c).Cast<object>().Select(x => (string)x).ToArray();
                if (hex.Length == 3) { e.Theme = Theme.Make(hex[0], hex[1], hex[2], str(d, "finish")); e.Theme.GameId = e.Id; }
            }
            list.Add(e);
        }
        Theme.Parade = list.Select(x => x.Theme).ToArray();

        // version -> accepted hashes and whether it is imported, from the engine itself.
        var accepted = new Dictionary<string, string>();
        string games = gen1Cli("games");
        Gen1Found = games != null;
        if (games == null) Note = "Gen1Recomp was not found, so its games are unavailable.";
        else foreach (var row in games.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 4) continue;
            var g = list.Find(x => x.Gen1 == f[0]);
            if (g != null) g.Imported = f[2] == "yes";
            foreach (var hash in f[3].Split(',')) accepted[hash] = f[0];
        }
        var folders = Config.RomFolders();
        folders.Add(Path.Combine(Config.FulldexDir ?? Config.Here, "roms"));
        foreach (var folder in folders) {
            if (!Directory.Exists(folder)) continue;
            foreach (var file in FilesUnder(folder)) {
                string ext = Path.GetExtension(file).ToLowerInvariant();
                if (ext != ".nds" && ext != ".gb" && ext != ".gbc" && ext != ".gba") continue;
                try { Sort(list, file, ext, accepted); } catch { }
            }
        }
        Games = list;
    }
    // One unreadable or over-long subfolder is skipped alone, instead of losing the whole folder.
    public static List<string> FilesUnder(string root) {
        var found = new List<string>();
        var left = new Stack<string>();
        left.Push(root);
        while (left.Count > 0) {
            string folder = left.Pop();
            try { found.AddRange(Directory.GetFiles(folder)); } catch { }
            try { foreach (var sub in Directory.GetDirectories(folder)) left.Push(sub); } catch { }
        }
        return found;
    }
    // Why a game can't start: a ROM with no engine to run it is a different problem from a missing or wrong ROM.
    public static string WhyNotPlayable(GameEntry g) {
        string rom = g.FulldexRom ?? g.FoundUnsupported;
        bool canRun = (g.Gen1 != null && Gen1Found) || (g.FulldexCode != null && File.Exists(Config.Fulldex)) || (g.Nativeplat != null && File.Exists(Config.Nativeplat));
        if (rom != null && !canRun) return "Found " + Path.GetFileName(rom) + ", but this copy of nativeMons doesn't have the engine to run it. Use the full download from the Releases page.";
        if (g.FoundUnsupported != null) return "Found " + Path.GetFileName(g.FoundUnsupported) + ", but it is not a version this game's engine accepts. Its SHA-1 is " + g.FoundSha1 + ". The README lists the accepted ones.";
        return "No ROM found. Add its folder under ROM folders.";
    }
    static string HeaderOf(string file, string ext) {
        using (var f = File.OpenRead(file)) {
            int offset = ext == ".nds" ? 0x0C : ext == ".gba" ? 0xAC : 0x134, count = ext == ".nds" || ext == ".gba" ? 4 : 16;
            if (f.Length < offset + count) return "";
            var b = new byte[count]; f.Seek(offset, SeekOrigin.Begin); f.Read(b, 0, count);
            return Encoding.ASCII.GetString(b).TrimEnd('\0', ' ');
        }
    }
    static string Sha1Of(string file) {
        using (var f = File.OpenRead(file)) using (var s = SHA1.Create())
            return BitConverter.ToString(s.ComputeHash(f)).Replace("-", "").ToLowerInvariant();
    }
    // nativeplat's own import refuses Platinum Rev 0, but pret's build files show the revisions differ only in the header's
    // revision byte and one GTS overlay, whose code nativeplat doesn't take from the ROM. Every game file is the same, and it
    // doesn't check the hash when a game starts.
    static readonly Dictionary<string, string[]> NativeplatDumps = new Dictionary<string, string[]> {
        { "diamond", new[] { "a46233d8b79a69ea87aa295a0efad5237d02841e" } }, { "pearl", new[] { "99083bf15ec7c6b81b4ba241ee10abd9e80999ac" } },
        { "platinum", new[] { "0862ec35b24de5c7e2dcb88c9eea0873110d755c", "ce81046eda7d232513069519cb2085349896dec7" } },
    };
    static void Sort(List<GameEntry> list, string file, string ext, Dictionary<string, string> accepted) {
        string header = HeaderOf(file, ext);
        var g = list.Find(x => x.Header != null && (ext == ".gb" || ext == ".gbc" ? header.StartsWith(x.Header) && x.Gen1 != null : header == x.Header));
        if (g == null) return;
        if (g.FulldexCode != null && g.FulldexRom == null) g.FulldexRom = file;
        // nativeplat only takes the exact dumps it was built against, so a game can have it or not depending on the ROM.
        if (g.Nativeplat != null && g.NativeplatRom == null) {
            string[] wanted;
            if (NativeplatDumps.TryGetValue(g.Nativeplat, out wanted) && wanted.Contains(Sha1Of(file))) g.NativeplatRom = file;
            else if (g.FulldexCode == null) { g.FoundUnsupported = file; g.FoundSha1 = Sha1Of(file); }
        }
        if (g.Gen1 != null) {
            string v;
            // A clean dump beats anything else with the same header, such as a ROM hack of that game.
            if (accepted.TryGetValue(Sha1Of(file), out v) && v == g.Gen1) { g.Gen1Rom = file; if (g.FulldexCode != null) g.FulldexRom = file; }
            else if (g.Gen1Rom == null) { g.FoundUnsupported = file; g.FoundSha1 = Sha1Of(file); }
        }
    }
    // Whether an .nds ROM still has its secure area encrypted, which stops it loading without a DS BIOS.
    public static bool NeedsPatch(string file) {
        if (file == null || !file.EndsWith(".nds", StringComparison.OrdinalIgnoreCase)) return false;
        try {
            using (var f = File.OpenRead(file)) {
                var head = new byte[0x24];
                if (f.Read(head, 0, head.Length) < head.Length) return false;
                uint off = BitConverter.ToUInt32(head, 0x20);
                if (off < 0x4000 || off >= 0x8000) return false;
                var mark = new byte[8];
                f.Seek(off, SeekOrigin.Begin); f.Read(mark, 0, 8);
                return !mark.SequenceEqual(new byte[] { 0xff, 0xde, 0xff, 0xe7, 0xff, 0xde, 0xff, 0xe7 });
            }
        } catch { return false; }
    }
}

// Pokemon pictures from pogo.hails.app, kept on disk because they rarely change. A picture is fetched only when it is
// missing or over 30 days old, and an old copy keeps working if the site can't be reached.
static class Sprites {
    const string Site = "https://pogo.hails.app/api/pokemon-sprite/";
    static readonly object gate = new object();
    static readonly Dictionary<string, Bitmap> loaded = new Dictionary<string, Bitmap>();
    static readonly HashSet<string> asked = new HashSet<string>();
    static readonly Queue<string> queue = new Queue<string>();
    static int workers;
    public static volatile int Arrived;   // goes up when a picture arrives, so the window can redraw
    static string Folder { get { return Path.Combine(Config.State, "sprite-cache"); } }
    static string FileOf(string key) { return Path.Combine(Folder, key.Replace('/', '-') + ".png"); }
    // key: "25" or "shiny/25". Null until it is ready; the first ask starts the fetch.
    public static Bitmap Get(string key) {
        lock (gate) {
            Bitmap b;
            if (loaded.TryGetValue(key, out b)) return b;
            if (!asked.Contains(key)) {
                // The whole evolution line comes together, this one first.
                string prefix = key.StartsWith("shiny/") ? "shiny/" : "", id = key.Substring(prefix.Length);
                string[] line;
                Enqueue(key);
                if (families.TryGetValue(id, out line)) foreach (var other in line) Enqueue(prefix + other);
            }
        }
        return null;
    }
    static void Enqueue(string key) {
        if (!asked.Add(key)) return;
        queue.Enqueue(key);
        if (workers < 3) { workers++; Task.Run((Action)Work); }
    }
    static void Work() {
        for (;;) {
            string key;
            lock (gate) { if (queue.Count == 0) { workers--; return; } key = queue.Dequeue(); }
            try { Fetch(key); } catch { }
        }
    }
    static void Fetch(string key) {
        Download(key);
        string file = FileOf(key);
        if (!File.Exists(file)) return;
        // A copy, so the file isn't kept open.
        Bitmap bmp;
        using (var fs = new MemoryStream(File.ReadAllBytes(file))) using (var image = Image.FromStream(fs)) bmp = new Bitmap(image);
        lock (gate) loaded[key] = bmp;
        Arrived++;
    }
    // True when a new copy was downloaded; a recent copy on disk is left alone.
    static bool Download(string key) {
        string file = FileOf(key);
        bool have = File.Exists(file);
        if (have && (DateTime.UtcNow - File.GetLastWriteTimeUtc(file)).TotalDays <= 30) return false;
        try {
            Directory.CreateDirectory(Folder);
            System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
            using (var web = new System.Net.WebClient()) {
                web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons";
                var data = web.DownloadData(Site + key + ".png");
                using (var check = new MemoryStream(data)) using (var image = Image.FromStream(check)) { }   // only keep real pictures
                string temp = file + "." + Guid.NewGuid().ToString("N") + ".part";
                File.WriteAllBytes(temp, data);
                File.Copy(temp, file, true);
                File.Delete(temp);
            }
            return true;
        } catch { return false; }
    }
    // Evolution families, so asking for one picture brings its whole line. Kept on disk after the first run.
    static readonly Dictionary<string, string[]> families = new Dictionary<string, string[]>();
    public static void LoadFamilies(Func<string> ask) {
        try {
            string f = Path.Combine(Config.State, "sprite-families.txt");
            string text = File.Exists(f) ? File.ReadAllText(f) : ask();
            if (string.IsNullOrWhiteSpace(text)) return;
            if (!File.Exists(f)) File.WriteAllText(f, text);
            lock (gate) foreach (var row in text.Split('\n')) {
                var ids = row.Trim().Split(',');
                foreach (var id in ids) if (id != "") families[id] = ids;
            }
        } catch { }
    }
    // Every picture for Generations 1 to 5, normal and shiny, for offline play. Safe to run again: it only fetches what's missing.
    public const int MaxDex = 649;
    public static void GetAll(Action<int, int, int, int> report) {
        var keys = new Queue<string>();
        for (int i = 1; i <= MaxDex; i++) { keys.Enqueue(i.ToString()); keys.Enqueue("shiny/" + i); }
        int total = keys.Count, done = 0, fresh = 0, failed = 0;
        var crew = new List<Task>();
        for (int w = 0; w < 4; w++) crew.Add(Task.Run((Action)delegate {
            for (;;) {
                string key;
                lock (keys) { if (keys.Count == 0) return; key = keys.Dequeue(); }
                bool got = Download(key);
                lock (keys) {
                    done++;
                    if (got) fresh++; else if (!File.Exists(FileOf(key))) failed++;
                    report(done, total, fresh, failed);
                }
            }
        }));
        Task.WaitAll(crew.ToArray());
    }
    public static void Wait(int seconds) {
        var until = DateTime.Now.AddSeconds(seconds);
        while (DateTime.Now < until) { lock (gate) { if (queue.Count == 0 && workers == 0) return; } System.Threading.Thread.Sleep(50); }
    }
}

// Keeps saves safe. Every save that's about to change is copied first, the copy is checked byte for byte, and only
// checked copies are kept. A game both engines can run shares one save: the cartridge file Fulldex uses.
static class SaveGuard {
    public static string Hash(string file) {
        if (file == null || !File.Exists(file)) return "";
        using (var f = File.OpenRead(file)) using (var s = SHA256.Create()) return BitConverter.ToString(s.ComputeHash(f)).Replace("-", "");
    }
    // Each save has its own backups, so one can't push out another's. The first keeps the original folder.
    public static string BackupFolder(GameEntry g) { return BackupFolder(g, SaveSets.Active(g)); }
    public static string BackupFolder(GameEntry g, string set) {
        string folder = Path.Combine(Config.State, "save-backups", g.Id);
        return set == "1" ? folder : Path.Combine(folder, "save-" + set);
    }
    public static string Bridge(string args, out int code) {
        var psi = new ProcessStartInfo(Config.MonBridge, args) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true, StandardOutputEncoding = Encoding.UTF8 };
        psi.EnvironmentVariables["NATIVEMONS_ORIGINALS"] = Path.Combine(Config.State, "pokemon-originals");
        psi.EnvironmentVariables["NATIVEMONS_NODE"] = Config.Node;
        psi.EnvironmentVariables["NATIVEMONS_OPENHOME"] = Config.OpenHomeTool;
        using (var p = Process.Start(psi)) {
            var e = p.StandardError.ReadToEndAsync(); var o = p.StandardOutput.ReadToEndAsync();
            if (!p.WaitForExit(60000)) { try { p.Kill(); } catch { } code = -1; return "That took too long and was stopped."; }
            code = p.ExitCode;
            return o.Result.Trim() != "" ? o.Result : e.Result;
        }
    }
    // Fulldex makes an empty file when a game first starts. Nothing has been saved in it yet, so it isn't damage.
    public static bool IsBlank(string file) {
        try {
            var data = File.ReadAllBytes(file);
            return data.Length > 0 && data.All(b => b == data[0]);
        } catch { return false; }
    }
    // Null when the save is healthy, otherwise what's wrong with it. Without the tool, nothing can be checked.
    public static string Problem(string file) {
        if (!File.Exists(Config.MonBridge) || IsBlank(file)) return null;
        int code; string o = Bridge("check \"" + file + "\"", out code);
        if (code == 0) return null;
        var f = o.Trim().Split('\t');
        return f.Length > 1 ? f[1] : o.Trim();
    }
    // A verified copy, skipped if the newest backup already holds these bytes. Damaged saves are kept too, and marked.
    public static string Backup(GameEntry g, string file, string reason) {
        if (file == null || !File.Exists(file)) return null;
        string folder = BackupFolder(g);
        Directory.CreateDirectory(folder);
        string hash = Hash(file);
        var newest = Directory.GetFiles(folder, "*.sav").OrderByDescending(x => x, StringComparer.Ordinal).FirstOrDefault();
        if (newest != null && Hash(newest) == hash) return newest;
        string mark = Problem(file) != null ? "-damaged" : "";
        string target = Path.Combine(folder, DateTime.Now.ToString("yyyyMMdd-HHmmss-fff") + "-" + reason + mark + ".sav");
        string temp = target + ".part";
        File.Copy(file, temp, true);
        if (Hash(temp) != hash) { File.Delete(temp); throw new IOException("A backup didn't match its save, so nothing was changed. Check the drive."); }
        File.Move(temp, target);
        // The newest 30 stay. The oldest healthy one is never the last to go, as only extras are removed.
        foreach (var old in Directory.GetFiles(folder, "*.sav").OrderByDescending(x => x, StringComparer.Ordinal).Skip(30)) { try { File.Delete(old); } catch { } }
        return target;
    }
    public static string[] Backups(GameEntry g) {
        string folder = BackupFolder(g);
        return Directory.Exists(folder) ? Directory.GetFiles(folder, "*.sav").OrderByDescending(x => x, StringComparer.Ordinal).ToArray() : new string[0];
    }
    public static void WriteInPlace(string source, string target) {
        string temp = target + ".part", hash = Hash(source);
        Directory.CreateDirectory(Path.GetDirectoryName(target));
        File.Copy(source, temp, true);
        if (Hash(temp) != hash) { File.Delete(temp); throw new IOException("A copy didn't match its source, so nothing was changed."); }
        File.Copy(temp, target, true);
        File.Delete(temp);
    }
}
class SyncResult { public bool Conflict, Changed, Failed; public string Note, Master, Gen1; }

// One menu row: greyed rows (On = false) can be selected to read why, but do nothing.
class Item {
    public string Text, Hint; public bool On = true; public Action Run;
    public string Info = ""; public string[] Records = new string[0];   // the detail panel
    public char Icon = '';
    public Theme Theme;   // a game's row: its colours
    public string Sprite; // a Pokemon's row: its picture (see Sprites)
    public string TileHint = "";   // a short second line on its grid tile, to tell similar rows apart
    public Item(string text, string hint, bool on, Action run) { Text = text; Hint = hint ?? ""; On = on; Run = run; }
}
class Page {
    public string Title; public List<Item> Items = new List<Item>(); public int Sel;
    public int TopRow; public float ScrollRow = -1;   // grid scrolling: where it's heading, and where it is now
    public string Body;          // a text page (notes, results) instead of a list
    public Action OnBack;        // null = pop the page
    public string Mono;          // a fixed-width block under the body
    public float BodyScroll;     // how far a long body has been scrolled, in pixels
    public bool Main;
    public Theme Theme;
}

class LauncherForm : Form {
    const ushort UP = 0x0001, DOWN = 0x0002, LEFT = 0x0004, RIGHT = 0x0008, A = 0x1000, B = 0x2000;
    // Term 1 of Gen1Recomp's LICENSE.MD sets the last sentence's exact wording, "at minimum", while we ship its engine. The rest says what it applies to.
    public const string Gen1Credit = "Parts of Gen1Recomp inspired this project and helped with research along the way. For the Gen1Recomp engine included here: Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).";
    const string Identity = "nativemons";   // Gen1Recomp's save folder, apart from any other install's saves

    int frame;
    readonly Stack<Page> pages = new Stack<Page>();
    readonly Timer tick = new Timer();
    readonly List<string> log = new List<string>();
    bool dirty;
    Process job; string jobTitle; bool jobIsGame, jobHid; DateTime jobStart, gameStart; float progress = -1; Action<int> jobDone;
    string waiting; bool starting; DateTime startSince; volatile string startStage;
    GameEntry current;
    ushort prevPad; int held; string lastAction = ""; bool lastFromPad; DateTime lastActionTime = DateTime.MinValue;
    Font monoFont, titleFont, itemFont, smallFont, heroFont, markFont, markLight, playFont, iconFont, iconSmall, itemBold, smallBold, bodyFont, splashMark, splashLight;

    readonly string playDirect;
    public LauncherForm() : this(null) { }
    // playId: start that game at once and close afterwards (--play <id>, for shortcuts).
    public LauncherForm(string playId) {
        playDirect = playId;
        Text = "nativeMons";
        // A device standing upright: no window frame, the window's own outline is the shell's.
        var area = Screen.PrimaryScreen.WorkingArea;
        int h = area.Height * 94 / 100;
        FormBorderStyle = FormBorderStyle.None;
        StartPosition = FormStartPosition.Manual; ClientSize = new Size(h * 60 / 100, h);
        Location = new Point(area.Left + (area.Width - Width) / 2, area.Top + (area.Height - Height) / 2);
        using (var outline = Round(new RectangleF(0, 0, ClientSize.Width, ClientSize.Height), CornerRadius(ClientSize.Width))) Region = new Region(outline);
        BackColor = Color.FromArgb(18, 20, 24); ForeColor = Color.White;
        DoubleBuffered = true; KeyPreview = true;
        try { Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath); } catch { }
        tick.Interval = 16; tick.Tick += delegate { Poll(); };
        Load += delegate { tick.Start(); };
        Shown += delegate { Start(); };
        MakeFonts(ClientSize.Width);
    }

    // Sizes follow the device's width.
    void MakeFonts(int width) {
        float h = Math.Max(420, width * 1.15f);
        foreach (var f in fitFonts.Values) f.Dispose();
        fitFonts.Clear();
        titleFont = new Font("Segoe UI Semibold", h / 24f, GraphicsUnit.Pixel);
        heroFont = new Font("Segoe UI Semibold", h / 17f, GraphicsUnit.Pixel);
        itemFont = new Font("Segoe UI", h / 34f, GraphicsUnit.Pixel);
        itemBold = new Font("Segoe UI Semibold", h / 34f, GraphicsUnit.Pixel);
        smallFont = new Font("Segoe UI", h / 52f, GraphicsUnit.Pixel);
        monoFont = new Font("Consolas", h / 50f, GraphicsUnit.Pixel);
        smallBold = new Font("Segoe UI Semibold", h / 52f, GraphicsUnit.Pixel);
        bodyFont = new Font("Segoe UI", h / 42f, GraphicsUnit.Pixel);
        markFont = new Font("Segoe UI Black", h / 30f, GraphicsUnit.Pixel);
        markLight = new Font("Segoe UI Light", h / 30f, GraphicsUnit.Pixel);
        splashMark = new Font("Segoe UI Black", h / 11f, GraphicsUnit.Pixel);
        splashLight = new Font("Segoe UI Light", h / 11f, GraphicsUnit.Pixel);
        playFont = new Font("Segoe UI Black", h / 26f, GraphicsUnit.Pixel);
        iconFont = new Font("Segoe MDL2 Assets", h / 26f, GraphicsUnit.Pixel);
        iconSmall = new Font("Segoe MDL2 Assets", h / 40f, GraphicsUnit.Pixel);
    }

    public void Snapshot(string file, string screen, int w, int h) {
        MakeFonts(w);
        try { Library.Scan(Gen1Cli); } catch (Exception e) { Library.Note = e.Message; }
        PickCurrent();
        int colon = screen.IndexOf(':');
        if (colon > 0) {
            string id = screen.Substring(colon + 1); screen = screen.Substring(0, colon);
            var preview = Library.Games.Find(g => g.Id == id);
            if (preview != null) current = preview;
        }
        if (screen == "splash") { starting = true; startSince = DateTime.Now.AddSeconds(-4); startStage = "Looking for your games"; }
        else pages.Push(MainPage());
        if (screen == "games") pages.Push(GamesPage());
        if (screen == "folders") pages.Push(FoldersPage());
        if (screen == "import") {
            var row = new List<FoundSave>();
            foreach (var folder in SaveFolders()) foreach (var path in Library.FilesUnder(folder)) {
                var one = IdentifySlot(path);
                if (one != null) { one.Info = ReadSaveInfo(path); row.Add(one); }
            }
            var page = new Page { Title = "Import saves" };
            foreach (var one in row) page.Items.Add(SaveRow(one));
            pages.Push(page);
        }
        if (screen == "saves" && current != null) pages.Push(SavesPage(current));
        if (screen == "folder-added") { firstSetup = true; pages.Push(FoldersPage()); pages.Push(FolderAddedPage(Config.RomFolders().FirstOrDefault() ?? "")); }
        if (screen == "credits") pages.Push(CreditsPage());
        var sample = Library.Games.Find(g => g.Playable) ?? (Library.Games.Count > 0 ? Library.Games[0] : null);
        if (screen == "game" && sample != null) pages.Push(GamePage(sample));
        if (screen == "mods") { var m = Library.Games.Find(g => g.OnGen1); if (m != null) pages.Push(ModsPage(m)); }
        if (screen == "sprites") {
            Sprites.LoadFamilies(BridgeFamilies);
            var sp = new Page { Title = "Pokemon FireRed: boxes", Theme = current != null ? current.Theme : null };
            foreach (var key in new[] { "25", "shiny/6", "150", "245", "384", "635" })
                sp.Items.Add(new Item("Dex " + key, "", true, null) { Icon = '\uE8AB', Sprite = key, Info = "A sample row." });
            pages.Push(sp);
            foreach (var it in sp.Items) Sprites.Get(it.Sprite);
            Sprites.Wait(30);
        }
        if (screen == "job") {
            job = new Process(); jobTitle = "Preparing Pokemon LeafGreen"; jobStart = DateTime.Now.AddSeconds(-14); progress = 0.62f;
            foreach (var l in new[] { "Reading the ROM", "Decoding maps", "Decoding Pokemon graphics" }) log.Add(l);
        }
        using (var bmp = new Bitmap(w, h)) {
            using (var g = Graphics.FromImage(bmp)) { Render(g, w, h); screenAt = selAt = -10; Render(g, w, h); }
            bmp.Save(file, System.Drawing.Imaging.ImageFormat.Png);
        }
    }

    // ---------- the engines ----------
    public static ProcessStartInfo Gen1Start(string args) {
        var psi = new ProcessStartInfo(Config.Love, "\"" + Config.Gen1 + "\"" + args) {
            UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = Config.Gen1,
            RedirectStandardOutput = true, RedirectStandardError = true };
        psi.EnvironmentVariables["POKEPORT_IDENTITY"] = Identity;
        return psi;
    }
    // Asks the Gen1Recomp fork something (src/nativemons/ModsCli.lua). Null when it can't run.
    public static string Gen1Cli(string command) {
        if (!File.Exists(Config.Love) || !File.Exists(Path.Combine(Config.Gen1, "main.lua"))) return null;
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = command;
        using (var p = Process.Start(psi)) {
            var err = p.StandardError.ReadToEndAsync();
            string o = p.StandardOutput.ReadToEnd();
            if (!p.WaitForExit(30000)) { try { p.Kill(); } catch { } return null; }
            return o;
        }
    }
    // The port's program is called main, like plenty of others, so only one started from its own folder counts.
    static bool PortRunning() {
        bool found = false;
        foreach (var p in Process.GetProcessesByName("main")) {
            try { if (string.Equals(p.MainModule.FileName, Config.PlatinumPort, StringComparison.OrdinalIgnoreCase)) found = true; } catch { }
            p.Dispose();
        }
        return found;
    }
    static string RunningGames() {
        var names = new List<string>();
        if (PortRunning()) names.Add("the Platinum PC port");
        var native = Process.GetProcessesByName("nativeplat");
        if (native.Length > 0) names.Add("nativeplat");
        foreach (var p in native) p.Dispose();
        foreach (var n in new[] { Path.GetFileNameWithoutExtension(Config.Fulldex), "lovec", "love" }) {
            var found = Process.GetProcessesByName(n);
            if (found.Length > 0) names.Add(n);
            foreach (var p in found) p.Dispose();
        }
        return string.Join(", ", names.ToArray());
    }

    // ---------- start ----------
    void Start() {
        Activate(); Native.Front(Handle);
        starting = true; startSince = DateTime.Now; startStage = "Looking for your games"; Invalidate();
        Task.Run(() => {
            string err = null;
            try { Library.Scan(Gen1Cli); RecoverSwitches(); } catch (Exception e) { err = e.Message; }
            // Title screens are gathered afterwards, without holding up the menu.
            Task.Run(() => { System.Threading.Thread.Sleep(3000); try { GameArt.CaptureMissing(() => job != null || RunningGames() != ""); } catch { } });
            Sprites.LoadFamilies(BridgeFamilies);
            int left = (int)((1.1 - (DateTime.Now - startSince).TotalSeconds) * 1000);
            if (left > 0) Task.Delay(left).Wait();
            return err;
        }).ContinueWith(t => BeginInvoke((Action)(() => FinishStart(t.Result))));
    }
    void FinishStart(string err) {
        if (ClientSize.Width > 0 && ClientSize.Height > 0) {
            splashShot = new Bitmap(ClientSize.Width, ClientSize.Height);
            using (var g = Graphics.FromImage(splashShot)) Render(g, ClientSize.Width, ClientSize.Height);
            outroAt = (float)clock.Elapsed.TotalSeconds;
        }
        starting = false;
        PickCurrent();
        pages.Push(MainPage());
        if (playDirect != null) {
            var direct = Library.Games.Find(g => g.Id == playDirect);
            if (direct != null && direct.Playable) { Play(direct); Invalidate(); return; }
            ShowText("Can't start " + playDirect, "That game is not in the list, or its ROM was not found.", delegate { Close(); });
            return;
        }
        if (err != null) ShowText("Something went wrong", err, null);
        else if (Config.RomFolders().Count == 0 && !Library.Games.Exists(g => g.Playable)) { firstSetup = true; pages.Push(FoldersPage()); }
        else AskAboutEncrypted();
        Invalidate();
    }
    void PickCurrent() {
        string last = Config.Read("last-game.txt", "");
        current = Library.Games.Find(g => g.Id == last && g.Playable) ?? Library.Games.Find(g => g.Playable);
    }
    // After a change: scan again behind a card, then redraw the main page and whichever page comes next.
    void Rescan(Action then) {
        Busy("Looking for your games", delegate { Library.Scan(Gen1Cli); return null; }, delegate(string e) {
            PickCurrent();
            ReplaceMain();
            if (then != null) then();
            AskAboutEncrypted();
        });
    }
    // Encrypted ROMs are raised as soon as they're found, not when someone tries to play. Each file is asked about once,
    // however the question is left; after that it stays flagged on its game's page, which can still patch it.
    static string PatchAskedFile { get { return Path.Combine(Config.State, "patch-asked.txt"); } }
    static List<string> PatchAsked() { try { return File.Exists(PatchAskedFile) ? File.ReadAllLines(PatchAskedFile).ToList() : new List<string>(); } catch { return new List<string>(); } }
    void AskAboutEncrypted() {
        var asked = PatchAsked();
        var found = Library.Games.Where(g => g.FulldexRom != null && Library.NeedsPatch(g.FulldexRom) && !asked.Contains(g.FulldexRom)).ToList();
        if (found.Count == 0) return;
        try { Directory.CreateDirectory(Config.State); File.AppendAllLines(PatchAskedFile, found.Select(g => g.FulldexRom)); } catch { }
        var p = new Page { Title = found.Count == 1 ? "Patch " + found[0].Title + "?" : "Patch " + found.Count + " games?" };
        p.Body = "Found encrypted: " + string.Join(", ", found.Select(g => g.Title).ToArray()) + ".\n\nFulldex can't load these without a DS BIOS until their secure area is decrypted. hails.nds-Patcher does that. It changes 2 KB of each ROM file and keeps the original bytes in a backup, so it can be undone.";
        p.Items.Add(new Item("Patch now", "Changes the ROM files, with backups.", File.Exists(Config.Patcher), delegate {
            pages.Pop();
            string files = string.Join(" ", found.Select(g => "\"" + g.FulldexRom + "\"").ToArray());
            RunTool("Patching", Config.Python, "\"" + Config.Patcher + "\" " + files, Path.GetDirectoryName(Config.Patcher), delegate(int code) {
                if (code != 0) ShowLog("Patching failed", "The patcher stopped with code " + code + ". It needs Python 3."); else ReplaceMain();
            });
        }) { Icon = '\uE72E' });
        p.Items.Add(new Item("Not now", "Nothing is changed. You can patch from the game's page.", true, delegate { pages.Pop(); }) { Icon = '\uE711' });
        pages.Push(p);
    }
    void ReplaceMain() {
        var all = new List<Page>(pages); all.Reverse(); pages.Clear();
        foreach (var pg in all) { if (pg.Main) { var m = MainPage(); m.Sel = Math.Min(pg.Sel, m.Items.Count - 1); pages.Push(m); } else pages.Push(pg); }
    }

    // ---------- pages ----------
    Page MainPage() {
        var p = new Page { Title = "nativeMons", Main = true };
        p.OnBack = delegate { };
        p.Items.Add(new Item(current != null ? "Play " + current.Title : "Play", current != null ? "Start " + current.Title + " with " + GameEntry.EngineName(current.Engine()) + "." : "Add a ROM folder first.", current != null, delegate { Play(current); }) { Icon = '' });
        int have = Library.Games.Count(g => g.Playable);
        p.Items.Add(new Item("Games", have + " of " + Library.Games.Count + " found in your ROM folders.", true, delegate { pages.Push(GamesPage()); }) { Icon = '' });
        p.Items.Add(new Item("My Collection", "Caught across your games.", have > 0 && File.Exists(Config.MonBridge), delegate { OpenCollection(); }) { Icon = '\uE8F1' });
        bool mods = Library.Games.Exists(g => g.OnGen1);
        p.Items.Add(new Item("Mods", mods ? "Mods for the Gen1Recomp games." : "Needs a Gen1Recomp game.", mods, delegate { pages.Push(ModGamesPage()); }) { Icon = '' });
        int folders = Config.RomFolders().Count;
        p.Items.Add(new Item("ROM folders", folders + (folders == 1 ? " folder" : " folders") + " searched for your ROMs.", true, delegate { pages.Push(FoldersPage()); }) { Icon = '' });
        p.Items.Add(new Item("Move Pokemon", "From one game to another.", File.Exists(Config.MonBridge) && have > 1, delegate { pages.Push(MoveFromPage()); }) { Icon = '\uE8AB' });
        p.Items.Add(new Item("Settings", "Game options, sprites, credits.", true, delegate { pages.Push(SettingsPage()); }) { Icon = '\uE713' });
        p.Items.Add(new Item("Exit", "Close the launcher.", true, delegate { Close(); }) { Icon = '' });
        return p;
    }
    // ---------- settings ----------
    // One place for both engines. Values are stored where each engine's own menus keep them, so the two always agree.
    Page SettingsPage() {
        var p = new Page { Title = "Settings" };
        p.Items.Add(new Item("Fulldex", "", File.Exists(Config.Fulldex), delegate { OpenEngineSettings("fulldex"); }) { Icon = '\uE7FC', Info = "DS and GBA games: widescreen, frame rate and resolution." });
        p.Items.Add(new Item("Gen1Recomp", "", File.Exists(Path.Combine(Config.Gen1, "main.lua")), delegate { OpenEngineSettings("gen1"); }) { Icon = '\uE7FC', Info = "Game Boy, Game Boy Color and GBA games: text speed and battles." });
        p.Items.Add(new Item("Import saves", "", File.Exists(Config.MonBridge), delegate { OpenImportSaves(); }) { Icon = '\uE896', Info = "Bring in saves from other emulators and folders, such as those beside your ROMs." });
        p.Items.Add(new Item("Export saves", "", Library.Games.Exists(g => g.Playable), delegate { pages.Push(ExportGamesPage()); }) { Icon = '\uE898', Info = "Send a save back to your 3DS, a flash cart or another emulator." });
        p.Items.Add(new Item("Download all sprites", "", true, delegate { GetAllSprites(); }) { Icon = '\uE896', Info = "Gets every Pokemon picture for offline play. Run it again at any time: it only fetches what's missing." });
        p.Items.Add(new Item("Game options", "", true, delegate { pages.Push(GameOptionsPage(null)); }) { Icon = '\uE8D2', Info = "Volume, fullscreen, text speed, battle style and more, for every game." });
        p.Items.Add(new Item("Credits", "", true, delegate { pages.Push(CreditsPage()); }) { Icon = '\uE946', Info = "The projects this is built from." });
        p.Items.Add(new Item("Last session", "", true, delegate { ShowText("Last session", Config.Read("last-session.txt", "No session yet."), null); }) { Icon = '\uE81C', Info = "How the last game ended." });
        return p;
    }
    Page GameOptionsPage(GameEntry g) {
        var p = new Page { Title = g == null ? "Game options" : "Options: " + g.Title };
        if (g != null) p.Theme = g.Theme;
        foreach (var option in GameOptions.All) {
            var o = option;
            if (g == null ? o.GameOnly : !GameOptions.Has(g, o)) continue;
            p.Items.Add(new Item(o.Name + ": " + GameOptions.Describe(g, o, GameOptions.Stored(g, o)), "Press to change.", true, delegate {
                GameOptions.Store(g, o, GameOptions.Next(g, o));
                var was = pages.Pop(); var again = GameOptionsPage(g); again.Sel = was.Sel; again.TopRow = was.TopRow; again.ScrollRow = was.ScrollRow; pages.Push(again);
            }) { Icon = o.Icon, Info = (o.Note != null ? o.Note + " " : "") + (g == null ? "Applies to every game when it starts, unless the game's own page says otherwise." + (o.Engine ? "" : " It replaces what's set in the game. Not on Black, White, Black 2 and White 2 yet.") : "Applies when this game starts. Global follows Game options in Settings.") });
        }
        return p;
    }
    // Called after the save is backed up and shared. Gen1Recomp is told at start instead (NATIVEMONS_OPTIONS). A save it can't
    // change is left alone and the game still starts. The sync record moves with it, so this never looks like a conflict.
    static void ApplyOptions(GameEntry g) {
        var wanted = GameOptions.ForGame(g);
        try { ApplyEngineOptions(g, wanted); } catch { }
        try {
            var inSave = wanted.Where(x => !GameOptions.IsEngine(x.Key)).Select(x => x.Key + "=" + x.Value).ToList();
            string master = MasterOf(g);
            if (inSave.Count == 0 || g.Engine() == "gen1" || !HasMaster(g) || !File.Exists(master) || SaveGuard.IsBlank(master) || !File.Exists(Config.MonBridge)) return;
            string before = SaveGuard.Hash(master);
            int code; SaveGuard.Bridge("options \"" + master + "\" " + string.Join(" ", inSave.ToArray()), out code);
            string after = SaveGuard.Hash(master);
            if (code != 0 || before == after) return;
            var st = ReadSync(g);
            string lastM, lastG, slot;
            st.TryGetValue("master", out lastM); st.TryGetValue("gen1", out lastG); st.TryGetValue("slot", out slot);
            if (lastM == before) WriteSync(g, after, lastG ?? "", slot ?? "");
        } catch { }
    }
    // Each engine's own settings file. Gen1Recomp's come in with NATIVEMONS_OPTIONS.
    static void ApplyEngineOptions(GameEntry g, Dictionary<string, string> w) {
        string v, engine = g.Engine();
        if (engine == "fulldex") {
            if (w.TryGetValue("vol", out v)) WriteFulldex("volume", v);
            if (w.TryGetValue("full", out v)) WriteFulldex("fullscreen", v == "on" ? "1" : "0");
            if (w.TryGetValue("wide", out v)) WriteFulldex("wide", v == "on" ? "1" : "0");
            if (w.TryGetValue("smooth", out v)) WriteFulldex("smooth", v == "on" ? "1" : "0");
        } else if (engine == "port") {
            string ini = Path.Combine(PortDir, "sim_config.ini");
            if (w.TryGetValue("vol", out v)) GameOptions.SetIni(ini, "General", "MasterVolume", (int.Parse(v) * 127 / 100).ToString());
            if (w.TryGetValue("full", out v)) GameOptions.SetIni(ini, "General", "FullScreen", v == "on" ? "true" : "false");
        } else if (engine == "nativeplat") {
            string ini = Path.Combine(NativeplatDir, "userdata", "options.ini");
            Directory.CreateDirectory(Path.GetDirectoryName(ini));
            if (w.TryGetValue("vol", out v)) GameOptions.SetIni(ini, "audio", "volume", v);
            if (w.TryGetValue("music", out v)) GameOptions.SetIni(ini, "game", "bgm_volume", v);
            if (w.TryGetValue("sfx", out v)) GameOptions.SetIni(ini, "game", "se_volume", v);
            if (w.TryGetValue("full", out v)) GameOptions.SetIni(ini, "video", "fullscreen", v == "on" ? "1" : "0");
            if (w.TryGetValue("wide", out v)) GameOptions.SetIni(ini, "game", "widescreen", v == "on" ? "1" : "0");
            if (w.TryGetValue("smooth", out v)) GameOptions.SetIni(ini, "video", "filter", v == "on" ? "linear" : "nearest");
        }
    }
    static string FulldexCfg { get { return Path.Combine(Config.FulldexDir, "settings.cfg"); } }
    static Dictionary<string, string> ReadFulldex() {
        var d = new Dictionary<string, string>();
        try { if (File.Exists(FulldexCfg)) foreach (var l in File.ReadAllLines(FulldexCfg)) { int e = l.IndexOf('='); if (e > 0) d[l.Substring(0, e)] = l.Substring(e + 1); } } catch { }
        return d;
    }
    // Changes one line, keeping every other setting in the file.
    static void WriteFulldex(string key, string value) {
        var lines = File.Exists(FulldexCfg) ? File.ReadAllLines(FulldexCfg).ToList() : new List<string>();
        int at = lines.FindIndex(l => l.StartsWith(key + "="));
        if (at >= 0) lines[at] = key + "=" + value; else lines.Add(key + "=" + value);
        Directory.CreateDirectory(Path.GetDirectoryName(FulldexCfg));
        File.WriteAllLines(FulldexCfg, lines.ToArray());
    }
    static string Gen1Ask(string command) {
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = command;
        int code; string o = RunCapture(psi, 30, out code);
        return code == 0 ? o : null;
    }
    static void WriteGen1(string key, string value) { Gen1Ask("option " + key + " " + value); }
    // A row that steps through its choices when pressed.
    Item Choice(string label, string info, string[] names, int current, Action<int> apply) {
        Item it = null; int at = Math.Max(0, current);
        it = new Item(label + ": " + names[at], "", true, delegate {
            at = (at + 1) % names.Length;
            it.Text = label + ": " + names[at];
            int chosen = at;
            Task.Run(delegate { try { apply(chosen); } catch { } });
        }) { Icon = '\uE713', Info = info };
        return it;
    }
    Item FulldexSwitch(Dictionary<string, string> cfg, string key, string label, bool fallback, string info) {
        string v; bool on = cfg.TryGetValue(key, out v) ? v == "1" : fallback;
        return Choice(label, info, new[] { "ON", "OFF" }, on ? 0 : 1, delegate(int i) { WriteFulldex(key, i == 0 ? "1" : "0"); });
    }
    void OpenEngineSettings(string group) {
        bool hasFulldex = File.Exists(Config.Fulldex), hasGen1 = File.Exists(Path.Combine(Config.Gen1, "main.lua"));
        var gen1 = new Dictionary<string, string>();
        Busy("Reading settings", delegate {
            if (hasGen1 && group != "fulldex") foreach (var row in (Gen1Ask("options") ?? "").Split('\n')) { var f = row.TrimEnd('\r').Split('\t'); if (f.Length == 2) gen1[f[0]] = f[1]; }
            return null;
        }, delegate(string e) {
            var cfg = hasFulldex ? ReadFulldex() : new Dictionary<string, string>();
            Func<string, string, string> g1 = delegate(string k, string d) { string v; return gen1.TryGetValue(k, out v) && v != "nil" ? v : d; };
            var p = new Page();
            if (group == "fulldex") {
                p.Title = "Fulldex";
                p.Items.Add(FulldexSwitch(cfg, "wide", "Widescreen world", true, "Draws the world out to the sides of a widescreen display."));
                p.Items.Add(FulldexSwitch(cfg, "fps60", "60 FPS", true, "Adds in-between frames for smoother motion."));
                p.Items.Add(FulldexSwitch(cfg, "smooth", "Smooth scaling", false, "Softens the picture instead of keeping sharp pixels."));
                p.Items.Add(FulldexSwitch(cfg, "wide_map", "Wide map loading", true, "Black, White, Black 2 and White 2: loads more of the map, so the sides don't run out of land."));
                p.Items.Add(FulldexSwitch(cfg, "busy_detail", "Lower res when busy", false, "Drops the resolution when your PC can't keep up."));
                p.Items.Add(FulldexSwitch(cfg, "show_fps", "FPS counter", false, "Shows frames per second in a corner."));
                string[] heights = { "720", "1080", "1440", "2160" };
                string lines; if (!cfg.TryGetValue("wide_lines", out lines)) lines = "1080";
                p.Items.Add(Choice("3D resolution", "How sharply the 3D world is drawn. Higher needs a stronger PC.", new[] { "720P", "1080P", "1440P", "4K" }, Array.IndexOf(heights, lines), delegate(int i) { WriteFulldex("wide_lines", heights[i]); }));
            } else {
                p.Title = "Gen1Recomp";
                string[] speeds = { "1", "3", "5" };
                p.Items.Add(Choice("Text speed", "How fast text appears in dialogue.", new[] { "FAST", "MEDIUM", "SLOW" }, Array.IndexOf(speeds, g1("textSpeed", "3")), delegate(int i) { WriteGen1("textSpeed", speeds[i]); }));
                p.Items.Add(Choice("Battle animations", "Plays the move animations in battle.", new[] { "ON", "OFF" }, g1("animations", "true") == "true" ? 0 : 1, delegate(int i) { WriteGen1("animations", i == 0 ? "true" : "false"); }));
                p.Items.Add(Choice("Battle layout", "The original screen shape, or a wider battle view.", new[] { "ORIGINAL", "WIDE" }, g1("battleLayout", "og") == "wide" ? 1 : 0, delegate(int i) { WriteGen1("battleLayout", i == 1 ? "wide" : "og"); }));
            }
            pages.Push(p);
        });
    }

    // ---------- one save for both engines ----------
    static string MasterOf(GameEntry g) { return Path.Combine(Config.FulldexDir, "saves", g.Save ?? ""); }
    public static bool Shares(GameEntry g) { return g.HasBoth && File.Exists(Config.MonBridge); }
    static string SyncFile(GameEntry g) { return Path.Combine(Config.State, "save-sync", g.Id + ".txt"); }
    static Dictionary<string, string> ReadSync(GameEntry g) {
        var d = new Dictionary<string, string>();
        try { if (File.Exists(SyncFile(g))) foreach (var l in File.ReadAllLines(SyncFile(g))) { int e = l.IndexOf('='); if (e > 0) d[l.Substring(0, e)] = l.Substring(e + 1); } } catch { }
        return d;
    }
    static void WriteSync(GameEntry g, string master, string gen1, string slot) {
        Directory.CreateDirectory(Path.GetDirectoryName(SyncFile(g)));
        string temp = SyncFile(g) + ".part";
        File.WriteAllText(temp, "master=" + master + "\ngen1=" + gen1 + "\nslot=" + slot + "\n");
        File.Copy(temp, SyncFile(g), true); File.Delete(temp);
    }
    // Gen1Recomp's save as a cartridge file. Null when it has none yet; "why" is set only when something went wrong.
    static string Gen1Export(GameEntry g, out string why) {
        why = null;
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = "export " + g.Gen1;
        int code; string o = RunCapture(psi, 90, out code).Trim();
        if (code == 0 && o != "") return o.Split('\n').Last().Trim().Replace('/', '\\');
        if (o.IndexOf("no save", StringComparison.OrdinalIgnoreCase) < 0) why = o != "" ? o : "Gen1Recomp couldn't export its save.";
        return null;
    }
    static string Gen1Slot(GameEntry g, string command) {
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = command;
        int code; string o = RunCapture(psi, 60, out code);
        return code == 0 ? o.Trim().Split('\n').Last().Trim() : null;
    }
    // Brings both engines level. Never throws, never replaces a save without a verified backup and a proven result, and
    // whatever happens the game can still start: a failure leaves both saves alone and is tried again next time.
    public static SyncResult SyncSave(GameEntry g, string prefer) {
        var r = new SyncResult();
        if (!Shares(g) || !g.Imported) return r;
        try {
            string why, gen1 = Gen1Export(g, out why);
            if (why != null) return Kept(r, why);
            string master = MasterOf(g);
            bool hasMaster = File.Exists(master) && !SaveGuard.IsBlank(master);
            string hm = SaveGuard.Hash(master), hg = SaveGuard.Hash(gen1);
            var st = ReadSync(g);
            string lastM, lastG, slot;
            st.TryGetValue("master", out lastM); st.TryGetValue("gen1", out lastG); st.TryGetValue("slot", out slot);
            bool known = lastM != null;
            bool mChanged = hasMaster && (!known || hm != lastM), gChanged = gen1 != null && (!known || hg != lastG);
            if (!hasMaster && gen1 == null) return r;
            if (hm == hg) { WriteSync(g, hm, hg, slot ?? ""); return r; }
            if (!mChanged && !gChanged) return r;
            string direction = mChanged && gChanged ? prefer : mChanged ? "fulldex" : "gen1";
            if (direction == null) { r.Conflict = true; r.Master = master; r.Gen1 = gen1; return r; }
            return direction == "fulldex" ? MasterToGen1(g, r, master, slot) : Gen1ToMaster(g, r, gen1, master, slot);
        } catch (Exception x) { return Kept(r, x.Message); }
    }
    static SyncResult Kept(SyncResult r, string why) {
        r.Failed = true;
        r.Note = "Your saves weren't shared this time, and neither was changed. It'll try again next time.\n\nWhy: " + why;
        return r;
    }
    static SyncResult Gen1ToMaster(GameEntry g, SyncResult r, string gen1, string master, string slot) {
        string problem = SaveGuard.Problem(gen1);
        if (problem != null) return Kept(r, "Gen1Recomp's save failed its check: " + problem);
        SaveGuard.Backup(g, master, "before-sync");
        SaveGuard.Backup(g, gen1, "gen1recomp");
        SaveGuard.WriteInPlace(gen1, master);
        int code; string o = SaveGuard.Bridge("compare \"" + gen1 + "\" \"" + master + "\"", out code);
        if (code != 0) return Kept(r, "The result didn't match: " + o.Trim());
        WriteSync(g, SaveGuard.Hash(master), SaveGuard.Hash(gen1), slot ?? "");
        r.Changed = true;
        return r;
    }
    static SyncResult MasterToGen1(GameEntry g, SyncResult r, string master, string slot) {
        string problem = SaveGuard.Problem(master);
        string source = master, repaired = null;
        if (problem != null) {
            // A wrong checksum may still be repairable. The repaired copy has to pass everything the original would.
            repaired = Path.Combine(Config.State, "save-sync", g.Id + "-repaired.sav");
            Directory.CreateDirectory(Path.GetDirectoryName(repaired));
            int rc; SaveGuard.Bridge("repair \"" + master + "\" \"" + repaired + "\"", out rc);
            if (rc != 0) return Kept(r, "The Fulldex save failed its check: " + problem);
            source = repaired;
        }
        string why, before = Gen1Export(g, out why);
        SaveGuard.Backup(g, master, "fulldex");
        SaveGuard.Backup(g, before, "before-sync");
        string active = Gen1Slot(g, "active " + g.Gen1);
        var last = TryImportProved(g, source, out why);
        // One more way in: the same save rewritten by PKHeX, which can get past a strict reader.
        if (last == null && repaired == null) {
            repaired = Path.Combine(Config.State, "save-sync", g.Id + "-repaired.sav");
            Directory.CreateDirectory(Path.GetDirectoryName(repaired));
            int rc; SaveGuard.Bridge("repair \"" + master + "\" \"" + repaired + "\"", out rc);
            string again = null; if (rc == 0) last = TryImportProved(g, repaired, out again);
            if (last == null && again != null) why = again;
        }
        if (last == null) {
            if (active != null && active != "nil") Gen1Slot(g, "activate " + g.Gen1 + " " + active);
            return Kept(r, why);
        }
        // The slot this sync made last time isn't needed now; the player's own slots are never removed.
        if (!string.IsNullOrEmpty(slot) && slot == active) Gen1Slot(g, "delete " + g.Gen1 + " " + slot);
        string after = Gen1Export(g, out why);
        WriteSync(g, SaveGuard.Hash(master), SaveGuard.Hash(after), last);
        if (repaired != null) try { File.Delete(repaired); } catch { }
        r.Changed = true;
        if (problem != null) r.Note = "Fulldex's save had a wrong checksum. It was repaired for Gen1Recomp, and the original is in your backups.";
        return r;
    }
    // Imports into a new slot and proves it matches. A slot that doesn't match is removed. Returns the slot, or null.
    static string TryImportProved(GameEntry g, string source, out string why) {
        var psi = Gen1Start("");
        int code; string slot;
        using (var plain = new PlainPath(source)) {
            psi.EnvironmentVariables["NATIVEMONS_CLI"] = "import " + g.Gen1 + " " + plain.Path_;
            slot = RunCapture(psi, 90, out code).Trim().Split('\n').Last().Trim();
        }
        if (code != 0) { why = "Gen1Recomp couldn't read it: " + slot; return null; }
        string exported = Gen1Export(g, out why);
        if (exported == null) { Gen1Slot(g, "delete " + g.Gen1 + " " + slot); why = why ?? "It couldn't be read back."; return null; }
        string o = SaveGuard.Bridge("compare \"" + source + "\" \"" + exported + "\"", out code);
        if (code != 0) { Gen1Slot(g, "delete " + g.Gen1 + " " + slot); why = "The result didn't match, so it wasn't used: " + o.Trim().Replace("\n", "; "); return null; }
        why = null;
        return slot;
    }
    // Runs the sync behind a card, asks if both engines' saves changed, then carries on whatever the outcome.
    void SyncThen(GameEntry g, Action then) {
        if (!Shares(g) || !g.Imported) { then(); return; }
        SyncResult result = null;
        Busy("Sharing your " + g.Title + " save", delegate { result = SyncSave(g, null); return null; }, delegate(string e) {
            if (result != null && result.Conflict) { pages.Push(ConflictPage(g, then)); Invalidate(); return; }
            string note = e ?? (result != null ? result.Note : null);
            if (note != null) { Config.Write("last-session.txt", g.Title + ": " + note); noticeAfter = note; }
            then();
        });
    }
    string noticeAfter;
    Page ConflictPage(GameEntry g, Action then) {
        var p = new Page { Title = "Which " + g.Title + " save?", Theme = g.Theme, Body = "Both engines' saves changed since they last matched, so they can't simply be merged. Choose the one to keep playing. The other is kept in your backups." };
        Action<string> use = delegate(string side) {
            pages.Pop();
            SyncResult result = null;
            Busy("Sharing your " + g.Title + " save", delegate { result = SyncSave(g, side); return null; }, delegate(string e) { string note = e ?? result.Note; if (note != null) noticeAfter = note; then(); });
        };
        p.Items.Add(new Item("Use the Fulldex save", "", true, delegate { use("fulldex"); }) { Icon = '\uE7FC' });
        p.Items.Add(new Item("Use the Gen1Recomp save", "", true, delegate { use("gen1"); }) { Icon = '\uE7FC' });
        p.Items.Add(new Item("Not now", "Starts the game without sharing. You'll be asked again.", true, delegate { pages.Pop(); then(); }) { Icon = '\uE711' });
        return p;
    }
    // The save a game is using right now, as a cartridge file.
    string CurrentSave(GameEntry g) { string why; return SaveOf(g, out why); }
    // After a game: back up, check for damage, share. The player hears only when there's something to decide.
    void AfterGame(GameEntry g, int code, Action done) {
        string damaged = null;
        SyncResult result = null;
        Busy("Checking your save", delegate {
            string file = CurrentSave(g);
            if (file != null) {
                try { SaveGuard.Backup(g, file, "played"); } catch { }
                damaged = SaveGuard.Problem(file);
            }
            if (damaged == null) result = SyncSave(g, null);
            return null;
        }, delegate(string e) {
            if (damaged != null) { pages.Push(DamagedPage(g, damaged, done)); Invalidate(); return; }
            if (result != null && result.Conflict) { pages.Push(ConflictPage(g, done)); Invalidate(); return; }
            string note = e ?? (result != null ? result.Note : null);
            if (note != null) noticeAfter = note;
            done();
        });
    }
    Page DamagedPage(GameEntry g, string why, Action done) {
        var good = SaveGuard.Backups(g).FirstOrDefault(x => !x.Contains("-damaged"));
        var p = new Page { Title = "Your " + g.Title + " save looks damaged", Theme = g.Theme, Body = why + (good != null ? "\n\nA healthy backup from " + BackupWhen(good) + " is available." : "\n\nThere's no healthy backup to go back to yet.") };
        if (good != null) p.Items.Add(new Item("Restore that backup", "", true, delegate { pages.Pop(); RestoreBackup(g, good, done); }) { Icon = '\uE72C' });
        p.Items.Add(new Item("Leave it for now", "Nothing is changed. The damaged save is in your backups.", true, delegate { pages.Pop(); done(); }) { Icon = '\uE711' });
        return p;
    }
    static string BackupWhen(string file) {
        DateTime when;
        string n = Path.GetFileName(file);
        return n.Length >= 15 && DateTime.TryParseExact(n.Substring(0, 15), "yyyyMMdd-HHmmss", null, System.Globalization.DateTimeStyles.None, out when) ? when.ToString("d MMM yyyy, HH:mm") : n;
    }
    Page BackupsPage(GameEntry g) {
        var p = new Page { Title = g.Title + " backups", Theme = g.Theme };
        foreach (var file in SaveGuard.Backups(g)) {
            string f = file; bool bad = f.Contains("-damaged");
            p.Items.Add(new Item(BackupWhen(f), "", !bad, delegate { RestoreBackup(g, f, delegate { }); }) { Icon = '\uE81C', Info = bad ? "This one failed its check, and is kept in case it's ever needed." : "Select to go back to this save. Your current save is backed up first.", Records = new[] { "File: " + Path.GetFileName(f) } });
        }
        if (p.Items.Count == 0) p.Items.Add(new Item("No backups yet", "", false, null) { Icon = '\uE81C', Info = "A backup is made whenever a save is about to change." });
        return p;
    }
    void RestoreBackup(GameEntry g, string file, Action done) {
        string problem = null;
        Busy("Restoring your save", delegate {
            try {
                problem = SaveGuard.Problem(file);
                if (problem != null) { problem = "That backup failed its check: " + problem; return null; }
                string current = CurrentSave(g);
                SaveGuard.Backup(g, current, "before-restore");
                if (g.Engine() != "gen1" || Shares(g)) SaveGuard.WriteInPlace(file, MasterOf(g));
                if (g.Engine() == "gen1") problem = ImportBack(g, file);
                if (problem == null && Shares(g)) { var r = SyncSave(g, "fulldex"); problem = r.Failed ? r.Note : null; }
            } catch (Exception x) { problem = x.Message; }
            return null;
        }, delegate(string e) { ShowText(e == null && problem == null ? "Restored" : "Not restored", e ?? problem ?? "Your " + g.Title + " save is back to that backup.", null); done(); });
    }

    // ---------- several saves per game ----------
    static bool HasMaster(GameEntry g) { return (g.FulldexCode != null || g.Nativeplat != null) && g.Save != null; }
    static bool HasGen1Slots(GameEntry g) { return g.Gen1 != null && g.Imported; }
    static string ParkedSave(GameEntry g, string set) { return Path.Combine(SaveSets.Dir(g, set), "fulldex.sav"); }
    static string ParkedSync(GameEntry g, string set) { return Path.Combine(SaveSets.Dir(g, set), "sync.txt"); }
    static string ParkedSlot(GameEntry g, string set) { return Path.Combine(SaveSets.Dir(g, set), "gen1.txt"); }
    static void CopyOrRemove(string source, string target) {
        if (File.Exists(source)) SaveGuard.WriteInPlace(source, target);
        else if (File.Exists(target)) File.Delete(target);
    }
    static string ReadSlot(GameEntry g, string set) {
        try { string f = ParkedSlot(g, set); if (File.Exists(f)) { string v = File.ReadAllText(f).Trim(); if (v != "" && v != "nil") return v; } } catch { }
        return null;
    }
    static void Park(GameEntry g, string set) {
        Directory.CreateDirectory(SaveSets.Dir(g, set));
        if (HasMaster(g)) CopyOrRemove(MasterOf(g), ParkedSave(g, set));
        CopyOrRemove(SyncFile(g), ParkedSync(g, set));
        if (HasGen1Slots(g)) {
            string slot = Gen1Slot(g, "active " + g.Gen1);
            if (slot == null) throw new IOException("Gen1Recomp couldn't say which of its save slots is in use.");
            File.WriteAllText(ParkedSlot(g, set), slot);
        }
    }
    // Puts a parked save in use. A save with nothing in it yet starts a new game in each engine.
    static void BringIn(GameEntry g, string set) {
        if (HasMaster(g)) CopyOrRemove(ParkedSave(g, set), MasterOf(g));
        Directory.CreateDirectory(Path.GetDirectoryName(SyncFile(g)));
        CopyOrRemove(ParkedSync(g, set), SyncFile(g));
        if (HasGen1Slots(g)) {
            string slot = ReadSlot(g, set);
            if (slot != null) { if (Gen1Slot(g, "activate " + g.Gen1 + " " + slot) == null) throw new IOException("Gen1Recomp couldn't switch to that save's slot."); }
            else {
                slot = Gen1Slot(g, "new " + g.Gen1);
                if (string.IsNullOrEmpty(slot)) throw new IOException("Gen1Recomp couldn't make a new save slot.");
                File.WriteAllText(ParkedSlot(g, set), slot);
            }
        }
        SaveSets.SetActive(g, set);
    }
    // Nothing is shared during a switch: each save is parked exactly as it stands, even mid-conflict, and meets it again later.
    // The save being left is parked and proven first, so a failure from then on can put everything back.
    public static string SwitchSave(GameEntry g, string target) {
        string from = SaveSets.Active(g);
        if (target == from) return null;
        string running = RunningGames();
        if (running != "") return "Close " + running + " first, so nothing is writing to the save.";
        try {
            if (HasMaster(g)) SaveGuard.Backup(g, MasterOf(g), "before-switch");
            Park(g, from);
        } catch (Exception x) { return "Nothing was changed. " + x.Message; }
        try {
            File.WriteAllText(SaveSets.Journal(g), from);
            BringIn(g, target);
            File.Delete(SaveSets.Journal(g));
            return null;
        } catch (Exception x) {
            try { RollBack(g, from); return "You're still on your previous save. " + x.Message; }
            catch (Exception y) { return "The switch failed and couldn't be undone yet. It'll try again next time the launcher starts. " + x.Message + " " + y.Message; }
        }
    }
    static void RollBack(GameEntry g, string from) {
        BringIn(g, from);
        File.Delete(SaveSets.Journal(g));
    }
    // A switch cut short, by a crash or a power cut, is undone at the next start.
    public static void RecoverSwitches() {
        foreach (var g in Library.Games) {
            try {
                if (!File.Exists(SaveSets.Journal(g))) continue;
                RollBack(g, File.ReadAllText(SaveSets.Journal(g)).Trim());
            } catch { }
        }
    }
    public static string NewSave(GameEntry g, string name) {
        string set = SaveSets.Next(g);
        if (!string.IsNullOrWhiteSpace(name)) SaveSets.SetName(g, set, name);
        string problem = SwitchSave(g, set);
        if (problem != null) { try { Directory.Delete(SaveSets.Dir(g, set), true); } catch { } }
        return problem;
    }
    // A separate copy, which starts its own story from here. The save in use stays in use.
    public static string DuplicateSave(GameEntry g, string source) {
        string set = null;
        try {
            bool active = source == SaveSets.Active(g);
            set = SaveSets.Next(g);
            SaveSets.SetName(g, set, SaveSets.Name(g, source) + " copy");
            if (HasMaster(g)) CopyOrRemove(active ? MasterOf(g) : ParkedSave(g, source), ParkedSave(g, set));
            CopyOrRemove(active ? SyncFile(g) : ParkedSync(g, source), ParkedSync(g, set));
            if (HasGen1Slots(g)) {
                string slot = active ? Gen1Slot(g, "active " + g.Gen1) : ReadSlot(g, source);
                // A slot with nothing saved in it has nothing to copy, and the copy gets a new one when first used.
                if (!string.IsNullOrEmpty(slot) && slot != "nil") {
                    string copy = Gen1Slot(g, "duplicate " + g.Gen1 + " " + slot);
                    if (!string.IsNullOrEmpty(copy)) File.WriteAllText(ParkedSlot(g, set), copy);
                }
            }
            return null;
        } catch (Exception x) {
            if (set != null) try { Directory.Delete(SaveSets.Dir(g, set), true); } catch { }
            return x.Message;
        }
    }
    // Only a save not in use. Its Fulldex file goes to its backups, and Gen1Recomp's slot stays, as the player's slots always do.
    public static string DeleteSave(GameEntry g, string set) {
        if (set == SaveSets.Active(g)) return "That save is in use. Switch to another one first.";
        try {
            string parked = ParkedSave(g, set);
            if (File.Exists(parked)) {
                string folder = SaveGuard.BackupFolder(g, set);
                Directory.CreateDirectory(folder);
                string kept = Path.Combine(folder, DateTime.Now.ToString("yyyyMMdd-HHmmss-fff") + "-deleted.sav");
                SaveGuard.WriteInPlace(parked, kept);
            }
            Directory.Delete(SaveSets.Dir(g, set), true);
            return null;
        } catch (Exception x) { return x.Message; }
    }
    // Where Gen1Recomp keeps a slot, for reading what's in it.
    static string Gen1SlotFile(GameEntry g, string slot) {
        try {
            if (string.IsNullOrEmpty(slot) || slot == "nil" || !HasGen1Slots(g)) return null;
            string paths = Gen1Cli("paths");
            var row = paths == null ? null : paths.Split('\n').Select(x => x.TrimEnd('\r').Split('\t')).FirstOrDefault(x => x.Length == 2 && x[0] == "save");
            return row == null ? null : Path.Combine(row[1].Replace('/', '\\'), "saves", g.Gen1, slot + ".lua");
        } catch { return null; }
    }
    Page SavesPage(GameEntry g) {
        var p = new Page { Title = g.Title + " saves", Theme = g.Theme };
        string active = SaveSets.Active(g);
        foreach (var one in SaveSets.All(g)) {
            string set = one; bool inUse = set == active;
            string file = !HasMaster(g) ? Gen1SlotFile(g, inUse ? Gen1Slot(g, "active " + g.Gen1) : ReadSlot(g, set)) : inUse ? MasterOf(g) : ParkedSave(g, set);
            var saved = ReadSaveInfo(file);
            string what = saved != null ? saved.Details(File.GetLastWriteTime(file)) : "Nothing saved here yet, or it's only in Gen1Recomp.";
            if (inUse) p.Sel = p.Items.Count;
            p.Items.Add(new Item(SaveSets.Name(g, set), "", true, delegate { pages.Push(SavePage(g, set)); }) { Icon = '\uE896', Theme = g.Theme, Info = (inUse ? "In use now.\n" : "") + what, TileHint = saved != null ? saved.TileLine : "" });
        }
        p.Items.Add(new Item("New save", "", true, delegate {
            string name = AskText("New save", "Name for the new save:", "");
            if (name == null) return;
            SaveAction(g, "Making a new save", delegate { return NewSave(g, name); }, 1);
        }) { Icon = '', Info = "Starts this game from the beginning. Your other saves stay as they are." });
        return p;
    }
    Page SavePage(GameEntry g, string set) {
        bool inUse = set == SaveSets.Active(g);
        var p = new Page { Title = SaveSets.Name(g, set), Theme = g.Theme, Body = inUse ? "This save is in use." : "Switching keeps your current save exactly as it is, ready to come back to." };
        if (!inUse) p.Items.Add(new Item("Use this save", "Play will use it from now on.", true, delegate { SaveAction(g, "Switching saves", delegate { return SwitchSave(g, set); }, 2); }) { Icon = '' });
        p.Items.Add(new Item("Rename", "", true, delegate {
            string name = AskText("Rename", "New name:", SaveSets.Name(g, set));
            if (name != null) SaveAction(g, "Renaming", delegate { SaveSets.SetName(g, set, name); return null; }, 2);
        }) { Icon = '' });
        p.Items.Add(new Item("Copy", "A separate save that starts from this point.", true, delegate { SaveAction(g, "Copying the save", delegate { return DuplicateSave(g, set); }, 2); }) { Icon = '' });
        if (!inUse) p.Items.Add(new Item("Delete", "Removes it from this list.", true, delegate {
            var sure = new Page { Title = "Delete " + SaveSets.Name(g, set) + "?", Theme = g.Theme, Body = "It leaves this list. A last copy stays in the game's backups folder, and Gen1Recomp keeps its slot." };
            sure.Items.Add(new Item("Keep it", "", true, delegate { pages.Pop(); }) { Icon = '' });
            sure.Items.Add(new Item("Delete it", "", true, delegate { SaveAction(g, "Deleting the save", delegate { return DeleteSave(g, set); }, 3); }) { Icon = '' });
            pages.Push(sure);
        }) { Icon = '' });
        return p;
    }
    // Runs a change behind a card, then returns to a fresh list of the game's saves, or says what went wrong.
    void SaveAction(GameEntry g, string title, Func<string> work, int popCount) {
        string problem = null; Page fresh = null;
        Busy(title, delegate { problem = work(); fresh = SavesPage(g); return null; }, delegate(string e) {
            for (int i = 0; i < popCount && pages.Count > 1; i++) pages.Pop();
            if (fresh != null) pages.Push(fresh);
            ReplaceMain();
            if (e != null || problem != null) ShowText("That didn't work", e ?? problem, null);
            Invalidate();
        });
    }
    void OpenSaves(GameEntry g) {
        Page saves = null;
        Busy("Reading your saves", delegate { saves = SavesPage(g); return null; }, delegate(string e) {
            if (e != null) { ShowText("Saves", e, null); return; }
            pages.Push(saves); Invalidate();
        });
    }
    string AskText(string title, string prompt, string start) {
        using (var f = new Form { Text = title, FormBorderStyle = FormBorderStyle.FixedDialog, StartPosition = FormStartPosition.CenterParent, MinimizeBox = false, MaximizeBox = false, ClientSize = new Size(340, 110) })
        using (var label = new Label { Text = prompt, Left = 12, Top = 12, Width = 316 })
        using (var box = new TextBox { Text = start, Left = 12, Top = 36, Width = 316, MaxLength = 40 })
        using (var ok = new Button { Text = "OK", Left = 172, Top = 72, Width = 75, DialogResult = DialogResult.OK })
        using (var cancel = new Button { Text = "Cancel", Left = 253, Top = 72, Width = 75, DialogResult = DialogResult.Cancel }) {
            f.Controls.AddRange(new Control[] { label, box, ok, cancel });
            f.AcceptButton = ok; f.CancelButton = cancel;
            if (f.ShowDialog(this) != DialogResult.OK) return null;
            string v = box.Text.Replace("\r", " ").Replace("\n", " ").Trim();
            return v;
        }
    }

    // ---------- exporting saves ----------
    // Everything needed to put a save back on a device. Copies byte for byte: the file is already in cartridge format.
    public static string ExportSave(GameEntry g, bool checkpoint, string outFolder, string backup, out bool ok) {
        ok = false;
        string why, file = SaveOf(g, out why);
        if (file == null) return why;
        int code; string info = Bridge("info \"" + file + "\"", out code);
        if (code != 0) return "This save didn't pass its checks, so it wasn't exported: " + info.Trim();
        file = file.Replace('/', '\\');
        long size = new FileInfo(file).Length;
        if (!checkpoint) {
            Directory.CreateDirectory(outFolder);
            string target = Path.Combine(outFolder, g.Id + ".sav");
            if (File.Exists(target)) File.Copy(target, target + ".old", true);
            File.Copy(file, target, true);
            ok = true;
            return "Exported to:\n" + target + "\n\nThis is a plain cartridge save (" + size + " bytes). Rename it to match your ROM if your emulator or flash cart needs that.";
        }
        string fileName = "sav.dat";
        string secure = null;
        if (g.IsDs) {
            if (backup == null) return "DS saves need a Checkpoint backup of this game, because Checkpoint names the file after the game on your 3DS. Back the game up in Checkpoint, copy that folder to this PC and choose it.";
            fileName = Directory.GetFiles(backup, "*.sav").Select(Path.GetFileName).FirstOrDefault();
            if (fileName == null) return "That folder has no .sav file, so it isn't a Checkpoint backup of a DS game.";
        } else if (g.Console != "Game Boy" && g.Console != "Game Boy Color") {
            return "Game Boy Advance games aren't on the 3DS as Virtual Console, so Checkpoint can't restore them. Use a plain .sav for flash carts or open_agb_firm.";
        }
        if (backup != null) {
            string reference = Path.Combine(backup, fileName);
            if (!File.Exists(reference)) return "That folder has no " + fileName + ", so it isn't a Checkpoint backup of this game.";
            long expected = new FileInfo(reference).Length;
            if (expected != size) return "Sizes don't match, so nothing was exported: your 3DS backup is " + expected + " bytes and this save is " + size + ". It could report as corrupted on the 3DS. Check that the backup is of the same game and region.";
            string sv = Path.Combine(backup, "SecureValue");
            if (File.Exists(sv)) secure = sv;
        }
        // Virtual Console saves are 32 KB, plus a 16-byte clock on Crystal. Anything longer carries another emulator's clock.
        if (!g.IsDs && size != 32768 && size != 32784)
            return "This save came from another emulator: at " + size + " bytes it has that emulator's clock on the end, and Virtual Console expects 32768 or 32784. It could report as corrupted on the 3DS, so nothing was exported. A plain .sav still works everywhere else.";
        string folder = Path.Combine(outFolder, "nativeMons-" + g.Id);
        Directory.CreateDirectory(folder);
        File.Copy(file, Path.Combine(folder, fileName), true);
        if (secure != null) File.Copy(secure, Path.Combine(folder, "SecureValue"), true);
        ok = true;
        return "Exported to:\n" + folder + "\n\nTo restore it:\n1. Over FTP, copy this folder to /3ds/Checkpoint/saves/<your game's folder>/, with the FTP client in binary mode.\n2. Check " + fileName + " on the 3DS is " + size + " bytes.\n3. In Checkpoint, select this backup, choose Restore, then start the game."
            + (backup == null ? "\n\nNo 3DS backup was given to compare with, so the size wasn't checked." : "") + "\n\nBack up your 3DS save first.";
    }
    Page ExportGamesPage() {
        var p = new Page { Title = "Export saves: which game?" };
        foreach (var game in Library.Games.Where(x => x.Playable)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { pages.Push(ExportPage(g)); }) { Icon = '\uE898', Theme = g.Theme, Info = "Uses this game's save from " + GameEntry.EngineName(g.Engine()) + "." });
        }
        return p;
    }
    Page ExportPage(GameEntry g) {
        var p = new Page { Title = "Export " + g.Title, Theme = g.Theme };
        p.Items.Add(new Item("Plain .sav file", "", true, delegate { RunExport(g, false, null); }) { Icon = '\uE898', Info = "For emulators, flash carts and PKHeX." });
        bool vc = g.Console == "Game Boy" || g.Console == "Game Boy Color";
        if (vc) p.Items.Add(new Item("3DS Virtual Console", "", true, delegate { RunExport(g, true, null); }) { Icon = '\uE898', Info = "Makes a Checkpoint folder holding sav.dat, for restoring to the 3DS over FTP." });
        if (vc || g.IsDs) p.Items.Add(new Item(g.IsDs ? "3DS, from a backup" : "3DS, matched to a backup", "", true, delegate { RunExport(g, true, PickFolder("Choose a Checkpoint backup of " + g.Title + " from your 3DS")); }) { Icon = '\uE898', Info = g.IsDs ? "Uses a Checkpoint backup of this game for the exact file name and size. Make one on your 3DS first and copy it to this PC." : "The safest way: checks the size against a Checkpoint backup of this game and copies its SecureValue." });
        return p;
    }
    string PickFolder(string prompt) {
        using (var d = new FolderBrowserDialog { Description = prompt }) return d.ShowDialog(this) == DialogResult.OK ? d.SelectedPath : null;
    }
    void RunExport(GameEntry g, bool checkpoint, string backup) {
        string outFolder = PickFolder("Where should the export go?");
        if (outFolder == null) return;
        string message = null; bool ok = false;
        Busy("Exporting " + g.Title, delegate { message = ExportSave(g, checkpoint, outFolder, backup, out ok); return null; }, delegate(string e) { ShowText(e == null && ok ? "Exported" : "Not exported", e ?? message, null); });
    }

    // ---------- importing saves ----------
    class FoundSave { public string File, Note; public GameEntry Game; public bool Slot; public SaveInfo Info; }
    // Null when it can't be read, or nothing has been saved in it yet. Never changes the file.
    public static SaveInfo ReadSaveInfo(string file) {
        try {
            if (file == null || !File.Exists(file)) return null;
            if (file.EndsWith(".lua", StringComparison.OrdinalIgnoreCase)) return SaveInfo.FromSlot(file);
            if (SaveGuard.IsBlank(file)) return null;
            int code; var f = Bridge("summary \"" + file + "\"", out code).TrimEnd('\r', '\n').Split('\t');
            if (code != 0 || f.Length < 6) return null;
            return new SaveInfo { Trainer = f[0], Hours = int.Parse(f[1]), Minutes = int.Parse(f[2]), Badges = int.Parse(f[3]), Caught = int.Parse(f[4]), Party = f[5] };
        } catch { return null; }
    }
    // Gen1Recomp's own slot files, as another install keeps them: saves\<version>\slot<n>.lua.
    static FoundSave IdentifySlot(string file) {
        if (!System.Text.RegularExpressions.Regex.IsMatch(Path.GetFileName(file), @"^slot\d+\.lua$", System.Text.RegularExpressions.RegexOptions.IgnoreCase)) return null;
        string version = Path.GetFileName(Path.GetDirectoryName(file));
        var game = Library.Games.Find(g => g.Gen1 != null && string.Equals(g.Gen1, version, StringComparison.OrdinalIgnoreCase));
        return game == null ? null : new FoundSave { File = file, Game = game, Slot = true };
    }
    // The file itself is only read. If the save in use has nothing in it yet this fills that one, otherwise it's a new save.
    public static string AdoptSlot(GameEntry g, string file, string name) {
        try {
            string running = RunningGames();
            if (running != "") return "Close " + running + " first, so nothing is writing to the save.";
            if (!g.Imported) return "Prepare " + g.Title + " first.";
            string why, current = Gen1Export(g, out why);
            bool empty = current == null && why == null && !(HasMaster(g) && File.Exists(MasterOf(g)) && !SaveGuard.IsBlank(MasterOf(g)));
            string madeEmpty = null, cameFrom = SaveSets.Active(g);
            if (!empty) {
                string problem = NewSave(g, name);
                if (problem != null) return problem;
                madeEmpty = Gen1Slot(g, "active " + g.Gen1);
            } else if (!string.IsNullOrWhiteSpace(name)) SaveSets.SetName(g, SaveSets.Active(g), name);
            else madeEmpty = null;
            var psi = Gen1Start("");
            int code; string slot;
            using (var plain = new PlainPath(file)) {
                psi.EnvironmentVariables["NATIVEMONS_CLI"] = "adopt " + g.Gen1 + " " + plain.Path_;
                slot = RunCapture(psi, 60, out code).Trim().Split('\n').Last().Trim();
            }
            if (code != 0) {
                // Back to the save the player was on, without the empty one made for this.
                if (!empty) {
                    string made = SaveSets.Active(g);
                    if (SwitchSave(g, cameFrom) == null) {
                        if (!string.IsNullOrEmpty(madeEmpty) && madeEmpty != "nil") Gen1Slot(g, "delete " + g.Gen1 + " " + madeEmpty);
                        DeleteSave(g, made);
                    }
                }
                return "Gen1Recomp couldn't take that save: " + slot;
            }
            File.WriteAllText(ParkedSlot(g, SaveSets.Active(g)), slot);
            // Only the empty slot made a moment ago goes. Slots holding a save are never removed.
            if (!string.IsNullOrEmpty(madeEmpty) && madeEmpty != "nil" && madeEmpty != slot) Gen1Slot(g, "delete " + g.Gen1 + " " + madeEmpty);
            if (Shares(g)) { var r = SyncSave(g, "gen1"); if (r.Failed) return r.Note; }
            return null;
        } catch (Exception x) { return x.Message; }
    }
    // What PKHeX calls each kind of save, and the games it can belong to.
    static readonly Dictionary<string, string[]> SaveKinds = new Dictionary<string, string[]> {
        { "RD", new[] { "red" } }, { "GN", new[] { "blue" } }, { "BU", new[] { "blue" } }, { "YW", new[] { "yellow" } }, { "RB", new[] { "red", "blue" } }, { "RBY", new[] { "red", "blue", "yellow" } },
        { "GD", new[] { "gold" } }, { "SI", new[] { "silver" } }, { "GS", new[] { "gold", "silver" } }, { "C", new[] { "crystal" } },
        { "R", new[] { "ruby" } }, { "S", new[] { "sapphire" } }, { "RS", new[] { "ruby", "sapphire" } }, { "E", new[] { "emerald" } }, { "FR", new[] { "firered" } }, { "LG", new[] { "leafgreen" } }, { "FRLG", new[] { "firered", "leafgreen" } },
        { "HG", new[] { "heartgold" } }, { "SS", new[] { "soulsilver" } }, { "HGSS", new[] { "heartgold", "soulsilver" } }, { "Pt", new[] { "platinum" } },
        { "B", new[] { "black" } }, { "W", new[] { "white" } }, { "BW", new[] { "black", "white" } }, { "B2", new[] { "black2" } }, { "W2", new[] { "white2" } }, { "B2W2", new[] { "black2", "white2" } } };
    // The game a save's name or neighbouring ROM points to.
    static GameEntry GameByName(string file) {
        string stem = Path.Combine(Path.GetDirectoryName(file), Path.GetFileNameWithoutExtension(file));
        foreach (var g in Library.Games)
            foreach (var rom in new[] { g.Gen1Rom, g.FulldexRom, g.NativeplatRom, g.FoundUnsupported })
                if (rom != null && string.Equals(Path.Combine(Path.GetDirectoryName(rom), Path.GetFileNameWithoutExtension(rom)), stem, StringComparison.OrdinalIgnoreCase)) return g;
        string name = System.Text.RegularExpressions.Regex.Replace(Path.GetFileNameWithoutExtension(file).ToLowerInvariant().Replace(" version", ""), "[^a-z0-9]+", " ");
        return Library.Games.Where(g => (" " + name + " ").Contains(" " + g.Title.Replace("Pokemon ", "").ToLowerInvariant() + " ")).OrderByDescending(g => g.Title.Length).FirstOrDefault();
    }
    // Reads the save itself. When its contents and its name disagree, its contents win, and the player is told.
    static FoundSave Identify(string file) {
        int code; string o = Bridge("info \"" + file + "\"", out code);
        if (code != 0) return null;
        var f = o.Trim().Split('\t');
        if (f.Length < 3) return null;
        string[] kinds;
        var named = GameByName(file);
        if (!SaveKinds.TryGetValue(f[1], out kinds)) return new FoundSave { File = file, Note = "This is a " + f[1] + " save, and no game here can use it." };
        if (named != null && kinds.Contains(named.Id)) return new FoundSave { File = file, Game = named };
        var game = kinds.Select(id => Library.Games.Find(g => g.Id == id)).FirstOrDefault(g => g != null && g.Playable) ?? Library.Games.Find(g => g.Id == kinds[0]);
        var save = new FoundSave { File = file, Game = game };
        if (named != null) save.Note = "It's named for " + named.Title + ", but it's really a " + (game != null ? game.Title : f[1]) + " save.";
        return save;
    }
    static List<string> SaveFolders() {
        var folders = Config.RomFolders();
        string extra = Path.Combine(Config.State, "save-folders.txt");
        if (File.Exists(extra)) folders.AddRange(File.ReadAllLines(extra).Where(x => x.Trim() != ""));
        return folders.Distinct(StringComparer.OrdinalIgnoreCase).ToList();
    }
    void OpenImportSaves() {
        var found = new List<FoundSave>();
        Busy("Looking for saves", delegate {
            foreach (var folder in SaveFolders()) {
                if (!Directory.Exists(folder)) continue;
                string[] files;
                try { files = Directory.GetFiles(folder, "*.sav", SearchOption.AllDirectories); } catch { continue; }
                foreach (var file in files) {
                    if (file.IndexOf("backup", StringComparison.OrdinalIgnoreCase) >= 0) continue;
                    var save = Identify(file);
                    if (save != null) { save.Info = ReadSaveInfo(file); found.Add(save); }
                }
                foreach (var file in Library.FilesUnder(folder)) {
                    var slot = IdentifySlot(file);
                    if (slot != null) { slot.Info = ReadSaveInfo(file); found.Add(slot); }
                }
            }
            return null;
        }, delegate(string e) {
            if (e != null) { ShowText("Import saves", e, null); return; }
            var p = new Page { Title = "Import saves" };
            foreach (var one in found) p.Items.Add(SaveRow(one));
            p.Items.Add(new Item("Choose a file", "", true, delegate { ChooseSaveFile(); }) { Icon = '\uE8E5', Info = "Pick any .sav file. It's read to work out which game it belongs to." });
            p.Items.Add(new Item("Add a save folder", "", true, delegate { AddSaveFolder(); }) { Icon = '\uE710', Info = "Also search another folder, such as another emulator's saves. Your ROM folders are always searched." });
            pages.Push(p);
        });
    }
    Item SaveRow(FoundSave one) {
        bool ready = one.Game != null && one.Game.Playable;
        string title = one.Game != null ? one.Game.Title : "Unknown game";
        string info = !ready ? (one.Game == null ? "" : "Its ROM isn't in your ROM folders, so there's nowhere to put it yet. ") : "Copies it to " + GameEntry.EngineName(one.Game.Engine()) + ". The file here isn't changed. ";
        // What's in the save comes first, as that's how the player tells two of the same game apart.
        if (one.Info != null) info = one.Info.Details(File.GetLastWriteTime(one.File)) + (one.Note != null ? "\n" + one.Note : "") + (ready ? "" : "\n" + info.Trim());
        else info += one.Note ?? "";
        var it = new Item(title, "", ready, delegate { ImportSave(one); }) { Icon = '\uE896', Info = info, TileHint = one.Info != null ? one.Info.TileLine : "", Records = new[] { "File: " + Path.GetFileName(one.File), "Folder: " + Path.GetDirectoryName(one.File) } };
        if (one.Game != null) it.Theme = one.Game.Theme;
        return it;
    }
    void ChooseSaveFile() {
        using (var d = new OpenFileDialog { Filter = "Cartridge save (*.sav)|*.sav|All files (*.*)|*.*", Title = "Choose a save" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            FoundSave save = null; string file = d.FileName;
            Busy("Reading the save", delegate { save = Identify(file); return null; }, delegate(string e) {
                if (e != null || save == null) { ShowText("Can't read that save", e ?? "PKHeX doesn't recognise this file as a save.", null); return; }
                if (save.Game == null || !save.Game.Playable) { ShowText("Can't import that save", (save.Note ?? "") + " Its game isn't in your ROM folders.", null); return; }
                ImportSave(save);
            });
        }
    }
    void AddSaveFolder() {
        using (var d = new FolderBrowserDialog { Description = "Choose a folder with your saves" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            string extra = Path.Combine(Config.State, "save-folders.txt");
            File.AppendAllText(extra, d.SelectedPath + Environment.NewLine);
            pages.Pop();
            OpenImportSaves();
        }
    }
    void ImportSave(FoundSave one) {
        var g = one.Game;
        if (one.Slot) {
            string name = AskText("Name this save", "Name for this save:", g.Title.Replace("Pokemon ", "") + " " + File.GetLastWriteTime(one.File).ToString("d MMM"));
            if (name == null) return;
            Action adopt = delegate {
                string problem = null;
                Busy("Importing " + g.Title, delegate { problem = AdoptSlot(g, one.File, name); return null; }, delegate(string e) { ReplaceMain(); ImportDone(g, e ?? problem); });
            };
            if (!g.Imported) Import(g, adopt); else adopt();
            return;
        }
        if (g.Engine() == "gen1" && !Shares(g)) {
            // A new save slot: anything already there is kept.
            Action run = delegate {
                string problem = null;
                Busy("Importing " + g.Title, delegate { problem = ImportBack(g, one.File); return null; }, delegate(string e) { ImportDone(g, e ?? problem); });
            };
            if (!g.Imported) Import(g, run); else run();
            return;
        }
        string target = Path.Combine(Config.FulldexDir, "saves", g.Save);
        if (!File.Exists(target)) { CopyInSave(g, one.File, target); return; }
        var p = new Page { Title = "Replace your " + g.Title + " save?", Body = "There's already a Fulldex save for this game. It'll be copied to save-backups first, so it can be restored.\n\n" + (one.Note ?? ""), Theme = g.Theme };
        p.Items.Add(new Item("Keep it", "", true, delegate { pages.Pop(); }) { Icon = '\uE711' });
        p.Items.Add(new Item("Replace it", "", true, delegate { pages.Pop(); CopyInSave(g, one.File, target); }) { Icon = '\uE896' });
        pages.Push(p);
    }
    void CopyInSave(GameEntry g, string file, string target) {
        try {
            if (File.Exists(target)) {
                string backups = Path.Combine(Config.State, "save-backups");
                Directory.CreateDirectory(backups);
                File.Copy(target, Path.Combine(backups, DateTime.Now.ToString("yyyyMMdd-HHmmss") + "-replaced-" + Path.GetFileName(target)), true);
            }
            Directory.CreateDirectory(Path.GetDirectoryName(target));
            File.Copy(file, target, true);
            ImportDone(g, null);
        } catch (Exception x) { ImportDone(g, x.Message); }
    }
    void ImportDone(GameEntry g, string problem) {
        ShowText(problem == null ? "Imported" : "Not imported", problem ?? "Your save is ready in " + g.Title + ". It now counts in My Collection and Move Pokemon.", null);
    }

    // Every installed game adds the entries it has, counted once however many games share them, and every save adds what it has caught.
    static bool AddCaught(HashSet<int> into, string file) {
        if (file == null || !File.Exists(file) || SaveGuard.IsBlank(file)) return false;
        int code; string o = Bridge("caught \"" + file + "\"", out code);
        if (code != 0) return false;
        foreach (var id in o.Trim().Split(',')) { int n; if (int.TryParse(id, out n)) into.Add(n); }
        return true;
    }
    // Each Pokemon once, however many of a game's saves have it. Null when none of them could be read.
    public static HashSet<int> CaughtInAllSaves(GameEntry g) {
        var all = new HashSet<int>();
        bool any = false;
        string active = SaveSets.Active(g);
        foreach (var set in SaveSets.All(g)) {
            if (set == active) { string why; any |= AddCaught(all, SaveOf(g, out why)); continue; }
            if (HasMaster(g) && File.Exists(ParkedSave(g, set))) { any |= AddCaught(all, ParkedSave(g, set)); continue; }
            // A Gen1Recomp slot can only be exported while it's in use, so it's briefly made so, unless a game is running.
            string slot = ReadSlot(g, set);
            if (slot == null || !HasGen1Slots(g) || RunningGames() != "") continue;
            string current = Gen1Slot(g, "active " + g.Gen1);
            if (current == null || Gen1Slot(g, "activate " + g.Gen1 + " " + slot) == null) continue;
            try { string why; any |= AddCaught(all, Gen1Export(g, out why)); }
            finally { Gen1Slot(g, "activate " + g.Gen1 + " " + current); }
        }
        return any ? all : null;
    }
    void OpenCollection() {
        var caughtBy = new Dictionary<GameEntry, HashSet<int>>();
        var noSave = new HashSet<GameEntry>();
        Busy("Counting your collection", delegate {
            foreach (var g in Library.Games.Where(x => x.Playable)) {
                var set = CaughtInAllSaves(g);
                if (set == null) noSave.Add(g); else caughtBy[g] = set;
            }
            return null;
        }, delegate(string e) {
            if (e != null) { ShowText("My Collection", e, null); return; }
            var games = Library.Games.Where(x => x.Playable).ToList();
            int entries = games.Count == 0 ? 0 : Enumerable.Range(1, games.Max(x => x.Dex)).Count(n => games.Exists(x => n <= x.Dex));
            var all = new HashSet<int>();
            foreach (var set in caughtBy.Values) all.UnionWith(set);
            int percent = entries == 0 ? 0 : all.Count * 100 / entries;
            var p = new Page { Title = "My Collection" };
            var total = new Item("All games: " + all.Count + " of " + entries, "", true, null) { Icon = '\uE8F1', Info = "You've caught " + all.Count + " different Pokemon out of the " + entries + " Pokedex entries in your " + games.Count + " games (" + percent + "%)." };
            p.Items.Add(total);
            foreach (var g in games) {
                bool has = caughtBy.ContainsKey(g);
                var it = new Item(g.Title + ": " + (has ? caughtBy[g].Count + " of " + g.Dex : "no save"), "", has, null) { Icon = '\uE7FC', Theme = g.Theme };
                int saveCount = SaveSets.All(g).Count;
                it.Info = has ? (saveCount > 1 ? "Different Pokemon caught across your " + saveCount + " saves" : "Caught in this save") + ", out of the " + g.Dex + " entries in this game." :"No save here yet. It still adds its entries to the total. Bring one in with Settings > Import saves.";
                if (has) {
                    int only = caughtBy[g].Count(n => !caughtBy.Where(o => o.Key != g).Any(o => o.Value.Contains(n)));
                    it.Records = new[] { "Only here: " + only };
                }
                p.Items.Add(it);
            }
            pages.Push(p);
        });
    }
    Page GamesPage() {
        var p = new Page { Title = "Games" };
        foreach (var game in Library.Games.OrderBy(g => g.Playable ? 0 : 1)) {
            var g = game;
            string why = Library.WhyNotPlayable(g);
            var it = new Item(g.Title, g.Playable ? "" : why, g.Playable, delegate { pages.Push(GamePage(g)); }) { Icon = '', Theme = g.Theme };
            it.Info = g.Playable ? g.Console + ". Runs with " + GameEntry.EngineName(g.Engine()) + "." : g.Console + ". " + why;
            it.Records = GameRecords(g);
            p.Items.Add(it);
        }
        if (p.Items.Count == 0) p.Items.Add(new Item("No games listed", Library.Note, false, null));
        return p;
    }
    string[] GameRecords(GameEntry g) {
        var r = new List<string>();
        if (g.Gen1Rom != null) r.Add("Gen1Recomp: " + Path.GetFileName(g.Gen1Rom) + (g.Imported ? " (prepared)" : ""));
        else if (g.Imported) r.Add("Gen1Recomp: prepared");
        if (g.FulldexRom != null) r.Add("Fulldex: " + Path.GetFileName(g.FulldexRom));
        if (Library.NeedsPatch(g.FulldexRom)) r.Add("Encrypted: needs patching for Fulldex");
        if (g.NativeplatRom != null) r.Add("nativeplat: " + Path.GetFileName(g.NativeplatRom));
        if (SaveSets.All(g).Count > 1) r.Add("Save: " + SaveSets.Name(g, SaveSets.Active(g)));
        return r.ToArray();
    }
    Page GamePage(GameEntry g) {
        var p = new Page { Title = g.Title, Theme = g.Theme };
        string engine = g.Engine();
        var play = new Item("Play", "Start with " + GameEntry.EngineName(engine) + ".", g.Playable, delegate { Play(g); }) { Icon = '' };
        play.Records = GameRecords(g);
        p.Items.Add(play);
        var engines = g.Engines();
        if (engines.Count > 1) {
            string other = engines[(engines.IndexOf(engine) + 1) % engines.Count];
            p.Items.Add(new Item("Engine: " + GameEntry.EngineShort(engine), "Switch to " + GameEntry.EngineName(other) + (Shares(g) || g.Port != null || g.Nativeplat != null ? ". They share one save." : "."), true, delegate {
                Config.Write("engine-" + g.Id + ".txt", other); pages.Pop(); pages.Push(GamePage(g)); ReplaceMain();
            }) { Icon = '' });
        }
        p.Items.Add(new Item("Saves", "In use: " + SaveSets.Name(g, SaveSets.Active(g)) + ".", true, delegate { OpenSaves(g); }) { Icon = '\uE896' });
        p.Items.Add(new Item("Game options", GameOptions.Available(g) ? "Volume, text speed, battle style and more." : "Volume, fullscreen and more.", true, delegate { pages.Push(GameOptionsPage(g)); }) { Icon = '\uE8D2' });
        if (g.OnGen1) p.Items.Add(new Item("Mods", "Mods for " + g.Title + ".", true, delegate { Busy("Reading mods", null, delegate(string e) { pages.Push(ModsPage(g)); }); }) { Icon = '' });
        p.Items.Add(new Item("Save backups", "Checked copies you can go back to.", true, delegate { pages.Push(BackupsPage(g)); }) { Icon = '\uE81C' });
        p.Items.Add(new Item("Export save", "Send it to a 3DS, flash cart or emulator.", true, delegate { pages.Push(ExportPage(g)); }) { Icon = '\uE898' });
        p.Items.Add(new Item("Open in PKHeX", "View or edit this game's save in PKHeX.", true, delegate { OpenInPkhex(g); }) { Icon = '\uE70F' });
        p.Items.Add(new Item("Open in OpenHome", "Move Pokemon across games and generations in OpenHome.", true, delegate { OpenInOpenHome(g); }) { Icon = '' });
        if (engine == "gen1") p.Items.Add(new Item("Bring back the edited save", "Takes the save you edited in PKHeX. Your older save slots stay.", true, delegate { BringBack(g); }) { Icon = '\uE896' });
        if (g.Imported && g.Gen1Rom != null) p.Items.Add(new Item("Prepare again", "Rebuilds the game data from your ROM. Saves are kept.", true, delegate { Import(g, null); }) { Icon = '' });
        if (engine == "fulldex" && Library.NeedsPatch(g.FulldexRom)) p.Items.Add(new Item("Patch this ROM", "It won't load until its secure area is decrypted.", true, delegate { pages.Push(PatchPage(g, false)); }) { Icon = '' });
        return p;
    }
    Page PatchPage(GameEntry g, bool thenPlay) {
        var p = new Page { Title = "Patch " + Path.GetFileName(g.FulldexRom) + "?", Theme = g.Theme };
        p.Body = "This ROM's secure area is still encrypted, so Fulldex can't load it without a DS BIOS.\n\nhails.nds-Patcher decrypts it. That changes 2 KB of your ROM file and keeps the original bytes in a backup, so it can be undone.\n\nIf no key table is available it downloads the DeSmuME emulator from GitHub to get one.";
        p.Items.Add(new Item("Patch it", "Changes the ROM file, with a backup.", File.Exists(Config.Patcher), delegate {
            pages.Pop();
            RunTool("Patching " + g.Title, Config.Python, "\"" + Config.Patcher + "\" \"" + g.FulldexRom + "\"", Path.GetDirectoryName(Config.Patcher), delegate(int code) {
                if (code != 0) { ShowLog("Patching failed", "The patcher stopped with code " + code + ". It needs Python 3."); return; }
                if (thenPlay) Play(g); else { pages.Pop(); pages.Push(GamePage(g)); }
            });
        }) { Icon = '' });
        p.Items.Add(new Item("Not now", "Nothing is changed.", true, delegate { pages.Pop(); }) { Icon = '' });
        return p;
    }
    Page ModGamesPage() {
        var p = new Page { Title = "Mods: choose a game" };
        foreach (var game in Library.Games.Where(x => x.OnGen1)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { pages.Push(ModsPage(g)); }) { Icon = '', Theme = g.Theme, Info = "Mods can differ per game." });
        }
        return p;
    }
    Page ModsPage(GameEntry g) {
        var p = new Page { Title = "Mods: " + g.Title, Theme = g.Theme };
        string list = Gen1Cli("list " + g.Gen1) ?? "";
        foreach (var row in list.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 6 || f[4] != "yes") continue;
            string id = f[0], name = f[1]; bool on = f[3] == "on";
            Item item = null;
            item = new Item(name + (on ? ": ON" : ": OFF"), "", true, delegate {
                on = !on;
                Gen1Cli("set " + g.Gen1 + " " + id + " " + (on ? "on" : "off"));
                item.Text = name + (on ? ": ON" : ": OFF");
            }) { Icon = '', Info = f[5] };
            item.Records = new[] { "Version: " + f[2] };
            p.Items.Add(item);
        }
        p.Items.Add(new Item("Install a mod (.zip)", "", true, delegate { InstallMod(g); }) { Icon = '\uE896', Info = "Copies a mod into the game's mods folder. Only install mods you trust: they run code in the game." });
        return p;
    }
    void InstallMod(GameEntry g) {
        using (var d = new OpenFileDialog { Filter = "Mod archive (*.zip)|*.zip", Title = "Choose a mod" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            string message;
            try { message = ExtractMod(d.FileName); } catch (Exception e) { message = "Could not install it: " + e.Message; }
            pages.Pop(); pages.Push(ModsPage(g));
            ShowText("Install a mod", message, null);
        }
    }
    static string ExtractMod(string zip) {
        string paths = Gen1Cli("paths");
        if (paths == null) return "Gen1Recomp was not found.";
        string mods = paths.Split('\n').Select(l => l.TrimEnd('\r').Split('\t')).Where(f => f[0] == "mods").Select(f => f[1]).First().Replace('/', '\\');
        string temp = Path.Combine(Config.State, "mod-unpack");
        if (Directory.Exists(temp)) Directory.Delete(temp, true);
        System.IO.Compression.ZipFile.ExtractToDirectory(zip, temp);
        // The manifest is at the top of the archive, or inside one folder.
        string root = File.Exists(Path.Combine(temp, "manifest.json")) ? temp : Directory.GetDirectories(temp).FirstOrDefault(x => File.Exists(Path.Combine(x, "manifest.json")));
        if (root == null) { Directory.Delete(temp, true); return "This archive has no manifest.json, so it is not a Gen1Recomp mod."; }
        string target = Path.Combine(mods, root == temp ? Path.GetFileNameWithoutExtension(zip) : Path.GetFileName(root));
        Directory.CreateDirectory(mods);
        if (Directory.Exists(target)) Directory.Delete(target, true);
        Directory.Move(root, target);
        if (Directory.Exists(temp)) Directory.Delete(temp, true);
        return "Installed " + Path.GetFileName(target) + ".";
    }
    Page FoldersPage() {
        var p = new Page { Title = "ROM folders" };
        foreach (var folder in Config.RomFolders()) {
            string f = folder;
            p.Items.Add(new Item(Path.GetFileName(f.TrimEnd('\\')), "", true, delegate { pages.Push(RemoveFolderPage(f)); }) { Icon = '', Info = f + "\n\nSelect to stop searching this folder. Your ROMs stay where they are." });
        }
        p.Items.Add(new Item("Add a folder", "", true, delegate {
            AddFolder();
        }) { Icon = '', Info = "nativeMons finds games by reading each ROM, so file names don't matter. No games are included: you need your own ROMs." });
        if (Config.RomFolders().Count > 0) p.Items.Add(new Item(FinishFoldersLabel, "", true, FinishFolders) { Icon = '', Info = "Go back to the main screen. Your folders are saved." });
        return p;
    }
    bool firstSetup;   // the ROM folders page opened by itself because nothing is set up yet
    string FinishFoldersLabel { get { return firstSetup ? "Complete setup" : "Done"; } }
    void FinishFolders() {
        firstSetup = false;
        while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
    }
    void AddFolder() {
        using (var d = new FolderBrowserDialog { Description = "Choose a folder with your ROMs (.gb, .gbc, .gba, .nds)" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            string chosen = d.SelectedPath;
            var l = Config.RomFolders();
            if (!l.Contains(chosen)) { l.Add(chosen); Config.SaveRomFolders(l); }
            while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
            Rescan(delegate { pages.Push(FoldersPage()); pages.Push(FolderAddedPage(chosen)); });
        }
    }
    // Says what the search found and how to carry on, with the likely next step selected.
    Page FolderAddedPage(string folder) {
        int have = Library.Games.Count(g => g.Playable);
        int roms = Library.Games.Count(g => g.FulldexRom != null || g.FoundUnsupported != null);
        var p = new Page { Title = "Folder added" };
        var finish = new Item(FinishFoldersLabel, "Go to the main screen.", true, FinishFolders) { Icon = '' };
        var another = new Item(have > 0 ? "Add another folder" : "Try another folder", "Choose a folder.", true, delegate { AddFolder(); }) { Icon = '' };
        if (have > 0) {
            p.Body = folder + "\n\nFound " + have + " of " + Library.Games.Count + " games.\n\nYou're ready to play. If you keep some games in another folder, add that one too.";
            p.Items.Add(finish); p.Items.Add(another);
        } else {
            p.Body = folder + (roms > 0
                ? "\n\nFound ROMs for " + roms + (roms == 1 ? " game" : " games") + ", but none can start. Games shows the reason for each one."
                : "\n\nNo games found. nativeMons looks through this folder and every folder inside it for .gb, .gbc, .gba and .nds ROMs.");
            p.Items.Add(another); p.Items.Add(finish);
        }
        return p;
    }
    Page RemoveFolderPage(string folder) {
        var p = new Page { Title = "Stop searching this folder?", Body = folder };
        p.Items.Add(new Item("Keep it", "", true, delegate { pages.Pop(); }) { Icon = '' });
        p.Items.Add(new Item("Stop searching it", "Nothing is deleted.", true, delegate {
            var l = Config.RomFolders(); l.Remove(folder); Config.SaveRomFolders(l);
            pages.Pop(); pages.Pop();
            Rescan(delegate { pages.Push(FoldersPage()); });
        }) { Icon = '' });
        return p;
    }
    Page CreditsPage() {
        return new Page { Title = "Credits", Body = "Pokemon Fulldex by mamesj (github.com/mamesj/pokemon-fulldex), a fork of ssrecomp by ClickingMaster. It uses melonDS, and mGBA by way of Pokemon GBA Widescreen. Function names come from the pokebw2, aletheia and pokeplatinum decompilations."
            + "\n\nPokemon Platinum PC port by cybervisi0n and contributors (github.com/cybervisi0n/pokeplatinum), from the pret decompilation, using libntr (MIT). They aren't affiliated with nativeMons."
            + "\n\nnativeplat by sohamb117 (github.com/sohamb117/gen4recomp), native Diamond, Pearl and Platinum from the pret decompilations. Not affiliated with nativeMons."
            + "\n\nMoving Pokemon between generations uses the conversion code of OpenHome by Andrew Benington (github.com/andrewbenington/OpenHome, GPL-3.0), running on Node.js. Open in OpenHome uses your own copy."
            + "\n\nPokemon pictures served by pogo.hails.app (hailsDotGO), from the PokeAPI sprites collection."
            + "\n\nMoving Pokemon uses PKHeX.Core by Kaphotics and contributors (github.com/kwsch/PKHeX, GPL-3.0)."
            + "\n\nhails.nds-Patcher by Hails. Launcher design from the hails-additions branch of the Skate 3 Rust Engine (SK8-ENGINE), github.com/Hailey-Ross/rusty-trucks."
            + "\n\n" + Gen1Credit
            + "\n\nNot affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokemon Company. Pokemon and all related names are the property of their owners. No games are included: you need your own ROMs." };
    }
    void ShowText(string title, string body, Action back) { ShowText(title, body, back, null); }
    void ShowText(string title, string body, Action back, string mono) {
        pages.Push(new Page { Title = title, Body = body, Mono = mono, OnBack = back });
        Invalidate();
    }
    void ShowLog(string title, string body) { ShowLog(title, body, null); }
    void ShowLog(string title, string body, Action back) {
        var last = log.Skip(Math.Max(0, log.Count - 12)).ToArray();
        ShowText(title, body, back, last.Length > 0 ? string.Join("\n", last) : null);
    }

    void GetAllSprites() {
        string summary = null;
        Busy("Downloading all sprites", delegate {
            Sprites.GetAll(delegate(int done, int total, int fresh, int failed) {
                progress = done / (float)total;
                if (done % 25 == 0 || done == total) BeginInvoke((Action)delegate { log.Add(done + " of " + total + " checked"); });
                dirty = true;
                if (done == total) summary = fresh + " downloaded, " + (total - fresh - failed) + " already here." + (failed > 0 ? "\n\n" + failed + " couldn't be fetched. Run this again when you're online: it only fetches what's missing." : "\n\nEvery picture is ready for offline play.");
            });
            return null;
        }, delegate(string e) { ShowText("Download all sprites", e ?? summary, null); });
    }

    // ---------- moving Pokemon (tools\mon-bridge, built on PKHeX.Core) ----------
    internal static string RunCapture(ProcessStartInfo psi, int seconds, out int code) {
        psi.UseShellExecute = false; psi.CreateNoWindow = true; psi.RedirectStandardOutput = true; psi.RedirectStandardError = true;
        psi.StandardOutputEncoding = Encoding.UTF8;
        using (var p = Process.Start(psi)) {
            var err = p.StandardError.ReadToEndAsync();
            var o = p.StandardOutput.ReadToEndAsync();
            if (!p.WaitForExit(seconds * 1000)) { try { p.Kill(); } catch { } code = -1; return "That took too long and was stopped."; }
            code = p.ExitCode;
            // The audio library always prints this when a run ends; it isn't a message about anything.
            string shown = code == 0 || o.Result.Trim() != "" ? o.Result : err.Result;
            string plain = string.Join("\n", shown.Split('\n').Where(l => !l.StartsWith("AL lib:") && !l.StartsWith("[")).ToArray());
            return code != 0 && plain.Trim() == "" ? err.Result + o.Result : plain;
        }
    }
    static string BridgeFamilies() {
        if (!File.Exists(Config.MonBridge)) return null;
        int code; string o = Bridge("families", out code);
        return code == 0 ? o : null;
    }
    static string Bridge(string args, out int code) { return RunCapture(new ProcessStartInfo(Config.MonBridge, args), 60, out code); }
    // Gen1Recomp keeps its own save format, so a cartridge .sav is exported for the move. Fulldex's are ready as they are.
    static string SaveOf(GameEntry g, out string why) {
        why = null;
        if (g.Engine() != "gen1") {
            string f = Path.Combine(Config.FulldexDir, "saves", g.Save ?? "");
            if (File.Exists(f)) return f;
            why = "No save yet for " + g.Title + ". Play and save once first.";
            return null;
        }
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = "export " + g.Gen1;
        int code; string o = RunCapture(psi, 60, out code).Trim();
        if (code == 0 && o != "") return o.Split('\n').Last().Trim();
        why = o != "" ? o : "No save yet for " + g.Title + ". Play and save once first.";
        return null;
    }
    static string ImportBack(GameEntry g, string file) {
        if (g.Engine() != "gen1") return null;
        var psi = Gen1Start("");
        int code; string o;
        using (var plain = new PlainPath(file)) {
            psi.EnvironmentVariables["NATIVEMONS_CLI"] = "import " + g.Gen1 + " " + plain.Path_;
            o = RunCapture(psi, 60, out code);
        }
        return code == 0 ? null : (o.Trim() != "" ? o.Trim() : "Gen1Recomp could not take the save back.");
    }
    Page MoveFromPage() {
        var p = new Page { Title = "Move Pokemon: from which game?" };
        foreach (var game in Library.Games.Where(x => x.Playable)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { OpenBoxes(g); }) { Icon = '', Theme = g.Theme, Info = "Choose a Pokemon from this game's PC boxes. Saves from " + GameEntry.EngineName(g.Engine()) + " are used.", Records = new[] { "Backups: both saves are copied first" } });
        }
        return p;
    }
    void OpenBoxes(GameEntry g) {
        string rows = null, why = null, file = null;
        Busy("Reading " + g.Title, delegate {
            file = SaveOf(g, out why);
            if (file == null) return null;
            int code; rows = Bridge("list \"" + file + "\"", out code);
            if (code != 0) why = rows.Trim();
            return null;
        }, delegate(string e) {
            string problem = e ?? why;
            if (problem != null) { ShowText("Can't read that save", problem, null); return; }
            var p = new Page { Title = g.Title + ": boxes", Theme = g.Theme };
            foreach (var row in rows.Split('\n')) {
                var f = row.TrimEnd('\r').Split('\t');
                if (f.Length < 8) continue;
                string box = f[0], slot = f[1], name = f[2] + " Lv " + f[4];
                p.Items.Add(new Item(name, "", true, delegate { pages.Push(MoveToPage(g, file, box, slot, name)); }) { Icon = '', Sprite = (f[7] == "shiny" ? "shiny/" : "") + f[6], Info = f[3] + ", from trainer " + f[5] + (f[7] == "shiny" ? ". Shiny!" : "."), Records = new[] { "Box: " + (int.Parse(box) + 1), "Slot: " + (int.Parse(slot) + 1) } });
            }
            if (p.Items.Count == 0) p.Items.Add(new Item("Boxes are empty", "Only Pokemon in the PC boxes can be moved, not your party.", false, null));
            pages.Push(p);
        });
    }
    Page MoveToPage(GameEntry from, string file, string box, string slot, string name) {
        var p = new Page { Title = "Move " + name + " to...", Theme = from.Theme };
        foreach (var game in Library.Games.Where(x => x.Playable && x != from)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { MoveNow(from, file, box, slot, name, g); }) { Icon = '', Theme = g.Theme, Info = "Moves it to the first free PC box slot. If something stands in the way, you'll be asked how to resolve it." });
        }
        return p;
    }
    // First ask what stands in the way. Anything the player must decide goes on a page; a clean move just happens.
    void MoveNow(GameEntry from, string file, string box, string slot, string name, GameEntry to) {
        string running = RunningGames();
        if (running != "") { ShowText("Close the game first", "Already running: " + running + ". Saves can't change while it plays.", null); return; }
        string plan = null, target = null, why = null; int planCode = 0;
        Busy("Checking " + name, delegate {
            target = SaveOf(to, out why);
            if (target == null) return null;
            plan = Bridge("plan \"" + file + "\" " + box + " " + slot + " \"" + target + "\"", out planCode);
            return null;
        }, delegate(string e) {
            if (e != null || target == null) { ShowText("Not moved", e ?? why, null); return; }
            var rows = plan.Split('\n').Select(l => l.TrimEnd('\r').Split('\t')).Where(f => f[0] != "").ToList();
            if (planCode != 0) { var n = rows.Find(f => f[0] == "nope"); ShowText("Not moved", n != null && n.Length > 1 ? n[1] : plan.Trim(), null); return; }
            bool forced = rows.Exists(f => f[0] == "forced");
            if (!forced && !rows.Exists(f => f[0] == "conflict")) { RunMove(from, file, box, slot, name, to, target, ""); return; }
            pages.Push(ResolvePage(from, file, box, slot, name, to, target, rows, forced));
        });
    }
    // A blocked move: the player deletes or changes each conflict, then retries.
    Page ResolvePage(GameEntry from, string file, string box, string slot, string name, GameEntry to, string target, List<string[]> rows, bool forced) {
        var p = new Page { Title = name + " to " + to.Title.Replace("Pokemon ", ""), Theme = to.Theme };
        var answers = new Dictionary<string, string>();   // move number -> new id, "0" deletes
        var kept = rows.Count(f => f[0] == "ok");
        var conflicts = rows.Where(f => f[0] == "conflict").ToList();
        var item = rows.Find(f => f[0] == "item");
        bool dropItem = false;
        Item retry = null;
        Action refresh = delegate {
            int left = kept + conflicts.Count(c => answers.ContainsKey(c[1]) && answers[c[1]] != "0");
            bool done = conflicts.All(c => answers.ContainsKey(c[1])) && (item == null || dropItem);
            retry.On = done && left > 0;
            retry.Hint = !done ? "Decide each conflict first." : left == 0 ? "It needs at least one move: change one instead of deleting it." : "";
            retry.Info = retry.Hint != "" ? retry.Hint : "Moves it with your choices. Both saves are backed up first.";
        };
        foreach (var c in conflicts) {
            string number = c[1], moveName = c[2];
            Item it = null;
            it = new Item(moveName + ": decide", "", true, delegate {
                var choice = new Page { Title = moveName + " doesn't exist in " + to.Title.Replace("Pokemon ", ""), Theme = to.Theme };
                choice.Items.Add(new Item("Delete this move", "", true, delegate { answers[number] = "0"; it.Text = moveName + ": delete"; refresh(); pages.Pop(); }) { Icon = '\uE74D', Info = "It leaves with one move fewer." });
                choice.Items.Add(new Item("Change it to...", "", true, delegate { pages.Push(ChangeMovePage(to, target, delegate(string id, string newName) { answers[number] = id; it.Text = moveName + " to " + newName; refresh(); pages.Pop(); pages.Pop(); })); }) { Icon = '\uE70F', Info = "Pick any move from that game." });
                pages.Push(choice);
            }) { Icon = '\uE8AB', Info = "This move doesn't exist in that game. Delete it, or change it to another move." };
            p.Items.Add(it);
        }
        if (item != null) {
            Item it = null;
            it = new Item("Remove " + item[1] + "?", "", true, delegate { dropItem = !dropItem; it.Text = dropItem ? item[1] + ": remove" : "Remove " + item[1] + "?"; refresh(); }) { Icon = '\uE74D', Info = "Held items can't cross between these games. Select to agree to removing it." };
            p.Items.Add(it);
        }
        retry = new Item("Retry the move", "", false, delegate {
            pages.Pop();
            string opts = string.Join(" ", answers.Select(a => "move:" + a.Key + ":" + a.Value).ToArray()) + (dropItem ? " dropitem" : "");
            RunMove(from, file, box, slot, name, to, target, opts);
        }) { Icon = '\uE72C' };
        p.Items.Add(retry);
        if (forced) p.Items.Add(new Item("About this move", "", false, null) { Icon = '\uE946', Info = "No real transfer exists here, so OpenHome's converter makes the copy. Its form, ball and origin may change, and PKHeX may flag it as not legal. Its original data is kept, and comes back when it returns to its own generation." });
        refresh();
        return p;
    }
    Page ChangeMovePage(GameEntry to, string target, Action<string, string> chosen) {
        var p = new Page { Title = "Change to which move?", Theme = to.Theme };
        int code; string list = Bridge("moves \"" + target + "\"", out code);
        foreach (var row in list.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 2) continue;
            string id = f[0], n = f[1];
            p.Items.Add(new Item(n, "", true, delegate { chosen(id, n); }) { Icon = '\uE8AB', Info = "Use " + n + " instead." });
        }
        return p;
    }
    void RunMove(GameEntry from, string file, string box, string slot, string name, GameEntry to, string target, string options) {
        string message = null; bool ok = false;
        Busy("Moving " + name, delegate {
            int code; string o = Bridge("move \"" + file + "\" " + box + " " + slot + " \"" + target + "\" \"" + Path.Combine(Config.State, "save-backups") + "\" " + options, out code);
            var f = o.Trim().Split('\t');
            if (code != 0) { message = f.Length > 1 ? f[1] : o.Trim(); return null; }
            // Destination first: any failure after this duplicates the Pokemon, it never loses it.
            string back = ImportBack(to, target) ?? ImportBack(from, file);
            ok = back == null;
            message = ok ? name + " is now in " + to.Title + "'s PC boxes." + (f.Length > 3 && f[3] == "restored" ? "\n\nIts original data is back: its origin, ball and moves, as they were before it left." : "") + (f.Length > 2 && f[2] == "not-legal" ? "\n\nPKHeX's legality check flags it as not legal, which matters only for online play." + (f.Length > 3 ? "" : " Its original data is kept, and comes back if it returns to its own generation.") : "") : back;
            return null;
        }, delegate(string e) {
            while (pages.Count > 1) pages.Pop();
            ShowText(ok ? "Moved" : "Not moved", e ?? message, null);
        });
    }
    void BringBack(GameEntry g) {
        using (var d = new OpenFileDialog { Filter = "Cartridge save (*.sav)|*.sav", Title = "Choose the edited save" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            string file = d.FileName, problem = null;
            Busy("Bringing back the save", delegate { problem = ImportBack(g, file); return null; }, delegate(string e) { ShowText(e == null && problem == null ? "Done" : "Not changed", e ?? problem ?? g.Title + " will start from the edited save. Your older slots are kept.", null); });
        }
    }
    // Opens the game's save in the player's own copy of PKHeX, which the launcher asks for the first time.
    void OpenInPkhex(GameEntry g) {
        string exe = Config.Read("pkhex-path.txt", "");
        if (!File.Exists(exe)) {
            using (var d = new OpenFileDialog { Filter = "PKHeX|PKHeX*.exe|Programs (*.exe)|*.exe", Title = "Where is PKHeX? (projectpokemon.org/home/files/file/1-pkhex)" }) {
                if (d.ShowDialog(this) != DialogResult.OK) return;
                exe = d.FileName; Config.Write("pkhex-path.txt", exe);
            }
        }
        string why = null, file = null;
        Busy("Opening " + g.Title + " in PKHeX", delegate { file = SaveOf(g, out why); return null; }, delegate(string e) {
            if (e != null || file == null) { ShowText("Can't open that save", e ?? why, null); return; }
            try { Process.Start(new ProcessStartInfo(exe, "\"" + file + "\"") { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe) }); }
            catch (Exception x) { ShowText("Can't start PKHeX", x.Message, null); return; }
            if (g.Engine() == "gen1") ShowText("Editing an exported copy", "Gen1Recomp keeps its own save format, so PKHeX has an exported copy:\n" + file + "\n\nSave your changes there in PKHeX, then choose Bring back the edited save on this game's page.", null);
        });
    }

    // OpenHome takes no file on its command line, as far as its source shows, so the player picks the save inside it.
    void OpenInOpenHome(GameEntry g) {
        string exe = Config.Read("openhome-path.txt", "");
        if (!File.Exists(exe)) {
            using (var d = new OpenFileDialog { Filter = "OpenHome|OpenHome*.exe|Programs (*.exe)|*.exe", Title = "Where is OpenHome? (github.com/andrewbenington/OpenHome/releases)" }) {
                string likely = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "OpenHome");
                if (Directory.Exists(likely)) d.InitialDirectory = likely;
                if (d.ShowDialog(this) != DialogResult.OK) return;
                exe = d.FileName; Config.Write("openhome-path.txt", exe);
            }
        }
        string why = null, file = null;
        Busy("Opening OpenHome", delegate { file = SaveOf(g, out why); return null; }, delegate(string e) {
            if (e != null || file == null) { ShowText("Can't open that save", e ?? why, null); return; }
            file = file.Replace('/', '\\');
            try { Process.Start(new ProcessStartInfo(exe) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe) }); }
            catch (Exception x) { ShowText("Can't start OpenHome", x.Message, null); return; }
            try { Clipboard.SetText(file); } catch { }
            string after = g.Engine() == "gen1"
                ? "\n\nGen1Recomp keeps its own save format, so this is an exported copy. Save your changes there, then choose Bring back the edited save on this game's page."
                : "\n\nThe launcher picks up what OpenHome saves here. A backup is made before each game.";
            ShowText("OpenHome is starting", "Open this save in OpenHome. Its path is on your clipboard:\n\n" + file + after + "\n\nOpenHome's moved Pokemon aren't guaranteed legal.", null);
        });
    }

    // ---------- running things ----------
    void Play(GameEntry g) {
        if (g == null || !g.Playable) return;
        string running = RunningGames();
        if (running != "") {
            waiting = g.Id;
            ShowText("Waiting", "Already running: " + running + "\nOnly one game at a time. Close it, or wait here: this continues by itself.\n\nB: cancel", delegate { waiting = null; pages.Pop(); });
            return;
        }
        current = g; Config.Write("last-game.txt", g.Id);
        if (g.Engine() == "fulldex") {
            if (Library.NeedsPatch(g.FulldexRom)) { pages.Push(PatchPage(g, true)); return; }
            BeforeGame(g, delegate { PlayFulldex(g); });
        }
        else if (g.Engine() == "port") BeforeGame(g, delegate { PlayPort(g); });
        else if (g.Engine() == "nativeplat") BeforeGame(g, delegate { PlayNativeplat(g); });
        else if (!g.Imported) Import(g, delegate { BeforeGame(g, delegate { PlayGen1(g); }); });
        else BeforeGame(g, delegate { PlayGen1(g); });
    }
    void BeforeGame(GameEntry g, Action start) {
        Busy("Backing up your save", delegate {
            string file = g.Imported || g.Engine() != "gen1" ? CurrentSave(g) : null;
            try { SaveGuard.Backup(g, file, "before-play"); } catch { }
            return null;
        }, delegate(string e) {
            SyncThen(g, delegate { Busy("Setting game options", delegate { ApplyOptions(g); return null; }, delegate(string e2) { start(); }); });
        });
    }
    void Import(GameEntry g, Action then) {
        var psi = Gen1Start("");
        psi.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
        psi.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
        var rom = new PlainPath(g.Gen1Rom);
        psi.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = rom.Path_;
        RunJob("Preparing " + g.Title, psi, false, delegate(int code) {
            rom.Dispose();
            if (code != 0) { ShowLog("Could not prepare " + g.Title, "Your ROM was not changed."); return; }
            g.Imported = true;
            if (then != null) then(); else ShowText("Ready", g.Title + " is prepared.", null);
        });
    }
    void PlayGen1(GameEntry g) {
        var psi = Gen1Start(" --game=" + g.Gen1);
        var wanted = GameOptions.ForGame(g);
        if (wanted.Count > 0) psi.EnvironmentVariables["NATIVEMONS_OPTIONS"] = string.Join(",", wanted.Select(x => x.Key + "=" + x.Value).ToArray());
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    // Explicit arguments, as Fulldex's own list builds them: it then skips that list, and QUIT ends the program.
    void PlayFulldex(GameEntry g) {
        string dir = Config.FulldexDir, saves = Path.Combine(dir, "saves");
        Directory.CreateDirectory(saves);
        string args;
        if (g.IsDs) {
            args = "\"" + g.FulldexRom + "\" --save \"" + Path.Combine(saves, g.Save) + "\" --config \"" + Path.Combine(dir, "settings.cfg") + "\" --jit";
            // SoulSilver and HeartGold use the program's own tables and must not be given a function table.
            if (g.FulldexCode != "IPGE" && g.FulldexCode != "IPKE") args += " --funcs \"" + Path.Combine(dir, "data", "funcs", g.FulldexCode + ".txt") + "\"";
        } else {
            // GBA games are only found in Fulldex's roms folder. A copy goes there, the original stays put.
            string roms = Path.Combine(dir, "roms"), copy = Path.Combine(roms, Path.GetFileName(g.FulldexRom));
            Directory.CreateDirectory(roms);
            if (!string.Equals(Path.GetFullPath(g.FulldexRom), Path.GetFullPath(copy), StringComparison.OrdinalIgnoreCase) && !File.Exists(copy)) File.Copy(g.FulldexRom, copy);
            args = "--game " + g.FulldexCode;
            watchGba = true; gbaPlaying = false; gbaClosed = false;
        }
        var psi = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    // After a GBA game ends, Fulldex's published build opens its own game list instead of exiting, and has no argument
    // to change that. Its list has a window title of its own, and by then the game has finished and saved, so seeing the
    // game's window give way to the list means it's safe to close the program and come straight back here.
    bool watchGba, gbaPlaying, gbaClosed;
    DateTime gbaWatchedAt;
    void WatchGba() {
        if (!watchGba || job == null || (DateTime.Now - gbaWatchedAt).TotalMilliseconds < 50) return;
        gbaWatchedAt = DateTime.Now;
        try {
            job.Refresh();
            string title = job.MainWindowTitle;
            if (title == "") return;
            bool list = title.IndexOf("Fulldex", StringComparison.OrdinalIgnoreCase) >= 0;
            if (!list) gbaPlaying = true;
            else if (gbaPlaying) { gbaClosed = true; watchGba = false; job.Kill(); }
        } catch { }
    }
    // ---------- the Platinum PC port ----------
    // It reads and writes save.bin beside its program, in its own padded layout (see PlatinumSave). Only one engine runs at a
    // time, so the save goes in, converted, before it starts and comes back, converted and checked, when it ends.
    static string PortDir { get { return Path.GetDirectoryName(Config.PlatinumPort); } }
    static string PortSave { get { return Path.Combine(PortDir, "save.bin"); } }
    static string PortRecord(GameEntry g) { return Path.Combine(Config.State, "save-sync", g.Id + "-port.txt"); }
    static string ReadPortRecord(GameEntry g) { try { return File.Exists(PortRecord(g)) ? File.ReadAllText(PortRecord(g)).Trim() : null; } catch { return null; } }
    static void WritePortRecord(GameEntry g, string hash) {
        Directory.CreateDirectory(Path.GetDirectoryName(PortRecord(g)));
        File.WriteAllText(PortRecord(g), hash ?? "");
    }
    // Null when all is well, or what the player should know. The shared save only changes once the port's has passed.
    public static string SaveFromPort(GameEntry g) {
        try {
            string save = PortSave, master = MasterOf(g);
            if (!File.Exists(save) || SaveGuard.IsBlank(save) || SaveGuard.Hash(save) == ReadPortRecord(g)) return null;
            string incoming = Path.Combine(Config.State, "save-sync", g.Id + "-port-incoming.sav");
            Directory.CreateDirectory(Path.GetDirectoryName(incoming));
            byte[] data;
            string problem = null;
            // A file the converter can't read is still kept, whole, as it may be the only copy of what was played.
            try { data = PlatinumSave.ToDs(File.ReadAllBytes(save)); }
            catch (InvalidDataException x) { data = File.ReadAllBytes(save); problem = x.Message; }
            File.WriteAllBytes(incoming, data);
            problem = problem ?? SaveGuard.Problem(incoming);
            if (problem != null) {
                SaveGuard.Backup(g, incoming, "platinum-port");
                try { File.Delete(incoming); } catch { }
                return "The Platinum PC port's save didn't pass its check, so your shared save wasn't changed. A copy is in your backups.\n\nWhy: " + problem;
            }
            SaveGuard.Backup(g, master, "before-port");
            SaveGuard.Backup(g, incoming, "platinum-port");
            SaveGuard.WriteInPlace(incoming, master);
            int code; string o = SaveGuard.Bridge("compare \"" + incoming + "\" \"" + master + "\"", out code);
            try { File.Delete(incoming); } catch { }
            if (code != 0) return "The Platinum PC port's save didn't copy correctly: " + o.Trim() + " Both are in your backups.";
            WritePortRecord(g, SaveGuard.Hash(save));
            return null;
        } catch (Exception x) { return "The Platinum PC port's save wasn't brought back, and it'll try again next time. " + x.Message; }
    }
    // Everything the port needs before it starts. Throws if it can't be got ready, so a save is never put at risk.
    public static void PreparePort(GameEntry g) {
        if (!File.Exists(Path.Combine(PortDir, "arm9.bin"))) NdsExtract.Run(g.FulldexRom, PortDir);
        // Anything left from a run that never came back goes home first, and a copy is kept if it can't.
        string note = SaveFromPort(g);
        if (note != null && File.Exists(PortSave)) SaveGuard.Backup(g, PortSave, "platinum-port-leftover");
        string master = MasterOf(g);
        if (File.Exists(master) && !SaveGuard.IsBlank(master)) {
            string converted = PortSave + ".in";
            File.WriteAllBytes(converted, PlatinumSave.ToPort(File.ReadAllBytes(master)));
            SaveGuard.WriteInPlace(converted, PortSave);
            File.Delete(converted);
        }
        else if (File.Exists(PortSave)) File.Delete(PortSave);
        WritePortRecord(g, File.Exists(PortSave) ? SaveGuard.Hash(PortSave) : "");
    }
    void PlayPort(GameEntry g) {
        string problem = null;
        Busy("Getting " + g.Title + " ready", delegate { try { PreparePort(g); } catch (Exception x) { problem = x.Message; } return null; }, delegate(string e) {
            if (e != null || problem != null) { ShowText("Could not start " + g.Title, "Nothing was changed.\n\n" + (e ?? problem), null); return; }
            var psi = new ProcessStartInfo(Config.PlatinumPort) { UseShellExecute = false, WorkingDirectory = PortDir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            RunJob(g.Title, psi, true, delegate(int code) {
                string note = null;
                Busy("Saving " + g.Title, delegate { note = SaveFromPort(g); return null; }, delegate(string e2) {
                    if (e2 != null || note != null) noticeAfter = e2 ?? note;
                    Finished(g, code);
                });
            });
        });
    }
    // ---------- nativeplat ----------
    // It keeps raw cartridge saves in named slots under userdata (portable.txt beside it). We give each game one slot and treat
    // it as the port's save.bin: the shared save goes in before the game and comes back, checked, after. Its own launcher only
    // shows if something is wrong, because the ROM and the slot are both there before it starts.
    const string NativeplatSlotName = "nativemons";
    const int DsSaveBytes = 0x80000;
    static string NativeplatDir { get { return Path.GetDirectoryName(Config.Nativeplat); } }
    static string NativeplatSlot(GameEntry g) { return Path.Combine(NativeplatDir, "userdata", "saves", g.Nativeplat, NativeplatSlotName + ".sav"); }
    static string NativeplatRecord(GameEntry g) { return Path.Combine(Config.State, "save-sync", g.Id + "-nativeplat.txt"); }
    static string ReadNativeplatRecord(GameEntry g) { try { return File.Exists(NativeplatRecord(g)) ? File.ReadAllText(NativeplatRecord(g)).Trim() : null; } catch { return null; } }
    static void WriteNativeplatRecord(GameEntry g, string hash) {
        Directory.CreateDirectory(Path.GetDirectoryName(NativeplatRecord(g)));
        File.WriteAllText(NativeplatRecord(g), hash ?? "");
    }
    // Null when all is well, or what the player should know. The shared save only changes once nativeplat's has passed.
    public static string SaveFromNativeplat(GameEntry g) {
        try {
            string save = NativeplatSlot(g), master = MasterOf(g);
            if (!File.Exists(save) || new FileInfo(save).Length == 0 || SaveGuard.IsBlank(save) || SaveGuard.Hash(save) == ReadNativeplatRecord(g)) return null;
            string problem = SaveGuard.Problem(save);
            if (problem != null) {
                SaveGuard.Backup(g, save, "nativeplat");
                return "nativeplat's save didn't pass its check, so your shared save wasn't changed. A copy is in your backups.\n\nWhy: " + problem;
            }
            Directory.CreateDirectory(Path.GetDirectoryName(master));
            SaveGuard.Backup(g, master, "before-nativeplat");
            SaveGuard.Backup(g, save, "nativeplat");
            SaveGuard.WriteInPlace(save, master);
            if (SaveGuard.Hash(save) != SaveGuard.Hash(master)) return "nativeplat's save didn't copy correctly. Both are in your backups.";
            WriteNativeplatRecord(g, SaveGuard.Hash(save));
            return null;
        } catch (Exception x) { return "nativeplat's save wasn't brought back, and it'll try again next time. " + x.Message; }
    }
    public static string NativeplatRomCopy(GameEntry g) {
        string rom = Path.Combine(NativeplatDir, "userdata", "roms", g.Nativeplat + ".nds");
        Directory.CreateDirectory(Path.GetDirectoryName(rom));
        if (!File.Exists(rom)) { File.Copy(g.NativeplatRom, rom + ".part", true); File.Move(rom + ".part", rom); }
        return rom;
    }
    // Everything nativeplat needs before it starts. Throws if it can't be got ready, so a save is never put at risk.
    public static void PrepareNativeplat(GameEntry g) {
        string marker = Path.Combine(NativeplatDir, "portable.txt");
        if (!File.Exists(marker)) File.WriteAllText(marker, "");
        NativeplatRomCopy(g);
        string note = SaveFromNativeplat(g), save = NativeplatSlot(g), master = MasterOf(g);
        if (note != null && File.Exists(save)) SaveGuard.Backup(g, save, "nativeplat-leftover");
        Directory.CreateDirectory(Path.GetDirectoryName(save));
        if (File.Exists(master) && !SaveGuard.IsBlank(master)) {
            long size = new FileInfo(master).Length;
            if (size != DsSaveBytes) throw new InvalidDataException("nativeplat needs a save of exactly 512 KB, and this one is " + size + " bytes.");
            SaveGuard.WriteInPlace(master, save);
        }
        else File.WriteAllBytes(save, new byte[0]);
        try { File.Delete(save + ".bak"); } catch { }
        WriteNativeplatRecord(g, SaveGuard.Hash(save));
    }
    void PlayNativeplat(GameEntry g) {
        string problem = null;
        Busy("Getting " + g.Title + " ready", delegate { try { PrepareNativeplat(g); } catch (Exception x) { problem = x.Message; } return null; }, delegate(string e) {
            if (e != null || problem != null) { ShowText("Could not start " + g.Title, "Nothing was changed.\n\n" + (e ?? problem), null); return; }
            var psi = new ProcessStartInfo(Config.Nativeplat, "--game " + g.Nativeplat + " --slot " + NativeplatSlotName) { UseShellExecute = false, WorkingDirectory = NativeplatDir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            RunJob(g.Title, psi, true, delegate(int code) {
                string note = null;
                Busy("Saving " + g.Title, delegate { note = SaveFromNativeplat(g); return null; }, delegate(string e2) {
                    if (e2 != null || note != null) noticeAfter = e2 ?? note;
                    Finished(g, code);
                });
            });
        });
    }
    void Finished(GameEntry g, int code) {
        var t = DateTime.Now - gameStart;
        string text = g.Title + " ran for " + ((int)t.TotalMinutes) + ":" + t.Seconds.ToString("00") + ".";
        if (code != 0) text += " It ended with code " + code + ".";
        string detail = code != 0 ? "\n\n" + string.Join("\n", log.Skip(Math.Max(0, log.Count - 12)).ToArray()) : "";
        Config.Write("last-session.txt", text + detail);
        var kept = log.ToArray();
        AfterGame(g, code, delegate {
            log.Clear(); log.AddRange(kept);
            if (code != 0) ShowLog("Finished, check this", text, delegate { if (playDirect != null) Close(); else pages.Pop(); });
            else if (noticeAfter != null) { ShowText("About your save", noticeAfter, delegate { if (playDirect != null) Close(); else pages.Pop(); }); }
            else if (playDirect != null) { Close(); return; }
            noticeAfter = null;
            ReplaceMain();
        });
    }
    void RunTool(string title, string exe, string args, string dir, Action<int> done) {
        RunJob(title, new ProcessStartInfo(exe, args) { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true }, false, done);
    }
    void RunJob(string title, ProcessStartInfo psi, bool isGame, Action<int> done) {
        log.Clear(); jobTitle = title; jobIsGame = isGame; jobHid = false; jobStart = DateTime.Now; if (isGame) gameStart = jobStart; progress = -1; jobDone = done;
        // Let the game take the foreground from us.
        Native.AllowSetForegroundWindow(-1);
        var p = new Process { StartInfo = psi, EnableRaisingEvents = true };
        DataReceivedEventHandler add = delegate(object s, DataReceivedEventArgs e) {
            if (e.Data == null) return;
            BeginInvoke((Action)delegate { AddLine(e.Data); dirty = true; });
        };
        p.OutputDataReceived += add; p.ErrorDataReceived += add;
        p.Exited += delegate { BeginInvoke((Action)JobEnded); };
        try { p.Start(); }
        catch (Exception e) { ShowText("Could not start", psi.FileName + "\n\n" + e.Message, null); return; }
        job = p;
        p.BeginOutputReadLine(); p.BeginErrorReadLine();
        Invalidate();
    }
    // Gen1Recomp reads paths from the environment in the system's code page, which garbles accented names such as
    // "Pokémon". Such a file goes in through a plain-named temporary copy. Dispose removes it.
    internal sealed class PlainPath : IDisposable {
        public readonly string Path_; readonly string temp;
        public PlainPath(string file) {
            if (file.All(c => c < 128)) { Path_ = file; return; }
            temp = Path.Combine(Path.GetTempPath(), "nativemons-" + Guid.NewGuid().ToString("N") + Path.GetExtension(file));
            File.Copy(file, temp, true);
            var shortName = new StringBuilder(600);
            Path_ = !temp.All(c => c < 128) && Native.GetShortPathName(temp, shortName, 600) > 0 ? shortName.ToString() : temp;
        }
        public void Dispose() { if (temp != null) try { File.Delete(temp); } catch { } }
    }
    // Gen1Recomp's import reports "nativemons: <percent> <text>".
    void AddLine(string raw) {
        string c = raw.Trim();
        if (c == "") return;
        if (c.StartsWith("nativemons: ")) {
            c = c.Substring(12);
            int sp = c.IndexOf(' '); int pct;
            if (sp > 0 && int.TryParse(c.Substring(0, sp), out pct)) { progress = pct / 100f; c = c.Substring(sp + 1); }
            else if (c.StartsWith("error ")) c = c.Substring(6);
            log.Add(c);
        } else if (jobIsGame) log.Add(c);
        if (log.Count > 400) log.RemoveAt(0);
    }
    void JobEnded() {
        var p = job; job = null; int code = 0;
        try { p.WaitForExit(); code = p.ExitCode; p.Dispose(); } catch { }
        if (gbaClosed) code = 0;
        watchGba = false; gbaClosed = false;
        if (!Visible) Show();
        if (WindowState == FormWindowState.Minimized) WindowState = FormWindowState.Normal;
        Activate(); Native.Front(Handle);
        var then = jobDone; jobDone = null;
        if (then != null) then(code);
        Invalidate();
    }
    // A short wait on another thread, shown as a card, so the window never freezes.
    void Busy(string title, Func<object, string> work, Action<string> then) {
        job = new Process(); jobTitle = title; jobIsGame = false; jobStart = DateTime.Now; progress = -1; log.Clear(); Invalidate();
        Task.Run(() => work == null ? null : work(null)).ContinueWith(t => BeginInvoke((Action)delegate {
            job = null; then(t.IsFaulted ? t.Exception.GetBaseException().Message : t.Result); Invalidate();
        }));
    }
    // Hide once the game's window shows, so it gets the foreground and its controller layout.
    void WatchGame() {
        if (job == null || !jobIsGame || jobHid) return;
        try {
            job.Refresh();
            IntPtr w = job.MainWindowHandle;
            if (w == IntPtr.Zero || !Native.IsWindowVisible(w)) return;
            Native.Front(w); jobHid = true; Hide();
        } catch { }
    }

    // ---------- input ----------
    static ushort Pad() {
        for (int i = 0; i < 4; i++) {
            Native.XState s;
            try {
                if (Native.XInputGetState(i, out s) == 0) {
                    ushort b = s.Buttons;
                    if (s.LY > 16000) b |= UP; if (s.LY < -16000) b |= DOWN;
                    if (s.LX < -16000) b |= LEFT; if (s.LX > 16000) b |= RIGHT;
                    return b;
                }
            } catch { return 0; }
        }
        return 0;
    }
    DateTime scannedAt; int spritesSeen;
    void Poll() {
        WatchGba();
        if ((DateTime.Now - scannedAt).TotalMilliseconds >= 250) {
            scannedAt = DateTime.Now;
            if (waiting != null && job == null && RunningGames() == "") {
                string id = waiting; waiting = null; pages.Pop();
                Play(Library.Games.Find(g => g.Id == id));
            }
            WatchGame();
        }
        if (Sprites.Arrived != spritesSeen) { spritesSeen = Sprites.Arrived; dirty = true; }
        if (dirty || Animating()) { dirty = false; Invalidate(); }
        if (Visible && ++frame % 2 == 0 && (job != null || starting || scrolling || (pages.Count > 0 && pages.Peek().Main))) Invalidate();
        // Only while the menu is in front: XInput reports the pad to every process.
        if (job != null || starting || Form.ActiveForm != this) { prevPad = Pad(); return; }
        ushort b = Pad(), pressed = (ushort)(b & ~prevPad);
        if ((pressed & A) != 0) Act("ok", true);
        else if ((pressed & B) != 0) Act("back", true);
        else if ((pressed & UP) != 0) Act("up", true);
        else if ((pressed & DOWN) != 0) Act("down", true);
        else if ((pressed & LEFT) != 0) Act("left", true);
        else if ((pressed & RIGHT) != 0) Act("right", true);
        else if ((b & (UP | DOWN)) != 0 && b == prevPad) { if (++held > 14 && held % 4 == 0) Act((b & UP) != 0 ? "up" : "down", true); }
        if (b != prevPad) held = 0;
        prevPad = b;
    }
    protected override bool ProcessCmdKey(ref Message msg, Keys key) {
        if (job == null && !starting) {
            switch (key) {
                case Keys.Up: case Keys.W: Act("up", false); return true;
                case Keys.Down: case Keys.S: Act("down", false); return true;
                case Keys.Left: case Keys.A: Act("left", false); return true;
                case Keys.Right: case Keys.D: Act("right", false); return true;
                case Keys.Enter: Act("ok", false); return true;
                case Keys.Escape: case Keys.Back: case Keys.Space: Act("back", false); return true;
            }
        }
        return base.ProcessCmdKey(ref msg, key);
    }
    // Mouse: hovering selects, a left click runs it, a right click goes back. Rectangles come from the last paint.
    bool scrolling;   // a grid is still gliding, so keep redrawing
    protected override void OnMouseWheel(MouseEventArgs e) {
        base.OnMouseWheel(e);
        if (job == null && e.Delta != 0) Act(e.Delta > 0 ? "up" : "down", false);
    }
    readonly List<KeyValuePair<RectangleF, int>> hits = new List<KeyValuePair<RectangleF, int>>();
    int HitAt(Point pt) {
        var inside = new PointF(pt.X - screenX, pt.Y - screenY);
        foreach (var h in hits) if (h.Key.Contains(inside)) return h.Value;
        return -1;
    }
    // No window frame, so dragging the shell moves the device.
    protected override void OnMouseDown(MouseEventArgs e) {
        base.OnMouseDown(e);
        if (e.Button == MouseButtons.Left && PromptAt(e.Location) == null && HitAt(e.Location) < 0) { Native.ReleaseCapture(); Native.SendMessage(Handle, 0xA1, (IntPtr)2, IntPtr.Zero); }
    }
    readonly List<KeyValuePair<RectangleF, string>> promptHits = new List<KeyValuePair<RectangleF, string>>();
    string hoverPrompt;
    string PromptAt(Point pt) { foreach (var h in promptHits) if (h.Key.Contains(pt)) return h.Value; return null; }
    protected override void OnMouseMove(MouseEventArgs e) {
        base.OnMouseMove(e);
        string over = PromptAt(e.Location);
        if (over != hoverPrompt) { hoverPrompt = over; Cursor = over != null ? Cursors.Hand : Cursors.Default; Invalidate(); }
        int i = HitAt(e.Location);
        if (job == null && i >= 0 && pages.Count > 0 && pages.Peek().Sel != i) { pages.Peek().Sel = i; Invalidate(); }
    }
    protected override void OnMouseClick(MouseEventArgs e) {
        base.OnMouseClick(e);
        string prompt = e.Button == MouseButtons.Left ? PromptAt(e.Location) : null;
        if (prompt == "M") { WindowState = FormWindowState.Minimized; return; }
        if (prompt == "X" && job == null) { Close(); return; }
        if (job != null) return;
        if (prompt != null) {
            Act(prompt == "A" ? "ok" : prompt == "B" ? "back" : prompt == "U" ? "up" : prompt == "D" ? "down" : prompt == "L" ? "left" : prompt == "R" ? "right" : "", false);
            return;
        }
        if (e.Button == MouseButtons.Right) { Act("back", false); return; }
        if (e.Button != MouseButtons.Left) return;
        int i = HitAt(e.Location);
        if (i >= 0 && pages.Count > 0) { pages.Peek().Sel = i; Act("ok", false); }
    }
    // The same action from the other source within 150 ms is dropped: Steam can send a key for a press XInput reports.
    void Act(string a, bool fromPad) {
        if (a == lastAction && fromPad != lastFromPad && (DateTime.Now - lastActionTime).TotalMilliseconds < 150) return;
        lastAction = a; lastFromPad = fromPad; lastActionTime = DateTime.Now;
        if (pages.Count == 0) return;
        var p = pages.Peek();
        if (a == "back") {
            if (p.OnBack != null) p.OnBack(); else pages.Pop();
            if (pages.Count == 0) pages.Push(MainPage());
        } else if (p.Items.Count > 0) {
            if (a == "ok") { var it = p.Items[p.Sel]; if (it.On && it.Run != null) it.Run(); }
            else if (p.Main) p.Sel = GridMove(p.Sel, a, p.Items.Count);
            else if (p.Body == null) p.Sel = PageGridMove(p.Sel, a, p.Items.Count, 1);
            else if (a == "up" || a == "left") p.Sel = (p.Sel - 1 + p.Items.Count) % p.Items.Count;
            else if (a == "down" || a == "right") p.Sel = (p.Sel + 1) % p.Items.Count;
        } else if ((a == "up" || a == "down") && p.Body != null) {
            p.BodyScroll += (a == "down" ? 1 : -1) * bodyFont.Height * 3;   // the drawing keeps it within the text
        } else if (a == "ok" && p.Body != null) {
            if (p.OnBack != null) p.OnBack(); else pages.Pop();
        }
        Invalidate();
    }
    static int GridMove(int sel, string a, int count) {
        if (a == "left") return Math.Max(0, sel - 1);
        if (a == "right") return Math.Min(count - 1, sel + 1);
        if (a == "down") return sel == 0 ? Math.Min(1, count - 1) : Math.Min(count - 1, sel + TileCols);
        if (a == "up") return sel <= TileCols ? 0 : sel - TileCols;
        return sel;
    }
    static int PageGridMove(int sel, string a, int count, int cols) {
        if (a == "left") return Math.Max(0, sel - 1);
        if (a == "right") return Math.Min(count - 1, sel + 1);
        if (a == "down") return sel / cols < (count - 1) / cols ? Math.Min(count - 1, sel + cols) : sel;
        if (a == "up") return sel >= cols ? sel - cols : sel;
        return sel;
    }

    // ---------- drawing ----------
    // nativeMons' own look: a handheld field device (lens and status lights on a coloured shell) showing retro cream
    // dialog boxes with a menu cursor, over a dim tiled floor. The shell takes the colour of the selected version.
    static readonly Color Cream = Color.FromArgb(250, 246, 230), CreamDim = Color.FromArgb(218, 214, 198), Line = Color.FromArgb(38, 42, 54);
    static readonly Color Text1 = Color.FromArgb(38, 40, 50), Text2 = Color.FromArgb(92, 94, 106), Text3 = Color.FromArgb(150, 148, 136);
    static readonly Color FootInk = Color.FromArgb(196, 202, 214);
    readonly Stopwatch clock = Stopwatch.StartNew();

    protected override void OnPaint(PaintEventArgs ev) { Render(ev.Graphics, ClientSize.Width, ClientSize.Height); Outro(ev.Graphics, ClientSize.Width, ClientSize.Height); }

    string screenKey = ""; int screenDepth, screenDir = 1, selIndex = -1; float screenAt = -1, selAt = -1, selEase = 1;
    static float Ease(float x) { x = Math.Max(0, Math.Min(1, x)); float r = 1 - x; return 1 - r * r * r; }
    bool Animating() { float t = (float)clock.Elapsed.TotalSeconds; return t - screenAt < 0.2f || t - selAt < 0.14f || splashShot != null; }
    const int TileCols = 4;   // the home screen's app grid
    static System.Drawing.Drawing2D.GraphicsPath Round(RectangleF r, float rad) {
        var path = new System.Drawing.Drawing2D.GraphicsPath();
        float d = Math.Min(rad * 2, Math.Min(r.Width, r.Height));
        if (d <= 0) { path.AddRectangle(r); return path; }
        path.AddArc(r.X, r.Y, d, d, 180, 90); path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90); path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }
    static void Card(Graphics g, RectangleF r, float rad, Color top, Color bottom, Color edge, float edgeW) {
        using (var path = Round(r, rad)) {
            using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(r, top, bottom, 90f)) g.FillPath(b, path);
            if (edgeW > 0) using (var pen = new Pen(edge, edgeW)) g.DrawPath(pen, path);
        }
    }
    static Color Fade(Color c, int alpha) { return Color.FromArgb(Math.Max(0, Math.Min(255, alpha)), c.R, c.G, c.B); }
    static Color Mix(Color a, Color b, float k) { return Color.FromArgb((int)(a.R + (b.R - a.R) * k), (int)(a.G + (b.G - a.G) * k), (int)(a.B + (b.B - a.B) * k)); }
    static void Write(Graphics g, string s, Font f, Color c, RectangleF r, StringAlignment h, StringAlignment v, bool wrap) {
        using (var b = new SolidBrush(c)) using (var sf = new StringFormat()) {
            sf.Alignment = h; sf.LineAlignment = v; sf.Trimming = StringTrimming.EllipsisCharacter;
            sf.FormatFlags = wrap ? StringFormatFlags.LineLimit : StringFormatFlags.NoWrap;
            g.DrawString(s ?? "", f, b, r, sf);
        }
    }

    // The colour in force: the highlighted game's, else the page's, else the selected game's.
    Theme Cur() {
        if (job == null && pages.Count > 0) {
            var p = pages.Peek();
            if (p.Items.Count > 0 && p.Items[Math.Min(p.Sel, p.Items.Count - 1)].Theme != null) return p.Items[Math.Min(p.Sel, p.Items.Count - 1)].Theme;
            if (p.Theme != null) return p.Theme;
        }
        return current != null ? current.Theme : Theme.Default;
    }

    // A surface in a version's colours, with a small scene that nods to that game. All original shapes, no game art.
    // The scene is drawn once per size and kept as a picture; only the few things that move are drawn each frame.
    class FillLayer { public Bitmap Picture; public PointF[] Sparks = new PointF[0]; }
    static readonly Dictionary<string, FillLayer> fillCache = new Dictionary<string, FillLayer>();
    static FillLayer LayerFor(int w, int h, Theme th) {
        string key = th.Main.ToArgb() + th.Finish + w + "x" + h;
        FillLayer f;
        if (fillCache.TryGetValue(key, out f)) return f;
        if (fillCache.Count > 48) { foreach (var old in fillCache.Values) old.Picture.Dispose(); fillCache.Clear(); }
        f = new FillLayer { Picture = new Bitmap(Math.Max(1, w), Math.Max(1, h)) };
        using (var g = Graphics.FromImage(f.Picture)) {
            g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            var r = new RectangleF(0, 0, w, h);
            // A thin bar has no room for a scene.
            bool scene = h >= 40;
            switch (scene ? th.Finish : "plain") {
                case "crystal": CrystalScene(g, r, th, f); break;
                case "gold": GoldScene(g, r, th, f); break;
                case "silver": SilverScene(g, r, th, f); break;
                case "flame": Enamel(g, r, th); FlameScene(g, r, th); break;
                case "tide": Enamel(g, r, th); TideScene(g, r, th); break;
                case "bolt": Enamel(g, r, th); BoltScene(g, r, th); break;
                default: Enamel(g, r, th); break;
            }
        }
        fillCache[key] = f;
        return f;
    }
    static void VersionFill(Graphics g, RectangleF r, float rad, Theme th, float t) {
        var layer = LayerFor((int)Math.Ceiling(r.Width), (int)Math.Ceiling(r.Height), th);
        using (var path = Round(r, rad)) {
            var st = g.Save();
            g.SetClip(path, System.Drawing.Drawing2D.CombineMode.Intersect);
            g.DrawImageUnscaled(layer.Picture, (int)r.X, (int)r.Y);
            if (r.Height >= 40) {
                if (th.Finish == "crystal") CrystalLive(g, r, layer, t);
                else if (th.Finish == "gold") { Sheen(g, r, t, Color.FromArgb(255, 250, 215)); Motes(g, r, t, Color.FromArgb(255, 180, 60), -1); }
                else if (th.Finish == "silver") { Sheen(g, r, t, Color.White); MoonGlints(g, r, t); }
                else if (th.Finish == "bolt") Crackle(g, r, t);
                else if (th.Finish == "flame") Motes(g, r, t, Color.FromArgb(255, 210, 90), -1);
            } else if (!th.Plain) Sheen(g, r, t, Color.White);
            g.Restore(st);
        }
    }
    static void Gradient(Graphics g, RectangleF r, Color[] colors, float[] stops) {
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(r.X, r.Y - 1, r.Width, r.Height + 2), colors[0], colors[colors.Length - 1], 90f)) {
            b.InterpolationColors = new System.Drawing.Drawing2D.ColorBlend { Colors = colors, Positions = stops };
            g.FillRectangle(b, r);
        }
    }
    static void Enamel(Graphics g, RectangleF r, Theme th) {
        Gradient(g, r, new[] { Mix(th.Main, th.Light, 0.35f), th.Main, th.Deep }, new[] { 0f, 0.55f, 1f });
        using (var b = new SolidBrush(Fade(Color.White, 22))) g.FillEllipse(b, r.X - r.Width * 0.15f, r.Y - r.Height * 1.1f, r.Width * 0.9f, r.Height * 1.7f);
    }
    // Polished metal: bands of light and shade, with fine brushed lines.
    static void MetalBase(Graphics g, RectangleF r, Theme th) {
        Gradient(g, r, new[] { th.Light, th.Main, th.Deep, th.Main, th.Light, th.Main, th.Deep }, new[] { 0f, 0.16f, 0.42f, 0.56f, 0.66f, 0.82f, 1f });
        using (var pen = new Pen(Fade(Color.White, 16), 1)) for (float y = r.Y; y < r.Bottom; y += 3) g.DrawLine(pen, r.X, y, r.Right, y);
        using (var pen = new Pen(Fade(th.Deep, 22), 1)) for (float y = r.Y + 1.5f; y < r.Bottom; y += 5) g.DrawLine(pen, r.X, y, r.Right, y);
    }
    static void Sheen(Graphics g, RectangleF r, float t, Color c) {
        float span = r.Width + r.Height * 2, w = Math.Max(r.Height * 0.4f, r.Width * 0.08f);
        float x = r.X - r.Height + ((t * r.Width * 0.22f) % (span * 1.6f));
        foreach (var layer in new[] { new[] { 1f, 46f }, new[] { 0.4f, 120f } }) {
            float wd = w * layer[0], c0 = x + w / 2;
            var band = new[] { new PointF(c0 - wd / 2 + r.Height, r.Y), new PointF(c0 + wd / 2 + r.Height, r.Y), new PointF(c0 + wd / 2, r.Bottom), new PointF(c0 - wd / 2, r.Bottom) };
            using (var b = new SolidBrush(Fade(c, (int)layer[1]))) g.FillPolygon(b, band);
        }
    }
    static void Star(Graphics g, float x, float y, float s, Color c) {
        using (var b = new SolidBrush(c))
            g.FillPolygon(b, new[] { new PointF(x, y - s), new PointF(x + s * 0.16f, y - s * 0.16f), new PointF(x + s, y), new PointF(x + s * 0.16f, y + s * 0.16f), new PointF(x, y + s), new PointF(x - s * 0.16f, y + s * 0.16f), new PointF(x - s, y), new PointF(x - s * 0.16f, y - s * 0.16f) });
    }
    // Little lights drifting up (dir -1) or down (dir 1).
    static void Motes(Graphics g, RectangleF r, float t, Color c, int dir) {
        float u = Math.Min(r.Height, r.Width * 0.55f);
        for (int i = 0; i < 14; i++) {
            float seed = (i * 0.618f) % 1f, speed = 0.05f + seed * 0.07f;
            float life = (t * speed + seed * 3f) % 1f;
            float y = dir < 0 ? r.Bottom - life * r.Height : r.Y + life * r.Height;
            float x = r.X + ((i * 0.381f + 0.5f * (float)Math.Sin(t * 0.3f + i) * 0.04f) % 1f) * r.Width;
            float size = u * (0.006f + 0.01f * seed);
            float a = (float)Math.Sin(life * Math.PI);
            using (var b = new SolidBrush(Fade(c, (int)(220 * a)))) g.FillEllipse(b, x - size, y - size, size * 2, size * 2);
        }
    }

    // ----- Crystal: Suicune's aurora over a night sky, a cut gem cluster, falling diamond dust.
    static void CrystalScene(Graphics g, RectangleF r, Theme th, FillLayer f) {
        Gradient(g, r, new[] { th.Deep, Mix(th.Deep, th.Main, 0.6f), th.Main, th.Light }, new[] { 0f, 0.42f, 0.78f, 1f });
        float u = Math.Min(r.Height, r.Width * 0.55f);
        // Aurora ribbons.
        Aurora(g, r, u, 0.10f, 0.20f, 1.0f, 0.3f, Color.FromArgb(96, 240, 208));
        Aurora(g, r, u, 0.24f, 0.15f, 1.4f, 2.0f, Color.FromArgb(150, 130, 255));
        Aurora(g, r, u, 0.34f, 0.10f, 0.8f, 4.1f, Color.FromArgb(120, 225, 255));
        // Stars.
        var rnd = new Random(1999);
        for (int i = 0; i < (int)(r.Width * r.Height / (u * u) * 40); i++) {
            float x = r.X + (float)rnd.NextDouble() * r.Width, y = r.Y + (float)rnd.NextDouble() * r.Height * 0.55f, s = u * (0.003f + 0.004f * (float)rnd.NextDouble());
            using (var b = new SolidBrush(Fade(Color.White, 90 + rnd.Next(150)))) g.FillEllipse(b, x, y, s, s);
        }
        // The gem cluster: a dim back row, then a bright front row, each rising towards the right.
        var tips = new List<PointF>();
        float span = r.Width;
        for (int row = 0; row < 2; row++) {
            int n = Math.Max(5, (int)(span / (u * (row == 0 ? 0.13f : 0.16f))));
            for (int i = 0; i < n; i++) {
                float k = (i + (float)rnd.NextDouble() * 0.6f) / n;
                float height = u * (row == 0 ? 0.92f : 0.68f) * (float)Math.Pow(k, 1.5) * (0.65f + 0.35f * (float)rnd.NextDouble()) + u * 0.1f;
                float width = u * (0.1f + 0.1f * (float)rnd.NextDouble()) * (0.6f + 0.6f * k);
                float lean = (k - 0.55f) * width * 1.3f + width * 0.5f * ((float)rnd.NextDouble() - 0.5f);
                tips.Add(Shard(g, r.X + k * span - width / 2, r.Bottom + 2, width, height, lean, th, row == 0 ? 0.55f : 1f, rnd));
            }
        }
        f.Sparks = tips.ToArray();
    }
    static void Aurora(Graphics g, RectangleF r, float u, float at, float depth, float waves, float phase, Color c) {
        int steps = 48;
        var top = new PointF[steps + 1]; var low = new PointF[steps + 1];
        for (int i = 0; i <= steps; i++) {
            float k = i / (float)steps, x = r.X + k * r.Width;
            float y = r.Y + u * at + (float)Math.Sin(k * Math.PI * 2 * waves + phase) * u * 0.05f + (float)Math.Sin(k * 9 + phase) * u * 0.015f;
            top[i] = new PointF(x, y);
            low[i] = new PointF(x, y + u * depth * (0.7f + 0.3f * (float)Math.Sin(k * 7 + phase * 2)));
        }
        // Soft curtains of thin vertical strokes, brightest at the upper edge.
        for (int i = 0; i < steps; i++) {
            var a = top[i]; var b = low[i];
            var quad = new[] { top[i], top[i + 1], low[i + 1], low[i] };
            if (b.Y <= a.Y + 1) continue;
            using (var br = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(a.X, a.Y - 1, 4, b.Y - a.Y + 2), Fade(c, 110), Fade(c, 0), 90f)) g.FillPolygon(br, quad);
        }
    }
    // One crystal: a hexagonal prism with a cut tip, three lit faces and bright edges. Returns its tip.
    static PointF Shard(Graphics g, float x, float bottom, float w, float h, float lean, Theme th, float strength, Random rnd) {
        float shoulder = bottom - h * 0.8f, tipY = bottom - h;
        float x1 = x, x2 = x + w * 0.3f, x3 = x + w * 0.72f, x4 = x + w;
        float tipX = x + w * 0.5f + lean;
        PointF sL = new PointF(x1 + lean * 0.75f, shoulder + h * 0.03f), sM1 = new PointF(x2 + lean * 0.75f, shoulder - h * 0.02f), sM2 = new PointF(x3 + lean * 0.75f, shoulder - h * 0.02f), sR = new PointF(x4 + lean * 0.75f, shoulder + h * 0.03f), tip = new PointF(tipX, tipY);
        int a = (int)(235 * strength);
        Color pale = Mix(th.Light, Color.White, 0.5f), mid = Mix(th.Main, th.Light, 0.45f), dark = Mix(th.Deep, th.Main, 0.45f);
        Face(g, new[] { new PointF(x1, bottom), sL, sM1, new PointF(x2, bottom) }, Fade(pale, a), Fade(mid, a));
        Face(g, new[] { new PointF(x2, bottom), sM1, sM2, new PointF(x3, bottom) }, Fade(mid, a), Fade(th.Main, a));
        Face(g, new[] { new PointF(x3, bottom), sM2, sR, new PointF(x4, bottom) }, Fade(th.Main, a), Fade(dark, a));
        Face(g, new[] { sL, tip, sM1 }, Fade(Color.White, a), Fade(pale, a));
        Face(g, new[] { sM1, tip, sM2 }, Fade(pale, a), Fade(mid, a));
        Face(g, new[] { sM2, tip, sR }, Fade(mid, a), Fade(dark, a));
        // Light caught inside: a thin diagonal of colour, like a prism.
        Color[] prism = { Color.FromArgb(255, 170, 220), Color.FromArgb(170, 255, 230), Color.FromArgb(200, 180, 255) };
        using (var pen = new Pen(Fade(prism[rnd.Next(3)], (int)(120 * strength)), Math.Max(1f, w * 0.05f)))
            g.DrawLine(pen, x2 + w * 0.1f, bottom - h * 0.12f, x3 + lean * 0.5f - w * 0.1f, shoulder + h * 0.14f);
        using (var pen = new Pen(Fade(Color.White, (int)(190 * strength)), 1)) {
            g.DrawLines(pen, new[] { sL, tip, sR });
            g.DrawLine(pen, sM1, tip); g.DrawLine(pen, sM2, tip);
            g.DrawLines(pen, new[] { sL, sM1, sM2, sR });
            g.DrawLine(pen, sM1, new PointF(x2, bottom)); g.DrawLine(pen, sM2, new PointF(x3, bottom));
        }
        return tip;
    }
    static void Face(Graphics g, PointF[] pts, Color a, Color b) {
        float minX = pts.Min(p => p.X), maxX = pts.Max(p => p.X), minY = pts.Min(p => p.Y), maxY = pts.Max(p => p.Y);
        using (var br = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(minX - 1, minY - 1, maxX - minX + 2, maxY - minY + 2), a, b, 65f)) g.FillPolygon(br, pts);
    }
    static void CrystalLive(Graphics g, RectangleF r, FillLayer f, float t) {
        float u = Math.Min(r.Height, r.Width * 0.55f);
        // Diamond dust.
        for (int i = 0; i < 16; i++) {
            float seed = (i * 0.618f) % 1f, life = (t * (0.03f + seed * 0.04f) + seed * 5f) % 1f;
            float x = r.X + ((i * 0.381f + life * 0.08f) % 1f) * r.Width, y = r.Y + life * r.Height;
            float beat = 0.5f + 0.5f * (float)Math.Sin(t * 3f + i * 2.1f);
            Star(g, x, y, u * (0.008f + 0.012f * seed) * (0.6f + 0.4f * beat), Fade(Color.White, (int)(90 + 150 * beat)));
        }
        // A glint that travels from tip to tip.
        if (f.Sparks.Length > 0) for (int i = 0; i < 3; i++) {
            float phase = t * 0.7f + i * 0.33f;
            int idx = (int)(Math.Floor(phase) * 7 + i * 13) % f.Sparks.Length;
            float beat = (float)Math.Sin((phase - Math.Floor(phase)) * Math.PI);
            var p = f.Sparks[Math.Abs(idx)];
            Star(g, r.X + p.X, r.Y + p.Y, u * 0.05f * beat, Fade(Color.White, (int)(255 * beat)));
        }
    }

    // ----- Gold: dawn light, a rainbow feather fan and a tiered tower, for Ho-Oh and the Bell Tower.
    static void GoldScene(Graphics g, RectangleF r, Theme th, FillLayer f) {
        MetalBase(g, r, th);
        float u = Math.Min(r.Height, r.Width * 0.55f);
        // On a wide panel the scene sits higher, clear of what is drawn over its lower part.
        bool wide = r.Width > r.Height;
        var sun = new PointF(r.X + r.Width * 0.76f, wide ? r.Y + r.Height * 0.56f : r.Bottom - u * 0.14f);
        float reach = Math.Max(r.Width, r.Height) * 1.3f;
        // Sunburst.
        for (int i = 0; i < 18; i++) {
            double a0 = Math.PI + i * Math.PI / 18, a1 = a0 + Math.PI / 36;
            using (var b = new SolidBrush(Fade(Color.FromArgb(255, 248, 200), 52)))
                g.FillPolygon(b, new[] { sun, new PointF(sun.X + (float)Math.Cos(a0) * reach, sun.Y + (float)Math.Sin(a0) * reach), new PointF(sun.X + (float)Math.Cos(a1) * reach, sun.Y + (float)Math.Sin(a1) * reach) });
        }
        // Rainbow feathers fanned out behind the tower.
        Color[] rainbow = { Color.FromArgb(232, 56, 48), Color.FromArgb(244, 128, 32), Color.FromArgb(250, 208, 48), Color.FromArgb(72, 184, 88), Color.FromArgb(48, 136, 216), Color.FromArgb(120, 84, 200) };
        int feathers = 13;
        for (int i = 0; i < feathers; i++) {
            float k = i / (float)(feathers - 1);
            double angle = Math.PI * (1.08 + 0.84 * k);
            float len = u * (0.62f + 0.28f * (float)Math.Sin(k * Math.PI));
            Color c = Mix(rainbow[(int)(k * 4.999f)], rainbow[(int)(k * 4.999f) + 1], (k * 4.999f) % 1f);
            Feather(g, sun, angle, len, u * 0.082f, c);
        }
        // The sun's disc.
        float d = u * 0.21f;
        using (var path = new System.Drawing.Drawing2D.GraphicsPath()) {
            path.AddEllipse(sun.X - d * 1.7f, sun.Y - d * 1.7f, d * 3.4f, d * 3.4f);
            using (var pg = new System.Drawing.Drawing2D.PathGradientBrush(path)) { pg.CenterColor = Fade(Color.FromArgb(255, 252, 220), 245); pg.SurroundColors = new[] { Fade(Color.FromArgb(255, 220, 120), 0) }; g.FillPath(pg, path); }
        }
        Tower(g, sun.X, wide ? sun.Y + u * 0.14f : r.Bottom + 2, u * 0.62f, u * 0.25f, Mix(th.Deep, Color.Black, 0.35f), Fade(th.Light, 220));
    }
    static void Feather(Graphics g, PointF from, double angle, float len, float wid, Color c) {
        var st = g.Save();
        g.TranslateTransform(from.X, from.Y); g.RotateTransform((float)(angle * 180 / Math.PI));
        using (var path = new System.Drawing.Drawing2D.GraphicsPath()) {
            path.AddBezier(new PointF(len * 0.12f, 0), new PointF(len * 0.45f, -wid), new PointF(len * 0.85f, -wid * 0.75f), new PointF(len, 0));
            path.AddBezier(new PointF(len, 0), new PointF(len * 0.85f, wid * 0.75f), new PointF(len * 0.45f, wid), new PointF(len * 0.12f, 0));
            using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(0, -wid, len + 1, wid * 2), Fade(Mix(c, Color.White, 0.25f), 205), Fade(c, 245), 0f)) g.FillPath(b, path);
            using (var pen = new Pen(Fade(Color.FromArgb(70, 40, 10), 190), 1)) g.DrawPath(pen, path);
            using (var pen = new Pen(Fade(Color.White, 170), 1)) g.DrawLine(pen, len * 0.15f, 0, len * 0.96f, 0);
        }
        g.Restore(st);
    }
    // A tiered tower silhouette with upswept eaves. Bottom centre at (cx, bottom).
    static void Tower(Graphics g, float cx, float bottom, float height, float width, Color fill, Color edge) {
        int tiers = 5; float y = bottom, tierH = height / (tiers + 0.7f);
        using (var b = new SolidBrush(fill)) using (var pen = new Pen(edge, 1)) {
            for (int i = 0; i < tiers; i++) {
                float k = 1 - i / (float)(tiers + 1), w = width * k, wallW = w * 0.62f;
                var wall = new RectangleF(cx - wallW / 2, y - tierH * 0.62f, wallW, tierH * 0.62f);
                g.FillRectangle(b, wall);
                using (var lit = new SolidBrush(Fade(edge, 170))) for (int s = 1; s < 4; s++) g.FillRectangle(lit, cx - wallW / 2 + wallW * s / 4f - wallW * 0.035f, wall.Y + wall.Height * 0.25f, wallW * 0.07f, wall.Height * 0.55f);
                float ry = y - tierH * 0.62f;
                var roof = new[] { new PointF(cx - w * 0.68f, ry - tierH * 0.1f), new PointF(cx - w * 0.5f, ry + tierH * 0.02f), new PointF(cx + w * 0.5f, ry + tierH * 0.02f), new PointF(cx + w * 0.68f, ry - tierH * 0.1f), new PointF(cx + w * 0.34f, ry - tierH * 0.38f), new PointF(cx - w * 0.34f, ry - tierH * 0.38f) };
                g.FillPolygon(b, roof); g.DrawPolygon(pen, roof);
                y -= tierH;
            }
            g.FillRectangle(b, cx - width * 0.012f, y - tierH * 0.5f, width * 0.024f, tierH * 0.9f);
        }
    }

    // ----- Silver: moonlight on a whirlpool and curved fins breaking the water, for Lugia and the Whirl Islands.
    static void SilverScene(Graphics g, RectangleF r, Theme th, FillLayer f) {
        MetalBase(g, r, th);
        float u = Math.Min(r.Height, r.Width * 0.55f);
        Gradient(g, new RectangleF(r.X, r.Y, r.Width, r.Height), new[] { Fade(Color.FromArgb(20, 40, 84), 128), Fade(Color.FromArgb(40, 70, 120), 70), Fade(Color.FromArgb(30, 60, 110), 96) }, new[] { 0f, 0.55f, 1f });
        // The moon, with a soft halo.
        var moon = new PointF(r.X + r.Width * 0.8f, r.Y + u * 0.26f);
        float mr = u * 0.17f;
        using (var path = new System.Drawing.Drawing2D.GraphicsPath()) {
            path.AddEllipse(moon.X - mr * 2.4f, moon.Y - mr * 2.4f, mr * 4.8f, mr * 4.8f);
            using (var pg = new System.Drawing.Drawing2D.PathGradientBrush(path)) { pg.CenterColor = Fade(Color.White, 170); pg.SurroundColors = new[] { Fade(Color.White, 0) }; g.FillPath(pg, path); }
        }
        using (var b = new SolidBrush(Color.FromArgb(246, 249, 255))) g.FillEllipse(b, moon.X - mr, moon.Y - mr, mr * 2, mr * 2);
        using (var b = new SolidBrush(Fade(th.Main, 130))) { g.FillEllipse(b, moon.X - mr * 0.5f, moon.Y - mr * 0.45f, mr * 0.42f, mr * 0.34f); g.FillEllipse(b, moon.X + mr * 0.15f, moon.Y + mr * 0.15f, mr * 0.5f, mr * 0.4f); g.FillEllipse(b, moon.X + mr * 0.22f, moon.Y - mr * 0.55f, mr * 0.24f, mr * 0.22f); }
        // The sea, darker going down, with the whirlpool in it.
        float sea = r.Bottom - u * 0.4f;
        var seaRect = new RectangleF(r.X, sea, r.Width, r.Bottom - sea);
        using (var path = WavePath(r, sea, u * 0.02f, 0.3f))
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(r.X, sea - u * 0.03f, r.Width, seaRect.Height + u * 0.05f), Fade(Color.FromArgb(120, 150, 186), 230), Fade(Color.FromArgb(28, 46, 84), 250), 90f)) g.FillPath(b, path);
        var clip = g.Save();
        g.SetClip(seaRect, System.Drawing.Drawing2D.CombineMode.Intersect);
        var eye = new PointF(r.X + r.Width * 0.68f, sea + seaRect.Height * 0.55f);
        for (int arm = 0; arm < 3; arm++) {
            var pts = new List<PointF>();
            for (double a = 0; a < Math.PI * 5; a += 0.14) {
                float rad = (float)(u * 0.018f * Math.Exp(0.2 * a));
                double ang = a + arm * Math.PI * 2 / 3;
                pts.Add(new PointF(eye.X + (float)Math.Cos(ang) * rad * 1.9f, eye.Y + (float)Math.Sin(ang) * rad * 0.62f));
            }
            using (var pen = new Pen(Fade(Color.FromArgb(226, 238, 255), 175), Math.Max(1f, u * 0.008f))) g.DrawLines(pen, pts.ToArray());
        }
        using (var b = new SolidBrush(Fade(Color.FromArgb(10, 20, 44), 200))) g.FillEllipse(b, eye.X - u * 0.05f, eye.Y - u * 0.018f, u * 0.1f, u * 0.036f);
        g.Restore(clip);
        // Fins rising and curving back, as something huge passes under the surface.
        for (int i = 0; i < 5; i++) {
            float k = i / 4f;
            Fin(g, r.X + r.Width * (0.1f + k * 0.34f), sea + u * 0.035f, u * (0.3f - 0.14f * Math.Abs(k - 0.35f) * 2), u * 0.12f, Color.FromArgb(224, 232, 244), Color.FromArgb(46, 62, 104));
        }
        using (var path = WavePath(r, sea + u * 0.03f, u * 0.014f, 2.2f)) using (var b = new SolidBrush(Fade(Color.FromArgb(52, 78, 124), 235))) g.FillPath(b, path);
        using (var pen = new Pen(Fade(Color.White, 170), 1)) for (int i = 0; i < 2; i++) using (var path = WaveLine(r, sea + u * (0.03f + i * 0.09f), u * 0.012f, 2.2f + i)) g.DrawPath(pen, path);
    }
    static System.Drawing.Drawing2D.GraphicsPath WaveLine(RectangleF r, float y, float amp, float phase) {
        var pts = new List<PointF>();
        float step = Math.Max(4, r.Width / 90);
        for (float x = r.X - step; x <= r.Right + step; x += step) pts.Add(new PointF(x, y + (float)Math.Sin(x / amp * 0.12f + phase) * amp));
        var path = new System.Drawing.Drawing2D.GraphicsPath();
        path.AddLines(pts.ToArray());
        return path;
    }
    static System.Drawing.Drawing2D.GraphicsPath WavePath(RectangleF r, float y, float amp, float phase) {
        var path = WaveLine(r, y, amp, phase);
        path.AddLine(r.Right + 6, y, r.Right + 6, r.Bottom + 4);
        path.AddLine(r.Right + 6, r.Bottom + 4, r.X - 6, r.Bottom + 4);
        path.CloseFigure();
        return path;
    }
    static void Fin(Graphics g, float x, float y, float h, float w, Color top, Color edge) {
        using (var path = new System.Drawing.Drawing2D.GraphicsPath()) {
            path.AddBezier(new PointF(x, y), new PointF(x + w * 0.15f, y - h * 0.6f), new PointF(x + w * 0.5f, y - h * 0.9f), new PointF(x + w * 1.15f, y - h));
            path.AddBezier(new PointF(x + w * 1.15f, y - h), new PointF(x + w * 0.85f, y - h * 0.55f), new PointF(x + w * 0.95f, y - h * 0.2f), new PointF(x + w, y));
            path.CloseFigure();
            using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(x - 1, y - h - 1, w * 1.2f + 2, h + 2), top, Mix(top, edge, 0.6f), 80f)) g.FillPath(b, path);
            using (var pen = new Pen(edge, Math.Max(1f, w * 0.03f))) g.DrawPath(pen, path);
        }
    }
    static void MoonGlints(Graphics g, RectangleF r, float t) {
        float u = Math.Min(r.Height, r.Width * 0.55f), sea = r.Bottom - u * 0.4f;
        for (int i = 0; i < 12; i++) {
            float seed = (i * 0.618f) % 1f, beat = 0.5f + 0.5f * (float)Math.Sin(t * 1.6f + i * 1.9f);
            float y = sea + u * 0.05f + seed * (u * 0.33f), x = r.X + r.Width * 0.8f + (seed - 0.5f) * u * 0.34f + (float)Math.Sin(t * 0.6f + i) * u * 0.02f;
            using (var pen = new Pen(Fade(Color.White, (int)(200 * beat)), Math.Max(1f, u * 0.006f))) g.DrawLine(pen, x - u * 0.03f, y, x + u * 0.03f, y);
        }
    }

    // ----- Red: tongues of flame. Blue: scalloped tide. Yellow: lightning.
    static void FlameScene(Graphics g, RectangleF r, Theme th) {
        float u = Math.Min(r.Height, r.Width * 0.55f);
        Color[] layers = { Fade(Color.FromArgb(120, 16, 8), 170), Fade(Color.FromArgb(244, 110, 24), 215), Fade(Color.FromArgb(255, 204, 64), 225) };
        float[] tall = { 0.62f, 0.42f, 0.24f };
        var rnd = new Random(1996);
        for (int l = 0; l < 3; l++) {
            int n = Math.Max(6, (int)(r.Width / (u * (0.2f - l * 0.03f))));
            float cw = r.Width / n;
            for (int i = -1; i <= n; i++) {
                float k = (i + 0.5f) / n, h = u * tall[l] * (0.35f + 0.65f * k * k) * (0.7f + 0.3f * (float)rnd.NextDouble());
                float x = r.X + i * cw, b = r.Bottom + 2;
                using (var path = new System.Drawing.Drawing2D.GraphicsPath()) {
                    path.AddBezier(new PointF(x - cw * 0.3f, b), new PointF(x, b - h * 0.5f), new PointF(x + cw * 0.05f, b - h * 0.7f), new PointF(x + cw * 0.75f, b - h));
                    path.AddBezier(new PointF(x + cw * 0.75f, b - h), new PointF(x + cw * 0.7f, b - h * 0.55f), new PointF(x + cw * 1.2f, b - h * 0.35f), new PointF(x + cw * 1.3f, b));
                    path.CloseFigure();
                    using (var br = new SolidBrush(layers[l])) g.FillPath(br, path);
                }
            }
        }
    }
    static void TideScene(Graphics g, RectangleF r, Theme th) {
        float u = Math.Min(r.Height, r.Width * 0.55f), s = u * 0.15f;
        // Overlapping scallops, row on row, growing towards the lower right.
        int rows = (int)(u * 0.6f / (s * 0.55f));
        for (int row = 0; row < rows; row++) {
            float y = r.Bottom - u * 0.6f + row * s * 0.55f;
            float a = 0.25f + 0.6f * row / Math.Max(1, rows - 1);
            for (float x = r.X - s + (row % 2) * s / 2; x < r.Right + s; x += s) {
                using (var b = new SolidBrush(Fade(Mix(th.Main, th.Deep, a), 235))) g.FillEllipse(b, x, y, s, s * 1.3f);
                using (var pen = new Pen(Fade(th.Light, 170), Math.Max(1f, u * 0.005f))) g.DrawArc(pen, x, y, s, s * 1.3f, 195, 150);
                using (var pen = new Pen(Fade(Color.White, 56), Math.Max(1f, u * 0.004f))) g.DrawArc(pen, x + s * 0.2f, y + s * 0.2f, s * 0.6f, s * 0.8f, 200, 140);
            }
        }
    }
    static readonly float[][] BoltShape = { new[] { 0.55f, 0f }, new[] { 0.1f, 0.55f }, new[] { 0.42f, 0.55f }, new[] { 0.22f, 1f }, new[] { 0.9f, 0.38f }, new[] { 0.55f, 0.38f }, new[] { 0.82f, 0f } };
    static void Bolt(Graphics g, float x, float y, float h, Color fill, Color edge) {
        var pts = BoltShape.Select(p => new PointF(x + p[0] * h * 0.62f, y + p[1] * h)).ToArray();
        using (var b = new SolidBrush(fill)) g.FillPolygon(b, pts);
        using (var pen = new Pen(edge, Math.Max(1.5f, h * 0.018f))) g.DrawPolygon(pen, pts);
    }
    static void BoltScene(Graphics g, RectangleF r, Theme th) {
        float u = Math.Min(r.Height, r.Width * 0.55f);
        // A burst of zigzag stripes, a big bolt, and two rosy round marks.
        var o = new PointF(r.X + r.Width * 0.8f, r.Y + r.Height * 0.5f);
        float reach = Math.Max(r.Width, r.Height) * 1.3f;
        for (int i = 0; i < 12; i++) {
            double a0 = i * Math.PI / 6, a1 = a0 + Math.PI / 12;
            using (var b = new SolidBrush(Fade(Color.White, 44))) g.FillPolygon(b, new[] { o, new PointF(o.X + (float)Math.Cos(a0) * reach, o.Y + (float)Math.Sin(a0) * reach), new PointF(o.X + (float)Math.Cos(a1) * reach, o.Y + (float)Math.Sin(a1) * reach) });
        }
        Bolt(g, o.X - u * 0.28f, o.Y - u * 0.45f, u * 0.9f, Color.FromArgb(255, 251, 212), Color.FromArgb(70, 50, 16));
        using (var b = new SolidBrush(Color.FromArgb(232, 72, 60))) using (var pen = new Pen(Color.FromArgb(120, 40, 24), 1)) foreach (var c in new[] { new PointF(r.X + r.Width * 0.58f, r.Bottom - u * 0.16f), new PointF(r.X + r.Width * 0.95f, r.Bottom - u * 0.16f) }) { float d = u * 0.09f; g.FillEllipse(b, c.X - d, c.Y - d, d * 2, d * 2); g.DrawEllipse(pen, c.X - d, c.Y - d, d * 2, d * 2); }
    }
    static void Crackle(Graphics g, RectangleF r, float t) {
        float u = Math.Min(r.Height, r.Width * 0.55f);
        for (int i = 0; i < 4; i++) {
            float phase = t * 1.3f + i * 0.31f; int seed = (int)Math.Floor(phase) * 31 + i * 17;
            float beat = (float)Math.Sin((phase - Math.Floor(phase)) * Math.PI);
            var rnd = new Random(seed);
            float x = r.X + (float)rnd.NextDouble() * r.Width, y = r.Y + (float)rnd.NextDouble() * r.Height;
            var pts = new PointF[5];
            for (int k = 0; k < 5; k++) { pts[k] = new PointF(x, y); x += u * (0.03f * (float)(rnd.NextDouble() * 2 - 1)) + u * 0.012f; y += u * 0.024f; }
            using (var pen = new Pen(Fade(Color.White, (int)(230 * beat)), Math.Max(1.2f, u * 0.006f))) g.DrawLines(pen, pts);
        }
    }

    // A retro dialog box: dark outline, cream paper, a thin inner line in the version's colour.
    static void Box(Graphics g, RectangleF r, float rad, Theme th, bool selected, bool on) {
        float o = Math.Max(2f, r.Height * 0.02f);
        using (var path = Round(new RectangleF(r.X + 2, r.Y + 3, r.Width, r.Height), rad)) using (var b = new SolidBrush(Fade(Color.Black, 90))) g.FillPath(b, path);
        Card(g, r, rad, selected ? th.Deep : Line, selected ? th.Deep : Line, Color.Empty, 0);
        var inner = new RectangleF(r.X + o, r.Y + o, r.Width - 2 * o, r.Height - 2 * o);
        Card(g, inner, Math.Max(1, rad - o), on ? Cream : CreamDim, on ? Color.FromArgb(240, 234, 214) : CreamDim, Color.Empty, 0);
        var line = new RectangleF(inner.X + o, inner.Y + o, inner.Width - 2 * o, inner.Height - 2 * o);
        using (var path = Round(line, Math.Max(1, rad - 2 * o))) using (var pen = new Pen(Fade(on ? th.Main : Text3, selected ? 255 : 95), 1)) g.DrawPath(pen, path);
    }
    // Pixel art stays crisp when it's enlarged.
    static void DrawSprite(Graphics g, Bitmap b, RectangleF r) {
        var mode = g.InterpolationMode; var offset = g.PixelOffsetMode;
        g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
        g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;
        g.DrawImage(b, r);
        g.InterpolationMode = mode; g.PixelOffsetMode = offset;
    }
    // The menu cursor.
    static void Cursor3(Graphics g, float x, float cy, float s, Color c) {
        using (var b = new SolidBrush(c)) g.FillPolygon(b, new[] { new PointF(x, cy - s), new PointF(x + s * 1.2f, cy), new PointF(x, cy + s) });
    }
    // The device's big lens, with its glint and the three small status lights.
    static void Lens(Graphics g, float cx, float cy, float r, float t, bool lights) {
        using (var b = new SolidBrush(Line)) g.FillEllipse(b, cx - r * 1.14f, cy - r * 1.14f, r * 2.28f, r * 2.28f);
        using (var b = new SolidBrush(Color.FromArgb(240, 242, 246))) g.FillEllipse(b, cx - r, cy - r, r * 2, r * 2);
        float i = r * 0.76f;
        var rect = new RectangleF(cx - i, cy - i, i * 2, i * 2);
        float pulse = 0.5f + 0.5f * (float)Math.Sin(t * 1.8f);
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(rect, Mix(Color.FromArgb(70, 170, 245), Color.FromArgb(130, 215, 255), pulse), Color.FromArgb(16, 74, 160), 60f)) g.FillEllipse(b, rect);
        using (var b = new SolidBrush(Fade(Color.White, 200))) g.FillEllipse(b, cx - i * 0.58f, cy - i * 0.62f, i * 0.55f, i * 0.42f);
        if (!lights) return;
        float d = r * 0.42f, x = cx + r * 1.5f, y = cy - r * 0.86f;
        Color[] on = { Color.FromArgb(240, 70, 60), Color.FromArgb(250, 205, 50), Color.FromArgb(90, 205, 90) };
        for (int k = 0; k < 3; k++) {
            int lit = ((int)(t * 1.5f)) % 3;
            using (var b = new SolidBrush(Line)) g.FillEllipse(b, x - 1.5f, y - 1.5f, d + 3, d + 3);
            using (var b = new SolidBrush(k == lit ? on[k] : Mix(on[k], Line, 0.45f))) g.FillEllipse(b, x, y, d, d);
            x += d * 1.45f;
        }
    }
    // A generic cartridge in the version's colour, `len` px wide with its top left at (x, y).
    static void Cart(Graphics g, float x, float y, float len, Theme th) {
        var st = g.Save();
        g.TranslateTransform(x, y); g.ScaleTransform(len / 100f, len / 100f);
        var rect = new RectangleF(10, 0, 80, 94);
        using (var shell = Round(rect, 7)) {
            using (var b = new SolidBrush(Line)) g.FillPath(b, shell);
            using (var inner = Round(new RectangleF(13, 3, 74, 88), 5)) using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(rect, th.Plain ? th.Main : th.Light, th.Deep, 90f)) g.FillPath(b, inner);
        }
        using (var lb = new SolidBrush(Cream)) using (var label = Round(new RectangleF(22, 13, 56, 44), 3)) g.FillPath(lb, label);
        using (var ink = new SolidBrush(th.Plain ? th.Main : th.Deep)) g.FillEllipse(ink, 39, 24, 22, 22);
        using (var slot = new SolidBrush(Fade(Line, 200))) for (int i = 0; i < 7; i++) g.FillRectangle(slot, 23 + i * 8f, 68, 4.4f, 16);
        g.Restore(st);
    }
    // The dim tiled floor behind everything, tinted by the version, drawn once per size and colour.
    Bitmap floor; string floorKey;
    void Floor(Graphics g, int w, int h, Theme th) {
        string key = w + "x" + h + th.Main.ToArgb();
        if (floor == null || floorKey != key) {
            if (floor != null) floor.Dispose();
            floor = new Bitmap(Math.Max(1, w), Math.Max(1, h)); floorKey = key;
            int s = Math.Max(16, h / 20);
            Color a = Mix(Color.FromArgb(24, 30, 40), th.Main, 0.1f), b = Mix(Color.FromArgb(30, 37, 48), th.Main, 0.14f);
            using (var f = Graphics.FromImage(floor)) using (var ba = new SolidBrush(a)) using (var bb = new SolidBrush(b)) using (var dot = new SolidBrush(Fade(th.Light, 26))) {
                f.Clear(a);
                for (int y = 0; y * s < h; y++) for (int x = 0; x * s < w; x++) {
                    if ((x + y) % 2 == 0) f.FillRectangle(bb, x * s, y * s, s, s);
                    if ((x * 7 + y * 13) % 11 == 0) f.FillRectangle(dot, x * s + s * 0.42f, y * s + s * 0.42f, s * 0.16f, s * 0.16f);
                }
            }
        }
        g.DrawImageUnscaled(floor, 0, 0);
    }
    static float Rolling(RectangleF track, float len, float t) {
        float span = track.Width + len;
        return track.X - len + (t * track.Width * 0.3f) % span;
    }
    // The whole window is the device: its shell, top lens, hardware controls, and an inset screen with the menu on it.
    float screenX, screenY;
    RectangleF ScreenRect(int w, int h) { return new RectangleF(w * 0.06f, h * 0.115f, w * 0.88f, h * 0.665f); }
    void Render(Graphics g, int w, int h) {
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
        hits.Clear(); promptHits.Clear();
        var th = Cur();
        float t = (float)clock.Elapsed.TotalSeconds;
        g.Clear(Line);
        Shell(g, w, h, th, t);
        var s = ScreenRect(w, h);
        screenX = s.X; screenY = s.Y;
        float rad = w * 0.03f;
        // A black glass front, set into a recess in the case.
        float inset = w * 0.016f, rim = inset * 0.45f;
        var front = new RectangleF(s.X - inset, s.Y - inset, s.Width + 2 * inset, s.Height + 2 * inset);
        var recess = new RectangleF(front.X - rim, front.Y - rim, front.Width + 2 * rim, front.Height + 2 * rim);
        using (var path = Round(recess, rad * 1.6f)) using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(recess, Mix(th.Deep, Color.Black, 0.55f), Mix(th.Main, Color.White, 0.25f), 90f)) g.FillPath(b, path);
        using (var bezel = Round(front, rad * 1.35f)) using (var b = new SolidBrush(Color.FromArgb(12, 12, 14))) g.FillPath(b, bezel);
        var outer = g.Save();
        using (var clip = Round(s, rad)) g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
        g.TranslateTransform(s.X, s.Y);
        int sw = (int)s.Width, sh = (int)s.Height;
        if (starting && job == null && pages.Count == 0) RenderSplash(g, sw, sh);
        else RenderScreen(g, sw, sh, th, t);
        g.Restore(outer);
        // One soft reflection across the glass.
        using (var clip = Round(s, rad)) {
            var kept = g.Save();
            g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
            using (var b = new SolidBrush(Color.FromArgb(9, 255, 255, 255))) g.FillPolygon(b, new[] { new PointF(s.X, s.Y), new PointF(s.X + s.Width * 0.55f, s.Y), new PointF(s.X, s.Y + s.Height * 0.42f) });
            g.Restore(kept);
        }
    }

    void Shell(Graphics g, int w, int h, Theme th, float t) {
        // A moulded case in the game's colour. The game's scene lives on the screen, as its wallpaper.
        float corner = CornerRadius(w), edge = Math.Max(3f, w * 0.008f);
        var body = new RectangleF(0, 0, w, h);
        using (var path = Round(body, corner)) using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(body, Mix(th.Main, Color.White, 0.1f), Mix(th.Main, th.Deep, 0.75f), 78f)) g.FillPath(b, path);
        var lip = new RectangleF(edge, edge, w - 2 * edge, h - 2 * edge);
        using (var path = Round(lip, corner - edge)) using (var pen = new Pen(Fade(Color.White, 70), Math.Max(1.5f, w * 0.003f))) g.DrawPath(pen, path);
        using (var path = Round(new RectangleF(1, 1, w - 2, h - 2), corner)) using (var pen = new Pen(Mix(th.Deep, Color.Black, 0.45f), edge)) g.DrawPath(pen, path);
        // The fold of the hinge, and its knuckles at the left edge.
        float fold = h * 0.1f;
        using (var pen = new Pen(Fade(Color.Black, 70), Math.Max(1.5f, h * 0.002f))) g.DrawLine(pen, edge, fold, w - edge, fold);
        using (var pen = new Pen(Fade(Color.White, 60), Math.Max(1f, h * 0.0012f))) g.DrawLine(pen, edge, fold + Math.Max(2f, h * 0.002f), w - edge, fold + Math.Max(2f, h * 0.002f));
        float knuckle = h * 0.032f, kw = w * 0.012f;
        foreach (float ky0 in new[] { fold - knuckle * 0.5f }) Card(g, new RectangleF(0, ky0, kw, knuckle), kw * 0.3f, Mix(th.Deep, Color.White, 0.2f), Mix(th.Deep, Color.Black, 0.35f), Color.Empty, 0);
        // The top: big lens, status lights, wordmark.
        float lensR = h * 0.038f, topY = h * 0.056f;
        Lens(g, w * 0.06f + lensR * 1.14f, topY, lensR, t, true);
        var native = g.MeasureString("NATIVE", markFont);
        float wx = w * 0.06f + lensR * 2.9f + lensR * 2.2f, wy = topY - native.Height * 0.1f;
        Write(g, "NATIVE", markFont, th.Ink, new RectangleF(wx, wy, native.Width + 8, native.Height), StringAlignment.Near, StringAlignment.Near, false);
        Write(g, "MONS", markLight, th.Ink, new RectangleF(wx + native.Width * 0.92f, wy, w * 0.3f, native.Height), StringAlignment.Near, StringAlignment.Near, false);
        // Close and minimise, as two small hardware keys.
        float key = h * 0.022f, ky = h * 0.02f, kx = w - w * 0.05f - key;
        HardKey(g, new RectangleF(kx, ky, key, key), "X", "");
        HardKey(g, new RectangleF(kx - key * 1.5f, ky, key, key), "M", "");
        // The bottom: D-pad, speaker slots, B and A.
        float cy = h * 0.888f, d = h * 0.05f, cx = w * 0.2f;
        using (var b = new SolidBrush(Fade(Color.Black, 70))) g.FillEllipse(b, cx - d * 1.75f, cy - d * 1.75f, d * 3.5f, d * 3.5f);
        float arm = d * 0.62f;
        Pad(g, new RectangleF(cx - arm, cy - d * 1.5f, arm * 2, d * 3f), new RectangleF(cx - d * 1.5f, cy - arm, d * 3f, arm * 2));
        DpadHit(new RectangleF(cx - arm, cy - d * 1.5f, arm * 2, d), "U", g, cx, cy - d, 0);
        DpadHit(new RectangleF(cx - arm, cy + d * 0.5f, arm * 2, d), "D", g, cx, cy + d, 180);
        DpadHit(new RectangleF(cx - d * 1.5f, cy - arm, d, arm * 2), "L", g, cx - d, cy, 270);
        DpadHit(new RectangleF(cx + d * 0.5f, cy - arm, d, arm * 2), "R", g, cx + d, cy, 90);
        // The speaker: rows of drilled holes.
        float hole = Math.Max(2f, h * 0.0034f), holeGap = hole * 2.3f;
        using (var dark = new SolidBrush(Mix(th.Deep, Color.Black, 0.6f))) using (var light = new SolidBrush(Fade(Color.White, 60)))
            for (int row = 0; row < 5; row++) for (int col = 0; col < 7; col++) {
                float hx = w * 0.42f + col * holeGap + (row % 2) * holeGap / 2, hy = cy - holeGap * 2 + row * holeGap;
                g.FillEllipse(light, hx, hy + 1, hole, hole);
                g.FillEllipse(dark, hx, hy, hole, hole);
            }
        float rb = h * 0.038f;
        RoundKey(g, w * 0.86f, cy - rb * 0.75f, rb, "A", "Select", Color.FromArgb(108, 196, 74), th);
        RoundKey(g, w * 0.70f, cy + rb * 0.55f, rb, "B", "Back", Color.FromArgb(228, 76, 70), th);
    }
    static float CornerRadius(int w) { return w * 0.075f; }
    void Pad(Graphics g, RectangleF a, RectangleF b) {
        float r = a.Width * 0.18f;
        using (var br = new SolidBrush(Line)) { using (var p = Round(a, r)) g.FillPath(br, p); using (var p = Round(b, r)) g.FillPath(br, p); }
    }
    void DpadHit(RectangleF area, string key, Graphics g, float cx, float cy, float angle) {
        promptHits.Add(new KeyValuePair<RectangleF, string>(area, key));
        float s = area.Width < area.Height ? area.Width * 0.22f : area.Height * 0.22f;
        var st = g.Save();
        g.TranslateTransform(cx, cy); g.RotateTransform(angle);
        using (var br = new SolidBrush(hoverPrompt == key ? Cream : Fade(Cream, 130))) g.FillPolygon(br, new[] { new PointF(0, -s), new PointF(s, s * 0.6f), new PointF(-s, s * 0.6f) });
        g.Restore(st);
    }
    void RoundKey(Graphics g, float cx, float cy, float r, string key, string label, Color c, Theme th) {
        var area = new RectangleF(cx - r, cy - r, r * 2, r * 2);
        promptHits.Add(new KeyValuePair<RectangleF, string>(area, key));
        using (var b = new SolidBrush(Line)) g.FillEllipse(b, cx - r * 1.1f, cy - r * 1.06f, r * 2.2f, r * 2.26f);
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(area, Mix(c, Color.White, hoverPrompt == key ? 0.45f : 0.2f), Mix(c, Color.Black, 0.18f), 90f)) g.FillEllipse(b, area);
        Write(g, key, itemBold, Color.FromArgb(16, 18, 24), area, StringAlignment.Center, StringAlignment.Center, false);
    }
    void HardKey(Graphics g, RectangleF r, string key, string glyph) {
        promptHits.Add(new KeyValuePair<RectangleF, string>(r, key));
        Card(g, r, r.Height * 0.3f, hoverPrompt == key ? Cream : Mix(Line, Color.White, 0.2f), hoverPrompt == key ? Cream : Line, Color.Empty, 0);
        using (var f = new Font("Segoe MDL2 Assets", r.Height * 0.46f, GraphicsUnit.Pixel)) Write(g, glyph, f, hoverPrompt == key ? Line : Cream, r, StringAlignment.Center, StringAlignment.Center, false);
    }

    // The screen is a phone's: status bar, home screen of apps, lists that fill the screen, and a home indicator.
    static readonly Color PhoneBg = Color.FromArgb(242, 242, 247), PhoneRow = Color.White, PhoneLine = Color.FromArgb(222, 222, 228), PhoneText = Color.FromArgb(28, 28, 30), PhoneSub = Color.FromArgb(108, 108, 114);
    // The game's colour, dark enough to read on white.
    static Color Accent(Theme th) {
        var c = th.Main;
        return 0.299 * c.R + 0.587 * c.G + 0.114 * c.B > 150 ? Mix(c, Color.Black, 0.42f) : c;
    }
    void RenderScreen(Graphics g, int w, int h, Theme th, float t) {
        bool home = job == null && pages.Count > 0 && pages.Peek().Main;
        var all = new RectangleF(0, 0, w, h);
        if (home) {
            var town = GameArt.Town(th.GameId);
            if (town != null) GameArt.Draw(g, town, all); else VersionFill(g, all, 0, th, t);
            using (var b = new SolidBrush(Color.FromArgb(town != null ? 30 : 80, 0, 0, 0))) g.FillRectangle(b, all);
        } else using (var b = new SolidBrush(PhoneBg)) g.FillRectangle(b, all);
        float m = w * 0.045f, top = h * 0.055f, bottom = h * 0.965f;
        string key = job != null ? "job" : pages.Count + "|" + (pages.Count > 0 ? pages.Peek().Title : "");
        if (key != screenKey) {
            int depth = job != null ? pages.Count + 1 : pages.Count;
            screenDir = depth >= screenDepth ? 1 : -1;
            screenKey = key; screenDepth = depth; screenAt = t;
        }
        float ease = Ease((t - screenAt) / 0.18f);
        var content = g.Save();
        g.TranslateTransform(screenDir * (1 - ease) * w * 0.04f, 0);
        if (job != null) RenderJob(g, w, h, m, top, bottom, t, th);
        else if (pages.Count > 0) {
            var p = pages.Peek();
            if (p.Sel != selIndex) { selIndex = p.Sel; selAt = t; }
            selEase = Ease((t - selAt) / 0.12f);
            if (p.Main) RenderMain(g, p, w, h, m, top, bottom, t, th); else RenderPage(g, p, w, h, m, top, bottom, t, th);
        }
        g.Restore(content);
        StatusBar(g, w, h, home ? Color.White : PhoneText);
        float barW = w * 0.34f, barH = Math.Max(3f, h * 0.0065f);
        using (var path = Round(new RectangleF((w - barW) / 2, h - barH * 2.4f, barW, barH), barH / 2)) using (var b = new SolidBrush(home ? Color.White : PhoneText)) g.FillPath(b, path);
    }
    void StatusBar(Graphics g, int w, int h, Color ink) {
        float bar = h * 0.05f, x = w * 0.06f;
        Write(g, DateTime.Now.ToString("h:mm"), smallBold, ink, new RectangleF(x, 0, w * 0.4f, bar), StringAlignment.Near, StringAlignment.Center, false);
        // The PC's real battery, on a PC that has one.
        var power = SystemInformation.PowerStatus;
        if ((power.BatteryChargeStatus & BatteryChargeStatus.NoSystemBattery) != 0) return;
        float bw = w * 0.055f, bh = bw * 0.5f, bx = w - x - bw, by = (bar - bh) / 2;
        using (var path = Round(new RectangleF(bx, by, bw, bh), bh * 0.28f)) using (var pen = new Pen(Fade(ink, 150), 1)) g.DrawPath(pen, path);
        float fill = (bw - 4) * Math.Max(0.05f, Math.Min(1f, power.BatteryLifePercent));
        using (var path = Round(new RectangleF(bx + 2, by + 2, fill, bh - 4), (bh - 4) * 0.25f)) using (var b = new SolidBrush(ink)) g.FillPath(b, path);
    }
    static readonly char[] AppIcons = { ' ', '\uE7FC', '\uE8F1', '\uE74C', '\uE8B7', '\uE8AB', '\uE713', '\uE7E8' };
    static readonly string[] AppNames = { "", "Games", "Collection", "Mods", "ROMs", "Move", "Settings", "Exit" };
    static readonly Color[] AppColors = { Color.Empty, Color.FromArgb(232, 62, 58), Color.FromArgb(240, 142, 30), Color.FromArgb(124, 84, 214), Color.FromArgb(40, 124, 232), Color.FromArgb(24, 164, 150), Color.FromArgb(120, 124, 134), Color.FromArgb(58, 60, 68) };
    void RenderMain(Graphics g, Page p, int w, int h, float m, float top, float bottom, float t, Theme th) {
        // The Play widget: the only place a game's intro plays.
        var widget = new RectangleF(m, top, w - 2 * m, (bottom - top) * 0.31f);
        float rad = w * 0.06f;
        if (p.Items.Count > 0) hits.Add(new KeyValuePair<RectangleF, int>(widget, 0));
        var intro = current != null ? GameArt.Frame(current.Id, t) : null;
        using (var shadow = Round(new RectangleF(widget.X, widget.Y + h * 0.006f, widget.Width, widget.Height), rad)) using (var b = new SolidBrush(Color.FromArgb(70, 0, 0, 0))) g.FillPath(b, shadow);
        using (var clip = Round(widget, rad)) {
            var st = g.Save();
            g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
            if (intro != null) GameArt.Draw(g, intro, widget);
            else using (var b = new SolidBrush(th.Deep)) g.FillRectangle(b, widget);
            var low = new RectangleF(widget.X, widget.Y + widget.Height * 0.45f, widget.Width, widget.Height * 0.55f);
            using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(new RectangleF(low.X, low.Y - 1, low.Width, low.Height + 2), Color.FromArgb(0, 0, 0, 0), Color.FromArgb(205, 0, 0, 0), 90f)) g.FillRectangle(b, low);
            g.Restore(st);
        }
        if (p.Sel == 0) using (var ring = Round(widget, rad)) using (var pen = new Pen(Color.White, Math.Max(3f, w * 0.009f))) g.DrawPath(pen, ring);
        float pad = widget.Width * 0.05f, pillH = widget.Height * 0.23f, pillW = pillH * 2.6f;
        var pill = new RectangleF(widget.Right - pad - pillW, widget.Bottom - pad - pillH, pillW, pillH);
        bool on = p.Items.Count > 0 && p.Items[0].On;
        using (var path = Round(pill, pillH / 2)) using (var b = new SolidBrush(on ? Color.White : Color.FromArgb(160, 255, 255, 255))) g.FillPath(b, path);
        var pink = on ? Accent(th) : PhoneSub;
        float tri = pillH * 0.19f, tx = pill.X + pillW * 0.2f, ty = pill.Y + pillH / 2;
        using (var b = new SolidBrush(pink)) g.FillPolygon(b, new[] { new PointF(tx, ty - tri), new PointF(tx + tri * 1.6f, ty), new PointF(tx, ty + tri) });
        Write(g, "Play", itemBold, pink, new RectangleF(tx + tri * 2.1f, pill.Y, pillW * 0.7f, pillH), StringAlignment.Near, StringAlignment.Center, false);
        float textW = pill.X - widget.X - pad * 1.4f, subH = smallFont.Height * 1.25f;
        string name = current != null ? current.Title.Replace("Pokemon ", "") : "No games yet";
        string sub = current != null ? GameEntry.EngineShort(current.Engine()) : "Add a ROM folder to start";
        Write(g, sub, smallFont, Color.FromArgb(225, 255, 255, 255), new RectangleF(widget.X + pad, widget.Bottom - pad - subH, textW, subH), StringAlignment.Near, StringAlignment.Far, false);
        var nameArea = new SizeF(textW, titleFont.Height * 1.25f);
        var nf = FitFont(g, name, nameArea, titleFont, 0.65f, false);
        Write(g, name, nf, Color.White, new RectangleF(widget.X + pad, widget.Bottom - pad - subH - nameArea.Height, textW, nameArea.Height), StringAlignment.Near, StringAlignment.Far, false);

        // The apps.
        float cell = (w - 2 * m) / TileCols, icon = cell * 0.68f, labelH = smallFont.Height * 1.35f, rowGap = h * 0.018f;
        int appRows = (Math.Min(p.Items.Count, AppNames.Length) - 1 + TileCols - 1) / TileCols;
        // At the bottom, where a thumb would reach, leaving the wallpaper's town in view above them.
        float gy = bottom - h * 0.012f - appRows * (icon + labelH) - (appRows - 1) * rowGap;
        for (int i = 1; i < p.Items.Count && i < AppNames.Length; i++) {
            var it = p.Items[i];
            int k = i - 1;
            float cx = m + (k % TileCols) * cell + cell / 2, cy = gy + (k / TileCols) * (icon + labelH + rowGap);
            bool sel = i == p.Sel;
            float size = sel ? icon * (1 + 0.07f * selEase) : icon;
            var r = new RectangleF(cx - size / 2, cy + (icon - size) / 2, size, size);
            hits.Add(new KeyValuePair<RectangleF, int>(new RectangleF(cx - cell / 2, cy, cell, icon + labelH), i));
            using (var shadow = Round(new RectangleF(r.X, r.Y + h * 0.004f, r.Width, r.Height), size * 0.24f)) using (var b = new SolidBrush(Color.FromArgb(70, 0, 0, 0))) g.FillPath(b, shadow);
            Color c = it.On ? AppColors[i] : Mix(AppColors[i], Color.Gray, 0.7f);
            Card(g, r, size * 0.24f, Mix(c, Color.White, 0.14f), Mix(c, Color.Black, 0.1f), Color.Empty, 0);
            if (sel) using (var ring = Round(new RectangleF(r.X - 3, r.Y - 3, r.Width + 6, r.Height + 6), size * 0.27f)) using (var pen = new Pen(Color.White, Math.Max(2.5f, w * 0.007f))) g.DrawPath(pen, ring);
            Write(g, AppIcons[i].ToString(), iconFont, it.On ? Color.White : Color.FromArgb(190, 255, 255, 255), r, StringAlignment.Center, StringAlignment.Center, false);
            var label = new RectangleF(cx - cell / 2, cy + icon + labelH * 0.1f, cell, labelH);
            float labelW = g.MeasureString(AppNames[i], smallBold).Width + smallBold.Height * 0.5f;
            using (var path = Round(new RectangleF(cx - labelW / 2, label.Y, labelW, label.Height), label.Height / 2)) using (var b = new SolidBrush(Color.FromArgb(150, 0, 0, 0))) g.FillPath(b, path);
            Write(g, AppNames[i], smallBold, Color.White, label, StringAlignment.Center, StringAlignment.Center, false);
        }
    }

    readonly Dictionary<string, Font> fitFonts = new Dictionary<string, Font>();
    Font FitFont(Graphics g, string s, SizeF area, Font max, float minScale, bool wrap) {
        if (string.IsNullOrEmpty(s)) return max;
        Font f = max;
        using (var sf = new StringFormat { FormatFlags = wrap ? StringFormatFlags.LineLimit : StringFormatFlags.NoWrap, Trimming = StringTrimming.EllipsisCharacter }) {
            for (int tenths = (int)(max.Size * 10); tenths >= (int)(max.Size * 10 * minScale); tenths -= 5) {
                string key = max.Name + "|" + max.Style + "|" + tenths;
                if (!fitFonts.TryGetValue(key, out f)) fitFonts[key] = f = new Font(max.FontFamily, tenths / 10f, max.Style, GraphicsUnit.Pixel);
                int chars, lines;
                g.MeasureString(s, f, area, sf, out chars, out lines);
                if (chars >= s.Length) break;
            }
        }
        return f;
    }
    // A white grouped panel, as phone settings use.
    static void Group(Graphics g, RectangleF r, float rad) {
        using (var path = Round(r, rad)) using (var b = new SolidBrush(PhoneRow)) g.FillPath(b, path);
    }
    void RenderPage(Graphics g, Page p, int w, int h, float m, float top, float bottom, float t, Theme th) {
        var accent = Accent(th);
        // Back, like a phone's: it does what B does.
        var back = new RectangleF(m * 0.6f, top - h * 0.004f, w * 0.3f, smallBold.Height * 2f);
        promptHits.Add(new KeyValuePair<RectangleF, string>(new RectangleF(back.X + screenX, back.Y + screenY, back.Width, back.Height), "B"));
        float ch = back.Height * 0.2f, cmid = back.Y + back.Height / 2, cx0 = back.X + m * 0.4f;
        using (var pen = new Pen(accent, Math.Max(2f, h * 0.0035f))) { pen.StartCap = pen.EndCap = System.Drawing.Drawing2D.LineCap.Round; g.DrawLines(pen, new[] { new PointF(cx0 + ch * 0.8f, cmid - ch), new PointF(cx0, cmid), new PointF(cx0 + ch * 0.8f, cmid + ch) }); }
        Write(g, "Back", itemFont, accent, new RectangleF(cx0 + ch * 1.4f, back.Y, back.Width, back.Height), StringAlignment.Near, StringAlignment.Center, false);
        float y = back.Bottom;
        var titleArea = new SizeF(w - 2 * m, titleFont.Height * 1.3f);
        var tf = FitFont(g, p.Title, titleArea, titleFont, 0.6f, false);
        Write(g, p.Title, tf, PhoneText, new RectangleF(m, y, titleArea.Width, titleArea.Height), StringAlignment.Near, StringAlignment.Center, false);
        y += titleArea.Height + h * 0.012f;
        float rad = w * 0.04f;
        if (p.Body != null) {
            float bodyH = p.Items.Count > 0 ? (bottom - y) * 0.6f : bottom - y;
            var br = new RectangleF(m, y, w - 2 * m, bodyH);
            Group(g, br, rad);
            float pad = h * 0.024f, bodyText = br.Height - 2 * pad;
            if (p.Mono != null) bodyText = Math.Min(bodyText, g.MeasureString(p.Body, bodyFont, (int)(br.Width - 2 * pad)).Height + smallFont.Height * 0.5f);
            // A body too long for its box scrolls, on a page with nothing else to move through.
            float whole = p.Items.Count == 0 && p.Mono == null ? g.MeasureString(p.Body, bodyFont, (int)(br.Width - 2 * pad)).Height + bodyFont.Height : 0;
            if (whole > bodyText) {
                p.BodyScroll = Math.Max(0, Math.Min(p.BodyScroll, whole - bodyText));
                var st = g.Save();
                g.SetClip(new RectangleF(br.X, br.Y + pad, br.Width, bodyText), System.Drawing.Drawing2D.CombineMode.Intersect);
                Write(g, p.Body, bodyFont, PhoneText, new RectangleF(br.X + pad, br.Y + pad - p.BodyScroll, br.Width - 2 * pad, whole), StringAlignment.Near, StringAlignment.Near, true);
                g.Restore(st);
                float maxScroll = whole - bodyText, barW = Math.Max(3f, w * 0.008f), barH = bodyText * bodyText / whole;
                using (var path = Round(new RectangleF(br.Right - barW * 2, br.Y + pad + (bodyText - barH) * p.BodyScroll / maxScroll, barW, barH), barW / 2)) using (var b = new SolidBrush(Color.FromArgb(120, 120, 120, 128))) g.FillPath(b, path);
            }
            else Write(g, p.Body, bodyFont, PhoneText, new RectangleF(br.X + pad, br.Y + pad, br.Width - 2 * pad, bodyText), StringAlignment.Near, StringAlignment.Near, true);
            if (p.Mono != null) {
                var eb = new RectangleF(br.X + pad, br.Y + pad + bodyText, br.Width - 2 * pad, br.Height - 2 * pad - bodyText);
                Card(g, eb, h * 0.012f, Color.FromArgb(28, 28, 30), Color.FromArgb(28, 28, 30), Color.Empty, 0);
                Write(g, p.Mono, monoFont, Color.FromArgb(235, 235, 240), new RectangleF(eb.X + pad * 0.5f, eb.Y + pad * 0.5f, eb.Width - pad, eb.Height - pad), StringAlignment.Near, StringAlignment.Near, true);
            }
            y = br.Bottom + h * 0.016f;
        }
        if (p.Items.Count == 0) return;
        // Below the list, a note about what's selected.
        float footH = 0;
        var chosen = p.Items[Math.Min(p.Sel, p.Items.Count - 1)];
        string note = p.Body != null ? "" : chosen.Info != "" ? chosen.Info : chosen.Hint;
        float width = w - 2 * m, noteX = m + w * 0.02f, noteW = width - w * 0.04f;
        if (p.Body == null) {
            if (note != "") footH += g.MeasureString(note, smallFont, (int)noteW).Height + h * 0.006f;
            foreach (var rec in chosen.Records) footH += g.MeasureString(rec.Replace(": ", ":  "), smallFont, (int)noteW).Height + h * 0.002f;
            footH = Math.Min(footH + h * 0.012f, (bottom - y) * 0.34f);
        }
        float listBottom = bottom - footH, rowH = h * 0.082f;
        int fit = Math.Max(1, (int)((listBottom - y) / rowH));
        // The list moves only when the selection would leave it, and glides there.
        if (p.Sel < p.TopRow) p.TopRow = p.Sel;
        else if (p.Sel >= p.TopRow + fit) p.TopRow = p.Sel - fit + 1;
        p.TopRow = Math.Max(0, Math.Min(p.TopRow, p.Items.Count - fit));
        if (p.ScrollRow < 0) p.ScrollRow = p.TopRow;
        p.ScrollRow += (p.TopRow - p.ScrollRow) * 0.3f;
        if (Math.Abs(p.TopRow - p.ScrollRow) < 0.01f) p.ScrollRow = p.TopRow;
        scrolling = p.ScrollRow != p.TopRow;
        // A list that doesn't all fit runs to its edge and is cut off there, which shows there's more.
        bool overflow = p.Items.Count > fit;
        float groupH = overflow ? listBottom - y : p.Items.Count * rowH;
        var group = new RectangleF(m, y, width, groupH);
        Group(g, group, rad);
        using (var clip = Round(group, rad)) {
            var kept = g.Save();
            g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
            for (int i = 0; i < p.Items.Count; i++) {
                var r = new RectangleF(m, y + (i - p.ScrollRow) * rowH, width, rowH);
                if (!r.IntersectsWith(group)) continue;
                ListRow(g, p.Items[i], i == p.Sel, r, i < p.Items.Count - 1, h, accent, th);
                hits.Add(new KeyValuePair<RectangleF, int>(RectangleF.Intersect(r, group), i));
            }
            g.Restore(kept);
        }
        if (overflow) {
            float barW = Math.Max(3f, w * 0.008f), barH = group.Height * fit / p.Items.Count;
            float barY = group.Y + (group.Height - barH) * p.ScrollRow / (p.Items.Count - fit);
            using (var path = Round(new RectangleF(group.Right - barW * 1.6f, barY, barW, barH), barW / 2)) using (var b = new SolidBrush(Color.FromArgb(120, 120, 120, 128))) g.FillPath(b, path);
        }
        if (p.Body != null) return;
        float fy = listBottom + h * 0.01f;
        if (note != "") {
            float nh = Math.Min(g.MeasureString(note, smallFont, (int)noteW).Height + 2, bottom - fy);
            Write(g, note, smallFont, PhoneSub, new RectangleF(noteX, fy, noteW, nh), StringAlignment.Near, StringAlignment.Near, true);
            fy += nh + h * 0.006f;
        }
        foreach (var rec in chosen.Records) {
            float rh = g.MeasureString(rec.Replace(": ", ":  "), smallFont, (int)noteW).Height;
            if (fy + rh > bottom) break;
            Write(g, rec.Replace(": ", ":  "), smallFont, PhoneSub, new RectangleF(noteX, fy, noteW, rh + 1), StringAlignment.Near, StringAlignment.Near, true);
            fy += rh + h * 0.002f;
        }
    }
    static readonly Color[] RowColors = { Color.FromArgb(232, 62, 58), Color.FromArgb(240, 142, 30), Color.FromArgb(46, 164, 79), Color.FromArgb(40, 124, 232), Color.FromArgb(124, 84, 214), Color.FromArgb(24, 164, 150), Color.FromArgb(226, 72, 140), Color.FromArgb(92, 104, 124) };
    void ListRow(Graphics g, Item it, bool sel, RectangleF r, bool line, int h, Color accent, Theme th) {
        if (sel) using (var b = new SolidBrush(Fade(accent, 38))) g.FillRectangle(b, r);
        float icon = r.Height * 0.62f, pad = r.Height * 0.2f, ix = r.X + pad, iy = r.Y + (r.Height - icon) / 2;
        var ib = new RectangleF(ix, iy, icon, icon);
        var pic = it.Sprite != null ? Sprites.Get(it.Sprite) : null;
        if (pic != null) DrawSprite(g, pic, new RectangleF(ib.X - icon * 0.15f, ib.Y - icon * 0.15f, icon * 1.3f, icon * 1.3f));
        else if (it.Theme != null) Cart(g, ib.X + icon * 0.1f, ib.Y, icon, it.Theme);
        else {
            Color tint = it.On ? RowColors[it.Icon % RowColors.Length] : Text3;
            Card(g, ib, icon * 0.24f, Mix(tint, Color.White, 0.12f), tint, Color.Empty, 0);
            Write(g, it.Icon.ToString(), iconSmall, Color.White, ib, StringAlignment.Center, StringAlignment.Center, false);
        }
        float tx = ib.Right + pad, right = r.Right - pad;
        string title = it.Text.Replace("Pokemon ", ""), value = null;
        int colon = title.IndexOf(": ");
        if (colon > 0) { value = title.Substring(colon + 2); title = title.Substring(0, colon); }
        Color ink = it.On ? PhoneText : Text3;
        if (value == null && it.On) {
            float c = r.Height * 0.1f, cy = r.Y + r.Height / 2, cx = right - c * 0.3f;
            using (var pen = new Pen(Color.FromArgb(190, 190, 196), Math.Max(2f, h * 0.0032f))) { pen.StartCap = pen.EndCap = System.Drawing.Drawing2D.LineCap.Round; g.DrawLines(pen, new[] { new PointF(cx - c * 0.7f, cy - c), new PointF(cx, cy), new PointF(cx - c * 0.7f, cy + c) }); }
            right -= c * 1.6f;
        }
        float valueW = 0;
        if (value != null) {
            valueW = Math.Min(g.MeasureString(value, itemFont).Width + 4, (right - tx) * 0.55f);
            Write(g, value, itemFont, sel ? accent : PhoneSub, new RectangleF(right - valueW, r.Y, valueW, r.Height), StringAlignment.Far, StringAlignment.Center, false);
        }
        float tw = right - tx - valueW;
        string sub = it.TileHint != "" ? it.TileHint : (!it.On ? it.Hint : "");
        if (sub != "") {
            float th1 = itemFont.Height * 1.2f, th2 = smallFont.Height * 1.25f, ty = r.Y + (r.Height - th1 - th2) / 2;
            Write(g, title, itemFont, ink, new RectangleF(tx, ty, tw, th1), StringAlignment.Near, StringAlignment.Center, false);
            Write(g, sub, smallFont, PhoneSub, new RectangleF(tx, ty + th1, tw, th2), StringAlignment.Near, StringAlignment.Center, false);
        } else Write(g, title, itemFont, ink, new RectangleF(tx, r.Y, tw, r.Height), StringAlignment.Near, StringAlignment.Center, false);
        if (line) using (var pen = new Pen(PhoneLine, 1)) g.DrawLine(pen, tx, r.Bottom - 1, r.Right, r.Bottom - 1);
    }

    // The start splash: the device powering on, its lens above the wordmark, every version's cartridge rolling by.
    void RenderSplash(Graphics g, int w, int h) {
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
        float t = (float)clock.Elapsed.TotalSeconds, since = (float)(DateTime.Now - startSince).TotalSeconds;
        var all = new RectangleF(0, 0, w, h);
        using (var b = new SolidBrush(Color.Black)) g.FillRectangle(b, all);
        // A phone starting up: black glass, the mark, and one quiet line of progress.
        float cx = w / 2f, cy = h * 0.42f, rise = (1 - Ease(since / 0.6f)) * h * 0.012f;
        Lens(g, cx, cy - h * 0.1f + rise, h * 0.052f, t, false);
        float size = h * 0.062f;
        using (var bold = new Font("Segoe UI Semibold", size, GraphicsUnit.Pixel)) using (var light = new Font("Segoe UI Light", size, GraphicsUnit.Pixel)) {
            var a1 = g.MeasureString("Native", bold); var a2 = g.MeasureString("Mons", light);
            float nudge = size * 0.28f, markW = a1.Width + a2.Width - nudge, mx = cx - markW / 2, my = cy + rise;
            Write(g, "Native", bold, Color.White, new RectangleF(mx, my, a1.Width + 8, a1.Height), StringAlignment.Near, StringAlignment.Near, false);
            Write(g, "Mons", light, Color.FromArgb(200, 255, 255, 255), new RectangleF(mx + a1.Width - nudge, my, a2.Width + 8, a2.Height), StringAlignment.Near, StringAlignment.Near, false);
        }
        var track = new RectangleF(w * 0.32f, cy + h * 0.115f, w * 0.36f, Math.Max(3f, h * 0.0048f));
        using (var path = Round(track, track.Height / 2)) using (var b = new SolidBrush(Color.FromArgb(60, 255, 255, 255))) g.FillPath(b, path);
        float run = track.Width * 0.3f, at = (t * 0.55f % 1f) * (track.Width + run) - run;
        var clip = g.Save();
        using (var path = Round(track, track.Height / 2)) g.SetClip(path, System.Drawing.Drawing2D.CombineMode.Intersect);
        using (var b = new SolidBrush(Color.White)) g.FillRectangle(b, track.X + at, track.Y, run, track.Height);
        g.Restore(clip);
        Write(g, startStage ?? "", smallFont, Color.FromArgb(170, 255, 255, 255), new RectangleF(0, track.Bottom + h * 0.018f, w, smallFont.Height * 1.4f), StringAlignment.Center, StringAlignment.Near, false);
        // Gen1Recomp's LICENSE.MD, term 1: its credit must show visibly in any launcher. Here on every start, and on the Credits page.
        Write(g, Gen1Credit, smallFont, Color.FromArgb(150, 255, 255, 255), new RectangleF(w * 0.07f, h - smallFont.Height * 5.6f, w * 0.86f, smallFont.Height * 5f), StringAlignment.Center, StringAlignment.Near, true);
        float fade = Ease(since / 0.45f);
        if (fade < 1) using (var veil = new SolidBrush(Color.FromArgb((int)(255 * (1 - fade)), 8, 10, 14))) g.FillRectangle(veil, all);
    }
    Bitmap splashShot; float outroAt = -10;
    void Outro(Graphics g, int w, int h) {
        if (splashShot == null) return;
        float k = Ease(((float)clock.Elapsed.TotalSeconds - outroAt) / 0.6f);
        if (k >= 1) { splashShot.Dispose(); splashShot = null; return; }
        float s = 1;
        var dest = new RectangleF(w / 2f - w * s / 2, h / 2f - h * s / 2, w * s, h * s);
        using (var ia = new System.Drawing.Imaging.ImageAttributes()) {
            ia.SetColorMatrix(new System.Drawing.Imaging.ColorMatrix { Matrix33 = 1 - k });
            g.DrawImage(splashShot, Rectangle.Round(dest), 0, 0, splashShot.Width, splashShot.Height, GraphicsUnit.Pixel, ia);
        }
    }

    void RenderJob(Graphics g, int w, int h, float m, float top, float bottom, float t, Theme th) {
        var accent = Accent(th);
        float y = top + h * 0.1f, cartLen = w * 0.22f;
        // The game's cartridge going in, or a tool at work.
        Cart(g, (w - cartLen) / 2, y, cartLen, th);
        y += cartLen * 1.12f;
        var ta = new SizeF(w - 2 * m, titleFont.Height * 1.3f);
        var tf = FitFont(g, jobTitle, ta, titleFont, 0.6f, false);
        Write(g, jobTitle, tf, PhoneText, new RectangleF(m, y, ta.Width, ta.Height), StringAlignment.Center, StringAlignment.Center, false);
        y += ta.Height + h * 0.018f;
        var track = new RectangleF(w * 0.2f, y, w * 0.6f, Math.Max(4f, h * 0.007f));
        Card(g, track, track.Height / 2, PhoneLine, PhoneLine, Color.Empty, 0);
        if (progress >= 0) {
            Card(g, new RectangleF(track.X, track.Y, Math.Max(track.Height, track.Width * Math.Min(1f, progress)), track.Height), track.Height / 2, accent, accent, Color.Empty, 0);
        } else {
            float run = track.Width * 0.3f, at = (t * 0.6f % 1f) * (track.Width + run) - run;
            var clip = g.Save();
            using (var path = Round(track, track.Height / 2)) g.SetClip(path, System.Drawing.Drawing2D.CombineMode.Intersect);
            Card(g, new RectangleF(track.X + at, track.Y, run, track.Height), track.Height / 2, accent, accent, Color.Empty, 0);
            g.Restore(clip);
        }
        y = track.Bottom + h * 0.018f;
        var el = DateTime.Now - jobStart;
        var lines = new List<string>();
        for (int i = log.Count - 1; i >= 0 && lines.Count < 9; i--) lines.Add(log[i]);
        Write(g, (lines.Count > 0 ? lines[0] : (jobIsGame ? "Starting" : "One moment")), smallFont, PhoneSub, new RectangleF(m, y, w - 2 * m, smallFont.Height * 1.3f), StringAlignment.Center, StringAlignment.Center, false);
        y += smallFont.Height * 1.4f;
        Write(g, ((int)el.TotalMinutes) + ":" + el.Seconds.ToString("00"), smallFont, PhoneSub, new RectangleF(m, y, w - 2 * m, smallFont.Height * 1.3f), StringAlignment.Center, StringAlignment.Center, false);
        float lh = smallFont.Height * 1.2f, yy = bottom - lh * 1.5f, floorY = y + lh * 2;
        for (int i = 1; i < lines.Count && yy >= floorY; i++) {
            Write(g, lines[i], smallFont, Fade(PhoneSub, 170), new RectangleF(m, yy, w - 2 * m, lh), StringAlignment.Center, StringAlignment.Center, false);
            yy -= lh;
        }
    }
}

static class NativeMonsLauncher {
    [STAThread]
    static void Main(string[] argv) {
        Application.EnableVisualStyles();
        Config.Load();
        // --export <game id> <plain|checkpoint> <folder> [checkpoint backup]: exports without opening the window; the result goes in <folder>\export-result.txt.
        // --sync <game id> <result file> [fulldex|gen1]: shares one game's save without opening the window.
        if (argv.Length >= 3 && argv[0] == "--sync") {
            Library.Scan(LauncherForm.Gen1Cli);
            var shared = Library.Games.Find(x => x.Id == argv[1]);
            var outcome = LauncherForm.SyncSave(shared, argv.Length > 3 ? argv[3] : null);
            File.WriteAllText(argv[2], (outcome.Conflict ? "conflict" : outcome.Failed ? "kept" : outcome.Changed ? "changed" : "level") + "\n" + outcome.Note);
            return;
        }
        if (argv.Length == 4 && argv[0] == "--platinum-convert") {
            try { File.WriteAllBytes(argv[3], argv[1] == "to-port" ? PlatinumSave.ToPort(File.ReadAllBytes(argv[2])) : PlatinumSave.ToDs(File.ReadAllBytes(argv[2]))); }
            catch (Exception x) { File.WriteAllText(argv[3] + ".error.txt", x.Message); Environment.ExitCode = 2; }
            return;
        }
        if (argv.Length == 3 && argv[0] == "--extract-nds") {
            try { NdsExtract.Run(argv[1], argv[2]); } catch (Exception x) { File.WriteAllText(Path.Combine(argv[2], "extract-error.txt"), x.ToString()); Environment.ExitCode = 2; }
            return;
        }
        // For tests: --saves <game> <result file> list | new [name] | use <n> | copy <n> | delete <n> | recover
        if (argv.Length >= 4 && argv[0] == "--saves") {
            Library.Scan(LauncherForm.Gen1Cli);
            var saved = Library.Games.Find(x => x.Id == argv[1]);
            string verb = argv[3], arg = argv.Length > 4 ? argv[4] : null, outcome = null;
            if (verb == "new") outcome = LauncherForm.NewSave(saved, arg);
            else if (verb == "use") outcome = LauncherForm.SwitchSave(saved, arg);
            else if (verb == "copy") outcome = LauncherForm.DuplicateSave(saved, arg);
            else if (verb == "delete") outcome = LauncherForm.DeleteSave(saved, arg);
            else if (verb == "adopt") outcome = LauncherForm.AdoptSlot(saved, arg, argv.Length > 5 ? argv[5] : null);
            else if (verb == "recover") LauncherForm.RecoverSwitches();
            else if (verb == "art") outcome = GameArt.Capture(saved);
            else if (verb == "town") outcome = GameArt.CaptureTown(saved);
            else if (verb == "port-in") LauncherForm.PreparePort(saved);
            else if (verb == "port-out") outcome = LauncherForm.SaveFromPort(saved);
            else if (verb == "caught") { var c = LauncherForm.CaughtInAllSaves(saved); outcome = null; File.WriteAllText(argv[2] + ".caught", c == null ? "none" : string.Join(",", c.OrderBy(x => x))); }
            File.WriteAllText(argv[2], (outcome ?? "ok") + "\nactive=" + SaveSets.Active(saved) + "\nall=" + string.Join(",", SaveSets.All(saved)));
            Environment.ExitCode = outcome == null ? 0 : 2;
            return;
        }
        if (argv.Length >= 4 && argv[0] == "--export") {
            Library.Scan(LauncherForm.Gen1Cli);
            var game = Library.Games.Find(x => x.Id == argv[1]);
            bool done = false;
            string result = game == null || !game.Playable ? "That game isn't available." : LauncherForm.ExportSave(game, argv[2] == "checkpoint", argv[3], argv.Length > 4 ? argv[4] : null, out done);
            Directory.CreateDirectory(argv[3]);
            File.WriteAllText(Path.Combine(argv[3], "export-result.txt"), (done ? "ok\n" : "failed\n") + result);
            Environment.ExitCode = done ? 0 : 2;
            return;
        }
        if (argv.Length >= 2 && argv[0] == "--snapshot") {
            var form = new LauncherForm();
            int sw = 600, sh = 1000;
            if (argv.Length > 3) { var wh = argv[3].Split('x'); sw = int.Parse(wh[0]); sh = int.Parse(wh[1]); }
            form.Snapshot(argv[1], argv.Length > 2 ? argv[2] : "main", sw, sh);
            return;
        }
        // One launcher per folder: a second start brings the first to the front.
        string folder = Path.GetFullPath(Config.Here).TrimEnd('\\').ToLowerInvariant().Replace('\\', '/');
        bool first;
        using (var instance = new System.Threading.Mutex(true, "Local\\NativeMonsLauncher-" + folder, out first)) {
            if (!first) {
                int me = Process.GetCurrentProcess().Id;
                foreach (var other in Process.GetProcessesByName("NativeMonsLauncher"))
                    if (other.Id != me && other.MainWindowHandle != IntPtr.Zero && Native.IsWindowVisible(other.MainWindowHandle)) { Native.Front(other.MainWindowHandle); break; }
                return;
            }
            int play = Array.IndexOf(argv, "--play");
            Application.Run(new LauncherForm(play >= 0 && play + 1 < argv.Length ? argv[play + 1] : null));
        }
    }
}
