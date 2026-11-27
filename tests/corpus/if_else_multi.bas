10 REM IF with several statements and ELSE on one line
15 P% = 0 : F% = 0
20 FOR I% = 1 TO 6
30   READ M%
40   IF M% >= 70 THEN G$ = "A" : P% = P% + 1 ELSE IF M% >= 50 THEN G$ = "B" : P% = P% + 1 ELSE G$ = "F" : F% = F% + 1
50   PRINT M%;" ";G$
60 NEXT I%
70 PRINT "Passes: ";P%;" Fails: ";F%
80 IF P% > F% THEN PRINT "More passes" ELSE PRINT "More fails"
90 X% = 5
100 IF X% > 3 THEN PRINT "Big"; : PRINT " number" ELSE PRINT "Small"
120 IF X% < 3 THEN PRINT "Not shown" : PRINT "Also not shown"
130 PRINT "Next line runs"
140 IF X% THEN PRINT "Non-zero is true"
150 IF NOT (X% = 4) THEN PRINT "NOT works"
160 IF X% > 1 AND X% < 9 THEN PRINT "In range"
170 IF X% = 5 THEN 190
180 PRINT "Skipped"
190 PRINT "Jumped with THEN line number"
200 DATA 82,45,50,69,70,12
