sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    m.global = screen.getGlobalNode()
    m.global.addField("tag", "string", false)
    m.global.tag = "global-ok"
    print "@@PROBE_BEGIN"
    print "main: before CreateScene"
    scene = screen.CreateScene("NodeApiScene")
    print "main: after CreateScene"
    screen.Show()
    print "main: after Show"
    while true
        message = wait(0, port)
        if type(message) = "roSGScreenEvent" and message.IsScreenClosed() then return
    end while
end sub
