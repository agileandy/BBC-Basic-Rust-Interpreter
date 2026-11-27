10 REM IF with no THEN, as BBC BASIC permits
20 X% = 5
30 IF X% = 5 PRINT "Five"
40 IF X% = 6 PRINT "Six" ELSE PRINT "Not six"
50 IF X% > 3 X% = X% + 1 : PRINT "Now ";X%
60 IF X% < 3 PRINT "Small" : PRINT "Not shown"
70 FOR I% = 1 TO 4
80   IF I% MOD 2 PRINT I%;" is odd" ELSE PRINT I%;" is even"
90 NEXT I%
100 A$ = "YES"
110 IF A$ = "YES" PROCsay("Agreed")
120 IF A$ <> "YES" PROCsay("Refused")
130 C% = 0
140 REPEAT C% = C% + 1 : IF C% = 3 PRINT "Three reached"
150 UNTIL C% = 4
160 END
170 DEF PROCsay(M$)
180 PRINT M$
190 ENDPROC
