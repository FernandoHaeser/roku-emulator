' Animation probe (see ProbeScene.xml).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()
    target = scene.findNode("box")
    print "@@PROBE_BEGIN"

    print "-- fraction-driven interpolators"
    f = scene.findNode("fopacity")
    for each fr in [0, 0.25, 0.5, 0.75, 1, 1.5, -0.5]
        f.fraction = fr
        print "opacity "; fr; " -> "; target.opacity
    end for
    v = scene.findNode("vtrans")
    for each fr in [0, 0.5, 1]
        v.fraction = fr
        print "translation "; fr; " -> "; target.translation[0]; target.translation[1]
    end for
    v.reverse = true
    v.fraction = 0.25
    print "reverse 0.25 -> "; target.translation[0]; target.translation[1]
    c = scene.findNode("ccolor")
    for each fr in [0, 0.5, 1]
        c.fraction = fr
        print "color "; fr; " -> "; StrI(target.color, 16)
    end for

    print "-- lifecycles"
    for each id in ["single", "seq", "seq1", "seq2", "par", "par1", "par2"]
        scene.findNode(id).observeField("state", port)
    end for
    print "initial "; scene.findNode("single").state
    scene.findNode("single").control = "start"
    print "after start "; scene.findNode("single").state
    waitStopped(port, ["single"])
    a = scene.findNode("a")
    print "single end width "; a.width
    scene.findNode("seq").control = "start"
    waitStopped(port, ["seq"])
    b = scene.findNode("b")
    print "seq end "; b.width; b.height
    scene.findNode("par").control = "start"
    waitStopped(port, ["par"])
    print "par end "; a.height; b.translation[0]; b.translation[1]
    print "@@PROBE_END"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub

' Prints every state event until all ids in `until` reported "stopped".
sub waitStopped(port, until)
    pending = {}
    for each id in until
        pending[id] = true
    end for
    while pending.Count() > 0
        message = wait(3000, port)
        if message = invalid then
            print "timeout"
            return
        end if
        if type(message) = "roSGNodeEvent" then
            id = message.getNode()
            print "  "; id; " "; message.getData()
            if message.getData() = "stopped" then pending.Delete(id)
        end if
    end while
end sub
