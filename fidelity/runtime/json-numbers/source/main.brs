' FormatJson number formatting for Float and Double values.
sub Main()
    print "@@PROBE_BEGIN"
    floats = [0.0, 1.5, 0.1, -2.25, 1.0/3.0, 19.0, 123456.7, 1234567.0, 16777217.0, 1e10, 2147483647.0, 1e-7, 3.4e38]
    for each value in floats
        print FormatJson(value); " "; FormatJson([value]); " print="; value
    end for
    doubles = [0.1#, 1.0# / 3.0#, 2147483648#, 1e20#, 19#]
    for each value in doubles
        print FormatJson(value); " print="; value
    end for
    big = 2147483647
    print FormatJson([big + 0.0, big + 19.0])
    small = 3
    print type(big); " "; type(2147483647); " "; type(999999999); " "; type(1000000000); " "; type(&h7FFFFFFF)
    print type(big + 0.0); " "; type(small + 0.5); " "; type(big * 1.0); " "; type(0.5 + big)
    for each n in [16777215, 16777216, 16777217, -16777217, 33554432, 33554433, 100000000]
        print n; " "; type(n + 0.5); " "; type(n - 0.0); " "; type(n * 1.0); " "; type(n / 2); " "; type(n > 0.5)
    end for
    f = 2147483647.0
    print type(f); " "; type(1.5); " "; type(16777217.0); " "; type(1234567.0)
    print "@@PROBE_END"
end sub
