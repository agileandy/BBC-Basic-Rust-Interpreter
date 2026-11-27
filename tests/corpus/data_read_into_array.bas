10 REM Read a times table straight into array elements
20 DIM T%(3,3), W$(3), P(4)
30 FOR R% = 1 TO 3
40   FOR C% = 1 TO 3
50     READ T%(R%,C%)
60   NEXT C%
70 NEXT R%
80 FOR I% = 1 TO 3 : READ W$(I%) : NEXT I%
90 FOR R% = 1 TO 3
100   PRINT W$(R%);":";
110   FOR C% = 1 TO 3 : PRINT " ";T%(R%,C%); : NEXT C%
120   PRINT
130 NEXT R%
140 READ P(1), P(2), P(3), P(4)
150 PRINT "Prices: ";P(1) + P(2) + P(3) + P(4)
160 DATA 1,2,3,2,4,6,3,6,9
170 DATA ONE,TWO,THREE
180 DATA 1.5,2.25,0.25,6
