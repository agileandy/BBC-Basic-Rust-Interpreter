10 REM Test LOCAL variables in procedures
20 X = 10
30 Y = 20
40 PRINT "Before PROC: X="; X; " Y="; Y
50 PROC test
60 PRINT "After PROC: X="; X; " Y="; Y
70 END
100 DEF PROC test
110 LOCAL X
120 X = 99
130 Y = 88
140 PRINT "Inside PROC: X="; X; " Y="; Y
150 ENDPROC
