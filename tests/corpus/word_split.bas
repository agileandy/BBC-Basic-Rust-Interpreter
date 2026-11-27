10 REM Split a sentence into words with INSTR
20 S$ = "BBC BASIC WAS MADE IN 1981"
30 DIM W$(10)
40 N% = 0
50 P% = 1
60 REPEAT
70   Q% = INSTR(S$ + " ", " ", P%)
80   N% = N% + 1
90   W$(N%) = MID$(S$, P%, Q% - P%)
100   P% = Q% + 1
110 UNTIL P% > LEN S$
120 PRINT "Words: ";N%
130 LONG% = 1
140 FOR I% = 1 TO N%
150   PRINT I%;" ";W$(I%);" (";LEN W$(I%);")"
160   IF LEN W$(I%) > LEN W$(LONG%) THEN LONG% = I%
170 NEXT I%
180 PRINT "Longest: ";W$(LONG%)
190 PRINT "Initials: ";
200 FOR I% = 1 TO N% : PRINT LEFT$(W$(I%),1); : NEXT
210 PRINT
