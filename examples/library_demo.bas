10 REM The 2 example libraries of examples/lib (issue #186)
20 REM Start the interpreter in the folder of the repository, then
30 REM type: CHAIN "examples/library_demo.bas"
40 INSTALL "examples/lib/strings"
50 INSTALL "examples/lib/numbers"
60 PRINT "Strings"
70 PRINT "[";FNtrim$("   trim me  ");"]"
80 PRINT "[";FNpadleft$("42", 5);"]"
90 PRINT "[";FNpadright$("42", 5);"]"
100 PRINT FNrepeat$("-=", 3)
110 L% = FNsplit("one,two,three", ",")
120 PRINT ;LISTLEN(L%);" parts: ";LISTGET$(L%, 0);" ";LISTGET$(L%, 1);" ";LISTGET$(L%, 2)
130 PRINT "Numbers"
140 PRINT "min ";FNmin(2, 9);" max ";FNmax(2, 9)
150 PRINT "clamp ";FNclamp(250, 0, 100)
160 PRINT "round ";FNround(PI, 3)
