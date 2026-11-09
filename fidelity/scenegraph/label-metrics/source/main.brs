' Prints the device's text metrics for every system font (see ProbeScene.xml).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    print "@@PROBE_BEGIN"
    for i = 1 to scene.getChildCount() - 1
        label = scene.getChild(i)
        rect = label.boundingRect()
        print label.id; " | "; label.text; " | "; rect.width; " x "; rect.height
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
