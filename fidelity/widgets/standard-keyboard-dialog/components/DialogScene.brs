sub init()
    print "string interface="; GetInterface("demo", "ifString") <> invalid
    field = m.top.findNode("field")
    print "max="; field.maxTextLength
    m.dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
    m.dialog.title = "Enter test text"
    m.dialog.buttons = ["OK"]
    m.dialog.observeField("text", "changed")
    m.dialog.observeField("buttonSelected", "selected")
    m.dialog.observeField("wasClosed", "closed")
    m.top.dialog = m.dialog
end sub
sub changed(event as object)
    print "text="; FormatJSON(event.getData())
end sub
sub selected(event as object)
    print "button="; event.getData()
    print "final="; FormatJSON(m.dialog.text)
    m.top.findNode("field").text = m.dialog.text
    m.dialog.close = true
end sub
sub closed()
    print "closed"
    m.top.setFocus(true)
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    if key = "back" and press
        print "@@PROBE_END"
        return true
    end if
    return false
end function
