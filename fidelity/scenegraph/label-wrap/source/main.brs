' Wrapped Label metrics (see ProbeScene.xml): id | text | height.
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    sleep(1500)
    print "@@PROBE_BEGIN"
    print "default lineSpacing "; CreateObject("roSGNode", "Label").lineSpacing
    for i = 1 to scene.getChildCount() - 1
        label = scene.getChild(i)
        print label.id; " | "; label.text; " | "; label.boundingRect().height
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
