sub init()
    m.inner = m.top.findNode("inner")
    m.outer = m.top.findNode("outer")
    m.inner.setFocus(true)
    print "inner focus"; m.inner.hasFocus(); " outer chain"; m.outer.isInFocusChain(); " outer focus"; m.outer.hasFocus()
    print "scene chain"; m.top.isInFocusChain(); " focusedChild "; m.top.focusedChild.id; " "; m.outer.focusedChild.id
    m.count = 0
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    print "scene key "; key; " press "; press
    if press then m.count = m.count + 1
    if key = "back" and press = false then print "@@PROBE_END"
    return key = "down" or key = "back"
end function
