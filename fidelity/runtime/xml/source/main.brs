' roXMLElement parsing and navigation.
sub Main()
    print "@@PROBE_BEGIN"
    xml = CreateObject("roXMLElement")
    ok = xml.Parse("<rss version=""2.0""><channel><title>Feed</title><item id=""1""><name>A</name></item><item id=""2""><name>B &amp; C</name></item></channel></rss>")
    print ok, type(xml), xml.GetName()
    print xml.GetAttributes()
    print xml@version
    channel = xml.GetChildElements()[0]
    print channel.GetName(), channel.GetChildElements().Count()
    items = channel.GetNamedElements("item")
    print type(items), items.Count()
    print items[1]@id, items[1].GetNamedElements("name")[0].GetText()
    print channel.title.GetText(), channel.item.Count()
    bad = CreateObject("roXMLElement")
    print bad.Parse("<open>")
    print "@@PROBE_END"
end sub
