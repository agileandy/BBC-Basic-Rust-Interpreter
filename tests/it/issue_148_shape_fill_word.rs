//! Issue 148: RECTANGLE, CIRCLE and ELLIPSE draw an outline. With the word
//! FILL after the keyword, each one draws a filled shape. RECTANGLE with 3
//! values draws a square. FILL x, y alone is still the flood fill.

use bbc_basic_interpreter::parser::{parse_statement, Expression, Statement};
use bbc_basic_interpreter::tokenizer::tokenize;

use crate::common::{run_binary_read_first, run_with_limit};

fn parse(line: &str) -> Statement {
    parse_statement(&tokenize(line).expect("line tokenizes")).expect("line parses")
}

fn int(value: i32) -> Expression {
    Expression::Integer(value)
}

// Parser

#[test]
fn test_rectangle_parses_as_an_outline() {
    assert_eq!(
        parse("RECTANGLE 100,100,200,50"),
        Statement::Rectangle {
            x1: int(100),
            y1: int(100),
            width: int(200),
            height: int(50),
            filled: false,
        }
    );
}

#[test]
fn test_rectangle_fill_parses_as_a_filled_rectangle() {
    assert_eq!(
        parse("RECTANGLE FILL 100,100,200,50"),
        Statement::Rectangle {
            x1: int(100),
            y1: int(100),
            width: int(200),
            height: int(50),
            filled: true,
        }
    );
}

#[test]
fn test_rectangle_with_3_values_parses_as_a_square() {
    assert_eq!(
        parse("RECTANGLE FILL 400,500,200"),
        Statement::Rectangle {
            x1: int(400),
            y1: int(500),
            width: int(200),
            height: int(200),
            filled: true,
        }
    );
}

#[test]
fn test_circle_and_circle_fill_parse() {
    assert_eq!(
        parse("CIRCLE 300,300,50"),
        Statement::Circle {
            x: int(300),
            y: int(300),
            radius: int(50),
            filled: false,
        }
    );
    assert_eq!(
        parse("CIRCLE FILL 300,300,50"),
        Statement::Circle {
            x: int(300),
            y: int(300),
            radius: int(50),
            filled: true,
        }
    );
}

#[test]
fn test_ellipse_and_ellipse_fill_parse() {
    assert_eq!(
        parse("ELLIPSE 300,300,80,40"),
        Statement::Ellipse {
            x: int(300),
            y: int(300),
            major: int(80),
            minor: int(40),
            filled: false,
        }
    );
    assert_eq!(
        parse("ELLIPSE FILL 300,300,80,40"),
        Statement::Ellipse {
            x: int(300),
            y: int(300),
            major: int(80),
            minor: int(40),
            filled: true,
        }
    );
}

#[test]
fn test_fill_alone_still_parses_as_the_flood_fill() {
    assert_eq!(
        parse("FILL 200,200"),
        Statement::Fill {
            x: int(200),
            y: int(200),
        }
    );
}

// Pixels

#[test]
fn test_rectangle_sets_an_edge_pixel_and_not_the_centre() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE 100,100,200,200",
        "20 PRINT POINT(100,100); POINT(300,300); POINT(200,100); POINT(200,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3330\n");
}

#[test]
fn test_rectangle_fill_sets_the_centre() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE FILL 100,100,200,200",
        "20 PRINT POINT(200,200); POINT(100,100); POINT(301,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         330\n");
}

#[test]
fn test_rectangle_with_3_values_draws_a_square() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE 100,100,50",
        "20 PRINT POINT(150,150); POINT(125,125)",
        "30 RECTANGLE FILL 400,500,200",
        "40 PRINT POINT(500,600); POINT(600,700); POINT(500,701)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         30\n         330\n");
}

#[test]
fn test_circle_fill_sets_the_centre() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : CIRCLE 300,300,50",
        "20 PRINT POINT(300,300)",
        "30 CIRCLE FILL 300,300,50",
        "40 PRINT POINT(300,300); POINT(351,300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n         30\n");
}

#[test]
fn test_ellipse_fill_sets_the_centre() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : ELLIPSE 300,300,80,40",
        "20 PRINT POINT(300,300)",
        "30 ELLIPSE FILL 300,300,80,40",
        "40 PRINT POINT(300,300); POINT(300,341)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         0\n         30\n");
}

#[test]
fn test_fill_still_fills_an_area_in_an_outline_rectangle() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE 100,100,200,200",
        "20 FILL 200,200",
        "30 PRINT POINT(200,200); POINT(350,350)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         30\n");
}

#[test]
fn test_each_filled_shape_draws_nothing_in_mode_7() {
    let (result, output) = run_with_limit(&[
        "10 MODE 7",
        "20 RECTANGLE FILL 100,100,200,200 : CIRCLE FILL 600,600,50",
        "30 ELLIPSE FILL 900,300,80,40",
        "40 PRINT POINT(200,200); POINT(600,600); POINT(900,300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         000\n");
}

// The last point. BBCSDL (src/bbexec.c, case TRECT) draws RECTANGLE as
// MOVE x,y : PLOT 9,w,0 : PLOT 9,0,h : PLOT 9,-w,0 : PLOT 9,0,-h, and
// RECTANGLE FILL as MOVE x,y : PLOT 97,w,h.

#[test]
fn test_rectangle_ends_at_the_start_corner() {
    // The last point is 100,100, thus the relative line goes to 100,50
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE 100,100,200,200",
        "20 PLOT 1,0,-50",
        "30 PRINT POINT(100,60)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

#[test]
fn test_rectangle_puts_the_top_left_corner_before_the_last_point() {
    // The triangle has the corners 100,300 and 100,100 and the point 50,200
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE 100,100,200,200",
        "20 PLOT 85,50,200",
        "30 PRINT POINT(80,200)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

#[test]
fn test_rectangle_fill_ends_at_the_far_corner() {
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : RECTANGLE FILL 100,100,200,200",
        "20 PLOT 1,50,0",
        "30 PRINT POINT(340,300)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

#[test]
fn test_circle_fill_ends_at_the_point_on_the_circle() {
    // CIRCLE FILL x,y,r is MOVE x,y : PLOT 153,r,0, thus the last point is 350,300
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : CIRCLE FILL 300,300,50",
        "20 PLOT 1,0,50",
        "30 PRINT POINT(350,340)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

#[test]
fn test_ellipse_fill_ends_at_the_corner_point() {
    // ELLIPSE FILL x,y,a,b is MOVE x,y : PLOT 0,a,0 : PLOT 201,0,b, thus the
    // last point is 380,340
    let (result, output) = run_with_limit(&[
        "10 MODE 1 : ELLIPSE FILL 300,300,80,40",
        "20 PLOT 1,50,0",
        "30 PRINT POINT(420,340)",
    ]);

    assert_eq!(result, Ok(()));
    assert_eq!(output, "         3\n");
}

// Out of scope (D3)

#[test]
fn test_rectangle_to_is_still_a_syntax_error() {
    let (result, _) = run_with_limit(&["10 MODE 1 : RECTANGLE 100,100,200,200 TO 300,300"]);

    assert_eq!(
        result,
        Err("Syntax error at line 10: Unexpected TO after expression".to_string())
    );
}

// The real binary

#[test]
fn test_list_shows_each_form_as_typed() {
    let output = run_binary_read_first(
        "10 RECTANGLE FILL 1,2,3,4\n20 CIRCLE FILL 5,6,7\n30 ELLIPSE FILL 1,2,3,4\n40 RECTANGLE 1,2,3\nLIST\nEXIT\n",
    );

    assert!(output.contains("10 RECTANGLE FILL 1,2,3,4\n"), "{output}");
    assert!(output.contains("20 CIRCLE FILL 5,6,7\n"), "{output}");
    assert!(output.contains("30 ELLIPSE FILL 1,2,3,4\n"), "{output}");
    assert!(output.contains("40 RECTANGLE 1,2,3\n"), "{output}");
}

#[test]
fn test_the_binary_runs_each_filled_shape() {
    let output = run_binary_read_first(
        "10 MODE 1 : RECTANGLE FILL 100,100,200,200 : CIRCLE FILL 600,600,50\n20 ELLIPSE FILL 900,300,80,40 : RECTANGLE 100,600,100,100\n30 PRINT POINT(200,200); POINT(600,600); POINT(900,300); POINT(150,650)\nRUN\nEXIT\n",
    );

    assert!(output.contains("3330\n"), "{output}");
}
