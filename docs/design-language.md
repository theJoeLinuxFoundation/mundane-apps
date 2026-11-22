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

Icon slots are 14, 16, 24 and 32 logical pixels at the default text scale. Titlebar symbols are stroked geometry, with 14 logical pixels of artwork inside a 32-pixel target. Supply an action name independently of the artwork. Feedback and transition durations are 100 and 180 ms. Both resolve to zero with reduced motion. State changes and completion feedback still render. [Design tokens](../crates/mundane-ui/src/tokens.rs)

## Core components

| Component | States and interaction |
| --- | --- |
| Window header | Negotiates client decorations and the compositor’s supported controls. Minimize, Maximize or Restore, and Close have names and keyboard focus. Drag the title to move and double-click to maximize where supported. Tab and Shift+Tab traverse actions. F6 or Escape returns to content. |
| Buttons and links | Buttons have bordered resting, hover and focused states. Enter or Space activates a focused button. Links show an underline and expose the Link role. For low-level `unavailable`, guard the command in its owner as well as removing the accessible Click action. |
| Fields | Single-line, password and multiline fields expose a name, caret and selection. Passwords mask their contents. Arrow and Shift+arrow move or select, with standard clipboard keys. Multiline fields support indentation, undo/redo, page movement, zoom, line numbers and optional attributed runs. |
| Tabs | A named tab list contains selectable tabs and a separate close button. The selected tab uses the selection fill. Activation and close are caller commands. The suite binds Ctrl+Page Up/Down to switch. |
| Lists | Each option has a stable model key, name, selection and position. Supporting text, a value and an optional fraction can accompany the label. Up/Down and Home/End move selection. The owner decides what activation does. Lists accept application-defined keys. |
| Media transport | Play, Pause or Replay reflects playback state. The seek slider exposes position and duration, accepts pointer dragging and steps by five seconds. A caller can supply normalized audio peaks to draw a waveform inside the same slider. The played portion uses the focus colour, the remaining peaks use the secondary text colour, and the thumb marks position. Keyboard and AccessKit value actions stay the same. Unavailable seeking and transport actions are guarded. |
| Labels, headings and covered content | Labels expose static text runs and headings expose their level. Pass a stable control ID and the current heading text when a title comes from a document, application or catalog. `ApplicationContent` hides the covered accessible subtree during a modal and republishes it when dismissed. The caller owns initial focus and restores it on dismissal. |

The screenshots use the production renderers, private labwc, software rendering and a fake Settings portal. At 125% text size the output scale stays at 1. Build with `cargo build -p mundane-ui --example design_sheet` and replay with `tests/wayland/run --design-sheet core`.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Core components and roles | [Dark](design/core-dark.png) | [Light](design/core-light.png) | [Contrast](design/core-high-contrast.png) | [Contrast](design/core-light-high-contrast.png) | [Accent](design/core-accent.png) | [Reduced](design/core-reduced-motion.png) |

[Core components at 125% text scale](design/core-text-125.png)

## Panel controls

`panel_button` accepts any icon element and a separate name. A selected fill marks active, a dot marks running, and a diamond marks occupied. Its accessible description names the states. Disabled buttons suppress activation from both the pointer and keyboard. `toggle` publishes its pressed state and On/Off value. It requests a new value from its owner. A named `badge` exposes status or a count without adding a tab stop. `with_tooltip` provides a hover hint and a description for keyboard and AT users. Use `with_keyboard_tooltip` for a visible F1 hint. Escape or moving focus closes it.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Buttons, toggle, badges and tooltip | [Dark](design/panel-dark.png) | [Light](design/panel-light.png) | [Contrast](design/panel-high-contrast.png) | [Contrast](design/panel-light-high-contrast.png) | [Accent](design/panel-accent.png) | [Reduced](design/panel-reduced-motion.png) |

Run `tests/wayland/run --design-sheet panel` for the images and the private-bus keyboard and AT-SPI checks.

## Launcher rows and icon slots

Use `icon_slot(tokens::icon::LARGE, artwork)` for a 24-pixel list icon and `icon_slot(tokens::icon::EXTRA_LARGE, artwork)` for a 32-pixel pinned or taskbar icon. The slot centers its artwork, reserves its width and height, and clips overflow. Supply artwork sized to the slot. Both sizes follow the text and output scales.

`launcher_row(name, focus, selected, enabled, theme, activate)` accepts icon and text children. Keep the focus handle with the item’s stable key. Idle rows have no visible border. Hover adds an accent tint, selection uses the selected background, keyboard focus draws the focus ring and holding the primary button inverts the fill and foreground. Disabled rows use the disabled colour and expose no activation action. A row exposes its name, button role and selected value. Enter, Space, pointer release and the direct AccessKit Click action run the caller’s callback. Containers own arrow navigation and context menus. Searchable `ChoiceMenu` uses the same renderer with the list-option role and selection state. Packages uses it for installed packages, search and discovery.

| Appearance | Hover, focus and selection | Primary button held |
| --- | --- | --- |
| Dark | [Rows and slots](design/launcher-dark.png) | [Pressed](design/launcher-dark-pressed.png) |
| Light | [Rows and slots](design/launcher-light.png) | [Pressed](design/launcher-light-pressed.png) |
| Dark high contrast | [Rows and slots](design/launcher-high-contrast.png) | [Pressed](design/launcher-high-contrast-pressed.png) |
| Light high contrast | [Rows and slots](design/launcher-light-high-contrast.png) | [Pressed](design/launcher-light-high-contrast-pressed.png) |
| Custom accent | [Rows and slots](design/launcher-accent.png) | [Pressed](design/launcher-accent-pressed.png) |
| Reduced motion | [Rows and slots](design/launcher-reduced-motion.png) | [Pressed](design/launcher-reduced-motion-pressed.png) |

Replay with `tests/wayland/run --design-sheet launcher` after building the design sheet and the header-pointer example. The private labwc test checks pixel references, keyboard activation and a direct AT-SPI click.

## Popovers and choices

`Popover` retains the trigger and moves focus to the initial control on open. A second activation closes it. `popover_contents` sends Escape to the dismissal callback. Call the same dismissal path when the compositor reports an outside click. The compositor consumes that click at the seat.

`ChoiceMenu` provides anchored menu contents or a searchable list. `menu_trigger` exposes the expanded state. Up/Down wrap through enabled choices, Home/End jump to either end, Enter selects and Escape dismisses. Search filters labels without changing stable option keys. Tab switches between filter and options. Pointer, keyboard and AT activation emit the same selection event. Reordering choices preserves the accessible identity of each surviving key. Dismissal restores the trigger’s focus. The owner subscribes to `MenuEvent` and executes the selected command.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Popover, menu and searchable list | [Dark](design/menus-dark.png) | [Light](design/menus-light.png) | [Contrast](design/menus-high-contrast.png) | [Contrast](design/menus-light-high-contrast.png) | [Accent](design/menus-accent.png) | [Reduced](design/menus-reduced-motion.png) |

Run `tests/wayland/run --design-sheet menus` to check filtering, disabled options, selection, both dismissal keys and focus restoration.

## Settings controls and confirmations

Quick Settings tiles receive a label, icon, status, availability, on/off state and optional error text from their owner. The labelled switch uses the Switch role and exposes its checked state as well as visible On/Off text. Activation requests a change. The next render reflects the owner’s response. Disabled controls issue no requests.

`ValueSlider` exposes its accessible name, numeric value, minimum, maximum and step. Pointer clicks and drags, arrow keys, Home/End, and AccessKit SetValue, Increment and Decrement share the range checks. Requests snap to the configured step. `ConfirmDialog::show` focuses Cancel. Tab and Shift+Tab stay inside, Escape cancels, and an explicit confirm emits the confirmed outcome. Both outcomes restore the trigger. Wrap underlying controls in `ApplicationContent` while the dialog is open.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Tiles, switch and slider | [Dark](design/settings-dark.png) | [Light](design/settings-light.png) | [Contrast](design/settings-high-contrast.png) | [Contrast](design/settings-light-high-contrast.png) | [Accent](design/settings-accent.png) | [Reduced](design/settings-reduced-motion.png) |
| Confirmation | [Dark](design/confirmation-dark.png) | [Light](design/confirmation-light.png) | [Contrast](design/confirmation-high-contrast.png) | [Contrast](design/confirmation-light-high-contrast.png) | [Accent](design/confirmation-accent.png) | [Reduced](design/confirmation-reduced-motion.png) |

Run `tests/wayland/run --design-sheet settings` to exercise authoritative tile state, switch activation, the slider Value interface and keyboard steps, and both dialog outcomes.

## Notifications, history and level feedback

A notification card groups its title, body, optional source, named actions and Dismiss button. Each action returns the notification key and action key to the caller. A new banner announces the title and body through a polite live region while focus stays on the current control. History uses stable notification keys so refreshing or reordering the list preserves the focused card. Up/Down and Home/End navigate history. Tab reaches the actions, and Enter or Space activates them. Removing the focused card transfers focus to its neighbour or the empty list.

The level OSD displays a named, read-only meter and announces its percentage or muted state. It has no focus handle or tab stop. The host owns mapping the overlay and its lifetime. Its window must also preserve the active surface’s keyboard focus.

| Reference sheet | Dark | Light | Dark high contrast | Light high contrast | Custom accent | Reduced motion |
| --- | --- | --- | --- | --- | --- | --- |
| Banner, history and level OSD | [Dark](design/notifications-dark.png) | [Light](design/notifications-light.png) | [Contrast](design/notifications-high-contrast.png) | [Contrast](design/notifications-light-high-contrast.png) | [Accent](design/notifications-accent.png) | [Reduced](design/notifications-reduced-motion.png) |

Run `tests/wayland/run --design-sheet notifications` to check live announcement events, focus during a passive update, card actions, history navigation and dismissal on the private bus. In the recorded run Orca spoke “New message. Message 1 arrived.” and “Volume: 55%”. [Orca recording](accessibility/orca-notifications.jsonl)
