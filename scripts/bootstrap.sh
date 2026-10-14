#!/usr/bin/env bash
# ════════════════════════════════════════════════════════════════════
#  FOLIO — scarica (o aggiorna) il repository e avvia l'installer.
#
#    curl -fsSL https://raw.githubusercontent.com/GonarioCongiu/Gon90/main/scripts/bootstrap.sh | bash
#    curl -fsSL …/bootstrap.sh | bash -s -- --check      # solo controlli
#
#  Le opzioni dopo `--` vengono passate a installer.sh.
# ════════════════════════════════════════════════════════════════════
set -Eeuo pipefail

REPO="${FOLIO_REPO:-https://github.com/GonarioCongiu/Gon90.git}"
BRANCH="${FOLIO_BRANCH:-main}"
DEST="${FOLIO_SRC:-$HOME/.local/src/folio}"

if ! command -v git >/dev/null 2>&1; then
    echo "· git non trovato: lo installo con pacman"
    sudo pacman -S --needed --noconfirm git
fi

if [[ -d "$DEST/.git" ]]; then
    echo "· aggiorno $DEST"
    git -C "$DEST" fetch --quiet origin "$BRANCH"
    git -C "$DEST" checkout --quiet "$BRANCH"
    git -C "$DEST" reset --quiet --hard "origin/$BRANCH"
else
    echo "· scarico Folio in $DEST"
    mkdir -p "$(dirname "$DEST")"
    git clone --quiet --branch "$BRANCH" "$REPO" "$DEST"
fi

cd "$DEST"
# Con `curl | bash` lo stdin è la pipe: pacman, hyprpm e le conferme
# devono leggere dal terminale.
if [[ -t 0 ]]; then
    exec ./installer.sh "$@"
else
    exec ./installer.sh "$@" </dev/tty
fi
