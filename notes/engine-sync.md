# Engine sync

`.github/workflows/engine-sync.yml` runs `.github/engine-sync/sync.sh` every night (and from the Actions tab). It brings upstream engine updates into the forks, moves nativeMons' pins on `dev`, and that push builds a new bleeding-edge release. Anything that isn't clean stops that engine and opens an issue in nativeMons titled "Engine sync: ...". The same problem never gets a second issue while the first is open.

It runs as the sync GitHub App, so its commits show as the App's bot, with the subject "Engine auto-sync with upstream". The bleeding-edge release built from one says so in its title and lists what moved. Its ID is the `ENGINE_SYNC_APP_ID` variable and its private key is the `ENGINE_SYNC_APP_KEY` secret.

## Forks

| Fork | Liaison branch | Upstream | Merged into hails-additions |
| --- | --- | --- | --- |
| Hailey-Ross/gen1recomp | `dev` | bryanthaboi/gen1recomp | yes |
| Hailey-Ross/pokemon-fulldex | `main` | mamesj/pokemon-fulldex | yes (`--rom`) |
| Hailey-Ross/gen4recomp | `main` | sohamb117/gen4recomp | yes |

1. The liaison branch fast-forwards to upstream. If it can't (it has commits of its own), the run stops for that fork.
2. The liaison branch is merged into `hails-additions`. A conflict stops the run, with one exception: in Gen1Recomp, upstream changing a file we removed for the licence keeps it removed (the paths in `tools/nativemons/check-removed.sh`).
3. A merge that changes a licence file, or brings back a removed launcher file, stops the run and isn't pushed.

## Pins

- **Gen1Recomp and Fulldex:** submodules, moved to the fork's new head.
- **gen4recomp:** pushing `hails-additions` builds a `windows-<sha9>` release. The run waits for that build (up to two and a half hours), then sets the tag in both `launcher/build-all.ps1` and `.github/actions/build-package/action.yml`. A push that only changes docs starts no build, and the pin stays.
- **Pinned releases:** `.github/engine-sync/pins.txt` records each asset's sha256. When a pinned Fulldex release (dated tags such as `fulldex-2026-11-05b`, earlier `pc-v1`) is re-uploaded with new files, the record changes and the push rebuilds with it. A new Fulldex tag, or any Platinum port change, only opens an issue: those get tested by hand first, and a new port needs its save padding checked.

## Testing it

`DRY_RUN=1` pushes nothing and opens no issues. Run it in a throwaway clone of nativeMons on `dev`, because it edits the checkout:

```
GH_TOKEN=$(gh auth token) DRY_RUN=1 BOT_NAME=test BOT_EMAIL=test@example.invalid bash .github/engine-sync/sync.sh
```
