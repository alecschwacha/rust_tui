use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::github::PRRequest;

pub fn render(frame: &mut Frame, prs: &[PRRequest]) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(10)])
        .split(frame.area());

    let main_panel = Paragraph::new("Going to be something good some day")
        .block(Block::default().title("Main Panel").borders(Borders::ALL));

    frame.render_widget(main_panel, areas[0]);

    let pr_text = build_pr_text(prs);
    let pr_panel =
        Paragraph::new(pr_text).block(Block::default().title("My Open PRs").borders(Borders::ALL));

    frame.render_widget(pr_panel, areas[1]);
}

fn build_pr_text(prs: &[PRRequest]) -> String {
    if prs.is_empty() {
        return "No open PRs found.".to_string();
    }
    prs.iter()
        .map(|pr| {
            format!(
                "{} #{}: {} {} ({})",
                pr.repo, pr.number, pr.title, pr.is_draft, pr.url
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}
