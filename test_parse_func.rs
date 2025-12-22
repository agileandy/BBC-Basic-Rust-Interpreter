use bbc_basic_interpreter::tokenizer::tokenize;
use bbc_basic_interpreter::parser::parse_statement;

fn main() {
    let line = tokenize("X% = ABS(-5)").unwrap();
    println!("Tokens: {:?}", line);
    
    let stmt = parse_statement(&line);
    println!("Statement: {:?}", stmt);
}
