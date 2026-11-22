//! Process-wide appearance preferences from the desktop's Settings portal.
use gpui::{App, Application, AsyncApp, Global};
use ordered_stream::OrderedStreamExt;
use std::collections::HashMap;
use zbus::{Connection, Proxy, zvariant::OwnedValue};

const NAMESPACE: &str = "org.freedesktop.appearance";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub surface: u32,
    pub panel: u32,
    pub text: u32,
    pub secondary: u32,
    pub border: u32,
    pub focus: u32,
    pub selection: u32,
    pub high_contrast: bool,
    pub light: bool,
}
impl Global for Theme {}

impl Theme {
    pub fn current(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or(Self::DARK)
    }
    pub const DARK: Self = Self {
        surface: 0x17201e,
        panel: 0x222e2a,
        text: 0xf0f3ed,
        secondary: 0xb4c3b9,
        border: 0x52675c,
        focus: 0xa0dfb3,
        selection: 0x406d58,
        high_contrast: false,
        light: false,
    };
    pub const LIGHT: Self = Self {
        surface: 0xf8faf8,
        panel: 0xe9eeea,
        text: 0x17201e,
        secondary: 0x47594e,
        border: 0x75847a,
        focus: 0x23623e,
        selection: 0xb3d9c1,
        high_contrast: false,
        light: true,
    };
    pub const HIGH_CONTRAST: Self = Self {
        surface: 0,
        panel: 0,
        text: 0xffffff,
        secondary: 0xffffff,
        border: 0xffffff,
        focus: 0xffff00,
        selection: 0x224488,
        high_contrast: true,
        light: false,
    };
    const LIGHT_HIGH_CONTRAST: Self = Self {
        surface: 0xffffff,
        panel: 0xffffff,
        text: 0,
        secondary: 0,
        border: 0,
        focus: 0x000099,
        selection: 0xaaccff,
        high_contrast: true,
        light: true,
    };
}

#[derive(Default)]
struct Preferences {
    light: bool,
    contrast: bool,
    accent: Option<u32>,
    reduced_motion: bool,
}

fn channels(color: u32) -> [f64; 3] {
    [16, 8, 0].map(|shift| f64::from((color >> shift) & 255u32) / 255.)
}

fn pack(rgb: [f64; 3]) -> u32 {
    rgb.into_iter()
        .fold(0, |c, v| c << 8 | (v * 255.).round() as u32)
}

fn luminance(color: u32) -> f64 {
    let [r, g, b] = channels(color).map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn ratio(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

// Keep the requested accent when legible. Move towards the appropriate palette
// extreme just far enough to preserve the focus ring or selected text.
fn legible(accent: u32, toward: u32, background: u32, minimum: f64) -> u32 {
    let start = channels(accent);
    let end = channels(toward);
    for step in 0..=32 {
        let color = pack(std::array::from_fn(|i| {
            start[i] + (end[i] - start[i]) * (step as f64 / 32.)
        }));
        if ratio(color, background) >= minimum {
            return color;
        }
    }
    toward
}

impl Preferences {
    fn set(&mut self, key: &str, value: OwnedValue) {
        match key {
            "color-scheme" => self.light = u32::try_from(value) == Ok(2),
            "contrast" => self.contrast = u32::try_from(value) == Ok(1),
            "reduced-motion" => self.reduced_motion = u32::try_from(value) == Ok(1),
            "accent-color" => {
                self.accent = <(f64, f64, f64)>::try_from(value)
                    .ok()
                    .map(|(r, g, b)| [r, g, b])
                    .filter(|rgb| rgb.iter().all(|v| (0.0..=1.0).contains(v)))
                    .map(pack)
            }
            _ => {}
        }
    }
    fn theme(&self, force_contrast: bool) -> Theme {
        let mut t = match (self.light, self.contrast || force_contrast) {
            (false, false) => Theme::DARK,
            (true, false) => Theme::LIGHT,
            (false, true) => Theme::HIGH_CONTRAST,
            (true, true) => Theme::LIGHT_HIGH_CONTRAST,
        };
        if let Some(accent) = self.accent {
            t.focus = legible(
                accent,
                t.text,
                t.panel,
                if t.high_contrast { 4.5 } else { 3. },
            );
            t.selection = legible(
                t.focus,
                t.panel,
                t.secondary,
                if t.high_contrast { 7. } else { 4.5 },
            );
        }
        t
    }
    fn apply(&self, force_contrast: bool, cx: &mut App) {
        let theme = self.theme(force_contrast);
        if cx.try_global::<Theme>() != Some(&theme) {
            cx.set_global(theme);
            cx.refresh_windows();
        }
        cx.set_reduce_motion(self.reduced_motion);
    }
}

async fn snapshot(proxy: &Proxy<'_>) -> Preferences {
    type Settings = HashMap<String, HashMap<String, OwnedValue>>;
    let mut prefs = Preferences::default();
    if let Ok(mut settings) = proxy
        .call::<_, _, Settings>("ReadAll", &([NAMESPACE].as_slice(),))
        .await
        && let Some(values) = settings.remove(NAMESPACE)
    {
        for (key, value) in values {
            prefs.set(&key, value)
        }
    }
    prefs
}

async fn watch(cx: &AsyncApp, force_contrast: bool) -> zbus::Result<()> {
    let bus = Connection::session().await?;
    let proxy = Proxy::new(
        &bus,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .await?;
    let owners = proxy.receive_owner_changed().await?;
    let changes = proxy
        .receive_signal_with_args("SettingChanged", &[(0, NAMESPACE)])
        .await?;
    // Subscribe before the initial read. Use the portal's authoritative snapshot
    // on ownership changes, including when it first appears after app launch.
    let mut prefs = snapshot(&proxy).await;
    cx.update(|cx| prefs.apply(force_contrast, cx));
    enum Event {
        Owner(bool),
        Setting(zbus::Message),
    }
    // Replay signals in bus order, including changes delivered while the
    // portal was preparing a snapshot. An old owner's signals precede its loss.
    let mut events = ordered_stream::join(
        owners.map(|name| Event::Owner(name.is_some())),
        changes.map(Event::Setting),
    );
    while let Some(event) = events.next().await {
        match event {
            Event::Owner(true) => prefs = snapshot(&proxy).await,
            Event::Owner(false) => prefs = Preferences::default(),
            Event::Setting(msg) => {
                if let Ok((_, key, value)) =
                    msg.body().deserialize::<(String, String, OwnedValue)>()
                {
                    prefs.set(&key, value);
                }
            }
        }
        cx.update(|cx| prefs.apply(force_contrast, cx));
    }
    Ok(())
}

/// Launch a suite app with GPUI's Linux accessibility bridge and live appearance.
pub fn run(launch: impl FnOnce(&mut App) + 'static) {
    Application::with_platform(gpui_linux::current_platform(false)).run(move |cx| {
        let force_contrast =
            std::env::var_os("MUNDANE_HIGH_CONTRAST").as_deref() == Some("1".as_ref());
        Preferences::default().apply(force_contrast, cx);
        cx.spawn(async move |cx| {
            let _ = watch(cx, force_contrast).await;
            cx.update(|cx| Preferences::default().apply(force_contrast, cx));
        })
        .detach();
        launch(cx)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn accent(rgb: (f64, f64, f64)) -> OwnedValue {
        Value::from(rgb).try_into().unwrap()
    }

    #[test]
    fn independent_preferences_and_unset_values() {
        let mut p = Preferences::default();
        p.set("color-scheme", 2u32.into());
        p.set("accent-color", accent((0.6, 0.2, 0.8)));
        p.set("contrast", 1u32.into());
        p.set("reduced-motion", 1u32.into());
        let t = p.theme(false);
        assert!(t.light && t.high_contrast && p.reduced_motion);
        assert_eq!(t.focus, 0x9933cc);
        for bad in [(1.1, 0., 0.), (0., -0.1, 0.), (0., 0., f64::NAN)] {
            p.set("accent-color", accent(bad));
            assert_eq!(p.theme(false), Theme::LIGHT_HIGH_CONTRAST);
        }
        p.set("contrast", 17u32.into());
        p.set("color-scheme", 0u32.into());
        p.set("reduced-motion", 99u32.into());
        assert_eq!(p.theme(false), Theme::DARK);
        assert_eq!(p.theme(true), Theme::HIGH_CONTRAST);
        assert!(!p.reduced_motion);
    }

    #[test]
    fn extreme_accents_keep_focus_and_selected_text_legible() {
        for light in [true, false] {
            for contrast in [true, false] {
                for accent in [0, 0xffffff, 0xf8faf8, 0x17201e, 0xffff00, 0x0000ff] {
                    let t = Preferences {
                        light,
                        contrast,
                        accent: Some(accent),
                        ..Default::default()
                    }
                    .theme(false);
                    let minimum = if contrast { 4.5 } else { 3. };
                    assert!(
                        ratio(t.focus, t.surface) >= minimum && ratio(t.focus, t.panel) >= minimum
                    );
                    for ink in [t.text, t.secondary] {
                        assert!(ratio(t.selection, ink) >= if contrast { 7. } else { 4.5 });
                    }
                }
            }
        }
    }
}
