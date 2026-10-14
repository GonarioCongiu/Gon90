#!/usr/bin/env bash
# ════════════════════════════════════════════════════════════════════
#  FOLIO — installer per CachyOS / Arch Linux con Hyprland ≥ 0.55
#
#    ./installer.sh            controlli di compatibilità + installazione
#    ./installer.sh --check    solo controlli (non modifica nulla)
#    ./installer.sh --help     tutte le opzioni
# ════════════════════════════════════════════════════════════════════
set -Eeuo pipefail

FOLIO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FOLIO_VERSION="$(grep -m1 '^version' "$FOLIO_DIR/Cargo.toml" | cut -d'"' -f2)"
MIN_HYPR_MINOR=55          # configurazione Lua
MIN_GTK_MINOR=12           # gtk4 ≥ 4.12
MIN_RUST="1.85"
MIN_DISK_GB=4
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
OS_RELEASE="${FOLIO_OS_RELEASE:-/etc/os-release}"                  # sostituibili per i test
INPUT_DEVICES="${FOLIO_INPUT_DEVICES:-/proc/bus/input/devices}"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/folio"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$STATE_DIR/install-$STAMP.log"
HYPRBARS_REPO="https://github.com/hyprwm/hyprland-plugins"
HYPRGRASS_REPO="https://github.com/horriblename/hyprgrass"
WVKBD_REPOS=("https://git.sr.ht/~proycon/wvkbd" "https://github.com/jjsullivan5196/wvkbd")
FONT_BASE="https://raw.githubusercontent.com/google/fonts/main/ofl/instrumentserif"

MODE="install"
ASSUME_YES=0
FORCE=0
DO_PACKAGES=1
DO_PLUGINS=1
DO_DOTFILES=1
DO_FONTS=1
LINK=0
PURGE=0
PREFIX="/usr/local/bin"

usage() {
    cat <<EOF
Folio $FOLIO_VERSION — installer

USO
  ./installer.sh [opzioni]

MODALITÀ
  (nessuna)          controlli di compatibilità, poi installazione completa
  --check            esegue solo i controlli e termina (nessuna modifica)
  --uninstall        rimuove Folio e ripristina l'ultimo backup dei dotfile
  --purge            con --uninstall: elimina anche impaginazione e log

OPZIONI
  -y, --yes          non chiedere conferme (pacman e hyprpm in modalità non interattiva)
  --force            prosegue anche se un controllo bloccante fallisce
  --no-packages      non installa/aggiorna pacchetti
  --no-plugins       non installa hyprbars e hyprgrass con hyprpm
  --no-dotfiles      installa solo il binario
  --no-fonts         non scarica Instrument Serif
  --link             collega i dotfile con symlink (per chi sviluppa Folio)
  --user-bin         installa il binario in ~/.local/bin (senza sudo)
  --prefix DIR       cartella del binario (predefinita: /usr/local/bin)
  -h, --help         questo aiuto
EOF
}

while (($#)); do
    case "$1" in
        --check) MODE="check" ;;
        --uninstall) MODE="uninstall" ;;
        --purge) PURGE=1 ;;
        -y|--yes) ASSUME_YES=1 ;;
        --force) FORCE=1 ;;
        --no-packages) DO_PACKAGES=0 ;;
        --no-plugins) DO_PLUGINS=0 ;;
        --no-dotfiles) DO_DOTFILES=0 ;;
        --no-fonts) DO_FONTS=0 ;;
        --link) LINK=1 ;;
        --user-bin) PREFIX="$HOME/.local/bin" ;;
        --prefix) shift; PREFIX="${1:?--prefix richiede una cartella}" ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Opzione sconosciuta: $1" >&2; usage; exit 2 ;;
    esac
    shift
done

# ───────────────────────────── Output ─────────────────────────────

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    B=$'\e[1m'; D=$'\e[2m'; RED=$'\e[31m'; GRN=$'\e[32m'; YLW=$'\e[33m'; ACC=$'\e[38;2;212;68;42m'; N=$'\e[0m'
else
    B=""; D=""; RED=""; GRN=""; YLW=""; ACC=""; N=""
fi
FAILS=0
WARNS=0

banner() {
    printf '\n%s  F O L I O%s   %speriodico di ambiente desktop — n° %s%s\n' "$B" "$N" "$D" "$FOLIO_VERSION" "$N"
    printf '%s  ────────────────────────────────────────────────────────%s\n' "$ACC" "$N"
}
section() { printf '\n%s%s%s\n' "$B" "$1" "$N"; }
ok()   { printf '  %s✓%s %-30s %s\n' "$GRN" "$N" "$1" "${2:-}"; }
warn() { WARNS=$((WARNS + 1)); printf '  %s!%s %-30s %s\n' "$YLW" "$N" "$1" "${2:-}"; }
fail() { FAILS=$((FAILS + 1)); printf '  %s✗%s %-30s %s\n' "$RED" "$N" "$1" "${2:-}"; }
info() { printf '  %s·%s %s\n' "$D" "$N" "$1"; }
step() { printf '\n%s▸ %s%s\n' "$ACC" "$1" "$N"; }
die()  { printf '\n%s✗ %s%s\n' "$RED" "$1" "$N" >&2; exit 1; }

ask() {
    ((ASSUME_YES)) && return 0
    local reply
    read -r -p "  $1 [S/n] " reply </dev/tty || return 1
    [[ -z "$reply" || "$reply" =~ ^[sSyY] ]]
}

have() { command -v "$1" >/dev/null 2>&1; }

# vers_ge A B → vero se A ≥ B (confronto per versioni)
vers_ge() { [[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" == "$2" ]]; }

# Versione installata di un pacchetto (vuota se assente).
# (senza epoca "1:" e senza rilascio "-2", così sort -V confronta correttamente)
pkg_ver() { { pacman -Q "$1" 2>/dev/null || true; } | awk '{print $2}' | sed -E 's/^[0-9]+://; s/-[0-9.]+$//'; }
# Versione disponibile nei repository (vuota se il pacchetto non esiste nei repo).
repo_ver() { { LC_ALL=C pacman -Si "$1" 2>/dev/null || true; } | awk -F': ' '/^Version/{print $2; exit}' | sed -E 's/^[0-9]+://; s/-[0-9.]+$//'; }

# ───────────────────────────── Controlli ─────────────────────────────

NEED_PKGS=()       # pacchetti da installare/aggiornare
AUR_PKGS=()        # pacchetti non presenti nei repo ufficiali
BUILD_WVKBD=0
HAS_ACCEL=0
IN_HYPRLAND=0
[[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]] && IN_HYPRLAND=1

want_pkg() { local p; for p in "$@"; do [[ " ${NEED_PKGS[*]} " == *" $p "* ]] || NEED_PKGS+=("$p"); done; }

check_user() {
    if ((EUID == 0)); then
        fail "utente" "non eseguire come root: l'installer usa sudo solo quando serve"
    else
        ok "utente" "$USER"
    fi
}

check_os() {
    local id="" like="" name=""
    if [[ -r "$OS_RELEASE" ]]; then
        # shellcheck source=/dev/null
        . "$OS_RELEASE"
        id="${ID:-}"; like="${ID_LIKE:-}"; name="${PRETTY_NAME:-$id}"
    fi
    if [[ "$id" == "cachyos" ]]; then
        ok "distribuzione" "$name"
    elif [[ "$id" == "arch" || "$like" == *arch* ]]; then
        ok "distribuzione" "$name (derivata Arch: supportata, testata su CachyOS)"
    else
        fail "distribuzione" "$name: servono CachyOS o un'altra Arch con pacman"
    fi
}

check_arch() {
    local m; m="$(uname -m)"
    case "$m" in
        x86_64)
            local lvl=""
            for v in v4 v3 v2; do
                if /lib/ld-linux-x86-64.so.2 --help 2>/dev/null | grep "x86-64-$v (supported" >/dev/null; then lvl="x86-64-$v"; break; fi
            done
            ok "architettura" "x86_64${lvl:+ ($lvl)}" ;;
        aarch64) warn "architettura" "aarch64: non testata (Arch Linux ARM)" ;;
        *) fail "architettura" "$m non supportata" ;;
    esac
}

check_tools() {
    if have pacman; then ok "pacman" "$(pacman -V 2>/dev/null | grep -o 'Pacman v[0-9.]*' | head -n1)"; else fail "pacman" "assente"; fi
    if have sudo; then
        if sudo -n true 2>/dev/null; then ok "sudo" "credenziali già valide"; else ok "sudo" "verrà chiesta la password"; fi
    else
        fail "sudo" "assente: installa sudo e aggiungi l'utente a wheel"
    fi
    if have git; then ok "git" "$(git --version | awk '{print $3}')"; else want_pkg git; info "git verrà installato"; fi
    if have curl; then ok "curl" "$(curl --version | head -n1 | awk '{print $2}')"; else want_pkg curl; info "curl verrà installato"; fi
}

check_network() {
    if ! have curl; then warn "rete" "curl assente: controllo saltato"; return; fi
    if curl -fsS -m 8 -o /dev/null https://github.com 2>/dev/null; then
        ok "rete" "GitHub raggiungibile (crate, plugin, font)"
    elif [[ "$MODE" == "check" ]]; then
        warn "rete" "GitHub non raggiungibile: servirà per l'installazione"
    else
        fail "rete" "GitHub non raggiungibile: servono crate Rust, plugin e font"
    fi
}

check_disk() {
    local avail_kb gb
    avail_kb="$(df -Pk "$HOME" | awk 'NR==2{print $4}')"
    gb=$((avail_kb / 1024 / 1024))
    if ((gb >= MIN_DISK_GB)); then
        ok "spazio libero" "${gb} GB in $HOME"
    elif ((gb >= 2)); then
        warn "spazio libero" "${gb} GB: la compilazione ne usa circa ${MIN_DISK_GB}"
    else
        fail "spazio libero" "${gb} GB: ne servono almeno ${MIN_DISK_GB}"
    fi
}

check_ram() {
    local kb gb
    kb="$(awk '/MemTotal/{print $2}' /proc/meminfo)"
    gb=$(((kb + 524288) / 1024 / 1024))
    if ((gb >= 4)); then ok "memoria" "${gb} GB"; else warn "memoria" "${gb} GB: la compilazione sarà lenta"; fi
}

hypr_minor() { sed -E 's/^v?0\.([0-9]+).*/\1/' <<<"$1"; }

check_hyprland() {
    local inst repo run
    inst="$(pkg_ver hyprland)"
    repo="$(repo_ver hyprland)"
    if [[ -n "$inst" ]]; then
        if (($(hypr_minor "$inst") >= MIN_HYPR_MINOR)); then
            ok "Hyprland installato" "$inst (configurazione Lua)"
        elif [[ -n "$repo" ]] && (($(hypr_minor "$repo") >= MIN_HYPR_MINOR)); then
            warn "Hyprland installato" "$inst → sarà aggiornato a $repo (serve ≥ 0.$MIN_HYPR_MINOR)"
            want_pkg hyprland
        else
            fail "Hyprland installato" "$inst: serve ≥ 0.$MIN_HYPR_MINOR (Lua) e il repo offre ${repo:-?}"
        fi
    elif [[ -n "$repo" ]]; then
        if (($(hypr_minor "$repo") >= MIN_HYPR_MINOR)); then
            ok "Hyprland" "non installato: verrà installato $repo"
            want_pkg hyprland
        else
            fail "Hyprland" "il repo offre $repo: serve ≥ 0.$MIN_HYPR_MINOR"
        fi
    else
        fail "Hyprland" "non trovato né installato né nei repository"
    fi

    if ((IN_HYPRLAND)) && have hyprctl; then
        run="$(hyprctl version -j 2>/dev/null | sed -nE '/"tag"/{s/.*"tag": *"([^"]+)".*/\1/p;q}' || true)"
        if [[ -n "$run" ]]; then
            ok "sessione Hyprland attiva" "$run"
            if [[ -n "$inst" && "${run#v}" != "$inst"* ]]; then
                info "la sessione in corso ($run) differisce dal pacchetto ($inst): riavvia Hyprland dopo l'installazione"
            fi
        fi
    else
        info "non sei in una sessione Hyprland: va bene, tutto si attiva al prossimo accesso"
    fi
    for p in hyprlock hypridle hyprpolkitagent xdg-desktop-portal-hyprland; do
        if [[ -n "$(pkg_ver "$p")" ]]; then ok "$p" "$(pkg_ver "$p")"; else want_pkg "$p"; info "$p verrà installato"; fi
    done
    if have start-hyprland; then
        info "avvia la sessione con «start-hyprland» (o dal display manager)"
    fi
}

check_plugin_deps() {
    if ((DO_PLUGINS == 0)); then info "plugin saltati (--no-plugins)"; return; fi
    local missing=()
    for p in cmake meson ninja cpio pkgconf gcc glm; do
        [[ -n "$(pkg_ver "$p")" ]] || missing+=("$p")
    done
    if ((${#missing[@]})); then
        info "per compilare hyprbars/hyprgrass verranno installati: ${missing[*]}"
        want_pkg "${missing[@]}"
    else
        ok "dipendenze di hyprpm" "cmake meson ninja cpio pkgconf gcc glm"
    fi
    if ((IN_HYPRLAND)) && have hyprctl; then
        local list; list="$(hyprctl plugin list 2>/dev/null || true)"
        local plugin
        for plugin in hyprbars hyprgrass; do
            if [[ "$list" == *"$plugin"* ]]; then ok "$plugin" "già caricato"; else info "$plugin verrà installato con hyprpm"; fi
        done
    fi
}

check_gpu() {
    if ! have lspci; then info "lspci assente: controllo GPU saltato (pacchetto pciutils)"; return; fi
    local gpus; gpus="$(lspci -nnk 2>/dev/null | grep -EA3 'VGA|3D|Display' || true)"
    if grep -qi nvidia <<<"$gpus"; then
        if [[ "$(cat /sys/module/nvidia_drm/parameters/modeset 2>/dev/null)" == "Y" ]]; then
            ok "GPU" "NVIDIA con modeset attivo"
        elif lsmod 2>/dev/null | grep '^nvidia' >/dev/null; then
            warn "GPU" "NVIDIA senza nvidia_drm.modeset=1: aggiungilo alla riga di avvio del kernel"
        else
            warn "GPU" "NVIDIA: installa nvidia-open (CachyOS: chwd) per Hyprland"
        fi
    elif grep -qiE 'amd|ati' <<<"$gpus"; then ok "GPU" "AMD"
    elif grep -qi intel <<<"$gpus"; then ok "GPU" "Intel"
    else info "GPU non riconosciuta: Hyprland richiede OpenGL ES 3.x"
    fi
}

check_gtk() {
    local v r
    v="$(pkg_ver gtk4)"; r="$(repo_ver gtk4)"
    if [[ -n "$v" ]] && vers_ge "$v" "4.$MIN_GTK_MINOR"; then
        ok "GTK 4" "$v"
    elif [[ -n "$r" ]] && vers_ge "$r" "4.$MIN_GTK_MINOR"; then
        ok "GTK 4" "${v:-assente} → $r"
        want_pkg gtk4
    else
        fail "GTK 4" "serve ≥ 4.$MIN_GTK_MINOR (trovato ${v:-nessuno}, repo ${r:-?})"
    fi
    v="$(pkg_ver gtk4-layer-shell)"; r="$(repo_ver gtk4-layer-shell)"
    if [[ -n "$v" ]]; then ok "gtk4-layer-shell" "$v"
    elif [[ -n "$r" ]]; then ok "gtk4-layer-shell" "verrà installato $r"; want_pkg gtk4-layer-shell
    else fail "gtk4-layer-shell" "non disponibile nei repository"
    fi
    want_pkg glib2 pango cairo gdk-pixbuf2
}

check_rust() {
    local v=""
    if have cargo; then v="$(cargo --version 2>/dev/null | awk '{print $2}')"; fi
    if [[ -n "$v" ]] && vers_ge "$v" "$MIN_RUST"; then
        ok "Rust" "cargo $v"
    elif have rustup; then
        ok "Rust" "rustup presente: verrà aggiornato il toolchain stable (${v:-assente} → ≥ $MIN_RUST)"
    else
        local r; r="$(repo_ver rust)"
        if [[ -n "$r" ]] && vers_ge "$r" "$MIN_RUST"; then
            ok "Rust" "${v:-assente} → rust $r dai repository"
            want_pkg rust
        else
            fail "Rust" "serve ≥ $MIN_RUST (trovato ${v:-nessuno})"
        fi
    fi
    want_pkg base-devel pkgconf
}

# Touchscreen: dispositivi con INPUT_PROP_DIRECT e assi multitouch (ABS_MT_POSITION_X = bit 53).
check_touch() {
    local found=0 name="" prop=0 abs=""
    while IFS= read -r line; do
        case "$line" in
            N:\ Name=*) name="${line#N: Name=}"; name="${name//\"/}" ;;
            B:\ PROP=*) prop=$((16#${line#B: PROP=})) ;;
            B:\ ABS=*) abs="${line#B: ABS=}" ;;
            "")
                if ((prop & 2)) && [[ -n "$abs" ]] && [[ "${name,,}" != *pen* && "${name,,}" != *stylus* ]]; then
                    local low="${abs##* }" mt="singolo tocco"
                    (((16#$low >> 53) & 1)) && mt="multitouch"
                    ok "touchscreen" "$name ($mt)"
                    found=1
                fi
                name=""; prop=0; abs="" ;;
        esac
    done < <(cat "$INPUT_DEVICES" 2>/dev/null; echo)
    if ((found == 0)); then
        warn "touchscreen" "nessuno rilevato: Folio funziona anche con mouse e touchpad"
    fi
    if grep -qE '^B: SW=.*[2367abef]$' "$INPUT_DEVICES" 2>/dev/null; then
        info "interruttore modalità tablet presente (2-in-1)"
    fi
}

check_sensors() {
    if compgen -G "/sys/bus/iio/devices/iio:device*/in_accel_x_raw" >/dev/null; then
        HAS_ACCEL=1
        ok "accelerometro" "rotazione automatica possibile (iio-sensor-proxy + iio-hyprland)"
        want_pkg iio-sensor-proxy
    else
        info "nessun accelerometro: rotazione manuale dal Notiziario"
    fi
}

check_osk() {
    if have wvkbd-mobintl; then
        ok "tastiera a schermo" "wvkbd-mobintl"
    elif [[ -n "$(repo_ver wvkbd)" ]]; then
        ok "tastiera a schermo" "wvkbd verrà installato dai repository"; want_pkg wvkbd
    elif have paru || have yay; then
        ok "tastiera a schermo" "wvkbd verrà installato da AUR"; AUR_PKGS+=("wvkbd")
    else
        ok "tastiera a schermo" "wvkbd verrà compilato dai sorgenti"; BUILD_WVKBD=1
        want_pkg wayland wayland-protocols libxkbcommon pango cairo scdoc
    fi
}

check_conflicts() {
    local p running=()
    for p in waybar mako dunst swaync ags eww hyprpanel quickshell nwg-dock-hyprland nwg-panel; do
        pgrep -x "$p" >/dev/null 2>&1 && running+=("$p")
    done
    if ((${#running[@]})); then
        warn "componenti in esecuzione" "${running[*]}: Folio ha già testata, dock e notifiche"
        info "toglili dall'autostart per evitare doppioni (i vecchi file restano nel backup)"
    else
        ok "conflitti" "nessuna barra o demone di notifiche concorrente"
    fi
}

check_existing_config() {
    local h="$CONFIG_HOME/hypr"
    if [[ -f "$h/hyprland.lua" ]]; then
        warn "hyprland.lua esistente" "verrà salvato nel backup e sostituito"
    elif [[ -f "$h/hyprland.conf" ]]; then
        ok "hyprland.conf (hyprlang)" "resta dov'è: con hyprland.lua presente Hyprland usa il Lua"
    else
        ok "configurazione Hyprland" "nessuna (prima installazione)"
    fi
    [[ -n "$(pkg_ver cachyos-hyprland-settings)" ]] && info "cachyos-hyprland-settings installato: le sue impostazioni restano disponibili nel backup"
    [[ -f "$CONFIG_HOME/folio/config.toml" ]] && ok "config.toml di Folio" "esistente: non verrà toccato"
    return 0
}

check_fonts() {
    local fam
    for fam in "Inter" "JetBrains Mono" "Instrument Serif"; do
        # (niente `grep -q`: con pipefail il SIGPIPE a monte darebbe un falso negativo)
        if fc-list : family 2>/dev/null | tr ',' '\n' | grep -x "$fam" >/dev/null; then
            ok "carattere" "$fam"
        else
            info "carattere $fam verrà installato"
        fi
    done
    want_pkg inter-font ttf-jetbrains-mono ttf-ibm-plex noto-fonts papirus-icon-theme adw-gtk-theme
}

run_checks() {
    section "1 · Sistema"
    check_user; check_os; check_arch; check_tools; check_network; check_disk; check_ram
    section "2 · Hyprland"
    check_hyprland; check_plugin_deps
    section "3 · Grafica e toolkit"
    check_gpu; check_gtk; check_rust
    section "4 · Tablet e touch"
    check_touch; check_sensors; check_osk
    section "5 · Ambiente esistente"
    check_conflicts; check_existing_config; check_fonts
    # Strumenti usati da Folio a runtime.
    want_pkg grim slurp wl-clipboard brightnessctl playerctl networkmanager bluez bluez-utils \
        wireplumber pipewire pipewire-pulse libnotify kitty xdg-desktop-portal-gtk
}

# ───────────────────────────── Installazione ─────────────────────────────

install_packages() {
    step "Pacchetti"
    local repo=() p
    for p in "${NEED_PKGS[@]}"; do
        if pacman -Si "$p" >/dev/null 2>&1 || pacman -Sg "$p" >/dev/null 2>&1; then
            repo+=("$p")
        else
            AUR_PKGS+=("$p")
        fi
    done
    local flags=(--needed)
    ((ASSUME_YES)) && flags+=(--noconfirm)
    info "pacman: ${repo[*]}"
    # Aggiornamento completo: su Arch gli aggiornamenti parziali non sono supportati.
    sudo pacman -Syu "${flags[@]}" "${repo[@]}"

    if have rustup; then
        rustup toolchain install stable --profile minimal >/dev/null
        rustup default stable >/dev/null
    fi

    if ((${#AUR_PKGS[@]})); then
        local helper=""
        have paru && helper="paru"
        [[ -z "$helper" ]] && have yay && helper="yay"
        if [[ -n "$helper" ]]; then
            info "$helper (AUR): ${AUR_PKGS[*]}"
            "$helper" -S "${flags[@]}" "${AUR_PKGS[@]}" || warn "AUR" "alcuni pacchetti non sono stati installati: ${AUR_PKGS[*]}"
        else
            warn "AUR" "nessun helper (paru/yay): salto ${AUR_PKGS[*]}"
            [[ " ${AUR_PKGS[*]} " == *" wvkbd "* ]] && BUILD_WVKBD=1
        fi
    fi
    if ((HAS_ACCEL)); then
        if ! have iio-hyprland; then
            if have paru || have yay; then
                info "rotazione automatica: iio-hyprland-git (AUR)"
                local h="yay"; have paru && h="paru"
                "$h" -S "${flags[@]}" iio-hyprland-git || warn "iio-hyprland" "non installato: rotazione solo manuale"
            else
                info "per la rotazione automatica installa iio-hyprland-git da AUR"
            fi
        fi
    fi
}

build_wvkbd() {
    ((BUILD_WVKBD)) || return 0
    step "Tastiera a schermo (wvkbd dai sorgenti)"
    local tmp; tmp="$(mktemp -d)"
    local cloned=0 url
    for url in "${WVKBD_REPOS[@]}"; do
        if git clone -q --depth 1 "$url" "$tmp/wvkbd" 2>/dev/null; then cloned=1; break; fi
    done
    if ((cloned == 0)); then warn "wvkbd" "sorgenti non raggiungibili: tastiera a schermo non installata"; return 0; fi
    if make -C "$tmp/wvkbd" -j"$(nproc)" >/dev/null 2>&1 && [[ -x "$tmp/wvkbd/wvkbd-mobintl" ]]; then
        install -Dm755 "$tmp/wvkbd/wvkbd-mobintl" "$HOME/.local/bin/wvkbd-mobintl"
        ok "wvkbd" "installato in ~/.local/bin"
    else
        warn "wvkbd" "compilazione non riuscita: tastiera a schermo non installata"
    fi
    rm -rf "$tmp"
}

install_fonts() {
    ((DO_FONTS)) || return 0
    step "Caratteri"
    if fc-list : family 2>/dev/null | tr ',' '\n' | grep -x "Instrument Serif" >/dev/null; then
        ok "Instrument Serif" "già presente"
        return 0
    fi
    local dir="$HOME/.local/share/fonts/folio" f
    mkdir -p "$dir"
    for f in InstrumentSerif-Regular.ttf InstrumentSerif-Italic.ttf; do
        if ! curl -fsSL -m 30 -o "$dir/$f" "$FONT_BASE/$f"; then
            warn "Instrument Serif" "download non riuscito: ripiego su IBM Plex Serif"
            return 0
        fi
    done
    fc-cache -f "$dir" >/dev/null 2>&1 || true
    ok "Instrument Serif" "installato (SIL Open Font License)"
}

build_folio() {
    step "Compilazione di Folio"
    have cargo || die "cargo non trovato: installa Rust e rilancia"
    (cd "$FOLIO_DIR" && cargo build --release --locked)
    local bin="$FOLIO_DIR/target/release/folio"
    [[ -x "$bin" ]] || die "compilazione non riuscita"
    if [[ -w "$(dirname "$PREFIX")" || "$PREFIX" == "$HOME"* ]]; then
        install -Dm755 "$bin" "$PREFIX/folio"
    else
        sudo install -Dm755 "$bin" "$PREFIX/folio"
    fi
    ok "folio" "$PREFIX/folio ($("$PREFIX/folio" --version))"
    echo "$PREFIX/folio" >"$STATE_DIR/binary"
}

# Copia (o collega) un file dei dotfile, salvando prima l'eventuale originale.
BACKUP_DIR="$STATE_DIR/backup-$STAMP"
put() {
    local src="$FOLIO_DIR/dotfiles/$1" dst="$CONFIG_HOME/$1"
    if [[ -e "$dst" || -L "$dst" ]]; then
        mkdir -p "$(dirname "$BACKUP_DIR/$1")"
        cp -a "$dst" "$BACKUP_DIR/$1"
        rm -rf "$dst"
    fi
    mkdir -p "$(dirname "$dst")"
    if ((LINK)); then ln -s "$src" "$dst"; else cp -a "$src" "$dst"; fi
    echo "$1" >>"$STATE_DIR/manifest"
}

# Come `put`, ma non sovrascrive mai un file personale: installa accanto "*.folio".
put_once() {
    local src="$FOLIO_DIR/dotfiles/$1" dst="$CONFIG_HOME/$1"
    mkdir -p "$(dirname "$dst")"
    if [[ -e "$dst" ]]; then
        cp -a "$src" "$dst.folio"
        info "$1 esiste: il riferimento aggiornato è in $1.folio"
    else
        cp -a "$src" "$dst"
        echo "$1" >>"$STATE_DIR/manifest"
    fi
}

install_dotfiles() {
    ((DO_DOTFILES)) || return 0
    step "Dotfile"
    : >"$STATE_DIR/manifest"
    put hypr/hyprland.lua
    put hypr/folio
    put hypr/hyprlock.conf
    put hypr/hypridle.conf
    put gtk-3.0/gtk.css
    put gtk-3.0/settings.ini
    put gtk-4.0/gtk.css
    put gtk-4.0/settings.ini
    put_once folio/config.toml
    put_once folio/style.css
    put_once kitty/kitty.conf
    if [[ ! -f "$CONFIG_HOME/hypr/user.lua" ]]; then
        cat >"$CONFIG_HOME/hypr/user.lua" <<'EOF'
-- Le tue aggiunte a Hyprland: questo file non viene mai sovrascritto.
-- Esempi:
--   hl.monitor({ output = "eDP-1", mode = "preferred", position = "auto", scale = 1.5 })
--   hl.config({ input = { kb_layout = "us" } })
--   hl.bind("SUPER + B", hl.dsp.exec_cmd("firefox"))
EOF
    fi
    mkdir -p "$HOME/Documenti"
    [[ -f "$HOME/Documenti/note.md" ]] || printf '# Appunti\n- Il widget "Appunti" mostra le prime righe di questo file\n' >"$HOME/Documenti/note.md"
    if [[ -d "$BACKUP_DIR" ]]; then ok "backup" "$BACKUP_DIR"; fi
    ok "dotfile" "$( ((LINK)) && echo collegati || echo copiati ) in $CONFIG_HOME"
}

install_plugins() {
    ((DO_PLUGINS)) || return 0
    step "Plugin di Hyprland (hyprpm)"
    if ! have hyprpm; then warn "hyprpm" "assente: barre del titolo in modalità nativa, gesti non disponibili"; return 0; fi
    # hyprpm chiede conferma prima di aggiungere un repository: con --yes rispondiamo noi.
    # (`yes || true` evita che SIGPIPE faccia fallire la pipeline con pipefail)
    hpm_add() {
        if ((ASSUME_YES)); then (yes || true) | hyprpm add "$1"; else hyprpm add "$1"; fi
    }
    if ! hyprpm update; then
        warn "hyprpm update" "non riuscito: riprova da una sessione Hyprland con «hyprpm update»"
        return 0
    fi
    local list; list="$(hyprpm list 2>/dev/null || true)"
    if [[ "$list" != *hyprbars* ]]; then
        hpm_add "$HYPRBARS_REPO" || warn "hyprland-plugins" "aggiunta non riuscita"
    fi
    if hyprpm enable hyprbars; then
        ok "hyprbars" "abilitato (barre del titolo con popup)"
    else
        warn "hyprbars" "non abilitato: Folio userà la maniglia nativa"
    fi
    if [[ "$list" != *hyprgrass* ]]; then
        hpm_add "$HYPRGRASS_REPO" || warn "hyprgrass" "aggiunta non riuscita"
    fi
    if hyprpm enable hyprgrass; then
        ok "hyprgrass" "abilitato (gesti touch stile Android)"
    else
        warn "hyprgrass" "non abilitato: niente gesti dai bordi"
    fi

    # Gesti facoltativi di volume e luminosità lungo i bordi laterali.
    local ex="$CONFIG_HOME/hypr/plugins/hyprgrass/extras" f
    mkdir -p "$ex"
    for f in wpctl.lua brightnessctl.lua; do
        curl -fsSL -m 20 -o "$ex/$f" "https://raw.githubusercontent.com/horriblename/hyprgrass/main/extras/$f" 2>/dev/null \
            || rm -f "$ex/$f"
    done
    if ((IN_HYPRLAND)); then hyprpm reload -n >/dev/null 2>&1 || true; fi
}

enable_services() {
    step "Servizi"
    local s
    for s in NetworkManager bluetooth; do
        if systemctl list-unit-files "$s.service" >/dev/null 2>&1; then
            if systemctl is-enabled "$s" >/dev/null 2>&1; then
                ok "$s" "già attivo"
            elif ask "Abilitare $s all'avvio?"; then
                sudo systemctl enable --now "$s" && ok "$s" "abilitato"
            fi
        fi
    done
}

finish() {
    section "Fatto."
    cat <<EOF

  ${B}Gesti (touchscreen)${N}
    dal basso, corto ............ Dock
    dal basso, lungo ............ Indice (home)
    dal basso, lungo + pausa .... Sommario (recenti, spazi di lavoro)
    lungo il bordo inferiore .... spazio precedente / successivo
    dai lati verso l'interno .... Indietro
    dall'alto ................... Notiziario (notifiche + impostazioni rapide)
    tre dita ↑ / ↓ / tocco ...... Sommario / riduci / comandi finestra
    tieni premuta la Testata o la Dock → Impaginazione (sposta tutto ovunque)

  ${B}Prossimi passi${N}
    1. Esci dalla sessione e rientra in Hyprland (il passaggio a Lua richiede un riavvio).
    2. Controlla l'ambiente:  folio doctor
    3. Personalizza:          $CONFIG_HOME/folio/config.toml  →  folio msg reload

  Log completo: $LOG
EOF
    if ((IN_HYPRLAND)) && have folio; then
        printf '\n'
        folio doctor || true
    fi
}

# ───────────────────────────── Disinstallazione ─────────────────────────────

uninstall() {
    banner
    step "Disinstallazione"
    if have folio; then folio msg quit >/dev/null 2>&1 || true; fi
    local bin; bin="$(cat "$STATE_DIR/binary" 2>/dev/null || echo "$PREFIX/folio")"
    if [[ -f "$bin" ]]; then
        if [[ -w "$(dirname "$bin")" ]]; then rm -f "$bin"; else sudo rm -f "$bin"; fi
        ok "binario" "rimosso ($bin)"
    fi
    if [[ -f "$STATE_DIR/manifest" ]]; then
        local f
        while IFS= read -r f; do
            [[ -n "$f" ]] && rm -rf "${CONFIG_HOME:?}/$f"
        done <"$STATE_DIR/manifest"
        ok "dotfile" "rimossi"
    fi
    local backups=("$STATE_DIR"/backup-*) last=""
    [[ -d "${backups[-1]}" ]] && last="${backups[-1]}"
    if [[ -n "$last" ]]; then
        cp -a "$last/." "$CONFIG_HOME/"
        ok "backup ripristinato" "$last"
    fi
    if have hyprpm && ask "Disabilitare hyprbars e hyprgrass?"; then
        hyprpm disable hyprbars >/dev/null 2>&1 || true
        hyprpm disable hyprgrass >/dev/null 2>&1 || true
        ok "plugin" "disabilitati"
    fi
    if ((PURGE)); then
        rm -rf "$STATE_DIR" "$HOME/.local/share/fonts/folio"
        ok "dati" "impaginazione, log e caratteri rimossi"
    else
        info "impaginazione e backup restano in $STATE_DIR (--purge per rimuoverli)"
    fi
    printf '\n  Arrivederci.\n\n'
}

# ───────────────────────────── Main ─────────────────────────────

mkdir -p "$STATE_DIR"
if [[ "$MODE" != "check" ]]; then
    exec > >(tee -a "$LOG") 2>&1
fi
trap 'printf "\n%s✗ errore alla riga %s: %s%s\n  log: %s\n" "$RED" "$LINENO" "$BASH_COMMAND" "$N" "$LOG" >&2' ERR

if [[ "$MODE" == "uninstall" ]]; then
    uninstall
    exit 0
fi

banner
printf '  %sControlli di compatibilità%s\n' "$D" "$N"
run_checks

section "Esito"
if ((FAILS > 0)); then
    printf '  %s%d problemi bloccanti%s, %d avvisi.\n' "$RED" "$FAILS" "$N" "$WARNS"
else
    printf '  %sCompatibile.%s %d avvisi non bloccanti.\n' "$GRN" "$N" "$WARNS"
fi

if [[ "$MODE" == "check" ]]; then
    ((FAILS == 0)) || exit 1
    exit 0
fi
if ((FAILS > 0 && FORCE == 0)); then
    die "Installazione interrotta. Risolvi i punti segnati con ✗ (o usa --force a tuo rischio)."
fi

section "Piano"
((DO_PACKAGES)) && info "pacchetti: ${NEED_PKGS[*]}${AUR_PKGS[*]:+ · AUR: ${AUR_PKGS[*]}}"
((DO_FONTS)) && info "carattere Instrument Serif da Google Fonts"
info "compilazione di Folio e installazione in $PREFIX"
((DO_DOTFILES)) && info "dotfile in $CONFIG_HOME (backup in $BACKUP_DIR)"
((DO_PLUGINS)) && info "hyprbars + hyprgrass con hyprpm"
ask "Procedere?" || die "Annullato."

((DO_PACKAGES)) && install_packages
build_wvkbd
install_fonts
build_folio
install_dotfiles
install_plugins
((DO_PACKAGES)) && enable_services
finish
