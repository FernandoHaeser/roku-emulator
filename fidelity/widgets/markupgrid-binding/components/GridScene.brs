sub init()
    m.grid = m.top.findNode("grid")
    content = CreateObject("roSGNode", "ContentNode")
    for each title in ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
        content.createChild("ContentNode").title = title
    end for
    for each field in ["itemFocused", "itemUnfocused", "itemSelected"]
        m.grid.observeField(field, "changed")
    end for
    m.grid.content = content
    m.grid.setFocus(true)
    print "focus="; m.grid.itemFocused
end sub
sub changed(event as object)
    print event.getField(); "="; FormatJSON(event.getData())
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press; " focus="; m.grid.itemFocused
    if key = "back" and press then
        print "@@PROBE_END"
        return true
    end if
    return false
end function
