//! The VDU statement: a queue of display bytes. A control code from 0 to 31
//! waits for its parameter bytes, then runs. The queue stays between VDU
//! statements, so `VDU 22` and then `VDU 4` is the same as `VDU 22, 4`.

use super::Executor;
use crate::error::Result;
use crate::parser::{Expression, VduItem};

/// The count of parameter bytes for each control code from 0 to 31, as on a
/// BBC Micro.
const PARAMETER_COUNTS: [usize; 32] = [
    0, 1, 0, 0, 0, 0, 0, 0, // 0 to 7
    0, 0, 0, 0, 0, 0, 0, 0, // 8 to 15
    0, 1, 2, 5, 0, 0, 1, 9, // 16 to 23
    8, 5, 0, 0, 4, 4, 0, 2, // 24 to 31
];

/// A 2-byte parameter, low byte first, as a signed value
fn word(low: u8, high: u8) -> Expression {
    Expression::Integer(i16::from_le_bytes([low, high]) as i32)
}

impl Executor {
    /// Execute VDU statement - send each item as 1 byte, or as 2 bytes (low
    /// byte first) if a semicolon follows it
    pub(super) fn execute_vdu(&mut self, items: &[VduItem]) -> Result<()> {
        for item in items {
            let value = self.eval_integer(&item.value)?;
            self.send_vdu_byte(value as u8)?;
            if item.two_bytes {
                self.send_vdu_byte((value >> 8) as u8)?;
            }
        }
        Ok(())
    }

    /// Add one byte to the queue. If the command is complete, run it.
    fn send_vdu_byte(&mut self, byte: u8) -> Result<()> {
        self.vdu_queue.push(byte);
        let code = self.vdu_queue[0];
        let parameter_count = PARAMETER_COUNTS.get(code as usize).copied().unwrap_or(0);
        if self.vdu_queue.len() <= parameter_count {
            return Ok(());
        }
        // Empty the queue before the command runs, so that an error does not
        // leave a part of the command in it
        let command = std::mem::take(&mut self.vdu_queue);
        self.run_vdu_command(code, &command[1..])
    }

    /// Run one complete VDU command. The codes that have no effect here
    /// only use their parameter bytes.
    fn run_vdu_command(&mut self, code: u8, parameters: &[u8]) -> Result<()> {
        let integer = |index: usize| Expression::Integer(parameters[index] as i32);
        match code {
            8 => {
                self.print_control("\x08");
                self.column = self.column.saturating_sub(1);
            }
            9 => self.column += 1,
            10 => self.print_output("\n"),
            12 => self.execute_cls()?,
            13 => {
                self.print_control("\r");
                self.column = 0;
            }
            16 => self.execute_clg()?,
            17 => self.set_text_colour(parameters[0]),
            18 => self.execute_gcol(&integer(0), &integer(1))?,
            22 => self.execute_mode(&integer(0))?,
            25 => self.execute_plot(
                &integer(0),
                &word(parameters[1], parameters[2]),
                &word(parameters[3], parameters[4]),
            )?,
            29 => self.execute_origin(
                &word(parameters[0], parameters[1]),
                &word(parameters[2], parameters[3]),
            )?,
            30 => self.column = 0,
            31 => self.move_text_cursor(parameters[0], parameters[1]),
            32..=126 => self.print_output(&char::from(code).to_string()),
            // 7 (bell), 127 (delete), the
            // other control codes and the codes from 128 to 255 do nothing
            _ => {}
        }
        Ok(())
    }

    /// Move the text cursor to column `x` and row `y` (VDU 31 and TAB(x, y)).
    /// The executor keeps only a print column, so `y` has no effect. No
    /// cursor code goes to the output buffer or to standard output, as for
    /// colour (issue #101).
    pub(super) fn move_text_cursor(&mut self, x: u8, _y: u8) {
        self.column = x as usize;
    }

    /// Send a control character to the display. It does not change the
    /// print column.
    fn print_control(&mut self, text: &str) {
        self.output.push_str(text);
        #[cfg(not(test))]
        {
            print!("{}", text);
        }
    }
}
