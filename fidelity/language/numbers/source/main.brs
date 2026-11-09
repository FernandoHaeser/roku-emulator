' Numeric semantics: integer/float/double/long, division, mod, overflow, formatting.
sub Main()
    print "@@PROBE_BEGIN"
    print 7 / 2, 7 \ 2, -7 \ 2, 7 mod 3, -7 mod 3
    print 2 ^ 10, 2 ^ 0.5, 10 ^ -2
    print 2147483647 + 1
    print -2147483648 - 1
    print 1 / 3, 2 / 3, 10 / 4
    print 1.0 / 3.0#
    print 0.1 + 0.2
    print 1e7, 1e6, 123456.7, 1234567.8
    print 0.00001, 0.0001
    print 1.5#, 1.0# / 3.0#
    print 123456789012&
    print &HFF, &hff, -&H10
    print 3 = 3.0, 1 < 2, 2 <= 1
    print type(1), type(1.5), type(1.5#), type(1&), type(1 / 2), type(7 \ 2)
    print type(2 ^ 2), type(1 + 1.5), type(1.5 + 1.5#), type(1& + 1)
    print int(3.7), int(-3.7), fix(-3.7), abs(-4), sqr(16), cint(2.5), cint(3.5)
    x = 5
    x += 2
    x *= 3
    x -= 1
    x /= 4
    print x, type(x)
    print 5.0, 5.0#, 100.0, 1000000.0, 10000000.0
    print -0.5, 3.14159265358979
    print "@@PROBE_END"
end sub
