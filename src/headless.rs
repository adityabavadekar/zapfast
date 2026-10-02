//! `zapfast --headless`: the backend without a window, for linking and
//! syncing from a terminal. The QR code prints to stdout; the log (stderr and
//! the log file) carries everything else. It runs until interrupted.

use std::time::Duration;

use crate::backend::{Backend, Event, LinkStatus, Waker};
use crate::paths::AppDirs;

/// Links and syncs until the process is stopped.
pub fn run(dirs: AppDirs, waker: Waker) {
    let mut backend = Backend::spawn(dirs, waker);
    if let Some(start) = backend.take_startup() {
        let _ = start.send(());
    }
    println!("ZapFast is running without a window. Press Ctrl+C to stop.");
    let mut shown_qr: Option<String> = None;
    let mut last_progress = None;
    loop {
        for event in backend.poll() {
            match event {
                Event::Link(LinkStatus::Unlinked { qr: Some(qr), .. }) => {
                    if shown_qr.as_ref() != Some(&qr) {
                        print_qr(&qr);
                        shown_qr = Some(qr);
                    }
                }
                Event::Link(LinkStatus::Connected) => println!("Linked and connected."),
                Event::Link(LinkStatus::LoggedOut) => {
                    println!("The phone unlinked this device. Restart to link again.");
                }
                Event::Link(LinkStatus::Failed(reason)) => println!("Could not start: {reason}"),
                Event::SyncProgress(progress) if last_progress != Some(progress) => {
                    println!("History sync {progress}%");
                    last_progress = Some(progress);
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Prints a QR code to scan from the phone, two rows per text line.
fn print_qr(text: &str) {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        println!("Could not draw the QR code.");
        return;
    };
    let image = code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    println!("\nScan with WhatsApp on your phone: Linked devices > Link a device\n\n{image}\n");
}
