-- Scorciatoie da tastiera. Su tablet si usano i gesti (plugins.lua);
-- con una tastiera collegata valgono le stesse azioni.
local mod      = "SUPER"
local terminal = "kitty"
local files    = "xdg-open " .. (os.getenv("HOME") or "~")

local function folio(cmd)
    return hl.dsp.exec_cmd("folio msg " .. cmd)
end

-- ── Folio ──
hl.bind(mod .. " + Super_L", folio("overview toggle"), { release = true }) -- come GNOME: Super da solo
hl.bind(mod .. " + Space",   folio("home toggle"))     -- Indice
hl.bind(mod .. " + Tab",     folio("overview toggle")) -- Sommario / Recenti
hl.bind(mod .. " + N",       folio("shade toggle"))    -- Notiziario
hl.bind(mod .. " + period",  folio("chrome toggle"))   -- comandi finestra (il popup della barra del titolo)
hl.bind(mod .. " + I",       folio("layout show"))     -- Impaginazione
hl.bind(mod .. " + D",       folio("dock toggle"))
hl.bind(mod .. " + H",       folio("minimize"))
hl.bind(mod .. " + SHIFT + H", folio("restore"))
hl.bind(mod .. " + K",       folio("osk toggle"))      -- tastiera a schermo
hl.bind(mod .. " + SHIFT + T", folio("theme toggle"))  -- edizione chiara / serale
hl.bind(mod .. " + BackSpace", folio("back"))

-- ── Finestre ──
hl.bind(mod .. " + Return", hl.dsp.exec_cmd(terminal))
hl.bind(mod .. " + E",      hl.dsp.exec_cmd(files))
hl.bind(mod .. " + Q",      hl.dsp.window.close())
hl.bind(mod .. " + F",      hl.dsp.window.fullscreen({ mode = "fullscreen", action = "toggle" }))
hl.bind(mod .. " + M",      hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" }))
hl.bind(mod .. " + V",      hl.dsp.window.float({ action = "toggle" }))
hl.bind(mod .. " + P",      hl.dsp.window.pin())
hl.bind(mod .. " + J",      hl.dsp.layout("togglesplit"))
hl.bind(mod .. " + L",      hl.dsp.exec_cmd("loginctl lock-session"))

for _, d in ipairs({ "left", "right", "up", "down" }) do
    hl.bind(mod .. " + " .. d,             hl.dsp.focus({ direction = d }))
    hl.bind(mod .. " + SHIFT + " .. d,     hl.dsp.window.move({ direction = d }))
end

-- Spazi di lavoro (dinamici: `e+1` crea il successivo se serve).
for i = 1, 9 do
    hl.bind(mod .. " + " .. i,         hl.dsp.focus({ workspace = i }))
    hl.bind(mod .. " + SHIFT + " .. i, hl.dsp.window.move({ workspace = i }))
end
hl.bind(mod .. " + CTRL + right", hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mod .. " + CTRL + left",  hl.dsp.focus({ workspace = "e-1" }))
hl.bind(mod .. " + mouse_down",   hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mod .. " + mouse_up",     hl.dsp.focus({ workspace = "e-1" }))

hl.bind(mod .. " + mouse:272", hl.dsp.window.drag(),   { mouse = true })
hl.bind(mod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })

-- ── Tasti multimediali (funzionano anche a schermo bloccato) ──
hl.bind("XF86AudioRaiseVolume",  hl.dsp.exec_cmd("wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume",  hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"),      { locked = true, repeating = true })
hl.bind("XF86AudioMute",         hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"),     { locked = true })
hl.bind("XF86AudioMicMute",      hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"),   { locked = true })
hl.bind("XF86MonBrightnessUp",   hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%+"),                  { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%-"),                  { locked = true, repeating = true })
hl.bind("XF86AudioNext",  hl.dsp.exec_cmd("playerctl next"),       { locked = true })
hl.bind("XF86AudioPrev",  hl.dsp.exec_cmd("playerctl previous"),   { locked = true })
hl.bind("XF86AudioPlay",  hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })

-- Istantanea: tutto lo schermo / una zona.
hl.bind("Print",        hl.dsp.exec_cmd("mkdir -p ~/Immagini/Schermate && grim ~/Immagini/Schermate/$(date +%Y-%m-%d_%H-%M-%S).png"))
hl.bind("SHIFT + Print", hl.dsp.exec_cmd("mkdir -p ~/Immagini/Schermate && grim -g \"$(slurp)\" ~/Immagini/Schermate/$(date +%Y-%m-%d_%H-%M-%S).png"))

-- Tasto di rotazione dei 2-in-1: ruota schermo e touchscreen insieme.
hl.bind("XF86RotateWindows", folio("rotate"))
