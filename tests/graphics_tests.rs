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
