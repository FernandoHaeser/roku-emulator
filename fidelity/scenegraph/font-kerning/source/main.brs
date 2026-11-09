' Prints "font|codeA|codeB|width" for every kerning pair (see ProbeScene.xml).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    print "@@PROBE_BEGIN"
    ' Child 0 is the Scene's implicit background Poster.
    for i = 1 to scene.getChildCount() - 1
        label = scene.getChild(i)
        print label.id; "|"; Asc(label.text); "|"; Asc(Right(label.text, 1)); "|"; label.boundingRect().width
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
