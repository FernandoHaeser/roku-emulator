' Remote Poster probe (see ProbeScene.xml).
sub Main()
    print "@@PROBE_BEGIN"
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    for attempt = 1 to 30
        if scene.done then exit for
        sleep(500)
    end for
    if not scene.done then print "timeout"
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
