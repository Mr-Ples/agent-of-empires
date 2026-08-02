use ratatui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellPos {
    col: u16,
    row: u16,
}

impl CellPos {
    fn new(col: u16, row: u16) -> Self {
        Self { col, row }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionGesture {
    Armed { anchor: CellPos },
    Selecting { anchor: CellPos, extent: CellPos },
    Finalized { anchor: CellPos, extent: CellPos },
}

#[derive(Debug, Default)]
pub(crate) struct GlobalSelection {
    gesture: Option<SelectionGesture>,
    snapshot: Vec<Vec<String>>,
    area: Rect,
    bounds: Option<Rect>,
}

impl GlobalSelection {
    pub(crate) fn capture_frame(&mut self, frame: &mut Frame) {
        let buf = frame.buffer_mut();
        self.area = buf.area;
        self.snapshot.clear();
        self.snapshot.reserve(buf.area.height as usize);
        for row in buf.area.y..buf.area.bottom() {
            let mut line = Vec::with_capacity(buf.area.width as usize);
            for col in buf.area.x..buf.area.right() {
                line.push(buf[(col, row)].symbol().to_string());
            }
            self.snapshot.push(line);
        }
    }

    pub(crate) fn mouse_down(&mut self, col: u16, row: u16, bounds: Option<Rect>) {
        self.bounds = bounds;
        self.gesture = Some(SelectionGesture::Armed {
            anchor: CellPos::new(col, row),
        });
    }

    pub(crate) fn mouse_drag(&mut self, col: u16, row: u16) -> bool {
        let Some(gesture) = self.gesture else {
            return false;
        };
        let extent = CellPos::new(col, row);
        match gesture {
            SelectionGesture::Armed { anchor } if anchor == extent => false,
            SelectionGesture::Armed { anchor } => {
                self.gesture = Some(SelectionGesture::Selecting { anchor, extent });
                true
            }
            SelectionGesture::Selecting { anchor, extent: old } if old == extent => false,
            SelectionGesture::Selecting { anchor, .. } => {
                self.gesture = Some(SelectionGesture::Selecting { anchor, extent });
                true
            }
            SelectionGesture::Finalized { .. } => false,
        }
    }

    pub(crate) fn mouse_up(&mut self) -> Option<String> {
        let SelectionGesture::Selecting { anchor, extent } = self.gesture? else {
            self.gesture = None;
            return None;
        };
        self.gesture = Some(SelectionGesture::Finalized { anchor, extent });
        self.selected_text(anchor, extent)
    }

    pub(crate) fn clear(&mut self) -> bool {
        self.bounds = None;
        self.gesture.take().is_some()
    }

    pub(crate) fn paint(&self, frame: &mut Frame, theme: &crate::tui::styles::Theme) {
        let Some((anchor, extent, finalized)) = self.selection_cells() else {
            return;
        };
        let bg = if finalized {
            theme.selection
        } else {
            theme.session_selection
        };
        let buf = frame.buffer_mut();
        for rect in self.selection_rects(anchor, extent) {
            let clipped = rect.intersection(buf.area);
            if clipped.width == 0 || clipped.height == 0 {
                continue;
            }
            for row in clipped.y..clipped.bottom() {
                for col in clipped.x..clipped.right() {
                    let cell = &mut buf[(col, row)];
                    cell.set_bg(bg);
                    cell.set_fg(theme.text);
                }
            }
        }
    }

    fn selection_cells(&self) -> Option<(CellPos, CellPos, bool)> {
        match self.gesture? {
            SelectionGesture::Selecting { anchor, extent } => Some((anchor, extent, false)),
            SelectionGesture::Finalized { anchor, extent } => Some((anchor, extent, true)),
            SelectionGesture::Armed { .. } => None,
        }
    }

    fn selected_text(&self, anchor: CellPos, extent: CellPos) -> Option<String> {
        if self.snapshot.is_empty() || self.area.width == 0 || self.area.height == 0 {
            return None;
        }
        let rects = self.selection_rects(anchor, extent);
        if rects.is_empty() {
            return None;
        }
        let mut out = String::new();
        for (idx, rect) in rects.into_iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            let row_idx = rect.y.saturating_sub(self.area.y) as usize;
            let Some(row) = self.snapshot.get(row_idx) else {
                continue;
            };
            let start = rect.x.saturating_sub(self.area.x) as usize;
            let end = rect.right().saturating_sub(self.area.x) as usize;
            for cell in row
                .iter()
                .skip(start)
                .take(end.saturating_sub(start))
            {
                out.push_str(cell);
            }
            trim_trailing_spaces_preserving_newlines(&mut out);
        }
        if out.chars().all(char::is_whitespace) {
            return None;
        }
        Some(out)
    }

    fn selection_rects(&self, anchor: CellPos, extent: CellPos) -> Vec<Rect> {
        if anchor == extent || self.area.width == 0 || self.area.height == 0 {
            return Vec::new();
        }
        let (start, end) = ordered(anchor, extent);
        let bounds = self.bounds.unwrap_or(self.area).intersection(self.area);
        if bounds.width == 0 || bounds.height == 0 {
            return Vec::new();
        }
        let top = bounds.y;
        let bottom = bounds.bottom().saturating_sub(1);
        let left = bounds.x;
        let right = bounds.right().saturating_sub(1);
        let start_row = start.row.clamp(top, bottom);
        let end_row = end.row.clamp(top, bottom);
        let mut rects = Vec::new();
        for row in start_row..=end_row {
            let from = if row == start.row {
                start.col.clamp(left, right)
            } else {
                left
            };
            let to = if row == end.row {
                end.col.clamp(left, right)
            } else {
                right
            };
            if to >= from {
                rects.push(Rect::new(from, row, to - from + 1, 1));
            }
        }
        rects
    }
}

fn ordered(a: CellPos, b: CellPos) -> (CellPos, CellPos) {
    if (a.row, a.col) <= (b.row, b.col) {
        (a, b)
    } else {
        (b, a)
    }
}

fn trim_trailing_spaces_preserving_newlines(s: &mut String) {
    while s.ends_with(' ') {
        s.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, widgets::Paragraph, Terminal};

    fn capture(text: &str, width: u16, height: u16) -> GlobalSelection {
        let mut selection = GlobalSelection::default();
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(Paragraph::new(text.to_string()), frame.area());
                selection.capture_frame(frame);
            })
            .unwrap();
        selection
    }

    #[test]
    fn copies_single_line_selection() {
        let mut selection = capture("hello world", 16, 2);
        selection.mouse_down(0, 0, None);
        assert!(selection.mouse_drag(4, 0));

        assert_eq!(selection.mouse_up().as_deref(), Some("hello"));
    }

    #[test]
    fn copies_multiline_selection_and_trims_row_padding() {
        let mut selection = capture("alpha\nbeta\ngamma", 16, 4);
        selection.mouse_down(2, 0, None);
        assert!(selection.mouse_drag(1, 2));

        assert_eq!(selection.mouse_up().as_deref(), Some("pha\nbeta\nga"));
    }

    #[test]
    fn clamps_selection_to_start_bounds() {
        let mut selection = capture("left    right\nleft    right", 16, 2);
        selection.mouse_down(0, 0, Some(Rect::new(0, 0, 6, 2)));
        assert!(selection.mouse_drag(15, 1));

        assert_eq!(selection.mouse_up().as_deref(), Some("left\nleft"));
    }
}
