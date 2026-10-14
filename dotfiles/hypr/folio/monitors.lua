-- Schermi. "preferred/auto" va bene per quasi tutti i tablet e i 2-in-1.
-- Per uno schermo specifico (es. il pannello interno di un tablet):
--   hl.monitor({ output = "eDP-1", mode = "2560x1600@60", position = "0x0", scale = 1.6 })
-- Per partire in verticale aggiungi `transform = 1` e, in input.lua,
-- `touchdevice = { transform = 1 }` (o usa "Ruota schermo" nel Notiziario).
hl.monitor({
    output   = "",
    mode     = "preferred",
    position = "auto",
    scale    = "auto",
})
