#!/usr/bin/env bash
# Pulls upstream engine updates into the forks and moves nativeMons' pins on dev (notes/engine-sync.md).
# Runs in a nativeMons checkout of dev. Needs GH_TOKEN (the sync App's token) and BOT_NAME/BOT_EMAIL.
# DRY_RUN=1 pushes nothing and opens no issues: run it in a throwaway clone, as it edits the checkout.
set -euo pipefail

OWNER=Hailey-Ross
MAIN=$OWNER/nativeMons
REPO_DIR=$(pwd)
WORK=${WORK:-$(mktemp -d)}; mkdir -p "$WORK"
DRY=${DRY_RUN:-0}
CHANGES=()
PROBLEMS=0

say() { echo "::notice::$*" 2>/dev/null || echo "$*"; }
run() { if [ "$DRY" = 1 ]; then echo "[dry run] $*"; else "$@"; fi; }
url() { echo "https://x-access-token:${GH_TOKEN}@github.com/$1.git"; }

# One open issue per problem: a second night with the same problem doesn't open another.
problem() {
  local title="Engine sync: $1" body=$2
  PROBLEMS=$((PROBLEMS + 1))
  echo "PROBLEM: $title"; echo "$body"
  [ "$DRY" = 1 ] && return 0
  if [ -z "$(gh issue list -R "$MAIN" --state open --search "in:title \"$title\"" --json number --jq '.[].number')" ]; then
    gh issue create -R "$MAIN" --title "$title" --body "$body" >/dev/null
  fi
}

# Upstream commits onto the liaison branch (fast-forward only), then the liaison into hails-additions (clean only).
# Writes the head the nativeMons pin should follow to $WORK/<name>.head, or nothing when the fork can't move.
sync_fork() {
  local name=$1 fork=$2 upstream=$3 liaison=$4 additions=$5
  local dir=$WORK/$name
  rm -rf "$dir"; : > "$WORK/$name.head"
  git clone -q --filter=blob:none --no-checkout "$(url "$fork")" "$dir"
  git -C "$dir" remote add upstream "https://github.com/$upstream.git"
  git -C "$dir" fetch -q upstream "$liaison"
  local ours theirs
  ours=$(git -C "$dir" rev-parse "origin/$liaison"); theirs=$(git -C "$dir" rev-parse "upstream/$liaison")
  if [ "$ours" != "$theirs" ]; then
    if ! git -C "$dir" merge-base --is-ancestor "$ours" "$theirs"; then
      problem "$name $liaison can't fast-forward" "\`$fork\` \`$liaison\` has commits upstream \`$upstream\` doesn't. The liaison branch only fast-forwards, so this needs a look by hand."
      return 0
    fi
    local count; count=$(git -C "$dir" rev-list --count "$ours..$theirs")
    run git -C "$dir" push -q origin "$theirs:refs/heads/$liaison"
    CHANGES+=("$name: $count upstream commit(s) onto $liaison (${theirs:0:9})")
  fi
  if [ "$additions" != 1 ]; then echo "$theirs" > "$WORK/$name.head"; return 0; fi

  git -C "$dir" checkout -q -B hails-additions origin/hails-additions
  if git -C "$dir" merge-base --is-ancestor "$theirs" HEAD; then git -C "$dir" rev-parse HEAD > "$WORK/$name.head"; return 0; fi
  git -C "$dir" -c user.name="$BOT_NAME" -c user.email="$BOT_EMAIL" \
    merge -q --no-ff --no-edit -m "Merge upstream $liaison (${theirs:0:9})" "$theirs" >/dev/null 2>&1 || true
  if [ -f "$dir/.git/MERGE_HEAD" ]; then
    # Gen1Recomp only: upstream changing a file we removed for the licence keeps it removed. Anything else stops.
    local left=""
    while IFS= read -r line; do
      local code=${line:0:2} path=${line:3}
      if [ "$name" = Gen1Recomp ] && [ "$code" = "DU" ] && removed_path "$path"; then
        git -C "$dir" rm -q -- "$path"
      else
        left+="$code $path"$'\n'
      fi
    done < <(git -C "$dir" status --porcelain | grep -E '^(DD|AU|UD|UA|DU|AA|UU) ')
    if [ -n "$left" ]; then
      git -C "$dir" merge --abort
      problem "$name merge has conflicts" "Merging \`$liaison\` (${theirs:0:9}) into \`hails-additions\` in \`$fork\` conflicts on:"$'\n```\n'"$left"'```'$'\n'"Nothing was pushed. Merge by hand (skill upstream-sync)."
      return 0
    fi
    git -C "$dir" -c user.name="$BOT_NAME" -c user.email="$BOT_EMAIL" commit -q --no-edit
  fi
  if ! git -C "$dir" merge-base --is-ancestor "$theirs" HEAD; then
    problem "$name merge failed" "Merging \`$liaison\` (${theirs:0:9}) into \`hails-additions\` in \`$fork\` didn't complete. Nothing was pushed."
    return 0
  fi
  # A licence change, or a removed file back, always needs a person.
  local touched; touched=$(git -C "$dir" diff --name-only origin/hails-additions HEAD)
  if echo "$touched" | grep -qiE '(^|/)licen[cs]e'; then
    problem "$name licence changed upstream" "Upstream changed a licence file in \`$fork\`: re-read it (skill licence-and-credits) and merge by hand."$'\n```\n'"$(echo "$touched" | grep -iE 'licen[cs]e')"$'\n```'
    return 0
  fi
  if [ "$name" = Gen1Recomp ]; then
    if ! out=$(sh "$dir/tools/nativemons/check-removed.sh" 2>&1); then
      problem "$name merge brings back removed launcher files" "$out"
      return 0
    fi
  fi
  run git -C "$dir" push -q origin HEAD:refs/heads/hails-additions
  CHANGES+=("$name: merged into hails-additions ($(git -C "$dir" rev-parse --short=9 HEAD))")
  git -C "$dir" rev-parse HEAD > "$WORK/$name.head"
}

# The paths tools/nativemons/check-removed.sh keeps out of Gen1Recomp.
removed_path() {
  case $1 in
    src/import/LauncherView.lua|src/import/LauncherSettings.lua|src/import/OnlinePanel.lua|src/import/CartLabelArt.lua|\
    src/import/CartShape.lua|src/import/RomImporter.lua|src/mods/LauncherMods.lua|src/import/online/*|assets/launcher/*|assets/labels/*) return 0 ;;
  esac
  return 1
}

pin_submodule() {
  local name=$1 path=$2 sha=$3
  [ -z "$sha" ] && return 0
  local now; now=$(git -C "$REPO_DIR" ls-tree HEAD "$path" | awk '{print $3}')
  [ "$now" = "$sha" ] && return 0
  git -C "$REPO_DIR" update-index --cacheinfo "160000,$sha,$path"
  CHANGES+=("$name pin: ${now:0:9} -> ${sha:0:9}")
}

# gen4recomp publishes windows-<sha9> from each push to hails-additions; nativeMons downloads that release.
pin_gen4recomp() {
  local head=$1 waited=0
  [ -z "$head" ] && return 0
  local tag="windows-${head:0:9}" now
  now=$(grep -oE "windows-[0-9a-f]+" "$REPO_DIR/launcher/build-all.ps1" | head -1)
  [ "$now" = "$tag" ] && return 0
  if [ "$DRY" != 1 ]; then
    # Its build takes up to two hours. A push it ignores (docs only) starts no run, and the pin stays.
    while ! gh release view "$tag" -R "$OWNER/gen4recomp" >/dev/null 2>&1; do
      local state; state=$(gh run list -R "$OWNER/gen4recomp" --workflow windows-release.yml --commit "$head" --json status,conclusion --jq '.[0] | "\(.status) \(.conclusion)"' 2>/dev/null || true)
      if [ -z "$state" ] && [ $waited -ge 300 ]; then say "gen4recomp: no build for ${head:0:9}, pin stays $now"; return 0; fi
      case $state in "completed failure"*|"completed cancelled"*) problem "gen4recomp build failed" "The Windows release build of \`hails-additions\` ${head:0:9} didn't finish. The pin stays \`$now\`."; return 0 ;; esac
      [ $waited -ge 9000 ] && { problem "gen4recomp build took too long" "No release \`$tag\` after 2.5 hours. The pin stays \`$now\`."; return 0; }
      sleep 60; waited=$((waited + 60))
    done
  fi
  sed -i "s/$now/$tag/g" "$REPO_DIR/launcher/build-all.ps1" "$REPO_DIR/.github/actions/build-package/action.yml"
  CHANGES+=("gen4recomp pin: $now -> $tag")
}

# Pinned releases: a re-upload under the same tag just needs a rebuild; a new tag needs testing by hand first.
check_releases() {
  local pins=$REPO_DIR/.github/engine-sync/pins.txt
  local fulldex_tag platinum_tag digest newest
  fulldex_tag=$(grep -oP "fulldexRelease = '\K[^']+" "$REPO_DIR/launcher/build-all.ps1")
  platinum_tag=$(grep -oP "platinumRelease = '\K[^']+" "$REPO_DIR/launcher/build-all.ps1")

  newest=$(gh api "repos/mamesj/pokemon-fulldex/releases?per_page=20" --jq '[.[].tag_name | select(test("^(pc|fulldex)-"))][0]')
  [ -n "$newest" ] && [ "$newest" != "$fulldex_tag" ] && problem "Fulldex $newest released" "We pin \`$fulldex_tag\`. Test \`$newest\` on copies (skill engine-updates), then move the pin in build-all.ps1 and action.yml."
  digest=$(gh api "repos/mamesj/pokemon-fulldex/releases/tags/$fulldex_tag" --jq '.assets[] | select(.name == "pokemon-ds-wide-pc-windows-x86_64.zip") | .digest')
  if [ -n "$digest" ] && ! grep -qx "Fulldex $digest" "$pins"; then
    sed -i "s|^Fulldex .*|Fulldex $digest|" "$pins"
    CHANGES+=("Fulldex $fulldex_tag re-uploaded upstream: rebuilding with it")
  fi

  newest=$(gh api "repos/cybervisi0n/pokeplatinum/releases?per_page=20" --jq '[.[].tag_name | select(startswith("v"))][0]')
  [ -n "$newest" ] && [ "$newest" != "$platinum_tag" ] && problem "Platinum port $newest released" "We pin \`$platinum_tag\`. A new port needs its save padding checked against PlatinumSave before the pin moves (skill upstream-sync)."
  digest=$(gh api "repos/cybervisi0n/pokeplatinum/releases/tags/$platinum_tag" --jq '[.assets[] | select(.name | test("win")) | .digest][0]')
  [ -n "$digest" ] && ! grep -qx "Platinum $digest" "$pins" && problem "Platinum port $platinum_tag re-uploaded" "Its files changed under the same tag. Check the save padding, then update .github/engine-sync/pins.txt."
  return 0
}

sync_fork Gen1Recomp "$OWNER/gen1recomp" bryanthaboi/gen1recomp dev 1
sync_fork Fulldex "$OWNER/pokemon-fulldex" mamesj/pokemon-fulldex main 1
sync_fork gen4recomp "$OWNER/gen4recomp" sohamb117/gen4recomp main 1

pin_submodule Gen1Recomp engines/gen1recomp "$(cat "$WORK/Gen1Recomp.head")"
pin_submodule Fulldex engines/fulldex "$(cat "$WORK/Fulldex.head")"
pin_gen4recomp "$(cat "$WORK/gen4recomp.head")"
check_releases

if [ ${#CHANGES[@]} -eq 0 ]; then
  say "Nothing new."
else
  printf '%s\n' "${CHANGES[@]}"
  if [ "$DRY" = 1 ]; then
    git -C "$REPO_DIR" diff HEAD --stat
  else
    git -C "$REPO_DIR" add launcher/build-all.ps1 .github/actions/build-package/action.yml .github/engine-sync/pins.txt
    # A fork's liaison branch can move with no pin change here: nothing to commit then.
    if ! git -C "$REPO_DIR" diff --cached --quiet; then
      msg="Engine auto-sync with upstream"$'\n\n'"$(printf -- '- %s\n' "${CHANGES[@]}")"
      git -C "$REPO_DIR" -c user.name="$BOT_NAME" -c user.email="$BOT_EMAIL" commit -q -m "$msg"
      git -C "$REPO_DIR" push -q origin HEAD:refs/heads/dev
    fi
  fi
fi
[ $PROBLEMS -eq 0 ] || say "$PROBLEMS problem(s) need a look: see the issues."
