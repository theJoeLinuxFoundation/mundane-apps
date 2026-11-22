# Suite polish review

The review covered the standard Wayland reference and keyboard paths in all 18 apps. Files now groups the two items in a name conflict, with space between their metadata and the action row. Cancel is the default. Replace is available for distinct items, and a changed item requires a fresh review. Ctrl+W closes Files.

F6 enters the header in Clocks, Characters, Fonts, Weather and Maps. From the header, F6 or Escape returns to the current content. Weather uses sentence case for conditions from both fresh forecasts and older caches. Clocks, Maps and Archive Manager distinguish one result or entry from several. The three viewers distinguish a failed open from their initial empty state.

Terminal, Text Editor, System Monitor, Calculator, Packages, Music, Disk Usage and Disks retained their existing layouts and shortcuts after review. Terminal keeps Ctrl+Shift+C, Ctrl+Shift+V and Ctrl+Shift+F so shell editing shortcuts still work.

## Before and after

Each pair shows the same fixture and keyboard sequence on a private, software-rendered Sway output. The **Before** column uses commit `2f404ab`. Open either image for its full resolution.

| App and state | Before | After |
| --- | --- | --- |
| Files · Replace two different items | [![Before](before/files-replace.png)](before/files-replace.png) | [![After](after/files-replace.png)](after/files-replace.png) |
| Files · Copy back into the same folder | [![Before](before/files-same-item.png)](before/files-same-item.png) | [![After](after/files-same-item.png)](after/files-same-item.png) |
| Weather · Forecast conditions and F6 focus | [![Before](before/weather-forecast.png)](before/weather-forecast.png) | [![After](after/weather-forecast.png)](after/weather-forecast.png) |
| Weather · Service unavailable with no cached forecast | [![Before](before/weather-error.png)](before/weather-error.png) | [![After](after/weather-error.png)](after/weather-error.png) |
| Clocks · One saved city and F6 focus | [![Before](before/clocks-header.png)](before/clocks-header.png) | [![After](after/clocks-header.png)](after/clocks-header.png) |
| Clocks · No saved cities | [![Before](before/clocks-empty.png)](before/clocks-empty.png) | [![After](after/clocks-empty.png)](after/clocks-empty.png) |
| Clocks · One matching time zone | [![Before](before/clocks-one-zone.png)](before/clocks-one-zone.png) | [![After](after/clocks-one-zone.png)](after/clocks-one-zone.png) |
| Clocks · No matching time zones | [![Before](before/clocks-no-results.png)](before/clocks-no-results.png) | [![After](after/clocks-no-results.png)](after/clocks-no-results.png) |
| Characters · F6 from character results | [![Before](before/characters-header.png)](before/characters-header.png) | [![After](after/characters-header.png)](after/characters-header.png) |
| Fonts · F6 from the family list | [![Before](before/fonts-header.png)](before/fonts-header.png) | [![After](after/fonts-header.png)](after/fonts-header.png) |
| Maps · F6 from the map | [![Before](before/maps-header.png)](before/maps-header.png) | [![After](after/maps-header.png)](after/maps-header.png) |
| Maps · One place found | [![Before](before/maps-one-result.png)](before/maps-one-result.png) | [![After](after/maps-one-result.png)](after/maps-one-result.png) |
| Archive Manager · Empty archive | [![Before](before/archive-manager-empty.png)](before/archive-manager-empty.png) | [![After](after/archive-manager-empty.png)](after/archive-manager-empty.png) |
| Archive Manager · Archive with one entry | [![Before](before/archive-manager-one-entry.png)](before/archive-manager-one-entry.png) | [![After](after/archive-manager-one-entry.png)](after/archive-manager-one-entry.png) |
| Image Viewer · Damaged image | [![Before](before/image-viewer-error.png)](before/image-viewer-error.png) | [![After](after/image-viewer-error.png)](after/image-viewer-error.png) |
| Document Viewer · Unreadable PDF | [![Before](before/document-viewer-error.png)](before/document-viewer-error.png) | [![After](after/document-viewer-error.png)](after/document-viewer-error.png) |
| Video · Damaged WebM file | [![Before](before/video-error.png)](before/video-error.png) | [![After](after/video-error.png)](after/video-error.png) |

After building the apps and their fixture examples, run the following from the checkout to compare all the after images pixel for pixel. The same launcher isolates the compositor, session bus, home directory and provider fixtures as the main Wayland tests.

```sh
cargo build --locked --workspace --bins --examples
for app in archive-manager characters clocks document-viewer files fonts image-viewer maps video weather; do
    tests/wayland/run --app "$app" --polish || exit 1
done
```

Files and Weather also have updated normal, fractional-scale and high-contrast references on Sway and labwc. [Files tests](../files-tests.md) · [Weather tests](../weather-tests.md)
