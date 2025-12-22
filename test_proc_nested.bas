10 REM Test nested PROC calls
20 X = 1
30 PROC outer
40 PRINT "Back in main, X ="; X
50 END
100 DEF PROC outer
110 X = 10
120 PRINT "In outer, X ="; X
130 PROC inner
140 PRINT "Back in outer, X ="; X
150 ENDPROC
200 DEF PROC inner
210 X = 100
220 PRINT "In inner, X ="; X
230 ENDPROC
