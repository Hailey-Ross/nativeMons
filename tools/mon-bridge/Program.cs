// mon-bridge: lets the nativeMons launcher read cartridge saves and move Pokemon between them, using PKHeX.Core
// (https://github.com/kwsch/PKHeX, GPL-3.0-or-later). Licensed under the GNU GPL v3 or later.
//
//   mon-bridge list <save>                                  one tab-separated row per Pokemon in the boxes:
//                                                           box, slot, species, nickname, level, trainer
//   mon-bridge info <save>                                  save type, game, generation, trainer
//   mon-bridge move <from save> <box> <slot> <to save> <backup folder>
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
            if (args.Length == 6 && args[0] == "move") return Move(args[1], int.Parse(args[2]), int.Parse(args[3]), args[4], args[5]);
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
                Console.WriteLine(string.Join('\t', box, slot, names[pk.Species], Clean(pk.Nickname), pk.CurrentLevel, Clean(pk.OriginalTrainerName)));
            }
        }
        return 0;
    }

    static int Refuse(string why)
    {
        Console.WriteLine("nope\t" + why);
        return 2;
    }

    static int Move(string fromPath, int box, int slot, string toPath, string backups)
    {
        var from = Open(fromPath);
        var to = Open(toPath);
        var pk = from.GetBoxSlotAtIndex(box, slot);
        if (pk.Species == 0) return Refuse("That box slot is empty.");
        string name = GameInfo.Strings.Species[pk.Species];

        if (pk.Species > to.MaxSpeciesID || !to.Personal.IsSpeciesInGame(pk.Species))
            return Refuse(name + " doesn't exist in " + to.Version + ".");

        // PKHeX has no route for these, and asking it can run for a very long time.
        bool oldFrom = from.Generation <= 2, oldTo = to.Generation <= 2;
        if (oldFrom != oldTo)
            return Refuse(name + " can't go between Generation " + from.Generation + " and Generation " + to.Generation + ": the games have no way to transfer between them.");
        if (!oldFrom && to.Generation < from.Generation)
            return Refuse(name + " can't go back from Generation " + from.Generation + " to Generation " + to.Generation + ": Pokemon only travel forward to newer games.");

        // Nothing has been written yet, so a conversion that never finishes can simply be abandoned.
        var work = Task.Run(() =>
        {
            var converted = EntityConverter.ConvertToType(pk, to.PKMType, out var result);
            if (converted is null || result != EntityConverterResult.Success) return ((PKM?)null, false);
            to.AdaptToSaveFile(converted);
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
