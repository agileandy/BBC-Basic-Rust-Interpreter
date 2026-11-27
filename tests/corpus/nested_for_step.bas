10 REM Nested FOR loops with STEP, and NEXT with two names
20 FOR ROW% = 1 TO 4
30   FOR COL% = 1 TO 4
40     PRINT TAB(COL% * 4 - 4);ROW% * COL%;
50   NEXT COL%
60   PRINT
70 NEXT ROW%
80 T% = 0
90 FOR I% = 1 TO 3
100   FOR J% = 10 TO 30 STEP 10
110     T% = T% + I% * J%
120 NEXT J%, I%
130 PRINT "Sum of products: ";T%
140 PRINT "Count down:";
150 FOR K% = 10 TO 1 STEP -3
160   PRINT " ";K%;
170 NEXT
180 PRINT
190 PRINT "Halves:";
200 FOR X = 0 TO 2 STEP 0.5
210   PRINT " ";X;
220 NEXT X
230 PRINT
240 FOR I% = 5 TO 1
250   PRINT "Body runs once with I% = ";I%
260 NEXT I%
270 PRINT "After the loop I% = ";I%
