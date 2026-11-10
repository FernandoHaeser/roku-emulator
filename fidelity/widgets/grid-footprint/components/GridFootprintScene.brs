sub init()
 m.global.addField("cells","array",false)
 m.global.cells=[]
 m.phase=0
 m.grid=m.top.findNode("grid")
 print "focusUri=";m.grid.focusBitmapUri
 print "footprintUri=";m.grid.focusFootprintBitmapUri
 for each field in ["wrapDividerHeight","wrapDividerWidth","vertFocusAnimationStyle","drawFocusFeedbackOnTop"]
  print field;"=";FormatJSON(m.grid.getField(field))
 end for
 m.content=CreateObject("roSGNode","ContentNode")
 for i=1 to 5
  item=m.content.createChild("ContentNode")
  item.id="i"+i.toStr()
 end for
 m.grid.content=m.content
 m.grid.setFocus(true)
 m.timer=m.top.findNode("timer")
 m.timer.observeField("fire","tick")
 m.timer.control="start"
end sub
sub tick()
 r=m.grid.boundingRect()
 print "phase=";m.phase; " rect=";FormatJSON([r.x+0,r.y+0,r.width,r.height])
 if m.phase < 4
  samples=[]
  for each cell in m.global.cells
   t=cell.translation
   b=cell.boundingRect()
   world=cell.sceneBoundingRect()
   sample="cell="+cell.id+" t="+FormatJSON([t[0]+0,t[1]+0])+" r="+FormatJSON([b.x+0,b.y+0,b.width,b.height])+" scene="+FormatJSON([world.x+0,world.y+0,world.width,world.height])
   samples.push(sample)
  end for
  samples.sort()
  for each sample in samples
   print sample
  end for
 end if
 m.phase++
 if m.phase=1
  m.grid.numRows=99
  for i=1 to 3
   item=m.content.createChild("ContentNode")
  item.id="i"+i.toStr()
  end for
  m.grid.jumpToItem=7
 else if m.phase=2
  m.grid.numColumns=1
  m.grid.drawFocusFeedback=false
 else if m.phase=3
  m.grid.numColumns=3
  m.grid.numRows=2
  m.grid.jumpToItem=0
  m.grid.drawFocusFeedback=true
 else if m.phase=4
  m.grid.wrapDividerHeight=0
 else if m.phase=5
  m.grid.vertFocusAnimationStyle="floatingfocus"
 else if m.phase=6
  m.global.cells=[]
  m.content=CreateObject("roSGNode","ContentNode")
  item=m.content.createChild("ContentNode")
  item.id="single"
  m.grid.content=m.content
 else if m.phase=7
  m.grid.content=invalid
 else if m.phase=8
  for i=1 to 2
   m.content.createChild("ContentNode")
  end for
  m.grid.content=m.content
 else
  m.timer.control="stop"
  print "@@PROBE_END"
 end if
end sub
