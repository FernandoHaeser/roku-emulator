' ParseJson / FormatJson.
sub Main()
    print "@@PROBE_BEGIN"
    data = ParseJson("{""b"": 1, ""a"": [1, 2.5, ""x"", true, null], ""Nested"": {""k"": -3}, ""big"": 3000000000, ""e"": 1e3}")
    print type(data), data.Count()
    print data.b, type(data.b)
    print data.a
    print type(data.a[1]), type(data.a[4]), data.a[4] = invalid
    print data.nested.k, data.Nested.k
    print type(data.big), data.big
    print type(data.e), data.e
    print FormatJson(data)
    print FormatJson([1, "two", 3.5, false, invalid])
    print FormatJson({ text: "quote "" slash / back \ tab" + Chr(9) + " nl" + Chr(10) + " uni " + Chr(233) })
    print FormatJson("plain"), FormatJson(42), FormatJson(1.5)
    print ParseJson("not json") = invalid
    print ParseJson("[]").Count(), ParseJson("{}").Count()
    print ParseJson("""str""")
    print FormatJson({}), FormatJson([])
    print "@@PROBE_END"
end sub
