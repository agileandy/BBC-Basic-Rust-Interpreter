10 REM Integer division with DIV and MOD
20 PRINT "17 DIV 5 = ";17 DIV 5;"  17 MOD 5 = ";17 MOD 5
30 PRINT "-17 DIV 5 = ";-17 DIV 5;"  -17 MOD 5 = ";-17 MOD 5
40 PRINT "17.9 DIV 2 = ";17.9 DIV 2
50 REM Seconds to hours, minutes and seconds
60 T% = 10000
70 H% = T% DIV 3600 : M% = (T% MOD 3600) DIV 60 : S% = T% MOD 60
80 PRINT T%;" seconds is ";H%;"h ";M%;"m ";S%;"s"
90 REM Digit sum and reverse
100 N% = 4096 : SUM% = 0 : REV% = 0
110 REPEAT
120   D% = N% MOD 10
130   SUM% = SUM% + D%
140   REV% = REV% * 10 + D%
150   N% = N% DIV 10
160 UNTIL N% = 0
170 PRINT "Digit sum ";SUM%;", reversed ";REV%
180 REM Binary from repeated DIV 2
190 N% = 37 : B$ = ""
200 REPEAT
210   B$ = STR$(N% MOD 2) + B$
220   N% = N% DIV 2
230 UNTIL N% = 0
240 PRINT "37 in binary is ";B$
250 PRINT "Leap years:";
260 FOR Y% = 1896 TO 1912 STEP 4
270   IF Y% MOD 4 = 0 AND (Y% MOD 100 <> 0 OR Y% MOD 400 = 0) THEN PRINT " ";Y%;
280 NEXT Y%
290 PRINT
