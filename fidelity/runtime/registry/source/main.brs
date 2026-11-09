' roRegistry / roRegistrySection. Leaves the registry empty at the end.
sub Main()
    print "@@PROBE_BEGIN"
    registry = CreateObject("roRegistry")
    for each name in registry.GetSectionList()
        registry.Delete(name)
    end for
    registry.Flush()
    section = CreateObject("roRegistrySection", "probe")
    print type(registry), type(section)
    print section.Exists("token"), section.Read("token") = ""
    print section.Write("token", "abc"), section.Write("Alpha", "1")
    print section.Exists("token"), section.Read("token"), section.Exists("TOKEN")
    print section.GetKeyList()
    print section.WriteMulti({ x: "1", y: "2" })
    print section.ReadMulti(["x", "y", "nope"])
    print section.Delete("token"), section.Delete("token"), section.Exists("token")
    print section.Flush()
    print registry.GetSectionList()
    print registry.Delete("probe"), registry.GetSectionList().Count()
    registry.Flush()
    print type(registry.GetSpaceAvailable())
    print "@@PROBE_END"
end sub
