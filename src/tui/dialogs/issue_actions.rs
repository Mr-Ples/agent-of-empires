use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_textarea::TextArea;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::github::{IssueEditRequest, IssueRecord};
use crate::tui::components::render_text_field;
use crate::tui::styles::Theme;
use super::DialogResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueAction {
    Edit {
        request: IssueEditRequest,
        new_labels: Vec<String>,
    },
    SetState(crate::github::IssueState),
    AttachSession(String),
    DetachSession,
    UpdateFromBase,
}

pub struct IssueActionsDialog {
    issue: IssueRecord,
    menu_cursor: usize,
    editing: bool,
    edit_field: usize,
    title: Input,
    body: TextArea<'static>,
    label_options: Vec<String>,
    selected_labels: Vec<String>,
    label_cursor: usize,
    new_label: Option<Input>,
    new_labels: Vec<String>,
    attached_session_id: Option<String>,
    session_choices: Vec<(String, String)>,
    session_picker: bool,
    session_cursor: usize,
}

impl IssueActionsDialog {
    pub fn new(
        issue: IssueRecord,
        mut label_options: Vec<String>,
        attached_session_id: Option<String>,
        session_choices: Vec<(String, String)>,
    ) -> Self {
        label_options.push(crate::github::DEFAULT_TRIAGE_LABEL.to_string());
        for label in &issue.labels {
            label_options.push(label.name.clone());
        }
        label_options.sort_by_key(|label| label.to_lowercase());
        label_options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        let selected_labels = issue.labels.iter().map(|label| label.name.clone()).collect();
        let mut body = TextArea::new(
            issue
                .body
                .as_deref()
                .unwrap_or_default()
                .lines()
                .map(ToString::to_string)
                .collect(),
        );
        body.set_style(Style::default());
        Self {
            title: Input::new(issue.title.clone()),
            body,
            label_options,
            selected_labels,
            label_cursor: 0,
            new_label: None,
            new_labels: Vec::new(),
            issue,
            menu_cursor: 0,
            editing: false,
            edit_field: 0,
            attached_session_id,
            session_choices,
            session_picker: false,
            session_cursor: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> DialogResult<IssueAction> {
        if self.session_picker {
            match key.code {
                KeyCode::Esc => self.session_picker = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.session_cursor = self.session_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.session_choices.is_empty() {
                        self.session_cursor =
                            (self.session_cursor + 1).min(self.session_choices.len() - 1);
                    }
                }
                KeyCode::Enter => {
                    if let Some((id, _)) = self.session_choices.get(self.session_cursor) {
                        return DialogResult::Submit(IssueAction::AttachSession(id.clone()));
                    }
                }
                _ => {}
            }
            return DialogResult::Continue;
        }

        if let Some(input) = &mut self.new_label {
            match key.code {
                KeyCode::Esc => self.new_label = None,
                KeyCode::Enter => {
                    let label = input.value().trim().to_string();
                    if !label.is_empty()
                        && !self.label_options.iter().any(|existing| existing.eq_ignore_ascii_case(&label))
                    {
                        self.label_options.push(label.clone());
                        self.label_options.sort_by_key(|name| name.to_lowercase());
                        self.new_labels.push(label.clone());
                        self.selected_labels.push(label);
                        self.label_cursor = self
                            .label_options
                            .iter()
                            .position(|name| name.eq_ignore_ascii_case(self.selected_labels.last().unwrap()))
                            .unwrap_or_default();
                    }
                    self.new_label = None;
                }
                _ => {
                    input.handle_event(&crossterm::event::Event::Key(key));
                }
            }
            return DialogResult::Continue;
        }
        if key.code == KeyCode::Esc {
            if self.editing {
                self.editing = false;
                return DialogResult::Continue;
            }
            return DialogResult::Cancel;
        }
        if self.editing {
            match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    self.edit_field = (self.edit_field + 1) % 3;
                }
                KeyCode::Enter if self.edit_field == 2 => {
                    return DialogResult::Submit(IssueAction::Edit {
                        request: IssueEditRequest {
                            title: Some(self.title.value().trim().to_string()),
                            body: Some(self.body.lines().join("\n")),
                            labels: Some(self.selected_labels.clone()),
                        },
                        new_labels: self.new_labels.clone(),
                    });
                }
                KeyCode::Up if self.edit_field == 2 => self.label_cursor = self.label_cursor.saturating_sub(1),
                KeyCode::Down if self.edit_field == 2 => {
                    self.label_cursor = (self.label_cursor + 1).min(self.label_options.len());
                }
                KeyCode::Char(' ') if self.edit_field == 2 => self.toggle_label(),
                _ => match self.edit_field {
                    0 => {
                        self.title.handle_event(&crossterm::event::Event::Key(key));
                    }
                    1 => {
                        self.body.input(key);
                    }
                    _ => {}
                },
            }
            return DialogResult::Continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.menu_cursor = self.menu_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.menu_cursor = (self.menu_cursor + 1).min(if self.attached_session_id.is_some() { 4 } else { 3 }),
            KeyCode::Enter => match self.menu_cursor {
                0 => self.editing = true,
                1 => {
                    return DialogResult::Submit(IssueAction::SetState(if self.issue.state == crate::github::IssueState::Open {
                        crate::github::IssueState::Closed
                    } else {
                        crate::github::IssueState::Open
                    }));
                }
                2 => {
                    if self.attached_session_id.is_some() {
                        return DialogResult::Submit(IssueAction::DetachSession);
                    }
                    self.session_picker = true;
                    self.session_cursor = 0;
                }
                3 if self.attached_session_id.is_some() => {
                    return DialogResult::Submit(IssueAction::UpdateFromBase);
                }
                _ => return DialogResult::Cancel,
            },
            _ => {}
        }
        DialogResult::Continue
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if self.session_picker {
            self.render_session_picker(frame, area, theme);
            return;
        }
        let dialog_area = super::centered_rect(area, 76, if self.editing { 20 } else { 13 });
        frame.render_widget(Clear, dialog_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .border_style(Style::default().fg(theme.accent))
            .title(if self.editing { " Edit GitHub Issue " } else { " Issue Actions " })
            .title_style(Style::default().fg(theme.title).bold());
        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        if !self.editing {
            let rows = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);
            frame.render_widget(Paragraph::new(format!("{}  {}", self.issue.issue_ref, self.issue.title)).style(Style::default().fg(theme.text)), rows[0]);
            let attachment_label = if self.attached_session_id.is_some() {
                "Detach session"
            } else {
                "Attach existing session"
            };
            let mut actions = vec![
                "Edit issue",
                if self.issue.state == crate::github::IssueState::Open { "Close issue" } else { "Reopen issue" },
                attachment_label,
            ];
            if self.attached_session_id.is_some() {
                actions.push("Update from base branch");
            }
            actions.push("Cancel");
            for (index, label) in actions.iter().enumerate() {
                let style = if index == self.menu_cursor { Style::default().fg(theme.background).bg(theme.accent) } else { Style::default().fg(theme.text) };
                frame.render_widget(Paragraph::new(format!("{} {}", if index == self.menu_cursor { ">" } else { " " }, label)).style(style), rows[index + 1]);
            }
            return;
        }

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .split(inner);
        render_text_field(frame, rows[0], "Title:", &self.title, self.edit_field == 0, None, theme);
        let body_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(if self.edit_field == 1 { theme.accent } else { theme.border })).title(" Body ");
        let mut body = self.body.clone();
        body.set_block(body_block);
        body.set_style(Style::default().fg(theme.text));
        frame.render_widget(&body, rows[1]);
        let labels_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if self.edit_field == 2 { theme.accent } else { theme.border }))
            .title(" Labels, Space to toggle ");
        let labels_inner = labels_block.inner(rows[2]);
        frame.render_widget(labels_block, rows[2]);
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
                let style = if index == self.label_cursor && self.edit_field == 2 {
                    Style::default().fg(theme.background).bg(theme.accent)
                } else {
                    Style::default().fg(theme.text)
                };
                Line::styled(format!("{marker} {label}"), style)
            })
            .collect::<Vec<_>>();
        let create_index = self.label_options.len();
        if self.label_cursor >= start && self.label_cursor <= start + visible_height {
            let style = if self.label_cursor == create_index && self.edit_field == 2 {
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
            Paragraph::new("Tab: next field  ↑↓: choose  Space: toggle  Enter: save  Esc: back")
                .style(Style::default().fg(theme.dimmed)),
            rows[3],
        );
    }

    fn render_session_picker(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let height = (self.session_choices.len() as u16 + 4).clamp(8, 20);
        let dialog_area = super::centered_rect(area, 76, height);
        frame.render_widget(Clear, dialog_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent))
            .title(" Attach existing session ");
        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);
        if self.session_choices.is_empty() {
            frame.render_widget(
                Paragraph::new("No sessions are available to attach. Press Esc.")
                    .style(Style::default().fg(theme.text)),
                inner,
            );
            return;
        }
        let rows = Layout::vertical(
            std::iter::once(Constraint::Length(1))
                .chain((0..self.session_choices.len()).map(|_| Constraint::Length(1)))
                .chain(std::iter::once(Constraint::Length(1)))
                .collect::<Vec<_>>(),
        )
        .split(inner);
        frame.render_widget(
            Paragraph::new("Select a session, Enter attaches, Esc returns")
                .style(Style::default().fg(theme.text)),
            rows[0],
        );
        for (index, (id, title)) in self.session_choices.iter().enumerate() {
            let label = format!(
                "{} {} [{}]",
                if index == self.session_cursor { ">" } else { " " },
                title,
                id
            );
            let style = if index == self.session_cursor {
                Style::default().fg(theme.background).bg(theme.accent)
            } else {
                Style::default().fg(theme.text)
            };
            frame.render_widget(Paragraph::new(label).style(style), rows[index + 1]);
        }
    }
}
