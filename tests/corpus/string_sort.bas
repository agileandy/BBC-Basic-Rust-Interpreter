10 REM Insertion sort of names
20 READ N%
30 DIM N$(N%)
40 FOR I% = 1 TO N%
50   READ W$
60   N$(I%) = W$
70 NEXT I%
80 FOR I% = 2 TO N%
90   K$ = N$(I%)
100   J% = I%
110   DONE% = FALSE
120   REPEAT
130     IF J% = 1 THEN DONE% = TRUE ELSE IF N$(J% - 1) <= K$ THEN DONE% = TRUE ELSE N$(J%) = N$(J% - 1) : J% = J% - 1
140   UNTIL DONE%
150   N$(J%) = K$
160 NEXT I%
170 FOR I% = 1 TO N%
180   PRINT ;I%;". ";N$(I%)
190 NEXT I%
200 PRINT "First: ";N$(1);"  Last: ";N$(N%)
210 DATA 6
220 DATA Smith,Jones,Brown,Adams,Taylor,Baker
