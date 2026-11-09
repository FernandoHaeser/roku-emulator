sub init()
    print "scene init, children:"; m.top.getChildCount()
    print "first name="; m.top.findNode("first").name
    child = CreateObject("roSGNode", "Child")
    print "dynamic child created"
    child.name = "dynamic"
    print "callFunc ->"; m.top.findNode("first").callFunc("describe", "arg")
    print "callFunc missing ->"; m.top.findNode("first").callFunc("nope")
    print "@@PROBE_END"
end sub
