' roDeviceInfo: only non-identifying values (no serial, ids or names).
sub Main()
    print "@@PROBE_BEGIN"
    info = CreateObject("roDeviceInfo")
    print type(info)
    print info.GetModel(), info.GetModelDisplayName(), info.GetModelType()
    print info.GetDisplaySize()
    print info.GetUIResolution()
    print info.GetDisplayMode(), info.GetDisplayType(), info.GetDisplayAspectRatio()
    print info.GetVideoMode(), info.GetCurrentLocale(), info.GetCountryCode()
    print info.GetOSVersion()
    print info.CanDecodeVideo({ codec: "mpeg4 avc" }).result
    print info.IsRIDADisabled() <> invalid, type(info.GetConnectionType())
    print info.GetDrmInfo()
    print "@@PROBE_END"
end sub
