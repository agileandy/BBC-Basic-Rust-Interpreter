10 REM LOCAL variables and LOCAL arrays in procedures and functions
20 DIM V(5)
25 DIM T(3) : T(1) = 1
30 FOR I% = 1 TO 5 : V(I%) = I% * 10 : NEXT I%
40 X = 99 : I% = 7
50 PRINT "Mean: ";FNmean(5)
60 PRINT "X is still ";X;" and I% is still ";I%
70 PROCreverse(5)
80 FOR I% = 1 TO 5 : PRINT " ";V(I%); : NEXT I%
90 PRINT
100 PROClocalarray
110 PRINT "Global T(1) is ";T(1)
120 END
130 DEF FNmean(N%)
140 LOCAL I%, X
150 X = 0
160 FOR I% = 1 TO N% : X = X + V(I%) : NEXT I%
170 = X / N%
180 DEF PROCreverse(N%)
190 LOCAL I%, T
200 FOR I% = 1 TO N% DIV 2
210   T = V(I%) : V(I%) = V(N% + 1 - I%) : V(N% + 1 - I%) = T
220 NEXT I%
230 ENDPROC
240 DEF PROClocalarray
250 LOCAL T()
260 DIM T(3)
270 T(1) = 555
280 PRINT "Local T(1) is ";T(1)
290 ENDPROC
