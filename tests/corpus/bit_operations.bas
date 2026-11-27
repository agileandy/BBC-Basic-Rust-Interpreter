10 REM Bit operations with AND, OR and EOR
20 A% = &F0 : B% = &3C
30 PRINT "A AND B = &";~(A% AND B%)
40 PRINT "A OR B = &";~(A% OR B%)
50 PRINT "A EOR B = &";~(A% EOR B%)
60 PRINT "NOT 0 = ";NOT 0
70 PRINT "NOT -1 = ";NOT -1
80 REM Count the bits that are set
90 N% = 181 : C% = 0
100 FOR BIT% = 0 TO 7
110   IF N% AND 2 ^ BIT% THEN C% = C% + 1
120 NEXT BIT%
130 PRINT "Bits set in 181: ";C%
140 REM Flags in one byte
150 FLAGS% = 0
160 FLAGS% = FLAGS% OR 1 : FLAGS% = FLAGS% OR 4 : FLAGS% = FLAGS% OR 8
170 FLAGS% = FLAGS% EOR 4
180 PRINT "Flags: ";FLAGS%;" (&";~FLAGS%;")"
190 REM Swap two values with EOR
200 X% = 12 : Y% = 25
210 X% = X% EOR Y% : Y% = X% EOR Y% : X% = X% EOR Y%
220 PRINT "X% = ";X%;" Y% = ";Y%
230 PRINT "TRUE AND FALSE = ";TRUE AND FALSE;"  TRUE OR FALSE = ";TRUE OR FALSE
240 PRINT "&FF EOR &0F = ";&FF EOR &0F
250 PRINT "Mask low nibble of &ABCD: &";~(&ABCD AND &F)
