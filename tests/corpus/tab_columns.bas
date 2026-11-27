10 REM A table of powers in columns with TAB
20 PRINT "N";TAB(5);"N^2";TAB(12);"N^3";TAB(20);"SQR"
30 FOR N% = 1 TO 6
40   PRINT N%;TAB(5);N%^2;TAB(12);N%^3;TAB(20);INT(SQR(N%)*100+0.5)/100
50 NEXT N%
60 PRINT
70 PRINT "Right aligned:"
80 FOR I% = 1 TO 4
90   V% = 7 ^ I%
100   PRINT SPC(6 - LEN STR$ V%);V%
110 NEXT I%
130 PRINT TAB(3);"Indented";SPC(2);"text"
140 PRINT "Hex of 255: ";~255
150 PRINT "Negative: ";-42;" and ";-0.5
