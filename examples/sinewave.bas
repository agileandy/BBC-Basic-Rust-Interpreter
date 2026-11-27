10 REM ** Simple Sine Wave Scroller **
20 REM ** Uses PRINT TAB to animate **
30 PRINT "Wave Animation Starting..."
40 PRINT
50 REM
60 REM Animate 12 frames
70 FOR FRAME% = 0 TO 11
80   REM Draw 12 rows of wave
90   FOR ROW% = 0 TO 11
100     REM Calculate wave position
110     PHASE% = FRAME% + ROW% * 4
120     REM Calculate X position from a sine
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
1010 REM Map the sine to screen columns 5-34
1020 X% = 20 + INT(15 * SIN(PHASE% / 4))
1030 RETURN
