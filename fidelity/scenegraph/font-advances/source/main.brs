' Prints "font|code|w10|w30" for every character (see ProbeScene.xml).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    print "@@PROBE_BEGIN"
    ' Child 0 is the Scene's implicit background Poster.
    count = scene.getChildCount()
    for i = 1 to count - 1 step 2
        short = scene.getChild(i)
        long = scene.getChild(i + 1)
        print short.id; "|"; Asc(short.text); "|"; short.boundingRect().width; "|"; long.boundingRect().width
    end for
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
