10 REM Test MOD, DIV, and ^ operators
20 REM
30 PRINT "Testing Power Operator (^)"
40 PRINT "========================="
50 PRINT "2 ^ 3 ="; 2 ^ 3; " (expected 8)"
60 PRINT "5 ^ 2 ="; 5 ^ 2; " (expected 25)"
70 PRINT "10 ^ 0 ="; 10 ^ 0; " (expected 1)"
80 PRINT "3 ^ 4 ="; 3 ^ 4; " (expected 81)"
90 PRINT
100 PRINT "Testing Integer Division (DIV)"
110 PRINT "=============================="
120 PRINT "10 DIV 3 ="; 10 DIV 3; " (expected 3)"
130 PRINT "15 DIV 4 ="; 15 DIV 4; " (expected 3)"
140 PRINT "20 DIV 7 ="; 20 DIV 7; " (expected 2)"
150 PRINT "100 DIV 10 ="; 100 DIV 10; " (expected 10)"
160 PRINT
170 PRINT "Testing Modulo (MOD)"
180 PRINT "===================="
190 PRINT "10 MOD 3 ="; 10 MOD 3; " (expected 1)"
200 PRINT "15 MOD 4 ="; 15 MOD 4; " (expected 3)"
210 PRINT "20 MOD 7 ="; 20 MOD 7; " (expected 6)"
220 PRINT "100 MOD 10 ="; 100 MOD 10; " (expected 0)"
230 PRINT
240 PRINT "Testing Combined Operations"
250 PRINT "==========================="
260 REM Calculate (10 MOD 3) + (2 ^ 3) = 1 + 8 = 9
270 PRINT "(10 MOD 3) + (2 ^ 3) ="; (10 MOD 3) + (2 ^ 3); " (expected 9)"
280 REM Calculate 100 DIV (2 ^ 3) = 100 DIV 8 = 12
290 PRINT "100 DIV (2 ^ 3) ="; 100 DIV (2 ^ 3); " (expected 12)"
300 REM Calculate (15 MOD 7) * (3 ^ 2) = 1 * 9 = 9
310 PRINT "(15 MOD 7) * (3 ^ 2) ="; (15 MOD 7) * (3 ^ 2); " (expected 9)"
320 PRINT
330 PRINT "All operator tests complete!"
340 END
