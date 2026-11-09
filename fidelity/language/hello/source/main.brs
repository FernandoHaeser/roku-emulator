' Smallest conformance probe: print formatting of basic values.
sub Main()
    print "@@PROBE_BEGIN"
    print "hello"
    print 7 \ 2, 7 / 2, 2 ^ 10
    print 1; 2; -3
    print "a"; "b", "c"
    print 1.5, 1.0 / 3, 100000000
    print true, false, invalid
    print "@@PROBE_END"
end sub
