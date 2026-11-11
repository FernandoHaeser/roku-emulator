sub init()
 m.timer=m.top.findNode("timer")
 m.timer.observeField("fire","run")
 m.timer.control="start"
end sub
sub run()
 card=m.top.findNode("card")
 targetRect=card.findNode("box")
 print "-- parent write"
 card.width=80
 print "read=";card.width;",";card.mirror;",";targetRect.width
 print "-- child write"
 targetRect.width=45
 print "read=";card.width;",";card.mirror;",";targetRect.width
 card.caption="parent"
 print "text=";card.findNode("title").text
 card.findNode("title").text="nested"
 print "caption=";card.caption
 card.point=[3,4]
 print "point=";FormatJSON(targetRect.translation)
 targetRect.translation=[7,8]
 print "point=";FormatJSON(card.point)
 print "-- repeat"
 card.width=45
 print "-- cascade"
 card.width=90
 print "final=";card.width;",";card.mirror;",";targetRect.width
 print "-- port and alwaysNotify"
 port=CreateObject("roMessagePort")
 card.observeField("heightAlias",port)
 card.heightAlias=20
 card.heightAlias=25
 card.heightAlias=27
 for i=1 to 2
  event=wait(1000,port)
  if event=invalid
   print "port timeout"
  else
   print "port=";event.getData();" field=";event.getField();" node=";event.getRoSGNode().subtype();" id=";event.getNode()
  end if
 end for
 card.unobserveField("heightAlias")
 card.heightAlias=26
 print "unobserved=";wait(10,port)=invalid
 print "-- vector equality"
 card.observeField("point",port)
 card.point=[7,8]
 print "repeat point suppressed=";wait(10,port)=invalid
 card.point=[9,10]
 event=wait(1000,port)
 if event<>invalid then print "point event=";FormatJSON(event.getData());" node=";event.getRoSGNode().subtype()
 card.unobserveField("point")
 print "-- nested alias"
 outer=m.top.findNode("outer")
 outer.observeField("innerWidth",port)
 outer.innerWidth=52
 event=wait(1000,port)
 if event<>invalid then print "nested event=";event.getData();" field=";event.getField();" node=";event.getRoSGNode().subtype()
 print "nested=";outer.innerWidth;",";outer.findNode("box").width
 outer.innerCaption="outside"
 print "nested text=";outer.findNode("title").text
 explicit=m.top.findNode("explicit")
 print "explicit=";explicit.caption;",";explicit.findNode("inside").text
 print "-- clone child"
 copiedChild=targetRect.clone(true)
 copiedChild.width=71
 print "copied child=";copiedChild.width;",original=";card.width
 print "-- clone"
 cloned=card.clone(true)
 print "deep clone invalid=";cloned=invalid
 print "shallow clone invalid=";card.clone(false)=invalid
 print "explicit clone invalid=";explicit.clone(true)=invalid
 print "outer clone invalid=";outer.clone(true)=invalid
 translated=CreateObject("roSGNode","Translated")
 print "builtin alias initial=";FormatJSON(translated.translation);",child=";FormatJSON(translated.findNode("target").translation)
 translated.translation=[4,5]
 print "builtin alias write=";FormatJSON(translated.translation);",child=";FormatJSON(translated.findNode("target").translation)
 translatedCopy=translated.clone(true)
 print "builtin alias clone invalid=";translatedCopy=invalid
 if translatedCopy<>invalid
  print "builtin alias clone subtype=";translatedCopy.subtype()
  translatedCopy.translation=[11,12]
  print "builtin alias copied=";FormatJSON(translatedCopy.translation);",child=";FormatJSON(translatedCopy.findNode("target").translation);",original=";FormatJSON(translated.translation)
 end if
 print "-- remove alias"
 print "removed=";card.removeField("caption")
 print "has caption=";card.hasField("caption")
 print "added=";card.addField("caption","string",false)
 card.caption="independent"
 card.findNode("title").text="child only"
 print "after remove=";card.caption;",";card.findNode("title").text
 print "@@PROBE_END"
end sub
