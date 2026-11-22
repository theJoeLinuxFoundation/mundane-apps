use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Profile {
    pub name: String,
    pub family: String,
    pub size: u16,
    pub history: usize,
    pub palette: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub login: bool,
    #[serde(default)]
    pub shortcuts: String,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            command: String::new(),
            login: false,
            shortcuts: String::new(),
            family: "DejaVu Sans Mono".into(),
            size: 16,
            history: 10000,
            palette: "Desktop".into(),
        }
    }
}
impl Profile {
    pub fn configure(&self, opts: &mut Options) {
        if self.command.is_empty() && !self.login {
            return;
        }
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let mut args = vec![];
        if self.login {
            args.push("-l".into());
        }
        if !self.command.is_empty() {
            args.extend(["-c".into(), self.command.clone()]);
        }
        opts.shell = Some(alacritty_terminal::tty::Shell::new(shell, args));
    }
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.family.trim().is_empty()
            || self.family.len() > 160
        {
            return Err("Enter a profile name and font family.".into());
        }
        if !(10..=40).contains(&self.size) || self.history > 100000 {
            return Err("Use a font size from 10 to 40 and up to 100,000 scrollback lines.".into());
        }
        if !matches!(self.palette.as_str(), "Desktop" | "Dark" | "Light") {
            return Err("Choose Desktop, Dark or Light.".into());
        }
        bindings(&self.shortcuts)?;
        if self.command.contains('\0') {
            return Err("A startup command cannot contain NUL.".into());
        }
        Ok(())
    }
    pub fn theme(&self, desktop: Theme) -> Theme {
        if desktop.high_contrast {
            return desktop;
        }
        match self.palette.as_str() {
            "Light" => Theme::LIGHT,
            "Dark" => Theme::DARK,
            _ => desktop,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Profiles {
    default: String,
    items: BTreeMap<String, Profile>,
}
impl Default for Profiles {
    fn default() -> Self {
        let p = Profile::default();
        Self {
            default: p.name.clone(),
            items: BTreeMap::from([(p.name.clone(), p)]),
        }
    }
}
impl Profiles {
    pub fn selected(&self) -> &Profile {
        &self.items[&self.default]
    }
    fn path() -> io::Result<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or_else(|| io::Error::other("No configuration directory"))?
            .join("mundane-terminal/profiles.json"))
    }
    fn read(path: &Path) -> io::Result<Self> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let set: Self = serde_json::from_reader(file)?;
        if !set.items.contains_key(&set.default)
            || set
                .items
                .iter()
                .any(|(k, p)| k != &p.name || p.validate().is_err())
        {
            return Err(io::Error::other("Invalid profile file"));
        }
        Ok(set)
    }
    pub fn load() -> io::Result<Self> {
        Self::read(&Self::path()?)
    }
    fn update(path: &Path, p: Profile, remove: bool) -> anyhow::Result<Self> {
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.lock()?;
        let mut set = Self::read(path)?;
        if remove {
            anyhow::ensure!(set.items.len() > 1, "Keep at least one saved profile.");
            anyhow::ensure!(
                set.items.remove(&p.name).is_some(),
                "Select a saved profile to remove."
            );
            if set.default == p.name {
                set.default = set.items.keys().next().unwrap().clone();
            }
        } else {
            p.validate().map_err(anyhow::Error::msg)?;
            set.default = p.name.clone();
            set.items.insert(p.name.clone(), p);
        }
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut tmp, &set)?;
        tmp.as_file().sync_all()?;
        tmp.persist(path)?;
        Ok(set)
    }
}
pub(super) struct Form {
    fields: [Entity<TextField>; 6],
    palette: String,
    login: bool,
    login_focus: FocusHandle,
    names: Vec<(String, FocusHandle)>,
    buttons: [FocusHandle; 4],
    error: Option<String>,
}
impl Form {
    fn fields(p: &Profile, cx: &mut Context<Terminal>) -> [Entity<TextField>; 6] {
        let t = Theme::current(cx);
        [
            ("Profile name", p.name.clone()),
            ("Font family", p.family.clone()),
            ("Font size", p.size.to_string()),
            ("Scrollback lines", p.history.to_string()),
            ("Startup command", p.command.clone()),
            ("Window shortcuts", p.shortcuts.clone()),
        ]
        .map(|(label, text)| cx.new(|cx| TextField::new(label, t, cx).select_value(text)))
    }
}
impl Terminal {
    pub fn edit_profiles(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let mut error = None;
        match Profiles::load() {
            Ok(set) => self.profiles = set,
            Err(e) => error = Some(e.to_string()),
        }
        let p = &self.tabs[self.active].profile;
        let fields = Form::fields(p, cx);
        fields[0].read(cx).focus_handle(cx).focus(w, cx);
        self.form = Some(Form {
            fields,
            login: p.login,
            login_focus: cx.focus_handle().tab_stop(true),
            palette: p.palette.clone(),
            names: self
                .profiles
                .items
                .keys()
                .map(|k| (k.clone(), cx.focus_handle().tab_stop(true)))
                .collect(),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            error,
        });
        cx.notify();
    }
    fn profile_action(&mut self, index: usize, w: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = &mut self.form else { return };
        if index == 0 {
            f.palette = match f.palette.as_str() {
                "Desktop" => "Dark",
                "Dark" => "Light",
                _ => "Desktop",
            }
            .into();
            cx.notify();
            return;
        }
        if index == 1 {
            self.form = None;
            self.grid_focus.focus(w, cx);
            cx.notify();
            return;
        }
        let result = (|| -> anyhow::Result<_> {
            let value = |i: usize| f.fields[i].read(cx).text().trim().to_string();
            let p = if index == 2 {
                Profile {
                    name: value(0),
                    ..Default::default()
                }
            } else {
                Profile {
                    name: value(0),
                    family: value(1),
                    size: value(2)
                        .parse()
                        .map_err(|_| anyhow::anyhow!("Enter a whole-number font size."))?,
                    history: value(3).parse().map_err(|_| {
                        anyhow::anyhow!("Enter a whole number of scrollback lines.")
                    })?,
                    palette: f.palette.clone(),
                    command: value(4),
                    login: f.login,
                    shortcuts: value(5),
                }
            };
            Profiles::update(&Profiles::path()?, p, index == 2)
        })();
        match result {
            Ok(set) => {
                let profile = set.selected().clone();
                self.tabs[self.active].apply_profile(profile.clone());
                self.layout.font_size = profile.size.into();
                self.profiles = set;
                self.form = None;
                self.grid_focus.focus(w, cx);
            }
            Err(e) => f.error = Some(e.to_string()),
        }
        cx.notify()
    }
    pub fn profile_dialog(&self, f: &Form, cx: &Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let controls = div().flex().gap_1().children(
            ["Cancel", "Delete", "Save and use"]
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    let v = cx.weak_entity();
                    button(label, &f.buttons[i + 1], t, move |w, cx| {
                        let _ = v.update(cx, |s, cx| s.profile_action(i + 1, w, cx));
                    })
                }),
        );
        let v = cx.weak_entity();
        let palette = mundane_ui::button_base(
            format!("Colors: {}", f.palette),
            &f.buttons[0],
            t,
            move |w, cx| {
                let _ = v.update(cx, |s, cx| s.profile_action(0, w, cx));
            },
        )
        .px_2()
        .child(f.palette.clone());
        let names = div()
            .id("profile-names")
            .flex()
            .gap_1()
            .overflow_x_scroll()
            .children(f.names.iter().map(|(name, focus)| {
                let v = cx.weak_entity();
                let key = name.clone();
                mundane_ui::button_base(name.clone(), focus, t, move |w, cx| {
                    let _ = v.update(cx, |s, cx| {
                        let p = &s.profiles.items[&key];
                        if let Some(f) = &mut s.form {
                            f.fields = Form::fields(p, cx);
                            f.palette = p.palette.clone();
                            f.login = p.login;
                            f.error = None;
                            f.fields[0].read(cx).focus_handle(cx).focus(w, cx);
                        }
                        cx.notify();
                    });
                })
                .px_2()
                .child(name.clone())
            }));
        mundane_ui::Role::Dialog
            .control("profiles", "Terminal profiles")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(rems(34.))
                    .max_w_full()
                    .max_h_full()
                    .id("profile-settings")
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(t.panel))
                    .child(mundane_ui::heading("profiles-title", "Terminal profiles"))
                    .child(names)
                    .children(
                        [
                            "Profile name",
                            "Font family",
                            "Font size",
                            "Scrollback lines", "Startup command","Window shortcuts",
                        ]
                        .into_iter()
                        .zip(&f.fields)
                        .map(|(label, field)| {
                            div()
                                .flex()
                                .items_center()
                                .child(div().w(px(145.)).child(label))
                                .child(div().flex_1().child(field.clone()))
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(div().w(px(145.)).child("Colors"))
                            .child(palette),
                    )
                    .child(mundane_ui::toggle("Login shell", &f.login_focus,f.login,true,t,{
                        let view = cx.weak_entity();
                        move |login, _, cx| {
                            let _ = view.update(cx, |terminal,cx| {
                                if let Some(form) = &mut terminal.form {
                                    form.login = login;
                                    cx.notify();
                                }
                            });
                        }
                    }))
                    .child("New shells use the startup command. Override tab, window, detach, close or profiles with entries like window=ctrl-alt-n.")
                    .when_some(f.error.clone(), |el, e| {
                        el.child(
                            mundane_ui::Role::Status
                                .control("profile-error", e.clone())
                                .child(e),
                        )
                    })
                    .child(controls),
            )
    }
    pub fn form_key(&mut self, k: &Keystroke, w: &mut Window, cx: &mut Context<Self>) -> bool {
        match k.key.as_str() {
            "escape" => self.profile_action(1, w, cx),
            "tab" => {
                let f = self.form.as_ref().unwrap();
                let handles: Vec<_> = f
                    .names
                    .iter()
                    .map(|(_, h)| h.clone())
                    .chain(f.fields.iter().map(|field| field.read(cx).focus_handle(cx)))
                    .chain(std::iter::once(f.login_focus.clone()))
                    .chain(f.buttons.iter().cloned())
                    .collect();
                let i = handles.iter().position(|h| h.is_focused(w)).unwrap_or(0);
                let delta = if k.modifiers.shift {
                    handles.len() - 1
                } else {
                    1
                };
                handles[(i + delta) % handles.len()].focus(w, cx);
            }
            "enter"
                if self.form.as_ref().is_some_and(|f| {
                    f.fields
                        .iter()
                        .any(|field| field.read(cx).focus_handle(cx).is_focused(w))
                }) =>
            {
                self.profile_action(3, w, cx)
            }
            _ => return false,
        }
        true
    }
}

const SHORTCUTS: [(&str, &str); 5] = [
    ("tab", "ctrl-shift-t"),
    ("window", "ctrl-shift-n"),
    ("detach", "ctrl-shift-d"),
    ("close", "ctrl-shift-w"),
    ("profiles", "ctrl-shift-p"),
];
pub(super) fn bindings(value: &str) -> Result<BTreeMap<&'static str, String>, String> {
    let mut map: BTreeMap<_, _> = SHORTCUTS
        .into_iter()
        .map(|(a, k)| (a, k.to_owned()))
        .collect();
    for part in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (action, stroke) = part
            .split_once('=')
            .ok_or("Use action=shortcut, separated by commas.")?;
        let slot = map
            .get_mut(action.trim())
            .ok_or("Unknown window shortcut action.")?;
        let k = Keystroke::parse_with_os(stroke.trim(), OperatingSystem::Linux)
            .map_err(|_| "Invalid shortcut.")?;
        if !k.modifiers.control && !k.modifiers.alt {
            return Err("A window shortcut needs Control or Alt.".into());
        }
        *slot = k.unparse(OperatingSystem::Linux);
    }
    if map
        .values()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != map.len()
    {
        return Err("Choose a different shortcut for each action.".into());
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::{Profile, Profiles, Theme};
    #[test]
    fn old_profiles_and_launch_shortcuts() {
        let mut p: Profile = serde_json::from_str(
            r#"{"name":"Work","family":"Mono","size":14,"palette":"Desktop","history":42}"#,
        )
        .unwrap();
        let mut opts = super::Options::default();
        p.configure(&mut opts);
        assert!(opts.shell.is_none());
        p.command = "printf '%s' 'two words'".into();
        p.login = true;
        p.configure(&mut opts);
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        assert_eq!(
            opts.shell.unwrap(),
            alacritty_terminal::tty::Shell::new(
                shell,
                vec!["-l".into(), "-c".into(), p.command.clone()]
            )
        );
        p.shortcuts = "window=ctrl-alt-n, tab=ctrl-alt-t".into();
        assert!(p.validate().is_ok());
        assert_eq!(
            super::bindings(&p.shortcuts).unwrap()["window"],
            "ctrl-alt-n"
        );
        for bad in ["window=ctrl-shift-t", "window=n", "unknown=ctrl-n"] {
            p.shortcuts = bad.into();
            assert!(p.validate().is_err());
        }
    }
    #[test]
    fn distinct_profiles_survive_updates_and_invalid_entries_keep_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let work = Profile {
            name: "Work".into(),
            size: 22,
            history: 300,
            palette: "Light".into(),
            ..Default::default()
        };
        Profiles::update(&path, work.clone(), false).unwrap();
        let read = Profiles::read(&path).unwrap();
        assert_eq!(read.selected(), &work);
        assert!(read.items.contains_key("Default"));
        assert_eq!(work.theme(Theme::DARK), Theme::LIGHT);
        assert_eq!(work.theme(Theme::HIGH_CONTRAST), Theme::HIGH_CONTRAST);
        assert!(
            Profiles::update(
                &path,
                Profile {
                    size: 1,
                    ..work.clone()
                },
                false
            )
            .is_err()
        );
        assert_eq!(Profiles::read(&path).unwrap().selected(), &work);
        assert_eq!(
            Profiles::update(&path, work, true).unwrap().selected(),
            &Profile::default()
        );
        assert!(Profiles::update(&path, Profile::default(), true).is_err());
    }
}
