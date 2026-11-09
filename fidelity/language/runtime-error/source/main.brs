' Runtime error: console output format of a crash.
sub Main()
    print "@@PROBE_BEGIN"
    value = "text"
    print "before"
    result = value + 1
    print "unreachable"; result
    print "@@PROBE_END"
end sub
