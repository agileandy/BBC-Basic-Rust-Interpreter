// Issue #113: a missing file gives ERR 214, the BBC BASIC number for
// "File not found".

use crate::common::run_with_limit;

const MISSING: &str = "/no/such/dir/issue_113_missing.json";

#[test]
fn test_on_error_gives_err_214_for_loadjson_of_missing_file() {
    let load = format!("20 A$ = LOADJSON$(\"{MISSING}\")");
    let (result, output) =
        run_with_limit(&["10 ON ERROR GOTO 100", &load, "30 END", "100 PRINT ERR"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, "       214\n");
}

#[test]
fn test_missing_file_message_text_does_not_change() {
    let load = format!("20 A$ = LOADJSON$(\"{MISSING}\")");
    let (result, output) =
        run_with_limit(&["10 ON ERROR GOTO 100", &load, "30 END", "100 PRINT REPORT$"]);
    assert_eq!(result, Ok(()));
    assert_eq!(output, format!("File not found: {MISSING}\n"));
}

#[test]
fn test_missing_file_with_no_handler_stops_with_message() {
    let load = format!("20 A$ = LOADJSON$(\"{MISSING}\")");
    let (result, _) = run_with_limit(&["10 PRINT 1", &load]);
    assert_eq!(result, Err(format!("File not found: {MISSING} at line 20")));
}
