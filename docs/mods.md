# Mods

[Back to the README](../README.md)

Installing, finding and managing mods for Gen1Recomp games. Mods only apply to games played on Gen1Recomp.

## Installing a mod

**Mods**, on the home screen or on a Gen1Recomp game's page, installs mods and turns them on. Choose a mod's `.zip`
from any drive. Installing a newer version over an old one keeps the old copy until the new one is in place.

A mod you install is switched on in every game it runs, unless it clashes with a mod that's already on there: then it
stays off, and you choose. The examples that come with Gen1Recomp, which need art imported from your own cartridge,
start switched off.

## Finding mods

**Find mods** browses the [Gen1Recomp community mod index](https://github.com/bryanthaboi/gen1recomp-mod-index): the
mods that fit the game, most downloaded first. Each shows its author, version, games, what it asks for (engine
internals, files, going online) and whether it's experimental.

**Install** downloads the mod's `.zip` from its author's own release and installs it as above. An installed mod shows
**Installed** or **Update**.

The index only lists mods. They're their authors' own code, and being listed isn't a check of them, so install only
what you trust.

## Mods that need other mods

A mod can name other mods it needs (`dependencies`) or can use (`optional_dependencies`) in its `manifest.json`.
After every install, the launcher checks them by Gen1Recomp's own rules, versions and games included. If one is
missing or the wrong version, **Needs other mods** lists it, and you choose where each comes from:
- the mod index;
- its GitHub releases (when the mod names a repo);
- a `.zip` you downloaded;
- a link to one.

Whatever you choose has to hold that mod, or nothing is installed. Anything that mod needs in turn is asked for next.
An optional one can be declined, and isn't asked about again. While anything is still missing, the game's Mods page
shows **Missing mods**.

## Mods that clash

A mod can name others it doesn't work with (`conflicts` or `incompatible`), optionally for some versions only. While
both are on, Gen1Recomp quietly skips the mod that said so, so the launcher tells you instead:
- The Mods page shows **Clashes** while any such pair is on together, and under each mod, who it clashes with ("Won't
  load: clashes with ..."). Choose which to keep, or leave both on.
- Switching on or installing a mod that clashes with one that's on asks first.
- In **Find mods**, a mod's page lists what it clashes with, and says if you have one of them.
