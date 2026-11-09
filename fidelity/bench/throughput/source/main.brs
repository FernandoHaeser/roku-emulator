' Benchmark probe: measures interpreter throughput on the reference device.
' Results calibrate the emulator's "device mode" throttling. Output is metrics,
' so this probe is never diffed against the emulator.
sub Main()
    print "@@PROBE_BEGIN"
    timer = CreateObject("roTimespan")

    timer.Mark()
    total = 0
    for i = 1 to 1000000
        total = total + (i mod 7)
    end for
    print "int_loop_1m_ms=" + timer.TotalMilliseconds().ToStr()

    timer.Mark()
    text = ""
    for i = 1 to 20000
        text = text + "x"
    end for
    print "string_concat_20k_ms=" + timer.TotalMilliseconds().ToStr()

    timer.Mark()
    map = {}
    for i = 1 to 100000
        map["k" + (i mod 1000).ToStr()] = i
    end for
    print "aa_set_100k_ms=" + timer.TotalMilliseconds().ToStr()

    timer.Mark()
    for i = 1 to 100000
        total = Increment(total)
    end for
    print "call_100k_ms=" + timer.TotalMilliseconds().ToStr()

    timer.Mark()
    items = []
    for i = 1 to 100000
        items.Push(i)
    end for
    print "array_push_100k_ms=" + timer.TotalMilliseconds().ToStr()

    print "@@PROBE_END"
end sub

function Increment(value as integer) as integer
    return value + 1
end function
