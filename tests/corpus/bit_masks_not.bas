10 REM Clear bits with AND NOT, and invert the bits with NOT
20 PRINT "NOT 4 = ";NOT 4
30 PRINT "NOT 0 = ";NOT 0
40 PRINT "NOT &FF = ";NOT &FF
50 FLAGS% = 1 OR 4 OR 8
60 PRINT "Flags: ";FLAGS%
70 FLAGS% = FLAGS% AND NOT 4
80 PRINT "After clearing bit 2: ";FLAGS%
90 FLAGS% = FLAGS% AND NOT 1
100 PRINT "After clearing bit 0: ";FLAGS%
110 M% = &F0
120 PRINT "Low nibble of &5A: &";~(&5A AND NOT M%)
130 PRINT "NOT NOT 7 = ";NOT NOT 7
140 X% = 6
150 PRINT "NOT X% + 1 = ";NOT X% + 1
160 IF NOT FALSE THEN PRINT "NOT FALSE is TRUE"
