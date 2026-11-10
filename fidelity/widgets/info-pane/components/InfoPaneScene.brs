sub init()
 m.phase=0
 m.pane=CreateObject("roSGNode","InfoPane")
 for each field in ["width","height","infoText","infoText2","bulletText","textColor","infoText2Color","infoText2BottomAlign"]
  print field;"=";FormatJSON(m.pane.getField(field))
 end for
 m.pane.translation=[100,200]
 m.pane.height=1
 m.pane.infoText="A"
 m.top.appendChild(m.pane)
 m.timer=m.top.findNode("timer")
 m.timer.observeField("fire","tick")
 m.timer.control="start"
end sub
sub tick()
 r=m.pane.boundingRect()
 print "phase=";m.phase;" type=";m.pane.subtype();" infoLabel=";m.pane.isSubtype("InfoLabel");" group=";m.pane.isSubtype("Group");" rect=";FormatJSON([r.x+0,r.y+0,r.width,r.height]);" children=";m.pane.getChildCount()
 if m.phase=0
  for each child in m.pane.getChildren(-1,0)
   if child.isSubtype("Label")
    print "label font=";child.font.uri;" size=";child.font.size;" spacing=";child.lineSpacing
   end if
  end for
 end if
 for i=0 to m.pane.getChildCount()-1
  child=m.pane.getChild(i)
  bounds=child.boundingRect()
  print "child=";i;" type=";child.subtype();" rect=";FormatJSON([bounds.x+0,bounds.y+0,bounds.width,bounds.height]);" visible=";child.visible
  if child.subtype()="BulletText"
   for j=0 to child.getChildCount()-1
    item=child.getChild(j)
    itemBounds=item.boundingRect()
    line=" item=" + j.toStr() + " type=" + item.subtype() + " rect=" + FormatJSON([itemBounds.x+0,itemBounds.y+0,itemBounds.width,itemBounds.height])
    if item.isSubtype("Label") or item.subtype()="Label"
     line=line + " text=" + FormatJSON(item.text) + " size=" + item.font.size.toStr()
    end if
    if item.subtype()="Poster"
     line=line + " uri=" + item.uri
    end if
    print line
    for k=0 to item.getChildCount()-1
     inner=item.getChild(k)
     innerBounds=inner.boundingRect()
     print "  inner=";k;" type=";inner.subtype();" rect=";FormatJSON([innerBounds.x+0,innerBounds.y+0,innerBounds.width,innerBounds.height])
    end for
   end for
  end if
 end for
 m.phase++
 if m.phase=1
  m.pane.infoText="A"+chr(10)+"B"
 else if m.phase=2
  m.pane.infoText="A"+chr(10)+chr(10)+"B"
 else if m.phase=3
  m.pane.height=150
  m.pane.infoText="Error:"+chr(10)+chr(10)+"Unable to connect."
 else if m.phase=4
  m.pane.infoText="Header"
  m.pane.bulletText=["One","Two"]
  m.pane.infoText2="Footer"
  m.pane.height=300
 else if m.phase=5
  m.pane.infoText2BottomAlign=true
 else if m.phase=6
  m.pane.width=400
  m.pane.bulletText=["One long bullet that wraps to another line","Two"]
 else if m.phase=7
  m.pane.bulletText=[]
  m.pane.infoText2=""
  m.pane.width=780
  m.pane.height=150
  m.pane.infoText="Error:"+chr(10)+chr(10)+"Unable to connect."
 else
  m.timer.control="stop"
  print "@@PROBE_END"
 end if
end sub
