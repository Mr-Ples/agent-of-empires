//! Minimal GitHub Issue creation dialog for the TUI Issues view.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_textarea::TextArea;
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
    pub new_labels: Vec<String>,
}

pub struct NewIssueDialog {
    repository: IssueRepository,
    repository_label: String,
    title: Input,
    body: TextArea<'static>,
    label_options: Vec<String>,
    selected_labels: Vec<String>,
    label_cursor: usize,
    new_label: Option<Input>,
    new_labels: Vec<String>,
    focused_field: NewIssueField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewIssueField {
    Title,
    Body,
    Labels,
}

impl NewIssueDialog {
    pub fn new(repository: IssueRepository, mut label_options: Vec<String>) -> Self {
        let repository_label = format!("{}/{}", repository.owner, repository.repo);
        label_options.push(crate::github::DEFAULT_TRIAGE_LABEL.to_string());
        label_options.sort_by_key(|label| label.to_lowercase());
        label_options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        let selected_labels = vec![crate::github::DEFAULT_TRIAGE_LABEL.to_string()];
        Self {
            repository,
            repository_label,
            title: Input::default(),
            body: TextArea::new(vec![String::new()]),
            label_options,
            selected_labels,
            label_cursor: 0,
            new_label: None,
            new_labels: Vec::new(),
            focused_field: NewIssueField::Title,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> DialogResult<NewIssueData> {
        if let Some(input) = &mut self.new_label {
            match key.code {
                KeyCode::Esc => self.new_label = None,
                KeyCode::Enter => {
                    let label = input.value().trim().to_string();
                    if !label.is_empty()
                        && !self.label_options[..self.label_options.len().saturating_sub(1)]
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(&label))
                    {
                        self.label_options.insert(self.label_options.len() - 1, label.clone());
                        self.new_labels.push(label.clone());
                        self.selected_labels.push(label);
                        self.label_cursor = self.label_options.len() - 2;
                    }
                    self.new_label = None;
                }
                _ => {
                    input.handle_event(&crossterm::event::Event::Key(key));
                }
            }
            return DialogResult::Continue;
        }
        match key.code {
            KeyCode::Esc => DialogResult::Cancel,
            KeyCode::Tab => {
                self.focused_field = match self.focused_field {
                    NewIssueField::Title => NewIssueField::Body,
                    NewIssueField::Body => NewIssueField::Labels,
                    NewIssueField::Labels => NewIssueField::Title,
                };
                DialogResult::Continue
            }
            KeyCode::BackTab => {
                self.focused_field = match self.focused_field {
                    NewIssueField::Title => NewIssueField::Labels,
                    NewIssueField::Body => NewIssueField::Title,
                    NewIssueField::Labels => NewIssueField::Body,
                };
                DialogResult::Continue
            }
            KeyCode::Enter if self.focused_field == NewIssueField::Labels => self.submit(),
            KeyCode::Enter if self.focused_field == NewIssueField::Title => self.submit(),
            KeyCode::Enter => {
                self.body.insert_newline();
                DialogResult::Continue
            }
            KeyCode::Up if self.focused_field == NewIssueField::Labels => {
                self.label_cursor = self.label_cursor.saturating_sub(1);
                DialogResult::Continue
            }
            KeyCode::Down if self.focused_field == NewIssueField::Labels => {
                if !self.label_options.is_empty() {
                    self.label_cursor = (self.label_cursor + 1).min(self.label_options.len());
                }
                DialogResult::Continue
            }
            KeyCode::Char(' ') if self.focused_field == NewIssueField::Labels => {
                self.toggle_label();
                DialogResult::Continue
            }
            _ => {
                match self.focused_field {
                    NewIssueField::Title => {
                        self.title
                            .handle_event(&crossterm::event::Event::Key(key));
                    }
                    NewIssueField::Labels => {}
                    NewIssueField::Body => {
                        self.body.input(key);
                    }
                }
                DialogResult::Continue
            }
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        match self.focused_field {
            NewIssueField::Title => super::paste_into_input(&mut self.title, text),
            NewIssueField::Body => {
                self.body.insert_str(text);
            }
            NewIssueField::Labels => {}
        }
    }

    fn toggle_label(&mut self) {
        if self.label_cursor == self.label_options.len() {
            self.new_label = Some(Input::default());
            return;
        }
        let Some(label) = self.label_options.get(self.label_cursor).cloned() else {
            return;
        };
        if let Some(index) = self
            .selected_labels
            .iter()
            .position(|selected| selected.eq_ignore_ascii_case(&label))
        {
            self.selected_labels.remove(index);
        } else {
            self.selected_labels.push(label);
        }
    }

    fn submit(&self) -> DialogResult<NewIssueData> {
        let title = self.title.value().trim();
        if title.is_empty() {
            return DialogResult::Continue;
        }

        let body = self.body.lines().join("\n").trim().to_string();
        let mut request = IssueCreateRequest::new(title);
        request.body = (!body.is_empty()).then_some(body);
        request.labels = self.selected_labels.clone();
        DialogResult::Submit(NewIssueData {
            repository: self.repository.clone(),
            request,
            new_labels: self.new_labels.clone(),
        })
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let dialog_area = super::centered_rect(area, 76, 20);
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
                Constraint::Length(5),
                Constraint::Length(7),
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
        let body_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if self.focused_field == NewIssueField::Body {
                theme.accent
            } else {
                theme.border
            }))
            .title(" Body ");
        let mut body = self.body.clone();
        body.set_block(body_block);
        body.set_style(Style::default().fg(theme.text));
        frame.render_widget(&body, chunks[2]);
        let labels_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if self.focused_field == NewIssueField::Labels {
                theme.accent
            } else {
                theme.border
            }))
            .title(" Labels, Space to toggle ");
        let labels_inner = labels_block.inner(chunks[3]);
        frame.render_widget(labels_block, chunks[3]);
        let visible_height = labels_inner.height as usize;
        let start = self.label_cursor.saturating_sub(visible_height.saturating_sub(1));
        let mut label_lines = self
            .label_options
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_height)
            .map(|(index, label)| {
                let selected = self
                    .selected_labels
                    .iter()
                    .any(|selected| selected.eq_ignore_ascii_case(label));
                let marker = if selected { "[x]" } else { "[ ]" };
                let style = if index == self.label_cursor && self.focused_field == NewIssueField::Labels {
                    Style::default().fg(theme.background).bg(theme.accent)
                } else {
                    Style::default().fg(theme.text)
                };
                Line::styled(format!("{marker} {label}"), style)
            })
            .collect::<Vec<_>>();
        let create_index = self.label_options.len();
        if self.label_cursor >= start && self.label_cursor <= start + visible_height {
            let selected = self.label_cursor == create_index;
            let style = if selected && self.focused_field == NewIssueField::Labels {
                Style::default().fg(theme.background).bg(theme.accent)
            } else {
                Style::default().fg(theme.accent)
            };
            label_lines.push(Line::styled("+ Create new label...", style));
        }
        frame.render_widget(Paragraph::new(label_lines), labels_inner);

        if let Some(input) = &self.new_label {
            let modal = super::centered_rect(area, 52, 5);
            frame.render_widget(Clear, modal);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.accent))
                .title(" Create GitHub Label ");
            let inner = block.inner(modal);
            frame.render_widget(block, modal);
            render_text_field(frame, inner, "Name:", input, true, None, theme);
        }
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Tab", Style::default().fg(theme.hint)),
                Span::raw(" next field  "),
                Span::styled("↑↓", Style::default().fg(theme.hint)),
                Span::raw(" choose  "),
                Span::styled("Space", Style::default().fg(theme.hint)),
                Span::raw(" toggle  "),
                Span::styled("Enter", Style::default().fg(theme.hint)),
                Span::raw(" next/create  "),
                Span::styled("Esc", Style::default().fg(theme.hint)),
                Span::raw(" cancel"),
            ])),
            chunks[4],
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
        let mut dialog = NewIssueDialog::new(IssueRepository::new("owner", "repo").unwrap(), vec![]);
        for ch in "Ship thing".chars() {
            dialog.handle_key(key(KeyCode::Char(ch)));
        }

        match dialog.handle_key(key(KeyCode::Enter)) {
            DialogResult::Submit(data) => {
                assert_eq!(data.request.title, "Ship thing");
                assert_eq!(data.request.body, None);
                assert_eq!(data.request.labels, vec![crate::github::DEFAULT_TRIAGE_LABEL]);
                assert!(data.new_labels.is_empty());
                assert!(!data.request.apply_default_triage_label);
                assert_eq!(data.repository.owner, "owner");
                assert_eq!(data.repository.repo, "repo");
            }
            _ => panic!("expected submit"),
        }
    }

    #[test]
    fn submit_includes_body_and_selected_labels() {
        let mut dialog = NewIssueDialog::new(
            IssueRepository::new("owner", "repo").unwrap(),
            vec!["bug".to_string(), "ready-for-agent".to_string()],
        );
        for ch in "Ship thing".chars() {
            dialog.handle_key(key(KeyCode::Char(ch)));
        }
        dialog.handle_key(key(KeyCode::Tab));
        dialog.handle_paste("Details\nwith context");
        dialog.handle_key(key(KeyCode::Tab));
        dialog.handle_key(key(KeyCode::Char(' ')));
        dialog.handle_key(key(KeyCode::Down));
        dialog.handle_key(key(KeyCode::Down));
        dialog.handle_key(key(KeyCode::Char(' ')));

        match dialog.handle_key(key(KeyCode::Enter)) {
            DialogResult::Submit(data) => {
                assert_eq!(data.request.body.as_deref(), Some("Details\nwith context"));
                assert_eq!(
                    data.request.labels,
                    vec![crate::github::DEFAULT_TRIAGE_LABEL, "bug", "ready-for-agent"]
                );
            }
            _ => panic!("expected submit"),
        }
    }

    #[test]
    fn empty_title_does_not_submit() {
        let mut dialog = NewIssueDialog::new(IssueRepository::new("owner", "repo").unwrap(), vec![]);
        assert!(matches!(
            dialog.handle_key(key(KeyCode::Enter)),
            DialogResult::Continue
        ));
    }

    #[test]
    fn deselected_triage_label_is_not_submitted() {
        let mut dialog = NewIssueDialog::new(IssueRepository::new("owner", "repo").unwrap(), vec![]);
        for ch in "Ship thing".chars() {
            dialog.handle_key(key(KeyCode::Char(ch)));
        }
        dialog.handle_key(key(KeyCode::Tab));
        dialog.handle_key(key(KeyCode::Tab));
        dialog.handle_key(key(KeyCode::Char(' ')));

        match dialog.handle_key(key(KeyCode::Enter)) {
            DialogResult::Submit(data) => {
                assert!(data.request.labels.is_empty());
                assert!(!data.request.apply_default_triage_label);
            }
            _ => panic!("expected submit"),
        }
    }
}
