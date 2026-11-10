sub init()
    m.keyboard = m.top.findNode("keyboard")
    rect = m.keyboard.boundingRect()
    print "bounds="; rect.width; ","; rect.height
    print "lower="; m.keyboard.lowerCase
    print "max="; m.keyboard.textEditBox.maxTextLength
    m.keyboard.observeField("text", "changed")
    m.keyboard.setFocus(true)
end sub
sub changed(event as object)
    print "text="; FormatJSON(event.getData()); " cursor="; m.keyboard.textEditBox.cursorPosition
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press
    if key = "back" and press
        print "final="; FormatJSON(m.keyboard.text)
        print "@@PROBE_END"
        return true
    end if
    return false
end function
