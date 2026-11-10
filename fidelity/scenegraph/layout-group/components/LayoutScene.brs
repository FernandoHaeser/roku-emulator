sub init()
    m.phase=0
    m.outer=m.top.findNode("outer")
    m.timer=m.top.findNode("timer")
    m.timer.observeField("fire","tick")
    m.timer.control="start"
end sub
sub snapshot()
    print "phase=";m.phase
    for each id in ["outer","a","b","inner","c","d","offset","wrapped","e","visibility","rectangleWrapper"]
        node=m.top.findNode(id)
        if node=invalid
            print id; " removed"
        else
            r=node.boundingRect()
            t=node.translation
            print id; " t=";FormatJSON([t[0]+0,t[1]+0]);" r=";FormatJSON([r.x+0,r.y+0,r.width,r.height])
        end if
    end for
end sub
sub tick()
    snapshot()
    m.phase++
    if m.phase=1
        m.top.findNode("a").width=100
        m.top.findNode("b").visible=false
        m.top.findNode("c").scale=[2,1]
        m.outer.horizAlignment="center"
        m.outer.vertAlignment="center"
        m.outer.itemSpacings=[10]
    else if m.phase=2
        m.outer.layoutDirection="vert"
        m.outer.addItemSpacingAfterChild=false
        m.outer.horizAlignment="right"
        m.outer.vertAlignment="center"
    else if m.phase=3
        m.top.findNode("b").visible=true
        m.outer.horizAlignment="custom"
        m.top.findNode("a").translation=[37,500]
        m.top.findNode("b").translation=[-23,600]
    else if m.phase=4
        m.outer.removeChild(m.top.findNode("b"))
        m.outer.itemSpacings=[2,30]
    else if m.phase=5
        m.timer.control="stop"
        print "@@PROBE_END"
    end if
end sub
