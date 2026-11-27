//! Issue #194: a procedure or a function takes a whole structure as a
//! parameter, as in BBC BASIC for Windows: `DEF PROCmove(p{}, dx)` and
//! `PROCmove(point{}, 5)`. The structure is passed by reference.

use crate::common::run_binary_read_first as run_binary;
use crate::common::run_with_limit as run;

#[test]
fn test_a_procedure_changes_a_member_of_its_parameter() {
    let (result, output) = run(&[
        "10 DIM point{x, y%}",
        "20 point.x = 1 : point.y% = 2",
        "30 PROCmove(point{}, 5)",
        "40 PRINT ;point.x;\" \";point.y%",
        "50 END",
        "60 DEF PROCmove(p{}, dx)",
        "70 p.x = p.x + dx : p.y% = p.y% * 10",
        "80 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "6 20\n");
}

#[test]
fn test_a_function_reads_2_structures() {
    let (result, output) = run(&[
        "10 DIM a{x, y}, b{x, y}",
        "20 a.x = 1 : a.y = 2 : b.x = 4 : b.y = 6",
        "30 PRINT ;FNdist(a{}, b{})",
        "40 END",
        "50 DEF FNdist(p{}, q{}) = SQR((q.x - p.x)^2 + (q.y - p.y)^2)",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "5\n");
}

#[test]
fn test_a_multi_line_function_with_a_structure_parameter() {
    let (result, output) = run(&[
        "10 DIM s{n$}",
        "20 s.n$ = \"Ann\"",
        "30 PRINT FNgreet$(s{})",
        "40 END",
        "50 DEF FNgreet$(r{})",
        "60 r.n$ = r.n$ + \"!\"",
        "70 = \"Hi \" + r.n$",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "Hi Ann!\n");
}

#[test]
fn test_a_recursive_procedure_with_a_structure_parameter() {
    let (result, output) = run(&[
        "10 DIM c{total%}",
        "20 PROCadd(c{}, 4)",
        "30 PRINT ;c.total%",
        "40 END",
        "50 DEF PROCadd(s{}, n%)",
        "60 IF n% = 0 THEN ENDPROC",
        "70 s.total% = s.total% + n%",
        "80 PROCadd(s{}, n% - 1)",
        "90 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "10\n");
}

#[test]
fn test_a_routine_passes_its_parameter_to_a_second_routine() {
    let (result, output) = run(&[
        "10 DIM p{x}",
        "20 PROCouter(p{})",
        "30 PRINT ;p.x",
        "40 END",
        "50 DEF PROCouter(a{})",
        "60 PROCinner(a{})",
        "70 ENDPROC",
        "80 DEF PROCinner(b{})",
        "90 b.x = 9",
        "100 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "9\n");
}

#[test]
fn test_a_structure_with_the_parameter_name_is_intact_after_the_call() {
    let (result, output) = run(&[
        "10 DIM p{x}, q{x}",
        "20 p.x = 1 : q.x = 2",
        "30 PROCset(q{})",
        "40 PRINT ;p.x;\" \";q.x",
        "50 END",
        "60 DEF PROCset(p{})",
        "70 p.x = 7",
        "80 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1 7\n");
}

#[test]
fn test_the_parameter_name_is_not_a_structure_after_the_call() {
    let (result, _) = run(&[
        "10 DIM q{x}",
        "20 PROCa(q{})",
        "30 PRINT p.x",
        "40 END",
        "50 DEF PROCa(p{})",
        "60 ENDPROC",
    ]);
    assert_eq!(result, Err("Bad use of structure at line 30".to_string()));
}

#[test]
fn test_a_number_for_a_structure_parameter_is_bad_use_of_structure() {
    let (result, _) = run(&["10 PROCa(5)", "20 END", "30 DEF PROCa(p{})", "40 ENDPROC"]);
    assert_eq!(result, Err("Bad use of structure at line 10".to_string()));
}

#[test]
fn test_a_structure_for_a_number_parameter_is_bad_use_of_structure() {
    let (result, _) = run(&[
        "10 DIM s{x}",
        "20 PROCa(s{})",
        "30 END",
        "40 DEF PROCa(n)",
        "50 ENDPROC",
    ]);
    assert_eq!(result, Err("Bad use of structure at line 20".to_string()));
}

#[test]
fn test_a_structure_with_no_dim_as_an_argument_is_bad_use_of_structure() {
    let (result, _) = run(&["10 PROCa(s{})", "20 END", "30 DEF PROCa(p{})", "40 ENDPROC"]);
    assert_eq!(result, Err("Bad use of structure at line 10".to_string()));
}

#[test]
fn test_a_missing_member_is_an_error_at_the_line_that_uses_it() {
    let (result, _) = run(&[
        "10 DIM s{x}",
        "20 PROCa(s{})",
        "30 END",
        "40 DEF PROCa(p{})",
        "50 PRINT p.y",
        "60 ENDPROC",
    ]);
    assert_eq!(result, Err("No such variable: p.y at line 50".to_string()));
}

#[test]
fn test_a_trapped_error_in_the_routine_removes_the_alias() {
    let (result, output) = run(&[
        "10 DIM p{x}, q{x}",
        "20 p.x = 1",
        "30 ON ERROR PRINT ;p.x : END",
        "40 PROCfail(q{})",
        "50 END",
        "60 DEF PROCfail(p{})",
        "70 p.x = 5",
        "80 PRINT 1/0",
        "90 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "1\n");
}

#[test]
fn test_a_local_structure_is_new_on_each_call_and_goes_after_it() {
    let (result, output) = run(&[
        "10 DIM s{x}",
        "20 s.x = 3",
        "30 PROCa : PROCa",
        "40 PRINT ;s.x",
        "50 END",
        "60 DEF PROCa",
        "70 LOCAL s{}",
        "80 DIM s{x, n%}",
        "90 PRINT ;s.x;\" \";s.n%",
        "100 s.x = 8 : s.n% = 9",
        "110 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0 0\n0 0\n3\n");
}

#[test]
fn test_each_level_of_a_recursion_has_its_own_local_structure() {
    let (result, output) = run(&[
        "10 PROCr(2)",
        "20 END",
        "30 DEF PROCr(n%)",
        "40 LOCAL t{}",
        "50 DIM t{v%}",
        "60 t.v% = n%",
        "70 IF n% > 0 THEN PROCr(n% - 1)",
        "80 PRINT ;t.v%",
        "90 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn test_a_structure_parameter_name_with_a_suffix_is_an_error() {
    let (result, _) = run(&["10 DEF PROCa(p%{})", "20 ENDPROC"]);
    assert_eq!(
        result,
        Err("Syntax error at line 10: A structure name cannot end in % or $".to_string())
    );
}

#[test]
fn test_a_local_structure_passes_to_a_second_routine() {
    let (result, output) = run(&[
        "10 PROCa",
        "20 END",
        "30 DEF PROCa",
        "40 LOCAL t{}",
        "50 DIM t{x}",
        "60 PROCb(t{})",
        "70 PRINT ;t.x",
        "80 ENDPROC",
        "90 DEF PROCb(u{})",
        "100 u.x = 4",
        "110 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4\n");
}

#[test]
fn test_a_local_structure_with_no_dim_is_bad_use_of_structure() {
    let (result, _) = run(&[
        "10 PROCa",
        "20 END",
        "30 DEF PROCa",
        "40 LOCAL t{}",
        "50 PRINT t.x",
        "60 ENDPROC",
    ]);
    assert_eq!(result, Err("Bad use of structure at line 50".to_string()));
}

#[test]
fn test_a_variable_and_an_array_with_the_parameter_name_still_work() {
    let (result, output) = run(&[
        "10 DIM s{x}, p(2)",
        "20 p = 4 : p(1) = 6",
        "30 PROCa(s{}, p)",
        "40 PRINT ;s.x;\" \";p;\" \";p(1)",
        "50 END",
        "60 DEF PROCa(p{}, n)",
        "70 p.x = n",
        "80 ENDPROC",
    ]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "4 4 6\n");
}

#[test]
fn test_a_whole_structure_outside_an_argument_is_still_an_error() {
    let (result, _) = run(&["10 DIM a{x}, b{x}", "20 b{} = a{}"]);
    assert_eq!(
        result,
        Err(
            "Syntax error at line 20: A reference to a whole structure is not supported"
                .to_string()
        )
    );
}

#[test]
fn test_the_real_binary_passes_a_structure() {
    let output = run_binary(
        "10 DIM point{x}\n20 PROCmove(point{}, 5)\n30 PRINT ;point.x\n40 END\n\
         50 DEF PROCmove(p{}, dx)\n60 p.x = p.x + dx\n70 ENDPROC\nRUN\nEXIT\n",
    );
    assert!(output.contains("> 5\n"), "{output}");
}
