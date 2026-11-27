10 REM Write a score file with PRINT# and read it with INPUT#
20 F$ = "@TEMP@/scores.txt"
30 C% = OPENOUT F$
40 FOR I% = 1 TO 4
50   READ N$, S%
60   PRINT#C%, N$, S%
70 NEXT I%
80 CLOSE#C%
90 C% = OPENUP F$
100 PTR#C% = EXT#C%
110 PRINT#C%, "Eve", 20
120 CLOSE#C%
130 C% = OPENIN F$
140 TOTAL% = 0
150 COUNT% = 0
160 REPEAT
170   INPUT#C%, N$, S%
180   PRINT N$;" scored ";S%
190   TOTAL% = TOTAL% + S%
200   COUNT% = COUNT% + 1
210 UNTIL EOF#C%
220 CLOSE#C%
230 PRINT "Players: ";COUNT%;"  Mean: ";TOTAL% / COUNT%
240 DATA Ann,17,Bob,23,Cat,9,Dan,31
