sub init()
    m.keyboard = m.top.findNode("keyboard")
    m.radio = m.top.findNode("radio")
    rect = m.keyboard.boundingRect()
    print "keyboard bounds="; rect.width; ","; rect.height
    print "radio parent="; m.radio.parentSubtype("RadioButtonList")
    print "checked="; m.radio.checkedItem; " auto="; m.radio.checkOnSelect
    root = CreateObject("roSGNode", "ContentNode")
    for each title in ["First", "Second", "Third"]
        child = root.createChild("ContentNode")
        child.title = title
    end for
    m.radio.content = root
    m.radio.observeField("checkedItem", "checked")
    m.radio.observeField("itemSelected", "selected")
    m.keyboard.setFocus(true)
end sub
sub checked(event as object)
    print "checked="; event.getData()
end sub
sub selected(event as object)
    print "selected="; event.getData(); " checked="; m.radio.checkedItem
    if event.getData() = 1 then m.radio.checkOnSelect = false
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press
    if not press and key = "options"
        m.radio.setFocus(true)
        return true
    end if
    if press and key = "back"
        print "@@PROBE_END"
        return true
    end if
    return false
end function
