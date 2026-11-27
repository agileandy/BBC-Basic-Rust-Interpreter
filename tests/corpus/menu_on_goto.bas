10 REM Menu with INPUT and ON GOTO
20 BAL = 100
30 PRINT "Balance: ";BAL
40 PRINT "1 Deposit  2 Withdraw  3 Quit"
50 INPUT "Choice";C%
60 ON C% GOTO 100,200,300 ELSE PRINT "Bad choice" : GOTO 50
100 INPUT "Amount";A
110 BAL = BAL + A
120 GOTO 30
200 INPUT "Amount";A
210 IF A > BAL THEN PRINT "Not enough money" : GOTO 30
220 BAL = BAL - A
230 GOTO 30
300 PRINT "Final balance: ";BAL
310 END
