10 REM Write one item in each PRINT#, then read the items with INPUT#
20 F$ = "@TEMP@/stock.txt"
30 C% = OPENOUT F$
40 FOR I% = 1 TO 3
50   READ N$, Q%
60   PRINT#C%, N$
70   PRINT#C%, Q%
80 NEXT I%
90 CLOSE#C%
100 C% = OPENUP F$
110 PTR#C% = EXT#C%
120 PRINT#C%, "Pears"
130 PRINT#C%, 12
140 CLOSE#C%
150 C% = OPENIN F$
160 TOTAL% = 0
170 REPEAT
180   INPUT#C%, N$
190   INPUT#C%, Q%
200   PRINT N$;": ";Q%
210   TOTAL% = TOTAL% + Q%
220 UNTIL EOF#C%
230 CLOSE#C%
240 PRINT "Items in stock: ";TOTAL%
250 DATA Apples,40,Plums,7,Figs,0
