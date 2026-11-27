10 REM Draw a shape from arrays of points and count the lit pixels
20 READ N%
30 DIM X%(N%), Y%(N%)
40 FOR I% = 1 TO N%
50   READ PX%, PY%
60   X%(I%) = PX% : Y%(I%) = PY%
70 NEXT I%
80 CLG
90 MOVE X%(N%), Y%(N%)
100 FOR I% = 1 TO N% : DRAW X%(I%), Y%(I%) : NEXT I%
110 PROCcount(400)
120 PROCcount(600)
130 PLOT 4, 900, 100
140 PLOT 4, 1100, 100
150 PLOT 85, 1000, 300
160 IF POINT(1000, 150) THEN PRINT "The triangle is filled"
170 MOVE 0, 900 : PLOT 1, 100, 0
180 IF POINT(100, 900) THEN PRINT "Relative line ends at 100,900"
190 IF POINT(150, 900) = 0 THEN PRINT "and stops there"
200 END
210 DEF PROCcount(Y%)
220 LOCAL X%, LIT%
230 FOR X% = 0 TO 1278 STEP 2
240   IF POINT(X%, Y%) THEN LIT% = LIT% + 1
250 NEXT X%
260 PRINT "Lit pixels on row ";Y%;": ";LIT%
270 ENDPROC
280 DATA 4
290 DATA 200,200,600,200,600,600,200,600
