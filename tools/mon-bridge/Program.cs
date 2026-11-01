// mon-bridge: lets the nativeMons launcher read cartridge saves and move Pokemon between them, using PKHeX.Core
// (https://github.com/kwsch/PKHeX, GPL-3.0-or-later). Licensed under the GNU GPL v3 or later.
//
//   mon-bridge list <save>                                  one tab-separated row per Pokemon in the boxes:
//                                                           box, slot, species, nickname, level, trainer, dex number, shiny/normal
//   mon-bridge info <save>                                  save type, game, generation, trainer
//   mon-bridge plan <from save> <box> <slot> <to save>      what stands in the way: forced, conflict/ok <n> <move>, item <name>
//   mon-bridge moves <save>                                 the moves that game has: id, name
//   mon-bridge move <from save> <box> <slot> <to save> <backup folder> [move:<1-4>:<id, 0 deletes>]... [dropitem]
//
// Exit codes: 0 done, 1 usage or unreadable save, 2 the move is not possible (saves untouched).
using System;
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
        SaveUtil.GetSaveFile(path) ?? throw new InvalidDataException("Not a save this tool can read: " + Path.GetFileName(path));

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
        bool force = NeedsForce(from, to);
        if (force) Console.WriteLine("forced");
        var moves = new[] { pk.Move1, pk.Move2, pk.Move3, pk.Move4 };
        for (int i = 0; i < 4; i++)
            if (moves[i] != 0)
                Console.WriteLine((moves[i] > to.MaxMoveID ? "conflict" : "ok") + "\t" + (i + 1) + "\t" + GameInfo.Strings.Move[moves[i]]);
        if (force && pk.HeldItem != 0) Console.WriteLine("item\t" + GameInfo.Strings.Item[pk.HeldItem]);
        return 0;
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

    static int Move(string fromPath, int box, int slot, string toPath, string backups, string[] options)
    {
        var from = Open(fromPath);
        var to = Open(toPath);
        var pk = from.GetBoxSlotAtIndex(box, slot);
        if (pk.Species == 0) return Refuse("That box slot is empty.");
        string name = GameInfo.Strings.Species[pk.Species];
        if (!SpeciesFits(pk.Species, to)) return Refuse(name + " doesn't exist in " + to.Version + ", so it can't go there.");

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
            if (force) converted = ForceCopy(pk, to);
            else
            {
                converted = EntityConverter.ConvertToType(pk, to.PKMType, out var result);
                if (converted is null || result != EntityConverterResult.Success) return ((PKM?)null, false);
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

        to.SetBoxSlotAtIndex(moved, open);
        from.SetBoxSlotAtIndex(from.BlankPKM, box, slot);

        // The destination first: if anything fails after this, the Pokemon is duplicated, never lost.
        WriteSafely(toPath, to);
        WriteSafely(fromPath, from);
        Console.WriteLine("ok\t" + name + "\t" + (legal ? "legal" : "not-legal"));
        return 0;
    }

    static void WriteSafely(string path, SaveFile sav)
    {
        string temp = path + ".moving";
        File.WriteAllBytes(temp, sav.Write().ToArray());
        File.Move(temp, path, true);
    }
}
