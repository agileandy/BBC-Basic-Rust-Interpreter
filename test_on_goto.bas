10 REM Test ON GOTO and ON GOSUB
20 REM Menu system demonstration
30 REM
40 PRINT "ON GOTO Test"
50 PRINT "-------------"
60 REM Test ON GOTO with different values
70 FOR I% = 1 TO 4
80   PRINT "Choice "; I%; ": ";
90   ON I% GOTO 120, 140, 160, 180
100  PRINT "Out of range"
110  GOTO 190
120  PRINT "Option A"
130  GOTO 190
140  PRINT "Option B"
150  GOTO 190
160  PRINT "Option C"
170  GOTO 190
180  PRINT "Option D"
190  NEXT I%
200 REM
210 PRINT ""
220 PRINT "ON GOSUB Test"
230 PRINT "-------------"
240 REM Test ON GOSUB
250 FOR J% = 1 TO 3
260   PRINT "Calling subroutine "; J%; ": ";
270   ON J% GOSUB 320, 350, 380
280 NEXT J%
290 REM
300 PRINT "All tests complete"
310 END
320 REM Subroutine 1
330 PRINT "Sub 1 executed"
340 RETURN
350 REM Subroutine 2
360 PRINT "Sub 2 executed"
370 RETURN
380 REM Subroutine 3
390 PRINT "Sub 3 executed"
400 RETURN
