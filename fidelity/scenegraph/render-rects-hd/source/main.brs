' Renderer geometry probe; the rectangles are created in ProbeScene.init()
' (render nodes cannot be created from the main thread on the device).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    print "@@PROBE_BEGIN"
    print scene.getChildCount()
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
