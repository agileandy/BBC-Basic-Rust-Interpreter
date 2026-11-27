10 REM A table of subroutines chosen with ON GOSUB
15 CALLS% = 0
20 FOR I% = 1 TO 4
30   READ OP%, A, B
40   ON OP% GOSUB 200, 300, 400, 500
50   PRINT A;" ";S$;" ";B;" = ";R
60 NEXT I%
70 GOSUB 600
80 PRINT "Calls: ";CALLS%
90 END
200 S$ = "+" : R = A + B : GOSUB 700 : RETURN
300 S$ = "-" : R = A - B : GOSUB 700 : RETURN
400 S$ = "*" : R = A * B : GOSUB 700 : RETURN
500 S$ = "/" : R = A / B : GOSUB 700 : RETURN
600 PRINT "Done"
610 RETURN
700 CALLS% = CALLS% + 1
710 RETURN
800 DATA 1,7,5,2,7,5,3,7,5,4,7,5
