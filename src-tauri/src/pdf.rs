//! Settings → Recipe book → Save as PDF: the window, in its print layout (the page has put the whole book
//! in it), printed straight to a file with AppKit's "save" job, without the print dialog.

use objc2_app_kit::{NSPrintInfo, NSPrintJobSavingURL, NSPrintSaveJob};
use objc2_foundation::NSURL;
use objc2_web_kit::WKWebView;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Page margins in points (72 to the inch), all four sides: the knob to turn if pages look cramped or clipped.
const MARGIN_PT: f64 = 36.0;

pub fn save(app: &AppHandle, to: &Path) -> Result<(), String> {
    let w = app.get_webview_window("main").ok_or("the Recipes window is closed")?;
    let target = to.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    // runs on the main thread, where AppKit and WebKit must be used
    w.with_webview(move |pw| {
        let _ = tx.send(start(pw.inner().cast(), &target));
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "the window didn't answer".to_string())??;
    // ponytail: no Objective-C delegate to hear when the job ends; the file is done once it ends with the
    // PDF trailer. Add a delegate (declare_class!) if this ever proves unreliable.
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(180) {
        std::thread::sleep(Duration::from_millis(300));
        if finished(to) {
            return Ok(());
        }
    }
    Err("the PDF wasn't written in time".into())
}

/// The file ends with `%%EOF`, which a PDF writer puts last.
fn finished(p: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(p) else { return false };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut tail = Vec::new();
    len > 0 && f.seek(SeekFrom::Start(len.saturating_sub(64))).is_ok() && f.read_to_end(&mut tail).is_ok() && tail.windows(5).any(|w| w == b"%%EOF")
}

fn start(webview: *const WKWebView, to: &Path) -> Result<(), String> {
    // SAFETY: tauri hands us its live WKWebView, on the main thread
    let wv = unsafe { &*webview };
    let url = NSURL::from_file_path(to).ok_or("that file name can't be used")?;
    // a new print info: changing the shared one would send the next ordinary Print to this file too
    let info = NSPrintInfo::new();
    info.setJobDisposition(unsafe { NSPrintSaveJob });
    // SAFETY: the dictionary's values are objects; the saving URL is an NSURL
    unsafe { info.dictionary().insert(NSPrintJobSavingURL, url.as_ref()) };
    info.setTopMargin(MARGIN_PT);
    info.setBottomMargin(MARGIN_PT);
    info.setLeftMargin(MARGIN_PT);
    info.setRightMargin(MARGIN_PT);
    // SAFETY: an ordinary print operation for this web view
    let op = unsafe { wv.printOperationWithPrintInfo(&info) };
    op.setShowsPrintPanel(false);
    op.setShowsProgressPanel(false);
    op.setCanSpawnSeparateThread(true);
    // without a frame, WebKit's print view has no size and the pages come out blank
    if let Some(v) = op.view() {
        v.setFrame(wv.bounds());
    }
    let window = wv.window().ok_or("the Recipes window is closed")?;
    // SAFETY: no delegate, so no selector or context is called back
    unsafe { op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(&window, None, None, std::ptr::null_mut()) };
    Ok(())
}
