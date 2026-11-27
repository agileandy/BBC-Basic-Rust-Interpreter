10 REM PRINT separators: the apostrophe, the semicolon and the comma
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
130 REM A number at the start of PRINT fills a field of 10
140 PRINT 42
150 REM A semicolon before a number turns the field off
160 PRINT ;42
170 REM A comma moves to the next field of 10 and turns the field on
180 PRINT "A";42,42
190 REM A comma at the edge of a field prints no spaces
200 PRINT 1,22,333
210 REM An apostrophe does not change the field
220 PRINT X'X * 2
230 PRINT ;X'X * 2
240 PRINT "No new line after this";
250 PRINT '"End"
