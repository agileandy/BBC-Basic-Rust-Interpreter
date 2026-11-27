10 REM PRINT with the apostrophe for a new line
20 PRINT "Line one"'"Line two"
30 PRINT "Gap below"''"After a blank line"
40 PRINT '"Starts on a new line"
50 FOR I% = 1 TO 3
60   PRINT "Item ";I%'"--"
70 NEXT I%
80 A$ = "Name" : B$ = "Bob"
90 PRINT A$;":"'B$
100 PRINT "Sum: ";2 + 3'"Product: ";2 * 3
110 PRINT TAB(2);"Tab";'SPC(2);"Space"
120 X = 1.5
130 PRINT X'X * 2'X * 3
140 PRINT "No new line after this";
150 PRINT '"End"
