' roList, boxing, interfaces and misc globals.
sub Main()
    print "@@PROBE_BEGIN"
    list = CreateObject("roList")
    list.AddTail(1)
    list.AddTail("b")
    list.AddHead(0)
    print type(list), list.Count(), list.GetHead(), list.GetTail()
    print list.RemoveHead(), list.RemoveTail(), list.Count()
    print list
    for each item in list
        print "item"; item
    end for
    s = CreateObject("roString")
    s.SetString("boxed")
    print type(s), s, s.Len()
    print type(Box(5)), type(Box("x")), type(Box(1.5))
    print GetInterface(list, "ifList") <> invalid, GetInterface(list, "ifNope") = invalid
    print type(CreateObject("roNope"))
    print Substitute("{0}-{1}-{0}", "a", "b")
    print Tr("Hello")
    print "@@PROBE_END"
end sub
