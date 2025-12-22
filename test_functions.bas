10 REM Test all mathematical and string functions
20 REM Mathematical Functions
30 PRINT "=== Mathematical Functions ==="
40 PRINT "SIN(90) = "; SIN(90)
50 PRINT "COS(0) = "; COS(0)
60 PRINT "TAN(45) = "; TAN(45)
70 PRINT "ATN(1) = "; ATN(1)
80 PRINT "SQR(16) = "; SQR(16)
90 PRINT "ABS(-42) = "; ABS(-42)
100 PRINT "INT(3.7) = "; INT(3.7)
110 PRINT "SGN(-5) = "; SGN(-5)
120 PRINT "EXP(1) = "; EXP(1)
130 PRINT "LN(2.718282) = "; LN(2.718282)
140 PRINT "LOG(100) = "; LOG(100)
150 PRINT "PI = "; PI
160 PRINT ""
170 REM String Functions
180 PRINT "=== String Functions ==="
190 A$ = "Hello World"
200 PRINT "Original: "; A$
210 PRINT "LEN(A$) = "; LEN(A$)
220 PRINT "LEFT$(A$, 5) = "; LEFT$(A$, 5)
230 PRINT "RIGHT$(A$, 5) = "; RIGHT$(A$, 5)
240 PRINT "MID$(A$, 7, 5) = "; MID$(A$, 7, 5)
250 PRINT "ASC(""A"") = "; ASC("A")
260 PRINT "CHR$(65) = "; CHR$(65)
270 PRINT "STR$(42) = "; STR$(42)
280 PRINT "VAL(""3.14"") = "; VAL("3.14")
290 PRINT ""
300 REM Combined example
310 PRINT "=== Combined Example ==="
320 X = 30
330 Y = SIN(X) * 10
340 PRINT "X = "; X; ", Y = SIN(X) * 10 = "; Y
350 S$ = "Result: " + STR$(INT(Y))
360 PRINT S$
370 END
