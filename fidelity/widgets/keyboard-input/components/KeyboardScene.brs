sub init()
    m.keyboard = m.top.findNode("keyboard")
    print "text="; FormatJSON(m.keyboard.text)
    print "max="; m.keyboard.textEditBox.maxTextLength
    m.keyboard.textEditBox.maxTextLength = 3
    m.keyboard.observeField("text", "changed")
    m.keyboard.setFocus(true)
end sub
sub changed(event as object)
    print "text="; FormatJSON(event.getData())
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press
    if key = "back" and press then
        print "final="; FormatJSON(m.keyboard.text); " cursor="; m.keyboard.textEditBox.cursorPosition
        print "@@PROBE_END"
        return true
    end if
    return false
end function
