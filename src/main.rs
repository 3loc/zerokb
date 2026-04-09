use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::UnixDatagram;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

mod keymap;

const DEFAULT_PORT: u16 = 7070;
/// Secondary TCP port for out-of-band control commands.
/// Accepts one line per connection; currently the only command is
/// "ABORT", which sets a shared flag that the typing loop checks
/// between bytes to short-circuit the current session.
const DEFAULT_CONTROL_PORT: u16 = 7071;
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

/// Control listener. Runs in its own thread, independent of the main
/// data listener so it stays responsive even when the data loop is
/// busy typing out a long payload.
///
/// Protocol: each connection reads one line. Recognized commands:
///
///   ABORT   — set the abort flag. The data-side typing loop checks
///             it between every byte and drains the rest of the
///             current connection silently instead of typing it.
///             Writes "OK\n" to the control connection and closes.
///
///   Anything else gets "ERR unknown command\n".
fn control_loop(port: u16, abort_flag: Arc<AtomicBool>) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .with_context(|| format!("bind control to port {port}"))?;
    eprintln!("zerokb control listening on :{port}");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("control accept error: {e}");
                continue;
            }
        };
        // Clone the arc for the spawned handler so each connection
        // can't starve others. Each control request is cheap (read
        // one line, maybe set a bool, write one line) so a
        // per-connection thread is fine and keeps the accept loop
        // hot.
        let flag = Arc::clone(&abort_flag);
        thread::spawn(move || {
            if let Err(e) = handle_control(stream, flag) {
                eprintln!("control handler error: {e:#}");
            }
        });
    }
    Ok(())
}

fn handle_control(mut stream: TcpStream, abort_flag: Arc<AtomicBool>) -> Result<()> {
    // Read a single line (up to newline or EOF). Do NOT use
    // BufReader::new(&mut stream) because that would move the borrow
    // and we need to write back to stream afterwards. Read into a
    // small owned buffer manually.
    let mut line = String::new();
    {
        let mut reader = BufReader::new(&stream);
        let _ = reader.read_line(&mut line);
    }
    let cmd = line.trim();
    match cmd {
        "ABORT" => {
            eprintln!("zerokb: ABORT command received");
            abort_flag.store(true, Ordering::SeqCst);
            let _ = writeln!(stream, "OK");
        }
        "" => {
            // Empty line — just close.
            let _ = writeln!(stream, "ERR empty command");
        }
        other => {
            eprintln!("zerokb: unknown control command: {other:?}");
            let _ = writeln!(stream, "ERR unknown command");
        }
    }
    Ok(())
}

fn handle_data_stream(mut stream: TcpStream, abort_flag: Arc<AtomicBool>) -> Result<()> {
    let peer = stream.peer_addr().ok();
    eprintln!(
        "connection from {}",
        peer.map_or("unknown".into(), |a: SocketAddr| a.to_string())
    );

    // New data session — reset the abort flag so a leftover ABORT
    // from a previous session doesn't silently kill this one. Any
    // ABORT that lands DURING this session will be observed by the
    // loop below because we re-check the flag between every byte.
    abort_flag.store(false, Ordering::SeqCst);

    let mut hid = OpenOptions::new()
        .write(true)
        .open(HID_DEVICE)
        .with_context(|| format!("open {HID_DEVICE}"))?;

    let mut buf = [0u8; 1];
    let mut bytes_typed: u64 = 0;
    let mut bytes_dropped: u64 = 0;
    let mut aborted = false;
    let mut announced_abort = false;

    while stream.read(&mut buf).context("read byte")? > 0 {
        // Re-check the abort flag each iteration. Relaxed is fine
        // here — we just need eventual visibility of the store from
        // the control thread, and there's no other memory being
        // coordinated. SeqCst on the store side is conservative
        // overhead we can absorb since ABORT is rare.
        if !aborted && abort_flag.load(Ordering::Relaxed) {
            aborted = true;
        }
        if aborted {
            if !announced_abort {
                eprintln!(
                    "zerokb: ABORT active — draining remaining bytes silently"
                );
                announced_abort = true;
            }
            bytes_dropped += 1;
            continue;
        }
        let ch = buf[0] as char;
        if let Some((modifier, keycode)) = keymap::lookup(ch) {
            send_report(&mut hid, modifier, keycode)?;
            bytes_typed += 1;
        }
    }

    eprintln!(
        "connection closed (typed={} dropped={})",
        bytes_typed, bytes_dropped
    );
    Ok(())
}

fn run() -> Result<()> {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let control_port = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CONTROL_PORT);

    start_watchdog();

    // Shared abort flag. Set by the control thread on ABORT; read by
    // the data thread between every byte. Wrapped in an Arc because
    // both threads need shared ownership.
    let abort_flag = Arc::new(AtomicBool::new(false));

    // Spawn the control listener. Separate thread so it stays
    // responsive even while the data loop is blocked writing HID
    // reports.
    let control_flag = Arc::clone(&abort_flag);
    thread::spawn(move || {
        if let Err(e) = control_loop(control_port, control_flag) {
            eprintln!("zerokb: control loop fatal: {e:#}");
        }
    });

    let listener = TcpListener::bind(("0.0.0.0", port))
        .with_context(|| format!("bind to port {port}"))?;

    eprintln!("zerokb listening on :{port}");

    for stream in listener.incoming() {
        let stream = stream.context("accept connection")?;
        if let Err(e) = handle_data_stream(stream, Arc::clone(&abort_flag)) {
            eprintln!("data stream error: {e:#}");
            // Don't exit — keep accepting subsequent connections.
            // The HID device is reopened per-connection so a
            // transient EIO doesn't wedge us.
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("zerokb: {e:#}");
        std::process::exit(1);
    }
}
