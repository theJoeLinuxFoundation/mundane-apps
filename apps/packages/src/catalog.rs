//! AppStream XML supplies catalogue metadata. Only the local desktop registry can launch an app.
use anyhow::{Context, Result, ensure};
use gio_unix::{DesktopAppInfo, prelude::*};
use mundane_services::provider;
use roxmltree::Node;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;

#[derive(Default, Debug, Clone)]
pub(super) struct Application {
    pub name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub package: Option<String>,
    pub desktop: Option<String>,
    pub installed: bool,
    pub screenshot: Option<(Url, String)>,
}
fn text<'a>(node: Node<'a, '_>, tag: &str) -> Option<&'a str> {
    node.children()
        .find(|n| {
            n.has_tag_name(tag)
                && n.attribute(("http://www.w3.org/XML/1998/namespace", "lang"))
                    .is_none()
        })?
        .text()
}
fn category_names<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut result: Vec<_> = values
        .filter_map(|name| {
            Some(
                match name {
                    "AudioVideo" | "Audio" | "Video" => "Sound and video",
                    "Development" => "Development",
                    "Education" => "Education",
                    "Game" => "Games",
                    "Graphics" => "Graphics",
                    "Network" => "Internet",
                    "Office" => "Office",
                    "Science" => "Science",
                    "Settings" => "Settings",
                    "System" => "System",
                    "Utility" => "Utilities",
                    _ => return None,
                }
                .to_owned(),
            )
        })
        .collect();
    result.sort();
    result.dedup();
    result
}
fn parse(xml: &str, source: &Path, apps: &mut BTreeMap<String, Application>) -> Result<()> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();
    let base = root
        .attribute("media_baseurl")
        .and_then(|s| Url::parse(&format!("{}/", s.trim_end_matches('/'))).ok())
        .or_else(|| Url::from_directory_path(source.parent()?).ok());
    for c in root.descendants().filter(|n| {
        n.has_tag_name("component") && n.attribute("type") == Some("desktop-application")
    }) {
        let Some(id) = text(c, "id") else { continue };
        let Some(name) = text(c, "name") else {
            continue;
        };
        let entry = c
            .children()
            .find(|n| n.has_tag_name("launchable") && n.attribute("type") == Some("desktop-id"))
            .and_then(|n| n.text());
        let desktop = entry
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}.desktop", id.trim_end_matches(".desktop")));
        if !desktop.ends_with(".desktop") || desktop.contains('/') {
            continue;
        }
        let description = c
            .children()
            .find(|n| n.has_tag_name("description"))
            .map(|n| {
                n.descendants()
                    .filter(|p| {
                        p.has_tag_name("p")
                            && p.attribute(("http://www.w3.org/XML/1998/namespace", "lang"))
                                .is_none()
                    })
                    .map(|p| {
                        p.descendants()
                            .filter(Node::is_text)
                            .filter_map(|n| n.text())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .unwrap_or_default();
        let screenshot = c
            .descendants()
            .filter(|n| n.has_tag_name("screenshot"))
            .find_map(|s| {
                let image = s
                    .children()
                    .filter(|n| n.has_tag_name("image"))
                    .min_by_key(|n| n.attribute("type") != Some("thumbnail"))?;
                let uri = base.as_ref()?.join(image.text()?.trim()).ok()?;
                let caption = text(s, "caption").unwrap_or(name).to_owned();
                matches!(uri.scheme(), "https" | "file").then_some((uri, caption))
            });
        let app = apps.entry(desktop.clone()).or_default();
        app.name = name.to_owned();
        if let Some(v) = text(c, "summary") {
            app.summary = v.into();
        }
        if !description.is_empty() {
            app.description = description;
        }
        app.categories.extend(category_names(
            c.descendants()
                .filter(|n| n.has_tag_name("category"))
                .filter_map(|n| n.text()),
        ));
        app.categories.sort();
        app.categories.dedup();
        if let Some(v) = text(c, "pkgname") {
            app.package = Some(v.into());
        }
        if screenshot.is_some() {
            app.screenshot = screenshot;
        }
        app.desktop = Some(desktop);
    }
    Ok(())
}
fn read_xml(path: &Path) -> Result<String> {
    let file = fs::File::open(path)?;
    let input: Box<dyn Read> = if path.extension().is_some_and(|e| e == "gz") {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut xml = String::new();
    input.take(64 * 1024 * 1024 + 1).read_to_string(&mut xml)?;
    ensure!(xml.len() <= 64 * 1024 * 1024, "Catalog exceeds 64 MiB");
    Ok(xml)
}
pub(super) fn discover() -> Vec<Application> {
    let mut apps = BTreeMap::new();
    let mut roots: Vec<_> = std::env::split_paths(
        &std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    )
    .collect();
    roots.reverse();
    if roots.iter().any(|p| p == Path::new("/usr/share")) {
        roots.insert(0, PathBuf::from("/var/cache"));
        roots.insert(0, PathBuf::from("/var/lib"));
    }
    if let Some(home) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
    {
        roots.push(home);
    }
    for root in roots {
        for dir in ["metainfo", "appdata", "app-info/xmls", "swcatalog/xml"] {
            let mut paths: Vec<_> = fs::read_dir(root.join(dir))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            paths.sort();
            for p in paths {
                if p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|n| n.ends_with(".xml") || n.ends_with(".xml.gz"))
                    && let Ok(xml) = read_xml(&p)
                {
                    let _ = parse(&xml, &p, &mut apps);
                }
            }
        }
    }
    for launcher in gio::AppInfo::all() {
        let Ok(launcher) = launcher.downcast::<DesktopAppInfo>() else {
            continue;
        };
        let Some(id) = launcher.id() else { continue };
        if !launcher.should_show() || launcher.is_hidden() {
            apps.remove(id.as_str());
            continue;
        }
        let app = apps.entry(id.to_string()).or_default();
        app.installed = true;
        app.desktop = Some(id.into());
        if app.name.is_empty() {
            app.name = launcher.display_name().into();
        }
        if app.summary.is_empty() {
            app.summary = launcher.description().map(Into::into).unwrap_or_default();
        }
        if app.categories.is_empty() {
            app.categories =
                category_names(launcher.categories().as_deref().unwrap_or("").split(';'));
        }
    }
    let mut apps: Vec<_> = apps.into_values().collect();
    for a in &mut apps {
        if a.categories.is_empty() {
            a.categories.push("Other".into());
        }
    }
    apps.sort_by_cached_key(|a| a.name.to_lowercase());
    apps
}
pub(super) fn launch(id: &str) -> Result<()> {
    let app = DesktopAppInfo::new(id).context("Application launcher is no longer installed")?;
    ensure!(
        app.should_show() && !app.is_hidden(),
        "Application launcher is hidden"
    );
    app.launch(&[], None::<&gio::AppLaunchContext>)?;
    Ok(())
}
pub(super) fn screenshot(url: &Url) -> Result<Vec<u8>> {
    const LIMIT: usize = 8 * 1024 * 1024;
    let bytes = match url.scheme() {
        "file" => {
            let f = fs::File::open(
                url.to_file_path()
                    .ok()
                    .context("Invalid local screenshot")?,
            )?;
            ensure!(f.metadata()?.is_file(), "Screenshot must be a regular file");
            let mut bytes = Vec::new();
            f.take(LIMIT as u64 + 1).read_to_end(&mut bytes)?;
            bytes
        }
        "https" => {
            let response = provider::get(
                url,
                &provider::agent("packages"),
                None,
                LIMIT,
                &provider::cache_dir("packages")?.join("screenshots"),
                Duration::from_millis(250),
            )?;
            ensure!(
                response.status == 200,
                "Screenshot server returned {}",
                response.status
            );
            response.body
        }
        _ => anyhow::bail!("Unsupported screenshot location"),
    };
    ensure!(bytes.len() <= LIMIT, "Screenshot exceeds 8 MiB");
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    let (w, h) = reader.into_dimensions()?;
    ensure!(
        u64::from(w) * u64::from(h) <= 12_000_000,
        "Screenshot exceeds 12 megapixels"
    );
    let thumb = image::load_from_memory(&bytes)?.thumbnail(800, 450);
    let mut png = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_collection_and_groups_without_guessing_package_names() {
        let mut apps = BTreeMap::new();
        let xml = r#"<components media_baseurl="https://example.test/media">
          <component type="desktop-application"><id>org.test.Draw</id><name>Draw</name>
          <summary>Make diagrams</summary><description><p>Draw <em>and</em> edit.</p></description>
          <launchable type="desktop-id">draw.desktop</launchable><categories><category>Graphics</category></categories>
          <screenshots><screenshot><image>overview.png</image></screenshot></screenshots></component>
          <component type="desktop-application"><id>ignored</id><name>Invalid</name><launchable type="desktop-id">../bad.desktop</launchable></component>
          </components>"#;
        parse(
            xml,
            Path::new("/usr/share/swcatalog/xml/joe.xml"),
            &mut apps,
        )
        .unwrap();
        assert_eq!(apps.len(), 1);
        let app = &apps["draw.desktop"];
        assert_eq!(app.description, "Draw and edit.");
        assert_eq!(app.categories, ["Graphics"]);
        assert_eq!(
            app.screenshot.as_ref().unwrap().0.as_str(),
            "https://example.test/media/overview.png"
        );
        assert!(app.package.is_none() && !app.installed);
    }
}
