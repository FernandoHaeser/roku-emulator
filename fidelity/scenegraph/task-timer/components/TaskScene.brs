sub init()
    m.timer = m.top.findNode("timer")
    print "timer control='"; m.timer.control; "' duration"; m.timer.duration
    m.timer.observeField("fire", "onFire")
    m.task = CreateObject("roSGNode", "Worker")
    m.task.observeField("state", "onState")
    m.task.observeField("result", "onResult")
    m.task.input = 20
    m.task.control = "RUN"
    print "task started"
end sub

sub onState()
    print "state "; m.task.state
end sub

sub onResult()
    print "result "; m.task.result
    m.timer.control = "start"
end sub

sub onFire()
    print "timer fired"
    print "@@PROBE_END"
end sub
