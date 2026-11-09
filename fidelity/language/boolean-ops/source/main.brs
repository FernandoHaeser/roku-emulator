' Logical operators on booleans vs bitwise on integers, comparisons with invalid.
sub Main()
    print "@@PROBE_BEGIN"
    print true and false, true or false, not true
    print 12 and 10, 12 or 3, not 0
    print 1 = 1 and 2 = 2
    print invalid = invalid, invalid <> invalid
    x = invalid
    print x = invalid, type(x)
    print 5 << 1, 256 >> 4
    print "@@PROBE_END"
end sub
