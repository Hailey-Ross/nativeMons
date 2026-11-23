// Test helper for nickname-move.ps1: names a Pokemon in a save copy, or prints one's names.
// nickname-helper rename <save> <box> <slot> <nickname> | show <save> <box> <slot>
using PKHeX.Core;

if (args.Length < 4) { Console.Error.WriteLine("usage: rename <save> <box> <slot> <nickname> | show <save> <box> <slot>"); return 1; }
var sav = SaveUtil.GetSaveFile(args[1]) ?? throw new InvalidDataException("Not a save: " + args[1]);
int box = int.Parse(args[2]), slot = int.Parse(args[3]);
var pk = sav.GetBoxSlotAtIndex(box, slot);
if (pk.Species == 0) { Console.Error.WriteLine("empty slot"); return 1; }

if (args[0] == "rename" && args.Length == 5)
{
    if (sav is not (SAV1 or SAV2)) { Console.Error.WriteLine("rename only edits Gen 1/2 copies"); return 1; }
    pk.IsNicknamed = true;
    pk.Nickname = args[4];
    sav.SetBoxSlotAtIndex(pk, box, slot);
    File.WriteAllBytes(args[1], sav.Write().ToArray());
    return 0;
}
if (args[0] == "dump" && args.Length == 5)
{
    File.WriteAllBytes(args[4], pk.Data.ToArray());
    Console.WriteLine(pk.Extension);
    return 0;
}
if (args[0] == "show")
{
    Console.WriteLine(string.Join('\t', GameInfo.Strings.Species[pk.Species], pk.Nickname, pk.IsNicknamed ? "nicknamed" : "species-name", pk.OriginalTrainerName, pk.Language));
    return 0;
}
Console.Error.WriteLine("unknown command");
return 1;
