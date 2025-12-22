10 REM Test DEF FN (user-defined functions)
20 DEF FN add(X, Y) = X + Y
30 DEF FN square(N) = N * N
40 DEF FN double(X) = X * 2
50 REM
60 PRINT "FN add(5, 3) = "; FN add(5, 3)
70 PRINT "FN square(7) = "; FN square(7)
80 PRINT "FN double(10) = "; FN double(10)
90 REM
100 REM Test with local scoping
110 X = 100
120 Y = 200
130 PRINT "Before FN: X ="; X; " Y ="; Y
140 PRINT "FN add(5, 3) = "; FN add(5, 3)
150 PRINT "After FN: X ="; X; " Y ="; Y
160 END
