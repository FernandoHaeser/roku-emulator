sub init()
    print "poster in init="; m.top.poster <> invalid
    print "default control="; m.top.control; " clockwise="; m.top.clockwise; " interval="; m.top.spinInterval
    print "children="; m.top.getChildCount()
    m.top.control = "stop"
end sub
