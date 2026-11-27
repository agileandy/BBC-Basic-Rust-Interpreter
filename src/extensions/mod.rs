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
//! | `LOADJSON$(file$)` | Read a JSON file as compact text (`src/json`) | ❌ No |
//! | `JSONVALID(json$)` | TRUE if the text is correct JSON (`src/json`) | ❌ No |
//! | `SAVEJSON file$, json$` | Statement: write JSON text with indentation (`src/json`) | ❌ No |
//! | `JSONGET$(json$, path$)` | The value at a path: a string as text, else JSON (`src/json`) | ❌ No |
//! | `JSONGET(json$, path$)` | The number at a path; TRUE -1, FALSE and null 0 (`src/json`) | ❌ No |
//! | `JSONTYPE$(json$, path$)` | The type name at a path, or `missing` (`src/json`) | ❌ No |
//! | `JSONLEN(json$, path$)` | The length of the array, object or string at a path (`src/json`) | ❌ No |
//! | `JSONKEY$(json$, path$, n)` | The key n (from 0) of the object at a path (`src/json`) | ❌ No |
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
