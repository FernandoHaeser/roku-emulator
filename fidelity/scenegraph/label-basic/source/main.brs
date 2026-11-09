' SceneGraph probe: static Rectangle + Label. Captures app-ui and a screenshot,
' validating renderer basics (fill, text, autoscale FHD -> device resolution).
sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("ProbeScene")
    screen.Show()

    print "@@PROBE_BEGIN"
    label = scene.FindNode("title")
    print label.text
    print label.boundingRect()
    print "@@PROBE_END"

    ' Keep the channel alive so the oracle can capture the screen.
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
