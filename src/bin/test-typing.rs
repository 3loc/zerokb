use std::io::{self, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, poll};
use crossterm::terminal;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const DEFAULT_ADDR: &str = "192.168.10.8:7070";
const MAX_LINES: usize = 100;
const ZEROKB_VID: &str = "3434";
const ZEROKB_PID: &str = "0333";

/// Check if the zerokb USB HID gadget (Keychron V3) is plugged into this machine.
/// If so, running the TUI here would create an infinite feedback loop.
fn check_feedback_loop() {
    let found = if cfg!(target_os = "macos") {
        Command::new("ioreg")
            .args(["-p", "IOUSB", "-l"])
            .output()
            .ok()
            .map(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.contains("idVendor\" = 13364") && out.contains("idProduct\" = 819")
            })
            .unwrap_or(false)
    } else {
        // Linux: check lsusb for VID:PID
        Command::new("lsusb")
            .args(["-d", &format!("{ZEROKB_VID}:{ZEROKB_PID}")])
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
    };

    if found {
        eprintln!("error: zerokb USB keyboard is plugged into THIS machine.");
        eprintln!("       running the TUI here would create a feedback loop.");
        eprintln!("       run zerokb-tui from a DIFFERENT machine.");
        std::process::exit(1);
    }
}

struct App {
    stream: TcpStream,
    lines: Vec<String>,
    last_ctrl_c: Option<Instant>,
    sent_count: usize,
    status: String,
}

impl App {
    fn new(stream: TcpStream, addr: &str) -> Self {
        Self {
            stream,
            lines: vec![String::new()],
            last_ctrl_c: None,
            sent_count: 0,
            status: format!("connected to {addr}"),
        }
    }

    fn send(&mut self, byte: u8) -> bool {
        if self.stream.write_all(&[byte]).is_err() || self.stream.flush().is_err() {
            self.status = "connection lost".into();
            return false;
        }
        self.sent_count += 1;
        true
    }

    fn push_char(&mut self, ch: char) {
        self.lines.last_mut().unwrap().push(ch);
    }

    fn push_newline(&mut self) {
        self.lines.push(String::new());
        if self.lines.len() > MAX_LINES {
            self.lines.remove(0);
        }
    }

    fn push_backspace(&mut self) {
        if let Some(line) = self.lines.last_mut() {
            line.pop();
        }
    }

    fn push_ctrl(&mut self, ch: char) {
        self.lines.last_mut().unwrap().push_str(&format!("^{ch}"));
    }

    fn text(&self) -> String {
        self.lines.join("\n")
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Double Ctrl+C within 500ms to quit
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            if self.last_ctrl_c.is_some_and(|t| t.elapsed().as_millis() < 500) {
                return false;
            }
            self.last_ctrl_c = Some(Instant::now());
        } else {
            self.last_ctrl_c = None;
        }

        match code {
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::CONTROL) => {
                let ctrl = (c as u8).wrapping_sub(b'a' - 1);
                if (0x01..=0x1a).contains(&ctrl) {
                    if !self.send(ctrl) { return false; }
                    self.push_ctrl((ctrl + b'A' - 1) as char);
                }
            }
            KeyCode::Char(c) => {
                if !self.send(c as u8) { return false; }
                self.push_char(c);
            }
            KeyCode::Enter => {
                if !self.send(b'\n') { return false; }
                self.push_newline();
            }
            KeyCode::Tab => {
                if !self.send(b'\t') { return false; }
                self.push_char('\t');
            }
            KeyCode::Backspace => {
                if !self.send(0x7f) { return false; }
                self.push_backspace();
            }
            _ => {}
        }
        true
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Header
    let header = Paragraph::new("zerokb test-typing")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(header, chunks[0]);

    // Text area with auto-scroll
    let text = app.text();
    let text_widget = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(" typing ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .scroll((app.lines.len().saturating_sub(chunks[1].height as usize - 2) as u16, 0));
    frame.render_widget(text_widget, chunks[1]);

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.status, Style::default().fg(Color::Green)),
        Span::raw("  │  "),
        Span::styled(format!("{} sent", app.sent_count), Style::default().fg(Color::Yellow)),
        Span::raw("  │  "),
        Span::styled("Ctrl-C Ctrl-C quit", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(status, chunks[2]);
}

fn main() {
    check_feedback_loop();

    let addr = std::env::args().nth(1).unwrap_or(DEFAULT_ADDR.into());

    let stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to connect to {addr}: {e}");
            std::process::exit(1);
        }
    };

    let mut app = App::new(stream, &addr);

    terminal::enable_raw_mode().expect("enable raw mode");
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout())).expect("init terminal");
    terminal.clear().ok();

    loop {
        terminal.draw(|f| ui(f, &app)).expect("draw");

        if poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
                if !app.handle_key(code, modifiers) {
                    break;
                }
            }
        }
    }

    terminal::disable_raw_mode().expect("disable raw mode");
    terminal.clear().ok();
}
