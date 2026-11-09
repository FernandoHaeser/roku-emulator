sub init()
    print "init: m.top subtype "; m.top.subtype(); " id='"; m.top.id; "'"
    print "global tag: "; m.global.tag
    print m.top.score, type(m.top.score), m.top.ratio, type(m.top.ratio), m.top.label = ""
    m.top.score = "12"
    print "score after string set:"; m.top.score; " "; type(m.top.score)
    m.top.score = 3.9
    print "score after float set:"; m.top.score
    print m.top.hasField("score"), m.top.hasField("SCORE"), m.top.hasField("nope")
    print m.top.flags, m.top.items
    group = m.top.findNode("group")
    print type(group), group.subtype(), group.getChildCount(), group.isSubtype("Group"), group.isSubtype("Node")
    rect = m.top.findNode("rect")
    print rect.width, rect.height, rect.color, type(rect.color)
    print rect.translation, rect.visible, rect.opacity
    print rect.getParent().id, rect.isSameNode(group.getChild(0))
    content = CreateObject("roSGNode", "ContentNode")
    content.title = "Movie"
    content.addFields({ custom: 1, other: "x" })
    child = content.createChild("ContentNode")
    child.id = "c1"
    print content.title, content.custom, content.other, content.getChildCount(), content.getChild(0).id
    print content.getFields().count() > 5, type(content.getFields())
    node = CreateObject("roSGNode", "Node")
    node.addField("value", "int", true)
    node.value = 5
    node.setField("value", 6)
    print node.getField("value"), node.value, node.id = ""
    print node.setFields({ value: 9, nope: 1 })
    print node.value
    print node
    print rect
    print content.count = invalid
    print group.removeChild(rect), group.getChildCount(), rect.getParent() = invalid
    print group.appendChild(rect), group.getChildCount(), group.getChild(1).id
    print group.insertChild(CreateObject("roSGNode", "Rectangle"), 0), group.getChild(0).subtype()
    print group.getChildren(-1, 0).count(), group.getChildren(1, 1)[0].id
    print m.top.findNode("missing") = invalid, CreateObject("roSGNode", "NotAComponent") = invalid
    print "@@PROBE_END"
end sub
