use crate::application::{DesktopTerminalScope, DesktopTerminalSessionId, DesktopTerminalSnapshot};

#[derive(Clone, Debug)]
pub enum TerminalViewMessage {
    Write(DesktopTerminalSessionId, Vec<u8>),
    Interrupt(DesktopTerminalSessionId),
    Eof(DesktopTerminalSessionId),
    CopyVisible(DesktopTerminalSessionId),
    Resize(DesktopTerminalSessionId, u16, u16),
    Terminate(DesktopTerminalSessionId),
    Reveal(DesktopTerminalSessionId),
    NewSession(DesktopTerminalScope),
}

pub fn terminal_screen(snapshot: &DesktopTerminalSnapshot) -> nana_ui::runtime::TerminalScreen {
    use nana_ui::runtime::{
        TerminalCell, TerminalCursor, TerminalCursorShape, TerminalPosition, TerminalScreen,
    };
    if snapshot.cells.is_empty() {
        return TerminalScreen::blank(snapshot.columns, snapshot.rows);
    }
    TerminalScreen {
        columns: snapshot.columns,
        rows: snapshot.rows,
        cells: snapshot
            .cells
            .iter()
            .map(|cell| TerminalCell {
                text: if cell.text.is_empty() && cell.width > 0 {
                    " ".into()
                } else {
                    cell.text.clone().into()
                },
                width: cell.width,
                foreground: terminal_color(cell.foreground),
                background: terminal_color(cell.background),
                bold: cell.bold,
                dim: cell.dim,
                italic: cell.italic,
                underline: cell.underline,
                inverse: cell.inverse,
            })
            .collect::<Vec<_>>()
            .into(),
        cursor: Some(TerminalCursor {
            position: TerminalPosition {
                row: snapshot.cursor_row,
                column: snapshot.cursor_column,
            },
            shape: TerminalCursorShape::Block,
            visible: snapshot.cursor_visible
                && snapshot.scrollback_position == 0
                && snapshot.process.is_running(),
        }),
        application_cursor: snapshot.application_cursor,
        bracketed_paste: snapshot.bracketed_paste,
    }
}

fn terminal_color(color: lilia_contracts::TerminalCellColor) -> Option<[f32; 4]> {
    use lilia_contracts::TerminalCellColor;
    let rgb = match color {
        TerminalCellColor::Default => return None,
        TerminalCellColor::Rgb(rgb) => rgb,
        TerminalCellColor::Indexed(index) => match index {
            0..=15 => [
                [0, 0, 0],
                [205, 49, 49],
                [13, 188, 121],
                [229, 229, 16],
                [36, 114, 200],
                [188, 63, 188],
                [17, 168, 205],
                [229, 229, 229],
                [102, 102, 102],
                [241, 76, 76],
                [35, 209, 139],
                [245, 245, 67],
                [59, 142, 234],
                [214, 112, 214],
                [41, 184, 219],
                [255, 255, 255],
            ][usize::from(index)],
            16..=231 => {
                let i = index - 16;
                let level = |v| if v == 0 { 0 } else { 55 + v * 40 };
                [level(i / 36), level(i / 6 % 6), level(i % 6)]
            }
            _ => [8 + (index - 232) * 10; 3],
        },
    };
    Some([
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        1.0,
    ])
}

pub fn terminal_plain_text(snapshot: &DesktopTerminalSnapshot) -> String {
    terminal_rows_plain_text(&snapshot.screen)
}

fn terminal_rows_plain_text(rows: &[crate::application::DesktopTerminalRow]) -> String {
    rows.iter()
        .map(|row| row.text.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end_matches('\n')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use crate::application::DesktopTerminalRow;

    use super::terminal_rows_plain_text;

    #[test]
    fn copied_terminal_output_preserves_lines_and_omits_screen_padding() {
        let rows = vec![
            DesktopTerminalRow {
                text: "first   ".to_owned(),
                styles: Vec::new(),
            },
            DesktopTerminalRow {
                text: "second".to_owned(),
                styles: Vec::new(),
            },
            DesktopTerminalRow {
                text: "        ".to_owned(),
                styles: Vec::new(),
            },
        ];
        assert_eq!(terminal_rows_plain_text(&rows), "first\nsecond");
    }
}
