//! Modern Extensions for BBC BASIC
//!
//! This module contains non-standard functions that extend BBC BASIC with
//! modern functionality. These are NOT part of the original BBC BASIC specification
//! but are provided as useful extensions for 2025 development.
//!
//! **Note**: The actual implementations of these functions are in the executor
//! to avoid circular module dependencies. This module serves as documentation
//! of which functions are extensions vs. standard BBC BASIC functions.
//!
//! ### Non-Standard String Functions
//!
//! | Function | Description | Standard BBC BASIC? |
//! |----------|-------------|---------------------|
//! | `UPPER$` | Convert string to uppercase | ❌ No |
//! | `LOWER$` | Convert string to lowercase | ❌ No |
//! | `STRING$` | Repeat a character N times | ❌ No |
//! | `REPORT$` | Get last error message as string | ❌ No |
//!
//! ### Standard BBC BASIC String Functions (for reference)
//!
//! | Function | Description |
//! |----------|-------------|
//! | `LEFT$` | Leftmost N characters |
//! | `RIGHT$` | Rightmost N characters |
//! | `MID$` | Middle substring |
//! | `CHR$` | ASCII code to character |
//! | `ASC` | Character to ASCII code |
//! | `STR$` | Number to string |
//! | `VAL` | String to number |
//! | `LEN` | String length |
//! | `INSTR` | Find substring position |

#[cfg(test)]
mod tests {
    //! Tests for extension functions are in the executor module
    //! where the actual implementations live.
}