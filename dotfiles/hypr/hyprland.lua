-- ════════════════════════════════════════════════════════════════════
--  FOLIO — Hyprland ≥ 0.55 (configurazione Lua)
--  Una shell touch impaginata come una rivista di design.
--
--  Struttura:
--    folio/palette.lua    carta, inchiostro, accento (condivisi)
--    folio/monitors.lua   schermi e rotazione
--    folio/env.lua        variabili d'ambiente
--    folio/look.lua       gabbia, bordi, ombre, animazioni
--    folio/input.lua      tastiera, touchscreen, touchpad
--    folio/rules.lua      regole per finestre e superfici di Folio
--    folio/binds.lua      scorciatoie da tastiera
--    folio/plugins.lua    hyprbars (barre del titolo) + hyprgrass (gesti)
--    folio/autostart.lua  avvio della sessione
--
--  Le personalizzazioni vanno in ~/.config/hypr/user.lua: non viene mai
--  sovrascritto dall'installer.
-- ════════════════════════════════════════════════════════════════════

local home = os.getenv("HOME") or ""
local config_home = os.getenv("XDG_CONFIG_HOME") or (home .. "/.config")
local here = config_home .. "/hypr/"
package.path = here .. "?.lua;" .. here .. "?/init.lua;" .. package.path

require("folio.monitors")
require("folio.env")
require("folio.look")
require("folio.input")
require("folio.rules")
require("folio.binds")
require("folio.plugins")
require("folio.autostart")

-- Personalizzazioni locali (facoltative).
local ok, err = pcall(require, "user")
if not ok and not tostring(err):find("module 'user' not found", 1, true) then
    hl.notification.create({ text = "user.lua: " .. tostring(err), timeout = 8000, icon = "error" })
end
