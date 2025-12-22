REM Test WHILE...ENDWHILE loops
REM This test demonstrates the WHILE loop functionality

10 REM Test 1: Basic WHILE loop counting from 1 to 5
20 PRINT "Test 1: Basic WHILE loop"
30 X% = 1
40 WHILE X% <= 5
50 PRINT "X% = "; X%
60 X% = X% + 1
70 ENDWHILE
80 PRINT "After loop: X% = "; X%
90 PRINT

100 REM Test 2: WHILE with false condition (should skip loop)
110 PRINT "Test 2: WHILE with false condition"
120 Y% = 10
130 PRINT "Before loop: Y% = "; Y%
140 WHILE Y% < 5
150 PRINT "This should not print"
160 Y% = Y% + 1
170 ENDWHILE
180 PRINT "After loop: Y% = "; Y%
190 PRINT

200 REM Test 3: WHILE loop with condition that becomes false
210 PRINT "Test 3: WHILE countdown"
220 C% = 5
230 WHILE C% > 0
240 PRINT "Countdown: "; C%
250 C% = C% - 1
260 ENDWHILE
270 PRINT "Liftoff!"
280 PRINT

300 REM Test 4: Nested WHILE loops
310 PRINT "Test 4: Nested WHILE loops"
320 I% = 1
330 WHILE I% <= 3
340 PRINT "Outer loop: I% = "; I%
350 J% = 1
360 WHILE J% <= 2
370 PRINT "  Inner loop: J% = "; J%
380 J% = J% + 1
390 ENDWHILE
400 I% = I% + 1
410 ENDWHILE
420 PRINT

500 REM Test 5: WHILE loop with string concatenation
510 PRINT "Test 5: String building with WHILE"
520 N% = 0
530 S$ = ""
540 WHILE N% < 5
550 S$ = S$ + STR$(N%) + " "
560 N% = N% + 1
570 ENDWHILE
580 PRINT "Result: "; S$
590 PRINT

600 REM Test 6: WHILE with complex condition
610 PRINT "Test 6: Complex condition"
620 A% = 0
630 B% = 10
640 WHILE A% < 5 AND B% > 5
650 PRINT "A% = "; A%; ", B% = "; B%
660 A% = A% + 1
670 B% = B% - 1
680 ENDWHILE
690 PRINT "Final: A% = "; A%; ", B% = "; B%
700 PRINT

800 PRINT "All WHILE loop tests completed!"
810 END
