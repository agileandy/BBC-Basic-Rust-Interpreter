//! Tests that ENDPROC closes the FOR, REPEAT and WHILE loops that the
//! procedure opened, and keeps the loops of the caller open (issue #134).

use crate::common::run_with_limit as run;

// D1: ENDPROC closes the loops of the procedure

#[test]
fn test_endproc_closes_open_for_loop_of_the_proc() {
    let (result, output) = run(&[
        "10 FOR J=1 TO 2",
        "20 PROCp",
        "30 PRINT \"J=\";J",
        "40 NEXT",
        "47 END",
        "50 DEF PROCp",
        "60 FOR I=1 TO 3",
        "70 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "J=1\nJ=2\n");
}

#[test]
fn test_endproc_closes_open_repeat_loop_of_the_proc() {
    let (result, output) = run(&[
        "10 N=0",
        "20 REPEAT",
        "30 N=N+1",
        "40 PROCp",
        "50 PRINT \"N=\";N",
        "60 UNTIL N=2",
        "70 END",
        "80 DEF PROCp",
        "90 REPEAT",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "N=1\nN=2\n");
}

#[test]
fn test_endproc_closes_open_while_loop_of_the_proc() {
    let (result, output) = run(&[
        "10 N=0",
        "20 WHILE N<2",
        "30 N=N+1",
        "40 PROCp",
        "50 PRINT \"N=\";N",
        "60 ENDWHILE",
        "70 END",
        "80 DEF PROCp",
        "90 WHILE TRUE",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "N=1\nN=2\n");
}

#[test]
fn test_endproc_in_open_gosub_closes_the_loop_of_the_proc() {
    let (result, output) = run(&[
        "10 FOR J=1 TO 2",
        "20 PROCp",
        "30 PRINT \"J=\";J",
        "40 NEXT",
        "50 END",
        "60 DEF PROCp",
        "70 FOR I=1 TO 3",
        "80 GOSUB 200",
        "90 NEXT",
        "100 ENDPROC",
        "200 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "J=1\nJ=2\n");
}

#[test]
fn test_recursive_proc_closes_the_loop_at_each_level() {
    let (result, output) = run(&[
        "10 FOR J=1 TO 2",
        "20 PROCr(3)",
        "30 PRINT \"J=\";J",
        "40 NEXT",
        "50 END",
        "60 DEF PROCr(D)",
        "70 FOR I=1 TO 5",
        "80 IF D>1 THEN PROCr(D-1)",
        "90 PRINT D;",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123J=1\n123J=2\n");
}

// D2: a loop that the caller opened stays open after ENDPROC

#[test]
fn test_loop_of_the_caller_stays_open_after_endproc() {
    let (result, output) = run(&[
        "10 FOR J=1 TO 3",
        "20 PROCp",
        "30 NEXT",
        "40 PRINT \"end\"",
        "50 END",
        "60 DEF PROCp",
        "70 PRINT J;",
        "80 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "123end\n");
}

#[test]
fn test_closed_loop_of_the_proc_keeps_the_loop_of_the_caller() {
    let (result, output) = run(&[
        "10 FOR J=1 TO 2",
        "20 PROCp",
        "30 NEXT J",
        "40 PRINT \"end\"",
        "50 END",
        "60 DEF PROCp",
        "70 FOR I=1 TO 2",
        "80 PRINT J;I;",
        "90 NEXT I",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "11122122end\n");
}
