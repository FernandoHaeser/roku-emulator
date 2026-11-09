' Print statement layout: comma tab zones, semicolons, trailing separators.
sub Main()
    print "@@PROBE_BEGIN"
    print "123456789012345", "x"
    print "1234567890123456", "x"
    print "12345678901234567", "x"
    print 1, 2, 3, 4, 5
    print -1, -2.5, 0
    print "a";
    print "b"
    print "c",
    print "d"
    print
    print "x"; 1; "y"; -1
    ? "question mark alias"
    print 1 + 2; " "; 3 * 4
    print "@@PROBE_END"
end sub
