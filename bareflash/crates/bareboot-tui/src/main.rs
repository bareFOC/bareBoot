use bareboot_tui::{worker, build, probe, config_editor};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    layout::Alignment,
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap},
    text::Text,
    Terminal,
};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Bus,
    Config,
    Build,
    Flash,
    Profiles,
}

struct App {
    tab: Tab,
    running: bool,
    logs: Vec<String>,
    nodes: Vec<(String, String)>,
    profiles: Vec<String>,
    profile_selected: usize,
    detail_scroll: usize,
    flash_step: u8,
    flash_profile: bareboot_tui::profile::FlashProfile,
    progress: u16,
    progress_history: Vec<u64>,
    config_text: String,
    config_status: String,
    config_form: bareboot_tui::form::Form,
    overlay: bareboot_tui::overlay::Overlay,
    worker: worker::WorkerHandles,
}

impl App {
    fn new() -> Self {
        let mut paths = vec![
            PathBuf::from("bareboot.toml"),
            PathBuf::from("../../../bareboot.toml"),
            PathBuf::from("/run/media/sun/halo/bareBoot/bareboot.toml"),
        ];
        let config_text = paths
            .iter()
            .find(|p| p.exists())
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| "No bareboot.toml found".to_string());
        let handles = worker::WorkerHandles::spawn();
        let mut app = Self {
            tab: Tab::Bus,
            running: true,
            logs: vec!["bareboot-tui started".to_string()],
            nodes: vec![],
            profiles: vec![],
            profile_selected: 0,
            detail_scroll: 0,
            flash_step: 0,
            flash_profile: bareboot_tui::profile::FlashProfile {
                version: 1, name: "wizard".to_string(), backend: "memory".to_string(),
                firmware: String::new(), serial_device: String::new(), baud: 115200,
                i2c_device: String::new(), i2c_address: "0x40".to_string(),
                can_interface: String::new(), node_id: "0x100".to_string(),
                tries: 5, timeout_secs: 2, verify: true,
            },
            progress: 0,
            progress_history: vec![],
            config_text: config_text.clone(),
            config_status: "Ready".to_string(),
            config_form: {
                let (target, transport, boot) = match bareboot_client::parse_config(&config_text) {
                    Ok(c) => (c.target, c.transport, c.boot_window_ms.to_string()),
                    Err(_) => ("ch32v203".to_string(), "can".to_string(), "300".to_string()),
                };
                bareboot_tui::form::Form::new(vec![
                    bareboot_tui::form::FormField::text("target", &target),
                    bareboot_tui::form::FormField::text("transport", &transport),
                    bareboot_tui::form::FormField::text("boot_window_ms", &boot),
                ])
            },
            overlay: bareboot_tui::overlay::Overlay::None,
            worker: handles,
        };
        if let Some(p) = bareboot_tui::build::discover_firmware(std::path::Path::new(".")) {
            app.push_log(format!("[firmware] discovered {}", p.display()));
            app.flash_profile.firmware = p.display().to_string();
        }
        let _ = app.worker.command_tx.send(worker::WorkerCommand::ScanBus);
        let _ = app.worker.command_tx.send(worker::WorkerCommand::LoadProfiles { dir: "profiles".to_string() });
        app
    }

    fn push_log(&mut self, line: String) {
        self.logs.push(line);
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(100);

    while app.running {
        // Drain worker events.
        while let Ok(ev) = app.worker.event_rx.try_recv() {
            match ev {
                worker::UiEvent::BusNode { name, uuid } => {
                    app.nodes.push((name, uuid));
                }
                worker::UiEvent::Progress { percent } => {
                    app.progress = percent as u16;
                    app.progress_history.push(percent as u64);
                    if app.progress_history.len() > 60 { app.progress_history.remove(0); }
                    app.push_log(format!("[progress] {percent}%"));
                }
                worker::UiEvent::Log { line } => app.push_log(line),
                worker::UiEvent::Error { message } => app.push_log(format!("ERROR: {message}")),
                worker::UiEvent::Done { message } => app.push_log(format!("DONE: {message}")),
                worker::UiEvent::ProfileList { names } => app.profiles = names,
            }
        }

        terminal.draw(|f| {
            let area = f.area();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(1), Constraint::Length(2)])
                .split(area);
            let status = Paragraph::new(format!("bareboot-tui │ tab: {:?} │ profiles: {} │ step: {}/5",
                app.tab, app.profiles.len(), app.flash_step + 1))
                .style(Style::default().fg(bareboot_tui::theme::THEME.accent));
            f.render_widget(status, layout[0]);

            // Three-pane layout: nav (left) / main (center) / context (right)
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(18), Constraint::Percentage(57), Constraint::Percentage(25)])
                .split(layout[1]);
            // Nav pane
            let nav_items: Vec<ListItem> = ["Bus","Config","Build","Flash","Profiles"].iter().enumerate().map(|(i,l)| {
                let style = if i == match app.tab { Tab::Bus=>0, Tab::Config=>1, Tab::Build=>2, Tab::Flash=>3, Tab::Profiles=>4 } {
                    Style::default().fg(Color::Black).bg(bareboot_tui::theme::THEME.active).add_modifier(Modifier::BOLD)
                } else { Style::default().fg(bareboot_tui::theme::THEME.fg) };
                ListItem::new(Line::from(Span::styled(*l, style)))
            }).collect();
            let nav = List::new(nav_items).block(Block::default().title("Nav (1-5)").borders(Borders::ALL).border_style(Style::default().fg(bareboot_tui::theme::THEME.border)));
            f.render_widget(nav, cols[0]);

            match app.tab {
                Tab::Bus => {
                    let body = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(30), Constraint::Length(3), Constraint::Percentage(67)])
                        .split(cols[1]);
                    let top = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                        .split(body[0]);
                    // context pane
                    let ctx_text = if app.nodes.is_empty() {
                        "No nodes detected".to_string()
                    } else {
                        format!("Nodes: {}\nSelected: {}", app.nodes.len(), app.nodes[0].0)
                    };
                    let ctx = Paragraph::new(ctx_text).block(Block::default().title("Context").borders(Borders::ALL));
                    f.render_widget(ctx, cols[2]);
                    let items: Vec<ListItem> = app
                        .nodes
                        .iter()
                        .map(|(n, u)| ListItem::new(Line::from(format!("{n}  {u}"))))
                        .collect();
                    let node_list = List::new(items)
                        .block(Block::default().title("Nodes").borders(Borders::ALL));
                    f.render_widget(node_list, top[0]);
                    let lines: Vec<Line> = app.logs.iter().map(|l| {
                        let color = if l.contains("ERROR") { bareboot_tui::theme::THEME.error }
                            else if l.contains("DONE") { bareboot_tui::theme::THEME.success }
                            else if l.contains("progress") { bareboot_tui::theme::THEME.accent }
                            else { bareboot_tui::theme::THEME.fg };
                        Line::from(Span::styled(l.clone(), Style::default().fg(color)))
                    }).collect();
                    let paragraph = Paragraph::new(Text::from(lines))
                        .block(Block::default().title("Bus/Protocol Log").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });
                    f.render_widget(paragraph, top[1]);
                    let sparkline = Sparkline::default()
                        .block(Block::default().title("Progress History").borders(Borders::ALL))
                        .data(&app.progress_history)
                        .style(Style::default().fg(bareboot_tui::theme::THEME.accent));
                    f.render_widget(sparkline, body[1]);
                }
                Tab::Config => {
                    let lines = app.config_form.render_lines();
                    let paragraph = Paragraph::new(lines)
                        .block(Block::default().title("bareboot.toml (form: Tab next, Enter toggle, w save)").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });
                    f.render_widget(paragraph, cols[1]);
                    let ctx = Paragraph::new(format!("Fields: {}\nPress w to save", app.config_form.fields.len())).block(Block::default().title("Config Help").borders(Borders::ALL));
                    f.render_widget(ctx, cols[2]);
                }
                Tab::Build => {
                    let paragraph = Paragraph::new(app.logs.join("\n"))
                        .block(Block::default().title("Build / Probe Output").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });
                    f.render_widget(paragraph, cols[1]);
                    let ctx = Paragraph::new("b: build\np: probe").block(Block::default().title("Build Help").borders(Borders::ALL));
                    f.render_widget(ctx, cols[2]);
                }
                Tab::Flash => {
                    let step_name = match app.flash_step { 0=>"Backend",1=>"Device",2=>"Firmware",3=>"Options",_=>"Run" };
                    let info = format!("Step {}: {} | backend={} firmware={} tries={} timeout={} verify={}\nKeys: n next, p prev, m/e/i/c backend, r run",
                        app.flash_step+1, step_name, app.flash_profile.backend, app.flash_profile.firmware,
                        app.flash_profile.tries, app.flash_profile.timeout_secs, app.flash_profile.verify);
                    let paragraph = Paragraph::new(format!("{}\n\n{}", info, app.logs.join("\n")))
                        .block(Block::default().title("Flash Wizard").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });
                    f.render_widget(paragraph, cols[1]);
                    let ctx = Paragraph::new(format!("Backend: {}\nFirmware: {}\nTries: {}", app.flash_profile.backend, app.flash_profile.firmware, app.flash_profile.tries)).block(Block::default().title("Flash Context").borders(Borders::ALL));
                    f.render_widget(ctx, cols[2]);
                }
                Tab::Profiles => {
                    let items: Vec<ListItem> = app.profiles.iter().map(|p| ListItem::new(Line::from(p.clone()))).collect();
                    let list = List::new(items).block(Block::default().title("Profiles (l refresh)").borders(Borders::ALL));
                    f.render_widget(list, cols[1]);
                    let detail = if app.profiles.is_empty() {
                        "No profiles saved".to_string()
                    } else if let Some(name) = app.profiles.get(app.profile_selected) {
                        format!("Selected: {}\nPath: profiles/{}.toml", name, name)
                    } else {
                        "No selection".to_string()
                    };
                    let p = Paragraph::new(detail).block(Block::default().title("Profile Detail").borders(Borders::ALL));
                    f.render_widget(p, cols[2]);
                }
            }
            let help = Paragraph::new("Mouse: click tabs | Keys: 1-5 tabs, s scan, b build, p probe, l profiles, ? help, q quit")
                .style(Style::default().fg(bareboot_tui::theme::THEME.muted))
                .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(bareboot_tui::theme::THEME.border)));
            f.render_widget(help, layout[3]);
            // Overlay
            if !app.overlay.is_none() {
                let area = f.area();
                let popup = Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(40), Constraint::Percentage(30)]).split(area);
                let h = Layout::default().direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(60), Constraint::Percentage(20)]).split(popup[1]);
                let text = match &app.overlay {
                    bareboot_tui::overlay::Overlay::Help(t) => t.clone(),
                    bareboot_tui::overlay::Overlay::Confirm { message } => format!("Confirm: {message}\n(y)es / (n)o"),
                    bareboot_tui::overlay::Overlay::ProfilePicker { names, selected } => names.iter().enumerate().map(|(i,n)| if i==*selected { format!("> {n}") } else { format!("  {n}") }).collect::<Vec<_>>().join("\n"),
                    bareboot_tui::overlay::Overlay::None => String::new(),
                };
                let p = Paragraph::new(text).block(Block::default().title("Overlay").borders(Borders::ALL).border_style(Style::default().fg(bareboot_tui::theme::THEME.accent)))
                    .wrap(Wrap { trim: true });
                f.render_widget(p, h[1]);
            }
        })?;

        if event::poll(tick_rate)? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => app.running = false,
                        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => app.running = false,
                        KeyCode::Char('?') => { app.overlay = bareboot_tui::overlay::Overlay::Help("Keys: 1-5 tabs, n/p step, m/e/i/c backend, r run, w save, l/s/d profiles, ? help, q quit".to_string()); }
                        KeyCode::Esc => { app.overlay = bareboot_tui::overlay::Overlay::None; }
                        KeyCode::Backspace if app.tab == Tab::Config => { app.config_form.handle_backspace(); }
                        KeyCode::Tab if app.tab == Tab::Config => { app.config_form.next(); }
                        KeyCode::Enter if app.tab == Tab::Config => { app.config_form.handle_toggle(); }
                        KeyCode::Char(c) if app.tab == Tab::Config && c != 'w' && c != 'q' => { app.config_form.handle_char(c); }
                        KeyCode::Backspace if app.tab == Tab::Flash && app.flash_step == 2 => { app.flash_profile.firmware.pop(); }
                        KeyCode::Char(c) if app.tab == Tab::Flash && app.flash_step == 2 && c != 'n' && c != 'p' && c != 'r' && c != 'm' && c != 'e' && c != 'i' && c != 'c' => { app.flash_profile.firmware.push(c); }
                        KeyCode::Backspace if app.tab == Tab::Flash && app.flash_step == 1 => { app.flash_profile.serial_device.pop(); }
                        KeyCode::Char(c) if app.tab == Tab::Flash && app.flash_step == 1 && c != 'n' && c != 'p' && c != 'r' && c != 'm' && c != 'e' && c != 'i' && c != 'c' => { app.flash_profile.serial_device.push(c); }
                        KeyCode::Char('+') if app.tab == Tab::Flash && app.flash_step == 3 => { app.flash_profile.tries += 1; }
                        KeyCode::Char('-') if app.tab == Tab::Flash && app.flash_step == 3 => { if app.flash_profile.tries > 1 { app.flash_profile.tries -= 1; } }
                        KeyCode::Up if app.tab == Tab::Profiles => { if app.profile_selected > 0 { app.profile_selected -= 1; } }
                        KeyCode::Down if app.tab == Tab::Profiles => { if app.profile_selected + 1 < app.profiles.len() { app.profile_selected += 1; } }
                        KeyCode::Enter if app.tab == Tab::Profiles => {
                            if let Some(name) = app.profiles.get(app.profile_selected).cloned() {
                                // load profile via worker by reading file directly
                                if let Ok(p) = bareboot_tui::profile::FlashProfile::load(std::path::Path::new("profiles").join(format!("{name}.toml")).as_path()) {
                                    app.flash_profile = p;
                                    app.push_log(format!("loaded profile {name}"));
                                }
                            }
                        }
                        KeyCode::Char('w') if app.tab == Tab::Config => {
                            let target = app.config_form.fields.get(0).map(|f| f.value_text()).unwrap_or_default();
                            let transport = app.config_form.fields.get(1).map(|f| f.value_text()).unwrap_or_default();
                            let boot = app.config_form.fields.get(2).map(|f| f.value_text()).unwrap_or_default();
                            let text = format!("target = \"{}\"\ntransport = \"{}\"\nboot_window_ms = {}\n", target, transport, boot);
                            match bareboot_client::parse_config(&text) {
                                Ok(cfg) => match cfg.validate() {
                                    Ok(_) => { app.config_text = text.clone(); let _ = app.worker.command_tx.send(worker::WorkerCommand::WriteConfig { path: "bareboot.toml".to_string(), text }); }
                                    Err(e) => app.push_log(format!("config invalid: {e}")),
                                },
                                Err(e) => app.push_log(format!("config parse error: {e}")),
                            }
                        }
                        KeyCode::Char('1') => app.tab = Tab::Bus,
                        KeyCode::Char('2') => app.tab = Tab::Config,
                        KeyCode::Char('3') => app.tab = Tab::Build,
                        KeyCode::Char('4') => app.tab = Tab::Flash,
                        KeyCode::Char('5') => app.tab = Tab::Profiles,
                        KeyCode::Char('n') if app.tab == Tab::Flash => { if app.flash_step < 4 { app.flash_step += 1; } }
                        KeyCode::Char('p') if app.tab == Tab::Flash => { if app.flash_step > 0 { app.flash_step -= 1; } }
                        KeyCode::Char('r') if app.tab == Tab::Flash => {
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::Flash { profile: app.flash_profile.clone() });
                        }
                        KeyCode::Char('m') if app.tab == Tab::Flash => { app.flash_profile.backend = "memory".to_string(); }
                        KeyCode::Char('e') if app.tab == Tab::Flash => { app.flash_profile.backend = "serial".to_string(); }
                        KeyCode::Char('i') if app.tab == Tab::Flash => { app.flash_profile.backend = "i2c".to_string(); }
                        KeyCode::Char('c') if app.tab == Tab::Flash => { app.flash_profile.backend = "can".to_string(); }
                        KeyCode::Char('j') => { app.detail_scroll = app.detail_scroll.saturating_add(1); }
                        KeyCode::Char('k') => { app.detail_scroll = app.detail_scroll.saturating_sub(1); }
                        KeyCode::Char('l') => {
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::LoadProfiles { dir: "profiles".to_string() });
                        }
                        KeyCode::Char('s') if app.tab == Tab::Profiles => {
                            let profile = bareboot_tui::profile::FlashProfile {
                                version: 1, name: "default".to_string(), backend: "memory".to_string(),
                                firmware: bareboot_tui::build::discover_firmware(std::path::Path::new(".")).map(|p| p.display().to_string()).unwrap_or_default(),
                                serial_device: String::new(), baud: 115200, i2c_device: String::new(),
                                i2c_address: "0x40".to_string(), can_interface: String::new(), node_id: "0x100".to_string(),
                                tries: 5, timeout_secs: 2, verify: true,
                            };
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::SaveProfile { dir: "profiles".to_string(), profile });
                        }
                        KeyCode::Char('d') if app.tab == Tab::Profiles => {
                            if let Some(sel) = app.profiles.get(app.profile_selected).cloned() {
                                app.overlay = bareboot_tui::overlay::Overlay::Confirm { message: format!("delete profile {sel}?") };
                            }
                        }
                        KeyCode::Char('y') if matches!(app.overlay, bareboot_tui::overlay::Overlay::Confirm { .. }) => {
                            if let bareboot_tui::overlay::Overlay::Confirm { message } = &app.overlay {
                                if let Some(name) = message.strip_prefix("delete profile ").and_then(|s| s.strip_suffix("?")) {
                                    let _ = app.worker.command_tx.send(worker::WorkerCommand::DeleteProfile { dir: "profiles".to_string(), name: name.to_string() });
                                }
                            }
                            app.overlay = bareboot_tui::overlay::Overlay::None;
                        }
                        KeyCode::Char('n') if matches!(app.overlay, bareboot_tui::overlay::Overlay::Confirm { .. }) => {
                            app.overlay = bareboot_tui::overlay::Overlay::None;
                        }
                        KeyCode::Char('o') if app.tab == Tab::Profiles => {
                            app.overlay = bareboot_tui::overlay::Overlay::ProfilePicker { names: app.profiles.clone(), selected: app.profile_selected };
                        }
                        KeyCode::Enter if matches!(app.overlay, bareboot_tui::overlay::Overlay::ProfilePicker { .. }) => {
                            if let bareboot_tui::overlay::Overlay::ProfilePicker { names, selected } = &app.overlay {
                                if let Some(n) = names.get(*selected) { app.profile_selected = *selected; app.push_log(format!("picked {n}")); }
                            }
                            app.overlay = bareboot_tui::overlay::Overlay::None;
                        }
                        KeyCode::Up if matches!(app.overlay, bareboot_tui::overlay::Overlay::ProfilePicker { .. }) => {
                            if let bareboot_tui::overlay::Overlay::ProfilePicker { names, selected } = &mut app.overlay {
                                if *selected > 0 { *selected -= 1; }
                            }
                        }
                        KeyCode::Down if matches!(app.overlay, bareboot_tui::overlay::Overlay::ProfilePicker { .. }) => {
                            if let bareboot_tui::overlay::Overlay::ProfilePicker { names, selected } = &mut app.overlay {
                                if *selected + 1 < names.len() { *selected += 1; }
                            }
                        },
                        KeyCode::Char('s') => {
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::ScanBus);
                        }
                        KeyCode::Char('b') => {
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::Build {
                                manifest_path: "Cargo.toml".to_string(),
                            });
                            app.push_log("started build".to_string());
                        }
                        KeyCode::Char('p') => {
                            let _ = app.worker.command_tx.send(worker::WorkerCommand::Provision);
                            app.push_log("started probe listing".to_string());
                        }
                        _ => {}
                    }
                }
            } else if let Event::Mouse(mouse) = event {
                if !app.overlay.is_none() && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    app.overlay = bareboot_tui::overlay::Overlay::None;
                } else if mouse.kind == MouseEventKind::Down(MouseButton::Left) && mouse.row <= 3 {
                    let size = terminal.size().unwrap_or_default();
                    let third = (size.width / 3).max(1);
                    let fifth = (size.width / 5).max(1);
                    let idx = (mouse.column / fifth).min(4) as usize;
                    app.tab = match idx {
                        0 => Tab::Bus,
                        1 => Tab::Config,
                        2 => Tab::Build,
                        3 => Tab::Flash,
                        _ => Tab::Profiles,
                    };
                }
            }
        }
    }

    // Clean worker thread.
    app.worker.shutdown();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
