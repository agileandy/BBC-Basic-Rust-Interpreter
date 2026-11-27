10 REM Sales table: 3 shops by 4 quarters
20 DIM S%(3,4), N$(3)
30 FOR SHOP% = 1 TO 3
40   READ T$
50   N$(SHOP%) = T$
60   FOR Q% = 1 TO 4
70     READ V%
80     S%(SHOP%,Q%) = V%
90   NEXT Q%
100 NEXT SHOP%
110 PRINT "Shop";TAB(8);"Total"
120 GRAND% = 0
130 FOR SHOP% = 1 TO 3
140   T% = 0
150   FOR Q% = 1 TO 4 : T% = T% + S%(SHOP%,Q%) : NEXT Q%
160   PRINT N$(SHOP%);TAB(8);T%
170   GRAND% = GRAND% + T%
180 NEXT SHOP%
190 PRINT "Quarter totals:";
200 FOR Q% = 1 TO 4
210   T% = 0
220   FOR SHOP% = 1 TO 3 : T% = T% + S%(SHOP%,Q%) : NEXT SHOP%
230   PRINT " ";T%;
240 NEXT Q%
250 PRINT
260 PRINT "Grand total: ";GRAND%
270 DATA North,12,15,9,20
280 DATA South,8,11,14,7
290 DATA West,21,18,16,25
