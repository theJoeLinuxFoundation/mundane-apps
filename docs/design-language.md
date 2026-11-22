# joe design language

The GPUI shell, Settings and applications can depend on `mundane-ui` from the same git revision. Call `mundane_ui::run` to use the accessible GPUI constructor and subscribe to desktop appearance. Read `Theme::current(cx)` during render. Give each control a stable identity, a human-readable name and a focus handle that survives rerenders. Services and commands belong to the caller. [Shell ownership contract](https://github.com/theJoeLinuxFoundation/mundaneDE/blob/26eec85/docs/shell-gpui.md#shared-design-system-and-ownership)

## Colour

| Role | Dark | Light | Dark high contrast | Light high contrast | Use |
| --- | --- | --- | --- | --- | --- |
| Surface | #17201E | #F8FAF8 | #000000 | #FFFFFF | Working area and field background |
| Panel | #222E2A | #E9EEEA | #000000 | #FFFFFF | Header, controls and grouped content |
| Text | #F0F3ED | #17201E | #FFFFFF | #000000 | Primary content and icons |
| Muted | #B4C3B9 | #47594E | #FFFFFF | #000000 | Supporting descriptions and units |
| Border | #52675C | #75847A | #FFFFFF | #000000 | Edges, separators and slider tracks |
| Focus | #A0DFB3 | #23623E | #FFFF00 | #000099 | Keyboard outline, caret, active marker |
| Selected | #406D58 | #B3D9C1 | #224488 | #AACCFF | Selected rows, tabs and text |
| Disabled | #99A59C | #58635C | #FFFFFF | #000000 | Unavailable control label or icon |
| Error | #FFB3A9 | #952E2B | #FFB3A9 | #952E2B | Explain a failure in text |

The Settings portal can change the palette while a window is open. An accent adjusts focus and selection. Focus targets at least 3:1 against the panel, or 4.5:1 in high contrast. Selection is adjusted against secondary text to 4.5:1 or 7:1 respectively. Keep labels, state markers and accessible values when changing colours. Dark is the default without a portal. [Palette and contrast calculations](../crates/mundane-ui/src/appearance.rs)

## Type, spacing and motion

The shared tokens use rem units. At the default 16 logical pixels per rem, caption, control, body and title sizes are 12, 14, 16 and 20. Use DejaVu Sans for controls and DejaVu Sans Mono for preformatted text. Use semibold for a section or window title. The host can apply its text scale with `tokens::set_text_scale`. The compositor scales logical pixels to the output independently.

Spacing steps are 4, 8, 12, 16 and 24. Compact button groups use 4 between actions. Form rows use 8, with 12 inside grouped panels. Control and dialog radii are 6 and 8. Use a one-logical-pixel border and a visible focus outline in the focus colour. A flat panel and its border establish grouping. A modal adds a translucent black scrim and an opaque panel. The shared components use no drop shadow.

Icon slots are 14, 16 and 24. Titlebar symbols are stroked geometry, with 14 logical pixels of artwork inside a 32-pixel target. Supply an action name independently of the artwork. Feedback and transition durations are 100 and 180 ms. Both resolve to zero with reduced motion. State changes and completion feedback still render. [Design tokens](../crates/mundane-ui/src/tokens.rs)

## Core components

| Component | States and interaction |
| --- | --- |
| Window header | Negotiates client decorations and the compositor’s supported controls. Minimize, Maximize or Restore, and Close have names and keyboard focus. Drag the title to move and double-click to maximize where supported. Tab and Shift+Tab traverse actions. F6 or Escape returns to content. |
| Buttons and links | Buttons have bordered resting, hover and focused states. Enter or Space activates a focused button. Links show an underline and expose the Link role. For low-level `unavailable`, guard the command in its owner as well as removing the accessible Click action. |
| Fields | Single-line, password and multiline fields expose a name, caret and selection. Passwords mask their contents. Arrow and Shift+arrow move or select, with standard clipboard keys. Multiline fields support indentation, undo/redo, page movement, zoom, line numbers and optional attributed runs. |
| Tabs | A named tab list contains selectable tabs and a separate close button. The selected tab uses the selection fill. Activation and close are caller commands. The suite binds Ctrl+Page Up/Down to switch. |
| Lists | Each option has a stable model key, name, selection and position. Supporting text, a value and an optional fraction can accompany the label. Up/Down and Home/End move selection. The owner decides what activation does. Lists accept application-defined keys. |
| Media transport | Play, Pause or Replay reflects playback state. The seek slider exposes position and duration, accepts pointer dragging and steps by five seconds. Unavailable seeking and transport actions are guarded. |
| Labels, headings and covered content | Labels expose static text runs and headings expose their level. `ApplicationContent` hides the covered accessible subtree during a modal and republishes it when dismissed. The caller owns initial focus and restores it on dismissal. |

The screenshots use the production renderers, private labwc, software rendering and a fake Settings portal. At 125% text size the output scale stays at 1. Build with `cargo build -p mundane-ui --example design_sheet` and replay with `tests/wayland/run --design-sheet core`.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Core components and roles | [Dark](design/core-dark.png) | [Light](design/core-light.png) | [Contrast](design/core-high-contrast.png) | [Contrast](design/core-light-high-contrast.png) | [Accent](design/core-accent.png) | [Reduced](design/core-reduced-motion.png) |

[Core components at 125% text scale](design/core-text-125.png)

## Panel controls

`panel_button` accepts any icon element and a separate name. A selected fill marks active, a dot marks running, and a diamond marks occupied. Its accessible description names the states. Disabled buttons suppress activation from both the pointer and keyboard. `toggle` publishes its pressed state and On/Off value. It requests a new value from its owner. A named `badge` exposes status or a count without adding a tab stop. `with_tooltip` provides a hover hint and a description for keyboard and AT users. The `tooltip` renderer can also be shown by an explicitly activated help control.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Buttons, toggle, badges and tooltip | [Dark](design/panel-dark.png) | [Light](design/panel-light.png) | [Contrast](design/panel-high-contrast.png) | [Contrast](design/panel-light-high-contrast.png) | [Accent](design/panel-accent.png) | [Reduced](design/panel-reduced-motion.png) |

Run `tests/wayland/run --design-sheet panel` for the images and the private-bus keyboard and AT-SPI checks.

## Popovers and choices

`Popover` retains the trigger and moves focus to the initial control on open. A second activation closes it. `popover_contents` sends Escape to the dismissal callback. Call the same dismissal path when the compositor reports an outside click. The compositor consumes that click at the seat.

`ChoiceMenu` provides anchored menu contents or a searchable list. `menu_trigger` exposes the expanded state. Up/Down wrap through enabled choices, Home/End jump to either end, Enter selects and Escape dismisses. Search filters labels without changing stable option keys. Tab switches between filter and options. Pointer, keyboard and AT activation emit the same selection event. Dismissal restores the trigger’s focus. The owner subscribes to `MenuEvent` and executes the selected command.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Popover, menu and searchable list | [Dark](design/menus-dark.png) | [Light](design/menus-light.png) | [Contrast](design/menus-high-contrast.png) | [Contrast](design/menus-light-high-contrast.png) | [Accent](design/menus-accent.png) | [Reduced](design/menus-reduced-motion.png) |

Run `tests/wayland/run --design-sheet menus` to check filtering, disabled options, selection, both dismissal keys and focus restoration.
