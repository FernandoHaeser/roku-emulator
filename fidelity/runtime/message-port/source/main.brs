' roMessagePort / wait semantics without SceneGraph.
sub Main()
    print "@@PROBE_BEGIN"
    port = CreateObject("roMessagePort")
    print type(port)
    print port.GetMessage() = invalid, port.PeekMessage() = invalid
    timer = CreateObject("roTimespan")
    message = wait(50, port)
    elapsed = timer.TotalMilliseconds()
    print message = invalid, elapsed >= 45, elapsed < 500
    print type(wait(1, port))
    print "@@PROBE_END"
end sub
