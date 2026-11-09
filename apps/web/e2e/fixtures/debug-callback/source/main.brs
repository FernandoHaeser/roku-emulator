sub Main()
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("DebugScene")
    screen.Show()
    print "callback completed"
    while true
        msg = wait(0, port)
        if type(msg) = "roSGScreenEvent" then exit while
    end while
end sub
