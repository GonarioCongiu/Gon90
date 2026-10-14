-- ════════════════════════════════════════════════════════════════════
--  Plugin: hyprbars (barre del titolo) e hyprgrass (gesti touch).
--  Si installano con `hyprpm` (l'installer lo fa per te) e vengono
--  caricati da `hyprpm reload` in autostart.lua; finché non sono caricati
--  le sezioni qui sotto vengono semplicemente saltate.
-- ════════════════════════════════════════════════════════════════════
local P = require("folio.palette")

local function folio(cmd)
    hl.exec_cmd("folio msg " .. cmd)
end

-- ── Barre del titolo ────────────────────────────────────────────────
-- Ogni finestra (anche i dialoghi) ha una testatina sottile con il titolo
-- e un solo pulsante "⋯": toccandolo, Folio apre il popup con i comandi
-- (chiudi, riduci, ingrandisci, schermo intero, fluttuante, affianca,
-- sposta in un altro spazio). Doppio tocco sulla barra = ingrandisci.
if hl.plugin and hl.plugin.hyprbars then
    hl.config({
        plugin = {
            hyprbars = {
                enabled                    = true,
                bar_height                 = 34,
                bar_color                  = P.rgb(P.paper),
                ["col.text"]               = P.rgb(P.ink),
                bar_text_font              = "Instrument Serif",
                bar_text_size              = 14,
                bar_text_align             = "left",
                bar_padding                = 16,
                bar_button_padding         = 12,
                bar_buttons_alignment      = "right",
                bar_part_of_window         = true,
                bar_precedence_over_border = true,
                on_double_click            = [[hyprctl dispatch 'hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })']],
            },
        },
    })
    hl.plugin.hyprbars.add_button({
        bg_color = "rgba(00000000)",
        fg_color = P.rgb(P.ink),
        size     = 22,
        icon     = "⋯",
        action   = "folio msg chrome toggle",
    })
    -- Niente barra sulle finestre "picture in picture".
    pcall(hl.window_rule, {
        name  = "folio-pip-nobar",
        match = { title = [[^(Picture-in-Picture|Picture in picture)$]] },
        ["hyprbars:no_bar"] = true,
    })
end

-- ── Gesti touch, come su un tablet Android ──────────────────────────
if hl.plugin and hl.plugin.hyprgrass then
    local G = hl.plugin.hyprgrass

    hl.config({
        plugin = {
            hyprgrass = {
                sensitivity                 = 4.0,  -- consigliato per i tablet
                long_press_delay            = 420,
                resize_on_border_long_press = true,
                edge_margin                 = 16,
            },
        },
    })

    -- 1 · Dal bordo inferiore verso l'alto: lo stesso gesto, tre esiti.
    --     corto → Dock · lungo → Home (Indice) · lungo + pausa → Recenti (Sommario).
    --     Il gesto viene misurato dal vivo e la decisione la prende Folio
    --     (soglie in ~/.config/folio/config.toml, sezione [gestures]).
    local up = { dy = 0, last_move = 0, height = 1080 }
    G.gesture({
        pattern = { kind = "edge", origin = "down", direction = "up" },
        action = {
            start = function(e)
                up.dy = 0
                up.last_move = e.time_ms
                up.height = (e.monitor and e.monitor.height) or 1080
            end,
            update = function(e)
                up.dy = up.dy - e.delta.y
                if math.abs(e.delta.y) > 1.5 then
                    up.last_move = e.time_ms
                end
            end,
            finish = function(e)
                if e.cancelled then return end
                local held = math.max(0, (e.time_ms or up.last_move) - up.last_move)
                folio(string.format("gesture edge-up %.3f %d", up.dy / up.height, held))
            end,
        },
    })

    -- 2 · Lungo il bordo inferiore, in orizzontale: passa da uno spazio all'altro
    --     seguendo il dito (il "quick switch" di Android).
    G.gesture({ pattern = { kind = "edge", origin = "down", direction = "horizontal" }, action = "workspace" })

    -- 3 · Indietro: dal bordo sinistro o destro verso l'interno.
    G.bind({ pattern = { kind = "edge", origin = "left",  direction = "right" }, action = function() folio("back") end })
    G.bind({ pattern = { kind = "edge", origin = "right", direction = "left" },  action = function() folio("back") end })

    -- 4 · Notiziario: dal bordo superiore verso il basso.
    G.bind({ pattern = { kind = "edge", origin = "up", direction = "down" }, action = function() folio("shade show") end })

    -- 5 · Tre dita.
    G.gesture({ pattern = { kind = "swipe", fingers = 3, direction = "horizontal" }, action = "workspace" })
    G.bind({ pattern = { kind = "swipe", fingers = 3, direction = "up" },   action = function() folio("overview show") end })
    G.bind({ pattern = { kind = "swipe", fingers = 3, direction = "down" }, action = function() folio("minimize") end })
    G.bind({ pattern = { kind = "tap",   fingers = 3 },                     action = function() folio("chrome toggle") end })

    -- 6 · Pizzico a tre dita: chiudi la mano → Sommario, aprila → Indice.
    G.bind({ pattern = { kind = "pinch", fingers = 3, direction = "pinchin" },  action = function() folio("overview show") end })
    G.bind({ pattern = { kind = "pinch", fingers = 3, direction = "pinchout" }, action = function() folio("home show") end })

    -- 7 · Pressione prolungata: tre dita sposta la finestra, quattro la ridimensiona.
    --     (sui bordi e negli spazi tra finestre basta un dito: resize_on_border_long_press)
    G.bind({ pattern = { kind = "longpress", fingers = 3 }, action = hl.dsp.window.drag(),   mouse = true })
    G.bind({ pattern = { kind = "longpress", fingers = 4 }, action = hl.dsp.window.resize(), mouse = true })

    -- 8 · Facoltativo: volume e luminosità scorrendo lungo i bordi laterali
    --     (copia extras/wpctl.lua e extras/brightnessctl.lua di hyprgrass in
    --     ~/.config/hypr/plugins/hyprgrass/extras/).
    local ok_v, vol = pcall(require, "plugins.hyprgrass.extras.wpctl")
    if ok_v and vol then
        G.gesture({
            pattern = { kind = "edge", origin = "right", direction = "vertical" },
            action  = vol.volume_action({ direction = "vertical", flip = true }),
        })
    end
    local ok_b, bri = pcall(require, "plugins.hyprgrass.extras.brightnessctl")
    if ok_b and bri and bri.brightness_action then
        G.gesture({
            pattern = { kind = "edge", origin = "left", direction = "vertical" },
            action  = bri.brightness_action({ direction = "vertical", flip = true }),
        })
    end
end
