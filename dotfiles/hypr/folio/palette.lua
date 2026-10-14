-- Palette condivisa. Deve coincidere con [theme] in ~/.config/folio/config.toml:
-- Folio la riallinea comunque a ogni cambio di edizione (chiara/scura).
local P = {
    paper  = "F3F0E8", -- carta
    ink    = "141413", -- inchiostro
    accent = "D4442A", -- rosso tipografico
    paper_dark = "121211",
    ink_dark   = "ECE8DE",
}

function P.rgb(hex) return "rgb(" .. hex .. ")" end
function P.rgba(hex, alpha_hex) return "rgba(" .. hex .. alpha_hex .. ")" end

return P
