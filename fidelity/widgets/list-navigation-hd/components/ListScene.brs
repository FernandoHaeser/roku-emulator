sub init()
    m.list = m.top.findNode("list")
    for each kind in ["LabelList", "MarkupList", "MarkupGrid", "RowList", "Button", "Keyboard", "Dialog"]
        node = CreateObject("roSGNode", kind)
        fields = node.getFields()
        print "defaults "; kind
        for each name in ["itemSize", "itemSpacing", "numRows", "numColumns", "itemFocused", "itemUnfocused", "itemSelected", "rowItemFocused", "rowItemSelected", "vertFocusAnimationStyle", "horizFocusAnimationStyle", "focusable", "text", "buttonSelected", "buttonFocused"]
            if fields.DoesExist(name) then print name; "="; FormatJSON(fields[name])
        end for
    end for
    content = CreateObject("roSGNode", "ContentNode")
    for each title in ["Alpha", "Beta", "Gamma"]
        item = content.createChild("ContentNode")
        item.title = title
    end for
    m.list.observeField("itemFocused", "changed")
    m.list.observeField("itemUnfocused", "changed")
    m.list.observeField("itemSelected", "changed")
    m.list.content = content
    print "content focus="; m.list.itemFocused
    m.list.setFocus(true)
    print "focused="; m.list.hasFocus(); " item="; m.list.itemFocused
end sub

sub changed(event as object)
    print "event "; event.getField(); "="; FormatJSON(event.getData())
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press; " focus="; m.list.itemFocused
    if key = "back" and press then
        print "@@PROBE_END"
        return true
    end if
    return false
end function
