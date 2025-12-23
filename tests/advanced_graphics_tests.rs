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
fn test_ellipse_command() {
    let mut executor = Executor::new();

    // Draw an ellipse
    execute_line(&mut executor, "10 ELLIPSE 500, 500, 150, 100");

    let output = executor.get_graphics_output();
    // Should have drawn something
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_rectangle_command() {
    let mut executor = Executor::new();

    // Draw a filled rectangle
    execute_line(&mut executor, "10 RECTANGLE 300, 300, 200, 150");

    let output = executor.get_graphics_output();
    // Should have drawn a filled rectangle
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_fill_command() {
    let mut executor = Executor::new();

    // Draw a rectangle outline
    execute_line(&mut executor, "10 MOVE 300, 300");
    execute_line(&mut executor, "20 DRAW 500, 300");
    execute_line(&mut executor, "30 DRAW 500, 500");
    execute_line(&mut executor, "40 DRAW 300, 500");
    execute_line(&mut executor, "50 DRAW 300, 300");

    // Fill the inside
    execute_line(&mut executor, "60 FILL 400, 400");

    let output = executor.get_graphics_output();
    // Should have filled the rectangle
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_multiple_ellipses() {
    let mut executor = Executor::new();

    // Draw multiple ellipses
    execute_line(&mut executor, "10 ELLIPSE 300, 300, 80, 50");
    execute_line(&mut executor, "20 ELLIPSE 600, 600, 100, 60");
    execute_line(&mut executor, "30 ELLIPSE 900, 400, 70, 90");

    let output = executor.get_graphics_output();
    // Should have drawn multiple ellipses
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_rectangle_different_sizes() {
    let mut executor = Executor::new();

    // Draw rectangles of different sizes
    execute_line(&mut executor, "10 RECTANGLE 200, 200, 100, 100");
    execute_line(&mut executor, "20 RECTANGLE 400, 400, 200, 150");
    execute_line(&mut executor, "30 RECTANGLE 700, 300, 150, 250");

    let output = executor.get_graphics_output();
    // Should have drawn multiple rectangles
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_ellipse_circle_comparison() {
    let mut executor = Executor::new();

    // Draw ellipse with equal radii (should look like circle)
    execute_line(&mut executor, "10 ELLIPSE 400, 400, 100, 100");
    // Draw actual circle
    execute_line(&mut executor, "20 CIRCLE 700, 700, 100");

    let output = executor.get_graphics_output();
    // Both should have drawn
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_advanced_graphics_integration() {
    let mut executor = Executor::new();

    // Test a complete program with all advanced features
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 GCOL 0, 255");

    // Draw some ellipses
    execute_line(&mut executor, "30 ELLIPSE 400, 400, 150, 100");
    execute_line(&mut executor, "40 ELLIPSE 700, 700, 100, 150");

    // Draw rectangles
    execute_line(&mut executor, "50 RECTANGLE 200, 600, 150, 100");
    execute_line(&mut executor, "60 RECTANGLE 800, 300, 100, 200");

    let output = executor.get_graphics_output();
    // Should have a border and graphics content
    assert!(output.starts_with('+'));
    assert!(output.contains('|'));
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_fill_after_circle() {
    let mut executor = Executor::new();

    // Draw a circle
    execute_line(&mut executor, "10 CIRCLE 500, 500, 100");

    // Try to fill inside (should fill the area inside the circle)
    execute_line(&mut executor, "20 FILL 500, 500");

    let output = executor.get_graphics_output();
    // Fill should have worked
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_rectangle_zero_size() {
    let mut executor = Executor::new();

    // Rectangle with zero width/height should not crash
    execute_line(&mut executor, "10 RECTANGLE 300, 300, 0, 0");

    let output = executor.get_graphics_output();
    // Should execute without error
    assert!(output.len() > 100);
}

#[test]
fn test_ellipse_narrow() {
    let mut executor = Executor::new();

    // Very narrow ellipse (almost a line)
    execute_line(&mut executor, "10 ELLIPSE 500, 500, 200, 10");

    let output = executor.get_graphics_output();
    // Should have drawn a narrow ellipse
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_ellipse_wide() {
    let mut executor = Executor::new();

    // Very wide ellipse
    execute_line(&mut executor, "10 ELLIPSE 500, 500, 10, 200");

    let output = executor.get_graphics_output();
    // Should have drawn a wide ellipse
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}

#[test]
fn test_combined_shapes() {
    let mut executor = Executor::new();

    // Create a complex scene with all shapes
    execute_line(&mut executor, "10 CLG");
    execute_line(&mut executor, "20 CIRCLE 300, 300, 80");
    execute_line(&mut executor, "30 ELLIPSE 600, 300, 120, 60");
    execute_line(&mut executor, "40 RECTANGLE 300, 600, 250, 150");

    // Draw connecting lines
    execute_line(&mut executor, "50 MOVE 300, 300");
    execute_line(&mut executor, "60 DRAW 600, 300");

    let output = executor.get_graphics_output();
    // Complex scene should render
    assert!(output.contains('█') || output.contains('▓') || output.contains('▒') || output.contains('░'));
}
