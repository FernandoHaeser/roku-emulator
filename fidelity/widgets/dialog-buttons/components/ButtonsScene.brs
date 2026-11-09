sub init()
    m.buttons = m.top.findNode("buttons")
    print "initial="; m.buttons.buttonFocused; " "; m.buttons.buttonSelected
    m.buttons.observeField("buttonFocused", "groupChanged")
    m.buttons.observeField("buttonSelected", "groupChanged")
    m.buttons.buttons = ["One", "Two"]
    print "children="; m.buttons.getChildCount()
    m.buttons.setFocus(true)
    print "groupfocus="; m.buttons.hasFocus(); " focused="; m.buttons.buttonFocused
end sub
sub groupChanged(event as object)
    print "group "; event.getField(); "="; FormatJSON(event.getData())
    if event.getField() = "buttonSelected" then
        m.dialog = CreateObject("roSGNode", "Dialog")
        m.dialog.title = "Confirm"
        m.dialog.message = "Choose one"
        m.dialog.buttons = ["Yes", "No"]
        print "dialog group="; m.dialog.buttonGroup.subtype(); " count="; m.dialog.buttonGroup.getChildCount()
        print "dialog defaults="; m.dialog.buttonFocused; " "; m.dialog.buttonSelected; " "; m.dialog.wasClosed
        for each name in ["buttonFocused", "buttonSelected", "wasClosed"]
            m.dialog.observeField(name, "dialogChanged")
        end for
        m.top.dialog = m.dialog
        print "shown="; m.dialog.visible
    end if
end sub
sub dialogChanged(event as object)
    print "dialog "; event.getField(); "="; FormatJSON(event.getData())
    if event.getField() = "buttonSelected" then m.dialog.close = true
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press
    if key = "back" and press then
        print "@@PROBE_END"
        return true
    end if
    return false
end function
