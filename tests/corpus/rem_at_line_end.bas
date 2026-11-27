10 REM Lines with several statements and a REM at the end
20 A% = 1 : B% = 2 : REM set the start values
30 C% = A% + B% : PRINT "C% = ";C% : REM print the sum
40 REM a REM hides the rest of the line : PRINT "Not shown"
50 FOR I% = 1 TO 3 : PRINT ;I%; : NEXT I% : PRINT : REM a loop on one line
60 IF C% = 3 THEN PRINT "Three" : REM inside the THEN part
70 IF C% = 4 THEN PRINT "Four" ELSE PRINT "Not four" : REM inside the ELSE part
80 PRINT "Colon in a string: A:B" : REM quotes protect the colon
90 D$ = "REM is text here" : PRINT D$
100 GOSUB 200 : PRINT "Back" : REM return comes here
110 PROCshow("X") : REM a procedure call
120 PRINT "A";:PRINT "B";:PRINT "C":REM no spaces around the colons
130 END : REM the end
200 PRINT "In the subroutine" : RETURN : REM after RETURN
210 DEF PROCshow(P$) : PRINT "Show ";P$ : ENDPROC : REM after ENDPROC
220 REM the last line
