' roAppInfo with values from the manifest.
sub Main()
    print "@@PROBE_BEGIN"
    info = CreateObject("roAppInfo")
    print type(info)
    print info.GetID(), info.IsDev(), info.GetTitle(), info.GetVersion()
    print info.GetValue("major_version"), info.GetValue("missing") = "", info.GetSubtitle() = ""
    print "@@PROBE_END"
end sub
