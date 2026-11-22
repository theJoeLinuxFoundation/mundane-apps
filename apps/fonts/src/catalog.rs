use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CStr, CString, c_char, c_void},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::Arc,
};

type Visit = extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char, i32);
unsafe extern "C" {
    fn mundane_font_list(fixture: *const c_char, data: *mut c_void, visit: Visit) -> i32;
    fn mundane_font_open(file: *const c_char, index: i32) -> *mut c_void;
    fn mundane_font_close(preview: *mut c_void);
    fn mundane_font_missing(preview: *mut c_void, code: u32) -> i32;
    fn mundane_font_paint(
        preview: *mut c_void,
        text: *const c_char,
        length: i32,
        size: i32,
        width: i32,
        height: i32,
        fg: u32,
        bg: u32,
        pixels: *mut u8,
    ) -> i32;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub style: String,
    file: PathBuf,
    index: i32,
}
pub struct Family {
    pub name: String,
    pub faces: Vec<Face>,
}
type Inventory = BTreeMap<String, Vec<Face>>;
extern "C" fn visit(
    data: *mut c_void,
    family: *const c_char,
    style: *const c_char,
    file: *const c_char,
    index: i32,
) {
    // Fontconfig owns all three NUL-terminated strings for the duration of this callback.
    let (family, style, file) = unsafe {
        (
            CStr::from_ptr(family),
            CStr::from_ptr(style),
            CStr::from_ptr(file),
        )
    };
    let inventory = unsafe { &mut *data.cast::<Inventory>() };
    inventory
        .entry(family.to_string_lossy().into_owned())
        .or_default()
        .push(Face {
            style: style.to_string_lossy().into_owned(),
            file: std::ffi::OsStr::from_bytes(file.to_bytes()).into(),
            index,
        });
}
pub fn installed(dir: Option<&Path>) -> Result<Vec<Family>, String> {
    let dir = dir
        .map(|p| CString::new(p.as_os_str().as_bytes()))
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut inventory = Inventory::new();
    if unsafe {
        mundane_font_list(
            dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
            (&mut inventory as *mut Inventory).cast(),
            visit,
        )
    } == 0
    {
        return Err("Could not read the Fontconfig inventory.".into());
    }
    Ok(inventory
        .into_iter()
        .map(|(name, mut faces)| {
            let regular = |style: &str| matches!(style, "Regular" | "Book" | "Normal" | "Roman");
            faces.sort_by(|a, b| {
                regular(&b.style)
                    .cmp(&regular(&a.style))
                    .then(a.style.cmp(&b.style))
                    .then(a.file.cmp(&b.file))
                    .then(a.index.cmp(&b.index))
            });
            faces.dedup();
            Family { name, faces }
        })
        .collect())
}
pub struct Renderer(NonNull<c_void>);
impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { mundane_font_close(self.0.as_ptr()) }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub face: Face,
    pub sample: String,
    pub size: i32,
    pub width: i32,
    pub height: i32,
    pub foreground: u32,
    pub background: u32,
}
pub struct Output {
    pub image: Arc<RenderImage>,
    pub missing: Vec<char>,
    pub clipped: bool,
}
impl Renderer {
    pub fn open(face: &Face) -> Result<Self, String> {
        let file = CString::new(face.file.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        NonNull::new(unsafe { mundane_font_open(file.as_ptr(), face.index) })
            .map(Self)
            .ok_or_else(|| format!("Could not open {} in {}.", face.style, face.file.display()))
    }
    pub fn draw(&self, r: &Request) -> Result<Output, String> {
        if !(1..=4096).contains(&r.width)
            || !(1..=2048).contains(&r.height)
            || !(1..=512).contains(&r.size)
        {
            return Err("Preview dimensions are unavailable.".into());
        }
        let len = r
            .sample
            .len()
            .try_into()
            .map_err(|_| "Sample is too long.")?;
        let missing = r
            .sample
            .chars()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|&c| unsafe { mundane_font_missing(self.0.as_ptr(), c as u32) != 0 })
            .collect();
        let mut pixels = RgbaImage::new(r.width as u32, r.height as u32);
        let clipped = unsafe {
            mundane_font_paint(
                self.0.as_ptr(),
                r.sample.as_ptr().cast(),
                len,
                r.size,
                r.width,
                r.height,
                r.foreground,
                r.background,
                pixels.as_mut_ptr(),
            )
        };
        if clipped < 0 {
            return Err("Could not draw the font preview.".into());
        }
        Ok(Output {
            image: Arc::new(RenderImage::new([Frame::new(pixels)])),
            missing,
            clipped: clipped != 0,
        })
    }
}
#[cfg(test)]
pub const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fonts");
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enumerate_faces_draw_distinct_glyphs_and_report_coverage() {
        let families = installed(Some(Path::new(FIXTURE))).unwrap();
        assert_eq!(families.len(), 1);
        let f = &families[0];
        assert_eq!(f.name, "Survey Fixture");
        assert_eq!(
            f.faces.iter().map(|f| f.style.as_str()).collect::<Vec<_>>(),
            ["Regular", "Bold"]
        );
        let mut rasters = vec![];
        for face in &f.faces {
            let r = Renderer::open(face).unwrap();
            let request = Request {
                face: face.clone(),
                sample: "A Ω星".into(),
                size: 32,
                width: 240,
                height: 80,
                foreground: 0xffffff,
                background: 0,
            };
            let p = r.draw(&request).unwrap();
            assert_eq!(p.missing, ['星']);
            assert!(!p.clipped);
            rasters.push(p.image.as_bytes(0).unwrap().to_vec());
        }
        assert_ne!(rasters[0], rasters[1]);
    }
}
