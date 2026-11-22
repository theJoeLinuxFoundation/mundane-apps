# Desktop appearance

All 18 apps use the Settings portal on the session bus. Each process subscribes to `SettingChanged` before reading `org.freedesktop.appearance` with `ReadAll`. The running app refreshes its windows when a preference changes. It returns to the suite defaults when the portal disappears and reads a fresh snapshot when the service acquires the name again. [XDG Settings portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)

| Preference | Effect in mundane-ui | Default |
| --- | --- | --- |
| `color-scheme` | Light or dark surfaces, panels and text | Dark |
| `contrast` | High-contrast palette in the preferred scheme | Normal contrast |
| `accent-color` | Focus indicators and selection backgrounds. Shades are adjusted for legible focus rings and selected text. | The palette’s focus and selection colors |
| `reduced-motion` | GPUI renders nonessential animations in their static state | Standard motion |

`MUNDANE_HIGH_CONTRAST=1` still forces high contrast in either color scheme. Appearance updates preserve focus and text selection. Fonts rerenders its sample, Maps redraws its vector tiles, and Terminal updates its default and ANSI palettes. User-supplied terminal colors, image pixels, PDF content and video playback retain their own colors or timing.

## Private Wayland test

Build the apps and the motion fixture, then run the test as a regular user with the suite’s Wayland test dependencies and Python GObject bindings installed. Set `MUNDANE_BIN_DIR` to an absolute binary directory for release builds.

```sh
cargo build --locked --workspace --bins --example motion
tests/wayland/run --appearance
```

The runner starts a private session bus, fake Settings portal and headless Sway with Pixman and Mesa lavapipe. It samples the rendered header of every app while the portal changes the settings and checks the window ID and process ID. It also checks the map raster, a selected document across portal loss and restart, and a repeating GPUI animation that stops and resumes. Maps and Weather receive loopback provider endpoints.

| Setting | Reference |
| --- | --- |
| Dark | [Selected document](../tests/wayland/appearance/dark.png) |
| Light | [Same window and selection](../tests/wayland/appearance/light.png) |
| High contrast | [Dark scheme](../tests/wayland/appearance/high-contrast.png) · [Light scheme](../tests/wayland/appearance/light-high-contrast.png) |
| Custom accent | [Purple focus ring and selection](../tests/wayland/appearance/custom-accent.png) |
| Reduced motion | [Animation fixture with a stationary marker](../tests/wayland/appearance/reduced-motion.png) |
