//! Screenshots while a note records: one click grabs the whole screen the
//! pointer is on, at once, and the saving happens afterwards on a thread of
//! its own. A picture is kept without loss when the screen is mostly
//! interface, as a JPEG when it's a video or a photo, and scaled down when
//! the screen is very wide, so it stays sharp and small. Windows' own text
//! recognition reads the words on it, on this PC, for the note's summary.
//! Pictures live beside the note's audio and never leave the PC.

use crate::managers::notes_store::{self as store, NoteShot};
use crate::notes::{self, manager};
use crate::notes_rules;
use crate::screen::{self, Frame};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// JPEG quality for a screen that doesn't compress without loss: high
/// enough that text stays sharp.
const JPEG_QUALITY: u8 = 88;

/// Sent to every window the moment a screenshot is grabbed.
const TAKEN_EVENT: &str = "note-shot-taken";

/// The screenshot shortcut: takes one, or says on the bar that screenshots
/// go into a note that's recording.
pub fn shortcut(app: &AppHandle) {
    if let Err(code) = take(app) {
        if code == "idle" {
            crate::overlay::show_overlay_notice(
                app,
                &crate::transforms::BarNotice::NothingRecording,
            );
        }
    }
}

/// Takes a screenshot for the note being recorded. The screen is grabbed
/// before this returns; the picture is saved and read afterwards. Errors
/// are codes the page words: "idle" when nothing records, "failed" when
/// Windows didn't hand over the screen.
pub fn take(app: &AppHandle) -> Result<(), String> {
    let (note_id, at_ms) = notes::recording_position().ok_or_else(|| "idle".to_string())?;
    let started = Instant::now();
    let frame = screen::grab_pointer_screen().map_err(|e| {
        warn!("Couldn't take a screenshot: {e}");
        "failed".to_string()
    })?;
    debug!(
        "Grabbed a {}x{} screen in {:?}",
        frame.width,
        frame.height,
        started.elapsed()
    );
    // The camera buttons show a tick, whichever of them or the shortcut
    // took it.
    let _ = app.emit(TAKEN_EVENT, note_id);
    let app = app.clone();
    std::thread::spawn(move || match save(&app, note_id, at_ms, &frame) {
        Ok(shot) => {
            info!(
                "Saved a screenshot of {} KB in {:?}",
                shot.bytes / 1024,
                started.elapsed()
            );
            manager(&app).changed();
            read(&app, &shot, &frame);
        }
        Err(e) => warn!("Couldn't save a screenshot: {e}"),
    });
    Ok(())
}

/// Encodes the frame, writes it beside the note's audio and saves its row.
fn save(app: &AppHandle, note_id: i64, at_ms: i64, frame: &Frame) -> Result<NoteShot> {
    let (bytes, ext, size) = encode(frame)?;
    let m = manager(app);
    let dir = m.audio_dir();
    let mut n = 0;
    let name = loop {
        let name = notes_rules::shot_file_name(note_id, at_ms, n, ext);
        if !dir.join(&name).exists() {
            break name;
        }
        n += 1;
    };
    let conn = m.conn()?;
    // The note may have stopped and been dropped while the picture was
    // made; then there's nothing to put it in.
    if store::get(&conn, note_id)?.is_none() {
        return Err(anyhow!("The note is gone"));
    }
    std::fs::write(dir.join(&name), &bytes)?;
    store::add_shot(
        &conn,
        note_id,
        at_ms,
        &name,
        (size.0 as i64, size.1 as i64),
        bytes.len() as i64,
    )
}

#[cfg(not(windows))]
fn encode(_frame: &Frame) -> Result<(Vec<u8>, &'static str, (u32, u32))> {
    Err(anyhow!("Screenshots need Windows"))
}

/// The picture as a file: its bytes, its extension and its size. Lossless
/// WebP when that comes out small, which a screen of text and interface
/// does; else a high-quality JPEG, whichever is smaller.
#[cfg(windows)]
fn encode(frame: &Frame) -> Result<(Vec<u8>, &'static str, (u32, u32))> {
    use image::codecs::jpeg::JpegEncoder;
    use image::codecs::webp::WebPEncoder;
    use image::{imageops, ExtendedColorType, RgbImage};

    let rgb: Vec<u8> = frame
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    let mut picture = RgbImage::from_raw(frame.width, frame.height, rgb)
        .ok_or_else(|| anyhow!("The screen's pixels don't match its size"))?;
    let (width, height) = notes_rules::shot_size(frame.width, frame.height);
    if (width, height) != (frame.width, frame.height) {
        picture = imageops::resize(&picture, width, height, imageops::FilterType::CatmullRom);
    }
    let mut webp = Vec::new();
    WebPEncoder::new_lossless(&mut webp).encode(
        picture.as_raw(),
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    if webp.len() <= notes_rules::SHOT_LOSSLESS_MAX {
        return Ok((webp, "webp", (width, height)));
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY).encode(
        picture.as_raw(),
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(if jpeg.len() < webp.len() {
        (jpeg, "jpg", (width, height))
    } else {
        (webp, "webp", (width, height))
    })
}

/// Reads the words on the screen and keeps them with the screenshot.
fn read(app: &AppHandle, shot: &NoteShot, frame: &Frame) {
    let started = Instant::now();
    let text = match screen::recognise(frame) {
        Ok(text) => notes_rules::screen_text(&text),
        Err(e) => {
            debug!("No text read from a screenshot: {e}");
            return;
        }
    };
    if text.is_empty() {
        return;
    }
    let m = manager(app);
    match m
        .conn()
        .and_then(|c| store::set_shot_text(&c, shot.id, &text))
    {
        Ok(()) => {
            debug!(
                "Read {} characters on a screenshot in {:?}",
                text.chars().count(),
                started.elapsed()
            );
            m.changed();
        }
        Err(e) => warn!("Couldn't keep a screenshot's text: {e}"),
    }
}

/// Removes a screenshot from its note; returns it, for Undo. The picture
/// goes once Undo has.
pub fn delete(app: &AppHandle, id: i64) -> Result<Option<NoteShot>> {
    let m = manager(app);
    let shot = store::delete_shot(&m.conn()?, id)?;
    if let Some(shot) = &shot {
        m.changed();
        forget_file_later(app, shot.file.clone());
    }
    Ok(shot)
}

/// Puts a removed screenshot back.
pub fn restore(app: &AppHandle, shot: &NoteShot) -> Result<()> {
    let m = manager(app);
    if !picture_path(m.audio_dir(), &shot.file).is_some_and(|p| p.exists()) {
        return Err(anyhow!("The picture is gone"));
    }
    store::restore_shot(&m.conn()?, shot)?;
    m.changed();
    Ok(())
}

/// Puts a screenshot on the clipboard as a picture.
pub fn copy(app: &AppHandle, id: i64) -> Result<()> {
    let m = manager(app);
    let conn = m.conn()?;
    let file: String = conn.query_row(
        "SELECT file FROM note_shot WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    let path = picture_path(m.audio_dir(), &file).ok_or_else(|| anyhow!("Not a screenshot"))?;
    let bytes = std::fs::read(path)?;
    write_picture(app, &bytes)
}

#[cfg(windows)]
fn write_picture(app: &AppHandle, bytes: &[u8]) -> Result<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let picture = image::load_from_memory(bytes)?.to_rgba8();
    let (width, height) = picture.dimensions();
    app.clipboard()
        .write_image(&tauri::image::Image::new_owned(
            picture.into_raw(),
            width,
            height,
        ))
        .map_err(|e| anyhow!("{e}"))
}

#[cfg(not(windows))]
fn write_picture(_app: &AppHandle, _bytes: &[u8]) -> Result<()> {
    Err(anyhow!("Screenshots need Windows"))
}

/// How long a removed screenshot's picture waits for Undo.
const UNDO_FOR: std::time::Duration = std::time::Duration::from_secs(60);

fn forget_file_later(app: &AppHandle, file: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(UNDO_FOR);
        let m = manager(&app);
        let in_use = m
            .conn()
            .and_then(|c| store::shot_file_in_use(&c, &file))
            .unwrap_or(true);
        if !in_use {
            remove_pictures(m.audio_dir(), &[file]);
        }
    });
}

/// A screenshot's path, when its name is a bare file name: never a path
/// that could lead out of the notes folder.
pub fn picture_path(dir: &Path, file: &str) -> Option<PathBuf> {
    let name = Path::new(file);
    (name.file_name() == Some(name.as_os_str()) && file.contains("-shot-")).then(|| dir.join(name))
}

/// Removes pictures from the notes folder, for a note that's gone for good.
pub fn remove_pictures(dir: &Path, files: &[String]) {
    for file in files {
        let Some(path) = picture_path(dir, file) else {
            continue;
        };
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => debug!("Couldn't remove a screenshot: {e}"),
        }
    }
}

/// A screenshot as a JPEG, for a Word document, which shows no WebP.
pub fn as_jpeg(dir: &Path, shot: &NoteShot) -> Option<Vec<u8>> {
    let path = picture_path(dir, &shot.file)?;
    let bytes = std::fs::read(&path).ok()?;
    if shot.file.ends_with(".jpg") {
        return Some(bytes);
    }
    #[cfg(windows)]
    {
        use image::codecs::jpeg::JpegEncoder;
        let picture = image::load_from_memory(&bytes).ok()?.to_rgb8();
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
            .encode(
                picture.as_raw(),
                picture.width(),
                picture.height(),
                image::ExtendedColorType::Rgb8,
            )
            .ok()?;
        Some(jpeg)
    }
    #[cfg(not(windows))]
    {
        None
    }
}
