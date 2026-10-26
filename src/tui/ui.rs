use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

use super::app::{App, Field, Mode, Zone};

pub fn render(frame: &mut Frame, app: &App) {
    let [clock, tabs, lists, detail, pr_area, status] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    render_clock(frame, clock);
    render_tabs(frame, tabs, app);

    // Both tabs share the row; the inactive one is squeezed to zero width, so switching slides.
    let todo_width = (lists.width as f64 * app.split / 100.0).round() as u16;
    let [todo, done] = Layout::horizontal([Constraint::Length(todo_width), Constraint::Min(0)]).areas(lists);

    render_zone(frame, todo, app, Zone::Todo);
    render_zone(frame, done, app, Zone::Done);
    render_detail(frame, detail, app);

    render_prs(frame, pr_area, app);

    if let Mode::Insert { field, editing, draft, cursor } = &app.mode {
        render_popup(frame, *field, *cursor, editing.is_some(), &draft.title, &draft.description);
    }

    let hints = match app.mode {
        Mode::Normal => " NORMAL  j/k gg/G move · C-hjkl pane · C-r refresh PRs · o new (from PR on PR pane) · e edit · x done/undo · dd delete · V visual · q quit",
        Mode::Visual { .. } => " VISUAL  j/k extend · x move · d delete · Esc cancel",
        Mode::Insert { .. } => " INSERT  Tab field · Enter save · Esc cancel · C-a/e C-b/f M-b/f move · C-w C-u C-k M-d C-d delete",
    };
    frame.render_widget(Line::from(hints).dark_gray(), status);
}

fn render_zone(frame: &mut Frame, area: Rect, app: &App, zone: Zone) {
    let tasks = app.list(zone);
    let focused = app.zone == zone;
    let visual = focused && matches!(app.mode, Mode::Visual { .. });
    let (a, b) = app.selection();

    let items: Vec<ListItem> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut item = ListItem::new(t.title.as_str());
            if zone == Zone::Done {
                item = item.crossed_out().dark_gray();
            }
            if visual && (a..=b).contains(&i) {
                item = item.on_blue();
            }
            item
        })
        .collect();

    let mut block = Block::bordered();
    if focused {
        block = block.cyan();
    }

    let list = List::new(items).block(block).highlight_style(Style::new().reversed());
    let mut state = ListState::default().with_selected(focused.then_some(app.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_detail(frame: &mut Frame, area: Rect, app: &App) {
    let text = app.list(app.zone).get(app.cursor).map(|t| t.description.as_str()).unwrap_or_default();
    let detail = Paragraph::new(text).wrap(Wrap { trim: false }).block(Block::bordered().title(" Details "));
    frame.render_widget(detail, area);
}

/// The active field draws a block cursor (reversed char) at `cursor`.
fn field_line(text: &str, cursor: Option<usize>) -> Line<'_> {
    let Some(cur) = cursor else { return Line::from(text) };
    let (before, rest) = text.split_at(cur);
    let at = rest.chars().next().map_or(0, char::len_utf8);
    let under = if at == 0 { " " } else { &rest[..at] };
    Line::from(vec![Span::raw(before), Span::raw(under).reversed(), Span::raw(&rest[at..])])
}

fn render_popup(frame: &mut Frame, field: Field, cursor: usize, editing: bool, title: &str, description: &str) {
    let area = frame.area().centered(Constraint::Percentage(60), Constraint::Length(9));
    let cursor_in = |f: Field| (field == f).then_some(cursor);
    let label = |f: Field, name: &str| {
        if field == f {
            Line::from(name.to_string()).cyan().bold()
        } else {
            Line::from(name.to_string()).dark_gray()
        }
    };
    let text = vec![
        label(Field::Title, "Title"),
        field_line(title, cursor_in(Field::Title)),
        Line::from(""),
        label(Field::Description, "Description"),
        field_line(description, cursor_in(Field::Description)),
    ];
    let block = Block::bordered()
        .title(if editing { " Edit task " } else { " New task " })
        .title_bottom(" Tab field · Enter save · Esc cancel ")
        .cyan();
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }).block(block), area);
}

fn render_prs(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.zone == Zone::Prs;
    let mut block = Block::bordered().title(" My Open PRs ");
    if focused {
        block = block.cyan();
    }
    let prs = match &app.prs {
        Err(e) => {
            let msg = Paragraph::new(format!("Could not load PRs: {e:#}")).wrap(Wrap { trim: false });
            frame.render_widget(msg.block(block), area);
            return;
        }
        Ok(prs) if prs.is_empty() => {
            frame.render_widget(Paragraph::new("No open PRs found.").block(block), area);
            return;
        }
        Ok(prs) => prs,
    };
    let items: Vec<ListItem> = prs
        .iter()
        .map(|pr| {
            let draft = if pr.is_draft { " [draft]" } else { "" };
            ListItem::new(format!("{} #{}: {}{} ({})", pr.repo, pr.number, pr.title, draft, pr.url))
        })
        .collect();
    let list = List::new(items).block(block).highlight_style(Style::new().reversed());
    let mut state = ListState::default().with_selected(focused.then_some(app.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}

/// 3x5 block glyphs for `0-9` and `:`.
fn glyph(c: char) -> [&'static str; 5] {
    match c {
        '0' => ["███", "█ █", "█ █", "█ █", "███"],
        '1' => ["  █", "  █", "  █", "  █", "  █"],
        '2' => ["███", "  █", "███", "█  ", "███"],
        '3' => ["███", "  █", "███", "  █", "███"],
        '4' => ["█ █", "█ █", "███", "  █", "  █"],
        '5' => ["███", "█  ", "███", "  █", "███"],
        '6' => ["███", "█  ", "███", "█ █", "███"],
        '7' => ["███", "  █", "  █", "  █", "  █"],
        '8' => ["███", "█ █", "███", "█ █", "███"],
        '9' => ["███", "█ █", "███", "  █", "███"],
        _ => [" ", "█", " ", "█", " "],
    }
}

fn render_clock(frame: &mut Frame, area: Rect) {
    let now = chrono::Local::now();
    let mut rows = vec![String::new(); 5];
    for c in now.format("%H:%M:%S").to_string().chars() {
        for (row, part) in rows.iter_mut().zip(glyph(c)) {
            row.push_str(part);
            row.push(' ');
        }
    }
    let lines: Vec<Line> = rows.into_iter().map(|r| Line::from(r).cyan()).collect();
    let date = Line::from(now.format(" %a %b %-d ").to_string()).dark_gray().right_aligned();
    let block = Block::bordered().title(" Clock ").title(date);
    frame.render_widget(Paragraph::new(lines).centered().block(block), area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles = [
        format!(" Todo ({}) ", app.tasks.todo.len()),
        format!(" Done ({}) ", app.tasks.done.len()),
    ];
    let mut highlight = Style::new().reversed();
    if matches!(app.zone, Zone::Todo | Zone::Done) {
        highlight = highlight.cyan().bold();
    }
    let selected = if app.split >= 50.0 { 0 } else { 1 };
    frame.render_widget(Tabs::new(titles).select(selected).highlight_style(highlight), area);
}
