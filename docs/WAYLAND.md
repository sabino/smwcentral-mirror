# Wayland Safety

`smwapt` avoids surprise GUI backend selection for visible clients and tools.

- `smwapt-gui` sets `QT_QPA_PLATFORM=wayland;xcb` when `WAYLAND_DISPLAY` is present and no explicit Qt platform was provided.
- Visible tool launches from the installer set conservative defaults for Qt, SDL, and GTK:
  - `QT_QPA_PLATFORM=wayland;xcb`
  - `SDL_VIDEODRIVER=wayland`
  - `GDK_BACKEND=wayland,x11`
  - `NO_AT_BRIDGE=1`
- `scripts/smwapt-gui-wayland` wraps the GUI with the same defaults.

These defaults can still be overridden by exporting the variables before launch.
