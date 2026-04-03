use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixDatagram;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

mod keymap;

const DEFAULT_PORT: u16 = 7070;
const HID_DEVICE: &str = "/dev/hidg0";
const NULL_REPORT: [u8; 8] = [0; 8];

fn send_report(hid: &mut std::fs::File, modifier: u8, keycode: u8) -> Result<()> {
    let report = [modifier, 0, keycode, 0, 0, 0, 0, 0];
    hid.write_all(&report).context("write HID report")?;
    hid.write_all(&NULL_REPORT).context("write HID release")?;
    Ok(())
}

/// Notify systemd watchdog on a background thread.
/// Does nothing if NOTIFY_SOCKET is not set.
fn start_watchdog() {
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else { return };
    thread::spawn(move || {
        let sock = UnixDatagram::unbound().expect("create notify socket");
        loop {
            let _ = sock.send_to(b"WATCHDOG=1", &path);
            thread::sleep(Duration::from_secs(10));
        }
    });
}

fn run() -> Result<()> {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    start_watchdog();

    let listener = TcpListener::bind(("0.0.0.0", port))
        .with_context(|| format!("bind to port {port}"))?;

    eprintln!("zerokb listening on :{port}");

    for stream in listener.incoming() {
        let stream = stream.context("accept connection")?;
        let peer = stream.peer_addr().ok();
        eprintln!(
            "connection from {}",
            peer.map_or("unknown".into(), |a: std::net::SocketAddr| a.to_string())
        );

        let mut hid = OpenOptions::new()
            .write(true)
            .open(HID_DEVICE)
            .with_context(|| format!("open {HID_DEVICE}"))?;

        let mut buf = [0u8; 1];
        let mut stream = stream;
        while stream.read(&mut buf).context("read byte")? > 0 {
            let ch = buf[0] as char;
            if let Some((modifier, keycode)) = keymap::lookup(ch) {
                send_report(&mut hid, modifier, keycode)?;
            }
        }

        eprintln!("connection closed");
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("zerokb: {e:#}");
        std::process::exit(1);
    }
}
