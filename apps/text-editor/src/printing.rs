use super::*;
use std::{
    ffi::{c_char, c_int},
    fs::File,
    io::{self, Seek},
    os::fd::AsRawFd,
};
unsafe extern "C" {
    fn mundane_text_pdf(fd: c_int, text: *const c_char, len: usize) -> c_int;
}
fn render(text: &str) -> io::Result<File> {
    let mut file = tempfile::tempfile()?;
    let text = text.replace('\0', "␀");
    if unsafe { mundane_text_pdf(file.as_raw_fd(), text.as_ptr().cast(), text.len()) } != 0 {
        return Err(io::Error::other("Could not render the document."));
    }
    file.rewind()?;
    Ok(file)
}
impl Editor {
    pub(super) fn print(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let text = self.page().body.read(cx).snapshot();
        let name = self.name();
        self.wait("Preparing print…", w, cx);
        let job = cx.background_executor().spawn(async move {
            use ashpd::desktop::print::{PrintOptions, PrintProxy};
            let file = render(&text)?;
            let portal = PrintProxy::new().await?;
            match portal
                .print(None, &name, &file, PrintOptions::default())
                .await
                .and_then(|r| r.response())
            {
                Ok(()) | Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                    Ok::<_, anyhow::Error>(())
                }
                Err(e) => Err(e.into()),
            }
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = job.await;
            let _ = v.update_in(cx, |s, w, cx| {
                s.ready(w, cx);
                if let Err(e) = result {
                    s.error = Some(format!("Could not print. {e}"));
                }
                cx.notify()
            });
        })
        .detach();
    }
}
