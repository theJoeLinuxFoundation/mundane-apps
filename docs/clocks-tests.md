# Clocks

Clocks uses Jiff and the distribution’s IANA zoneinfo files. Its countdown uses Linux boot time so suspend counts towards the deadline and changing the wall clock leaves the duration intact. Keep Clocks running until expiry. The app shows the finished timer in its window and also calls the session notification service. Dismiss closes the notification returned by that service.

The unit tests cross New York’s spring DST gap and advance a countdown through its deadline. The GPUI test adds New York, starts a timer and dismisses it. The Wayland test types the city name into the real window, checks the saved zone, then starts a one-second countdown and observes Notify and CloseNotification on a private bus. Its reference executable uses the production view with the wall clock fixed to 07:00 UTC on 9 March 2025. The private AT-SPI test reads both city times from the list.

Build both executables with `cargo build --locked -p mundane-clocks --bin mundane-clocks --example clocks-fixture`. Run `tests/wayland/run --app clocks` for the standard headless Sway reference. The same command accepts `--compositor labwc`, `--profile fractional|contrast`, and on Sway, `--at-spi`. Both compositors use software rendering.
