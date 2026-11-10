sub init()
    m.username = m.top.findNode("username")
    m.password = m.top.findNode("password")
    m.current = m.username
    m.current.active = true
    m.current.setFocus(true)
    m.username.observeField("text", "changed")
    m.password.observeField("text", "changed")
    print "INPUT_READY"
end sub
sub changed(event as object)
    print "text="; FormatJSON(event.getData())
end sub
function onKeyEvent(key as string, pressed as boolean) as boolean
    if not pressed then return false
    if key = "down"
        m.current.active = false
        m.current = m.password
        m.current.active = true
        m.current.setFocus(true)
        print "PASSWORD_READY"
        return true
    else if key = "OK"
        m.dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
        m.dialog.title = "Edit test text"
        m.dialog.buttons = ["OK"]
        m.dialog.text = m.current.text
        m.dialog.textEditBox.secureMode = m.current.secureMode
        m.dialog.observeField("buttonSelected", "confirm")
        m.top.dialog = m.dialog
        print "DIALOG_READY"
        return true
    end if
    return false
end function
sub confirm()
    m.current.text = m.dialog.text
    print "confirmed="; FormatJSON(m.current.text)
    m.dialog.close = true
    m.current.setFocus(true)
end sub
