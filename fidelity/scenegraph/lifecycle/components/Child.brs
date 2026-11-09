sub init()
    print "child init id='"; m.top.id; "' name='"; m.top.name; "' parent? "; m.top.getParent() <> invalid
    print "child sees base helper: "; baseHelper()
end sub

sub onName()
    print "child onName "; m.top.name
end sub

function describe(param) as string
    return "described " + m.top.id + " " + param
end function
