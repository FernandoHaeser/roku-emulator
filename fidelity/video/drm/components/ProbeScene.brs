sub init()
    m.phases = ["dash", "widevine", "badlicense", "nodrm"]
    m.next = 0
    m.top.findNode("stopTimer").observeField("fire", "onStopTimer")
    m.top.findNode("guardTimer").observeField("fire", "onGuardTimer")
    content = CreateObject("roSGNode", "ContentNode")
    print "content drmParams "; type(content.drmParams)
    nextPhase()
end sub

sub nextPhase()
    m.top.findNode("guardTimer").control = "stop"
    if m.video <> invalid then
        m.video.unobserveField("state")
        m.top.removeChild(m.video)
        m.video = invalid
    end if
    if m.next >= m.phases.count() then
        print "-- done"
        m.top.done = true
        return
    end if
    m.phase = m.phases[m.next]
    m.next = m.next + 1
    print "-- "; m.phase
    assets = "https://storage.googleapis.com/shaka-demo-assets/"
    content = CreateObject("roSGNode", "ContentNode")
    content.streamFormat = "dash"
    if m.phase = "dash" then
        content.url = assets + "angel-one/dash.mpd"
    else
        content.url = assets + "angel-one-widevine/dash.mpd"
    end if
    if m.phase = "widevine" then
        content.drmParams = { keySystem: "widevine", licenseServerURL: "https://cwip-shaka-proxy.appspot.com/no_auth" }
    else if m.phase = "badlicense" then
        content.drmParams = { keySystem: "widevine", licenseServerURL: assets + "missing-license" }
    end if
    m.video = m.top.createChild("Video")
    m.video.width = 640
    m.video.height = 360
    m.video.observeField("state", "onState")
    m.video.content = content
    m.video.control = "play"
    m.top.findNode("guardTimer").control = "start"
end sub

sub onState()
    state = m.video.state
    print m.phase; " "; state
    if state = "playing" then
        print "duration "; Int(m.video.duration)
        m.top.findNode("stopTimer").control = "start"
    else if state = "error" then
        print "errorCode "; m.video.errorCode
        print "errorMsg "; m.video.errorMsg
        ' Keep the stable part; ":extra:" carries internal ids.
        detail = m.video.errorStr
        cut = Instr(1, detail, ":extra:")
        if cut > 0 then detail = Left(detail, cut - 1)
        print "errorStr "; detail
        info = m.video.errorInfo
        if info <> invalid then
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
        end if
    else if state = "finished" or state = "stopped" then
        nextPhase()
    end if
end sub

sub onStopTimer()
    m.video.control = "stop"
end sub

sub onGuardTimer()
    print m.phase; " no final state, last "; m.video.state
    nextPhase()
end sub
