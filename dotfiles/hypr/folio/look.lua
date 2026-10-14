-- Aspetto: una gabbia ariosa, filetti da un pixel, ombre morbide e brevi.
local P = require("folio.palette")

hl.config({
    general = {
        gaps_in     = 7,
        gaps_out    = 14,
        border_size = 1,
        col = {
            active_border   = P.rgba(P.ink, "d9"),
            inactive_border = P.rgba(P.ink, "1f"),
        },
        -- Con il dito i bordi sono sottili: allarghiamo l'area di presa.
        resize_on_border        = true,
        extend_border_grab_area = 24,
        layout = "dwindle",
        snap = {
            enabled      = true,
            respect_gaps = true,
        },
    },

    decoration = {
        rounding       = 10,
        rounding_power = 2.4,
        active_opacity   = 1.0,
        inactive_opacity = 1.0,
        dim_inactive = false,
        dim_modal    = true,
        shadow = {
            enabled        = true,
            range          = 26,
            render_power   = 4,
            color          = "rgba(0000002e)",
            color_inactive = "rgba(00000014)",
        },
        blur = {
            enabled  = true,
            size     = 6,
            passes   = 3,
            noise    = 0.015,
            vibrancy = 0.12,
            popups   = true,
        },
    },

    animations = { enabled = true },

    dwindle = {
        preserve_split = true,
    },

    misc = {
        disable_hyprland_logo    = true,
        disable_splash_rendering = true,
        force_default_wallpaper  = 0,
        -- Visibile solo per un istante all'avvio: la Copertina di Folio lo copre.
        background_color         = P.rgb(P.paper),
        focus_on_activate        = true,
        animate_manual_resizes   = true,
        middle_click_paste       = false,
        allow_session_lock_restore = true,
    },

    cursor = {
        -- Tablet: il cursore sparisce appena si tocca lo schermo.
        hide_on_touch    = true,
        inactive_timeout = 4,
    },
})

-- Curve: uscita lunga e decisa, come lo scorrere di una pagina.
hl.curve("folio",  { type = "bezier", points = { {0.22, 1.0}, {0.36, 1.0} } })
hl.curve("linear", { type = "bezier", points = { {0, 0}, {1, 1} } })
hl.curve("paper",  { type = "spring", mass = 1, stiffness = 260, damping = 26 })

hl.animation({ leaf = "global",           enabled = true, speed = 8,   bezier = "folio" })
hl.animation({ leaf = "border",           enabled = true, speed = 5,   bezier = "folio" })
hl.animation({ leaf = "windows",          enabled = true, speed = 4.5, spring = "paper" })
hl.animation({ leaf = "windowsIn",        enabled = true, speed = 4,   spring = "paper",  style = "popin 94%" })
hl.animation({ leaf = "windowsOut",       enabled = true, speed = 1.8, bezier = "linear", style = "popin 94%" })
hl.animation({ leaf = "fade",             enabled = true, speed = 3,   bezier = "folio" })
hl.animation({ leaf = "layersIn",         enabled = true, speed = 3,   bezier = "folio",  style = "fade" })
hl.animation({ leaf = "layersOut",        enabled = true, speed = 2,   bezier = "linear", style = "fade" })
hl.animation({ leaf = "workspaces",       enabled = true, speed = 4.2, bezier = "folio",  style = "slide" })
hl.animation({ leaf = "specialWorkspace", enabled = true, speed = 4,   bezier = "folio",  style = "slidevert" })
