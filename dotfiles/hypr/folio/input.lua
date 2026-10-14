-- Ingresso: pensato prima per il dito, poi per tastiera e touchpad.
hl.config({
    input = {
        kb_layout   = "it",
        follow_mouse = 1,
        sensitivity = 0,
        touchpad = {
            natural_scroll = true,
            tap_to_click   = true,
        },
        touchdevice = {
            enabled = true,
            -- transform = 1,   -- da allineare a `transform` del monitor se ruotato
        },
    },

    -- Swipe dal bordo del touchscreen per scorrere gli spazi (gesto nativo).
    gestures = {
        workspace_swipe_touch        = true,
        workspace_swipe_cancel_ratio = 0.15,
        workspace_swipe_create_new   = true,
        workspace_swipe_distance     = 700,
    },
})

-- Touchpad (2-in-1 con cover-tastiera): gli stessi gesti dei tablet.
hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
hl.gesture({ fingers = 3, direction = "up",   action = function() hl.exec_cmd("folio msg overview show") end })
hl.gesture({ fingers = 3, direction = "down", action = function() hl.exec_cmd("folio msg minimize") end })
hl.gesture({ fingers = 4, direction = "up",   action = function() hl.exec_cmd("folio msg home show") end })
