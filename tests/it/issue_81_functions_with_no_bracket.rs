//! Issue #81: RND, COUNT, POS and VPOS give a value with no bracket.

use crate::common::run_lines as run;

#[test]
fn test_rnd_without_bracket_gives_different_integers() {
    // 5 pairs: at least 1 pair must be different
    let (result, output) = run(&[
        "10 FOR I% = 1 TO 5",
        "20 X% = RND : Y% = RND",
        "30 PRINT X% = Y%",
        "40 NEXT I%",
    ]);

    assert_eq!(result, Ok(()));
    assert!(output.contains("0\n"), "all 5 pairs were equal: {output:?}");
}

#[test]
fn test_rnd_without_bracket_is_an_integer() {
    // A real value is truncated by INT, so INT(RND) = RND only for an integer
    let (result, output) = run(&["10 X = RND", "20 PRINT X = INT(X)"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_rnd_1_stays_below_1() {
    let (result, output) = run(&["10 PRINT RND(1) < 1"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "        -1\n");
}

#[test]
fn test_pos_gives_print_column() {
    let (result, output) = run(&["10 PRINT \"AB\";POS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "AB2\n");
}

#[test]
fn test_count_gives_characters_since_new_line() {
    let (result, output) = run(&["10 PRINT \"ABC\";COUNT"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "ABC3\n");
}

#[test]
fn test_vpos_gives_0() {
    let (result, output) = run(&["10 PRINT VPOS"]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n");
}

#[test]
fn test_variables_that_start_with_the_words_keep_behaviour() {
    let (result, output) = run(&[
        "10 RNDSEED = 7",
        "20 COUNTER% = 3",
        "30 POSX = 4",
        "40 VPOSY% = 5",
        "50 PRINT RNDSEED;COUNTER%;POSX;VPOSY%",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         7345\n");
}
