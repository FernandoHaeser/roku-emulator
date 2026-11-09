' String functions and operators.
sub Main()
    print "@@PROBE_BEGIN"
    s = "Hello, World"
    print len(s), left(s, 5), right(s, 5), mid(s, 8), mid(s, 8, 3)
    print instr(1, s, "o"), instr(6, s, "o"), instr(1, s, "z")
    print ucase(s), lcase(s)
    print "ab" + "cd", "abc" < "abd", "a" = "A", "B" > "a"
    print str(5), str(-5), str(2.5), stri(42), val("12.5"), val("abc"), val("3 apples")
    print chr(65), asc("A"), asc("")
    print "quote: ""x"""
    print s.len(), s.left(2), s.mid(1, 3), s.instr("World"), s.instr(0, "o")
    print "  pad  ".trim() + "|"
    print "a,b,,c".split(",").count()
    print "42".toint() + 1, "2.5".tofloat() * 2
    print 12.toStr() + "|", (1.5).toStr() + "|", (-3).toStr() + "|"
    print "abc".ucase(), "ABC".lcase()
    print "x".len() = 1
    print type("x"), type(s.len())
    print "@@PROBE_END"
end sub
