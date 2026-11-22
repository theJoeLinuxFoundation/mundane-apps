# Weather

Weather reads MET Norway’s Locationforecast 2.0 compact forecast. Choose a city from the local IANA zone table, or enter latitude, longitude and time zone (for example, `59.91 10.75 Europe/Oslo`). City selection stays local. A forecast request sends coordinates truncated to four decimal places. The view shows temperature in °C, wind in m/s, and update and forecast times in the chosen zone. It credits MET Norway and links to the source and CC BY 4.0 licence. Times and condition labels are formatted for display. [Locationforecast data model](https://api.met.no/doc/locationforecast/datamodel)

Every GET identifies the suite with `mundane-weather/VERSION https://github.com/theJoeLinuxFoundation/mundane-apps`. Weather caches each coordinate pair and endpoint under the XDG cache directory. It observes Expires, revalidates with the original Last-Modified value, and retains the forecast on a 304 or failed update. An unavailable connection with expired data shows an offline status and the original update time. Requests occur when opening a saved location, choosing a location or pressing Refresh. A cross-process gate spaces starts at least two seconds apart for the same user.

Ship IANA tzdata (including zone.tab), system libcurl with HTTPS and compression (curl licence), zlib (Zlib licence) and trusted CA certificates. Keep their package notices. Set `MUNDANE_WEATHER_ENDPOINT` to a compatible shared caching proxy when distributing at scale. MET’s traffic threshold covers all installations combined. [MET Norway terms](https://api.met.no/doc/TermsOfService)

## Verification

The loopback fixture supplies synthetic temperatures and HTTP dates. Tests cover request identity, coordinate precision, cache separation, Celsius and wind units, New York’s DST transition, expired entries, 304 revalidation and reuse before expiry. The GPUI and Wayland tests choose Oslo, stop the HTTP server and reopen the saved forecast offline. The six headless Sway and labwc references cover standard, fractional and high-contrast rendering. The private AT-SPI probe checks the forecast, stale status and attribution links.

Run `cargo test -p mundane-weather`, `cargo build -p mundane-weather` and `tests/wayland/run --app weather`. Use `--compositor labwc`, `--profile fractional|contrast` or, on Sway, `--at-spi` for the other profiles.

Daily (Ctrl+D) groups forecasts by the selected place’s local calendar date. It displays the lowest and highest sample temperatures and the condition from the available sample nearest local noon. Locations puts previously selected places first and keeps them in the user data directory. Existing single-location preferences are read on upgrade.

Current location (Ctrl+G) makes a one-time, city-accuracy request to the XDG Location portal. The app closes the session after permission refusal or after receiving a fix. Using a fix for a forecast does not add those coordinates to saved locations. The private-bus test covers approval and refusal with a synthetic position. [Location portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Location.html)
