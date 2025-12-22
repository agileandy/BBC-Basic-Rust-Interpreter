#!/bin/bash
# Demo script for BBC BASIC Interpreter

echo "=== BBC BASIC Interpreter Demo ==="
echo

echo "Building interpreter..."
cargo build --release --quiet
echo "✓ Build complete"
echo

echo "=== Demo 1: Simple Variables ==="
echo -e 'A% = 42\nB = 3.14\nC$ = "HELLO"\nPRINT "Integer:"; A%\nPRINT "Real:"; B\nPRINT "String:"; C$\nEXIT' | ./target/release/bbc-basic-interpreter 2>/dev/null | tail -n +4
echo

echo "=== Demo 2: Expressions ==="
echo -e 'X = 2 + 3 * 4\nPRINT "2 + 3 * 4 ="; X\nY = (2 + 3) * 4\nPRINT "(2 + 3) * 4 ="; Y\nEXIT' | ./target/release/bbc-basic-interpreter 2>/dev/null | tail -n +4
echo

echo "=== Demo 3: FOR Loop ==="
echo -e 'FOR I% = 1 TO 5\nPRINT "Count:"; I%\nNEXT I%\nEXIT' | ./target/release/bbc-basic-interpreter 2>/dev/null | tail -n +4
echo

echo "=== Demo 4: Countdown ==="
echo -e 'FOR I% = 5 TO 1 STEP -1\nPRINT I%\nNEXT I%\nPRINT "Blastoff!"\nEXIT' | ./target/release/bbc-basic-interpreter 2>/dev/null | tail -n +4
echo

echo "=== Demo 5: Arrays ==="
echo -e 'DIM A%(5)\nPRINT "Array A% dimensioned to 5 elements"\nEXIT' | ./target/release/bbc-basic-interpreter 2>/dev/null | tail -n +4
echo

echo "=== All demos complete! ==="
echo
echo "To run interactively, type:"
echo "  cargo run --release"
echo "or"
echo "  ./target/release/bbc-basic-interpreter"
