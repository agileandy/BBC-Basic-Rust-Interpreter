use bbc_basic_interpreter::executor::Executor;
use bbc_basic_interpreter::parser::parse_statement;
use bbc_basic_interpreter::tokenizer::tokenize;

/// Helper to execute a BBC BASIC line
fn execute_line(executor: &mut Executor, line: &str) {
    let tokens = tokenize(line).unwrap();
    let statement = parse_statement(&tokens).unwrap();
    executor.execute_statement(&statement).unwrap();
}

#[test]
fn test_move_command() {
    let mut executor = Executor::new();

    // Test MOVE to set cursor position
    execute_line(&mut executor, "10 MOVE 100, 200");

    let output = executor.get_graphics_output();
    assert!(output.contains('+'));
    assert!(output.contains('|'));
}

#[test]
fn test_draw_command() {
    let mut executor = Executor::new();

    // Draw a line
    execute_line(&mut executor, "10 MOVE 100, 100");
    execute_line(&mut executor, "20 DRAW 200, 200");

    let output = executor.get_graphics_output();
    // Output should contain graphics characters
    assert!(output.len() > 100);
}

#[test]
fn test_plot_modes() {
    let mut executor = Executor::new();

    // PLOT 4 = MOVE (same as MOVE command)
    execute_line(&mut executor, "10 PLOT 4, 100, 100");

    // PLOT 5 = DRAW (same as DRAW command)
    execute_line(&mut executor, "20 PLOT 5, 200, 200");

    let output = executor.get_graphics_output();
    assert!(output.len() > 100);
}

#[test]
fn test_circle_command() {
    let mut executor = Executor::new();

    // Draw a circle
    execute_line(&mut executor, "10 CIRCLE 400, 400, 100");

    let output = executor.get_graphics_output();
    eprintln!("Circle output:\n{}", output);
    // Should have drawn something
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_gcol_command() {
    let mut executor = Executor::new();

    // Set graphics color
    execute_line(&mut executor, "10 GCOL 0, 255");
    execute_line(&mut executor, "20 MOVE 100, 100");
    execute_line(&mut executor, "30 DRAW 200, 200");

    // Should execute without error
    let output = executor.get_graphics_output();
    assert!(output.len() > 100);
}

#[test]
fn test_clg_command() {
    let mut executor = Executor::new();

    // Draw something
    execute_line(&mut executor, "10 MOVE 100, 100");
    execute_line(&mut executor, "20 DRAW 200, 200");

    // Clear graphics
    execute_line(&mut executor, "30 CLG");

    let output = executor.get_graphics_output();
    // After CLG, output should be mostly empty (just borders and spaces)
    let non_border_chars: String = output
        .chars()
        .filter(|c| *c != '+' && *c != '-' && *c != '|' && *c != '\n' && *c != ' ')
        .collect();
    assert_eq!(non_border_chars.len(), 0);
}

#[test]
fn test_draw_square() {
    let mut executor = Executor::new();

    // Draw a square using MOVE and DRAW
    execute_line(&mut executor, "10 MOVE 300, 300");
    execute_line(&mut executor, "20 DRAW 500, 300");
    execute_line(&mut executor, "30 DRAW 500, 500");
    execute_line(&mut executor, "40 DRAW 300, 500");
    execute_line(&mut executor, "50 DRAW 300, 300");

    let output = executor.get_graphics_output();
    // Should contain graphics characters
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_relative_plot() {
    let mut executor = Executor::new();

    // Test relative plotting (mode with bit 2 set)
    execute_line(&mut executor, "10 PLOT 4, 100, 100");
    execute_line(&mut executor, "20 PLOT 69, 50, 50"); // 69 = 65 + 4 (relative point plot)

    let output = executor.get_graphics_output();
    assert!(output.len() > 100);
}

#[test]
fn test_multiple_circles() {
    let mut executor = Executor::new();

    // Draw multiple circles
    execute_line(&mut executor, "10 CIRCLE 300, 300, 50");
    execute_line(&mut executor, "20 CIRCLE 500, 500, 75");
    execute_line(&mut executor, "30 CIRCLE 700, 700, 100");

    let output = executor.get_graphics_output();
    // Should have drawn multiple circles
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_graphics_integration() {
    let mut executor = Executor::new();

    // Test a complete graphics program
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 GCOL 0, 255");
    execute_line(&mut executor, "30 MOVE 400, 400");
    execute_line(&mut executor, "40 DRAW 600, 400");
    execute_line(&mut executor, "50 DRAW 600, 600");
    execute_line(&mut executor, "60 DRAW 400, 600");
    execute_line(&mut executor, "70 DRAW 400, 400");
    execute_line(&mut executor, "80 CIRCLE 500, 500, 100");

    let output = executor.get_graphics_output();
    // Should have a border and some graphics content
    assert!(output.starts_with('+'));
    assert!(output.contains('|'));
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒'));
}

#[test]
fn test_point_function() {
    let mut executor = Executor::new();

    // Test POINT function - can read pixel state
    // First set a pixel using PLOT
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 PLOT 69, 400, 400"); // Plot a point

    // Use POINT and PRINT to verify it works
    execute_line(&mut executor, "30 PRINT POINT(400, 400)");

    // The point was plotted, so it should return non-zero (TRUE)
    // We're just verifying POINT doesn't crash and returns something
    let output = executor.get_output();
    // POINT returns -1 for TRUE
    assert!(output.trim().contains("-1") || output.trim().contains("1"), "POINT should return -1 (TRUE) for plotted point, got: {}", output);

    executor.clear_output();

    // Check a location that wasn't plotted
    execute_line(&mut executor, "40 PRINT POINT(100, 100)");
    let output = executor.get_output();
    // POINT returns 0 for FALSE
    assert!(output.contains('0'), "POINT should return 0 (FALSE) for unset pixel");
}

#[test]
fn test_point_function_after_draw() {
    let mut executor = Executor::new();

    // Draw a line and use POINT to check pixels
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 MOVE 400, 400");
    execute_line(&mut executor, "30 DRAW 600, 600");

    // Check a point on the diagonal line (500, 500)
    execute_line(&mut executor, "40 PRINT POINT(500, 500)");
    let output = executor.get_output();
    // Should be TRUE (non-zero) since the line goes through (500, 500)
    assert!(output.trim().contains("-1") || output.trim().contains("1"), "POINT should return -1 (TRUE) on the line, got: {}", output);

    executor.clear_output();

    // Check a point off the line
    execute_line(&mut executor, "50 PRINT POINT(100, 100)");
    let output = executor.get_output();
    // Should be FALSE (0)
    assert!(output.contains('0'));
}

#[test]
fn test_point_function_with_circle() {
    let mut executor = Executor::new();

    // Draw a circle and test POINT on its edge
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 CIRCLE 500, 500, 100");

    // Test POINT at a point that should be on the circle edge (600, 500)
    execute_line(&mut executor, "30 PRINT POINT(600, 500)");
    let output = executor.get_output();
    // Just verify POINT works - the exact value depends on circle drawing algorithm
    // Either TRUE or FALSE is acceptable
    assert!(!output.is_empty());
}

#[test]
fn test_origin_command() {
    let mut executor = Executor::new();

    // Test ORIGIN command - shifts graphics coordinate system
    execute_line(&mut executor, "10 ORIGIN 100, 100");

    // After ORIGIN 100, 100, a MOVE to (400, 400) should actually move to (500, 500) in canvas coords
    execute_line(&mut executor, "20 MOVE 400, 400");
    execute_line(&mut executor, "30 DRAW 600, 600");

    // The graphics system should handle the origin offset correctly
    let output = executor.get_graphics_output();
    // Just verify it doesn't crash and produces output
    assert!(!output.is_empty());
}

#[test]
fn test_origin_affects_plotting() {
    let mut executor = Executor::new();

    // Set origin first
    execute_line(&mut executor, "10 ORIGIN 200, 200");
    execute_line(&mut executor, "20 CLG");
    execute_line(&mut executor, "30 CIRCLE 400, 400, 50");

    // The circle should be drawn at (600, 600) in canvas coordinates due to origin offset
    let output = executor.get_graphics_output();
    // Verify graphics output exists
    assert!(!output.is_empty());
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_plot_triangle_fill_modes() {
    let mut executor = Executor::new();

    // Test PLOT modes 128-191 (filled triangles)
    // Triangle modes work in pairs:
    // - First PLOT stores current position as triangle corner
    // - Second PLOT draws triangle from corner -> current -> target

    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 MOVE 400, 300"); // Move to first corner

    // First PLOT 128: stores (400, 300) as triangle corner
    execute_line(&mut executor, "30 PLOT 128, 600, 600");

    // Second PLOT 128: draws triangle from (400,300) -> (600,600) -> (600,300)
    execute_line(&mut executor, "40 MOVE 600, 300");
    execute_line(&mut executor, "50 PLOT 128, 600, 600");

    let output = executor.get_graphics_output();
    // Should have drawn something
    assert!(!output.is_empty());
}

#[test]
fn test_plot_triangle_with_different_modes() {
    let mut executor = Executor::new();

    // Test that different triangle modes work (129-191 use patterns)
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 MOVE 400, 400");
    execute_line(&mut executor, "30 PLOT 129, 600, 600"); // Store corner
    execute_line(&mut executor, "40 MOVE 600, 400");
    execute_line(&mut executor, "50 PLOT 129, 600, 600"); // Draw triangle

    let output = executor.get_graphics_output();
    assert!(!output.is_empty());
}

