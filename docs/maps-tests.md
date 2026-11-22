# Maps

Maps draws OpenFreeMap’s OpenMapTiles vector data with a suite-owned style. It reads the TileJSON manifest to discover the current tile URLs and zoom range. The renderer projects coordinates in Web Mercator, wraps across the date line and clips at the poles. It draws land use, parks, buildings, boundaries, roads, waterways, water and place labels. Missing tiles have a hatched background. The map and search results both show linked OpenFreeMap, OpenMapTiles and OpenStreetMap credits and the ODbL licence. [OpenFreeMap terms](https://openfreemap.org/tos/) · [OpenStreetMap copyright](https://www.openstreetmap.org/copyright)

## Place search and traffic

Enter or **Search** sends the text to Nominatim. Typing in the field stays local. Results are cached on disk by endpoint and normalized query, including empty result sets. A per-user lock shared by Maps processes spaces uncached search requests at least 1.2 seconds apart. Tile and search requests use `mundane-maps/VERSION https://github.com/theJoeLinuxFoundation/mundane-apps`. The providers receive the client IP and the requested tiles or submitted search text. Avoid submitting private information.

The public Nominatim policy applies its one-request-per-second limit to traffic from the entire application. Independent machines have separate clocks and caches. For a distribution-wide deployment, point clients at a shared caching and rate-limiting endpoint or a self-hosted Nominatim service. Both endpoints can be changed without rebuilding the application. [Nominatim usage policy](https://operations.osmfoundation.org/policies/nominatim/)

| Environment variable | Expected interface | Default |
| --- | --- | --- |
| `MUNDANE_MAPS_TILEJSON` | OpenMapTiles-compatible TileJSON containing an XYZ or TMS vector tile template | `https://tiles.openfreemap.org/planet` |
| `MUNDANE_MAPS_SEARCH` | Nominatim Search accepting `q`, `limit` and `format=jsonv2` | `https://nominatim.openstreetmap.org/search` |

## Saved area

**Save area** stores the complete visible view at its current zoom, including its selected place. **Replace saved area** updates that snapshot atomically. Each tile endpoint has its own snapshot. The app reopens it offline on launch. **Go online** restores live navigation. Offline panning can reveal the edge of the saved coverage. Offline place searches use the local results cache.

Saving reuses the displayed tiles. Each snapshot is limited to 3,000 distinct tiles and 50,000,000 bytes, including its encoded file. Maps fetches tiles serially, with one in-flight tile request across the user’s Maps processes. Failed or incomplete views leave the previous snapshot available. [OpenFreeMap offline download limits](https://openfreemap.org/quick_start/)

## Verification and packaging

The HTTP fixture serves original harbour geometry encoded as gzipped MVT and synthetic place records. The tests cover tile boundaries at the poles and date line, 404 tiles, malformed tile input, both offline limits, concurrent search pacing and cache reuse. The GPUI test selects the second result, pans and zooms by keyboard, saves the view, stops the server and reopens it. The Wayland harness checks that typing issued no search, verifies the identifying User-Agent, and compares six pixel references on software-rendered Sway and labwc. The private AT-SPI probe reads the selected place and zoom and checks every attribution link. [Fixture geometry](../tests/providers/vector_tile.py)

Ship system libcurl with HTTPS, zlib and trusted CA certificates, as for Weather. Rust dependencies include prost (Apache-2.0), tiny-skia (BSD-3-Clause) and rmp-serde (MIT). Provider data stays in the user cache. The repository’s synthetic tile and search fixtures use the suite licence.

Run `cargo test -p mundane-maps`, `cargo build -p mundane-maps` and `tests/wayland/run --app maps`. The harness accepts `--compositor labwc`, `--profile fractional|contrast` and, on Sway, `--at-spi`.

Places (Ctrl+B) lists favourites and the current-location action. Ctrl+D saves or removes the marked place, or the map centre when there is no marker. The favourites file lives under the XDG user data directory and can be reopened offline. Ctrl+G requests a one-shot fix from the XDG Location portal. The private-bus test refuses the first request, grants the next, then navigates back to the saved place without another search. Directions are deferred by provider decision.
