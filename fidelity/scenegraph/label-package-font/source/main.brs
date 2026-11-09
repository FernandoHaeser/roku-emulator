' Package font probe (see ProbeScene.xml); reports Label metrics.
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
        if node.subtype() = "Label" then
            rect = node.boundingRect()
            print node.id; " | "; rect.width; " x "; rect.height
        end if
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
