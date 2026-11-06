// nativeMons launcher: one menu for native Pokemon ports on PC. Licensed under the GNU GPL v3 or later (see LICENSE).
// Built with Windows' own C# 5 compiler (build.bat). Design, credits and command line: notes/launcher.md.
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
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, int msg, IntPtr w, IntPtr l);

    // Windows only lets the foreground owner hand the foreground on; joining its input for the call lifts that.
    public static readonly bool Quiet = Environment.GetEnvironmentVariable("NATIVEMONS_TEST_QUIET") == "1";
    public static void Front(IntPtr h) {
        if (Quiet) return;
        if (h == IntPtr.Zero) return;
        if (IsIconic(h)) ShowWindow(h, 9);
        uint fg = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero), me = GetCurrentThreadId();
        bool joined = fg != 0 && fg != me && AttachThreadInput(me, fg, true);
        BringWindowToTop(h); SetForegroundWindow(h);
        if (joined) AttachThreadInput(me, fg, false);
    }
}

// Discord Rich Presence over Discord's local pipe, with no library (notes/launcher.md, Discord).
static class DiscordPresence {
    // Registered at discord.com/developers. A file in state overrides it, for testing.
    const string AppId = "1566980838325747914";
    const string ImageKey = "nativemons", Home = "https://github.com/Hailey-Ross/nativeMons";
    static readonly object gate = new object();
    static System.IO.Pipes.NamedPipeClientStream pipe;
    static int generation;
    public static bool Enabled { get { return Config.Read("discord.txt", "on").Trim() != "off"; } set { Config.Write("discord.txt", value ? "on" : "off"); if (!value) Stop(); } }
    static string Id { get { string own = Config.Read("discord-app.txt", "").Trim(); return own != "" ? own : AppId; } }
    public static bool Available { get { return Id != ""; } }
    public static void Playing(GameEntry g) {
        if (g == null || !Enabled || !Available) return;
        string title = g.Title, id = Id;
        long started = (long)(DateTime.UtcNow - new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc)).TotalSeconds;
        int mine;
        lock (gate) mine = ++generation;
        Task.Run(() => {
            try {
                var p = Connect(id);
                if (p == null) return;
                // The game, and a button to the project's GitHub page.
                var activity = new Dictionary<string, object> {
                    { "details", title.StartsWith("Pokemon ") ? "Pokémon " + title.Substring(8) : title },
                    { "details_url", Home },
                    { "timestamps", new Dictionary<string, object> { { "start", started } } },
                    { "assets", new Dictionary<string, object> { { "large_image", ImageKey }, { "large_text", "nativeMons launcher" }, { "large_url", Home } } },
                    { "buttons", new object[] { new Dictionary<string, object> { { "label", "nativeMons on GitHub" }, { "url", Home } } } },
                };
                Send(p, 1, new Dictionary<string, object> { { "cmd", "SET_ACTIVITY" }, { "nonce", Guid.NewGuid().ToString() },
                    { "args", new Dictionary<string, object> { { "pid", Process.GetCurrentProcess().Id }, { "activity", activity } } } });
                string reply = Receive(p);
                string testLog = Environment.GetEnvironmentVariable("NATIVEMONS_TEST_LOG");
                if (testLog != null) try { File.AppendAllText(testLog, "discord: " + reply + Environment.NewLine); } catch { }
                lock (gate) {
                    // The game may have closed while this was connecting.
                    if (mine != generation) { p.Dispose(); return; }
                    if (pipe != null) pipe.Dispose();
                    pipe = p;
                }
            } catch { }
        });
    }
    public static void Stop() {
        lock (gate) { generation++; if (pipe != null) { try { pipe.Dispose(); } catch { } pipe = null; } }
    }
    static System.IO.Pipes.NamedPipeClientStream Connect(string id) {
        for (int i = 0; i < 10; i++) {
            var p = new System.IO.Pipes.NamedPipeClientStream(".", "discord-ipc-" + i, System.IO.Pipes.PipeDirection.InOut);
            try {
                p.Connect(200);
                Send(p, 0, new Dictionary<string, object> { { "v", 1 }, { "client_id", id } });
                // Discord answers the handshake with READY, or closes the pipe for an unknown application.
                if (Receive(p) != null) return p;
            } catch { }
            p.Dispose();
        }
        return null;
    }
    static void Send(Stream s, int op, object body) {
        byte[] data = Encoding.UTF8.GetBytes(new JavaScriptSerializer().Serialize(body));
        var frame = new byte[8 + data.Length];
        BitConverter.GetBytes(op).CopyTo(frame, 0);
        BitConverter.GetBytes(data.Length).CopyTo(frame, 4);
        data.CopyTo(frame, 8);
        s.Write(frame, 0, frame.Length); s.Flush();
    }
    static string Receive(Stream s) {
        var head = ReadExactly(s, 8);
        if (head == null) return null;
        int op = BitConverter.ToInt32(head, 0), length = BitConverter.ToInt32(head, 4);
        if (length < 0 || length > 1 << 20) return null;
        var body = ReadExactly(s, length);
        // Op 2 is Discord closing the connection, with its reason.
        return body == null || op == 2 ? null : Encoding.UTF8.GetString(body);
    }
    static byte[] ReadExactly(Stream s, int n) {
        var b = new byte[n]; int got = 0;
        while (got < n) { int r = s.Read(b, got, n - got); if (r <= 0) return null; got += r; }
        return b;
    }
}
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
    public static string GbaPlayer { get { return Get("gbawide", @"engines\gbawide\gbawide.exe"); } }
    // Installed PC games live beside the engines, where a launcher update never looks.
    public static string InfiniteFusion { get { return Get("infinitefusion", @"games\infinitefusion\Game.exe"); } }
    public static string InfiniteFusion2 { get { return Get("infinitefusion2", @"games\infinitefusion2\InfiniteFusion2.exe"); } }
    public static string Patcher { get { return Get("patcher", @"tools\nds-patcher\hails_patcher.py"); } }
    public static string MonBridge { get { return Get("monbridge", @"tools\mon-bridge\mon-bridge.exe"); } }
    public static string MgbaCore { get { return Get("mgbacore", @"tools\mgba\mgba_libretro.dll"); } }
    public static string GamesFile { get { return Get("games", "games.json"); } }
    // A python= setting, else the Python the launcher fetched for the patcher (PythonSetup), else the system's.
    public static string Python { get { string v; if (values.TryGetValue("python", out v)) return v; string own = Path.Combine(PythonDir, "python.exe"); return File.Exists(own) ? own : "python"; } }
    public static string PythonDir { get { return Get("pythondir", @"tools\python"); } }
    public static string FulldexDir { get { return Path.GetDirectoryName(Fulldex); } }
    public static List<string> RomFolders() {
        string f = Path.Combine(State, "rom-folders.txt");
        var l = File.Exists(f) ? File.ReadAllLines(f).Where(x => x.Trim() != "").ToList() : new List<string>();
        return l;
    }
    public static void SaveRomFolders(List<string> folders) { File.WriteAllLines(Path.Combine(State, "rom-folders.txt"), folders.ToArray()); }
    // ROMs added one at a time, used where they are.
    public static List<string> RomFiles() {
        string f = Path.Combine(State, "rom-files.txt");
        return File.Exists(f) ? File.ReadAllLines(f).Where(x => x.Trim() != "").Distinct(StringComparer.OrdinalIgnoreCase).ToList() : new List<string>();
    }
    public static void SaveRomFiles(List<string> files) { File.WriteAllLines(Path.Combine(State, "rom-files.txt"), files.ToArray()); }
    // ROMs from a .zip: our own copies, one folder per zip.
    public static string Unzipped { get { return Path.Combine(State, "roms"); } }
    public static string Read(string name, string fallback) {
        try { string f = Path.Combine(State, name); return File.Exists(f) ? File.ReadAllText(f).Trim() : fallback; } catch { return fallback; }
    }
    public static void Write(string name, string text) { try { File.WriteAllText(Path.Combine(State, name), text); } catch { } }
}

// A version's colours from games.json, and its finish: plain or a scene.
class Theme {
    public Color Main, Deep, Light, Ink; public string Finish = "plain";
    public string GameId;   // whose captured title screen to show
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
    public Theme Copy() { return (Theme)MemberwiseClone(); }
}
class GameEntry {
    public Theme Theme = Theme.Default;
    public int Dex;   // how many National Dex entries this game has
    public string Id, Title, Console, Gen1, FulldexCode, Header, Save, Port, Nativeplat, TownCore;
    // A PC program of its own (PcGames), with no ROM. Save tools, Move and Collection leave it out.
    public string Pc;
    public bool OnPc { get { return Pc != null && PcGames.Installed(this); } }
    public bool SaveTools { get { return Playable && Pc == null; } }
    public int ArtFrame, TownFrame;   // frames from power-on to the title screen
    public string Gen1Rom, FulldexRom, NativeplatRom, FoundUnsupported, FoundSha1;
    public List<RomFile> Roms = new List<RomFile>();   // every distinct dump found
    public RomFile Chosen;
    public bool CanVerify { get { return Gen1 != null || Nativeplat != null; } }
    // Two dumps can share a revision, such as a clean one and a hack. They must not read the same.
    public bool IsModified(RomFile r) { return !r.Verified && Roms.Any(x => x != r && x.Verified && x.Revision == r.Revision); }
    public string LabelOf(RomFile r) {
        if (r == null) return "";
        if (IsModified(r)) return "Romhack or modified";
        if (!Roms.Any(x => x != r && x.Revision == r.Revision && !IsModified(x))) return r.Label;
        return r.Label + " (" + r.Sha1.Substring(0, 6) + ")";
    }
    public bool Imported;
    // A romhack (Library.Hacks): a game of its own on Fulldex, with its own save.
    public string BaseId;
    // Known patched builds of this game, by SHA-1: { id, title } (games.json "hacks").
    public Dictionary<string, string[]> KnownHacks = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
    public bool IsHack { get { return BaseId != null; } }
    public GameEntry CopyOf() { return (GameEntry)MemberwiseClone(); }
    public bool OnGen1 { get { return Gen1 != null && (Imported || Gen1Rom != null); } }
    public bool OnFulldex { get { return FulldexCode != null && FulldexRom != null && File.Exists(Config.Fulldex); } }
    public bool OnPort { get { return Port != null && FulldexRom != null && File.Exists(Config.PlatinumPort); } }
    public bool OnNativeplat { get { return Nativeplat != null && NativeplatRom != null && File.Exists(Config.Nativeplat); } }
    public bool Playable { get { return OnGen1 || OnFulldex || OnPort || OnNativeplat || OnPc; } }
    public bool IsDs { get { return Console == "Nintendo DS"; } }
    public bool HasBoth { get { return OnGen1 && OnFulldex; } }
    // The player's choice where both engines can run it; Fulldex by default (widescreen).
    public List<string> Engines() {
        var l = new List<string>();
        if (OnFulldex) l.Add("fulldex");
        if (OnGen1) l.Add("gen1");
        if (OnPort) l.Add("port");
        if (OnNativeplat) l.Add("nativeplat");
        if (OnPc) l.Add("pc");
        // A romhack only runs on Fulldex: the other engines only take clean copies.
        if (Chosen != null && IsModified(Chosen) && l.Contains("fulldex")) return new List<string> { "fulldex" };
        return l;
    }
    public string Engine() {
        var l = Engines();
        if (l.Count == 0) return "gen1";
        string chosen = Config.Read("engine-" + Id + ".txt", l[0]);
        return l.Contains(chosen) ? chosen : l[0];
    }
    public static string EngineName(string engine) { return engine == "fulldex" ? "Fulldex (widescreen, 60 fps)" : engine == "port" ? "the Platinum PC port (native)" : engine == "nativeplat" ? "nativeplat (native)" : engine == "pc" ? "its own program" : "Gen1Recomp (native, with mods)"; }
    public static string EngineShort(string engine) { return engine == "fulldex" ? "Fulldex" : engine == "port" ? "Platinum PC port" : engine == "nativeplat" ? "nativeplat" : engine == "pc" ? "PC" : "Gen1Recomp"; }
}

// Games that are PC programs of their own, such as Infinite Fusion (notes/launcher.md, PC games).
static class PcGames {
    // Each PC game the launcher installs from its GitHub: Infinite Fusion and Infinite Fusion 2 (Hoenn), both Pokemon
    // Essentials on mkxp-z, so they install, update and keep their saves the same way.
    // Title*: its title screen's pieces under Graphics (background layers back to front, the one that drifts, the swirl,
    // the logo); Town: the picture its wallpaper comes from.
    public class Profile { public string Name, Repo, Branch, Exe, SaveFolder, Size, TitleSwirl, TitleLogo, Town; public string[] TitleBg; public int Drift = -1; public Func<string> OwnExe; }
    static readonly Dictionary<string, Profile> Profiles = new Dictionary<string, Profile> {
        { "infinitefusion", new Profile { Name = "Pokemon Infinite Fusion", Repo = "https://github.com/infinitefusion/infinitefusion-e18.git",
            Branch = "releases", Exe = "Game.exe", SaveFolder = "infinitefusion", Size = "about 5 GB", OwnExe = () => Config.InfiniteFusion,
            TitleBg = new[] { @"Titles\gen1_bg_v5.png" }, TitleSwirl = @"Titles\PokemonInfiniteFusionLogo_Back_25.png",
            TitleLogo = @"Titles\PokemonInfiniteFusionLogo_Main_25.png", Town = @"Panoramas\Kanto_HGSS.png" } },
        { "infinitefusion2", new Profile { Name = "Pokemon Infinite Fusion 2", Repo = "https://github.com/infinitefusion/infinitefusion-hoenn-public.git",
            Branch = "releases", Exe = "InfiniteFusion2.exe", SaveFolder = "infinitefusion-hoenn", Size = "about 3 GB", OwnExe = () => Config.InfiniteFusion2,
            TitleBg = new[] { @"Titles\bg_back.png", @"Titles\bg_clouds.png", @"Titles\bg_front.png" }, Drift = 1,
            TitleSwirl = @"Titles\PokemonInfiniteFusionLogo_Back_25.png", TitleLogo = @"Titles\InfiniteFusionHoenn.png",
            Town = @"Pictures\map\map_hoenn.png" } },
    };
    public static Profile Of(GameEntry g) { Profile p; return g.Pc != null && Profiles.TryGetValue(g.Pc, out p) ? p : null; }
    public static bool IsFusion(GameEntry g) { return Of(g) != null; }
    // Imported (the launcher's copy) or existing (the player's, run in place). state\pc-<id>.txt names an existing one.
    static string ExistingFile(GameEntry g) { return Path.Combine(Config.State, "pc-" + g.Id + ".txt"); }
    public static string Existing(GameEntry g) { string d = Config.Read("pc-" + g.Id + ".txt", "").Trim(); return d != "" ? d : null; }
    public static bool IsExisting(GameEntry g) { return Existing(g) != null; }
    static string OwnExe(GameEntry g) { var p = Of(g); return p != null ? p.OwnExe() : null; }
    public static bool IsCustom(GameEntry g) { return g.Pc == "custom"; }
    public static string Exe(GameEntry g) {
        if (IsCustom(g)) { var c = CustomGames.Find(g.Id); return c != null ? CustomGames.Exe(c) : null; }
        string e = Existing(g); return e != null ? Path.Combine(e, Of(g).Exe) : OwnExe(g);
    }
    public static string Dir(GameEntry g) { string e = Exe(g); return e == null ? null : Path.GetDirectoryName(e); }
    public static string OwnDir(GameEntry g) { string e = OwnExe(g); return e == null ? null : Path.GetDirectoryName(e); }
    public static bool Installed(GameEntry g) { string e = Exe(g); return e != null && File.Exists(e); }
    public static string GameSaveDir(GameEntry g) { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), Of(g).SaveFolder); }
    public static string SaveDir(GameEntry g) { return IsExisting(g) ? GameSaveDir(g) : Path.Combine(Config.State, "saves", g.Id); }
    // Keeps an imported copy's saves beside the launcher (notes/launcher.md, PC games).
    const string PreloadName = "nativemons-saves.rb";
    const string Preload = "# nativeMons: when the launcher starts this copy, its saves go in the launcher's folder.\n"
        + "if ENV['NATIVEMONS_SAVE_DIR'] && !ENV['NATIVEMONS_SAVE_DIR'].empty?\n"
        + "  module System\n"
        + "    def self.data_directory\n"
        + "      d = ENV['NATIVEMONS_SAVE_DIR'].tr('\\\\', '/')\n"
        + "      d.end_with?('/') ? d : d + '/'\n"
        + "    end\n"
        + "  end\n"
        + "end\n";
    public static void PrepareSaves(GameEntry g, ProcessStartInfo psi) {
        if (IsExisting(g) || IsCustom(g)) return;
        string dir = OwnDir(g), saves = SaveDir(g);
        Directory.CreateDirectory(saves);   // the game falls back to its own folder when this doesn't exist
        File.WriteAllText(Path.Combine(dir, PreloadName), Preload);
        // The game's own mkxp.json settings (vsync, frame skip...) stay: the preload line goes in after its opening brace.
        string json = Path.Combine(dir, "mkxp.json"), line = "  \"preloadScript\": [\"" + PreloadName + "\"],\n";
        string had = File.Exists(json) ? File.ReadAllText(json) : null;
        int brace = had == null ? -1 : had.IndexOf('{');
        if (had == null || brace < 0) File.WriteAllText(json, "{\n" + line + "}\n");
        else if (!had.Contains(PreloadName)) File.WriteAllText(json, had.Substring(0, brace + 1) + "\n" + line + had.Substring(brace + 1));
        psi.EnvironmentVariables["NATIVEMONS_SAVE_DIR"] = saves;
    }
    // Saves already in the game's usual folder are copied in; the originals stay.
    static string BringSaves(GameEntry g) {
        string from = GameSaveDir(g), to = SaveDir(g);
        Directory.CreateDirectory(to);
        if (!Directory.Exists(from) || Directory.GetFiles(to, "*.rxdata").Length > 0) return null;
        var files = Directory.GetFiles(from, "*.rxdata");
        foreach (var f in files) File.Copy(f, Path.Combine(to, Path.GetFileName(f)));
        return files.Length > 0 ? files.Length + (files.Length == 1 ? " save was" : " saves were") + " copied in from " + from + "; the originals are still there." : null;
    }
    public static string AddExisting(GameEntry g, string folder) {
        var pf = Of(g);
        if (!File.Exists(Path.Combine(folder, pf.Exe))) return "That folder has no " + pf.Exe + ", so it isn't " + pf.Name + ".";
        if (string.Equals(Path.GetFullPath(folder).TrimEnd('\\'), Path.GetFullPath(OwnDir(g)).TrimEnd('\\'), StringComparison.OrdinalIgnoreCase)) return "That is the launcher's own copy.";
        Directory.CreateDirectory(Config.State);
        File.WriteAllText(ExistingFile(g), Path.GetFullPath(folder));
        return null;
    }
    // Forgets an existing installation; nothing of it is deleted.
    public static void Forget(GameEntry g) { try { File.Delete(ExistingFile(g)); } catch { } }
    static bool Running(GameEntry g) {
        string exe = Exe(g);
        foreach (var p in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exe)))
            try { if (string.Equals(p.MainModule.FileName, exe, StringComparison.OrdinalIgnoreCase)) return true; } catch { } finally { p.Dispose(); }
        return false;
    }
    public static bool RunningFrom(string dir) {
        string root = Path.GetFullPath(dir).TrimEnd('\\') + "\\";
        foreach (var p in Process.GetProcesses())
            try { if (p.MainModule.FileName.StartsWith(root, StringComparison.OrdinalIgnoreCase)) return true; } catch { } finally { p.Dispose(); }
        return false;
    }
    public static bool AnyRunning() { return Library.Games.Any(g => g.Pc != null && Installed(g) && Running(g)); }
    // Git on the PC, or null.
    public static string Git() {
        foreach (var dir in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(';'))
            try { string f = Path.Combine(dir.Trim(), "git.exe"); if (dir.Trim() != "" && File.Exists(f)) return f; } catch { }
        string pf = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Git", "cmd", "git.exe");
        return File.Exists(pf) ? pf : null;
    }
    static int RunGit(string args, string dir, Action<string> line) {
        var psi = new ProcessStartInfo(Git(), args) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
        if (dir != null) psi.WorkingDirectory = dir;
        psi.EnvironmentVariables["GIT_TERMINAL_PROMPT"] = "0";
        using (var p = new Process { StartInfo = psi }) {
            DataReceivedEventHandler each = delegate(object s, DataReceivedEventArgs e) { if (e.Data != null && line != null) line(e.Data); };
            p.OutputDataReceived += each; p.ErrorDataReceived += each;
            p.Start(); p.BeginOutputReadLine(); p.BeginErrorReadLine();
            p.WaitForExit();
            return p.ExitCode;
        }
    }
    // "6.8.2" and the release's own words, from the last commit's subject ("[6.8.2] Safari Zone crash patch").
    public static string Version(GameEntry g) {
        if (IsCustom(g)) { try { string v = Installed(g) ? FileVersionInfo.GetVersionInfo(Exe(g)).ProductVersion : null; return string.IsNullOrWhiteSpace(v) ? null : v.Trim(); } catch { return null; } }
        if (!Installed(g) || Git() == null || !Directory.Exists(Path.Combine(Dir(g), ".git"))) return null;
        string subject = null;
        try { RunGit("log -1 --format=%s", Dir(g), delegate(string l) { if (subject == null) subject = l.Trim(); }); } catch { }
        return subject;
    }
    // An install replaces the old folder only once it's complete.
    // What the last install did with the player's saves, for its message.
    public static string Saved;
    public static string Install(GameEntry g, string source, Action<string, float> report) {
        Saved = null;
        var pf = Of(g);
        string target = OwnDir(g), work = target + ".installing";
        try {
            if (Directory.Exists(work)) Directory.Delete(work, true);
            if (source == null) {
                if (Git() == null) return "Downloading needs Git for Windows (git-scm.com). Or install from your own copy of the game.";
                Directory.CreateDirectory(Path.GetDirectoryName(work));
                report("Downloading " + pf.Name + " from GitHub (" + pf.Size + ")", -1);
                int code = RunGit("clone --depth=1 --branch " + pf.Branch + " --progress " + pf.Repo + " \"" + work + "\"", null, delegate(string l) { report(l, -1); });
                if (code != 0) return "The download stopped (git said " + code + "). Nothing was changed.";
            } else if (File.Exists(source)) {
                using (var zip = System.IO.Compression.ZipFile.OpenRead(source)) {
                    // The game's own zip has its files at the top; one inside a single folder is unwrapped.
                    var exe = zip.Entries.Where(e => e.Name.Equals(pf.Exe, StringComparison.OrdinalIgnoreCase)).OrderBy(e => e.FullName.Length).FirstOrDefault();
                    if (exe == null) return "That zip has no " + pf.Exe + ", so it isn't " + pf.Name + ".";
                    string prefix = exe.FullName.Substring(0, exe.FullName.Length - exe.Name.Length);
                    var files = zip.Entries.Where(e => e.Name != "" && e.FullName.StartsWith(prefix)).ToList();
                    long total = Math.Max(1, files.Sum(e => e.Length)), done = 0;
                    if (!Room(target, total)) return "There isn't room for it on that drive: it needs about " + (total >> 30) + " GB.";
                    foreach (var e in files) {
                        string to = Path.Combine(work, e.FullName.Substring(prefix.Length).Replace('/', '\\'));
                        Directory.CreateDirectory(Path.GetDirectoryName(to));
                        System.IO.Compression.ZipFileExtensions.ExtractToFile(e, to, true);
                        done += e.Length;
                        report("Unpacking " + e.FullName.Substring(prefix.Length), (float)done / total);
                    }
                }
            } else if (Directory.Exists(source)) {
                if (!File.Exists(Path.Combine(source, pf.Exe))) return "That folder has no " + pf.Exe + ", so it isn't " + pf.Name + ".";
                if (string.Equals(Path.GetFullPath(source).TrimEnd('\\'), Path.GetFullPath(target).TrimEnd('\\'), StringComparison.OrdinalIgnoreCase)) return "That is the launcher's own copy.";
                var files = Library.FilesUnder(source);
                long total = Math.Max(1, files.Sum(f => { try { return new FileInfo(f).Length; } catch { return 0L; } })), done = 0;
                if (!Room(target, total)) return "There isn't room for it on that drive: it needs about " + (total >> 30) + " GB.";
                foreach (var f in files) {
                    string rel = f.Substring(source.TrimEnd('\\').Length + 1), to = Path.Combine(work, rel);
                    Directory.CreateDirectory(Path.GetDirectoryName(to));
                    File.Copy(f, to, true);
                    File.SetAttributes(to, File.GetAttributes(to) & ~FileAttributes.ReadOnly);
                    try { done += new FileInfo(f).Length; } catch { }
                    report("Copying " + rel, (float)done / total);
                }
            } else return "That file or folder isn't there any more.";
            if (!File.Exists(Path.Combine(work, pf.Exe))) return pf.Exe + " didn't arrive, so nothing was changed.";
            report("Finishing", 1f);
            if (Directory.Exists(target)) Remove(target);
            Directory.Move(work, target);
            // From now on the launcher's copy is the one played, with its saves in the launcher's folder.
            Forget(g);
            Saved = BringSaves(g);
            return null;
        } catch (Exception x) { return "Installing stopped: " + x.Message + " The copy you had (if any) is unchanged."; }
        finally { try { if (Directory.Exists(work)) Remove(work); } catch { } }
    }
    // A shallow fetch and hard reset, as the game's own updater does. Only the launcher's own copy.
    public static string Update(GameEntry g, Action<string> line) {
        if (IsExisting(g)) return "This is your own installation, so the launcher leaves it alone. Use the game's own updater, or import a copy into the launcher.";
        string dir = OwnDir(g);
        if (Git() == null) return "Updating needs Git for Windows (git-scm.com).";
        if (!Directory.Exists(Path.Combine(dir, ".git"))) return "This copy has no update history (.git). Install it again from GitHub to get updates.";
        try { File.Delete(Path.Combine(dir, ".git", "shallow.lock")); } catch { }
        var pf = Of(g);
        RunGit("remote set-url origin " + pf.Repo, dir, null);
        if (RunGit("fetch --depth=1 --progress origin " + pf.Branch, dir, line) != 0) return "Couldn't reach the game's GitHub. Nothing was changed.";
        if (RunGit("reset --hard origin/" + pf.Branch, dir, line) != 0) return "The update didn't apply cleanly. Install it again to repair it.";
        return null;
    }
    // Git marks its object files read-only, which Directory.Delete refuses.
    public static void Remove(string dir) {
        foreach (var f in Directory.GetFiles(dir, "*", SearchOption.AllDirectories)) try { File.SetAttributes(f, FileAttributes.Normal); } catch { }
        Directory.Delete(dir, true);
    }
    static bool Room(string target, long bytes) {
        try { var d = new DriveInfo(Path.GetPathRoot(Path.GetFullPath(target))); return d.AvailableFreeSpace > bytes + (1L << 30); } catch { return true; }
    }
    // Before each session: the save files copied to state\save-backups\<id>\<time>, newest 30 kept.
    public static void Backup(GameEntry g) {
        if (IsCustom(g)) return;   // where an added game keeps its saves is its own business
        string from = SaveDir(g);
        if (!Directory.Exists(from)) return;
        var files = Directory.GetFiles(from, "*.rxdata");
        if (files.Length == 0) return;
        string root = Path.Combine(Config.State, "save-backups", g.Id), to = Path.Combine(root, DateTime.Now.ToString("yyyyMMdd-HHmmss"));
        Directory.CreateDirectory(to);
        foreach (var f in files) File.Copy(f, Path.Combine(to, Path.GetFileName(f)), true);
        foreach (var old in Directory.GetDirectories(root).OrderByDescending(x => x, StringComparer.Ordinal).Skip(30)) try { Directory.Delete(old, true); } catch { }
    }
}

// PC games the player adds, kept in state\pc-games.json (notes/launcher.md, PC games).
static class CustomGames {
    // exe: the program run in place, or what the copy came from. rel: its program inside the copy, when not at the top.
    public class Entry { public string id, title, exe, rel; public bool copy; }
    static string File_ { get { return Path.Combine(Config.State, "pc-games.json"); } }
    public static List<Entry> All() {
        try { return File.Exists(File_) ? new JavaScriptSerializer().Deserialize<List<Entry>>(File.ReadAllText(File_)) ?? new List<Entry>() : new List<Entry>(); }
        catch { return new List<Entry>(); }
    }
    static void Save(List<Entry> all) { Directory.CreateDirectory(Config.State); File.WriteAllText(File_, new JavaScriptSerializer().Serialize(all)); }
    public static Entry Find(string id) { return All().Find(e => e.id == id); }
    // Their themes come from this set in turn, so they don't all look alike.
    static readonly string[][] Looks = {
        new[] { "#4F6D8F", "#243447", "#9CC3E6" }, new[] { "#5E8C61", "#2C4430", "#A9D8A3" }, new[] { "#8A5A9E", "#41284D", "#D6B0E6" },
        new[] { "#B5793A", "#5A3A16", "#F0C48A" }, new[] { "#3E8A8A", "#1C4242", "#9BDADA" }, new[] { "#9E4F5E", "#4A2129", "#E6A7B3" } };
    public static GameEntry ToGame(Entry e, int n) {
        var look = Looks[n % Looks.Length];
        var g = new GameEntry { Id = e.id, Title = e.title, Console = "PC", Pc = "custom" };
        g.Theme = Theme.Make(look[0], look[1], look[2], "plain"); g.Theme.GameId = g.Id;
        return g;
    }
    public static string Dir(Entry e) { return e.copy ? Path.Combine(Config.Here, "games", e.id) : Path.GetDirectoryName(e.exe); }
    public static string Exe(Entry e) { return e.copy ? Path.Combine(Dir(e), string.IsNullOrEmpty(e.rel) ? Path.GetFileName(e.exe) : e.rel) : e.exe; }
    // A folder's programs, likeliest first, leaving out installers, crash reporters and runtimes.
    static readonly string[] NotTheGame = { "unins", "setup", "install", "crash", "redist", "vcredist", "dxsetup", "dotnet", "update", "launcherhelper", "notification_helper" };
    public static List<string> Programs(string dir) {
        string root = dir.TrimEnd('\\');
        return Library.FilesUnder(root).Where(f => Path.GetExtension(f).ToLowerInvariant() == ".exe")
            .Where(f => { string n = Path.GetFileNameWithoutExtension(f).ToLowerInvariant().Replace(" ", "").Replace("_", ""); return !NotTheGame.Any(n.Contains); })
            .OrderBy(f => f.Substring(root.Length).Count(c => c == '\\')).ThenBy(f => f, StringComparer.OrdinalIgnoreCase).ToList();
    }
    // Unpacks a .zip into games\<id>, never outside it. choose: null keeps it all for the player to pick.
    public static string Unpack(string zip, Action<string, float> report, out string work, out List<string> programs) {
        work = null; programs = null;
        if (!File.Exists(zip)) return "That zip isn't there.";
        // One left by a choice that was never made (the page closed with Back) goes first.
        try { if (Directory.Exists(Path.Combine(Config.Here, "games"))) foreach (var old in Directory.GetDirectories(Path.Combine(Config.Here, "games"), "unpacking-*")) PcGames.Remove(old); } catch { }
        string stage = Path.Combine(Config.Here, "games", "unpacking-" + Guid.NewGuid().ToString("N").Substring(0, 8));
        try {
            using (var archive = System.IO.Compression.ZipFile.OpenRead(zip)) {
                var files = archive.Entries.Where(x => x.Name != "").ToList();
                if (files.Count == 0) return "That zip is empty.";
                string top = null;
                var firsts = files.Select(x => x.FullName.Replace('\\', '/').Split('/')).ToList();
                if (firsts.All(s => s.Length > 1) && firsts.Select(s => s[0]).Distinct(StringComparer.OrdinalIgnoreCase).Count() == 1) top = firsts[0][0] + "/";
                long total = Math.Max(1, files.Sum(x => x.Length)), done = 0;
                string full = Path.GetFullPath(stage) + "\\";
                foreach (var entry in files) {
                    string name = entry.FullName.Replace('\\', '/');
                    if (top != null) name = name.Substring(top.Length);
                    string target = Path.GetFullPath(Path.Combine(stage, name.Replace('/', '\\')));
                    if (!target.StartsWith(full, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("It holds a file meant for outside its folder (" + entry.FullName + ").");
                    Directory.CreateDirectory(Path.GetDirectoryName(target));
                    System.IO.Compression.ZipFileExtensions.ExtractToFile(entry, target, true);
                    done += entry.Length;
                    report("Unpacking " + name, (float)done / total);
                }
            }
            programs = Programs(stage);
            if (programs.Count == 0) { PcGames.Remove(stage); return "That zip has no program (.exe) in it, so there's nothing to start. Nothing was added."; }
            work = stage;
            return null;
        } catch (Exception x) {
            try { if (Directory.Exists(stage)) PcGames.Remove(stage); } catch { }
            return "Unpacking stopped: " + x.Message + " Nothing was added.";
        }
    }
    // The unpacked zip becomes the game's own folder in games\, running the chosen program.
    public static string AddUnpacked(string zip, string work, string program, out Entry added) {
        added = null;
        try {
            var e = NewEntry(program, Path.GetFileNameWithoutExtension(zip));
            e.exe = Path.GetFullPath(zip); e.copy = true; e.rel = program.Substring(work.TrimEnd('\\').Length + 1);
            Directory.Move(work, Dir(e));
            var all = All(); all.Add(e); Save(all); added = e;
            return null;
        } catch (Exception x) { try { if (Directory.Exists(work)) PcGames.Remove(work); } catch { } return "Adding stopped: " + x.Message + " Nothing was added."; }
    }
    // An id and a name: the program's own product name, else the fallback (its folder's, or the zip's).
    static Entry NewEntry(string exe, string fallback) {
        string title = null;
        try { title = FileVersionInfo.GetVersionInfo(exe).ProductName; } catch { }
        if (string.IsNullOrWhiteSpace(title) || title.Length > 40) title = fallback;
        string slug = System.Text.RegularExpressions.Regex.Replace(title.ToLowerInvariant(), "[^a-z0-9]+", "-").Trim('-');
        if (slug == "") slug = "game";
        var all = All();
        string id = "pc-" + slug; for (int i = 2; all.Any(x => x.id == id) || Library.Games.Any(x => x.Id == id); i++) id = "pc-" + slug + "-" + i;
        return new Entry { id = id, title = title, exe = Path.GetFullPath(exe) };
    }
    // Copied into the launcher, or run where it is. Named after the program's product name, else its folder.
    public static string Add(string exe, bool copy, Action<string, float> report, out Entry added) {
        added = null;
        if (!File.Exists(exe)) return "That program isn't there.";
        var all = All();
        var e = NewEntry(exe, new DirectoryInfo(Path.GetDirectoryName(exe)).Name); e.copy = copy;
        if (copy) {
            string from = Path.GetDirectoryName(e.exe), to = Dir(e), work = to + ".adding";
            if (Path.GetPathRoot(from).TrimEnd('\\') == from.TrimEnd('\\')) return "That program sits at the top of a drive, so its folder can't be copied. Add it where it is instead.";
            try {
                if (Directory.Exists(work)) PcGames.Remove(work);
                var files = Library.FilesUnder(from);
                long total = Math.Max(1, files.Sum(f => { try { return new FileInfo(f).Length; } catch { return 0L; } })), done = 0;
                foreach (var f in files) {
                    string rel = f.Substring(from.TrimEnd('\\').Length + 1), target = Path.Combine(work, rel);
                    Directory.CreateDirectory(Path.GetDirectoryName(target));
                    File.Copy(f, target, true);
                    File.SetAttributes(target, File.GetAttributes(target) & ~FileAttributes.ReadOnly);
                    try { done += new FileInfo(f).Length; } catch { }
                    report("Copying " + rel, (float)done / total);
                }
                Directory.Move(work, to);
            } catch (Exception x) { try { if (Directory.Exists(work)) PcGames.Remove(work); } catch { } return "Copying stopped: " + x.Message + " Nothing was added."; }
        }
        all.Add(e); Save(all); added = e;
        return null;
    }
    public static void Rename(string id, string title) { var all = All(); var e = all.Find(x => x.id == id); if (e == null) return; e.title = title; Save(all); }
    // Forgets the game; a copy the launcher held is deleted, the player's own never is.
    public static string Remove(string id) {
        var all = All(); var e = all.Find(x => x.id == id);
        if (e == null) return null;
        if (e.copy && Directory.Exists(Dir(e))) try { PcGames.Remove(Dir(e)); } catch (Exception x) { return "Couldn't delete its copy: " + x.Message; }
        all.Remove(e); Save(all);
        return null;
    }
}

// Read-only Ruby Marshal 4.8, as RPG Maker saves use. Anything unknown stops it (notes/launcher.md, PC games).
sealed class RSym {
    public readonly string Name;
    public RSym(string n) { Name = n; }
    public override bool Equals(object o) { var r = o as RSym; return r != null && r.Name == Name; }
    public override int GetHashCode() { return Name.GetHashCode(); }
    public override string ToString() { return ":" + Name; }
}
sealed class RObj {
    public string Class; public Dictionary<string, object> Ivars = new Dictionary<string, object>();
    public object this[string ivar] { get { object v; return Ivars.TryGetValue(ivar, out v) ? v : null; } }
}
sealed class RubyMarshal {
    readonly byte[] b; int p = 2;
    readonly List<RSym> syms = new List<RSym>(); readonly List<object> objs = new List<object>();
    RubyMarshal(byte[] data) { b = data; }
    public static object Load(byte[] data) {
        if (data.Length < 2 || data[0] != 4 || data[1] != 8) throw new InvalidDataException("Not Ruby Marshal 4.8 data.");
        return new RubyMarshal(data).Read();
    }
    byte Byte() { if (p >= b.Length) throw new InvalidDataException("Marshal data cut short."); return b[p++]; }
    long Long() {
        int c = (sbyte)Byte();
        if (c == 0) return 0;
        if (c > 4) return c - 5;
        if (c < -4) return c + 5;
        long v = 0;
        if (c > 0) { for (int i = 0; i < c; i++) v |= (long)Byte() << (8 * i); return v; }
        v = -1;
        for (int i = 0; i < -c; i++) { v &= ~(0xFFL << (8 * i)); v |= (long)Byte() << (8 * i); }
        return v;
    }
    byte[] Bytes() { int n = (int)Long(); if (n < 0 || p + n > b.Length) throw new InvalidDataException("Marshal data cut short."); var v = new byte[n]; Array.Copy(b, p, v, 0, n); p += n; return v; }
    RSym Sym() {
        char t = (char)Byte();
        if (t == ':') { var s = new RSym(Encoding.UTF8.GetString(Bytes())); syms.Add(s); return s; }
        if (t == ';') return syms[(int)Long()];
        if (t == 'I') { var s = Sym(); long n = Long(); for (long i = 0; i < n; i++) { Sym(); Read(); } return s; }
        throw new InvalidDataException("Marshal: a symbol was expected.");
    }
    object Read() {
        char t = (char)Byte();
        switch (t) {
            case '0': return null;
            case 'T': return true;
            case 'F': return false;
            case 'i': return Long();
            case ':': case ';': p--; return Sym();
            case '@': return objs[(int)Long()];
            case 'I': { var v = Read(); long n = Long(); for (long i = 0; i < n; i++) { Sym(); Read(); } return v; }
            case '"': { var v = Encoding.UTF8.GetString(Bytes()); objs.Add(v); return v; }
            case 'f': { var txt = Encoding.ASCII.GetString(Bytes()); double d; object v = double.TryParse(txt, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out d) ? d : 0.0; objs.Add(v); return v; }
            case 'l': { Byte(); int n = (int)Long() * 2; p += n; objs.Add(0L); return 0L; }   // a big number: skipped
            case '[': { var a = new List<object>(); objs.Add(a); long n = Long(); for (long i = 0; i < n; i++) a.Add(Read()); return a; }
            case '{': case '}': {
                var h = new Dictionary<object, object>(); objs.Add(h); long n = Long();
                for (long i = 0; i < n; i++) { var k = Read(); var v = Read(); if (k != null) h[k] = v; }
                if (t == '}') Read();
                return h;
            }
            case 'o': case 'S': {
                var o = new RObj { Class = Sym().Name }; objs.Add(o); long n = Long();
                for (long i = 0; i < n; i++) { var k = Sym().Name; o.Ivars[k] = Read(); }
                return o;
            }
            case 'u': { var o = new RObj { Class = Sym().Name }; o.Ivars["_dump"] = Bytes(); objs.Add(o); return o; }
            case 'U': { var o = new RObj { Class = Sym().Name }; objs.Add(o); o.Ivars["_data"] = Read(); return o; }
            case 'e': case 'C': Sym(); return Read();
            case 'c': case 'm': { var v = Encoding.UTF8.GetString(Bytes()); objs.Add(v); return v; }
            case '/': { var v = Bytes(); Byte(); objs.Add(v); return v; }
        }
        throw new InvalidDataException("Marshal: unknown kind '" + t + "' at byte " + (p - 1) + ".");
    }
}

// Infinite Fusion's Pokedex, from its saves and Data\species.dat (notes/launcher.md, PC games).
static class InfiniteFusionDex {
    static readonly byte[] Key = { 0x4A, 0x8F, 0x2C, 0xE1, 0x73, 0xB5, 0x96, 0x0D, 0x5E, 0xA2, 0x3F, 0xC7, 0x81, 0x14, 0x6B, 0xD9 };
    public static string Plain(string name) {
        var o = new StringBuilder();
        foreach (char c in name.ToLowerInvariant().Replace("\u2640", "f").Replace("\u2642", "m").Normalize(NormalizationForm.FormD))
            if (char.IsLetterOrDigit(c) && c < 128) o.Append(c);
        return o.ToString();
    }
    static string namesFrom; static Dictionary<int, string> names;
    static Dictionary<int, string> Species(string gameDir) {
        string file = Path.Combine(gameDir, "Data", "species.dat");
        string print = file + "|" + new FileInfo(file).LastWriteTimeUtc.Ticks;
        if (names != null && namesFrom == print) return names;
        var raw = File.ReadAllBytes(file);
        if (!(raw.Length > 1 && raw[0] == 4 && raw[1] == 8)) for (int i = 0; i < raw.Length; i++) raw[i] ^= Key[i % Key.Length];
        var all = RubyMarshal.Load(raw) as Dictionary<object, object>;
        if (all == null) throw new InvalidDataException("Its species list isn't in the form expected.");
        var d = new Dictionary<int, string>();
        foreach (var kv in all) {
            var o = kv.Value as RObj; if (o == null || !(kv.Key is long)) continue;
            var n = o["real_name"] as string ?? o["@real_name"] as string;
            if (n != null) d[(int)(long)kv.Key] = n;
        }
        names = d; namesFrom = print;
        return d;
    }
    static int Count(object table) {
        int n = 0; var rows = table as List<object>; if (rows == null) return 0;
        foreach (var row in rows) { var r = row as List<object>; if (r != null) foreach (var x in r) if (x is bool && (bool)x) n++; }
        return n;
    }
    // Every save of the game, read together. national: a Pokemon's plain name (Plain) to its National Dex number.
    public static void Read(string gameDir, IEnumerable<string> saves, Dictionary<string, int> national, HashSet<int> caught, HashSet<int> seen, out int fusionsCaught, out int fusionsSeen, HashSet<int> laterCaught, HashSet<int> laterSeen) {
        fusionsCaught = 0; fusionsSeen = 0;
        var species = Species(gameDir);
        foreach (var file in saves) {
            var save = RubyMarshal.Load(File.ReadAllBytes(file)) as Dictionary<object, object>;
            object player; if (save == null || !save.TryGetValue(new RSym("player"), out player)) continue;
            var pl = player as RObj; var dex = pl != null ? pl["@pokedex"] as RObj : null;
            if (dex == null) continue;
            foreach (var key in new[] { "@owned_standard", "@seen_standard" }) {
                var list = dex[key] as List<object>; if (list == null) continue;
                bool owned = key == "@owned_standard";
                for (int i = 1; i < list.Count; i++) {
                    if (!(list[i] is bool) || !(bool)list[i]) continue;
                    string name; int nat;
                    if (i <= 251) nat = i;
                    else if (!species.TryGetValue(i, out name) || !national.TryGetValue(Plain(name), out nat)) {
                        // A later Pokemon than the collection lists (Oricorio, Carbink): kept by its own number.
                        (owned ? laterCaught : laterSeen).Add(i);
                        continue;
                    }
                    (owned ? caught : seen).Add(nat);
                }
            }
            fusionsCaught = Math.Max(fusionsCaught, Count(dex["@owned_fusion"]));
            fusionsSeen = Math.Max(fusionsSeen, Count(dex["@seen_fusion"]));
        }
        seen.UnionWith(caught); laterSeen.UnionWith(laterCaught);
    }
}

// The Gen1Recomp community mod index (notes/launcher.md, Mods).
static class ModIndex {
    public const string Feed = "https://bryanthaboi.github.io/gen1recomp-mod-index/data/index.json";
    public const string Site = "https://github.com/bryanthaboi/gen1recomp-mod-index";
    const long MaxZip = 300L * 1024 * 1024;
    public class Entry {
        public string Id, Title, Author, Summary, Version, Repo, License, ZipUrl, Folder;
        public List<string> Games = new List<string>(), Permissions = new List<string>(), Categories = new List<string>(), Needs = new List<string>(), Clashes = new List<string>();
        public bool Experimental; public long ZipSize; public long Downloads = -1;
    }
    static string CacheFile { get { return Path.Combine(Config.State, "mod-index.json"); } }
    public static int CachedCount() {
        try { return File.Exists(CacheFile) ? Parse(File.ReadAllText(CacheFile, Encoding.UTF8)).Count : 0; } catch { return 0; }
    }
    static string Str(Dictionary<string, object> d, string k) { object v; return d != null && d.TryGetValue(k, out v) ? v as string : null; }
    static List<string> Strs(Dictionary<string, object> d, string k) {
        object v; var o = new List<string>();
        if (d != null && d.TryGetValue(k, out v) && v is System.Collections.IEnumerable && !(v is string)) foreach (var x in (System.Collections.IEnumerable)v) if (x is string) o.Add((string)x);
        return o;
    }
    static Dictionary<string, object> Obj(Dictionary<string, object> d, string k) { object v; return d != null && d.TryGetValue(k, out v) ? v as Dictionary<string, object> : null; }
    // The list, from the feed; a copy kept six hours, and used whenever the feed can't be reached.
    public static List<Entry> Load(out string note) {
        note = null; string text = null;
        var age = File.Exists(CacheFile) ? DateTime.UtcNow - File.GetLastWriteTimeUtc(CacheFile) : TimeSpan.MaxValue;
        if (age > TimeSpan.FromHours(6)) {
            try {
                System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
                using (var web = new System.Net.WebClient()) {
                    web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons";
                    web.Encoding = Encoding.UTF8;
                    text = web.DownloadString(Feed);
                }
                Parse(text);   // a feed that doesn't read is never kept
                Directory.CreateDirectory(Config.State);
                File.WriteAllText(CacheFile + ".part", text, Encoding.UTF8);
                if (File.Exists(CacheFile)) File.Delete(CacheFile);
                File.Move(CacheFile + ".part", CacheFile);
            } catch (Exception x) {
                text = null;
                if (!File.Exists(CacheFile)) throw new IOException("The mod index couldn't be reached. " + x.Message);
                note = "The mod index couldn't be reached, so this is the list from " + File.GetLastWriteTime(CacheFile).ToString("d MMM, h:mm tt") + ".";
            }
        }
        if (text == null) text = File.ReadAllText(CacheFile, Encoding.UTF8);
        return Parse(text);
    }
    static List<Entry> Parse(string text) {
        var js = new System.Web.Script.Serialization.JavaScriptSerializer { MaxJsonLength = int.MaxValue };
        var root = js.Deserialize<Dictionary<string, object>>(text.TrimStart('\uFEFF'));
        object ver; root.TryGetValue("schema_version", out ver);
        if (ver == null || Convert.ToInt32(ver) != 1) throw new InvalidDataException("The mod index is in a newer form than this launcher reads. Update nativeMons.");
        var list = new List<Entry>();
        object mods; root.TryGetValue("mods", out mods);
        foreach (var o in (mods as System.Collections.IEnumerable) ?? new object[0]) {
            var m = o as Dictionary<string, object>; if (m == null) continue;
            var e = new Entry { Id = Str(m, "id"), Title = Str(m, "title"), Author = Str(m, "author"), Summary = Str(m, "summary"), Version = Str(m, "version"), Repo = Str(m, "repo"), License = Str(m, "license"), Folder = Str(m, "folder") };
            if (string.IsNullOrEmpty(e.Id) || string.IsNullOrEmpty(e.Title)) continue;
            e.Games = Strs(m, "games"); e.Permissions = Strs(m, "permissions"); e.Categories = Strs(m, "categories");
            // Its required mods, by id (a spec string is id, then @range, then #repo).
            e.Needs = Strs(m, "dependencies").Select(spec => spec.Split('@', '#')[0]).Where(id => id != "").ToList();
            e.Clashes = Strs(m, "conflicts").Select(spec => spec.Split('@', '#')[0]).Where(id => id != "").ToList();
            object x; e.Experimental = m.TryGetValue("experimental", out x) && x is bool && (bool)x;
            var latest = Obj(m, "latest"); var zip = Obj(latest, "zip");
            if (latest != null && !string.IsNullOrEmpty(Str(latest, "version"))) e.Version = Str(latest, "version");
            e.ZipUrl = Str(zip, "url") ?? Str(m, "downloadURL");
            object size; if (zip != null && zip.TryGetValue("size", out size) && size != null) e.ZipSize = Convert.ToInt64(size);
            var dl = Obj(m, "downloads"); object total; if (dl != null && dl.TryGetValue("total", out total) && total != null) e.Downloads = Convert.ToInt64(total);
            if (e.ZipUrl == null || !e.ZipUrl.StartsWith("https://", StringComparison.OrdinalIgnoreCase)) continue;   // nothing installable
            list.Add(e);
        }
        return list.OrderByDescending(e => e.Downloads).ThenBy(e => e.Title, StringComparer.OrdinalIgnoreCase).ToList();
    }
    static string Generation(string version) {
        switch (version) {
            case "red": case "blue": case "yellow": return "gen1";
            case "gold": case "silver": case "crystal": return "gen2";
            default: return "gen3";
        }
    }
    // A mod that names no games is offered everywhere (the game itself decides, as it does for any installed mod).
    public static bool Fits(Entry e, string version) {
        return e.Games.Count == 0 || e.Games.Any(x => string.Equals(x, version, StringComparison.OrdinalIgnoreCase) || string.Equals(x, Generation(version), StringComparison.OrdinalIgnoreCase));
    }
    // A repo's newest release .zip, in the engine's own updater's order. Null url when there's none.
    public static string GithubZip(string repo, string id, out long size, out string version) {
        size = 0; version = null;
        System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
        string text;
        using (var web = new System.Net.WebClient()) {
            web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons";
            web.Headers[System.Net.HttpRequestHeader.Accept] = "application/vnd.github+json";
            web.Encoding = Encoding.UTF8;
            text = web.DownloadString("https://api.github.com/repos/" + repo + "/releases?per_page=10");
        }
        var releases = new System.Web.Script.Serialization.JavaScriptSerializer { MaxJsonLength = int.MaxValue }.Deserialize<List<Dictionary<string, object>>>(text);
        foreach (var r in releases.OrderBy(x => x.ContainsKey("prerelease") && x["prerelease"] is bool && (bool)x["prerelease"] ? 1 : 0)) {
            if (r.ContainsKey("draft") && r["draft"] is bool && (bool)r["draft"]) continue;
            var assets = new List<Dictionary<string, object>>();
            object a; if (r.TryGetValue("assets", out a) && a is System.Collections.IEnumerable) foreach (var x in (System.Collections.IEnumerable)a) { var d = x as Dictionary<string, object>; if (d != null) assets.Add(d); }
            var zips = assets.Where(x => (Str(x, "name") ?? "").EndsWith(".zip", StringComparison.OrdinalIgnoreCase)).ToList();
            var pick = zips.FirstOrDefault(x => (Str(x, "name") ?? "").StartsWith(id + "-", StringComparison.OrdinalIgnoreCase)) ?? zips.FirstOrDefault();
            if (pick == null) continue;
            object sz; if (pick.TryGetValue("size", out sz) && sz != null) size = Convert.ToInt64(sz);
            version = (Str(r, "tag_name") ?? "").TrimStart('v', 'V');
            return Str(pick, "browser_download_url");
        }
        return null;
    }
    public static string PermissionWords(string p) {
        switch (p) {
            case "engine_internals": return "reaches into the game engine's own code";
            case "filesystem": return "reads and writes files";
            case "network": return "goes online";
            default: return p.Replace('_', ' ');
        }
    }
    // Downloads the release .zip as <id>.zip (the name a bare archive's folder takes) and installs it.
    public static string Install(Entry e, Action<float> progress) {
        if (e.ZipSize > MaxZip) return e.Title + " is " + (e.ZipSize / (1024 * 1024)) + " MB, larger than nativeMons installs from the index.";
        return InstallFrom(e.ZipUrl, e.Id, e.Title, e.ZipSize, progress);
    }
    // Any https link to a mod's .zip, checked to be the mod named id before it goes in.
    public static string InstallFrom(string url, string id, string title, long size, Action<float> progress) {
        string dir = Path.Combine(Config.State, "mod-downloads"), zip = Path.Combine(dir, id + ".zip");
        Directory.CreateDirectory(dir);
        try {
            if (url == null || !url.StartsWith("https://", StringComparison.OrdinalIgnoreCase)) return "Only https:// links are used.";
            System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
            var req = (System.Net.HttpWebRequest)System.Net.WebRequest.Create(url);
            req.UserAgent = "nativeMons";
            using (var resp = req.GetResponse()) using (var input = resp.GetResponseStream()) using (var output = File.Create(zip + ".part")) {
                long total = resp.ContentLength > 0 ? resp.ContentLength : size, got = 0;
                var buffer = new byte[81920]; int n;
                while ((n = input.Read(buffer, 0, buffer.Length)) > 0) {
                    got += n;
                    if (got > MaxZip) throw new IOException("the download is larger than nativeMons installs from the index");
                    output.Write(buffer, 0, n);
                    if (total > 0) progress((float)got / total);
                }
            }
            if (File.Exists(zip)) File.Delete(zip);
            File.Move(zip + ".part", zip);
            return LauncherForm.ExtractMod(zip, id);
        } catch (Exception x) { return "Couldn't install " + title + ": " + x.Message; }
        finally { try { File.Delete(zip + ".part"); } catch { } try { File.Delete(zip); } catch { } }
    }
}

// Mods that need other mods: what ModsCli's deps reports for one game, less the optional ones the player declined.
static class ModDeps {
    public class Dep { public string ModId, ModName, Id, Range, Github, Status, Have; public bool Required; }
    static string SkipFile { get { return Path.Combine(Config.State, "mod-deps-declined.txt"); } }
    static HashSet<string> Declined() {
        try { return File.Exists(SkipFile) ? new HashSet<string>(File.ReadAllLines(SkipFile).Where(x => x.Trim() != "")) : new HashSet<string>(); } catch { return new HashSet<string>(); }
    }
    public static void Decline(Dep d) {
        var all = Declined(); all.Add(d.ModId + "\t" + d.Id);
        Directory.CreateDirectory(Config.State); File.WriteAllLines(SkipFile, all.OrderBy(x => x).ToArray());
    }
    // What's still to settle for this game: missing or the wrong version, required first.
    public static List<Dep> Open(GameEntry g) {
        var declined = Declined(); var list = new List<Dep>();
        foreach (var row in (LauncherForm.Gen1Cli("deps " + g.Gen1) ?? "").Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 8 || f[6] == "ok") continue;
            var d = new Dep { ModId = f[0], ModName = f[1], Required = f[2] == "required", Id = f[3], Range = f[4], Github = f[5], Status = f[6], Have = f[7] };
            if (!d.Required && declined.Contains(d.ModId + "\t" + d.Id)) continue;
            // The same mod asked for by several: one row, the strongest ask.
            var same = list.Find(x => x.Id == d.Id);
            if (same != null) { if (d.Required && !same.Required) list[list.IndexOf(same)] = d; continue; }
            list.Add(d);
        }
        return list.OrderBy(x => x.Required ? 0 : 1).ThenBy(x => x.Id).ToList();
    }
}

// Mods that say they don't work with others (notes/launcher.md, Mods).
static class ModClashes {
    public class Clash {
        public string ModId, ModName, OtherId, OtherName, Range; public bool ModOn, OtherOn;
        public bool Active { get { return ModOn && OtherOn; } }
    }
    public static List<Clash> Read(GameEntry g) {
        var list = new List<Clash>();
        foreach (var row in (LauncherForm.Gen1Cli("conflicts " + g.Gen1) ?? "").Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 7) continue;
            list.Add(new Clash { ModId = f[0], ModName = f[1], ModOn = f[2] == "on", OtherId = f[3], OtherName = f[4], OtherOn = f[5] == "on", Range = f[6] });
        }
        return list;
    }
    // What a mod's row says about its clashes, or null.
    public static string Note(List<Clash> all, string id) {
        var mine = all.FirstOrDefault(c => c.ModId == id && c.Active) ?? all.FirstOrDefault(c => c.ModId == id);
        if (mine != null) return (mine.Active ? "Won't load: clashes with " : "Clashes with ") + mine.OtherName;
        var theirs = all.FirstOrDefault(c => c.OtherId == id && c.Active) ?? all.FirstOrDefault(c => c.OtherId == id);
        if (theirs != null) return theirs.Active ? theirs.ModName + " won't load while this is on" : theirs.ModName + " clashes with it";
        return null;
    }
}

// One dump of a game. Two files with the same contents are one dump.
class RomFile {
    public string File, Sha1; public int Revision; public DateTime Added; public bool Gen1Ok, NativeplatOk, Encrypted, EngineCopy;
    public bool Verified { get { return Gen1Ok || NativeplatOk; } }
    public string Label { get { return "Rev " + Revision; } }
}

// Unpacks a DS ROM as the Platinum port does on first launch, so its file picker never shows.
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

// The Platinum port's save.bin: a DS save with 64-bit padding (notes/launcher.md, Saves).
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
    // Ports up to v0.1.13 padded the save; v0.2.0 on use the DS format. An old save has a footer signature at 0xCF38 (as the port checks).
    public static bool IsOldPort(byte[] d) {
        foreach (int half in new[] { 0, HalfSize }) if (d.Length >= half + MainPort && BitConverter.ToUInt32(d, half + MainPort - 8) == Signature) return true;
        return false;
    }
    public static byte[] ToPort(byte[] ds) { return Convert(ds, true); }
    public static byte[] ToDs(byte[] port) { return Convert(port, false); }
}

// Each game's title screen, captured from the player's ROM on their PC. Never shipped (notes/launcher.md, Pictures).
// Runs a GBA ROM on mGBA's libretro core with no window or sound, pressing scripted buttons and saving chosen
// frames as PNGs. Used for romhacks' title screens, in a process of its own (--gba-art).
static class LibretroShots {
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate bool EnvFn(uint cmd, IntPtr data);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void VideoFn(IntPtr data, uint w, uint h, UIntPtr pitch);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void SampleFn(short l, short r);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate UIntPtr BatchFn(IntPtr data, UIntPtr frames);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void PollFn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate short StateFn(uint port, uint device, uint index, uint id);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void SetFn(IntPtr cb);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void VoidFn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate bool LoadFn(ref GameInfo info);
    [StructLayout(LayoutKind.Sequential)] struct GameInfo { public IntPtr Path, Data; public UIntPtr Size; public IntPtr Meta; }
    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)] static extern IntPtr LoadLibrary(string file);
    [DllImport("kernel32", CharSet = CharSet.Ansi)] static extern IntPtr GetProcAddress(IntPtr lib, string name);

    static int format, width, height, pitch; static byte[] frame;
    static IntPtr dirPtr;
    static int now; static Dictionary<int, HashSet<uint>> held = new Dictionary<int, HashSet<uint>>();
    static T Fn<T>(IntPtr lib, string name) where T : class {
        var p = GetProcAddress(lib, name);
        if (p == IntPtr.Zero) throw new InvalidOperationException("The core has no " + name + ".");
        return Marshal.GetDelegateForFunctionPointer(p, typeof(T)) as T;
    }
    static readonly Dictionary<string, uint> Buttons = new Dictionary<string, uint> { { "b", 0 }, { "select", 2 }, { "start", 3 }, { "up", 4 }, { "down", 5 }, { "left", 6 }, { "right", 7 }, { "a", 8 } };
    // press: frame:button[:held frames], separated by ';'. shots: the frames to save.
    public static string Run(string core, string rom, string outDir, string press, IEnumerable<int> shots, int frames) {
        foreach (var step in (press == "-" ? "" : press ?? "").Split(new[] { ';' }, StringSplitOptions.RemoveEmptyEntries)) {
            var f = step.Split(':'); int at = int.Parse(f[0]), hold = f.Length > 2 ? int.Parse(f[2]) : 6;
            for (int i = at; i < at + hold; i++) { HashSet<uint> s; if (!held.TryGetValue(i, out s)) held[i] = s = new HashSet<uint>(); s.Add(Buttons[f[1].ToLowerInvariant()]); }
        }
        var want = new HashSet<int>(shots);
        var lib = LoadLibrary(core);
        if (lib == IntPtr.Zero) return "The mGBA core couldn't be loaded.";
        Directory.CreateDirectory(outDir);
        string work = Path.Combine(outDir, ".core"); Directory.CreateDirectory(work);
        dirPtr = Marshal.StringToHGlobalAnsi(work);
        EnvFn env = Env; VideoFn video = Video; SampleFn sample = delegate { }; BatchFn batch = (d, n) => n; PollFn poll = delegate { };
        StateFn state = (port, device, index, id) => { HashSet<uint> s; return (short)(port == 0 && device == 1 && held.TryGetValue(now, out s) && s.Contains(id) ? 1 : 0); };
        var keep = new object[] { env, video, sample, batch, poll, state };
        Fn<SetFn>(lib, "retro_set_environment")(Marshal.GetFunctionPointerForDelegate(env));
        Fn<SetFn>(lib, "retro_set_video_refresh")(Marshal.GetFunctionPointerForDelegate(video));
        Fn<SetFn>(lib, "retro_set_audio_sample")(Marshal.GetFunctionPointerForDelegate(sample));
        Fn<SetFn>(lib, "retro_set_audio_sample_batch")(Marshal.GetFunctionPointerForDelegate(batch));
        Fn<SetFn>(lib, "retro_set_input_poll")(Marshal.GetFunctionPointerForDelegate(poll));
        Fn<SetFn>(lib, "retro_set_input_state")(Marshal.GetFunctionPointerForDelegate(state));
        Fn<VoidFn>(lib, "retro_init")();
        var data = File.ReadAllBytes(rom);
        var pin = GCHandle.Alloc(data, GCHandleType.Pinned);
        var path = Marshal.StringToHGlobalAnsi(rom);
        try {
            var info = new GameInfo { Path = path, Data = pin.AddrOfPinnedObject(), Size = (UIntPtr)data.Length };
            if (!Fn<LoadFn>(lib, "retro_load_game")(ref info)) return "The core wouldn't load the ROM.";
            var run = Fn<VoidFn>(lib, "retro_run");
            for (now = 0; now < frames; now++) {
                run();
                if (want.Contains(now) && frame != null) Save(Path.Combine(outDir, "f-" + now.ToString("000000") + ".png"));
            }
            Fn<VoidFn>(lib, "retro_unload_game")();
            Fn<VoidFn>(lib, "retro_deinit")();
            GC.KeepAlive(keep);
            return null;
        } finally {
            pin.Free(); Marshal.FreeHGlobal(path);
            try { Directory.Delete(work, true); } catch { }
        }
    }
    static bool Env(uint cmd, IntPtr data) {
        switch (cmd) {
            case 3: Marshal.WriteByte(data, 1); return true;                      // can dupe frames
            case 9: case 31: Marshal.WriteIntPtr(data, dirPtr); return true;      // system and save folders
            case 10: format = Marshal.ReadInt32(data); return format == 1 || format == 2 || format == 0;
            case 16: return true;                                                 // variables: defaults are fine
            case 17: Marshal.WriteByte(data, 0); return true;                     // no variable updates
            default: return false;
        }
    }
    static void Video(IntPtr data, uint w, uint h, UIntPtr p) {
        if (data == IntPtr.Zero) return;
        width = (int)w; height = (int)h; pitch = (int)p;
        if (frame == null || frame.Length != pitch * height) frame = new byte[pitch * height];
        Marshal.Copy(data, frame, 0, frame.Length);
    }
    static void Save(string file) {
        using (var bmp = new Bitmap(width, height, System.Drawing.Imaging.PixelFormat.Format32bppRgb)) {
            var lk = bmp.LockBits(new Rectangle(0, 0, width, height), System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
            var row = new byte[width * 4];
            for (int y = 0; y < height; y++) {
                int o = y * pitch;
                for (int x = 0; x < width; x++) {
                    int r, g, b;
                    if (format == 1) { b = frame[o + x * 4]; g = frame[o + x * 4 + 1]; r = frame[o + x * 4 + 2]; }
                    else {
                        int v = frame[o + x * 2] | frame[o + x * 2 + 1] << 8;
                        if (format == 2) { r = (v >> 11 & 31) * 255 / 31; g = (v >> 5 & 63) * 255 / 63; b = (v & 31) * 255 / 31; }
                        else { r = (v >> 10 & 31) * 255 / 31; g = (v >> 5 & 31) * 255 / 31; b = (v & 31) * 255 / 31; }
                    }
                    row[x * 4] = (byte)b; row[x * 4 + 1] = (byte)g; row[x * 4 + 2] = (byte)r; row[x * 4 + 3] = 255;
                }
                Marshal.Copy(row, 0, lk.Scan0 + y * lk.Stride, row.Length);
            }
            bmp.UnlockBits(lk);
            bmp.Save(file, System.Drawing.Imaging.ImageFormat.Png);
        }
    }
}

static class GameArt {
    const int Count = 48, Step = 6;   // ten pictures a second, for just under five seconds
    static readonly object gate = new object();
    static string loadedId; static Bitmap[] loaded; static Bitmap loadedLogo;
    static string Folder(string id) { return Path.Combine(Config.State, "game-art", id); }
    static string FrameFile(string id, int i) { return Path.Combine(Folder(id), "frame-" + i.ToString("00") + ".jpg"); }
    static string LogoFile(string id) { return Path.Combine(Folder(id), "logo.png"); }
    public static bool Has(string id) { return id != null && File.Exists(FrameFile(id, Count - 1)); }
    // A romhack shows its base game's pictures until it has its own.
    public static volatile Dictionary<string, string> BaseOf = new Dictionary<string, string>();
    static string Shown(string id, Func<string, bool> has) { string b; return id == null || has(id) || !BaseOf.TryGetValue(id, out b) || !has(b) ? id : b; }
    // The home screen's wallpaper: one of the game's towns.
    static string TownFile(string id) { return Path.Combine(Folder(id), "town.jpg"); }
    public static bool HasTown(string id) { return id != null && File.Exists(TownFile(id)); }
    // Per town (games.json townView): the point of the picture to centre between the Play card and the apps, and a zoom
    // where 1 shows its whole width. A hack without its own uses its base game's.
    public static readonly Dictionary<string, float[]> TownViews = new Dictionary<string, float[]>();
    static float[] TownView(string id) {
        float[] v; string b;
        if (id != null && TownViews.TryGetValue(id, out v)) return v;
        if (id != null && BaseOf.TryGetValue(id, out b) && TownViews.TryGetValue(b, out v)) return v;
        return new[] { 0.5f, 0.5f, 1f };
    }
    static string framedId; static Bitmap framedSoft, framedBlur;
    // The town framed in band; where it doesn't reach the screen's edges, a blurred, darker copy fills in behind.
    public static bool DrawTown(Graphics g, string gameId, RectangleF screen, RectangleF band) {
        var pic = Town(gameId);
        if (pic == null) return false;
        // A wide window has no open band between them: the town just fills it.
        if (screen.Width > screen.Height || band.Height < screen.Height * 0.08f) { Draw(g, pic, screen); return true; }
        string id = Shown(gameId, HasTown);
        var view = TownView(id);
        lock (gate) if (framedId != id) {
            if (framedSoft != null) framedSoft.Dispose(); if (framedBlur != null) framedBlur.Dispose();
            framedSoft = Feathered(pic); framedBlur = Blurred(pic); framedId = id;
        }
        float scale = screen.Width / pic.Width * Math.Max(1f, view[2]), dw = pic.Width * scale, dh = pic.Height * scale;
        float x = Math.Max(screen.Right - dw, Math.Min(screen.X, band.X + band.Width / 2 - view[0] * dw));
        float y = band.Y + band.Height / 2 - view[1] * dh;
        if (dh >= screen.Height) y = Math.Max(screen.Bottom - dh, Math.Min(screen.Y, y));
        var st = g.Save();
        if (y > screen.Y || y + dh < screen.Bottom) {
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBilinear;
            float cover = Math.Max(screen.Width / framedBlur.Width, screen.Height / framedBlur.Height) * 1.1f;
            g.DrawImage(framedBlur, screen.X + (screen.Width - framedBlur.Width * cover) / 2, screen.Y + (screen.Height - framedBlur.Height * cover) / 2, framedBlur.Width * cover, framedBlur.Height * cover);
            using (var b = new SolidBrush(Color.FromArgb(90, 0, 0, 0))) g.FillRectangle(b, screen);
        }
        g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
        g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;
        g.DrawImage(dh >= screen.Height ? pic : framedSoft, x, y, dw, dh);
        g.Restore(st);
        return true;
    }
    // Its top and bottom fade out, so the sharp picture meets the blurred one softly.
    static Bitmap Feathered(Bitmap pic) {
        var b = new Bitmap(pic.Width, pic.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(b)) g.DrawImage(pic, 0, 0, pic.Width, pic.Height);
        var data = b.LockBits(new Rectangle(0, 0, b.Width, b.Height), System.Drawing.Imaging.ImageLockMode.ReadWrite, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        var px = new byte[data.Stride * b.Height];
        System.Runtime.InteropServices.Marshal.Copy(data.Scan0, px, 0, px.Length);
        int edge = Math.Max(1, b.Height / 10);
        for (int row = 0; row < b.Height; row++) {
            int d = Math.Min(row, b.Height - 1 - row);
            if (d >= edge) continue;
            float k = d / (float)edge; byte a = (byte)(255 * k * k * (3 - 2 * k));
            for (int col = 0; col < b.Width; col++) px[row * data.Stride + col * 4 + 3] = a;
        }
        System.Runtime.InteropServices.Marshal.Copy(px, 0, data.Scan0, px.Length);
        b.UnlockBits(data);
        return b;
    }
    static Bitmap Blurred(Bitmap pic) {
        var small = new Bitmap(Math.Max(4, pic.Width / 24), Math.Max(4, pic.Height / 24));
        using (var g = Graphics.FromImage(small)) { g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBilinear; g.DrawImage(pic, 0, 0, small.Width, small.Height); }
        return small;
    }
    static string loadedTownId; static Bitmap loadedTown;
    public static Bitmap Town(string id) {
        id = Shown(id, HasTown);
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
    // A game nativeplat can show but doesn't run for the player names its core as townCore: its wallpaper comes from there.
    public static bool CanCaptureTown(GameEntry g) { return g.Pc != null ? g.OnPc && !PcGames.IsCustom(g) :((g.TownFrame > 0 && g.FulldexRom != null && File.Exists(Config.Fulldex) && !Library.NeedsPatch(g.FulldexRom)) || g.OnNativeplat || (g.TownCore != null && g.FulldexRom != null && File.Exists(Config.Nativeplat) && !Library.NeedsPatch(g.FulldexRom)) || (g.Gen1 != null && g.Gen1Rom != null)); }
    // Towns from nativeplat's test mode, which never uses the player's saves (notes/launcher.md, Pictures).
    const string DpNewGame = "1250:start;1350:a:4:40:35;2800:down;2820:down;2840:a;2900:a:4:40:14;3560:tap:128:100:8:40:3;3700:a:4:40:49;5850:tap:82:118:6;6600:b:4:40:6;6950:down;6980:a;7040:a:4:40:40;9300:a:4:40:120";
    const string HgNewGame = "1300:start;1700:start;2100:a:4:40:10;3100:tap:220:170:4:80:30;6100:tap:128:72:4;6500:a:4;6800:tap:128:147:4;7200:tap:220:170:4:80:50;11500:tap:70:140:4;11900:tap:220:180:4:80:3;12500:tap:60:80:4;12800:tap:220:180:4;13200:tap:190:55:4;13500:tap:220:180:4:80:3;14000:start:4;14100:a:4;14500:tap:220:180:4;14800:tap:190:55:4;15100:tap:220:180:4:80:20";
    const string Twinleaf = "411", TwinleafX = "110", TwinleafZ = "888";
    static int RunNativeplatTest(string spec, string lab, string warp) {
        var psi = new ProcessStartInfo(Config.Nativeplat) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(Config.Nativeplat), RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        psi.EnvironmentVariables["SDL_VIDEO_DRIVER"] = "dummy";
        psi.EnvironmentVariables["SDL_AUDIO_DRIVER"] = "dummy";
        psi.EnvironmentVariables["NP_AUTOTEST"] = spec;
        if (lab != null) { psi.EnvironmentVariables["PC_LAB"] = lab; psi.EnvironmentVariables["PC_LAB_AT"] = "300"; }
        if (warp != null) psi.EnvironmentVariables["PC_WARP"] = warp;
        using (var p = Process.Start(psi)) { Watch(p);
            try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
            p.BeginOutputReadLine(); p.BeginErrorReadLine();
            if (!p.WaitForExit(600000)) { try { p.Kill(); } catch { } return -1; }
            return p.ExitCode;
        }
    }
    // Generation 5's opening flies over the home town, at a frame set in games.json.
    static string CaptureTownOpening(GameEntry g) {
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-town"), script = temp + ".txt";
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            var presses = new List<string>();
            for (int f = 1500; f < g.TownFrame; f += 60) presses.Add(f + " " + (f + 6) + " A");
            File.WriteAllLines(script, presses.ToArray());
            string dir = Config.FulldexDir;
            string args = "\"" + g.FulldexRom + "\" --no-save --config \"" + Path.Combine(dir, "settings.cfg") + "\" --jit --headless --compose 960x720 --input \"" + script + "\" --shot-dir \"" + temp + "\" --shot-at " + g.TownFrame + " --frames " + (g.TownFrame + 10) + " --funcs \"" + Path.Combine(dir, "data", "funcs", g.FulldexCode + ".txt") + "\"";
            var psi = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            using (var p = Process.Start(psi)) { Watch(p);
                try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
                p.BeginOutputReadLine(); p.BeginErrorReadLine();
                if (!p.WaitForExit(240000)) { try { p.Kill(); } catch { } return "It took too long."; }
            }
            var shot = Directory.GetFiles(temp, "*.bmp").FirstOrDefault();
            return shot == null ? "No picture came out." : SaveTown(g, shot);
        } catch (Exception x) { return x.Message; }
        finally {
            try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { }
            try { if (File.Exists(script)) File.Delete(script); } catch { }
        }
    }
    static string CaptureTownNativeplat(GameEntry g) {
        string shot = Path.Combine(Config.State, "game-art", g.Id + "-town.png"), root = Path.Combine(Path.GetDirectoryName(Config.Nativeplat), "userdata");
        string slotName = "nativemons-wallpaper", slot = Path.Combine(root, "saves", g.TownCore ?? g.Nativeplat, slotName + ".sav"), script = shot + ".press";
        string romCopy = shot + ".nds", core = g.TownCore ?? g.Nativeplat;
        string view = "size=1024x768,layout=top,zoom=1024,game=" + core + ",png=" + shot + ",rom=" + (g.TownCore != null ? romCopy : LauncherForm.NativeplatRomCopy(g));
        try {
            Directory.CreateDirectory(Path.GetDirectoryName(shot));
            if (File.Exists(shot)) File.Delete(shot);
            if (g.TownCore != null) {
                // New Bark Town, from a new game on a copy of the player's ROM. The ROM stays as it is.
                File.Copy(g.FulldexRom, romCopy, true);
                File.SetAttributes(romCopy, FileAttributes.Normal);
                File.WriteAllText(script, HgNewGame.Replace(";", Environment.NewLine));
                RunNativeplatTest("frames=18500," + view + ",press=@" + script, null, "60:686:393:1");
                if (!File.Exists(shot)) return "No picture came out.";
                return SaveTown(g, shot, new RectangleF(0.02f, 0.24f, 0.76f, 0.64f));
            }
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
            foreach (var f in new[] { shot, script, romCopy, slot, slot + ".bak", slot + ".tmp", Path.Combine(root, "options-autotest.ini") }) try { if (File.Exists(f)) File.Delete(f); } catch { }
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
    // Always under the throwaway identity: the new game this needs must never meet a real save.
    public static string CaptureTown(GameEntry g) {
        if (g.Pc != null) return CapturePcTown(g);
        if (g.TownFrame > 0) return CaptureTownOpening(g);
        if (g.IsDs) return CaptureTownNativeplat(g);
        string shot = Path.Combine(Config.State, "game-art", g.Id + "-town.png");
        try {
            Directory.CreateDirectory(Path.GetDirectoryName(shot));
            if (File.Exists(shot)) File.Delete(shot);
            var import = LauncherForm.Gen1Start("");
            LauncherForm.UseArtIdentity(import, ArtIdentity);
            import.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
            import.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
            int code;
            using (var rom = new LauncherForm.PlainPath(g.Gen1Rom)) {
                import.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = rom.Path_;
                LauncherForm.RunCapture(import, 240, out code);
            }
            if (code != 0) return "Could not prepare the game for its picture.";
            var psi = LauncherForm.Gen1Start("");
            LauncherForm.UseArtIdentity(psi, ArtIdentity);
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
            LauncherForm.RemoveArtData(ArtIdentity);
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
        id = Shown(id, Has);
        if (!Has(id)) return null;
        lock (gate) {
            try { Load(id); } catch { return null; }
            int n = (int)(t * 10) % (Count * 2 - 2);
            return loaded[n < Count ? n : Count * 2 - 2 - n];
        }
    }
    public static Bitmap Logo(string id) {
        id = Shown(id, Has);
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
    // Widescreen shows both screens. The logo is the half that's the same in every picture.
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
    // A PC game's title screen and town come from its own pictures, put together here, not from running it.
    static string PcGraphics(GameEntry g) { return Path.Combine(PcGames.Dir(g), "Graphics"); }
    static string CapturePc(GameEntry g) {
        try {
            string gr = PcGraphics(g);
            var pf = PcGames.Of(g);
            var layers = pf.TitleBg.Select(f => new Bitmap(Path.Combine(gr, f))).ToList();
            try {
            using (var swirl = new Bitmap(Path.Combine(gr, pf.TitleSwirl)))
            using (var logo = new Bitmap(Path.Combine(gr, pf.TitleLogo))) {
                Directory.CreateDirectory(Folder(g.Id));
                var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
                var quality = new System.Drawing.Imaging.EncoderParameters(1);
                quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 88L);
                int w = layers[0].Width, h = layers[0].Height;
                float lw = w * 0.62f, lh = lw * logo.Height / logo.Width;
                for (int i = 0; i < Count; i++) {
                    // Played there and back, so the swirl sways a little behind the logo and the light breathes.
                    float k = (float)i / (Count - 1), sway = -7f + 14f * k, glow = 0.82f + 0.18f * (float)Math.Sin(k * Math.PI);
                    using (var frame = new Bitmap(w, h))
                    using (var f = Graphics.FromImage(frame)) {
                        f.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
                        for (int n = 0; n < layers.Count; n++) {
                            var b = layers[n];
                            // The drifting layer (clouds) slides along with the frames; the others fill the picture.
                            if (n == pf.Drift) f.DrawImage(b, -(b.Width - w) * k, 0f, b.Width, h);
                            else f.DrawImage(b, 0, 0, w, h);
                        }
                        var st = f.Save();
                        f.TranslateTransform(w / 2f, h * 0.47f); f.RotateTransform(sway);
                        var cm = new System.Drawing.Imaging.ColorMatrix { Matrix33 = glow };
                        using (var ia = new System.Drawing.Imaging.ImageAttributes()) {
                            ia.SetColorMatrix(cm);
                            f.DrawImage(swirl, new Rectangle((int)(-lw * 0.5f), (int)(-lh * 0.5f), (int)lw, (int)lh), 0, 0, swirl.Width, swirl.Height, GraphicsUnit.Pixel, ia);
                        }
                        f.Restore(st);
                        f.DrawImage(logo, (w - lw) / 2f, h * 0.47f - lh / 2f, lw, lh);
                        frame.Save(FrameFile(g.Id, i), jpeg, quality);
                    }
                }
            }
            } finally { foreach (var b in layers) b.Dispose(); }
            return null;
        } catch (Exception x) { return x.Message; }
    }
    static string CapturePcTown(GameEntry g) {
        try {
            Directory.CreateDirectory(Folder(g.Id));
            using (var map = new Bitmap(Path.Combine(PcGraphics(g), PcGames.Of(g).Town))) {
                var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
                var quality = new System.Drawing.Imaging.EncoderParameters(1);
                quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 90L);
                map.Save(TownFile(g.Id), jpeg, quality);
            }
            return null;
        } catch (Exception x) { return x.Message; }
    }
    public static bool CanCapture(GameEntry g) {
        // Added PC games go without a captured title screen or wallpaper.
        if (g.Pc != null) return g.OnPc && !PcGames.IsCustom(g);
        if (g.IsDs && g.FulldexCode == null) return g.OnNativeplat;
        if (g.IsDs) return g.ArtFrame > 0 && g.FulldexRom != null && File.Exists(Config.Fulldex) && (!Library.NeedsPatch(g.FulldexRom) || File.Exists(Config.Patcher));
        if (g.IsHack) return g.Roms.Count > 0 && File.Exists(Config.MgbaCore);
        return g.Gen1 != null && (g.Imported || g.Gen1Rom != null);
    }
    // Game Boy and GBA title screens, from a hidden, silent Gen1Recomp window (notes/launcher.md, Pictures).
    const string ArtIdentity = "nativemons-art";
    static string CaptureGen1(GameEntry g) {
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots");
        bool throwaway = !g.Imported;
        try {
            if (throwaway) {
                var import = LauncherForm.Gen1Start("");
                LauncherForm.UseArtIdentity(import, ArtIdentity);
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
            if (throwaway) LauncherForm.UseArtIdentity(psi, ArtIdentity);
            psi.EnvironmentVariables["POKEPORT_VERSION"] = g.Gen1;
            psi.EnvironmentVariables["POKEPORT_BACKGROUND"] = "1";
            psi.EnvironmentVariables["POKEPORT_DRIVER"] = "src/nativemons/TitleArt.lua";
            psi.EnvironmentVariables["NATIVEMONS_TITLE_ART"] = temp.Replace('\\', '/');
            using (var p = Process.Start(psi)) { Watch(p);
                try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
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
            if (throwaway) LauncherForm.RemoveArtData(ArtIdentity);
        }
    }
    // A GBA romhack's title screen, from mGBA in a process of its own (--gba-art), on a copy of the ROM.
    // START skips the opening; the title screen shows after it (notes/launcher.md, Pictures).
    const int GbaPress = 1000, GbaFirstShot = 1500;
    static string CaptureGba(GameEntry g) {
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots"), rom = temp + ".gba";
        if (Library.IsUltraVioletPatch((g.Chosen ?? g.Roms[0]).File)) return "A patch, not a ROM: no title screen to capture.";
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            File.Copy((g.Chosen ?? g.Roms[0]).File, rom, true);
            File.SetAttributes(rom, FileAttributes.Normal);
            var at = Enumerable.Range(0, Count).Select(i => (GbaFirstShot + i * Step).ToString()).ToArray();
            string args = "--gba-art \"" + rom + "\" \"" + temp + "\" " + (GbaFirstShot + Count * Step + 2) + " " + GbaPress + ":start " + string.Join(",", at);
            var psi = new ProcessStartInfo(Application.ExecutablePath, args) { UseShellExecute = false, CreateNoWindow = true };
            using (var p = Process.Start(psi)) { Watch(p);
                try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
                if (!p.WaitForExit(180000)) { try { p.Kill(); } catch { } return "It took too long."; }
            }
            var shots = Directory.GetFiles(temp, "f-*.png").OrderBy(x => x, StringComparer.Ordinal).ToArray();
            if (shots.Length != Count) return "Only " + shots.Length + " pictures came out.";
            Directory.CreateDirectory(Folder(g.Id));
            var jpeg = System.Drawing.Imaging.ImageCodecInfo.GetImageEncoders().First(c => c.MimeType == "image/jpeg");
            var quality = new System.Drawing.Imaging.EncoderParameters(1);
            quality.Param[0] = new System.Drawing.Imaging.EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 90L);
            for (int i = 0; i < Count; i++) using (var pic = new Bitmap(shots[i])) pic.Save(FrameFile(g.Id, i), jpeg, quality);
            return null;
        } catch (Exception x) { return x.Message; }
        finally {
            try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { }
            try { if (File.Exists(rom)) File.Delete(rom); } catch { }
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
            using (var p = Process.Start(psi)) { Watch(p);
                try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
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
        if (g.Pc != null) return CapturePc(g);
        if (!g.IsDs && g.IsHack) return CaptureGba(g);
        if (!g.IsDs) return CaptureGen1(g);
        if (g.FulldexCode == null) return CaptureNativeplat(g);
        string temp = Path.Combine(Config.State, "game-art", g.Id + "-shots"), patched = temp + "-rom";
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            Directory.CreateDirectory(temp);
            string dir = Config.FulldexDir, romFile = g.FulldexRom;
            // An encrypted ROM won't start. Its picture comes from a patched copy, and the player's file isn't touched.
            if (Library.NeedsPatch(romFile)) {
                if (!PythonSetup.Works()) return "Its picture needs the ROM patched, and that needs Python 3.";
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
            using (var p = Process.Start(psi)) { Watch(p);
                try { p.PriorityClass = Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
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
    public static volatile bool Capturing, Hold;
    [ThreadStatic] public static bool OnCaptureThread;
    static volatile Process current;
    public static void Watch(Process p) { current = p; if (Hold) StopNow(); }
    // Nothing a capture makes is kept unless it finished, so stopping one costs only a retry at the next start.
    public static void StopNow() { Hold = true; try { var p = current; if (p != null) p.Kill(); } catch { } }
    public static void CaptureMissing(Func<bool> busy, Action first) {
        Background(delegate { first(); CaptureAll(() => Hold || busy()); });
    }
    public static void Background(Action work) {
        lock (BackgroundLock) {
            Hold = false; Capturing = true; OnCaptureThread = true;
            try { work(); } catch { } finally { Capturing = false; OnCaptureThread = false; current = null; }
        }
    }
    static readonly object BackgroundLock = new object();
    // At the start, behind the splash: at normal priority, since nothing else is happening yet.
    public static volatile bool Eager;
    class Job { public GameEntry Game; public bool Town; }
    static List<Job> Missing(GameEntry first) {
        var jobs = Library.Games.Where(x => CanCapture(x) && !Has(x.Id) && !Failed(x, false)).Select(x => new Job { Game = x })
            .Concat(Library.Games.Where(x => CanCaptureTown(x) && !HasTown(x.Id) && !Failed(x, true)).Select(x => new Job { Game = x, Town = true }));
        // The game the launcher opens on comes first, so its screen is ready even if the rest is stopped.
        return jobs.OrderBy(j => j.Game == first ? 0 : 1).ToList();
    }
    // The game whose title screen is being made now, so its Play tile shows the loading Poke Ball meanwhile.
    public static volatile string Making;
    static string Run(Job j) {
        if (!j.Town) Making = j.Game.Id;
        try {
            string why = j.Town ? CaptureTown(j.Game) : Capture(j.Game);
            if (why != null && !Hold) NoteFailed(j.Game, j.Town);
            return why;
        } finally { Making = null; }
    }
    public static void CaptureAtStart(GameEntry first, Action<int, int> progress) {
        Background(delegate {
            // Only the game the menu opens on: the rest take minutes, so they're made in the background once the menu is up.
            var jobs = Missing(first).Where(j => j.Game == first).ToList();
            Eager = true;
            try { for (int i = 0; i < jobs.Count && !Hold; i++) { progress(i, jobs.Count); Run(jobs[i]); } }
            finally { Eager = false; }
        });
    }
    static void CaptureAll(Func<bool> busy) {
        foreach (var j in Missing(null)) {
            if (busy()) return;
            Run(j);
        }
    }
    // A picture that can't be made isn't tried at every start: only after the ROM or the launcher changes.
    static string FailedFile { get { return Path.Combine(Config.State, "game-art", "failed.txt"); } }
    static string FailKey(GameEntry g, bool town) {
        var parts = new List<string> { g.Id, town ? "town" : "title" };
        foreach (var rom in new[] { g.FulldexRom, g.Gen1Rom }) if (rom != null) try { var f = new FileInfo(rom); parts.Add(rom + "|" + f.Length + "|" + f.LastWriteTimeUtc.Ticks); } catch { }
        try { parts.Add(File.GetLastWriteTimeUtc(System.Reflection.Assembly.GetExecutingAssembly().Location).Ticks.ToString()); } catch { }
        return string.Join("|", parts.ToArray());
    }
    static bool Failed(GameEntry g, bool town) {
        try { return File.Exists(FailedFile) && File.ReadAllLines(FailedFile).Contains(FailKey(g, town)); } catch { return false; }
    }
    static void NoteFailed(GameEntry g, bool town) {
        try { Directory.CreateDirectory(Path.GetDirectoryName(FailedFile)); File.AppendAllText(FailedFile, FailKey(g, town) + Environment.NewLine); } catch { }
    }
}

// A default for all games in Settings, which each game follows or overrides (notes/launcher.md, Settings).
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
            var e = new GameEntry { Id = str(d, "id"), Title = str(d, "title"), Console = str(d, "console"), Gen1 = str(d, "gen1"), FulldexCode = str(d, "fulldex"), Header = str(d, "header"), Save = str(d, "save"), Port = str(d, "port"), Nativeplat = str(d, "nativeplat"), TownCore = str(d, "townCore"), Pc = str(d, "pc") };
            object c, dx;
            if (d.TryGetValue("dex", out dx)) e.Dex = Convert.ToInt32(dx);
            object af; if (d.TryGetValue("artFrame", out af)) e.ArtFrame = Convert.ToInt32(af);
            object tf; if (d.TryGetValue("townFrame", out tf)) e.TownFrame = Convert.ToInt32(tf);
            object tv; if (d.TryGetValue("townView", out tv)) GameArt.TownViews[e.Id] = ((System.Collections.IEnumerable)tv).Cast<object>().Select(Convert.ToSingle).ToArray();
            object hk; if (d.TryGetValue("hacks", out hk)) foreach (var o in (System.Collections.IEnumerable)hk) {
                var k = o as Dictionary<string, object>;
                if (k != null && str(k, "id") != null && str(k, "title") != null && k.ContainsKey("sha1"))
                    foreach (var s in ((System.Collections.IEnumerable)k["sha1"]).Cast<object>()) e.KnownHacks[(string)s] = new[] { str(k, "id"), str(k, "title") };
            }
            if (d.TryGetValue("colors", out c)) {
                var hex = ((System.Collections.IEnumerable)c).Cast<object>().Select(x => (string)x).ToArray();
                if (hex.Length == 3) { e.Theme = Theme.Make(hex[0], hex[1], hex[2], str(d, "finish")); e.Theme.GameId = e.Id; }
            }
            list.Add(e);
        }
        // PC games the player added (CustomGames), after the built-in ones.
        int added = 0;
        foreach (var c in CustomGames.All()) if (!list.Any(x => x.Id == c.id)) list.Add(CustomGames.ToGame(c, added++));
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
        LoadHashes();
        var folders = Config.RomFolders();
        folders.Add(Path.Combine(Config.FulldexDir ?? Config.Here, "roms"));
        folders.Add(Config.Unzipped);
        var files = Config.RomFiles().Where(File.Exists).ToList();
        foreach (var folder in folders) if (Directory.Exists(folder)) files.AddRange(FilesUnder(folder));
        foreach (var file in files) {
            if (!IsRom(file)) { if (IsUltraVioletPatch(file)) try { AddPatch(list, file); } catch { } continue; }
            try { Sort(list, file, Path.GetExtension(file).ToLowerInvariant(), accepted); } catch { }
        }
        SaveHashes();
        list = Hacks(list);
        GameArt.BaseOf = list.Where(x => x.IsHack).ToDictionary(x => x.Id, x => x.BaseId);
        foreach (var g in list) ChooseRom(g);
        Games = list;
    }
    // Romhacks become games of their own, after their base game (notes/launcher.md, ROMs and romhacks).
    static List<GameEntry> Hacks(List<GameEntry> list) {
        var result = new List<GameEntry>();
        foreach (var g in list) {
            result.Add(g);
            if (g.FulldexCode == null || g.Pc != null) continue;
            // Where no engine knows the clean dumps, only hacks known by their hash are told apart.
            bool clean = g.CanVerify && !(g.Gen1 != null && !Gen1Found);
            foreach (var r in g.Roms.Where(r => !r.Encrypted && (IsUltraViolet(r.File) || (clean ? !r.Verified : g.KnownHacks.ContainsKey(r.Sha1 ?? "")))).ToList()) {
                g.Roms.Remove(r);
                result.Add(HackOf(g, r, list.Concat(result)));
            }
        }
        return result;
    }
    // What a file name says besides its base game, such as "Rocket Edition". Tags, region and version words don't count.
    static readonly string[] PlainWords = { "pokemon", "pokémon", "version", "usa", "europe", "eur", "us", "u", "e", "j", "en", "rev", "v1", "v0", "rom", "the" };
    public static string HackWords(GameEntry g, string file) {
        string name = System.Text.RegularExpressions.Regex.Replace(Path.GetFileNameWithoutExtension(file), @"[\(\[\{][^\)\]\}]*[\)\]\}]", " ");
        var baseWords = new HashSet<string>(g.Title.ToLowerInvariant().Split(' ').Concat(new[] { g.Id }));
        var words = System.Text.RegularExpressions.Regex.Split(name, @"[^\p{L}\p{N}']+").Where(w => w != "").ToList();
        var kept = words.Where(w => !PlainWords.Contains(w.ToLowerInvariant()) && !baseWords.Contains(w.ToLowerInvariant()) && !System.Text.RegularExpressions.Regex.IsMatch(w, @"^\d+$")).ToList();
        return string.Join(" ", kept.ToArray());
    }
    static string HackNamesFile { get { return Path.Combine(Config.State, "hack-names.txt"); } }
    // The player's names for hacks, by SHA-1: a hack keeps its name whatever its file is called.
    public static Dictionary<string, string> HackNames() {
        var d = new Dictionary<string, string>();
        try { if (File.Exists(HackNamesFile)) foreach (var l in File.ReadAllLines(HackNamesFile)) { var f = l.Split('\t'); if (f.Length == 2 && f[1].Trim() != "") d[f[0]] = f[1].Trim(); } } catch { }
        return d;
    }
    public static void RenameHack(string sha1, string name) {
        var d = HackNames(); d[sha1] = name;
        Directory.CreateDirectory(Config.State);
        File.WriteAllLines(HackNamesFile, d.Select(x => x.Key + "\t" + x.Value).ToArray());
    }
    // Fulldex names a GBA game's save after its ROM file: lower case, every run of other characters one dash.
    public static string FulldexKey(string file) {
        return System.Text.RegularExpressions.Regex.Replace(Path.GetFileNameWithoutExtension(file).ToLowerInvariant(), "[^a-z0-9]+", "-").Trim('-');
    }
    static GameEntry HackOf(GameEntry b, RomFile r, IEnumerable<GameEntry> taken) {
        var h = b.CopyOf();
        string words = HackWords(b, r.File), own; string[] known;
        string title = words != "" ? b.Title + " " + words : b.Title + " (modified " + r.Sha1.Substring(0, 6) + ")";
        // A known build is named by its hash, whatever its file is called. The player's own name still wins.
        bool isKnown = b.KnownHacks.TryGetValue(r.Sha1, out known);
        if (isKnown) title = known[1];
        bool uv = !b.IsDs && IsUltraViolet(r.File);
        if (uv && !isKnown) title = "Pokemon Ultra Violet";
        if (HackNames().TryGetValue(r.Sha1, out own)) title = own;
        // Otherwise the id comes from the file's name, as the game's save does, so renaming it here changes neither.
        string slug = System.Text.RegularExpressions.Regex.Replace((words != "" ? words : r.Sha1.Substring(0, 8)).ToLowerInvariant(), "[^a-z0-9]+", "-").Trim('-');
        if (isKnown) slug = known[0].StartsWith(b.Id + "-") ? known[0].Substring(b.Id.Length + 1) : known[0];
        string id = b.Id + "-" + (slug == "" ? r.Sha1.Substring(0, 8) : slug);
        var used = taken.Select(x => x.Id).ToList();
        for (int i = 2; used.Contains(id); i++) id = b.Id + "-" + slug + "-" + i;
        h.Id = id; h.Title = title; h.BaseId = b.Id;
        h.Gen1 = null; h.Nativeplat = null; h.Port = null; h.Pc = null; h.Imported = false;
        h.Gen1Rom = null; h.FulldexRom = null; h.NativeplatRom = null; h.FoundUnsupported = null; h.FoundSha1 = null; h.Chosen = null;
        h.Roms = new List<RomFile> { r };
        // Its save is the one Fulldex already gives it (a GBA game's is named after its file), so nothing moves.
        h.Save = b.IsDs ? id + ".sav" : uv ? "ultraviolet.sav" : FulldexKey(r.File) + ".sav";
        h.Theme = b.Theme.Copy(); h.Theme.GameId = id;
        return h;
    }
    // Ultra Violet (a FireRed 1.0 hack), told by its file's name as Fulldex does; saved as ultraviolet.sav there.
    public static bool IsUltraViolet(string file) {
        return System.Text.RegularExpressions.Regex.Replace(Path.GetFileNameWithoutExtension(file).ToLowerInvariant(), "[^a-z0-9]", "").Contains("ultraviolet");
    }
    public static bool IsUltraVioletPatch(string file) { return Path.GetExtension(file).ToLowerInvariant() == ".ips" && IsUltraViolet(file); }
    // Its .ips plays over FireRed 1.0 (the GBA player's --base), so it is listed with FireRed's dumps until Hacks splits it off.
    static void AddPatch(List<GameEntry> list, string file) {
        var g = list.Find(x => x.Header == "BPRE");
        if (g == null) return;
        string sha = Sha1Cached(file);
        if (g.Roms.Any(x => x.Sha1 == sha)) return;
        g.Roms.Add(new RomFile { File = file, Sha1 = sha, Added = File.GetCreationTimeUtc(file) });
    }
    public static bool IsRom(string file) { string ext = Path.GetExtension(file).ToLowerInvariant(); return ext == ".nds" || ext == ".gb" || ext == ".gbc" || ext == ".gba"; }
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
        if (g.Pc == "custom") return "Its program has moved or been deleted.";
        if (g.Pc != null) return "Not installed yet. Select it to install.";
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
    // nativeplat takes Platinum Rev 0 too (notes/launcher.md, ROMs and romhacks).
    static readonly Dictionary<string, string[]> NativeplatDumps = new Dictionary<string, string[]> {
        { "diamond", new[] { "a46233d8b79a69ea87aa295a0efad5237d02841e" } }, { "pearl", new[] { "99083bf15ec7c6b81b4ba241ee10abd9e80999ac" } },
        { "platinum", new[] { "0862ec35b24de5c7e2dcb88c9eea0873110d755c", "ce81046eda7d232513069519cb2085349896dec7" } },
    };
    static void Sort(List<GameEntry> list, string file, string ext, Dictionary<string, string> accepted) {
        string header = HeaderOf(file, ext);
        var g = list.Find(x => x.Header != null && (ext == ".gb" || ext == ".gbc" ? header.StartsWith(x.Header) && x.Gen1 != null : header == x.Header));
        if (g == null) return;
        string sha = Sha1Cached(file), version;
        string[] wanted;
        string engineRoms = Path.Combine(Config.FulldexDir ?? Config.Here, "roms") + Path.DirectorySeparatorChar;
        var rom = new RomFile { File = file, Sha1 = sha, Revision = RevisionOf(file, ext), Added = File.GetCreationTimeUtc(file), Encrypted = NeedsPatch(file), EngineCopy = file.StartsWith(engineRoms, StringComparison.OrdinalIgnoreCase) };
        rom.Gen1Ok = g.Gen1 != null && accepted.TryGetValue(sha, out version) && version == g.Gen1;
        rom.NativeplatOk = g.Nativeplat != null && NativeplatDumps.TryGetValue(g.Nativeplat, out wanted) && wanted.Contains(sha);
        // The same contents twice is one dump: the player's own file, not a copy kept in an engine's folder.
        int same = g.Roms.FindIndex(x => x.Sha1 == sha);
        if (same < 0) g.Roms.Add(rom);
        else if (g.Roms[same].EngineCopy && !rom.EngineCopy) g.Roms[same] = rom;
    }
    // Verified dumps first, then ready to run, then the oldest, so a new version never takes over.
    static IEnumerable<RomFile> Ranked(GameEntry g) {
        return g.Roms.OrderBy(r => r.EngineCopy ? 1 : 0).ThenBy(r => r.Verified ? 0 : 1).ThenBy(r => r.Encrypted ? 1 : 0).ThenBy(r => r.Added).ThenBy(r => r.File, StringComparer.OrdinalIgnoreCase);
    }
    static void ChooseRom(GameEntry g) {
        if (g.Roms.Count == 0) return;
        string saved = Config.Read("rom-" + g.Id + ".txt", "").Trim();
        g.Chosen = g.Roms.Find(r => r.Sha1 == saved) ?? Ranked(g).First();
        // Kept from the first time there's a choice, so moving or adding files can't change it later.
        if (g.Roms.Count > 1 && g.Chosen.Sha1 != saved) Config.Write("rom-" + g.Id + ".txt", g.Chosen.Sha1);
        if (g.FulldexCode != null) g.FulldexRom = g.Chosen.File;
        // An engine with an exact list uses the chosen dump if it's on the list, or its best one that is.
        if (g.Gen1 != null) { var r = g.Chosen.Gen1Ok ? g.Chosen : Ranked(g).FirstOrDefault(x => x.Gen1Ok); g.Gen1Rom = r != null ? r.File : null; }
        if (g.Nativeplat != null) { var r = g.Chosen.NativeplatOk ? g.Chosen : Ranked(g).FirstOrDefault(x => x.NativeplatOk); g.NativeplatRom = r != null ? r.File : null; }
        bool refused = g.Gen1 != null ? g.Gen1Rom == null : g.Nativeplat != null && g.FulldexCode == null && g.NativeplatRom == null;
        if (refused) { g.FoundUnsupported = g.Chosen.File; g.FoundSha1 = g.Chosen.Sha1; }
    }
    static int RevisionOf(string file, string ext) {
        int at = ext == ".nds" ? 0x1E : ext == ".gba" ? 0xBC : 0x14C;
        using (var f = File.OpenRead(file)) { if (f.Length <= at) return 0; f.Seek(at, SeekOrigin.Begin); return f.ReadByte(); }
    }
    // Reading every dump at each start would be slow on a large folder, so a hash is kept until the file changes.
    static readonly Dictionary<string, string> hashes = new Dictionary<string, string>();
    static string HashFile { get { return Path.Combine(Config.State, "rom-hashes.txt"); } }
    static void LoadHashes() {
        try { if (File.Exists(HashFile)) foreach (var l in File.ReadAllLines(HashFile)) { int cut = l.LastIndexOf('|'); if (cut > 0) hashes[l.Substring(0, cut)] = l.Substring(cut + 1); } } catch { }
    }
    static void SaveHashes() {
        try { Directory.CreateDirectory(Config.State); File.WriteAllLines(HashFile, hashes.Select(x => x.Key + "|" + x.Value).ToArray()); } catch { }
    }
    public static string Sha1Cached(string file) {
        var info = new FileInfo(file);
        string key = file + "|" + info.Length + "|" + info.LastWriteTimeUtc.Ticks, sha;
        if (hashes.TryGetValue(key, out sha)) return sha;
        return hashes[key] = Sha1Of(file);
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

// Pictures from pogo.hails.app, fetched again after 30 days. Pokedex text from PokeAPI, then a cache,
// so no game text is in this project.
class DexInfo {
    public string Genus = ""; public int Height, Weight; public string[] Types = new string[0], Abilities = new string[0];
    public int[] Stats = new int[6]; public Dictionary<string, string> Flavor = new Dictionary<string, string>();
}
static class DexInfos {
    static readonly object gate = new object();
    static readonly Dictionary<int, DexInfo> loaded = new Dictionary<int, DexInfo>();
    static readonly Dictionary<int, DateTime> asked = new Dictionary<int, DateTime>();
    static string FileOf(int id) { return Path.Combine(Config.State, "pokedex-cache", id + ".json"); }
    static System.Web.Script.Serialization.JavaScriptSerializer Json() { return new System.Web.Script.Serialization.JavaScriptSerializer { MaxJsonLength = int.MaxValue }; }
    // Null until it's ready; asking starts the fetch, and after a failure it's tried again a minute later.
    public static DexInfo Get(int id) {
        lock (gate) {
            DexInfo d; DateTime when;
            if (loaded.TryGetValue(id, out d)) return d;
            if (asked.TryGetValue(id, out when) && (DateTime.Now - when).TotalSeconds < 60) return null;
            asked[id] = DateTime.Now;
            fetching.Add(id);
        }
        Task.Run(delegate { try { Fetch(id); } catch { } lock (gate) fetching.Remove(id); Sprites.Touch(); });
        return null;
    }
    static readonly HashSet<int> fetching = new HashSet<int>();
    public static bool Loading(int id) { lock (gate) return fetching.Contains(id); }
    public static bool Wait(int id, int seconds) {
        for (int i = 0; i < seconds * 10; i++) { if (Get(id) != null) return true; System.Threading.Thread.Sleep(100); }
        return false;
    }
    // Every entry, for offline use. Ones already here aren't fetched again. Reports done, total, newly fetched, failed.
    public static void GetAll(int total, Action<int, int, int, int> report) {
        int done = 0, fresh = 0, failed = 0;
        System.Threading.Tasks.Parallel.For(1, total + 1, new System.Threading.Tasks.ParallelOptions { MaxDegreeOfParallelism = 3 }, id => {
            bool had = File.Exists(FileOf(id));
            try {
                Fetch(id);
                if (!had) System.Threading.Interlocked.Increment(ref fresh);
            } catch { System.Threading.Interlocked.Increment(ref failed); }
            report(System.Threading.Interlocked.Increment(ref done), total, fresh, failed);
        });
    }
    static string Download(string url) {
        System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
        using (var web = new System.Net.WebClient()) {
            web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons";
            web.Encoding = Encoding.UTF8;
            return web.DownloadString(url);
        }
    }
    static void Fetch(int id) {
        string file = FileOf(id);
        DexInfo info = null;
        if (File.Exists(file)) try { info = Json().Deserialize<DexInfo>(File.ReadAllText(file)); } catch { }
        if (info == null) {
            var species = (Dictionary<string, object>)Json().DeserializeObject(Download("https://pokeapi.co/api/v2/pokemon-species/" + id));
            var mon = (Dictionary<string, object>)Json().DeserializeObject(Download("https://pokeapi.co/api/v2/pokemon/" + id));
            info = new DexInfo();
            info.Height = Convert.ToInt32(mon["height"]); info.Weight = Convert.ToInt32(mon["weight"]);
            Func<object, string, string> name = delegate(object row, string key) { return (string)((Dictionary<string, object>)((Dictionary<string, object>)row)[key])["name"]; };
            info.Types = ((object[])mon["types"]).Select(x => name(x, "type")).ToArray();
            info.Abilities = ((object[])mon["abilities"]).Select(x => name(x, "ability")).ToArray();
            info.Stats = ((object[])mon["stats"]).Select(x => Convert.ToInt32(((Dictionary<string, object>)x)["base_stat"])).Take(6).ToArray();
            foreach (var row in (object[])species["genera"]) if (name(row, "language") == "en") info.Genus = (string)((Dictionary<string, object>)row)["genus"];
            foreach (var row in (object[])species["flavor_text_entries"]) if (name(row, "language") == "en") {
                string text = ((string)((Dictionary<string, object>)row)["flavor_text"]).Replace("-\n", "-").Replace('\n', ' ').Replace('\f', ' ').Replace("\u00AD ", "").Replace("POK\u00E9MON", "Pok\u00E9mon").Replace("POKeMON", "Pok\u00E9mon");
                info.Flavor[name(row, "version")] = System.Text.RegularExpressions.Regex.Replace(text, " +", " ").Trim();
            }
            Directory.CreateDirectory(Path.GetDirectoryName(file));
            File.WriteAllText(file + ".part", Json().Serialize(info));
            File.Copy(file + ".part", file, true); File.Delete(file + ".part");
        }
        lock (gate) loaded[id] = info;
    }
}

// Python 3 for hails.nds-Patcher, only when the player wants to patch: python.org's embeddable package, checked by its
// SHA-256, unpacked into tools\python (no installer, no admin). Its ._pth also gets the patcher's folder, for its imports.
static class PythonSetup {
    const string Url = "https://www.python.org/ftp/python/3.14.10/python-3.14.10-embed-amd64.zip";
    const string Sha256 = "90905e64a840557d8c1bc4f5b74db3d7ee85e3ffb217611cac1c7813acea4159";
    public const string Size = "about 12 MB";
    static bool works;
    // Whether Config.Python runs Python 3 (Windows' "python" can be only the Microsoft Store's shortcut).
    public static bool Works() {
        if (works) return true;
        try {
            var psi = new ProcessStartInfo(Config.Python, "-c \"import sys; print(sys.version_info[0])\"") { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
            using (var p = Process.Start(psi)) {
                string o = p.StandardOutput.ReadToEnd();
                if (!p.WaitForExit(15000)) { try { p.Kill(); } catch { } return false; }
                works = p.ExitCode == 0 && o.Trim() == "3";
            }
        } catch { works = false; }
        return works;
    }
    // Null when done, or why not.
    public static string Install() {
        string dir = Config.PythonDir, zip = dir + ".download.zip", temp = dir + ".new";
        try {
            System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
            Directory.CreateDirectory(Path.GetDirectoryName(dir));
            using (var web = new System.Net.WebClient()) { web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons"; web.DownloadFile(Url, zip); }
            string hash;
            using (var f = File.OpenRead(zip)) using (var sha = SHA256.Create()) hash = BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "").ToLowerInvariant();
            if (hash != Sha256) return "The download from python.org wasn't the expected file, so it wasn't used.";
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            System.IO.Compression.ZipFile.ExtractToDirectory(zip, temp);
            string pth = Directory.GetFiles(temp, "python3*._pth").FirstOrDefault();
            if (pth == null || !File.Exists(Path.Combine(temp, "python.exe"))) return "The Python package was missing files.";
            // (relative to tools\python, so the app folder can move)
            var from = new Uri(Path.GetFullPath(dir).TrimEnd('\\') + "\\");
            var to = new Uri(Path.GetFullPath(Path.GetDirectoryName(Config.Patcher)).TrimEnd('\\') + "\\");
            string rel = Uri.UnescapeDataString(from.MakeRelativeUri(to).ToString()).Replace('/', '\\').TrimEnd('\\');
            if (rel.StartsWith("file:", StringComparison.OrdinalIgnoreCase)) rel = Path.GetFullPath(Path.GetDirectoryName(Config.Patcher));
            File.AppendAllText(pth, "\r\n" + (rel == "" ? "." : rel) + "\r\n");
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
            Directory.Move(temp, dir);
            works = false;
            return Works() ? null : "Python was unpacked but didn't start.";
        } catch (Exception e) { return "Python couldn't be fetched: " + e.Message; }
        finally { try { File.Delete(zip); } catch { } try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { } }
    }
}

// Updates come whole from this project's releases. The state folder and the files below are never touched.
static class Updater {
    const string Site = "https://github.com/Hailey-Ross/nativeMons/releases/download/";
    public class Release { public string channel, revision, package, sha256; public int build; }
    public static volatile Release Latest;
    static string Base { get { return Environment.GetEnvironmentVariable("NATIVEMONS_UPDATE_URL") ?? Site; } }
    static Dictionary<string, string> Version() {
        var d = new Dictionary<string, string>();
        try { foreach (var l in File.ReadAllLines(Path.Combine(Config.Here, "version.txt"))) { int e = l.IndexOf('='); if (e > 0) d[l.Substring(0, e)] = l.Substring(e + 1).Trim(); } } catch { }
        return d;
    }
    public static int Build { get { string v; int n; return Version().TryGetValue("build", out v) && int.TryParse(v, out n) ? n : 0; } }
    public const string StableTag = "stable", DevTag = "bleeding-edge";
    static string Installed { get { string v; return Version().TryGetValue("channel", out v) ? v : null; } }
    static string Revision { get { string v; return Version().TryGetValue("revision", out v) ? v : ""; } }
    // The player's choice, or else what this copy came from. The tags stay as published; players see Stable and Dev.
    public static string Channel { get { string c = Config.Read("update-channel.txt", "").Trim(); return c == StableTag || c == DevTag ? c : Installed ?? DevTag; } }
    public static void Choose(string channel) { Config.Write("update-channel.txt", channel); Latest = null; }
    public static string Name(string channel) { return channel == StableTag ? "Stable" : "Dev"; }
    public static string InstalledName { get { return Name(Installed ?? DevTag); } }
    public static bool Switching { get { return Installed != null && Installed != Channel; } }
    // A copy built here (version.txt's local=, from build.bat or build-all.ps1): no update is offered over it, so a
    // release can't replace newer work; Settings > Updates can still take one on request.
    public static string Local { get { string v; return Version().TryGetValue("local", out v) ? v : null; } }
    public static bool Ready { get { var r = Latest; if (Local != null || r == null || string.IsNullOrEmpty(r.sha256) || r.revision == Revision) return false; return Installed != null && r.channel != Installed ? r.revision != Revision : r.build > Build; } }
    static System.Net.WebClient Web() {
        System.Net.ServicePointManager.SecurityProtocol |= System.Net.SecurityProtocolType.Tls12;
        var web = new System.Net.WebClient();
        web.Headers[System.Net.HttpRequestHeader.UserAgent] = "nativeMons";
        return web;
    }
    // Null when all is well, or why it couldn't look.
    public static string Check() {
        try {
            string text;
            using (var web = Web()) { web.Encoding = Encoding.UTF8; text = web.DownloadString(Base + Channel + "/release.json"); }
            Latest = new System.Web.Script.Serialization.JavaScriptSerializer().Deserialize<Release>(text.TrimStart('\uFEFF'));
            Sprites.Touch();
            return null;
        } catch (Exception x) { Latest = null; return x.Message; }
    }
    static string[] Kept = {
        @"engines\fulldex\saves\", @"engines\fulldex\roms\", @"engines\fulldex\states\", @"engines\fulldex\cheats\", @"engines\fulldex\settings.cfg",
        @"engines\nativeplat\userdata\", @"engines\pokeplatinum\save.bin", @"engines\pokeplatinum\sim_config.ini", "state\\",
    };
    static bool IsKept(string relative) { return Kept.Any(k => relative.StartsWith(k, StringComparison.OrdinalIgnoreCase)); }
    // Downloads, proves and applies the latest release. Returns null when done. Nothing changes until the download has proved good.
    public static string Apply(Action<string, float> report) {
        var release = Latest;
        if (release == null || string.IsNullOrEmpty(release.sha256)) return "There's no release to update to.";
        string work = Path.Combine(Config.State, "update"), zip = Path.Combine(work, "package.zip"), here = Config.Here;
        try {
            if (Directory.Exists(work)) Directory.Delete(work, true);
            Directory.CreateDirectory(work);
            using (var web = Web()) {
                web.DownloadProgressChanged += (s, e) => report("Downloading build " + release.build, e.ProgressPercentage / 100f * 0.7f);
                var wait = new System.Threading.ManualResetEventSlim();
                Exception failed = null;
                web.DownloadFileCompleted += (s, e) => { failed = e.Error; wait.Set(); };
                web.DownloadFileAsync(new Uri(Base + Channel + "/" + release.package), zip);
                wait.Wait();
                if (failed != null) return "The download didn't finish, and nothing was changed. " + failed.Message;
            }
            report("Checking the download", 0.72f);
            string hash;
            using (var f = File.OpenRead(zip)) using (var sha = SHA256.Create()) hash = BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "").ToLowerInvariant();
            if (hash != release.sha256.ToLowerInvariant()) return "The download didn't match its checksum, so nothing was changed.";
            using (var archive = System.IO.Compression.ZipFile.OpenRead(zip)) {
                var files = archive.Entries.Where(e => e.Name != "").ToList();
                Func<System.IO.Compression.ZipArchiveEntry, string> relative = e => { string p = e.FullName.Replace('/', '\\'); int cut = p.IndexOf('\\'); return cut >= 0 ? p.Substring(cut + 1) : p; };
                var names = new HashSet<string>(files.Select(relative), StringComparer.OrdinalIgnoreCase);
                foreach (var needed in new[] { "NativeMonsLauncher.exe", "games.json", @"engines\gen1recomp\main.lua", @"engines\fulldex\Pokemon Fulldex.exe", @"engines\nativeplat\nativeplat.exe", @"tools\mon-bridge\mon-bridge.exe" })
                    if (!names.Contains(needed)) return "This release is missing " + needed + ", so nothing was changed.";
                int done = 0, changed = 0;
                foreach (var entry in files) {
                    string rel = relative(entry), target = Path.Combine(here, rel);
                    report("Updating", 0.75f + 0.25f * done++ / files.Count);
                    if (IsKept(rel)) continue;
                    if (File.Exists(target) && new FileInfo(target).Length == entry.Length && SameFile(entry, target)) continue;
                    Directory.CreateDirectory(Path.GetDirectoryName(target));
                    string part = target + ".update";
                    System.IO.Compression.ZipFileExtensions.ExtractToFile(entry, part, true);
                    if (string.Equals(rel, "NativeMonsLauncher.exe", StringComparison.OrdinalIgnoreCase) && File.Exists(target)) {
                        // A running program can be renamed but not overwritten. The old one goes at the next start.
                        string old = target + ".old";
                        if (File.Exists(old)) File.Delete(old);
                        File.Move(target, old);
                    } else if (File.Exists(target)) File.Delete(target);
                    File.Move(part, target);
                    changed++;
                }
                // Gen1Recomp's code is replaced exactly, so a file upstream removed can't linger. Its player data lives elsewhere.
                string code = Path.Combine(here, "engines", "gen1recomp", "src");
                if (Directory.Exists(code))
                    foreach (var f in Directory.GetFiles(code, "*", SearchOption.AllDirectories))
                        if (!names.Contains(f.Substring(here.Length).TrimStart('\\'))) File.Delete(f);
                // A package without this stamp would look out of date for ever.
                File.WriteAllText(Path.Combine(here, "version.txt"), "channel=" + release.channel + "\nbuild=" + release.build + "\nrevision=" + release.revision + "\n");
                report(changed + " files updated", 1f);
            }
            return null;
        } catch (Exception x) { return "The update stopped partway: " + x.Message + " Your saves and settings weren't touched. Run it again to finish."; }
        finally { try { if (Directory.Exists(work)) Directory.Delete(work, true); } catch { } }
    }
    static bool SameFile(System.IO.Compression.ZipArchiveEntry entry, string file) {
        using (var sha = SHA256.Create()) {
            byte[] a, b;
            using (var s = entry.Open()) a = sha.ComputeHash(s);
            using (var f = File.OpenRead(file)) b = sha.ComputeHash(f);
            return a.SequenceEqual(b);
        }
    }
    public static void CleanUp() { try { string old = Path.Combine(Config.Here, "NativeMonsLauncher.exe.old"); if (File.Exists(old)) File.Delete(old); } catch { } }
}

static class Sprites {
    public static void Touch() { Arrived++; }
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
            lock (gate) tried.Add(key);
            Arrived++;
        }
    }
    // Asked for and still on its way (not here, not given up on): the window shows the loading Poke Ball meanwhile.
    static readonly HashSet<string> tried = new HashSet<string>();
    public static bool Loading(string key) { lock (gate) return key != null && asked.Contains(key) && !loaded.ContainsKey(key) && !tried.Contains(key); }
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
    // Every picture to the end of the National Dex, normal and shiny, for offline play. Safe to run again: it only fetches what's missing.
    public const int MaxDex = 1025;
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

// Verified backups before any change, and one shared save per game (notes/launcher.md, Saves).
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
    // A romhack's save only has to read: many store parts of it their own way, which fails the original's checksums.
    public static string Problem(GameEntry g, string file) {
        if (g == null || !g.IsHack) return Problem(file);
        if (!File.Exists(Config.MonBridge) || IsBlank(file)) return null;
        int code; string o = Bridge("info \"" + file + "\"", out code);
        if (code == 0) return null;
        // PKHeX can't read every hack's save (pokeemerald-expansion moves things): the game's own sectors decide then.
        return !g.IsDs && Gen3Intact(file) ? null : "It can't be read.";
    }
    // A GBA Pokemon save the game can load: at least one whole save (14 sectors) carrying the game's sector signature.
    static bool Gen3Intact(string file) {
        try {
            var d = File.ReadAllBytes(file);
            if (d.Length < 0x20000) return false;
            int signed = 0;
            for (int s = 0; s < 32; s++) if (BitConverter.ToUInt32(d, s * 0x1000 + 0xFF8) == 0x08012025) signed++;
            return signed >= 14;
        } catch { return false; }
    }
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
        string mark = Problem(g, file) != null ? "-damaged" : "";
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
    public int Fold;   // a section header: 1 open, 2 closed
    public Color Tint = Color.Empty; // its icon card's colour, when the icon's own won't do
    public Item(string text, string hint, bool on, Action run) { Text = text; Hint = hint ?? ""; On = on; Run = run; }
}
class Page {
    public string Title; public List<Item> Items = new List<Item>(); public int Sel;
    public int TopRow; public float ScrollRow = -1;   // grid scrolling: where it's heading, and where it is now
    public string Body;          // a text page (notes, results) instead of a list
    public Action OnBack;        // null = pop the page
    public string Mono;          // a fixed-width block under the body
    public float BodyScroll;     // how far a long body has been scrolled, in pixels
    public int Jump;             // left and right move this many rows at once, on a very long list
    public int DexId;            // a Pokedex entry: this species
    public int[] DexOrder;       // what left and right step through on an entry
    public bool DexShiny;        // the entry is showing its shiny picture
    public GameEntry DexOnly;    // a list or entry of one game's own Pokedex (a romhack's), not the collection's
    public bool FitBody;         // the Body's box only as tall as its text, so the list below it has room
    public float DexHeight;      // how tall the entry came out
    public bool Mods;            // hides the clash banner while open
    public BoxView Boxes;        // Move Pokemon's box screen
    public bool Main;
    public Theme Theme;
}

// Move Pokemon as boxes, as Pokemon HOME shows them: two games' boxes, with Pokemon dragged between them.
class BoxMon { public string Species, Nick, Trainer; public int Level, Dex; public bool Shiny; }
class BoxSide {
    public GameEntry Game; public string File, Problem;
    public bool NoSave;   // nothing saved yet: not a fault, and said as such
    public int Boxes = 1, Slots = 30;
    public List<string> Names = new List<string>();
    public Dictionary<int, BoxMon> Mons = new Dictionary<int, BoxMon>();   // box * Slots + slot
    public BoxMon At(int box, int slot) { BoxMon m; return Mons.TryGetValue(box * Slots + slot, out m) ? m : null; }
}
class BoxView {
    public BoxSide[] Side = new BoxSide[2];
    public int[] Box = new int[2];
    public int Pane, Cursor, Col;          // Cursor -1 is the pane's header (game and box)
    public int HeldPane = -1, HeldBox, HeldSlot;
    public bool Dragging, Dragged; public PointF Drag, DragStart;
    public string HoverArrow; public DateTime HoverSince;
    public bool Wide;                      // side by side (the tablet), not one above the other
    public string Note;
    public bool Holding { get { return HeldPane >= 0; } }
}

class LauncherForm : Form {
    const ushort UP = 0x0001, DOWN = 0x0002, LEFT = 0x0004, RIGHT = 0x0008, A = 0x1000, B = 0x2000, X = 0x4000, LB = 0x0100, RB = 0x0200;
    // Gen1Recomp's credit, on the Credits page only. The last sentence keeps its LICENSE.MD term 1 wording.
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
    string directSlot, chooseId;   // --slot: which save the shortcut's game uses; --launcher: the game the menu opens on
    public LauncherForm() : this(null, null, null) { }
    // playId: start that game at once and close afterwards (--game <id>, for shortcuts).
    public LauncherForm(string playId, string slot, string choose) {
        playDirect = playId; directSlot = slot; chooseId = choose;
        Text = "nativeMons";
        try { Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath); } catch { }
        // The waiting quick-start game goes with the launcher: waited for here, as a background task dies with the program.
        FormClosing += (o, e) => {
            // Closed by the user, the device powers down first (PowerOffTick), then closes for real.
            if (powerOff < 0 && Visible && playDirect == null && !Native.Quiet && WindowState == FormWindowState.Normal
                && (e.CloseReason == CloseReason.UserClosing || e.CloseReason == CloseReason.None)) {
                e.Cancel = true; powerOff = (float)clock.Elapsed.TotalSeconds; powerTop = Top; return;
            }
            if (e.Cancel) return;
            Hide(); DropReady(true); StopSnapshot();
        };
        // A device standing upright: no window frame, the window's own outline is the shell's.
        var area = Screen.PrimaryScreen.WorkingArea;
        int h = area.Height * 94 / 100;
        FormBorderStyle = FormBorderStyle.None;
        StartPosition = FormStartPosition.Manual; ClientSize = new Size(h * 60 / 100, h);
        Location = new Point(area.Left + (area.Width - Width) / 2, area.Top + (area.Height - Height) / 2);
        using (var outline = Round(new RectangleF(0, 0, ClientSize.Width, ClientSize.Height), CornerRadius(ClientSize.Width))) Region = new Region(outline);
        BackColor = Color.FromArgb(18, 20, 24); ForeColor = Color.White;
        DoubleBuffered = true; KeyPreview = true;
        AllowDrop = true;
        DragEnter += delegate(object o, DragEventArgs e) { e.Effect = e.Data.GetDataPresent(DataFormats.FileDrop) && CanTakeDrop() ? DragDropEffects.Copy : DragDropEffects.None; };
        DragDrop += delegate(object o, DragEventArgs e) { var dropped = e.Data.GetData(DataFormats.FileDrop) as string[]; if (dropped != null && CanTakeDrop()) BeginInvoke((Action)delegate { TakeDrop(dropped); }); };
        try { Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath); } catch { }
        tick.Interval = 16; tick.Tick += delegate { Poll(); };
        Load += delegate { tick.Start(); };
        Shown += delegate { Start(); };
        MakeFonts(ClientSize.Width);
    }

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

    string previewId;
    public void Snapshot(string file, string screen, int w, int h) {
        // A size wider than tall shows the tablet, with the phone's text.
        if (w > h) { phoneW = h * 60 / 100; tabletK = 1; MakeFonts(phoneW); } else MakeFonts(w);
        try { Library.Scan(Gen1Cli); } catch (Exception e) { Library.Note = e.Message; }
        PickCurrent();
        int colon = screen.IndexOf(':');
        if (colon > 0) {
            string id = screen.Substring(colon + 1); screen = screen.Substring(0, colon);
            previewId = id;
            var preview = Library.Games.Find(g => g.Id == id);
            if (preview != null) current = preview;
        }
        // clashbanner[:<game>]: the clash banner, read now.
        if (screen == "clashbanner") {
            foreach (var mg in Library.Games.Where(x => x.OnGen1 && (previewId == null || x.Id == previewId))) {
                int n = ModClashes.Read(mg).Count(c => c.Active);
                if (n > 0) { clashCount += n; if (clashGame == null) clashGame = mg; }
            }
            // Nothing to show: a sample.
            if (clashGame == null) { clashGame = Library.Games.Find(x => x.OnGen1); clashCount = 1; }
            clashAt = -100;
        }
        if (screen == "updatebanner") { Updater.Latest = new Updater.Release { build = Updater.Build + 1, sha256 = "preview" }; bannerBuild = Updater.Latest.build; bannerAt = -1; }
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
        if (screen == "settings") pages.Push(SettingsPage());
        if (screen == "controls") pages.Push(ControlsPage());
        if (screen == "screenshots") pages.Push(ScreenshotsPage());
        if (screen.StartsWith("controls-")) { int n = int.Parse(screen.Substring(9)); pages.Push(ControlsPage()); pages.Push(new Page { Title = ControlSheets[n][0] + " controls", Body = ControlSheets[n][2] }); }
        var sample = (previewId != null ? Library.Games.Find(g => g.Id == previewId) : null) ?? Library.Games.Find(g => g.Playable) ?? (Library.Games.Count > 0 ? Library.Games[0] : null);
        if (screen == "game" && sample != null) pages.Push(GamePage(sample));
        // clashes:<game> (Mods that clash), clashon:<game>,<mod id> (switching that mod on beside one it clashes with).
        if (screen == "clashes" || screen == "clashon") {
            var ids = (previewId ?? "").Split(',');
            var mg = Library.Games.Find(x => x.Id == ids[0] && x.OnGen1);
            if (mg != null) {
                pages.Push(ModsPage(mg));
                if (screen == "clashes") pages.Push(ClashesPage(mg, null));
                else {
                    var c = ModClashes.Read(mg).FirstOrDefault(x => x.ModId == ids[1] || x.OtherId == ids[1]);
                    if (c != null) pages.Push(ClashChoicePage(mg, ids[1], c.ModId == ids[1] ? c.ModName : c.OtherName, c));
                }
            }
        }
        // deps:<game> (Needs other mods), dep:<game>,<dependency id> (where to get one).
        if (screen == "deps" || screen == "dep") {
            var ids = (previewId ?? "").Split(',');
            var mg = Library.Games.Find(x => x.Id == ids[0] && x.OnGen1);
            if (mg != null) {
                pages.Push(ModsPage(mg)); pages.Push(DepsPage(mg, screen == "deps" ? "Installed (the mod just added)." : null));
                var one = ids.Length > 1 ? ModDeps.Open(mg).Find(x => x.Id == ids[1]) : null;
                if (screen == "dep" && one != null) { string note; List<ModIndex.Entry> all; try { all = ModIndex.Load(out note); } catch { all = new List<ModIndex.Entry>(); } pages.Push(DepPage(mg, one, all)); }
            }
        }
        if (screen == "findmods" || screen == "modentry") {
            var ids = (previewId ?? "").Split(',');
            var mg = Library.Games.Find(x => x.Id == ids[0] && x.OnGen1);
            if (mg != null) {
                string note; var all = ModIndex.Load(out note); var have = InstalledMods(mg);
                pages.Push(ModsPage(mg)); pages.Push(FindModsPage(mg, have, all, note));
                var one = ids.Length > 1 ? all.Find(x => x.Id == ids[1]) : null;
                if (screen == "modentry" && one != null) pages.Push(ModEntryPage(mg, one, have));
            }
        }
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
        // boxes|boxheld|boxchoose:<top id>,<bottom id>: the box screen, reading the saves only.
        if (screen == "boxes" || screen == "boxheld" || screen == "boxchoose") {
            var ids = (previewId ?? "").Split(',');
            var a = Library.Games.Find(g => g.Id == ids[0] && g.SaveTools); var b = ids.Length > 1 ? Library.Games.Find(g => g.Id == ids[1] && g.SaveTools) : a;
            if (a != null && b != null) {
                Sprites.LoadFamilies(BridgeFamilies);
                var v = new BoxView(); v.Side[0] = LoadSide(a); v.Side[1] = b.Id == a.Id ? v.Side[0] : LoadSide(b);
                var first = v.Side[0].Mons.Keys.Where(k => k / v.Side[0].Slots == 0).OrderBy(k => k).FirstOrDefault();
                v.Cursor = first % v.Side[0].Slots;
                if (screen == "boxheld" && v.Side[0].At(0, v.Cursor) != null) { v.HeldPane = 0; v.HeldBox = 0; v.HeldSlot = v.Cursor; BoxOther(v); }
                pages.Push(new Page { Title = "Move Pokemon", Boxes = v });
                if (screen == "boxchoose") ChooseBoxGame(v, 1);
                for (int i = 0; i < 2; i++) foreach (var mon in v.Side[i].Mons.Where(x => x.Key / v.Side[i].Slots == v.Box[i]).Select(x => x.Value)) Sprites.Get((mon.Shiny ? "shiny/" : "") + mon.Dex);
                Sprites.Wait(30);
            }
        }
        if (screen == "roms" || screen == "gamepage") {
            var which = Library.Games.Find(g => g.Id == previewId);
            if (which != null) pages.Push(screen == "roms" ? RomsPage(which) : GamePage(which));
        }
        if (screen == "dexother") { LoadDex(); pages.Push(DexHomePage()); pages.Push(DexOtherPage()); }
        if (screen == "dexgame") { LoadDex(); var one = Library.Games.Find(g => g.Id == previewId); if (one != null) { pages.Push(DexOtherPage()); pages.Push(DexListPage(one.Title, 1, 151, one)); } }
        if (screen == "dex" || screen == "dexlist" || screen == "dexentry") {
            LoadDex();
            int species; if (!(colon > 0 && int.TryParse(previewId, out species))) species = 25;
            var order = Enumerable.Range(1, Sprites.MaxDex).Where(DexSeen).ToArray();
            if (screen == "dex") pages.Push(DexHomePage());
            else if (screen == "dexlist") { var list = DexListPage("Kanto", 1, 151); list.Sel = Math.Min(species - 1, 150); pages.Push(list); for (int i = 0; i < 12 && list.Sel + i < 151; i++) if (DexSeen(list.Sel + i + 1)) Sprites.Get((list.Sel + i + 1).ToString()); Sprites.Wait(15); }
            else { pages.Push(DexEntryPage(species, order)); if (DexCaught(species)) DexInfos.Wait(species, 30); Sprites.Get((DexShiny(species) ? "shiny/" : "") + species); Sprites.Wait(15); }
        }
        if (screen == "job") {
            job = new Process(); jobTitle = "Preparing Pokemon LeafGreen"; jobStart = DateTime.Now.AddSeconds(-14); progress = 0.62f;
            foreach (var l in new[] { "Reading the ROM", "Decoding maps", "Decoding Pokemon graphics" }) log.Add(l);
        }
        if (screen == "appicons") {
            // Each app's highlighted motion, still first, then moments along it.
            var times = new[] { -1f, 0.25f, 0.35f, 0.45f, 0.55f, 0.65f, 1.0f, 1.6f, 2.2f };
            using (var bmp = new Bitmap(times.Length * 90, (AppNames.Length - 1) * 90)) using (var g = Graphics.FromImage(bmp)) {
                g.Clear(Color.FromArgb(60, 60, 70));
                for (int a = 1; a < AppNames.Length; a++) for (int k = 0; k < times.Length; k++) {
                    var r = new RectangleF(k * 90 + 10, (a - 1) * 90 + 10, 70, 70);
                    GlossTile(g, r, 70 * 0.24f, AppColors[a]); BallShake(-1); AppArt(g, a, r, AppColors[a], times[k]);
                }
                bmp.Save(file, System.Drawing.Imaging.ImageFormat.Png);
            }
            return;
        }
        using (var bmp = new Bitmap(w, h)) {
            using (var g = Graphics.FromImage(bmp)) { Render(g, w, h); screenAt = selAt = -10; Render(g, w, h); }
            bmp.Save(file, System.Drawing.Imaging.ImageFormat.Png);
        }
    }

    // ---------- the engines ----------
    // Gen1Recomp's data is in state\gen1recomp (notes/launcher.md, Gen1Recomp).
    public static string Gen1Data { get { return Path.Combine(Config.State, "gen1recomp"); } }
    // LOVE's folder follows APPDATA, so the launcher reads it the same way (and a test can point both elsewhere).
    static string LoveRoot { get { return Path.Combine(Environment.GetEnvironmentVariable("APPDATA") ?? Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LOVE"); } }
    public static string OldGen1Data { get { return Path.Combine(LoveRoot, Identity); } }
    // Where its saves are now: the old folder only while the move hasn't happened.
    public static string Gen1Root { get { return Directory.Exists(Gen1Data) ? Gen1Data : OldGen1Data; } }
    const string DataIdentity = "nativemons-portable";
    public static bool Gen1DataMoveNeeded { get { return !Directory.Exists(Gen1Data) && Directory.Exists(OldGen1Data); } }
    static readonly object gen1DataLock = new object();
    static string gen1DataProblem;
    // Null once the folder is there. A move that fails leaves nothing half done, and is tried again at the next start.
    public static string EnsureGen1Data() {
        lock (gen1DataLock) {
            if (Directory.Exists(Gen1Data)) return null;
            if (gen1DataProblem != null) return gen1DataProblem;
            string part = Gen1Data + ".part";
            try {
                if (Directory.Exists(part)) Directory.Delete(part, true);
                Directory.CreateDirectory(part);
                int files = 0;
                if (Directory.Exists(OldGen1Data)) files = CopyChecked(OldGen1Data, part);
                File.WriteAllText(Path.Combine(part, "nativemons-moved.txt"), Directory.Exists(OldGen1Data)
                    ? "Copied from " + OldGen1Data + " on " + DateTime.Now.ToString("yyyy-MM-dd HH:mm") + ", " + files + " files, each checked. The old folder was left as it was.\r\n"
                    : "Started here on " + DateTime.Now.ToString("yyyy-MM-dd HH:mm") + ".\r\n");
                Directory.Move(part, Gen1Data);
                return null;
            } catch (Exception x) {
                try { if (Directory.Exists(part)) Directory.Delete(part, true); } catch { }
                gen1DataProblem = "Gen1Recomp's saves couldn't be moved beside the launcher, so they stay where they were, in " + OldGen1Data + ". It'll try again next time. " + x.Message;
                return gen1DataProblem;
            }
        }
    }
    // Every file copied, then read back: same length and the same SHA-1, or the whole move is off.
    static int CopyChecked(string from, string to) {
        int n = 0;
        foreach (var dir in Directory.GetDirectories(from, "*", SearchOption.AllDirectories))
            Directory.CreateDirectory(Path.Combine(to, dir.Substring(from.Length).TrimStart('\\')));
        foreach (var file in Directory.GetFiles(from, "*", SearchOption.AllDirectories)) {
            string copy = Path.Combine(to, file.Substring(from.Length).TrimStart('\\'));
            File.Copy(file, copy, true);
            if (new FileInfo(copy).Length != new FileInfo(file).Length || SaveGuard.Hash(copy) != SaveGuard.Hash(file))
                throw new IOException("A copy didn't match: " + file);
            n++;
        }
        return n;
    }
    // A folder path any program reads correctly from an environment variable: its short form when it isn't plain ASCII.
    static string PlainDir(string dir) {
        if (dir.All(c => c < 128)) return dir;
        var s = new StringBuilder(600);
        return Native.GetShortPathName(dir, s, 600) > 0 ? s.ToString() : dir;
    }
    // The picture jobs' throwaway Gen1Recomp, apart from the player's.
    static string Gen1ArtData { get { return Path.Combine(Config.State, "gen1recomp-art"); } }
    public static void UseArtIdentity(ProcessStartInfo psi, string identity) {
        psi.EnvironmentVariables["POKEPORT_IDENTITY"] = identity;
        if (psi.EnvironmentVariables.ContainsKey("NATIVEMONS_DATA_DIR")) {
            Directory.CreateDirectory(Gen1ArtData);
            psi.EnvironmentVariables["NATIVEMONS_DATA_DIR"] = PlainDir(Gen1ArtData);
        }
    }
    public static void RemoveArtData(string identity) {
        foreach (var d in new[] { Gen1ArtData, Path.Combine(LoveRoot, identity) })
            try { if (Directory.Exists(d)) Directory.Delete(d, true); } catch { }
    }
    public static ProcessStartInfo Gen1Start(string args) {
        var psi = new ProcessStartInfo(Config.Love, "\"" + Config.Gen1 + "\"" + args) {
            UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = Config.Gen1,
            RedirectStandardOutput = true, RedirectStandardError = true };
        // Its data is beside the launcher once it's there; until then (a move that couldn't finish), where it always was.
        if (EnsureGen1Data() == null) {
            psi.EnvironmentVariables["POKEPORT_IDENTITY"] = DataIdentity;
            psi.EnvironmentVariables["NATIVEMONS_DATA_DIR"] = PlainDir(Gen1Data);
        } else psi.EnvironmentVariables["POKEPORT_IDENTITY"] = Identity;
        // The launcher shows its own Discord status, so Gen1Recomp's is off.
        psi.EnvironmentVariables["POKEPORT_NO_DISCORD"] = "1";
        return psi;
    }
    // Mods now off by default, switched off once in older settings (notes/launcher.md, Gen1Recomp).
    static readonly string[] OffByDefault = { "example_lttp_overhaul" };
    static void ModDefaults() {
        if (!Library.Gen1Found) return;
        string done = Config.Read("mod-defaults.txt", "");
        foreach (var id in OffByDefault) {
            if (done.Split('\n').Any(l => l.Trim() == id)) continue;
            var psi = Gen1Start("");
            psi.EnvironmentVariables["NATIVEMONS_CLI"] = "default-off " + id;
            int code; RunCapture(psi, 30, out code);
            if (code != 0) continue;   // tried again next start
            Config.Write("mod-defaults.txt", (done = done.TrimEnd() + "\n" + id).Trim() + "\n");
        }
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
    // A wallpaper capture runs an engine too: it stops after this picture, then this carries on.
    bool AfterCapture(Action then) {
        if (!GameArt.Capturing) return false;
        GameArt.StopNow();
        Busy("Just a moment", delegate { while (GameArt.Capturing) System.Threading.Thread.Sleep(200); return null; }, delegate(string e) { then(); });
        return true;
    }
    static string RunningGames() {
        var names = new List<string>();
        if (PortRunning()) names.Add("the Platinum PC port");
        var native = Process.GetProcessesByName("nativeplat");
        if (native.Any(p => p.Id != readyPid)) names.Add("nativeplat");
        if (PcGames.AnyRunning()) names.Add("a PC game");
        foreach (var p in native) p.Dispose();
        foreach (var n in new[] { Path.GetFileNameWithoutExtension(Config.Fulldex), "lovec", "love" }) {
            var found = Process.GetProcessesByName(n);
            // The game quick start keeps waiting isn't running for the player.
            if (found.Any(p => p.Id != readyPid && p.Id != snapPid)) names.Add(n);
            foreach (var p in found) p.Dispose();
        }
        return string.Join(", ", names.ToArray());
    }

    // ---------- start ----------
    void Start() {
        if (!Native.Quiet) Activate(); Native.Front(Handle);
        starting = true; startSince = DateTime.Now; startStage = "Looking for your games"; Invalidate();
        Task.Run(() => {
            string err = null;
            // Once: Gen1Recomp's saves come beside the launcher (EnsureGen1Data), before anything asks it about them.
            if (Gen1DataMoveNeeded) { startStage = "Moving Gen1Recomp's saves beside the launcher"; err = EnsureGen1Data(); startStage = "Looking for your games"; }
            try { Library.Scan(Gen1Cli); RecoverSwitches(); } catch (Exception e) { err = e.Message; }
            try { ModDefaults(); } catch { }
            try { PrepareAll(); } catch { }
            // A shortcut straight to a game doesn't wait for pictures; they're made in the background instead.
            if (playDirect == null) try { WallpapersAtStart(); } catch { }
            // Title screens are gathered afterwards, without holding up the menu.
            Updater.CleanUp();
            Task.Run(() => { System.Threading.Thread.Sleep(2000); Updater.Check(); });
            Task.Run(() => { System.Threading.Thread.Sleep(3000); try { GameArt.CaptureMissing(() => job != null || RunningGames() != "", delegate { LoadDex(true); }); } catch { } });
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
        ReadClashes();
        if (playDirect != null) {
            var direct = Library.Games.Find(g => g.Id == playDirect);
            if (direct != null && direct.Playable) { PlayWithSlot(direct, directSlot, delegate { Close(); }); Invalidate(); return; }
            ShowText("Can't start " + playDirect, "That game is not in the list, or its ROM was not found.", delegate { Close(); });
            return;
        }
        if (chooseId != null) { Request("show", chooseId, directSlot); chooseId = null; }
        // For tests: straight to a screen once the start is done, with no key presses to miss (NATIVEMONS_TEST_OPEN=move).
        if (Environment.GetEnvironmentVariable("NATIVEMONS_TEST_OPEN") == "move") OpenBoxView();
        if (err != null) ShowText("Something went wrong", err, null);
        else if (Config.RomFolders().Count == 0 && !Library.Games.Exists(g => g.Playable)) { firstSetup = true; pages.Push(FoldersPage()); }
        else AskAboutEncrypted();
        Invalidate();
    }
    // A shortcut started while the launcher is open (see Main), or --launcher at start.
    public const int WM_REQUEST = 0x8000 + 0x4E4D;
    protected override void WndProc(ref Message m) {
        if (m.Msg == WM_REQUEST) {
            try {
                string f = Path.Combine(Config.State, "request.txt");
                var l = File.ReadAllText(f).Split('\n'); File.Delete(f);
                if (l.Length >= 2) pendingRequest = l;
            } catch { }
            return;
        }
        base.WndProc(ref m);
    }
    string[] pendingRequest;
    // Waits for the menu: a request while a game or a task runs is dropped, since only one game runs at a time.
    void RequestTick() {
        if (pendingRequest == null || starting) return;
        var r = pendingRequest; pendingRequest = null;
        if (job != null) return;
        Request(r[0].Trim(), r[1].Trim(), r.Length > 2 && r[2].Trim() != "" ? r[2].Trim() : null);
    }
    void Request(string what, string id, string slot) {
        var g = Library.Games.Find(x => x.Id == id);
        while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
        if (g == null || !g.Playable) { ShowText("Can't start " + id, "That game is not in the list, or its ROM was not found.", null); return; }
        if (what == "play") { if (!Visible) Show(); Native.Front(Handle); PlayWithSlot(g, slot, null); return; }
        current = g; ReplaceMain(); pages.Push(GamePage(g));
        if (!Visible) Show();
        Native.Front(Handle); Invalidate();
    }
    // A save named by its number or its name. Switching goes through SwitchSave, the same as the Saves page.
    void PlayWithSlot(GameEntry g, string slot, Action failed) {
        if (slot == null || !g.SaveTools) { if (slot != null && !g.SaveTools) ShowText("Can't pick a save", g.Title + " keeps its saves itself.", failed); else Play(g); return; }
        string set = SaveSets.All(g).FirstOrDefault(x => x == slot) ?? SaveSets.All(g).FirstOrDefault(x => string.Equals(SaveSets.Name(g, x), slot, StringComparison.OrdinalIgnoreCase));
        if (set == null) { ShowText("No save called " + slot, g.Title + " has: " + string.Join(", ", SaveSets.All(g).Select(x => SaveSets.Name(g, x) + " (" + x + ")").ToArray()) + ".", failed); return; }
        if (set == SaveSets.Active(g)) { Play(g); return; }
        string problem = null;
        Busy("Switching saves", delegate { problem = SwitchSave(g, set); return null; }, delegate(string e) {
            ReplaceMain();
            if (e != null || problem != null) { ShowText("Couldn't switch saves", e ?? problem, failed); return; }
            Play(g);
        });
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
    // Encrypted ROMs are raised once, when found. After that they stay flagged on their game's page.
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
            WithPython(delegate {
                RunTool("Patching", Config.Python, "\"" + Config.Patcher + "\" " + files, Path.GetDirectoryName(Config.Patcher), delegate(int code) {
                    if (code != 0) ShowLog("Patching failed", "The patcher stopped with code " + code + "."); else ReplaceMain();
                });
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
        p.Items.Add(new Item("My Collection", "Your Pokedex, across your games.", have > 0 && File.Exists(Config.MonBridge), delegate { OpenCollection(); }) { Icon = '\uE8F1' });
        bool mods = Library.Games.Exists(g => g.OnGen1);
        p.Items.Add(new Item("Mods", mods ? "Mods for the Gen1Recomp games." : "Needs a Gen1Recomp game.", mods, delegate { pages.Push(ModGamesPage()); }) { Icon = '' });
        int folders = Config.RomFolders().Count;
        p.Items.Add(new Item("ROM folders", folders + (folders == 1 ? " folder" : " folders") + " searched for your ROMs.", true, delegate { pages.Push(FoldersPage()); }) { Icon = '' });
        p.Items.Add(new Item("Move Pokemon", "From one game to another.", File.Exists(Config.MonBridge) && Library.Games.Count(g => g.SaveTools) > 1, delegate { OpenBoxView(); }) { Icon = '\uE8AB' });
        p.Items.Add(new Item("Settings", "Game options, sprites, credits.", true, delegate { pages.Push(SettingsPage()); }) { Icon = '\uE713' });
        p.Items.Add(new Item("Exit", "Close the launcher.", true, delegate { Close(); }) { Icon = '' });
        return p;
    }
    // ---------- settings ----------
    // Stored where each engine's own menus keep them, so the two agree.
    Page SettingsPage() {
        var p = new Page { Title = "Settings" };
        p.Items.Add(new Item("Fulldex", "", File.Exists(Config.Fulldex), delegate { OpenEngineSettings("fulldex"); }) { Icon = '\uE7FC', Info = "DS and GBA games: widescreen, frame rate and resolution." });
        p.Items.Add(new Item("Gen1Recomp", "", File.Exists(Path.Combine(Config.Gen1, "main.lua")), delegate { OpenEngineSettings("gen1"); }) { Icon = '\uE7FC', Info = "Game Boy, Game Boy Color and GBA games: text speed and battles." });
        p.Items.Add(new Item("Import saves", "", File.Exists(Config.MonBridge), delegate { OpenImportSaves(); }) { Icon = '\uE896', Info = "Bring in saves from other emulators and folders, such as those beside your ROMs." });
        p.Items.Add(new Item("Export saves", "", Library.Games.Exists(g => g.SaveTools), delegate { pages.Push(ExportGamesPage()); }) { Icon = '\uE898', Info = "Send a save back to your 3DS, a flash cart or another emulator." });
        p.Items.Add(new Item("Download Pokedex", "", true, delegate { GetAllSprites(); }) { Icon = '\uE896', Info = "Gets every Pokemon picture and Pokedex entry for offline play. Run it again at any time: it only fetches what's missing." });
        p.Items.Add(new Item("Game options", "", true, delegate { pages.Push(GameOptionsPage(null)); }) { Icon = '\uE8D2', Info = "Volume, fullscreen, text speed, battle style and more, for every game." });
        p.Items.Add(new Item("Controls", "", true, delegate { pages.Push(ControlsPage()); }) { Icon = '\uE765', Info = "The keys each engine uses while you play: saving, fast forward, snapshots, screenshots." });
        p.Items.Add(new Item("Screenshots", "", true, delegate { pages.Push(ScreenshotsPage()); }) { Icon = '', Info = "Open the folders where nativeplat and Infinite Fusion put the screenshots you take while you play." });
        p.Items.Add(new Item("Quick start: " + (QuickStart ? "On" : "Off"), "", true, delegate { QuickStart = !QuickStart; pages.Pop(); pages.Push(SettingsPage()); }) { Icon = '', Info = "Games open straight at their title screen, past the intro. The last game you played waits there while the menu is open, so Play starts it at once. Gen1Recomp, nativeplat, and DS games on Fulldex. Select to turn it " + (QuickStart ? "off" : "on") + "." });
        if (DiscordPresence.Available) p.Items.Add(new Item("Discord status: " + (DiscordPresence.Enabled ? "On" : "Off"), "", true, delegate { DiscordPresence.Enabled = !DiscordPresence.Enabled; pages.Pop(); pages.Push(SettingsPage()); }) { Icon = '', Info = "While you play, Discord shows which game, played through nativeMons. Select to turn it " + (DiscordPresence.Enabled ? "off" : "on") + "." });
        p.Items.Add(new Item(Updater.Ready ? "Updates: Build " + Updater.Latest.build + " ready" : "Updates", "", true, delegate { OpenUpdates(); }) { Icon = '\uE895', Info = "Keeps the launcher and every engine up to date from this project's releases. Your saves and settings stay as they are." });
        p.Items.Add(new Item("Credits", "", true, delegate { pages.Push(CreditsPage()); }) { Icon = '\uE946', Info = "The projects this is built from." });
        p.Items.Add(new Item("Last session", "", true, delegate { ShowText("Last session", Config.Read("last-session.txt", "No session yet."), null); }) { Icon = '\uE81C', Info = "How the last game ended." });
        return p;
    }
    void OpenUpdates() {
        string problem = null;
        Busy("Looking for updates", delegate { problem = Updater.Check(); return null; }, delegate(string e) {
            string local = Updater.Local;
            string here = local != null ? "a local build" + (local.Length >= 7 && local != "yes" ? " of " + local.Substring(0, 7) : "") : Updater.Build > 0 ? Updater.InstalledName + " build " + Updater.Build : "a local build, not from a release";
            string chosen = Updater.Name(Updater.Channel);
            string other = Updater.Channel == Updater.StableTag ? Updater.DevTag : Updater.StableTag;
            var channel = new Item("Get updates from: " + chosen, "", true, delegate { Updater.Choose(other); pages.Pop(); OpenUpdates(); }) { Icon = '\uE895', Info = "Stable is the tested build. Dev gets every change first, and may have more problems. Select to switch to " + Updater.Name(other) + "." };
            var p = new Page { Title = "Updates" };
            if (problem != null || e != null) { p.Body = "This copy is " + here + ".\n\nCouldn't look for updates on " + chosen + ". " + (e ?? problem); p.Items.Add(channel); pages.Push(p); return; }
            if (local != null && Updater.Latest != null && !string.IsNullOrEmpty(Updater.Latest.sha256)) {
                p.Body = "This copy is " + here + ", so updates aren't offered: a release could replace newer work.\n\nThe newest on " + chosen + " is build " + Updater.Latest.build + ". Updating replaces this copy's launcher and engines with it; your saves and settings stay.";
                p.Items.Add(new Item("Update anyway", "", true, delegate { pages.Pop(); StartUpdate(); }) { Icon = '' });
                p.Items.Add(new Item("Not now", "", true, delegate { pages.Pop(); }) { Icon = '' });
                p.Items.Add(channel); pages.Push(p); return;
            }
            if (!Updater.Ready) { p.Body = "This copy is " + here + ".\n\nThe newest on " + chosen + " is build " + Updater.Latest.build + ". You're up to date."; p.Items.Add(channel); pages.Push(p); return; }
            p.Body = "This copy is " + here + ".\n\n" + chosen + " build " + Updater.Latest.build + " is ready." + (Updater.Switching ? " It can be older than what you have now." : "") + " It updates the launcher and every engine together, downloading about 200 MB.\n\nYour saves, ROM copies, wallpapers and settings stay as they are. The launcher restarts when it's done.";
            p.Items.Add(new Item("Update now", "", true, delegate { pages.Pop(); StartUpdate(); }) { Icon = '\uE896' });
            p.Items.Add(new Item("Not now", "", true, delegate { pages.Pop(); }) { Icon = '\uE711' });
            p.Items.Add(channel);
            pages.Push(p);
        });
    }
    void StartUpdate() {
        if (AfterCapture(StartUpdate)) return;
        string running = RunningGames();
        if (running != "") { ShowText("Close your game first", "Already running: " + running + ". Close it, then update.", null); return; }
        string wrong = null;
        Busy("Updating nativeMons", delegate {
            wrong = Updater.Apply(delegate(string stage, float part) {
                progress = part; dirty = true;
                BeginInvoke((Action)delegate { if (log.Count == 0 || log[log.Count - 1] != stage) log.Add(stage); });
            });
            return null;
        }, delegate(string e) {
            if (e != null || wrong != null) { ShowText("Update stopped", e ?? wrong, null); return; }
            try { Process.Start(Path.Combine(Config.Here, "NativeMonsLauncher.exe")); Close(); }
            catch (Exception x) { ShowText("Updated", "Start the launcher again to finish. " + x.Message, null); }
        });
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
    // After the save is backed up and shared. A save it can't change is left alone.
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
    // Brings both engines level. Never throws, or replaces a save without a verified backup (notes/launcher.md, Saves).
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
        // A PC game's saves are its own business: backed up before the session, nothing to share or check after.
        if (g.Pc != null) { done(); return; }
        string damaged = null;
        SyncResult result = null;
        Busy("Checking your save", delegate {
            string file = CurrentSave(g);
            if (file != null) {
                try { SaveGuard.Backup(g, file, "played"); } catch { }
                damaged = SaveGuard.Problem(g, file);
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
                problem = SaveGuard.Problem(g, file);
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
    // Each save is parked as it stands. The one being left is proven first, so a failure can be undone.
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
    // Only a save not in use. Its Fulldex file and its Gen1Recomp slot go to its backups first, then both are removed (user, 2026-11-04).
    public static string DeleteSave(GameEntry g, string set) {
        if (set == SaveSets.Active(g)) return "That save is in use. Switch to another one first.";
        try {
            string folder = SaveGuard.BackupFolder(g, set), stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss-fff");
            string parked = ParkedSave(g, set);
            if (File.Exists(parked)) {
                Directory.CreateDirectory(folder);
                SaveGuard.WriteInPlace(parked, Path.Combine(folder, stamp + "-deleted.sav"));
            }
            string slot = HasGen1Slots(g) ? ReadSlot(g, set) : null;
            if (slot != null) {
                // Never the slot in use, nor one another save still points at.
                if (slot == Gen1Slot(g, "active " + g.Gen1) || SaveSets.All(g).Any(s => s != set && ReadSlot(g, s) == slot)) slot = null;
            }
            if (slot != null) {
                string file = Gen1SlotFile(g, slot);
                if (file != null && File.Exists(file)) {
                    Directory.CreateDirectory(folder);
                    SaveGuard.WriteInPlace(file, Path.Combine(folder, stamp + "-deleted-gen1recomp.lua"));
                }
                if (Gen1Slot(g, "delete " + g.Gen1 + " " + slot) == null) return "Gen1Recomp couldn't delete that save's slot, so nothing was deleted.";
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
            var sure = new Page { Title = "Delete " + SaveSets.Name(g, set) + "?", Theme = g.Theme, Body = "It leaves this list. A last copy stays in the game's backups folder." };
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
    // Copies byte for byte: the file is already in cartridge format.
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
        foreach (var game in Library.Games.Where(x => x.SaveTools)) {
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
        if (named != null && kinds.Contains(named.BaseId ?? named.Id)) return new FoundSave { File = file, Game = named };
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

    public static HashSet<int> CaughtInAllSaves(GameEntry g) {
        var d = DexInAllSaves(g);
        return d == null ? null : d.Caught;
    }
    class DexGame { public HashSet<int> Caught = new HashSet<int>(), Seen = new HashSet<int>(), Shiny = new HashSet<int>(); public int FusionsCaught = -1, FusionsSeen = -1, LaterCaught, LaterSeen; }
    // Games whose Pokedex is read: those with save tools, and Infinite Fusion, read from its own saves.
    static bool ReadsDex(GameEntry g) { return g.SaveTools || (PcGames.IsFusion(g) && g.OnPc); }
    static List<string> IfSaves(GameEntry g) {
        string dir = PcGames.SaveDir(g);
        return Directory.Exists(dir) ? Directory.GetFiles(dir, "*.rxdata").OrderBy(x => x, StringComparer.OrdinalIgnoreCase).ToList() : new List<string>();
    }
    DexGame IfDex(GameEntry g) {
        var saves = IfSaves(g);
        if (saves.Count == 0) return null;
        var national = new Dictionary<string, int>();
        foreach (var kv in dexNames) { string k = InfiniteFusionDex.Plain(kv.Value); if (!national.ContainsKey(k)) national[k] = kv.Key; }
        var d = new DexGame(); int fc, fs; var lc = new HashSet<int>(); var ls = new HashSet<int>();
        InfiniteFusionDex.Read(PcGames.Dir(g), saves, national, d.Caught, d.Seen, out fc, out fs, lc, ls);
        d.FusionsCaught = fc; d.FusionsSeen = fs; d.LaterCaught = lc.Count; d.LaterSeen = ls.Count;
        return d;
    }
    static bool AddDex(DexGame into, string file) {
        if (file == null || !File.Exists(file) || SaveGuard.IsBlank(file)) return false;
        int code; string o = Bridge("dex \"" + file + "\"", out code);
        if (code != 0) return false;
        foreach (var line in o.Split('\n')) {
            int at = line.IndexOf(':');
            if (at < 0) continue;
            var set = line.StartsWith("caught") ? into.Caught : line.StartsWith("seen") ? into.Seen : line.StartsWith("shiny") ? into.Shiny : null;
            if (set == null) continue;
            foreach (var id in line.Substring(at + 1).Trim().Split(',')) { int n; if (int.TryParse(id, out n)) set.Add(n); }
        }
        return true;
    }
    // Each Pokemon once, however many of a game's saves have it. Null when none of them could be read.
    static DexGame DexInAllSaves(GameEntry g) { bool partial; return DexInAllSaves(g, true, out partial); }
    static DexGame DexInAllSaves(GameEntry g, bool mayMoveSlots, out bool partial) {
        partial = false;
        var all = new DexGame();
        bool any = false;
        string active = SaveSets.Active(g);
        foreach (var set in SaveSets.All(g)) {
            if (set == active) { string why; any |= AddDex(all, SaveOf(g, out why)); continue; }
            if (HasMaster(g) && File.Exists(ParkedSave(g, set))) { any |= AddDex(all, ParkedSave(g, set)); continue; }
            // A Gen1Recomp slot can only be exported while it's in use, so it's briefly made so, unless a game is running.
            string slot = ReadSlot(g, set);
            if (slot == null || !HasGen1Slots(g)) continue;
            if (!mayMoveSlots || RunningGames() != "") { partial = true; continue; }
            string current = Gen1Slot(g, "active " + g.Gen1);
            if (current == null || Gen1Slot(g, "activate " + g.Gen1 + " " + slot) == null) continue;
            try { string why; any |= AddDex(all, Gen1Export(g, out why)); }
            finally { Gen1Slot(g, "activate " + g.Gen1 + " " + current); }
        }
        return any ? all : null;
    }
    // ---------- the Pokedex ----------
    static readonly string[] DexRegions = { "Kanto", "Johto", "Hoenn", "Sinnoh", "Unova", "Kalos", "Alola", "Galar", "Paldea" };
    static readonly int[] DexRegionStart = { 1, 152, 252, 387, 494, 650, 722, 810, 906, 1026 };
    Dictionary<GameEntry, DexGame> dexBy = new Dictionary<GameEntry, DexGame>();
    HashSet<GameEntry> dexNoSave = new HashSet<GameEntry>();
    readonly Dictionary<int, string> dexNames = new Dictionary<int, string>();
    string DexName(int id) { string n; return dexNames.TryGetValue(id, out n) ? n : "#" + id; }
    // Romhacks and fan games never count toward the collection: each has its own Pokedex.
    static bool CountsInCollection(GameEntry g) { return !g.IsHack && g.Pc == null; }
    IEnumerable<DexGame> DexOf(GameEntry only) {
        if (only != null) { DexGame d; return dexBy.TryGetValue(only, out d) ? new[] { d } : new DexGame[0]; }
        return dexBy.Where(x => CountsInCollection(x.Key)).Select(x => x.Value);
    }
    bool DexCaught(int id) { return DexCaught(id, null); }
    bool DexSeen(int id) { return DexSeen(id, null); }
    bool DexShiny(int id) { return DexShiny(id, null); }
    bool DexCaught(int id, GameEntry only) { return DexOf(only).Any(d => d.Caught.Contains(id)); }
    bool DexSeen(int id, GameEntry only) { return DexOf(only).Any(d => d.Seen.Contains(id) || d.Caught.Contains(id)); }
    bool DexShiny(int id, GameEntry only) { return DexOf(only).Any(d => d.Shiny.Contains(id)); }
    // What a game's Pokedex was read from, cheaply: when this changes, it's read again, and otherwise never.
    static string DexPrint(GameEntry g) {
        var o = new StringBuilder(g.Engine() + "|" + SaveSets.Active(g) + "|" + string.Join(",", SaveSets.All(g)));
        Action<string> file = delegate(string f) { try { var i = new FileInfo(f); if (i.Exists) o.Append("|" + i.Name + ":" + i.Length + ":" + i.LastWriteTimeUtc.Ticks); } catch { } };
        Action<string> folder = delegate(string d) { try { if (Directory.Exists(d)) foreach (var f in Directory.GetFiles(d, "*", SearchOption.AllDirectories).OrderBy(x => x, StringComparer.Ordinal)) file(f); } catch { } };
        // Read again when the list grows (to 1025 on 2026-11-04), as its later Pokemon move into it.
        if (PcGames.IsFusion(g)) { o.Append("|" + Sprites.MaxDex); foreach (var f in IfSaves(g)) file(f); return o.ToString(); }
        if (g.Save != null) file(MasterOf(g));
        folder(Path.Combine(Config.State, "save-sets", g.Id));
        if (g.Gen1 != null) folder(Path.Combine(Gen1Root, "saves", g.Gen1));
        return o.ToString();
    }
    class DexKept { public string Print; public DexGame Dex; }
    static readonly Dictionary<string, DexKept> dexKept = new Dictionary<string, DexKept>();
    static bool dexKeptRead;
    static string DexFile { get { return Path.Combine(Config.State, "dex-cache.txt"); } }
    static string Ids(HashSet<int> set) { return string.Join(",", set.OrderBy(x => x)); }
    static void ReadIds(string text, HashSet<int> into) { foreach (var id in text.Split(',')) { int n; if (int.TryParse(id, out n)) into.Add(n); } }
    static void ReadDexKept() {
        if (dexKeptRead) return;
        dexKeptRead = true;
        try {
            if (!File.Exists(DexFile)) return;
            foreach (var l in File.ReadAllLines(DexFile)) {
                var f = l.Split('\t');
                if (f.Length < 3) continue;
                DexGame d = null;
                if (f.Length >= 5) { d = new DexGame(); ReadIds(f[2], d.Caught); ReadIds(f[3], d.Seen); ReadIds(f[4], d.Shiny); }
                if (d != null && f.Length >= 7) { int.TryParse(f[5], out d.FusionsCaught); int.TryParse(f[6], out d.FusionsSeen); }
                if (d != null && f.Length >= 9) { int.TryParse(f[7], out d.LaterCaught); int.TryParse(f[8], out d.LaterSeen); }
                dexKept[f[0]] = new DexKept { Print = f[1], Dex = d };
            }
        } catch { }
    }
    static void SaveDexKept() {
        try {
            Directory.CreateDirectory(Config.State);
            File.WriteAllLines(DexFile, dexKept.Select(x => x.Key + "\t" + x.Value.Print + "\t" + (x.Value.Dex == null ? "none" : Ids(x.Value.Dex.Caught) + "\t" + Ids(x.Value.Dex.Seen) + "\t" + Ids(x.Value.Dex.Shiny) + (x.Value.Dex.FusionsSeen >= 0 ? "\t" + x.Value.Dex.FusionsCaught + "\t" + x.Value.Dex.FusionsSeen + "\t" + x.Value.Dex.LaterCaught + "\t" + x.Value.Dex.LaterSeen : ""))).ToArray());
        } catch { }
    }
    // Everything already known, on the entries of the latest scan. True when none of it is out of date.
    bool DexReady() {
        lock (dexKept) {
            ReadDexKept();
            if (dexNames.Count == 0) return false;
            var by = new Dictionary<GameEntry, DexGame>(); var none = new HashSet<GameEntry>(); bool all = true;
            foreach (var g in Library.Games.Where(ReadsDex)) {
                DexKept k;
                if (!dexKept.TryGetValue(g.Id, out k) || k.Print != DexPrint(g)) { all = false; continue; }
                if (k.Dex == null) none.Add(g); else by[g] = k.Dex;
            }
            if (all) { dexBy = by; dexNoSave = none; }
            return all;
        }
    }
    string LoadDex() { return LoadDex(false); }
    // In the background it gives way to anything the player starts, and keeps what it has so far.
    string LoadDex(bool background) {
        lock (dexKept) {
            ReadDexKept();
            if (dexNames.Count == 0) {
                int code; string o = Bridge("names", out code);
                if (code != 0) return "Couldn't read the Pokemon names.";
                foreach (var line in o.Split('\n')) { var f = line.TrimEnd('\r').Split('\t'); int id; if (f.Length == 2 && int.TryParse(f[0], out id)) dexNames[id] = f[1]; }
            }
            ReadDexStats();
            bool changed = false;
            foreach (var g in Library.Games.Where(ReadsDex).ToList()) {
                if (background && (GameArt.Hold || job != null || RunningGames() != "")) break;
                string print = DexPrint(g); DexKept k;
                if (dexKept.TryGetValue(g.Id, out k) && k.Print == print) continue;
                bool partial = false; DexGame d;
                if (g.Pc != null) { try { d = IfDex(g); } catch { d = null; } }
                else d = DexInAllSaves(g, !background, out partial);
                // Stopped or incomplete: it's read again later, not remembered wrong.
                if (partial || (background && GameArt.Hold)) continue;
                dexKept[g.Id] = new DexKept { Print = print, Dex = d }; changed = true;
            }
            if (changed) SaveDexKept();
            var by = new Dictionary<GameEntry, DexGame>(); var none = new HashSet<GameEntry>();
            foreach (var g in Library.Games.Where(ReadsDex)) { DexKept k; if (!dexKept.TryGetValue(g.Id, out k) || k.Dex == null) none.Add(g); else by[g] = k.Dex; }
            dexBy = by; dexNoSave = none;
        }
        return null;
    }
    void OpenCollection() {
        if (!GameArt.Capturing && DexReady()) { pages.Push(DexHomePage()); return; }
        if (AfterCapture(OpenCollection)) return;
        Busy("Opening your Pokedex", delegate { return LoadDex(false); }, delegate(string e) {
            if (e != null) { ShowText("Pokedex", e, null); return; }
            pages.Push(DexHomePage());
        });
    }
    string DexCount(int from, int to) { return Enumerable.Range(from, to - from + 1).Count(DexCaught) + " of " + (to - from + 1); }
    Page DexHomePage() {
        var p = new Page { Title = "Pokedex" };
        int last = DexRegionStart[DexRegions.Length] - 1;
        p.Items.Add(new Item("National: " + DexCount(1, last), "", true, delegate { pages.Push(DexListPage("National", 1, last)); }) { Icon = '\uE8F1', Info = "Every Pokemon from 001 to " + last.ToString("0000") + "." });
        for (int i = 0; i < DexRegions.Length; i++) {
            int a = DexRegionStart[i], b = DexRegionStart[i + 1] - 1; string name = DexRegions[i];
            p.Items.Add(new Item(name + ": " + DexCount(a, b), "", true, delegate { pages.Push(DexListPage(name, a, b)); }) { Icon = '\uE707', Info = "Numbers " + a.ToString("000") + " to " + b.ToString("000") + ", the Pokemon introduced in " + name + "." });
        }
        p.Items.Add(new Item("By game", "", true, delegate { pages.Push(DexGamesPage()); }) { Icon = '\uE7FC', Info = "How many each of your games has caught." });
        if (Library.Games.Any(OwnSection))
            p.Items.Add(new Item("Romhacks and fan games", "", true, delegate { pages.Push(DexOtherPage()); }) { Icon = '\uE734', Info = "Each with a Pokedex of its own. They never count toward your collection above." });
        return p;
    }
    // Romhacks you have, and Infinite Fusion once it's installed.
    static bool OwnSection(GameEntry g) { return (g.IsHack && g.Playable) || (PcGames.IsFusion(g) && g.OnPc); }
    Page DexOtherPage() {
        var p = new Page { Title = "Romhacks and fan games" };
        foreach (var game in Library.Games.Where(OwnSection)) {
            var g = game;
            bool has = dexBy.ContainsKey(g);
            if (g.Pc != null) {
                var d = has ? dexBy[g] : null;
                int caughtAll = has ? d.Caught.Count + d.LaterCaught : 0, seenAll = has ? d.Seen.Count + d.LaterSeen : 0;
                p.Items.Add(new Item(g.Title + ": " + (has ? caughtAll + " caught" : "no save"), "", has, delegate { pages.Push(DexListPage(g.Title, 1, DexRegionStart[DexRegions.Length] - 1, g)); }) {
                    Icon = '\uE7FC', Theme = g.Theme,
                    Info = has ? "Its own Pokedex, read from its saves: " + caughtAll + " caught and " + seenAll + " seen" + (d.LaterSeen > 0 ? " (" + d.LaterSeen + " of them outside the National Dex list)" : "") + ", and " + d.FusionsCaught + " fusions caught, " + d.FusionsSeen + " seen. It doesn't count toward your collection." : "No save to read yet, or its saves couldn't be read. Play and save once." });
                if (has && d.FusionsSeen >= 0) p.Items[p.Items.Count - 1].Records = new[] { "Fusions: " + d.FusionsCaught + " caught, " + d.FusionsSeen + " seen" };
                continue;
            }
            int last = Math.Min(g.Dex > 0 ? g.Dex : DexRegionStart[DexRegions.Length] - 1, DexRegionStart[DexRegions.Length] - 1);
            p.Items.Add(new Item(g.Title + ": " + (has ? dexBy[g].Caught.Count + " caught" : "no save"), "", has, delegate { pages.Push(DexListPage(g.Title, 1, last, g)); }) {
                Icon = '\uE7FC', Theme = g.Theme,
                Info = has ? "Its own Pokedex, read from its saves as " + (Library.Games.Where(x => x.Id == g.BaseId).Select(x => x.Title).FirstOrDefault() ?? "its base game") + "'s. It doesn't count toward your collection." : "No save to read yet. Play and save once." });
        }
        return p;
    }
    Page DexListPage(string title, int from, int to) { return DexListPage(title, from, to, null); }
    Page DexListPage(string title, int from, int to, GameEntry only) {
        var p = new Page { Title = title, Jump = 10, DexOnly = only };
        var order = Enumerable.Range(from, to - from + 1).Where(x => DexSeen(x, only)).ToArray();
        for (int n = from; n <= to; n++) {
            int id = n; bool caught = DexCaught(id, only), seen = DexSeen(id, only);
            string label = "#" + id.ToString("000") + " " + (seen ? DexName(id) : "???");
            if (DexShiny(id, only)) label += ": \u2605";
            else if (caught) label += ": Caught";
            p.Items.Add(new Item(label, seen ? "" : "Not seen yet", seen, delegate { pages.Push(DexEntryPage(id, order, only)); }) { Icon = '\uE9CE', Sprite = seen ? id.ToString() : null, Info = caught ? "Caught." : seen ? "Seen, not caught." : "Not seen yet." });
        }
        return p;
    }
    Page DexEntryPage(int id, int[] order) { return DexEntryPage(id, order, null); }
    Page DexEntryPage(int id, int[] order, GameEntry only) { return new Page { Title = "#" + id.ToString("000") + " " + DexName(id), DexId = id, DexOrder = order, DexShiny = DexShiny(id, only), DexOnly = only }; }
    Page DexGamesPage() {
        var games = Library.Games.Where(x => x.SaveTools && CountsInCollection(x)).ToList();
        var p = new Page { Title = "By game" };
        foreach (var g in games) {
            bool has = dexBy.ContainsKey(g);
            var it = new Item(g.Title + ": " + (has ? dexBy[g].Caught.Count + " of " + g.Dex : "no save"), "", has, null) { Icon = '\uE7FC', Theme = g.Theme };
            int saveCount = SaveSets.All(g).Count;
            it.Info = has ? (saveCount > 1 ? "Different Pokemon caught across your " + saveCount + " saves" : "Caught in this save") + ", out of the " + g.Dex + " entries in this game." : "No save to read yet. Play and save once.";
            if (has) {
                int only = dexBy[g].Caught.Count(n => !dexBy.Where(o => o.Key != g && CountsInCollection(o.Key)).Any(o => o.Value.Caught.Contains(n)));
                it.Records = new[] { "Only here: " + only };
            }
            p.Items.Add(it);
        }
        return p;
    }
    static readonly Dictionary<string, Color> TypeColors = new Dictionary<string, Color> {
        { "normal", Color.FromArgb(156, 156, 118) }, { "fire", Color.FromArgb(232, 118, 52) }, { "water", Color.FromArgb(84, 132, 232) }, { "electric", Color.FromArgb(232, 186, 36) },
        { "grass", Color.FromArgb(104, 178, 72) }, { "ice", Color.FromArgb(104, 192, 192) }, { "fighting", Color.FromArgb(186, 48, 42) }, { "poison", Color.FromArgb(158, 66, 158) },
        { "ground", Color.FromArgb(206, 170, 86) }, { "flying", Color.FromArgb(150, 124, 232) }, { "psychic", Color.FromArgb(236, 76, 126) }, { "bug", Color.FromArgb(160, 172, 42) },
        { "rock", Color.FromArgb(176, 150, 60) }, { "ghost", Color.FromArgb(108, 84, 148) }, { "dragon", Color.FromArgb(108, 56, 240) }, { "dark", Color.FromArgb(108, 84, 70) },
        { "steel", Color.FromArgb(160, 160, 190) }, { "fairy", Color.FromArgb(226, 130, 170) },
    };
    static string Titled(string s) { return string.Join(" ", s.Split('-').Select(w => w.Length == 0 ? w : char.ToUpper(w[0]) + w.Substring(1)).ToArray()); }
    // The game's own words: from a game you caught it in, otherwise the newest one nativeMons runs, otherwise the newest
    // game that has any. gen: that game's generation, whose stats and abilities the entry shows.
    string DexFlavor(DexInfo info, int id, out string from, out int gen) {
        from = null; gen = 9;
        var mine = dexBy.Where(x => x.Value.Caught.Contains(id)).Select(x => x.Key).ToList();
        mine.AddRange(Library.Games.AsEnumerable().Reverse());
        foreach (var g in mine) {
            string gid = g.BaseId ?? g.Id, key = gid == "black2" ? "black-2" : gid == "white2" ? "white-2" : gid, text;
            if (info.Flavor.TryGetValue(key, out text)) { from = g.Title; gen = GenOfDex(g.Dex); return text; }
        }
        if (info.Flavor.Count == 0) return null;
        var last = info.Flavor.Last();
        from = Titled(last.Key);
        return last.Value;
    }
    static readonly int[] GenLast = { 151, 251, 386, 493, 649, 721, 809, 905, 1025 };
    static int GenOfDex(int dex) { for (int i = 0; i < GenLast.Length; i++) if (dex > 0 && dex <= GenLast[i]) return i + 1; return 9; }
    // Each Pokemon as each generation had it (mon-bridge stats, from PKHeX's tables). Key: generation * 10000 + dex number.
    class DexStat { public string[] Types, Abilities; public int[] Stats; }
    static readonly Dictionary<int, DexStat> dexStats = new Dictionary<int, DexStat>();
    static void ReadDexStats() {
        lock (dexStats) {
            if (dexStats.Count > 0 || !File.Exists(Config.MonBridge)) return;
            int code; string o = Bridge("stats", out code);
            if (code != 0) return;
            foreach (var line in o.Split('\n')) {
                var f = line.TrimEnd('\r').Split('\t'); int gen, id;
                if (f.Length < 5 || !int.TryParse(f[0], out gen) || !int.TryParse(f[1], out id)) continue;
                dexStats[gen * 10000 + id] = new DexStat { Types = f[2].Split(','), Stats = f[3].Split(',').Select(int.Parse).ToArray(), Abilities = f[4] == "" ? new string[0] : f[4].Split(',') };
            }
        }
    }
    // As that generation had it, else the nearest later one that has it (Meltan isn't in any Generation 7 table).
    static DexStat DexStatOf(int id, ref int gen) {
        lock (dexStats) {
            DexStat s;
            for (int g = gen; g <= 9; g++) if (dexStats.TryGetValue(g * 10000 + id, out s)) { gen = g; return s; }
            for (int g = gen - 1; g >= 1; g--) if (dexStats.TryGetValue(g * 10000 + id, out s)) { gen = g; return s; }
            return null;
        }
    }
    void RenderBoxes(Graphics g, BoxView v, int w, int h, float m, float y, float bottom, float t, Theme th) {
        boxHits.Clear();
        var accent = Accent(th);
        const int cols = 6;
        float footH = smallFont.Height * 3.1f, gap = h * 0.008f, hh = itemFont.Height * 2.05f;
        // On the tablet the two games sit side by side; on the phone, one above the other.
        v.Wide = w > h * 1.1f;
        float paneW = v.Wide ? (w - 2 * m - gap * 3) / 2f : w - 2 * m, paneH = v.Wide ? bottom - y - footH - gap : (bottom - y - footH - gap) / 2f;
        RectangleF cursorRect = RectangleF.Empty; float heldCell = 0;
        for (int pane = 0; pane < 2; pane++) {
            var s = v.Side[pane];
            if (s == null) continue;
            float px = v.Wide ? m + pane * (paneW + gap * 3) : m, py = v.Wide ? y : y + pane * (paneH + gap);
            int rows = (s.Slots + cols - 1) / cols;
            float cell = Math.Min(paneW / cols, (paneH - hh - gap * 0.5f) / rows), gw = cell * cols, gx = px + (paneW - gw) / 2;
            // The game and box, across its side: arrows step through its boxes, the name changes the game.
            var head = new RectangleF(px, py, paneW, hh);
            var gt = s.Game.Theme;
            Card(g, head, hh * 0.3f, gt.Main, gt.Deep, Color.Empty, 0);
            if (v.Pane == pane && v.Cursor < 0) using (var ring = Round(head, hh * 0.3f)) using (var pen = new Pen(accent, Math.Max(2.5f, w * 0.007f))) g.DrawPath(pen, ring);
            float aw = hh * 0.9f;
            var left = new RectangleF(head.X, head.Y, aw, hh); var right = new RectangleF(head.Right - aw, head.Y, aw, hh);
            var mid = new RectangleF(head.X + aw, head.Y, head.Width - 2 * aw, hh);
            Write(g, "◀", itemFont, gt.Ink, left, StringAlignment.Center, StringAlignment.Center, false);
            Write(g, "▶", itemFont, gt.Ink, right, StringAlignment.Center, StringAlignment.Center, false);
            string gameName = s.Game.Title.Replace("Pokemon ", "");
            var nameArea = new RectangleF(mid.X, mid.Y + hh * 0.06f, mid.Width, hh * 0.5f);
            Write(g, gameName, FitFont(g, gameName, nameArea.Size, itemBold, 0.6f, false), gt.Ink, nameArea, StringAlignment.Center, StringAlignment.Center, false);
            string boxName = s.NoSave ? "No save yet" : s.Problem != null ? "Can't read its save" : (v.Box[pane] < s.Names.Count ? s.Names[v.Box[pane]] : "Box " + (v.Box[pane] + 1)) + "  ·  " + (v.Box[pane] + 1) + " of " + s.Boxes;
            var boxArea = new RectangleF(mid.X, mid.Y + hh * 0.5f, mid.Width, hh * 0.42f);
            Write(g, boxName, FitFont(g, boxName, boxArea.Size, smallFont, 0.6f, false), Fade(gt.Ink, 210), boxArea, StringAlignment.Center, StringAlignment.Center, false);
            boxHits.Add(new KeyValuePair<RectangleF, string>(left, "l" + pane));
            boxHits.Add(new KeyValuePair<RectangleF, string>(right, "r" + pane));
            boxHits.Add(new KeyValuePair<RectangleF, string>(mid, "h" + pane));
            // The box: one slot per place, its Pokemon's picture in it.
            float gy = head.Bottom + gap * 0.5f;
            var well = new RectangleF(gx, gy, gw, cell * rows);
            Group(g, well, cell * 0.16f);
            if (s.Problem != null) {
                // Nothing saved yet reads as a plain fact, large: its title, then what to do. A real fault stays small print.
                var inner = RectangleF.Inflate(well, -cell * 0.3f, -cell * 0.3f);
                if (s.NoSave) {
                    var lines = s.Problem.Split(new[] { "\n\n" }, StringSplitOptions.None);
                    float titleH = itemBold.Height * 1.4f;
                    Write(g, lines[0], itemBold, PhoneText, new RectangleF(inner.X, inner.Y + inner.Height / 2 - titleH, inner.Width, titleH), StringAlignment.Center, StringAlignment.Far, false);
                    if (lines.Length > 1) Write(g, lines[1], smallFont, PhoneSub, new RectangleF(inner.X, inner.Y + inner.Height / 2 + itemBold.Height * 0.2f, inner.Width, inner.Height / 2), StringAlignment.Center, StringAlignment.Near, true);
                } else Write(g, s.Problem, smallFont, PhoneSub, inner, StringAlignment.Center, StringAlignment.Center, true);
                continue;
            }
            // The slots first, then the Pokemon over them, as their pictures reach past their own slot.
            for (int i = 0; i < s.Slots; i++) {
                var r = new RectangleF(gx + (i % cols) * cell, gy + (i / cols) * cell, cell, cell);
                Card(g, RectangleF.Inflate(r, -cell * 0.06f, -cell * 0.06f), cell * 0.16f, Color.FromArgb(242, 242, 247), Color.FromArgb(232, 232, 238), Color.Empty, 0);
                boxHits.Add(new KeyValuePair<RectangleF, string>(r, "s" + pane + ":" + i));
            }
            for (int i = 0; i < s.Slots; i++) {
                var r = new RectangleF(gx + (i % cols) * cell, gy + (i / cols) * cell, cell, cell);
                var inner = RectangleF.Inflate(r, -cell * 0.06f, -cell * 0.06f);
                var mon = s.At(v.Box[pane], i);
                bool fromHere = v.Holding && v.Side[v.HeldPane] == s && v.HeldBox == v.Box[pane] && v.HeldSlot == i;
                if (mon != null && !fromHere) {
                    string key = (mon.Shiny ? "shiny/" : "") + mon.Dex;
                    var pic = Sprites.Get(key);
                    // Sprites carry wide empty margins: drawn larger than the slot, the Pokemon itself fills it.
                    if (pic != null) DrawSprite(g, pic, RectangleF.Inflate(inner, cell * 0.22f, cell * 0.22f));
                    else if (Sprites.Loading(key)) { SpinningBall(g, inner.X + inner.Width / 2, inner.Y + inner.Height / 2, inner.Width * 0.55f, t); spinning = true; }
                    else Write(g, "#" + mon.Dex, smallFont, PhoneSub, inner, StringAlignment.Center, StringAlignment.Center, false);
                    if (mon.Shiny) Write(g, "★", smallFont, Color.FromArgb(232, 170, 20), new RectangleF(inner.X + cell * 0.04f, inner.Y, inner.Width, smallFont.Height * 1.2f), StringAlignment.Near, StringAlignment.Near, false);
                }
                if (fromHere) using (var ring = Round(inner, cell * 0.16f)) using (var pen = new Pen(Fade(accent, 200), Math.Max(1.5f, cell * 0.04f)) { DashStyle = System.Drawing.Drawing2D.DashStyle.Dash }) g.DrawPath(pen, ring);
                if (v.Pane == pane && v.Cursor == i) {
                    cursorRect = inner; heldCell = cell;
                    using (var ring = Round(inner, cell * 0.16f)) using (var pen = new Pen(accent, Math.Max(2.5f, cell * 0.06f))) g.DrawPath(pen, ring);
                }
                if (heldCell == 0) heldCell = cell;
            }
            // The game the cursor isn't in steps back, as HOME's does.
            if (v.Pane != pane && !v.Dragging) using (var veil = new SolidBrush(Color.FromArgb(110, PhoneBg))) using (var path = Round(RectangleF.Inflate(well, cell * 0.02f, cell * 0.02f), cell * 0.16f)) g.FillPath(veil, path);
        }
        // The Pokemon in hand: under the pointer while dragging, else just above the cursor.
        BoxMon held = v.Holding ? v.Side[v.HeldPane].At(v.HeldBox, v.HeldSlot) : null;
        if (held != null) {
            float size = heldCell * 1.8f;
            PointF at = v.Dragging ? v.Drag : !cursorRect.IsEmpty ? new PointF(cursorRect.X + cursorRect.Width / 2, cursorRect.Y - size * 0.15f) : PointF.Empty;
            if (!at.IsEmpty) {
                var r = new RectangleF(at.X - size / 2, at.Y - size / 2, size, size);
                using (var shadow = new SolidBrush(Color.FromArgb(60, 0, 0, 0))) g.FillEllipse(shadow, r.X + size * 0.15f, r.Bottom - size * 0.18f, size * 0.7f, size * 0.16f);
                string key = (held.Shiny ? "shiny/" : "") + held.Dex;
                var pic = Sprites.Get(key);
                if (pic != null) DrawSprite(g, pic, r); else { SpinningBall(g, at.X, at.Y, size * 0.5f, t); spinning = true; }
            }
        }
        // What's under the cursor, or what to do; a note when a drop didn't happen.
        var foot = new RectangleF(m, bottom - footH, w - 2 * m, footH);
        var side = v.Side[v.Pane];
        BoxMon under = side != null && v.Cursor >= 0 ? side.At(v.Box[v.Pane], v.Cursor) : null;
        var show = held ?? under;
        string first = v.Note ?? (show != null ? (show.Nick != "" && !string.Equals(show.Nick, show.Species, StringComparison.OrdinalIgnoreCase) ? show.Nick + " (" + show.Species + ")" : show.Species) + "   Lv " + show.Level + (show.Shiny ? "   ★ shiny" : "") + "   OT " + show.Trainer : "");
        var hints = v.Holding ? new[] { "A", "Put down", "X", "Other game", "LB RB", "Box", "B", "Put back" }
            : v.Cursor < 0 ? new[] { "A", "Change game", "◀ ▶", "Box", "X", "Other game", "B", "Back" }
            : new[] { "A", "Pick up", "X", "Other game", "LB RB", "Box", "B", "Back" };
        Write(g, first, v.Note != null ? smallBold : smallBold, v.Note != null ? Color.FromArgb(200, 60, 50) : PhoneText, new RectangleF(foot.X, foot.Y, foot.Width, footH * 0.45f), StringAlignment.Center, StringAlignment.Center, false);
        ButtonHints(g, hints, new RectangleF(foot.X, foot.Y + footH * 0.45f, foot.Width, footH * 0.55f));
        if (v.Dragging) spinning = true;   // keep drawing while the pointer moves
    }
    // Buttons and what they do, in a row: each button a dark chip, as the Switch shows them.
    void ButtonHints(Graphics g, string[] pairs, RectangleF r) {
        float chipH = smallBold.Height * 1.45f, sp = chipH * 0.3f, total = 0;
        var widths = new float[pairs.Length];
        for (int i = 0; i < pairs.Length; i++) {
            widths[i] = g.MeasureString(pairs[i], smallBold).Width + (i % 2 == 0 ? chipH * 0.3f : 0);
            if (i % 2 == 0) widths[i] = Math.Max(widths[i], chipH);
            total += widths[i] + (i % 2 == 0 ? sp * 0.5f : sp * 2.2f);
        }
        float x = r.X + (r.Width - total) / 2, cy = r.Y + (r.Height - chipH) / 2;
        for (int i = 0; i < pairs.Length; i++) {
            var cr = new RectangleF(x, cy, widths[i], chipH);
            if (i % 2 == 0) {
                Card(g, cr, chipH / 2, Color.FromArgb(60, 60, 67), Color.FromArgb(40, 40, 46), Color.Empty, 0);
                Write(g, pairs[i], smallBold, Color.White, cr, StringAlignment.Center, StringAlignment.Center, false);
                x += widths[i] + sp * 0.5f;
            } else {
                Write(g, pairs[i], smallBold, PhoneText, new RectangleF(x, cy - chipH, widths[i] + sp, chipH * 3), StringAlignment.Near, StringAlignment.Center, false);
                x += widths[i] + sp * 2.2f;
            }
        }
    }
    void RenderDexEntry(Graphics g, Page p, int w, int h, float m, float y, float bottom, float t, Theme th) {
        int id = p.DexId; bool caught = DexCaught(id, p.DexOnly);
        var area = new RectangleF(0, y, w, bottom - y);
        float view = area.Height, width = w - 2 * m, pad = w * 0.04f, rad = w * 0.04f, gap = h * 0.016f;
        p.BodyScroll = Math.Max(0, Math.Min(p.BodyScroll, Math.Max(0, p.DexHeight - view)));
        var kept = g.Save();
        g.SetClip(area, System.Drawing.Drawing2D.CombineMode.Intersect);
        float top0 = y - p.BodyScroll, cy = top0;
        var info = caught ? DexInfos.Get(id) : null;
        string from = null, flavor = null; int gen = 9; DexStat era = null;
        if (info != null) { flavor = DexFlavor(info, id, out from, out gen); era = DexStatOf(id, ref gen); }
        var types = era != null ? era.Types : info != null ? info.Types : new string[0];
        // The picture and the facts beside it.
        float pic = width * 0.42f, headH = pic + pad * 0.6f;
        Group(g, new RectangleF(m, cy, width, headH), rad);
        string spriteKey = (p.DexShiny ? "shiny/" : "") + id;
        var sprite = Sprites.Get(spriteKey);
        if (sprite != null) DrawSprite(g, sprite, new RectangleF(m + pad * 0.3f, cy + pad * 0.3f, pic, pic));
        else if (Sprites.Loading(spriteKey)) { SpinningBall(g, m + pad * 0.3f + pic / 2, cy + pad * 0.3f + pic / 2, pic * 0.32f, t); spinning = true; }
        if (p.DexShiny) Write(g, "\u2605", titleFont, Color.FromArgb(232, 170, 20), new RectangleF(m + pad * 0.6f, cy + pad * 0.4f, pic, titleFont.Height * 1.2f), StringAlignment.Near, StringAlignment.Near, false);
        float fx = m + pic + pad * 0.8f, fw = width - pic - pad * 1.2f, fy = cy + pad;
        if (!caught) {
            Write(g, "Seen, not caught yet.", itemFont, PhoneSub, new RectangleF(fx, fy, fw, headH - 2 * pad), StringAlignment.Near, StringAlignment.Near, true);
        } else if (info == null) {
            Write(g, "Getting this entry. It needs the internet the first time.", smallFont, PhoneSub, new RectangleF(fx, fy, fw, headH - 2 * pad), StringAlignment.Near, StringAlignment.Near, true);
            if (DexInfos.Loading(id)) { float b = Math.Min(fw, headH) * 0.28f; SpinningBall(g, fx + b * 0.6f, cy + headH - pad - b * 0.6f, b, t); spinning = true; }
        } else {
            Write(g, info.Genus, itemFont, PhoneText, new RectangleF(fx, fy, fw, itemFont.Height * 2.4f), StringAlignment.Near, StringAlignment.Near, true);
            fy += g.MeasureString(info.Genus, itemFont, (int)fw).Height + pad * 0.4f;
            float tx = fx, chipH = smallBold.Height * 1.5f;
            foreach (var type in types) {
                string label = Titled(type); float cw = g.MeasureString(label, smallBold).Width + chipH * 0.5f;
                if (tx + cw > fx + fw) { tx = fx; fy += chipH * 1.15f; }
                Color tc; if (!TypeColors.TryGetValue(type, out tc)) tc = PhoneSub;
                Card(g, new RectangleF(tx, fy, cw, chipH), chipH / 2, tc, tc, Color.Empty, 0);
                Write(g, label, smallBold, Color.White, new RectangleF(tx, fy, cw, chipH), StringAlignment.Center, StringAlignment.Center, false);
                tx += cw + chipH * 0.25f;
            }
            fy += chipH + pad * 0.6f;
            Write(g, "Height", smallFont, PhoneSub, new RectangleF(fx, fy, fw, smallFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
            Write(g, (info.Height / 10.0).ToString("0.0") + " m", smallBold, PhoneText, new RectangleF(fx, fy, fw, smallFont.Height * 1.3f), StringAlignment.Far, StringAlignment.Near, false);
            fy += smallFont.Height * 1.4f;
            Write(g, "Weight", smallFont, PhoneSub, new RectangleF(fx, fy, fw, smallFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
            Write(g, (info.Weight / 10.0).ToString("0.0") + " kg", smallBold, PhoneText, new RectangleF(fx, fy, fw, smallFont.Height * 1.3f), StringAlignment.Far, StringAlignment.Near, false);
        }
        cy += headH + gap;
        if (caught && info != null) {
            // The game's own description.
            if (flavor != null) {
                float textW = width - 2 * pad, textH = g.MeasureString(flavor, bodyFont, (int)textW).Height + 2, boxH = textH + smallFont.Height * 1.5f + pad * 1.4f;
                Group(g, new RectangleF(m, cy, width, boxH), rad);
                Write(g, flavor, bodyFont, PhoneText, new RectangleF(m + pad, cy + pad * 0.7f, textW, textH), StringAlignment.Near, StringAlignment.Near, true);
                Write(g, from.Replace("Pokemon ", ""), smallFont, PhoneSub, new RectangleF(m + pad, cy + pad * 0.7f + textH + 2, textW, smallFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
                cy += boxH + gap;
            }
            // Base stats, as that game's generation had them: Generation 1 had one Special.
            int[] stats = era != null ? era.Stats : info.Stats;
            bool special = era != null && gen == 1;
            string[] names = special ? new[] { "HP", "Attack", "Defense", "Special", "Speed" } : new[] { "HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed" };
            int[] pick = special ? new[] { 0, 1, 2, 3, 5 } : new[] { 0, 1, 2, 3, 4, 5 };
            string era0 = era == null ? "Base stats" : "Base stats" + (from != null ? ", as in " + from.Replace("Pokemon ", "") : "") + " (Generation " + gen + ")";
            float rowH = smallFont.Height * 1.75f, statsH = rowH * (names.Length + 2) + pad * 0.8f, nameW = width * 0.22f, numW = width * 0.1f;
            Group(g, new RectangleF(m, cy, width, statsH), rad);
            float sy = cy + pad * 0.4f, barX = m + pad + nameW + numW, barW = width - 2 * pad - nameW - numW, barH = Math.Max(4f, h * 0.008f);
            Write(g, era0, smallFont, PhoneSub, new RectangleF(m + pad, sy, width - 2 * pad, rowH), StringAlignment.Near, StringAlignment.Center, false);
            sy += rowH;
            var stroke = Accent(th);
            int total = 0;
            for (int i = 0; i < names.Length && pick[i] < stats.Length; i++) {
                int v = stats[pick[i]]; total += v;
                Write(g, names[i], smallFont, PhoneSub, new RectangleF(m + pad, sy, nameW, rowH), StringAlignment.Near, StringAlignment.Center, false);
                Write(g, v.ToString(), smallBold, PhoneText, new RectangleF(m + pad + nameW, sy, numW * 0.8f, rowH), StringAlignment.Far, StringAlignment.Center, false);
                var bar = new RectangleF(barX, sy + (rowH - barH) / 2, barW, barH);
                Card(g, bar, barH / 2, PhoneLine, PhoneLine, Color.Empty, 0);
                Card(g, new RectangleF(bar.X, bar.Y, Math.Max(barH, barW * Math.Min(1f, v / 255f)), barH), barH / 2, stroke, stroke, Color.Empty, 0);
                sy += rowH;
            }
            Write(g, "Total", smallBold, PhoneText, new RectangleF(m + pad, sy, nameW, rowH), StringAlignment.Near, StringAlignment.Center, false);
            Write(g, total.ToString(), smallBold, PhoneText, new RectangleF(m + pad + nameW, sy, numW * 0.8f, rowH), StringAlignment.Far, StringAlignment.Center, false);
            cy += statsH + gap;
            // Abilities came with Generation 3.
            if (era == null || gen >= 3) {
                string abilities = era != null ? string.Join(", ", era.Abilities) : string.Join(", ", info.Abilities.Select(Titled).ToArray());
                float abH = g.MeasureString(abilities, itemFont, (int)(width - 2 * pad)).Height + smallFont.Height * 1.3f + pad;
                Group(g, new RectangleF(m, cy, width, abH), rad);
                Write(g, "Abilities", smallFont, PhoneSub, new RectangleF(m + pad, cy + pad * 0.5f, width - 2 * pad, smallFont.Height * 1.3f), StringAlignment.Near, StringAlignment.Near, false);
                Write(g, abilities, itemFont, PhoneText, new RectangleF(m + pad, cy + pad * 0.5f + smallFont.Height * 1.3f, width - 2 * pad, abH), StringAlignment.Near, StringAlignment.Near, true);
                cy += abH + gap;
            }
        }
        // Where you have it.
        var rows = dexBy.Where(x => (p.DexOnly != null ? x.Key == p.DexOnly : CountsInCollection(x.Key)) && (x.Value.Caught.Contains(id) || x.Value.Seen.Contains(id))).OrderBy(x => x.Value.Caught.Contains(id) ? 0 : 1).ToList();
        float gameH = h * 0.068f;
        if (rows.Count > 0) {
            Group(g, new RectangleF(m, cy, width, gameH * rows.Count), rad);
            foreach (var row in rows) {
                float icon = gameH * 0.62f;
                Cart(g, m + pad, cy + (gameH - icon) / 2, icon, row.Key.Theme);
                Write(g, row.Key.Title.Replace("Pokemon ", ""), itemFont, PhoneText, new RectangleF(m + pad * 1.4f + icon, cy, width * 0.5f, gameH), StringAlignment.Near, StringAlignment.Center, false);
                bool shiny = row.Value.Shiny.Contains(id);
                string mark = shiny ? "\u2605 Shiny" : row.Value.Caught.Contains(id) ? "Caught" : "Seen";
                Write(g, mark, shiny ? smallBold : smallFont, shiny ? Color.FromArgb(196, 138, 8) : PhoneSub, new RectangleF(m, cy, width - pad, gameH), StringAlignment.Far, StringAlignment.Center, false);
                cy += gameH;
            }
            cy += gap;
        }
        p.DexHeight = cy - top0;
        g.Restore(kept);
        if (p.DexHeight > view) {
            float barW = Math.Max(3f, w * 0.008f), barH = view * view / p.DexHeight, barY = y + (view - barH) * p.BodyScroll / (p.DexHeight - view);
            using (var path = Round(new RectangleF(w - m * 0.5f, barY, barW, barH), barW / 2)) using (var b = new SolidBrush(Color.FromArgb(120, 120, 120, 128))) g.FillPath(b, path);
        }
    }
    // Original games, then romhacks and fan games, each section folding away (kept in games-folded.txt).
    Page GamesPage() { return GamesPage(null); }
    Page GamesPage(string at) {
        var p = new Page { Title = "Games" };
        var folded = Config.Read("games-folded.txt", "").Split(new[] { '\n', '\r' }, StringSplitOptions.RemoveEmptyEntries).ToList();
        var sections = new[] {
            new { Key = "originals", Name = "Original games", Games = Library.Games.Where(x => !x.IsHack && x.Pc == null).ToList() },
            new { Key = "hacks", Name = "Romhacks and fan games", Games = Library.Games.Where(x => x.IsHack || x.Pc != null).ToList() } };
        foreach (var sec in sections) {
            if (sec.Games.Count == 0) continue;
            var key = sec.Key; bool open = !folded.Contains(key);
            if (key == at) p.Sel = p.Items.Count;
            p.Items.Add(new Item(sec.Name + ": " + sec.Games.Count, "", true, delegate {
                if (open) folded.Add(key); else folded.Remove(key);
                Config.Write("games-folded.txt", string.Join("\n", folded.ToArray()));
                pages.Pop(); pages.Push(GamesPage(key));
            }) { Fold = open ? 1 : 2, Info = (open ? "Select or press Left to hide these." : "Select or press Right to show these.") + " " + sec.Games.Count(x => x.Playable) + " of " + sec.Games.Count + " ready to play." });
            if (open) AddGameRows(p, sec.Games);
        }
        if (p.Items.Count == 0) p.Items.Add(new Item("No games listed", Library.Note, false, null));
        p.Items.Add(new Item("Add a PC game", "Any game with its own program, such as a fan game.", true, delegate { AddPcGame(); }) { Icon = '', Info = "From its folder (run where it is), its .zip (unpacked into the launcher) or its .exe. It's run as it is, with no extras: the launcher starts it and shows it on Discord, and leaves its saves to the game." });
        return p;
    }
    // Numbers compare as numbers, so 1.3.1 comes after 1.3 and 1.10 after 1.9.
    static string Natural(string s) { return System.Text.RegularExpressions.Regex.Replace(s ?? "", @"\d+", m => m.Value.PadLeft(10, '0')); }
    static string Family(string title) { return System.Text.RegularExpressions.Regex.Replace(title ?? "", @"\s+v?\d[\w.\- ]*$", ""); }
    void AddGameRows(Page p, List<GameEntry> games) {
        // A hack's versions sit together under its base game, newest first.
        var order = games.Select((x, i) => new { x, i }).ToDictionary(o => o.x, o => o.i);
        var bases = Library.Games.Select((x, i) => new { x.Id, i }).ToDictionary(o => o.Id, o => o.i);
        Func<GameEntry, int> baseAt = x => { int i; return x.BaseId != null && bases.TryGetValue(x.BaseId, out i) ? i : order[x]; };
        foreach (var game in games.OrderBy(g => g.Playable ? 0 : 1).ThenBy(baseAt).ThenBy(g => Family(g.Title)).ThenByDescending(g => Natural(g.Title))) {
            var g = game;
            string why = Library.WhyNotPlayable(g);
            // A PC game not yet installed opens its page, where it's installed.
            var it = new Item(g.Title, g.Playable ? "" : why, g.Playable || g.Pc != null, delegate { pages.Push(GamePage(g)); }) { Icon = '', Theme = g.Theme };
            it.Info = g.Pc != null ? (g.Playable ? "A PC game of its own, installed in the launcher." : why) : g.Playable ? g.Console + ". Runs with " + GameEntry.EngineName(g.Engine()) + "." : g.Console + ". " + why;
            it.Records = GameRecords(g);
            p.Items.Add(it);
        }
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
        if (g.Pc != null) return PcGamePage(g);
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
        if (g.Roms.Count > 1) p.Items.Add(new Item("ROM version: " + g.LabelOf(g.Chosen), "", true, delegate { pages.Push(RomsPage(g)); }) { Icon = '\uE8B7', Info = g.Roms.Count + " different dumps of this game were found. " + Path.GetFileName(g.Chosen.File) + " is in use." });
        if (g.IsHack) {
            var baseGame = Library.Games.Find(x => x.Id == g.BaseId);
            string of = baseGame != null ? baseGame.Title : g.BaseId;
            p.Items.Add(new Item("Rename", "Shown as " + g.Title + ".", g.Chosen != null, delegate {
                string name = AskText("Rename", "Name for this game:", g.Title);
                if (string.IsNullOrWhiteSpace(name)) return;
                Library.RenameHack(g.Chosen.Sha1, name.Trim());
                string id = g.Id;
                Rescan(delegate { pages.Pop(); var fresh = Library.Games.Find(x => x.Id == id); if (fresh != null) pages.Push(GamePage(fresh)); ReplaceMain(); });
            }) { Icon = '\uE70F', Info = "A romhack of " + of + ", named after its file" + (g.Chosen != null ? " (" + Path.GetFileName(g.Chosen.File) + ")" : "") + ". It runs on Fulldex, with a save of its own, and its saves read as " + of + "'s do: for Move Pokemon, My Collection and PKHeX." });
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
    // Which engines can use a dump, in words.
    string RomEngines(GameEntry g, RomFile r) {
        var yes = new List<string>(); var no = new List<string>();
        if (g.FulldexCode != null) { if (r.Encrypted) no.Add("Fulldex until it's patched"); else yes.Add("Fulldex"); }
        if (g.Port != null) yes.Add("Platinum PC port");
        if (g.Gen1 != null && !g.IsModified(r)) { if (r.Gen1Ok) yes.Add("Gen1Recomp"); else no.Add("Gen1Recomp"); }
        if (g.Nativeplat != null) { if (r.NativeplatOk) yes.Add("nativeplat"); else no.Add("nativeplat"); }
        return (yes.Count > 0 ? "Runs on " + string.Join(", ", yes.ToArray()) + "." : "") + (no.Count > 0 ? " Not on " + string.Join(", ", no.ToArray()) + "." : "");
    }
    Page RomsPage(GameEntry g) {
        var p = new Page { Title = "ROM version: " + g.Title.Replace("Pokemon ", ""), Theme = g.Theme };
        foreach (var rom in g.Roms.OrderByDescending(r => r.Revision).ThenBy(r => r.File)) {
            var r = rom; bool now = r == g.Chosen;
            p.Items.Add(new Item(g.LabelOf(r) + (now ? ": In use" : ""), "", true, delegate { ChooseVersion(g, r); }) { Icon = '\uE8B7', TileHint = Path.GetFileName(r.File), Info = RomEngines(g, r).Trim() + (g.IsModified(r) ? " A romhack or changed copy, not a clean one. It only runs on Fulldex, and has its own save there, separate from this game's." : " Your saves stay with this game whichever version you choose.") + (g.IsModified(r) ? "" : r.Verified || !g.CanVerify ? "" : " No engine here has verified this dump: it may be modified."), Records = new[] { "File: " + r.File, "SHA-1: " + r.Sha1 } });
        }
        return p;
    }
    void ChooseVersion(GameEntry g, RomFile r) {
        if (r == g.Chosen) { pages.Pop(); return; }
        if (AfterCapture(delegate { ChooseVersion(g, r); })) return;
        string running = RunningGames();
        if (running != "") { ShowText("Close your game first", "Already running: " + running + ". Close it, then change version.", null); return; }
        Config.Write("rom-" + g.Id + ".txt", r.Sha1);
        string oldGen1 = g.Gen1Rom; string id = g.Id;
        pages.Pop();
        Rescan(delegate {
            var fresh = Library.Games.Find(x => x.Id == id);
            if (fresh == null) return;
            // A scan makes new game entries, so the page behind this one still holds the old ROM.
            pages.Pop(); pages.Push(GamePage(fresh));
            // Gen1Recomp keeps game data made from its ROM, so it's made again, with saves kept. The town picture is the same game's.
            if (fresh.Imported && fresh.Gen1Rom != null && !string.Equals(oldGen1, fresh.Gen1Rom, StringComparison.OrdinalIgnoreCase)) Import(fresh, null);
        });
    }
    // The patcher needs Python 3: run `go` with it, or first offer to fetch it (PythonSetup) when there is none.
    void WithPython(Action go) {
        if (PythonSetup.Works()) { go(); return; }
        var p = new Page { Title = "Python 3 needed" };
        p.Body = "hails.nds-Patcher is a Python script, and no working Python 3 was found on this PC.\n\nnativeMons can get Python's official Windows package from python.org (" + PythonSetup.Size + ", checked against a known checksum) and keep it in its own tools\\python folder, just for the patcher. Nothing is installed in Windows, and deleting that folder removes it.";
        p.Items.Add(new Item("Get Python", "Downloads it from python.org, then patches.", true, delegate {
            pages.Pop();
            Busy("Getting Python", delegate { return PythonSetup.Install(); }, delegate(string err) {
                if (err != null) { ShowLog("Python couldn't be set up", err); return; }
                go();
            });
        }) { Icon = '' });
        p.Items.Add(new Item("I'll install it myself", "Opens python.org's download page.", true, delegate {
            pages.Pop();
            try { Process.Start("https://www.python.org/downloads/windows/"); } catch { }
        }) { Icon = '' });
        p.Items.Add(new Item("Not now", "Nothing is changed.", true, delegate { pages.Pop(); }) { Icon = '' });
        pages.Push(p);
    }
    Page PatchPage(GameEntry g, bool thenPlay) {
        var p = new Page { Title = "Patch " + Path.GetFileName(g.FulldexRom) + "?", Theme = g.Theme };
        p.Body = "This ROM's secure area is still encrypted, so Fulldex can't load it without a DS BIOS.\n\nhails.nds-Patcher decrypts it. That changes 2 KB of your ROM file and keeps the original bytes in a backup, so it can be undone.\n\nIf no key table is available it downloads the DeSmuME emulator from GitHub to get one.";
        p.Items.Add(new Item("Patch it", "Changes the ROM file, with a backup.", File.Exists(Config.Patcher), delegate {
            pages.Pop();
            WithPython(delegate {
                RunTool("Patching " + g.Title, Config.Python, "\"" + Config.Patcher + "\" \"" + g.FulldexRom + "\"", Path.GetDirectoryName(Config.Patcher), delegate(int code) {
                    if (code != 0) { ShowLog("Patching failed", "The patcher stopped with code " + code + "."); return; }
                    if (thenPlay) Play(g); else { pages.Pop(); pages.Push(GamePage(g)); }
                });
            });
        }) { Icon = '' });
        p.Items.Add(new Item("Not now", "Nothing is changed.", true, delegate { pages.Pop(); }) { Icon = '' });
        return p;
    }
    Page ModGamesPage() {
        var p = new Page { Title = "Mods: choose a game", Mods = true };
        foreach (var game in Library.Games.Where(x => x.OnGen1)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { pages.Push(ModsPage(g)); }) { Icon = '', Theme = g.Theme, TileHint = g.Console, Info = "Each game has its own mods, switched on and off for it alone." });
        }
        return p;
    }
    // A game's mods: Find mods, then what's installed, then installing from a file.
    Page ModsPage(GameEntry g) {
        var p = new Page { Title = "Mods: " + g.Title, Theme = g.Theme, Mods = true };
        string list = Gen1Cli("list " + g.Gen1) ?? "";
        var installed = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var row in list.Split('\n')) { var f = row.TrimEnd('\r').Split('\t'); if (f.Length >= 3) installed[f[0]] = f[2]; }
        int listed = ModIndex.CachedCount();
        p.Items.Add(new Item("Find mods", "", true, delegate { FindMods(g, installed); }) {
            Icon = '\uE721', TileHint = listed > 0 ? listed + " mods in the community index" : "Browse the community index",
            Info = "Browse the Gen1Recomp community mod index and install from it. The mods are their authors' own, and being listed isn't a check of them." });
        var open = ModDeps.Open(g);
        if (open.Count > 0) {
            int needed = open.Count(x => x.Required);
            p.Items.Add(new Item("Missing mods: " + open.Count, "", true, delegate { pages.Push(DepsPage(g, null)); }) {
                Icon = '\uE7BA', Tint = Warn, TileHint = needed > 0 ? needed + " needed for a mod to load" : "Optional extras your mods can use",
                Info = "Mods you installed ask for others: " + string.Join(", ", open.Select(x => x.Id).ToArray()) + ". Select to choose where each comes from." });
        }
        var clashes = ModClashes.Read(g);
        int active = clashes.Count(c => c.Active);
        if (active > 0) p.Items.Add(new Item("Clashes: " + active, "", true, delegate { pages.Push(ClashesPage(g, null)); }) {
            Icon = '\uE7BA', Tint = Warn, TileHint = active == 1 ? "A mod won't load until you choose" : "Mods won't load until you choose",
            Info = "Some mods say they don't work with others that are on. While both are on, the game skips the one that says so. Select to choose which to keep." });
        int mine = 0;
        foreach (var row in list.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 6 || f[4] != "yes") continue;
            string id = f[0], name = f[1]; bool on = f[3] == "on";
            string clash = ModClashes.Note(clashes, id);
            Item item = null;
            item = new Item(name + (on ? ": On" : ": Off"), "", true, delegate {
                // Switching on beside a mod it clashes with: ask first.
                var against = !on ? clashes.FirstOrDefault(c => (c.ModId == id && c.OtherOn) || (c.OtherId == id && c.ModOn)) : null;
                if (against != null) { pages.Push(ClashChoicePage(g, id, name, against)); return; }
                on = !on;
                Gen1Cli("set " + g.Gen1 + " " + id + " " + (on ? "on" : "off"));
                int sel = p.Sel; pages.Pop(); var fresh = ModsPage(g); fresh.Sel = Math.Min(sel, fresh.Items.Count - 1); pages.Push(fresh);
            }) { Icon = clash != null ? '\uE7BA' : '\uE74C', Tint = clash != null ? Warn : Color.Empty, TileHint = clash ?? "Version " + f[2], Info = f[5] + (f[5] != "" ? "\n" : "") + (clash != null ? clash + ". " : "") + "Select to switch it on or off for " + g.Title.Replace("Pokemon ", "") + "." };
            p.Items.Add(item); mine++;
        }
        if (mine == 0) p.Items.Add(new Item("No mods installed yet", "", false, null) { Icon = '\uE74C', Hint = "Find mods above, or install one from a file below." });
        p.Items.Add(new Item("Install from a .zip file", "", true, delegate { InstallMod(g); }) { Icon = '\uE896', TileHint = "A mod you downloaded yourself", Info = "Copies a mod into the game's mods folder. Only install mods you trust: they run code in the game." });
        return p;
    }
    // The community mod index, for one game.
    void FindMods(GameEntry g, Dictionary<string, string> installed) {
        List<ModIndex.Entry> all = null; string note = null, problem = null;
        Busy("Reading the mod index", delegate { try { all = ModIndex.Load(out note); } catch (Exception x) { problem = x.Message; } return null; }, delegate(string e) {
            if (e != null || problem != null) { ShowText("Find mods", e ?? problem, null); return; }
            pages.Push(FindModsPage(g, installed, all, note));
        });
    }
    // What the list shows: one kind or all, most downloaded or A to Z. Kept while the launcher runs.
    string modKind; bool modAlphabetical;
    static string KindName(string c) { return c == "UI" ? "UI" : c == "QOL" ? "Quality of life" : Titled(c.ToLowerInvariant().Replace('_', '-')).Replace('-', ' '); }
    static char KindIcon(string c) {
        switch (c) {
            case "GAMEPLAY": return '\uE7FC'; case "CONTENT": return '\uE8A5'; case "BALANCE": return '\uE8CB'; case "ART": return '\uE790';
            case "AUDIO": return '\uE8D6'; case "UI": return '\uE8A9'; case "QOL": return '\uE713'; case "TRANSLATION": return '\uE774';
            case "TOTAL_CONVERSION": return '\uE7B8'; case "LIBRARY": return '\uE8F1'; case "TOOL": return '\uE90F'; default: return '\uE74C';
        }
    }
    Page FindModsPage(GameEntry g, Dictionary<string, string> installed, List<ModIndex.Entry> all, string note) {
        var fits = all.Where(x => ModIndex.Fits(x, g.Gen1)).ToList();
        var kinds = fits.SelectMany(x => x.Categories).GroupBy(x => x).OrderByDescending(x => x.Count()).Select(x => x.Key).ToList();
        if (modKind != null && !kinds.Contains(modKind)) modKind = null;
        var shown = fits.Where(x => modKind == null || x.Categories.Contains(modKind));
        shown = modAlphabetical ? shown.OrderBy(x => x.Title, StringComparer.OrdinalIgnoreCase) : shown.OrderByDescending(x => x.Downloads).ThenBy(x => x.Title, StringComparer.OrdinalIgnoreCase);
        var p = new Page { Title = "Find mods: " + g.Title.Replace("Pokemon ", ""), Theme = g.Theme, Jump = 10 };
        Action again = delegate { int sel = p.Sel; pages.Pop(); var fresh = FindModsPage(g, installed, all, note); fresh.Sel = Math.Min(sel, fresh.Items.Count - 1); pages.Push(fresh); };
        p.Items.Add(new Item("Kind: " + (modKind == null ? "All" : KindName(modKind)), "", true, delegate {
            int at = modKind == null ? -1 : kinds.IndexOf(modKind);
            modKind = at + 1 < kinds.Count ? kinds[at + 1] : null; again();
        }) { Icon = '\uE71C', TileHint = shown.Count() + " of " + fits.Count + " mods for this game", Info = "Select to show one kind of mod at a time: " + string.Join(", ", kinds.Select(KindName).ToArray()) + ", or all." });
        p.Items.Add(new Item("Sort: " + (modAlphabetical ? "A to Z" : "Most downloaded"), "", true, delegate { modAlphabetical = !modAlphabetical; again(); }) { Icon = '\uE8CB', Info = "Select to sort " + (modAlphabetical ? "by downloads." : "by name.") });
        if (note != null) p.Items.Add(new Item("Showing the last list", "", false, null) { Icon = '\uE946', Hint = "The index couldn't be reached", Info = note });
        foreach (var entry in shown) {
            var m = entry; string have;
            bool has = installed.TryGetValue(m.Id, out have);
            string state = has ? (have == m.Version ? "Installed" : "Update") : m.Experimental ? "Experimental" : "";
            p.Items.Add(new Item(m.Title + (state != "" ? ": " + state : ""), "", true, delegate { pages.Push(ModEntryPage(g, m, installed)); }) {
                Icon = KindIcon(m.Categories.FirstOrDefault() ?? ""),
                TileHint = "by " + m.Author + (m.Downloads >= 0 ? "  ·  " + m.Downloads.ToString("N0") + " downloads" : ""),
                Info = m.Summary ?? "" });
        }
        if (!shown.Any()) p.Items.Add(new Item("Nothing listed here yet", "", false, null) { Icon = '\uE74C', Hint = modKind != null ? "Try another kind" : "" });
        return p;
    }
    static Dictionary<string, string> InstalledMods(GameEntry g) {
        var installed = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var row in (Gen1Cli("list " + g.Gen1) ?? "").Split('\n')) { var f = row.TrimEnd('\r').Split('\t'); if (f.Length >= 3) installed[f[0]] = f[2]; }
        return installed;
    }
    // One mod: what it does and the trust note, Install first, then its details as a table, then its own page.
    Page ModEntryPage(GameEntry g, ModIndex.Entry m, Dictionary<string, string> installed) {
        string have; bool has = installed.TryGetValue(m.Id, out have);
        string about = !string.IsNullOrEmpty(m.Summary) ? m.Summary : "No description given.";
        var p = new Page { Title = m.Title, Theme = g.Theme, FitBody = true, Body = about + "\n\nIts author's own code, run in the game. Being listed isn't a check of it: install only what you trust." };
        string label = !has ? "Install" : have == m.Version ? "Install again" : "Update to " + m.Version;
        p.Items.Add(new Item(label, "", true, delegate {
            string message = null;
            Busy("Installing " + m.Title, delegate { message = ModIndex.Install(m, delegate(float at) { BeginInvoke((Action)delegate { progress = at; dirty = true; }); }); return null; }, delegate(string e) {
                AfterModInstall(g, e ?? message);
            });
        }) { Icon = '\uE896', TileHint = has ? "You have version " + have : m.Experimental ? "Experimental: it installs switched off" : (m.ZipSize > 0 ? Math.Max(1, m.ZipSize / 1024).ToString("N0") + " KB download" : "") });
        Action<string, string, char> fact = delegate(string name, string value, char icon) { if (!string.IsNullOrEmpty(value)) p.Items.Add(new Item(name + ": " + value, "", true, null) { Icon = icon, Info = name + ": " + value }); };
        // First, what else it takes: those are asked for after the install.
        if (m.Needs.Count > 0) p.Items.Add(new Item("Needs: " + string.Join(", ", m.Needs.ToArray()), "", true, null) { Icon = '\uE7BA', Tint = Warn, Info = "It needs these mods too. After it's installed, the launcher asks where to get each one." });
        if (m.Clashes.Count > 0) {
            var mine = m.Clashes.Where(installed.ContainsKey).ToList();
            p.Items.Add(new Item("Clashes with: " + string.Join(", ", m.Clashes.ToArray()), "", true, null) {
                Icon = '\uE7BA', Tint = Warn, TileHint = mine.Count > 0 ? "You have " + string.Join(", ", mine.ToArray()) : "",
                Info = "Its author says it doesn't work with these. While one of them is on, it doesn't load" + (mine.Count > 0 ? ". You have " + string.Join(", ", mine.ToArray()) + ": after the install you choose which to keep on." : ".") });
        }
        fact("Author", m.Author, '\uE77B');
        fact("Version", m.Version, '\uE946');
        if (m.Downloads >= 0) fact("Downloads", m.Downloads.ToString("N0"), '\uE896');
        fact("Games", m.Games.Count == 0 ? "Any" : string.Join(", ", m.Games.Select(x => Titled(x)).ToArray()), '\uE7FC');
        if (m.Categories.Count > 0) fact("Kind", string.Join(", ", m.Categories.Select(KindName).ToArray()), KindIcon(m.Categories[0]));
        if (m.Permissions.Count > 0) {
            p.Items.Add(new Item("Asks for: " + string.Join(", ", m.Permissions.Select(x => x == "engine_internals" ? "Engine" : x == "filesystem" ? "Files" : x == "network" ? "Internet" : Titled(x.Replace('_', '-'))).ToArray()), "", true, null) { Icon = '\uE72E', Info = "Besides what a plain mod does, it " + string.Join(", and ", m.Permissions.Select(ModIndex.PermissionWords).ToArray()) + "." });
        }
        fact("Licence", m.License, '\uE8A5');
        if (!string.IsNullOrEmpty(m.Repo)) p.Items.Add(new Item("Open its page", "", m.Repo.StartsWith("https://", StringComparison.OrdinalIgnoreCase), delegate { try { Process.Start(m.Repo); } catch { } }) { Icon = '\uE774', TileHint = m.Repo.Replace("https://", ""), Info = m.Repo });
        return p;
    }
    // What installed mods ask for, for one game: each missing one to install from where the player says, or leave.
    // intro: what just happened (an install), shown above the list.
    Page DepsPage(GameEntry g, string intro) {
        var open = ModDeps.Open(g);
        var p = new Page { Title = "Needs other mods", Theme = g.Theme, FitBody = true };
        p.Body = (intro != null ? intro + "\n\n" : "") + (open.Count == 0 ? "Every mod they ask for is installed." :
            "Your mods ask for these. A required one has to be there for the mod to load; an optional one adds to it. Choose where each comes from.");
        foreach (var dep in open) {
            var d = dep;
            string state = d.Status == "version" ? "Wrong version" : d.Required ? "Required" : "Optional";
            p.Items.Add(new Item(d.Id + ": " + state, "", true, delegate { OpenDep(g, d); }) {
                Icon = d.Required ? '\uE7BA' : '\uE74C', Tint = d.Required ? Warn : Color.Empty,
                TileHint = "For " + d.ModName + (d.Range != "" ? ", version " + d.Range : "") + (d.Status == "version" ? " (you have " + d.Have + ")" : ""),
                Info = (d.Required ? d.ModName + " won't load without it." : d.ModName + " can use it, and works without it.") + " Select to install it." });
        }
        p.Items.Add(new Item(open.Count == 0 ? "Done" : "Not now", "", true, delegate { BackToMods(g, null); }) { Icon = '\uE8FB', TileHint = open.Count == 0 ? "" : "Back to the mods. The Mods page keeps a row for these." });
        return p;
    }
    // Back to the game's mods, read again, and a note if there is one.
    void BackToMods(GameEntry g, string note) {
        while (pages.Count > 0 && !(pages.Peek().Title ?? "").StartsWith("Mods: ")) pages.Pop();
        if (pages.Count > 0) pages.Pop();
        pages.Push(ModsPage(g));
        if (note != null) ShowText("Mods", note, null);
    }
    // Clashing pairs that are both on: choose which stays. intro: what just happened (an install), above the list.
    Page ClashesPage(GameEntry g, string intro) {
        var active = ModClashes.Read(g).Where(c => c.Active).ToList();
        var p = new Page { Title = "Mods that clash", Theme = g.Theme, FitBody = true };
        p.Body = (intro != null ? intro + "\n\n" : "") + (active.Count == 0 ? "No mods that are on clash with each other." :
            "These mods say they don't work together. While both are on, the game skips the one that says so. Choose which to keep on.");
        foreach (var clash in active) {
            var c = clash;
            p.Items.Add(new Item(c.ModName + " and " + c.OtherName, "", true, delegate { pages.Push(ClashPairPage(g, c)); }) {
                Icon = '\uE7BA', Tint = Warn, TileHint = c.ModName + " won't load while " + c.OtherName + " is on",
                Info = c.ModName + " names " + c.OtherName + (c.Range != "" ? " (version " + c.Range + ")" : "") + " as a mod it doesn't work with." });
        }
        p.Items.Add(new Item(active.Count == 0 ? "Done" : "Not now", "", true, delegate { BackToMods(g, null); }) { Icon = '\uE8FB', TileHint = active.Count == 0 ? "" : "The Mods page keeps a row for these." });
        return p;
    }
    void SetMod(GameEntry g, string id, bool on) { Gen1Cli("set " + g.Gen1 + " " + id + " " + (on ? "on" : "off")); }
    Page ClashPairPage(GameEntry g, ModClashes.Clash c) {
        var p = new Page { Title = "Which to keep?", Theme = g.Theme, FitBody = true,
            Body = c.ModName + " says it doesn't work with " + c.OtherName + ". While both are on, " + c.ModName + " doesn't load." };
        Action back = delegate { while (pages.Count > 0 && pages.Peek().Title != "Mods that clash") pages.Pop(); if (pages.Count > 0) pages.Pop(); pages.Push(ClashesPage(g, null)); };
        p.Items.Add(new Item("Keep " + c.ModName, "", true, delegate { SetMod(g, c.OtherId, false); back(); }) { Icon = '\uE73E', TileHint = c.OtherName + " is switched off" });
        p.Items.Add(new Item("Keep " + c.OtherName, "", true, delegate { SetMod(g, c.ModId, false); back(); }) { Icon = '\uE73E', TileHint = c.ModName + " is switched off" });
        p.Items.Add(new Item("Leave both on", "", true, delegate { pages.Pop(); }) { Icon = '\uE711', TileHint = c.ModName + " stays skipped" });
        return p;
    }
    // Switching a mod on beside one it clashes with.
    Page ClashChoicePage(GameEntry g, string id, string name, ModClashes.Clash c) {
        bool claims = c.ModId == id;   // this mod made the claim, so it's the one that won't load
        string other = claims ? c.OtherName : c.ModName, otherId = claims ? c.OtherId : c.ModId;
        var p = new Page { Title = "Switch " + name + " on?", Theme = g.Theme, FitBody = true,
            Body = (claims ? name + " says it doesn't work with " + other + ", which is on. While both are on, " + name + " doesn't load."
                           : other + " says it doesn't work with " + name + ". While both are on, " + other + " doesn't load.") };
        Action back = delegate { while (pages.Count > 0 && !(pages.Peek().Title ?? "").StartsWith("Mods: ")) pages.Pop(); if (pages.Count > 0) pages.Pop(); pages.Push(ModsPage(g)); };
        p.Items.Add(new Item("Switch " + other + " off", "", true, delegate { SetMod(g, otherId, false); SetMod(g, id, true); back(); }) { Icon = '\uE73E', TileHint = name + " on, " + other + " off" });
        p.Items.Add(new Item("Switch it on anyway", "", true, delegate { SetMod(g, id, true); back(); }) { Icon = '\uE7BA', Tint = Warn, TileHint = (claims ? name : other) + " won't load" });
        p.Items.Add(new Item("Leave it off", "", true, delegate { pages.Pop(); }) { Icon = '\uE711' });
        return p;
    }
    // The last mod ExtractMod installed, and enable-new's answer: "<version>\ton" or "<version>\tclash\t<id>\t<name>".
    public static string LastInstalled, LastEnable;
    // Where the new mod was left off for this game because it clashes with one that's on: the clash, or null.
    static ModClashes.Clash NewModClash(GameEntry g) {
        if (LastInstalled == null || LastEnable == null) return null;
        bool left = LastEnable.Split('\n').Select(l => l.TrimEnd('\r').Split('\t')).Any(f => f.Length >= 2 && f[0] == g.Gen1 && f[1] == "clash");
        return left ? ModClashes.Read(g).FirstOrDefault(c => c.ModId == LastInstalled || c.OtherId == LastInstalled) : null;
    }
    void AfterModInstall(GameEntry g, string result) {
        var held = NewModClash(g);
        if (held != null) {
            // It went in switched off here: choose, with the install's result above.
            string id = LastInstalled, name = held.ModId == id ? held.ModName : held.OtherName;
            while (pages.Count > 0 && !(pages.Peek().Title ?? "").StartsWith("Mods: ")) pages.Pop();
            if (pages.Count > 0) pages.Pop();
            pages.Push(ModsPage(g));
            var choice = ClashChoicePage(g, id, name, held);
            choice.Body = result + " It's off for now.\n\n" + choice.Body;
            pages.Push(choice);
            LastInstalled = null;
            return;
        }
        bool deps = ModDeps.Open(g).Count > 0, clash = !deps && ModClashes.Read(g).Any(c => c.Active);
        if (deps || clash) {
            while (pages.Count > 0 && !(pages.Peek().Title ?? "").StartsWith("Mods: ")) pages.Pop();
            if (pages.Count > 0) pages.Pop();
            pages.Push(ModsPage(g));
            pages.Push(deps ? DepsPage(g, result) : ClashesPage(g, result));
        } else BackToMods(g, result);
    }
    void OpenDep(GameEntry g, ModDeps.Dep d) {
        List<ModIndex.Entry> all = null; string note;
        Busy("Looking for " + d.Id, delegate { try { all = ModIndex.Load(out note); } catch { all = new List<ModIndex.Entry>(); } return null; }, delegate(string e) {
            pages.Push(DepPage(g, d, all ?? new List<ModIndex.Entry>()));
        });
    }
    Page DepPage(GameEntry g, ModDeps.Dep d, List<ModIndex.Entry> index) {
        var listed = index.Find(x => string.Equals(x.Id, d.Id, StringComparison.OrdinalIgnoreCase));
        var p = new Page { Title = d.Id, Theme = g.Theme, FitBody = true };
        p.Body = d.ModName + (d.Required ? " needs this mod to load." : " can use this mod. It works without it.")
            + (d.Range != "" ? " It asks for version " + d.Range + "." : "")
            + (d.Status == "version" ? " You have " + d.Have + ", which doesn't fit." : "")
            + "\n\nChoose where to get it. Whatever you choose has to hold " + d.Id + ", or nothing is installed.";
        Action<string, Func<Action<float>, string>> install = delegate(string what, Func<Action<float>, string> work) {
            string result = null;
            Busy("Installing " + what, delegate { result = work(delegate(float at) { BeginInvoke((Action)delegate { progress = at; dirty = true; }); }); return null; }, delegate(string e) {
                // Back to the list, read again: this one done, and anything it asks for in turn.
                while (pages.Count > 0 && pages.Peek().Title != "Needs other mods") pages.Pop();
                if (pages.Count > 0) pages.Pop();
                pages.Push(DepsPage(g, e ?? result));
            });
        };
        if (listed != null) p.Items.Add(new Item("From the mod index", "", true, delegate { install(listed.Title, delegate(Action<float> at) { return ModIndex.Install(listed, at); }); }) {
            Icon = '\uE721', TileHint = listed.Title + ", version " + listed.Version + " by " + listed.Author, Info = "The release the Gen1Recomp community mod index lists for it." });
        if (d.Github != "") p.Items.Add(new Item("From its GitHub releases", "", true, delegate {
            install(d.Id, delegate(Action<float> at) {
                long size; string version;
                string url = ModIndex.GithubZip(d.Github, d.Id, out size, out version);
                return url == null ? d.Github + " has no release with a .zip to install." : ModIndex.InstallFrom(url, d.Id, d.Id + " " + version, size, at);
            });
        }) { Icon = '\uE774', TileHint = "github.com/" + d.Github, Info = "The newest release there with a .zip, as " + d.ModName + " names it." });
        p.Items.Add(new Item("Choose a downloaded .zip", "", true, delegate {
            string zip;
            using (var dlg = new OpenFileDialog { Filter = "Mod archive (*.zip)|*.zip", Title = "Choose the .zip for " + d.Id }) { if (dlg.ShowDialog(this) != DialogResult.OK) return; zip = dlg.FileName; }
            install(d.Id, delegate(Action<float> at) { try { return ExtractMod(zip, d.Id); } catch (Exception x) { return "Could not install it: " + x.Message; } });
        }) { Icon = '\uE8B7', TileHint = "One you already downloaded", Info = "A mod archive on your PC. It has to be " + d.Id + "." });
        p.Items.Add(new Item("Enter a download link", "", true, delegate {
            string url = AskText("Download link", "Link to the .zip of " + d.Id + " (https://):", "");
            if (string.IsNullOrWhiteSpace(url)) return;
            url = url.Trim();
            install(d.Id, delegate(Action<float> at) { return ModIndex.InstallFrom(url, d.Id, d.Id, 0, at); });
        }) { Icon = '\uE71B', TileHint = "A link to its .zip", Info = "Where its author publishes it: an https:// link straight to the .zip." });
        if (!d.Required) p.Items.Add(new Item("Don't install it", "", true, delegate { ModDeps.Decline(d); pages.Pop(); pages.Pop(); pages.Push(DepsPage(g, null)); }) {
            Icon = '\uE711', TileHint = "Not asked again for " + d.ModName, Info = d.ModName + " works without it. You can still install it later from a .zip." });
        return p;
    }
    void InstallMod(GameEntry g) {
        using (var d = new OpenFileDialog { Filter = "Mod archive (*.zip)|*.zip", Title = "Choose a mod" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            string message;
            try { message = ExtractMod(d.FileName); } catch (Exception e) { message = "Could not install it: " + e.Message; }
            AfterModInstall(g, message);
        }
    }
    public static string ExtractMod(string zip) { return ExtractMod(zip, null); }
    // The mod's id from its manifest.json, or null.
    static string ManifestId(string folder) {
        try {
            var d = new System.Web.Script.Serialization.JavaScriptSerializer().Deserialize<Dictionary<string, object>>(File.ReadAllText(Path.Combine(folder, "manifest.json")).TrimStart('\uFEFF'));
            object id; return d != null && d.TryGetValue("id", out id) ? id as string : null;
        } catch { return null; }
    }
    // expected: the mod this zip has to be (a dependency asked for by id), or null for any.
    public static string ExtractMod(string zip, string expected) {
        string paths = Gen1Cli("paths");
        if (paths == null) return "Gen1Recomp was not found.";
        string mods = paths.Split('\n').Select(l => l.TrimEnd('\r').Split('\t')).Where(f => f[0] == "mods").Select(f => f[1]).First().Replace('/', '\\');
        // Unpacked beside the mods folder, so the last step is a move on one drive. Not inside it, where the game would see it.
        Directory.CreateDirectory(mods);
        string parent = Path.GetDirectoryName(mods.TrimEnd('\\'));
        string temp = Path.Combine(parent, "nativemons-mod-unpack"), old = Path.Combine(parent, "nativemons-mod-old");
        try {
            if (Directory.Exists(temp)) Directory.Delete(temp, true);
            System.IO.Compression.ZipFile.ExtractToDirectory(zip, temp);
            // The manifest is at the top of the archive, or inside one folder.
            string root = File.Exists(Path.Combine(temp, "manifest.json")) ? temp : Directory.GetDirectories(temp).FirstOrDefault(x => File.Exists(Path.Combine(x, "manifest.json")));
            if (root == null) return "This archive has no manifest.json, so it is not a Gen1Recomp mod.";
            string holds = ManifestId(root);
            if (expected != null && !string.Equals(holds, expected, StringComparison.OrdinalIgnoreCase))
                return "That archive holds " + (holds ?? "a mod with no id") + ", not " + expected + ", so nothing was installed.";
            string target = Path.Combine(mods, root == temp ? Path.GetFileNameWithoutExtension(zip) : Path.GetFileName(root));
            // An older copy steps aside first and comes back if the new one can't go in.
            if (Directory.Exists(old)) Directory.Delete(old, true);
            bool replacing = Directory.Exists(target);
            if (replacing) Directory.Move(target, old);
            try { MoveTree(root, target); }
            catch { if (Directory.Exists(target)) Directory.Delete(target, true); if (replacing) Directory.Move(old, target); throw; }
            if (replacing) Directory.Delete(old, true);
            // A newly installed mod is switched on in each game, unless it clashes there with one that's on.
            LastInstalled = holds; LastEnable = holds != null ? (Gen1Cli("enable-new " + holds) ?? "") : "";
            return (replacing ? "Updated " : "Installed ") + Path.GetFileName(target) + ".";
        }
        finally { try { if (Directory.Exists(temp)) Directory.Delete(temp, true); } catch { } }
    }
    // A move, or a copy then delete when the folders are on different drives.
    static void MoveTree(string from, string to) {
        if (string.Equals(Path.GetPathRoot(from), Path.GetPathRoot(to), StringComparison.OrdinalIgnoreCase)) { Directory.Move(from, to); return; }
        Directory.CreateDirectory(to);
        foreach (var f in Directory.GetFiles(from)) File.Copy(f, Path.Combine(to, Path.GetFileName(f)));
        foreach (var d in Directory.GetDirectories(from)) MoveTree(d, Path.Combine(to, Path.GetFileName(d)));
        Directory.Delete(from, true);
    }
    Page FoldersPage() {
        var p = new Page { Title = "ROM folders" };
        foreach (var folder in Config.RomFolders()) {
            string f = folder;
            p.Items.Add(new Item(Path.GetFileName(f.TrimEnd('\\')), "", true, delegate { pages.Push(RemoveFolderPage(f)); }) { Icon = '', Info = f + "\n\nSelect to stop searching this folder. Your ROMs stay where they are." });
        }
        foreach (var file in Config.RomFiles()) {
            string f = file;
            p.Items.Add(new Item(Path.GetFileName(f), "", true, delegate { pages.Push(RemoveRomFilePage(f)); }) { Icon = '', Info = f + (File.Exists(f) ? "" : "\n\nNot found there any more.") + "\n\nA ROM added on its own. Select to stop using it. The file stays where it is." });
        }
        if (Directory.Exists(Config.Unzipped)) foreach (var dir in Directory.GetDirectories(Config.Unzipped)) {
            string d = dir;
            p.Items.Add(new Item(Path.GetFileName(d) + ".zip", "", true, delegate { pages.Push(RemoveUnzippedPage(d)); }) { Icon = '', Info = "ROMs taken out of this zip, kept by nativeMons in its own folder.\n\nSelect to delete these copies. The zip itself was never changed." });
        }
        p.Items.Add(new Item("Add a folder", "", true, delegate { AddFolder(); }) { Icon = '', Info = "nativeMons finds games by reading each ROM, so file names don't matter. No games are included: you need your own ROMs.\n\nYou can also drop a folder, a ROM or a .zip on this window." });
        p.Items.Add(new Item("Add a ROM file", "", true, delegate { AddRomFile(); }) { Icon = '', Info = "One .gb, .gbc, .gba or .nds ROM, used where it is. A .zip works too: its ROMs are copied out, and the zip isn't changed." });
        if (Config.RomFolders().Count > 0 || Config.RomFiles().Count > 0 || Library.Games.Exists(g => g.Playable)) p.Items.Add(new Item(FinishFoldersLabel, "", true, FinishFolders) { Icon = '', Info = "Go back to the main screen. Your folders are saved." });
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
    Page RemoveRomFilePage(string file) {
        var p = new Page { Title = "Stop using this ROM?", Body = file };
        p.Items.Add(new Item("Keep it", "", true, delegate { pages.Pop(); }) { Icon = '' });
        p.Items.Add(new Item("Stop using it", "Nothing is deleted.", true, delegate {
            var l = Config.RomFiles(); l.RemoveAll(x => string.Equals(x, file, StringComparison.OrdinalIgnoreCase)); Config.SaveRomFiles(l);
            pages.Pop(); pages.Pop();
            Rescan(delegate { pages.Push(FoldersPage()); });
        }) { Icon = '' });
        return p;
    }
    Page RemoveUnzippedPage(string dir) {
        var p = new Page { Title = "Delete these copies?", Body = "The ROMs taken out of " + Path.GetFileName(dir) + ".zip. The zip itself stays as it is, and your saves are kept." };
        p.Items.Add(new Item("Keep them", "", true, delegate { pages.Pop(); }) { Icon = '' });
        p.Items.Add(new Item("Delete the copies", "", RunningGames() == "", delegate {
            pages.Pop(); pages.Pop();
            Busy("Deleting", delegate { Directory.Delete(dir, true); return null; }, delegate(string e) {
                Rescan(delegate { pages.Push(FoldersPage()); if (e != null) ShowText("Not deleted", e, null); });
            });
        }) { Icon = '' });
        return p;
    }
    void AddRomFile() {
        using (var d = new OpenFileDialog { Filter = "ROM or zip (*.gb, *.gbc, *.gba, *.nds, *.zip)|*.gb;*.gbc;*.gba;*.nds;*.zip|All files (*.*)|*.*", Title = "Choose a ROM", Multiselect = true }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            TakeDrop(d.FileNames);
        }
    }
    // ---------- dropped files ----------
    bool CanTakeDrop() { return job == null && !starting && pages.Count > 0 && pages.Peek().Boxes == null; }
    // Folders become ROM folders, ROMs are used in place, a zip's ROMs are copied out, saves are imported. Nothing dropped changes.
    void TakeDrop(string[] paths) {
        if (!CanTakeDrop()) return;
        Native.Front(Handle);
        var folders = paths.Where(Directory.Exists).ToList();
        var roms = paths.Where(x => File.Exists(x) && Library.IsRom(x)).ToList();
        var zips = paths.Where(x => File.Exists(x) && Path.GetExtension(x).ToLowerInvariant() == ".zip").ToList();
        var saves = paths.Where(x => File.Exists(x) && Path.GetExtension(x).ToLowerInvariant() == ".sav").ToList();
        var other = paths.Where(x => !folders.Contains(x) && !roms.Contains(x) && !zips.Contains(x) && !saves.Contains(x)).ToList();
        if (folders.Count + roms.Count + zips.Count == 0) {
            if (saves.Count > 0) { ImportSaveFiles(saves); return; }
            bool packed = other.Any(x => new[] { ".7z", ".rar" }.Contains(Path.GetExtension(x).ToLowerInvariant()));
            ShowText("Can't use that", string.Join(", ", other.Select(Path.GetFileName).ToArray()) + "\n\nDrop a folder of ROMs, a .gb, .gbc, .gba or .nds ROM, a .zip of ROMs, or a .sav save." + (packed ? "\n\n.7z and .rar can't be opened here: extract it first, then drop the ROM." : ""), null);
            return;
        }
        var notes = new List<string>();
        var added = new List<string>();   // every ROM this drop brought in, to say what each one turned out to be
        Busy("Adding your games", delegate { AddPaths(folders, roms, zips, added, notes); return null; }, delegate(string e) {
            PickCurrent();
            ReplaceMain();
            if (e != null) { ShowText("Something went wrong", e, null); return; }
            var lines = new List<string>();
            foreach (var f in folders) lines.Add("Folder added: " + f);
            foreach (var f in added) lines.Add(Path.GetFileName(f) + ": " + WhatRomIs(f));
            lines.AddRange(notes);
            if (other.Count > 0) lines.Add("Not used: " + string.Join(", ", other.Select(Path.GetFileName).ToArray()));
            int have = Library.Games.Count(g => g.Playable);
            var p = new Page { Title = "Added", Body = string.Join("\n\n", lines.ToArray()) + "\n\nYou have " + have + " of " + Library.Games.Count + " games ready." };
            p.Items.Add(new Item("Done", "Back to where you were.", true, delegate { pages.Pop(); }) { Icon = '' });
            if (saves.Count > 0) p.Items.Add(new Item("Import the saves too", saves.Count + (saves.Count == 1 ? " save" : " saves") + " dropped with them.", File.Exists(Config.MonBridge), delegate { pages.Pop(); ImportSaveFiles(saves); }) { Icon = '' });
            pages.Push(p);
            AskAboutEncrypted();
        });
    }
    // The work of a drop, apart from the window, so --add-roms can run it too.
    public static void AddPaths(List<string> folders, List<string> roms, List<string> zips, List<string> added, List<string> notes) {
        if (folders.Count > 0) {
            var l = Config.RomFolders();
            foreach (var f in folders) if (!l.Contains(f, StringComparer.OrdinalIgnoreCase)) l.Add(f);
            Config.SaveRomFolders(l);
        }
        if (roms.Count > 0) {
            var l = Config.RomFiles();
            foreach (var f in roms) {
                string full = Path.GetFullPath(f);
                // Already inside a ROM folder: it's found there, so it needn't be listed on its own.
                bool covered = Config.RomFolders().Any(dir => full.StartsWith(Path.GetFullPath(dir).TrimEnd('\\') + "\\", StringComparison.OrdinalIgnoreCase));
                if (!covered && !l.Contains(full, StringComparer.OrdinalIgnoreCase)) l.Add(full);
                added.Add(full);
            }
            Config.SaveRomFiles(l);
        }
        foreach (var z in zips) {
            string problem = Unzip(z, added);
            if (problem != null) notes.Add(Path.GetFileName(z) + ": " + problem);
        }
        Library.Scan(Gen1Cli);
        // A ROM no game here uses isn't kept in the list: it would only be read again at every start.
        if (roms.Count > 0) {
            var used = new HashSet<string>(Library.Games.SelectMany(g => g.Roms).Select(r => r.File), StringComparer.OrdinalIgnoreCase);
            var l = Config.RomFiles();
            if (l.RemoveAll(f => added.Contains(f, StringComparer.OrdinalIgnoreCase) && !used.Contains(f)) > 0) Config.SaveRomFiles(l);
        }
    }
    // Which game a ROM turned out to be, or why it can't be used, with its SHA-1 when nothing knows it.
    public static string WhatRomIs(string file) {
        foreach (var g in Library.Games) {
            var rom = g.Roms.Find(r => string.Equals(r.File, file, StringComparison.OrdinalIgnoreCase));
            if (rom == null) continue;
            if (g.Playable) return g.Title + ", ready to play." + (g.Roms.Count > 1 ? " With more than one copy, its ROM version page picks which." : "");
            return g.Title + ". " + Library.WhyNotPlayable(g);
        }
        string sha = "";
        try { using (var f = File.OpenRead(file)) using (var h = SHA1.Create()) sha = BitConverter.ToString(h.ComputeHash(f)).Replace("-", "").ToLowerInvariant(); } catch { }
        // The same dump found somewhere else first: the list keeps one file per dump.
        foreach (var g in Library.Games) {
            var same = g.Roms.Find(r => r.Sha1 == sha);
            if (same != null) return g.Title + ". You already have this exact ROM: " + same.File;
        }
        return "not one of the games nativeMons runs" + (sha != "" ? " (SHA-1 " + sha + ")" : "") + ", so it wasn't added. The README lists the versions each engine accepts.";
    }
    // Copies a zip's ROMs out into state\roms\<zip name>. Other files in it are left alone.
    static string Unzip(string zip, List<string> added) {
        try {
            string to = Path.Combine(Config.Unzipped, Path.GetFileNameWithoutExtension(zip));
            int n = 0;
            using (var archive = System.IO.Compression.ZipFile.OpenRead(zip)) {
                foreach (var entry in archive.Entries) {
                    if (entry.Name == "" || !Library.IsRom(entry.Name)) continue;
                    Directory.CreateDirectory(to);
                    string target = Path.Combine(to, entry.Name);
                    n++; added.Add(target);
                    if (File.Exists(target) && new FileInfo(target).Length == entry.Length) continue;
                    System.IO.Compression.ZipFileExtensions.ExtractToFile(entry, target + ".part", true);
                    if (File.Exists(target)) File.Delete(target);
                    File.Move(target + ".part", target);
                }
            }
            return n == 0 ? "no .gb, .gbc, .gba or .nds ROM inside." : null;
        } catch (Exception x) { return "couldn't be opened (" + x.Message + ")."; }
    }
    // One dropped save goes straight to importing, as Choose a file does; several get a list to pick from.
    void ImportSaveFiles(List<string> files) {
        if (!File.Exists(Config.MonBridge)) { ShowText("Can't import saves", "This copy of nativeMons is missing its save tools.", null); return; }
        var found = new List<FoundSave>();
        Busy("Reading the saves", delegate {
            foreach (var f in files) { var one = Identify(f) ?? IdentifySlot(f); if (one != null) { one.Info = ReadSaveInfo(f); found.Add(one); } }
            return null;
        }, delegate(string e) {
            if (e != null) { ShowText("Can't read that save", e, null); return; }
            if (found.Count == 0) { ShowText("Can't read that save", "PKHeX doesn't recognise " + (files.Count == 1 ? "this file" : "these files") + " as a save.", null); return; }
            if (found.Count == 1 && files.Count == 1) {
                var one = found[0];
                if (one.Game == null || !one.Game.Playable) { ShowText("Can't import that save", ((one.Note ?? "") + " Its game isn't in your ROM folders.").Trim(), null); return; }
                ImportSave(one); return;
            }
            var p = new Page { Title = "Import saves" };
            foreach (var one in found) p.Items.Add(SaveRow(one));
            pages.Push(p);
        });
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
            + "\n\nPokemon Infinite Fusion and Infinite Fusion 2 by chardub (Frogman) and their contributors (github.com/infinitefusion/infinitefusion-e18, github.com/infinitefusion/infinitefusion-hoenn-public), made with Pokemon Essentials on RPG Maker XP. Its custom fusion sprites are by the Infinite Fusion artist community. Installed from your own copy or its GitHub; not included, and not affiliated with nativeMons."
            + "\n\nMoving Pokemon between generations uses the conversion code of OpenHome by Andrew Benington (github.com/andrewbenington/OpenHome, GPL-3.0), running on Node.js. Open in OpenHome uses your own copy."
            + "\n\nPokemon pictures served by pogo.hails.app (hailsDotGO), from the PokeAPI sprites collection. Pokedex entries from PokeAPI (pokeapi.co)."
            + "\n\nFind mods reads the Gen1Recomp community mod index (github.com/bryanthaboi/gen1recomp-mod-index). Its listings are by the mods' authors, and the mods are theirs."
            + "\n\nThe GBA player is ours, in our Fulldex fork, on mGBA by endrift and contributors (MPL-2.0), after Pokemon GBA Widescreen by mamesj. Widescreen for romhacks and the versions our patches don't cover comes from PokeDaisy's EXPAND (GPL-3.0). Based on PokeDaisy by Lidor Itzhari - https://github.com/lidor30/pokedaisy"
            + "\n\nRomhacks' title screens are captured with mGBA by endrift and contributors (github.com/mgba-emu/mgba, MPL-2.0), as its libretro core."
            + "\n\nMoving Pokemon uses PKHeX.Core by Kaphotics and contributors (github.com/kwsch/PKHeX, GPL-3.0)."
            + "\n\nhails.nds-Patcher by Hails. Launcher design from the hails-additions branch of the Skate 3 Rust Engine (SK8-ENGINE), github.com/Hailey-Ross/rusty-trucks."
            + "\n\n" + Gen1Credit
            + "\n\nNot affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokemon Company. Pokemon and all related names are the property of their owners. No games are included: you need your own ROMs." };
    }
    // Keys each engine answers to while a game runs, from each project's README, in our words.
    static readonly string[][] ControlSheets = {
        new[] { "Fulldex", "DS and GBA games in widescreen.",
            "Move: arrow keys\nA: X    B: Z\nX: S    Y: A\nL: Q    R: W\nStart: Enter\nSelect: Backspace\nTouch screen: the mouse\n\nPause menu: Esc\nFast forward: hold Left Shift\nSave state: F5\nLoad state: F8\nWindow or fullscreen: F11\n\nA controller plays as a DS: hold its right trigger to fast forward, Start opens the pause menu.\n\nChange any key in the pause menu, under Controls, then Keys and buttons." },
        new[] { "Gen1Recomp", "Game Boy, Game Boy Color and GBA games.",
            "Move: arrow keys or W A S D\nA: Z, Enter or Space\nB: X or Backspace\nStart: Esc\nSelect: Tab or Shift\n\nSave: F1\nLoad: F2\nGame speed up, down: 1, 0\nColours: 2\nTilt: 3\nZoom: 4, or - and =, or the mouse wheel\nMods: F10\n\nOn a controller, R2 and L2 change the game speed.\n\nChange any key in the game's Options, under Controls." },
        new[] { "nativeplat", "Diamond, Pearl and Platinum, native.",
            "Move: arrow keys or W A S D\nA: Z, Enter or Space\nB: X or Backspace\nX: C    Y: V\nL: Q    R: E\nStart: Esc\nSelect: Tab or Shift\nTouch screen: the mouse on the lower screen\n\nOptions: F10\nFast forward: hold F, or G to turn it on and off\nSpeed 1x to uncapped: 1\nRewind: hold R\nSave in the game: F1\nBack to the last save: F2 twice\nSnapshot: F5 to take, F6 for the next of 4, F7 to go back\nCamera: - and =, tilt 3 and 4, reset 0\nTouch controls: F9\nFullscreen: F11\nScreenshot: F12\n\nChange any key in Options, under Controls." },
        new[] { "Platinum PC port", "Platinum, native.",
            "Settings and debug tools: the ` key (grave, under Esc).\n\nIts other keys are listed in its own settings." },
        new[] { "Infinite Fusion", "Its own program.",
            "Speed up: the A key\nScreenshot: F8\nKeys and controller: F1\n\nWhether A holds or toggles the speed is set in the game's Options." },
    };
    // Where each engine's screenshot key writes. Fulldex and Gen1Recomp have none.
    static List<KeyValuePair<string, string>> ScreenshotFolders() {
        var list = new List<KeyValuePair<string, string>>();
        if (File.Exists(Config.Nativeplat)) list.Add(new KeyValuePair<string, string>("nativeplat", Path.Combine(NativeplatDir, "userdata", "screenshots")));
        foreach (var g in Library.Games.Where(x => PcGames.IsFusion(x) && PcGames.Installed(x)))
            list.Add(new KeyValuePair<string, string>(g.Title, PcGames.SaveDir(g)));
        return list;
    }
    static int CountPictures(string dir) {
        try { return Directory.Exists(dir) ? Directory.GetFiles(dir, "*.png").Length : 0; } catch { return 0; }
    }
    Page ScreenshotsPage() {
        var p = new Page { Title = "Screenshots" };
        foreach (var kv in ScreenshotFolders()) {
            string dir = kv.Value; int n = CountPictures(dir);
            string key = kv.Key == "nativeplat" ? "F12" : "F8";
            string count = n == 0 ? "None yet: press " + key + " in the game." : n + (n == 1 ? " picture" : " pictures") + ", taken with " + key + ".";
            p.Items.Add(new Item(kv.Key, "", true, delegate {
                Directory.CreateDirectory(dir);
                try { Process.Start("explorer.exe", "\"" + dir + "\""); } catch { }
            }) { Icon = '', Info = count + " Select to open the folder: " + dir });
        }
        p.Items.Add(new Item("Other games", "", true, delegate {
            ShowText("Screenshots in other games", "Fulldex and Gen1Recomp have no screenshot key of their own.\n\nWindows can take one of any game, these and the Platinum PC port too: Windows + Shift + S picks an area, and Windows + Alt + Print Screen saves the game's window to Videos\\Captures.", null);
        }) { Icon = '', Info = "Taking screenshots in Fulldex, Gen1Recomp and the Platinum PC port." });
        return p;
    }
    Page ControlsPage() {
        var p = new Page { Title = "Controls" };
        foreach (var sheet in ControlSheets) {
            var s = sheet;
            p.Items.Add(new Item(s[0], "", true, delegate { ShowText(s[0] + " controls", s[2], null); }) { Icon = '', Info = s[1] + " The keys it answers to while you play." });
        }
        return p;
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
        string summary = null, dexSummary = null;
        Busy("Downloading the Pokedex", delegate {
            Sprites.GetAll(delegate(int done, int total, int fresh, int failed) {
                progress = done / (float)total;
                if (done % 25 == 0 || done == total) BeginInvoke((Action)delegate { log.Add(done + " of " + total + " checked"); });
                dirty = true;
                if (done == total) summary = fresh + " downloaded, " + (total - fresh - failed) + " already here." + (failed > 0 ? "\n\n" + failed + " couldn't be fetched. Run this again when you're online: it only fetches what's missing." : "\n\nEvery picture is ready for offline play.");
            });
            log.Add("Pictures done. Now the Pokedex entries.");
            DexInfos.GetAll(Sprites.MaxDex, delegate(int done, int total, int fresh, int failed) {
                progress = done / (float)total;
                if (done % 25 == 0 || done == total) BeginInvoke((Action)delegate { log.Add(done + " of " + total + " entries"); });
                dirty = true;
                if (done == total) dexSummary = "Pokedex entries: " + fresh + " downloaded, " + (total - fresh - failed) + " already here." + (failed > 0 ? " " + failed + " couldn't be fetched. Run this again when you're online." : "");
            });
            return null;
        }, delegate(string e) { ShowText("Download Pokedex", e ?? ("Pictures: " + summary + "\n\n" + dexSummary), null); });
    }

    // ---------- moving Pokemon (tools\mon-bridge, built on PKHeX.Core) ----------
    internal static string RunCapture(ProcessStartInfo psi, int seconds, out int code) {
        psi.UseShellExecute = false; psi.CreateNoWindow = true; psi.RedirectStandardOutput = true; psi.RedirectStandardError = true;
        psi.StandardOutputEncoding = Encoding.UTF8;
        using (var p = Process.Start(psi)) { if (GameArt.OnCaptureThread) GameArt.Watch(p);
            if (GameArt.OnCaptureThread) try { p.PriorityClass = GameArt.Eager ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Idle; } catch { }
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
    // Whether a game has a save to show, without running anything.
    static bool HasBoxSave(GameEntry g) {
        try {
            if (g.Engine() != "gen1") { var f = new FileInfo(Path.Combine(Config.FulldexDir, "saves", g.Save ?? "")); return f.Exists && f.Length > 0; }
            string dir = Path.Combine(Gen1Root, "saves", g.Gen1 ?? "");
            return Directory.Exists(dir) && Directory.GetFiles(dir, "*.lua").Any(x => !x.EndsWith(".bak.lua", StringComparison.OrdinalIgnoreCase));
        } catch { return true; }
    }
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
    // ---------- Move Pokemon: the box screen ----------
    static BoxSide LoadSide(GameEntry g) {
        var s = new BoxSide { Game = g };
        string why;
        s.File = SaveOf(g, out why);
        if (s.File == null) {
            s.NoSave = !HasBoxSave(g) || why == null || why.StartsWith("No save yet") || why.IndexOf("no save", StringComparison.OrdinalIgnoreCase) >= 0;
            s.Problem = s.NoSave ? "No save yet\n\nPlay " + g.Title + " and save once in the game. Its boxes show here after that." : why;
            return s;
        }
        int code; string layout = Bridge("layout \"" + s.File + "\"", out code);
        if (code != 0) { s.Problem = layout.Trim(); return s; }
        foreach (var row in layout.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f[0] == "boxes" && f.Length >= 3) { s.Boxes = Math.Max(1, int.Parse(f[1])); s.Slots = Math.Max(1, int.Parse(f[2])); }
            else if (f[0] == "box" && f.Length >= 3) s.Names.Add(f[2]);
        }
        string list = Bridge("list \"" + s.File + "\"", out code);
        if (code != 0) { s.Problem = list.Trim(); return s; }
        foreach (var row in list.Split('\n')) {
            var f = row.TrimEnd('\r').Split('\t');
            if (f.Length < 8) continue;
            int box, slot, level, dex;
            if (!int.TryParse(f[0], out box) || !int.TryParse(f[1], out slot)) continue;
            int.TryParse(f[4], out level); int.TryParse(f[6], out dex);
            s.Mons[box * s.Slots + slot] = new BoxMon { Species = f[2], Nick = f[3], Level = level, Trainer = f[5], Dex = dex, Shiny = f[7] == "shiny" };
        }
        return s;
    }
    // Both boxes read again; the same game in both is one save, read once.
    void LoadBoxes(BoxView v, GameEntry top, GameEntry bottom, Action then) {
        BoxSide a = null, b = null;
        Busy("Opening your boxes", delegate {
            a = LoadSide(top);
            b = bottom.Id == top.Id ? a : LoadSide(bottom);
            return null;
        }, delegate(string e) {
            if (e != null) { ShowText("Can't open the boxes", e, null); return; }
            v.Side[0] = a; v.Side[1] = b;
            for (int i = 0; i < 2; i++) v.Box[i] = Math.Min(v.Box[i], v.Side[i].Boxes - 1);
            if (then != null) then();
            Invalidate();
        });
    }
    void OpenBoxView() {
        var games = Library.Games.Where(x => x.SaveTools).ToList();
        if (games.Count == 0) return;
        var saved = games.Where(HasBoxSave).ToList();
        if (saved.Count == 0) { ShowText("Move Pokemon", "None of your games has a save yet. Play one and save once in the game, then its boxes show here.", null); return; }
        var top = current != null && saved.Contains(current) ? current : saved[0];
        var bottom = saved.FirstOrDefault(x => x.Id != top.Id) ?? top;
        var v = new BoxView();
        LoadBoxes(v, top, bottom, delegate { pages.Push(new Page { Title = "Move Pokemon", Boxes = v }); });
    }
    // Another game in one of the two places.
    void ChooseBoxGame(BoxView v, int pane) {
        var p = new Page { Title = (pane == 0 ? "Top" : "Bottom") + ": which game?" };
        foreach (var game in Library.Games.Where(x => x.SaveTools)) {
            var g = game;
            bool has = HasBoxSave(g);
            p.Items.Add(new Item(g.Title, g.Id == v.Side[pane].Game.Id ? "Showing now." : has ? "" : "No save yet. Play and save once first.", has || g.Id == v.Side[pane].Game.Id, delegate {
                pages.Pop();
                var other = v.Side[1 - pane].Game;
                v.Box[pane] = 0; v.HeldPane = -1; v.Cursor = -1; v.Pane = pane;
                LoadBoxes(v, pane == 0 ? g : other, pane == 0 ? other : g, null);
            }) { Icon = '', Theme = g.Theme });
        }
        pages.Push(p);
    }
    // Where the box screen's move goes, and back to the boxes after. The list pages leave these empty.
    string moveAt; Action moveAfter;
    void BoxDrop(BoxView v, int toPane, int toSlot) {
        int fromPane = v.HeldPane, fromBox = v.HeldBox, fromSlot = v.HeldSlot, toBox = v.Box[toPane];
        v.HeldPane = -1; v.Dragging = false; v.Note = null;
        var from = v.Side[fromPane]; var to = v.Side[toPane];
        var mon = from.At(fromBox, fromSlot);
        if (mon == null) return;
        // Moves go between the games: back on its own side, it goes back where it was.
        if (fromPane == toPane) return;
        if (from.Game.Id == to.Game.Id) {
            if (fromBox == toBox && fromSlot == toSlot) return;
            if (AfterCapture(delegate { BoxDrop(v, toPane, toSlot); })) return;
            string running = RunningGames();
            if (running != "") { v.Note = "Close " + running + " first: saves can't change while it plays."; return; }
            string problem = null; var g = from.Game; string file = from.File;
            Busy("Moving " + mon.Species, delegate {
                int code; string o = Bridge("relocate \"" + file + "\" " + fromBox + " " + fromSlot + " " + toBox + " " + toSlot + " \"" + Path.Combine(Config.State, "save-backups") + "\"", out code);
                if (code != 0) { var f = o.Trim().Split('\t'); problem = f.Length > 1 ? f[1] : o.Trim(); return null; }
                problem = ImportBack(g, file);
                return null;
            }, delegate(string e) {
                if (e != null || problem != null) v.Note = e ?? problem;
                LoadBoxes(v, v.Side[0].Game, v.Side[1].Game, null);
            });
            return;
        }
        if (to.At(toBox, toSlot) != null) { v.Note = "That slot is taken. Drop it on an empty one."; return; }
        moveAt = "at:" + toBox + ":" + toSlot;
        moveAfter = delegate { LoadBoxes(v, v.Side[0].Game, v.Side[1].Game, null); };
        MoveNow(from.Game, from.File, fromBox.ToString(), fromSlot.ToString(), mon.Species, to.Game);
    }
    // The cursor to the other game, as HOME's button does: the same place, or holding, the nearest empty slot.
    static void BoxOther(BoxView v) {
        int at = v.Cursor; v.Pane = 1 - v.Pane; var s = v.Side[v.Pane];
        if (s == null || s.Problem != null) { v.Cursor = -1; return; }
        if (at < 0) return;
        at = Math.Min(at, s.Slots - 1);
        if (v.Holding && s.At(v.Box[v.Pane], at) != null)
            for (int d = 1; d < s.Slots; d++) {
                if (at + d < s.Slots && s.At(v.Box[v.Pane], at + d) == null) { at += d; break; }
                if (at - d >= 0 && s.At(v.Box[v.Pane], at - d) == null) { at -= d; break; }
            }
        v.Cursor = at; v.Col = at % 6;
    }
    void BoxAct(Page p, string a) {
        var v = p.Boxes; var side = v.Side[v.Pane];
        const int cols = 6;
        int count = side.Slots, rows = (count + cols - 1) / cols;
        if (a == "switch") { BoxOther(v); v.Note = null; return; }
        if (a == "boxprev" || a == "boxnext") {
            if (side.Problem == null) { v.Box[v.Pane] = (v.Box[v.Pane] + (a == "boxnext" ? 1 : -1) + side.Boxes) % side.Boxes; v.Note = null; }
            if (v.Holding && v.Cursor >= 0 && v.Pane != v.HeldPane) { v.Pane = 1 - v.Pane; BoxOther(v); }
            return;
        }
        if (a == "left" || a == "right") {
            int d = a == "right" ? 1 : -1;
            if (v.Cursor < 0) { v.Box[v.Pane] = (v.Box[v.Pane] + d + side.Boxes) % side.Boxes; v.Note = null; }
            else {
                int col = v.Cursor % cols + d;
                if (col >= 0 && col < cols && v.Cursor + d < count) { v.Cursor += d; v.Col = col; }
                else if (v.Wide && ((d > 0 && v.Pane == 0) || (d < 0 && v.Pane == 1))) {
                    // Side by side, the edge of one box leads into the other game's, on the same row.
                    int row = v.Cursor / cols; v.Pane = 1 - v.Pane; var other = v.Side[v.Pane];
                    int orows = (other.Slots + cols - 1) / cols;
                    v.Col = d > 0 ? 0 : cols - 1;
                    v.Cursor = Math.Min(other.Slots - 1, Math.Min(row, orows - 1) * cols + v.Col);
                }
            }
        } else if (v.Wide && (a == "up" || a == "down")) {
            // Side by side, up and down stay in the one game: the header at the top, its rows below.
            if (a == "up") { if (v.Cursor >= cols) v.Cursor -= cols; else v.Cursor = -1; }
            else { if (v.Cursor < 0) v.Cursor = Math.Min(count - 1, v.Col); else if (v.Cursor / cols < rows - 1) v.Cursor = Math.Min(count - 1, v.Cursor + cols); }
        } else if (a == "up") {
            if (v.Cursor < 0) { if (v.Pane == 1) { v.Pane = 0; var s0 = v.Side[0]; int r0 = (s0.Slots + cols - 1) / cols; v.Cursor = Math.Min(s0.Slots - 1, (r0 - 1) * cols + v.Col); } }
            else if (v.Cursor < cols) v.Cursor = -1;
            else v.Cursor -= cols;
        } else if (a == "down") {
            if (v.Cursor < 0) v.Cursor = Math.Min(count - 1, v.Col);
            else if (v.Cursor / cols < rows - 1) v.Cursor = Math.Min(count - 1, v.Cursor + cols);
            else if (v.Pane == 0) { v.Pane = 1; v.Cursor = -1; }
        } else if (a == "ok") {
            if (v.Cursor < 0) { if (!v.Holding) ChooseBoxGame(v, v.Pane); return; }
            if (!v.Holding) {
                if (side.At(v.Box[v.Pane], v.Cursor) != null) { v.HeldPane = v.Pane; v.HeldBox = v.Box[v.Pane]; v.HeldSlot = v.Cursor; v.Note = null; BoxOther(v); }
            } else BoxDrop(v, v.Pane, v.Cursor);
        }
    }
    // The box screen's places under the pointer, from the last paint: s<pane>:<slot>, h<pane> (the game), l/r<pane> (boxes).
    readonly List<KeyValuePair<RectangleF, string>> boxHits = new List<KeyValuePair<RectangleF, string>>();
    string BoxHitAt(Point pt) {
        var inside = new PointF(pt.X - screenX, pt.Y - screenY);
        foreach (var h in boxHits) if (h.Key.Contains(inside)) return h.Value;
        return null;
    }
    BoxView TopBoxes() { return job == null && pages.Count > 0 ? pages.Peek().Boxes : null; }
    // Pressing on a Pokemon picks it up, to drag; with one already held, pressing on a slot puts it down there.
    bool BoxMouseDown(MouseEventArgs e) {
        var v = TopBoxes();
        if (v == null || e.Button != MouseButtons.Left) return false;
        string hit = BoxHitAt(e.Location);
        if (hit == null) return false;
        if (hit[0] != 's') return true;   // headers act on the click
        int pane = hit[1] - '0', slot = int.Parse(hit.Substring(3));
        v.Pane = pane; v.Cursor = slot; v.Col = slot % 6;
        if (v.Holding) { BoxDrop(v, pane, slot); swallowClick = true; Invalidate(); return true; }
        if (v.Side[pane].At(v.Box[pane], slot) == null) return true;
        v.HeldPane = pane; v.HeldBox = v.Box[pane]; v.HeldSlot = slot; v.Note = null;
        v.Dragging = true; v.Dragged = false; v.DragStart = v.Drag = new PointF(e.X - screenX, e.Y - screenY);
        swallowClick = true; Invalidate();
        return true;
    }
    bool BoxMouseMove(MouseEventArgs e) {
        var v = TopBoxes();
        if (v == null) return false;
        string hit = BoxHitAt(e.Location);
        if (v.Dragging) {
            v.Drag = new PointF(e.X - screenX, e.Y - screenY);
            if (Math.Abs(v.Drag.X - v.DragStart.X) + Math.Abs(v.Drag.Y - v.DragStart.Y) > 8) v.Dragged = true;
            // Held over a box arrow, the box turns, as HOME does.
            string arrow = hit != null && (hit[0] == 'l' || hit[0] == 'r') ? hit : null;
            if (arrow != v.HoverArrow) { v.HoverArrow = arrow; v.HoverSince = DateTime.Now; }
            else if (arrow != null && (DateTime.Now - v.HoverSince).TotalSeconds > 0.7) {
                int pane = arrow[1] - '0'; var s = v.Side[pane];
                v.Box[pane] = (v.Box[pane] + (arrow[0] == 'r' ? 1 : -1) + s.Boxes) % s.Boxes; v.HoverSince = DateTime.Now;
            }
            Invalidate();
        }
        if (hit != null && hit[0] == 's') { int pane = hit[1] - '0', slot = int.Parse(hit.Substring(3)); if (v.Pane != pane || v.Cursor != slot) { v.Pane = pane; v.Cursor = slot; v.Col = slot % 6; Invalidate(); } }
        return true;
    }
    bool BoxMouseUp(MouseEventArgs e) {
        var v = TopBoxes();
        if (v == null || !v.Dragging) return false;
        v.Dragging = false; v.HoverArrow = null;
        string hit = BoxHitAt(e.Location);
        // A press and release on the same place is a pick-up: the next press puts it down.
        if (!v.Dragged) { Invalidate(); return true; }
        if (hit != null && hit[0] == 's') BoxDrop(v, hit[1] - '0', int.Parse(hit.Substring(3)));
        else v.HeldPane = -1;
        Invalidate();
        return true;
    }
    bool swallowClick;
    bool BoxClick(MouseEventArgs e) {
        var v = TopBoxes();
        if (v == null) return false;
        if (swallowClick) { swallowClick = false; return true; }
        if (e.Button != MouseButtons.Left) return false;
        string hit = BoxHitAt(e.Location);
        if (hit == null) return false;
        int pane = hit[1] - '0'; var s = v.Side[pane];
        if (hit[0] == 'l' || hit[0] == 'r') { v.Box[pane] = (v.Box[pane] + (hit[0] == 'r' ? 1 : -1) + s.Boxes) % s.Boxes; v.Pane = pane; v.Cursor = -1; v.Note = null; }
        else if (hit[0] == 'h' && !v.Holding) ChooseBoxGame(v, pane);
        Invalidate();
        return true;
    }
    Page MoveFromPage() {
        var p = new Page { Title = "Move Pokemon: from which game?" };
        foreach (var game in Library.Games.Where(x => x.SaveTools)) {
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
        foreach (var game in Library.Games.Where(x => x.SaveTools && x != from)) {
            var g = game;
            p.Items.Add(new Item(g.Title, "", true, delegate { MoveNow(from, file, box, slot, name, g); }) { Icon = '', Theme = g.Theme, Info = "Moves it to the first free PC box slot. If something stands in the way, you'll be asked how to resolve it." });
        }
        return p;
    }
    // First ask what stands in the way. Anything the player must decide goes on a page; a clean move just happens.
    void MoveNow(GameEntry from, string file, string box, string slot, string name, GameEntry to) {
        string at = moveAt; Action after = moveAfter; moveAt = null; moveAfter = null;
        MoveNow(from, file, box, slot, name, to, at, after);
    }
    void MoveNow(GameEntry from, string file, string box, string slot, string name, GameEntry to, string at, Action after) {
        if (AfterCapture(delegate { MoveNow(from, file, box, slot, name, to, at, after); })) return;
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
            if (!forced && !rows.Exists(f => f[0] == "conflict")) { RunMove(from, file, box, slot, name, to, target, "", at, after); return; }
            pages.Push(ResolvePage(from, file, box, slot, name, to, target, rows, forced, at, after));
        });
    }
    // A blocked move: the player deletes or changes each conflict, then retries.
    Page ResolvePage(GameEntry from, string file, string box, string slot, string name, GameEntry to, string target, List<string[]> rows, bool forced, string at, Action after) {
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
            RunMove(from, file, box, slot, name, to, target, opts, at, after);
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
    void RunMove(GameEntry from, string file, string box, string slot, string name, GameEntry to, string target, string options, string at, Action after) {
        string message = null; bool ok = false, notes = false;
        Busy("Moving " + name, delegate {
            int code; string o = Bridge("move \"" + file + "\" " + box + " " + slot + " \"" + target + "\" \"" + Path.Combine(Config.State, "save-backups") + "\" " + options + (at != null ? " " + at : ""), out code);
            var f = o.Trim().Split('\t');
            if (code != 0) { message = f.Length > 1 ? f[1] : o.Trim(); return null; }
            // Destination first: any failure after this duplicates the Pokemon, it never loses it.
            string back = ImportBack(to, target) ?? ImportBack(from, file);
            ok = back == null;
            // ok, name, legal or not-legal, restored (or empty), and what PKHeX's legality check found
            bool restored = f.Length > 3 && f[3] == "restored", notLegal = f.Length > 2 && f[2] == "not-legal";
            string why = f.Length > 4 ? f[4].Trim() : "";
            notes = ok && (notLegal || restored);
            message = ok ? name + " is now in " + to.Title + "'s PC boxes." + (restored ? "\n\nIts original data is back: its origin, ball and moves, as they were before it left." : "") + (notLegal ? "\n\nPKHeX's legality check flags it as not legal, which matters only for online play." + (why != "" ? "\n\nWhat it found: " + why + "." : "") + (restored ? "" : "\n\nIts original data is kept, and comes back if it returns to its own generation.") : "") : back;
            return null;
        }, delegate(string e) {
            if (after != null) {
                // From the box screen: back to it, read again; a word only when there's something to say.
                while (pages.Count > 1 && pages.Peek().Boxes == null) pages.Pop();
                after();
                if (e != null || !ok || notes) ShowText(ok ? "Moved" : "Not moved", e ?? message, null);
                return;
            }
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
        // The waiting game is kept through everything before it starts (a picture job finishing, the backup and sharing);
        // PlayGen1 decides whether it's still good.
        if (ready != null && readyFor.Id == g.Id) readyHeld = true;
        // A snapshot still being made can't be used now, and the game gets the PC.
        StopSnapshot();
        if (AfterCapture(delegate { Play(g); })) return;
        string running = RunningGames();
        if (running != "") {
            waiting = g.Id;
            ShowText("Waiting", "Already running: " + running + "\nOnly one game at a time. Close it, or wait here: this continues by itself.\n\nB: cancel", delegate { waiting = null; pages.Pop(); });
            return;
        }
        current = g; Config.Write("last-game.txt", g.Id);
        if (g.Engine() == "pc") { Busy("Backing up your saves", delegate { PcGames.Backup(g); return null; }, delegate(string e) { PlayPc(g); }); return; }
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
    // Prepares every Gen1Recomp game behind the splash (notes/launcher.md, Gen1Recomp).
    void PrepareAll() {
        if (RunningGames() != "") return;
        var todo = Library.Games.Where(g => g.Gen1 != null && !g.Imported && g.Gen1Rom != null && !PrepareFailed(g.Gen1Rom)).ToList();
        if (todo.Count == 0) return;
        // Three at once, two extract workers each. Defender scanning the files is most of the load.
        int lanes = Math.Max(1, Math.Min(3, Environment.ProcessorCount / 6));
        int workers = Math.Max(1, Math.Min(2, Environment.ProcessorCount / (lanes * 4)));
        int done = 0;
        Action<GameEntry> one = g => {
            if (PrepareNow(g, workers)) g.Imported = true; else NotePrepareFailed(g.Gen1Rom);
            int n = System.Threading.Interlocked.Increment(ref done);
            startStage = n < todo.Count ? "Preparing your games (" + n + " of " + todo.Count + " ready)" : "Almost ready";
        };
        startStage = todo.Count == 1 ? "Preparing " + todo[0].Title : "Preparing your games";
        one(todo[0]);
        // Biggest ROMs first: a GBA game takes several times longer than a Game Boy one, so the slow ones overlap.
        var rest = todo.Skip(1).OrderByDescending(g => { try { return new FileInfo(g.Gen1Rom).Length; } catch { return 0L; } }).ToList();
        Parallel.ForEach(System.Collections.Concurrent.Partitioner.Create(rest, true), new ParallelOptions { MaxDegreeOfParallelism = lanes }, one);
    }
    // The opening game's title screen and town are made once, before the menu appears.
    void WallpapersAtStart() {
        string last = Config.Read("last-game.txt", "");
        var first = Library.Games.Find(g => g.Id == last && g.Playable) ?? Library.Games.Find(g => g.Playable);
        if (RunningGames() == "")
            GameArt.CaptureAtStart(first, delegate(int n, int total) { startStage = "Making wallpapers (" + (n + 1) + " of " + total + ")"; });
        if (first != null) { GameArt.Frame(first.Id, 0); GameArt.Town(first.Id); }
    }
    static bool PrepareNow(GameEntry g, int workers) {
        var psi = Gen1Start("");
        psi.EnvironmentVariables["POKEPORT_EXTRACT_WORKERS"] = workers.ToString();
        psi.EnvironmentVariables["POKEPORT_IMPORT_ONLY"] = "1";
        psi.EnvironmentVariables["POKEPORT_FORCE_IMPORT"] = "1";
        using (var rom = new PlainPath(g.Gen1Rom)) {
            psi.EnvironmentVariables["POKEPORT_IMPORT_ROM"] = rom.Path_;
            using (var p = Process.Start(psi)) {
                // Below whatever the player has open, so a video or game they're running keeps its CPU.
                try { p.PriorityClass = ProcessPriorityClass.BelowNormal; } catch { }
                p.StandardError.ReadToEndAsync(); p.StandardOutput.ReadToEndAsync();
                if (!p.WaitForExit(300000)) { try { p.Kill(); p.WaitForExit(5000); } catch { } return false; }
                return p.ExitCode == 0;
            }
        }
    }
    static string PrepareFailedFile { get { return Path.Combine(Config.State, "prepare-failed.txt"); } }
    static string PrepareKey(string rom) { var f = new FileInfo(rom); return rom + "|" + f.Length + "|" + f.LastWriteTimeUtc.Ticks; }
    static bool PrepareFailed(string rom) {
        try { return File.Exists(PrepareFailedFile) && File.ReadAllLines(PrepareFailedFile).Contains(PrepareKey(rom)); } catch { return false; }
    }
    static void NotePrepareFailed(string rom) {
        try { File.AppendAllText(PrepareFailedFile, PrepareKey(rom) + Environment.NewLine); } catch { }
    }
    void PlayGen1(GameEntry g) {
        // The waiting game is used only if its options and save haven't changed since it loaded.
        if (ReadyFor(g)) { UseReady(g, delegate(int code) { Finished(g, code); }); return; }
        if (ready != null) QuickNote("not used for " + g.Id);
        DropReady();
        var psi = Gen1Start(" --game=" + g.Gen1);
        string options = Gen1Options(g);
        if (options != "") psi.EnvironmentVariables["NATIVEMONS_OPTIONS"] = options;
        if (QuickStart) psi.EnvironmentVariables["NATIVEMONS_TO_TITLE"] = "1";
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    void PlayPc(GameEntry g) {
        var psi = new ProcessStartInfo(PcGames.Exe(g)) { UseShellExecute = false, WorkingDirectory = PcGames.Dir(g), RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        try { PcGames.PrepareSaves(g, psi); } catch (Exception x) { ShowText("Could not start " + g.Title, "Its save folder couldn't be set up: " + x.Message, null); return; }
        DateTime started = DateTime.Now;
        RunJob(g.Title, psi, true, delegate(int code) {
            // Some games start through a program that quits at once: then the session lasts while their folder's programs run.
            if (!PcGames.IsCustom(g) || (DateTime.Now - started).TotalSeconds > 15) { Finished(g, code); return; }
            string dir = PcGames.Dir(g);
            Busy("Playing " + g.Title, delegate {
                DateTime quietSince = DateTime.Now;
                while ((DateTime.Now - quietSince).TotalSeconds < 3) {
                    if (PcGames.RunningFrom(dir)) quietSince = DateTime.Now;
                    System.Threading.Thread.Sleep(500);
                }
                return null;
            }, delegate(string e) { Finished(g, 0); });
        });
    }
    // ---------- PC games the player adds (CustomGames) ----------
    // From its folder, its .zip or its program (notes/launcher.md, PC games).
    void AddPcGame() {
        var p = new Page { Title = "Add a PC game", Body = "Any game with its own program, such as a fan game. It runs as it is, with no extras, and keeps its saves wherever it always does." };
        p.Items.Add(new Item("From its folder", "Runs where it is. Nothing is copied.", true, delegate {
            string dir = PickFolder("Choose the game's folder");
            if (dir == null) return;
            var programs = CustomGames.Programs(dir);
            if (programs.Count == 0) { ShowText("No program there", "There's no .exe in " + dir + " or its folders, so there's nothing to start.", null); return; }
            pages.Pop();
            ChooseProgram(programs, dir, delegate(string exe) { AddCustom(delegate(Action<string, float> report, out CustomGames.Entry added) { return CustomGames.Add(exe, false, report, out added); }, "Adding the game"); }, null);
        }) { Icon = '\uE8B7', Info = "Point to the folder the game lives in. The launcher finds its program and leaves every file where it is." });
        p.Items.Add(new Item("From a .zip", "Unpacked into the launcher's games folder.", true, delegate {
            string zip;
            using (var d = new OpenFileDialog { Filter = "Zip (*.zip)|*.zip", Title = "Choose the game's .zip" }) { if (d.ShowDialog(this) != DialogResult.OK) return; zip = d.FileName; }
            pages.Pop();
            string work = null, problem = null; List<string> programs = null;
            Busy("Unpacking the game", delegate {
                problem = CustomGames.Unpack(zip, delegate(string what, float at) { BeginInvoke((Action)delegate { progress = at; AddLine("nativemons: " + what); dirty = true; }); }, out work, out programs);
                return null;
            }, delegate(string e) {
                if (e != null || problem != null) { ShowText("Not added", e ?? problem, null); return; }
                string w = work;
                ChooseProgram(programs, w, delegate(string exe) { AddCustom(delegate(Action<string, float> report, out CustomGames.Entry added) { return CustomGames.AddUnpacked(zip, w, exe, out added); }, "Adding the game"); },
                    delegate { try { PcGames.Remove(w); } catch { } });
            });
        }) { Icon = '\uE7B8', Info = "The launcher unpacks the zip into games\\, finds the game's program and keeps the copy, removed again with the game. The zip stays as it is." });
        p.Items.Add(new Item("Choose its program (.exe)", "Then run it where it is, or copy its folder.", true, delegate { pages.Pop(); AddPcGameExe(); }) { Icon = '\uE7FC' });
        p.Items.Add(new Item("Cancel", "", true, delegate { pages.Pop(); }) { Icon = '\uE711' });
        pages.Push(p);
    }
    // One program goes ahead; several are listed, the likeliest first. cancelled: what to undo if none is chosen.
    void ChooseProgram(List<string> programs, string root, Action<string> chosen, Action cancelled) {
        if (programs.Count == 1) { chosen(programs[0]); return; }
        var p = new Page { Title = "Which one starts the game?", Body = "This folder has more than one program." };
        foreach (var exe in programs.Take(12)) {
            string e = exe;
            p.Items.Add(new Item(Path.GetFileName(e), "", true, delegate { pages.Pop(); chosen(e); }) { Icon = '\uE7FC', Info = e.Substring(root.TrimEnd('\\').Length + 1) });
        }
        p.Items.Add(new Item("Cancel", cancelled != null ? "Nothing is added, and the unpacked files are deleted." : "", true, delegate { pages.Pop(); if (cancelled != null) cancelled(); }) { Icon = '\uE711' });
        pages.Push(p);
    }
    delegate string CustomAdd(Action<string, float> report, out CustomGames.Entry added);
    void AddCustom(CustomAdd work, string title) {
        CustomGames.Entry added = null; string problem = null;
        Busy(title, delegate {
            problem = work(delegate(string what, float at) { BeginInvoke((Action)delegate { progress = at; AddLine("nativemons: " + what); dirty = true; }); }, out added);
            return null;
        }, delegate(string e) {
            if (e != null || problem != null) { ShowText("Not added", e ?? problem, null); return; }
            string id = added.id;
            Rescan(delegate {
                while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
                pages.Push(GamesPage());
                var g = Library.Games.Find(x => x.Id == id);
                if (g != null) pages.Push(GamePage(g));
            });
        });
    }
    void AddPcGameExe() {
        string exe;
        using (var d = new OpenFileDialog { Filter = "Programs (*.exe)|*.exe", Title = "Choose the game's program (.exe)" }) {
            if (d.ShowDialog(this) != DialogResult.OK) return;
            exe = d.FileName;
        }
        var p = new Page { Title = "Add " + Path.GetFileNameWithoutExtension(exe) };
        p.Body = exe + "\n\nRun it where it is, or let the launcher keep its own copy of the game's folder (removed again with it). Either way the game keeps its saves wherever it always does.";
        Action<bool> add = delegate(bool copy) {
            CustomGames.Entry added = null; string problem = null;
            Busy(copy ? "Copying the game" : "Adding the game", delegate {
                problem = CustomGames.Add(exe, copy, delegate(string what, float at) { BeginInvoke((Action)delegate { progress = at; AddLine("nativemons: " + what); dirty = true; }); }, out added);
                return null;
            }, delegate(string e) {
                if (e != null || problem != null) { ShowText("Not added", e ?? problem, null); return; }
                string id = added.id;
                Rescan(delegate {
                    while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
                    pages.Push(GamesPage());
                    var g = Library.Games.Find(x => x.Id == id);
                    if (g != null) pages.Push(GamePage(g));
                });
            });
        };
        p.Items.Add(new Item("Run it where it is", "Nothing is copied.", true, delegate { pages.Pop(); add(false); }) { Icon = '' });
        p.Items.Add(new Item("Copy it into the launcher", "Its whole folder, kept in games\\.", true, delegate { pages.Pop(); add(true); }) { Icon = '' });
        p.Items.Add(new Item("Cancel", "", true, delegate { pages.Pop(); }) { Icon = '' });
        pages.Push(p);
    }
    Page CustomGamePage(GameEntry g) {
        var p = new Page { Title = g.Title, Theme = g.Theme };
        var c = CustomGames.Find(g.Id);
        if (c == null) { p.Body = "This game is no longer in the launcher."; return p; }
        string version = PcGames.Version(g);
        if (!g.OnPc) p.Body = "Its program isn't at " + CustomGames.Exe(c) + " any more. Remove it here, then add it again from where it is now.";
        var play = new Item("Play", version != null ? "Version " + version + "." : "Start " + g.Title + ".", g.OnPc, delegate { Play(g); }) { Icon = '' };
        play.Records = new[] { (c.copy ? "The launcher's copy: " : "Runs from: ") + CustomGames.Exe(c), "Saves: wherever the game keeps them" };
        p.Items.Add(play);
        p.Items.Add(new Item("Rename", "Shown as " + g.Title + ".", true, delegate {
            string name = AskText("Rename", "Name for this game:", g.Title);
            if (string.IsNullOrWhiteSpace(name)) return;
            CustomGames.Rename(g.Id, name.Trim());
            string id = g.Id;
            Rescan(delegate { pages.Pop(); pages.Pop(); pages.Push(GamesPage()); var fresh = Library.Games.Find(x => x.Id == id); if (fresh != null) pages.Push(GamePage(fresh)); ReplaceMain(); });
        }) { Icon = '' });
        p.Items.Add(new Item("Open its folder", "", Directory.Exists(CustomGames.Dir(c)), delegate { try { Process.Start("explorer.exe", "\"" + CustomGames.Dir(c) + "\""); } catch { } }) { Icon = '', Info = CustomGames.Dir(c) });
        p.Items.Add(new Item("Remove from the launcher", c.copy ? "Deletes the launcher's copy. Saves stay." : "Forgets it. Nothing is deleted.", true, delegate {
            if (RunningGames() != "") { ShowText("Close your game first", "Already running: " + RunningGames() + ".", null); return; }
            string problem = null, id = g.Id;
            Busy("Removing " + g.Title, delegate { problem = CustomGames.Remove(id); return null; }, delegate(string e) {
                if (e != null || problem != null) { ShowText("Not removed", e ?? problem, null); return; }
                if (Config.Read("last-game.txt", "") == id) Config.Write("last-game.txt", "");
                Rescan(delegate { while (pages.Count > 1 && !pages.Peek().Main) pages.Pop(); pages.Push(GamesPage()); ReplaceMain(); });
            });
        }) { Icon = '' });
        return p;
    }
    Page PcGamePage(GameEntry g) {
        if (PcGames.IsCustom(g)) return CustomGamePage(g);
        var p = new Page { Title = g.Title, Theme = g.Theme };
        Action pickExisting = delegate {
            string f = PickFolder("Choose your installation's folder, the one with " + PcGames.Of(g).Exe);
            if (f == null) return;
            string problem = PcGames.AddExisting(g, f);
            if (problem != null) { ShowText("Not added", problem, null); return; }
            pages.Pop(); pages.Push(PcGamePage(g));
            Busy("Making its pictures", delegate { GameArt.Background(delegate { GameArt.Capture(g); GameArt.CaptureTown(g); }); return null; }, delegate(string e) { ReplaceMain(); });
        };
        Action<Page> importItems = delegate(Page into) {
            into.Items.Add(new Item("Import from a folder", "Copies your installation into the launcher.", true, delegate { string f = PickFolder("Choose the folder with " + PcGames.Of(g).Exe); if (f != null) InstallPc(g, f); }) { Icon = '' });
            into.Items.Add(new Item("Import from a zip", "Unpacks the game's download into the launcher.", true, delegate {
                using (var d = new OpenFileDialog { Filter = "Zip (*.zip)|*.zip", Title = "Choose " + g.Title + "'s zip" }) if (d.ShowDialog(this) == DialogResult.OK) InstallPc(g, d.FileName);
            }) { Icon = '' });
            into.Items.Add(new Item("Download", "From the game's GitHub (a few GB). Needs Git for Windows.", PcGames.Git() != null, delegate { InstallPc(g, null); }) { Icon = '' });
        };
        if (!g.OnPc) {
            p.Body = g.Title + " is a PC game of its own, " + PcGames.Of(g).Size + ".\n\nImport it, and the launcher keeps the game and its saves in its own folders and updates it from the game's GitHub. Saves you already have are copied in.\n\nOr add your existing installation: it runs where it is, unchanged, with its saves where the game keeps them (" + PcGames.GameSaveDir(g) + ").";
            importItems(p);
            p.Items.Add(new Item("Add an existing installation", "Runs your own copy where it is.", true, pickExisting) { Icon = '' });
            return p;
        }
        bool existing = PcGames.IsExisting(g);
        string version = PcGames.Version(g);
        var play = new Item("Play", version != null ? "Version " + version + "." : "Start " + g.Title + ".", true, delegate { Play(g); }) { Icon = '' };
        play.Records = new[] { (existing ? "Your installation: " : "Imported into the launcher: ") + PcGames.Dir(g), "Saves in " + PcGames.SaveDir(g) };
        p.Items.Add(play);
        if (!existing) p.Items.Add(new Item("Update", "Gets the newest release from the game's GitHub.", PcGames.Git() != null, delegate { UpdatePc(g); }) { Icon = '', Info = "The same update the game's own updater does. Your saves aren't touched." });
        p.Items.Add(new Item("Saves folder", existing ? "Where the game keeps them." : "In the launcher's folder.", true, delegate {
            string dir = PcGames.SaveDir(g);
            Directory.CreateDirectory(dir);
            try { Process.Start("explorer.exe", "\"" + dir + "\""); } catch { }
        }) { Icon = '', Info = PcGames.SaveDir(g) });
        p.Items.Add(new Item("Save backups", "Copies made before each session.", true, delegate {
            string dir = Path.Combine(Config.State, "save-backups", g.Id);
            Directory.CreateDirectory(dir);
            try { Process.Start("explorer.exe", "\"" + dir + "\""); } catch { }
        }) { Icon = '', Info = "The launcher copies your save files here before each session, keeping the newest 30. The game also keeps backups of its own in its save folder." });
        if (existing) {
            p.Items.Add(new Item("Import into the launcher", "A copy the launcher keeps and updates.", true, delegate {
                var sub = new Page { Title = "Import " + g.Title, Theme = g.Theme, Body = "The launcher makes its own copy, with its own saves (yours are copied in), and updates it. Your installation stays as it is." };
                sub.Items.Add(new Item("Import this installation", "Copies " + PcGames.Dir(g) + ".", true, delegate { InstallPc(g, PcGames.Dir(g)); }) { Icon = '' });
                importItems(sub);
                pages.Push(sub);
            }) { Icon = '' });
            p.Items.Add(new Item("Remove from the launcher", "Forgets it. Nothing is deleted.", true, delegate { PcGames.Forget(g); pages.Pop(); ShowText("Removed", g.Title + " is no longer in the launcher. Your installation and saves are where they were.", null); ReplaceMain(); }) { Icon = '' });
        } else {
            p.Items.Add(new Item("Use an existing installation", "Plays your own copy instead.", true, pickExisting) { Icon = '', Info = "This copy and its saves in the launcher's folder stay; switching back is a matter of importing again." });
            p.Items.Add(new Item("Uninstall", "Removes the launcher's copy. Saves stay.", true, delegate { pages.Push(PcRemovePage(g)); }) { Icon = '' });
        }
        return p;
    }
    Page PcRemovePage(GameEntry g) {
        var p = new Page { Title = "Uninstall " + g.Title + "?", Theme = g.Theme };
        p.Body = "This deletes the launcher's copy in " + PcGames.OwnDir(g) + ".\n\nIts saves in " + PcGames.SaveDir(g) + " stay, and so does any copy of the game you imported it from.";
        p.Items.Add(new Item("Uninstall", "", true, delegate {
            if (AfterCapture(delegate { pages.Pop(); pages.Push(PcRemovePage(g)); })) return;
            if (RunningGames() != "") { ShowText("Close your game first", "Already running: " + RunningGames() + ".", null); return; }
            Busy("Removing " + g.Title, delegate { PcGames.Remove(PcGames.OwnDir(g)); return null; }, delegate(string e) {
                pages.Pop(); pages.Pop();
                ShowText(e == null ? "Uninstalled" : "Not fully removed", e ?? g.Title + " was removed. Its saves are where they were.", null);
                ReplaceMain();
            });
        }) { Icon = '' });
        p.Items.Add(new Item("Keep it", "", true, delegate { pages.Pop(); }) { Icon = '' });
        return p;
    }
    void InstallPc(GameEntry g, string source) {
        if (RunningGames() != "") { ShowText("Close your game first", "Already running: " + RunningGames() + ".", null); return; }
        string problem = null;
        Busy("Importing " + g.Title, delegate {
            problem = PcGames.Install(g, source, delegate(string what, float at) { BeginInvoke((Action)delegate { if (at >= 0) progress = at; AddLine("nativemons: " + what); dirty = true; }); });
            return null;
        }, delegate(string e) {
            if (e != null || problem != null) { ShowText("Not imported", e ?? problem, null); return; }
            while (pages.Count > 1 && !pages.Peek().Main) pages.Pop();
            pages.Push(GamesPage()); pages.Push(PcGamePage(g));
            if (PcGames.Saved != null) ShowText("Imported", g.Title + " is in the launcher. " + PcGames.Saved, null);
            // Its title screen and town, from its own pictures, then the menu shows them.
            Busy("Making its pictures", delegate { GameArt.Background(delegate { GameArt.Capture(g); GameArt.CaptureTown(g); }); return null; }, delegate(string e2) { ReplaceMain(); });
        });
    }
    void UpdatePc(GameEntry g) {
        if (RunningGames() != "") { ShowText("Close your game first", "Already running: " + RunningGames() + ".", null); return; }
        string problem = null, before = PcGames.Version(g);
        Busy("Updating " + g.Title, delegate { problem = PcGames.Update(g, delegate(string l) { BeginInvoke((Action)delegate { AddLine("nativemons: " + l); dirty = true; }); }); return null; }, delegate(string e) {
            string after = PcGames.Version(g);
            ShowText(e != null || problem != null ? "Not updated" : "Updated", e ?? problem ?? (after == before ? "You already have the newest release: " + after + "." : "Now on " + after + "."), null);
            pages.Pop(); pages.Push(PcGamePage(g));
        });
    }
    static string Gen1Options(GameEntry g) { return string.Join(",", GameOptions.ForGame(g).Select(x => x.Key + "=" + x.Value).ToArray()); }

    // ---------- quick start ----------
    // The last game waits at its title screen while the menu is up (notes/launcher.md, Quick start).
    public static bool QuickStart { get { return Config.Read("quick-start.txt", "on").Trim() != "off"; } set { Config.Write("quick-start.txt", value ? "on" : "off"); } }
    Process ready; GameEntry readyFor; string readyOptions, readySave, readyFailed; bool readyUp, readyHeld;
    static volatile int readyPid;
    DateTime menuSince = DateTime.MaxValue;
    static void QuickNote(string what) { if (TestLog != null) try { File.AppendAllText(TestLog, "quick-start: " + what + Environment.NewLine); } catch { } }
    // Gen1Recomp games, and nativeplat's (its side: NP_QUICK_START in our gen4recomp fork, shell/src/main.c).
    static bool CanQuickStart(GameEntry g) {
        if (g == null || !g.Playable) return false;
        if (g.Engine() == "gen1") return g.Imported && g.Gen1 != null;
        return g.Engine() == "nativeplat" && g.Nativeplat != null && File.Exists(Config.Nativeplat);
    }
    static string NativeplatOptionsFile { get { return Path.Combine(NativeplatDir, "userdata", "options.ini"); } }
    // What the waiting game read when it started: its options, and its save (both games may read the save while booting).
    static string QuickOptions(GameEntry g) { return g.Engine() == "nativeplat" ? SaveHash(NativeplatOptionsFile) : Gen1Options(g); }
    string QuickSave(GameEntry g) { return SaveGuard.Hash(g.Engine() == "nativeplat" ? NativeplatSlot(g) : CurrentSave(g)); }
    bool ReadyFor(GameEntry g) {
        return ready != null && readyUp && readyFor.Id == g.Id && !ready.HasExited && readyOptions == QuickOptions(g) && readySave == QuickSave(g);
    }
    bool readyStarting;
    void QuickStartTick() {
        bool menu = job == null && !starting && waiting == null && playDirect == null && pages.Count > 0 && pages.Peek().Main;
        if (menu) readyHeld = false;
        if (!menu) menuSince = DateTime.MaxValue; else if (menuSince == DateTime.MaxValue) menuSince = DateTime.Now;
        SnapshotTick(menu && (DateTime.Now - menuSince).TotalSeconds >= 1.5);
        bool wanted = menu && QuickStart && CanQuickStart(current);
        if (ready != null) {
            bool stale = current == null || readyFor.Id != current.Id || readyOptions != QuickOptions(current) || ready.HasExited;
            if ((!wanted && !readyHeld) || stale) DropReady();
            return;
        }
        if (readyStarting || !wanted || (DateTime.Now - menuSince).TotalSeconds < 1.5 || RunningGames() != "") return;
        string key = current.Id + "|" + current.Engine() + "|" + QuickOptions(current) + "|" + (current.Gen1Rom ?? current.FulldexRom);
        if (readyFailed == key) return;
        var g = current;
        if (g.Engine() != "nativeplat") { StartReady(g, key); return; }
        // nativeplat plays a copy of the save in its own slot: put it there first, as Play does, off the window's thread.
        readyStarting = true;
        string problem = null;
        Task.Run(() => { try { PrepareNativeplat(g); } catch (Exception x) { problem = x.Message; } }).ContinueWith(t => BeginInvoke((Action)delegate {
            readyStarting = false;
            if (problem != null) { readyFailed = key; QuickNote("failed " + g.Id + ": " + problem); return; }
            bool still = job == null && pages.Count > 0 && pages.Peek().Main && current != null && current.Id == g.Id && QuickStart && RunningGames() == "";
            if (still && ready == null) StartReady(g, key);
        }));
    }
    void StartReady(GameEntry g, string key) {
        ProcessStartInfo psi;
        string options = QuickOptions(g);
        if (g.Engine() == "nativeplat") {
            psi = new ProcessStartInfo(Config.Nativeplat, "--game " + g.Nativeplat + " --slot " + NativeplatSlotName) { UseShellExecute = false, WorkingDirectory = NativeplatDir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            psi.EnvironmentVariables["NP_QUICK_START"] = "wait";
        } else {
            psi = Gen1Start(" --game=" + g.Gen1);
            if (options != "") psi.EnvironmentVariables["NATIVEMONS_OPTIONS"] = options;
            psi.EnvironmentVariables["NATIVEMONS_QUICK_START"] = "1";
        }
        psi.RedirectStandardInput = true;
        var p = new Process { StartInfo = psi, EnableRaisingEvents = true };
        DataReceivedEventHandler line = delegate(object s, DataReceivedEventArgs e) {
            if (e.Data == null) return;
            string text = e.Data;
            BeginInvoke((Action)delegate {
                if (job == p) { AddLine(text); dirty = true; return; }
                if (ready != p) return;
                if (text.Contains("nativemons: quick-start ready") || text.Contains("nativeplat: quick-start ready")) { readyUp = true; QuickNote("ready " + g.Id); }
                else if (text.Contains("nativemons: quick-start failed")) { readyFailed = key; QuickNote("failed " + g.Id + ": " + text); DropReady(); }
            });
        };
        p.OutputDataReceived += line; p.ErrorDataReceived += line;
        p.Exited += delegate { BeginInvoke((Action)delegate { if (job == p) JobEnded(); else if (ready == p) DropReady(); }); };
        try { p.Start(); } catch (Exception x) { readyFailed = key; QuickNote("could not start: " + x.Message); return; }
        // Behind whatever the player has open while it gets ready.
        try { p.PriorityClass = ProcessPriorityClass.BelowNormal; } catch { }
        p.BeginOutputReadLine(); p.BeginErrorReadLine();
        ready = p; readyPid = p.Id; readyFor = g; readyOptions = options; readyUp = false;
        readySave = QuickSave(g);
        QuickNote("starting " + g.Id + " pid " + p.Id);
    }
    // Fulldex DS quick start: a title-screen snapshot, used only while the save matches (notes/launcher.md, Quick start).
    static string SnapDir { get { return Path.Combine(Config.State, "quick-start"); } }
    static string SnapFile(GameEntry g) { return Path.Combine(SnapDir, g.Id + ".snap"); }
    static string SnapInfo(GameEntry g) { return Path.Combine(SnapDir, g.Id + ".txt"); }
    static string FulldexSave(GameEntry g) { return Path.Combine(Config.FulldexDir, "saves", g.Save); }
    static string FulldexFuncs(GameEntry g) { return g.FulldexCode == "IPGE" || g.FulldexCode == "IPKE" ? null : Path.Combine(Config.FulldexDir, "data", "funcs", g.FulldexCode + ".txt"); }
    static bool CanSnapshot(GameEntry g) {
        return g != null && g.Playable && g.Engine() == "fulldex" && g.IsDs && g.ArtFrame > 0 && g.FulldexRom != null && g.FulldexCode != null
            && File.Exists(Config.Fulldex) && !Library.NeedsPatch(g.FulldexRom);
    }
    static string FileKey(string path) { try { var f = new FileInfo(path); return f.Exists ? path + "|" + f.Length + "|" + f.LastWriteTimeUtc.Ticks : path + "|none"; } catch { return path + "|?"; } }
    static string SnapKey(GameEntry g) {
        return string.Join("\n", new[] { FileKey(g.FulldexRom), FileKey(Config.Fulldex), SaveGuard.Hash(Path.Combine(Config.FulldexDir, "settings.cfg")), FulldexFuncs(g) ?? "", g.ArtFrame.ToString() });
    }
    // The save's hash, worked out again only when the file changes: this is asked several times a second.
    static readonly Dictionary<string, KeyValuePair<string, string>> hashes = new Dictionary<string, KeyValuePair<string, string>>();
    static string SaveHash(string path) {
        string at = FileKey(path);
        KeyValuePair<string, string> known;
        lock (hashes) if (hashes.TryGetValue(path, out known) && known.Key == at) return known.Value;
        string h = SaveGuard.Hash(path);
        lock (hashes) hashes[path] = new KeyValuePair<string, string>(at, h);
        return h;
    }
    static bool SnapshotGood(GameEntry g) {
        try {
            if (!CanSnapshot(g) || !File.Exists(SnapFile(g)) || !File.Exists(SnapInfo(g))) return false;
            var info = File.ReadAllText(SnapInfo(g)).Split(new[] { "\n--\n" }, StringSplitOptions.None);
            return info.Length == 2 && info[0] == SnapKey(g) && info[1] == SaveHash(FulldexSave(g));
        } catch { return false; }
    }
    Process snapping; string snapFailed;
    static volatile int snapPid;
    bool snapStopped;
    void StopSnapshot() { var p = snapping; if (p == null) return; snapStopped = true; try { p.Kill(); } catch { } }
    void SnapshotTick(bool menuIdle) {
        // Never beside a game: the player's game gets the PC.
        if (snapping != null) { if (job != null && jobIsGame) StopSnapshot(); return; }
        if (!menuIdle || !QuickStart || !CanSnapshot(current) || SnapshotGood(current) || RunningGames() != "") return;
        var g = current;
        string key = SnapKey(g), save = FulldexSave(g), saveHash = SaveHash(save);
        if (snapFailed == key + saveHash) return;
        string work = Path.Combine(SnapDir, g.Id + "-making"), copy = Path.Combine(work, "source.sav"), made = Path.Combine(work, "title.snap");
        try {
            if (Directory.Exists(work)) Directory.Delete(work, true);
            Directory.CreateDirectory(work);
            if (File.Exists(save)) { File.Copy(save, copy); File.SetAttributes(copy, FileAttributes.Normal); }
            // The copy must be the save that was hashed; if the player's save moved on meanwhile, try again later.
            if (SaveGuard.Hash(copy) != saveHash) { Directory.Delete(work, true); return; }
        } catch { return; }
        string dir = Config.FulldexDir, funcs = FulldexFuncs(g);
        string args = "\"" + g.FulldexRom + "\" --save \"" + copy + "\" --no-save --config \"" + Path.Combine(dir, "settings.cfg") + "\" --keep-settings --jit"
            + (funcs != null ? " --funcs \"" + funcs + "\"" : "") + " --headless --frames " + (g.ArtFrame + 5) + " --state-save \"" + made + "\" --state-at " + g.ArtFrame;
        var p = new Process { StartInfo = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true } };
        try { p.Start(); } catch { snapFailed = key + saveHash; return; }
        try { p.PriorityClass = ProcessPriorityClass.BelowNormal; } catch { }
        snapping = p; snapPid = p.Id;
        QuickNote("snapshot making " + g.Id);
        Task.Run(() => {
            bool ok = false;
            try {
                p.StandardOutput.ReadToEndAsync(); p.StandardError.ReadToEndAsync();
                if (!p.WaitForExit(180000)) { try { p.Kill(); } catch { } }
                else if (p.ExitCode == 0 && File.Exists(made) && SnapKey(g) == key) {
                    if (File.Exists(SnapFile(g))) File.Delete(SnapFile(g));
                    File.Move(made, SnapFile(g));
                    File.WriteAllText(SnapInfo(g), key + "\n--\n" + saveHash);
                    ok = true;
                }
            } catch { }
            try { Directory.Delete(work, true); } catch { }
            BeginInvoke((Action)delegate {
                // Stopped for a game isn't a failure: it's made again later.
                bool stopped = snapStopped;
                snapping = null; snapPid = 0; snapStopped = false;
                if (!ok && !stopped) snapFailed = key + saveHash;
                QuickNote(ok ? "snapshot ready " + g.Id : "snapshot failed " + g.Id);
            });
        });
    }
    // Ends the waiting game: closing its input tells it to quit, and it's stopped if it doesn't.
    void DropReady(bool wait = false) {
        var p = ready;
        if (p == null) return;
        ready = null; readyPid = 0; readyUp = false; readyFor = null;
        QuickNote("dropped");
        // nativeplat writes its save slot as it closes: it gets longer, so it's never stopped part way through.
        int grace = 2000;
        try { if (p.ProcessName.Equals("nativeplat", StringComparison.OrdinalIgnoreCase)) grace = 15000; } catch { }
        var t = Task.Run(() => { try { p.StandardInput.Close(); if (!p.WaitForExit(grace)) p.Kill(); p.WaitForExit(2000); p.Dispose(); } catch { } });
        if (wait) t.Wait(grace + 3000);
    }
    void UseReady(GameEntry g, Action<int> done) {
        var p = ready; ready = null; readyPid = 0; readyUp = false; readyHeld = false;
        try { p.PriorityClass = ProcessPriorityClass.Normal; } catch { }
        log.Clear(); jobTitle = g.Title; jobIsGame = true; jobHid = false; jobStart = DateTime.Now; gameStart = jobStart; progress = -1;
        jobDone = done;
        if (!Native.Quiet) Native.AllowSetForegroundWindow(-1);
        job = p;
        DiscordPresence.Playing(g);
        QuickNote("used " + g.Id);
        try { p.StandardInput.WriteLine("play"); p.StandardInput.Flush(); } catch { }
        Invalidate();
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
            // Quick start: from the title-screen snapshot, only while the save is exactly the one it was made from.
            if (QuickStart && SnapshotGood(g)) { args += " --state-load \"" + SnapFile(g) + "\""; QuickNote("snapshot used " + g.Id); }
            else if (QuickStart && CanSnapshot(g)) QuickNote("snapshot not used " + g.Id);
        } else if (File.Exists(Config.GbaPlayer)) {
            PlayGba(g, saves);
            return;
        } else {
            // GBA games are only found in Fulldex's roms folder. A copy goes there, the original stays put.
            string roms = Path.Combine(dir, "roms"), copy = Path.Combine(roms, Path.GetFileName(g.FulldexRom));
            Directory.CreateDirectory(roms);
            if (!string.Equals(Path.GetFullPath(g.FulldexRom), Path.GetFullPath(copy), StringComparison.OrdinalIgnoreCase) && !File.Exists(copy)) File.Copy(g.FulldexRom, copy);
            // It finds a game by its code, so another dump of this game in its folder could be run instead. Only copies go, whose
            // original is still in the player's own folders.
            foreach (var other in g.Roms.Where(r => r.EngineCopy && !string.Equals(Path.GetFullPath(r.File), Path.GetFullPath(copy), StringComparison.OrdinalIgnoreCase)).ToList())
                try { File.Delete(other.File); g.Roms.Remove(other); } catch { }
            foreach (var mine in g.Roms.Where(r => !r.EngineCopy && r.Sha1 != g.Chosen.Sha1)) {
                string stale = Path.Combine(roms, Path.GetFileName(mine.File));
                try { if (File.Exists(stale) && !string.Equals(stale, copy, StringComparison.OrdinalIgnoreCase) && Library.Sha1Cached(stale) == mine.Sha1) File.Delete(stale); } catch { }
            }
            // A hack shares its base game's code, so --game would start the clean game: its list starts the hack instead.
            args = g.IsHack || g.IsModified(g.Chosen) ? "" : "--game " + g.FulldexCode;
            watchGba = true; gbaPlaying = false; gbaClosed = false;
            // A romhack is made Fulldex's last game played, then started with one key press.
            gbaPress = g.IsHack || g.IsModified(g.Chosen); gbaPresses = 0; gbaPressAt = DateTime.MinValue;
            if (gbaPress) try { File.WriteAllText(Path.Combine(dir, "last-game.txt"), dir + "/roms/" + Path.GetFileName(g.FulldexRom) + Environment.NewLine); } catch { gbaPress = false; }
        }
        var psi = new ProcessStartInfo(Config.Fulldex, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        // That key press must never answer an update prompt, so update checks go nowhere.
        if (gbaPress && !g.IsDs) psi.EnvironmentVariables["FULLDEX_UPDATE_URL"] = "http://127.0.0.1:9/";
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    // Our GBA player (the Fulldex fork's gba folder): the ROM where it is, the shared save in Fulldex's saves folder.
    void PlayGba(GameEntry g, string saves) {
        string dir = Path.GetDirectoryName(Config.GbaPlayer);
        string args = "--rom \"" + g.FulldexRom + "\" --saves \"" + saves + "\" --data \"" + dir + "\"";
        if (!string.IsNullOrEmpty(g.Save)) args += " --save \"" + Path.Combine(saves, g.Save) + "\"";
        // RetroAchievements: the account Fulldex keeps.
        if (Config.FulldexDir != null) args += " --config \"" + Path.Combine(Config.FulldexDir, "settings.cfg") + "\"";
        if (Library.IsUltraVioletPatch(g.FulldexRom)) {
            var b = Library.Games.Find(x => x.Id == g.BaseId);
            var fr = b == null ? null : b.Roms.FirstOrDefault(r => r.Revision == 0 && Path.GetExtension(r.File).ToLowerInvariant() == ".gba");
            if (fr != null) args += " --base \"" + fr.File + "\"";
        }
        string full;
        if (!ReadFulldex().TryGetValue("fullscreen", out full) || full == "1") args += " --fullscreen";
        var psi = new ProcessStartInfo(Config.GbaPlayer, args) { UseShellExecute = false, WorkingDirectory = dir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        RunJob(g.Title, psi, true, delegate(int code) { Finished(g, code); });
    }
    // Fulldex opens its game list after a GBA game. Seeing it means the game has saved, so Fulldex can close.
    bool watchGba, gbaPlaying, gbaClosed, gbaPress;
    int gbaPresses; DateTime gbaPressAt;
    DateTime gbaWatchedAt;
    void WatchGba() {
        if (!watchGba || job == null || (DateTime.Now - gbaWatchedAt).TotalMilliseconds < 50) return;
        gbaWatchedAt = DateTime.Now;
        try {
            job.Refresh();
            string title = job.MainWindowTitle;
            if (title == "") return;
            bool list = title.IndexOf("Fulldex", StringComparison.OrdinalIgnoreCase) >= 0;
            if (!list) { gbaPlaying = true; gbaPress = false; }
            else if (gbaPlaying) { gbaClosed = true; watchGba = false; job.Kill(); }
            else if (gbaPress) {
                if (gbaPressAt == DateTime.MinValue) gbaPressAt = DateTime.Now.AddSeconds(1.5);
                else if (DateTime.Now >= gbaPressAt) {
                    Native.PostMessage(job.MainWindowHandle, 0x100, (IntPtr)0x0D, (IntPtr)0x001C0001);
                    Native.PostMessage(job.MainWindowHandle, 0x101, (IntPtr)0x0D, (IntPtr)unchecked((int)0xC01C0001));
                    gbaPressAt = DateTime.Now.AddSeconds(3);
                    if (++gbaPresses >= 3) gbaPress = false;
                }
            }
        } catch { }
    }
    // ---------- the Platinum PC port ----------
    // Its save goes in as it is before it starts, and comes back checked (notes/launcher.md, Saves).
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
            try { data = File.ReadAllBytes(save); if (PlatinumSave.IsOldPort(data)) data = PlatinumSave.ToDs(data); }
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
            SaveGuard.WriteInPlace(master, PortSave);
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
    // One cartridge slot per game, treated like the port's save.bin (notes/launcher.md, Saves).
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
        // Its copy follows the dump in use, or a change of version would have no effect.
        if (!File.Exists(rom) || new FileInfo(rom).Length != new FileInfo(g.NativeplatRom).Length || Library.Sha1Cached(rom) != Library.Sha1Cached(g.NativeplatRom)) {
            File.Copy(g.NativeplatRom, rom + ".part", true);
            if (File.Exists(rom)) File.Delete(rom);
            File.Move(rom + ".part", rom);
        }
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
            Action<int> done = delegate(int code) {
                string note = null;
                Busy("Saving " + g.Title, delegate { note = SaveFromNativeplat(g); return null; }, delegate(string e2) {
                    if (e2 != null || note != null) noticeAfter = e2 ?? note;
                    Finished(g, code);
                });
            };
            // The game kept waiting is used only if its slot and options are still what it read while starting.
            if (ReadyFor(g)) { UseReady(g, done); return; }
            if (ready != null) QuickNote("not used for " + g.Id);
            DropReady();
            var psi = new ProcessStartInfo(Config.Nativeplat, "--game " + g.Nativeplat + " --slot " + NativeplatSlotName) { UseShellExecute = false, WorkingDirectory = NativeplatDir, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            if (QuickStart) psi.EnvironmentVariables["NP_QUICK_START"] = "title";
            RunJob(g.Title, psi, true, done);
        });
    }
    void Finished(GameEntry g, int code) {
        Task.Run(() => { System.Threading.Thread.Sleep(5000); GameArt.Background(delegate { LoadDex(true); }); });
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
        if (!Native.Quiet) Native.AllowSetForegroundWindow(-1);
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
        if (isGame) DiscordPresence.Playing(current);
        p.BeginOutputReadLine(); p.BeginErrorReadLine();
        Invalidate();
    }
    // Gen1Recomp garbles accented paths from the environment, so such files go in as plain-named copies.
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
        if (jobIsGame) { DiscordPresence.Stop(); ReadClashes(); }
        if (gbaClosed) code = 0;
        watchGba = false; gbaClosed = false;
        if (!Visible) Show();
        if (WindowState == FormWindowState.Minimized) WindowState = FormWindowState.Normal;
        if (!Native.Quiet) Activate(); Native.Front(Handle);
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
    DateTime updateCheckedAt = DateTime.Now;
    void Poll() {
        PowerOffTick();
        if (powerOff >= 0) return;
        WatchGba();
        OpenTick();
        if ((DateTime.Now - scannedAt).TotalMilliseconds >= 250) {
            scannedAt = DateTime.Now;
            if (waiting != null && job == null && RunningGames() == "") {
                string id = waiting; waiting = null; pages.Pop();
                Play(Library.Games.Find(g => g.Id == id));
            }
            WatchGame();
            RequestTick();
            QuickStartTick();
            // A launcher left open still finds out.
            if (job == null && (DateTime.Now - updateCheckedAt).TotalHours >= 6) { updateCheckedAt = DateTime.Now; Task.Run(() => Updater.Check()); }
        }
        // A tablet while Move Pokemon's boxes are anywhere in the page stack, a phone again once they've closed.
        MorphTowards(pages.Any(pg => pg.Boxes != null));
        if (BannerAge((float)clock.Elapsed.TotalSeconds) >= 0) dirty = true;
        if (ClashShowing() && (float.IsNaN(clashAt) || (float)clock.Elapsed.TotalSeconds - clashAt < 0.4f)) dirty = true;
        if (Sprites.Arrived != spritesSeen) { spritesSeen = Sprites.Arrived; dirty = true; }
        if (dirty || Animating()) { dirty = false; Invalidate(); }
        // A highlighted app moves at the full rate, so its motion stays smooth.
        if (Visible && job == null && pages.Count > 0 && pages.Peek().Main && pages.Peek().Sel >= 0 && frame % 2 == 0) Invalidate();
        if (Visible && ++frame % 2 == 0 && (job != null || starting || scrolling || spinning || (pages.Count > 0 && pages.Peek().Main))) Invalidate();
        // Only while the menu is in front: XInput reports the pad to every process.
        if (job != null || starting || Form.ActiveForm != this) { prevPad = Pad(); return; }
        ushort b = Pad(), pressed = (ushort)(b & ~prevPad);
        if ((pressed & A) != 0) Act("ok", true);
        else if ((pressed & B) != 0) Act("back", true);
        else if ((pressed & UP) != 0) Act("up", true);
        else if ((pressed & DOWN) != 0) Act("down", true);
        else if ((pressed & LEFT) != 0) Act("left", true);
        else if ((pressed & RIGHT) != 0) Act("right", true);
        else if (TopBoxes() != null && (pressed & X) != 0) Act("switch", true);
        else if (TopBoxes() != null && (pressed & (LB | RB)) != 0) Act((pressed & LB) != 0 ? "boxprev" : "boxnext", true);
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
            // The box screen's own: Tab or X the other game, Q and E (or Page Up and Down) its boxes.
            if (TopBoxes() != null) switch (key) {
                case Keys.Tab: case Keys.X: Act("switch", false); return true;
                case Keys.Q: case Keys.PageUp: Act("boxprev", false); return true;
                case Keys.E: case Keys.PageDown: Act("boxnext", false); return true;
            }
        }
        return base.ProcessCmdKey(ref msg, key);
    }
    // Mouse: hovering selects, a left click runs it, a right click goes back. Rectangles come from the last paint.
    bool scrolling;   // a grid is still gliding, so keep redrawing
    protected override void OnMouseWheel(MouseEventArgs e) {
        base.OnMouseWheel(e);
        if (Morphing) return;
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
        if (Morphing) return;
        if (PromptAt(e.Location) == null && BoxMouseDown(e)) return;
        if (e.Button == MouseButtons.Left && PromptAt(e.Location) == null && HitAt(e.Location) < 0) { Native.ReleaseCapture(); Native.SendMessage(Handle, 0xA1, (IntPtr)2, IntPtr.Zero); }
    }
    protected override void OnMouseUp(MouseEventArgs e) {
        base.OnMouseUp(e);
        if (Morphing) return;
        BoxMouseUp(e);
    }
    protected override bool ShowWithoutActivation { get { return Native.Quiet; } }
    readonly List<KeyValuePair<RectangleF, string>> promptHits = new List<KeyValuePair<RectangleF, string>>();
    string hoverPrompt;
    string PromptAt(Point pt) { foreach (var h in promptHits) if (h.Key.Contains(pt)) return h.Value; return null; }
    protected override void OnMouseMove(MouseEventArgs e) {
        base.OnMouseMove(e);
        if (Morphing) return;
        string over = PromptAt(e.Location);
        if (over != hoverPrompt) { hoverPrompt = over; Cursor = over != null ? Cursors.Hand : Cursors.Default; Invalidate(); }
        if (BoxMouseMove(e)) return;
        int i = HitAt(e.Location);
        if (job == null && over != "N" && over != "NX" && over != "C" && i >= 0 && pages.Count > 0 && pages.Peek().Sel != i) { pages.Peek().Sel = i; Invalidate(); }
    }
    protected override void OnMouseClick(MouseEventArgs e) {
        base.OnMouseClick(e);
        if (Morphing) return;
        string prompt = e.Button == MouseButtons.Left ? PromptAt(e.Location) : null;
        if (prompt == "M") { WindowState = FormWindowState.Minimized; return; }
        if (prompt == "X" && job == null) { Close(); return; }
        if (job != null) return;
        if (prompt == "NX") { bannerClosed = bannerBuild.ToString(); Config.Write("update-dismissed.txt", bannerClosed); Invalidate(); return; }
        if (prompt == "N") { bannerAt = -BannerSeconds * 2; OpenUpdates(); return; }
        if (prompt == "C") { OpenClashes(); return; }
        if (prompt != null) {
            Act(prompt == "A" ? "ok" : prompt == "B" ? "back" : prompt == "U" ? "up" : prompt == "D" ? "down" : prompt == "L" ? "left" : prompt == "R" ? "right" : "", false);
            return;
        }
        if (e.Button == MouseButtons.Right) { Act("back", false); return; }
        if (BoxClick(e)) return;
        if (e.Button != MouseButtons.Left) return;
        int i = HitAt(e.Location);
        if (i >= 0 && pages.Count > 0) { pages.Peek().Sel = i; Act("ok", false); }
    }
    // The same action from the other source within 150 ms is dropped: Steam can send a key for a press XInput reports.
    void Act(string a, bool fromPad) {
        if (a == lastAction && fromPad != lastFromPad && (DateTime.Now - lastActionTime).TotalMilliseconds < 150) return;
        lastAction = a; lastFromPad = fromPad; lastActionTime = DateTime.Now;
        if (pages.Count == 0 || openRun != null || powerOff >= 0) return;
        var p = pages.Peek();
        if (a == "back" && p.Boxes != null && p.Boxes.Holding) {
            // B puts a held Pokemon back where it was, before it leaves the boxes.
            p.Boxes.HeldPane = -1; p.Boxes.Dragging = false;
        } else if (a == "back") {
            if (p.OnBack != null) p.OnBack(); else pages.Pop();
            if (pages.Count == 0) pages.Push(MainPage());
        } else if (p.Boxes != null) {
            BoxAct(p, a);
        } else if (p.DexId > 0) {
            if (a == "up" || a == "down") p.BodyScroll += (a == "down" ? 1 : -1) * bodyFont.Height * 3;
            else if (a == "ok") { if (DexShiny(p.DexId, p.DexOnly)) p.DexShiny = !p.DexShiny; }
            else if ((a == "left" || a == "right") && p.DexOrder != null && p.DexOrder.Length > 0) {
                // The next Pokemon you've met, as the games' Pokedex steps through them.
                int at = Array.IndexOf(p.DexOrder, p.DexId), next = at + (a == "right" ? 1 : -1);
                if (at >= 0 && next >= 0 && next < p.DexOrder.Length) { pages.Pop(); pages.Push(DexEntryPage(p.DexOrder[next], p.DexOrder, p.DexOnly)); }
            }
        } else if (p.Items.Count > 0) {
            if (a == "ok") {
                var it = p.Items[p.Sel];
                if (it.On && it.Run != null && p.Main && p.Sel >= 1 && p.Sel < AppNames.Length && AppNames[p.Sel] != "Move" && AppNames[p.Sel] != "Exit" && !appRects[p.Sel].IsEmpty) {
                    openRun = it.Run; openApp = p.Sel; openFrom = appRects[p.Sel]; openAt = (float)clock.Elapsed.TotalSeconds;
                } else if (it.On && it.Run != null) it.Run();
            }
            else if (p.Main) p.Sel = GridMove(p.Sel, a, p.Items.Count);
            else if ((a == "left" && p.Items[p.Sel].Fold == 1) || (a == "right" && p.Items[p.Sel].Fold == 2)) p.Items[p.Sel].Run();
            else if ((a == "left" || a == "right") && p.Items[p.Sel].Fold != 0) { }
            else if (p.Jump > 0 && a == "left") p.Sel = Math.Max(0, p.Sel - p.Jump);
            else if (p.Jump > 0 && a == "right") p.Sel = Math.Min(p.Items.Count - 1, p.Sel + p.Jump);
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
    // A handheld device in the selected version's colour, showing cream dialog boxes.
    static readonly Color Cream = Color.FromArgb(250, 246, 230), CreamDim = Color.FromArgb(218, 214, 198), Line = Color.FromArgb(38, 42, 54);
    static readonly Color Text1 = Color.FromArgb(38, 40, 50), Text2 = Color.FromArgb(92, 94, 106), Text3 = Color.FromArgb(150, 148, 136);
    static readonly Color FootInk = Color.FromArgb(196, 202, 214);
    readonly Stopwatch clock = Stopwatch.StartNew();

    // Set while a loading Poke Ball is on screen, so the window keeps redrawing and it turns.
    bool spinning;
    protected override void OnPaint(PaintEventArgs ev) {
        spinning = false;
        if (drawW > 0) { ev.Graphics.TranslateTransform(drawX, 0); Render(ev.Graphics, drawW, ClientSize.Height); return; }
        Render(ev.Graphics, ClientSize.Width, ClientSize.Height); Outro(ev.Graphics, ClientSize.Width, ClientSize.Height);
    }

    string screenKey = ""; int screenDepth, screenDir = 1, selIndex = -1; float screenAt = -1, selAt = -1, selEase = 1;
    static float Ease(float x) { x = Math.Max(0, Math.Min(1, x)); float r = 1 - x; return 1 - r * r * r; }
    // Power down on close: the screen (CrtTime) with the lights cut just after it (LightsAt); the device leaves while their glow dies.
    const float CrtTime = 0.45f, LightsAt = 0.1f, LightsTime = 0.3f, LiftTime = 0.24f, FallTime = 0.5f;
    float powerOff = -1; int powerTop; bool poweredOff;
    void PowerOffTick() {
        if (powerOff < 0 || poweredOff) return;
        float u = (float)clock.Elapsed.TotalSeconds - powerOff;
        PowerLevel = PowerCurve(u - LightsAt);
        if (u >= LightsAt && FrozenT < 0) FrozenT = powerOff + LightsAt;
        float k = u - LightsAt - LightsTime;
        bool gone = false;
        if (k > 0) {
            // Put away: down to the bottom of the screen, a soft lift as a hand picks it up, then it falls away out of sight.
            var scr = Screen.FromControl(this);
            int rest = Math.Max(powerTop, scr.WorkingArea.Bottom - Height), bottom = scr.Bounds.Bottom + 20;
            float lift = Height * 0.065f, peak = rest - lift, g = 2 * (bottom - peak) / (FallTime * FallTime);
            float drop = (float)Math.Sqrt(2 * (rest - powerTop) / g), y;
            if (k < drop) y = powerTop + 0.5f * g * k * k;
            else if (k < drop + LiftTime) { float q = (k - drop) / LiftTime; y = rest - lift * q * q * (3 - 2 * q); }
            else { float tau = k - drop - LiftTime; y = peak + 0.5f * g * tau * tau; }
            Top = (int)y;
            gone = y >= bottom;
        }
        Invalidate();
        if (gone) { poweredOff = true; Close(); }
    }
    bool Animating() { float t = (float)clock.Elapsed.TotalSeconds; return powerOff >= 0 || t - screenAt < 0.2f || t - selAt < 0.14f || splashShot != null || openRun != null || t - openAt < OpenTime + 0.3f || t - bounceAt < 1.4f; }
    // An app opening: its icon grows to fill the screen, then the screen it opens has its rows pop in one by one.
    const float OpenTime = 0.3f;
    float openAt = -10, bounceAt = -10; int openApp; RectangleF openFrom; Action openRun; bool bounceArm;
    readonly RectangleF[] appRects = new RectangleF[8];
    static float BackOut(float x) { x = Math.Max(0, Math.Min(1, x)) - 1; return 1 + 2.4f * x * x * x + 1.4f * x * x; }
    void OpenTick() {
        if (openRun == null || (float)clock.Elapsed.TotalSeconds - openAt < OpenTime) return;
        var run = openRun; openRun = null; bounceArm = true; run();
    }
    void RenderOpen(Graphics g, int w, int h, float t) {
        float k = (t - openAt) / OpenTime;
        if (k < 0 || openApp <= 0 || k > 1 + 0.25f / OpenTime) return;
        var all = new RectangleF(0, 0, w, h);
        float e = Ease(Math.Min(1, k)), fade = k <= 1 ? 1 : 1 - (k - 1) * OpenTime / 0.25f;
        var r = new RectangleF(openFrom.X + (all.X - openFrom.X) * e, openFrom.Y + (all.Y - openFrom.Y) * e, openFrom.Width + (all.Width - openFrom.Width) * e, openFrom.Height + (all.Height - openFrom.Height) * e);
        var c = AppColors[openApp];
        if (fade >= 1) GlossTile(g, r, openFrom.Width * 0.24f * (1 - e), c);
        else using (var b = new SolidBrush(Color.FromArgb((int)(255 * Math.Max(0, fade)), c))) g.FillRectangle(b, r);
        if (fade >= 1) {
            float size = openFrom.Width + (Math.Min(w, h) * 0.32f - openFrom.Width) * e;
            AppArt(g, openApp, new RectangleF(r.X + (r.Width - size) / 2, r.Y + (r.Height - size) / 2, size, size), c);
        }
    }
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
    // A glossy key like the shell's own: its colour top to bottom, a soft shine on the upper half and a darker rim.
    static void GlossTile(Graphics g, RectangleF r, float rad, Color c) {
        Card(g, r, rad, Mix(c, Color.White, 0.18f), Mix(c, Color.Black, 0.14f), Color.Empty, 0);
        using (var path = Round(r, rad)) {
            var st = g.Save();
            g.SetClip(path, System.Drawing.Drawing2D.CombineMode.Intersect);
            var shine = new RectangleF(r.X - r.Width * 0.2f, r.Y - r.Height * 0.62f, r.Width * 1.4f, r.Height * 1.08f);
            using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(shine, Color.FromArgb(80, 255, 255, 255), Color.FromArgb(6, 255, 255, 255), 90f)) g.FillEllipse(b, shine);
            g.Restore(st);
            using (var pen = new Pen(Mix(c, Color.Black, 0.35f), Math.Max(1f, r.Width * 0.018f))) g.DrawPath(pen, path);
        }
    }
    static readonly Color Ink = Color.FromArgb(40, 36, 52);
    // The home screen's app icons, drawn in the games' menu style: chunky dark outlines over two-tone fills. Original shapes.
    // A highlighted app's ball shakes as in a catch: one shake, then a pause of 0.3 to 1 s, on its own random rhythm.
    static readonly List<float> shakeStarts = new List<float>(); static Random shakeRandom; static float shakeU;
    const float ShakeLength = 0.62f;
    static float BallShake(float u) {
        if (u < 0) { shakeRandom = null; return 0; }
        if (shakeRandom == null || u < shakeU) { shakeRandom = new Random(); shakeStarts.Clear(); shakeStarts.Add(0.15f); }
        shakeU = u;
        while (shakeStarts[shakeStarts.Count - 1] + ShakeLength < u) shakeStarts.Add(shakeStarts[shakeStarts.Count - 1] + ShakeLength + 0.3f + (float)shakeRandom.NextDouble() * 0.7f);
        float k = (u - shakeStarts[shakeStarts.Count - 1]) / ShakeLength;
        if (k <= 0 || k >= 1) return 0;
        return 24f * (float)(Math.Sin(k * Math.PI * 2) * Math.Sin(k * Math.PI));
    }
    // u: seconds since the app was highlighted, so its little motion plays; below 0 it's the still icon.
    static void AppArt(Graphics g, int app, RectangleF r, Color tile, float u = -1) {
        float s = r.Width, lw = Math.Max(1.5f, s * 0.045f);
        Func<float, float, PointF> P = (x, y) => new PointF(r.X + x * s, r.Y + y * s);
        Func<float, float, float, float, RectangleF> R = (x, y, w2, h2) => new RectangleF(r.X + x * s, r.Y + y * s, w2 * s, h2 * s);
        Action<PointF, float> Turn = (c, deg) => { g.TranslateTransform(c.X, c.Y); g.RotateTransform(deg); g.TranslateTransform(-c.X, -c.Y); };
        float wave = u < 0 ? 0 : (float)Math.Sin(u * Math.PI * 2 / 1.4);
        var st = g.Save();
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        using (var ink = new Pen(Ink, lw) { LineJoin = System.Drawing.Drawing2D.LineJoin.Round, StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) {
            switch (AppNames[app]) {
                case "Games": {
                    // Rocks on its base, the button glowing while it shakes.
                    float a = u < 0 ? 0 : BallShake(u);
                    if (a != 0) Turn(P(0.5f, 0.83f), a);
                    Ball(g, P(0.5f, 0.5f), s * 0.33f, ink);
                    if (Math.Abs(a) > 0.5f) using (var b = new SolidBrush(Color.FromArgb((int)Math.Min(200, Math.Abs(a) * 10), 255, 70, 60))) g.FillEllipse(b, R(0.5f - 0.075f, 0.5f - 0.075f, 0.15f, 0.15f));
                    break;
                }
                case "Collection": {
                    var body = R(0.22f, 0.15f, 0.56f, 0.7f);
                    using (var path = Round(body, s * 0.07f)) {
                        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(body, Color.FromArgb(240, 70, 64), Color.FromArgb(186, 30, 36), 90f)) g.FillPath(b, path);
                        using (var b = new SolidBrush(Color.FromArgb(150, 20, 26))) g.FillRectangle(b, R(0.22f, 0.15f, 0.07f, 0.7f));
                        g.DrawPath(ink, path);
                    }
                    g.DrawLine(ink, P(0.29f, 0.17f), P(0.29f, 0.83f));
                    var lens = R(0.35f, 0.21f, 0.2f, 0.2f);
                    using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(lens, Color.FromArgb(120, 200, 255), Color.FromArgb(30, 100, 210), 60f)) g.FillEllipse(b, lens);
                    g.FillEllipse(Brushes.White, R(0.39f, 0.25f, 0.06f, 0.06f));
                    if (u >= 0) using (var b = new SolidBrush(Color.FromArgb((int)(70 + 70 * wave), 255, 255, 255))) g.FillEllipse(b, lens);
                    g.DrawEllipse(ink, lens);
                    var lights = new[] { Color.FromArgb(255, 90, 80), Color.FromArgb(255, 214, 64), Color.FromArgb(90, 210, 110) };
                    int lit = u < 0 ? -1 : (int)(u * 3.5f) % 3;
                    for (int k = 0; k < 3; k++) { var dot = R(0.59f + k * 0.055f, 0.24f, 0.04f, 0.04f); using (var b = new SolidBrush(u < 0 || k == lit ? lights[k] : Mix(lights[k], Color.Black, 0.45f))) g.FillEllipse(b, dot); }
                    var screen = R(0.34f, 0.5f, 0.36f, 0.25f);
                    using (var path = Round(screen, s * 0.03f)) { using (var b = new SolidBrush(Color.FromArgb(206, 240, 228))) g.FillPath(b, path); g.DrawPath(ink, path); }
                    // Its entry being written out, line by line.
                    float k1 = 1, k2 = 1;
                    if (u >= 0) { float c = u % 2.2f / 1.6f; k1 = Math.Min(1, c * 2); k2 = Math.Max(0, Math.Min(1, c * 2 - 1)); }
                    using (var pen = new Pen(Color.FromArgb(120, 170, 150), lw * 0.6f)) { if (k1 > 0.02f) g.DrawLine(pen, P(0.39f, 0.58f), P(0.39f + 0.21f * k1, 0.58f)); if (k2 > 0.02f) g.DrawLine(pen, P(0.39f, 0.66f), P(0.39f + 0.15f * k2, 0.66f)); }
                    break;
                }
                case "Mods": {
                    var disc = R(0.15f, 0.15f, 0.7f, 0.7f);
                    using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(disc, Color.FromArgb(250, 251, 255), Color.FromArgb(160, 168, 186), 45f)) g.FillEllipse(b, disc);
                    using (var pen = new Pen(Color.FromArgb(150, 255, 255, 255), lw * 1.2f)) g.DrawArc(pen, R(0.22f, 0.22f, 0.56f, 0.56f), 200, 60);
                    var label = R(0.31f, 0.31f, 0.38f, 0.38f);
                    // Spinning: the label turns under the light, which stays put.
                    var spin = g.Save();
                    if (u >= 0) Turn(P(0.5f, 0.5f), u * 300f);
                    using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(label, Mix(tile, Color.White, 0.35f), Mix(tile, Color.Black, 0.15f), 90f)) g.FillEllipse(b, label);
                    if (u >= 0) using (var pen = new Pen(Color.FromArgb(170, 255, 255, 255), lw * 0.9f) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) g.DrawArc(pen, R(0.36f, 0.36f, 0.28f, 0.28f), 300, 70);
                    g.Restore(spin);
                    g.DrawEllipse(ink, label);
                    var hole = R(0.44f, 0.44f, 0.12f, 0.12f);
                    using (var b = new SolidBrush(Mix(tile, Color.Black, 0.45f))) g.FillEllipse(b, hole);
                    g.DrawEllipse(ink, hole);
                    g.DrawEllipse(ink, disc);
                    Sparkle(g, P(0.8f, 0.2f), s * 0.09f * (u < 0 ? 1 : 0.75f + 0.4f * Math.Abs((float)Math.Sin(u * Math.PI / 0.9))));
                    break;
                }
                case "ROMs": {
                    if (u >= 0) g.TranslateTransform(0, -s * 0.045f * (0.5f - 0.5f * (float)Math.Cos(u * Math.PI * 2 / 1.1)));
                    var body = new System.Drawing.Drawing2D.GraphicsPath();
                    body.AddLines(new[] { P(0.24f, 0.2f), P(0.6f, 0.2f), P(0.68f, 0.28f), P(0.76f, 0.28f), P(0.76f, 0.82f), P(0.24f, 0.82f) });
                    body.CloseFigure();
                    using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(R(0.24f, 0.2f, 0.52f, 0.62f), Color.FromArgb(232, 234, 240), Color.FromArgb(158, 164, 178), 90f)) g.FillPath(b, body);
                    var label = R(0.31f, 0.33f, 0.38f, 0.3f);
                    using (var path = Round(label, s * 0.025f)) {
                        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(label, Mix(tile, Color.White, 0.3f), Mix(tile, Color.Black, 0.12f), 90f)) g.FillPath(b, path);
                        g.DrawPath(ink, path);
                    }
                    using (var thin = new Pen(Ink, lw * 0.7f)) Ball(g, P(0.5f, 0.48f), s * 0.08f, thin);
                    for (int k = 0; k < 5; k++) g.DrawLine(ink, P(0.33f + k * 0.085f, 0.72f), P(0.33f + k * 0.085f, 0.78f));
                    g.DrawPath(ink, body);
                    body.Dispose();
                    break;
                }
                case "Move": {
                    Func<float, float> Hop = ph => u < 0 ? 0 : -0.07f * Math.Max(0, (float)Math.Sin((u / 1.2f + ph) * Math.PI * 2));
                    Ball(g, P(0.3f, 0.62f + Hop(0)), s * 0.16f, ink);
                    Ball(g, P(0.7f, 0.62f + Hop(0.5f)), s * 0.16f, ink);
                    var arc = new System.Drawing.Drawing2D.GraphicsPath();
                    arc.AddBezier(P(0.3f, 0.38f), P(0.36f, 0.12f), P(0.64f, 0.12f), P(0.7f, 0.36f));
                    var head = new[] { P(0.61f, 0.3f), P(0.705f, 0.4f), P(0.77f, 0.28f) };
                    using (var back = new Pen(Ink, lw * 3f) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round, LineJoin = System.Drawing.Drawing2D.LineJoin.Round }) { g.DrawPath(back, arc); g.DrawLines(back, head); }
                    using (var front = new Pen(Color.White, lw * 1.4f) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round, LineJoin = System.Drawing.Drawing2D.LineJoin.Round }) { g.DrawPath(front, arc); g.DrawLines(front, head); }
                    arc.Dispose();
                    break;
                }
                case "Settings": {
                    var gear = new System.Drawing.Drawing2D.GraphicsPath();
                    var pts = new List<PointF>(); var c0 = P(0.5f, 0.5f);
                    for (int k = 0; k < 8; k++) {
                        double a = k * Math.PI / 4;
                        foreach (var off in new[] { -0.24, -0.13, 0.13, 0.24 }) {
                            double ang = a + off * 0.95; float rr = Math.Abs(off) < 0.2 ? 0.35f : 0.27f;
                            pts.Add(new PointF(c0.X + (float)Math.Cos(ang) * rr * s, c0.Y + (float)Math.Sin(ang) * rr * s));
                        }
                    }
                    gear.AddPolygon(pts.ToArray());
                    if (u >= 0) Turn(c0, u * 45f);
                    using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(R(0.15f, 0.15f, 0.7f, 0.7f), Color.FromArgb(252, 252, 255), Color.FromArgb(184, 190, 204), 90f)) g.FillPath(b, gear);
                    g.DrawPath(ink, gear);
                    var hole = R(0.39f, 0.39f, 0.22f, 0.22f);
                    using (var b = new SolidBrush(Mix(tile, Color.Black, 0.3f))) g.FillEllipse(b, hole);
                    g.DrawEllipse(ink, hole);
                    gear.Dispose();
                    break;
                }
                case "Exit": {
                    var ring = R(0.24f, 0.26f, 0.52f, 0.52f);
                    using (var back = new Pen(Ink, lw * 3.2f) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) { g.DrawArc(back, ring, -60, 300); g.DrawLine(back, P(0.5f, 0.17f), P(0.5f, 0.47f)); }
                    using (var front = new Pen(u < 0 ? Color.FromArgb(255, 118, 110) : Mix(Color.FromArgb(255, 118, 110), Color.White, 0.25f + 0.25f * wave), lw * 1.5f) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) { g.DrawArc(front, ring, -60, 300); g.DrawLine(front, P(0.5f, 0.17f), P(0.5f, 0.47f)); }
                    break;
                }
            }
        }
        g.Restore(st);
    }
    // The list rows' common icons in the same style: white stickers with a dark outline. False: draw the glyph instead.
    static bool RowArt(Graphics g, char icon, RectangleF r, Color tint) {
        switch (icon) {
            case '': AppArt(g, 6, r, tint); return true;
            case '': AppArt(g, 2, r, tint); return true;
            case '': AppArt(g, 3, r, tint); return true;
        }
        float s = r.Width;
        Func<float, float, PointF> P = (x, y) => new PointF(r.X + x * s, r.Y + y * s);
        var lines = new List<PointF[]>(); System.Drawing.Drawing2D.GraphicsPath fill = null; Color fillColor = Color.White; Action extra = null;
        switch (icon) {
            case '': fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.38f, 0.26f), P(0.76f, 0.5f), P(0.38f, 0.74f) }); break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddLines(new[] { P(0.22f, 0.26f), P(0.78f, 0.26f), P(0.78f, 0.62f), P(0.46f, 0.62f), P(0.32f, 0.76f), P(0.34f, 0.62f), P(0.22f, 0.62f) }); fill.CloseFigure();
                extra = delegate { using (var pen = new Pen(Ink, Math.Max(1f, s * 0.05f)) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) { g.DrawLine(pen, P(0.32f, 0.38f), P(0.68f, 0.38f)); g.DrawLine(pen, P(0.32f, 0.5f), P(0.56f, 0.5f)); } }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddRectangle(new RectangleF(r.X + s * 0.18f, r.Y + s * 0.32f, s * 0.64f, s * 0.38f));
                extra = delegate { using (var b = new SolidBrush(Ink)) { for (int row = 0; row < 2; row++) for (int k = 0; k < 4; k++) g.FillRectangle(b, r.X + s * (0.26f + k * 0.13f), r.Y + s * (0.39f + row * 0.1f), s * 0.07f, s * 0.06f); g.FillRectangle(b, r.X + s * 0.32f, r.Y + s * 0.6f, s * 0.36f, s * 0.05f); } }; break;
            case '': case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddLines(new[] { P(0.2f, 0.36f), P(0.36f, 0.36f), P(0.41f, 0.28f), P(0.59f, 0.28f), P(0.64f, 0.36f), P(0.8f, 0.36f), P(0.8f, 0.74f), P(0.2f, 0.74f) }); fill.CloseFigure();
                extra = delegate { var lens = new RectangleF(r.X + s * 0.37f, r.Y + s * 0.42f, s * 0.26f, s * 0.26f); using (var b = new SolidBrush(Color.FromArgb(70, 140, 230))) g.FillEllipse(b, lens); using (var pen = new Pen(Ink, Math.Max(1.2f, s * 0.06f))) g.DrawEllipse(pen, lens); g.FillEllipse(Brushes.White, lens.X + lens.Width * 0.2f, lens.Y + lens.Height * 0.2f, lens.Width * 0.28f, lens.Height * 0.28f); }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.56f, 0.18f), P(0.28f, 0.56f), P(0.47f, 0.56f), P(0.4f, 0.82f), P(0.72f, 0.42f), P(0.53f, 0.42f) });
                fillColor = Color.FromArgb(255, 214, 52); break;
            case '': lines.Add(new[] { P(0.33f, 0.33f), P(0.67f, 0.67f) }); lines.Add(new[] { P(0.67f, 0.33f), P(0.33f, 0.67f) }); break;
            case '': lines.Add(new[] { P(0.27f, 0.52f), P(0.43f, 0.68f), P(0.73f, 0.34f) }); break;
            case '': lines.Add(new[] { P(0.5f, 0.27f), P(0.5f, 0.73f) }); lines.Add(new[] { P(0.27f, 0.5f), P(0.73f, 0.5f) }); break;
            case '': case '':
                lines.Add(new[] { P(0.5f, 0.24f), P(0.5f, 0.56f) }); lines.Add(new[] { P(0.37f, 0.45f), P(0.5f, 0.58f), P(0.63f, 0.45f) });
                lines.Add(new[] { P(0.27f, 0.62f), P(0.27f, 0.74f), P(0.73f, 0.74f), P(0.73f, 0.62f) }); break;
            case '':
                lines.Add(new[] { P(0.5f, 0.58f), P(0.5f, 0.26f) }); lines.Add(new[] { P(0.37f, 0.37f), P(0.5f, 0.24f), P(0.63f, 0.37f) });
                lines.Add(new[] { P(0.27f, 0.62f), P(0.27f, 0.74f), P(0.73f, 0.74f), P(0.73f, 0.62f) }); break;
            case '':
                lines.Add(new[] { P(0.27f, 0.39f), P(0.71f, 0.39f) }); lines.Add(new[] { P(0.6f, 0.29f), P(0.72f, 0.39f), P(0.6f, 0.49f) });
                lines.Add(new[] { P(0.73f, 0.61f), P(0.29f, 0.61f) }); lines.Add(new[] { P(0.4f, 0.51f), P(0.28f, 0.61f), P(0.4f, 0.71f) }); break;
            case '': {
                var arc = new List<PointF>();
                for (int k = 0; k <= 10; k++) { double a = (-20 + k * 25) * Math.PI / 180; arc.Add(P(0.5f + 0.21f * (float)Math.Cos(a), 0.5f + 0.21f * (float)Math.Sin(a))); }
                lines.Add(arc.ToArray());
                // A solid head at the arc's end, pointing along it.
                double ea = (-20 + 250) * Math.PI / 180, tx = -Math.Sin(ea), ty = Math.Cos(ea);
                var tip = P(0.5f + 0.21f * (float)Math.Cos(ea) + 0.13f * (float)tx, 0.5f + 0.21f * (float)Math.Sin(ea) + 0.13f * (float)ty);
                var bl = P(0.5f + 0.33f * (float)Math.Cos(ea), 0.5f + 0.33f * (float)Math.Sin(ea));
                var br = P(0.5f + 0.09f * (float)Math.Cos(ea), 0.5f + 0.09f * (float)Math.Sin(ea));
                extra = delegate { var head = new[] { tip, bl, br }; g.FillPolygon(Brushes.White, head); using (var pen = new Pen(Ink, Math.Max(1.2f, s * 0.06f)) { LineJoin = System.Drawing.Drawing2D.LineJoin.Round }) g.DrawPolygon(pen, head); };
                break;
            }
            case '':
                lines.Add(new[] { P(0.5f, 0.46f), P(0.5f, 0.72f) });
                extra = delegate { float d = s * 0.13f; var c = P(0.5f, 0.3f); using (var b = new SolidBrush(Ink)) g.FillEllipse(b, c.X - d * 0.8f, c.Y - d * 0.8f, d * 1.6f, d * 1.6f); g.FillEllipse(Brushes.White, c.X - d / 2, c.Y - d / 2, d, d); }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.62f, 0.22f), P(0.78f, 0.38f), P(0.42f, 0.74f), P(0.24f, 0.78f), P(0.28f, 0.6f) });
                fillColor = Color.FromArgb(255, 206, 72);
                lines.Add(new[] { P(0.54f, 0.3f), P(0.7f, 0.46f) }); break;
            case '': case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.22f, 0.3f), P(0.42f, 0.3f), P(0.48f, 0.37f), P(0.78f, 0.37f), P(0.78f, 0.73f), P(0.22f, 0.73f) });
                fillColor = Color.FromArgb(255, 210, 92); break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.5f, 0.2f), P(0.8f, 0.76f), P(0.2f, 0.76f) });
                fillColor = Color.FromArgb(255, 214, 60);
                extra = delegate { using (var pen = new Pen(Ink, Math.Max(1.5f, s * 0.08f)) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round }) g.DrawLine(pen, P(0.5f, 0.4f), P(0.5f, 0.57f)); var c = P(0.5f, 0.67f); float d = s * 0.08f; using (var b = new SolidBrush(Ink)) g.FillEllipse(b, c.X - d / 2, c.Y - d / 2, d, d); }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddPolygon(new[] { P(0.3f, 0.34f), P(0.7f, 0.34f), P(0.65f, 0.76f), P(0.35f, 0.76f) });
                lines.Add(new[] { P(0.24f, 0.31f), P(0.76f, 0.31f) }); lines.Add(new[] { P(0.42f, 0.24f), P(0.58f, 0.24f) });
                extra = delegate { using (var pen = new Pen(Ink, Math.Max(1f, s * 0.045f))) { g.DrawLine(pen, P(0.45f, 0.43f), P(0.45f, 0.67f)); g.DrawLine(pen, P(0.55f, 0.43f), P(0.55f, 0.67f)); } }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath();
                fill.AddArc(r.X + s * 0.16f, r.Y + s * 0.34f, s * 0.3f, s * 0.36f, 90, 180); fill.AddArc(r.X + s * 0.54f, r.Y + s * 0.34f, s * 0.3f, s * 0.36f, 270, 180); fill.CloseFigure();
                extra = delegate {
                    using (var pen = new Pen(Ink, Math.Max(1.2f, s * 0.06f))) { g.DrawLine(pen, P(0.25f, 0.52f), P(0.39f, 0.52f)); g.DrawLine(pen, P(0.32f, 0.45f), P(0.32f, 0.59f)); }
                    float d = s * 0.08f;
                    using (var b = new SolidBrush(Color.FromArgb(236, 64, 60))) g.FillEllipse(b, r.X + s * 0.64f - d / 2, r.Y + s * 0.47f - d / 2, d, d);
                    using (var b = new SolidBrush(Color.FromArgb(52, 120, 232))) g.FillEllipse(b, r.X + s * 0.72f - d / 2, r.Y + s * 0.56f - d / 2, d, d);
                }; break;
            case '':
                fill = new System.Drawing.Drawing2D.GraphicsPath(); fill.AddRectangle(new RectangleF(r.X + s * 0.28f, r.Y + s * 0.22f, s * 0.44f, s * 0.56f));
                lines.Add(new[] { P(0.36f, 0.22f), P(0.36f, 0.78f) });
                extra = delegate { using (var b = new SolidBrush(Color.FromArgb(236, 64, 60))) g.FillPolygon(b, new[] { P(0.55f, 0.22f), P(0.65f, 0.22f), P(0.65f, 0.46f), P(0.6f, 0.41f), P(0.55f, 0.46f) }); }; break;
            default: return false;
        }
        var st = g.Save();
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        float back = Math.Max(3f, s * 0.17f), front = Math.Max(1.5f, s * 0.09f);
        Func<float, Color, Pen> pen0 = (wd, c) => new Pen(c, wd) { StartCap = System.Drawing.Drawing2D.LineCap.Round, EndCap = System.Drawing.Drawing2D.LineCap.Round, LineJoin = System.Drawing.Drawing2D.LineJoin.Round };
        if (fill != null) {
            using (var b = new SolidBrush(fillColor)) g.FillPath(b, fill);
            using (var pen = pen0(Math.Max(1.2f, s * 0.06f), Ink)) g.DrawPath(pen, fill);
            fill.Dispose();
        }
        if (lines.Count > 0) {
            using (var pen = pen0(back, Ink)) foreach (var l in lines) g.DrawLines(pen, l);
            using (var pen = pen0(front, Color.White)) foreach (var l in lines) g.DrawLines(pen, l);
        }
        if (extra != null) extra();
        g.Restore(st);
        return true;
    }
    // A Poke Ball: red over white, the dark band and its button, with a glint.
    static void Ball(Graphics g, PointF c, float rad, Pen ink) {
        var box = new RectangleF(c.X - rad, c.Y - rad, rad * 2, rad * 2);
        g.FillEllipse(Brushes.White, box);
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(box, Color.FromArgb(250, 84, 72), Color.FromArgb(196, 28, 30), 90f)) g.FillPie(b, box.X, box.Y, box.Width, box.Height, 180, 180);
        using (var b = new SolidBrush(Color.FromArgb(70, 150, 158, 178))) g.FillPie(b, box.X + rad * 0.12f, box.Y, box.Width - rad * 0.24f, box.Height, 25, 130);
        using (var band = new Pen(Ink, Math.Max(1.2f, rad * 0.2f))) g.DrawLine(band, c.X - rad, c.Y, c.X + rad, c.Y);
        g.DrawEllipse(ink, box);
        float br = rad * 0.32f;
        var btn = new RectangleF(c.X - br, c.Y - br, br * 2, br * 2);
        g.FillEllipse(Brushes.White, btn);
        g.DrawEllipse(ink, btn);
        using (var b = new SolidBrush(Color.FromArgb(150, 255, 255, 255))) g.FillEllipse(b, c.X - rad * 0.62f, c.Y - rad * 0.72f, rad * 0.4f, rad * 0.26f);
    }
    static void Sparkle(Graphics g, PointF c, float r) {
        var pts = new[] { new PointF(c.X, c.Y - r), new PointF(c.X + r * 0.25f, c.Y - r * 0.25f), new PointF(c.X + r, c.Y), new PointF(c.X + r * 0.25f, c.Y + r * 0.25f), new PointF(c.X, c.Y + r), new PointF(c.X - r * 0.25f, c.Y + r * 0.25f), new PointF(c.X - r, c.Y), new PointF(c.X - r * 0.25f, c.Y - r * 0.25f) };
        g.FillPolygon(Brushes.White, pts);
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

    // A surface in a version's colours with a small original scene, cached. Only the moving parts redraw.
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
    // The lights as power is cut: 1 on, a surge above 1, a sputter, then a bulb's glow dying away to 0.
    static float PowerLevel = 1, FrozenT = -1;
    static readonly float[] Sputter = { 0.15f, 0.85f, 0.05f, 0.6f, 0.2f, 0.75f, 0.08f, 0.45f, 0.3f };
    static float PowerCurve(float x) {
        if (x < 0) return 1;
        if (x < 0.04f) return 1.25f;
        if (x < 0.04f + Sputter.Length * 0.022f) return Sputter[(int)((x - 0.04f) / 0.022f)];
        return 0.4f * (float)Math.Exp(-(x - 0.04f - Sputter.Length * 0.022f) / 0.08f);
    }
    // A light at that power: brighter on the surge, warm and dim as it dies, then dark.
    static Color Powered(Color c, Color off) {
        float l = PowerLevel;
        if (l >= 1) return Mix(c, Color.White, Math.Min(1, (l - 1) * 2));
        return Mix(off, Mix(c, Color.FromArgb(255, 120, 40), (1 - l) * 0.5f), l);
    }
    static void Lens(Graphics g, float cx, float cy, float r, float t, bool lights) {
        using (var b = new SolidBrush(Line)) g.FillEllipse(b, cx - r * 1.14f, cy - r * 1.14f, r * 2.28f, r * 2.28f);
        using (var b = new SolidBrush(Color.FromArgb(240, 242, 246))) g.FillEllipse(b, cx - r, cy - r, r * 2, r * 2);
        float i = r * 0.76f;
        var rect = new RectangleF(cx - i, cy - i, i * 2, i * 2);
        if (FrozenT >= 0) t = FrozenT;
        float pulse = 0.5f + 0.5f * (float)Math.Sin(t * 1.8f);
        using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(rect, Powered(Mix(Color.FromArgb(70, 170, 245), Color.FromArgb(130, 215, 255), pulse), Color.FromArgb(14, 30, 60)), Powered(Color.FromArgb(16, 74, 160), Color.FromArgb(6, 14, 30)), 60f)) g.FillEllipse(b, rect);
        using (var b = new SolidBrush(Fade(Color.White, 200))) g.FillEllipse(b, cx - i * 0.58f, cy - i * 0.62f, i * 0.55f, i * 0.42f);
        if (!lights) return;
        float d = r * 0.42f, x = cx + r * 1.5f, y = cy - r * 0.86f;
        Color[] on = { Color.FromArgb(240, 70, 60), Color.FromArgb(250, 205, 50), Color.FromArgb(90, 205, 90) };
        for (int k = 0; k < 3; k++) {
            int lit = ((int)(t * 1.5f)) % 3;
            using (var b = new SolidBrush(Line)) g.FillEllipse(b, x - 1.5f, y - 1.5f, d + 3, d + 3);
            var off = Mix(on[k], Color.Black, 0.7f);
            using (var b = new SolidBrush(k == lit ? Powered(on[k], off) : Mix(off, Mix(on[k], Line, 0.45f), Math.Min(1, PowerLevel)))) g.FillEllipse(b, x, y, d, d);
            x += d * 1.45f;
        }
    }
    // The working screen's spinner, after pogo.hails.app's "Updating" page (notes/launcher.md, Pictures).
    static readonly Color[] Rainbow = { Color.FromArgb(255, 0, 0), Color.FromArgb(255, 136, 0), Color.FromArgb(255, 255, 0), Color.FromArgb(0, 204, 0), Color.FromArgb(0, 102, 255), Color.FromArgb(136, 0, 255) };
    static Color RainbowAt(float t) {
        float k = (t / 2f % 1f) * Rainbow.Length;
        int i = (int)k % Rainbow.Length;
        return Mix(Rainbow[i], Rainbow[(i + 1) % Rainbow.Length], k - (int)k);
    }
    static void SpinningBall(Graphics g, float cx, float cy, float size, float t) { SpinningBall(g, cx, cy, size, t, false); }
    // dark: the page's own colours for a dark background (translucent white edges, a dark button); otherwise dark edges.
    static void SpinningBall(Graphics g, float cx, float cy, float size, float t, bool dark) {
        var st = g.Save();
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.TranslateTransform(cx, cy); g.RotateTransform(t * 180f % 360f); g.ScaleTransform(size / 64f, size / 64f);
        var whole = new RectangleF(-30, -30, 60, 60);
        using (var top = new SolidBrush(Fade(RainbowAt(t), dark ? 217 : 230))) g.FillPie(top, whole.X, whole.Y, whole.Width, whole.Height, 180, 180);
        using (var bottom = new SolidBrush(dark ? Color.FromArgb(209, 255, 255, 255) : Color.White)) g.FillPie(bottom, whole.X, whole.Y, whole.Width, whole.Height, 0, 180);
        using (var band = new Pen(dark ? Color.FromArgb(51, 255, 255, 255) : Fade(Line, 200), 3f)) g.DrawLine(band, -30, 0, 30, 0);
        using (var edge = new Pen(dark ? Color.FromArgb(38, 255, 255, 255) : Fade(Line, 220), 2f)) g.DrawEllipse(edge, whole);
        using (var button = new SolidBrush(dark ? Color.FromArgb(153, 0, 0, 0) : Color.White)) using (var ring = new Pen(dark ? Color.FromArgb(51, 255, 255, 255) : Fade(Line, 200), 2f)) { g.FillEllipse(button, -8, -8, 16, 16); g.DrawEllipse(ring, -8, -8, 16, 16); }
        if (dark) using (var dot = new SolidBrush(Color.FromArgb(38, 255, 255, 255))) g.FillEllipse(dot, -4, -4, 8, 8);
        else using (var dot = new Pen(Fade(Line, 70), 1.5f)) g.DrawEllipse(dot, -4, -4, 8, 8);
        g.Restore(st);
    }
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
    RectangleF ScreenRect(int w, int h) {
        // Unfolding, the screen grows down into the space the D-pad leaves.
        float pm = PhoneW(h) * 0.06f, e = TabletEase;
        return new RectangleF(pm, h * 0.115f, w - 2 * pm, h * (0.665f + 0.18f * e));
    }
    // Tablet mode for Move Pokemon (notes/launcher.md, Window).
    float tabletK, tabletLast; int phoneW, tabletW, morphCentre;
    bool tablet { get { return tabletK > 0; } }
    float TabletEase { get { float k = tabletK; return k * k * (3 - 2 * k); } }
    int PhoneW(int h) { return phoneW > 0 ? phoneW : h * 60 / 100; }
    // The window resizes once per fold. In between, only the drawn device changes, so it never judders.
    int drawX, drawW;
    bool Morphing { get { return tabletK > 0 && tabletK < 1; } }
    void MorphTowards(bool on) {
        float target = on ? 1 : 0, now = (float)clock.Elapsed.TotalSeconds;
        if (tabletK == target) { tabletLast = now; return; }
        if (phoneW == 0) phoneW = ClientSize.Width;
        var area = Screen.FromControl(this).WorkingArea;
        int h = ClientSize.Height;
        if (tabletK == 0 || tabletK == 1) {
            // Starting out: where it opens from, and how wide it gets on this screen.
            morphCentre = Left + Width / 2;
            if (tabletK == 0) {
                tabletW = Math.Max(phoneW, Math.Min(area.Width * 94 / 100, h * 3 / 2));
                int left = Math.Max(area.Left, Math.Min(area.Right - tabletW, morphCentre - tabletW / 2));
                drawW = phoneW; drawX = Left - left;
                // One change of the window, before anything moves: its final place and size, showing only the phone.
                SetBounds(left, Top, tabletW, h);
                SetMorphRegion(drawX, drawW, h);
                Refresh();
            }
            tabletLast = now - 0.016f;   // the first step counts one tick, so it gets going
        }
        float step = Math.Min(0.05f, Math.Max(0f, now - tabletLast)) / 0.35f;
        tabletLast = now;
        tabletK = on ? Math.Min(1f, tabletK + step) : Math.Max(0f, tabletK - step);
        int w = (int)Math.Round(phoneW + (tabletW - phoneW) * TabletEase);
        // It widens from (and narrows back to) the phone's centre, kept inside the window.
        drawW = w;
        drawX = Math.Max(0, Math.Min(ClientSize.Width - w, morphCentre - Left - w / 2));
        if (tabletK == 0) {
            // Folded: the window becomes the phone again, where the phone now is.
            int left = Left + drawX;
            drawX = 0; drawW = 0;
            SetBounds(left, Top, phoneW, h);
            SetMorphRegion(0, phoneW, h);
            Refresh();
            return;
        } else if (tabletK == 1) { drawX = 0; drawW = 0; SetMorphRegion(0, ClientSize.Width, h); }
        else SetMorphRegion(drawX, w, h);
        if (TestLog != null) try { File.AppendAllText(TestLog, "morph " + ((int)(now * 1000)) + " ms k=" + tabletK.ToString("0.000") + " w=" + w + " x=" + drawX + Environment.NewLine); } catch { }
        Invalidate();
    }
    void SetMorphRegion(int x, int w, int h) {
        using (var outline = Round(new RectangleF(x, 0, w, h), CornerRadius(phoneW))) Region = new Region(outline);
    }
    void TabletShell(Graphics g, int w, int h, Theme th, float t) {
        int pw = PhoneW(h);
        float corner = CornerRadius(pw), edge = Math.Max(3f, pw * 0.008f);
        var body = new RectangleF(0, 0, w, h);
        using (var path = Round(body, corner)) using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(body, Mix(th.Main, Color.White, 0.1f), Mix(th.Main, th.Deep, 0.75f), 78f)) g.FillPath(b, path);
        var lip = new RectangleF(edge, edge, w - 2 * edge, h - 2 * edge);
        using (var path = Round(lip, corner - edge)) using (var pen = new Pen(Fade(Color.White, 70), Math.Max(1.5f, pw * 0.003f))) g.DrawPath(pen, path);
        using (var path = Round(new RectangleF(1, 1, w - 2, h - 2), corner)) using (var pen = new Pen(Mix(th.Deep, Color.Black, 0.45f), edge)) g.DrawPath(pen, path);
        float fold = h * 0.1f;
        using (var pen = new Pen(Fade(Color.Black, 70), Math.Max(1.5f, h * 0.002f))) g.DrawLine(pen, edge, fold, w - edge, fold);
        using (var pen = new Pen(Fade(Color.White, 60), Math.Max(1f, h * 0.0012f))) g.DrawLine(pen, edge, fold + Math.Max(2f, h * 0.002f), w - edge, fold + Math.Max(2f, h * 0.002f));
        float lensR = h * 0.038f, topY = h * 0.056f;
        Lens(g, pw * 0.06f + lensR * 1.14f, topY, lensR, t, true);
        var native = g.MeasureString("NATIVE", markFont);
        float wx = pw * 0.06f + lensR * 2.9f + lensR * 2.2f, wy = topY - native.Height * 0.1f;
        Write(g, "NATIVE", markFont, th.Ink, new RectangleF(wx, wy, native.Width + 8, native.Height), StringAlignment.Near, StringAlignment.Near, false);
        Write(g, "MONS", markLight, th.Ink, new RectangleF(wx + native.Width * 0.92f, wy, pw * 0.3f, native.Height), StringAlignment.Near, StringAlignment.Near, false);
        float key = h * 0.022f, ky = h * 0.02f, kx = w - pw * 0.05f - key;
        HardKey(g, new RectangleF(kx, ky, key, key), "X", "");
        HardKey(g, new RectangleF(kx - key * 1.5f, ky, key, key), "M", "");
        // While it unfolds, the phone's controls slide down out of the case.
        float e = TabletEase;
        if (e < 1) {
            var st = g.Save();
            using (var clip = Round(lip, corner - edge)) g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
            g.TranslateTransform((w - pw) / 2f, h * 0.3f * e);
            ShellControls(g, pw, h, th);
            g.Restore(st);
        }
    }
    void Render(Graphics g, int w, int h) {
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
        hits.Clear(); promptHits.Clear();
        var th = Cur();
        float t = (float)clock.Elapsed.TotalSeconds;
        g.Clear(Line);
        if (tablet) TabletShell(g, w, h, th, t); else Shell(g, w, h, th, t);
        var s = ScreenRect(w, h);
        screenX = s.X; screenY = s.Y;
        // Sized by the phone's width, so the tablet's glass sits as the phone's does.
        int pw = tablet ? PhoneW(h) : w;
        float rad = pw * 0.03f;
        // A black glass front, set into a recess in the case.
        float inset = pw * 0.016f, rim = inset * 0.45f;
        var front = new RectangleF(s.X - inset, s.Y - inset, s.Width + 2 * inset, s.Height + 2 * inset);
        var recess = new RectangleF(front.X - rim, front.Y - rim, front.Width + 2 * rim, front.Height + 2 * rim);
        using (var path = Round(recess, rad * 1.6f)) using (var b = new System.Drawing.Drawing2D.LinearGradientBrush(recess, Mix(th.Deep, Color.Black, 0.55f), Mix(th.Main, Color.White, 0.25f), 90f)) g.FillPath(b, path);
        using (var bezel = Round(front, rad * 1.35f)) using (var b = new SolidBrush(Color.FromArgb(12, 12, 14))) g.FillPath(b, bezel);
        var outer = g.Save();
        using (var clip = Round(s, rad)) g.SetClip(clip, System.Drawing.Drawing2D.CombineMode.Intersect);
        g.TranslateTransform(s.X, s.Y);
        int sw = (int)s.Width, sh = (int)s.Height;
        float crt = powerOff < 0 ? -1 : (t - powerOff) / CrtTime;
        if (crt >= 0) {
            // An old tube switching off: the picture squeezes to a bright line, the line to a dot, and the dot blips out.
            using (var b = new SolidBrush(Color.FromArgb(8, 8, 10))) g.FillRectangle(b, 0, 0, sw, sh);
            float squash = Ease(Math.Min(1, crt / 0.45f)), narrow = Ease(Math.Max(0, Math.Min(1, (crt - 0.45f) / 0.3f)));
            float lineH = Math.Max(2f, sh * (1 - squash)), lineW = Math.Max(3f, sw * (1 - narrow));
            if (crt < 0.75f) {
                var kept = g.Save();
                g.TranslateTransform(sw / 2f, sh / 2f); g.ScaleTransform(lineW / sw, lineH / sh); g.TranslateTransform(-sw / 2f, -sh / 2f);
                if (squash < 0.97f) RenderScreen(g, sw, sh, th, t);
                g.Restore(kept);
                using (var b = new SolidBrush(Color.FromArgb((int)(255 * Math.Min(1, squash * 1.1f)), 235, 245, 255))) g.FillRectangle(b, (sw - lineW) / 2, (sh - lineH) / 2, lineW, lineH);
            }
            float dot = crt < 0.75f ? 0 : 1 - Math.Min(1, (crt - 0.75f) / 0.25f);
            if (dot > 0) {
                float rr = Math.Max(2f, sw * 0.018f * (0.4f + 0.6f * dot));
                using (var b = new SolidBrush(Color.FromArgb((int)(90 * dot), 200, 225, 255))) g.FillEllipse(b, sw / 2f - rr * 2.4f, sh / 2f - rr * 2.4f, rr * 4.8f, rr * 4.8f);
                using (var b = new SolidBrush(Color.FromArgb((int)(255 * dot), 245, 250, 255))) g.FillEllipse(b, sw / 2f - rr, sh / 2f - rr, rr * 2, rr * 2);
            }
        }
        else if (starting && job == null && pages.Count == 0) RenderSplash(g, sw, sh);
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
        ShellControls(g, w, h, th);
    }
    // The bottom: D-pad, speaker slots, B and A, across a phone-wide strip.
    void ShellControls(Graphics g, int w, int h, Theme th) {
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
    // For tests: which screen is showing, so a script knows where its key presses landed.
    static readonly string TestLog = Environment.GetEnvironmentVariable("NATIVEMONS_TEST_LOG");
    void RenderScreen(Graphics g, int w, int h, Theme th, float t) {
        bool home = job == null && pages.Count > 0 && pages.Peek().Main;
        var all = new RectangleF(0, 0, w, h);
        float m = w * 0.045f, top = h * 0.055f, bottom = h * 0.965f;
        if (home) {
            float bandTop = top + (bottom - top) * 0.31f, bandBottom = AppsTop(pages.Peek(), w, h, m, bottom);
            bool town = GameArt.DrawTown(g, th.GameId, all, new RectangleF(0, bandTop, w, Math.Max(1, bandBottom - bandTop)));
            if (!town) VersionFill(g, all, 0, th, t);
            using (var b = new SolidBrush(Color.FromArgb(town ? 30 : 80, 0, 0, 0))) g.FillRectangle(b, all);
        } else using (var b = new SolidBrush(PhoneBg)) g.FillRectangle(b, all);
        string key = job != null ? "job" : pages.Count + "|" + (pages.Count > 0 ? pages.Peek().Title : "");
        if (key != screenKey) {
            if (TestLog != null) try { File.AppendAllText(TestLog, key + Environment.NewLine); } catch { }
            int depth = job != null ? pages.Count + 1 : pages.Count;
            screenDir = depth >= screenDepth ? 1 : -1;
            screenKey = key; screenDepth = depth; screenAt = t;
            if (bounceArm) { bounceArm = false; bounceAt = t; }
            // Leaving the mods pages: what was switched there may have started or ended a clash.
            bool inMods = pages.Any(x => x.Mods);
            if (wasInMods && !inMods) ReadClashes();
            wasInMods = inMods;
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
        RenderOpen(g, w, h, t);
        StatusBar(g, w, h, home ? Color.White : PhoneText);
        float below = ClashBanner(g, w, h, t);
        UpdateBanner(g, w, h, t, below);
        float barW = w * 0.34f, barH = Math.Max(3f, h * 0.0065f);
        using (var path = Round(new RectangleF((w - barW) / 2, h - barH * 2.4f, barW, barH), barH / 2)) using (var b = new SolidBrush(home ? Color.White : PhoneText)) g.FillPath(b, path);
    }
    // Update notice: slides in, leaves after 8 s and takes no focus. Closing it hides that build for good.
    const float BannerSeconds = 8f;
    int bannerBuild; float bannerAt; string bannerClosed;
    float BannerAge(float t) {
        if (!Updater.Ready || job != null || starting) return -1;
        if (bannerClosed == null) bannerClosed = Config.Read("update-dismissed.txt", "").Trim();
        int build = Updater.Latest.build;
        if (bannerClosed == build.ToString()) return -1;
        if (bannerBuild != build) { bannerBuild = build; bannerAt = t; }
        // It waits while the mouse is on it.
        if ((hoverPrompt == "N" || hoverPrompt == "NX") && t - bannerAt > 0.3f) bannerAt = t - 0.3f;
        float age = t - bannerAt;
        return age <= BannerSeconds ? age : -1;
    }
    // below: how far down the clash banner reaches, so this one sits under it.
    void UpdateBanner(Graphics g, int w, int h, float t, float below) {
        float age = BannerAge(t);
        if (age < 0) return;
        float show = Math.Min(Ease(age / 0.28f), Ease((BannerSeconds - age) / 0.28f));
        // A pill as wide as its text.
        string text = "Build " + bannerBuild + " is ready";
        float bh = h * 0.056f, y = Math.Max(h * 0.052f, below) - (1 - show) * (bh + h * 0.06f), rad = bh / 2;
        float pad = bh * 0.16f, size = bh - 2 * pad, close = bh * 0.8f;
        float textW = g.MeasureString(text, smallBold).Width + pad;
        float bw = Math.Min(w * 0.9f, pad + size + pad * 1.4f + textW + close);
        var r = new RectangleF((w - bw) / 2, y, bw, bh);
        using (var shadow = Round(new RectangleF(r.X, r.Y + h * 0.004f, r.Width, r.Height), rad)) using (var b = new SolidBrush(Color.FromArgb((int)(55 * show), 0, 0, 0))) g.FillPath(b, shadow);
        using (var body = Round(r, rad)) using (var b = new SolidBrush(Color.FromArgb((int)(248 * show), 252, 252, 254))) g.FillPath(b, body);
        var icon = new RectangleF(r.X + pad, r.Y + pad, size, size);
        Card(g, icon, size / 2, Color.FromArgb(52, 134, 240), Color.FromArgb(30, 104, 214), Color.Empty, 0);
        Write(g, "\uE895", iconSmall, Color.White, icon, StringAlignment.Center, StringAlignment.Center, false);
        float x = icon.Right + pad * 1.4f;
        Write(g, text, smallBold, PhoneText, new RectangleF(x, r.Y, r.Right - close - x, bh), StringAlignment.Near, StringAlignment.Center, false);
        var x1 = new RectangleF(r.Right - close, r.Y + (bh - close) / 2, close, close);
        float cross = close * 0.14f, cx = x1.X + close / 2, cy = x1.Y + close / 2;
        using (var pen = new Pen(hoverPrompt == "NX" ? PhoneText : PhoneSub, Math.Max(1.5f, h * 0.0024f))) { g.DrawLine(pen, cx - cross, cy - cross, cx + cross, cy + cross); g.DrawLine(pen, cx - cross, cy + cross, cx + cross, cy - cross); }
        // The close button first: the first match wins.
        promptHits.Add(new KeyValuePair<RectangleF, string>(new RectangleF(x1.X + screenX, x1.Y + screenY, x1.Width, x1.Height), "NX"));
        promptHits.Add(new KeyValuePair<RectangleF, string>(new RectangleF(r.X + screenX, r.Y + screenY, r.Width, r.Height), "N"));
    }
    // Clashes across the Gen1Recomp games, read in the background, since each read starts the engine.
    GameEntry clashGame; int clashCount; float clashAt = float.NaN; bool clashReading, clashAgain, wasInMods;
    void ReadClashes() {
        if (!IsHandleCreated) return;
        if (clashReading) { clashAgain = true; return; }
        clashReading = true;
        var games = Library.Games.Where(x => x.OnGen1).ToList();
        Task.Run(() => {
            GameEntry first = null; int n = 0;
            foreach (var mg in games) {
                try { int c = ModClashes.Read(mg).Count(x => x.Active); if (c > 0) { n += c; if (first == null) first = mg; } } catch { }
            }
            return Tuple.Create(first, n);
        }).ContinueWith(task => BeginInvoke((Action)delegate {
            clashReading = false;
            clashGame = task.Result.Item1; clashCount = task.Result.Item2; Invalidate();
            if (clashAgain) { clashAgain = false; ReadClashes(); }
        }));
    }
    bool ClashShowing() { return clashGame != null && !starting && job == null && !pages.Any(x => x.Mods); }
    void OpenClashes() {
        var mg = clashGame;
        if (mg == null) return;
        Busy("Reading mods", null, delegate(string e) { pages.Push(ModsPage(mg)); pages.Push(ClashesPage(mg, null)); });
    }
    // Red and sticky until the clash is settled. Hidden on the Mods pages (notes/launcher.md, Mods).
    // Returns how far down it reaches, or 0.
    float ClashBanner(Graphics g, int w, int h, float t) {
        if (!ClashShowing()) { clashAt = float.NaN; return 0; }
        if (float.IsNaN(clashAt)) clashAt = t;
        float show = Ease((t - clashAt) / 0.28f);
        string text = "Mods are clashing and may cause issues";
        float bh = h * 0.056f, y = h * 0.052f - (1 - show) * (bh + h * 0.06f), rad = bh / 2;
        float pad = bh * 0.16f, size = bh - 2 * pad;
        float textW = g.MeasureString(text, smallBold).Width + pad;
        float bw = Math.Min(w * 0.9f, pad + size + pad * 1.4f + textW + pad * 1.6f);
        var r = new RectangleF((w - bw) / 2, y, bw, bh);
        bool over = hoverPrompt == "C";
        using (var shadow = Round(new RectangleF(r.X, r.Y + h * 0.004f, r.Width, r.Height), rad)) using (var b = new SolidBrush(Color.FromArgb((int)(70 * show), 90, 0, 0))) g.FillPath(b, shadow);
        using (var body = Round(r, rad)) using (var b = new SolidBrush(Color.FromArgb((int)(250 * show), over ? 236 : 214, over ? 58 : 40, over ? 58 : 44))) g.FillPath(b, body);
        var icon = new RectangleF(r.X + pad, r.Y + pad, size, size);
        Card(g, icon, size / 2, Color.White, Color.FromArgb(255, 226, 226), Color.Empty, 0);
        Write(g, "", iconSmall, Color.FromArgb(200, 32, 36), icon, StringAlignment.Center, StringAlignment.Center, false);
        float x = icon.Right + pad * 1.4f;
        Write(g, text, smallBold, Color.White, new RectangleF(x, r.Y, r.Right - pad - x, bh), StringAlignment.Near, StringAlignment.Center, false);
        promptHits.Add(new KeyValuePair<RectangleF, string>(new RectangleF(r.X + screenX, r.Y + screenY, r.Width, r.Height), "C"));
        return r.Bottom + h * 0.012f;
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
    // Highlighted Play: now and then its letters greet you with a soft, floaty move, picked at random, each a little different.
    class HopMove { public float Start, Speed, Size; public int Kind, Pick; }
    readonly List<HopMove> hops = new List<HopMove>(); Random hopRandom; float hopU;
    // wave, wave back, float, nod, peek, catch, skip, dropped ball, bops, flip
    static readonly float[] HopLength = { 1.05f, 1.05f, 1.35f, 0.95f, 1.1f, 2f, 0.85f, 0.8f, 0.72f, 0.75f };
    static readonly int[] HopWeight = { 3, 2, 3, 3, 2, 2, 2, 1, 1, 1 };
    HopMove NextHop(float start, int last) {
        int next;
        do { int roll = hopRandom.Next(HopWeight.Sum()); next = 0; while (roll >= HopWeight[next]) roll -= HopWeight[next++]; } while (next == last);
        return new HopMove { Start = start, Kind = next, Pick = hopRandom.Next(4), Speed = 1.25f + (float)hopRandom.NextDouble() * 0.5f, Size = 0.7f + (float)hopRandom.NextDouble() * 0.45f };
    }
    void PlayHop(Graphics g, RectangleF r, Color c, float pillH, float u) {
        const string s = "Play";
        RectangleF[] at;
        using (var sf = new StringFormat { LineAlignment = StringAlignment.Center, FormatFlags = StringFormatFlags.NoWrap }) {
            sf.SetMeasurableCharacterRanges(Enumerable.Range(0, s.Length).Select(i => new CharacterRange(i, 1)).ToArray());
            at = g.MeasureCharacterRanges(s, itemBold, r, sf).Select(x => x.GetBounds(g)).ToArray();
        }
        if (hopRandom == null || u < hopU) { hopRandom = new Random(); hops.Clear(); hops.Add(NextHop(1.2f, -1)); }
        hopU = u;
        for (var h = hops[hops.Count - 1]; u > h.Start + HopLength[h.Kind] * h.Speed; h = hops[hops.Count - 1])
            hops.Add(NextHop(h.Start + HopLength[h.Kind] * h.Speed + 7f + (float)hopRandom.NextDouble() * 13f, h.Kind));
        var mv = hops[hops.Count - 1];
        float tau = (u - mv.Start) / mv.Speed, lead = at[0].X - r.X;
        int pick = mv.Pick;
        for (int i = 0; i < s.Length; i++) {
            float lift = 0, turn = 0, land = -1; bool pivot = false;
            switch (mv.Kind) {
                case 0: lift = 0.15f * Soft((tau - i * 0.12f) / 0.6f); break;              // a slow wave, left to right
                case 1: lift = 0.15f * Soft((tau - (3 - i) * 0.12f) / 0.6f); break;        // and right to left
                case 2: {                                                                   // floats up together, hangs, drifts down
                    float x = tau - i * 0.04f;
                    float e = x < 0.5f ? Smooth(x / 0.5f) : x < 0.7f ? 1 : 1 - Smooth((x - 0.7f) / 0.5f) - 0.12f * Soft((x - 1.05f) / 0.25f);
                    lift = 0.13f * e; turn = (i - 1.5f) * 1.6f * Math.Max(0, e); break;
                }
                case 3: lift = -0.07f * Soft((tau - i * 0.08f) / 0.5f) + 0.03f * Soft((tau - i * 0.08f - 0.42f) / 0.3f); break;  // a little nod hello
                case 4: {                                                                   // one leans out to peek at you
                    float e = Soft(tau / 1.1f);
                    if (i == pick) { lift = 0.1f * e; turn = (pick < 2 ? -11 : 11) * e; }
                    else if (Math.Abs(i - pick) == 1) lift = 0.025f * e;
                    break;
                }
                case 5: {                                                                   // a catch: three gentle rocks, then the click
                    int rock = (int)(tau / 0.6f); float q = (tau - rock * 0.6f) / 0.4f;
                    if (rock < 3 && q < 1) { turn = (rock % 2 == 0 ? -6 : 6) * Soft(q); pivot = true; }
                    land = (tau - 1.8f) / 0.16f; lift = 0.05f * Hop(land); break;
                }
                case 6: land = (tau - i * 0.1f) / 0.42f; lift = 0.13f * Hop(land); turn = 5 * Hop(land); break;  // a follower's skip
                case 7: if (i == pick) {                                                    // one bounces like a dropped ball
                        if (tau < 0.34f) { land = tau / 0.34f; lift = 0.18f * Hop(land); } else if (tau < 0.58f) { land = (tau - 0.34f) / 0.24f; lift = 0.08f * Hop(land); } else { land = (tau - 0.58f) / 0.16f; lift = 0.03f * Hop(land); }
                    } break;
                case 8: if (tau < 0.34f) { land = tau / 0.34f; lift = 0.09f * Hop(land); } else { land = (tau - 0.4f) / 0.3f; lift = 0.07f * Hop(land); } break;  // two happy bops
                case 9: if (i == pick) { land = tau / 0.7f; lift = 0.2f * Hop(land); turn = 360 * Smooth(land); } break;  // the rare flip
            }
            if (mv.Kind != 9) turn *= mv.Size;
            lift *= pillH * mv.Size;
            float squash = land > 1 && land < 1.3f ? 0.1f * (float)Math.Sin((land - 1) / 0.3f * Math.PI) : 0;
            var box = new RectangleF(at[i].X - lead, r.Y - lift, r.Right - at[i].X + lead, r.Height);
            var st = g.Save();
            if (turn != 0 || squash != 0) {
                float fx = at[i].X + at[i].Width / 2;
                var o = pivot ? new PointF((at[0].X + at[s.Length - 1].Right) / 2, at[0].Bottom) : turn != 0 ? new PointF(fx, at[i].Y + at[i].Height / 2 - lift) : new PointF(fx, at[i].Bottom - lift);
                g.TranslateTransform(o.X, o.Y); g.RotateTransform(turn); g.ScaleTransform(1 + squash, 1 - squash); g.TranslateTransform(-o.X, -o.Y);
            }
            Write(g, s.Substring(i, 1), itemBold, c, box, StringAlignment.Near, StringAlignment.Center, false);
            g.Restore(st);
        }
    }
    // A thrown ball's arc over k from 0 to 1, and a floaty one that starts and stops softly.
    static float Hop(float k) { return k <= 0 || k >= 1 ? 0 : (float)Math.Sin(k * Math.PI); }
    static float Soft(float k) { if (k <= 0 || k >= 1) return 0; float v = (float)Math.Sin(k * Math.PI); return v * v; }
    static float Smooth(float k) { k = Math.Max(0, Math.Min(1, k)); return k * k * (3 - 2 * k); }
    static readonly char[] AppIcons = {' ', '\uE7FC', '\uE8F1', '\uE74C', '\uE8B7', '\uE8AB', '\uE713', '\uE7E8' };
    static readonly string[] AppNames = { "", "Games", "Collection", "Mods", "ROMs", "Move", "Settings", "Exit" };
    static readonly Color[] AppColors = { Color.Empty, Color.FromArgb(232, 62, 58), Color.FromArgb(240, 142, 30), Color.FromArgb(124, 84, 214), Color.FromArgb(40, 124, 232), Color.FromArgb(24, 164, 150), Color.FromArgb(120, 124, 134), Color.FromArgb(58, 60, 68) };
    // At the bottom, where a thumb would reach, leaving the wallpaper's town in view above them.
    float AppsTop(Page p, int w, int h, float m, float bottom) {
        float cell = (w - 2 * m) / TileCols, icon = cell * 0.68f, labelH = smallFont.Height * 1.35f, rowGap = h * 0.018f;
        int appRows = (Math.Min(p.Items.Count, AppNames.Length) - 1 + TileCols - 1) / TileCols;
        return bottom - h * 0.012f - appRows * (icon + labelH) - (appRows - 1) * rowGap;
    }
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
            else {
                using (var b = new SolidBrush(th.Deep)) g.FillRectangle(b, widget);
                // Its title screen is being made right now.
                if (current != null && GameArt.Making == current.Id) { SpinningBall(g, widget.X + widget.Width / 2, widget.Y + widget.Height * 0.36f, widget.Height * 0.34f, t, true); spinning = true; }
            }
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
        var word = new RectangleF(tx + tri * 2.1f, pill.Y, pillW * 0.7f, pillH);
        if (p.Sel == 0 && on) PlayHop(g, word, pink, pillH, t - selAt);
        else Write(g, "Play", itemBold, pink, word, StringAlignment.Near, StringAlignment.Center, false);
        float textW = pill.X - widget.X - pad * 1.4f, subH = smallFont.Height * 1.25f;
        string name = current != null ? current.Title.Replace("Pokemon ", "") : "No games yet";
        string sub = current != null ? GameEntry.EngineShort(current.Engine()) : "Add a ROM folder to start";
        Write(g, sub, smallFont, Color.FromArgb(225, 255, 255, 255), new RectangleF(widget.X + pad, widget.Bottom - pad - subH, textW, subH), StringAlignment.Near, StringAlignment.Far, false);
        var nameArea = new SizeF(textW, titleFont.Height * 1.25f);
        var nf = FitFont(g, name, nameArea, titleFont, 0.65f, false);
        Write(g, name, nf, Color.White, new RectangleF(widget.X + pad, widget.Bottom - pad - subH - nameArea.Height, textW, nameArea.Height), StringAlignment.Near, StringAlignment.Far, false);

        // The apps.
        float cell = (w - 2 * m) / TileCols, icon = cell * 0.68f, labelH = smallFont.Height * 1.35f, rowGap = h * 0.018f;
        float gy = AppsTop(p, w, h, m, bottom);
        if (p.Sel < 1) BallShake(-1);
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
            GlossTile(g, r, size * 0.24f, c);
            if (sel) using (var ring = Round(new RectangleF(r.X - 3, r.Y - 3, r.Width + 6, r.Height + 6), size * 0.27f)) using (var pen = new Pen(Color.White, Math.Max(2.5f, w * 0.007f))) g.DrawPath(pen, ring);
            appRects[i] = r;
            if (openRun != null && i == openApp) continue;
            AppArt(g, i, r, c, sel && it.On ? t - selAt : -1);
            if (!it.On) using (var path = Round(r, size * 0.24f)) using (var b = new SolidBrush(Color.FromArgb(110, c))) g.FillPath(b, path);
            if (AppNames[i] == "Settings" && Updater.Ready) {
                float dot = size * 0.26f;
                using (var b = new SolidBrush(Color.FromArgb(240, 58, 50))) g.FillEllipse(b, r.Right - dot * 0.72f, r.Y - dot * 0.28f, dot, dot);
            }
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
        // The boxes take the whole screen under Back: their headers already say what they are.
        if (p.Boxes != null) { RenderBoxes(g, p.Boxes, w, h, m, y + h * 0.004f, bottom, t, th); return; }
        var titleArea = new SizeF(w - 2 * m, titleFont.Height * 1.3f);
        var tf = FitFont(g, p.Title, titleArea, titleFont, 0.6f, false);
        Write(g, p.Title, tf, PhoneText, new RectangleF(m, y, titleArea.Width, titleArea.Height), StringAlignment.Near, StringAlignment.Center, false);
        y += titleArea.Height + h * 0.012f;
        if (p.DexId > 0) { RenderDexEntry(g, p, w, h, m, y, bottom, t, th); return; }
        float rad = w * 0.04f;
        if (p.Body != null) {
            float bodyH = p.Items.Count > 0 ? (bottom - y) * 0.6f : bottom - y;
            // A page that asks for it gets a box only as tall as its text (up to the same 60%), leaving room for its list.
            if (p.FitBody && p.Items.Count > 0) bodyH = Math.Min(bodyH, g.MeasureString(p.Body, bodyFont, (int)(w - 2 * m - 2 * h * 0.024f)).Height + 2 * h * 0.024f + bodyFont.Height * 0.3f);
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
                float pop = BackOut((t - bounceAt - 0.05f * Math.Max(0, i - p.ScrollRow)) / 0.36f);
                if (pop <= 0.01f) continue;
                var popped = g.Save();
                if (t - bounceAt < 1.4f) { float cx = r.X + r.Width / 2, cy = r.Y + r.Height / 2; g.TranslateTransform(cx, cy + (1 - pop) * rowH * 0.5f); g.ScaleTransform(0.85f + 0.15f * pop, 0.85f + 0.15f * pop); g.TranslateTransform(-cx, -cy); }
                ListRow(g, p.Items[i], i == p.Sel, r, i < p.Items.Count - 1, h, accent, th);
                g.Restore(popped);
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
    static readonly Color Warn = Color.FromArgb(232, 128, 24);
    static readonly Color[] RowColors = { Color.FromArgb(232, 62, 58), Color.FromArgb(240, 142, 30), Color.FromArgb(46, 164, 79), Color.FromArgb(40, 124, 232), Color.FromArgb(124, 84, 214), Color.FromArgb(24, 164, 150), Color.FromArgb(226, 72, 140), Color.FromArgb(92, 104, 124) };
    void ListRow(Graphics g, Item it, bool sel, RectangleF r, bool line, int h, Color accent, Theme th) {
        if (sel) using (var b = new SolidBrush(Fade(accent, 38))) g.FillRectangle(b, r);
        if (it.Fold != 0) { SectionRow(g, it, sel, r, line, h, accent); return; }
        float icon = r.Height * 0.62f, pad = r.Height * 0.2f, ix = r.X + pad, iy = r.Y + (r.Height - icon) / 2;
        var ib = new RectangleF(ix, iy, icon, icon);
        var pic = it.Sprite != null ? Sprites.Get(it.Sprite) : null;
        if (pic != null) DrawSprite(g, pic, new RectangleF(ib.X - icon * 0.15f, ib.Y - icon * 0.15f, icon * 1.3f, icon * 1.3f));
        else if (Sprites.Loading(it.Sprite)) { SpinningBall(g, ib.X + icon / 2, ib.Y + icon / 2, icon * 0.8f, (float)clock.Elapsed.TotalSeconds); spinning = true; }
        else if (it.Theme != null) Cart(g, ib.X + icon * 0.1f, ib.Y, icon, it.Theme);
        else {
            Color tint = !it.On ? Text3 : it.Tint != Color.Empty ? it.Tint : RowColors[it.Icon % RowColors.Length];
            GlossTile(g, ib, icon * 0.24f, tint);
            if (!RowArt(g, it.Icon, ib, tint)) {
            Write(g, it.Icon.ToString(), iconSmall, Color.FromArgb(90, Ink), new RectangleF(ib.X, ib.Y + icon * 0.04f, ib.Width, ib.Height), StringAlignment.Center, StringAlignment.Center, false);
            Write(g, it.Icon.ToString(), iconSmall, Color.White, ib, StringAlignment.Center, StringAlignment.Center, false);
            }
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
            Write(g, value, itemFont, value == "★" ? Color.FromArgb(226, 164, 16) : sel ? accent : PhoneSub, new RectangleF(right - valueW, r.Y, valueW, r.Height), StringAlignment.Far, StringAlignment.Center, false);
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
    // A section header: an arrow pointing down when open, right when folded, then its name and how many it holds.
    void SectionRow(Graphics g, Item it, bool sel, RectangleF r, bool line, int h, Color accent) {
        float pad = r.Height * 0.2f, c = r.Height * 0.11f, cx = r.X + pad + r.Height * 0.31f, cy = r.Y + r.Height / 2;
        var arrow = it.Fold == 1
            ? new[] { new PointF(cx - c, cy - c * 0.5f), new PointF(cx, cy + c * 0.5f), new PointF(cx + c, cy - c * 0.5f) }
            : new[] { new PointF(cx - c * 0.5f, cy - c), new PointF(cx + c * 0.5f, cy), new PointF(cx - c * 0.5f, cy + c) };
        using (var pen = new Pen(accent, Math.Max(2f, h * 0.0035f))) { pen.StartCap = pen.EndCap = System.Drawing.Drawing2D.LineCap.Round; pen.LineJoin = System.Drawing.Drawing2D.LineJoin.Round; g.DrawLines(pen, arrow); }
        string title = it.Text, count = "";
        int colon = title.LastIndexOf(": ");
        if (colon > 0) { count = title.Substring(colon + 2); title = title.Substring(0, colon); }
        float tx = r.X + pad * 2 + r.Height * 0.62f, right = r.Right - pad;
        float cw = count != "" ? g.MeasureString(count, itemFont).Width + 4 : 0;
        if (count != "") Write(g, count, itemFont, sel ? accent : PhoneSub, new RectangleF(right - cw, r.Y, cw, r.Height), StringAlignment.Far, StringAlignment.Center, false);
        Write(g, title, itemBold, sel ? accent : PhoneText, new RectangleF(tx, r.Y, right - cw - tx, r.Height), StringAlignment.Near, StringAlignment.Center, false);
        if (line) using (var pen = new Pen(PhoneLine, 1)) g.DrawLine(pen, r.X, r.Bottom - 1, r.Right, r.Bottom - 1);
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
        // Starting up: the same rainbow Poke Ball as the working screen, in its own colours on this dark screen.
        SpinningBall(g, cx, cy - h * 0.1f + rise, h * 0.104f, t, true);
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
        // Working: the spinning Poke Ball, where the cartridge picture was.
        SpinningBall(g, w / 2f, y + cartLen * 0.47f, cartLen * 0.9f, t);
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
    // Another copy of nativeMons, elsewhere on the PC, is a different launcher with its own games and saves.
    static bool SameFolder(Process other) {
        try { return string.Equals(Path.GetFullPath(Path.GetDirectoryName(other.MainModule.FileName)).TrimEnd('\\'), Path.GetFullPath(Config.Here).TrimEnd('\\'), StringComparison.OrdinalIgnoreCase); }
        catch { return false; }
    }
    [STAThread]
    static void Main(string[] argv) {
        Application.EnableVisualStyles();
        Config.Load();
        // Modes that run without the window (notes/launcher.md, Command line).
        if (argv.Length >= 3 && argv[0] == "--add-roms") {
            var paths = argv.Skip(2).ToList();
            var added = new List<string>(); var notes = new List<string>();
            LauncherForm.AddPaths(paths.Where(Directory.Exists).ToList(), paths.Where(x => File.Exists(x) && Library.IsRom(x)).ToList(), paths.Where(x => File.Exists(x) && x.EndsWith(".zip", StringComparison.OrdinalIgnoreCase)).ToList(), added, notes);
            File.WriteAllLines(argv[1], added.Select(f => f + "\t" + LauncherForm.WhatRomIs(f)).Concat(notes).ToArray());
            return;
        }
        // --add-pc <result> folder|zip <path> [program number]
        if (argv.Length >= 4 && argv[0] == "--add-pc") {
            var o = new List<string>(); CustomGames.Entry e = null; string problem = null, work = null;
            List<string> programs;
            if (argv[2] == "zip") problem = CustomGames.Unpack(argv[3], delegate { }, out work, out programs);
            else programs = CustomGames.Programs(argv[3]);
            if (problem == null && programs.Count == 0) problem = "no program";
            int pick = 0;
            if (problem == null && programs.Count > 1 && !(argv.Length > 4 && int.TryParse(argv[4], out pick))) {
                for (int i = 0; i < programs.Count; i++) o.Add("program " + i + "\t" + programs[i]);
                if (work != null) PcGames.Remove(work);
                problem = "choose";
            }
            if (problem == null) problem = argv[2] == "zip" ? CustomGames.AddUnpacked(argv[3], work, programs[pick], out e) : CustomGames.Add(programs[pick], false, delegate { }, out e);
            o.Insert(0, problem == null ? "added\t" + e.id + "\t" + e.title + "\t" + CustomGames.Exe(e) + "\tcopy=" + e.copy : "not added\t" + problem);
            File.WriteAllLines(argv[1], o.ToArray());
            return;
        }
        // --install-index-mod <game id> <mod id> <result file>: Find mods' Install, without the window (for tests).
        if (argv.Length >= 4 && argv[0] == "--install-index-mod") {
            Library.Scan(LauncherForm.Gen1Cli);
            string note, result;
            try {
                var one = ModIndex.Load(out note).Find(x => x.Id == argv[2]);
                var game = Library.Games.Find(x => x.Id == argv[1]);
                result = one == null ? "not in the index" : game == null || !ModIndex.Fits(one, game.Gen1) ? "not for that game" : ModIndex.Install(one, delegate { });
            } catch (Exception x) { result = x.Message; }
            File.WriteAllText(argv[3], result);
            return;
        }
        // --mod-clashes <game id> <result file>: the clashes the Mods page shows for that game.
        // --gba-art <rom> <folder> <frames> <press> <shot,shot,...|every:<n>>: saves those frames of a GBA ROM, run on mGBA.
        if (argv.Length >= 6 && argv[0] == "--gba-art") {
            int frames = int.Parse(argv[3]);
            var shots = argv[5].StartsWith("every:") ? Enumerable.Range(0, frames).Where(i => i % int.Parse(argv[5].Substring(6)) == 0) : argv[5].Split(',').Select(int.Parse);
            string why = LibretroShots.Run(Config.MgbaCore, argv[1], argv[2], argv[4], shots, frames);
            if (why != null) File.WriteAllText(Path.Combine(argv[2], "error.txt"), why);
            Environment.Exit(why == null ? 0 : 1);
        }
        if (argv.Length >= 3 && argv[0] == "--mod-clashes") {
            Library.Scan(LauncherForm.Gen1Cli);
            var game = Library.Games.Find(x => x.Id == argv[1]);
            var lines = game == null ? new List<string> { "no such game" } : ModClashes.Read(game).Select(c => c.ModId + " (" + (c.ModOn ? "on" : "off") + ") clashes with " + c.OtherId + " (" + (c.OtherOn ? "on" : "off") + ")" + (c.Active ? ": " + c.ModId + " won't load" : "")).ToList();
            File.WriteAllLines(argv[2], lines.Count == 0 ? new[] { "no clashes" } : lines.ToArray());
            return;
        }
        // --mod-deps <game id> <result file>: what the Needs other mods page lists for that game.
        if (argv.Length >= 3 && argv[0] == "--mod-deps") {
            Library.Scan(LauncherForm.Gen1Cli);
            var game = Library.Games.Find(x => x.Id == argv[1]);
            var lines = game == null ? new List<string> { "no such game" } : ModDeps.Open(game).Select(d => d.Id + "\t" + (d.Required ? "required" : "optional") + "\t" + d.Status + "\tfor " + d.ModId + (d.Range != "" ? "\t" + d.Range : "") + (d.Github != "" ? "\tgithub " + d.Github : "")).ToList();
            File.WriteAllLines(argv[2], lines.Count == 0 ? new[] { "nothing missing" } : lines.ToArray());
            return;
        }
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
        // For tests: --roms <result file> lists what the scan found for every game, and what each engine uses.
        if (argv.Length == 2 && argv[0] == "--roms") {
            Library.Scan(LauncherForm.Gen1Cli);
            var o = new List<string>();
            foreach (var g in Library.Games) {
                o.Add(g.Id + " found=" + g.Roms.Count + " chosen=" + (g.Chosen != null ? g.Chosen.Label + " " + Path.GetFileName(g.Chosen.File) : "-") + " | fulldex=" + Path.GetFileName(g.FulldexRom ?? "") + " | gen1=" + Path.GetFileName(g.Gen1Rom ?? "") + " | nativeplat=" + Path.GetFileName(g.NativeplatRom ?? "") + " | refused=" + Path.GetFileName(g.FoundUnsupported ?? ""));
                foreach (var r in g.Roms) o.Add("    " + r.Label + " gen1=" + r.Gen1Ok + " nativeplat=" + r.NativeplatOk + " enc=" + r.Encrypted + " copy=" + r.EngineCopy + " " + r.File);
            }
            File.WriteAllLines(argv[1], o.ToArray());
            return;
        }
        // For tests: --update <result file> checks for a release and applies it, with no window.
        if (argv.Length == 2 && argv[0] == "--update") {
            string problem = Updater.Check();
            string line = "installed " + Updater.Build + " latest " + (Updater.Latest != null ? Updater.Latest.build.ToString() : "?") + " ready " + Updater.Ready;
            if (problem == null && Updater.Ready) problem = Updater.Apply(delegate(string a, float b) { });
            File.WriteAllText(argv[1], line + "\nresult " + (problem ?? "ok") + "\nnow " + Updater.Build + "\n");
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
        // --install-mod <zip> <result> [mod id]: only that mod, when an id is given.
        if (argv.Length >= 3 && argv[0] == "--install-mod") {
            string result;
            try { result = LauncherForm.ExtractMod(argv[1], argv.Length > 3 ? argv[3] : null); } catch (Exception e) { result = "Could not install it: " + e.Message; }
            File.WriteAllText(argv[2], result);
            return;
        }
        // --install-pc <game id> <folder|zip|download> <result file>: installs a PC game as its page does, then its pictures.
        if (argv.Length >= 4 && argv[0] == "--install-pc") {
            Library.Scan(LauncherForm.Gen1Cli);
            var pc = Library.Games.Find(x => x.Id == argv[1] && x.Pc != null);
            string result = pc == null ? "No such PC game." : PcGames.Install(pc, argv[2] == "download" ? null : argv[2], delegate(string what, float at) { });
            if (pc != null && result == null) { result = (GameArt.Capture(pc) ?? "title ok") + "; " + (GameArt.CaptureTown(pc) ?? "town ok") + "; version " + PcGames.Version(pc); }
            File.WriteAllText(argv[3], result ?? "");
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
        // A test's own state folder (NATIVEMONS_STATE) is its own launcher, so tests can run side by side.
        if (Environment.GetEnvironmentVariable("NATIVEMONS_STATE") != null) folder += "|" + Path.GetFullPath(Config.State).ToLowerInvariant().Replace('\\', '/');
        bool first;
        using (var instance = new System.Threading.Mutex(true, "Local\\NativeMonsLauncher-" + folder, out first)) {
            // Shortcut flags: --game (or --play) <id>, --slot <save>, --launcher.
            Func<string, string> flag = delegate(string name) { int i = Array.IndexOf(argv, name); return i >= 0 && i + 1 < argv.Length ? argv[i + 1] : null; };
            string game = flag("--game") ?? flag("--play"), slot = flag("--slot");
            bool menu = Array.IndexOf(argv, "--launcher") >= 0;
            if (!first) {
                // The one already open does it: it's told through a file in state\ and a message to its window.
                if (game != null) try { File.WriteAllText(Path.Combine(Config.State, "request.txt"), (menu ? "show" : "play") + "\n" + game + "\n" + (slot ?? "")); } catch { }
                int me = Process.GetCurrentProcess().Id;
                foreach (var other in Process.GetProcessesByName("NativeMonsLauncher"))
                    if (other.Id != me && other.MainWindowHandle != IntPtr.Zero && SameFolder(other)) {
                        if (game != null) Native.PostMessage(other.MainWindowHandle, LauncherForm.WM_REQUEST, IntPtr.Zero, IntPtr.Zero);
                        if (Native.IsWindowVisible(other.MainWindowHandle)) Native.Front(other.MainWindowHandle);
                        break;
                    }
                return;
            }
            Application.Run(new LauncherForm(menu ? null : game, slot, menu ? game : null));
        }
    }
}
