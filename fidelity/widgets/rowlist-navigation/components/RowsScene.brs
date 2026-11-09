sub init()
    m.rows = m.top.findNode("rows")
    content = CreateObject("roSGNode", "ContentNode")
    for rowIndex = 0 to 2
        row = content.createChild("ContentNode")
        row.title = "Row " + rowIndex.ToStr()
        for col = 0 to rowIndex + 1
            item = row.createChild("ContentNode")
            item.title = rowIndex.ToStr() + ":" + col.ToStr()
        end for
    end for
    for each field in ["itemFocused", "itemUnfocused", "itemSelected", "rowItemFocused", "rowItemSelected"]
        m.rows.observeField(field, "changed")
    end for
    m.rows.content = content
    print "content="; m.rows.itemFocused; " "; FormatJSON(m.rows.rowItemFocused)
    m.rows.setFocus(true)
    print "focus="; m.rows.itemFocused; " "; FormatJSON(m.rows.rowItemFocused)
end sub
sub changed(event as object)
    print event.getField(); "="; FormatJSON(event.getData())
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    print "scene "; key; " "; press; " "; FormatJSON(m.rows.rowItemFocused)
    if key = "back" and press then
        print "@@PROBE_END"
        return true
    end if
    return false
end function
