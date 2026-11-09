' Console lifecycle around a channel that exits by returning from Main.
' The OS-level lines (beacons) are outside the markers; the raw console log
' captured by the recorder is the reference for the emulator's launch/exit lines.
sub Main()
    print "@@PROBE_BEGIN"
    print "exiting"
    print "@@PROBE_END"
end sub
