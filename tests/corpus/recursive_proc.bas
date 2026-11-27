10 REM Towers of Hanoi with a recursive PROC
20 MOVES% = 0
30 PROChanoi(3, "A", "C", "B")
40 PRINT "Moves: ";MOVES%
50 PRINT "Count down:";
60 PROCcount(5)
70 PRINT
80 END
90 DEF PROChanoi(N%, F$, T$, V$)
100 IF N% = 0 THEN ENDPROC
110 PROChanoi(N% - 1, F$, V$, T$)
120 MOVES% = MOVES% + 1
130 PRINT "Disc ";N%;" from ";F$;" to ";T$
140 PROChanoi(N% - 1, V$, T$, F$)
150 ENDPROC
160 DEF PROCcount(N%)
170 PRINT " ";N%;
180 IF N% > 1 THEN PROCcount(N% - 1)
190 ENDPROC
