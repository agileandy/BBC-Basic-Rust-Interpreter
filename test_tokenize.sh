#!/bin/bash
cat > /tmp/test_tok.rs << 'EEOF'
fn main() {
    let line = bbc_basic_interpreter::tokenizer::tokenize("X% = ABS(-5)").unwrap();
    println!("Tokens: {:?}", line.tokens);
}
EEOF

rustc --edition 2021 -L target/debug/deps /tmp/test_tok.rs -o /tmp/test_tok --extern bbc_basic_interpreter=target/debug/libbbc_basic_interpreter.rlib 2>&1 && /tmp/test_tok
