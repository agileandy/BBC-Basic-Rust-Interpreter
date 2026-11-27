10 REM Recursive functions
12 REM A table of N and N!, each at the right of a field of 10
15 PRINT TAB(9);"N";TAB(18);"N!"
20 FOR N% = 0 TO 6
30   PRINT N%, FNfact(N%)
40 NEXT N%
50 PRINT "Fibonacci:";
60 FOR N% = 1 TO 10 : PRINT " ";FNfib(N%); : NEXT N%
70 PRINT
80 PRINT "GCD of 84 and 36 is ";FNgcd(84,36)
90 PRINT "2 to the 10 is ";FNpower(2,10)
100 PRINT "Digits of 90125: ";FNdigits(90125)
110 END
120 DEF FNfact(N%)
130 IF N% <= 1 THEN =1
140 =N% * FNfact(N% - 1)
150 DEF FNfib(N%)
160 IF N% < 3 THEN =1
170 =FNfib(N% - 1) + FNfib(N% - 2)
180 DEF FNgcd(A%, B%)
190 IF B% = 0 THEN =A%
200 =FNgcd(B%, A% MOD B%)
210 DEF FNpower(B, E%)
220 IF E% = 0 THEN =1
230 =B * FNpower(B, E% - 1)
240 DEF FNdigits(N%)
250 IF N% < 10 THEN =1
260 =1 + FNdigits(N% DIV 10)
