1 REM Test error handling with ON ERROR
2 REM This program demonstrates ON ERROR GOTO, ERL, and ERR

10 PRINT "Testing error handling"
20 ON ERROR GOTO 1000
30 PRINT "Error handler installed"
40 PRINT ""

45 REM Test 1: Division by zero
50 PRINT "Test 1: Division by zero"
60 X = 1 / 0
70 PRINT "This should not print"

90 REM Test 2: Type mismatch
100 PRINT ""
110 PRINT "Test 2: Continuing after error"
120 Y = 42
130 PRINT "Y ="; Y
140 END

990 REM Error handler
1000 PRINT ""
1010 PRINT "Error caught!"
1020 PRINT "Error number: "; ERR
1030 PRINT "Error line: "; ERL
1040 PRINT ""
1050 IF ERL = 60 THEN GOTO 100
1060 END
