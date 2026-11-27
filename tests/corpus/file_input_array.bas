10 REM Save an array to a file and load it into a second array
20 F$ = "@TEMP@/table.txt"
30 DIM A(5), B(5)
40 FOR I% = 1 TO 5 : A(I%) = I% * I% : NEXT I%
50 C% = OPENOUT F$
60 PRINT#C%, 5
70 FOR I% = 1 TO 5 : PRINT#C%, A(I%) : NEXT I%
80 CLOSE#C%
90 C% = OPENIN F$
100 INPUT#C%, N%
110 FOR I% = 1 TO N%
120   INPUT#C%, B(I%)
130 NEXT I%
140 CLOSE#C%
150 REM A table of index and value, each in a field of 10
160 FOR I% = 1 TO N% : PRINT I%, B(I%) : NEXT I%
