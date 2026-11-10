sub init()
    m.phase = ""
    m.plays = 0
    m.positions = []
    printDefaults()
    m.top.findNode("pauseTimer").observeField("fire", "onPauseTimer")
    m.top.findNode("seekTimer").observeField("fire", "onSeekTimer")
    m.top.findNode("stopTimer").observeField("fire", "onStopTimer")
    startPhase("clip", "pkg:/media/clip.mp4", "mp4")
end sub

sub printDefaults()
    video = CreateObject("roSGNode", "Video")
    fields = video.getFields()
    names = []
    for each name in fields
        names.push(name)
    end for
    names.sort()
    print "-- defaults"
    for each name in names
        value = fields[name]
        kind = type(value)
        if kind = "roString" or kind = "String" then
            print name; "="; Chr(34); value; Chr(34)
        else if kind = "roBoolean" or kind = "Boolean" or kind = "roInt" or kind = "Integer" or kind = "roFloat" or kind = "Float" or kind = "roDouble" or kind = "Double" or kind = "LongInteger" then
            print name; "="; value
        else
            print name; " :"; kind
        end if
    end for
end sub

sub startPhase(name as string, url as string, format as string)
    print "-- "; name
    m.phase = name
    m.plays = 0
    content = CreateObject("roSGNode", "ContentNode")
    content.url = url
    content.streamFormat = format
    if m.video <> invalid then m.top.removeChild(m.video)
    m.video = m.top.createChild("Video")
    m.video.width = 640
    m.video.height = 360
    m.video.observeField("state", "onState")
    m.video.observeField("position", "onPosition")
    m.video.content = content
    m.video.control = "play"
end sub

sub onPosition()
    if m.phase = "clip" then m.positions.push(Int(m.video.position))
end sub

sub onState()
    state = m.video.state
    print m.phase; " "; state
    if state = "playing" then m.plays = m.plays + 1
    if m.phase = "clip" then
        if state = "playing" and m.plays = 1 then
            print "duration "; m.video.duration
            m.top.findNode("pauseTimer").control = "start"
        else if state = "paused" then
            print "paused after start "; m.video.position > 0
            m.video.control = "resume"
        else if state = "playing" and m.plays = 2 then
            m.top.findNode("seekTimer").control = "start"
        else if state = "finished" then
            print "finished at end "; m.video.position >= 5
            startPhase("hls", "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8", "hls")
        end if
    else if m.phase = "hls" then
        if state = "playing" and m.plays = 1 then
            print "duration "; Int(m.video.duration)
            m.top.findNode("stopTimer").control = "start"
        else if state = "stopped" then
            startPhase("missing", "https://test-streams.mux.dev/missing/missing.m3u8", "hls")
        end if
    else if m.phase = "missing" then
        if state = "error" then
            print "errorCode "; m.video.errorCode
            print "errorMsg "; m.video.errorMsg
            ' Keep the stable part; ":extra:" carries internal ids.
            detail = m.video.errorStr
            cut = Instr(1, detail, ":extra:")
            if cut > 0 then detail = Left(detail, cut - 1)
            print "errorStr "; detail
            printErrorInfo(m.video.errorInfo)
            print "-- done"
            m.top.done = true
        end if
    end if
end sub

sub onPauseTimer()
    m.video.control = "pause"
end sub

sub onStopTimer()
    m.video.control = "stop"
    print "after stop "; m.video.state
end sub

sub onSeekTimer()
    m.video.seek = 4
end sub

sub printErrorInfo(info as dynamic)
    if info = invalid then return
    keys = []
    for each key in info
        keys.push(key)
    end for
    keys.sort()
    for each key in keys
        value = info[key]
        ' clipid counts contents per app run; dbgmsg repeats errorStr
        ' with the internal ids after ":extra:".
        if key = "dbgmsg" then
            cut = Instr(1, value, ":extra:")
            if cut > 0 then value = Left(value, cut - 1)
        end if
        if key <> "clipid" then print "errorInfo "; key; "="; value
    end for
end sub
