10 REM REPEAT loops that end on a flag
20 N% = 27
30 STEPS% = 0
40 PEAK% = N%
50 REPEAT
60   IF N% MOD 2 = 0 THEN N% = N% DIV 2 ELSE N% = 3 * N% + 1
70   STEPS% = STEPS% + 1
80   IF N% > PEAK% THEN PEAK% = N%
90 UNTIL N% = 1
100 PRINT "27 reaches 1 in ";STEPS%;" steps, peak ";PEAK%
110 C% = 200
120 FOUND% = FALSE
130 REPEAT
140   C% = C% + 1
150   PRIME% = TRUE
160   D% = 2
170   REPEAT
180     IF C% MOD D% = 0 THEN PRIME% = FALSE
190     D% = D% + 1
200   UNTIL D% * D% > C% OR NOT PRIME%
210   IF PRIME% THEN FOUND% = TRUE
220 UNTIL FOUND%
230 PRINT "First prime after 200: ";C%
240 PRINT "TRUE is ";TRUE;" and FALSE is ";FALSE
