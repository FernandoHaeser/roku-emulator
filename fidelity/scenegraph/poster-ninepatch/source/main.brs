' 9-patch probe (see ProbeScene.xml); reports Poster load results.
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    sleep(1500)
    print "@@PROBE_BEGIN"
    for i = 0 to scene.getChildCount() - 1
        node = scene.getChild(i)
        if node.subtype() = "Poster" and node.uri <> "" then
            print node.loadStatus; " "; node.bitmapWidth; "x"; node.bitmapHeight; " "; node.boundingRect()
        end if
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
