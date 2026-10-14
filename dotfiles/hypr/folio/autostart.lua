-- Avvio della sessione.
hl.on("hyprland.start", function()
    -- Variabili per i servizi utente (portali, polkit, notifiche D-Bus).
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE")

    -- Plugin (hyprbars, hyprgrass): caricati prima di tutto il resto.
    hl.exec_cmd("hyprpm reload -n")

    -- La shell: Testata, Dock, Copertina, Indice, Sommario, Notiziario, notifiche.
    hl.exec_cmd("folio")

    -- Tastiera a schermo, nascosta finché non serve (Folio la mostra al tocco di un campo).
    hl.exec_cmd("command -v wvkbd-mobintl >/dev/null && wvkbd-mobintl --hidden -L 280 --fn 'Inter 18'")

    -- Rotazione automatica (se è installato iio-hyprland).
    hl.exec_cmd("command -v iio-hyprland >/dev/null && iio-hyprland")

    -- Autenticazione, inattività, appunti.
    hl.exec_cmd("systemctl --user start hyprpolkitagent")
    hl.exec_cmd("hypridle")

    -- Le app GTK leggono la stessa tipografia e la stessa edizione.
    hl.exec_cmd("gsettings set org.gnome.desktop.interface font-name 'Inter 11'")
    hl.exec_cmd("gsettings set org.gnome.desktop.interface document-font-name 'Instrument Serif 13'")
    hl.exec_cmd("gsettings set org.gnome.desktop.interface monospace-font-name 'JetBrains Mono 11'")
    hl.exec_cmd("gsettings set org.gnome.desktop.interface icon-theme 'Papirus'")
    hl.exec_cmd("gsettings set org.gnome.desktop.interface gtk-theme 'adw-gtk3'")
end)
