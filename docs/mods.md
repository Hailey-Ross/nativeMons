# Mods

[Back to the README](../README.md)

Installing, finding and managing mods for Gen1Recomp games.

**Mods** on the home screen, or on a Gen1Recomp game's page, installs and turns on mods for Gen1Recomp games. Choose a mod's `.zip` from any drive; installing a newer version over an old one keeps the old copy until the new one is in place. Mods only apply to games played on Gen1Recomp.

**Find mods** browses the [Gen1Recomp community mod index](https://github.com/bryanthaboi/gen1recomp-mod-index): the mods that fit the game, most downloaded first, each with its author, version, games, what it asks for (engine internals, files, going online) and whether it's experimental. **Install** downloads its release `.zip` from the author's own release and installs it as above; an installed mod shows **Installed** or **Update**. The index only lists mods: they're their authors' own code, and being listed isn't a check of them, so install only what you trust. A mod you install is switched on in each game it runs, unless it clashes there with one that's already on: then it stays off, and you choose. The examples that come with Gen1Recomp and need art imported from your own cartridge start switched off.

**Mods that need other mods.** A mod can name others it needs (`dependencies`) or can use (`optional_dependencies`) in its `manifest.json`. After any install, the launcher reads them by Gen1Recomp's own rules, versions and games included, and if one is missing or the wrong version, **Needs other mods** lists it. For each, you choose where it comes from: the mod index, its GitHub releases (when the mod names a repo), a `.zip` you downloaded, or a link to one. Whatever you choose has to hold that mod, or nothing is installed, and what it needs in turn is asked for next. An optional one can be declined, and isn't asked again. While anything is still missing, the game's Mods page shows **Missing mods**.

**Mods that clash.** A mod can name others it doesn't work with (`conflicts`, or `incompatible`), optionally for some versions only. While both are on, Gen1Recomp skips the mod that said so, without a word in the game, so the launcher says it instead: the Mods page shows **Clashes** while any pair is on together, and under each mod who it clashes with ("Won't load: clashes with ..."). Choose which to keep, or leave both on. Switching on a mod that clashes with one that's on asks first, and so does installing one. In **Find mods**, a mod's page lists what it clashes with, and says if you have one of them.
