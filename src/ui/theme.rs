//! All colours of the IDE (issue #174). The frame uses the colours of mode 7
//! on black. No pane file has a colour value.

use ratatui::style::{Color, Style};

/// The 8 steady colours of a BBC Micro, in the order of the colour numbers.
const STEADY: [Color; 8] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::White,
];

/// The terminal colour of a logical colour, for a mode with `colours`
/// colours. 2 colours: black and white. 4 colours: black, red, yellow and
/// white. 8 or 16 colours: the 8 steady colours, 2 times for 16, so that a
/// flashing colour shows as its steady colour.
pub(crate) fn palette(colour: u8, colours: u8) -> Color {
    let index = match colours {
        2 => [0, 7][usize::from(colour % 2)],
        4 => [0, 1, 3, 7][usize::from(colour % 4)],
        _ => usize::from(colour % 8),
    };
    STEADY[index]
}

/// White text on black: the text of the frame and of the panes.
pub(super) fn text() -> Style {
    Style::new().fg(Color::White).bg(Color::Black)
}

/// The text of less importance, such as the line numbers of the editor.
pub(super) fn dim() -> Style {
    Style::new().fg(Color::DarkGray).bg(Color::Black)
}

/// The mark of a fault in the text.
pub(super) fn fault() -> Style {
    Style::new().fg(Color::Red).bg(Color::Black)
}

/// The mode in the status line, and a question.
pub(super) fn highlight() -> Style {
    Style::new().fg(Color::Yellow).bg(Color::Black)
}

/// The status line text, the keys and the header of a list.
pub(super) fn info() -> Style {
    Style::new().fg(Color::Cyan).bg(Color::Black)
}

/// A text screen colour pair of the run pane, from the logical colours.
pub(super) fn screen(foreground: u8, background: u8, colours: u8) -> Style {
    Style::new()
        .fg(palette(foreground, colours))
        .bg(palette(background, colours))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_follows_the_colour_count_of_the_mode() {
        assert_eq!(palette(0, 2), Color::Black);
        assert_eq!(palette(1, 2), Color::White);
        assert_eq!(palette(1, 4), Color::Red);
        assert_eq!(palette(2, 4), Color::Yellow);
        assert_eq!(palette(3, 4), Color::White);
        assert_eq!(palette(4, 8), Color::Blue);
        assert_eq!(palette(6, 16), Color::Cyan);
        assert_eq!(palette(9, 16), Color::Red);
        assert_eq!(palette(15, 16), Color::White);
    }
}
