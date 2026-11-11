---
section: fixed
---
- **The Windows taskbar badge is drawn at the taskbar's size and made as Microsoft says** (zgx-desktop): `Platform::set_badge` (`ui.badge`) drew one 16-pixel icon with a pixel font, which Windows stretched to 20, 24 or 32 pixels at a higher scale and cut off at two digits. It is now an antialiased pill in the app's accent colour (`badge_art::FILL`) with the count in Geist Bold, made at the small-icon size of the window's DPI (16, 20, 24 or 32 pixels) and again when the DPI changes; a hundred or more says `99+`; text that is no count is a dot. The taskbar is made, and the badge told, only after the window's `TaskbarButtonCreated` (a window already showing has had it), and again when the button is made anew (Explorer restarting), as `ITaskbarList3` asks.
