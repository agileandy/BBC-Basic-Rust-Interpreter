10 REM ** Simple Sine Wave Scroller **
20 REM ** Uses PRINT TAB to animate **
30 PRINT "Wave Animation Starting..."
40 PRINT
50 REM
60 REM Animate 50 frames
70 FOR FRAME% = 0 TO 50
80   REM Draw 12 rows of wave
90   FOR ROW% = 0 TO 11
100     REM Calculate wave position
110     PHASE% = FRAME% + ROW% * 4
120     REM Use lookup table for X position
130     GOSUB 1000
140     REM Print the wave character
150     PRINT TAB(X%); "*"
160   NEXT ROW%
170   REM Small delay loop
180   FOR WAIT% = 1 TO 200
190   NEXT WAIT%
200 NEXT FRAME%
210 PRINT
220 PRINT "Wave animation complete!"
230 END
240 REM
1000 REM ** Calculate X position from PHASE% **
1010 REM Cycle through 0-39
1020 X% = PHASE%
1030 FOR T% = 1 TO 20
1040   GOSUB 1100
1050 NEXT T%
1060 REM Map to screen position (10-50)
1070 X% = 10 + X%
1080 RETURN
1090 REM
1100 REM ** Modulo 40 subroutine **
1110 TEMP% = X% - 40
1120 X% = TEMP%
1130 RETURN
