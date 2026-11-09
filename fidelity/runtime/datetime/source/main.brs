' roDateTime with fixed instants (no dependency on the current clock).
sub Main()
    print "@@PROBE_BEGIN"
    date = CreateObject("roDateTime")
    print type(date)
    date.FromSeconds(1700000000)
    print date.AsSeconds(), date.ToISOString()
    print date.GetYear(), date.GetMonth(), date.GetDayOfMonth(), date.GetHours(), date.GetMinutes(), date.GetSeconds()
    print date.GetDayOfWeek(), date.GetWeekday(), date.GetMilliseconds(), date.GetLastDayOfMonth()
    print date.AsDateString("long-date"), "|"; date.AsDateString("short-month-short-weekday"); "|"
    print date.AsDateStringNoParam()
    date.FromISO8601String("2024-02-29T23:59:58Z")
    print date.AsSeconds(), date.GetDayOfWeek(), date.ToISOString()
    date.FromISO8601String("2024-01-02 03:04:05")
    print date.AsSeconds(), date.ToISOString()
    date.FromSeconds(0)
    print date.ToISOString(), date.GetYear()
    print type(date.GetTimeZoneOffset())
    print "@@PROBE_END"
end sub
