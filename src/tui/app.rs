use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::github::{GithubClient, PRRequest};
use crate::tasks::{Task, Tasks};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Todo,
    Done,
    Prs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Description,
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Normal,
    Visual { anchor: usize },
    /// `cursor` is a byte offset into the active field, always on a char boundary.
    Insert { field: Field, editing: Option<usize>, draft: Task, cursor: usize },
}

pub struct App {
    pub tasks: Tasks,
    pub prs: anyhow::Result<Vec<PRRequest>>,
    pub zone: Zone,
    pub cursor: usize,
    pub mode: Mode,
    pending: Option<char>,
    pub dirty: bool,
    pub quit: bool,
    /// Todo tab's share of the width in percent (100 = Todo shown, 0 = Done shown),
    /// eased toward `split_target()` each tick so switching tabs slides.
    pub split: f64,
}

impl App {
    pub fn new(tasks: Tasks, prs: anyhow::Result<Vec<PRRequest>>) -> Self {
        Self { tasks, prs, zone: Zone::Todo, cursor: 0, mode: Mode::Normal, pending: None, dirty: false, quit: false, split: 100.0 }
    }

    fn split_target(&self) -> f64 {
        match self.zone {
            Zone::Todo => 100.0,
            Zone::Done => 0.0,
            // Keep showing whichever tab was last active.
            Zone::Prs => self.split.round(),
        }
    }

    pub fn animating(&self) -> bool {
        self.split != self.split_target()
    }

    // ponytail: eases a fixed fraction per frame, so speed follows the ~60fps tick; use elapsed time if it ever stutters.
    pub fn tick(&mut self) {
        let target = self.split_target();
        self.split += (target - self.split) * 0.2;
        if (target - self.split).abs() < 0.5 {
            self.split = target;
        }
    }

    pub fn list(&self, zone: Zone) -> &[Task] {
        match zone {
            Zone::Todo => &self.tasks.todo,
            Zone::Done => &self.tasks.done,
            Zone::Prs => &[],
        }
    }

    fn list_mut(&mut self) -> &mut Vec<Task> {
        match self.zone {
            Zone::Todo => &mut self.tasks.todo,
            Zone::Done => &mut self.tasks.done,
            Zone::Prs => unreachable!("task op on PR pane"),
        }
    }

    fn len(&self) -> usize {
        match (self.zone, &self.prs) {
            (Zone::Prs, Ok(prs)) => prs.len(),
            (Zone::Prs, Err(_)) => 0,
            (zone, _) => self.list(zone).len(),
        }
    }

    /// Inclusive index range of the visual selection, or just the cursor.
    pub fn selection(&self) -> (usize, usize) {
        match self.mode {
            Mode::Visual { anchor } => (anchor.min(self.cursor), anchor.max(self.cursor)),
            _ => (self.cursor, self.cursor),
        }
    }

    fn switch_zone(&mut self, zone: Zone) {
        self.zone = zone;
        self.cursor = self.cursor.min(self.len().saturating_sub(1));
    }

    fn after_edit(&mut self) {
        self.mode = Mode::Normal;
        self.cursor = self.cursor.min(self.len().saturating_sub(1));
        self.dirty = true;
    }

    fn take_selection(&mut self) -> Vec<Task> {
        if self.len() == 0 {
            return Vec::new();
        }
        let (a, b) = self.selection();
        self.list_mut().drain(a..=b).collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if matches!(self.mode, Mode::Insert { .. }) {
            self.insert_key(key);
        } else if key.modifiers.contains(KeyModifiers::CONTROL) {
            // Ctrl-hjkl: move between panes, like vim windows.
            self.pending = None;
            self.mode = Mode::Normal;
            match key.code {
                KeyCode::Char('h') => self.switch_zone(Zone::Todo),
                KeyCode::Char('l') => self.switch_zone(Zone::Done),
                KeyCode::Char('j') => self.switch_zone(Zone::Prs),
                KeyCode::Char('k') => self.switch_zone(Zone::Todo),
                // ponytail: blocks the UI while gh runs (~1s); move to a thread if that gets annoying.
                KeyCode::Char('r') => {
                    self.prs = GithubClient::new().my_open_prs();
                    self.switch_zone(self.zone);
                }
                _ => {}
            }
        } else {
            self.normal_key(key.code);
        }
    }

    fn normal_key(&mut self, code: KeyCode) {
        let pending = self.pending.take();
        let visual = matches!(self.mode, Mode::Visual { .. });
        let tasks = self.zone != Zone::Prs;
        let last = self.len().saturating_sub(1);

        match code {
            KeyCode::Char('j') | KeyCode::Down => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') if pending == Some('g') => self.cursor = 0,
            KeyCode::Char('g') => self.pending = Some('g'),
            KeyCode::Char('G') => self.cursor = last,
            // On the PR pane, `o` prefills the new task from the selected PR.
            KeyCode::Char('o') if !visual => {
                let draft = match (self.zone, &self.prs) {
                    (Zone::Prs, Ok(prs)) => prs
                        .get(self.cursor)
                        .map(|pr| Task {
                            title: format!("{} #{}: {}", pr.repo, pr.number, pr.title),
                            description: pr.url.clone(),
                        })
                        .unwrap_or_default(),
                    _ => Task::default(),
                };
                self.switch_zone(Zone::Todo);
                let cursor = draft.title.len();
                self.mode = Mode::Insert { field: Field::Title, editing: None, draft, cursor };
            }
            KeyCode::Char('q') if !visual => self.quit = true,
            // Everything below edits tasks, so not on the PR pane.
            _ if !tasks => {}
            KeyCode::Char('V' | 'v') if !visual && self.len() > 0 => {
                self.mode = Mode::Visual { anchor: self.cursor }
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            // `d` in visual, `dd` in normal: delete.
            KeyCode::Char('d') if visual || pending == Some('d') => {
                self.take_selection();
                self.after_edit();
            }
            KeyCode::Char('d') => self.pending = Some('d'),
            // Move selection to the other zone (todo -> done, done -> todo).
            KeyCode::Char('x' | ' ') => {
                let moved = self.take_selection();
                match self.zone {
                    Zone::Todo => self.tasks.done.extend(moved),
                    Zone::Done => self.tasks.todo.extend(moved),
                    Zone::Prs => unreachable!(),
                }
                self.after_edit();
            }
            KeyCode::Char('i' | 'e') if !visual && self.len() > 0 => {
                let draft = self.list(self.zone)[self.cursor].clone();
                let cursor = draft.title.len();
                self.mode = Mode::Insert { field: Field::Title, editing: Some(self.cursor), draft, cursor };
            }
            _ => {}
        }
    }

    fn insert_key(&mut self, key: KeyEvent) {
        let Mode::Insert { field, draft, cursor, .. } = &mut self.mode else { return };

        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Tab | KeyCode::BackTab => {
                *field = if *field == Field::Title { Field::Description } else { Field::Title };
                *cursor = match field {
                    Field::Title => draft.title.len(),
                    Field::Description => draft.description.len(),
                };
            }
            KeyCode::Enter => {
                let Mode::Insert { editing, draft, .. } = std::mem::replace(&mut self.mode, Mode::Normal) else {
                    unreachable!()
                };
                if draft.title.trim().is_empty() {
                    return;
                }
                match editing {
                    Some(i) => self.list_mut()[i] = draft,
                    None => {
                        self.tasks.todo.push(draft);
                        self.cursor = self.tasks.todo.len() - 1;
                    }
                }
                self.dirty = true;
            }
            _ => {
                let text = match field {
                    Field::Title => &mut draft.title,
                    Field::Description => &mut draft.description,
                };
                edit_line(text, cursor, key);
            }
        }
    }
}

/// Start of the whitespace-delimited word before `from` (readline's Ctrl-w / Alt-b).
fn word_start(s: &str, from: usize) -> usize {
    let t = s[..from].trim_end();
    t.char_indices().rev().find(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8())
}

/// End of the word after `from` (readline's Alt-f / Alt-d).
fn word_end(s: &str, from: usize) -> usize {
    let rest = &s[from..];
    let gap = rest.len() - rest.trim_start().len();
    let word = &rest[gap..];
    from + gap + word.find(char::is_whitespace).unwrap_or(word.len())
}

/// Bash/readline-style single-line editing.
fn edit_line(text: &mut String, cur: &mut usize, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let prev = text[..*cur].char_indices().next_back().map_or(0, |(i, _)| i);
    let next = text[*cur..].chars().next().map_or(*cur, |c| *cur + c.len_utf8());

    match (key.code, ctrl, alt) {
        (KeyCode::Left, true, _) | (KeyCode::Char('b'), _, true) => *cur = word_start(text, *cur),
        (KeyCode::Right, true, _) | (KeyCode::Char('f'), _, true) => *cur = word_end(text, *cur),
        (KeyCode::Char('d'), _, true) => {
            let end = word_end(text, *cur);
            text.replace_range(*cur..end, "");
        }
        (KeyCode::Char('w'), true, _) | (KeyCode::Backspace, _, true) => {
            let start = word_start(text, *cur);
            text.replace_range(start..*cur, "");
            *cur = start;
        }
        (KeyCode::Left, ..) | (KeyCode::Char('b'), true, _) => *cur = prev,
        (KeyCode::Right, ..) | (KeyCode::Char('f'), true, _) => *cur = next,
        (KeyCode::Home, ..) | (KeyCode::Char('a'), true, _) => *cur = 0,
        (KeyCode::End, ..) | (KeyCode::Char('e'), true, _) => *cur = text.len(),
        (KeyCode::Backspace, ..) | (KeyCode::Char('h'), true, _) => {
            text.replace_range(prev..*cur, "");
            *cur = prev;
        }
        (KeyCode::Delete, ..) | (KeyCode::Char('d'), true, _) => text.replace_range(*cur..next, ""),
        (KeyCode::Char('u'), true, _) => {
            text.replace_range(..*cur, "");
            *cur = 0;
        }
        (KeyCode::Char('k'), true, _) => text.truncate(*cur),
        (KeyCode::Char(c), false, false) => {
            text.insert(*cur, c);
            *cur += c.len_utf8();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(app: &mut App, s: &str) {
        for c in s.chars() {
            let code = match c {
                '\n' => KeyCode::Enter,
                '\t' => KeyCode::Tab,
                '\x1b' => KeyCode::Esc,
                _ => KeyCode::Char(c),
            };
            app.handle_key(KeyEvent::from(code));
        }
    }

    fn titles(v: &[Task]) -> Vec<&str> {
        v.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn vim_flow() {
        let mut app = App::new(Tasks::default(), Ok(vec![]));
        keys(&mut app, "oa\tdesc a\nob\noc\nod\n");
        assert_eq!(titles(&app.tasks.todo), ["a", "b", "c", "d"]);
        assert_eq!(app.tasks.todo[0].description, "desc a");

        keys(&mut app, "ggx"); // a -> done
        assert_eq!(titles(&app.tasks.done), ["a"]);

        keys(&mut app, "Vjd"); // group delete b, c
        assert_eq!(titles(&app.tasks.todo), ["d"]);

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        keys(&mut app, "dd"); // delete a from done
        assert!(app.tasks.done.is_empty());

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        keys(&mut app, "exx\n"); // edit title of d
        assert_eq!(titles(&app.tasks.todo), ["dxx"]);

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        keys(&mut app, "ddx"); // task keys are ignored on the PR pane
        assert_eq!(app.zone, Zone::Prs);
        assert_eq!(titles(&app.tasks.todo), ["dxx"]);
    }

    #[test]
    fn pr_to_todo() {
        use crate::github::{CIStatus, PRRequest, ReviewStatus};
        let pr = |n| PRRequest {
            repo: "me/repo".into(),
            number: n,
            title: format!("pr {n}"),
            url: format!("https://x/{n}"),
            author: "me".into(),
            is_draft: false,
            review_status: ReviewStatus::Unknown,
            ci_status: CIStatus::Unknown,
        };
        let tasks = Tasks { todo: vec![Task { title: "t".into(), ..Default::default() }], done: vec![] };
        let mut app = App::new(tasks, Ok(vec![pr(1), pr(2), pr(3)]));

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        keys(&mut app, "Go\x1b"); // cancel on PR 3: cursor must clamp to the 1-item todo list
        keys(&mut app, "dd");
        assert!(app.tasks.todo.is_empty());

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        keys(&mut app, "jo\n");
        assert_eq!(app.zone, Zone::Todo);
        assert_eq!(app.tasks.todo, [Task { title: "me/repo #2: pr 2".into(), description: "https://x/2".into() }]);
    }

    #[test]
    fn readline_keys() {
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        let alt = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
        let edit = |text: &str, keys: &[KeyEvent]| {
            let (mut t, mut cur) = (String::new(), 0);
            for c in text.chars() {
                edit_line(&mut t, &mut cur, KeyEvent::from(KeyCode::Char(c)));
            }
            for k in keys {
                edit_line(&mut t, &mut cur, *k);
            }
            (t, cur)
        };

        // C-w "world", M-b before "big", C-k drops it, C-a C-f past "h", C-d the multibyte é,
        // type "e", C-e to end, C-h the trailing space.
        let keys = [ctrl('w'), alt('b'), ctrl('k'), ctrl('a'), ctrl('f'), ctrl('d')];
        let (t, cur) = edit("héllo big world", &keys);
        assert_eq!((t.as_str(), cur), ("hllo ", 1));
        let keys = [keys.as_slice(), &[KeyEvent::from(KeyCode::Char('e')), ctrl('e'), ctrl('h')]].concat();
        assert_eq!(edit("héllo big world", &keys), ("hello".into(), 5));

        assert_eq!(edit("one two", &[ctrl('a'), alt('f'), alt('d')]), ("one".into(), 3));
        assert_eq!(edit("one two", &[alt('b'), ctrl('u')]), ("two".into(), 0));
    }

    #[test]
    fn split_slides_to_focused_pane() {
        let mut app = App::new(Tasks::default(), Ok(vec![]));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert!(app.animating());
        for _ in 0..100 {
            app.tick();
        }
        assert_eq!(app.split, 0.0);
        assert!(!app.animating());

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        app.tick();
        assert_eq!(app.split, 0.0); // PR pane keeps the Done tab showing
    }
}
