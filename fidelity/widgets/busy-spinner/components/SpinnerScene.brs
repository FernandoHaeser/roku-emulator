sub init()
    m.spinner = m.top.findNode("spinner")
    print "poster type="; m.spinner.poster.subtype()
    print "initial child same="; m.spinner.getChild(0).isSameNode(m.spinner.poster)
    print "uri="; m.spinner.uri; " poster="; m.spinner.poster.uri
    print "load="; m.spinner.poster.loadStatus
    print "size="; m.spinner.poster.bitmapWidth; ","; m.spinner.poster.bitmapHeight
    m.spinner.poster.uri = ""
    print "clear alias="; m.spinner.uri = ""
    replacement = CreateObject("roSGNode", "Poster")
    replacement.uri = "pkg:/images/panel.png"
    m.spinner.poster = replacement
    print "replacement alias="; m.spinner.uri
    print "replacement children="; m.spinner.getChildCount()
    print "replacement child same="; m.spinner.getChild(0).isSameNode(m.spinner.poster)
    m.spinner.uri = "pkg:/images/other.png"
    print "forward child uri="; m.spinner.getChild(0).uri
    print "forward public uri="; m.spinner.poster.uri
    print "@@PROBE_END"
end sub
function onKeyEvent(key as string, press as boolean) as boolean
    return false
end function
