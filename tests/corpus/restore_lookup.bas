10 REM Look up values in DATA with RESTORE
20 DIM Q%(3)
30 RESTORE 300
40 FOR I% = 1 TO 3 : READ M% : Q%(I%) = M% : NEXT I%
50 FOR I% = 1 TO 3
60   PROCdays(Q%(I%))
70 NEXT I%
80 END
90 DEF PROCdays(M%)
100 LOCAL J%, N$, D%
110 RESTORE
120 FOR J% = 1 TO M%
130   READ N$, D%
140 NEXT J%
150 PRINT N$;" has ";D%;" days"
160 ENDPROC
200 DATA January,31,February,28,March,31,April,30,May,31,June,30
210 DATA July,31,August,31,September,30,October,31,November,30,December,31
300 DATA 2,9,12
