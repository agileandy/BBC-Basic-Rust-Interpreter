//! RENUMBER: give the program lines new numbers and correct each jump to a line.

use super::ProgramStore;
use crate::tokenizer::{Token, TokenizedLine};
use std::collections::{BTreeMap, BTreeSet};

impl ProgramStore {
    /// Give the lines the numbers `start`, `start + step`, ... and correct each
    /// line number after GOTO, GOSUB, RESTORE, THEN and ELSE, and in an
    /// ON ... GOTO or ON ... GOSUB list.
    ///
    /// Return the new numbers of the lines that hold a jump to a line that does
    /// not exist. Such a jump stays unchanged. If a step is 0 or a new line
    /// number is more than 65535, change nothing and return an error.
    pub fn renumber(&mut self, start: u16, step: u16) -> Result<Vec<u16>, String> {
        if step == 0 {
            return Err("Silly".to_string());
        }
        let mut new_numbers = BTreeMap::new();
        for (index, old) in self.lines.keys().enumerate() {
            let new = u16::try_from(u64::from(start) + index as u64 * u64::from(step))
                .map_err(|_| "Line number too big".to_string())?;
            new_numbers.insert(*old, new);
        }

        let mut failed = BTreeSet::new();
        let mut lines = BTreeMap::new();
        for (old, line) in &self.lines {
            let new = new_numbers[old];
            let mut tokens = line.tokens.clone();
            for index in line_reference_indexes(&tokens) {
                match line_reference(&tokens[index]).and_then(|n| new_numbers.get(&n)) {
                    Some(target) => tokens[index] = Token::Integer((*target).into()),
                    None => {
                        failed.insert(new);
                    }
                }
            }
            lines.insert(new, TokenizedLine::new(Some(new), tokens));
        }
        self.lines = lines;
        Ok(failed.into_iter().collect())
    }
}

const ELSE: u8 = 0x8B;
const THEN: u8 = 0x8C;
const GOSUB: u8 = 0xE4;
const GOTO: u8 = 0xE5;
const REM: u8 = 0xF4;
const RESTORE: u8 = 0xF7;

/// The line number that a token holds, if it can be one
fn line_reference(token: &Token) -> Option<u16> {
    match token {
        Token::Integer(n) => u16::try_from(*n).ok(),
        Token::LineNumber(n) => Some(*n),
        _ => None,
    }
}

/// True if the token at `index` ends a line reference: the end of the line,
/// a colon, a comma or ELSE. Any other token makes the number part of a calculation.
fn ends_reference(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index),
        None | Some(Token::Separator(':' | ',')) | Some(Token::Keyword(ELSE))
    )
}

/// The indexes of the number tokens that are jump targets. A target is a
/// number directly after GOTO, GOSUB, RESTORE, THEN or ELSE, or a number in
/// the comma list after GOTO or GOSUB. The scan stops at REM.
fn line_reference_indexes(tokens: &[Token]) -> Vec<usize> {
    let mut indexes = Vec::new();
    let mut in_list = false;
    for (index, token) in tokens.iter().enumerate() {
        let is_number = matches!(token, Token::Integer(_) | Token::LineNumber(_));
        let after_keyword = match index.checked_sub(1).map(|i| &tokens[i]) {
            Some(Token::Keyword(GOTO | GOSUB)) => {
                in_list = true;
                true
            }
            Some(Token::Keyword(RESTORE | THEN | ELSE)) => {
                in_list = false;
                true
            }
            Some(Token::Separator(',')) => in_list,
            _ => false,
        };
        if is_number && after_keyword && ends_reference(tokens, index + 1) {
            indexes.push(index);
            in_list = in_list && matches!(tokens.get(index + 1), Some(Token::Separator(',')));
        } else if !matches!(token, Token::Keyword(GOTO | GOSUB) | Token::Separator(',')) {
            in_list = false;
        }
        if matches!(token, Token::Keyword(REM)) {
            break;
        }
    }
    indexes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{detokenize, tokenize};

    fn store(source: &[&str]) -> ProgramStore {
        let mut program = ProgramStore::new();
        for line in source {
            program.store_line(tokenize(line).expect("line tokenizes"));
        }
        program
    }

    fn listing(program: &ProgramStore) -> Vec<String> {
        program
            .list()
            .into_iter()
            .map(|(_, line)| detokenize(line).expect("line lists"))
            .collect()
    }

    #[test]
    fn test_renumber_with_defaults_gives_10_20_30() {
        let mut program = store(&["5 PRINT 1", "7 PRINT 2", "100 PRINT 3"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(program.get_line_numbers(), vec![10, 20, 30]);
        assert_eq!(
            listing(&program),
            ["10 PRINT 1", "20 PRINT 2", "30 PRINT 3"]
        );
    }

    #[test]
    fn test_renumber_with_start_and_step() {
        let mut program = store(&["5 PRINT 1", "7 PRINT 2", "100 PRINT 3"]);

        assert_eq!(program.renumber(1000, 5), Ok(vec![]));

        assert_eq!(program.get_line_numbers(), vec![1000, 1005, 1010]);
        assert_eq!(program.get_line(1005).unwrap().line_number, Some(1005));
    }

    #[test]
    fn test_goto_and_gosub_targets_follow_their_lines() {
        let mut program = store(&["5 GOSUB 100", "7 GOTO 5", "100 RETURN"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(
            listing(&program),
            ["10 GOSUB 30", "20 GOTO 10", "30 RETURN"]
        );
    }

    #[test]
    fn test_restore_target_follows_its_line() {
        let mut program = store(&["5 RESTORE 7", "7 DATA 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 RESTORE 20");
    }

    #[test]
    fn test_then_and_else_targets_follow_their_lines() {
        let mut program = store(&["5 IF X THEN 7 ELSE 9", "7 PRINT 1", "9 PRINT 2"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 IF X THEN 20 ELSE 30");
    }

    #[test]
    fn test_on_goto_and_on_gosub_lists_follow_their_lines() {
        let mut program = store(&[
            "5 ON X GOTO 7, 9 : ON X GOSUB 9,7",
            "7 PRINT 1",
            "9 PRINT 2",
        ]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 ON X GOTO 20,30:ON X GOSUB 30,20");
    }

    #[test]
    fn test_on_error_goto_target_follows_its_line() {
        let mut program = store(&["5 ON ERROR GOTO 7", "7 PRINT 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 ON ERROR GOTO 20");
    }

    #[test]
    fn test_target_after_colon_follows_its_line() {
        let mut program = store(&["5 PRINT 1 : GOTO 7", "7 PRINT 2"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 PRINT 1:GOTO 20");
    }

    #[test]
    fn test_missing_target_stays_and_is_reported_with_new_number() {
        let mut program = store(&["5 GOTO 99", "7 GOSUB 5 : GOTO 98 : GOTO 97", "9 PRINT 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![10, 20]));

        assert_eq!(
            listing(&program)[..2],
            ["10 GOTO 99", "20 GOSUB 10:GOTO 98:GOTO 97"]
        );
    }

    #[test]
    fn test_number_in_a_calculation_is_not_changed() {
        let mut program = store(&["5 GOTO 7 + X", "7 PRINT 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 GOTO 7 + X");
    }

    #[test]
    fn test_number_that_is_not_after_a_jump_keyword_is_not_changed() {
        let mut program = store(&["5 PRINT 7, 5 : X = 7", "7 PRINT 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 PRINT 7,5:X = 7");
    }

    #[test]
    fn test_text_after_rem_is_not_changed() {
        let mut program = store(&["5 REM GOTO 7", "7 PRINT 1"]);

        assert_eq!(program.renumber(10, 10), Ok(vec![]));

        assert_eq!(listing(&program)[0], "10 REM GOTO 7");
    }

    #[test]
    fn test_line_number_too_big_changes_nothing() {
        let mut program = store(&["5 GOTO 7", "7 PRINT 1"]);

        assert!(program.renumber(65530, 10).is_err());

        assert_eq!(listing(&program), ["5 GOTO 7", "7 PRINT 1"]);
    }

    #[test]
    fn test_last_line_number_65535_is_permitted() {
        let mut program = store(&["5 PRINT 1", "7 PRINT 2"]);

        assert_eq!(program.renumber(65525, 10), Ok(vec![]));

        assert_eq!(program.get_line_numbers(), vec![65525, 65535]);
    }

    #[test]
    fn test_step_zero_changes_nothing() {
        let mut program = store(&["5 PRINT 1", "7 PRINT 2"]);

        assert!(program.renumber(10, 0).is_err());

        assert_eq!(program.get_line_numbers(), vec![5, 7]);
    }
}
