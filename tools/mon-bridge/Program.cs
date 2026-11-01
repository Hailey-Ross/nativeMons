// mon-bridge: lets the nativeMons launcher read cartridge saves and move Pokemon between them, using PKHeX.Core
// (https://github.com/kwsch/PKHeX, GPL-3.0-or-later). Licensed under the GNU GPL v3 or later.
//
//   mon-bridge list <save>                                  one tab-separated row per Pokemon in the boxes:
//                                                           box, slot, species, nickname, level, trainer, dex number, shiny/normal
//   mon-bridge info <save>                                  save type, game, generation, trainer
//   mon-bridge plan <from save> <box> <slot> <to save>      what stands in the way: forced, conflict/ok <n> <move>, item <name>
//   mon-bridge caught <save>                                dex numbers marked caught, comma separated
//   mon-bridge check <save>                                 corruption check: ok or bad, with why
//   mon-bridge compare <a> <b>                              same trainer, Pokemon, Pokedex and items?
//   mon-bridge repair <save> <new file>                     rewrites a copy with correct checksums
//   mon-bridge families                                     evolution families: comma-separated dex numbers
//   mon-bridge moves <save>                                 the moves that game has: id, name
//   mon-bridge move <from save> <box> <slot> <to save> <backup folder> [move:<1-4>:<id, 0 deletes>]... [dropitem]
//
// Exit codes: 0 done, 1 usage or unreadable save, 2 the move is not possible (saves untouched).
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using PKHeX.Core;

static class Program
{
    static int Main(string[] args)
    {
        Console.OutputEncoding = System.Text.Encoding.UTF8;
        try
        {
            if (args.Length == 2 && args[0] == "list") return List(args[1]);
            if (args.Length == 2 && args[0] == "info") { var i = Open(args[1]); Console.WriteLine(i.GetType().Name + "\t" + i.Version + "\t" + i.Generation + "\t" + Clean(i.OT)); return 0; }
            if (args.Length >= 6 && args[0] == "move") return Move(args[1], int.Parse(args[2]), int.Parse(args[3]), args[4], args[5], args.Skip(6).ToArray());
            if (args.Length == 5 && args[0] == "plan") return Plan(args[1], int.Parse(args[2]), int.Parse(args[3]), args[4]);
            if (args.Length == 2 && args[0] == "moves") return Moves(args[1]);
            if (args.Length == 1 && args[0] == "families") return Families();
            if (args.Length == 2 && args[0] == "check") return Check(args[1]);
            if (args.Length == 2 && args[0] == "summary") return Summary(args[1]);
            if (args.Length == 3 && args[0] == "compare") return Compare(args[1], args[2]);
            if (args.Length == 3 && args[0] == "repair") return Repair(args[1], args[2]);
            if (args.Length == 2 && args[0] == "caught") return Caught(args[1]);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine(e.Message);
            return 1;
        }
        Console.Error.WriteLine("usage: mon-bridge list <save> | move <from save> <box> <slot> <to save> <backup folder>");
        return 1;
    }

    static SaveFile Open(string path) =>
        Read(path) ?? throw new InvalidDataException("Not a save this tool can read: " + Path.GetFileName(path));

    static SaveFile? Read(string path)
    {
        var sav = SaveUtil.GetSaveFile(path);
        if (sav is not SAV3E) return sav;
        var data = File.ReadAllBytes(path);
        return HasFireRedLayout(data) ? new SAV3FRLG(data) : sav;
    }

    // PKHeX tells Gen 3 games apart by one value in the save's header, and calls anything but 0 or 1 Emerald. Some real
    // FireRed saves hold FFFFFFFF there. The party count settles it, as it sits at a different place in each layout and a
    // real Emerald save always has 0 to 6 at Emerald's place.
    static bool HasFireRedLayout(byte[] data)
    {
        const int sectionSize = 0x1000, sectionsPerSlot = 14, signature = 0x08012025;
        bool seen = false;
        foreach (int slot in new[] { 0, 0xE000 })
            for (int i = 0; i < sectionsPerSlot; i++)
            {
                int start = slot + i * sectionSize;
                if (start + sectionSize > data.Length) return false;
                if (BitConverter.ToInt32(data, start + 0xFF8) != signature || BitConverter.ToUInt16(data, start + 0xFF4) != 1) continue;
                int fireRedCount = data[start + 0x34], emeraldCount = data[start + 0x234];
                if (fireRedCount is < 1 or > 6 || emeraldCount <= 6) return false;
                seen = true;
            }
        return seen;
    }

    static string Clean(string s) => s.Replace('\t', ' ').Replace('\n', ' ').Replace('\r', ' ');

    static int List(string path)
    {
        var sav = Open(path);
        var names = GameInfo.Strings.Species;
        for (int box = 0; box < sav.BoxCount; box++)
        {
            for (int slot = 0; slot < sav.BoxSlotCount; slot++)
            {
                var pk = sav.GetBoxSlotAtIndex(box, slot);
                if (pk.Species == 0) continue;
                Console.WriteLine(string.Join('\t', box, slot, names[pk.Species], Clean(pk.Nickname), pk.CurrentLevel, Clean(pk.OriginalTrainerName), pk.Species, pk.IsShiny ? "shiny" : "normal"));
            }
        }
        return 0;
    }

    static int Refuse(string why)
    {
        Console.WriteLine("nope\t" + why);
        return 2;
    }

    // PKHeX has no route between these eras, and its converter never returns if asked. They are copied field by field.
    static bool NeedsForce(SaveFile from, SaveFile to)
    {
        bool oldFrom = from.Generation <= 2, oldTo = to.Generation <= 2;
        return oldFrom != oldTo || (!oldFrom && to.Generation < from.Generation);
    }

    static bool SpeciesFits(ushort species, SaveFile to) => species <= to.MaxSpeciesID && to.Personal.IsSpeciesInGame(species);

    // What stands in the way, for the player to resolve: moves the destination doesn't have, and a held item.
    static int Plan(string fromPath, int box, int slot, string toPath)
    {
        var from = Open(fromPath);
        var to = Open(toPath);
        var pk = from.GetBoxSlotAtIndex(box, slot);
        if (pk.Species == 0) return Refuse("That box slot is empty.");
        if (!SpeciesFits(pk.Species, to))
            return Refuse(GameInfo.Strings.Species[pk.Species] + " doesn't exist in " + to.Version + ", so it can't go there.");
        // Going home to its original format: nothing to resolve, since its own moves and item come back with it.
        if (FindOriginal(pk, to, PlanOriginals(toPath)) is not null) return 0;
        bool force = NeedsForce(from, to);
        if (force) Console.WriteLine("forced");
        var moves = new[] { pk.Move1, pk.Move2, pk.Move3, pk.Move4 };
        for (int i = 0; i < 4; i++)
            if (moves[i] != 0)
                Console.WriteLine((moves[i] > to.MaxMoveID ? "conflict" : "ok") + "\t" + (i + 1) + "\t" + GameInfo.Strings.Move[moves[i]]);
        if (force && pk.HeldItem != 0) Console.WriteLine("item\t" + GameInfo.Strings.Item[pk.HeldItem]);
        return 0;
    }

    // Every evolution family up to Generation 5, the newest games here: one comma-separated row each, lowest dex number first.
    static int Families()
    {
        const int max = 649;
        var parent = Enumerable.Range(0, max + 1).ToArray();
        int Find(int x) { while (parent[x] != x) x = parent[x] = parent[parent[x]]; return x; }
        var forward = EvolutionTree.Evolves5.Forward;
        for (ushort species = 1; species <= max; species++)
            foreach (var (next, _) in forward.GetEvolutions(species, 0))
                if (next is > 0 and <= max) parent[Find(next)] = Find(species);
        foreach (var family in Enumerable.Range(1, max).GroupBy(Find))
            Console.WriteLine(string.Join(',', family.OrderBy(x => x)));
        return 0;
    }

    // The dex numbers this save's Pokedex marks as caught, comma separated.
    static int Caught(string path)
    {
        var sav = Open(path);
        var caught = sav.HasPokeDex ? Enumerable.Range(1, sav.MaxSpeciesID).Where(i => sav.GetCaught((ushort)i)) : Enumerable.Empty<int>();
        Console.WriteLine(string.Join(',', caught));
        return 0;
    }

    // For telling saves apart: trainer, hours, minutes, badges (-1 when unknown), caught, party.
    static int Summary(string path)
    {
        var sav = Open(path);
        int badges = -1;
        var property = sav.GetType().GetProperty("Badges");
        if (property?.GetValue(sav) is { } value)
        {
            try { badges = System.Numerics.BitOperations.PopCount(Convert.ToUInt64(value)); } catch { badges = -1; }
        }
        int caught = sav.HasPokeDex ? Enumerable.Range(1, sav.MaxSpeciesID).Count(i => sav.GetCaught((ushort)i)) : -1;
        var party = new List<string>();
        for (int i = 0; i < sav.PartyCount; i++)
        {
            var mon = sav.GetPartySlotAtIndex(i);
            if (mon.Species != 0) party.Add(GameInfo.Strings.Species[mon.Species] + " " + mon.CurrentLevel);
        }
        Console.WriteLine(string.Join('\t', Clean(sav.OT), sav.PlayedHours, sav.PlayedMinutes, badges, caught, string.Join(", ", party)));
        return 0;
    }

    // Corruption check: readable, and every checksum in it right.
    static int Check(string path)
    {
        var sav = Read(path);
        if (sav is null) { Console.WriteLine("bad\tThis isn't a save that can be read: it may be damaged."); return 3; }
        if (!sav.ChecksumsValid) { Console.WriteLine("bad\tIts checksums are wrong: " + Clean(sav.ChecksumInfo)); return 3; }
        Console.WriteLine(string.Join('\t', "ok", sav.Version, Clean(sav.OT), Facts(sav).Count));
        return 0;
    }

    // What must survive a conversion, one fact per line.
    static List<string> Facts(SaveFile sav)
    {
        var facts = new List<string> { "trainer " + sav.OT + " " + sav.TID16 + " money " + sav.Money };
        string Mon(PKM p) => string.Join(' ', p.Species, p.CurrentLevel, p.EXP, p.Nickname, p.OriginalTrainerName, p.TID16, p.HeldItem, p.Move1, p.Move2, p.Move3, p.Move4, p.IV_HP, p.IV_ATK, p.IV_DEF, p.IV_SPA, p.IV_SPD, p.IV_SPE, p.EV_HP, p.EV_ATK, p.EV_DEF, p.EV_SPA, p.EV_SPD, p.EV_SPE, p.IsShiny);
        for (int i = 0; i < sav.PartyCount; i++) facts.Add("party " + (i + 1) + ": " + Mon(sav.GetPartySlotAtIndex(i)));
        for (int box = 0; box < sav.BoxCount; box++)
            for (int slot = 0; slot < sav.BoxSlotCount; slot++)
            {
                var p = sav.GetBoxSlotAtIndex(box, slot);
                if (p.Species != 0) facts.Add("box " + (box + 1) + " slot " + (slot + 1) + ": " + Mon(p));
            }
        if (sav.HasPokeDex)
            facts.Add("caught " + string.Join(',', Enumerable.Range(1, sav.MaxSpeciesID).Where(i => sav.GetCaught((ushort)i))));
        foreach (var pouch in sav.Inventory.Pouches)
            foreach (var item in pouch.Items.Where(x => x.Index != 0 && x.Count != 0).OrderBy(x => x.Index))
                facts.Add("item " + pouch.Type + " " + item.Index + " x" + item.Count);
        return facts;
    }

    // Exit 0 when two saves hold the same trainer, Pokemon, Pokedex and items; otherwise prints what differs.
    static int Compare(string a, string b)
    {
        var x = Facts(Open(a)); var y = Facts(Open(b));
        var gone = x.Except(y).ToList(); var added = y.Except(x).ToList();
        if (gone.Count == 0 && added.Count == 0) { Console.WriteLine("same\t" + x.Count); return 0; }
        Console.WriteLine("different\t" + gone.Count + " lost, " + added.Count + " new");
        foreach (var f in gone.Take(8)) Console.WriteLine("lost\t" + Clean(f));
        foreach (var f in added.Take(8)) Console.WriteLine("new\t" + Clean(f));
        return 4;
    }

    // Rewrites a save through PKHeX, which recalculates its checksums. Only ever writes the new file.
    static int Repair(string from, string to)
    {
        var sav = Open(from);
        var data = sav.Write().ToArray();
        if (sav is SAV3FRLG) MarkAsFireRed(data);
        File.WriteAllBytes(to, data);
        return Check(to);
    }

    // Stricter readers, Gen1Recomp's among them, go by the header value alone (see HasFireRedLayout), so a repaired copy gets
    // the value FireRed normally has. It's only ever written to the new file.
    static void MarkAsFireRed(byte[] data)
    {
        const int sectionSize = 0x1000, sectionsPerSlot = 14, trainerLength = 3884;
        foreach (int slot in new[] { 0, 0xE000 })
            for (int i = 0; i < sectionsPerSlot; i++)
            {
                int start = slot + i * sectionSize;
                if (start + sectionSize > data.Length || BitConverter.ToUInt16(data, start + 0xFF4) != 0) continue;
                if (BitConverter.ToInt32(data, start + 0xFF8) != 0x08012025) continue;
                BitConverter.TryWriteBytes(data.AsSpan(start + 0xAC), 1);
                uint sum = 0;
                for (int at = 0; at < trainerLength; at += 4) sum += BitConverter.ToUInt32(data, start + at);
                BitConverter.TryWriteBytes(data.AsSpan(start + 0xFF6), (ushort)((sum >> 16) + sum));
            }
    }

    static int Moves(string path)
    {
        var sav = Open(path);
        for (int id = 1; id <= sav.MaxMoveID; id++) Console.WriteLine(id + "\t" + GameInfo.Strings.Move[id]);
        return 0;
    }

    static PKM ForceCopy(PKM pk, SaveFile to)
    {
        var e = to.BlankPKM;
        e.Species = pk.Species;
        e.Form = 0;
        e.CurrentLevel = pk.CurrentLevel;
        e.Language = pk.Language == 0 ? 2 : pk.Language;
        e.OriginalTrainerName = pk.OriginalTrainerName.Length > e.MaxStringLengthTrainer ? pk.OriginalTrainerName[..e.MaxStringLengthTrainer] : pk.OriginalTrainerName;
        e.OriginalTrainerGender = pk.OriginalTrainerGender;
        e.TID16 = pk.TID16;
        if (to.Generation >= 3) e.SID16 = pk.SID16;
        e.Nickname = pk.Nickname.Length > e.MaxStringLengthNickname ? pk.Nickname[..e.MaxStringLengthNickname] : pk.Nickname;
        e.IsNicknamed = pk.IsNicknamed;
        // Each era has its own range for these, and the DV and IV scales differ by 2 either way.
        int Iv(int v) => from2(pk) == from2(e) ? Math.Min(v, e.MaxIV) : from2(pk) ? Math.Min(v * 2 + 1, e.MaxIV) : Math.Min(v / 2, e.MaxIV);
        e.IV_HP = Iv(pk.IV_HP); e.IV_ATK = Iv(pk.IV_ATK); e.IV_DEF = Iv(pk.IV_DEF);
        e.IV_SPA = Iv(pk.IV_SPA); e.IV_SPD = Iv(pk.IV_SPD); e.IV_SPE = Iv(pk.IV_SPE);
        if (from2(pk) == from2(e))
        {
            e.EV_HP = Math.Min(pk.EV_HP, e.MaxEV); e.EV_ATK = Math.Min(pk.EV_ATK, e.MaxEV); e.EV_DEF = Math.Min(pk.EV_DEF, e.MaxEV);
            e.EV_SPA = Math.Min(pk.EV_SPA, e.MaxEV); e.EV_SPD = Math.Min(pk.EV_SPD, e.MaxEV); e.EV_SPE = Math.Min(pk.EV_SPE, e.MaxEV);
        }
        if (to.Generation >= 3)
        {
            if (pk.Format >= 3) { e.PID = pk.PID; e.Gender = pk.Gender; }
            else { e.PID = (uint)Random.Shared.NextInt64(1L << 32); e.SetPIDGender(pk.Gender); }
            e.Nature = pk.Format >= 3 ? pk.Nature : (Nature)(e.PID % 25);
            e.RefreshAbility(0);
            e.Ball = 4;
            e.MetLevel = pk.CurrentLevel;
            e.Version = to.Version;
        }
        return e;
    }
    static bool from2(PKM p) => p.Format <= 2;

    // OpenHome's converter (tools/openhome-bridge), which keeps more than ForceCopy can. Null when it isn't there, fails, or
    // gives back anything but this Pokemon in the right format, and the caller falls back to ForceCopy.
    static PKM? OpenHomeCopy(PKM pk, SaveFile to)
    {
        string? node = Environment.GetEnvironmentVariable("NATIVEMONS_NODE"), tool = Environment.GetEnvironmentVariable("NATIVEMONS_OPENHOME");
        if (string.IsNullOrEmpty(node) || string.IsNullOrEmpty(tool) || !File.Exists(node) || !File.Exists(tool)) return null;
        string input = Path.GetTempFileName(), output = Path.GetTempFileName();
        try
        {
            File.WriteAllBytes(input, pk.Data.ToArray());
            var start = new System.Diagnostics.ProcessStartInfo(node) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardError = true, RedirectStandardOutput = true };
            foreach (var a in new[] { tool, pk.Extension, input, to.BlankPKM.Extension, output }) start.ArgumentList.Add(a);
            using var p = System.Diagnostics.Process.Start(start)!;
            var quiet = p.StandardError.ReadToEndAsync(); var quieter = p.StandardOutput.ReadToEndAsync();
            if (!p.WaitForExit(15000)) { try { p.Kill(); } catch { } return null; }
            if (p.ExitCode != 0) return null;
            var made = EntityFormat.GetFromBytes(File.ReadAllBytes(output), to.Context);
            if (made is null || made.GetType() != to.PKMType || made.Species != pk.Species) return null;
            return made;
        }
        catch { return null; }
        finally { try { File.Delete(input); File.Delete(output); } catch { } }
    }

    static int Move(string fromPath, int box, int slot, string toPath, string backups, string[] options)
    {
        var from = Open(fromPath);
        var to = Open(toPath);
        var pk = from.GetBoxSlotAtIndex(box, slot);
        if (pk.Species == 0) return Refuse("That box slot is empty.");
        string name = GameInfo.Strings.Species[pk.Species];
        if (!SpeciesFits(pk.Species, to)) return Refuse(name + " doesn't exist in " + to.Version + ", so it can't go there.");
        string originals = Path.Combine(Path.GetDirectoryName(Path.GetFullPath(backups))!, "pokemon-originals");
        string? home = FindOriginal(pk, to, originals);
        if (home is not null) return MoveHome(from, to, pk, box, slot, fromPath, toPath, backups, home, name);

        // The player's answers: move:<1-4>:<new id, 0 deletes>. dropitem agrees to losing a held item.
        var moves = new[] { pk.Move1, pk.Move2, pk.Move3, pk.Move4 };
        bool dropItem = false;
        foreach (var o in options)
        {
            if (o == "dropitem") { dropItem = true; continue; }
            var p = o.Split(':');
            if (p.Length == 3 && p[0] == "move") moves[int.Parse(p[1]) - 1] = ushort.Parse(p[2]);
        }
        if (moves.Any(m => m > to.MaxMoveID)) return Refuse("Some of its moves don't exist in " + to.Version + ". Delete or change them first.");
        var known = moves.Where(m => m != 0).Distinct().ToArray();
        if (known.Length == 0) return Refuse("It needs at least one move.");
        bool force = NeedsForce(from, to);
        if (force && pk.HeldItem != 0 && !dropItem) return Refuse("Its held item can't go with it. Agree to remove it first.");

        // Nothing has been written yet, so a conversion that never finishes can simply be abandoned.
        var work = Task.Run(() =>
        {
            PKM? converted;
            if (force)
            {
                converted = OpenHomeCopy(pk, to);
                Console.Error.WriteLine(converted is null ? "converter: hand copy" : "converter: OpenHome");
                converted ??= ForceCopy(pk, to);
            }
            else
            {
                converted = EntityConverter.ConvertToType(pk, to.PKMType, out var result);
                if (converted is null || result is not (EntityConverterResult.Success or EntityConverterResult.None)) return ((PKM?)null, false);
            }
            converted.Move1 = known.Length > 0 ? known[0] : (ushort)0;
            converted.Move2 = known.Length > 1 ? known[1] : (ushort)0;
            converted.Move3 = known.Length > 2 ? known[2] : (ushort)0;
            converted.Move4 = known.Length > 3 ? known[3] : (ushort)0;
            converted.HealPP();
            to.AdaptToSaveFile(converted);
            converted.RefreshChecksum();
            return ((PKM?)converted, new LegalityAnalysis(converted).Valid);
        });
        if (!work.Wait(TimeSpan.FromSeconds(20)))
        {
            Refuse(name + " can't go from " + from.Version + " to " + to.Version + ": PKHeX could not finish converting it.");
            Console.Out.Flush();
            Environment.Exit(2);
        }
        var (moved, legal) = work.Result;
        if (moved is null)
            return Refuse(name + " can't go from " + from.Version + " to " + to.Version + ": PKHeX has no conversion between those games.");

        int open = to.NextOpenBoxSlot();
        if (open < 0) return Refuse("The boxes in " + to.Version + " are full.");

        Directory.CreateDirectory(backups);
        string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
        File.Copy(fromPath, Path.Combine(backups, stamp + "-from-" + Path.GetFileName(fromPath)), true);
        File.Copy(toPath, Path.Combine(backups, stamp + "-to-" + Path.GetFileName(toPath)), true);

        // A hand copy loses what the older game can't hold, so its full data is kept for the trip back.
        if (force) KeepOriginal(pk, moved, originals);

        to.SetBoxSlotAtIndex(moved, open);
        from.SetBoxSlotAtIndex(from.BlankPKM, box, slot);

        // The destination first: if anything fails after this, the Pokemon is duplicated, never lost.
        WriteSafely(toPath, to);
        WriteSafely(fromPath, from);
        Console.WriteLine("ok\t" + name + "\t" + (legal ? "legal" : "not-legal"));
        return 0;
    }

    // Who a Pokemon is, whatever save it's in. Evolving doesn't change it. Game Boy formats have no personality value.
    static string Identity(PKM p) => p.Format >= 3
        ? "pid-" + p.PID.ToString("x8") + "-" + p.TID16
        : "dv-" + p.IV_ATK + "." + p.IV_DEF + "." + p.IV_SPE + "." + p.IV_SPA + "-" + p.TID16;

    static string PlanOriginals(string savePath) => Environment.GetEnvironmentVariable("NATIVEMONS_ORIGINALS") ?? "";

    static void KeepOriginal(PKM original, PKM copy, string folder)
    {
        Directory.CreateDirectory(folder);
        string file = Path.Combine(folder, Identity(copy) + "." + original.Extension);
        File.WriteAllBytes(file + ".part", original.Data.ToArray());
        File.Move(file + ".part", file, true);
    }

    // The kept file for this Pokemon, when it's in the format this save uses.
    static string? FindOriginal(PKM pk, SaveFile to, string folder)
    {
        if (folder == "" || !Directory.Exists(folder)) return null;
        string file = Path.Combine(folder, Identity(pk) + "." + to.BlankPKM.Extension);
        return File.Exists(file) ? file : null;
    }

    static int MoveHome(SaveFile from, SaveFile to, PKM pk, int box, int slot, string fromPath, string toPath, string backups, string file, string name)
    {
        var original = EntityFormat.GetFromBytes(File.ReadAllBytes(file), to.Context);
        if (original is null || original.GetType() != to.PKMType) return Refuse("The original copy of " + name + " couldn't be read.");
        // What happened while it was away still counts: it may have evolved or gained levels.
        if (pk.Species != original.Species && SpeciesFits(pk.Species, to)) original.Species = pk.Species;
        if (pk.CurrentLevel > original.CurrentLevel) original.CurrentLevel = pk.CurrentLevel;
        to.AdaptToSaveFile(original);
        original.RefreshChecksum();
        bool legal = new LegalityAnalysis(original).Valid;
        int open = to.NextOpenBoxSlot();
        if (open < 0) return Refuse("The boxes in " + to.Version + " are full.");
        Directory.CreateDirectory(backups);
        string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
        File.Copy(fromPath, Path.Combine(backups, stamp + "-from-" + Path.GetFileName(fromPath)), true);
        File.Copy(toPath, Path.Combine(backups, stamp + "-to-" + Path.GetFileName(toPath)), true);
        to.SetBoxSlotAtIndex(original, open);
        from.SetBoxSlotAtIndex(from.BlankPKM, box, slot);
        WriteSafely(toPath, to);
        WriteSafely(fromPath, from);
        File.Delete(file);
        Console.WriteLine("ok\t" + name + "\t" + (legal ? "legal" : "not-legal") + "\trestored");
        return 0;
    }

    static void WriteSafely(string path, SaveFile sav)
    {
        string temp = path + ".moving";
        File.WriteAllBytes(temp, sav.Write().ToArray());
        File.Move(temp, path, true);
    }
}
