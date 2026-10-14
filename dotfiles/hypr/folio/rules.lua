-- Regole per finestre e superfici.

-- Le app non decidono da sole di massimizzarsi: lo fa il dito, dal popup.
hl.window_rule({
    name  = "folio-no-self-maximize",
    match = { class = ".*" },
    suppress_event = "maximize",
})

-- Dialoghi: fluttuanti e centrati, come un riquadro a margine.
hl.window_rule({
    name   = "folio-dialogs",
    match  = { modal = true },
    float  = true,
    center = true,
})
hl.window_rule({
    name   = "folio-pickers",
    match  = { title = [[^(Open File|Save File|Save As|Apri file|Salva file|Salva come|Seleziona.*|Select a File|Choose Files|File Upload|Open Folder)$]] },
    float  = true,
    center = true,
})
hl.window_rule({
    name   = "folio-utilities",
    match  = { class = [[^(pavucontrol|org\.pulseaudio\.pavucontrol|nm-connection-editor|blueman-manager|org\.gnome\.Calculator|xdg-desktop-portal-gtk|hyprland-share-picker|polkit-gnome-authentication-agent-1|hyprpolkitagent)$]] },
    float  = true,
    center = true,
})

-- Picture-in-picture: fissata su tutti gli spazi, proporzioni bloccate.
hl.window_rule({
    name  = "folio-pip",
    match = { title = [[^(Picture-in-Picture|Picture in picture|Picture-in-picture)$]] },
    float = true,
    pin   = true,
    keep_aspect_ratio = true,
})

-- XWayland: evita problemi di trascinamento (dall'esempio ufficiale).
hl.window_rule({
    name  = "folio-xwayland-drags",
    match = { class = "^$", title = "^$", xwayland = true, float = true, fullscreen = false, pin = false },
    no_focus = true,
})

-- ── Superfici di Folio ──────────────────────────────────────────────
-- Vetro smerigliato leggero dietro le superfici semitrasparenti.
hl.layer_rule({
    name  = "folio-blur",
    match = { namespace = "^folio-(home|overview|shade|chrome|dock|toast|layout|bar)$" },
    blur  = true,
    ignore_alpha = 0.3,
})
-- Copertina e widget della scrivania non devono animarsi a ogni avvio.
hl.layer_rule({
    name    = "folio-static",
    match   = { namespace = "^folio-(cover|desk|handle)$" },
    no_anim = true,
})
-- Movimento delle superfici, come su un tablet: il Notiziario scende dall'alto,
-- l'Indice sale dal basso (il cassetto delle app di Android), il resto sfuma.
hl.layer_rule({ name = "folio-shade-anim", match = { namespace = "^folio-shade$" }, animation = "slide top" })
hl.layer_rule({ name = "folio-home-anim", match = { namespace = "^folio-home$" }, animation = "slide bottom" })
hl.layer_rule({ name = "folio-sheets-anim", match = { namespace = "^folio-(overview|layout|chrome)$" }, animation = "fade" })
-- Le anteprime del Sommario non devono finire nelle condivisioni dello schermo.
hl.layer_rule({ name = "folio-private", match = { namespace = "^folio-(toast|shade)$" }, no_screen_share = true })

-- Tastiera a schermo utilizzabile anche sulla schermata di blocco.
hl.layer_rule({ name = "folio-osk-lock", match = { namespace = "^wvkbd$" }, above_lock = 2 })

-- Lo spazio speciale delle finestre ridotte non oscura il resto dello schermo.
hl.config({ decoration = { dim_special = 0.0 } })

-- grim deve poter catturare lo schermo per le anteprime del Sommario
-- (necessario solo se attivi `ecosystem.enforce_permissions`).
hl.permission("/usr/(bin|local/bin)/grim", "screencopy", "allow")
hl.permission("/usr/(bin|local/bin)/hyprpm", "plugin", "allow")
