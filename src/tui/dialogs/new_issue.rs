//! Minimal GitHub Issue creation dialog for the TUI Issues view.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::*;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use super::DialogResult;
use crate::github::{IssueCreateRequest, IssueRepository};
use crate::tui::components::render_text_field;
use crate::tui::styles::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewIssueData {
    pub repository: IssueRepository,
    pub request: IssueCreateRequest,
}

pub struct NewIssueDialog {
    repository: IssueRepository,
    repository_label: String,
    title: Input,
}

impl NewIssueDialog {
    pub fn new(repository: IssueRepository) -> Self {
        let repository_label = format!("{}/{}", repository.owner, repository.repo);
        Self {
            repository,
            repository_label,
            title: Input::default(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> DialogResult<NewIssueData> {
        match key.code {
            KeyCode::Esc => DialogResult::Cancel,
            KeyCode::Enter => {
                let title = self.title.value().trim();
                if title.is_empty() {
                    return DialogResult::Continue;
                }
                let mut request = IssueCreateRequest::new(title);
                request.apply_default_triage_label = true;
                DialogResult::Submit(NewIssueData {
                    repository: self.repository.clone(),
                    request,
                })
            }
            _ => {
                self.title.handle_event(&crossterm::event::Event::Key(key));
                DialogResult::Continue
            }
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        super::paste_into_input(&mut self.title, text);
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let dialog_area = super::centered_rect(area, 62, 9);
        frame.render_widget(Clear, dialog_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .border_style(Style::default().fg(theme.accent))
            .title(" New GitHub Issue ")
            .title_style(Style::default().fg(theme.title).bold());
        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Repository: ", Style::default().fg(theme.dimmed)),
                Span::styled(&self.repository_label, Style::default().fg(theme.text)),
            ])),
            chunks[0],
        );
        render_text_field(frame, chunks[1], "Title:", &self.title, true, None, theme);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Enter", Style::default().fg(theme.hint)),
                Span::raw(" create  "),
                Span::styled("Esc", Style::default().fg(theme.hint)),
                Span::raw(" cancel"),
            ])),
            chunks[3],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn submit_requests_default_triage_label() {
        let mut dialog = NewIssueDialog::new(IssueRepository::new("owner", "repo").unwrap());
        for ch in "Ship thing".chars() {
            dialog.handle_key(key(KeyCode::Char(ch)));
        }

        match dialog.handle_key(key(KeyCode::Enter)) {
            DialogResult::Submit(data) => {
                assert_eq!(data.request.title, "Ship thing");
                assert!(data.request.apply_default_triage_label);
                assert_eq!(data.repository.owner, "owner");
                assert_eq!(data.repository.repo, "repo");
            }
            _ => panic!("expected submit"),
        }
    }

    #[test]
    fn empty_title_does_not_submit() {
        let mut dialog = NewIssueDialog::new(IssueRepository::new("owner", "repo").unwrap());
        assert!(matches!(
            dialog.handle_key(key(KeyCode::Enter)),
            DialogResult::Continue
        ));
    }
}
