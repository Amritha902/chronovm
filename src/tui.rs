//! The terminal time-travel debugger UI, built on ratatui.
//!
//! The whole UI is a pure function of one number: `cursor`, the frame we are
//! looking at. Stepping forward or backward just changes that index and the
//! entire machine state re-renders from the recorded [`Trace`]. Because every
//! frame is already recorded, moving backward is as cheap as moving forward.
//!
//! The headline interaction is `w` ("why?"): it walks the provenance graph of
//! the selected variable and teleports you to the step that produced its value,
//! showing the full causal chain.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::vm::{CausalNode, Trace};

/// How far the `[` / `]` keys leap along the timeline.
const LEAP: usize = 25;
/// Auto-play advances one step per this interval.
const PLAY_INTERVAL: Duration = Duration::from_millis(70);

struct CausalView {
    var: String,
    chain: Vec<CausalNode>,
    sel: usize,
}

struct App {
    trace: Trace,
    cursor: usize,
    playing: bool,
    /// Index into the current frame's variable list, for causal queries.
    var_sel: usize,
    causal: Option<CausalView>,
    should_quit: bool,
}

impl App {
    fn new(trace: Trace) -> Self {
        App {
            trace,
            cursor: 0,
            playing: false,
            var_sel: 0,
            causal: None,
            should_quit: false,
        }
    }

    fn last(&self) -> usize {
        self.trace.last()
    }

    fn step_to(&mut self, target: usize) {
        self.cursor = target.min(self.last());
        // Keep the variable selection in range as the variable set changes.
        let n = self.trace.frames[self.cursor].vars.len();
        if n == 0 {
            self.var_sel = 0;
        } else if self.var_sel >= n {
            self.var_sel = n - 1;
        }
    }

    fn forward(&mut self, n: usize) {
        self.step_to(self.cursor.saturating_add(n));
    }

    fn back(&mut self, n: usize) {
        self.step_to(self.cursor.saturating_sub(n));
    }

    /// Trigger the causal "why?" query on the currently selected variable.
    fn explain_selected(&mut self) {
        let frame = &self.trace.frames[self.cursor];
        let Some((name, _)) = frame.vars.iter().nth(self.var_sel) else {
            return;
        };
        let name = name.clone();
        let chain = self.trace.explain_var(self.cursor, &name);
        if let Some(first) = chain.first() {
            let jump = first.step;
            self.causal = Some(CausalView {
                var: name,
                chain,
                sel: 0,
            });
            self.step_to(jump);
        }
    }

    fn causal_move(&mut self, delta: isize) {
        if let Some(view) = &mut self.causal {
            let len = view.chain.len() as isize;
            let mut idx = view.sel as isize + delta;
            idx = idx.clamp(0, len - 1);
            view.sel = idx as usize;
            let jump = view.chain[view.sel].step;
            self.step_to(jump);
        }
    }
}

/// Entry point: take ownership of a recorded trace and run the debugger.
pub fn run(trace: Trace) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new(trace);
    // Start parked on the final frame so you see the outcome first, then rewind.
    app.cursor = app.last();

    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;

        // Poll so auto-play can tick even without keypresses.
        if event::poll(PLAY_INTERVAL)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code, key.modifiers);
                }
            }
        } else if app.playing {
            if app.cursor >= app.last() {
                app.playing = false;
            } else {
                app.forward(1);
            }
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // When the causal panel is open it captures navigation keys.
    if app.causal.is_some() {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => app.causal = None,
            KeyCode::Up | KeyCode::Char('k') => app.causal_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.causal_move(1),
            KeyCode::Left => {
                app.causal = None;
                app.back(1);
            }
            KeyCode::Right => {
                app.causal = None;
                app.forward(1);
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Left | KeyCode::Char('h') => {
            app.playing = false;
            app.back(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.playing = false;
            app.forward(1);
        }
        KeyCode::Char('[') => {
            app.playing = false;
            app.back(LEAP);
        }
        KeyCode::Char(']') => {
            app.playing = false;
            app.forward(LEAP);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.playing = false;
            app.step_to(0);
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.playing = false;
            app.step_to(app.last());
        }
        KeyCode::Char(' ') => {
            // Restart from the top if we're parked at the end.
            if app.cursor >= app.last() {
                app.step_to(0);
            }
            app.playing = !app.playing;
        }
        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
            let n = app.trace.frames[app.cursor].vars.len();
            if n > 0 {
                app.var_sel = if mods.contains(KeyModifiers::SHIFT) {
                    (app.var_sel + n - 1) % n
                } else {
                    (app.var_sel + 1) % n
                };
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let n = app.trace.frames[app.cursor].vars.len();
            if n > 0 {
                app.var_sel = (app.var_sel + n - 1) % n;
            }
        }
        KeyCode::Char('w') | KeyCode::Enter => app.explain_selected(),
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // main panes
            Constraint::Length(3), // timeline
            Constraint::Length(1), // help
        ])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(root[0]);

    render_source(f, app, main[0]);
    render_right(f, app, main[1]);
    render_timeline(f, app, root[1]);
    render_help(f, app, root[2]);
}

fn render_source(f: &mut Frame, app: &App, area: Rect) {
    let frame = &app.trace.frames[app.cursor];
    let program = &app.trace.program;
    // Highlight the instruction that just executed to reach this frame; on the
    // initial frame, point at the first instruction about to run.
    let active = frame.last_ip.unwrap_or(frame.ip);

    let mut items: Vec<ListItem> = Vec::new();
    let mut active_row: usize = 0;
    for idx in 0..program.len() {
        for label in &program.labels_at[idx] {
            items.push(ListItem::new(Line::from(Span::styled(
                format!("{label}:"),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ))));
        }
        if idx == active {
            active_row = items.len();
        }
        let is_active = idx == active;
        let marker = if is_active { "▶" } else { " " };
        let addr = format!("{idx:>3}");
        let line = Line::from(vec![
            Span::styled(format!(" {marker} "), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{addr} "), Style::default().fg(Color::DarkGray)),
            Span::raw(program.source[idx].clone()),
        ]);
        let style = if is_active {
            Style::default()
                .bg(Color::Rgb(40, 44, 72))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(ListItem::new(line).style(style));
    }

    let title = format!(" source · {} instructions ", program.len());
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    let mut state = ListState::default();
    state.select(Some(active_row));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_right(f: &mut Frame, app: &App, area: Rect) {
    // Split the right column: state row on top, output/causal below.
    let has_causal = app.causal.is_some();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(if has_causal { 25 } else { 50 }),
            Constraint::Percentage(if has_causal { 25 } else { 0 }),
        ])
        .split(area);

    let state_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    render_stack(f, app, state_row[0]);
    render_vars(f, app, state_row[1]);
    render_output(f, app, rows[1]);
    if has_causal {
        render_causal(f, app, rows[2]);
    }
}

fn render_stack(f: &mut Frame, app: &App, area: Rect) {
    let frame = &app.trace.frames[app.cursor];
    let mut items: Vec<ListItem> = Vec::new();
    // Top of stack first, so it reads like a stack growing upward.
    for (depth, (&v, &origin)) in frame
        .stack
        .iter()
        .rev()
        .zip(frame.stack_origin.iter().rev())
        .enumerate()
    {
        let tag = if depth == 0 { "top →" } else { "     " };
        let line = Line::from(vec![
            Span::styled(format!("{tag} "), Style::default().fg(Color::Green)),
            Span::styled(
                format!("{v:>8}"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   (from step {origin})"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        items.push(ListItem::new(line));
    }
    if items.is_empty() {
        items.push(ListItem::new(Span::styled(
            "  (empty)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" stack · depth {} ", frame.stack.len()))
            .border_style(Style::default().fg(Color::Green)),
    );
    f.render_widget(list, area);
}

fn render_vars(f: &mut Frame, app: &App, area: Rect) {
    let frame = &app.trace.frames[app.cursor];
    let mut items: Vec<ListItem> = Vec::new();
    for (i, (name, &val)) in frame.vars.iter().enumerate() {
        let selected = i == app.var_sel;
        let def = frame.var_def.get(name).copied().unwrap_or(0);
        let marker = if selected { "◆" } else { " " };
        let line = Line::from(vec![
            Span::styled(
                format!(" {marker} "),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("{name:<8}"),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("= "),
            Span::styled(
                format!("{val}"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   (set @ step {def})"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let style = if selected {
            Style::default().bg(Color::Rgb(48, 40, 24))
        } else {
            Style::default()
        };
        items.push(ListItem::new(line).style(style));
    }
    if items.is_empty() {
        items.push(ListItem::new(Span::styled(
            "  (no variables yet)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" variables · [w] why? ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    f.render_widget(list, area);
}

fn render_output(f: &mut Frame, app: &App, area: Rect) {
    let frame = &app.trace.frames[app.cursor];
    let mut text = frame.output.clone();
    if let Some(err) = &frame.error {
        text.push_str(&format!("\n⚠ fault: {err}"));
    }
    let lines = text.lines().count() as u16;
    let height = area.height.saturating_sub(2);
    let scroll = lines.saturating_sub(height);
    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" output ")
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(para, area);
}

fn render_causal(f: &mut Frame, app: &App, area: Rect) {
    let Some(view) = &app.causal else { return };
    let mut items: Vec<ListItem> = Vec::new();
    for (i, node) in view.chain.iter().enumerate() {
        let selected = i == view.sel;
        let marker = if selected { "▶" } else { "·" };
        let line = Line::from(vec![
            Span::styled(format!(" {marker} "), Style::default().fg(Color::Red)),
            Span::styled(
                format!("step {:>4}  ", node.step),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(node.description.clone()),
        ]);
        let style = if selected {
            Style::default()
                .bg(Color::Rgb(60, 24, 24))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(ListItem::new(line).style(style));
    }
    let title = format!(" why is `{}` == {}? · [↑↓] walk causes · [esc] close ", view.var, {
        app.trace.frames[app.cursor]
            .vars
            .get(&view.var)
            .copied()
            .unwrap_or_default()
    });
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Red)),
    );
    f.render_widget(list, area);
}

fn render_timeline(f: &mut Frame, app: &App, area: Rect) {
    let last = app.last().max(1);
    let ratio = app.cursor as f64 / last as f64;
    let frame = &app.trace.frames[app.cursor];

    let status = if let Some(err) = &frame.error {
        format!("⚠ {err}")
    } else if frame.halted {
        "halted".to_string()
    } else if app.playing {
        "▶ playing".to_string()
    } else {
        "paused".to_string()
    };

    let label = format!(
        "step {} / {}   ·   {}",
        app.cursor,
        app.last(),
        status
    );
    let color = if frame.error.is_some() {
        Color::Red
    } else if app.playing {
        Color::Green
    } else {
        Color::Cyan
    };
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" timeline · [←→] step · [ [ ] ] leap · [space] play · [home/end] ends ")
                .border_style(Style::default().fg(color)),
        )
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, area);
}

fn render_help(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.causal.is_some() {
        "  causal view — [↑↓/jk] walk the chain · [←→] step out · [esc] close"
    } else {
        "  [←→] step  [ [ ] ] leap  [space] play  [tab] pick var  [w] why?  [home/end] jump  [q] quit"
    };
    let para = Paragraph::new(Text::from(Line::from(Span::styled(
        text,
        Style::default().fg(Color::DarkGray),
    ))));
    f.render_widget(para, area);
}
