sub init()
    m.top.functionName = "work"
    print "worker init"
end sub

sub work()
    print "worker running input"; m.top.input
    total = 0
    for i = 1 to m.top.input
        total = total + i
    end for
    m.top.result = total
    print "worker done"
end sub
