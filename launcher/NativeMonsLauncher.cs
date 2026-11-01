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
    [DllImport("user32.dll")] public static extern bool ReleaseRunCapture();
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
    public string Id, Title, Console, Gen1, FulldexCode, Header, Save;
    public string Gen1Rom, FulldexRom, FoundUnsupported;
    public bool Imported;
    public bool OnGen1 { get { return Gen1 != null && (Imported || Gen1Rom != null); } }
    public bool OnFulldex { get { return FulldexCode != null && FulldexRom != null && File.Exists(Config.Fulldex); } }
    public bool Playable { get { return OnGen1 || OnFulldex; } }
    public bool IsDs { get { return Console == "Nintendo DS"; } }
    public bool HasBoth { get { return OnGen1 && OnFulldex; } }
    // The player's choice where both engines can run it; Fulldex by default (widescreen).
    public string Engine() {
        if (OnFulldex && OnGen1) return Config.Read("engine-" + Id + ".txt", "fulldex");
        return OnFulldex ? "fulldex" : "gen1";
    }
    public static string EngineName(string engine) { return engine == "fulldex" ? "Fulldex (widescreen, 60 fps)" : "Gen1Recomp (native, with mods)"; }
}

// The ROM folders and game list. Reads the game code from each ROM, and hashes only what Gen1Recomp needs.
static class Library {
    public static List<GameEntry> Games = new List<GameEntry>();
    public static string Note = "";
    public static void Scan(Func<string, string> gen1Cli) {
        var list = new List<GameEntry>();
        var json = new JavaScriptSerializer().Deserialize<List<Dictionary<string, object>>>(File.ReadAllText(Config.GamesFile));
        Func<Dictionary<string, object>, string, string> str = delegate(Dictionary<string, object> d, string k) { object v; return d.TryGetValue(k, out v) ? v as string : null; };
        foreach (var d in json) {
            var e = new GameEntry { Id = str(d, "id"), Title = str(d, "title"), Console = str(d, "console"), Gen1 = str(d, "gen1"), FulldexCode = str(d, "fulldex"), Header = str(d, "header"), Save = str(d, "save") };
            object c, dx;
            if (d.TryGetValue("dex", out dx)) e.Dex = Convert.ToInt32(dx);
            if (d.TryGetValue("colors", out c)) {
                var hex = ((System.Collections.IEnumerable)c).Cast<object>().Select(x => (string)x).ToArray();
                if (hex.Length == 3) e.Theme = Theme.Make(hex[0], hex[1], hex[2], str(d, "finish"));
            }
            list.Add(e);
        }
        Theme.Parade = list.Select(x => x.Theme).ToArray();

        // version -> accepted hashes and whether it is imported, from the engine itself.
        var accepted = new Dictionary<string, string>();
        string games = gen1Cli("games");
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
            IEnumerable<string> files;
            try { files = Directory.GetFiles(folder, "*", SearchOption.AllDirectories); } catch { continue; }
            foreach (var file in files) {
                string ext = Path.GetExtension(file).ToLowerInvariant();
                if (ext != ".nds" && ext != ".gb" && ext != ".gbc" && ext != ".gba") continue;
                try { Sort(list, file, ext, accepted); } catch { }
            }
        }
        Games = list;
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
    static void Sort(List<GameEntry> list, string file, string ext, Dictionary<string, string> accepted) {
        string header = HeaderOf(file, ext);
        var g = list.Find(x => x.Header != null && (ext == ".gb" || ext == ".gbc" ? header.StartsWith(x.Header) && x.Gen1 != null : header == x.Header));
        if (g == null) return;
        if (g.FulldexCode != null && g.FulldexRom == null) g.FulldexRom = file;
        if (g.Gen1 != null) {
            string v;
            // A clean dump beats anything else with the same header, such as a ROM hack of that game.
            if (accepted.TryGetValue(Sha1Of(file), out v) && v == g.Gen1) { g.Gen1Rom = file; if (g.FulldexCode != null) g.FulldexRom = file; }
            else if (g.Gen1Rom == null) g.FoundUnsupported = file;
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

// One menu row: greyed rows (On = false) can be selected to read why, but do nothing.
class Item {
    public string Text, Hint; public bool On = true; public Action Run;
    public string Info = ""; public string[] Records = new string[0];   // the detail panel
    public char Icon = '';
    public Theme Theme;   // a game's row: its colours
    public string Sprite; // a Pokemon's row: its picture (see Sprites)
    public Item(string text, string hint, bool on, Action run) { Text = text; Hint = hint ?? ""; On = on; Run = run; }
}
class Page {
    public string Title; public List<Item> Items = new List<Item>(); public int Sel;
    public string Body;          // a text page (notes, results) instead of a list
    public Action OnBack;        // null = pop the page
    public string Mono;          // a fixed-width block under the body
    public bool Main;
    public Theme Theme;
}

class LauncherForm : Form {
    const ushort UP = 0x0001, DOWN = 0x0002, LEFT = 0x0004, RIGHT = 0x0008, A = 0x1000, B = 0x2000;
    public const string Gen1Credit = "Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp)";
    const string Identity = "nativemons";   // Gen1Recomp's save folder, apart from any other install's saves

    int frame;
    readonly Stack<Page> pages = new Stack<Page>();
    readonly Timer tick = new Timer();
    readonly List<string> log = new List<string>();
    bool dirty;
    Process job; string jobTitle; bool jobIsGame, jobHid; DateTime jobStart; float progress = -1; Action<int> jobDone;
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
    static ProcessStartInfo Gen1Start(string args) {
        var psi = new ProcessStartInfo(Config.Love, "\"" + Config.Gen1 + "\"" + args) {
            UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = Config.Gen1,
            RedirectStandardOutput = true, RedirectStandardError = true };
        psi.EnvironmentVariables["POKEPORT_IDENTITY"] = Identity;
        return psi;
    }
    // Asks the Gen1Recomp fork something (src/nativemons/ModsCli.lua). Null when it can't run.
    static string Gen1Cli(string command) {
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
    static string RunningGames() {
        var names = new List<string>();
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
            try { Library.Scan(Gen1Cli); } catch (Exception e) { err = e.Message; }
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
        else if (Config.RomFolders().Count == 0 && !Library.Games.Exists(g => g.Playable)) pages.Push(FoldersPage());
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
        });
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
        p.Items.Add(new Item("More", "Sprites, credits, last session.", true, delegate { pages.Push(MorePage()); }) { Icon = '\uE712' });
        p.Items.Add(new Item("Exit", "Close the launcher.", true, delegate { Close(); }) { Icon = '' });
        return p;
    }
    Page MorePage() {
        var p = new Page { Title = "More" };
        p.Items.Add(new Item("Get all sprites", "", true, delegate { GetAllSprites(); }) { Icon = '\uE896', Info = "Downloads every Pokemon picture for offline play. Run it again at any time: it only fetches what's missing." });
        p.Items.Add(new Item("Credits", "", true, delegate { pages.Push(CreditsPage()); }) { Icon = '\uE946', Info = "The projects this is built from." });
        p.Items.Add(new Item("Last session", "", true, delegate { ShowText("Last session", Config.Read("last-session.txt", "No session yet."), null); }) { Icon = '\uE81C', Info = "How the last game ended." });
        return p;
    }
    // Every installed game adds the entries it has, counted once however many games share them, and every save adds what it has caught.
    void OpenCollection() {
        var caughtBy = new Dictionary<GameEntry, HashSet<int>>();
        var noSave = new HashSet<GameEntry>();
        Busy("Counting your collection", delegate {
            foreach (var g in Library.Games.Where(x => x.Playable)) {
                string why, file = SaveOf(g, out why);
                if (file == null) { noSave.Add(g); continue; }
                int code; string o = Bridge("caught \"" + file + "\"", out code);
                if (code != 0) { noSave.Add(g); continue; }
                var set = new HashSet<int>();
                foreach (var id in o.Trim().Split(',')) { int n; if (int.TryParse(id, out n)) set.Add(n); }
                caughtBy[g] = set;
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
                it.Info = has ? "Caught in this save, out of the " + g.Dex + " entries in this game." : "Not started: it counts towards the total entries, but has nothing caught yet.";
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
            string why = g.FoundUnsupported != null && !g.Playable ? "Found " + Path.GetFileName(g.FoundUnsupported) + ", but it is not a version these games accept." : "No ROM found. Add its folder under ROM folders.";
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
        return r.ToArray();
    }
    Page GamePage(GameEntry g) {
        var p = new Page { Title = g.Title, Theme = g.Theme };
        string engine = g.Engine();
        var play = new Item("Play", "Start with " + GameEntry.EngineName(engine) + ".", g.Playable, delegate { Play(g); }) { Icon = '' };
        play.Records = GameRecords(g);
        p.Items.Add(play);
        if (g.HasBoth) {
            string other = engine == "fulldex" ? "gen1" : "fulldex";
            p.Items.Add(new Item("Engine: " + (engine == "fulldex" ? "Fulldex" : "Gen1Recomp"), "Switch to " + GameEntry.EngineName(other) + ". Each engine keeps its own save.", true, delegate {
                Config.Write("engine-" + g.Id + ".txt", other); pages.Pop(); pages.Push(GamePage(g)); ReplaceMain();
            }) { Icon = '' });
        }
        if (g.OnGen1) p.Items.Add(new Item("Mods", "Mods for " + g.Title + ".", true, delegate { Busy("Reading mods", null, delegate(string e) { pages.Push(ModsPage(g)); }); }) { Icon = '' });
        p.Items.Add(new Item("Open in PKHeX", "View or edit this game's save in PKHeX.", true, delegate { OpenInPkhex(g); }) { Icon = '\uE70F' });
        if (engine != "fulldex") p.Items.Add(new Item("Bring back the edited save", "Takes the save you edited in PKHeX. Your older save slots stay.", true, delegate { BringBack(g); }) { Icon = '\uE896' });
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
        p.Items.Add(new Item("Install a mod (.zip)", "", true, delegate { InstallMod(g); }) { Icon = '', Info = "Copies a mod into the game's mods folder. Only install mods you trust: they run code in the game." });
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
            using (var d = new FolderBrowserDialog { Description = "Choose a folder with your ROMs (.gb, .gbc, .gba, .nds)" }) {
                if (d.ShowDialog(this) != DialogResult.OK) return;
                var l = Config.RomFolders();
                if (!l.Contains(d.SelectedPath)) { l.Add(d.SelectedPath); Config.SaveRomFolders(l); }
                pages.Pop();
                Rescan(delegate { pages.Push(FoldersPage()); });
            }
        }) { Icon = '', Info = "nativeMons finds games by reading each ROM, so file names don't matter. No games are included: you need your own ROMs." });
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
        return new Page { Title = "Credits", Body = Gen1Credit
            + "\n\nPokemon Fulldex by mamesj (github.com/mamesj/pokemon-fulldex), a fork of ssrecomp by ClickingMaster. It uses melonDS, and mGBA by way of Pokemon GBA Widescreen. Function names come from the pokebw2, aletheia and pokeplatinum decompilations."
            + "\n\nPokemon pictures served by pogo.hails.app (hailsDotGO), from the PokeAPI sprites collection."
            + "\n\nMoving Pokemon uses PKHeX.Core by Kaphotics and contributors (github.com/kwsch/PKHeX, GPL-3.0)."
            + "\n\nhails.nds-Patcher by Hails. Launcher design from the hails-additions branch of the Skate 3 Rust Engine (SK8-ENGINE), github.com/Hailey-Ross/rusty-trucks."
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
        Busy("Getting all sprites", delegate {
            Sprites.GetAll(delegate(int done, int total, int fresh, int failed) {
                progress = done / (float)total;
                if (done % 25 == 0 || done == total) BeginInvoke((Action)delegate { log.Add(done + " of " + total + " checked"); });
                dirty = true;
                if (done == total) summary = fresh + " downloaded, " + (total - fresh - failed) + " already here." + (failed > 0 ? "\n\n" + failed + " couldn't be fetched. Run this again when you're online: it only fetches what's missing." : "\n\nEvery picture is ready for offline play.");
            });
            return null;
        }, delegate(string e) { ShowText("All sprites", e ?? summary, null); });
    }

    // ---------- moving Pokemon (tools\mon-bridge, built on PKHeX.Core) ----------
    static string RunCapture(ProcessStartInfo psi, int seconds, out int code) {
        psi.UseShellExecute = false; psi.CreateNoWindow = true; psi.RedirectStandardOutput = true; psi.RedirectStandardError = true;
        psi.StandardOutputEncoding = Encoding.UTF8;
        using (var p = Process.Start(psi)) {
            var err = p.StandardError.ReadToEndAsync();
            var o = p.StandardOutput.ReadToEndAsync();
            if (!p.WaitForExit(seconds * 1000)) { try { p.Kill(); } catch { } code = -1; return "That took too long and was stopped."; }
            code = p.ExitCode;
            return code == 0 || o.Result.Trim() != "" ? o.Result : err.Result;
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
        if (g.Engine() == "fulldex") {
            string f = Path.Combine(Config.FulldexDir, "saves", g.Save ?? "");
            if (File.Exists(f)) return f;
            why = "No Fulldex save yet for " + g.Title + ". Play and save once first.";
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
        if (g.Engine() == "fulldex") return null;
        var psi = Gen1Start("");
        psi.EnvironmentVariables["NATIVEMONS_CLI"] = "import " + g.Gen1 + " " + file;
        int code; string o = RunCapture(psi, 60, out code);
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
        if (forced) p.Items.Add(new Item("About this move", "", false, null) { Icon = '\uE946', Info = "No real transfer exists here, so it is copied by hand. Its form, ball and origin may change, and PKHeX may flag it as not legal." });
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
            message = ok ? name + " is now in " + to.Title + "'s PC boxes." + (f.Length > 2 && f[2] == "not-legal" ? "\n\nPKHeX's legality check flags it as not legal, which matters only for online play." : "") : back;
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
            if (g.Engine() != "fulldex") ShowText("Editing an exported copy", "Gen1Recomp keeps its own save format, so PKHeX has an exported copy:\n" + file + "\n\nSave your changes there in PKHeX, then choose Bring back the edited save on this game's page.", null);
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
            PlayFulldex(g);
        }
        else if (!g.Imported) Import(g, delegate { PlayGen1(g); });
        else PlayGen1(g);
    }
    void Import(GameEntry g, Action then) {
        var psi = Gen1Start("");
        psi.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
        psi.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
        psi.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = g.Gen1Rom;
        RunJob("Preparing " + g.Title, psi, false, delegate(int code) {
            if (code != 0) { ShowLog("Could not prepare " + g.Title, "Your ROM was not changed."); return; }
            g.Imported = true;
            if (then != null) then(); else ShowText("Ready", g.Title + " is prepared.", null);
        });
    }
    void PlayGen1(GameEntry g) {
        RunJob(g.Title, Gen1Start(" --game=" + g.Gen1), true, delegate(int code) { Finished(g, code); });
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
        }
        var psi = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    void Finished(GameEntry g, int code) {
        var t = DateTime.Now - jobStart;
        string text = g.Title + " ran for " + ((int)t.TotalMinutes) + ":" + t.Seconds.ToString("00") + ".";
        if (code != 0) text += " It ended with code " + code + ".";
        string detail = code != 0 ? "\n\n" + string.Join("\n", log.Skip(Math.Max(0, log.Count - 12)).ToArray()) : "";
        Config.Write("last-session.txt", text + detail);
        if (code != 0) ShowLog("Finished, check this", text, delegate { if (playDirect != null) Close(); else pages.Pop(); });
        else if (playDirect != null) { Close(); return; }
        ReplaceMain();
    }
    void RunTool(string title, string exe, string args, string dir, Action<int> done) {
        RunJob(title, new ProcessStartInfo(exe, args) { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true }, false, done);
    }
    void RunJob(string title, ProcessStartInfo psi, bool isGame, Action<int> done) {
        log.Clear(); jobTitle = title; jobIsGame = isGame; jobHid = false; jobStart = DateTime.Now; progress = -1; jobDone = done;
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
        if (Visible && ++frame % 2 == 0 && (job != null || starting || (pages.Count > 0 && pages.Peek().Main))) Invalidate();
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
    readonly List<KeyValuePair<RectangleF, int>> hits = new List<KeyValuePair<RectangleF, int>>();
    int HitAt(Point pt) {
        var inside = new PointF(pt.X - screenX, pt.Y - screenY);
        foreach (var h in hits) if (h.Key.Contains(inside)) return h.Value;
        return -1;
    }
    // No window frame, so dragging the shell moves the device.
    protected override void OnMouseDown(MouseEventArgs e) {
        base.OnMouseDown(e);
        if (e.Button == MouseButtons.Left && PromptAt(e.Location) == null && HitAt(e.Location) < 0) { Native.ReleaseRunCapture(); Native.SendMessage(Handle, 0xA1, (IntPtr)2, IntPtr.Zero); }
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
            else if (p.Body == null) p.Sel = PageGridMove(p.Sel, a, p.Items.Count, 2);
            else if (a == "up" || a == "left") p.Sel = (p.Sel - 1 + p.Items.Count) % p.Items.Count;
            else if (a == "down" || a == "right") p.Sel = (p.Sel + 1) % p.Items.Count;
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
    const int TileCols = 2;
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
        using (var bezel = Round(new RectangleF(s.X - w * 0.014f, s.Y - w * 0.014f, s.Width + w * 0.028f, s.Height + w * 0.028f), rad * 1.3f)) using (var b = new SolidBrush(Line)) g.FillPath(b, bezel);
        var outer = g.Save();
        using (var clip = Round(s, rad)) g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
        g.TranslateTransform(s.X, s.Y);
        int sw = (int)s.Width, sh = (int)s.Height;
        if (starting && job == null && pages.Count == 0) RenderSplash(g, sw, sh);
        else RenderScreen(g, sw, sh, th, t);
        g.Restore(outer);
    }

    void Shell(Graphics g, int w, int h, Theme th, float t) {
        var body = new RectangleF(0, 0, w, h);
        VersionFill(g, body, CornerRadius(w), th, t);
        using (var path = Round(new RectangleF(1, 1, w - 2, h - 2), CornerRadius(w))) using (var pen = new Pen(Line, Math.Max(3f, w * 0.008f))) g.DrawPath(pen, path);
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
        using (var pen = new Pen(Fade(Line, 150), Math.Max(2f, h * 0.0035f))) for (int i = 0; i < 4; i++) { float sx = w * 0.46f + i * w * 0.028f; g.DrawLine(pen, sx, cy - h * 0.025f, sx, cy + h * 0.025f); }
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
        Write(g, label, smallBold, th.Ink, new RectangleF(cx - r * 1.6f, cy + r * 1.1f, r * 3.2f, smallBold.Height * 1.3f), StringAlignment.Center, StringAlignment.Near, false);
    }
    void HardKey(Graphics g, RectangleF r, string key, string glyph) {
        promptHits.Add(new KeyValuePair<RectangleF, string>(r, key));
        Card(g, r, r.Height * 0.3f, hoverPrompt == key ? Cream : Mix(Line, Color.White, 0.2f), hoverPrompt == key ? Cream : Line, Color.Empty, 0);
        using (var f = new Font("Segoe MDL2 Assets", r.Height * 0.46f, GraphicsUnit.Pixel)) Write(g, glyph, f, hoverPrompt == key ? Line : Cream, r, StringAlignment.Center, StringAlignment.Center, false);
    }

    void RenderScreen(Graphics g, int w, int h, Theme th, float t) {
        Floor(g, w, h, th);
        float m = w * 0.04f, top = h * 0.028f, bottom = h - h * 0.028f;
        string key = job != null ? "job" : pages.Count + "|" + (pages.Count > 0 ? pages.Peek().Title : "");
        if (key != screenKey) {
            int depth = job != null ? pages.Count + 1 : pages.Count;
            screenDir = depth >= screenDepth ? 1 : -1;
            screenKey = key; screenDepth = depth; screenAt = t;
        }
        float ease = Ease((t - screenAt) / 0.18f);
        var content = g.Save();
        g.TranslateTransform(screenDir * (1 - ease) * w * 0.04f, 0);
        if (job != null) { RenderJob(g, w, h, m, top, bottom, t, th); g.Restore(content); return; }
        if (pages.Count == 0) { g.Restore(content); return; }
        var p = pages.Peek();
        if (p.Sel != selIndex) { selIndex = p.Sel; selAt = t; }
        selEase = Ease((t - selAt) / 0.12f);
        if (p.Main) RenderMain(g, p, w, h, m, top, bottom, t, th); else RenderPage(g, p, w, h, m, top, bottom, t, th);
        g.Restore(content);
    }

    void RenderMain(Graphics g, Page p, int w, int h, float m, float top, float bottom, float t, Theme th) {
        // The required credit stays on the launcher's main screen (Gen1Recomp's LICENSE.MD, term 1).
        float creditH = smallFont.Height * 2.5f;
        Write(g, Gen1Credit, smallFont, FootInk, new RectangleF(m, bottom - creditH, w - 2 * m, creditH), StringAlignment.Center, StringAlignment.Far, true);
        bottom -= creditH + h * 0.012f;
        // The Play panel: the selected version's colour and finish.
        var hero = new RectangleF(m, top, w - 2 * m, (bottom - top) * 0.42f);
        float heroRad = w * 0.03f;
        using (var path = Round(new RectangleF(hero.X + 2, hero.Y + 3, hero.Width, hero.Height), heroRad)) using (var b = new SolidBrush(Fade(Color.Black, 110))) g.FillPath(b, path);
        VersionFill(g, hero, heroRad, th, t);
        using (var path = Round(hero, heroRad)) using (var pen = new Pen(Line, Math.Max(2f, w * 0.006f))) g.DrawPath(pen, path);
        float pad = hero.Width * 0.05f;
        Write(g, "NOW SELECTED", smallBold, th.Ink, new RectangleF(hero.X + pad, hero.Y + pad * 0.7f, hero.Width - 2 * pad, smallFont.Height * 1.2f), StringAlignment.Near, StringAlignment.Near, false);
        string name = current != null ? current.Title : "No games yet";
        var vr = new RectangleF(hero.X + pad, hero.Y + pad * 0.7f + smallFont.Height * 1.2f, hero.Width - 2 * pad, heroFont.Height * 1.2f);
        var vf = FitFont(g, name, vr.Size, heroFont, 0.6f, false);
        Write(g, name, vf, th.Ink, new RectangleF(vr.X, vr.Y + (heroFont.Height - vf.Height) / 2, vr.Width, vr.Height), StringAlignment.Near, StringAlignment.Near, false);
        string sub = current != null ? current.Console + " with " + GameEntry.EngineName(current.Engine()) : "Add a ROM folder to get started.";
        Write(g, sub, smallBold, th.Ink, new RectangleF(hero.X + pad, vr.Bottom, hero.Width - 2 * pad, smallFont.Height * 2.5f), StringAlignment.Near, StringAlignment.Near, true);
        if (p.Items.Count > 0) {
            bool sel = p.Sel == 0, on = p.Items[0].On;
            float bh = hero.Height * 0.3f;
            var pr = new RectangleF(hero.X + pad, hero.Bottom - bh - pad * 0.7f, hero.Width - 2 * pad, bh);
            hits.Add(new KeyValuePair<RectangleF, int>(pr, 0));
            if (sel && on) using (var glow = Round(new RectangleF(pr.X - 5, pr.Y - 5, pr.Width + 10, pr.Height + 10), pr.Height * 0.3f)) using (var gb = new SolidBrush(Fade(Color.White, 70 + (int)(45 * Math.Sin(t * 3))))) g.FillPath(gb, glow);
            Box(g, pr, pr.Height * 0.24f, th, sel, on);
            Color ink = on ? (th.OnCream) : Text3;
            var size = g.MeasureString("PLAY", playFont);
            float cx = pr.X + (pr.Width - size.Width - pr.Height * 0.5f) / 2;
            Cursor3(g, cx, pr.Y + pr.Height / 2, pr.Height * 0.2f, ink);
            Write(g, "PLAY", playFont, ink, new RectangleF(cx + pr.Height * 0.4f, pr.Y, size.Width + 8, pr.Height), StringAlignment.Near, StringAlignment.Center, false);
        }
        float gy = hero.Bottom + h * 0.018f, gap = w * 0.022f;
        int n = p.Items.Count - 1, rows = Math.Max(1, (n + TileCols - 1) / TileCols);
        float tw = (w - 2 * m - gap * (TileCols - 1)) / TileCols, th2 = (bottom - gy - gap * (rows - 1)) / rows;
        var hint = PageHintFont(g, p, 1, new RectangleF(0, 0, tw, th2), true);
        for (int i = 1; i < p.Items.Count; i++) {
            var it = p.Items[i];
            int k = i - 1; var r = new RectangleF(m + (k % TileCols) * (tw + gap), gy + (k / TileCols) * (th2 + gap), tw, th2);
            Tile(g, it, i == p.Sel, r, true, hint, th);
            hits.Add(new KeyValuePair<RectangleF, int>(r, i));
        }
    }

    void Tile(Graphics g, Item it, bool sel, RectangleF r, bool icon, Font hint, Theme th) {
        float rad = r.Height * 0.14f;
        var own = it.Theme ?? th;
        if (sel) { float grow = 2 * selEase; r = new RectangleF(r.X - grow, r.Y - grow, r.Width + 2 * grow, r.Height + 2 * grow); }
        Box(g, r, rad, own, sel, it.On);
        var ink = it.On ? Text1 : Text3;
        float pad = r.Height * 0.2f, tx = r.X + pad;
        if (sel) Cursor3(g, r.X + pad * 0.42f, r.Y + r.Height / 2, r.Height * 0.09f, own.OnCream);
        if (icon) {
            var ib = new RectangleF(r.X + pad * 1.05f, r.Y + r.Height * 0.25f, r.Height * 0.5f, r.Height * 0.5f);
            var pic = it.Sprite != null ? Sprites.Get(it.Sprite) : null;
            if (pic != null) { float grow = ib.Height * 0.35f; DrawSprite(g, pic, new RectangleF(ib.X - grow, ib.Y - grow, ib.Width + grow * 2, ib.Height + grow * 2)); }
            else if (it.Theme != null) Cart(g, ib.X, ib.Y - ib.Height * 0.02f, ib.Height, it.Theme);
            else {
                Card(g, ib, ib.Height * 0.2f, it.On ? th.Main : Text3, it.On ? th.Deep : Text3, Line, 1);
                Write(g, it.Icon.ToString(), iconSmall, it.On ? th.Ink : CreamDim, ib, StringAlignment.Center, StringAlignment.Center, false);
            }
            tx = ib.Right + pad * 0.7f;
        }
        float tw = r.Right - tx - pad * 0.6f, ty = r.Y + r.Height * 0.16f;
        if (it.Hint == "") {
            Write(g, it.Text, itemBold, ink, new RectangleF(tx, r.Y, tw, r.Height), StringAlignment.Near, StringAlignment.Center, true);
            return;
        }
        Write(g, it.Text, itemBold, ink, new RectangleF(tx, ty, tw, itemBold.Height * 1.15f), StringAlignment.Near, StringAlignment.Near, false);
        Write(g, it.Hint, hint, it.On ? Text2 : Text3, HintArea(r, icon), StringAlignment.Near, StringAlignment.Near, true);
    }
    RectangleF HintArea(RectangleF r, bool icon) {
        float pad = r.Height * 0.2f, tx = icon ? r.X + pad * 1.05f + r.Height * 0.5f + pad * 0.7f : r.X + pad;
        float hy = r.Y + r.Height * 0.16f + itemBold.Height * 1.15f;
        return new RectangleF(tx, hy, r.Right - tx - pad * 0.6f, r.Bottom - hy - pad * 0.3f);
    }
    Font PageHintFont(Graphics g, Page p, int from, RectangleF tile, bool icon) {
        var area = HintArea(tile, icon).Size;
        Font f = smallFont;
        for (int i = from; i < p.Items.Count; i++) {
            var x = FitFont(g, p.Items[i].Hint, area, smallFont, 0.7f, true);
            if (x.Size < f.Size) f = x;
        }
        return f;
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

    void RenderPage(Graphics g, Page p, int w, int h, float m, float top, float bottom, float t, Theme th) {
        Write(g, p.Title, titleFont, Cream, new RectangleF(m, top, w - 2 * m, titleFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
        float y = top + titleFont.Height * 1.5f;
        if (p.Body != null) {
            float bodyH = p.Items.Count > 0 ? (bottom - y) * 0.55f : bottom - y - h * 0.01f;
            var br = new RectangleF(m, y, w - 2 * m, bodyH);
            Box(g, br, h * 0.018f, th, false, true);
            float pad = h * 0.032f, bodyText = br.Height - 2 * pad;
            if (p.Mono != null) bodyText = Math.Min(bodyText, g.MeasureString(p.Body, bodyFont, (int)(br.Width - 2 * pad)).Height + smallFont.Height * 0.5f);
            Write(g, p.Body, bodyFont, Text1, new RectangleF(br.X + pad, br.Y + pad, br.Width - 2 * pad, bodyText), StringAlignment.Near, StringAlignment.Near, true);
            if (p.Mono != null) {
                var eb = new RectangleF(br.X + pad, br.Y + pad + bodyText, br.Width - 2 * pad, br.Height - 2 * pad - bodyText);
                Card(g, eb, h * 0.01f, Color.FromArgb(40, 20, 22), Color.FromArgb(26, 14, 16), Color.FromArgb(120, 228, 76, 70), 1);
                Write(g, p.Mono, monoFont, Color.FromArgb(236, 226, 226), new RectangleF(eb.X + pad * 0.5f, eb.Y + pad * 0.5f, eb.Width - pad, eb.Height - pad), StringAlignment.Near, StringAlignment.Near, true);
            }
            y = br.Bottom + h * 0.025f;
        }
        if (p.Items.Count == 0) return;
        if (p.Body == null) {
            const int cols = 2;
            float tgap = w * 0.022f, tw = (w - 2 * m - tgap * (cols - 1)) / cols, tileH = h * 0.1f;
            float detailH = (bottom - y) * 0.46f;
            RenderDetail(g, p.Items[p.Sel], new RectangleF(m, y, w - 2 * m, detailH), h, t, th);
            y += detailH + tgap;
            int rows = (p.Items.Count + cols - 1) / cols;
            int fitRows = Math.Max(1, (int)((bottom - y + tgap) / (tileH + tgap)));
            int firstRow = Math.Max(0, Math.Min(p.Sel / cols - fitRows / 2, rows - fitRows));
            for (int i = firstRow * cols; i < p.Items.Count && i < (firstRow + fitRows) * cols; i++) {
                int k = i - firstRow * cols;
                var r = new RectangleF(m + (k % cols) * (tw + tgap), y + (k / cols) * (tileH + tgap), tw, tileH);
                var shown = new Item(p.Items[i].Text.Replace("Pokemon ", ""), "", p.Items[i].On, null) { Icon = p.Items[i].Icon, Theme = p.Items[i].Theme, Sprite = p.Items[i].Sprite };
                Tile(g, shown, i == p.Sel, r, true, smallFont, th);
                hits.Add(new KeyValuePair<RectangleF, int>(r, i));
            }
            if (rows > fitRows)
                Write(g, (p.Sel + 1) + " / " + p.Items.Count, smallFont, FootInk, new RectangleF(m, top, w - 2 * m, titleFont.Height * 1.3f), StringAlignment.Far, StringAlignment.Center, false);
            return;
        }
        float ch = h * 0.095f, gap = h * 0.014f;
        int fit = Math.Max(1, (int)((bottom - y + gap) / (ch + gap)));
        int first = Math.Max(0, Math.Min(p.Sel - fit / 2, p.Items.Count - fit));
        var hint = PageHintFont(g, p, 0, new RectangleF(0, 0, w - 2 * m, ch), false);
        for (int i = first; i < p.Items.Count && i < first + fit; i++) {
            var r = new RectangleF(m, y, w - 2 * m, ch);
            Tile(g, p.Items[i], i == p.Sel, r, false, hint, th);
            hits.Add(new KeyValuePair<RectangleF, int>(r, i));
            y += ch + gap;
        }
    }

    // The detail panel: a banner in the highlighted version's colour over a dialog box.
    void RenderDetail(Graphics g, Item it, RectangleF r, int h, float t, Theme th) {
        float rad = h * 0.022f, pad = h * 0.04f;
        Box(g, r, rad, th, false, true);
        float bannerH = titleFont.Height * 2.3f;
        var banner = new RectangleF(r.X + 4, r.Y + 4, r.Width - 8, bannerH);
        VersionFill(g, banner, rad * 0.75f, th, t);
        using (var path = Round(banner, rad * 0.75f)) using (var pen = new Pen(Line, 2)) g.DrawPath(pen, path);
        float x = r.X + pad, cw = r.Width - 2 * pad;
        Write(g, it.Text, titleFont, th.Ink, new RectangleF(x, banner.Y, cw, bannerH), StringAlignment.Near, StringAlignment.Center, false);
        var big = it.Sprite != null ? Sprites.Get(it.Sprite) : null;
        if (big != null) { float bs = bannerH * 1.3f; DrawSprite(g, big, new RectangleF(banner.Right - bs - pad * 0.3f, banner.Y + (bannerH - bs) / 2, bs, bs)); }
        float y = banner.Bottom + h * 0.024f;
        string pill = it.On ? "READY" : "NOT AVAILABLE";
        var ps = g.MeasureString(pill, smallBold);
        var pr = new RectangleF(x, y, ps.Width + smallBold.Height * 0.9f, smallBold.Height * 1.4f);
        Color pc = it.On ? Color.FromArgb(46, 140, 58) : Color.FromArgb(196, 52, 46);
        Card(g, pr, pr.Height / 2, Fade(pc, 40), Fade(pc, 40), pc, 1);
        Write(g, pill, smallBold, pc, pr, StringAlignment.Center, StringAlignment.Center, false);
        y += pr.Height + h * 0.024f;
        string info = it.Info != "" ? it.Info : it.Hint;
        if (info != "") {
            float ih = Math.Min(g.MeasureString(info, bodyFont, (int)cw).Height + 2, r.Bottom - pad - y);
            Write(g, info, bodyFont, Text1, new RectangleF(x, y, cw, ih), StringAlignment.Near, StringAlignment.Near, true);
            y += ih + h * 0.024f;
        }
        float nameW = cw * 0.3f, dx = x + nameW + pad * 0.5f, dw = x + cw - dx, rowGap = h * 0.008f;
        using (var line = new Pen(Fade(Line, 70), 1)) {
            foreach (var rec in it.Records) {
                int cut = rec.IndexOf(": ");
                string a = cut > 0 ? rec.Substring(0, cut) : rec, b = cut > 0 ? rec.Substring(cut + 2) : "";
                float rh = Math.Max(g.MeasureString(a, smallBold, (int)nameW).Height, g.MeasureString(b, smallFont, (int)dw).Height);
                if (y + rh + rowGap > r.Bottom - pad) return;
                g.DrawLine(line, x, y, x + cw, y);
                y += rowGap;
                Write(g, a, smallBold, Text1, new RectangleF(x, y, nameW, rh), StringAlignment.Near, StringAlignment.Near, true);
                Write(g, b, smallFont, Text2, new RectangleF(dx, y, dw, rh), StringAlignment.Near, StringAlignment.Near, true);
                y += rh + rowGap;
            }
        }
    }

    // The start splash: the device powering on, its lens above the wordmark, every version's cartridge rolling by.
    void RenderSplash(Graphics g, int w, int h) {
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
        float t = (float)clock.Elapsed.TotalSeconds, since = (float)(DateTime.Now - startSince).TotalSeconds;
        Floor(g, w, h, Theme.Default);
        var all = new RectangleF(0, 0, w, h);
        float cx = w / 2f, cy = h * 0.46f;
        float rise = (1 - Ease(since / 0.6f)) * h * 0.02f;
        Lens(g, cx, cy - h * 0.25f + rise, h * 0.055f, t, false);
        var a1 = g.MeasureString("NATIVE", splashMark); var a2 = g.MeasureString("MONS", splashLight);
        float markW = a1.Width * 0.94f + a2.Width, mx = cx - markW / 2, my = cy - a1.Height * 0.95f + rise;
        Write(g, "NATIVE", splashMark, Cream, new RectangleF(mx, my, a1.Width + 8, a1.Height), StringAlignment.Near, StringAlignment.Near, false);
        Write(g, "MONS", splashLight, CreamDim, new RectangleF(mx + a1.Width * 0.94f, my, a2.Width + 8, a1.Height), StringAlignment.Near, StringAlignment.Near, false);
        float trackW = markW * 1.2f, cartLen = trackW * 0.09f;
        var track = new RectangleF(cx - trackW / 2, cy + h * 0.095f + rise, trackW, Math.Max(3f, h * 0.005f));
        Card(g, track, track.Height / 2, Line, Line, Color.Empty, 0);
        var row = Theme.Parade;
        var clip = g.Save(); g.SetClip(new RectangleF(track.X, 0, track.Width, h));
        float gapX = cartLen * 1.25f, span = gapX * row.Length, shift = (t * cartLen * 0.9f) % span;
        for (int i = 0; i < row.Length; i++) {
            float x = track.X + ((i * gapX + shift) % span) - cartLen;
            if (x < track.Right) Cart(g, x, track.Y - cartLen * 0.97f, cartLen, row[i]);
        }
        g.Restore(clip);
        Write(g, startStage ?? "", itemFont, FootInk, new RectangleF(0, track.Bottom + h * 0.04f, w, itemFont.Height * 1.4f), StringAlignment.Center, StringAlignment.Near, false);
        Write(g, Gen1Credit, smallFont, FootInk, new RectangleF(w * 0.05f, h - smallFont.Height * 3.4f, w * 0.9f, smallFont.Height * 2.8f), StringAlignment.Center, StringAlignment.Near, true);
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
        var card = new RectangleF(m, top, w - 2 * m, bottom - top);
        Box(g, card, h * 0.024f, th, false, true);
        float pad = card.Height * 0.07f;
        Write(g, jobIsGame ? "STARTING" : "WORKING", smallBold, th.OnCream, new RectangleF(card.X + pad, card.Y + pad, card.Width - 2 * pad, smallFont.Height * 1.2f), StringAlignment.Near, StringAlignment.Near, false);
        var el = DateTime.Now - jobStart;
        Write(g, ((int)el.TotalMinutes) + ":" + el.Seconds.ToString("00"), smallBold, Text2, new RectangleF(card.X + pad, card.Y + pad, card.Width - 2 * pad, smallFont.Height * 1.2f), StringAlignment.Far, StringAlignment.Near, false);
        Write(g, jobTitle, heroFont, Text1, new RectangleF(card.X + pad, card.Y + pad + smallFont.Height * 1.3f, card.Width - 2 * pad, heroFont.Height * 1.25f), StringAlignment.Near, StringAlignment.Near, false);
        float ty = card.Y + pad + smallFont.Height * 1.4f + heroFont.Height * 1.5f, cartLen = card.Height * 0.1f;
        var track = new RectangleF(card.X + pad, ty + cartLen, card.Width - 2 * pad, h * 0.012f);
        Card(g, track, track.Height / 2, Line, Line, Color.Empty, 0);
        if (progress >= 0) {
            var fill = new RectangleF(track.X, track.Y, Math.Max(track.Height, track.Width * Math.Min(1f, progress)), track.Height);
            VersionFill(g, fill, track.Height / 2, th, t);
            Cart(g, fill.Right - cartLen * 0.5f, track.Y - cartLen * 0.97f, cartLen, th);
        } else {
            var clip = g.Save(); g.SetClip(card);
            Cart(g, Rolling(track, cartLen, t), track.Y - cartLen * 0.97f, cartLen, th);
            g.Restore(clip);
        }
        float ly = track.Bottom + h * 0.03f;
        var lines = new List<string>();
        for (int i = log.Count - 1; i >= 0 && lines.Count < 9; i--) lines.Add(log[i]);
        Write(g, lines.Count > 0 ? lines[0] : "One moment...", itemFont, Text1, new RectangleF(card.X + pad, ly, card.Width - 2 * pad, itemFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
        ly += itemFont.Height * 1.6f;
        var logBox = new RectangleF(card.X + pad, ly, card.Width - 2 * pad, card.Bottom - pad - ly);
        Card(g, logBox, h * 0.012f, Color.FromArgb(232, 226, 206), Color.FromArgb(226, 220, 200), Fade(Line, 90), 1);
        float lh = smallFont.Height * 1.15f, lp = h * 0.015f, yy = logBox.Bottom - lp - lh;
        for (int i = 1; i < lines.Count && yy >= logBox.Y + lp; i++) {
            Write(g, lines[i], smallFont, Text2, new RectangleF(logBox.X + lp, yy, logBox.Width - 2 * lp, lh), StringAlignment.Near, StringAlignment.Near, false);
            yy -= lh;
        }
    }
}

static class NativeMonsLauncher {
    [STAThread]
    static void Main(string[] argv) {
        Application.EnableVisualStyles();
        Config.Load();
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
