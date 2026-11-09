' Arrays and associative arrays: methods, ordering, printing.
sub Main()
    print "@@PROBE_BEGIN"
    a = [3, 1, 2]
    a.push(9)
    print a.count(), a[0], a[3], a.peek()
    print a.pop(), a.count()
    a.unshift(0)
    print a.shift(), a.join("-")
    print a
    a.sort()
    print a.join(",")
    print a[10] = invalid
    aa = {b: 2, a: 1, Zed: "z"}
    aa.c = 3
    aa["D"] = 4
    print aa.count(), aa.a, aa.B, aa["zed"], aa.doesExist("ZED"), aa.missing = invalid
    print aa.keys().join(",")
    for each key in aa
        print key; "="; aa[key]
    end for
    print aa
    aa.delete("b")
    print aa.count()
    nested = {list: [1, {x: 1}], name: "n"}
    print nested
    print [1, "two", 3.5, true, invalid]
    m.counter = 5
    print m.counter
    print type(a), type(aa)
    print "@@PROBE_END"
end sub
