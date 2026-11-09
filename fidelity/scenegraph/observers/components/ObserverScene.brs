sub init()
    m.top.observeField("value", "onValueA")
    m.top.observeField("value", "onValueB")
    m.top.observeField("quiet", "onQuiet")
    m.top.observeField("always", "onAlways")
    print "set value 1"
    m.top.value = 1
    print "set value 1 again"
    m.top.value = 1
    print "set value 2"
    m.top.value = 2
    print "set quiet 5 twice"
    m.top.quiet = 5
    m.top.quiet = 5
    print "set always 3 twice"
    m.top.always = 3
    m.top.always = 3
    print "unobserve value"
    m.top.unobserveField("value")
    m.top.value = 4
    print "after unobserve"
    node = CreateObject("roSGNode", "Node")
    node.addField("count", "integer", false)
    node.observeField("count", "onCount")
    node.count = 10
    print "setFields"
    m.top.setFields({ value: 7, quiet: 8 })
    print "@@PROBE_END"
end sub

sub onValueInterface()
    print "  onValueInterface "; m.top.value
end sub

sub onValueA(event as object)
    print "  onValueA "; type(event); " "; event.getField(); " "; event.getData(); " "; event.getNode()
end sub

sub onValueB()
    print "  onValueB "; m.top.value
end sub

sub onQuiet(event)
    print "  onQuiet "; event.getData()
end sub

sub onAlways(event)
    print "  onAlways "; event.getData()
end sub

sub onCount(event)
    print "  onCount "; event.getData(); " role node subtype "; event.getRoSGNode().subtype()
end sub
