' Control flow: if, for, for each, while, exit, single-line if.
sub Main()
    print "@@PROBE_BEGIN"
    for i = 1 to 3
        print "for"; i
    end for
    for i = 10 to 1 step -4
        print "down"; i
    end for
    for i = 1 to 0
        print "never"
    end for
    print "after loop i ="; i
    for each item in [10, "x", true]
        print item
    end for
    n = 0
    while true
        n = n + 1
        if n = 2 then continue while
        if n > 4 then exit while
        print "while"; n
    end while
    for i = 1 to 5
        if i = 3 then exit for
        print "exit-for"; i
    next
    x = 7
    if x > 5 then print "single-line then"
    if x < 5 then print "no" else print "single-line else"
    if x = 1
        print "one"
    else if x = 7
        print "seven"
    else
        print "other"
    end if
    if x > 1 and x < 10 or false then print "logic"
    if not x = 3 then print "not"
    print "@@PROBE_END"
end sub
